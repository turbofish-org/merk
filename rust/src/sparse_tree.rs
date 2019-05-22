use std::fmt;
use std::ops::{Deref, DerefMut};

use crate::node::*;

type GetNodeFn = fn(link: &Link) -> Node;

/// A selection of connected nodes in a tree.
///
/// SparseTrees are acyclic, and have exactly one root node.
pub struct SparseTree {
    node: Node,
    get_node: GetNodeFn,
    left: Option<Box<SparseTree>>,
    right: Option<Box<SparseTree>>
}

impl SparseTree {
    pub fn new(node: Node, get_node: GetNodeFn) -> SparseTree {
        SparseTree{
            node,
            get_node,
            left: None,
            right: None
        }
    }

    pub fn get(link: &Link, get_node: GetNodeFn) -> SparseTree {
        SparseTree::new(get_node(link), get_node)
    }

    pub fn put(&mut self, key: &[u8], value: &[u8]) {
        if self.node.key == key {
            // same key, just update the value of this node
            self.set_value(value);

            // we can return early since we know no children
            // have updated
            return;
        }

        // bytewise key comparison to get traversal direction
        let left = key < &self.node.key;

        // try to get child, fetching from db if necessary
        match self.maybe_get_child(left) {
            Some(child_tree) => {
                // recursively put value under child
                child_tree.put(key, value);

                // update link since we know child hash changed
                self.update_link(left);
            },
            None => {
                // no child here, create node and set as child
                let child_tree = Box::new(
                    SparseTree::new(
                        Node::new(key, value),
                        self.get_node
                    )
                );

                // set child field, update link, and update child's parent_key
                self.set_child(left, Some(child_tree));
            }
        };

        // rebalance if necessary
        self.maybe_rebalance();
    }

    fn update_link(&mut self, left: bool) {
        // compute child link
        let link = self.child_tree(left).map(|child| {
            child.as_link()
        });

        // set link on our Node
        self.node.set_child(left, link);
    }

    fn set_child(&mut self, left: bool, child_tree: Option<Box<SparseTree>>) {
        // set child field
        {
            let child_field = self.child_field_mut(left);
            *child_field = child_tree;
        }

        // update link
        self.update_link(left);

        // update child node's parent_key to point to us
        let self_key = self.node.key.clone();
        let child_field = self.child_field_mut(left);
        child_field.as_mut().map(|child| {
            child.set_parent(Some(self_key));
        });
    }

    fn child_tree(&self, left: bool) -> Option<&SparseTree> {
        let option = if left {
            &self.left
        } else {
            &self.right
        };
        option.as_ref().map(|_box| _box.as_ref())
    }

    fn child_tree_mut(&mut self, left: bool) -> Option<&mut SparseTree> {
        let option = if left {
            &mut self.left
        } else {
            &mut self.right
        };
        option.as_mut().map(|_box| _box.as_mut())
    }

    fn child_field_mut(&mut self, left: bool) -> &mut Option<Box<SparseTree>> {
        if left {
            &mut self.left
        } else {
            &mut self.right
        }
    }

    fn maybe_get_child(&mut self, left: bool) -> Option<&mut Box<SparseTree>> {
        if let Some(link) = self.child_link(left) {
            // node has a link
            let get_node = self.get_node;
            let child_field = self.child_field_mut(left);
            // if field is already set, get mutable reference to existing child
            // tree. if not, call out to `get_node and put result in a box.
            let child_tree = child_field.get_or_insert_with(|| {
                Box::new(SparseTree::get(&link, get_node))
            });
            Some(child_tree)
        } else {
            // node has no link, nothing to get
            None
        }
    }

    fn maybe_rebalance(&mut self) {
        let balance_factor = self.balance_factor();

        // return early if we don't need to balance
        if (balance_factor.abs() <= 1) {
            return;
        }

        // get child
       let left = balance_factor < 0;
       // (this unwrap should never panic, if the tree
       // is unbalanced in this direction then we know
       // there is a child)
       let child = self.maybe_get_child(left).unwrap();

         // maybe do a double rotation
        let double = if left {
            child.balance_factor() > 0
        } else {
            child.balance_factor() < 0
        };
        if double {
            // rotate child opposite direction, then update link
            child.rotate(!left);
            self.update_link(left);
        }

        self.rotate(left);
    }

    fn rotate(&mut self, left: bool) {
        self.maybe_get_child(left);
        let mut child = self.child_field_mut(left).take().unwrap();

        child.maybe_get_child(!left);
        let grandchild = child.child_field_mut(!left).take();
        self.set_child(left, grandchild);

        self.swap(child.as_mut());
        self.update_link(left);
        child.update_link(!left);
        self.set_child(!left, Some(child));
    }

    fn swap(&mut self, other: &mut SparseTree) {
        // XXX: this could be more efficient, we clone the whole node
        //      including its key/value

        let self_node = self.node.clone();
        let self_left = self.left.take();
        let self_right = self.right.take();
        let self_parent = self.node.parent_key.take();
        let other_parent = other.node.parent_key.take();

        self.node = other.node.clone();
        self.left = other.left.take();
        self.right = other.right.take();
        self.set_parent(self_parent);

        other.node = self_node;
        other.left = self_left;
        other.right = self_right;
        other.set_parent(other_parent);
    }
}

impl Deref for SparseTree {
    type Target = Node;

    fn deref(&self) -> &Node {
        &self.node
    }
}

impl DerefMut for SparseTree {
    fn deref_mut(&mut self) -> &mut Node {
        &mut self.node
    }
}

impl fmt::Debug for SparseTree {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fn traverse(f: &mut fmt::Formatter, cursor: &SparseTree, depth: u8, left: bool) {
            write!(f, "{}", "  ".repeat(depth as usize));

            let prefix = if depth == 0 {
                ""
            } else if left {
                "L: "
            } else {
                "R: "
            };
            write!(f, "{}{:?}\n", prefix, cursor.node);

            match &cursor.left {
                Some(child) => { traverse(f, &child, depth + 1, true); },
                None => {}
            };
            match &cursor.right {
                (Some(child)) => { traverse(f, &child, depth + 1, false); },
                (None) => {}
            };
        };

        traverse(f, self, 0, false);
        write!(f, "\n")
    }
}
//
// #[cfg(test)]
// mod tests {
//     use crate::sparse_tree::*;

#[test]
fn it_works() {
    // let st = SparseTree{node: Node::new(b"a", b"b"), left: None, right: None};
    // println!("{:?}", st);
    //
    // let st = SparseTree::join(
    //     Node::new(b"aa", b"b"), Some(st), Some(SparseTree::new(Node::new(b"aa", b"b")))
    // );
    //
    // let st = SparseTree::join(
    //     Node::new(b"ab", b"b"), Some(st), Some(SparseTree::new(Node::new(b"abc", b"b")))
    // );
    // println!("{:?}", st);

    let mut st = SparseTree::new(
        Node::new(b"abc", b"x"),
        |link| Node::new(link.key.as_slice(), b"x")
    );
    println!("{:?}", st);

    st.put(
        b"abcd", b"x"
    );
    println!("{:?}", st);

    st.put(
        b"a", b"x"
    );
    println!("{:?}", st);

    st.put(
        b"ab", b"x"
    );
    println!("{:?}", st);

    st.put(
        b"ab", b"y"
    );
    println!("{:?}", st);

    st.put(
        b"6", b"x"
    );
    println!("{:?}", st);

    st.put(
        b"b", b"x"
    );
    println!("{:?}", st);

    st.put(
        b"bc", b"x"
    );
    println!("{:?}", st);

    st.put(b"x", b"x");
    st.put(b"xx", b"x");
    st.put(b"xxx", b"x");
    st.put(b"xxxx", b"x");
    st.put(b"xxxxx", b"x");
    st.put(b"xxxxxx", b"x");
    println!("{:?}", st);

    // let mut node = Node::new(b"foo", b"bar");
    // node.update_kv_hash();
    // println!("node: {:?}", node);
    // println!("encoded length: {:?}", node.encode().unwrap().len());
    //
    // let node2 = Node::decode(&node.encode().unwrap()[..]);
    // println!("node2: {:?}", node2);
    //
    // let mut node3 = Node::new(b"foo2", b"bar2");
    // node.set_child(true, &mut node3);
    //
    // println!("node: {:?}", node);
}
