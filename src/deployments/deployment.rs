use std::future::Future;
use std::path::{Component, Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;

use futures::{stream, Stream, StreamExt};
use serde::Deserialize;

use crate::container::commit::CommitContainer;
use crate::container::ContainerStatus;
use crate::db::{BuildResult, Deployment as DbDeployment};
use crate::deployment_hooks::StatusHooks;
use crate::label::Label;
use crate::sqlite_db::ProdSqliteDb;
use crate::{
    container::Container,
    db::{Db, DeploymentWithProject},
    github::Github,
};

use super::worker::WorkerHandle;

#[derive(Debug, Clone)]
pub(crate) struct Deployment {
    pub(crate) branch: String,
    pub(crate) default_branch: bool,
    pub(crate) sha: String,
    pub(crate) id: i64,
    pub(crate) project: i64,
    pub(crate) url_id: String,
    pub(crate) timestamp: i64,
    pub(crate) created: i64,
    pub(crate) forced_prod: bool, // TODO: review if im using this
    pub(crate) app_container: Arc<Container>, // FIXME: try to remove Arc, only needed to make access to socket/public generic
}

#[derive(Deserialize)]
enum Visibility {
    standard,
    public,
    private,
}

#[derive(Deserialize, Default)]
struct DeploymentConfig {
    visibility: Option<Visibility>,
}

impl DeploymentConfig {
    // FIXME: if there's just a network problem, I might interpret as there is no file
    // and make the prod deployment public when it was indeed set to private in the repo
    async fn fetch_from_repo(github: &Github, repo_id: i64, sha: &str, root: &str) -> Option<Self> {
        let conf_path = PathBuf::from(root).join("prezel.json");
        let valid_components = conf_path
            .components()
            .filter(|comp| !matches!(comp, Component::CurDir));
        let valid_path: PathBuf = valid_components.collect();
        let content = github
            .download_file(repo_id, &sha, valid_path.to_str()?)
            .await
            .ok()?;
        serde_json::from_str(&content).ok()
    }

    fn get_visibility(&self) -> Visibility {
        self.visibility.unwrap_or(Visibility::standard)
    }

    // fn get_visibility_for_prod_deployment(&self) {
    //     match self.visibility.unwrap_or(Visibility::standard) {
    //         Visibility::standard => true,
    //         Visibility::public => true,
    //         Visibility::private => false,
    //     }
    // }

    // fn get_visibility_for_preview_deployment(&self) {
    //     match self.visibility.unwrap_or(Visibility::standard) {
    //         Visibility::standard => false,
    //         Visibility::public => true,
    //         Visibility::private => false,
    //     }
    // }
}

impl Deployment {
    pub(crate) fn iter_arc_containers(&self) -> impl Stream<Item = Arc<Container>> + Send + '_ {
        let containers: [Pin<Box<dyn Future<Output = Option<Arc<Container>>> + Send>>; 2] = [
            Box::pin(async { Some(self.app_container.clone()) }),
            Box::pin(async {
                self.app_container
                    .status
                    .read()
                    .await
                    .get_db_setup()
                    .map(|setup| setup.container.clone())
            }),
        ];
        stream::iter(containers).filter_map(|container| container)
    }

    pub(crate) async fn new(
        deployment: DeploymentWithProject,
        build_queue: WorkerHandle,
        github: Github,
        db: Db,
        project_db: &ProdSqliteDb,
    ) -> Self {
        let DeploymentWithProject {
            deployment,
            project,
        } = deployment;
        let default_branch = deployment.is_default_branch();
        let DbDeployment {
            sha,
            env,
            branch,
            id,
            url_id,
            timestamp,
            created,
            ..
        } = deployment;

        let conf = DeploymentConfig::fetch_from_repo(&github, project.repo_id, &sha, &project.root)
            .await
            .unwrap_or_default();
        let is_public = match conf.get_visibility() {
            Visibility::standard => default_branch,
            Visibility::public => true,
            Visibility::private => false,
        };

        let env = env.into();
        let hooks = StatusHooks::new(db, id);

        let (inistial_status, build_result) = match deployment.result {
            Some(BuildResult::Failed) => (ContainerStatus::Failed, Some(BuildResult::Failed)),
            Some(BuildResult::Built) => (ContainerStatus::Built, Some(BuildResult::Built)),
            _ => (
                ContainerStatus::Queued {
                    trigger_access: None,
                },
                None,
            ),
        };

        let is_branch_deployment = !default_branch;
        let commit_container = CommitContainer::new(
            build_queue.clone(),
            hooks,
            github,
            project.repo_id,
            sha.clone(),
            id,
            env,
            project.root.clone(),
            is_branch_deployment,
            is_public,
            project_db,
            inistial_status,
            build_result,
        );

        Self {
            branch,
            default_branch,
            sha,
            id,
            project: project.id,
            url_id,
            timestamp,
            created,
            forced_prod: project.prod_id.is_some_and(|prod_id| id == prod_id),
            app_container: commit_container.into(),
        }
    }

    pub(crate) fn get_app_hostname(&self, box_domain: &str, project_name: &str) -> String {
        Label::Deployment {
            project: project_name.to_string(),
            deployment: self.url_id.to_string(),
        }
        .format_hostname(box_domain)
    }

    pub(crate) fn get_prod_hostname(&self, box_domain: &str, project_name: &str) -> String {
        Label::Prod {
            project: project_name.to_string(),
        }
        .format_hostname(box_domain)
    }
}
