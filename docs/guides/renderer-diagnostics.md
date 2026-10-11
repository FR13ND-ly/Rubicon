# Renderer Diagnostics

**Status:** Runtime stage collection verified; exploratory coarse timing experiment completed with declared coverage limits
**Created:** 2026-10-11
**Last updated:** 2026-10-11
**Context:** Follow-up to [performance pilot 006](../experiments/006-ssr-performance-pilot.md)
**Repository base:** `1912af2`; this step adds the author's feature, renderer, logging, and HTTP shutdown changes

## Goal and Current State

Locate wall time in the current single worker before changing scheduling, concurrency, or runtime integration. The author has implemented the optional render-timing feature, coarse and per-stage Instant measurements, a bounded asynchronous log writer, and basic graceful HTTP shutdown. Timing collection remains disabled by default.

| Field | Meaning |
| --- | --- |
| queue_us | Job creation to worker processing start, including enqueue/handoff and wake-up costs. |
| call_export_us | The whole call_export operation: preparation, JavaScript invocation and asynchronous waits, and owned result conversion. |
| ok | Whether call_export returned Ok; this does not establish HTTP/content correctness. |

These are wall-time intervals, not thread CPU times. Canceled queued jobs and rejected admissions are outside these processed-job events. The client counters remain necessary.

## Runtime Stage Collection

The follow-up to repository revision 13f3d6e separates call_export into three sequential intervals. Each reached stage emits runtime_stage_timing with stage, elapsed_us, and ok. Renderer instruments the call future with a render span containing request_id. Instrument enters the span when the future is polled, so events remain correlated across asynchronous suspension. Generic calls outside Renderer need their own context if request correlation is required.

| Stage | Interval boundaries and included work |
| --- | --- |
| prepare | Before module namespace access through completion of the preparation closure: export lookup, argument serialization to V8, global handle creation, and local scope teardown. |
| execute | Before argument-array and call-future construction through return of with_event_loop_promise: JavaScript invocation, event-loop polling, Promise settlement, and asynchronous waits. In the current adapter this includes Angular handling and response.text() body buffering. |
| convert | Before opening the result handle scope through completion of serde_v8::from_v8 and scope teardown: deserialization into the owned Rust result, including the response body string. |

Each elapsed value is captured before its event is formatted and enqueued. Instant values, stage events, the request span, and Instrument usage are compiled only with render-timing enabled. No dependency or public call_export signature change is required.

The preparation closure retains errors as a Result so its interval can be logged before propagation. Execution and conversion follow the same result-then-log ordering. A handled preparation failure emits only prepare with ok=false; an execution failure emits prepare and execute; a conversion failure emits all three stages with the last one false. Normal success emits all three with ok=true. The worker still emits its existing coarse event for each completed call, including errors. A stage's ok describes its Result, not HTTP status or correct application content. Panic, cancellation, or a call that never completes does not establish a complete stage sequence.

For a completed call, the sum of its recorded stages can be smaller than call_export_us. The enclosing interval also contains span setup, event formatting/enqueueing, other bookkeeping and cleanup, and timer rounding. The difference is not a direct logging CPU measurement. A successful processed job now emits four timing events instead of one, which changes instrumentation cost and exporter pressure. Evaluate that cost again in a controlled release comparison.

The original benchmark analyzer reads only coarse render_timing records and remains unchanged. The later [stage pipeline](../../benchmarks/diagnostics/stages-README.md) adds a per-request stage join, expected error sequences, compact replay, and a separate initial-render observation. Its preflight verifies complete real smoke sequences and rejects altered logs and changed replay inputs. Existing experiment 007 measurements came from the earlier binaries; they contain no stage measurements and remain valid within their declared scope.

## Implemented Collection

The render-timing feature enables optional tracing 0.1.44, tracing-subscriber 0.3.23, and tracing-appender 0.2.5 dependencies. The executable configures the subscriber; the reusable library emits structured events. The writer queue holds 1,024 entries and uses lossy=true: a full diagnostic queue drops events instead of waiting for output capacity. Main retains the ErrorCounter and WorkerGuard.

Formatting and enqueueing still cost work in the emitting thread. The exporter handles sink writes on its own thread. A bounded entry count is not a general byte-size cap; the selected timing fields are small and the HTTP host generates bounded numeric identifiers.

On Ctrl+C, main signals Axum to stop accepting connections and awaits its HTTP shutdown before dropping the logging guard. It then prints RUBICON_DIAGNOSTICS_DROPPED lines=N outside the renderer. This basic HTTP shutdown does not establish worker joining, active JavaScript interruption, or a shutdown deadline.

The current dependency graph enables Tokio's signal feature through deno_core's full feature. Declare signal explicitly in Rubicon's Tokio dependency when refining its feature list, so direct signal usage does not depend on that transitive configuration.

The existing pilot runner forcibly kills its servers. The separate [diagnostic harness](../../benchmarks/diagnostics/README.md) now uses a verified private console for normal Ctrl+C shutdown and rejects forced exits. Its preflight passed exact event coverage and zero-loss checks. An abrupt termination can bypass guard cleanup. A missing final counter or lost/missing timing records makes a complete internal timing distribution unsupported; it does not erase separately valid client observations.

## Verification on 2026-10-11

A separate ignored target/timing-logger-probe program compiled the selected versions and field types. It emitted three structured events, completed an ephemeral loopback HTTP server's shutdown through an automatically triggered signal future, flushed its logging guard, and reported zero dropped lines. This checks the collection APIs and shutdown composition.

Repository verification used the updated Cargo.lock:

- cargo fmt --manifest-path crates/rubicon/Cargo.toml -- --check passed.
- cargo check --locked --offline --manifest-path crates/rubicon/Cargo.toml passed with the feature disabled and enabled.
- cargo test --locked --offline --manifest-path crates/rubicon/Cargo.toml passed: 8 library tests, 4 executable tests, and all 6 renderer integration fixtures.
- The same full test command with --features render-timing passed with the same counts. A generated deno_core extension doctest was ignored in both configurations.

The first full-suite attempt was blocked by Windows while the existing server held debug/rubicon.exe open. After the author confirmed stopping the server, both complete suites passed. The author's terminal output from the manual Ctrl+C check was not captured by the agent. Local full-suite logs are retained under ignored target/render-timing-full-default-tests.log and target/render-timing-full-feature-tests.log. The existing LNK4098 linker warning remains; an incremental-cache hard-link warning also appeared and Cargo fell back to copying files.

Before interpreting timing distributions under load, capture a real Home response, its timing event, and the final zero-loss counter in one controlled session. Add a deliberate exporter-overflow check and reconcile expected versus recorded timing events. Record instrumented versus uninstrumented cost in a separate release experiment.

The diagnostic preflight subsequently exercised real production Home/About/query/HEAD/404 responses, both feature configurations, and actual Windows Ctrl+C delivery. The final instrumented smoke recorded all 109 expected processed events and a zero-loss final counter. The standalone blocked-sink probe reconciled 65 emitted, 3 retained, and 62 dropped records with a two-entry queue. Four altered-log negative checks were rejected, and both smoke runs replayed byte for byte from compact client and timing data.

[Experiment 007](../experiments/007-renderer-timing-diagnostic.md) completed the 12-case release sequence. Four enabled low-load runs have exact timing coverage and approximately 28 ms median call_export wall time. Both enabled 50/s runs retain unsupported exact coverage because of client timeouts, despite zero final writer-drop counters. All twelve client observations, raw timing records, the interrupted first attempt, and continuation revisions remain preserved. Logging overhead is inconclusive under the short uncontrolled runs. Future measurements need a per-request client/server join, finer intervals, and thread CPU evidence.

The initial eprintln timing block was replaced by a tracing event, preserving its durations and moving sink writes to a separate thread. The later stage implementation separates preparation, execution/Promise settlement, and Rust conversion. JavaScript response.text() and HTTP handling still require their own intervals.

## Stage Verification and Manual Observation on 2026-10-11

The author implemented the proposed stage changes in lib.rs and renderer.rs. Repository formatting passed. After the author stopped the server, both complete repository test suites passed: 8 library tests, 4 executable tests, and all 6 integration fixtures with the feature disabled and enabled. The generated extension doctest remained ignored; the existing LNK4098 warning remained nonfatal. Full-suite logs are retained under ignored target/runtime-stage-full-default-tests.log and target/runtime-stage-full-feature-tests.log.

The previously tested temporary preview passed the same suites. Its dedicated asynchronous probe verified exactly 12 stage events and 4 coarse events across preparation, execution, and conversion failures followed by successful recovery. It checked request context after suspension, expected stage sequences, and that enclosing call durations cover their stage sums. Preview files and logs remain under the ignored target/runtime-stage-preview directory.

The author provided this real application excerpt:

```text
Rubicon is running at http://127.0.0.1:3000
2026-10-11T09:53:10.522688Z  INFO render{request_id=1}: runtime_stage_timing stage="prepare" elapsed_us=147 ok=true
2026-10-11T09:53:11.007546Z  INFO render{request_id=1}: runtime_stage_timing stage="execute" elapsed_us=484789 ok=true
2026-10-11T09:53:11.007669Z  INFO render{request_id=1}: runtime_stage_timing stage="convert" elapsed_us=71 ok=true
2026-10-11T09:53:11.007689Z  INFO render_timing request_id=1 queue_us=34 call_export_us=485290 ok=true
```

| Observed interval | Wall time, ms |
| --- | ---: |
| Queue | 0.034 |
| Preparation | 0.147 |
| Execution and waits | 484.789 |
| Rust conversion | 0.071 |
| Whole call_export | 485.290 |
| Whole call minus stage sum | 0.283 |

Execution and waits account for approximately 99.90% of this one call's enclosing interval. This establishes where that observation's wall time lies; it does not attribute the time to Angular CPU, V8, a timer, or I/O. request_id=1 is consistent with the first SSR request after process startup. Initial rendering setup, JIT work, and asynchronous waits are hypotheses for further investigation. Module/runtime startup occurs before the ready message and is outside this call interval.

The excerpt does not identify build profile, CPU affinity, warm-up state, or HTTP/content validation. If it was produced by the preceding cargo run command without --release, it used a debug build. It cannot be compared directly with the warmed release distributions in experiment 007 or used as a throughput estimate. The author subsequently confirmed Ctrl+C shutdown with RUBICON_DIAGNOSTICS_DROPPED lines=0; the final counter was reported by the author rather than independently captured in the excerpt. Zero reported writer drops alone does not establish complete stage coverage.

The stage coverage/replay pipeline and [experiment 008](../experiments/008-runtime-stage-diagnostic.md) now record the first render separately from warmed requests. Split the JavaScript handler from body buffering and record outer HTTP work in later instrumentation steps. Use thread CPU evidence before attributing execution wall time to computation.

## Primary References

- [Non-blocking writer](https://docs.rs/tracing-appender/latest/tracing_appender/non_blocking/index.html).
- [Queue capacity and lossy behavior](https://docs.rs/tracing-appender/latest/tracing_appender/non_blocking/struct.NonBlockingBuilder.html).
- [Dropped-line counter](https://docs.rs/tracing-appender/latest/tracing_appender/non_blocking/struct.ErrorCounter.html).
