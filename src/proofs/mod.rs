mod encoding;

use std::collections::LinkedList;
use crate::error::Result;
use crate::tree::{Tree, Link, RefWalker, Hash, Fetch};
pub use encoding::{encode_into, encoding_length};

#[derive(Debug, PartialEq)]
pub enum Op {
    Push(Node),
    Parent,
    Child
}

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Hash(Hash),
    KVHash(Hash),
    KV(Vec<u8>, Vec<u8>)
}

impl Link {
    fn to_hash_node(&self) -> Node {
        let hash = match self {
            Link::Modified { .. } => {
                panic!("Cannot convert Link::Modified to proof hash node");
            },
            Link::Pruned { hash, .. } => hash,
            Link::Stored { hash, .. } => hash
        };
        Node::Hash(*hash)
    }
}

impl<'a, S> RefWalker<'a, S>
    where S: Fetch + Sized + Send + Clone
{
    fn to_kv_node(&self) -> Node {
        Node::KV(
            self.tree().key().to_vec(),
            self.tree().value().to_vec()
        )
    }

    fn to_kvhash_node(&self) -> Node {
        Node::KVHash(*self.tree().kv_hash())
    }

    pub fn create_proof(
        &mut self,
        keys: &[Vec<u8>],
    ) -> Result<(
        LinkedList<Op>,
        (bool, bool)
    )> {
        let search = keys.binary_search_by(
            |key| key.as_slice().cmp(self.tree().key())
        );

        let (left_keys, right_keys) = match search {
            Ok(index) => (&keys[..index], &keys[index+1..]),
            Err(index) => (&keys[..index], &keys[index..])
        };

        let (mut proof, left_absence) =
            self.create_child_proof(true, left_keys)?;
        let (mut right_proof, right_absence) =
            self.create_child_proof(false, right_keys)?;

        let (has_left, has_right) = (
            !proof.is_empty(),
            !right_proof.is_empty()
        );

        proof.push_back(match search {
            Ok(_) => Op::Push(self.to_kv_node()),
            Err(_) => {
                if left_absence.1 || right_absence.0 {
                    Op::Push(self.to_kv_node())
                } else {
                    Op::Push(self.to_kvhash_node())
                }
            }
        });

        if has_left {
            proof.push_back(Op::Parent);
        }

        if has_right {
            proof.append(&mut right_proof);
            proof.push_back(Op::Child);
        }

        Ok((
            proof,
            (left_absence.0, right_absence.1)
        ))
    }

    fn create_child_proof(
        &mut self,
        left: bool,
        keys: &[Vec<u8>]
    ) -> Result<(
        LinkedList<Op>,
        (bool, bool)
    )> {
        Ok(if !keys.is_empty() {
            if let Some(mut child) = self.walk(left)? {
                child.create_proof(keys)?
            } else {
                (LinkedList::new(), (true, true))
            }
        } else if let Some(link) = self.tree().link(left) {
            let mut proof = LinkedList::new();
            proof.push_back(Op::Push(link.to_hash_node()));
            (proof, (false, false))
        } else {
            (LinkedList::new(), (false, false))
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::tree::{PanicSource, RefWalker};

    fn make_3_node_tree() -> Tree {
        Tree::from_fields(
            vec![5], vec![5], [105; 20],
            Some(Link::Stored {
                child_heights: (0, 0),
                hash: [3; 20],
                tree: Tree::from_fields(
                    vec![3], vec![3], [103; 20],
                    None, None
                )
            }),
            Some(Link::Stored {
                child_heights: (0, 0),
                hash: [7; 20],
                tree: Tree::from_fields(
                    vec![7], vec![7], [107; 20],
                    None, None
                )
            })
        )
    }

    #[test]
    fn empty_proof() {
        let mut tree = make_3_node_tree();
        let mut walker = RefWalker::new(&mut tree, PanicSource {});

        let (proof, absence) = walker
            .create_proof(vec![].as_slice())
            .expect("create_proof errored");

        let mut iter = proof.iter();
        assert_eq!(iter.next(), Some(&Op::Push(Node::Hash([3; 20]))));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KVHash([105; 20]))));
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(iter.next(), Some(&Op::Push(Node::Hash([7; 20]))));
        assert_eq!(iter.next(), Some(&Op::Child));
        assert!(iter.next().is_none());
        assert_eq!(absence, (false, false));
    }

    #[test]
    fn root_proof() {
        let mut tree = make_3_node_tree();
        let mut walker = RefWalker::new(&mut tree, PanicSource {});

        let (proof, absence) = walker
            .create_proof(vec![vec![5]].as_slice())
            .expect("create_proof errored");

        let mut iter = proof.iter();
        assert_eq!(iter.next(), Some(&Op::Push(Node::Hash([3; 20]))));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![5], vec![5]))));
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(iter.next(), Some(&Op::Push(Node::Hash([7; 20]))));
        assert_eq!(iter.next(), Some(&Op::Child));
        assert!(iter.next().is_none());
        assert_eq!(absence, (false, false));
    }

    #[test]
    fn leaf_proof() {
        let mut tree = make_3_node_tree();
        let mut walker = RefWalker::new(&mut tree, PanicSource {});

        let (proof, absence) = walker
            .create_proof(vec![vec![3]].as_slice())
            .expect("create_proof errored");

        let mut iter = proof.iter();
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![3], vec![3]))));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KVHash([105; 20]))));
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(iter.next(), Some(&Op::Push(Node::Hash([7; 20]))));
        assert_eq!(iter.next(), Some(&Op::Child));
        assert!(iter.next().is_none());
        assert_eq!(absence, (false, false));
    }

    #[test]
    fn double_leaf_proof() {
        let mut tree = make_3_node_tree();
        let mut walker = RefWalker::new(&mut tree, PanicSource {});

        let (proof, absence) = walker
            .create_proof(vec![vec![3], vec![7]].as_slice())
            .expect("create_proof errored");

        let mut iter = proof.iter();
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![3], vec![3]))));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KVHash([105; 20]))));
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![7], vec![7]))));
        assert_eq!(iter.next(), Some(&Op::Child));
        assert!(iter.next().is_none());
        assert_eq!(absence, (false, false));
    }

    #[test]
    fn all_nodes_proof() {
        let mut tree = make_3_node_tree();
        let mut walker = RefWalker::new(&mut tree, PanicSource {});

        let (proof, absence) = walker
            .create_proof(vec![vec![3], vec![5], vec![7]].as_slice())
            .expect("create_proof errored");

        let mut iter = proof.iter();
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![3], vec![3]))));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![5], vec![5]))));
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![7], vec![7]))));
        assert_eq!(iter.next(), Some(&Op::Child));
        assert!(iter.next().is_none());
        assert_eq!(absence, (false, false));
    }

    #[test]
    fn global_edge_absence_proof() {
        let mut tree = make_3_node_tree();
        let mut walker = RefWalker::new(&mut tree, PanicSource {});

        let (proof, absence) = walker
            .create_proof(vec![vec![8]].as_slice())
            .expect("create_proof errored");

        let mut iter = proof.iter();
        assert_eq!(iter.next(), Some(&Op::Push(Node::Hash([3; 20]))));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KVHash([105; 20]))));
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![7], vec![7]))));
        assert_eq!(iter.next(), Some(&Op::Child));
        assert!(iter.next().is_none());
        assert_eq!(absence, (false, true));
    }

    #[test]
    fn absence_proof() {
        let mut tree = make_3_node_tree();
        let mut walker = RefWalker::new(&mut tree, PanicSource {});

        let (proof, absence) = walker
            .create_proof(vec![vec![6]].as_slice())
            .expect("create_proof errored");

        let mut iter = proof.iter();
        assert_eq!(iter.next(), Some(&Op::Push(Node::Hash([3; 20]))));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![5], vec![5]))));
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![7], vec![7]))));
        assert_eq!(iter.next(), Some(&Op::Child));
        assert!(iter.next().is_none());
        assert_eq!(absence, (false, false));
    }

    #[test]
    fn doc_proof() {
        let mut tree = Tree::from_fields(
            vec![5], vec![5], [105; 20],
            Some(Link::Stored {
                child_heights: (0, 0),
                hash: [2; 20],
                tree: Tree::from_fields(
                    vec![2], vec![2], [102; 20],
                    Some(Link::Stored {
                        child_heights: (0, 0),
                        hash: [1; 20],
                        tree: Tree::from_fields(
                            vec![1], vec![1], [101; 20],
                            None, None
                        )
                    }),
                    Some(Link::Stored {
                        child_heights: (0, 0),
                        hash: [4; 20],
                        tree: Tree::from_fields(
                            vec![4], vec![4], [104; 20],
                            Some(Link::Stored {
                                child_heights: (0, 0),
                                hash: [3; 20],
                                tree: Tree::from_fields(
                                    vec![3], vec![3], [103; 20],
                                    None, None
                                )
                            }),
                            None
                        )
                    })
                )
            }),
            Some(Link::Stored {
                child_heights: (0, 0),
                hash: [9; 20],
                tree: Tree::from_fields(
                    vec![9], vec![9], [109; 20],
                    Some(Link::Stored {
                        child_heights: (0, 0),
                        hash: [7; 20],
                        tree: Tree::from_fields(
                            vec![7], vec![7], [107; 20],
                            Some(Link::Stored {
                                child_heights: (0, 0),
                                hash: [6; 20],
                                tree: Tree::from_fields(
                                    vec![6], vec![6], [106; 20],
                                    None, None
                                )
                            }),
                            Some(Link::Stored {
                                child_heights: (0, 0),
                                hash: [8; 20],
                                tree: Tree::from_fields(
                                    vec![8], vec![8], [108; 20],
                                    None, None
                                )
                            })
                        )
                    }),
                    Some(Link::Stored {
                        child_heights: (0, 0),
                        hash: [11; 20],
                        tree: Tree::from_fields(
                            vec![11], vec![11], [111; 20],
                            Some(Link::Stored {
                                child_heights: (0, 0),
                                hash: [10; 20],
                                tree: Tree::from_fields(
                                    vec![10], vec![10], [110; 20],
                                    None, None
                                )
                            }),
                            None
                        )
                    })
                )
            })
        );
        let mut walker = RefWalker::new(&mut tree, PanicSource {});

        let (proof, absence) = walker.create_proof(vec![
            vec![1],
            vec![2],
            vec![3],
            vec![4]
        ].as_slice()).expect("create_proof errored");

        let mut iter = proof.iter();
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![1], vec![1]))));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![2], vec![2]))));
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![3], vec![3]))));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![4], vec![4]))));
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(iter.next(), Some(&Op::Child));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KVHash([105; 20]))));
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(iter.next(), Some(&Op::Push(Node::Hash([9; 20]))));
        assert_eq!(iter.next(), Some(&Op::Child));
        assert!(iter.next().is_none());
        assert_eq!(absence, (false, false));

        let mut bytes = vec![];
        encode_into(proof.iter(), &mut bytes);
        assert_eq!(bytes, vec![3, 1, 1, 1, 0, 1, 3, 1, 2, 1, 0, 2, 16, 3, 1, 3, 1, 0, 3, 3, 1, 4, 1, 0, 4, 16, 17, 2, 105, 105, 105, 105, 105, 105, 105, 105, 105, 105, 105, 105, 105, 105, 105, 105, 105, 105, 105, 105, 16, 1, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 17]);
    }
}

