pub trait Hasher {
    fn hash(&self, data: &[u8]) -> (u64, u64);
}