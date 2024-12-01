use actix_web::{
    delete, get, patch, post,
    web::{Data, Json, Path},
    HttpResponse, Responder,
};
use futures::future::join_all;

use crate::{
    api::{
        security::RequireApiKey,
        utils::{get_all_deployments, get_prod_deployment, get_prod_deployment_id},
        AppState, ErrorResponse, FullProjectInfo, ProjectInfo,
    },
    db::{InsertProject, UpdateProject},
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
#[get("/apps", wrap = "RequireApiKey")]
async fn get_projects(state: Data<AppState>) -> impl Responder {
    let projects = state.db.get_projects().await;
    let projects_with_deployments = projects.into_iter().map(|project| {
        let state = state.clone();
        async move {
            let prod_deployment = get_prod_deployment(&state, project.id).await;
            let prod_deployment_id = get_prod_deployment_id(&state.db, &project).await;

            // TODO: if the repo is not available, simply don't return that info
            let repo = state
                .github
                .get_repo(&project.repo_id)
                .await
                .unwrap()
                .unwrap();
            ProjectInfo {
                name: project.name.clone(),
                id: project.id,
                repo: repo.into(),
                created: project.created,
                env: project.env.clone(),
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
#[get("/apps/{name}", wrap = "RequireApiKey")]
async fn get_project(state: Data<AppState>, name: Path<String>) -> impl Responder {
    let name = name.into_inner();
    let project = state.db.get_project_by_name(&name).await;
    match project {
        Some(project) => {
            let repo = state
                .github
                .get_repo(&project.repo_id)
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
#[post("/apps", wrap = "RequireApiKey")] // TODO: return project when successfully inserted
async fn create_project(project: Json<InsertProject>, state: Data<AppState>) -> impl Responder {
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
#[patch("/apps/{id}", wrap = "RequireApiKey")]
async fn update_project(
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
#[delete("/apps/{id}", wrap = "RequireApiKey")]
async fn delete_project(state: Data<AppState>, id: Path<i64>) -> impl Responder {
    state.db.delete_project(id.into_inner()).await;
    state.manager.sync_with_db().await;
    HttpResponse::Ok()
}
