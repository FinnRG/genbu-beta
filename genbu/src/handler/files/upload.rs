use std::fmt::Debug;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use utoipa::ToSchema;

use crate::stores::{
    files::{
        database::LeaseID,
        storage::{FileError, Part},
        FileStorage, UploadLease, UploadLeaseError, UploadLeaseStore,
    },
    Uuid,
};

pub type UploadAPIResult<T> = std::result::Result<T, UploadAPIError>;

// TODO: Make this configurable?
static MAX_FILE_SIZE: u64 = 1_000_000_000;
static CHUNK_SIZE: u64 = 10_000_000;

#[derive(Debug, Error)]
pub enum UploadAPIError {
    #[error("file storage error")]
    StorageError(#[from] FileError),

    #[error("lease store error")]
    DatabaseError(#[from] UploadLeaseError),

    #[error("file too large, {0} exceeds {1}")]
    FileTooLarge(u64, u64),

    #[error("file not found")]
    NotFound(Box<dyn Debug + Send + Sync>),

    #[error("size {0} is negative")]
    NegativeSize(i64),

    #[error("unknown api error")]
    Unknown,
}

type Result<T> = UploadAPIResult<T>;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UploadFileRequest {
    pub name: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UploadFileResponse {
    pub lease_id: LeaseID,
    pub upload_id: Option<String>,
    pub uris: Vec<String>,
}

/// The handler for direct uploads to the userfiles bucket. This can't be used
/// for uploads to other buckets like videofiles or notebookfiles.
#[tracing::instrument(skip(file_storage, lease_store))]
pub async fn post(
    file_storage: impl FileStorage,
    mut lease_store: impl UploadLeaseStore,
    user_id: Uuid,
    upload_req: UploadFileRequest,
) -> Result<UploadFileResponse> {
    if upload_req.size > MAX_FILE_SIZE {
        return Err(UploadAPIError::FileTooLarge(upload_req.size, MAX_FILE_SIZE));
    }

    let size = upload_req
        .size
        .try_into()
        .map_err(|_| UploadAPIError::Unknown)?;

    let lease = lease_store
        .add(&UploadLease {
            owner: user_id,
            size,
            name: user_id.to_string() + "\\" + &upload_req.name,
            ..UploadLease::template()
        })
        .await?;

    let (uris, upload_id) = get_presigned_upload_urls(file_storage, &lease).await?;
    Ok(UploadFileResponse {
        upload_id,
        uris,
        lease_id: lease.id,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GetUrisRequest {
    lease_id: LeaseID,
}

/// The handler for getting the uploads urls for any upload that was previously registered
/// with an ```UploadLease```
#[tracing::instrument(skip(file_storage, lease_store), err(Debug))]
pub async fn get(
    file_storage: impl FileStorage,
    lease_store: impl UploadLeaseStore,
    start_req: GetUrisRequest,
) -> Result<UploadFileResponse> {
    let lease_id = start_req.lease_id;
    let lease = lease_store
        .get(&lease_id)
        .await?
        .ok_or(UploadAPIError::NotFound(Box::new(lease_id)))?;
    let (uris, upload_id) = get_presigned_upload_urls(file_storage, &lease).await?;
    Ok(UploadFileResponse {
        upload_id,
        uris,
        lease_id: lease.id,
    })
}

#[tracing::instrument(skip(file_storage), err(Debug))]
async fn get_presigned_upload_urls(
    file_storage: impl FileStorage,
    lease: &UploadLease,
) -> Result<(Vec<String>, Option<String>)> {
    let size = lease
        .size
        .try_into()
        .map_err(|_| UploadAPIError::NegativeSize(lease.size))?;
    let (uris, upload_id) = file_storage
        .get_presigned_upload_urls(lease.bucket, &lease.name, size, CHUNK_SIZE)
        .await?;
    Ok((uris, Some(upload_id)))
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct FinishUploadRequest {
    lease_id: LeaseID,
    upload_id: String,
    parts: Vec<Part>,
}

#[tracing::instrument(skip(file_storage, lease_store), err(Debug))]
pub async fn finish_upload(
    file_storage: impl FileStorage,
    mut lease_store: impl UploadLeaseStore,
    finish_req: FinishUploadRequest,
) -> Result<()> {
    let lease_id = finish_req.lease_id;
    let Some(lease) = lease_store.mark_completed(&lease_id).await? else {
        return Err(UploadAPIError::NotFound(Box::new(lease_id)))
    };

    file_storage
        .finish_multipart_upload(
            lease.bucket,
            &lease.name,
            &finish_req.upload_id,
            finish_req.parts,
        )
        .await?;
    Ok(())
}
