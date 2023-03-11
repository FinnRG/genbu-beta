use bytes::Bytes;
use tracing::error;
use wopi_rs::{
    file::{
        CheckFileInfoRequest, CheckFileInfoResponse, FileRequest, FileRequestType, LockRequest,
        LockResponse, PutRelativeFileRequest, PutRelativeFileResponse,
    },
    FileBody, WopiResponse,
};

use crate::stores::{
    files::{database::DBFileError, filesystem::Filesystem},
    users::User,
    DataStore, Uuid,
};

pub async fn wopi_file(
    filesystem: impl Filesystem,
    file_db: impl DataStore,
    user: &User,
    file_req: FileRequest<Bytes>,
) -> http::Response<Bytes> {
    let Ok(id) = Uuid::parse_str(&file_req.file_id) else {
        return WopiResponse::<LockResponse>::NotFound.into();
    };
    match file_req.request {
        FileRequestType::CheckFileInfo(r) => {
            handle_check_file_info(file_db, user, id, r).await.into()
        }
        FileRequestType::Lock(r) => handle_lock(file_db, user, id, r).await.into(),
        FileRequestType::PutRelativeFile(r) => {
            handle_put_relative(filesystem, user, r).await.into()
        }
        _ => todo!(),
    }
}

type Response<T> = WopiResponse<T>;

#[tracing::instrument(skip(file_db))]
async fn handle_check_file_info(
    file_db: impl DataStore,
    user: &User,
    id: Uuid,
    req: CheckFileInfoRequest,
) -> Response<CheckFileInfoResponse> {
    let db_file = match file_db.get_dbfile(id).await {
        Ok(Some(f)) => f,
        Ok(None) => return WopiResponse::NotFound,
        Err(e) => {
            error!("error connecting to db: {:?}", e);
            return WopiResponse::InternalServerError;
        }
    };
    let name = match db_file.path.split('/').last() {
        None | Some("") => return WopiResponse::NotFound,
        Some(a) => {
            if let Some(n) = a.rfind('.') {
                a.split_at(n).1
            } else {
                a
            }
        }
    };
    // TODO: Add version and size
    let resp = CheckFileInfoResponse {
        base_file_name: name.to_owned(),
        owner_id: user.id.to_string(), // TODO: Update this if sharing is enabled
        user_id: user.id.to_string(),
        ..CheckFileInfoResponse::default()
    };
    WopiResponse::Ok(resp)
}

async fn handle_lock(
    mut file_db: impl DataStore,
    user: &User,
    id: Uuid,
    req: LockRequest,
) -> Response<LockResponse> {
    match file_db.lock(id, req.lock.into()).await {
        Ok(Some(f)) => Response::Ok(LockResponse::Ok { item_version: None }),
        Ok(None) => Response::NotFound,
        Err(DBFileError::Locked(l)) => Response::Ok(LockResponse::Conflict {
            lock: l.unwrap_or_default().to_string(),
            lock_failure_reason: None,
        }),
        Err(e) => {
            error!("error while locking file id: {}, error: {:?}", id, e);
            Response::InternalServerError
        }
    }
}

async fn handle_put_relative(
    mut filesystem: impl Filesystem,
    user: &User,
    req: FileBody<Bytes, PutRelativeFileRequest>,
) -> Response<PutRelativeFileResponse> {
    todo!()
}
