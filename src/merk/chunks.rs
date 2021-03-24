use super::Merk;
use crate::proofs::{chunk::get_next_chunk, Node, Op};

use crate::Result;
use ed::Encode;
use failure::bail;
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
        let trunk = merk.walk(|maybe_walker| match maybe_walker {
            Some(mut walker) => walker.create_trunk_proof(),
            None => Ok(vec![]),
        })?;

        let chunk_boundaries = trunk
            .iter()
            .filter_map(|op| match op {
                Op::Push(Node::KV(key, _)) => Some(key.clone()),
                _ => None,
            })
            .collect();

        let mut raw_iter = merk.db.raw_iterator();
        raw_iter.seek_to_first();

        Ok(ChunkProducer {
            trunk,
            chunk_boundaries,
            raw_iter,
            index: 0,
        })
    }

    /// Gets the chunk with the given index. Errors if the index is out of
    /// bounds - the number of chunks can be checked by calling
    /// `producer.len()`.
    pub fn chunk(&mut self, index: usize) -> Result<Vec<u8>> {
        if index >= self.len() {
            bail!("Chunk index out-of-bounds");
        }

        self.index = index;

        if index == 0 || index == 1 {
            self.raw_iter.seek_to_first();
        } else {
            let preceding_key = self.chunk_boundaries.get(index - 1).unwrap();
            self.raw_iter.seek(preceding_key);
            self.raw_iter.next();
        }

        self.next_chunk()
    }

    /// Returns the total number of chunks for the underlying Merk tree.
    pub fn len(&self) -> usize {
        self.chunk_boundaries.len() + 1
    }

    /// Gets the next chunk based on the `ChunkProducer`'s internal index state.
    /// This is mostly useful for letting `ChunkIter` yield the chunks in order,
    /// optimizing throughput compared to random access.
    fn next_chunk(&mut self) -> Result<Vec<u8>> {
        if self.index == 0 {
            self.index += 1;
            return self.trunk.encode();
        }

        if self.index >= self.len() {
            panic!("Called next_chunk after end");
        }

        let end_key = self.chunk_boundaries.get(self.index - 1);
        let end_key_slice = end_key.as_ref().map(|k| k.as_slice());

        self.index += 1;

        let chunk = get_next_chunk(&mut self.raw_iter, end_key_slice)?;
        chunk.encode()
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
    fn generate_and_verify_chunks() {
        let mut merk = TempMerk::new().unwrap();
        let batch = make_batch_seq(1..111);
        merk.apply(batch.as_slice(), &[]).unwrap();

        let mut chunks = merk.chunks().unwrap().into_iter().map(Result::unwrap);

        let chunk = chunks.next().unwrap();
        let ops = Decoder::new(chunk.as_slice());
        let (trunk, height) = verify_trunk(ops).unwrap();
        assert_eq!(height, 7);

        assert_eq!(trunk.layer(3).count(), 8);

        for (chunk, node) in chunks.zip(trunk.layer(height / 2)) {
            let ops = Decoder::new(chunk.as_slice());
            verify_leaf(ops, node.hash()).unwrap();
        }
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
}
