use crate::containers::Bytes;
use core::{mem, ops::Range};
use memmap2::{Mmap, MmapOptions};
use std::{
    fs::File,
    io::{self, Write},
    sync::Arc,
};

pub(crate) enum Endian {
    Little,
    Big,
    #[allow(dead_code)]
    Native,
}

pub(crate) trait Source<'bytes> {
    fn as_bytes(&self) -> &[u8];

    fn read_bytes(&mut self, len: usize) -> io::Result<Bytes<'bytes>>;

    #[must_use]
    fn read_bytes_to_end(&mut self) -> Bytes<'bytes>;

    fn read_into(&mut self, buf: &mut [u8]) -> io::Result<()>;

    fn seek_absolute(&mut self, pos: usize) -> io::Result<()>;

    #[must_use]
    fn stream_position(&self) -> usize;

    fn read<T>(&mut self, endian: Endian) -> io::Result<T>
    where
        T: BinaryReadable<Item = T>,
    {
        T::from_stream(self, endian)
    }

    fn read_protocol<T>(&mut self, endian: Endian) -> io::Result<T::Item>
    where
        T: BinaryReadable,
    {
        T::from_stream(self, endian)
    }

    fn save_restore_position<F, T>(&mut self, f: F) -> io::Result<T>
    where
        F: FnOnce(&mut Self) -> T,
    {
        let position = self.stream_position();
        let result = f(self);
        self.seek_absolute(position)?;
        Ok(result)
    }

    fn seek_relative(&mut self, offset: isize) -> io::Result<()> {
        if let Some(pos) = self.stream_position().checked_add_signed(offset) {
            self.seek_absolute(pos)
        } else {
            Err(io::ErrorKind::UnexpectedEof.into())
        }
    }
}

macro_rules! make_sourceable {
    ($this:ty, $bytes_lifetime:lifetime $(,$this_lifetime:lifetime)?) => {
        impl $(<$this_lifetime>)? Source<$bytes_lifetime> for $this {
            fn as_bytes(&self) -> &[u8] {
                &self.source[..]
            }

            fn read_into(&mut self, buf: &mut [u8]) -> io::Result<()> {
                let len = buf.len();
                let start = self.pos;
                let stop = start + len;
                if stop > self.source.len() {
                    Err(io::ErrorKind::UnexpectedEof.into())
                } else {
                    self.pos += len;
                    buf.copy_from_slice(&self.source[start..stop]);
                    Ok(())
                }
            }

            fn read_bytes(&mut self, len: usize) -> io::Result<Bytes<$bytes_lifetime>> {
                let start = self.pos;
                let stop = start + len;
                if stop > self.source.len() {
                    Err(io::ErrorKind::UnexpectedEof.into())
                } else {
                    self.pos += len;
                    Ok(self.make_bytes(start..stop))
                }
            }

            fn read_bytes_to_end(&mut self) -> Bytes<$bytes_lifetime> {
                let len = self.source.len();
                let start = self.pos;
                let stop = len - start;
                self.make_bytes(start..stop)
            }

            fn seek_absolute(&mut self, pos: usize) -> io::Result<()> {
                if pos > self.source.len() {
                    Err(io::ErrorKind::UnexpectedEof.into())
                } else {
                    self.pos = pos;
                    Ok(())
                }
            }

            fn stream_position(&self) -> usize {
                self.pos
            }
        }
    };
}

pub(crate) struct BorrowedSource<'bytes> {
    source: &'bytes [u8],
    pos: usize,
}

impl<'bytes> BorrowedSource<'bytes> {
    #[must_use]
    fn make_bytes(&self, range: Range<usize>) -> Bytes<'bytes> {
        Bytes::from_borrowed(&self.source[range])
    }
}

impl<'bytes> From<&'bytes [u8]> for BorrowedSource<'bytes> {
    fn from(source: &'bytes [u8]) -> Self {
        Self { source, pos: 0 }
    }
}

make_sourceable!(BorrowedSource<'bytes>, 'bytes, 'bytes);

pub(crate) struct CopiedSource<'bytes> {
    source: &'bytes [u8],
    pos: usize,
}

impl<'bytes> CopiedSource<'bytes> {
    #[must_use]
    fn make_bytes(&self, range: Range<usize>) -> Bytes<'static> {
        Bytes::from_owned(self.source[range].into())
    }
}

impl<'bytes> From<&'bytes [u8]> for CopiedSource<'bytes> {
    fn from(source: &'bytes [u8]) -> Self {
        Self { source, pos: 0 }
    }
}

make_sourceable!(CopiedSource<'bytes>, 'static, 'bytes);

pub(crate) struct MappedSource {
    source: Arc<Mmap>,
    pos: usize,
}

impl MappedSource {
    #[must_use]
    fn make_bytes(&self, range: Range<usize>) -> Bytes<'static> {
        Bytes::from_mapped(range.start, range.len(), self.source.clone())
    }
}

impl TryFrom<&File> for MappedSource {
    type Error = io::Error;

    fn try_from(value: &File) -> Result<Self, Self::Error> {
        let options = MmapOptions::new();
        let mapping = unsafe { options.map(value) }?;
        Ok(Self {
            source: Arc::new(mapping),
            pos: 0,
        })
    }
}

make_sourceable!(MappedSource, 'static);

pub(crate) trait BinaryReadable {
    type Item;

    fn from_ne_stream<'bytes, In>(stream: &mut In) -> io::Result<Self::Item>
    where
        In: ?Sized + Source<'bytes>;

    fn from_be_stream<'bytes, In>(stream: &mut In) -> io::Result<Self::Item>
    where
        In: ?Sized + Source<'bytes>,
    {
        Self::from_ne_stream(stream)
    }

    fn from_le_stream<'bytes, In>(stream: &mut In) -> io::Result<Self::Item>
    where
        In: ?Sized + Source<'bytes>,
    {
        Self::from_ne_stream(stream)
    }

    fn from_stream<'bytes, In>(stream: &mut In, endian: Endian) -> io::Result<Self::Item>
    where
        In: ?Sized + Source<'bytes>,
    {
        match endian {
            Endian::Big => Self::from_be_stream(stream),
            Endian::Little => Self::from_le_stream(stream),
            Endian::Native => Self::from_ne_stream(stream),
        }
    }
}

pub(crate) trait BinaryWriteable {
    type Item: ?Sized;

    fn to_ne_stream<Out>(stream: &mut Out, item: &Self::Item) -> io::Result<()>
    where
        Out: ?Sized + Write;

    fn to_be_stream<Out>(stream: &mut Out, item: &Self::Item) -> io::Result<()>
    where
        Out: ?Sized + Write,
    {
        Self::to_ne_stream(stream, item)
    }

    fn to_le_stream<Out>(stream: &mut Out, item: &Self::Item) -> io::Result<()>
    where
        Out: ?Sized + Write,
    {
        Self::to_ne_stream(stream, item)
    }

    fn to_stream<Out>(stream: &mut Out, item: &Self::Item, endian: Endian) -> io::Result<()>
    where
        Out: ?Sized + Write,
    {
        match endian {
            Endian::Big => Self::to_be_stream(stream, item),
            Endian::Little => Self::to_le_stream(stream, item),
            Endian::Native => Self::to_ne_stream(stream, item),
        }
    }
}

macro_rules! make_binary_streamable {
    ($t:ty) => {
        impl BinaryReadable for $t {
            type Item = $t;

            fn from_be_stream<'bytes, In>(stream: &mut In) -> io::Result<Self::Item>
            where
                In: ?Sized + Source<'bytes>,
            {
                let mut bytes = [0u8; mem::size_of::<Self::Item>()];
                stream.read_into(&mut bytes)?;
                Ok(Self::from_be_bytes(bytes))
            }

            fn from_le_stream<'bytes, In>(stream: &mut In) -> io::Result<Self::Item>
            where
                In: ?Sized + Source<'bytes>,
            {
                let mut bytes = [0u8; mem::size_of::<Self::Item>()];
                stream.read_into(&mut bytes)?;
                Ok(Self::from_le_bytes(bytes))
            }

            fn from_ne_stream<'bytes, In>(stream: &mut In) -> io::Result<Self::Item>
            where
                In: ?Sized + Source<'bytes>,
            {
                let mut bytes = [0u8; mem::size_of::<Self::Item>()];
                stream.read_into(&mut bytes)?;
                Ok(Self::from_ne_bytes(bytes))
            }
        }

        impl BinaryWriteable for $t {
            type Item = $t;

            fn to_be_stream<Out>(stream: &mut Out, item: &Self::Item) -> io::Result<()>
            where
                Out: ?Sized + Write,
            {
                let mut bytes = item.to_be_bytes();
                stream.write_all(&mut bytes)
            }

            fn to_le_stream<Out>(stream: &mut Out, item: &Self::Item) -> io::Result<()>
            where
                Out: ?Sized + Write,
            {
                let mut bytes = item.to_le_bytes();
                stream.write_all(&mut bytes)
            }

            fn to_ne_stream<Out>(stream: &mut Out, item: &Self::Item) -> io::Result<()>
            where
                Out: ?Sized + Write,
            {
                let mut bytes = item.to_ne_bytes();
                stream.write_all(&mut bytes)
            }
        }
    };
}

make_binary_streamable!(u8);
make_binary_streamable!(u16);
make_binary_streamable!(u32);
make_binary_streamable!(u64);

make_binary_streamable!(i8);
make_binary_streamable!(i16);
make_binary_streamable!(i32);
make_binary_streamable!(i64);

macro_rules! make_binary_streamable_tuple {
    ($($idx:tt $t:ident),+) => {
        impl<$($t,)+> BinaryReadable for ($($t,)+)
        where
            $($t: BinaryReadable,)+
        {
            type Item = ($($t::Item,)+);

            fn from_be_stream<'bytes, In>(stream: &mut In) -> io::Result<Self::Item>
            where
                In: ?Sized + Source<'bytes>,
            {
                Ok(($(
                    $t::from_be_stream(stream)?,
                )+))
            }

            fn from_le_stream<'bytes, In>(stream: &mut In) -> io::Result<Self::Item>
            where
                In: ?Sized + Source<'bytes>,
            {
                Ok(($(
                    $t::from_le_stream(stream)?,
                )+))
            }

            fn from_ne_stream<'bytes, In>(stream: &mut In) -> io::Result<Self::Item>
            where
                In: ?Sized + Source<'bytes>,
            {
                Ok(($(
                    $t::from_ne_stream(stream)?,
                )+))
            }
        }

        impl<$($t,)+> BinaryWriteable for ($($t,)+)
        where
            $($t: BinaryWriteable, <$t as BinaryWriteable>::Item: Sized,)+
        {
            type Item = ($($t::Item,)+);

            fn to_be_stream<Out>(stream: &mut Out, item: &Self::Item) -> io::Result<()>
            where
                Out: ?Sized + Write,
            {
                $(
                    $t::to_be_stream(stream, &item.$idx)?;
                )+
                Ok(())
            }

            fn to_le_stream<Out>(stream: &mut Out, item: &Self::Item) -> io::Result<()>
            where
                Out: ?Sized + Write,
            {
                $(
                    $t::to_le_stream(stream, &item.$idx)?;
                )+
                Ok(())
            }

            fn to_ne_stream<Out>(stream: &mut Out, item: &Self::Item) -> io::Result<()>
            where
                Out: ?Sized + Write,
            {
                $(
                    $t::to_ne_stream(stream, &item.$idx)?;
                )+
                Ok(())
            }
        }
    };
}

make_binary_streamable_tuple!(0 T0);
make_binary_streamable_tuple!(0 T0, 1 T1);
make_binary_streamable_tuple!(0 T0, 1 T1, 2 T2);
make_binary_streamable_tuple!(0 T0, 1 T1, 2 T2, 3 T3);
make_binary_streamable_tuple!(0 T0, 1 T1, 2 T2, 3 T3, 4 T4);
make_binary_streamable_tuple!(0 T0, 1 T1, 2 T2, 3 T3, 4 T4, 5 T5);
make_binary_streamable_tuple!(0 T0, 1 T1, 2 T2, 3 T3, 4 T4, 5 T5, 6 T6);
make_binary_streamable_tuple!(0 T0, 1 T1, 2 T2, 3 T3, 4 T4, 5 T5, 6 T6, 7 T7);
make_binary_streamable_tuple!(0 T0, 1 T1, 2 T2, 3 T3, 4 T4, 5 T5, 6 T6, 7 T7, 8 T8);
make_binary_streamable_tuple!(0 T0, 1 T1, 2 T2, 3 T3, 4 T4, 5 T5, 6 T6, 7 T7, 8 T8, 9 T9);

pub(crate) struct Sink<'stream, Out>
where
    Out: Write,
{
    stream: &'stream mut Out,
}

impl<'stream, Out> Sink<'stream, Out>
where
    Out: Write,
{
    #[must_use]
    pub(crate) fn new(stream: &'stream mut Out) -> Self {
        Self { stream }
    }

    pub(crate) fn write<T>(&mut self, item: &T, endian: Endian) -> io::Result<()>
    where
        T: BinaryWriteable<Item = T>,
    {
        T::to_stream(&mut self.stream, item, endian)
    }

    pub(crate) fn write_protocol<T>(&mut self, item: &T::Item, endian: Endian) -> io::Result<()>
    where
        T: BinaryWriteable,
    {
        T::to_stream(&mut self.stream, item, endian)
    }

    pub(crate) fn write_bytes(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.stream.write_all(bytes)
    }
}
