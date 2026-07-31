#[path = "b+_tree.rs"]
mod b_plus_tree;
mod b_tree;

use b_plus_tree::BPlusTree;
use b_tree::BTree;

fn main() {
    demo_b_tree();
    println!();
    demo_b_plus_tree();
}

fn demo_b_tree() {
    println!("================= B-TREE (order 4) =================");
    let mut tree = BTree::new(4);

    println!("-- insert 0..=15 --");
    for i in 0..=15 {
        tree.insert(i, i * i);
    }
    tree.print_tree();

    println!("\nsearch(7) = {:?}", tree.search(&7));
    println!("search(99) = {:?}", tree.search(&99));
    println!("contains(13) = {}", tree.contains(&13));
    println!("order = {}, len = {}, is_empty = {}", tree.order(), tree.len(), tree.is_empty());
    println!("min = {:?}, max = {:?}", tree.min(), tree.max());
    println!("height = {}", tree.height());

    println!("\nrange(5..=10) = {:?}", tree.range(&5, &10));

    println!("\n-- delete even keys 0..=14 --");
    for i in (0..=14).step_by(2) {
        assert!(tree.delete(&i), "delete {i}");
    }
    tree.print_tree();
    println!("remaining = {:?}", tree.iter().into_iter().map(|(k, _)| k).collect::<Vec<_>>());

    println!("\nvalidate = {:?}", tree.validate());
}

fn demo_b_plus_tree() {
    println!("================= B+ TREE (order 4) =================");
    let mut tree = BPlusTree::new(4);

    println!("-- insert 0..=15 --");
    for i in 0..=15 {
        tree.insert(i, i * i);
    }
    tree.print_tree();

    println!("\nsearch(7) = {:?}", tree.search(&7));
    println!("search(99) = {:?}", tree.search(&99));
    println!("contains(13) = {}", tree.contains(&13));
    println!("order = {}, len = {}, is_empty = {}", tree.order(), tree.len(), tree.is_empty());
    println!("min = {:?}, max = {:?}", tree.min(), tree.max());
    println!("height = {}", tree.height());

    // Range scan walking the leaf linked list.
    println!("\nrange(5..=10) = {:?}", tree.range(&5, &10));

    println!("\n-- delete even keys 0..=14 --");
    for i in (0..=14).step_by(2) {
        assert!(tree.delete(&i), "delete {i}");
    }
    tree.print_tree();
    println!("remaining = {:?}", tree.iter().into_iter().map(|(k, _)| k).collect::<Vec<_>>());

    println!("\nvalidate = {:?}", tree.validate());
}
