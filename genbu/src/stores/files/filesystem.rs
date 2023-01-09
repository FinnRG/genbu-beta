use std::error::Error;

use crate::stores::Uuid;

use super::FileStorage;

#[derive(Clone, Debug, oso::PolarClass, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct Userfile {
    pub name: String,
    // TODO: Is this really important?
    pub owner: Uuid,
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

pub trait Filesystem: FileStorage {
    async fn list(&self, user_id: Uuid, base_path: &str) -> SResult<Vec<Userfile>>;
    async fn delete(&mut self, path: &str) -> SResult<()>;
}
