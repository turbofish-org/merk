use std::env::temp_dir;
use std::ops::{Deref, DerefMut};
use std::path::Path;
use std::time::SystemTime;
use crate::{Merk, Result};

pub struct TempMerk {
    inner: Option<Merk>
}

impl TempMerk {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<TempMerk> {
        let inner = Some(Merk::open(path)?);
        Ok(TempMerk { inner })
    }

    pub fn new() -> Result<TempMerk> {
        let time = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let mut path = temp_dir();
        path.push(format!("merk-temp–{}", time));
        TempMerk::open(path)
    }
}

impl Drop for TempMerk {
    fn drop(&mut self) {
        self.inner
            .take().unwrap()
            .destroy().expect("failed to delete db");
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
