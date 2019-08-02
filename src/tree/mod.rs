mod node;
mod walk;
mod hash;

pub use node::Node;
pub use walk::{OwnedWalker, Fetch};

enum Link {
    Pruned {
        hash: Hash,
        height: u8,
        key: Vec<u8>
    },
    Modified {
        pending_writes: usize,
        height: u8,
        tree: Tree
    },
    Stored {
        hash: Hash,
        height: u8,
        tree: Tree
    }
}

impl Link {
    fn from_modified_tree(tree: Tree) -> Self {
        Link::Modified {
            height: tree.height(),
            tree
        }
    }

    fn maybe_from_modified_tree(maybe_tree: Option<Tree>) -> Option<Self> {
        maybe_tree.map(|tree| Some(Link::from_modified_tree(tree)))
    }

    fn tree(&self) -> Option<&Tree> {
        match self {
            Link::Pruned => None,
            Link::Modified { tree } => Some(&tree),
            Link::Stored { tree } => Some(&tree)
        }
    }

    fn height(&self) -> u8 {
        match self {
            Link::Pruned { height } => height,
            Link::Modified { height } => height,
            Link::Stored { height } => height
        }
    }

    // fn prune(&mut self) {
    //     *self = match self {
    //         Link::Pruned => self,
    //         Link::Modified => panic!("Cannot prune Modified tree"),
    //         Link::Stored { hash, height, tree } => Link::Pruned {
    //             hash,
    //             height,
    //             key: tree.key
    //         }
    //     };
    // }
}

struct TreeInner {
    node: Node, // TODO: key, value, kv_hash? or in its own struct
    left: Option<Link>,
    right: Option<Link>
}

pub struct Tree {
    inner: Box<TreeInner>
}

impl Tree {
    pub fn new(node: Node) -> Self {
        Tree {
            inner: Box::new(TreeInner {
                node,
                left: None,
                right: None
            })
        }
    }

    #[inline]
    pub fn link(&self, left: bool) -> Option<&Link> {
        if left {
            &self.inner.left
        } else {
            &self.inner.right
        }
    }

    pub fn child(&self, left: bool) -> Option<&Self> {
        self.link(left).map(|link| link.tree())
    }

    pub fn attach(mut self, left: bool, maybe_child: Option<Self>) -> Self {
        let slot = self.slot_mut(left);

        if slot.is_some() {
            panic!(
                "Tried to attach to {} tree slot, but it is already Some",
                side_to_str(left)
            );
        }

        *slot = Some(Link::maybe_from_modified_tree(maybe_child));

        self
    }

    pub fn detach(&mut self, left: bool) -> Option<Self> {
        match self.slot_mut(left).take() {
            None => None,
            Some(Link::Pruned) => None,
            Some(Link::Modified { tree }) => Some(tree),
            Some(Link::Stored { tree }) => Some(tree)
        }
    }

    pub fn detach_expect(&mut self, left: bool) -> Self {
        let maybe_child = self.detach(left);

        if let Some(child) = maybe_child {
            child
        } else {
            panic!(
                "Expected tree to have {} child, but got None",
                side_to_str(left)
            );
        }
    }

    fn slot_mut(&mut self, left: bool) -> &mut Option<Link> {
        if left {
            &mut self.inner.left
        } else {
            &mut self.inner.right
        }
    }

    #[inline]
    pub fn node(&self) -> &Node {
        &self.inner.node
    }

    pub fn with_value(mut self, value: &[u8]) -> Self {
        self.inner.node.set_value(value);
        self
    } 
}

pub fn side_to_str(left: bool) -> &'static str {
    if left { "left" } else { "right" }
}

#[cfg(test)]
mod test {
    use super::{Tree, Node};

    // struct SumNode {
    //     n: usize,
    //     left_sum: usize,
    //     right_sum: usize
    // }

    // impl SumNode {
    //     fn new(n: usize) -> Self {
    //         SumNode { n, left_sum: 0, right_sum: 0 }
    //     }

    //     fn sum(&self) -> usize {
    //         self.n + self.left_sum + self.right_sum
    //     }
    // }

    // impl Node for SumNode {
    //     fn link_to(&mut self, left: bool, child: Option<&Self>) {
    //         let sum = child.map_or(0, |c| c.sum());
    //         if left {
    //             self.left_sum = sum;
    //         } else {
    //             self.right_sum = sum;
    //         }
    //     }
    // }

    // #[test]
    // fn build_tree() {
    //     let tree = Tree::new(SumNode::new(1));
    //     assert_eq!(tree.node().sum(), 1);
    //     assert!(tree.child(true).is_none());
    //     assert!(tree.child(false).is_none());

    //     let tree = tree.attach(true, None);
    //     assert_eq!(tree.node().sum(), 1);
    //     assert!(tree.child(true).is_none());
    //     assert!(tree.child(false).is_none());

    //     let tree = tree.attach(
    //         true,
    //         Some(Tree::new(SumNode::new(2)))
    //     );
    //     assert_eq!(tree.node().sum(), 3);
    //     assert_eq!(tree.child(true).unwrap().node().sum(), 2);
    //     assert!(tree.child(false).is_none());

    //     let tree = Tree::new(SumNode::new(3))
    //         .attach(false, Some(tree));
    //     assert_eq!(tree.node().sum(), 6);
    //     assert_eq!(tree.child(false).unwrap().node().sum(), 3);
    //     assert!(tree.child(true).is_none());
    // }

    // #[should_panic]
    // #[test]
    // fn attach_existing() {
    //     Tree::new(SumNode::new(1))
    //         .attach(true, Some(Tree::new(SumNode::new(1))))
    //         .attach(true, Some(Tree::new(SumNode::new(1))));
    // }

    // #[test]
    // fn detach() {
    //     let tree = Tree::new(SumNode::new(1))
    //         .attach(true, Some(Tree::new(SumNode::new(1))))
    //         .attach(false, Some(Tree::new(SumNode::new(1))));

    //     let (tree, left_opt) = tree.detach(true);
    //     assert_eq!(tree.node().sum(), 3);
    //     assert!(tree.child(true).is_none());
    //     assert!(tree.child(false).is_some());
    //     assert_eq!(left_opt.as_ref().unwrap().node().sum(), 1);

    //     let (tree, right) = tree.detach_expect(false);
    //     assert_eq!(tree.node().sum(), 3);
    //     assert!(tree.child(true).is_none());
    //     assert!(tree.child(false).is_none());
    //     assert_eq!(right.node().sum(), 1);

    //     let tree = tree
    //         .attach(true, left_opt)
    //         .attach(false, Some(right));
    //     assert_eq!(tree.node().sum(), 3);
    //     assert!(tree.child(true).is_some());
    //     assert!(tree.child(false).is_some());
    // }
}
