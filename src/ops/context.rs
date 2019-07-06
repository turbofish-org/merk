use super::{Batch, Op, Worker};
use crate::tree::{Tree, Node};
use crate::error::Result;

pub struct Context<'a> {
    pub batch: &'a Batch<'a>,
    pub workers: &'a [Worker],
    pub get_node: Box<dyn Fn(Vec<u8>) -> Result<Node>>
}
