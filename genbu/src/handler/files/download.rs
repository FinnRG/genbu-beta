use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use thiserror::Error;
use utoipa::{IntoParams, ToSchema};

use crate::stores::{
    files::{
        storage::{Bucket, FileError},
        FileStorage,
    },
    Uuid,
};

use super::userfiles::build_path;

pub type DownloadAPIResult<T> = std::result::Result<T, DownloadAPIError>;
type Result<T> = DownloadAPIResult<T>;

#[derive(Debug, Error)]
pub enum DownloadAPIError {
    #[error("file storage error")]
    StorageError(#[from] FileError),

    #[error("file not found")]
    NotFound(Box<dyn Debug + Send + Sync>),

    #[error("unknown api error")]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, IntoParams)]
pub struct StartDownloadRequest {
    file_path: String,
    bucket: Bucket,
}

#[tracing::instrument(skip(file_storage))]
pub async fn start_download(
    file_storage: impl FileStorage,
    user_id: Uuid,
    req: StartDownloadRequest,
) -> Result<String> {
    let path = build_path(user_id, &req.file_path);
    Ok(match req.bucket {
        Bucket::UserFiles => file_storage.get_download_url(req.bucket, &path).await?,
        _ => unimplemented!(),
    })
}
