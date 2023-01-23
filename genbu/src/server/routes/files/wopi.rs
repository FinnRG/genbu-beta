use axum::extract::FromRequest;
use http::Request;
use wopi_rs::file::FileRequest;

pub struct Wopi<T>(pub FileRequest<T>);

#[async_trait::async_trait]
impl<S: Send + Sync, B: Send + 'static> FromRequest<S, B> for Wopi<B> {
    type Rejection = http::StatusCode;

    async fn from_request(req: Request<B>, _: &S) -> Result<Self, Self::Rejection> {
        Ok(Wopi(
            FileRequest::try_from(req).map_err(|_| http::StatusCode::BAD_REQUEST)?,
        ))
    }
}
