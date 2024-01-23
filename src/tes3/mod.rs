mod archive;
mod file;
mod hashing;

pub use self::{
    archive::{Archive, Key as ArchiveKey},
    file::File,
    hashing::{hash_file, hash_file_in_place, FileHash, Hash},
};

use core::num::TryFromIntError;
use std::io;

#[non_exhaustive]
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("an operation on an integer would have truncated and corrupted data")]
    IntegralTruncation,

    #[error("invalid magic read from archive header: {0}")]
    InvalidMagic(u32),

    #[error(transparent)]
    Io(#[from] io::Error),
}

impl From<TryFromIntError> for Error {
    fn from(_: TryFromIntError) -> Self {
        Self::IntegralTruncation
    }
}

pub type Result<T> = core::result::Result<T, Error>;
