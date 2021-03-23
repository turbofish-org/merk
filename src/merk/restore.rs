use super::Merk;
use crate::{
    proofs::{
        chunk::{verify_leaf, verify_trunk},
        verify::Tree,
        Decoder, Node,
    },
    Hash, Op, Result,
};
use failure::bail;
use rocksdb::WriteBatch;
use std::path::Path;

pub struct Restorer {
    leaf_hashes: Option<std::iter::Peekable<std::vec::IntoIter<Hash>>>,
    merk: Merk,
    expected_root_hash: Hash,
    stated_length: usize,
}

impl Restorer {
    pub fn new<P: AsRef<Path>>(
        db_path: P,
        expected_root_hash: Hash,
        stated_length: usize,
    ) -> Result<Self> {
        if db_path.as_ref().exists() {
            bail!("The given path already exists");
        }

        Ok(Self {
            expected_root_hash,
            stated_length,
            merk: Merk::open(db_path)?,
            leaf_hashes: None,
        })
    }

    /// Verifies a chunk and writes it to the working RocksDB instance. Expects
    /// to be called for each chunk in order. Returns the number of remaining
    /// chunks.
    ///
    /// Once there are no remaining chunks to be processed, `finalize` should
    /// be called.
    pub fn process_chunk(&mut self, chunk_bytes: &[u8]) -> Result<usize> {
        let ops = Decoder::new(chunk_bytes);

        match self.leaf_hashes {
            None => self.process_trunk(ops),
            Some(_) => self.process_leaf(ops),
        }
    }

    pub fn finalize(self) -> Result<Merk> {
        if self.remaining_chunks() != 0 {
            bail!("Called finalize before all chunks were processed");
        }

        self.merk.flush()?;

        Ok(self.merk)
    }

    fn write_chunk(&mut self, tree: Tree) -> Result<()> {
        // Write nodes in trunk proof to database
        let mut batch = WriteBatch::default();
        tree.visit_nodes(&mut |node| {
            if let Node::KV(key, value) = node {
                batch.put(key, value);
            }
        });
        self.merk.write(batch)?;

        Ok(())
    }

    fn process_trunk(&mut self, ops: Decoder) -> Result<usize> {
        let (trunk, height) = verify_trunk(ops)?;

        if trunk.hash() != self.expected_root_hash {
            bail!(
                "Proof did not match expected hash\n\tExpected: {:?}\n\tActual: {:?}",
                self.expected_root_hash,
                trunk.hash()
            );
        }

        let root_key = match trunk.node {
            Node::KV(ref key, _) => key.clone(),
            _ => panic!("Expected root node to be type KV"),
        };
        let leaf_hashes = trunk
            .layer(height / 2)
            .map(|node| node.hash())
            .collect::<Vec<Hash>>()
            .into_iter()
            .peekable();
        self.leaf_hashes = Some(leaf_hashes);

        let chunks_remaining = (2 as usize).pow(height as u32 / 2);
        assert_eq!(self.remaining_chunks(), chunks_remaining);
        assert_eq!(self.stated_length, chunks_remaining);

        // note that these writes don't happen atomically, which is fine here
        // because if anything fails during the restore process we will just
        // scrap the whole restore and start over
        self.write_chunk(trunk)?;
        self.merk.set_root_key(root_key)?;

        Ok(chunks_remaining)
    }

    fn process_leaf(&mut self, ops: Decoder) -> Result<usize> {
        let leaf_hashes = self.leaf_hashes.as_mut().unwrap();
        let leaf_hash = leaf_hashes
            .peek()
            .expect("Received more chunks than expected");

        let tree = verify_leaf(ops, *leaf_hash)?;

        self.write_chunk(tree)?;

        let leaf_hashes = self.leaf_hashes.as_mut().unwrap();
        leaf_hashes.next();
        Ok(self.remaining_chunks())
    }

    fn remaining_chunks(&self) -> usize {
        self.leaf_hashes.as_ref().unwrap().len()
    }
}

impl Merk {
    pub fn restore<P: AsRef<Path>>(
        path: P,
        expected_root_hash: Hash,
        stated_length: usize,
    ) -> Result<Restorer> {
        Restorer::new(path, expected_root_hash, stated_length)
    }
}
