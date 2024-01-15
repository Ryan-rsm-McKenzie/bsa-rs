use crate::{
    containers::CompressableByteContainer,
    derive,
    io::{Endian, Source},
    protocols::{self, BZString, ZString},
    tes4::{
        self, directory::Map as DirectoryMap, Directory, DirectoryKey, Error, File, Hash, Result,
        Version,
    },
};
use bstr::BString;
use core::mem;

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

    pub const BSA: u32 = cc::make_four(b"BSA");

    pub const HEADER_SIZE: u32 = 0x24;
    pub const DIRECTORY_ENTRY_SIZE_X86: usize = 0x10;
    pub const DIRECTORY_ENTRY_SIZE_X64: usize = 0x18;
    pub const FILE_ENTRY_SIZE: usize = 0x10;

    pub const FILE_FLAG_COMPRESSION: u32 = 1 << 30;
    pub const FILE_FLAG_CHECKED: u32 = 1 << 31;
    pub const FILE_FLAG_SECONDARY_ARCHIVE: u32 = 1 << 31;
}

struct Offsets {
    file_entries: usize,
    file_names: usize,
}

struct Header {
    version: Version,
    archive_flags: Flags,
    directory_count: u32,
    file_count: u32,
    directory_names_len: u32,
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
        Offsets {
            file_entries,
            file_names,
        }
    }
}

derive::key!(Key);

impl Key {
    #[must_use]
    fn hash_in_place(name: &mut BString) -> Hash {
        tes4::hash_directory_in_place(name)
    }
}

#[non_exhaustive]
#[derive(Clone, Copy, Default)]
pub struct Options {
    pub version: Version,
    pub flags: Flags,
    pub types: Types,
}

type ReadResult<T> = (T, Options);
derive::archive!(Archive => ReadResult, Map: Key => Directory);

impl<'a> Archive<'a> {
    fn do_read<I>(source: &mut I) -> Result<ReadResult<Self>>
    where
        I: ?Sized + Source<'a>,
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

    fn read_directory<I>(
        source: &mut I,
        header: &Header,
        offsets: &mut Offsets,
    ) -> Result<(Key, Directory<'a>)>
    where
        I: ?Sized + Source<'a>,
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
            source.save_restore_position(|source| -> Result<(BString, Directory<'a>)> {
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

        Ok((Key { hash, name }, directory))
    }

    fn read_file_entry<I>(
        source: &mut I,
        header: &Header,
        offsets: &mut Offsets,
        directory_name: &mut Option<BString>,
    ) -> Result<(DirectoryKey, File<'a>)>
    where
        I: ?Sized + Source<'a>,
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
            source.save_restore_position(|source| -> Result<CompressableByteContainer<'a>> {
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
                    .read_container(data_size)?
                    .into_compressable(decompressed_len);
                Ok(container)
            })??;

        Ok((
            DirectoryKey {
                hash,
                name: name.unwrap_or_default(),
            },
            File { container },
        ))
    }

    fn read_hash<I>(source: &mut I, endian: Endian) -> Result<Hash>
    where
        I: ?Sized + Source<'a>,
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

    fn read_header<I>(source: &mut I) -> Result<Header>
    where
        I: ?Sized + Source<'a>,
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
        let _: u32 = file_names_len;
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
            archive_types,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        prelude::*,
        tes4::{Archive, ArchiveKey, DirectoryKey, Error, File, FileCompressionOptions},
    };
    use anyhow::Context as _;
    use std::{fs, io, path::Path};

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
            let compression_options = {
                let mut x = FileCompressionOptions::default();
                x.version = options.version;
                x
            };

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
                    .compress(compression_options)
                    .with_context(|| format!("failed to compress file: {path:?}"))?;
                assert_eq!(
                    compressed_from_archive.decompressed_len(),
                    compressed_from_disk.decompressed_len()
                );

                let decompressed_from_archive = compressed_from_archive
                    .decompress(compression_options)
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
    fn xbox_decompressed() -> anyhow::Result<()> {
        let root = Path::new("data/tes4_xbox_read_test");

        let (normal, normal_options) = Archive::read(root.join("normal.bsa").as_path())
            .context("failed to read normal archive")?;
        assert!(!normal_options.flags.xbox_archive());
        assert!(!normal_options.flags.xbox_compressed());
        assert!(!normal_options.flags.compressed());

        let (xbox, xbox_options) = Archive::read(root.join("xbox.bsa").as_path())
            .context("failed to read xbox archive")?;
        assert!(xbox_options.flags.xbox_archive());
        assert!(!xbox_options.flags.xbox_compressed());
        assert!(!xbox_options.flags.compressed());

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
    fn file_compression_diverges_from_archive_compression() -> anyhow::Result<()> {
        let root = Path::new("data/tes4_compression_mismatch_test");
        let (bsa, options) =
            Archive::read(root.join("test.bsa").as_path()).context("failed to read archive")?;
        assert!(options.flags.compressed());

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
}
