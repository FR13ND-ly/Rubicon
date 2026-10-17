# ADR 002: Separate library, CLI and demo

**Recorded:** 2026-10-17

**Status:** Accepted; records an existing project decision

## Context

The first executable mixed hosting code with application paths and routes. That made another application difficult to connect.

## Decision

Keep hosting in `crates/rubicon`, process tooling in `crates/rubicon-cli`, Angular integration in `packages/angular`, and application code in the demo. Each consuming application supplies a small Rust executable using the library.

## Alternative

One universal executable would be easier to distribute initially. It would also need a separate mechanism for loading application-specific Rust routes and services. Keeping everything in the demo would avoid the extraction, but would not provide a reusable product.

## Consequences

Application routes stay ordinary Rust code compiled with the application's dependencies. The application must build its own host executable. The CLI builds Angular artifacts and starts that host; it does not replace the application's backend.

Implementation: [workspace](../../Cargo.toml) and [server entry](../../crates/rubicon/src/server.rs).
