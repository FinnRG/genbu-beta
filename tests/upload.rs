use std::{fs::File, io::Read};

use axum::http::{Request, StatusCode};
use common::TestClient;
use genbu_server::handler::files::upload::UploadFileResponse;
use reqwest::Client;
use serde_json::json;

use crate::common::{response_json, RequestBuilderExt, Result};

mod common;

#[tokio::test]
async fn upload_small_file() -> Result<()> {
    let mut client = TestClient::new().await;
    client.register_default().await;

    let mut resp = client
        .request(Request::post("/api/files/upload").json(json! {{
            "name": "test.jpg",
            "size": 2365
        }}))
        .await;

    assert_eq!(resp.status(), StatusCode::OK);

    let resp: UploadFileResponse = serde_json::from_value(response_json(&mut resp).await)?;

    let uris = resp.uris;
    assert_eq!(uris.len(), 1);

    let mut buffer = Vec::new();
    let mut file = File::open("./tesfile.png")?;
    file.read(&mut buffer)?;
    let upload_resp = Client::new()
        .put(&uris[0])
        .body(buffer)
        .header("Content-Type", "image/png")
        .send()
        .await?;

    assert_eq!(upload_resp.status(), StatusCode::OK);

    Ok(())
}

#[tokio::test]
async fn restrict_file_size() -> Result<()> {
    let mut client = TestClient::new().await;
    client.register_default().await;

    let mut resp = client
        .request(Request::post("/api/files/upload").json(json! {{
            "name": "test.jpg",
            "size": 1_000_000_100
        }}))
        .await;

    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let resp = response_json(&mut resp).await;
    assert_eq!(resp.get("error").unwrap(), "File is too large");

    Ok(())
}

#[tokio::test]
async fn upload_multipart() -> Result<()> {
    let mut client = TestClient::new().await;
    client.register_default().await;

    let mut resp = client
        .request(Request::post("/api/files/upload").json(json! {{
            "name": "test.jpg",
            "size": 20_000_000
        }}))
        .await;

    assert_eq!(resp.status(), StatusCode::OK);

    let resp: UploadFileResponse = serde_json::from_value(response_json(&mut resp).await)?;
    assert!(resp.upload_id.is_some());
    assert!(resp.uris.len() > 1);

    Ok(())
}
