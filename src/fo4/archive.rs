use crate::{
    containers::Bytes,
    derive,
    fo4::{
        self, Chunk, ChunkDX10, ChunkExtra, CompressionFormat, Error, File, FileDX10, FileHash,
        FileHeader, Format, Hash, Result, Version,
    },
    io::{Endian, Sink, Source},
    protocols::WString,
};
use bstr::BString;
use std::io::Write;

mod constants {
    use crate::cc;

    pub(crate) const MAGIC: u32 = cc::make_four(b"BTDX");

    pub(crate) const GNRL: u32 = cc::make_four(b"GNRL");
    pub(crate) const DX10: u32 = cc::make_four(b"DX10");

    pub(crate) const HEADER_SIZE_V1: usize = 0x18;
    pub(crate) const HEADER_SIZE_V2: usize = 0x20;
    pub(crate) const HEADER_SIZE_V3: usize = 0x24;

    pub(crate) const FILE_HEADER_SIZE_GNRL: usize = 0x10;
    pub(crate) const FILE_HEADER_SIZE_DX10: usize = 0x18;

    pub(crate) const CHUNK_SIZE_GNRL: u16 = 0x14;
    pub(crate) const CHUNK_SIZE_DX10: u16 = 0x18;

    pub(crate) const CHUNK_SENTINEL: u32 = 0xBAAD_F00D;
}

struct Offsets {
    file_data: usize,
    strings: usize,
}

impl Offsets {
    #[must_use]
    pub fn new(archive: &Archive, options: Options) -> Self {
        let chunks_offset = match options.version {
            Version::v1 => constants::HEADER_SIZE_V1,
            Version::v2 => constants::HEADER_SIZE_V2,
            Version::v3 => constants::HEADER_SIZE_V3,
        };

        let file_data_offset = {
            let (file_header_size, chunk_size) = match options.format {
                Format::GNRL => (constants::FILE_HEADER_SIZE_GNRL, constants::CHUNK_SIZE_GNRL),
                Format::DX10 => (constants::FILE_HEADER_SIZE_DX10, constants::CHUNK_SIZE_DX10),
            };
            let chunks_count: usize = archive.values().map(File::len).sum();
            chunks_offset
                + (archive.len() * file_header_size)
                + (chunks_count * usize::from(chunk_size))
        };

        let strings_offset = {
            let data_size: usize = archive.values().flat_map(File::iter).map(Chunk::len).sum();
            file_data_offset + data_size
        };

        Self {
            file_data: file_data_offset,
            strings: strings_offset,
        }
    }
}

struct Header {
    version: Version,
    format: Format,
    file_count: u32,
    string_table_offset: u64,
    compression_format: CompressionFormat,
}

#[derive(Clone, Copy, Debug, Default)]
#[repr(transparent)]
pub struct OptionsBuilder(Options);

impl OptionsBuilder {
    #[must_use]
    pub fn build(self) -> Options {
        self.0
    }

    #[must_use]
    pub fn compression_format(mut self, compression_format: CompressionFormat) -> Self {
        self.0.compression_format = compression_format;
        self
    }

    #[must_use]
    pub fn format(mut self, format: Format) -> Self {
        self.0.format = format;
        self
    }

    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn strings(mut self, strings: bool) -> Self {
        self.0.strings = strings;
        self
    }

    #[must_use]
    pub fn version(mut self, version: Version) -> Self {
        self.0.version = version;
        self
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Options {
    format: Format,
    version: Version,
    compression_format: CompressionFormat,
    strings: bool,
}

impl Options {
    #[must_use]
    pub fn builder() -> OptionsBuilder {
        OptionsBuilder::new()
    }

    #[must_use]
    pub fn compression_format(&self) -> CompressionFormat {
        self.compression_format
    }

    #[must_use]
    pub fn format(&self) -> Format {
        self.format
    }

    #[must_use]
    pub fn strings(&self) -> bool {
        self.strings
    }

    #[must_use]
    pub fn version(&self) -> Version {
        self.version
    }
}

derive::key!(Key: FileHash);

impl Key {
    #[must_use]
    fn hash_in_place(name: &mut BString) -> FileHash {
        fo4::hash_file_in_place(name)
    }
}

type ReadResult<T> = (T, Options);
derive::archive!(Archive => ReadResult, Map: (Key: FileHash) => File);

impl<'bytes> Archive<'bytes> {
    pub fn write<Out>(&self, stream: &mut Out, options: &Options) -> Result<()>
    where
        Out: Write,
    {
        let mut sink = Sink::new(stream);
        let (header, mut offsets) = self.make_header(*options)?;
        Self::write_header(&mut sink, &header)?;

        for (key, file) in self {
            Self::write_file(&mut sink, &header, &mut offsets, &key.hash, file)?;
        }

        for file in self.values() {
            for chunk in file {
                sink.write_bytes(chunk.as_bytes())?;
            }
        }

        if options.strings {
            for key in self.keys() {
                sink.write_protocol::<WString>(key.name.as_ref(), Endian::Little)?;
            }
        }

        Ok(())
    }

    fn make_header(&self, options: Options) -> Result<(Header, Offsets)> {
        let offsets = Offsets::new(self, options);
        Ok((
            Header {
                version: options.version,
                format: options.format,
                file_count: self.len().try_into()?,
                string_table_offset: if options.strings {
                    offsets.strings as u64
                } else {
                    0
                },
                compression_format: options.compression_format,
            },
            offsets,
        ))
    }

    fn write_chunk<Out>(
        sink: &mut Sink<Out>,
        header: &Header,
        offsets: &mut Offsets,
        chunk: &Chunk<'bytes>,
    ) -> Result<()>
    where
        Out: Write,
    {
        let data_offset: u64 = offsets.file_data.try_into()?;
        offsets.file_data += chunk.len();
        let (compressed_size, decompressed_size): (u32, u32) =
            if let Some(decompressed_len) = chunk.decompressed_len() {
                (chunk.len().try_into()?, decompressed_len.try_into()?)
            } else {
                (0, chunk.len().try_into()?)
            };
        sink.write(
            &(data_offset, compressed_size, decompressed_size),
            Endian::Little,
        )?;

        match (header.format, &chunk.extra) {
            (Format::GNRL, ChunkExtra::GNRL) => (),
            (Format::DX10, ChunkExtra::DX10(x)) => {
                sink.write(&(x.mips.start, x.mips.end), Endian::Little)?;
            }
            _ => {
                return Err(Error::FormatMismatch);
            }
        }

        sink.write(&constants::CHUNK_SENTINEL, Endian::Little)?;
        Ok(())
    }

    fn write_file<Out>(
        sink: &mut Sink<Out>,
        header: &Header,
        offsets: &mut Offsets,
        hash: &FileHash,
        file: &File<'bytes>,
    ) -> Result<()>
    where
        Out: Write,
    {
        Self::write_hash(sink, hash)?;

        let chunk_count: u8 = file.len().try_into()?;
        let chunk_size = match header.format {
            Format::GNRL => constants::CHUNK_SIZE_GNRL,
            Format::DX10 => constants::CHUNK_SIZE_DX10,
        };
        sink.write(&(0u8, chunk_count, chunk_size), Endian::Little)?;

        match (header.format, &file.header) {
            (Format::GNRL, FileHeader::GNRL) => (),
            (Format::DX10, FileHeader::DX10(x)) => {
                sink.write(
                    &(
                        x.height,
                        x.width,
                        x.mip_count,
                        x.format,
                        x.flags,
                        x.tile_mode,
                    ),
                    Endian::Little,
                )?;
            }
            (_, _) => {
                return Err(Error::FormatMismatch);
            }
        }

        for chunk in file {
            Self::write_chunk(sink, header, offsets, chunk)?;
        }

        Ok(())
    }

    fn write_hash<Out>(sink: &mut Sink<Out>, hash: &Hash) -> Result<()>
    where
        Out: Write,
    {
        sink.write(&(hash.file, hash.extension, hash.directory), Endian::Little)?;
        Ok(())
    }

    fn write_header<Out>(sink: &mut Sink<Out>, header: &Header) -> Result<()>
    where
        Out: Write,
    {
        let format = match header.format {
            Format::GNRL => constants::GNRL,
            Format::DX10 => constants::DX10,
        };

        sink.write(
            &(
                constants::MAGIC,
                header.version as u32,
                format,
                header.file_count,
                header.string_table_offset,
            ),
            Endian::Little,
        )?;

        if header.version >= Version::v2 {
            sink.write(&1u64, Endian::Little)?;
        }

        if header.version >= Version::v3 {
            let format: u32 = match header.compression_format {
                CompressionFormat::Zip => 0,
                CompressionFormat::LZ4 => 3,
            };
            sink.write(&format, Endian::Little)?;
        }

        Ok(())
    }

    fn do_read<In>(source: &mut In) -> Result<ReadResult<Self>>
    where
        In: ?Sized + Source<'bytes>,
    {
        let header = Self::read_header(source)?;
        let mut map = Map::default();
        let mut strings: usize = header.string_table_offset.try_into()?;
        for _ in 0..header.file_count {
            let (key, value) = Self::read_file(source, &header, &mut strings)?;
            map.insert(key, value);
        }

        Ok((
            Self { map },
            Options {
                format: header.format,
                version: header.version,
                compression_format: header.compression_format,
                strings: header.string_table_offset != 0,
            },
        ))
    }

    fn read_chunk<In>(source: &mut In, header: &Header) -> Result<Chunk<'bytes>>
    where
        In: ?Sized + Source<'bytes>,
    {
        let (data_offset, compressed_size, decompressed_size): (u64, u32, u32) =
            source.read(Endian::Little)?;
        let extra = match header.format {
            Format::GNRL => ChunkExtra::GNRL,
            Format::DX10 => {
                let (mip_first, mip_last) = source.read(Endian::Little)?;
                ChunkDX10 {
                    mips: mip_first..mip_last,
                }
                .into()
            }
        };

        let sentinel = source.read(Endian::Little)?;
        if sentinel != constants::CHUNK_SENTINEL {
            return Err(Error::InvalidChunkSentinel(sentinel));
        }

        let bytes = source.save_restore_position(|source| -> Result<Bytes<'bytes>> {
            source.seek_absolute(data_offset.try_into()?)?;
            let len = if compressed_size == 0 {
                decompressed_size
            } else {
                compressed_size
            };
            let bytes = source.read_bytes(len as usize)?;
            Ok(bytes)
        })??;
        let decompressed_len = (compressed_size != 0).then_some(decompressed_size as usize);
        let bytes = bytes.into_compressable(decompressed_len);

        Ok(Chunk { bytes, extra })
    }

    fn read_file<In>(
        source: &mut In,
        header: &Header,
        strings: &mut usize,
    ) -> Result<(Key, File<'bytes>)>
    where
        In: ?Sized + Source<'bytes>,
    {
        let name = if *strings == 0 {
            BString::default()
        } else {
            source.save_restore_position(|source| -> Result<BString> {
                source.seek_absolute(*strings)?;
                let name = source.read_protocol::<WString>(Endian::Little)?;
                *strings = source.stream_position();
                Ok(name)
            })??
        };

        let hash = Self::read_hash(source)?;
        let (_, chunk_count, chunk_size): (u8, u8, u16) = source.read(Endian::Little)?;
        if !matches!(
            (header.format, chunk_size),
            (Format::GNRL, constants::CHUNK_SIZE_GNRL) | (Format::DX10, constants::CHUNK_SIZE_DX10)
        ) {
            return Err(Error::InvalidChunkSize(chunk_size));
        }

        let file_header = match header.format {
            Format::GNRL => FileHeader::GNRL,
            Format::DX10 => {
                let (height, width, mip_count, format, flags, tile_mode) =
                    source.read(Endian::Little)?;
                FileDX10 {
                    height,
                    width,
                    mip_count,
                    format,
                    flags,
                    tile_mode,
                }
                .into()
            }
        };

        let mut chunks = Vec::with_capacity(chunk_count.into());
        for _ in 0..chunk_count {
            let chunk = Self::read_chunk(source, header)?;
            chunks.push(chunk);
        }

        Ok((
            Key {
                hash: hash.into(),
                name,
            },
            File {
                chunks,
                header: file_header,
            },
        ))
    }

    fn read_hash<In>(source: &mut In) -> Result<Hash>
    where
        In: ?Sized + Source<'bytes>,
    {
        let (file, extension, directory) = source.read(Endian::Little)?;
        Ok(Hash {
            file,
            extension,
            directory,
        })
    }

    fn read_header<In>(source: &mut In) -> Result<Header>
    where
        In: ?Sized + Source<'bytes>,
    {
        let (magic, version, contents_format, file_count, string_table_offset) =
            source.read(Endian::Little)?;

        if magic != constants::MAGIC {
            return Err(Error::InvalidMagic(magic));
        }

        let format = match contents_format {
            constants::GNRL => Format::GNRL,
            constants::DX10 => Format::DX10,
            _ => return Err(Error::InvalidFormat(contents_format)),
        };

        let version = match version {
            1 => Version::v1,
            2 => Version::v2,
            3 => Version::v3,
            _ => return Err(Error::InvalidVersion(version)),
        };

        if version >= Version::v2 {
            source.read::<u64>(Endian::Little)?;
        }

        let compression_format = if version >= Version::v3 {
            let format: u32 = source.read(Endian::Little)?;
            if format == 3 {
                CompressionFormat::LZ4
            } else {
                CompressionFormat::Zip
            }
        } else {
            CompressionFormat::Zip
        };

        Ok(Header {
            version,
            format,
            file_count,
            string_table_offset,
            compression_format,
        })
    }
}
