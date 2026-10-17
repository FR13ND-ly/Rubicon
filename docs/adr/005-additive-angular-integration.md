# ADR 005: Additive Angular integration

**Recorded:** 2026-10-17

**Status:** Accepted; records an existing project decision

## Context

Adopting Rubicon should not require moving an Angular application into a prescribed frontend/backend structure or editing several existing configuration files.

## Decision

Use `rb integrate --backend-path` to connect an existing Rust host. Add one Angular entry, `server.rubicon.ts`, and one project configuration, `rubicon.toml`. Reuse the application's existing component, server configuration and server routes.

Angular build changes are supplied through generated files in Angular's cache. Existing source files and `angular.json` remain untouched.

## Alternatives

A required frontend/backend directory layout would simplify discovery but force project reorganization. Adding providers to `app.config.server.ts` would work but introduce another integration edit. A Rust controller next to every component would mix backend organization with frontend structure.

## Consequences

Integration must inspect the existing workspace rather than assume component names. It currently supports the standard Angular 22.2.2 standalone SSR bootstrap. Custom bootstrap logic needs manual configuration.

Rerunning integration leaves matching files unchanged. Conflicting files are preserved and reported; the command does not overwrite them. It connects an existing Rubicon backend rather than converting an arbitrary Rust server.

Implementation: [integration command](../../crates/rubicon-cli/src/integrate.rs) and [SDK helper](../../packages/angular/integrate.mjs).
