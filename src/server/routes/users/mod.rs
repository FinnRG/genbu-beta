use std::fmt::Debug;

use axum::{
    extract::Path,
    http::HeaderValue,
    middleware,
    response::{AppendHeaders, IntoResponse},
    routing::{get, post},
    Extension, Json, Router,
};
use axum_extra::extract::cookie::{Cookie, SameSite};
use genbu_auth::authn::{self, verify_password, HashError};
use hyper::{header, StatusCode};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::error;
use utoipa::ToSchema;

use crate::{
    server::middlewares::auth::auth,
    stores::{
        users::{User, UserAvatar, UserError, UserUpdate},
        DataStore, Uuid,
    },
};

pub fn router<DS: DataStore>() -> Router {
    Router::new()
        .route(
            "/api/user/:id",
            get(get_user::<DS>)
                .delete(delete_user::<DS>)
                .patch(update_user::<DS>),
        )
        .route("/api/user/all", get(get_users::<DS>))
        .route("/api/user", post(create_user::<DS>))
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
) -> APIResult<Json<User>> {
    let user = user_store
        .get(&user_id)
        .await?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(user))
}

#[utoipa::path(
    get,
    path = "/api/user/all",
    responses(
        (status = 200, description = "List all users successfully", body = [User])
    )
)]
async fn get_users<DS: DataStore>(
    Extension(user_store): Extension<DS>,
) -> APIResult<impl IntoResponse> {
    let all_users = user_store.get_all().await?;
    Ok(Json(all_users))
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

async fn add_user_to_store<DS: DataStore>(mut store: DS, new_user: NewUser) -> APIResult<Uuid> {
    let hash = authn::hash_password(&new_user.password)?;

    let user = User {
        name: new_user.name,
        email: new_user.email,
        hash,
        avatar: new_user.avatar,
        ..User::template()
    };

    Ok(store.add(&user).await.map(|_| user.id)?)
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
) -> APIResult<(StatusCode, Json<UserResponse>)> {
    let new_user_res = add_user_to_store(user_store, new_user).await;
    let id = new_user_res?;
    Ok((StatusCode::CREATED, Json(UserResponse { id })))
}

/// Creates a response which creates a user-specific __Host-Token cookie. The token is secure, http
/// only and utilizes the strict SameSite policy.
///
/// # Errors
///
/// This function will return an error if a cryptographic error occurs during the creation of the
/// JWT.
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
) -> APIResult<impl IntoResponse> {
    let id = add_user_to_store(user_store, new_user).await?;
    Ok(start_session_response(id)?)
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
) -> APIResult<impl IntoResponse> {
    let db_user = user_store.get_by_email(&user.email).await?;

    let res = tokio::task::spawn_blocking(move || {
        // We still check this random hash to prevent timing attacks
        let user_exists = db_user.is_some();
        let hash = db_user.as_ref().map_or(
            "$argon2id$v=19$m=16,t=2,p=1$MVVDSUtUUThaQzh0RHRkNg$mD5KaV0QFxQzWhmVZ+5tsA",
            |u| &u.hash,
        );

        if verify_password(&user.password, hash)? && user_exists && let Some(u) = db_user {
            return start_session_response(u.id);
        }
        Err(StatusCode::UNAUTHORIZED)
    })
    .await
    .map_err(|e| {
        error!("error while spawning tokio task: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(res?)
}

// TODO: Better logging
#[utoipa::path(
    delete,
    path = "/api/user/{id}",
    responses(
        (status = 200, description = "User deleted successfully"),
        (status = 404, description = "No user found")
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

#[utoipa::path(
    patch,
    path = "/api/user/{id}",
    responses(
        (status = 200, description = "User updated successfully")
    ),
    params(
        ("id" = Uuid, Path, description = "User database id")
    )
)]
async fn update_user<DS: DataStore>(
    Extension(mut user_store): Extension<DS>,
    Path(user_id): Path<Uuid>,
    Json(req): Json<UserUpdate>,
) -> APIResult<impl IntoResponse> {
    // Empty user update
    if req == UserUpdate::default() {
        return get_user(Extension(user_store), Path(user_id)).await;
    }
    let updated_user = user_store
        .update(&user_id, req)
        .await?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(updated_user))
}

type APIResult<T> = Result<T, APIError>;
struct APIError {
    status: Option<StatusCode>,
    error: Option<UserError>,
}

impl IntoResponse for APIError {
    fn into_response(self) -> axum::response::Response {
        debug_assert!(self.error.is_some() ^ self.status.is_some());
        if let Some(status) = self.status {
            return status.into_response();
        }
        // You shouldn't depend on this behaviour
        if self.error.is_none() {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
        let (status, error_message) = match self.error.unwrap() {
            UserError::EmailAlreadyExists(_) => (StatusCode::CONFLICT, "E-Mail already exists"),
            UserError::IDAlreadyExists(_) => (StatusCode::CONFLICT, "ID already exists"),
            UserError::Connection(_) => (
                StatusCode::BAD_GATEWAY,
                "Server failed to establish connection to database",
            ),
            UserError::Other(_) | UserError::Infallible => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Unknown internal error")
            }
        };

        let body = Json(json!({ "error": error_message }));

        (status, body).into_response()
    }
}

impl From<UserError> for APIError {
    fn from(val: UserError) -> Self {
        APIError {
            status: None,
            error: Some(val),
        }
    }
}

impl From<StatusCode> for APIError {
    fn from(val: StatusCode) -> Self {
        APIError {
            status: Some(val),
            error: None,
        }
    }
}

impl From<HashError> for APIError {
    fn from(_: HashError) -> Self {
        APIError {
            status: Some(StatusCode::INTERNAL_SERVER_ERROR),
            error: None,
        }
    }
}

// TODO: Separate files for routes (especially login and register)
