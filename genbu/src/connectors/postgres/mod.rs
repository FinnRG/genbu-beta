use std::ops::Deref;

use async_trait::async_trait;
use sqlx::{migrate::MigrateDatabase, postgres::PgPoolOptions, PgPool};
use tracing::instrument;

use crate::stores::{
    users::{SResult, User, UserAvatar, UserError, UserStore, UserUpdate},
    DataStore, Reset, Setup, Uuid,
};

pub mod access_token;
pub mod file;

#[derive(Clone, Debug)]
pub struct PgStore {
    pub(crate) conn: PgPool,
    conn_str: String,
}

impl From<sqlx::Error> for UserError {
    fn from(value: sqlx::Error) -> Self {
        match &value {
            sqlx::Error::Io(_)
            | sqlx::Error::Tls(_)
            | sqlx::Error::Protocol(_)
            | sqlx::Error::PoolTimedOut
            | sqlx::Error::PoolClosed => Self::Connection(Box::new(value)),
            sqlx::Error::Database(_) => {
                let e = value.as_database_error();
                if let Some(db_err) = e {
                    match db_err.constraint() {
                        Some("user_email_key") => Self::EmailAlreadyExists(String::new()),
                        Some("user_pkey") => Self::IDAlreadyExists(None),
                        _ => Self::Other(Box::new(value)),
                    }
                } else {
                    Self::Other(Box::new(value))
                }
            }
            _ => Self::Other(Box::new(value)),
        }
    }
}

#[async_trait]
impl UserStore for PgStore {
    #[instrument]
    async fn add(&mut self, user: &User) -> SResult<()> {
        let res = sqlx::query_as!(User, r#"INSERT INTO "user" (id, name, email, created_at, hash, avatar) VALUES ($1, $2, $3, $4, $5, $6)"#,
            user.id,
            user.name,
            user.email,
            user.created_at,
            user.hash,
            user.avatar as _
        ).execute(&self.conn)
            .await
            .map(|_| ())?;
        Ok(res)
    }

    #[instrument]
    async fn delete(&mut self, id: &Uuid) -> SResult<Option<User>> {
        let res = sqlx::query_as!(
            User,
            r#"DELETE FROM "user" WHERE id = $1 RETURNING id,name,email,created_at,hash,avatar as "avatar: UserAvatar""#,
            id
        )
            .fetch_optional(&self.conn)
            .await?;
        Ok(res)
    }

    #[instrument]
    async fn get(&self, id: &Uuid) -> SResult<Option<User>> {
        let res = sqlx::query_as!(
            User,
            r#"SELECT id,name,email,created_at,hash,avatar as "avatar: UserAvatar" FROM "user" WHERE id = $1"#,
            id
        )
            .fetch_optional(&self.conn)
            .await?;
        Ok(res)
    }

    #[instrument]
    async fn get_all(&self) -> SResult<Vec<User>> {
        let res = sqlx::query_as!(
            User,
            r#"SELECT id,name,email,created_at,hash,avatar as "avatar: UserAvatar" FROM "user""#
        )
        .fetch_all(&self.conn)
        .await?;
        Ok(res)
    }

    #[instrument]
    async fn get_by_email(&self, email: &str) -> SResult<Option<User>> {
        let res = sqlx::query_as!(
            User,
            r#"SELECT id,name,email,hash,created_at,avatar as "avatar: UserAvatar" FROM "user" WHERE email = $1"#,
            email
        )
            .fetch_optional(&self.conn).await?;
        Ok(res)
    }

    #[instrument]
    async fn update(&mut self, id: &Uuid, update: UserUpdate) -> SResult<Option<User>> {
        let res = sqlx::query_as!(
            User,
            r#"
                UPDATE "user"
                SET email = coalesce($1, "user".email),
                    avatar = coalesce($2, "user".avatar),
                    name = coalesce($3, "user".name)
                WHERE id = $4
                RETURNING id,name,email,hash,created_at,avatar as "avatar: UserAvatar"
            "#,
            update.email,
            update.avatar.as_ref().map(Deref::deref),
            update.name,
            id
        )
        .fetch_optional(&self.conn)
        .await?;
        Ok(res)
    }
}

#[async_trait]
impl DataStore for PgStore {
    async fn new(conn: String) -> Result<Self, Box<dyn std::error::Error>> {
        let pool = PgPoolOptions::new().connect_lazy(&conn)?;
        Ok(Self {
            conn: pool,
            conn_str: conn,
        })
    }
}

#[async_trait]
impl Reset for PgStore {
    #[cfg(debug_assertions)]
    async fn reset(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        match sqlx::postgres::Postgres::drop_database(&self.conn_str).await {
            Err(e) => {
                tracing::error!("{:?}", e);
                Err(Box::new(e))
            }
            _ => Ok(()),
        }
    }
}

#[async_trait]
impl Setup for PgStore {
    async fn setup(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.conn.close().await;
        sqlx::Postgres::create_database(&self.conn_str).await?;
        self.conn = PgPool::connect(&self.conn_str).await?;
        sqlx::migrate!().run(&self.conn).await?;
        Ok(())
    }
}
