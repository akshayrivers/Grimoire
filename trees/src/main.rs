use std::collections::BTreeMap;
use std::time::Instant;

use trees::b_tree::BTree;

fn main() {
    let n = 100_000;

    println!("===== My BTree =====");

    let mut my_tree = BTree::new();

    let start = Instant::now();
    for i in 0..n {
        my_tree.insert(i, format!("value-{i}"));
    }
    println!("Insert : {:?}", start.elapsed());

    let start = Instant::now();
    for i in 0..n {
        my_tree.search(i);
    }
    println!("Search : {:?}", start.elapsed());

    let start = Instant::now();
    for i in 0..n {
        my_tree.delete(i);
    }
    println!("Delete : {:?}", start.elapsed());

    println!("\n===== std::collections::BTreeMap =====");

    let mut std_tree: BTreeMap<i32, String> = BTreeMap::new();

    let start = Instant::now();
    for i in 0..n {
        std_tree.insert(i, format!("value-{i}"));
    }
    println!("Insert : {:?}", start.elapsed());

    let start = Instant::now();
    for i in 0..n {
        std_tree.get(&i);
    }
    println!("Search : {:?}", start.elapsed());

    let start = Instant::now();
    for i in 0..n {
        std_tree.remove(&i);
    }
    println!("Delete : {:?}", start.elapsed());
}
