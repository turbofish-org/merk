mod map;

#[cfg(feature = "full")]
use {super::Op, std::collections::LinkedList};

use super::tree::execute;
use super::{Decoder, Node};
use crate::error::{Error, Result};
use crate::tree::{Fetch, Hash, Link, RefWalker};
use std::cmp::{max, min, Ordering};
use std::collections::BTreeSet;
use std::ops::{Range, RangeInclusive};

pub use map::*;

/// `Query` represents one or more keys or ranges of keys, which can be used to
/// resolve a proof which will include all of the requested values.
#[derive(Default)]
pub struct Query {
    items: BTreeSet<QueryItem>,
}

impl Query {
    /// Creates a new query which contains no items.
    pub fn new() -> Self {
        Default::default()
    }

    pub(crate) fn len(&self) -> usize {
        self.items.len()
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &QueryItem> {
        self.items.iter()
    }

    /// Adds an individual key to the query, so that its value (or its absence)
    /// in the tree will be included in the resulting proof.
    ///
    /// If the key or a range including the key already exists in the query,
    /// this will have no effect. If the query already includes a range that has
    /// a non-inclusive bound equal to the key, the bound will be changed to be
    /// inclusive.
    pub fn insert_key(&mut self, key: Vec<u8>) {
        let key = QueryItem::Key(key);
        self.items.insert(key);
    }

    /// Adds a range to the query, so that all the entries in the tree with keys
    /// in the range will be included in the resulting proof.
    ///
    /// If a range including the range already exists in the query, this will
    /// have no effect. If the query already includes a range that overlaps with
    /// the range, the ranges will be joined together.
    pub fn insert_range(&mut self, range: Range<Vec<u8>>) {
        let range = QueryItem::Range(range);
        self.insert_item(range);
    }

    /// Adds an inclusive range to the query, so that all the entries in the
    /// tree with keys in the range will be included in the resulting proof.
    ///
    /// If a range including the range already exists in the query, this will
    /// have no effect. If the query already includes a range that overlaps with
    /// the range, the ranges will be merged together.
    pub fn insert_range_inclusive(&mut self, range: RangeInclusive<Vec<u8>>) {
        let range = QueryItem::RangeInclusive(range);
        self.insert_item(range);
    }

    /// Adds the `QueryItem` to the query, first checking to see if it collides
    /// with any existing ranges or keys. All colliding items will be removed
    /// then merged together so that the query includes the minimum number of
    /// items (with no items covering any duplicate parts of keyspace) while
    /// still including every key or range that has been added to the query.
    pub fn insert_item(&mut self, mut item: QueryItem) {
        // since `QueryItem::eq` considers items equal if they collide at all
        // (including keys within ranges or ranges which partially overlap),
        // `items.take` will remove the first item which collides
        while let Some(existing) = self.items.take(&item) {
            item = item.merge(existing);
        }

        self.items.insert(item);
    }
}

impl<Q: Into<QueryItem>> From<Vec<Q>> for Query {
    fn from(other: Vec<Q>) -> Self {
        let items = other.into_iter().map(Into::into).collect();
        Query { items }
    }
}

impl From<Query> for Vec<QueryItem> {
    fn from(q: Query) -> Vec<QueryItem> {
        q.into_iter().collect()
    }
}

impl IntoIterator for Query {
    type Item = QueryItem;
    type IntoIter = <BTreeSet<QueryItem> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

/// A `QueryItem` represents a key or range of keys to be included in a proof.
#[derive(Clone, Debug)]
pub enum QueryItem {
    Key(Vec<u8>),
    Range(Range<Vec<u8>>),
    RangeInclusive(RangeInclusive<Vec<u8>>),
}

impl QueryItem {
    pub fn lower_bound(&self) -> &[u8] {
        match self {
            QueryItem::Key(key) => key.as_slice(),
            QueryItem::Range(range) => range.start.as_ref(),
            QueryItem::RangeInclusive(range) => range.start().as_ref(),
        }
    }

    pub fn upper_bound(&self) -> (&[u8], bool) {
        match self {
            QueryItem::Key(key) => (key.as_slice(), true),
            QueryItem::Range(range) => (range.end.as_ref(), false),
            QueryItem::RangeInclusive(range) => (range.end().as_ref(), true),
        }
    }

    pub fn contains(&self, key: &[u8]) -> bool {
        let (bound, inclusive) = self.upper_bound();
        return key >= self.lower_bound() && (key < bound || (key == bound && inclusive));
    }

    fn merge(self, other: QueryItem) -> QueryItem {
        // TODO: don't copy into new vecs
        let start = min(self.lower_bound(), other.lower_bound()).to_vec();
        let end = max(self.upper_bound(), other.upper_bound());
        if end.1 {
            QueryItem::RangeInclusive(RangeInclusive::new(start, end.0.to_vec()))
        } else {
            QueryItem::Range(Range {
                start,
                end: end.0.to_vec(),
            })
        }
    }
}

impl PartialEq for QueryItem {
    fn eq(&self, other: &QueryItem) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl PartialEq<&[u8]> for QueryItem {
    fn eq(&self, other: &&[u8]) -> bool {
        matches!(self.partial_cmp(other), Some(Ordering::Equal))
    }
}

impl Eq for QueryItem {}

impl Ord for QueryItem {
    fn cmp(&self, other: &QueryItem) -> Ordering {
        let cmp_lu = self.lower_bound().cmp(other.upper_bound().0);
        let cmp_ul = self.upper_bound().0.cmp(other.lower_bound());
        let self_inclusive = self.upper_bound().1;
        let other_inclusive = other.upper_bound().1;

        match (cmp_lu, cmp_ul) {
            (Ordering::Less, Ordering::Less) => Ordering::Less,
            (Ordering::Less, Ordering::Equal) => match self_inclusive {
                true => Ordering::Equal,
                false => Ordering::Less,
            },
            (Ordering::Less, Ordering::Greater) => Ordering::Equal,
            (Ordering::Equal, _) => match other_inclusive {
                true => Ordering::Equal,
                false => Ordering::Greater,
            },
            (Ordering::Greater, _) => Ordering::Greater,
        }
    }
}

impl PartialOrd for QueryItem {
    fn partial_cmp(&self, other: &QueryItem) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialOrd<&[u8]> for QueryItem {
    fn partial_cmp(&self, other: &&[u8]) -> Option<Ordering> {
        let other = QueryItem::Key(other.to_vec());
        Some(self.cmp(&other))
    }
}

impl From<Vec<u8>> for QueryItem {
    fn from(key: Vec<u8>) -> Self {
        QueryItem::Key(key)
    }
}

impl Link {
    /// Creates a `Node::Hash` from this link. Panics if the link is of variant
    /// `Link::Modified` since its hash has not yet been computed.
    #[cfg(feature = "full")]
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
    #[cfg(feature = "full")]
    pub(crate) fn create_proof(
        &mut self,
        query: &[QueryItem],
    ) -> Result<(LinkedList<Op>, (bool, bool))> {
        // TODO: don't copy into vec, support comparing QI to byte slice
        let node_key = QueryItem::Key(self.tree().key().to_vec());
        let search = query.binary_search_by(|key| key.cmp(&node_key));

        let (left_items, right_items) = match search {
            Ok(index) => {
                let item = &query[index];
                let left_bound = item.lower_bound();
                let right_bound = item.upper_bound().0;

                // if range starts before this node's key, include it in left
                // child's query
                let left_query = if left_bound < self.tree().key() {
                    &query[..=index]
                } else {
                    &query[..index]
                };

                // if range ends after this node's key, include it in right
                // child's query
                let right_query = if right_bound > self.tree().key() {
                    &query[index..]
                } else {
                    &query[index + 1..]
                };

                (left_query, right_query)
            }
            Err(index) => (&query[..index], &query[index..]),
        };

        let (mut proof, left_absence) = self.create_child_proof(true, left_items)?;
        let (mut right_proof, right_absence) = self.create_child_proof(false, right_items)?;

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
    #[cfg(feature = "full")]
    fn create_child_proof(
        &mut self,
        left: bool,
        query: &[QueryItem],
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

pub fn verify(bytes: &[u8], expected_hash: Hash) -> Result<Map> {
    let ops = Decoder::new(bytes);
    let mut map_builder = MapBuilder::new();

    let root = execute(ops, true, |node| map_builder.insert(node))?;

    if root.hash() != expected_hash {
        return Err(Error::HashMismatch(expected_hash, root.hash()));
    }

    Ok(map_builder.build())
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
#[deprecated]
pub fn verify_query(
    bytes: &[u8],
    query: &Query,
    expected_hash: Hash,
) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
    let mut output = Vec::with_capacity(query.len());
    let mut last_push = None;
    let mut query = query.iter().peekable();
    let mut in_range = false;

    let ops = Decoder::new(bytes);

    let root = execute(ops, true, |node| {
        if let Node::KV(key, value) = node {
            while let Some(item) = query.peek() {
                // get next item in query
                let query_item = *item;
                // we have not reached next queried part of tree
                if *query_item > key.as_slice() {
                    // continue to next push
                    break;
                }

                if !in_range {
                    // this is the first data we have encountered for this query
                    // item. ensure lower bound of query item is proven
                    match last_push {
                        // lower bound is proven - we have an exact match
                        _ if key == query_item.lower_bound() => {}

                        // lower bound is proven - this is the leftmost node
                        // in the tree
                        None => {}

                        // lower bound is proven - the preceding tree node
                        // is lower than the bound
                        Some(Node::KV(_, _)) => {}

                        // cannot verify lower bound - we have an abridged
                        // tree so we cannot tell what the preceding key was
                        Some(_) => {
                            return Err(Error::Bound(
                                "Cannot verify lower bound of queried range".into(),
                            ));
                        }
                    }
                }

                if key.as_slice() >= query_item.upper_bound().0 {
                    // at or past upper bound of range (or this was an exact
                    // match on a single-key queryitem), advance to next query
                    // item
                    query.next();
                    in_range = false;
                } else {
                    // have not reached upper bound, we expect more values
                    // to be proven in the range (and all pushes should be
                    // unabridged until we reach end of range)
                    in_range = true;
                }

                // this push matches the queried item
                if query_item.contains(key) {
                    // add data to output
                    output.push((key.clone(), value.clone()));

                    // continue to next push
                    break;
                }

                // continue to next queried item
            }
        } else if in_range {
            // we encountered a queried range but the proof was abridged (saw a
            // non-KV push), we are missing some part of the range
            return Err(Error::MissingData);
        }

        last_push = Some(node.clone());

        Ok(())
    })?;

    // we have remaining query items, check absence proof against right edge of
    // tree
    if query.peek().is_some() {
        match last_push {
            // last node in tree was less than queried item
            Some(Node::KV(_, _)) => {}

            // proof contains abridged data so we cannot verify absence of
            // remaining query items
            _ => {
                return Err(Error::MissingData);
            }
        }
    }

    if root.hash() != expected_hash {
        return Err(Error::HashMismatch(expected_hash, root.hash()));
    }

    Ok(output)
}

#[allow(deprecated)]
#[cfg(test)]
mod test {
    use super::super::encoding::encode_into;
    use super::super::*;
    use super::*;
    use crate::test_utils::make_tree_seq;
    use crate::tree::{NoopCommit, PanicSource, RefWalker, Tree};

    fn make_3_node_tree() -> Tree {
        let mut tree = Tree::new(vec![5], vec![5])
            .attach(true, Some(Tree::new(vec![3], vec![3])))
            .attach(false, Some(Tree::new(vec![7], vec![7])));
        tree.commit(&mut NoopCommit {}).expect("commit failed");
        tree
    }

    fn verify_keys_test(keys: Vec<Vec<u8>>, expected_result: Vec<Option<Vec<u8>>>) {
        let mut tree = make_3_node_tree();
        let mut walker = RefWalker::new(&mut tree, PanicSource {});

        let (proof, _) = walker
            .create_proof(
                keys.clone()
                    .into_iter()
                    .map(QueryItem::Key)
                    .collect::<Vec<_>>()
                    .as_slice(),
            )
            .expect("failed to create proof");
        let mut bytes = vec![];
        encode_into(proof.iter(), &mut bytes);

        let expected_hash = [
            152, 49, 143, 253, 103, 178, 190, 32, 220, 107, 195, 90, 13, 69, 91, 129, 200, 90, 83,
            174, 124, 122, 64, 230, 201, 226, 250, 125, 102, 139, 137, 124,
        ];

        let mut query = Query::new();
        for key in keys.iter() {
            query.insert_key(key.clone());
        }

        let result = verify_query(bytes.as_slice(), &query, expected_hash).expect("verify failed");

        let mut values = std::collections::HashMap::new();
        for (key, value) in result {
            assert!(values.insert(key, value).is_none());
        }

        for (key, expected_value) in keys.iter().zip(expected_result.iter()) {
            assert_eq!(values.get(key), expected_value.as_ref());
        }
    }

    #[test]
    fn root_verify() {
        verify_keys_test(vec![vec![5]], vec![Some(vec![5])]);
    }

    #[test]
    fn single_verify() {
        verify_keys_test(vec![vec![3]], vec![Some(vec![3])]);
    }

    #[test]
    fn double_verify() {
        verify_keys_test(vec![vec![3], vec![5]], vec![Some(vec![3]), Some(vec![5])]);
    }

    #[test]
    fn double_verify_2() {
        verify_keys_test(vec![vec![3], vec![7]], vec![Some(vec![3]), Some(vec![7])]);
    }

    #[test]
    fn triple_verify() {
        verify_keys_test(
            vec![vec![3], vec![5], vec![7]],
            vec![Some(vec![3]), Some(vec![5]), Some(vec![7])],
        );
    }

    #[test]
    fn left_edge_absence_verify() {
        verify_keys_test(vec![vec![2]], vec![None]);
    }

    #[test]
    fn right_edge_absence_verify() {
        verify_keys_test(vec![vec![8]], vec![None]);
    }

    #[test]
    fn inner_absence_verify() {
        verify_keys_test(vec![vec![6]], vec![None]);
    }

    #[test]
    fn absent_and_present_verify() {
        verify_keys_test(vec![vec![5], vec![6]], vec![Some(vec![5]), None]);
    }

    #[test]
    fn empty_proof() {
        let mut tree = make_3_node_tree();
        let mut walker = RefWalker::new(&mut tree, PanicSource {});

        let (proof, absence) = walker
            .create_proof(vec![].as_slice())
            .expect("create_proof errored");

        let mut iter = proof.iter();
        assert_eq!(
            iter.next(),
            Some(&Op::Push(Node::Hash([
                122, 206, 248, 199, 28, 44, 167, 54, 247, 186, 254, 117, 199, 105, 171, 34, 61, 30,
                248, 155, 175, 1, 234, 202, 135, 51, 148, 45, 52, 250, 165, 24
            ])))
        );
        assert_eq!(
            iter.next(),
            Some(&Op::Push(Node::KVHash([
                51, 102, 120, 71, 161, 248, 19, 99, 151, 15, 58, 53, 42, 157, 10, 119, 161, 38, 54,
                254, 88, 131, 22, 49, 223, 231, 198, 153, 66, 62, 71, 71
            ])))
        );
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(
            iter.next(),
            Some(&Op::Push(Node::Hash([
                14, 130, 75, 65, 251, 244, 9, 188, 62, 47, 255, 76, 139, 67, 19, 236, 33, 6, 164,
                8, 119, 188, 80, 177, 184, 15, 255, 250, 143, 112, 23, 57
            ])))
        );
        assert_eq!(iter.next(), Some(&Op::Child));
        assert!(iter.next().is_none());
        assert_eq!(absence, (false, false));

        let mut bytes = vec![];
        encode_into(proof.iter(), &mut bytes);
        let res = verify_query(bytes.as_slice(), &Query::new(), tree.hash()).unwrap();
        assert!(res.is_empty());
    }

    #[test]
    fn root_proof() {
        let mut tree = make_3_node_tree();
        let mut walker = RefWalker::new(&mut tree, PanicSource {});

        let queryitems = vec![QueryItem::Key(vec![5])];
        let (proof, absence) = walker
            .create_proof(queryitems.as_slice())
            .expect("create_proof errored");

        let mut iter = proof.iter();
        assert_eq!(
            iter.next(),
            Some(&Op::Push(Node::Hash([
                122, 206, 248, 199, 28, 44, 167, 54, 247, 186, 254, 117, 199, 105, 171, 34, 61, 30,
                248, 155, 175, 1, 234, 202, 135, 51, 148, 45, 52, 250, 165, 24
            ])))
        );
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![5], vec![5]))));
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(
            iter.next(),
            Some(&Op::Push(Node::Hash([
                14, 130, 75, 65, 251, 244, 9, 188, 62, 47, 255, 76, 139, 67, 19, 236, 33, 6, 164,
                8, 119, 188, 80, 177, 184, 15, 255, 250, 143, 112, 23, 57
            ])))
        );
        assert_eq!(iter.next(), Some(&Op::Child));
        assert!(iter.next().is_none());
        assert_eq!(absence, (false, false));

        let mut bytes = vec![];
        encode_into(proof.iter(), &mut bytes);
        let mut query = Query::new();
        for item in queryitems {
            query.insert_item(item);
        }
        let res = verify_query(bytes.as_slice(), &query, tree.hash()).unwrap();
        assert_eq!(res, vec![(vec![5], vec![5])]);
    }

    #[test]
    fn leaf_proof() {
        let mut tree = make_3_node_tree();
        let mut walker = RefWalker::new(&mut tree, PanicSource {});

        let queryitems = vec![QueryItem::Key(vec![3])];
        let (proof, absence) = walker
            .create_proof(queryitems.as_slice())
            .expect("create_proof errored");

        let mut iter = proof.iter();
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![3], vec![3]))));
        assert_eq!(
            iter.next(),
            Some(&Op::Push(Node::KVHash([
                51, 102, 120, 71, 161, 248, 19, 99, 151, 15, 58, 53, 42, 157, 10, 119, 161, 38, 54,
                254, 88, 131, 22, 49, 223, 231, 198, 153, 66, 62, 71, 71
            ])))
        );
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(
            iter.next(),
            Some(&Op::Push(Node::Hash([
                14, 130, 75, 65, 251, 244, 9, 188, 62, 47, 255, 76, 139, 67, 19, 236, 33, 6, 164,
                8, 119, 188, 80, 177, 184, 15, 255, 250, 143, 112, 23, 57
            ])))
        );
        assert_eq!(iter.next(), Some(&Op::Child));
        assert!(iter.next().is_none());
        assert_eq!(absence, (false, false));

        let mut bytes = vec![];
        encode_into(proof.iter(), &mut bytes);
        let mut query = Query::new();
        for item in queryitems {
            query.insert_item(item);
        }
        let res = verify_query(bytes.as_slice(), &query, tree.hash()).unwrap();
        assert_eq!(res, vec![(vec![3], vec![3])]);
    }

    #[test]
    fn double_leaf_proof() {
        let mut tree = make_3_node_tree();
        let mut walker = RefWalker::new(&mut tree, PanicSource {});

        let queryitems = vec![QueryItem::Key(vec![3]), QueryItem::Key(vec![7])];
        let (proof, absence) = walker
            .create_proof(queryitems.as_slice())
            .expect("create_proof errored");

        let mut iter = proof.iter();
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![3], vec![3]))));
        assert_eq!(
            iter.next(),
            Some(&Op::Push(Node::KVHash([
                51, 102, 120, 71, 161, 248, 19, 99, 151, 15, 58, 53, 42, 157, 10, 119, 161, 38, 54,
                254, 88, 131, 22, 49, 223, 231, 198, 153, 66, 62, 71, 71
            ])))
        );
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![7], vec![7]))));
        assert_eq!(iter.next(), Some(&Op::Child));
        assert!(iter.next().is_none());
        assert_eq!(absence, (false, false));

        let mut bytes = vec![];
        encode_into(proof.iter(), &mut bytes);
        let mut query = Query::new();
        for item in queryitems {
            query.insert_item(item);
        }
        let res = verify_query(bytes.as_slice(), &query, tree.hash()).unwrap();
        assert_eq!(res, vec![(vec![3], vec![3]), (vec![7], vec![7]),]);
    }

    #[test]
    fn all_nodes_proof() {
        let mut tree = make_3_node_tree();
        let mut walker = RefWalker::new(&mut tree, PanicSource {});

        let queryitems = vec![
            QueryItem::Key(vec![3]),
            QueryItem::Key(vec![5]),
            QueryItem::Key(vec![7]),
        ];
        let (proof, absence) = walker
            .create_proof(queryitems.as_slice())
            .expect("create_proof errored");

        let mut iter = proof.iter();
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![3], vec![3]))));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![5], vec![5]))));
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![7], vec![7]))));
        assert_eq!(iter.next(), Some(&Op::Child));
        assert!(iter.next().is_none());
        assert_eq!(absence, (false, false));

        let mut bytes = vec![];
        encode_into(proof.iter(), &mut bytes);
        let mut query = Query::new();
        for item in queryitems {
            query.insert_item(item);
        }
        let res = verify_query(bytes.as_slice(), &query, tree.hash()).unwrap();
        assert_eq!(
            res,
            vec![(vec![3], vec![3]), (vec![5], vec![5]), (vec![7], vec![7]),]
        );
    }

    #[test]
    fn global_edge_absence_proof() {
        let mut tree = make_3_node_tree();
        let mut walker = RefWalker::new(&mut tree, PanicSource {});

        let queryitems = vec![QueryItem::Key(vec![8])];
        let (proof, absence) = walker
            .create_proof(queryitems.as_slice())
            .expect("create_proof errored");

        let mut iter = proof.iter();
        assert_eq!(
            iter.next(),
            Some(&Op::Push(Node::Hash([
                122, 206, 248, 199, 28, 44, 167, 54, 247, 186, 254, 117, 199, 105, 171, 34, 61, 30,
                248, 155, 175, 1, 234, 202, 135, 51, 148, 45, 52, 250, 165, 24
            ])))
        );
        assert_eq!(
            iter.next(),
            Some(&Op::Push(Node::KVHash([
                51, 102, 120, 71, 161, 248, 19, 99, 151, 15, 58, 53, 42, 157, 10, 119, 161, 38, 54,
                254, 88, 131, 22, 49, 223, 231, 198, 153, 66, 62, 71, 71
            ])))
        );
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![7], vec![7]))));
        assert_eq!(iter.next(), Some(&Op::Child));
        assert!(iter.next().is_none());
        assert_eq!(absence, (false, true));

        let mut bytes = vec![];
        encode_into(proof.iter(), &mut bytes);
        let mut query = Query::new();
        for item in queryitems {
            query.insert_item(item);
        }
        let res = verify_query(bytes.as_slice(), &query, tree.hash()).unwrap();
        assert_eq!(res, vec![]);
    }

    #[test]
    fn absence_proof() {
        let mut tree = make_3_node_tree();
        let mut walker = RefWalker::new(&mut tree, PanicSource {});

        let queryitems = vec![QueryItem::Key(vec![6])];
        let (proof, absence) = walker
            .create_proof(queryitems.as_slice())
            .expect("create_proof errored");

        let mut iter = proof.iter();
        assert_eq!(
            iter.next(),
            Some(&Op::Push(Node::Hash([
                122, 206, 248, 199, 28, 44, 167, 54, 247, 186, 254, 117, 199, 105, 171, 34, 61, 30,
                248, 155, 175, 1, 234, 202, 135, 51, 148, 45, 52, 250, 165, 24
            ])))
        );
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![5], vec![5]))));
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![7], vec![7]))));
        assert_eq!(iter.next(), Some(&Op::Child));
        assert!(iter.next().is_none());
        assert_eq!(absence, (false, false));

        let mut bytes = vec![];
        encode_into(proof.iter(), &mut bytes);
        let mut query = Query::new();
        for item in queryitems {
            query.insert_item(item);
        }
        let res = verify_query(bytes.as_slice(), &query, tree.hash()).unwrap();
        assert_eq!(res, vec![]);
    }

    #[test]
    fn doc_proof() {
        let mut tree = Tree::new(vec![5], vec![5])
            .attach(
                true,
                Some(
                    Tree::new(vec![2], vec![2])
                        .attach(true, Some(Tree::new(vec![1], vec![1])))
                        .attach(
                            false,
                            Some(
                                Tree::new(vec![4], vec![4])
                                    .attach(true, Some(Tree::new(vec![3], vec![3]))),
                            ),
                        ),
                ),
            )
            .attach(
                false,
                Some(
                    Tree::new(vec![9], vec![9])
                        .attach(
                            true,
                            Some(
                                Tree::new(vec![7], vec![7])
                                    .attach(true, Some(Tree::new(vec![6], vec![6])))
                                    .attach(false, Some(Tree::new(vec![8], vec![8]))),
                            ),
                        )
                        .attach(
                            false,
                            Some(
                                Tree::new(vec![11], vec![11])
                                    .attach(true, Some(Tree::new(vec![10], vec![10]))),
                            ),
                        ),
                ),
            );
        tree.commit(&mut NoopCommit {}).unwrap();

        let mut walker = RefWalker::new(&mut tree, PanicSource {});

        let queryitems = vec![
            QueryItem::Key(vec![1]),
            QueryItem::Key(vec![2]),
            QueryItem::Key(vec![3]),
            QueryItem::Key(vec![4]),
        ];
        let (proof, absence) = walker
            .create_proof(queryitems.as_slice())
            .expect("create_proof errored");

        let mut iter = proof.iter();
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![1], vec![1]))));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![2], vec![2]))));
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![3], vec![3]))));
        assert_eq!(iter.next(), Some(&Op::Push(Node::KV(vec![4], vec![4]))));
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(iter.next(), Some(&Op::Child));
        assert_eq!(
            iter.next(),
            Some(&Op::Push(Node::KVHash([
                51, 102, 120, 71, 161, 248, 19, 99, 151, 15, 58, 53, 42, 157, 10, 119, 161, 38, 54,
                254, 88, 131, 22, 49, 223, 231, 198, 153, 66, 62, 71, 71
            ])))
        );
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(
            iter.next(),
            Some(&Op::Push(Node::Hash([
                7, 132, 203, 145, 2, 40, 89, 172, 87, 248, 48, 26, 61, 218, 45, 51, 183, 186, 103,
                1, 102, 244, 85, 147, 189, 105, 81, 131, 98, 134, 8, 22
            ])))
        );
        assert_eq!(iter.next(), Some(&Op::Child));
        assert!(iter.next().is_none());
        assert_eq!(absence, (false, false));

        let mut bytes = vec![];
        encode_into(proof.iter(), &mut bytes);
        assert_eq!(
            bytes,
            vec![
                3, 1, 1, 0, 1, 1, 3, 1, 2, 0, 1, 2, 16, 3, 1, 3, 0, 1, 3, 3, 1, 4, 0, 1, 4, 16, 17,
                2, 51, 102, 120, 71, 161, 248, 19, 99, 151, 15, 58, 53, 42, 157, 10, 119, 161, 38,
                54, 254, 88, 131, 22, 49, 223, 231, 198, 153, 66, 62, 71, 71, 16, 1, 7, 132, 203,
                145, 2, 40, 89, 172, 87, 248, 48, 26, 61, 218, 45, 51, 183, 186, 103, 1, 102, 244,
                85, 147, 189, 105, 81, 131, 98, 134, 8, 22, 17
            ]
        );

        let mut bytes = vec![];
        encode_into(proof.iter(), &mut bytes);
        let mut query = Query::new();
        for item in queryitems {
            query.insert_item(item);
        }
        let res = verify_query(bytes.as_slice(), &query, tree.hash()).unwrap();
        assert_eq!(
            res,
            vec![
                (vec![1], vec![1]),
                (vec![2], vec![2]),
                (vec![3], vec![3]),
                (vec![4], vec![4]),
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
        assert!(
            QueryItem::RangeInclusive(vec![10]..=vec![20]) == QueryItem::Range(vec![20]..vec![30])
        );
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
        assert_eq!(
            mine.merge(other),
            QueryItem::RangeInclusive(vec![10]..=vec![30])
        );

        let mine = QueryItem::Key(vec![5]);
        let other = QueryItem::Range(vec![1]..vec![10]);
        assert_eq!(mine.merge(other), QueryItem::Range(vec![1]..vec![10]));

        let mine = QueryItem::Key(vec![10]);
        let other = QueryItem::RangeInclusive(vec![1]..=vec![10]);
        assert_eq!(
            mine.merge(other),
            QueryItem::RangeInclusive(vec![1]..=vec![10])
        );
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
        assert_eq!(
            format!("{:?}", iter.next()),
            "Some(RangeInclusive([3]..=[7]))"
        );
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn range_proof() {
        let mut tree = make_tree_seq(10);
        let mut walker = RefWalker::new(&mut tree, PanicSource {});

        let queryitems = vec![QueryItem::Range(
            vec![0, 0, 0, 0, 0, 0, 0, 5]..vec![0, 0, 0, 0, 0, 0, 0, 7],
        )];
        let (proof, absence) = walker
            .create_proof(queryitems.as_slice())
            .expect("create_proof errored");

        let mut iter = proof.iter();
        assert_eq!(
            iter.next(),
            Some(&Op::Push(Node::Hash([
                17, 220, 117, 95, 12, 128, 227, 62, 27, 85, 63, 171, 7, 164, 229, 207, 31, 194,
                159, 191, 127, 156, 78, 120, 179, 192, 172, 18, 161, 143, 80, 158
            ])))
        );
        assert_eq!(
            iter.next(),
            Some(&Op::Push(Node::KVHash([
                195, 69, 92, 176, 93, 179, 168, 91, 24, 44, 179, 237, 40, 86, 200, 163, 117, 138,
                171, 243, 169, 55, 183, 24, 6, 195, 18, 170, 69, 249, 202, 142
            ])))
        );
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(
            iter.next(),
            Some(&Op::Push(Node::Hash([
                164, 12, 89, 84, 85, 215, 17, 121, 91, 12, 85, 43, 76, 134, 159, 179, 194, 12, 91,
                231, 114, 116, 248, 137, 144, 224, 102, 147, 169, 112, 83, 90
            ])))
        );
        assert_eq!(
            iter.next(),
            Some(&Op::Push(Node::KV(
                vec![0, 0, 0, 0, 0, 0, 0, 5],
                vec![123; 60]
            )))
        );
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(
            iter.next(),
            Some(&Op::Push(Node::KV(
                vec![0, 0, 0, 0, 0, 0, 0, 6],
                vec![123; 60]
            )))
        );
        assert_eq!(iter.next(), Some(&Op::Child));
        assert_eq!(
            iter.next(),
            Some(&Op::Push(Node::KV(
                vec![0, 0, 0, 0, 0, 0, 0, 7],
                vec![123; 60]
            )))
        );
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(
            iter.next(),
            Some(&Op::Push(Node::Hash([
                216, 52, 8, 192, 196, 34, 57, 23, 142, 151, 139, 82, 192, 119, 107, 161, 96, 226,
                79, 6, 52, 71, 64, 24, 107, 241, 110, 239, 220, 62, 245, 107
            ])))
        );
        assert_eq!(iter.next(), Some(&Op::Child));
        assert_eq!(iter.next(), Some(&Op::Child));
        assert!(iter.next().is_none());
        assert_eq!(absence, (false, false));

        let mut bytes = vec![];
        encode_into(proof.iter(), &mut bytes);
        let mut query = Query::new();
        for item in queryitems {
            query.insert_item(item);
        }
        let res = verify_query(bytes.as_slice(), &query, tree.hash()).unwrap();
        assert_eq!(
            res,
            vec![
                (vec![0, 0, 0, 0, 0, 0, 0, 5], vec![123; 60]),
                (vec![0, 0, 0, 0, 0, 0, 0, 6], vec![123; 60]),
            ]
        );
    }

    #[test]
    fn range_proof_inclusive() {
        let mut tree = make_tree_seq(10);
        let mut walker = RefWalker::new(&mut tree, PanicSource {});

        let queryitems = vec![QueryItem::RangeInclusive(
            vec![0, 0, 0, 0, 0, 0, 0, 5]..=vec![0, 0, 0, 0, 0, 0, 0, 7],
        )];
        let (proof, absence) = walker
            .create_proof(queryitems.as_slice())
            .expect("create_proof errored");

        let mut iter = proof.iter();
        assert_eq!(
            iter.next(),
            Some(&Op::Push(Node::Hash([
                17, 220, 117, 95, 12, 128, 227, 62, 27, 85, 63, 171, 7, 164, 229, 207, 31, 194,
                159, 191, 127, 156, 78, 120, 179, 192, 172, 18, 161, 143, 80, 158
            ])))
        );
        assert_eq!(
            iter.next(),
            Some(&Op::Push(Node::KVHash([
                195, 69, 92, 176, 93, 179, 168, 91, 24, 44, 179, 237, 40, 86, 200, 163, 117, 138,
                171, 243, 169, 55, 183, 24, 6, 195, 18, 170, 69, 249, 202, 142
            ])))
        );
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(
            iter.next(),
            Some(&Op::Push(Node::Hash([
                164, 12, 89, 84, 85, 215, 17, 121, 91, 12, 85, 43, 76, 134, 159, 179, 194, 12, 91,
                231, 114, 116, 248, 137, 144, 224, 102, 147, 169, 112, 83, 90
            ])))
        );
        assert_eq!(
            iter.next(),
            Some(&Op::Push(Node::KV(
                vec![0, 0, 0, 0, 0, 0, 0, 5],
                vec![123; 60]
            )))
        );
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(
            iter.next(),
            Some(&Op::Push(Node::KV(
                vec![0, 0, 0, 0, 0, 0, 0, 6],
                vec![123; 60]
            )))
        );
        assert_eq!(iter.next(), Some(&Op::Child));
        assert_eq!(
            iter.next(),
            Some(&Op::Push(Node::KV(
                vec![0, 0, 0, 0, 0, 0, 0, 7],
                vec![123; 60]
            )))
        );
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(
            iter.next(),
            Some(&Op::Push(Node::Hash([
                216, 52, 8, 192, 196, 34, 57, 23, 142, 151, 139, 82, 192, 119, 107, 161, 96, 226,
                79, 6, 52, 71, 64, 24, 107, 241, 110, 239, 220, 62, 245, 107
            ])))
        );
        assert_eq!(iter.next(), Some(&Op::Child));
        assert_eq!(iter.next(), Some(&Op::Child));
        assert!(iter.next().is_none());
        assert_eq!(absence, (false, false));

        let mut bytes = vec![];
        encode_into(proof.iter(), &mut bytes);
        let mut query = Query::new();
        for item in queryitems {
            query.insert_item(item);
        }
        let res = verify_query(bytes.as_slice(), &query, tree.hash()).unwrap();
        assert_eq!(
            res,
            vec![
                (vec![0, 0, 0, 0, 0, 0, 0, 5], vec![123; 60]),
                (vec![0, 0, 0, 0, 0, 0, 0, 6], vec![123; 60]),
                (vec![0, 0, 0, 0, 0, 0, 0, 7], vec![123; 60]),
            ]
        );
    }

    #[test]
    fn range_proof_missing_upper_bound() {
        let mut tree = make_tree_seq(10);
        let mut walker = RefWalker::new(&mut tree, PanicSource {});

        let queryitems = vec![QueryItem::Range(
            vec![0, 0, 0, 0, 0, 0, 0, 5]..vec![0, 0, 0, 0, 0, 0, 0, 6, 5],
        )];
        let (proof, absence) = walker
            .create_proof(queryitems.as_slice())
            .expect("create_proof errored");

        let mut iter = proof.iter();
        assert_eq!(
            iter.next(),
            Some(&Op::Push(Node::Hash([
                17, 220, 117, 95, 12, 128, 227, 62, 27, 85, 63, 171, 7, 164, 229, 207, 31, 194,
                159, 191, 127, 156, 78, 120, 179, 192, 172, 18, 161, 143, 80, 158
            ])))
        );
        assert_eq!(
            iter.next(),
            Some(&Op::Push(Node::KVHash([
                195, 69, 92, 176, 93, 179, 168, 91, 24, 44, 179, 237, 40, 86, 200, 163, 117, 138,
                171, 243, 169, 55, 183, 24, 6, 195, 18, 170, 69, 249, 202, 142
            ])))
        );
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(
            iter.next(),
            Some(&Op::Push(Node::Hash([
                164, 12, 89, 84, 85, 215, 17, 121, 91, 12, 85, 43, 76, 134, 159, 179, 194, 12, 91,
                231, 114, 116, 248, 137, 144, 224, 102, 147, 169, 112, 83, 90
            ])))
        );
        assert_eq!(
            iter.next(),
            Some(&Op::Push(Node::KV(
                vec![0, 0, 0, 0, 0, 0, 0, 5],
                vec![123; 60]
            )))
        );
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(
            iter.next(),
            Some(&Op::Push(Node::KV(
                vec![0, 0, 0, 0, 0, 0, 0, 6],
                vec![123; 60]
            )))
        );
        assert_eq!(iter.next(), Some(&Op::Child));
        assert_eq!(
            iter.next(),
            Some(&Op::Push(Node::KV(
                vec![0, 0, 0, 0, 0, 0, 0, 7],
                vec![123; 60]
            )))
        );
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(
            iter.next(),
            Some(&Op::Push(Node::Hash([
                216, 52, 8, 192, 196, 34, 57, 23, 142, 151, 139, 82, 192, 119, 107, 161, 96, 226,
                79, 6, 52, 71, 64, 24, 107, 241, 110, 239, 220, 62, 245, 107
            ])))
        );
        assert_eq!(iter.next(), Some(&Op::Child));
        assert_eq!(iter.next(), Some(&Op::Child));
        assert!(iter.next().is_none());
        assert_eq!(absence, (false, false));

        let mut bytes = vec![];
        encode_into(proof.iter(), &mut bytes);
        let mut query = Query::new();
        for item in queryitems {
            query.insert_item(item);
        }
        let res = verify_query(bytes.as_slice(), &query, tree.hash()).unwrap();
        assert_eq!(
            res,
            vec![
                (vec![0, 0, 0, 0, 0, 0, 0, 5], vec![123; 60]),
                (vec![0, 0, 0, 0, 0, 0, 0, 6], vec![123; 60]),
            ]
        );
    }

    #[test]
    fn range_proof_missing_lower_bound() {
        let mut tree = make_tree_seq(10);
        let mut walker = RefWalker::new(&mut tree, PanicSource {});

        let queryitems = vec![
            // 7 is not inclusive
            QueryItem::Range(vec![0, 0, 0, 0, 0, 0, 0, 5, 5]..vec![0, 0, 0, 0, 0, 0, 0, 7]),
        ];
        let (proof, absence) = walker
            .create_proof(queryitems.as_slice())
            .expect("create_proof errored");

        let mut iter = proof.iter();
        assert_eq!(
            iter.next(),
            Some(&Op::Push(Node::Hash([
                17, 220, 117, 95, 12, 128, 227, 62, 27, 85, 63, 171, 7, 164, 229, 207, 31, 194,
                159, 191, 127, 156, 78, 120, 179, 192, 172, 18, 161, 143, 80, 158
            ])))
        );
        assert_eq!(
            iter.next(),
            Some(&Op::Push(Node::KVHash([
                195, 69, 92, 176, 93, 179, 168, 91, 24, 44, 179, 237, 40, 86, 200, 163, 117, 138,
                171, 243, 169, 55, 183, 24, 6, 195, 18, 170, 69, 249, 202, 142
            ])))
        );
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(
            iter.next(),
            Some(&Op::Push(Node::Hash([
                164, 12, 89, 84, 85, 215, 17, 121, 91, 12, 85, 43, 76, 134, 159, 179, 194, 12, 91,
                231, 114, 116, 248, 137, 144, 224, 102, 147, 169, 112, 83, 90
            ])))
        );
        assert_eq!(
            iter.next(),
            Some(&Op::Push(Node::KV(
                vec![0, 0, 0, 0, 0, 0, 0, 5],
                vec![123; 60]
            )))
        );
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(
            iter.next(),
            Some(&Op::Push(Node::KV(
                vec![0, 0, 0, 0, 0, 0, 0, 6],
                vec![123; 60]
            )))
        );
        assert_eq!(iter.next(), Some(&Op::Child));
        assert_eq!(
            iter.next(),
            Some(&Op::Push(Node::KV(
                vec![0, 0, 0, 0, 0, 0, 0, 7],
                vec![123; 60]
            )))
        );
        assert_eq!(iter.next(), Some(&Op::Parent));
        assert_eq!(
            iter.next(),
            Some(&Op::Push(Node::Hash([
                216, 52, 8, 192, 196, 34, 57, 23, 142, 151, 139, 82, 192, 119, 107, 161, 96, 226,
                79, 6, 52, 71, 64, 24, 107, 241, 110, 239, 220, 62, 245, 107
            ])))
        );
        assert_eq!(iter.next(), Some(&Op::Child));
        assert_eq!(iter.next(), Some(&Op::Child));
        assert!(iter.next().is_none());
        assert_eq!(absence, (false, false));

        let mut bytes = vec![];
        encode_into(proof.iter(), &mut bytes);
        let mut query = Query::new();
        for item in queryitems {
            query.insert_item(item);
        }
        let res = verify_query(bytes.as_slice(), &query, tree.hash()).unwrap();
        assert_eq!(res, vec![(vec![0, 0, 0, 0, 0, 0, 0, 6], vec![123; 60]),]);
    }

    #[test]
    fn query_from_vec() {
        let queryitems = vec![QueryItem::Range(
            vec![0, 0, 0, 0, 0, 0, 0, 5, 5]..vec![0, 0, 0, 0, 0, 0, 0, 7],
        )];
        let query = Query::from(queryitems);

        let mut expected = BTreeSet::new();
        expected.insert(QueryItem::Range(
            vec![0, 0, 0, 0, 0, 0, 0, 5, 5]..vec![0, 0, 0, 0, 0, 0, 0, 7],
        ));
        assert_eq!(query.items, expected);
    }

    #[test]
    fn query_into_vec() {
        let mut query = Query::new();
        query.insert_item(QueryItem::Range(
            vec![0, 0, 0, 0, 0, 0, 5, 5]..vec![0, 0, 0, 0, 0, 0, 0, 7],
        ));
        let query_vec: Vec<QueryItem> = query.into();
        let expected = vec![QueryItem::Range(
            vec![0, 0, 0, 0, 0, 0, 5, 5]..vec![0, 0, 0, 0, 0, 0, 0, 7],
        )];
        assert_eq!(
            query_vec.get(0).unwrap().lower_bound(),
            expected.get(0).unwrap().lower_bound()
        );
        assert_eq!(
            query_vec.get(0).unwrap().upper_bound(),
            expected.get(0).unwrap().upper_bound()
        );
    }

    #[test]
    fn query_item_from_vec_u8() {
        let queryitems: Vec<u8> = vec![42];
        let query = QueryItem::from(queryitems);

        let expected = QueryItem::Key(vec![42]);
        assert_eq!(query, expected);
    }

    #[test]
    fn verify_ops() {
        let mut tree = Tree::new(vec![5], vec![5]);
        tree.commit(&mut NoopCommit {}).expect("commit failed");

        let root_hash = tree.hash();
        let mut walker = RefWalker::new(&mut tree, PanicSource {});

        let (proof, _) = walker
            .create_proof(vec![QueryItem::Key(vec![5])].as_slice())
            .expect("failed to create proof");
        let mut bytes = vec![];

        encode_into(proof.iter(), &mut bytes);

        let map = verify(&bytes, root_hash).unwrap();
        assert_eq!(
            map.get(vec![5].as_slice()).unwrap().unwrap(),
            vec![5].as_slice()
        );
    }

    #[test]
    #[should_panic(expected = "verify failed")]
    fn verify_ops_mismatched_hash() {
        let mut tree = Tree::new(vec![5], vec![5]);
        tree.commit(&mut NoopCommit {}).expect("commit failed");

        let mut walker = RefWalker::new(&mut tree, PanicSource {});

        let (proof, _) = walker
            .create_proof(vec![QueryItem::Key(vec![5])].as_slice())
            .expect("failed to create proof");
        let mut bytes = vec![];

        encode_into(proof.iter(), &mut bytes);

        let _map = verify(&bytes, [42; 32]).expect("verify failed");
    }

    #[test]
    #[should_panic(expected = "verify failed")]
    fn verify_query_mismatched_hash() {
        let mut tree = make_3_node_tree();
        let mut walker = RefWalker::new(&mut tree, PanicSource {});
        let keys = vec![vec![5], vec![7]];
        let (proof, _) = walker
            .create_proof(
                keys.clone()
                    .into_iter()
                    .map(QueryItem::Key)
                    .collect::<Vec<_>>()
                    .as_slice(),
            )
            .expect("failed to create proof");
        let mut bytes = vec![];
        encode_into(proof.iter(), &mut bytes);

        let mut query = Query::new();
        for key in keys.iter() {
            query.insert_key(key.clone());
        }

        let _result = verify_query(bytes.as_slice(), &query, [42; 32]).expect("verify failed");
    }
}
