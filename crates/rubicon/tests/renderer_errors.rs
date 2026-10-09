use std::{path::Path, time::Duration};

use rubicon::{RenderError, Renderer};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), String> {
    deno_core::JsRuntime::init_platform(None);

    let module_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/renderer_errors.mjs");

    let renderer = tokio::time::timeout(Duration::from_secs(5), Renderer::start(module_path))
        .await
        .expect("Worker initialization exceeded the test time limit")?;

    assert_eq!(
        render(&renderer).await.expect("First render failed"),
        "<p>Render 1</p>"
    );

    match render(&renderer).await {
        Err(RenderError::Failed(message)) => {
            assert!(message.starts_with("Export execution failed:"));
            assert!(message.contains("intentional renderer test error"));
        }
        other => {
            panic!("Expected a JavaScript execution error, got {other:?}")
        }
    }

    assert_eq!(
        render(&renderer)
            .await
            .expect("Render after exception failed"),
        "<p>Render 3</p>",
    );

    match render(&renderer).await {
        Err(RenderError::Failed(message)) => {
            assert!(message.starts_with("String conversion failed:"));
        }
        other => {
            panic!("Expected a string conversion error, got {other:?}")
        }
    }

    assert_eq!(
        render(&renderer)
            .await
            .expect("Render after conversion error failed"),
        "<p>Render 5</p>",
    );

    drop(renderer);

    println!("Renderer error checks passed.");
    Ok(())
}

async fn render(renderer: &Renderer) -> Result<String, RenderError> {
    tokio::time::timeout(Duration::from_secs(5), renderer.render())
        .await
        .expect("Rendering exceeded the test time limit")
}
