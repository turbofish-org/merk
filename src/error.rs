pub use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    RocksDBError(#[from] rocksdb::Error),
    #[error("Index OoB Error: {0}")]
    IndexOutOfBounds(String),
    #[error("Fetch Error: {0}")]
    FetchError(String),
    #[error("Key not found: {0}")]
    KeyNotFound(String),
    #[error("Batch Key Error: {0}")]
    BatchKey(String),
    #[error("Key Error: {0}")]
    KeyError(String),
    #[error("Path Error: {0}")]
    PathError(String),
    #[error("Chunk Processing Error: {0}")]
    ChunkProcessingError(String),
    #[error("Proof did not match expected hash\n\tExpected: {0}\n\tActual: {1}")]
    ProofHashMismatch(String, String),
    #[error("Proof Error: {0}")]
    ProofError(String),
    #[error("Leaf Chunk proof did not match expected hash\n\tExpected: {0}\n\tActual: {1}")]
    LeafChunkHashMismatch(String, String),
    #[error("Query Error: {0}")]
    QueryError(String),
    #[error("Bound Error: {0}")]
    BoundError(String),
    #[error("Tree Error: {0}")]
    TreeError(String),
    #[error("Unexpected Node Error: {0}")]
    UnexpectedNodeError(String),
    #[error("Attach Error: {0}")]
    AttachError(String),
    #[error("Tried to delete non-existent key {0}")]
    KeyDeleteError(String),
    #[error(transparent)]
    IOError(#[from] std::io::Error),
    #[error("Stack Underflow")]
    StackUnderflow,
    #[error("Unknown Error")]
    Unknown,
}

pub type Result<T> = std::result::Result<T, Error>;
