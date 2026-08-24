use crate::bit_array::BitArray;
use crate::hasher::Hasher;

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

impl<H: Hasher> BloomFilter<H> {
    pub fn new(m: usize, k: usize, r: usize, hasher: H) -> Self {
        assert!(m > 0);
        assert!(k > 0);
        assert!(r > 0);
        assert!(m % r == 0);

        Self {
            bits: BitArray::new(m),
            collisions: BitArray::new(r),
            m,
            k,
            r,
            hasher,
        }
    }
    // Enhanced double hashing:
    // An optimisation referenced from Rocks DB
    // h_i = h1 + i*h2 + i(i-1)/2
    //
    // The additional quadratic/triangular term prevents the probe
    // sequence from being a simple linear progression and reduces
    // systematic correlations between generated positions.
    fn positions(&self, data: &[u8]) -> impl Iterator<Item = usize> + use<H> {
        let (h1, h2) = self.hasher.hash(data);
        let k = self.k;
        let m = self.m as u64;

        (0..k).map(move |i| {
            let i = i as u64;
            let quadratic_term = (i * (i.wrapping_sub(1))) / 2;
            let hash = h1
                .wrapping_add(i.wrapping_mul(h2))
                .wrapping_add(quadratic_term);

            (hash % m) as usize
        })
    }
    fn region_for(&self, position: usize) -> usize {
        let region_size = self.m / self.r;

        position / region_size
    }
    pub fn insert(&mut self, data: &[u8]) {
        for position in self.positions(data) {
            let region = self.region_for(position);

            if self.bits.get(position) {
                self.collisions.set(region);
            }

            self.bits.set(position);
        }
    }
    pub fn contains(&self, data: &[u8]) -> bool {
        self.positions(data).all(|position| self.bits.get(position))
    }
    pub fn delete(&mut self, data: &[u8]) -> DeleteResult {
        // First make sure the element might actually exist.
        if !self.contains(data) {
            return DeleteResult::NotFound;
        }

        // Find a bit that is in a collision-free region.
        for position in self.positions(data) {
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
    fn insert_sets_bits() {
        let hasher = Xxhash::new(42);

        let mut filter = BloomFilter::new(1024, 4, 32, hasher);

        let positions: Vec<usize> = filter.positions(b"manglu").collect();

        filter.insert(b"manglu");

        for position in positions {
            assert!(filter.bits.get(position));
        }
    }
    #[test]
    fn inserting_same_element_creates_collisions() {
        let hasher = Xxhash::new(42);

        let mut filter = BloomFilter::new(1024, 4, 32, hasher);

        filter.insert(b"manglu");
        filter.insert(b"manglu");

        let positions: Vec<usize> = filter.positions(b"manglu").collect();

        for position in positions {
            let region = filter.region_for(position);

            assert!(filter.collisions.get(region));
        }
    }
    #[test]
    fn contains_inserted_element() {
        let hasher = Xxhash::new(42);

        let mut filter = BloomFilter::new(1024, 4, 32, hasher);

        filter.insert(b"manglu");

        assert!(filter.contains(b"manglu"));
    }
    #[test]
    fn contains_definitely_absent_element() {
        let hasher = Xxhash::new(42);

        let mut filter = BloomFilter::new(1024, 4, 32, hasher);

        filter.insert(b"manglu");

        assert!(!filter.contains(b"akshay"));
    }
    #[test]
    fn delete_inserted_element() {
        let hasher = Xxhash::new(42);

        let mut filter = BloomFilter::new(1024, 4, 32, hasher);

        filter.insert(b"manglu");

        assert!(filter.contains(b"manglu"));

        let result = filter.delete(b"manglu");

        assert!(matches!(result, DeleteResult::Deleted));
        assert!(!filter.contains(b"manglu"));
    }
    #[test]
    fn delete_missing_element() {
        let hasher = Xxhash::new(42);

        let mut filter = BloomFilter::new(1024, 4, 32, hasher);

        let result = filter.delete(b"manglu");

        assert!(matches!(result, DeleteResult::NotFound));
    }
    #[test]
    fn delete_returns_unsafe_when_region_has_collision() {
        let hasher = Xxhash::new(42);

        let mut filter = BloomFilter::new(64, 1, 1, hasher);

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

        let mut filter = BloomFilter::new(1024, 4, 32, hasher);

        filter.insert(b"manglu");
        filter.insert(b"akshay");

        assert!(filter.contains(b"manglu"));
        assert!(filter.contains(b"akshay"));

        let result = filter.delete(b"manglu");

        if matches!(result, DeleteResult::Deleted) {
            assert!(filter.contains(b"akshay"));
        }
    }
    #[test]
    fn generates_k_positions() {
        let hasher = Xxhash::new(42);

        let filter = BloomFilter::new(1024, 4, 32, hasher);

        let positions: Vec<usize> = filter.positions(b"manglu").collect();

        assert_eq!(positions.len(), 4);

        for position in positions {
            assert!(position < 1024);
        }
    }
    #[test]
    fn positions_are_unique() {
        let hasher = Xxhash::new(42);

        let filter = BloomFilter::new(1024, 8, 32, hasher);

        let positions: Vec<usize> = filter.positions(b"manglu").collect();

        let unique_count = {
            let mut unique = positions.clone();
            unique.sort_unstable();
            unique.dedup();
            unique.len()
        };

        assert_eq!(unique_count, positions.len());
    }

    fn normal_positions(h1: u64, h2: u64, k: usize, m: usize) -> Vec<usize> {
        (0..k)
            .map(|i| {
                let hash = h1.wrapping_add((i as u64).wrapping_mul(h2));

                (hash % m as u64) as usize
            })
            .collect()
    }
    #[test]
    fn enhanced_hashing_changes_probe_sequence() {
        let hasher = Xxhash::new(42);

        let filter = BloomFilter::new(1024, 8, 32, hasher);

        let (h1, h2) = filter.hasher.hash(b"manglu");

        let normal = normal_positions(h1, h2, filter.k, filter.m);

        let enhanced: Vec<usize> = filter.positions(b"manglu").collect();

        assert_ne!(normal, enhanced);
    }

    #[test]
    fn zero_false_negatives_on_large_set() {
        let hasher = Xxhash::new(12345);
        let mut filter = BloomFilter::new(100_000, 7, 1000, hasher);

        let count = 5_000;
        let elements: Vec<Vec<u8>> = (0..count)
            .map(|i| format!("item-{i}").into_bytes())
            .collect();

        for el in &elements {
            filter.insert(el);
        }

        for el in &elements {
            assert!(
                filter.contains(el),
                "False negative detected! Bloom filters must never produce false negatives."
            );
        }
    }

    #[test]
    fn deletions_never_corrupt_surviving_elements() {
        let hasher = Xxhash::new(999);
        let mut filter = BloomFilter::new(100_000, 7, 1000, hasher);

        let total = 2_000;
        let elements: Vec<Vec<u8>> = (0..total)
            .map(|i| format!("key-{i}").into_bytes())
            .collect();

        for el in &elements {
            filter.insert(el);
        }

        let to_delete = &elements[0..1_000];
        let to_keep = &elements[1_000..total];

        let mut deleted_count = 0;
        for el in to_delete {
            if matches!(filter.delete(el), DeleteResult::Deleted) {
                deleted_count += 1;
                assert!(!filter.contains(el), "Deleted element should no longer be present");
            }
        }

        // Deletions must never cause false negatives in surviving elements
        for el in to_keep {
            assert!(
                filter.contains(el),
                "Surviving element was corrupted by deletions of other elements!"
            );
        }

        assert!(deleted_count > 0, "Expected some elements to be safely deleted");
    }
}
