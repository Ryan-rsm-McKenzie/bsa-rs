mod chunk;
mod file;
mod hashing;

pub use self::{
    chunk::{
        Chunk, Extra as ChunkExtra, Options as ChunkOptions, OptionsBuilder as ChunkOptionsBuilder,
        DX10 as ChunkDX10,
    },
    file::{
        CapacityError as FileCapacityError, File, Header as FileHeader,
        ReadOptions as FileReadOptions, ReadOptionsBuilder as FileReadOptionsBuilder,
        Reader as FileReader, WriteOptions as FileWriteOptions,
        WriteOptionsBuilder as FileWriteOptionsBuilder, DX10 as FileDX10,
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

    #[error("an operation on an integer would have truncated and corrupted data")]
    IntegralTruncation,

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
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Version {
    #[default]
    v1 = 1,
    v2 = 2,
    v3 = 3,
}
