#![feature(test)]

extern crate test;

use merk::proofs::encode_into as encode_proof_into;
use merk::restore::Restorer;
use merk::test_utils::*;
use merk::{Merk, Result};
use rand::prelude::*;
use std::thread;
use test::Bencher;

#[bench]
fn get_1m_rocksdb(b: &mut Bencher) {
    let initial_size = 1_000_000;
    let batch_size = 2_000;
    let num_batches = initial_size / batch_size;

    let path = thread::current().name().unwrap().to_owned();
    let mut merk = TempMerk::open(path).expect("failed to open merk");

    let mut batches = vec![];
    for i in 0..num_batches {
        let batch = make_batch_rand(batch_size, i);
        unsafe { merk.apply_unchecked(&batch, &[]).expect("apply failed") };
        batches.push(batch);
    }

    let mut i = 0;
    b.iter(|| {
        let batch_index = (i % num_batches) as usize;
        let key_index = (i / num_batches) as usize;

        let key = &batches[batch_index][key_index].0;
        merk.get(key).expect("get failed");

        i = (i + 1) % initial_size;
    });
}

#[bench]
fn insert_1m_2k_seq_rocksdb_noprune(b: &mut Bencher) {
    let initial_size = 1_000_000;
    let batch_size = 2_000;

    let path = thread::current().name().unwrap().to_owned();
    let mut merk = TempMerk::open(path).expect("failed to open merk");

    for i in 0..(initial_size / batch_size) {
        let batch = make_batch_seq((i * batch_size)..((i + 1) * batch_size));
        unsafe { merk.apply_unchecked(&batch, &[]).expect("apply failed") };
    }

    let mut i = initial_size / batch_size;
    b.iter(|| {
        let batch = make_batch_seq((i * batch_size)..((i + 1) * batch_size));
        unsafe { merk.apply_unchecked(&batch, &[]).expect("apply failed") };
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
        unsafe { merk.apply_unchecked(&batch, &[]).expect("apply failed") };
    }

    let mut i = initial_size / batch_size;
    b.iter(|| {
        let batch = make_batch_rand(batch_size, i);
        unsafe { merk.apply_unchecked(&batch, &[]).expect("apply failed") };
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
        let batch = make_batch_seq((i * batch_size)..((i + 1) * batch_size));
        unsafe { merk.apply_unchecked(&batch, &[]).expect("apply failed") };
    }

    let mut i = 0;
    b.iter(|| {
        let batch = make_batch_seq((i * batch_size)..((i + 1) * batch_size));
        unsafe { merk.apply_unchecked(&batch, &[]).expect("apply failed") };
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
        unsafe { merk.apply_unchecked(&batch, &[]).expect("apply failed") };
    }

    let mut i = 0;
    b.iter(|| {
        let batch = make_batch_rand(batch_size, i);
        unsafe { merk.apply_unchecked(&batch, &[]).expect("apply failed") };
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
        unsafe { merk.apply_unchecked(&batch, &[]).expect("apply failed") };
    }

    let mut i = 0;
    b.iter(|| {
        if i >= (initial_size / batch_size) {
            println!("WARNING: too many bench iterations, whole tree deleted");
            return;
        }
        let batch = make_del_batch_rand(batch_size, i);
        unsafe { merk.apply_unchecked(&batch, &[]).expect("apply failed") };
        i = (i + 1) % (initial_size / batch_size);
    });
}

#[bench]
fn prove_1m_1_rand_rocksdb_noprune(b: &mut Bencher) {
    let initial_size = 1_000_000;
    let batch_size = 1_000;
    let proof_size = 1;

    let path = thread::current().name().unwrap().to_owned();
    let mut merk = TempMerk::open(path).expect("failed to open merk");

    for i in 0..(initial_size / batch_size) {
        let batch = make_batch_rand(batch_size, i);
        unsafe { merk.apply_unchecked(&batch, &[]).expect("apply failed") };
    }

    let mut i = 0;
    b.iter(|| {
        let batch = make_batch_rand(proof_size, i);
        let mut keys = Vec::with_capacity(batch.len());
        for (key, _) in batch {
            keys.push(merk::proofs::query::QueryItem::Key(key));
        }
        merk.prove_unchecked(keys).expect("prove failed");
        i = (i + 1) % (initial_size / batch_size);

        merk.commit(std::collections::LinkedList::new(), &[])
            .unwrap();
    });
}

#[bench]
fn build_trunk_chunk_1m_1_rand_rocksdb_noprune(b: &mut Bencher) {
    let initial_size = 1_000_000;
    let batch_size = 1_000;

    let path = thread::current().name().unwrap().to_owned();
    let mut merk = TempMerk::open(path).expect("failed to open merk");

    for i in 0..(initial_size / batch_size) {
        let batch = make_batch_rand(batch_size, i);
        unsafe { merk.apply_unchecked(&batch, &[]).expect("apply failed") };
    }

    let mut bytes = vec![];

    b.iter(|| {
        bytes.clear();

        let (ops, _) = merk.walk(|walker| walker.unwrap().create_trunk_proof().unwrap());
        encode_proof_into(ops.iter(), &mut bytes);

        merk.commit(std::collections::LinkedList::new(), &[])
            .unwrap();
    });

    b.bytes = bytes.len() as u64;
}

#[bench]
fn chunkproducer_rand_1m_1_rand_rocksdb_noprune(b: &mut Bencher) {
    let mut rng = rand::thread_rng();

    let initial_size = 1_000_000;
    let batch_size = 1_000;

    let path = thread::current().name().unwrap().to_owned();
    let mut merk = TempMerk::open(path).expect("failed to open merk");

    for i in 0..(initial_size / batch_size) {
        let batch = make_batch_rand(batch_size, i);
        unsafe { merk.apply_unchecked(&batch, &[]).expect("apply failed") };
    }

    let mut chunks = merk.chunks().unwrap();
    let mut total_bytes = 0;
    let mut i = 0;

    let mut next = || {
        let index = rng.gen::<usize>() % chunks.len();
        chunks.chunk(index).unwrap()
    };

    b.iter(|| {
        let chunk = next();
        total_bytes += chunk.len();
        i += 1;
    });

    b.bytes = (total_bytes / i) as u64;
}

#[bench]
fn chunk_iter_1m_1_rand_rocksdb_noprune(b: &mut Bencher) {
    let initial_size = 1_000_000;
    let batch_size = 1_000;

    let path = thread::current().name().unwrap().to_owned();
    let mut merk = TempMerk::open(path).expect("failed to open merk");

    for i in 0..(initial_size / batch_size) {
        let batch = make_batch_rand(batch_size, i);
        unsafe { merk.apply_unchecked(&batch, &[]).expect("apply failed") };
    }

    let mut chunks = merk.chunks().unwrap().into_iter();
    let mut total_bytes = 0;
    let mut i = 0;

    let mut next = || match chunks.next() {
        Some(chunk) => chunk,
        None => {
            chunks = merk.chunks().unwrap().into_iter();
            chunks.next().unwrap()
        }
    };

    b.iter(|| {
        let chunk = next();
        total_bytes += chunk.unwrap().len();
        i += 1;
    });

    b.bytes = (total_bytes / i) as u64;
}

#[bench]
fn restore_1m_1_rand_rocksdb_noprune(b: &mut Bencher) {
    let initial_size = 1_000_000;
    let batch_size = 1_000;

    let path = thread::current().name().unwrap().to_owned();
    let mut merk = TempMerk::open(path).expect("failed to open merk");

    for i in 0..(initial_size / batch_size) {
        let batch = make_batch_rand(batch_size, i);
        unsafe { merk.apply_unchecked(&batch, &[]).expect("apply failed") };
    }

    let chunks = merk
        .chunks()
        .unwrap()
        .into_iter()
        .collect::<Result<Vec<_>>>()
        .unwrap();

    let path = thread::current().name().unwrap().to_owned() + "_restore";
    let mut restorer: Option<Restorer> = None;

    let mut total_bytes = 0;
    let mut i = 0;

    b.iter(|| {
        if i % chunks.len() == 0 {
            if i != 0 {
                let restorer_merk = restorer.take().unwrap().finalize();
                drop(restorer_merk);
                std::fs::remove_dir_all(&path).unwrap();
            }

            restorer = Some(Merk::restore(&path, merk.root_hash(), chunks.len()).unwrap());
        }

        let restorer = restorer.as_mut().unwrap();
        let chunk = chunks[i % chunks.len()].as_slice();
        restorer.process_chunk(chunk).unwrap();

        total_bytes += chunk.len();
        i += 1;
    });

    std::fs::remove_dir_all(&path).unwrap();

    b.bytes = (total_bytes / i) as u64;
}

#[bench]
fn checkpoint_create_destroy_1m_1_rand_rocksdb_noprune(b: &mut Bencher) {
    let initial_size = 1_000_000;
    let batch_size = 1_000;

    let path = thread::current().name().unwrap().to_owned();
    let mut merk = TempMerk::open(&path).expect("failed to open merk");

    for i in 0..(initial_size / batch_size) {
        let batch = make_batch_rand(batch_size, i);
        unsafe { merk.apply_unchecked(&batch, &[]).expect("apply failed") };
    }

    let path = path + ".checkpoint";
    b.iter(|| {
        let checkpoint = merk.checkpoint(&path).unwrap();
        checkpoint.destroy().unwrap();
    });
}
