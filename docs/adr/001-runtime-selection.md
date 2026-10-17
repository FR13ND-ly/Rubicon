# ADR 001: Embedded JavaScript runtime

**Created:** 2026-10-09

**Updated:** 2026-10-17

**Status:** Accepted for the prototype

## Context

Angular SSR executes JavaScript. A Rust HTTP server therefore needs either an embedded runtime or a separate rendering process. Native application services should remain accessible from Rust.

## Decision

Use `deno_core` with the Deno extensions required by the Angular build. Each rendering worker owns its V8 runtime. Angular continues to perform the rendering through its public server APIs.

## Alternatives

A persistent Node process would use a familiar Angular SSR environment, but communication with Rust would cross a process boundary. A broader Deno runtime would provide more of the environment up front, with more configuration and dependencies. Other engines remain possible candidates for a separate compatibility experiment.

`deno_core` was selected for its Rust embedding interface and native operations. This is not a claim that its engine renders Angular faster than Node.

## Consequences

Rubicon supplies Web APIs, module loading and event-loop integration. Runtime and Angular upgrades require compatibility checks. Dependencies that assume a full Node environment may need changes.

Replacing the engine later means adapting initialization, module loading, operations and worker lifecycle. The application-facing [rendering contract](../specs/rendering-contract.md) can stay stable, but replacement is not a configuration switch.

Implementation: [runtime](../../crates/rubicon/src/lib.rs) and [Web globals](../../crates/rubicon/js/web.js).
