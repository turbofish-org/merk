use std::cmp::Ordering;
use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::tree::{Tree, Link, Fetch, Walker, Commit};
use super::ops::{Batch, BatchEntry};

// TODO: use a column family or something to keep the root key separate
const ROOT_KEY_KEY: [u8; 12] = *b"\00\00root\00\00";

/// A handle to a Merkle key/value store backed by RocksDB.
pub struct Merk {
    tree: Option<Tree>,
    db: rocksdb::DB,
    path: PathBuf
}

impl Merk {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Merk> {
        let db_opts = default_db_opts();
        let mut path_buf = PathBuf::new();
        path_buf.push(path);
        let db = rocksdb::DB::open(&db_opts, &path_buf)?;

        // try to load root node
        let tree = match db.get_pinned(ROOT_KEY_KEY)? {
            Some(root_key) => Some(get_node(&db, &root_key)?),
            None => None
        };

        Ok(Merk { tree, db, path: path_buf })
    }

    pub fn get(&self, key: &[u8]) -> Result<Vec<u8>> {
        // TODO: ignore other fields when reading from node bytes
        let node = get_node(&self.db, key)?;
        // TODO: don't reallocate
        Ok(node.value().to_vec())
    }

    pub fn apply(&mut self, batch: &mut Batch) -> Result<()> {
        // sort batch and ensure there are no duplicate keys
        let mut duplicate = false;
        batch.sort_by(|a, b| {
            let cmp = a.0.cmp(&b.0);
            if let Ordering::Equal = cmp {
                duplicate = true;
            }
            cmp
        });
        if duplicate {
            bail!("Batch must not have duplicate keys");
        }

        self.apply_unchecked(batch)
    }

    pub fn apply_unchecked(&mut self, batch: &Batch) -> Result<()> {
        let maybe_walker = self.tree.take()
            .map(|tree| Walker::new(tree, self.source()));

        // TODO: will return set of deleted keys
        self.tree = Walker::apply_to(maybe_walker, batch)?;

        // commit changes to db
        self.commit()
    }

    pub fn destroy(self) -> Result<()> {
        let opts = default_db_opts();
        drop(self.db);
        rocksdb::DB::destroy(&opts, &self.path)?;
        Ok(())
    }

    fn commit(&mut self) -> Result<()> {
        // TODO: concurrent commit

        let mut batch = rocksdb::WriteBatch::default();

        if let Some(tree) = &mut self.tree {
            // TODO: configurable committer
            let mut committer = MerkCommitter::new(tree.height(), 1);
            tree.commit(&mut committer)?;

            committer.batch.sort_by(|a, b| a.0.cmp(&b.0));
            for (key, value) in committer.batch {
                batch.put(key, value)?;
            }

            // update pointer to root node
            batch.put(ROOT_KEY_KEY, tree.key())?;
        } else {
            // empty tree, delete pointer to root
            batch.delete(ROOT_KEY_KEY)?;
        }

        // write to db
        let mut opts = rocksdb::WriteOptions::default();
        opts.set_sync(false);
        opts.disable_wal(true);
        self.db.write_opt(batch, &opts)?;

        Ok(())
    }

    pub fn map_range<F: FnMut(Tree)>(
        &self,
        start: &[u8],
        end: &[u8],
        f: &mut F
    ) -> Result<()> {
        let iter = self.db.iterator(
            rocksdb::IteratorMode::From(
                start,
                rocksdb::Direction::Forward
            )
        );

        for (key, value) in iter {
            let node = Tree::decode(&key, &value)?;
            f(node);

            if key[..] >= end[..] {
                break;
            }
        }

        Ok(())
    }

    fn source(&self) -> MerkSource {
        MerkSource { db: &self.db }
    }

    fn tree(&self) -> Option<&Tree> {
        self.tree.as_ref()
    }
}

#[derive(Clone)]
struct MerkSource<'a> {
    db: &'a rocksdb::DB
}

impl<'a> Fetch for MerkSource<'a> {
    fn fetch(&self, link: &Link) -> Result<Tree> {
        get_node(&self.db, link.key())
    }
}

struct MerkCommitter {
    batch: Vec<(Vec<u8>, Vec<u8>)>,
    height: u8,
    levels: u8
}

impl MerkCommitter {
    fn new(height: u8, levels: u8) -> Self {
        MerkCommitter { batch: Vec::with_capacity(10000), height, levels }
    }
}

impl Commit for MerkCommitter {
    fn write(&mut self, tree: &Tree) -> Result<()> {
        let mut buf = Vec::with_capacity(tree.encoding_length());
        tree.encode_into(&mut buf);
        self.batch.push((tree.key().to_vec(), buf));
        Ok(())
    }

    fn prune(&self, tree: &Tree) -> (bool, bool) {
        // keep N top levels of tree
        let prune = (self.height - tree.height()) > self.levels;
        (prune, prune)
    }
}

fn get_node(db: &rocksdb::DB, key: &[u8]) -> Result<Tree> {
    // TODO: for bottom levels, iterate and return tree with descendants
    let bytes = db.get_pinned(key)?;
    if let Some(bytes) = bytes {
        Tree::decode(key, &bytes)
    } else {
        bail!("key not found: '{:?}'", key)
    }
}

fn default_db_opts() -> rocksdb::Options {
    let mut opts = rocksdb::Options::default();
    opts.create_if_missing(true);
    opts.increase_parallelism(num_cpus::get() as i32);
    // opts.set_advise_random_on_open(false);
    opts.set_allow_mmap_writes(true);
    opts.set_allow_mmap_reads(true);
    // TODO: tune
    opts
}

#[cfg(test)]
mod test {
    use std::thread;
    use crate::*;
    use crate::test_utils::*;

    #[test]
    fn simple_insert_apply_unchecked() {
        let batch_size = 20;

        let path = thread::current().name().unwrap().to_owned();
        let mut merk = TempMerk::open(path).expect("failed to open merk");

        let mut batch = make_batch_seq(0..batch_size);
        merk.apply_unchecked(&mut batch).expect("apply failed");

        assert_tree_invariants(merk.tree().expect("expected tree"));
    }

    #[test]
    fn insert_uncached() {
        let batch_size = 20;

        let path = thread::current().name().unwrap().to_owned();
        let mut merk = TempMerk::open(path).expect("failed to open merk");

        let batch = make_batch_seq(0..batch_size);
        merk.apply_unchecked(&batch).expect("apply failed");
        assert_tree_invariants(merk.tree().expect("expected tree"));

        let batch = make_batch_seq(batch_size..(batch_size*2));
        merk.apply_unchecked(&batch).expect("apply failed");
        assert_tree_invariants(merk.tree().expect("expected tree"));
    }

    #[test]
    fn insert_rand() {
        let tree_size = 40;
        let batch_size = 4;

        let path = thread::current().name().unwrap().to_owned();
        let mut merk = TempMerk::open(path).expect("failed to open merk");

        for i in 0..(tree_size / batch_size) {
            println!("i:{}", i);
            let batch = make_batch_rand(batch_size, i);
            merk.apply_unchecked(&batch).expect("apply failed");

        }
    }
}
