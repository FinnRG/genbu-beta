use std::borrow::BorrowMut;

use axum::{
    body::{Body, BoxBody, Bytes, HttpBody},
    http::{header, request, HeaderValue, Request, Response, StatusCode},
    Router,
};
use genbu_server::{
    connectors::{postgres::PgStore, s3},
    server::{builder::GenbuServer, routes::ServerAppState},
    stores::{DataStore, Setup, Uuid},
};
use http_body::combinators::UnsyncBoxBody;
use serde_json::json;
use tower::ServiceExt;

pub trait RequestBuilderExt {
    fn json(self, json: serde_json::Value) -> Request<Body>;

    fn empty_body(self) -> Request<Body>;
}

impl RequestBuilderExt for request::Builder {
    fn json(self, json: serde_json::Value) -> Request<Body> {
        self.header("Content-Type", "application/json")
            .body(Body::from(json.to_string()))
            .expect("failed to build request")
    }

    fn empty_body(self) -> Request<Body> {
        self.body(Body::empty()).expect("failed to build request")
    }
}

pub async fn response_json(resp: &mut axum::http::Response<BoxBody>) -> serde_json::Value {
    assert_eq!(
        resp.headers()
            .get(header::CONTENT_TYPE)
            .expect("expected Content-Type"),
        "application/json"
    );

    let body = resp.body_mut();

    let mut bytes = Vec::new();

    while let Some(res) = body.data().await {
        let chunk = res.expect("error reading response body");

        bytes.extend_from_slice(&chunk[..]);
    }

    serde_json::from_slice(&bytes).expect("failed to read response body as json")
}

#[derive(Clone)]
pub struct TestClient {
    app: Router,
    token: Option<HeaderValue>,
}

impl TestClient {
    pub async fn new() -> Self {
        // TODO: Use PgStore and MemStore based on feature flags
        let app = build_app().await;
        TestClient { app, token: None }
    }

    pub async fn register_default(&mut self) {
        let resp = self
            .request(Request::post("/api/register").json(json! {{
                "name": "TestUser",
                "email": "test@example.com",
                "password": "strong_password"
            }}))
            .await;

        assert_eq!(resp.status(), StatusCode::OK);
        assert!(resp.headers().contains_key(header::SET_COOKIE));

        self.token = Some(resp.headers().get(header::SET_COOKIE).unwrap().clone());
    }

    pub async fn request(
        &mut self,
        mut req: Request<Body>,
    ) -> Response<UnsyncBoxBody<Bytes, axum::Error>> {
        if let Some(token) = self.token.clone() {
            req.headers_mut().insert(header::COOKIE, token);
        }
        self.request_raw(req).await
    }

    pub async fn request_raw(
        &mut self,
        req: Request<Body>,
    ) -> Response<UnsyncBoxBody<Bytes, axum::Error>> {
        self.app.borrow_mut().oneshot(req).await.unwrap()
    }
}

pub async fn build_app() -> Router {
    dotenvy::dotenv().expect("Unable to start dotenvy");

    let mut pg_store = PgStore::new(build_connection_string(&Uuid::new_v4().to_string()))
        // TODO:
        // Make
        // this
        // configurable
        .await
        .unwrap();
    pg_store.setup().await.expect("Unable to setup store");
    let mut file_store = s3::S3Store::new().await;
    file_store
        .setup()
        .await
        .expect("Unable to setup file_store");
    let state = ServerAppState::new(pg_store, file_store, "http://localhost:8080".to_owned());
    GenbuServer::new(state).app()
}

pub fn build_connection_string(db_name: &str) -> String {
    "postgres://genbu:strong_password@127.0.0.1:5432/gtest-".to_owned() + db_name
}

#[allow(dead_code)]
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
