use axum::http::{header, Request, StatusCode};
use genbu_server::stores::{users::User, Uuid};
use serde_json::{json, Value};

mod common;
use common::{response_json, RequestBuilderExt, TestClient};

#[tokio::test]
async fn basic_email_login() {
    let mut client = TestClient::new().await;
    let resp1 = client
        .request_raw(Request::post("/api/register").json(json! {{
            "name": "TestUser",
            "email": "test@example.com",
            "password": "strong_password"
        }}))
        .await;

    assert_eq!(resp1.status(), StatusCode::OK);
    assert!(resp1.headers().contains_key(header::SET_COOKIE));

    let resp2 = client
        .request_raw(Request::post("/api/login").json(json! {{
                "email": "test@example.com",
                "password": "strong_password"
        }}))
        .await;

    assert_eq!(resp2.status(), StatusCode::OK);
    assert!(resp2.headers().contains_key(header::SET_COOKIE));

    let resp3 = client
        .request_raw(Request::post("/api/login").json(json! {{
                "email": "test@example.com",
                "password": "false_password"
        }}))
        .await;

    assert_eq!(resp3.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn user_not_found_404() {
    let mut client = TestClient::new().await;
    client.register_default().await;

    let id = Uuid::new_v4();
    let resp1 = client
        .request(Request::get(format!("/api/user/{id}")).empty_body())
        .await;
    assert_eq!(resp1.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn conflict_duplicate_email() {
    let mut client = TestClient::new().await;
    client.register_default().await;

    let resp1 = client
        .request(Request::post("/api/register").json(json! {{
            "name": "TestUser2",
            "email": "test@example.com",
            "password": "strong_password"
        }}))
        .await;

    assert_eq!(resp1.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn password_normalization() {
    let mut client = TestClient::new().await;

    let _ = client
        .request_raw(Request::post("/api/register").json(json! {{
            "name": "TestUser",
            "email": "test@example.com",
            "password": "sⓣrong_password"
        }}))
        .await;

    let resp1 = client
        .request_raw(Request::post("/api/login").json(json! {{
            "email": "test@example.com",
            "password": "ⓢtrong_password"
        }}))
        .await;

    assert_eq!(resp1.status(), StatusCode::OK);
}

#[tokio::test]
// A register attempt without a specified name should fail
async fn register_require_name() {
    let mut client = TestClient::new().await;

    let resp1 = client
        .request_raw(Request::post("/api/register").json(json! {{
            "email": "test@example.com",
            "password": "strong_password"
        }}))
        .await;

    assert_eq!(resp1.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert!(!resp1.headers().contains_key(header::SET_COOKIE));
}

#[tokio::test]
// Tests that two users with different email adresses and the same username can register
async fn double_register() {
    let mut client = TestClient::new().await;
    client.register_default().await;

    let resp1 = client
        .request(Request::post("/api/register").json(json! {{
            "name": "TestUser",
            "email": "test2@example.com",
            "password": "strong_password"
        }}))
        .await;
    assert_eq!(resp1.status(), StatusCode::OK);
}

#[tokio::test]
// Tests all routes under /api/user which require authorization
async fn require_authn() {
    let mut client = TestClient::new().await;

    let get_user_all = client
        .request(Request::get("/api/user/all").empty_body())
        .await;
    let get_spec_user = client
        .request(Request::get("/api/user/132").empty_body())
        .await;
    let get_unspec_user = client.request(Request::get("/api/user").empty_body()).await;
    let delete_spec_user = client
        .request(Request::delete("/api/user/132").empty_body())
        .await;
    let add_user = client
        .request(Request::post("/api/user").empty_body())
        .await;
    let patch_user = client
        .request(Request::patch("/api/user/132").empty_body())
        .await;
    assert_eq!(get_user_all.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(get_spec_user.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(get_unspec_user.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(delete_spec_user.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(add_user.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(patch_user.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn get_all_users() {
    let mut client = TestClient::new().await;
    client.register_default().await;

    let req = Request::get("/api/user/all");
    let mut resp2 = client.request(req.empty_body()).await;

    let resp2_json = response_json(&mut resp2).await;
    assert!(matches!(resp2_json, Value::Array(_)));

    let Value::Array(content) = resp2_json else {
        panic!("resp2_json should be an array");
    };
    assert_eq!(content.len(), 1);

    assert!(matches!(content[0]["id"], Value::String(_)));
    assert_eq!(content[0]["email"], "test@example.com");
    assert_eq!(content[0]["name"], "TestUser");
    assert_eq!(content[0].get("hash"), None);
}

#[tokio::test]
// GET /api/user/all and GET /api/user/:id should never return the hashed user password
async fn never_return_hash() {
    let mut client = TestClient::new().await;
    client.register_default().await;

    let mut get_all = client
        .request(Request::get("/api/user/all").empty_body())
        .await;

    let Value::Array(content) = response_json(&mut get_all).await else {
        panic!("get_all should return an array");
    };

    assert_eq!(content[0].get("hash"), None);

    let id = match content[0].get("id") {
        Some(Value::String(s)) => s,
        _ => panic!("content should include an id"),
    };
    let mut get_single = client
        .request(Request::get(format!("/api/user/{id}")).empty_body())
        .await;

    let get_single_json = response_json(&mut get_single).await;
    assert_eq!(get_single_json.get("hash"), None);
}

#[tokio::test]
async fn delete_user() {
    let mut client = TestClient::new().await;
    client.register_default().await;

    let mut get_all = client
        .request(Request::get("/api/user/all").empty_body())
        .await;

    let Value::Array(content) = response_json(&mut get_all).await else {
        panic!("get_all should return an array");
    };

    assert_eq!(content.len(), 1);

    let id = content[0].get("id").unwrap().as_str().unwrap();

    client
        .request(Request::delete(format!("/api/user/{id}")).empty_body())
        .await;

    let mut get_all = client
        .request(Request::get("/api/user/all").empty_body())
        .await;

    let Value::Array(content) = response_json(&mut get_all).await else {
        panic!("get_all should return an array");
    };

    assert_eq!(content.len(), 0);
}

#[tokio::test]
async fn update_user() {
    let mut client = TestClient::new().await;
    client.register_default().await;
    let mut get_user_req = client
        .request(Request::get("/api/user/all").empty_body())
        .await;
    let users: Vec<User> = serde_json::from_value(response_json(&mut get_user_req).await).unwrap();
    let user_id = users[0].id;

    // Response should contain the updated user
    let mut update_user_req = client
        .request(
            Request::patch("/api/user/".to_owned() + &user_id.to_string()).json(json! {{
                "name": "UpdatedName"
            }}),
        )
        .await;
    let users: User = serde_json::from_value(response_json(&mut update_user_req).await).unwrap();
    assert_eq!(users.name, "UpdatedName");

    // Get all should return the updated user
    let mut get_user_req = client
        .request(Request::get("/api/user/all").empty_body())
        .await;
    let users: Vec<User> = serde_json::from_value(response_json(&mut get_user_req).await).unwrap();
    assert_eq!(users[0].name, "UpdatedName");
}

#[tokio::test]
async fn restrict_update() {
    let mut client = TestClient::new().await;
    client.register_default().await;
    let mut get_user_req = client
        .request(Request::get("/api/user/all").empty_body())
        .await;
    let users: Vec<User> = serde_json::from_value(response_json(&mut get_user_req).await).unwrap();
    let user_id = users[0].id;

    let new_id = Uuid::new_v4();
    // Response should contain the updated user
    client
        .request(
            Request::patch("/api/user/".to_owned() + &user_id.to_string()).json(json! {{
                "id": &new_id.to_string()
            }}),
        )
        .await;

    // Get all should return the updated user
    let mut get_user_req = client
        .request(Request::get("/api/user/all").empty_body())
        .await;
    let users: Vec<User> = serde_json::from_value(response_json(&mut get_user_req).await).unwrap();
    assert_ne!(users[0].id, new_id);
}
