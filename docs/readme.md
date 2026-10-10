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

The embedded runtime now exposes URL, URLSearchParams, TextEncoder, TextDecoder, and four timer functions through pinned Deno Web extensions. Selected URL and UTF-8 behavior is checked by runtime tests executing within current-thread Tokio contexts. A third worker fixture verifies awaited timer callbacks and explicit timeout and interval cancellation while retaining response and error recovery checks. Other Web API families and Angular request execution remain pending.

A separate Angular 22.2.2 application now serves Home and About pages through its conventional Node.js entrypoint. Direct SSR access, browser hydration, a counter interaction, and navigation have been verified. Its setup, reproduction commands, and browser screenshot are documented alongside the application. Angular execution inside Rubicon, request isolation, native services, and performance evaluation remain pending.

1. [Project overview](specs/project-overview.md)
2. [Architecture](specs/architecture.md)
3. [Rendering contract](specs/rendering-contract.md)
4. [Benchmark protocol](specs/benchmark-protocol.md)
5. [Runtime selection ADR](adr/001-runtime-selection.md)
6. [Angular reference application](../examples/angular-demo/README.md)

Update this index when document status changes or new documents are added.

## Experiments

- [001: Angular neutral bundle compatibility](experiments/001-angular-runtime-compatibility.md), Completed exploratory check with a verified repository follow-up: the neutral build's module loaded in bare deno_core, while actual embedded rendering and required Web API support remain pending. This is correctness evidence, not a performance benchmark.
