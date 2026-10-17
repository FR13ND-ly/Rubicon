use std::{
    path::Path,
    sync::atomic::{AtomicUsize, Ordering},
};

use axum::{Json, Router, routing::get};
use rubicon::{Backend, JavaScriptRuntime, ServerConfig, backend_extension, serve};

#[cfg(windows)]
use rubicon::TimerResolutionGuard;

mod catalog;
mod shutdown;
mod warmup;

#[tokio::main]
async fn main() -> Result<(), String> {
    #[cfg(windows)]
    let _timer_resolution = TimerResolutionGuard::from_environment()?;

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
    let config = ServerConfig {
        bind: ([127, 0, 0, 1], 3000).into(),
        origin: "http://127.0.0.1:3000".to_owned(),
        server_entry: Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../dist/rubicon/server/server.mjs"),
        browser_dir: Path::new(env!("CARGO_MANIFEST_DIR")).join("../dist/rubicon/browser"),
        renderer: Default::default(),
    }
    .with_environment_overrides()?;

    let application_routes = Router::new().route("/api/products", get(products));
    let backend = Backend::new(application_routes.clone());

    let shutdown = shutdown::signal()?;
    let warmup_requests = warmup::requests_from_environment()?;
    let next_runtime_id = AtomicUsize::new(1);

    let server_result = serve(
        config,
        application_routes,
        move || {
            let mut extensions = vec![backend_extension(backend.clone(), 2 * 1024 * 1024)];

            if let Some(requests) = warmup_requests {
                let runtime_id = next_runtime_id.fetch_add(1, Ordering::Relaxed);
                extensions.push(warmup::extension(runtime_id, requests));
            }

            JavaScriptRuntime::with_extensions(extensions)
        },
        shutdown,
    )
    .await;

    #[cfg(feature = "render-timing")]
    {
        drop(timing_guard);

        eprintln!(
            "RUBICON_DIAGNOSTICS_DROPPED lines={}",
            timing_dropped.dropped_lines(),
        );
    }

    server_result?;

    Ok(())
}

async fn products() -> Json<Vec<catalog::Product>> {
    let products = catalog::list_products().await;

    Json(products)
}
