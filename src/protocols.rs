use crate::io::{BinaryReadable, BinaryWriteable, Endian, Source};
use bstr::{BStr as ByteStr, BString as ByteString};
use std::io::{self, Write};

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("postfix null-terminator was missing from read string")]
    MissingNullTerminator,

    #[error("a string is too large to be written without data loss")]
    StringTooLarge,
}

impl From<Error> for io::Error {
    fn from(value: Error) -> Self {
        Self::new(io::ErrorKind::InvalidData, value)
    }
}

pub(crate) struct BString;

impl BinaryReadable for BString {
    type Item = ByteString;

    fn from_ne_stream<'bytes, In>(stream: &mut In) -> io::Result<Self::Item>
    where
        In: ?Sized + Source<'bytes>,
    {
        let len: u8 = stream.read(Endian::Native)?;
        let mut result = Vec::new();
        result.resize_with(usize::from(len), Default::default);
        stream.read_bytes(&mut result[..])?;
        result.shrink_to_fit();
        Ok(result.into())
    }
}

impl BinaryWriteable for BString {
    type Item = ByteStr;

    fn to_ne_stream<Out>(stream: &mut Out, item: &Self::Item) -> io::Result<()>
    where
        Out: ?Sized + Write,
    {
        let len: Result<u8, _> = item.len().try_into();
        match len {
            Ok(len) => {
                stream.write_all(&len.to_ne_bytes())?;
                stream.write_all(item)?;
                Ok(())
            }
            Err(_) => Err(Error::StringTooLarge.into()),
        }
    }
}

pub(crate) struct ZString;

impl BinaryReadable for ZString {
    type Item = ByteString;

    fn from_ne_stream<'bytes, In>(stream: &mut In) -> io::Result<Self::Item>
    where
        In: ?Sized + Source<'bytes>,
    {
        let mut result = Vec::new();
        loop {
            let byte: u8 = stream.read(Endian::Native)?;
            match byte {
                0 => break,
                byte => result.push(byte),
            };
        }

        result.shrink_to_fit();
        Ok(result.into())
    }
}

impl BinaryWriteable for ZString {
    type Item = ByteStr;

    fn to_ne_stream<Out>(stream: &mut Out, item: &Self::Item) -> io::Result<()>
    where
        Out: ?Sized + Write,
    {
        stream.write_all(item)?;
        stream.write_all(b"\0")?;
        Ok(())
    }
}

pub(crate) struct BZString;

impl BinaryReadable for BZString {
    type Item = ByteString;

    fn from_ne_stream<'bytes, In>(stream: &mut In) -> io::Result<Self::Item>
    where
        In: ?Sized + Source<'bytes>,
    {
        let len: u8 = stream.read(Endian::Native)?;
        if len > 0 {
            let mut result = Vec::new();
            result.resize_with(usize::from(len), Default::default);
            stream.read_bytes(&mut result[..])?;
            match result.pop() {
                Some(b'\0') => {
                    result.shrink_to_fit();
                    Ok(result.into())
                }
                _ => Err(Error::MissingNullTerminator.into()),
            }
        } else {
            Ok(Self::Item::default())
        }
    }
}

impl BinaryWriteable for BZString {
    type Item = ByteStr;

    fn to_ne_stream<Out>(stream: &mut Out, item: &Self::Item) -> io::Result<()>
    where
        Out: ?Sized + Write,
    {
        let len: Result<u8, _> = (item.len() + 1).try_into();
        match len {
            Ok(len) => {
                stream.write_all(&len.to_ne_bytes())?;
                stream.write_all(item)?;
                stream.write_all(b"\0")?;
                Ok(())
            }
            Err(_) => Err(Error::StringTooLarge.into()),
        }
    }
}
