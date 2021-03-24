use super::Merk;
use crate::{Hash, Op, Result, proofs::{Decoder, Node, chunk::{verify_leaf, verify_trunk}, verify::{Child, Tree as ProofTree}}, tree::{Tree, Link}};
use failure::bail;
use rocksdb::WriteBatch;
use std::path::Path;
use std::iter::Peekable;

/// A `Restorer` handles decoding, verifying, and storing chunk proofs to
/// replicate an entire Merk tree. It expects the chunks to be processed in
/// order, retrying the last chunk if verification fails.
pub struct Restorer {
    leaf_hashes: Option<Peekable<std::vec::IntoIter<Hash>>>,
    parent_keys: Option<Peekable<std::vec::IntoIter<Vec<u8>>>>,
    merk: Merk,
    expected_root_hash: Hash,
    stated_length: usize,
}

impl Restorer {
    /// Creates a new `Restorer`, which will initialize a new Merk at the given
    /// file path. The first chunk (the "trunk") will be compared against
    /// `expected_root_hash`, then each subsequent chunk will be compared
    /// against the hashes stored in the trunk, so that the restore process will
    /// never allow malicious peers to send more than a single invalid chunk.
    ///
    /// The `stated_length` should be the number of chunks stated by the peer,
    /// which will be verified after processing a valid first chunk to make it
    /// easier to download chunks from peers without needing to trust this
    /// length.
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
            parent_keys: None,
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

    /// Consumes the `Restorer` and returns the newly-created, fully-populated
    /// Merk instance. This method will return an error if called before
    /// processing all chunks (e.g. `restorer.remaining_chunks()` is not equal
    /// to 0).
    pub fn finalize(self) -> Result<Merk> {
        if self.remaining_chunks() != 0 {
            bail!("Called finalize before all chunks were processed");
        }

        self.merk.flush()?;

        Ok(self.merk)
    }

    /// Returns the number of remainign chunks to be processed.
    pub fn remaining_chunks(&self) -> usize {
        self.leaf_hashes.as_ref().unwrap().len()
    }

    /// Writes the data contained in `tree` (extracted from a verified chunk
    /// proof) to the RocksDB.
    fn write_chunk(&mut self, tree: ProofTree) -> Result<()> {
        let mut batch = WriteBatch::default();

        tree.visit_refs(&mut |proof_node| {
            let (key, value) = match &proof_node.node {
                Node::KV(key, value) => (key, value),
                _ => return
            };

            // TODO: encode tree node without cloning key/value
            let mut node = Tree::new(key.clone(), value.clone());
            *node.slot_mut(true) = proof_node.left.as_ref().map(Child::as_link);
            *node.slot_mut(false) = proof_node.right.as_ref().map(Child::as_link);

            let bytes = node.encode();
            batch.put(key, bytes);
        });

        self.merk.write(batch)
    }

    /// Verifies the trunk then writes its data to the RocksDB.
    ///
    /// The trunk contains a height proof which lets us verify the total number
    /// of expected chunks is the same as `stated_length` as passed into
    /// `Restorer::new()`. We also verify the expected root hash at this step.
    fn process_trunk(&mut self, ops: Decoder) -> Result<usize> {
        let (trunk, height) = verify_trunk(ops)?;

        if trunk.hash() != self.expected_root_hash {
            bail!(
                "Proof did not match expected hash\n\tExpected: {:?}\n\tActual: {:?}",
                self.expected_root_hash,
                trunk.hash()
            );
        }

        let root_key = trunk.key().to_vec();

        let leaf_hashes = trunk
            .layer(height / 2)
            .map(|node| node.hash())
            .collect::<Vec<Hash>>()
            .into_iter()
            .peekable();
        self.leaf_hashes = Some(leaf_hashes);

        let parent_keys = trunk
            .layer(height / 2 - 1)
            .map(|node| node.key().to_vec())
            .collect::<Vec<Vec<u8>>>()
            .into_iter()
            .peekable();
        self.parent_keys = Some(parent_keys);
        assert_eq!(
            self.parent_keys.as_ref().unwrap().len(),
            self.leaf_hashes.as_ref().unwrap().len() / 2
        );

        let chunks_remaining = (2 as usize).pow(height as u32 / 2);
        assert_eq!(self.remaining_chunks(), chunks_remaining);
        
        // TODO: this one shouldn't be an assert because it comes from a peer
        assert_eq!(self.stated_length, chunks_remaining);

        // note that these writes don't happen atomically, which is fine here
        // because if anything fails during the restore process we will just
        // scrap the whole restore and start over
        self.write_chunk(trunk)?;
        self.merk.set_root_key(root_key)?;

        Ok(chunks_remaining)
    }

    /// Verifies a leaf chunk then writes it to the RocksDB. This needs to be
    /// called in order, retrying the last chunk for any failed verifications.
    fn process_leaf(&mut self, ops: Decoder) -> Result<usize> {
        let leaf_hashes = self.leaf_hashes.as_mut().unwrap();
        let leaf_hash = leaf_hashes
            .peek()
            .expect("Received more chunks than expected");

        let tree = verify_leaf(ops, *leaf_hash)?;
        
        // the parent of the root node of the leaf does not yet know the key of
        // its children. now that we have verified this leaf, we can write the
        // key into the parent node's entry. note that this does not need to
        // recalcuate hashes since it already had the child hash.
        let parent_keys = self.parent_keys.as_mut().unwrap();
        let parent_key = parent_keys.peek().unwrap().clone();
        let mut parent = self.merk.fetch_node(parent_key.as_slice())?
            .expect("Could not find parent of leaf chunk");
        let is_left_child = self.remaining_chunks() % 2 == 0;
        if let Some(Link::Reference { ref mut key, .. }) = parent.link_mut(is_left_child) {
            *key = tree.key().to_vec();
        } else {
            panic!("Expected parent links to be type Link::Reference");
        }
        let parent_bytes = parent.encode();
        self.merk.db.put(parent_key, parent_bytes)?;

        self.write_chunk(tree)?;

        let leaf_hashes = self.leaf_hashes.as_mut().unwrap();
        leaf_hashes.next();

        if !is_left_child {
            let parent_keys = self.parent_keys.as_mut().unwrap();
            parent_keys.next();
        }

        Ok(self.remaining_chunks())
    }
}

impl Merk {
    /// Creates a new `Restorer`, which can be used to verify chunk proofs to
    /// replicate an entire Merk tree. A new Merk instance will be initialized
    /// by creating a RocksDB at `path`.
    ///
    /// The restoration process will verify integrity by checking that the
    /// incoming chunk proofs match `expected_root_hash`. The `stated_length`
    /// should be the number of chunks as stated by peers, which will also be
    /// verified during the restoration process.
    pub fn restore<P: AsRef<Path>>(
        path: P,
        expected_root_hash: Hash,
        stated_length: usize,
    ) -> Result<Restorer> {
        Restorer::new(path, expected_root_hash, stated_length)
    }
}

impl Child {
    fn as_link(&self) -> Link {
        let key = match &self.tree.node {
            Node::KV(key, _) => key.as_slice(),
            // for the connection between the trunk and leaf chunks, we don't
            // have the child key so we must first write in an empty one. once
            // the leaf gets verified, we can write in this key to its parent
            _ => &[],
        };

        Link::Reference {
            hash: self.hash,
            child_heights: (
                self.tree.left.as_ref().map_or(0, |c| c.tree.height as u8),
                self.tree.right.as_ref().map_or(0, |c| c.tree.height as u8),
            ),
            key: key.to_vec(),
        }
    }
}
