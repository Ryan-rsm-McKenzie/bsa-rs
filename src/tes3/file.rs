use crate::{
    containers::Bytes,
    derive,
    io::Source,
    tes3::{Error, Result},
};
use std::io::Write;

#[derive(Default)]
pub struct File<'bytes> {
    pub(crate) bytes: Bytes<'bytes>,
}

type ReadResult<T> = T;
derive::bytes!(File => ReadResult);

impl<'bytes> File<'bytes> {
    pub fn write<Out>(&self, stream: &mut Out) -> Result<()>
    where
        Out: ?Sized + Write,
    {
        stream.write_all(self.as_bytes())?;
        Ok(())
    }

    #[allow(clippy::unnecessary_wraps)]
    fn do_read<In>(stream: &mut In) -> Result<ReadResult<Self>>
    where
        In: ?Sized + Source<'bytes>,
    {
        Ok(Self {
            bytes: stream.read_bytes_to_end(),
        })
    }
}

impl<'bytes> From<&'bytes [u8]> for File<'bytes> {
    fn from(value: &'bytes [u8]) -> Self {
        Self {
            bytes: Bytes::from_borrowed(value),
        }
    }
}

impl From<Vec<u8>> for File<'static> {
    fn from(value: Vec<u8>) -> Self {
        Self {
            bytes: Bytes::from_owned(value),
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
