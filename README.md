# Rubicon

Rubicon is a bachelor's thesis project exploring a Rust-hosted server-side rendering framework for Angular. The proposed architecture uses Rust for HTTP handling and application services, with an embedded JavaScript runtime for Angular rendering.

The repository currently contains initial documentation drafts and an Axum HTTP server connected to a persistent JavaScript rendering worker. Angular runtime integration and performance improvements remain to be implemented and evaluated.

The Rust package is defined in [crates/rubicon/Cargo.toml](crates/rubicon/Cargo.toml), with Axum pinned to 0.8.9, deno_core to 0.412.0, Tokio to 1.53.2, and Serde to 1.0.229. The executable listens on `127.0.0.1:3000`; GET and HEAD requests to `/` and `/{*path}` invoke the `render(request)` export from [render.mjs](crates/rubicon/js/render.mjs). Successful responses use `200 OK` and `Content-Type: text/html; charset=utf-8`; GET delivers the returned HTML and HEAD omits its body.

Run the local server with:

```text
cargo run --locked --manifest-path crates/rubicon/Cargo.toml
```

The library's `JavaScriptRuntime` owns one embedded runtime, evaluates synchronous scripts through `execute_i64`, loads a main module, and calls string-returning exports with serializable arguments. The public [Renderer](crates/rubicon/src/renderer.rs) accepts an owned [RenderRequest](crates/rubicon/src/request.rs), containing `url: String` and `method: String`, and sends it with each job to a dedicated thread that owns the runtime and loads the module once. The worker borrows the request, converts it directly to a V8 object through `serde_v8::to_v8`, and passes it to JavaScript. Its fixed queue holds up to 16 waiting jobs, with one render active at a time. Cloning `Renderer` shares access to this worker.

The HTTP handler combines the fixed application origin `http://127.0.0.1:3000` with the original request path and query, and extracts the HTTP method. The demonstration module displays both fields with HTML text escaping and a counter increasing on every render. For example, a GET to `/products?sort=asc&page=2` displays `Method: GET` and `URL: http://127.0.0.1:3000/products?sort=asc&amp;page=2` in the HTML source. The counter verifies module reuse; it is not an example of storing user data.

Queue-full and unavailable-worker errors map to HTTP 503; rendering errors map to HTTP 500. The module initialization handshake completes before the HTTP listener is bound, but does not validate the complete renderer contract. Application origin configuration, proxy handling, request headers, request identifiers, and API/static-asset routing remain to be implemented.

HEAD omits its body, but its current Content-Length describes HTML displaying `Method: HEAD`, which is one byte longer than the corresponding GET demonstration HTML. Aligning this metadata with the GET representation remains open; HEAD contract acceptance is incomplete.

The module path currently uses the crate's build-time source directory. Worker configuration, asynchronous renderer verification, request isolation, deadline enforcement, graceful shutdown, deployable artifact packaging, and Angular SSR remain future steps. Linker warning LNK4098 persists; performance has not been measured.

The tests verify continued execution after synchronous script errors, rejection of a render when the queue is full, and continued worker execution after an exported JavaScript function throws or returns a non-string value. The worker integration check uses [renderer_errors.rs](crates/rubicon/tests/renderer_errors.rs) and a controlled [JavaScript fixture](crates/rubicon/tests/fixtures/renderer_errors.mjs); successful calls return counter values 1, 3, and 5 with request pairs GET/A, HEAD/B, and GET/A across the two failures. Exact comparisons verify both object fields, including query separators and apostrophes in the URLs.

Run the complete test suite with:

```text
cargo test --locked --manifest-path crates/rubicon/Cargo.toml
```

The integration target has its own `main` (`harness = false`) so it initializes the V8 platform before starting its worker. The test time limits bound asynchronous waits; they do not establish active JavaScript interruption or graceful worker shutdown. HTTP error mapping, load behavior, and request isolation still require separate verification.

Start with the [documentation index](docs/readme.md), the [writing guide](docs/guides/how-to-document.md), and the [documentation templates](docs/templates/readme.md).
