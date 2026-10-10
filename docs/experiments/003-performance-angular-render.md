# Experiment 003: Performance API and First Embedded Angular Render

**Status:** Completed temporary check; repository performance API verified; Angular adapter pending
**Created:** 2026-10-11
**Last updated:** 2026-10-11
**Type:** Exploratory correctness check; not a performance benchmark
**Source revision:** `49709a9`
**Implementation scope:** Temporary Angular rendering diagnostic; performance bootstrap and sixth fixture subsequently implemented in the repository

## Question and Result

Can the current persistent Rust worker render the pinned Angular application's neutral build after the remaining environment dependency is supplied?

Yes, for the five calls checked below. The current runtime initially failed the first GET with `ReferenceError: performance is not defined` during lazy application-module initialization. A temporary bootstrap addition exposing Deno's real performance object and initializing timeOrigin enabled Home and About SSR, a controlled 404 fallback, repeated Home rendering, and a HEAD representation. Each response also passed Rubicon's existing HTTP response conversion helper.

The Angular 22.2.2 development artifact was built earlier using the normal Angular CLI. The rendering calls here executed in the Rust worker and embedded V8; no HTTP server was started by the diagnostic. The author subsequently implemented the performance bootstrap and fixture, as recorded below. The actual repository executable still loads its demonstration module; the Angular adapter remains pending.

This establishes initial embedded rendering correctness. Browser asset serving, hydration through Rubicon, native networking, isolation, lifecycle acceptance, and performance measurement remain pending.

## Observed Missing API

The neutral server entry module had already loaded successfully in earlier checks. The failure occurred when Angular lazily imported main.server.mjs and initialized its component definitions:

```text
Export execution failed: ReferenceError: performance is not defined
    at performanceMarkFeature (.../main.server.mjs:9640:3)
```

The pinned generated source uses `performance?.mark?.("mark_feature_usage", ...)`. Optional property access does not prevent a ReferenceError when the performance identifier itself is absent. Other generated profiling paths call mark, measure, and clearMarks. These are real API calls; an empty stub would not establish compatibility.

## Proposed Bootstrap Change

In [web.js](../../crates/rubicon/js/web.js), immediately before the existing global registration loop, add:

```javascript
const { performance, setTimeOrigin } = core.loadExtScript(
  'ext:deno_web/15_performance.js',
);
setTimeOrigin();
```

Add `performance,` after `Response,` in the object passed to Object.entries. The existing descriptors make the global writable and configurable. No new dependency or Rust extension is required.

The pinned deno_web 0.290.0 performance script exports both performance and setTimeOrigin. Its timeOrigin variable is initially unset. setTimeOrigin invokes the registered op_time_origin, while now uses op_now and the per-runtime monotonic StartTime. Both script and Rust operation sources were reviewed; initialization was exercised by the fixture below.

## Proposed Performance Fixture

Create `crates/rubicon/tests/fixtures/renderer_performance.mjs` with:

```javascript
import { render as renderSync } from "./renderer_errors.mjs";

export async function render(request) {
    const start = performance.now();
    if (
        !Number.isFinite(start) ||
        start < 0 ||
        !Number.isFinite(performance.timeOrigin) ||
        performance.timeOrigin <= 0
    ) {
        throw new Error("Performance clock was not initialized");
    }

    performance.mark("rubicon-test-start", {
        detail: { requestId: request.request_id },
    });
    await new Promise((resolve) => setTimeout(resolve, 0));
    performance.mark("rubicon-test-end");

    const measure = performance.measure(
        "rubicon-test-render",
        "rubicon-test-start",
        "rubicon-test-end",
    );
    const marks = performance.getEntriesByName("rubicon-test-start", "mark");

    if (
        performance.now() < start ||
        !Number.isFinite(measure.duration) ||
        measure.duration < 0 ||
        measure.entryType !== "measure" ||
        marks.length !== 1 ||
        marks[0].detail.requestId !== request.request_id
    ) {
        throw new Error("Performance marks or measure were incorrect");
    }

    performance.clearMarks("rubicon-test-start");
    performance.clearMarks("rubicon-test-end");
    performance.clearMeasures("rubicon-test-render");

    if (
        performance.getEntriesByName("rubicon-test-start").length !== 0 ||
        performance.getEntriesByName("rubicon-test-end").length !== 0 ||
        performance.getEntriesByName("rubicon-test-render").length !== 0
    ) {
        throw new Error("Performance entries were not cleared");
    }

    return renderSync(request);
}
```

Add `"renderer_performance.mjs",` after `"renderer_fetch_objects.mjs",` in [renderer_errors.rs](../../crates/rubicon/tests/renderer_errors.rs).

The fixture checks initialized finite clock values, nondecreasing now values, a named mark carrying the request identifier, a finite nonnegative measure, and removal of its own named entries. It awaits a zero-delay timer between marks and delegates to the existing response/error fixture. There are no elapsed-time thresholds, throughput measurements, or speed claims. Cleanup is limited to the fixture's three names; it does not establish automatic cleanup of application marks.

## Temporary Angular Adapter

The diagnostic copied the existing `examples/angular-demo/dist/rubicon/server` directory, including the neutral entrypoint, main module, manifests, polyfills, and embedded HTML asset modules. It placed the following `render.rubicon.mjs` alongside server.mjs:

```javascript
import { reqHandler } from "./server.mjs";

export async function render(request) {
    const response = await reqHandler(
        new Request(request.url, {
            method: request.method,
            headers: new Headers(request.headers),
        }),
    );

    if (response === null) {
        return {
            status: 404,
            headers: [["content-type", "text/plain; charset=utf-8"]],
            body: "Not found",
        };
    }

    const headers = Array.from(response.headers.entries()).filter(
        ([name]) => name !== "set-cookie",
    );
    for (const cookie of response.headers.getSetCookie()) {
        headers.push(["set-cookie", cookie]);
    }

    return {
        status: response.status,
        headers,
        body: await response.text(),
    };
}
```

This adapter constructs Angular's Web Request from the owned Rust URL, method, and header pairs. A null handler result becomes a prototype 404 text response. It removes Set-Cookie from ordinary iteration and reinserts each getSetCookie value separately; it consumes the response body into a String-compatible value. Cookie behavior was checked independently by the existing Web object fixture; the Angular pages here did not generate cookies.

The wrapper is a diagnostic proposal, not a retained application entrypoint. The eventual repository adapter must be added and rebuilt by the author. Angular route handling occurs before response conversion, and request IDs are not injected into Angular application providers by this wrapper.

## Temporary Rust Diagnostic

The corrected diagnostic was added as `src/bin/angular_render_diagnostic.rs` in the copy, preserving the normal main.rs and its four tests:

```rust
use std::{path::PathBuf, time::Duration};

use rubicon::{RenderRequest, Renderer, into_http_response};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), String> {
    deno_core::JsRuntime::init_platform(None);
    let module_path = PathBuf::from(std::env::args().nth(1).ok_or("Missing module path")?);
    let renderer = tokio::time::timeout(Duration::from_secs(10), Renderer::start(module_path))
        .await
        .map_err(|_| "Angular worker startup timed out".to_owned())??;

    for (index, (method, path, heading)) in [
        ("GET", "/", "Home"),
        ("GET", "/about", "About"),
        ("GET", "/missing", ""),
        ("GET", "/", "Home"),
        ("HEAD", "/", "Home"),
    ]
    .into_iter()
    .enumerate()
    {
        let response = tokio::time::timeout(
            Duration::from_secs(10),
            renderer.render(RenderRequest {
                request_id: format!("angular-probe-{index}"),
                url: format!("http://127.0.0.1:3000{path}"),
                method: method.to_owned(),
                headers: vec![("x-rubicon-demo".to_owned(), "probe".to_owned())],
            }),
        )
        .await
        .map_err(|_| format!("Angular render timed out: {method} {path}"))?
        .map_err(|error| format!("Angular render failed for {method} {path}: {error:?}"))?;

        let expected_status = if path == "/missing" { 404 } else { 200 };
        if response.status != expected_status {
            return Err(format!("Unexpected status for {method} {path}: {}", response.status));
        }
        if path != "/missing" {
            if !response.body.contains(&format!("<h2>{heading}</h2>"))
                || !response.body.contains("ng-server-context=\"ssr\"")
                || !response.body.contains("ngh=")
            {
                return Err(format!("SSR markers or heading missing for {path}"));
            }
            if path == "/" && !response.body.contains("id=\"counter-value\">0</span>") {
                return Err("Home counter was not zero".to_owned());
            }
        }
        println!("ANGULAR_RENDER_OK {method} {path} {} {} bytes", response.status, response.body.len());
        into_http_response(response)
            .map_err(|error| format!("Angular HTTP response conversion failed: {error}"))?;
    }
    Ok(())
}
```

It initializes V8 on the main thread, starts the existing worker, and submits five sequential owned requests to that same worker. The timeouts bound asynchronous waiting in this controlled diagnostic; they are not production deadlines and do not prove interruption of blocking JavaScript. The heading and counter assertions are specific to the current reference templates.

An earlier diagnostic incorrectly required the worker's HEAD result to have an empty body. Angular returned the complete Home representation. That is compatible with the existing worker boundary: the Axum GET/HEAD route removes the body at HTTP delivery. The corrected diagnostic checks the HEAD representation and conversion helper; it does not perform a HEAD HTTP request or verify wire-level Content-Length for Angular.

## Commands and Outcomes

The final copy used package name `rubicon-angular-performance-final-probe`, library name `rubicon`, and the corresponding root package rename in its copied Cargo.lock. It kept the existing dependency versions and normal executable sources and reused the ignored repository target directory.

```text
cargo fmt --manifest-path <copy>/Cargo.toml
cargo run --locked --offline --bin angular_render_diagnostic --manifest-path <copy>/Cargo.toml --target-dir C:/Users/motri/Desktop/rubicon/crates/rubicon/target -- <copied-server>/render.rubicon.mjs
cargo check --locked --offline --manifest-path <copy>/Cargo.toml --all-targets --target-dir C:/Users/motri/Desktop/rubicon/crates/rubicon/target
cargo test --locked --offline --manifest-path <copy>/Cargo.toml --target-dir C:/Users/motri/Desktop/rubicon/crates/rubicon/target
```

All commands passed in the final copy. The render diagnostic printed:

```text
ANGULAR_RENDER_OK GET / 200 1670 bytes
ANGULAR_RENDER_OK GET /about 200 1583 bytes
ANGULAR_RENDER_OK GET /missing 404 9 bytes
ANGULAR_RENDER_OK GET / 200 1670 bytes
ANGULAR_RENDER_OK HEAD / 200 1670 bytes
```

The three successful GET page renders contained their expected heading, ng-server-context="ssr", and hydration annotations. Both Home renders contained counter zero; HEAD likewise returned the Home representation with counter zero. All five responses passed into_http_response. Byte counts describe this development artifact only and are not payload optimization or performance evidence. HTML hydration annotations do not establish browser hydration.

Eight library tests, four normal executable tests, and all six integration fixtures passed. The extra diagnostic binary had no unit tests. One macro-generated documentation example was ignored. LNK4098 persisted. The performance fixture retained response statuses, exact body/header comparisons, the intentional export failure on call 2, conversion failure on call 4, and successful subsequent calls.

The latest complete local output is in ignored `crates/rubicon/target/angular-render-probe.log`. Earlier missing-performance and HEAD-assumption outputs are retained in ignored `angular-render-missing-performance.log` and `angular-render-head-assumption.log`. The temporary source and Angular artifact copies were removed after verification; generated and diagnostic artifacts are outside version control.

## Temporary Copy Cache Observation

Reusing a previously compiled temporary package after its source directory had been deleted caused runtime initialization to report `The system cannot find the path specified. (os error 3)`. Changing the package variant forced a fresh library compilation and resolved that failure. The inferred cause is reuse of a library containing an obsolete extension source path: the pinned deno_core ExtensionFileSource implementation supports loading JavaScript from recorded filesystem paths. The observed failure did not identify the missing file directly.

When repeating this experiment, keep the current copy alive through execution and ensure its library is compiled for that copy, for example with a fresh temporary package identity. A sandboxed rerun also emitted incremental-cache hard-link warnings and used copying as a fallback. Deployable extension and application packaging remain separate work.

## Repository Performance Verification

On 2026-10-11, starting from `436efd1`, the author applied the performance bootstrap addition and added renderer_performance.mjs as the sixth integration fixture. No dependency version, runtime scheduler, HTTP handler, or request/response type changed. The bootstrap now exposes twenty selected public Web globals.

Repository formatting verification, locked offline compilation for all targets, and the full test suite passed:

```text
cargo fmt --manifest-path crates/rubicon/Cargo.toml -- --check
cargo check --locked --offline --manifest-path crates/rubicon/Cargo.toml --all-targets
cargo test --locked --offline --manifest-path crates/rubicon/Cargo.toml
```

Eight library tests, four executable tests, and all six integration fixtures passed. One macro-generated documentation example was ignored. LNK4098 persists. The fixture checks finite clock values, positive timeOrigin, nondecreasing readings, marks with request detail, a nonnegative measure, and removal of its own named entries before the controlled response/error sequence. It uses no timing threshold and does not measure throughput or latency.

## Author's Next Step

The performance bootstrap and sixth fixture are verified in the repository. The next [HTTP adapter proposal](004-angular-http-adapter.md) includes the exact TypeScript entry and Rust path edits, with successful temporary build and HTTP checks. Apply and verify those edits in the repository, then serve the matching browser assets and repeat the established browser hydration and counter checks.

## Limits

The public performance object is standard runtime infrastructure; adding it is not a measured performance improvement. This experiment does not prove complete Angular or Web API compatibility, concurrent rendering, request isolation, bounded memory, recovery after Angular bootstrap failures, native fetch, cancellation, deadlines, or automatic resource cleanup. The successful 404 is an adapter fallback, not an Angular not-found page. The Rust response remains buffered.

This is additional evidence for continuing [ADR 001](../adr/001-runtime-selection.md), which remains Draft until its other acceptance conditions are satisfied.

## Sources

- Downloaded deno_web 0.290.0 `15_performance.js` and `timers.rs`.
- Downloaded deno_core 0.412.0 `extensions.rs`.
- Generated neutral Angular 22.2.2 development artifact and its performance call sites, from the existing [reference build](../../examples/angular-demo/README.md#neutral-build-for-rubicon).
- [Web Request/Response experiment](002-web-fetch-objects.md) and [initial neutral bundle diagnostic](001-angular-runtime-compatibility.md).
