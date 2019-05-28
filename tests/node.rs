#![feature(test)]

extern crate test;

use merk::*;

macro_rules! assert_err {
    ($result:ident, $kind:path) => {
        match $result {
        Err(err) => {
            match err.kind() {
                $kind(ref _kind) => {},
                _ => panic!("Unexpected error kind")
            }
        },
        _ => panic!("Expected Err, got Ok")
    }
    };
}

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

#[test]
fn decode_empty() {
    let res = Node::decode(&[], &[]);
    assert_err!(res, ErrorKind::Bincode);
}

#[test]
fn decode_short() {
    let res = Node::decode(&[123], &[1, 2, 3, 4]);
    assert_err!(res, ErrorKind::Bincode);
}
