use std::path::PathBuf;

use tokio::fs;

use crate::{
    deployment_hooks::NoopHooks,
    deployments::{manager::Manager, worker::WorkerHandle},
    env::EnvVars,
    paths::HostFile,
};

use super::{
    BuildResult, Container, ContainerConfig, ContainerSetup, ContainerStatus, ContextBuilderOutput,
    FileSystemOutput,
};

const PRISMA_DOCKERFILE: &'static str = include_str!("../../resources/prisma.Dockerfile");

#[derive(Clone, Debug)]
pub(crate) struct PrismaContainer {}

impl PrismaContainer {
    pub(crate) fn new(db_file: HostFile, build_queue: WorkerHandle) -> Container {
        let builder = Self {};

        Container::new(
            builder,
            ContainerConfig {
                args: EnvVars::empty(),
                host_files: vec![db_file.clone()],
                env: [(
                    "DATABASE_URL",
                    db_file.get_container_file().to_str().unwrap(),
                )]
                .as_ref() // FIXME: should not need this
                .into(),
                initial_status: ContainerStatus::Built, // TODO: maybe I need a different status for this? it's true that I can assume this is always build successfully
                result: Some(BuildResult::Built),
            },
            build_queue,
            None,
            false, // public,
            NoopHooks,
        )
    }
    async fn build_context(path: PathBuf) -> anyhow::Result<PathBuf> {
        let dockerfile = path.join("Dockerfile");
        fs::write(dockerfile, PRISMA_DOCKERFILE).await?;
        Ok(path)
    }
}

impl ContainerSetup for PrismaContainer {
    fn setup_build_context(&self, path: PathBuf) -> ContextBuilderOutput {
        Box::pin(async { Self::build_context(path).await })
    }

    fn setup_filesystem(&self) -> FileSystemOutput {
        Box::pin(async { Ok(()) })
    }
}
