use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::tree::{
    Tree,
    Link,
    Fetch,
    Walker,
    RefWalker,
    Commit,
    Batch
};
use crate::proofs::encode_into;

// TODO: use a column family or something to keep the root key separate
const ROOT_KEY_KEY: [u8; 12] = *b"\00\00root\00\00";

/// A handle to a Merkle key/value store backed by RocksDB.
pub struct Merk {
    tree: Option<Tree>,
    db: rocksdb::DB,
    path: PathBuf
}

impl Merk {
    /// Opens a store with the specified file path. If no store exists at that
    /// path, one will be created.
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

    /// Gets a value for the given key. Returns an `Err` if the key is not found
    /// or something else goes wrong.
    ///
    /// Note that this is essentially the same as a normal RocksDB `get`, so
    /// should be a fast operation and has almost no tree overhead.
    pub fn get(&self, key: &[u8]) -> Result<Vec<u8>> {
        // TODO: ignore other fields when reading from node bytes
        let node = get_node(&self.db, key)?;
        // TODO: don't reallocate
        Ok(node.value().to_vec())
    }

    /// Applies a batch of operations (puts and deletes) to the tree.
    ///
    /// This will fail if the keys in `batch` are not sorted and unique. This
    /// check creates some overhead, so if you are sure your batch is sorted and
    /// unique you can use the unsafe `apply_unchecked` for a small performance
    /// gain.
    ///
    /// # Example
    /// ```
    /// # let mut store = merk::test_utils::TempMerk::new().unwrap();
    /// # store.apply(&[(vec![4,5,6], Op::Put(vec![0]))]).unwrap();
    /// 
    /// use merk::Op;
    ///
    /// let batch = &[
    ///     (vec![1, 2, 3], Op::Put(vec![4, 5, 6])), // puts value [4,5,6] to key [1,2,3]
    ///     (vec![4, 5, 6], Op::Delete) // deletes key [4,5,6]
    /// ];
    /// store.apply(batch).unwrap();
    /// ```
    pub fn apply(&mut self, batch: &Batch) -> Result<()> {
        // ensure keys in batch are sorted and unique
        let mut maybe_prev_key = None;
        for (key, _) in batch.iter() {
            if let Some(prev_key) = maybe_prev_key {
                if prev_key > *key {
                    bail!("Keys in batch must be sorted");
                } else if prev_key == *key {
                    bail!("Keys in batch must be unique");
                }
            }
            maybe_prev_key = Some(key.to_vec());
        }

        unsafe { self.apply_unchecked(batch) }
    }


    /// Applies a batch of operations (puts and deletes) to the tree.
    ///
    /// This is unsafe because the keys in `batch` must be sorted and unique -
    /// if they are not, there will be undefined behavior. For a safe version of
    /// this method which checks to ensure the batch is sorted and unique, see
    /// `apply`.
    ///
    /// # Example
    /// ```
    /// # let mut store = merk::test_utils::TempMerk::new().unwrap();
    /// # store.apply(&[(vec![4,5,6], Op::Put(vec![0]))]).unwrap();
    /// 
    /// use merk::Op;
    ///
    /// let batch = &[
    ///     (vec![1, 2, 3], Op::Put(vec![4, 5, 6])), // puts value [4,5,6] to key [1,2,3]
    ///     (vec![4, 5, 6], Op::Delete) // deletes key [4,5,6]
    /// ];
    /// unsafe { store.apply_unchecked(batch).unwrap() };
    /// ```
    pub unsafe fn apply_unchecked(&mut self, batch: &Batch) -> Result<()> {
        let maybe_walker = self.tree.take()
            .map(|tree| Walker::new(tree, self.source()));

        // TODO: will return set of deleted keys
        self.tree = Walker::apply_to(maybe_walker, batch)?;

        // commit changes to db
        self.commit()
    }

    /// Closes the store and deletes all data from disk.
    pub fn destroy(self) -> Result<()> {
        let opts = default_db_opts();
        drop(self.db);
        rocksdb::DB::destroy(&opts, &self.path)?;
        Ok(())
    }

    /// Creates a Merkle proof for the list of queried keys. For each key in the
    /// query, if the key is found in the store then the value will be proven to
    /// be in the tree. For each key in the query that does not exist in the
    /// tree, its absence will be proven by including boundary keys.
    ///
    /// The proof returned is in an encoded format which can be verified with
    /// `merk::verify`.
    ///
    /// This will fail if the keys in `query` are not sorted and unique. This
    /// check adds some overhead, so if you are sure your batch is sorted and
    /// unique you can use the unsafe `prove_unchecked` for a small performance
    /// gain.
    pub fn prove(&mut self, query: &[Vec<u8>]) -> Result<Vec<u8>> {
        // ensure keys in query are sorted and unique
        let mut maybe_prev_key = None;
        for key in query.iter() {
            if let Some(prev_key) = maybe_prev_key {
                if prev_key > *key {
                    bail!("Keys in query must be sorted");
                } else if prev_key == *key {
                    bail!("Keys in query must be unique");
                }
            }
            maybe_prev_key = Some(key.to_vec());
        }

        unsafe { self.prove_unchecked(query) }
    }

    /// Creates a Merkle proof for the list of queried keys. For each key in the
    /// query, if the key is found in the store then the value will be proven to
    /// be in the tree. For each key in the query that does not exist in the
    /// tree, its absence will be proven by including boundary keys.
    ///
    /// The proof returned is in an encoded format which can be verified with
    /// `merk::verify`.
    ///
    /// This is unsafe because the keys in `query` must be sorted and unique -
    /// if they are not, there will be undefined behavior. For a safe version of
    /// this method which checks to ensure the batch is sorted and unique, see
    /// `prove`.
    pub unsafe fn prove_unchecked(&mut self, query: &[Vec<u8>]) -> Result<Vec<u8>> {
        let mut tree = match self.tree.take() {
            None => bail!("Cannot create proof for empty tree"),
            Some(tree) => tree
        };

        let mut ref_walker = RefWalker::new(&mut tree, self.source());
        let (proof, _) = ref_walker.create_proof(query)?;

        self.tree = Some(tree);

        let mut bytes = Vec::with_capacity(128);
        encode_into(proof.iter(), &mut bytes);
        Ok(bytes)
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
    use crate::test_utils::*;

    #[test]
    fn simple_insert_apply() {
        let batch_size = 20;

        let path = thread::current().name().unwrap().to_owned();
        let mut merk = TempMerk::open(path).expect("failed to open merk");

        let batch = make_batch_seq(0..batch_size);
        merk.apply(&batch).expect("apply failed");

        assert_tree_invariants(merk.tree().expect("expected tree"));
    }

    #[test]
    fn insert_uncached() {
        let batch_size = 20;

        let path = thread::current().name().unwrap().to_owned();
        let mut merk = TempMerk::open(path).expect("failed to open merk");

        let batch = make_batch_seq(0..batch_size);
        merk.apply(&batch).expect("apply failed");
        assert_tree_invariants(merk.tree().expect("expected tree"));

        let batch = make_batch_seq(batch_size..(batch_size*2));
        merk.apply(&batch).expect("apply failed");
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
            merk.apply(&batch).expect("apply failed");

        }
    }
}
