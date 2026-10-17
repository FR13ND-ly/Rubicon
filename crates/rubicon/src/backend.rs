use axum::{Router, body::Body, http::Request, response::Response};
use tower::ServiceExt;

#[derive(Clone)]
pub struct Backend {
    routes: Router,
}

impl Backend {
    pub fn new(routes: Router) -> Self {
        Self { routes }
    }

    pub async fn dispatch(&self, request: Request<Body>) -> Response {
        match self.routes.clone().oneshot(request).await {
            Ok(response) => response,
            Err(error) => match error {},
        }
    }
}
