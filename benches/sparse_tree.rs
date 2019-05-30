#![feature(test)]

extern crate rand;
extern crate test;

use merk::*;
use rand::prelude::*;

#[bench]
fn bench_batch_put(b: &mut test::Bencher) {
    let mut rng = rand::thread_rng();

    let mut tree = Some(Box::new(
        SparseTree::new(Node::new(b"0", b"x"))
    ));

    let mut i = 0;
    b.iter(|| {
        let mut keys = vec![];
        for _ in 0..2_000 {
            keys.push(random_bytes(&mut rng, 4));
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

fn random_bytes(rng: &mut ThreadRng, length: usize) -> Vec<u8> {
    (0..length).map(|_| -> u8 { rng.gen() }).collect()
}
