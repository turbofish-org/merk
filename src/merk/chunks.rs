use super::Merk;
use crate::proofs::{chunk::get_next_chunk, Node, Op};

use crate::Result;
use ed::Encode;
use failure::bail;
use rocksdb::DBRawIterator;

pub struct ChunkProducer<'a> {
    trunk: Vec<Op>,
    chunk_boundaries: Vec<Vec<u8>>,
    raw_iter: DBRawIterator<'a>,
    index: usize,
}

impl<'a> ChunkProducer<'a> {
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

    pub fn len(&self) -> usize {
        self.chunk_boundaries.len() + 1
    }

    pub(crate) fn from_merk(merk: &'a Merk) -> Result<Self> {
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

    fn next_chunk(&mut self) -> Result<Vec<u8>> {
        if self.index == 0 {
            self.index += 1;
            return self.trunk.encode();
        }

        if self.index >= self.len() {
            panic!("Called next_chunk after end");
        }

        let end_key = self.chunk_boundaries.get(self.index);
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
    pub fn chunks(&self) -> Result<ChunkProducer> {
        ChunkProducer::from_merk(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::*;
    use ed::Decode;

    #[test]
    fn generate_and_verify_chunks() {
        let mut merk = TempMerk::new().unwrap();
        let batch = make_batch_seq(1..111);
        merk.apply(batch.as_slice(), &[]).unwrap();

        let chunks = merk.chunks().unwrap();
        for chunk in chunks {
            let chunk: Vec<Op> = Vec::decode(chunk.unwrap().as_slice()).unwrap();
            println!("\n\n{:?}", chunk);
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
