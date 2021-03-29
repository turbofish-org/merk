#![feature(test)]

extern crate test;

use merk::owner::Owner;
use merk::test_utils::*;
use test::Bencher;

#[bench]
fn insert_1m_10k_seq_memonly(b: &mut Bencher) {
    let initial_size = 1_000_000;
    let batch_size = 10_000;

    let mut tree = Owner::new(make_tree_seq(initial_size));

    let mut i = initial_size / batch_size;
    b.iter(|| {
        let batch = make_batch_seq((i * batch_size)..((i + 1) * batch_size));
        tree.own(|tree| apply_memonly_unchecked(tree, &batch));
        i += 1;
    });
}

#[bench]
fn insert_1m_10k_rand_memonly(b: &mut Bencher) {
    let initial_size = 1_000_000;
    let batch_size = 10_000;

    let mut tree = Owner::new(make_tree_rand(initial_size, batch_size, 0));

    let mut i = initial_size / batch_size;
    b.iter(|| {
        let batch = make_batch_rand(batch_size, i);
        tree.own(|tree| apply_memonly_unchecked(tree, &batch));
        i += 1;
    });
}

#[bench]
fn update_1m_10k_seq_memonly(b: &mut Bencher) {
    let initial_size = 1_000_000;
    let batch_size = 10_000;

    let mut tree = Owner::new(make_tree_seq(initial_size));

    let mut i = 0;
    b.iter(|| {
        let batch = make_batch_seq((i * batch_size)..((i + 1) * batch_size));
        tree.own(|tree| apply_memonly_unchecked(tree, &batch));
        i = (i + 1) % (initial_size / batch_size);
    });
}

#[bench]
fn update_1m_10k_rand_memonly(b: &mut Bencher) {
    let initial_size = 1_010_000;
    let batch_size = 10_000;

    let mut tree = Owner::new(make_tree_rand(initial_size, batch_size, 0));

    let mut i = 0;
    b.iter(|| {
        let batch = make_batch_rand(batch_size, i);
        tree.own(|tree| apply_memonly_unchecked(tree, &batch));
        i = (i + 1) % (initial_size / batch_size);
    });
}
