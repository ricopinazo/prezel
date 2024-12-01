use std::fmt;

use async_trait::async_trait;

use crate::{
    db::{BuildResult, Db},
    time::now,
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
pub(crate) struct StatusHooks {
    db: Db,
    id: i64,
}

impl StatusHooks {
    pub(crate) fn new(db: Db, deployment_id: i64) -> Self {
        Self {
            db,
            id: deployment_id,
        }
    }
}

// TODO: write also error status to db, and send updates to github!!
#[async_trait]
impl DeploymentHooks for StatusHooks {
    async fn on_build_log(&self, output: &str, error: bool) {
        self.db
            .insert_deployment_build_log(self.id, output, error) // TODO: differentiate error logs
            .await;
    }

    async fn on_build_started(&self) {
        self.db.clear_deployment_build_logs(self.id).await;
        self.db.update_deployment_build_start(self.id, now()).await;
        self.db.reset_deployment_build_end(self.id).await;
    }

    async fn on_build_finished(&self) {
        self.db.update_deployment_build_end(self.id, now()).await;
        self.db
            .update_deployment_result(self.id, BuildResult::Built) // FIXME: the db should maybe only have a flag error: bool
            .await
    }

    async fn on_build_failed(&self) {
        self.db.update_deployment_build_end(self.id, now()).await;
        self.db
            .update_deployment_result(self.id, BuildResult::Failed)
            .await
    }
}

#[derive(Debug)]
pub(crate) struct NoopHooks;

#[async_trait]
impl DeploymentHooks for NoopHooks {
    async fn on_build_log(&self, _output: &str, error: bool) {}
    async fn on_build_started(&self) {}
    async fn on_build_finished(&self) {}
    async fn on_build_failed(&self) {}
}
