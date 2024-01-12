use crate::{containers::CompressableByteContainer, derive, io::Source, CompressableFrom};
use bstr::BString;
use core::num::TryFromIntError;
use flate2::{
    write::{ZlibDecoder, ZlibEncoder},
    Compression,
};
use lzzzz::lz4f::{self, AutoFlush, PreferencesBuilder};
use std::io::{self, Write};

pub mod errors {
    use core::fmt::{self, Display, Formatter};
    use std::error;

    #[derive(Clone, Copy, Debug)]
    pub struct DecompressionSizeMismatch {
        pub expected: usize,
        pub actual: usize,
    }

    impl Display for DecompressionSizeMismatch {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "buffer failed to decompress to the expected size... expected {} bytes, but got {} bytes", self.expected, self.actual)
        }
    }

    impl error::Error for DecompressionSizeMismatch {}
}

use errors::DecompressionSizeMismatch;

#[non_exhaustive]
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("can not compress the given file because it is already compressed")]
    AlreadyCompressed,

    #[error("can not decompress the given file because it is already decompressed")]
    AlreadyDecompressed,

    #[error(transparent)]
    DecompressionSizeMismatch(#[from] errors::DecompressionSizeMismatch),

    #[error(transparent)]
    IntegralTruncation(#[from] TryFromIntError),

    #[error(transparent)]
    Io(#[from] io::Error),

    #[error(transparent)]
    LZ4(#[from] lz4f::Error),
}

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CompressionCodec {
    #[default]
    Normal,
    //XMem,
}

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub enum Version {
    #[default]
    TES4 = 103,
    FO3 = 104,
    SSE = 105,
}

impl Version {
    pub const FNV: Version = Version::FO3;
    pub const TES5: Version = Version::FO3;
}

pub mod hashing {
    use crate::hashing as detail;
    use bstr::{BStr, BString, ByteSlice as _};
    use core::cmp::Ordering;

    #[derive(Clone, Copy, Debug, Default)]
    #[repr(C)]
    pub struct Hash {
        pub last: u8,
        pub last2: u8,
        pub length: u8,
        pub first: u8,
        pub crc: u32,
    }

    impl Hash {
        #[must_use]
        pub fn new() -> Self {
            Self::default()
        }

        #[allow(clippy::identity_op, clippy::erasing_op)]
        #[must_use]
        pub fn numeric(&self) -> u64 {
            (u64::from(self.last) << (0 * 8))
                | (u64::from(self.last2) << (1 * 8))
                | (u64::from(self.length) << (2 * 8))
                | (u64::from(self.first) << (3 * 8))
                | (u64::from(self.crc) << (4 * 8))
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

    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc: u32 = 0;
        for &b in bytes {
            crc = u32::from(b).wrapping_add(crc.wrapping_mul(0x1003F));
        }
        crc
    }

    #[must_use]
    const fn make_four_cc(cc: &[u8]) -> u32 {
        let buffer = match cc.len() {
            0 => [0, 0, 0, 0],
            1 => [cc[0], 0, 0, 0],
            2 => [cc[0], cc[1], 0, 0],
            3 => [cc[0], cc[1], cc[2], 0],
            _ => [cc[0], cc[1], cc[2], cc[3]],
        };
        u32::from_le_bytes(buffer)
    }

    #[must_use]
    pub fn hash_directory(path: &BStr) -> (Hash, BString) {
        let mut path = BString::new(path.to_vec());
        (hash_directory_in_place(&mut path), path)
    }

    #[must_use]
    pub fn hash_directory_in_place(path: &mut BString) -> Hash {
        detail::normalize_path(path);
        let mut h = Hash::new();
        let len = path.len();
        if len >= 3 {
            h.last2 = path[len - 2];
        }
        if len >= 1 {
            h.last = path[len - 1];
            h.first = path[0];
        }

        // truncation here is intentional, this is how bethesda does it
        #[allow(clippy::cast_possible_truncation)]
        {
            h.length = len as u8;
        }

        if h.length > 3 {
            // skip first and last two chars -> already processed
            h.crc = crc32(&path[1..len - 2]);
        }

        h
    }

    #[must_use]
    pub fn hash_file(path: &BStr) -> (Hash, BString) {
        let mut path = BString::new(path.to_vec());
        (hash_file_in_place(&mut path), path)
    }

    #[must_use]
    pub fn hash_file_in_place(path: &mut BString) -> Hash {
        const LUT: [u32; 6] = [
            make_four_cc(b""),
            make_four_cc(b".nif"),
            make_four_cc(b".kf"),
            make_four_cc(b".dds"),
            make_four_cc(b".wav"),
            make_four_cc(b".adp"),
        ];

        detail::normalize_path(path);
        if let Some(pos) = path.iter().rposition(|&x| x == b'\\') {
            path.drain(..=pos);
        }

        let path: &_ = path;
        let (stem, extension) = if let Some(split_at) = path.iter().rposition(|&x| x == b'.') {
            (&path[..split_at], &path[split_at..])
        } else {
            (&path[..], b"".as_slice())
        };

        if !stem.is_empty() && stem.len() < 260 && extension.len() < 16 {
            let mut h = hash_directory(stem.as_bstr()).0;
            h.crc = u32::wrapping_add(h.crc, crc32(extension));

            let cc = make_four_cc(extension);
            // truncations are on purpose
            #[allow(clippy::cast_possible_truncation)]
            if let Some(i) = LUT.iter().position(|&x| x == cc) {
                let i = i as u8;
                h.first = u32::from(h.first).wrapping_add(32 * u32::from(i & 0xFC)) as u8;
                h.last = u32::from(h.last).wrapping_add(u32::from(i & 0xFE) << 6) as u8;
                h.last2 = u32::from(h.last2).wrapping_add(u32::from(i.wrapping_shl(7))) as u8;
            }

            h
        } else {
            Hash::default()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{hash_directory, hash_file, make_four_cc};
        use bstr::ByteSlice as _;

        #[test]
        fn four_cc() -> anyhow::Result<()> {
            assert_eq!(make_four_cc(b""), 0x00000000);
            assert_eq!(make_four_cc(b"A"), 0x00000041);
            assert_eq!(make_four_cc(b"AB"), 0x00004241);
            assert_eq!(make_four_cc(b"ABC"), 0x00434241);
            assert_eq!(make_four_cc(b"ABCD"), 0x44434241);
            assert_eq!(make_four_cc(b"ABCDE"), 0x44434241);
            Ok(())
        }

        #[test]
        fn validate_directory_hashes() {
            let h = |path: &[u8]| hash_directory(path.as_bstr()).0.numeric();
            assert_eq!(
                h(b"textures/armor/amuletsandrings/elder council"),
                0x04BC422C742C696C
            );
            assert_eq!(
                h(b"sound/voice/skyrim.esm/maleuniquedbguardian"),
                0x594085AC732B616E
            );
            assert_eq!(h(b"textures/architecture/windhelm"), 0xC1D97EBE741E6C6D);
        }

        #[test]
        fn validate_file_hashes() {
            let h = |path: &[u8]| hash_file(path.as_bstr()).0.numeric();
            assert_eq!(h(b"darkbrotherhood__0007469a_1.fuz"), 0x011F11B0641B5F31);
            assert_eq!(h(b"elder_council_amulet_n.dds"), 0xDC531E2F6516DFEE);
            assert_eq!(
                h(b"testtoddquest_testtoddhappy_00027fa2_1.mp3"),
                0xDE0301EE74265F31
            );
            assert_eq!(h(b"Mar\xEDa_F.fuz"), 0x690E07826D075F66);
        }

        #[test]
        fn empty_path_equivalent_to_current_path() {
            let empty = hash_directory(b"".as_bstr());
            let current = hash_directory(b".".as_bstr());
            assert_eq!(empty, current);
        }

        #[test]
        fn archive_tool_detects_file_extensions_incorrectly() {
            let gitignore = hash_file(b".gitignore".as_bstr()).0;
            let gitmodules = hash_file(b".gitmodules".as_bstr()).0;
            assert_eq!(gitignore, gitmodules);
            assert_eq!(gitignore.first, b'\0');
            assert_eq!(gitignore.last2, b'\0');
            assert_eq!(gitignore.last, b'\0');
            assert_eq!(gitignore.length, 0);
            assert_eq!(gitignore.crc, 0);
            assert_eq!(gitignore.numeric(), 0);
        }

        #[test]
        fn root_paths_are_included_in_hashes() {
            let h1 = hash_directory(b"C:\\foo\\bar\\baz".as_bstr()).0;
            let h2 = hash_directory(b"foo/bar/baz".as_bstr()).0;
            assert_ne!(h1, h2);
        }

        #[test]
        fn directories_longer_than_259_chars_are_equivalent_to_empty_path() {
            let long = hash_directory([0u8; 260].as_bstr()).0;
            let empty = hash_directory(b"".as_bstr()).0;
            assert_eq!(long, empty);
        }

        #[test]
        fn files_longer_than_259_chars_will_fail() {
            let good = hash_file([0u8; 259].as_bstr()).0;
            let bad = hash_file([0u8; 260].as_bstr()).0;
            assert_ne!(good.numeric(), 0);
            assert_eq!(bad.numeric(), 0)
        }

        #[test]
        fn file_extensions_longer_than_14_chars_will_fail() {
            let good = hash_file(b"test.123456789ABCDE".as_bstr()).0;
            let bad = hash_file(b"test.123456789ABCDEF".as_bstr()).0;
            assert_ne!(good.numeric(), 0);
            assert_eq!(bad.numeric(), 0);
        }

        #[test]
        fn root_paths_are_included_in_directory_names() {
            let h1 = hash_directory(b"C:\\foo\\bar\\baz".as_bstr()).0;
            let h2 = hash_directory(b"foo\\bar\\baz".as_bstr()).0;
            assert_ne!(h1, h2);
        }

        #[test]
        fn parent_directories_are_not_included_in_file_names() {
            let h1 = hash_file(b"users/john/test.txt".as_bstr()).0;
            let h2 = hash_file(b"test.txt".as_bstr()).0;
            assert_eq!(h1, h2);
        }
    }
}

use hashing::Hash;

#[derive(Clone, Copy, Default)]
pub struct CompressionOptions {
    version: Version,
    compression_codec: CompressionCodec,
}

impl CompressionOptions {
    #[must_use]
    pub fn build(self) -> Self {
        self
    }

    #[must_use]
    pub fn compression_codec(&mut self, compression_codec: CompressionCodec) -> &mut Self {
        self.compression_codec = compression_codec;
        self
    }

    #[must_use]
    pub fn version(&mut self, version: Version) -> &mut Self {
        self.version = version;
        self
    }
}

#[derive(Default)]
pub struct File<'a> {
    container: CompressableByteContainer<'a>,
}

derive::container!(File);

impl<'a> File<'a> {
    pub fn compress(&self, options: CompressionOptions) -> Result<File<'static>> {
        let mut bytes = Vec::new();
        self.compress_into(&mut bytes, options)?;
        bytes.shrink_to_fit();
        Ok(File {
            container: CompressableByteContainer::from_owned(bytes, Some(self.len())),
        })
    }

    pub fn compress_into(&self, out: &mut Vec<u8>, options: CompressionOptions) -> Result<()> {
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

    pub fn decompress(&self, options: CompressionOptions) -> Result<File<'static>> {
        let mut bytes = Vec::new();
        self.decompress_into(&mut bytes, options)?;
        bytes.shrink_to_fit();
        Ok(File {
            container: CompressableByteContainer::from_owned(bytes, None),
        })
    }

    pub fn decompress_into(&self, out: &mut Vec<u8>, options: CompressionOptions) -> Result<()> {
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
            Err(Error::from(DecompressionSizeMismatch {
                expected: decompressed_len,
                actual: out_len,
            }))
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

    pub fn write<O>(&self, stream: &mut O, options: CompressionOptions) -> Result<()>
    where
        O: ?Sized + Write,
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

    #[must_use]
    fn do_read<I>(stream: &mut I) -> Result<Self>
    where
        I: ?Sized + Source<'a>,
    {
        Ok(Self {
            container: stream.read_to_end().into_compressable(None),
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

impl<'a> CompressableFrom<&'a [u8]> for File<'a> {
    fn from_compressed(value: &'a [u8], decompressed_len: usize) -> Self {
        Self {
            container: CompressableByteContainer::from_borrowed(value, Some(decompressed_len)),
        }
    }

    fn from_decompressed(value: &'a [u8]) -> Self {
        Self {
            container: CompressableByteContainer::from_borrowed(value, None),
        }
    }
}

impl CompressableFrom<Vec<u8>> for File<'static> {
    fn from_compressed(value: Vec<u8>, decompressed_len: usize) -> Self {
        Self {
            container: CompressableByteContainer::from_owned(value, Some(decompressed_len)),
        }
    }

    fn from_decompressed(value: Vec<u8>) -> Self {
        Self {
            container: CompressableByteContainer::from_owned(value, None),
        }
    }
}

derive::key!(DirectoryKey);

impl DirectoryKey {
    #[must_use]
    fn hash_in_place(name: &mut BString) -> Hash {
        hashing::hash_directory_in_place(name)
    }
}

derive::mapping!(Directory, DirectoryMap: DirectoryKey => File);

derive::key!(ArchiveKey);

impl ArchiveKey {
    #[must_use]
    fn hash_in_place(name: &mut BString) -> Hash {
        hashing::hash_file_in_place(name)
    }
}

derive::archive!(Archive, ArchiveMap: ArchiveKey => Directory);

impl<'a> Archive<'a> {
    fn do_read<I>(source: &mut I) -> Result<Self>
    where
        I: ?Sized + Source<'a>,
    {
        todo!()
    }
}
