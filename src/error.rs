pub use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Attach Error: {0}")]
    Attach(String),
    #[error("Batch Key Error: {0}")]
    BatchKey(String),
    #[error("Bound Error: {0}")]
    Bound(String),
    #[error("Chunk Processing Error: {0}")]
    ChunkProcessing(String),
    #[error(transparent)]
    Ed(#[from] ed::Error),
    #[error("Fetch Error: {0}")]
    Fetch(String),
    #[error("Proof did not match expected hash\n\tExpected: {0:?}\n\tActual: {1:?}")]
    HashMismatch([u8; 32], [u8; 32]),
    #[error("Index OoB Error: {0}")]
    IndexOutOfBounds(String),
    #[error(transparent)]
    IO(#[from] std::io::Error),
    #[error("Tried to delete non-existent key {0:?}")]
    KeyDelete(Vec<u8>),
    #[error("Key Error: {0}")]
    Key(String),
    #[error("Key not found: {0}")]
    KeyNotFound(String),
    #[error("Proof is missing data for query")]
    MissingData,
    #[error("Path Error: {0}")]
    Path(String),
    #[error("Proof Error: {0}")]
    Proof(String),
    #[cfg(feature = "full")]
    #[error(transparent)]
    RocksDB(#[from] rocksdb::Error),
    #[error("Stack Underflow")]
    StackUnderflow,
    #[error("Tree Error: {0}")]
    Tree(String),
    #[error("Unexpected Node Error: {0}")]
    UnexpectedNode(String),
    #[error("Unknown Error")]
    Unknown,
}

pub type Result<T> = std::result::Result<T, Error>;
