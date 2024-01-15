macro_rules! reader {
    ($this:ident => $result:ident) => {
        impl<'a> crate::Reader<crate::Borrowed<'a>> for $this<'a> {
            type Error = Error;
            type Item = $result<$this<'a>>;

            fn read(source: crate::Borrowed<'a>) -> Result<Self::Item> {
                let mut source = crate::io::BorrowedSource::from(source.0);
                Self::do_read(&mut source)
            }
        }

        impl<'a> crate::Reader<crate::Copied<'a>> for $this<'static> {
            type Error = Error;
            type Item = $result<$this<'static>>;

            fn read(source: crate::Copied<'a>) -> Result<Self::Item> {
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

macro_rules! container {
    ($this:ident => $result:ident) => {
        crate::derive::reader!($this => $result);

		impl<'a> crate::Sealed for $this<'a> {}

        impl<'a> $this<'a> {
            #[must_use]
            pub fn as_bytes(&self) -> &[u8] {
                self.container.as_bytes()
            }

            #[must_use]
            pub fn as_ptr(&self) -> *const u8 {
                self.container.as_ptr()
            }

            #[must_use]
            pub fn into_owned(self) -> $this<'static> {
                $this {
                    container: self.container.into_owned(),
                }
            }

            #[must_use]
            pub fn is_empty(&self) -> bool {
                self.container.is_empty()
            }

            #[must_use]
            pub fn len(&self) -> usize {
                self.container.len()
            }

            #[must_use]
            pub fn new() -> Self {
                Self::default()
            }
        }
    };
}

pub(crate) use container;

macro_rules! key {
    ($this:ident) => {
        #[derive(::core::clone::Clone, ::core::fmt::Debug, ::core::default::Default)]
        pub struct $this {
            pub hash: Hash,
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

        impl ::core::borrow::Borrow<Hash> for $this {
            fn borrow(&self) -> &Hash {
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
    ($this:ident, $mapping:ident: $key:ty => $value:ident) => {
        pub(crate) type $mapping<'a> = ::std::collections::BTreeMap<$key, $value<'a>>;

        impl<'a> crate::Sealed for $this<'a> {}

        #[derive(::core::default::Default)]
        pub struct $this<'a> {
            pub(crate) map: $mapping<'a>,
        }

        impl<'a> $this<'a> {
            pub fn clear(&mut self) {
                self.map.clear();
            }

            #[must_use]
            pub fn get<K>(&self, key: &K) -> ::core::option::Option<&$value<'a>>
            where
                K: ::core::borrow::Borrow<Hash>,
            {
                self.map.get(key.borrow())
            }

            #[must_use]
            pub fn get_key_value<K>(&self, key: &K) -> ::core::option::Option<(&$key, &$value<'a>)>
            where
                K: ::core::borrow::Borrow<Hash>,
            {
                self.map.get_key_value(key.borrow())
            }

            #[must_use]
            pub fn get_mut<K>(&mut self, key: &K) -> ::core::option::Option<&mut $value<'a>>
            where
                K: ::core::borrow::Borrow<Hash>,
            {
                self.map.get_mut(key.borrow())
            }

            pub fn insert<K>(
                &mut self,
                key: K,
                value: $value<'a>,
            ) -> ::core::option::Option<$value<'a>>
            where
                K: ::core::convert::Into<$key>,
            {
                self.map.insert(key.into(), value)
            }

            #[must_use]
            pub fn is_empty(&self) -> bool {
                self.map.is_empty()
            }

            pub fn iter(&self) -> impl ::core::iter::Iterator<Item = (&$key, &$value<'a>)> {
                self.map.iter()
            }

            pub fn iter_mut(
                &mut self,
            ) -> impl ::core::iter::Iterator<Item = (&$key, &mut $value<'a>)> {
                self.map.iter_mut()
            }

            #[must_use]
            pub fn len(&self) -> usize {
                self.map.len()
            }

            #[must_use]
            pub fn new() -> Self {
                Self::default()
            }

            pub fn remove<K>(&mut self, key: &K) -> ::core::option::Option<$value<'a>>
            where
                K: ::core::borrow::Borrow<Hash>,
            {
                self.map.remove(key.borrow())
            }

            pub fn remove_entry<K>(&mut self, key: &K) -> ::core::option::Option<($key, $value<'a>)>
            where
                K: ::core::borrow::Borrow<Hash>,
            {
                self.map.remove_entry(key.borrow())
            }
        }

        impl<'a> ::core::iter::IntoIterator for $this<'a> {
            type Item = <$mapping<'a> as ::core::iter::IntoIterator>::Item;
            type IntoIter = <$mapping<'a> as ::core::iter::IntoIterator>::IntoIter;

            fn into_iter(self) -> Self::IntoIter {
                self.map.into_iter()
            }
        }

        impl<'a, 'b> ::core::iter::IntoIterator for &'b $this<'a> {
            type Item = <&'b $mapping<'a> as ::core::iter::IntoIterator>::Item;
            type IntoIter = <&'b $mapping<'a> as ::core::iter::IntoIterator>::IntoIter;

            fn into_iter(self) -> Self::IntoIter {
                self.map.iter()
            }
        }

        impl<'a, 'b> ::core::iter::IntoIterator for &'b mut $this<'a> {
            type Item = <&'b mut $mapping<'a> as ::core::iter::IntoIterator>::Item;
            type IntoIter = <&'b mut $mapping<'a> as ::core::iter::IntoIterator>::IntoIter;

            fn into_iter(self) -> Self::IntoIter {
                self.map.iter_mut()
            }
        }
    };
}

pub(crate) use mapping;

macro_rules! archive {
	($this:ident => $result:ident, $mapping:ident: $key:ty => $value:ident) => {
		crate::derive::mapping!($this, $mapping: $key => $value);
		crate::derive::reader!($this => $result);
	};
}

pub(crate) use archive;
