# Deletable Bloom Filter (DlBF) in Rust

A Bloom filter is a probabilistic data structure that answers a simple question:

> "Could this element be present?"

It does this using very little memory.

A Bloom filter has an important guarantee:

- **Definitely not present** → guaranteed to be correct.
- **Possibly present** → may be a false positive.

For example, if the filter says `manglu` is not present, we know that
`manglu` was never inserted. If it says `manglu` might be present,
the actual storage must still be checked.


## What I Built

A memory-efficient implementation of the **Deletable Bloom Filter (DlBF)** written in Rust, supporting non-cryptographic hashing algorithms (xxHash, MurmurHash3, FNV-1a) with enhanced double-hashing probe sequences.

This project is an implementation of a **Deletable Bloom Filter (DlBF)**,
based on the approach described in the paper:

> *A Deletable Bloom Filter* — [paper](https://arxiv.org/pdf/1005.0352)

The implementation focuses on the mechanics of the data structure rather
than the probability mathematics.

For the mathematical foundations of Bloom filters, I found
[Arpit Bhayani's Bloom Filter article](https://arpitbhayani.me/blogs/bloom-filters)
particularly useful.



## Basic Bloom Filter

A traditional Bloom filter consists of two fundamental components:

1. A bit array of `m` bits
2. `k` hash functions used to generate bit positions

When inserting an element:

```text
element
   │
   ▼
hash functions
   │
   ├──► position 1
   ├──► position 2
   ├──► ...
   └──► position k
          │
          ▼
       set bits
```

When querying an element, all k positions are checked.

If any bit is unset, the element is definitely absent.

If all bits are set, the element is possibly present.

## Hashing
The implementation supports multiple non-cryptographic hash functions:

- xxHash
- MurmurHash3
- FNV-1a

Instead of computing k independent hash functions, the implementation
uses the Kirsch–Mitzenmacher optimization, deriving the probe sequence
from two hash values:
```
g_i(x) = h1(x) + i × h2(x)
```
This reduces the number of independent hash computations required.

The implementation also uses an enhanced probe sequence to reduce the
correlation issues associated with basic double hashing:
```
h_i = h1 + i × h2 + i(i - 1) / 2
```

## Deleteable Bloom Filter
A normal Bloom filter cannot safely delete elements.

Suppose:
```
A → bits 1, 4, 7
B → bits 4, 8, 10
```
If we delete A and clear bit 4, we would accidentally affect B.

That can introduce a false negative, which violates the fundamental
Bloom filter guarantee.

The Deletable Bloom Filter takes a different approach.

Instead of storing counters for every bit, the filter divides the bit array
into r logical regions and maintains a separate collision bitmap.

```
                        Deletable Bloom Filter
                                 │
                 ┌───────────────┴───────────────┐
                 ▼                               ▼
       Bloom Filter Bit Array            Collision Bitmap
           (m bits, Vec<u64>)                 (r bits)
                 │                               │
         ┌───────┴───────┐                ┌──────┴──────┐
         ▼               ▼                ▼             ▼
    Position p₁ ... Position pₖ       Region R(p₁) ... Region R(pₖ)
         │               │                │             │
         └───────────────┼────────────────┘             │
                         ▼                              ▼
                 Bit Set / Test                 Collision Tracking
```
###  Insertion

For every generated bit position:

1. Check whether the bit is already set.
2. If it is, mark its region as having a collision.
3. Set the bit.

## Deletion

To delete an element:

1. Check whether all of its bits are present.
2. Find a bit belonging to a collision-free region.
3. If one exists, that bit can be safely cleared.
4. If every bit belongs to a collision region, deletion is unsafe.

Therefore deletion is probabilistic: 
Deleted successfully
        OR
Unsafe to delete

The advantage is lower memory overhead compared with the counting Bloom filters, at the cost of some deletions being impossible.

## Implementation
The project is written in Rust
```
.
├── Cargo.lock
├── Cargo.toml
├── README.md
├── benches
│   ├── deletion.rs
│   ├── insertion.rs
│   └── lookup.rs
├── examples
│   ├── basic.rs
│   └── deletion.rs
└── src
    ├── bit_array.rs
    ├── bloom.rs
    ├── hash
    │   ├── fnv.rs
    │   ├── mod.rs
    │   ├── murmur3.rs
    │   └── xxhash.rs
    ├── hasher.rs
    └── lib.rs
```
The bit array is stored as: 
```rust
Vec<u64>
```
This means the implementation stores 64 Bloom-filter bits inside each `u64`, rather than allocating one byte per bit.

For example:
```
u64
┌──────────────────────────────────────────────────────────────┐
│ 64 individual Bloom-filter bits                              │
└──────────────────────────────────────────────────────────────┘
```
An m = 1024 bit filter therefore requires:
```
1024 / 64 = 16 u64 words
```
rather than 1024 individual storage units.

## Mathematical Foundations

### 1. Optimal Number of Hash Functions ($k$)
Given filter size $m$ and expected items $n$, the optimal $k$ that minimizes the false positive rate is:
$$k_{opt} = \frac{m}{n} \ln 2 \approx 0.69315 \cdot \left(\frac{m}{n}\right)$$

### 2. Theoretical False Positive Rate ($p$)
The probability of a false positive after inserting $n$ elements into an $m$-bit filter with $k$ hash functions is:
$$p \approx \left( 1 - e^{-k n / m} \right)^k$$

### 3. Space Requirement
To achieve a target false positive probability $p$:
$$\frac{m}{n} = -\frac{\ln p}{(\ln 2)^2} \approx -1.4427 \log_2 p \quad \text{bits/element}$$

### 4. Memory Overhead Comparison

| Filter Type | Storage per Element ($p \approx 1\%$) | Auxiliary Metadata | Memory Overhead vs Standard BF |
| :--- | :---: | :---: | :---: |
| **Standard Bloom Filter** | $\approx 9.6$ bits | $0$ bits | Baseline ($1.0\times$) |
| **Deletable Bloom Filter (DlBF, $r = m/1000$)** | $\approx 9.6$ bits | $0.001$ bits | **$+0.1\%$ ($1.001\times$)** |
| **Deletable Bloom Filter (DlBF, $r = m/100$)** | $\approx 9.6$ bits | $0.01$ bits | **$+1.0\%$ ($1.01\times$)** |
| **Counting Bloom Filter (4-bit counter)** | $\approx 38.4$ bits | None | **$+300\%$ ($4.0\times$)** |
| **Counting Bloom Filter (8-bit counter)** | $\approx 76.8$ bits | None | **$+700\%$ ($8.0\times$)** |

## Benchmarks & Empirical Results

*Benchmarked on Apple Silicon (M1, 8 cores) using Criterion.rs with $100,000$ operations per iteration.*

### 1. Insertion Throughput

| Hash Algorithm | Total Time ($100\text{k}$ inserts) | Latency per Insert | Throughput |
| :--- | :---: | :---: | :---: |
| **XXH3 (xxHash)** | **$4.69\text{ ms}$** | **$46.9\text{ ns}$** | **$21.3\text{ M ops/sec}$** |
| **FNV-1a** | $6.13\text{ ms}$ | $61.3\text{ ns}$ | $16.3\text{ M ops/sec}$ |
| **MurmurHash3 (x64_128)** | $6.51\text{ ms}$ | $65.1\text{ ns}$ | $15.4\text{ M ops/sec}$ |

### 2. Lookup Latency

| Query Type | Total Time ($100\text{k}$ queries) | Latency per Lookup | Throughput |
| :--- | :---: | :---: | :---: |
| **Present Elements** (all $k=7$ bits match) | $1.02\text{ ms}$ | **$10.2\text{ ns}$** | **$98.0\text{ M ops/sec}$** |
| **Absent Elements** (early short-circuit) | $2.17\text{ ms}$ | **$21.7\text{ ns}$** | **$46.1\text{ M ops/sec}$** |

### 3. False Positive Rate: Theoretical vs. Measured ($N = 100,000$)

| Bits / Element ($m/n$) | Hash Count ($k$) | Theoretical FPR | Empirical False Positives | Measured FPR | Error ($\Delta$) |
| :---: | :---: | :---: | :---: | :---: | :---: |
| **6** | 4 | $5.607\%$ | $5,562\ /\ 100,000$ | **$5.562\%$** | $-0.045\%$ |
| **8** | 6 | $2.158\%$ | $2,184\ /\ 100,000$ | **$2.184\%$** | $+0.026\%$ |
| **10** | 7 | $0.819\%$ | $778\ /\ 100,000$ | **$0.778\%$** | $-0.041\%$ |
| **12** | 8 | $0.314\%$ | $308\ /\ 100,000$ | **$0.308\%$** | $-0.006\%$ |
| **14** | 10 | $0.119\%$ | $125\ /\ 100,000$ | **$0.125\%$** | $+0.006\%$ |

### 4. Deletion Success Rate ($M = 1,000,000\text{ bits}, k = 7$)

The success rate of deletions depends on the number of regions $r$ and the load factor $n/m$:

| Inserted Elements ($N$) | Load Factor ($k N / M$) | Regions $r = 100$ | Regions $r = 250$ | Regions $r = 500$ | Regions $r = 1,000$ |
| :---: | :---: | :---: | :---: | :---: | :---: |
| **1,000** | $0.7\%$ | $100.00\%$ | $100.00\%$ | $100.00\%$ | **$100.00\%$** |
| **5,000** | $3.5\%$ | $12.94\%$ | $46.08\%$ | $86.54\%$ | **$99.22\%$** |
| **10,000** | $7.0\%$ | $0.00\%$ | $0.00\%$ | $5.16\%$ | **$47.52\%$** |
| **25,000+** | $\ge 17.5\%$ | $0.00\%$ | $0.00\%$ | $0.00\%$ | **$0.00\%$** |

> **Insight:** At higher element counts ($N > 25,000$), the number of inserted bits ($k N \ge 175,000$) causes collisions across all regions (Birthday Paradox per region $s = m/r$). Increasing $r$ (finer-grained regions) or expanding $M$ scales deletion capacity up smoothly.

## Usage

Add this to your `Cargo.toml`:
```toml
[dependencies]
bloom_filter = { path = "." }
```

### Basic Example
```rust
use bloom_filter::BloomFilter;
use bloom_filter::hash::xxhash::Xxhash;

fn main() {
    let hasher = Xxhash::new(42);
    // m = 1,000,000 bits, k = 7 hashes, r = 1,000 regions
    let mut filter = BloomFilter::new(1_000_000, 7, 1_000, hasher);

    filter.insert(b"user_session_123");

    assert!(filter.contains(b"user_session_123"));
    assert!(!filter.contains(b"unknown_session"));
}
```

### Deletion Example
```rust
use bloom_filter::{BloomFilter, DeleteResult};
use bloom_filter::hash::xxhash::Xxhash;

fn main() {
    let hasher = Xxhash::new(42);
    let mut filter = BloomFilter::new(1_000_000, 7, 1_000, hasher);

    filter.insert(b"cache_key_alpha");

    match filter.delete(b"cache_key_alpha") {
        DeleteResult::Deleted => println!("Successfully deleted!"),
        DeleteResult::NotFound => println!("Key was not found."),
        DeleteResult::UnsafeToDelete => println!("Key collided in all regions; deletion prevented."),
    }

    assert!(!filter.contains(b"cache_key_alpha"));
}
```

---

## Running Tests & Benchmarks

Run the complete test suite (17 unit tests):
```bash
cargo test
```

Run Criterion microbenchmarks:
```bash
cargo bench
```