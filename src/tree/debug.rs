use std::fmt::{Debug, Formatter, Result};
use colored::Colorize;
use super::Tree;

impl Debug for Tree {
    fn fmt(&self, f: &mut Formatter) -> Result {
        fn traverse(
            f: &mut Formatter,
            cursor: &Tree,
            stack: &mut Vec<(Vec<u8>, Vec<u8>)>,
            left: bool
        ) {
            if let Some(child_link) = cursor.link(true) {
                if let Some(child_tree) = child_link.tree() {
                    stack.push((child_tree.key().to_vec(), cursor.key().to_vec()));
                    traverse(f, child_tree, stack, true);
                    stack.pop();
                } else {
                    // TODO: print pruned link
                }
            }

            let depth = stack.len();

            if depth > 0 {
                // draw ancestor's vertical lines
                for (low, high) in stack.iter().take(depth-1) {
                    let draw_line = cursor.key() > &low && cursor.key() < &high;
                    write!(
                        f,
                        "{}",
                        if draw_line { " │  " } else { "    " }.dimmed()
                    ).unwrap();
                }
            }

            let prefix = if depth == 0 {
                ""
            } else if left {
                " ┌-"
            } else {
                " └-"
            };
            writeln!(f, "{}{:?}", prefix.dimmed(), cursor.key()).unwrap();

            if let Some(child_link) = cursor.link(false) {
                if let Some(child_tree) = child_link.tree() {
                    stack.push((cursor.key().to_vec(), child_tree.key().to_vec()));
                    traverse(f, child_tree, stack, false);
                    stack.pop();
                } else {
                    // TODO: print pruned link
                }
            }
        };

        let mut stack = vec![];
        traverse(f, self, &mut stack, false);
        writeln!(f)
    }
}
