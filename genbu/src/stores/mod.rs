use async_trait::async_trait;
use std::error::Error;

pub mod files;
pub mod groups;
pub mod users;

pub type Uuid = uuid::Uuid;
pub type UuidError = uuid::Error;
pub type OffsetDateTime = time::OffsetDateTime;

#[async_trait]
pub trait DataStore:
    users::UserStore
    + files::UploadLeaseStore
    + files::database::DBFileStore
    + Reset
    + Setup
    + Sized
    + Send
    + Sync
    + Clone
    + 'static
{
    // TODO: Replace this with server config
    async fn new(arg: String) -> Result<Self, Box<dyn Error>>;
}

#[async_trait]
pub trait Setup {
    async fn setup(&mut self) -> Result<(), Box<dyn Error>>;
}

#[async_trait]
pub trait Reset {
    #[cfg(debug_assertions)]
    async fn reset(&mut self) -> Result<(), Box<dyn Error>>;
}
