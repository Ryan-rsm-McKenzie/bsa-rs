#![doc(alias = "oblivion")]
#![doc(alias = "fallout 3")]
#![doc(alias = "fo3")]
#![doc(alias = "fallout new vegas")]
#![doc(alias = "new vegas")]
#![doc(alias = "fnv")]
#![doc(alias = "tes5")]
#![doc(alias = "skyrim")]
#![doc(alias = "sse")]
#![doc(alias = "special edition")]

//! TES IV: Oblivion
//!
//! *"You ... I've seen you... Let me see your face... You are the one from my dreams... Then the stars were right, and this is the day. Gods give me strength."*
//!
//! This format debuted with Oblivion and sunset with Skyrim: SSE. This is the first format to introduce compression, and primarily utilizes zlib/lz4 for this purpose. Unlike other formats, [`tes4`](crate::tes4) utilizes a split architecture where files and directories are tracked as separate paths, rather than combined.
//!
//! # Reading
//! ```rust
//! use ba2::{
//!     prelude::*,
//!     tes4::{Archive, ArchiveKey, DirectoryKey, FileCompressionOptions},
//! };
//! use std::{fs, path::Path};
//!
//! fn example() -> Option<()> {
//!     let path = Path::new("path/to/oblivion/Data/Oblivion - Voices2.bsa");
//!     let (archive, meta) = Archive::read(path).ok()?;
//!     let file = archive
//!         .get(&ArchiveKey::from(b"sound/voice/oblivion.esm/imperial/m"))?
//!         .get(&DirectoryKey::from(
//!             b"testtoddquest_testtoddhappy_00027fa2_1.mp3",
//!         ))?;
//!     let mut dst = fs::File::create("happy.mp3").ok()?;
//!     let options: FileCompressionOptions = meta.into();
//!     file.write(&mut dst, &options).ok()?;
//!     Some(())
//! }
//! ```
//!
//! # Writing
//! ```rust
//! use ba2::{
//!     prelude::*,
//!     tes4::{
//!         Archive, ArchiveKey, ArchiveOptions, ArchiveTypes, Directory, DirectoryKey, File, Version,
//!     },
//! };
//! use std::fs;
//!
//! fn example() -> Option<()> {
//!     let file = File::from_decompressed(b"Hello world!\n");
//!     let directory: Directory = [(DirectoryKey::from(b"hello.txt"), file)]
//!         .into_iter()
//!         .collect();
//!     let archive: Archive = [(ArchiveKey::from(b"misc"), directory)]
//!         .into_iter()
//!         .collect();
//!     let mut dst = fs::File::create("example.bsa").ok()?;
//!     let options = ArchiveOptions::builder()
//!         .types(ArchiveTypes::MISC)
//!         .version(Version::SSE)
//!         .build();
//!     archive.write(&mut dst, &options).ok()?;
//!     Some(())
//! }
//! ```

mod archive;
mod directory;
mod file;
mod hashing;

pub use self::{
    archive::{
        Archive, Flags as ArchiveFlags, Key as ArchiveKey, Options as ArchiveOptions,
        OptionsBuilder as ArchiveOptionsBuilder, Types as ArchiveTypes,
    },
    directory::{Directory, Key as DirectoryKey},
    file::{
        CompressionOptions as FileCompressionOptions,
        CompressionOptionsBuilder as FileCompressionOptionsBuilder, File,
        ReadOptions as FileReadOptions, ReadOptionsBuilder as FileReadOptionsBuilder,
    },
    hashing::{
        hash_directory, hash_directory_in_place, hash_file, hash_file_in_place, DirectoryHash,
        FileHash, Hash,
    },
};

use core::num::TryFromIntError;
use lzzzz::lz4f;
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

    #[error("an operation on two integers would have overflowed and corrupted data ({0})")]
    IntegralOverflow(&'static str),

    #[error("an operation on an integer would have truncated and corrupted data")]
    IntegralTruncation,

    #[error("invalid size read from archive header: {0}")]
    InvalidHeaderSize(u32),

    #[error("invalid magic read from archive header: {0}")]
    InvalidMagic(u32),

    #[error("invalid version read from archive header: {0}")]
    InvalidVersion(u32),

    #[error(transparent)]
    Io(#[from] io::Error),

    #[error(transparent)]
    LZ4(#[from] lz4f::Error),
}

impl From<TryFromIntError> for Error {
    fn from(_: TryFromIntError) -> Self {
        Self::IntegralTruncation
    }
}

pub type Result<T> = core::result::Result<T, Error>;

/// Specifies the codec to use when performing compression/decompression actions on files.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CompressionCodec {
    /// The default compression codec.
    #[default]
    Normal,
    //XMem,
}

/// The archive version.
///
/// Each version has an impact on the abi of the TES4 archive file format.
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub enum Version {
    #[default]
    v103 = 103,
    v104 = 104,
    v105 = 105,
}

impl Version {
    /// The Elder Scrolls IV: Oblivion.
    pub const TES4: Self = Self::v103;
    /// Fallout 3.
    pub const FO3: Self = Self::v104;
    /// Fallout: New Vegas.
    pub const FNV: Self = Self::v104;
    /// The Elder Scrolls V: Skyrim.
    pub const TES5: Self = Self::v104;
    /// The Elder Scrolls V: Skyrim - Special Edition.
    pub const SSE: Self = Self::v105;
}
