use std::{
    fs::File,
    io::{Seek, SeekFrom, Write},
};

use axum::{extract::Path, Extension, Json};

use crate::stores::{
    files::storage::{Bucket, FileError, FileStorage},
    Uuid,
};

use super::{multipart_upload::get_presigned_upload_urls, APIResult};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct UploadFileRequest {
    pub name: String,
    pub size: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct UploadFileResponse {
    pub presigned: bool,
    pub upload_id: Option<String>,
    pub uris: Option<Vec<String>>,
}

// TODO: Make this configurable
static MAX_FILE_SIZE: usize = 1_000_000_000;

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
pub async fn upload_file_request<F: FileStorage>(
    Extension(file_store): Extension<F>,
    Json(req): Json<UploadFileRequest>,
) -> APIResult<Json<UploadFileResponse>> {
    if req.size > MAX_FILE_SIZE {
        return Err(FileError::FileTooLarge(req.size).into());
    }
    if <F as FileStorage>::can_presign() {
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

#[allow(dead_code)]
#[derive(Debug, utoipa::ToSchema)]
pub struct UploadUnsignedRequest {
    #[schema(value_type = String, format = Binary)]
    file: Option<File>,
}

// TODO: Limit max upload size to prevent DOS
#[utoipa::path(
    post,
    tag = "files",
    path = "/api/files/upload/unsigned/{id}",
    request_body(content = UploadUnsignedRequest),
    responses(
        (status = 200, description = "File uploaded successfully"),
        (status = 500, description = "An internal error occured while uploading")
    ),
    params(
        ("id" = Uuid, Path, description = "Upload task id")
    )
)]
// TODO: Use the task_id
pub async fn upload_unsigned<F: FileStorage>(
    Extension(mut file_store): Extension<F>,
    Path(task_id): Path<Uuid>,
    bytes: bytes::Bytes,
) -> APIResult<()> {
    let file = tempfile::tempfile();
    let mut file = match file {
        Ok(file) => file,
        Err(e) => return Err(FileError::IOError(e).into()),
    };
    write_part_to_file(&mut file, bytes).await?;
    Ok(file_store
        .upload_file(Bucket::UserFiles, &file, "test_unsigned")
        .await?)
}

async fn write_part_to_file(file: &mut File, data: bytes::Bytes) -> Result<(), FileError> {
    file.write_all(&data)?;
    file.seek(SeekFrom::Start(0))?;
    Ok(())
}
