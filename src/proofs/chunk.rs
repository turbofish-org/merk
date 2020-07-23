use crate::error::Result;
use crate::tree::Hash;
use super::{Op, Node, encoding::encode_into};
use crate::tree::{Link, RefWalker, Fetch};

impl<'a, S> RefWalker<'a, S>
    where S: Fetch + Sized + Send + Clone
{
    fn create_trunk_proof(&mut self) -> Result<Vec<Op>> {
        let approx_size = 2u8.pow((self.tree().height() / 2) as u32);
        let mut proof = Vec::with_capacity(approx_size as usize);

        let trunk_height = self.traverse_for_height_proof(&mut proof, 1)?;
        self.traverse_for_trunk(&mut proof, 1, trunk_height, true)?;

        Ok(proof)
    }

    // traverse to leftmost node to prove height of tree
    fn traverse_for_height_proof(
        &mut self,
        proof: &mut Vec<Op>,
        depth: u8
    ) -> Result<u8> {
        let mut maybe_child = self.walk(true)?;
        let has_left_child = maybe_child.is_some();

        let trunk_height = if let Some(mut child) = maybe_child {
            child.traverse_for_height_proof(proof, depth + 1)?
        } else {
            depth / 2
        };

        if depth > trunk_height {
            // only inlcude hash, this node just proves height
            proof.push(Op::Push(self.to_hash_node()));

            if has_left_child {
                proof.push(Op::Parent);
            }
        }

        Ok(trunk_height)
    }

    // build proof for all chunk nodes
    fn traverse_for_trunk(
        &mut self,
        proof: &mut Vec<Op>,
        depth: u8,
        trunk_height: u8,
        is_leftmost: bool
    ) -> Result<()> {
        if depth == trunk_height {
            // return early if we have reached bottom of trunk
            proof.push(Op::Push(self.to_kv_node()));

            // connect to height proof, if there is one and this node is
            // the leftmost node of the trunk
            let has_left_child = self.tree().link(true).is_some();
            if is_leftmost && has_left_child {
                proof.push(Op::Parent);
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

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::tree::PanicSource;
    use crate::test_utils::*;

    #[test]
    fn trunk() {
        let mut tree = make_tree_seq(31);
        println!("{:?}", tree);
        let mut walker = RefWalker::new(&mut tree, PanicSource {});

        let proof = walker.create_trunk_proof().unwrap();
        println!("{:#?}", proof);
    }    
}
