use crate::{
    connectors::postgres::PgStore,
    stores::{
        files::{
            database::LeaseID, database::SResult, storage::Bucket, UploadLease, UploadLeaseError,
            UploadLeaseStore,
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
            | sqlx::Error::PoolClosed => UploadLeaseError::Connection(Box::new(value)),
            sqlx::Error::Database(e) => UploadLeaseError::Other(e.into()),
            _ => UploadLeaseError::Other(Box::new(value)),
        }
    }
}

#[async_trait::async_trait]
impl UploadLeaseStore for PgStore {
    async fn add(&mut self, lease: &UploadLease) -> SResult<UploadLease> {
        let res = sqlx::query_as!(
            UploadLease,
            r#"insert into upload_lease (id, owner, name, bucket, size, expires_at)
                values ($1, $2, $3, $4, $5, $6)
                returning id as "id: LeaseID",owner,name,bucket as "bucket: Bucket",completed,size,created_at,expires_at"#,
            lease.id as _,
            lease.owner,
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
            returning id as "id: LeaseID",owner,name,bucket as "bucket: Bucket",completed,size,created_at,expires_at"#,
            id as _
        ).fetch_optional(&self.conn).await?;
        Ok(res)
    }

    async fn get(&self, id: &LeaseID) -> SResult<Option<UploadLease>> {
        let res = sqlx::query_as!(
            UploadLease,
            r#"select id as "id: LeaseID",owner,name,bucket as "bucket: Bucket",completed,size,created_at,expires_at
                from "upload_lease" where id = $1"#,
            id as _
        )
            .fetch_optional(&self.conn)
            .await?;
        Ok(res)
    }

    async fn get_by_user(&self, id: &LeaseID) -> SResult<Vec<UploadLease>> {
        let res = sqlx::query_as!(
            UploadLease,
            r#"select id as "id: LeaseID",owner,name,bucket as "bucket: Bucket",completed,size,created_at,expires_at
                from "upload_lease" where owner = $1"#,
            id as _
        )
            .fetch_all(&self.conn)
            .await?;
        Ok(res)
    }
}
