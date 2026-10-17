use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use axum::{Json, Router, routing::get};
use rubicon::{
    Backend, JavaScriptRuntime, RenderRequest, Renderer, RendererConfig, backend_extension,
};

#[path = "../src/catalog.rs"]
mod catalog;

#[tokio::test(flavor = "current_thread")]
#[ignore = "Requires the Angular bundle: rb build --config examples/angular-demo/rubicon.toml"]
async fn angular_http_client_uses_existing_rust_routes_and_automatic_transfer_cache() {
    let calls = Arc::new(AtomicUsize::new(0));
    let route_calls = calls.clone();
    let backend = Backend::new(Router::new().route(
        "/api/products",
        get(move || {
            let calls = route_calls.clone();
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Json(catalog::list_products().await)
            }
        }),
    ));
    let entry = Path::new(env!("CARGO_MANIFEST_DIR")).join("../dist/rubicon/server/server.mjs");
    let renderer = Renderer::start_with_config(
        entry,
        RendererConfig {
            workers: 2,
            max_in_flight: 4,
            queue_capacity: 16,
        },
        move || {
            JavaScriptRuntime::with_extensions(vec![backend_extension(
                backend.clone(),
                2 * 1024 * 1024,
            )])
        },
    )
    .await
    .unwrap();

    // Every render creates its own application and HTTP transfer-cache scope.
    let mut tasks = Vec::new();
    for index in 0..8 {
        let renderer = renderer.clone();
        tasks.push(tokio::spawn(async move {
            let response = renderer
                .render(RenderRequest {
                    request_id: index.to_string(),
                    url: format!(
                        "http://localhost/{}?request={index}",
                        if index % 2 == 0 {
                            "products"
                        } else {
                            "dashboard"
                        }
                    ),
                    method: "GET".into(),
                    headers: Vec::new(),
                })
                .await
                .unwrap();
            assert_eq!(response.status, 200);
            let html = response.body.as_text().unwrap();
            for expected in ["Tastatură", "Mouse", "Monitor", "ng-state", "\"keyboard\""] {
                assert!(html.contains(expected), "Missing {expected}");
            }
            assert!(!html.contains("rubicon-demo:products:v1"));
        }));
    }
    for task in tasks {
        task.await.unwrap();
    }
    renderer.shutdown().await.unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 8);
}
