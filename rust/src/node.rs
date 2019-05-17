extern crate byteorder;
extern crate blake2_rfc;
extern crate hex;
extern crate serde;

use std::fmt;
use std::cmp::max;

use blake2_rfc::blake2b::Blake2b;
use byteorder::{ByteOrder, BigEndian};
use serde::{Serialize, Deserialize};

const HASH_LENGTH: usize = 20;
type Hash = [u8; HASH_LENGTH];
const NULL_HASH: Hash = [0 as u8; HASH_LENGTH];

#[derive(Serialize, Deserialize)]
pub struct Child {
    pub key: Vec<u8>,
    pub hash: Hash,
    pub height: u8
}

/// Represents a tree node, and provides methods for working with
/// the tree structure stored in a database.
#[derive(Serialize, Deserialize)]
pub struct Node {
    pub key: Vec<u8>,
    pub value: Vec<u8>,
    pub kv_hash: Hash,
    pub parent_key: Option<Vec<u8>>,
    pub left: Option<Child>,
    pub right: Option<Child>
}

///
impl Node {
    /// Creates a new node from a key and value.
    pub fn new(key: &[u8], value: &[u8]) -> Node {
        Node{
            key: key.to_vec(),
            value: value.to_vec(),
            kv_hash: Default::default(),
            parent_key: None,
            left: None,
            right: None
        }
    }

    pub fn decode(bytes: &[u8]) -> bincode::Result<Node> {
        bincode::deserialize(bytes)
    }

    pub fn update_kv_hash (&mut self) {
        // TODO: make generic to allow other hashers
        let mut hasher = Blake2b::new(HASH_LENGTH);

        hasher.update(&[ self.key.len() as u8 ]);
        hasher.update(&self.key);

        let mut val_length = [0; 2];
        BigEndian::write_u16(&mut val_length, self.value.len() as u16);
        hasher.update(&val_length);

        hasher.update(&self.value);

        let res = hasher.finalize();
        self.kv_hash.copy_from_slice(res.as_bytes());
    }

    pub fn hash (&self) -> Hash {
        // TODO: make generic to allow other hashers
        let mut hasher = Blake2b::new(HASH_LENGTH);
        hasher.update(&self.kv_hash);
        hasher.update(match &self.left {
            Some(left) => &(left.hash),
            None => &NULL_HASH
        });
        hasher.update(match &self.right {
            Some(right) => &(right.hash),
            None => &NULL_HASH
        });
        let res = hasher.finalize();
        let mut hash: Hash = Default::default();
        hash.copy_from_slice(res.as_bytes());
        hash
    }

    pub fn child(&self, left: bool) -> &Option<Child> {
        if left { &self.left } else { &self.right }
    }

    pub fn child_height(&self, left: bool) -> u8 {
        let child = self.child(left);
        match child {
            Some(child) => child.height,
            None => 0
        }
    }

    pub fn height(&self) -> u8 {
        max(
            self.child_height(true),
            self.child_height(false)
        ) + 1
    }

    pub fn to_child(&self) -> Child {
        Child{
            key: self.key.to_vec(),
            hash: self.hash(),
            height: self.height()
        }
    }

    pub fn set_child(&mut self, left: bool, child_node: &mut Node) {
        let child = Some(child_node.to_child());
        if left {
            self.left = child;
        } else {
            self.right = child;
        }

        child_node.parent_key = Some(self.key.to_vec());
    }

    pub fn encode(&self) -> bincode::Result<Vec<u8>> {
        bincode::serialize(&self)
    }

    // pub fn put(
    //     &mut self,
    //     key: &'a [u8],
    //     value: &'a [u8]
    // ) -> Result<(), Error> {
    //     if self.key == key {
    //         // same key, just update the value of this node
    //         self.valu = value;
    //         // self.calculate_kv_hash();
    //         // self.calculate_hash(&left_hash, &right_hash);
    //         // self.save()?;
    //         return Ok(());
    //     }
    //
    //     let left = key < self.key;
    //     println!("left: {:?}", left);
    //     let child = self.child(left);
    //     return Ok(());
    // }
    //
    // pub fn child(&self, left: bool) -> Result<Option<Node<'a, S>>, Error> {
    //     let key = if left {
    //         self.left_key
    //     } else {
    //         self.right_key
    //     };
    //     match key {
    //         None => Ok(None),
    //         Some(key) => {
    //             let value = self.store.get(key)??;
    //             let mut value2 = vec![0 as u8; value.len()];
    //             value2.copy_from_slice(&value[..]);
    //             let child_raw = RawNode::from_bytes(&mut value2)?;
    //             let child = Node::from_raw(self.store, key, &child_raw);
    //             Ok(Some(child))
    //         }
    //     }
    // }
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "({:?}: {:?}, h{:?})",
            String::from_utf8(self.key.to_vec()).unwrap(),
            String::from_utf8(self.value.to_vec()).unwrap(),
            hex::encode(self.hash())
        )
    }
}

// pub struct MockStore {
//
// }
// impl Store for MockStore {
//     fn get(&self, key: &[u8]) -> Result<Option<&[u8]>, Error> {
//         Ok(Some(&[]))
//     }
//
//     fn put(&self, key: &[u8], value: &[u8]) -> Result<(), Error> {
//         Ok(())
//     }
// }

#[cfg(test)]
mod tests {
    use crate::node::*;

    #[test]
    fn it_works() {
        let mut node = Node::new(b"foo", b"bar");
        node.update_kv_hash();
        println!("node: {:?}", node);
        println!("encoded length: {:?}", node.encode().unwrap().len());

        let node2 = Node::decode(&node.encode().unwrap()[..]);
        println!("node2: {:?}", node2);

        let mut node3 = Node::new(b"foo2", b"bar2");
        node.set_child(true, &mut node3);

        println!("node: {:?}", node);
    }
}
