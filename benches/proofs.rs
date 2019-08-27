#![feature(test)]

extern crate test;

use test::Bencher;
use merk::test_utils::*;
use merk::tree::*;

#[bench]
fn proof_1m_1_memonly(b: &mut Bencher) {
    proof_memonly(b, 1_000_000, 1);
}

#[bench]
fn proof_1m_4_memonly(b: &mut Bencher) {
    proof_memonly(b, 1_000_000, 4);
}

#[bench]
fn proof_1m_16_memonly(b: &mut Bencher) {
    proof_memonly(b, 1_000_000, 16);
}

#[bench]
fn proof_1m_64_memonly(b: &mut Bencher) {
    proof_memonly(b, 1_000_000, 64);
}

#[bench]
fn proof_1m_256_memonly(b: &mut Bencher) {
    proof_memonly(b, 1_000_000, 256);
}

#[bench]
fn proof_1m_1024_memonly(b: &mut Bencher) {
    proof_memonly(b, 1_000_000, 1024);
}

fn proof_memonly(b: &mut Bencher, tree_size: u64, proof_size: u64) {
    let batch_size = 10_000;
    let seed = 59421441857 * proof_size;
    let mut tree = make_tree_rand(tree_size, batch_size, seed);
    let mut walker = RefWalker::new(&mut tree, PanicSource {});

    let mut i = 0;
    b.iter(|| {
        let batch = make_batch_rand(proof_size, seed + i);
        let mut keys = Vec::with_capacity(proof_size as usize);
        for (key, _) in batch {
            keys.push(key);
        }
        let (proof, absence) = walker.create_proof(keys.as_slice())
            .expect("create_proof errored");
        i = (i + 1) % (tree_size / batch_size);
    });
}
