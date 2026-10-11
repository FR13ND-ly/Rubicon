# Runtime Stage Diagnostic

**Status:** Eight release cases completed; seven valid cases and one failed warm-up retained
**Created:** 2026-10-11
**Application source:** fc3f69c
**Protocol:** [stages-config.json](stages-config.json)

Locate preparation, JavaScript execution/Promise waits, and owned Rust conversion on the real production Angular demo. Record the initial render separately. The eight-case plan compares feature-off and feature-on release binaries at 5 and 20 requests/s, with two independent repetitions. Warm-up is 10 seconds at 20/s with a 16-VU cap, followed by two seconds to settle and 20 seconds of measurement with 128 VUs. Each case starts a fresh process. These passing-load candidates do not establish maximum throughput or overload behavior.

## Inputs and First Render

Retained binaries are rubicon-stage-fc3f69c-off.exe and rubicon-stage-fc3f69c-on.exe under ignored target/diagnostics-artifacts. They were built using the locked offline release commands, with the latter enabling render-timing. The stage build manifest records the application revision, release profile, commands, compiler versions, source hashes, and both binary hashes. The runner refuses changed build inputs before creating its output directory or launching a server. Reproducing the build requires fresh retained binary names and corresponding verified build metadata; do not overwrite experiment 007 artifacts.

On a fresh reproduction checkout, use the source revision declared by the configuration. These commands refuse existing destinations, build both variants, and record their hashes:

```powershell
$stageConfig = Get-Content benchmarks/diagnostics/stages-config.json -Raw | ConvertFrom-Json
foreach ($stageVariant in @('off', 'on')) {
    if (Test-Path -LiteralPath $stageConfig.binary_paths.$stageVariant) { throw 'Retained binary already exists.' }
}
cargo build --release --locked --offline --jobs 4 --manifest-path crates/rubicon/Cargo.toml --bin rubicon
if ($LASTEXITCODE -ne 0) { throw 'Feature-off build failed.' }
Copy-Item -LiteralPath crates/rubicon/target/release/rubicon.exe -Destination $stageConfig.binary_paths.off
cargo build --release --locked --offline --jobs 4 --manifest-path crates/rubicon/Cargo.toml --bin rubicon --features render-timing
if ($LASTEXITCODE -ne 0) { throw 'Feature-on build failed.' }
Copy-Item -LiteralPath crates/rubicon/target/release/rubicon.exe -Destination $stageConfig.binary_paths.on
python benchmarks/diagnostics/record-stage-build.py --config benchmarks/diagnostics/stages-config.json
```

The runner waits for the flushed ready banner without sending HTTP requests. It then measures and correctness-checks exactly one initial Home request; startup failure does not silently become a warm-up retry. The first control interval contains exactly one expected export with ID 1, and all its stages must lie inside that interval. The association relies on controlled ordering and the server's initial counter, rather than a returned request-ID header.

Server first-export time excludes process/module startup. OS/filesystem caches are not reset, so a fresh process does not establish a machine/cache cold start. The separately retained .NET first-control wall time includes connection, client construction, receiving and decoding; client initialization varies with run order. It is a different measurement from k6's response duration and should not be used to infer a logging cost. The ordinary Home/About/query/HEAD/404 gates, production artifact checks, warm-up, load, and recovery gates remain in use. Existing browser evidence is reused without claiming a new isolation test.

## Analysis and Coverage

[analyze-stages.py](analyze-stages.py) runs after the unchanged coarse analyzer and writes stage-summary.json plus deterministic runtime-events.csv.gz. The compact stream retains parsed stage and export events in their original order. Parser failures and input hashes remain in the summary so invalid cases also replay consistently. Explicit runtime_stage_schema=1 is required; a legacy result directory is refused before stage files are written.

Every completed successful export must have exactly prepare/execute/convert in that order, all true. Handled failures have one, two, or three reached stages ending in false. Unknown, duplicate, missing, orphaned, reversed, late, or wrongly associated stages are rejected. Each sum must fit its enclosing call, and stage timestamps must precede export completion. Events after the final logger-drop counter are rejected. Feature-off cases require no stage/export events. Normal private-console Ctrl+C shutdown, zero writer drops, and the original whole-session processed-event reconciliation remain required.

Stages are joined by server request ID before selecting the export-completion window. All phase distributions use that same server cohort; its exported requests are not directly joined to the k6 latency population. Do not sum phase percentiles or interpret execution/wait wall time as isolate-thread CPU. The enclosing residual contains logging and other work and is not pure logging CPU cost. Invalid summaries retain their observations and reasons; they cannot establish a complete population.

[check-stages.py](check-stages.py) preserves adversarial fixtures, including exchanged request IDs that leave global counts unchanged, missing phases, late events, wrong outcomes, compact corruption, and legacy-schema refusal. It also accepts the four legitimate success/handled-error sequences. Positive and negative compact replay must reproduce the summaries byte for byte. Changed replay inputs are refused before overwriting the preserved summary. [stage-preflight.json](stage-preflight.json) records four real smoke runs, 12 byte-identical summary replays, 22 stage/sequence checks, six unchanged coarse-analysis checks, and a build-hash refusal before launch. Smoke latency and threshold outcomes are retained as collection checks and are not performance results.

## Run and Replay

```powershell
./benchmarks/diagnostics/run.ps1 -RuntimeStages -ConfigPath benchmarks/diagnostics/stages-config.json -PythonPath <python.exe> -Smoke -OutputDirectory <fresh-repository-smoke-directory>
./benchmarks/diagnostics/run.ps1 -RuntimeStages -ConfigPath benchmarks/diagnostics/stages-config.json -PythonPath <python.exe> -OutputDirectory <fresh-repository-result-directory>
python benchmarks/diagnostics/analyze-stages.py --replay <run-directory>
python benchmarks/diagnostics/analyze-stages.py --all <experiment-directory>
```

The full run requires a clean committed checkout. Fresh output paths are mandatory. Stage attempts are not resumed or repeated to replace unfavorable cases; preserve any interrupted attempt. [plot-stages.py](plot-stages.py) generates PNG/SVG figures from the committed summaries, keeping repetitions separate and including only verified stage populations. The full plan does not include 50/s: overload and a direct client/server ID join remain separate investigations.

## Recorded Release Experiment

[Experiment 008](../../docs/experiments/008-runtime-stage-diagnostic.md) and its [retained dataset](../results/2026-10-11-w01-runtime-stages-001/README.md) contain all eight planned cases. Three enabled populations are verified; on-r2-5rps remains invalid because warm-up had a timeout and 22 dropped arrivals. Its observed stage joins are structurally sound, but exact request-population reconciliation is unsupported. A separate valid disabled 20/s case exceeds the exploratory p95 limit. Neither observation was replaced.

The final plot excludes the invalid internal population and marks its client p95 with a gray X. The collection manifest preserves the original plotting source hash; final drawing source and PNG/SVG hashes are recorded separately in verification.json. All 24 compact summary replays preserve the classifications byte for byte. Follow-up CPU, JavaScript-handler/body-buffering, warm-up, and HTTP investigations remain separate steps.
