# Documentation

## Specifications

- [Project overview](specs/project-overview.md): purpose, current scope and next work.
- [Architecture](specs/architecture.md): library, CLI, Angular SDK and request flow.
- [Rendering contract](specs/rendering-contract.md): request and response boundaries.
- [Benchmark protocol](specs/benchmark-protocol.md): requirements for the suite to be rebuilt after the CLI.

## Decisions

- [001 — Embedded runtime](adr/001-runtime-selection.md)
- [002 — Library, CLI and demo separation](adr/002-library-cli-demo.md)
- [003 — Bounded renderer pool](adr/003-renderer-pool.md)
- [004 — Reuse backend routes during SSR](adr/004-native-backend-routing.md)
- [005 — Additive Angular integration](adr/005-additive-angular-integration.md)
- [006 — Child process configuration and shutdown](adr/006-process-configuration.md)

## Working notes

- [Writing documentation](guides/how-to-document.md)
- [Git workflow](guides/git-workflow.md)
- [Performance evaluation](guides/performance-evaluation.md)
- [Templates](templates/readme.md)

The specifications describe the current prototype. ADRs explain choices; source code and tests carry implementation details. There is no maintained benchmark runner in this revision.
