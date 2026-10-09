# Rubicon

Rubicon is a bachelor's thesis project exploring a Rust-hosted server-side rendering framework for Angular. The proposed architecture uses Rust for HTTP handling and application services, with an embedded JavaScript runtime for Angular rendering.

The repository currently contains initial documentation drafts, a JavaScript runtime integration prototype, and an Axum HTTP server. Angular runtime integration and performance improvements remain to be implemented and evaluated.

The Rust package is defined in [crates/rubicon/Cargo.toml](crates/rubicon/Cargo.toml), with Axum pinned to 0.8.9, deno_core to 0.412.0, and Tokio to 1.53.2. The executable listens on `127.0.0.1:3000`; `GET /` returns `200 OK`, `Content-Type: text/html; charset=utf-8`, and the fixed body `<h1>Rubicon</h1>`.

Run the local server with:

```text
cargo run --locked --manifest-path crates/rubicon/Cargo.toml
```

The library's `JavaScriptRuntime` owns one embedded runtime, evaluates synchronous scripts through `execute_i64`, loads a main module, and calls string-returning exports. The earlier module prototype verified two calls to [render.mjs](crates/rubicon/js/render.mjs). Connecting this renderer to HTTP through a persistent worker, asynchronous renderer behavior, and request isolation remain future steps.

A regression test verifies that a JavaScript exception and an object-to-integer conversion failure return errors, and that the same runtime continues executing valid scripts with its counter state preserved after each failure.

Start with the [documentation index](docs/readme.md), the [writing guide](docs/guides/how-to-document.md), and the [documentation templates](docs/templates/readme.md).
