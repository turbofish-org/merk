use super::{Node, Op};
use std::collections::{LinkedList, BTreeSet};
use std::ops::{Range, RangeInclusive, RangeBounds, Bound};
use std::cmp::{Ordering, min, max};
use crate::error::Result;
use crate::tree::{Fetch, Link, RefWalker};

#[derive(Default)]
pub struct Query {
    items: BTreeSet<QueryItem>
}

impl Query {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn insert_key(&mut self, key: Vec<u8>) {
        let key = QueryItem::Key(key);
        self.items.insert(key);
    }

    pub fn insert_range(&mut self, range: Range<Vec<u8>>) {
        let range = QueryItem::Range(range);
        self.merge_or_insert(range);
    }

    pub fn insert_range_inclusive(&mut self, range: RangeInclusive<Vec<u8>>) {
        let range = QueryItem::RangeInclusive(range);
        self.merge_or_insert(range);
    }

    fn merge_or_insert(&mut self, mut item: QueryItem) {
        while let Some(existing) = self.items.get(&item) {
            let existing = existing.clone();
            self.items.remove(&existing);
            item = item.merge(existing);
        }

        self.items.insert(item);
    }
}

impl Into<Vec<QueryItem>> for Query {
    fn into(self) -> Vec<QueryItem> {
        self.items.into_iter().collect()
    }
}

#[derive(Clone, Debug)]
pub(crate) enum QueryItem {
    Key(Vec<u8>),
    Range(Range<Vec<u8>>),
    RangeInclusive(RangeInclusive<Vec<u8>>)
}

impl QueryItem {
    fn lower_bound(&self) -> Vec<u8> {
        match self {
            QueryItem::Key(key) => key.clone(),
            QueryItem::Range(range) => range.start.clone(),
            QueryItem::RangeInclusive(range) => range.start().clone()
        }
    }

    fn upper_bound(&self) -> (Vec<u8>, bool) {
        match self {
            QueryItem::Key(key) => (key.clone(), true),
            QueryItem::Range(range) => (range.end.clone(), false),
            QueryItem::RangeInclusive(range) => (range.end().clone(), true)
        }
    }

    fn merge(self, other: QueryItem) -> QueryItem {
        // TODO: don't copy into new vecs
        let start = min(self.lower_bound(), other.lower_bound()).to_vec();
        let end = max(self.upper_bound(), other.upper_bound());
        if end.1 {
            QueryItem::RangeInclusive(
                RangeInclusive::new(start, end.0.to_vec())
            )
        } else {
            QueryItem::Range(Range { start, end: end.0.to_vec() })
        }
    }
}

impl PartialEq for QueryItem {
    fn eq(&self, other: &QueryItem) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for QueryItem {}

impl Ord for QueryItem {
    fn cmp(&self, other: &QueryItem) -> Ordering {
        let cmp_lu = self.lower_bound().cmp(&other.upper_bound().0);
        let cmp_ul = self.upper_bound().0.cmp(&other.lower_bound());
        let self_inclusive = self.upper_bound().1;
        let other_inclusive = other.upper_bound().1;

        match (cmp_lu, cmp_ul) {
            (Ordering::Less, Ordering::Less) => Ordering::Less,
            (Ordering::Less, Ordering::Equal) => match self_inclusive {
                true => Ordering::Equal,
                false => Ordering::Less
            },
            (Ordering::Less, Ordering::Greater) => Ordering::Equal,
            (Ordering::Equal, _) => match other_inclusive {
                true => Ordering::Equal,
                false => Ordering::Greater
            },
            (Ordering::Greater, _) => Ordering::Greater
        }
    }
}

impl PartialOrd for QueryItem {
    fn partial_cmp(&self, other: &QueryItem) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Link {
    /// Creates a `Node::Hash` from this link. Panics if the link is of variant
    /// `Link::Modified` since its hash has not yet been computed.
    fn to_hash_node(&self) -> Node {
        let hash = match self {
            Link::Reference { hash, .. } => hash,
            Link::Modified { .. } => {
                panic!("Cannot convert Link::Modified to proof hash node");
            }
            Link::Uncommitted { hash, .. } => hash,
            Link::Loaded { hash, .. } => hash,
        };
        Node::Hash(*hash)
    }
}

impl<'a, S> RefWalker<'a, S>
where
    S: Fetch + Sized + Send + Clone,
{
    /// Creates a `Node::KV` from the key/value pair of the root node.
    pub(crate) fn to_kv_node(&self) -> Node {
        Node::KV(self.tree().key().to_vec(), self.tree().value().to_vec())
    }

    /// Creates a `Node::KVHash` from the hash of the key/value pair of the root
    /// node.
    pub(crate) fn to_kvhash_node(&self) -> Node {
        Node::KVHash(*self.tree().kv_hash())
    }

    /// Creates a `Node::Hash` from the hash of the node.
    pub(crate) fn to_hash_node(&self) -> Node {
        Node::Hash(self.tree().hash())
    }

    /// Generates a proof for the list of queried keys. Returns a tuple
    /// containing the generated proof operators, and a tuple representing if
    /// any keys were queried were less than the left edge or greater than the
    /// right edge, respectively.
    pub(crate) fn create_proof(
        &mut self,
        query: &[QueryItem],
    ) -> Result<(LinkedList<Op>, (bool, bool))> {
        // TODO: don't copy into vec, support comparing QI to byte slice
        let node_key = QueryItem::Key(self.tree().key().to_vec());
        let search = query.binary_search_by(
            |key| key.cmp(&node_key)
        );

        let (left_items, right_items) = match search {
            Ok(index) => {
                let item = &query[index];
                let left_bound = item.lower_bound();
                let right_bound = item.upper_bound().0;

                // if range starts before this node's key, include it in left
                // child's query
                let left_query = if left_bound.as_slice() < self.tree().key() {
                    &query[..=index]
                } else {
                    &query[..index]
                };

                // if range ends after this node's key, include it in right
                // child's query
                let right_query = if right_bound.as_slice() > self.tree().key() {
                    &query[index..]
                } else {
                    &query[index + 1..]
                };

                (left_query, right_query)
            },
            Err(index) => (&query[..index], &query[index..])
        };

        let (mut proof, left_absence) =
            self.create_child_proof(true, left_items)?;
        let (mut right_proof, right_absence) =
            self.create_child_proof(false, right_items)?;

        let (has_left, has_right) = (!proof.is_empty(), !right_proof.is_empty());

        proof.push_back(match search {
            Ok(_) => Op::Push(self.to_kv_node()),
            Err(_) => {
                if left_absence.1 || right_absence.0 {
                    Op::Push(self.to_kv_node())
                } else {
                    Op::Push(self.to_kvhash_node())
                }
            }
        });

        if has_left {
            proof.push_back(Op::Parent);
        }

        if has_right {
            proof.append(&mut right_proof);
            proof.push_back(Op::Child);
        }

        Ok((proof, (left_absence.0, right_absence.1)))
    }

    /// Similar to `create_proof`. Recurses into the child on the given side and
    /// generates a proof for the queried keys.
    fn create_child_proof(
        &mut self,
        left: bool,
        query: &[QueryItem]
    ) -> Result<(LinkedList<Op>, (bool, bool))> {
        Ok(if !query.is_empty() {
            if let Some(mut child) = self.walk(left)? {
                child.create_proof(query)?
            } else {
                (LinkedList::new(), (true, true))
            }
        } else if let Some(link) = self.tree().link(left) {
            let mut proof = LinkedList::new();
            proof.push_back(Op::Push(link.to_hash_node()));
            (proof, (false, false))
        } else {
            (LinkedList::new(), (false, false))
        })
    }
}

#[cfg(test)]
mod test {
    use super::super::encoding::encode_into;
    use super::*;
    use crate::tree::{Tree, PanicSource, RefWalker};
    use crate::test_utils::make_tree_seq;

    fn make_3_node_tree() -> Tree {
        Tree::from_fields(
            vec![5],
            vec![5],
            [105; 32],
            Some(Link::Loaded {
                child_heights: (0, 0),
                hash: [3; 32],
                tree: Tree::from_fields(vec![3], vec![3], [103; 32], None, None),
            }),
            Some(Link::Loaded {
                child_heights: (0, 0),
                hash: [7; 32],
                tree: Tree::from_fields(vec![7], vec![7], [107; 32], None, None),
            }),
        )
    }

    #[test]
    fn empty_proof() {
        let mut tree = make_3_node_tree();
        let mut walker = RefWalker::new(&mut tree, PanicSource {});

        let (proof, absence) = walker
            .create_proof(vec![].as_slice())
            .expect("create_proof errored");

        let mut iter = proof.iter();
        assert_eq!(iter.next(), Some(&Op::Push(Node::Hash([3; 32]))));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KVHash([105; 32]))));
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(iter.next(), Some(&Op::Push(Node::Hash([7; 32]))));
        assert_eq!(iter.next(), Some(&Op::Child));
        assert!(iter.next().is_none());
        assert_eq!(absence, (false, false));
    }

    #[test]
    fn root_proof() {
        let mut tree = make_3_node_tree();
        let mut walker = RefWalker::new(&mut tree, PanicSource {});

        let (proof, absence) = walker
            .create_proof(vec![QueryItem::Key(vec![5])].as_slice())
            .expect("create_proof errored");

        let mut iter = proof.iter();
        assert_eq!(iter.next(), Some(&Op::Push(Node::Hash([3; 32]))));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![5], vec![5]))));
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(iter.next(), Some(&Op::Push(Node::Hash([7; 32]))));
        assert_eq!(iter.next(), Some(&Op::Child));
        assert!(iter.next().is_none());
        assert_eq!(absence, (false, false));
    }

    #[test]
    fn leaf_proof() {
        let mut tree = make_3_node_tree();
        let mut walker = RefWalker::new(&mut tree, PanicSource {});

        let (proof, absence) = walker
            .create_proof(vec![QueryItem::Key(vec![3])].as_slice())
            .expect("create_proof errored");

        let mut iter = proof.iter();
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![3], vec![3]))));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KVHash([105; 32]))));
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(iter.next(), Some(&Op::Push(Node::Hash([7; 32]))));
        assert_eq!(iter.next(), Some(&Op::Child));
        assert!(iter.next().is_none());
        assert_eq!(absence, (false, false));
    }

    #[test]
    fn double_leaf_proof() {
        let mut tree = make_3_node_tree();
        let mut walker = RefWalker::new(&mut tree, PanicSource {});

        let (proof, absence) = walker
            .create_proof(vec![
                QueryItem::Key(vec![3]),
                QueryItem::Key(vec![7])
            ].as_slice())
            .expect("create_proof errored");

        let mut iter = proof.iter();
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![3], vec![3]))));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KVHash([105; 32]))));
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![7], vec![7]))));
        assert_eq!(iter.next(), Some(&Op::Child));
        assert!(iter.next().is_none());
        assert_eq!(absence, (false, false));
    }

    #[test]
    fn all_nodes_proof() {
        let mut tree = make_3_node_tree();
        let mut walker = RefWalker::new(&mut tree, PanicSource {});

        let (proof, absence) = walker
            .create_proof(vec![
                QueryItem::Key(vec![3]),
                QueryItem::Key(vec![5]),
                QueryItem::Key(vec![7])
            ].as_slice())
            .expect("create_proof errored");

        let mut iter = proof.iter();
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![3], vec![3]))));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![5], vec![5]))));
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![7], vec![7]))));
        assert_eq!(iter.next(), Some(&Op::Child));
        assert!(iter.next().is_none());
        assert_eq!(absence, (false, false));
    }

    #[test]
    fn global_edge_absence_proof() {
        let mut tree = make_3_node_tree();
        let mut walker = RefWalker::new(&mut tree, PanicSource {});

        let (proof, absence) = walker
            .create_proof(vec![QueryItem::Key(vec![8])].as_slice())
            .expect("create_proof errored");

        let mut iter = proof.iter();
        assert_eq!(iter.next(), Some(&Op::Push(Node::Hash([3; 32]))));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KVHash([105; 32]))));
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![7], vec![7]))));
        assert_eq!(iter.next(), Some(&Op::Child));
        assert!(iter.next().is_none());
        assert_eq!(absence, (false, true));
    }

    #[test]
    fn absence_proof() {
        let mut tree = make_3_node_tree();
        let mut walker = RefWalker::new(&mut tree, PanicSource {});

        let (proof, absence) = walker
            .create_proof(vec![QueryItem::Key(vec![6])].as_slice())
            .expect("create_proof errored");

        let mut iter = proof.iter();
        assert_eq!(iter.next(), Some(&Op::Push(Node::Hash([3; 32]))));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![5], vec![5]))));
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![7], vec![7]))));
        assert_eq!(iter.next(), Some(&Op::Child));
        assert!(iter.next().is_none());
        assert_eq!(absence, (false, false));
    }

    #[test]
    fn doc_proof() {
        let mut tree = Tree::from_fields(
            vec![5],
            vec![5],
            [105; 32],
            Some(Link::Loaded {
                child_heights: (0, 0),
                hash: [2; 32],
                tree: Tree::from_fields(
                    vec![2],
                    vec![2],
                    [102; 32],
                    Some(Link::Loaded {
                        child_heights: (0, 0),
                        hash: [1; 32],
                        tree: Tree::from_fields(vec![1], vec![1], [101; 32], None, None),
                    }),
                    Some(Link::Loaded {
                        child_heights: (0, 0),
                        hash: [4; 32],
                        tree: Tree::from_fields(
                            vec![4],
                            vec![4],
                            [104; 32],
                            Some(Link::Loaded {
                                child_heights: (0, 0),
                                hash: [3; 32],
                                tree: Tree::from_fields(vec![3], vec![3], [103; 32], None, None),
                            }),
                            None,
                        ),
                    }),
                ),
            }),
            Some(Link::Loaded {
                child_heights: (0, 0),
                hash: [9; 32],
                tree: Tree::from_fields(
                    vec![9],
                    vec![9],
                    [109; 32],
                    Some(Link::Loaded {
                        child_heights: (0, 0),
                        hash: [7; 32],
                        tree: Tree::from_fields(
                            vec![7],
                            vec![7],
                            [107; 32],
                            Some(Link::Loaded {
                                child_heights: (0, 0),
                                hash: [6; 32],
                                tree: Tree::from_fields(vec![6], vec![6], [106; 32], None, None),
                            }),
                            Some(Link::Loaded {
                                child_heights: (0, 0),
                                hash: [8; 32],
                                tree: Tree::from_fields(vec![8], vec![8], [108; 32], None, None),
                            }),
                        ),
                    }),
                    Some(Link::Loaded {
                        child_heights: (0, 0),
                        hash: [11; 32],
                        tree: Tree::from_fields(
                            vec![11],
                            vec![11],
                            [111; 32],
                            Some(Link::Loaded {
                                child_heights: (0, 0),
                                hash: [10; 32],
                                tree: Tree::from_fields(vec![10], vec![10], [110; 32], None, None),
                            }),
                            None,
                        ),
                    }),
                ),
            }),
        );
        let mut walker = RefWalker::new(&mut tree, PanicSource {});

        let (proof, absence) = walker.create_proof(vec![
            QueryItem::Key(vec![1]),
            QueryItem::Key(vec![2]),
            QueryItem::Key(vec![3]),
            QueryItem::Key(vec![4])
        ].as_slice()).expect("create_proof errored");

        let mut iter = proof.iter();
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![1], vec![1]))));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![2], vec![2]))));
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![3], vec![3]))));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![4], vec![4]))));
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(iter.next(), Some(&Op::Child));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KVHash([105; 32]))));
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(iter.next(), Some(&Op::Push(Node::Hash([9; 32]))));
        assert_eq!(iter.next(), Some(&Op::Child));
        assert!(iter.next().is_none());
        assert_eq!(absence, (false, false));

        let mut bytes = vec![];
        encode_into(proof.iter(), &mut bytes);
        assert_eq!(
            bytes,
            vec![
                3, 1, 1, 0, 1, 1, 3, 1, 2, 0, 1, 2, 16, 3, 1, 3, 0, 1, 3, 3, 1, 4, 0, 1, 4, 16, 17,
                2, 105, 105, 105, 105, 105, 105, 105, 105, 105, 105, 105, 105, 105, 105, 105, 105,
                105, 105, 105, 105, 105, 105, 105, 105, 105, 105, 105, 105, 105, 105, 105, 105, 16,
                1, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9,
                9, 9, 9, 9, 9, 17
            ]
        );
    }

    #[test]
    fn query_item_cmp() {
        assert!(QueryItem::Key(vec![10]) < QueryItem::Key(vec![20]));
        assert!(QueryItem::Key(vec![10]) == QueryItem::Key(vec![10]));
        assert!(QueryItem::Key(vec![20]) > QueryItem::Key(vec![10]));

        assert!(QueryItem::Key(vec![10]) < QueryItem::Range(vec![20]..vec![30]));
        assert!(QueryItem::Key(vec![10]) == QueryItem::Range(vec![10]..vec![20]));
        assert!(QueryItem::Key(vec![15]) == QueryItem::Range(vec![10]..vec![20]));
        assert!(QueryItem::Key(vec![20]) > QueryItem::Range(vec![10]..vec![20]));
        assert!(QueryItem::Key(vec![20]) == QueryItem::RangeInclusive(vec![10]..=vec![20]));
        assert!(QueryItem::Key(vec![30]) > QueryItem::Range(vec![10]..vec![20]));

        assert!(QueryItem::Range(vec![10]..vec![20]) < QueryItem::Range(vec![30]..vec![40]));
        assert!(QueryItem::Range(vec![10]..vec![20]) < QueryItem::Range(vec![20]..vec![30]));
        assert!(QueryItem::RangeInclusive(vec![10]..=vec![20]) == QueryItem::Range(vec![20]..vec![30]));
        assert!(QueryItem::Range(vec![15]..vec![25]) == QueryItem::Range(vec![20]..vec![30]));
        assert!(QueryItem::Range(vec![20]..vec![30]) > QueryItem::Range(vec![10]..vec![20]));
    }

    #[test]
    fn query_item_merge() {
        let mine = QueryItem::Range(vec![10]..vec![30]);
        let other = QueryItem::Range(vec![15]..vec![20]);
        assert_eq!(mine.merge(other), QueryItem::Range(vec![10]..vec![30]));

        let mine = QueryItem::RangeInclusive(vec![10]..=vec![30]);
        let other = QueryItem::Range(vec![20]..vec![30]);
        assert_eq!(mine.merge(other), QueryItem::RangeInclusive(vec![10]..=vec![30]));

        let mine = QueryItem::Key(vec![5]);
        let other = QueryItem::Range(vec![1]..vec![10]);
        assert_eq!(mine.merge(other), QueryItem::Range(vec![1]..vec![10]));
        
        let mine = QueryItem::Key(vec![10]);
        let other = QueryItem::RangeInclusive(vec![1]..=vec![10]);
        assert_eq!(mine.merge(other), QueryItem::RangeInclusive(vec![1]..=vec![10]));
    }
    
    #[test]
    fn query_insert() {
        let mut query = Query::new();
        query.insert_key(vec![2]);
        query.insert_range(vec![3]..vec![5]);
        query.insert_range_inclusive(vec![5]..=vec![7]);
        query.insert_range(vec![4]..vec![6]);
        query.insert_key(vec![5]);

        let mut iter = query.items.iter();
        assert_eq!(format!("{:?}", iter.next()), "Some(Key([2]))");
        assert_eq!(format!("{:?}", iter.next()), "Some(RangeInclusive([3]..=[7]))");
        assert_eq!(iter.next(), None);
    }
    
    #[test]
    fn range_proof() {
        let mut tree = make_tree_seq(10);
        let mut walker = RefWalker::new(&mut tree, PanicSource {});

        let (proof, absence) = walker
            .create_proof(vec![
                QueryItem::Range(vec![0,0,0,0,0,0,0,5]..vec![0,0,0,0,0,0,0,7])
            ].as_slice())
            .expect("create_proof errored");

        let mut iter = proof.iter();
        assert_eq!(iter.next(), Some(&Op::Push(Node::Hash([204, 255, 35, 67, 8, 211, 132, 121, 65, 159, 55, 183, 110, 86, 97, 211, 150, 0, 254, 205]))));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KVHash([224, 252, 14, 4, 3, 65, 193, 166, 134, 97, 239, 90, 154, 161, 123, 126, 48, 33, 15, 69]))));
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(iter.next(), Some(&Op::Push(Node::Hash([139, 133, 223, 134, 15, 5, 20, 201, 160, 238, 34, 170, 157, 157, 191, 12, 66, 74, 98, 109]))));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![0,0,0,0,0,0,0,5], vec![123; 60]))));
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![0,0,0,0,0,0,0,6], vec![123; 60]))));
        assert_eq!(iter.next(), Some(&Op::Child));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![0,0,0,0,0,0,0,7], vec![123; 60]))));
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(iter.next(), Some(&Op::Push(Node::Hash([36, 71, 231, 173, 167, 169, 98, 105, 190, 22, 250, 41, 176, 144, 249, 78, 233, 154, 223, 221]))));
        assert_eq!(iter.next(), Some(&Op::Child));
        assert_eq!(iter.next(), Some(&Op::Child));
        assert!(iter.next().is_none());
        assert_eq!(absence, (false, false));
    }

    #[test]
    fn range_proof_inclusive() {
        let mut tree = make_tree_seq(10);
        let mut walker = RefWalker::new(&mut tree, PanicSource {});

        let (proof, absence) = walker
            .create_proof(vec![
                QueryItem::RangeInclusive(vec![0,0,0,0,0,0,0,5]..=vec![0,0,0,0,0,0,0,7])
            ].as_slice())
            .expect("create_proof errored");

        let mut iter = proof.iter();
        assert_eq!(iter.next(), Some(&Op::Push(Node::Hash([204, 255, 35, 67, 8, 211, 132, 121, 65, 159, 55, 183, 110, 86, 97, 211, 150, 0, 254, 205]))));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KVHash([224, 252, 14, 4, 3, 65, 193, 166, 134, 97, 239, 90, 154, 161, 123, 126, 48, 33, 15, 69]))));
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(iter.next(), Some(&Op::Push(Node::Hash([139, 133, 223, 134, 15, 5, 20, 201, 160, 238, 34, 170, 157, 157, 191, 12, 66, 74, 98, 109]))));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![0,0,0,0,0,0,0,5], vec![123; 60]))));
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![0,0,0,0,0,0,0,6], vec![123; 60]))));
        assert_eq!(iter.next(), Some(&Op::Child));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![0,0,0,0,0,0,0,7], vec![123; 60]))));
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(iter.next(), Some(&Op::Push(Node::Hash([36, 71, 231, 173, 167, 169, 98, 105, 190, 22, 250, 41, 176, 144, 249, 78, 233, 154, 223, 221]))));
        assert_eq!(iter.next(), Some(&Op::Child));
        assert_eq!(iter.next(), Some(&Op::Child));
        assert!(iter.next().is_none());
        assert_eq!(absence, (false, false));
    }
    
    #[test]
    fn range_proof_missing_upper_bound() {
        let mut tree = make_tree_seq(10);
        let mut walker = RefWalker::new(&mut tree, PanicSource {});

        let (proof, absence) = walker
            .create_proof(vec![
                QueryItem::Range(vec![0,0,0,0,0,0,0,5]..vec![0,0,0,0,0,0,0,6,5])
            ].as_slice())
            .expect("create_proof errored");

        let mut iter = proof.iter();
        assert_eq!(iter.next(), Some(&Op::Push(Node::Hash([204, 255, 35, 67, 8, 211, 132, 121, 65, 159, 55, 183, 110, 86, 97, 211, 150, 0, 254, 205]))));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KVHash([224, 252, 14, 4, 3, 65, 193, 166, 134, 97, 239, 90, 154, 161, 123, 126, 48, 33, 15, 69]))));
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(iter.next(), Some(&Op::Push(Node::Hash([139, 133, 223, 134, 15, 5, 20, 201, 160, 238, 34, 170, 157, 157, 191, 12, 66, 74, 98, 109]))));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![0,0,0,0,0,0,0,5], vec![123; 60]))));
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![0,0,0,0,0,0,0,6], vec![123; 60]))));
        assert_eq!(iter.next(), Some(&Op::Child));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![0,0,0,0,0,0,0,7], vec![123; 60]))));
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(iter.next(), Some(&Op::Push(Node::Hash([36, 71, 231, 173, 167, 169, 98, 105, 190, 22, 250, 41, 176, 144, 249, 78, 233, 154, 223, 221]))));
        assert_eq!(iter.next(), Some(&Op::Child));
        assert_eq!(iter.next(), Some(&Op::Child));
        assert!(iter.next().is_none());
        assert_eq!(absence, (false, false));
    }

    #[test]
    fn range_proof_missing_lower_bound() {
        let mut tree = make_tree_seq(10);
        let mut walker = RefWalker::new(&mut tree, PanicSource {});

        let (proof, absence) = walker
            .create_proof(vec![
                // 7 is not inclusive
                QueryItem::Range(vec![0,0,0,0,0,0,0,5,5]..vec![0,0,0,0,0,0,0,7])
            ].as_slice())
            .expect("create_proof errored");

        let mut iter = proof.iter();
        assert_eq!(iter.next(), Some(&Op::Push(Node::Hash([204, 255, 35, 67, 8, 211, 132, 121, 65, 159, 55, 183, 110, 86, 97, 211, 150, 0, 254, 205]))));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KVHash([224, 252, 14, 4, 3, 65, 193, 166, 134, 97, 239, 90, 154, 161, 123, 126, 48, 33, 15, 69]))));
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(iter.next(), Some(&Op::Push(Node::Hash([139, 133, 223, 134, 15, 5, 20, 201, 160, 238, 34, 170, 157, 157, 191, 12, 66, 74, 98, 109]))));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![0,0,0,0,0,0,0,5], vec![123; 60]))));
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![0,0,0,0,0,0,0,6], vec![123; 60]))));
        assert_eq!(iter.next(), Some(&Op::Child));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![0,0,0,0,0,0,0,7], vec![123; 60]))));
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(iter.next(), Some(&Op::Push(Node::Hash([36, 71, 231, 173, 167, 169, 98, 105, 190, 22, 250, 41, 176, 144, 249, 78, 233, 154, 223, 221]))));
        assert_eq!(iter.next(), Some(&Op::Child));
        assert_eq!(iter.next(), Some(&Op::Child));
        assert!(iter.next().is_none());
        assert_eq!(absence, (false, false));
    }
}
