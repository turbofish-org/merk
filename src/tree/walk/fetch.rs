use crate::error::Result;
use super::super::Node;

pub trait Fetch {
    fn fetch(&self, key: &[u8]) -> Result<Node>;
}
