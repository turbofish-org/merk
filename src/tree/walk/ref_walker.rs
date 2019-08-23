use crate::error::Result;
use super::Fetch;
use super::super::{Tree, Link};

pub struct RefWalker<'a, S>
    where S: Fetch + Sized + Clone + Send
{
    tree: &'a Tree,
    source: S
}

impl<'a, S> RefWalker<'a, S>
    where S: Fetch + Sized + Clone + Send
{
    pub fn new(tree: &'a mut Tree, source: S) -> Self {
        Walker { tree, source }
    }

    pub fn tree(&self) -> &'a Tree {
        self.tree
    }

    pub fn walk(&mut self, left: bool) -> Result<Option<Self>> {
        let link = match self.tree.link(left) {
            None => return Ok(None),
            Some(link) => link
        };

        match link {
            Link::Modified { .. } => panic!("Cannot traverse Link::Modified"),
            Link::Stored { .. } => {},
            Link::Pruned { .. } => {
                self.tree.load(left, &self.source)?;
            }
        }

        Ok(self.tree.child_mut(left))
    }

    pub fn walk_expect(&self, left: bool) -> Result<Self> {

    }

    pub fn tree(&self) -> &Tree {
        &self.tree
    }

    fn wrap(&self, tree: Tree) -> Self {
        Walker::new(tree, self.source.clone())
    }

    pub fn clone_source(&self) -> S {
        self.source.clone()
    }
}

#[cfg(test)]
mod test {
    
}

