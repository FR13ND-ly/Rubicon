# Experiment 004: Angular Adapter and Rust HTTP Delivery

**Status:** Completed temporary check; author implementation pending
**Created:** 2026-10-11
**Last updated:** 2026-10-11
**Type:** Integration correctness check; not a performance benchmark
**Source revision:** `15f085c`
**Application:** Angular 22.2.2, development neutral build
**Implementation scope:** Temporary application and Rust copies; repository source unchanged

## Question and Result

Can the neutral TypeScript entry export Rubicon's render contract, and can the existing Axum server use the generated module to deliver Angular pages over HTTP?

Yes, for the controlled requests checked below. The temporary application built successfully with a named render export in server.mjs. The existing Rust server selected that module and delivered Home and About SSR, a plain-text 404 for an unhandled route, repeated Home, and Home with a query string. HEAD returned no body and Content-Length matching the GET representation. The browser script referenced by the HTML returned 404 because this step adds no static-file service.

The earlier [worker diagnostic](003-performance-angular-render.md) used a separate JavaScript wrapper around an already-built handler and did not start an HTTP server. This experiment places the adapter in the application's TypeScript source, rebuilds it, and exercises real HTTP delivery through Axum. The actual repository executable still selects js/render.mjs until the author applies the edits.

## Proposed TypeScript Entry

Replace examples/angular-demo/src/server.rubicon.ts with:

```typescript
import { AngularAppEngine, createRequestHandler } from '@angular/ssr';

const angularApp = new AngularAppEngine();

export const reqHandler = createRequestHandler((request: Request) =>
  angularApp.handle(request),
);

type RubiconRequest = {
  request_id: string;
  url: string;
  method: string;
  headers: [string, string][];
};

type RubiconResponse = {
  status: number;
  headers: [string, string][];
  body: string;
};

export async function render(
  request: RubiconRequest,
): Promise<RubiconResponse> {
  const response = await reqHandler(
    new Request(request.url, {
      method: request.method,
      headers: new Headers(request.headers),
    }),
  );

  if (response === null) {
    return {
      status: 404,
      headers: [['content-type', 'text/plain; charset=utf-8']],
      body: 'Not found',
    };
  }

  const headers: [string, string][] = Array.from(
    response.headers.entries(),
  ).filter(([name]) => name !== 'set-cookie');

  for (const cookie of response.headers.getSetCookie()) {
    headers.push(['set-cookie', cookie]);
  }

  return {
    status: response.status,
    headers,
    body: await response.text(),
  };
}
```

The types mirror Rubicon's owned request and buffered response objects. The request identifier is accepted as part of the contract but is not yet injected into Angular providers or used for application data. Only the URL, method, and text header pairs are used to construct the Web Request. The existing host routes accept GET and HEAD; request bodies and additional methods remain separate work.

One AngularAppEngine instance is retained at module scope. createRequestHandler preserves the public Angular handler entrypoint; render delegates to it and awaits the result. An unhandled route returns null, which the adapter maps to a 404 text response. This is not an Angular not-found component.

Response headers are copied into name/value pairs. Set-Cookie is excluded from the ordinary entries loop and copied separately through getSetCookie to avoid combining cookies. The reference pages do not set cookies; selected cookie behavior is covered by the existing [Web objects fixture](../../crates/rubicon/tests/fixtures/renderer_fetch_objects.mjs). The body is fully consumed with response.text before it crosses the Rust boundary.

## Proposed Rust Module Selection

In crates/rubicon/src/main.rs, replace:

```rust
let module_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("js/render.mjs");
```

with:

```rust
let module_path = Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../examples/angular-demo/dist/rubicon/server/server.mjs");
```

The existing loader canonicalizes the path. No dependency, worker scheduler, HTTP converter, or route handler needs to change for this step. The generated module requires its neighboring manifests, chunks, and embedded HTML assets, so the whole server output directory must remain available.

This path is a local prototype choice anchored to the crate's build-time manifest directory. Configurable application selection, deployable packaging, and an executable independent of the source checkout remain future work.

## Temporary Verification Method

A fresh temporary directory contained copies of the Angular application source/configuration and Rust crate. Its node_modules junction pointed to the already-installed, pinned application dependencies. The adapter above was written only in that copy and compiled with development,rubicon. The generated module was checked for its named render export.

The Rust copy used package name rubicon-angular-http-probe-391ea457 and library name rubicon, with the matching root-package rename in its copied Cargo.lock. This fresh identity forced compilation against the current extension source paths, avoiding the temporary-cache problem recorded in experiment 003. Its main.rs selected the copied Angular artifact. Both the bind address and fixed application origin used the same available loopback port; the normal repository values remain port 3000.

The copied server process was started with a hidden window, and requests were made only after its listener became available. It was stopped after verification. The node_modules junction was removed before recursively removing the checked temporary root. Existing repository application outputs and any other server processes were preserved.

## Commands and Outcomes

The temporary copies used the following command shapes:

```text
npm.cmd run build -- --configuration development,rubicon
cargo fmt --manifest-path <copy>/Cargo.toml
cargo check --locked --offline --manifest-path <copy>/Cargo.toml --all-targets --target-dir <repository>/crates/rubicon/target
cargo test --locked --offline --manifest-path <copy>/Cargo.toml --target-dir <repository>/crates/rubicon/target
cargo build --locked --offline --manifest-path <copy>/Cargo.toml --bin <temporary-package> --target-dir <repository>/crates/rubicon/target
```

All commands passed. Angular prerendered zero static routes. Eight library tests, four executable tests, and all six renderer fixtures passed. One macro-generated documentation example was ignored. LNK4098 persisted. The Angular root-component tests were not repeated for this adapter-only proposal; compilation and the actual HTTP page checks exercised the changed entrypoint.

The HTTP diagnostic printed:

```text
ANGULAR_HTTP_OK GET / 200 1670 bytes
ANGULAR_HTTP_OK GET /about 200 1583 bytes
ANGULAR_HTTP_OK GET /missing 404 9 bytes
ANGULAR_HTTP_OK GET /?sort=asc&page=2 200 1670 bytes
ANGULAR_HTTP_OK GET / 200 1670 bytes
ANGULAR_HTTP_OK HEAD / 200 0 bytes; Content-Length=1670
BROWSER_ASSET_PENDING /main.js 404
```

Successful GET page responses had text/html content type, expected Home/About headings, ng-server-context="ssr", and hydration annotations. All Home representations started with counter zero. The unknown route body was exactly Not found. HEAD had status 200, zero received body bytes, and Content-Length equal to the checked GET representation. The query case verifies route resolution when a query is present; the current application does not display query values.

The reference HTML's main.js request returned 404, establishing the immediate static-file gap. No browser hydration or interaction was checked through this Rust host. Byte counts describe this development artifact and are not optimization or performance evidence.

Local diagnostic output is retained only in ignored crates/rubicon/target/angular-http-probe.log and angular-http-server-stdout.log / angular-http-server-stderr.log. The temporary process and source copies were removed.

## Author Implementation and Reproduction

1. Apply the TypeScript entry and Rust path edits above.
2. Build the neutral application from examples/angular-demo:

   ```text
   npm.cmd run build -- --configuration development,rubicon
   ```

3. From the repository root, run:

   ```text
   cargo fmt --manifest-path crates/rubicon/Cargo.toml
   cargo check --locked --offline --manifest-path crates/rubicon/Cargo.toml --all-targets
   cargo test --locked --offline --manifest-path crates/rubicon/Cargo.toml
   cargo run --locked --offline --manifest-path crates/rubicon/Cargo.toml
   ```

4. In another terminal, request the server:

   ```text
   curl.exe -i "http://127.0.0.1:3000/"
   curl.exe -i "http://127.0.0.1:3000/about"
   curl.exe -i "http://127.0.0.1:3000/missing"
   curl.exe -I "http://127.0.0.1:3000/"
   ```

Home and About should return 200 and already contain their headings in the HTML source. The fallback should return 404 with Not found. HEAD should return 200 with headers and no body. Stop an older Rubicon process before starting the new executable on the same port.

After actual repository verification and its implementation commit, serve the matching dist/rubicon/browser assets through Rust, then repeat the established [hydration and counter checks](../../examples/angular-demo/README.md#browser-hydration-check).

## Limits and Remaining Work

This verifies selected development SSR pages through one persistent worker and the existing buffered HTTP response contract. It does not establish browser hydration, request isolation, concurrent rendering, cancellation, deadlines, bounded memory, native data access, public fetch, or performance superiority. There is no streaming response bridge. The fixed local origin, allowed hostnames, and build-time artifact path remain prototype configuration.

The [runtime decision](../adr/001-runtime-selection.md) remains Draft. Browser behavior, lifecycle guarantees, and controlled production benchmarks still require evidence.

## Sources

- Installed Angular 22.2.2 declarations for AngularAppEngine and createRequestHandler in node_modules/@angular/ssr/types.
- Existing [neutral build configuration](../../examples/angular-demo/README.md#neutral-build-for-rubicon).
- Existing [request type](../../crates/rubicon/src/request.rs), [response type](../../crates/rubicon/src/response.rs), [worker](../../crates/rubicon/src/renderer.rs), and [HTTP converter](../../crates/rubicon/src/http_response.rs).
- [Experiment 003](003-performance-angular-render.md) for performance initialization and the initial embedded-render diagnostic.
