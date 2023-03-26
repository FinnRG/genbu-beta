use axum::{
    extract::{Query, State},
    middleware,
    response::{IntoResponse, Redirect},
    routing::{get, post},
    Extension, Json, Router,
};
use bytes::Bytes;
use genbu_auth::authn::Claims;
use hyper::StatusCode;

use serde_json::json;
use tracing::error;
use wopi_rs::{content::FileContentRequest, file::FileRequest};

use crate::{
    handler::files::upload as handler,
    handler::files::{
        download as download_handler,
        download::{DownloadAPIError, StartDownloadRequest},
        upload::UploadAPIError,
        userfiles::UserfilesAPIError,
        wopi as wopi_handler,
    },
    server::middlewares::auth::auth,
    stores::files::{database::DBFileError, storage::FileError, UploadLeaseError},
};

use self::wopi::{Wopi, WopiResponse};

use super::AppState;

pub mod userfiles;
pub mod wopi;

pub fn router<S: AppState>() -> Router<S> {
    Router::new()
        .merge(userfiles::router())
        .route("/api/files/download", get(start_download::<S>))
        .route("/api/files/upload", post(upload_file_request::<S>)) // TODO: COnsider using put
        // instead of post,
        .route("/api/files/upload/finish", post(finish_upload::<S>))
        .route(
            "/api/wopi/files/:id",
            get(wopi_check_file_info::<S>), // .post(todo!())
        )
        .route("/api/wopi/files/:id/contents", get(wopi_file_content::<S>))
        .route_layer(middleware::from_fn(auth))
}

#[utoipa::path(
    get,
    tag = "files",
    path = "/api/files/download",
    params(StartDownloadRequest),
    responses(
        (status = 307, description = "Redirect to file location")
    )
)]
pub async fn start_download<S: AppState>(
    State(state): State<S>,
    Extension(user): Extension<Claims>,
    Query(req): Query<StartDownloadRequest>,
) -> download_handler::DownloadAPIResult<Redirect> {
    let redirect = download_handler::start_download(state.file(), user.sub, req).await?;
    Ok(Redirect::temporary(&redirect))
}

pub async fn wopi_check_file_info<S: AppState>(
    State(state): State<S>,
    Extension(user): Extension<Claims>,
    Wopi(req): Wopi<FileRequest<Bytes>>,
) -> impl IntoResponse {
    let resp = wopi_handler::wopi_file(state, user.sub, req).await;
    WopiResponse(resp)
}

pub async fn wopi_file_content<S: AppState>(
    State(state): State<S>,
    Extension(user): Extension<Claims>,
    Wopi(req): Wopi<FileContentRequest<Bytes>>,
) -> impl IntoResponse {
    let resp = wopi_handler::wopi_file_content(state, user.sub, req).await;
    WopiResponse(resp)
}

#[utoipa::path(
    post,
    tag = "files",
    path = "/api/files/upload",
    request_body = UploadFileRequest,
    responses(
        (status = 200, description = "Upload request is valid and accepted", body = UploadFileResponse),
        (status = 400, description = "Upload request is invalid (i.e. negative size)"),
        (status = 409, description = "Upload request is forbidden (i.e. file is too large)")
    )
)]
pub async fn upload_file_request<S: AppState>(
    State(state): State<S>,
    Extension(user): Extension<Claims>,
    Json(req): Json<handler::UploadFileRequest>,
) -> handler::UploadAPIResult<Json<handler::UploadFileResponse>> {
    Ok(Json(
        handler::post(state.file(), state.store(), user.sub, req).await?,
    ))
}

#[utoipa::path(
    post,
    tag = "files",
    path = "/api/files/upload/finish",
    request_body(content = FinishUploadRequest),
    responses(
        (status = 200, description = "File uploaded finished successfully"),
        (status = 500, description = "An internal error occured while uploading")
    )
)]
pub async fn finish_upload<S: AppState>(
    State(state): State<S>,
    Extension(user): Extension<Claims>,
    Json(req): Json<handler::FinishUploadRequest>,
) -> handler::UploadAPIResult<()> {
    handler::finish_upload(state, user.sub, req).await
}

impl IntoResponse for FileError {
    fn into_response(self) -> axum::response::Response {
        let (status, error_message) = match self {
            Self::Connection(_) => (
                StatusCode::BAD_GATEWAY,
                "Server failed to establish connection to database",
            ),
            Self::Other(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Unknown internal error"),
            Self::Presigning(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Error during presigning"),
        };

        let body = Json(json!({ "error": error_message }));

        (status, body).into_response()
    }
}

impl IntoResponse for UploadLeaseError {
    fn into_response(self) -> axum::response::Response {
        let resp = match self {
            Self::Connection(_) => (
                StatusCode::BAD_GATEWAY,
                "Server failed to establish connection to database",
            ),
            Self::InvalidSize => (StatusCode::BAD_REQUEST, "Invalid file size"),
            Self::LeaseExpired(_) => (StatusCode::GONE, "Upload lease expired"),
            Self::Other(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Unknown internal error"),
        };

        resp.into_response()
    }
}

impl IntoResponse for DBFileError {
    fn into_response(self) -> axum::response::Response {
        let resp = match self {
            DBFileError::Connection(_) => (
                StatusCode::BAD_GATEWAY,
                "Server failed to establish connection to database",
            ),
            DBFileError::Locked(_) => (StatusCode::CONFLICT, "File is already locked"),
            DBFileError::Other(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Unknown internal error"),
        };

        resp.into_response()
    }
}

impl IntoResponse for UploadAPIError {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::StorageError(e) => e.into_response(),
            Self::UploadLeaseError(e) => e.into_response(),
            Self::DBFileError(e) => e.into_response(),
            Self::FileTooLarge(size, max_size) => (
                StatusCode::FORBIDDEN,
                format!("file size {size} exceeds maximum {max_size}"),
            )
                .into_response(),
            Self::NotFound(_) => (StatusCode::NOT_FOUND, "Upload lease not found").into_response(),
            Self::NegativeSize(_) => {
                (StatusCode::BAD_REQUEST, "File size is negative").into_response()
            }
            Self::Unknown => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Unknown internal error").into_response()
            }
        }
    }
}

impl IntoResponse for UserfilesAPIError {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::NotFound(_) => (StatusCode::NOT_FOUND, "User file not found").into_response(),
            Self::Filesystem(e) => e.into_response(),
        }
    }
}

impl IntoResponse for DownloadAPIError {
    fn into_response(self) -> axum::response::Response {
        match self {
            DownloadAPIError::StorageError(e) => {
                error!("file storage error {e:?}");
                (StatusCode::INTERNAL_SERVER_ERROR, "Unknown internal error").into_response()
            }
            DownloadAPIError::NotFound(_) => {
                (StatusCode::NOT_FOUND, "File not found").into_response()
            }
            DownloadAPIError::Unknown => {
                error!("unknown internal error!");
                (StatusCode::INTERNAL_SERVER_ERROR, "Unknown internal error").into_response()
            }
        }
    }
}
