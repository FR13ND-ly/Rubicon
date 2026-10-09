use std::path::Path;

use axum::{Router, extract::State, http::StatusCode, response::Html, routing::get};
use rubicon::{RenderError, Renderer};

#[tokio::main]
async fn main() -> Result<(), String> {
    deno_core::JsRuntime::init_platform(None);

    let module_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("js/render.mjs");

    let renderer = Renderer::start(module_path).await?;

    let app = Router::new().route("/", get(root)).with_state(renderer);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .map_err(|error| format!("HTTP listener creation failed: {error}"))?;

    println!("Rubicon is running at http://127.0.0.1:3000");

    axum::serve(listener, app)
        .await
        .map_err(|error| format!("HTTP server failed: {error}"))?;

    Ok(())
}

async fn root(
    State(renderer): State<Renderer>,
) -> Result<Html<String>, (StatusCode, &'static str)> {
    match renderer.render().await {
        Ok(html) => Ok(Html(html)),

        Err(RenderError::Busy) => Err((StatusCode::SERVICE_UNAVAILABLE, "Server busy. Try again.")),

        Err(RenderError::Unavailable) => {
            Err((StatusCode::SERVICE_UNAVAILABLE, "Renderer unavailable."))
        }

        Err(RenderError::Failed(error)) => {
            eprintln!("Rendering failed: {error}");

            Err((StatusCode::INTERNAL_SERVER_ERROR, "Rendering failed."))
        }
    }
}
