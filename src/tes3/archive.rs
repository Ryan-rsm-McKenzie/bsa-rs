use crate::{
    containers::ByteContainer,
    derive,
    io::{Endian, Sink, Source},
    strings::ZString,
    tes3::{hashing, Error, File, Hash, Result},
};
use bstr::BString;
use std::io::Write;

mod constants {
    pub const FILE_ENTRY_SIZE: usize = 0x8;
    pub const HASH_SIZE: usize = 0x8;
    pub const HEADER_MAGIC: u32 = 0x100;
    pub const HEADER_SIZE: usize = 0xC;
}

struct Offsets {
    name_offsets: usize,
    names: usize,
    hashes: usize,
    file_data: usize,
}

struct Header {
    hash_offset: u32,
    file_count: u32,
}

impl Header {
    #[must_use]
    fn compute_offsets(&self) -> Offsets {
        let file_count = self.file_count as usize;
        let name_offsets = constants::HEADER_SIZE + constants::FILE_ENTRY_SIZE * file_count;
        let names = name_offsets + 0x4 * file_count;
        let hashes = constants::HEADER_SIZE + self.hash_offset as usize;
        let file_data = hashes + constants::HASH_SIZE * file_count;
        Offsets {
            name_offsets,
            names,
            hashes,
            file_data,
        }
    }
}

derive::key!(Key);

impl Key {
    #[must_use]
    fn hash_in_place(name: &mut BString) -> Hash {
        hashing::hash_file_in_place(name)
    }
}

type ReadResult<T> = (T,);
derive::archive!(Archive => ReadResult, Map: Key => File);

impl<'a> Archive<'a> {
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

    fn do_read<I>(source: &mut I) -> Result<ReadResult<Self>>
    where
        I: ?Sized + Source<'a>,
    {
        let header = Self::read_header(source)?;
        let offsets = header.compute_offsets();
        let mut map = Map::default();

        for i in 0..header.file_count as usize {
            let (key, value) = Self::read_file(source, i, &offsets)?;
            map.insert(key, value);
        }

        Ok((Self { map },))
    }

    fn read_file<I>(source: &mut I, idx: usize, offsets: &Offsets) -> Result<(Key, File<'a>)>
    where
        I: ?Sized + Source<'a>,
    {
        let hash = source.save_restore_position(|source| -> Result<Hash> {
            source.seek_absolute(offsets.hashes + constants::HASH_SIZE * idx)?;
            Self::read_hash(source)
        })??;

        let name = source.save_restore_position(|source| -> Result<BString> {
            source.seek_absolute(offsets.name_offsets + 0x4 * idx)?;
            let offset: u32 = source.read(Endian::Little)?;
            source.seek_absolute(offsets.names + offset as usize)?;
            let name = source.read_protocol::<ZString>(Endian::Little)?;
            Ok(name)
        })??;

        let (size, offset): (u32, u32) = source.read(Endian::Little)?;
        let container = source.save_restore_position(|source| -> Result<ByteContainer<'a>> {
            source.seek_absolute(offsets.file_data + offset as usize)?;
            let result = source.read_container(size as usize)?;
            Ok(result)
        })??;

        Ok((Key { hash, name }, File { container }))
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
            file_count: self.map.len().try_into()?,
            hash_offset: {
                let names_offset = 0xC * self.map.len();
                let names_len: usize = self.map.keys().map(|x| x.name.len() + 1).sum();
                (names_offset + names_len).try_into()?
            },
        })
    }

    fn write_files<O>(&self, sink: &mut Sink<O>) -> Result<()>
    where
        O: Write,
    {
        let mut offset: u32 = 0;
        for file in self.map.values() {
            let size: u32 = file.container.len().try_into()?;
            sink.write(&(size, offset), Endian::Little)?;
            offset += size;
        }

        Ok(())
    }

    fn write_file_data<O>(&self, sink: &mut Sink<O>) -> Result<()>
    where
        O: Write,
    {
        for file in self.map.values() {
            sink.write_bytes(file.as_bytes())?;
        }

        Ok(())
    }

    fn write_hashes<O>(&self, sink: &mut Sink<O>) -> Result<()>
    where
        O: Write,
    {
        for key in self.map.keys() {
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
        for key in self.map.keys() {
            sink.write(&offset, Endian::Little)?;
            offset += u32::try_from(key.name.len() + 1)?;
        }

        Ok(())
    }

    fn write_names<O>(&self, sink: &mut Sink<O>) -> Result<()>
    where
        O: Write,
    {
        for key in self.map.keys() {
            sink.write_protocol::<ZString>(&key.name, Endian::Little)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        prelude::*,
        tes3::{self, Archive, ArchiveKey, Error, File, Hash},
        Borrowed,
    };
    use anyhow::Context as _;
    use bstr::{BString, ByteSlice as _};
    use memmap2::Mmap;
    use std::{
        ffi::OsStr,
        fs,
        io::{self, Read as _},
        path::Path,
    };
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
        let read_result = Archive::read(&stream);
        let test = match read_result {
            Err(Error::InvalidMagic(0x200)) => true,
            _ => false,
        };
        assert!(test);

        Ok(())
    }

    #[test]
    fn invalid_out_of_bounds() -> anyhow::Result<()> {
        let path = Path::new("data/tes3_invalid_test/invalid_exhausted.bsa");
        let stream =
            fs::File::open(&path).with_context(|| format!("failed to open file: {path:?}"))?;
        let read_result = Archive::read(&stream);
        let test = match read_result {
            Err(Error::Io(io)) if io.kind() == io::ErrorKind::UnexpectedEof => true,
            _ => false,
        };
        assert!(test);

        Ok(())
    }

    #[test]
    fn reading() -> anyhow::Result<()> {
        let root_path = Path::new("data/tes3_read_test/");
        let (archive,) = {
            let archive_path = root_path.join("test.bsa");
            let stream = fs::File::open(&archive_path)
                .with_context(|| format!("failed to open test archive: {archive_path:?}"))?;
            Archive::read(&stream)
                .with_context(|| format!("failed to read from archive: {archive_path:?}"))?
        };

        for file_path in WalkDir::new(root_path) {
            if let Ok(file_path) = file_path {
                let metadata = file_path
                    .metadata()
                    .context("failed to get file path metadata")?;
                if metadata.is_file() && file_path.path().extension() != Some(OsStr::new("bsa")) {
                    let key = file_path
                        .path()
                        .strip_prefix(root_path)
                        .with_context(|| {
                            format!(
                                "failed to strip prefix ({root_path:?}) from path ({file_path:?})"
                            )
                        })?
                        .as_os_str();
                    let file_hash = tes3::hash_file(key.as_encoded_bytes().as_bstr()).0;
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
            key: ArchiveKey,
            path: &'a Path,
        }

        impl<'a> Info<'a> {
            fn new(lo: u32, hi: u32, path: &'a str) -> Self {
                let hash = Hash { lo, hi };
                let key = ArchiveKey::from(BString::from(path));
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

        let (archive,) =
            Archive::read(Borrowed(&stream)).context("failed to read from archive in memory")?;
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
        let key = ArchiveKey::default();
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
