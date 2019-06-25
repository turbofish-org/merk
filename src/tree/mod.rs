mod hash;
mod node;

use std::fmt;
use std::ops::{Deref, DerefMut};

use crate::error::Result;
use crate::ops::*;

pub use node::*;

pub trait GetNodeFn = FnMut(&Link) -> Result<Node>;

/// A selection of connected nodes in a tree.
///
/// Trees are acyclic, and always have at least one node.
///
/// Operations fetch [`Node`s] from the backing database lazily, and retain them
/// in memory. Mutation operations only operate on the in-memory structure, but
/// a consumer can flush the updated structure to a backing database.
///
/// [`Node`s]: struct.Node.html
pub struct Tree {
    pub node: Node,
    pub left: TreeContainer,
    pub right: TreeContainer
}

pub type TreeContainer = Option<Box<Tree>>;

///
impl Tree {
    /// Returns a new Tree which has the gien `Node` as its root, and no
    /// children.
    pub fn new(node: Node) -> Tree {
        Tree {
            node,
            left: None,
            right: None,
        }
    }

    pub fn remove(
        self_container: &mut TreeContainer,
        get_node: &mut dyn GetNodeFn
    ) -> Result<Box<Tree>> {
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
            if tall_child.is_some() {
                let edge = Tree::remove_edge(&mut tall_child, get_node, !left)?.unwrap();
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
        Tree::maybe_rebalance(self_container, get_node)?;

        Ok(tree)
    }

    pub fn remove_edge(
        self_container: &mut TreeContainer,
        get_node: &mut dyn GetNodeFn,
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
                if tree.maybe_get_child(get_node, !left)?.is_some() {
                    *self_container = tree.child_container_mut(!left).take();
                    tree.update_link(!left);
                }

                Ok(tree_container)
            },
            Some(_) => {
                let child = tree.child_container_mut(left);
                let result = Tree::remove_edge(child, get_node, left);
                tree.update_link(left);
                // rebalance if necessary
                Tree::maybe_rebalance(self_container, get_node)?;
                result
            }
        }
    }

    pub fn prune(&mut self) {
        // TODO: keep upper levels of tree?
        self.left.take();
        self.right.take();
    }

    pub fn load_all(&mut self, get_node: &mut dyn GetNodeFn) -> Result<()> {
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

    pub fn map_branch<F: FnMut(&Node)>(
        tree: Option<&mut Tree>,
        get_node: &mut dyn GetNodeFn,
        key: &[u8],
        f: &mut F
    ) -> Result<()> {
        fn traverse<F: FnMut(&Node)>(
            tree: Option<&mut Tree>,
            get_node: &mut dyn GetNodeFn,
            key: &[u8],
            f: &mut F
        ) -> Result<()> {
            let tree = match tree {
                None => return Ok(()),
                Some(tree) => tree
            };

            f(tree.node());

            if tree.key == key {
                // found target, return
                Ok(())
            } else if &tree.key[..] < key {
                // try to descend to right child
                tree.maybe_get_child(get_node, false)?;
                Ok(())
            } else {
                // try to descend to left child
                tree.maybe_get_child(get_node, true)?;
                Ok(())
            }
        }

        traverse(tree, get_node, key, f)
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
    pub fn child_tree(&self, left: bool) -> Option<&Tree> {
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
        get_node: &mut dyn GetNodeFn,
        left: bool,
    ) -> Result<Option<&mut Tree>> {
        if let Some(link) = self.child_link(left) {
            // node has a link, get from memory or fetch from db

            if self.child_tree(left).is_none() {
                // fetch child from db and put it in child field
                let node = get_node(&link)?;
                let child_container = self.child_container_mut(left);
                *child_container = Some(Box::new(Tree::new(node)));
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
        get_node: &mut dyn GetNodeFn
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
            Tree::rotate(child_container, get_node, !left)?;
            Tree::maybe_rebalance(child_container, get_node)?;
            tree.update_link(left);
        }

        // do the rotation
        Tree::rotate(self_container, get_node, left)?;
        let tree = self_container.as_mut()
            .expect("container must not be empty");

        // rebalance recursively if necessary
        tree.maybe_get_child(get_node, !left)?;
        let child_container = tree.child_container_mut(!left);
        Tree::maybe_rebalance(child_container, get_node)?;
        tree.update_link(!left);

        // continue if still unbalanced
        Tree::maybe_rebalance(self_container, get_node)
    }

    fn rotate(
        self_container: &mut TreeContainer,
        get_node: &mut dyn GetNodeFn,
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

impl PartialEq for Tree {
    fn eq(&self, other: &Tree) -> bool {
        self.node == other.node
    }
}

impl Deref for Tree {
    type Target = Node;

    fn deref(&self) -> &Node {
        &self.node
    }
}

impl DerefMut for Tree {
    fn deref_mut(&mut self) -> &mut Node {
        &mut self.node
    }
}

impl fmt::Debug for Tree {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use colored::Colorize;

        // TODO: show sparse links

        fn traverse(
            f: &mut fmt::Formatter,
            cursor: &Tree,
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

#[cfg(test)]
mod tests {
    extern crate rand;

    use rand::prelude::*;

    use super::*;
    use crate::ops::*;

    // #[test]
    // fn empty_single_put() {
    //     let batch: &[BatchEntry] = &[
    //         (b"0000", Op::Put(b"0000"))
    //     ];
    //     let tree = Tree::apply(batch).unwrap().unwrap();

    //     assert_eq!(tree.node().key, b"0000");
    //     assert_eq!(tree.node().value, b"0000");
    //     assert_tree_valid(&tree);
    // }

    // #[test]
    // fn empty_1k_put() {
    //     let mut keys: Vec<[u8; 4]> = vec![];
    //     for i in 0..1000 {
    //         let key = (i as u32).to_be_bytes();
    //         keys.push(key);
    //     }

    //     let mut batch: Vec<BatchEntry> = vec![];
    //     for key in keys.iter() {
    //         batch.push((key, Op::Put(key)));
    //     }

    //     let tree = Tree::apply(&batch).unwrap().unwrap();
    //     assert_tree_valid(&tree);
    //     assert_tree_keys(&tree, &keys);
    // }

    // #[test]
    // fn empty_deletes_only() {
    //     let batch: &[TreeBatchEntry] = &[
    //         (&[1, 2, 3], Op::Delete),
    //         (&[1, 2, 4], Op::Delete),
    //         (&[1, 2, 5], Op::Delete)
    //     ];
    //     let result = Tree::empty(batch);
    //     assert_err!(result, "Tried to delete non-existent key: [1, 2, 4]");
    // }

    // #[test]
    // fn empty_puts_and_deletes() {
    //     let batch: &[ops::BatchEntry] = &[
    //         (&[1, 2, 3], Op::Put(b"xyz")),
    //         (&[1, 2, 4], Op::Delete),
    //         (&[1, 2, 5], Op::Put(b"foo")),
    //         (&[1, 2, 6], Op::Put(b"bar"))
    //     ];
    //     let result = Tree::apply(batch);
    //     assert_err!(result, "Tried to delete non-existent key: [1, 2, 4]");
    // }

    // #[test]
    // fn empty() {
    //     let batch: &[ops::BatchEntry] = &[];
    //     let tree = Tree::apply(batch).unwrap();
    //     assert!(tree.is_none());
    // }

    // #[test]
    // fn apply_simple_insert() {
    //     let mut container = None;
    //     let batch: &[TreeBatchEntry] = &[
    //         (b"key", Op::Put(b"value"))
    //     ];
    //     Tree::apply(&mut container, &mut |_| unreachable!(), batch).unwrap();

    //     let tree = container.unwrap();
    //     assert_eq!(tree.key, b"key");
    //     assert_eq!(tree.value, b"value");
    //     assert_tree_valid(&tree);
    //     assert_tree_keys(&tree, &[b"key"]);
    // }

    // #[test]
    // fn apply_simple_update() {
    //     let mut container = Some(Box::new(
    //         Tree::new(
    //             Node::new(b"key", b"value")
    //         )
    //     ));
    //     let batch: &[TreeBatchEntry] = &[
    //         (b"key", Op::Put(b"new value"))
    //     ];
    //     Tree::apply(&mut container, &mut |_| unreachable!(), batch).unwrap();

    //     let tree = container.unwrap();
    //     assert_eq!(tree.key, b"key");
    //     assert_eq!(tree.value, b"new value");
    //     assert_tree_valid(&tree);
    //     assert_tree_keys(&tree, &[b"key"]);
    // }

    // #[test]
    // fn apply_simple_delete() {
    //     let mut container = Some(Box::new(
    //         Tree::new(
    //             Node::new(b"key", b"value")
    //         )
    //     ));
    //     let batch: &[TreeBatchEntry] = &[
    //         (b"key", Op::Delete)
    //     ];
    //     Tree::apply(&mut container, &mut |_| unreachable!(), batch).unwrap();
    //     assert_eq!(container, None);
    // }

    // #[test]
    // fn apply_insert_under() {
    //     let mut container = Some(Box::new(
    //         Tree::new(
    //             Node::new(&[5], b"value")
    //         )
    //     ));
    //     let batch: &[TreeBatchEntry] = &[
    //         (&[6], Op::Put(b"value"))
    //     ];
    //     Tree::apply(&mut container, &mut |_| unreachable!(), batch).unwrap();

    //     let tree = container.unwrap();
    //     assert_eq!(tree.key, &[5]);
    //     assert_eq!(tree.value, b"value");
    //     assert_eq!(tree.right.as_ref().unwrap().key, &[6]);
    //     assert_eq!(tree.child_tree(false).unwrap().value, b"value");
    //     assert_eq!(tree.height(), 2);
    //     assert_tree_valid(&tree);
    //     assert_tree_keys(&tree, &[[5], [6]]);
    // }

    // #[test]
    // fn apply_update_and_insert() {
    //     let mut container = Some(Box::new(
    //         Tree::new(
    //             Node::new(&[5], b"value")
    //         )
    //     ));
    //     let batch: &[TreeBatchEntry] = &[
    //         (&[5], Op::Put(b"value2")),
    //         (&[6], Op::Put(b"value3"))
    //     ];
    //     Tree::apply(&mut container, &mut |_| unreachable!(), batch).unwrap();

    //     let tree = container.unwrap();
    //     assert_eq!(tree.key, &[5]);
    //     assert_eq!(tree.value, b"value2");
    //     assert_eq!(tree.right.as_ref().unwrap().key, &[6]);
    //     assert_eq!(tree.child_tree(false).unwrap().value, b"value3");
    //     assert_eq!(tree.height(), 2);
    //     assert_tree_valid(&tree);
    //     assert_tree_keys(&tree, &[[5], [6]]);
    // }

    // #[test]
    // fn apply_insert_balance() {
    //     let mut container = Some(Box::new(
    //         Tree::new(
    //             Node::new(&[5], b"value")
    //         )
    //     ));
    //     let batch: &[TreeBatchEntry] = &[
    //         (&[6], Op::Put(b"value2")),
    //         (&[7], Op::Put(b"value3"))
    //     ];
    //     Tree::apply(&mut container, &mut |_| unreachable!(), batch).unwrap();

    //     let tree = container.unwrap();
    //     assert_eq!(tree.key, &[6]);
    //     assert_eq!(tree.value, b"value2");
    //     assert_eq!(tree.left.as_ref().unwrap().key, &[5]);
    //     assert_eq!(tree.right.as_ref().unwrap().key, &[7]);
    //     assert_eq!(tree.child_tree(true).unwrap().value, b"value");
    //     assert_eq!(tree.child_tree(false).unwrap().value, b"value3");
    //     assert_eq!(tree.height(), 2);
    //     assert_tree_valid(&tree);
    //     assert_tree_keys(&tree, &[[5], [6], [7]]);
    // }

    // #[test]
    // fn apply_delete_inner() {
    //     let mut container = Some(Box::new(
    //         Tree::new(
    //             Node::new(&[5], b"value")
    //         )
    //     ));
    //     let batch: &[TreeBatchEntry] = &[
    //         (&[6], Op::Put(b"value2")),
    //         (&[7], Op::Put(b"value3")),
    //         (&[8], Op::Put(b"value4")),
    //         (&[9], Op::Put(b"value5")),
    //     ];
    //     Tree::apply(&mut container, &mut |_| unreachable!(), batch).unwrap();

    //     let batch: &[TreeBatchEntry] = &[
    //         (&[8], Op::Delete)
    //     ];
    //     Tree::apply(&mut container, &mut |_| unreachable!(), batch).unwrap();

    //     let tree = container.unwrap();
    //     assert_eq!(tree.key, &[7]);
    //     assert_eq!(tree.left.as_ref().unwrap().key, &[5]);
    //     assert_eq!(tree.right.as_ref().unwrap().key, &[9]);
    //     assert_eq!(tree.height(), 3);
    //     assert_tree_valid(&tree);
    //     assert_tree_keys(&tree, &[[5], [6], [7], [9]]);
    // }

    // #[test]
    // fn insert_100() {
    //     let mut tree = None;
    //     let keys = sequential_keys(0, 100);
    //     let batch = puts(&keys);
    //     Tree::apply(
    //         &mut tree,
    //         &mut |_| unreachable!(),
    //         &batch
    //     ).unwrap();

    //     let tree = tree.expect("tree should not be empty");
    //     assert_tree_valid(&tree);
    //     assert_tree_keys(&tree, &keys);

    //     // known final state for deterministic tree
    //     // assert_eq!(
    //     //     hex::encode(tree.hash()),
    //     //     "ba2e3b6397061744c2dece97b12e212a292d3a1f"
    //     // );
    //     // assert_eq!(
    //     //     tree.node().key,
    //     //     [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 20, 216]
    //     // );
    //     // assert_eq!(tree.height(), 16);
    //     // assert_eq!(tree.child_height(true), 15);
    //     // assert_eq!(tree.child_height(false), 15);
    // }

    // #[test]
    // fn update_100() {
    //     let mut tree = None;
    //     let keys = sequential_keys(0, 100);
    //     let batch = puts(&keys);
    //     Tree::apply(
    //         &mut tree,
    //         &mut |_| unreachable!(),
    //         &batch
    //     ).unwrap();

    //     let tree_box = tree.as_ref().expect("tree should not be empty");
    //     assert_tree_valid(&tree_box);
    //     assert_tree_keys(&tree_box, &keys);

    //     // put sequential keys again
    //     let keys = sequential_keys(0, 100);
    //     let batch = puts(&keys);
    //     Tree::apply(&mut tree, &mut |_| unreachable!(), &batch).unwrap();

    //     let tree_box = tree.expect("tree should not be empty");
    //     assert_tree_valid(&tree_box);
    //     assert_tree_keys(&tree_box, &keys);

    //     assert_eq!(tree_box.key, &[0, 0, 0, 59]);
    //     assert_eq!(tree_box.height(), 8);
    //     assert_eq!(tree_box.child_height(true), 7);
    //     assert_eq!(tree_box.child_height(false), 6);
    // }

    // #[test]
    // fn delete_100() {
    //     let mut tree = None;
    //     let keys = sequential_keys(0, 100);
    //     let batch = puts(&keys);
    //     Tree::apply(
    //         &mut tree,
    //         &mut |_| unreachable!(),
    //         &batch
    //     ).unwrap();
    //     let tree_box = tree.as_ref().expect("tree should not be empty");
    //     assert_tree_valid(&tree_box);
    //     assert_tree_keys(&tree_box, &keys);

    //     // delete sequential keys
    //     let keys = sequential_keys(0, 100);
    //     let mut batch: Vec<TreeBatchEntry> = vec![];
    //     for key in keys.iter().take(99) {
    //         batch.push((key, Op::Delete));
    //     }
    //     Tree::apply(&mut tree, &mut |_| unreachable!(), &batch).unwrap();

    //     let tree = tree.as_ref().expect("tree should not be empty");
    //     assert_tree_valid(&tree);
    //     assert_eq!(tree.height(), 1);
    //     assert_eq!(tree.key, &keys[99]);
    // }

    // #[test]
    // fn delete_sequential() {
    //     let mut tree = None;
    //     let keys = sequential_keys(0, 100);
    //     let batch = puts(&keys);
    //     Tree::apply(
    //         &mut tree,
    //         &mut |_| unreachable!(),
    //         &batch
    //     ).unwrap();

    //     let tree_box = tree.as_ref().expect("tree should not be empty");
    //     assert_tree_valid(&tree_box);
    //     assert_tree_keys(&tree_box, &keys);

    //     // delete sequential keys
    //     let keys = sequential_keys(0, 100);
    //     for i in 0..99 {
    //         let batch: &[TreeBatchEntry] = &[
    //             (&keys[i], Op::Delete)
    //         ];
    //         Tree::apply(&mut tree, &mut |_| unreachable!(), &batch).unwrap();

    //         let tree_box = tree.as_ref().expect("tree should not be empty");
    //         assert_tree_valid(&tree_box);
    //         assert_tree_keys(&tree_box, &keys[i+1..]);
    //     }
    // }

    // #[test]
    // fn insert_sparse() {
    //     let mut tree = None;
    //     let keys = sequential_keys(0, 5);
    //     let batch = puts(&keys);
    //     Tree::apply(
    //         &mut tree,
    //         &mut |_| unreachable!(),
    //         &batch
    //     ).unwrap();

    //     let tree_box = tree.as_ref().expect("tree should not be empty");
    //     assert_tree_valid(&tree_box);
    //     assert_tree_keys(&tree_box, &keys);

    //     // add keys 5 - 10
    //     let mut cloned_tree = Some(Box::new(
    //         Tree::new(tree.as_ref().unwrap().node().clone())
    //     ));
    //     let keys = sequential_keys(5, 10);
    //     let batch = puts(&keys);
    //     let mut get_node = |link: &Link| {
    //         // get nodes from original tree (which is fully loaded in memory)
    //         println!("get node {:?}", &link.key);
    //         fn traverse (link: &Link, node: &Tree) -> Node {
    //             if &node.key == &link.key {
    //                 node.node().clone()
    //             } else if &link.key < &node.key {
    //                 traverse(link, node.left.as_ref().unwrap())
    //             } else {
    //                 traverse(link, node.right.as_ref().unwrap())
    //             }
    //         };
    //         Ok(traverse(link, tree.as_ref().unwrap()))
    //     };
    //     Tree::apply(&mut cloned_tree, &mut get_node, &batch).unwrap();

    //     cloned_tree.as_mut().unwrap().load_all(&mut get_node).unwrap();
    //     println!("{:?}", cloned_tree.as_ref().unwrap());

    //     let tree_box = cloned_tree.as_ref().expect("tree should not be empty");
    //     assert_tree_valid(&tree_box);
    //     assert_tree_keys(&tree_box, &sequential_keys(0, 10));
    // }

    // #[test]
    // fn delete_sparse() {
    //     let mut tree = None;
    //     let keys = sequential_keys(0, 100);
    //     let batch = puts(&keys);
    //     Tree::apply(
    //         &mut tree,
    //         &mut |_| unreachable!(),
    //         &batch
    //     ).unwrap();

    //     let tree_box = tree.as_ref().expect("tree should not be empty");
    //     assert_tree_valid(&tree_box);
    //     assert_tree_keys(&tree_box, &keys);

    //     // delete sequential keys
    //     let mut cloned_tree = Some(Box::new(
    //         Tree::new(tree.as_ref().unwrap().node().clone())
    //     ));
    //     let keys = sequential_keys(0, 100);
    //     let mut batch: Vec<TreeBatchEntry> = Vec::with_capacity(100);
    //     for i in 0..99 {
    //         batch.push((&keys[i], Op::Delete));
    //     }
    //     Tree::apply(
    //         &mut cloned_tree,
    //         // get nodes from original tree (which is fully loaded in memory)
    //         &mut |link| {
    //             fn traverse (link: &Link, node: &Tree) -> Node {
    //                 if &node.key == &link.key {
    //                     node.node().clone()
    //                 } else if &link.key < &node.key {
    //                     traverse(link, node.left.as_ref().unwrap())
    //                 } else {
    //                     traverse(link, node.right.as_ref().unwrap())
    //                 }
    //             };
    //             Ok(traverse(link, tree.as_ref().unwrap()))
    //         },
    //         &batch
    //     ).unwrap();

    //     let tree_box = cloned_tree.as_ref().expect("tree should not be empty");
    //     assert_tree_valid(&tree_box);
    //     assert_tree_keys(&tree_box, &[ keys.last().unwrap() ]);
    // }

    // #[test]
    // fn fuzz() {
    //     use std::collections::HashSet;

    //     let mut rng = rand::thread_rng();

    //     let mut values = vec![];
    //     for _ in 0..1_000 {
    //         let length = (rng.gen::<u8>() as usize) % 5;
    //         let value = random_bytes(&mut rng, length);
    //         values.push(value);
    //     }

    //     for _ in 0..10 {
    //         let mut tree = None;
            
    //         let mut keys: Vec<Vec<u8>> = vec![];
    //         let mut key_set = HashSet::new();

    //         for i in 0..100 {
    //             println!("batch {}", i);

    //             let modify_count = if keys.len() > 10 {
    //                 rng.gen::<usize>() % std::cmp::min(keys.len() / 10, 10)
    //             } else {
    //                 0
    //             };
    //             let insert_count = rng.gen::<usize>() % 10;

    //             let mut batch: Vec<TreeBatchEntry> = Vec::with_capacity(
    //                 modify_count + insert_count
    //             );

    //             // updates/deletes
    //             let mut batch_keys = HashSet::new();
    //             let mut delete_keys = HashSet::new();
    //             for _ in 0..modify_count {
    //                 loop {
    //                     let index = rng.gen::<usize>() % keys.len();
    //                     let key = &keys[index];

    //                     // don't allow duplicate keys
    //                     let not_duplicate = batch_keys.insert(key);
    //                     if !not_duplicate { continue }

    //                     // add to batch
    //                     let op = if rng.gen::<bool>() {
    //                         let index = rng.gen::<usize>() % values.len();
    //                         Op::Put(&values[index])
    //                     } else {
    //                         delete_keys.insert(key.clone());
    //                         Op::Delete
    //                     };
    //                     batch.push((&key, op));

    //                     break
    //                 }
    //             }

    //             // inserts
    //             let mut insert_keys: Vec<Vec<u8>> = Vec::with_capacity(insert_count);
    //             for _ in 0..insert_count {
    //                 loop {
    //                     let length = rng.gen::<usize>() % 4;
    //                     let key = random_bytes(&mut rng, length);
    //                     let key2 = key.clone();

    //                     // don't allow duplicate keys
    //                     let not_duplicate = key_set.insert(key2);
    //                     if !not_duplicate { continue }

    //                     insert_keys.push(key);
    //                     break
    //                 }
    //             }

    //             for key in insert_keys.iter() {
    //                 let index = rng.gen::<usize>() % values.len();
    //                 batch.push((&key, Op::Put(&values[index])));
    //             }

    //             // sort batch
    //             batch.sort_by(|a, b| a.0.cmp(&b.0));

    //             // apply batch
    //             println!("applying, batch size: {}, tree size: {}", batch.len(), keys.len());
    //             Tree::apply(&mut tree, &mut |_| unreachable!(), &batch).unwrap();

    //             // add newly inserted keys to keys vector
    //             keys.append(&mut insert_keys);
    //             // remove deleted keys from keys vector
    //             let mut new_keys = Vec::with_capacity(
    //                 keys.len() - delete_keys.len()
    //             );
    //             for key in keys.iter() {
    //                 if !delete_keys.contains(key) {
    //                     new_keys.push(key.clone());
    //                 }
    //             }
    //             keys = new_keys;
    //             // sort keys
    //             keys.sort_by(|a, b| a.cmp(&b));

    //             // check tree
    //             match tree.as_ref() {
    //                 Some(tree) => {
    //                     assert_tree_valid(tree);
    //                     assert_tree_keys(tree, &keys);
    //                 },
    //                 None => {
    //                     assert_eq!(keys.len(), 0);
    //                 }
    //             }
    //         }
    //     }
    // }

    // fn random_bytes(rng: &mut ThreadRng, length: usize) -> Vec<u8> {
    //     (0..length).map(|_| -> u8 { rng.gen() }).collect()
    // }

    // /// Recursively asserts invariants for each node in the tree.
    // fn assert_tree_valid(tree: &Tree) {
    //     assert!(
    //         tree.balance_factor().abs() <= 1,
    //         format!(
    //             "node should be balanced. bf={}",
    //             tree.balance_factor()
    //         )
    //     );

    //     let assert_child_valid = |child: &Tree, left: bool| {
    //         assert!(
    //             (child.node().key < tree.node().key) == left,
    //             "child should be ordered by key"
    //         );

    //         assert_eq!(
    //             tree.child_link(left).as_ref().unwrap(),
    //             &child.as_link(),
    //             "parent link should match child"
    //         );

    //         // recursive validity check
    //         assert_tree_valid(child);
    //     };

    //     // check left child
    //     if let Some(left) = tree.child_tree(true) {
    //         assert_child_valid(left, true);
    //     }

    //     // check right child
    //     if let Some(right) = tree.child_tree(false) {
    //         assert_child_valid(right, false);
    //     }

    //     // ensure keys are globally ordered (root only)
    //     let keys = tree_keys(tree);
    //     if !keys.is_empty() {
    //         let mut prev = &keys[0];
    //         for key in keys[1..].iter() {
    //             assert!(key > prev);
    //             prev = &key;
    //         }
    //     }
    // }

    // fn tree_keys<'a>(tree: &'a Tree) -> Vec<&'a [u8]> {
    //     fn traverse<'a>(tree: &'a Tree, keys: Vec<&'a [u8]>) -> Vec<&'a [u8]> {
    //         let mut keys = match tree.child_tree(true) {
    //             None => keys,
    //             Some(child) => traverse(child, keys)
    //         };

    //         keys.push(&tree.key);

    //         match tree.child_tree(false) {
    //             None => keys,
    //             Some(child) => traverse(child, keys)
    //         }
    //     }

    //     traverse(tree, vec![])
    // }

    // fn assert_tree_keys<K: AsRef<[u8]>>(tree: &Tree, expected_keys: &[K]) {
    //     let actual_keys = tree_keys(tree);
    //     println!("keys {:?}", actual_keys);
    //     assert_eq!(actual_keys.len(), expected_keys.len());
    //     for i in 0..actual_keys.len() {
    //         assert_eq!(actual_keys[i], expected_keys[i].as_ref());
    //     }
    // }

    // fn sequential_keys(start: usize, end: usize) -> Vec<[u8; 4]> {
    //     let mut keys = vec![];
    //     for i in start..end {
    //         keys.push((i as u32).to_be_bytes());
    //     }
    //     keys
    // }

    // fn puts<'a>(keys: &'a [[u8; 4]]) -> Vec<TreeBatchEntry<'a>> {
    //     let mut batch: Vec<TreeBatchEntry> = vec![];
    //     for key in keys.iter() {
    //         batch.push((&key[..], Op::Put(b"x")));
    //     }
    //     batch
    // }
}