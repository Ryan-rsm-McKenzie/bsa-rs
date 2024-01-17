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
    file::{File, Options as FileOptions, OptionsBuilder as FileOptionsBuilder},
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

    #[error("an operation on two integers would have overflowed and corrupted data")]
    IntegralOverflow,

    #[error("an operation on an integer would have truncated and corrupted data")]
    IntegralTruncation,

    #[error("invalid header size read from file header: {0}")]
    InvalidHeaderSize(u32),

    #[error("invalid magic read from file header: {0}")]
    InvalidMagic(u32),

    #[error("invalid version read from file header: {0}")]
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

#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CompressionCodec {
    #[default]
    Normal,
    //XMem,
}

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub enum Version {
    #[default]
    v103 = 103,
    v104 = 104,
    v105 = 105,
}

impl Version {
    pub const TES4: Self = Self::v103;
    pub const FO3: Self = Self::v104;
    pub const FNV: Self = Self::v104;
    pub const TES5: Self = Self::v104;
    pub const SSE: Self = Self::v105;
}
