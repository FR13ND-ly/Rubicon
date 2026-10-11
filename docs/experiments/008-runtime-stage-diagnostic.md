# 008 — First Render and Runtime Stage Diagnostic

**Status:** Protocol frozen; preflight verified; release measurements pending
**Created:** 2026-10-11
**Last updated:** 2026-10-11
**Workload:** W-01, unchanged production Angular Home SSR
**Application source:** fc3f69c
**Previous experiment:** [007](007-renderer-timing-diagnostic.md)

## Question and Method

Determine where call_export wall time lies at the tested passing-load candidates, and separate the initial render from warmed exports. The author has implemented prepare, execute, and convert timing events. Execution includes JavaScript/Angular handling, asynchronous waits, and adapter body buffering. The three phases alone do not distinguish computation from waiting or identify a particular Angular function.

[The frozen plan](../../benchmarks/diagnostics/stages-config.json) contains eight fresh-process cases: feature off/on, 5 and 20 requests/s, two independent repetitions. Rate and variant order alternate as in experiment 007. Warm-up is 10 seconds at 20/s, settle is two seconds, and measurement is 20 seconds. The server uses affinity mask 1; k6 uses mask 20 with GOMAXPROCS=2. The 128-VU cap, two-second request timeout, three-second drain allowance, half-second resource sampling, and exploratory correctness/latency limits are unchanged. No overload case or new capacity estimate is included.

The same production Angular artifacts are reused and hashed. Both Rust binaries use the same author source and locked dependency graph. The instrumented variant now emits four records per successful export; instrumentation cost must be assessed anew. Build/source hashes are checked before launch and recorded with the protocol. CPU allocation is one logical CPU, not proof of physical-core separation. Memory is observed without a cap; thermal/power state and background activity remain uncontrolled.

## First Render and Shared Cohorts

Wait for the ready banner before sending the first HTTP request. Correctness-check that initial Home response and retain its .NET client wall time. In the enabled case, initial server ID 1 and its stages must be the only export inside that control interval. This is an ordered-control association rather than a response-header client/server join. It excludes process/module startup; client-control time includes connection and decoding and varies with controller initialization.

For warmed requests, join all reached stages to their enclosing export by request ID, then select one export-completion window. Preparation, execution, conversion, their sum, and residual distributions share the same server cohort. k6 latency uses a separate correct-response cohort including its tail; do not add or subtract separate percentiles as an exact breakdown.

## Validation and Retention

The [stage preflight](../../benchmarks/diagnostics/stage-preflight.json) verifies real HTTP gates, expected stage coverage, normal private-console Ctrl+C shutdown, zero reported writer losses, and compact replay. Altered logs test missing, duplicated, reversed, unknown, wrongly associated, or late stages, incorrect outcomes, compact corruption, and undeclared schema. Original coarse analysis remains unchanged, preserving earlier data semantics. Details and replay commands are in the [stage guide](../../benchmarks/diagnostics/stages-README.md).

All planned observations will remain visible, including failed limits or unsupported coverage. Stage summary validity inherits the coarse diagnostic and client requirements. Zero writer losses alone does not establish exact populations. The full run starts only after committing the harness and protocol; compact samples, resource traces, metadata, and summaries will be versioned, with raw streams and binaries retained locally.

## Results

Pending the frozen release run. The preceding smoke observations validate collection only. They must not be reported as measured capacity, a logging speedup, or precise instrumentation overhead.
