use std::{iter::once, time::Duration};

use axum::{
    body::{Body, BoxBody},
    routing::get,
    Router, Server,
};
use axum_prometheus::PrometheusMetricLayer;
use http::{Request, Response};
use hyper::header;
use tower::ServiceBuilder;
use tower_http::{
    cors::CorsLayer, sensitive_headers::SetSensitiveRequestHeadersLayer, trace::TraceLayer,
};
use tracing::Span;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use super::{
    apidoc::ApiDoc,
    routes::{files, users, AppState},
};

pub struct GenbuServer<S: AppState> {
    state: S,
}

impl<S: AppState> GenbuServer<S> {
    fn api_router(&self) -> Router {
        users::router::<S>()
            .merge(files::router::<S>())
            .merge(SwaggerUi::new("/swagger-ui").url("/api-doc/openapi.json", ApiDoc::openapi()))
            .with_state(self.state.clone())
    }

    pub fn new(state: S) -> Self {
        Self { state }
    }

    pub fn app(&self) -> Router {
        let mut app = self.api_router().layer(
            ServiceBuilder::new()
                .layer(SetSensitiveRequestHeadersLayer::new(once(header::COOKIE)))
                .layer(
                    // TODO: Refactor this into a separate file
                    TraceLayer::new_for_http()
                        .make_span_with(|req: &Request<Body>| {
                            tracing::debug_span!(
                                "request",
                                status_code = tracing::field::Empty,
                                uri = req.uri().to_string()
                            )
                        })
                        .on_response(
                            |response: &Response<BoxBody>, _latency: Duration, span: &Span| {
                                span.record("status_code", response.status().as_u16());

                                tracing::debug!("response generated");
                            },
                        ),
                ),
        );
        if cfg!(any(test, feature = "testing")) {
            let (prometheus_layer, metric_handle) = PrometheusMetricLayer::pair();
            app = app
                .layer(prometheus_layer)
                .route("/metrics", get(|| async move { metric_handle.render() }));
        }

        // TODO: Move frontend into this repo
        let spa = tower_http::services::ServeDir::new("./dist");
        app = app.nest_service("", spa);

        #[cfg(debug_assertions)]
        {
            app = app.layer(CorsLayer::very_permissive());
        }
        app
    }

    // TODO: Proper error handling
    pub async fn start(&self) -> Result<(), hyper::Error> {
        let app = self.app();

        Server::bind(&"0.0.0.0:8080".parse().unwrap())
            .serve(app.into_make_service())
            .await
    }
}
