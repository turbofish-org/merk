use super::{Op, Node};
use crate::tree::{NULL_HASH, Hash, kv_hash, node_hash};
use crate::error::Result;

struct Tree {
    node: Node,
    left: Option<Box<Tree>>,
    right: Option<Box<Tree>>
}

impl From<Node> for Tree {
    fn from(node: Node) -> Self {
        Tree { node, left: None, right: None }
    }
}

impl Tree {
    fn child(&self, left: bool) -> Option<&Box<Tree>> {
        if left {
            self.left.as_ref()
        } else {
            self.right.as_ref()
        }
    }

    fn child_mut(&mut self, left: bool) -> &mut Option<Box<Tree>> {
        if left {
            &mut self.left
        } else {
            &mut self.right
        }
    }

    fn attach(&mut self, left: bool, child: Tree) -> Result<()> {
        if self.child(left).is_some() {
            bail!("Tried to attach to left child, but it is already Some");
        }

        let child = child.into_hash();
        let boxed = Box::new(child);
        *self.child_mut(left) = Some(boxed);
        Ok(())
    }

    #[inline]
    fn hash(&self) -> Hash {
        match self.node {
            Node::Hash(hash) => hash,
            _ => unreachable!("Expected Node::Hash")
        }
    }

    #[inline]
    fn child_hash(&self, left: bool) -> Hash {
        self.child(left)
            .map_or(NULL_HASH, |c| c.hash())
    }

    fn into_hash(self) -> Tree {
        fn to_hash_node(tree: &Tree, kv_hash: Hash) -> Node {
            let hash = node_hash(
                &kv_hash,
                &tree.child_hash(true),
                &tree.child_hash(false)
            );
            Node::Hash(hash)
        }

        match &self.node {
            Node::Hash(hash) => self.node,
            Node::KVHash(kv_hash) => to_hash_node(&self, *kv_hash),
            Node::KV(key, value) => {
                let kv_hash = kv_hash(key.as_slice(), value.as_slice());
                to_hash_node(&self, kv_hash)
            }
        }.into()
    }
}

pub fn verify(
    bytes: &[u8],
    keys: &[Vec<u8>],
    expected_hash: Hash
) -> Result<Vec<Option<Vec<u8>>>> {
    // TODO: enforce a maximum proof size

    let mut stack: Vec<Tree> = Vec::with_capacity(32);
    let mut output = Vec::with_capacity(keys.len());

    let mut key_index = 0;
    let mut last_push = None;

    fn try_pop(stack: &mut Vec<Tree>) -> Result<Tree> {
        match stack.pop() {
            None => bail!("Stack underflow"),
            Some(tree) => Ok(tree)
        }
    };

    let mut offset = 0;
    loop {
        if bytes.len() <= offset {
            break;
        }

        let op = Op::decode(&bytes[offset..])?;
        offset += op.encoding_length();

        match op {
            Op::Parent => {
                let (mut parent, child) = (
                    try_pop(&mut stack)?,
                    try_pop(&mut stack)?
                );
                parent.attach(true, child)?;
                stack.push(parent);
            },
            Op::Child => {
                let (child, mut parent) = (
                    try_pop(&mut stack)?,
                    try_pop(&mut stack)?
                );
                parent.attach(false, child)?;
                stack.push(parent);
            },
            Op::Push(node) => {
                let node_clone = node.clone();
                let tree: Tree = node.into();
                stack.push(tree);

                if let Node::KV(key, value) = &node_clone {
                    // keys should always be increasing
                    if let Some(Node::KV(last_key, _)) = &last_push {
                        if key <= &last_key {
                            bail!("Incorrect key ordering");
                        }
                    }

                    loop {
                        if key_index >= keys.len() || key < &keys[key_index] {
                            break;
                        } else if key == &keys[key_index] {
                            // KV for queried key
                            output.push(Some(value.clone()));
                        } else if key > &keys[key_index] {
                            match &last_push {
                                None | Some(Node::KV(_, _)) => {
                                    // previous push was a boundary (global edge or lower key),
                                    // so this is a valid absence proof
                                    output.push(None);
                                },
                                // proof is incorrect since it skipped queried keys
                                _ => bail!("Proof incorrectly formed")
                            }
                        }

                        key_index += 1;
                    }
                }

                last_push = Some(node_clone);
            }
        }
    }

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

    if stack.len() != 1 {
        bail!("Expected proof to result in exactly one stack item");
    }

    let root = stack.pop().unwrap();
    let hash = root.into_hash().hash();
    if hash != expected_hash {
        bail!(
            "Proof did not match expected hash\n\tExpected: {:?}\n\tActual: {:?}",
            expected_hash, hash
        );
    }

    Ok(output)
}

#[cfg(test)]
mod test {
    use super::*;
    use super::super::*;
    use crate::tree::{NoopCommit, RefWalker, PanicSource};
    use crate::tree;

    fn make_3_node_tree() -> tree::Tree {
        let mut tree = tree::Tree::new(vec![5], vec![5])
            .attach(true, Some(tree::Tree::new(vec![3], vec![3])))
            .attach(false, Some(tree::Tree::new(vec![7], vec![7])));
        tree.commit(&mut NoopCommit {})
            .expect("commit failed");
        tree
    }

    fn verify_test(keys: Vec<Vec<u8>>, expected_result: Vec<Option<Vec<u8>>>) {
        let mut tree = make_3_node_tree();
        let mut walker = RefWalker::new(&mut tree, PanicSource {});

        let (proof, _) = walker.create_proof(keys.as_slice())
            .expect("failed to create proof");
        let mut bytes = vec![];
        encode_into(proof.iter(), &mut bytes);

        let expected_hash = [65, 23, 96, 10, 165, 42, 240, 100, 206, 125, 192, 81, 44, 89, 119, 39, 35, 215, 211, 24];
        let result = verify(bytes.as_slice(), keys.as_slice(), expected_hash)
            .expect("verify failed");
        assert_eq!(result, expected_result);
    }

    #[test]
    fn root_verify() {
        verify_test(vec![ vec![5] ], vec![ Some(vec![5]) ]);
    }

    #[test]
    fn single_verify() {
        verify_test(vec![ vec![3] ], vec![ Some(vec![3]) ]);
    }

    #[test]
    fn double_verify() {
       verify_test(
           vec![ vec![3], vec![5] ],
           vec![ Some(vec![3]), Some(vec![5]) ]
        );
    }

    #[test]
    fn double_verify_2() {
       verify_test(
           vec![ vec![3], vec![7] ],
           vec![ Some(vec![3]), Some(vec![7]) ]
        );
    }

    #[test]
    fn triple_verify() {
       verify_test(
           vec![ vec![3], vec![5], vec![7] ],
           vec![ Some(vec![3]), Some(vec![5]), Some(vec![7]) ]
        );
    }

    #[test]
    fn left_edge_absence_verify() {
       verify_test(
           vec![ vec![2] ],
           vec![ None ]
        );
    }

    #[test]
    fn right_edge_absence_verify() {
       verify_test(
           vec![ vec![8] ],
           vec![ None ]
        );
    }

    #[test]
    fn inner_absence_verify() {
       verify_test(
           vec![ vec![6] ],
           vec![ None ]
        );
    }

    #[test]
    fn absent_and_present_verify() {
       verify_test(
           vec![ vec![5], vec![6] ],
           vec![ Some(vec![5]), None ]
        );
    }
}

