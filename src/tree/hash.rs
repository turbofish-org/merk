use sha2::{Digest, Sha512_256};
use std::{convert::TryFrom, num::TryFromIntError};

/// The hash algorithm used for both KV hashes and node hashes.
pub type Hasher = Sha512_256;

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
pub fn kv_hash(key: &[u8], value: &[u8]) -> Result<Hash, TryFromIntError> {
    // TODO: result instead of panic
    // TODO: make generic to allow other hashers
    let mut hasher = Hasher::new();
    hasher.update(&[0]);

    u32::try_from(key.len()).and_then(|key| u32::try_from(value.len()).map(|value| (key, value)))
    .map(|(key_length, val_length)| {
        hasher.update(&key_length.to_le_bytes());
        hasher.update(key);

        hasher.update(&val_length.to_le_bytes());
        hasher.update(value);

        let res = hasher.finalize();
        let mut hash: Hash = Default::default();
        hash.copy_from_slice(&res[..]);
        hash
    })
}

/// Hashes a node based on the hash of its key/value pair, the hash of its left
/// child (if any), and the hash of its right child (if any).
pub fn node_hash(kv: &Hash, left: &Hash, right: &Hash) -> Hash {
    // TODO: make generic to allow other hashers
    let mut hasher = Hasher::new();
    hasher.update(&[1]);
    hasher.update(kv);
    hasher.update(left);
    hasher.update(right);

    let res = hasher.finalize();
    let mut hash: Hash = Default::default();
    hash.copy_from_slice(&res[..]);
    hash
}
