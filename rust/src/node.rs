extern crate byteorder;
extern crate blake2_rfc;
extern crate hex;
extern crate serde;

use std::fmt;
use std::cmp::max;
use std::ops::{Deref, DerefMut};

use blake2_rfc::blake2b::Blake2b;
use byteorder::{ByteOrder, BigEndian};
use serde::{Serialize, Deserialize};

const HASH_LENGTH: usize = 20;
type Hash = [u8; HASH_LENGTH];
const NULL_HASH: Hash = [0 as u8; HASH_LENGTH];

type GetNodeFn = fn(link: &Link) -> Node;

#[derive(Serialize, Deserialize, Clone)]
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
    get_node: GetNodeFn,
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

    pub fn child_link(&self, left: bool) -> Option<Link> {
        if left { self.left.clone() } else { self.right.clone() }
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

    pub fn set_child(&mut self, left: bool, child_link: Link) {
        let link = Some(child_link);
        if left {
            self.left = link;
        } else {
            self.right = link;
        }
    }

    pub fn set_parent(&mut self, parent_key: Vec<u8>) {
        self.parent_key = Some(self.key.to_vec());
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
    pub fn new(node: Node, get_node: GetNodeFn) -> SparseTree {
        SparseTree{
            node,
            get_node,
            left: None,
            right: None
        }
    }

    pub fn get(link: &Link, get_node: GetNodeFn) -> SparseTree {
        SparseTree::new(get_node(link), get_node)
    }

    pub fn put(&mut self, key: &[u8], value: &[u8]) {
        if self.node.key == key {
            // same key, just update the value of this node
            self.node.set_value(value);
            return;
        }

        let left = key < &self.node.key;
        let child_tree = self.maybe_get_child(left);
        let child_tree = match child_tree {
            Some(child_tree) => {
                // println!("-{:?}", child_node);
                // recursively put value under child
                child_tree.put(key, value);
            },
            None => {
                // no child here, create node and set as child
                let child_tree = Box::new(
                    SparseTree::new(
                        Node::new(key, value),
                        self.get_node
                    )
                );
                if left {
                    self.left = Some(child_tree);
                } else {
                    self.right = Some(child_tree);
                };
            }
        };

        // update link
        self.update_child(left);

        // TODO: rebalance if necessary
        // tree.maybe_rebalance(get_node)
    }

    pub fn update_child(&mut self, left: bool) {
        let child_link = if left {
            self.left.as_ref().unwrap().to_link()
        } else {
            self.right.as_ref().unwrap().to_link()
        };
        self.set_child(left, child_link);

        let child_tree = if left {
            &mut self.left
        } else {
            &mut self.right
        };
        child_tree.as_mut().unwrap().set_parent(self.node.key.to_vec());
    }

    pub fn child_tree_mut(&mut self, left: bool) -> &mut Option<Box<SparseTree>> {
        if left {
            &mut self.left
        } else {
            &mut self.right
        }
    }

    pub fn maybe_get_child(&mut self, left: bool) -> Option<&mut Box<SparseTree>> {
        if let Some(link) = self.child_link(left) {
            let get_node = self.get_node;
            let child_field = self.child_tree_mut(left);
            let child_tree = child_field.get_or_insert_with(|| {
                Box::new(SparseTree::get(&link, get_node))
            });
            Some(child_tree)
        } else {
            None
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

impl Deref for SparseTree {
    type Target = Node;

    fn deref(&self) -> &Node {
        &self.node
    }
}

impl DerefMut for SparseTree {
    fn deref_mut(&mut self) -> &mut Node {
        &mut self.node
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

        let mut st = SparseTree::new(
            Node::new(b"abc", b"x"),
            |link| Node::new(link.key.as_slice(), b"x")
        );
        println!("{:?}", st);

        st.put(
            b"abcd", b"x"
        );
        println!("{:?}", st);

        st.put(
            b"a", b"x"
        );
        println!("{:?}", st);

        st.put(
            b"ab", b"x"
        );
        println!("{:?}", st);

        st.put(
            b"ab", b"y"
        );
        println!("{:?}", st);

        st.put(
            b"6", b"x"
        );
        println!("{:?}", st);

        st.put(
            b"b", b"x"
        );
        println!("{:?}", st);

        st.put(
            b"bc", b"x"
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
