# Rendering contract

**Created:** 2026-10-09

**Updated:** 2026-10-17

**Status:** Implemented prototype boundary

## Request

The Rust [RenderRequest](../../crates/rubicon/src/request.rs) carries:

| Field | Meaning |
| --- | --- |
| `request_id` | Process-local identifier, represented as a string |
| `url` | Absolute URL built from the configured origin and original path/query |
| `method` | HTTP method, normally GET or HEAD for page rendering |
| `headers` | Name/value pairs preserving repeated values |

The host generates identifiers with checked increment. They restart with the process. Exhaustion returns HTTP 503 instead of wrapping; an identifier is not an authentication token.

Header values must fit the current text representation. Unsupported values produce HTTP 400 before rendering. Runtime arguments are serialized as values, not inserted into JavaScript source.

## Angular adapter

`server.rubicon.ts` exports a server created by `createRubiconServer`. It uses the application's root component and existing server configuration. The SDK installs its backend provider during bootstrap and calls `AngularAppEngine.handle`.

`app.routes.server.ts` remains the source of SSR, prerender and client-rendering policies. The adapter preserves Angular's response status and headers. No matching Angular response becomes a 404.

Request context and backend transport are passed per render. They are not automatically serialized to the browser. Application code must avoid putting per-user state in module globals reused by the runtime.

## Response

The result contains a status, header pairs and a body. [RenderResponse](../../crates/rubicon/src/response.rs) supports buffered text/bytes; `render_stream` also supports a streamed body.

Rust validates status and header syntax before HTTP delivery. Repeated headers, including separate cookies, are retained. Renderer-supplied transport headers such as `Content-Length` and `Transfer-Encoding` are rejected. Bodyless statuses must have empty bodies. HEAD sends no body.

Streaming uses bounded chunk delivery. A consumer cancellation reaches the producer and releases the worker slot. Once headers are sent, a later body failure ends the stream rather than replacing the response with a different HTTP status.

## Native API transport

During SSR, matching same-origin `/api` calls made through `HttpClient` reach the existing Rust router through the SDK's `HttpBackend`. Calls outside that scope use the normal fetch backend.

The transport carries method, URL, header pairs and body bytes. The Rust operation bounds the collected response body using its configured limit. Application authentication, authorization and API middleware remain responsibilities of that router.

Aborting a call prevents delivery of its result. It does not forcibly interrupt an arbitrary Rust handler already running. Angular's normal transfer cache handles eligible requests; the transport does not invent a separate application cache.

## Failures and shutdown

| Outcome | HTTP behavior |
| --- | --- |
| Full renderer queue | 503 |
| Renderer unavailable | 503 |
| JavaScript or response-conversion failure | Generic 500, details logged on the server |
| Unsupported request-header text | 400 |
| No Angular route response | 404 |

Accepted renders are drained during normal shutdown. Ordinary execution failures do not require discarding every worker. Arbitrary infinite JavaScript, request deadlines and forced native-operation cancellation need further work.
