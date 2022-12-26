use std::{error::Error, fmt::Debug, ops::Deref, str::FromStr};

use crate::util::{deep_into, deep_into_vec};
use async_trait::async_trait;
use oso::PolarClass;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::{serde::iso8601, OffsetDateTime};
use uuid::{Error as UuidError, Uuid};

#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Clone, Debug, PolarClass, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
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

#[derive(Clone, Debug, Eq, PartialEq, Error)]
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

#[derive(Debug, Error)]
pub enum UserError {
    #[error("a user with the email `{0}` already exists in the store")]
    EmailAlreadyExists(String),

    #[error("a user with the id `{0:?}` already exists in the store")]
    IDAlreadyExists(Option<Uuid>),

    #[error("unable to establish a database connection")]
    Connection(#[source] Box<dyn Error>),

    #[error("unknown data store error")]
    Other(#[source] Box<dyn Error>),

    #[error("this error shouldn't appear")]
    Infallible,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserUpdate {
    pub id: Uuid,
    pub name: Option<String>,
    pub avatar: Option<UserAvatar>,
}

pub type SResult<T> = Result<T, UserError>;

/// Main data layer abstraction for users.
#[async_trait]
pub trait UserStore {
    // TODO: Better error handling
    type StoreUser: Into<User>;

    async fn int_add(&mut self, user: &User) -> SResult<()>;
    async fn add(&mut self, user: &User) -> SResult<()> {
        self.int_add(user).await.map_err(Into::into)
    }

    // TODO: Test that the delete endpoint really returns the user if it previously existed
    async fn int_delete(&mut self, id: &Uuid) -> SResult<Option<Self::StoreUser>>;
    async fn delete(&mut self, id: &Uuid) -> SResult<Option<User>> {
        deep_into(self.int_delete(id).await)
    }

    async fn int_get(&self, id: &Uuid) -> SResult<Option<Self::StoreUser>>;
    async fn get(&self, id: &Uuid) -> SResult<Option<User>> {
        deep_into(self.int_get(id).await)
    }
    async fn int_get_by_email(&self, email: &str) -> SResult<Option<Self::StoreUser>>;
    async fn get_by_email(&self, email: &str) -> SResult<Option<User>> {
        deep_into(self.int_get_by_email(email).await)
    }

    async fn int_get_all(&self) -> SResult<Vec<Self::StoreUser>>;
    async fn get_all(&self) -> SResult<Vec<User>> {
        deep_into_vec(self.int_get_all().await)
    }

    async fn update(&mut self, user_update: UserUpdate) -> SResult<Option<User>>;
}

#[cfg(test)]
mod tests {
    use oso::{Oso, PolarClass};

    use crate::{
        users::{User, UserAvatar},
        Uuid,
    };

    #[test]
    fn test_oso() -> Result<(), Box<dyn std::error::Error>> {
        let mut oso = Oso::new();

        dbg!(oso.load_files(vec!["src/test.polar"]))?;
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
