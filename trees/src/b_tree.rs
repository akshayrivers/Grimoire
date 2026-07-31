pub const M: usize = 32; // order = max children per node
const MAX_KEYS: usize = M - 1;
const MIN_KEYS: usize = (M + 1) / 2 - 1; // ceil(M/2)-1 = 1 for non root nodes

#[derive(Debug)]
struct Node {
    keys: Vec<i32>,
    values: Vec<String>,
    children: Vec<usize>, // it will be empty when its the leaf noe
    is_leaf: bool,
}

pub struct BTree {
    nodes: Vec<Node>,
    root: usize,
}

impl Node {
    fn new(is_leaf: bool) -> Self {
        Node { keys: vec![], values: vec![], children: vec![], is_leaf }
    }
    fn is_full(&self) -> bool {
        self.keys.len() == MAX_KEYS
    }
}

impl BTree {
    pub fn new() -> Self {
        // so we create a root custom node
        BTree { nodes: vec![Node::new(true)], root: 0 }
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
        match node.keys.binary_search(&key) {
            Ok(pos) => Some(&node.values[pos]),
            Err(pos) => {
                if node.is_leaf { None } else { self.search_node(node.children[pos], key) }
            }
        }
    }

    pub fn insert(&mut self, key: i32, value: String) {
        if self.nodes[self.root].is_full() {
            // Step 1: create the new root
            let new_root_idx = self.new_node(false);

            // Step 2: old root becomes its child at position 0
            self.nodes[new_root_idx].children.push(self.root);

            // Step 3: split that child (position 0) since we know it's full
            self.split_child(new_root_idx, 0);
            // Step 4: point self.root at the new root
            self.root = new_root_idx;
        }

        // Step 5: delegate down, regardless of whether we just grew the tree
        self.insert_non_full(self.root, key, value);
    }
    fn insert_non_full(&mut self, node_idx: usize, key: i32, value: String) {
        let is_leaf = self.nodes[node_idx].is_leaf;
        let search_result = self.nodes[node_idx].keys.binary_search(&key);

        match search_result {
            Ok(pos) => {
                // key already exists - we overwrite the value
                self.nodes[node_idx].values[pos] = value;
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
                        // after splitting, the promoted median key now lives at
                        // node_idx.keys[child_pos] — if our insert key equals it,
                        // the key already exists right here; overwrite and stop.
                        match key.cmp(&self.nodes[node_idx].keys[child_pos]) {
                            std::cmp::Ordering::Equal => {
                                self.nodes[node_idx].values[child_pos] = value;
                                return;
                            }
                            std::cmp::Ordering::Greater => {
                                child_pos += 1;
                            }
                            std::cmp::Ordering::Less => {}
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
        let is_leaf = self.nodes[child_idx].is_leaf;
        let mid = self.nodes[child_idx].keys.len() / 2;

        let mid_key = self.nodes[child_idx].keys.remove(mid);
        let mid_value = self.nodes[child_idx].values.remove(mid);

        let new_node_idx = self.new_node(is_leaf);
        self.nodes[new_node_idx].keys = self.nodes[child_idx].keys.split_off(mid);
        self.nodes[new_node_idx].values = self.nodes[child_idx].values.split_off(mid);

        if !is_leaf {
            self.nodes[new_node_idx].children = self.nodes[child_idx].children.split_off(mid + 1);
        }

        self.nodes[parent_idx].keys.insert(child_pos, mid_key);
        self.nodes[parent_idx].values.insert(child_pos, mid_value);
        self.nodes[parent_idx].children.insert(child_pos + 1, new_node_idx);
    }
    pub fn delete(&mut self, key: i32) {
        self.delete_from(self.root, key);
        if !self.nodes[self.root].is_leaf && self.nodes[self.root].keys.is_empty() {
            self.root = self.nodes[self.root].children[0];
        }
    }

    fn delete_from(&mut self, node_idx: usize, key: i32) {
        match self.nodes[node_idx].keys.binary_search(&key) {
            // Key exists in this node
            Ok(pos) => {
                if self.nodes[node_idx].is_leaf {
                    self.remove_from_leaf(node_idx, pos);
                } else {
                    self.remove_from_internal(node_idx, pos);
                }
            }
            // Key not in this node
            Err(pos) => {
                if self.nodes[node_idx].is_leaf {
                    // Key doesn't exist.
                    return;
                }
                // fill_child ensures children[pos] is safe to descend into, and returns
                // the (possibly shifted, if a merge with the left sibling happened) position
                // to actually descend into — no length-inference needed.
                let child_pos = self.fill_child(node_idx, pos);
                let child_idx = self.nodes[node_idx].children[child_pos];

                self.delete_from(child_idx, key);
            }
        }
    }

    fn remove_from_leaf(&mut self, node_idx: usize, pos: usize) {
        // Case A: trivial where current node is leaf
        self.nodes[node_idx].keys.remove(pos);
        self.nodes[node_idx].values.remove(pos);
    }

    fn remove_from_internal(&mut self, node_idx: usize, pos: usize) {
        let left_idx = self.nodes[node_idx].children[pos];
        let right_idx = self.nodes[node_idx].children[pos + 1];

        // Case 1: predecessor
        if self.nodes[left_idx].keys.len() > MIN_KEYS {
            let (pred_key, pred_value) = self.get_predecessor(left_idx);

            self.nodes[node_idx].keys[pos] = pred_key;
            self.nodes[node_idx].values[pos] = pred_value;

            self.delete_from(left_idx, pred_key);
        } else if
            // Case 2: successor
            self.nodes[right_idx].keys.len() > MIN_KEYS
        {
            let (succ_key, succ_value) = self.get_successor(right_idx);

            self.nodes[node_idx].keys[pos] = succ_key;
            self.nodes[node_idx].values[pos] = succ_value;

            self.delete_from(right_idx, succ_key);
        } else {
            // Case 3: merge
            // Save the key before it disappears from the parent.
            let key = self.nodes[node_idx].keys[pos];

            self.merge_children(node_idx, pos);

            let merged_idx = self.nodes[node_idx].children[pos];

            self.delete_from(merged_idx, key);
        }
    }

    fn get_predecessor(&self, node_idx: usize) -> (i32, String) {
        // rightmost key in the subtree rooted at node_idx
        let is_leaf = self.nodes[node_idx].is_leaf;
        if is_leaf {
            let pos = self.nodes[node_idx].keys.len() - 1;
            let key = self.nodes[node_idx].keys[pos];
            let value = self.nodes[node_idx].values[pos].clone();
            (key, value)
        } else {
            let child = *self.nodes[node_idx].children.last().unwrap();
            self.get_predecessor(child)
        }
    }

    fn get_successor(&self, node_idx: usize) -> (i32, String) {
        // leftmost key in the subtree rooted at node_idx
        let is_leaf = self.nodes[node_idx].is_leaf;
        if is_leaf {
            let key = self.nodes[node_idx].keys[0];
            let value = self.nodes[node_idx].values[0].clone();
            (key, value)
        } else {
            let child = *self.nodes[node_idx].children.first().unwrap();
            self.get_successor(child)
        }
    }

    // Ensures children[child_pos] has more than MIN_KEYS keys before we descend into it.
    // Returns the position to actually descend into — this is child_pos unchanged, UNLESS
    // a merge with the LEFT sibling happened (only possible when child_pos was the last
    // child), in which case the merged node now sits at child_pos - 1.
    fn fill_child(&mut self, node_idx: usize, child_pos: usize) -> usize {
        let child_idx = self.nodes[node_idx].children[child_pos];

        // Already safe.
        if self.nodes[child_idx].keys.len() > MIN_KEYS {
            return child_pos;
        }

        // Try borrowing from the left sibling.
        if child_pos > 0 {
            let left_idx = self.nodes[node_idx].children[child_pos - 1];
            if self.nodes[left_idx].keys.len() > MIN_KEYS {
                self.borrow_from_left(node_idx, child_pos);
                return child_pos;
            }
        }

        // Try borrowing from the right sibling.
        if child_pos + 1 < self.nodes[node_idx].children.len() {
            let right_idx = self.nodes[node_idx].children[child_pos + 1];
            if self.nodes[right_idx].keys.len() > MIN_KEYS {
                self.borrow_from_right(node_idx, child_pos);
                return child_pos;
            }
        }

        // Neither sibling can lend, so merge.
        if child_pos + 1 < self.nodes[node_idx].children.len() {
            self.merge_children(node_idx, child_pos);
            child_pos
        } else {
            self.merge_children(node_idx, child_pos - 1);
            child_pos - 1
        }
    }
    fn borrow_from_left(&mut self, node_idx: usize, child_pos: usize) {
        let left_idx = self.nodes[node_idx].children[child_pos - 1];
        let child_idx = self.nodes[node_idx].children[child_pos];

        let parent_key = self.nodes[node_idx].keys[child_pos - 1];
        let parent_value = self.nodes[node_idx].values[child_pos - 1].clone();

        let borrowed_key;
        let borrowed_value;
        {
            let (left, child) = if left_idx < child_idx {
                let (a, b) = self.nodes.split_at_mut(child_idx);
                (&mut a[left_idx], &mut b[0])
            } else {
                let (a, b) = self.nodes.split_at_mut(left_idx);
                (&mut b[0], &mut a[child_idx])
            };

            borrowed_key = left.keys.pop().unwrap();
            borrowed_value = left.values.pop().unwrap();

            child.keys.insert(0, parent_key);
            child.values.insert(0, parent_value);

            if !left.is_leaf {
                let borrowed_child = left.children.pop().unwrap();
                child.children.insert(0, borrowed_child);
            }
        }
        self.nodes[node_idx].keys[child_pos - 1] = borrowed_key;
        self.nodes[node_idx].values[child_pos - 1] = borrowed_value;
    }
    fn borrow_from_right(&mut self, node_idx: usize, child_pos: usize) {
        let child_idx = self.nodes[node_idx].children[child_pos];
        let right_idx = self.nodes[node_idx].children[child_pos + 1];

        let parent_key = self.nodes[node_idx].keys[child_pos];
        let parent_value = self.nodes[node_idx].values[child_pos].clone();

        let borrowed_key;
        let borrowed_value;
        {
            let (child, right) = if child_idx < right_idx {
                let (a, b) = self.nodes.split_at_mut(right_idx);
                (&mut a[child_idx], &mut b[0])
            } else {
                let (a, b) = self.nodes.split_at_mut(child_idx);
                (&mut b[0], &mut a[right_idx])
            };

            borrowed_key = right.keys.remove(0);
            borrowed_value = right.values.remove(0);

            child.keys.push(parent_key);
            child.values.push(parent_value);

            if !right.is_leaf {
                let borrowed_child = right.children.remove(0);
                child.children.push(borrowed_child);
            }
        }
        self.nodes[node_idx].keys[child_pos] = borrowed_key;
        self.nodes[node_idx].values[child_pos] = borrowed_value;
    }
    fn merge_children(&mut self, node_idx: usize, child_pos: usize) {
        let left_child_idx = self.nodes[node_idx].children[child_pos];
        let right_child_idx = self.nodes[node_idx].children[child_pos + 1];
        let key = self.nodes[node_idx].keys.remove(child_pos);
        let value = self.nodes[node_idx].values.remove(child_pos);

        self.nodes[node_idx].children.remove(child_pos + 1);

        // borrowing two children simultaneously
        let (left_slice, right_slice) = if left_child_idx < right_child_idx {
            let (a, b) = self.nodes.split_at_mut(right_child_idx);
            (&mut a[left_child_idx], &mut b[0])
        } else {
            let (a, b) = self.nodes.split_at_mut(left_child_idx);
            (&mut b[0], &mut a[right_child_idx])
        };
        // moving parent to the ledt child
        left_slice.keys.push(key);
        left_slice.values.push(value);

        // moving the right child to left too
        left_slice.keys.append(&mut right_slice.keys);
        left_slice.values.append(&mut right_slice.values);

        if !left_slice.is_leaf {
            left_slice.children.append(&mut right_slice.children);
        }
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

        // Keys sorted
        for i in 1..node.keys.len() {
            assert!(node.keys[i - 1] < node.keys[i]);
        }

        // Key range
        for &k in &node.keys {
            if let Some(lo) = min {
                assert!(k > lo);
            }
            if let Some(hi) = max {
                if !(k < hi) {
                    println!("node {:?}", node.keys);
                    println!("k = {}", k);
                    println!("hi = {}", hi);
                    panic!("upper bound violated");
                }
            }
        }

        // Root / non-root size rules
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
        let tree = BTree::new();
        tree.validate();
        assert!(tree.search(10).is_none());
    }

    #[test]
    fn single_insert() {
        let mut tree = BTree::new();
        tree.insert(10, "a".into());
        tree.validate();
        assert_eq!(tree.search(10), Some(&"a".to_string()));
    }

    #[test]
    fn overwrite_value() {
        let mut tree = BTree::new();
        tree.insert(5, "hello".into());
        tree.insert(5, "world".into());
        tree.validate();
        assert_eq!(tree.search(5), Some(&"world".to_string()));
    }

    #[test]
    fn ascending_insert() {
        let mut tree = BTree::new();
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
        let mut tree = BTree::new();
        for i in (0..100).rev() {
            tree.insert(i, format!("{}", i));
            tree.validate();
        }
    }

    #[test]
    fn delete_all_forward() {
        let mut tree = BTree::new();
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
        let mut tree = BTree::new();
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
        let mut tree = BTree::new();
        tree.insert(10, "10".into());
        tree.delete(999);
        tree.validate();
    }

    #[test]
    fn mixed_operations() {
        let mut tree = BTree::new();
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
        let mut tree = BTree::new();
        for i in 0..1000 {
            tree.insert(i, format!("{}", i));
            tree.validate();
        }
        for i in 0..1000 {
            tree.delete(i);
            tree.validate();
        }
    }
}
