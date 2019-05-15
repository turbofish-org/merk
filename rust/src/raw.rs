use std::mem::{size_of, transmute};

pub const HASH_LENGTH: usize = 20;
pub const MAX_KEY_LENGTH: usize = 80;
pub const RAWNODE_HEAD_LENGTH: usize = size_of::<RawNodeHead>();

pub type Hash = [u8; HASH_LENGTH];
pub type Key = [u8; MAX_KEY_LENGTH];

#[repr(C)]
struct RawNodeHead {
    hash: Hash,
    kv_hash: Hash,
    parent_key: Key,
    left_key: Key,
    right_key: Key,
    left_height: u8,
    right_height: u8
}

/// Represents a tree node as stored in the database.
#[repr(C)]
pub struct RawNode {
    head: RawNodeHead,
    value: [u8]
}

#[derive(Debug)]
pub struct FromBytesError ();

///
impl<'a> RawNode {
    /// Creates a `RawNode` by wrapping a slice of bytes.
    ///
    /// This operation is zero-copy, so the resulting `&mut RawNode` points to
    /// the same memory as the slice. Any direct modifications to the slice
    /// will affect the values of the `RawNode` as well.
    ///
    /// The slice is checked to be at least [`RAWNODE_HEAD_LENGTH`] bytes long,
    /// and returns `Err(FromBytesError)` if it is too small.
    ///
    /// Note that the lifetime of the `&mut RawNode` will match the lifetime
    /// of the input byte slice.
    ///
    /// [`RAWNODE_HEAD_LENGTH`]: constant.RAWNODE_HEAD_LENGTH.html
    pub fn from_bytes(bytes: &'a mut [u8]) -> Result<&'a mut RawNode, FromBytesError> {
        if bytes.len() < RAWNODE_HEAD_LENGTH {
            // length is variable since we have the `value` field, but we know
            // it must be at least as long as `RawNodeHead` which contains all
            // the required fields
            Err(FromBytesError())
        } else {
            let value_length = bytes.len() - RAWNODE_HEAD_LENGTH;
            // create a reference with the same address but a shortened length,
            // so that when we transmute, the length of the `value` field is correct
            let shortened_ptr = &mut bytes[0..value_length];
            // TODO: ensure this works on all architectures
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

    // TODO: make key methods return an Option<&[u8]>

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
            // 7,7,7,7,7, 7,7,7,7,7
        ];
        {
            let mut node = Node::from_bytes(&mut bytes).unwrap();

            println!("{:?}", node.hash());
            println!("{:?}", node.kv_hash());
            println!("{:?}", &node.left_key()[..]);
            println!("{:?}", node.left_height());
            *node.right_height_mut() += 10;
            println!("{:?}", node.right_height());
            println!("{:?}", &node.value()[..]);
        }
        println!("{:?}", &bytes[..]);
    }
}
