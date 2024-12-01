use octocrab::params::checks::{CheckRunConclusion, CheckRunStatus};

use crate::{
    db::Project,
    deployment::{StartedDeployment, Status},
    github::{github, upsert_pull_check, upsert_pull_comment},
    port_mapper::RegisteredUrl,
};

// use super::DeploymentHooks;

// #[derive(Clone)]
// pub(crate) struct PreviewHooks {
//     crab: Octocrab,
//     project: Project,
//     domain: String,
//     db_domain: String,
//     pull: u64,
//     sha: String,
// }

pub(crate) async fn update_github_status(deployment: &StartedDeployment, pull: u64) {
    let crab = github().await;
    let (check_status, conclusion) = match deployment.state.read().await.status {
        Status::Queued => (CheckRunStatus::Queued, None),
        Status::Building => (CheckRunStatus::InProgress, None),
        Status::Ready => (CheckRunStatus::Completed, Some(CheckRunConclusion::Success)),
        Status::Error => (CheckRunStatus::Completed, Some(CheckRunConclusion::Failure)),
    };
    upsert_pull_check(
        &crab,
        &deployment.deployment.project.repo_id,
        &deployment.deployment.sha,
        check_status,
        conclusion,
    )
    .await;
    let comment = create_preview_comment(
        &deployment.deployment.project,
        &deployment.state.read().await.url,
        &deployment.state.read().await.db_url,
        &deployment.state.read().await.status,
    )
    .await;
    upsert_pull_comment(
        &crab,
        &deployment.deployment.project.repo_id,
        &comment,
        pull,
    )
    .await;
}

async fn create_preview_comment(
    project: &Project,
    url: &RegisteredUrl,
    db_url: &RegisteredUrl,
    status: &Status,
) -> String {
    let name = &project.name;
    let formatted_status = match status {
        Status::Queued => "⏳ Queued",
        Status::Building => "🔨 Building",
        Status::Ready => "✅ Ready",
        Status::Error => "❌ Error",
    };
    let now = chrono::offset::Utc::now();
    let updated = now.format("%b %e, %Y %l:%M%P").to_string();

    let visit_preview = url
        .enabled_url()
        .await
        .map(|url| format!("[Visit Preview]({url})"))
        .unwrap_or("".to_owned());

    let db_preview = db_url
        .enabled_url()
        .await
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
| **{name}** | {formatted_status} ([Inspect](http://localhost:3000/docs)) | {visit_preview} | {db_preview} | {updated} |")
}
