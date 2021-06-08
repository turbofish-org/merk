pub mod chunk;
pub mod encoding;
pub mod query;
pub mod tree;

use crate::tree::Hash;

pub use encoding::{encode_into, Decoder};
pub use query::Query;
pub use tree::Tree;

/// A proof operator, executed to verify the data in a Merkle proof.
#[derive(Debug, PartialEq)]
pub enum Op {
    /// Pushes a node on the stack.
    Push(Node),

    /// Pops the top stack item as `parent`. Pops the next top stack item as
    /// `child`. Attaches `child` as the left child of `parent`. Pushes the
    /// updated `parent` back on the stack.
    Parent,

    /// Pops the top stack item as `child`. Pops the next top stack item as
    /// `parent`. Attaches `child` as the right child of `parent`. Pushes the
    /// updated `parent` back on the stack.
    Child,
}

/// A selected piece of data about a single tree node, to be contained in a
/// `Push` operator in a proof.
#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    /// Represents the hash of a tree node.
    Hash(Hash),

    /// Represents the hash of the key/value pair of a tree node.
    KVHash(Hash),

    /// Represents the key and value of a tree node.
    KV(Vec<u8>, Vec<u8>),
}
