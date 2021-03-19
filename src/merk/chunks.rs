use super::Merk;
use crate::proofs::{chunk::get_next_chunk, Node, Op};
use crate::tree::RefWalker;

use crate::Result;
use ed::Encode;
use rocksdb::DBRawIterator;

pub struct ChunkIter<'a> {
    raw_iter: DBRawIterator<'a>,
    emitted_trunk: bool,
    trunk: Vec<Op>,
    index: usize,
}

impl<'a> ChunkIter<'a> {
    fn from_merk(merk: &'a mut Merk) -> Result<Self> {
        let trunk = merk.use_tree_mut(|tree| match tree {
            Some(tree) => {
                let mut walker = RefWalker::new(tree, merk.source());
                walker.create_trunk_proof()
            }
            None => Ok(vec![]),
        })?;
        let mut raw_iter = merk.db.raw_iterator();
        raw_iter.seek_to_first();
        Ok(Self {
            raw_iter,
            emitted_trunk: false,
            trunk,
            index: 0,
        })
    }
}
impl<'a> Iterator for ChunkIter<'a> {
    type Item = Result<Vec<u8>>;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.emitted_trunk {
            self.emitted_trunk = true;
            return Some(self.trunk.encode());
        }
        if !self.raw_iter.valid() {
            return None;
        }
        let end_key = loop {
            if self.index >= self.trunk.len() {
                break None;
            }
            self.index += 1;

            if let Op::Push(Node::KV(ref key, _)) = self.trunk[self.index - 1] {
                break Some(key.as_slice());
            }
        };
        let chunk_res = get_next_chunk(&mut self.raw_iter, end_key);
        match chunk_res {
            Ok(chunk) => Some(chunk.encode()),
            Err(err) => Some(Err(err)),
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
