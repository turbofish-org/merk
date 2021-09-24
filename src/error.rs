pub use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Index OoB Error: {0}")]
    IndexOutOfBounds(String),
    #[error("Fetch Error: {0}")]
    FetchError(String),
    #[error("Unknown Error")]
    Unknown,
}

pub type Result<T> = std::result::Result<T, Error>;
