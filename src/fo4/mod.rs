mod archive;
mod chunk;
mod file;
mod hashing;

pub use self::{
    archive::{Archive, Options as ArchiveOptions, OptionsBuilder as ArchiveOptionsBuilder},
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

use core::num::TryFromIntError;
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CompressionFormat {
    #[default]
    Zip,
    LZ4,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CompressionLevel {
    #[default]
    FO4,
    FO4Xbox,
    SF,
}

impl CompressionLevel {
    pub const FO76: Self = Self::FO4;
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Format {
    #[default]
    GNRL,
    DX10,
}

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub enum Version {
    #[default]
    v1 = 1,
    v2 = 2,
    v3 = 3,
}
