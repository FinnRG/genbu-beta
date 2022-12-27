use axum::{
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use axum_extra::extract::CookieJar;
use genbu_auth::authn::validate_jwt;
use tracing::{debug, warn};

#[tracing::instrument(skip_all)]
pub async fn auth<B>(
    cookie_jar: CookieJar,
    mut req: Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> {
    let token_cookie = cookie_jar.get("__Host-Token").ok_or_else(|| {
        warn!("authn_token_not_provided attempted unauthorized access");
        StatusCode::UNAUTHORIZED
    })?;

    match validate_jwt(token_cookie.value()) {
        Ok(claims) => {
            req.extensions_mut().insert(claims);
            debug!("authn_token_accepted jwt validated");
            Ok(next.run(req).await)
        }
        Err(e) => {
            warn!("authn_token_invalid jwt error: {:?}", e);
            Err(e.into())
        }
    }
}
