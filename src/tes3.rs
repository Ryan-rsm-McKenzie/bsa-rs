use crate::{
    containers::ByteContainer,
    io::{BorrowedSource, CopiedSource, Endian, MappedSource, Sink, Source},
    strings::ZString,
    Borrowed, Copied, Read,
};
use bstr::BString;
use std::{
    borrow::Borrow,
    cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd},
    collections::BTreeMap,
    fs,
    io::{self, Write},
    num::TryFromIntError,
};

#[non_exhaustive]
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("invalid magic read from file header")]
    InvalidMagic(u32),

    #[error(transparent)]
    IntegralTruncation(#[from] TryFromIntError),

    #[error(transparent)]
    Io(#[from] io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

mod constants {
    pub const FILE_ENTRY_SIZE: u32 = 0x8;
    pub const HASH_SIZE: u32 = 0x8;
    pub const HEADER_MAGIC: u32 = 0x100;
    pub const HEADER_SIZE: u32 = 0xC;
}

struct Offsets {
    name_offsets: u32,
    names: u32,
    hashes: u32,
    file_data: u32,
}

struct Header {
    hash_offset: u32,
    file_count: u32,
}

impl Header {
    fn compute_offsets(&self) -> Offsets {
        let name_offsets = constants::HEADER_SIZE + constants::FILE_ENTRY_SIZE * self.file_count;
        let names = name_offsets + 0x4 * self.file_count;
        let hashes = constants::HEADER_SIZE + self.hash_offset;
        let file_data = hashes + constants::HASH_SIZE * self.file_count;
        Offsets {
            name_offsets,
            names,
            hashes,
            file_data,
        }
    }
}

pub mod hashing {
    use crate::hashing as detail;
    use bstr::{BStr, BString};
    use std::cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd};

    #[derive(Clone, Copy, Debug, Default)]
    pub struct Hash {
        pub lo: u32,
        pub hi: u32,
    }

    impl Hash {
        #[must_use]
        pub fn new() -> Self {
            Self::default()
        }

        #[must_use]
        pub fn numeric(&self) -> u64 {
            u64::from(self.hi) | (u64::from(self.lo) << (4 * 8))
        }
    }

    impl PartialEq for Hash {
        fn eq(&self, other: &Self) -> bool {
            self.numeric() == other.numeric()
        }
    }

    impl Eq for Hash {}

    impl PartialOrd for Hash {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }

    impl Ord for Hash {
        fn cmp(&self, other: &Self) -> Ordering {
            self.numeric().cmp(&other.numeric())
        }
    }

    #[must_use]
    pub fn hash_file(path: &BStr) -> (Hash, BString) {
        let mut path = BString::new(path.to_vec());
        (hash_file_in_place(&mut path), path)
    }

    pub fn hash_file_in_place(path: &mut BString) -> Hash {
        detail::normalize_path(path);
        let midpoint = path.len() / 2;
        let mut h = Hash::new();
        let mut i: usize = 0;

        // rotate between first 4 bytes
        while i < midpoint {
            h.lo ^= u32::from(path[i]) << ((i % 4) * 8);
            i += 1;
        }

        // rotate between last 4 bytes
        while i < path.len() {
            let rot = u32::from(path[i]) << (((i - midpoint) % 4) * 8);
            h.hi = u32::rotate_right(h.hi ^ rot, rot);
            i += 1;
        }

        h
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use bstr::ByteSlice as _;

        #[test]
        fn hashes_start_empty() {
            let h: Hash = Default::default();
            assert_eq!(h.lo, 0);
            assert_eq!(h.hi, 0);
            assert_eq!(h.numeric(), 0);
        }

        #[test]
        fn validate_hashing() {
            let hash = |x: &[u8]| super::hash_file(x.as_bstr()).0.numeric();
            assert_eq!(
                hash(b"meshes/c/artifact_bloodring_01.nif"),
                0x1C3C1149920D5F0C
            );
            assert_eq!(
                hash(b"meshes/x/ex_stronghold_pylon00.nif"),
                0x20250749ACCCD202
            );
            assert_eq!(hash(b"meshes/r/xsteam_centurions.kf"), 0x6E5C0F3125072EA6);
            assert_eq!(hash(b"textures/tx_rock_cave_mu_01.dds"), 0x58060C2FA3D8F759);
            assert_eq!(hash(b"meshes/f/furn_ashl_chime_02.nif"), 0x7C3B2F3ABFFC8611);
            assert_eq!(hash(b"textures/tx_rope_woven.dds"), 0x5865632F0C052C64);
            assert_eq!(hash(b"icons/a/tx_templar_skirt.dds"), 0x46512A0B60EDA673);
            assert_eq!(hash(b"icons/m/misc_prongs00.dds"), 0x51715677BBA837D3);
            assert_eq!(
                hash(b"meshes/i/in_c_stair_plain_tall_02.nif"),
                0x2A324956BF89B1C9
            );
            assert_eq!(hash(b"meshes/r/xkwama worker.nif"), 0x6D446E352C3F5A1E);
        }

        #[test]
        fn forward_slashes_are_same_as_back_slashes() {
            let hash = |x: &[u8]| super::hash_file(x.as_bstr()).0;
            assert_eq!(hash(b"foo/bar/baz"), hash(b"foo\\bar\\baz"));
        }

        #[test]
        fn hashes_are_case_insensitive() {
            let hash = |x: &[u8]| super::hash_file(x.as_bstr()).0;
            assert_eq!(hash(b"FOO/BAR/BAZ"), hash(b"foo/bar/baz"));
        }

        #[test]
        fn sort_order() {
            let lhs = Hash { lo: 0, hi: 1 };
            let rhs = Hash { lo: 1, hi: 0 };
            assert!(lhs < rhs);
        }
    }
}

use hashing::Hash;

#[derive(Default)]
pub struct File<'a> {
    bytes: ByteContainer<'a>,
}

impl<'a> File<'a> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.bytes.as_bytes()
    }

    #[must_use]
    pub fn as_ptr(&self) -> *const u8 {
        self.bytes.as_ptr()
    }

    #[must_use]
    pub fn into_owned(self) -> File<'static> {
        File {
            bytes: self.bytes.into_owned(),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    fn do_read<I>(stream: &mut I) -> Self
    where
        I: ?Sized + Source<'a>,
    {
        Self {
            bytes: stream.read_to_end(),
        }
    }

    pub fn write<O>(&self, stream: &mut O) -> Result<()>
    where
        O: ?Sized + Write,
    {
        stream.write_all(self.as_bytes())?;
        Ok(())
    }

    fn from_container(bytes: ByteContainer<'a>) -> Self {
        Self { bytes }
    }
}

impl<'a> From<&'a [u8]> for File<'a> {
    fn from(value: &'a [u8]) -> Self {
        Self {
            bytes: ByteContainer::from_borrowed(value),
        }
    }
}

impl From<Vec<u8>> for File<'static> {
    fn from(value: Vec<u8>) -> Self {
        Self {
            bytes: ByteContainer::from_owned(value),
        }
    }
}

impl<'a> Read<Borrowed<'a>> for File<'a> {
    type Error = Error;

    fn read(source: Borrowed<'a>) -> Result<Self> {
        let mut source = BorrowedSource::from(source.0);
        Ok(Self::do_read(&mut source))
    }
}

impl<'a> Read<Copied<'a>> for File<'static> {
    type Error = Error;

    fn read(source: Copied<'a>) -> Result<Self> {
        let mut source = CopiedSource::from(source.0);
        Ok(Self::do_read(&mut source))
    }
}

impl Read<fs::File> for File<'static> {
    type Error = Error;

    fn read(source: fs::File) -> Result<Self> {
        let mut source = MappedSource::try_from(source)?;
        Ok(Self::do_read(&mut source))
    }
}

#[derive(Clone, Debug, Default)]
pub struct Key {
    pub hash: Hash,
    pub name: BString,
}

impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.hash.eq(&other.hash)
    }
}

impl Eq for Key {}

impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Key {
    fn cmp(&self, other: &Self) -> Ordering {
        self.hash.cmp(&other.hash)
    }
}

impl Borrow<Hash> for Key {
    fn borrow(&self) -> &Hash {
        &self.hash
    }
}

impl From<BString> for Key {
    fn from(mut name: BString) -> Self {
        let hash = hashing::hash_file_in_place(&mut name);
        Self { hash, name }
    }
}

type FileMap<'a> = BTreeMap<Key, File<'a>>;

#[derive(Default)]
pub struct Archive<'a> {
    files: FileMap<'a>,
}

impl<'a> Archive<'a> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Key, &File<'a>)> {
        self.files.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&Key, &mut File<'a>)> {
        self.files.iter_mut()
    }

    pub fn get<K>(&self, key: &K) -> Option<&File<'a>>
    where
        K: Borrow<Hash>,
    {
        self.files.get(key.borrow())
    }

    pub fn clear(&mut self) {
        self.files.clear();
    }

    pub fn remove<K>(&mut self, key: &K) -> Option<File<'a>>
    where
        K: Borrow<Hash>,
    {
        self.files.remove(key.borrow())
    }

    pub fn remove_entry<K>(&mut self, key: &K) -> Option<(Key, File<'a>)>
    where
        K: Borrow<Hash>,
    {
        self.files.remove_entry(key.borrow())
    }

    pub fn insert<K>(&mut self, key: K, value: File<'a>) -> Option<File<'a>>
    where
        K: Into<Key>,
    {
        self.files.insert(key.into(), value)
    }

    fn do_read<I>(source: &mut I) -> Result<Self>
    where
        I: ?Sized + Source<'a>,
    {
        let header = Self::read_header(source)?;
        let offsets = header.compute_offsets();
        let mut files = FileMap::default();

        for i in 0..header.file_count {
            let (hash, name, file) = Self::read_file(source, i, &offsets)?;
            files.insert(Key { hash, name }, file);
        }

        Ok(Self { files })
    }

    fn read_file<I>(
        source: &mut I,
        idx: u32,
        offsets: &Offsets,
    ) -> Result<(Hash, BString, File<'a>)>
    where
        I: ?Sized + Source<'a>,
    {
        let hash = source.save_restore_position(|source| -> Result<Hash> {
            source.seek_absolute((offsets.hashes + constants::HASH_SIZE * idx) as usize)?;
            Self::read_hash(source)
        })??;

        let name = source.save_restore_position(|source| -> Result<BString> {
            source.seek_absolute((offsets.name_offsets + 0x4 * idx) as usize)?;
            let offset: u32 = source.read(Endian::Little)?;
            source.seek_absolute((offsets.names + offset) as usize)?;
            let name = source.read_protocol::<ZString>(Endian::Little)?;
            Ok(name)
        })??;

        let (size, offset): (u32, u32) = source.read(Endian::Little)?;
        let data = source.save_restore_position(|source| -> Result<ByteContainer<'a>> {
            source.seek_absolute((offsets.file_data + offset) as usize)?;
            let result = source.read_container(size as usize)?;
            Ok(result)
        })??;

        let file = File::from_container(data);
        Ok((hash, name, file))
    }

    fn read_hash<I>(source: &mut I) -> Result<Hash>
    where
        I: ?Sized + Source<'a>,
    {
        let (lo, hi) = source.read(Endian::Little)?;
        Ok(Hash { lo, hi })
    }

    fn read_header<I>(source: &mut I) -> Result<Header>
    where
        I: ?Sized + Source<'a>,
    {
        let (magic, hash_offset, file_count) = source.read(Endian::Little)?;
        match magic {
            constants::HEADER_MAGIC => Ok(Header {
                hash_offset,
                file_count,
            }),
            _ => Err(Error::InvalidMagic(magic)),
        }
    }

    fn make_header(&self) -> Result<Header> {
        Ok(Header {
            file_count: self.files.len().try_into()?,
            hash_offset: {
                let names_offset = 0xC * self.files.len();
                let names_len: usize = self.files.keys().map(|x| x.name.len() + 1).sum();
                (names_offset + names_len).try_into()?
            },
        })
    }

    pub fn write<O>(&self, stream: &mut O) -> Result<()>
    where
        O: Write,
    {
        let mut sink = Sink::new(stream);
        let header = self.make_header()?;
        Self::write_header(&mut sink, &header)?;
        self.write_files(&mut sink)?;
        self.write_name_offsets(&mut sink)?;
        self.write_names(&mut sink)?;
        self.write_hashes(&mut sink)?;
        self.write_file_data(&mut sink)?;

        Ok(())
    }

    fn write_files<O>(&self, sink: &mut Sink<O>) -> Result<()>
    where
        O: Write,
    {
        let mut offset: u32 = 0;
        for file in self.files.values() {
            let size: u32 = file.bytes.len().try_into()?;
            sink.write(&(size, offset), Endian::Little)?;
            offset += size;
        }

        Ok(())
    }

    fn write_file_data<O>(&self, sink: &mut Sink<O>) -> Result<()>
    where
        O: Write,
    {
        for file in self.files.values() {
            sink.write_bytes(file.as_bytes())?;
        }

        Ok(())
    }

    fn write_hashes<O>(&self, sink: &mut Sink<O>) -> Result<()>
    where
        O: Write,
    {
        for key in self.files.keys() {
            let hash = &key.hash;
            sink.write(&(hash.lo, hash.hi), Endian::Little)?;
        }

        Ok(())
    }

    fn write_header<O>(sink: &mut Sink<O>, header: &Header) -> Result<()>
    where
        O: Write,
    {
        sink.write(
            &(
                constants::HEADER_MAGIC,
                header.hash_offset,
                header.file_count,
            ),
            Endian::Little,
        )?;
        Ok(())
    }

    fn write_name_offsets<O>(&self, sink: &mut Sink<O>) -> Result<()>
    where
        O: Write,
    {
        let mut offset: u32 = 0;
        for key in self.files.keys() {
            sink.write(&offset, Endian::Little)?;
            offset += u32::try_from(key.name.len() + 1)?;
        }

        Ok(())
    }

    fn write_names<O>(&self, sink: &mut Sink<O>) -> Result<()>
    where
        O: Write,
    {
        for key in self.files.keys() {
            sink.write_protocol::<ZString>(&key.name, Endian::Little)?;
        }

        Ok(())
    }
}

impl<'a> IntoIterator for Archive<'a> {
    type Item = <FileMap<'a> as IntoIterator>::Item;
    type IntoIter = <FileMap<'a> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.files.into_iter()
    }
}

impl<'a, 'b> IntoIterator for &'b Archive<'a> {
    type Item = <&'b FileMap<'a> as IntoIterator>::Item;
    type IntoIter = <&'b FileMap<'a> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.files.iter()
    }
}

impl<'a, 'b> IntoIterator for &'b mut Archive<'a> {
    type Item = <&'b mut FileMap<'a> as IntoIterator>::Item;
    type IntoIter = <&'b mut FileMap<'a> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.files.iter_mut()
    }
}

impl<'a> Read<Borrowed<'a>> for Archive<'a> {
    type Error = Error;

    fn read(source: Borrowed<'a>) -> Result<Self> {
        let mut source = BorrowedSource::from(source.0);
        Self::do_read(&mut source)
    }
}

impl<'a> Read<Copied<'a>> for Archive<'static> {
    type Error = Error;

    fn read(source: Copied<'a>) -> Result<Self> {
        let mut source = CopiedSource::from(source.0);
        Self::do_read(&mut source)
    }
}

impl Read<fs::File> for Archive<'static> {
    type Error = Error;

    fn read(source: fs::File) -> Result<Self> {
        let mut source = MappedSource::try_from(source)?;
        Self::do_read(&mut source)
    }
}

#[cfg(test)]
mod tests {
    mod file {
        use super::super::*;

        #[test]
        fn default_state() -> anyhow::Result<()> {
            let f = File::new();
            assert!(f.is_empty());
            assert!(f.len() == 0);
            assert!(f.as_bytes().is_empty());
            Ok(())
        }
    }

    mod archive {
        use super::super::*;
        use anyhow::Context as _;
        use bstr::ByteSlice as _;
        use memmap2::Mmap;
        use std::{ffi::OsStr, fs, io::Read as _, path::Path};
        use walkdir::WalkDir;

        #[test]
        fn default_state() -> anyhow::Result<()> {
            let bsa = Archive::new();
            assert!(bsa.is_empty());
            assert!(bsa.len() == 0);
            Ok(())
        }

        #[test]
        fn invalid_magic() -> anyhow::Result<()> {
            let path = Path::new("data/tes3_invalid_test/invalid_magic.bsa");
            let stream =
                fs::File::open(&path).with_context(|| format!("failed to open file: {path:?}"))?;
            let read_result = Archive::read(stream);
            let test = match read_result {
                Err(Error::InvalidMagic(0x200)) => true,
                _ => false,
            };
            assert!(test);

            Ok(())
        }

        #[test]
        fn reading() -> anyhow::Result<()> {
            let root_path = Path::new("data/tes3_read_test/");
            let archive = {
                let archive_path = root_path.join("test.bsa");
                let stream = fs::File::open(&archive_path)
                    .with_context(|| format!("failed to open test archive: {archive_path:?}"))?;
                Archive::read(stream)
                    .with_context(|| format!("failed to read from archive: {archive_path:?}"))?
            };

            for file_path in WalkDir::new(root_path) {
                if let Ok(file_path) = file_path {
                    let metadata = file_path
                        .metadata()
                        .context("failed to get file path metadata")?;
                    if metadata.is_file() && file_path.path().extension() != Some(OsStr::new("bsa"))
                    {
                        let key = file_path
                            .path()
                            .strip_prefix(root_path)
                            .with_context(|| {
                                format!(
                                "failed to strip prefix ({root_path:?}) from path ({file_path:?})"
                            )
                            })?
                            .as_os_str();
                        let file_hash = hashing::hash_file(key.as_encoded_bytes().as_bstr()).0;
                        let file = archive
                            .get(&file_hash)
                            .with_context(|| format!("failed to get file with key: {key:?}"))?;
                        assert_eq!(file.len() as u64, metadata.len());

                        let mut original_data = Vec::new();
                        fs::File::open(file_path.path())
                            .with_context(|| format!("failed to open file: {file_path:?}"))?
                            .read_to_end(&mut original_data)
                            .with_context(|| format!("failed to read from file: {file_path:?}"))?;
                        assert_eq!(file.as_bytes(), &original_data[..]);
                    }
                }
            }

            Ok(())
        }

        #[test]
        fn writing() -> anyhow::Result<()> {
            struct Info<'a> {
                key: Key,
                path: &'a Path,
            }

            impl<'a> Info<'a> {
                fn new(lo: u32, hi: u32, path: &'a str) -> Self {
                    let hash = Hash { lo, hi };
                    let key = Key::from(BString::from(path));
                    assert_eq!(hash, key.hash);
                    Self {
                        key,
                        path: Path::new(path),
                    }
                }
            }

            let infos = [
                Info::new(0x0C18356B, 0xA578DB74, "Tiles/tile_0001.png"),
                Info::new(0x1B0D3416, 0xF5D5F30E, "Share/License.txt"),
                Info::new(0x1B3B140A, 0x07B36E53, "Background/background_middle.png"),
                Info::new(0x29505413, 0x1EB4CED7, "Construct 3/Pixel Platformer.c3p"),
                Info::new(0x4B7D031B, 0xD4701AD4, "Tilemap/characters_packed.png"),
                Info::new(0x74491918, 0x2BEBCD0A, "Characters/character_0001.png"),
            ];

            let mmapped = {
                let mut result = Vec::<Mmap>::new();
                for info in &infos {
                    let file_path = Path::new("data/tes3_write_test/data").join(info.path);
                    let fd = fs::File::open(file_path.clone())
                        .with_context(|| format!("failed to open file: {file_path:?}"))?;
                    let file = unsafe {
                        Mmap::map(&fd)
                            .with_context(|| format!("failed to memory map file: {file_path:?}"))?
                    };
                    result.push(file);
                }
                result
            };

            let stream = {
                let mut archive = Archive::new();
                for (data, info) in mmapped.iter().zip(&infos) {
                    let file = File::from(&data[..]);
                    assert!(archive.insert(info.key.clone(), file).is_none());
                }
                let mut result = Vec::<u8>::new();
                archive
                    .write(&mut result)
                    .context("failed to write test archive to memory")?;
                result
            };

            let archive = Archive::read(Borrowed(&stream))
                .context("failed to read from archive in memory")?;
            for (data, info) in mmapped.iter().zip(&infos) {
                let file = archive.get(&info.key.hash).with_context(|| {
                    format!("failed to get value from archive with key: {:?}", info.path)
                })?;
                assert_eq!(file.as_bytes(), &data[..]);
            }

            Ok(())
        }

        #[test]
        fn assert_generic_interfaces_compile() -> anyhow::Result<()> {
            let mut bsa = Archive::default();
            let key = Key::default();
            let hash = Hash::default();

            _ = bsa.get(&key);
            _ = bsa.get(&hash);

            _ = bsa.remove(&key);
            _ = bsa.remove(&hash);

            _ = bsa.remove_entry(&key);
            _ = bsa.remove_entry(&hash);

            _ = bsa.insert(key, Default::default());
            _ = bsa.insert(BString::default(), Default::default());

            Ok(())
        }
    }
}
