use actix_web::{get, HttpResponse, Responder};

use crate::{api::bearer::AnyRole, docker::get_container_execution_logs};

/// Get system logs
#[utoipa::path(
    responses(
        (status = 200, description = "Fetched system logs", body = [Log])
    ),
    security(
        ("bearerAuth" = [])
    )
)]
#[get("/api/system/logs")]
async fn get_logs(_auth: AnyRole) -> impl Responder {
    let logs = get_container_execution_logs("prezel").await;
    HttpResponse::Ok().json(logs.collect::<Vec<_>>())
}
