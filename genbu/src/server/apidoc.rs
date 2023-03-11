use crate::handler::files::download::StartDownloadRequest;
use crate::handler::files::upload::{
    FinishUploadRequest, GetUrisRequest, UploadFileRequest, UploadFileResponse,
};
use crate::handler::files::userfiles::{
    DeleteUserfileRequest, GetUserfilesRequest, GetUserfilesResponse,
};
use crate::handler::users::{auth::LoginRequest, CreateUserRequest};
use crate::server::routes::{
    files::{self, userfiles},
    users::{self, UserResponse},
};
use crate::stores::files::database::LeaseID;
use crate::stores::files::filesystem::Userfile;
use crate::stores::files::storage::{Bucket, Part};
use crate::stores::users::{User, UserAvatar};
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
        files::upload_file_request,
        files::finish_upload,
        files::start_download,
        userfiles::get_userfiles,
        userfiles::delete_userfile
    ),
    components(
        schemas(
            User,
            UserAvatar,
            CreateUserRequest,
            LoginRequest,
            UserResponse,
            UploadFileRequest,
            UploadFileResponse,
            FinishUploadRequest,
            StartDownloadRequest,
            GetUrisRequest,
            Part,
            LeaseID,
            GetUserfilesRequest,
            DeleteUserfileRequest,
            GetUserfilesResponse,
            Userfile,
            Bucket
        )
    ),
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
                SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new("Token"))),
            );
        }
    }
}
