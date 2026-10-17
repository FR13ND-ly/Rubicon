#![cfg(feature = "streaming")]

use deno_core::futures::{StreamExt, poll};
use rubicon::{
    JavaScriptRuntime, RenderContent, RenderError, RenderRequest, RenderResponse, Renderer,
    RendererConfig, into_http_streaming_response,
};
use std::{path::Path, time::Duration};

async fn start(limit: usize) -> Renderer {
    Renderer::start_with_config(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/renderer_streaming.mjs"),
        RendererConfig {
            workers: 1,
            max_in_flight: limit,
            queue_capacity: 16,
        },
        JavaScriptRuntime::new,
    )
    .await
    .unwrap()
}

fn request(path: &str) -> RenderRequest {
    RenderRequest {
        request_id: path.to_owned(),
        url: format!("http://localhost{path}"),
        method: "GET".to_owned(),
        headers: Vec::new(),
    }
}

async fn streamed(renderer: &Renderer, path: &str) -> RenderResponse<RenderContent> {
    tokio::time::timeout(
        Duration::from_secs(5),
        renderer.render_stream(request(path)),
    )
    .await
    .unwrap()
    .unwrap()
}

async fn text(renderer: &Renderer, path: &str) -> String {
    tokio::time::timeout(Duration::from_secs(5), renderer.render(request(path)))
        .await
        .unwrap()
        .unwrap()
        .body
        .as_text()
        .unwrap()
        .to_owned()
}

async fn stop(renderer: &Renderer) {
    tokio::time::timeout(Duration::from_secs(5), renderer.shutdown())
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn headers_and_first_chunk_arrive_before_the_tail_and_cookies_survive() {
    let renderer = start(2).await;
    let response = streamed(&renderer, "/delayed").await;
    assert_eq!(response.headers.len(), 2);
    let RenderContent::Stream(mut stream) = response.body else {
        panic!("Expected stream");
    };
    assert_eq!(stream.next().await.unwrap().unwrap(), "<p>țară ".as_bytes());
    let mut tail = Box::pin(stream.next());
    assert!(poll!(tail.as_mut()).is_pending());
    assert_eq!(text(&renderer, "/release").await, "released");
    assert_eq!(tail.await.unwrap().unwrap(), "🌍</p>".as_bytes());
    assert!(stream.next().await.is_none());
    stop(&renderer).await;
}

#[tokio::test(flavor = "current_thread")]
async fn backpressure_bounds_production_and_drop_cancels_the_reader() {
    let renderer = start(2).await;
    let response = streamed(&renderer, "/many").await;
    let state = text(&renderer, "/state").await;
    let produced: usize = state.split('|').next().unwrap().parse().unwrap();
    assert!(
        produced <= 3,
        "Only two queued chunks and one pending write are allowed: {state}"
    );
    drop(response);
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if text(&renderer, "/state").await.split('|').nth(1).unwrap() == "1" {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    stop(&renderer).await;
}

#[tokio::test(flavor = "current_thread")]
async fn streams_keep_the_admission_slot_and_shutdown_waits_for_the_consumer() {
    for path in ["/many", "/pending", "/pending-cancel"] {
        let renderer = start(1).await;
        let response = streamed(&renderer, path).await;
        let mut queued = Box::pin(renderer.render(request("/buffered")));
        assert!(poll!(queued.as_mut()).is_pending());
        let mut shutdown = Box::pin(renderer.shutdown());
        assert!(poll!(shutdown.as_mut()).is_pending());
        drop(shutdown);
        drop(response);
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(5), queued)
                .await
                .unwrap()
                .unwrap()
                .body,
            "healthy"
        );
        stop(&renderer).await;
    }
}

#[tokio::test(flavor = "current_thread")]
async fn empty_streams_finish_and_errors_do_not_poison_the_worker() {
    let renderer = start(2).await;
    assert!(text(&renderer, "/empty").await.is_empty());
    assert_eq!(text(&renderer, "/getter-body").await, "once");
    assert_eq!(text(&renderer, "/state").await.split('|').nth(2), Some("1"));
    let large = renderer.render(request("/large")).await.unwrap();
    assert_eq!(large.body.as_bytes().len(), 200000);
    assert!(large.body.as_bytes().iter().all(|byte| *byte == 65));
    for path in ["/error", "/invalid-chunk", "/throw-body", "/locked"] {
        let error = tokio::time::timeout(Duration::from_secs(5), renderer.render(request(path)))
            .await
            .unwrap()
            .unwrap_err();
        assert!(matches!(error, RenderError::Failed(_)));
        assert_eq!(text(&renderer, "/buffered").await, "healthy");
    }
    for path in ["/bodyless", "/invalid-headers"] {
        assert!(into_http_streaming_response(streamed(&renderer, path).await).is_err());
    }
    stop(&renderer).await;
}

#[tokio::test(flavor = "current_thread")]
async fn abandoned_buffered_responses_are_not_deserialized() {
    for path in ["/cancelled-response", "/cancelled-stream-response"] {
        let renderer = start(2).await;
        let call = {
            let renderer = renderer.clone();
            tokio::spawn(async move { renderer.render(request(path)).await })
        };
        while !text(&renderer, "/state").await.ends_with("true") {
            tokio::task::yield_now().await;
        }
        call.abort();
        assert!(call.await.unwrap_err().is_cancelled());
        assert_eq!(text(&renderer, "/release").await, "released");
        if path == "/cancelled-response" {
            assert_eq!(text(&renderer, "/state").await.split('|').nth(2), Some("0"));
        } else {
            tokio::time::timeout(Duration::from_secs(5), async {
                while text(&renderer, "/state").await.split('|').nth(1) != Some("1") {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
        }
        stop(&renderer).await;
    }
}

#[tokio::test(flavor = "current_thread")]
async fn fatal_worker_errors_fail_the_stream_instead_of_returning_clean_eof() {
    let renderer = start(1).await;
    let response = streamed(&renderer, "/fatal").await;
    let RenderContent::Stream(mut stream) = response.body else {
        panic!("Expected stream");
    };
    assert!(
        tokio::time::timeout(Duration::from_secs(5), stream.next())
            .await
            .unwrap()
            .unwrap()
            .is_err()
    );
    stop(&renderer).await;
}
