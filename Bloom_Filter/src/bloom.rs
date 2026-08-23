use crate::hasher::Hasher;
use crate::bit_array::BitArray;

pub enum DeleteResult {
    Deleted,
    NotFound,
    UnsafeToDelete,
}
pub struct BloomFilter<H> {
    bits: BitArray,
    collisions: BitArray,

    m: usize,
    k: usize,
    r: usize,

    hasher: H,
}

impl<H: Hasher> BloomFilter<H>{
    pub fn new(m:usize, k:usize, r: usize, hasher:H)-> Self{
        assert!(m>0);
        assert!(k>0);
        assert!(r>0);
        assert!(m%r == 0);

        Self { bits: BitArray::new(m), collisions: BitArray::new(r), m, k, r, hasher }
    }
    fn positions(&self, data: &[u8]) -> impl Iterator<Item = usize> + use<'_,H>{
        let (h1, h2) = self.hasher.hash(data);

        (0..self.k).map(move |i| {
            let hash = h1.wrapping_add((i as u64).wrapping_mul(h2));

            (hash % self.m as u64) as usize
        })
    }
    fn region_for(&self, position: usize) -> usize {
        let region_size = self.m / self.r;

        position / region_size
    }
    pub fn insert(&mut self, data: &[u8]) {
        let positions: Vec<usize> = self.positions(data).collect();
        for position in positions {
            let region = self.region_for(position);

            if self.bits.get(position) {
                self.collisions.set(region);
            }

            self.bits.set(position);
        }
    }
    pub fn contains(&self, data: &[u8]) -> bool {
        self.positions(data)
            .all(|position| self.bits.get(position))
    }
    pub fn delete(&mut self, data: &[u8]) -> DeleteResult {
        let positions: Vec<usize> = self.positions(data).collect();

        // First make sure the element might actually exist.
        if positions.iter().any(|&position| !self.bits.get(position)) {
            return DeleteResult::NotFound;
        }

        // Find a bit that is in a collision-free region.
        for position in positions {
            let region = self.region_for(position);

            if !self.collisions.get(region) {
                self.bits.clear(position);

                return DeleteResult::Deleted;
            }
        }

        DeleteResult::UnsafeToDelete
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::xxhash::Xxhash;

    #[test]
    fn generates_k_positions() {
        let hasher = Xxhash::new(42);

        let filter = BloomFilter::new(
            1024,
            4,
            32,
            hasher,
        );

        let positions: Vec<usize> =
            filter.positions(b"manglu").collect();

        assert_eq!(positions.len(), 4);

        for position in positions {
            assert!(position < 1024);
        }
    }
    #[test]
    fn insert_sets_bits() {
        let hasher = Xxhash::new(42);

        let mut filter = BloomFilter::new(
            1024,
            4,
            32,
            hasher,
        );

        let positions: Vec<usize> =
            filter.positions(b"manglu").collect();

        filter.insert(b"manglu");

        for position in positions {
            assert!(filter.bits.get(position));
        }
    }
    #[test]
    fn inserting_same_element_creates_collisions() {
        let hasher = Xxhash::new(42);

        let mut filter = BloomFilter::new(
            1024,
            4,
            32,
            hasher,
        );

        filter.insert(b"manglu");
        filter.insert(b"manglu");

        let positions: Vec<usize> =
            filter.positions(b"manglu").collect();

        for position in positions {
            let region = filter.region_for(position);

            assert!(filter.collisions.get(region));
        }
    }
    #[test]
    fn contains_inserted_element() {
        let hasher = Xxhash::new(42);

        let mut filter = BloomFilter::new(
            1024,
            4,
            32,
            hasher,
        );

        filter.insert(b"manglu");

        assert!(filter.contains(b"manglu"));
    }
    #[test]
    fn contains_definitely_absent_element() {
        let hasher = Xxhash::new(42);

        let mut filter = BloomFilter::new(
            1024,
            4,
            32,
            hasher,
        );

        filter.insert(b"manglu");

        assert!(!filter.contains(b"akshay"));
    }
    #[test]
    fn delete_inserted_element() {
        let hasher = Xxhash::new(42);

        let mut filter = BloomFilter::new(
            1024,
            4,
            32,
            hasher,
        );

        filter.insert(b"manglu");

        assert!(filter.contains(b"manglu"));

        let result = filter.delete(b"manglu");

        assert!(matches!(result, DeleteResult::Deleted));
        assert!(!filter.contains(b"manglu"));
    }
    #[test]
    fn delete_missing_element() {
        let hasher = Xxhash::new(42);

        let mut filter = BloomFilter::new(
            1024,
            4,
            32,
            hasher,
        );

        let result = filter.delete(b"manglu");

        assert!(matches!(result, DeleteResult::NotFound));
    }
    #[test]
    fn delete_returns_unsafe_when_region_has_collision() {
        let hasher = Xxhash::new(42);

        let mut filter = BloomFilter::new(
            64,
            1,
            1,
            hasher,
        );

        filter.insert(b"manglu");

        let first_position = filter.positions(b"manglu").next().unwrap();

        // Find another value that maps to the same bit.
        let mut other = 0u64;

        loop {
            let candidate = format!("candidate-{other}");

            let position = filter.positions(candidate.as_bytes()).next().unwrap();

            if position == first_position {
                filter.insert(candidate.as_bytes());

                let result = filter.delete(b"manglu");

                assert!(matches!(result, DeleteResult::UnsafeToDelete));

                break;
            }

            other += 1;
        }
    }
    #[test]
    fn deleting_one_element_does_not_remove_another() {
        let hasher = Xxhash::new(42);

        let mut filter = BloomFilter::new(
            1024,
            4,
            32,
            hasher,
        );

        filter.insert(b"manglu");
        filter.insert(b"akshay");

        assert!(filter.contains(b"manglu"));
        assert!(filter.contains(b"akshay"));

        let result = filter.delete(b"manglu");

        if matches!(result, DeleteResult::Deleted) {
            assert!(filter.contains(b"akshay"));
        }
    }
}