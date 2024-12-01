use std::{future::Future, sync::Arc};

use futures::future::join_all;
use rand::seq::SliceRandom;
use tokio::sync::RwLock;

use crate::{
    container::{Container, ContainerStatus},
    db::Db,
    deployments::{
        map::DeploymentMap,
        worker::{Worker, WorkerHandle},
    },
    github::Github,
};

#[derive(Clone)]
pub(crate) struct BuildWorker {
    // TODO: define a new function instead of having these public, same for other workers
    pub(crate) map: Arc<RwLock<DeploymentMap>>,
    pub(crate) db: Db,
    pub(crate) github: Github,
    pub(crate) build_queue: WorkerHandle,
}

impl Worker for BuildWorker {
    fn work(&self) -> impl Future<Output = ()> + Send {
        async {
            loop {
                if let Some(container) = self.get_container_to_build().await {
                    container.setup_as_standby().await;
                    self.map
                        .write()
                        .await
                        .read_db_and_build_updates(&self.build_queue, &self.github, &self.db)
                        .await;
                } else {
                    break;
                }
            }
        }
    }
}

impl BuildWorker {
    async fn get_container_to_build(&self) -> Option<Arc<Container>> {
        // this block helds this read guard
        let map = self.map.read().await;
        // TODO: use stream
        let futures = map.iter_containers().map(|container| async {
            let status = container.status.read().await.clone();
            (container, status)
        });

        let queued_containers = join_all(futures)
            .await
            .into_iter()
            .filter_map(|(container, status)| {
                if let ContainerStatus::Queued { trigger_access } = status {
                    Some((container, trigger_access))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        let first_container_accessed = queued_containers
            .iter()
            .filter_map(|(container, trigger_access)| Some((container, trigger_access.clone()?)))
            .min_by_key(|(_, trigger_access)| trigger_access.clone())
            .map(|(container, _)| container);

        if first_container_accessed.is_some() {
            first_container_accessed.cloned()
        } else {
            // if no container is accessed, just return a random one
            queued_containers
                .choose(&mut rand::thread_rng())
                .map(|(container, _)| container.clone())
        }
    }
}