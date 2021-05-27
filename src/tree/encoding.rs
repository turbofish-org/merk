use super::Tree;
use ed::{Decode, Encode};

impl Tree {
    #[inline]
    pub fn encode(&self) -> Vec<u8> {
        // operation is infallible so it's ok to unwrap
        Encode::encode(self).unwrap()
    }

    #[inline]
    pub fn encode_into(&self, dest: &mut Vec<u8>) {
        // operation is infallible so it's ok to unwrap
        Encode::encode_into(self, dest).unwrap()
    }

    #[inline]
    pub fn encoding_length(&self) -> usize {
        // operation is infallible so it's ok to unwrap
        Encode::encoding_length(self).unwrap()
    }

    #[inline]
    pub fn decode_into(&mut self, key: Vec<u8>, input: &[u8]) {
        // operation is infallible so it's ok to unwrap
        Decode::decode_into(self, input).unwrap();
        self.inner.kv.key = key;
    }

    #[inline]
    pub fn decode(key: Vec<u8>, input: &[u8]) -> Tree {
        // operation is infallible so it's ok to unwrap
        let mut tree: Tree = Decode::decode(input).unwrap();
        tree.inner.kv.key = key;
        tree
    }
}

#[cfg(test)]
mod tests {
    use super::super::Link;
    use super::*;

    #[test]
    fn encode_leaf_tree() {
        let tree = Tree::from_fields(vec![0], vec![1], [55; 32], None, None);
        assert_eq!(tree.encoding_length(), 35);
        assert_eq!(
            tree.encode(),
            vec![
                0, 0, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55,
                55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 1,
            ]
        );
    }

    #[test]
    #[should_panic]
    fn encode_modified_tree() {
        let tree = Tree::from_fields(
            vec![0],
            vec![1],
            [55; 32],
            Some(Link::Modified {
                pending_writes: 1,
                child_heights: (123, 124),
                tree: Tree::new(vec![2], vec![3]),
            }),
            None,
        );
        tree.encode();
    }

    #[test]
    fn encode_loaded_tree() {
        let tree = Tree::from_fields(
            vec![0],
            vec![1],
            [55; 32],
            Some(Link::Loaded {
                hash: [66; 32],
                child_heights: (123, 124),
                tree: Tree::new(vec![2], vec![3]),
            }),
            None,
        );
        assert_eq!(
            tree.encode(),
            vec![
                1, 1, 2, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66,
                66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 123, 124, 0, 55, 55, 55,
                55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55,
                55, 55, 55, 55, 55, 55, 55, 55, 1
            ]
        );
    }

    #[test]
    fn encode_uncommitted_tree() {
        let tree = Tree::from_fields(
            vec![0],
            vec![1],
            [55; 32],
            Some(Link::Uncommitted {
                hash: [66; 32],
                child_heights: (123, 124),
                tree: Tree::new(vec![2], vec![3]),
            }),
            None,
        );
        assert_eq!(
            tree.encode(),
            vec![
                1, 1, 2, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66,
                66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 123, 124, 0, 55, 55, 55,
                55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55,
                55, 55, 55, 55, 55, 55, 55, 55, 1
            ]
        );
    }

    #[test]
    fn encode_reference_tree() {
        let tree = Tree::from_fields(
            vec![0],
            vec![1],
            [55; 32],
            Some(Link::Reference {
                hash: [66; 32],
                child_heights: (123, 124),
                key: vec![2],
            }),
            None,
        );
        assert_eq!(tree.encoding_length(), 71);
        assert_eq!(
            tree.encode(),
            vec![
                1, 1, 2, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66,
                66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 123, 124, 0, 55, 55, 55,
                55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55,
                55, 55, 55, 55, 55, 55, 55, 55, 1
            ]
        );
    }

    #[test]
    fn decode_leaf_tree() {
        let bytes = vec![
            0, 0, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55,
            1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
        ];
        let tree = Tree::decode(vec![0], bytes.as_slice());
        assert_eq!(tree.key(), &[0]);
        assert_eq!(tree.value(), &[1]);
    }

    #[test]
    fn decode_reference_tree() {
        let bytes = vec![
            1, 1, 2, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66,
            66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 123, 124, 0, 55, 55, 55, 55, 55,
            55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55, 55,
            55, 55, 55, 55, 55, 1,
        ];
        let tree = Tree::decode(vec![0], bytes.as_slice());
        assert_eq!(tree.key(), &[0]);
        assert_eq!(tree.value(), &[1]);
        if let Some(Link::Reference {
            key,
            child_heights,
            hash,
        }) = tree.link(true)
        {
            assert_eq!(*key, [2]);
            assert_eq!(*child_heights, (123 as u8, 124 as u8));
            assert_eq!(*hash, [66 as u8; 32]);
        } else {
            panic!("Expected Link::Reference");
        }
    }
}
