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
pub mod tree;
mod merk;
pub mod test_utils;
pub mod owner;

pub use error::{Error, Result};
pub use self::merk::Merk;
pub use tree::{Batch, BatchEntry, Op, PanicSource};
