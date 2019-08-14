mod walk;
mod hash;
mod kv;
mod link;
mod encoding;
mod commit;
mod debug;

use std::cmp::max;

pub use walk::{Walker, Fetch};
use super::error::Result;
pub use commit::{Commit, NoopCommit};
use kv::KV;
pub use link::Link;
use hash::{Hash, node_hash, NULL_HASH};
use super::owner::Owner;

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

    pub fn from_fields(
        key: Vec<u8>,
        value: Vec<u8>,
        kv_hash: Hash,
        left: Option<Link>,
        right: Option<Link>
    ) -> Tree {
        Tree {
            inner: Box::new(TreeInner {
                kv: KV::from_fields(key, value, kv_hash),
                left,
                right
            })
        }
    }

    #[inline]
    pub fn key(&self) -> &[u8] {
        self.inner.kv.key()
    }

    #[inline]
    pub fn take_key(self) -> Vec<u8> {
        self.inner.kv.take_key()
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

    #[inline]
    pub fn child_heights(&self) -> (u8, u8) {
        (
            self.child_height(true),
            self.child_height(false)
        )
    }

    #[inline]
    pub fn height(&self) -> u8 {
        1 + max(
            self.child_height(true),
            self.child_height(false)
        )
    }

    #[inline]
    pub fn balance_factor(&self) -> i8 {
        let left_height = self.child_height(true) as i8;
        let right_height = self.child_height(false) as i8;
        right_height - left_height
    }

    pub fn attach(mut self, left: bool, maybe_child: Option<Self>) -> Self {
        debug_assert_ne!(
            Some(self.key()),
            maybe_child.as_ref().map(|c| c.key()),
            "Tried to attach tree with same key"
        );

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

    pub unsafe fn detach(mut self, left: bool) -> (Self, Option<Self>) {
        let maybe_child = match self.slot_mut(left).take() {
            None => None,
            Some(Link::Pruned { .. }) => None,
            Some(Link::Modified { tree, .. }) => Some(tree),
            Some(Link::Stored { tree, .. }) => Some(tree)
        };

        (self, maybe_child)
    }

    pub unsafe fn detach_expect(mut self, left: bool) -> (Self, Self) {
        let (parent, maybe_child) = self.detach(left);

        if let Some(child) = maybe_child {
            (parent, child)
        } else {
            panic!(
                "Expected tree to have {} child, but got None",
                side_to_str(left)
            );
        }
    }

    pub fn walk<F>(mut self, left: bool, f: F) -> Self
        where F: FnOnce(Option<Self>) -> Option<Self>
    {
        let (tree, maybe_child) = unsafe { self.detach(left) };
        tree.attach(left, f(maybe_child))
    }

    pub fn walk_expect<F>(mut self, left: bool, f: F) -> Self
        where F: FnOnce(Self) -> Option<Self>
    {
        let (tree, child) = unsafe { self.detach_expect(left) };
        tree.attach(left, f(child))
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

    pub fn commit<C: Commit>(&mut self, c: &mut C) -> Result<()> {
        // TODO: make this method less ugly
        // TODO: call write in-order for better performance in writing batch to db?

        if let Some(Link::Modified { .. }) = self.inner.left {
            if let Some(Link::Modified { mut tree, child_heights, .. }) = self.inner.left.take() {
                tree.commit(c)?;
                self.inner.left = Some(Link::Stored {
                    hash: tree.hash(),
                    tree,
                    child_heights
                });
            }
        }

        if let Some(Link::Modified { .. }) = self.inner.right {
            if let Some(Link::Modified { mut tree, child_heights, .. }) = self.inner.right.take() {
                tree.commit(c)?;
                self.inner.right = Some(Link::Stored {
                    hash: tree.hash(),
                    tree,
                    child_heights
                });
            }
        }

        c.write(&self)?;

        let (prune_left, prune_right) = c.prune(&self);
        if prune_left {
            self.inner.left = self.inner.left.take()
                .map(|link| link.to_pruned());
        }
        if prune_right {
            self.inner.right = self.inner.right.take()
                .map(|link| link.to_pruned());
        }

        Ok(())
    }
}

pub fn side_to_str(left: bool) -> &'static str {
    if left { "left" } else { "right" }
}

#[cfg(test)]
mod test {
    use super::Tree;
    use super::hash::NULL_HASH;
    use super::commit::NoopCommit;

    #[test]
    fn build_tree() {
        let tree = Tree::new(vec![1], vec![101]);
        assert_eq!(tree.key(), &[1]);
        assert_eq!(tree.value(), &[101]);
        assert!(tree.child(true).is_none());
        assert!(tree.child(false).is_none());

        let tree = tree.attach(true, None);
        assert!(tree.child(true).is_none());
        assert!(tree.child(false).is_none());

        let tree = tree.attach(
            true,
            Some(Tree::new(vec![2], vec![102]))
        );
        assert_eq!(tree.key(), &[1]);
        assert_eq!(tree.child(true).unwrap().key(), &[2]);
        assert!(tree.child(false).is_none());

        let tree = Tree::new(vec![3], vec![103])
            .attach(false, Some(tree));
        assert_eq!(tree.key(), &[3]);
        assert_eq!(tree.child(false).unwrap().key(), &[1]);
        assert!(tree.child(true).is_none());
    }

    #[should_panic]
    #[test]
    fn attach_existing() {
        Tree::new(vec![0], vec![1])
            .attach(true, Some(Tree::new(vec![2], vec![3])))
            .attach(true, Some(Tree::new(vec![4], vec![5])));
    }

    #[test]
    fn modify() {
        let mut tree = Tree::new(vec![0], vec![1])
            .attach(true, Some(Tree::new(vec![2], vec![3])))
            .attach(false, Some(Tree::new(vec![4], vec![5])));

        let tree = tree.walk(true, |left_opt| {
            assert_eq!(left_opt.as_ref().unwrap().key(), &[2]);
            None
        });
        assert!(tree.child(true).is_none());
        assert!(tree.child(false).is_some());

        let tree = tree.walk(true, |left_opt| {
            assert!(left_opt.is_none());
            Some(Tree::new(vec![2], vec![3]))
        });
        assert_eq!(tree.link(true).unwrap().key(), &[2]);

        let tree = tree.walk_expect(false, |right| {
            assert_eq!(right.key(), &[4]);
            None
        });
        assert!(tree.child(true).is_some());
        assert!(tree.child(false).is_none());
    }

    #[test]
    fn child_and_link() {
        let mut tree = Tree::new(vec![0], vec![1])
            .attach(true, Some(Tree::new(vec![2], vec![3])));
        assert!(tree.link(true).expect("expected link").is_modified());
        assert!(tree.child(true).is_some());
        assert!(tree.link(false).is_none());
        assert!(tree.child(false).is_none());

        tree.commit(&mut NoopCommit {}).expect("commit failed");
        assert!(tree.link(true).expect("expected link").is_stored());
        assert!(tree.child(true).is_some());

        // tree.link(true).prune(true);
        // assert!(tree.link(true).expect("expected link").is_pruned());
        // assert!(tree.child(true).is_none());

        let tree = tree.walk(true, |_| None);
        assert!(tree.link(true).is_none());
        assert!(tree.child(true).is_none());
    }

    #[test]
    fn child_hash() {
        let mut tree = Tree::new(vec![0], vec![1])
            .attach(true, Some(Tree::new(vec![2], vec![3])));
        tree.commit(&mut NoopCommit {}).expect("commit failed");
        assert_eq!(tree.child_hash(true), &[23, 66, 77, 65, 141, 140, 245, 11, 53, 36, 157, 248, 208, 6, 160, 222, 213, 143, 249, 85]);
        assert_eq!(tree.child_hash(false), &NULL_HASH);
    }

    #[test]
    fn hash() {
        let tree = Tree::new(vec![0], vec![1]);
        assert_eq!(tree.hash(), [9, 242, 41, 142, 47, 227, 251, 242, 27, 29, 140, 24, 184, 111, 118, 188, 20, 58, 223, 197]);
    }

    #[test]
    fn child_pending_writes() {
        let tree = Tree::new(vec![0], vec![1]);
        assert_eq!(tree.child_pending_writes(true), 0);
        assert_eq!(tree.child_pending_writes(false), 0);

        let tree = tree.attach(true, Some(Tree::new(vec![2], vec![3])));
        assert_eq!(tree.child_pending_writes(true), 1);
        assert_eq!(tree.child_pending_writes(false), 0);
    }

    #[test]
    fn height_and_balance() {
        let tree = Tree::new(vec![0], vec![1]);
        assert_eq!(tree.height(), 1);
        assert_eq!(tree.child_height(true), 0);
        assert_eq!(tree.child_height(false), 0);
        assert_eq!(tree.balance_factor(), 0);

        let mut tree = tree.attach(true, Some(Tree::new(vec![2], vec![3])));
        assert_eq!(tree.height(), 2);
        assert_eq!(tree.child_height(true), 1);
        assert_eq!(tree.child_height(false), 0);
        assert_eq!(tree.balance_factor(), -1);

        let (tree, maybe_child) = unsafe { tree.detach(true) };
        let tree = tree.attach(false, maybe_child);
        assert_eq!(tree.height(), 2);
        assert_eq!(tree.child_height(true), 0);
        assert_eq!(tree.child_height(false), 1);
        assert_eq!(tree.balance_factor(), 1);
    }

    #[test]
    fn commit() {
        let mut tree = Tree::new(vec![0], vec![1])
            .attach(false, Some(Tree::new(vec![2], vec![3])));
        tree.commit(&mut NoopCommit {}).expect("commit failed");

        assert!(tree.link(false).expect("expected link").is_stored());
    }
}
