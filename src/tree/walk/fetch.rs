use crate::error::Result;
use super::super::Tree;

pub trait Fetch {
    fn fetch(&self, key: &[u8]) -> Result<Tree>;
}
