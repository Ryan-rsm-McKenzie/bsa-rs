use crate::io::{BinaryStreamable, Endian, Source};
use bstr::BString as BinaryString;
use std::io::{self, Write};

macro_rules! streamable_boilerplate {
    () => {
        fn from_be_stream<'a, I>(stream: &mut I) -> io::Result<Self::Item>
        where
            I: ?Sized + Source<'a>,
        {
            Self::from_ne_stream(stream)
        }

        fn from_le_stream<'a, I>(stream: &mut I) -> io::Result<Self::Item>
        where
            I: ?Sized + Source<'a>,
        {
            Self::from_ne_stream(stream)
        }

        fn to_be_stream<O>(stream: &mut O, item: &Self::Item) -> io::Result<()>
        where
            O: ?Sized + Write,
        {
            Self::to_ne_stream(stream, item)
        }

        fn to_le_stream<O>(stream: &mut O, item: &Self::Item) -> io::Result<()>
        where
            O: ?Sized + Write,
        {
            Self::to_ne_stream(stream, item)
        }
    };
}

#[derive(Debug, thiserror::Error)]
enum MalformedStringError {
    #[error("postfix null-terminator was missing from read string")]
    MissingNullTerminator,

    #[error("a string is too large to be written without data loss")]
    StringTooLarge,
}

impl MalformedStringError {
    fn marshal(self) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidData, self)
    }
}

pub struct BString;

impl BinaryStreamable for BString {
    type Item = BinaryString;

    streamable_boilerplate!();

    fn from_ne_stream<'a, I>(stream: &mut I) -> io::Result<Self::Item>
    where
        I: ?Sized + Source<'a>,
    {
        let len: u8 = stream.read(Endian::Native)?;
        let mut result = Vec::new();
        result.resize_with(usize::from(len), Default::default);
        stream.read_bytes(&mut result[..])?;
        result.shrink_to_fit();
        Ok(BinaryString::new(result))
    }

    fn to_ne_stream<O>(stream: &mut O, item: &Self::Item) -> io::Result<()>
    where
        O: ?Sized + Write,
    {
        let len: Result<u8, _> = item.len().try_into();
        match len {
            Ok(len) => {
                stream.write_all(&len.to_ne_bytes())?;
                stream.write_all(&item[..])?;
                Ok(())
            }
            Err(_) => Err(MalformedStringError::StringTooLarge.marshal()),
        }
    }
}

pub struct ZString;

impl BinaryStreamable for ZString {
    type Item = BinaryString;

    streamable_boilerplate!();

    fn from_ne_stream<'a, I>(stream: &mut I) -> io::Result<Self::Item>
    where
        I: ?Sized + Source<'a>,
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
        Ok(BinaryString::new(result))
    }

    fn to_ne_stream<O>(stream: &mut O, item: &Self::Item) -> io::Result<()>
    where
        O: ?Sized + Write,
    {
        stream.write_all(&item[..])?;
        stream.write_all(b"\0")?;
        Ok(())
    }
}

pub struct BZString;

impl BinaryStreamable for BZString {
    type Item = BinaryString;

    streamable_boilerplate!();

    fn from_ne_stream<'a, I>(stream: &mut I) -> io::Result<Self::Item>
    where
        I: ?Sized + Source<'a>,
    {
        let len: u8 = stream.read(Endian::Native)?;
        if len > 0 {
            let mut result = Vec::new();
            result.resize_with(usize::from(len), Default::default);
            stream.read_bytes(&mut result[..])?;
            match result.pop() {
                Some(b'\0') => {
                    result.shrink_to_fit();
                    Ok(BinaryString::new(result))
                }
                _ => Err(MalformedStringError::MissingNullTerminator.marshal()),
            }
        } else {
            Ok(BinaryString::default())
        }
    }

    fn to_ne_stream<O>(stream: &mut O, item: &Self::Item) -> io::Result<()>
    where
        O: ?Sized + Write,
    {
        let len: Result<u8, _> = (item.len() + 1).try_into();
        match len {
            Ok(len) => {
                stream.write_all(&len.to_ne_bytes())?;
                stream.write_all(&item[..])?;
                stream.write_all(b"\0")?;
                Ok(())
            }
            Err(_) => Err(MalformedStringError::StringTooLarge.marshal()),
        }
    }
}
