use merk;
use merk::tree::Tree;

pub fn assert_tree_invariants(tree: Tree) {
    assert!(tree.balance_factor().abs() < 2);

    let maybe_left = tree.link(true);
    if let Some(left) = maybe_left {
        assert!(left.key() < tree.key());
        assert!(!left.is_modified());
    }

    let maybe_right = tree.link(true);
    if let Some(right) = maybe_right {
        assert!(right.key() > tree.key());
        assert!(!right.is_modified());
    }

    tree.child(true).map(|left| assert_tree_invariants(left));
    tree.child(false).map(|right| assert_tree_invariants(right));
}
