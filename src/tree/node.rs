use std::cmp::max;
use std::fmt;

use crate::error::*;

use super::hash::*;

pub type Hash = [u8; 20];

/// Represents a reference to another tree node.
///
/// Note that the referenced node is not necessarily
/// loaded in memory, but can be fetched from a backing
/// database by its key.
#[derive(Clone, PartialEq)]
pub struct Link {
    pub key: Vec<u8>,
    pub hash: Hash,
    pub height: u8
}

// impl fmt::Debug for Link {
//     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//         write!(
//             f,
//             "(key={}, hash={}, height={})",
//             String::from_utf8(self.key.to_vec()).unwrap(),
//             hex::encode(&self.hash[0..3]),
//             self.height
//         )
//     }
// }

// TODO: enforce maximum key/value lengths, to prevent DoS (e.g. when verifying
// a proof)

/// Represents a tree node and its associated key/value pair.
#[derive(Clone, PartialEq)]
pub struct Node {
    // don't serialize key since it's implied from the db
    // #[serde(skip)]
    pub key: Vec<u8>,
    pub value: Vec<u8>,
    pub kv_hash: Hash,
    pub left: Option<Link>,
    pub right: Option<Link>
}

impl Node {
    /// Creates a new node from a key and value.
    // TODO: take Vec<u8>s for simpler top-level APIs
    pub fn new(key: &[u8], value: &[u8]) -> Node {
        let mut node = Node {
            key: key.to_vec(),
            value: value.to_vec(),
            kv_hash: Default::default(),
            left: None,
            right: None,
        };
        node.update_kv_hash();
        node
    }

    // pub fn decode(key: &[u8], bytes: &[u8]) -> Result<Node> {
    //     let node = bincode::deserialize(bytes).map(|mut node: Node| {
    //         set_vec(&mut node.key, key);
    //         node
    //     })?;
    //     Ok(node)
    // }

    pub fn update_kv_hash(&mut self) {
        self.kv_hash = kv_hash(&self.key, &self.value);
    }

    pub fn hash(&self) -> Hash {
        hash(
           &self.kv_hash,
           self.left.as_ref().map(|l| &l.hash),
           self.right.as_ref().map(|r| &r.hash)
        ) 
    }

    #[inline]
    pub fn child_link(&self, left: bool) -> &Option<Link> {
        if left {
            &self.left
        } else {
            &self.right
        }
    }

    pub fn child_height(&self, left: bool) -> u8 {
        let link = self.child_link(left);
        match link {
            Some(link) => link.height,
            None => 0,
        }
    }

    pub fn height(&self) -> u8 {
        max(self.child_height(true), self.child_height(false)) + 1
    }

    pub fn balance_factor(&self) -> i8 {
        self.child_height(false) as i8 - self.child_height(true) as i8
    }

    #[inline]
    pub fn as_link(&self) -> Link {
        Link {
            key: self.key.to_vec(),
            hash: self.hash(),
            height: self.height(),
        }
    }
    
    #[inline]
    pub fn link_to(&mut self, left: bool, child: Option<&Node>) {
        let link = child.as_ref().map(|c| c.as_link());
        if left {
            self.left = link;
        } else {
            self.right = link;
        }
    }

    #[inline]
    pub fn set_value(&mut self, value: &[u8]) {
        set_vec(&mut self.value, value);
        self.update_kv_hash();
    }

    // pub fn encode(&self) -> Result<Vec<u8>> {
    //     let bytes = bincode::serialize(&self)?;
    //     Ok(bytes)
    // }

    #[inline]
    pub fn is_leaf(&self) -> bool {
        !(self.left.is_some() || self.right.is_some())
    }
}

fn set_vec<T: Copy>(vec: &mut Vec<T>, value: &[T]) {
    vec.clear();
    vec.extend_from_slice(value);
}

// impl fmt::Debug for Node {
//     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//         write!(
//             f,
//             "({:?}: {:?}, hash={}, height={})",
//             String::from_utf8(self.key.to_vec())
//                 .unwrap_or_else(|_| format!("{:?}", &self.key)),
//             String::from_utf8(self.value.to_vec())
//                 .unwrap_or_else(|_| format!("{:?}", &self.value)),
//             hex::encode(&self.hash()[..3]),
//             self.height()
//         )
//     }
// }
