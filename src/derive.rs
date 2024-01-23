macro_rules! reader {
    ($this:ident => $result:ident) => {
        impl<'bytes> crate::Reader<crate::Borrowed<'bytes>> for $this<'bytes> {
            type Error = Error;
            type Item = $result<$this<'bytes>>;

            fn read(source: crate::Borrowed<'bytes>) -> Result<Self::Item> {
                let mut source = crate::io::BorrowedSource::from(source.0);
                Self::do_read(&mut source)
            }
        }

        impl<'bytes> crate::Reader<crate::Copied<'bytes>> for $this<'static> {
            type Error = Error;
            type Item = $result<$this<'static>>;

            fn read(source: crate::Copied<'bytes>) -> Result<Self::Item> {
                let mut source = crate::io::CopiedSource::from(source.0);
                Self::do_read(&mut source)
            }
        }

        impl crate::Reader<&::std::fs::File> for $this<'static> {
            type Error = Error;
            type Item = $result<$this<'static>>;

            fn read(source: &::std::fs::File) -> Result<Self::Item> {
                let mut source = crate::io::MappedSource::try_from(source)?;
                Self::do_read(&mut source)
            }
        }

        impl crate::Reader<&::std::path::Path> for $this<'static> {
            type Error = Error;
            type Item = $result<$this<'static>>;

            fn read(source: &::std::path::Path) -> Result<Self::Item> {
                let fd = ::std::fs::File::open(source)?;
                Self::read(&fd)
            }
        }
    };
}

pub(crate) use reader;

macro_rules! reader_with_options {
    (($this:ident: $options:ident) => $result:ident) => {
        impl<'bytes> crate::ReaderWithOptions<crate::Borrowed<'bytes>> for $this<'bytes> {
            type Error = Error;
            type Item = $result<$this<'bytes>>;
            type Options = $options;

            fn read(
                source: crate::Borrowed<'bytes>,
                options: &Self::Options,
            ) -> Result<Self::Item> {
                let mut source = crate::io::BorrowedSource::from(source.0);
                Self::do_read(&mut source, options)
            }
        }

        impl<'bytes> crate::ReaderWithOptions<crate::Copied<'bytes>> for $this<'static> {
            type Error = Error;
            type Item = $result<$this<'static>>;
            type Options = $options;

            fn read(source: crate::Copied<'bytes>, options: &Self::Options) -> Result<Self::Item> {
                let mut source = crate::io::CopiedSource::from(source.0);
                Self::do_read(&mut source, options)
            }
        }

        impl crate::ReaderWithOptions<&::std::fs::File> for $this<'static> {
            type Error = Error;
            type Item = $result<$this<'static>>;
            type Options = $options;

            fn read(source: &::std::fs::File, options: &Self::Options) -> Result<Self::Item> {
                let mut source = crate::io::MappedSource::try_from(source)?;
                Self::do_read(&mut source, options)
            }
        }

        impl crate::ReaderWithOptions<&::std::path::Path> for $this<'static> {
            type Error = Error;
            type Item = $result<$this<'static>>;
            type Options = $options;

            fn read(source: &::std::path::Path, options: &Self::Options) -> Result<Self::Item> {
                let fd = ::std::fs::File::open(source)?;
                Self::read(&fd, options)
            }
        }
    };
}

pub(crate) use reader_with_options;

macro_rules! bytes {
    ($this:ident) => {
        impl<'bytes> crate::Sealed for $this<'bytes> {}

        impl<'bytes> $this<'bytes> {
            #[must_use]
            pub fn as_bytes(&self) -> &[u8] {
                self.bytes.as_bytes()
            }

            #[must_use]
            pub fn as_ptr(&self) -> *const u8 {
                self.bytes.as_ptr()
            }

            #[must_use]
            pub fn into_owned(self) -> $this<'static> {
                #[allow(clippy::needless_update)]
                $this {
                    bytes: self.bytes.into_owned(),
                    ..self
                }
            }

            #[must_use]
            pub fn is_empty(&self) -> bool {
                self.bytes.is_empty()
            }

            #[must_use]
            pub fn len(&self) -> usize {
                self.bytes.len()
            }

            #[must_use]
            pub fn new() -> Self {
                Self::default()
            }
        }
    };
}

pub(crate) use bytes;

macro_rules! compressable_bytes {
    ($this:ident: $options:ident) => {
        crate::derive::bytes!($this);

        impl<'bytes> $this<'bytes> {
            pub fn compress(&self, options: &$options) -> Result<$this<'static>> {
                let mut bytes = ::std::vec::Vec::new();
                self.compress_into(&mut bytes, options)?;
                bytes.shrink_to_fit();
                Ok(Self::from_bytes(CompressableBytes::from_owned(
                    bytes,
                    Some(self.len()),
                )))
            }

            pub fn decompress(&self, options: &$options) -> Result<$this<'static>> {
                let mut bytes = ::std::vec::Vec::new();
                self.decompress_into(&mut bytes, options)?;
                bytes.shrink_to_fit();
                Ok(Self::from_bytes(CompressableBytes::from_owned(bytes, None)))
            }

            #[must_use]
            pub fn decompressed_len(&self) -> ::core::option::Option<usize> {
                self.bytes.decompressed_len()
            }

            #[must_use]
            pub fn is_compressed(&self) -> bool {
                self.bytes.is_compressed()
            }

            #[must_use]
            pub fn is_decompressed(&self) -> bool {
                !self.is_compressed()
            }

            pub fn write<Out>(&self, stream: &mut Out, options: &$options) -> Result<()>
            where
                Out: ?::core::marker::Sized + ::std::io::Write,
            {
                if self.is_compressed() {
                    let mut bytes = ::std::vec::Vec::new();
                    self.decompress_into(&mut bytes, options)?;
                    stream.write_all(&bytes)?;
                } else {
                    stream.write_all(self.as_bytes())?;
                }

                Ok(())
            }
        }

        impl<'bytes> crate::CompressableFrom<&'bytes [u8]> for $this<'bytes> {
            fn from_compressed(value: &'bytes [u8], decompressed_len: usize) -> Self {
                Self::from_bytes(CompressableBytes::from_borrowed(
                    value,
                    Some(decompressed_len),
                ))
            }

            fn from_decompressed(value: &'bytes [u8]) -> Self {
                Self::from_bytes(CompressableBytes::from_borrowed(value, None))
            }
        }

        impl crate::CompressableFrom<::std::vec::Vec<u8>> for $this<'static> {
            fn from_compressed(value: ::std::vec::Vec<u8>, decompressed_len: usize) -> Self {
                Self::from_bytes(CompressableBytes::from_owned(value, Some(decompressed_len)))
            }

            fn from_decompressed(value: ::std::vec::Vec<u8>) -> Self {
                Self::from_bytes(CompressableBytes::from_owned(value, None))
            }
        }
    };
}

pub(crate) use compressable_bytes;

macro_rules! key {
    ($this:ident: $hash:ident) => {
        #[derive(::core::clone::Clone, ::core::fmt::Debug, ::core::default::Default)]
        pub struct $this {
            pub hash: $hash,
            pub name: ::bstr::BString,
        }

        impl ::core::cmp::PartialEq for $this {
            fn eq(&self, other: &Self) -> bool {
                self.hash.eq(&other.hash)
            }
        }

        impl ::core::cmp::Eq for $this {}

        impl ::core::cmp::PartialOrd for $this {
            fn partial_cmp(&self, other: &Self) -> ::core::option::Option<::core::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }

        impl ::core::cmp::Ord for $this {
            fn cmp(&self, other: &Self) -> ::core::cmp::Ordering {
                self.hash.cmp(&other.hash)
            }
        }

        impl ::core::borrow::Borrow<$hash> for $this {
            fn borrow(&self) -> &$hash {
                &self.hash
            }
        }

        impl<T> ::core::convert::From<T> for $this
        where
            T: Into<::bstr::BString>,
        {
            fn from(value: T) -> Self {
                let mut name = value.into();
                let hash = Self::hash_in_place(&mut name);
                Self { hash, name }
            }
        }
    };
}

pub(crate) use key;

macro_rules! mapping {
    ($this:ident, $mapping:ident: ($key:ident: $hash:ident) => $value:ident) => {
        pub(crate) type $mapping<'bytes> = ::std::collections::BTreeMap<$key, $value<'bytes>>;

        impl<'bytes> crate::Sealed for $this<'bytes> {}

        #[derive(::core::default::Default)]
        pub struct $this<'bytes> {
            pub(crate) map: $mapping<'bytes>,
        }

        impl<'bytes> $this<'bytes> {
            pub fn clear(&mut self) {
                self.map.clear();
            }

            #[must_use]
            pub fn get<K>(&self, key: &K) -> ::core::option::Option<&$value<'bytes>>
            where
                K: ::core::borrow::Borrow<$hash>,
            {
                self.map.get(key.borrow())
            }

            #[must_use]
            pub fn get_key_value<K>(
                &self,
                key: &K,
            ) -> ::core::option::Option<(&$key, &$value<'bytes>)>
            where
                K: ::core::borrow::Borrow<$hash>,
            {
                self.map.get_key_value(key.borrow())
            }

            #[must_use]
            pub fn get_mut<K>(&mut self, key: &K) -> ::core::option::Option<&mut $value<'bytes>>
            where
                K: ::core::borrow::Borrow<$hash>,
            {
                self.map.get_mut(key.borrow())
            }

            pub fn insert<K>(
                &mut self,
                key: K,
                value: $value<'bytes>,
            ) -> ::core::option::Option<$value<'bytes>>
            where
                K: ::core::convert::Into<$key>,
            {
                self.map.insert(key.into(), value)
            }

            #[must_use]
            pub fn is_empty(&self) -> bool {
                self.map.is_empty()
            }

            pub fn iter(&self) -> impl ::core::iter::Iterator<Item = (&$key, &$value<'bytes>)> {
                self.map.iter()
            }

            pub fn iter_mut(
                &mut self,
            ) -> impl ::core::iter::Iterator<Item = (&$key, &mut $value<'bytes>)> {
                self.map.iter_mut()
            }

            pub fn keys(&self) -> impl ::core::iter::Iterator<Item = &$key> {
                self.map.keys()
            }

            #[must_use]
            pub fn len(&self) -> usize {
                self.map.len()
            }

            #[must_use]
            pub fn new() -> Self {
                Self::default()
            }

            pub fn remove<K>(&mut self, key: &K) -> ::core::option::Option<$value<'bytes>>
            where
                K: ::core::borrow::Borrow<$hash>,
            {
                self.map.remove(key.borrow())
            }

            pub fn remove_entry<K>(
                &mut self,
                key: &K,
            ) -> ::core::option::Option<($key, $value<'bytes>)>
            where
                K: ::core::borrow::Borrow<$hash>,
            {
                self.map.remove_entry(key.borrow())
            }

            pub fn values(&self) -> impl ::core::iter::Iterator<Item = &$value<'bytes>> {
                self.map.values()
            }

            pub fn values_mut(
                &mut self,
            ) -> impl ::core::iter::Iterator<Item = &mut $value<'bytes>> {
                self.map.values_mut()
            }
        }

        impl<'bytes> ::core::iter::FromIterator<($key, $value<'bytes>)> for $this<'bytes> {
            fn from_iter<T>(iter: T) -> Self
            where
                T: ::core::iter::IntoIterator<Item = ($key, $value<'bytes>)>,
            {
                Self {
                    map: iter.into_iter().collect(),
                }
            }
        }

        impl<'bytes> ::core::iter::IntoIterator for $this<'bytes> {
            type Item = <$mapping<'bytes> as ::core::iter::IntoIterator>::Item;
            type IntoIter = <$mapping<'bytes> as ::core::iter::IntoIterator>::IntoIter;

            fn into_iter(self) -> Self::IntoIter {
                self.map.into_iter()
            }
        }

        impl<'bytes, 'this> ::core::iter::IntoIterator for &'this $this<'bytes> {
            type Item = <&'this $mapping<'bytes> as ::core::iter::IntoIterator>::Item;
            type IntoIter = <&'this $mapping<'bytes> as ::core::iter::IntoIterator>::IntoIter;

            fn into_iter(self) -> Self::IntoIter {
                self.map.iter()
            }
        }

        impl<'bytes, 'this> ::core::iter::IntoIterator for &'this mut $this<'bytes> {
            type Item = <&'this mut $mapping<'bytes> as ::core::iter::IntoIterator>::Item;
            type IntoIter = <&'this mut $mapping<'bytes> as ::core::iter::IntoIterator>::IntoIter;

            fn into_iter(self) -> Self::IntoIter {
                self.map.iter_mut()
            }
        }
    };
}

pub(crate) use mapping;

macro_rules! archive {
    ($this:ident => $result:ident, $mapping:ident: ($key:ident: $hash:ident) => $value:ident) => {
        crate::derive::mapping!($this, $mapping: ($key: $hash) => $value);
        crate::derive::reader!($this => $result);
    };
}

pub(crate) use archive;

macro_rules! hash {
    ($this:ident) => {
        #[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
        #[repr(transparent)]
        pub struct $this(Hash);

        impl $this {
            #[must_use]
            pub fn new() -> Self {
                Self::default()
            }
        }

        impl ::core::convert::AsMut<Hash> for $this {
            fn as_mut(&mut self) -> &mut Hash {
                &mut self.0
            }
        }

        impl ::core::convert::AsRef<Hash> for $this {
            fn as_ref(&self) -> &Hash {
                &self.0
            }
        }

        impl ::core::borrow::Borrow<Hash> for $this {
            fn borrow(&self) -> &Hash {
                &self.0
            }
        }

        impl ::core::borrow::BorrowMut<Hash> for $this {
            fn borrow_mut(&mut self) -> &mut Hash {
                &mut self.0
            }
        }

        impl ::core::ops::Deref for $this {
            type Target = Hash;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl ::core::ops::DerefMut for $this {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }

        impl ::core::convert::From<Hash> for $this {
            fn from(value: Hash) -> Self {
                Self(value)
            }
        }

        impl ::core::convert::From<$this> for Hash {
            fn from(value: $this) -> Self {
                value.0
            }
        }

        impl ::core::cmp::PartialEq<Hash> for $this {
            fn eq(&self, other: &Hash) -> bool {
                self.0.eq(other)
            }
        }

        impl ::core::cmp::PartialEq<$this> for Hash {
            fn eq(&self, other: &$this) -> bool {
                self.eq(&other.0)
            }
        }

        impl ::core::cmp::PartialOrd<Hash> for $this {
            fn partial_cmp(&self, other: &Hash) -> ::core::option::Option<::core::cmp::Ordering> {
                self.0.partial_cmp(other)
            }
        }

        impl ::core::cmp::PartialOrd<$this> for Hash {
            fn partial_cmp(&self, other: &$this) -> ::core::option::Option<::core::cmp::Ordering> {
                self.partial_cmp(&other.0)
            }
        }
    };
}

pub(crate) use hash;
