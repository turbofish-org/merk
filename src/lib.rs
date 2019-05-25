#![feature(trait_alias)]

#[macro_use]
extern crate error_chain;
#[macro_use]
extern crate serde_derive;
extern crate rocksdb;
extern crate bincode;
extern crate num_cpus;
extern crate byteorder;
extern crate blake2_rfc;
extern crate hex;
extern crate serde;
extern crate colored;

mod node;
mod sparse_tree;
mod merk;
mod error;

// collect all internal module exports and re-export as root module
pub use crate::node::*;
pub use crate::sparse_tree::*;
pub use crate::merk::*;
pub use crate::error::*;
