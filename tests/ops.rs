extern crate merk;

mod common;

use merk::*;
use merk::tree::{Tree, Walker, NoopCommit}; 
use common::{assert_tree_invariants, apply_noop};

#[test]
fn insert_empty_single() {
    let batch = vec![ (vec![0], Op::Put(vec![1])) ];
    let tree = Walker::<PanicSource>::apply_to(None, &batch)
        .expect("apply_to failed")
        .expect("expected tree");
    assert_eq!(tree.key(), &[0]);
    assert_eq!(tree.value(), &[1]);
    assert_tree_invariants(&tree);
}

#[test]
fn insert_root_single() {
    let tree = Tree::new(vec![5], vec![123]);
    let batch = vec![ (vec![6], Op::Put(vec![123])) ];
    let tree = apply_noop(tree, &batch);
    assert_eq!(tree.key(), &[5]);
    assert!(tree.child(true).is_none());
    assert_eq!(tree.child(false).expect("expected child").key(), &[6]);
}

#[test]
fn insert_root_double() {
    let tree = Tree::new(vec![5], vec![123]);
    let batch = vec![
        (vec![4], Op::Put(vec![123])),
        (vec![6], Op::Put(vec![123]))
    ];
    let tree = apply_noop(tree, &batch);
    assert_eq!(tree.key(), &[5]);
    assert_eq!(tree.child(true).expect("expected child").key(), &[4]);
    assert_eq!(tree.child(false).expect("expected child").key(), &[6]);
}

#[test]
fn insert_rebalance() {
    let tree = Tree::new(vec![5], vec![123]);

    let batch = vec![ (vec![6], Op::Put(vec![123])) ];
    let tree = apply_noop(tree, &batch);

    let batch = vec![ (vec![7], Op::Put(vec![123])) ];
    let tree = apply_noop(tree, &batch);

    assert_eq!(tree.key(), &[6]);
    assert_eq!(tree.child(true).expect("expected child").key(), &[5]);
    assert_eq!(tree.child(false).expect("expected child").key(), &[7]);
}
