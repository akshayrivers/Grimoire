pub const M: usize = 32; // order = max children per node
const MAX_KEYS: usize = M - 1;
const MIN_KEYS: usize = (M + 1) / 2 - 1; // ceil(M/2)-1 = 1 for non root nodes

#[derive(Debug)]
struct Node {
    keys: Vec<i32>,
    values: Vec<String>,
    children: Vec<usize>,
    is_leaf: bool,
    next_leaf: Option<usize>, // Linked list of leaves
}

pub struct BPlusTree {
    nodes: Vec<Node>,
    root: usize,
}

impl Node {
    fn new(is_leaf: bool) -> Self {
        Node { keys: vec![], values: vec![], children: vec![], is_leaf, next_leaf: None }
    }
    fn is_full(&self) -> bool {
        self.keys.len() == MAX_KEYS
    }
}

impl BPlusTree {
    pub fn new() -> Self {
        BPlusTree { nodes: vec![Node::new(true)], root: 0 }
    }
    fn new_node(&mut self, is_leaf: bool) -> usize {
        self.nodes.push(Node::new(is_leaf));
        self.nodes.len() - 1
    }
    pub fn search(&self, key: i32) -> Option<&String> {
        self.search_node(self.root, key)
    }
    fn search_node(&self, node_idx: usize, key: i32) -> Option<&String> {
        let node = &self.nodes[node_idx];

        if node.is_leaf {
            return match node.keys.binary_search(&key) {
                Ok(pos) => Some(&node.values[pos]),
                Err(_) => None,
            };
        }

        let child = match node.keys.binary_search(&key) {
            Ok(pos) => pos + 1,
            Err(pos) => pos,
        };

        self.search_node(node.children[child], key)
    }
    pub fn range_search(&self, start: i32, end: i32) -> Vec<(i32, String)> {
        let mut result = Vec::new();

        if start > end {
            return result;
        }

        let leaf_idx = self.find_leaf(self.root, start);
        let mut current = Some(leaf_idx);

        while let Some(idx) = current {
            let leaf = &self.nodes[idx];

            for (k, v) in leaf.keys.iter().zip(leaf.values.iter()) {
                if *k < start {
                    continue;
                }
                if *k > end {
                    return result;
                }
                result.push((*k, v.clone()));
            }

            current = leaf.next_leaf;
        }

        result
    }

    fn find_leaf(&self, node_idx: usize, key: i32) -> usize {
        let node = &self.nodes[node_idx];

        if node.is_leaf {
            return node_idx;
        }

        let child = match node.keys.binary_search(&key) {
            Ok(pos) => pos + 1,
            Err(pos) => pos,
        };

        self.find_leaf(node.children[child], key)
    }
    pub fn insert(&mut self, key: i32, value: String) {
        if self.nodes[self.root].is_full() {
            let new_root_idx = self.new_node(false);
            self.nodes[new_root_idx].children.push(self.root);
            self.split_child(new_root_idx, 0);
            self.root = new_root_idx;
        }
        self.insert_non_full(self.root, key, value);
    }
    fn insert_non_full(&mut self, node_idx: usize, key: i32, value: String) {
        let is_leaf = self.nodes[node_idx].is_leaf;
        let search_result = self.nodes[node_idx].keys.binary_search(&key);

        match search_result {
            Ok(pos) => {
                if is_leaf {
                    self.nodes[node_idx].values[pos] = value;
                } else {
                    let child = self.nodes[node_idx].children[pos + 1];
                    self.insert_non_full(child, key, value);
                }
            }
            Err(pos) => {
                if is_leaf {
                    self.nodes[node_idx].keys.insert(pos, key);
                    self.nodes[node_idx].values.insert(pos, value);
                } else {
                    let mut child_pos = pos;
                    let child_idx = self.nodes[node_idx].children[child_pos];
                    if self.nodes[child_idx].is_full() {
                        self.split_child(node_idx, child_pos);
                        if key >= self.nodes[node_idx].keys[child_pos] {
                            child_pos += 1;
                        }
                    }
                    let target_child = self.nodes[node_idx].children[child_pos];
                    self.insert_non_full(target_child, key, value);
                }
            }
        }
    }
    fn split_child(&mut self, parent_idx: usize, child_pos: usize) {
        let child_idx = self.nodes[parent_idx].children[child_pos];

        if self.nodes[child_idx].is_leaf {
            self.split_leaf(parent_idx, child_pos);
        } else {
            self.split_internal(parent_idx, child_pos);
        }
    }

    fn split_leaf(&mut self, parent_idx: usize, child_pos: usize) {
        let child_idx = self.nodes[parent_idx].children[child_pos];
        let mid = self.nodes[child_idx].keys.len() / 2;

        let new_leaf_idx = self.new_node(true);

        self.nodes[new_leaf_idx].keys = self.nodes[child_idx].keys.split_off(mid);
        self.nodes[new_leaf_idx].values = self.nodes[child_idx].values.split_off(mid);

        let old_next = self.nodes[child_idx].next_leaf;
        self.nodes[new_leaf_idx].next_leaf = old_next;
        self.nodes[child_idx].next_leaf = Some(new_leaf_idx);

        let promoted = self.nodes[new_leaf_idx].keys[0];

        self.nodes[parent_idx].keys.insert(child_pos, promoted);
        self.nodes[parent_idx].children.insert(child_pos + 1, new_leaf_idx);
    }
    fn split_internal(&mut self, parent_idx: usize, child_pos: usize) {
        let child_idx = self.nodes[parent_idx].children[child_pos];
        let mid = self.nodes[child_idx].keys.len() / 2;

        let promoted = self.nodes[child_idx].keys.remove(mid);

        let new_internal_idx = self.new_node(false);

        self.nodes[new_internal_idx].keys = self.nodes[child_idx].keys.split_off(mid);
        self.nodes[new_internal_idx].children = self.nodes[child_idx].children.split_off(mid + 1);

        self.nodes[parent_idx].keys.insert(child_pos, promoted);
        self.nodes[parent_idx].children.insert(child_pos + 1, new_internal_idx);
    }
    pub fn delete(&mut self, key: i32) {
        self.delete_recursive(self.root, key);

        if !self.nodes[self.root].is_leaf && self.nodes[self.root].keys.is_empty() {
            self.root = self.nodes[self.root].children[0];
        }
    }

    fn delete_recursive(&mut self, node_idx: usize, key: i32) -> bool {
        if self.nodes[node_idx].is_leaf {
            self.remove_from_leaf(node_idx, key);

            if node_idx == self.root {
                return false;
            }
            return self.nodes[node_idx].keys.len() < MIN_KEYS;
        }

        let child_pos = match self.nodes[node_idx].keys.binary_search(&key) {
            Ok(pos) => pos + 1,
            Err(pos) => pos,
        };

        let child_idx = self.nodes[node_idx].children[child_pos];
        let child_underflow = self.delete_recursive(child_idx, key);

        if child_underflow {
            if self.nodes[child_idx].is_leaf {
                self.fix_leaf(node_idx, child_pos);
            } else {
                self.fix_internal(node_idx, child_pos);
            }
        }

        self.refresh_separator(node_idx);

        if node_idx == self.root {
            return false;
        }
        self.nodes[node_idx].keys.len() < MIN_KEYS
    }

    // --- was missing entirely ---
    fn remove_from_leaf(&mut self, node_idx: usize, key: i32) {
        if let Ok(pos) = self.nodes[node_idx].keys.binary_search(&key) {
            self.nodes[node_idx].keys.remove(pos);
            self.nodes[node_idx].values.remove(pos);
        }
        // Err case: key not present — nothing to do (matches delete_missing_key test)
    }

    fn fix_leaf(&mut self, parent_idx: usize, child_pos: usize) {
        let leaf_idx = self.nodes[parent_idx].children[child_pos];

        if self.nodes[leaf_idx].keys.len() >= MIN_KEYS {
            return;
        }

        if child_pos > 0 {
            let left_idx = self.nodes[parent_idx].children[child_pos - 1];
            if self.nodes[left_idx].keys.len() > MIN_KEYS {
                self.borrow_from_left_leaf(parent_idx, child_pos);
                return;
            }
        }

        if child_pos + 1 < self.nodes[parent_idx].children.len() {
            let right_idx = self.nodes[parent_idx].children[child_pos + 1];
            if self.nodes[right_idx].keys.len() > MIN_KEYS {
                self.borrow_from_right_leaf(parent_idx, child_pos);
                return;
            }
        }

        if child_pos > 0 {
            self.merge_leaf(parent_idx, child_pos - 1);
        } else {
            self.merge_leaf(parent_idx, child_pos);
        }
    }

    // --- was missing entirely: internal-node equivalent of fix_leaf ---
    fn fix_internal(&mut self, parent_idx: usize, child_pos: usize) {
        let child_idx = self.nodes[parent_idx].children[child_pos];

        if self.nodes[child_idx].keys.len() >= MIN_KEYS {
            return;
        }

        if child_pos > 0 {
            let left_idx = self.nodes[parent_idx].children[child_pos - 1];
            if self.nodes[left_idx].keys.len() > MIN_KEYS {
                self.borrow_from_left_internal(parent_idx, child_pos);
                return;
            }
        }

        if child_pos + 1 < self.nodes[parent_idx].children.len() {
            let right_idx = self.nodes[parent_idx].children[child_pos + 1];
            if self.nodes[right_idx].keys.len() > MIN_KEYS {
                self.borrow_from_right_internal(parent_idx, child_pos);
                return;
            }
        }

        if child_pos > 0 {
            self.merge_internal(parent_idx, child_pos - 1);
        } else {
            self.merge_internal(parent_idx, child_pos);
        }
    }

    fn refresh_separator(&mut self, node_idx: usize) {
        if self.nodes[node_idx].is_leaf {
            return;
        }
        let child_count = self.nodes[node_idx].children.len();
        for i in 1..child_count {
            let child = self.nodes[node_idx].children[i];
            let first = self.first_key(child);
            self.nodes[node_idx].keys[i - 1] = first;
        }
    }
    fn first_key(&self, mut node_idx: usize) -> i32 {
        while !self.nodes[node_idx].is_leaf {
            node_idx = self.nodes[node_idx].children[0];
        }
        self.nodes[node_idx].keys[0]
    }

    fn borrow_from_left_leaf(&mut self, parent_idx: usize, child_pos: usize) {
        let left_idx = self.nodes[parent_idx].children[child_pos - 1];
        let leaf_idx = self.nodes[parent_idx].children[child_pos];

        let (left, leaf) = {
            let (a, b) = self.nodes.split_at_mut(leaf_idx);
            (&mut a[left_idx], &mut b[0])
        };

        let key = left.keys.pop().unwrap();
        let value = left.values.pop().unwrap();

        leaf.keys.insert(0, key);
        leaf.values.insert(0, value);

        self.nodes[parent_idx].keys[child_pos - 1] = leaf.keys[0];
    }
    fn borrow_from_right_leaf(&mut self, parent_idx: usize, child_pos: usize) {
        let leaf_idx = self.nodes[parent_idx].children[child_pos];
        let right_idx = self.nodes[parent_idx].children[child_pos + 1];

        let (leaf, right) = {
            let (a, b) = self.nodes.split_at_mut(right_idx);
            (&mut a[leaf_idx], &mut b[0])
        };

        let key = right.keys.remove(0);
        let value = right.values.remove(0);

        leaf.keys.push(key);
        leaf.values.push(value);

        self.nodes[parent_idx].keys[child_pos] = right.keys[0];
    }
    fn merge_leaf(&mut self, parent_idx: usize, left_pos: usize) {
        let left_idx = self.nodes[parent_idx].children[left_pos];
        let right_idx = self.nodes[parent_idx].children[left_pos + 1];

        self.nodes[parent_idx].keys.remove(left_pos);
        self.nodes[parent_idx].children.remove(left_pos + 1);

        let (left, right) = {
            let (a, b) = self.nodes.split_at_mut(right_idx);
            (&mut a[left_idx], &mut b[0])
        };

        left.keys.append(&mut right.keys);
        left.values.append(&mut right.values);
        left.next_leaf = right.next_leaf;
    }

    fn borrow_from_left_internal(&mut self, parent_idx: usize, child_pos: usize) {
        let left_idx = self.nodes[parent_idx].children[child_pos - 1];
        let child_idx = self.nodes[parent_idx].children[child_pos];

        {
            let (left, child) = if left_idx < child_idx {
                let (a, b) = self.nodes.split_at_mut(child_idx);
                (&mut a[left_idx], &mut b[0])
            } else {
                let (a, b) = self.nodes.split_at_mut(left_idx);
                (&mut b[0], &mut a[child_idx])
            };

            let borrowed_child = left.children.pop().unwrap();
            child.children.insert(0, borrowed_child);

            left.keys.pop().unwrap();
            child.keys.insert(0, 0);
        }

        self.refresh_separator(child_idx);
    }

    fn borrow_from_right_internal(&mut self, parent_idx: usize, child_pos: usize) {
        let child_idx = self.nodes[parent_idx].children[child_pos];
        let right_idx = self.nodes[parent_idx].children[child_pos + 1];

        {
            let (child, right) = if child_idx < right_idx {
                let (a, b) = self.nodes.split_at_mut(right_idx);
                (&mut a[child_idx], &mut b[0])
            } else {
                let (a, b) = self.nodes.split_at_mut(child_idx);
                (&mut b[0], &mut a[right_idx])
            };

            let borrowed_child = right.children.remove(0);
            child.children.push(borrowed_child);

            right.keys.remove(0);
            child.keys.push(0);
        }

        self.refresh_separator(child_idx);
    }

    fn merge_internal(&mut self, parent_idx: usize, left_pos: usize) {
        let left_idx = self.nodes[parent_idx].children[left_pos];
        let right_idx = self.nodes[parent_idx].children[left_pos + 1];

        self.nodes[parent_idx].keys.remove(left_pos);
        self.nodes[parent_idx].children.remove(left_pos + 1);

        {
            let (left, right) = if left_idx < right_idx {
                let (a, b) = self.nodes.split_at_mut(right_idx);
                (&mut a[left_idx], &mut b[0])
            } else {
                let (a, b) = self.nodes.split_at_mut(left_idx);
                (&mut b[0], &mut a[right_idx])
            };

            left.keys.push(0); // placeholder for the newly-created internal boundary
            left.keys.append(&mut right.keys);
            left.children.append(&mut right.children);
        }

        self.refresh_separator(left_idx);
    }
    pub fn debug_print(&self) {
        self.debug_print_node(self.root, 0);
    }
    fn debug_print_node(&self, node_idx: usize, depth: usize) {
        let node = &self.nodes[node_idx];
        println!("{}{:?} (leaf: {})", "  ".repeat(depth), node.keys, node.is_leaf);
        if !node.is_leaf {
            for &child_idx in &node.children {
                self.debug_print_node(child_idx, depth + 1);
            }
        }
    }
    pub fn validate(&self) {
        let mut leaf_depth = None;
        self.validate_node(self.root, 0, &mut leaf_depth, None, None);
    }
    fn validate_node(
        &self,
        node_idx: usize,
        depth: usize,
        leaf_depth: &mut Option<usize>,
        min: Option<i32>,
        max: Option<i32>
    ) {
        let node = &self.nodes[node_idx];
        for i in 1..node.keys.len() {
            assert!(node.keys[i - 1] < node.keys[i]);
        }
        for &k in &node.keys {
            if let Some(lo) = min {
                assert!(k >= lo);
            }
            if let Some(hi) = max {
                assert!(k < hi);
            }
        }
        if node_idx != self.root {
            assert!(node.keys.len() >= MIN_KEYS);
        }
        assert!(node.keys.len() <= MAX_KEYS);

        if node.is_leaf {
            assert!(node.children.is_empty());
            match leaf_depth {
                None => {
                    *leaf_depth = Some(depth);
                }
                Some(d) => assert_eq!(*d, depth),
            }
            return;
        }

        assert_eq!(node.children.len(), node.keys.len() + 1);
        for i in 0..node.children.len() {
            let child_min = if i == 0 { min } else { Some(node.keys[i - 1]) };
            let child_max = if i == node.keys.len() { max } else { Some(node.keys[i]) };
            self.validate_node(node.children[i], depth + 1, leaf_depth, child_min, child_max);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_tree() {
        let tree = BPlusTree::new();
        tree.validate();
        assert!(tree.search(10).is_none());
    }
    #[test]
    fn single_insert() {
        let mut tree = BPlusTree::new();
        tree.insert(10, "a".into());
        tree.validate();
        assert_eq!(tree.search(10), Some(&"a".to_string()));
    }
    #[test]
    fn overwrite_value() {
        let mut tree = BPlusTree::new();
        tree.insert(5, "hello".into());
        tree.insert(5, "world".into());
        tree.validate();
        assert_eq!(tree.search(5), Some(&"world".to_string()));
    }
    #[test]
    fn ascending_insert() {
        let mut tree = BPlusTree::new();
        for i in 0..100 {
            tree.insert(i, format!("{}", i));
            tree.validate();
        }
        for i in 0..100 {
            assert!(tree.search(i).is_some());
        }
    }
    #[test]
    fn descending_insert() {
        let mut tree = BPlusTree::new();
        for i in (0..100).rev() {
            tree.insert(i, format!("{}", i));
            tree.validate();
        }
    }
    #[test]
    fn delete_all_forward() {
        let mut tree = BPlusTree::new();
        for i in 0..100 {
            tree.insert(i, format!("{}", i));
        }
        for i in 0..100 {
            tree.delete(i);
            tree.validate();
        }
        for i in 0..100 {
            assert!(tree.search(i).is_none());
        }
    }
    #[test]
    fn delete_all_reverse() {
        let mut tree = BPlusTree::new();
        for i in 0..100 {
            tree.insert(i, format!("{}", i));
        }
        for i in (0..100).rev() {
            tree.delete(i);
            tree.validate();
        }
    }
    #[test]
    fn delete_missing_key() {
        let mut tree = BPlusTree::new();
        tree.insert(10, "10".into());
        tree.delete(999);
        tree.validate();
    }
    #[test]
    fn mixed_operations() {
        let mut tree = BPlusTree::new();
        tree.insert(10, "10".into());
        tree.insert(20, "20".into());
        tree.insert(30, "30".into());
        tree.delete(20);
        tree.insert(40, "40".into());
        tree.delete(10);
        tree.insert(50, "50".into());
        tree.validate();
    }
    #[test]
    fn stress() {
        let mut tree = BPlusTree::new();
        for i in 0..1000 {
            tree.insert(i, format!("{}", i));
            tree.validate();
        }
        for i in 0..1000 {
            tree.delete(i);
            tree.validate();
        }
    }
    #[test]
    fn range_search_test() {
        let mut tree = BPlusTree::new();
        for i in 0..20 {
            tree.insert(i, format!("{}", i));
        }
        let ans = tree.range_search(5, 10);
        assert_eq!(
            ans,
            vec![
                (5, "5".to_string()),
                (6, "6".to_string()),
                (7, "7".to_string()),
                (8, "8".to_string()),
                (9, "9".to_string()),
                (10, "10".to_string())
            ]
        );
    }
    #[test]
    fn stress_descending_delete_finds_internal_reorder() {
        // targets the sibling-index-ordering edge case described above
        let mut tree = BPlusTree::new();
        for i in 0..500 {
            tree.insert(i, format!("{}", i));
        }
        for i in (0..500).rev() {
            tree.delete(i);
            tree.validate();
        }
    }
}
