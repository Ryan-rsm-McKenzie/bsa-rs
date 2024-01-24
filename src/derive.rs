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
                Ok(self.copy_with(CompressableBytes::from_owned(
                    bytes.into(),
                    Some(self.len()),
                )))
            }

            pub fn decompress(&self, options: &$options) -> Result<$this<'static>> {
                let mut bytes = ::std::vec::Vec::new();
                self.decompress_into(&mut bytes, options)?;
                Ok(self.copy_with(CompressableBytes::from_owned(bytes.into(), None)))
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

        #[allow(clippy::needless_update)]
        impl<'bytes> crate::CompressableFrom<&'bytes [u8]> for $this<'bytes> {
            fn from_compressed(value: &'bytes [u8], decompressed_len: usize) -> Self {
                Self {
                    bytes: CompressableBytes::from_borrowed(value, Some(decompressed_len)),
                    ..Default::default()
                }
            }

            fn from_decompressed(value: &'bytes [u8]) -> Self {
                Self {
                    bytes: CompressableBytes::from_borrowed(value, None),
                    ..Default::default()
                }
            }
        }

        #[allow(clippy::needless_update)]
        impl crate::CompressableFrom<::std::boxed::Box<[u8]>> for $this<'static> {
            fn from_compressed(value: ::std::boxed::Box<[u8]>, decompressed_len: usize) -> Self {
                Self {
                    bytes: CompressableBytes::from_owned(value, Some(decompressed_len)),
                    ..Default::default()
                }
            }

            fn from_decompressed(value: ::std::boxed::Box<[u8]>) -> Self {
                Self {
                    bytes: CompressableBytes::from_owned(value, None),
                    ..Default::default()
                }
            }
        }
    };
}

pub(crate) use compressable_bytes;

macro_rules! key {
    ($this:ident: $hash:ident) => {
        #[derive(::core::clone::Clone, ::core::fmt::Debug, ::core::default::Default)]
        pub struct $this<'bytes> {
            pub(crate) hash: $hash,
            pub(crate) name: crate::containers::Bytes<'bytes>,
        }

        impl<'bytes> $this<'bytes> {
            #[must_use]
            pub fn hash(&self) -> &$hash {
                &self.hash
            }

            #[must_use]
            pub fn name(&self) -> &::bstr::BStr {
                ::bstr::BStr::new(self.name.as_bytes())
            }
        }

        // false positive
        #[allow(clippy::unconditional_recursion)]
        impl<'bytes> ::core::cmp::PartialEq for $this<'bytes> {
            fn eq(&self, other: &Self) -> bool {
                self.hash.eq(&other.hash)
            }
        }

        impl<'bytes> ::core::cmp::Eq for $this<'bytes> {}

        impl<'bytes> ::core::cmp::PartialOrd for $this<'bytes> {
            fn partial_cmp(&self, other: &Self) -> ::core::option::Option<::core::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }

        impl<'bytes> ::core::cmp::Ord for $this<'bytes> {
            fn cmp(&self, other: &Self) -> ::core::cmp::Ordering {
                self.hash.cmp(&other.hash)
            }
        }

        impl<'bytes> ::core::borrow::Borrow<$hash> for $this<'bytes> {
            fn borrow(&self) -> &$hash {
                &self.hash
            }
        }

        impl ::core::convert::From<$hash> for $this<'static> {
            fn from(value: $hash) -> Self {
                Self {
                    hash: value,
                    name: crate::containers::Bytes::default(),
                }
            }
        }

        impl<T> ::core::convert::From<T> for $this<'static>
        where
            T: ::core::convert::Into<::bstr::BString>,
        {
            fn from(value: T) -> Self {
                let mut name = value.into();
                let hash = Self::hash_in_place(&mut name);
                let v: Vec<u8> = name.into();
                Self {
                    hash,
                    name: crate::containers::Bytes::from_owned(v.into()),
                }
            }
        }
    };
}

pub(crate) use key;

macro_rules! mapping {
    ($this:ident, $mapping:ident: ($key:ident: $hash:ident) => $value:ident) => {
        pub(crate) type $mapping<'bytes> =
            ::std::collections::BTreeMap<$key<'bytes>, $value<'bytes>>;

        impl<'bytes> crate::Sealed for $this<'bytes> {}

        #[derive(::core::clone::Clone, ::core::fmt::Debug, ::core::default::Default)]
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
            ) -> ::core::option::Option<(&$key<'bytes>, &$value<'bytes>)>
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
                K: ::core::convert::Into<$key<'bytes>>,
            {
                self.map.insert(key.into(), value)
            }

            #[must_use]
            pub fn is_empty(&self) -> bool {
                self.map.is_empty()
            }

            pub fn iter(
                &self,
            ) -> impl ::core::iter::Iterator<Item = (&$key<'bytes>, &$value<'bytes>)> {
                self.map.iter()
            }

            pub fn iter_mut(
                &mut self,
            ) -> impl ::core::iter::Iterator<Item = (&$key<'bytes>, &mut $value<'bytes>)> {
                self.map.iter_mut()
            }

            pub fn keys(&self) -> impl ::core::iter::Iterator<Item = &$key<'bytes>> {
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
            ) -> ::core::option::Option<($key<'bytes>, $value<'bytes>)>
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

        impl<'bytes> ::core::iter::FromIterator<($key<'bytes>, $value<'bytes>)> for $this<'bytes> {
            fn from_iter<T>(iter: T) -> Self
            where
                T: ::core::iter::IntoIterator<Item = ($key<'bytes>, $value<'bytes>)>,
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
