#![feature(test)]

extern crate test;
extern crate rand;

use rand::prelude::*;
use merk::*;

#[bench]
fn bench_batch_put(b: &mut test::Bencher) {
    let mut rng = rand::thread_rng();

    let mut tree = SparseTree::new(
        Node::new(b"0", b"x")
    );

    let mut i = 0;
    b.iter(|| {
        let mut keys = vec![];
        for i in 0..10_000 {
            keys.push(random_bytes(&mut rng, 4));
        }

        let mut batch: Vec<(&[u8], &[u8])> = vec![];
        for i in 0..10_000 {
            batch.push((&keys[i], b"x"));
        }

        tree.put_batch(
            // we build from scratch in this test, so we never call get_node
            &mut |link| unreachable!(),
            &batch[..]
        );
        i += 1;
    });
    println!("final tree size: {}", i * 10_000);
}

fn random_bytes(rng: &mut ThreadRng, length: usize) -> Vec<u8> {
    (0..length)
        .map(|_| -> u8 { rng.gen() })
        .collect()
}
