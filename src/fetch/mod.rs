use crate::tree::{Node, Link};

pub trait Fetch {
    fn fetch(&self, link: Link) -> Result<Node>;
}
