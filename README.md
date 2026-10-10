# Rubicon

Rubicon is a bachelor's thesis project exploring a Rust-hosted server-side rendering framework for Angular. The proposed architecture uses Rust for HTTP handling and application services, with an embedded JavaScript runtime for Angular rendering.

The repository currently contains initial documentation drafts and an Axum HTTP server connected to a persistent JavaScript rendering worker. Angular runtime integration and performance improvements remain to be implemented and evaluated.

The Rust package is defined in [crates/rubicon/Cargo.toml](crates/rubicon/Cargo.toml), with Axum pinned to 0.8.9, deno_core to 0.412.0, Tokio to 1.53.2, and Serde to 1.0.229. The executable listens on `127.0.0.1:3000`; GET and HEAD requests to `/` and `/{*path}` invoke the `render(request)` export from [render.mjs](crates/rubicon/js/render.mjs). Successful responses use `200 OK` and `Content-Type: text/html; charset=utf-8`; GET delivers the returned HTML and HEAD omits its body.

Run the local server with:

```text
cargo run --locked --manifest-path crates/rubicon/Cargo.toml
```

The library's `JavaScriptRuntime` owns one embedded runtime, evaluates synchronous scripts through `execute_i64`, loads a main module, and calls exports through `call_export<TArgument, TResult>`, using serializable arguments and owned deserializable results. The public [Renderer](crates/rubicon/src/renderer.rs) accepts an owned [RenderRequest](crates/rubicon/src/request.rs), containing `request_id: String`, `url: String`, `method: String`, and `headers: Vec<(String, String)>`, and sends it with each job to a dedicated thread that owns the runtime and loads the module once. The worker borrows the request, converts it directly to a V8 object through `serde_v8::to_v8`, and passes it to JavaScript. The worker currently selects `String` as the result type. Its fixed queue holds up to 16 waiting jobs, with one render active at a time. Cloning `Renderer` shares access to this worker.

The library also exposes [RenderResponse](crates/rubicon/src/response.rs), a response data type containing `status: u16`, `headers: Vec<(String, String)>`, and `body: String`, with Serde deserialization support. The generic runtime conversion can select this result type. A prior proposal check in a temporary copy verified structured conversion, repeated header pairs, Unicode body text, and continued execution after conversion and JavaScript errors; its extra test is not retained in the repository. Transport of this type through the worker and delivery of its metadata remain to be connected. The running renderer still returns an HTML string. See the [generic export verification](docs/specs/architecture.md#generic-export-verification).

The public [into_http_response](crates/rubicon/src/http_response.rs) converts a `RenderResponse` into an Axum response. It accepts final statuses 200 through 599, requires empty bodies for 204, 205, and 304, validates header syntax, preserves repeated fields with `append`, and rejects renderer-supplied transport headers such as Content-Length, Transfer-Encoding, and Connection. It does not insert a default content type. Four library tests verify redirect metadata, repeated cookies, exact UTF-8 body bytes, and rejection cases. The helper is not yet called by the live HTTP handler; the [conversion milestone](docs/specs/architecture.md#http-response-conversion-verification) records checks and limits.

The HTTP handler combines the fixed application origin `http://127.0.0.1:3000` with the original request path and query, and extracts the HTTP method. The demonstration module displays both fields with HTML text escaping and a counter increasing on every render. For example, a GET to `/products?sort=asc&page=2` displays `Method: GET` and `URL: http://127.0.0.1:3000/products?sort=asc&amp;page=2` in the HTML source. The counter verifies module reuse; it is not an example of storing user data.

The host converts each request header value into a separate name-value pair, preserving repeated values for the same name. Header names use their normalized form; ordering across different names is unspecified. Values rejected by `HeaderValue::to_str()` produce HTTP 400 before rendering. This provisional text policy does not support every valid HTTP header byte sequence. The demonstration displays only `x-rubicon-demo` values as escaped list items. HTTP checks verified repeated values through the worker and JavaScript boundary.

The HTTP handler generates a decimal request identifier from a process-local atomic counter before header conversion. Client-supplied `X-Request-ID` does not replace it. Identifiers cross the JavaScript boundary as strings, preserving large integer values, and appear in the demonstration HTML and rendering-failure logs. They are unique within one server execution, reset on restart, and do not provide authentication or request isolation. Checked increment prevents wrapping: once the counter reaches `u64::MAX`, further matched requests receive HTTP 503. The maximum value itself is not emitted. Library callers supply their own identifier. See the [identifier verification](docs/specs/architecture.md#request-identifier-verification) for checks and limits.

Queue-full and unavailable-worker errors map to HTTP 503; rendering errors map to HTTP 500. The module initialization handshake completes before the HTTP listener is bound, but does not validate the complete renderer contract. Application origin configuration, proxy handling, and API/static-asset routing remain to be implemented.

For HEAD, the demonstration generates the same page representation as GET by displaying `Method: GET`, while preserving the actual `HEAD` method in the request object. HTTP checks verified an empty HEAD body and Content-Length matching that representation. The counter can change the representation length as its number of digits grows. The complete application response contract remains future work.

The module path currently uses the crate's build-time source directory. Worker configuration, asynchronous renderer verification, request isolation, deadline enforcement, graceful shutdown, deployable artifact packaging, and Angular SSR remain future steps. Linker warning LNK4098 persists; performance has not been measured.

The tests verify continued execution after synchronous script errors, rejection of a render when the queue is full, and continued worker execution after an exported JavaScript function throws or returns a non-string value. Four executable unit tests verify repeated header conversion, rejection of an unsupported text value, identifier uniqueness across four threads, and identifier exhaustion without wrapping. The worker integration check uses [renderer_errors.rs](crates/rubicon/tests/renderer_errors.rs) and a controlled [JavaScript fixture](crates/rubicon/tests/fixtures/renderer_errors.mjs); successful calls return counter values 1, 3, and 5 with request pairs GET/A, HEAD/B, and GET/A across the two failures. Exact comparisons verify identifier, URL, and method, including identifiers above JavaScript's safe integer range and query separators and apostrophes in the URLs. Those integration requests currently use empty header lists; header delivery evidence is recorded in the [architecture](docs/specs/architecture.md#request-header-verification).

Run the complete test suite with:

```text
cargo test --locked --manifest-path crates/rubicon/Cargo.toml
```

The integration target has its own `main` (`harness = false`) so it initializes the V8 platform before starting its worker. The test time limits bound asynchronous waits; they do not establish active JavaScript interruption or graceful worker shutdown. HTTP error mapping, load behavior, and request isolation still require separate verification.

Start with the [documentation index](docs/readme.md), the [writing guide](docs/guides/how-to-document.md), and the [documentation templates](docs/templates/readme.md).
