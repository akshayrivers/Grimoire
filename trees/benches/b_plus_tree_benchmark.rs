use std::collections::BTreeMap;
use std::hint::black_box;
use std::time::Duration;

use bplustree::BPlusTree as StdBPlusTree;
use criterion::{ criterion_group, criterion_main, Criterion };

use trees::b_plus_tree::BPlusTree;
use trees::b_tree::BTree;

const N: i32 = 10_000;

fn criterion_config() -> Criterion {
    Criterion::default()
        .sample_size(100)
        .measurement_time(Duration::from_secs(60))
        .warm_up_time(Duration::from_secs(5))
}

criterion_group!(
    name = benches;
    config = criterion_config();
    targets =
        benchmark_my_btree,
        benchmark_my_bplustree,
        benchmark_std_btreemap,
        benchmark_bplustree_crate
);

criterion_main!(benches);

fn benchmark_my_btree(c: &mut Criterion) {
    c.bench_function("my_btree_insert", |b| {
        b.iter(|| {
            let mut tree = BTree::new();
            for i in 0..N {
                tree.insert(black_box(i), format!("value-{i}"));
            }
        })
    });

    c.bench_function("my_btree_search", |b| {
        let mut tree = BTree::new();
        for i in 0..N {
            tree.insert(i, format!("value-{i}"));
        }

        b.iter(|| {
            for i in 0..N {
                black_box(tree.search(i));
            }
        });
    });

    c.bench_function("my_btree_delete", |b| {
        b.iter(|| {
            let mut tree = BTree::new();
            for i in 0..N {
                tree.insert(i, format!("value-{i}"));
            }

            for i in 0..N {
                tree.delete(i);
            }
        });
    });
}

fn benchmark_my_bplustree(c: &mut Criterion) {
    c.bench_function("my_bplustree_insert", |b| {
        b.iter(|| {
            let mut tree = BPlusTree::new();
            for i in 0..N {
                tree.insert(black_box(i), format!("value-{i}"));
            }
        })
    });

    c.bench_function("my_bplustree_search", |b| {
        let mut tree = BPlusTree::new();
        for i in 0..N {
            tree.insert(i, format!("value-{i}"));
        }

        b.iter(|| {
            for i in 0..N {
                black_box(tree.search(i));
            }
        });
    });

    c.bench_function("my_bplustree_range_scan", |b| {
        let mut tree = BPlusTree::new();
        for i in 0..N {
            tree.insert(i, format!("value-{i}"));
        }

        b.iter(|| {
            black_box(tree.range_search(0, N - 1));
        });
    });

    c.bench_function("my_bplustree_delete", |b| {
        b.iter(|| {
            let mut tree = BPlusTree::new();
            for i in 0..N {
                tree.insert(i, format!("value-{i}"));
            }

            for i in 0..N {
                tree.delete(i);
            }
        });
    });
}

fn benchmark_std_btreemap(c: &mut Criterion) {
    c.bench_function("std_btreemap_insert", |b| {
        b.iter(|| {
            let mut tree = BTreeMap::<i32, String>::new();

            for i in 0..N {
                tree.insert(black_box(i), format!("value-{i}"));
            }
        })
    });

    c.bench_function("std_btreemap_search", |b| {
        let mut tree = BTreeMap::<i32, String>::new();

        for i in 0..N {
            tree.insert(i, format!("value-{i}"));
        }

        b.iter(|| {
            for i in 0..N {
                black_box(tree.get(&i));
            }
        });
    });

    c.bench_function("std_btreemap_range_scan", |b| {
        let mut tree = BTreeMap::<i32, String>::new();

        for i in 0..N {
            tree.insert(i, format!("value-{i}"));
        }

        b.iter(|| {
            black_box(tree.range(0..N).collect::<Vec<_>>());
        });
    });

    c.bench_function("std_btreemap_delete", |b| {
        b.iter(|| {
            let mut tree = BTreeMap::<i32, String>::new();

            for i in 0..N {
                tree.insert(i, format!("value-{i}"));
            }

            for i in 0..N {
                tree.remove(&i);
            }
        });
    });
}

fn benchmark_bplustree_crate(c: &mut Criterion) {
    c.bench_function("crate_bplustree_insert", |b| {
        b.iter(|| {
            let tree = StdBPlusTree::new();

            for i in 0..N {
                tree.insert(black_box(i), format!("value-{i}"));
            }
        })
    });

    c.bench_function("crate_bplustree_search", |b| {
        let tree = StdBPlusTree::new();

        for i in 0..N {
            tree.insert(i, format!("value-{i}"));
        }

        b.iter(|| {
            for i in 0..N {
                black_box(tree.lookup(&i, |v| v.clone()));
            }
        });
    });

    c.bench_function("crate_bplustree_range_scan", |b| {
        let tree = StdBPlusTree::new();

        for i in 0..N {
            tree.insert(i, format!("value-{i}"));
        }

        b.iter(|| {
            let mut iter = tree.raw_iter();
            iter.seek_to_first();

            while let Some((k, v)) = iter.next() {
                black_box((k, v));
            }
        });
    });

    c.bench_function("crate_bplustree_delete", |b| {
        b.iter(|| {
            let tree = StdBPlusTree::new();

            for i in 0..N {
                tree.insert(i, format!("value-{i}"));
            }

            for i in 0..N {
                tree.remove(&i);
            }
        });
    });
}
