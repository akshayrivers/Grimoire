use bloom_filter::hash::xxhash::Xxhash;
use bloom_filter::{BloomFilter, DeleteResult};
use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

const ELEMENTS: usize = 100_000;

fn benchmark_deletion(c: &mut Criterion) {
    let mut group = c.benchmark_group("deletion");

    group.bench_function("Xxhash", |b| {
        b.iter_batched(
            || {
                let hasher = Xxhash::new(42);

                let mut filter = BloomFilter::new(1_000_000, 7, 100, hasher);

                let elements: Vec<Vec<u8>> = (0..ELEMENTS)
                    .map(|i| format!("element-{i}").into_bytes())
                    .collect();

                for element in &elements {
                    filter.insert(element);
                }

                (filter, elements)
            },
            |(mut filter, elements)| {
                let mut deleted = 0;

                for element in &elements {
                    if matches!(filter.delete(black_box(element)), DeleteResult::Deleted) {
                        deleted += 1;
                    }
                }

                black_box(deleted);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();

    measure_deletion_success_rate();
}

fn measure_deletion_success_rate() {
    println!();
    println!("==============================================");
    println!("Deletion Success Rate");
    println!("==============================================");

    const M: usize = 1_000_000;
    const K: usize = 7;

    for elements_count in [1_000, 5_000, 10_000, 25_000, 50_000, 100_000] {
        println!();
        println!("Elements: {elements_count}");
        println!("----------------------------------------------");

        for regions in [10, 25, 50, 100, 250, 500, 1000] {
            let hasher = Xxhash::new(42);

            let mut filter = BloomFilter::new(M, K, regions, hasher);

            let elements: Vec<Vec<u8>> = (0..elements_count)
                .map(|i| format!("element-{i}").into_bytes())
                .collect();

            for element in &elements {
                filter.insert(element);
            }

            let mut deleted = 0;
            let mut unsafe_to_delete = 0;
            let mut not_found = 0;

            for element in &elements {
                match filter.delete(element) {
                    DeleteResult::Deleted => {
                        deleted += 1;
                    }

                    DeleteResult::UnsafeToDelete => {
                        unsafe_to_delete += 1;
                    }

                    DeleteResult::NotFound => {
                        not_found += 1;
                    }
                }
            }

            let success_rate = deleted as f64 / elements_count as f64 * 100.0;

            println!(
                "r = {:>4} | deleted = {:>6} | unsafe = {:>6} | not found = {:>6} | success = {:>6.2}%",
                regions, deleted, unsafe_to_delete, not_found, success_rate
            );
        }
    }

    println!();
}

criterion_group!(benches, benchmark_deletion);
criterion_main!(benches);
