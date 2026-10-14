use std::{
    cell::RefCell,
    collections::HashSet,
    path::Path,
    rc::Rc,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread::{self, ThreadId},
    time::Duration,
};

use deno_core::{
    OpState,
    futures::{future::join_all, poll},
    op2,
};
use rubicon::{JavaScriptRuntime, RenderError, RenderRequest, Renderer, RendererConfig};
use tokio::sync::{Barrier, Semaphore};

#[op2]
async fn op_pool_test_wait(state: Rc<RefCell<OpState>>) {
    let barrier = {
        let state = state.borrow();
        Arc::clone(state.borrow::<Arc<Barrier>>())
    };

    barrier.wait().await;
}

deno_core::extension!(
    pool_test,
    ops = [op_pool_test_wait],
    options = {
        barrier: Arc<Barrier>,
    },
    state = |state, options| {
        state.put(options.barrier);
    },
);

struct RuntimeDropProbe(Arc<AtomicUsize>);

impl Drop for RuntimeDropProbe {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

deno_core::extension!(
    pool_cleanup_test,
    options = {
        dropped: Arc<AtomicUsize>,
    },
    state = |state, options| {
        state.put(RuntimeDropProbe(options.dropped));
    },
);

struct StartupWaitState {
    started: mpsc::Sender<()>,
    completed: Arc<AtomicUsize>,
}

#[op2]
async fn op_pool_test_startup_wait(state: Rc<RefCell<OpState>>) {
    let (started, completed) = {
        let state = state.borrow();
        let wait = state.borrow::<StartupWaitState>();

        (wait.started.clone(), Arc::clone(&wait.completed))
    };

    drop(state);

    started
        .send(())
        .expect("Cannot signal pending module initialization");

    tokio::time::sleep(Duration::from_secs(5)).await;

    completed.fetch_add(1, Ordering::SeqCst);
}

deno_core::extension!(
    pool_startup_wait_test,
    ops = [op_pool_test_startup_wait],
    options = {
        started: mpsc::Sender<()>,
        completed: Arc<AtomicUsize>,
    },
    state = |state, options| {
        state.put(StartupWaitState {
            started: options.started,
            completed: options.completed,
        });
    },
);

struct RenderWaitState {
    started: mpsc::Sender<()>,
    release: Arc<Semaphore>,
}

#[op2]
async fn op_pool_test_render_wait(state: Rc<RefCell<OpState>>) -> bool {
    let (started, release) = {
        let state = state.borrow();
        let wait = state.borrow::<RenderWaitState>();

        (wait.started.clone(), Arc::clone(&wait.release))
    };

    drop(state);

    started.send(()).expect("Cannot signal active rendering");

    match tokio::time::timeout(Duration::from_secs(10), release.acquire_owned()).await {
        Ok(Ok(permit)) => {
            permit.forget();
            true
        }
        _ => false,
    }
}

deno_core::extension!(
    pool_render_wait_test,
    ops = [op_pool_test_render_wait],
    options = {
        started: mpsc::Sender<()>,
        release: Arc<Semaphore>,
    },
    state = |state, options| {
        state.put(RenderWaitState {
            started: options.started,
            release: options.release,
        });
    },
);

#[derive(Default)]
struct SyncWaitState {
    threads: Vec<ThreadId>,
    timed_out: bool,
}

struct SyncWaitGate {
    participants: usize,
    state: Mutex<SyncWaitState>,
    changed: Condvar,
}

impl SyncWaitGate {
    fn new(participants: usize) -> Self {
        Self {
            participants,
            state: Mutex::new(SyncWaitState::default()),
            changed: Condvar::new(),
        }
    }

    fn wait(&self) -> bool {
        let mut state = self.state.lock().unwrap();
        state.threads.push(thread::current().id());
        self.changed.notify_all();

        let (mut state, timeout) = self
            .changed
            .wait_timeout_while(state, Duration::from_secs(5), |state| {
                state.threads.len() < self.participants && !state.timed_out
            })
            .unwrap();

        if timeout.timed_out() && state.threads.len() < self.participants {
            // Later serial calls must not turn a timed-out rendezvous into success.
            state.timed_out = true;
            self.changed.notify_all();
        }

        !state.timed_out && state.threads.len() == self.participants
    }
}

#[op2(fast)]
fn op_pool_test_sync_wait(state: &mut OpState) -> bool {
    // Deliberately block this worker's synchronous JavaScript call stack.
    state.borrow::<Arc<SyncWaitGate>>().wait()
}

deno_core::extension!(
    pool_sync_test,
    ops = [op_pool_test_sync_wait],
    options = {
        gate: Arc<SyncWaitGate>,
    },
    state = |state, options| {
        state.put(options.gate);
    },
);

fn request(id: &str) -> RenderRequest {
    RenderRequest {
        request_id: id.to_owned(),
        url: format!("http://127.0.0.1:3000/pool?id={id}"),
        method: "GET".to_owned(),
        headers: vec![("x-marker".to_owned(), id.to_owned())],
    }
}

async fn check_synchronous_worker_execution(workers: usize, overlapped: bool) {
    let participants = workers.max(2);
    let gate = Arc::new(SyncWaitGate::new(participants));
    let factory_gate = Arc::clone(&gate);

    let entry = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/renderer_pool_sync.mjs");

    let renderer = tokio::time::timeout(
        Duration::from_secs(10),
        Renderer::start_with_config(
            entry,
            RendererConfig {
                workers,
                max_in_flight: 16,
                queue_capacity: 16,
            },
            move || {
                JavaScriptRuntime::with_extensions(vec![pool_sync_test::init(Arc::clone(
                    &factory_gate,
                ))])
            },
        ),
    )
    .await
    .expect("Synchronous pool initialization timed out")
    .expect("Synchronous pool initialization failed");

    let identifiers: Vec<_> = (0..participants)
        .map(|index| format!("sync-{workers}-{index}"))
        .collect();

    let responses = tokio::time::timeout(
        Duration::from_secs(15),
        join_all(identifiers.iter().map(|id| renderer.render(request(id)))),
    )
    .await;

    tokio::time::timeout(Duration::from_secs(10), renderer.shutdown())
        .await
        .expect("Synchronous pool shutdown timed out")
        .expect("Synchronous pool shutdown failed");

    for (id, result) in identifiers
        .into_iter()
        .zip(responses.expect("Synchronous rendering timed out"))
    {
        let response = result.expect("Synchronous rendering failed");
        assert_eq!(response.status, 200);
        assert_eq!(
            response.headers,
            vec![("x-request-id".to_owned(), id.clone())],
        );
        assert_eq!(response.body, format!("{id}|{overlapped}|704982704"));
    }

    let state = gate.state.lock().unwrap();
    assert_eq!(state.threads.len(), participants);
    assert_eq!(state.timed_out, !overlapped);
    assert_eq!(
        state.threads.iter().copied().collect::<HashSet<_>>().len(),
        workers,
        "Synchronous rendering must use the expected number of distinct worker threads",
    );
    assert!(
        !state.threads.contains(&thread::current().id()),
        "Rendering must execute outside the caller's Tokio thread",
    );
}

#[tokio::test(flavor = "current_thread")]
async fn synchronous_renders_are_serial_with_one_worker() {
    check_synchronous_worker_execution(1, false).await;
}

#[tokio::test(flavor = "current_thread")]
async fn synchronous_renders_overlap_on_two_worker_threads() {
    check_synchronous_worker_execution(2, true).await;
}

#[tokio::test(flavor = "current_thread")]
async fn synchronous_renders_overlap_on_four_worker_threads() {
    check_synchronous_worker_execution(4, true).await;
}

#[tokio::test(flavor = "current_thread")]
async fn workers_keep_independent_persistent_javascript_state() {
    let barrier = Arc::new(Barrier::new(2));

    let entry = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/renderer_pool.mjs");

    let renderer = tokio::time::timeout(
        Duration::from_secs(5),
        Renderer::start_with_config(
            entry,
            RendererConfig {
                workers: 2,
                max_in_flight: 1,
                queue_capacity: 16,
            },
            move || JavaScriptRuntime::with_extensions(vec![pool_test::init(Arc::clone(&barrier))]),
        ),
    )
    .await
    .expect("Pool initialization timed out")
    .expect("Pool initialization failed");

    for round in 1..=3 {
        let first_id = format!("round-{round}-A");
        let second_id = format!("round-{round}-B");

        let (first, second) = tokio::time::timeout(Duration::from_secs(5), async {
            tokio::join!(
                renderer.render(request(&first_id)),
                renderer.render(request(&second_id)),
            )
        })
        .await
        .expect("Both workers must reach the barrier");

        for (id, result) in [(first_id, first), (second_id, second)] {
            let response = result.expect("Pool render failed");

            assert_eq!(response.status, 200);
            assert_eq!(
                response.headers,
                vec![("x-request-id".to_owned(), id.clone())],
            );
            assert_eq!(
                response.body,
                format!("{round}|{id}|http://127.0.0.1:3000/pool?id={id}|{id}"),
            );
        }
    }
}

#[tokio::test(flavor = "current_thread")]
async fn healthy_worker_survives_another_workers_fatal_error() {
    let barrier = Arc::new(Barrier::new(2));

    let entry =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/renderer_pool_fatal.mjs");

    let renderer = tokio::time::timeout(
        Duration::from_secs(5),
        Renderer::start_with_config(
            entry,
            RendererConfig {
                workers: 2,
                max_in_flight: 1,
                queue_capacity: 16,
            },
            move || JavaScriptRuntime::with_extensions(vec![pool_test::init(Arc::clone(&barrier))]),
        ),
    )
    .await
    .expect("Pool initialization timed out")
    .expect("Pool initialization failed");

    let (fatal, survivor) = tokio::time::timeout(Duration::from_secs(5), async {
        tokio::join!(
            renderer.render(request("fatal")),
            renderer.render(request("survivor")),
        )
    })
    .await
    .expect("Worker failure did not notify the waiting caller");

    assert!(
        matches!(fatal, Err(RenderError::Unavailable)),
        "Expected the failed worker to drop its active request, got {fatal:?}",
    );

    let survivor = survivor.expect("The healthy worker must respond");

    assert_eq!(survivor.status, 200);
    assert_eq!(
        survivor.body,
        "1|survivor|http://127.0.0.1:3000/pool?id=survivor|survivor",
    );
    assert_eq!(
        survivor.headers,
        vec![("x-request-id".to_owned(), "survivor".to_owned())],
    );

    for sequence in 2..=5 {
        let id = format!("after-failure-{sequence}");

        let response = tokio::time::timeout(Duration::from_secs(5), renderer.render(request(&id)))
            .await
            .expect("The surviving worker stopped responding")
            .expect("The pool must remain available");

        assert_eq!(response.status, 200);
        assert_eq!(
            response.headers,
            vec![("x-request-id".to_owned(), id.clone())],
        );
        assert_eq!(
            response.body,
            format!("{sequence}|{id}|http://127.0.0.1:3000/pool?id={id}|{id}"),
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn startup_failure_drops_the_runtime_already_created() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let dropped = Arc::new(AtomicUsize::new(0));

    let factory_attempts = Arc::clone(&attempts);
    let factory_dropped = Arc::clone(&dropped);

    let (created_sender, created_receiver) = mpsc::channel::<()>();
    let created_receiver = Mutex::new(created_receiver);

    let entry = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/renderer_async.mjs");

    let result = tokio::time::timeout(
        Duration::from_secs(10),
        Renderer::start_with_config(
            entry,
            RendererConfig {
                workers: 2,
                max_in_flight: 1,
                queue_capacity: 16,
            },
            move || {
                let attempt = factory_attempts.fetch_add(1, Ordering::SeqCst);

                if attempt == 0 {
                    let runtime =
                        JavaScriptRuntime::with_extensions(vec![pool_cleanup_test::init(
                            Arc::clone(&factory_dropped),
                        )])?;

                    created_sender
                        .send(())
                        .map_err(|error| format!("Runtime creation signal failed: {error}"))?;

                    Ok(runtime)
                } else {
                    created_receiver
                        .lock()
                        .unwrap()
                        .recv_timeout(Duration::from_secs(5))
                        .map_err(|error| format!("First runtime was not created: {error}"))?;

                    Err("intentional startup failure".to_owned())
                }
            },
        ),
    )
    .await
    .expect("Pool startup cleanup timed out");

    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("Pool startup must fail"),
    };

    assert!(
        error.contains("intentional startup failure"),
        "Unexpected startup error: {error}",
    );
    assert_eq!(
        attempts.load(Ordering::SeqCst),
        2,
        "Both workers must attempt initialization",
    );
    assert_eq!(
        dropped.load(Ordering::SeqCst),
        1,
        "The created runtime must be cleaned up before startup returns",
    );
}

#[tokio::test(flavor = "current_thread")]
async fn startup_failure_cancels_a_pending_module_initialization() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let dropped = Arc::new(AtomicUsize::new(0));
    let completed = Arc::new(AtomicUsize::new(0));

    let factory_attempts = Arc::clone(&attempts);
    let factory_dropped = Arc::clone(&dropped);
    let factory_completed = Arc::clone(&completed);

    let (started_sender, started_receiver) = mpsc::channel::<()>();
    let started_receiver = Mutex::new(started_receiver);

    let entry =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/renderer_pool_startup_wait.mjs");

    let result = tokio::time::timeout(
        Duration::from_secs(15),
        Renderer::start_with_config(
            entry,
            RendererConfig {
                workers: 2,
                max_in_flight: 1,
                queue_capacity: 16,
            },
            move || {
                let attempt = factory_attempts.fetch_add(1, Ordering::SeqCst);

                if attempt == 0 {
                    JavaScriptRuntime::with_extensions(vec![
                        pool_cleanup_test::init(Arc::clone(&factory_dropped)),
                        pool_startup_wait_test::init(
                            started_sender.clone(),
                            Arc::clone(&factory_completed),
                        ),
                    ])
                } else {
                    started_receiver
                        .lock()
                        .unwrap()
                        .recv_timeout(Duration::from_secs(5))
                        .map_err(|error| {
                            format!("Module initialization did not reach its wait: {error}")
                        })?;

                    Err("intentional failure while another module is loading".to_owned())
                }
            },
        ),
    )
    .await
    .expect("Pending startup cleanup timed out");

    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("Pool startup must fail"),
    };

    assert!(
        error.contains("intentional failure while another module is loading"),
        "Unexpected startup error: {error}",
    );
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
    assert_eq!(
        dropped.load(Ordering::SeqCst),
        1,
        "The waiting runtime must be cleaned up before startup returns",
    );
    assert_eq!(
        completed.load(Ordering::SeqCst),
        0,
        "Initialization must be cancelled before the operation completes naturally",
    );
}

#[tokio::test(flavor = "current_thread")]
async fn last_renderer_handle_drop_releases_all_runtime_states() {
    let barrier = Arc::new(Barrier::new(2));
    let dropped = Arc::new(AtomicUsize::new(0));

    let factory_dropped = Arc::clone(&dropped);

    let entry = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/renderer_pool.mjs");

    let renderer = tokio::time::timeout(
        Duration::from_secs(5),
        Renderer::start_with_config(
            entry,
            RendererConfig {
                workers: 2,
                max_in_flight: 1,
                queue_capacity: 16,
            },
            move || {
                JavaScriptRuntime::with_extensions(vec![
                    pool_test::init(Arc::clone(&barrier)),
                    pool_cleanup_test::init(Arc::clone(&factory_dropped)),
                ])
            },
        ),
    )
    .await
    .expect("Pool initialization timed out")
    .expect("Pool initialization failed");

    let remaining = renderer.clone();
    drop(renderer);

    assert_eq!(
        dropped.load(Ordering::SeqCst),
        0,
        "A remaining Renderer handle must keep the pool available",
    );

    let (first, second) = tokio::time::timeout(Duration::from_secs(5), async {
        tokio::join!(
            remaining.render(request("shutdown-A")),
            remaining.render(request("shutdown-B")),
        )
    })
    .await
    .expect("The remaining Renderer handle stopped responding");

    for (id, result) in [("shutdown-A", first), ("shutdown-B", second)] {
        let response = result.expect("Rendering through the clone failed");

        assert_eq!(response.status, 200);
        assert_eq!(
            response.body,
            format!("1|{id}|http://127.0.0.1:3000/pool?id={id}|{id}"),
        );
    }

    drop(remaining);

    tokio::time::timeout(Duration::from_secs(5), async {
        while dropped.load(Ordering::SeqCst) < 2 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("Runtime states were not released after the last handle");

    assert_eq!(dropped.load(Ordering::SeqCst), 2);
}

#[tokio::test(flavor = "current_thread")]
async fn explicit_shutdown_is_shared_and_rejects_further_renders() {
    let dropped = Arc::new(AtomicUsize::new(0));
    let factory_dropped = Arc::clone(&dropped);

    let entry = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/renderer_async.mjs");

    let renderer = tokio::time::timeout(
        Duration::from_secs(5),
        Renderer::start_with_config(
            entry,
            RendererConfig {
                workers: 2,
                max_in_flight: 1,
                queue_capacity: 16,
            },
            move || {
                JavaScriptRuntime::with_extensions(vec![pool_cleanup_test::init(Arc::clone(
                    &factory_dropped,
                ))])
            },
        ),
    )
    .await
    .expect("Pool initialization timed out")
    .expect("Pool initialization failed");

    let other = renderer.clone();

    tokio::time::timeout(Duration::from_secs(5), async {
        let (first, second) = tokio::join!(renderer.shutdown(), other.shutdown(),);

        first.expect("First shutdown failed");
        second.expect("Concurrent shutdown failed");

        assert_eq!(
            dropped.load(Ordering::SeqCst),
            2,
            "Both runtime states must be released before shutdown returns",
        );

        assert!(matches!(
            renderer.render(request("after-shutdown")).await,
            Err(RenderError::Unavailable),
        ));

        assert!(matches!(
            other.render(request("after-shutdown-clone")).await,
            Err(RenderError::Unavailable),
        ));

        renderer.shutdown().await.expect("Repeated shutdown failed");

        other
            .shutdown()
            .await
            .expect("Repeated shutdown through the clone failed");
    })
    .await
    .expect("Explicit shutdown checks timed out");
}

#[tokio::test(flavor = "current_thread")]
async fn shutdown_drains_accepted_requests_after_waiter_cancellation() {
    let release = Arc::new(Semaphore::new(0));
    let dropped = Arc::new(AtomicUsize::new(0));

    let factory_release = Arc::clone(&release);
    let factory_dropped = Arc::clone(&dropped);

    let (started_sender, started_receiver) = mpsc::channel::<()>();

    let entry = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/renderer_pool_shutdown_wait.mjs");

    let renderer = tokio::time::timeout(
        Duration::from_secs(5),
        Renderer::start_with_config(
            entry,
            RendererConfig {
                workers: 2,
                max_in_flight: 1,
                queue_capacity: 2,
            },
            move || {
                JavaScriptRuntime::with_extensions(vec![
                    pool_render_wait_test::init(
                        started_sender.clone(),
                        Arc::clone(&factory_release),
                    ),
                    pool_cleanup_test::init(Arc::clone(&factory_dropped)),
                ])
            },
        ),
    )
    .await
    .expect("Pool initialization timed out")
    .expect("Pool initialization failed");

    let [active_a, active_b] = ["active-A", "active-B"].map(|id| {
        let renderer = renderer.clone();

        tokio::spawn(async move { renderer.render(request(id)).await })
    });

    tokio::task::spawn_blocking(move || {
        for _ in 0..2 {
            started_receiver
                .recv_timeout(Duration::from_secs(5))
                .expect("Both workers must start rendering");
        }
    })
    .await
    .expect("Render start monitor failed");

    let mut queued_a = Box::pin(renderer.render(request("queued-A")));
    let mut queued_b = Box::pin(renderer.render(request("queued-B")));

    assert!(
        poll!(queued_a.as_mut()).is_pending(),
        "Queued request A must be accepted and wait",
    );
    assert!(
        poll!(queued_b.as_mut()).is_pending(),
        "Queued request B must be accepted and wait",
    );

    let mut abandoned = Box::pin(renderer.shutdown());

    assert!(
        poll!(abandoned.as_mut()).is_pending(),
        "Shutdown must wait for accepted work",
    );

    drop(abandoned);

    let mut shutdown = Box::pin(renderer.shutdown());

    assert!(
        poll!(shutdown.as_mut()).is_pending(),
        "A new shutdown waiter must still wait for the workers",
    );

    assert!(matches!(
        renderer.render(request("too-late")).await,
        Err(RenderError::Unavailable),
    ));

    release.add_permits(2);

    let (first, second, third, fourth, stopped) =
        tokio::time::timeout(Duration::from_secs(5), async {
            tokio::join!(active_a, active_b, queued_a, queued_b, shutdown,)
        })
        .await
        .expect("Accepted requests or shutdown did not finish");

    for (id, result) in [
        ("active-A", first.expect("Active task A failed")),
        ("active-B", second.expect("Active task B failed")),
        ("queued-A", third),
        ("queued-B", fourth),
    ] {
        let response = result.expect("An accepted render was lost");

        assert_eq!(response.status, 200);
        assert_eq!(response.body, id);
        assert_eq!(
            response.headers,
            vec![("x-request-id".to_owned(), id.to_owned())],
        );
    }

    stopped.expect("Shutdown failed");

    assert_eq!(dropped.load(Ordering::SeqCst), 2);
}
