# Rubicon Project Overview

**Status:** Draft
**Created:** 2026-10-09
**Last updated:** 2026-10-10
**Implementation:** HTTP-to-JavaScript worker prototype and Node.js Angular SSR/hydration reference; Angular integration into Rubicon pending

## Purpose and Context

Rubicon is a bachelor's thesis project investigating a reusable Rust host for Angular server-side rendering (SSR). The proposed framework uses Rust for HTTP handling, scheduling, and application services, with an embedded JavaScript runtime executing Angular rendering.

Angular already provides SSR, prerendering, and client hydration. Rubicon investigates a different hosting architecture for these capabilities. The intended contribution is the integration of a Rust host with Angular, the management of rendering resources, and an evaluation of the resulting tradeoffs.

The primary research question is: **Under which workloads and resource constraints does a Rust-hosted Angular SSR architecture improve latency, sustainable throughput, or resource consumption compared with Angular SSR on Node.js?**

The intended user is a developer building an Angular application who wants to run its server-side rendering and backend services under a Rust host. A small example application will demonstrate consumption of Rubicon and provide a controlled benchmark fixture.

The Rust prototype verifies JavaScript execution, runtime reuse, local ES module loading, and Axum routes invoking a persistent worker. The host forwards an owned request containing a generated string identifier, absolute URL, method, and repeated header pairs. The worker returns an owned response containing status, repeated headers, and a buffered body; the HTTP handler validates that response before delivery. The queue holds 16 waiting jobs with one active render. Automated checks cover admission rejection, identifiers without wrapping or JavaScript numeric precision loss, response metadata, Promise fulfillment and rejection, and continued execution after ordinary execution and conversion failures. Earlier HTTP checks also verify escaped request values and empty HEAD delivery. These checks do not establish request isolation or resource cleanup.

The separate [reference application](../../examples/angular-demo/README.md) pins Angular 22.2.2 and runs on the generated Node.js server. The initial welcome application's production build and asset delivery passed. The routed application now has Home and About pages and a signal-based counter; its development build, two root-component tests, and direct SSR access passed. Browser checks verify hydration diagnostics, counter increments, navigation, and direct About reload without captured warnings or errors. Neither checked build prerenders routes. The application does not yet consume Rubicon. Angular compatibility with the embedded runtime, native services, isolation, and performance results remain pending. The [architecture](architecture.md), [rendering contract](rendering-contract.md), and [runtime ADR](../adr/001-runtime-selection.md) record the completed checks and their limits.

## Scope

The intended initial scope includes:

- A reusable Rust library and a server executable.
- A TypeScript adapter connecting an Angular application to the rendering contract.
- Loading and executing a compiled Angular server bundle.
- Serving the corresponding browser assets and returning rendered responses.
- Persistent rendering workers with request isolation and bounded admission.
- One example of asynchronous data access through a native Rust service.
- Correctness checks and a reproducible comparison with Angular SSR on Node.js.

Later work may include HTML caching, incremental static regeneration, startup snapshots, installation schematics, and development tooling. These features require separate justification and evaluation.

The initial scope excludes frontend WASM state management, rewriting private Ivy instructions, a replacement Angular compiler, native DOM serialization, and a complete replacement build pipeline. Progressive HTML streaming and binary state transfer also require separate specifications.

The main assumptions are that a selected Angular build can execute in the embedded environment, that its HTML remains hydratable, and that the integration can be implemented within the thesis schedule. Each assumption must be checked by a prototype.

## Requirements

In this draft, `must` states an intended acceptance requirement, `should` a preference, and `may` an optional capability. These terms do not imply that the behavior is implemented.

| ID | Requirement | Acceptance evidence |
| --- | --- | --- |
| OV-01 | Rubicon must expose reusable hosting functionality independently of the example application's components and routes. | An example consumes the library through its public interface. |
| OV-02 | The Rust server must return Angular-generated HTML for a route explicitly configured for SSR. | HTTP content checks for direct route access. |
| OV-03 | The browser must hydrate the returned page and support a representative interaction. | Browser check without hydration errors. |
| OV-04 | Rendering must preserve response status, repeated headers, redirects, and request-specific content. | Rendering-contract verification cases. |
| OV-05 | Repeated and concurrent requests must remain isolated and release request resources after completion. | Alternating identities, concurrency checks, and sustained operation. |
| OV-06 | The prototype must demonstrate asynchronous access to a Rust service during SSR. | An awaited service result appears in the rendered page. |
| OV-07 | Performance comparisons must use equivalent workloads and an explicit resource budget. | A frozen benchmark manifest and raw results. |
| OV-08 | Documentation must distinguish proposals, implemented behavior, and measured results. | Documentation review linked to the tested revision. |

## Design and Request Flow

A developer builds the example application with the Angular toolchain. The proposed integration produces browser assets and a server entrypoint compatible with Rubicon's renderer adapter.

At startup, the Rust executable reads configuration, initializes rendering workers, and loads the server entrypoint. For an SSR request, the host creates a request context, obtains a rendering slot, and delegates to the Angular adapter. The host then returns the resulting response and completes request cleanup.

The library owns general hosting behavior. The application owns its components, routes, and business rules. The executable supplies an entrypoint for configuration and startup. See the [architecture](architecture.md) for component responsibilities.

## Contracts and Data

| Boundary | Input | Output | Failure behavior |
| --- | --- | --- | --- |
| Application build to host | Server entrypoint and browser artifacts | Loadable application | Startup fails if required artifacts are invalid or unavailable. |
| HTTP host to renderer | Validated request information and request context | Rendered response, no matching route, or failure | The host applies the documented failure policy. |
| Renderer to native service | A scoped data request | Serializable data or service failure | The application or adapter handles the failure explicitly. |
| Server to browser | HTML and matching browser assets | A hydratable Angular page | A mismatch fails correctness verification. |

The [rendering contract](rendering-contract.md) is authoritative for request and response semantics. Initial cross-boundary application data should use explicitly documented JSON-compatible values. Native runtime handles and server-only resources remain host-owned.

## Lifecycle, Concurrency, and Isolation

Runtime initialization and loaded modules may be reused across requests. Request context, rendered documents, user data, and request-scoped service results must have a defined lifetime and cleanup path.

The initial scheduling proposal permits one active render per runtime worker. Worker count and queue capacity are configuration choices to evaluate. Reuse of a JavaScript runtime must not imply reuse of a previous user's live Angular application state.

## Failure and Resource Limits

The prototype must distinguish invalid requests, missing routes, application failures, unavailable workers, expired deadlines, and cancelled requests. These outcomes must remain visible in correctness and performance accounting.

Startup must validate application artifacts before the host advertises rendering readiness. Queue, deadline, and response-size limits are required design parameters; their initial values remain TBD pending the first integration and pilot runs.

## Performance Expectations

**Not measured.**

The hypotheses are that resource scheduling, selected native services, and reduced integration overhead may improve some workloads. Runtime pooling also introduces memory costs, and cross-boundary calls may introduce copying or serialization costs.

A faster host does not guarantee a faster Angular render. Frontend responsiveness and change detection are outside the initial server-performance claim.

For a fixed workload and resource budget, the primary optimization objective is the highest sustained rate of correct responses meeting declared latency, error, and memory limits. Secondary outcomes include CPU cost, startup behavior, and maintainability.

## Verification

| Requirement | Verification method | Planned evidence | State |
| --- | --- | --- | --- |
| OV-01 | Inspect public boundaries and run the consuming example. | Reference application at `examples/angular-demo`; Rubicon consumption pending. | Partial: separate Node.js reference created; consuming adapter planned. |
| OV-02, OV-04 | Check dynamic content, status, headers, redirects, and missing routes. | HTTP correctness checks. | Partial: structured prototype checks and Node.js Angular SSR for two pages verified; Angular delivery through Rust planned. |
| OV-03 | Open the SSR page and perform an interaction. | Browser hydration check. | Partial: Node.js reference hydration and counter interaction verified; repeat after Rubicon integration. |
| OV-05 | Alternate request identities, exercise concurrency, and observe cleanup. | Isolation and sustained-operation records. | Planned |
| OV-06 | Await a controlled native data operation. | Native-service integration check. | Planned |
| OV-07 | Execute the benchmark protocol. | Versioned experiment record and raw artifacts. | Planned |
| OV-08 | Review documentation against the implementation revision. | Documentation review. | Planned |

The thesis can establish feasibility and explain performance tradeoffs even if a proposed optimization produces no gain. An inconclusive or unfavorable result must be reported with its limitations.

## Open Questions

| Question | Impact | Next action |
| --- | --- | --- |
| How will the pinned Angular 22.2.2 server artifacts be loaded by Rubicon? | Defines integration compatibility. | Inspect the reference build's server entrypoints, manifests, and dependencies. |
| Which APIs and modules are required by the selected server bundle? | Determines embedding effort. | Inspect artifacts and run a compatibility spike. |
| Which deployment platform and resource budgets will be evaluated? | Limits the generality of performance conclusions. | Select and record the benchmark environment. |
| Which native service best demonstrates the proposed integration? | Defines the data-access workload. | Select a small deterministic example. |
| What thesis milestones and acceptance thresholds are practical? | Controls scope and completion criteria. | Review with the author and supervisor before implementation. |

## Related Decisions and Sources

- [Architecture](architecture.md).
- [Rendering contract](rendering-contract.md).
- [Benchmark protocol](benchmark-protocol.md).
- [ADR 001: JavaScript runtime selection](../adr/001-runtime-selection.md).
- [Angular reference application](../../examples/angular-demo/README.md), Angular 22.2.2 with committed dependency lock.
- [Angular SSR guide](https://angular.dev/guide/ssr), rolling documentation checked on 2026-10-10; reference application pinned to Angular 22.2.2.
