use crate::{
    containers::CompressableBytes,
    derive,
    io::Source,
    tes4::{CompressionCodec, Error, Result, Version},
    CompressionResult,
};
use flate2::{
    write::{ZlibDecoder, ZlibEncoder},
    Compression,
};
use lzzzz::lz4f::{self, AutoFlush, PreferencesBuilder};
use std::io::Write;

#[derive(Debug, Default)]
#[repr(transparent)]
pub struct CompressionOptionsBuilder(CompressionOptions);

impl CompressionOptionsBuilder {
    #[must_use]
    pub fn build(self) -> CompressionOptions {
        self.0
    }

    #[must_use]
    pub fn compression_codec(mut self, compression_codec: CompressionCodec) -> Self {
        self.0.compression_codec = compression_codec;
        self
    }

    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn version(mut self, version: Version) -> Self {
        self.0.version = version;
        self
    }
}

/// Common parameters to configure how files are compressed/decompressed.
///
/// ```rust
/// use ba2::tes4::{FileCompressionOptions, Version};
///
/// // Configure for TES:IV
/// let _ = FileCompressionOptions::builder()
///     .version(Version::TES4)
///     .build();
///
/// // Configure for F3/FNV/TES:V
/// let _ = FileCompressionOptions::builder()
///     .version(Version::FO3)
///     .build();
///
/// // Configure for SSE
/// let _ = FileCompressionOptions::builder()
///     .version(Version::SSE)
///     .build();
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct CompressionOptions {
    version: Version,
    compression_codec: CompressionCodec,
}

impl CompressionOptions {
    #[must_use]
    pub fn builder() -> CompressionOptionsBuilder {
        CompressionOptionsBuilder::new()
    }

    #[must_use]
    pub fn compression_codec(&self) -> CompressionCodec {
        self.compression_codec
    }

    #[must_use]
    pub fn version(&self) -> Version {
        self.version
    }
}

#[derive(Debug, Default)]
#[repr(transparent)]
pub struct ReadOptionsBuilder(ReadOptions);

impl ReadOptionsBuilder {
    #[must_use]
    pub fn build(self) -> ReadOptions {
        self.0
    }

    #[must_use]
    pub fn compression_codec(mut self, compression_codec: CompressionCodec) -> Self {
        self.0.compression_options.compression_codec = compression_codec;
        self
    }

    #[must_use]
    pub fn compression_result(mut self, compression_result: CompressionResult) -> Self {
        self.0.compression_result = compression_result;
        self
    }

    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn version(mut self, version: Version) -> Self {
        self.0.compression_options.version = version;
        self
    }
}

/// Common parameters to configure how files are read.
///
/// ```rust
/// use ba2::{
///     tes4::{FileReadOptions, Version},
///     CompressionResult,
/// };
///
/// // Read and compress a file for TES:IV
/// let _ = FileReadOptions::builder()
///     .version(Version::TES4)
///     .compression_result(CompressionResult::Compressed)
///     .build();
///
/// // Read and compress a file for F3/FNV/TES:V
/// let _ = FileReadOptions::builder()
///     .version(Version::FO3)
///     .compression_result(CompressionResult::Compressed)
///     .build();
///
/// // Read and compress a file for SSE
/// let _ = FileReadOptions::builder()
///     .version(Version::SSE)
///     .compression_result(CompressionResult::Compressed)
///     .build();
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct ReadOptions {
    compression_options: CompressionOptions,
    compression_result: CompressionResult,
}

impl ReadOptions {
    #[must_use]
    pub fn builder() -> ReadOptionsBuilder {
        ReadOptionsBuilder::new()
    }

    #[must_use]
    pub fn compression_codec(&self) -> CompressionCodec {
        self.compression_options.compression_codec
    }

    #[must_use]
    pub fn compression_result(&self) -> CompressionResult {
        self.compression_result
    }

    #[must_use]
    pub fn version(&self) -> Version {
        self.compression_options.version
    }
}

/// Represents a file within the TES4 virtual filesystem.
#[derive(Clone, Debug, Default)]
pub struct File<'bytes> {
    pub(crate) bytes: CompressableBytes<'bytes>,
}

type ReadResult<T> = T;
derive::compressable_bytes!(File: CompressionOptions);
derive::reader_with_options!((File: ReadOptions) => ReadResult);

impl<'bytes> File<'bytes> {
    pub fn compress_into(&self, out: &mut Vec<u8>, options: &CompressionOptions) -> Result<()> {
        if self.is_compressed() {
            Err(Error::AlreadyCompressed)
        } else {
            match options.version {
                Version::v103 => self.compress_into_zlib(out),
                Version::v104 => match options.compression_codec {
                    CompressionCodec::Normal => self.compress_into_zlib(out),
                },
                Version::v105 => self.compress_into_lz4(out),
            }
        }
    }

    pub fn decompress_into(&self, out: &mut Vec<u8>, options: &CompressionOptions) -> Result<()> {
        let Some(decompressed_len) = self.decompressed_len() else {
            return Err(Error::AlreadyDecompressed);
        };

        out.reserve_exact(decompressed_len);
        let out_len = match options.version {
            Version::v103 => self.decompress_into_zlib(out),
            Version::v104 => match options.compression_codec {
                CompressionCodec::Normal => self.decompress_into_zlib(out),
            },
            Version::v105 => self.decompress_into_lz4(out),
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

    #[allow(clippy::unused_self)]
    fn copy_with<'other>(&self, bytes: CompressableBytes<'other>) -> File<'other> {
        File { bytes }
    }

    fn compress_into_lz4(&self, out: &mut Vec<u8>) -> Result<()> {
        let prefs = PreferencesBuilder::new()
            .compression_level(9)
            .auto_flush(AutoFlush::Enabled)
            .build();
        lz4f::compress_to_vec(self.as_bytes(), out, &prefs)?;
        Ok(())
    }

    fn compress_into_zlib(&self, out: &mut Vec<u8>) -> Result<()> {
        let mut e = ZlibEncoder::new(out, Compression::default());
        e.write_all(self.as_bytes())?;
        e.finish()?;
        Ok(())
    }

    fn decompress_into_lz4(&self, out: &mut Vec<u8>) -> Result<usize> {
        let len = lz4f::decompress_to_vec(self.as_bytes(), out)?;
        Ok(len)
    }

    fn decompress_into_zlib(&self, out: &mut Vec<u8>) -> Result<usize> {
        let mut d = ZlibDecoder::new(out);
        d.write_all(self.as_bytes())?;
        Ok(d.total_out().try_into()?)
    }

    #[allow(clippy::trivially_copy_pass_by_ref)]
    fn do_read<In>(stream: &mut In, options: &ReadOptions) -> Result<ReadResult<Self>>
    where
        In: ?Sized + Source<'bytes>,
    {
        let decompressed = Self {
            bytes: stream.read_bytes_to_end().into_compressable(None),
        };
        match options.compression_result {
            CompressionResult::Decompressed => Ok(decompressed),
            CompressionResult::Compressed => decompressed.compress(&options.compression_options),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{prelude::*, tes4::File};

    #[test]
    fn default_state() {
        let f = File::new();
        assert!(!f.is_compressed());
        assert!(f.is_empty());
        assert_eq!(f.len(), 0);
        assert_eq!(f.as_bytes().len(), 0);
    }

    #[test]
    fn assign_state() {
        let payload = [0u8; 64];
        let f = File::from_decompressed(&payload[..]);
        assert_eq!(f.len(), payload.len());
        assert_eq!(f.as_ptr(), payload.as_ptr());
        assert_eq!(f.as_bytes().len(), payload.len());
        assert_eq!(f.as_bytes().as_ptr(), payload.as_ptr());
    }
}
