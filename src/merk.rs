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

    pub fn put_batch(&mut self, batch: &[(&[u8], &[u8])]) -> Result<(), Error> {
        let db = &self.db;
        let mut get_node = |link: &Link| {
            // TODO: Result instead of unwrap
            let bytes = &db.get(&link.key).unwrap().unwrap()[..];
            Node::decode(&link.key, bytes).unwrap()
        };

        match &mut self.tree {
            Some(tree) => {
                // tree is not empty, put under it
                tree.put_batch(&mut get_node, batch);
            },
            None => {
                // empty tree, set middle key/value as root
                let mid = batch.len() / 2;
                let mut tree = SparseTree::new(
                    Node::new(batch[mid].0, batch[mid].1)
                );

                // put the rest of the batch under the tree
                if batch.len() > 1 {
                    tree.put_batch(&mut get_node, &batch[..mid]);
                }
                if batch.len() > 2 {
                    tree.put_batch(&mut get_node, &batch[mid+1..]);
                }

                self.tree = Some(tree);
            }
        }

        // commit changes to db
        self.commit()
    }

    fn commit(&mut self) -> Result<(), Error> {
        if let Some(tree) = &mut self.tree {
            let batch = tree.to_write_batch();

            // TODO: store pointer to root node

            // TODO: write options
            self.db.write(batch)?;

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

#[test]
fn simple_put() {
    let mut merk = Merk::new("./test.db").unwrap();
    let batch: Vec<(&[u8], &[u8])> = vec![
        (b"key", b"value"),
        (b"key2", b"value2"),
    ];
    merk.put_batch(&batch);

    // let entries = merk.tree.as_ref().unwrap().entries();
    // for (key, value) in entries {
    //     println!(
    //         "{:?}: {:?}",
    //         String::from_utf8(key.to_vec()).unwrap(),
    //         String::from_utf8(value.to_vec()).unwrap()
    //     );
    // }
}
