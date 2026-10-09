# Evaluating Rubicon Performance

Use this guide to write the [benchmark protocol](../specs/benchmark-protocol.md) and an experiment record under `docs/experiments/`. The protocol defines the method; each experiment preserves the configuration and evidence from actual runs. No performance results are established by this guide.

## Define the Objective

Define performance for a workload and a resource budget. A useful primary objective is the highest sustained rate of correct responses that meets declared latency, error, and memory limits.

Choose thresholds before comparing candidates. For example, p95 below 200 ms and request failures below 0.1% could be experimental acceptance criteria; these numbers are illustrative and must be justified for the selected workload. k6 can express pass/fail conditions using [thresholds](https://grafana.com/docs/k6/latest/using-k6/thresholds/).

Treat every proposed optimization as a hypothesis. A valid experiment may show a regression, a workload-specific tradeoff, or no detectable difference.

## Establish Comparable Variants

Use Angular SSR on Node as a reference, a minimal correct Rubicon implementation as the initial candidate, and separately identified optimized variants. Keep the Angular application, version, output, data, and request semantics equivalent.

Evaluate both a single-core budget and, where relevant, a fixed multicore budget. Allow each implementation to use that budget appropriately; comparing a single Node worker with a Rust pool using the whole machine would confound architecture with resource allocation.

Match cache policy, compression, TLS, connection reuse, data-service configuration, and logging for comparisons intended to isolate the host. Record unavoidable differences, including JavaScript engine versions. If several factors change, describe the result as a comparison of complete systems.

A future comparison with a Bun subprocess includes IPC and process-management costs. It cannot by itself establish that JavaScriptCore is faster or slower than V8. Keep the experiment independent of whether a Bun embedding becomes available.

## Choose Workloads

| Workload | Controlled inputs | Question |
| --- | --- | --- |
| Small SSR page | Fixed components and data, no external I/O | What is the cost of a basic render? |
| Complex SSR page | Fixed component tree, bindings, and fixture | How does rendering scale with Angular work? |
| Large HTML response | Fixed row count and output size | What are the rendering and response-transfer costs? |
| SSR with controlled asynchronous data | Fixed service behavior and request sequence | How does the host behave while waiting for data? |
| SSR with a real database | Versioned dataset, query, indexes, and connection limits | How does the complete application perform? |
| Cached or prerendered HTML | Explicit keys, request distribution, initial state, and policy | What is the serving capacity when rendering is avoided? |

Keep static serving, cache hits, and dynamic SSR results separate. For cache experiments, report observed hit rates, misses, and revalidation behavior. Check that personalized content follows the intended isolation policy.

Use a versioned or seeded request sequence and include multiple routes or identities where needed. State whether an optimization changes the amount of work performed rather than the cost of the same work.

## Check Correctness First

Verify status, headers, expected content, and request-specific data. Exercise error paths and request isolation. For SSR, run browser checks for hydration and a representative interaction before interpreting the load results.

During the load test, perform inexpensive response checks and report incorrect responses as failures. Account for timed-out, rejected, and incomplete requests. A fast HTTP 200 response with missing content is not a successful render.

## Measure at the Client and Server

Record offered load, achieved correct responses per second, latency p50/p95/p99, failures, timeouts, and rejections. Keep total response time and TTFB distinct. k6's `http_req_duration` excludes initial connection establishment, while `http_req_waiting` measures waiting for the first byte; record connection settings with the results. See the [metric definitions](https://grafana.com/docs/k6/latest/using-k6/metrics/reference/).

On the server, record CPU consumption, memory over time and its peak, runtime count, queue depth, and time spent waiting for a render slot. Include all processes in the evaluated system. State the memory accounting method; summed process RSS can count shared pages more than once.

Use request-correlated instrumentation to examine rendering and data access. Nested spans overlap: do not add data-wait time to a render duration that already includes it, or add separate p95 values to estimate total p95. Run detailed profiling separately or control its overhead consistently across variants.

## Generate Load Without Hiding Saturation

For arrival-rate experiments, use an open model with a specified request schedule. With one request per iteration, k6's `constant-arrival-rate` can represent a fixed request rate. A closed model can reduce offered load as responses slow down; choose and report the model intentionally. See [open and closed models](https://grafana.com/docs/k6/latest/using-k6/scenarios/concepts/open-vs-closed/).

Run a pilot to find a useful range, then increase load in steps and refine the range around saturation. Compare latency at the same offered rates and find the highest rate meeting the declared criteria. Rejecting requests quickly must not inflate the reported useful capacity.

Track `dropped_iterations` as scheduled work that was not started. Verify generator CPU/network capacity and virtual-user allocation before attributing drops to the server. Preserve the target schedule and actual load in the report. See [dropped iterations](https://grafana.com/docs/k6/latest/using-k6/scenarios/concepts/dropped-iterations/).

## Run a Repeatable Protocol

1. Record commits, local patches, dependency locks, tool versions, build flags, hardware, and resource limits in the experiment manifest.
2. Build production Angular artifacts and release Rust binaries. Use a clean, identified test checkout when possible.
3. Restore the intended data and cache state. Record application caches separately from operating-system or database caches.
4. Warm the runtime until behavior stabilizes for steady-state tests, and exclude warm-up samples. Check that warm-up did not violate the intended cache scenario.
5. Run a measurement window and save raw latency, correctness, and resource data. Two to five minutes is an initial planning range, not a universal sufficient duration.
6. Repeat important comparisons at least five times initially, varying run order. Increase duration or repetition count when variation or sample size prevents a useful conclusion.
7. Preserve failed runs and document any exclusion rule. Record the analysis commands and figure-generation inputs.

Prefer a separate load-generator machine. When sharing a machine, partition resources and check that the generator does not become the bottleneck. Record power and thermal conditions where they affect repeatability; [Google Benchmark's variance guide](https://github.com/google/benchmark/blob/main/docs/reducing_variance.md) describes relevant CPU-frequency effects.

## Evaluate Startup and Sustained Operation Separately

For startup, report process launch to readiness and process launch to the first correct HTML response. Define readiness, including whether the full pool is initialized. Repeat fresh-process launches and state what remains warm in the OS or data layer. Label time to restore and use a snapshot separately where useful.

For steady-state stability, add a longer run, initially 15-30 minutes at a sustainable load. Observe memory growth and latency drift. An overload experiment should also measure recovery after the offered rate returns to a sustainable level.

Do not reuse warmed steady-state measurements as evidence for cold-start performance.

## Isolate Optimizations and Report Uncertainty

Compare a candidate with its reference while changing one factor where possible: runtime reuse, pool size, native data access, or snapshot initialization. Then evaluate combinations to identify interactions. HTML caching is a distinct scenario because it avoids rendering work.

Report per-run summaries, run-to-run variation, and the confidence-interval method if used. Treat independent runs as the replication units when estimating uncertainty between variants; requests from one run are not automatically independent replications. Explain percentile aggregation rather than simply averaging percentiles.

Include absolute measurements with relative differences. For latency, define reduction as `(reference - candidate) / reference`; for throughput, define gain as `(candidate - reference) / reference`. Keep the workload, offered rate or acceptance limits, and resource budget attached to each comparison.

Useful figures include p95 latency against offered load, sustainable correct-response rate, memory against pool size, and the effect of individual optimizations. Separate successes, failures, and unstarted work when presenting behavior near saturation.

## Preserve Evidence for the Thesis

Keep the experiment record, machine-readable raw results, reproduction manifest, and analysis scripts associated with the tested revision. Prefer relative links to versioned artifacts. Large artifacts may use stable archived URLs with checksums and retention details.

Record protocol deviations and inconclusive or unfavorable results. A backend replacement should be justified by correctness, resource constraints, and the measured workloads. Link that evidence from the corresponding ADR.

The external references in this guide were checked on 2026-09-17. Pin the actual tools used by each experiment and recheck version-sensitive behavior when they change.
