use super::super::{Link, Tree};
use super::Fetch;
use crate::error::{Error::ModifiedLinkTraversal, Result};

/// Allows read-only traversal of a `Tree`, fetching from the given source when
/// traversing to a pruned node.
///
/// The fetched nodes are in memory until they (possibly) get pruned on the next
/// commit.
///
/// Only finalized trees may be walked (trees which have had `commit` called
/// since the last update).
pub struct RefWalker<'a, S>
where
    S: Fetch + Sized + Clone + Send,
{
    tree: &'a mut Tree,
    source: S,
}

impl<'a, S> RefWalker<'a, S>
where
    S: Fetch + Sized + Clone + Send,
{
    /// Creates a `RefWalker` with the given tree and source.
    pub fn new(tree: &'a mut Tree, source: S) -> Self {
        RefWalker { tree, source }
    }

    /// Gets an immutable reference to the `Tree` wrapped by this `RefWalker`.
    pub fn tree(&self) -> &Tree {
        self.tree
    }

    /// Traverses to the child on the given side (if any), fetching from the
    /// source if pruned. When fetching, the link is upgraded from
    /// `Link::Reference` to `Link::Loaded`.
    pub fn walk(&mut self, left: bool) -> Result<Option<RefWalker<S>>> {
        let link = match self.tree.link(left) {
            None => return Ok(None),
            Some(link) => link,
        };

        match link {
            Link::Reference { .. } => self.tree.load(left, &self.source),
            Link::Modified { .. } => Err(ModifiedLinkTraversal),
            Link::Uncommitted { .. } | Link::Loaded { .. } => Ok(()),
        }?;

        let child = self.tree.child_mut(left).unwrap();
        Ok(Some(RefWalker::new(child, self.source.clone())))
    }
}
