# Rubicon Documentation

This documentation supports the design, implementation, and evaluation of Rubicon. Proposed behavior and performance hypotheses must remain distinguishable from implemented behavior and measured results.

## Guides

- [How to write documentation](guides/how-to-document.md): document types, naming, status, and writing order.
- [Performance evaluation](guides/performance-evaluation.md): benchmark design, execution, and interpretation.
- [Git workflow](guides/git-workflow.md): focused changes, review, and reproducible experiment revisions.

## Templates

See the [template catalog](templates/readme.md) for specification and architecture decision templates.

## Project Documents

The following documents contain initial drafts for author review. Specifications describe proposed behavior; the architecture records the verified HTTP-to-JavaScript worker prototype, its queue rejection check, and continued execution after ordinary renderer errors. The rendering contract records the request object containing URL and method, its public owned `RenderRequest`, and the HTTP and worker checks. HEAD body omission is verified, while its demonstration Content-Length still needs alignment with GET metadata. The runtime ADR records verified capabilities including local ES module loading and repeated calls returning HTML. The complete request/response contract, request isolation, Angular SSR integration, and performance evaluation remain future work:

1. [Project overview](specs/project-overview.md)
2. [Architecture](specs/architecture.md)
3. [Rendering contract](specs/rendering-contract.md)
4. [Benchmark protocol](specs/benchmark-protocol.md)
5. [Runtime selection ADR](adr/001-runtime-selection.md)

Update this index when document status changes or new documents are added.
