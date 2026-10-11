# Renderer Timing Diagnostic

**Status:** Harness preflight passed; [experiment 007](../../docs/experiments/007-renderer-timing-diagnostic.md) completed with recorded internal coverage limitations
**Created:** 2026-10-11
**Application source:** `68bcbb8`

Compare release binaries with render-timing disabled and enabled, using the same production Angular artifacts and the pilot's CPU allocation. Locate queue versus call_export wall time before changing the renderer. This is an exploratory follow-up to [pilot 006](../../docs/experiments/006-ssr-performance-pilot.md), not the full thesis protocol.

## Frozen Plan

[config.json](config.json) selects 5, 20, and 50 requests/s, two independent repetitions, ten seconds of warm-up at 20/s, two seconds to settle, and twenty seconds of measurement. Each run starts a fresh server. Rate order reverses in the second repetition, and variant order alternates. The server receives logical CPU 0; k6 receives logical CPUs 2 and 4. Logging runs inside the server's allocation. Memory is observed, without a cap. The laptop's power, thermal state, and other activity remain uncontrolled.

The feature-disabled executable measures complete client responses without timing events. The feature-enabled executable adds the existing structured events and a 1,024-entry lossy writer queue. Formatting/enqueueing cost remains on the renderer thread. Two repetitions permit descriptive paired differences, not a precise or statistically established instrumentation-cost estimate.

## Build and Run

Retain the original pilot executable before replacing target/release/rubicon.exe. Build both variants from the same application revision, then copy each artifact into the ignored diagnostics-artifacts directory. Refuse to overwrite retained experiment binaries.

```powershell
cargo build --release --locked --offline --jobs 4 --manifest-path crates/rubicon/Cargo.toml --bin rubicon
Copy-Item -LiteralPath crates/rubicon/target/release/rubicon.exe -Destination crates/rubicon/target/diagnostics-artifacts/rubicon-off.exe
cargo build --release --locked --offline --jobs 4 --manifest-path crates/rubicon/Cargo.toml --bin rubicon --features render-timing
Copy-Item -LiteralPath crates/rubicon/target/release/rubicon.exe -Destination crates/rubicon/target/diagnostics-artifacts/rubicon-on.exe
```

Use PowerShell 7 and a local Python 3 interpreter. The runner uses the same pinned k6 2.3.0 executable and correctness-checked client/analysis code as the pilot. Supply a Python executable path; no additional Python packages are needed.

```powershell
./benchmarks/diagnostics/run.ps1 -PythonPath <python.exe> -Smoke -OutputDirectory crates/rubicon/target/diagnostics-smoke-001
./benchmarks/diagnostics/run.ps1 -PythonPath <python.exe> -OutputDirectory benchmarks/results/2026-10-11-w01-diagnostics-001
./benchmarks/diagnostics/run.ps1 -PythonPath <python.exe> -Resume -OutputDirectory <interrupted-experiment-directory>
```

The smoke check only validates plumbing. Commit the checked harness/configuration before the complete experiment; the full runner requires a clean checkout. Existing output directories and occupied server ports are refused. The production application/browser artifacts are reused from pilot 006 and their exact hashes are recorded. No new identity-isolation claim is made.

## Normal Shutdown and Coverage

[console-host.py](console-host.py) creates a private hidden Windows console and starts only its own Rust subprocess there. Before sending CTRL_C_EVENT, it verifies that the console contains exactly the helper and its server. It never attaches to an existing console. The Rust process handles Ctrl+C through its existing shutdown code; exit status, both membership checks, signal type, and forced-stop state are retained. A 15-second cleanup limit protects the harness: fallback termination invalidates the diagnostic run, rather than proving a production shutdown deadline.

The controlled console avoids broadcasting Ctrl+C into the user's terminal or the Codex process tree. Windows cannot restrict CTRL_C_EVENT to a nonzero process group, so checking private-console membership is required. [Microsoft signal semantics](https://learn.microsoft.com/en-us/windows/console/generateconsolectrlevent), [console membership](https://learn.microsoft.com/en-us/windows/console/getconsoleprocesslist), and [console creation](https://learn.microsoft.com/en-us/windows/console/creation-of-a-console) define this helper's behavior.

Feature-enabled runs require one final RUBICON_DIAGNOSTICS_DROPPED lines=0 record, unique request IDs, and exact whole-session event coverage. Expected processed events equal successful control HTTP responses plus correct warm-up and measured responses. The fixed application only returns 503 through host admission failure; those requests have no export-completion events. Unexpected content, other statuses, or transport errors make this reconciliation unsupported and invalidate internal analysis. This internal requirement applies only to the enabled variant; client failures in the disabled variant remain measurable failures. Whole-session counts include readiness, HTTP gates, warm-up, measured completions after the window, and recovery.

Internal distributions select event timestamps inside the client's declared measurement window. Events mark export completion before response conversion/delivery; client samples mark response receipt. These are separate completion-window populations without a per-request join. Never add their percentile values or call call_export_us pure Angular CPU time.

## Preflight and Replay

The standalone [overflow probe](overflow-probe.rs) blocks the sink and uses a two-entry queue. A fixed burst must return before releasing the sink, count 62 dropped records, then flush three retained records: 65 emitted = 3 retained + 62 dropped. This validates lossy behavior, not a performance bound.

[preflight.json](preflight.json) records two successful smoke pairs, exact event coverage, graceful exits, zero lost log records, and byte-identical client/timing replay. Four deliberately altered logs were rejected: missing event, duplicate event, nonzero drop counter, and missing final counter. Smoke latencies are not performance results.

The first full attempt stopped after eight runs because the analyzer applied the instrumented outcome restriction to the disabled variant, which had one client timeout. Its original data and failed manifest are retained. The restriction was corrected and fixture checks now require a disabled-variant timeout to remain a valid external observation, while an enabled-variant timeout prevents exact internal reconciliation. The load plan, binaries, client script, and application remain the same for the replacement attempt.

The replacement attempt then encountered five real client timeouts in the enabled 50/s run, with two more timing events than could be explained by correct/control responses. Its internal summary remains invalid: zero counted writer losses alone does not establish exact coverage. The runner now distinguishes this preserved limitation (analysis exit 2) from a broken analysis process (exit 1), allowing later planned cases to run. Resume is allowed once: it preserves the interrupted manifest, checks all immutable input hashes and the complete original run order, records the continuation revision, and skips only completed cases with both summaries present. Completed runs are never repeated to replace an unfavorable outcome.

```powershell
cargo run --locked --offline --manifest-path benchmarks/diagnostics/probe/Cargo.toml
python benchmarks/diagnostics/check-analysis.py <instrumented-smoke-run> <fresh-negative-check-directory>
node benchmarks/pilot/analyze.mjs --replay <run-directory>
python benchmarks/diagnostics/analyze.py --replay <run-directory>
```

Each run preserves compact client samples, resources, compact timing events, both summaries, run metadata, and raw-log hashes. Raw streams/logs and executable files remain local under ignored directories. Replaying from committed compact samples must reproduce summaries. Retain failed or invalidated checks with their reasons.

The completed [replacement sequence](../results/2026-10-11-w01-diagnostics-002/README.md) retains all 12 cases: 8 pass the exploratory limits and 4 overloaded cases fail. Two enabled overload cases have unsupported exact internal coverage because of client timeouts, despite zero writer drops. [plot.py](plot.py) plots all client runs and only verified internal populations using matplotlib 3.10.8. The interrupted first attempt remains separate.
