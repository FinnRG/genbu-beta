use std::{error::Error, fmt::Debug, ops::Deref, str::FromStr};

use time::{serde::iso8601, OffsetDateTime};
use uuid::{Error as UuidError, Uuid};

#[derive(Clone, Debug, oso::PolarClass, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct User {
    #[polar(attribute)]
    pub id: Uuid,
    pub name: String,
    pub email: String,
    #[serde(skip)]
    pub hash: String,
    #[serde(with = "iso8601")]
    pub created_at: OffsetDateTime,
    pub avatar: Option<UserAvatar>,
}

impl User {
    #[must_use]
    pub fn template() -> Self {
        Self {
            id: Uuid::new_v4(),
            name: String::new(),
            email: String::new(),
            hash: String::new(),
            created_at: OffsetDateTime::now_utc(),
            avatar: None,
        }
    }
}

#[derive(
    Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize, utoipa::ToSchema, sqlx::Type,
)]
#[sqlx(transparent)]
pub struct UserAvatar(Uuid);

impl UserAvatar {
    #[must_use]
    pub const fn new(id: Uuid) -> Self {
        Self(id)
    }

    #[must_use]
    pub const fn id(&self) -> Uuid {
        self.0
    }
}

impl Deref for UserAvatar {
    type Target = Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum UserAvatarError {
    #[error("uuid error")]
    UuidError(UuidError),
}

impl FromStr for UserAvatar {
    type Err = UserAvatarError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match Uuid::from_str(s) {
            Ok(id) => Ok(Self(id)),
            Err(e) => Err(UserAvatarError::UuidError(e)),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum UserError {
    #[error("a user with the email `{0}` already exists in the store")]
    EmailAlreadyExists(String),

    #[error("a user with the id `{0:?}` already exists in the store")]
    IDAlreadyExists(Option<Uuid>),

    #[error("unable to establish a database connection")]
    Connection(#[source] Box<dyn Error + Send + Sync>),

    #[error("unknown data store error")]
    Other(#[source] Box<dyn Error + Send + Sync>),

    #[error("this error shouldn't appear")]
    Infallible,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct UserUpdate {
    pub name: Option<String>,
    pub email: Option<String>,
    pub avatar: Option<UserAvatar>,
}

pub type SResult<T> = Result<T, UserError>;

/// Main data layer abstraction for users.
#[async_trait::async_trait]
pub trait UserStore {
    // TODO: Better error handling
    async fn add(&mut self, user: &User) -> SResult<()>;

    // TODO: Test that the delete endpoint really returns the user if it previously existed
    async fn delete(&mut self, id: &Uuid) -> SResult<Option<User>>;

    async fn get(&self, id: &Uuid) -> SResult<Option<User>>;
    async fn get_by_email(&self, email: &str) -> SResult<Option<User>>;

    async fn get_all(&self) -> SResult<Vec<User>>;

    async fn update(&mut self, id: &Uuid, update: UserUpdate) -> SResult<Option<User>>;
}

// TODO: Remove this test
#[cfg(test)]
mod tests {
    use oso::{Oso, PolarClass};

    use crate::stores::Uuid;

    use super::{User, UserAvatar};

    #[test]
    fn test_oso() -> Result<(), Box<dyn std::error::Error>> {
        let mut oso = Oso::new();

        dbg!(oso.load_files(vec!["src/stores/test.polar"]))?;
        oso.register_class(Uuid::get_polar_class())?;
        oso.register_class(User::get_polar_class())?;

        dbg!(oso.is_allowed(
            User {
                name: String::from("TestUser"),
                email: String::from("test@email.com"),
                avatar: Some(UserAvatar::new(
                    Uuid::parse_str("f8af16d5-a014-4441-a8aa-a91a95ced6dc").unwrap()
                )),
                ..User::template()
            },
            "",
            ""
        ))?;
        Ok(())
    }
}
