use axum::http::{Request, StatusCode};
use common::TestClient;
use genbu_server::handler::files::{upload::UploadFileResponse, userfiles::GetUserfilesResponse};
use reqwest::Client;
use serde_json::json;

use crate::common::{response_json, RequestBuilderExt, Result};

mod common;

async fn upload_small_file(mut client: TestClient) -> Result<()> {
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

    let buffer: Vec<u8> = vec![0; 2365];
    assert_eq!(buffer.len(), 2365);
    let upload_resp = Client::new()
        .put(&uris[0])
        .body(buffer)
        .header("Content-Type", "image/png")
        .send()
        .await?;
    assert_eq!(upload_resp.status(), StatusCode::OK);
    let e_tag = upload_resp.headers().get("ETag").unwrap().to_str().unwrap();

    let resp = client
        .request(Request::post("/api/files/upload/finish").json(json! {{
            "lease_id": resp.lease_id,
            "upload_id": resp.upload_id.unwrap(),
            "parts" : [
                {
                    "e_tag": e_tag,
                    "part_number": 1
                }
            ]
        }}))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);

    Ok(())
}

#[tokio::test]
async fn test_upload_small_file() -> Result<()> {
    let mut client = TestClient::new().await;
    client.register_default().await;
    upload_small_file(client).await?;
    Ok(())
}

#[tokio::test]
async fn restrict_file_size() -> Result<()> {
    let mut client = TestClient::new().await;
    client.register_default().await;

    let resp = client
        .request(Request::post("/api/files/upload").json(json! {{
            "name": "test.jpg",
            "size": 1_000_000_100
        }}))
        .await;

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    Ok(())
}

#[tokio::test]
async fn negative_size() -> Result<()> {
    let mut client = TestClient::new().await;
    client.register_default().await;

    let resp = client
        .request(Request::post("/api/files/upload").json(json! {{
            "name": "test.jpg",
            "size": -10
        }}))
        .await;

    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

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

#[tokio::test]
async fn get_userfiles() -> Result<()> {
    let mut client = TestClient::new().await;
    client.register_default().await;
    upload_small_file(client.clone()).await?;
    let mut resp = client
        .request(Request::get("/api/filesystem?base_path=").empty_body())
        .await;
    let resp: GetUserfilesResponse = serde_json::from_value(response_json(&mut resp).await)?;
    assert_eq!(resp.files.len(), 1);
    assert_eq!(resp.files[0].name, "test.jpg");
    assert_eq!(resp.files[0].size, Some(2365));
    Ok(())
}
