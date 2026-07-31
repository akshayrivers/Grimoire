use std::collections::BTreeMap;
use std::time::Instant;

use trees::b_tree::BTree;
use trees::b_plus_tree::BPlusTree;
use bplustree::BPlusTree as StdBPlusTree;

fn main() {
    let n = 1_000_000;

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

    println!("\n===== My BPlusTree =====");
    let mut my_bplus = BPlusTree::new();

    let start = Instant::now();
    for i in 0..n {
        my_bplus.insert(i, format!("value-{i}"));
    }
    println!("Insert : {:?}", start.elapsed());

    let start = Instant::now();
    for i in 0..n {
        my_bplus.search(i);
    }
    println!("Search : {:?}", start.elapsed());

    let start = Instant::now();
    my_bplus.range_search(0, n - 1);
    println!("Full range scan : {:?}", start.elapsed());

    let start = Instant::now();
    for i in 0..n {
        my_bplus.delete(i);
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

    // Full range scan
    let start = Instant::now();
    for (_k, _v) in std_tree.range(0..n) {
        std::hint::black_box((_k, _v));
    }
    println!("Full range scan : {:?}", start.elapsed());

    let start = Instant::now();
    for i in 0..n {
        std_tree.remove(&i);
    }
    println!("Delete : {:?}", start.elapsed());

    println!("\n===== bplustree crate =====");

    let mut std_tree = StdBPlusTree::new();

    let start = Instant::now();
    for i in 0..n {
        std_tree.insert(i, format!("value-{i}"));
    }
    println!("Insert : {:?}", start.elapsed());

    let start = Instant::now();
    for i in 0..n {
        std_tree.lookup(&i, |v| v.clone());
    }
    // for i in 0..n {
    //     std_tree.lookup(&i, |_| ());
    // }
    println!("Search : {:?}", start.elapsed());

    let start = Instant::now();
    for i in 0..n {
        std_tree.remove(&i);
    }
    println!("Delete : {:?}", start.elapsed());
    // Full range scan
    let start = Instant::now();
    let mut iter = std_tree.raw_iter();
    iter.seek_to_first();

    while let Some((_k, _v)) = iter.next() {}

    println!("Full range scan : {:?}", start.elapsed());
}
