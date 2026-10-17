# Project overview

**Created:** 2026-10-09

**Updated:** 2026-10-17

**Status:** Prototype

## Purpose

Rubicon investigates whether a Rust host can improve the performance or resource use of Angular SSR compared with Node.js. Angular still renders the application. Rust owns HTTP delivery, rendering workers and application services.

The intended use is an Angular application connected to a Rust backend, with little integration code added to the frontend. Existing components, providers and route configuration stay in place.

## Current scope

The prototype contains:

- A reusable Rust library with an Axum server and persistent embedded V8 workers.
- An Angular SDK that connects Angular SSR and `HttpClient` to Rust.
- A CLI with `integrate`, `build` and `serve` commands.
- A demo with API data rendered on the server and transferred for hydration.
- Tests for rendering, HTTP semantics, worker lifecycle and integration.

Runtime pooling, bounded admission, binary response bodies and optional response streaming are implemented. These features need workload-specific evaluation; their presence alone does not establish a performance advantage.

## Current priority

Finish the installation and CLI workflow, then rebuild the performance suite. The next evaluation should cover several application sizes, concurrency levels, latency, CPU and RAM.

HTML caching, frontend WASM acceleration and startup snapshots are later work. The project does not replace Angular's compiler, router or change detection.

## Evaluation question

For the same application and comparable resources, where does Rubicon improve latency, throughput or resource use, and what does that improvement cost?

Previous exploratory measurements helped guide the prototype. They are not presented here as the final evaluation. The [benchmark protocol](benchmark-protocol.md) records what the new suite must measure.
