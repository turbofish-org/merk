use std::collections::BTreeMap;
use std::collections::btree_map;
use std::ops::{RangeBounds, Bound};
use failure::{format_err, bail, ensure};
use super::super::Node;
use crate::Result;

/// `MapBuilder` allows a consumer to construct a `Map` by inserting the nodes
/// contained in a proof, in key-order.
pub(crate) struct MapBuilder(Map);

impl MapBuilder {
    /// Creates a new `MapBuilder` with an empty internal `Map`.
    pub fn new() -> Self {
        MapBuilder(Map {
            entries: Default::default(),
            right_edge: true
        })
    }

    /// Adds the node's data to the uncerlying `Map` (if node is type `KV`), or
    /// makes a note of non-contiguous data (if node is type `KVHash` or
    /// `Hash`).
    pub fn insert(&mut self, node: &Node) -> Result<()> {
        match node {
            Node::KV(key, value) => {
                if let Some((prev_key, _)) = self.0.entries.last_key_value() {
                    ensure!(
                        key > prev_key,
                        "Expected nodes to be in increasing key order"
                    );
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
    pub fn range<'a, R: RangeBounds<[u8]> + 'a>(&'a self, bounds: R) -> Range {
        let start_key = to_inner(bounds.start_bound()).map(Into::into);
        let end_key = to_inner(bounds.start_bound()).map(Into::into);

        Range {
            map: self,
            start_key,
            end_key,
            iter: self.entries.range(bounds)
        }
    }
}

/// Returns `None` for `Bound::Unbounded`, or the inner key value for
/// `Bound::Included` and `Bound::Excluded`.
fn to_inner<T>(bound: Bound<T>) -> Option<T> {
    match bound {
        Bound::Unbounded => None,
        Bound::Included(key) | Bound::Excluded(key) => Some(key),
    }
}

/// An iterator over (key, value) entries as extracted from a verified proof. If
/// during iteration we encounter a gap in the data (e.g. the proof did not
/// include all nodes within the range), the iterator will yield an error.
pub struct Range<'a> {
    map: &'a Map,
    start_key: Option<Vec<u8>>,
    end_key: Option<Vec<u8>>,
    iter: btree_map::Range<'a, Vec<u8>, (bool, Vec<u8>)>,
}

impl<'a> Range<'a> {
    /// Returns an error if the proof does not properly prove the end of the
    /// range.
    fn check_end_bound(&self) -> Result<()> {
        let excluded_data = match self.end_key {
            // unbounded end, ensure proof has not excluded data at global right
            // edge of tree
            None => !self.map.right_edge,

            // bounded end (inclusive or exclusive), ensure we had an exact
            // match or next node is contiguous
            Some(ref key) => {
                // get neighboring node to the right (if any)
                let range = (
                    Bound::Included(key.to_vec()),
                    Bound::<Vec<u8>>::Unbounded
                );
                let maybe_end_node = self.map.entries.range(range).next();

                match maybe_end_node {
                    // reached global right edge of tree
                    None => !self.map.right_edge,

                    // got end node, must be exact match for end bound, or be
                    // greater than end bound and contiguous
                    Some((next_key, (contiguous, _))) => {
                       next_key != key && !contiguous
                    }
                }
            }
        };

        if excluded_data {
            bail!("Proof is missing data for query");
        }

        Ok(())
    }
}

impl<'a> Iterator for Range<'a> {
    type Item = Result<(&'a [u8], &'a [u8])>;

    fn next(&mut self) -> Option<Self::Item> {
        let (key, (contiguous, value)) = match self.iter.next() {
            // no more items, ensure no data was excluded at end of range
            None => return match self.check_end_bound() {
                Err(err) => Some(Err(err)),
                Ok(_) => None,
            },

            // got next item, destructure
            Some((key, (contiguous, value))) => (key, (contiguous, value)),
        };

        // don't checking for contiguous nodes if we have an exact match for
        // lower bound
        let skip_exclusion_check = if let Some(ref start_key) = self.start_key {
            start_key == key
        } else {
            false
        };

        // if nodes weren't contiguous, we cannot verify that we have all values
        // in the desired range
        if !skip_exclusion_check && !contiguous {
            return Some(Err(format_err!("Proof is missing data for query")));
        }

        // passed checks, return entry
        Some(Ok((key.as_slice(), value.as_slice())))
    }
}


