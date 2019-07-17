mod node;
mod walk;
mod build;

pub use node::Node;

pub struct TreeInner<N: Node> {
    node: N,
    left: Option<Tree<N>>,
    right: Option<Tree<N>>,
}

pub struct Tree<N: Node> {
    inner: Box<TreeInner<N>>,
}

impl<N: Node> Tree<N> {
    pub fn new(node: N) -> Self {
        Tree {
            inner: Box::new(TreeInner {
                node,
                left: None,
                right: None
            })
        }
    }

    pub fn node(&self) -> &N {
        &self.inner.node
    }

    pub fn child(&self, left: bool) -> Option<&Self> {
        let child = if left {
            &self.inner.left
        } else {
            &self.inner.right
        };
        child.as_ref()
    }

    pub fn attach(mut self, left: bool, maybe_child: Option<Self>) -> Self {
        if self.child(left).is_some() {
            unreachable!(format!(
                "Tried to attach to {} tree slot, but it is already Some",
                side_to_str(left)
            ));
        }
        
        let maybe_child_node = maybe_child.as_ref().map(|c| &c.node);
        self.inner.node.link_to(left, maybe_child_node);

        let slot = self.slot_mut(left);
        *slot = maybe_child;

        self
    }

    pub fn detach(mut self, left: bool) -> (Self, Option<Self>) {
        let slot = self.slot_mut(left);
        let maybe_child = slot.take();
        (self, maybe_child)
    }

    pub fn detach_expect(mut self, left: bool) -> (Self, Self) {
        let (tree, maybe_child) = self.detach(left);

        if let Some(child) = maybe_child {
            (tree, child)
        } else {
            unreachable!(format!(
                "Expected tree to have {} child, but got None",
                side_to_str(left)
            ));
        }
    }

    fn slot_mut(&mut self, left: bool) -> &mut Option<Self> {
        if left {
            &mut self.inner.left
        } else {
            &mut self.inner.right
        }
    }

    #[inline]
    pub fn node(&self) -> &N {
        &self.inner.node
    }
}

fn side_to_str(left: bool) -> &'static str {
    if left { "left" } else { "right" }
}

#[cfg(test)]
mod test {
    use super::{Tree, Node};

    struct SumNode {
        n: usize,
        left_sum: usize,
        right_sum: usize
    }

    impl SumNode {
        fn new(n: usize) -> Self {
            SumNode { n, left_sum: 0, right_sum: 0 }
        }

        fn sum(&self) -> usize {
            self.n + self.left_sum + self.right_sum
        }
    }

    impl Node for SumNode {
        fn link_to(&mut self, left: bool, child: Option<&Self>) {
            let sum = child.map_or(0, |c| c.sum());
            if left {
                self.left_sum = sum;
            } else {
                self.right_sum = sum;
            }
        }
    }

    #[test]
    fn build_tree() {
        let tree = Tree::new(SumNode::new(1));
        assert_eq!(tree.node().sum(), 1);
        assert!(tree.child(true).is_none());
        assert!(tree.child(false).is_none());

        let tree = tree.attach(true, None);
        assert_eq!(tree.node().sum(), 1);
        assert!(tree.child(true).is_none());
        assert!(tree.child(false).is_none());

        let tree = tree.attach(
            true,
            Some(Tree::new(SumNode::new(2)))
        );
        assert_eq!(tree.node().sum(), 3);
        assert_eq!(tree.child(true).unwrap().node().sum(), 2);
        assert!(tree.child(false).is_none());

        let tree = Tree::new(SumNode::new(3))
            .attach(false, Some(tree));
        assert_eq!(tree.node().sum(), 6);
        assert_eq!(tree.child(false).unwrap().node().sum(), 3);
        assert!(tree.child(true).is_none());
    }

    #[should_panic]
    #[test]
    fn attach_existing() {
        Tree::new(SumNode::new(1))
            .attach(true, Some(Tree::new(SumNode::new(1))))
            .attach(true, Some(Tree::new(SumNode::new(1))));
    }

    #[test]
    fn detach() {
        let tree = Tree::new(SumNode::new(1))
            .attach(true, Some(Tree::new(SumNode::new(1))))
            .attach(false, Some(Tree::new(SumNode::new(1))));

        let (tree, left_opt) = tree.detach(true);
        assert_eq!(tree.node().sum(), 3);
        assert!(tree.child(true).is_none());
        assert!(tree.child(false).is_some());
        assert_eq!(left_opt.as_ref().unwrap().node().sum(), 1);

        let (tree, right) = tree.detach_expect(false);
        assert_eq!(tree.node().sum(), 3);
        assert!(tree.child(true).is_none());
        assert!(tree.child(false).is_none());
        assert_eq!(right.node().sum(), 1);

        let tree = tree
            .attach(true, left_opt)
            .attach(false, Some(right));
        assert_eq!(tree.node().sum(), 3);
        assert!(tree.child(true).is_some());
        assert!(tree.child(false).is_some());
    }
}
