use axum::{
    extract::Path,
    http::HeaderValue,
    middleware,
    response::{AppendHeaders, IntoResponse},
    routing::{get, post},
    Extension, Json, Router,
};
use axum_extra::extract::cookie::{Cookie, SameSite};
use genbu_auth::authn;
use hyper::{header, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;

use crate::{
    handler,
    server::middlewares::auth::auth,
    stores::{
        users::{UserError, UserUpdate},
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
) -> handler::users::UserAPIResult<impl IntoResponse> {
    let user = handler::users::get(user_store, user_id).await;
    Ok(Json(user?))
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
) -> handler::users::UserAPIResult<impl IntoResponse> {
    Ok(Json(handler::users::get_all(user_store).await?))
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
async fn create_user<DS: DataStore>(
    Extension(user_store): Extension<DS>,
    Json(new_user): Json<handler::users::CreateUserRequest>,
) -> handler::users::UserAPIResult<impl IntoResponse> {
    let user_id = handler::users::create(user_store, new_user).await?;
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
    request_body = CreateUserRequest,
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
    Json(new_user): Json<handler::users::CreateUserRequest>,
) -> handler::users::UserAPIResult<impl IntoResponse> {
    let id = handler::users::auth::register_password(user_store, new_user).await?;
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
async fn login<DS: DataStore>(
    Extension(user_store): Extension<DS>,
    Json(login_req): Json<handler::users::auth::LoginRequest>,
) -> handler::users::UserAPIResult<impl IntoResponse> {
    let user_id = handler::users::auth::login_password(user_store, login_req).await?;
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
async fn delete_user<DS: DataStore>(
    Extension(user_store): Extension<DS>,
    Path(user_id): Path<Uuid>,
) -> handler::users::UserAPIResult<impl IntoResponse> {
    Ok(Json(handler::users::delete(user_store, user_id).await?))
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
    Extension(user_store): Extension<DS>,
    Path(user_id): Path<Uuid>,
    Json(req): Json<UserUpdate>,
) -> handler::users::UserAPIResult<impl IntoResponse> {
    Ok(Json(
        handler::users::update(user_store, user_id, req).await?,
    ))
}

impl IntoResponse for handler::users::APIError {
    fn into_response(self) -> axum::response::Response {
        // You shouldn't depend on this behaviour
        match self {
            Self::StoreError(e) => {
                let (status, error_message) = match e {
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

                let body = Json(json!({ "error": error_message }));

                (status, body).into_response()
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
// TODO: Separate files for routes (especially login and register)
