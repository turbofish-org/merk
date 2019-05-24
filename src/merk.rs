extern crate rocksdb;
extern crate num_cpus;
extern crate rand;

use rand::prelude::*;

use std::path::{Path, PathBuf};
use rocksdb::Error;

use crate::*;

/// A handle to a Merklized key/value store backed by RocksDB.
pub struct Merk {
    tree: Option<SparseTree>,
    db: Option<rocksdb::DB>,
    path: PathBuf
}

impl Merk {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Merk, Error> {
        let db_opts = defaultDbOpts();
        let mut path_buf = PathBuf::new();
        path_buf.push(path);
        Ok(Merk{
            tree: None,
            db: Some(rocksdb::DB::open(&db_opts, &path_buf)?),
            path: path_buf
        })
    }

    pub fn put_batch(&mut self, batch: &[(&[u8], &[u8])]) -> Result<(), Error> {
        let db = &self.db.as_ref().unwrap();
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

    pub fn delete(mut self) -> Result<(), Error> {
        let opts = defaultDbOpts();
        self.db.take();
        rocksdb::DB::destroy(&opts, &self.path)
    }

    fn commit(&mut self) -> Result<(), Error> {
        if let Some(tree) = &mut self.tree {
            let batch = tree.to_write_batch();

            // TODO: store pointer to root node

            // TODO: write options
            self.db.as_ref().unwrap().write(batch)?;

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
    let mut merk = Merk::new("./test_merk_simple_put.db").unwrap();
    let batch: Vec<(&[u8], &[u8])> = vec![
        (b"key", b"value"),
        (b"key2", b"value2"),
    ];
    merk.put_batch(&batch).unwrap();
    merk.delete().unwrap();

    // let entries = merk.tree.as_ref().unwrap().entries();
    // for (key, value) in entries {
    //     println!(
    //         "{:?}: {:?}",
    //         String::from_utf8(key.to_vec()).unwrap(),
    //         String::from_utf8(value.to_vec()).unwrap()
    //     );
    // }
}

#[bench]
fn bench_put(b: &mut test::Bencher) {
    let mut merk = Merk::new("./test_merk_bench_put.db").unwrap();

    let mut rng = rand::thread_rng();

    let mut tree = SparseTree::new(
        Node::new(b"0", b"x")
    );

    let mut i = 0;
    b.iter(|| {
        let mut kvs = vec![];
        for i in 0..10_000 {
            kvs.push(random_bytes(&mut rng, 220));
        }

        let mut batch: Vec<(&[u8], &[u8])> = vec![];
        for i in 0..10_000 {
            batch.push((&kvs[i][..20], &kvs[i][20..]));
        }

        merk.put_batch(&batch).unwrap();

        i += 1;
    });

    println!("final tree size: {}", i * 10_000);

    merk.delete().unwrap();
}

fn random_bytes(rng: &mut ThreadRng, length: usize) -> Vec<u8> {
    (0..length)
        .map(|_| -> u8 { rng.gen() })
        .collect()
}
