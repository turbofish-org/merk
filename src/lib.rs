#[global_allocator] 
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

pub use rocksdb;

/// Error and Result types.
mod error;
/// The top-level store API.
mod merk;
/// The core tree data structure.
pub mod tree;
/// Algorithms for generating and verifying Merkle proofs.
mod proofs;
/// Various helpers useful for tests or benchmarks.
pub mod test_utils;
/// Provides a container type that allows temporarily taking ownership of a value.
// TODO: move this into its own crate
pub mod owner;

pub use error::{Error, Result};
pub use self::merk::Merk;
pub use tree::{
  Batch,
  BatchEntry,
  Op,
  PanicSource,
  Hash,
  HASH_LENGTH
};
pub use proofs::verify as verify_proof;

