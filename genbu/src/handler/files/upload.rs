use std::fmt::Debug;

use thiserror::Error;

use crate::stores::{
    files::{
        database::LeaseID, storage::FileError, FileStorage, UploadLease, UploadLeaseError,
        UploadLeaseStore,
    },
    users::User,
};

pub type FileAPIResult<T> = std::result::Result<T, APIError>;

static MAX_FILE_SIZE: u64 = 1_000_000_000;
static CHUNK_SIZE: u64 = 10_000_000;

#[derive(Debug, Error)]
pub enum APIError {
    #[error("file storage error")]
    StorageError(#[from] FileError),

    #[error("lease store error")]
    DatabaseError(#[from] UploadLeaseError),

    #[error("file too large, {0} exceeds {1}")]
    FileTooLarge(u64, u64),

    #[error("file not found")]
    NotFound(Box<dyn Debug + Send + Sync>),

    #[error("lease for upload expired")]
    LeaseExpired,

    #[error("unknown api error")]
    Unknown,
}

type Result<T> = FileAPIResult<T>;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct UploadFileRequest {
    pub name: String,
    pub size: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct UploadFileResponse {
    pub upload_id: Option<String>,
    pub uris: Vec<String>,
}

/// The handler for direct uploads to the userfiles bucket. This can't be used
/// for uploads to other buckets like videofiles or notebookfiles.
pub async fn post(
    file_storage: impl FileStorage,
    mut lease_store: impl UploadLeaseStore,
    user: &User,
    upload_req: UploadFileRequest,
) -> Result<UploadFileResponse> {
    if upload_req.size > MAX_FILE_SIZE {
        return Err(APIError::FileTooLarge(upload_req.size, MAX_FILE_SIZE));
    }

    let lease = lease_store
        .add(&UploadLease {
            owner: user.id,
            size: upload_req.size as i64,
            name: user.id.to_string() + &upload_req.name,
            ..UploadLease::template()
        })
        .await?;

    let (uris, upload_id) = get_presigned_upload_urls(file_storage, &lease).await?;
    return Ok(UploadFileResponse { uris, upload_id });
}

/// The handler for getting the uploads urls for any upload that was previously registered
/// with an ```UploadLease```
pub async fn get(
    file_storage: impl FileStorage,
    lease_store: impl UploadLeaseStore,
    lease_id: LeaseID,
) -> Result<UploadFileResponse> {
    let lease = lease_store
        .get(&lease_id)
        .await?
        .ok_or(APIError::NotFound(Box::new(lease_id)))?;
    let (uris, upload_id) = get_presigned_upload_urls(file_storage, &lease).await?;
    return Ok(UploadFileResponse { uris, upload_id });
}

async fn get_presigned_upload_urls(
    file_storage: impl FileStorage,
    lease: &UploadLease,
) -> Result<(Vec<String>, Option<String>)> {
    debug_assert!(lease.size > 0);
    let (uris, upload_id) = file_storage
        .get_presigned_upload_urls(lease.bucket, &lease.name, lease.size as u64, CHUNK_SIZE)
        .await?;
    Ok((uris, Some(upload_id)))
}
