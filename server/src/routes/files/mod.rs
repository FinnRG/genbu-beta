use axum::{response::IntoResponse, Extension, Json};
use genbu_stores::files::file_storage::{Bucket, FileStore, FileStoreError};
use hyper::StatusCode;

pub mod multipart_upload;
pub mod upload;

pub mod routes {
    use axum::Router;
    use genbu_stores::files::file_storage::FileStore;

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

pub type APIResult<T> = Result<T, FileAPIError>;

#[derive(Debug)]
pub struct FileAPIError(FileStoreError);

impl From<FileStoreError> for FileAPIError {
    fn from(value: FileStoreError) -> Self {
        Self(value)
    }
}

impl IntoResponse for FileAPIError {
    fn into_response(self) -> axum::response::Response {
        let (status, error_message) = match self.0 {
            FileStoreError::FileNotFound(_) => (StatusCode::NOT_FOUND, "File not found"),
            FileStoreError::FileIsEmpty => (StatusCode::UNPROCESSABLE_ENTITY, "File is empty"),
            FileStoreError::FileTooLarge(_) => {
                (StatusCode::UNPROCESSABLE_ENTITY, "File is too large")
            }
            FileStoreError::Connection(_) => (
                StatusCode::BAD_GATEWAY,
                "Server failed to establish connection to database",
            ),
            FileStoreError::NameAlreadyExists(_) => {
                (StatusCode::CONFLICT, "File with this name already exists")
            }
            FileStoreError::Other(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Unknown internal error")
            }
            FileStoreError::Presigning(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Error during presigning")
            }
            FileStoreError::IOError(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal IO error"),
            _ => todo!(),
        };

        let body = Json(json!({ "error": error_message }));

        (status, body).into_response()
    }
}
