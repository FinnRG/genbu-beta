use axum::{
    body::Body,
    extract::{FromRequest, FromRequestParts, Query, State},
    response::IntoResponse,
    RequestPartsExt,
};
use bytes::Bytes;
use http::{request::Parts, Request, StatusCode};
use hyper::body::to_bytes;
use serde::Deserialize;
use tracing::{error, warn};
use wopi_rs::WopiRequest;

use crate::{
    server::routes::AppState,
    stores::{
        files::access_token::{AccessToken, AccessTokenContext, AccessTokenStore},
        Uuid,
    },
};

pub struct Wopi<T: TryFrom<http::Request<Bytes>>>(pub WopiRequest<T>);
pub struct WopiResponse(pub http::Response<Bytes>);

#[async_trait::async_trait]
impl<T: TryFrom<http::Request<Bytes>>, S: Send + Sync> FromRequest<S, Body> for Wopi<T> {
    type Rejection = StatusCode;

    // TODO: Check Content-Length for malicious input (see to_bytes docs for example)
    async fn from_request(req: Request<Body>, _: &S) -> Result<Self, Self::Rejection> {
        let (parts, b) = req.into_parts();
        let b = to_bytes(b).await.map_err(|e| {
            error!("error while collecting body {:?}", e);
            http::StatusCode::INTERNAL_SERVER_ERROR
        })?;
        let req = Request::from_parts(parts, b);
        Ok(Wopi(
            WopiRequest::try_from(req).map_err(|_| http::StatusCode::BAD_REQUEST)?,
        ))
    }
}

pub struct WopiAuth(pub AccessTokenContext);

#[derive(Deserialize)]
pub struct WopiQuery {
    access_token: Option<String>,
}

#[async_trait::async_trait]
impl<S: Send + Sync + AppState> FromRequestParts<S> for WopiAuth {
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Query(wopi) = parts.extract::<Query<WopiQuery>>().await.map_err(|_| {
            warn!("unauthorized wopi query attempt");
            StatusCode::UNAUTHORIZED
        })?;

        let access_token: AccessToken = wopi
            .access_token
            .ok_or_else(|| {
                warn!("no access token provided");
                StatusCode::UNAUTHORIZED
            })?
            .parse::<Uuid>()
            .map_err(|_| {
                warn!("unable to parse access token as uuid");
                StatusCode::BAD_REQUEST
            })?
            .into();

        let context = match state.store().get_token_context(access_token).await {
            Ok(Some(c)) => c,
            _ => {
                warn!("token {access_token:?} not found");
                return Err(StatusCode::UNAUTHORIZED);
            }
        };

        Ok(WopiAuth(context))
    }
}

impl IntoResponse for WopiResponse {
    fn into_response(self) -> axum::response::Response {
        let resp: http::Response<Bytes> = self.0;
        let (parts, b) = resp.into_parts();
        (parts, b).into_response()
    }
}
