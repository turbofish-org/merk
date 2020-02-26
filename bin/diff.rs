use clap::Clap;
use merk::{Merk, tree::{RefWalker, Fetch}};


#[derive(Clap, Debug)]
#[clap()]
struct Args {
    /// The file path of the first Merk store to compare
    first_path: String,

    /// The file path of the second Merk store to compare
    second_path: String
}

fn main() {
    let args = Args::parse();

    // TODO: ensure stores exist

    let first_store = Merk::open(args.first_path)
        .expect("Could not open first store");
    let second_store = Merk::open(args.second_path)
        .expect("Could not open second store");

    first_store.walk(|maybe_first_walker| {
        second_store.walk(|maybe_second_walker| {
            compare_trees(maybe_first_walker, maybe_second_walker);
        });
    });
}

fn compare_trees<S, S2>(
    maybe_first_tree: Option<RefWalker<S>>,
    maybe_second_tree: Option<RefWalker<S2>>
)
    where
        S: Fetch + Sized + Clone + Send,
        S2: Fetch + Sized + Clone + Send
{
    match (&maybe_first_tree, &maybe_second_tree) {
        (None, None) => return,
        _ => compare_nodes(maybe_first_tree, maybe_second_tree)
    }
}

fn compare_nodes<S, S2>(
    maybe_first_tree: Option<RefWalker<S>>,
    maybe_second_tree: Option<RefWalker<S2>>
)
    where
        S: Fetch + Sized + Clone + Send,
        S2: Fetch + Sized + Clone + Send
{
    let (mut first_tree, mut second_tree) = match (maybe_first_tree, maybe_second_tree) {
        (None, None) => return,
        (None, Some(tree)) => {
            display_diff(
                "(empty)",
                format!("key={:?}", tree.tree().key())
            );
            return;
        },
        (Some(tree), None) => {
            display_diff(
                format!("key={:?}", tree.tree().key()),
                "(empty)"
            );
            return;
        },
        (Some(first), Some(second)) => (first, second)
    };

    if first_tree.tree().hash() == second_tree.tree().hash() {
        return;
    }

    if first_tree.tree().key() != second_tree.tree().key() {
        display_diff(
            format!("key={:x?}", first_tree.tree().key()),
            format!("key={:x?}", second_tree.tree().key())
        );
        return;
    }

    if first_tree.tree().value() != second_tree.tree().value() {
        display_diff(
            format!("value={:x?}", first_tree.tree().value()),
            format!("value={:x?}", second_tree.tree().value())
        );
        return;
    }

    // recurse into children (if any)
    compare_trees(
        first_tree.walk(true).unwrap(),
        second_tree.walk(true).unwrap()
    );
    compare_trees(
        first_tree.walk(false).unwrap(),
        second_tree.walk(false).unwrap()
    );
}

fn display_diff<A: AsRef<str>, B: AsRef<str>>(first: A, second: B) {
    println!("
< {}
> {}
    ", first.as_ref(), second.as_ref());
}
