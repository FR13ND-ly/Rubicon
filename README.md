# Rubicon

Rubicon is a bachelor's thesis project exploring a Rust-hosted server-side rendering framework for Angular. The proposed architecture uses Rust for HTTP handling and application services, with an embedded JavaScript runtime for Angular rendering.

The repository currently contains initial documentation drafts and an Axum HTTP server connected to a persistent JavaScript rendering worker. Angular runtime integration and performance improvements remain to be implemented and evaluated.

The Rust package is defined in [crates/rubicon/Cargo.toml](crates/rubicon/Cargo.toml), with Axum pinned to 0.8.9, deno_core to 0.412.0, and Tokio to 1.53.2. The executable listens on `127.0.0.1:3000`; `GET /` invokes the `render` export from [render.mjs](crates/rubicon/js/render.mjs) and returns its string as `200 OK` with `Content-Type: text/html; charset=utf-8`.

Run the local server with:

```text
cargo run --locked --manifest-path crates/rubicon/Cargo.toml
```

The library's `JavaScriptRuntime` owns one embedded runtime, evaluates synchronous scripts through `execute_i64`, loads a main module, and calls string-returning exports. The public [Renderer](crates/rubicon/src/renderer.rs) sends work to a dedicated thread that owns the runtime and loads the module once. Its fixed queue holds up to 16 waiting jobs, with one render active at a time. Cloning `Renderer` shares access to this worker.

The demonstration module returns `<h1>Rubicon</h1><p>Render N</p>`, with a counter increasing on every render. This counter verifies module reuse; it is not an example of storing user data. Queue-full and unavailable-worker errors map to HTTP 503; rendering errors map to HTTP 500. The module initialization handshake completes before the HTTP listener is bound, but does not validate the complete renderer contract.

The module path currently uses the crate's build-time source directory. Worker configuration, asynchronous renderer verification, request isolation, deadline enforcement, graceful shutdown, deployable artifact packaging, and Angular SSR remain future steps. Linker warning LNK4098 persists; performance has not been measured.

A regression test verifies that a JavaScript exception and an object-to-integer conversion failure return errors, and that the same runtime continues executing valid scripts with its counter state preserved after each failure.

Start with the [documentation index](docs/readme.md), the [writing guide](docs/guides/how-to-document.md), and the [documentation templates](docs/templates/readme.md).
