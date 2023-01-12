use std::cell::Cell;

use crate::{
    proofs::{query::QueryItem, Query},
    tree::{Fetch, RefWalker, Tree, NULL_HASH},
    Hash, Result,
};

pub struct Snapshot<'a> {
    db: rocksdb::Snapshot<'a>,
    tree: Cell<Option<Tree>>,
}

impl<'a> Snapshot<'a> {
    pub fn new(db: rocksdb::Snapshot<'a>, tree: Option<Tree>) -> Self {
        Snapshot {
            db,
            tree: Cell::new(tree),
        }
    }

    pub fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        self.use_tree(|maybe_tree| {
            maybe_tree
                .and_then(|tree| super::get(tree, self.source(), key).transpose())
                .transpose()
        })
    }

    pub fn root_hash(&self) -> Hash {
        self.use_tree(|tree| tree.map_or(NULL_HASH, |tree| tree.hash()))
    }

    pub fn prove(&self, query: Query) -> Result<Vec<u8>> {
        self.prove_unchecked(query)
    }

    pub fn prove_unchecked<Q, I>(&self, query: I) -> Result<Vec<u8>>
    where
        Q: Into<QueryItem>,
        I: IntoIterator<Item = Q>,
    {
        self.use_tree_mut(move |maybe_tree| {
            super::prove_unchecked(maybe_tree, self.source(), query.into_iter())
        })
    }

    pub fn walk<T>(&self, f: impl FnOnce(Option<RefWalker<SnapshotSource>>) -> T) -> T {
        let mut tree = self.tree.take();
        let maybe_walker = tree
            .as_mut()
            .map(|tree| RefWalker::new(tree, self.source()));
        let res = f(maybe_walker);
        self.tree.set(tree);
        res
    }

    pub fn raw_iter(&self) -> rocksdb::DBRawIterator {
        self.db.raw_iterator()
    }

    fn source(&self) -> SnapshotSource {
        SnapshotSource(&self.db)
    }

    fn use_tree<T>(&self, f: impl FnOnce(Option<&Tree>) -> T) -> T {
        let tree = self.tree.take();
        let res = f(tree.as_ref());
        self.tree.set(tree);
        res
    }

    fn use_tree_mut<T>(&self, f: impl FnOnce(Option<&mut Tree>) -> T) -> T {
        let mut tree = self.tree.take();
        let res = f(tree.as_mut());
        self.tree.set(tree);
        res
    }
}

#[derive(Clone)]
pub struct SnapshotSource<'a>(&'a rocksdb::Snapshot<'a>);

impl<'a> Fetch for SnapshotSource<'a> {
    fn fetch_by_key(&self, key: &[u8]) -> Result<Option<Tree>> {
        Ok(self
            .0
            .get(key)?
            .map(|bytes| Tree::decode(key.to_vec(), &bytes)))
    }
}
