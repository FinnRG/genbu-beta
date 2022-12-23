use axum::{response::IntoResponse, Extension, Json};
use genbu_stores::files::file_storage::{Bucket, FileStore};
use hyper::StatusCode;
use tracing::error;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct FinishUploadRequest {
    name: String,
    upload_id: String,
}

// TODO: Make this configurable
static CHUNK_SIZE: usize = 10_000_000;

async fn single_file_upload_url(
    file_store: impl FileStore,
    bucket: Bucket,
    name: &str,
    size: usize,
) -> Result<(Vec<String>, Option<String>), StatusCode> {
    match file_store
        .get_presigned_upload_urls(bucket, name, size, CHUNK_SIZE)
        .await
    {
        Ok((uris, upload_id)) => Ok((uris, Some(upload_id))),
        Err(e) => {
            error!("file store error {:?}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn multipart_upload_url(
    file_store: impl FileStore,
    bucket: Bucket,
    name: &str,
) -> Result<(Vec<String>, Option<String>), StatusCode> {
    return file_store
        .get_presigned_upload_url(bucket, name)
        .await
        .map(|uri| (vec![uri], None))
        .map_err(|e| {
            error!("file store error {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        });
}

pub(crate) async fn get_presigned_upload_urls(
    file_store: impl FileStore,
    req: super::UploadFileRequest,
) -> Result<(Vec<String>, Option<String>), StatusCode> {
    if req.size <= CHUNK_SIZE {
        return multipart_upload_url(file_store, Bucket::UserFiles, "test_new").await;
    }
    single_file_upload_url(file_store, Bucket::UserFiles, "test", req.size).await
}

#[utoipa::path(
    post,
    path = "/api/files/upload/finish",
    request_body(content = FinishUploadRequest),
    responses(
        (status = 200, description = "File uploaded finished successfully"),
        (status = 500, description = "An internal error occured while uploading")
    )
)]
pub(crate) async fn finish_upload<F: FileStore>(
    Extension(file_store): Extension<F>,
    Json(req): Json<FinishUploadRequest>,
) -> impl IntoResponse {
    file_store
        .finish_multipart_upload(Bucket::UserFiles, &req.name, &req.upload_id)
        .await
        .map_err(|e| {
            error!("{:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}
