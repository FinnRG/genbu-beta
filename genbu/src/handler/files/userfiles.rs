use crate::stores::{
    files::filesystem::{Filesystem, FilesystemError, Userfile},
    users::User,
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

#[derive(
    Clone, Debug, serde::Serialize, serde::Deserialize, utoipa::ToSchema, utoipa::IntoParams,
)]
pub struct GetUserfilesRequest {
    base_path: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct GetUserfilesResponse {
    files: Vec<Userfile>,
}

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

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct DeleteUserfileRequest {
    path: String,
}

pub async fn delete_userfile(
    mut filesystem: impl Filesystem,
    user: &User,
    delete_req: DeleteUserfileRequest,
) -> Result<()> {
    let path = build_path(user.id, &delete_req.path);
    filesystem.delete(&path).await?;
    Ok(())
}

fn build_path(user_id: Uuid, path: &str) -> String {
    format!("{}/{}", user_id, path.deref().trim_end_matches('/'))
}
