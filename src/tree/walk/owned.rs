use crate::error::Result;
use crate::fetch::Fetch;
use crate::util;
use super::Walk;
use super::super::Tree;

pub struct OwnedWalker<F: Fetch + Sized + Send> {
    tree: Tree,
    source: F
}

deref!(OwnedWalker, Tree, tree);

impl Walk<Tree> for OwnedWalker {
    fn walk(&self, left: bool) -> Result<Option<Self>> {
        let link = match self.tree.child_link(left) {
            Some(link) => link,
            None => return Ok(None)
        };

        let child = self.tree.child(left);
        
    }

    fn unwrap(self) -> Tree {
        self.tree
    }
}