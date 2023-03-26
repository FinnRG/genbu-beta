use axum::{body::Body, extract::FromRequest, response::IntoResponse};
use bytes::Bytes;
use http::Request;
use hyper::body::to_bytes;
use tracing::error;

pub struct Wopi<T: TryFrom<http::Request<Bytes>>>(pub T);
pub struct WopiResponse(pub http::Response<Bytes>);

#[async_trait::async_trait]
impl<T: TryFrom<http::Request<Bytes>>, S: Send + Sync> FromRequest<S, Body> for Wopi<T> {
    type Rejection = http::StatusCode;

    // TODO: Check Content-Length for malicious input (see to_bytes docs for example)
    async fn from_request(req: Request<Body>, _: &S) -> Result<Self, Self::Rejection> {
        let (parts, b) = req.into_parts();
        let b = to_bytes(b).await.map_err(|e| {
            error!("error while collecting body {:?}", e);
            http::StatusCode::INTERNAL_SERVER_ERROR
        })?;
        let req = Request::from_parts(parts, b);
        Ok(Wopi(
            T::try_from(req).map_err(|_| http::StatusCode::BAD_REQUEST)?,
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
