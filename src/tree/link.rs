use std::cmp::max;
use std::io::{Read, Write};

use ed::{Decode, Encode, Result, Terminated};

use super::hash::Hash;
use super::Tree;

// TODO: optimize memory footprint

/// Represents a reference to a child tree node. Links may or may not contain
/// the child's `Tree` instance (storing its key if not).
pub enum Link {
    /// Represents a child tree node which has been pruned from memory, only
    /// retaining a reference to it (its key). The child node can always be
    /// fetched from the backing store by this key when necessary.
    Reference {
        hash: Hash,
        child_heights: (u8, u8),
        key: Vec<u8>,
    },

    /// Represents a tree node which has been modified since the `Tree`'s last
    /// hash computation. The child's hash is not stored since it has not yet
    /// been recomputed. The child's `Tree` instance is stored in the link.
    #[rustfmt::skip]
    Modified {
        pending_writes: usize, // TODO: rename to `pending_hashes`
        child_heights: (u8, u8),
        tree: Tree
    },

    // Represents a tree node which has been modified since the `Tree`'s last
    // commit, but which has an up-to-date hash. The child's `Tree` instance is
    // stored in the link.
    Uncommitted {
        hash: Hash,
        child_heights: (u8, u8),
        tree: Tree,
    },

    /// Represents a tree node which has not been modified, has an up-to-date
    /// hash, and which is being retained in memory.
    Loaded {
        hash: Hash,
        child_heights: (u8, u8),
        tree: Tree,
    },
}

impl Link {
    /// Creates a `Link::Modified` from the given `Tree`.
    #[inline]
    pub fn from_modified_tree(tree: Tree) -> Self {
        let pending_writes = 1 + tree.child_pending_writes(true) + tree.child_pending_writes(false);

        Link::Modified {
            pending_writes,
            child_heights: tree.child_heights(),
            tree,
        }
    }

    /// Creates a `Link::Modified` from the given tree, if any. If `None`,
    /// returns `None`.
    pub fn maybe_from_modified_tree(maybe_tree: Option<Tree>) -> Option<Self> {
        maybe_tree.map(Link::from_modified_tree)
    }

    /// Returns `true` if the link is of the `Link::Reference` variant.
    #[inline]
    pub fn is_reference(&self) -> bool {
        matches!(self, Link::Reference { .. })
    }

    /// Returns `true` if the link is of the `Link::Modified` variant.
    #[inline]
    pub fn is_modified(&self) -> bool {
        matches!(self, Link::Modified { .. })
    }

    /// Returns `true` if the link is of the `Link::Uncommitted` variant.
    #[inline]
    pub fn is_uncommitted(&self) -> bool {
        matches!(self, Link::Uncommitted { .. })
    }

    /// Returns `true` if the link is of the `Link::Loaded` variant.
    #[inline]
    pub fn is_stored(&self) -> bool {
        matches!(self, Link::Loaded { .. })
    }

    /// Returns the key of the tree referenced by this link, as a slice.
    #[inline]
    pub fn key(&self) -> &[u8] {
        match self {
            Link::Reference { key, .. } => key.as_slice(),
            Link::Modified { tree, .. } => tree.key(),
            Link::Uncommitted { tree, .. } => tree.key(),
            Link::Loaded { tree, .. } => tree.key(),
        }
    }

    /// Returns the `Tree` instance of the tree referenced by the link. If the
    /// link is of variant `Link::Reference`, the returned value will be `None`.
    #[inline]
    pub fn tree(&self) -> Option<&Tree> {
        match self {
            // TODO: panic for Reference, don't return Option?
            Link::Reference { .. } => None,
            Link::Modified { tree, .. } => Some(tree),
            Link::Uncommitted { tree, .. } => Some(tree),
            Link::Loaded { tree, .. } => Some(tree),
        }
    }

    /// Returns the hash of the tree referenced by the link. Panics if link is
    /// of variant `Link::Modified` since we have not yet recomputed the tree's
    /// hash.
    #[inline]
    pub fn hash(&self) -> &Hash {
        match self {
            Link::Modified { .. } => panic!("Cannot get hash from modified link"),
            Link::Reference { hash, .. } => hash,
            Link::Uncommitted { hash, .. } => hash,
            Link::Loaded { hash, .. } => hash,
        }
    }

    /// Returns the height of the children of the tree referenced by the link,
    /// if any (note: not the height of the referenced tree itself). Return
    /// value is `(left_child_height, right_child_height)`.
    #[inline]
    pub fn height(&self) -> u8 {
        let (left_height, right_height) = match self {
            Link::Reference { child_heights, .. } => *child_heights,
            Link::Modified { child_heights, .. } => *child_heights,
            Link::Uncommitted { child_heights, .. } => *child_heights,
            Link::Loaded { child_heights, .. } => *child_heights,
        };
        1 + max(left_height, right_height)
    }

    /// Returns the balance factor of the tree referenced by the link.
    #[inline]
    pub fn balance_factor(&self) -> i8 {
        let (left_height, right_height) = match self {
            Link::Reference { child_heights, .. } => *child_heights,
            Link::Modified { child_heights, .. } => *child_heights,
            Link::Uncommitted { child_heights, .. } => *child_heights,
            Link::Loaded { child_heights, .. } => *child_heights,
        };
        right_height as i8 - left_height as i8
    }

    /// Consumes the link and converts to variant `Link::Reference`. Panics if the
    /// link is of variant `Link::Modified` or `Link::Uncommitted`.
    #[inline]
    pub fn into_reference(self) -> Self {
        match self {
            Link::Reference { .. } => self,
            Link::Modified { .. } => panic!("Cannot prune Modified tree"),
            Link::Uncommitted { .. } => panic!("Cannot prune Uncommitted tree"),
            Link::Loaded {
                hash,
                child_heights,
                tree,
            } => Link::Reference {
                hash,
                child_heights,
                key: tree.take_key(),
            },
        }
    }

    #[inline]
    #[cfg(feature = "full")]
    pub(crate) fn child_heights_mut(&mut self) -> &mut (u8, u8) {
        match self {
            Link::Reference {
                ref mut child_heights,
                ..
            } => child_heights,
            Link::Modified {
                ref mut child_heights,
                ..
            } => child_heights,
            Link::Uncommitted {
                ref mut child_heights,
                ..
            } => child_heights,
            Link::Loaded {
                ref mut child_heights,
                ..
            } => child_heights,
        }
    }
}

impl Encode for Link {
    #[inline]
    fn encode_into<W: Write>(&self, out: &mut W) -> Result<()> {
        let (hash, key, (left_height, right_height)) = match self {
            Link::Reference {
                hash,
                key,
                child_heights,
            } => (hash, key.as_slice(), child_heights),
            Link::Loaded {
                hash,
                tree,
                child_heights,
            } => (hash, tree.key(), child_heights),
            Link::Uncommitted {
                hash,
                tree,
                child_heights,
            } => (hash, tree.key(), child_heights),

            Link::Modified { .. } => panic!("No encoding for Link::Modified"),
        };

        debug_assert!(key.len() < 256, "Key length must be less than 256");

        out.write_all(&[key.len() as u8])?;
        out.write_all(key)?;

        out.write_all(hash)?;

        out.write_all(&[*left_height, *right_height])?;

        Ok(())
    }

    #[inline]
    fn encoding_length(&self) -> Result<usize> {
        debug_assert!(self.key().len() < 256, "Key length must be less than 256");

        Ok(match self {
            Link::Reference { key, .. } => 1 + key.len() + 32 + 2,
            Link::Modified { .. } => panic!("No encoding for Link::Modified"),
            Link::Uncommitted { tree, .. } => 1 + tree.key().len() + 32 + 2,
            Link::Loaded { tree, .. } => 1 + tree.key().len() + 32 + 2,
        })
    }
}

impl Link {
    #[inline]
    fn default_reference() -> Self {
        Link::Reference {
            key: Vec::with_capacity(64),
            hash: Default::default(),
            child_heights: (0, 0),
        }
    }
}

impl Decode for Link {
    #[inline]
    fn decode<R: Read>(input: R) -> Result<Link> {
        let mut link = Link::default_reference();
        Link::decode_into(&mut link, input)?;
        Ok(link)
    }

    #[inline]
    fn decode_into<R: Read>(&mut self, mut input: R) -> Result<()> {
        if !self.is_reference() {
            // don't create new struct if self is already Link::Reference,
            // so we can re-use the key vec
            *self = Link::default_reference();
        }

        if let Link::Reference {
            ref mut key,
            ref mut hash,
            ref mut child_heights,
        } = self
        {
            let length = read_u8(&mut input)? as usize;

            key.resize(length, 0);
            input.read_exact(key.as_mut())?;

            input.read_exact(&mut hash[..])?;

            child_heights.0 = read_u8(&mut input)?;
            child_heights.1 = read_u8(&mut input)?;
        } else {
            unreachable!()
        }

        Ok(())
    }
}

impl Terminated for Link {}

#[inline]
fn read_u8<R: Read>(mut input: R) -> Result<u8> {
    let mut length = [0];
    input.read_exact(length.as_mut())?;
    Ok(length[0])
}

#[cfg(test)]
mod test {
    use super::super::hash::NULL_HASH;
    use super::super::Tree;
    use super::*;

    #[test]
    fn from_modified_tree() {
        let tree = Tree::new(vec![0], vec![1]);
        let link = Link::from_modified_tree(tree);
        assert!(link.is_modified());
        assert_eq!(link.height(), 1);
        assert_eq!(link.tree().expect("expected tree").key(), &[0]);
        if let Link::Modified { pending_writes, .. } = link {
            assert_eq!(pending_writes, 1);
        } else {
            panic!("Expected Link::Modified");
        }
    }

    #[test]
    fn maybe_from_modified_tree() {
        let link = Link::maybe_from_modified_tree(None);
        assert!(link.is_none());

        let tree = Tree::new(vec![0], vec![1]);
        let link = Link::maybe_from_modified_tree(Some(tree));
        assert!(link.expect("expected link").is_modified());
    }

    #[test]
    fn types() {
        let hash = NULL_HASH;
        let child_heights = (0, 0);
        let pending_writes = 1;
        let key = vec![0];
        let tree = || Tree::new(vec![0], vec![1]);

        let reference = Link::Reference {
            hash,
            child_heights,
            key,
        };
        let modified = Link::Modified {
            pending_writes,
            child_heights,
            tree: tree(),
        };
        let uncommitted = Link::Uncommitted {
            hash,
            child_heights,
            tree: tree(),
        };
        let loaded = Link::Loaded {
            hash,
            child_heights,
            tree: tree(),
        };

        assert!(reference.is_reference());
        assert!(!reference.is_modified());
        assert!(!reference.is_uncommitted());
        assert!(!reference.is_stored());
        assert!(reference.tree().is_none());
        assert_eq!(reference.hash(), &[0; 32]);
        assert_eq!(reference.height(), 1);
        assert!(reference.into_reference().is_reference());

        assert!(!modified.is_reference());
        assert!(modified.is_modified());
        assert!(!modified.is_uncommitted());
        assert!(!modified.is_stored());
        assert!(modified.tree().is_some());
        assert_eq!(modified.height(), 1);

        assert!(!uncommitted.is_reference());
        assert!(!uncommitted.is_modified());
        assert!(uncommitted.is_uncommitted());
        assert!(!uncommitted.is_stored());
        assert!(uncommitted.tree().is_some());
        assert_eq!(uncommitted.hash(), &[0; 32]);
        assert_eq!(uncommitted.height(), 1);

        assert!(!loaded.is_reference());
        assert!(!loaded.is_modified());
        assert!(!loaded.is_uncommitted());
        assert!(loaded.is_stored());
        assert!(loaded.tree().is_some());
        assert_eq!(loaded.hash(), &[0; 32]);
        assert_eq!(loaded.height(), 1);
        assert!(loaded.into_reference().is_reference());
    }

    #[test]
    #[should_panic]
    fn modified_hash() {
        Link::Modified {
            pending_writes: 1,
            child_heights: (1, 1),
            tree: Tree::new(vec![0], vec![1]),
        }
        .hash();
    }

    #[test]
    #[should_panic]
    fn modified_into_reference() {
        Link::Modified {
            pending_writes: 1,
            child_heights: (1, 1),
            tree: Tree::new(vec![0], vec![1]),
        }
        .into_reference();
    }

    #[test]
    #[should_panic]
    fn uncommitted_into_reference() {
        Link::Uncommitted {
            hash: [1; 32],
            child_heights: (1, 1),
            tree: Tree::new(vec![0], vec![1]),
        }
        .into_reference();
    }

    #[test]
    fn encode_link() {
        let link = Link::Reference {
            key: vec![1, 2, 3],
            child_heights: (123, 124),
            hash: [55; 32],
        };
        assert_eq!(link.encoding_length().unwrap(), 38);

        let mut bytes = vec![];
        link.encode_into(&mut bytes).unwrap();
        assert_eq!(
            bytes,
            vec![
                3, 1, 2, 3, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55,
                55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 123, 124
            ]
        );
    }

    #[test]
    #[should_panic]
    fn encode_link_long_key() {
        let link = Link::Reference {
            key: vec![123; 300],
            child_heights: (123, 124),
            hash: [55; 32],
        };
        let mut bytes = vec![];
        link.encode_into(&mut bytes).unwrap();
    }
}
