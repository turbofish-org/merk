use super::Tree;
use crate::{error::Result, tree::Link};

/// To be used when committing a tree (writing it to a store after applying the
/// changes).
pub trait Commit {
    /// Called when a finalized tree is to be written to a backing store or
    /// cache.
    fn commit(&mut self, tree: &mut Tree) -> Result<()>;
}

/// A `Commit` implementation which does not write to a store and does not prune
/// any nodes from the Tree. Useful when only keeping a tree in memory.
pub struct NoopCommit {}
impl Commit for NoopCommit {
    fn commit(&mut self, tree: &mut Tree) -> Result<()> {
        tree.try_modify_link(true, |link| {
            if let Link::Modified {
                mut tree,
                child_heights,
                ..
            } = link
            {
                self.commit(&mut tree)?;
                return Ok(Link::Loaded {
                    hash: tree.hash(),
                    tree,
                    child_heights,
                });
            }

            Ok(link)
        })?;

        tree.try_modify_link(false, |link| {
            if let Link::Modified {
                mut tree,
                child_heights,
                ..
            } = link
            {
                self.commit(&mut tree)?;
                return Ok(Link::Loaded {
                    hash: tree.hash(),
                    tree,
                    child_heights,
                });
            }

            Ok(link)
        })?;

        Ok(())
    }
}
