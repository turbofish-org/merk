use crate::error::Result;
use super::Fetch;
use super::super::{Tree, Link};

pub struct RefWalker<'a, S>
    where S: Fetch + Sized + Clone + Send
{
    tree: &'a mut Tree,
    source: S
}

impl<'a, S> RefWalker<'a, S>
    where S: Fetch + Sized + Clone + Send
{
    pub fn new(tree: &'a mut Tree, source: S) -> Self {
        RefWalker { tree, source }
    }

    pub fn tree(&self) -> &Tree {
        self.tree
    }

    pub fn walk<'b>(&'b mut self, left: bool) -> Result<Option<RefWalker<'b, S>>> {
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

        let child = self.tree.child_mut(left).unwrap();
        Ok(Some(RefWalker::new(child, self.source.clone())))
    }

    pub fn walk_expect<'b>(&'b mut self, left: bool) -> Result<RefWalker<'b, S>> {
        let maybe_child = self.walk(left)?;

        if let Some(child) = maybe_child {
            Ok(child)
        } else {
            panic!(
                "Expected {} child, got None",
                if left { "left" } else { "right" }
            );
        }
    }

    pub fn clone_source(&self) -> S {
        self.source.clone()
    }
}

#[cfg(test)]
mod test {
    
}

