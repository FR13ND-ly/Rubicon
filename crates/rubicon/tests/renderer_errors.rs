use std::{
    path::Path,
    time::{Duration, Instant},
};

use rubicon::{JavaScriptRuntime, RenderError, RenderRequest, RenderResponse, Renderer};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), String> {
    deno_core::JsRuntime::init_platform(None);

    for fixture_name in [
        "renderer_errors.mjs",
        "renderer_async.mjs",
        "renderer_timers.mjs",
        "renderer_streams.mjs",
        "renderer_fetch_objects.mjs",
        "renderer_performance.mjs",
    ] {
        check_renderer_fixture(fixture_name, JavaScriptRuntime::new).await?;
    }

    check_renderer_idle_timers().await?;

    Ok(())
}

async fn check_renderer_idle_timers() -> Result<(), String> {
    let module_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/renderer_idle_timers.mjs");

    let renderer = tokio::time::timeout(Duration::from_secs(5), Renderer::start(module_path))
        .await
        .expect("Worker initialization exceeded the test time limit")?;

    let first = render(
        &renderer,
        "idle-start",
        "GET",
        "http://127.0.0.1:3000/start",
    )
    .await
    .expect("Starting background timers failed");

    assert_eq!(first.status, 200);
    assert_eq!(first.body, "scheduled");

    let idle_started = Instant::now();

    tokio::time::sleep(Duration::from_millis(500)).await;

    let idle_ms = idle_started.elapsed().as_secs_f64() * 1000.0;

    let inspected = render(
        &renderer,
        "idle-inspect",
        "GET",
        "http://127.0.0.1:3000/inspect",
    )
    .await
    .expect("An active interval must allow another render");

    assert_eq!(inspected.status, 200);

    let (cleanup, ticks) = inspected
        .body
        .as_text()
        .expect("Timer response should be UTF-8")
        .split_once('|')
        .expect("Invalid timer test response");

    let cleanup_ms: f64 = cleanup.parse().expect("The cleanup timer did not execute");

    assert!(
        cleanup_ms.is_finite() && cleanup_ms >= 0.0 && cleanup_ms < idle_ms,
        "Cleanup must execute before the second request: \
         cleanup_ms={cleanup_ms}, idle_ms={idle_ms}"
    );

    let ticks: u64 = ticks.parse().expect("Invalid interval tick count");

    assert!(ticks > 0, "The interval did not execute while idle");

    drop(renderer);

    println!("Renderer idle timers passed.");
    Ok(())
}

async fn check_renderer_fixture(
    fixture_name: &str,
    create_runtime: fn() -> Result<JavaScriptRuntime, String>,
) -> Result<(), String> {
    let module_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(fixture_name);

    let renderer = tokio::time::timeout(
        Duration::from_secs(5),
        Renderer::start_with_runtime(module_path, create_runtime),
    )
    .await
    .expect("Worker initialization exceeded the test time limit")?;

    let first_url = "http://127.0.0.1:3000/products?sort=asc&page=2";
    let second_url = "http://127.0.0.1:3000/search?q='quoted'&page=1";

    assert_response(
        render(&renderer, "9007199254740993", "GET", first_url)
            .await
            .expect("First render failed"),
        302,
        format!(
            "<p>Răspuns</p><p>Request ID: 9007199254740993</p><p>Render 1</p><p>Method: GET</p><p>URL: {first_url}</p>"
        ),
    );

    match render(&renderer, "test-2", "GET", second_url).await {
        Err(RenderError::Failed(message)) => {
            assert!(message.starts_with("Export execution failed:"));
            assert!(message.contains("intentional renderer test error"));
        }
        other => panic!("Expected a JavaScript execution error, got {other:?}"),
    }

    assert_response(
        render(&renderer, "test-3", "HEAD", second_url)
            .await
            .expect("Render after exception failed"),
        404,
        format!(
            "<p>Răspuns</p><p>Request ID: test-3</p><p>Render 3</p><p>Method: HEAD</p><p>URL: {second_url}</p>"
        ),
    );

    match render(&renderer, "test-4", "HEAD", first_url).await {
        Err(RenderError::Failed(message)) => {
            assert!(message.starts_with("Result conversion failed:"));
        }
        other => panic!("Expected a response conversion error, got {other:?}"),
    }

    assert_response(
        render(&renderer, "9007199254740995", "GET", first_url)
            .await
            .expect("Render after conversion error failed"),
        200,
        format!(
            "<p>Răspuns</p><p>Request ID: 9007199254740995</p><p>Render 5</p><p>Method: GET</p><p>URL: {first_url}</p>"
        ),
    );

    drop(renderer);

    println!("Renderer fixture passed: {fixture_name}");
    Ok(())
}

fn assert_response(response: RenderResponse, status: u16, body: String) {
    assert_eq!(response.status, status);
    assert_eq!(response.body, body);

    let mut expected_headers = vec![
        (
            "content-type".to_owned(),
            "text/html; charset=utf-8".to_owned(),
        ),
        ("set-cookie".to_owned(), "first=1; Path=/".to_owned()),
        ("set-cookie".to_owned(), "second=2; Path=/".to_owned()),
    ];

    if status == 302 {
        expected_headers.push(("location".to_owned(), "/products".to_owned()));
    }

    assert_eq!(response.headers, expected_headers);
}

async fn render(
    renderer: &Renderer,
    request_id: &str,
    method: &str,
    url: &str,
) -> Result<RenderResponse, RenderError> {
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
