use std::{
    cell::RefCell,
    path::Path,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};

use deno_core::{OpState, op2};
use rubicon::{JavaScriptRuntime, RenderError, RenderRequest, Renderer, RendererConfig};
use tokio::sync::Barrier;

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

fn request(id: &str) -> RenderRequest {
    RenderRequest {
        request_id: id.to_owned(),
        url: format!("http://127.0.0.1:3000/pool?id={id}"),
        method: "GET".to_owned(),
        headers: vec![("x-marker".to_owned(), id.to_owned())],
    }
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
