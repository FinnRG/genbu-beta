use std::{
    fs::File,
    io::{Seek, SeekFrom, Write},
};

use axum::{
    extract::{multipart::Field, Multipart, Path},
    Extension, Json,
};

use crate::stores::{
    files::file_storage::{Bucket, FileError, FileStore},
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
    presigned: bool,
    upload_id: Option<String>,
    uris: Option<Vec<String>>,
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
        (status = 403, description = "Upload request is invalid")
    )
)]
pub async fn upload_file_request<F: FileStore>(
    Extension(file_store): Extension<F>,
    Json(req): Json<UploadFileRequest>,
) -> APIResult<Json<UploadFileResponse>> {
    if req.size > MAX_FILE_SIZE {
        return Err(FileError::FileTooLarge(req.size).into());
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
    request_body(content = UploadUnsignedRequest, content_type = "multipart/form-data"),
    responses(
        (status = 200, description = "File uploaded successfully"),
        (status = 500, description = "An internal error occured while uploading")
    ),
    params(
        ("id" = Uuid, Path, description = "Upload task id")
    )
)]
// TODO: Use the task_id
pub async fn upload_unsigned<F: FileStore>(
    Extension(mut file_store): Extension<F>,
    Path(task_id): Path<Uuid>,
    mut multipart: Multipart,
) -> APIResult<()> {
    let file = tempfile::tempfile();
    let field = multipart.next_field().await;
    let (mut file, field) = match (file, field) {
        (Ok(file), Ok(Some(field))) => (file, field),
        (Err(e), _) => return Err(FileError::IOError(e).into()),
        (_, Err(e)) => return Err(FileError::Other(Box::new(e)).into()),
        // TODO: Rethink this error message
        (_, Ok(None)) => return Err(FileError::FileIsEmpty.into()),
    };
    write_part_to_file(&mut file, field).await?;
    Ok(file_store
        .upload_file(Bucket::UserFiles, &file, "test_unsigned")
        .await?)
}

async fn write_part_to_file(file: &mut File, field: Field<'_>) -> Result<(), FileError> {
    let data = field
        .bytes()
        .await
        .map_err(|e| FileError::Other(Box::new(e)))?;
    file.write_all(&data)?;
    file.seek(SeekFrom::Start(0))?;
    Ok(())
}
