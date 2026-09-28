#!/usr/bin/env python3
"""Plot interval width vs. query count from bleichenbacher.py --csv logs."""
import csv, sys
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt

def load(path):
    qs, ws = [], []
    with open(path) as f:
        for row in csv.DictReader(f):
            qs.append(int(row["queries"]))
            ws.append(int(row["interval_width"]))
    return qs, ws

def main():
    files = sys.argv[1:] or ["weak_log.csv", "strict_log.csv"]
    titles = {"weak_log.csv": "Weak oracle\n(header bytes only: 00 02)",
              "strict_log.csv": "Strict oracle\n(real v15_decode: header + separator)"}
    colors = {"weak_log.csv": "#d64545", "strict_log.csv": "#2b6cb0"}

    present = [p for p in files if __import__("os").path.exists(p)]
    fig, axes = plt.subplots(1, len(present), figsize=(6.5 * len(present), 5.2))
    if len(present) == 1:
        axes = [axes]

    for ax, path in zip(axes, present):
        qs, ws = load(path)
        ax.plot(qs, ws, color=colors.get(path, "#333"), linewidth=1.6)
        ax.scatter([qs[-1]], [ws[-1]], color=colors.get(path, "#333"), zorder=5, s=30)
        ax.set_yscale("log")
        ax.set_xlabel("oracle queries")
        ax.set_ylabel("candidate-plaintext interval width (log scale)")
        ax.set_title(f"{titles.get(path, path)}\nfinal: {qs[-1]:,} queries")
        ax.grid(True, which="both", alpha=0.25)
        ax.ticklabel_format(axis="x", style="plain")

    fig.suptitle("Bleichenbacher's attack narrowing the plaintext interval\n"
                  "(against the real compiled RSA.cpp oracle — same key, same message)",
                  y=1.04)
    fig.tight_layout()
    fig.savefig("bleichenbacher_narrowing.png", dpi=160, bbox_inches="tight")
    print("wrote bleichenbacher_narrowing.png")

if __name__ == "__main__":
    main()
