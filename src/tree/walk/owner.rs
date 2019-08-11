use std::ops::{Deref, DerefMut};

pub struct Owner<T> {
    inner: Option<T>
}

impl<T> Owner<T> {
    pub fn new(value: T) -> Owner<T> {
        Owner { inner: Some(value) }
    }

    // TODO: rename to own_return, with an alternate own which doesn't
    //       return anything other than the owned value
    pub fn own<R, F>(&mut self, f: F) -> R
        where
            R: Sized,
            F: FnOnce(T) -> (T, R)
    {
        let old_value = unwrap(self.inner.take());
        let (new_value, return_value) = f(old_value);
        self.inner = Some(new_value);
        return_value
    }

    pub fn into_inner(mut self) -> T {
        unwrap(self.inner.take())
    }
}

impl<T> Deref for Owner<T> {
    type Target = T;

    fn deref(&self) -> &T {
        unwrap(self.inner.as_ref())
    }
}

impl<T> DerefMut for Owner<T> {
    fn deref_mut(&mut self) -> &mut T {
        unwrap(self.inner.as_mut())
    }
}

fn unwrap<T>(option: Option<T>) -> T {
    match option {
        Some(value) => value,
        None => unreachable!("value should be Some")
    }
}
