use crate::{
    containers::ByteContainer,
    derive,
    io::Source,
    tes3::{Error, Result},
};
use std::io::Write;

#[derive(Default)]
pub struct File<'a> {
    pub(crate) container: ByteContainer<'a>,
}

type ReadResult<T> = T;
derive::container!(File => ReadResult);

impl<'a> File<'a> {
    pub fn write<O>(&self, stream: &mut O) -> Result<()>
    where
        O: ?Sized + Write,
    {
        stream.write_all(self.as_bytes())?;
        Ok(())
    }

    #[allow(clippy::unnecessary_wraps)]
    fn do_read<I>(stream: &mut I) -> Result<ReadResult<Self>>
    where
        I: ?Sized + Source<'a>,
    {
        Ok(Self {
            container: stream.read_to_end(),
        })
    }
}

impl<'a> From<&'a [u8]> for File<'a> {
    fn from(value: &'a [u8]) -> Self {
        Self {
            container: ByteContainer::from_borrowed(value),
        }
    }
}

impl From<Vec<u8>> for File<'static> {
    fn from(value: Vec<u8>) -> Self {
        Self {
            container: ByteContainer::from_owned(value),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::tes3::File;

    #[test]
    fn default_state() {
        let f = File::new();
        assert!(f.is_empty());
        assert!(f.len() == 0);
        assert!(f.as_bytes().is_empty());
    }
}
