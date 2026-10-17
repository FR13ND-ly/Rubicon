# Benchmark protocol

**Created:** 2026-10-09

**Updated:** 2026-10-17

**Status:** Deferred until the CLI workflow is complete

## Goal

Compare Rubicon and conventional Angular SSR on the same application. Measure latency, successful throughput, errors, CPU and memory. The earlier benchmark implementation has been removed from the working tree; the new suite will be built with the CLI.

## Agreed approach

Python will start and stop servers, coordinate repetitions and collect process resources. k6 will generate HTTP load. Servers will be measured separately, with alternating order between rounds.

Begin with the uncached prototype. Add page complexity, data volume and concurrency as separate dimensions. Later cache, WASM and other optimizations should be compared against this baseline rather than bundled into one change.

## Controls

- Use release Rust and production Angular builds, recording versions and the tested revision.
- Check response content and status before and during a measurement.
- Separate startup and first-render behavior from warmed operation; warm every configured worker.
- Record the complete process tree, including helper processes, when collecting CPU and RAM.
- Declare whether the comparison includes backend transport differences. In-memory Rust routing and direct JavaScript data access are different paths.
- Count failures and rejected requests; do not discard them from the report.

## Report

For each scenario, retain workload parameters, repetitions, latency percentiles, completed requests, errors, CPU time and sampled memory. Raw data stays outside Git. Summaries must identify the configuration that produced them.

Request counts, durations, load levels, memory definitions and acceptance limits will be agreed when the new suite is implemented. No current number in this document is a performance result.
