use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
};

use axum::{
    extract::{Multipart, Path},
    response::IntoResponse,
    routing::{get, post},
    Extension, Json, Router,
};
use genbu_stores::{
    files::file_storage::{Bucket, FileStore},
    Uuid,
};
use hyper::StatusCode;
use serde::{Deserialize, Serialize};
use tempfile::tempfile;
use tracing::error;
use utoipa::ToSchema;

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

#[utoipa::path(
    get,
    path = "/api/files",
    responses(
        (status = 200, description = "Upload request is valid and accepted", body = String)
    )
)]
async fn get_presigned_url<F: FileStore>(Extension(file_store): Extension<F>) -> impl IntoResponse {
    file_store
        .get_presigned_url(Bucket::UserFiles, "test_new")
        .await
        .unwrap()
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct UploadFileRequest {
    name: String,
    size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct UploadFileResponse {
    presigned: bool,
    upload_id: Option<String>,
    uris: Option<Vec<String>>,
}

// TODO: Make this configurable
static MAX_FILE_SIZE: usize = 1_000_000_000;

#[utoipa::path(
    post,
    path = "/api/files/upload",
    request_body = UploadFileRequest,
    responses(
        (status = 200, description = "Upload request is valid and accepted", body = UploadFileResponse)
    )
)]
async fn upload_file_request<F: FileStore>(
    Extension(file_store): Extension<F>,
    Json(req): Json<UploadFileRequest>,
) -> impl IntoResponse {
    if req.size > MAX_FILE_SIZE {
        return Err(StatusCode::FORBIDDEN);
    }
    if <F as FileStore>::can_presign() {
        let (uris, upload_id) = get_presigned_upload_urls(file_store, req).await?;
        return Ok(Json(UploadFileResponse {
            presigned: true,
            uris: Some(uris),
            upload_id,
        }));
    }
    Ok(Json(UploadFileResponse {
        presigned: false,
        uris: None,
        upload_id: None,
    }))
}

// TODO: Change this
static CHUNK_SIZE: usize = 10_000_000;

async fn get_presigned_upload_urls<F: FileStore>(
    file_store: F,
    req: UploadFileRequest,
) -> Result<(Vec<String>, Option<String>), StatusCode> {
    if req.size <= CHUNK_SIZE {
        return match file_store
            .get_presigned_upload_url(Bucket::UserFiles, "test_new")
            .await
        {
            Ok(uri) => Ok((vec![uri], None)),
            Err(e) => {
                error!("file store error {:?}", e);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        };
    }
    match file_store
        .get_presigned_upload_urls(Bucket::UserFiles, "test_new", req.size, CHUNK_SIZE)
        .await
    {
        Ok((uris, upload_id)) => Ok((uris, Some(upload_id))),
        Err(e) => {
            error!("file store error {:?}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, ToSchema)]
pub(crate) struct UploadUnsignedRequest {
    #[schema(value_type = String, format = Binary)]
    file: Option<File>,
}

// TODO: Limit max upload size to prevent DOS
#[utoipa::path(
    post,
    path = "/api/files/upload/unsigned/{id}",
    request_body(content = UploadUnsignedRequest, content_type = "multipart/form-data"),
    responses(
        (status = 200, description = "File uploaded successfully"),
        (status = 500, description = "An internal error occured while uploading")
    ),
    params(
        ("id" = Uuid, Path, description = "Upload task id")
    )
)]
async fn upload_unsigned<F: FileStore>(
    Extension(mut file_store): Extension<F>,
    Path(task_id): Path<Uuid>,
    mut multipart: Multipart,
) -> Result<(), StatusCode> {
    // TODO: Use the task_id
    match (tempfile(), multipart.next_field().await) {
        (Ok(mut file), Ok(Some(field))) => {
            let data = field.bytes().await.unwrap();
            // TODO: Better error handling
            file.write(&data).unwrap();
            file.seek(SeekFrom::Start(0)).unwrap();
            file_store
                .upload_file(Bucket::UserFiles, &file, "test_unsigned")
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            Ok(())
        }
        _ => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct FinishUploadRequest {
    name: String,
    upload_id: String,
}

async fn finish_upload<F: FileStore>(
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
