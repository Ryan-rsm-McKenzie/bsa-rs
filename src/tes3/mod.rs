//! TES III: Morrowind
//!
//!  *"Ahh yes, we've been expecting you. You'll have to be recorded before you're officially released. There are a few ways we can do this, and the choice is yours."*
//!
//! This format debuted and sunset with Morrowind. It is the simplest of all the formats, using no compression or special tricks to organize the data.
//!
//! # Reading
//! ```rust
//! use ba2::{
//!     prelude::*,
//!     tes3::{Archive, ArchiveKey},
//! };
//! use std::{fs, path::Path};
//!
//! fn example() -> Option<()> {
//!     let path = Path::new("path/to/morrowind/Data Files/Morrowind.bsa");
//!     let archive = Archive::read(path).ok()?;
//!     let key: ArchiveKey = b"icons/gold.dds".into();
//!     let file = archive.get(&key)?;
//!     let mut dst = fs::File::create("gold.dds").ok()?;
//!     file.write(&mut dst).ok()?;
//!     Some(())
//! }
//! ```
//!
//! # Writing
//! ```rust
//! use ba2::{
//!     prelude::*,
//!     tes3::{Archive, ArchiveKey, File},
//! };
//! use std::fs;
//!
//! fn example() -> Option<()> {
//!     let file: File = b"Hello world!\n".into();
//!     let key: ArchiveKey = b"hello.txt".into();
//!     let archive: Archive = [(key, file)].into_iter().collect();
//!     let mut dst = fs::File::create("example.bsa").ok()?;
//!     archive.write(&mut dst).ok()?;
//!     Some(())
//! }
//! ```

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
