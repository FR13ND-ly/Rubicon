use deno_core::futures::future::join_all;
use rubicon::{
    JavaScriptRuntime, RenderError, RenderRequest, RenderResponse, Renderer, RendererConfig,
};
use std::{path::Path, time::Duration};

#[cfg(feature = "render-timing")]
#[path = "support/timing_capture.rs"]
mod timing_capture;

async fn call(renderer: &Renderer, id: &str, path: &str) -> Result<RenderResponse, RenderError> {
    tokio::time::timeout(
        Duration::from_secs(5),
        renderer.render(RenderRequest {
            request_id: id.to_owned(),
            url: format!("http://127.0.0.1:3000{path}"),
            method: "GET".to_owned(),
            headers: vec![("x-marker".to_owned(), id.to_owned())],
        }),
    )
    .await
    .expect("Concurrent request timed out")
}
fn spawn(
    renderer: &Renderer,
    id: &str,
    path: &str,
) -> tokio::task::JoinHandle<Result<RenderResponse, RenderError>> {
    let renderer = renderer.clone();
    let id = id.to_owned();
    let path = path.to_owned();
    tokio::spawn(async move { call(&renderer, &id, &path).await })
}
async fn state(renderer: &Renderer) -> (usize, usize) {
    let response = call(renderer, "state", "/state").await.unwrap();
    let (active, maximum) = response.body.split_once('|').unwrap();
    (active.parse().unwrap(), maximum.parse().unwrap())
}
async fn wait_active(renderer: &Renderer, expected: usize) {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if state(renderer).await.0 == expected {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("Expected number of independent calls did not start");
}
async fn release(renderer: &Renderer, key: &str) {
    assert_eq!(
        call(renderer, "release", &format!("/release?key={key}"))
            .await
            .unwrap()
            .body,
        "released"
    );
}
fn assert_echo(response: RenderResponse, id: &str, path: &str) {
    assert_eq!(
        response.body,
        format!("{id}|http://127.0.0.1:3000{path}|{id}")
    );
    assert_eq!(
        response.headers,
        vec![
            ("x-request-id".to_owned(), id.to_owned()),
            ("set-cookie".to_owned(), format!("marker={id}")),
        ]
    );
}
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), String> {
    #[cfg(feature = "render-timing")]
    let timing = {
        let capture = timing_capture::TimingCapture::default();
        tracing::subscriber::set_global_default(capture.clone()).unwrap();
        capture
    };
    deno_core::JsRuntime::init_platform(None);
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/renderer_concurrent.mjs");
    let renderer = Renderer::start(fixture.clone()).await?;
    let slow = spawn(&renderer, "slow-A", "/gated?key=A");
    wait_active(&renderer, 1).await;
    let faster = spawn(&renderer, "fast-B", "/gated?key=B");
    wait_active(&renderer, 2).await;
    release(&renderer, "B").await;
    assert_echo(faster.await.unwrap().unwrap(), "fast-B", "/gated?key=B");
    assert!(
        !slow.is_finished(),
        "A slow request must still be pending when B finishes"
    );
    #[cfg(feature = "render-timing")]
    {
        assert_eq!(timing.stages("fast-B"), ["angular_handle", "body_read"]);
        assert!(timing.stages("slow-A").is_empty());
    }
    println!("PASS: independent out-of-order completion, URL, ID and headers");

    let rejected = spawn(&renderer, "reject", "/gated-reject?key=reject");
    wait_active(&renderer, 2).await;
    release(&renderer, "reject").await;
    assert!(
        matches!(rejected.await.unwrap(), Err(RenderError::Failed(message)) if message.contains("intentional independent rejection"))
    );
    assert!(!slow.is_finished());
    assert_echo(
        call(&renderer, "healthy", "/echo?x=1").await.unwrap(),
        "healthy",
        "/echo?x=1",
    );
    let invalid = spawn(&renderer, "invalid", "/gated-invalid?key=invalid");
    wait_active(&renderer, 2).await;
    release(&renderer, "invalid").await;
    assert!(
        matches!(invalid.await.unwrap(), Err(RenderError::Failed(message)) if message.starts_with("Result conversion failed:"))
    );
    assert!(!slow.is_finished());
    println!("PASS: a rejection or conversion failure leaves other calls alive");

    let cancelled = spawn(&renderer, "cancelled", "/gated-reject?key=cancel");
    wait_active(&renderer, 2).await;
    cancelled.abort();
    assert!(cancelled.await.unwrap_err().is_cancelled());
    release(&renderer, "cancel").await;
    wait_active(&renderer, 1).await;
    release(&renderer, "A").await;
    assert_echo(slow.await.unwrap().unwrap(), "slow-A", "/gated?key=A");
    #[cfg(feature = "render-timing")]
    {
        assert_eq!(timing.stages("slow-A"), ["angular_handle", "body_read"]);
        assert_eq!(timing.stages("fast-B"), ["angular_handle", "body_read"]);
        println!("PASS: adapter timing retains request IDs across interleaved promises");
    }
    assert_echo(
        call(&renderer, "after-cancel", "/echo").await.unwrap(),
        "after-cancel",
        "/echo",
    );
    println!("PASS: dropped consumer and later rejection do not poison the runtime");

    assert!(
        matches!(call(&renderer, "never", "/never").await, Err(RenderError::Failed(message)) if message.contains("event loop has already resolved"))
    );
    assert!(
        matches!(call(&renderer, "sync-error", "/sync-error").await, Err(RenderError::Failed(message)) if message.contains("intentional synchronous error"))
    );
    assert_echo(
        call(&renderer, "after-never", "/echo").await.unwrap(),
        "after-never",
        "/echo",
    );
    println!("PASS: quiescent promise and synchronous exception return request errors");

    let tasks = (0..64).map(|index| {
        let renderer = renderer.clone();
        async move {
            let id = format!("burst-{index}");
            let result = call(&renderer, &id, "/delay").await;
            if let Ok(response) = &result {
                assert_eq!(
                    response.body,
                    format!("{id}|http://127.0.0.1:3000/delay|{id}")
                );
            }
            result
        }
    });
    let results = join_all(tasks).await;
    assert!(
        results
            .iter()
            .any(|result| matches!(result, Err(RenderError::Busy))),
        "Overflow should reject rather than queue indefinitely"
    );
    assert!(
        results
            .iter()
            .all(|result| result.is_ok() || matches!(result, Err(RenderError::Busy)))
    );
    let (_, maximum) = state(&renderer).await;
    assert!(
        maximum > 1 && maximum <= 16,
        "Unexpected active count: {maximum}"
    );
    println!("PASS: maximum {maximum} active renders, bounded queue, overload rejection");
    drop(renderer);

    for limit in [1, 4, 16] {
        let configured = Renderer::start_with_config(
            fixture.clone(),
            RendererConfig {
                max_in_flight: limit,
                queue_capacity: 64,
            },
            JavaScriptRuntime::new,
        )
        .await?;
        let calls = (0..32).map(|index| {
            let id = format!("limit-{limit}-{index}");
            let configured = configured.clone();
            async move {
                assert_echo(
                    call(&configured, &id, "/delay").await.unwrap(),
                    &id,
                    "/delay",
                );
            }
        });
        join_all(calls).await;
        assert_eq!(state(&configured).await, (0, limit));
        drop(configured);
    }
    println!("PASS: configured limits 1, 4 and 16, with independent responses");

    let small_queue = Renderer::start_with_config(
        fixture.clone(),
        RendererConfig {
            max_in_flight: 1,
            queue_capacity: 3,
        },
        JavaScriptRuntime::new,
    )
    .await?;
    let burst = (0..32).map(|index| {
        let renderer = small_queue.clone();
        async move { call(&renderer, &format!("small-{index}"), "/delay").await }
    });
    let results = join_all(burst).await;
    let admitted = results.iter().filter(|result| result.is_ok()).count();
    assert!(
        (1..=4).contains(&admitted),
        "Unexpected admission count: {admitted}"
    );
    assert!(
        results
            .iter()
            .all(|result| result.is_ok() || matches!(result, Err(RenderError::Busy)))
    );
    assert_eq!(state(&small_queue).await.1, 1);
    drop(small_queue);
    println!("PASS: configured queue holds only three waiting requests");

    let fatal_renderer = Renderer::start(fixture).await?;
    let waiting = spawn(&fatal_renderer, "fatal-waiter", "/gated?key=fatal-waiter");
    wait_active(&fatal_renderer, 1).await;
    assert_eq!(
        call(&fatal_renderer, "fatal", "/fatal").await.unwrap().body,
        "scheduled"
    );
    assert!(matches!(
        waiting.await.unwrap(),
        Err(RenderError::Unavailable)
    ));
    assert!(matches!(
        call(&fatal_renderer, "after-fatal", "/echo").await,
        Err(RenderError::Unavailable)
    ));
    println!("PASS: fatal background error closes the worker and notifies waiting callers");
    Ok(())
}
