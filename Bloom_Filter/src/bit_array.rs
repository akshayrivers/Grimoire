pub struct BitArray {
    bits: Vec<u64>,
}

impl BitArray {
    pub fn new(size: usize) -> Self {
        let words = size.div_ceil(64);

        Self {
            bits: vec![0; words],
        }
    }
    pub fn set(&mut self, idx: usize) {
        let word_idx = idx / 64;
        let bit_idx = idx % 64;

        self.bits[word_idx] |= 1u64 << bit_idx;
    }
    pub fn get(&self, idx: usize) -> bool {
        let word_idx = idx / 64;
        let bit_idx = idx % 64;

        (self.bits[word_idx] & (1u64 << bit_idx)) != 0
    }
    pub fn clear(&mut self, idx: usize) {
        let word_idx = idx / 64;
        let bit_idx = idx % 64;

        self.bits[word_idx] &= !(1u64 << bit_idx);
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_get_clear() {
        let mut bits = BitArray::new(1024);

        bits.set(137);
        assert!(bits.get(137));

        bits.clear(137);
        assert!(!bits.get(137));
    }
}
