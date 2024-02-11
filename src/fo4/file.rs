use crate::{
    containers::CompressableBytes,
    derive,
    fo4::{
        ArchiveOptions, Chunk, ChunkCompressionOptions, ChunkDX10, ChunkExtra, CompressionFormat,
        CompressionLevel, Error, Format, Result,
    },
    io::Source,
    CompressionResult, Sealed,
};
use core::{
    fmt::{self, Debug, Display, Formatter},
    num::NonZeroUsize,
    ops::{Index, IndexMut, Range, RangeBounds},
    ptr::NonNull,
    result, slice,
};
use directxtex::{ScratchImage, TexMetadata, CP_FLAGS, DDS_FLAGS, TEX_DIMENSION, TEX_MISC_FLAG};
use std::{error, io::Write};

pub struct CapacityError<'bytes>(Chunk<'bytes>);

impl<'bytes> CapacityError<'bytes> {
    #[must_use]
    pub fn into_element(self) -> Chunk<'bytes> {
        self.0
    }
}

impl<'bytes> Debug for CapacityError<'bytes> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        <Self as Display>::fmt(self, f)
    }
}

impl<'bytes> Display for CapacityError<'bytes> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "could not insert another chunk because the file was already full"
        )
    }
}

impl<'bytes> error::Error for CapacityError<'bytes> {}

#[derive(Debug, Default)]
#[repr(transparent)]
pub struct ReadOptionsBuilder(ReadOptions);

impl ReadOptionsBuilder {
    #[must_use]
    pub fn build(self) -> ReadOptions {
        self.0
    }

    #[must_use]
    pub fn compression_format(mut self, compression_format: CompressionFormat) -> Self {
        self.0.compression_options.compression_format = compression_format;
        self
    }

    #[must_use]
    pub fn compression_level(mut self, compression_level: CompressionLevel) -> Self {
        self.0.compression_options.compression_level = compression_level;
        self
    }

    #[must_use]
    pub fn compression_result(mut self, compression_result: CompressionResult) -> Self {
        self.0.compression_result = compression_result;
        self
    }

    #[must_use]
    pub fn format(mut self, format: Format) -> Self {
        self.0.format = format;
        self
    }

    #[must_use]
    pub fn mip_chunk_height(mut self, mip_chunk_height: usize) -> Self {
        self.0.mip_chunk_height = mip_chunk_height;
        self
    }

    #[must_use]
    pub fn mip_chunk_width(mut self, mip_chunk_width: usize) -> Self {
        self.0.mip_chunk_width = mip_chunk_width;
        self
    }

    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl From<ArchiveOptions> for ReadOptionsBuilder {
    fn from(value: ArchiveOptions) -> Self {
        (&value).into()
    }
}

impl From<&ArchiveOptions> for ReadOptionsBuilder {
    fn from(value: &ArchiveOptions) -> Self {
        Self(value.into())
    }
}

/// Common parameters to configure how files are read.
///
/// ```rust
/// use ba2::{
///     fo4::{CompressionFormat, CompressionLevel, FileReadOptions, Format},
///     CompressionResult,
/// };
///
/// // Read and compress a file for FO4/FO76, GNRL format
/// let _ = FileReadOptions::builder()
///     .format(Format::GNRL)
///     .compression_format(CompressionFormat::Zip)
///     .compression_level(CompressionLevel::FO4)
///     .compression_result(CompressionResult::Compressed)
///     .build();
///
/// // Read and compress a file for FO4/FO76, DX10 format
/// let _ = FileReadOptions::builder()
///     .format(Format::DX10)
///     .compression_format(CompressionFormat::Zip)
///     .compression_level(CompressionLevel::FO4)
///     .compression_result(CompressionResult::Compressed)
///     .build();
///
/// // Read and compress a file for FO4 on the xbox, GNRL format
/// let _ = FileReadOptions::builder()
///     .format(Format::GNRL)
///     .compression_format(CompressionFormat::Zip)
///     .compression_level(CompressionLevel::FO4Xbox)
///     .compression_result(CompressionResult::Compressed)
///     .build();
///
/// // Read and compress a file for FO4 on the xbox, DX10 format
/// let _ = FileReadOptions::builder()
///     .format(Format::DX10)
///     .compression_format(CompressionFormat::Zip)
///     .compression_level(CompressionLevel::FO4Xbox)
///     .compression_result(CompressionResult::Compressed)
///     .build();
///
/// // Read and compress a file for SF, GNRL format
/// let _ = FileReadOptions::builder()
///     .format(Format::GNRL)
///     .compression_format(CompressionFormat::Zip)
///     .compression_level(CompressionLevel::SF)
///     .compression_result(CompressionResult::Compressed)
///     .build();
///
/// // Read and compress a file for SF, DX10 format
/// let _ = FileReadOptions::builder()
///     .format(Format::DX10)
///     .compression_format(CompressionFormat::LZ4)
///     .compression_result(CompressionResult::Compressed)
///     .build();
/// ```
#[derive(Clone, Copy, Debug)]
pub struct ReadOptions {
    format: Format,
    mip_chunk_width: usize,
    mip_chunk_height: usize,
    compression_options: ChunkCompressionOptions,
    compression_result: CompressionResult,
}

impl ReadOptions {
    #[must_use]
    pub fn builder() -> ReadOptionsBuilder {
        ReadOptionsBuilder::new()
    }

    #[must_use]
    pub fn compression_format(&self) -> CompressionFormat {
        self.compression_options.compression_format
    }

    #[must_use]
    pub fn compression_level(&self) -> CompressionLevel {
        self.compression_options.compression_level
    }

    #[must_use]
    pub fn compression_result(&self) -> CompressionResult {
        self.compression_result
    }

    #[must_use]
    pub fn format(&self) -> Format {
        self.format
    }

    #[must_use]
    pub fn mip_chunk_height(&self) -> usize {
        self.mip_chunk_height
    }

    #[must_use]
    pub fn mip_chunk_width(&self) -> usize {
        self.mip_chunk_width
    }
}

impl Default for ReadOptions {
    fn default() -> Self {
        Self {
            format: Format::default(),
            mip_chunk_width: 512,
            mip_chunk_height: 512,
            compression_options: ChunkCompressionOptions::default(),
            compression_result: CompressionResult::default(),
        }
    }
}

impl From<ArchiveOptions> for ReadOptions {
    fn from(value: ArchiveOptions) -> Self {
        (&value).into()
    }
}

impl From<&ArchiveOptions> for ReadOptions {
    fn from(value: &ArchiveOptions) -> Self {
        Self {
            format: value.format(),
            compression_options: value.into(),
            ..Default::default()
        }
    }
}

#[derive(Debug, Default)]
#[repr(transparent)]
pub struct WriteOptionsBuilder(WriteOptions);

impl WriteOptionsBuilder {
    #[must_use]
    pub fn build(self) -> WriteOptions {
        self.0
    }

    #[must_use]
    pub fn compression_format(mut self, compression_format: CompressionFormat) -> Self {
        self.0.compression_format = compression_format;
        self
    }

    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl From<ArchiveOptions> for WriteOptionsBuilder {
    fn from(value: ArchiveOptions) -> Self {
        (&value).into()
    }
}

impl From<&ArchiveOptions> for WriteOptionsBuilder {
    fn from(value: &ArchiveOptions) -> Self {
        Self(value.into())
    }
}

/// Common parameters to configure how files are written.
///
/// ```rust
/// use ba2::fo4::{CompressionFormat, FileWriteOptions, Format};
///
/// // Write a file for FO4/FO76
/// let _ = FileWriteOptions::builder()
///     .compression_format(CompressionFormat::Zip)
///     .build();
///
/// // Write a file for SF, GNRL format
/// let _ = FileWriteOptions::builder()
///     .compression_format(CompressionFormat::Zip)
///     .build();
///
/// // Write a file for SF, DX10 format
/// let _ = FileWriteOptions::builder()
///     .compression_format(CompressionFormat::LZ4)
///     .build();
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct WriteOptions {
    compression_format: CompressionFormat,
}

impl WriteOptions {
    #[must_use]
    pub fn builder() -> WriteOptionsBuilder {
        WriteOptionsBuilder::new()
    }

    #[must_use]
    pub fn compression_format(&self) -> CompressionFormat {
        self.compression_format
    }
}

impl From<ArchiveOptions> for WriteOptions {
    fn from(value: ArchiveOptions) -> Self {
        (&value).into()
    }
}

impl From<&ArchiveOptions> for WriteOptions {
    fn from(value: &ArchiveOptions) -> Self {
        Self {
            compression_format: value.compression_format(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DX10 {
    pub height: u16,
    pub width: u16,
    pub mip_count: u8,
    pub format: u8,
    pub flags: u8,
    pub tile_mode: u8,
}

#[allow(clippy::upper_case_acronyms)]
#[non_exhaustive]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum Header {
    #[default]
    GNRL,
    DX10(DX10),
}

impl From<DX10> for Header {
    fn from(value: DX10) -> Self {
        Self::DX10(value)
    }
}

type Container<'bytes> = Vec<Chunk<'bytes>>;

/// Represents a file within the FO4 virtual filesystem.
#[derive(Clone, Debug, Default)]
pub struct File<'bytes> {
    pub(crate) chunks: Container<'bytes>,
    pub header: Header,
}

impl<'bytes> Sealed for File<'bytes> {}

type ReadResult<T> = T;
derive::reader_with_options!((File: ReadOptions) => ReadResult);

impl<'bytes> File<'bytes> {
    #[must_use]
    pub fn as_mut_ptr(&mut self) -> *mut Chunk<'bytes> {
        self.chunks.as_mut_ptr()
    }

    #[must_use]
    pub fn as_mut_slice(&mut self) -> &mut [Chunk<'bytes>] {
        self.chunks.as_mut_slice()
    }

    #[must_use]
    pub fn as_ptr(&self) -> *const Chunk<'bytes> {
        self.chunks.as_ptr()
    }

    #[must_use]
    pub fn as_slice(&self) -> &[Chunk<'bytes>] {
        self.chunks.as_slice()
    }

    pub fn clear(&mut self) {
        self.chunks.clear();
    }

    /// # Panics
    ///
    /// Panics if [`start_bound`](RangeBounds::start_bound) exceeds [`end_bound`](RangeBounds::end_bound), or if [`end_bound`](RangeBounds::end_bound) exceeds [`len`](Self::len).
    pub fn drain<R>(&mut self, range: R) -> impl Iterator<Item = Chunk<'bytes>> + '_
    where
        R: RangeBounds<usize>,
    {
        self.chunks.drain(range)
    }

    /// # Panics
    ///
    /// Panics if `index` exceeds [`len`](Self::len), or [`is_full`](Self::is_full).
    pub fn insert(&mut self, index: usize, element: Chunk<'bytes>) {
        self.try_insert(index, element).unwrap();
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    #[must_use]
    pub fn is_full(&self) -> bool {
        self.len() >= 4
    }

    pub fn iter(&self) -> impl Iterator<Item = &Chunk<'bytes>> {
        self.chunks.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Chunk<'bytes>> {
        self.chunks.iter_mut()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.chunks.len()
    }

    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn pop(&mut self) -> Option<Chunk<'bytes>> {
        self.chunks.pop()
    }

    /// # Panics
    ///
    /// Panics if [`is_full`](Self::is_full).
    pub fn push(&mut self, element: Chunk<'bytes>) {
        self.try_push(element).unwrap();
    }

    #[must_use]
    pub fn remaining_capacity(&self) -> usize {
        4usize.saturating_sub(self.len())
    }

    /// # Panics
    ///
    /// Panics if `index` exceeds [`len`](Self::len).
    pub fn remove(&mut self, index: usize) -> Chunk<'bytes> {
        self.chunks.remove(index)
    }

    pub fn retain_mut<F>(&mut self, f: F)
    where
        F: FnMut(&mut Chunk<'bytes>) -> bool,
    {
        self.chunks.retain_mut(f);
    }

    /// # Panics
    ///
    /// Panics if `index` exceeds [`len`](Self::len), or [`is_empty`](Self::is_empty).
    pub fn swap_remove(&mut self, index: usize) -> Chunk<'bytes> {
        self.try_swap_remove(index).unwrap()
    }

    pub fn truncate(&mut self, len: usize) {
        self.chunks.truncate(len);
    }

    /// # Panics
    ///
    /// Panics if `index` exceeds [`len`](Self::len).
    pub fn try_insert(
        &mut self,
        index: usize,
        element: Chunk<'bytes>,
    ) -> result::Result<(), CapacityError<'bytes>> {
        if self.is_full() {
            Err(CapacityError(element))
        } else {
            self.do_reserve();
            self.chunks.insert(index, element);
            Ok(())
        }
    }

    pub fn try_push(
        &mut self,
        element: Chunk<'bytes>,
    ) -> result::Result<(), CapacityError<'bytes>> {
        if self.is_full() {
            Err(CapacityError(element))
        } else {
            self.do_reserve();
            self.chunks.push(element);
            Ok(())
        }
    }

    /// # Panics
    ///
    /// Panics if `index` exceeds [`len`](Self::len).
    pub fn try_swap_remove(&mut self, index: usize) -> Option<Chunk<'bytes>> {
        if index < self.len() {
            Some(self.chunks.swap_remove(index))
        } else {
            None
        }
    }

    pub fn write<Out>(&self, stream: &mut Out, options: &WriteOptions) -> Result<()>
    where
        Out: ?Sized + Write,
    {
        match self.header {
            Header::GNRL => self.write_gnrl(stream, *options)?,
            Header::DX10(x) => self.write_dx10(stream, *options, x)?,
        }

        Ok(())
    }

    fn do_reserve(&mut self) {
        match self.len() {
            0 | 3 => self.chunks.reserve_exact(1),
            1 => self.chunks.reserve_exact(3),
            2 => self.chunks.reserve_exact(2),
            _ => (),
        }
    }

    fn do_read<In>(stream: &mut In, options: &ReadOptions) -> Result<Self>
    where
        In: ?Sized + Source<'bytes>,
    {
        let mut this = match options.format {
            Format::GNRL => Self::read_gnrl(stream),
            Format::DX10 => Self::read_dx10(stream, options),
        }?;

        if options.compression_result == CompressionResult::Compressed {
            for chunk in &mut this {
                *chunk = chunk.compress(&options.compression_options)?;
            }
        }

        Ok(this)
    }

    fn read_dx10<In>(stream: &In, options: &ReadOptions) -> Result<Self>
    where
        In: ?Sized + Source<'bytes>,
    {
        let scratch =
            ScratchImage::load_dds(stream.as_bytes(), DDS_FLAGS::DDS_FLAGS_NONE, None, None)?;
        let meta = scratch.metadata();
        let is_cubemap = meta.is_cubemap();
        let header: Header = DX10 {
            height: meta.height.try_into()?,
            width: meta.width.try_into()?,
            mip_count: meta.mip_levels.try_into()?,
            format: meta.format.bits().try_into()?,
            flags: is_cubemap.into(),
            tile_mode: 8,
        }
        .into();

        let images = scratch.images();
        let chunk_from_mips = |range: Range<usize>| -> Result<Chunk> {
            let try_clamp = |num: usize| -> Result<u16> {
                let result = usize::min(meta.mip_levels.saturating_sub(1), num).try_into()?;
                Ok(result)
            };
            let mips = try_clamp(range.start)?..=try_clamp(range.end - 1)?;
            let mut bytes = Vec::new();
            for image in &images[range] {
                let ptr = NonNull::new(image.pixels).unwrap_or(NonNull::dangling());
                let pixels = unsafe { slice::from_raw_parts(ptr.as_ptr(), image.slice_pitch) };
                bytes.extend_from_slice(pixels);
            }
            Ok(Chunk {
                // dxtex always allocates internally, so we have to copy bytes and use from_owned here
                bytes: CompressableBytes::from_owned(bytes.into(), None),
                extra: ChunkDX10 { mips }.into(),
            })
        };

        let chunks = if let Some(images_len) = NonZeroUsize::new(images.len()) {
            if is_cubemap {
                // don't chunk cubemaps
                let chunk = chunk_from_mips(0..images_len.get())?;
                [chunk].into_iter().collect()
            } else {
                let pitch = meta.format.compute_pitch(
                    options.mip_chunk_width,
                    options.mip_chunk_height,
                    CP_FLAGS::CP_FLAGS_NONE,
                )?;

                let mut v = Vec::with_capacity(4);
                let mut size = 0;
                let mut start = 0;
                let mut stop = 0;
                loop {
                    let image = &images[stop];
                    if size == 0 || size + image.slice_pitch < pitch.slice {
                        size += image.slice_pitch;
                    } else {
                        let chunk = chunk_from_mips(start..stop)?;
                        v.push(chunk);
                        start = stop;
                        size = image.slice_pitch;
                    }

                    stop += 1;
                    if stop == images_len.get() || v.len() == 3 {
                        break;
                    }
                }

                if stop < images_len.get() {
                    let chunk = chunk_from_mips(stop..images_len.get())?;
                    v.push(chunk);
                } else {
                    let chunk = chunk_from_mips(start..stop)?;
                    v.push(chunk);
                }

                debug_assert!(v.len() <= 4);
                v
            }
        } else {
            Vec::new()
        };

        Ok(Self { chunks, header })
    }

    #[allow(clippy::unnecessary_wraps)]
    fn read_gnrl<In>(stream: &mut In) -> Result<Self>
    where
        In: ?Sized + Source<'bytes>,
    {
        let bytes = stream.read_bytes_to_end().into_compressable(None);
        let chunk = Chunk {
            bytes,
            extra: ChunkExtra::GNRL,
        };
        Ok([chunk].into_iter().collect())
    }

    fn write_dx10<Out>(&self, stream: &mut Out, options: WriteOptions, dx10: DX10) -> Result<()>
    where
        Out: ?Sized + Write,
    {
        let meta = TexMetadata {
            width: dx10.width.into(),
            height: dx10.height.into(),
            depth: 1,
            array_size: 1,
            mip_levels: dx10.mip_count.into(),
            misc_flags: if (dx10.flags & 1) == 0 {
                0
            } else {
                #[allow(clippy::useless_conversion)]
                {
                    TEX_MISC_FLAG::TEX_MISC_TEXTURECUBE.bits().try_into()?
                }
            },
            misc_flags2: 0,
            format: u32::from(dx10.format).into(),
            dimension: TEX_DIMENSION::TEX_DIMENSION_TEXTURE2D,
        };

        let header = meta.encode_dds_header(DDS_FLAGS::DDS_FLAGS_NONE)?;
        stream.write_all(&header)?;
        self.write_gnrl(stream, options)
    }

    fn write_gnrl<Out>(&self, stream: &mut Out, options: WriteOptions) -> Result<()>
    where
        Out: ?Sized + Write,
    {
        let mut buf = Vec::new();
        let options = ChunkCompressionOptions::builder()
            .compression_format(options.compression_format)
            .build();

        for chunk in self {
            let bytes = if chunk.is_compressed() {
                buf.clear();
                chunk.decompress_into(&mut buf, &options)?;
                &buf
            } else {
                chunk.as_bytes()
            };
            stream.write_all(bytes)?;
        }

        Ok(())
    }
}

impl<'bytes> Index<usize> for File<'bytes> {
    type Output = Chunk<'bytes>;

    fn index(&self, index: usize) -> &Self::Output {
        &self.chunks[index]
    }
}

impl<'bytes> IndexMut<usize> for File<'bytes> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.chunks[index]
    }
}

impl<'bytes> FromIterator<Chunk<'bytes>> for File<'bytes> {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = Chunk<'bytes>>,
    {
        let chunks: Vec<_> = iter.into_iter().collect();
        assert!(chunks.len() <= 4);
        Self {
            chunks,
            header: Header::default(),
        }
    }
}

impl<'bytes> IntoIterator for File<'bytes> {
    type Item = <Container<'bytes> as IntoIterator>::Item;
    type IntoIter = <Container<'bytes> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.chunks.into_iter()
    }
}

impl<'bytes, 'this> IntoIterator for &'this File<'bytes> {
    type Item = <&'this Container<'bytes> as IntoIterator>::Item;
    type IntoIter = <&'this Container<'bytes> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.chunks.iter()
    }
}

impl<'bytes, 'this> IntoIterator for &'this mut File<'bytes> {
    type Item = <&'this mut Container<'bytes> as IntoIterator>::Item;
    type IntoIter = <&'this mut Container<'bytes> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.chunks.iter_mut()
    }
}

#[cfg(test)]
mod tests {
    use crate::fo4::File;

    #[test]
    fn default_state() {
        let f = File::default();
        assert!(f.is_empty());
        assert!(f.as_slice().is_empty());
        assert!(!f.is_full());
    }
}
