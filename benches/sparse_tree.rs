#![feature(test)]

extern crate test;

use merk::*;

#[bench]
fn bench_batch_insert_2k(b: &mut test::Bencher) {
    let mut tree = Some(Box::new(
        SparseTree::new(Node::new(b"0", b"x"))
    ));

    let mut i = 0;
    b.iter(|| {
        let mut keys = vec![];
        for j in 0..2_000 {
            keys.push((i + (j * 301) as u64).to_be_bytes());
        }

        let mut batch: Vec<TreeBatchEntry> = vec![];
        for key in keys.iter() {
            batch.push((&key[..], TreeOp::Put(b"x")));
        }

        SparseTree::apply(
            &mut tree,
            // we build from scratch in this test, so we never call get_node
            &mut |_| unreachable!(),
            &batch[..],
        )
        .unwrap();
        i += 1;
    });
    println!("final tree size: {}", i * 2_000);
}

#[bench]
fn bench_batch_update_2k(b: &mut test::Bencher) {
    let mut tree = Some(Box::new(
        SparseTree::new(Node::new(b"0", b"x"))
    ));

    for i in 0..100 {
        let mut keys = vec![];
        for j in 0..2_000 {
            keys.push(((i * 2_000) + j as u64).to_be_bytes());
        }

        let mut batch: Vec<TreeBatchEntry> = vec![];
        for key in keys.iter() {
            batch.push((&key[..], TreeOp::Put(b"x")));
        }

        SparseTree::apply(
            &mut tree,
            // we build from scratch in this test, so we never call get_node
            &mut |_| unreachable!(),
            &batch[..],
        )
        .unwrap();
    }

    let mut i = 0;
    b.iter(|| {
        let mut keys = vec![];
        for j in 0..2_000 {
            keys.push(((i % 100) + (j * 100) as u64).to_be_bytes());
        }

        let mut batch: Vec<TreeBatchEntry> = vec![];
        for key in keys.iter() {
            batch.push((&key[..], TreeOp::Put(b"x")));
        }

        SparseTree::apply(
            &mut tree,
            // we build from scratch in this test, so we never call get_node
            &mut |_| unreachable!(),
            &batch[..],
        )
        .unwrap();
        i += 1;
    });
    println!("final tree size: {}", i * 2_000);
}

#[bench]
fn bench_batch_delete_2k(b: &mut test::Bencher) {
    let mut tree = Some(Box::new(
        SparseTree::new(Node::new(b"0", b"x"))
    ));

    for i in 0..1_000 {
        let mut keys = vec![];
        for j in 0..2_000 {
            keys.push((((i * 2_000) + j) as u64).to_be_bytes());
        }

        let mut batch: Vec<TreeBatchEntry> = vec![];
        for key in keys.iter() {
            batch.push((&key[..], TreeOp::Put(b"x")));
        }

        SparseTree::apply(
            &mut tree,
            // we build from scratch in this test, so we never call get_node
            &mut |_| unreachable!(),
            &batch[..],
        )
        .unwrap();
    }

    let mut i = 0;
    b.iter(|| {
        let mut keys = vec![];
        for j in 0..2_000 {
            keys.push(((i + (j * 1000)) as u64).to_be_bytes());
        }

        let mut batch: Vec<TreeBatchEntry> = vec![];
        for key in keys.iter() {
            batch.push((&key[..], TreeOp::Delete));
        }

        SparseTree::apply(
            &mut tree,
            // we build from scratch in this test, so we never call get_node
            &mut |_| unreachable!(),
            &batch[..],
        )
        .unwrap();
        i += 1;
    });
    println!("final tree size: {}", (1_000 - i) * 2_000);
}
