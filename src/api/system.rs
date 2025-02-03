use std::env;

use actix_web::{get, post, web::Json, HttpResponse, Responder};
use anyhow::ensure;
use tracing::error;

use crate::{
    api::bearer::{AnyRole, OwnerRole},
    docker::{
        create_container_with_explicit_binds, get_container_execution_logs, get_image_id,
        get_prezel_image_version, pull_image, run_container,
    },
};

/// Hello world
#[utoipa::path(
    responses(
        (status = 200, description = "Said hi to the world", body = &str)
    )
)]
#[get("/health")]
async fn health() -> impl Responder {
    HttpResponse::Ok().json("Healthy")
}

/// Get system version
#[utoipa::path(
    responses(
        (status = 200, description = "Fetched system version", body = String),
        (status = 500, description = "A problem was found when trying to read the version"),
    ),
    security(
        ("api_key" = [])
    )
)]
#[get("/system/version")]
async fn get_system_version(_auth: AnyRole) -> impl Responder {
    match get_prezel_image_version().await {
        Some(version) => HttpResponse::Ok().json(version),
        None => HttpResponse::InternalServerError().json("internal server error"),
    }
}

/// Get system logs
#[utoipa::path(
    responses(
        (status = 200, description = "Fetched system logs", body = [Log])
    ),
    security(
        ("api_key" = [])
    )
)]
#[get("/system/logs")]
async fn get_system_logs(_auth: AnyRole) -> impl Responder {
    let logs = get_container_execution_logs("prezel").await;
    HttpResponse::Ok().json(logs.collect::<Vec<_>>())
}

/// Update version
#[utoipa::path(
    request_body = String,
    responses(
        (status = 200, description = "Version update was initiated"),
        (status = 500, description = "A problem was found when trying to update the version"),
    ),
    security(
        ("api_key" = [])
    )
)]
#[post("/system/update")]
async fn update_version(_auth: OwnerRole, version: Json<String>) -> impl Responder {
    dbg!();
    match run_update_container(&version.0).await {
        Ok(()) => HttpResponse::Ok().finish(),
        Err(error) => {
            error!("{error}");
            HttpResponse::InternalServerError().json("internal server error")
        }
    }
}

async fn run_update_container(version: &str) -> anyhow::Result<()> {
    let image = format!("prezel/prezel:{version}");
    let id = get_image_id(&image).await;
    if id.is_none() {
        pull_image(&image).await;
        let id = get_image_id(&image).await;
        ensure!(id.is_some());
    }

    let create_template = r#"&& curl --unix-socket /var/run/docker.sock -H "Content-Type: application/json" -X POST \
        -d '{
              "Image": "$IMAGE",
              "Env": ["PREZEL_HOME='$PREZEL_HOME'"],
              "ExposedPorts": {
                "80/tcp": {},
                "443/tcp": {}
              },
              "HostConfig": {
                "PortBindings": {
                  "80/tcp": [{"HostPort": "80"}],
                  "443/tcp": [{"HostPort": "443"}]
                },
                "Binds": [
                  "'$PREZEL_HOME':'/opt/prezel'",
                  "/var/run/docker.sock:/var/run/docker.sock"
                ],
                "NetworkMode": "prezel",
                "RestartPolicy": {
                  "Name": "always"
                }
              }
            }' \
        http://localhost/containers/create?name=prezel"#;
    let create = create_template
        .replace("$PREZEL_HOME", &env::var("PREZEL_HOME").unwrap())
        .replace("$IMAGE", &image);
    let command = [
        "curl --unix-socket /var/run/docker.sock -X POST http://localhost/containers/prezel/stop",
        "&& curl --unix-socket /var/run/docker.sock -X DELETE http://localhost/containers/prezel",
        &create,
        "&& curl --unix-socket /var/run/docker.sock -X POST http://localhost/containers/prezel/start",
    ]
    .join(" ");

    let image = "alpine/curl".to_owned();
    pull_image(&image).await;
    let binds = vec!["/var/run/docker.sock:/var/run/docker.sock".to_owned()];
    let container =
        create_container_with_explicit_binds(image, Default::default(), binds, Some(command))
            .await?;
    Ok(run_container(&container).await?)
}
