use crate::{
    containers::CompressableBytes,
    derive,
    fo4::{ArchiveOptions, CompressionFormat, CompressionLevel, Error, FileWriteOptions, Result},
};
use core::ops::RangeInclusive;
use flate2::{
    write::{ZlibDecoder, ZlibEncoder},
    Compress, Compression,
};
use lzzzz::{lz4, lz4_hc};
use std::io::Write;

/// See also [`ChunkCompressionOptions`](CompressionOptions).
#[derive(Debug, Default)]
#[repr(transparent)]
pub struct CompressionOptionsBuilder(CompressionOptions);

impl CompressionOptionsBuilder {
    #[must_use]
    pub fn build(self) -> CompressionOptions {
        self.0
    }

    #[must_use]
    pub fn compression_format(mut self, compression_format: CompressionFormat) -> Self {
        self.0.compression_format = compression_format;
        self
    }

    #[must_use]
    pub fn compression_level(mut self, compression_level: CompressionLevel) -> Self {
        self.0.compression_level = compression_level;
        self
    }

    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl From<ArchiveOptions> for CompressionOptionsBuilder {
    fn from(value: ArchiveOptions) -> Self {
        (&value).into()
    }
}

impl From<&ArchiveOptions> for CompressionOptionsBuilder {
    fn from(value: &ArchiveOptions) -> Self {
        Self(value.into())
    }
}

impl From<FileWriteOptions> for CompressionOptionsBuilder {
    fn from(value: FileWriteOptions) -> Self {
        (&value).into()
    }
}

impl From<&FileWriteOptions> for CompressionOptionsBuilder {
    fn from(value: &FileWriteOptions) -> Self {
        Self(value.into())
    }
}

/// Common parameters to configure how chunks are compressed.
///
/// ```rust
/// use ba2::fo4::{ChunkCompressionOptions, CompressionFormat, CompressionLevel};
///
/// // Configure for FO4/FO76
/// let _ = ChunkCompressionOptions::builder()
///     .compression_format(CompressionFormat::Zip)
///     .compression_level(CompressionLevel::FO4)
///     .build();
///
/// // Configure for FO4 on the xbox
/// let _ = ChunkCompressionOptions::builder()
///     .compression_format(CompressionFormat::Zip)
///     .compression_level(CompressionLevel::FO4Xbox)
///     .build();
///
/// // Configure for SF, GNRL format
/// let _ = ChunkCompressionOptions::builder()
///     .compression_format(CompressionFormat::Zip)
///     .compression_level(CompressionLevel::SF)
///     .build();
///
/// // Configure for SF, DX10 format
/// let _ = ChunkCompressionOptions::builder()
///     .compression_format(CompressionFormat::LZ4)
///     .build();
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct CompressionOptions {
    pub(crate) compression_format: CompressionFormat,
    pub(crate) compression_level: CompressionLevel,
}

impl CompressionOptions {
    #[must_use]
    pub fn builder() -> CompressionOptionsBuilder {
        CompressionOptionsBuilder::new()
    }

    #[must_use]
    pub fn compression_format(&self) -> CompressionFormat {
        self.compression_format
    }

    #[must_use]
    pub fn compression_level(&self) -> CompressionLevel {
        self.compression_level
    }
}

impl From<ArchiveOptions> for CompressionOptions {
    fn from(value: ArchiveOptions) -> Self {
        (&value).into()
    }
}

impl From<&ArchiveOptions> for CompressionOptions {
    fn from(value: &ArchiveOptions) -> Self {
        Self {
            compression_format: value.compression_format(),
            ..Default::default()
        }
    }
}

impl From<FileWriteOptions> for CompressionOptions {
    fn from(value: FileWriteOptions) -> Self {
        (&value).into()
    }
}

impl From<&FileWriteOptions> for CompressionOptions {
    fn from(value: &FileWriteOptions) -> Self {
        Self {
            compression_format: value.compression_format(),
            ..Default::default()
        }
    }
}

/// Represents a chunk of a file within the FO4 virtual filesystem.
#[derive(Clone, Debug, Default)]
pub struct Chunk<'bytes> {
    pub(crate) bytes: CompressableBytes<'bytes>,
    pub mips: Option<RangeInclusive<u16>>,
}

derive::compressable_bytes!(Chunk: CompressionOptions);

impl<'bytes> Chunk<'bytes> {
    pub fn compress_into(&self, out: &mut Vec<u8>, options: &CompressionOptions) -> Result<()> {
        if self.is_compressed() {
            Err(Error::AlreadyCompressed)
        } else {
            match options.compression_format {
                CompressionFormat::Zip => match options.compression_level {
                    CompressionLevel::FO4 => {
                        self.compress_into_zlib(out, Compression::default(), 15)
                    }
                    CompressionLevel::FO4Xbox => {
                        self.compress_into_zlib(out, Compression::best(), 12)
                    }
                    CompressionLevel::SF => self.compress_into_zlib(out, Compression::best(), 15),
                },
                CompressionFormat::LZ4 => self.compress_into_lz4(out),
            }
        }
    }

    pub fn decompress_into(&self, out: &mut Vec<u8>, options: &CompressionOptions) -> Result<()> {
        let Some(decompressed_len) = self.decompressed_len() else {
            return Err(Error::AlreadyDecompressed);
        };

        out.reserve_exact(decompressed_len);
        let out_len = match options.compression_format {
            CompressionFormat::Zip => self.decompress_into_zlib(out),
            CompressionFormat::LZ4 => self.decompress_into_lz4(out),
        }?;

        if out_len == decompressed_len {
            Ok(())
        } else {
            Err(Error::DecompressionSizeMismatch {
                expected: decompressed_len,
                actual: out_len,
            })
        }
    }

    pub(crate) fn copy_with<'other>(&self, bytes: CompressableBytes<'other>) -> Chunk<'other> {
        Chunk {
            bytes,
            mips: self.mips.clone(),
        }
    }

    fn compress_into_lz4(&self, out: &mut Vec<u8>) -> Result<()> {
        lz4_hc::compress_to_vec(self.as_bytes(), out, lz4_hc::CLEVEL_MAX)?;
        Ok(())
    }

    fn compress_into_zlib(
        &self,
        out: &mut Vec<u8>,
        level: Compression,
        window_bits: u8,
    ) -> Result<()> {
        let mut e = ZlibEncoder::new_with_compress(
            out,
            Compress::new_with_window_bits(level, true, window_bits),
        );
        e.write_all(self.as_bytes())?;
        e.finish()?;
        Ok(())
    }

    fn decompress_into_lz4(&self, out: &mut [u8]) -> Result<usize> {
        let len = lz4::decompress(self.as_bytes(), out)?;
        Ok(len)
    }

    fn decompress_into_zlib(&self, out: &mut Vec<u8>) -> Result<usize> {
        let mut d = ZlibDecoder::new(out);
        d.write_all(self.as_bytes())?;
        Ok(d.total_out().try_into()?)
    }
}

#[cfg(test)]
mod tests {
    use super::Chunk;

    #[test]
    fn default_state() {
        let c = Chunk::default();
        assert!(c.is_empty());
        assert!(!c.is_compressed());
        assert!(c.is_decompressed());
        assert_eq!(c.len(), 0);
        assert_eq!(c.mips, None);
    }
}
