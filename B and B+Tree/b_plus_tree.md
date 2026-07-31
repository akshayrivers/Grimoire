# B+ Tree Implementation Notes

This document records the implementation details and design decisions behind
`src/b+_tree.rs`.

## Big Picture

A B+ Tree of order `m`:

- **internal nodes** hold separators and child indices, but **no values** —
  they only route queries;
- **leaf nodes** hold all keys *and* their values, sorted;
- leaves are chained with a `next` link so range scans and full iteration are a
  linear walk of the leaf level;
- every non-root node holds between `⌈m/2⌉ - 1` and `m - 1` keys;
- all leaves sit at the same depth.

The minimum supported order is `3` (`MIN_ORDER`), enforced by an assert in
`BPlusTree::new`, for the same reason as the B-Tree (orders 1–2 make minimum
occupancy degenerate).

## Arena-Based Storage

Same design as the B-Tree: nodes live in a flat `Vec<Node<K, V>>` addressed by
`usize` indices. A node is:

```rust
struct Node<K, V> {
    keys: Vec<K>,
    values: Vec<V>,          // only meaningful in leaves
    children: Vec<usize>,    // only meaningful in internal nodes
    next: Option<usize>,     // leaf chain
    parent: Option<usize>,
}
```

Every node uses the same `Node<K, V>` type; `values` is empty in internal nodes
and `children` is empty in leaves. `is_leaf()` is `children.is_empty()`. Keeping
one struct (rather than `enum` variants) keeps splits, merges, and validation
uniform at the cost of a little wasted space in internal nodes.

The arena buy-us-less-memory-allocation, cheap validation, and dangling-pointer
safety arguments from `b_tree.md` apply identically here.

## Leaf Chain and `first_leaf`

Leaves are singly linked via `next`. `first_leaf` caches the leftmost leaf so
`iter()` and `range()` start in `O(1)` instead of re-descending the tree:

- `first_leaf` is refreshed at the end of every insert/delete;
- `iter` walks `next` from `first_leaf`, so the leaf order is read-only
  validated against the stored chain.

This is the defining advantage over a B-Tree: **range queries never touch an
internal node** once the starting leaf is found.

## The Separator Invariant

For every internal node, `keys[i]` must equal the **minimum key of the subtree
rooted at `children[i+1]`**. This is what makes searches correct: a query that
reaches an internal node picks the right child by comparing against the
separators.

Two consequences drive most of the tricky code:

1. **Inserting a new minimum** (or deleting one) changes the subtree minimum of
   a leftmost chain of nodes — every ancestor's `keys[0]` that mirrors it must
   be refreshed.
2. **Splits, merges, and borrows rearrange subtree boundaries**, so the
   separator that falls into or out of a node can go stale.

The rest of this document is largely about keeping that invariant true.

## Insert (upsert semantics)

1. Descend to the leaf; if the key exists, overwrite the value and return
   (`len` unchanged).
2. Insert into the leaf's sorted position (`partition_point`); increment `len`.
3. If the new key became the leaf's minimum, call `propagate_min` — see below.
4. While the node is overfull, `split` it.

The B-Tree's "delete via successor" trick is unnecessary here: **all data is in
leaves**, so every physical removal already happens at the leaf level.

## Split: leaves vs. internal nodes

Both halves split at the same point, but the separator is treated differently:

- **Leaf split**: the promoted key is real data, so it is *kept* in the right
  half and a *copy* is pushed up as a separator. A range scan over the leaves
  must still see the promoted key.
- **Internal split**: the median is a routing separator, not data, so it is
  *moved* up wholesale and removed from the node it split out of.

An overfull root splits into a new root with one separator and two children —
the only place height grows.

`insert_separator` inserts the promoted key into the parent and wires the new
sibling into `children`, shifting child indices; it returns the index in the
parent where the separator landed so the caller can refresh the one separator
(which is the fresh minimum of the new subtree) exactly when the new sibling
landed in the `children[0]` slot.

## Propagating a Minimum Change (`propagate_min`)

`propagate_min(node, key)` walks upward from `node` carrying the subtree minimum
`key`:

- if the current node is `children[0]` of its parent, the parent's own minimum
  is still `key` (the leftmost subtree's minimum *is* the parent's minimum), so
  the walk continues upward unchanged;
- otherwise the parent's `keys[pos - 1]` separator mirrors this subtree's
  minimum and is set to `key`, and the walk stops.

`node_min(node)` computes the current minimum of a subtree: a leaf's `keys[0]`,
or the minimum of an internal node's `children[0]` subtree (an internal node's
`keys[0]` is *not* its minimum — it is the separator for `children[1]`). This
subtlety caused the nastiest bug in the project: re-reading `node.keys[0]`
during the walk is wrong for internal nodes and produced stale separators after
deleting a run of minimum keys.

## Delete

1. Search down to the leaf; if the key is absent, return `false`.
2. Remove the key/value from the leaf, decrement `len`.
3. If the removed key was the leaf's minimum and the leaf is still non-empty,
   `propagate_min` **before** any rebalancing — so that later merges never copy
   a stale separator downward.
4. While a non-root node is underfull (`< ⌈m/2⌉ - 1` keys), `rebalance` it and
   move up. Leaves and internal nodes rebalance differently (below).
5. After rebalancing, `propagate_min` once more from the final node: a merge of
   a node in the `children[0]` position can change an ancestor's minimum
   silently even when that node itself ends up valid.
6. `fix_root` — if the root is an internal node with a single child, the child
   becomes the new root; if the root is an empty leaf (tree is empty), it is
   cleared to `None`.

### Leaf rebalancing

For an underfull leaf:

- **Borrow from a right sibling** if it has more than the minimum: take the
  sibling's first key, and refresh the parent separator that mirrored it
  (`keys[pos]` becomes the sibling's new minimum). The separator that was
  mirrored by the borrowed key is then refreshed separately.
- **Borrow from a left sibling** if it has more than the minimum: take the
  sibling's last key, and refresh the separator `keys[pos - 1]`.
- **Merge** with a sibling otherwise: the separator between them is *pulled
  down* (it is only a routing key, the leaf chain no longer needs it) and the
  sibling is unlinked from the `next` chain.

### Internal rebalancing

For an underfull internal node:

- **Borrow from a sibling** if it has more than the minimum: the parent
  separator falls down into the underfull node and the sibling's edge key moves
  up to the parent.
- **Merge** with a sibling otherwise: the *falling separator is re-derived from
  the sibling's subtree minimum* (`subtree_min`) rather than copied from the
  parent — the parent copy may already be stale. The merged node takes all the
  sibling's keys and children, and the sibling's key is removed from the parent.

Whenever a borrow/merge rearranges `children[0]`, the affected parent separators
are refreshed by the `propagate_min` pass, which is what keeps the leftmost edge
of the tree honest.

The root is exempt from minimum occupancy; internal rebalancing therefore only
runs while the node has a parent.

## Iteration and Range Queries

- `iter()` starts at `first_leaf` and walks `next` — `O(n)` and fully sorted.
- `range(lo, hi)` walks `next` from the first leaf whose minimum is `>= lo`,
  stopping when a leaf's minimum exceeds `hi`. Leaves beyond the query range are
  never visited. No internal nodes are visited after descent.

## Validation (`validate`)

Walks the tree asserting:

- every leaf is reachable from the root and depths are uniform;
- parent links are consistent;
- occupancy bounds hold (root exempt);
- internal nodes have no values; leaves have no children;
- **for every internal node, `keys[i] == subtree_min(children[i+1])`** — the
  separator invariant;
- the leaf `next` chain is sorted, matches `first_leaf`, and covers exactly the
  leaves;
- `len` equals the number of key/value pairs in the leaves.

`validate` runs after every mutation in the tests; the regression test
`delete_min_keys_keep_separators_fresh` specifically deletes every even key for
orders 3–6 and validates after each delete — it is what caught the stale
separator bug.

## Testing

Nine unit tests, including a randomized differential test against
`std::collections::BTreeMap` (thousands of random insert/delete/search ops,
checked at every step) and the separator regression test described above.
