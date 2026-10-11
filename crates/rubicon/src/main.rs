use std::{
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

use axum::{
    Router,
    extract::{OriginalUri, State},
    http::{HeaderMap, Method, StatusCode},
    response::Response,
    routing::get,
};
use rubicon::{RenderError, RenderRequest, Renderer, into_http_response};
use tower_http::services::ServeDir;

const APPLICATION_ORIGIN: &str = "http://127.0.0.1:3000";
static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

#[tokio::main]
async fn main() -> Result<(), String> {
    #[cfg(feature = "render-timing")]
    let (timing_guard, timing_dropped) = {
        let (writer, guard) = tracing_appender::non_blocking::NonBlockingBuilder::default()
            .buffered_lines_limit(1024)
            .lossy(true)
            .thread_name("rubicon-diagnostics")
            .finish(std::io::stderr());

        let dropped = writer.error_counter();

        tracing_subscriber::fmt()
            .with_writer(writer)
            .with_ansi(false)
            .with_target(false)
            .with_max_level(tracing::Level::INFO)
            .try_init()
            .map_err(|error| format!("Diagnostic logging initialization failed: {error}"))?;

        (guard, dropped)
    };
    deno_core::JsRuntime::init_platform(None);

    let module_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/angular-demo/dist/rubicon/server/server.mjs");

    let renderer = Renderer::start(module_path).await?;

    let browser_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/angular-demo/dist/rubicon/browser");

    let ssr = Router::new()
        .route("/", get(root))
        .route("/{*path}", get(root))
        .with_state(renderer);

    let app = Router::new().fallback_service(
        ServeDir::new(browser_path)
            .append_index_html_on_directories(false)
            .fallback(ssr),
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .map_err(|error| format!("HTTP listener creation failed: {error}"))?;

    println!("Rubicon is running at {APPLICATION_ORIGIN}");

    let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel::<()>();

    let server = axum::serve(listener, app).with_graceful_shutdown(async move {
        let _ = shutdown_receiver.await;
    });

    let server = std::future::IntoFuture::into_future(server);
    tokio::pin!(server);

    let server_result = tokio::select! {
        result = &mut server => result,

        signal = tokio::signal::ctrl_c() => {
            signal.map_err(|error| {
                format!("Shutdown signal failed: {error}")
            })?;

            let _ = shutdown_sender.send(());
            server.await
        }
    };

    #[cfg(feature = "render-timing")]
    {
        drop(timing_guard);

        eprintln!(
            "RUBICON_DIAGNOSTICS_DROPPED lines={}",
            timing_dropped.dropped_lines(),
        );
    }

    server_result.map_err(|error| format!("HTTP server failed: {error}"))?;

    Ok(())
}

async fn root(
    State(renderer): State<Renderer>,
    OriginalUri(uri): OriginalUri,
    method: Method,
    headers: HeaderMap,
) -> Result<Response, (StatusCode, &'static str)> {
    let path_and_query = uri
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or("/");

    let url = format!("{APPLICATION_ORIGIN}{path_and_query}");
    let request_id = next_request_id(&NEXT_REQUEST_ID)?;

    let request = RenderRequest {
        request_id: request_id.clone(),
        url,
        method: method.as_str().to_owned(),
        headers: request_headers(&headers)?,
    };

    match renderer.render(request).await {
        Ok(rendered) => into_http_response(rendered).map_err(|error| {
            eprintln!("Response conversion failed [request_id={request_id}]: {error}");

            (StatusCode::INTERNAL_SERVER_ERROR, "Rendering failed.")
        }),

        Err(RenderError::Busy) => Err((StatusCode::SERVICE_UNAVAILABLE, "Server busy. Try again.")),

        Err(RenderError::Unavailable) => {
            Err((StatusCode::SERVICE_UNAVAILABLE, "Renderer unavailable."))
        }

        Err(RenderError::Failed(error)) => {
            eprintln!("Rendering failed [request_id={request_id}]: {error}");

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

fn next_request_id(counter: &AtomicU64) -> Result<String, (StatusCode, &'static str)> {
    counter
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1)
        })
        .map(|value| value.to_string())
        .map_err(|_| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                "Request identifiers exhausted.",
            )
        })
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashSet,
        sync::atomic::{AtomicU64, Ordering},
    };

    use axum::http::{HeaderMap, HeaderValue, StatusCode};

    use super::{next_request_id, request_headers};

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

    #[test]
    fn request_ids_are_unique_across_threads() {
        let counter = AtomicU64::new(1);

        let identifiers = std::thread::scope(|scope| {
            let workers: Vec<_> = (0..4)
                .map(|_| {
                    scope.spawn(|| {
                        (0..64)
                            .map(|_| next_request_id(&counter).expect("ID generation failed"))
                            .collect::<Vec<_>>()
                    })
                })
                .collect();

            workers
                .into_iter()
                .flat_map(|worker| worker.join().expect("ID worker panicked"))
                .collect::<HashSet<_>>()
        });

        assert_eq!(identifiers.len(), 256);
    }

    #[test]
    fn request_ids_do_not_wrap_after_exhaustion() {
        let counter = AtomicU64::new(u64::MAX - 1);

        assert_eq!(
            next_request_id(&counter).expect("Last available ID failed"),
            (u64::MAX - 1).to_string(),
        );

        for _ in 0..2 {
            assert_eq!(
                next_request_id(&counter),
                Err((
                    StatusCode::SERVICE_UNAVAILABLE,
                    "Request identifiers exhausted."
                )),
            );
        }

        assert_eq!(counter.load(Ordering::Relaxed), u64::MAX);
    }
}
