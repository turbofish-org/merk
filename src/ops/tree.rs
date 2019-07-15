use crate::tree;

pub struct Tree {
    tree: tree::Tree,
    pending_writes: u64,
    pending_deletes: Vec<Vec<u8>>
}

impl tree::Build for Tree {
    fn detach(mut self, left: bool) -> (Self, Option<Self>) {
        // detach marks node for update
        let (tree, maybe_child) = self.inner.detach();
        self.pending_writes = 1 + self.inner.child(!left).pending_writes;
    }

    fn attach(self, left: bool, child: Option<Self>) -> Self {

    }
}
