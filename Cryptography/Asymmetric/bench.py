#!/usr/bin/env python3
"""
bench.py: repeat the Bleichenbacher lab many times and report medians/IQR.

Put this next to bleichenbacher.py and the compiled ./rsa, then:
    python3 bench.py                                   # 20 trials per oracle, 512-bit keys
    python3 bench.py --trials 30 --workers 6 --max-queries 8000000

What it does:
  1. Measures the acceptance rate directly: random values uniform in [2B, 3B)
     are encrypted and sent to both oracles. Weak should accept ~100%,
     strict accepts the fraction that also has a well-formed separator.
     Compared against the closed-form prediction.
  2. Runs N full attacks per oracle (fresh key + fresh ciphertext each trial,
     against the real ./rsa oracle) and records total queries, queries spent
     in step 2a (finding the first accepted s), and wall time.
  3. Prints median / IQR / min / max and writes bench_results.csv.

Trials that exceed --max-queries are counted as "capped" and enter the stats
as lower bounds (value = cap).
"""
import argparse, csv, secrets, statistics, subprocess, sys, time
from concurrent.futures import ProcessPoolExecutor, as_completed

from bleichenbacher import Oracle, bleichenbacher

SECRET = b"Manglu!!!"


class QueryCap(Exception):
    pass


class CapOracle(Oracle):
    """Oracle that stops after `cap` queries and remembers the first accepted query."""

    def __init__(self, binpath, kv, strict, cap):
        super().__init__(binpath, kv, strict)
        self.cap = cap
        self.first_hit = None

    def ask(self, c_int, k_bytes):
        if self.queries >= self.cap:
            raise QueryCap()
        r = super().ask(c_int, k_bytes)
        if r and self.first_hit is None:
            self.first_hit = self.queries
        return r


def gen_key(binpath, bits):
    out = subprocess.run([binpath, "keygen", str(bits)], capture_output=True,
                         text=True, check=True).stdout
    kv = {}
    for line in out.splitlines():
        if "=" in line:
            a, b = line.strip().split("=", 1)
            kv[a] = b
    return kv


def accept_rate(binpath, bits, samples):
    kv = gen_key(binpath, bits)
    n, e = int(kv["n"], 16), int(kv["e"], 16)
    k = (n.bit_length() + 7) // 8
    B = 1 << (8 * (k - 2))
    weak = Oracle(binpath, kv, False)
    strict = Oracle(binpath, kv, True)
    hw = hs = 0
    for _ in range(samples):
        em = 2 * B + secrets.randbelow(B)          # uniform in [2B, 3B)
        c = pow(em, e, n)
        hw += weak.ask(c, k)
        hs += strict.ask(c, k)
    weak.close()
    strict.close()
    # strict needs: bytes 2..9 all non-zero, and a zero somewhere in bytes 10..k-2
    pred = (255 / 256) ** 8 * (1 - (255 / 256) ** (k - 11))
    print(f"[accept rate] k={k} bytes, {samples} random values in [2B,3B)")
    print(f"  weak   accepts {hw / samples:.4f}   (expect 1.0000)")
    print(f"  strict accepts {hs / samples:.4f}   (predicted {pred:.4f}, "
          f"so strict hit is ~{1 / pred:.1f}x rarer)\n", flush=True)


def run_trial(job):
    mode, idx, bits, binpath, cap = job
    kv = gen_key(binpath, bits)
    n, e = int(kv["n"], 16), int(kv["e"], 16)
    k = (n.bit_length() + 7) // 8
    out = subprocess.run([binpath, "v15-encrypt", kv["n"], kv["e"], SECRET.hex()],
                         capture_output=True, text=True, check=True)
    c0 = int(out.stdout.strip(), 16)
    oracle = CapOracle(binpath, kv, mode == "strict", cap)
    t0 = time.time()
    status, correct = "ok", False
    try:
        rec = bleichenbacher(oracle, n, e, c0, k, [], progress_every=10 ** 12)
        em = rec.to_bytes(k, "big")
        correct = em[em.index(0, 2) + 1:] == SECRET
    except QueryCap:
        status = "capped"
    finally:
        oracle.close()
    return dict(mode=mode, trial=idx, status=status, correct=correct,
                queries=oracle.queries, step2a=oracle.first_hit or oracle.queries,
                seconds=round(time.time() - t0, 2))


def quart(xs):
    if len(xs) < 2:
        return (xs[0], xs[0], xs[0]) if xs else (0, 0, 0)
    q = statistics.quantiles(xs, n=4, method="inclusive")
    return q[0], q[1], q[2]


def summarize(rows, mode):
    rs = [r for r in rows if r["mode"] == mode]
    capped = sum(r["status"] == "capped" for r in rs)
    wrong = sum(r["status"] == "ok" and not r["correct"] for r in rs)
    tot = [r["queries"] for r in rs]
    a2 = [r["step2a"] for r in rs]
    secs = [r["seconds"] for r in rs]
    q1, med, q3 = quart(tot)
    print(f"{mode.upper()}  trials={len(rs)}  capped={capped}  wrong_plaintext={wrong}")
    print(f"  total queries : min={min(tot):,}  Q1={q1:,.0f}  median={med:,.0f}  "
          f"Q3={q3:,.0f}  max={max(tot):,}")
    print(f"  step 2a only  : median={statistics.median(a2):,.0f}   "
          f"(share of total: {statistics.median(a2) / med:.0%})")
    print(f"  seconds       : median={statistics.median(secs):.1f}  max={max(secs):.1f}\n")
    return med, q1, q3


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--trials", type=int, default=20)
    ap.add_argument("--bits", type=int, default=512)
    ap.add_argument("--workers", type=int, default=4)
    ap.add_argument("--max-queries", type=int, default=5_000_000)
    ap.add_argument("--accept-samples", type=int, default=20000)
    ap.add_argument("--bin", default="./rsa")
    ap.add_argument("--csv", default="bench_results.csv")
    a = ap.parse_args()

    accept_rate(a.bin, a.bits, a.accept_samples)

    jobs = [(m, i, a.bits, a.bin, a.max_queries)
            for i in range(a.trials) for m in ("strict", "weak")]
    rows = []
    with ProcessPoolExecutor(max_workers=a.workers) as pool:
        futs = [pool.submit(run_trial, j) for j in jobs]
        for f in as_completed(futs):
            r = f.result()
            rows.append(r)
            print(f"  {r['mode']:6s} #{r['trial']:<3d} {r['status']:6s} "
                  f"queries={r['queries']:>9,d}  {r['seconds']:>7.1f}s", flush=True)

    with open(a.csv, "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0].keys()))
        w.writeheader()
        w.writerows(sorted(rows, key=lambda r: (r["mode"], r["trial"])))
    print(f"\nper-trial results written to {a.csv}\n")

    mw, w1, w3 = summarize(rows, "weak")
    ms, s1, s3 = summarize(rows, "strict")
    print(f"median ratio strict/weak = {ms / mw:.1f}x")
    print("\n--- paste-ready ---")
    print(f"{a.trials} runs per oracle, fresh {a.bits}-bit key and ciphertext each run.")
    print(f"- weak:   median {mw:,.0f} queries (IQR {w1:,.0f} to {w3:,.0f})")
    print(f"- strict: median {ms:,.0f} queries (IQR {s1:,.0f} to {s3:,.0f})")


if __name__ == "__main__":
    main()