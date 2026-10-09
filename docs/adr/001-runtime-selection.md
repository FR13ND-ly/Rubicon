# ADR 001: JavaScript Runtime Selection

**Date:** 2026-10-09
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
2. Execute a small module and verify its asynchronous behavior in the embedded runtime.
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
- [AngularAppEngine API](https://angular.dev/api/ssr/AngularAppEngine), rolling documentation checked on 2026-10-09; selected Angular version TBD.
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

This prototype creates and drops a runtime for every function call. It establishes synchronous engine embedding and numeric result conversion only. Persistent runtime reuse, ES modules, asynchronous work, Angular SSR, hydration, request isolation, and performance remain unverified. This evidence does not change the ADR's Draft status.

## Decision History

| Date | Change | Reason |
| --- | --- | --- |
| 2026-10-09 | Draft created and expanded. | Record deno_core as a prototype candidate, alternatives, and validation conditions. |
| 2026-10-09 | Record first synchronous execution and selected deno_core version. | Preserve the verified result, exact checks, unresolved linker warning, and remaining compatibility work. |
