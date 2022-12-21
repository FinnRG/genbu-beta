use std::error::Error;

use async_trait::async_trait;
use thiserror::Error;

use crate::Uuid;

#[derive(Debug, Error)]
pub enum UploadLeaseStoreError {
    #[error("unable to establish a file storage connection")]
    Connection(#[source] Box<dyn Error>),

    #[error("unable to find a lease with the id: {0}")]
    LeaseNotFound(Uuid),
}

pub struct UploadLease {
    id: Uuid,
}

#[async_trait]
pub trait UploadLeaseStore {}
