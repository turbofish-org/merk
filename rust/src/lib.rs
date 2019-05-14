extern crate rocksdb;

mod node {

    use rocksdb::{DB, DBIterator};
    use std::convert::TryInto;

    // TODO: include neighbor keys, or just use iterator to access?
    pub struct Node<'a> {
        bytes: &'a mut [u8]
    }

    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    pub struct TryFromBytesError ();

    macro_rules! field {
        ($name:ident, $offset:expr, $length:expr) => {
            pub fn $name (&self) -> &[u8; $length] {
                self.bytes[$offset..($offset + $length)]
                    .try_into()
                    .unwrap()
            }
        };
        ($name:ident, $offset:expr) => {
            pub fn $name (&self) -> u8 {
                self.bytes[$offset]
            }
        };
    }

    macro_rules! field_mut {
        ($name:ident, $offset:expr, $length:expr) => {
            pub fn $name (&mut self) -> &mut [u8; $length] {
                (&mut self.bytes[$offset..($offset + $length)])
                    .try_into()
                    .unwrap()
            }
        };
        ($name:ident, $offset:expr) => {
            pub fn $name (&mut self) -> &mut u8 {
                &mut self.bytes[$offset]
            }
        };
    }

    impl<'a> Node<'a> {
        pub fn from_bytes(bytes: &'a mut [u8]) -> Result<Node<'a>, TryFromBytesError> {
            if bytes.len() < 63 {
                Err(TryFromBytesError())
            } else {
                Ok(Node{bytes})
            }
        }

        field!(hash, 0, 20);
        field_mut!(hash_mut, 0, 20);

        field!(kv_hash, 20, 20);
        field_mut!(kv_hash_mut, 20, 20);

        field!(parent_hash, 42, 20);
        field_mut!(parent_hash_mut, 42, 20);

        field!(parent_side, 60);
        field_mut!(parent_side_mut, 60);

        field!(left_height, 61);
        field_mut!(left_height_mut, 61);

        field!(right_height, 62);
        field_mut!(right_height_mut, 62);
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
            2,2,2,2,2, 2,2,2,2,2, 2,2,2,2,2, 2,2,2,2,2,
            3, 4, 5
        ];
        let mut node = Node::from_bytes(&mut bytes).unwrap();

        println!("{:?}", node.hash());
        println!("{:?}", node.kv_hash());
        println!("{:?}", node.left_height());
        node.hash_mut()[0] += 10;
        *node.left_height_mut() += 20;
        println!("{:?}", node.hash());
        println!("{:?}", node.kv_hash());
        println!("{:?}", node.left_height());
        println!("{:?}", &bytes[..]);
    }
}
