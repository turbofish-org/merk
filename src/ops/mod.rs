use std::fmt;
use std::collections::BTreeSet;
use crate::error::Result;
use crate::tree::{Tree, Link, Walker, Fetch};
use Op::*;

pub enum Op {
    Put(Vec<u8>),
    Delete
}

impl fmt::Debug for Op {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "{}", match self {
            Put(value) => format!("Put({:?})", value),
            Delete => "Delete".to_string()
        })
    }
}


pub type BatchEntry = (Vec<u8>, Op);

pub type Batch = [BatchEntry];

#[derive(Clone)]
pub struct PanicSource {}
impl Fetch for PanicSource {
    fn fetch(&self, link: &Link) -> Result<Tree> {
        unreachable!("'fetch' should not have been called")
    }
}

impl<S> Walker<S>
    where S: Fetch + Sized + Send + Clone
{
    pub fn apply_to(
        maybe_tree: Option<Self>,
        batch: &Batch
    ) -> Result<Option<Tree>> {
        let maybe_walker = if batch.is_empty() {
            maybe_tree
        } else {
            match maybe_tree {
                None => return Self::build(batch),
                Some(tree) => tree.apply(batch)?
            }
        };

        Ok(maybe_walker.map(|walker| walker.into_inner()))
    }

    fn build(batch: &Batch) -> Result<Option<Tree>> {
        if batch.is_empty() {
            return Ok(None);
        }

        let mid_index = batch.len() / 2;
        let (mid_key, mid_op) = &batch[mid_index];
        let mid_value = match mid_op {
            Delete => panic!("Tried to delete non-existent key {:?}", mid_key),
            Put(value) => value
        };

        // TODO: take from batch so we don't have to clone
        let mid_tree = Tree::new(mid_key.to_vec(), mid_value.to_vec());
        let mid_walker = Walker::new(mid_tree, PanicSource {});
        Ok(mid_walker
            .recurse(batch, mid_index, true)?
            .map(|w| w.into_inner()))
    }

    fn apply(self, batch: &Batch) -> Result<Option<Self>> {
        // binary search to see if this node's key is in the batch, and to split
        // into left and right batches
        let search = batch.binary_search_by(
            |(key, _op)| key.as_slice().cmp(self.tree().key())
        );
        let tree = if let Ok(index) = search {
            // a key matches this node's key, apply op to this node
            match &batch[index].1 {
                // TODO: take vec from batch so we don't need to clone
                Put(value) => self.with_value(value.to_vec()),
                Delete => {
                    // self.remove()?;
                    panic!("remove not yet implemented")
                }
            }
        } else {
            self
        };

        let (mid, exclusive) = match search {
            Ok(index) => (index, true),
            Err(index) => (index, false)
        };

        tree.recurse(batch, mid, exclusive)
    }

    fn recurse(
        mut self,
        batch: &Batch,
        mid: usize,
        exclusive: bool
    ) -> Result<Option<Self>> {
        let left_batch = &batch[..mid];
        let right_batch = match exclusive {
            true => &batch[mid + 1..],
            false => &batch[mid..]
        };
        
        let _self = match left_batch.is_empty() {
            false => {
                self
                    .walk(true, |maybe_left|
                        Self::apply_to(maybe_left, left_batch)
                    )?
                    .maybe_balance()?
            },
            true => self
        };

        let _self = match right_batch.is_empty() {
            false => {
                _self
                    .walk(false, |maybe_right|
                        Self::apply_to(maybe_right, right_batch)
                    )?
                    .maybe_balance()?
            },
            true => _self
        };

        Ok(Some(_self))
    }

    #[inline]
    fn balance_factor(&self) -> i8 {
        self.tree().balance_factor()
    }

    fn maybe_balance(mut self) -> Result<Self> {
        let balance_factor = self.balance_factor();
        if balance_factor.abs() <= 1 {
            return Ok(self);
        }

        let left = balance_factor < 0;

        // maybe do a double rotation
        let _self = if left == (self.tree().link(left).unwrap().balance_factor() > 0) {
            self.walk_expect(left, |child| Ok(Some(child.rotate(!left)?)))?
        } else {
            self
        };
        
        _self.rotate(left)
    }

    fn rotate(mut self, left: bool) -> Result<Self> {
        unsafe {
            let (_self, child) = self.detach_expect(left)?;
            let (child, maybe_grandchild) = child.detach(!left)?;

            // attach grandchild to self
            let _self = _self
                .attach(left, maybe_grandchild)
                .maybe_balance()?;

            // attach self to child, return child
            child
                .attach(!left, Some(_self))
                .maybe_balance()
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::tree::*;

    #[test]
    fn simple_put() {
        let batch = [
            (
                b"foo2".to_vec(),
                Op::Put(b"bar2".to_vec())
            )
        ];
        let tree = Tree::new(b"foo".to_vec(), b"bar".to_vec());
        let mut walker = Walker::new(tree, PanicSource {})
            .apply(&batch)
            .expect("apply errored")
            .expect("should be Some");
        assert_eq!(walker.tree().key(), b"foo");
        assert_eq!(walker.into_inner().child(false).unwrap().key(), b"foo2");
    }

    #[test]
    fn apply_empty_none() {
        let maybe_tree = Walker::<PanicSource>::apply_to(None, &vec![])
            .expect("apply_to failed");
        assert!(maybe_tree.is_none());
    }
}
