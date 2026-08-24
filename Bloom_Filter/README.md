# Bloom Filter

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
             ┌───────────┴───────────┐
             │                       │
        Bloom bit array         Collision bitmap
             │                       │
          m bits                   r bits
             │                       │
       ┌─────┴─────┐          region collision
       │           │             tracking
       ▼           ▼
    positions   regions
       │           │
       └─────┬─────┘
             ▼
       collision state
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

## Benchmarks
The implementation includes Criterion benchmarks for:

- insertion
- lookup
- deletion
- false-positive rate
- deletion success rate
Command : 
```
cargo bench
```
Benchamrked on Macbook Air M1 , macOS Tahoe Version 26.5.2
### False Positive Rate
| Bits / element |  k | False positives |    FPR |
| -------------: | -: | --------------: | -----: |
|              6 |  4 |            5658 | 5.658% |
|              8 |  6 |            2119 | 2.119% |
|             10 |  7 |             780 | 0.780% |
|             12 |  8 |             374 | 0.374% |
|             14 | 10 |             139 | 0.139% |

As expected, increasing the number of bits available per element reduces the observed false-positive rate.

### Deletion Success Rate
Deletion success depends heavily on the number of regions `r` and the number of inserted elements.

For example, with 5,000 elements: 
| Regions (`r`) | Successful deletions |
| ------------: | -------------------: |
|           250 |               51.94% |
|           500 |               89.68% |
|          1000 |               99.56% |

This demonstrates the fundamental trade-off of the Deletable Bloom Filter:
more regions provide finer-grained collision tracking and therefore make
more deletions possible, at the cost of additional collision metadata.

Tests:
```
cargo test
```
current test suite: 
```
running 15 tests
test bloom::tests::delete_inserted_element ... ok
test bloom::tests::contains_definitely_absent_element ... ok
test bloom::tests::contains_inserted_element ... ok
test bit_array::tests::set_get_clear ... ok
test bloom::tests::delete_missing_element ... ok
test bloom::tests::delete_returns_unsafe_when_region_has_collision ... ok
test bloom::tests::enhanced_hashing_changes_probe_sequence ... ok
test bloom::tests::deleting_one_element_does_not_remove_another ... ok
test bloom::tests::generates_k_positions ... ok
test bloom::tests::insert_sets_bits ... ok
test bloom::tests::inserting_same_element_creates_collisions ... ok
test bloom::tests::positions_are_unique ... ok
test hash::fnv::tests::same_input_same_hash ... ok
test hash::xxhash::tests::same_input_same_hash ... ok
test hash::murmur3::tests::same_input_same_hash ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```
