use crate::{
    derive,
    tes4::{self, File, FileHash},
};
use bstr::BString;

derive::key!(Key: FileHash);

impl Key {
    #[must_use]
    fn hash_in_place(name: &mut BString) -> FileHash {
        tes4::hash_file_in_place(name)
    }
}

derive::mapping!(Directory, Map: (Key: FileHash) => File);

#[cfg(test)]
mod tests {
    use crate::tes4::Directory;

    #[test]
    fn default_state() {
        let d = Directory::new();
        assert!(d.is_empty());
        assert!(d.len() == 0);
    }
}
