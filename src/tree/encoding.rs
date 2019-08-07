use std::convert::TryInto;

use super::{Tree, Link, Hash};
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

    pub fn decode(bytes: &[u8]) -> Result<Link> {
        let mut offset = 0;

        let length = bytes[offset];
        offset += 1;

        let key = bytes[offset..offset + length as usize].to_vec();
        offset += length as usize;

        let mut hash: Hash = Default::default();
        hash.copy_from_slice(&bytes[offset..offset + 20]);
        offset += 20;

        let height = bytes[offset];
        offset += 1;

        Ok(Link::Pruned { key, hash, height })
    }
}

impl Tree {
    pub fn encode_into(&self, output: &mut Vec<u8>) {
        let value_len = self.value().len();
        // TODO: use byteorder package
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
        let mut offset = 0;

        let value_len =
            bytes[offset] as usize
            + ((bytes[offset + 1] as usize) << 8);
        offset += 2;

        let value = bytes[offset..offset + value_len].to_vec();
        offset += value_len;

        let mut kv_hash: Hash = Default::default();
        kv_hash.copy_from_slice(&bytes[offset..offset + 20]);
        offset += 20;

        let link_length = bytes[offset];
        let left = if link_length > 0 {
            let link = Link::decode(&bytes[offset..])?;
            offset += link.encoding_length();
            Some(link)
        } else {
            offset += 1;
            None
        };

        let link_length = bytes[offset];
        let right = if link_length > 0 {
            let link = Link::decode(&bytes[offset..])?;
            offset += link.encoding_length();
            Some(link)
        } else {
            offset += 1;
            None
        };

        Ok(Tree::from_fields(
            key.to_vec(),
            value,
            kv_hash,
            left,
            right
        ))
    }
}

#[cfg(test)]
mod test {
    use super::super::{Tree, Link};

    #[test]
    fn encode_leaf_tree() {
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

    #[test]
    #[should_panic]
    fn encode_link_long_key() {
        let link = Link::Pruned {
            key: vec![123; 300],
            height: 123,
            hash: [55; 20]
        };
        let mut bytes = vec![];
        link.encode_into(&mut bytes);
    }

    #[test]
    #[should_panic]
    fn encode_modified_tree() {
        let tree = Tree::from_fields(
            vec![0], vec![1],
            [55; 20],
            Some(Link::Modified {
                pending_writes: 1,
                height: 123,
                tree: Tree::new(vec![2], vec![3])
            }),
            None
        );
        let mut bytes = vec![];
        tree.encode_into(&mut bytes);
    }

    #[test]
    #[should_panic]
    fn encode_stored_tree() {
        let tree = Tree::from_fields(
            vec![0], vec![1],
            [55; 20],
            Some(Link::Stored {
                hash: [66; 20],
                height: 123,
                tree: Tree::new(vec![2], vec![3])
            }),
            None
        );
        let mut bytes = vec![];
        tree.encode_into(&mut bytes);
    }

    #[test]
    fn encode_pruned_tree() {
        let tree = Tree::from_fields(
            vec![0], vec![1],
            [55; 20],
            Some(Link::Pruned {
                hash: [66; 20],
                height: 123,
                key: vec![2]
            }),
            None
        );
        assert_eq!(tree.encoding_length(), 47);
        
        let mut bytes = vec![];
        tree.encode_into(&mut bytes);
        assert_eq!(bytes, vec![1, 0, 1, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 1, 2, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 123, 0].as_slice());
    }

    #[test]
    fn decode_leaf_tree() {
        let bytes = vec![1, 0, 1, 195, 201, 244, 70, 50, 255, 177, 215, 40, 246, 8, 69, 174, 17, 72, 99, 29, 112, 226, 212, 0, 0];
        let tree = Tree::decode(&[0], bytes.as_slice()).expect("decode failed");
        assert_eq!(tree.key(), &[0]);
        assert_eq!(tree.value(), &[1]);
    }

    #[test]
    fn decode_pruned_tree() {
        let bytes = vec![1, 0, 1, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 1, 2, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 123, 0];
        let tree = Tree::decode(&[0], bytes.as_slice()).expect("decode failed");
        assert_eq!(tree.key(), &[0]);
        assert_eq!(tree.value(), &[1]);
        if let Some(Link::Pruned { key, height, hash }) = tree.link(true) {
            assert_eq!(key, &[2]);
            assert_eq!(*height, 123 as u8);
            assert_eq!(hash, &[66; 20]);
        } else {
            panic!("Expected Link::Pruned");
        }
    }
}
