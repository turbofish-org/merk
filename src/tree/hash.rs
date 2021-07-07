use std::convert::TryFrom;

/// The length of a `Hash` (in bytes).
pub const HASH_LENGTH: usize = 32;

/// A zero-filled `Hash`.
pub const NULL_HASH: Hash = [0; HASH_LENGTH];

/// A cryptographic hash digest.
pub type Hash = [u8; HASH_LENGTH];

/// Hashes a key/value pair.
///
/// **NOTE:** This will panic if the key is longer than 255 bytes, or the value
/// is longer than 65,535 bytes.
pub fn kv_hash(key: &[u8], value: &[u8]) -> Hash {
    // TODO: result instead of panic
    // TODO: make generic to allow other hashers
    let mut hasher = blake3::Hasher::new();
    // panics if key is longer than 255!
    let key_length = u8::try_from(key.len()).expect("key must be less than 256 bytes");
    hasher.update(&key_length.to_be_bytes());
    hasher.update(key);

    // panics if value is longer than 65535!
    let val_length = u16::try_from(value.len()).expect("value must be less than 65,536 bytes");
    hasher.update(&val_length.to_be_bytes());
    hasher.update(value);

    let res = hasher.finalize();
    let mut hash: Hash = Default::default();
    hash.copy_from_slice(res.as_bytes());
    hash
}

/// Hashes a node based on the hash of its key/value pair, the hash of its left
/// child (if any), and the hash of its right child (if any).
pub fn node_hash(kv: &Hash, left: &Hash, right: &Hash) -> Hash {
    // TODO: make generic to allow other hashers
    let mut hasher = blake3::Hasher::new();
    hasher.update(kv);
    hasher.update(left);
    hasher.update(right);

    let res = hasher.finalize();
    let mut hash: Hash = Default::default();
    hash.copy_from_slice(res.as_bytes());
    hash
}
