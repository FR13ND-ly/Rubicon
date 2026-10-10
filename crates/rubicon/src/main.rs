use std::path::Path;

use axum::{
    Router,
    extract::{OriginalUri, State},
    http::{HeaderMap, Method, StatusCode},
    response::Html,
    routing::get,
};
use rubicon::{RenderError, RenderRequest, Renderer};

const APPLICATION_ORIGIN: &str = "http://127.0.0.1:3000";

#[tokio::main]
async fn main() -> Result<(), String> {
    deno_core::JsRuntime::init_platform(None);

    let module_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("js/render.mjs");

    let renderer = Renderer::start(module_path).await?;

    let app = Router::new()
        .route("/", get(root))
        .route("/{*path}", get(root))
        .with_state(renderer);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .map_err(|error| format!("HTTP listener creation failed: {error}"))?;

    println!("Rubicon is running at {APPLICATION_ORIGIN}");

    axum::serve(listener, app)
        .await
        .map_err(|error| format!("HTTP server failed: {error}"))?;

    Ok(())
}

async fn root(
    State(renderer): State<Renderer>,
    OriginalUri(uri): OriginalUri,
    method: Method,
    headers: HeaderMap,
) -> Result<Html<String>, (StatusCode, &'static str)> {
    let path_and_query = uri
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or("/");

    let url = format!("{APPLICATION_ORIGIN}{path_and_query}");
    let request = RenderRequest {
        url,
        method: method.as_str().to_owned(),
        headers: request_headers(&headers)?,
    };

    match renderer.render(request).await {
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

fn request_headers(
    headers: &HeaderMap,
) -> Result<Vec<(String, String)>, (StatusCode, &'static str)> {
    let mut pairs = Vec::with_capacity(headers.len());

    for name in headers.keys() {
        for value in headers.get_all(name) {
            let value = value
                .to_str()
                .map_err(|_| (StatusCode::BAD_REQUEST, "Unsupported request header value."))?;

            pairs.push((name.as_str().to_owned(), value.to_owned()));
        }
    }

    Ok(pairs)
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, HeaderValue, StatusCode};

    use super::request_headers;

    #[test]
    fn request_headers_preserve_repeated_values() {
        let mut headers = HeaderMap::new();
        headers.append("x-rubicon-demo", HeaderValue::from_static("first"));
        headers.append("accept", HeaderValue::from_static("text/html"));
        headers.append("x-rubicon-demo", HeaderValue::from_static("second"));

        let pairs = request_headers(&headers).expect("Header conversion failed");
        let repeated: Vec<&str> = pairs
            .iter()
            .filter(|(name, _)| name == "x-rubicon-demo")
            .map(|(_, value)| value.as_str())
            .collect();

        assert_eq!(pairs.len(), 3);
        assert_eq!(repeated, vec!["first", "second"]);
        assert!(pairs.contains(&("accept".to_owned(), "text/html".to_owned())));
    }

    #[test]
    fn request_headers_reject_unsupported_text_values() {
        let mut headers = HeaderMap::new();
        headers.append(
            "x-rubicon-demo",
            HeaderValue::from_bytes(b"\xff").expect("Valid HTTP header bytes"),
        );

        assert_eq!(
            request_headers(&headers),
            Err((StatusCode::BAD_REQUEST, "Unsupported request header value.")),
        );
    }
}
