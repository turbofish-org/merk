#![feature(test)]

extern crate test;

use std::thread;
use test::Bencher;
use merk::test_utils::*;

#[bench]
fn insert_1m_2k_seq_rocksdb_noprune(b: &mut Bencher) {
    let initial_size = 1_000_000;
    let batch_size = 2_000;

    let path = thread::current().name().unwrap().to_owned();
    let mut merk = TempMerk::open(path).expect("failed to open merk");

    for i in 0..(initial_size / batch_size) {
        let batch = make_batch_seq((i * batch_size)..((i+1) * batch_size));
        merk.apply_unchecked(&batch).expect("apply failed");
    }

    let mut i = initial_size / batch_size;
    b.iter(|| {
        let batch = make_batch_seq((i * batch_size)..((i+1) * batch_size));
        merk.apply_unchecked(&batch).expect("apply failed");
        i += 1;
    });
}

#[bench]
fn insert_1m_2k_rand_rocksdb_noprune(b: &mut Bencher) {
    let initial_size = 1_000_000;
    let batch_size = 2_000;

    let path = thread::current().name().unwrap().to_owned();
    let mut merk = TempMerk::open(path).expect("failed to open merk");

    for i in 0..(initial_size / batch_size) {
        let batch = make_batch_rand(batch_size, i);
        merk.apply_unchecked(&batch).expect("apply failed");
    }

    let mut i = initial_size / batch_size;
    b.iter(|| {
        let batch = make_batch_rand(batch_size, i);
        merk.apply_unchecked(&batch).expect("apply failed");
        i += 1;
    });
}

#[bench]
fn update_1m_2k_seq_rocksdb_noprune(b: &mut Bencher) {
    let initial_size = 1_000_000;
    let batch_size = 2_000;

    let path = thread::current().name().unwrap().to_owned();
    let mut merk = TempMerk::open(path).expect("failed to open merk");

    for i in 0..(initial_size / batch_size) {
        let batch = make_batch_seq((i * batch_size)..((i+1) * batch_size));
        merk.apply_unchecked(&batch).expect("apply failed");
    }

    let mut i = 0;
    b.iter(|| {
        let batch = make_batch_seq((i * batch_size)..((i+1) * batch_size));
        merk.apply_unchecked(&batch).expect("apply failed");
        i = (i + 1) % (initial_size / batch_size);
    });
}

#[bench]
fn update_1m_2k_rand_rocksdb_noprune(b: &mut Bencher) {
    let initial_size = 1_000_000;
    let batch_size = 2_000;

    let path = thread::current().name().unwrap().to_owned();
    let mut merk = TempMerk::open(path).expect("failed to open merk");

    for i in 0..(initial_size / batch_size) {
        let batch = make_batch_rand(batch_size, i);
        merk.apply_unchecked(&batch).expect("apply failed");
    }

    let mut i = 0;
    b.iter(|| {
        let batch = make_batch_rand(batch_size, i);
        merk.apply_unchecked(&batch).expect("apply failed");
        i = (i + 1) % (initial_size / batch_size);
    });
}

#[bench]
fn delete_1m_2k_rand_rocksdb_noprune(b: &mut Bencher) {
    let initial_size = 1_000_000;
    let batch_size = 2_000;

    let path = thread::current().name().unwrap().to_owned();
    let mut merk = TempMerk::open(path).expect("failed to open merk");

    for i in 0..(initial_size / batch_size) {
        let batch = make_batch_rand(batch_size, i);
        merk.apply_unchecked(&batch).expect("apply failed");
    }

    let mut i = 0;
    b.iter(|| {
        if i >= (initial_size / batch_size) {
            println!("WARNING: too many bench iterations, whole tree deleted");
            return;
        }
        let batch = make_del_batch_rand(batch_size, i);
        merk.apply_unchecked(&batch).expect("apply failed");
        i = (i + 1) % (initial_size / batch_size);
    });
}
