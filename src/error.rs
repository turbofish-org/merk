pub use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Attach Error: {0}")]
    AttachError(String),
    #[error("Batch Key Error: {0}")]
    BatchKey(String),
    #[error("Bound Error: {0}")]
    BoundError(String),
    #[error("Chunk Processing Error: {0}")]
    ChunkProcessingError(String),
    #[error(transparent)]
    EdError(#[from] ed::Error),
    #[error("Fetch Error: {0}")]
    FetchError(String),
    #[error("Index OoB Error: {0}")]
    IndexOutOfBounds(String),
    #[error(transparent)]
    IOError(#[from] std::io::Error),
    #[error("Tried to delete non-existent key {0:?}")]
    KeyDeleteError(Vec<u8>),
    #[error("Key Error: {0}")]
    KeyError(String),
    #[error("Key not found: {0}")]
    KeyNotFound(String),
    #[error("Leaf Chunk proof did not match expected hash\n\tExpected: {0:?}\n\tActual: {1:?}")]
    LeafChunkHashMismatch([u8; 32], [u8; 32]),
    #[error("Path Error: {0}")]
    PathError(String),
    #[error("Proof Error: {0}")]
    ProofError(String),
    #[error("Proof did not match expected hash\n\tExpected: {0:?}\n\tActual: {1:?}")]
    ProofHashMismatch([u8; 32], [u8; 32]),
    #[error("Query Error: {0}")]
    QueryError(String),
    #[error(transparent)]
    RocksDBError(#[from] rocksdb::Error),
    #[error("Stack Underflow")]
    StackUnderflow,
    #[error("Tree Error: {0}")]
    TreeError(String),
    #[error("Unexpected Node Error: {0}")]
    UnexpectedNodeError(String),
    #[error("Unknown Error")]
    Unknown,
}

pub type Result<T> = std::result::Result<T, Error>;
