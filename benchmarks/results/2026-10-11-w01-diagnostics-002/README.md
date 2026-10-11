# Completed Renderer Diagnostic Sequence

**Status:** All 12 planned cases completed; two internal timing coverage limitations retained
**Date:** 2026-10-11
**Application:** `68bcbb8`
**Initial session:** `b7046a8`
**Continuation:** `fb7c762`

See [experiment 007](../../../docs/experiments/007-renderer-timing-diagnostic.md) for definitions, the retained interrupted first attempt, results, and interpretation. Client accounting is valid in all twelve cases. Eight cases at 5/s and 20/s pass the exploratory limits. Four cases at 50/s fail with HTTP 503 and client timeouts. This is not a sustained-capacity benchmark.

The initial session stopped after six cases because a strict internal coverage check encountered client timeouts. The continuation preserved those cases and completed only the remaining six in the original order. All 42 existing per-run artifact hashes were checked unchanged. The initial failed manifest remains in interrupted-manifest.json; the final manifest retains its historical error and records continuation metadata. Completed means that the original sequence finished, not that every internal population is verified.

Both enabled 50/s runs retain valid=false in diagnostics-summary.json. Final dropped-line counters are zero, but timeouts prevent exact processed-event reconciliation. Their client data and raw timing samples remain preserved. The figure only plots verified internal windows; all client outcomes remain plotted and counted.

verification.json records 60 checked artifact hashes, 24 byte-identical compact replays, normal shutdowns, generator headroom, recovery, and the complete failure categories. Per-run metadata retains its actual session revision. Raw logs and full k6 streams remain local in ignored raw directories; compact samples, timings, resource data, summaries, and hashes are versioned.
