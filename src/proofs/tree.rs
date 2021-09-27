use super::{Node, Op};
use crate::error::{Error, Result};
use crate::tree::{kv_hash, node_hash, Hash, NULL_HASH};

/// Contains a tree's child node and its hash. The hash can always be assumed to
/// be up-to-date.
#[derive(Debug)]
pub struct Child {
    pub tree: Box<Tree>,
    pub hash: Hash,
}

/// A binary tree data structure used to represent a select subset of a tree
/// when verifying Merkle proofs.
#[derive(Debug)]
pub struct Tree {
    pub node: Node,
    pub left: Option<Child>,
    pub right: Option<Child>,
    pub height: usize,
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
    /// Checks equality for the root hashes of the two trees.
    fn eq(&self, other: &Self) -> bool {
        self.hash() == other.hash()
    }
}

impl Tree {
    /// Gets or computes the hash for this tree node.
    pub fn hash(&self) -> Hash {
        fn compute_hash(tree: &Tree, kv_hash: Hash) -> Hash {
            node_hash(&kv_hash, &tree.child_hash(true), &tree.child_hash(false))
        }

        match &self.node {
            Node::Hash(hash) => *hash,
            Node::KVHash(kv_hash) => compute_hash(self, *kv_hash),
            Node::KV(key, value) => {
                let kv_hash = kv_hash(key.as_slice(), value.as_slice());
                compute_hash(self, kv_hash)
            }
        }
    }

    /// Creates an iterator that yields the in-order traversal of the nodes at
    /// the given depth.
    pub fn layer(&self, depth: usize) -> LayerIter {
        LayerIter::new(self, depth)
    }

    /// Consumes the `Tree` and does an in-order traversal over all the nodes in
    /// the tree, calling `visit_node` for each.
    pub fn visit_nodes<F: FnMut(Node)>(mut self, visit_node: &mut F) {
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
    pub fn visit_refs<F: FnMut(&Tree)>(&self, visit_node: &mut F) {
        if let Some(child) = &self.left {
            child.tree.visit_refs(visit_node);
        }

        visit_node(self);

        if let Some(child) = &self.right {
            child.tree.visit_refs(visit_node);
        }
    }

    /// Returns an immutable reference to the child on the given side, if any.
    pub fn child(&self, left: bool) -> Option<&Child> {
        if left {
            self.left.as_ref()
        } else {
            self.right.as_ref()
        }
    }

    /// Returns a mutable reference to the child on the given side, if any.
    pub(crate) fn child_mut(&mut self, left: bool) -> &mut Option<Child> {
        if left {
            &mut self.left
        } else {
            &mut self.right
        }
    }

    /// Attaches the child to the `Tree`'s given side. Panics if there is
    /// already a child attached to this side.
    pub(crate) fn attach(&mut self, left: bool, child: Tree) -> Result<()> {
        if self.child(left).is_some() {
            return Err(Error::Attach(
                "Tried to attach to left child, but it is already Some".into(),
            ));
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

    #[cfg(feature = "full")]
    pub(crate) fn key(&self) -> &[u8] {
        match self.node {
            Node::KV(ref key, _) => key,
            _ => panic!("Expected node to be type KV"),
        }
    }
}

/// `LayerIter` iterates over the nodes in a `Tree` at a given depth. Nodes are
/// visited in order.
pub struct LayerIter<'a> {
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
            None => Err(Error::StackUnderflow),
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
                            return Err(Error::Key("Incorrect key ordering".into()));
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
        return Err(Error::Proof(
            "Expected proof to result in exactly on stack item".into(),
        ));
    }

    Ok(stack.pop().unwrap())
}

#[cfg(test)]
mod test {
    use super::super::*;
    use super::Tree as ProofTree;
    use super::*;

    fn make_7_node_prooftree() -> ProofTree {
        let make_node = |i| -> super::super::tree::Tree { Node::KV(vec![i], vec![]).into() };

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

    #[test]
    fn height_counting() {
        fn recurse(tree: &super::Tree, expected_height: usize) {
            assert_eq!(tree.height, expected_height);
            tree.left
                .as_ref()
                .map(|l| recurse(&l.tree, expected_height - 1));
            tree.right
                .as_ref()
                .map(|r| recurse(&r.tree, expected_height - 1));
        }

        let tree = make_7_node_prooftree();
        recurse(&tree, 3);
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
