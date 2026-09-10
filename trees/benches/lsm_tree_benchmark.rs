use std::hint::black_box;
use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};
use trees::lsm_tree::{LsmConfig, LsmTree};

const N: i32 = 10_000;

fn criterion_config() -> Criterion {
    Criterion::default()
        .sample_size(30)
        .measurement_time(Duration::from_secs(20))
        .warm_up_time(Duration::from_secs(3))
}

criterion_group!(
    name = benches;
    config = criterion_config();
    targets =
        benchmark_lsm_tree_insert,
        benchmark_fjall_insert,
        benchmark_lsm_tree_search,
        benchmark_fjall_search,
        benchmark_lsm_tree_range_scan,
        benchmark_fjall_range_scan,
        benchmark_lsm_tree_delete,
        benchmark_fjall_delete
);

criterion_main!(benches);

fn benchmark_lsm_tree_insert(c: &mut Criterion) {
    let temp_dir = std::env::temp_dir().join("bench_lsm_insert");
    c.bench_function("lsm_tree_insert", |b| {
        b.iter(|| {
            let _ = std::fs::remove_dir_all(&temp_dir);
            let mut tree = LsmTree::open_with_config(
                &temp_dir,
                LsmConfig {
                    memtable_capacity_bytes: 512 * 1024,
                    sstable_block_size: 4096,
                    sync_wal: false,
                },
            )
            .unwrap();

            for i in 0..N {
                tree.put_i32(black_box(i), &format!("value-{i}")).unwrap();
            }
        })
    });
    let _ = std::fs::remove_dir_all(&temp_dir);
}

fn benchmark_lsm_tree_search(c: &mut Criterion) {
    let temp_dir = std::env::temp_dir().join("bench_lsm_search");
    let _ = std::fs::remove_dir_all(&temp_dir);
    let mut tree = LsmTree::open_with_config(
        &temp_dir,
        LsmConfig {
            memtable_capacity_bytes: 256 * 1024,
            sstable_block_size: 4096,
            sync_wal: false,
        },
    )
    .unwrap();

    for i in 0..N {
        tree.put_i32(i, &format!("value-{i}")).unwrap();
    }
    // Flush to disk so we benchmark real SSTable bloom filter + sparse index lookups
    tree.flush().unwrap();

    c.bench_function("lsm_tree_search", |b| {
        b.iter(|| {
            for i in 0..N {
                black_box(tree.get_i32(i).unwrap());
            }
        });
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

fn benchmark_lsm_tree_range_scan(c: &mut Criterion) {
    let temp_dir = std::env::temp_dir().join("bench_lsm_scan");
    let _ = std::fs::remove_dir_all(&temp_dir);
    let mut tree = LsmTree::open_with_config(
        &temp_dir,
        LsmConfig {
            memtable_capacity_bytes: 256 * 1024,
            sstable_block_size: 4096,
            sync_wal: false,
        },
    )
    .unwrap();

    for i in 0..N {
        tree.put_i32(i, &format!("value-{i}")).unwrap();
    }
    tree.flush().unwrap();

    c.bench_function("lsm_tree_range_scan", |b| {
        b.iter(|| {
            black_box(tree.scan(&0i32.to_be_bytes(), &(N - 1).to_be_bytes()).unwrap());
        });
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

fn benchmark_lsm_tree_delete(c: &mut Criterion) {
    let temp_dir = std::env::temp_dir().join("bench_lsm_delete");
    c.bench_function("lsm_tree_delete", |b| {
        b.iter(|| {
            let _ = std::fs::remove_dir_all(&temp_dir);
            let mut tree = LsmTree::open_with_config(
                &temp_dir,
                LsmConfig {
                    memtable_capacity_bytes: 512 * 1024,
                    sstable_block_size: 4096,
                    sync_wal: false,
                },
            )
            .unwrap();

            for i in 0..N {
                tree.put_i32(i, &format!("value-{i}")).unwrap();
            }

            for i in 0..N {
                tree.delete_i32(i).unwrap();
            }
        });
    });
    let _ = std::fs::remove_dir_all(&temp_dir);
}

fn benchmark_fjall_insert(c: &mut Criterion) {
    let temp_dir = std::env::temp_dir().join("bench_fjall_insert");
    c.bench_function("fjall_insert", |b| {
        b.iter(|| {
            let _ = std::fs::remove_dir_all(&temp_dir);
            let db = fjall::Database::builder(&temp_dir).open().unwrap();
            let items = db.keyspace("default", || fjall::KeyspaceCreateOptions::default()).unwrap();

            for i in 0..N {
                items.insert(black_box(i.to_be_bytes()), format!("value-{i}")).unwrap();
            }
        })
    });
    let _ = std::fs::remove_dir_all(&temp_dir);
}

fn benchmark_fjall_search(c: &mut Criterion) {
    let temp_dir = std::env::temp_dir().join("bench_fjall_search");
    let _ = std::fs::remove_dir_all(&temp_dir);
    let db = fjall::Database::builder(&temp_dir).open().unwrap();
    let items = db.keyspace("default", || fjall::KeyspaceCreateOptions::default()).unwrap();

    for i in 0..N {
        items.insert(i.to_be_bytes(), format!("value-{i}")).unwrap();
    }
    db.persist(fjall::PersistMode::SyncAll).unwrap();

    c.bench_function("fjall_search", |b| {
        b.iter(|| {
            for i in 0..N {
                black_box(items.get(i.to_be_bytes()).unwrap());
            }
        });
    });

    drop(items);
    drop(db);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

fn benchmark_fjall_range_scan(c: &mut Criterion) {
    let temp_dir = std::env::temp_dir().join("bench_fjall_scan");
    let _ = std::fs::remove_dir_all(&temp_dir);
    let db = fjall::Database::builder(&temp_dir).open().unwrap();
    let items = db.keyspace("default", || fjall::KeyspaceCreateOptions::default()).unwrap();

    for i in 0..N {
        items.insert(i.to_be_bytes(), format!("value-{i}")).unwrap();
    }
    db.persist(fjall::PersistMode::SyncAll).unwrap();

    c.bench_function("fjall_range_scan", |b| {
        b.iter(|| {
            for item in items.range(0i32.to_be_bytes()..=(N - 1).to_be_bytes()) {
                black_box(item);
            }
        });
    });

    drop(items);
    drop(db);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

fn benchmark_fjall_delete(c: &mut Criterion) {
    let temp_dir = std::env::temp_dir().join("bench_fjall_delete");
    c.bench_function("fjall_delete", |b| {
        b.iter(|| {
            let _ = std::fs::remove_dir_all(&temp_dir);
            let db = fjall::Database::builder(&temp_dir).open().unwrap();
            let items = db.keyspace("default", || fjall::KeyspaceCreateOptions::default()).unwrap();

            for i in 0..N {
                items.insert(i.to_be_bytes(), format!("value-{i}")).unwrap();
            }

            for i in 0..N {
                items.remove(i.to_be_bytes()).unwrap();
            }
        });
    });
    let _ = std::fs::remove_dir_all(&temp_dir);
}

