# Experiment 001: Angular Neutral Bundle Compatibility

**Status:** Completed
**Created:** 2026-10-10
**Last updated:** 2026-10-10
**Type:** Exploratory correctness check; not a performance benchmark
**Source revision:** `96f66452a5a585557885d27a9f7f27b77cf50a21`
**Initial implementation scope:** Temporary Angular entrypoint and configuration; repository implementation and later prototypes are recorded separately below

## Question and Result

Can Angular's public non-Node SSR entrypoint be bundled without static Node imports and evaluated by Rubicon's existing bare deno_core runtime?

Yes, for the pinned example and development configuration checked here. A neutral Angular build passed. Its handler rendered Home and About when invoked in Node.js with real Web API objects. The same generated `server.mjs` loaded and evaluated successfully through Rubicon's `JavaScriptRuntime::load_main_module` in deno_core. No render request was invoked in deno_core. The runtime still lacks the Web APIs needed to proceed to that check.

This result supports continuing the compatibility prototype. It does not accept [ADR 001](../adr/001-runtime-selection.md), prove complete Angular execution in Rust, or establish a speed improvement.

## Environment and Temporary Changes

The check used Windows, rustc 1.97.1, deno_core 0.412.0, Tokio 1.53.2, Node.js v24.19.0, npm 11.17.0, and the example's installed Angular 22.2.2 dependencies. The source revision's Cargo.lock and package-lock.json remain unchanged.

The example's source and configuration were copied to a temporary directory. A junction reused its installed node_modules. The copy gained the following `src/server.rubicon.ts`:

```typescript
import { AngularAppEngine, createRequestHandler } from '@angular/ssr';

const angularApp = new AngularAppEngine();

export const reqHandler = createRequestHandler((request: Request) =>
  angularApp.handle(request),
);
```

The temporary `angular.json` gained a build configuration named `rubicon`, alongside `production` and `development`:

```json
"rubicon": {
  "ssr": {
    "entry": "src/server.rubicon.ts",
    "platform": "neutral"
  },
  "outputPath": "dist/rubicon"
}
```

The default Node entrypoint and output directory were retained. In the pinned builder's installed `src/builders/application/schema.json`, `ssr.platform` accepts `node` and `neutral`. Its esbuild setup conditionally adds the `node:module` createRequire banner only for Node output. This check used the official builder option and public `@angular/ssr` APIs; it did not remove imports from generated files or manually register Angular's private manifests.

## Build and Handler Checks

From the temporary example directory, with `NG_CLI_ANALYTICS=false`:

```text
npm.cmd run build -- --configuration development,rubicon
```

The build passed and reported zero prerendered routes. It generated seven `.mjs` files under `dist/rubicon/server`, including the engine and application manifests and their embedded HTML assets. The engine manifest points to `main.server.mjs`; the application manifest supplies bootstrap, route metadata, and HTML assets. These filenames describe this version and configuration, not a permanent Rubicon artifact contract.

A TypeScript parser walked import declarations, export declarations with module specifiers, and string-literal dynamic imports in the generated JavaScript. There were no static external imports. One dynamic external import remained: `xhr2` in `main.server.mjs`. It was not invoked by the checked routes. Relative dynamic imports still need to work during rendering; a successful initial load does not verify every possible branch.

An initial regular-expression import inventory matched comments and ordinary strings incorrectly and was discarded. The recorded import result comes from the parser scan. Future dependency audits must distinguish executable imports from text that merely resembles them.

A Node.js script imported the neutral `server.mjs` and called its `reqHandler` with real `Request` objects using `http://127.0.0.1:3000` as the URL origin. This was direct handler invocation, not an HTTP server on port 3000. It checked:

| Request | Observed result |
| --- | --- |
| GET `/` | Response status 200, Home heading, SSR marker, hydration annotations, counter zero |
| GET `/about` | Response status 200, About heading, SSR marker, hydration annotations |
| GET `/missing` | `null`, meaning the host must choose its fallback behavior |

The generated handler does not start a listener, serve browser assets, or implement Rubicon's `render(RenderRequest) -> RenderResponse` export. `createRequestHandler` marks the Web handler for Angular tooling. A later adapter must convert Rubicon's owned request object into a Web `Request` and return validated status, header pairs, and body content.

## Embedded Module Diagnostic

A separate temporary Rust executable depended on the unchanged Rubicon library at the source revision and pinned deno_core and Tokio to their existing versions. It initialized V8 on the main thread, created `JavaScriptRuntime`, and queried each global using `execute_i64` with `typeof globalThis.<name>`. It then called `load_main_module` for the generated neutral `server.mjs`, with a ten-second asynchronous diagnostic time limit.

```text
cargo run --offline --manifest-path <temporary-probe>/Cargo.toml --target-dir C:/Users/motri/Desktop/rubicon/crates/rubicon/target -- <temporary-example>/dist/rubicon/server/server.mjs
```

The temporary executable used `rubicon` as a path dependency and `deno_core = "=0.412.0"`, plus `tokio = { version = "=1.53.2", features = ["rt", "macros", "time"] }`. Its lockfile was newly resolved from the local cache; this command did not use `--locked`. It reused the repository's ignored target directory. The existing LNK4098 linker warning appeared again.

The module diagnostic was:

```rust
use std::path::Path;
use rubicon::JavaScriptRuntime;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), String> {
    deno_core::JsRuntime::init_platform(None);
    let mut runtime = JavaScriptRuntime::new()?;
    for name in ["console", "URL", "URLSearchParams", "TextEncoder", "TextDecoder",
                 "Headers", "Request", "Response", "AbortController", "ReadableStream",
                 "setTimeout", "clearTimeout", "queueMicrotask", "performance", "fetch"] {
        let exists = runtime.execute_i64(&format!("typeof globalThis.{name} === 'undefined' ? 0 : 1"))?;
        println!("GLOBAL {name}: {}", if exists == 1 { "present" } else { "missing" });
    }
    let path = std::env::args().nth(1).ok_or("Expected module path")?;
    match tokio::time::timeout(
        std::time::Duration::from_secs(10),
        runtime.load_main_module(Path::new(&path)),
    ).await {
        Ok(Ok(_)) => println!("MODULE_LOAD_OK"),
        Ok(Err(error)) => println!("MODULE_LOAD_ERROR: {error}"),
        Err(_) => return Err("Module loading exceeded diagnostic timeout".to_owned()),
    }
    Ok(())
}
```

Selected actual output:

```text
DYNAMIC_EXTERNAL_IMPORT main.server.mjs xhr2
NEUTRAL_STATIC_EXTERNAL_IMPORTS: none
WEB_HANDLER_OK / 200
WEB_HANDLER_OK /about 200
WEB_HANDLER_OK /missing null
GLOBAL console: present
GLOBAL URL: missing
GLOBAL URLSearchParams: missing
GLOBAL TextEncoder: missing
GLOBAL TextDecoder: missing
GLOBAL Headers: missing
GLOBAL Request: missing
GLOBAL Response: missing
GLOBAL AbortController: missing
GLOBAL ReadableStream: missing
GLOBAL setTimeout: missing
GLOBAL clearTimeout: missing
GLOBAL queueMicrotask: present
GLOBAL performance: missing
GLOBAL fetch: missing
MODULE_LOAD_OK
```

The global inventory identifies presence, not complete API conformance or proof that every missing global is required by this example. The Angular server application implementation directly uses URL, TextEncoder, Headers, Response, request abort signals, and ReadableStream. Required scheduler and other APIs must be verified during actual rendering. External data fetching is a separate capability check.

## Limits and Next Actions

The temporary directories and node_modules junction were removed after verification. Full local output remains in the ignored `crates/rubicon/target/angular-neutral-probe.log`; the selected evidence above is retained in Git. Temporary binaries and bundles are not archived. Reproduction requires rebuilding from the stated revision with the documented entrypoint and configuration. The author's running Node server and existing application build were not replaced.

The check establishes initial module loading and evaluation only in deno_core. It does not verify dynamic application bootstrap, rendering, browser hydration under Rust, static asset delivery, HEAD or cookie handling, native network operations, deadlines, cleanup, request isolation, or performance. The asynchronous diagnostic timeout does not establish interruption of blocking JavaScript.

1. Have the author add the neutral entrypoint and named configuration to the example, then verify the build and commit that implementation step.
2. Register compatible Web API implementations in the runtime and verify their event-loop behavior. Do not substitute empty timer or stream implementations to make Angular appear to work.
3. Add the `render` adapter and invoke Home and About through Rubicon's worker, including response conversion and recovery checks.
4. Serve the matching browser assets through the Rust host and repeat the established hydration, interaction, and navigation checks.
5. Complete service, isolation, and lifecycle checks before the [performance protocol](../specs/benchmark-protocol.md).

## Repository Entrypoint Verification

Later on 2026-10-10, the author applied the documented `server.rubicon.ts` and named `rubicon` configuration to the actual reference application, starting from commit `5574598`. The source matches the temporary prototype described above. A repository build using `npm.cmd run build -- --configuration development,rubicon` passed and generated zero prerendered routes under `examples/angular-demo/dist/rubicon`.

A separate temporary Node.js checker imported this actual `server.mjs` and repeated the three direct handler requests. Home and About returned status 200 and HTML content type, their expected headings, SSR markers, and hydration annotations; Home's counter was zero. The unknown route returned `null`. The checker was removed after validation.

The previously compiled `rubicon-neutral-probe.exe` was then run against the repository's new neutral output. It printed the same global inventory and `MODULE_LOAD_OK`. This reused a temporary executable compiled against the unchanged Rust sources; it is not a retained integration test, and no Angular render was invoked in deno_core. The regular Node output was not rebuilt for this step. The [application README](../../examples/angular-demo/README.md#neutral-build-for-rubicon) now documents the implemented neutral build.

## Web Foundation Prototype

After the neutral entrypoint was committed as `36e25f6`, a separate temporary copy of `crates/rubicon` tested the first runtime Web APIs. The project's Rust implementation remained unchanged. The copy added exact dependencies `deno_web = "=0.290.0"` and `deno_webidl = "=0.259.0"`, both compatible with deno_core 0.412.0 according to their downloaded crate manifests. The current deno_url crate is deprecated and points to deno_web; no deno_url dependency was added.

The runtime extension order was deno_webidl, deno_web, then a small Rubicon bootstrap extension. deno_web initialized a per-runtime BlobStore, no location, disabled optional CSS parser features, and the default broadcast channel. The bootstrap used `core.loadExtScript` to obtain URL/URLSearchParams from `ext:deno_web/00_url.js` and TextEncoder/TextDecoder from `ext:deno_web/08_text_encoding.js`, then defined those four globals as writable, configurable properties. These imports and initialization calls were checked against the pinned crate sources and executed in the prototype.

Two new runtime tests covered relative URL resolution, percent-decoded query values, repeated parameters, a Unicode query round trip, a UTF-8 byte round trip with Romanian characters and an emoji, and rejection of invalid UTF-8 in fatal decoding mode. The existing execution/conversion recovery test remained in the test set.

The first synchronous test run terminated with Windows status `0xc0000409`. A sequential run with captured output disabled exposed this V8 diagnostic:

```text
V8 posted a delayed task, but this isolate was created outside of a tokio runtime context and the delay cannot be honored. Enter a tokio runtime (e.g. via `tokio::runtime::Runtime::enter()`) before creating the `JsRuntime`.
```

The corrected prototype ran all three JavaScriptRuntime tests as `#[tokio::test(flavor = "current_thread")] async fn ...`, without changing their synchronous JavaScript operations. The actual worker already creates its runtime inside a current-thread Tokio executor. Future constructor callers must likewise create the isolate with an active Tokio context; a synchronous constructor signature does not remove that requirement. Test serialization alone did not resolve the failure.

The corrected temporary package retained the library name `rubicon` but used package name `rubicon-web-probe`, avoiding replacement of the author's normal application executable. It reused the ignored target directory. Validation passed with these commands, where `<copy>` is the temporary crate:

```text
cargo fmt --manifest-path <copy>/Cargo.toml
cargo check --offline --manifest-path <copy>/Cargo.toml --all-targets --target-dir C:/Users/motri/Desktop/rubicon/crates/rubicon/target
cargo test --locked --offline --manifest-path <copy>/Cargo.toml --target-dir C:/Users/motri/Desktop/rubicon/crates/rubicon/target
```

The all-target check passed. Eight library tests and four executable tests passed; both synchronous and asynchronous renderer fixtures passed. One macro-generated documentation example was ignored. The corrected tests used the default test harness concurrency, rather than relying on `--test-threads=1`. LNK4098 remained present. The temporary crate was removed; the latest full local output is in ignored `crates/rubicon/target/web-foundation-probe.log`. Its copied lockfile was updated for the new dependencies; the project's Cargo.lock was not modified.

This verifies a proposed setup for four Web globals and the required test context. The author still needs to apply these Rust and JavaScript changes to the actual crate. It does not provide Request/Response/Headers, streams, abort support, public timers, performance, fetch, or Angular rendering. Those capabilities remain separate steps.

## Repository Web Foundation Verification

The author applied the URL/UTF-8 proposal to the actual crate, starting from commit `6314901`: exact deno_web and deno_webidl dependencies and Cargo.lock, the bootstrap extension in lib.rs, `js/web.js`, and two API tests. The existing runtime recovery test now also uses a current-thread Tokio test context. A review found that the original deno_core import had been removed; a temporary corrected-copy check passed, and the author then restored the import in the repository.

On 2026-10-10, repository formatting verification, locked offline compilation for all targets, and the full locked offline test command passed. Eight library tests, four executable tests, and both renderer fixtures passed, with one ignored macro-generated documentation example. The commands and scope are recorded in the [architecture milestone](../specs/architecture.md#url-and-utf8-runtime-verification). LNK4098 remains. This implements four selected Web globals; public timers, other Web API families, actual embedded Angular rendering, and performance remain pending.

## Web Timers Prototype

A temporary copy of the Web foundation sources committed as `58f331c` checked the next proposal: exposing setTimeout, clearTimeout, setInterval, and clearInterval from the pinned `ext:deno_web/02_timers.js` through the existing bootstrap. No dependency or worker change was needed. The registered deno_web extension already supplies the underlying operations; this proposal adds the four public globals.

The copy added `tests/fixtures/renderer_timers.mjs` and included it as the third module in the existing integration executable, retaining a fresh worker for each fixture. The exact proposed fixture was:

```javascript
import { render as renderSync } from "./renderer_errors.mjs";

export async function render(request) {
    let cancelledTimerFired = false;
    const cancelledTimer = setTimeout(() => {
        cancelledTimerFired = true;
    }, 0);
    clearTimeout(cancelledTimer);

    let ticks = 0;

    await new Promise((resolve) => {
        const interval = setInterval(() => {
            ticks += 1;
            if (ticks === 2) {
                clearInterval(interval);
                resolve();
            }
        }, 1);
    });

    await new Promise((resolve) => setTimeout(resolve, 5));

    if (cancelledTimerFired) {
        throw new Error("A cancelled timeout executed");
    }
    if (ticks !== 2) {
        throw new Error("A cleared interval kept executing");
    }

    return renderSync(request);
}

```

Each of the existing five integration calls now awaited actual timer callbacks before delegating to the response/error fixture. A zero-delay timeout was cancelled immediately and must not fire. The interval had to invoke its callback twice and then stop; the later five-millisecond timeout provided an observation window in which an incorrect clear operation could be detected. These delays are inputs for a correctness check, not latency or timer-precision measurements.

The temporary copy used these commands:

```text
cargo fmt --manifest-path <copy>/Cargo.toml
cargo check --locked --offline --manifest-path <copy>/Cargo.toml --all-targets --target-dir C:/Users/motri/Desktop/rubicon/crates/rubicon/target
cargo test --locked --offline --manifest-path <copy>/Cargo.toml --target-dir C:/Users/motri/Desktop/rubicon/crates/rubicon/target
```

Formatting, locked offline all-target compilation, and the full locked offline test command passed in the copy, using the repository's ignored target directory. Eight library tests, four executable tests, and all three fixture success messages passed. The new fixture preserved the existing response comparisons, Promise rejection on call 2, body-conversion failure on call 4, and valid subsequent calls. One macro-generated documentation example was ignored, and LNK4098 persisted. The temporary package used the name `rubicon-timer-probe` with library name `rubicon` and corresponding root lockfile rename so it did not replace the normal executable. Its directories were removed; the full local output remains in ignored `crates/rubicon/target/timer-foundation-probe.log`.

This is a verified proposal awaiting author implementation. It checks timer fulfillment and explicit JavaScript timer cancellation within controlled renders. It does not establish request cancellation, general cleanup of application timers, interruption of blocking JavaScript, HTTP deadline behavior, streams, Fetch objects, Angular rendering, or performance.

## Repository Web Timers Verification

On 2026-10-10, the author applied the timer proposal to the actual crate, starting from `0944362`. The bootstrap exposes the four timer globals, `renderer_timers.mjs` matches the checked proposal, and the integration executable includes it as a third fixture. Dependencies and worker scheduling are unchanged.

Repository formatting verification, locked offline all-target compilation, and the full locked offline test command passed. Eight library tests, four executable tests, and all three fixture success messages passed, with one ignored macro-generated documentation example. The timer fixture retains exact response and failure comparisons across five calls. The [architecture milestone](../specs/architecture.md#web-timers-verification) records the behavior and limits. LNK4098 remains present.

Timer callback execution and explicit cancellation are now implemented and verified. This does not establish a host request-cancellation policy, automatic application timer cleanup, deadline enforcement, embedded Angular rendering, or performance.

## Sources

- [Angular non-Node SSR configuration](https://angular.dev/guide/ssr#non-nodejs), checked on 2026-10-10.
- [AngularAppEngine](https://angular.dev/api/ssr/AngularAppEngine), checked on 2026-10-10.
- [createRequestHandler](https://angular.dev/api/ssr/createRequestHandler), checked on 2026-10-10.
- [deno_web](https://docs.rs/crate/deno_web/latest) and [deno_webidl](https://docs.rs/crate/deno_webidl/latest), rolling crate pages checked on 2026-10-10; prototype versions pinned to 0.290.0 and 0.259.0 and verified from downloaded manifests and source.
- Installed Angular 22.2.2 builder schema and esbuild source, and `@angular/ssr/fesm2022/ssr.mjs`, resolved by the example's package-lock.json at the source revision.

Rolling documentation explains the supported API direction. The observed compatibility result is limited to the pinned dependencies and generated artifacts tested here.
