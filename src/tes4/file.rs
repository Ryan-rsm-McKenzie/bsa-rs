use crate::{
    containers::CompressableBytes,
    derive,
    io::Source,
    tes4::{CompressionCodec, Error, Result, Version},
    CompressableFrom,
};
use flate2::{
    write::{ZlibDecoder, ZlibEncoder},
    Compression,
};
use lzzzz::lz4f::{self, AutoFlush, PreferencesBuilder};
use std::io::Write;

#[non_exhaustive]
#[derive(Clone, Copy, Default)]
pub struct CompressionOptions {
    pub version: Version,
    pub compression_codec: CompressionCodec,
}

#[derive(Default)]
pub struct File<'bytes> {
    pub(crate) container: CompressableBytes<'bytes>,
}

type ReadResult<T> = T;
derive::container!(File => ReadResult);

impl<'bytes> File<'bytes> {
    pub fn compress(&self, options: &CompressionOptions) -> Result<File<'static>> {
        let mut bytes = Vec::new();
        self.compress_into(&mut bytes, options)?;
        bytes.shrink_to_fit();
        Ok(File {
            container: CompressableBytes::from_owned(bytes, Some(self.len())),
        })
    }

    pub fn compress_into(&self, out: &mut Vec<u8>, options: &CompressionOptions) -> Result<()> {
        if self.is_compressed() {
            Err(Error::AlreadyCompressed)
        } else {
            match options.version {
                Version::TES4 => self.compress_into_zlib(out),
                Version::FO3 => match options.compression_codec {
                    CompressionCodec::Normal => self.compress_into_zlib(out),
                },
                Version::SSE => self.compress_into_lz4(out),
            }
        }
    }

    pub fn decompress(&self, options: &CompressionOptions) -> Result<File<'static>> {
        let mut bytes = Vec::new();
        self.decompress_into(&mut bytes, options)?;
        bytes.shrink_to_fit();
        Ok(File {
            container: CompressableBytes::from_owned(bytes, None),
        })
    }

    pub fn decompress_into(&self, out: &mut Vec<u8>, options: &CompressionOptions) -> Result<()> {
        let Some(decompressed_len) = self.decompressed_len() else {
            return Err(Error::AlreadyDecompressed);
        };

        out.reserve_exact(decompressed_len);
        let out_len = match options.version {
            Version::TES4 => self.decompress_into_zlib(out),
            Version::FO3 => match options.compression_codec {
                CompressionCodec::Normal => self.decompress_into_zlib(out),
            },
            Version::SSE => self.decompress_into_lz4(out),
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

    #[must_use]
    pub fn decompressed_len(&self) -> Option<usize> {
        self.container.decompressed_len()
    }

    #[must_use]
    pub fn is_compressed(&self) -> bool {
        self.container.is_compressed()
    }

    #[must_use]
    pub fn is_decompressed(&self) -> bool {
        !self.is_compressed()
    }

    pub fn write<Out>(&self, stream: &mut Out, options: &CompressionOptions) -> Result<()>
    where
        Out: ?Sized + Write,
    {
        if self.is_compressed() {
            let mut bytes = Vec::new();
            self.decompress_into(&mut bytes, options)?;
            stream.write_all(&bytes)?;
        } else {
            stream.write_all(self.as_bytes())?;
        }

        Ok(())
    }

    #[allow(clippy::unnecessary_wraps)]
    fn do_read<In>(stream: &mut In) -> Result<ReadResult<Self>>
    where
        In: ?Sized + Source<'bytes>,
    {
        Ok(Self {
            container: stream.read_bytes_to_end().into_compressable(None),
        })
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
}

impl<'bytes> CompressableFrom<&'bytes [u8]> for File<'bytes> {
    fn from_compressed(value: &'bytes [u8], decompressed_len: usize) -> Self {
        Self {
            container: CompressableBytes::from_borrowed(value, Some(decompressed_len)),
        }
    }

    fn from_decompressed(value: &'bytes [u8]) -> Self {
        Self {
            container: CompressableBytes::from_borrowed(value, None),
        }
    }
}

impl CompressableFrom<Vec<u8>> for File<'static> {
    fn from_compressed(value: Vec<u8>, decompressed_len: usize) -> Self {
        Self {
            container: CompressableBytes::from_owned(value, Some(decompressed_len)),
        }
    }

    fn from_decompressed(value: Vec<u8>) -> Self {
        Self {
            container: CompressableBytes::from_owned(value, None),
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
