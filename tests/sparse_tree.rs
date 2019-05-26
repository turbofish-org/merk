#![feature(test)]

extern crate test;
extern crate merk;

use merk::*;

#[test]
fn batch_put_insert() {
    let mut tree = SparseTree::new(Node::new(b"test", b"0"));
    assert_tree_valid(&tree);

    // put sequential keys
    let mut keys = vec![];
    let mut batch: Vec<(&[u8], &[u8])> = vec![];
    for i in 0..10_000 {
        keys.push((i as u128).to_be_bytes());
    }
    for key in keys.iter() {
        batch.push((&key[..], b"x"));
    }
    tree.put_batch(
        &mut |_| unreachable!(),
        &batch
    ).unwrap();

    assert_tree_valid(&tree);

    // known final state for deterministic tree
    assert_eq!(
        hex::encode(tree.hash()),
        "eebf7114a67b227c16b6bda662e1731d112f371f"
    );
    assert_eq!(
        tree.node().key,
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 26, 12]
    );
    assert_eq!(tree.height(), 16);
    assert_eq!(tree.child_height(true), 15);
    assert_eq!(tree.child_height(false), 14);
}

#[test]
fn batch_put_update() {
    let mut tree = SparseTree::new(Node::new(&[63], b"0"));
    assert_tree_valid(&tree);

    // put sequential keys
    let mut keys = vec![];
    let mut batch: Vec<(&[u8], &[u8])> = vec![];
    for i in 0..10_000 {
        keys.push((i as u128).to_be_bytes());
    }
    for key in keys.iter() {
        batch.push((&key[..], b"x"));
    }
    tree.put_batch(
        &mut |_| unreachable!(),
        &batch
    ).unwrap();

    assert_tree_valid(&tree);

    // put sequential keys again
    let mut keys = vec![];
    let mut batch: Vec<(&[u8], &[u8])> = vec![];
    for i in 0..10_000 {
        keys.push((i as u128).to_be_bytes());
    }
    for key in keys.iter() {
        batch.push((&key[..], b"x"));
    }
    tree.put_batch(
        &mut |_| unreachable!(),
        &batch
    ).unwrap();

    assert_tree_valid(&tree);

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
    // ensure node is balanced
    assert!(
        tree.balance_factor().abs() <= 1,
        format!("bf:{} {:?}", tree.balance_factor(), tree)
    );

    let assert_child_valid = |child: &SparseTree, left: bool| {
        // check key ordering
        assert!((child.node().key < tree.node().key) == left);

        // ensure child points to parent
        assert_eq!(
            child.node().parent_key.as_ref().unwrap(),
            &tree.node().key
        );

        // ensure parent link matches child
        assert_eq!(
            tree.child_link(left).unwrap(),
            child.as_link()
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
