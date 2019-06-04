use std::fmt;

use crate::*;

const MAX_STACK_SIZE: usize = 50;

pub enum Op {
  Push(Node),
  Parent,
  Child
}

impl fmt::Debug for Op {
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

pub enum Node {
  Data { key: Vec<u8>, value: Vec<u8> },
  Hash { left: bool, child_hash: Hash, kv_hash: Hash }
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Node::Data {key, value} => {
                write!(
                    f,
                    "({}: {})",
                    String::from_utf8(key.to_vec())
                        .unwrap_or_else(|_| format!("{:?}", key)),
                    String::from_utf8(value.to_vec())
                        .unwrap_or_else(|_| format!("{:?}", value))
                )
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

pub fn create(
    store: &Merk,
    start: &[u8],
    end: &[u8]
) -> Result<Vec<Op>> {
    // TODO: get bounds
    // TODO: remove prev_key, parent_stack (only for invariant assertion)
    // TODO: can we do this without so many clones?

    let mut proof = vec![];
    let mut prev_key: Option<Vec<u8>> = None;
    let mut child_stack: Vec<Vec<u8>> = vec![];
    let mut parent_stack = vec![];
    store.map_range(start, end, &mut |node| {
        let op = Op::Push(Node::Data {
            key: node.key.clone(),
            value: node.value.clone()
        });
        proof.push(op);

        let mut key = node.key.clone();
        while let Some(pop_key) = child_stack.last() {
            if &key == pop_key {
                proof.push(Op::Child);
                child_stack.pop();
                key = parent_stack.pop().unwrap();
            } else {
                break;
            }
        }

        if let Some(prev_key) = &prev_key {
            // TODO: only emit Parent op if child is in range
            if let Some(left_child) = &node.left {
                assert_eq!(
                    &left_child.key, prev_key,
                    "Expected left child to be previous node"
                );
                proof.push(Op::Parent);
            }
        }

        if let Some(right_child) = &node.right {
            child_stack.push(right_child.key.clone());
            parent_stack.push(node.key.clone());
        }

        prev_key = Some(key);
    })?;

    Ok(proof)
}

pub fn reconstruct(proof: &[Op]) -> Result<SparseTree> {
    let mut stack: Vec<SparseTree> = vec![];
    let mut prev_key = None;
    for op in proof {
        match op {
            Op::Push(node) => match node {
                Node::Data { key, value } => {
                    if stack.len() >= MAX_STACK_SIZE {
                        bail!("Stack exceeded maximum size");
                    }
                    if let Some(prev_key) = &prev_key {
                        assert!(
                            key > prev_key,
                            "Invalid key ordering"
                        );
                    }
                    prev_key = Some(key.clone());

                    let tree = SparseTree::new(
                        crate::Node::new(&key, &value)
                    );
                    stack.push(tree);
                },
                Node::Hash { left, child_hash, kv_hash } => {
                    panic!("hash nodes not yet handled");
                }
            },
            Op::Parent => {
                let mut top = stack.pop()
                    .expect("Expected node on stack");
                if top.left.is_some() {
                    bail!("Got PARENT op for node that already has left child");
                }

                let bottom = stack.pop()
                    .expect("Expected node on stack");

                // TODO: make SparseTree API cleaner
                top.left = Some(Box::new(bottom));
                top.update_link(true);

                stack.push(top);
            },
            Op::Child => {
                let bottom = stack.pop()
                    .expect("Expected node on stack");
                let mut top = stack.pop()
                    .expect("Expected node on stack");
                if top.right.is_some() {
                    bail!("Got CHILD op for node that already has right child");
                }

                top.right = Some(Box::new(bottom));
                top.update_link(false);

                stack.push(top);
            }
        }
    }

    assert_eq!(
        stack.len(), 1,
        "Proof resulted in multiple disjoint trees"
    );
    Ok(stack.pop().unwrap())
}

#[test]
fn proof_debug() {
    let debug = format!("{:?}", &[
        Op::Child,
        Op::Parent,
        Op::Push(Node::Data{
            key: vec![1,2,3],
            value: vec![4,5,6]
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
    assert_eq!(debug, "[CHILD, PARENT, PUSH(\u{1}\u{2}\u{3}: \u{4}\u{5}\u{6}), PUSH(left=010101010101 kv=020202020202), PUSH(right=030303030303 kv=040404040404)]");
}

#[test]
fn proof_reconstruct() {
    let proof = [
        Op::Push(Node::Data { key: vec![0], value: vec![123] }),
        Op::Push(Node::Data { key: vec![1], value: vec![123] }),
        Op::Parent,
        Op::Push(Node::Data { key: vec![2], value: vec![123] }),
        Op::Child,
        Op::Push(Node::Data { key: vec![3], value: vec![123] }),
        Op::Parent,
        Op::Push(Node::Data { key: vec![4], value: vec![123] }),
        Op::Push(Node::Data { key: vec![5], value: vec![123] }),
        Op::Child,
        Op::Parent
    ];

    let tree = reconstruct(&proof).unwrap();
    println!("{:?}", tree);
}