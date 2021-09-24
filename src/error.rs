pub use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Unknown Error")]
    Unknown,
}

pub type Result<T> = std::result::Result<T, Error>;
