mod worker;

use std::fmt;

use crate::tree::{Tree, Node};
use crate::error::Result;

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

pub struct Context<'a> {
    pub batch: &'a Batch<'a>,
    pub pool: Pool<'a>,
    // TODO: create a GetNode trait which Pool should implement
    pub get_node: Box<dyn Fn(&[u8]) -> Result<Node>>
}

impl<'a> Context<'a> {
    pub fn with_batch(self, batch: &'a Batch<'a>) -> Context {
        self.batch = batch;
        self
    }
}

fn apply_or_build(tree: Option<Tree>, ctx: Context) -> Result<Option<Tree>> {
    if let Some(tree) = tree {
        tree.apply(ctx)
    } else {
        Tree::build(ctx)
    }
}

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
    fn apply(self, ctx: Context) -> Result<Option<Tree>> {
        // binary search to see if this node's key is in the batch, and to split
        // into left and right batches
        let search = ctx.batch.binary_search_by(
            |(key, _op)| key.cmp(&&self.key[..])
        );
        if let Ok(index) = search {
            // a key matches this node's key, apply op to this node
            match ctx.batch[index].1 {
                Put(value) => {
                    self.node_mut().set_value(value);
                },
                Delete => {
                    // self.remove()?;
                    panic!("remove not yet implemented");
                }
            };
        }

        let (mid, exclusive) = match search {
            Ok(index) => (index, true),
            Err(index) => (index, false)
        };
        self.recurse(ctx, mid, exclusive)
    }

    fn recurse(
        self,
        ctx: Context,
        mid: usize,
        exclusive: bool
    ) -> Result<Option<Tree>> {
        let left_batch = &ctx.batch[..mid];
        let right_batch = match exclusive {
            true => &ctx.batch[mid + 1..],
            false => &ctx.batch[mid..]
        };

        let tree = match (left_batch.is_empty(), right_batch.is_empty()) {
            // batches are empty, don't recurse
            (true, true) => self,
            
            // only left batch is not empty
            (false, true) => {
                self.child(true, &ctx.get_node)?;
                let (tree, left) = self.detach(true);
                let left = apply_or_build(left, ctx.with_batch(left_batch))?;
                tree.attach(true, left)
            },

            // only right batch is not empty
            (true, false) => {
                self.child(false, &ctx.get_node)?;
                let (tree, right) = self.detach(false);
                let right = apply_or_build(right, ctx.with_batch(right_batch))?;
                tree.attach(false, right)
            },

            // both contexts are not empty
            (false, false) => {
                // split up workers based on ratio of batch sizes.
                // it is possible for one side to have 0 workers, which means
                // it will just run in the same thread.
                let ratio = left_batch.len() as f32 / right_batch.len() as f32;
                let left_pool_len = (ctx.pool.len() * ratio) as u32;
                let (left_pool, right_pool) = ctx.pool.split_at(left_pool_len);

                // start working on right side in parallel
                self.child(false, &ctx.get_node)?;
                let (tree, right_tree) = self.detach(false);
                let right_join = right_pool.apply(right_batch, right_tree);

                // work on left side in this thread
                self.child(true, &ctx.get_node)?;
                let (tree, left_tree) = tree.detach(true);
                let left = apply_or_build(left_tree, Context {
                    batch: left_batch,
                    pool: left_pool,
                    get_node: ctx.get_node
                })?;

                // join with right side
                let right = right_join();
                
                tree
                    .attach(true, left)
                    .attach(false, right);
            }
        };

        Ok(Some(tree.maybe_balance(&ctx.get_node)?))
    }

    fn maybe_balance(
        self,
        get_node: &Box<dyn Fn(&[u8]) -> Result<Node>>
    ) -> Result<Tree> {
        let balance_factor = self.balance_factor();
        if balance_factor.abs() <= 1 {
            return Ok(self);
        }

        let left = balance_factor < 0;
        let (parent, child) = self.walk_expect(left, get_node)?;

        // maybe do a double rotation
        let child = match left == (child.balance_factor() > 0) {
            true => child.rotate(!left, get_node)?,
            false => child
        };

        parent.rotate(left, get_node)
    }

    fn rotate(
        self,
        left: bool,
        get_node: &Box<dyn Fn(&[u8]) -> Result<Node>>
    ) -> Result<Tree> {
        let (tree, child) = self.walk_expect(left, get_node)?;
        let (child, maybe_grandchild) = child.walk(!left, get_node)?;

        // attach grandchild to self
        let tree = self
            .attach(left, maybe_grandchild)
            .maybe_balance(get_node)?;

        // attach self to child, return child
        child
            .attach(!left, Some(tree))
            .maybe_balance(get_node)
    }

    fn build(ctx: Context) -> Result<Option<Tree>> {
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
        tree.recurse(ctx, mid, true)
    }

    fn child(
        &mut self,
        left: bool,
        get_node: &Box<dyn Fn(&[u8]) -> Result<Node>>
    ) -> Result<Option<&Tree>> {
        let link = match self.child_link(left) {
            None => return Ok(None),
            Some(child) => child
        };

        let node = get_node(&link.key[..])?;
        let tree = Tree::new(node);
        let slot = self.child_slot_mut(left);
        *slot = Some(tree);
        Ok(slot.as_ref())
    }
}
