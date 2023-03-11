use genbu_auth::authn;
use secrecy::SecretString;
use serde::Deserialize;
use std::fmt::Debug;
use utoipa::ToSchema;

use crate::{
    stores::{users::UserStore, Uuid},
    telemetry::spawn_blocking_with_tracing,
};

use super::APIError;

pub type AuthAPIError<T> = std::result::Result<T, APIError>;
type Result<T> = AuthAPIError<T>;

#[derive(Debug, Deserialize, ToSchema)]
pub struct LoginRequest {
    email: String,
    #[schema(value_type = String, format = Password)]
    password: SecretString,
}

pub async fn register_password<US: UserStore>(
    user_store: US,
    register_req: super::CreateUserRequest,
) -> Result<Uuid> {
    let user_id = super::add_user_to_store(user_store, register_req).await?;
    Ok(user_id)
}

pub async fn login_password<US: UserStore>(
    user_store: US,
    login_req: LoginRequest,
) -> Result<Uuid> {
    let db_user = user_store.get_by_email(&login_req.email).await?;
    spawn_blocking_with_tracing(move || {
        let hash = db_user.as_ref().map_or(
            "$argon2id$v=19$m=16,t=2,p=1$MVVDSUtUUThaQzh0RHRkNg$mD5KaV0QFxQzWhmVZ+5tsA",
            |u| &u.hash,
        );

        if authn::verify_password(&login_req.password, hash)? && let Some(u) = db_user {
            return Ok(u.id);
        }
        Err(APIError::WrongCredentials)
    })
    .await
    .map_err(|_| APIError::Unknown)?
}
