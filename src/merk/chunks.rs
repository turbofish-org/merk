use std::iter::FilterMap;

use super::Merk;
use crate::proofs::{chunk::get_next_chunk, Node, Op};
use crate::tree::RefWalker;

use crate::Result;
use ed::Encode;
use rocksdb::DBRawIterator;

type OpFilterFn = fn(Op) -> Option<Vec<u8>>;

pub enum ChunkIter<'a> {
    Trunk {
        trunk: Option<Vec<Op>>,
        merk: Option<&'a mut Merk>,
    },
    PostTrunk {
        chunk_boundaries: FilterMap<std::vec::IntoIter<Op>, OpFilterFn>,
        raw_iter: DBRawIterator<'a>,
    },
}

impl<'a> ChunkIter<'a> {
    fn from_merk(merk: &'a mut Merk) -> Result<Self> {
        let trunk = merk.walk(|mut maybe_walker| match maybe_walker {
            Some(mut walker) => walker.create_trunk_proof(),
            None => Ok(vec![]),
        })?;

        Ok(ChunkIter::Trunk { trunk: Some(trunk), merk: Some(merk) })
    }
}

fn filter_trunk_ops(op: Op) -> Option<Vec<u8>> {
    match op {
        Op::Push(Node::KV(key, _)) => Some(key),
        _ => None
    }
}

impl<'a> Iterator for ChunkIter<'a> {
    type Item = Result<Vec<u8>>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            ChunkIter::Trunk { merk, trunk } => {
                // TODO: can we do a partial move more cleanly?
                let trunk = trunk.take().unwrap();
                let merk = merk.take().unwrap();

                let trunk_bytes = trunk.encode();

                let mut raw_iter = merk.db.raw_iterator();
                raw_iter.seek_to_first();

                let chunk_boundaries = trunk
                    .into_iter()
                    .filter_map(filter_trunk_ops as OpFilterFn);

                *self = ChunkIter::PostTrunk {
                    chunk_boundaries,
                    raw_iter
                };

                Some(trunk_bytes)
            },
            ChunkIter::PostTrunk { raw_iter, chunk_boundaries } => {
                if !raw_iter.valid() {
                    return None;
                }

                let end_key = chunk_boundaries.next();
                let end_key_slice = end_key.as_ref().map(|k| k.as_slice());

                match get_next_chunk(raw_iter, end_key_slice) {
                    Ok(chunk) => Some(chunk.encode()),
                    Err(err) => Some(Err(err)),
                }
            }
        }
    }
}

impl Merk {
    pub fn chunks(&mut self) -> Result<ChunkIter> {
        ChunkIter::from_merk(self)
    }
}

#[cfg(test)]
mod tests {

    use ed::Decode;

    use crate::test_utils::*;

    use super::*;

    #[test]
    fn generate_and_verify_chunks() {
        let mut merk = TempMerk::new().unwrap();
        let batch = make_batch_seq(1..111);
        merk.apply(batch.as_slice(), &[]).unwrap();

        let chunks = merk.chunks().unwrap();
        for chunk in chunks {
            let chunk: Vec<Op> = Vec::decode(chunk.unwrap().as_slice()).unwrap();
            println!("chunk: {:?}", chunk);
        }
    }
}
