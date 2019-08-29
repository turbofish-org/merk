use std::convert::TryInto;

use super::{Op, Node};
use crate::tree::HASH_LENGTH;
use crate::error::Result;

// TODO: Encode, Decode traits

impl Op {
    pub fn encode_into(&self, output: &mut Vec<u8>) {
        match self {
            Op::Push(Node::Hash(hash)) => {
                output.push(0x01);
                output.extend(hash);
            },
            Op::Push(Node::KVHash(kv_hash)) => {
                output.push(0x02);
                output.extend(kv_hash);
            },
            Op::Push(Node::KV(key, value)) => {
                output.push(0x03);
                output.push(key.len().try_into().unwrap());
                output.extend(key);
                output.push((value.len() & 0xff).try_into().unwrap());
                output.push((value.len() >> 8).try_into().unwrap());
                output.extend(value);
            },
            Op::Parent => output.push(0x10),
            Op::Child => output.push(0x11)
        }
    }

    pub fn encoding_length(&self) -> usize {
        match self {
            Op::Push(Node::Hash(_)) => 1 + HASH_LENGTH,
            Op::Push(Node::KVHash(_)) => 1 + HASH_LENGTH,
            Op::Push(Node::KV(key, value)) => 4 + key.len() + value.len(),
            Op::Parent => 1,
            Op::Child => 1
        }
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        Ok(match bytes[0] {
            0x01 => {
                let mut hash = [0; HASH_LENGTH];
                hash.copy_from_slice(&bytes[1..HASH_LENGTH + 1]);
                Op::Push(Node::Hash(hash))
            },
            0x02 => {
                let mut hash = [0; HASH_LENGTH];
                hash.copy_from_slice(&bytes[1..HASH_LENGTH + 1]);
                Op::Push(Node::KVHash(hash))
            },
            0x03 => {
                let mut offset = 1;

                let key_len = bytes[offset] as usize;
                offset += 1;
                let key = bytes[offset..offset + key_len].to_vec();
                offset += key_len;

                let value_len =
                    bytes[offset] as usize
                    + ((bytes[offset + 1] as usize) << 8);
                offset += 2;
                let value = bytes[offset..offset + value_len].to_vec();
                // offset += value_len;

                Op::Push(Node::KV(key, value))
            },
            0x10 => Op::Parent,
            0x11 => Op::Child,
            _ => bail!("Proof has unexpected value")
        })
    }
}

pub fn encode_into<'a, T: Iterator<Item=&'a Op>>(ops: T, output: &mut Vec<u8>) {
    for op in ops {
        op.encode_into(output);
    }
}

pub fn encoding_length<'a, T: Iterator<Item=&'a Op>>(ops: T) -> usize {
    ops.map(|op| op.encoding_length()).sum()
}

#[cfg(test)]
mod test {
    use super::super::{Op, Node};
    use crate::tree::HASH_LENGTH;

    #[test]
    fn encode_push_hash() {
        let op = Op::Push(Node::Hash([123; HASH_LENGTH]));
        assert_eq!(op.encoding_length(), 1 + HASH_LENGTH);

        let mut bytes = vec![];
        op.encode_into(&mut bytes);
        assert_eq!(bytes, vec![0x01, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123]);
    }

    #[test]
    fn encode_push_kvhash() {
        let op = Op::Push(Node::KVHash([123; HASH_LENGTH]));
        assert_eq!(op.encoding_length(), 1 + HASH_LENGTH);

        let mut bytes = vec![];
        op.encode_into(&mut bytes);
        assert_eq!(bytes, vec![0x02, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123]);
    }

    #[test]
    fn encode_push_kv() {
        let op = Op::Push(Node::KV(vec![1, 2, 3], vec![4, 5, 6]));
        assert_eq!(op.encoding_length(), 10);

        let mut bytes = vec![];
        op.encode_into(&mut bytes);
        assert_eq!(bytes, vec![0x03, 3, 1, 2, 3, 3, 0, 4, 5, 6]);
    }

    #[test]
    fn encode_parent() {
        let op = Op::Parent;
        assert_eq!(op.encoding_length(), 1);

        let mut bytes = vec![];
        op.encode_into(&mut bytes);
        assert_eq!(bytes, vec![0x10]);
    }

    #[test]
    fn encode_child() {
        let op = Op::Child;
        assert_eq!(op.encoding_length(), 1);

        let mut bytes = vec![];
        op.encode_into(&mut bytes);
        assert_eq!(bytes, vec![0x11]);
    }

    #[test]
    #[should_panic]
    fn encode_push_kv_long_key() {
        let op = Op::Push(Node::KV(vec![123; 300], vec![4, 5, 6]));
        let mut bytes = vec![];
        op.encode_into(&mut bytes);
    }

    #[test]
    fn decode_push_hash() {
        let bytes = [0x01, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123];
        let op = Op::decode(&bytes[..]).expect("decode failed");
        assert_eq!(op, Op::Push(Node::Hash([123; HASH_LENGTH])));
    }

    #[test]
    fn decode_push_kvhash() {
        let bytes = [0x02, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123, 123];
        let op = Op::decode(&bytes[..]).expect("decode failed");
        assert_eq!(op, Op::Push(Node::KVHash([123; HASH_LENGTH])));
    }

    #[test]
    fn decode_push_kv() {
        let bytes = [0x03, 3, 1, 2, 3, 3, 0, 4, 5, 6];
        let op = Op::decode(&bytes[..]).expect("decode failed");
        assert_eq!(op, Op::Push(Node::KV(vec![1, 2, 3], vec![4, 5, 6])));
    }

    #[test]
    fn decode_parent() {
        let bytes = [0x10];
        let op = Op::decode(&bytes[..]).expect("decode failed");
        assert_eq!(op, Op::Parent);
    }

    #[test]
    fn decode_child() {
        let bytes = [0x11];
        let op = Op::decode(&bytes[..]).expect("decode failed");
        assert_eq!(op, Op::Child);
    }

    #[test]
    fn decode_unknown() {
        let bytes = [0x88];
        assert!(Op::decode(&bytes[..]).is_err());
    }
}
