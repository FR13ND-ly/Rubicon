use std::{path::Path, time::Duration};

use rubicon::{RenderError, RenderRequest, Renderer};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), String> {
    deno_core::JsRuntime::init_platform(None);

    let module_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/renderer_errors.mjs");

    let renderer = tokio::time::timeout(Duration::from_secs(5), Renderer::start(module_path))
        .await
        .expect("Worker initialization exceeded the test time limit")?;

    let first_url = "http://127.0.0.1:3000/products?sort=asc&page=2";
    let second_url = "http://127.0.0.1:3000/search?q='quoted'&page=1";

    assert_eq!(
        render(&renderer, "9007199254740993", "GET", first_url)
            .await
            .expect("First render failed"),
        format!(
            "<p>Request ID: 9007199254740993</p><p>Render 1</p><p>Method: GET</p><p>URL: {first_url}</p>"
        ),
    );

    match render(&renderer, "test-2", "GET", second_url).await {
        Err(RenderError::Failed(message)) => {
            assert!(message.starts_with("Export execution failed:"));
            assert!(message.contains("intentional renderer test error"));
        }
        other => panic!("Expected a JavaScript execution error, got {other:?}"),
    }

    assert_eq!(
        render(&renderer, "test-3", "HEAD", second_url)
            .await
            .expect("Render after exception failed"),
        format!(
            "<p>Request ID: test-3</p><p>Render 3</p><p>Method: HEAD</p><p>URL: {second_url}</p>"
        ),
    );

    match render(&renderer, "test-4", "HEAD", first_url).await {
        Err(RenderError::Failed(message)) => {
            assert!(message.starts_with("String conversion failed:"));
        }
        other => panic!("Expected a string conversion error, got {other:?}"),
    }

    assert_eq!(
        render(&renderer, "9007199254740995", "GET", first_url)
            .await
            .expect("Render after conversion error failed"),
        format!(
            "<p>Request ID: 9007199254740995</p><p>Render 5</p><p>Method: GET</p><p>URL: {first_url}</p>"
        ),
    );

    drop(renderer);

    println!("Renderer request ID and error checks passed.");
    Ok(())
}

async fn render(
    renderer: &Renderer,
    request_id: &str,
    method: &str,
    url: &str,
) -> Result<String, RenderError> {
    let request = RenderRequest {
        request_id: request_id.to_owned(),
        url: url.to_owned(),
        method: method.to_owned(),
        headers: Vec::new(),
    };

    tokio::time::timeout(Duration::from_secs(5), renderer.render(request))
        .await
        .expect("Rendering exceeded the test time limit")
}
