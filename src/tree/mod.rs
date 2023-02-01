mod commit;
#[cfg(feature = "full")]
mod debug;
mod encoding;
mod fuzz_tests;
mod hash;
mod iter;
mod kv;
mod link;
mod ops;
mod walk;

use std::cmp::max;

use ed::{Decode, Encode};

use super::error::Result;
pub use commit::{Commit, NoopCommit};
pub use hash::{kv_hash, node_hash, Hash, Hasher, HASH_LENGTH, NULL_HASH};
use kv::KV;
pub use link::Link;
pub use ops::{Batch, BatchEntry, Op, PanicSource};
pub use walk::{Fetch, RefWalker, Walker};

// TODO: remove need for `TreeInner`, and just use `Box<Self>` receiver for
// relevant methods

/// The fields of the `Tree` type, stored on the heap.
#[derive(Encode, Decode)]
pub struct TreeInner {
    left: Option<Link>,
    right: Option<Link>,
    kv: KV,
}

/// A binary AVL tree data structure, with Merkle hashes.
///
/// Trees' inner fields are stored on the heap so that nodes can recursively
/// link to each other, and so we can detach nodes from their parents, then
/// reattach without allocating or freeing heap memory.
#[derive(Encode, Decode)]
pub struct Tree {
    inner: Box<TreeInner>,
}

impl Tree {
    /// Creates a new `Tree` with the given key and value, and no children.
    ///
    /// Hashes the key/value pair and initializes the `kv_hash` field.
    pub fn new(key: Vec<u8>, value: Vec<u8>) -> Result<Self> {
        KV::new(key, value).map_err(Into::into).map(|kv| Tree {
            inner: Box::new(TreeInner {
                kv,
                left: None,
                right: None,
            }),
        })
    }

    /// Creates a `Tree` by supplying all the raw struct fields (mainly useful
    /// for testing). The `kv_hash` and `Link`s are not ensured to be correct.
    pub fn from_fields(
        key: Vec<u8>,
        value: Vec<u8>,
        kv_hash: Hash,
        left: Option<Link>,
        right: Option<Link>,
    ) -> Tree {
        Tree {
            inner: Box::new(TreeInner {
                kv: KV::from_fields(key, value, kv_hash),
                left,
                right,
            }),
        }
    }

    /// Returns the root node's key as a slice.
    #[inline]
    pub fn key(&self) -> &[u8] {
        self.inner.kv.key()
    }

    /// Consumes the tree and returns its root node's key, without having to
    /// clone or allocate.
    #[inline]
    pub fn take_key(self) -> Vec<u8> {
        self.inner.kv.take_key()
    }

    /// Returns the root node's value as a slice.
    #[inline]
    pub fn value(&self) -> &[u8] {
        self.inner.kv.value()
    }

    /// Returns the hash of the root node's key/value pair.
    #[inline]
    pub fn kv_hash(&self) -> &Hash {
        self.inner.kv.hash()
    }

    /// Returns a reference to the root node's `Link` on the given side, if any.
    /// If there is no child, returns `None`.
    #[inline]
    pub fn link(&self, left: bool) -> Option<&Link> {
        if left {
            self.inner.left.as_ref()
        } else {
            self.inner.right.as_ref()
        }
    }

    /// Returns a mutable reference to the root node's `Link` on the given side,
    /// if any. If there is no child, returns `None`.
    #[inline]
    pub fn link_mut(&mut self, left: bool) -> Option<&mut Link> {
        if left {
            self.inner.left.as_mut()
        } else {
            self.inner.right.as_mut()
        }
    }

    /// Returns a reference to the root node's child on the given side, if any.
    /// If there is no child, returns `None`.
    #[inline]
    pub fn child(&self, left: bool) -> Option<&Self> {
        match self.link(left) {
            None => None,
            Some(link) => link.tree(),
        }
    }

    /// Returns a mutable reference to the root node's child on the given side,
    /// if any. If there is no child, returns `None`.
    #[inline]
    pub fn child_mut(&mut self, left: bool) -> Option<&mut Self> {
        match self.slot_mut(left).as_mut() {
            None => None,
            Some(Link::Reference { .. }) => None,
            Some(Link::Modified { tree, .. }) => Some(tree),
            Some(Link::Uncommitted { tree, .. }) => Some(tree),
            Some(Link::Loaded { tree, .. }) => Some(tree),
        }
    }

    /// Returns the hash of the root node's child on the given side, if any. If
    /// there is no child, returns the null hash (zero-filled).
    #[inline]
    pub fn child_hash(&self, left: bool) -> &Hash {
        self.link(left).map_or(&NULL_HASH, |link| link.hash())
    }

    /// Computes and returns the hash of the root node.
    #[inline]
    pub fn hash(&self) -> Hash {
        node_hash::<Hasher>(
            self.inner.kv.hash(),
            self.child_hash(true),
            self.child_hash(false),
        )
    }

    /// Returns the number of pending writes for the child on the given side, if
    /// any. If there is no child, returns 0.
    #[inline]
    pub fn child_pending_writes(&self, left: bool) -> usize {
        match self.link(left) {
            Some(Link::Modified { pending_writes, .. }) => *pending_writes,
            _ => 0,
        }
    }

    /// Returns the height of the child on the given side, if any. If there is
    /// no child, returns 0.
    #[inline]
    pub fn child_height(&self, left: bool) -> u8 {
        self.link(left).map_or(0, |child| child.height())
    }

    #[inline]
    pub fn child_heights(&self) -> (u8, u8) {
        (self.child_height(true), self.child_height(false))
    }

    /// Returns the height of the tree (the number of levels). For example, a
    /// single node has height 1, a node with a single descendant has height 2,
    /// etc.
    #[inline]
    pub fn height(&self) -> u8 {
        1 + max(self.child_height(true), self.child_height(false))
    }

    /// Returns the balance factor of the root node. This is the difference
    /// between the height of the right child (if any) and the height of the
    /// left child (if any). For example, a balance factor of 2 means the right
    /// subtree is 2 levels taller than the left subtree.
    #[inline]
    pub fn balance_factor(&self) -> i8 {
        let left_height = self.child_height(true) as i8;
        let right_height = self.child_height(false) as i8;
        right_height - left_height
    }

    /// Attaches the child (if any) to the root node on the given side. Creates
    /// a `Link` of variant `Link::Modified` which contains the child.
    ///
    /// Panics if there is already a child on the given side.
    #[inline]
    pub fn attach(mut self, left: bool, maybe_child: Option<Self>) -> Self {
        debug_assert_ne!(
            Some(self.key()),
            maybe_child.as_ref().map(|c| c.key()),
            "Tried to attach tree with same key"
        );

        let slot = self.slot_mut(left);

        assert!(
            !slot.is_some(),
            "Tried to attach to {} tree slot, but it is already Some",
            side_to_str(left)
        );

        *slot = Link::maybe_from_modified_tree(maybe_child);

        self
    }

    /// Detaches the child on the given side (if any) from the root node, and
    /// returns `(root_node, maybe_child)`.
    ///
    /// One will usually want to reattach (see `attach`) a child on the same
    /// side after applying some operation to the detached child.
    #[inline]
    pub fn detach(mut self, left: bool) -> (Self, Option<Self>) {
        let maybe_child = match self.slot_mut(left).take() {
            None => None,
            Some(Link::Reference { .. }) => None,
            Some(Link::Modified { tree, .. }) => Some(tree),
            Some(Link::Uncommitted { tree, .. }) => Some(tree),
            Some(Link::Loaded { tree, .. }) => Some(tree),
        };

        (self, maybe_child)
    }

    /// Detaches the child on the given side from the root node, and
    /// returns `(root_node, child)`.
    ///
    /// Panics if there is no child on the given side.
    ///
    /// One will usually want to reattach (see `attach`) a child on the same
    /// side after applying some operation to the detached child.
    #[inline]
    pub fn detach_expect(self, left: bool) -> (Self, Self) {
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

    /// Detaches the child on the given side and passes it into `f`, which must
    /// return a new child (either the same child, a new child to take its
    /// place, or `None` to explicitly keep the slot empty).
    ///
    /// This is the same as `detach`, but with the function interface to enforce
    /// at compile-time that an explicit final child value is returned. This is
    /// less error prone that detaching with `detach` and reattaching with
    /// `attach`.
    #[inline]
    pub fn walk<F>(self, left: bool, f: F) -> Self
    where
        F: FnOnce(Option<Self>) -> Option<Self>,
    {
        let (tree, maybe_child) = self.detach(left);
        tree.attach(left, f(maybe_child))
    }

    /// Like `walk`, but panics if there is no child on the given side.
    #[inline]
    pub fn walk_expect<F>(self, left: bool, f: F) -> Self
    where
        F: FnOnce(Self) -> Option<Self>,
    {
        let (tree, child) = self.detach_expect(left);
        tree.attach(left, f(child))
    }

    /// Returns a mutable reference to the child slot for the given side.
    #[inline]
    pub(crate) fn slot_mut(&mut self, left: bool) -> &mut Option<Link> {
        if left {
            &mut self.inner.left
        } else {
            &mut self.inner.right
        }
    }

    /// Replaces the root node's value with the given value and returns the
    /// modified `Tree`.
    #[inline]
    pub fn with_value(mut self, value: Vec<u8>) -> Result<Self> {
        self.inner.kv = self.inner.kv.with_value(value)?;
        Ok(self)
    }

    // TODO: add compute_hashes method

    /// Called to finalize modifications to a tree, recompute its hashes, and
    /// write the updated nodes to a backing store.
    ///
    /// Traverses through the tree, computing hashes for all modified links and
    /// replacing them with `Link::Loaded` variants, writes out all changes to
    /// the given `Commit` object's `write` method, and calls the its `prune`
    /// method to test whether or not to keep or prune nodes from memory.
    #[inline]
    pub fn commit<C: Commit>(&mut self, c: &mut C) -> Result<()> {
        // TODO: make this method less ugly
        // TODO: call write in-order for better performance in writing batch to db?

        if let Some(Link::Modified { .. }) = self.inner.left {
            if let Some(Link::Modified {
                mut tree,
                child_heights,
                ..
            }) = self.inner.left.take()
            {
                tree.commit(c)?;
                self.inner.left = Some(Link::Loaded {
                    hash: tree.hash(),
                    tree,
                    child_heights,
                });
            } else {
                unreachable!()
            }
        }

        if let Some(Link::Modified { .. }) = self.inner.right {
            if let Some(Link::Modified {
                mut tree,
                child_heights,
                ..
            }) = self.inner.right.take()
            {
                tree.commit(c)?;
                self.inner.right = Some(Link::Loaded {
                    hash: tree.hash(),
                    tree,
                    child_heights,
                });
            } else {
                unreachable!()
            }
        }

        c.write(self)?;

        let (prune_left, prune_right) = c.prune(self);
        if prune_left {
            self.inner.left = self.inner.left.take().map(|link| link.into_reference());
        }
        if prune_right {
            self.inner.right = self.inner.right.take().map(|link| link.into_reference());
        }

        Ok(())
    }

    /// Fetches the child on the given side using the given data source, and
    /// places it in the child slot (upgrading the link from `Link::Reference` to
    /// `Link::Loaded`).
    #[inline]
    pub fn load<S: Fetch>(&mut self, left: bool, source: &S) -> Result<()> {
        // TODO: return Err instead of panic?
        let link = self.link(left).expect("Expected link");
        let (child_heights, hash) = match link {
            Link::Reference {
                child_heights,
                hash,
                ..
            } => (child_heights, hash),
            _ => panic!("Expected Some(Link::Reference)"),
        };

        let tree = source.fetch(link)?;
        debug_assert_eq!(tree.key(), link.key());
        *self.slot_mut(left) = Some(Link::Loaded {
            tree,
            hash: *hash,
            child_heights: *child_heights,
        });

        Ok(())
    }

    pub fn get_value(&self, key: &[u8]) -> Result<GetResult> {
        let mut cursor = self;

        loop {
            if key == cursor.key() {
                return Ok(GetResult::Found(cursor.value().to_vec()));
            }

            let left = key < cursor.key();
            let link = match cursor.link(left) {
                None => return Ok(GetResult::NotFound), // not found
                Some(link) => link,
            };

            let maybe_child = link.tree();
            match maybe_child {
                None => return Ok(GetResult::Pruned), // value is pruned, caller will have to fetch from disk
                Some(child) => cursor = child,        // traverse to child
            }
        }
    }
}

pub enum GetResult {
    Found(Vec<u8>),
    Pruned,
    NotFound,
}

pub fn side_to_str(left: bool) -> &'static str {
    if left {
        "left"
    } else {
        "right"
    }
}

#[cfg(test)]
mod test {
    use super::commit::NoopCommit;
    use super::hash::NULL_HASH;
    use super::Tree;
    use crate::error::Result;

    #[test]
    fn build_tree() -> Result<()> {
        let tree = Tree::new(vec![1], vec![101])?;
        assert_eq!(tree.key(), &[1]);
        assert_eq!(tree.value(), &[101]);
        assert!(tree.child(true).is_none());
        assert!(tree.child(false).is_none());

        let tree = tree.attach(true, None);
        assert!(tree.child(true).is_none());
        assert!(tree.child(false).is_none());

        let tree = tree.attach(true, Some(Tree::new(vec![2], vec![102])?));
        assert_eq!(tree.key(), &[1]);
        assert_eq!(tree.child(true).unwrap().key(), &[2]);
        assert!(tree.child(false).is_none());

        let tree = Tree::new(vec![3], vec![103])?.attach(false, Some(tree));
        assert_eq!(tree.key(), &[3]);
        assert_eq!(tree.child(false).unwrap().key(), &[1]);
        assert!(tree.child(true).is_none());
        Ok(())
    }

    #[should_panic]
    #[test]
    fn attach_existing() {
        Tree::new(vec![0], vec![1])
            .expect("tree construction failed")
            .attach(
                true,
                Some(Tree::new(vec![2], vec![3]).expect("tree construction failed")),
            )
            .attach(
                true,
                Some(Tree::new(vec![4], vec![5]).expect("tree construction failed")),
            );
    }

    #[test]
    fn modify() -> Result<()> {
        let tree = Tree::new(vec![0], vec![1])?
            .attach(true, Some(Tree::new(vec![2], vec![3])?))
            .attach(false, Some(Tree::new(vec![4], vec![5])?));

        let tree = tree.walk(true, |left_opt| {
            assert_eq!(left_opt.as_ref().unwrap().key(), &[2]);
            None
        });
        assert!(tree.child(true).is_none());
        assert!(tree.child(false).is_some());
        let fixed_tree = Some(Tree::new(vec![2], vec![3])?);
        let tree = tree.walk(true, |left_opt| {
            assert!(left_opt.is_none());
            fixed_tree
        });
        assert_eq!(tree.link(true).unwrap().key(), &[2]);

        let tree = tree.walk_expect(false, |right| {
            assert_eq!(right.key(), &[4]);
            None
        });
        assert!(tree.child(true).is_some());
        assert!(tree.child(false).is_none());
        Ok(())
    }

    #[test]
    fn child_and_link() -> Result<()> {
        let mut tree =
            Tree::new(vec![0], vec![1])?.attach(true, Some(Tree::new(vec![2], vec![3])?));
        assert!(tree.link(true).expect("expected link").is_modified());
        assert!(tree.child(true).is_some());
        assert!(tree.link(false).is_none());
        assert!(tree.child(false).is_none());

        tree.commit(&mut NoopCommit {}).expect("commit failed");
        assert!(tree.link(true).expect("expected link").is_stored());
        assert!(tree.child(true).is_some());

        let tree = tree.walk(true, |_| None);
        assert!(tree.link(true).is_none());
        assert!(tree.child(true).is_none());
        Ok(())
    }

    #[test]
    fn child_hash() -> Result<()> {
        let mut tree =
            Tree::new(vec![0], vec![1])?.attach(true, Some(Tree::new(vec![2], vec![3])?));
        tree.commit(&mut NoopCommit {}).expect("commit failed");
        assert_eq!(
            tree.child_hash(true),
            &[
                114, 93, 76, 193, 9, 87, 168, 251, 191, 152, 173, 130, 33, 46, 251, 13, 179, 15,
                209, 218, 113, 72, 118, 83, 206, 100, 36, 49, 156, 239, 102, 205
            ]
        );
        assert_eq!(tree.child_hash(false), &NULL_HASH);
        Ok(())
    }

    #[test]
    fn hash() -> Result<()> {
        let tree = Tree::new(vec![0], vec![1])?;
        assert_eq!(
            tree.hash(),
            [
                36, 100, 183, 19, 15, 225, 83, 239, 215, 167, 216, 53, 52, 30, 234, 176, 74, 197,
                161, 7, 102, 226, 181, 251, 145, 100, 74, 179, 253, 222, 183, 117
            ]
        );
        Ok(())
    }

    #[test]
    fn child_pending_writes() -> Result<()> {
        let tree = Tree::new(vec![0], vec![1])?;
        assert_eq!(tree.child_pending_writes(true), 0);
        assert_eq!(tree.child_pending_writes(false), 0);

        let tree = tree.attach(true, Some(Tree::new(vec![2], vec![3])?));
        assert_eq!(tree.child_pending_writes(true), 1);
        assert_eq!(tree.child_pending_writes(false), 0);
        Ok(())
    }

    #[test]
    fn height_and_balance() -> Result<()> {
        let tree = Tree::new(vec![0], vec![1])?;
        assert_eq!(tree.height(), 1);
        assert_eq!(tree.child_height(true), 0);
        assert_eq!(tree.child_height(false), 0);
        assert_eq!(tree.balance_factor(), 0);

        let tree = tree.attach(true, Some(Tree::new(vec![2], vec![3])?));
        assert_eq!(tree.height(), 2);
        assert_eq!(tree.child_height(true), 1);
        assert_eq!(tree.child_height(false), 0);
        assert_eq!(tree.balance_factor(), -1);

        let (tree, maybe_child) = tree.detach(true);
        let tree = tree.attach(false, maybe_child);
        assert_eq!(tree.height(), 2);
        assert_eq!(tree.child_height(true), 0);
        assert_eq!(tree.child_height(false), 1);
        assert_eq!(tree.balance_factor(), 1);
        Ok(())
    }

    #[test]
    fn commit() -> Result<()> {
        let mut tree =
            Tree::new(vec![0], vec![1])?.attach(false, Some(Tree::new(vec![2], vec![3])?));
        tree.commit(&mut NoopCommit {}).expect("commit failed");

        assert!(tree.link(false).expect("expected link").is_stored());
        Ok(())
    }
}
