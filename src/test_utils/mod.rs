use std::ops::Range;
use std::convert::TryInto;
use byteorder::{BigEndian, WriteBytesExt};
use crate::tree::{Tree, Walker, NoopCommit};
use crate::{Batch, Op, PanicSource, BatchEntry};

pub fn assert_tree_invariants(tree: &Tree) {
    assert!(tree.balance_factor().abs() < 2);

    let maybe_left = tree.link(true);
    if let Some(left) = maybe_left {
        assert!(left.key() < tree.key());
        assert!(!left.is_modified());
    }

    let maybe_right = tree.link(false);
    if let Some(right) = maybe_right {
        assert!(right.key() > tree.key());
        assert!(!right.is_modified());
    }

    tree.child(true).map(|left| assert_tree_invariants(left));
    tree.child(false).map(|right| assert_tree_invariants(right));
}

pub fn apply_memonly_unchecked(tree: Tree, batch: &Batch) -> Tree {
    let walker = Walker::<PanicSource>::new(tree, PanicSource {});
    let mut tree = Walker::<PanicSource>::apply_to(Some(walker), batch)
        .expect("apply failed")
        .expect("expected tree");
    tree.commit(&mut NoopCommit {})
        .expect("commit failed");
    tree
}

pub fn apply_memonly(tree: Tree, batch: &Batch) -> Tree {
    let tree = apply_memonly_unchecked(tree, batch);
    assert_tree_invariants(&tree);
    tree
}

fn put_entry(n: u64) -> BatchEntry {
    let mut key = vec![0; 12];
    key.write_u64::<BigEndian>(n)
        .expect("writing to key failed");
    (key, Op::Put(vec![123; 60]))
}

pub fn make_batch_seq(range: Range<u64>) -> Vec<BatchEntry> {
    let mut batch = Vec::with_capacity(
        (range.end - range.start).try_into().unwrap()
    );
    for n in range {
        batch.push(put_entry(n));
    }
    batch
}

// pub fn make_batch_rand(size: usize, seed: usize) -> Vec<BatchEntry> {
//     let mut batch = Vec::with_capacity(size);
// }

pub fn make_tree_seq(node_count: u64) -> Tree {
    let batch_size = 10_000;
    assert!(node_count % batch_size == 0);

    let value = vec![123; 60];
    let mut tree = Tree::new(vec![0; 20], value.clone());
    
    let batch_count = node_count / batch_size;
    for i in 0..batch_count {
        let batch = make_batch_seq((i * batch_size)..((i+1) * batch_size));
        tree = apply_memonly(tree, &batch);
    }

    tree
}
