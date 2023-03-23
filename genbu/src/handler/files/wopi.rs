use bytes::Bytes;
use tracing::error;
use wopi_rs::{
    file::{
        CheckFileInfoRequest, CheckFileInfoResponse, FileRequest, FileRequestType, LockRequest,
        LockResponse, PutRelativeFileRequest, PutRelativeFileResponse, PutRelativeFileResponseBody,
    },
    FileBody, WopiResponse,
};

use crate::stores::{
    files::{
        database::{DBFileError, DBFileStore, LeaseID},
        filesystem::Filesystem,
        storage::{Bucket, FileError},
        UploadLeaseStore,
    },
    users::User,
    DataStore, Uuid,
};

use super::userfiles::build_path;

pub async fn wopi_file(
    filesystem: impl Filesystem,
    file_db: impl DataStore,
    user: &User,
    file_req: FileRequest<Bytes>,
) -> http::Response<Bytes> {
    let Ok(id) = Uuid::parse_str(&file_req.file_id) else {
        return WopiResponse::<LockResponse>::NotFound.into();
    };
    let id = LeaseID(id);
    match file_req.request {
        FileRequestType::CheckFileInfo(r) => {
            handle_check_file_info(file_db, user, id, r).await.into()
        }
        FileRequestType::Lock(r) => handle_lock(file_db, user, id, r).await.into(),
        FileRequestType::PutRelativeFile(r) => {
            handle_put_relative(filesystem, file_db, user, id, r)
                .await
                .into()
        }
        _ => todo!(),
    }
}

type Response<T> = WopiResponse<T>;

#[tracing::instrument(skip(file_db))]
async fn handle_check_file_info(
    file_db: impl DataStore,
    user: &User,
    id: LeaseID,
    req: CheckFileInfoRequest,
) -> Response<CheckFileInfoResponse> {
    // For some reason this does't work.
    // TODO: Create reproducible example and ask on the axum repo
    // let (db_file, upload_lease) =
    //     futures::join!(file_db.get_dbfile(id.0), file_db.get_upload_lease(&id));
    let db_file = match file_db.get_dbfile(id.0).await {
        Ok(Some(f)) => f,
        Ok(None) => return WopiResponse::NotFound,
        Err(e) => {
            error!("error connecting to db: {:?}", e);
            return WopiResponse::InternalServerError;
        }
    };
    let upload_lease = match file_db.get_upload_lease(&id).await {
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
    // TODO: Add version
    let resp = CheckFileInfoResponse {
        base_file_name: name.to_owned(),
        owner_id: user.id.to_string(), // TODO: Update this if sharing is enabled
        user_id: user.id.to_string(),
        size: upload_lease.size,
        ..CheckFileInfoResponse::default()
    };
    WopiResponse::Ok(resp)
}

async fn handle_lock(
    file_db: impl DataStore,
    user: &User,
    id: LeaseID,
    req: LockRequest,
) -> Response<LockResponse> {
    match file_db.lock(id.0, req.lock.into()).await {
        Ok(Some(_)) => Response::Ok(LockResponse::Ok { item_version: None }),
        Ok(None) => Response::NotFound,
        Err(DBFileError::Locked(l)) => Response::Ok(LockResponse::Conflict {
            lock: l.unwrap_or_default().to_string(),
            lock_failure_reason: None,
        }),
        Err(e) => {
            error!("error while locking file id: {id:?}, error: {e:?}");
            Response::InternalServerError
        }
    }
}

async fn handle_put_relative(
    filesystem: impl Filesystem,
    file_db: impl DBFileStore + UploadLeaseStore,
    user: &User,
    lease_id: LeaseID,
    req: FileBody<Bytes, PutRelativeFileRequest>,
) -> Response<PutRelativeFileResponse> {
    let data = req.body;
    match req.request {
        PutRelativeFileRequest::Specific {
            relative_target,
            overwrite_relative_target,
            size,
            file_conversion,
        } => {
            handle_put_relative_specific(
                filesystem,
                file_db,
                lease_id,
                data,
                relative_target,
                overwrite_relative_target,
                size,
                file_conversion,
            )
            .await
        }
        PutRelativeFileRequest::Suggested {
            suggested_target,
            size,
            file_conversion,
        } => todo!(),
    }
}

// TODO: Binary file conversion?
async fn handle_put_relative_specific(
    filesystem: impl Filesystem,
    file_db: impl DBFileStore + UploadLeaseStore,
    lease_id: LeaseID,
    data: Bytes,
    relative_target: String,
    overwrite_relative_target: bool,
    size: u64,
    _file_conversion: bool,
) -> Response<PutRelativeFileResponse> {
    let dbfile = match file_db.get_dbfile(lease_id.0).await {
        Ok(Some(f)) => f,
        Ok(None) => return Response::NotFound,
        Err(DBFileError::Connection(e)) => {
            error!("db connection error {e:?}");
            return Response::InternalServerError;
        }
        Err(_) => return Response::InternalServerError, // TODO: Better Other error handling
    };

    let path = dbfile.parent_folder() + &relative_target;

    if !overwrite_relative_target {
        match file_db.get_dbfile_by_path(&path).await {
            // Happy path (file doesn't exist)
            Ok(None) => {}
            // File exists and is locked
            Ok(Some(f)) if f.is_locked() => {
                return Response::Ok(PutRelativeFileResponse::Locked {
                    lock: f.lock.unwrap().to_string(),
                })
            }
            // File exists and isn't locked
            Ok(Some(_)) => {
                return Response::Ok(PutRelativeFileResponse::FileAlreadyExists {
                    valid_relative_target: None,
                })
            }
            // DB Error
            Err(e) => {
                error!("dbfileerror while checking for existing file {e:?}");
                return Response::InternalServerError;
            }
        };
    }
    // TODO: Create UploadLease, DBFile
    match filesystem
        .upload(Bucket::UserFiles, &path, data.into())
        .await
    {
        Ok(_) => Response::Ok(PutRelativeFileResponse::Ok(PutRelativeFileResponseBody {
            name: relative_target,
            url: todo!(),
            host_view_url: todo!(),
            host_edit_url: todo!(),
        })),
        Err(e) => {
            error!("error while uploading to userfiles {e:?}");
            Response::InternalServerError
        }
    }
}
