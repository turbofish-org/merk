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
    pub node: Node,
    pub left: TreeContainer,
    pub right: TreeContainer
}

pub enum TreeOp<'a> {
    Put(&'a [u8]),
    Delete
}

impl<'a> fmt::Debug for TreeOp<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "{}", match self {
            Put(value) => format!("Put({:?})", value),
            Delete => "Delete".to_string()
        })
    }
}


pub type TreeBatchEntry<'a> = (&'a [u8], TreeOp<'a>);
pub type TreeBatch<'a> = [TreeBatchEntry<'a>];

pub type TreeContainer = Option<Box<SparseTree>>;

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

    pub fn from_batch(batch: &TreeBatch) -> Result<TreeContainer> {
        if batch.is_empty() {
            return Ok(None);
        }

        let mid = batch.len() / 2;
        let (mid_key, mid_op) = &batch[mid];

        let mid_value = match mid_op {
            Delete => bail!("Tried to delete non-existent key: {:?}", mid_key),
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
        SparseTree::apply(
            &mut tree,
            // this is a fresh tree so we never need to fetch nodes
            &mut |_| unreachable!("should not fetch"),
            left_batch
        )?;

        let right_batch = &batch[mid+1..];
        SparseTree::apply(
            &mut tree,
            // this is a fresh tree so we never need to fetch nodes
            &mut |_| unreachable!("should not fetch"),
            right_batch
        )?;

        Ok(tree)
    }

    pub fn modified<'a>(&'a self) -> Result<Vec<(&'a [u8], Vec<u8>)>> {
        fn traverse<'a>(
            tree: &'a SparseTree,
            output: Vec<(&'a [u8], Vec<u8>)>
        ) -> Result<Vec<(&'a [u8], Vec<u8>)>> {
            let mut output = if let Some(child) = tree.child_tree(true) {
                traverse(child, output)?
            } else {
                output
            };

            // TODO: Result
            let bytes = tree.encode()?;
            output.push((&tree.key, bytes));

            if let Some(child) = tree.child_tree(false) {
                traverse(child, output)
            } else {
                Ok(output)
            }
        }

        traverse(self, vec![])
    }

    /// Applies the batch of operations (puts and deletes) to the tree.
    ///
    /// The tree structure and relevant Merkle hashes are updated in memory.
    ///
    /// This method will fetch relevant missing nodes (if any) from the backing
    /// database.
    ///
    /// **NOTE:** The keys in the batch *MUST* be sorted and unique. This
    /// condition is checked in debug builds, but for performance reasons it is
    /// unchecked in release builds - unsorted or duplicate keys will result in
    /// undefined behavior.
    pub fn apply(
        self_container: &mut TreeContainer,
        get_node: &mut GetNodeFn,
        batch: &TreeBatch
    ) -> Result<()> {
        if batch.is_empty() {
            return Ok(());
        }

        // ensure keys in batch are sorted and unique. this check is expensive,
        // so we only do it in debug builds. in release builds, non-sorted or
        // duplicate keys results in UB!
        for pair in batch.windows(2) {
            debug_assert!(
                pair[0].0 < pair[1].0,
                "keys must be sorted and unique"
            );
        }

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
            |(key, _op)| key.cmp(&&tree.key[..])
        );
        let (left_batch, right_batch) = match search {
            Ok(index) => {
                // a key matches this node's key, apply op to this node
                match batch[index].1 {
                    Put(value) => tree.set_value(value),
                    Delete => {
                        SparseTree::remove(self_container, get_node)?;
                        SparseTree::apply(self_container, get_node, &batch[..index])?;
                        return SparseTree::apply(self_container, get_node, &batch[index + 1..])
                    }
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

    pub fn remove(
        self_container: &mut TreeContainer,
        get_node: &mut GetNodeFn
    ) -> Result<Box<SparseTree>> {
        let tree = match self_container {
            None => unreachable!("cannot delete empty tree"),
            Some(tree) => tree
        };

        let has_left = tree.child_link(true).is_some();
        let has_right = tree.child_link(false).is_some();

        let mut tree = self_container.take().unwrap();

        if has_left && has_right {
            tree.maybe_get_child(get_node, true)?;
            tree.maybe_get_child(get_node, false)?;

            // promote edge of taller child
            let left = tree.child_height(true) > tree.child_height(false);
            let mut tall_child = tree.child_container_mut(left).take();
            if let Some(_) = tall_child {
                let edge = SparseTree::remove_edge(&mut tall_child, get_node, !left)?.unwrap();
                self_container.replace(edge);

                let short_child = tree.child_container_mut(!left).take();
                let edge = self_container.as_mut().unwrap();
                *edge.child_container_mut(left) = tall_child;
                *edge.child_container_mut(!left) = short_child;
                edge.update_link(true);
                edge.update_link(false);
            }
        } else if has_left {
            // TODO: we shouldn't need to actually fetch node, just update link
            tree.maybe_get_child(get_node, true)?;
            *self_container = tree.left.take();
        } else if has_right {
            // TODO: we shouldn't need to actually fetch node, just update link
            tree.maybe_get_child(get_node, false)?;
            *self_container = tree.right.take();
        }

        // rebalance if necessary
        SparseTree::maybe_rebalance(self_container, get_node)?;

        Ok(tree)
    }

    pub fn remove_edge(
        self_container: &mut TreeContainer,
        get_node: &mut GetNodeFn,
        left: bool
    ) -> Result<TreeContainer> {
        let tree = match self_container {
            None => unreachable!("called edge on empty tree"),
            Some(tree) => tree.as_mut()
        };

        match tree.maybe_get_child(get_node, left)? {
            None => {
                let mut tree_container = self_container.take();

                // promote edge's child if it exists
                let tree = tree_container.as_mut().unwrap(); 
                if let Some(_) = tree.maybe_get_child(get_node, !left)? {
                    *self_container = tree.child_container_mut(!left).take();
                    tree.update_link(!left);
                }

                Ok(tree_container)
            },
            Some(_) => {
                let child = tree.child_container_mut(left);
                let result = SparseTree::remove_edge(child, get_node, left);
                tree.update_link(left);
                // rebalance if necessary
                SparseTree::maybe_rebalance(self_container, get_node)?;
                result
            }
        }
    }

    pub fn prune(&mut self) {
        // TODO: keep upper levels of tree?
        self.left.take();
        self.right.take();
    }

    pub fn load_all(&mut self, get_node: &mut GetNodeFn) -> Result<()> {
        self.maybe_get_child(get_node, true)?;
        self.maybe_get_child(get_node, false)?;

        if let Some(left) = &mut self.left {
            left.load_all(get_node)?;
        }
        if let Some(right) = &mut self.right {
            right.load_all(get_node)?;
        }
        Ok(())
    }

    #[inline]
    pub fn node(&self) -> &Node {
        &self.node
    }

    /// Compute child link and set on our node.
    pub fn update_link(&mut self, left: bool) {
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
    fn child_container_mut(&mut self, left: bool) -> &mut TreeContainer {
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
        self_container: &mut TreeContainer,
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
            tree.update_link(left);
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
        self_container: &mut TreeContainer,
        get_node: &mut GetNodeFn,
        left: bool
    ) -> Result<()> {
        // take ownership of self. very inspiring.
        let mut tree = self_container.take()
            .expect("container must not be empty");

        // take ownership of child
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

impl PartialEq for SparseTree {
    fn eq(&self, other: &SparseTree) -> bool {
        self.node == other.node
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

        // TODO: show sparse links

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
