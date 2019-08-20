#![feature(drain_filter)]

use std::convert::TryInto;
use rand::prelude::*;
use merk::tree::*;
use merk::test_utils::*;

const ITERATIONS: usize = 1_000;

#[test]
fn fuzz() {
    let mut rng = thread_rng();

    for i in 0..ITERATIONS {
        println!("i:{}", i);
        let initial_size = rng.gen::<u64>() % 32;
        let seed = rng.gen::<u64>();
        let mut maybe_tree = Some(make_tree_rand(initial_size, initial_size, seed));

        for j in 0..4 {
            let batch_size = rng.gen::<u64>() % 8;
            println!("j:{} {}", j, batch_size);
            let batch = make_batch(maybe_tree.as_ref(), batch_size);
            println!("applying");
            maybe_tree = apply_to_memonly(maybe_tree, &batch);
        }
    }
}


pub fn make_batch(maybe_tree: Option<&Tree>, size: u64) -> Vec<BatchEntry> {
    let mut batch = Vec::with_capacity(size.try_into().unwrap());

    let get_random_key = || {
        let mut rng = thread_rng();
        let tree = maybe_tree.as_ref().unwrap();
        let entries: Vec<_> = tree.iter().collect();
        let index = rng.gen::<u64>() as usize % entries.len();
        entries[index].0.clone()
    };

    let insert = || {
        (random_value(2), Op::Put(random_value(2)))
    };
    let update = || {
        let key = get_random_key();
        (key.to_vec(), Op::Put(random_value(2)))
    };
    let delete = || {
        let key = get_random_key();
        (key.to_vec(), Op::Delete)
    };

    let mut rng = thread_rng();
    for _ in 0..size {
        let entry = if maybe_tree.is_some() {
            let kind = rng.gen::<u64>() % 3;
            if kind == 0 { insert() }
            else if kind == 1 { update() }
            else { delete() }
        } else {
            insert()
        };
        batch.push(entry);
    }
    batch.sort_by(|a, b| a.0.cmp(&b.0));

    // remove dupes
    let mut maybe_prev_key: Option<Vec<u8>> = None;
    batch
        .drain_filter(|entry| {
            let remove = if let Some(prev_key) = &maybe_prev_key {
                *prev_key == entry.0
            } else {
                false
            };
            maybe_prev_key = Some(entry.0.clone());
            remove
        })
        .collect::<Vec<_>>()
}
