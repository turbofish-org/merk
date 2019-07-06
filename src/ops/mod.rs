mod worker;
mod context;

use std::fmt;

use crate::tree::{Tree, Node};
use crate::error::Result;

pub use context::Context;
pub use Op::*;
pub use worker::{Worker, Pool};

pub enum Op<'a> {
    Put(&'a [u8]),
    Delete
}

impl<'a> fmt::Debug for Op<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "{}", match self {
            Put(value) => format!("Put({:?})", value),
            Delete => "Delete".to_string()
        })
    }
}

pub type BatchEntry<'a> = (&'a [u8], Op<'a>);

pub type Batch<'a> = [BatchEntry<'a>];

pub fn apply<'a>(
    tree: Option<Tree>,
    batch: &Batch<'a>,
    pool: &Pool
) -> Result<Option<Tree>> {
    for pair in batch.windows(2) {
        debug_assert!(
            pair[0].0 < pair[1].0,
            "keys must be sorted and unique"
        );
    }

    // apply ops to tree, updating its structure and data
    let mut tree = pool.apply(tree, batch)?;

    // write updates to db, and possibly prune nodes from memory
    // (needs mut to mark updated nodes as written)
    // TODO: we don't necessarily want to write every call to `apply`
    pool.write(&mut tree)?;

    Ok(tree)
}

impl Tree {
    fn apply(self, ctx: Context) -> Result<Tree> {

    }

    fn recurse(
        self,
        ctx: Context,
        mid: usize,
        exclusive: bool
    ) -> Result<Tree> {
        let left_batch = &ctx.batch[..mid];
        let right_batch = match exclusive {
            true => &ctx.batch[mid + 1..],
            false => &ctx.batch[mid..]
        };

        let ratio = left_batch.len() as f32 / right_batch.len() as f32;
        let left_workers_len = (ctx.workers.len() * ratio) as u32;
        let (left_workers, right_workers) =
            ctx.workers.split_at(left_workers_len);

        match (left_batch.is_empty(), right_batch.is_empty()) {
            // batches are empty, don't recurse
            (true, true) => Ok(self),
            
            // only left batch is not empty
            (false, true) => {
                let (tree, left_tree) = self.detach(true);
                let left = left_ctx.apply(left)?;
                Ok(tree.attach(true, left))
            },

            // only right batch is not empty
            (true, false) => {
                let (tree, right_tree) = self.detach(false);
                let right = right_ctx.apply(right)?;
                Ok(tree.attach(false, right))
            },

            // both contexts are not empty
            (false, false) => {
                // work on right side, possibly in a different thread
                let (tree, right_tree) = self.detach(false);
                let right_join = right_ctx.maybe_fork(right)?;

                // work on left side in this thread
                let (tree, left) = tree.detach(true);
                let left = left_ctx.apply(left)?;

                // join with right side
                let right = right_join();
                
                let tree = tree
                    .attach(true, left)
                    .attach(false, right_join());
                Ok(tree)
            }
        }
    }

    fn build(ctx: Context) -> Result<Tree> {
        // got to an empty tree, all ops should be inserts.

        // use middle batch item as root
        let mid = ctx.batch.len() / 2;
        let (mid_key, mid_op) = &ctx.batch[mid];
        let mid_value = match mid_op {
            Op::Delete => bail!("Tried to delete non-existent key: {:?}", mid_key),
            Op::Put(value) => value
        };
        let tree = Tree::new(
            Node::new(mid_key, mid_value)
        );

        // recursively build left and right subtrees
        tree.recurse_exclusive(ctx, mid)
    }
}
