use crate::error::Result;
use super::{Fetch, super::Tree};

// TODO: turn into a trait to make composable?
//       or add methods on wrapper/newtype?

pub struct OwnedWalker<S>
    where S: Fetch + Sized + Clone + Send
{
    tree: Tree,
    source: S
}

impl<S> OwnedWalker<S>
    where S: Fetch + Sized + Clone + Send
{
    pub fn new(tree: Tree, source: S) -> Self {
        OwnedWalker { tree, source }
    }

    pub fn walk(&mut self, left: bool) -> Result<Option<Self>> {
        if self.tree.link(left).is_none() {
            return Ok(None)
        }

        let maybe_child = self.tree.detach(left);
        let child = if let Some(child) = maybe_child {
            child
        } else {
            let key = match self.tree.link(left) {
                Link::Pruned { key } => key.as_slice(),
                _ => unreachable!("Expected Link::Pruned")
            };
            self.source.fetch(key)?
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

    pub fn into_inner(self) -> Tree {
        self.tree
    }

    fn wrap(&self, tree: Tree) -> Self {
        OwnedWalker {
            tree,
            source: self.source.clone()
        }
    }

    pub fn clone_source(&self) -> S {
        self.source.clone()
    }

    pub fn attach(mut self, left: bool, maybe_child: Option<Tree>) -> Self {
        self.tree = self.tree.attach(left, maybe_child);
        self
    }
}

#[cfg(test)]
mod test {
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
        let tree = Tree::new(b"test", b"abc")
            .attach(true, Some(Tree::new(b"foo", b"bar")));

        let source = MockSource {};
        let mut walker = OwnedWalker::new(tree, source);

        let child = walker.walk(true).unwrap();
        assert_eq!(child.unwrap().tree().node().key, b"foo");
        assert_eq!(
            walker
                .walk(true)
                .unwrap()
                .into_inner()
                .tree()
                .node()
                .key,
            b"foo"
        );
    }
}
