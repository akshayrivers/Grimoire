# B-Tree Implementation Notes

This document records the implementation details and design decisions behind
`src/b_tree.rs`.

## Big Picture

A B-Tree of order `m`:

- every node holds at most `m - 1` keys and at most `m` children;
- every non-root node holds at least `⌈m/2⌉ - 1` keys;
- the root holds at least `1` key unless the tree is empty;
- all leaves sit at the same depth (perfect height balance);
- keys inside a node are sorted ascending and unique;
- unlike a B+ tree, **every node stores real `(key, value)` data** — there is
  no separate data level.

The minimum supported order is `3` (`MIN_ORDER`). Orders of 1 or 2 would let a
node legally hold zero minimum keys, making the occupancy rules degenerate, so
`BTree::new` panics below order 3.

## Arena-Based Storage (no pointers)

Nodes live in a flat `Vec<Node<K, V>>` and are referenced by `usize` index, not
`Box`/`Rc`. A node is just:

```rust
struct Node<K, V> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<usize>,
    parent: Option<usize>,
}
```

Decisions this drives:

- **No allocation churn during rebalancing.** Splits push new nodes onto the
  arena and merges leave dead nodes in place. `children[..]` is never freed or
  reused, so an index always names a live, reachable node — no dangling handles.
- **Easy validation.** `validate` walks the whole arena once, checking parent
  links, occupancy, sortedness, depth uniformity, and that `len` matches the
  actual key count.
- **Trade-off:** deleted nodes are leaked until the tree is dropped. For a
  teaching/learning implementation this buys huge simplicity; a production
  version would add a free-list.
- Indices are cheap to copy, so `parent: Option<usize>` costs nothing over a raw
  pointer and keeps the type trivially `Clone`.

## Node Invariants

A node with `k` keys has exactly `k + 1` children, and for internal nodes the
"slot" between `keys[i]` and `keys[i+1]` roots the subtree of keys strictly
between them. This is the same "key is a routing separator inside its own node"
convention used by the classic textbook B-Tree.

## Search

`find_key` walks from the root, using `partition_point` for binary search within
a node: the first slot `i` where `node.keys[i] >= key`; if that key matches we
stop (data lives at every level), otherwise descend into `children[i]`. Search is
`O(log n)` in node accesses.

## Insert (upsert semantics)

1. Descend to a leaf, tracking the node and slot where the key *should* live.
2. If the key already exists, **overwrite the value** — `len` is unchanged.
   This is an upsert, not a duplicate-key insertion.
3. Otherwise insert into the leaf's sorted position and increment `len`.
4. While the node is overfull (`> m - 1` keys), `split` it.

Splitting the root creates a new root with one key and two children (tree grows
at the root — the only place height ever increases).

## Split

- An overfull node is split into a left half of `⌈k/2⌉ - 1` keys and a right
  half of the remaining keys; the middle key is promoted.
- The promoted key, its left/right child pointers, and the parent link are
  wired up; the new sibling is pushed onto the arena.
- Because data lives in every node, promotion moves the key *up* as real data —
  it is removed from the node it came from (unlike the B+ leaf case, where the
  promoted key is a copy).

## Delete

1. Locate the key with `find_key`.
2. If it lives in an internal node, use the classic **successor trick**:
   - find the minimum key of the right subtree (`children[i+1]`, then follow
     `children[0]` down to a leaf),
   - move that successor `(key, value)` up into the internal slot,
   - then physically delete the successor from its leaf.
   This guarantees the key actually removed always comes from a leaf, so the
   rebalancing code below only ever has to fix underfull leaves and their
   ancestors — no special internal-key-removal case.
3. Fix underflow from that leaf upward: while a non-root node is underfull
   (`< ⌈m/2⌉ - 1` keys), call `rebalance`, which returns the parent so the walk
   can continue.
4. `shrink_root_if_needed`: if the root ends up with zero keys and one child,
   the child becomes the new root (tree shrinks at the root — the only place
   height ever decreases).

## Rebalancing (borrow before merge)

`rebalance` tries, in order:

- **Borrow from left sibling** — parent key comes down, sibling's rightmost key
  goes up to the parent.
- **Borrow from right sibling** — mirror image: parent key comes down, sibling's
  leftmost key goes up.
- **Merge with a sibling** — the parent separator falls down into the merged
  node, one parent key and one child pointer disappear. A merge may cascade
  underflow one level up, which is why `rebalance` returns the parent.

The root is exempt from the minimum-occupancy rule: a root holding one key (even
zero keys transiently) is legal and only fixed by `shrink_root_if_needed`.

## Iteration and Range Queries

- `iter` is an in-order traversal of the arena returning sorted `(key, value)`
  pairs.
- `range(lo, hi)` uses `collect_in_range`, a recursive range-pruned walk that
  only descends into subtrees whose key ranges overlap `[lo, hi]`.

Because keys are spread across internal and leaf nodes, range queries must walk
the tree itself. This is the main structural disadvantage vs. a B+ tree, whose
leaves are linked — see `b_plus_tree.md`.

## Validation (`validate`)

Walks the entire tree and asserts:

- parent links are consistent (no cross-tree edges);
- occupancy bounds are respected (root exempt from the minimum);
- `k` keys ⇒ `k + 1` children, leaf ⇒ no children;
- every node's keys are sorted and strictly increasing;
- all leaves sit at the same depth;
- the subtree key ranges match the routing separators;
- `len` equals the number of keys actually reachable.

`validate` is called after every mutation in the tests, so any invariant break is
caught immediately.

## Testing

Eight unit tests, including a randomized differential test that runs
thousands of random insert/delete/search operations against
`std::collections::BTreeMap` and asserts identical results at every step. The
differential test is the strongest correctness check in the suite.
