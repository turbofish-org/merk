extern crate rocksdb;

mod node {
    use rocksdb::{DB, DBIterator};
    use std::mem::{size_of, transmute};

    const HASH_LENGTH: usize = 20;
    const MAX_KEY_LENGTH: usize = 80;
    const NODE_HEAD_LENGTH: usize = size_of::<NodeHead>();

    type Hash = [u8; HASH_LENGTH];
    type Key = [u8; MAX_KEY_LENGTH];

    #[repr(C)]
    struct NodeHead {
        hash: Hash,
        kv_hash: Hash,
        parent_key: Key,
        left_key: Key,
        right_key: Key,
        left_height: u8,
        right_height: u8
    }

    #[repr(C)]
    pub struct Node {
        head: NodeHead,
        value: [u8]
    }

    #[derive(Debug)]
    pub struct TryFromBytesError ();

    impl<'a> Node {
        pub fn try_from_bytes(bytes: &'a mut [u8]) -> Result<&'a mut Node, TryFromBytesError> {
            if bytes.len() < NODE_HEAD_LENGTH {
                Err(TryFromBytesError())
            } else {
                let value_length = bytes.len() - NODE_HEAD_LENGTH;
                let shortened_ptr = &mut bytes[0..value_length];
                unsafe {
                    Ok(transmute(shortened_ptr))
                }
            }
        }

        pub fn hash(&self) -> &Hash {
            &self.head.hash
        }
        pub fn hash_mut(&mut self) -> &mut Hash {
            &mut self.head.hash
        }

        pub fn kv_hash(&self) -> &Hash {
            &self.head.kv_hash
        }
        pub fn kv_hash_mut(&mut self) -> &mut Hash {
            &mut self.head.kv_hash
        }

        pub fn parent_key(&self) -> &Key {
            &self.head.parent_key
        }
        pub fn parent_key_mut(&mut self) -> &mut Key {
            &mut self.head.parent_key
        }

        pub fn left_key(&self) -> &Key {
            &self.head.left_key
        }
        pub fn left_key_mut(&mut self) -> &mut Key {
            &mut self.head.left_key
        }

        pub fn right_key(&self) -> &Key {
            &self.head.right_key
        }
        pub fn right_key_mut(&mut self) -> &mut Key {
            &mut self.head.right_key
        }

        pub fn right_height(&self) -> u8 {
            self.head.right_height
        }
        pub fn right_height_mut(&mut self) -> &mut u8 {
            &mut self.head.right_height
        }

        pub fn left_height(&self) -> u8 {
            self.head.left_height
        }
        pub fn left_height_mut(&mut self) -> &mut u8 {
            &mut self.head.left_height
        }

        pub fn value(&self) -> &[u8] {
            &self.value
        }
        pub fn value_mut(&mut self) -> &mut [u8] {
            &mut self.value
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::node::*;

    #[test]
    fn it_works() {
        let mut bytes = [
            0,0,0,0,0, 0,0,0,0,0, 0,0,0,0,0, 0,0,0,0,0,
            1,1,1,1,1, 1,1,1,1,1, 1,1,1,1,1, 1,1,1,1,1,
            2,2,2,2,2, 2,2,2,2,2, 2,2,2,2,2, 2,2,2,2,2, 2,2,2,2,2, 2,2,2,2,2, 2,2,2,2,2, 2,2,2,2,2, 2,2,2,2,2, 2,2,2,2,2, 2,2,2,2,2, 2,2,2,2,2, 2,2,2,2,2, 2,2,2,2,2, 2,2,2,2,2, 2,2,2,2,2,
            3,3,3,3,3, 3,3,3,3,3, 3,3,3,3,3, 3,3,3,3,3, 3,3,3,3,3, 3,3,3,3,3, 3,3,3,3,3, 3,3,3,3,3, 3,3,3,3,3, 3,3,3,3,3, 3,3,3,3,3, 3,3,3,3,3, 3,3,3,3,3, 3,3,3,3,3, 3,3,3,3,3, 3,3,3,3,3,
            4,4,4,4,4, 4,4,4,4,4, 4,4,4,4,4, 4,4,4,4,4, 4,4,4,4,4, 4,4,4,4,4, 4,4,4,4,4, 4,4,4,4,4, 4,4,4,4,4, 4,4,4,4,4, 4,4,4,4,4, 4,4,4,4,4, 4,4,4,4,4, 4,4,4,4,4, 4,4,4,4,4, 4,4,4,4,4,
            5, 6,
            7,7,7,7,7, 7,7,7,7,7
        ];
        {
            let mut node = Node::try_from_bytes(&mut bytes).unwrap();

            println!("{:?}", node.hash());
            println!("{:?}", node.kv_hash());
            println!("{:?}", &node.left_key()[..]);
            println!("{:?}", node.left_height());
            *node.right_height_mut() += 10;
            println!("{:?}", node.right_height());
            println!("{:?}", &node.value()[..]);
        }
        println!("{:?}", &bytes[..]);
        // println!("{:?}", &node.value[..]);
        // println!("{:?}", node.head.hash);
        // println!("{:?}", node.head.kv_hash);
        // println!("{:?}", node.head.left_height);
        // println!("{:?}", node.left_height());
        // node.hash_mut()[0] += 10;
        // *node.left_height_mut() += 20;
        // println!("{:?}", node.hash());
        // println!("{:?}", node.kv_hash());
        // println!("{:?}", node.left_height());
        // println!("{:?}", &bytes[..]);
    }
}
