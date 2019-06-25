use std::cmp::max;
use std::fmt;

use crate::error::*;

use super::hash::*;

/// Represents a reference to another tree node.
///
/// Note that the referenced node is not necessarily
/// loaded in memory, but can be fetched from a backing
/// database by its key.
#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct Link {
    pub key: Vec<u8>,
    pub hash: Hash,
    pub height: u8,
}

impl fmt::Debug for Link {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "(key={}, hash={}, height={})",
            String::from_utf8(self.key.to_vec()).unwrap(),
            hex::encode(&self.hash[0..3]),
            self.height
        )
    }
}

// TODO: enforce maximum key/value lengths, to prevent DoS (e.g. when verifying
// a proof)

/// Represents a tree node and its associated key/value pair.
#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct Node {
    // don't serialize key since it's implied from the db
    #[serde(skip)]
    pub key: Vec<u8>,
    pub value: Vec<u8>,
    pub kv_hash: Hash,
    pub left: Option<Link>,
    pub right: Option<Link>
}

/// Replaces the value of a `Vec<T>` by cloning into it,
/// possibly not needing to allocate.
#[inline]
fn set_vec<T: Clone>(dest: &mut Vec<T>, src: &[T]) {
    dest.clear();
    dest.extend_from_slice(src);
}

///
impl Node {
    /// Creates a new node from a key and value.
    pub fn new(key: &[u8], value: &[u8]) -> Node {
        let mut node = Node {
            key: key.to_vec(),
            value: value.to_vec(),
            kv_hash: Default::default(),
            left: None,
            right: None,
        };
        node.update_kv_hash();
        node
    }

    pub fn decode(key: &[u8], bytes: &[u8]) -> Result<Node> {
        let node = bincode::deserialize(bytes).map(|mut node: Node| {
            set_vec(&mut node.key, key);
            node
        })?;
        Ok(node)
    }

    pub fn update_kv_hash(&mut self) {
        self.kv_hash = kv_hash(&self.key, &self.value);
    }

    pub fn hash(&self) -> Hash {
       hash(
           &self.kv_hash,
           self.left.as_ref().map(|l| &l.hash),
           self.right.as_ref().map(|r| &r.hash)
        ) 
    }

    #[inline]
    pub fn child_link(&self, left: bool) -> &Option<Link> {
        if left {
            &self.left
        } else {
            &self.right
        }
    }

    pub fn child_height(&self, left: bool) -> u8 {
        let link = self.child_link(left);
        match link {
            Some(link) => link.height,
            None => 0,
        }
    }

    pub fn height(&self) -> u8 {
        max(self.child_height(true), self.child_height(false)) + 1
    }

    pub fn balance_factor(&self) -> i8 {
        self.child_height(false) as i8 - self.child_height(true) as i8
    }

    #[inline]
    pub fn as_link(&self) -> Link {
        Link {
            key: self.key.to_vec(),
            hash: self.hash(),
            height: self.height(),
        }
    }
    
    #[inline]
    pub fn set_child(&mut self, left: bool, link: Option<Link>) {
        if left {
            self.left = link;
        } else {
            self.right = link;
        }
    }

    #[inline]
    pub fn set_value(&mut self, value: &[u8]) {
        set_vec(&mut self.value, value);
        self.update_kv_hash();
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        let bytes = bincode::serialize(&self)?;
        Ok(bytes)
    }

    #[inline]
    pub fn is_leaf(&self) -> bool {
        !(self.left.is_some() || self.right.is_some())
    }
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "({:?}: {:?}, hash={}, height={})",
            String::from_utf8(self.key.to_vec())
                .unwrap_or_else(|_| format!("{:?}", &self.key)),
            String::from_utf8(self.value.to_vec())
                .unwrap_or_else(|_| format!("{:?}", &self.value)),
            hex::encode(&self.hash()[..3]),
            self.height()
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::test_util;

    use super::*;

    #[test]
    fn constructor() {
        let node = Node::new(b"foo", b"bar");

        assert_eq!(
            node.key, b"foo",
            "key should be set"
        );
        assert_eq!(
            node.value, b"bar",
            "value should be set"
        );
        assert_eq!(
            &node.kv_hash[..],
            [6, 133, 157, 221, 98, 163, 219, 49, 224, 197, 121, 136, 24, 170, 250, 130, 228, 3, 124, 144],
            "kv_hash should be set"
        );
    }

    #[should_panic]
    #[test]
    fn long_key() {
        Node::new(&[0; 1_000], &[1, 2, 3]);
    }

    #[should_panic]
    #[test]
    fn long_value() {
        Node::new(&[1, 2, 3], &[0; 100_000]);
    }

    #[test]
    fn decode_empty() {
        let res = Node::decode(&[123], &[]);
        assert_err!(res, ErrorKind::Bincode);
    }

    #[test]
    fn decode_short() {
        let res = Node::decode(&[123], &[1, 2, 3, 4]);
        assert_err!(res, ErrorKind::Bincode);
    }

    #[test]
    fn codec_simple() {
        let node = Node::new(&[1, 2, 3], b"123");
        let bytes = node.encode().unwrap();

        let decoded = Node::decode(&[1, 2, 3], &bytes).unwrap();
        assert_eq!(node, decoded);

        let bytes2 = decoded.encode().unwrap();
        assert_eq!(bytes, bytes2);
    }

    #[test]
    fn kv_hash_fixtures() {
        assert_eq!(
            Node::new(&[], &[]).kv_hash,
            [93, 183, 219, 10, 45, 136, 39, 149, 47, 220, 190, 248, 229, 184, 205, 165, 28, 95, 198, 187]
        );
        assert_eq!(
            Node::new(&[1, 2, 3], &[4, 5, 6]).kv_hash,
            [44, 207, 168, 83, 90, 164, 155, 178, 18, 239, 152, 87, 31, 209, 217, 222, 50, 228, 52, 242]
        );
        assert_eq!(
            Node::new(&[88; 200], &[123; 5000]).kv_hash,
            [59, 124, 231, 30, 226, 8, 242, 209, 173, 112, 105, 236, 21, 146, 13, 123, 160, 35, 156, 6]
        );
    }

    #[test]
    fn hash_fixtures() {
        let mut a = Node::new(&[1, 2, 3], &[4, 5, 6]);
        let mut b = Node::new(&[1, 2, 4], &[4, 5, 7]);
        let c = Node::new(&[1, 2, 5], &[4, 5, 8]);
        let d = Node::new(&[1, 2, 6], &[4, 5, 9]);

        assert_eq!(
            a.hash(),
            [20, 57, 42, 92, 132, 74, 251, 239, 41, 140, 17, 75, 169, 244, 8, 5, 253, 187, 94, 106]
        );

        a.set_child(true, Some(b.as_link()));
        assert_eq!(
            a.hash(),
            [229, 187, 159, 168, 109, 230, 97, 94, 213, 21, 214, 253, 223, 222, 177, 245, 38, 175, 203, 70]
        );

        a.set_child(false, Some(c.as_link()));
        assert_eq!(
            a.hash(),
            [228, 32, 11, 176, 191, 201, 49, 47, 91, 52, 98, 14, 198, 67, 238, 255, 109, 150, 147, 110]
        );

        b.set_child(true, Some(d.as_link()));
        a.set_child(true, Some(b.as_link()));
        assert_eq!(
            a.hash(),
            [173, 219, 117, 240, 216, 171, 145, 136, 131, 101, 43, 231, 87, 153, 71, 46, 168, 179, 204, 98]
        );
    }

    #[test]
    fn link_fixtures() {
        let mut a = Node::new(&[1, 2, 3], &[4, 5, 6]);
        let mut b = Node::new(&[1, 2, 4], &[4, 5, 7]);
        let c = Node::new(&[1, 2, 5], &[4, 5, 8]);
        let d = Node::new(&[1, 2, 6], &[4, 5, 9]);

        assert_eq!(
            a.as_link(),
            Link {
                key: vec![1, 2, 3],
                hash: [20, 57, 42, 92, 132, 74, 251, 239, 41, 140, 17, 75, 169, 244, 8, 5, 253, 187, 94, 106],
                height: 1
            }
        );

        a.set_child(true, Some(b.as_link()));
        assert_eq!(
            a.as_link(),
            Link {
                key: vec![1, 2, 3],
                hash: [229, 187, 159, 168, 109, 230, 97, 94, 213, 21, 214, 253, 223, 222, 177, 245, 38, 175, 203, 70],
                height: 2
            }
        );

        a.set_child(false, Some(c.as_link()));
        assert_eq!(
            a.as_link(),
            Link {
                key: vec![1, 2, 3],
                hash: [228, 32, 11, 176, 191, 201, 49, 47, 91, 52, 98, 14, 198, 67, 238, 255, 109, 150, 147, 110],
                height: 2
            }
        );

        b.set_child(true, Some(d.as_link()));
        a.set_child(true, Some(b.as_link()));
        assert_eq!(
            a.as_link(),
            Link {
                key: vec![1, 2, 3],
                hash: [173, 219, 117, 240, 216, 171, 145, 136, 131, 101, 43, 231, 87, 153, 71, 46, 168, 179, 204, 98],
                height: 3
            }
        );
    }

    #[test]
    fn height_and_balance() {
        let mut node = Node::new(b"abc", b"def");
        assert_eq!(node.height(), 1);
        assert_eq!(node.balance_factor(), 0);

        // left child with height of 1
        node.set_child(true, Some(Link {
            key: vec![],
            hash: [0; HASH_LENGTH],
            height: 1
        }));
        assert_eq!(node.height(), 2);
        assert_eq!(node.balance_factor(), -1);

        // two children, each with height of 1
        node.set_child(false, Some(Link {
            key: vec![],
            hash: [0; HASH_LENGTH],
            height: 1
        }));
        assert_eq!(node.height(), 2);
        assert_eq!(node.balance_factor(), 0);

        // left child has height 2, right has height 1
        node.set_child(true, Some(Link {
            key: vec![],
            hash: [0; HASH_LENGTH],
            height: 2
        }));
        assert_eq!(node.height(), 3);
        assert_eq!(node.balance_factor(), -1);

        // left has height 2, right child has height 20
        node.set_child(false, Some(Link {
            key: vec![],
            hash: [0; HASH_LENGTH],
            height: 20
        }));
        assert_eq!(node.height(), 21);
        assert_eq!(node.balance_factor(), 18);

        // left has height 2, no right child
        node.set_child(false, None);
        assert_eq!(node.height(), 3);
        assert_eq!(node.balance_factor(), -2);
    }

    #[test]
    fn update_value() {
        let mut node = Node::new(b"abc", b"123");
        let original_kvh = node.kv_hash;

        node.set_value(b"456");
        assert_eq!(node.value, b"456");
        assert_eq!(node.key, b"abc");

        // ensure kv_hash is updated
        assert_ne!(node.kv_hash, original_kvh);
    }
}