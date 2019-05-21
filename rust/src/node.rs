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

type GetNodeFn = fn(link: &Link) -> Node;

#[derive(Serialize, Deserialize)]
pub struct Link {
    pub key: Vec<u8>,
    pub hash: Hash,
    pub height: u8
}

/// Represents a tree node, and provides methods for working with
/// the tree structure stored in a database.
#[derive(Serialize, Deserialize)]
pub struct Node {
    // TODO: don't serialize key since it's implied from the db
    pub key: Vec<u8>,
    pub value: Vec<u8>,
    pub kv_hash: Hash,
    pub parent_key: Option<Vec<u8>>,
    pub left: Option<Link>,
    pub right: Option<Link>
}

/// A selection of connected nodes in a tree.
pub struct SparseTree {
    pub node: Node,
    left: Option<Box<SparseTree>>,
    right: Option<Box<SparseTree>>
}

/// Replaces the value of a `Vec<T>` by cloning into it,
/// possibly not needing to allocate.
fn set_vec<T: Clone>(dest: &mut Vec<T>, src: &[T]) {
    dest.clear();
    dest.extend_from_slice(src);
}

///
impl Node {
    /// Creates a new node from a key and value.
    pub fn new(key: &[u8], value: &[u8]) -> Node {
        let mut node = Node{
            key: key.to_vec(),
            value: value.to_vec(),
            kv_hash: Default::default(),
            parent_key: None,
            left: None,
            right: None
        };
        node.update_kv_hash();
        node
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

    pub fn child_link(&self, left: bool) -> &Option<Link> {
        if left { &self.left } else { &self.right }
    }

    pub fn child_height(&self, left: bool) -> u8 {
        let link = self.child_link(left);
        match link {
            Some(link) => link.height,
            None => 0
        }
    }

    pub fn height(&self) -> u8 {
        max(
            self.child_height(true),
            self.child_height(false)
        ) + 1
    }

    pub fn balance_factor(&self) -> i8 {
        self.child_height(false) as i8 -
        self.child_height(true) as i8
    }

    pub fn to_link(&self) -> Link {
        Link{
            key: self.key.to_vec(),
            hash: self.hash(),
            height: self.height()
        }
    }

    pub fn set_child(&mut self, left: bool, child: &mut Node) {
        let link = Some(child.to_link());
        if left {
            self.left = link;
        } else {
            self.right = link;
        }

        child.parent_key = Some(self.key.to_vec());
    }

    pub fn set_value(&mut self, value: &[u8]) {
        set_vec(&mut self.value, value);
        self.update_kv_hash();
    }

    pub fn encode(&self) -> bincode::Result<Vec<u8>> {
        bincode::serialize(&self)
    }
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "({}: {}, h={}, p={:?})",
            String::from_utf8(self.key.to_vec()).unwrap(),
            String::from_utf8(self.value.to_vec()).unwrap(),
            hex::encode(self.hash()),
            self.parent_key.clone().map(|k| String::from_utf8(k))
        )
    }
}

impl SparseTree {
    pub fn new(node: Node) -> SparseTree {
        SparseTree{
            node,
            left: None,
            right: None
        }
    }

    pub fn put(
        mut self,
        get_node: GetNodeFn,
        key: &[u8],
        value: &[u8]
    ) -> SparseTree {
        if self.node.key == key {
            // same key, just update the value of this node
            self.node.set_value(value);
            return self;
        }

        let left = key < &self.node.key;
        let child = self.get_child(left, get_node);
        let mut child_tree = match child {
            Some(child_node) => {
                // recursively put value under child
                let child_tree = SparseTree::new(child_node);
                child_tree.put(get_node, key, value)
            },
            None => {
                // no child here, create node and set as child
                SparseTree::new(
                    Node::new(key, value)
                )
            }
        };

        self.node.set_child(left, &mut child_tree.node);

        let wrapped = Some(Box::new(child_tree));
        if left {
            self.left = wrapped;
        } else {
            self.right = wrapped;
        }

        // TODO: rebalance if necessary
        // tree.maybe_rebalance(get_node)

        self
    }

    pub fn get_child(
        &self,
        left: bool,
        get_node: GetNodeFn
    ) -> Option<Node> {
        match self.node.child_link(left) {
            Some(link) => Some(get_node(&link)),
            None => None
        }
    }

    // pub fn maybe_rebalance(self, get_node: GetNodeFn) -> SparseTree {
    //     let balance_factor = self.node.balance_factor();
    //
    //     // check if we need to balance
    //     if (balance_factor.abs() <= 1) {
    //         return self;
    //     }
    //
    //      // check if we should do a double rotation
    //     let left = balance_factor < 0;
    //     let child = self.child(left, get_node);
    //     let double = if left {
    //         child.balance_factor() > 0
    //     } else {
    //         child.balance_factor() < 0
    //     };
    //
    //     if double {
    //         let new_child = child.rotate(store, !left);
    //         self.set_child(left, new_child);
    //     }
    //     self.rotate(store, left)
    // }

    fn child(&mut self, left: bool, get_node: GetNodeFn) -> Option<&mut SparseTree> {
        let child = if left { &mut self.left } else { &mut self.right };

        match child {
            Some(child) => {
                return Some(&mut *child);
            },
            None => {
                return None;
            }
        }
    }
}

impl fmt::Debug for SparseTree {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fn traverse(f: &mut fmt::Formatter, cursor: &SparseTree, depth: u8, left: bool) {
            write!(f, "{}", "  ".repeat(depth as usize));

            let prefix = if depth == 0 {
                ""
            } else if left {
                "L: "
            } else {
                "R: "
            };
            write!(f, "{}{:?}\n", prefix, cursor.node);

            match &cursor.left {
                Some(child) => { traverse(f, &child, depth + 1, true); },
                None => {}
            };
            match &cursor.right {
                (Some(child)) => { traverse(f, &child, depth + 1, false); },
                (None) => {}
            };
        };

        traverse(f, self, 0, false);
        write!(f, "\n")
    }
}

#[cfg(test)]
mod tests {
    use crate::node::*;

    #[test]
    fn it_works() {
        // let st = SparseTree{node: Node::new(b"a", b"b"), left: None, right: None};
        // println!("{:?}", st);
        //
        // let st = SparseTree::join(
        //     Node::new(b"aa", b"b"), Some(st), Some(SparseTree::new(Node::new(b"aa", b"b")))
        // );
        //
        // let st = SparseTree::join(
        //     Node::new(b"ab", b"b"), Some(st), Some(SparseTree::new(Node::new(b"abc", b"b")))
        // );
        // println!("{:?}", st);

        let st = SparseTree::new(Node::new(b"abc", b"x"));
        println!("{:?}", st);

        let st = st.put(
            |link| Node::new(link.key.as_slice(), b"x"),
            b"abcd", b"x"
        );
        println!("{:?}", st);

        let st = st.put(
            |link| Node::new(link.key.as_slice(), b"x"),
            b"a", b"x"
        );
        println!("{:?}", st);

        let st = st.put(
            |link| Node::new(link.key.as_slice(), b"x"),
            b"ab", b"x"
        );
        println!("{:?}", st);

        let st = st.put(
            |link| Node::new(link.key.as_slice(), b"x"),
            b"ab", b"y"
        );
        println!("{:?}", st);

        // let mut node = Node::new(b"foo", b"bar");
        // node.update_kv_hash();
        // println!("node: {:?}", node);
        // println!("encoded length: {:?}", node.encode().unwrap().len());
        //
        // let node2 = Node::decode(&node.encode().unwrap()[..]);
        // println!("node2: {:?}", node2);
        //
        // let mut node3 = Node::new(b"foo2", b"bar2");
        // node.set_child(true, &mut node3);
        //
        // println!("node: {:?}", node);
    }
}
