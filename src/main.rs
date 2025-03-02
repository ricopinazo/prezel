use api::server::run_api_server;
use conf::Conf;
use db::Db;
use deployments::manager::Manager;
use github::Github;
use proxy::run_proxy;
use tls::CertificateStore;
use traces::init_tracing_subscriber;
use tracing::info;

mod api;
mod conf;
mod container;
mod db;
mod deployments;
mod docker;
mod docker_bridge;
mod env;
mod github;
mod hooks;
mod label;
mod listener;
mod logging;
mod paths;
mod provider;
mod proxy;
mod sqlite_db;
mod tls;
mod tokens;
mod traces;
mod utils;

pub(crate) const DOCKER_PORT: u16 = 5046;

// struct DeploymentFilter;

// impl Filter<Registry> for DeploymentFilter {
//     fn enabled(
//         &self,
//         meta: &tracing::Metadata<'_>,
//         _ctx: &tracing_subscriber::layer::Context<'_, Registry>,
//     ) -> bool {
//         meta.fields().field("deployment").is_some() // TODO: rename this field to prezel?
//     }
// }

#[tokio::main]
async fn main() {
    let _guard = init_tracing_subscriber();
    info!("prezel is starting...");

    let conf = Conf::read();
    let cloned_conf = conf.clone();

    let db = Db::setup().await;
    let github = Github::new().await;

    let certificates = CertificateStore::load(&conf).await;
    let manager = Manager::new(
        conf.hostname.clone(),
        github.clone(),
        db.clone(),
        certificates.clone(),
    );
    let cloned_manager = manager.clone();

    tokio::task::spawn_blocking(|| run_proxy(cloned_manager, cloned_conf, certificates));

    manager.full_sync_with_github().await;

    let api_hostname = format!("api.{}", &conf.hostname);
    run_api_server(manager, db, github, &api_hostname, conf.secret)
        .await
        .unwrap();
}
