#![feature(test)]

extern crate test;

use test::Bencher;
use merk::*;
use merk::test_utils::{
    make_tree_seq,
    make_batch_seq,
    apply_memonly_unchecked
};
use merk::tree::Owner;

#[bench]
fn insert_10k_seq_nowrite_noprune(b: &mut Bencher) {
    let mut tree = Owner::new(make_tree_seq(1_000_000));

    let batch_size = 10_000;

    let mut i = 0;
    b.iter(|| {
        let batch = make_batch_seq((i * batch_size)..((i+1) * batch_size));
        tree.own(|tree| (apply_memonly_unchecked(tree, &batch), 0));
        i += 1;
    });
}
