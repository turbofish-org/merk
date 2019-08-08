use crate::error::Result;
use super::{Fetch, Owner};
use super::super::{Tree, Link};

pub struct Walker<S>
    where S: Fetch + Sized + Clone + Send
{
    tree: Owner<Tree>,
    source: S
}

impl<S> Walker<S>
    where S: Fetch + Sized + Clone + Send
{
    pub fn new(tree: Tree, source: S) -> Self {
        Walker { tree: Owner::new(tree), source }
    }

    pub fn walk(&mut self, left: bool) -> Result<Option<Self>> {
        if self.tree.link(left).is_none() {
            return Ok(None)
        }

        let maybe_child = self.tree.own(|tree| tree.detach(left));
        let child = if let Some(child) = maybe_child {
            child
        } else {
            let key = match self.tree.link(left) {
                Some(Link::Pruned { key, .. }) => key.as_slice(),
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
        self.tree.into_inner()
    }

    fn wrap(&self, tree: Tree) -> Self {
        Walker::new(tree, self.source.clone())
    }

    pub fn clone_source(&self) -> S {
        self.source.clone()
    }

    pub fn attach(mut self, left: bool, maybe_child: Option<Tree>) -> Self {
        let tree = self.tree.into_inner()
            .attach(left, maybe_child);
        self.tree = Owner::new(tree);
        self
    }

    pub fn with_value(mut self, value: Vec<u8>) -> Self {
        let tree = self.tree.into_inner()
            .with_value(value);
        self.tree = Owner::new(tree);
        self
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::tree::Tree;

    #[derive(Clone)]
    struct MockSource {}

    impl Fetch for MockSource {
        fn fetch(&self, key: &[u8]) -> Result<Tree> {
            Ok(Tree::new(key.to_vec(), b"foo".to_vec()))
        }
    }

    #[test]
    fn walk_modified() {
        let tree = Tree::new(
                b"test".to_vec(),
                b"abc".to_vec()
            )
            .attach(true, Some(Tree::new(
                b"foo".to_vec(),
                b"bar".to_vec()
            )));

        let source = MockSource {};
        let mut walker = Walker::new(tree, source);

        let child = walker.walk(true).expect("walk failed");
        assert_eq!(child.expect("should have child").tree().key(), b"foo");
        assert!(walker.into_inner().child(true).is_none());
    }

    #[test]
    fn walk_stored() {
        let mut tree = Tree::new(
                b"test".to_vec(),
                b"abc".to_vec()
            )
            .attach(true, Some(Tree::new(
                b"foo".to_vec(),
                b"bar".to_vec()
            )));
        tree.commit(&mut |tree: &Tree| Ok(()))
            .expect("commit failed");

        let source = MockSource {};
        let mut walker = Walker::new(tree, source);

        let child = walker.walk(true).expect("walk failed");
        assert_eq!(child.expect("should have child").tree().key(), b"foo");
        assert!(walker.into_inner().child(true).is_none());
    }

    #[test]
    fn walk_pruned() {
        // TODO: enable once we can prune tree
        // let tree = Tree::new(
        //     b"test".to_vec(),
        //     b"abc".to_vec()
        // );

        // let source = MockSource {};
        // let mut walker = Walker::new(tree, source);

        // let child = walker.walk(true).expect("walk failed");
        // assert_eq!(child.expect("should have child").tree().key(), b"foo");
        // assert!(walker.into_inner().child(true).is_none());
    }
    
    #[test]
    fn walk_none() {
        let tree = Tree::new(
            b"test".to_vec(),
            b"abc".to_vec()
        );

        let source = MockSource {};
        let mut walker = Walker::new(tree, source);

        let child = walker.walk(true).expect("walk failed");
        assert!(child.is_none());
    }
}
