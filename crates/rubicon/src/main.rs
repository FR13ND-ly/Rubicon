use std::path::Path;

use axum::{Json, Router, routing::get};
use rubicon::{JavaScriptRuntime, ServerConfig, serve};

mod catalog;
mod catalog_native;

#[cfg(windows)]
mod windows_timer;

#[tokio::main]
async fn main() -> Result<(), String> {
    #[cfg(windows)]
    let _timer_resolution = windows_timer::TimerResolutionGuard::from_environment()?;

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
            .join("../../examples/angular-demo/dist/rubicon/server/server.mjs"),
        browser_dir: Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/angular-demo/dist/rubicon/browser"),
    };

    let application_routes = Router::new().route("/api/products", get(products));

    let server_result = serve(
        config,
        application_routes,
        || JavaScriptRuntime::with_extensions(vec![catalog_native::extension()]),
        async {
            tokio::signal::ctrl_c()
                .await
                .map_err(|error| format!("Shutdown signal failed: {error}"))
        },
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
