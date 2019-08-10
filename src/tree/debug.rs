use std::fmt::{Debug, Formatter, Result};
use colored::Colorize;
use super::Tree;

impl Debug for Tree {
    fn fmt(&self, f: &mut Formatter) -> Result {
        fn traverse(
            f: &mut Formatter,
            cursor: &Tree,
            stack: &mut Vec<bool>,
            left: bool,
            has_sibling_after: bool,
        ) {
            let depth = stack.len();

            if depth > 0 {
                // draw ancestor's vertical lines
                for &line in stack.iter().take(depth-1) {
                    write!(
                        f,
                        "{}",
                        if line { " │  " } else { "    " }
                            .dimmed()
                    ).unwrap();
                }

                // draw our connecting line to parent
                write!(
                    f,
                    "{}",
                    if has_sibling_after { " ├" } else { " └" }
                        .dimmed()
                ).unwrap();
            }

            let prefix = if depth == 0 {
                ""
            } else if left {
                "L─"
            } else {
                "R─"
            };
            writeln!(f, "{}{:?} h={}", prefix.dimmed(), cursor.key(), cursor.height()).unwrap();

            if let Some(child_link) = cursor.link(true) {
                if let Some(child_tree) = child_link.tree() {
                    stack.push(true);
                    traverse(f, child_tree, stack, true, cursor.child(false).is_some());
                    stack.pop();
                } else {
                    // TODO: print pruned link
                }
            }

            if let Some(child_link) = cursor.link(false) {
                if let Some(child_tree) = child_link.tree() {
                    stack.push(false);
                    traverse(f, child_tree, stack, false, false);
                    stack.pop();
                } else {
                    // TODO: print pruned link
                }
            }
        };

        let mut stack = vec![];
        traverse(f, self, &mut stack, false, false);
        writeln!(f)
    }
}
