use std::collections::BTreeMap;
use std::hint::black_box;

use criterion::{ criterion_group, criterion_main, Criterion };

use trees::b_tree::BTree;

fn criterion_config() -> Criterion {
    Criterion::default()
        .sample_size(100)
        .measurement_time(std::time::Duration::from_secs(60))
        .warm_up_time(std::time::Duration::from_secs(5))
}

criterion_group! {
    name = benches;
    config = criterion_config();
    targets = benchmark_my_btree, benchmark_std
}

criterion_main!(benches);
fn benchmark_my_btree(c: &mut Criterion) {
    c.bench_function("my_btree_insert", |b| {
        b.iter(|| {
            let mut tree = BTree::new();

            for i in 0..10_000 {
                tree.insert(black_box(i), format!("value-{i}"));
            }
        })
    });

    c.bench_function("my_btree_search", |b| {
        let mut tree = BTree::new();

        for i in 0..10_000 {
            tree.insert(i, format!("value-{i}"));
        }

        b.iter(|| {
            for i in 0..10_000 {
                black_box(tree.search(i));
            }
        });
    });

    c.bench_function("my_btree_delete", |b| {
        b.iter(|| {
            let mut tree = BTree::new();

            for i in 0..10_000 {
                tree.insert(i, format!("value-{i}"));
            }

            for i in 0..10_000 {
                tree.delete(i);
            }
        });
    });
}

fn benchmark_std(c: &mut Criterion) {
    c.bench_function("std_insert", |b| {
        b.iter(|| {
            let mut tree: BTreeMap<i32, String> = BTreeMap::new();

            for i in 0..10_000 {
                tree.insert(black_box(i), format!("value-{i}"));
            }
        })
    });

    c.bench_function("std_search", |b| {
        let mut tree: BTreeMap<i32, String> = BTreeMap::new();

        for i in 0..10_000 {
            tree.insert(i, format!("value-{i}"));
        }

        b.iter(|| {
            for i in 0..10_000 {
                black_box(tree.get(&i));
            }
        });
    });

    c.bench_function("std_delete", |b| {
        b.iter(|| {
            let mut tree: BTreeMap<i32, String> = BTreeMap::new();

            for i in 0..10_000 {
                tree.insert(i, format!("value-{i}"));
            }

            for i in 0..10_000 {
                tree.remove(&i);
            }
        });
    });
}
