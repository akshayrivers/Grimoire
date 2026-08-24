use xxhash_rust::xxh3::xxh3_64_with_seed;

use crate::hasher::Hasher;

pub struct Xxhash {
    seed: u64,
}

impl Xxhash {
    pub fn new(seed: u64) -> Self {
        Self { seed }
    }
}

impl Hasher for Xxhash {
    fn hash(&self, data: &[u8]) -> (u64, u64) {
        let h1 = xxh3_64_with_seed(data, self.seed);
        let h2 = xxh3_64_with_seed(data, self.seed.wrapping_add(1));

        (h1, h2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_input_same_hash() {
        let hasher = Xxhash::new(42);

        let first = hasher.hash(b"manglu");
        let second = hasher.hash(b"manglu");

        assert_eq!(first, second);
    }
}
