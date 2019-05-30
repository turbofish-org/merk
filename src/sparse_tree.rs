use std::fmt;
use std::ops::{Deref, DerefMut};

use crate::error::Result;
use crate::node::{Link, Node};

use TreeOp::{Delete, Put};

pub trait GetNodeFn = FnMut(&Link) -> Result<Node>;

/// A selection of connected nodes in a tree.
///
/// SparseTrees are acyclic, and always have at least one node.
///
/// Operations fetch [`Node`s] from the backing database lazily, and retain them
/// in memory. Mutation operations only operate on the in-memory structure, but
/// a consumer can flush the updated structure to a backing database.
///
/// [`Node`s]: struct.Node.html
pub struct SparseTree {
    node: Node,
    left: Option<Box<SparseTree>>,
    right: Option<Box<SparseTree>>,
}

pub enum TreeOp<'a> {
    Put(&'a [u8]),
    Delete
}

pub type TreeBatchEntry<'a> = (&'a [u8], TreeOp<'a>);
pub type TreeBatch<'a> = [TreeBatchEntry<'a>];

///
impl SparseTree {
    /// Returns a new SparseTree which has the gien `Node` as its root, and no
    /// children.
    pub fn new(node: Node) -> SparseTree {
        SparseTree {
            node,
            left: None,
            right: None,
        }
    }

    pub fn from_batch(batch: &TreeBatch) -> Result<Option<Box<SparseTree>>> {
        if batch.is_empty() {
            return Ok(None);
        }

        let mid = batch.len() / 2;
        let (mid_key, mid_op) = &batch[mid];

        let mid_value = match mid_op {
            Delete => bail!("Tried to delete non-existent key"),
            Put(value) => value
        };

        // use middle value as root of new tree
        let mut tree = Some(Box::new(
            SparseTree::new(
                Node::new(mid_key, mid_value)
            )
        ));

        // add the rest of the batch to the new tree, split into 2
        // batches
        let left_batch = &batch[..mid];
        if !left_batch.is_empty() {
            SparseTree::apply(
                &mut tree,
                // this is a fresh tree so we never need to fetch nodes
                &mut |_| unreachable!("should not fetch"),
                left_batch
            )?;
        }

        let right_batch = &batch[mid+1..];
        if !right_batch.is_empty() {
            SparseTree::apply(
                &mut tree,
                // this is a fresh tree so we never need to fetch nodes
                &mut |_| unreachable!("should not fetch"),
                right_batch
            )?;
        }

        Ok(tree)
    }

    pub fn to_write_batch(&self) -> rocksdb::WriteBatch {
        fn traverse(tree: &SparseTree, batch: &mut rocksdb::WriteBatch) {
            if let Some(child) = tree.child_tree(true) {
                traverse(child, batch);
            }

            // TODO: Result
            let bytes = tree.node.encode().unwrap();
            batch.put(&tree.node.key, bytes).unwrap();

            if let Some(child) = tree.child_tree(false) {
                traverse(child, batch);
            }
        }

        let mut batch = rocksdb::WriteBatch::default();
        traverse(self, &mut batch);
        batch
    }

    /// Applies the batch of operations (puts and deletes) to the tree.
    ///
    /// The tree structure and relevant Merkle hashes are updated in memory.
    ///
    /// This method will fetch relevant missing nodes (if any) from the backing
    /// database.
    pub fn apply(
        self_container: &mut Option<Box<SparseTree>>,
        get_node: &mut GetNodeFn,
        batch: &TreeBatch
    ) -> Result<()> {
        // TODO: build and return db batch, and maybe prune as we ascend

        let tree = match self_container {
            // if no tree, build one and point the parent reference to it
            None => {
                *self_container = SparseTree::from_batch(batch)?;
                return Ok(());
            },

            // otherwise, do operations on this tree
            Some(tree) => tree
        };

        // binary search to see if this node's key is in the batch, and to split
        // into left and right batches
        let search = batch.binary_search_by(
            |(key, _op)| key.cmp(&& tree.node.key[..])
        );
        let (left_batch, right_batch) = match search {
            Ok(index) => {
                // a key matches this node's key, apply op to this node
                match batch[index].1 {
                    Put(value) => tree.set_value(value),
                    Delete => panic!("not implemented yet") // TODO: tree.delete()
                };

                // split batch into left and right batches for recursive ops,
                // exluding matched value
                (&batch[..index], &batch[index + 1..])
            }
            Err(index) => {
                // split batch into left and right batches for recursive puts
                batch.split_at(index)
            }
        };

        // apply ops recursively to children (if batches aren't empty)
        tree.apply_child(true, get_node, left_batch)?;
        tree.apply_child(false, get_node, right_batch)?;

        // rebalance if necessary
        SparseTree::maybe_rebalance(self_container, get_node)
    }

    // recursively apply ops to child
    #[inline]
    fn apply_child(
        &mut self,
        left: bool,
        get_node: &mut GetNodeFn,
        batch: &TreeBatch
    ) -> Result<()> {
        // return early if batch is empty
        if batch.is_empty() {
            return Ok(());
        }

        // try to get child, fetching from db if necessary
        self.maybe_get_child(get_node, left)?;

        // apply recursive batch to child, modifying child_container
        let child_container = self.child_container_mut(left);
        SparseTree::apply(child_container, get_node, batch)?;

        // recompute hash/height of child
        self.update_link(left);

        Ok(())
    }

    // fn delete(&mut self, get_node: &mut GetNodeFn) {
    //     // if this node is not a leaf node, traverse to edge of taller child,
    //     // then promote it to our position
    //     if !self.is_leaf() {
    //         let left = self.child_height(true) > self.child_height(false);
    //         let edge = self.edge()
    //     }
    // }

    // fn edge(
    //     &mut self,
    //     get_node: &mut GetNodeFn,
    //     left: bool
    // ) -> Result<&mut SparseTree> {
    //     match self.maybe_get_child(get_node, left)? {
    //         // if no child on that side, this node is the edge
    //         None => Ok(self),
    //         // otherwise, recurse
    //         Some(child) => child.edge(get_node, left)
    //     }
    // }

    pub fn prune(&mut self) {
        // TODO: keep upper levels of tree?
        self.left.take();
        self.right.take();
    }

    #[inline]
    pub fn node(&self) -> &Node {
        &self.node
    }

    fn update_link(&mut self, left: bool) {
        // compute child link and set on our node
        let link = self.child_tree(left)
            .map(|child| child.as_link());
        self.node.set_child(left, link);
    }

    #[inline]
    pub fn child_tree(&self, left: bool) -> Option<&SparseTree> {
        let option = if left { &self.left } else { &self.right };
        option.as_ref().map(|x| x.as_ref())
    }

    #[inline]
    fn child_container_mut(&mut self, left: bool) -> &mut Option<Box<SparseTree>> {
        if left {
            &mut self.left
        } else {
            &mut self.right
        }
    }

    fn maybe_get_child(
        &mut self,
        get_node: &mut GetNodeFn,
        left: bool,
    ) -> Result<Option<&mut SparseTree>> {
        if let Some(link) = self.child_link(left) {
            // node has a link, get from memory or fetch from db

            if self.child_tree(left).is_none() {
                // fetch child from db and put it in child field
                let node = get_node(&link)?;
                let child_container = self.child_container_mut(left);
                *child_container = Some(Box::new(SparseTree::new(node)));
            }

            let child_container = self.child_container_mut(left);
            Ok(child_container.as_mut().map(|x| x.as_mut()))
        } else {
            // node has no link, nothing to get
            Ok(None)
        }
    }

    fn maybe_rebalance(
        self_container: &mut Option<Box<SparseTree>>,
        get_node: &mut GetNodeFn
    ) -> Result<()> {
        // unwrap self_container or return early if empty
        let tree = match self_container {
            None => return Ok(()),
            Some(tree) => tree
        };

        // return early if we don't need to balance
        let balance_factor = tree.balance_factor();
        if balance_factor.abs() <= 1 {
            return Ok(());
        }

        // get child. (this child should always be Some: if the tree is
        // unbalanced in this direction then a child must exist)
        let left = balance_factor < 0;
        let child_balance_factor = match tree.maybe_get_child(get_node, left)? {
            None => unreachable!("child must exist"),
            Some(child) => child.balance_factor()
        };
        // get container for child
        let child_container = tree.child_container_mut(left);

        // maybe do a double rotation
        if left == (child_balance_factor > 0) {
            // rotate child opposite direction, then update link
            SparseTree::rotate(child_container, get_node, !left)?;
            SparseTree::maybe_rebalance(child_container, get_node)?;
            tree.update_link(!left);
        }

        // do the rotation
        SparseTree::rotate(self_container, get_node, left)?;
        let tree = self_container.as_mut()
            .expect("container must not be empty");

        // rebalance recursively if necessary
        tree.maybe_get_child(get_node, !left)?;
        let child_container = tree.child_container_mut(!left);
        SparseTree::maybe_rebalance(child_container, get_node)?;
        tree.update_link(!left);

        // continue if still unbalanced
        SparseTree::maybe_rebalance(self_container, get_node)
    }

    fn rotate(
        self_container: &mut Option<Box<SparseTree>>,
        get_node: &mut GetNodeFn,
        left: bool
    ) -> Result<()> {
        // take ownership of self. very inspiring.
        let mut tree = self_container.take()
            .expect("container must not be empty");

        // take ownership of child. just like when Karen took the fucking kids
        // and moved to her sister's place :(
        tree.maybe_get_child(get_node, left)?;
        let child_container = tree.child_container_mut(left);
        let mut child = child_container.take()
            .expect("child container must not be empty");

        // also take grandchild (may be None)
        child.maybe_get_child(get_node, !left)?;
        let grandchild_container = child.child_container_mut(!left);
        let grandchild = grandchild_container.take();

        // switcheroo (this is the actual rotation)
        *child_container = grandchild;
        tree.update_link(left);
        grandchild_container.replace(tree);
        child.update_link(!left);
        self_container.replace(child);

        Ok(())
    }
}

impl Deref for SparseTree {
    type Target = Node;

    fn deref(&self) -> &Node {
        &self.node
    }
}

impl DerefMut for SparseTree {
    fn deref_mut(&mut self) -> &mut Node {
        &mut self.node
    }
}

impl fmt::Debug for SparseTree {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use colored::Colorize;

        fn traverse(
            f: &mut fmt::Formatter,
            cursor: &SparseTree,
            stack: &mut Vec<bool>,
            left: bool,
            has_sibling_after: bool,
        ) {
            let depth = stack.len();

            if depth > 0 {
                // draw ancestor's vertical lines
                for &line in stack.iter().take(depth-1) {
                    write!(
                        f,
                        "{}",
                        if line { " │  " } else { "    " }
                            .dimmed()
                    ).unwrap();
                }

                // draw our connecting line to parent
                write!(
                    f,
                    "{}",
                    if has_sibling_after { " ├" } else { " └" }
                        .dimmed()
                ).unwrap();
            }

            let prefix = if depth == 0 {
                ""
            } else if left {
                "L─"
            } else {
                "R─"
            };
            writeln!(f, "{}{:?}", prefix.dimmed(), cursor.node).unwrap();

            if let Some(child) = &cursor.left {
                stack.push(true);
                traverse(f, &child, stack, true, cursor.right.is_some());
                stack.pop();
            }

            if let Some(child) = &cursor.right {
                stack.push(false);
                traverse(f, &child, stack, false, false);
                stack.pop();
            }
        };

        let mut stack = vec![];
        traverse(f, self, &mut stack, false, false);
        writeln!(f)
    }
}
