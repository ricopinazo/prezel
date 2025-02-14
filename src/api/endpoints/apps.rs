use actix_web::{
    delete, get, patch, post,
    web::{Data, Json, Path},
    HttpMessage, HttpRequest, HttpResponse, Responder,
};
use futures::future::join_all;

use crate::{
    api::{
        bearer::{AnyRole, OwnerRole},
        utils::{get_all_deployments, get_prod_deployment, get_prod_deployment_id},
        AppState, ErrorResponse, FullProjectInfo, ProjectInfo,
    },
    db::{EnvVar, InsertProject, UpdateProject},
    tokens::TokenClaims,
};

/// Get projects
#[utoipa::path(
    responses(
        (status = 200, description = "Hello world", body = [ProjectInfo])
    ),
    security(
        ("api_key" = [])
    )
)]
#[get("/api/apps")]
#[tracing::instrument]
async fn get_projects(auth: AnyRole, state: Data<AppState>) -> impl Responder {
    let projects = state.db.get_projects().await;
    let projects_with_deployments = projects.into_iter().map(|project| {
        let state = state.clone();
        async move {
            let prod_deployment = get_prod_deployment(&state, project.id).await;
            let prod_deployment_id = get_prod_deployment_id(&state.db, &project).await;

            // TODO: if the repo is not available, simply don't return that info
            let repo = state
                .github
                .get_repo(project.repo_id)
                .await
                .unwrap()
                .unwrap();
            ProjectInfo {
                name: project.name.clone(),
                id: project.id,
                repo: repo.into(),
                created: project.created,
                env: project.env,
                custom_domains: project.custom_domains,
                prod_deployment_id,
                prod_deployment,
            }
        }
    });

    HttpResponse::Ok().json(join_all(projects_with_deployments).await)
}

/// Get project by name
#[utoipa::path(
    responses(
        (status = 200, description = "Hello world", body = FullProjectInfo),
        (status = 404, description = "Project not found", body = ErrorResponse)
    ),
    security(
        ("api_key" = [])
    )
)]
#[get("/api/apps/{name}")]
#[tracing::instrument]
async fn get_project(auth: AnyRole, state: Data<AppState>, name: Path<String>) -> impl Responder {
    let name = name.into_inner();
    let project = state.db.get_project_by_name(&name).await;
    match project {
        Some(project) => {
            let repo = state
                .github
                .get_repo(project.repo_id)
                .await
                .unwrap()
                .unwrap();

            let prod_deployment_id = get_prod_deployment_id(&state.db, &project).await;
            let prod_deployment = get_prod_deployment(&state, project.id).await;
            let deployments = get_all_deployments(&state, project.id).await;

            HttpResponse::Ok().json(FullProjectInfo {
                name: project.name,
                id: project.id,
                repo: repo.into(),
                created: project.created,
                env: project.env,
                custom_domains: project.custom_domains,
                prod_deployment_id,
                prod_deployment,
                deployments,
            })
        }
        None => HttpResponse::NotFound().json(ErrorResponse::NotFound(format!("name = {name}"))),
    }
}

/// Create project
#[utoipa::path(
    request_body = InsertProject,
    responses(
        (status = 201, description = "Project created successfully"),
        (status = 400, description = "'api' is not a valid app name"),
    ),
    security(
        ("api_key" = [])
    )
)]
#[post("/api/apps")] // TODO: return project when successfully inserted
#[tracing::instrument]
async fn create_project(
    _auth: OwnerRole,
    project: Json<InsertProject>,
    state: Data<AppState>,
) -> impl Responder {
    if &project.name != "api" {
        state.db.insert_project(project.0).await;
        state.manager.full_sync_with_github().await;
        HttpResponse::Ok()
    } else {
        HttpResponse::BadRequest()
    }
}

/// Update project
#[utoipa::path(
    request_body = UpdateProject,
    responses(
        (status = 200, description = "Project updated successfully"),
        // (status = 409, description = "Todo with id already exists", body = ErrorResponse, example = json!(ErrorResponse::Conflict(String::from("id = 1"))))
    ),
    security(
        ("api_key" = [])
    )
)]
#[patch("/api/apps/{id}")]
#[tracing::instrument]
async fn update_project(
    auth: OwnerRole,
    project: Json<UpdateProject>,
    state: Data<AppState>,
    id: Path<i64>,
) -> impl Responder {
    state.db.update_project(id.into_inner(), project.0).await;
    state.manager.sync_with_db().await; // TODO: review if its fine not doing a full sync with github here
    HttpResponse::Ok()
}

/// Delete project
#[utoipa::path(
    responses(
        (status = 200, description = "Project deleted successfully"),
    ),
    security(
        ("api_key" = [])
    )
)]
#[delete("/api/apps/{id}")]
#[tracing::instrument]
async fn delete_project(
    auth: OwnerRole,
    req: HttpRequest,
    state: Data<AppState>,
    id: Path<i64>,
) -> impl Responder {
    req.extensions().get::<TokenClaims>();
    state.db.delete_project(id.into_inner()).await;
    state.manager.sync_with_db().await;
    HttpResponse::Ok()
}

/// Upsert env
#[utoipa::path(
    request_body = EnvVar,
    responses(
        (status = 200, description = "Env upserted successfully"),
    ),
    security(
        ("api_key" = [])
    )
)]
#[patch("/api/apps/{id}/env")]
#[tracing::instrument]
async fn upsert_env(
    auth: OwnerRole,
    env: Json<EnvVar>,
    state: Data<AppState>,
    id: Path<i64>,
) -> impl Responder {
    state
        .db
        .upsert_env(id.into_inner(), &env.0.name, &env.0.value)
        .await;
    // state.manager.sync_with_db().await; // TODO: review if its fine not calling sync here
    HttpResponse::Ok()
}

/// Delete env
#[utoipa::path(
    responses(
        (status = 200, description = "Env deleted successfully"),
    ),
    security(
        ("api_key" = [])
    )
)]
#[delete("/api/apps/{id}/env/{name}")]
#[tracing::instrument]
async fn delete_env(
    auth: OwnerRole,
    state: Data<AppState>,
    path: Path<(i64, String)>,
) -> impl Responder {
    state.db.delete_env(path.0, &path.1).await;
    // state.manager.sync_with_db().await; // TODO: review if its fine not calling sync here
    HttpResponse::Ok()
}
