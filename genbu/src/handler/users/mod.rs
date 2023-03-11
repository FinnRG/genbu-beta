use std::fmt::Debug;

use genbu_auth::authn::{self, HashError};
use secrecy::SecretString;
use serde::Deserialize;
use thiserror::Error;
use utoipa::ToSchema;

pub mod auth;

use crate::stores::{
    users::{User, UserError, UserStore, UserUpdate},
    Uuid,
};

pub type UserAPIResult<T> = std::result::Result<T, APIError>;

#[derive(Debug, Error)]
pub enum APIError {
    #[error("user store error")]
    StoreError(#[from] UserError),
    #[error("internal crypto error")]
    CryptoError,
    #[error("user not found")]
    NotFound(String),
    #[error("unknown api error")]
    Unknown,
    #[error("invalid credentials")]
    WrongCredentials,
}

type Result<T> = UserAPIResult<T>;

pub async fn get<US: UserStore>(user_store: US, user_id: Uuid) -> Result<User> {
    user_store
        .get(&user_id)
        .await?
        .ok_or(APIError::NotFound(user_id.to_string()))
}

pub async fn get_all<US: UserStore>(user_store: US) -> Result<Vec<User>> {
    Ok(user_store.get_all().await?)
}

pub async fn update<US: UserStore>(
    mut user_store: US,
    user_id: Uuid,
    update: UserUpdate,
) -> Result<User> {
    // Empty user update
    if update == UserUpdate::default() {
        return get(user_store, user_id).await;
    }
    user_store
        .update(&user_id, update)
        .await?
        .ok_or(APIError::NotFound(user_id.to_string()))
}

pub async fn delete<US: UserStore>(mut user_store: US, user_id: Uuid) -> Result<User> {
    user_store
        .delete(&user_id)
        .await?
        .ok_or(APIError::NotFound(user_id.to_string()))
}

#[derive(Clone, Deserialize, ToSchema)]
pub struct CreateUserRequest {
    name: String,
    email: String,
    #[schema(value_type = String, format = Password)]
    password: SecretString,
}

pub(crate) async fn add_user_to_store<US: UserStore>(
    mut user_store: US,
    create_req: CreateUserRequest,
) -> Result<Uuid> {
    let hash = authn::hash_password(&create_req.password)?;

    let user = User {
        name: create_req.name,
        email: create_req.email,
        hash,
        avatar: None,
        ..User::template()
    };

    user_store.add(&user).await?;
    Ok(user.id)
}

pub async fn create<US: UserStore>(user_store: US, create_req: CreateUserRequest) -> Result<Uuid> {
    add_user_to_store(user_store, create_req).await
}

impl From<HashError> for APIError {
    fn from(_: HashError) -> Self {
        Self::CryptoError
    }
}
