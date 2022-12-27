use std::fmt::Display;

use crate::stores::{
    files::{database::UploadLease, file_storage::Bucket},
    users::{User, UserAvatar},
    OffsetDateTime, Uuid,
};

#[derive(Clone, Debug, sqlx::Type)]
#[sqlx(transparent)]
pub struct StoreUserAvatar(Uuid);

impl Display for StoreUserAvatar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug)]
pub struct StoreUser {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub created_at: OffsetDateTime,
    pub avatar: Option<StoreUserAvatar>,
    pub hash: String,
}

impl From<StoreUserAvatar> for UserAvatar {
    fn from(val: StoreUserAvatar) -> Self {
        Self::new(val.0)
    }
}

impl From<&UserAvatar> for StoreUserAvatar {
    fn from(val: &UserAvatar) -> Self {
        Self(val.id())
    }
}

impl From<StoreUserAvatar> for Uuid {
    fn from(value: StoreUserAvatar) -> Self {
        value.0
    }
}

impl From<StoreUser> for User {
    fn from(val: StoreUser) -> Self {
        Self {
            id: val.id,
            name: val.name,
            email: val.email,
            hash: val.hash,
            created_at: val.created_at,
            avatar: val.avatar.map(Into::into),
        }
    }
}

#[derive(Debug, Clone)]
pub struct StoreUploadLease {
    id: Uuid,
    owner: Uuid,
    completed: bool,
    size: u64,
    created_at: OffsetDateTime,
    expires_at: OffsetDateTime,
    bucket: Bucket,
    name: String,
}

impl From<StoreUploadLease> for UploadLease {
    fn from(val: StoreUploadLease) -> Self {
        Self {
            id: val.id,
            owner: val.owner,
            completed: val.completed,
            size: val.size,
            created_at: val.created_at,
            expires_at: val.expires_at,
            bucket: val.bucket,
            name: val.name,
        }
    }
}
