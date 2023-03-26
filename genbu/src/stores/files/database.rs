use std::{error::Error, fmt::Display};

use async_trait::async_trait;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::{Duration, OffsetDateTime};
use utoipa::ToSchema;

use crate::stores::Uuid;

use super::storage::Bucket;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[sqlx(transparent)]
#[serde(transparent)]
pub struct LeaseID(pub Uuid);

impl Display for LeaseID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Error)]
pub enum UploadLeaseError {
    #[error("unable to establish a file storage connection")]
    Connection(#[source] Box<dyn Error>),

    #[error("invalid / no size specified")]
    InvalidSize,

    #[error("lease {0:?} expired")]
    LeaseExpired(LeaseID),

    #[error("unknown internal error")]
    Other(#[source] Box<dyn Error>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadLease {
    pub id: LeaseID,
    pub s3_upload_id: String,
    pub owner: Uuid,
    pub completed: bool,
    pub size: i64,
    pub created_at: OffsetDateTime,
    pub expires_at: OffsetDateTime,
    pub bucket: Bucket,
    pub name: String,
}

impl UploadLease {
    #[must_use]
    pub fn template() -> Self {
        Self {
            id: LeaseID(Uuid::new_v4()),
            s3_upload_id: String::new(),
            owner: Uuid::new_v4(),
            completed: false,
            size: -1,
            created_at: OffsetDateTime::now_utc(),
            expires_at: OffsetDateTime::now_utc() + Duration::hours(6),
            bucket: Bucket::UserFiles,
            name: "template-file-name".to_owned(),
        }
    }
}

pub type SResult<T> = Result<T, UploadLeaseError>;

#[async_trait]
pub trait UploadLeaseStore {
    async fn add(&mut self, lease: &UploadLease) -> SResult<UploadLease>;

    async fn delete(&mut self, id: &LeaseID) -> SResult<Option<UploadLease>>;

    async fn get_upload_lease(&self, id: &LeaseID) -> SResult<Option<UploadLease>>;
    async fn get_by_user(&self, id: &Uuid) -> SResult<Vec<UploadLease>>;

    async fn mark_completed(&mut self, id: &LeaseID) -> SResult<Option<UploadLease>>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DBFile {
    pub id: LeaseID,
    pub size: i64,
    pub path: String,
    pub lock: Option<FileLock>,
    pub lock_expires_at: Option<OffsetDateTime>,
    pub created_by: Uuid,
    pub created_at: OffsetDateTime,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[sqlx(transparent)]
#[serde(transparent)]
pub struct FileLock(String);

impl FileLock {
    #[must_use]
    pub fn new() -> Self {
        FileLock(Uuid::new_v4().to_string())
    }
}

impl Display for FileLock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl Default for FileLock {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Into<String>> From<T> for FileLock {
    fn from(value: T) -> Self {
        Self(value.into())
    }
}

impl DBFile {
    pub fn new(path: impl Into<String>, user_id: Uuid, size: i64) -> Self {
        let now = OffsetDateTime::now_utc();
        DBFile {
            id: LeaseID(Uuid::new_v4()),
            path: path.into(),
            size,
            lock: None,
            lock_expires_at: None,
            created_by: user_id,
            created_at: now,
        }
    }

    pub fn parent_folder(&self) -> String {
        let parts = self.path.split_terminator('\\').rev().collect::<Vec<_>>();
        parts.iter().skip(1).rev().join("\\")
    }

    /// Returns the name of the file including the extension.
    pub fn name(&self) -> String {
        self.path
            .split('\\')
            .rev()
            .next()
            .unwrap_or_default()
            .to_owned()
    }

    #[must_use]
    pub fn is_locked(&self) -> bool {
        self.lock.is_some()
            && self
                .lock_expires_at
                .is_some_and(|x| x > OffsetDateTime::now_utc())
    }

    fn validate_lock(&self, lock: &FileLock) -> bool {
        if self.is_locked() {
            return self.lock.as_ref().unwrap() == lock;
        }
        true
    }

    fn unchecked_unlock(&mut self) {
        self.lock = None;
        self.lock_expires_at = None;
    }

    fn unchecked_lock(&mut self, lock: FileLock) {
        self.lock = Some(lock);
        self.lock_expires_at = Some(OffsetDateTime::now_utc() + Duration::minutes(30));
    }

    fn unchecked_extend_lock(&mut self) {
        self.lock_expires_at = Some(OffsetDateTime::now_utc() + Duration::minutes(30));
    }

    pub fn lock(&mut self, lock: FileLock) -> Result<(), &FileLock> {
        if self.validate_lock(&lock) {
            self.unchecked_lock(lock);
            return Ok(());
        }
        Err(self.lock.as_ref().unwrap())
    }

    pub fn unlock(&mut self, lock: &FileLock) -> Result<(), &FileLock> {
        if self.validate_lock(lock) {
            self.unchecked_unlock();
            return Ok(());
        }
        Err(self.lock.as_ref().unwrap())
    }

    pub fn extend_lock(&mut self, lock: &FileLock) -> Result<(), &FileLock> {
        if self.validate_lock(lock) {
            self.unchecked_extend_lock();
            return Ok(());
        }
        Err(self.lock.as_ref().unwrap())
    }
}

#[derive(Debug, Error)]
pub enum DBFileError {
    #[error("unable to establish a file store connection")]
    Connection(#[source] Box<dyn Error>),

    #[error("file is locked")]
    Locked(Option<FileLock>),

    #[error("unknown internal error")]
    Other(#[source] Box<dyn Error>),
}

pub type FileResult<T> = Result<T, DBFileError>;

#[async_trait::async_trait]
pub trait DBFileStore {
    async fn get_dbfile(&self, file_id: Uuid) -> FileResult<Option<DBFile>>;
    async fn get_dbfile_by_path(&self, path: &str) -> FileResult<Option<DBFile>>;
    async fn validate_lock(&self, file_id: Uuid, lock: FileLock) -> FileResult<Option<bool>> {
        let Some(file) = self.get_dbfile(file_id).await? else {
            return Ok(None);
        };
        Ok(Some(file.lock.is_some_and(|x| x == lock)))
    }
    async fn add_dbfile(&self, file: &DBFile) -> FileResult<DBFile>;
    async fn lock(&self, file_id: Uuid, lock: FileLock) -> FileResult<Option<()>>;
    async fn unlock(&self, file_id: Uuid, lock: FileLock) -> FileResult<Option<()>>;
    async fn extend_lock(&self, file_id: Uuid, lock: FileLock) -> FileResult<Option<()>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_dbfile() -> DBFile {
        DBFile::new("\\test", Uuid::nil(), 0)
    }

    #[test]
    fn parent_folder() {
        let dbf = DBFile::new("folder\\test.txt", Uuid::nil(), 0);
        assert_eq!(dbf.parent_folder(), "folder");
    }

    #[test]
    fn parent_folder_of_folder() {
        let dbf = DBFile::new("folder1\\folder2\\", Uuid::nil(), 0);
        assert_eq!(dbf.parent_folder(), "folder1");
    }

    #[test]
    fn file_name() {
        let db = create_dbfile();
        assert_eq!(db.name(), "test");
    }

    #[test]
    fn default_unlocked() {
        let dbf = create_dbfile();
        assert!(!dbf.is_locked());
    }

    #[test]
    fn lock_unlock() {
        let mut dbf = create_dbfile();
        let lock: FileLock = "Test".into();
        assert!(dbf.lock(lock.clone()).is_ok());
        assert!(dbf.is_locked());
        assert!(dbf.unlock(&lock).is_ok());
        assert!(!dbf.is_locked());
    }

    #[test]
    fn return_correct_lock() {
        let mut dbf = create_dbfile();
        let valid_lock: FileLock = "Test".into();
        let invalid_lock: FileLock = "test".into();
        dbf.lock(valid_lock.clone()).expect("unable to lock dbf");
        assert_eq!(dbf.unlock(&invalid_lock), Err(&valid_lock));
    }
}
