use crate::{
    derive,
    tes4::{self, File, Hash},
};
use bstr::BString;

derive::key!(Key);

impl Key {
    #[must_use]
    fn hash_in_place(name: &mut BString) -> Hash {
        tes4::hash_file_in_place(name)
    }
}

derive::mapping!(Directory, Map: Key => File);

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
