use std::fmt::Debug;

use axum::{
    extract::Path,
    http::HeaderValue,
    middleware,
    response::{AppendHeaders, IntoResponse},
    routing::{delete, get, post},
    Extension, Json, Router,
};
use axum_extra::extract::cookie::{Cookie, SameSite};
use genbu_auth::authn::{self, verify_password};
use genbu_stores::{
    stores::DataStore,
    users::{User, UserAvatar, UserError},
    Uuid,
};
use hyper::{header, StatusCode};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::middlewares::auth::auth;

pub fn router<DS: DataStore>() -> Router {
    Router::new()
        .route("/api/user/:id", get(get_user::<DS>))
        .route("/api/user/all", get(get_users::<DS>))
        .route("/api/user", post(create_user::<DS>))
        .route("/api/user/:id", delete(delete_user::<DS>))
        .route_layer(middleware::from_fn(auth))
        .route("/api/register", post(register::<DS>))
        .route("/api/login", post(login::<DS>))
}

#[utoipa::path(
    get,
    path = "/api/user/{id}",
    responses(
        (status = 200, description = "User found successfully", body = User)
    ),
    params(
        ("id" = Uuid, Path, description = "User database id")
    )
)]
async fn get_user<DS: DataStore>(
    Extension(user_store): Extension<DS>,
    Path(user_id): Path<Uuid>,
) -> impl IntoResponse {
    match user_store.get(&user_id).await {
        Ok(Some(user)) => Ok(Json(user)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR), // TODO: Differentiate between internal server
                                                          // error and id not found
    }
}

#[utoipa::path(
    get,
    path = "/api/user/all",
    responses(
        (status = 200, description = "List all users successfully", body = [User])
    )
)]
async fn get_users<DS: DataStore>(Extension(user_store): Extension<DS>) -> impl IntoResponse {
    match user_store.get_all().await {
        Ok(users) => Ok(Json(users)),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

#[derive(Clone, Deserialize, ToSchema)]
pub struct NewUser {
    name: String,
    email: String,
    avatar: Option<UserAvatar>,
    #[schema(value_type = String, format = Password)]
    password: SecretString,
}

#[derive(Clone, Serialize, Deserialize, ToSchema)]
pub struct UserResponse {
    id: Uuid,
}

async fn add_user_to_store<DS: DataStore>(
    mut store: DS,
    new_user: NewUser,
) -> Result<Uuid, StatusCode> {
    let hash =
        authn::hash_password(&new_user.password).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let user = User {
        name: new_user.name,
        email: new_user.email,
        hash,
        avatar: new_user.avatar,
        ..User::template()
    };

    store
        .add(&user)
        .await
        .map(|_| user.id)
        .map_err(|e| match e {
            UserError::EmailAlreadyExists(_) | UserError::IDAlreadyExists(_) => {
                StatusCode::CONFLICT
            }
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        })
}

// TODO: Better logging
// TODO: Return user instead of just the id? (Then change back to 200 instead of 201)
// TODO: Add a test for this endpoint
#[utoipa::path(
    post,
    path = "/api/user",
    request_body = NewUser,
    responses(
        (status = 201, description = "User created successfully", body = UserResponse),
        (status = 409, description = "User data already exists in the database")
    )
)]
async fn create_user<DS: DataStore>(
    Extension(user_store): Extension<DS>,
    Json(new_user): Json<NewUser>,
) -> Result<(StatusCode, Json<UserResponse>), StatusCode> {
    let new_user_res = add_user_to_store(user_store, new_user).await;
    let id = new_user_res?;
    Ok((StatusCode::CREATED, Json(UserResponse { id })))
}

fn start_session_response(id: Uuid) -> Result<impl IntoResponse, StatusCode> {
    let token = authn::create_jwt(id)?;

    let cookie = Cookie::build("__Host-Token", token)
        .secure(true)
        .http_only(true)
        .same_site(SameSite::Strict)
        .finish();
    let set_cookie_header = HeaderValue::from_str(&cookie.to_string())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(AppendHeaders([(header::SET_COOKIE, set_cookie_header)]))
}

// TODO: Better logging
#[utoipa::path(
    post,
    path = "/api/register",
    request_body = NewUser,
    responses(
        (status = 200, description = "User registered successfully", body = UserResponse,
            headers(
                ("Set-Cookie" = String, description = "Sets the JWT Cookie")
        )),
        (status = 409, description = "User data already exists in the database")
    )
)]
async fn register<DS: DataStore>(
    Extension(user_store): Extension<DS>,
    Json(new_user): Json<NewUser>,
) -> Result<impl IntoResponse, StatusCode> {
    let id = add_user_to_store(user_store, new_user).await?;
    start_session_response(id)
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct LoginRequest {
    email: String,
    password: SecretString,
}

// TODO: Better logging
#[utoipa::path(
    post,
    path = "/api/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "User registered successfully", body = UserResponse,
            headers(
                ("Set-Cookie" = String, description = "Sets the JWT Cookie")
        )),
        (status = 401, description = "Wrong credentials")
    )
)]
async fn login<DS: DataStore>(
    Extension(user_store): Extension<DS>,
    Json(user): Json<LoginRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    let db_user = user_store
        .get_by_email(&user.email)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?; // DB error
                                                          // We still have to verify the password if there isn't a user with the specified E-Mail
                                                          // address to prevent timing attacks
    let hash = match db_user.as_ref() {
        None => "$argon2id$v=19$m=16,t=2,p=1$MVVDSUtUUThaQzh0RHRkNg$mD5KaV0QFxQzWhmVZ+5tsA",
        Some(u) => &u.hash,
    };

    if verify_password(&user.password, &hash)? {
        return match db_user {
            Some(u) => start_session_response(u.id),
            None => Err(StatusCode::UNAUTHORIZED),
        };
    }
    Err(StatusCode::UNAUTHORIZED)
}

// TODO: Better logging
#[utoipa::path(
    delete,
    path = "/api/user/{id}",
    responses(
        (status = 200, description = "User deleted successfully")
    ),
    params(
        ("id" = Uuid, Path, description = "User database id")
    )
)]
async fn delete_user<DS: DataStore>(
    Extension(mut user_store): Extension<DS>,
    Path(user_id): Path<Uuid>,
) -> impl IntoResponse {
    match user_store.delete(&user_id).await {
        Ok(Some(_)) => StatusCode::OK,
        Ok(None) => StatusCode::NOT_FOUND,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

// TODO: Patch user
// TODO: Separate files for routes (especially login and register)
