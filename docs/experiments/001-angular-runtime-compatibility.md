# Experiment 001: Angular Neutral Bundle Compatibility

**Status:** Completed
**Created:** 2026-10-10
**Last updated:** 2026-10-10
**Type:** Exploratory correctness check; not a performance benchmark
**Source revision:** `96f66452a5a585557885d27a9f7f27b77cf50a21`
**Implementation scope:** Temporary Angular entrypoint and configuration; no application or framework source changes committed

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

## Sources

- [Angular non-Node SSR configuration](https://angular.dev/guide/ssr#non-nodejs), checked on 2026-10-10.
- [AngularAppEngine](https://angular.dev/api/ssr/AngularAppEngine), checked on 2026-10-10.
- [createRequestHandler](https://angular.dev/api/ssr/createRequestHandler), checked on 2026-10-10.
- Installed Angular 22.2.2 builder schema and esbuild source, and `@angular/ssr/fesm2022/ssr.mjs`, resolved by the example's package-lock.json at the source revision.

Rolling documentation explains the supported API direction. The observed compatibility result is limited to the pinned dependencies and generated artifacts tested here.
