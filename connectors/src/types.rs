use std::fmt::Display;

use genbu_stores::{
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
        User {
            id: val.id,
            name: val.name,
            email: val.email,
            hash: val.hash,
            created_at: val.created_at,
            avatar: val.avatar.map(Into::into),
        }
    }
}
