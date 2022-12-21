use std::fmt::Debug;

use genbu_axum_server::builder::GenbuServerBuilder;
use genbu_default_connectors::{memory::MemStore, postgres::PgStore, s3};
use genbu_stores::{files::file_storage::FileStore, stores::DataStore};

#[tokio::main]
async fn main() -> Result<(), impl Debug> {
    let _pg_store = PgStore::new("postgres://genbu:strong_password@127.0.0.1:5432/genbu".into())
        // TODO:
        // Make
        // this
        // configurable
        .await
        .unwrap();
    let _mem_store = MemStore::new();

    let mut s3_store = s3::S3Store::new().await;
    s3_store.setup().await;

    let server = GenbuServerBuilder::new()
        .with_store(_mem_store)
        .with_file_store(s3_store)
        .build()
        .unwrap();
    server.start().await
}
