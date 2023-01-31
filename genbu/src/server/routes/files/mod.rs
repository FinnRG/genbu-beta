use axum::{
    response::IntoResponse,
    routing::{get, post},
    Extension, Json, Router,
};
use bytes::Bytes;
use http::Response;
use hyper::StatusCode;

use serde_json::json;

use crate::{
    handler::files::upload as handler,
    handler::files::{upload::UploadAPIError, wopi as wopi_handler},
    stores::{
        files::{
            database::DBFileStore,
            filesystem::Filesystem,
            storage::{FileError, FileStorage},
            UploadLeaseError, UploadLeaseStore,
        },
        users::User,
        DataStore,
    },
};

use self::wopi::{Wopi, WopiResponse};

pub mod wopi;

pub fn router<F: FileStorage + Filesystem, L: DataStore>() -> Router {
    Router::new()
        .route("/api/files/upload", post(upload_file_request::<F, L>)) // TODO: COnsider using put
        // instead of post,
        .route("/api/files/upload/finish", post(finish_upload::<F, L>))
        // .route("/api/wopi/files/:id/contents", get(todo!()))
        .route(
            "/api/wopi/files/:id",
            get(wopi_check_file_info::<F, L>), // .post(todo!())
        )
    //.route_layer(middleware::from_fn(auth))
    // TODO: Add auth middleware back
}

pub async fn wopi_check_file_info<F: Filesystem, D: DBFileStore>(
    Extension(file_storage): Extension<F>,
    Extension(db_file_store): Extension<D>,
    Wopi(req): Wopi<Bytes>,
) -> impl IntoResponse {
    let user = User::template();
    let resp = wopi_handler::wopi_file(file_storage, db_file_store, &user, req).await;
    WopiResponse(resp)
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
    // TODO: Get current user
    Ok(Json(
        handler::post(file_storage, lease_store, &User::template(), req).await?,
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
            Self::NegativeSize(_) | Self::Unknown => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Unknown internal error").into_response()
            }
        }
    }
}
