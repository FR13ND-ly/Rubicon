use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{HeaderValue, Request, StatusCode},
    response::Response,
    routing::{get, post},
};
use rubicon::{Backend, JavaScriptRuntime, backend_extension};

#[tokio::test(flavor = "current_thread")]
async fn native_backend_reuses_routes_and_recovers_after_errors() {
    let calls = Arc::new(AtomicUsize::new(0));
    let state = calls.clone();
    let routes = Router::new()
        .route(
            "/api/echo",
            post(|request: Request<Body>| async move {
                let query = request.uri().query().unwrap_or("").to_owned();
                let input = request.headers().get("x-input").unwrap().clone();
                let bytes = to_bytes(request.into_body(), 1024).await.unwrap();
                Response::builder()
                    .status(StatusCode::ACCEPTED)
                    .header("x-query", query)
                    .header("x-input", input)
                    .body(Body::from(bytes))
                    .unwrap()
            }),
        )
        .route(
            "/api/cookies",
            get(|| async {
                let mut response = Response::new(Body::from("cookies"));
                response
                    .headers_mut()
                    .append("set-cookie", HeaderValue::from_static("a=1"));
                response
                    .headers_mut()
                    .append("set-cookie", HeaderValue::from_static("b=2"));
                response
                    .headers_mut()
                    .insert("x-latin", HeaderValue::from_bytes(&[0xe9]).unwrap());
                response
            }),
        )
        .route(
            "/api/status",
            get(|| async { (StatusCode::SERVICE_UNAVAILABLE, "Unavailable") }),
        )
        .route("/api/large", get(|| async { "x".repeat(1025) }))
        .route(
            "/api/count",
            get(move || {
                let state = state.clone();
                async move {
                    tokio::task::yield_now().await;
                    (state.fetch_add(1, Ordering::SeqCst) + 1).to_string()
                }
            }),
        );
    let backend = Backend::new(routes);
    let mut runtime =
        JavaScriptRuntime::with_extensions(vec![backend_extension(backend, 1024)]).unwrap();
    let entry = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/backend_native.mjs");
    let module = runtime.load_main_module(&entry).await.unwrap();
    let checked: u32 = runtime.call_export(module, "check", &()).await.unwrap();
    assert_eq!(checked, 10);
    assert_eq!(calls.load(Ordering::SeqCst), 8);
}
