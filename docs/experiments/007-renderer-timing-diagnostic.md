# Experiment 007: Renderer Timing and Instrumentation Cost

**Status:** Replacement session interrupted by internal coverage limitations; continuation planned
**Created:** 2026-10-11
**Last updated:** 2026-10-11
**Type:** Exploratory local W-01 release diagnostic
**Application implementation:** `68bcbb8`
**Replacement source/harness revision:** `b7046a8`
**Frozen plan:** [configuration](../../benchmarks/diagnostics/config.json)
**Retained interrupted attempt:** [first-attempt record](../../benchmarks/results/2026-10-11-w01-diagnostics-001/README.md)

## Questions

How much worker wall time is spent waiting in the queue versus executing call_export? Does the bounded timing collector visibly change client results under the same workload and allocation? Which intervals still require finer measurements?

This follows [pilot 006](006-ssr-performance-pilot.md), which observed approximately 31–32 correct responses/s during overload despite low mean CPU. It evaluates the current implementation, not a general Rust or runtime-language speed claim.

## Design and Boundaries

Both executables use the same Rust application revision, dependencies, persistent single worker, 16 waiting slots, and unchanged production Angular 22.2.2 artifacts. The off binary disables render-timing; the on binary enables it. Both include the author's basic graceful HTTP shutdown. The original pilot binary was preserved before rebuilding. Release build and copied artifact hashes are recorded in the manifest/preflight.

The fixed plan offers 5, 20, and 50 requests/s for twenty seconds each, with two independent runs per variant/rate. Every run starts a fresh process, passes HTTP gates, warms at 20/s for ten seconds, and settles for two seconds. Rate order reverses in repetition two and variant order alternates. k6 2.3.0 uses the same correctness-checked client as the pilot: Home HTML, keep-alive, identity encoding, no redirects/TLS, 128 measurement VUs, two-second timeout, and three-second drain. No HTML cache is configured.

The complete server process, including its logger thread, is pinned to logical CPU 0. The generator uses logical CPUs 2 and 4 and GOMAXPROCS=2. Resource data is sampled every 500 ms. There is no memory cap, identified physical-core separation, controlled frequency/thermal state, or excluded background activity. The console helper/controller are outside server accounting. These are short exploratory windows, not sustained-capacity tests.

queue_us spans job creation to worker processing start. call_export_us spans namespace/argument preparation, JavaScript invocation, Promise/event-loop waits, adapter body buffering, and owned Rust conversion. Neither is CPU time. The event's ok field refers to the export result, not HTTP 200 or application content correctness.

Internal distributions select export-completion timestamps inside the client's declared measurement window. Client latency uses the full correct-request cohort, including its drain tail. Client completions and export completions are different populations without a per-request join. Their separate percentiles must not be added or subtracted as an exact latency breakdown.

## Correctness and Data Collection

The unchanged production application's prior browser interaction/hydration evidence is reused, with exact artifact hashes checked. Each run verifies Home, About, a query URL, HEAD, a missing-route 404, and post-load Home/About recovery. Load responses require the expected HTML headings, SSR/hydration annotations, and initial counter zero. This does not establish personalized-data isolation or arbitrary-application compatibility.

The [harness preflight](../../benchmarks/diagnostics/preflight.json) passed two smoke pairs and actual Windows Ctrl+C shutdown in a private hidden console. Membership checks allow a signal only when that console contains exactly the helper and its owned server. Normal exit, zero dropped lines, unique event IDs, and whole-session processed-event coverage are required for internal analysis. Expected events include control responses, warm-up responses, measured correct responses including the tail, and recovery; host admission 503 responses emit no export event.

The blocked-sink probe reconciled 65 emitted events as 3 retained and 62 dropped with a two-entry lossy queue. Four altered-log fixtures were rejected. Additional checks confirmed that a disabled-variant transport failure remains a measurable external failure, while a timeout in the enabled variant prevents exact event reconciliation. Compact smoke client/timing replay was byte-identical. Formatting/enqueueing overhead remains in the renderer; the writer moves sink I/O to another thread.

The first attempt at revision 55e292d stopped after eight of twelve planned runs. off-r2-50rps had one client timeout, and the analyzer incorrectly applied an instrumented-only outcome restriction to that disabled variant. All eight client summaries retain their original measurements, including that timeout. The four enabled runs had complete timing coverage and zero losses. The failed manifest and original false invalidation remain preserved; no remaining runs were filled into that attempt. After correcting and testing only the analysis restriction, the complete unchanged plan was restarted from clean revision b7046a8. No binary, application, client script, or offered-load parameter changed.

The replacement stopped after six runs when the enabled 50/s case produced two client timeouts. It recorded 785 events against 783 events explained by correct responses/control requests, with a final zero-loss counter. This is consistent with additional work completing after client failure, but there is no per-request join to prove exact coverage. Its internal summary remains invalid and the timeouts remain in client metrics. A continuation now distinguishes preserved internal limitations from analysis-process errors, retains the first six cases, and executes only the remaining six original cases. It records both revisions and verifies unchanged binaries, workload/configuration, application artifacts, and original plan order.

## Results

Replacement results and interpretation will be recorded after the full sequence and artifact verification. The interrupted attempt is retained as a separate dataset and is not a complete paired comparison.

## Next Investigation

Separate preparation, JavaScript/Promise settlement, adapter response.text(), Rust conversion, and HTTP handling with explicit interval boundaries. Include measurements before enqueueing and after the export returns because the coarse worker intervals do not cover the complete client response. Pair wall time with thread CPU evidence before attributing waits to Angular, Windows scheduling, or Deno integration. Changing worker count or runtime would be a separate controlled experiment.
