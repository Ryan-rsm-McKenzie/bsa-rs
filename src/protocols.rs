use crate::io::{BinaryReadable, BinaryWriteable, Endian, Sink, Source};
use bstr::{BStr as ByteStr, BString as ByteString};
use std::io::{self, Write};

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("postfix null terminator was missing from a string")]
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

    fn from_stream<'bytes, In>(stream: &mut In, endian: Endian) -> io::Result<Self::Item>
    where
        In: ?Sized + Source<'bytes>,
    {
        let len: u8 = stream.read(endian)?;
        let mut result = Vec::new();
        result.resize_with(len.into(), Default::default);
        stream.read_into(&mut result[..])?;
        result.shrink_to_fit();
        Ok(result.into())
    }
}

impl BinaryWriteable for BString {
    type Item = ByteStr;

    fn to_stream<Out>(stream: &mut Sink<'_, Out>, item: &Self::Item, _: Endian) -> io::Result<()>
    where
        Out: Write,
    {
        let len: Result<u8, _> = item.len().try_into();
        match len {
            Ok(len) => {
                stream.write_bytes(&len.to_ne_bytes())?;
                stream.write_bytes(item)?;
                Ok(())
            }
            Err(_) => Err(Error::StringTooLarge.into()),
        }
    }
}

pub(crate) struct ZString;

impl BinaryReadable for ZString {
    type Item = ByteString;

    fn from_stream<'bytes, In>(stream: &mut In, endian: Endian) -> io::Result<Self::Item>
    where
        In: ?Sized + Source<'bytes>,
    {
        let mut result = Vec::new();
        loop {
            let byte: u8 = stream.read(endian)?;
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

    fn to_stream<Out>(stream: &mut Sink<'_, Out>, item: &Self::Item, _: Endian) -> io::Result<()>
    where
        Out: Write,
    {
        stream.write_bytes(item)?;
        stream.write_bytes(b"\0")?;
        Ok(())
    }
}

pub(crate) struct BZString;

impl BinaryReadable for BZString {
    type Item = ByteString;

    fn from_stream<'bytes, In>(stream: &mut In, endian: Endian) -> io::Result<Self::Item>
    where
        In: ?Sized + Source<'bytes>,
    {
        let len: u8 = stream.read(endian)?;
        if len > 0 {
            let mut result = Vec::new();
            result.resize_with(len.into(), Default::default);
            stream.read_into(&mut result[..])?;
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

    fn to_stream<Out>(stream: &mut Sink<'_, Out>, item: &Self::Item, _: Endian) -> io::Result<()>
    where
        Out: Write,
    {
        let len: Result<u8, _> = (item.len() + 1).try_into();
        match len {
            Ok(len) => {
                stream.write_bytes(&len.to_ne_bytes())?;
                stream.write_bytes(item)?;
                stream.write_bytes(b"\0")?;
                Ok(())
            }
            Err(_) => Err(Error::StringTooLarge.into()),
        }
    }
}

pub(crate) struct WString;

impl BinaryReadable for WString {
    type Item = ByteString;

    fn from_stream<'bytes, In>(stream: &mut In, endian: Endian) -> io::Result<Self::Item>
    where
        In: ?Sized + Source<'bytes>,
    {
        let len: u16 = stream.read(endian)?;
        let mut result = Vec::new();
        result.resize_with(len.into(), Default::default);
        stream.read_into(&mut result[..])?;
        result.shrink_to_fit();
        Ok(result.into())
    }
}

impl BinaryWriteable for WString {
    type Item = ByteStr;

    fn to_stream<Out>(
        stream: &mut Sink<'_, Out>,
        item: &Self::Item,
        endian: Endian,
    ) -> io::Result<()>
    where
        Out: Write,
    {
        let len: Result<u16, _> = item.len().try_into();
        match len {
            Ok(len) => {
                stream.write(&len, endian)?;
                stream.write_bytes(item)?;
                Ok(())
            }
            Err(_) => Err(Error::StringTooLarge.into()),
        }
    }
}
