#[macro_use]
extern crate error_chain;
extern crate blake2_rfc;
extern crate rocksdb;

mod error;
pub mod tree;
mod ops;
mod merk;

pub use error::{Error, Result};
pub use merk::Merk;
