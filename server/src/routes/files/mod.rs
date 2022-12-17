use std::io::Write;

use axum::{extract::Multipart, response::IntoResponse, routing::post, Extension, Json, Router};
use genbu_stores::files::{Bucket, FileStore};
use hyper::StatusCode;
use serde::{Deserialize, Serialize};
use tempfile::tempfile;
use tracing::error;
use utoipa::ToSchema;

pub fn router<F: FileStore>() -> Router {
    Router::new()
        .route("/api/files/setup", post(setup_files::<F>))
        .route("/api/files/upload", post(upload_file_request::<F>)) // TODO: COnsider using put
        // instead of post,
        .route("/api/files/upload/unsigned", post(upload_unsigned::<F>))
    //.route_layer(middleware::from_fn(auth))
    // TODO: Add auth middleware back
}

// TODO: This should be implemented in a startup routine in builder, not as an endpoint
#[utoipa::path(
    post,
    path = "/api/files/setup",
    responses(
        (status = 200, description = "File setup completed", body = UploadFileResponse)
    )
)]
async fn setup_files<F: FileStore>(Extension(mut file_store): Extension<F>) -> impl IntoResponse {
    println!("{:?}", file_store.setup().await);
    // println!(
    //     "{:?}",
    //     file_store
    //         .upload_file(Bucket::UserFiles, std::path::Path::new("deny.toml"), "deny")
    //         .await
    // );
    // println!(
    //     "{:?}",
    //     file_store.delete_file(Bucket::UserFiles, "deny").await
    // );
    StatusCode::OK
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
struct UploadFileRequest {
    name: String,
    size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
struct UploadFileResponse {
    presigned: bool,
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
        let uris = get_presigned_urls(file_store, req).await?;
        return Ok(Json(UploadFileResponse {
            presigned: true,
            uris: Some(uris),
        }));
    }
    Ok(Json(UploadFileResponse {
        presigned: false,
        uris: None,
    }))
}

static CHUNK_SIZE: usize = 5_000_000;

async fn get_presigned_urls<F: FileStore>(
    file_store: F,
    req: UploadFileRequest,
) -> Result<Vec<String>, StatusCode> {
    if req.size <= CHUNK_SIZE {
        return match file_store
            .get_presigned_upload_url(Bucket::UserFiles, "test")
            .await
        {
            Ok(uri) => Ok(vec![uri]),
            Err(e) => {
                error!("file store error {:?}", e);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        };
    }
    match file_store
        .get_presigned_upload_urls(Bucket::UserFiles, "test", req.size, CHUNK_SIZE)
        .await
    {
        Ok(uris) => Ok(uris),
        Err(e) => {
            error!("file store error {:?}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// TODO: Limit max upload size to prevent DOS
async fn upload_unsigned<F: FileStore>(
    Extension(mut file_store): Extension<F>,
    mut multipart: Multipart,
) -> Result<(), StatusCode> {
    match (tempfile(), multipart.next_field().await) {
        (Ok(mut file), Ok(Some(field))) => {
            let data = field.bytes().await.unwrap();
            // TODO: Better error handling
            file.write(&data)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            file_store
                .upload_file(Bucket::UserFiles, &file, "test")
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            Ok(())
        }
        _ => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}
