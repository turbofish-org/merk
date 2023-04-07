use super::super::Node;
use crate::{Error, Result};
use std::collections::btree_map;
use std::collections::BTreeMap;
use std::iter::Peekable;
use std::ops::{Bound, RangeBounds};

/// `MapBuilder` allows a consumer to construct a `Map` by inserting the nodes
/// contained in a proof, in key-order.
pub(crate) struct MapBuilder(Map);

impl MapBuilder {
    /// Creates a new `MapBuilder` with an empty internal `Map`.
    pub fn new() -> Self {
        MapBuilder(Map {
            entries: Default::default(),
            right_edge: true,
        })
    }

    /// Adds the node's data to the uncerlying `Map` (if node is type `KV`), or
    /// makes a note of non-contiguous data (if node is type `KVHash` or
    /// `Hash`).
    pub fn insert(&mut self, node: &Node) -> Result<()> {
        match node {
            Node::KV(key, value) => {
                if let Some((prev_key, _)) = self.0.entries.last_key_value() {
                    if key <= prev_key {
                        return Err(Error::Key(
                            "Expected nodes to be in increasing key order".into(),
                        ));
                    }
                }

                let value = (self.0.right_edge, value.clone());
                self.0.entries.insert(key.clone(), value);
                self.0.right_edge = true;
            }
            _ => self.0.right_edge = false,
        }

        Ok(())
    }

    /// Consumes the `MapBuilder` and returns its internal `Map`.
    pub fn build(self) -> Map {
        self.0
    }
}

/// `Map` stores data extracted from a proof (which has already been verified
/// against a known root hash), and allows a consumer to access the data by
/// looking up individual keys using the `get` method, or iterating over ranges
/// using the `range` method.
pub struct Map {
    entries: BTreeMap<Vec<u8>, (bool, Vec<u8>)>,
    right_edge: bool,
}

impl Map {
    /// Gets the value for a single key, or `None` if the key was proven to not
    /// exist in the tree. If the proof does not include the data and also does
    /// not prove that the key is absent in the tree (meaning the proof is not
    /// valid), an error will be returned.
    pub fn get<'a>(&'a self, key: &'a [u8]) -> Result<Option<&'a [u8]>> {
        // if key is in proof just get from entries
        if let Some((_, value)) = self.entries.get(key) {
            return Ok(Some(value.as_slice()));
        }

        // otherwise, use range which only includes exact key match to check
        // absence proof
        let entry = self
            .range((Bound::Included(key), Bound::Included(key)))
            .next()
            .transpose()?
            .map(|(_, value)| value);
        Ok(entry)
    }

    /// Returns an iterator over all (key, value) entries in the requested range
    /// of keys. If during iteration we encounter a gap in the data (e.g. the
    /// proof did not include all nodes within the range), the iterator will
    /// yield an error.
    pub fn range<'a>(&'a self, bounds: impl RangeBounds<&'a [u8]>) -> Range {
        let start_bound = bound_to_inner(bounds.start_bound());
        let end_bound = bound_to_inner(bounds.end_bound());
        let outer_bounds = (
            start_bound.map_or(Bound::Unbounded, |k| {
                self.entries
                    .range(..=k.to_vec())
                    .next_back()
                    .map_or(Bound::Unbounded, |prev| Bound::Included(prev.0.clone()))
            }),
            end_bound.map_or(Bound::Unbounded, |k| {
                self.entries
                    .range(k.to_vec()..)
                    .next()
                    .map_or(Bound::Unbounded, |next| Bound::Included(next.0.clone()))
            }),
        );

        Range {
            map: self,
            bounds: bounds_to_vec(bounds),
            done: false,
            iter: self.entries.range(outer_bounds).peekable(),
        }
    }

    fn contiguous_right(&self, key: &[u8]) -> bool {
        self.entries
            .range((Bound::Excluded(key.to_vec()), Bound::Unbounded))
            .next()
            .map_or(self.right_edge, |(_, (contiguous, _))| *contiguous)
    }
}

/// Returns `None` for `Bound::Unbounded`, or the inner key value for
/// `Bound::Included` and `Bound::Excluded`.
fn bound_to_inner<T>(bound: Bound<T>) -> Option<T> {
    match bound {
        Bound::Unbounded => None,
        Bound::Included(key) | Bound::Excluded(key) => Some(key),
    }
}

fn bound_to_vec(bound: Bound<&&[u8]>) -> Bound<Vec<u8>> {
    match bound {
        Bound::Unbounded => Bound::Unbounded,
        Bound::Excluded(k) => Bound::Excluded(k.to_vec()),
        Bound::Included(k) => Bound::Included(k.to_vec()),
    }
}

fn bounds_to_vec<'a, R: RangeBounds<&'a [u8]>>(bounds: R) -> (Bound<Vec<u8>>, Bound<Vec<u8>>) {
    (
        bound_to_vec(bounds.start_bound()),
        bound_to_vec(bounds.end_bound()),
    )
}

/// An iterator over (key, value) entries as extracted from a verified proof. If
/// during iteration we encounter a gap in the data (e.g. the proof did not
/// include all nodes within the range), the iterator will yield an error.
pub struct Range<'a> {
    map: &'a Map,
    bounds: (Bound<Vec<u8>>, Bound<Vec<u8>>),
    done: bool,
    iter: Peekable<btree_map::Range<'a, Vec<u8>, (bool, Vec<u8>)>>,
}

impl<'a> Range<'a> {
    fn yield_entry_if_contiguous(
        &mut self,
        entry: (&'a Vec<u8>, &'a (bool, Vec<u8>)),
        contiguous: bool,
        forward: bool,
    ) -> Option<Result<(&'a [u8], &'a [u8])>> {
        if !contiguous {
            self.done = true;
            return Some(Err(Error::MissingData));
        }

        self.yield_entry(entry, forward)
    }

    fn yield_entry(
        &mut self,
        entry: (&'a Vec<u8>, &'a (bool, Vec<u8>)),
        forward: bool,
    ) -> Option<Result<(&'a [u8], &'a [u8])>> {
        let (key, (_, value)) = entry;
        if forward {
            self.bounds.0 = Bound::Excluded(key.clone());
        } else {
            self.bounds.1 = Bound::Excluded(key.clone());
        }
        Some(Ok((key.as_slice(), value.as_slice())))
    }

    fn yield_none_if_contiguous(
        &mut self,
        contiguous: bool,
    ) -> Option<Result<(&'a [u8], &'a [u8])>> {
        self.done = true;

        if !contiguous {
            return Some(Err(Error::MissingData));
        }

        None
    }

    fn yield_next_if_contiguous(&mut self) -> Option<Result<(&'a [u8], &'a [u8])>> {
        if let Some((_, (contiguous, _))) = self.iter.peek() {
            if !contiguous {
                self.done = true;
                return Some(Err(Error::MissingData));
            }
        }

        self.next()
    }

    fn yield_next_back_if_contiguous(
        &mut self,
        contiguous: bool,
    ) -> Option<Result<(&'a [u8], &'a [u8])>> {
        if !contiguous {
            self.done = true;
            return Some(Err(Error::MissingData));
        }

        self.next_back()
    }
}

impl<'a> Iterator for Range<'a> {
    type Item = Result<(&'a [u8], &'a [u8])>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        let entry = match self.iter.next() {
            None => return self.yield_none_if_contiguous(self.map.right_edge),
            Some(entry) => entry,
        };
        let (key, (contiguous, _)) = entry;

        let past_start = match bound_to_inner(self.bounds.0.clone()) {
            None => true,
            Some(ref start_bound) => key > start_bound,
        };
        let at_start = match self.bounds.0 {
            Bound::Unbounded => true,
            Bound::Included(ref start_bound) => key == start_bound,
            Bound::Excluded(_) => false,
        };
        let past_end = match self.bounds.1 {
            Bound::Unbounded => false,
            Bound::Included(ref end_bound) => key > end_bound,
            Bound::Excluded(ref end_bound) => key >= end_bound,
        };

        if past_end {
            self.yield_none_if_contiguous(*contiguous)
        } else if past_start {
            self.yield_entry_if_contiguous(entry, *contiguous, true)
        } else if at_start {
            self.yield_entry(entry, true)
        } else {
            self.yield_next_if_contiguous()
        }
    }
}

impl<'a> DoubleEndedIterator for Range<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        let entry = match self.iter.next_back() {
            None => return self.yield_none_if_contiguous(self.map.contiguous_right(&[])),
            Some(entry) => entry,
        };
        let (key, (contiguous_l, _)) = entry;
        let contiguous_r = self.map.contiguous_right(key);

        let past_end = match bound_to_inner(self.bounds.1.clone()) {
            None => true,
            Some(ref end_bound) => key < end_bound,
        };
        let at_end = match self.bounds.1 {
            Bound::Unbounded => true,
            Bound::Included(ref end_bound) => key == end_bound,
            Bound::Excluded(_) => false,
        };
        let past_start = match self.bounds.0 {
            Bound::Unbounded => false,
            Bound::Included(ref start_bound) => key < start_bound,
            Bound::Excluded(ref start_bound) => key <= start_bound,
        };

        if past_start {
            self.yield_none_if_contiguous(contiguous_r)
        } else if past_end {
            self.yield_entry_if_contiguous(entry, contiguous_r, false)
        } else if at_end {
            self.yield_entry(entry, false)
        } else {
            self.yield_next_back_if_contiguous(*contiguous_l)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HASH_LENGTH;

    #[test]
    #[should_panic(expected = "Expected nodes to be in increasing key order")]
    fn mapbuilder_insert_out_of_order() {
        let mut builder = MapBuilder::new();
        builder.insert(&Node::KV(vec![1, 2, 3], vec![])).unwrap();
        builder.insert(&Node::KV(vec![1, 2, 2], vec![])).unwrap();
    }

    #[test]
    #[should_panic(expected = "Expected nodes to be in increasing key order")]
    fn mapbuilder_insert_dupe() {
        let mut builder = MapBuilder::new();
        builder.insert(&Node::KV(vec![1, 2, 3], vec![])).unwrap();
        builder.insert(&Node::KV(vec![1, 2, 3], vec![])).unwrap();
    }

    #[test]
    fn mapbuilder_insert_including_edge() {
        let mut builder = MapBuilder::new();
        builder.insert(&Node::Hash([0; HASH_LENGTH])).unwrap();
        builder.insert(&Node::KV(vec![1, 2, 4], vec![])).unwrap();

        assert!(builder.0.right_edge);
    }

    #[test]
    fn mapbuilder_insert_abridged_edge() {
        let mut builder = MapBuilder::new();
        builder.insert(&Node::KV(vec![1, 2, 3], vec![])).unwrap();
        builder.insert(&Node::Hash([0; HASH_LENGTH])).unwrap();

        assert!(!builder.0.right_edge);
    }

    #[test]
    fn mapbuilder_build() {
        let mut builder = MapBuilder::new();
        builder.insert(&Node::KV(vec![1, 2, 3], vec![1])).unwrap();
        builder.insert(&Node::Hash([0; HASH_LENGTH])).unwrap();
        builder.insert(&Node::KV(vec![1, 2, 4], vec![2])).unwrap();

        let map = builder.build();
        let mut entries = map.entries.iter();
        assert_eq!(entries.next(), Some((&vec![1, 2, 3], &(true, vec![1]))));
        assert_eq!(entries.next(), Some((&vec![1, 2, 4], &(false, vec![2]))));
        assert_eq!(entries.next(), None);
        assert!(map.right_edge);
    }

    #[test]
    fn map_get_included() {
        let mut builder = MapBuilder::new();
        builder.insert(&Node::KV(vec![1, 2, 3], vec![1])).unwrap();
        builder.insert(&Node::Hash([0; HASH_LENGTH])).unwrap();
        builder.insert(&Node::KV(vec![1, 2, 4], vec![2])).unwrap();

        let map = builder.build();
        assert_eq!(map.get(&[1, 2, 3]).unwrap().unwrap(), vec![1],);
        assert_eq!(map.get(&[1, 2, 4]).unwrap().unwrap(), vec![2],);
    }

    #[test]
    #[should_panic(expected = "MissingData")]
    fn map_get_missing_absence_proof() {
        let mut builder = MapBuilder::new();
        builder.insert(&Node::KV(vec![1, 2, 3], vec![1])).unwrap();
        builder.insert(&Node::Hash([0; HASH_LENGTH])).unwrap();
        builder.insert(&Node::KV(vec![1, 2, 4], vec![2])).unwrap();

        let map = builder.build();
        map.get(&[1, 2, 3, 4]).unwrap();
    }

    #[test]
    fn map_get_valid_absence_proof() {
        let mut builder = MapBuilder::new();
        builder.insert(&Node::KV(vec![1, 2, 3], vec![1])).unwrap();
        builder.insert(&Node::KV(vec![1, 2, 4], vec![2])).unwrap();

        let map = builder.build();
        assert!(map.get(&[1, 2, 3, 4]).unwrap().is_none());
    }

    #[test]
    #[should_panic(expected = "MissingData")]
    fn range_abridged() {
        let mut builder = MapBuilder::new();
        builder.insert(&Node::KV(vec![1, 2, 3], vec![1])).unwrap();
        builder.insert(&Node::Hash([0; HASH_LENGTH])).unwrap();
        builder.insert(&Node::KV(vec![1, 2, 4], vec![2])).unwrap();

        let map = builder.build();
        let mut range = map.range(&[1u8, 2, 3][..]..&[1u8, 2, 4][..]);
        assert_eq!(range.next().unwrap().unwrap(), (&[1, 2, 3][..], &[1][..]));
        range.next().unwrap().unwrap();
    }

    #[test]
    fn range_ok() {
        let mut builder = MapBuilder::new();
        builder.insert(&Node::KV(vec![1, 2, 3], vec![1])).unwrap();
        builder.insert(&Node::KV(vec![1, 2, 4], vec![2])).unwrap();
        builder.insert(&Node::KV(vec![1, 2, 5], vec![3])).unwrap();

        let map = builder.build();
        let mut range = map.range(&[1u8, 2, 3][..]..&[1u8, 2, 5][..]);
        assert_eq!(range.next().unwrap().unwrap(), (&[1, 2, 3][..], &[1][..]));
        assert_eq!(range.next().unwrap().unwrap(), (&[1, 2, 4][..], &[2][..]));
        assert!(range.next().is_none());
        assert!(range.next().is_none());
    }

    #[test]
    #[should_panic(expected = "MissingData")]
    fn range_lower_unbounded_map_non_contiguous() {
        let mut builder = MapBuilder::new();
        builder.insert(&Node::KV(vec![1, 2, 3], vec![1])).unwrap();
        builder.insert(&Node::Hash([1; HASH_LENGTH])).unwrap();
        builder.insert(&Node::KV(vec![1, 2, 4], vec![1])).unwrap();

        let map = builder.build();

        let mut range = map.range(..&[1u8, 2, 5][..]);
        range.next().unwrap().unwrap();
        assert_eq!(range.next().unwrap().unwrap(), (&[1][..], &[1][..]));
    }

    #[test]
    fn range_reach_proof_end() {
        let mut builder = MapBuilder::new();
        builder.insert(&Node::KV(vec![1, 2, 3], vec![1])).unwrap();
        builder.insert(&Node::KV(vec![1, 2, 4], vec![2])).unwrap();

        let map = builder.build();
        let mut range = map.range(&[1u8, 2, 3][..]..);
        assert_eq!(range.next().unwrap().unwrap(), (&[1, 2, 3][..], &[1][..]));
        assert_eq!(range.next().unwrap().unwrap(), (&[1, 2, 4][..], &[2][..]));
        assert!(range.next().is_none());
    }

    #[test]
    fn range_unbounded() {
        let mut builder = MapBuilder::new();
        builder.insert(&Node::KV(vec![1, 2, 3], vec![1])).unwrap();
        builder.insert(&Node::KV(vec![1, 2, 4], vec![2])).unwrap();

        let map = builder.build();
        let mut range = map.range(..);
        assert_eq!(range.next().unwrap().unwrap(), (&[1, 2, 3][..], &[1][..]));
        assert_eq!(range.next().unwrap().unwrap(), (&[1, 2, 4][..], &[2][..]));
        assert!(range.next().is_none());
    }
}
