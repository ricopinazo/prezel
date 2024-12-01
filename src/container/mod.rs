use anyhow::{anyhow, bail};
use async_trait::async_trait;
use futures::lock::{Mutex, MutexGuard};
use http::StatusCode;
use std::{
    fmt,
    future::Future,
    net::SocketAddrV4,
    ops::Deref,
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
    time::{Duration, Instant},
};
use tempfile::TempDir;
use tokio::{sync::RwLock, time::sleep};

use crate::{
    // db::Status,
    api::Status,
    db::BuildResult,
    deployment_hooks::DeploymentHooks,
    deployments::{manager::Manager, worker::WorkerHandle},
    docker::{
        build_dockerfile, create_container, delete_container, delete_image,
        get_bollard_container_ipv4, get_container_execution_logs, run_container, stop_container,
        DockerLog,
    },
    env::EnvVars,
    listener::{Access, Listener},
    paths::HostFile,
};

pub(crate) mod commit;
pub(crate) mod prisma;

#[derive(Debug)]
pub(crate) struct ContainerConfig {
    pub(crate) env: EnvVars,
    pub(crate) args: EnvVars,
    pub(crate) host_files: Vec<HostFile>,
    pub(crate) initial_status: ContainerStatus,
    pub(crate) result: Option<BuildResult>,
}

pub(crate) type ContextBuilderOutput =
    Pin<Box<dyn Future<Output = anyhow::Result<PathBuf>> + Send>>;
pub(crate) type FileSystemOutput = Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>>;

pub(crate) trait ContainerSetup: 'static + Send + Sync + fmt::Debug {
    fn setup_build_context(&self, path: PathBuf) -> ContextBuilderOutput; // TODO: make this return a TempDir!!!!
    fn setup_filesystem(&self) -> FileSystemOutput;
}

#[derive(Debug, Clone)]
pub(crate) enum ContainerStatus {
    // FIXME ideally I would save the image in the db as well, since anyways there is no guarantee
    // the image pointed by StandBy {image} exists
    // I could store the image id in case the build was successful
    // if there is build_start and build_end but not image id, I can assume the build failed
    // To be fair, if I detect an image has been removed, I should change the status to be None...
    /// this means the container was previously built successfully but the image is not known anymore
    Built,
    StandBy {
        image: String,
    },
    Queued {
        trigger_access: Option<Instant>,
    },
    Building,
    Ready {
        image: String,
        container: String,
        socket: SocketAddrV4,
        last_access: Arc<RwLock<Instant>>,
    },
    Failed,
}

#[derive(Debug)]
pub(crate) struct AtomicStatus {
    status: Arc<RwLock<ContainerStatus>>,
    lock: Mutex<()>,
}

impl From<ContainerStatus> for AtomicStatus {
    fn from(value: ContainerStatus) -> Self {
        Self {
            status: RwLock::new(value).into(),
            lock: Default::default(),
        }
    }
}

impl AtomicStatus {
    pub(crate) async fn read(&self) -> tokio::sync::RwLockReadGuard<'_, ContainerStatus> {
        self.status.read().await
    }

    async fn aquire(&self) -> WritableStatus {
        let lock = self.lock.lock().await;

        WritableStatus {
            status: self.status.clone(),
            lock,
        }
    }
}

#[derive(Debug)]
struct WritableStatus<'a> {
    pub(crate) status: Arc<RwLock<ContainerStatus>>, // access this to read/write the status
    lock: MutexGuard<'a, ()>,
}

impl<'a> Deref for WritableStatus<'a> {
    type Target = RwLock<ContainerStatus>;

    fn deref(&self) -> &Self::Target {
        &self.status
    }
}

impl ContainerStatus {
    fn get_container_id(&self) -> Option<String> {
        if let Self::Ready { container, .. } = self {
            Some(container.clone())
        } else {
            None
        }
    }

    pub(crate) fn to_status(&self) -> Status {
        match self {
            Self::Built => Status::Built,
            Self::StandBy { .. } => Status::StandBy,
            Self::Building => Status::Building,
            Self::Queued { .. } => Status::Queued,
            Self::Ready { .. } => Status::Ready,
            Self::Failed => Status::Failed,
        }
    }

    // fn is_not_built(&self) -> bool {
    //     match self {
    //         Self::Built => true, // FIXME: this seems like a contradiction!!
    //         _ => false,
    //     }
    // }

    // pub(crate) fn is_built(&self) -> bool {
    //     match self {
    //         Self::StandBy { .. } | Self::Ready { .. } => true,
    //         Self::Built | Self::Queued { .. } | Self::Failed { .. } | Self::Building => false,
    //     }
    // }
}

// Potential problems ot be aware of
// - Two builds should not be started at the same time for the same container
// - Two docker containers should not be created at the same time for the same container
// - A container that is waiting to be created should not be removed imediately if there are clients actively requesting it

/// This is a lazy container, it might be that it is not running, or even that the container is deleted, or even that
/// the image itself to create this container was deleted, and it will make sure, upon access, to rebuild the image / start the container
/// before responding with the socket address
#[derive(Debug)]
pub(crate) struct Container {
    pub(crate) status: AtomicStatus,
    pub(crate) result: RwLock<Option<BuildResult>>,
    setup: Box<dyn ContainerSetup>,
    config: ContainerConfig,
    hooks: Box<dyn DeploymentHooks>,
    pub(crate) logging_deployment_id: Option<i64>,
    pub(crate) public: bool,
    build_queue: WorkerHandle,
}

impl Container {
    // TODO: remove this function and just make the private fields public within the module (they are already no?)
    pub(crate) fn new(
        setup: impl ContainerSetup,
        config: ContainerConfig,
        build_queue: WorkerHandle,
        logging_deployment_id: Option<i64>,
        public: bool,
        hooks: impl DeploymentHooks,
    ) -> Self {
        Self {
            status: config.initial_status.clone().into(),
            result: RwLock::new(config.result),
            setup: Box::new(setup),
            config,
            hooks: Box::new(hooks),
            logging_deployment_id,
            public,
            build_queue,
        }
    }

    // TODO: review, do we really need to expose the container id in the api?
    pub(crate) async fn get_container_id(&self) -> Option<String> {
        self.status.read().await.get_container_id()
    }

    pub(crate) async fn get_logs(&self) -> Box<dyn Iterator<Item = DockerLog>> {
        if let Some(container) = self.get_container_id().await {
            Box::new(get_container_execution_logs(&container).await)
        } else {
            Box::new(std::iter::empty())
        }
    }

    /// this function runs no sanity checks on the current status before setting the new one
    pub(crate) async fn enqueue(&self) {
        let status = self.status.aquire().await;
        *status.write().await = ContainerStatus::Queued {
            trigger_access: None,
        };
    }

    pub(crate) async fn setup_as_standby(&self) -> anyhow::Result<()> {
        self.build().await?;
        self.setup.setup_filesystem().await?; // FIXME: wtf is this and why am I not calling it when I do access?????????
        Ok(())
    }

    pub(crate) async fn downgrade_if_unused(&self) {
        let status = self.status.aquire().await;

        let new_status = if let ContainerStatus::Ready {
            image, last_access, ..
        } = status.read().await.deref()
        {
            let last_access = last_access.read().await;
            let elapsed = Instant::now().checked_duration_since(*last_access);
            if elapsed.is_some_and(|elapsed| elapsed > Duration::from_secs(30)) {
                Some(ContainerStatus::StandBy {
                    image: image.clone(),
                })
            } else {
                None
            }
        } else {
            None
        };

        if let Some(new_status) = new_status {
            *status.write().await = new_status;
        }
    }

    // TODO: remove, not using this
    // pub(crate) async fn full_delete(&self) -> anyhow::Result<()> {
    //     self.delete().await?;
    //     self.delete_image().await
    // }

    // TODO: remove, not using this
    // pub(crate) async fn delete(&self) -> anyhow::Result<()> {
    //     let status = self.status.aquire().await;

    //     let current = if let ContainerStatus::Ready {
    //         image, container, ..
    //     } = status.read().await.deref()
    //     {
    //         Some((image.clone(), container.clone()))
    //     } else {
    //         None
    //     };

    //     if let Some((image, container)) = current {
    //         stop_container(&container).await?;
    //         delete_container(&container).await?;
    //         *status.write().await = ContainerStatus::StandBy { image };
    //     }
    //     Ok(())
    // }

    // TODO: remove, not using this
    // pub(crate) async fn delete_image(&self) -> anyhow::Result<()> {
    //     let status = self.status.aquire().await;

    //     let image = status.read().await.get_undeployed_image();

    //     if let Some(image) = image {
    //         delete_image(&image).await?;
    //         *status.write().await = ContainerStatus::None;
    //     }

    //     Ok(())
    // }

    async fn build(&self) -> anyhow::Result<()> {
        let status = self.status.aquire().await;
        self.hooks.on_build_started().await;
        *status.write().await = ContainerStatus::Building;

        match self.build_with_result().await {
            Ok(image) => {
                self.hooks.on_build_finished().await;
                *status.write().await = ContainerStatus::StandBy { image };
                *self.result.write().await = Some(BuildResult::Built);
            }
            Err(error) => {
                self.hooks.on_build_failed().await;
                *status.write().await = ContainerStatus::Failed;
                *self.result.write().await = Some(BuildResult::Failed);
            }
        }
        Ok(())
    }

    // TODO: rename this
    async fn build_with_result(&self) -> anyhow::Result<String> {
        let tempdir = TempDir::new()?;
        let path = tempdir.as_ref();
        let path = self.setup.setup_build_context(path.to_path_buf()).await?;
        let image = build_dockerfile(&path, self.config.args.clone(), &mut |chunk| async {
            if let Some(stream) = chunk.stream {
                self.hooks.on_build_log(&stream, false).await
            } else if let Some(error) = chunk.error {
                self.hooks.on_build_log(&error, true).await
            }
        })
        .await?;

        Ok(image)
    }

    pub(crate) async fn start(&self) -> anyhow::Result<SocketAddrV4> {
        let status = self.status.aquire().await;
        let cloned_status = status.read().await.clone();
        if let ContainerStatus::StandBy { image } = cloned_status {
            let container = create_container(
                image.clone(),
                self.config.env.clone(),
                self.config.host_files.iter(),
            )
            .await?;
            run_container(&container).await?;

            let ip = get_bollard_container_ipv4(&container)
                .await
                .ok_or(anyhow!("Could not get IP for container"))?;
            let socket = SocketAddrV4::new(ip, 80);
            while !is_online(&socket.to_string()).await {
                sleep(Duration::from_millis(200)).await;
            }

            // FIXME: this will deadlock as status has a read lock on it
            // what im doing seems fundamentally wrong
            // maybe I should not be able to create a read lock on a WritableStatus
            *status.write().await = ContainerStatus::Ready {
                image: image.clone(),
                container,
                socket,
                last_access: RwLock::new(Instant::now()).into(),
            };

            Ok(socket)
        } else if let ContainerStatus::Ready { socket, .. } = cloned_status {
            Ok(socket)
        } else {
            bail!("Tried to start container in a state different than StandBy")
        }
    }

    // async fn commit_access(&self) -> anyhow::Result<RwLockReadGuard<ContainerStatus>> {
    //     let status = self.status.read().await;
    //     if let ContainerStatus::Ready {last_access, ..} = status.deref() {
    //         Ok(*last_access.write().await = Instant::now())
    //             Ok(status)
    //     } else {
    //         bail!("the status is other than Ready")
    //     }
    // }
}

#[async_trait]
impl Listener for Arc<Container> {
    fn is_public(&self) -> bool {
        self.public
    }

    async fn access(&self) -> anyhow::Result<Access> {
        let socket = match self.status.read().await.deref() {
            ContainerStatus::Ready {
                socket,
                last_access,
                ..
            } => {
                *last_access.write().await = Instant::now();
                Some(socket.clone())
            }
            _ => None,
        };

        // all of this duplication is just to avoid holding a read lock onto the status...
        // wait but Im creating it anyways

        // FIXME: should aquire the lock at this point, so even if allow reads,
        // I know no one is going to be changing the status in the meantime.
        // If so, I should pass the WritableStatus down to container.start() / container.build()
        //
        // FIXME: instead of AtomicStatus, I don't think it is the end of the world aquiring a write lock on the status for a container that is not in prod in ready mode

        match socket {
            Some(socket) => Ok(Access::Socket(socket.clone())),
            None => {
                let status = self.status.read().await.clone();
                match status {
                    ContainerStatus::Ready {
                        socket,
                        last_access,
                        ..
                    } => {
                        // FIXME: boilerplate in here
                        *last_access.write().await = Instant::now();
                        Ok(Access::Socket(socket.clone()))
                    }
                    ContainerStatus::StandBy { .. } => {
                        let socket = self.start().await?;
                        Ok(Access::Socket(socket))
                    }
                    ContainerStatus::Built => {
                        *self.status.aquire().await.write().await = ContainerStatus::Queued {
                            trigger_access: Some(Instant::now()),
                        };
                        self.build_queue.trigger();
                        Ok(Access::Loading)
                    }
                    ContainerStatus::Queued { .. } | ContainerStatus::Building => {
                        Ok(Access::Loading)
                    }
                    ContainerStatus::Failed => {
                        bail!("container failed to build")
                    }
                }
            }
        }
    }
}

// FIXME: this might fail, especially for some API server with no / route
// there has to be another way
async fn is_online(host: &str) -> bool {
    let url = format!("http://{host}");
    let response = reqwest::get(url).await;
    match response {
        Ok(response) => response.status() == StatusCode::OK,
        Err(_) => false,
    }
}

// TODO: !!!!!!!!!!!!!!
// impl Drop for Container {
//     fn drop(&mut self) {
//         todo!()
//     }
// }