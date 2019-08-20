use super::Tree;

struct StackItem<'a> {
    tree: &'a Tree,
    traversed: (bool, bool, bool)
}

impl<'a> StackItem<'a> {
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

impl Tree {
    pub fn iter<'a>(&'a self) -> Iter<'a> {
        let stack = vec![
            StackItem { tree: self, traversed: (false, false, false) }
        ];
        Iter { stack }
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = (Vec<u8>, Vec<u8>);

    fn next(&mut self) -> Option<Self::Item> {
        println!("next {} {:?}", self.stack.len(), self.stack.last().map(|l| l.tree.key()));

        if self.stack.is_empty() {
            return None;
        }

        let last = self.stack.last().unwrap();
        if !last.traversed.0 {
            if let Some(tree) = last.tree.child(true) {
                self.stack.last_mut().unwrap().traversed.0 = true;
                self.stack.push(StackItem { tree, traversed: (false, false, false) });
                self.next()
            } else {
                let last = self.stack.last_mut().unwrap();
                last.traversed.0 = true;
                self.next()
            }
        } else if !last.traversed.1 {
            let last = self.stack.last_mut().unwrap();
            last.traversed.1 = true;
            Some(last.to_entry())
        } else if !last.traversed.2 {
            if let Some(tree) = last.tree.child(false) {
                let last = self.stack.last_mut().unwrap();
                last.traversed.2 = true;
                self.stack.push(StackItem { tree, traversed: (false, false, false) });
                self.next()
            } else {
                self.stack.pop();
                self.next()
            }
        } else {
            self.stack.pop();
            self.next()
        }
    }
}
