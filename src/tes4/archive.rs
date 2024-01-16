use crate::{
    containers::CompressableBytes,
    derive,
    io::{Endian, Sink, Source},
    protocols::{self, BZString, ZString},
    tes4::{
        self, directory::Map as DirectoryMap, Directory, DirectoryHash, DirectoryKey, Error, File,
        Hash, Result, Version,
    },
};
use bstr::{BStr, BString, ByteSlice as _};
use core::mem;
use std::{borrow::Cow, io::Write};

bitflags::bitflags! {
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct Flags: u32 {
        const DIRECTORY_STRINGS = 1 << 0;
        const FILE_STRINGS = 1 << 1;
        const COMPRESSED = 1 << 2;
        const RETAIN_DIRECTORY_NAMES = 1 << 3;
        const RETAIN_FILE_NAMES = 1 << 4;
        const RETAIN_FILE_NAME_OFFSETS = 1 << 5;
        const XBOX_ARCHIVE = 1 << 6;
        const RETAIN_STRINGS_DURING_STARTUP = 1 << 7;
        const EMBEDDED_FILE_NAMES = 1 << 8;
        const XBOX_COMPRESSED = 1 << 9;
    }
}

impl Default for Flags {
    fn default() -> Self {
        Self::DIRECTORY_STRINGS | Self::FILE_STRINGS
    }
}

impl Flags {
    #[must_use]
    pub fn directory_strings(&self) -> bool {
        self.contains(Self::DIRECTORY_STRINGS)
    }

    #[must_use]
    pub fn file_strings(&self) -> bool {
        self.contains(Self::FILE_STRINGS)
    }

    #[must_use]
    pub fn compressed(&self) -> bool {
        self.contains(Self::COMPRESSED)
    }

    #[must_use]
    pub fn retain_directory_names(&self) -> bool {
        self.contains(Self::RETAIN_DIRECTORY_NAMES)
    }

    #[must_use]
    pub fn retain_file_names(&self) -> bool {
        self.contains(Self::RETAIN_FILE_NAMES)
    }

    #[must_use]
    pub fn retain_file_name_offsets(&self) -> bool {
        self.contains(Self::RETAIN_FILE_NAME_OFFSETS)
    }

    #[must_use]
    pub fn xbox_archive(&self) -> bool {
        self.contains(Self::XBOX_ARCHIVE)
    }

    #[must_use]
    pub fn retain_strings_during_startup(&self) -> bool {
        self.contains(Self::RETAIN_STRINGS_DURING_STARTUP)
    }

    #[must_use]
    pub fn embedded_file_names(&self) -> bool {
        self.contains(Self::EMBEDDED_FILE_NAMES)
    }

    #[must_use]
    pub fn xbox_compressed(&self) -> bool {
        self.contains(Self::XBOX_COMPRESSED)
    }
}

bitflags::bitflags! {
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
    pub struct Types: u16 {
        const MESHES = 1 << 0;
        const TEXTURES = 1 << 1;
        const MENUS = 1 << 2;
        const SOUNDS = 1 << 3;
        const VOICES = 1 << 4;
        const SHADERS = 1 << 5;
        const TREES = 1 << 6;
        const FONTS = 1 << 7;
        const MISC = 1 << 8;
    }
}

impl Types {
    #[must_use]
    pub fn meshes(&self) -> bool {
        self.contains(Self::MESHES)
    }

    #[must_use]
    pub fn textures(&self) -> bool {
        self.contains(Self::TEXTURES)
    }

    #[must_use]
    pub fn menus(&self) -> bool {
        self.contains(Self::MENUS)
    }

    #[must_use]
    pub fn sounds(&self) -> bool {
        self.contains(Self::SOUNDS)
    }

    #[must_use]
    pub fn voices(&self) -> bool {
        self.contains(Self::VOICES)
    }

    #[must_use]
    pub fn shaders(&self) -> bool {
        self.contains(Self::SHADERS)
    }

    #[must_use]
    pub fn trees(&self) -> bool {
        self.contains(Self::TREES)
    }

    #[must_use]
    pub fn fonts(&self) -> bool {
        self.contains(Self::FONTS)
    }

    #[must_use]
    pub fn misc(&self) -> bool {
        self.contains(Self::MISC)
    }
}

mod constants {
    use crate::cc;

    pub(crate) const BSA: u32 = cc::make_four(b"BSA");

    pub(crate) const HEADER_SIZE: u32 = 0x24;
    pub(crate) const DIRECTORY_ENTRY_SIZE_X86: usize = 0x10;
    pub(crate) const DIRECTORY_ENTRY_SIZE_X64: usize = 0x18;
    pub(crate) const FILE_ENTRY_SIZE: usize = 0x10;

    pub(crate) const FILE_FLAG_COMPRESSION: u32 = 1 << 30;
    pub(crate) const FILE_FLAG_CHECKED: u32 = 1 << 31;
    pub(crate) const FILE_FLAG_SECONDARY_ARCHIVE: u32 = 1 << 31;
}

struct Offsets {
    file_entries: usize,
    file_names: usize,
    file_data: usize,
}

struct Header {
    version: Version,
    archive_flags: Flags,
    directory_count: u32,
    file_count: u32,
    directory_names_len: u32,
    file_names_len: u32,
    archive_types: Types,
}

impl Header {
    #[must_use]
    fn hash_endian(&self) -> Endian {
        if self.archive_flags.xbox_archive() {
            Endian::Big
        } else {
            Endian::Little
        }
    }

    #[must_use]
    fn compute_offsets(&self) -> Offsets {
        let file_entries = {
            let directory_entries = constants::HEADER_SIZE as usize;
            let directory_entry_size = match self.version {
                Version::TES4 | Version::FO3 => constants::DIRECTORY_ENTRY_SIZE_X86,
                Version::SSE => constants::DIRECTORY_ENTRY_SIZE_X64,
            };
            directory_entries + (directory_entry_size * self.directory_count as usize)
        };

        let file_names = {
            let directory_names_len = if self.archive_flags.directory_strings() {
                // directory names are stored using a bzstring
                // directory_names_len includes the length of the string + the null terminator,
                // but not the prefix length byte, so we add directory_count to include it
                self.directory_names_len as usize + self.directory_count as usize
            } else {
                0
            };
            file_entries
                + (directory_names_len + constants::FILE_ENTRY_SIZE * self.file_count as usize)
        };

        let file_data = if self.archive_flags.file_strings() {
            file_names + self.file_names_len as usize
        } else {
            file_names
        };

        Offsets {
            file_entries,
            file_names,
            file_data,
        }
    }
}

struct SortedFile<'this, 'bytes> {
    key: &'this DirectoryKey,
    this: &'this File<'bytes>,
    embedded_name: Option<Cow<'this, BStr>>,
}

struct SortedDirectory<'this, 'bytes> {
    key: &'this Key,
    this: &'this Directory<'bytes>,
    files: Vec<SortedFile<'this, 'bytes>>,
}

derive::key!(Key: DirectoryHash);

impl Key {
    #[must_use]
    fn hash_in_place(name: &mut BString) -> DirectoryHash {
        tes4::hash_directory_in_place(name)
    }
}

#[repr(transparent)]
pub struct OptionsBuilder(Options);

impl OptionsBuilder {
    #[must_use]
    pub fn build(self) -> Options {
        self.0
    }

    #[must_use]
    pub fn flags(mut self, flags: Flags) -> Self {
        self.0.flags = flags;
        self
    }

    #[must_use]
    pub fn types(mut self, types: Types) -> Self {
        self.0.types = types;
        self
    }

    #[must_use]
    pub fn version(mut self, version: Version) -> Self {
        self.0.version = version;
        self
    }

    #[must_use]
    fn new() -> Self {
        Self(Options {
            version: Version::default(),
            flags: Flags::default(),
            types: Types::default(),
        })
    }
}

#[derive(Clone, Copy)]
pub struct Options {
    version: Version,
    flags: Flags,
    types: Types,
}

impl Options {
    #[must_use]
    pub fn builder() -> OptionsBuilder {
        OptionsBuilder::new()
    }

    #[must_use]
    pub fn flags(&self) -> Flags {
        self.flags
    }

    #[must_use]
    pub fn types(&self) -> Types {
        self.types
    }

    #[must_use]
    pub fn version(&self) -> Version {
        self.version
    }
}

type ReadResult<T> = (T, Options);
derive::archive!(Archive => ReadResult, Map: (Key: DirectoryHash) => Directory);

impl<'bytes> Archive<'bytes> {
    pub fn write<Out>(&self, stream: &mut Out, options: &Options) -> Result<()>
    where
        Out: Write,
    {
        let mut sink = Sink::new(stream);
        let header = self.make_header(*options)?;
        Self::write_header(&mut sink, &header)?;

        let offsets = header.compute_offsets();
        let directories = self.sort_for_write(*options);

        // let mut file_entries_offset = offsets.file_entries + header.file_names_len;
        let mut file_entries_offset = u32::try_from(offsets.file_entries)?
            .checked_add(header.file_names_len)
            .ok_or(Error::IntegralOverflow)?;
        for directory in &directories {
            Self::write_directory_entry(
                &mut sink,
                *options,
                directory.key,
                directory.this,
                &mut file_entries_offset,
            )?;
        }

        let mut file_data_offset = u32::try_from(offsets.file_data)?;
        for directory in &directories {
            if options.flags.directory_strings() {
                sink.write_protocol::<BZString>(directory.key.name.as_ref(), Endian::Little)?;
            }
            for file in &directory.files {
                Self::write_file_entry(
                    &mut sink,
                    *options,
                    file.key,
                    file.this,
                    &mut file_data_offset,
                    file.embedded_name.as_ref().map(AsRef::as_ref),
                )?;
            }
        }

        if options.flags.file_strings() {
            for directory in &directories {
                for file in &directory.files {
                    sink.write_protocol::<ZString>(file.key.name.as_ref(), Endian::Little)?;
                }
            }
        }

        for directory in &directories {
            for file in &directory.files {
                Self::write_file_data(
                    &mut sink,
                    file.this,
                    file.embedded_name.as_ref().map(AsRef::as_ref),
                )?;
            }
        }

        Ok(())
    }

    fn make_header(&self, options: Options) -> Result<Header> {
        #[derive(Default)]
        struct Info {
            count: usize,
            names_len: usize,
        }

        let mut files = Info::default();
        let mut directories = Info::default();

        for directory in self {
            directories.count += 1;
            if options.flags.directory_strings() {
                // zstring -> include null terminator
                directories.names_len += directory.0.name.len() + 1;
            }

            for file in directory.1 {
                files.count += 1;
                if options.flags.file_strings() {
                    // zstring -> include null terminator
                    files.names_len += file.0.name.len() + 1;
                }
            }
        }

        Ok(Header {
            version: options.version,
            archive_flags: options.flags,
            directory_count: directories.count.try_into()?,
            file_count: files.count.try_into()?,
            directory_names_len: directories.names_len.try_into()?,
            file_names_len: files.names_len.try_into()?,
            archive_types: options.types,
        })
    }

    fn concat_directory_and_file_name<'string>(
        directory: &'string Key,
        file: &'string DirectoryKey,
    ) -> Cow<'string, BStr> {
        let directory = &directory.name;
        let file = &file.name;

        let directory = match directory.len() {
            0 => b"".as_bstr(),
            1 => match directory[0] {
                b'/' | b'\\' | b'.' => b"".as_bstr(),
                _ => directory.as_ref(),
            },
            _ => directory.as_ref(),
        };

        match (directory.is_empty(), file.is_empty()) {
            (true, true) => Cow::default(),
            (true, false) => Cow::from(file.as_ref()),
            (false, true) => Cow::from(directory),
            (false, false) => {
                let string: BString = [directory, b"\\".as_bstr(), file.as_ref()]
                    .into_iter()
                    .flat_map(|x| x.as_bytes())
                    .copied()
                    .collect::<Vec<_>>()
                    .into();
                Cow::from(string)
            }
        }
    }

    fn sort_for_write<'this>(&'this self, options: Options) -> Vec<SortedDirectory<'this, 'bytes>> {
        let mut directories: Vec<_> = self
            .iter()
            .map(|(directory_key, directory)| {
                let mut files: Vec<_> = directory
                    .iter()
                    .map(|(file_key, file)| {
                        let embedded_name = match options.version {
                            Version::FO3 | Version::SSE if options.flags.embedded_file_names() => {
                                Some(Self::concat_directory_and_file_name(
                                    directory_key,
                                    file_key,
                                ))
                            }
                            _ => None,
                        };
                        SortedFile {
                            key: file_key,
                            this: file,
                            embedded_name,
                        }
                    })
                    .collect();
                if options.flags.xbox_archive() {
                    files.sort_by_key(|x| x.key.hash.numeric().swap_bytes());
                }
                SortedDirectory {
                    key: directory_key,
                    this: directory,
                    files,
                }
            })
            .collect();
        if options.flags.xbox_archive() {
            directories.sort_by_key(|x| x.key.hash.numeric().swap_bytes());
        }
        directories
    }

    fn write_directory_entry<Out>(
        sink: &mut Sink<Out>,
        options: Options,
        key: &Key,
        directory: &Directory<'bytes>,
        file_entries_offset: &mut u32,
    ) -> Result<()>
    where
        Out: Write,
    {
        Self::write_hash(sink, options, key.hash.into())?;

        let file_count: u32 = directory.len().try_into()?;
        sink.write(&file_count, Endian::Little)?;

        if options.version == Version::SSE {
            sink.write(&0u32, Endian::Little)?;
        }

        sink.write(file_entries_offset, Endian::Little)?;

        if options.version == Version::SSE {
            sink.write(&0u32, Endian::Little)?;
        }

        if options.flags.directory_strings() {
            // bzstring -> include prefix byte and null terminator
            // file_entries_offset += key.name.len() + 2;
            *file_entries_offset = file_entries_offset
                .checked_add((key.name.len() + 2).try_into()?)
                .ok_or(Error::IntegralOverflow)?;
        }

        // file_entries_offset += directory.len() * constants::FILE_ENTRY_SIZE;
        *file_entries_offset = file_entries_offset
            .checked_add(
                directory
                    .len()
                    .checked_mul(constants::FILE_ENTRY_SIZE)
                    .ok_or(Error::IntegralOverflow)?
                    .try_into()?,
            )
            .ok_or(Error::IntegralOverflow)?;

        Ok(())
    }

    fn write_file_data<Out>(
        sink: &mut Sink<Out>,
        file: &File<'bytes>,
        embedded_file_name: Option<&BStr>,
    ) -> Result<()>
    where
        Out: Write,
    {
        if let Some(name) = embedded_file_name {
            sink.write_protocol::<protocols::BString>(name, Endian::Little)?;
        }

        if let Some(len) = file.decompressed_len() {
            let len: u32 = len.try_into()?;
            sink.write(&len, Endian::Little)?;
        }

        sink.write_bytes(file.as_bytes())?;
        Ok(())
    }

    fn write_file_entry<Out>(
        sink: &mut Sink<Out>,
        options: Options,
        key: &DirectoryKey,
        file: &File<'bytes>,
        file_data_offset: &mut u32,
        embedded_file_name: Option<&BStr>,
    ) -> Result<()>
    where
        Out: Write,
    {
        Self::write_hash(sink, options, key.hash.into())?;

        let (size_with_info, size) = {
            let mut size = file.len();
            if let Some(name) = embedded_file_name {
                // include prefix byte
                size += name.len() + 1;
            }
            if file.is_compressed() {
                size += mem::size_of::<u32>();
            }

            let size: u32 = size.try_into()?;
            let masked = size & !(constants::FILE_FLAG_COMPRESSION | constants::FILE_FLAG_CHECKED);
            if masked != size {
                return Err(Error::IntegralTruncation);
            }

            if file.is_compressed() == options.flags.compressed() {
                (size, masked)
            } else {
                (size | constants::FILE_FLAG_COMPRESSION, masked)
            }
        };
        sink.write(&(size_with_info, *file_data_offset), Endian::Little)?;

        // file_data_offset += size;
        *file_data_offset = file_data_offset
            .checked_add(size)
            .ok_or(Error::IntegralOverflow)?;

        Ok(())
    }

    fn write_hash<Out>(sink: &mut Sink<Out>, options: Options, hash: Hash) -> Result<()>
    where
        Out: Write,
    {
        sink.write(
            &(hash.last, hash.last2, hash.length, hash.first),
            Endian::Little,
        )?;

        let endian = if options.flags.xbox_archive() {
            Endian::Big
        } else {
            Endian::Little
        };
        sink.write(&hash.crc, endian)?;

        Ok(())
    }

    fn write_header<Out>(sink: &mut Sink<Out>, header: &Header) -> Result<()>
    where
        Out: Write,
    {
        sink.write(
            &(
                constants::BSA,
                header.version as u32,
                constants::HEADER_SIZE,
                header.archive_flags.bits(),
                header.directory_count,
                header.file_count,
                header.directory_names_len,
                header.file_names_len,
                header.archive_types.bits(),
                0u16,
            ),
            Endian::Little,
        )?;
        Ok(())
    }

    fn do_read<In>(source: &mut In) -> Result<ReadResult<Self>>
    where
        In: ?Sized + Source<'bytes>,
    {
        let header = Self::read_header(source)?;
        let mut offsets = header.compute_offsets();
        let mut map = Map::default();

        for _ in 0..header.directory_count {
            let (key, value) = Self::read_directory(source, &header, &mut offsets)?;
            map.insert(key, value);
        }

        Ok((
            Self { map },
            Options {
                version: header.version,
                flags: header.archive_flags,
                types: header.archive_types,
            },
        ))
    }

    fn read_directory<In>(
        source: &mut In,
        header: &Header,
        offsets: &mut Offsets,
    ) -> Result<(Key, Directory<'bytes>)>
    where
        In: ?Sized + Source<'bytes>,
    {
        let hash = Self::read_hash(source, header.hash_endian())?;
        let file_count: u32 = source.read(Endian::Little)?;
        #[allow(clippy::cast_possible_wrap)]
        match header.version {
            Version::TES4 | Version::FO3 => source.seek_relative(mem::size_of::<u32>() as isize)?,
            Version::SSE => source.seek_relative((mem::size_of::<u32>() * 3) as isize)?,
        }

        let mut map = DirectoryMap::default();
        let (name, directory) =
            source.save_restore_position(|source| -> Result<(BString, Directory<'bytes>)> {
                source.seek_absolute(offsets.file_entries)?;
                let mut name = if header.archive_flags.directory_strings() {
                    Some(source.read_protocol::<BZString>(Endian::Little)?)
                } else {
                    None
                };
                for _ in 0..file_count {
                    let (key, value) = Self::read_file_entry(source, header, offsets, &mut name)?;
                    map.insert(key, value);
                }
                offsets.file_entries = source.stream_position();
                Ok((name.unwrap_or_default(), Directory { map }))
            })??;

        Ok((
            Key {
                hash: hash.into(),
                name,
            },
            directory,
        ))
    }

    fn read_file_entry<In>(
        source: &mut In,
        header: &Header,
        offsets: &mut Offsets,
        directory_name: &mut Option<BString>,
    ) -> Result<(DirectoryKey, File<'bytes>)>
    where
        In: ?Sized + Source<'bytes>,
    {
        let hash = Self::read_hash(source, header.hash_endian())?;
        let (compression_flipped, mut data_size, data_offset) = {
            let (size, offset): (u32, u32) = source.read(Endian::Little)?;
            (
                (size & constants::FILE_FLAG_COMPRESSION) != 0,
                (size & !(constants::FILE_FLAG_COMPRESSION | constants::FILE_FLAG_CHECKED))
                    as usize,
                (offset & !constants::FILE_FLAG_SECONDARY_ARCHIVE) as usize,
            )
        };

        let mut name = if header.archive_flags.file_strings() {
            source.save_restore_position(|source| -> Result<Option<BString>> {
                source.seek_absolute(offsets.file_names)?;
                let result = source.read_protocol::<ZString>(Endian::Little)?;
                offsets.file_names = source.stream_position();
                Ok(Some(result))
            })??
        } else {
            None
        };

        let container =
            source.save_restore_position(|source| -> Result<CompressableBytes<'bytes>> {
                source.seek_absolute(data_offset)?;

                match header.version {
                    Version::FO3 | Version::SSE if header.archive_flags.embedded_file_names() => {
                        let mut s = source.read_protocol::<protocols::BString>(Endian::Little)?;
                        data_size -= s.len() + 1; // include prefix byte
                        if let Some(pos) = s.iter().rposition(|&x| x == b'\\' || x == b'/') {
                            if directory_name.is_none() {
                                *directory_name = Some(s[..pos].into());
                            }
                            s.drain(..=pos);
                        }
                        if name.is_none() {
                            name = Some(s);
                        }
                    }
                    _ => (),
                }

                let decompressed_len =
                    match (header.archive_flags.compressed(), compression_flipped) {
                        (true, false) | (false, true) => {
                            let result: u32 = source.read(Endian::Little)?;
                            data_size -= mem::size_of::<u32>();
                            Some(result as usize)
                        }
                        (true, true) | (false, false) => None,
                    };

                let container = source
                    .read_bytes(data_size)?
                    .into_compressable(decompressed_len);
                Ok(container)
            })??;

        Ok((
            DirectoryKey {
                hash: hash.into(),
                name: name.unwrap_or_default(),
            },
            File { container },
        ))
    }

    fn read_hash<In>(source: &mut In, endian: Endian) -> Result<Hash>
    where
        In: ?Sized + Source<'bytes>,
    {
        let (last, last2, length, first, crc) = source.read(endian)?;
        Ok(Hash {
            last,
            last2,
            length,
            first,
            crc,
        })
    }

    fn read_header<In>(source: &mut In) -> Result<Header>
    where
        In: ?Sized + Source<'bytes>,
    {
        let (
            magic,
            version,
            header_size,
            archive_flags,
            directory_count,
            file_count,
            directory_names_len,
            file_names_len,
            archive_types,
            padding,
        ) = source.read(Endian::Little)?;
        let _: u16 = padding;

        if magic != constants::BSA {
            return Err(Error::InvalidMagic(magic));
        }

        let version = match version {
            103 => Version::TES4,
            104 => Version::FO3,
            105 => Version::SSE,
            _ => return Err(Error::InvalidVersion(version)),
        };

        if header_size != constants::HEADER_SIZE {
            return Err(Error::InvalidHeaderSize(header_size));
        }

        // there probably exist "valid" archives which set extra bits, so it's not worth validating...
        let archive_flags = Flags::from_bits_truncate(archive_flags);
        let archive_types = Types::from_bits_truncate(archive_types);

        Ok(Header {
            version,
            archive_flags,
            directory_count,
            file_count,
            directory_names_len,
            file_names_len,
            archive_types,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        prelude::*,
        tes4::{
            Archive, ArchiveFlags, ArchiveKey, ArchiveOptions, Directory, DirectoryKey, Error,
            File, FileOptions, Version,
        },
        Borrowed,
    };
    use anyhow::Context as _;
    use memmap2::Mmap;
    use std::{
        fs, io,
        path::{Path, PathBuf},
    };

    #[test]
    fn default_state() {
        let bsa = Archive::new();
        assert!(bsa.is_empty());
        assert!(bsa.len() == 0);
    }

    #[test]
    fn read_compressed() -> anyhow::Result<()> {
        let test = |file_name: &str| -> anyhow::Result<()> {
            let root = Path::new("data/tes4_compression_test");

            let (bsa, options) = Archive::read(root.join(file_name).as_path())
                .with_context(|| format!("failed to read archive: {file_name}"))?;
            let compression_options = FileOptions::builder().version(options.version).build();

            let files = ["License.txt", "Preview.png"];
            for file_name in files {
                let path = root.join(file_name);
                let directory = bsa
                    .get(&ArchiveKey::from(b"."))
                    .with_context(|| format!("failed to get directory for: {file_name}"))?;
                let compressed_from_archive = directory
                    .get(&DirectoryKey::from(file_name))
                    .with_context(|| format!("failed to get file for: {file_name}"))?;
                assert!(compressed_from_archive.is_compressed());

                let metadata = fs::metadata(&path)
                    .with_context(|| format!("failed to get metadata for: {path:?}"))?;
                let decompressed_len = compressed_from_archive
                    .decompressed_len()
                    .with_context(|| format!("file was not compressed: {path:?}"))?
                    as u64;
                assert_eq!(decompressed_len, metadata.len());

                let decompressed_from_disk = File::read(path.as_path())
                    .with_context(|| format!("failed to read file from disk: {path:?}"))?;
                let compressed_from_disk = decompressed_from_disk
                    .compress(&compression_options)
                    .with_context(|| format!("failed to compress file: {path:?}"))?;
                assert_eq!(
                    compressed_from_archive.decompressed_len(),
                    compressed_from_disk.decompressed_len()
                );

                let decompressed_from_archive = compressed_from_archive
                    .decompress(&compression_options)
                    .with_context(|| format!("failed to decompress file: {file_name}"))?;
                assert_eq!(
                    decompressed_from_archive.as_bytes(),
                    decompressed_from_disk.as_bytes()
                );
            }

            Ok(())
        };

        test("test_104.bsa").context("v104")?;
        test("test_105.bsa").context("v105")?;

        Ok(())
    }

    #[test]
    fn xbox_decompressed_read() -> anyhow::Result<()> {
        let root = Path::new("data/tes4_xbox_read_test");

        let (normal, normal_options) = Archive::read(root.join("normal.bsa").as_path())
            .context("failed to read normal archive")?;
        assert!(!normal_options.flags().xbox_archive());
        assert!(!normal_options.flags().xbox_compressed());
        assert!(!normal_options.flags().compressed());

        let (xbox, xbox_options) = Archive::read(root.join("xbox.bsa").as_path())
            .context("failed to read xbox archive")?;
        assert!(xbox_options.flags().xbox_archive());
        assert!(!xbox_options.flags().xbox_compressed());
        assert!(!xbox_options.flags().compressed());

        assert_eq!(normal.len(), xbox.len());
        for (directory_normal, directory_xbox) in normal.iter().zip(xbox) {
            assert_eq!(directory_normal.0.hash, directory_xbox.0.hash);
            assert_eq!(directory_normal.0.name, directory_xbox.0.name);
            assert_eq!(directory_normal.1.len(), directory_xbox.1.len());

            for (file_normal, file_xbox) in directory_normal.1.iter().zip(directory_xbox.1) {
                assert_eq!(file_normal.0.hash, file_xbox.0.hash);
                assert_eq!(file_normal.0.name, file_xbox.0.name);
                assert!(!file_normal.1.is_compressed());
                assert!(!file_xbox.1.is_compressed());
                assert_eq!(file_normal.1.len(), file_xbox.1.len());
                assert_eq!(file_normal.1.as_bytes(), file_xbox.1.as_bytes());
            }
        }

        Ok(())
    }

    #[test]
    fn xbox_decompressed_write() -> anyhow::Result<()> {
        let path = Path::new("data/tes4_xbox_write_test/in.bsa");

        let original = {
            let fd =
                fs::File::open(path).with_context(|| format!("failed to open file: {path:?}"))?;
            unsafe { Mmap::map(&fd) }
                .with_context(|| format!("failed to memory map file: {path:?}"))?
        };

        let copy = {
            let (archive, options) =
                Archive::read(path).with_context(|| format!("failed to read archive: {path:?}"))?;
            let mut v = Vec::new();
            archive
                .write(&mut v, &options)
                .with_context(|| format!("failed to write archive: {path:?}"))?;
            v
        };

        let original = &original[..];
        let copy = &copy[..];
        assert_eq!(original.len(), copy.len());
        assert_eq!(original, copy);

        Ok(())
    }

    #[test]
    fn file_compression_diverges_from_archive_compression() -> anyhow::Result<()> {
        let root = Path::new("data/tes4_compression_mismatch_test");
        let (bsa, options) =
            Archive::read(root.join("test.bsa").as_path()).context("failed to read archive")?;
        assert!(options.flags().compressed());

        let files = ["License.txt", "SampleA.png"];
        let directory = bsa
            .get(&ArchiveKey::from(b"."))
            .context("failed to get root directory from archive")?;
        assert_eq!(directory.len(), files.len());

        for file_name in files {
            let path = root.join(file_name);
            let metadata = fs::metadata(&path)
                .with_context(|| format!("failed to get metadata for file: {path:?}"))?;
            let file = directory
                .get(&DirectoryKey::from(file_name))
                .with_context(|| format!("failed to get file from directory: {file_name}"))?;
            assert!(!file.is_compressed());
            assert_eq!(file.len() as u64, metadata.len());
        }

        Ok(())
    }

    #[test]
    fn flag_combinations() -> anyhow::Result<()> {
        let infos = {
            struct Pair {
                hash: u64,
                name: &'static str,
            }

            struct Info {
                directory: Pair,
                file: Pair,
            }

            impl Info {
                fn new(
                    directory_hash: u64,
                    directory_name: &'static str,
                    file_hash: u64,
                    file_name: &'static str,
                ) -> Self {
                    Self {
                        directory: Pair {
                            hash: directory_hash,
                            name: directory_name,
                        },
                        file: Pair {
                            hash: file_hash,
                            name: file_name,
                        },
                    }
                }
            }

            [
                Info::new(
                    0x006819F973057265,
                    "Share",
                    0xDC415D456C077365,
                    "License.txt",
                ),
                Info::new(
                    0x00691A4374056573,
                    "Tiles",
                    0xDDE285B874093030,
                    "tile_0000.png",
                ),
                Info::new(
                    0x0E09AFBA620A6E64,
                    "Background",
                    0xC41A947762116F6D,
                    "background_bottom.png",
                ),
                Info::new(
                    0x4ADF420B74076170,
                    "Tilemap",
                    0x0D9BA627630A7273,
                    "characters.png",
                ),
                Info::new(
                    0x6A326CD4630B2033,
                    "Construct 3",
                    0xC7EDDCEA72066D65,
                    "Readme.txt",
                ),
                Info::new(
                    0x79CD3FEC630A7273,
                    "Characters",
                    0xD0E4FC14630E3030,
                    "character_0000.png",
                ),
            ]
        };

        let mappings: Vec<_> = infos
            .iter()
            .map(|info| {
                let path: PathBuf = [
                    "data/tes4_flags_test",
                    "data",
                    info.directory.name,
                    info.file.name,
                ]
                .into_iter()
                .collect();
                let fd = fs::File::open(&path)
                    .with_context(|| format!("failed to open file: {path:?}"))?;
                let map = unsafe { Mmap::map(&fd) }
                    .with_context(|| format!("failed to memory map file: {path:?}"))?;
                Ok(map)
            })
            .collect::<anyhow::Result<_>>()?;
        let main: Archive = infos
            .iter()
            .zip(&mappings)
            .map(|(info, mapping)| {
                let file = File::from_decompressed(&mapping[..]);
                let directory: Directory = [(info.file.name.into(), file)].into_iter().collect();
                (info.directory.name.into(), directory)
            })
            .collect();

        let test = |version: Version, flags: ArchiveFlags| -> anyhow::Result<()> {
            let buffer = {
                let mut v = Vec::new();
                let options = ArchiveOptions::builder()
                    .version(version)
                    .flags(flags)
                    .build();
                main.write(&mut v, &options)
                    .context("failed to write archive to buffer")?;
                v
            };

            let (child, options) =
                Archive::read(Borrowed(&buffer)).context("failed to read archive from buffer")?;
            assert_eq!(options.version(), version);
            assert_eq!(options.flags(), flags);
            assert_eq!(main.len(), child.len());

            let embedded_file_names = version != Version::TES4 && flags.embedded_file_names();

            for (info, mapping) in infos.iter().zip(&mappings) {
                let archive_key: ArchiveKey = info.directory.name.into();
                let directory = child
                    .get_key_value(&archive_key)
                    .with_context(|| format!("failed to get directory: {}", info.directory.name))?;
                assert_eq!(directory.0.hash.numeric(), info.directory.hash);
                assert_eq!(directory.1.len(), 1);
                if flags.directory_strings() || embedded_file_names {
                    assert_eq!(directory.0.name, archive_key.name);
                }

                let directory_key: DirectoryKey = info.file.name.into();
                let file = directory
                    .1
                    .get_key_value(&directory_key)
                    .with_context(|| format!("failed to get file: {}", info.file.name))?;
                assert_eq!(file.0.hash.numeric(), info.file.hash);
                if flags.file_strings() || embedded_file_names {
                    assert_eq!(file.0.name, directory_key.name);
                }

                let decompressed_file = if file.1.is_compressed() {
                    let options = FileOptions::builder().version(version).build();
                    let result = file.1.decompress(&options).with_context(|| {
                        format!("failed to decompress file: {}", info.file.name)
                    })?;
                    Some(result)
                } else {
                    None
                };
                let decompressed_bytes = decompressed_file.as_ref().unwrap_or(file.1).as_bytes();
                assert_eq!(decompressed_bytes, &mapping[..]);
            }

            Ok(())
        };

        let versions = [Version::TES4, Version::FO3, Version::SSE];
        let flags = [
            ArchiveFlags::DIRECTORY_STRINGS,
            ArchiveFlags::FILE_STRINGS,
            ArchiveFlags::COMPRESSED,
            ArchiveFlags::XBOX_ARCHIVE,
            ArchiveFlags::EMBEDDED_FILE_NAMES,
        ];

        for version in versions {
            for i in 0..flags.len() {
                for j in i..flags.len() {
                    let f = flags[i..=j]
                        .iter()
                        .copied()
                        .reduce(|acc, x| acc.union(x))
                        .unwrap();
                    test(version, f).with_context(|| {
                        format!("archive test failed for version ({version:?}) with flags ({f:?})")
                    })?;
                }
            }
        }

        Ok(())
    }

    #[test]
    fn invalid_magic() -> anyhow::Result<()> {
        let path = Path::new("data/tes4_invalid_test/invalid_magic.bsa");
        match Archive::read(path) {
            Err(Error::InvalidMagic(0x00324142)) => Ok(()),
            Err(err) => Err(anyhow::Error::from(err)),
            Ok(_) => anyhow::bail!("read should have failed"),
        }
    }

    #[test]
    fn invalid_size() -> anyhow::Result<()> {
        let path = Path::new("data/tes4_invalid_test/invalid_size.bsa");
        match Archive::read(path) {
            Err(Error::InvalidHeaderSize(0xCC)) => Ok(()),
            Err(err) => Err(anyhow::Error::from(err)),
            Ok(_) => anyhow::bail!("read should have failed"),
        }
    }

    #[test]
    fn invalid_version() -> anyhow::Result<()> {
        let path = Path::new("data/tes4_invalid_test/invalid_version.bsa");
        match Archive::read(path) {
            Err(Error::InvalidVersion(42)) => Ok(()),
            Err(err) => Err(anyhow::Error::from(err)),
            Ok(_) => anyhow::bail!("read should have failed"),
        }
    }

    #[test]
    fn invalid_exhausted() -> anyhow::Result<()> {
        let path = Path::new("data/tes4_invalid_test/invalid_exhausted.bsa");
        match Archive::read(path) {
            Err(Error::Io(error)) => {
                assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
                Ok(())
            }
            Err(err) => Err(anyhow::Error::from(err)),
            Ok(_) => anyhow::bail!("read should have failed"),
        }
    }

    #[test]
    fn data_sharing_name() -> anyhow::Result<()> {
        let path = Path::new("data/tes4_data_sharing_name_test/share.bsa");
        let (archive, options) = Archive::read(path).context("failed to read archive")?;
        assert_eq!(options.version, Version::TES5);
        assert!(options.flags.embedded_file_names());

        let find = |directory_name: &str, file_name: &str| -> anyhow::Result<()> {
            let directory = archive
                .get_key_value(&ArchiveKey::from(directory_name))
                .with_context(|| format!("failed to get directory: {directory_name}"))?;
            assert_eq!(directory.0.name, directory_name);

            let file = directory
                .1
                .get_key_value(&DirectoryKey::from(file_name))
                .with_context(|| format!("failed to get file: {file_name}"))?;
            assert_eq!(file.0.name, file_name);

            Ok(())
        };

        find("misc1", "example1.txt")?;
        find("misc2", "example2.txt")?;

        Ok(())
    }
}
