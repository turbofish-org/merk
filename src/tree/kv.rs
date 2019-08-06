use super::hash::{Hash, kv_hash};

// TODO: maybe use something similar to Vec but without capacity field,
//       (should save 16 bytes per entry). also, maybe a shorter length
//       field to save even more. also might be possible to combine key
//       field and value field.

pub struct KV {
    key: Vec<u8>,
    value: Vec<u8>,
    hash: Hash
}

impl KV {
    #[inline]
    pub fn new(key: Vec<u8>, value: Vec<u8>) -> Self {
        // TODO: length checks?
        let hash = kv_hash(key.as_slice(), value.as_slice());
        KV { key, value, hash }
    }

    #[inline]
    pub fn with_value(mut self, value: Vec<u8>) -> Self {
        // TODO: length check?
        self.value = value;
        self.hash = kv_hash(self.key(), self.value());
        self
    }

    #[inline]
    pub fn key(&self) -> &[u8] {
        self.key.as_slice()
    }
    
    #[inline]
    pub fn value(&self) -> &[u8] {
        self.value.as_slice()
    }

    #[inline]
    pub fn hash(&self) -> &Hash {
        &self.hash
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn new_kv() {
        let kv = KV::new(vec![1, 2, 3], vec![4, 5, 6]);

        assert_eq!(kv.key(), &[1, 2, 3]);
        assert_eq!(kv.value(), &[4, 5, 6]);
        assert_ne!(kv.hash(), &super::super::hash::NULL_HASH);
    }

    #[test]
    fn with_value() {
        let kv = KV::new(vec![1, 2, 3], vec![4, 5, 6])
            .with_value(vec![7, 8, 9]);

        assert_eq!(kv.key(), &[1, 2, 3]);
        assert_eq!(kv.value(), &[4, 5, 6]);
        assert_ne!(kv.hash(), &super::super::hash::NULL_HASH);
    }
}
