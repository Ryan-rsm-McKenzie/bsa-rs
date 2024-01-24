use crate::{derive, hashing};
use bstr::{BStr, BString};
use core::cmp::Ordering;

/// The underlying hash object used to uniquely identify objects within the archive.
#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct Hash {
    pub lo: u32,
    pub hi: u32,
}

derive::hash!(FileHash);

impl Hash {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(clippy::identity_op, clippy::erasing_op)]
    #[must_use]
    pub fn numeric(&self) -> u64 {
        (u64::from(self.hi) << (0 * 8)) | (u64::from(self.lo) << (4 * 8))
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

/// Produces a hash using the given path.
#[must_use]
pub fn hash_file(path: &BStr) -> (FileHash, BString) {
    let mut path = path.to_owned();
    (hash_file_in_place(&mut path), path)
}

/// Produces a hash using the given path.
///
/// The path is normalized in place. After the function returns, the path contains the string that would be stored on disk.
#[must_use]
pub fn hash_file_in_place(path: &mut BString) -> FileHash {
    hashing::normalize_path(path);
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

    h.into()
}

#[cfg(test)]
mod tests {
    use crate::tes3::{self, Hash};
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
        let hash = |x: &[u8]| tes3::hash_file(x.as_bstr()).0.numeric();
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
        let hash = |x: &[u8]| tes3::hash_file(x.as_bstr()).0;
        assert_eq!(hash(b"foo/bar/baz"), hash(b"foo\\bar\\baz"));
    }

    #[test]
    fn hashes_are_case_insensitive() {
        let hash = |x: &[u8]| tes3::hash_file(x.as_bstr()).0;
        assert_eq!(hash(b"FOO/BAR/BAZ"), hash(b"foo/bar/baz"));
    }

    #[test]
    fn sort_order() {
        let lhs = Hash { lo: 0, hi: 1 };
        let rhs = Hash { lo: 1, hi: 0 };
        assert!(lhs < rhs);
    }
}
