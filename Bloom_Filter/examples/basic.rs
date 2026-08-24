use bloom_filter::BloomFilter;
use bloom_filter::hash::xxhash::Xxhash;

fn main() {
    let hasher = Xxhash::new(42);

    let mut filter = BloomFilter::new(1024, 4, 32, hasher);

    filter.insert(b"manglu");

    println!("manglu: {}", filter.contains(b"manglu"));

    println!("akshay: {}", filter.contains(b"akshay"));
}
