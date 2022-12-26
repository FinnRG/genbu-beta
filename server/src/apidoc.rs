use crate::routes::{
    files::{
        self,
        multipart_upload::FinishUploadRequest,
        upload::{UploadFileRequest, UploadFileResponse, UploadUnsignedRequest},
    },
    users::{self, LoginRequest, NewUser, UserResponse},
};
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
        users::register,
        users::login,
        files::get_presigned_url,
        files::upload::upload_file_request,
        files::upload::upload_unsigned,
        files::multipart_upload::finish_upload
    ),
    components(schemas(User, UserAvatar, NewUser, LoginRequest, UserResponse, UploadFileRequest, UploadFileResponse, UploadUnsignedRequest, FinishUploadRequest)),
    modifiers(&SecurityAddon),
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
