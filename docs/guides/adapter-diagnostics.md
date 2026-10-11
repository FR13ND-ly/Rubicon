# Angular Adapter Stage Diagnostics

**Status:** Author implementation pending; isolated preview verified
**Created:** 2026-10-11
**Last updated:** 2026-10-11
**Base:** b8de828
**Context:** Follow-up to [experiment 008](../experiments/008-runtime-stage-diagnostic.md)
**Verification:** [adapter-preflight.json](../../benchmarks/diagnostics/adapter-preflight.json)

## Purpose and Boundaries

Separate obtaining Angular's Response from consuming its body inside the existing execute interval. The previous verified warmed runs locate 98.6–99.3% of enclosing call wall time in execution/waits. This step adds observable boundaries before investigating causes.

| Event stage | Boundary | Included work |
| --- | --- | --- |
| angular_handle | Immediately before reqHandler(webRequest) through the awaited result or exception. | Angular handling and asynchronous waits until a Response or null is available. Request construction precedes this stage. |
| body_read | Immediately before response.text() through the awaited string or exception. | Stream consumption, UTF-8 decoding, body production that happens during consumption, and asynchronous waits. Header extraction precedes this stage. |

A returned Response does not establish that all HTML has been produced. body_read can include remaining rendering work for a lazy/streamed body. These are wall-time boundaries, not Angular CPU versus pure copying. Their sum fits inside the enclosing execute interval; it omits request/header work, event emission, and other bookkeeping. Do not add nested timings or separate percentiles as total latency.

The private optional __rubiconAdapterTiming bridge is installed before application module evaluation only in render-timing builds. TypeScript reads it once at module load. Numeric codes 1 and 2 select the fixed stage names. Rust emits through the existing tracing subscriber and bounded asynchronous writer. The current Renderer Instrument span supplies request_id across suspension; the adapter does not convert the ID to a JavaScript number or maintain a global current-request variable. This correlation applies to the current single-active-call runtime; future concurrent calls in one isolate require separate validation.

## 1. Create crates/rubicon/src/diagnostics.rs

Paste the following entire file. op2 exposes the Rust function to JavaScript. It validates the converted stage code and finite nonnegative duration, converts milliseconds to integer microseconds by flooring, and emits a fixed event. false means invalid input was rejected before emission; true means emission was attempted, not that an HTTP response was correct or that the writer retained the line.

```rust
use deno_core::op2;

#[op2(fast)]
fn op_rubicon_adapter_timing(stage: u32, elapsed_ms: f64, ok: bool) -> bool {
    let stage = match stage {
        1 => "angular_handle",
        2 => "body_read",
        _ => return false,
    };

    let elapsed_us = elapsed_ms * 1000.0;
    if !elapsed_us.is_finite() || elapsed_us < 0.0 || elapsed_us >= u64::MAX as f64 {
        return false;
    }

    tracing::info!(
        target: "rubicon::adapter_timing",
        stage,
        elapsed_us = elapsed_us.floor() as u64,
        ok,
        "adapter_stage_timing",
    );

    true
}

deno_core::extension!(
    rubicon_diagnostics,
    deps = [rubicon_web],
    ops = [op_rubicon_adapter_timing],
    esm_entry_point = "ext:rubicon_diagnostics/diagnostics.js",
    esm = [dir "js", "diagnostics.js"],
);

pub(crate) fn extension() -> deno_core::Extension {
    rubicon_diagnostics::init()
}
```

The source file is compiled through the gated module declaration in the next step. Existing Cargo features/dependencies provide op2 and tracing; this proposal requires no dependency changes.

## 2. Create crates/rubicon/js/diagnostics.js

Paste the following entire file. The extension imports its native function and exposes a frozen optional bridge. The application adapter imports no ext: module directly.

```javascript
import { op_rubicon_adapter_timing } from 'ext:core/ops';

Object.defineProperty(globalThis, '__rubiconAdapterTiming', {
  value: Object.freeze({ record: op_rubicon_adapter_timing }),
  writable: false,
  configurable: false,
});
```

## 3. Update crates/rubicon/src/lib.rs

Immediately before the existing mod http_response; declaration, add:

```rust
#[cfg(feature = "render-timing")]
mod diagnostics;
```

Replace the entire existing pub fn new() method inside impl JavaScriptRuntime with this method:

```rust
pub fn new() -> Result<Self, String> {
    let extensions = vec![
        deno_webidl::deno_webidl::init(),
        deno_web::deno_web::init(
            Arc::new(deno_web::BlobStore::default()),
            None,
            false,
            Default::default(),
        ),
        deno_net::deno_net::init(None, None),
        deno_fetch::deno_fetch::init(Default::default()),
        rubicon_web::init(),
    ];

    #[cfg(feature = "render-timing")]
    let extensions = {
        let mut extensions = extensions;
        extensions.push(diagnostics::extension());
        extensions
    };

    let inner = JsRuntime::try_new(RuntimeOptions {
        module_loader: Some(Rc::new(FsModuleLoader)),
        extensions,
        ..Default::default()
    })
    .map_err(|error| format!("Runtime initialization failed: {error}"))?;

    Ok(Self { inner })
}
```

The Web extensions remain first. The feature-gated block appends diagnostics after rubicon_web, matching its declared dependency. call_export and Renderer already provide the enclosing timers and span context. RenderRequest and RenderResponse retain their existing fields.

## 4. Replace examples/angular-demo/src/server.rubicon.ts

Paste the entire file below. The two try/finally blocks measure both success and failure without catching or replacing the original operation's exception. Each timer stops before the record call. With the bridge absent, performance.now() and record are not called; the small branch/assignment changes still belong to the new baseline and have no measured overhead claim.

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

type AdapterTiming = {
  record(stage: 1 | 2, elapsedMs: number, ok: boolean): boolean;
};

const adapterTiming = (
  globalThis as typeof globalThis & {
    __rubiconAdapterTiming?: AdapterTiming;
  }
).__rubiconAdapterTiming;

export async function render(
  request: RubiconRequest,
): Promise<RubiconResponse> {
  const webRequest = new Request(request.url, {
    method: request.method,
    headers: new Headers(request.headers),
  });

  const angularStarted = adapterTiming ? performance.now() : 0;
  let angularOk = false;
  let response: Response | null;

  try {
    response = await reqHandler(webRequest);
    angularOk = true;
  } finally {
    if (adapterTiming) {
      adapterTiming.record(1, performance.now() - angularStarted, angularOk);
    }
  }

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

  const bodyStarted = adapterTiming ? performance.now() : 0;
  let bodyOk = false;
  let body: string;

  try {
    body = await response.text();
    bodyOk = true;
  } finally {
    if (adapterTiming) {
      adapterTiming.record(2, performance.now() - bodyStarted, bodyOk);
    }
  }

  return { status: response.status, headers, body };
}
```

The stage codes match the native mapping: 1 is angular_handle and 2 is body_read. angularOk becomes true when the handler returns, including a null/unhandled route. A null result returns the existing 404 without a body-read stage. bodyOk becomes true only after the body string is available. Response status, repeated cookies, and body transport retain their existing meaning. Timing records use the diagnostic channel rather than response metadata.

## Build and Check After Applying

Run from the repository root. Stop a running host before rebuilding its loaded Angular files or executable. Previous production Angular graphs were copied under ignored crates/rubicon/target/diagnostics-artifacts/angular-fc3f69c-production, with full file hashes and eight matches to experiment 008's recorded inputs. Preserve those baseline graphs and retained binaries.

```powershell
cargo fmt --manifest-path crates/rubicon/Cargo.toml
cargo test --locked --manifest-path crates/rubicon/Cargo.toml
cargo test --locked --manifest-path crates/rubicon/Cargo.toml --features render-timing
npm.cmd --prefix examples/angular-demo run build -- --configuration production,rubicon
cargo run --release --locked --manifest-path crates/rubicon/Cargo.toml --features render-timing
```

Use a second terminal:

```powershell
curl.exe -i "http://127.0.0.1:3000/"
curl.exe -i "http://127.0.0.1:3000/about?query=rust"
curl.exe -I "http://127.0.0.1:3000/"
curl.exe -i "http://127.0.0.1:3000/missing"
```

The first two requests should return correct Home/About HTML with status 200. HEAD should deliver no body. The missing route should return the existing 404. A successful handled render should emit, in order, prepare, angular_handle, body_read, execute, convert, then coarse render_timing. Each adapter event should contain the same render span ID as its enclosing runtime events:

```text
INFO render{request_id=1}: adapter_stage_timing stage="angular_handle" elapsed_us=<integer> ok=true
INFO render{request_id=1}: adapter_stage_timing stage="body_read" elapsed_us=<integer> ok=true
```

For an unhandled route, expect only angular_handle from the adapter, with ok=true, followed by successful runtime conversion and the application 404. A handler exception emits angular_handle with ok=false; a body exception emits a successful handler stage and body_read with ok=false. Original exceptions propagate to the existing rendering error path. Panic, cancellation, or a never-settling operation does not establish a completed diagnostic sequence.

Stop the host with Ctrl+C and check RUBICON_DIAGNOSTICS_DROPPED lines=0. Then run the host without --features render-timing and repeat the HTTP gates: the private bridge and timing events should be absent. Commit the four author-applied source files only after verification, with a proposed message feat(diagnostics): split Angular handling and body consumption. Update this guide's implementation status using the actual repository evidence.

## Isolated Verification and Limits

The exact proposed code was prepared under ignored target/adapter-stage-preview and examples/angular-demo/tmp/adapter-stage-preview. The preview crate uses a distinct package identity with the same dependency graph and a shared dependency build cache. Its final feature-on and feature-off runs verify eight existing library tests and all six renderer integration fixtures each. A separate asynchronous probe executes the exact proposed TypeScript adapter against controllable handler/body failures, null, UTF-8 content, repeated cookies, and recovery. A production neutral Angular build also passes and its actual module renders Home, About with query, an unhandled route, and HEAD through both embedded configurations.

The instrumented probe verifies 15 adapter events, 25 runtime events, and nine enclosing coarse events. All adapter stages carry the expected ID after asynchronous suspension, have the expected reached-stage/error sequences, and fit their enclosing execute duration. Five invalid native inputs are refused. The bridge is present/frozen only with the feature, and the bounded writer reports zero losses after flushing. The existing linker and incremental-cache warnings remain nonfatal.

These are isolated collection/correctness checks with a debug Rust host, not performance results. They do not replace author-applied repository verification, an HTTP listener check, or browser/hydration checks. Framework/application source and the original production artifact paths were unchanged while preparing this guide.

The current benchmark analyzers validate runtime-stage events only. Before a new experiment, add an adapter-event parser/join with explicit handled/null/error sequences, provenance checks, and replay. Successful handled exports now produce six diagnostic records instead of four. Reevaluate logger pressure and feature-on/off cost using newly retained binaries and a declared protocol; do not reuse experiment 008's binary/build hashes. Outer HTTP intervals, client/server joining, warm-up stalls, and thread CPU attribution remain subsequent investigations.

## Primary API Notes

[Official op2 notes](https://github.com/denoland/deno_core/blob/main/ops/op2/README.md) describe native bindings and fastcall annotations. The code above was checked against the repository's installed deno_core 0.412.0/deno_ops 0.288.0 sources and executed in the isolated preview; an online main-branch description alone does not establish compatibility with the locked versions.
