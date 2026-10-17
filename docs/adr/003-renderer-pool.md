# ADR 003: Bounded renderer pool

**Recorded:** 2026-10-17

**Status:** Accepted; records an existing project decision

## Context

One serial renderer leaves capacity unused while a render awaits asynchronous data. Adding runtime instances can use more cores, but each instance also consumes memory.

## Decision

Use persistent worker threads, each owning one runtime, with a shared bounded `async-channel` queue. Configure the worker count, active renders per worker and queue capacity separately.

Defaults are one worker, 16 active renders per worker and 16 queued requests. A full queue rejects work instead of growing without a limit.

## Alternatives

Serial execution is simpler but cannot overlap asynchronous waits. One runtime per request repeats startup work. An unlimited queue defers overload into rising latency and memory use. A custom queue would add synchronization code that `async-channel` already supplies.

## Consequences

Overlapping renders require request-local state. They do not make JavaScript execute simultaneously inside one isolate. More workers increase the number of JavaScript threads and runtime instances, so their memory and throughput tradeoff must be measured.

Normal shutdown stops admission, drains accepted work and joins the workers. Infinite JavaScript still requires a future interruption policy.

Implementation: [renderer](../../crates/rubicon/src/renderer.rs) and [configuration](../../crates/rubicon/src/config.rs).
