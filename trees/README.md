# Trees & Storage Engines

High-performance, educational implementations of **B-Tree**, **B+ Tree**, and a disk-backed **LSM (Log-Structured Merge) Tree** in Rust.

---

## 1. Benchmarks ($N = 1,000,000$ Operations)

### Release Build (`cargo run --release`)

| Data Structure | Insert | Search | Delete | Full Range Scan | Storage Medium |
|---|---:|---:|---:|---:|:---:|
| **My BTree** | 151.15 ms | 40.30 ms | 109.10 ms | N/A | In-Memory (Arena) |
| **My BPlusTree** | 109.66 ms | 44.67 ms | 361.82 ms | 37.19 ms | In-Memory (Arena) |
| **`std::collections::BTreeMap`** | 92.41 ms | 44.27 ms | 41.55 ms | 9.89 ms | In-Memory |
| **`bplustree` crate** | 77.46 ms | 138.23 ms | 119.90 ms | 709 ns | In-Memory |
| **My LsmTree** | 1.19 s | 714.07 ms | 965.29 ms | 890.93 ms | **Disk-Backed (WAL + SSTables)** |
| **`fjall create (Standard LSM-Tree)`** | 1.24 s | 773.12 ms | 97.81 ms | 1.9 s | **Disk-Backed (WAL + SSTables)** |

> [! NOTE]
> `My LsmTree` performs **real disk I/O**: sequential Write-Ahead Logging (WAL), ~4KB SSTable chunking, serialized Bloom filters, and sparse block indexing. Point lookup utilizes an in-memory **Block Cache** and **zero-allocation slice scanner**, achieving **1.4M+ reads/sec**.

---

## 2. Write Flows

### B-Tree & B+ Tree Write Flow (In-Place Mutation)

```
[Insert Key]
     │
     ▼
Is Node Full? (keys == M - 1)
     │
     ├── YES ──► Preemptively Split Node
     │           ├─ Push Median Key up to Parent
     │           └─ Create Sibling Node
     │
     └── NO  ──► Binary search position & insert in sorted order
                 (B+ Tree: only leaf holds data; internal nodes hold routing keys)
```

### LSM Tree Write Flow (Append-Only)

```
[put(k, v) / delete(k)]
     │
     ├── 1. Append to WAL (Sequential disk write for durability)
     │      [CRC32 | SeqNum | Type | KeyLen | Key | ValLen | Val]
     │
     └── 2. Insert into MemTable (BTreeMap in RAM, sorted by key)
            │
            ▼
     MemTable size >= 4MB?
            │
            ├── NO  ──► Done (sub-microsecond write!)
            │
            └── YES ──► FLUSH TO DISK
                        │
                        ▼
                 Create new immutable SSTable (.sst)
                 ├─ 4KB Data Blocks (sorted entries + CRC32)
                 ├─ Filter Block (Bloom Filter over all keys)
                 ├─ Sparse Index Block (last_key -> block_offset)
                 └─ 40-byte Footer at EOF
```

---

## 3. Read & Search Flows

### B-Tree vs B+ Tree Search

```
B-Tree:                                B+ Tree:
  Node [K1, K2]                          Index [K1, K2]
   ├── Key matches? Return Value!          └── Route down child pointers
   └── Recurse into child pointer                │
                                                 ▼
                                         Leaf [K1:V1, K2:V2] ──► [Next Leaf]
                                           (All data lives in leaves.
                                            Range scans follow leaf linked list!)
```

### LSM Tree 5-Stage Point Lookup Pipeline

```
[get(k)]
   │
   ▼
[Stage 1: Active MemTable (RAM)]
   ├── Key found (Put)       ──► Return Some(Value)
   ├── Key found (Tombstone) ──► Return None (Deleted!)
   └── Not found ────────────┐
                             ▼
[Stage 2: Check SSTables (Newest -> Oldest)]
   │
   ├──► [Stage 3: Range Filter]
   │       Is key < min_key || key > max_key?
   │       └── YES ──► SKIP SSTable! (0 I/O)
   │
   ├──► [Stage 4: Bloom Filter Check]
   │       Does Bloom Filter contain key?
   │       └── NO  ──► SKIP SSTable! (0 Data Block I/O!)
   │
   └──► [Stage 5: Sparse Index + Block Cache]
           ├── Binary search Sparse Index in memory
           │   └── Pinpoints single ~4KB Data Block offset
           │
           ├── Block in Block Cache?
           │   ├── YES ──► Skip Disk I/O!
           │   └── NO  ──► Read single 4KB block from disk & cache it
           │
           └── Zero-allocation slice search inside 4KB block
               ├── Found Put       ──► Return Some(Value)
               ├── Found Tombstone ──► Return None (Deleted!)
               └── Not Found       ──► Check next older SSTable
```

---

## 4. Internal Data Layouts

### B-Tree Node Layout

```
+-------------------------------------------------------------+
| Node: is_leaf = false                                       |
| Keys:     [ 10  |  20  |  30  ]                             |
| Values:   [ V10 |  V20 |  V30 ]                             |
| Children: [ C0  |  C1  |  C2  |  C3 ]                       |
+-------------------------------------------------------------+
```

### B+ Tree Leaf Layout (Linked List of Leaves)

```
+--------------------------------+       +--------------------------------+
| Leaf Node 0                    |       | Leaf Node 1                    |
| Keys:   [ 10  |  20  ]         |       | Keys:   [ 30  |  40  ]         |
| Values: [ V10 |  V20 ]         |──next►| Values: [ V30 |  V40 ]         |
+--------------------------------+       +--------------------------------+
```

### LSM Tree SSTable File Layout (`.sst`)

```
+-------------------------------------------------------------------------------+
| Data Block 0:  [Record 0, Record 1, ...] + CRC32 (4B)                         |  <- ~4KB chunk
+-------------------------------------------------------------------------------+
| Data Block 1:  [Record 2, Record 3, ...] + CRC32 (4B)                         |  <- ~4KB chunk
+-------------------------------------------------------------------------------+
| Filter Block:  [m: u64 | k: u64 | r: u64 | raw_words...] + CRC32 (4B)         |  <- Bloom Filter
+-------------------------------------------------------------------------------+
| Index Block:   [min_key | (last_key, offset, len)...] + CRC32 (4B)            |  <- Sparse Index
+-------------------------------------------------------------------------------+
| Footer:        [filter_offset (8B) | filter_len (8B) |                         |  <- Fixed 40 bytes
|                 index_offset  (8B) | index_len  (8B) |                         |     at EOF - 40
|                 MAGIC = 0x5353_5441_424C_4531 (8B)   ]                         |
+-------------------------------------------------------------------------------+
```

### LSM Write-Ahead Log (WAL) Frame Layout

```
+-----------+--------------+------------+---------------+-----+---------------+-------+
| CRC32(4B) | SeqNum (8B)  | Type (1B)  | KeyLen (4B)   | Key | ValLen (4B)   | Value |
+-----------+--------------+------------+---------------+-----+---------------+-------+
```

---

## 5. Delete & Compaction Flows

### Deletion: In-Place vs Append-Only

```
B-Tree / B+ Tree:                  LSM Tree (Append-Only):
  Mutates tree in-place              Write Tombstone to WAL & MemTable
  Underflow -> Borrow/Merge nodes    Zero disk seeks on delete!
                                     Old versions masked until Compaction.
```

### LSM Tree Compaction Flow (Space Reclamation & Tombstone Purge)

```
Before Compaction:
  SSTable 1: [k:10 (Seq 5), k:20 (Seq 4, Tombstone)]
  SSTable 2: [k:10 (Seq 2), k:20 (Seq 1), k:30 (Seq 3)]
                   │
                   ▼  (materialize + deduplicate + rewrite compaction strategy)
Consolidated Stream:
  k:10 -> keep Seq 5 (drop older Seq 2)
  k:20 -> newest is Tombstone at bottom level -> PURGE COMPLETELY!
  k:30 -> keep Seq 3
                   │
                   ▼
After Compaction:
  SSTable 3 (New): [k:10 (Seq 5), k:30 (Seq 3)]
  (SSTable 1 and SSTable 2 deleted from disk. Dead space & tombstones reclaimed!)
```

---

## 6. How to Use

### 1. B-Tree

```rust
use trees::b_tree::BTree;

let mut tree = BTree::new();
tree.insert(10, "ten".to_string());

if let Some(val) = tree.search(10) {
    println!("Found: {val}");
}

tree.delete(10);
```

### 2. B+ Tree

```rust
use trees::b_plus_tree::BPlusTree;

let mut bplus = BPlusTree::new();
bplus.insert(10, "ten".to_string());
bplus.insert(20, "twenty".to_string());

// Range scan across leaf linked list
let results = bplus.range_search(10, 20);
for (k, v) in results {
    println!("{k} -> {v}");
}

bplus.delete(10);
```

### 3. LSM Tree

```rust
use trees::lsm_tree::{LsmTree, LsmConfig};

let dir = "/tmp/my_lsm_data";
let mut lsm = LsmTree::open_with_config(
    dir,
    LsmConfig {
        memtable_capacity_bytes: 4 * 1024 * 1024, // 4MB MemTable
        sstable_block_size: 4096,                 // 4KB blocks
        sync_wal: false,                          // High-throughput buffered WAL
    },
)?;

// Writes (Append-only WAL + MemTable)
lsm.put_str("user:101", "Alice")?;
lsm.put_str("user:102", "Bob")?;

// Point Lookup (MemTable -> Bloom Filter -> Sparse Index -> Block Cache)
if let Some(val) = lsm.get_str("user:101")? {
    println!("Found: {val}");
}

// Range Scan (Multi-way merge across MemTable and all SSTables)
let users = lsm.scan_str("user:100", "user:105")?;

// Deletions (Tombstone)
lsm.delete_str("user:101")?;

// Manual Flush & Compaction
lsm.flush()?;
lsm.compact()?;
```

---

## 7. Running Tests & Benchmarks

```bash
# Run all unit tests (34 tests covering BTree, BPlusTree, and LsmTree)
cargo test

# Run comparison benchmark across all trees
cargo run --release

# Run Criterion micro-benchmarks
cargo bench --bench lsm_tree_benchmark
cargo bench --bench b_plus_tree_benchmark
cargo bench --bench btree_benchmark
```
