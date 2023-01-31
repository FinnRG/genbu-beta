use time::{Duration, OffsetDateTime};

use crate::{
    connectors::postgres::PgStore,
    stores::{
        files::{
            database::{DBFile, DBFileError, FileLock, FileResult, SResult},
            database::{DBFileStore, LeaseID},
            storage::Bucket,
            UploadLease, UploadLeaseError, UploadLeaseStore,
        },
        Uuid,
    },
};

impl From<sqlx::Error> for UploadLeaseError {
    fn from(value: sqlx::Error) -> Self {
        match value {
            sqlx::Error::Io(_)
            | sqlx::Error::Tls(_)
            | sqlx::Error::Protocol(_)
            | sqlx::Error::PoolTimedOut
            | sqlx::Error::PoolClosed => Self::Connection(Box::new(value)),
            sqlx::Error::Database(e) => Self::Other(e.into()),
            _ => Self::Other(Box::new(value)),
        }
    }
}

#[async_trait::async_trait]
impl UploadLeaseStore for PgStore {
    #[tracing::instrument(skip(self), err(Debug))]
    async fn add(&mut self, lease: &UploadLease) -> SResult<UploadLease> {
        let res = sqlx::query_as!(
            UploadLease,
            r#"insert into upload_lease (id, owner, name, s3_upload_id, bucket, size, expires_at)
                values ($1, $2, $3, $4, $5, $6, $7)
                returning id as "id: LeaseID",owner,s3_upload_id,name,bucket as "bucket: Bucket",completed,size,created_at,expires_at"#,
            lease.id as _,
            lease.owner,
            lease.name,
            lease.s3_upload_id,
            lease.bucket as _,
            lease.size,
            lease.expires_at
        ).fetch_one(&self.conn).await?;
        Ok(res)
    }

    async fn delete(&mut self, id: &LeaseID) -> SResult<Option<UploadLease>> {
        let res = sqlx::query_as!(UploadLease,
        r#"delete from "upload_lease"
            where id = $1
            returning id as "id: LeaseID",owner,s3_upload_id,name,bucket as "bucket: Bucket",completed,size,created_at,expires_at"#,
            id as _
        ).fetch_optional(&self.conn).await?;
        Ok(res)
    }

    async fn get(&self, id: &LeaseID) -> SResult<Option<UploadLease>> {
        let res = sqlx::query_as!(
            UploadLease,
            r#"select id as "id: LeaseID",owner,s3_upload_id,name,bucket as "bucket: Bucket",completed,size,created_at,expires_at
                from "upload_lease"
                where id = $1"#,
            id as _
        )
            .fetch_optional(&self.conn)
            .await?;
        Ok(res)
    }

    async fn get_by_user(&self, id: &Uuid) -> SResult<Vec<UploadLease>> {
        let res = sqlx::query_as!(
            UploadLease,
            r#"select id as "id: LeaseID",owner,s3_upload_id,name,bucket as "bucket: Bucket",completed,size,created_at,expires_at
                from "upload_lease"
                where owner = $1"#,
            id
        )
            .fetch_all(&self.conn)
            .await?;
        Ok(res)
    }

    async fn mark_completed(&mut self, id: &LeaseID) -> SResult<Option<UploadLease>> {
        let lease = self.get(id).await?;
        let Some(lease) = lease else {
            return Ok(None);
        };
        if OffsetDateTime::now_utc() > lease.expires_at {
            return Err(UploadLeaseError::LeaseExpired(*id));
        }

        let res = sqlx::query_as!(
            UploadLease,
            r#"update "upload_lease"
                set completed = true
                where id = $1
                returning id as "id: LeaseID",owner,s3_upload_id,name,bucket as "bucket: Bucket",completed,size,created_at,expires_at
            "#,
            id as _
        ).fetch_optional(&self.conn).await?;
        Ok(res)
    }
}

impl From<sqlx::Error> for DBFileError {
    fn from(value: sqlx::Error) -> Self {
        match value {
            sqlx::Error::Io(_)
            | sqlx::Error::Tls(_)
            | sqlx::Error::Protocol(_)
            | sqlx::Error::PoolTimedOut
            | sqlx::Error::PoolClosed => Self::Connection(Box::new(value)),
            sqlx::Error::Database(e) => Self::Other(e.into()),
            _ => Self::Other(Box::new(value)),
        }
    }
}

#[async_trait::async_trait]
impl DBFileStore for PgStore {
    async fn add_dbfile(&mut self, file: &DBFile) -> FileResult<DBFile> {
        let res = sqlx::query_as!(
            DBFile,
            r#"
                insert into file (id, path, created_by)
                values ($1, $2, $3)
                returning id as "id: LeaseID",path,lock as "lock: FileLock",lock_expires_at,created_by,created_at
            "#,
            file.id as _,
            file.path,
            file.created_by
        )
        .fetch_one(&self.conn)
        .await?;
        Ok(res)
    }

    async fn unlock(&mut self, file_id: Uuid, lock: FileLock) -> FileResult<Option<()>> {
        let res = sqlx::query_scalar!(
            r#"
                update file
                set lock = null,lock_expires_at = null
                where id = $1
                returning id as "id: LeaseID"
            "#,
            file_id
        )
        .fetch_optional(&self.conn)
        .await?;
        Ok(res.map(|_| ()))
    }

    async fn extend_lock(&mut self, file_id: Uuid, lock: FileLock) -> FileResult<Option<()>> {
        let file = match self.get_dbfile(file_id).await? {
            Some(f) => f,
            None => return Ok(None),
        };
        if !file.lock.as_ref().is_some_and(|x| x == &lock) {
            return Err(DBFileError::Locked(file.lock));
        }
        let extended_timeout = file
            .lock_expires_at
            .map(|x| x + time::Duration::minutes(30));
        let res = sqlx::query_scalar!(
            r#"
                update file
                set lock_expires_at = $1
                where id = $2
                returning id as "id: LeaseID"
            "#,
            extended_timeout,
            file_id
        )
        .fetch_optional(&self.conn)
        .await?;
        Ok(res.map(|_| ()))
    }

    async fn lock(&mut self, file_id: Uuid, lock: FileLock) -> FileResult<Option<()>> {
        let conn = self.conn.begin().await?;
        let mut file = match self.get_dbfile(file_id).await? {
            Some(f) => f,
            None => return Ok(None),
        };
        if file.is_locked() {
            file.extend_lock(lock)
                .map_err(|l| DBFileError::Locked(Some(l.clone())))?;
            return Ok(Some(()));
        }

        sqlx::query_scalar!(
            r#"
                update file
                set lock = $1, lock_expires_at = $2
                where id = $3
                returning id as "id: LeaseID"
            "#,
            &lock as _,
            OffsetDateTime::now_utc() + Duration::minutes(30),
            file_id
        )
        .fetch_optional(&self.conn)
        .await?;

        conn.commit().await?;

        Ok(Some(()))
    }

    async fn get_dbfile(&self, file_id: Uuid) -> FileResult<Option<DBFile>> {
        let res = sqlx::query_as!(DBFile, r#"
                select id as "id: LeaseID",path,lock as "lock: FileLock",lock_expires_at,created_by,created_at
                from file
                where id = $1
            "#, file_id).fetch_optional(&self.conn).await?;
        Ok(res)
    }
}
