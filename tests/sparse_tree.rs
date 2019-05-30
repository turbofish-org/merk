#![feature(test)]

extern crate test;
extern crate merk;

mod util;

use merk::*;

#[test]
fn from_batch_single_put() {
    let batch: &[TreeBatchEntry] = &[
        (b"0000", TreeOp::Put(b"0000"))
    ];
    let tree = SparseTree::from_batch(batch).unwrap().unwrap();

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
    let batch: &[TreeBatchEntry] = &[
        (&[1, 2, 3], TreeOp::Delete),
        (&[1, 2, 4], TreeOp::Delete),
        (&[1, 2, 5], TreeOp::Delete)
    ];
    let result = SparseTree::from_batch(batch);
    assert_err!(result, "Tried to delete non-existent key: [1, 2, 4]");
}

#[test]
fn from_batch_puts_and_deletes() {
    let batch: &[TreeBatchEntry] = &[
        (&[1, 2, 3], TreeOp::Put(b"xyz")),
        (&[1, 2, 4], TreeOp::Delete),
        (&[1, 2, 5], TreeOp::Put(b"foo")),
        (&[1, 2, 6], TreeOp::Put(b"bar"))
    ];
    let result = SparseTree::from_batch(batch);
    assert_err!(result, "Tried to delete non-existent key: [1, 2, 4]");
}

#[test]
fn from_batch_empty() {
    let batch: &[TreeBatchEntry] = &[];
    let tree = SparseTree::from_batch(batch).unwrap();
    assert!(tree.is_none());
}

#[test]
fn apply_simple_insert() {
    let mut container = None;
    let batch: &[TreeBatchEntry] = &[
        (b"key", TreeOp::Put(b"value"))
    ];
    SparseTree::apply(&mut container, &mut |_| unreachable!(), batch).unwrap();

    let tree = container.unwrap();
    assert_eq!(tree.key, b"key");
    assert_eq!(tree.value, b"value");
    assert_tree_valid(&tree);
    assert_tree_keys(&tree, &[b"key"]);
}

#[test]
fn apply_simple_update() {
    let mut container = Some(Box::new(
        SparseTree::new(
            Node::new(b"key", b"value")
        )
    ));
    let batch: &[TreeBatchEntry] = &[
        (b"key", TreeOp::Put(b"new value"))
    ];
    SparseTree::apply(&mut container, &mut |_| unreachable!(), batch).unwrap();

    let tree = container.unwrap();
    assert_eq!(tree.key, b"key");
    assert_eq!(tree.value, b"new value");
    assert_tree_valid(&tree);
    assert_tree_keys(&tree, &[b"key"]);
}

#[test]
fn apply_simple_delete() {
    let mut container = Some(Box::new(
        SparseTree::new(
            Node::new(b"key", b"value")
        )
    ));
    let batch: &[TreeBatchEntry] = &[
        (b"key", TreeOp::Delete)
    ];
    SparseTree::apply(&mut container, &mut |_| unreachable!(), batch).unwrap();
    assert_eq!(container, None);
}

#[test]
fn apply_insert_under() {
    let mut container = Some(Box::new(
        SparseTree::new(
            Node::new(&[5], b"value")
        )
    ));
    let batch: &[TreeBatchEntry] = &[
        (&[6], TreeOp::Put(b"value"))
    ];
    SparseTree::apply(&mut container, &mut |_| unreachable!(), batch).unwrap();

    let tree = container.unwrap();
    assert_eq!(tree.key, &[5]);
    assert_eq!(tree.value, b"value");
    assert_eq!(tree.right.as_ref().unwrap().key, &[6]);
    assert_eq!(tree.child_tree(false).unwrap().value, b"value");
    assert_eq!(tree.height(), 2);
    assert_tree_valid(&tree);
    assert_tree_keys(&tree, &[[5], [6]]);
}

#[test]
fn apply_update_and_insert() {
    let mut container = Some(Box::new(
        SparseTree::new(
            Node::new(&[5], b"value")
        )
    ));
    let batch: &[TreeBatchEntry] = &[
        (&[5], TreeOp::Put(b"value2")),
        (&[6], TreeOp::Put(b"value3"))
    ];
    SparseTree::apply(&mut container, &mut |_| unreachable!(), batch).unwrap();

    let tree = container.unwrap();
    assert_eq!(tree.key, &[5]);
    assert_eq!(tree.value, b"value2");
    assert_eq!(tree.right.as_ref().unwrap().key, &[6]);
    assert_eq!(tree.child_tree(false).unwrap().value, b"value3");
    assert_eq!(tree.height(), 2);
    assert_tree_valid(&tree);
    assert_tree_keys(&tree, &[[5], [6]]);
}

#[test]
fn apply_insert_balance() {
    let mut container = Some(Box::new(
        SparseTree::new(
            Node::new(&[5], b"value")
        )
    ));
    let batch: &[TreeBatchEntry] = &[
        (&[6], TreeOp::Put(b"value2")),
        (&[7], TreeOp::Put(b"value3"))
    ];
    SparseTree::apply(&mut container, &mut |_| unreachable!(), batch).unwrap();

    let tree = container.unwrap();
    assert_eq!(tree.key, &[6]);
    assert_eq!(tree.value, b"value2");
    assert_eq!(tree.left.as_ref().unwrap().key, &[5]);
    assert_eq!(tree.right.as_ref().unwrap().key, &[7]);
    assert_eq!(tree.child_tree(true).unwrap().value, b"value");
    assert_eq!(tree.child_tree(false).unwrap().value, b"value3");
    assert_eq!(tree.height(), 2);
    assert_tree_valid(&tree);
    assert_tree_keys(&tree, &[[5], [6], [7]]);
}

#[test]
fn apply_delete_inner() {
    let mut container = Some(Box::new(
        SparseTree::new(
            Node::new(&[5], b"value")
        )
    ));
    let batch: &[TreeBatchEntry] = &[
        (&[6], TreeOp::Put(b"value2")),
        (&[7], TreeOp::Put(b"value3")),
        (&[8], TreeOp::Put(b"value4")),
        (&[9], TreeOp::Put(b"value5")),
    ];
    SparseTree::apply(&mut container, &mut |_| unreachable!(), batch).unwrap();

    let batch: &[TreeBatchEntry] = &[
        (&[8], TreeOp::Delete)
    ];
    SparseTree::apply(&mut container, &mut |_| unreachable!(), batch).unwrap();

    let tree = container.unwrap();
    assert_eq!(tree.key, &[7]);
    assert_eq!(tree.left.as_ref().unwrap().key, &[5]);
    assert_eq!(tree.right.as_ref().unwrap().key, &[9]);
    assert_eq!(tree.height(), 3);
    assert_tree_valid(&tree);
    assert_tree_keys(&tree, &[[5], [6], [7], [9]]);
}

#[test]
fn insert_100() {
    let mut tree = None;
    let keys = sequential_keys(0, 100);
    let batch = puts(&keys);
    SparseTree::apply(
        &mut tree,
        &mut |_| unreachable!(),
        &batch
    ).unwrap();

    let tree = tree.expect("tree should not be empty");
    assert_tree_valid(&tree);
    assert_tree_keys(&tree, &keys);

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
fn update_100() {
    let mut tree = None;
    let keys = sequential_keys(0, 100);
    let batch = puts(&keys);
    SparseTree::apply(
        &mut tree,
        &mut |_| unreachable!(),
        &batch
    ).unwrap();

    let tree_box = tree.as_ref().expect("tree should not be empty");
    assert_tree_valid(&tree_box);
    assert_tree_keys(&tree_box, &keys);

    // put sequential keys again
    let keys = sequential_keys(0, 100);
    let batch = puts(&keys);
    SparseTree::apply(&mut tree, &mut |_| unreachable!(), &batch).unwrap();

    let tree_box = tree.expect("tree should not be empty");
    assert_tree_valid(&tree_box);
    assert_tree_keys(&tree_box, &keys);

    assert_eq!(tree_box.key, &[0, 0, 0, 59]);
    assert_eq!(tree_box.height(), 8);
    assert_eq!(tree_box.child_height(true), 7);
    assert_eq!(tree_box.child_height(false), 6);
}

#[test]
fn delete_100() {
    let mut tree = None;
    let keys = sequential_keys(0, 100);
    let batch = puts(&keys);
    SparseTree::apply(
        &mut tree,
        &mut |_| unreachable!(),
        &batch
    ).unwrap();
    let tree_box = tree.as_ref().expect("tree should not be empty");
    assert_tree_valid(&tree_box);
    assert_tree_keys(&tree_box, &keys);

    // delete sequential keys
    let keys = sequential_keys(0, 100);
    let mut batch: Vec<TreeBatchEntry> = vec![];
    for i in 0..99 {
        batch.push((&keys[i], TreeOp::Delete));
    }
    SparseTree::apply(&mut tree, &mut |_| unreachable!(), &batch).unwrap();

    let tree = tree.as_ref().expect("tree should not be empty");
    assert_tree_valid(&tree);
    assert_eq!(tree.height(), 1);
    assert_eq!(tree.key, &keys[99]);
}

#[test]
fn delete_sequential() {
    let mut tree = None;
    let keys = sequential_keys(0, 100);
    let batch = puts(&keys);
    SparseTree::apply(
        &mut tree,
        &mut |_| unreachable!(),
        &batch
    ).unwrap();

    let tree_box = tree.as_ref().expect("tree should not be empty");
    assert_tree_valid(&tree_box);
    assert_tree_keys(&tree_box, &keys);

    // delete sequential keys
    let keys = sequential_keys(0, 100);
    for i in 0..99 {
        let batch: &[TreeBatchEntry] = &[
            (&keys[i], TreeOp::Delete)
        ];
        SparseTree::apply(&mut tree, &mut |_| unreachable!(), &batch).unwrap();

        let tree_box = tree.as_ref().expect("tree should not be empty");
        assert_tree_valid(&tree_box);
        assert_tree_keys(&tree_box, &keys[i+1..]);
    }
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

    // ensure keys are globally ordered (root only)
    let keys = tree_keys(tree);
    if !keys.is_empty() {
        let mut prev = &keys[0];
        for key in keys[1..].iter() {
            assert!(key > prev);
            prev = &key;
        }
    }
}

fn tree_keys<'a>(tree: &'a SparseTree) -> Vec<&'a [u8]> {
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

    traverse(tree, vec![])
}

fn assert_tree_keys<K: AsRef<[u8]>>(tree: &SparseTree, expected_keys: &[K]) {
    let actual_keys = tree_keys(tree);
    assert_eq!(actual_keys.len(), expected_keys.len());
    for i in 0..actual_keys.len() {
        assert_eq!(actual_keys[i], expected_keys[i].as_ref());
    }
}

fn sequential_keys(start: usize, end: usize) -> Vec<[u8; 4]> {
    let mut keys = vec![];
    for i in start..end {
        keys.push((i as u32).to_be_bytes());
    }
    keys
}

fn puts<'a>(keys: &'a [[u8; 4]]) -> Vec<TreeBatchEntry<'a>> {
    let mut batch: Vec<TreeBatchEntry> = vec![];
    for key in keys.iter() {
        batch.push((&key[..], TreeOp::Put(b"x")));
    }
    batch
}