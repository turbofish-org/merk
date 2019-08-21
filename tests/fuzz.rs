#![feature(drain_filter)]

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::iter::FromIterator;
use rand::prelude::*;
use merk::tree::*;
use merk::Batch;
use merk::test_utils::*;

const ITERATIONS: usize = 2_000;

type Map = BTreeMap<Vec<u8>, Vec<u8>>;

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
    let tree = make_tree_rand(initial_size, initial_size, seed);
    let mut map = Map::from_iter(tree.iter());
    let mut maybe_tree = Some(tree);
    println!("====== MERK FUZZ ======");
    println!("SEED: {}", seed);
    println!("{:?}", maybe_tree.as_ref().unwrap());

    for j in 0..3 {
        let batch_size = (rng.gen::<u64>() % 3) + 1;
        let batch = make_batch(maybe_tree.as_ref(), batch_size, rng.gen::<u64>());
        println!("BATCH {}", j);
        println!("{:?}", batch);
        maybe_tree = apply_to_memonly(maybe_tree, &batch);
        apply_to_map(&mut map, &batch);
        assert_map(maybe_tree.as_ref(), &map);
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

pub fn apply_to_map(map: &mut Map, batch: &Batch) {
    for entry in batch.iter() {
        match entry {
            (key, Op::Put(value)) => {
                map.insert(key.to_vec(), value.to_vec());
            },
            (key, Op::Delete) => {
                map.remove(key);
            }
        }
    }
}

pub fn assert_map(maybe_tree: Option<&Tree>, map: &Map) {
    if map.is_empty() {
        assert!(maybe_tree.is_none(), "expected tree to be None");
        return;
    }

    let tree = maybe_tree.expect("expected tree to be Some");

    let mut count = 0;
    for (key, tree_value) in tree.iter() {
        let map_value = map.get(key.as_slice())
            .expect("expected value");
        assert_eq!(tree_value, *map_value);
        count += 1;
    }

    assert_eq!(count, map.len());
}
