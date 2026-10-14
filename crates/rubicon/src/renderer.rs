use async_channel::{Receiver, Sender, TrySendError};
use deno_core::{
    futures::{
        FutureExt, Stream, StreamExt,
        future::{BoxFuture, Shared},
        stream::FuturesUnordered,
    },
    v8,
};
use std::{future::poll_fn, path::PathBuf, sync::Arc, task::Poll, thread};
use tokio::sync::oneshot;

use crate::{JavaScriptRuntime, PendingExport, RenderRequest, RenderResponse, RendererConfig};

struct RenderJob {
    request: RenderRequest,
    reply: oneshot::Sender<Result<RenderResponse, String>>,

    #[cfg(feature = "render-timing")]
    queued_at: std::time::Instant,
}

struct PoolLifetime {
    receiver: Receiver<RenderJob>,
}

impl Drop for PoolLifetime {
    fn drop(&mut self) {
        self.receiver.close();

        while let Ok(job) = self.receiver.try_recv() {
            drop(job);
        }
    }
}

async fn join_workers(workers: Vec<thread::JoinHandle<()>>) -> Result<(), String> {
    if workers.is_empty() {
        return Ok(());
    }

    tokio::task::spawn_blocking(move || {
        let mut panicked = false;

        for worker in workers {
            if worker.join().is_err() {
                panicked = true;
            }
        }

        if panicked {
            Err("A renderer worker panicked while shutting down.".to_owned())
        } else {
            Ok(())
        }
    })
    .await
    .map_err(|error| format!("Worker cleanup task failed: {error}"))?
}

#[derive(Clone)]
pub struct Renderer {
    sender: Sender<RenderJob>,
    completion: Shared<BoxFuture<'static, Result<(), String>>>,
}

#[derive(Debug)]
pub enum RenderError {
    Busy,
    Unavailable,
    Failed(String),
}

#[cfg(feature = "render-timing")]
fn record_render_timing(job: &RenderJob, started: std::time::Instant, ok: bool) {
    tracing::info!(
        target: "rubicon::render_timing",
        request_id = %job.request.request_id,
        queue_us = started.duration_since(job.queued_at).as_micros(),
        call_export_us = started.elapsed().as_micros(),
        ok,
        "render_timing",
    );
}

impl Renderer {
    fn from_workers(sender: Sender<RenderJob>, workers: Vec<thread::JoinHandle<()>>) -> Self {
        Self {
            sender,
            completion: join_workers(workers).boxed().shared(),
        }
    }

    pub async fn start(module_path: PathBuf) -> Result<Self, String> {
        Self::start_with_runtime(module_path, JavaScriptRuntime::new).await
    }

    pub async fn start_with_runtime<F>(
        module_path: PathBuf,
        create_runtime: F,
    ) -> Result<Self, String>
    where
        F: Fn() -> Result<JavaScriptRuntime, String> + Send + Sync + 'static,
    {
        Self::start_with_config(module_path, RendererConfig::default(), create_runtime).await
    }

    pub async fn start_with_config<F>(
        module_path: PathBuf,
        config: RendererConfig,
        create_runtime: F,
    ) -> Result<Self, String>
    where
        F: Fn() -> Result<JavaScriptRuntime, String> + Send + Sync + 'static,
    {
        config.validate()?;
        deno_core::JsRuntime::init_platform(None);

        let (sender, receiver) = async_channel::bounded(config.queue_capacity);

        let create_runtime = Arc::new(create_runtime);
        let pool_lifetime = Arc::new(PoolLifetime {
            receiver: receiver.clone(),
        });

        let mut workers = Vec::new();
        let mut ready = FuturesUnordered::new();
        let max_in_flight = config.max_in_flight;

        for worker_id in 1..=config.workers {
            let worker_module = module_path.clone();
            let worker_receiver = receiver.clone();
            let worker_factory = Arc::clone(&create_runtime);
            let worker_lifetime = Arc::clone(&pool_lifetime);
            let (ready_sender, ready_receiver) = oneshot::channel();

            let worker = thread::Builder::new()
                .name(format!("rubicon-renderer-{worker_id}"))
                .spawn(move || {
                    run_worker(
                        worker_id,
                        worker_module,
                        worker_receiver,
                        ready_sender,
                        worker_factory,
                        max_in_flight,
                        worker_lifetime,
                    )
                });

            match worker {
                Ok(worker) => workers.push(worker),
                Err(error) => {
                    let error = format!("Worker {worker_id} thread creation failed: {error}");

                    sender.close();
                    ready.clear();
                    drop(receiver);
                    drop(pool_lifetime);

                    join_workers(workers)
                        .await
                        .map_err(|cleanup| format!("{error}; {cleanup}"))?;

                    return Err(error);
                }
            }

            ready.push(async move { (worker_id, ready_receiver.await) });
        }

        drop(receiver);

        while let Some((worker_id, result)) = ready.next().await {
            let error = match result {
                Ok(Ok(())) => continue,
                Ok(Err(error)) => error,
                Err(_) => "Worker stopped during initialization".to_owned(),
            };

            let error = format!("Worker {worker_id} initialization failed: {error}");

            sender.close();
            ready.clear();
            drop(pool_lifetime);

            join_workers(workers)
                .await
                .map_err(|cleanup| format!("{error}; {cleanup}"))?;

            return Err(error);
        }

        drop(pool_lifetime);

        if sender.is_closed() {
            let error = "All renderer workers stopped during initialization.".to_owned();

            join_workers(workers)
                .await
                .map_err(|cleanup| format!("{error}; {cleanup}"))?;

            return Err(error);
        }

        Ok(Self::from_workers(sender, workers))
    }

    pub async fn render(&self, request: RenderRequest) -> Result<RenderResponse, RenderError> {
        let (reply, response) = oneshot::channel();

        self.sender
            .try_send(RenderJob {
                request,
                reply,

                #[cfg(feature = "render-timing")]
                queued_at: std::time::Instant::now(),
            })
            .map_err(|error| match error {
                TrySendError::Full(_) => RenderError::Busy,
                TrySendError::Closed(_) => RenderError::Unavailable,
            })?;

        response
            .await
            .map_err(|_| RenderError::Unavailable)?
            .map_err(RenderError::Failed)
    }

    pub async fn shutdown(&self) -> Result<(), String> {
        self.sender.close();
        self.completion.clone().await
    }
}

fn run_worker<F>(
    worker_id: usize,
    module_path: PathBuf,
    receiver: Receiver<RenderJob>,
    mut ready: oneshot::Sender<Result<(), String>>,
    create_runtime: Arc<F>,
    max_in_flight: usize,
    lifetime: Arc<PoolLifetime>,
) where
    F: Fn() -> Result<JavaScriptRuntime, String> + Send + Sync + 'static,
{
    let executor = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(executor) => executor,
        Err(error) => {
            let _ = ready.send(Err(format!("Worker executor creation failed: {error}")));
            return;
        }
    };

    executor.block_on(async move {
        tokio::pin!(receiver);
        let initialization = tokio::select! {
            biased;

            _ = ready.closed() => return,

            result = async {
                let mut runtime = create_runtime()?;
                let module_id = runtime.load_main_module(&module_path).await?;

                Ok::<_, String>((runtime, module_id))
            } => result,
        };

        let (mut runtime, module_id) = match initialization {
            Ok(initialized) => initialized,
            Err(error) => {
                let _ = ready.send(Err(error));
                return;
            }
        };

        if ready.send(Ok(())).is_err() {
            return;
        }
        struct ActiveCall {
            job: RenderJob,
            pending: PendingExport,
            #[cfg(feature = "render-timing")]
            started: std::time::Instant,
            #[cfg(feature = "render-timing")]
            span: tracing::Span,
        }
        enum Event {
            Job(RenderJob),
            Completed(usize, Result<v8::Global<v8::Value>, String>),
            Closed,
            Quiescent,
            Failed(String),
        }
        fn poll_completed(active: &mut [ActiveCall], cx: &mut std::task::Context<'_>) -> Option<Event> {
            for (index, call) in active.iter_mut().enumerate() {
                #[cfg(feature = "render-timing")]
                let state = call.span.in_scope(|| call.pending.as_mut().poll(cx));
                #[cfg(not(feature = "render-timing"))]
                let state = call.pending.as_mut().poll(cx);
                if let Poll::Ready(result) = state {
                    return Some(Event::Completed(index, result));
                }
            }
            None
        }
        let mut active = Vec::<ActiveCall>::new();
        let mut closed = false;
        loop {
            if closed && active.is_empty() {
                break;
            }
            let event = poll_fn(|cx| {
                if let Some(event) = poll_completed(&mut active, cx) {
                    return Poll::Ready(event);
                }
                let event_loop = runtime.poll_event_loop(cx);
                if let Poll::Ready(Err(error)) = &event_loop {
                    return Poll::Ready(Event::Failed(error.clone()));
                }
                if let Some(event) = poll_completed(&mut active, cx) {
                    return Poll::Ready(event);
                }
                if matches!(event_loop, Poll::Ready(Ok(()))) && !active.is_empty() {
                    return Poll::Ready(Event::Quiescent);
                }
                if !closed && active.len() < max_in_flight {
                    match receiver.as_mut().poll_next(cx) {
                        Poll::Ready(Some(job)) => return Poll::Ready(Event::Job(job)),
                        Poll::Ready(None) => return Poll::Ready(Event::Closed),
                        Poll::Pending => {}
                    }
                }
                Poll::Pending
            }).await;
            match event {
                Event::Job(job) => {
                    if job.reply.is_closed() { continue; }
                    #[cfg(feature = "render-timing")]
                    let span = tracing::info_span!(
                        target: "rubicon::runtime_timing", "render",
                        request_id = %job.request.request_id,
                    );
                    #[cfg(feature = "render-timing")]
                    let started = std::time::Instant::now();
                    #[cfg(feature = "render-timing")]
                    let launched = span.in_scope(|| runtime.begin_export(module_id, "render", &job.request));
                    #[cfg(not(feature = "render-timing"))]
                    let launched = runtime.begin_export(module_id, "render", &job.request);
                    match launched {
                        Ok(pending) => active.push(ActiveCall {
                            job, pending,
                            #[cfg(feature = "render-timing")]
                            started,
                            #[cfg(feature = "render-timing")]
                            span,
                        }),
                        Err(error) => {
                            #[cfg(feature = "render-timing")]
                            record_render_timing(&job, started, false);
                            let _ = job.reply.send(Err(error));
                        }
                    }
                }
                Event::Completed(index, result) => {
                    let call = active.swap_remove(index);
                    #[cfg(feature = "render-timing")]
                    let response = call.span.in_scope(|| result.and_then(|value| runtime.convert_result(value)));
                    #[cfg(not(feature = "render-timing"))]
                    let response = result.and_then(|value| runtime.convert_result(value));
                    #[cfg(feature = "render-timing")]
                    record_render_timing(&call.job, call.started, response.is_ok());
                    let _ = call.job.reply.send(response);
                }
                Event::Closed => closed = true,
                Event::Quiescent => {
                    for call in active.drain(..) {
                        #[cfg(feature = "render-timing")]
                        record_render_timing(&call.job, call.started, false);
                        let _ = call.job.reply.send(Err(
                            "Export execution failed: Promise resolution is still pending but the event loop has already resolved".to_owned(),
                        ));
                    }
                }
                Event::Failed(error) => {
                    drop(lifetime);

                    eprintln!("Renderer worker {worker_id}: {error}");
                    return;
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{RenderError, RenderJob, Renderer};
    use crate::{RenderRequest, RendererConfig};
    use tokio::sync::oneshot;

    #[tokio::test]
    async fn closed_queue_drains_active_calls_and_skips_cancelled_waiters() {
        deno_core::JsRuntime::init_platform(None);
        let entry = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/renderer_concurrent.mjs");
        let renderer = Renderer::start(entry).await.unwrap();
        let mut replies = Vec::new();
        for (id, path, cancelled) in [
            ("cancelled-before-start", "/gated?key=never-start", true),
            ("drain-A", "/delay", false),
            ("drain-B", "/delay", false),
            ("drain-state", "/state", false),
        ] {
            let (reply, received) = oneshot::channel();
            if cancelled {
                drop(received);
            } else {
                replies.push((id, received));
            }
            renderer
                .sender
                .try_send(RenderJob {
                    request: RenderRequest {
                        request_id: id.to_owned(),
                        url: format!("http://127.0.0.1:3000{path}"),
                        method: "GET".to_owned(),
                        headers: Vec::new(),
                    },
                    reply,
                    #[cfg(feature = "render-timing")]
                    queued_at: std::time::Instant::now(),
                })
                .unwrap();
        }
        drop(renderer);
        for (id, reply) in replies {
            let response = tokio::time::timeout(Duration::from_secs(5), reply)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            if id == "drain-state" {
                assert_eq!(response.body, "2|2");
            } else {
                assert!(response.body.starts_with(id));
            }
        }
    }

    #[tokio::test]
    async fn full_queue_rejects_render_without_waiting_for_capacity() {
        let queue_capacity = 3;
        let (sender, receiver) = async_channel::bounded(queue_capacity);
        let renderer = Renderer::from_workers(sender, Vec::new());
        let mut pending_replies = Vec::new();

        for _ in 0..queue_capacity {
            let request = RenderRequest {
                request_id: "queue-test".to_owned(),
                url: "http://127.0.0.1:3000/test".to_owned(),
                method: "GET".to_owned(),
                headers: Vec::new(),
            };
            let (reply, response) = oneshot::channel();

            assert!(
                renderer
                    .sender
                    .try_send(RenderJob {
                        request,
                        reply,

                        #[cfg(feature = "render-timing")]
                        queued_at: std::time::Instant::now(),
                    })
                    .is_ok()
            );
            pending_replies.push(response);
        }

        let result = tokio::time::timeout(
            Duration::from_secs(2),
            renderer.render(RenderRequest {
                request_id: "queue-test".to_owned(),
                url: "http://127.0.0.1:3000/test".to_owned(),
                method: "GET".to_owned(),
                headers: Vec::new(),
            }),
        )
        .await
        .expect("A full queue must reject the request without waiting for capacity");

        assert!(matches!(result, Err(RenderError::Busy)));
        assert_eq!(receiver.len(), queue_capacity);
    }

    #[tokio::test]
    async fn invalid_config_is_rejected_before_creating_a_runtime() {
        let result = Renderer::start_with_config(
            std::path::PathBuf::from("not-a-module.mjs"),
            RendererConfig {
                workers: 1,
                max_in_flight: 0,
                queue_capacity: 16,
            },
            || panic!("An invalid configuration must not create a runtime"),
        )
        .await;
        assert!(matches!(result, Err(message) if message.contains("max_in_flight")));
    }

    #[tokio::test]
    async fn pool_initializes_two_independent_workers() {
        let threads = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let factory_threads = std::sync::Arc::clone(&threads);

        let entry = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/renderer_concurrent.mjs");

        let renderer = Renderer::start_with_config(
            entry,
            RendererConfig {
                workers: 2,
                max_in_flight: 16,
                queue_capacity: 16,
            },
            move || {
                factory_threads
                    .lock()
                    .unwrap()
                    .push(std::thread::current().id());

                crate::JavaScriptRuntime::new()
            },
        )
        .await
        .expect("Pool initialization failed");

        {
            let created = threads.lock().unwrap();

            assert_eq!(created.len(), 2);
            assert_ne!(created[0], created[1]);
        }

        let response = tokio::time::timeout(
            Duration::from_secs(5),
            renderer.render(RenderRequest {
                request_id: "pool-check".to_owned(),
                url: "http://127.0.0.1:3000/echo".to_owned(),
                method: "GET".to_owned(),
                headers: vec![("x-marker".to_owned(), "pool-check".to_owned())],
            }),
        )
        .await
        .expect("Pool render timed out")
        .expect("Pool render failed");

        assert_eq!(response.status, 200);
        assert_eq!(
            response.body,
            "pool-check|http://127.0.0.1:3000/echo|pool-check",
        );
    }
}
