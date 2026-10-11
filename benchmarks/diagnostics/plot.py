"""Standalone figure; each mark is an independent run, without pooled p95."""

import json
import os
from pathlib import Path
import sys

os.environ.setdefault("MPLCONFIGDIR", str(Path(__file__).resolve().parents[2] / "crates/rubicon/target/matplotlib-cache"))
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.lines import Line2D

root = Path(sys.argv[1]).resolve()
data = json.loads((root / "diagnostics-results.json").read_text(encoding="utf-8"))["runs"]
colors = {"off": "#1d4e89", "on": "#c65b19"}
markers = {1: "o", 2: "s"}
plt.rcParams.update({"font.family": "DejaVu Sans", "font.size": 10,
                     "axes.spines.top": False, "axes.spines.right": False})
fig, axes = plt.subplots(2, 3, figsize=(13.5, 8))
axes = axes.ravel()
for variant in ("off", "on"):
    for repetition in (1, 2):
        runs = sorted((r for r in data if r["variant"] == variant and r["repetition"] == repetition), key=lambda r: r["rate"])
        rates = [r["rate"] for r in runs]
        style = dict(color=colors[variant], marker=markers[repetition], linestyle="-" if repetition == 1 else "--", linewidth=1.3, markersize=5)
        axes[0].plot(rates, [r["client"]["latency_correct_ms"]["p95"] for r in runs], **style)
        axes[3].plot(rates, [r["client"]["correct_rps_in_window"] for r in runs], **style)
        axes[4].plot(rates, [100 * r["client"]["failure_fraction_of_started"] for r in runs], **style)
        axes[5].plot(rates, [r["client"]["server"]["allocated_cpu_mean_percent"] for r in runs], **style)
        if variant == "on":
            for percentile, color in (("p50", "#047857"), ("p95", "#7e3a94")):
                internal_style = dict(style, color=color)
                for index, metric in ((1, "queue_ms"), (2, "call_export_ms")):
                    axes[index].plot(rates, [r["internal"]["window"][metric][percentile] for r in runs], **internal_style)
titles = ["Correct client responses: p95", "Queue wait: enabled variant", "call_export wall time: enabled variant",
          "Correct completions in measurement window", "Failures among started requests", "Server CPU: one logical CPU allocation"]
labels = ["Complete response (ms)", "Queue wait (ms, log scale)", "call_export (ms)", "Correct responses/s", "Failure fraction (%)", "Mean allocated CPU (%)"]
for axis, title, label in zip(axes, titles, labels):
    axis.set_title(title, loc="left", fontsize=11, fontweight="bold")
    axis.set_xlabel("Offered requests/s")
    axis.set_ylabel(label)
    axis.set_xticks([5, 20, 50])
    axis.grid(True, alpha=.18)
axes[1].set_yscale("log")
for index in (0, 2, 3, 4, 5):
    axes[index].set_ylim(bottom=0)
axes[3].plot([5, 50], [5, 50], color="#9ca3af", linewidth=1, linestyle=":", zorder=0)
axes[4].set_ylim(top=100)
axes[5].set_ylim(top=100)
variant_legend = [Line2D([0], [0], color=colors[v], label=f"Timing {v}", linewidth=2) for v in ("off", "on")]
internal_legend = [Line2D([0], [0], color=color, label=label, linewidth=2) for label, color in (("p50", "#047857"), ("p95", "#7e3a94"))]
axes[0].legend(handles=variant_legend, fontsize=9)
axes[1].legend(handles=internal_legend, fontsize=9)
fig.suptitle("Rubicon: renderer timing diagnostic (W-01)", x=.065, y=.985, ha="left", fontsize=19, fontweight="bold")
fig.text(.065, .943, "Release binaries · same Angular page · 2 independent repetitions per variant/rate · 20-second windows", fontsize=10, color="#4b5563")
fig.text(.065, .025, "Circles/solid = repetition 1; squares/dashed = repetition 2. Internal export completion and client receipt are separate populations.\nLocal laptop, logical CPU masks, no memory cap. Exploratory data; no sustained-capacity or precise logging-overhead claim.", fontsize=9, color="#4b5563")
fig.tight_layout(rect=(.02, .085, .995, .915), h_pad=2.5, w_pad=2)
fig.savefig(root / "diagnostics.png", dpi=180, facecolor="white")
fig.savefig(root / "diagnostics.svg", facecolor="white")
plt.close(fig)
print(f"DIAGNOSTIC_FIGURE {root / 'diagnostics.png'}")
