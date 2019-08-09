use merk;
use merk::tree::{Tree, Walker, NoopCommit};
use merk::{Batch, PanicSource};

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

pub fn apply_noop(tree: Tree, batch: &Batch) -> Tree {
    let walker = Walker::<PanicSource>::new(tree, PanicSource {});
    let mut tree = Walker::<PanicSource>::apply_to(Some(walker), batch)
        .expect("apply failed")
        .expect("expected tree");
    tree.commit(&mut NoopCommit {})
        .expect("commit failed");
    assert_tree_invariants(&tree);
    tree
}
