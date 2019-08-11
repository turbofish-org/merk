#![feature(test)]

extern crate test;

use test::Bencher;
use merk::*;
use merk::test_utils::{
    make_tree_seq,
    make_batch_seq,
    make_batch_rand,
    apply_memonly_unchecked
};
use merk::tree::Owner;

#[bench]
fn insert_1m_10k_seq_memonly(b: &mut Bencher) {
    let mut tree = Owner::new(make_tree_seq(1_000_000));

    let batch_size = 10_000;

    let mut i = 0;
    b.iter(|| {
        let batch = make_batch_seq((i * batch_size)..((i+1) * batch_size));
        tree.own(|tree| (apply_memonly_unchecked(tree, &batch), 0));
        i += 1;
    });
}

#[bench]
fn insert_1m_10k_rand_memonly(b: &mut Bencher) {
    let mut tree = Owner::new(make_tree_seq(1_000_000));

    let batch_size = 10_000;

    let mut i = 0;
    b.iter(|| {
        let batch = make_batch_rand(batch_size, i);
        tree.own(|tree| (apply_memonly_unchecked(tree, &batch), 0));
        i += 1;
    });
}