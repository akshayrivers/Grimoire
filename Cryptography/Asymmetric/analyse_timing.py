#!/usr/bin/env python3
"""
Analyze timing CSVs from `./rsa time-v15` / `./rsa time-oaep`.

Usage:
    python3 analyze_timing.py v15_timing.csv oaep.png_prefix
    python3 analyze_timing.py oaep_timing.csv oaep

Each CSV has columns: category,ns
Trims the top 1% as GC/scheduler noise (standard practice for timing
measurements on a shared, non-isolated CPU), then reports the median per
category and runs a Mann-Whitney U test between categories: the
nonparametric test used because timing distributions are never normal
(long right tail from scheduler jitter), and what real timing-attack
papers use to establish statistical separation.
"""
import sys
import csv
import statistics
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from scipy.stats import mannwhitneyu

def load(path):
    cats = {}
    with open(path) as f:
        for row in csv.DictReader(f):
            cats.setdefault(row["category"], []).append(int(row["ns"]))
    return cats

def trim(xs, pct=1.0):
    xs = sorted(xs)
    cut = int(len(xs) * pct / 100)
    return xs[:len(xs) - cut] if cut else xs

def main():
    path = sys.argv[1] if len(sys.argv) > 1 else "v15_timing.csv"
    out_prefix = sys.argv[2] if len(sys.argv) > 2 else path.rsplit(".", 1)[0]

    cats = load(path)
    trimmed = {k: trim(v) for k, v in cats.items()}

    print(f"=== {path} ===")
    for k, v in trimmed.items():
        print(f"{k:20s} n={len(v):6d}  median={statistics.median(v):8.0f} ns  "
              f"mean={statistics.mean(v):8.0f} ns  stdev={statistics.stdev(v):8.0f} ns")

    names = list(trimmed.keys())
    print()
    for i in range(len(names)):
        for j in range(i + 1, len(names)):
            a, b = trimmed[names[i]], trimmed[names[j]]
            u, p = mannwhitneyu(a, b, alternative="two-sided")
            med_a, med_b = statistics.median(a), statistics.median(b)
            sep = "SEPARABLE" if p < 1e-6 else "not clearly separable"
            print(f"{names[i]:12s} vs {names[j]:20s}  "
                  f"median diff={med_b-med_a:+8.0f} ns   p={p:.2e}   {sep}")

    # ---- plot: overlapping histograms, log-x, trimmed ----
    fig, ax = plt.subplots(figsize=(9, 5.5))
    colors = {"valid": "#2b6cb0", "bad_header": "#d64545",
              "bad_late_separator": "#d69e2e", "bad_lhash": "#38a169"}
    all_vals = [v for vals in trimmed.values() for v in vals]
    bins = 80
    for name, vals in trimmed.items():
        ax.hist(vals, bins=bins, alpha=0.55, label=f"{name} (median {statistics.median(vals):.0f} ns)",
                 color=colors.get(name), density=True)
    ax.set_xlabel("decode time (ns), top 1% trimmed")
    ax.set_ylabel("density")
    ax.set_title(f"Timing distribution: {path}\n(single decode call, RSA exponentiation excluded)")
    ax.legend()
    fig.tight_layout()
    outpng = f"{out_prefix}_timing_hist.png"
    fig.savefig(outpng, dpi=160)
    print(f"\nwrote {outpng}")

if __name__ == "__main__":
    main()