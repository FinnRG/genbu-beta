use axum::{response::IntoResponse, Extension, Json};
use hyper::StatusCode;

pub mod multipart_upload;
pub mod upload;

pub mod routes {
    use axum::Router;

    use crate::stores::files::file_storage::FileStore;

    use super::upload::upload_unsigned;
    use super::{get_presigned_url, multipart_upload::finish_upload, upload::upload_file_request};
    use axum::routing::{get, post};

    pub fn router<F: FileStore>() -> Router {
        Router::new()
            .route("/api/files", get(get_presigned_url::<F>))
            .route("/api/files/upload", post(upload_file_request::<F>)) // TODO: COnsider using put
            // instead of post,
            .route("/api/files/upload/unsigned/:id", post(upload_unsigned::<F>)) // TODO: Remove upload
            .route("/api/files/upload/finish", post(finish_upload::<F>))
        //.route_layer(middleware::from_fn(auth))
        // TODO: Add auth middleware back
    }
}

// TODO: Accept any file
#[utoipa::path(
    get,
    tag = "files",
    path = "/api/files",
    responses(
        (status = 200, description = "Upload request is valid and accepted", body = String)
    )
)]
pub async fn get_presigned_url<F: FileStore>(
    Extension(file_store): Extension<F>,
) -> impl IntoResponse {
    file_store
        .get_presigned_url(Bucket::UserFiles, "test_new")
        .await
        .unwrap()
}

use serde_json::json;

use crate::stores::files::file_storage::{Bucket, FileError, FileStore};

pub type APIResult<T> = Result<T, FileAPIError>;

#[derive(Debug)]
pub struct FileAPIError(FileError);

impl From<FileError> for FileAPIError {
    fn from(value: FileError) -> Self {
        Self(value)
    }
}

impl IntoResponse for FileAPIError {
    fn into_response(self) -> axum::response::Response {
        let (status, error_message) = match self.0 {
            FileError::FileNotFound(_) => (StatusCode::NOT_FOUND, "File not found"),
            FileError::FileIsEmpty => (StatusCode::UNPROCESSABLE_ENTITY, "File is empty"),
            FileError::FileTooLarge(_) => (StatusCode::UNPROCESSABLE_ENTITY, "File is too large"),
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
            FileError::IOError(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal IO error"),
        };

        let body = Json(json!({ "error": error_message }));

        (status, body).into_response()
    }
}
