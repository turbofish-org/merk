#![feature(test)]

extern crate test;

use test::Bencher;
use merk::test_utils::*;
use merk::tree::*;
use merk::proofs::*;

#[bench]
fn proof_1m_1_memonly(b: &mut Bencher) {
    proof_memonly(b, 1_000_000, 1);
}

#[bench]
fn proof_1m_16_memonly(b: &mut Bencher) {
    proof_memonly(b, 1_000_000, 16);
}

#[bench]
fn proof_1m_256_memonly(b: &mut Bencher) {
    proof_memonly(b, 1_000_000, 256);
}

#[bench]
fn verify_present_1m_1_memonly(b: &mut Bencher) {
    verify_present_bench(b, 1_000_000, 1);
}

#[bench]
fn verify_present_1m_16_memonly(b: &mut Bencher) {
    verify_present_bench(b, 1_000_000, 16);
}

#[bench]
fn verify_present_1m_256_memonly(b: &mut Bencher) {
    verify_present_bench(b, 1_000_000, 256);
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

fn verify_present_bench(b: &mut Bencher, tree_size: u64, proof_size: u64) {
    let batch_size = 10_000;
    let seed = 59421441857 * proof_size;
    let mut tree = make_tree_rand(tree_size, batch_size, seed);
    let mut walker = RefWalker::new(&mut tree, PanicSource {});

    let mut keys_and_proofs = vec![];
    for i in 0..10 {
        let batch = make_batch_rand(proof_size, seed + i);
        let mut keys = Vec::with_capacity(proof_size as usize);
        for (key, _) in batch {
            keys.push(key);
        }
        let (proof, absence) = walker.create_proof(keys.as_slice())
            .expect("create_proof errored");

        let mut bytes = vec![];
        encode_into(proof.iter(), &mut bytes);
        
        keys_and_proofs.push((keys, bytes));
    }

    let mut i = 0;
    b.iter(|| {
        let (keys, bytes) = &keys_and_proofs[i];
        verify(bytes.as_slice(), keys.as_slice(), [0; 20]);
        i = (i + 1) % keys_and_proofs.len();
    });
}
