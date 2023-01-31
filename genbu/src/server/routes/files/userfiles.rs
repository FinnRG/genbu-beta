use axum::{extract::Query, response::IntoResponse, routing::get, Extension, Json, Router};
use genbu_auth::authn::Claims;

use crate::{
    handler::files::userfiles::{self as handler, GetUserfilesRequest},
    stores::files::filesystem::Filesystem,
};

pub fn router<F: Filesystem>() -> Router {
    Router::new().route("/api/filesystem", get(get_userfiles::<F>))
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
pub async fn get_userfiles<F: Filesystem>(
    Extension(filesystem): Extension<F>,
    Extension(claims): Extension<Claims>,
    Query(req): Query<GetUserfilesRequest>,
) -> handler::UserfilesAPIResult<impl IntoResponse> {
    Ok(Json(
        handler::get_userfiles(filesystem, claims.sub, &req).await?,
    ))
}
