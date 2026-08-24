use fnv::FnvHasher;
use std::hash::Hasher as StdHasher;

use crate::hasher::Hasher;

pub struct Fnv {
    seed1: u64,
    seed2: u64,
}

impl Fnv {
    pub fn new(seed1: u64, seed2: u64) -> Self {
        Self { seed1, seed2 }
    }
}

impl Hasher for Fnv {
    fn hash(&self, data: &[u8]) -> (u64, u64) {
        let mut hasher1 = FnvHasher::default();
        hasher1.write_u64(self.seed1);
        hasher1.write(data);
        let h1 = hasher1.finish();

        let mut hasher2 = FnvHasher::default();
        hasher2.write_u64(self.seed2);
        hasher2.write(data);
        let h2 = hasher2.finish();

        (h1, h2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_input_same_hash() {
        let hasher = Fnv::new(42, 50);

        let first = hasher.hash(b"manglu");
        let second = hasher.hash(b"manglu");

        assert_eq!(first, second);
    }
}
