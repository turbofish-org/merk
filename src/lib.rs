#![feature(map_first_last)]
#![feature(trivial_bounds)]

#[global_allocator]
#[cfg(feature = "jemallocator")]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

#[cfg(feature = "full")]
pub use rocksdb;

/// Error and Result types.
mod error;
/// The top-level store API.
#[cfg(feature = "full")]
mod merk;
/// Provides a container type that allows temporarily taking ownership of a value.
// TODO: move this into its own crate
pub mod owner;
/// Algorithms for generating and verifying Merkle proofs.
pub mod proofs;

/// Various helpers useful for tests or benchmarks.
#[cfg(feature = "full")]
pub mod test_utils;
/// The core tree data structure.
pub mod tree;

#[cfg(feature = "full")]
pub use crate::merk::{chunks, restore, Merk};

pub use error::{Error, Result};
pub use tree::{Batch, BatchEntry, Hash, Op, PanicSource, HASH_LENGTH};

#[allow(deprecated)]
pub use proofs::query::verify_query;

pub use proofs::query::verify;
