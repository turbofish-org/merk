use crate::tree::{Tree, side_to_str};

struct ModTreeInner {
    tree: Tree,
    left: Option<ModTree>,
    right: Option<ModTree>,
    pending_writes: usize,
    deleted_keys: Vec<Vec<u8>>
}

pub struct ModTree {
    inner: Box<ModTreeInner>
}

impl ModTree {
    pub fn new(tree: Tree) -> ModTree {
        ModTree {
            tree,
            left: None,
            right: None,
            deleted_keys: vec![]
        }
    }

    pub fn attach(mut self, left: bool, maybe_child: Option<Self>) -> Self {
        if self.child(left).is_some() || self.tree.child(left).is_some() {
            panic!(
                "Tried to attach to {} tree slot, but it is already Some",
                side_to_str(left)
            );
        }

        let slot = self.slot_mut(left);
        *slot = maybe_child;

        self.pending_writes = 1
            + self.inner.deleted_keys.len()
            + self.child(left).map_or(0, |c| c.inner.pending_writes)
            + self.child(!left).map_or(0, |c| c.inner.pending_writes);
    }

    pub fn child(&self, left: bool) -> Option<&Self> {
        if left {
            &self.inner.left
        } else {
            &self.inner.right
        }
    }

    fn slot_mut(&mut self, left: bool) -> &mut Option<Self> {
        if left {
            &mut self.inner.left
        } else {
            &mut self.inner.right
        }
    }
}
