use std::iter::Flatten;

use super::{Decoder, Node, Op};
use crate::error::Result;
use crate::tree::{kv_hash, node_hash, Hash, NULL_HASH};
use failure::bail;

/// Contains a tree's child node and its hash. The hash can always be assumed to
/// be up-to-date.
#[derive(Debug)]
pub(crate) struct Child {
    pub(crate) tree: Box<Tree>,
    pub(crate) hash: Hash,
}

/// A binary tree data structure used to represent a select subset of a tree
/// when verifying Merkle proofs.
#[derive(Debug)]
pub(crate) struct Tree {
    pub(crate) node: Node,
    pub(crate) left: Option<Child>,
    pub(crate) right: Option<Child>,
    pub(crate) height: usize,
}

impl From<Node> for Tree {
    /// Creates a childless tree with the target node as the `node` field.
    fn from(node: Node) -> Self {
        Tree {
            node,
            left: None,
            right: None,
            height: 1,
        }
    }
}

impl PartialEq for Tree {
    /// Checks equality for the hashes of the two trees.
    fn eq(&self, other: &Self) -> bool {
        self.hash() == other.hash()
    }
}

impl Tree {
    /// Returns an immutable reference to the child on the given side, if any.
    pub(crate) fn child(&self, left: bool) -> Option<&Child> {
        if left {
            self.left.as_ref()
        } else {
            self.right.as_ref()
        }
    }

    /// Returns a mutable reference to the child on the given side, if any.
    fn child_mut(&mut self, left: bool) -> &mut Option<Child> {
        if left {
            &mut self.left
        } else {
            &mut self.right
        }
    }

    /// Attaches the child to the `Tree`'s given side. Panics if there is
    /// already a child attached to this side.
    fn attach(&mut self, left: bool, child: Tree) -> Result<()> {
        if self.child(left).is_some() {
            bail!("Tried to attach to left child, but it is already Some");
        }

        self.height = self.height.max(child.height + 1);

        let hash = child.hash();
        let tree = Box::new(child);
        *self.child_mut(left) = Some(Child { tree, hash });

        Ok(())
    }

    /// Returns the already-computed hash for this tree node's child on the
    /// given side, if any. If there is no child, returns the null hash
    /// (zero-filled).
    #[inline]
    fn child_hash(&self, left: bool) -> Hash {
        self.child(left).map_or(NULL_HASH, |c| c.hash)
    }

    /// Consumes the tree node, calculates its hash, and returns a `Node::Hash`
    /// variant.
    fn into_hash(self) -> Tree {
        Node::Hash(self.hash()).into()
    }

    /// Gets or computes the hash for this tree node.
    pub(crate) fn hash(&self) -> Hash {
        fn compute_hash(tree: &Tree, kv_hash: Hash) -> Hash {
            node_hash(&kv_hash, &tree.child_hash(true), &tree.child_hash(false))
        }

        match &self.node {
            Node::Hash(hash) => *hash,
            Node::KVHash(kv_hash) => compute_hash(&self, *kv_hash),
            Node::KV(key, value) => {
                let kv_hash = kv_hash(key.as_slice(), value.as_slice());
                compute_hash(&self, kv_hash)
            }
        }
    }

    /// Creates an iterator that yields the in-order traversal of the nodes at
    /// the given depth.
    pub(crate) fn layer(&self, depth: usize) -> LayerIter {
        LayerIter::new(self, depth)
    }

    /// Consumes the `Tree` and does an in-order traversal over all the nodes in
    /// the tree, calling `visit_node` for each.
    pub(crate) fn visit_nodes<F: FnMut(Node)>(mut self, visit_node: &mut F) {
        if let Some(child) = self.left.take() {
            child.tree.visit_nodes(visit_node);
        }

        let maybe_right_child = self.right.take();
        visit_node(self.node);

        if let Some(child) = maybe_right_child {
            child.tree.visit_nodes(visit_node);
        }
    }

    /// Does an in-order traversal over references to all the nodes in the tree,
    /// calling `visit_node` for each.
    pub(crate) fn visit_refs<F: FnMut(&Tree)>(&self, visit_node: &mut F) {
        if let Some(child) = &self.left {
            child.tree.visit_refs(visit_node);
        }

        visit_node(self);

        if let Some(child) = &self.right {
            child.tree.visit_refs(visit_node);
        }
    }
}

/// `LayerIter` iterates over the nodes in a `Tree` at a given depth. Nodes are
/// visited in order.
pub(crate) struct LayerIter<'a> {
    stack: Vec<&'a Tree>,
    depth: usize,
}

impl<'a> LayerIter<'a> {
    /// Creates a new `LayerIter` that iterates over `tree` at the given depth.
    fn new(tree: &'a Tree, depth: usize) -> Self {
        let mut iter = LayerIter {
            stack: Vec::with_capacity(depth),
            depth,
        };

        iter.traverse_to_start(tree, depth);
        iter
    }

    /// Builds up the stack by traversing through left children to the desired depth.
    fn traverse_to_start(&mut self, tree: &'a Tree, remaining_depth: usize) {
        self.stack.push(tree);

        if remaining_depth == 0 {
            return;
        }

        if let Some(child) = tree.child(true) {
            self.traverse_to_start(&child.tree, remaining_depth - 1)
        } else {
            panic!("Could not traverse to given layer")
        }
    }
}

impl<'a> Iterator for LayerIter<'a> {
    type Item = &'a Tree;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.stack.pop();
        let mut popped = item;

        loop {
            if self.stack.is_empty() {
                return item;
            }

            let parent = self.stack.last().unwrap();
            let left_child = parent.child(true).unwrap();
            let right_child = parent.child(false).unwrap();

            if left_child.tree.as_ref() == popped.unwrap() {
                self.stack.push(&right_child.tree);

                while self.stack.len() - 1 < self.depth {
                    let parent = self.stack.last().unwrap();
                    let left_child = parent.child(true).unwrap();
                    self.stack.push(&left_child.tree);
                }

                return item;
            } else {
                popped = self.stack.pop();
            }
        }
    }
}

/// Executes a proof by stepping through its operators, modifying the
/// verification stack as it goes. The resulting stack item is returned.
///
/// If the `collapse` option is set to `true`, nodes will be hashed and pruned
/// from memory during execution. This results in the minimum amount of memory
/// usage, and the returned `Tree` will only contain a single node of type
/// `Node::Hash`. If `false`, the returned `Tree` will contain the entire
/// subtree contained in the proof.
///
/// `visit_node` will be called once for every push operation in the proof, in
/// key-order. If `visit_node` returns an `Err` result, it will halt the
/// execution and `execute` will return the error.
pub(crate) fn execute<I, F>(ops: I, collapse: bool, mut visit_node: F) -> Result<Tree>
where
    I: IntoIterator<Item = Result<Op>>,
    F: FnMut(&Node) -> Result<()>,
{
    let mut stack: Vec<Tree> = Vec::with_capacity(32);
    let mut maybe_last_key = None;

    fn try_pop(stack: &mut Vec<Tree>) -> Result<Tree> {
        match stack.pop() {
            None => bail!("Stack underflow"),
            Some(tree) => Ok(tree),
        }
    }

    for op in ops {
        match op? {
            Op::Parent => {
                let (mut parent, child) = (try_pop(&mut stack)?, try_pop(&mut stack)?);
                parent.attach(true, if collapse { child.into_hash() } else { child })?;
                stack.push(parent);
            }
            Op::Child => {
                let (child, mut parent) = (try_pop(&mut stack)?, try_pop(&mut stack)?);
                parent.attach(false, if collapse { child.into_hash() } else { child })?;
                stack.push(parent);
            }
            Op::Push(node) => {
                if let Node::KV(key, _) = &node {
                    // keys should always increase
                    if let Some(last_key) = &maybe_last_key {
                        if key <= last_key {
                            bail!("Incorrect key ordering");
                        }
                    }

                    maybe_last_key = Some(key.clone());
                }

                visit_node(&node)?;

                let tree: Tree = node.into();
                stack.push(tree);
            }
        }
    }

    if stack.len() != 1 {
        bail!("Expected proof to result in exactly one stack item");
    }

    Ok(stack.pop().unwrap())
}

/// Verifies the encoded proof with the given query and expected hash.
///
/// Every key in `keys` is checked to either have a key/value pair in the proof,
/// or to have its absence in the tree proven.
///
/// Returns `Err` if the proof is invalid, or a list of proven values associated
/// with `keys`. For example, if `keys` contains keys `A` and `B`, the returned
/// list will contain 2 elements, the value of `A` and the value of `B`. Keys
/// proven to be absent in the tree will have an entry of `None`, keys that have
/// a proven value will have an entry of `Some(value)`.
pub fn verify_query(
    bytes: &[u8],
    keys: &[Vec<u8>],
    expected_hash: Hash,
) -> Result<Vec<Option<Vec<u8>>>> {
    let mut key_index = 0;
    let mut last_push = None;
    let mut output = Vec::with_capacity(keys.len());

    let ops = Decoder::new(bytes);

    let root = execute(ops, true, |node| {
        if let Node::KV(key, value) = node {
            loop {
                if key_index >= keys.len() || *key < keys[key_index] {
                    // TODO: should we error if proof includes unused keys?
                    break;
                } else if key == &keys[key_index] {
                    // KV for queried key
                    output.push(Some(value.clone()));
                } else if *key > keys[key_index] {
                    match &last_push {
                        None | Some(Node::KV(_, _)) => {
                            // previous push was a boundary (global edge or lower key),
                            // so this is a valid absence proof
                            output.push(None);
                        }
                        // proof is incorrect since it skipped queried keys
                        _ => bail!("Proof incorrectly formed"),
                    }
                }

                key_index += 1;
            }
        }

        last_push = Some(node.clone());

        Ok(())
    })?;

    // absence proofs for right edge
    if key_index < keys.len() {
        if let Some(Node::KV(_, _)) = last_push {
            for _ in 0..(keys.len() - key_index) {
                output.push(None);
            }
        } else {
            bail!("Proof incorrectly formed");
        }
    } else {
        debug_assert_eq!(keys.len(), output.len());
    }

    if root.hash() != expected_hash {
        bail!(
            "Proof did not match expected hash\n\tExpected: {:?}\n\tActual: {:?}",
            expected_hash,
            root.hash()
        );
    }

    Ok(output)
}

#[cfg(test)]
mod test {
    use super::super::*;
    use super::*;
    use crate::tree;
    use crate::tree::{NoopCommit, PanicSource, RefWalker};

    fn make_3_node_tree() -> tree::Tree {
        let mut tree = tree::Tree::new(vec![5], vec![5])
            .attach(true, Some(tree::Tree::new(vec![3], vec![3])))
            .attach(false, Some(tree::Tree::new(vec![7], vec![7])));
        tree.commit(&mut NoopCommit {}).expect("commit failed");
        tree
    }

    fn make_7_node_prooftree() -> super::Tree {
        let make_node = |i| -> super::Tree { Node::KV(vec![i], vec![]).into() };

        let mut tree = make_node(3);
        let mut left = make_node(1);
        left.attach(true, make_node(0)).unwrap();
        left.attach(false, make_node(2)).unwrap();
        let mut right = make_node(5);
        right.attach(true, make_node(4)).unwrap();
        right.attach(false, make_node(6)).unwrap();
        tree.attach(true, left).unwrap();
        tree.attach(false, right).unwrap();

        tree
    }

    fn verify_test(keys: Vec<Vec<u8>>, expected_result: Vec<Option<Vec<u8>>>) {
        let mut tree = make_3_node_tree();
        let mut walker = RefWalker::new(&mut tree, PanicSource {});

        let (proof, _) = walker
            .create_proof(keys.as_slice())
            .expect("failed to create proof");
        let mut bytes = vec![];
        encode_into(proof.iter(), &mut bytes);

        let expected_hash = [
            65, 23, 96, 10, 165, 42, 240, 100, 206, 125, 192, 81, 44, 89, 119, 39, 35, 215, 211, 24,
        ];
        let result =
            verify_query(bytes.as_slice(), keys.as_slice(), expected_hash).expect("verify failed");
        assert_eq!(result, expected_result);
    }

    #[test]
    fn root_verify() {
        verify_test(vec![vec![5]], vec![Some(vec![5])]);
    }

    #[test]
    fn single_verify() {
        verify_test(vec![vec![3]], vec![Some(vec![3])]);
    }

    #[test]
    fn double_verify() {
        verify_test(vec![vec![3], vec![5]], vec![Some(vec![3]), Some(vec![5])]);
    }

    #[test]
    fn double_verify_2() {
        verify_test(vec![vec![3], vec![7]], vec![Some(vec![3]), Some(vec![7])]);
    }

    #[test]
    fn triple_verify() {
        verify_test(
            vec![vec![3], vec![5], vec![7]],
            vec![Some(vec![3]), Some(vec![5]), Some(vec![7])],
        );
    }

    #[test]
    fn left_edge_absence_verify() {
        verify_test(vec![vec![2]], vec![None]);
    }

    #[test]
    fn right_edge_absence_verify() {
        verify_test(vec![vec![8]], vec![None]);
    }

    #[test]
    fn inner_absence_verify() {
        verify_test(vec![vec![6]], vec![None]);
    }

    #[test]
    fn absent_and_present_verify() {
        verify_test(vec![vec![5], vec![6]], vec![Some(vec![5]), None]);
    }

    #[test]
    fn layer_iter() {
        let tree = make_7_node_prooftree();

        let assert_node = |node: &Tree, i| match node.node {
            Node::KV(ref key, _) => assert_eq!(key[0], i),
            _ => unreachable!(),
        };

        let mut iter = tree.layer(0);
        assert_node(iter.next().unwrap(), 3);
        assert!(iter.next().is_none());

        let mut iter = tree.layer(1);
        assert_node(iter.next().unwrap(), 1);
        assert_node(iter.next().unwrap(), 5);
        assert!(iter.next().is_none());

        let mut iter = tree.layer(2);
        assert_node(iter.next().unwrap(), 0);
        assert_node(iter.next().unwrap(), 2);
        assert_node(iter.next().unwrap(), 4);
        assert_node(iter.next().unwrap(), 6);
        assert!(iter.next().is_none());
    }

    #[test]
    fn visit_nodes() {
        let tree = make_7_node_prooftree();

        let assert_node = |node: Node, i| match node {
            Node::KV(ref key, _) => assert_eq!(key[0], i),
            _ => unreachable!(),
        };

        let mut visited = vec![];
        tree.visit_nodes(&mut |node| visited.push(node));

        let mut iter = visited.into_iter();
        for i in 0..7 {
            assert_node(iter.next().unwrap(), i);
        }
        assert!(iter.next().is_none());
    }
}
