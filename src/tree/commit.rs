use crate::error::Result;
use super::Tree;

pub trait Commit {
    fn write(&mut self, tree: &Tree) -> Result<()>;

    fn prune(&self, tree: &Tree) -> (bool, bool) {
        (true, true)
    }
}

pub struct NoopCommit {}
impl Commit for NoopCommit {
    fn write(&mut self, tree: &Tree) -> Result<()> {
        Ok(())
    }

    fn prune(&self, tree: &Tree) -> (bool, bool) {
        (false, false)
    }
}
