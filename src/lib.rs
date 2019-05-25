#![feature(trait_alias)]
#![feature(test)]
#![feature(try_trait)]

#[macro_use]
extern crate error_chain;

#[macro_use]
extern crate serde_derive;

extern crate test;

mod node;
mod sparse_tree;
mod merk;
mod error;

// collect all internal module exports and re-export as root module
pub use crate::node::*;
pub use crate::sparse_tree::*;
pub use crate::merk::*;
pub use crate::error::*;
