pub use failure::Error;
pub use thiserror::Error;

#[derive(thiserror::Error, Debug)]
pub enum MyError {
    #[error("Unknown Error")]
    Unknown,
}

pub type Result<T> = std::result::Result<T, Error>;
