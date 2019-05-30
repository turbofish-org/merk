#![feature(test)]

extern crate test;
extern crate merk;

mod util;

use merk::*;

#[test]
fn from_batch_single_put() {
    let batch: Vec<TreeBatchEntry> = vec![
        (b"0000", TreeOp::Put(b"0000"))
    ];
    let tree = SparseTree::from_batch(&batch).unwrap().unwrap();

    assert_eq!(tree.node().key, b"0000");
    assert_eq!(tree.node().value, b"0000");
    assert_tree_valid(&tree);
}

#[test]
fn from_batch_1k_put() {
    let mut keys: Vec<[u8; 4]> = vec![];
    for i in 0..1000 {
        let key = (i as u32).to_be_bytes();
        keys.push(key);
    }

    let mut batch: Vec<TreeBatchEntry> = vec![];
    for key in keys.iter() {
        batch.push((key, TreeOp::Put(key)));
    }

    let tree = SparseTree::from_batch(&batch).unwrap().unwrap();
    assert_tree_valid(&tree);
    assert_tree_keys(&tree, &keys);
}

#[test]
fn from_batch_deletes_only() {
    let batch: Vec<TreeBatchEntry> = vec![
        (&[1, 2, 3], TreeOp::Delete),
        (&[1, 2, 4], TreeOp::Delete),
        (&[1, 2, 5], TreeOp::Delete)
    ];
    let result = SparseTree::from_batch(&batch);
    assert_err!(result, "Tried to delete non-existent key");
}

#[test]
fn from_batch_puts_and_deletes() {
    let batch: Vec<TreeBatchEntry> = vec![
        (&[1, 2, 3], TreeOp::Put(b"xyz")),
        (&[1, 2, 4], TreeOp::Delete),
        (&[1, 2, 5], TreeOp::Put(b"foo")),
        (&[1, 2, 6], TreeOp::Put(b"bar"))
    ];
    let result = SparseTree::from_batch(&batch);
    assert_err!(result, "Tried to delete non-existent key");
}

#[test]
fn from_batch_empty() {
    let batch: Vec<TreeBatchEntry> = vec![];
    let tree = SparseTree::from_batch(&batch).unwrap();
    assert!(tree.is_none());
}

#[test]
fn batch_put_insert() {
    let mut tree = Some(Box::new(
        SparseTree::new(Node::new(b"test", b"0"))
    ));
    assert_tree_valid(&tree.as_mut().expect("tree should not be empty"));

    // put sequential keys
    let mut keys = vec![];
    let mut batch: Vec<TreeBatchEntry> = vec![];
    for i in 0..100 {
        keys.push((i as u32).to_be_bytes());
    }
    for key in keys.iter() {
        batch.push((&key[..], TreeOp::Put(b"x")));
    }
    SparseTree::apply(
        &mut tree,
        &mut |_| unreachable!(),
        &batch
    ).unwrap();

    let tree = tree.expect("tree should not be empty");
    assert_tree_valid(&tree);

    // known final state for deterministic tree
    // assert_eq!(
    //     hex::encode(tree.hash()),
    //     "ba2e3b6397061744c2dece97b12e212a292d3a1f"
    // );
    // assert_eq!(
    //     tree.node().key,
    //     [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 20, 216]
    // );
    // assert_eq!(tree.height(), 16);
    // assert_eq!(tree.child_height(true), 15);
    // assert_eq!(tree.child_height(false), 15);
}

#[test]
fn batch_put_update() {
    let mut tree = Some(Box::new(
        SparseTree::new(Node::new(&[63], b"0"))
    ));
    assert_tree_valid(&tree.as_ref().expect("tree should not be empty"));

    // put sequential keys
    let mut keys = vec![];
    let mut batch: Vec<TreeBatchEntry> = vec![];
    for i in 0..100 {
        keys.push((i as u32).to_be_bytes());
    }
    for key in keys.iter() {
        batch.push((&key[..], TreeOp::Put(b"x")));
    }
    SparseTree::apply(
        &mut tree,
        &mut |_| unreachable!(),
        &batch
    ).unwrap();

    let tree_box = tree.as_ref().expect("tree should not be empty");
    assert_tree_valid(&tree_box);

    // put sequential keys again
    let mut keys = vec![];
    let mut batch: Vec<TreeBatchEntry> = vec![];
    for i in 0..100 {
        keys.push((i as u32).to_be_bytes());
    }
    for key in keys.iter() {
        batch.push((&key[..], TreeOp::Put(b"x")));
    }
    SparseTree::apply(
        &mut tree,
        &mut |_| unreachable!(),
        &batch
    ).unwrap();

    let tree_box = tree.expect("tree should not be empty");
    assert_tree_valid(&tree_box);

    // known final state for deterministic tree
    // assert_eq!(
    //     hex::encode(tree.hash()),
    //     "7a9968205f500cb8de6ac37ddf53fcd97cef6524"
    // );
    // assert_eq!(tree.node.key, b"3");
    // assert_eq!(tree.height(), 5);
    // assert_eq!(tree.child_height(true), 4);
    // assert_eq!(tree.child_height(false), 3);
}

/// Recursively asserts invariants for each node in the tree.
fn assert_tree_valid(tree: &SparseTree) {
    assert!(
        tree.balance_factor().abs() <= 1,
        format!("node should be balanced. bf={}", tree.balance_factor())
    );

    let assert_child_valid = |child: &SparseTree, left: bool| {
        assert!(
            (child.node().key < tree.node().key) == left,
            "child should be ordered by key.\n{:?}",
            tree
        );

        assert_eq!(
            tree.child_link(left).as_ref().unwrap(),
            &child.as_link(),
            "parent link should match child"
        );

        // recursive validity check
        assert_tree_valid(child);
    };

    // check left child
    if let Some(left) = tree.child_tree(true) {
        assert_child_valid(left, true);
    }

    // check right child
    if let Some(right) = tree.child_tree(false) {
        assert_child_valid(right, false);
    }

    // // ensure keys are globally ordered (root only)
    // let entries = tree.entries();
    // let mut prev = &entries[0].0;
    // for (k, _) in tree.entries()[1..].iter() {
    //     assert!(k > prev);
    //     prev = &k;
    // }
}

fn assert_tree_keys<K: AsRef<[u8]>>(tree: &SparseTree, expected_keys: &[K]) {
    fn traverse<'a>(tree: &'a SparseTree, keys: Vec<&'a [u8]>) -> Vec<&'a [u8]> {
        let mut keys = match tree.child_tree(true) {
            None => keys,
            Some(child) => traverse(child, keys)
        };

        keys.push(&tree.key);

        match tree.child_tree(false) {
            None => keys,
            Some(child) => traverse(child, keys)
        }
    }

    let actual_keys = traverse(tree, vec![]);
    assert_eq!(actual_keys.len(), expected_keys.len());
    for i in 0..actual_keys.len() {
        assert_eq!(actual_keys[i], expected_keys[i].as_ref());
    }
}
