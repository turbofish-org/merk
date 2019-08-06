mod walk;
mod hash;
mod kv;
mod link;

use std::cmp::max;

pub use walk::{Walker, Fetch};
use kv::KV;
use link::Link;
use hash::{Hash, node_hash, NULL_HASH};

struct TreeInner {
    kv: KV,
    left: Option<Link>,
    right: Option<Link>
}

pub struct Tree {
    inner: Box<TreeInner>
}

impl Tree {
    pub fn new(key: Vec<u8>, value: Vec<u8>) -> Self {
        Tree {
            inner: Box::new(TreeInner {
                kv: KV::new(key, value),
                left: None,
                right: None
            })
        }
    }

    #[inline]
    pub fn key(&self) -> &[u8] {
        self.inner.kv.key()
    }

    #[inline]
    pub fn value(&self) -> &[u8] {
        self.inner.kv.value()
    }

    #[inline]
    pub fn link(&self, left: bool) -> Option<&Link> {
        if left {
            self.inner.left.as_ref()
        } else {
            self.inner.right.as_ref()
        }
    }

    pub fn child(&self, left: bool) -> Option<&Self> {
        match self.link(left) {
            None => None,
            Some(link) => link.tree()
        }
    }

    pub fn child_hash(&self, left: bool) -> &Hash {
        self.link(left)
            .map_or(&NULL_HASH, |link| link.hash())
    }

    pub fn hash(&self) -> Hash {
        node_hash(
            self.inner.kv.hash(),
            self.child_hash(true),
            self.child_hash(false)
        )
    }

    pub fn child_pending_writes(&self, left: bool) -> usize {
        match self.link(left) {
            Some(Link::Modified { pending_writes, .. }) => *pending_writes,
            _ => 0
        }
    }

    pub fn child_height(&self, left: bool) -> u8 {
        self.link(left)
            .map_or(0, |child| child.height())
    }

    pub fn height(&self) -> u8 {
        1 + max(
            self.child_height(true),
            self.child_height(false)
        )
    }

    pub fn balance_factor(&self) -> i8 {
        let left_height = self.child_height(true) as i8;
        let right_height = self.child_height(false) as i8;
        left_height - right_height
    }

    pub fn attach(mut self, left: bool, maybe_child: Option<Self>) -> Self {
        let slot = self.slot_mut(left);

        if slot.is_some() {
            panic!(
                "Tried to attach to {} tree slot, but it is already Some",
                side_to_str(left)
            );
        }

        *slot = Link::maybe_from_modified_tree(maybe_child);

        self
    }

    pub fn detach(&mut self, left: bool) -> Option<Self> {
        match self.slot_mut(left).take() {
            None => None,
            Some(Link::Pruned { .. }) => None,
            Some(Link::Modified { tree, .. }) => Some(tree),
            Some(Link::Stored { tree, .. }) => Some(tree)
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

    #[inline]
    fn slot_mut(&mut self, left: bool) -> &mut Option<Link> {
        if left {
            &mut self.inner.left
        } else {
            &mut self.inner.right
        }
    }

    #[inline]
    pub fn with_value(mut self, value: Vec<u8>) -> Self {
        self.inner.kv = self.inner.kv.with_value(value);
        self
    }
}

pub fn side_to_str(left: bool) -> &'static str {
    if left { "left" } else { "right" }
}

#[cfg(test)]
mod test {
    use super::Tree;

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
