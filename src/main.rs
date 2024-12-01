use api::server::run_api_server;
use conf::Conf;
use db::Db;
use deployments::manager::Manager;
use github::Github;
use proxy::run_proxy;
use tls::get_or_generate_tls_certificate;
use tracing::info;
use tracing_subscriber::{
    layer::{Filter, SubscriberExt},
    util::SubscriberInitExt,
    EnvFilter, Layer, Registry,
};

mod alphabet;
mod api;
mod conf;
mod container;
mod db;
mod deployment_hooks;
mod deployments;
mod docker;
mod docker_bridge;
mod env;
mod github;
mod listener;
mod logging;
mod paths;
mod proxy;
mod time;
mod tls;

pub(crate) const DOCKER_PORT: u16 = 5046;

struct DeploymentFilter;

impl Filter<Registry> for DeploymentFilter {
    fn enabled(
        &self,
        meta: &tracing::Metadata<'_>,
        _ctx: &tracing_subscriber::layer::Context<'_, Registry>,
    ) -> bool {
        meta.fields().field("deployment").is_some() // TODO: rename this field to prezel?
    }
}

#[tokio::main]
async fn main() {
    // TODO: sort all of this tracing conf
    // TODO: remove tracing_appender dep
    // let file_appender = tracing_appender::rolling::hourly("/opt/prezel/log", "prezel.log");
    // let json_layer = tracing_subscriber::fmt::layer()
    //     .json()
    //     .with_writer(file_appender)
    //     .with_filter(DeploymentFilter);

    let stdout_layer = tracing_subscriber::fmt::layer()
        .pretty()
        .with_writer(std::io::stdout)
        .with_filter(EnvFilter::new("info")); // TODO: read from env

    // env_logger::init_from_env(env_logger::Env::new().default_filter_or("info")); -> old version

    tracing_subscriber::registry()
        // .with(json_layer)
        .with(stdout_layer)
        .init();

    let conf = Conf::read();
    let cloned_conf = conf.clone();

    let db = Db::setup().await;
    let github = Github::new().await;

    let manager = Manager::new(conf.hostname.clone(), github.clone(), db.clone());
    let cloned_manager = manager.clone();

    let api_hostname = format!("api.{}", &conf.hostname);

    let tls_certificate = get_or_generate_tls_certificate(&conf).await;

    tokio::task::spawn_blocking(|| run_proxy(cloned_manager, cloned_conf, tls_certificate));

    manager.full_sync_with_github().await;

    run_api_server(manager, db, github, &api_hostname, conf.coordinator)
        .await
        .unwrap();
}
