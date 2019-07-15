const HASH_LENGTH = 20;
type Hash = [u8; HASH_LENGTH];

type Key = Vec<u8>;

pub struct Link {
    key: Key,
    hash: Option<Hash>,
    height: u8,
    pending_writes: u64,
    child: Option<Box<Node>>
}

pub struct Node {
    key: Key,
    value: Vec<u8>,
    kv_hash: Hash,
}