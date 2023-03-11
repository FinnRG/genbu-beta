use std::error::Error;

use oso::PolarClass;
use serde::{Deserialize, Serialize};
use time::{serde::iso8601::option as iso8601, OffsetDateTime};
use utoipa::ToSchema;

use crate::stores::Uuid;

use super::{
    storage::{Bucket, FileError},
    FileStorage,
};

#[derive(Clone, Debug, PolarClass, Serialize, Deserialize, ToSchema)]
pub struct Userfile {
    pub name: String,
    #[serde(with = "iso8601")]
    pub last_modified: Option<OffsetDateTime>,
    pub owner: Uuid,
    /// Size is only None if is_folder is true
    pub size: Option<i64>,
    pub is_folder: bool,
}

pub type SResult<T> = Result<T, FileError>;

#[async_trait::async_trait]
pub trait Filesystem: FileStorage {
    async fn list_files(&self, user_id: Uuid, base_path: &str) -> SResult<Vec<Userfile>>;
    async fn delete_file_at_path(&mut self, path: &str) -> Result<(), FileError> {
        self.delete_file(Bucket::UserFiles, path).await
    }
}
