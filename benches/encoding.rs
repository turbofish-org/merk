#![feature(test)]

extern crate test;

use std::thread;
use test::Bencher;
use merk::test_utils::*;
use merk::tree::Tree;

#[bench]
fn tree_encode_into(b: &mut Bencher) {
    let mut buf = Vec::with_capacity(256);
    let tree = make_tree_seq(3);

    b.iter(|| tree.encode_into(&mut buf));
}

#[bench]
fn tree_decode(b: &mut Bencher) {
    let mut buf = Vec::with_capacity(256);
    let tree = make_tree_seq(3);
    tree.encode_into(&mut buf);

    b.iter(|| Tree::decode(&[0], buf.as_slice()).expect("decode failed"));
}
