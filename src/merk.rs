use std::cmp::Ordering;
use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::node::{Link, Node};
use crate::sparse_tree::{SparseTree, TreeBatch};

const KEY_PREFIX: [u8; 1] = *b".";
const ROOT_KEY_KEY: [u8; 4] = *b"root";

/// A handle to a Merkle key/value store backed by RocksDB.
pub struct Merk {
    pub tree: Option<Box<SparseTree>>,
    db: rocksdb::DB,
    path: PathBuf,
}

impl Merk {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Merk> {
        let db_opts = default_db_opts();
        let mut path_buf = PathBuf::new();
        path_buf.push(path);
        let db = rocksdb::DB::open(&db_opts, &path_buf)?;

        // try to load root node
        let tree = match db.get_pinned(ROOT_KEY_KEY)? {
            Some(root_key) => {
                let root_node = get_node(&db, &root_key)?;
                Some(Box::new(SparseTree::new(root_node)))
            },
            None => None
        };

        Ok(Merk { tree, db, path: path_buf })
    }

    pub fn apply(&mut self, batch: &mut TreeBatch) -> Result<()> {
        let db = &self.db;
        let mut get_node = |link: &Link| -> Result<Node> {
            get_node(db, &link.key)
        };

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

        // apply tree operations, setting resulting root node in self.tree
        SparseTree::apply(&mut self.tree, &mut get_node, batch)?;

        // commit changes to db
        self.commit()
    }

    pub fn apply_unchecked(&mut self, batch: &TreeBatch) -> Result<()> {
        let db = &self.db;
        let mut get_node = |link: &Link| -> Result<Node> {
            get_node(db, &link.key)
        };

        // apply tree operations, setting resulting root node in self.tree
        SparseTree::apply(&mut self.tree, &mut get_node, batch)?;

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
        let mut batch = rocksdb::WriteBatch::default();

        if let Some(tree) = &mut self.tree {
            // get nodes to flush to disk
            let modified = tree.modified()?;
            for (key, value) in modified {
                let key = concat(&KEY_PREFIX, key);
                batch.put(key, value)?;
            }

            // update pointer to root node
            batch.put(ROOT_KEY_KEY, &tree.key)?;
        } else {
            // empty tree, delete pointer to root
            batch.delete(ROOT_KEY_KEY)?;
        }

        // write to db
        let mut opts = rocksdb::WriteOptions::default();
        opts.set_sync(false);
        self.db.write_opt(batch, &opts)?;

        if let Some(tree) = &mut self.tree {
            // clear tree so it only contains the root node
            // TODO: strategies for persisting nodes in memory
            tree.prune();
        }

        Ok(())
    }
}

fn get_node(db: &rocksdb::DB, key: &[u8]) -> Result<Node> {
    // errors if there is a db issue
    let bytes = db.get_pinned(key)?;
    if let Some(bytes) = bytes {
        // errors if we can't decode the bytes
        Node::decode(key, &bytes)
    } else {
        // key not found error
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

fn concat(a: &[u8], b: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(a.len() + b.len());
    result.extend_from_slice(a);
    result.extend_from_slice(b);
    result
}
