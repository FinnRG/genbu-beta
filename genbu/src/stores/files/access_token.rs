use std::{error::Error, net::IpAddr};

use thiserror::Error;

use crate::stores::{users::User, Uuid};

use super::database::DBFile;

#[derive(Debug, Error)]
pub enum AccessTokenError {
    #[error("unable to establish a file store connection")]
    Connection(#[source] Box<dyn Error>),

    #[error("unknown internal error")]
    Other(#[source] Box<dyn Error>),
}

pub type TokenResult<T> = std::result::Result<T, AccessTokenError>;
type Result<T> = TokenResult<T>;

#[derive(sqlx::Type, Debug, Clone, Hash, Eq, PartialEq)]
#[sqlx(transparent)]
pub struct AccessToken(Uuid);

impl From<Uuid> for AccessToken {
    fn from(value: Uuid) -> Self {
        AccessToken(value)
    }
}

#[async_trait::async_trait]
pub trait AccessTokenStore {
    async fn create_token(&self, user: &User, file: &DBFile, from: IpAddr) -> Result<AccessToken>;
    async fn revoke_token(&self, token: &AccessToken) -> Result<()>;
    // TODO: Consider future functions: get_tokens,get_tokens_for_user
}
