use std::error::Error;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::Duration;

use crate::stores::{OffsetDateTime, Uuid};

use super::storage::Bucket;

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, utoipa::ToSchema, sqlx::Type,
)]
#[sqlx(transparent)]
#[serde(transparent)]
pub struct LeaseID(Uuid);

#[derive(Debug, Error)]
pub enum UploadLeaseError {
    #[error("unable to establish a file storage connection")]
    Connection(#[source] Box<dyn Error>),

    #[error("invalid / no size specified")]
    InvalidSize,

    #[error("lease {0:?} expired")]
    LeaseExpired(LeaseID),

    #[error("unknown internal error")]
    Other(#[source] Box<dyn Error>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadLease {
    pub id: LeaseID,
    pub s3_upload_id: String,
    pub owner: Uuid,
    pub completed: bool,
    pub size: i64,
    pub created_at: OffsetDateTime,
    pub expires_at: OffsetDateTime,
    pub bucket: Bucket,
    pub name: String,
}

impl UploadLease {
    #[must_use]
    pub fn template() -> Self {
        Self {
            id: LeaseID(Uuid::new_v4()),
            s3_upload_id: String::new(),
            owner: Uuid::new_v4(),
            completed: false,
            size: -1,
            created_at: OffsetDateTime::now_utc(),
            expires_at: OffsetDateTime::now_utc() + Duration::hours(6),
            bucket: Bucket::UserFiles,
            name: "template-file-name".to_owned(),
        }
    }
}

pub type SResult<T> = Result<T, UploadLeaseError>;

#[async_trait]
pub trait UploadLeaseStore: Sized + Send + Sync + Clone + 'static {
    async fn add(&mut self, lease: &UploadLease) -> SResult<UploadLease>;

    async fn delete(&mut self, id: &LeaseID) -> SResult<Option<UploadLease>>;

    async fn get(&self, id: &LeaseID) -> SResult<Option<UploadLease>>;
    async fn get_by_user(&self, id: &Uuid) -> SResult<Vec<UploadLease>>;

    async fn mark_completed(&mut self, id: &LeaseID) -> SResult<Option<UploadLease>>;
}
