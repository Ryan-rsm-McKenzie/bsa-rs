use std::{
    io::{self, Read, Seek, SeekFrom, Write},
    mem,
};

pub enum Endian {
    Little,
    #[allow(dead_code)]
    Big,
    #[allow(dead_code)]
    Native,
}

pub trait BinaryStreamable {
    type Item;

    fn from_be_stream<R: Read>(stream: &mut R) -> io::Result<Self::Item>;
    fn from_le_stream<R: Read>(stream: &mut R) -> io::Result<Self::Item>;
    fn from_ne_stream<R: Read>(stream: &mut R) -> io::Result<Self::Item>;
    fn from_stream<R: Read>(stream: &mut R, endian: Endian) -> io::Result<Self::Item> {
        match endian {
            Endian::Big => Self::from_be_stream(stream),
            Endian::Little => Self::from_le_stream(stream),
            Endian::Native => Self::from_ne_stream(stream),
        }
    }

    fn to_be_stream<W: Write>(stream: &mut W, item: &Self::Item) -> io::Result<()>;
    fn to_le_stream<W: Write>(stream: &mut W, item: &Self::Item) -> io::Result<()>;
    fn to_ne_stream<W: Write>(stream: &mut W, item: &Self::Item) -> io::Result<()>;
    fn to_stream<W: Write>(stream: &mut W, item: &Self::Item, endian: Endian) -> io::Result<()>
    where
        Self: Sized,
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
        impl BinaryStreamable for $t {
            type Item = $t;

            fn from_be_stream<R: Read>(stream: &mut R) -> io::Result<Self::Item> {
                let mut bytes = [0u8; mem::size_of::<Self::Item>()];
                stream.read_exact(&mut bytes)?;
                Ok(Self::from_be_bytes(bytes))
            }

            fn from_le_stream<R: Read>(stream: &mut R) -> io::Result<Self::Item> {
                let mut bytes = [0u8; mem::size_of::<Self::Item>()];
                stream.read_exact(&mut bytes)?;
                Ok(Self::from_le_bytes(bytes))
            }

            fn from_ne_stream<R: Read>(stream: &mut R) -> io::Result<Self::Item> {
                let mut bytes = [0u8; mem::size_of::<Self::Item>()];
                stream.read_exact(&mut bytes)?;
                Ok(Self::from_ne_bytes(bytes))
            }

            fn to_be_stream<W: Write>(stream: &mut W, item: &Self::Item) -> io::Result<()> {
                let mut bytes = item.to_be_bytes();
                stream.write_all(&mut bytes)
            }

            fn to_le_stream<W: Write>(stream: &mut W, item: &Self::Item) -> io::Result<()> {
                let mut bytes = item.to_le_bytes();
                stream.write_all(&mut bytes)
            }

            fn to_ne_stream<W: Write>(stream: &mut W, item: &Self::Item) -> io::Result<()> {
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
        impl<$($t,)+> BinaryStreamable for ($($t,)+)
        where
            $($t: BinaryStreamable,)+
        {
            type Item = ($($t::Item,)+);

            fn from_be_stream<R: Read>(stream: &mut R) -> io::Result<Self::Item> {
                Ok(($(
                    $t::from_be_stream(stream)?,
                )+))
            }

            fn from_le_stream<R: Read>(stream: &mut R) -> io::Result<Self::Item> {
                Ok(($(
                    $t::from_le_stream(stream)?,
                )+))
            }

            fn from_ne_stream<R: Read>(stream: &mut R) -> io::Result<Self::Item> {
                Ok(($(
                    $t::from_ne_stream(stream)?,
                )+))
            }

            fn to_be_stream<W: Write>(stream: &mut W, item: &Self::Item) -> io::Result<()> {
                $(
                    $t::to_be_stream(stream, &item.$idx)?;
                )+
                Ok(())
            }

            fn to_le_stream<W: Write>(stream: &mut W, item: &Self::Item) -> io::Result<()> {
                $(
                    $t::to_le_stream(stream, &item.$idx)?;
                )+
                Ok(())
            }

            fn to_ne_stream<W: Write>(stream: &mut W, item: &Self::Item) -> io::Result<()> {
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

pub struct Source<'a, R>
where
    R: Read + Seek,
{
    stream: &'a mut R,
}

impl<'a, R> Source<'a, R>
where
    R: Read + Seek,
{
    pub fn new(stream: &'a mut R) -> Self {
        Self { stream }
    }

    pub fn read<T>(&mut self, endian: Endian) -> io::Result<T>
    where
        T: BinaryStreamable<Item = T>,
    {
        T::from_stream(&mut self.stream, endian)
    }

    pub fn read_protocol<T>(&mut self, endian: Endian) -> io::Result<T::Item>
    where
        T: BinaryStreamable,
    {
        T::from_stream(&mut self.stream, endian)
    }

    pub fn read_bytes(&mut self, bytes: &mut [u8]) -> io::Result<()> {
        self.stream.read_exact(bytes)
    }

    pub fn save_restore_position<F, T>(&mut self, f: F) -> io::Result<T>
    where
        F: FnOnce(&mut Self) -> T,
    {
        let position = self.stream.stream_position()?;
        let result = f(self);
        self.stream.seek(SeekFrom::Start(position))?;
        Ok(result)
    }

    pub fn seek(&mut self, pos: SeekFrom) -> io::Result<()> {
        self.stream.seek(pos).map(|_| ())
    }
}

pub struct Sink<'a, R>
where
    R: Write,
{
    stream: &'a mut R,
}

impl<'a, R> Sink<'a, R>
where
    R: Write,
{
    pub fn new(stream: &'a mut R) -> Self {
        Self { stream }
    }

    pub fn write<T>(&mut self, item: &T, endian: Endian) -> io::Result<()>
    where
        T: BinaryStreamable<Item = T>,
    {
        T::to_stream(&mut self.stream, item, endian)
    }

    pub fn write_protocol<T>(&mut self, item: &T::Item, endian: Endian) -> io::Result<()>
    where
        T: BinaryStreamable,
    {
        T::to_stream(&mut self.stream, item, endian)
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.stream.write_all(bytes)
    }
}
