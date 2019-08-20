#![feature(drain_filter)]

use rand::prelude::*;
use merk::tree::*;
use merk::test_utils::*;

const ITERATIONS: usize = 1_000;

#[test]
fn fuzz() {
    let mut rng = thread_rng();

    for i in 0..ITERATIONS {
        let initial_size = (rng.gen::<u64>() % 16) + 1;
        let seed = rng.gen::<u64>();
        let mut maybe_tree = Some(make_tree_rand(initial_size, initial_size, seed));
        println!("====== i:{} ======", i);
        println!("{:?}\n", maybe_tree.as_ref().unwrap());

        for j in 0..4 {
            let batch_size = (rng.gen::<u64>() % 5) + 1;
            let batch = make_batch(maybe_tree.as_ref(), batch_size);
            println!("   === j:{} ===", j);
            println!("{} {:?}", batch.len(), batch);
            maybe_tree = apply_to_memonly(maybe_tree, &batch);
            if let Some(tree) = &maybe_tree {
                println!("{:?}", &tree);
            } else {
                println!("(Empty tree)");
            }
            println!("\n");
        }
    }
}


pub fn make_batch(maybe_tree: Option<&Tree>, size: u64) -> Vec<BatchEntry> {
    let mut batch = Vec::with_capacity(size as usize);

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
            let should_yield = if let Some(prev_key) = &maybe_prev_key {
                *prev_key != entry.0
            } else {
                true
            };
            maybe_prev_key = Some(entry.0.clone());
            should_yield
        })
        .collect::<Vec<_>>()
}

pub fn random_value(size: usize) -> Vec<u8> {
    let mut value = vec![0; size];
    let mut rng = thread_rng();
    rng.fill_bytes(&mut value[..]);
    value
}
