#[macro_use]
extern crate error_chain;
extern crate blake2_rfc;
extern crate rocksdb;
extern crate colored;
extern crate byteorder;
extern crate rand;
extern crate jemallocator;

#[global_allocator] 
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

/// Error and Result types.
mod error;
/// The top-level store API.
mod merk;
/// The core tree data structure.
mod tree;
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

