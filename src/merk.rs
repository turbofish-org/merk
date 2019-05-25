extern crate rocksdb;
extern crate num_cpus;

use std::path::{Path, PathBuf};

use crate::sparse_tree::*;
use crate::node::*;
use crate::error::*;

/// A handle to a Merkle key/value store backed by RocksDB.
pub struct Merk {
    pub tree: Option<SparseTree>,
    db: Option<rocksdb::DB>,
    path: PathBuf
}

impl Merk {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Merk> {
        let db_opts = defaultDbOpts();
        let mut path_buf = PathBuf::new();
        path_buf.push(path);
        Ok(Merk{
            tree: None,
            db: Some(rocksdb::DB::open(&db_opts, &path_buf)?),
            path: path_buf
        })
    }

    pub fn put_batch(&mut self, batch: &[(&[u8], &[u8])]) -> Result<()> {
        // TODO: db shouldn't be option?
        let db = &self.db.as_ref().unwrap();

        let mut get_node = |link: &Link| -> Result<Node> {
            // errors if there is a db issue
            let bytes = db.get_pinned(&link.key)?;
            if let Some(bytes) = bytes {
                // errors if we can't decode the bytes
                Node::decode(&link.key, &bytes)
            } else {
                // key not found error
                bail!("key not found: '{:?}'", &link.key)
            }
        };

        match &mut self.tree {
            Some(tree) => {
                // tree is not empty, put under it
                tree.put_batch(&mut get_node, batch)?;
            },
            None => {
                // empty tree, set middle key/value as root
                let mid = batch.len() / 2;
                let mut tree = SparseTree::new(
                    Node::new(batch[mid].0, batch[mid].1)
                );

                // put the rest of the batch under the tree
                if batch.len() > 1 {
                    tree.put_batch(&mut get_node, &batch[..mid])?;
                }
                if batch.len() > 2 {
                    tree.put_batch(&mut get_node, &batch[mid+1..])?;
                }

                self.tree = Some(tree);
            }
        }

        // commit changes to db
        self.commit()
    }

    pub fn delete(mut self) -> Result<()> {
        let opts = defaultDbOpts();
        self.db.take();
        rocksdb::DB::destroy(&opts, &self.path)?;
        Ok(())
    }

    fn commit(&mut self) -> Result<()> {
        if let Some(tree) = &mut self.tree {
            let batch = tree.to_write_batch();

            // TODO: store pointer to root node

            let mut opts = rocksdb::WriteOptions::default();
            opts.set_sync(false);

            self.db.as_ref().unwrap().write_opt(batch, &opts)?;

            // clear tree so it only contains the root node
            // TODO: strategies for persisting nodes in memory
            tree.prune();
        } else {
            // TODO: clear db
        }

        Ok(())
    }
}

fn defaultDbOpts() -> rocksdb::Options {
    let mut opts = rocksdb::Options::default();
    opts.create_if_missing(true);
    opts.increase_parallelism(num_cpus::get() as i32);
    // TODO: tune
    opts
}
