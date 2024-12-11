use nixpacks::{
    create_docker_image,
    nixpacks::{builder::docker::DockerBuilderOptions, plan::generator::GeneratePlanOptions},
};
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use tokio::fs;

use crate::{deployment_hooks::StatusHooks, env::EnvVars, github::Github, paths::HostFile};

use super::{
    BuildResult, Container, ContainerConfig, ContainerSetup, ContainerStatus, ContextBuilderOutput,
    FileSystemOutput, WorkerHandle,
};

const DB_PATH_ENV_NAME: &str = "PREZEL_DB_URL";

#[derive(Clone, Debug)]
pub(crate) struct CommitContainer {
    github: Github,
    main_db_file: HostFile,
    cloned_db_file: Option<HostFile>,
    pub(crate) repo_id: String,
    pub(crate) sha: String,
    env: EnvVars,
    root: String,
}

impl CommitContainer {
    pub(crate) fn new(
        build_queue: WorkerHandle,
        hooks: StatusHooks,
        github: Github,
        repo_id: String,
        sha: String,
        deployment: i64,
        env: EnvVars, // TODO: this is duplicated in ContainerConfig...
        root: String,
        public: bool, // TODO: should not this be in ContainerConfig
        main_db_file: HostFile,
        cloned_db_file: Option<HostFile>,
        initial_status: ContainerStatus,
        result: Option<BuildResult>,
    ) -> Container {
        let db_file = cloned_db_file
            .clone()
            .unwrap_or_else(|| main_db_file.clone());
        let db_path = db_file.get_container_file();
        let db_path_str = db_path.to_str().unwrap();
        let default_env = [
            (DB_PATH_ENV_NAME, format!("file:{db_path_str}").as_str()),
            ("HOST", "0.0.0.0"),
            ("PORT", "80"),
        ]
        .as_ref()
        .into();
        let extended_env = env + default_env;

        let builder = Self {
            github,
            main_db_file,
            cloned_db_file,
            repo_id,
            sha,
            env: extended_env.clone(),
            root,
        };

        Container::new(
            builder,
            ContainerConfig {
                args: extended_env.clone(),
                host_files: vec![db_file],
                env: extended_env,
                initial_status,
                result,
            },
            build_queue,
            Some(deployment),
            public,
            hooks,
        )
    }
    async fn build_context(&self, path: &Path) -> anyhow::Result<PathBuf> {
        self.github
            .download_commit(&self.repo_id, &self.sha, &path)
            .await
            .unwrap(); // FIXME: this should result in an Error type that causes the build tu be cancelled and retry later on
        assert!(path.exists());

        let inner_path = path.join(&self.root);

        let env_vec: Vec<String> = self.env.clone().into();
        create_docker_image(
            inner_path.to_str().unwrap(),
            env_vec.iter().map(String::as_str).collect(),
            &GeneratePlanOptions::default(),
            &DockerBuilderOptions {
                out_dir: Some(inner_path.to_str().unwrap().to_owned()), // TODO: test what happens if I omit this ?
                // name: Some(name.clone()),
                // print_dockerfile: false,
                // quiet: false,
                // cache_key: None,
                // no_cache: true,
                // inline_cache: false,
                // platform: vec![],
                // current_dir: true,
                // no_error_without_start: true,
                // verbose: true,
                // docker_host: Some("unix:///var/run/docker.sock".to_owned()),
                ..Default::default()
            },
        )
        .await?;

        // FIXME: what if there is a Dockerfile in the root folder of the project????
        // maybe I shouldnt use nixpacks at all in the first place
        fs::rename(
            inner_path.join(".nixpacks").join("Dockerfile"),
            inner_path.join("Dockerfile"),
        )
        .await?;
        Ok(inner_path)
    }
}

impl ContainerSetup for CommitContainer {
    fn setup_build_context(&self, path: PathBuf) -> ContextBuilderOutput {
        let builder = self.clone();
        Box::pin(async move { builder.build_context(&path).await })
    }

    fn setup_filesystem(&self) -> FileSystemOutput {
        let main_db_path = self.main_db_file.get_container_file();
        let cloned_db_file = self.cloned_db_file.clone();

        Box::pin(async move {
            if !main_db_path.exists() {
                fs::File::create_new(&main_db_path).await?;
            }
            if let Some(cloned_db_file) = cloned_db_file {
                let cloned_db_path = cloned_db_file.get_container_file();
                fs::copy(main_db_path, cloned_db_path).await?;
            }
            Ok(())
        })
    }
}
