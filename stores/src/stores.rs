use async_trait::async_trait;
use std::error::Error;

use crate::users::UserStore;

#[async_trait]
pub trait DataStore: Sized + UserStore + Reset + Setup {
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
