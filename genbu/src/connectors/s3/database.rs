use crate::{
    connectors::postgres::PgStore,
    stores::{
        files::{
            database::LeaseID, database::SResult, storage::Bucket, UploadLease, UploadLeaseError,
            UploadLeaseStore,
        },
        OffsetDateTime, Uuid,
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
    async fn add(&mut self, lease: &UploadLease) -> SResult<UploadLease> {
        let res = sqlx::query_as!(
            UploadLease,
            r#"insert into upload_lease (id, owner, name, s3_upload_id, bucket, size, expires_at)
                values ($1, $2, $3, $4, $5, $6, $7)
                returning id as "id: LeaseID",owner,s3_upload_id,name,bucket as "bucket: Bucket",completed,size,created_at,expires_at"#,
            lease.id as _,
            lease.owner,
            lease.s3_upload_id,
            lease.name,
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
                from "upload_lease" where id = $1"#,
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
                from "upload_lease" where owner = $1"#,
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
        if OffsetDateTime::now_utc() < lease.expires_at {
            return Err(UploadLeaseError::LeaseExpired(*id));
        }

        let res = sqlx::query_as!(
            UploadLease,
            r#"update "upload_lease"
                set completed = true
                where id = $1 and expires_at < now()
                returning id as "id: LeaseID",owner,s3_upload_id,name,bucket as "bucket: Bucket",completed,size,created_at,expires_at
            "#,
            id as _
        ).fetch_optional(&self.conn).await?;
        Ok(res)
    }
}
