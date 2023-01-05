use std::error::Error;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::stores::{Reset, Setup};

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PresignError {
    #[error("file store doesn't support presigning")]
    Unsupported,

    #[error("unknown presign error")]
    Other(#[source] Box<dyn Error>),
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum FileError {
    #[error("unable to establish a file storage connection")]
    Connection(#[source] Box<dyn Error>),

    #[error("a file with this name already exists")]
    NameAlreadyExists(#[source] Box<dyn Error>),

    #[error("unknown file storage error")]
    Other(#[source] Box<dyn Error>),

    #[error("error while presigning operation")]
    Presigning(#[source] PresignError),
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "bucket", rename_all = "lowercase")]
pub enum Bucket {
    ProfileImages,
    VideoFiles,
    UserFiles,
    NotebookFiles,
}

impl Bucket {
    #[must_use]
    pub const fn to_bucket_name(&self) -> &str {
        match self {
            Self::ProfileImages => "avatars",
            Self::VideoFiles => "videos",
            Self::UserFiles => "userfiles",
            Self::NotebookFiles => "notebookfiles",
        }
    }
}

pub type SResult<T> = Result<T, FileError>;

#[async_trait]
pub trait FileStorage: Reset + Setup + Clone + Sized + Send + Sync + 'static {
    async fn delete_file(&mut self, bucket: Bucket, name: &str) -> SResult<()>;
    async fn get_presigned_url(&self, bucket: Bucket, name: &str) -> SResult<String>;
    async fn get_presigned_upload_url(&self, bucket: Bucket, name: &str) -> SResult<String>;
    async fn get_presigned_upload_urls(
        &self,
        bucket: Bucket,
        name: &str,
        size: u64,
        chunk_size: u64,
    ) -> SResult<(Vec<String>, String)>;
    async fn finish_multipart_upload(
        &self,
        bucket: Bucket,
        name: &str,
        upload_id: &str,
    ) -> SResult<()>;
}
