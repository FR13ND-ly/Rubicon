# Rubicon Architecture

**Status:** Draft
**Created:** 2026-10-09
**Last updated:** 2026-10-10
**Implementation:** Initial runtime and HTTP prototypes; Angular SSR not implemented

## Purpose and Context

This specification describes the proposed components, build artifacts, and request lifecycle of Rubicon. The architecture must support a reusable hosting product and a separate consuming Angular application.

The initial runtime candidate is deno_core, subject to compatibility validation. Angular rendering remains JavaScript execution within the host process. Rust manages HTTP, scheduling, and native services.

## Scope

This draft covers startup, artifact loading, request dispatch, rendering workers, native service access, cleanup, and shutdown. It also identifies the responsibilities of the TypeScript integration and server executable.

The Angular compiler and existing build tools remain responsible for compiling the application. Rubicon's initial tooling may coordinate those tools and package their outputs. Custom Angular builders, installation schematics, HTML caching, and snapshots are later extensions.

## Requirements

| ID | Requirement | Acceptance evidence |
| --- | --- | --- |
| ARCH-01 | The Rust library must be reusable without application-specific components or routes. | The example consumes documented public entrypoints. |
| ARCH-02 | The host must validate configuration and initialize its declared rendering capacity before advertising full readiness. | Startup checks and readiness observations. |
| ARCH-03 | The runtime adapter must own engine-specific loading, operation registration, and event-loop behavior. | Boundary review and renderer integration. |
| ARCH-04 | The scheduler must enforce configured worker and queue limits. | Capacity, rejection, and queue instrumentation checks. |
| ARCH-05 | Runtime reuse must preserve request isolation and per-request cleanup. | Sequential and concurrent identity checks. |
| ARCH-06 | Native services must be accessed through scoped operations with explicit results and failures. | Async service integration and failure checks. |
| ARCH-07 | The host must preserve renderer response semantics through HTTP delivery. | Contract verification. |
| ARCH-08 | Shutdown and failed-render recovery must have bounded, observable behavior. | Cancellation, failure, and shutdown checks. |

## Design and Request Flow

### Proposed Components

| Component | Responsibility |
| --- | --- |
| Rust library | Configuration, HTTP integration, scheduling, response delivery, and service interfaces. |
| Server executable | Read configuration, construct the host, start serving, and initiate shutdown. |
| Runtime adapter | Initialize JavaScript workers, resolve modules, register native operations, and drive asynchronous execution. |
| Angular/TypeScript adapter | Map Rubicon requests to Angular rendering and map Angular outcomes back to the contract. |
| Application | Define components, routes, providers, and business rules. |
| Example application | Exercise the public integration and provide controlled correctness fixtures. |
| Benchmark harness | Generate traffic and preserve correctness, latency, and resource evidence. |

The first Rust package may contain both a library target and an executable target. Additional crates should be introduced when an actual boundary or independent dependency justifies them.

### Proposed Build Flow

The Angular toolchain compiles the browser application and server code. The integration must identify the server entrypoint, include the renderer adapter, and supply required assets or manifests. The exact artifact layout is TBD and must be discovered against a pinned Angular version.

The initial pipeline must not require the Rust host to transpile TypeScript or compile Angular templates at request time. Node may remain a build dependency even when the production host uses an embedded runtime.

### Proposed Request Flow

The following diagram describes a proposal, not an implemented system.

```mermaid
flowchart TD
    Browser["Browser"] --> Host["Rust HTTP host"]
    Host --> Assets["Static browser assets"]
    Assets --> Browser
    Host --> Queue["Bounded SSR queue"]
    Queue --> Worker["Persistent runtime worker"]
    Worker --> Adapter["Angular renderer adapter"]
    Adapter --> Angular["Application SSR"]
    Angular --> Adapter
    Adapter --> Worker
    Worker --> Host
    Host --> Browser
    Adapter <--> Services["Scoped native Rust services"]
```

1. The host validates the request and constructs its effective URL using an explicit proxy policy.
2. Host-owned endpoints and browser assets are dispatched by the Rust router.
3. An application-rendering request receives an identifier, deadline, and isolated context.
4. The scheduler either admits it, queues it within capacity, or rejects it.
5. A worker invokes the renderer and drives the asynchronous work required to complete it.
6. The adapter invokes Angular for the application route and normalizes the outcome.
7. The host validates response metadata, applies transport rules, and sends the response.
8. Request resources are released before the worker becomes available again.

Angular remains responsible for matching application routes during rendering. The host's endpoint routing must have a documented precedence so API and asset paths are not accidentally treated as application pages.

## Contracts and Data

| Boundary | Input | Output | Failure behavior |
| --- | --- | --- | --- |
| Configuration to host | Artifact paths, worker settings, limits, and service configuration | Initialized host | Invalid configuration prevents readiness. |
| Scheduler to worker | Request context and cancellation/deadline state | Contract outcome | Expired or cancelled work is removed or stopped. |
| Worker to Angular adapter | Serializable request information and scoped service access | Response, no matching route, or failure | The worker reports the failure and cleans up. |
| Adapter to native service | Request-scoped operation and arguments | Data or explicit error | Late results cannot affect a different request. |

The [rendering contract](rendering-contract.md) owns field definitions, response outcomes, and failure semantics. Configuration format, module export names, and service-operation names remain TBD; they must be documented once the prototype establishes a working integration.

The initial response is buffered HTML. The matching browser artifacts must come from the same application build. Any HTML processing must preserve the structure and state required for hydration.

## Lifecycle, Concurrency, and Isolation

### Process and Worker State

Process-wide state includes immutable configuration, service pools, and scheduling infrastructure. Worker-wide state includes the JavaScript runtime, loaded modules, and reusable renderer metadata. Request-local state includes input, Angular application resources, document state, service-operation tracking, and pending response data.

deno_core's JsRuntime is not Send or Sync. The initial implementation should create each runtime on its owning thread and keep runtime operations there, communicating with the scheduler through messages. V8 platform initialization must follow the selected version's requirements. These are integration constraints, not evidence of higher throughput.

### Initialization and Reuse

Each worker initializes its environment, loads the application entrypoint, and completes a readiness check. Reuse retains engine and module initialization while request cleanup disposes of request-specific resources.

A persistent Angular engine or cached bootstrap function must not retain a previous request's user data. Application-level global state and providers require an explicit ownership review.

### Scheduling and Shutdown

The first prototype permits one active render per worker. This conservative policy must be evaluated on both CPU-heavy and asynchronous workloads. Pool size, queue capacity, and render deadline are configurable, with initial values TBD.

Shutdown stops admission, allows active requests to finish within a grace period, cancels remaining work, and releases worker and service resources. The implementation must identify which steps can be interrupted and how an unusable runtime is replaced.

## Failure and Resource Limits

Queue-full, missing-route, application-failure, and runtime-failure outcomes must be recorded separately. A failed render must not silently return incomplete HTML as a successful response.

Dropping an asynchronous wait is not sufficient evidence that JavaScript execution or native work has stopped. The runtime adapter must implement and verify actual cancellation or interruption behavior. A worker with uncertain cleanup must not be reused.

The required configurable limits include queue capacity, active worker count, render deadline, and maximum buffered response size. Request-input and native-operation limits must also be defined before load testing. Startup failures and worker replacement must remain observable.

## Performance Expectations

**Not measured.**

The architecture permits experiments on pool size, scheduling, native data access, and startup initialization. Each additional worker consumes memory, and each Rust-JavaScript boundary may incur copying, conversion, or serialization costs.

The minimal candidate should already reuse initialized workers. A conventional Node baseline also uses a persistent server; an artificial per-request engine startup is not an adequate reference for steady-state SSR.

Snapshots are a later startup experiment. HTML caching is a separate serving scenario because it bypasses dynamic rendering.

## Verification

| Requirement | Verification method | Planned evidence | State |
| --- | --- | --- | --- |
| ARCH-01, ARCH-03 | Inspect component boundaries and run the consuming example. | Library and adapter integration review. | Planned |
| ARCH-02 | Exercise missing artifacts, invalid configuration, and startup readiness. | Startup checks. | Planned |
| ARCH-04 | Offer work above capacity and inspect admission and queue counts. | Scheduler checks and overload run. | Planned |
| ARCH-05 | Alternate identities and verify cleanup after successful and failed renders. | Isolation checks and sustained-operation run. | Planned |
| ARCH-06 | Exercise service success, failure, and late completion. | Native-service integration checks. | Planned |
| ARCH-07 | Verify HTTP delivery against the rendering contract. | Contract checks. | Planned |
| ARCH-08 | Disconnect clients, expire deadlines, and initiate shutdown. | Cancellation and recovery checks. | Planned |

### Initial HTTP Host Verification

On 2026-10-10, the author added Axum 0.8.9 and enabled Tokio's multi-thread runtime and network features. The executable binds to `127.0.0.1:3000`, routes `GET /` to a handler returning `Html`, and serves requests through `axum::serve`. The response body is the fixed Rust string `<h1>Rubicon</h1>`; the HTTP handler does not yet invoke the JavaScript renderer.

The following checks completed successfully:

```text
cargo fmt --manifest-path crates/rubicon/Cargo.toml -- --check
cargo check --locked --offline --manifest-path crates/rubicon/Cargo.toml
cargo test --locked --offline --manifest-path crates/rubicon/Cargo.toml --lib
cargo build --locked --offline --manifest-path crates/rubicon/Cargo.toml
curl.exe --max-time 5 -sS -i http://127.0.0.1:3000/
```

The HTTP check used the author's already-running server and observed `200 OK`, `Content-Type: text/html; charset=utf-8`, and the exact body `<h1>Rubicon</h1>`. The existing runtime regression test passed, with one test passed and zero failed. Linker warning LNK4098 persists in the library test executable; no linker warning was emitted by the application build during this validation.

These checks verify the initial HTTP scaffold, not acceptance of the complete architecture requirements. ARCH-07 still requires connection to the renderer and verification of its response semantics. Worker scheduling, readiness, request isolation, graceful shutdown, Angular SSR, and performance remain unverified.

## Open Questions

| Question | Impact | Next action |
| --- | --- | --- |
| Which generated Angular entrypoint and manifests are required? | Defines the artifact-loading integration. | Inspect a pinned example build. |
| Which Web or Node APIs are required by the application bundle? | Determines runtime extensions. | Run a compatibility spike and inventory dependencies. |
| How will blocking JavaScript be interrupted safely? | Determines deadline enforcement and recovery. | Prototype the selected engine's interruption mechanism. |
| Which native service calls are exposed to the adapter? | Defines scope and data ownership. | Specify one deterministic asynchronous operation. |
| What pool and queue settings meet the resource budget? | Controls latency and memory. | Pilot under the benchmark protocol. |

## Related Decisions and Sources

- [Project overview](project-overview.md).
- [Rendering contract](rendering-contract.md).
- [Benchmark protocol](benchmark-protocol.md).
- [ADR 001: JavaScript runtime selection](../adr/001-runtime-selection.md).
- [AngularAppEngine API](https://angular.dev/api/ssr/AngularAppEngine), rolling documentation checked on 2026-10-09; selected Angular version TBD.
- [deno_core JsRuntime](https://docs.rs/deno_core/latest/deno_core/struct.JsRuntime.html), documentation observed at 0.412.0 on 2026-10-09; prototype dependency pinned to 0.412.0.
- [Axum serve API](https://docs.rs/axum/latest/axum/fn.serve.html), rolling documentation observed at 0.8.9 on 2026-10-10; prototype dependency pinned to 0.8.9.
- [Angular library creation](https://angular.dev/tools/libraries/creating-libraries), rolling documentation checked on 2026-10-09.
- [Cargo workspaces](https://doc.rust-lang.org/cargo/reference/workspaces.html), rolling documentation checked on 2026-10-09.
