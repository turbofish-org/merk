//! In-memory snapshots of database state.
//!
//! Snapshots are read-only views of the database state at a particular point in
//! time. This can be useful for retaining recent versions of history which can
//! be queried against. Merk snapshots are backed by the similar RocksDB
//! snapshot, but with the added ability to create proofs.

use std::cell::Cell;

use crate::{
    proofs::query::QueryItem,
    tree::{Fetch, RefWalker, Tree, NULL_HASH},
    Hash, Result,
};

/// A read-only view of the database state at a particular point in time.
///
/// `Snapshot`s are cheap to create since they are just a handle and don't copy
/// any data - they instead just prevent the underlying replaced data from being
/// compacted in RocksDB until they are dropped. They are only held in memory,
/// and will not be persisted after the process exits.
pub struct Snapshot<'a> {
    /// The underlying RocksDB snapshot.
    ss: Option<rocksdb::Snapshot<'a>>,
    /// The Merk tree at the time the snapshot was created.
    tree: Cell<Option<Tree>>,
    /// Whether the underlying RocksDB snapshot should be dropped when the
    /// `Snapshot` is dropped.
    should_drop_ss: bool,
}

impl<'a> Snapshot<'a> {
    /// Creates a new `Snapshot` from a RocksDB snapshot and a Merk tree.
    ///
    /// The RocksDB snapshot will be dropped when the [Snapshot] is dropped.
    pub fn new(db: rocksdb::Snapshot<'a>, tree: Option<Tree>) -> Self {
        Snapshot {
            ss: Some(db),
            tree: Cell::new(tree),
            should_drop_ss: true,
        }
    }

    /// Converts the [Snapshot] into a [StaticSnapshot], an alternative which
    /// has easier (but more dangerous) lifetime requirements.
    pub fn staticize(mut self) -> StaticSnapshot {
        let ss: RocksDBSnapshot = unsafe { std::mem::transmute(self.ss.take().unwrap()) };
        StaticSnapshot {
            tree: Cell::new(self.tree.take()),
            inner: ss.inner,
            should_drop: false,
        }
    }

    /// Gets the value associated with the given key, from the time the snapshot
    /// was created.
    pub fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        self.use_tree(|maybe_tree| {
            maybe_tree
                .and_then(|tree| super::get(tree, self.source(), key).transpose())
                .transpose()
        })
    }

    /// Gets the root hash of the tree at the time the snapshot was created.
    pub fn root_hash(&self) -> Hash {
        self.use_tree(|tree| tree.map_or(NULL_HASH, |tree| tree.hash()))
    }

    /// Proves the given query against the tree at the time the snapshot was
    /// created.
    pub fn prove<Q, I>(&self, query: I) -> Result<Vec<u8>>
    where
        Q: Into<QueryItem>,
        I: IntoIterator<Item = Q>,
    {
        self.use_tree_mut(move |maybe_tree| super::prove(maybe_tree, self.source(), query))
    }

    /// Walks the tree at the time the snapshot was created, fetching the child
    /// node from the backing store if necessary.
    pub fn walk<T>(&self, f: impl FnOnce(Option<RefWalker<SnapshotSource>>) -> T) -> T {
        let mut tree = self.tree.take();
        let maybe_walker = tree
            .as_mut()
            .map(|tree| RefWalker::new(tree, self.source()));
        let res = f(maybe_walker);
        self.tree.set(tree);
        res
    }

    /// Returns an iterator over the keys and values in the backing store from
    /// the time the snapshot was created.
    pub fn raw_iter(&self) -> rocksdb::DBRawIterator {
        self.ss.as_ref().unwrap().raw_iterator()
    }

    /// A data source which can be used to fetch values from the backing store,
    /// from the time the snapshot was created.
    fn source(&self) -> SnapshotSource {
        SnapshotSource(self.ss.as_ref().unwrap())
    }

    /// Uses the tree, and then puts it back.
    fn use_tree<T>(&self, f: impl FnOnce(Option<&Tree>) -> T) -> T {
        let tree = self.tree.take();
        let res = f(tree.as_ref());
        self.tree.set(tree);
        res
    }

    /// Uses the tree mutably, and then puts it back.
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

/// A data source which can be used to fetch values from the backing store, from
/// the time the snapshot was created.
///
/// This implements [Fetch] and should be used with a type such as [RefWalker].
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

/// A read-only view of the database state at a particular point in time, but
/// with an internal raw pointer to allow for manual lifetime management.
///
/// This is useful when you would otherwise want a [Snapshot], but you want to
/// use the database while the snapshot is still alive. This is unsafe because
/// it is the caller's responsibility to ensure that the underlying RocksDB
/// snapshot outlives the [StaticSnapshot].
///
/// By default, the RocksDB snapshot will not be dropped when the
/// [StaticSnapshot] is dropped, resulting in a memory leak. For correct usage,
/// you must call [StaticSnapshot::drop] to ensure the RocksDB snapshot gets
/// dropped when the [StaticSnapshot] is dropped.
pub struct StaticSnapshot {
    /// A Merk tree based on the database state at the time the snapshot was
    /// created.
    tree: Cell<Option<Tree>>,
    /// A raw pointer to the RocksDB snapshot.
    inner: *const (),
    /// Used to detect whether the `StaticSnapshot` was set to manually drop
    /// before its [Drop::drop] implementation was called.
    pub should_drop: bool,
}

/// An equivalent struct to the [rocksdb::Snapshot] struct within the `rocksdb`
/// crate. This is used to access the private fields of the foreign crate's
/// struct by first transmuting.
///
/// To guarantee that breaking changes in the `rocksdb` crate do not affect the
/// transmutation into this struct, see the
/// [tests::rocksdb_snapshot_struct_format] test.
struct RocksDBSnapshot<'a> {
    /// A reference to the associated RocksDB database.
    _db: &'a rocksdb::DB,
    /// A raw pointer to the snapshot handle.
    inner: *const (),
}

// We need this because we have a raw pointer to a RocksDB snapshot, but we
// know that our usage of it is thread-safe:
// https://github.com/facebook/rocksdb/blob/main/include/rocksdb/snapshot.h#L15-L16
unsafe impl Send for StaticSnapshot {}
unsafe impl Sync for StaticSnapshot {}

impl StaticSnapshot {
    /// Converts the [StaticSnapshot] to a [Snapshot] by re-associating with the
    /// database it was originally created from.
    ///
    /// # Safety
    /// This will cause undefined behavior if a database other than the one
    /// originally used to create the snapshot is passed as an argument.
    ///
    /// This will also cause a memory leak if the underlying RocksDB snapshot is
    /// not dropped by calling [StaticSnapshot::drop]. Unlike most uses of
    /// [Snapshot], the RocksDB snapshot will not be dropped when the
    /// [Snapshot] returned by this method is dropped.
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

    /// Drops the [StaticSnapshot] and the underlying RocksDB snapshot.
    ///
    /// # Safety
    /// This function is unsafe because it results in the RocksDB snapshot being
    /// dropped, which could lead to use-after-free bugs if there are still
    /// references to the snapshot in other [Snapshot] or [StaticSnapshot]
    /// instances. The caller must be sure this is the last remaining reference
    /// before calling this method.
    pub unsafe fn drop(mut self, db: &rocksdb::DB) {
        let mut ss = self.with_db(db);
        ss.should_drop_ss = true;
        self.should_drop = true;
        // the snapshot drop implementation is now called, which includes
        // dropping the RocksDB snapshot
    }

    /// Clones the root node of the Merk tree into a new [Tree].
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
