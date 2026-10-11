"""Plot independent pilot runs; never average request percentiles across trials."""
import argparse
import json
import os
from pathlib import Path

os.environ.setdefault("MPLCONFIGDIR", str(Path(__file__).resolve().parents[2] / "crates/rubicon/target/pilot-tools/matplotlib-cache"))
import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.lines import Line2D


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("experiment", type=Path)
    args = parser.parse_args()
    results = json.loads((args.experiment / "results.json").read_text(encoding="utf-8-sig"))
    config = json.loads((args.experiment / "manifest.json").read_text(encoding="utf-8-sig"))["config"]
    rates = sorted({r["offered_rps"] for r in results})
    fig, axes = plt.subplots(3, 2, figsize=(11, 10), layout="constrained")
    colors = {"rubicon": "#2469af", "node": "#31834b"}
    panels = [
        ("Correct completions in the 20 s window", "Responses / s", lambda r: r["correct_rps_in_window"]),
        ("p95 duration of correct responses", "Milliseconds (log scale)", lambda r: r["latency_correct_ms"]["p95"] if r["latency_correct_ms"] else None),
        ("Failed or unfinished / started", "Percent", lambda r: 100 * r["failure_fraction_of_started"]),
        ("Unstarted arrivals / nominal offered", "Percent", lambda r: 100 * r["dropped"] / r["nominal_offered"]),
        ("Server CPU, normalized to one logical CPU", "Percent of allocation", lambda r: r["server"]["allocated_cpu_mean_percent"]),
        ("Server working set, sampled peak", "MiB", lambda r: r["server"]["working_set_peak_mib"]),
    ]
    for variant, color in colors.items():
        for repetition in sorted({r["repetition"] for r in results}):
            rows = sorted([r for r in results if r["variant"] == variant and r["repetition"] == repetition], key=lambda r: r["offered_rps"])
            for ax, (_, _, value) in zip(axes.flat, panels):
                points = [(r["offered_rps"], value(r), r["valid"]) for r in rows if value(r) is not None]
                ax.plot([p[0] for p in points], [p[1] for p in points], color=color,
                        linestyle="-" if repetition == 1 else "--", marker="o" if repetition == 1 else "s", markersize=4)
                for x, y, valid in points:
                    if not valid:
                        ax.scatter([x], [y], color="#bf2626", marker="x", s=65, zorder=5)
    for ax, (title, ylabel, _) in zip(axes.flat, panels):
        ax.set(title=title, ylabel=ylabel, xlabel="Offered requests / s")
        ax.set_xscale("log", base=2)
        ax.set_xticks(rates, [str(r) for r in rates])
        ax.grid(alpha=0.2)
        ax.set_ylim(bottom=0)
    axes[0, 0].plot(rates, rates, ":", color="#777777", linewidth=1, label="Offered")
    axes[0, 1].set_yscale("log")
    axes[0, 1].set_ylim(bottom=1)
    axes[0, 1].axhline(config["exploratory_p95_limit_ms"], linestyle=":", color="#777777")
    fig.suptitle("Angular Home SSR — exploratory local pilot\nFresh processes · one logical CPU per server · two independent repetitions", fontsize=14)
    legend = [Line2D([], [], color=c, label=v.capitalize()) for v, c in colors.items()]
    legend += [Line2D([], [], color="#444444", linestyle="-", marker="o", label="Repetition 1"),
               Line2D([], [], color="#444444", linestyle="--", marker="s", label="Repetition 2"),
               Line2D([], [], color="#777777", linestyle=":", label="Offered / 100 ms screening line")]
    fig.legend(handles=legend, loc="outside lower center", ncol=3, frameon=False)
    for extension in ["png", "svg"]:
        fig.savefig(args.experiment / f"pilot.{extension}", dpi=160)
    plt.close(fig)
    print(f"PILOT_PLOT {len(results)} independent runs; matplotlib {matplotlib.__version__}")


if __name__ == "__main__":
    main()
