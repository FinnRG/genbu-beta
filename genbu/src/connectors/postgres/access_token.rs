use std::net::IpAddr;

use crate::stores::{
    files::{
        access_token::{AccessToken, AccessTokenError, AccessTokenStore, TokenResult},
        database::DBFile,
    },
    users::User,
};

use super::PgStore;

impl From<sqlx::Error> for AccessTokenError {
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
impl AccessTokenStore for PgStore {
    #[tracing::instrument(skip(self))]
    async fn create_token(
        &self,
        user: &User,
        file: &DBFile,
        from: IpAddr,
    ) -> TokenResult<AccessToken> {
        let token = sqlx::query_scalar!(
            r#"
            insert into access_token (user_id, file_id, created_from)
            values ($1, $2, $3)
            returning token
        "#,
            user.id,
            file.id as _,
            from as _
        )
        .fetch_one(&self.conn)
        .await?;
        Ok(token.into())
    }

    #[tracing::instrument(skip(self))]
    async fn revoke_token(&self, token: &AccessToken) -> TokenResult<()> {
        Ok(sqlx::query!(
            r#"
            delete from access_token
            where token = $1
        "#,
            token as _
        )
        .execute(&self.conn)
        .await
        .map(|_| ())?)
    }
}
