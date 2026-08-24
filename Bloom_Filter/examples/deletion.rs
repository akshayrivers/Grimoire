use bloom_filter::hash::xxhash::Xxhash;
use bloom_filter::{BloomFilter, DeleteResult};

fn main() {
    let hasher = Xxhash::new(42);

    let mut filter = BloomFilter::new(1024, 4, 32, hasher);

    filter.insert(b"manglu");

    println!("Before deletion: {}", filter.contains(b"manglu"));

    match filter.delete(b"manglu") {
        DeleteResult::Deleted => {
            println!("manglu was safely deleted");
        }

        DeleteResult::NotFound => {
            println!("manglu was not present");
        }

        DeleteResult::UnsafeToDelete => {
            println!("manglu could not be safely deleted");
        }
    }

    println!("After deletion: {}", filter.contains(b"manglu"));
}
