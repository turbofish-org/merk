pub mod worker;

use std::fmt;

use Op::*;

pub enum Op<'a> {
    Put(&'a [u8]),
    Delete
}

impl<'a> fmt::Debug for Op<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "{}", match self {
            Put(value) => format!("Put({:?})", value),
            Delete => "Delete".to_string()
        })
    }
}

pub type BatchEntry<'a> = (&'a [u8], Op<'a>);

pub type Batch<'a> = [BatchEntry<'a>];
