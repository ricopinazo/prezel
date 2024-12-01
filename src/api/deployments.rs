use actix_web::{
    delete, get, post,
    web::{Data, Json, Path},
    HttpResponse, Responder,
};

use crate::{
    api::{security::RequireApiKey, utils::clone_deployment, AppState},
    logging::{read_request_event_logs, Log},
};

// TODO: this should take the id from the PATH, should not be POST I guess
/// Re-deploy based on an existing deployment
#[utoipa::path(
    request_body = i64,
    responses(
        (status = 200, description = "Deployment redeployed successfully"),
    ),
    security(
        ("api_key" = [])
    )
)]
#[post("/deployments/redeploy", wrap = "RequireApiKey")]
async fn redeploy(deployment: Json<i64>, state: Data<AppState>) -> impl Responder {
    clone_deployment(&state.db, deployment.0).await;
    state.manager.sync_with_db().await;
    HttpResponse::Ok()
}

/// Delete deployment
#[utoipa::path(
    responses(
        (status = 200, description = "Deployment deleted successfully"),
    ),
    security(
        ("api_key" = [])
    )
)]
#[delete("/deployments/{id}", wrap = "RequireApiKey")]
async fn delete_deployment(state: Data<AppState>, id: Path<i64>) -> impl Responder {
    state.db.delete_deployment(id.into_inner()).await;
    state.manager.sync_with_db().await;
    HttpResponse::Ok()
}

/// Sync deployments with github
#[utoipa::path(
    responses(
        (status = 200, description = "Sync triggered successfully"),
    ),
    security(
        ("api_key" = [])
    )
)]
#[post("/sync", wrap = "RequireApiKey")]
async fn sync(state: Data<AppState>) -> impl Responder {
    state.manager.full_sync_with_github().await;
    HttpResponse::Ok()
}

/// Get deployment execution logs
#[utoipa::path(
    responses(
        (status = 200, description = "Fetched deployment execution logs", body = [Log]),
        (status = 404, description = "Deployment not found", body = String),
        (status = 500, description = "Internal error when fetching logs", body = String)
    ),
    security(
        ("api_key" = [])
    )
)]
#[get("/deployments/{id}/logs", wrap = "RequireApiKey")]
async fn get_deployment_logs(state: Data<AppState>, id: Path<i64>) -> impl Responder {
    let id = id.into_inner();
    let app_container = match state.manager.get_deployment(id).await {
        Some(deployment) => deployment.app_container.clone(),
        None => return HttpResponse::NotFound().json("not found"),
    };

    let container_logs = app_container
        .get_logs()
        .await
        .map(|log| Log::from_docker(log, id));

    match read_request_event_logs() {
        Ok(logs) => {
            let mut logs = logs
                .filter(|log| log.deployment == id)
                .chain(container_logs)
                .collect::<Vec<_>>();
            logs.sort_by_key(|log| -log.time); // from latest to oldest
            HttpResponse::Ok().json(logs)
        }
        Err(error) => HttpResponse::InternalServerError().json(error.to_string()), // need a ErrorResponse variant for this
    }
}

/// Get deployment build logs
#[utoipa::path(
    responses(
        (status = 200, description = "Fetched deployment build logs", body = [Log]),
        // (status = 404, description = "Deployment not found", body = String),
        // (status = 500, description = "Internal error when fetching logs", body = String) // TODO: re-enable errors
    ),
    security(
        ("api_key" = [])
    )
)]
#[get("/deployments/{id}/build", wrap = "RequireApiKey")]
async fn get_deployment_build_logs(state: Data<AppState>, id: Path<i64>) -> impl Responder {
    let id = id.into_inner();
    let logs: Vec<Log> = state
        .db
        .get_deployment_build_logs(id)
        .await
        .into_iter()
        .map(|log| log.into())
        .collect();
    HttpResponse::Ok().json(logs)
}
