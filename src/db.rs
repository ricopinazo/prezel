use std::{collections::HashMap, ops::Deref, sync::Arc};

use futures::{future::join_all, stream, StreamExt};
use nanoid::nanoid;
use serde::Deserialize;
use sqlx::{sqlite::SqlitePool, FromRow, Pool, Sqlite};
use tracing::info;
use utoipa::ToSchema;

use crate::{
    alphabet,
    paths::get_instance_db_path,
    time::{self, now},
};

#[derive(sqlx::Type, PartialEq, Clone, Copy, Debug)]
#[sqlx(rename_all = "lowercase")]
pub(crate) enum BuildResult {
    Built,
    Failed,
}

#[derive(Clone, Debug)]
pub(crate) struct PlainProject {
    pub(crate) id: i64,
    pub(crate) name: String,
    pub(crate) repo_id: String,
    pub(crate) created: i64,
    pub(crate) env: String,
    pub(crate) root: String,
    pub(crate) prod_id: Option<i64>,
}

#[derive(Clone, Debug)]
pub(crate) struct Project {
    pub(crate) id: i64,
    pub(crate) name: String,
    pub(crate) repo_id: String,
    pub(crate) created: i64,
    pub(crate) env: String,
    pub(crate) root: String,
    pub(crate) prod_id: Option<i64>,
    pub(crate) custom_domains: Vec<String>,
}

impl Project {
    fn new(project: PlainProject, custom_domains: Vec<String>) -> Self {
        Self {
            id: project.id,
            name: project.name,
            repo_id: project.repo_id,
            created: project.created,
            env: project.env,
            root: project.root,
            prod_id: project.prod_id,
            custom_domains,
        }
    }
}

#[derive(Deserialize, Debug, ToSchema)]
pub(crate) struct InsertProject {
    pub(crate) name: String,
    pub(crate) repo_id: String,
    pub(crate) env: String,
    pub(crate) root: String,
}

#[derive(Deserialize, Debug, ToSchema)]
pub(crate) struct UpdateProject {
    name: Option<String>,
    env: Option<String>,
    custom_domains: Option<Vec<String>>,
}

// #[derive(Clone, Debug)]
// pub(crate) struct DeploymentWithProject {
//     pub(crate) id: String,
//     pub(crate) project_name: String,
//     pub(crate) project_id: i64,
//     pub(crate) repo_id: String,
//     pub(crate) sha: String,
//     pub(crate) env: String,
//     pub(crate) pr: Option<i64>,
//     pub(crate) created: i64,
// }

#[derive(FromRow)]
pub(crate) struct Deployment {
    pub(crate) id: i64,
    pub(crate) url_id: String,
    pub(crate) timestamp: i64,
    pub(crate) created: i64,
    pub(crate) env: String,
    pub(crate) sha: String,
    pub(crate) branch: String, // I might need to have here a list of prs somehow
    pub(crate) default_branch: i64,
    pub(crate) result: Option<BuildResult>,
    pub(crate) build_started: Option<i64>,
    pub(crate) build_finished: Option<i64>,
    pub(crate) project: i64,
}

impl Deployment {
    pub(crate) fn is_default_branch(&self) -> bool {
        self.default_branch != 0
    }
}

#[derive(FromRow)]
pub(crate) struct BuildLog {
    pub(crate) id: i64,
    pub(crate) content: String,
    pub(crate) timestamp: i64,
    pub(crate) error: i64,
    pub(crate) deployment: i64,
}

pub(crate) struct DeploymentWithProject {
    pub(crate) deployment: Deployment,
    pub(crate) project: Arc<Project>,
}

impl Deref for DeploymentWithProject {
    type Target = Deployment;

    fn deref(&self) -> &Self::Target {
        &self.deployment
    }
}

pub(crate) struct InsertDeployment {
    pub(crate) env: String,
    pub(crate) sha: String,
    pub(crate) timestamp: i64,
    pub(crate) branch: String,
    pub(crate) default_branch: i64,
    pub(crate) project: i64,
}

fn create_deployment_url_id() -> String {
    nanoid!(10, &alphabet::LOWERCASE_PLUS_NUMBERS)
}

#[derive(Clone, Debug)]
pub(crate) struct Db {
    conn: Pool<Sqlite>, // TODO: put this in a module with db.rs and make this provate
}

impl Db {
    pub(crate) async fn setup() -> Self {
        let db_path = get_instance_db_path();
        let db_path_str = db_path.to_str().expect("Path to DB coud not be generated");

        // ensure that the db file exists
        // create_dir_all(app_dir).unwrap();
        if !db_path.exists() {
            std::fs::File::create_new(&db_path).unwrap();
        }

        let conn = SqlitePool::connect(db_path_str)
            .await
            .expect("Failed to connect to the app DB");

        sqlx::migrate!("./migrations").run(&conn).await.unwrap();

        info!(
            "db setup at {}",
            db_path.canonicalize().unwrap().to_str().unwrap()
        );

        Self { conn }
    }

    // TODO: try to make the manager have access only to the read methods in here
    pub(crate) async fn get_project(&self, id: i64) -> Option<Project> {
        let project = sqlx::query_as!(
            PlainProject,
            "select * from projects where projects.id = ?",
            id
        )
        .fetch_optional(&self.conn)
        .await
        .unwrap()?;
        Some(self.append_custom_domains(project).await)
    }

    pub(crate) async fn get_project_by_name(&self, name: &str) -> Option<Project> {
        let project = sqlx::query_as!(
            PlainProject,
            "select * from projects where projects.name = ?",
            name
        )
        .fetch_optional(&self.conn)
        .await
        .unwrap()?;
        Some(self.append_custom_domains(project).await)
    }

    pub(crate) async fn get_projects(&self) -> Vec<Project> {
        let projects = sqlx::query_as!(PlainProject, "select * from projects")
            .fetch_all(&self.conn)
            .await
            .unwrap();

        stream::iter(projects)
            .then(|project| self.append_custom_domains(project))
            .collect()
            .await
    }

    async fn append_custom_domains(&self, project: PlainProject) -> Project {
        let custom_domains = sqlx::query!("select * from domains where project = ?", project.id)
            .fetch_all(&self.conn)
            .await
            .unwrap()
            .into_iter()
            .map(|record| record.domain)
            .collect();
        Project::new(project, custom_domains)
    }

    pub(crate) async fn insert_project(
        &self,
        InsertProject {
            name,
            repo_id,
            env,
            root,
        }: InsertProject,
    ) {
        let created = time::now();
        sqlx::query!(
            "insert into projects (name, repo_id, created, env, root) values (?, ?, ?, ?, ?)",
            name,
            repo_id,
            created,
            env,
            root
        )
        .execute(&self.conn)
        .await
        .unwrap();
    }

    pub(crate) async fn update_project(
        &self,
        id: i64,
        UpdateProject {
            name,
            env,
            custom_domains,
        }: UpdateProject,
    ) {
        if let Some(name) = name {
            sqlx::query!("update projects set name = ? where id = ?", name, id)
                .execute(&self.conn)
                .await
                .unwrap();
        }

        if let Some(env) = env {
            sqlx::query!("update projects set env = ? where id = ?", env, id)
                .execute(&self.conn)
                .await
                .unwrap();
        }

        if let Some(custom_domains) = custom_domains {
            let mut tx = self.conn.begin().await.unwrap();
            sqlx::query!("delete from domains WHERE project = ?", id)
                .execute(&mut *tx)
                .await
                .unwrap();
            for domain in custom_domains {
                sqlx::query!(
                    "insert into domains (domain, project) values (?, ?)",
                    domain,
                    id
                )
                .execute(&mut *tx)
                .await
                .unwrap();
            }
            tx.commit().await.unwrap();
        }
    }

    pub(crate) async fn delete_project(&self, id: i64) {
        sqlx::query!("delete from projects where id = ?", id)
            .execute(&self.conn)
            .await
            .unwrap();
    }

    pub(crate) async fn get_deployment(&self, deployment: i64) -> Option<Deployment> {
        sqlx::query_as!(
            Deployment,
            r#"select id, url_id, timestamp, created, env, sha, branch, default_branch, result as "result: BuildResult", build_started, build_finished,project from deployments where deployments.id = ?"#,
            deployment
        )
        .fetch_optional(&self.conn)
        .await
        .unwrap()
    }

    pub(crate) async fn delete_deployment(&self, id: i64) {
        sqlx::query!("delete from deployments where id = ?", id)
            .execute(&self.conn)
            .await
            .unwrap();
    }

    pub(crate) async fn get_deployments(&self) -> impl Iterator<Item = Deployment> {
        sqlx::query_as!(
            Deployment,
            r#"select id, url_id, timestamp, created, env, sha, branch, default_branch, result as "result: BuildResult", build_started, build_finished, project from deployments"#
        )
        .fetch_all(&self.conn)
        .await
        .unwrap()
        .into_iter()
    }

    // TODO: implement this using SQL
    pub(crate) async fn get_latest_successful_prod_deployment_for_project(
        &self,
        project: i64,
    ) -> Option<Deployment> {
        let mut deployments: Vec<_> = self
            .get_deployments()
            .await
            .filter(|deployment| deployment.project == project && deployment.is_default_branch())
            .filter(|deployment| deployment.result != Some(BuildResult::Failed))
            .collect();
        deployments.sort_by_key(|deployment| deployment.timestamp);
        deployments.pop()
    }

    pub(crate) async fn get_deployment_with_project(
        &self,
        deployment: i64,
    ) -> Option<DeploymentWithProject> {
        let deployment = self.get_deployment(deployment).await?;
        let project = self.get_project(deployment.project).await?;
        Some(DeploymentWithProject {
            project: project.into(),
            deployment,
        })
    }

    pub(crate) async fn get_deployments_with_project(
        &self,
    ) -> impl Iterator<Item = DeploymentWithProject> {
        let project_iter = self.get_projects().await.into_iter();
        let projects: HashMap<_, Arc<_>> = project_iter
            .map(|project| (project.id, project.into()))
            .collect();
        self.get_deployments().await.filter_map(move |deployment| {
            Some(DeploymentWithProject {
                project: projects.get(&deployment.project)?.clone(),
                deployment,
            })
        })
    }

    pub(crate) async fn insert_deployment(&self, deployment: InsertDeployment) {
        let created = time::now();
        let url_id = create_deployment_url_id();
        sqlx::query!(
            "insert into deployments (url_id, timestamp, created, env, sha, branch, default_branch, project) values (?, ?, ?, ?, ?, ?, ?, ?)",
            url_id,
            deployment.timestamp,
            created,
            deployment.env,
            deployment.sha,
            deployment.branch,
            deployment.default_branch,
            deployment.project
        )
        .execute(&self.conn)
        .await
        .unwrap();
    }

    pub(crate) async fn update_deployment_result(&self, id: i64, status: BuildResult) {
        sqlx::query!("update deployments set result = ? where id = ?", status, id)
            .execute(&self.conn)
            .await
            .unwrap();
    }

    pub(crate) async fn update_deployment_build_start(&self, id: i64, build_started: i64) {
        sqlx::query!(
            "update deployments set build_started = ? where id = ?",
            build_started,
            id
        )
        .execute(&self.conn)
        .await
        .unwrap();
    }

    pub(crate) async fn update_deployment_build_end(&self, id: i64, build_finished: i64) {
        sqlx::query!(
            "update deployments set build_finished = ? where id = ?",
            build_finished,
            id
        )
        .execute(&self.conn)
        .await
        .unwrap();
    }

    pub(crate) async fn reset_deployment_build_end(&self, id: i64) {
        sqlx::query!(
            "update deployments set build_finished = NULL where id = ?",
            id
        )
        .execute(&self.conn)
        .await
        .unwrap();
    }

    pub(crate) async fn get_deployment_build_logs(&self, deployment: i64) -> Vec<BuildLog> {
        sqlx::query_as!(
            BuildLog,
            r#"select * from build where build.deployment = ?"#,
            deployment
        )
        .fetch_all(&self.conn)
        .await
        .unwrap()
    }

    pub(crate) async fn insert_deployment_build_log(
        &self,
        deployment: i64,
        content: &str,
        error: bool,
    ) {
        let time = now();
        let error = error as i64;
        sqlx::query!(
            "insert into build (timestamp, content, error, deployment) values (?, ?, ?, ?)",
            time,
            content,
            error,
            deployment
        )
        .execute(&self.conn)
        .await
        .unwrap();
    }

    pub(crate) async fn clear_deployment_build_logs(&self, deployment: i64) {
        sqlx::query!("delete from build where build.deployment = ?", deployment)
            .execute(&self.conn)
            .await
            .unwrap();
    }

    pub(crate) async fn hash_exists(&self, sha: &str) -> bool {
        sqlx::query!("select id from deployments where deployments.sha=?", sha)
            .fetch_optional(&self.conn)
            .await
            .unwrap()
            .is_some()
    }
}
