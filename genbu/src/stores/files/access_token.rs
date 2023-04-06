use std::{error::Error, fmt::Display, net::IpAddr};

use thiserror::Error;

use crate::stores::Uuid;

#[derive(Debug, Error)]
pub enum AccessTokenError {
    #[error("unable to establish a file store connection")]
    Connection(#[source] Box<dyn Error>),

    #[error("unknown internal error")]
    Other(#[source] Box<dyn Error>),
}

pub type TokenResult<T> = std::result::Result<T, AccessTokenError>;
type Result<T> = TokenResult<T>;

#[derive(sqlx::Type, Debug, Copy, Clone, Hash, Eq, PartialEq)]
#[sqlx(transparent)]
pub struct AccessToken(Uuid);

impl From<Uuid> for AccessToken {
    fn from(value: Uuid) -> Self {
        AccessToken(value)
    }
}

impl Display for AccessToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct AccessTokenContext {
    pub token: AccessToken,
    pub file_id: Uuid,
    pub user_id: Uuid,
}

#[async_trait::async_trait]
pub trait AccessTokenStore {
    async fn create_token(&self, user_id: Uuid, file_id: Uuid, from: IpAddr)
        -> Result<AccessToken>;
    async fn get_token_context(&self, token: AccessToken) -> Result<Option<AccessTokenContext>>;
    async fn revoke_token(&self, token: AccessToken) -> Result<()>;
    // TODO: Consider future functions: get_tokens,get_tokens_for_user
}
