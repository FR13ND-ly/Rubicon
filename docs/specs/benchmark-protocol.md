# Rubicon Benchmark Protocol

**Status:** Draft
**Created:** 2026-10-09
**Last updated:** 2026-10-09
**Implementation:** Not started

## Purpose and Context

This protocol defines a reproducible performance comparison between Angular SSR on Node.js and Rubicon. It applies to implementations that first satisfy the required correctness checks.

The primary objective is the highest sustained rate of correct responses that meets declared latency, failure, and memory limits within a fixed resource budget. Secondary metrics include CPU cost, startup time, queue behavior, and sustained stability.

No performance results are established by this document. Exact workload parameters, versions, hardware, thresholds, and durations must be frozen in the experiment manifest before a comparison is interpreted.

## Scope

The initial evaluation covers dynamic SSR, buffered response delivery, asynchronous data access, startup, and sustained operation. Cache hits, prerendered files, optional Bun variants, and later optimizations are distinct scenarios.

Browser hydration is a correctness gate. Frontend computation, animation frame rate, WASM speed, and the Angular build pipeline are outside the initial server benchmark.

## Requirements

| ID | Requirement | Acceptance evidence |
| --- | --- | --- |
| BP-01 | Every evaluated variant must pass content, HTTP, hydration, and isolation checks. | A recorded correctness gate for the tested revision. |
| BP-02 | Comparisons must use equivalent application behavior and explicit CPU and memory budgets. | Configuration and resource manifest. |
| BP-03 | Scheduled load, started work, successful work, incorrect responses, and failures must be accounted for. | Raw client counters and latency records. |
| BP-04 | Startup and warmed steady-state measurements must remain distinct. | Separate run types and reported readiness definitions. |
| BP-05 | Important comparisons must use independent repeated runs with recorded run order. | Repetition records and uncertainty analysis. |
| BP-06 | Each result must be traceable to code, dependencies, data, configuration, and analysis inputs. | Reproduction manifest and artifacts. |
| BP-07 | Optimization claims must state which factors and amount of work changed. | Variant definitions and controlled comparisons. |
| BP-08 | Invalidated, inconclusive, and unfavorable runs must be retained with explanations. | Experiment history and exclusion rules. |

## Design and Request Flow

### Variants and Controls

| Variant | Role | Required configuration |
| --- | --- | --- |
| Angular SSR on Node.js | Reference implementation. | Persistent production server and explicit SSR routes. |
| Minimal Rubicon | First correct candidate. | Persistent workers, the same Angular application, and measured host settings. |
| Optimized Rubicon | Evaluate a named change. | A versioned delta from the minimal candidate. |
| Optional alternative renderer | Compare a complete alternative deployment. | Its processes, transport, resource use, and integration differences recorded. |

Routes must be configured for server rendering so the initial comparison cannot accidentally serve prerendered HTML. Match application behavior, data, cache policy, compression, TLS configuration, connection reuse, logging, and browser artifacts as far as the comparison requires.

Use both a single-core budget and an identified multicore budget when practical. Each variant may use the allocated resources appropriately. A multicore Rust pool must not be compared with a single Node worker as evidence of an architectural speedup.

For a comparison isolating the host, use equivalent data-access behavior. A variant replacing HTTP data access with direct native calls measures an additional system change and must identify it. Engine-version differences and unavoidable configuration differences must also be reported.

### Workloads

| ID | Workload | Controlled parameters |
| --- | --- | --- |
| W-01 | Small SSR page without external I/O. | Fixed route, components, bindings, and fixture. |
| W-02 | More complex SSR page. | Fixed component tree, conditional content, and data. |
| W-03 | Large HTML response. | Versioned row counts, fixture sizes, and output-size expectations. |
| W-04 | Controlled asynchronous data. | Deterministic service behavior, delays, failures, and request sequence. |
| W-05 | Real database integration, if included. | Dataset, indexes, query, connection limits, and database version. |
| W-06 | Cache or prerender serving, if implemented. | Keys, request distribution, cache state, and observed hit/miss rates. |

Exact fixture sizes and parameter sets are TBD. They must be recorded before measurement. Include multiple routes or request identities where needed to detect state leakage.

### Load Generation and Run Sequence

The proposed load generator is k6 using an open arrival-rate model. With one HTTP request per iteration, constant-arrival-rate represents an intended request schedule independently of response completion. A closed model may be used for a separately stated concurrency question.

Run a pilot to identify a useful offered-load range, then measure several rates and refine the region near saturation. Record virtual-user allocation, generator CPU and network use, and dropped iterations. A dropped iteration is scheduled work that was not started; its cause must be investigated before attributing it to the server.

The planned measurement sequence is:

1. Identify the tested revisions and produce production Angular artifacts and release Rust binaries.
2. Pass correctness checks and restore the declared fixture and cache state.
3. Apply resource limits and start the variant with its recorded configuration.
4. Warm it until behavior stabilizes, excluding warm-up samples from measurement.
5. Apply the scheduled load and collect client and server evidence.
6. Repeat important comparisons at least five times initially, varying run order.
7. Run separate startup, sustained-operation, and overload-recovery experiments.

Two to five minutes per steady-state measurement is an initial planning range. A 15-30 minute sustained run is an initial stability check. Exact durations and additional repetitions must be justified by pilot variability and the question being evaluated.

## Contracts and Data

Each experiment manifest must record:

- Experiment identity, hypothesis, protocol revision, and run identifiers.
- Repository commits, local patches, dependency locks, and tool/runtime versions.
- Build commands, flags, artifact identity, and application configuration.
- CPU model, core allocation, RAM, operating system, and power/thermal conditions where relevant.
- Server processes, workers, queues, deadlines, service pools, and memory limits.
- Load-generator placement, connection settings, schedule, fixture seed, and check definitions.
- Cache and database state, including what remains warm between runs.
- Exact thresholds, run durations, repetition count, and exclusion rules.
- Analysis procedure and links or checksums for preserved artifacts.

Future experiment records and machine-readable artifacts should live under documented experiment and benchmark locations. Their exact paths remain TBD; this draft does not imply that a harness or experiment record already exists.

| Metric | Definition or reporting rule |
| --- | --- |
| Offered load | Intended request schedule. |
| Started load | Requests actually issued by the generator. |
| Correct throughput | Complete responses passing the workload checks per measured second. |
| Latency distribution | p50, p95, and p99, with failed requests and censoring behavior reported. |
| TTFB | Time to the first response byte at the client; it must not be labeled time to visible content. |
| Complete response time | Time to receive the entire body, with connection timing defined separately. |
| Failure accounting | Incorrect content, HTTP failures, deadlines, rejections, cancelled or incomplete work. |
| Server CPU | Utilization and CPU time, optionally normalized per correct response. |
| Server memory | Peak and time series, with accounting method and all evaluated processes included. |
| Queue behavior | Waiting time, depth, admission, rejection, and active-worker counts. |
| Startup | Process launch to declared readiness and to the first correct response. |

k6's http_req_duration covers sending, waiting, and receiving rather than initial connection establishment. Report connection settings and additional timing where relevant. If headers can arrive before body content, measure first body content separately when discussing user-visible improvements.

## Lifecycle, Concurrency, and Isolation

For each independent steady-state run, restart the evaluated server and apply the same warm-up procedure. Native service and database state must follow the manifest, rather than being silently reset or reused.

Separate server, generator, and data-service resource use. Prefer a separate generator machine; otherwise allocate resources and verify that the generator does not become the bottleneck.

Startup runs launch fresh processes. They must distinguish process-cold initialization from operating-system, file, database, or artifact caches that remain warm. Full readiness must state whether the entire worker pool is initialized.

Request identity checks must cover repeated renders on reused workers. Detailed profiling should run separately or have controlled overhead in every compared variant.

## Failure and Resource Limits

A fast rejection or an HTTP 200 response with incomplete content is not useful serving capacity. Report unsuccessful and unstarted work alongside completed requests. Do not discard timeout behavior to make latency percentiles look better.

Overload testing must record queue growth, rejection behavior, memory, latency, and recovery after the offered load returns to a sustainable level. It must respect declared host and generator limits.

A run may be invalidated for a documented issue such as generator saturation, incorrect artifacts, broken content checks, or protocol deviation. Preserve the run and reason; do not silently remove unfavorable results.

## Performance Expectations

**Not measured.**

Latency, failure-rate, and memory acceptance thresholds are TBD and must be chosen before confirmatory comparisons. They may differ by workload when justified. A candidate's useful capacity is the highest sustained correct-response rate meeting all applicable thresholds.

Evaluate named changes individually where practical, then examine combinations. Initial factors include worker count and native data access. Snapshots address startup; cache experiments bypass rendering. Recreating an engine for every request is a diagnostic configuration, not the conventional persistent Node baseline.

Report absolute measurements and relative differences. Define latency reduction as `(reference - candidate) / reference` and throughput gain as `(candidate - reference) / reference`, attaching the workload and budget.

Use independent runs as the replication units for comparison uncertainty. Report run-to-run variation and the method used for confidence intervals. Do not treat every request in one run as an independent experiment or average percentiles without defining the aggregation.

Request-correlated spans may overlap. Do not add nested timing spans or separate p95 values to infer total latency.

## Verification

| Requirement | Verification method | Planned evidence | State |
| --- | --- | --- | --- |
| BP-01 | Inspect content, HTTP semantics, hydration, and identity isolation. | Correctness gate. | Planned |
| BP-02 | Compare manifests and enforced limits. | Configuration review. | Planned |
| BP-03 | Reconcile scheduled, started, completed, failed, and unstarted work. | Client and server counters. | Planned |
| BP-04 | Review startup and steady-state run definitions. | Separate experiment records. | Planned |
| BP-05 | Review repetition, order, and statistical interpretation. | Run summaries and analysis. | Planned |
| BP-06 | Reproduce a representative run from preserved artifacts. | Reproduction record. | Planned |
| BP-07 | Inspect variant differences and changed workload effort. | Optimization comparison. | Planned |
| BP-08 | Review run retention and invalidation history. | Experiment audit. | Planned |

## Open Questions

| Question | Impact | Next action |
| --- | --- | --- |
| Which hardware, operating system, and resource budgets will be used? | Determines scope of conclusions. | Select an available environment and freeze its manifest. |
| What fixture sizes and offered-load range are appropriate? | Determines sensitivity and saturation behavior. | Run a pilot after correctness passes. |
| What acceptance thresholds reflect the selected application? | Determines useful capacity. | Justify thresholds before confirmatory runs. |
| Which timing and memory collection tools are available? | Determines metric precision and overhead. | Validate instrumentation on the chosen platform. |
| Are native data access and the host evaluated separately? | Determines attribution of any gain. | Define matched-access and full-system variants. |
| How long and how often must runs repeat? | Determines uncertainty and stability evidence. | Use pilot variation to fix exact settings. |

## Related Decisions and Sources

- [Project overview](project-overview.md).
- [Architecture](architecture.md).
- [Rendering contract](rendering-contract.md).
- [Performance evaluation guide](../guides/performance-evaluation.md).
- [k6 open and closed models](https://grafana.com/docs/k6/latest/using-k6/scenarios/concepts/open-vs-closed/).
- [k6 constant-arrival-rate](https://grafana.com/docs/k6/latest/using-k6/scenarios/executors/constant-arrival-rate/).
- [k6 dropped iterations](https://grafana.com/docs/k6/latest/using-k6/scenarios/concepts/dropped-iterations/).
- [k6 built-in metrics](https://grafana.com/docs/k6/latest/using-k6/metrics/reference/).
- [k6 thresholds](https://grafana.com/docs/k6/latest/using-k6/thresholds/).

The k6 sources are rolling documentation checked on 2026-10-09. The actual tool version and instrumentation remain TBD and must be pinned in the experiment manifest.
