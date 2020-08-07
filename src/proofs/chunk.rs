use rocksdb::DBRawIterator;

use super::{Node, Op};
use crate::error::Result;
use crate::tree::{Fetch, RefWalker, Tree};

impl<'a, S> RefWalker<'a, S>
where
    S: Fetch + Sized + Send + Clone,
{
    fn create_trunk_proof(&mut self) -> Result<Vec<Op>> {
        let approx_size = 2u8.pow((self.tree().height() / 2) as u32);
        let mut proof = Vec::with_capacity(approx_size as usize);

        let trunk_height = self.traverse_for_height_proof(&mut proof, 1)?;
        self.traverse_for_trunk(&mut proof, 1, trunk_height, true)?;

        Ok(proof)
    }

    // traverse to leftmost node to prove height of tree
    fn traverse_for_height_proof(&mut self, proof: &mut Vec<Op>, depth: u8) -> Result<u8> {
        let maybe_left = self.walk(true)?;
        let has_left_child = maybe_left.is_some();

        let trunk_height = if let Some(mut left) = maybe_left {
            left.traverse_for_height_proof(proof, depth + 1)?
        } else {
            depth / 2
        };

        if depth > trunk_height {
            proof.push(Op::Push(self.to_kvhash_node()));

            if has_left_child {
                proof.push(Op::Parent);
            }

            if let Some(right) = self.walk(false)? {
                proof.push(Op::Push(right.to_hash_node()));
                proof.push(Op::Child);
            }
        }

        Ok(trunk_height)
    }

    // build proof for all nodes in chunk
    fn traverse_for_trunk(
        &mut self,
        proof: &mut Vec<Op>,
        depth: u8,
        trunk_height: u8,
        is_leftmost: bool,
    ) -> Result<()> {
        if depth == trunk_height {
            // return early if we have reached bottom of trunk

            // connect to hash of left child
            // for leftmost node, we already have height proof
            if !is_leftmost {
                if let Some(left_child) = self.tree().link(true) {
                    proof.push(Op::Push(Node::Hash(*left_child.hash())));
                }
            }

            // add this node's data
            proof.push(Op::Push(self.to_kv_node()));

            // add parent op to connect left child
            if let Some(_) = self.tree().link(true) {
                proof.push(Op::Parent);
            }

            // connect to hash of right child
            if let Some(right_child) = self.tree().link(false) {
                proof.push(Op::Push(Node::Hash(*right_child.hash())));
                proof.push(Op::Child);
            }

            return Ok(());
        }

        // traverse left, guaranteed to have child
        let mut left = self.walk(true)?.unwrap();
        left.traverse_for_trunk(proof, depth + 1, trunk_height, is_leftmost)?;

        // add this node's data
        proof.push(Op::Push(self.to_kv_node()));
        proof.push(Op::Parent);

        // traverse right, guaranteed to have child
        let mut right = self.walk(false)?.unwrap();
        right.traverse_for_trunk(proof, depth + 1, trunk_height, false)?;
        proof.push(Op::Child);

        Ok(())
    }
}

fn get_next_chunk(iter: &mut DBRawIterator, end_key: Option<&[u8]>) -> Result<Vec<Op>> {
    let mut chunk = Vec::with_capacity(512);
    let mut stack = Vec::with_capacity(32);
    let mut node = Tree::new(vec![], vec![]);

    while iter.valid() {
        let key = iter.key().unwrap();

        if let Some(end_key) = end_key {
            if key == end_key {
                break;
            }
        }

        let encoded_node = iter.value().unwrap();
        Tree::decode_into(&mut node, vec![], encoded_node);

        let kv = Node::KV(key.to_vec(), node.value().to_vec());
        chunk.push(Op::Push(kv));

        if node.link(true).is_some() {
            chunk.push(Op::Parent);
        }

        if let Some(child) = node.link(false) {
            stack.push(child.key().to_vec());
        } else {
            while let Some(top_key) = stack.last() {
                if top_key.as_slice() > key {
                    break;
                }
                stack.pop();
                chunk.push(Op::Child);
            }
        }

        iter.next();
    }

    Ok(chunk)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::*;
    use crate::tree::PanicSource;

    #[test]
    fn trunk() {
        let mut tree = make_tree_seq(31);
        let mut walker = RefWalker::new(&mut tree, PanicSource {});

        let proof = walker.create_trunk_proof().unwrap();
        println!("{:?}", proof);
    }

    #[test]
    fn leaf_chunk() {
        let mut merk = TempMerk::new().unwrap();
        let batch = make_batch_seq(1..11);
        merk.apply(batch.as_slice(), &[]).unwrap();

        let root_node = merk.tree.take();
        let root_key = root_node.as_ref().unwrap().key().to_vec();
        merk.tree.set(root_node);

        let mut iter = merk.db.raw_iterator();
        iter.seek_to_first();
        let chunk = get_next_chunk(&mut iter, Some(root_key.as_slice()));
        println!("{:?}", chunk);
    }
}
