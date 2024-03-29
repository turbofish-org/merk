use std::cell::Cell;

use crate::{
    proofs::{query::QueryItem, Query},
    tree::{Fetch, RefWalker, Tree, NULL_HASH},
    Hash, Result,
};

pub struct Snapshot<'a> {
    ss: Option<rocksdb::Snapshot<'a>>,
    tree: Cell<Option<Tree>>,
    should_drop_ss: bool,
}

impl<'a> Snapshot<'a> {
    pub fn new(db: rocksdb::Snapshot<'a>, tree: Option<Tree>) -> Self {
        Snapshot {
            ss: Some(db),
            tree: Cell::new(tree),
            should_drop_ss: true,
        }
    }

    pub fn staticize(mut self) -> StaticSnapshot {
        let ss: RocksDBSnapshot = unsafe { std::mem::transmute(self.ss.take().unwrap()) };
        StaticSnapshot {
            tree: Cell::new(self.tree.take()),
            inner: ss.inner,
            should_drop: false,
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
        self.ss.as_ref().unwrap().raw_iterator()
    }

    fn source(&self) -> SnapshotSource {
        SnapshotSource(self.ss.as_ref().unwrap())
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

impl<'a> Drop for Snapshot<'a> {
    fn drop(&mut self) {
        if !self.should_drop_ss {
            std::mem::forget(self.ss.take());
        }
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

pub struct StaticSnapshot {
    tree: Cell<Option<Tree>>,
    inner: *const (),
    pub should_drop: bool,
}

struct RocksDBSnapshot<'a> {
    _db: &'a rocksdb::DB,
    inner: *const (),
}

// We need this because we have a raw pointer to a RocksDB snapshot, but we
// know that our usage of it is thread-safe:
// https://github.com/facebook/rocksdb/blob/main/include/rocksdb/snapshot.h#L15-L16
unsafe impl<'a> Send for StaticSnapshot {}
unsafe impl<'a> Sync for StaticSnapshot {}

impl StaticSnapshot {
    pub unsafe fn with_db<'a>(&self, db: &'a rocksdb::DB) -> Snapshot<'a> {
        let db_ss = RocksDBSnapshot {
            _db: db,
            inner: self.inner,
        };
        let db_ss: rocksdb::Snapshot<'a> = std::mem::transmute(db_ss);

        Snapshot {
            ss: Some(db_ss),
            tree: self.clone_tree(),
            should_drop_ss: false,
        }
    }

    pub unsafe fn drop<'a>(mut self, db: &'a rocksdb::DB) {
        let mut ss = self.with_db(db);
        ss.should_drop_ss = true;
        self.should_drop = true;
    }

    fn clone_tree(&self) -> Cell<Option<Tree>> {
        let tree = self.tree.take().unwrap();
        let tree_clone = Cell::new(Some(Tree::decode(
            tree.key().to_vec(),
            tree.encode().as_slice(),
        )));
        self.tree.set(Some(tree));
        tree_clone
    }
}

impl Drop for StaticSnapshot {
    fn drop(&mut self) {
        if !self.should_drop {
            log::debug!("StaticSnapshot must be manually dropped");
        }
    }
}

impl Clone for StaticSnapshot {
    fn clone(&self) -> Self {
        Self {
            tree: self.clone_tree(),
            inner: self.inner,
            should_drop: self.should_drop,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::mem::transmute;

    use super::RocksDBSnapshot;
    use crate::test_utils::TempMerk;

    #[test]
    fn rocksdb_snapshot_struct_format() {
        assert_eq!(std::mem::size_of::<rocksdb::Snapshot>(), 16);

        let merk = TempMerk::new().unwrap();
        let exptected_db_ptr = merk.db() as *const _;

        let ss = merk.db().snapshot();
        let ss: RocksDBSnapshot = unsafe { transmute(ss) };
        let db_ptr = ss._db as *const _;

        assert_eq!(exptected_db_ptr, db_ptr);
    }
}
