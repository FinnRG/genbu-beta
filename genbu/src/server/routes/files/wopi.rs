use axum::{body::Body, extract::FromRequest, response::IntoResponse};
use bytes::Bytes;
use http::Request;
use hyper::body::to_bytes;
use tracing::error;
use wopi_rs::file::FileRequest;

pub struct Wopi<T>(pub FileRequest<T>);
pub struct WopiResponse(pub http::Response<Bytes>);

#[async_trait::async_trait]
impl<S: Send + Sync> FromRequest<S, Body> for Wopi<Bytes> {
    type Rejection = http::StatusCode;

    async fn from_request(req: Request<Body>, _: &S) -> Result<Self, Self::Rejection> {
        let (parts, b) = req.into_parts();
        let b = to_bytes(b).await.map_err(|e| {
            error!("error while collecting body {:?}", e);
            http::StatusCode::INTERNAL_SERVER_ERROR
        })?;
        let req = Request::from_parts(parts, b);
        Ok(Wopi(
            FileRequest::try_from(req).map_err(|_| http::StatusCode::BAD_REQUEST)?,
        ))
    }
}

impl IntoResponse for WopiResponse {
    fn into_response(self) -> axum::response::Response {
        let resp: http::Response<Bytes> = self.0;
        let (parts, b) = resp.into_parts();
        (parts, b).into_response()
    }
}
