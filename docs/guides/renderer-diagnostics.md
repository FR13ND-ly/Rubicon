# Renderer Diagnostics

**Status:** Basic collection implemented and test suites verified; measurement coverage and cost evaluation pending
**Created:** 2026-10-11
**Last updated:** 2026-10-11
**Context:** Follow-up to [performance pilot 006](../experiments/006-ssr-performance-pilot.md)
**Repository base:** `1912af2`; this step adds the author's feature, renderer, logging, and HTTP shutdown changes

## Goal and Current State

Locate wall time in the current single worker before changing scheduling, concurrency, or runtime integration. The author has implemented the optional render-timing feature, coarse Instant measurements, a bounded asynchronous log writer, and basic graceful HTTP shutdown. Timing collection remains disabled by default. Both complete test suites passed after stopping the existing local server.

| Field | Meaning |
| --- | --- |
| queue_us | Job creation to worker processing start, including enqueue/handoff and wake-up costs. |
| call_export_us | The whole call_export operation: preparation, JavaScript invocation and asynchronous waits, and owned result conversion. |
| ok | Whether call_export returned Ok; this does not establish HTTP/content correctness. |

These are wall-time intervals, not thread CPU times. Canceled queued jobs and rejected admissions are outside these processed-job events. The client counters remain necessary.

## Implemented Collection

The render-timing feature enables optional tracing 0.1.44, tracing-subscriber 0.3.23, and tracing-appender 0.2.5 dependencies. The executable configures the subscriber; the reusable library emits structured events. The writer queue holds 1,024 entries and uses lossy=true: a full diagnostic queue drops events instead of waiting for output capacity. Main retains the ErrorCounter and WorkerGuard.

Formatting and enqueueing still cost work in the emitting thread. The exporter handles sink writes on its own thread. A bounded entry count is not a general byte-size cap; the selected timing fields are small and the HTTP host generates bounded numeric identifiers.

On Ctrl+C, main signals Axum to stop accepting connections and awaits its HTTP shutdown before dropping the logging guard. It then prints RUBICON_DIAGNOSTICS_DROPPED lines=N outside the renderer. This basic HTTP shutdown does not establish worker joining, active JavaScript interruption, or a shutdown deadline.

The current dependency graph enables Tokio's signal feature through deno_core's full feature. Declare signal explicitly in Rubicon's Tokio dependency when refining its feature list, so direct signal usage does not depend on that transitive configuration.

The existing pilot runner forcibly kills its servers. Adapt that lifecycle before relying on end-of-process diagnostics: an abrupt termination can bypass guard cleanup. A missing final counter or lost/missing timing records makes a complete internal timing distribution unsupported; it does not erase separately valid client observations.

## Verification on 2026-10-11

A separate ignored target/timing-logger-probe program compiled the selected versions and field types. It emitted three structured events, completed an ephemeral loopback HTTP server's shutdown through an automatically triggered signal future, flushed its logging guard, and reported zero dropped lines. This checks the collection APIs and shutdown composition.

Repository verification used the updated Cargo.lock:

- cargo fmt --manifest-path crates/rubicon/Cargo.toml -- --check passed.
- cargo check --locked --offline --manifest-path crates/rubicon/Cargo.toml passed with the feature disabled and enabled.
- cargo test --locked --offline --manifest-path crates/rubicon/Cargo.toml passed: 8 library tests, 4 executable tests, and all 6 renderer integration fixtures.
- The same full test command with --features render-timing passed with the same counts. A generated deno_core extension doctest was ignored in both configurations.

The first full-suite attempt was blocked by Windows while the existing server held debug/rubicon.exe open. After the author confirmed stopping the server, both complete suites passed. The author's terminal output from the manual Ctrl+C check was not captured by the agent. Local full-suite logs are retained under ignored target/render-timing-full-default-tests.log and target/render-timing-full-feature-tests.log. The existing LNK4098 linker warning remains; an incremental-cache hard-link warning also appeared and Cargo fell back to copying files.

Before interpreting timing distributions under load, capture a real Home response, its timing event, and the final zero-loss counter in one controlled session. Add a deliberate exporter-overflow check and reconcile expected versus recorded timing events. Record instrumented versus uninstrumented cost in a separate release experiment.

The initial eprintln timing block has been replaced by a tracing event. This step preserves its durations and moves sink writes to a separate thread. Later separate export preparation, Promise settlement, JavaScript response.text(), Rust conversion, and HTTP handling, with explicit interval boundaries.

## Primary References

- [Non-blocking writer](https://docs.rs/tracing-appender/latest/tracing_appender/non_blocking/index.html).
- [Queue capacity and lossy behavior](https://docs.rs/tracing-appender/latest/tracing_appender/non_blocking/struct.NonBlockingBuilder.html).
- [Dropped-line counter](https://docs.rs/tracing-appender/latest/tracing_appender/non_blocking/struct.ErrorCounter.html).
