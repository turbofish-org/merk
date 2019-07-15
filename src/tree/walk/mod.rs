mod fetch;
// mod detach;

use crate::error::Result;

pub use fetch::FetchWalker;

pub fn unreachable_got_none<T>(left: bool) -> (impl FnOnce() -> T) {
    || {
        let message = format!(
            "Expected node to have {} child, but got None",
            if left { "left" } else { "right" }
        );
        unreachable!(message);
    }
}

pub trait Walk<T> {
    fn walk(&self, left: bool) -> Result<Option<Self>>
        where Self: Sized;

    fn walk_expect(&self, left: bool) -> Result<Self>
        where Self: Sized
    {
        let maybe_child = self.walk(left)?;
        let child = maybe_child.unwrap_or_else(
            unreachable_got_none(left)
        );
        Ok(child)
    }

    fn unwrap(self) -> T;
}

pub trait WalkMut<T> {
    fn walk_mut(&mut self, left: bool) -> Result<Option<Self>>
        where Self: Sized;

    fn walk_mut_expect(&mut self, left: bool) -> Result<Self>
        where Self: Sized
    {
        let maybe_child = self.walk_mut(left)?;
        let child = maybe_child.unwrap_or_else(
            unreachable_got_none(left)
        );
        Ok(child)
    }

    fn unwrap(self) -> T;
}