# Rubicon Rendering Contract

**Status:** Draft
**Created:** 2026-10-09
**Last updated:** 2026-10-17
**Implementation:** Single-entry Angular SDK and reusable native HTTP backend verified; buffered and optional streaming responses supported; general deadlines and native-operation cancellation pending

## Purpose and Context

The rendering contract defines the boundary between the Rust HTTP host and the application's Angular renderer. It specifies request information, response outcomes, lifecycle responsibilities, and failure behavior.

The implementation passes a string request identifier, URL, method, and text header pairs to a JavaScript `render(request)` export and receives status, headers, and either a buffered body or a readable byte stream. The Rust handler validates metadata before HTTP delivery. The [SDK response adapter](../../packages/angular/src/render.ts) constructs a Web Request, invokes the Angular handler, maps null to 404, and preserves separate Set-Cookie fields. The application's [server.rubicon.ts](../../examples/angular-demo/src/server.rubicon.ts) configures its component, existing Angular configuration and backend transport. Buffered delivery remains the demo default; streaming is optional. General request deadlines and cancellation of arbitrary application work remain design work. The worker depends directly on deno_core; replacement engines require integration work.

The intended boundary permits a future runtime replacement without changing application-facing request semantics. Engine lifecycle and integration work still belong to the replacement adapter.

## Scope

The target scope covers buffered and streamed GET/HEAD page responses, request-local context, asynchronous native data access, redirects, missing routes, failures, and cancellation. The implemented boundary below covers only part of this scope.

API request bodies and other methods are handled by explicitly configured host endpoints or rejected by the host's method policy. Bounded response streaming is implemented below. WebSockets, binary state transfer, and arbitrary cross-boundary native objects remain later work.

The adapter must preserve Angular's hydration requirements. Standard Angular TransferState uses JSON; Rubicon does not assume a binary replacement in this prototype.

## Requirements

| ID | Requirement | Acceptance evidence |
| --- | --- | --- |
| RC-01 | A render must receive an absolute URL, method, repeated-header representation, and unique request identifier. | URL, query, method, repeated-header, identifier uniqueness, and exhaustion cases. |
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

AngularAppEngine is the implemented public integration point. Its handle method accepts a Request and optional context and returns a Response or null. The SDK carries the backend and application context in a per-render wrapper and installs its HttpBackend provider during application bootstrap. The same Axum router serves HTTP API requests and native SSR dispatch; applications do not duplicate route mappings. Angular HttpClient and its normal HTTP transfer cache replace the demo's product-specific operation and manual state transfer.

`rb build` uses Angular Architect with generated bootstrap entries and a generated tsconfig in Angular's ignored versioned cache. It preserves existing Angular source and configuration files, including `app.routes.server.ts`. A separately generated Angular 22.2.2 application installed from a local SDK tarball passed SSR, client-only routing, prerendering, route status/header, concurrent native-data, HEAD and managed-shutdown checks; all 14 original application files included in the hash comparison remained unchanged. Compatibility with other Angular versions is not established by this check.

## Contracts and Data

### Implemented Request Object Boundary

`Renderer::render(request: RenderRequest)` returns a buffered `RenderResponse`; `render_stream` permits a streamed body. The public [RenderRequest](../../crates/rubicon/src/request.rs) contains `request_id: String`, `url: String`, `method: String`, and `headers: Vec<(String, String)>` and derives `serde::Serialize`. A queued job owns this request and its reply channel. The worker serializes the request with `serde_v8::to_v8`, invokes its cached render function using `call_with_args`, and immediately drops the Rust request. Active calls retain only their completion state, reply channel, and optional timing metadata. Identifiers become JavaScript strings and header pairs become arrays; values are never interpolated into JavaScript source. General `JavaScriptRuntime::call_export` remains available separately.

GET handlers for `/` and `/{*path}` obtain `OriginalUri` and append its original path and query to `ServerConfig.origin`. The CLI reads and validates `server.origin` from `rubicon.toml`; the Rust configuration also accepts `RUBICON_ORIGIN`. The host does not decode or reconstruct query parameters or derive this origin from request Host or forwarded headers. At build time, the CLI adds the configured origin's hostname to Angular's existing allowed-host list without rewriting `angular.json` or adding a wildcard. The independent consumer check verifies that an unconfigured Host still produces HTTP 400. General proxy trust and URL size policies remain open.

The handler extracts the actual HTTP Method and stores its string form. Application routes take precedence over the browser-file service. ServeDir handles existing browser files with directory index lookup disabled; unmatched GET or HEAD requests reach the Angular adapter. It forwards the method and text header pairs through a Web Request. Angular applies its configured rendering policy, including serving its generated prerendered content; a null result becomes a 404 response. Axum omits HEAD bodies, and buffered representation lengths match GET. The earlier demonstration that displayed and manually escaped request values remains a historical fixture rather than the executable's application entrypoint.

The handler extracts `HeaderMap` and converts each value into a separate name-value pair using `keys()` and `get_all()`. Repeated values remain separate and retain their per-name order; ordering across different names is unspecified. Names use their normalized lowercase form. Values rejected by `HeaderValue::to_str()` produce HTTP 400 before worker admission. This is a provisional supported-text policy, not a classification of every rejected value as invalid HTTP. Arbitrary valid header byte sequences remain unsupported at this page-request boundary. Historical fixtures display `x-rubicon-demo` values with HTML text escaping.

The HTTP handler generates a decimal `request_id` from a process-local atomic counter initialized to 1 before converting headers. `checked_add` prevents wrapping; the last emitted identifier is `u64::MAX - 1`, and subsequent calls at `u64::MAX` produce HTTP 503 with `Request identifiers exhausted.` Client-supplied `X-Request-ID` does not select the identifier. Rejected headers consume an identifier without entering the renderer. Identifiers reset on process restart and are not globally unique or authentication credentials. Direct `Renderer` callers supply their own identifier; the library does not generate or validate uniqueness. Historical fixtures escape the identifier as HTML text, and the host retains a clone for rendering-failure logs. It does not yet include an identifier response header or cover requests rejected outside the page handler.

Queued jobs contain identifier, URL, method, and header pairs. Responses support status, repeated headers, buffered content, and bounded readable byte streams. Axum omits HEAD bodies; buffered representation lengths are known, while streamed lengths can be unknown. General deadline and native-operation cancellation policies remain unfinished.

### Request Model

| Field | Meaning | Ownership and constraints |
| --- | --- | --- |
| Request identifier | Correlates rendering, operations, and cleanup. | Generated by the HTTP host as a string; unique within one process execution, reset on restart, and never wrapped. Direct library callers supply their own identifier. Operations and cleanup association remain future work. |
| URL | Absolute application URL including query parameters. | Validated by the host; proxy-derived values require an explicit trust policy. |
| Method | GET or HEAD for the initial page-rendering contract. | Validated before admission. |
| Headers | Collection of name-value pairs, retaining the order of values for the same name. | Must support repeated fields; ordering across different names is unspecified. The current prototype supports only values accepted by `HeaderValue::to_str()`. |
| Context | Application metadata made available during this request. | Server-side value supplied by `createRequestContext`, carried separately from the backend in the per-render wrapper; not automatically serialized to the browser. Size and field policies remain application responsibilities. |
| Deadline and cancellation state | Controls how long work remains eligible. | Host-owned lifecycle controls; their runtime representation is adapter-specific. |

User identity in context must come from the configured authentication mechanism. An arbitrary context field is not an authorization decision. Server-only credentials and native resource handles are not automatically serialized to the browser.

### Defined Response Type

The library exposes [RenderResponse](../../crates/rubicon/src/response.rs) through `rubicon::RenderResponse`. It derives `Debug` and `serde::Deserialize` and contains:

| Field | Rust type | Intended meaning |
| --- | --- | --- |
| `status` | `u16` | Final HTTP status; the HTTP handler invokes the conversion helper, which accepts 200 through 599 and checks selected body rules. |
| `headers` | `Vec<(String, String)>` | Response name-value pairs, capable of representing repeated fields such as Set-Cookie. |
| `body` | `RenderBody` | Rust-owned buffered content: `Text(String)` or `Bytes(Vec<u8>)`. |

Since 2026-10-14, the JavaScript body may be a string, ArrayBuffer, or ArrayBuffer view. Binary data is copied into an owned Rust vector during result conversion on the renderer thread; a JavaScript subview contributes only its selected bytes. No V8 backing store crosses to the HTTP task. Plain JavaScript arrays and objects remain invalid bodies. Existing JavaScript text responses continue to work. Rust callers construct body values with `.into()`, inspect bytes with `as_bytes()`, or request checked UTF-8 with `as_text()`.

The Angular demo supports `RUBICON_DEMO_BODY_TRANSPORT=text|bytes|stream`, confirmed once at process startup. Text remains the default. Stream requires a server built with the opt-in Cargo feature `streaming`; the demo rejects that mode clearly when the feature is absent. Text and bytes buffer the response using `Response.text()` or `Response.arrayBuffer()`. Stream forwards the Angular Response's readable body without collecting it first. This transports chunks Angular actually produces; it does not add incremental Angular component rendering or replace Angular TransferState.

### Streaming and worker execution (2026-10-15)

The module's `render` function is resolved once when each worker starts. The binding must remain stable for that worker's lifetime; the general `JavaScriptRuntime::call_export` API still resolves exports dynamically. The Rust request is released immediately after serialization. If the caller abandons a pending buffered response, its body and headers are not deserialized; this does not interrupt JavaScript that is already executing.

`RenderResponse<B = RenderBody>` retains its buffered default. `Renderer::render_stream` returns `RenderResponse<RenderContent>`, whose body is either `Buffered(RenderBody)` or, with the `streaming` Cargo feature, `Stream(RenderStream)`. `Renderer::render` remains compatible and collects a streamed body into owned bytes. The HTTP host uses `render_stream` and `into_http_streaming_response` to deliver headers before the complete body. Without the feature, buffered responses keep their direct conversion path and no streaming extension is initialized.

A JavaScript response may return a `ReadableStream<Uint8Array>` as its body when `streaming` is enabled. The bridge splits chunks into at most 64 KiB and awaits a native write into a two-chunk queue. At most two queued chunks and one pending write are owned by this bridge in Rust. One cancellation observer is installed per stream; reading each chunk does not retain another observer on a pending closure promise. An application's upstream stream can retain additional data, including its original unsplit chunk. No V8-backed memory crosses into HTTP tasks.

Streaming retains an admission slot until the producer finishes or the consumer cancels. Dropping the Rust body cancels the JavaScript reader, including a pending read. Shutdown drains accepted streams, so a consumer that never reads or drops its body can delay shutdown; application deadlines remain future work. A fatal runtime failure becomes a stream error rather than a clean end of body. Errors after headers have been sent terminate the body; they cannot replace the already sent status. Stream bodies are rejected for statuses 204, 205 and 304. HEAD sends no body; a streamed response of unknown length does not invent a Content-Length. Buffered HEAD representation lengths remain unchanged.

The opt-in `renderer-batching` Cargo feature admits at most four queued jobs between event-loop polls, stopping admission after a 1 ms observed batch budget or the configured active limit. A synchronous JavaScript call itself cannot be preempted by this budget. The feature is disabled by default pending performance evaluation. Existing timer, error, queue, pool and shutdown rules continue to apply.

The runtime converts status and headers, reads the body property once, and converts buffered bodies directly. Only readable stream bodies enter the JavaScript streaming bridge; buffered calls have no extra Promise wrapper. The worker returns buffered data or a stream receiver through its reply channel. Repository fixtures verify redirects, repeated cookies, Unicode, identifiers, errors and runtime recovery; streaming tests additionally cover early delivery, backpressure, cancellation, empty streams and fatal runtime errors. Conversion failures use `Result conversion failed:`. The HTTP handler validates metadata before delivery. Complete RC-03 outcomes remain future work. See the historical [structured response verification](architecture.md#structured-response-transport-verification).

### Implemented HTTP Response Conversion

The public [into_http_response](../../crates/rubicon/src/http_response.rs) accepts a `RenderResponse` and returns `Result<axum::response::Response, String>`. It validates the complete supplied header list before returning a response. The current host policy accepts final statuses from 200 through 599 and rejects all other numbers, including informational statuses. Nonempty bodies for 204, 205, and 304 are rejected rather than silently discarded; the absence-of-content rules follow [RFC 9110](https://www.rfc-editor.org/rfc/rfc9110.html#section-15).

Header names are validated with `HeaderName::from_bytes`; `HeaderValue::try_from(String)` validates values while reusing their owned byte buffers. Names are normalized before applying the policy rejecting Connection, Content-Length, Transfer-Encoding, Keep-Alive, Proxy-Connection, TE, Trailer, and Upgrade. These fields remain controlled by the HTTP server. Application-supplied Content-Length metadata for HEAD or 304 remains unsupported. Buffered HEAD representation length is preserved; streamed HEAD responses of unknown length omit it.

Accepted pairs are appended to `HeaderMap`, preserving separate Set-Cookie values and their order. The body is moved into `Body::from`, and the validated status and headers are assigned to the Axum response. The helper supplies no default Content-Type, calculates no Content-Length header itself, and does not validate all status-specific application semantics. Applications must supply their intended content type. Response-size limits, conditional-request handling, and actual HTTP transport behavior remain separate work.

Four repository unit tests verify status 302 with Location, two distinct cookies, exact Unicode body bytes, rejected status numbers, invalid header syntax, rejected renderer framing headers, and empty-body requirements. The live worker and handler now use the structured response path. Additional temporary router checks verified metadata delivery, HEAD omission, generic 500 responses for invalid results and metadata, and a subsequent valid request. These are response-object and router checks, not new network delivery evidence. See the [structured response verification](architecture.md#structured-response-transport-verification).

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
| Request identifier counter exhausted | 503 with a generic exhaustion message; implemented for the current page handler. | Keep the counter exhausted and reject before worker admission; do not wrap or reuse identifiers in that execution. |
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
| RC-01 | Exercise query parameters, direct route access, methods, repeated headers, and identifiers. | Request conversion, concurrent identifier generation, and exhaustion checks. | Partial: URL path/query, GET/HEAD method, repeated text headers, and process-local string identifiers verified; arbitrary header bytes unsupported. |
| RC-02 | Alternate contexts and complete an operation after cancellation. | Request-scope checks. | Planned |
| RC-03 | Exercise each terminal outcome and attempted late completion. | Outcome checks. | Planned |
| RC-04 | Check status, redirects, Set-Cookie, and body content. | Response-object and HTTP delivery checks. | Partial: structured worker metadata and body preservation verified; temporary router delivery checks passed; network and Angular integration remain planned. |
| RC-05 | Compare GET representation metadata with HEAD delivery. | HTTP delivery checks. | Partial: earlier demonstration network evidence and temporary structured-response router checks verify body omission and representation length; Angular metadata and broader transport coverage remain planned. |
| RC-06, RC-07 | Expire or cancel work, then render a new request on recovered capacity. | Cleanup and recovery checks. | Partial: continued execution after ordinary synchronous errors, Promise rejection, and conversion errors verified; cancellation, cleanup, and replacement planned. |
| RC-08 | Hydrate a page and perform a representative interaction. | Browser check. | Planned |
| RC-09 | Compare public error responses with internal diagnostics. | Failure reporting review. | Planned |

### URL Transfer Evidence

On 2026-10-10, the repository formatting check, locked offline compilation check for all targets, and full test command passed. The worker integration target alternates two URLs across successful calls 1, 3, and 5, with a JavaScript exception on call 2 and a string-conversion failure on call 4. It checks the exact echoed URL, including query separators and literal apostrophes, after each ordinary failure. This verifies argument association and continued execution; it does not verify isolation of user contexts or recovery from fatal engine failures.

Two HTTP requests to `/products?sort=asc&page=2` and `/search?q=rust&lang=ro` returned 200 HTML responses containing the corresponding absolute URL with `&amp;` in the HTML source. Consecutive counters were 3 and 4. Commands and observations are recorded in the [architecture URL verification](architecture.md#request-url-http-verification). These checks do not establish Angular compatibility, asynchronous renderer behavior, or performance gains.

The owned request refactor was verified on 2026-10-10 with the repository formatting check, locked offline compilation check for all targets, and full test command. Both library tests and the standalone URL/error integration target passed. The [architecture verification](architecture.md#owned-request-type-verification) records the commands and limits. At that milestone, it introduced a Rust type while JavaScript still received only a URL string.

### Request Object and Method Evidence

Later on 2026-10-10, the same Cargo checks passed with `url` and `method` converted into a JavaScript object. Successful integration calls verified GET/A, HEAD/B, and GET/A, retaining exact field values after the two intentional errors. The standalone executable printed `Renderer request and error checks passed.` Argument-conversion failures are classified in code but were not separately induced.

At the request-object milestone, HTTP checks returned 200 HTML for GET, an empty HEAD response, and a subsequent successful GET. The GET counters were 3 and 5. Their Content-Length was 111; HEAD reported 112 because the demonstration included the method name in its generated HTML. The [architecture milestone](architecture.md#request-object-and-method-verification) preserves those observations and the identified mismatch.

### HEAD Metadata Correction Evidence

Later on 2026-10-10, the author applied the demonstration's page-method adjustment. A repeated GET / HEAD / GET sequence returned 200 HTML metadata and Content-Length 111 for all three responses; HEAD delivered no body, and the GET counters were 3 and 5. The HEAD length matched the GET representation for the intervening counter value. A prior router check in a temporary copy also passed with the same adjustment, exact GET HTML, and an empty HEAD body. The [correction milestone](architecture.md#head-representation-metadata-correction) records the checks and their limits. The temporary test is not part of the repository suite.

The actual request method remains available to JavaScript, including `HEAD`; the controlled worker fixture still checks that field. The demonstration now verifies body omission and its representation length. These checks do not establish complete RC-05 acceptance, request-context isolation, Angular compatibility, or performance gains.

### Request Header Evidence

On 2026-10-10, formatting, locked offline compilation for all targets, and the complete repository test command passed after completing the header changes. Two library tests, two executable header-conversion tests, and the standalone worker integration executable succeeded. The worker fixture still uses empty header lists and verifies URL/method association after its intentional errors.

Actual HTTP GET / HEAD / GET checks with two `X-Rubicon-Demo` values returned 200 and Content-Length 163. GET displayed both escaped values in order and counters 1 and 3. HEAD reported the length of the corresponding GET representation. A prior router test in a temporary copy checked the same proposal's repeated values, empty HEAD body, unsupported-value 400, and continued execution without rendering the rejected request. That temporary test is not part of the repository suite. The [architecture milestone](architecture.md#request-header-verification) records commands, observations, and limitations.

These checks support RC-01 for the current text-header policy. They do not verify user-context isolation, all valid HTTP header byte sequences, complete application response metadata, Angular compatibility, or performance gains.

### Request Identifier Evidence

Later on 2026-10-10, formatting, locked offline compilation for all targets, and the complete repository test command passed. Two library tests, four executable unit tests, and the standalone worker integration executable succeeded. Identifier tests verified 256 distinct values across four threads and refusal to wrap after exhaustion. Exact integration comparisons preserved large string identifiers above JavaScript's safe integer range, including after the fixture's intentional execution and conversion errors.

Actual HTTP checks generated identifiers 1, 2, and 3 for GET / HEAD / GET despite a client-supplied identifier header. Responses returned 200 with the HTML content type and Content-Length 183 for the checked single-digit representations. A subsequent unsupported-header request returned 400; the next GET displayed identifier 5 with render counter 4. The identifier therefore belongs to the HTTP handler and is independent of successful render count. Exhaustion was induced in a local unit-test counter, rather than by exhausting the live server. The [architecture milestone](architecture.md#request-identifier-verification) records commands, exact cases, and limitations.

These checks support process-local identity and lossless string transfer. They do not verify authentication, request-context isolation, globally unique identifiers, lifecycle controls, complete application response metadata, Angular compatibility, or performance gains.

### Structured Response Transport Evidence

On 2026-10-10, formatting, locked offline compilation for all targets, and the complete repository test suite passed with the connected response path. Six library tests, four executable unit tests, and the standalone worker integration target succeeded. Integration comparisons verify status, exact header pairs, and Unicode body content for successful calls 1, 3, and 5; ordinary execution and body-conversion failures on calls 2 and 4 do not prevent later results. An intentional application 404 remains a successful `RenderResponse`.

A temporary proposal check exercised the handler through an Axum Router, verifying redirects, repeated cookies, UTF-8 body length, empty HEAD bodies, and generic 500 mapping for execution errors, malformed results, and invalid HTTP metadata. Those tests and their Tower dependency are not retained in the repository, and new network checks were not run for this milestone. The [architecture milestone](architecture.md#structured-response-transport-verification) records exact cases and limits. Angular outcomes, async rendering, context isolation, cancellation, cleanup, and performance remain unverified.

### Asynchronous Renderer Evidence

On 2026-10-10, formatting, locked offline compilation for all targets, and the complete repository test suite passed after adding the [async fixture](../../crates/rubicon/tests/fixtures/renderer_async.mjs). The integration executable runs the same five response/error checks against the synchronous fixture and its async wrapper, using a fresh worker for each. The wrapper imports the synchronous function, awaits `Promise.resolve()`, and then delegates. Valid responses are deserialized after fulfillment; the intentional throw becomes a rejected Promise, and a fulfilled response with an object body fails typed conversion. Calls after either failure succeed with exact status, response headers, body, and request fields.

Six library tests and four executable unit tests passed, and both fixture success messages were printed. No source change to the runtime or HTTP handler and no dependency update was required. The [architecture milestone](architecture.md#asynchronous-renderer-verification) records exact checks. This establishes Promise and microtask behavior in the controlled fixture, rather than asynchronous native I/O, cancellation, cleanup guarantees, or Angular compatibility. Test wait limits do not establish interruption of JavaScript or recovery from a permanently pending Promise.

## Open Questions

| Question | Impact | Next action |
| --- | --- | --- |
| What complete module export and Rust/TypeScript types implement the Angular boundary? | Defines Angular application integration beyond the current structured request and response objects. | Extend the prototype through the Angular renderer spike. |
| Which context fields and encodings are supported? | Defines data compatibility and size limits. | Specify the example service and transfer needs. |
| How are asynchronous native operations cancelled? | Defines cleanup guarantees. | Prototype success, failure, and late completion. |
| What is the effective URL and trusted-proxy policy? | Affects routing and request validation. | Select a deployment configuration. |
| Should future responses support streaming? | Changes response ownership and commitment rules. | Evaluate buffered behavior first. |

## Related Decisions and Sources

- [Architecture](architecture.md).
- [Benchmark protocol](benchmark-protocol.md).
- [ADR 001: JavaScript runtime selection](../adr/001-runtime-selection.md).
- [AngularAppEngine API](https://angular.dev/api/ssr/AngularAppEngine), rolling documentation checked on 2026-10-10; reference application pinned to Angular 22.2.2, with the Rubicon adapter still pending.
- [Angular hydration](https://angular.dev/guide/hydration), rolling documentation checked on 2026-10-09.
- [Angular TransferState](https://angular.dev/api/core/TransferState), rolling documentation checked on 2026-10-09.
- [Angular SSR guide](https://angular.dev/guide/ssr), request context and transfer-cache documentation checked on 2026-10-09.
- [RFC 9110 Content-Length semantics](https://www.rfc-editor.org/rfc/rfc9110.html#section-8.6), checked on 2026-10-10 for HEAD representation metadata.
