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
    users::{User, UserAvatar, UserError, UserStore},
    Uuid,
};
use hyper::{header, StatusCode};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::middlewares::auth::auth;

pub fn router<US: UserStore>() -> Router {
    Router::new()
        .route("/api/user/:id", get(get_user::<US>))
        .route("/api/user/all", get(get_users::<US>))
        .route("/api/user", post(create_user::<US>))
        .route("/api/user/:id", delete(delete_user::<US>))
        .route_layer(middleware::from_fn(auth))
        .route("/api/register", post(register::<US>))
        .route("/api/login", post(login::<US>))
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
async fn get_user<US: UserStore>(
    Extension(user_store): Extension<US>,
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
async fn get_users<US: UserStore>(Extension(user_store): Extension<US>) -> impl IntoResponse {
    match user_store.get_all().await {
        Ok(users) => Ok(Json(users)),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

#[derive(Clone, Deserialize, ToSchema)]
pub(crate) struct NewUser {
    name: String,
    email: String,
    avatar: Option<UserAvatar>,
    #[schema(value_type = String, format = Password)]
    password: SecretString,
}

#[derive(Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct UserResponse {
    id: Uuid,
}

async fn add_user_to_store<US: UserStore>(
    mut store: US,
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
async fn create_user<US: UserStore>(
    Extension(user_store): Extension<US>,
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
        (status = 201, description = "User registered successfully", body = UserResponse,
            headers(
                ("Set-Cookie" = String, description = "Sets the JWT Cookie")
        )),
        (status = 409, description = "User data already exists in the database")
    )
)]
async fn register<US: UserStore>(
    Extension(user_store): Extension<US>,
    Json(new_user): Json<NewUser>,
) -> Result<impl IntoResponse, StatusCode> {
    let id = add_user_to_store(user_store, new_user).await?;
    start_session_response(id)
}

#[derive(Debug, Deserialize)]
struct LoginRequest {
    email: String,
    password: SecretString,
}

// TODO: Better logging
async fn login<US: UserStore>(
    Extension(user_store): Extension<US>,
    Json(user): Json<LoginRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    let db_user = user_store
        .get_by_email(&user.email)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)? // DB error
        .ok_or(StatusCode::UNAUTHORIZED)?; // No user with email in DB

    if verify_password(&user.password, &db_user.hash)? {
        return start_session_response(db_user.id);
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
async fn delete_user<US: UserStore>(
    Extension(mut user_store): Extension<US>,
    Path(user_id): Path<Uuid>,
) -> impl IntoResponse {
    match user_store.delete(&user_id).await {
        Ok(Some(_)) => StatusCode::OK,
        Ok(None) => StatusCode::NOT_FOUND,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
