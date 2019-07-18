use crate::error::Result;
use crate::tree::Node;

pub trait Fetch {
    fn fetch(&self, key: &[u8]) -> Result<Node>;
}
