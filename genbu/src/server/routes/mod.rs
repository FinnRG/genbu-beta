use crate::stores::{files::filesystem::Filesystem, DataStore};

pub mod files;
pub mod users;

pub trait AppState: Send + Sync + Clone + 'static {
    fn store(&self) -> impl DataStore;
    fn file(&self) -> impl Filesystem;
    fn host(&self) -> &str;
}

// TODO: Add Server Config here

#[derive(Clone)]
pub struct ServerAppState<S: DataStore, F: Filesystem> {
    store: S,
    file: F,
    host: String,
}

impl<S: DataStore, F: Filesystem> ServerAppState<S, F> {
    pub fn new(store: S, file: F, host: String) -> Self {
        Self { store, file, host }
    }
}

impl<S: DataStore, F: Filesystem> AppState for ServerAppState<S, F> {
    fn store(&self) -> impl DataStore {
        self.store.clone()
    }

    fn file(&self) -> impl Filesystem {
        self.file.clone()
    }

    fn host(&self) -> &str {
        &self.host
    }
}
