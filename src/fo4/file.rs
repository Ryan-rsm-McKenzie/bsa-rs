use crate::{
    containers::CompressableBytes,
    derive,
    fo4::{
        ArchiveOptions, Chunk, ChunkCompressionOptions, CompressionFormat, CompressionLevel, Error,
        Format, Result,
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
use directxtex::{
    ScratchImage, TexMetadata, CP_FLAGS, DDS_FLAGS, DXGI_FORMAT, FORMAT_TYPE, TEX_DIMENSION,
    TEX_MISC_FLAG,
};
use std::{error, io::Write};

#[allow(clippy::unnecessary_cast)]
const TEX_MISC_TEXTURECUBE: u32 = TEX_MISC_FLAG::TEX_MISC_TEXTURECUBE.bits() as u32;

/// File is at chunk capacity.
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

/// See also [`FileReadOptions`](ReadOptions).
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

/// See also [`FileWriteOptions`](WriteOptions).
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

/// File header for DX10 archives.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DX10 {
    pub height: u16,
    pub width: u16,
    pub mip_count: u8,
    pub format: u8,
    pub flags: u8,
    pub tile_mode: u8,
}

/// File header for GNMF archives.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GNMF {
    /// See [here](https://github.com/tge-was-taken/GFD-Studio/blob/dad6c2183a6ec0716c3943b71991733bfbd4649d/GFDLibrary/Textures/GNF/GNFTexture.cs#L529-L536) for more info.
    pub metadata: [u32; 8],
}

macro_rules! bit_field {
    ($getter:ident, $setter:ident, [$slot:literal], $count:literal << $shift:literal,) => {
        #[allow(unused)]
        fn $getter(&self) -> u32 {
            const MASK: u32 = {
                let mut i = 0;
                let mut mask: u32 = 0;
                while i < $count {
                    mask = (mask << 1) | 1;
                    i += 1;
                }
                mask << $shift
            };
            (self.metadata[$slot] & MASK) >> $shift
        }

        #[allow(unused)]
        fn $setter(&mut self, $getter: u32) -> &mut Self {
            const MASK: u32 = {
                let mut i = 0;
                let mut mask: u32 = 0;
                while i < $count {
                    mask = (mask << 1) | 1;
                    i += 1;
                }
                mask << $shift
            };
            self.metadata[$slot] |= ($getter << $shift) & MASK;
            self
        }
    };
}

impl GNMF {
    bit_field! {
        min_lod_clamp,
        with_min_lod_clamp,
        [1],
        12 << 8,
    }

    bit_field! {
        surface_format,
        with_surface_format,
        [1],
        6 << 20,
    }

    bit_field! {
        channel_type,
        with_channel_type,
        [1],
        4 << 26,
    }

    bit_field! {
        width,
        with_width,
        [2],
        14 << 0,
    }

    bit_field! {
        height,
        with_height,
        [2],
        14 << 14,
    }

    bit_field! {
        sampler_modulation_factor,
        with_sampler_modulation_factor,
        [2],
        3 << 28,
    }

    bit_field! {
        channel_order_x,
        with_channel_order_x,
        [3],
        3 << 0,
    }

    bit_field! {
        channel_order_y,
        with_channel_order_y,
        [3],
        3 << 3,
    }

    bit_field! {
        channel_order_z,
        with_channel_order_z,
        [3],
        3 << 6,
    }

    bit_field! {
        channel_order_w,
        with_channel_order_w,
        [3],
        3 << 9,
    }

    bit_field! {
        base_mip_level,
        with_base_mip_level,
        [3],
        4 << 12,
    }

    bit_field! {
        last_mip_level,
        with_last_mip_level,
        [3],
        4 << 16,
    }

    bit_field! {
        tile_mode,
        with_tile_mode,
        [3],
        5 << 20,
    }

    bit_field! {
        padded_to_pow2,
        with_padded_to_pow2,
        [3],
        1 << 25,
    }

    bit_field! {
        texture_type,
        with_texture_type,
        [3],
        4 << 28,
    }

    bit_field! {
        depth,
        with_depth,
        [4],
        13 << 0,
    }

    bit_field! {
        pitch,
        with_pitch,
        [4],
        14 << 13,
    }

    bit_field! {
        base_array_slice_index,
        with_base_array_slice_index,
        [5],
        13 << 0,
    }

    bit_field! {
        last_array_slice_index,
        with_last_array_slice_index,
        [5],
        13 << 13,
    }

    bit_field! {
        min_lod_warning,
        with_min_lod_warning,
        [6],
        12 << 0,
    }

    bit_field! {
        mip_stats_counter_index,
        with_mip_stats_counter_index,
        [6],
        8 << 12,
    }

    bit_field! {
        mip_stats_enabled,
        with_mip_stats_enabled,
        [6],
        1 << 20,
    }

    bit_field! {
        metadata_compression_enabled,
        with_metadata_compression_enabled,
        [6],
        1 << 21,
    }

    bit_field! {
        dcc_alpha_on_msb,
        with_dcc_alpha_on_msb,
        [6],
        1 << 22,
    }

    bit_field! {
        dcc_color_transform,
        with_dcc_color_transform,
        [6],
        1 << 23,
    }

    bit_field! {
        use_alt_tile_mode,
        with_use_alt_tile_mode,
        [6],
        1 << 24,
    }

    fn block_size(&self) -> Result<usize> {
        // https://learn.microsoft.com/en-us/windows/win32/direct3d11/texture-block-compression-in-direct3d-11
        match self.surface_format() {
            SurfaceFormat::FORMAT_8_8_8_8 => Ok(4),
            SurfaceFormat::BC1 | SurfaceFormat::BC4 => Ok(8),
            SurfaceFormat::BC2
            | SurfaceFormat::BC3
            | SurfaceFormat::BC5
            | SurfaceFormat::BC6
            | SurfaceFormat::BC7 => Ok(16),
            _ => Err(Error::NotImplemented),
        }
    }
}

impl Default for GNMF {
    fn default() -> Self {
        let mut this = Self {
            metadata: Default::default(),
        };
        _ = this
            .with_sampler_modulation_factor(SamplerModulationFactor::FACTOR_1_0000)
            .with_channel_order_x(TextureChannel::X)
            .with_channel_order_y(TextureChannel::Y)
            .with_channel_order_z(TextureChannel::Z)
            .with_channel_order_w(TextureChannel::W)
            .with_tile_mode(TileMode::THIN_1D_THIN)
            .with_texture_type(TextureType::TYPE_2D)
            .with_depth(1);
        this
    }
}

impl TryFrom<&TexMetadata> for GNMF {
    type Error = Error;

    fn try_from(value: &TexMetadata) -> Result<Self> {
        let mut this = Self::default();
        _ = this
            .with_surface_format(match value.format {
                DXGI_FORMAT::DXGI_FORMAT_R8G8B8A8_TYPELESS
                | DXGI_FORMAT::DXGI_FORMAT_R8G8B8A8_UNORM
                | DXGI_FORMAT::DXGI_FORMAT_R8G8B8A8_UNORM_SRGB
                | DXGI_FORMAT::DXGI_FORMAT_R8G8B8A8_UINT
                | DXGI_FORMAT::DXGI_FORMAT_R8G8B8A8_SNORM
                | DXGI_FORMAT::DXGI_FORMAT_R8G8B8A8_SINT => SurfaceFormat::FORMAT_8_8_8_8,
                DXGI_FORMAT::DXGI_FORMAT_BC1_TYPELESS
                | DXGI_FORMAT::DXGI_FORMAT_BC1_UNORM
                | DXGI_FORMAT::DXGI_FORMAT_BC1_UNORM_SRGB => SurfaceFormat::BC1,
                DXGI_FORMAT::DXGI_FORMAT_BC2_TYPELESS
                | DXGI_FORMAT::DXGI_FORMAT_BC2_UNORM
                | DXGI_FORMAT::DXGI_FORMAT_BC2_UNORM_SRGB => SurfaceFormat::BC2,
                DXGI_FORMAT::DXGI_FORMAT_BC3_TYPELESS
                | DXGI_FORMAT::DXGI_FORMAT_BC3_UNORM
                | DXGI_FORMAT::DXGI_FORMAT_BC3_UNORM_SRGB => SurfaceFormat::BC3,
                DXGI_FORMAT::DXGI_FORMAT_BC4_TYPELESS
                | DXGI_FORMAT::DXGI_FORMAT_BC4_UNORM
                | DXGI_FORMAT::DXGI_FORMAT_BC4_SNORM => SurfaceFormat::BC4,
                DXGI_FORMAT::DXGI_FORMAT_BC5_TYPELESS
                | DXGI_FORMAT::DXGI_FORMAT_BC5_UNORM
                | DXGI_FORMAT::DXGI_FORMAT_BC5_SNORM => SurfaceFormat::BC5,
                DXGI_FORMAT::DXGI_FORMAT_BC6H_TYPELESS
                | DXGI_FORMAT::DXGI_FORMAT_BC6H_UF16
                | DXGI_FORMAT::DXGI_FORMAT_BC6H_SF16 => SurfaceFormat::BC6,
                DXGI_FORMAT::DXGI_FORMAT_BC7_TYPELESS
                | DXGI_FORMAT::DXGI_FORMAT_BC7_UNORM
                | DXGI_FORMAT::DXGI_FORMAT_BC7_UNORM_SRGB => SurfaceFormat::BC7,
                _ => return Err(Error::NotImplemented),
            })
            .with_channel_type(if value.format.is_srgb() {
                ChannelType::SRGB
            } else {
                match value.format.format_data_type() {
                    FORMAT_TYPE::FORMAT_TYPE_FLOAT => ChannelType::FLOAT,
                    FORMAT_TYPE::FORMAT_TYPE_UNORM => ChannelType::UNORM,
                    FORMAT_TYPE::FORMAT_TYPE_SNORM => ChannelType::SNORM,
                    FORMAT_TYPE::FORMAT_TYPE_UINT => ChannelType::UINT,
                    FORMAT_TYPE::FORMAT_TYPE_SINT => ChannelType::SINT,
                    _ => return Err(Error::NotImplemented),
                }
            })
            .with_width((value.width - 1).try_into()?)
            .with_height((value.height - 1).try_into()?)
            .with_last_mip_level((value.mip_levels - 1).try_into()?)
            .with_texture_type(if value.is_cubemap() {
                TextureType::CUBEMAP
            } else {
                match value.dimension {
                    TEX_DIMENSION::TEX_DIMENSION_TEXTURE1D => TextureType::TYPE_1D,
                    TEX_DIMENSION::TEX_DIMENSION_TEXTURE2D => TextureType::TYPE_2D,
                    TEX_DIMENSION::TEX_DIMENSION_TEXTURE3D => TextureType::TYPE_3D,
                    _ => return Err(Error::NotImplemented),
                }
            })
            .with_depth((value.depth - 1).try_into()?)
            .with_last_array_slice_index((value.array_size - 1).try_into()?);
        Ok(this)
    }
}

impl TryFrom<&GNMF> for TexMetadata {
    type Error = Error;

    fn try_from(value: &GNMF) -> Result<Self> {
        let texture_type = value.texture_type();
        Ok(Self {
            width: value.width() as usize + 1,
            height: value.height() as usize + 1,
            depth: value.depth() as usize + 1,
            array_size: (value.last_array_slice_index() - value.base_array_slice_index()) as usize
                + 1,
            mip_levels: (value.last_mip_level() - value.base_mip_level()) as usize + 1,
            misc_flags: if texture_type == TextureType::CUBEMAP {
                TEX_MISC_TEXTURECUBE
            } else {
                0
            },
            misc_flags2: 0,
            format: match (value.surface_format(), value.channel_type()) {
                (SurfaceFormat::FORMAT_8_8_8_8, ChannelType::UNORM) => {
                    DXGI_FORMAT::DXGI_FORMAT_R8G8B8A8_UNORM
                }
                (SurfaceFormat::FORMAT_8_8_8_8, ChannelType::SRGB) => {
                    DXGI_FORMAT::DXGI_FORMAT_R8G8B8A8_UNORM_SRGB
                }
                (SurfaceFormat::FORMAT_8_8_8_8, ChannelType::UINT) => {
                    DXGI_FORMAT::DXGI_FORMAT_R8G8B8A8_UINT
                }
                (SurfaceFormat::FORMAT_8_8_8_8, ChannelType::SNORM) => {
                    DXGI_FORMAT::DXGI_FORMAT_R8G8B8A8_SNORM
                }
                (SurfaceFormat::FORMAT_8_8_8_8, ChannelType::SINT) => {
                    DXGI_FORMAT::DXGI_FORMAT_R8G8B8A8_SINT
                }

                (SurfaceFormat::BC1, ChannelType::UNORM) => DXGI_FORMAT::DXGI_FORMAT_BC1_UNORM,
                (SurfaceFormat::BC1, ChannelType::SRGB) => DXGI_FORMAT::DXGI_FORMAT_BC1_UNORM_SRGB,

                (SurfaceFormat::BC2, ChannelType::UNORM) => DXGI_FORMAT::DXGI_FORMAT_BC2_UNORM,
                (SurfaceFormat::BC2, ChannelType::SRGB) => DXGI_FORMAT::DXGI_FORMAT_BC2_UNORM_SRGB,

                (SurfaceFormat::BC3, ChannelType::UNORM) => DXGI_FORMAT::DXGI_FORMAT_BC3_UNORM,
                (SurfaceFormat::BC3, ChannelType::SRGB) => DXGI_FORMAT::DXGI_FORMAT_BC3_UNORM_SRGB,

                (SurfaceFormat::BC4, ChannelType::UNORM) => DXGI_FORMAT::DXGI_FORMAT_BC4_UNORM,
                (SurfaceFormat::BC4, ChannelType::SNORM) => DXGI_FORMAT::DXGI_FORMAT_BC4_SNORM,

                (SurfaceFormat::BC5, ChannelType::UNORM) => DXGI_FORMAT::DXGI_FORMAT_BC5_UNORM,
                (SurfaceFormat::BC5, ChannelType::SNORM) => DXGI_FORMAT::DXGI_FORMAT_BC5_SNORM,

                (SurfaceFormat::BC6, _) => DXGI_FORMAT::DXGI_FORMAT_BC6H_UF16,

                (SurfaceFormat::BC7, ChannelType::UNORM) => DXGI_FORMAT::DXGI_FORMAT_BC7_UNORM,
                (SurfaceFormat::BC7, ChannelType::SRGB) => DXGI_FORMAT::DXGI_FORMAT_BC7_UNORM_SRGB,

                _ => return Err(Error::NotImplemented),
            },
            dimension: match texture_type {
                TextureType::TYPE_1D => TEX_DIMENSION::TEX_DIMENSION_TEXTURE1D,
                TextureType::TYPE_2D | TextureType::CUBEMAP => {
                    TEX_DIMENSION::TEX_DIMENSION_TEXTURE2D
                }
                TextureType::TYPE_3D => TEX_DIMENSION::TEX_DIMENSION_TEXTURE3D,
                _ => return Err(Error::NotImplemented),
            },
        })
    }
}

struct ChannelType;

// https://github.com/tge-was-taken/GFD-Studio/blob/dad6c2183a6ec0716c3943b71991733bfbd4649d/GFDLibrary/Textures/GNF/ChannelType.cs#L3
#[allow(unused)]
impl ChannelType {
    const UNORM: u32 = 0x0;
    const SNORM: u32 = 0x1;
    const USCALED: u32 = 0x2;
    const SSCALED: u32 = 0x3;
    const UINT: u32 = 0x4;
    const SINT: u32 = 0x5;
    const SNORM_NO_ZERO: u32 = 0x6;
    const FLOAT: u32 = 0x7;
    const SRGB: u32 = 0x9;
    const UBNORM: u32 = 0xA;
    const UBNORM_NO_ZERO: u32 = 0xB;
    const UBINT: u32 = 0xC;
    const UBSCALED: u32 = 0xD;
}

struct SamplerModulationFactor;

// https://github.com/tge-was-taken/GFD-Studio/blob/dad6c2183a6ec0716c3943b71991733bfbd4649d/GFDLibrary/Textures/GNF/SamplerModulationFactor.cs#L3
#[allow(unused)]
impl SamplerModulationFactor {
    const FACTOR_0_0000: u32 = 0x0;
    const FACTOR_0_1250: u32 = 0x1;
    const FACTOR_0_3125: u32 = 0x2;
    const FACTOR_0_4375: u32 = 0x3;
    const FACTOR_0_5625: u32 = 0x4;
    const FACTOR_0_6875: u32 = 0x5;
    const FACTOR_0_8750: u32 = 0x6;
    const FACTOR_1_0000: u32 = 0x7;
}

struct SurfaceFormat;

// https://github.com/tge-was-taken/GFD-Studio/blob/dad6c2183a6ec0716c3943b71991733bfbd4649d/GFDLibrary/Textures/GNF/SurfaceFormat.cs#L3
#[allow(unused)]
impl SurfaceFormat {
    const INVALID: u32 = 0x0;
    const FORMAT_8: u32 = 0x1;
    const FORMAT_16: u32 = 0x2;
    const FORMAT_8_8: u32 = 0x3;
    const FORMAT_32: u32 = 0x4;
    const FORMAT_16_16: u32 = 0x5;
    const FORMAT_10_11_11: u32 = 0x6;
    const FORMAT_11_11_10: u32 = 0x7;
    const FORMAT_10_10_10_2: u32 = 0x8;
    const FORMAT_2_10_10_10: u32 = 0x9;
    const FORMAT_8_8_8_8: u32 = 0xA;
    const FORMAT_32_32: u32 = 0xB;
    const FORMAT_16_16_16_16: u32 = 0xC;
    const FORMAT_32_32_32: u32 = 0xD;
    const FORMAT_32_32_32_32: u32 = 0xE;
    const FORMAT_5_6_5: u32 = 0x10;
    const FORMAT_1_5_5_5: u32 = 0x11;
    const FORMAT_5_5_5_1: u32 = 0x12;
    const FORMAT_4_4_4_4: u32 = 0x13;
    const FORMAT_8_24: u32 = 0x14;
    const FORMAT_24_8: u32 = 0x15;
    const FORMAT_X24_8_32: u32 = 0x16;
    const GB_GR: u32 = 0x20;
    const BG_RG: u32 = 0x21;
    const FORMAT_5_9_9_9: u32 = 0x22;
    const BC1: u32 = 0x23;
    const BC2: u32 = 0x24;
    const BC3: u32 = 0x25;
    const BC4: u32 = 0x26;
    const BC5: u32 = 0x27;
    const BC6: u32 = 0x28;
    const BC7: u32 = 0x29;
    const FMASK_8_S2_F1: u32 = 0x2C;
    const FMASK_8_S4_F1: u32 = 0x2D;
    const FMASK_8_S8_F1: u32 = 0x2E;
    const FMASK_8_S2_F2: u32 = 0x2F;
    const FMASK_8_S4_F2: u32 = 0x30;
    const FMASK_8_S4_F4: u32 = 0x31;
    const FMASK_16_S16_F1: u32 = 0x32;
    const FMASK_16_S8_F2: u32 = 0x33;
    const FMASK_32_S16_F2: u32 = 0x34;
    const FMASK_32_S8_F4: u32 = 0x35;
    const FMASK_32_S8_F8: u32 = 0x36;
    const FMASK_64_S16_F4: u32 = 0x37;
    const FMASK_64_S16_F8: u32 = 0x38;
    const FORMAT_4_4: u32 = 0x39;
    const FORMAT_6_5_5: u32 = 0x3A;
    const FORMAT_1: u32 = 0x3B;
    const FORMAT_1_REVERSED: u32 = 0x3C;
}

struct TextureChannel;

// https://github.com/tge-was-taken/GFD-Studio/blob/dad6c2183a6ec0716c3943b71991733bfbd4649d/GFDLibrary/Textures/GNF/TextureChannel.cs#L3
#[allow(unused)]
impl TextureChannel {
    const CONSTANT_0: u32 = 0x0;
    const CONSTANT_1: u32 = 0x1;
    const X: u32 = 0x4;
    const Y: u32 = 0x5;
    const Z: u32 = 0x6;
    const W: u32 = 0x7;
}

struct TextureType;

// https://github.com/tge-was-taken/GFD-Studio/blob/dad6c2183a6ec0716c3943b71991733bfbd4649d/GFDLibrary/Textures/GNF/TextureType.cs#L3
#[allow(unused)]
impl TextureType {
    const TYPE_1D: u32 = 0x8;
    const TYPE_2D: u32 = 0x9;
    const TYPE_3D: u32 = 0xA;
    const CUBEMAP: u32 = 0xB;
    const TYPE_1D_ARRAY: u32 = 0xC;
    const TYPE_2D_ARRAY: u32 = 0xD;
    const TYPE_2D_MSAA: u32 = 0xE;
    const TYPE_2D_ARRAY_MSAA: u32 = 0xF;
}

struct TileMode;

// https://github.com/tge-was-taken/GFD-Studio/blob/dad6c2183a6ec0716c3943b71991733bfbd4649d/GFDLibrary/Textures/GNF/TileMode.cs#L6
#[allow(unused)]
impl TileMode {
    const DEPTH_2D_THIN_64: u32 = 0x0;
    const DEPTH_2D_THIN_128: u32 = 0x1;
    const DEPTH_2D_THIN_256: u32 = 0x2;
    const DEPTH_2D_THIN_512: u32 = 0x3;
    const DEPTH_2D_THIN_1K: u32 = 0x4;
    const DEPTH_1D_THIN: u32 = 0x5;
    const DEPTH_2D_THIN_PRT_256: u32 = 0x6;
    const DEPTH_2D_THIN_PRT_1K: u32 = 0x7;
    const DISPLAY_LINEAR_ALIGNED: u32 = 0x8;
    const DISPLAY_1D_THIN: u32 = 0x9;
    const DISPLAY_2D_THIN: u32 = 0xA;
    const DISPLAY_THIN_PRT: u32 = 0xB;
    const DISPLAY_2D_THIN_PRT: u32 = 0xC;
    const THIN_1D_THIN: u32 = 0xD;
    const THIN_2D_THIN: u32 = 0xE;
    const THIN_3D_THIN: u32 = 0xF;
    const THIN_THIN_PRT: u32 = 0x10;
    const THIN_2D_THIN_PRT: u32 = 0x11;
    const THIN_3D_THIN_PRT: u32 = 0x12;
    const THIN_1D_THICK: u32 = 0x13;
    const THIN_2D_THICK: u32 = 0x14;
    const THIN_3D_THICK: u32 = 0x15;
    const THIN_THICK_PRT: u32 = 0x16;
    const THIN_2D_THICK_PRT: u32 = 0x17;
    const THIN_3D_THICK_PRT: u32 = 0x18;
    const THIN_2DX_THICK: u32 = 0x19;
    const THIN_3DX_THICK: u32 = 0x1A;
    const DISPLAY_LINEAR_GENERAL: u32 = 0x1F;
}

/// Optionally present file header.
///
/// The header variant must match the archive [`Format`] when writing.
#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum Header {
    #[default]
    GNRL,
    DX10(DX10),
    GNMF(GNMF),
}

impl From<DX10> for Header {
    fn from(value: DX10) -> Self {
        Self::DX10(value)
    }
}

impl From<GNMF> for Header {
    fn from(value: GNMF) -> Self {
        Self::GNMF(value)
    }
}

mod swizzle {
    // https://github.com/tge-was-taken/GFD-Studio/blob/dad6c2183a6ec0716c3943b71991733bfbd4649d/GFDLibrary/Textures/Swizzle/SwizzleUtilities.cs#L9
    fn morton(t: usize, sx: usize, sy: usize) -> usize {
        let mut num1 = 1;
        let mut num2 = 1;
        let mut num3 = t;
        let mut num4 = sx;
        let mut num5 = sy;
        let mut num6 = 0;
        let mut num7 = 0;

        while num4 > 1 || num5 > 1 {
            if num4 > 1 {
                num6 += num2 * (num3 & 1);
                num3 >>= 1;
                num2 *= 2;
                num4 >>= 1;
            }
            if num5 > 1 {
                num7 += num1 * (num3 & 1);
                num3 >>= 1;
                num1 *= 2;
                num5 >>= 1;
            }
        }

        num7 * sx + num6
    }

    pub(crate) mod ps4 {
        // https://github.com/tge-was-taken/GFD-Studio/blob/dad6c2183a6ec0716c3943b71991733bfbd4649d/GFDLibrary/Textures/Swizzle/PS4SwizzleAlgorithm.cs#L20
        fn do_swizzle(
            source: &[u8],
            destination: &mut Vec<u8>,
            width: usize,
            height: usize,
            block_size: usize,
            unswizzle: bool,
        ) {
            destination.clear();
            destination.resize_with(source.len(), Default::default);
            let height_texels = height / 4;
            let height_texels_aligned = (height_texels + 7) / 8;
            let width_texels = width / 4;
            let width_texels_aligned = (width_texels + 7) / 8;
            let mut data_index = 0;

            for y in 0..height_texels_aligned {
                for x in 0..width_texels_aligned {
                    for t in 0..64 {
                        let pixel_index = super::morton(t, 8, 8);
                        let div = pixel_index / 8;
                        let rem = pixel_index % 8;
                        let y_offset = (y * 8) + div;
                        let x_offset = (x * 8) + rem;

                        if x_offset < width_texels && y_offset < height_texels {
                            let dest_pixel_index = y_offset * width_texels + x_offset;
                            let dest_index = block_size * dest_pixel_index;
                            let (src, dst) = if unswizzle {
                                (data_index, dest_index)
                            } else {
                                (dest_index, data_index)
                            };
                            destination[dst..dst + block_size]
                                .copy_from_slice(&source[src..src + block_size]);
                        }

                        data_index += block_size;
                    }
                }
            }
        }

        pub(crate) fn swizzle(
            source: &[u8],
            destination: &mut Vec<u8>,
            width: usize,
            height: usize,
            block_size: usize,
        ) {
            do_swizzle(source, destination, width, height, block_size, false);
        }

        pub(crate) fn unswizzle(
            source: &[u8],
            destination: &mut Vec<u8>,
            width: usize,
            height: usize,
            block_size: usize,
        ) {
            do_swizzle(source, destination, width, height, block_size, true);
        }
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
        match &self.header {
            Header::GNRL => self.write_gnrl(stream, *options),
            Header::DX10(x) => self.write_dx10(stream, *options, *x),
            Header::GNMF(_) => Err(Error::NotImplemented), //self.write_gnmf(stream, *options, x)?,
        }
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
            Format::GNMF => Err(Error::NotImplemented), // Self::read_gnmf(stream, options),
        }?;

        if options.compression_result == CompressionResult::Compressed {
            for chunk in &mut this {
                *chunk = chunk.compress(&options.compression_options)?;
            }
        }

        Ok(this)
    }

    fn make_chunks(scratch: &ScratchImage, options: &ReadOptions) -> Result<Vec<Chunk<'bytes>>> {
        let metadata = scratch.metadata();
        let images = scratch.images();

        let chunk_from_mips = |range: Range<usize>| -> Result<Chunk> {
            let try_clamp = |num: usize| -> Result<u16> {
                let result = usize::min(metadata.mip_levels.saturating_sub(1), num).try_into()?;
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
                mips: Some(mips),
            })
        };

        let chunks = if let Some(images_len) = NonZeroUsize::new(images.len()) {
            if metadata.is_cubemap() {
                // don't chunk cubemaps
                let chunk = chunk_from_mips(0..images_len.get())?;
                [chunk].into_iter().collect()
            } else {
                let pitch = metadata.format.compute_pitch(
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

                    if v.len() == 3 {
                        break;
                    }

                    stop += 1;
                    if stop == images_len.get() {
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

        Ok(chunks)
    }

    fn read_dx10<In>(stream: &In, options: &ReadOptions) -> Result<Self>
    where
        In: ?Sized + Source<'bytes>,
    {
        let scratch =
            ScratchImage::load_dds(stream.as_bytes(), DDS_FLAGS::DDS_FLAGS_NONE, None, None)?;
        let meta = scratch.metadata();
        let header: Header = DX10 {
            height: meta.height.try_into()?,
            width: meta.width.try_into()?,
            mip_count: meta.mip_levels.try_into()?,
            format: meta.format.bits().try_into()?,
            flags: meta.is_cubemap().into(),
            tile_mode: 8,
        }
        .into();

        let chunks = Self::make_chunks(&scratch, options)?;
        Ok(Self { chunks, header })
    }

    #[allow(unused)]
    fn read_gnmf<In>(stream: &mut In, options: &ReadOptions) -> Result<Self>
    where
        In: ?Sized + Source<'bytes>,
    {
        let scratch =
            ScratchImage::load_dds(stream.as_bytes(), DDS_FLAGS::DDS_FLAGS_NONE, None, None)?;
        let metadata = scratch.metadata();
        let gnmf = {
            let mut gnmf: GNMF = metadata.try_into()?;
            let len: usize = scratch.images().iter().map(|x| x.slice_pitch).sum();
            gnmf.metadata[7] = len.try_into()?;
            gnmf
        };

        let mut chunks = Self::make_chunks(&scratch, options)?;
        let mut scratch_buffer = Vec::new();
        let mut width = metadata.width;
        let mut height = metadata.height;
        let block_size = gnmf.block_size()?;
        for chunk in &mut chunks {
            let mips = chunk.mips.as_ref().expect("GNMF chunks should have mips");
            let mut unswizzled_bytes = Vec::new();
            let mut offset = 0;
            for _ in mips.clone() {
                let pitch =
                    metadata
                        .format
                        .compute_pitch(width, height, CP_FLAGS::CP_FLAGS_NONE)?;
                swizzle::ps4::swizzle(
                    &chunk.as_bytes()[offset..offset + pitch.slice],
                    &mut scratch_buffer,
                    width,
                    height,
                    block_size,
                );
                unswizzled_bytes.extend_from_slice(&scratch_buffer);
                offset += pitch.slice;
                width = usize::max(1, width / 2);
                height = usize::max(1, height / 2);
            }
            chunk.bytes = CompressableBytes::from_owned(unswizzled_bytes.into_boxed_slice(), None);
        }

        Ok(Self {
            chunks,
            header: gnmf.into(),
        })
    }

    #[allow(clippy::unnecessary_wraps)]
    fn read_gnrl<In>(stream: &mut In) -> Result<Self>
    where
        In: ?Sized + Source<'bytes>,
    {
        let bytes = stream.read_bytes_to_end().into_compressable(None);
        let chunk = Chunk { bytes, mips: None };
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
                TEX_MISC_TEXTURECUBE
            },
            misc_flags2: 0,
            format: u32::from(dx10.format).into(),
            dimension: TEX_DIMENSION::TEX_DIMENSION_TEXTURE2D,
        };

        let header = meta.encode_dds_header(DDS_FLAGS::DDS_FLAGS_NONE)?;
        stream.write_all(&header)?;
        self.write_gnrl(stream, options)
    }

    #[allow(unused)]
    fn write_gnmf<Out>(&self, stream: &mut Out, options: WriteOptions, gnmf: &GNMF) -> Result<()>
    where
        Out: ?Sized + Write,
    {
        let metadata: TexMetadata = gnmf.try_into()?;
        let header = metadata.encode_dds_header(DDS_FLAGS::DDS_FLAGS_NONE)?;
        stream.write_all(&header)?;

        let mut bytes_buffer = Vec::new();
        let options: ChunkCompressionOptions = options.into();
        let mut unswizzled_bytes = Vec::new();
        let mut width = metadata.width;
        let mut height = metadata.height;
        let block_size = gnmf.block_size()?;
        for chunk in self {
            let mut offset = 0;
            let Some(mips) = chunk.mips.as_ref() else {
                return Err(Error::FormatMismatch);
            };
            let swizzled_bytes = if chunk.is_compressed() {
                bytes_buffer.clear();
                chunk.decompress_into(&mut bytes_buffer, &options)?;
                &bytes_buffer
            } else {
                chunk.as_bytes()
            };
            for _ in mips.clone() {
                let pitch =
                    metadata
                        .format
                        .compute_pitch(width, height, CP_FLAGS::CP_FLAGS_NONE)?;
                swizzle::ps4::unswizzle(
                    &swizzled_bytes[offset..offset + pitch.slice],
                    &mut unswizzled_bytes,
                    width,
                    height,
                    block_size,
                );
                stream.write_all(&unswizzled_bytes)?;
                offset += pitch.slice;
                width = usize::max(1, width / 2);
                height = usize::max(1, height / 2);
            }
        }

        Ok(())
    }

    fn write_gnrl<Out>(&self, stream: &mut Out, options: WriteOptions) -> Result<()>
    where
        Out: ?Sized + Write,
    {
        let mut bytes_buffer = Vec::new();
        let options: ChunkCompressionOptions = options.into();

        for chunk in self {
            let bytes = if chunk.is_compressed() {
                bytes_buffer.clear();
                chunk.decompress_into(&mut bytes_buffer, &options)?;
                &bytes_buffer
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
