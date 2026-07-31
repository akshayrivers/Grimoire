//! B-Tree implementation in Rust.
//!
//! A B-Tree is a self-balancing search tree that keeps data sorted and
//! supports search / insert / delete in `O(log n)`. Unlike a B+ Tree, keys
//! (and their associated values) are stored in *every* node — internal
//! nodes carry real data, not just routing separators.
//!
//! # Terminology
//!
//! A B-Tree of *order* `m` satisfies:
//!
//! - every node holds at most `m - 1` keys and at most `m` children;
//! - every non-root node holds at least `⌈m/2⌉ - 1` keys;
//! - the root holds at least `1` key unless the tree is empty;
//! - all leaves sit at the same depth (perfect height balance);
//! - keys within a node are sorted ascending and are unique.

/// Smallest supported order. Smaller orders would allow nodes with zero
/// minimum keys, which makes the "minimum occupancy" rules degenerate.
pub const MIN_ORDER: usize = 3;

#[derive(Debug, Clone)]
struct Node<K, V> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<usize>,
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
            parent: None,
        }
    }
}

/// A B-Tree stored in an *arena*: a flat `Vec<Node>` where nodes are
/// referenced by `usize` indices instead of owning `Box`/`Rc` pointers.
///
/// See `b_tree.md` for the rationale behind this design.
pub struct BTree<K, V> {
    nodes: Vec<Node<K, V>>,
    root: Option<usize>,
    order: usize,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> BTree<K, V> {
    /// Creates an empty B-Tree of the given order. Panics if `order < 3`.
    pub fn new(order: usize) -> Self {
        assert!(order >= MIN_ORDER, "BTree order must be at least {MIN_ORDER}");
        BTree {
            nodes: Vec::new(),
            root: None,
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

    /// Returns the value associated with `key`, if present.
    pub fn search(&self, key: &K) -> Option<&V> {
        let mut node = self.root?;
        loop {
            let i = self.nodes[node].keys.partition_point(|k| k < key);
            if i < self.nodes[node].keys.len() && &self.nodes[node].keys[i] == key {
                return Some(&self.nodes[node].values[i]);
            }
            if self.nodes[node].is_leaf() {
                return None;
            }
            node = self.nodes[node].children[i];
        }
    }

    pub fn min(&self) -> Option<&K> {
        let mut node = self.root?;
        while !self.nodes[node].is_leaf() {
            node = self.nodes[node].children[0];
        }
        self.nodes[node].keys.first()
    }

    pub fn max(&self) -> Option<&K> {
        let mut node = self.root?;
        while !self.nodes[node].is_leaf() {
            node = *self.nodes[node].children.last()?;
        }
        self.nodes[node].keys.last()
    }

    /// Returns all `(key, value)` pairs in ascending key order.
    pub fn iter(&self) -> Vec<(K, V)> {
        let mut out = Vec::with_capacity(self.len);
        if let Some(root) = self.root {
            self.collect_in_range(root, None, None, &mut out);
        }
        out
    }

    /// Returns all `(key, value)` pairs with `lo <= key <= hi`.
    pub fn range(&self, lo: &K, hi: &K) -> Vec<(K, V)> {
        let mut out = Vec::new();
        if let Some(root) = self.root {
            self.collect_in_range(root, Some(lo), Some(hi), &mut out);
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
                self.nodes.push(n);
                self.len = 1;
                return;
            }
        };

        // Descend to the leaf where the key belongs.
        let mut node = root;
        loop {
            let i = self.nodes[node].keys.partition_point(|k| k < &key);
            if i < self.nodes[node].keys.len() && self.nodes[node].keys[i] == key {
                self.nodes[node].values[i] = value;
                return;
            }
            if self.nodes[node].is_leaf() {
                self.nodes[node].keys.insert(i, key);
                self.nodes[node].values.insert(i, value);
                break;
            }
            node = self.nodes[node].children[i];
        }
        self.len += 1;

        // Fix overflow from the modified leaf upward.
        while self.is_overfull(node) {
            node = self.split(node);
        }
    }

    /// Splits an overfull node (exactly `order` keys) into two siblings and
    /// promotes the median key into the parent. Returns the parent index —
    /// either the existing parent or a freshly created root.
    fn split(&mut self, idx: usize) -> usize {
        let mid = self.order / 2;

        // Promote the median key/value to the parent.
        let promoted_key = self.nodes[idx].keys[mid].clone();
        let promoted_value = self.nodes[idx].values[mid].clone();

        // The right sibling takes everything strictly after the median.
        let right_keys: Vec<_> = self.nodes[idx].keys.drain(mid + 1..).collect();
        let right_values: Vec<_> = self.nodes[idx].values.drain(mid + 1..).collect();
        let right_children: Vec<_> = if self.nodes[idx].is_leaf() {
            Vec::new()
        } else {
            self.nodes[idx].children.drain(mid + 1..).collect()
        };

        // Drop the promoted key from the left half.
        self.nodes[idx].keys.remove(mid);
        self.nodes[idx].values.remove(mid);

        let parent = self.nodes[idx].parent;
        let right = self.nodes.len();
        self.nodes.push(Node {
            keys: right_keys,
            values: right_values,
            children: right_children,
            parent,
        });
        for c in self.nodes[right].children.clone() {
            self.nodes[c].parent = Some(right);
        }

        match parent {
            Some(p) => {
                let pos = self.nodes[p].keys.partition_point(|k| k < &promoted_key);
                self.nodes[p].keys.insert(pos, promoted_key);
                self.nodes[p].values.insert(pos, promoted_value);
                self.nodes[p].children.insert(pos + 1, right);
                p
            }
            None => {
                let new_root = self.nodes.len();
                self.nodes.push(Node {
                    keys: vec![promoted_key],
                    values: vec![promoted_value],
                    children: vec![idx, right],
                    parent: None,
                });
                self.nodes[idx].parent = Some(new_root);
                self.nodes[right].parent = Some(new_root);
                self.root = Some(new_root);
                new_root
            }
        }
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

        let (mut node, pos) = match self.find_key(root, key) {
            Some(x) => x,
            None => return false,
        };

        self.len -= 1;

        if !self.nodes[node].is_leaf() {
            // Classic trick: replace the internal key with its in-order
            // successor (the minimum key of the right subtree), then delete
            // that successor from its leaf. This guarantees the key that is
            // physically removed always lives in a leaf.
            let mut succ = self.nodes[node].children[pos + 1];
            while !self.nodes[succ].is_leaf() {
                succ = self.nodes[succ].children[0];
            }
            let s_key = self.nodes[succ].keys.remove(0);
            let s_value = self.nodes[succ].values.remove(0);
            self.nodes[node].keys[pos] = s_key;
            self.nodes[node].values[pos] = s_value;
            node = succ;
        } else {
            self.nodes[node].keys.remove(pos);
            self.nodes[node].values.remove(pos);
        }

        // Fix underflow from the modified leaf upward. The root is exempt
        // from the minimum-occupancy rule.
        while self.nodes[node].parent.is_some() && self.is_underfull(node) {
            node = self.rebalance(node);
        }

        self.shrink_root_if_needed();
        true
    }

    fn find_key(&self, start: usize, key: &K) -> Option<(usize, usize)> {
        let mut node = start;
        loop {
            let i = self.nodes[node].keys.partition_point(|k| k < key);
            if i < self.nodes[node].keys.len() && &self.nodes[node].keys[i] == key {
                return Some((node, i));
            }
            if self.nodes[node].is_leaf() {
                return None;
            }
            node = self.nodes[node].children[i];
        }
    }

    /// Repairs an underfull node by borrowing a key from a sibling or by
    /// merging with one. Returns the parent, which may itself now underflow.
    fn rebalance(&mut self, idx: usize) -> usize {
        let parent = self.nodes[idx].parent.unwrap();
        let pos = self
            .nodes[parent]
            .children
            .iter()
            .position(|&c| c == idx)
            .unwrap();
        let min_keys = self.min_keys();

        // Try to borrow from the left sibling first.
        if pos > 0 {
            let left = self.nodes[parent].children[pos - 1];
            if self.nodes[left].keys.len() > min_keys {
                self.borrow_from_left(idx, left, parent, pos);
                return parent;
            }
        }
        // Then from the right sibling.
        if pos + 1 < self.nodes[parent].children.len() {
            let right = self.nodes[parent].children[pos + 1];
            if self.nodes[right].keys.len() > min_keys {
                self.borrow_from_right(idx, right, parent, pos);
                return parent;
            }
        }
        // Otherwise merge. Prefer merging into the left sibling.
        if pos > 0 {
            let left = self.nodes[parent].children[pos - 1];
            self.merge_into_left(idx, left, parent, pos);
        } else {
            let right = self.nodes[parent].children[pos + 1];
            self.merge_right_into(idx, right, parent, pos);
        }
        parent
    }

    /// Rotates the separator key down from the parent and the left sibling's
    /// last key up into the parent.
    fn borrow_from_left(&mut self, idx: usize, left: usize, parent: usize, pos: usize) {
        let sep = self.nodes[parent].keys[pos - 1].clone();
        let sep_value = self.nodes[parent].values[pos - 1].clone();

        let l_key = self.nodes[left].keys.pop().unwrap();
        let l_value = self.nodes[left].values.pop().unwrap();

        self.nodes[parent].keys[pos - 1] = l_key;
        self.nodes[parent].values[pos - 1] = l_value;

        self.nodes[idx].keys.insert(0, sep);
        self.nodes[idx].values.insert(0, sep_value);

        if !self.nodes[idx].is_leaf() {
            let c = self.nodes[left].children.pop().unwrap();
            self.nodes[c].parent = Some(idx);
            self.nodes[idx].children.insert(0, c);
        }
    }

    /// Rotates the separator key down from the parent and the right
    /// sibling's first key up into the parent.
    fn borrow_from_right(&mut self, idx: usize, right: usize, parent: usize, pos: usize) {
        let sep = self.nodes[parent].keys[pos].clone();
        let sep_value = self.nodes[parent].values[pos].clone();

        let r_key = self.nodes[right].keys.remove(0);
        let r_value = self.nodes[right].values.remove(0);

        self.nodes[parent].keys[pos] = r_key;
        self.nodes[parent].values[pos] = r_value;

        self.nodes[idx].keys.push(sep);
        self.nodes[idx].values.push(sep_value);

        if !self.nodes[idx].is_leaf() {
            let c = self.nodes[right].children.remove(0);
            self.nodes[c].parent = Some(idx);
            self.nodes[idx].children.push(c);
        }
    }

    /// Merges `idx` into its left sibling, pulling the parent separator
    /// down between them.
    fn merge_into_left(&mut self, idx: usize, left: usize, parent: usize, pos: usize) {
        let sep = self.nodes[parent].keys.remove(pos - 1);
        let sep_value = self.nodes[parent].values.remove(pos - 1);

        let mut idx_keys = std::mem::take(&mut self.nodes[idx].keys);
        let mut idx_values = std::mem::take(&mut self.nodes[idx].values);
        let mut idx_children = std::mem::take(&mut self.nodes[idx].children);

        self.nodes[left].keys.push(sep);
        self.nodes[left].values.push(sep_value);
        self.nodes[left].keys.append(&mut idx_keys);
        self.nodes[left].values.append(&mut idx_values);
        for &c in &idx_children {
            self.nodes[c].parent = Some(left);
        }
        self.nodes[left].children.append(&mut idx_children);

        self.nodes[parent].children.remove(pos);
    }

    /// Merges the right sibling into `idx`, pulling the parent separator
    /// down between them.
    fn merge_right_into(&mut self, idx: usize, right: usize, parent: usize, pos: usize) {
        let sep = self.nodes[parent].keys.remove(pos);
        let sep_value = self.nodes[parent].values.remove(pos);

        let mut r_keys = std::mem::take(&mut self.nodes[right].keys);
        let mut r_values = std::mem::take(&mut self.nodes[right].values);
        let mut r_children = std::mem::take(&mut self.nodes[right].children);

        self.nodes[idx].keys.push(sep);
        self.nodes[idx].values.push(sep_value);
        self.nodes[idx].keys.append(&mut r_keys);
        self.nodes[idx].values.append(&mut r_values);
        for &c in &r_children {
            self.nodes[c].parent = Some(idx);
        }
        self.nodes[idx].children.append(&mut r_children);

        self.nodes[parent].children.remove(pos + 1);
    }

    /// If the root lost its last key, either shrink the tree by one level or
    /// mark it empty.
    fn shrink_root_if_needed(&mut self) {
        let root = match self.root {
            Some(r) => r,
            None => return,
        };
        if self.nodes[root].is_leaf() {
            if self.nodes[root].keys.is_empty() {
                self.root = None;
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

    fn min_keys(&self) -> usize {
        self.order.div_ceil(2) - 1
    }

    fn is_overfull(&self, idx: usize) -> bool {
        self.nodes[idx].keys.len() > self.max_keys()
    }

    fn is_underfull(&self, idx: usize) -> bool {
        self.nodes[idx].keys.len() < self.min_keys()
    }

    // ------------------------------------------------------------------
    // Traversal helpers
    // ------------------------------------------------------------------

    fn collect_in_range(
        &self,
        node: usize,
        lo: Option<&K>,
        hi: Option<&K>,
        out: &mut Vec<(K, V)>,
    ) {
        let is_leaf = self.nodes[node].is_leaf();
        for i in 0..self.nodes[node].keys.len() {
            if !is_leaf {
                self.collect_in_range(self.nodes[node].children[i], lo, hi, out);
            }
            let key = &self.nodes[node].keys[i];
            let lo_ok = lo.is_none_or(|lo| lo <= key);
            let hi_ok = hi.is_none_or(|hi| key <= hi);
            if lo_ok && hi_ok {
                out.push((key.clone(), self.nodes[node].values[i].clone()));
            }
        }
        if !is_leaf {
            let last = self.nodes[node].children.len() - 1;
            self.collect_in_range(self.nodes[node].children[last], lo, hi, out);
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
            tree: &BTree<K, V>,
            idx: usize,
            indent: usize,
        ) {
            let node = &tree.nodes[idx];
            let label = if node.is_leaf() { "leaf" } else { "int " };
            let keys: Vec<String> = node.keys.iter().map(|k| k.to_string()).collect();
            println!("{indent}{label} [{}]", keys.join(", "));
            for &c in &node.children {
                walk(tree, c, indent + 2);
            }
        }
        match self.root {
            Some(r) => walk(self, r, 0),
            None => println!("(empty)"),
        }
    }

    /// Verifies every structural invariant of the B-Tree. Returns `Ok(())`
    /// when the tree is consistent, otherwise a description of the failure.
    pub fn validate(&self) -> Result<(), String> {
        let Some(root) = self.root else { return Ok(()) };
        let mut leaf_depth = None;
        self.validate_node(root, 0, &mut leaf_depth)
    }

    fn validate_node(
        &self,
        idx: usize,
        depth: usize,
        leaf_depth: &mut Option<usize>,
    ) -> Result<(), String> {
        let n = &self.nodes[idx];
        let node_label = |pos: usize| format!("node #{idx} (pos {pos})");

        if n.keys.len() > self.max_keys() {
            return Err(format!("{} overfull: {} > {}", node_label(depth), n.keys.len(), self.max_keys()));
        }
        if n.parent.is_some() && n.keys.len() < self.min_keys() {
            return Err(format!(
                "{} underfull: {} < {}",
                node_label(depth),
                n.keys.len(),
                self.min_keys()
            ));
        }
        for w in n.keys.windows(2) {
            if w[0] >= w[1] {
                return Err(format!("{} keys not strictly sorted", node_label(depth)));
            }
        }

        if n.is_leaf() {
            if n.keys.len() != n.values.len() {
                return Err(format!("{} key/value count mismatch", node_label(depth)));
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
                    "{} has {} children for {} keys",
                    node_label(depth),
                    n.children.len(),
                    n.keys.len()
                ));
            }
            for &c in &n.children {
                if self.nodes[c].parent != Some(idx) {
                    return Err(format!("{} child {c} has wrong parent", node_label(depth)));
                }
                self.validate_node(c, depth + 1, leaf_depth)?;
            }
        }
        Ok(())
    }
}

    #[cfg(test)]
    mod tests {
        use super::*;

        fn build_with_inserts(order: usize, keys: &[i32]) -> BTree<i32, i32> {
        let mut t = BTree::new(order);
        for &k in keys {
            t.insert(k, k * 100);
        }
        t
    }

    #[test]
    fn order_must_be_at_least_three() {
        assert!(std::panic::catch_unwind(|| BTree::<i32, i32>::new(2)).is_err());
    }

    #[test]
    fn insert_search_and_upsert() {
        let mut t = BTree::new(3);
        assert!(t.is_empty());
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
    fn iter_returns_sorted_unique_keys() {
        for order in 3..=6 {
            let t = build_with_inserts(order, &[5, 3, 8, 1, 9, 2, 7, 4, 6, 0]);
            let keys: Vec<_> = t.iter().into_iter().map(|(k, _)| k).collect();
            assert_eq!(keys, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9], "order {order}");
            t.validate().unwrap();
        }
    }

    #[test]
    fn range_query() {
        let mut t = BTree::new(4);
        for i in (0..50).step_by(2) {
            t.insert(i, i);
        }
        let got: Vec<_> = t.range(&10, &20).into_iter().map(|(k, _)| k).collect();
        assert_eq!(got, vec![10, 12, 14, 16, 18, 20]);
        assert!(t.range(&100, &200).is_empty());
        t.validate().unwrap();
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
    fn random_ops_match_std_btreemap() {
        use std::collections::BTreeMap;
        let mut tree = BTree::new(4);
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
        let mut t = BTree::new(3);
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
