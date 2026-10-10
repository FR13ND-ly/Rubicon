# Rubicon Documentation

This documentation supports the design, implementation, and evaluation of Rubicon. Proposed behavior and performance hypotheses must remain distinguishable from implemented behavior and measured results.

## Guides

- [How to write documentation](guides/how-to-document.md): document types, naming, status, and writing order.
- [Performance evaluation](guides/performance-evaluation.md): benchmark design, execution, and interpretation.
- [Git workflow](guides/git-workflow.md): focused changes, review, and reproducible experiment revisions.

## Templates

See the [template catalog](templates/readme.md) for specification and architecture decision templates.

## Project Documents

The following documents contain drafts for author review and evidence of completed prototype steps. The Rust host transports owned request and response objects through a persistent JavaScript worker, validates response metadata, and maps rendering failures to HTTP errors. Checks cover request identifiers, repeated headers, controlled queue rejection, structured response conversion, Promise fulfillment and rejection, and continued execution after ordinary failures. The architecture records the commands, outcomes, and limits of each milestone.

The embedded runtime now exposes URL/encoding APIs, four timer functions, exception and event interfaces, abort signals, three stream constructors, Headers/Request/Response, and initialized performance through pinned Deno extensions. Selected URL and UTF-8 behavior is checked by runtime tests executing within current-thread Tokio contexts. Six worker fixtures retain response and error recovery checks while covering microtasks, timer cancellation, Unicode stream content and closure, signal-driven cancellation, Web request/response semantics, and performance marks/measures with explicit cleanup. HTTP responses remain buffered; public fetch, host request cancellation, and repository Angular HTTP integration remain pending.

A separate Angular 22.2.2 application now serves Home and About pages through its conventional Node.js entrypoint. Direct SSR access, browser hydration, a counter interaction, and navigation have been verified. Its setup, reproduction commands, and browser screenshot are documented alongside the application. Angular execution inside Rubicon, request isolation, native services, and performance evaluation remain pending.

1. [Project overview](specs/project-overview.md)
2. [Architecture](specs/architecture.md)
3. [Rendering contract](specs/rendering-contract.md)
4. [Benchmark protocol](specs/benchmark-protocol.md)
5. [Runtime selection ADR](adr/001-runtime-selection.md)
6. [Angular reference application](../examples/angular-demo/README.md)

Update this index when document status changes or new documents are added.

## Experiments

- [003: Performance API and first embedded Angular render](experiments/003-performance-angular-render.md), Completed temporary correctness check with verified repository performance API implementation. A real Deno performance object enabled Home/About SSR through the temporary Rust worker, a controlled 404, and a HEAD representation. Six fixtures passed in both the copy and repository. The Angular adapter, HTTP delivery, browser hydration through Rubicon, and performance measurement remain pending.

- [002: Web Request and Response objects](experiments/002-web-fetch-objects.md), Completed temporary correctness check with verified repository implementation. Pinned Deno extensions and a fifth worker fixture passed selected Request/Response semantics, cookie separation, and split UTF-8 body decoding. No network or Angular handler was invoked in this experiment.

- [001: Angular neutral bundle compatibility](experiments/001-angular-runtime-compatibility.md), Completed exploratory check with a verified repository follow-up: the neutral build's module loaded in bare deno_core, while actual embedded rendering and required Web API support remain pending. This is correctness evidence, not a performance benchmark.
