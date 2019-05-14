extern crate time;
extern crate rocksdb;
extern crate rand;

use rand::prelude::*;

const NODES: u64 = 10000000;
const KEY_LENGTH: usize = 30;
const DATA_LENGTH: usize = 200;

macro_rules! benchmark {
    ( $name:expr , $n:expr , $code:block ) => {
        let start = time::precise_time_ns();

        for _ in 0..$n {
            $code
        }

        let elapsed = time::precise_time_ns() - start;
        let ops_per_s = $n as f64 / (elapsed as f64 / 1e9);

        println!("{:?}: {:?} ops/s", $name, ops_per_s as u64);
    }
}

fn main() {
    let mut rng = rand::thread_rng();
    let db = rocksdb::DB::open_default("temp.db").unwrap();

    // benchmark!("write random keys", NODES, {
    //     let key = random_bytes(&mut rng, KEY_LENGTH);
    //     let value = random_bytes(&mut rng, DATA_LENGTH);
    //     db.put(key, value).unwrap();
    // });

    let mut range_benchmark = |range_size| {
        benchmark!(
            format!(
                "read random ranges ({}-node, {}B chunks)",
                range_size,
                (KEY_LENGTH + DATA_LENGTH) as u64 * range_size
            ),
            NODES / range_size,
            {
                let key = random_bytes(&mut rng, KEY_LENGTH);
                let iter = db.iterator(
                    rocksdb::IteratorMode::From(
                        key.as_slice(),
                        rocksdb::Direction::Forward
                    )
                );
                let mut i = 0;
                for (_key, _value) in iter {
                    i += 1;
                    if i == range_size { break };
                }
            }
        );
    };

    for i in 5..15 {
        range_benchmark(2_u64.pow(i));
    }
}

fn random_bytes(rng: &mut ThreadRng, length: usize) -> Vec<u8> {
    (0..length)
        .map(|_| -> u8 { rng.gen() })
        .collect()
}
