//! A high-performance Merkle key/value store.
//!
//! Merk is a crypto key/value store - more specifically, it's a Merkle AVL tree
//! built on top of RocksDB (Facebook's fork of LevelDB).
//!
//! Its priorities are performance and reliability. While Merk was designed to
//! be the state database for blockchains, it can also be used anywhere an
//! auditable key/value store is needed.

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
/// Provides a container type that allows temporarily taking ownership of a
/// value.
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
pub use crate::merk::{chunks, restore, snapshot, Merk, MerkSource, Snapshot};

pub use error::{Error, Result};
pub use tree::{Batch, BatchEntry, Hash, Op, PanicSource, HASH_LENGTH};

#[allow(deprecated)]
pub use proofs::query::verify_query;

pub use proofs::query::verify;
pub use proofs::query::execute_proof;
