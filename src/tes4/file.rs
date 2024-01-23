use crate::{
    containers::CompressableBytes,
    derive,
    tes4::{CompressionCodec, Error, Result, Version},
};
use flate2::{
    write::{ZlibDecoder, ZlibEncoder},
    Compression,
};
use lzzzz::lz4f::{self, AutoFlush, PreferencesBuilder};
use std::io::Write;

#[repr(transparent)]
pub struct OptionsBuilder(Options);

impl OptionsBuilder {
    #[must_use]
    pub fn build(self) -> Options {
        self.0
    }

    #[must_use]
    pub fn compression_options(mut self, compression_codec: CompressionCodec) -> Self {
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

impl Default for OptionsBuilder {
    fn default() -> Self {
        Self(Options {
            version: Version::default(),
            compression_codec: CompressionCodec::default(),
        })
    }
}

#[derive(Clone, Copy)]
pub struct Options {
    version: Version,
    compression_codec: CompressionCodec,
}

impl Options {
    #[must_use]
    pub fn builder() -> OptionsBuilder {
        OptionsBuilder::new()
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

#[derive(Default)]
pub struct File<'bytes> {
    pub(crate) bytes: CompressableBytes<'bytes>,
}

type ReadResult<T> = T;
derive::compressable_bytes!(File);
derive::reader!(File => ReadResult);

impl<'bytes> File<'bytes> {
    pub fn compress_into(&self, out: &mut Vec<u8>, options: &Options) -> Result<()> {
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

    pub fn decompress_into(&self, out: &mut Vec<u8>, options: &Options) -> Result<()> {
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

    fn from_bytes(bytes: CompressableBytes<'_>) -> File<'_> {
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

    #[allow(clippy::unnecessary_wraps)]
    fn do_read<In>(stream: &mut In) -> Result<ReadResult<Self>>
    where
        In: ?::core::marker::Sized + crate::io::Source<'bytes>,
    {
        Ok(Self::from_bytes(
            stream.read_bytes_to_end().into_compressable(None),
        ))
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
