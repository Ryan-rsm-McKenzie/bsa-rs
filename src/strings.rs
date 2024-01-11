use crate::io::{BinaryStreamable, Endian, Source};
use bstr::BString;
use std::io::{self, Write};

pub struct ZString;

impl BinaryStreamable for ZString {
    type Item = BString;

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

    fn from_ne_stream<'a, I>(stream: &mut I) -> io::Result<Self::Item>
    where
        I: ?Sized + Source<'a>,
    {
        let mut result = Vec::<u8>::new();
        loop {
            let byte = stream.read::<u8>(Endian::Native)?;
            match byte {
                0 => break,
                byte => result.push(byte),
            };
        }

        Ok(BString::new(result))
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

    fn to_ne_stream<O>(stream: &mut O, item: &Self::Item) -> io::Result<()>
    where
        O: ?Sized + Write,
    {
        stream.write_all(&item[..])?;
        let null_terminator = [0u8; 1];
        stream.write_all(&null_terminator[..])?;
        Ok(())
    }
}
