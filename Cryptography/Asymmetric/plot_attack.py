#!/usr/bin/env python3
"""Plot per-trial query counts from bench_results.csv -> bleichenbacher_bench.png"""
import csv, random, statistics
import matplotlib.pyplot as plt

rows = list(csv.DictReader(open("bench_results.csv")))
modes = (("weak", "#d64545", "weak oracle\n(header 00 02 only)"),
         ("strict", "#2a6ab5", "strict oracle\n(real v15_decode)"))
data = {m: [(int(r["queries"]), r["status"] == "capped") for r in rows if r["mode"] == m]
        for m, _, _ in modes}

random.seed(1)
fig, ax = plt.subplots(figsize=(7.5, 5.5))
for i, (m, color, _) in enumerate(modes):
    for q, capped in data[m]:
        ax.scatter(i + random.uniform(-0.13, 0.13), q, color=color, s=55, alpha=0.85,
                   marker="^" if capped else "o",
                   edgecolor="black" if capped else "none")
    med = statistics.median(q for q, _ in data[m])
    ax.hlines(med, i - 0.3, i + 0.3, color="black", lw=2)
    ax.text(i + 0.33, med, f"median {med:,.0f}", va="center", fontsize=9)

n = len(data["weak"])
ax.set_yscale("log")
ax.set_xticks([0, 1])
ax.set_xticklabels([lbl for _, _, lbl in modes])
ax.set_xlim(-0.6, 1.95)
ax.set_ylabel("oracle queries to recover the plaintext (log scale)")
ax.set_title(f"Bleichenbacher against the real RSA.cpp oracle\n"
             f"{n} runs per oracle, fresh 512-bit key each run "
             f"(triangle = hit the query cap)", fontsize=10)
ax.grid(alpha=0.3, axis="y", which="both")
fig.tight_layout()
fig.savefig("bleichenbacher_bench.png", dpi=200)
print("wrote bleichenbacher_bench.png")