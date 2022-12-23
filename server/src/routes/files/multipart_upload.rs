use axum::{Extension, Json};
use genbu_stores::files::file_storage::{Bucket, FileStore};

use super::{upload::UploadFileRequest, APIResult};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct FinishUploadRequest {
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
) -> APIResult<(Vec<String>, Option<String>)> {
    let (uris, upload_id) = file_store
        .get_presigned_upload_urls(bucket, name, size, CHUNK_SIZE)
        .await?;
    Ok((uris, Some(upload_id)))
}

async fn multipart_upload_url(
    file_store: impl FileStore,
    bucket: Bucket,
    name: &str,
) -> APIResult<(Vec<String>, Option<String>)> {
    let res = file_store
        .get_presigned_upload_url(bucket, name)
        .await
        .map(|uri| (vec![uri], None))?;
    Ok(res)
}

pub async fn get_presigned_upload_urls(
    file_store: impl FileStore,
    req: UploadFileRequest,
) -> APIResult<(Vec<String>, Option<String>)> {
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
pub async fn finish_upload<F: FileStore>(
    Extension(file_store): Extension<F>,
    Json(req): Json<FinishUploadRequest>,
) -> APIResult<()> {
    file_store
        .finish_multipart_upload(Bucket::UserFiles, &req.name, &req.upload_id)
        .await?;
    Ok(())
}
