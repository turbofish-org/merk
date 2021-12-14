//! Provides `ChunkProducer`, which creates chunk proofs for full replication of
//! a Merk.

use super::Merk;
use crate::proofs::{chunk::get_next_chunk, Node, Op};

use crate::{Error, Result};
use ed::Encode;
use rocksdb::DBRawIterator;

/// A `ChunkProducer` allows the creation of chunk proofs, used for trustlessly
/// replicating entire Merk trees. Chunks can be generated on the fly in a
/// random order, or iterated in order for slightly better performance.
pub struct ChunkProducer<'a> {
    trunk: Vec<Op>,
    chunk_boundaries: Vec<Vec<u8>>,
    raw_iter: DBRawIterator<'a>,
    index: usize,
}

impl<'a> ChunkProducer<'a> {
    /// Creates a new `ChunkProducer` for the given `Merk` instance. In the
    /// constructor, the first chunk (the "trunk") will be created.
    pub fn new(merk: &'a Merk) -> Result<Self> {
        let (trunk, has_more) = merk.walk(|maybe_walker| match maybe_walker {
            Some(mut walker) => walker.create_trunk_proof(),
            None => Ok((vec![], false)),
        })?;

        let chunk_boundaries = if has_more {
            trunk
                .iter()
                .filter_map(|op| match op {
                    Op::Push(Node::KV(key, _)) => Some(key.clone()),
                    _ => None,
                })
                .collect()
        } else {
            vec![]
        };

        let mut raw_iter = merk.raw_iter();
        raw_iter.seek_to_first();

        Ok(ChunkProducer {
            trunk,
            chunk_boundaries,
            raw_iter,
            index: 0,
        })
    }

    /// Gets the chunk with the given index. Errors if the index is out of
    /// bounds or the tree is empty - the number of chunks can be checked by calling
    /// `producer.len()`.
    pub fn chunk(&mut self, index: usize) -> Result<Vec<u8>> {
        if index >= self.len() {
            return Err(Error::IndexOutOfBounds("Chunk index out-of-bounds".into()));
        }

        self.index = index;

        if index == 0 || index == 1 {
            self.raw_iter.seek_to_first();
        } else {
            let preceding_key = self.chunk_boundaries.get(index - 2).unwrap();
            self.raw_iter.seek(preceding_key);
            self.raw_iter.next();
        }

        self.next_chunk()
    }

    /// Returns the total number of chunks for the underlying Merk tree.
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        let boundaries_len = self.chunk_boundaries.len();
        if boundaries_len == 0 {
            1
        } else {
            boundaries_len + 2
        }
    }

    /// Gets the next chunk based on the `ChunkProducer`'s internal index state.
    /// This is mostly useful for letting `ChunkIter` yield the chunks in order,
    /// optimizing throughput compared to random access.
    fn next_chunk(&mut self) -> Result<Vec<u8>> {
        if self.index == 0 {
            if self.trunk.is_empty() {
                return Err(Error::Fetch(
                    "Attempted to fetch chunk on empty tree".into(),
                ));
            }
            self.index += 1;
            return Ok(self.trunk.encode()?);
        }

        assert!(!(self.index >= self.len()), "Called next_chunk after end");

        let end_key = self.chunk_boundaries.get(self.index - 1);
        let end_key_slice = end_key.as_ref().map(|k| k.as_slice());

        self.index += 1;

        let chunk = get_next_chunk(&mut self.raw_iter, end_key_slice)?;
        Ok(chunk.encode()?)
    }
}

impl<'a> IntoIterator for ChunkProducer<'a> {
    type IntoIter = ChunkIter<'a>;
    type Item = <ChunkIter<'a> as Iterator>::Item;

    fn into_iter(self) -> Self::IntoIter {
        ChunkIter(self)
    }
}

/// A `ChunkIter` iterates through all the chunks for the underlying `Merk`
/// instance in order (the first chunk is the "trunk" chunk). Yields `None`
/// after all chunks have been yielded.
pub struct ChunkIter<'a>(ChunkProducer<'a>);

impl<'a> Iterator for ChunkIter<'a> {
    type Item = Result<Vec<u8>>;

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.0.len(), Some(self.0.len()))
    }

    fn next(&mut self) -> Option<Self::Item> {
        if self.0.index >= self.0.len() {
            None
        } else {
            Some(self.0.next_chunk())
        }
    }
}

impl Merk {
    /// Creates a `ChunkProducer` which can return chunk proofs for replicating
    /// the entire Merk tree.
    pub fn chunks(&self) -> Result<ChunkProducer> {
        ChunkProducer::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        proofs::{
            chunk::{verify_leaf, verify_trunk},
            Decoder,
        },
        test_utils::*,
    };

    #[test]
    fn len_small() {
        let mut merk = TempMerk::new().unwrap();
        let batch = make_batch_seq(1..256);
        merk.apply(batch.as_slice(), &[]).unwrap();

        let chunks = merk.chunks().unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks.into_iter().size_hint().0, 1);
    }

    #[test]
    fn len_big() {
        let mut merk = TempMerk::new().unwrap();
        let batch = make_batch_seq(1..10_000);
        merk.apply(batch.as_slice(), &[]).unwrap();

        let chunks = merk.chunks().unwrap();
        assert_eq!(chunks.len(), 129);
        assert_eq!(chunks.into_iter().size_hint().0, 129);
    }

    #[test]
    fn generate_and_verify_chunks() {
        let mut merk = TempMerk::new().unwrap();
        let batch = make_batch_seq(1..10_000);
        merk.apply(batch.as_slice(), &[]).unwrap();

        let mut chunks = merk.chunks().unwrap().into_iter().map(Result::unwrap);

        let chunk = chunks.next().unwrap();
        let ops = Decoder::new(chunk.as_slice());
        let (trunk, height) = verify_trunk(ops).unwrap();
        assert_eq!(height, 14);
        assert_eq!(trunk.hash(), merk.root_hash());

        assert_eq!(trunk.layer(7).count(), 128);

        for (chunk, node) in chunks.zip(trunk.layer(height / 2)) {
            let ops = Decoder::new(chunk.as_slice());
            verify_leaf(ops, node.hash()).unwrap();
        }
    }

    #[test]
    fn chunks_from_reopen() {
        let time = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = format!("chunks_from_reopen_{}.db", time);

        let original_chunks = {
            let mut merk = Merk::open(&path).unwrap();
            let batch = make_batch_seq(1..10);
            merk.apply(batch.as_slice(), &[]).unwrap();

            merk.chunks()
                .unwrap()
                .into_iter()
                .map(Result::unwrap)
                .collect::<Vec<_>>()
                .into_iter()
        };

        let merk = TempMerk::open(path).unwrap();
        let reopen_chunks = merk.chunks().unwrap().into_iter().map(Result::unwrap);

        for (original, checkpoint) in original_chunks.zip(reopen_chunks) {
            assert_eq!(original.len(), checkpoint.len());
        }
    }

    #[test]
    fn chunks_from_checkpoint() {
        let mut merk = TempMerk::new().unwrap();
        let batch = make_batch_seq(1..10);
        merk.apply(batch.as_slice(), &[]).unwrap();

        let path: std::path::PathBuf = "generate_and_verify_chunks_from_checkpoint.db".into();
        if path.exists() {
            std::fs::remove_dir_all(&path).unwrap();
        }
        let checkpoint = merk.checkpoint(&path).unwrap();

        let original_chunks = merk.chunks().unwrap().into_iter().map(Result::unwrap);
        let checkpoint_chunks = checkpoint.chunks().unwrap().into_iter().map(Result::unwrap);

        for (original, checkpoint) in original_chunks.zip(checkpoint_chunks) {
            assert_eq!(original.len(), checkpoint.len());
        }

        std::fs::remove_dir_all(&path).unwrap();
    }

    #[test]
    fn random_access_chunks() {
        let mut merk = TempMerk::new().unwrap();
        let batch = make_batch_seq(1..111);
        merk.apply(batch.as_slice(), &[]).unwrap();

        let chunks = merk
            .chunks()
            .unwrap()
            .into_iter()
            .map(Result::unwrap)
            .collect::<Vec<_>>();

        let mut producer = merk.chunks().unwrap();
        for i in 0..chunks.len() * 2 {
            let index = i % chunks.len();
            assert_eq!(producer.chunk(index).unwrap(), chunks[index]);
        }
    }

    #[test]
    #[should_panic(expected = "Attempted to fetch chunk on empty tree")]
    fn test_chunk_empty() {
        let merk = TempMerk::new().unwrap();

        let _chunks = merk
            .chunks()
            .unwrap()
            .into_iter()
            .map(Result::unwrap)
            .collect::<Vec<_>>();
    }

    #[test]
    #[should_panic(expected = "Chunk index out-of-bounds")]
    fn test_chunk_index_oob() {
        let mut merk = TempMerk::new().unwrap();
        let batch = make_batch_seq(1..42);
        merk.apply(batch.as_slice(), &[]).unwrap();

        let mut producer = merk.chunks().unwrap();
        let _chunk = producer.chunk(50000).unwrap();
    }

    #[test]
    fn test_chunk_index_gt_1_access() {
        let mut merk = TempMerk::new().unwrap();
        let batch = make_batch_seq(1..513);
        merk.apply(batch.as_slice(), &[]).unwrap();

        let mut producer = merk.chunks().unwrap();
        println!("length: {}", producer.len());
        let chunk = producer.chunk(2).unwrap();
        assert_eq!(
            chunk,
            vec![
                3, 8, 0, 0, 0, 0, 0, 0, 0, 18, 0, 60, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 3, 8, 0, 0, 0, 0, 0, 0, 0, 19, 0, 60, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 16, 3, 8, 0, 0, 0, 0, 0, 0, 0, 20, 0, 60, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 17, 3, 8, 0, 0, 0, 0, 0, 0, 0,
                21, 0, 60, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 16, 3, 8, 0,
                0, 0, 0, 0, 0, 0, 22, 0, 60, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 3, 8, 0, 0, 0, 0, 0, 0, 0, 23, 0, 60, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 16, 3, 8, 0, 0, 0, 0, 0, 0, 0, 24, 0, 60, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 17, 17, 3, 8, 0, 0, 0, 0, 0, 0, 0, 25, 0,
                60, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 16, 3, 8, 0, 0, 0, 0,
                0, 0, 0, 26, 0, 60, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 3,
                8, 0, 0, 0, 0, 0, 0, 0, 27, 0, 60, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 16, 3, 8, 0, 0, 0, 0, 0, 0, 0, 28, 0, 60, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 17, 3, 8, 0, 0, 0, 0, 0, 0, 0, 29, 0, 60, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 16, 3, 8, 0, 0, 0, 0, 0, 0,
                0, 30, 0, 60, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 3, 8, 0, 0,
                0, 0, 0, 0, 0, 31, 0, 60, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 16, 3, 8, 0, 0, 0, 0, 0, 0, 0, 32, 0, 60, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123,
                123, 123, 123, 123, 123, 17, 17, 17
            ]
        );
    }

    #[test]
    #[should_panic(expected = "Called next_chunk after end")]
    fn test_next_chunk_index_oob() {
        let mut merk = TempMerk::new().unwrap();
        let batch = make_batch_seq(1..42);
        merk.apply(batch.as_slice(), &[]).unwrap();

        let mut producer = merk.chunks().unwrap();
        let _chunk1 = producer.next_chunk();
        let _chunk2 = producer.next_chunk();
    }
}
