pub struct Context<S>
    where S: Fetch + Sized + Send + Clone
{
    tree: OpTree,
    source: S
}
