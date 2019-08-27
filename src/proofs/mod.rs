use std::collections::LinkedList;
use crate::error::Result;
use crate::tree::{Tree, Link, RefWalker, Hash, Fetch};

#[derive(Debug)]
pub enum Op {
    Push(Node),
    Parent,
    Child
}

#[derive(Debug)]
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
        Node::Hash(hash.clone())
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
        Node::KVHash(self.tree().kv_hash().clone())
    }

    pub(crate) fn create_proof(
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
            Ok(index) => Op::Push(self.to_kv_node()),
            Err(index) => {
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
    use crate::test_utils::{make_tree_seq, seq_key};
    use crate::tree::{PanicSource, RefWalker};

    #[test]
    fn simple_proof() {
        let mut tree = make_tree_seq(20);
        let mut walker = RefWalker::new(&mut tree, PanicSource {});

        let (proof, absence) = walker.create_proof(vec![
            seq_key(4),
            seq_key(5),
            seq_key(10),
            seq_key(100)
        ].as_slice()).expect("create_proof errored");

        println!("{:#?}", proof);
    }
}

