"""Standalone scientific figure; preserve independent runs and shared stage cohorts."""

import json
import os
from pathlib import Path
import sys

os.environ.setdefault("MPLCONFIGDIR", str(Path(__file__).resolve().parents[2] / "crates/rubicon/target/matplotlib-cache"))
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt

root = Path(sys.argv[1]).resolve()
runs = json.loads((root / "stage-results.json").read_text(encoding="utf-8"))["runs"]
clients = json.loads((root / "diagnostics-results.json").read_text(encoding="utf-8"))["runs"]
enabled = sorted((run for run in runs if run["variant"] == "on" and run["valid"]), key=lambda run: (run["rate"], run["repetition"]))
labels = [f"{run['rate']}/s · r{run['repetition']}" for run in enabled]
plt.rcParams.update({"font.family": "DejaVu Sans", "font.size": 10,
                     "axes.spines.top": False, "axes.spines.right": False,
                     "svg.hashsalt": "rubicon-runtime-stages-v1"})
fig, axes = plt.subplots(1, 3, figsize=(14, 5))
positions = list(range(len(enabled)))
for offset, color, label, values in (
    (-.18, "#c65b19", "First server export", [run["first_render"]["call_export_ms"] for run in enabled]),
    (.18, "#047857", "Warmed exports: mean", [next(record["internal"]["window"]["call_export_ms"]["mean"] for record in clients if record["id"] == run["id"]) for run in enabled]),
):
    axes[0].bar([position + offset for position in positions], values, width=.32, color=color, label=label)
axes[0].set_title("Initial and warmed server exports", loc="left", fontweight="bold")
axes[0].set_ylabel("call_export wall time (ms)")
axes[0].set_ylim(bottom=0)
axes[0].legend(fontsize=8)
for metric, color, marker, label in (
    ("prepare_ms", "#0e7490", "o", "Preparation"),
    ("execute_ms", "#c65b19", "s", "Execution and waits"),
    ("convert_ms", "#7c3aed", "^", "Rust conversion"),
    ("other_call_ms", "#64748b", "D", "Other enclosing work"),
):
    axes[1].plot(positions, [run["window"][metric]["mean"] * 1000 for run in enabled],
                 color=color, marker=marker, linewidth=1, markersize=5, label=label)
axes[1].set_title("Warmed exports: phase means", loc="left", fontweight="bold")
axes[1].set_ylabel("Wall time (µs, log scale)")
axes[1].set_yscale("log")
axes[1].legend(fontsize=8, loc="center right")
for axis in axes[:2]:
    axis.set_xticks(positions, labels, rotation=25, ha="right")
    axis.set_xlabel("Each mark is an independent run")
for variant, color in (("off", "#1d4e89"), ("on", "#c65b19")):
    for repetition, marker in ((1, "o"), (2, "s")):
        group = sorted((record for record in clients if record["variant"] == variant and record["repetition"] == repetition), key=lambda record: record["rate"])
        axes[2].plot([record["rate"] for record in group], [record["client"]["latency_correct_ms"]["p95"] for record in group],
                     color=color, marker=marker, linestyle="-" if repetition == 1 else "--", label=f"Timing {variant} · r{repetition}")
axes[2].set_title("Correct client responses: p95", loc="left", fontweight="bold")
axes[2].set_ylabel("Complete response (ms)")
axes[2].set_xlabel("Offered requests/s")
axes[2].set_xticks([5, 20])
axes[2].set_ylim(0, max(110, 1.2 * max(record["client"]["latency_correct_ms"]["p95"] for record in clients)))
axes[2].axhline(100, linewidth=1, linestyle=":", color="#9ca3af")
axes[2].legend(fontsize=8)
for axis in axes:
    axis.grid(True, axis="y", alpha=.18)
fig.suptitle("Rubicon: first render and runtime stages", x=.06, y=.985, ha="left", fontsize=19, fontweight="bold")
fig.text(.06, .91, "Release · one logical CPU for the server · 2 independent repetitions · 20-second warmed measurement windows", color="#4b5563", fontsize=10)
fig.text(.06, .025, f"Stage panels include {len(enabled)}/4 verified enabled runs. Stage means share the same server completion cohort.\nExecution includes asynchronous waits; the enclosing residual includes logging/bookkeeping. Client latency has a separate population.\nFirst export excludes module/process startup. Local laptop without memory cap; this does not establish maximum throughput or precise logging overhead.", fontsize=9, color="#4b5563")
fig.tight_layout(rect=(.015, .14, .995, .88), w_pad=2)
fig.savefig(root / "runtime-stages.png", dpi=180, facecolor="white")
fig.savefig(root / "runtime-stages.svg", facecolor="white", metadata={"Date": None})
plt.close(fig)
print(root / "runtime-stages.png")
