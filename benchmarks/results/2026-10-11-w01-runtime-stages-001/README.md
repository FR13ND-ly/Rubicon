# First Render and Runtime Stage Results

**Status:** Original eight-case plan completed; one invalid warm-up and one valid latency-limit failure retained
**Date:** 2026-10-11
**Application source:** fc3f69c
**Collection source:** 38b22f7, clean checkout
**Protocol:** [stages-config.json](../../diagnostics/stages-config.json)
**Report:** [Experiment 008](../../../docs/experiments/008-runtime-stage-diagnostic.md)

All 2,005 measured requests completed with correct Home content, with zero measured failures or dropped arrivals. Seven cases meet validity requirements and six pass exploratory limits. The last case, on-r2-5rps, retains valid=false: warm-up had 178 correct responses, one timeout, and 22 dropped arrivals at the fixed 16-VU cap. Later measured success does not repair that prerequisite. off-r1-20rps is valid but its client p95 of 134.75 ms exceeds the 100 ms limit. Cases were not rerun or replaced.

The three verified enabled sessions contain 1,526 exports and 4,578 stage records. Their warmed server cohorts contain 100/400/400 exports; mean call_export is 28.33/29.80/30.03 ms and first export is 102.74/91.29/93.74 ms. Execution/waiting dominates wall time. Another 286 observed exports and 858 stage records remain preserved in the invalid case; sound observed joins and zero writer losses do not establish its exact request population.

verification.json records 127 checked collection/source/binary/browser/per-run artifact hashes and 24 byte-identical compact summary replays, including the invalid summaries. All eight servers stopped normally through verified private consoles. All four enabled loss counters were zero. Previous coarse datasets remain unchanged. This verifies retained evidence and classifications; it does not make every measurement valid.

manifest.json, plan.json, and per-run metadata are original collection records. The final plot was clarified after collection to mark invalid client observations and keep legends away from data. Its current source and output hashes are recorded separately in verification.json; the manifest's original plot hash still identifies the committed collection version. The numerical analysis scripts and recorded classifications are unchanged.

Raw streams/process logs and release binaries remain local in ignored paths. Compact client samples, server stage/export events, coarse timings, resource traces, summaries, and PNG/SVG figures are versioned. Replays use the commands in the [stage guide](../../diagnostics/stages-README.md); an invalid stage replay intentionally exits with status 2 after reproducing its preserved summary.

First render excludes process/module startup and does not reset OS/filesystem caches. Client latency and server stages use different populations. Stages measure wall time including waits, and two short laptop repetitions do not establish precise logging overhead, maximum throughput, or production readiness.
