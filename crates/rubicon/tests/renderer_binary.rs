use std::{path::Path, time::Duration};

use axum::body::to_bytes;
use rubicon::{
    RenderBody, RenderError, RenderRequest, RenderResponse, Renderer, into_http_response,
};

async fn render(renderer: &Renderer, path: &str) -> Result<RenderResponse, RenderError> {
    tokio::time::timeout(
        Duration::from_secs(5),
        renderer.render(RenderRequest {
            request_id: path.to_owned(),
            url: format!("http://127.0.0.1:3000{path}"),
            method: "GET".to_owned(),
            headers: Vec::new(),
        }),
    )
    .await
    .expect("Binary render timed out")
}

#[tokio::test(flavor = "current_thread")]
async fn transports_owned_binary_bodies_without_text_conversion() {
    let entry = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/renderer_binary.mjs");
    let renderer = Renderer::start(entry).await.unwrap();

    for (path, expected) in [
        ("/utf8", "<p>țară 🌍</p>".as_bytes()),
        ("/slice", &[0, 128, 255][..]),
        ("/buffer", &[0, 128, 255][..]),
        ("/empty", &[][..]),
    ] {
        let response = render(&renderer, path).await.unwrap();
        assert!(matches!(response.body, RenderBody::Bytes(_)));
        assert_eq!(response.body.as_bytes(), expected);
        let http = into_http_response(response).unwrap();
        assert_eq!(http.status(), 200);
        assert_eq!(http.headers().get_all("set-cookie").iter().count(), 2);
        assert_eq!(
            to_bytes(http.into_body(), 1024).await.unwrap().as_ref(),
            expected
        );
    }

    let owned = render(&renderer, "/owned").await.unwrap();
    assert!(
        owned.body.as_text().is_err(),
        "Invalid UTF-8 is valid binary data"
    );
    assert_eq!(render(&renderer, "/mutate").await.unwrap().body, "mutated");
    assert_eq!(owned.body.as_bytes(), &[0, 128, 255]);

    assert!(matches!(render(&renderer, "/invalid").await,
        Err(RenderError::Failed(message)) if message.starts_with("Result conversion failed:")));
    assert_eq!(
        render(&renderer, "/text").await.unwrap().body,
        "text response"
    );

    renderer.shutdown().await.unwrap();
    assert_eq!(
        owned.body.as_bytes(),
        &[0, 128, 255],
        "Body must outlive its V8 runtime"
    );
}
