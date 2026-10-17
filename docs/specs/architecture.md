# Architecture

**Created:** 2026-10-09

**Updated:** 2026-10-17

**Status:** Current prototype

## Components

| Component | Responsibility |
| --- | --- |
| `crates/rubicon` | HTTP host, runtime integration, renderer pool and native backend dispatch |
| `crates/rubicon-cli` | Integration, Angular builds and server process control |
| `packages/angular` | Angular bootstrap, rendering adapter and HTTP backend transport |
| `examples/angular-demo` | Application and Rust host consuming the public interfaces |

The library is independent of demo routes. Angular is the current frontend integration; support for other frameworks has not been implemented.

## Build and startup

`rb integrate --backend-path` examines an existing Angular SSR workspace and a Rust backend. It generates `server.rubicon.ts` and `rubicon.toml`. The backend must already be a Rubicon host.

`rb build` invokes Angular Architect through the installed SDK. Generated bootstrap entries and a tsconfig live in Angular's cache. The application configuration is not rewritten. The build produces the server module and matching browser assets.

The Rust executable is compiled separately. `rb serve` reads the TOML configuration and starts it with configuration in child-process environment variables. Workers load the server module before the HTTP listener starts accepting requests.

## HTTP request

1. Axum checks application routes, including the API.
2. Otherwise, the browser-file service checks the indexed asset directory.
3. An unmatched GET or HEAD is submitted to the renderer queue.
4. A worker passes its URL, method, headers and identifier to the Angular adapter.
5. `AngularAppEngine` applies the application's server route policy.
6. Rust validates the returned response metadata and sends the body.

The configured origin supplies the URL origin. Client `Host` and forwarded headers do not replace it. Angular's allowed-host configuration is extended with the configured build origin, without adding a wildcard.

## Runtime ownership

Each worker thread owns a `JsRuntime` and a current-thread Tokio executor. Messages cross the thread boundary; runtime handles stay on their owning thread. Modules are reused between requests.

Workers share one bounded `async-channel` queue. Defaults are one worker, up to 16 active renders per worker and 16 queued requests. Active renders can overlap while awaiting asynchronous work. JavaScript within one isolate still executes on one thread.

A full queue rejects new work. Shutdown closes admission, drains accepted work and joins workers. General execution deadlines and interruption of arbitrary JavaScript are not implemented.

## Backend data

The application supplies an Axum `Router`. HTTP API calls use that router normally. During SSR, Angular's `HttpBackend` implementation sends matching same-origin `/api` requests through a native operation to the same router in memory.

Components keep ordinary `HttpClient` calls. Angular interceptors remain above the transport, and Angular's HTTP transfer cache carries eligible SSR data to hydration. Browser calls use HTTP.

## Responses

Buffered bodies contain owned text or bytes. With streaming enabled, chunks cross a bounded channel and keep the rendering slot occupied until completion or cancellation. The demo uses buffering by default.

See the [rendering contract](rendering-contract.md) for field and failure semantics, and [ADRs](../readme.md#decisions) for the reasons behind these choices.
