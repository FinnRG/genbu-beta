use std::fmt::Debug;

use genbu_server::connectors::{memory::MemStore, postgres::PgStore, s3};
use genbu_server::server::builder::GenbuServerBuilder;
use genbu_server::stores::{DataStore, Setup};
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

async fn send_test_email() -> Result<(), Box<dyn std::error::Error>> {
    let mailer = AsyncSmtpTransport::<Tokio1Executor>::relay("localhost")?
        .port(1025)
        .build();
    let email = Message::builder()
        .from("Genbu <no-reply@genbu.com>".parse()?)
        .to("FinnRG <finngaertner2@gmx.de>".parse()?)
        .subject("TestTestTest")
        .body("This is a test".to_string())?;
    mailer.send(email).await.unwrap();
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), impl Debug> {
    // TODO: Remove this
    // dbg!(send_test_email().await.unwrap());
    // return Ok(());
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
        .with_store(_pg_store)
        .with_file_store(s3_store)
        .build()
        .unwrap();
    server.start().await
}
