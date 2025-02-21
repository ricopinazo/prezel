use std::{fmt, sync::Arc};

use async_trait::async_trait;
use octocrab::params::checks::{CheckRunConclusion, CheckRunStatus};
use tokio::sync::Mutex;

use crate::{
    conf::Conf,
    db::{nano_id::NanoId, BuildResult, Db},
    github::Github,
    provider,
    utils::now,
};

// type DeploymentHooks = Box<dyn DeploymentHooksOps>;

#[async_trait]
pub(crate) trait DeploymentHooks: 'static + Send + Sync + fmt::Debug {
    async fn on_build_log(&self, output: &str, error: bool);
    async fn on_build_started(&self);
    async fn on_build_finished(&self);
    async fn on_build_failed(&self);
}

#[derive(Debug)]
pub(crate) struct NoopHooks;

#[async_trait]
impl DeploymentHooks for NoopHooks {
    async fn on_build_log(&self, _output: &str, _error: bool) {}
    async fn on_build_started(&self) {}
    async fn on_build_finished(&self) {}
    async fn on_build_failed(&self) {}
}

#[derive(Debug, Clone)]
pub(crate) struct StatusHooks {
    db: Db,
    id: NanoId,
    github: Arc<Mutex<Github>>,
}

impl StatusHooks {
    pub(crate) fn new(deployment_id: NanoId, db: Db, github: Github) -> Self {
        Self {
            db,
            id: deployment_id,
            github: Mutex::new(github).into(),
        }
    }
}

// TODO: write also error status to db, and send updates to github!!
#[async_trait]
impl DeploymentHooks for StatusHooks {
    async fn on_build_log(&self, output: &str, error: bool) {
        self.db
            .insert_deployment_build_log(&self.id, output, error) // TODO: differentiate error logs
            .await;
    }

    async fn on_build_started(&self) {
        self.db.clear_deployment_build_logs(&self.id).await;
        self.db.update_deployment_build_start(&self.id, now()).await;
        self.db.reset_deployment_build_end(&self.id).await;
        self.update_github(Status::Building);
    }

    async fn on_build_finished(&self) {
        self.db.update_deployment_build_end(&self.id, now()).await;
        self.db
            .update_deployment_result(&self.id, BuildResult::Built) // FIXME: the db should maybe only have a flag error: bool
            .await;
        self.update_github(Status::Ready);
    }

    async fn on_build_failed(&self) {
        self.db.update_deployment_build_end(&self.id, now()).await;
        self.db
            .update_deployment_result(&self.id, BuildResult::Failed)
            .await;
        self.update_github(Status::Failed);
    }
}

#[derive(Clone, Copy)]
enum Status {
    Building,
    Ready,
    Failed,
}

impl From<Status> for CheckRunStatus {
    fn from(value: Status) -> Self {
        match value {
            Status::Building => Self::InProgress,
            Status::Ready => Self::Completed,
            Status::Failed => Self::Completed,
        }
    }
}

impl StatusHooks {
    fn update_github(&self, status: Status) {
        let hooks = self.clone();
        tokio::spawn(async move {
            let github = hooks.github.lock().await;
            let Conf {
                hostname, provider, ..
            } = Conf::read(); // FIXME: this should be async
            let deployment = hooks
                .db
                .get_deployment_with_project(&hooks.id)
                .await
                .unwrap();
            let repo_id = deployment.project.repo_id;
            let prs = github.get_open_pulls(repo_id).await.unwrap();
            for pr in prs {
                if deployment.branch == pr.head.ref_field {
                    // TODO: bear in mind the case where there are multiple apps deployed in one repo
                    // each of the should own a row in the table

                    // FIXME: there is something wrong in here
                    // urls are always defined, but especially the db one...
                    // maybe should only be defined when it's ready or it does exist???

                    let team = provider::get_team_name().await.unwrap();

                    let project_name = &deployment.project.name;
                    let slug = &deployment.url_id;
                    let provider_url = format!("{provider}/{team}/{project_name}/{slug}");
                    let url = Some(deployment.get_app_base_url(&hostname));
                    let db_url = Some(deployment.get_branch_sqlite_base_url(&hostname));
                    let content =
                        create_preview_comment(project_name, url, db_url, provider_url, status);
                    let _ = github
                        .upsert_pull_comment(repo_id, &content, pr.number)
                        .await;

                    let (status, conclusion) = match status {
                        Status::Building => (CheckRunStatus::InProgress, None),
                        Status::Ready => {
                            (CheckRunStatus::Completed, Some(CheckRunConclusion::Success))
                        }
                        Status::Failed => {
                            (CheckRunStatus::Completed, Some(CheckRunConclusion::Failure))
                        }
                    };
                    let _ = github
                        .upsert_pull_check(repo_id, &deployment.sha, status, conclusion)
                        .await;
                }
            }
        });
    }
}

fn create_preview_comment(
    project_name: &str,
    url: Option<String>, // FIXME: do these really need to be strings??
    db_url: Option<String>,
    provider_url: String,
    status: Status,
) -> String {
    let formatted_status = match status {
        // Status::Queued => "⏳ Queued",
        Status::Building => "🔨 Building",
        Status::Ready => "✅ Ready",
        Status::Failed => "❌ Failed",
    };
    let now = chrono::offset::Utc::now();
    let updated = now.format("%b %e, %Y %l:%M%P").to_string();

    let visit_preview = url
        .map(|url| format!("[Visit Preview]({url})"))
        .unwrap_or("".to_owned());

    let db_preview = db_url
        .map(|url| format!("💾 [Inspect]({url})"))
        .unwrap_or("".to_owned());

    println!("preview hooks: rendering comment with preview -> {visit_preview}");

    // this is the same as vercel, keep as a reference:
    // format!("[vc]: #yUYc4WnCSams3/Vxz23GavTMVXEuCeHcj9OuxbpiYDw=:eyJpc01vbm9yZXBvIjp0cnVlLCJ0eXBlIjoiZ2l0aHViIiwicHJvamVjdHMiOlt7Im5hbWUiOiJjdWJlcnMtbWFwIiwibGl2ZUZlZWRiYWNrIjp7InJlc29sdmVkIjowLCJ1bnJlc29sdmVkIjowLCJ0b3RhbCI6MCwibGluayI6ImN1YmVycy1tYXAtZ2l0LWZpeC1zcGFjZS1idWctcmljb3BpbmF6b3MtcHJvamVjdHMudmVyY2VsLmFwcCJ9LCJpbnNwZWN0b3JVcmwiOiJodHRwczovL3ZlcmNlbC5jb20vcmljb3BpbmF6b3MtcHJvamVjdHMvY3ViZXJzLW1hcC84bnY2SzhUSm9rNERzZjhKY3hVZWZUckZKdXVRIiwibmV4dENvbW1pdFN0YXR1cyI6IkRFUExPWUVEIiwicHJldmlld1VybCI6ImN1YmVycy1tYXAtZ2l0LWZpeC1zcGFjZS1idWctcmljb3BpbmF6b3MtcHJvamVjdHMudmVyY2VsLmFwcCIsInJvb3REaXJlY3RvcnkiOm51bGx9XX0=
    // **The latest updates on your projects**. Learn more about [Prezel for Git ↗︎](https://vercel.link/github-learn-more)

    // | Name | Status | Preview | Comments | Updated (UTC) |
    // | :--- | :----- | :------ | :------- | :------ |
    // | **{name}** | {formatted_status} ([Inspect](http://localhost:60000)) | [Visit Preview](http://localhost:60000) | 💬 [**Add feedback**](http://localhost:60000) | {updated} |")

    format!("
**The latest updates on your projects**. Learn more about [Prezel for Git ↗︎](https://github.com/ricopinazo/prezel)

| Name | Status | Preview | Sqlite DB | Updated (UTC) |
| :--- | :----- | :------ | :------- | :------ |
| **{project_name}** | {formatted_status} ([Inspect]({provider_url})) | {visit_preview} | {db_preview} | {updated} |")
}
