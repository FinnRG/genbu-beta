use std::{error::Error, fmt::Debug, str::FromStr};

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
        User {
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
    pub fn new(id: Uuid) -> Self {
        UserAvatar(id)
    }

    #[must_use]
    pub fn id(&self) -> Uuid {
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
            Ok(id) => Ok(UserAvatar(id)),
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

// Used to convert the internal user representations into the standard user.
fn deep_into<T: Into<U>, U, E: Into<F>, F>(res: Result<Option<T>, E>) -> Result<Option<U>, F> {
    match res {
        Ok(Some(u)) => Ok(Some(u.into())),
        Ok(None) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn deep_into_vec<T: Into<U>, U, E: Into<F>, F>(res: Result<Vec<T>, E>) -> Result<Vec<U>, F> {
    match res {
        Ok(mut v) => Ok(v.drain(..).map(Into::into).collect::<Vec<U>>()),
        Err(e) => Err(e.into()),
    }
}

/// Main data layer abstraction for users.
#[async_trait]
pub trait UserStore: Clone + Sized + Send + Sync + 'static {
    // TODO: Better error handling
    type StoreUser: Into<User>;
    type StoreError: Into<UserError>;

    async fn int_add(&mut self, user: &User) -> Result<(), Self::StoreError>;
    async fn add(&mut self, user: &User) -> Result<(), UserError> {
        self.int_add(user).await.map_err(Into::into)
    }

    async fn int_delete(&mut self, id: &Uuid) -> Result<Option<Self::StoreUser>, Self::StoreError>;
    async fn delete(&mut self, id: &Uuid) -> Result<Option<User>, UserError> {
        deep_into(self.int_delete(id).await)
    }

    async fn int_get(&self, id: &Uuid) -> Result<Option<Self::StoreUser>, Self::StoreError>;
    async fn get(&self, id: &Uuid) -> Result<Option<User>, UserError> {
        deep_into(self.int_get(id).await)
    }
    async fn int_get_by_email(
        &self,
        email: &str,
    ) -> Result<Option<Self::StoreUser>, Self::StoreError>;
    async fn get_by_email(&self, email: &str) -> Result<Option<User>, UserError> {
        deep_into(self.int_get_by_email(email).await)
    }

    async fn int_get_all(&self) -> Result<Vec<Self::StoreUser>, Self::StoreError>;
    async fn get_all(&self) -> Result<Vec<User>, UserError> {
        deep_into_vec(self.int_get_all().await)
    }

    async fn update(&mut self, user_update: UserUpdate) -> Result<Option<User>, UserError>;
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
