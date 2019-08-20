#![feature(drain_filter)]

use std::cell::RefCell;
use rand::prelude::*;
use merk::tree::*;
use merk::test_utils::*;

const ITERATIONS: usize = 2_000;

#[test]
fn fuzz() {
    let mut rng = thread_rng();

    for _ in 0..ITERATIONS {
        let seed = rng.gen::<u64>();
        fuzz_case(seed);
    }
}

#[test]
fn fuzz_17391518417409062786() {
    fuzz_case(17391518417409062786);
}

#[test]
fn fuzz_396148930387069749() {
    fuzz_case(396148930387069749);
}

pub fn fuzz_case(seed: u64) {
    let mut rng: SmallRng = SeedableRng::seed_from_u64(seed);
    let initial_size = (rng.gen::<u64>() % 10) + 1;
    let mut maybe_tree = Some(make_tree_rand(initial_size, initial_size, seed));
    println!("====== MERK FUZZ ======");
    println!("SEED: {}", seed);
    println!("{:?}", maybe_tree.as_ref().unwrap());

    for j in 0..3 {
        let batch_size = (rng.gen::<u64>() % 3) + 1;
        let batch = make_batch(maybe_tree.as_ref(), batch_size, rng.gen::<u64>());
        println!("BATCH {}", j);
        println!("{:?}", batch);
        maybe_tree = apply_to_memonly(maybe_tree, &batch);
        if let Some(tree) = &maybe_tree {
            println!("{:?}", &tree);
        } else {
            println!("(Empty tree)");
        }
    }
}

pub fn make_batch(maybe_tree: Option<&Tree>, size: u64, seed: u64) -> Vec<BatchEntry> {
    let rng: RefCell<SmallRng> = RefCell::new(
        SeedableRng::seed_from_u64(seed)
    );
    let mut batch = Vec::with_capacity(size as usize);

    let get_random_key = || {
        let tree = maybe_tree.as_ref().unwrap();
        let entries: Vec<_> = tree.iter().collect();
        let index = rng.borrow_mut().gen::<u64>() as usize % entries.len();
        entries[index].0.clone()
    };

    let random_value = |size| {
        let mut value = vec![0; size];
        rng.borrow_mut().fill_bytes(&mut value[..]);
        value
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

    for _ in 0..size {
        let entry = if maybe_tree.is_some() {
            let kind = rng.borrow_mut().gen::<u64>() % 3;
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
