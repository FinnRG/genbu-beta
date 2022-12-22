use std::iter::once;

use axum::{
    response::{IntoResponse, Redirect},
    Extension, Router, Server,
};
use axum_extra::routing::SpaRouter;
use genbu_stores::{files::file_storage::FileStore, stores::DataStore};
use hyper::{header, Uri};
use tower::ServiceBuilder;
use tower_http::{
    cors::CorsLayer, sensitive_headers::SetSensitiveRequestHeadersLayer, trace::TraceLayer,
};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::{
    apidoc::ApiDoc,
    routes::{files, users},
};

pub struct GenbuServerBuilder<S: DataStore, F: FileStore> {
    users: Option<S>,
    files: Option<F>,
}

pub struct GenbuServer<S: DataStore, F: FileStore> {
    users: S,
    files: F,
}

impl<S: DataStore + Send + Sync, F: FileStore + Send + Sync> GenbuServerBuilder<S, F> {
    #[must_use]
    pub fn new() -> Self {
        GenbuServerBuilder {
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

impl<S: DataStore, F: FileStore> Default for GenbuServerBuilder<S, F> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: DataStore, F: FileStore> GenbuServer<S, F> {
    fn api_router() -> Router {
        users::router::<S>()
            .merge(files::router::<F>())
            .merge(SwaggerUi::new("/swagger-ui").url("/api-doc/openapi.json", ApiDoc::openapi()))
    }

    pub fn app(&self) -> Router {
        let mut app = Self::api_router()
            .layer(
                ServiceBuilder::new()
                    .layer(SetSensitiveRequestHeadersLayer::new(once(header::COOKIE)))
                    .layer(TraceLayer::new_for_http()),
            )
            .layer(Extension(self.users.clone()))
            .layer(Extension(self.files.clone()));
        #[cfg(not(debug_assertions))]
        {
            let spa = SpaRouter::new("", "../genbu-frontend/dist");
            app = app.merge(spa);
        }
        #[cfg(debug_assertions)]
        {
            app = app.layer(CorsLayer::permissive())
        }
        app
    }

    // TODO: Proper error handling
    pub async fn start(&self) -> Result<(), hyper::Error> {
        tracing_subscriber::fmt::init();

        let app = self.app();

        Server::bind(&"0.0.0.0:8080".parse().unwrap())
            .serve(app.into_make_service())
            .await
    }
}
