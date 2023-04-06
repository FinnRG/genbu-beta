use bytes::Bytes;
use http::StatusCode;
use tracing::{debug, error, trace};
use wopi_rs::{
    content::{FileContentRequest, FileContentRequestType, PutFileRequest, PutFileResponse},
    file::{
        CheckFileInfoRequest, CheckFileInfoResponse, FileRequest, FileRequestType, GetLockResponse,
        LockRequest, LockResponse, PutRelativeFileRequest, PutRelativeFileResponse,
        PutRelativeFileResponseBody, UnlockAndRelockResponse, UnlockResponse,
    },
    FileBody, WopiResponse,
};

use crate::{
    server::routes::AppState,
    stores::{
        files::{
            access_token::AccessTokenStore,
            database::{DBFile, DBFileError, DBFileStore, FileLock, LeaseID, PartialDBFile},
            storage::Bucket,
            FileStorage,
        },
        DataStore, Uuid,
    },
};

pub async fn wopi_file(
    state: impl AppState,
    user_id: Uuid,
    file_req: FileRequest<Bytes>,
) -> http::Response<Bytes> {
    let Ok(id) = Uuid::parse_str(&file_req.file_id) else {
        return WopiResponse::<LockResponse>::NotFound.into();
    };
    let id = LeaseID(id);
    match file_req.request {
        FileRequestType::CheckFileInfo(r) => handle_check_file_info(state.store(), user_id, id, r)
            .await
            .into(),
        FileRequestType::Lock(r) => handle_lock(state.store(), user_id, id, r).await.into(),
        FileRequestType::GetLock(_) => handle_get_lock(state, id.0).await.into(),
        FileRequestType::PutRelativeFile(r) => {
            handle_put_relative(state, user_id, id, r).await.into()
        }
        FileRequestType::Unlock(r) => handle_unlock(state, id.0, r.lock.into()).await.into(),
        FileRequestType::UnlockAndRelock(r) => {
            handle_unlock_and_relock(state, id.0, r.old_lock.into(), r.lock.into())
                .await
                .into()
        }
    }
}

pub async fn wopi_file_content(
    state: impl AppState,
    user_id: Uuid,
    req: FileContentRequest<Bytes>,
) -> http::Response<Bytes> {
    let Ok(file_id) = Uuid::parse_str(&req.file_id) else {
        return WopiResponse::<LockResponse>::NotFound.into();
    };
    match req.request {
        FileContentRequestType::GetFile(_) => handle_get_file(state, file_id).await,
        FileContentRequestType::PutFile(FileBody { body, request }) => {
            handle_put_file(state, file_id, request, body).await.into()
        }
    }
}

type Response<T> = WopiResponse<T>;

#[tracing::instrument(skip(file_db))]
async fn handle_check_file_info(
    file_db: impl DataStore,
    user_id: Uuid,
    id: LeaseID,
    req: CheckFileInfoRequest,
) -> Response<CheckFileInfoResponse> {
    let db_file = match file_db.get_dbfile(id.0).await {
        Ok(Some(f)) => f,
        Ok(None) => return WopiResponse::NotFound,
        Err(e) => {
            error!("error connecting to db: {:?}", e);
            return WopiResponse::InternalServerError;
        }
    };
    // TODO: Add version
    let resp = CheckFileInfoResponse {
        base_file_name: db_file.name(),
        owner_id: user_id.to_string(), // TODO: Update this if sharing is enabled
        user_id: user_id.to_string(),
        size: db_file.size,
        read_only: Some(false),
        user_can_write: Some(true),
        supports_locks: Some(true),
        supports_get_lock: Some(true),
        supports_extended_lock_length: Some(true),
        supports_update: Some(true), // TODO: Check group permissions in the future
        user_can_not_write_relative: Some(false),
        ..CheckFileInfoResponse::default()
    };
    WopiResponse::Ok(resp)
}

async fn handle_lock(
    file_db: impl DataStore,
    user_id: Uuid,
    id: LeaseID,
    req: LockRequest,
) -> Response<LockResponse> {
    trace!("lock attempt with {}", req.lock);
    match file_db.lock(id.0, req.lock.into()).await {
        Ok(Some(_)) => Response::Ok(LockResponse::Ok { item_version: None }),
        Ok(None) => Response::NotFound,
        Err(DBFileError::Locked(l)) => Response::Ok(LockResponse::Conflict {
            lock: l.to_string(),
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
    user_id: Uuid,
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
                user_id,
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
                user_id,
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
    user_id: Uuid,
    dbfile: DBFile,
    data: Bytes,
    relative_target: String,
    overwrite_relative_target: bool,
    size: i64,
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
    let new_file = DBFile::new(&path, user_id, size);
    match file_db.add_dbfile(&new_file).await {
        Ok(_) => {}
        Err(e) => {
            // At this stage the file should never be locked
            debug_assert!(!matches!(e, DBFileError::Locked(_)));
            return Response::InternalServerError;
        }
    }

    let access_token = match state
        .store()
        .create_token(user_id, new_file.id.0, "127.0.0.1".parse().unwrap())
        .await
    {
        Ok(t) => t,
        Err(e) => {
            error!("{e:?}");
            return Response::InternalServerError;
        }
    };

    match state
        .file()
        .upload(Bucket::UserFiles, &path, data.into())
        .await
    {
        Ok(_) => Response::Ok(PutRelativeFileResponse::Ok(PutRelativeFileResponseBody {
            name: relative_target,
            url: format!(
                "{}/api/wopi/files/{}?access_token={}",
                state.host(),
                new_file.id,
                access_token
            ),
            host_view_url: None,
            host_edit_url: None,
        })),
        Err(e) => {
            error!("error while uploading to userfiles {e:?}");
            Response::InternalServerError
        }
    }
}

async fn handle_put_relative_file_suggested(
    state: impl AppState,
    user_id: Uuid,
    dbfile: DBFile,
    data: Bytes,
    suggested_target: String,
    size: i64,
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

    let new_file = DBFile::new(&path, user_id, size);

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

    let access_token = match state
        .store()
        .create_token(user_id, new_file.id.0, "127.0.0.1".parse().unwrap())
        .await
    {
        Ok(t) => t,
        Err(e) => {
            error!("{e:?}");
            return Response::InternalServerError;
        }
    };

    Response::Ok(PutRelativeFileResponse::Ok(PutRelativeFileResponseBody {
        name: suggestion,
        url: format!(
            "{}/api/wopi/files/{}?access_token={}",
            state.host(),
            new_file.id,
            access_token
        ),
        host_view_url: None,
        host_edit_url: None,
    }))
}

fn new_response(code: StatusCode) -> http::Response<Bytes> {
    http::Response::builder()
        .status(code)
        .body(Bytes::new())
        .unwrap()
}

async fn handle_get_file(state: impl AppState, file_id: Uuid) -> http::Response<Bytes> {
    let dbfile = match state.store().get_dbfile(file_id).await {
        Ok(Some(f)) => f,
        Ok(None) => {
            debug!("no dbfile with id {file_id} found");
            return new_response(StatusCode::NOT_FOUND);
        }
        Err(e) => {
            error!("error {e:?} while retrieving dbfile with file_id {file_id}");
            return new_response(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let data = match state.file().download(Bucket::UserFiles, &dbfile.path).await {
        Ok(d) => d,
        Err(e) => {
            error!("{e:?}");
            return new_response(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    http::Response::builder().body(Bytes::from(data)).unwrap()
}

async fn handle_put_file(
    state: impl AppState,
    file_id: Uuid,
    PutFileRequest { lock, editors }: PutFileRequest,
    body: Bytes,
) -> Response<PutFileResponse> {
    let dbfile = match state.store().get_dbfile(file_id).await {
        Ok(Some(f)) => f,
        Ok(None) => return Response::NotFound,
        Err(e) => {
            error!("error {e:?} while retrieving dbfile {file_id} from store");
            return Response::InternalServerError;
        }
    };

    // Check for active locks or unlocked files with more than 0 bytes
    if let Some(lock) = lock {
        if !dbfile.validate_lock(&lock.into()) {
            return Response::Ok(PutFileResponse::Conflict {
                lock: dbfile.lock.unwrap().take(),
                lock_failure_reason: None,
            });
        }
    } else if dbfile.size > 0 {
        error!("PutFile conflict: No lock specified and file size > 0");
        // THe WOPi spec is unclear here.
        // You have to respond with X-WOPI-Lock if you use the StatusCode 409,
        // but the file isn't necessarily locked if it is larger than 0 bytes.
        return Response::Ok(PutFileResponse::Conflict {
            lock: String::new(),
            lock_failure_reason: None,
        });
    };

    // TODO: This implicitly imposes an upper limit on the max file size at i64::MAX
    let Ok(new_size) = body.len().try_into() else {
        return Response::Ok(PutFileResponse::TooLarge);
    };

    // Update file size in database if necessary
    // TODO: Do this in parallen with uploading
    if dbfile.size != new_size {
        match state
            .store()
            .update_dbfile(
                file_id,
                &PartialDBFile {
                    size: Some(new_size),
                    ..Default::default()
                },
            )
            .await
        {
            Ok(_) => {}
            Err(e) => {
                error!("{e:?}");
                return Response::InternalServerError;
            }
        }
    }

    match state
        .file()
        .upload(Bucket::UserFiles, &dbfile.path, body.to_vec())
        .await
    {
        Ok(_) => {
            // TODO: Add the item version here
            Response::Ok(PutFileResponse::Ok { item_version: None })
        }
        Err(e) => {
            error!("error {e:?} while uploading file {file_id}");
            Response::InternalServerError
        }
    }
}

async fn handle_get_lock(state: impl AppState, file_id: Uuid) -> Response<GetLockResponse> {
    let dbfile = match state.store().get_dbfile(file_id).await {
        Ok(Some(f)) => f,
        Ok(None) => return Response::NotFound,
        Err(e) => {
            error!("error {e:?} while retrieving dbfile {file_id}");
            return Response::InternalServerError;
        }
    };
    let lock = if dbfile.is_locked() {
        dbfile.lock.unwrap().take()
    } else {
        String::new()
    };
    Response::Ok(GetLockResponse::Ok { lock })
}

async fn handle_unlock(
    state: impl AppState,
    file_id: Uuid,
    lock: FileLock,
) -> Response<UnlockResponse> {
    match state.store().unlock(file_id, lock).await {
        Ok(Some(_)) => Response::Ok(UnlockResponse::Ok { item_version: None }),
        Ok(None) => Response::NotFound,
        Err(DBFileError::Locked(lock)) => Response::Ok(UnlockResponse::Conflict {
            lock: lock.to_string(),
            lock_failure_reason: None,
        }),
        Err(e) => {
            error!("error {e:?} while unlocking {file_id}");
            Response::InternalServerError
        }
    }
}

async fn handle_unlock_and_relock(
    state: impl AppState,
    file_id: Uuid,
    old_lock: FileLock,
    new_lock: FileLock,
) -> Response<UnlockAndRelockResponse> {
    match state
        .store()
        .unlock_and_relock(file_id, old_lock, new_lock)
        .await
    {
        Ok(Some(_)) => Response::Ok(UnlockAndRelockResponse::Ok),
        Ok(None) => Response::NotFound,
        Err(DBFileError::Locked(lock)) => Response::Ok(UnlockAndRelockResponse::Conflict {
            lock: lock.to_string(),
            lock_failure_reason: None,
        }),
        Err(e) => {
            error!("{e:?}");
            Response::InternalServerError
        }
    }
}
