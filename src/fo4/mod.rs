//! Fallout 4
//!
//! *"Good morning! Vault-Tec calling! ... You can't begin to know how happy I am to finally speak with you. I've been trying for days. It's a matter of utmost urgency, I assure you."*
//!
//! This format is the latest iteration, having debuted with Fallout 4. It primarily uses zlib for compression, but Starfield has introduced lz4 into the mix. Unlike previous formats, texture files are now split into chunks to enable streaming mips at a more granular level.
//!
//! # Reading
//! ```rust
//! use ba2::{
//!     fo4::{Archive, ArchiveKey, FileWriteOptions},
//!     prelude::*,
//! };
//! use std::{fs, path::Path};
//!
//! fn example() -> Option<()> {
//!     let path = Path::new(r"path/to/fallout4/Data/Fallout4 - Interface.ba2");
//!     let (archive, meta) = Archive::read(path).ok()?;
//!     let key: ArchiveKey = b"Interface/HUDMenu.swf".into();
//!     let file = archive.get(&key)?;
//!     let mut dst = fs::File::create("HUDMenu.swf").ok()?;
//!     let options: FileWriteOptions = meta.into();
//!     file.write(&mut dst, &options).ok()?;
//!     Some(())
//! }
//! ```
//!
//! # Writing
//! ```rust
//! use ba2::{
//!     fo4::{Archive, ArchiveKey, ArchiveOptions, Chunk, File},
//!     prelude::*,
//! };
//! use std::fs;
//!
//! fn example() -> Option<()> {
//!     let chunk = Chunk::from_decompressed(b"Hello world!\n");
//!     let file: File = [chunk].into_iter().collect();
//!     let key: ArchiveKey = b"hello.txt".into();
//!     let archive: Archive = [(key, file)].into_iter().collect();
//!     let mut dst = fs::File::create("example.ba2").ok()?;
//!     let options = ArchiveOptions::default();
//!     archive.write(&mut dst, &options).ok()?;
//!     Some(())
//! }
//! ```

mod archive;
mod chunk;
mod file;
mod hashing;

pub use self::{
    archive::{
        Archive, Key as ArchiveKey, Options as ArchiveOptions,
        OptionsBuilder as ArchiveOptionsBuilder,
    },
    chunk::{
        Chunk, CompressionOptions as ChunkCompressionOptions,
        CompressionOptionsBuilder as ChunkCompressionOptionsBuilder, Extra as ChunkExtra,
        DX10 as ChunkDX10,
    },
    file::{
        CapacityError as FileCapacityError, File, Header as FileHeader,
        ReadOptions as FileReadOptions, ReadOptionsBuilder as FileReadOptionsBuilder,
        WriteOptions as FileWriteOptions, WriteOptionsBuilder as FileWriteOptionsBuilder,
        DX10 as FileDX10,
    },
    hashing::{hash_file, hash_file_in_place, FileHash, Hash},
};

use core::{convert::Infallible, num::TryFromIntError};
use directxtex::HResultError;
use std::io;

#[non_exhaustive]
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("can not compress the given file because it is already compressed")]
    AlreadyCompressed,

    #[error("can not decompress the given file because it is already decompressed")]
    AlreadyDecompressed,

    #[error("buffer failed to decompress to the expected size... expected {expected} bytes, but got {actual} bytes")]
    DecompressionSizeMismatch { expected: usize, actual: usize },

    #[error("error while working with a dds file")]
    DX10(#[from] HResultError),

    #[error(
        "attempted to write an archive in a format that does not match a file/chunk in the archive"
    )]
    FormatMismatch,

    #[doc(hidden)]
    #[error(transparent)]
    Infallible(#[from] Infallible),

    #[error("an operation on an integer would have truncated and corrupted data")]
    IntegralTruncation,

    #[error("invalid sentinel read from chunk: {0}")]
    InvalidChunkSentinel(u32),

    #[error("invalid chunk size read from file header: {0}")]
    InvalidChunkSize(u16),

    #[error("invalid format read from archive header: {0}")]
    InvalidFormat(u32),

    #[error("invalid magic read from archive header: {0}")]
    InvalidMagic(u32),

    #[error("invalid version read from archive header: {0}")]
    InvalidVersion(u32),

    #[error(transparent)]
    Io(#[from] io::Error),

    #[error(transparent)]
    LZ4(#[from] lzzzz::Error),
}

impl From<TryFromIntError> for Error {
    fn from(_: TryFromIntError) -> Self {
        Self::IntegralTruncation
    }
}

pub type Result<T> = core::result::Result<T, Error>;

/// A list of all compression methods supported by the ba2 format.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CompressionFormat {
    /// The default compression format, compatible with all games that utilize the ba2 format.
    #[default]
    Zip,

    /// A more specialized format leveraging lz4's fast decompression to improve streaming time.
    ///
    /// Only compatible with Starfield or later.
    LZ4,
}

/// Specifies the compression level to use when compressing data.
///
/// Only compatible with [`CompressionFormat::Zip`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CompressionLevel {
    /// Fallout 4.
    #[default]
    FO4,

    /// Fallout 4 on the xbox.
    ///
    /// Uses a smaller windows size, but higher a compression level to yield a higher compression ratio.
    FO4Xbox,

    /// Starfield.
    ///
    /// Uses a custom DEFLATE algorithm with zlib wrapper to obtain a good compression ratio.
    SF,
}

impl CompressionLevel {
    /// Fallout 76.
    pub const FO76: Self = Self::FO4;
}

/// Represents the file format for an archive.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Format {
    /// A general archive can contain any kind of file.
    #[default]
    GNRL,

    /// A directx archive can only contain .dds files.
    DX10,
}

/// Indicates the version of an archive.
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub enum Version {
    /// Initial format introduced in Fallout 4.
    #[default]
    v1 = 1,

    /// Intoduced in Starfield.
    v2 = 2,

    /// Intoduced in Starfield.
    v3 = 3,
}
