use axum::{
    extract::{Query, State},
    response::IntoResponse,
    routing::get,
    Extension, Json, Router,
};
use genbu_auth::authn::Claims;

use crate::{
    handler::files::userfiles::{self as handler, DeleteUserfileRequest, GetUserfilesRequest},
    server::routes::AppState,
};

pub fn router<S: AppState>() -> Router<S> {
    Router::new().route(
        "/api/filesystem",
        get(get_userfiles::<S>).delete(delete_userfile::<S>),
    )
}

#[utoipa::path(
    get,
    tag = "files",
    path = "/api/filesystem",
    params(
        GetUserfilesRequest
    ),
    responses(
        (status = 200, description = "List all userfiles successfully", body = GetUserfilesResponse)
    )
)]
pub async fn get_userfiles<S: AppState>(
    State(state): State<S>,
    Extension(claims): Extension<Claims>,
    Query(req): Query<GetUserfilesRequest>,
) -> handler::UserfilesAPIResult<impl IntoResponse> {
    Ok(Json(
        handler::get_userfiles(state.file(), claims.sub, &req).await?,
    ))
}

#[utoipa::path(
    delete,
    tag = "files",
    path = "/api/filesystem",
    params(DeleteUserfileRequest),
    responses(
        (status = 200, description = "File deleted successfully")
    )
)]
pub async fn delete_userfile<S: AppState>(
    State(state): State<S>,
    Extension(claims): Extension<Claims>,
    Query(req): Query<DeleteUserfileRequest>,
) -> handler::UserfilesAPIResult<()> {
    handler::delete_userfile(state.file(), claims.sub, req).await?;
    Ok(())
}
