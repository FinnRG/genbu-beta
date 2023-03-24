use axum::{
    extract::{Path, State},
    http::HeaderValue,
    middleware,
    response::{AppendHeaders, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use axum_extra::extract::cookie::{Cookie, SameSite};
use genbu_auth::authn;
use hyper::{header, StatusCode};
use serde::{Deserialize, Serialize};
use time::Duration;
use utoipa::ToSchema;

use crate::{
    handler,
    server::middlewares::auth::auth,
    stores::{
        users::{UserError, UserUpdate},
        OffsetDateTime, Uuid,
    },
};

use super::AppState;

pub fn router<S: AppState>() -> Router<S> {
    Router::new()
        .route(
            "/api/user/:id",
            get(get_user::<S>)
                .delete(delete_user::<S>)
                .patch(update_user::<S>),
        )
        .route("/api/user/all", get(get_users::<S>))
        .route("/api/user", post(create_user::<S>))
        .route_layer(middleware::from_fn(auth))
        .route("/api/register", post(register::<S>))
        .route("/api/login", post(login::<S>))
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
async fn get_user<S: AppState>(
    State(state): State<S>,
    Path(user_id): Path<Uuid>,
) -> handler::users::UserAPIResult<impl IntoResponse> {
    let user = handler::users::get(state.store(), user_id).await;
    Ok(Json(user?))
}

#[utoipa::path(
    get,
    path = "/api/user/all",
    responses(
        (status = 200, description = "List all users successfully", body = [User])
    )
)]
async fn get_users<S: AppState>(
    State(state): State<S>,
) -> handler::users::UserAPIResult<impl IntoResponse> {
    Ok(Json(handler::users::get_all(state.store()).await?))
}

#[derive(Clone, Serialize, Deserialize, ToSchema)]
pub struct UserResponse {
    id: Uuid,
}

// TODO: Better logging
// TODO: Return user instead of just the id? (Then change back to 200 instead of 201)
// TODO: Add a test for this endpoint
#[utoipa::path(
    post,
    path = "/api/user",
    request_body = CreateUserRequest,
    responses(
        (status = 201, description = "User created successfully", body = UserResponse),
        (status = 409, description = "User data already exists in the database")
    )
)]
async fn create_user<S: AppState>(
    State(state): State<S>,
    Json(new_user): Json<handler::users::CreateUserRequest>,
) -> handler::users::UserAPIResult<impl IntoResponse> {
    let user_id = handler::users::create(state.store(), new_user).await?;
    Ok(Json(UserResponse { id: user_id }))
}

/// Creates a response which creates a user-specific __Host-Token cookie. The token is secure, http
/// only and utilizes the strict `SameSite` policy.
///
/// # Errors
///
/// This function will return an error if a cryptographic error occurs during the creation of the
/// JWT.
fn start_session_response(id: Uuid) -> Result<impl IntoResponse, StatusCode> {
    let token = authn::create_jwt(id)?;

    let mut cookie = Cookie::build("Token", token)
        .expires(OffsetDateTime::now_utc() + Duration::days(1)) // TODO: Rethink if 1 day is a good expiration time
        .http_only(true)
        .same_site(SameSite::Strict)
        .finish();

    if !cfg!(debug_assertions) {
        cookie.set_secure(Some(true));
    }

    let set_cookie_header = HeaderValue::from_str(&cookie.to_string())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok((
        AppendHeaders([(header::SET_COOKIE, set_cookie_header)]),
        Json(UserResponse { id }),
    ))
}

// TODO: Better logging
#[utoipa::path(
    post,
    path = "/api/register",
    request_body = CreateUserRequest,
    responses(
        (status = 200, description = "User registered successfully", body = UserResponse,
            headers(
                ("Set-Cookie" = String, description = "Sets the JWT Cookie")
        )),
        (status = 409, description = "User data already exists in the database")
    )
)]
async fn register<S: AppState>(
    State(state): State<S>,
    Json(new_user): Json<handler::users::CreateUserRequest>,
) -> handler::users::UserAPIResult<impl IntoResponse> {
    let id = handler::users::auth::register_password(state.store(), new_user).await?;
    Ok(start_session_response(id))
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
async fn login<S: AppState>(
    State(state): State<S>,
    Json(login_req): Json<handler::users::auth::LoginRequest>,
) -> handler::users::UserAPIResult<impl IntoResponse> {
    let user_id = handler::users::auth::login_password(state.store(), login_req).await?;
    Ok(start_session_response(user_id))
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
async fn delete_user<S: AppState>(
    State(state): State<S>,
    Path(user_id): Path<Uuid>,
) -> handler::users::UserAPIResult<impl IntoResponse> {
    Ok(Json(handler::users::delete(state.store(), user_id).await?))
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
async fn update_user<S: AppState>(
    State(state): State<S>,
    Path(user_id): Path<Uuid>,
    Json(req): Json<UserUpdate>,
) -> handler::users::UserAPIResult<impl IntoResponse> {
    Ok(Json(
        handler::users::update(state.store(), user_id, req).await?,
    ))
}

impl IntoResponse for handler::users::APIError {
    fn into_response(self) -> axum::response::Response {
        // You shouldn't depend on this behaviour
        match self {
            Self::StoreError(e) => {
                let resp = match e {
                    UserError::EmailAlreadyExists(_) => {
                        (StatusCode::CONFLICT, "E-Mail already exists")
                    }
                    UserError::IDAlreadyExists(_) => (StatusCode::CONFLICT, "ID already exists"),
                    UserError::Connection(_) => (
                        StatusCode::BAD_GATEWAY,
                        "Server failed to establish connection to database",
                    ),
                    UserError::Other(_) | UserError::Infallible => {
                        (StatusCode::INTERNAL_SERVER_ERROR, "Unknown internal error")
                    }
                };

                resp.into_response()
            }
            Self::WrongCredentials => {
                (StatusCode::UNAUTHORIZED, "wrong credentials").into_response()
            }
            Self::Unknown => (StatusCode::INTERNAL_SERVER_ERROR, "unknown error").into_response(),
            Self::CryptoError => {
                (StatusCode::INTERNAL_SERVER_ERROR, "internal crypto error").into_response()
            }
            Self::NotFound(_) => (StatusCode::NOT_FOUND, "").into_response(),
        }
    }
}
