use std::error::Error;

use time::{serde::iso8601::option as iso8601, OffsetDateTime};

use crate::stores::Uuid;

use super::FileStorage;

#[derive(Clone, Debug, oso::PolarClass, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct Userfile {
    pub name: String,
    #[serde(with = "iso8601")]
    pub last_modified: Option<OffsetDateTime>,
    pub owner: Uuid,
    /// Size is only None if is_folder is true
    pub size: Option<i64>,
    pub is_folder: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum FilesystemError {
    #[error("a file with this path `{0}` already exists")]
    FileAlreadyExists(String),

    #[error("unable to establish a database connection")]
    Connection(#[source] Box<dyn Error + Send + Sync>),

    #[error("unknown file system error")]
    Other(#[source] Box<dyn Error + Send + Sync>),
}

pub type SResult<T> = Result<T, FilesystemError>;

#[async_trait::async_trait]
pub trait Filesystem: FileStorage {
    async fn list(&self, user_id: Uuid, base_path: &str) -> SResult<Vec<Userfile>>;
    async fn delete(&mut self, path: &str) -> SResult<()>;
}
