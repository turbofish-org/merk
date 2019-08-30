use crate::error::Result;
use super::super::{Tree, Link};

/// A source of data to be used by the tree when encountering a pruned node.
/// This typcially means fetching the tree node from a backing store by its key,
/// but could also implement an in-memory cache for example.
pub trait Fetch {
    /// Called when the tree needs to fetch a node with the given `Link`. The
    /// `link` value will always be a `Link::Pruned` variant.
    fn fetch(&self, link: &Link) -> Result<Tree>;
}
