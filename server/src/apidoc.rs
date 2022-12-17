use crate::routes::users::{self, NewUser, UserResponse};
use genbu_stores::users::{User, UserAvatar};
use utoipa::{
    openapi::security::{ApiKey, ApiKeyValue, SecurityScheme},
    Modify, OpenApi,
};

#[derive(OpenApi)]
#[openapi(
    paths(
        users::get_user,
        users::get_users,
        users::create_user,
        users::delete_user,
        users::register
    ),
    components(schemas(User, UserAvatar, NewUser, UserResponse)),
    modifiers(&SecurityAddon),
    tags(
        (name = "user", description = "User management API")
    ),
    security(
        ("token" = [])
    )
)]
pub(crate) struct ApiDoc;

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "token",
                SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new("__Host-Token"))),
            );
        }
    }
}
