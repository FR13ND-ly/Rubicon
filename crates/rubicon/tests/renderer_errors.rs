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
        render(&renderer, first_url)
            .await
            .expect("First render failed"),
        format!("<p>Render 1</p><p>URL: {first_url}</p>"),
    );

    match render(&renderer, second_url).await {
        Err(RenderError::Failed(message)) => {
            assert!(message.starts_with("Export execution failed:"));
            assert!(message.contains("intentional renderer test error"));
        }
        other => {
            panic!("Expected a JavaScript execution error, got {other:?}")
        }
    }

    assert_eq!(
        render(&renderer, second_url)
            .await
            .expect("Render after exception failed"),
        format!("<p>Render 3</p><p>URL: {second_url}</p>"),
    );

    match render(&renderer, first_url).await {
        Err(RenderError::Failed(message)) => {
            assert!(message.starts_with("String conversion failed:"));
        }
        other => {
            panic!("Expected a string conversion error, got {other:?}")
        }
    }

    assert_eq!(
        render(&renderer, first_url)
            .await
            .expect("Render after conversion error failed"),
        format!("<p>Render 5</p><p>URL: {first_url}</p>"),
    );

    drop(renderer);

    println!("Renderer URL and error checks passed.");
    Ok(())
}

async fn render(renderer: &Renderer, url: &str) -> Result<String, RenderError> {
    let request = RenderRequest {
        url: url.to_owned(),
    };

    tokio::time::timeout(Duration::from_secs(5), renderer.render(request))
        .await
        .expect("Rendering exceeded the test time limit")
}
