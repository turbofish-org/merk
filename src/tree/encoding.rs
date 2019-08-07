use std::convert::TryInto;

use super::{Tree, Link};
use crate::error::Result;

// TODO: Encode, Decode traits

impl Link {
    pub fn encode_into(&self, output: &mut Vec<u8>) {
        let (hash, key, height) = match self {
            Link::Pruned { hash, key, height } => (hash, key, height),
            Link::Modified { .. } => panic!("No encoding for Link::Modified"),
            Link::Stored { .. } => panic!("No encoding for Link::Stored")
        };

        output.push(key.len().try_into().unwrap());
        output.extend_from_slice(key.as_slice());

        output.extend_from_slice(hash);

        output.push(*height);
    }

    pub fn encoding_length(&self) -> usize {
        match self {
            Link::Pruned { hash, key, .. } => {
                1 +
                key.len() +
                20 +
                1
            },
            Link::Modified { .. } => panic!("No encoding for Link::Modified"),
            Link::Stored { .. } => panic!("No encoding for Link::Stored")
        }
    }
}

impl Tree {
    pub fn encode_into(&self, output: &mut Vec<u8>) {
        let value_len = self.value().len();
        output.push((value_len & 0xff).try_into().unwrap());
        output.push((value_len >> 8).try_into().unwrap());
        output.extend_from_slice(self.value());

        output.extend_from_slice(self.inner.kv.hash());

        match self.link(true) {
            None => output.push(0),
            Some(link) => link.encode_into(output)
        }

        match self.link(false) {
            None => output.push(0),
            Some(link) => link.encode_into(output)
        }
    }

    pub fn encoding_length(&self) -> usize {
        2 + // value length
        self.inner.kv.value().len() + // value bytes
        20 + // kv_hash length
        self.link(true).map_or(1, |link| link.encoding_length()) +
        self.link(false).map_or(1, |link| link.encoding_length())
    }

    pub fn decode(key: &[u8], bytes: &[u8]) -> Result<Tree> {
        unimplemented!("todo")
    }
}

#[cfg(test)]
mod test {
    use super::super::{Tree, Link};

    #[test]
    fn encode_tree() {
        let tree = Tree::new(vec![0], vec![1]);
        assert_eq!(tree.encoding_length(), 25);

        let mut bytes = vec![];
        tree.encode_into(&mut bytes);
        assert_eq!(bytes, vec![1, 0, 1, 195, 201, 244, 70, 50, 255, 177, 215, 40, 246, 8, 69, 174, 17, 72, 99, 29, 112, 226, 212, 0, 0]);
    }

    #[test]
    fn encode_link() {
        let link = Link::Pruned {
            key: vec![1, 2, 3],
            height: 123,
            hash: [55; 20]
        };
        assert_eq!(link.encoding_length(), 25);

        let mut bytes = vec![];
        link.encode_into(&mut bytes);
        assert_eq!(bytes, vec![3, 1, 2, 3, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 123]);
    }
}
