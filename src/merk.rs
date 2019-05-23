extern crate rocksdb;
extern crate num_cpus;

use std::path::Path;
use rocksdb::Error;

use crate::*;

/// A handle to a Merklized key/value store backed by RocksDB.
pub struct Merk {
    tree: Option<SparseTree>,
    db: rocksdb::DB
}

impl Merk {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Merk, Error> {
        let db_opts = defaultDbOpts();
        Ok(Merk{
            tree: None,
            db: rocksdb::DB::open(&db_opts, path)?
        })
    }

    pub fn put(&mut self, key: &[u8], value: &[u8]) {
        let db = &self.db;
        let mut get_node = |link: &Link| {
            // TODO: Result instead of unwrap
            let bytes = &db.get(&link.key).unwrap().unwrap()[..];
            Node::decode(&link.key, bytes).unwrap()
        };

        match &mut self.tree {
            Some(tree) => {
                // tree is not empty, put under it
                tree.put(&mut get_node, key, value);
            },
            None => {
                // empty tree, set key/value as root
                let tree = SparseTree::new(
                    Node::new(key, value)
                );
                self.tree = Some(tree);
            }
        }
    }

    pub fn commit(&mut self) -> Result<(), Error> {
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

#[test]
fn simple_put() {
    let mut merk = Merk::new("./test.db").unwrap();
    merk.put(b"key", b"value");
    merk.put(b"key", b"value2");
    merk.put(b"key2", b"value");

    let entries = merk.tree.as_ref().unwrap().entries();
    for (key, value) in entries {
        println!(
            "{:?}: {:?}",
            String::from_utf8(key.to_vec()).unwrap(),
            String::from_utf8(value.to_vec()).unwrap()
        );
    }
}
