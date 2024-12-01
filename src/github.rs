use anyhow::ensure;
use flate2::read::GzDecoder;
use http::StatusCode;
use http_body_util::BodyExt;
use log::info;
use octocrab::{
    models::{pulls::PullRequest, InstallationRepositories, IssueState, Repository},
    params::{
        checks::{CheckRunConclusion, CheckRunStatus},
        repos::Commitish,
    },
    Octocrab, Result as OctocrabResult,
};
use serde::Serialize;
use std::{io::Cursor, path::Path, sync::Arc};
use tar::Archive;
use tokio::sync::RwLock;

use crate::{conf::Conf, time::now};

const CHECK_NAME: &str = "prezel";
const COMMENT_START: &'static str = "[prezel]: authored";

#[derive(Serialize, Debug)]
struct RequestBody {
    token: String,
    id: String,
}

pub(crate) struct Commit {
    pub(crate) timestamp: i64,
    pub(crate) sha: String,
}

#[derive(Debug)]
struct Token {
    secret: String,
    millis: i64,
}

#[derive(Clone, Debug)]
pub(crate) struct Github {
    token: Arc<RwLock<Token>>,
}

impl Github {
    pub(crate) async fn new() -> Self {
        Self {
            token: RwLock::new(
                get_installation_access_token()
                    .await
                    .expect("Failed to get app installation token on startup"),
            )
            .into(),
        }
    }
    pub(crate) async fn get_open_pulls(&self, repo_id: &str) -> anyhow::Result<Vec<PullRequest>> {
        let crab = self.get_crab().await?;
        let (owner, name) = self.get_owner_and_name(repo_id).await?;
        let pulls = crab.pulls(owner, name).list().send().await?;
        Ok(pulls
            .into_iter()
            .filter(|pull| pull.state == Some(IssueState::Open))
            .collect())
    }

    pub(crate) async fn get_repo(&self, id: &str) -> anyhow::Result<Option<Repository>> {
        let crab = self.get_crab().await?;
        Ok(crab
            .get(format!("/repositories/{id}"), None::<&()>)
            .await
            .unwrap())
    }

    pub(crate) async fn get_repos(&self) -> anyhow::Result<Vec<Repository>> {
        let crab = self.get_crab().await?;
        let installation_repos: InstallationRepositories = crab
            .get("/installation/repositories", None::<&()>)
            .await
            .unwrap();
        Ok(installation_repos.repositories)
    }

    pub(crate) async fn get_pull(&self, repo_id: &str, number: u64) -> anyhow::Result<PullRequest> {
        let crab = self.get_crab().await?;
        let (owner, name) = self.get_owner_and_name(repo_id).await?;
        Ok(crab.pulls(owner, name).get(number).await?)
    }

    pub(crate) async fn get_default_branch(&self, repo_id: &str) -> anyhow::Result<String> {
        let crab = self.get_crab().await?;
        let (owner, name) = self.get_owner_and_name(repo_id).await?;
        let repository = crab.repos(owner, name).get().await.unwrap();
        Ok(repository.default_branch.unwrap())
    }

    pub(crate) async fn get_latest_commit(
        &self,
        repo_id: &str,
        branch: &str,
    ) -> anyhow::Result<Option<Commit>> {
        let crab = self.get_crab().await?;
        let (owner, name) = self.get_owner_and_name(repo_id).await?;
        Ok(Self::get_latest_commit_option(&crab, &owner, &name, branch).await)
    }

    async fn get_latest_commit_option(
        crab: &Octocrab,
        owner: &str,
        name: &str,
        branch: &str,
    ) -> Option<Commit> {
        // let crab = self.get_crab().await;
        // let (owner, name) = self.get_owner_and_name(repo_id).await;
        let commit = crab.commits(owner, name).get(branch).await.ok()?;
        let timestamp = commit.commit.committer?.date?.timestamp_millis();
        Some(Commit {
            timestamp,
            sha: commit.sha,
        })
    }

    pub(crate) async fn download_commit(
        &self,
        repo_id: &str,
        sha: &str,
        path: &Path,
    ) -> anyhow::Result<()> {
        let crab = self.get_crab().await?;
        let (owner, name) = self.get_owner_and_name(repo_id).await?;
        let response = crab
            .repos(owner, name)
            .download_tarball(sha.to_owned())
            .await
            .unwrap();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let content = Cursor::new(bytes);
        let mut archive = Archive::new(GzDecoder::new(content));
        for entry in archive.entries().unwrap() {
            // let path: String = entry.unwrap().path().unwrap();
            let mut entry = entry.unwrap();
            let entry_path = entry.path().unwrap();
            let mut components = entry_path.components();
            components.next();
            let inner_path = components.as_path();
            entry.unpack(&path.join(inner_path)).unwrap();
        }
        Ok(())
    }

    pub(crate) async fn upsert_pull_check(
        &self,
        repo_id: &str,
        sha: &str,
        status: CheckRunStatus,
        conclusion: Option<CheckRunConclusion>,
    ) -> anyhow::Result<()> {
        let crab = self.get_crab().await?;
        let (owner, name) = self.get_owner_and_name(repo_id).await?;
        let check_handler = crab.checks(owner, name);
        let checks = check_handler
            .list_check_runs_for_git_ref(Commitish(sha.into()))
            .send()
            .await
            .unwrap();

        let app_check = checks
            .check_runs
            .iter()
            .find(|check| check.name == CHECK_NAME);

        match app_check {
            Some(check) => {
                println!("updating check run for {sha}");
                let mut builder = check_handler.update_check_run(check.id).status(status);
                if let Some(conclusion) = conclusion {
                    builder = builder.conclusion(conclusion);
                }
                builder.send().await.unwrap();
            }
            None => {
                println!("creating check run for {sha}");
                let mut builder = check_handler
                    .create_check_run(CHECK_NAME, sha)
                    // .details_url(details_url) // TODO: add this -> have a look at the vercel details URL
                    .status(status);

                if let Some(conclusion) = conclusion {
                    builder = builder.conclusion(conclusion);
                }
                builder.send().await.unwrap();
            }
        }
        Ok(())
    }

    pub(crate) async fn upsert_pull_comment(
        &self,
        repo_id: &str,
        content: &str,
        pull: u64,
    ) -> anyhow::Result<()> {
        let crab = self.get_crab().await?;
        let (owner, name) = self.get_owner_and_name(repo_id).await?;
        // let app: octocrab::models::App = crab.get("/app", None::<&()>).await.unwrap();
        // let app_slug = app.slug.unwrap();
        // let app_name_in_comments = format!("{app_slug}[bot]"); // TODO: maybe there is another field in the comments that is not user.login

        println!("adding comment to pull {pull}");
        let comments = crab
            .issues(&owner, &name)
            .list_comments(pull)
            .send()
            .await
            .unwrap();

        // TODO: put the app name in a shared constant
        let app_comment = comments.items.iter().find(|comment| {
            let body = comment.body.as_ref();
            body.is_some_and(|body| body.starts_with(COMMENT_START))
        });

        let content = format!("{COMMENT_START}\n{content}");
        if let Some(comment) = app_comment {
            println!("updating comment for pull {pull}");
            crab.issues(owner, name)
                .update_comment(comment.id, content)
                .await
                .unwrap();
        } else {
            println!("creating comment for pull {pull}");
            crab.issues(owner, name)
                .create_comment(pull, content)
                .await
                .unwrap();
        }
        Ok(())
    }

    // TODO: make this receive crab as argument
    async fn get_owner_and_name(&self, id: &str) -> anyhow::Result<(String, String)> {
        let repo = self.get_repo(id).await?.unwrap();
        Ok((repo.owner.unwrap().login, repo.name))
    }

    async fn get_crab(&self) -> anyhow::Result<Octocrab> {
        self.update_token().await?;
        Ok(octocrab::OctocrabBuilder::default()
            .user_access_token(self.token.read().await.secret.clone())
            .build()
            .unwrap())
    }

    async fn update_token(&self) -> anyhow::Result<()> {
        let token_too_old = {
            let token = self.token.read().await;
            is_token_too_old(&token)
        };
        if token_too_old {
            let mut token = self.token.write().await;
            if is_token_too_old(&token) {
                *token = get_installation_access_token().await?
            }
        }
        Ok(())
    }
}

fn is_token_too_old(token: &Token) -> bool {
    let age = now() - token.millis;
    age > 30 * 60 * 1000
}

async fn get_installation_access_token() -> anyhow::Result<Token> {
    let Conf {
        coordinator,
        token,
        hostname: id,
    } = Conf::read();

    let client = reqwest::Client::new();
    let url = format!("{coordinator}/api/instance/token");
    let json = RequestBody { id, token };
    info!("requesting Github installation token from {url}");
    let response = client.post(url).json(&json).send().await?;
    ensure!(response.status() == StatusCode::OK);
    let secret = response.text().await?;
    Ok(Token {
        secret,
        millis: now(),
    })
}
