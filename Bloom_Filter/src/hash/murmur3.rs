use std::io::Cursor;

use crate::hasher::Hasher;

pub struct Murmur3{
    seed:u32,
}

impl Murmur3{
    pub fn new(seed: u32)->Self{
        Self{ seed }
    }
}

impl Hasher for Murmur3{
    fn hash(&self, data: &[u8])->(u64,u64){
        let mut cursor = Cursor::new(data);

        let hash = murmur3::murmur3_x64_128(&mut cursor, self.seed).expect("Murmur3 hashing failed");

        let h1 = hash as u64;
        let h2 = (hash >> 64) as u64;
        (h1, h2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_input_same_hash() {
        let hasher = Murmur3::new(42);

        let first = hasher.hash(b"manglu");
        let second = hasher.hash(b"manglu");

        assert_eq!(first, second);
    }
}