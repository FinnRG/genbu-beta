use std::error::Error;

use crate::stores::Uuid;

use super::FileStorage;

#[derive(Clone, Debug, oso::PolarClass, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct Userfile {
    pub id: Uuid,
    pub name: String,
    pub owner: Uuid,
    pub is_folder: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum FilesystemError {
    #[error("a file with this path `{0}`already exists")]
    FileAlreadyExists(String),

    #[error("unable to establish a database connection")]
    Connection(#[source] Box<dyn Error + Send + Sync>),

    #[error("unknown file system error")]
    Other(#[source] Box<dyn Error + Send + Sync>),
}

pub type SResult<T> = Result<T, FilesystemError>;

#[async_trait::async_trait]
pub trait Filesystem: FileStorage {
    async fn list(user_id: Uuid, base_path: &str) -> SResult<Option<Userfile>>;
    async fn delete(user_id: Uuid, path: &str) -> SResult<Option<Userfile>>;
}
