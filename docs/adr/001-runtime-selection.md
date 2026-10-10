# ADR 001: JavaScript Runtime Selection

**Date:** 2026-10-09
**Last updated:** 2026-10-10
**Status:** Draft
**Decision scope:** Prototype

## Context

Rubicon proposes a Rust HTTP server that delegates Angular SSR to JavaScript execution while retaining native application services in Rust. A runtime integration must load the selected Angular artifacts, support their required environment, drive asynchronous rendering, and return correct HTTP response semantics.

Embedding an engine and executing a small JavaScript function does not by itself establish compatibility with Angular SSR. Module loading, required Web or Node APIs, Angular lifecycle behavior, hydration, and native-operation cleanup must be tested.

No runtime has been benchmarked in this project. This ADR records a candidate for a compatibility prototype and conditions for a later accepted decision.

## Decision Drivers

- Keep HTTP ownership and application-service integration in Rust.
- Support compiled Angular server artifacts and correct browser hydration.
- Provide an explicit boundary for asynchronous native Rust operations.
- Permit bounded scheduling, cancellation, cleanup, and worker recovery.
- Fit the implementation and maintenance effort within the thesis scope.
- Evaluate latency, correct throughput, memory, CPU cost, and startup under the benchmark protocol.
- Preserve application-facing semantics if a future runtime adapter is introduced.

## Options Considered

| Option | Benefits | Costs and limitations | Evidence or unknowns |
| --- | --- | --- | --- |
| deno_core with selected extensions | Rust embedding APIs, V8 execution, explicit native operations, and control of integration. | Requires construction of the needed environment and module-loading behavior. | Core capabilities are documented; Angular compatibility and performance are unverified. |
| deno_runtime | Assembles a broader Deno environment on top of deno_core. | More dependencies and configuration; application module resolution still needs integration. | The runtime source demonstrates Web/API extensions; fit for this Angular build is unverified. |
| Persistent Bun process controlled by Rust | Alternative renderer environment with an independent lifecycle. | IPC, process management, data transport, and total process memory must be counted. | Angular execution and equivalent request semantics require a prototype. |
| Bun embedding through a community project or future official API | Potential in-process alternative using JavaScriptCore. | Supported platforms, upstream coupling, lifecycle guarantees, and packaging need review. | A community libbun project exists; the official embedding request remains open as checked on 2026-10-09. |

Bun's move to Rust concerns its runtime implementation; JavaScript execution still uses JavaScriptCore. The implementation language does not establish which option renders this Angular workload faster.

Node.js remains the conventional SSR reference for evaluation. A Rust host invoking a Node process could also be investigated if it answers a useful integration question, but it is not the proposed initial embedded-runtime experiment.

## Decision

**Propose deno_core with the minimum verified extensions as the first prototype candidate.**

This is a draft proposal, not an accepted project-wide commitment. Acceptance requires a working Angular SSR and hydration example, asynchronous native-service integration, request isolation, and verified cleanup after success and failure.

The prototype should use the public Angular rendering surface where practical, with AngularAppEngine as an initial integration candidate. It must identify the complete required environment rather than assume that its Request/Response interface guarantees execution inside bare deno_core.

The application-facing rendering contract should remain separate from engine-specific initialization, module resolution, native operations, event-loop handling, and interruption. Exact adapter interfaces remain TBD until the compatibility spike.

## Rationale

deno_core supplies an embeddable JavaScript execution context and a mechanism for registering Rust operations. This makes it a direct candidate for the proposed host and native-data boundary.

Its event loop must be driven by the embedding application, and additional runtime functionality must be supplied where required. The benefit is explicit integration control; the cost is implementation work.

Starting with this candidate tests the central research assumption: whether Angular SSR can operate correctly within a Rust-owned execution environment. It does not establish that a minimal environment is the fastest or easiest long-term choice.

If the required compatibility layer becomes substantial, deno_runtime may be a more practical candidate. If a Bun process or embedding option is evaluated, its complete system cost and equivalent behavior must be measured.

## Consequences

### Benefits

The prototype can keep native service access and HTTP control within the Rust host and expose their behavior through a documented rendering contract. The runtime adapter creates a boundary for experimentation without requiring application components to know the engine internals.

### Costs

The implementation must provide or configure required APIs, resolve the compiled modules, drive asynchronous work, and verify interruption and cleanup. Runtime workers must respect the selected engine's thread ownership requirements.

Dependency and engine upgrades require compatibility checks against the selected Angular version and application artifacts.

### Limits

This choice does not implement native Angular rendering, frontend WASM acceleration, private Ivy rewriting, or a new Angular compiler. It also does not establish support for arbitrary Angular dependencies requiring Node APIs.

No latency, throughput, startup, or memory improvement has been measured.

### Follow-up

1. Pin Angular, Rust, deno_core, and relevant extension versions.
2. Execute a small module and verify its asynchronous behavior in the embedded runtime. Local import, Promise fulfillment, rejection after an awaited microtask, and continued execution are verified below; asynchronous native-service integration remains a separate condition.
3. Load a minimal Angular server artifact and obtain correct HTML.
4. Serve that HTML with its matching browser assets and verify hydration.
5. Demonstrate asynchronous native data access and request isolation.
6. Verify failure cleanup, deadline behavior, and worker recovery.
7. Review this ADR before accepting the runtime choice for the prototype.

## Reconsideration Conditions

Revisit the proposal if:

- The selected Angular build requires an impractical compatibility layer.
- Correctness, hydration, isolation, or bounded recovery cannot be demonstrated.
- Dependency maintenance or platform support exceeds the thesis budget.
- Measurements show a tradeoff inconsistent with the project's selected workload and resource constraints.
- A usable alternative embedding or process integration offers relevant capabilities and can be evaluated with equivalent behavior.

A replacement must first satisfy the rendering contract and correctness checks. Performance evidence must include engine, transport, process, and resource differences. Changing a runtime adapter may also require changes to worker lifecycle and packaging.

If a significant accepted decision is later superseded, create a replacement ADR and retain this document's original reasoning.

## Evidence and Related Documents

- [Project overview](../specs/project-overview.md).
- [Architecture](../specs/architecture.md).
- [Rendering contract](../specs/rendering-contract.md).
- [Benchmark protocol](../specs/benchmark-protocol.md).
- [Deno Core README](https://github.com/denoland/deno/blob/main/libs/core/README.md), rolling main branch checked on 2026-10-09.
- [deno_core JsRuntime](https://docs.rs/deno_core/latest/deno_core/struct.JsRuntime.html), documentation observed at 0.412.0 on 2026-10-09; the first synchronous execution prototype pins deno_core to 0.412.0 in Cargo.toml and records resolved dependencies in Cargo.lock.
- [Deno runtime implementation](https://docs.rs/deno_runtime/latest/src/deno_runtime/worker.rs.html), rolling latest documentation checked on 2026-10-09; selected dependency version TBD.
- [AngularAppEngine API](https://angular.dev/api/ssr/AngularAppEngine), rolling documentation checked on 2026-10-10; reference application pinned to Angular 22.2.2.
- [Bun Rust rewrite announcement](https://bun.com/blog/bun-in-rust), checked on 2026-10-09.
- [Official Bun embedding feature request](https://github.com/oven-sh/bun/issues/12017), open status checked on 2026-10-09.
- [Community libbun project](https://github.com/enki/libbun), project claims and documented integration checked on 2026-10-09; no local validation performed.

These sources support the option descriptions. They do not establish this project's Angular compatibility or performance. Record exact dependency versions and source commits for each subsequent prototype and experiment.

### First Synchronous Execution Prototype

On 2026-10-09, the author's Rust implementation successfully evaluated `4 + 2` through deno_core 0.412.0, converted the V8 result to a Rust `i64`, and printed `JavaScript result: 6` from the executable. Initialization, execution, and conversion errors are mapped to strings and propagated through `Result`; their failure paths have not yet been exercised in this validation.

The following checks completed successfully:

```text
cargo fmt --manifest-path crates/rubicon/Cargo.toml -- --check
cargo check --locked --offline --manifest-path crates/rubicon/Cargo.toml
cargo run --locked --offline --manifest-path crates/rubicon/Cargo.toml
```

The executable build emitted linker warning LNK4098 about `libcmt.lib` conflicting with other libraries. Execution still completed successfully; the warning remains unresolved and should be investigated before treating this build configuration as a benchmark reference.

That initial revision created and dropped a runtime for every function call. It established synchronous engine embedding and numeric result conversion only. Persistent runtime reuse, ES modules, asynchronous work, Angular SSR, hydration, request isolation, and performance were unverified at that milestone. This evidence did not change the ADR's Draft status.

### Reusing a Runtime Instance

On 2026-10-09, the author replaced the free evaluation function with a `JavaScriptRuntime` type that owns a private `JsRuntime`. Its constructor initializes the engine once; `execute_i64(&mut self, source)` evaluates subsequent synchronous scripts using that existing instance and converts each result to a Rust `i64`.

The executable initializes `globalThis.counter` to zero, then increments it in two separate evaluations. The formatting, locked offline compilation, and locked offline execution checks listed above completed successfully again. Execution produced:

```text
First execution: 1
Second execution: 2
```

This verified sequential reuse and persistence of JavaScript global state within one runtime. It also demonstrated why request data must later be scoped explicitly: a reused execution context retains globals between calls. At that milestone, separate-instance isolation, recovery after JavaScript exceptions or conversion failures, worker scheduling, ES modules, asynchronous work, Angular SSR, and performance were unverified. Linker warning LNK4098 persisted. The ADR remained Draft.

### Continuing After Synchronous Errors

On 2026-10-09, the author added the regression test `runtime_recovers_after_execution_and_conversion_errors`. It uses one runtime on one thread for the entire sequence: initialize a counter, execute a script that throws an ordinary JavaScript exception, increment the counter, evaluate an object that cannot be converted to `i64`, and increment the counter again.

The test checks the execution and conversion error categories separately and requires the counter increments to return 1 and 2 after the respective failures. This verifies error propagation and subsequent valid execution without replacing the runtime or resetting its global state.

The following checks completed successfully:

```text
cargo fmt --manifest-path crates/rubicon/Cargo.toml -- --check
cargo test --locked --offline --manifest-path crates/rubicon/Cargo.toml --lib
```

The test result was one passed, zero failed. Linker warning LNK4098 also occurred when building the test executable and remains unresolved. Test harness duration is not performance evidence.

This verification covers the specific ordinary synchronous exception and conversion failure used in the test. It does not establish rollback of script side effects, recovery from resource exhaustion or termination, asynchronous rejection handling, request cleanup, separate-instance isolation, or Angular SSR compatibility. The ADR remains Draft.

### Local ES Module Returning HTML

On 2026-10-09, the author configured `FsModuleLoader`, added `load_main_module` and `call_string_export`, and pinned Tokio 1.53.2 as a direct dependency with the `rt` and `macros` features. The executable uses Tokio's current-thread runtime, loads `js/render.mjs` once, evaluates it while driving the JavaScript event loop, and calls its exported `render()` function twice on the same embedded runtime.

The module exports a synchronous function returning `<h1>Rubicon</h1>`. Each call looks up the function in the evaluated module namespace, drives the call future with the JavaScript event loop, and converts the returned V8 value to a Rust `String`. Module path resolution starts from the package's compile-time manifest directory, avoiding dependence on the shell's current directory. The file loader uses synchronous filesystem reads during module loading; this prototype performs that loading before the two render calls.

The following checks completed successfully:

```text
cargo fmt --manifest-path crates/rubicon/Cargo.toml -- --check
cargo check --locked --offline --manifest-path crates/rubicon/Cargo.toml
cargo test --locked --offline --manifest-path crates/rubicon/Cargo.toml --lib
cargo run --locked --offline --manifest-path crates/rubicon/Cargo.toml
```

The existing synchronous-error regression test still passed, with one test passed and zero failed. Execution produced:

```text
First render: <h1>Rubicon</h1>
Second render: <h1>Rubicon</h1>
```

This verifies local module loading, evaluation, exported-function access, repeated invocation, and HTML string transfer to Rust. The Rust orchestration is asynchronous, but the tested JavaScript renderer is synchronous. Promise-returning renderers, import graphs, module failure recovery, HTTP serving, worker scheduling, Angular SSR, hydration, request isolation, and performance remain unverified. Linker warning LNK4098 persists in both the test executable and application executable. The ADR remains Draft.

### Asynchronous Renderer Verification

On 2026-10-10, the author added an async fixture that imports the existing synchronous renderer and awaits `Promise.resolve()` before delegating to it. The worker integration executable now runs the same response and failure checks for both entrypoints, each in a fresh worker initialized after the process-wide V8 platform setup. No engine or runtime implementation change was needed.

Repository formatting, locked offline compilation for all targets, and the complete test suite passed. Six library and four executable unit tests succeeded. The integration executable printed `Renderer fixture passed: renderer_errors.mjs` and `Renderer fixture passed: renderer_async.mjs`. Both paths retain exact structured response metadata and request fields; the async path converts successful Promise values, reports the intentional rejection, rejects an invalid body shape after fulfillment, and returns valid later results. Linker warning LNK4098 persists. The [architecture evidence](../specs/architecture.md#asynchronous-renderer-verification) records the full commands and cases.

This verifies a small local import graph and Promise/microtask behavior for the pinned deno_core candidate. It does not verify the Angular artifact environment, timers or native I/O, cancellation, cleanup, isolation, or performance. The ADR remains Draft, with acceptance conditions unchanged.

### Angular Node.js Reference

On 2026-10-10, the author added a separate Angular 22.2.2 SSR application with exact direct Angular version pins and package-lock.json. Its conventional Node.js production build and HTTP delivery passed: the welcome heading was present in server-rendered HTML, and the matching browser JavaScript asset was served successfully. Local host validation is explicitly configured, and the build prerenders zero routes. The [architecture evidence](../specs/architecture.md#angular-nodejs-reference-verification) records the environment, checks, and resolved setup failures; the [application README](../../examples/angular-demo/README.md) provides reproduction commands.

The generated Node entrypoint is a reference implementation for subsequent artifact and environment inspection. It does not prove execution inside deno_core, successful browser hydration, asynchronous native services, isolation, cleanup, or a performance improvement. This milestone selects the first Angular version to investigate but leaves the runtime decision and its acceptance conditions in Draft.

### Angular Routes and Hydration

Later on 2026-10-10, the Node.js reference application added Home and About pages, shared navigation, and a signal-based counter. The repository's two root-component tests and development build passed. Direct HTTP requests returned SSR content for both routes, and browser checks confirmed hydration diagnostics, counter increments, navigation, and direct About reload without captured warnings or errors. The [architecture evidence](../specs/architecture.md#angular-routes-and-hydration-verification) records the observations and the [application README](../../examples/angular-demo/README.md#browser-hydration-check) explains reproduction.

This establishes a small Angular SSR and hydration reference on Node.js. It does not establish compatibility with the Rust-owned runtime. The same application must still pass after integration into deno_core, and native services, isolation, cleanup, and performance remain unverified. The runtime decision remains Draft.

### Angular Neutral Bundle Diagnostic

The [compatibility experiment](../experiments/001-angular-runtime-compatibility.md) built a public, platform-neutral Angular entrypoint in a temporary copy and successfully loaded and evaluated its generated server module through the unchanged deno_core runtime. Its handler rendered the reference pages when invoked in Node.js with real Web API objects. Missing Web globals were inventoried in deno_core, but no embedded Angular render was invoked. The prototype therefore has a verified entrypoint direction; runtime APIs, complete embedded SSR, hydration, isolation, cleanup, and performance still require evidence. The ADR remains Draft.

## Decision History

| Date | Change | Reason |
| --- | --- | --- |
| 2026-10-09 | Draft created and expanded. | Record deno_core as a prototype candidate, alternatives, and validation conditions. |
| 2026-10-09 | Record first synchronous execution and selected deno_core version. | Preserve the verified result, exact checks, unresolved linker warning, and remaining compatibility work. |
| 2026-10-09 | Record reuse of one runtime instance. | Verify sequential execution and retained global state while keeping request isolation and performance claims separate. |
| 2026-10-09 | Record continued execution after synchronous errors. | Verify execution and conversion error propagation and valid subsequent execution on the same runtime. |
| 2026-10-09 | Record local ES module loading and HTML return values. | Verify loading once and calling the synchronous exported renderer twice through asynchronous Rust orchestration. |
| 2026-10-10 | Record Promise-returning renderer checks. | Verify a local import, fulfillment, rejection after an awaited microtask, typed response conversion, and continued execution while keeping native I/O and Angular acceptance pending. |
| 2026-10-10 | Record Angular 22.2.2 on the conventional Node.js server. | Establish a pinned application and verified SSR reference before embedded-runtime compatibility work. |
| 2026-10-10 | Record routed pages, counter interaction, and Node.js reference hydration. | Establish observable browser behavior to repeat after embedded-runtime integration. |
| 2026-10-10 | Record neutral bundle loading and missing Web globals. | Verify a public Angular entrypoint direction in a temporary copy while keeping actual embedded SSR and runtime acceptance pending. |
| 2026-10-10 | Record the author's implemented neutral build entrypoint. | Verify the actual repository build, direct Web handler results in Node.js, and module evaluation in bare deno_core; keep embedded rendering acceptance pending. |
| 2026-10-10 | Record implemented URL and UTF-8 runtime support. | Verify the Deno Web extension setup and selected API behavior in the repository, with runtime tests inside Tokio; keep Angular rendering and broader environment acceptance pending. |
