use crate::error::Result;
use crate::fetch::Fetch;
use crate::util;
use super::Walk;
use super::super::Tree;

pub struct FetchWalker<F: Fetch + Sized> {
    tree: Tree,
    source: F
}

deref!(FetchWalker, Tree, tree);

impl Walk<Tree> for FetchWalker {
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