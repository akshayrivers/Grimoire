use std::collections::BTreeMap;
use trees::b_tree::BTree;
#[test]
fn compare_with_std_btreemap() {
    let mut mine = BTree::new();
    let mut std_map = BTreeMap::new();

    // Insert
    for i in 0..1000 {
        mine.insert(i, format!("value-{i}"));
        std_map.insert(i, format!("value-{i}"));
    }

    // Search
    for i in 0..1000 {
        assert_eq!(
            mine.search(i).map(|v| v.as_str()),
            std_map.get(&i).map(|v| v.as_str())
        );
    }

    // Delete every third key
    for i in (0..1000).step_by(3) {
        mine.delete(i);
        std_map.remove(&i);
    }

    // Compare again
    for i in 0..1000 {
        assert_eq!(
            mine.search(i).map(|v| v.as_str()),
            std_map.get(&i).map(|v| v.as_str())
        );
    }

    mine.validate();
}

use rand::{ rng, Rng };

#[test]
fn randomized_operations() {
    let mut rng = rng();

    let mut mine = BTree::new();
    let mut std_map = BTreeMap::new();

    for _ in 0..20_000 {
        let key = rng.gen_range(0..500);

        match rng.gen_range(0..3) {
            0 => {
                let value = format!("v{}", key);
                mine.insert(key, value.clone());
                std_map.insert(key, value);
            }

            1 => {
                mine.delete(key);
                std_map.remove(&key);
            }

            _ => {
                assert_eq!(
                    mine.search(key).map(|v| v.as_str()),
                    std_map.get(&key).map(|v| v.as_str())
                );
            }
        }

        mine.validate();
    }
}
#[test]
fn empty_tree() {
    let mut t = BTree::new();

    assert!(t.search(1).is_none());

    t.delete(1);

    t.validate();
}
#[test]
fn duplicate_insert() {
    let mut t = BTree::new();

    t.insert(10, "A".into());
    t.insert(10, "B".into());

    assert_eq!(t.search(10).unwrap(), "B");

    t.validate();
}
#[test]
fn ascending_insert_delete() {
    let mut t = BTree::new();

    for i in 0..500 {
        t.insert(i, i.to_string());
    }

    t.validate();

    for i in 0..500 {
        t.delete(i);
        t.validate();
    }

    assert!(t.search(100).is_none());
}
#[test]
fn descending_insert_delete() {
    let mut t = BTree::new();

    for i in (0..500).rev() {
        t.insert(i, i.to_string());
    }

    t.validate();

    for i in (0..500).rev() {
        t.delete(i);
        t.validate();
    }
}
#[test]
fn root_shrinks() {
    let mut t = BTree::new();

    for i in 0..30 {
        t.insert(i, i.to_string());
    }

    for i in 0..30 {
        t.delete(i);
        t.validate();
    }

    assert!(t.search(10).is_none());
}
#[test]
fn delete_missing() {
    let mut t = BTree::new();

    for i in 0..20 {
        t.insert(i, i.to_string());
    }

    t.delete(1000);

    t.validate();
}
