use async_trait::async_trait;
use genbu_stores::{
    stores::{DataStore, Reset, Setup},
    users::{SResult, User, UserError, UserStore, UserUpdate},
    Uuid,
};
use sqlx::{migrate::MigrateDatabase, postgres::PgPoolOptions, PgPool};
use tracing::instrument;

use crate::types::{StoreUser, StoreUserAvatar};

#[derive(Clone, Debug)]
pub struct PgStore {
    conn: PgPool,
    conn_str: String,
}

fn map_sqlx_err(value: sqlx::Error) -> UserError {
    match &value {
        sqlx::Error::Io(_)
        | sqlx::Error::Tls(_)
        | sqlx::Error::Protocol(_)
        | sqlx::Error::PoolTimedOut
        | sqlx::Error::PoolClosed => UserError::Connection(Box::new(value)),
        sqlx::Error::Database(_) => {
            let e = value.as_database_error();
            if let Some(db_err) = e {
                match db_err.constraint() {
                    Some("users_email_key") => UserError::EmailAlreadyExists(String::new()),
                    Some("users_pkey") => UserError::IDAlreadyExists(None),
                    _ => UserError::Other(Box::new(value)),
                }
            } else {
                UserError::Other(Box::new(value))
            }
        }
        _ => UserError::Other(Box::new(value)),
    }
}

#[async_trait]
impl UserStore for PgStore {
    #[instrument]
    async fn add(&mut self, user: &User) -> SResult<()> {
        let res = sqlx::query_as!(StoreUser, r#"INSERT INTO users (id, name, email, created_at, hash, avatar) VALUES ($1, $2, $3::TEXT::CITEXT, $4, $5, $6)"#,
            user.id,
            user.name,
            user.email,
            user.created_at,
            user.hash,
            user.avatar.as_ref().map(StoreUserAvatar::from).map(Into::<Uuid>::into)
        ).execute(&self.conn)
            .await
            .map(|_| ())
            .map_err(map_sqlx_err)?;
        Ok(res)
    }

    #[instrument]
    async fn delete(&mut self, id: &Uuid) -> SResult<Option<User>> {
        let res = sqlx::query_as!(
            StoreUser,
            r#"DELETE FROM users WHERE id = $1 RETURNING id,name,email::TEXT as "email!",created_at,hash,avatar as "avatar: StoreUserAvatar""#,
            id
        )
            .fetch_optional(&self.conn)
            .await
            .map_err(map_sqlx_err)?
            .map(Into::into);
        Ok(res)
    }

    #[instrument]
    async fn get(&self, id: &Uuid) -> SResult<Option<User>> {
        let res = sqlx::query_as!(
            StoreUser,
            r#"SELECT id,name,email::TEXT as "email!",created_at,hash,avatar as "avatar: StoreUserAvatar" FROM users WHERE id = $1"#,
            id
        )
            .fetch_optional(&self.conn)
            .await
            .map_err(map_sqlx_err)?
            .map(Into::into);
        Ok(res)
    }

    #[instrument]
    async fn get_all(&self) -> SResult<Vec<User>> {
        let res = sqlx::query_as!(
            StoreUser,
            r#"SELECT id,name,email::TEXT as "email!",created_at,hash,avatar as "avatar: StoreUserAvatar" FROM users"#
        )
            .fetch_all(&self.conn)
            .await
            .map_err(map_sqlx_err)?
            .into_iter()
            .map(Into::into)
            .collect();
        Ok(res)
    }

    #[instrument]
    async fn get_by_email(&self, email: &str) -> SResult<Option<User>> {
        let res = sqlx::query_as!(
            StoreUser,
            r#"SELECT id,name,email::TEXT as "email!",created_at,hash,avatar as "avatar: StoreUserAvatar" FROM users WHERE email = $1::TEXT::CITEXT"#,
            email
        )
            .fetch_optional(&self.conn).await
            .map_err(map_sqlx_err)?
            .map(Into::into);
        Ok(res)
    }

    #[instrument]
    async fn update(&mut self, _id: UserUpdate) -> SResult<Option<User>> {
        todo!()
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
