use std::fmt::Debug;

use genbu_server::connectors::{postgres::PgStore, s3};
use genbu_server::server::builder::GenbuServerBuilder;
use genbu_server::stores::{DataStore, Setup};
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use opentelemetry::sdk::propagation::TraceContextPropagator;
use opentelemetry::{global, runtime::Tokio};
use tracing::info;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{layer::SubscriberExt, EnvFilter};

async fn _send_test_email() -> Result<(), Box<dyn std::error::Error>> {
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

async fn init_telemetry() {
    global::set_text_map_propagator(TraceContextPropagator::new());
    let jaeger_tracer = opentelemetry_jaeger::new_agent_pipeline()
        .with_endpoint("0.0.0.0:6831")
        .with_service_name("genbu-server")
        .install_batch(Tokio)
        .expect("unable to install opentelemetry-jaeger");
    let fmt_layer = tracing_subscriber::fmt::layer().json();
    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(tracing_opentelemetry::layer().with_tracer(jaeger_tracer))
        .with(EnvFilter::from_default_env())
        .try_init()
        .expect("unable to initialize tacing-subscriber");
}

#[tokio::main]
async fn main() -> Result<(), impl Debug> {
    dotenvy::dotenv().expect("unable to initialize dotenvy");
    init_telemetry().await;

    info!("Trying to connect to to postgres");
    let pg_store = PgStore::new("postgres://genbu:strong_password@127.0.0.1:5432/genbu".into())
        // TODO:
        // Make
        // this
        // configurable
        .await
        .expect("unable to connect to Postgres");

    let mut s3_store = s3::S3Store::new().await;

    info!("Trying to connect to S3");
    s3_store.setup().await.expect("unable to setup S3");

    info!("Starting server");
    let server = GenbuServerBuilder::new()
        .with_store(pg_store)
        .with_file_store(s3_store)
        .build()
        .unwrap();
    server.start().await
}
