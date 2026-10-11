# W-01 Exploratory Performance Pilot

**Status:** Harness checked; full pilot measurements pending
**Created:** 2026-10-11
**Last updated:** 2026-10-11
**Scope:** Correctness-gated dynamic Home SSR, without external I/O

## Question and Limits

At what offered rates do the existing Rubicon and Node reference begin to show latency, failed responses, or unstarted work on this laptop? This pilot selects useful rates and follow-up questions. It is not a confirmatory estimate of sustainable capacity or an attribution of differences solely to Rust.

The application has no personalized data or native services. Previous browser checks and the pilot HTTP gates establish only the reference pages' behavior; request identity isolation, general cleanup, deadlines, and worker recovery remain incomplete. These are declared deviations from the full [benchmark protocol](../../docs/specs/benchmark-protocol.md), which still governs final comparisons.

## Fixed Configuration

[config.json](config.json) declares the rates, run order inputs, durations, resource masks, and exploratory limits before the pilot. Each variant receives logical CPU 0. k6 receives logical CPUs 2 and 4 and GOMAXPROCS=2. The masks do not establish physical-core separation. The controller, Windows, background applications, CPU frequency, and thermal conditions remain uncontrolled. No memory cap is enforced.

Each independent run starts a fresh server process, checks Home/About/query/HEAD/404, warms Home at 20 requests/s for 10 seconds, allows two seconds to settle, and offers a constant arrival rate for 20 seconds. The initial grid is 20, 50, 100, 200, and 400 requests/s. A two-second HTTP timeout and three-second drain interval preserve failure accounting. Two repetitions reverse rate order and alternate which variant runs first. These short windows and repetitions are pilot-specific deviations from the longer final protocol. The fixed warm-up is not a proof of convergence.

Exploratory acceptance requires p95 of correct complete-response durations at most 100 ms, failures among started requests at most 0.1%, no dropped arrivals, verified recovery, and generator mean CPU below 85% of its allocation. These are screening criteria for a small local page, not a validated application SLA. A one-request scheduling residual is tolerated. Correct throughput within the measurement window and completed cohort throughput are reported separately.

The generated Node Express/Angular handler is imported by [node-host.mjs](node-host.mjs) and bound to loopback. Its rendering and file middleware are preserved. Rubicon uses the current single persistent worker with 16 waiting slots. Requests fetch only Home HTML, with HTTP keep-alive, no redirects, Accept-Encoding identity, no TLS, and no HTML cache configured. Node's one-year static-file cache header is outside this HTML workload. The Node writer and Rubicon's buffered conversion are different implementation paths whose costs are included.

The installed Node v24.19.0 reports V8 13.6.233.17-node.51. A temporary diagnostic linked against the release V8 binding/native archive reports 15.0.245.2-rusty for Rubicon (binding 150.4.0). The runner checks the binding pin against Cargo.lock. This substantial engine-version difference prevents attributing observed performance solely to the hosting language. The manifest also checks that corresponding browser JavaScript, CSS, and favicon artifacts have identical hashes.

## Build and Tool Preparation

From examples/angular-demo:

```powershell
npm.cmd run build -- --configuration production
npm.cmd run build -- --configuration production,rubicon
```

Both builds must report zero prerendered routes. From the repository root:

```powershell
cargo build --release --locked --offline --jobs 4 --manifest-path crates/rubicon/Cargo.toml --bin rubicon
```

Cargo offline resolution does not prevent a dependency build script from downloading a native archive. The first release build may require the official rusty_v8 archive. Record build failures rather than silently measuring a debug binary.

k6 is pinned to 2.3.0. The official Windows archive is [k6-v2.3.0-windows-amd64.zip](https://github.com/grafana/k6/releases/download/v2.3.0/k6-v2.3.0-windows-amd64.zip); verify its SHA-256 against the [published checksums](https://github.com/grafana/k6/releases/download/v2.3.0/k6-v2.3.0-checksums.txt). The checked archive hash is `112276d495e5741c968e2bc09ea6196099c1275bd6db9ee0875d173c7148ce43`. Extract it under the ignored crates/rubicon/target/pilot-tools directory, or supply -K6Path. No system installation is required.

## Run and Replay

Use PowerShell 7 on Windows. The runner starts only its own processes and refuses occupied ports or an existing output directory. Affinity and process-resource inspection require the normal host permissions; the Codex restricted execution environment may need an approved local execution.

```powershell
./benchmarks/pilot/run.ps1 -Smoke -OutputDirectory crates/rubicon/target/pilot-smoke-001
```

The smoke check uses two-second warm-up and three-second measurement windows at 20 requests/s. It validates the measurement plumbing, not performance. Commit the checked harness before the full pilot; the full runner requires a clean checkout before it creates its result directory.

```powershell
./benchmarks/pilot/run.ps1 -OutputDirectory benchmarks/results/2026-10-11-w01-pilot-001
```

Browser checks should cover the exact production artifacts before interpreting the pilot. Keep browser automation and other intentional load outside the measurement windows.

## Harness Preflight

[preflight.json](preflight.json) records the checks before the full pilot. The first smoke configuration offered a 100 requests/s warm-up with only 16 virtual users and produced unstarted warm-up arrivals. Both runs were marked invalid and retained under the ignored target/pilot-smoke-001 directory. Before freezing the full configuration, warm-up was reduced to 20 requests/s and the pilot grid was narrowed to 20–400 requests/s. The replacement smoke had correct responses, no dropped arrivals, valid accounting, matching CPU masks, and successful recovery for both servers. Smoke latencies are not pilot results.

The negative integration check uses an independent local fixture with correct HTML, deliberately incorrect counter content, and HTTP 503. Actual k6 counters agreed with the fixture: 4 correct, 4 incorrect-content, and 3 HTTP 503 responses; the expected threshold exit was 99. Replay from compact smoke samples reproduced both summaries byte for byte.

```powershell
node benchmarks/pilot/check-accounting.mjs <k6.exe> <fresh-output-directory>
```

Before measurement, the production builds also passed browser counter interaction (0 → 1 → 2), About navigation and direct reload, and Home return at zero through both the Rust release executable and the Node reference. Captured warning/error lists were empty. This does not establish cross-request identity isolation.

## Evidence and Analysis

Each run preserves run.json, resources.csv, the k6 JSON summary, compact samples.csv.gz, and an independently calculated summary.json. The experiment root records its manifest, artifact hashes, source revision, plan, results.json, and results.csv. Full k6 JSON points and process logs remain in ignored raw directories, with raw SHA-256 recorded in each summary. Keep those directories for later investigation; the committed compact samples preserve all custom request metrics, dropped iterations, and connection/block timing points used by the analysis.

The custom checks require HTTP 200, HTML content type, the expected headings, SSR/hydration annotations, and counter zero. Started, completed, correct, HTTP 503, other HTTP failures, incorrect content, transport failures, unfinished work, and unstarted iterations remain distinct. The runner retains k6 threshold exit 99 as a measured failed screening criterion. Unexpected process failures abort the pilot and preserve a failed manifest.

Complete-response duration is k6 sending + waiting + receiving and excludes initial connection establishment. TTFB is k6 waiting; it is not time to visible content. Connection and blocking samples are preserved separately. Latency percentiles use explicit Type-7 interpolation, computed per independent run. They are never averaged as if the request samples were independent experiments.

Resource samples are taken every 500 ms. Working set and private bytes describe one complete server process; reported peaks are sampled peaks within the window. CPU is normalized to the declared logical-processor allocation. CPU cost per correct response uses the same sampled interval for CPU and successful completions. These observations are not a long-run memory-leak or stability check.

Replay a run without the large raw stream:

```powershell
node benchmarks/pilot/analyze.mjs --replay <run-directory>
node benchmarks/pilot/analyze.mjs --all <experiment-directory>
```

The standalone [plot.py](plot.py) uses matplotlib 3.10.8 and plots each repetition separately. Install it in a local environment, then run `python benchmarks/pilot/plot.py <experiment-directory>`. The PNG and SVG are derived artifacts; compact samples and per-run summaries remain the numerical evidence.

## Primary References

- [k6 constant-arrival-rate](https://grafana.com/docs/k6/latest/using-k6/scenarios/executors/constant-arrival-rate/), checked for 2.3.0 on 2026-10-11.
- [Dropped iterations](https://grafana.com/docs/k6/latest/using-k6/scenarios/concepts/dropped-iterations/).
- [HTTP metric definitions](https://grafana.com/docs/k6/latest/using-k6/metrics/reference/).
- [Granular JSON and gzip output](https://grafana.com/docs/k6/latest/results-output/real-time/json/).
- [Custom end-of-test summary](https://grafana.com/docs/k6/latest/results-output/end-of-test/custom-summary/).
