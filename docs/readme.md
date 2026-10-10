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

A separate Angular 22.2.2 application now builds and serves server-rendered welcome content through its conventional Node.js entrypoint. Its setup and reproduction commands are documented alongside the application. Browser hydration, Angular execution inside Rubicon, request isolation, native services, and performance evaluation remain pending.

1. [Project overview](specs/project-overview.md)
2. [Architecture](specs/architecture.md)
3. [Rendering contract](specs/rendering-contract.md)
4. [Benchmark protocol](specs/benchmark-protocol.md)
5. [Runtime selection ADR](adr/001-runtime-selection.md)
6. [Angular reference application](../examples/angular-demo/README.md)

Update this index when document status changes or new documents are added.
