# Rubicon Documentation

This documentation supports the design, implementation, and evaluation of Rubicon. Proposed behavior and performance hypotheses must remain distinguishable from implemented behavior and measured results.

## Guides

- [How to write documentation](guides/how-to-document.md): document types, naming, status, and writing order.
- [Performance evaluation](guides/performance-evaluation.md): benchmark design, execution, and interpretation.
- [Git workflow](guides/git-workflow.md): focused changes, review, and reproducible experiment revisions.
- [Renderer diagnostics](guides/renderer-diagnostics.md): implemented timing boundaries and bounded log collection, test evidence, and remaining measurement checks after pilot 006.

## Templates

See the [template catalog](templates/readme.md) for specification and architecture decision templates.

## Project Documents

The following documents contain drafts for author review and evidence of completed prototype steps. The Rust host transports owned request and response objects through a persistent JavaScript worker, validates response metadata, and maps rendering failures to HTTP errors. Checks cover request identifiers, repeated headers, controlled queue rejection, structured response conversion, Promise fulfillment and rejection, and continued execution after ordinary failures. The architecture records the commands, outcomes, and limits of each milestone.

The embedded runtime now exposes URL/encoding APIs, four timer functions, exception and event interfaces, abort signals, three stream constructors, Headers/Request/Response, and initialized performance through pinned Deno extensions. Selected URL and UTF-8 behavior is checked by runtime tests executing within current-thread Tokio contexts. Six worker fixtures retain response and error recovery checks while covering microtasks, timer cancellation, Unicode stream content and closure, signal-driven cancellation, Web request/response semantics, and performance marks/measures with explicit cleanup. HTTP responses remain buffered; public fetch and host request cancellation remain pending.

A separate Angular 22.2.2 application serves Home and About pages through its conventional Node.js entrypoint. Direct SSR access, browser hydration, a counter interaction, and navigation have been verified on that reference. Its neutral TypeScript adapter and tower-http 0.7.1 file service are now implemented and verified through the Rust HTTP server. Both development and production Angular builds passed Home/About SSR, query routing, HEAD, asset-byte checks, counter interaction, navigation, and direct About reload. Development logs confirmed hydration with no skipped components. Request isolation, native services, lifecycle controls, and the full performance evaluation remain pending; experiment 006 records the completed exploratory pilot.

1. [Project overview](specs/project-overview.md)
2. [Architecture](specs/architecture.md)
3. [Rendering contract](specs/rendering-contract.md)
4. [Benchmark protocol](specs/benchmark-protocol.md)
5. [Runtime selection ADR](adr/001-runtime-selection.md)
6. [Angular reference application](../examples/angular-demo/README.md)

Update this index when document status changes or new documents are added.

The [exploratory performance pilot](../benchmarks/pilot/README.md) provides a checked local load runner, request accounting, process-resource sampling, compact replayable data, and plotting. Its 20 runs are complete; the full benchmark protocol remains a draft with additional correctness and experimental controls required.

## Experiments

- [007: Renderer timing and instrumentation cost](experiments/007-renderer-timing-diagnostic.md), Completed 12-case release sequence with disabled/enabled timing variants. All 5/s and 20/s runs passed exploratory limits; 50/s overloaded with HTTP 503 and timeouts. Verified low-load exports have approximately 28 ms median wall time. Two overload internal populations remain unsupported for exact coverage; all failures and the interrupted first attempt are retained. Logging overhead is inconclusive.

- [006: Exploratory SSR performance pilot](experiments/006-ssr-performance-pilot.md), Completed local W-01 load test of release Rust and production Angular Node. All 20 runs had valid accounting and recovery. Node delivered more correct work at lower latency on this page; Rubicon saturated around 31–32 correct responses/s with HTTP 503, despite low mean CPU. Short windows, differing V8 versions, and client concurrency/environmental limits prevent a general or sustained-capacity claim. Compact data, per-run percentiles, resources, hashes, and plots are retained.

- [005: Browser assets and hydration through Rust](experiments/005-browser-assets-hydration.md), Completed temporary check and repository verification. The author selected tower-http 0.7.1; actual development and production Angular builds passed SSR, matching JavaScript/CSS/favicon delivery, HEAD, missing paths, browser interaction, navigation, reload, and warning/error checks. The Rust executable was a debug build; performance evaluation remains pending.

- [004: Angular adapter and Rust HTTP delivery](experiments/004-angular-http-adapter.md), Completed temporary integration check with verified repository implementation. The TypeScript entry exports render, and Axum delivers Home/About SSR, an unknown-route 404, query routing, and an empty HEAD response with matching Content-Length. Browser assets returned 404 at this milestone; experiment 005 records their subsequent implementation and browser checks. Performance remains pending.

- [003: Performance API and first embedded Angular render](experiments/003-performance-angular-render.md), Completed temporary correctness check with verified repository performance API implementation. A real Deno performance object enabled Home/About SSR through the temporary Rust worker, a controlled 404, and a HEAD representation. Six fixtures passed in both the copy and repository. Experiment 004 records the subsequent repository adapter and HTTP delivery; browser hydration through Rubicon and performance measurement remain pending.

- [002: Web Request and Response objects](experiments/002-web-fetch-objects.md), Completed temporary correctness check with verified repository implementation. Pinned Deno extensions and a fifth worker fixture passed selected Request/Response semantics, cookie separation, and split UTF-8 body decoding. No network or Angular handler was invoked in this experiment.

- [001: Angular neutral bundle compatibility](experiments/001-angular-runtime-compatibility.md), Completed exploratory check with a verified repository follow-up: the neutral build's module loaded in bare deno_core, while actual embedded rendering and required Web API support remain pending. This is correctness evidence, not a performance benchmark.
