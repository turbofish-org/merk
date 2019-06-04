use std::fmt;

use crate::*;

pub enum Op<'a> {
  Push(Node<'a>),
  Parent,
  Child
}

impl<'a> fmt::Debug for Op<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Op::Push(node) => {
                write!(f, "PUSH{:?}", node)
            },
            Op::Parent => write!(f, "PARENT"),
            Op::Child => write!(f, "CHILD")
        }
    }
}

pub enum Node<'a> {
  Data { key: &'a [u8], value: &'a [u8] },
  Hash { left: bool, child_hash: Hash, kv_hash: Hash }
}

impl<'a> fmt::Debug for Node<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Node::Data {key, value} => {
                write!(f, "({:?}: {:?})", key, value)
            },
            Node::Hash {left, child_hash, kv_hash} => {
                write!(
                    f,
                    "({}={} kv={})",
                    if *left { "left" } else { "right" },
                    hex::encode(&child_hash[..6]),
                    hex::encode(&kv_hash[..6])
                )
            }
        }
    }
}

#[test]
fn proof_debug() {
    let debug = format!("{:?}", &[
        Op::Child,
        Op::Parent,
        Op::Push(Node::Data{
            key: &[1,2,3],
            value: &[4,5,6]
        }),
        Op::Push(Node::Hash{
            left: true,
            child_hash: [1; HASH_LENGTH],
            kv_hash: [2; HASH_LENGTH]
        }),
        Op::Push(Node::Hash{
            left: false,
            child_hash: [3; HASH_LENGTH],
            kv_hash: [4; HASH_LENGTH]
        })
    ]);
    assert_eq!(debug, "[CHILD, PARENT, PUSH([1, 2, 3]: [4, 5, 6]), PUSH(left=010101010101 kv=020202020202), PUSH(right=030303030303 kv=040404040404)]");
}


