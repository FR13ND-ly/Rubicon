use std::{
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
use rubicon::{JavaScriptRuntime, ServerConfig, serve};

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
        std::fs::write(assets.0.join("test.css"), "body { color: red; }").unwrap();
        assets
    }
}

impl Drop for BrowserAssets {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.0.join("test.css"));
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
async fn propagates_shutdown_notification_errors() {
    let config = ServerConfig {
        bind: "127.0.0.1:0".parse().unwrap(),
        origin: "http://127.0.0.1:3000".to_owned(),
        server_entry: renderer_entry(),
        browser_dir: Path::new(env!("CARGO_MANIFEST_DIR")).join("js"),
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
