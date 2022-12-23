use std::{error::Error, fs::File, io, path::PathBuf};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PresignError {
    #[error("file size {0} is too large")]
    FileTooLarge(usize),

    #[error("file store doesn't support presigning")]
    Unsupported,

    #[error("unknown presign error")]
    Other(#[source] Box<dyn Error>),
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum FileStoreError {
    #[error("unable to establish a file storage connection")]
    Connection(#[source] Box<dyn Error>),

    #[error("a file with this name already exists")]
    NameAlreadyExists(#[source] Box<dyn Error>),

    #[error("unknown file storage error")]
    Other(#[source] Box<dyn Error>),

    #[error("error while presigning operation")]
    Presigning(#[source] PresignError),

    #[error("file not found")]
    FileNotFound(PathBuf),

    #[error("file has size 0")]
    FileIsEmpty,

    #[error("unknown io error")]
    IOError(#[from] io::Error),
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
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

#[async_trait]
pub trait FileStore: Clone + Sized + Send + Sync + 'static {
    fn can_presign() -> bool;
    async fn setup(&mut self) -> Result<(), FileStoreError>;

    async fn upload_file(
        &mut self,
        bucket: Bucket,
        name: &File,
        name: &str,
    ) -> Result<(), FileStoreError>;
    async fn delete_file(&mut self, bucket: Bucket, name: &str) -> Result<(), FileStoreError>;
    async fn get_presigned_url(&self, bucket: Bucket, name: &str)
        -> Result<String, FileStoreError>;
    async fn get_presigned_upload_url(
        &self,
        bucket: Bucket,
        name: &str,
    ) -> Result<String, FileStoreError>;
    async fn get_presigned_upload_urls(
        &self,
        bucket: Bucket,
        name: &str,
        size: usize,
        chunk_size: usize,
    ) -> Result<(Vec<String>, String), FileStoreError>;
    async fn finish_multipart_upload(
        &self,
        bucket: Bucket,
        name: &str,
        upload_id: &str,
    ) -> Result<(), FileStoreError>;
}
