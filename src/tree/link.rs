use super::hash::Hash;
use super::Tree;

// TODO: optimize memory footprint
pub enum Link {
    Pruned {
        hash: Hash,
        height: u8,
        key: Vec<u8>
    },
    Modified {
        pending_writes: usize,
        height: u8,
        tree: Tree
    },
    Stored {
        hash: Hash,
        height: u8,
        tree: Tree
    }
}

impl Link {
    #[inline]
    pub fn from_modified_tree(tree: Tree) -> Self {
        let mut pending_writes = 1
            + tree.child_pending_writes(true)
            + tree.child_pending_writes(false);

        Link::Modified {
            pending_writes,
            height: tree.height(),
            tree
        }
    }

    pub fn maybe_from_modified_tree(maybe_tree: Option<Tree>) -> Option<Self> {
        maybe_tree.map(|tree| Link::from_modified_tree(tree))
    }

    #[inline]
    pub fn is_pruned(&self) -> bool {
        match self {
            Link::Pruned { .. } => true,
            _ => false
        }
    }

    #[inline]
    pub fn is_modified(&self) -> bool {
        match self {
            Link::Modified { .. } => true,
            _ => false
        }
    }

    #[inline]
    pub fn is_stored(&self) -> bool {
        match self {
            Link::Stored { .. } => true,
            _ => false
        }
    }

    pub fn tree(&self) -> Option<&Tree> {
        match self {
            Link::Pruned { .. } => None,
            Link::Modified { tree, .. } => Some(tree),
            Link::Stored { tree, .. } => Some(tree)
        }
    }

    pub fn hash(&self) -> &Hash {
        match self {
            Link::Modified { .. } => panic!("Cannot get hash from modified link"),
            Link::Pruned { hash, .. } => hash,
            Link::Stored { hash, .. } => hash
        }
    }

    pub fn height(&self) -> u8 {
        match self {
            Link::Pruned { height, .. } => *height,
            Link::Modified { height, .. } => *height,
            Link::Stored { height, .. } => *height
        }
    }

    // pub fn prune(&mut self) {
    //     *self = match self {
    //         Link::Pruned => self,
    //         Link::Modified => panic!("Cannot prune Modified tree"),
    //         Link::Stored { hash, height, tree } => Link::Pruned {
    //             hash,
    //             height,
    //             key: tree.key
    //         }
    //     };
    // }
}

#[cfg(test)]
mod test {
    #[test]
    fn from_modified_tree() {
        
    }
}
