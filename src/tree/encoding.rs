use super::Tree;
use crate::error::Result;

impl Tree {
    pub fn encode(&self) -> Vec<u8> {
        unimplemented!("todo")
    }

    pub fn decode(key: &[u8], bytes: &[u8]) -> Result<Tree> {
        unimplemented!("todo")
    }
}
