use std::{iter::once, time::Duration};

use crate::stores::{files::filesystem::Filesystem, DataStore};
use axum::{
    body::{Body, BoxBody},
    routing::get,
    Extension, Router, Server,
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
    routes::{files, users},
};

pub struct GenbuServerBuilder<S: DataStore, F: Filesystem> {
    users: Option<S>,
    files: Option<F>,
}

pub struct GenbuServer<S: DataStore, F: Filesystem> {
    users: S,
    files: F,
}

impl<S: DataStore, F: Filesystem> GenbuServerBuilder<S, F> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            users: None,
            files: None,
        }
    }

    pub fn with_store(&mut self, user_store: S) -> &mut Self {
        self.users = Some(user_store);
        self
    }

    pub fn with_file_store(&mut self, file_store: F) -> &mut Self {
        self.files = Some(file_store);
        self
    }

    #[must_use]
    pub fn build(&mut self) -> Option<GenbuServer<S, F>> {
        self.users.as_ref()?;
        Some(GenbuServer {
            users: self.users.take().unwrap(),
            files: self.files.take().unwrap(),
        })
    }
}

impl<S: DataStore, F: Filesystem> Default for GenbuServerBuilder<S, F> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: DataStore, F: Filesystem> GenbuServer<S, F> {
    fn api_router() -> Router {
        users::router::<S>()
            .merge(files::router::<F, S>())
            .merge(SwaggerUi::new("/swagger-ui").url("/api-doc/openapi.json", ApiDoc::openapi()))
    }

    pub fn app(&self) -> Router {
        let mut app = Self::api_router()
            .layer(
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

                                    tracing::debug!("response generated")
                                },
                            ),
                    ),
            )
            .layer(Extension(self.users.clone()))
            .layer(Extension(self.files.clone()));
        if cfg!(any(test, feature = "testing")) {
            let (prometheus_layer, metric_handle) = PrometheusMetricLayer::pair();
            app = app
                .layer(prometheus_layer)
                .route("/metrics", get(|| async move { metric_handle.render() }));
        }
        //#[cfg(not(debug_assertions))]
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
