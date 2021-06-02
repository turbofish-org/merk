#![feature(test)]

use criterion::*;
use merk::owner::Owner;
use merk::test_utils::*;

fn insert_1m_10k_seq_memonly(c: &mut Criterion) {
    let initial_size = 1_000_000;
    let batch_size = 10_000;

    let mut tree = Owner::new(make_tree_seq(initial_size));

    let mut i = initial_size / batch_size;
    c.bench_function(
        "insert_1m_10k_seq_memonly",
        |b| b.iter(|| {
            let batch = make_batch_seq((i * batch_size)..((i + 1) * batch_size));
            tree.own(|tree| apply_memonly_unchecked(tree, &batch));
            i += 1;
        })
    );
}

fn insert_1m_10k_rand_memonly(c: &mut Criterion) {
    let initial_size = 1_000_000;
    let batch_size = 10_000;

    let mut tree = Owner::new(make_tree_rand(initial_size, batch_size, 0));

    let mut i = initial_size / batch_size;
    c.bench_function(
        "insert_1m_10k_rand_memonly",
        |b| b.iter(|| {
            let batch = make_batch_rand(batch_size, i);
            tree.own(|tree| apply_memonly_unchecked(tree, &batch));
            i += 1;
        })
    );
}

fn update_1m_10k_seq_memonly(c: &mut Criterion) {
    let initial_size = 1_000_000;
    let batch_size = 10_000;

    let mut tree = Owner::new(make_tree_seq(initial_size));

    let mut i = 0;
    c.bench_function(
        "update_1m_10k_seq_memonly",
        |b| b.iter(|| {
            let batch = make_batch_seq((i * batch_size)..((i + 1) * batch_size));
            tree.own(|tree| apply_memonly_unchecked(tree, &batch));
            i = (i + 1) % (initial_size / batch_size);
        })
    );
}

fn update_1m_10k_rand_memonly(c: &mut Criterion) {
    let initial_size = 1_010_000;
    let batch_size = 10_000;

    let mut tree = Owner::new(make_tree_rand(initial_size, batch_size, 0));

    let mut i = 0;
    c.bench_function(
        "update_1m_10k_rand_memonly",
        |b| b.iter(|| {
            let batch = make_batch_rand(batch_size, i);
            tree.own(|tree| apply_memonly_unchecked(tree, &batch));
            i = (i + 1) % (initial_size / batch_size);
        })
    );
}

criterion_group!{
    name = ops;
    config = Criterion::default();
    targets =  insert_1m_10k_seq_memonly,
        insert_1m_10k_rand_memonly,
        update_1m_10k_seq_memonly,
        update_1m_10k_rand_memonly,
}

criterion_main!(ops);
