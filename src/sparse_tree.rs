use std::fmt;
use std::ops::{Deref, DerefMut};

use crate::error::Result;
use crate::node::{Link, Node};

pub trait GetNodeFn = FnMut(&Link) -> Result<Node>;

/// A selection of connected nodes in a tree.
///
/// SparseTrees are acyclic, and always have at least one node.
///
/// Operations fetch [`Node`s] from the backing database lazily,
/// and retain them in memory. Mutation operations only operate
/// on the in-memory structure, but a consumer can flush the
/// updated structure to a backing database.
///
/// [`Node`s]: struct.Node.html
pub struct SparseTree {
    node: Node,
    left: Option<Box<SparseTree>>,
    right: Option<Box<SparseTree>>,
}

///
impl SparseTree {
    /// Returns a new SparseTree which has the gien `Node` as its
    /// root, and no children.
    pub fn new(node: Node) -> SparseTree {
        SparseTree {
            node,
            left: None,
            right: None,
        }
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

    /// Puts the batch of key/value pairs into the tree.
    ///
    /// The tree structure and relevant Merkle hashes
    /// are updated in memory.
    ///
    /// This method will fetch relevant missing nodes
    /// (if any) from the backing database.
    pub fn put_batch(&mut self, get_node: &mut GetNodeFn, batch: &[(&[u8], &[u8])]) -> Result<()> {
        // TODO: build and return db batch, and maybe prune as we ascend

        // binary search to see if this node's key is in the batch,
        // and to split into left_batch and right_batch
        let search = batch.binary_search_by(|(key, _value)| key.cmp(&&self.node.key[..]));
        let (left_batch, right_batch) = match search {
            Ok(index) => {
                // a key matches this node's key, update the value
                self.set_value(batch[index].1);
                // split batch into left and right batches for recursive puts,
                // exluding matched value
                (&batch[..index], &batch[index + 1..])
            }
            Err(index) => {
                // split batch into left and right batches for recursive puts
                batch.split_at(index)
            }
        };

        let mut recurse = |batch, left| -> Result<()> {
            // try to get child, fetching from db if necessary
            match self.maybe_get_child(get_node, left)? {
                Some(child_tree) => {
                    // recursively put value under child
                    child_tree.put_batch(get_node, batch)?;

                    // update link since we know child hash changed
                    self.update_link(left);
                }
                None => {
                    // no child here, create subtree set as child,
                    // with middle value as root (to minimize balances)
                    let mid = batch.len() / 2;
                    let mut child_tree =
                        Box::new(SparseTree::new(Node::new(batch[mid].0, batch[mid].1)));

                    // add the rest of the batch to the new subtree
                    if batch.len() > 1 {
                        child_tree.put_batch(get_node, &batch[..mid])?;
                    }
                    if batch.len() > 2 {
                        child_tree.put_batch(get_node, &batch[mid + 1..])?;
                    }

                    // set child field, update link, and update child's parent_key
                    self.set_child(left, Some(child_tree));
                }
            };
            Ok(())
        };
        if !left_batch.is_empty() {
            recurse(left_batch, true)?;
        }
        if !right_batch.is_empty() {
            recurse(right_batch, false)?;
        }

        // rebalance if necessary
        self.maybe_rebalance(get_node)
    }

    pub fn prune(&mut self) {
        // TODO: keep upper levels of tree?
        self.left.take();
        self.right.take();
    }

    pub fn node(&self) -> &Node {
        &self.node
    }

    fn update_link(&mut self, left: bool) {
        // compute child link
        let link = self.child_tree(left).map(|child| child.as_link());

        // set link on our Node
        self.node.set_child(left, link);
    }

    fn set_child(&mut self, left: bool, child_tree: Option<Box<SparseTree>>) {
        // set child field
        {
            let child_field = self.child_field_mut(left);
            *child_field = child_tree;
        }

        // update link
        self.update_link(left);

        // update child node's parent_key to point to us
        let self_key = self.node.key.clone();
        let child_field = self.child_field_mut(left);
        if let Some(child) = child_field.as_mut() {
            child.set_parent(Some(self_key));
        }
    }

    #[inline]
    pub fn child_tree(&self, left: bool) -> Option<&SparseTree> {
        let option = if left { &self.left } else { &self.right };
        option.as_ref().map(|x| x.as_ref())
    }

    #[inline]
    fn child_field_mut(&mut self, left: bool) -> &mut Option<Box<SparseTree>> {
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
                let child_field = self.child_field_mut(left);
                *child_field = Some(Box::new(SparseTree::new(node)));
            }

            let child_field = self.child_field_mut(left);
            Ok(child_field.as_mut().map(|x| x.as_mut()))
        } else {
            // node has no link, nothing to get
            Ok(None)
        }
    }

    fn maybe_rebalance(&mut self, get_node: &mut GetNodeFn) -> Result<()> {
        let balance_factor = self.balance_factor();

        // return early if we don't need to balance
        if balance_factor.abs() <= 1 {
            return Ok(());
        }

        // get child
        let left = balance_factor < 0;
        // (this unwrap should never panic, if the tree
        // is unbalanced in this direction then we know
        // there is a child)
        let child = self.maybe_get_child(get_node, left)?.unwrap();

        // maybe do a double rotation
        if left == (child.balance_factor() > 0) {
            // rotate child opposite direction, then update link
            child.rotate(get_node, !left)?;
            self.update_link(left);
        }

        self.rotate(get_node, left)?;

        // rebalance recursively if necessary
        let child = self.maybe_get_child(get_node, !left)?.unwrap();
        child.maybe_rebalance(get_node)?;
        self.update_link(!left);

        // rebalance self if necessary
        self.maybe_rebalance(get_node)?;

        Ok(())
    }

    fn rotate(&mut self, get_node: &mut GetNodeFn, left: bool) -> Result<()> {
        self.maybe_get_child(get_node, left)?;
        let mut child = self.child_field_mut(left).take().unwrap();

        child.maybe_get_child(get_node, !left)?;
        let grandchild = child.child_field_mut(!left).take();
        self.set_child(left, grandchild);

        self.swap(child.as_mut());
        self.update_link(left);
        child.update_link(!left);
        self.set_child(!left, Some(child));

        Ok(())
    }

    fn swap(&mut self, other: &mut SparseTree) {
        // TODO: speed up by only cloning 1, and reassigning other

        let self_node = self.node.clone();
        let self_left = self.left.take();
        let self_right = self.right.take();
        let self_parent = self.node.parent_key.take();
        let other_parent = other.node.parent_key.take();

        self.node = other.node.clone();
        self.left = other.left.take();
        self.right = other.right.take();
        self.set_parent(self_parent);

        other.node = self_node;
        other.left = self_left;
        other.right = self_right;
        other.set_parent(other_parent);
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
                for i in 0..depth - 1 {
                    write!(
                        f,
                        "{}",
                        match stack[i] {
                            true => " │   ",
                            false => "    ",
                        }
                        .dimmed()
                    ).unwrap();
                }

                // draw our connecting line to parent
                write!(
                    f,
                    "{}",
                    match has_sibling_after {
                        true => " ├",
                        false => " └",
                    }
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
            write!(f, "{}{:?}\n", prefix.dimmed(), cursor.node).unwrap();

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
        write!(f, "\n")
    }
}
