#![feature(trait_alias)]

#[macro_use]
extern crate error_chain;
#[macro_use]
extern crate serde_derive;
extern crate bincode;
extern crate blake2_rfc;
extern crate colored;
extern crate hex;
extern crate num_cpus;
extern crate rocksdb;
extern crate serde;

mod util;

mod test_util;
mod ops;
mod proof;
mod tree;
mod error;
mod merk;
mod fetch;

// collect all internal module exports and re-export as root module
pub use crate::error::*;
pub use crate::merk::*;
pub use crate::tree::*;
pub use crate::proof::*;
