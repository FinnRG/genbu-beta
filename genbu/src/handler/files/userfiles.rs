use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::stores::{
    files::filesystem::{Filesystem, FilesystemError, Userfile},
    Uuid,
};
use std::{fmt::Debug, ops::Deref};

#[derive(Debug, thiserror::Error)]
pub enum UserfilesAPIError {
    #[error("filesystem error")]
    Filesystem(#[from] FilesystemError),

    #[error("file {0:?} not found")]
    NotFound(Box<dyn Debug + Send + Sync>),
}

pub type UserfilesAPIResult<T> = std::result::Result<T, UserfilesAPIError>;
type Result<T> = UserfilesAPIResult<T>;

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, IntoParams)]
pub struct GetUserfilesRequest {
    pub base_path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct GetUserfilesResponse {
    pub files: Vec<Userfile>,
}

#[tracing::instrument(skip_all)]
pub async fn get_userfiles(
    filesystem: impl Filesystem,
    user_id: Uuid,
    get_req: &GetUserfilesRequest,
) -> Result<GetUserfilesResponse> {
    let path = build_path(user_id, &get_req.base_path);
    let mut files = filesystem.list(user_id, &path).await?;
    files
        .iter_mut()
        .for_each(|f| f.name = f.name.split_off(build_path(user_id, "").len()));
    Ok(GetUserfilesResponse { files })
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, IntoParams)]
pub struct DeleteUserfileRequest {
    path: String,
}

pub async fn delete_userfile(
    mut filesystem: impl Filesystem,
    user_id: Uuid,
    delete_req: DeleteUserfileRequest,
) -> Result<()> {
    let path = build_path(user_id, &delete_req.path);
    filesystem.delete(&path).await?;
    Ok(())
}

pub fn build_path(user_id: Uuid, path: &str) -> String {
    format!("{}\\{}", user_id, path.deref())
}
