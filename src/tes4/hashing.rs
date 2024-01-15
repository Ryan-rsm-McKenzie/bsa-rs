use crate::{cc, hashing};
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
pub fn hash_directory(path: &BStr) -> (Hash, BString) {
    let mut path = path.to_owned();
    (hash_directory_in_place(&mut path), path)
}

#[must_use]
pub fn hash_directory_in_place(path: &mut BString) -> Hash {
    hashing::normalize_path(path);
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
    let mut path = path.to_owned();
    (hash_file_in_place(&mut path), path)
}

#[must_use]
pub fn hash_file_in_place(path: &mut BString) -> Hash {
    const LUT: [u32; 6] = [
        cc::make_four(b""),
        cc::make_four(b".nif"),
        cc::make_four(b".kf"),
        cc::make_four(b".dds"),
        cc::make_four(b".wav"),
        cc::make_four(b".adp"),
    ];

    hashing::normalize_path(path);
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

        let cc = cc::make_four(extension);
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
    use crate::tes4;
    use bstr::ByteSlice as _;

    #[test]
    fn validate_directory_hashes() {
        let h = |path: &[u8]| tes4::hash_directory(path.as_bstr()).0.numeric();
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
        let h = |path: &[u8]| tes4::hash_file(path.as_bstr()).0.numeric();
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
        let empty = tes4::hash_directory(b"".as_bstr());
        let current = tes4::hash_directory(b".".as_bstr());
        assert_eq!(empty, current);
    }

    #[test]
    fn archive_tool_detects_file_extensions_incorrectly() {
        let gitignore = tes4::hash_file(b".gitignore".as_bstr()).0;
        let gitmodules = tes4::hash_file(b".gitmodules".as_bstr()).0;
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
        let h1 = tes4::hash_directory(b"C:\\foo\\bar\\baz".as_bstr()).0;
        let h2 = tes4::hash_directory(b"foo/bar/baz".as_bstr()).0;
        assert_ne!(h1, h2);
    }

    #[test]
    fn directories_longer_than_259_chars_are_equivalent_to_empty_path() {
        let long = tes4::hash_directory([0u8; 260].as_bstr()).0;
        let empty = tes4::hash_directory(b"".as_bstr()).0;
        assert_eq!(long, empty);
    }

    #[test]
    fn files_longer_than_259_chars_will_fail() {
        let good = tes4::hash_file([0u8; 259].as_bstr()).0;
        let bad = tes4::hash_file([0u8; 260].as_bstr()).0;
        assert_ne!(good.numeric(), 0);
        assert_eq!(bad.numeric(), 0)
    }

    #[test]
    fn file_extensions_longer_than_14_chars_will_fail() {
        let good = tes4::hash_file(b"test.123456789ABCDE".as_bstr()).0;
        let bad = tes4::hash_file(b"test.123456789ABCDEF".as_bstr()).0;
        assert_ne!(good.numeric(), 0);
        assert_eq!(bad.numeric(), 0);
    }

    #[test]
    fn root_paths_are_included_in_directory_names() {
        let h1 = tes4::hash_directory(b"C:\\foo\\bar\\baz".as_bstr()).0;
        let h2 = tes4::hash_directory(b"foo\\bar\\baz".as_bstr()).0;
        assert_ne!(h1, h2);
    }

    #[test]
    fn parent_directories_are_not_included_in_file_names() {
        let h1 = tes4::hash_file(b"users/john/test.txt".as_bstr()).0;
        let h2 = tes4::hash_file(b"test.txt".as_bstr()).0;
        assert_eq!(h1, h2);
    }
}
