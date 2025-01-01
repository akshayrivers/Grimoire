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

fn main() {
    println!("Implementation of basic B+ Tree");
}
