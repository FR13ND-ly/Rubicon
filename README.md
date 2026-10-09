# Rubicon

Rubicon is a bachelor's thesis project exploring a Rust-hosted server-side rendering framework for Angular. The proposed architecture uses Rust for HTTP handling and application services, with an embedded JavaScript runtime for Angular rendering.

The repository currently contains initial documentation drafts and an Axum HTTP server connected to a persistent JavaScript rendering worker. Angular runtime integration and performance improvements remain to be implemented and evaluated.

The Rust package is defined in [crates/rubicon/Cargo.toml](crates/rubicon/Cargo.toml), with Axum pinned to 0.8.9, deno_core to 0.412.0, and Tokio to 1.53.2. The executable listens on `127.0.0.1:3000`; GET requests to `/` and `/{*path}` invoke the `render(url)` export from [render.mjs](crates/rubicon/js/render.mjs) and return its string as `200 OK` with `Content-Type: text/html; charset=utf-8`.

Run the local server with:

```text
cargo run --locked --manifest-path crates/rubicon/Cargo.toml
```

The library's `JavaScriptRuntime` owns one embedded runtime, evaluates synchronous scripts through `execute_i64`, loads a main module, and calls string-returning exports with a string argument. The public [Renderer](crates/rubicon/src/renderer.rs) accepts an owned URL string and sends it with each job to a dedicated thread that owns the runtime and loads the module once. Its fixed queue holds up to 16 waiting jobs, with one render active at a time. Cloning `Renderer` shares access to this worker.

The HTTP handler combines the fixed application origin `http://127.0.0.1:3000` with the original request path and query. It passes the result to JavaScript as a V8 string value. The demonstration module displays that URL with HTML text escaping and a counter increasing on every render. For example, `/products?sort=asc&page=2` displays `URL: http://127.0.0.1:3000/products?sort=asc&amp;page=2` in the HTML source. The counter verifies module reuse; it is not an example of storing user data.

Queue-full and unavailable-worker errors map to HTTP 503; rendering errors map to HTTP 500. The module initialization handshake completes before the HTTP listener is bound, but does not validate the complete renderer contract. Application origin configuration, proxy handling, request methods and headers, request identifiers, and API/static-asset routing remain to be implemented.

The module path currently uses the crate's build-time source directory. Worker configuration, asynchronous renderer verification, request isolation, deadline enforcement, graceful shutdown, deployable artifact packaging, and Angular SSR remain future steps. Linker warning LNK4098 persists; performance has not been measured.

The tests verify continued execution after synchronous script errors, rejection of a render when the queue is full, and continued worker execution after an exported JavaScript function throws or returns a non-string value. The worker integration check uses [renderer_errors.rs](crates/rubicon/tests/renderer_errors.rs) and a controlled [JavaScript fixture](crates/rubicon/tests/fixtures/renderer_errors.mjs); successful calls return counter values 1, 3, and 5 with their respective URLs in the order A, B, A across the two failures. The checked strings include query separators and apostrophes.

Run the complete test suite with:

```text
cargo test --locked --manifest-path crates/rubicon/Cargo.toml
```

The integration target has its own `main` (`harness = false`) so it initializes the V8 platform before starting its worker. The test time limits bound asynchronous waits; they do not establish active JavaScript interruption or graceful worker shutdown. HTTP error mapping, load behavior, and request isolation still require separate verification.

Start with the [documentation index](docs/readme.md), the [writing guide](docs/guides/how-to-document.md), and the [documentation templates](docs/templates/readme.md).
