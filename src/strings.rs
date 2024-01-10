use crate::io::BinaryStreamable;
use bstr::BString;
use std::io::{self, Read, Write};

pub struct ZString;

impl BinaryStreamable for ZString {
    type Item = BString;

    fn from_be_stream<R: Read>(stream: &mut R) -> io::Result<Self::Item> {
        Self::from_ne_stream(stream)
    }

    fn from_le_stream<R: Read>(stream: &mut R) -> io::Result<Self::Item> {
        Self::from_ne_stream(stream)
    }

    fn from_ne_stream<R: Read>(stream: &mut R) -> io::Result<Self::Item> {
        let mut result = Vec::<u8>::new();
        loop {
            let mut buffer = [0u8; 1];
            stream.read_exact(&mut buffer)?;
            match buffer[0] {
                0 => break,
                byte => result.push(byte),
            };
        }

        Ok(BString::new(result))
    }

    fn to_be_stream<W: Write>(stream: &mut W, item: &Self::Item) -> io::Result<()> {
        Self::to_ne_stream(stream, item)
    }

    fn to_le_stream<W: Write>(stream: &mut W, item: &Self::Item) -> io::Result<()> {
        Self::to_ne_stream(stream, item)
    }

    fn to_ne_stream<W: Write>(stream: &mut W, item: &Self::Item) -> io::Result<()> {
        stream.write_all(&item[..])?;
        let null_terminator = [0u8; 1];
        stream.write_all(&null_terminator[..])?;
        Ok(())
    }
}
