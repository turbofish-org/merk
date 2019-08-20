use super::Tree;

struct StackItem<'a> {
    tree: &'a Tree,
    traversed: (bool, bool, bool)
}

impl<'a> StackItem<'a> {
    fn new(tree: &'a Tree) -> Self {
        StackItem {
            tree,
            traversed: (
                tree.child(true).is_none(),
                false,
                tree.child(false).is_none()
            )
        }
    }

    fn to_entry(&self) -> (Vec<u8>, Vec<u8>) {
        (
            self.tree.key().to_vec(),
            self.tree.value().to_vec()
        )
    }
}

pub struct Iter<'a> {
    stack: Vec<StackItem<'a>>
}

impl<'a> Iter<'a> {
    pub fn new(tree: &'a Tree) -> Self {
        let stack = vec![ StackItem::new(tree) ];
        Iter { stack }
    }
}

impl<'a> Tree {
    pub fn iter(&'a self) -> Iter<'a> {
        Iter::new(self)
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = (Vec<u8>, Vec<u8>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.stack.is_empty() {
            return None;
        }

        let last = self.stack.last_mut().unwrap();
        if !last.traversed.0 {
            last.traversed.0 = true;
            let tree = last.tree.child(true).unwrap();
            self.stack.push(StackItem::new(tree));
            self.next()
        } else if !last.traversed.1 {
            last.traversed.1 = true;
            Some(last.to_entry())
        } else if !last.traversed.2 {
            last.traversed.2 = true;
            let tree = last.tree.child(false).unwrap();
            self.stack.push(StackItem::new(tree));
            self.next()
        } else {
            self.stack.pop();
            self.next()
        }
    }
}
