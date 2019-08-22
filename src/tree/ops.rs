use std::fmt;
use crate::error::Result;
use super::{Tree, Link, Walker, Fetch};
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
    fn fetch(&self, _link: &Link) -> Result<Tree> {
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
                    // TODO: we shouldn't have to do this as 2 different calls to apply
                    let source = self.clone_source();
                    let wrap = |maybe_tree: Option<Tree>| {
                        maybe_tree.map(|tree| Self::new(tree, source.clone()))
                    };
                    let maybe_tree = self.remove()?;
                    let maybe_tree = wrap(Self::apply_to(maybe_tree, &batch[..index])?);
                    let maybe_tree = wrap(Self::apply_to(maybe_tree, &batch[index + 1..])?);
                    return Ok(maybe_tree);
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
        self,
        batch: &Batch,
        mid: usize,
        exclusive: bool
    ) -> Result<Option<Self>> {
        let left_batch = &batch[..mid];
        let right_batch = if exclusive {
            &batch[mid + 1..]
        } else {
            &batch[mid..]
        };
        
        let tree = if !left_batch.is_empty() {
            self.walk(true, |maybe_left|
                Self::apply_to(maybe_left, left_batch)
            )?
        } else {
            self
        };

        let tree = if !right_batch.is_empty() {
            tree.walk(false, |maybe_right|
                Self::apply_to(maybe_right, right_batch)
            )?
        } else {
            tree
        };

        let tree = tree.maybe_balance()?;

        Ok(Some(tree))
    }

    #[inline]
    fn balance_factor(&self) -> i8 {
        self.tree().balance_factor()
    }

    fn maybe_balance(self) -> Result<Self> {
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

    fn rotate(self, left: bool) -> Result<Self> {
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

    pub fn remove(self) -> Result<Option<Self>> {
        let tree = self.tree();
        let has_left = tree.link(true).is_some();
        let has_right = tree.link(false).is_some();
        let left = tree.child_height(true) > tree.child_height(false);

        // TODO: propagate up deleted keys?

        let maybe_tree = unsafe {
            if has_left && has_right {
                // two children, promote edge of taller child
                let (tree, tall_child) = self.detach_expect(left)?;
                let (_, short_child) = tree.detach_expect(!left)?;
                Some(tall_child.promote_edge(!left, short_child)?)
            } else if has_left || has_right {
                // single child, promote it
                Some(self.detach_expect(left)?.1)
            } else {
                // no child
                None
            }
        };

        Ok(maybe_tree)
    }

    fn promote_edge(self, left: bool, attach: Self) -> Result<Self> {
        let (edge, maybe_child) = self.remove_edge(left)?;
        edge
            .attach(!left, maybe_child)
            .attach(left, Some(attach))
            .maybe_balance()
    }

    fn remove_edge(self, left: bool) -> Result<(Self, Option<Self>)> {
        if self.tree().link(left).is_some() {
            // this node is not the edge, recurse
            let (tree, child) = unsafe { self.detach_expect(left)? };
            let (edge, maybe_child) = child.remove_edge(left)?;
            let tree = tree
                .attach(left, maybe_child)
                .maybe_balance()?;
            Ok((edge, Some(tree)))
        } else {
            // this node is the edge, detach its child if present
            unsafe { self.detach(!left) }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::tree::*;
    use crate::test_utils::{make_tree_seq, del_entry};

    #[test]
    fn simple_insert() {
        let batch = [
            (
                b"foo2".to_vec(),
                Op::Put(b"bar2".to_vec())
            )
        ];
        let tree = Tree::new(b"foo".to_vec(), b"bar".to_vec());
        let walker = Walker::new(tree, PanicSource {})
            .apply(&batch)
            .expect("apply errored")
            .expect("should be Some");
        assert_eq!(walker.tree().key(), b"foo");
        assert_eq!(walker.into_inner().child(false).unwrap().key(), b"foo2");
    }

    #[test]
    fn simple_update() {
        let batch = [
            (
                b"foo".to_vec(),
                Op::Put(b"bar2".to_vec())
            )
        ];
        let tree = Tree::new(b"foo".to_vec(), b"bar".to_vec());
        let walker = Walker::new(tree, PanicSource {})
            .apply(&batch)
            .expect("apply errored")
            .expect("should be Some");
        assert_eq!(walker.tree().key(), b"foo");
        assert_eq!(walker.tree().value(), b"bar2");
        assert!(walker.tree().link(true).is_none());
        assert!(walker.tree().link(false).is_none());
    }

    #[test]
    fn simple_delete() {
        let batch = [
            (b"foo2".to_vec(), Op::Delete)
        ];
        let tree = Tree::from_fields(
            b"foo".to_vec(), b"bar".to_vec(),
            [123; 20],
            None,
            Some(Link::Stored {
                hash: [123; 20],
                child_heights: (0, 0),
                tree: Tree::new(b"foo2".to_vec(), b"bar2".to_vec())
            })
        );
        let walker = Walker::new(tree, PanicSource {})
            .apply(&batch)
            .expect("apply errored")
            .expect("should be Some");
        assert_eq!(walker.tree().key(), b"foo");
        assert_eq!(walker.tree().value(), b"bar");
        assert!(walker.tree().link(true).is_none());
        assert!(walker.tree().link(false).is_none());
    }

    #[test]
    #[should_panic]
    fn delete_non_existent() {
        let batch = [
            (b"foo2".to_vec(), Op::Delete)
        ];
        let tree = Tree::new(b"foo".to_vec(), b"bar".to_vec());
        Walker::new(tree, PanicSource {})
            .apply(&batch)
            .unwrap();
    }

    #[test]
    fn delete_only_node() {
        let batch = [
            (b"foo".to_vec(), Op::Delete)
        ];
        let tree = Tree::new(b"foo".to_vec(), b"bar".to_vec());
        let walker = Walker::new(tree, PanicSource {})
            .apply(&batch)
            .expect("apply errored");
        assert!(walker.is_none());
    }

    #[test]
    fn delete_deep() {
        let tree = make_tree_seq(50);
        let batch = [ del_entry(5) ]; 
        Walker::new(tree, PanicSource {})
            .apply(&batch)
            .expect("apply errored")
            .expect("should be Some");
        // TODO: assert set of keys are correct
    }

    #[test]
    fn delete_recursive() {
        let tree = make_tree_seq(50);
        let batch = [ del_entry(29), del_entry(34) ]; 
        Walker::new(tree, PanicSource {})
            .apply(&batch)
            .expect("apply errored")
            .expect("should be Some");
        // TODO: assert set of keys are correct
    }

    #[test]
    fn delete_recursive_2() {
        let tree = make_tree_seq(10);
        let batch = [ del_entry(7), del_entry(9) ]; 
        let walker = Walker::new(tree, PanicSource {})
            .apply(&batch)
            .expect("apply errored")
            .expect("should be Some");
        // TODO: assert set of keys are correct

        println!("{:?}", walker.tree());
    }

    #[test]
    fn apply_empty_none() {
        let maybe_tree = Walker::<PanicSource>::apply_to(None, &vec![])
            .expect("apply_to failed");
        assert!(maybe_tree.is_none());
    }
}
