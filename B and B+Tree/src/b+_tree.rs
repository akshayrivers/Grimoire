//! B+ Tree implementation in Rust.
//!
//! A B+ Tree is a B-Tree variant designed for databases and file systems:
//!
//! - **Internal nodes** store only *separator* keys used for routing; they
//!   hold no data.
//! - **Leaf nodes** store every `(key, value)` pair, in sorted order.
//! - **Leaves are linked** with a `next` pointer, forming a singly linked
//!   list that makes range scans / full in-order iteration cheap.
//!
//! Invariants of a B+ Tree of order `m`:
//!
//! - every node holds at most `m - 1` keys;
//! - every non-root leaf holds at least `⌈(m - 1)/2⌉` keys;
//! - every non-root internal node holds at least `⌈m/2⌉ - 1` keys;
//! - every separator `keys[i]` of an internal node equals the smallest key
//!   stored in the subtree rooted at `children[i + 1]`;
//! - all leaves sit at the same depth and are chained by `next`.

/// Smallest supported order (see `MIN_ORDER` in `b_tree.rs`).
pub const MIN_ORDER: usize = 3;

#[derive(Debug, Clone)]
struct Node<K, V> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<usize>,
    next: Option<usize>,
    parent: Option<usize>,
}

impl<K, V> Node<K, V> {
    fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn new_leaf() -> Self {
        Node {
            keys: Vec::new(),
            values: Vec::new(),
            children: Vec::new(),
            next: None,
            parent: None,
        }
    }
}

/// A B+ Tree stored in an arena (`Vec<Node>` indexed by `usize`), with a
/// cached pointer to the leftmost leaf for fast iteration and range scans.
///
/// See `b_plus_tree.md` for the rationale behind this design.
pub struct BPlusTree<K, V> {
    nodes: Vec<Node<K, V>>,
    root: Option<usize>,
    first_leaf: Option<usize>,
    order: usize,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> BPlusTree<K, V> {
    /// Creates an empty B+ Tree of the given order. Panics if `order < 3`.
    pub fn new(order: usize) -> Self {
        assert!(order >= MIN_ORDER, "BPlusTree order must be at least {MIN_ORDER}");
        BPlusTree {
            nodes: Vec::new(),
            root: None,
            first_leaf: None,
            order,
            len: 0,
        }
    }

    // ------------------------------------------------------------------
    // Queries
    // ------------------------------------------------------------------

    pub fn order(&self) -> usize {
        self.order
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Height of the tree in edges (`0` for a tree with a single leaf).
    pub fn height(&self) -> usize {
        let mut node = match self.root {
            Some(r) => r,
            None => return 0,
        };
        let mut h = 0;
        while !self.nodes[node].is_leaf() {
            node = self.nodes[node].children[0];
            h += 1;
        }
        h
    }

    pub fn contains(&self, key: &K) -> bool {
        self.search(key).is_some()
    }

    /// Returns the value associated with `key`, if present. The search
    /// always descends to a leaf — internal nodes only carry separators.
    pub fn search(&self, key: &K) -> Option<&V> {
        let mut node = self.root?;
        loop {
            if self.nodes[node].is_leaf() {
                break;
            }
            let j = self.nodes[node].keys.partition_point(|k| key >= k);
            node = self.nodes[node].children[j];
        }
        let i = self.nodes[node].keys.partition_point(|k| key > k);
        if i < self.nodes[node].keys.len() && &self.nodes[node].keys[i] == key {
            Some(&self.nodes[node].values[i])
        } else {
            None
        }
    }

    pub fn min(&self) -> Option<&K> {
        let first = self.first_leaf?;
        self.nodes[first].keys.first()
    }

    pub fn max(&self) -> Option<&K> {
        let mut leaf = self.first_leaf?;
        while let Some(n) = self.nodes[leaf].next {
            leaf = n;
        }
        self.nodes[leaf].keys.last()
    }

    /// Returns every `(key, value)` pair in ascending order by walking the
    /// leaf linked list — no internal traversal needed.
    pub fn iter(&self) -> Vec<(K, V)> {
        let mut out = Vec::with_capacity(self.len);
        let mut leaf = self.first_leaf;
        while let Some(l) = leaf {
            for i in 0..self.nodes[l].keys.len() {
                out.push((
                    self.nodes[l].keys[i].clone(),
                    self.nodes[l].values[i].clone(),
                ));
            }
            leaf = self.nodes[l].next;
        }
        out
    }

    /// Returns all `(key, value)` pairs with `lo <= key <= hi`, found by
    /// locating the leaf for `lo` and walking the leaf chain.
    pub fn range(&self, lo: &K, hi: &K) -> Vec<(K, V)> {
        let mut out = Vec::new();
        let Some(root) = self.root else {
            return out;
        };

        let mut node = root;
        loop {
            if self.nodes[node].is_leaf() {
                break;
            }
            let j = self.nodes[node].keys.partition_point(|k| lo >= k);
            node = self.nodes[node].children[j];
        }

        let mut leaf = Some(node);
        let mut start = self.nodes[node].keys.partition_point(|k| lo > k);
        while let Some(l) = leaf {
            while start < self.nodes[l].keys.len() && &self.nodes[l].keys[start] <= hi {
                out.push((
                    self.nodes[l].keys[start].clone(),
                    self.nodes[l].values[start].clone(),
                ));
                start += 1;
            }
            if start < self.nodes[l].keys.len() {
                break;
            }
            start = 0;
            leaf = self.nodes[l].next;
        }
        out
    }

    // ------------------------------------------------------------------
    // Insertion
    // ------------------------------------------------------------------

    /// Inserts `key`/`value`. If `key` already exists its value is
    /// overwritten (upsert semantics).
    pub fn insert(&mut self, key: K, value: V) {
        let root = match self.root {
            Some(r) => r,
            None => {
                let mut n = Node::new_leaf();
                n.keys.push(key);
                n.values.push(value);
                self.root = Some(self.nodes.len());
                self.first_leaf = self.root;
                self.nodes.push(n);
                self.len = 1;
                return;
            }
        };

        // Route to the leaf that may hold the key.
        let key_ref = &key;
        let mut node = root;
        loop {
            if self.nodes[node].is_leaf() {
                break;
            }
            let j = self.nodes[node].keys.partition_point(|k| key_ref >= k);
            node = self.nodes[node].children[j];
        }

        let i = self.nodes[node].keys.partition_point(|k| key_ref > k);
        if i < self.nodes[node].keys.len() && self.nodes[node].keys[i] == key {
            self.nodes[node].values[i] = value;
            return;
        }

        let min_changed = i == 0;
        self.nodes[node].keys.insert(i, key);
        self.nodes[node].values.insert(i, value);
        self.len += 1;

        // Fix overflow by splitting upward.
        let mut cur = node;
        while self.is_overfull(cur) {
            cur = self.split(cur);
        }

        // If we inserted a new minimum for this leaf, the separators up the
        // tree that mirror this subtree's minimum must be refreshed.
        if min_changed {
            if let Some(min) = self.node_min(node) {
                self.propagate_min(node, min);
            }
        }
    }

    /// Splits an overfull node into two siblings. Leaves and internal nodes
    /// split differently:
    ///
    /// - a **leaf** split keeps the promoted key in the right half (it is
    ///   real data), so the separator is a *copy* that is also pushed up;
    /// - an **internal** split moves the median key up wholesale (it is a
    ///   separator, not data) and the right half takes the keys after it.
    ///
    /// Returns the parent (existing or newly created root).
    fn split(&mut self, idx: usize) -> usize {
        let mid = self.order / 2;
        let parent = self.nodes[idx].parent;
        let right = self.nodes.len();

        if self.nodes[idx].is_leaf() {
            let right_keys: Vec<_> = self.nodes[idx].keys.split_off(mid);
            let right_values: Vec<_> = self.nodes[idx].values.split_off(mid);
            let right_next = self.nodes[idx].next;

            self.nodes.push(Node {
                keys: right_keys,
                values: right_values,
                children: Vec::new(),
                next: right_next,
                parent,
            });
            self.nodes[idx].next = Some(right);
            let promoted = self.nodes[right].keys[0].clone();
            self.insert_separator(parent, idx, right, promoted)
        } else {
            let right_keys: Vec<_> = self.nodes[idx].keys.split_off(mid + 1);
            let right_children: Vec<_> = self.nodes[idx].children.split_off(mid + 1);
            let promoted = self.nodes[idx].keys.pop().unwrap();

            self.nodes.push(Node {
                keys: right_keys,
                values: Vec::new(),
                children: right_children.clone(),
                next: None,
                parent,
            });
            for c in right_children {
                self.nodes[c].parent = Some(right);
            }
            self.insert_separator(parent, idx, right, promoted)
        }
    }

    /// Inserts a separator key into an internal node (or builds a fresh
    /// root), linking the split halves `left` and `right`.
    fn insert_separator(
        &mut self,
        parent: Option<usize>,
        left: usize,
        right: usize,
        key: K,
    ) -> usize {
        match parent {
            Some(p) => {
                let pos = self
                    .nodes[p]
                    .children
                    .iter()
                    .position(|&c| c == left)
                    .unwrap();
                self.nodes[p].keys.insert(pos, key);
                self.nodes[p].children.insert(pos + 1, right);
                p
            }
            None => {
                let new_root = self.nodes.len();
                self.nodes.push(Node {
                    keys: vec![key],
                    values: Vec::new(),
                    children: vec![left, right],
                    next: None,
                    parent: None,
                });
                self.nodes[left].parent = Some(new_root);
                self.nodes[right].parent = Some(new_root);
                self.root = Some(new_root);
                new_root
            }
        }
    }

    /// Walks upward from `node`, refreshing the separator that mirrors its
    /// subtree minimum. `key` is that minimum and is carried up unchanged:
    /// whenever the current node is `children[0]`, the parent's own minimum
    /// equals `key`, so the walk continues.
    fn propagate_min(&mut self, mut node: usize, key: K) {
        loop {
            let parent = match self.nodes[node].parent {
                Some(p) => p,
                None => return,
            };
            let pos = self
                .nodes[parent]
                .children
                .iter()
                .position(|&c| c == node)
                .unwrap();
            if pos == 0 {
                node = parent;
                continue;
            }
            self.nodes[parent].keys[pos - 1] = key;
            return;
        }
    }

    /// The minimum key of the subtree rooted at `node` (`None` only for an
    /// empty leaf).
    fn node_min(&self, node: usize) -> Option<K> {
        if self.nodes[node].is_leaf() {
            return self.nodes[node].keys.first().cloned();
        }
        Some(self.subtree_min(self.nodes[node].children[0]))
    }

    // ------------------------------------------------------------------
    // Deletion
    // ------------------------------------------------------------------

    /// Deletes `key` and returns `true` if it was present.
    pub fn delete(&mut self, key: &K) -> bool {
        let root = match self.root {
            Some(r) => r,
            None => return false,
        };

        // Route to the leaf that may hold the key.
        let mut node = root;
        loop {
            if self.nodes[node].is_leaf() {
                break;
            }
            let j = self.nodes[node].keys.partition_point(|k| key >= k);
            node = self.nodes[node].children[j];
        }

        let i = self.nodes[node].keys.partition_point(|k| key > k);
        if i == self.nodes[node].keys.len() || &self.nodes[node].keys[i] != key {
            return false;
        }

        self.nodes[node].keys.remove(i);
        self.nodes[node].values.remove(i);
        self.len -= 1;

        // If the removed key was the leaf's minimum, its subtree's minimum
        // changed. Refresh the affected separators *before* any rebalancing
        // so merges never copy stale separators downward.
        if i == 0 {
            if let Some(min) = self.node_min(node) {
                self.propagate_min(node, min);
            }
        }

        // Fix underflow from the modified leaf upward. The root is exempt
        // from the minimum-occupancy rules.
        let mut cur = node;
        while self.nodes[cur].parent.is_some() && self.is_underfull(cur) {
            cur = self.rebalance(cur);
        }

        // A merge of a node in the leftmost (children[0]) position can
        // silently change an ancestor's minimum; refresh once more from the
        // final rebalanced node.
        if let Some(min) = self.node_min(cur) {
            self.propagate_min(cur, min);
        }

        self.fix_root();
        true
    }

    /// Repairs an underfull node by borrowing from or merging with a
    /// sibling. Returns the parent, which may itself now underflow.
    fn rebalance(&mut self, idx: usize) -> usize {
        let parent = self.nodes[idx].parent.unwrap();
        let pos = self
            .nodes[parent]
            .children
            .iter()
            .position(|&c| c == idx)
            .unwrap();

        if self.nodes[idx].is_leaf() {
            self.rebalance_leaf(idx, parent, pos)
        } else {
            self.rebalance_internal(idx, parent, pos)
        }
    }

    fn rebalance_leaf(&mut self, idx: usize, parent: usize, pos: usize) -> usize {
        let min = self.min_leaf_keys();
        if pos > 0 {
            let left = self.nodes[parent].children[pos - 1];
            if self.nodes[left].keys.len() > min {
                self.leaf_borrow_from_left(idx, left, parent, pos);
                return parent;
            }
        }
        if pos + 1 < self.nodes[parent].children.len() {
            let right = self.nodes[parent].children[pos + 1];
            if self.nodes[right].keys.len() > min {
                self.leaf_borrow_from_right(idx, right, parent, pos);
                return parent;
            }
        }
        if pos > 0 {
            let left = self.nodes[parent].children[pos - 1];
            self.leaf_merge_into_left(idx, left, parent, pos);
        } else {
            let right = self.nodes[parent].children[pos + 1];
            self.leaf_merge_right_into(idx, right, parent, pos);
        }
        parent
    }

    fn rebalance_internal(&mut self, idx: usize, parent: usize, pos: usize) -> usize {
        let min = self.min_internal_keys();
        if pos > 0 {
            let left = self.nodes[parent].children[pos - 1];
            if self.nodes[left].keys.len() > min {
                self.internal_borrow_from_left(idx, left, parent, pos);
                return parent;
            }
        }
        if pos + 1 < self.nodes[parent].children.len() {
            let right = self.nodes[parent].children[pos + 1];
            if self.nodes[right].keys.len() > min {
                self.internal_borrow_from_right(idx, right, parent, pos);
                return parent;
            }
        }
        if pos > 0 {
            let left = self.nodes[parent].children[pos - 1];
            self.internal_merge_into_left(idx, left, parent, pos);
        } else {
            let right = self.nodes[parent].children[pos + 1];
            self.internal_merge_right_into(idx, right, parent, pos);
        }
        parent
    }

    fn leaf_borrow_from_left(&mut self, idx: usize, left: usize, parent: usize, pos: usize) {
        let k = self.nodes[left].keys.pop().unwrap();
        let v = self.nodes[left].values.pop().unwrap();
        self.nodes[idx].keys.insert(0, k);
        self.nodes[idx].values.insert(0, v);
        self.nodes[parent].keys[pos - 1] = self.nodes[idx].keys[0].clone();
    }

    fn leaf_borrow_from_right(&mut self, idx: usize, right: usize, parent: usize, pos: usize) {
        let k = self.nodes[right].keys.remove(0);
        let v = self.nodes[right].values.remove(0);
        self.nodes[idx].keys.push(k);
        self.nodes[idx].values.push(v);
        self.nodes[parent].keys[pos] = self.nodes[right].keys[0].clone();
        // If the borrowing leaf lost its minimum (or was empty), the
        // separator mirroring it must also be refreshed.
        if pos > 0 {
            self.nodes[parent].keys[pos - 1] = self.nodes[idx].keys[0].clone();
        }
    }

    fn leaf_merge_into_left(&mut self, idx: usize, left: usize, parent: usize, pos: usize) {
        let mut idx_keys = std::mem::take(&mut self.nodes[idx].keys);
        let mut idx_values = std::mem::take(&mut self.nodes[idx].values);
        let idx_next = self.nodes[idx].next;

        self.nodes[left].keys.append(&mut idx_keys);
        self.nodes[left].values.append(&mut idx_values);
        self.nodes[left].next = idx_next;

        self.nodes[parent].children.remove(pos);
        self.nodes[parent].keys.remove(pos - 1);
    }

    fn leaf_merge_right_into(&mut self, idx: usize, right: usize, parent: usize, pos: usize) {
        let mut r_keys = std::mem::take(&mut self.nodes[right].keys);
        let mut r_values = std::mem::take(&mut self.nodes[right].values);
        let r_next = self.nodes[right].next;

        self.nodes[idx].keys.append(&mut r_keys);
        self.nodes[idx].values.append(&mut r_values);
        self.nodes[idx].next = r_next;

        self.nodes[parent].children.remove(pos + 1);
        self.nodes[parent].keys.remove(pos);
    }

    fn internal_borrow_from_left(&mut self, idx: usize, left: usize, parent: usize, pos: usize) {
        // Node gains left's last child at the front. Its new first separator
        // must mirror the *old* children[0] subtree, which we re-derive;
        // left's last key moves up into the parent, mirroring the moved
        // child (node's new children[0]).
        let sep = self.subtree_min(idx);
        let moved_key = self.nodes[left].keys.pop().unwrap();
        let moved_child = self.nodes[left].children.pop().unwrap();

        self.nodes[idx].keys.insert(0, sep);
        self.nodes[idx].children.insert(0, moved_child);
        self.nodes[moved_child].parent = Some(idx);
        self.nodes[parent].keys[pos - 1] = moved_key;
    }

    fn internal_borrow_from_right(&mut self, idx: usize, right: usize, parent: usize, pos: usize) {
        // The separator (minimum of right's leftmost subtree) moves down as
        // node's last key; right's first separator moves up into the parent.
        // The separator is re-derived from the subtree so a stale copy is
        // never rotated downward.
        let sep = self.subtree_min(right);
        let moved_child = self.nodes[right].children.remove(0);

        self.nodes[idx].keys.push(sep);
        self.nodes[parent].keys[pos] = self.nodes[right].keys.remove(0);
        self.nodes[idx].children.push(moved_child);
        self.nodes[moved_child].parent = Some(idx);
    }

    fn internal_merge_into_left(&mut self, idx: usize, left: usize, parent: usize, pos: usize) {
        // The separator that falls between the two halves is re-derived from
        // the node being absorbed, so it mirrors the current subtree minimum.
        let sep = self.subtree_min(idx);
        self.nodes[parent].keys.remove(pos - 1);
        let mut idx_keys = std::mem::take(&mut self.nodes[idx].keys);
        let mut idx_children = std::mem::take(&mut self.nodes[idx].children);

        self.nodes[left].keys.push(sep);
        self.nodes[left].keys.append(&mut idx_keys);
        for &c in &idx_children {
            self.nodes[c].parent = Some(left);
        }
        self.nodes[left].children.append(&mut idx_children);

        self.nodes[parent].children.remove(pos);
    }

    fn internal_merge_right_into(&mut self, idx: usize, right: usize, parent: usize, pos: usize) {
        // Same as above: re-derive the falling separator from the sibling
        // being absorbed so it matches the current subtree minimum.
        let sep = self.subtree_min(right);
        self.nodes[parent].keys.remove(pos);
        let mut r_keys = std::mem::take(&mut self.nodes[right].keys);
        let mut r_children = std::mem::take(&mut self.nodes[right].children);

        self.nodes[idx].keys.push(sep);
        self.nodes[idx].keys.append(&mut r_keys);
        for &c in &r_children {
            self.nodes[c].parent = Some(idx);
        }
        self.nodes[idx].children.append(&mut r_children);

        self.nodes[parent].children.remove(pos + 1);
    }

    /// If the root lost its last key, either shrink the tree by one level or
    /// mark it empty.
    fn fix_root(&mut self) {
        let root = match self.root {
            Some(r) => r,
            None => return,
        };
        if self.nodes[root].is_leaf() {
            if self.nodes[root].keys.is_empty() {
                self.root = None;
                self.first_leaf = None;
            }
        } else if self.nodes[root].keys.is_empty() {
            debug_assert_eq!(self.nodes[root].children.len(), 1);
            let new_root = self.nodes[root].children[0];
            self.nodes[new_root].parent = None;
            self.root = Some(new_root);
        }
    }

    // ------------------------------------------------------------------
    // Capacity helpers
    // ------------------------------------------------------------------

    fn max_keys(&self) -> usize {
        self.order - 1
    }

    fn min_leaf_keys(&self) -> usize {
        self.order / 2
    }

    fn min_internal_keys(&self) -> usize {
        self.order.div_ceil(2) - 1
    }

    fn is_overfull(&self, idx: usize) -> bool {
        self.nodes[idx].keys.len() > self.max_keys()
    }

    fn is_underfull(&self, idx: usize) -> bool {
        let n = &self.nodes[idx];
        if n.is_leaf() {
            n.keys.len() < self.min_leaf_keys()
        } else {
            n.keys.len() < self.min_internal_keys()
        }
    }

    // ------------------------------------------------------------------
    // Debugging & validation
    // ------------------------------------------------------------------

    /// Prints the tree shape (one line per node, indented by depth).
    pub fn print_tree(&self)
    where
        K: std::fmt::Display,
        V: std::fmt::Display,
    {
        fn walk<K: std::fmt::Display, V: std::fmt::Display>(
            tree: &BPlusTree<K, V>,
            idx: usize,
            indent: usize,
        ) {
            let node = &tree.nodes[idx];
            let label = if node.is_leaf() { "leaf" } else { "int " };
            let keys: Vec<String> = node.keys.iter().map(|k| k.to_string()).collect();
            println!("{indent}node#{idx} {label}[{}]", keys.join(", "));
            for &c in &node.children {
                walk(tree, c, indent + 2);
            }
        }
        match self.root {
            Some(r) => walk(self, r, 0),
            None => println!("(empty)"),
        }
    }

    /// Verifies every structural invariant of the B+ Tree. Returns `Ok(())`
    /// when the tree is consistent, otherwise a description of the failure.
    pub fn validate(&self) -> Result<(), String> {
        let Some(root) = self.root else {
            return Ok(());
        };

        // `first_leaf` must be the leftmost leaf.
        let mut leftmost = root;
        while !self.nodes[leftmost].is_leaf() {
            leftmost = self.nodes[leftmost].children[0];
        }
        if self.first_leaf != Some(leftmost) {
            return Err("first_leaf is not the leftmost leaf".into());
        }

        let mut leaf_depth = None;
        self.validate_node(root, 0, &mut leaf_depth)?;

        // The leaf chain must be sorted and contain every key.
        let mut count = 0;
        let mut prev: Option<K> = None;
        let mut leaf = self.first_leaf;
        while let Some(l) = leaf {
            for k in &self.nodes[l].keys {
                if let Some(p) = &prev {
                    if p >= k {
                        return Err("keys not strictly increasing across leaves".into());
                    }
                }
                prev = Some(k.clone());
                count += 1;
            }
            leaf = self.nodes[l].next;
        }
        if count != self.len {
            return Err(format!(
                "len mismatch: leaf chain holds {count}, tree claims {}",
                self.len
            ));
        }
        Ok(())
    }

    fn validate_node(
        &self,
        idx: usize,
        depth: usize,
        leaf_depth: &mut Option<usize>,
    ) -> Result<(), String> {
        let n = &self.nodes[idx];
        let label = format!("node #{idx}");

        if n.keys.len() > self.max_keys() {
            return Err(format!("{label} overfull: {} > {}", n.keys.len(), self.max_keys()));
        }
        if n.parent.is_some() {
            if n.is_leaf() && n.keys.len() < self.min_leaf_keys() {
                return Err(format!(
                    "{label} leaf underfull: {} < {}",
                    n.keys.len(),
                    self.min_leaf_keys()
                ));
            }
            if !n.is_leaf() && n.keys.len() < self.min_internal_keys() {
                return Err(format!(
                    "{label} internal underfull: {} < {}",
                    n.keys.len(),
                    self.min_internal_keys()
                ));
            }
        }
        for w in n.keys.windows(2) {
            if w[0] >= w[1] {
                return Err(format!("{label} keys not strictly sorted"));
            }
        }

        if n.is_leaf() {
            if n.keys.len() != n.values.len() {
                return Err(format!("{label} key/value count mismatch"));
            }
            if let Some(nn) = n.next {
                if !self.nodes[nn].is_leaf() {
                    return Err(format!("{label} next pointer does not lead to a leaf"));
                }
            }
            match leaf_depth {
                Some(d) if *d != depth => {
                    return Err(format!("leaf at depth {depth}, expected {d}"));
                }
                None => *leaf_depth = Some(depth),
                _ => {}
            }
        } else {
            if n.children.len() != n.keys.len() + 1 {
                return Err(format!(
                    "{label} has {} children for {} keys",
                    n.children.len(),
                    n.keys.len()
                ));
            }
            if !n.values.is_empty() {
                return Err(format!("{label} internal node must not hold values"));
            }
            // Every separator must equal the minimum key of the subtree to
            // its right.
            for i in 0..n.keys.len() {
                let min = self.subtree_min(n.children[i + 1]);
                if n.keys[i] != min {
                    return Err(format!(
                        "{label} separator at index {i} does not match the minimum of its right child subtree"
                    ));
                }
            }
            for &c in &n.children {
                if self.nodes[c].parent != Some(idx) {
                    return Err(format!("{label} child {c} has wrong parent"));
                }
                self.validate_node(c, depth + 1, leaf_depth)?;
            }
        }
        Ok(())
    }

    fn subtree_min(&self, mut node: usize) -> K {
        while !self.nodes[node].is_leaf() {
            node = self.nodes[node].children[0];
        }
        self.nodes[node].keys[0].clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_with_inserts(order: usize, keys: &[i32]) -> BPlusTree<i32, i32> {
        let mut t = BPlusTree::new(order);
        for &k in keys {
            t.insert(k, k * 100);
        }
        t
    }

    #[test]
    fn order_must_be_at_least_three() {
        assert!(std::panic::catch_unwind(|| BPlusTree::<i32, i32>::new(2)).is_err());
    }

    #[test]
    fn insert_search_and_upsert() {
        let mut t = BPlusTree::new(3);
        for i in 0..100 {
            t.insert(i, i);
        }
        assert_eq!(t.len(), 100);
        for i in 0..100 {
            assert_eq!(t.search(&i), Some(&i));
        }
        assert_eq!(t.search(&100), None);
        t.insert(50, -1);
        assert_eq!(t.search(&50), Some(&-1));
        assert_eq!(t.len(), 100);
        t.validate().unwrap();
    }

    #[test]
    fn iter_is_sorted_and_matches_chain() {
        for order in 3..=6 {
            let t = build_with_inserts(order, &[5, 3, 8, 1, 9, 2, 7, 4, 6, 0]);
            let keys: Vec<_> = t.iter().into_iter().map(|(k, _)| k).collect();
            assert_eq!(keys, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9], "order {order}");
            t.validate().unwrap();
        }
    }

    #[test]
    fn range_query_uses_leaf_chain() {
        let mut t = BPlusTree::new(4);
        for i in (0..50).step_by(2) {
            t.insert(i, i);
        }
        let got: Vec<_> = t.range(&10, &20).into_iter().map(|(k, _)| k).collect();
        assert_eq!(got, vec![10, 12, 14, 16, 18, 20]);
        assert!(t.range(&100, &200).is_empty());
        assert!(t.range(&30, &5).is_empty());
        t.validate().unwrap();
    }

    #[test]
    fn all_keys_are_in_leaves() {
        let t = build_with_inserts(3, &[1, 2, 3, 4, 5, 6, 7, 8, 9]);
        // A B+ tree of order 3 must grow an internal level, so verify every
        // key lives in a leaf (reached via the chain).
        assert!(t.height() >= 1);
        let chained: Vec<_> = t.iter().into_iter().map(|(k, _)| k).collect();
        assert_eq!(chained, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
        t.validate().unwrap();
    }

    #[test]
    fn delete_min_keys_keep_separators_fresh() {
        // Regression: deleting the minimum key of a leaf used to leave stale
        // separators that were then copied downward during merges.
        for order in 3..=6 {
            let mut t = BPlusTree::new(order);
            for i in 0..=15 {
                t.insert(i, i);
            }
            for i in (0..=14).step_by(2) {
                assert!(t.delete(&i), "order {order}, delete {i}");
                t.validate().unwrap_or_else(|e| {
                    panic!("order {order} after delete {i}: {e}")
                });
            }
            assert_eq!(t.iter().into_iter().map(|(k, _)| k).collect::<Vec<_>>(),
                       vec![1, 3, 5, 7, 9, 11, 13, 15]);
        }
    }

    #[test]
    fn delete_all_keys_leaves_empty_tree() {
        for order in 3..=7 {
            let mut t = build_with_inserts(order, &[5, 3, 8, 1, 9, 2, 7, 4, 6, 0]);
            for i in 0..10 {
                assert!(t.delete(&i), "order {order}, key {i}");
            }
            assert!(t.is_empty());
            assert_eq!(t.root, None);
            assert_eq!(t.first_leaf, None);
            t.validate().unwrap();
        }
    }

    #[test]
    fn delete_missing_key_returns_false() {
        let mut t = build_with_inserts(3, &[5, 3, 8]);
        assert!(!t.delete(&10));
        assert_eq!(t.len(), 3);
    }

    #[test]
    fn min_key_updates_separators() {
        let mut t = build_with_inserts(4, &[10, 20, 30]);
        // Inserting new minimums forces separator refreshes up the tree.
        for k in (0..10).rev() {
            t.insert(k, k);
        }
        assert_eq!(t.min(), Some(&0));
        assert_eq!(t.search(&0), Some(&0));
        t.validate().unwrap();
    }

    #[test]
    fn random_ops_match_std_btreemap() {
        use std::collections::BTreeMap;
        let mut tree = BPlusTree::new(4);
        let mut model = BTreeMap::new();
        for seed in 0..10_000u64 {
            let op = seed % 4;
            let key = ((seed * 2654435761) % 200) as i32;
            match op {
                0 | 1 => {
                    tree.insert(key, key);
                    model.insert(key, key);
                }
                2 => {
                    let a = tree.delete(&key);
                    let b = model.remove(&key).is_some();
                    assert_eq!(a, b);
                }
                _ => {
                    let a = tree.search(&key);
                    let b = model.get(&key);
                    assert_eq!(a, b);
                }
            }
            if seed % 500 == 0 {
                tree.validate().unwrap();
            }
        }
        assert_eq!(tree.len(), model.len());
        let got: Vec<_> = tree.iter().into_iter().map(|(k, _)| k).collect();
        let want: Vec<_> = model.keys().cloned().collect();
        assert_eq!(got, want);
        tree.validate().unwrap();
    }

    #[test]
    fn min_max_height() {
        let mut t = BPlusTree::new(3);
        assert_eq!(t.min(), None);
        assert_eq!(t.max(), None);
        for i in 0..50 {
            t.insert(i, i);
        }
        assert_eq!(t.min(), Some(&0));
        assert_eq!(t.max(), Some(&49));
        assert!(t.height() >= 2);
    }
}
