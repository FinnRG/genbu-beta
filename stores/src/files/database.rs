use std::error::Error;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    util::{deep_into, deep_into_vec},
    OffsetDateTime, Uuid,
};

use super::file_storage::Bucket;

#[derive(Debug, Error)]
pub enum UploadLeaseStoreError {
    #[error("unable to establish a file storage connection")]
    Connection(#[source] Box<dyn Error>),

    #[error("unable to find a lease with the id: {0}")]
    LeaseNotFound(Uuid),

    #[error("invalid / no size specified")]
    InvalidSize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadLease {
    pub id: Uuid,
    pub owner: Uuid,
    pub completed: bool,
    pub size: u64,
    pub created_at: OffsetDateTime,
    pub expires_at: OffsetDateTime,
    pub bucket: Bucket,
    pub name: String,
}

#[async_trait]
pub trait UploadLeaseStore {
    type StoreLease: Into<UploadLease>;

    async fn int_add(&mut self, lease: &UploadLease) -> Result<(), UploadLeaseStoreError>;
    #[inline(always)]
    async fn add(&mut self, lease: &UploadLease) -> Result<(), UploadLeaseStoreError> {
        self.int_add(lease).await.map_err(Into::into)
    }

    async fn int_delete(
        &mut self,
        id: &Uuid,
    ) -> Result<Option<Self::StoreLease>, UploadLeaseStoreError>;
    #[inline(always)]
    async fn delete(&mut self, id: &Uuid) -> Result<Option<UploadLease>, UploadLeaseStoreError> {
        deep_into(self.int_delete(id).await)
    }

    async fn int_get(&self, id: &Uuid) -> Result<Option<Self::StoreLease>, UploadLeaseStoreError>;
    #[inline(always)]
    async fn get(&self, id: &Uuid) -> Result<Option<UploadLease>, UploadLeaseStoreError> {
        deep_into(self.int_get(id).await)
    }
    async fn int_get_by_user(
        &self,
        id: &Uuid,
    ) -> Result<Vec<Self::StoreLease>, UploadLeaseStoreError>;
    #[inline(always)]
    async fn get_by_user(&self, id: &Uuid) -> Result<Vec<UploadLease>, UploadLeaseStoreError> {
        deep_into_vec(self.int_get_by_user(id).await)
    }
}
