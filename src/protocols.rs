use crate::{
    containers::Bytes,
    io::{BinaryReadable, BinaryWriteable, Endian, Sink, Source},
};
use bstr::BStr as ByteStr;
use core::num::NonZeroU8;
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

impl<'bytes> BinaryReadable<'bytes> for BString {
    type Item = Bytes<'bytes>;

    fn from_stream<In>(stream: &mut In, endian: Endian) -> io::Result<Self::Item>
    where
        In: ?Sized + Source<'bytes>,
    {
        let len: u8 = stream.read(endian)?;
        stream.read_bytes(len.into())
    }
}

impl BinaryWriteable for BString {
    type Item = ByteStr;

    fn to_stream<Out>(stream: &mut Sink<Out>, item: &Self::Item, endian: Endian) -> io::Result<()>
    where
        Out: ?Sized + Write,
    {
        let len: Result<u8, _> = item.len().try_into();
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

pub(crate) struct ZString;

impl<'bytes> BinaryReadable<'bytes> for ZString {
    type Item = Bytes<'bytes>;

    fn from_stream<In>(stream: &mut In, endian: Endian) -> io::Result<Self::Item>
    where
        In: ?Sized + Source<'bytes>,
    {
        let start = stream.stream_position();
        let mut len = 0;
        loop {
            let byte: u8 = stream.read(endian)?;
            match byte {
                0 => break,
                _ => len += 1,
            };
        }

        stream.seek_absolute(start)?;
        let result = stream.read_bytes(len)?;
        stream.seek_relative(1)?; // skip null terminator
        Ok(result)
    }
}

impl BinaryWriteable for ZString {
    type Item = ByteStr;

    fn to_stream<Out>(stream: &mut Sink<Out>, item: &Self::Item, _: Endian) -> io::Result<()>
    where
        Out: ?Sized + Write,
    {
        stream.write_bytes(item)?;
        stream.write_bytes(b"\0")?;
        Ok(())
    }
}

pub(crate) struct BZString;

impl<'bytes> BinaryReadable<'bytes> for BZString {
    type Item = Bytes<'bytes>;

    fn from_stream<In>(stream: &mut In, endian: Endian) -> io::Result<Self::Item>
    where
        In: ?Sized + Source<'bytes>,
    {
        let len: u8 = stream.read(endian)?;
        let Some(len) = NonZeroU8::new(len) else {
            return Err(Error::MissingNullTerminator.into());
        };

        let result = stream.read_bytes((len.get() - 1).into())?;
        match stream.read(endian)? {
            b'\0' => Ok(result),
            _ => Err(Error::MissingNullTerminator.into()),
        }
    }
}

impl BinaryWriteable for BZString {
    type Item = ByteStr;

    fn to_stream<Out>(stream: &mut Sink<Out>, item: &Self::Item, endian: Endian) -> io::Result<()>
    where
        Out: ?Sized + Write,
    {
        let len: Result<u8, _> = (item.len() + 1).try_into();
        match len {
            Ok(len) => {
                stream.write(&len, endian)?;
                stream.write_bytes(item)?;
                stream.write_bytes(b"\0")?;
                Ok(())
            }
            Err(_) => Err(Error::StringTooLarge.into()),
        }
    }
}

pub(crate) struct WString;

impl<'bytes> BinaryReadable<'bytes> for WString {
    type Item = Bytes<'bytes>;

    fn from_stream<In>(stream: &mut In, endian: Endian) -> io::Result<Self::Item>
    where
        In: ?Sized + Source<'bytes>,
    {
        let len: u16 = stream.read(endian)?;
        stream.read_bytes(len.into())
    }
}

impl BinaryWriteable for WString {
    type Item = ByteStr;

    fn to_stream<Out>(stream: &mut Sink<Out>, item: &Self::Item, endian: Endian) -> io::Result<()>
    where
        Out: ?Sized + Write,
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
