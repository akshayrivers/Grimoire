use bloom_filter::BloomFilter;
use bloom_filter::hash::xxhash::Xxhash;
use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

const ELEMENTS: usize = 100_000;

fn benchmark_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("lookup");

    let inserted: Vec<Vec<u8>> = (0..ELEMENTS)
        .map(|i| format!("element-{i}").into_bytes())
        .collect();

    let absent: Vec<Vec<u8>> = (0..ELEMENTS)
        .map(|i| format!("absent-{i}").into_bytes())
        .collect();

    let hasher = Xxhash::new(42);

    let mut filter = BloomFilter::new(1_000_000, 7, 100, hasher);

    for element in &inserted {
        filter.insert(element);
    }

    group.bench_function("present", |b| {
        b.iter(|| {
            for element in &inserted {
                black_box(filter.contains(element));
            }
        });
    });

    group.bench_function("absent", |b| {
        b.iter(|| {
            for element in &absent {
                black_box(filter.contains(element));
            }
        });
    });

    group.finish();

    measure_false_positive_rate();
}

fn measure_false_positive_rate() {
    println!();
    println!("==============================");
    println!("False Positive Rate");
    println!("==============================");

    for bits_per_element in [6, 8, 10, 12, 14] {
        let m = ELEMENTS * bits_per_element;

        // Approximate optimal k.
        let k = ((bits_per_element as f64 * 0.693).round() as usize).max(1);

        let hasher = Xxhash::new(42);

        let mut filter = BloomFilter::new(m, k, 100, hasher);

        for i in 0..ELEMENTS {
            let element = format!("element-{i}");
            filter.insert(element.as_bytes());
        }

        let mut false_positives = 0;

        for i in 0..ELEMENTS {
            let element = format!("absent-{i}");

            if filter.contains(element.as_bytes()) {
                false_positives += 1;
            }
        }

        let rate = false_positives as f64 / ELEMENTS as f64 * 100.0;

        println!(
            "{:>2} bits/element | k = {:>2} | false positives = {:>6} | FPR = {:.4}%",
            bits_per_element, k, false_positives, rate
        );
    }

    println!();
}

criterion_group!(benches, benchmark_lookup);
criterion_main!(benches);
