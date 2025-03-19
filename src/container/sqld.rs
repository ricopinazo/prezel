use std::path::PathBuf;

use crate::{deployments::worker::WorkerHandle, hooks::NoopHooks, sqlite_db::SqliteDbSetup};

use super::{BuildResult, Container, ContainerConfig, ContainerSetup, ContainerStatus};

const VERSION: &str = "0.24.28";

#[derive(Clone, Debug)]
pub(crate) struct SqldContainer;

impl SqldContainer {
    #[tracing::instrument]
    pub(crate) fn new(db_folder: PathBuf, key: &str, build_queue: WorkerHandle) -> Container {
        let builder = Self {};
        let db_path = db_folder.display().to_string();
        Container::new(
            builder,
            ContainerConfig {
                host_folders: vec![db_folder.clone()],
                pull: true,
                env: [
                    ("SQLD_HTTP_LISTEN_ADDR", "0.0.0.0:80"),
                    ("SQLD_DB_PATH", &db_path),
                    ("SQLD_AUTH_JWT_KEY", key),
                ]
                .as_ref() // FIXME: should not need this
                .into(),
                initial_status: ContainerStatus::StandBy {
                    image: format!("ghcr.io/tursodatabase/libsql-server:v{VERSION}"),
                    db_setup: None,
                },
                command: None,
                result: Some(BuildResult::Built),
            },
            build_queue,
            None,
            true, // FIXME: make sure I handle auth at the
            NoopHooks,
        )
    }
}

// FIXME: this being empty clearly means the abstraction is pointless
impl ContainerSetup for SqldContainer {
    fn setup_db<'a>(
        &'a self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = anyhow::Result<Option<SqliteDbSetup>>> + Send + 'a>,
    > {
        todo!()
    }
    fn build<'a>(
        &'a self,
        _hooks: &'a Box<dyn super::DeploymentHooks>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<String>> + Send + 'a>>
    {
        todo!()
    }
}
