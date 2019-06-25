// use std::fmt;

// use crate::*;
// use crate::node;

// const MAX_STACK_SIZE: usize = 50;

// #[derive(Serialize, Deserialize)]
// pub enum Op {
//   Push(Node),
//   Parent,
//   Child
// }

// impl fmt::Debug for Op {
//     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//         match self {
//             Op::Push(node) => {
//                 write!(f, "PUSH{:?}", node)
//             },
//             Op::Parent => write!(f, "PARENT"),
//             Op::Child => write!(f, "CHILD")
//         }
//     }
// }

// #[derive(Clone, Serialize, Deserialize)]
// pub enum Node {
//     Hash(Hash),
//     KVHash(Hash),
//     KV(Vec<u8>, Vec<u8>)
//     // TODO: fourth variant: (key, hash of value)
//     //       requires changing hashing scheme to kv_hash = H(key, H(value)),
//     //       to prevent sending long values in boundary nodes
// }

// impl fmt::Debug for Node {
//     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//         match self {
//             Node::Hash(hash) => {
//                 write!(f, "(hash={})", hex::encode(&hash[..6]))
//             },
//             Node::KVHash(kv_hash) => {
//                 write!(f, "(kv_hash={})", hex::encode(&kv_hash[..6]))
//             },
//             Node::KV(key, value) => {
//                 write!(
//                     f, "({}: {})",
//                     String::from_utf8(key.to_vec())
//                         .unwrap_or_else(|_| format!("{:?}", key)),
//                     String::from_utf8(value.to_vec())
//                         .unwrap_or_else(|_| format!("{:?}", value))
//                 )
//             }
//         }
//     }
// }

// pub struct Tree {
//     pub node: Node,
//     pub left: Option<Box<Tree>>,
//     pub right: Option<Box<Tree>>
// }

// impl Tree {
//     pub fn new(node: Node) -> Tree {
//         Tree {
//             node,
//             left: None,
//             right: None
//         }
//     }

//     pub fn hash(self) -> Hash {
//         match self.node {
//             Node::Hash(hash) => hash,
//             Node::KVHash(kv_hash) => node::hash(
//                 &kv_hash,
//                 self.left.map(|l| l.hash()).as_ref(),
//                 self.right.map(|r| r.hash()).as_ref()
//             ),
//             Node::KV(key, value) => node::hash(
//                 &node::kv_hash(&key, &value),
//                 self.left.map(|l| l.hash()).as_ref(),
//                 self.right.map(|r| r.hash()).as_ref()
//             )
//         }
//     }

//     pub fn with_left(mut self, child: Tree) -> Result<Tree> {
//         if self.left.is_some() {
//             bail!("Node already has left child");
//         }
//         if let Node::Hash(_) = self.node {
//             bail!("Cannot add child to hash-only node");
//         }
//         self.left = Some(Box::new(child));
//         Ok(self)
//     }

//     pub fn with_right(mut self, child: Tree) -> Result<Tree> {
//         if self.right.is_some() {
//             bail!("Node already has right child");
//         }
//         if let Node::Hash(_) = self.node {
//             bail!("Cannot add child to hash-only node");
//         }
//         self.right = Some(Box::new(child));
//         Ok(self)
//     }
// }

// pub fn create(
//     store: &Merk,
//     start: &[u8],
//     end: &[u8]
// ) -> Result<Vec<Op>> {
//     // TODO: get bounds
//     // TODO: remove prev_key, parent_stack (only for invariant assertion)
//     // TODO: can we do this without so many clones?

//     let mut proof = vec![];
//     let mut prev_key: Option<Vec<u8>> = None;
//     let mut child_stack: Vec<Vec<u8>> = vec![];
//     let mut parent_stack = vec![];
//     store.map_range(start, end, &mut |node: Node| {
//         let op = Op::Push(Node::KV(
//             node.key.clone(),
//             node.value.clone()
//         ));
//         proof.push(op);

//         if let Some(prev_key) = &prev_key {
//             // TODO: only emit Parent op if child is in range
//             if let Some(left_child) = &node.left {
//                 assert_eq!(
//                     &left_child.key, prev_key,
//                     "Expected left child to be previous node"
//                 );
//                 proof.push(Op::Parent);
//             }
//         }

//         let mut key = node.key.clone();
//         while let Some(pop_key) = child_stack.last() {
//             if key == *pop_key {
//                 proof.push(Op::Child);
//                 child_stack.pop();
//                 key = parent_stack.pop().unwrap();
//             } else {
//                 break;
//             }
//         }

//         if let Some(right_child) = &node.right {
//             child_stack.push(right_child.key.clone());
//             parent_stack.push(node.key.clone());
//         }

//         prev_key = Some(key);
//     })?;

//     Ok(proof)
// }

// pub fn verify(expected_hash: &Hash, proof: &[Op]) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
//     // TODO: verify recursively instead of with vector stack (probably better
//     //       for CPU cache locality)
//     let mut stack: Vec<Tree> =
//         Vec::with_capacity(MAX_STACK_SIZE / 2);
//     let mut entries: Vec<(Vec<u8>, Vec<u8>)> =
//         Vec::with_capacity(proof.len() / 2);

//     for op in proof {
//         match op {
//             Op::Push(node) => {
//                 if stack.len() >= MAX_STACK_SIZE {
//                     bail!("Stack exceeded maximum size");
//                 }

//                 if let Node::KV(key, value) = &node {
//                     if let Some((prev_key, _)) = entries.last() {
//                         if key <= prev_key {
//                             bail!("Invalid key ordering");
//                         }
//                     }
//                     entries.push((key.clone(), value.clone()));
//                 }

//                 let tree = Tree::new((*node).clone());
//                 stack.push(tree);
//             },
//             Op::Parent => {
//                 let mut pop = || stack.pop().expect("Expected node on stack");
//                 let top = pop();
//                 let bottom = pop();
//                 stack.push(top.with_left(bottom)?);
//             },
//             Op::Child => {
//                 let mut pop = || stack.pop().expect("Expected node on stack");
//                 let bottom = pop();
//                 let top = pop().with_right(bottom)?;
//                 // now this node is complete, just keep the hash
//                 let hash = top.hash();
//                 stack.push(Tree::new(Node::Hash(hash)));
//             }
//         }
//     }

//     if stack.len() != 1 {
//         bail!("Proof must end with exactly one tree");
//     }
//     let tree = stack.pop().unwrap();
//     if tree.hash() != *expected_hash {
//         bail!("Computed root hash does not match expected hash");
//     }

//     Ok(entries)
// }

// // pub fn reconstruct(expected_hash: &Hash, proof: &[Op]) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
// //     // TODO: verify recursively instead of with vector stack (probably better
// //     //       for CPU cache locality)
// //     let mut stack: Vec<Tree> =
// //         Vec::with_capacity(MAX_STACK_SIZE);
// //     let mut entries: Vec<(Vec<u8>, Vec<u8>)> =
// //         Vec::with_capacity(proof.len() / 2);
// //     let mut prev_key = None;
// //     for op in proof {
// //         match op {
// //             Op::Push(node) => match node {
// //                 Node::Hash(hash) => {
// //                     panic!("hash nodes not yet handled");
// //                 },
// //                 Node::KVHash(kv_hash) => {
// //                     panic!("kv_hash nodes not yet handled");
// //                 },
// //                 Node::KV(key, value) => {
// //                     if stack.len() >= MAX_STACK_SIZE {
// //                         bail!("Stack exceeded maximum size");
// //                     }
// //                     if let Some(prev_key) = &prev_key {
// //                         assert!(
// //                             key > prev_key,
// //                             "Invalid key ordering"
// //                         );
// //                     }
// //                     prev_key = Some(key.clone());

// //                     let tree = Tree::new(
// //                         crate::Node::new(&key, &value)
// //                     );
// //                     stack.push(tree);
// //                 }
// //             },
// //             Op::Parent => {
// //                 let mut top = stack.pop()
// //                     .expect("Expected node on stack");
// //                 if top.left.is_some() {
// //                     bail!("Got PARENT op for node that already has left child");
// //                 }
// //                 let bottom = stack.pop()
// //                     .expect("Expected node on stack");

// //                 // TODO: make Tree API cleaner
// //                 top.left = Some(Box::new(bottom));
// //                 top.update_link(true);

// //                 stack.push(top);
// //             },
// //             Op::Child => {
// //                 let bottom = stack.pop()
// //                     .expect("Expected node on stack");
// //                 let mut top = stack.pop()
// //                     .expect("Expected node on stack");
// //                 if top.right.is_some() {
// //                     bail!("Got CHILD op for node that already has right child");
// //                 }

// //                 top.right = Some(Box::new(bottom));
// //                 top.update_link(false);

// //                 stack.push(top);
// //             }
// //         }
// //     }

// //     assert_eq!(
// //         stack.len(), 1,
// //         "Proof must end with exactly one tree"
// //     );
// //     Ok(stack.pop().unwrap())
// // }

// pub fn encode(proof: &[Op]) -> Result<Vec<u8>> {
//     let bytes = bincode::serialize(proof)?;
//     Ok(bytes)
// }

// #[test]
// fn proof_debug() {
//     let debug = format!("{:?}", &[
//         Op::Child,
//         Op::Parent,
//         Op::Push(Node::KV(
//             vec![1,2,3],
//             vec![4,5,6]
//         )),
//         Op::Push(Node::Hash([1; HASH_LENGTH])),
//         Op::Push(Node::KVHash([2; HASH_LENGTH]))
//     ]);
//     assert_eq!(debug, "[CHILD, PARENT, PUSH(\u{1}\u{2}\u{3}: \u{4}\u{5}\u{6}), PUSH(hash=010101010101), PUSH(kv_hash=020202020202)]");
// }

// // #[test]
// // fn proof_reconstruct() {
// //     let proof = [
// //         Op::Push(Node::KV(vec![0], vec![123])),
// //         Op::Push(Node::KV(vec![1], vec![123])),
// //         Op::Parent,
// //         Op::Push(Node::KV(vec![2], vec![123])),
// //         Op::Child,
// //         Op::Push(Node::KV(vec![3], vec![123])),
// //         Op::Parent,
// //         Op::Push(Node::KV(vec![4], vec![123])),
// //         Op::Push(Node::KV(vec![5], vec![123])),
// //         Op::Child,
// //         Op::Parent
// //     ];

// //     let tree = reconstruct(&proof).unwrap();
// //     println!("{:?}", tree);
// // }

// #[test]
// fn proof_verify_simple() {
//     let proof = [
//         Op::Push(Node::KV(vec![0], vec![123])),
//         Op::Push(Node::KV(vec![1], vec![123])),
//         Op::Parent,
//         Op::Push(Node::KV(vec![2], vec![123])),
//         Op::Child,
//         Op::Push(Node::KV(vec![3], vec![123])),
//         Op::Parent,
//         Op::Push(Node::KV(vec![4], vec![123])),
//         Op::Push(Node::KV(vec![5], vec![123])),
//         Op::Child,
//         Op::Child
//     ];

//     let entries = verify(
//         &[41, 29, 90, 208, 171, 38, 227, 53, 179, 107, 233, 53, 159, 150, 22, 13, 108, 150, 150, 215],
//         &proof
//     ).unwrap();
//     assert_eq!(entries, vec![
//         (vec![0], vec![123]),
//         (vec![1], vec![123]),
//         (vec![2], vec![123]),
//         (vec![3], vec![123]),
//         (vec![4], vec![123]),
//         (vec![5], vec![123])
//     ]);
// }

// #[test]
// fn proof_encode() {
//     let proof = [
//         Op::Push(Node::KV(vec![0], vec![123])),
//         Op::Push(Node::KV(vec![1], vec![123])),
//         Op::Parent,
//         Op::Push(Node::KV(vec![2], vec![123])),
//         Op::Child,
//         Op::Push(Node::KV(vec![3], vec![123])),
//         Op::Parent,
//         Op::Push(Node::KV(vec![4], vec![123])),
//         Op::Push(Node::KV(vec![5], vec![123])),
//         Op::Child,
//         Op::Parent
//     ];

//     let bytes = encode(&proof).unwrap();
//     println!("{}", hex::encode(bytes));
// }