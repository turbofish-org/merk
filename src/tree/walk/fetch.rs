use super::super::{Link, Tree};
use crate::error::{Error, Result};

/// A source of data to be used by the tree when encountering a pruned node.
///
/// This typcially means fetching the tree node from a backing store by its key,
/// but could also implement an in-memory cache for example.
pub trait Fetch {
    fn fetch_by_key(&self, key: &[u8]) -> Result<Option<Tree>>;

    /// Called when the tree needs to fetch a node with the given `Link`. The
    /// `link` value will always be a `Link::Reference` variant.
    fn fetch(&self, link: &Link) -> Result<Tree> {
        self.fetch_by_key_expect(link.key())
    }

    fn fetch_by_key_expect(&self, key: &[u8]) -> Result<Tree> {
        self.fetch_by_key(key)?
            .ok_or_else(|| Error::Key(format!("Key does not exist: {key:?}")))
    }
}
