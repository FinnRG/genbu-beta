use axum::{response::IntoResponse, routing::post, Extension, Json, Router};
use hyper::StatusCode;

use serde_json::json;

use crate::{
    handler::files::upload as handler,
    handler::files::upload::UploadAPIError,
    stores::{
        files::{
            storage::{FileError, FileStorage},
            UploadLeaseError, UploadLeaseStore,
        },
        users::User,
        DataStore,
    },
};

pub fn router<F: FileStorage, L: DataStore>() -> Router {
    Router::new()
        .route("/api/files/upload", post(upload_file_request::<F, L>)) // TODO: COnsider using put
        // instead of post,
        .route("/api/files/upload/finish", post(finish_upload::<F, L>))
    //.route_layer(middleware::from_fn(auth))
    // TODO: Add auth middleware back
}

#[utoipa::path(
    post,
    tag = "files",
    path = "/api/files/upload",
    request_body = UploadFileRequest,
    responses(
        (status = 200, description = "Upload request is valid and accepted", body = UploadFileResponse),
        (status = 422, description = "Upload request is invalid (i.e. file is too large)")
    )
)]
pub async fn upload_file_request<F: FileStorage, L: UploadLeaseStore>(
    Extension(file_storage): Extension<F>,
    Extension(lease_store): Extension<L>,
    Json(req): Json<handler::UploadFileRequest>,
) -> handler::UploadAPIResult<Json<handler::UploadFileResponse>> {
    // TODO: Get current
    let res = handler::post(file_storage, lease_store, &User::template(), req).await?;
    Ok(Json(res))
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
pub async fn finish_upload<F: FileStorage, L: UploadLeaseStore>(
    Extension(file_storage): Extension<F>,
    Extension(lease_store): Extension<L>,
    Json(req): Json<handler::FinishUploadRequest>,
) -> handler::UploadAPIResult<()> {
    handler::finish_upload(file_storage, lease_store, req).await
}

impl IntoResponse for FileError {
    fn into_response(self) -> axum::response::Response {
        let (status, error_message) = match self {
            FileError::Connection(_) => (
                StatusCode::BAD_GATEWAY,
                "Server failed to establish connection to database",
            ),
            FileError::NameAlreadyExists(_) => {
                (StatusCode::CONFLICT, "File with this name already exists")
            }
            FileError::Other(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Unknown internal error"),
            FileError::Presigning(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Error during presigning")
            }
        };

        let body = Json(json!({ "error": error_message }));

        (status, body).into_response()
    }
}

impl IntoResponse for UploadLeaseError {
    fn into_response(self) -> axum::response::Response {
        let (status, error_message) = match self {
            Self::Connection(_) => (
                StatusCode::BAD_GATEWAY,
                "Server failed to establish connection to database",
            ),
            Self::InvalidSize => (StatusCode::BAD_REQUEST, "Invalid file size"),
            Self::LeaseExpired(_) => (StatusCode::GONE, "Upload lease expired"),
            Self::Other(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Unknown internal error"),
        };

        let body = Json(json!({ "error": error_message }));

        (status, body).into_response()
    }
}

impl IntoResponse for UploadAPIError {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::StorageError(e) => e.into_response(),
            Self::DatabaseError(e) => e.into_response(),
            Self::FileTooLarge(size, max_size) => (
                StatusCode::BAD_REQUEST,
                format!("file size {size} exceeds maximum {max_size}"),
            )
                .into_response(),
            Self::NotFound(_) => (StatusCode::NOT_FOUND, "Upload lease not found").into_response(),
            Self::Unknown => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Unknown internal error").into_response()
            }
        }
    }
}
