#!/usr/bin/env python3
"""
Bleichenbacher's 1998 padding-oracle attack, run against the ACTUAL
compiled ./rsa oracle binary (RSA.cpp), not a Python re-implementation.

Usage:
    ./rsa keygen 512 > key.txt
    python3 bleichenbacher.py key.txt [--strict] [--csv out.csv]

What it does:
  1. Reads the key, builds a genuine PKCS#1 v1.5 ciphertext for a secret
     message using `./rsa v15-encrypt` (the public key only — this is
     exactly what the Mage would send).
  2. Spawns `./rsa oracle ...` as a persistent subprocess (the vulnerable
     server) and feeds it one ciphertext-guess per line over stdin,
     reading back "1"/"0" over stdout. This is a REAL padding oracle,
     backed by the user's real raw_decrypt_crt + padding check.
  3. Runs Bleichenbacher's step 2a/2b/2c interval-narrowing algorithm,
     recovers the plaintext, and logs the interval width after every
     query so it can be plotted.
"""
import subprocess, sys, csv, time, argparse

def load_key(path):
    kv = {}
    with open(path) as f:
        for line in f:
            if "=" in line:
                k, v = line.strip().split("=", 1)
                kv[k] = v
    return kv

class Oracle:
    """Persistent handle to `./rsa oracle ...` — one process, many queries."""
    def __init__(self, binpath, kv, strict):
        args = [binpath, "oracle", kv["n"], kv["d"], kv["p"], kv["q"],
                kv["dp"], kv["dq"], kv["qinv"]]
        if strict:
            args.append("--strict")
        self.proc = subprocess.Popen(
            args, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            text=True, bufsize=1)
        self.queries = 0

    def ask(self, c_int, k_bytes):
        """c_int: integer ciphertext. Returns True if padding looked valid."""
        self.queries += 1
        hex_c = format(c_int, f"0{k_bytes*2}x")
        self.proc.stdin.write(hex_c + "\n")
        self.proc.stdin.flush()
        resp = self.proc.stdout.readline().strip()
        return resp == "1"

    def close(self):
        self.proc.stdin.close()
        self.proc.wait()


def bleichenbacher(oracle, n, e, c0, k, log_rows, progress_every=200):
    B = 1 << (8 * (k - 2))
    B2, B3 = 2 * B, 3 * B
    cdiv = lambda a, b: -(-a // b)  # ceil division

    ask_s = lambda s: oracle.ask((c0 * pow(s, e, n)) % n, k)

    def width_sum(M):
        return sum(b - a for a, b in M)

    def narrow(M, s):
        out = []
        for a, b in M:
            r_lo = cdiv(a * s - B3 + 1, n)
            r_hi = (b * s - B2) // n
            for r in range(r_lo, r_hi + 1):
                lo = max(a, cdiv(B2 + r * n, s))
                hi = min(b, (B3 - 1 + r * n) // s)
                if lo <= hi:
                    out.append((lo, hi))
        out.sort()
        merged = []
        for lo, hi in out:
            if merged and lo <= merged[-1][1] + 1:
                merged[-1] = (merged[-1][0], max(hi, merged[-1][1]))
            else:
                merged.append((lo, hi))
        return merged

    t0 = time.time()

    # Step 2a: find the first conforming s >= n/3B (ceil(n/B3)).
    s = cdiv(n, B3)
    while not ask_s(s):
        s += 1
        if oracle.queries % progress_every == 0:
            print(f"  [2a] queries={oracle.queries} s={s}", file=sys.stderr)

    M = narrow([(B2, B3 - 1)], s)
    log_rows.append((oracle.queries, width_sum(M), time.time() - t0))

    while not (len(M) == 1 and M[0][0] == M[0][1]):
        if len(M) > 1:
            # Step 2b: multiple intervals, just increment s.
            s += 1
            while not ask_s(s):
                s += 1
        else:
            # Step 2c: one interval, search a narrowing range of s.
            a, b = M[0]
            r = cdiv(2 * (b * s - B2), n)
            found = False
            while not found:
                lo_s = cdiv(B2 + r * n, b)
                hi_s = (B3 - 1 + r * n) // a
                for cand in range(lo_s, hi_s + 1):
                    if ask_s(cand):
                        s = cand
                        found = True
                        break
                r += 1
        M = narrow(M, s)
        log_rows.append((oracle.queries, width_sum(M), time.time() - t0))
        if oracle.queries % progress_every < 5:
            print(f"  [narrow] queries={oracle.queries} "
                  f"width={width_sum(M)} intervals={len(M)}", file=sys.stderr)

    return M[0][0]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("keyfile")
    ap.add_argument("--strict", action="store_true",
                     help="attack the real v15_decode (header+separator) "
                          "instead of the header-only weak oracle")
    ap.add_argument("--secret", default="4d616e676c75212121",
                     help="hex message to attack (default: 'Manglu!!!' )")
    ap.add_argument("--csv", default=None, help="write per-query log here")
    ap.add_argument("--bin", default="./rsa")
    args = ap.parse_args()

    kv = load_key(args.keyfile)
    n = int(kv["n"], 16)
    e = int(kv["e"], 16)
    k = (n.bit_length() + 7) // 8
    print(f"key: {n.bit_length()} bits, k={k} bytes, "
          f"mode={'strict' if args.strict else 'weak'}", file=sys.stderr)

    # Build the victim's real ciphertext via the actual v15_encode + raw_encrypt.
    out = subprocess.run([args.bin, "v15-encrypt", kv["n"], kv["e"], args.secret],
                          capture_output=True, text=True, check=True)
    c0 = int(out.stdout.strip(), 16)
    print(f"target ciphertext built via real v15_encode()", file=sys.stderr)

    oracle = Oracle(args.bin, kv, args.strict)
    log_rows = []
    t0 = time.time()
    try:
        recovered_int = bleichenbacher(oracle, n, e, c0, k, log_rows)
    finally:
        oracle.close()
    elapsed = time.time() - t0

    em = recovered_int.to_bytes(k, "big")
    msg = em[em.index(0, 2) + 1:]
    print(f"\nqueries: {oracle.queries}")
    print(f"elapsed: {elapsed:.1f}s")
    print(f"recovered plaintext: {msg!r}")
    expected = bytes.fromhex(args.secret)
    print(f"expected plaintext:  {expected!r}")
    print(f"MATCH: {msg == expected}")

    if args.csv:
        with open(args.csv, "w", newline="") as f:
            w = csv.writer(f)
            w.writerow(["queries", "interval_width", "elapsed_s"])
            w.writerows(log_rows)
        print(f"log written to {args.csv}")


if __name__ == "__main__":
    main()
