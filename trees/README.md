## Release Build (`cargo run --release`)

| Tree | Insert | Search | Delete | Full Range Scan |
|---|---:|---:|---:|---:|
| My BTree | 135.05 ms | 40.38 ms | 127.98 ms | N/A |
| My BPlusTree | 113.36 ms | 43.45 ms | 353.13 ms | 38.20 ms |
| `std::collections::BTreeMap` | 97.39 ms | 46.27 ms | 41.73 ms | 6.72 ms |
| `bplustree` crate | 83.33 ms | 101.57 ms | 115.96 ms | 666 ns |

---

## Debug Build (`cargo run`)

| Tree | Insert | Search | Delete | Full Range Scan |
|---|---:|---:|---:|---:|
| My BTree | 553.71 ms | 318.36 ms | 494.29 ms | N/A |
| My BPlusTree | 526.89 ms | 351.28 ms | 3.23 s | 54.30 ms |
| `std::collections::BTreeMap` | 822.55 ms | 437.66 ms | 448.46 ms | 49.47 ms |
| `bplustree` crate | 1.46 s | 1.22 s | 1.18 s | 2.17 µs |