use std::cmp::Ordering;
use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::tree::{Tree, Fetch, Walker};
use super::ops::Batch;

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
            let mut encode_buf = Vec::with_capacity(256);

            // in-order traversal of modified nodes
            tree.commit(&mut |tree: &Tree| {
                encode_buf.clear();
                tree.encode_into(&mut encode_buf);
                batch.put(tree.key(), &encode_buf)?;
                Ok(())
            })?;

            // update pointer to root node
            batch.put(ROOT_KEY_KEY, tree.key())?;
        } else {
            // empty tree, delete pointer to root
            batch.delete(ROOT_KEY_KEY)?;
        }

        // write to db
        let mut opts = rocksdb::WriteOptions::default();
        opts.set_sync(false);
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
}

#[derive(Clone)]
struct MerkSource<'a> {
    db: &'a rocksdb::DB
}

impl<'a> Fetch for MerkSource<'a> {
    fn fetch(&self, key: &[u8]) -> Result<Tree> {
        get_node(&self.db, key)
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
    // TODO: tune
    opts
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn apply_unchecked() {
        
    }
}
