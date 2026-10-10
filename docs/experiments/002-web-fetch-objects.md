# Experiment 002: Web Request and Response Objects

**Status:** Completed temporary check with verified repository implementation
**Created:** 2026-10-10
**Last updated:** 2026-10-10
**Type:** Exploratory correctness check; not a performance benchmark
**Source revision:** `8e977c7`
**Initial implementation scope:** Temporary Rust crate copy; the later repository implementation is recorded below

## Question and Result

Can the pinned Deno extensions provide Headers, Request, and Response objects for a future Angular adapter while preserving the existing worker behavior?

The temporary copy passed the checks recorded below. Request metadata, header normalization, separate Set-Cookie values, response status, body cloning, split UTF-8 decoding, and rejection of a second body read were verified through the same persistent worker used by Rubicon. No Angular handler or network operation was invoked. The executable in the actual repository still loads the demonstration module.

## Dependencies and Source Review

Add these exact dependencies alongside the existing Deno entries in [Cargo.toml](../../crates/rubicon/Cargo.toml):

```toml
deno_fetch = "=0.283.0"
deno_net = "=0.251.0"
```

Downloaded manifests for both versions depend on deno_core 0.412.0, matching the existing crate. The deno_fetch extension declares deno_webidl and deno_web dependencies. Its Request script loads the HTTP client script, which loads `ext:deno_net/02_tls.js`; that script also loads the networking implementation. Registering deno_net provides those real scripts and operations. It does not expose a global fetch function or a Deno namespace in this proposal.

The extension setup was checked from the pinned crate sources. A rolling Docs.rs page for deno_net returned older 0.248.0 metadata; the proposal version comes from Cargo's registry metadata and downloaded 0.251.0 manifest, rather than that stale page.

## Proposed Runtime Changes

In [lib.rs](../../crates/rubicon/src/lib.rs), replace the bootstrap extension's dependency list with:

```rust
deps = [deno_webidl, deno_web, deno_net, deno_fetch],
```

In `RuntimeOptions.extensions`, insert the following after the existing deno_web initialization and immediately before `rubicon_web::init()`:

```rust
deno_net::deno_net::init(None, None),
deno_fetch::deno_fetch::init(Default::default()),
```

The two None arguments configure no custom certificate provider and no certificate-error bypass list. Native networking and its permission state are outside this experiment; the fixture does not perform fetch, TLS, or filesystem operations.

In [web.js](../../crates/rubicon/js/web.js), before the existing global registration loop, add:

```javascript
const { Headers } = core.loadExtScript('ext:deno_fetch/20_headers.js');
const { Request } = core.loadExtScript('ext:deno_fetch/23_request.js');
const { Response } = core.loadExtScript('ext:deno_fetch/23_response.js');
```

Add `Headers`, `Request`, and `Response` to the object used by that loop. Their global properties use the existing writable/configurable descriptors.

## Proposed Integration Fixture

Create `crates/rubicon/tests/fixtures/renderer_fetch_objects.mjs` with:

```javascript
import { render as renderSync } from "./renderer_errors.mjs";

export async function render(request) {
    const expectedUrl = new URL(request.url).href;
    const webRequest = new Request(request.url, {
        method: request.method,
        headers: new Headers(request.headers),
    });
    const requestClone = webRequest.clone();

    if (
        !(webRequest.headers instanceof Headers) ||
        !(webRequest.signal instanceof AbortSignal) ||
        webRequest.url !== expectedUrl ||
        webRequest.method !== request.method ||
        webRequest.body !== null ||
        requestClone.url !== webRequest.url ||
        requestClone.method !== webRequest.method
    ) {
        throw new Error("Web Request did not preserve request metadata");
    }

    const headers = new Headers([
        ["X-Demo", "one"],
        ["x-demo", "two"],
    ]);
    if (headers.get("X-DEMO") !== "one, two") {
        throw new Error("Headers did not normalize and combine repeated values");
    }

    const cookies = [
        "session=first; Path=/; HttpOnly",
        "theme=dark; Expires=Wed, 21 Oct 2026 07:28:00 GMT; Path=/",
    ];
    headers.append("content-type", "text/html; charset=utf-8");
    for (const cookie of cookies) {
        headers.append("set-cookie", cookie);
    }

    const text = "Răspuns 🦀";
    const bytes = new TextEncoder().encode(text);
    const body = new ReadableStream({
        start(controller) {
            controller.enqueue(bytes.slice(0, -2));
            controller.enqueue(bytes.slice(-2));
            controller.close();
        },
    });
    const response = new Response(body, { status: 201, headers });
    const responseClone = response.clone();
    const actualCookies = response.headers.getSetCookie();

    if (
        response.status !== 201 ||
        response.bodyUsed ||
        response.headers.get("content-type") !== "text/html; charset=utf-8" ||
        actualCookies.length !== cookies.length ||
        actualCookies.some((cookie, index) => cookie !== cookies[index])
    ) {
        throw new Error("Web Response lost status or separate cookies");
    }

    const bodies = await Promise.all([response.text(), responseClone.text()]);
    if (
        bodies.some((value) => value !== text) ||
        !response.bodyUsed ||
        !responseClone.bodyUsed
    ) {
        throw new Error("Response body or clone lost split UTF-8 content");
    }

    let secondReadError;
    try {
        await response.text();
    } catch (error) {
        secondReadError = error;
    }
    if (!(secondReadError instanceof TypeError)) {
        throw new Error("A consumed response body was readable again");
    }

    return renderSync(request);
}
```

Then include `"renderer_fetch_objects.mjs",` after `"renderer_streams.mjs",` in the fixture list in [renderer_errors.rs](../../crates/rubicon/tests/renderer_errors.rs).

The fixture builds a Web Request from each existing GET or HEAD request and checks a metadata clone. Request.url uses the canonical URL representation obtained from new URL(request.url).href, while the owned Rust request retains its original text. Those integration requests have empty input headers; two controlled X-Demo values independently check normalization and comma combination. The response contains two separate cookies, including an Expires value with a comma. The check uses getSetCookie rather than splitting a combined string.

Its UTF-8 stream deliberately divides the final emoji between two byte chunks. Both the response and its clone must decode the original text. Reading either consumes its body, and a second read of the original response must reject with TypeError. These local Web objects are consumed before delegating to the existing synchronous fixture; that export still supplies the plain object converted into Rust RenderResponse.

## Temporary Verification

The copy used package name `rubicon-fetch-probe`, retained library name `rubicon`, and renamed the corresponding root package in the copied Cargo.lock. It reused the ignored repository target directory. Initial dependency resolution and downloads changed only the copy's lockfile.

```text
cargo fmt --manifest-path <copy>/Cargo.toml
cargo check --offline --manifest-path <copy>/Cargo.toml --all-targets --target-dir C:/Users/motri/Desktop/rubicon/crates/rubicon/target
cargo test --locked --offline --manifest-path <copy>/Cargo.toml --target-dir C:/Users/motri/Desktop/rubicon/crates/rubicon/target
```

The first proposal incorrectly required Request.url to equal the raw request string. Call 2, whose query contains apostrophes, failed the metadata assertion before reaching the intended fixture exception. A diagnostic rerun exposed that assertion; pinned Request source constructs a URL and uses its href. The corrected proposal compares with the canonical URL representation and retains the original request when delegating to the response fixture. The two failed-run logs remain ignored as fetch-objects-probe-first-failure.log and fetch-objects-probe-diagnostic-failure.log.

Formatting, offline all-target compilation, and the full locked offline test command passed for the corrected proposal. Eight library tests, four executable unit tests, and all five renderer fixtures passed, including `Renderer fixture passed: renderer_fetch_objects.mjs`. One macro-generated documentation example was ignored. LNK4098 remained present. All fixtures retain exact response comparisons, the intentional export failure on call 2, the conversion failure on call 4, and successful subsequent calls.

The temporary source directory was removed after verification. Full local output and the resolved copy lockfile remain in ignored `crates/rubicon/target/fetch-objects-probe.log` and `fetch-objects-probe.Cargo.lock`. The retained lockfile names the temporary package; it is diagnostic evidence, not a replacement for the project's Cargo.lock.

## Author Verification Commands

After applying the proposal in the actual crate, run from the repository root:

```text
cargo fmt --manifest-path crates/rubicon/Cargo.toml
cargo check --manifest-path crates/rubicon/Cargo.toml --all-targets
cargo test --locked --offline --manifest-path crates/rubicon/Cargo.toml
```

The first check intentionally omits --locked so Cargo can resolve the new dependencies and update the real Cargo.lock. Subsequent checks should use the committed lockfile. These were the proposed author commands; the completed repository verification is recorded below.

## Repository Verification

On 2026-10-10, starting from `80b8073`, the author applied the pinned dependencies, Cargo.lock, extension registration, public bindings, and fifth fixture to the actual crate. The fixture uses `Response 🦀` rather than `Răspuns 🦀`, preserving the final emoji split across two UTF-8 byte chunks and the other checked behavior.

Repository formatting verification, locked offline all-target compilation, and the full locked offline test command passed:

```text
cargo fmt --manifest-path crates/rubicon/Cargo.toml -- --check
cargo check --locked --offline --manifest-path crates/rubicon/Cargo.toml --all-targets
cargo test --locked --offline --manifest-path crates/rubicon/Cargo.toml
```

Eight library tests, four executable tests, and all five renderer fixtures passed; one macro-generated documentation example was ignored. LNK4098 remains. The [architecture milestone](../specs/architecture.md#web-request-and-response-verification) records the implemented scope. Public fetch, networking, HTTP request-body support, Angular rendering, and performance remain pending.

## Limits and Next Step

This is selected correctness evidence, not full Fetch standard conformance or a performance result. Native network access, FormData/Blob public methods, request bodies from Axum, abort propagation from HTTP, size limits, and automatic application cleanup were not verified. Rust still receives a buffered String body through its existing response contract.

With the repository implementation verified, continue the environment check and add an adapter from the owned Rust request to Angular's Web Request handler. Preserve status, body, and separate cookie fields when converting the Web Response. Actual Angular rendering, matching browser asset delivery, and hydration remain pending.

## Sources

- [deno_fetch on Docs.rs](https://docs.rs/crate/deno_fetch/latest), checked on 2026-10-10, reporting 0.283.0 and deno_core 0.412.0.
- Downloaded deno_fetch 0.283.0 manifest, `lib.rs`, `20_headers.js`, `22_body.js`, `22_http_client.js`, `23_request.js`, and `23_response.js`.
- Downloaded deno_net 0.251.0 manifest, `lib.rs`, `01_net.js`, and `02_tls.js`, obtained with Cargo's registry tooling on 2026-10-10.
- [Angular non-Node SSR configuration](https://angular.dev/guide/ssr#non-nodejs) and the [previous compatibility experiment](001-angular-runtime-compatibility.md) establish why a Web Request/Response boundary is the integration direction.
