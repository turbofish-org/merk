// use std::collections::BTreeMap;
// use std::fmt;
// use std::ops::{Deref, DerefMut};

// use crate::sparse_tree::*;
// use crate::error::Result;
// use crate::sparse_tree::Op::{Put, Delete};
// use crate::node::Node;

// pub struct OpContext<'a> {
//     tree: &'a mut TreeContainer,
//     batch: &'a TreeBatch<'a>,
//     worker_pool: &'a [OpWorker],
//     worker: Option<OpWorker>,
//     get_node: &'a mut dyn GetNodeFn
// }

// impl<'a> OpContext<'a> {
//     /// Applies the batch of operations (puts and deletes) to the tree.
//     ///
//     /// The tree structure and relevant Merkle hashes are updated in memory.
//     ///
//     /// This method will fetch relevant missing nodes (if any) from the backing
//     /// database.
//     ///
//     /// **NOTE:** The keys in the batch *MUST* be sorted and unique. This
//     /// condition is checked in debug builds, but for performance reasons it is
//     /// unchecked in release builds - unsorted or duplicate keys will result in
//     /// undefined behavior.
//     pub fn apply(self) -> Result<Vec<Vec<&'a Node>>> {
//         if self.batch.is_empty() {
//             return Ok(vec![]);
//         }

//         // ensure keys in batch are sorted and unique. this check is expensive,
//         // so we only do it in debug builds. in release builds, non-sorted or
//         // duplicate keys results in UB!
//         for pair in self.batch.windows(2) {
//             debug_assert!(
//                 pair[0].0 < pair[1].0,
//                 "keys must be sorted and unique"
//             );
//         }

//         let tree = match self.tree {
//             // if no tree, build one and point the parent reference to it
//             None => {
//                 // use middle batch item as root
//                 let mid = self.batch.len() / 2;
//                 let (mid_key, mid_op) = &self.batch[mid];
//                 let mid_value = match mid_op {
//                     Delete => bail!("Tried to delete non-existent key: {:?}", mid_key),
//                     Put(value) => value
//                 };
                
//                 *self.tree = Some(Box::new(
//                     Tree::new(
//                         Node::new(mid_key, mid_value)
//                     )
//                 ));

//                 // recursively build left and right subtrees
//                 return self.split_exclusive(mid);
//             },

//             // otherwise, do operations on this tree
//             Some(tree) => tree
//         };

//         // binary search to see if this node's key is in the batch, and to split
//         // into left and right batches
//         let search = self.batch.binary_search_by(
//             |(key, _op)| key.cmp(&&tree.key[..])
//         );
//         match search {
//             Ok(index) => {
//                 // a key matches this node's key, apply op to this node
//                 let mut writes = match self.batch[index].1 {
//                     Put(value) => {
//                         tree.set_value(value);
//                         vec![ vec![ tree.node() ] ]
//                     },
//                     Delete => {
//                         self.remove()?
//                     }
//                 };

//                 // recurse left and right, excluding node we just operated on
//                 writes.append(&mut self.split_exclusive(index)?);

//                 Ok(writes)
//             }
//             Err(index) => {
//                 // recurse left and right
//                 self.split(index)
//             }
//         }
//     }

//     // recursively apply ops to child
//     #[inline]
//     fn apply_child(
//         &mut self,
//         left: bool,
//         get_node: &mut dyn GetNodeFn,
//         batch: &TreeBatch
//     ) -> Result<()> {
//         // return early if batch is empty
//         if batch.is_empty() {
//             return Ok(());
//         }

//         // try to get child, fetching from db if necessary
//         self.maybe_get_child(get_node, left)?;

//         // apply recursive batch to child, modifying child_container
//         let child_container = self.child_container_mut(left);
//         Tree::apply(child_container, get_node, batch)?;

//         // recompute hash/height of child
//         self.update_link(left);

//         Ok(())
//     }

//     fn split_at(
//         &'a mut self,
//         left_index: usize,
//         right_index: usize
//     ) -> Result<Vec<Vec<&'a Node>>> {
//         debug_assert!(left_index <= right_index);

//         let tree = self.tree.unwrap();

//         let left = OpContext {
//             tree: &mut tree.left,
//             batch: &self.batch[..left_index],
//             worker_pool: &[],
//             worker: None,
//             get_node: self.get_node
//         };

//         let right = OpContext {
//             tree: &mut tree.right,
//             batch: &self.batch[right_index..],
//             worker_pool: &[],
//             worker: None,
//             get_node: self.get_node
//         };

//         let mut writes = left.apply()?;
//         writes.append(&mut right.apply()?);

//         writes.append(&mut self.maybe_rebalance()?);

//         Ok(writes)
//     }

//     fn split(&'a mut self, index: usize) -> Result<Vec<Vec<&'a Node>>> {
//         self.split_at(index, index)
//     }

//     fn split_exclusive(&'a mut self, index: usize) -> Result<Vec<Vec<&'a Node>>> {
//         self.split_at(index, index + 1)
//     }

//     fn maybe_rebalance(&mut self) -> Result<Vec<Vec<&'a Node>>> {

//     }

//     fn remove(&mut self) -> Result<Vec<Vec<&'a Node>>> {

//     }
// }