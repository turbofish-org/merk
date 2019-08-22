use crate::error::Result;
use super::super::{Tree, Link};

pub trait Fetch {
    fn fetch(&self, link: &Link) -> Result<Tree>;
}
