use std::error::Error;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use time::OffsetDateTime;
use uuid::Uuid;

use super::storage::Bucket;

#[derive(
    Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize, utoipa::ToSchema, sqlx::Type,
)]
#[sqlx(transparent)]
pub struct LeaseID(Uuid);

#[derive(Debug, Error)]
pub enum UploadLeaseError {
    #[error("unable to establish a file storage connection")]
    Connection(#[source] Box<dyn Error>),

    #[error("unable to find a lease with the id: {0}")]
    LeaseNotFound(Uuid),

    #[error("invalid / no size specified")]
    InvalidSize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadLease {
    pub id: LeaseID,
    pub owner: Uuid,
    pub completed: bool,
    pub size: u64,
    pub created_at: OffsetDateTime,
    pub expires_at: OffsetDateTime,
    pub bucket: Bucket,
    pub name: String,
}

pub type SResult<T> = Result<T, UploadLeaseError>;

#[async_trait]
pub trait UploadLeaseStore {
    async fn add(&mut self, lease: &UploadLease) -> SResult<()>;

    async fn delete(&mut self, id: &Uuid) -> SResult<Option<UploadLease>>;

    async fn get(&self, id: &Uuid) -> SResult<Option<UploadLease>>;
    async fn get_by_user(&self, id: &Uuid) -> SResult<Vec<UploadLease>>;
}
