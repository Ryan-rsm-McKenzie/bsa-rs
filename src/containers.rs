use memmap2::Mmap;
use std::sync::Arc;

struct Mapping {
    pos: usize,
    len: usize,
    mapping: Arc<Mmap>,
}

impl Mapping {
    pub fn as_bytes(&self) -> &[u8] {
        &self.mapping[self.pos..self.pos + self.len]
    }

    pub fn as_ptr(&self) -> *const u8 {
        self.as_bytes().as_ptr()
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn len(&self) -> usize {
        self.len
    }
}

enum ByteContainerInner<'a> {
    Owned(Vec<u8>),
    Borrowed(&'a [u8]),
    Mapped(Mapping),
}

use ByteContainerInner::*;

pub struct ByteContainer<'a> {
    inner: ByteContainerInner<'a>,
}

impl<'a> ByteContainer<'a> {
    pub fn as_bytes(&self) -> &[u8] {
        match &self.inner {
            Owned(x) => x,
            Borrowed(x) => x,
            Mapped(x) => x.as_bytes(),
        }
    }

    pub fn as_ptr(&self) -> *const u8 {
        match &self.inner {
            Owned(x) => x.as_ptr(),
            Borrowed(x) => x.as_ptr(),
            Mapped(x) => x.as_ptr(),
        }
    }

    pub fn from_borrowed(bytes: &'a [u8]) -> Self {
        Self {
            inner: Borrowed(bytes),
        }
    }

    pub fn is_empty(&self) -> bool {
        match &self.inner {
            Owned(x) => x.is_empty(),
            Borrowed(x) => x.is_empty(),
            Mapped(x) => x.is_empty(),
        }
    }

    pub fn len(&self) -> usize {
        match &self.inner {
            Owned(x) => x.len(),
            Borrowed(x) => x.len(),
            Mapped(x) => x.len(),
        }
    }

    pub fn into_owned(self) -> ByteContainer<'static> {
        ByteContainer {
            inner: match self.inner {
                Owned(x) => Owned(x),
                Borrowed(x) => Owned(x.to_owned()),
                Mapped(x) => Mapped(x),
            },
        }
    }
}

impl ByteContainer<'static> {
    pub fn from_owned(bytes: Vec<u8>) -> Self {
        Self {
            inner: Owned(bytes),
        }
    }

    pub fn from_mapped(pos: usize, len: usize, mapping: Arc<Mmap>) -> Self {
        Self {
            inner: Mapped(Mapping { pos, len, mapping }),
        }
    }
}

impl<'a> Default for ByteContainer<'a> {
    fn default() -> Self {
        Self {
            inner: Owned(Vec::new()),
        }
    }
}
