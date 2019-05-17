use crate::raw::*;

pub struct Error ();

impl From<FromBytesError> for Error {
    fn from(error: FromBytesError) -> Self {
        Self()
    }
}

impl From<std::option::NoneError> for Error {
    fn from(error: std::option::NoneError) -> Self {
        Self()
    }
}

/// Trait for storing and accessing data, either on disk or in memory.
pub trait Store {
    fn get(&self, key: &[u8]) -> Result<Option<&[u8]>, Error>;

    fn put(&self, key: &[u8], value: &[u8]) -> Result<(), Error>;
}
