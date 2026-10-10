# Rubicon Rendering Contract

**Status:** Draft
**Created:** 2026-10-09
**Last updated:** 2026-10-10
**Implementation:** Partial URL-and-method request object; Angular adapter not implemented; HEAD metadata correction pending

## Purpose and Context

The rendering contract defines the boundary between the Rust HTTP host and the application's Angular renderer. It specifies request information, response outcomes, lifecycle responsibilities, and failure behavior.

This is a semantic contract for the intended prototype. The current implementation passes an object containing URL and method to a JavaScript `render(request)` export and receives an HTML string. The complete Angular adapter's Rust types, TypeScript exports, module names, and serialization representation remain TBD. A runtime adapter implements this contract for a particular engine.

The intended boundary permits a future runtime replacement without changing application-facing request semantics. Engine lifecycle and integration work still belong to the replacement adapter.

## Scope

The target scope covers buffered responses for GET and HEAD application-page requests, request-local context, asynchronous native data access, redirects, missing routes, failures, and cancellation. The implemented request boundary below covers only part of this scope.

API request bodies and other methods are handled by explicitly configured host endpoints or rejected by the host's method policy. Streaming responses, WebSockets, binary state transfer, and arbitrary cross-boundary native objects are later work.

The adapter must preserve Angular's hydration requirements. Standard Angular TransferState uses JSON; Rubicon does not assume a binary replacement in this prototype.

## Requirements

| ID | Requirement | Acceptance evidence |
| --- | --- | --- |
| RC-01 | A render must receive an absolute URL, method, repeated-header representation, and unique request identifier. | URL, query, method, and repeated-header cases. |
| RC-02 | Context and native operations must be scoped to the current request. | Alternating identities and late-result checks. |
| RC-03 | Rendering must return exactly one terminal outcome: response, no matching route, or failure. | Outcome and duplicate-completion checks. |
| RC-04 | A successful outcome must preserve status, response-header multiplicity, and body content. | HTTP response checks, including Set-Cookie and redirects. |
| RC-05 | HEAD responses must deliver no body while preserving applicable response metadata. | Comparison with GET and transport checks. |
| RC-06 | Cancellation and deadline expiry must prevent a late response from being delivered and release request resources. | Disconnect, timeout, and resource checks. |
| RC-07 | Runtime state must remain safe for reuse or the worker must be replaced. | Failure followed by a valid request. |
| RC-08 | Delivered HTML must hydrate with browser assets from the same build. | Browser hydration and interaction check. |
| RC-09 | Failure classes must remain observable without exposing internal diagnostic details to clients. | Public-response and diagnostic checks. |

## Design and Request Flow

1. The host validates the HTTP request, chooses the application, and assigns request identity and lifecycle controls.
2. The runtime adapter converts supported fields and invokes the application's renderer.
3. The Angular adapter creates an Angular-compatible request and supplies request-local context.
4. Rendering may await scoped native data operations.
5. The adapter converts the Angular outcome into one contract outcome.
6. The host validates status and headers, applies HTTP transport rules, and delivers or discards the result according to cancellation state.
7. All request-owned resources are released, including pending operations and Angular application resources.

AngularAppEngine is a candidate public integration point because its handle method accepts a Request and optional context and returns a Response or null. Compatibility with the embedded environment still requires validation.

## Contracts and Data

### Implemented Request Object Boundary

`Renderer::render(request: RenderRequest)` accepts an owned Rust request. The public [RenderRequest](../../crates/rubicon/src/request.rs) contains `url: String` and `method: String` and derives `serde::Serialize`. Each `RenderJob` owns that request and its own reply channel to the persistent worker. The worker borrows `job.request`; the generic `JavaScriptRuntime::call_string_export<T: serde::Serialize>` converts it directly into a V8 object through `serde_v8::to_v8` and invokes the export through `call_with_args`. Values are not interpolated into JavaScript source. The module's `render(request)` export returns a string that the host delivers through Axum's `Html` response.

GET handlers for `/` and `/{*path}` obtain `OriginalUri` and append its original path and query to the fixed application origin `http://127.0.0.1:3000`. The host does not decode or reconstruct query parameters. Source review confirms that request `Host` and forwarded headers are not used to construct this origin; header-spoofing behavior has not been separately exercised. Configurable origins, trusted proxies, and URL size and validation policies remain open.

The handler also extracts the actual HTTP `Method` and stores its string form. The demonstration module escapes `&`, `<`, `>`, double quotes, and apostrophes when displaying either field as HTML text. Every matched GET or HEAD page path invokes the demonstration renderer; this does not implement Angular route matching, application 404 outcomes, or precedence for API and browser-asset paths.

The job currently contains only URL and method from the request model. Repeated headers, request identifier, context, native services, and lifecycle controls remain to be added. A string result also does not implement the complete terminal-outcome and response-metadata models below. Axum omits the HEAD body, but the demonstration's Content-Length is currently based on HTML displaying `HEAD` rather than the corresponding GET representation; RC-05 remains incomplete.

### Request Model

| Field | Meaning | Ownership and constraints |
| --- | --- | --- |
| Request identifier | Correlates rendering, operations, and cleanup. | Generated by the host; unique among active requests. |
| URL | Absolute application URL including query parameters. | Validated by the host; proxy-derived values require an explicit trust policy. |
| Method | GET or HEAD for the initial page-rendering contract. | Validated before admission. |
| Headers | Ordered collection of name-value pairs. | Must support repeated fields; a simple string map is insufficient. |
| Context | Application metadata made available during this request. | JSON-compatible values with explicit size and field policy. |
| Deadline and cancellation state | Controls how long work remains eligible. | Host-owned lifecycle controls; their runtime representation is adapter-specific. |

User identity in context must come from the configured authentication mechanism. An arbitrary context field is not an authorization decision. Server-only credentials and native resource handles are not automatically serialized to the browser.

### Terminal Outcomes

| Outcome | Contents | Host responsibility |
| --- | --- | --- |
| Response | Valid final status, header pairs, and buffered body. | Preserve application semantics and apply transport rules. |
| No matching route | An explicit unmatched result. | Apply the configured fallback; the prototype default is 404. |
| Failure | Classified error and diagnostic correlation identifier. | Record it and generate the documented public error response. |

Application responses such as 401, 403, 404, or redirects remain response outcomes when produced intentionally. They must not be relabeled as runtime failures.

The initial HTML body is UTF-8 text, converted to bytes for HTTP delivery. Conversion and size checks must occur before a response is committed. Empty bodies are valid when the method or status requires them.

The host must preserve repeated response fields such as Set-Cookie and must not comma-join them indiscriminately. Transport-owned fields, including connection handling and body-length calculation, remain controlled by the HTTP layer. A changed body requires correct length metadata.

### Native Data Operations

A native operation receives explicit arguments within a host-established request scope and returns serializable data or an explicit service error. Request association must not depend on an unvalidated identifier supplied by application input.

The initial data boundary uses documented JSON-compatible values. Date-like values, large integers, binary data, and class instances require explicit encoding if introduced. The adapter must associate pending operations with their originating request and discard late results after that request ends.

Custom native data operations must define how data needed for hydration reaches the browser. They must not assume Angular's HTTP transfer cache automatically captures calls that bypass HttpClient.

## Lifecycle, Concurrency, and Isolation

A request moves through admitted, queued or running, terminal, and cleaned-up states. Cancellation or deadline expiry may happen before execution, during a native operation, or during JavaScript execution.

Loaded modules and runtime initialization may persist. Request contexts, document state, user data, pending operations, and response buffers have a request-scoped lifetime.

Cleanup must occur after success, missing-route results, failure, cancellation, and deadline expiry. Completion callbacks from an old request must not modify a later request's state. A worker returns to the available pool only after cleanup is complete and its state is known to be reusable.

Cancellation of a Rust future alone does not establish interruption of synchronous JavaScript. The runtime adapter must verify its actual interruption and recovery behavior.

## Failure and Resource Limits

The following mapping is proposed for the initial host and remains subject to review:

| Failure or condition | Proposed public behavior | Recovery |
| --- | --- | --- |
| Invalid request | 400 with a generic message. | Reject before rendering. |
| Unsupported page-rendering method | 405 with the configured allowed-method information. | Dispatch to a configured endpoint or reject before rendering. |
| No matching application route | 404 unless an explicit fallback is configured. | Complete normally. |
| Application rendering exception | 500 with a generic message. | Clean up; reuse only if state is valid. |
| Runtime initialization or execution failure | 500 for the affected render; unavailable capacity is reflected in readiness. | Replace the worker when recovery is uncertain. |
| Queue full or no usable rendering capacity | 503 with an explicit rejection classification in diagnostics. | Do not admit additional rendering work. |
| Render deadline exceeded | 503 with deadline expiry recorded separately from overload. | Interrupt or cancel work and verify cleanup. |
| Client disconnected | No response delivered to that client. | Cancel or discard work and release resources. |

Automatic render retries are outside the initial policy. Any future retry must account for deadlines, side effects, and duplicate operations.

The current worker has a provisional hardcoded queue capacity of 16 waiting jobs. Configurable capacity, deadlines, input/context size, and maximum buffered response size remain TBD. These values must be selected and recorded before benchmark execution. Exceeding a limit must produce an explicit outcome rather than a partial successful page.

## Performance Expectations

**Not measured.**

The contract favors a small number of coarse interactions and explicit data conversion. Buffered HTML simplifies the prototype but affects memory use and when body content becomes available.

The benchmark must account for conversion and data-copy costs. Streaming would require an extension covering chunk ownership, cancellation, response commitment, and errors after headers are sent.

## Verification

| Requirement | Verification method | Planned evidence | State |
| --- | --- | --- | --- |
| RC-01 | Exercise query parameters, direct route access, methods, and repeated headers. | Request conversion checks. | Partial: URL path/query and GET/HEAD method transfer verified; headers and request identifier planned. |
| RC-02 | Alternate contexts and complete an operation after cancellation. | Request-scope checks. | Planned |
| RC-03 | Exercise each terminal outcome and attempted late completion. | Outcome checks. | Planned |
| RC-04 | Check status, redirects, Set-Cookie, and body content. | HTTP delivery checks. | Partial: successful 200 HTML delivery verified; application response metadata planned. |
| RC-05 | Compare GET representation metadata with HEAD delivery. | HTTP delivery checks. | Partial: empty HEAD body verified; demonstration Content-Length mismatch requires correction. |
| RC-06, RC-07 | Expire or cancel work, then render a new request on recovered capacity. | Cleanup and recovery checks. | Partial: continued execution after ordinary JavaScript and conversion errors verified; cancellation, cleanup, and replacement planned. |
| RC-08 | Hydrate a page and perform a representative interaction. | Browser check. | Planned |
| RC-09 | Compare public error responses with internal diagnostics. | Failure reporting review. | Planned |

### URL Transfer Evidence

On 2026-10-10, the repository formatting check, locked offline compilation check for all targets, and full test command passed. The worker integration target alternates two URLs across successful calls 1, 3, and 5, with a JavaScript exception on call 2 and a string-conversion failure on call 4. It checks the exact echoed URL, including query separators and literal apostrophes, after each ordinary failure. This verifies argument association and continued execution; it does not verify isolation of user contexts or recovery from fatal engine failures.

Two HTTP requests to `/products?sort=asc&page=2` and `/search?q=rust&lang=ro` returned 200 HTML responses containing the corresponding absolute URL with `&amp;` in the HTML source. Consecutive counters were 3 and 4. Commands and observations are recorded in the [architecture URL verification](architecture.md#request-url-http-verification). These checks do not establish Angular compatibility, asynchronous renderer behavior, or performance gains.

The owned request refactor was verified on 2026-10-10 with the repository formatting check, locked offline compilation check for all targets, and full test command. Both library tests and the standalone URL/error integration target passed. The [architecture verification](architecture.md#owned-request-type-verification) records the commands and limits. At that milestone, it introduced a Rust type while JavaScript still received only a URL string.

### Request Object and Method Evidence

Later on 2026-10-10, the same Cargo checks passed with `url` and `method` converted into a JavaScript object. Successful integration calls verified GET/A, HEAD/B, and GET/A, retaining exact field values after the two intentional errors. The standalone executable printed `Renderer request and error checks passed.` Argument-conversion failures are classified in code but were not separately induced.

Actual HTTP checks returned 200 HTML for GET, an empty HEAD response, and a subsequent successful GET. The GET counters were 3 and 5. Their Content-Length was 111; HEAD reported 112 because the demonstration includes the method name in its generated HTML. The [architecture milestone](architecture.md#request-object-and-method-verification) records commands, observations, and the required metadata correction. These checks do not establish complete RC-05 acceptance, request-context isolation, Angular compatibility, or performance gains.

## Open Questions

| Question | Impact | Next action |
| --- | --- | --- |
| What complete module export and Rust/TypeScript types implement the Angular boundary? | Defines application integration beyond the current string argument. | Extend the URL prototype through the Angular renderer spike. |
| Which context fields and encodings are supported? | Defines data compatibility and size limits. | Specify the example service and transfer needs. |
| How are asynchronous native operations cancelled? | Defines cleanup guarantees. | Prototype success, failure, and late completion. |
| What is the effective URL and trusted-proxy policy? | Affects routing and request validation. | Select a deployment configuration. |
| Should future responses support streaming? | Changes response ownership and commitment rules. | Evaluate buffered behavior first. |

## Related Decisions and Sources

- [Architecture](architecture.md).
- [Benchmark protocol](benchmark-protocol.md).
- [ADR 001: JavaScript runtime selection](../adr/001-runtime-selection.md).
- [AngularAppEngine API](https://angular.dev/api/ssr/AngularAppEngine), rolling documentation checked on 2026-10-09; selected Angular version TBD.
- [Angular hydration](https://angular.dev/guide/hydration), rolling documentation checked on 2026-10-09.
- [Angular TransferState](https://angular.dev/api/core/TransferState), rolling documentation checked on 2026-10-09.
- [Angular SSR guide](https://angular.dev/guide/ssr), request context and transfer-cache documentation checked on 2026-10-09.
- [RFC 9110 Content-Length semantics](https://www.rfc-editor.org/rfc/rfc9110.html#section-8.6), checked on 2026-10-10 for HEAD representation metadata.
