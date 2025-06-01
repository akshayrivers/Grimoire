/*
    B+ Tree implementation in Rust
    B+ Tree is a self-balancing tree that ensures all keys are stored in sorted order and provides fast lookups.
    We will implement B+ Tree using the conventional method with splitting and merging nodes.

    Basic design of B+ Tree:
    1. It is a self-balancing tree based on height.
    2. Internal nodes store keys and pointers to child nodes. They guide the search process.
    3. Leaf nodes store the actual keys and values along with a pointer to the next leaf node, enabling range queries.
    4. The order of the tree determines the maximum number of keys a node can hold. For a given order `m`:
        - Internal nodes can have up to `m - 1` keys and `m` child pointers.
        - Leaf nodes can hold up to `m - 1` key-value pairs.
    5. All leaf nodes are linked together in a doubly-linked list for fast traversal.

    Properties:
    1. All leaf nodes are at the same depth (balanced tree).
    2. Each node, except the root, has at least ⌈m/2⌉ keys.
    3. The root node must have at least 2 keys unless it is the only node in the tree.

    How it works:
    1. Search:
        - Start at the root and traverse down to the appropriate leaf node.
        - Use binary or linear search within the node to locate the key.
    2. Insert:
        - Insert the key-value pair into the appropriate leaf node.
        - If the node exceeds its capacity, split it and propagate the split upwards.
    3. Delete:
        - Remove the key-value pair from the appropriate leaf node.
        - If a node has fewer than the minimum number of keys, merge or redistribute keys with its sibling.

*/

// IMPLEMENTATION OF B TREE
struct BTree {
    root: Option<Box<Node>>, // Root of the BTree, could be a leaf or internal node
    order: usize,            // Order of the BTree (maximum number of children per node)
}

impl BTree {
    fn new(order: usize) -> Self {
        BTree { root: None, order }
    }

    // Insert function will be implemented here
    fn insert(&mut self, key: i32, value: i32) {
        // Implement insertion logic
    }

    // Search function will be implemented here
    fn search(&self, key: i32) -> Option<i32> {
        // Implement search logic
        None
    }
}

// Define Node enum, which can either be a Leaf or Internal Node
#[derive(Debug)]
enum Node {
    InternalNode {
        keys: Vec<i32>,
        children: Vec<Box<Node>>,
    },
    LeafNode {
        keys: Vec<i32>,
        values: Vec<i32>,
    },
}

// Helper function to create an InternalNode
impl Node {
    fn new_internal_node() -> Self {
        Node::InternalNode {
            keys: Vec::new(),
            children: Vec::new(),
        }
    }

    // Helper function to create a LeafNode
    fn new_leaf_node() -> Self {
        Node::LeafNode {
            keys: Vec::new(),
            values: Vec::new(),
        }
    }
}

// a array of length 4 max if it exceed we initialise a new one or we can do it through vector too
const Max_size: usize = 4;
#[derive(Debug)]
struct BPlusTree {
    root: Option<Box<Node>>,
}

fn main() {
    let mut tree = BTree::new(3); // A B Tree of order 3
    tree.insert(10, 100); // Insert key-value pair into the tree
    tree.insert(20, 200);
    tree.insert(5, 50);

    // Search for a key in the tree
    if let Some(value) = tree.search(10) {
        println!("Found value: {}", value);
    } else {
        println!("Value not found.");
    }
}
