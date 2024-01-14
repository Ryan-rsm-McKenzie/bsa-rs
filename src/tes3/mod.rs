pub mod archive;
pub mod file;
pub mod hashing;

pub use self::{
    archive::{Archive, Key as ArchiveKey},
    file::File,
    hashing::{hash_file, hash_file_in_place, Hash},
};

use core::num::TryFromIntError;
use std::io;

#[non_exhaustive]
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    IntegralTruncation(#[from] TryFromIntError),

    #[error("invalid magic read from file header: {0}")]
    InvalidMagic(u32),

    #[error(transparent)]
    Io(#[from] io::Error),
}

pub type Result<T> = core::result::Result<T, Error>;
