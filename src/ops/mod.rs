use std::fmt;
use std::collections::BTreeSet;
use crate::error::Result;
use crate::tree::{Tree, Node, OwnedWalker, Fetch};
use Op::*;

pub enum Op {
    Put(Vec<u8>),
    Delete
}

impl fmt::Debug for Op {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "{}", match self {
            Put(value) => format!("Put({:?})", value),
            Delete => "Delete".to_string()
        })
    }
}


pub type BatchEntry = (Vec<u8>, Op);

pub type Batch = [BatchEntry];

#[derive(Clone)]
struct PanicSource {}
impl Fetch for PanicSource {
    fn fetch(&self, key: &[u8]) -> Result<Node> {
        unreachable!("'fetch' should not have been called")
    }
}

impl<S> OwnedWalker<S>
    where S: Fetch + Sized + Send + Clone
{
    pub fn apply_to(
        maybe_tree: Option<Self>,
        batch: &Batch
    ) -> Result<Option<Tree>> {
        if batch.is_empty() {
            return Ok(maybe_tree.map(|w| w.unwrap()));
        }

        match maybe_tree {
            None => Self::build(batch),
            Some(tree) => Ok(tree.apply(batch)?.map(|w| w.unwrap()))
        }
    }

    fn build(batch: &Batch) -> Result<Option<Tree>> {
        if batch.is_empty() {
            return Ok(None);
        }

        let mid_index = batch.len() / 2;
        let (mid_key, mid_op) = &batch[mid_index];
        let mid_value = match mid_op {
            Delete => panic!("Tried to delete non-existent key {:?}", mid_key),
            Put(value) => value
        };

        let mid_tree = Tree::new(
            Node::new(&mid_key[..], &mid_value[..])
        );
        let mid_walker = OwnedWalker::<PanicSource>::new(mid_tree, PanicSource {});
        Ok(mid_walker.recurse(batch, mid_index, true)?.map(|w| w.unwrap()))
    }

    fn apply(self, batch: &Batch) -> Result<Option<Self>> {
        // binary search to see if this node's key is in the batch, and to split
        // into left and right batches
        let search = batch.binary_search_by(
            |(key, _op)| key.cmp(&self.tree().node().key)
        );
        let tree = if let Ok(index) = search {
            // a key matches this node's key, apply op to this node
            match &batch[index].1 {
                Put(value) => {
                    // TODO: explode instead of cloning, or use a replacement method
                    //       - should be able to cleanly add op to write batch
                    let source = self.clone_source();
                    OwnedWalker::new(
                        self.unwrap().with_value(value),
                        source
                    )
                },
                Delete => {
                    // self.remove()?;
                    panic!("remove not yet implemented")
                }
            }
        } else {
            self
        };

        let (mid, exclusive) = match search {
            Ok(index) => (index, true),
            Err(index) => (index, false)
        };

        tree.recurse(batch, mid, exclusive)
    }

    fn recurse(
        mut self,
        batch: &Batch,
        mid: usize,
        exclusive: bool
    ) -> Result<Option<Self>> {
        let left_batch = &batch[..mid];
        let right_batch = match exclusive {
            true => &batch[mid + 1..],
            false => &batch[mid..]
        };
 
        let maybe_left = Self::apply_to(self.walk(true)?, left_batch)?;
        let maybe_right = Self::apply_to(self.walk(false)?, right_batch)?;
        let source = self.clone_source();
        // TODO: explode instead of cloning source and unwrapping,
        //       or use a tree replacement method
        let tree = self.unwrap()
            // TODO: attach_left, attach_right
            .attach(true, maybe_left)
            .attach(false, maybe_right);
        let walker = Self::new(tree, source);
        Ok(Some(walker))

        // let tree = match (left_batch.is_empty(), right_batch.is_empty()) {
        //     // batches are empty, don't recurse
        //     (true, true) => self,
            
        //     // one batch is empty
        //     (left_ne, right_ne) if left_ne != right_ne => {
        //         let maybe_child = self.walk_mut(right_ne)?;
        //         let child = apply_or_build(maybe_child, self.with_batch(left_batch))?;
        //         self.attach(right_ne, child)
        //     },

        //     // neither batch is empty
        //     (false, false) => {
        //         // split up workers based on ratio of batch sizes.
        //         // it is possible for one side to have 0 workers, which means
        //         // it will just run in the same thread.
        //         let ratio = left_batch.len() as f32 / right_batch.len() as f32;
        //         let left_pool_len = (self.pool.len() * ratio) as u32;
        //         let (left_pool, right_pool) = self.pool.split_at(left_pool_len);

        //         // start working on right side in parallel
        //         let maybe_right = self.walk_mut(false)?;
        //         let right_join = right_pool.apply(right_batch, maybe_right);

        //         // work on left side in this thread
        //         let maybe_left = self.walk_mut(true)?;
        //         let left = apply_or_build(maybe_left, Context {
        //             batch: left_batch,
        //             pool: left_pool,
        //             get_node: self.get_node
        //         })?;

        //         // join with right side
        //         let right = right_join();
                
        //         self.attach(true, left)
        //             .attach(false, right)
        //             .maybe_balance()?
        //     }
        // }.unwrap();

        // Ok(Some(tree))
    }

    #[inline]
    fn balance_factor(&self) -> i8 {
        self.tree().node().balance_factor()
    }

    fn maybe_balance(mut self) -> Result<Self> {
        let balance_factor = self.balance_factor();
        if balance_factor.abs() <= 1 {
            return Ok(self);
        }

        let left = balance_factor < 0;
        let child = self.walk_expect(left)?;

        // maybe do a double rotation
        let child = match left == (child.balance_factor() > 0) {
            true => child.rotate(!left)?,
            false => child
        };

        self.rotate(left)
    }

    fn rotate(mut self, left: bool) -> Result<Self> {
        let mut child = self.walk_expect(left)?;
        let maybe_grandchild = child.walk(!left)?;

        // attach grandchild to self
        let tree = self.attach(left, maybe_grandchild)
            .maybe_balance()?;

        // attach self to child, return child
        child.attach(!left, Some(tree))
            .maybe_balance()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::tree::*;

    #[test]
    fn simple_put() {
        let batch = [
            (
                b"foo2".to_vec(),
                Op::Put(b"bar2".to_vec())
            )
        ];
        let tree = Tree::new(Node::new(b"foo", b"bar"));
        let mut walker = OwnedWalker::new(tree, PanicSource {})
            .apply(&batch)
            .expect("apply errored")
            .expect("should be Some");
        assert_eq!(walker.tree().node().key, b"foo");
        assert_eq!(walker.walk_expect(false).unwrap().tree().node().key, b"foo2");
    }
}
