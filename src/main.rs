use std::time::Duration;

use api::server::run_api_server;
use conf::Conf;
use db::Db;
use deployments::manager::Manager;
use github::Github;
use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig as _;
use opentelemetry_sdk::Resource;
// use opentelemetry_otlp::WithExportConfig;
use proxy::run_proxy;
use tls::CertificateStore;
// use tracing_subscriber::fmt::Subscriber;
// use tracing_subscriber::{
//     layer::{Filter, SubscriberExt},
//     util::SubscriberInitExt,
//     EnvFilter, Layer, Registry,
// };

// use opentelemetry::{
//     global::{self, ObjectSafeTracerProvider},
//     trace::{TraceContextExt, TraceError},
//     KeyValue,
// };
// use opentelemetry_sdk::trace::Tracer;
// use opentelemetry_sdk::trace::TracerProvider;
// use opentelemetry_sdk::{runtime, Resource};
// use opentelemetry_semantic_conventions::resource::SERVICE_NAME;
// use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, Registry};

use opentelemetry::global;
use opentelemetry::trace::{TraceContextExt, TraceError, Tracer, TracerProvider as _};
use opentelemetry_sdk::trace::TracerProvider;
use tracing::{error, event, span, Level};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer, Registry};

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
    ///////////// THIS WORKS !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
    // let exporter = opentelemetry_otlp::SpanExporter::builder()
    //     .with_tonic()
    //     // .with_endpoint("http://jaegerrrr:4318/v1/metrics")
    //     .with_timeout(Duration::from_secs(3))
    //     .build()
    //     .unwrap();
    // let tracer_provider = TracerProvider::builder()
    //     .with_simple_exporter(exporter)
    //     .with_resource(Resource::new(vec![KeyValue::new(
    //         "prezel.app",
    //         "tracing-jaeger",
    //     )]))
    //     .build();

    // global::set_tracer_provider(tracer_provider.clone());
    // let tracer = global::tracer("tracing-jaeger");
    // tracer.in_span("main-operation", |cx| {
    //     let span = cx.span();
    //     span.set_attribute(KeyValue::new("my-attribute", "my-value"));
    //     span.add_event(
    //         "Main span event".to_string(),
    //         vec![KeyValue::new("foo", "1")],
    //     );
    //     tracer.in_span("child-operation...", |cx| {
    //         let span = cx.span();
    //         span.add_event("Sub span event", vec![KeyValue::new("bar", "1")]);
    //     });
    // });

    /////////////////////////////////////////
    // from tokio: https://tokio.rs/tokio/topics/tracing-next-steps
    /////////////////////////////////

    // global::set_text_map_propagator(opentelemetry_jaeger::Propagator::new());
    // let tracer = opentelemetry_jaeger::new_pipeline()
    //     .with_service_name("prezel")
    //     .install_simple()?;

    // let opentelemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    // // The SubscriberExt and SubscriberInitExt traits are needed to extend the
    // // Registry to accept `opentelemetry (the OpenTelemetryLayer type).
    // tracing_subscriber::registry()
    //     .with(opentelemetry)
    //     // Continue logging to stdout
    //     // .with(fmt::Layer::default())
    //     .try_init()?;

    //////////////////////////////////////////////
    // from raphtory
    // //////////////////////////////
    // opentelemetry_otlp::new_pipeline()
    //     .tracing()
    //     .with_exporter(
    //         opentelemetry_otlp::new_exporter()
    //             .tonic()
    //             // .with_endpoint(format!(
    //             //     "{}:{}",
    //             //     self.otlp_agent_host.clone(),
    //             //     self.otlp_agent_port.clone()
    //             // ))
    //             .with_timeout(Duration::from_secs(3)),
    //     )
    //     .with_trace_config(
    //         trace::Config::default()
    //             .with_sampler(Sampler::AlwaysOn)
    //             .with_resource(Resource::new(vec![KeyValue::new(
    //                 "service.name",
    //                 self.otlp_tracing_service_name.clone(),
    //             )])),
    //     )
    //     .install_batch(opentelemetry_sdk::runtime::Tokio);

    // let registry = Registry::default();
    // registry
    //     .with(tracing_opentelemetry::layer().with_tracer(tp.tracer(tracer_name.clone())))
    //     .try_init()
    //     .ok();
    ///////////////////////////
    // getting inspo from raphtory
    // ////////////////////////////////

    // let tp = TracerProvider::builder()
    //     // .tracing()
    //     // try as well with batch exporter !!!!!!!!!!!!!!!!!!!!!
    //     .with_simple_exporter(
    //         opentelemetry_otlp::SpanExporter::builder()
    //             .with_tonic()
    //             // .with_endpoint(format!(
    //             //     "{}:{}",
    //             //     self.otlp_agent_host.clone(),
    //             //     self.otlp_agent_port.clone()
    //             // ))
    //             .with_timeout(Duration::from_secs(3))
    //             .build()
    //             .unwrap(),
    //     )
    //     // .with_trace_config(
    //     //     trace::Config::default()
    //     //         .with_sampler(Sampler::AlwaysOn)
    //     //         .with_resource(Resource::new(vec![KeyValue::new(
    //     //             "service.name",
    //     //             self.otlp_tracing_service_name.clone(),
    //     //         )])),
    //     // )
    //     // .install_batch(opentelemetry_sdk::runtime::Tokio)
    //     // //////////////////
    //     .with_resource(Resource::new(vec![KeyValue::new(
    //         "prezel.app",
    //         "tracing-jaeger",
    //     )]))
    //     .build();

    // let registry = Registry::default();
    // registry
    //     .with(tracing_opentelemetry::layer().with_tracer(tp.boxed_tracer("prezel".to_owned())))
    //     .try_init()
    //     .ok();
    ////////////////
    // example from tracing_opentelemetry
    ////////////////////////////

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_timeout(Duration::from_secs(3))
        .build()
        .unwrap();
    let provider = TracerProvider::builder()
        .with_simple_exporter(exporter)
        .with_resource(Resource::new(vec![KeyValue::new("prezel.app", "prezel")]))
        .build();

    let tracer = provider.tracer("prezel");
    // global::set_tracer_provider(provider.clone());
    // let tracer = global::tracer("tracing-jaegerrr");

    ////////////<- this tracer here beol works, but how do I create a tracing layer with it
    tracer.in_span("main-operation", |cx| {
        let span = cx.span();
        span.set_attribute(KeyValue::new("my-attribute", "my-value"));
        span.add_event(
            "Main span event".to_string(),
            vec![KeyValue::new("foo", "1")],
        );
        tracer.in_span("child-operation...", |cx| {
            let span = cx.span();
            span.add_event("Sub span event", vec![KeyValue::new("bar", "1")]);
        });
    });

    // Create a tracing layer with the configured tracer
    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    let stdout_layer = tracing_subscriber::fmt::layer()
        .pretty()
        .with_writer(std::io::stdout)
        .with_filter(EnvFilter::new("info"));

    // Registry::default()
    //     .with(telemetry)
    //     .with(stdout_layer)
    //     .try_init()
    //     .unwrap();
    let subscriber = Registry::default().with(telemetry).with(stdout_layer);
    tracing::subscriber::set_global_default(subscriber).expect("Failed to set subscriber");

    #[tracing::instrument]
    async fn example_function() {
        event!(Level::ERROR, "event inside function");
    }
    example_function().await;

    // old tracing conf
    /////////////////////////////////////////////////////////////////
    // let stdout_layer = tracing_subscriber::fmt::layer()
    //     .pretty()
    //     .with_writer(std::io::stdout)
    //     .with_filter(EnvFilter::new("info")); // TODO: read from env
    // tracing_subscriber::registry()
    //     .with(stdout_layer)
    //     .init();
    /////////////////////////////////////////////////////////////////

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
    run_api_server(manager, db, github, &api_hostname, conf.coordinator)
        .await
        .unwrap();
}
