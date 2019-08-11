use std::path::Path;
use std::ops::{Deref, DerefMut};
use crate::{Merk, Result};

pub struct TempMerk {
    inner: Option<Merk>
}

impl TempMerk {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<TempMerk> {
        let inner = Some(Merk::open(path)?);
        Ok(TempMerk { inner })
    }
}

impl Drop for TempMerk {
    fn drop(&mut self) {
        self.inner.take().unwrap().destroy();
    }
}

impl Deref for TempMerk {
    type Target = Merk;

    fn deref(&self) -> &Merk {
        self.inner.as_ref().unwrap()
    }
}

impl DerefMut for TempMerk {
    fn deref_mut(&mut self) -> &mut Merk {
        self.inner.as_mut().unwrap()
    }
}
