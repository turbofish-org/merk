use crate::error::Result;
use crate::fetch::Fetch;
use super::super::Tree;

pub struct OwnedWalker<S>
    where S: Fetch + Sized + Send + Clone
{
    tree: Tree,
    source: S
}

impl<S> OwnedWalker<S>
    where S: Fetch + Sized + Send + Clone
{
    pub fn walk(&mut self, left: bool) -> Result<Option<Self>> {
        if self.tree.node().child_link(left).is_none() {
            return Ok(None)
        }

        let maybe_child = self.tree.detach(left);
        let child = if let Some(child) = maybe_child {
            child
        } else {
            let link = self.tree.node().child_link(left).as_ref().unwrap();
            let node = self.source.fetch(&link.key[..])?;
            Tree::new(node)
        };

        Ok(Some(self.wrap(child)))
    }

    pub fn walk_expect(&mut self, left: bool) -> Result<Self> {
        let maybe_walker = self.walk(left)?;

        if let Some(walker) = maybe_walker {
            Ok(walker)
        } else {
            unreachable!(
                "Expected {} child, got None",
                if left { "left" } else { "right" }
            )
        }
    }

    pub fn tree(&self) -> &Tree {
        &self.tree
    }

    pub fn unwrap(self) -> Tree {
        self.tree
    }

    fn wrap(&self, tree: Tree) -> Self {
        OwnedWalker {
            tree,
            source: self.source.clone()
        }
    }
}

#[cfg(test)]
mod test {
    use crate::fetch::Fetch;
    use super::*;
    use crate::tree::{Tree, Node};

    #[derive(Clone)]
    struct MockSource {}

    impl Fetch for MockSource {
        fn fetch(&self, key: &[u8]) -> Result<Node> {
            Ok(Node::new(key, b"foo"))
        }
    }

    #[test]
    fn test() {
        let tree = Tree::new(Node::new(b"test", b"abc"))
            .attach(true, Some(Tree::new(Node::new(b"foo", b"bar"))));

        let source = MockSource {};
        let mut walker = OwnedWalker { tree, source };

        let child = walker.walk(true).unwrap();
        assert_eq!(child.unwrap().tree().node().key, b"foo");
    }
}
