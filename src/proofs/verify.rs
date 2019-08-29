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
    fn child(&self, left: bool) -> Option<&Tree> {
        if left {
            self.left.as_ref()
        } else {
            self.right.as_ref()
        }
    }

    fn child_mut(&mut self, left: bool) -> Option<&mut Tree> {
        if left {
            self.left.as_mut()
        } else {
            self.right.as_mut()
        }
    }

    fn attach(&mut self, left: bool, child: Tree) {
        if self.child(left).is_some() {
            bail!("Tried to attach to left child, but it is already Some");
        }

        let child = child.into_hash();
        let boxed = Box::new(child);
        *self.child_mut(left) = Some(boxed);
    }

    #[inline]
    fn hash(&self) -> &Hash {
        match self.node {
            Node::Hash(hash) => &hash,
            _ => unreachable!("Expected Node::Hash")
        }
    }

    #[inline]
    fn child_hash(&self, left: bool) -> Option<&Hash> {
        self.child(left)
            .map_or(NULL_HASH, |c| c.hash())
    }

    fn into_hash(self) -> Tree {
        let to_hash_node = |kv_hash| {
            let hash = node_hash(
                &kv_hash,
                self.child_hash(true),
                self.child_hash(false)
            );
            Node::Hash(hash)
        };

        match self.node {
            Node::Hash(hash) => self,
            Node::KVHash(kv_hash) => to_hash_node(kv_hash),
            Node::KV(key, value) => {
                let kv_hash = kv_hash(key.as_slice(), value.as_slice());
                to_hash_node(kv_hash)
            }
        }
    }
}

pub fn verify(
    bytes: &[u8],
    keys: &[Vec<u8>],
    expected_hash: Hash
) -> Result<Vec<Option<Vec<u8>>>> {
    // TODO: enforce a maximum proof size

    let mut stack = Vec::with_capacity(32);
    let mut output = Vec::with_capacity(keys.len());

    let mut key_index = 0;
    let mut last_push = None;

    let try_pop = || {
        match stack.pop() {
            None => bail!("Stack underflow"),
            Some(tree) => tree
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
                let (parent, child) = (try_pop()?, try_pop()?);
                parent.attach(true, child);
                stack.push(parent);
            },
            Op::Child => {
                let (child, parent) = (try_pop()?, try_pop()?);
                parent.attach(false, child);
                stack.push(parent);
            },
            Op::Push(node) => {
                let node_clone = node.clone();
                let tree = node.into::<Tree>();
                stack.push(tree);

                if let Node::KV(key, value) = node_clone {
                    // keys should always be increasing
                    if let Some(Node::KV(last_key, _)) = last_push {
                        if key <= last_key {
                            bail!("Incorrect key ordering");
                        }
                    }

                    loop {
                        if key == keys[key_index] {
                            // KV for queried key
                            output.push(Some(value.clone()));
                        } else if key > keys[key_index] {
                            match last_push {
                                None | Node::KV(_, _) => {
                                    // previous push was a boundary (global edge or lower key),
                                    // so this is a valid absence proof
                                    output.push(None);
                                },
                                // proof is incorrect since it skipped queried keys
                                _ => bail!("Proof incorrectly formed");
                            }
                        } else {
                            break;
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

    let root = stack[0];
    if root.hash() != expected_hash {
        bail!("Proof did not match expected hash");
    }

    Ok(output)
}

#[cfg(test)]
mod test {
    use super::*;
}

