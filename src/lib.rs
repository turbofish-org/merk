#![feature(map_first_last)]

#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

#[cfg(feature = "full")]
pub use rocksdb;

/// Error and Result types.
#[cfg(feature = "full")]
mod error;
/// The top-level store API.
#[cfg(feature = "full")]
mod merk;
/// Provides a container type that allows temporarily taking ownership of a value.
// TODO: move this into its own crate
#[cfg(feature = "full")]
pub mod owner;
/// Algorithms for generating and verifying Merkle proofs.
#[cfg(feature = "full")]
pub mod proofs;
/// Various helpers useful for tests or benchmarks.
#[cfg(feature = "full")]
pub mod test_utils;
/// The core tree data structure.
#[cfg(feature = "full")]
pub mod tree;

#[cfg(feature = "full")]
pub use crate::merk::{chunks, restore, Merk};

#[cfg(feature = "full")]
pub use error::{Error, Result};
#[cfg(feature = "full")]
pub use tree::{Batch, BatchEntry, Hash, Op, PanicSource, HASH_LENGTH};

#[allow(deprecated)]
#[cfg(feature = "full")]
pub use proofs::query::verify_query;
