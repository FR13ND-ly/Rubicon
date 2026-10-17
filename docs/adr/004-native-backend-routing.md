# ADR 004: Reuse backend routes during SSR

**Recorded:** 2026-10-17

**Status:** Accepted; records an existing project decision

## Context

Calling a Rust service by a special operation name made the Angular application depend on Rubicon-specific service mappings. Calling the host over HTTP would reuse routes, but would add a local network request for data already available in the process.

## Decision

Reuse the application's Axum router. Angular code keeps ordinary `HttpClient` calls such as `/api/products`. During SSR, an `HttpBackend` implementation sends matching requests through a native operation to that router in memory. Browser requests use HTTP.

## Alternatives

A separate function registry would require maintaining route and operation mappings. A local HTTP round trip is more conventional and remains a useful comparison variant. Replacing `HttpClient` with an application-specific SDK would make adoption harder.

## Consequences

Existing handlers and middleware run for both paths. Angular interceptors remain in place above the transport. Transfer to hydration uses Angular's HTTP cache rather than manual product-specific state.

The bridge must preserve methods, headers, body bytes and error responses. Router-level authentication and authorization remain necessary. Native and HTTP transports must be identified separately in performance comparisons.

Implementation: [Rust backend](../../crates/rubicon/src/backend.rs) and [Angular provider](../../packages/angular/src/provide-backend.ts).
