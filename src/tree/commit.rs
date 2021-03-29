use super::Tree;
use crate::error::Result;

/// To be used when committing a tree (writing it to a store after applying the
/// changes).
pub trait Commit {
    /// Called once per updated node when a finalized tree is to be written to a
    /// backing store or cache.
    fn write(&mut self, tree: &Tree) -> Result<()>;

    /// Called once per node after writing a node and its children. The returned
    /// tuple specifies whether or not to prune the left and right child nodes,
    /// respectively. For example, returning `(true, true)` will prune both
    /// nodes, removing them from memory.
    fn prune(&self, _tree: &Tree) -> (bool, bool) {
        (true, true)
    }
}

/// A `Commit` implementation which does not write to a store and does not prune
/// any nodes from the Tree. Useful when only keeping a tree in memory.
pub struct NoopCommit {}
impl Commit for NoopCommit {
    fn write(&mut self, _tree: &Tree) -> Result<()> {
        Ok(())
    }

    fn prune(&self, _tree: &Tree) -> (bool, bool) {
        (false, false)
    }
}
