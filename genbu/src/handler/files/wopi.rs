use bytes::Bytes;
use tracing::{debug, error, trace};
use wopi_rs::{
    file::{
        CheckFileInfoRequest, CheckFileInfoResponse, FileRequest, FileRequestType, LockRequest,
        LockResponse, PutRelativeFileRequest, PutRelativeFileResponse, PutRelativeFileResponseBody,
    },
    FileBody, WopiResponse,
};

use crate::{
    server::routes::AppState,
    stores::{
        files::{
            database::{DBFile, DBFileError, DBFileStore, LeaseID},
            filesystem::Filesystem,
            storage::Bucket,
            FileStorage, UploadLeaseStore,
        },
        users::User,
        DataStore, Uuid,
    },
};

pub async fn wopi_file(
    state: impl AppState,
    user: &User,
    file_req: FileRequest<Bytes>,
) -> http::Response<Bytes> {
    let Ok(id) = Uuid::parse_str(&file_req.file_id) else {
        return WopiResponse::<LockResponse>::NotFound.into();
    };
    let id = LeaseID(id);
    match file_req.request {
        FileRequestType::CheckFileInfo(r) => handle_check_file_info(state.store(), user, id, r)
            .await
            .into(),
        FileRequestType::Lock(r) => handle_lock(state.store(), user, id, r).await.into(),
        FileRequestType::PutRelativeFile(r) => handle_put_relative(state, user, id, r).await.into(),
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
    state: impl AppState,
    user: &User,
    lease_id: LeaseID,
    req: FileBody<Bytes, PutRelativeFileRequest>,
) -> Response<PutRelativeFileResponse> {
    let data = req.body;
    // Get the base file, from which we will place the relative file
    let dbfile = match state.store().get_dbfile(lease_id.0).await {
        Ok(Some(f)) => f,
        Ok(None) => return Response::NotFound,
        Err(DBFileError::Connection(e)) => {
            error!("db connection error {e:?}");
            return Response::InternalServerError;
        }
        Err(_) => return Response::InternalServerError, // TODO: Better Other error handling
    };
    match req.request {
        PutRelativeFileRequest::Specific {
            relative_target,
            overwrite_relative_target,
            size,
            file_conversion,
        } => {
            debug!("Processing PutRelativeFile in Specific mode");
            handle_put_relative_specific(
                state,
                user,
                dbfile,
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
        } => {
            debug!("Processing PutRelativeFile in Suggested mode");
            handle_put_relative_file_suggested(
                state,
                user,
                dbfile,
                data,
                suggested_target,
                size,
                file_conversion,
            )
            .await
        }
    }
}

// TODO: Binary file conversion?
async fn handle_put_relative_specific(
    state: impl AppState,
    user: &User,
    dbfile: DBFile,
    data: Bytes,
    relative_target: String,
    overwrite_relative_target: bool,
    _size: u64,
    _file_conversion: bool,
) -> Response<PutRelativeFileResponse> {
    let file_db = state.store();

    let path = dbfile.parent_folder() + &relative_target;
    trace!("constructed path {path:?}");

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

    // Add a new DBFile to the database
    let dbfile = DBFile::with_path_and_user(&path, user);
    match file_db.add_dbfile(&dbfile).await {
        Ok(_) => {}
        Err(e) => {
            // At this stage the file should never be locked
            debug_assert!(!matches!(e, DBFileError::Locked(_)));
            return Response::InternalServerError;
        }
    }

    match state
        .file()
        .upload(Bucket::UserFiles, &path, data.into())
        .await
    {
        Ok(_) => Response::Ok(PutRelativeFileResponse::Ok(PutRelativeFileResponseBody {
            name: relative_target,
            url: format!("{}/api/wopi/files/{}", state.host(), dbfile.id),
            host_view_url: todo!(),
            host_edit_url: todo!(),
        })),
        Err(e) => {
            error!("error while uploading to userfiles {e:?}");
            Response::InternalServerError
        }
    }
}

async fn handle_put_relative_file_suggested(
    state: impl AppState,
    user: &User,
    dbfile: DBFile,
    data: Bytes,
    suggested_target: String,
    size: u64,
    _file_conversion: bool,
) -> Response<PutRelativeFileResponse> {
    let file_db = state.store();

    // Parse suggested_target as extension or full file name
    let mut suggestion = suggested_target.clone();
    if suggested_target.starts_with('.') {
        suggestion = dbfile.name() + &suggestion;
    }

    let mut counter = 1;
    let mut path;

    // Try so long until you don't find a dbfile with the specified path
    loop {
        path = dbfile.parent_folder() + "\\" + &suggestion;
        match file_db.get_dbfile_by_path(&path).await {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => {
                error!("error {e:?} while searching for path {suggestion}");
                return Response::InternalServerError;
            }
        };
        counter += 1;
        suggestion = counter.to_string() + &suggestion;
    }

    let new_file = DBFile::with_path_and_user(&path, user);

    match file_db.add_dbfile(&new_file).await {
        Ok(_) => {}
        Err(e) => {
            error!("error {e:?} while adding dbfile {new_file:?}");
            return Response::InternalServerError;
        }
    }

    match state
        .file()
        .upload(Bucket::UserFiles, &path, data.into())
        .await
    {
        Ok(_) => {}
        Err(e) => {
            error!("error {e:?} while uploading new file to filesystem");
            // TODO: Try to remove dbfile again
            return Response::InternalServerError;
        }
    }

    Response::Ok(PutRelativeFileResponse::Ok(PutRelativeFileResponseBody {
        name: suggestion,
        url: todo!(),
        host_view_url: todo!(),
        host_edit_url: todo!(),
    }))
}
