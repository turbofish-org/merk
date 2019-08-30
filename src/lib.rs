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

mod error;
mod merk;
mod tree;
mod proofs;

/// Various helpers useful for tests or benchmarks.
pub mod test_utils;
/// Provides a container type that allows temporarily taking ownership of a value.
// TODO: move this into its own crate
pub mod owner;

pub use error::{Error, Result};
pub use self::merk::Merk;
pub use tree::{Batch, BatchEntry, Op, PanicSource};
