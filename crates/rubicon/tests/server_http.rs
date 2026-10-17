use std::{
    future::Future,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use axum::{Router, routing::get};
use rubicon::{JavaScriptRuntime, RendererConfig, ServerConfig, serve};

struct HttpRuntimeDropProbe(Arc<AtomicUsize>);

impl Drop for HttpRuntimeDropProbe {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

deno_core::extension!(
    http_cleanup_test,
    options = {
        dropped: Arc<AtomicUsize>,
    },
    state = |state, options| {
        state.put(HttpRuntimeDropProbe(options.dropped));
    },
);

async fn serve_with_cleanup_probe<S>(bind: SocketAddr, shutdown: S) -> Result<(), String>
where
    S: Future<Output = Result<(), String>> + Send + 'static,
{
    let dropped = Arc::new(AtomicUsize::new(0));
    let factory_dropped = Arc::clone(&dropped);

    let config = ServerConfig {
        bind,
        origin: format!("http://{bind}"),
        server_entry: renderer_entry(),
        browser_dir: Path::new(env!("CARGO_MANIFEST_DIR")).join("js"),
        renderer: RendererConfig {
            workers: 2,
            ..Default::default()
        },
    };

    let result = tokio::time::timeout(
        Duration::from_secs(10),
        serve(
            config,
            Router::new(),
            move || {
                JavaScriptRuntime::with_extensions(vec![http_cleanup_test::init(Arc::clone(
                    &factory_dropped,
                ))])
            },
            shutdown,
        ),
    )
    .await
    .expect("Server lifecycle check timed out");

    assert_eq!(
        dropped.load(Ordering::SeqCst),
        2,
        "Both runtime states must be released before serve returns: {result:?}",
    );

    result
}

struct BrowserAssets(PathBuf);

impl BrowserAssets {
    fn new() -> Self {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "rubicon-server-http-{}-{suffix}",
            std::process::id(),
        ));
        std::fs::create_dir(&directory).unwrap();
        let assets = Self(directory);
        std::fs::create_dir(assets.0.join("assets")).unwrap();
        std::fs::create_dir(assets.0.join("api")).unwrap();
        for (name, body) in [
            ("test.css", "body { color: red; }"),
            ("download", "extensionless asset"),
            ("space name.css", "space asset"),
            ("țară.css", "unicode asset"),
            ("percent%20.css", "literal percent asset"),
            ("assets/nested.css", "nested asset"),
            (
                "api/test",
                "static route must not override application route",
            ),
        ] {
            std::fs::write(assets.0.join(name), body).unwrap();
        }
        assets
    }
}

impl Drop for BrowserAssets {
    fn drop(&mut self) {
        for name in [
            "test.css",
            "download",
            "space name.css",
            "țară.css",
            "percent%20.css",
            "assets/nested.css",
            "api/test",
        ] {
            let _ = std::fs::remove_file(self.0.join(name));
        }
        let _ = std::fs::remove_dir(self.0.join("assets"));
        let _ = std::fs::remove_dir(self.0.join("api"));
        let _ = std::fs::remove_dir(&self.0);
    }
}

fn renderer_entry() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("js/render.mjs")
}

async fn request(
    address: SocketAddr,
    method: &str,
    path: &str,
    headers: &str,
) -> (u16, String, String) {
    let packet = format!(
        "{method} {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n{headers}\r\n",
    );

    tokio::task::spawn_blocking(move || {
        let mut connection = TcpStream::connect_timeout(&address, Duration::from_secs(5)).unwrap();
        connection
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        connection
            .set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        connection.write_all(packet.as_bytes()).unwrap();

        let mut response = Vec::new();
        connection.read_to_end(&mut response).unwrap();
        let separator = response
            .windows(4)
            .position(|bytes| bytes == b"\r\n\r\n")
            .unwrap();
        let headers = String::from_utf8(response[..separator].to_vec()).unwrap();
        let status = headers.split_whitespace().nth(1).unwrap().parse().unwrap();
        let body = String::from_utf8(response[separator + 4..].to_vec()).unwrap();
        (status, headers, body)
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn serves_application_routes_assets_ssr_and_shuts_down() {
    let assets = BrowserAssets::new();
    let reserved = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = reserved.local_addr().unwrap();
    let origin = format!("http://{address}");
    let factory_calls = Arc::new(AtomicUsize::new(0));
    let calls = Arc::clone(&factory_calls);
    let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel();
    let config = ServerConfig {
        bind: address,
        origin: origin.clone(),
        server_entry: renderer_entry(),
        browser_dir: assets.0.clone(),
        renderer: Default::default(),
    };
    let routes = Router::new().route("/api/test", get(|| async { "application route" }));
    drop(reserved);

    let server = tokio::spawn(serve(
        config,
        routes,
        move || {
            calls.fetch_add(1, Ordering::Relaxed);
            JavaScriptRuntime::new()
        },
        async move {
            shutdown_receiver
                .await
                .map_err(|_| "Shutdown sender was dropped".to_owned())
        },
    ));

    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            assert!(!server.is_finished(), "Server exited before readiness");
            if tokio::net::TcpStream::connect(address).await.is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("Server startup timed out");

    let (status, _, body) = request(address, "GET", "/api/test", "").await;
    assert_eq!(status, 200);
    assert_eq!(body, "application route");

    let (status, headers, body) = request(address, "GET", "/test.css", "").await;
    assert_eq!(status, 200);
    assert!(
        headers
            .to_ascii_lowercase()
            .contains("content-type: text/css")
    );
    assert!(body.contains("body { color: red; }"));

    let etag = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("etag")
                .then(|| value.trim().to_owned())
        })
        .expect("Static assets must preserve conditional HTTP support");
    let (status, _, body) = request(
        address,
        "GET",
        "/test.css",
        &format!("If-None-Match: {etag}\r\n"),
    )
    .await;
    assert_eq!(status, 304);
    assert!(body.is_empty());
    let (status, _, body) = request(address, "GET", "/test.css", "Range: bytes=0-3\r\n").await;
    assert_eq!(status, 206);
    assert_eq!(body, "body");
    let (status, headers, body) = request(address, "HEAD", "/test.css", "").await;
    assert_eq!(status, 200);
    assert!(body.is_empty());
    assert!(headers.to_ascii_lowercase().contains("content-length: 20"));
    for (path, expected) in [
        ("/download?version=1", "extensionless asset"),
        ("/space%20name.css", "space asset"),
        ("/%C8%9Bar%C4%83.css", "unicode asset"),
        ("/percent%2520.css", "literal percent asset"),
        ("/assets%2Fnested.css", "nested asset"),
        ("/%74est.css", "body { color: red; }"),
    ] {
        let (status, _, body) = request(address, "GET", path, "").await;
        assert_eq!(status, 200, "Static asset failed: {path}");
        assert_eq!(body, expected, "Static asset changed: {path}");
    }

    let path = "/products?q=rust&lang=ro";
    let (status, headers, body) = request(
        address,
        "GET",
        path,
        "x-rubicon-demo: first\r\nx-rubicon-demo: second\r\n",
    )
    .await;
    assert_eq!(status, 200);
    assert!(
        headers
            .to_ascii_lowercase()
            .contains("content-type: text/html")
    );
    assert!(body.contains(&format!("{origin}/products?q=rust&amp;lang=ro")));
    assert!(body.contains("<li>first</li><li>second</li>"));

    let (status, _, body) = request(address, "HEAD", path, "").await;
    assert_eq!(status, 200);
    assert!(body.is_empty(), "HEAD must not send a body");
    assert_eq!(factory_calls.load(Ordering::Relaxed), 1);

    shutdown_sender.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .expect("Server shutdown timed out")
        .expect("Server task panicked")
        .expect("Server failed during shutdown");
    TcpListener::bind(address).expect("Shutdown must release the listening port");
}

#[tokio::test]
async fn binary_ssr_preserves_utf8_cookies_and_head_semantics() {
    let reserved = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = reserved.local_addr().unwrap();
    let config = ServerConfig {
        bind: address,
        origin: format!("http://{address}"),
        server_entry: Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/renderer_binary.mjs"),
        browser_dir: Path::new(env!("CARGO_MANIFEST_DIR")).join("js"),
        renderer: Default::default(),
    };
    let (sender, receiver) = tokio::sync::oneshot::channel();
    drop(reserved);
    let server = tokio::spawn(serve(
        config,
        Router::new(),
        JavaScriptRuntime::new,
        async {
            receiver
                .await
                .map_err(|_| "Shutdown sender dropped".to_owned())
        },
    ));
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            assert!(
                !server.is_finished(),
                "Binary HTTP server exited before readiness"
            );
            if tokio::net::TcpStream::connect(address).await.is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let expected = "<p>țară 🌍</p>";
    let (status, headers, body) = request(address, "GET", "/utf8", "").await;
    assert_eq!(status, 200);
    assert_eq!(body, expected);
    assert_eq!(
        headers.to_ascii_lowercase().matches("set-cookie:").count(),
        2
    );
    let (status, headers, body) = request(address, "HEAD", "/utf8", "").await;
    assert_eq!(status, 200);
    assert!(body.is_empty());
    assert!(
        headers
            .to_ascii_lowercase()
            .contains(&format!("content-length: {}", expected.len()))
    );
    sender.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    TcpListener::bind(address).expect("Binary server must release its port");
}

#[tokio::test]
#[cfg(feature = "streaming")]
async fn streamed_http_delivers_chunks_cookies_and_cancels_head() {
    use std::io::{BufRead, BufReader};
    fn chunk(reader: &mut BufReader<TcpStream>) -> Vec<u8> {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let length = usize::from_str_radix(line.trim(), 16).unwrap();
        assert!(length <= 65536);
        let mut bytes = vec![0; length];
        reader.read_exact(&mut bytes).unwrap();
        let mut end = [0; 2];
        reader.read_exact(&mut end).unwrap();
        assert_eq!(&end, b"\r\n");
        bytes
    }
    let reserved = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = reserved.local_addr().unwrap();
    let config = ServerConfig {
        bind: address,
        origin: format!("http://{address}"),
        server_entry: Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/renderer_streaming.mjs"),
        browser_dir: Path::new(env!("CARGO_MANIFEST_DIR")).join("js"),
        renderer: RendererConfig {
            workers: 1,
            max_in_flight: 2,
            queue_capacity: 16,
        },
    };
    let (sender, receiver) = tokio::sync::oneshot::channel();
    drop(reserved);
    let server = tokio::spawn(serve(
        config,
        Router::new(),
        JavaScriptRuntime::new,
        async {
            receiver
                .await
                .map_err(|_| "Shutdown sender dropped".to_owned())
        },
    ));
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            assert!(!server.is_finished());
            if tokio::net::TcpStream::connect(address).await.is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let mut reader = tokio::task::spawn_blocking(move || {
        let mut connection = TcpStream::connect_timeout(&address, Duration::from_secs(5)).unwrap();
        connection
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        connection
            .set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        connection
            .write_all(
                format!("GET /delayed HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n")
                    .as_bytes(),
            )
            .unwrap();
        let mut reader = BufReader::new(connection);
        let mut headers = String::new();
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if line == "\r\n" {
                break;
            }
            assert!(!line.is_empty());
            headers.push_str(&line);
            assert!(headers.len() < 65536);
        }
        assert!(headers.starts_with("HTTP/1.1 200"));
        let headers = headers.to_ascii_lowercase();
        assert_eq!(headers.matches("set-cookie:").count(), 2);
        assert!(headers.contains("transfer-encoding: chunked"));
        assert!(!headers.contains("content-length:"));
        assert_eq!(chunk(&mut reader), "<p>țară ".as_bytes());
        reader
    })
    .await
    .unwrap();
    assert_eq!(request(address, "GET", "/release", "").await.2, "released");
    tokio::task::spawn_blocking(move || {
        assert_eq!(chunk(&mut reader), "🌍</p>".as_bytes());
        assert!(chunk(&mut reader).is_empty());
    })
    .await
    .unwrap();
    let (status, headers, body) = request(address, "HEAD", "/delayed", "").await;
    assert_eq!(status, 200);
    assert!(body.is_empty());
    assert!(!headers.to_ascii_lowercase().contains("content-length:"));
    sender.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    TcpListener::bind(address).expect("Streaming server must release its port");
}

#[tokio::test]
async fn propagates_shutdown_notification_errors() {
    let config = ServerConfig {
        bind: "127.0.0.1:0".parse().unwrap(),
        origin: "http://127.0.0.1:3000".to_owned(),
        server_entry: renderer_entry(),
        browser_dir: Path::new(env!("CARGO_MANIFEST_DIR")).join("js"),
        renderer: Default::default(),
    };

    let result = tokio::time::timeout(
        Duration::from_secs(10),
        serve(config, Router::new(), JavaScriptRuntime::new, async {
            Err("test shutdown failure".to_owned())
        }),
    )
    .await
    .expect("Server did not propagate the shutdown error");

    assert_eq!(result, Err("test shutdown failure".to_owned()));
}

#[tokio::test]
async fn serve_releases_workers_after_normal_shutdown() {
    serve_with_cleanup_probe("127.0.0.1:0".parse().unwrap(), async { Ok(()) })
        .await
        .expect("Normal server shutdown failed");
}

#[tokio::test]
async fn serve_releases_workers_after_shutdown_signal_error() {
    let result = serve_with_cleanup_probe("127.0.0.1:0".parse().unwrap(), async {
        Err("intentional shutdown signal failure".to_owned())
    })
    .await;

    assert_eq!(
        result,
        Err("intentional shutdown signal failure".to_owned()),
    );
}

#[tokio::test]
async fn serve_releases_workers_when_listening_port_is_occupied() {
    let occupied = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = occupied.local_addr().unwrap();

    let result =
        serve_with_cleanup_probe(address, std::future::pending::<Result<(), String>>()).await;

    let error = result.expect_err("An occupied port must be rejected");

    assert!(
        error.starts_with("HTTP listener creation failed:"),
        "Unexpected server error: {error}",
    );
}
