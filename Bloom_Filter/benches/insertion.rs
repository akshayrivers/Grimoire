use bloom_filter::BloomFilter;
use bloom_filter::hash::fnv::Fnv;
use bloom_filter::hash::murmur3::Murmur3;
use bloom_filter::hash::xxhash::Xxhash;
use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

const ELEMENTS: usize = 100_000;

fn benchmark_insertion(c: &mut Criterion) {
    let mut group = c.benchmark_group("insertion");

    let data: Vec<Vec<u8>> = (0..ELEMENTS)
        .map(|i| format!("element-{i}").into_bytes())
        .collect();

    group.bench_function("Xxhash", |b| {
        b.iter(|| {
            let hasher = Xxhash::new(42);

            let mut filter = BloomFilter::new(1_000_000, 7, 100, hasher);

            for element in &data {
                filter.insert(black_box(element));
            }

            black_box(filter);
        });
    });

    group.bench_function("murmur3", |b| {
        b.iter(|| {
            let hasher = Murmur3::new(42);

            let mut filter = BloomFilter::new(1_000_000, 7, 100, hasher);

            for element in &data {
                filter.insert(black_box(element));
            }

            black_box(filter);
        });
    });

    group.bench_function("fnv", |b| {
        b.iter(|| {
            let hasher = Fnv::new(42, 43);

            let mut filter = BloomFilter::new(1_000_000, 7, 100, hasher);

            for element in &data {
                filter.insert(black_box(element));
            }

            black_box(filter);
        });
    });

    group.finish();
}

criterion_group!(benches, benchmark_insertion);
criterion_main!(benches);
