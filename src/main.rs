extern crate rand;
extern crate rocksdb;
extern crate time;

use rand::prelude::*;

const NODES: u64 = 10000000;
const KEY_LENGTH: usize = 30;
const DATA_LENGTH: usize = 200;

macro_rules! benchmark {
    ( $name:expr , $n:expr , $code:block ) => {
        let start = time::precise_time_ns();

        for i in 0..$n {
            $code
        }

        let elapsed = time::precise_time_ns() - start;
        let ops_per_s = $n as f64 / (elapsed as f64 / 1e9);

        println!("{:?}: {:?} ops/s", $name, ops_per_s as u64);
    };
}

fn main() {
    let mut rng = rand::thread_rng();
    let db = rocksdb::DB::open_default("temp.db").unwrap();

    let mut write_options = rocksdb::WriteOptions::default();
    // write_options.set_sync(false);
    // write_options.disable_wal(true);

    // let mut batch = rocksdb::WriteBatch::default();

    // benchmark!("write random keys to batch", NODES, {
    //     let key = random_bytes(&mut rng, KEY_LENGTH);
    //     let value = random_bytes(&mut rng, DATA_LENGTH);
    //     (&mut batch).put(key, value).unwrap();
    // });
    //
    // let start = time::precise_time_s();
    // db.write(batch).unwrap();
    // let elapsed = time::precise_time_s() - start;
    // println!("write batch: {:?}s", elapsed);
    //
    // let start = time::precise_time_s();
    // db.flush().unwrap();
    // let elapsed = time::precise_time_s() - start;
    // println!("flush: {:?}s", elapsed);

    benchmark!("write random sequential keys (batches of 15,000)", 20, {
        let mut batch = rocksdb::WriteBatch::default();
        let mut key = random_bytes(&mut rng, KEY_LENGTH);
        let value = random_bytes(&mut rng, DATA_LENGTH);
        for i in 0..15000 {
            batch.put(&key, &value).unwrap();
            (&mut key)[KEY_LENGTH - 1] += 1;
        }
        db.write_opt(batch, &write_options).unwrap();
    });

    let start = time::precise_time_s();
    db.flush().unwrap();
    let elapsed = time::precise_time_s() - start;
    println!("flush: {:?}s", elapsed);

    let mut keys = [[0 as u8; KEY_LENGTH]; 10000];
    for i in 0..10000 {
        let start = random_bytes(&mut rng, KEY_LENGTH);
        let iter = db.iterator(rocksdb::IteratorMode::From(
            start.as_slice(),
            rocksdb::Direction::Forward,
        ));
        for (key, _value) in iter {
            keys[i].clone_from_slice(&key);
            break;
        }
    }

    let range_benchmark = |range_size, keys: &[[u8; KEY_LENGTH]; 10000]| {
        let mut i = 0;
        benchmark!(
            format!(
                "read random ranges ({}-node, {}B chunks)",
                range_size,
                (KEY_LENGTH + DATA_LENGTH) as u64 * range_size
            ),
            5000,
            {
                let key = keys[i];
                i += 1;
                let iter = db.iterator(rocksdb::IteratorMode::From(
                    &key[..],
                    rocksdb::Direction::Forward,
                ));
                let mut j = 0;
                for (_key, _value) in iter {
                    j += 1;
                    if j == range_size {
                        break;
                    };
                }
            }
        );
    };

    let mut i = 0;
    benchmark!("read random keys", 10000, {
        let key = keys[i];
        i += 1;
        db.get(key);
    });

    for i in 0..15 {
        range_benchmark(2_u64.pow(i), &keys);
    }
}

fn random_bytes(rng: &mut ThreadRng, length: usize) -> Vec<u8> {
    (0..length).map(|_| -> u8 { rng.gen() }).collect()
}
