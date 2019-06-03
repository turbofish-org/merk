#![feature(test)]

extern crate rand;
extern crate test;

use merk::*;
use rand::prelude::*;

#[bench]
fn bench_put_insert_unchecked(b: &mut test::Bencher) {
    let mut merk = Merk::open("./test_merk_bench_put_insert.db").unwrap();

    let mut i = 0;
    b.iter(|| {
        let mut keys = vec![];
        for j in 0..2_000 {
            let n = i as u128 + (j * 100) as u128;
            keys.push(n.to_be_bytes());
        }

        let value = [123 as u8; 40];

        let mut batch: Vec<TreeBatchEntry> = vec![];
        for key in keys.iter() {
            batch.push((&key[..], TreeOp::Put(&value)));
        }

        merk.apply(&batch).unwrap();

        i += 1;
    });

    println!("final tree size: {}", i * 10_000);

    merk.destroy().unwrap();
}

#[bench]
fn bench_put_update_unchecked(b: &mut test::Bencher) {
    let mut merk = Merk::open("./test_merk_bench_put_update.db").unwrap();

    let mut rng = rand::thread_rng();
    let value = random_bytes(&mut rng, 40);

    let mut i = 0;
    b.iter(|| {
        let mut keys = vec![];
        for j in 0..2_000 {
            let n = (i % 100) as u128 + (j * 100) as u128;
            keys.push(n.to_be_bytes());
        }

        let mut batch: Vec<TreeBatchEntry> = vec![];
        for key in keys.iter() {
            batch.push((&key[..], TreeOp::Put(&value)));
        }

        merk.apply_unchecked(&batch).unwrap();

        i += 1;
    });

    println!("height: {}", merk.tree.as_ref().unwrap().height());

    merk.destroy().unwrap();
}

fn random_bytes(rng: &mut ThreadRng, length: usize) -> Vec<u8> {
    (0..length).map(|_| -> u8 { rng.gen() }).collect()
}
