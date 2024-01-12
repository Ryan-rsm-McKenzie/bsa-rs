use memmap2::Mmap;
use std::sync::Arc;

struct Mapping {
    pos: usize,
    len: usize,
    mapping: Arc<Mmap>,
}

impl Mapping {
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.mapping[self.pos..self.pos + self.len]
    }

    #[must_use]
    pub fn as_ptr(&self) -> *const u8 {
        self.as_bytes().as_ptr()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[must_use]
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
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        match &self.inner {
            Owned(x) => x,
            Borrowed(x) => x,
            Mapped(x) => x.as_bytes(),
        }
    }

    #[must_use]
    pub fn as_ptr(&self) -> *const u8 {
        match &self.inner {
            Owned(x) => x.as_ptr(),
            Borrowed(x) => x.as_ptr(),
            Mapped(x) => x.as_ptr(),
        }
    }

    #[must_use]
    pub fn from_borrowed(bytes: &'a [u8]) -> Self {
        Self {
            inner: Borrowed(bytes),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        match &self.inner {
            Owned(x) => x.is_empty(),
            Borrowed(x) => x.is_empty(),
            Mapped(x) => x.is_empty(),
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        match &self.inner {
            Owned(x) => x.len(),
            Borrowed(x) => x.len(),
            Mapped(x) => x.len(),
        }
    }

    #[must_use]
    pub fn into_owned(self) -> ByteContainer<'static> {
        ByteContainer {
            inner: match self.inner {
                Owned(x) => Owned(x),
                Borrowed(x) => Owned(x.to_owned()),
                Mapped(x) => Mapped(x),
            },
        }
    }

    #[must_use]
    pub fn into_compressable(
        self,
        decompressed_len: Option<usize>,
    ) -> CompressableByteContainer<'a> {
        CompressableByteContainer {
            inner: match (self.inner, decompressed_len) {
                (Owned(x), Some(len)) => OwnedCompressed(x, len),
                (Owned(x), None) => OwnedDecompressed(x),
                (Borrowed(x), Some(len)) => BorrowedCompressed(x, len),
                (Borrowed(x), None) => BorrowedDecompressed(x),
                (Mapped(x), Some(len)) => MappedCompressed(x, len),
                (Mapped(x), None) => MappedDecompressed(x),
            },
        }
    }
}

impl ByteContainer<'static> {
    #[must_use]
    pub fn from_owned(bytes: Vec<u8>) -> Self {
        Self {
            inner: Owned(bytes),
        }
    }

    #[must_use]
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

enum CompressableByteContainerInner<'a> {
    OwnedDecompressed(Vec<u8>),
    OwnedCompressed(Vec<u8>, usize),
    BorrowedDecompressed(&'a [u8]),
    BorrowedCompressed(&'a [u8], usize),
    MappedDecompressed(Mapping),
    MappedCompressed(Mapping, usize),
}

use CompressableByteContainerInner::*;

pub struct CompressableByteContainer<'a> {
    inner: CompressableByteContainerInner<'a>,
}

impl<'a> CompressableByteContainer<'a> {
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        match &self.inner {
            OwnedDecompressed(x) | OwnedCompressed(x, _) => x,
            BorrowedDecompressed(x) | BorrowedCompressed(x, _) => x,
            MappedDecompressed(x) | MappedCompressed(x, _) => x.as_bytes(),
        }
    }

    #[must_use]
    pub fn as_ptr(&self) -> *const u8 {
        match &self.inner {
            OwnedDecompressed(x) | OwnedCompressed(x, _) => x.as_ptr(),
            BorrowedDecompressed(x) | BorrowedCompressed(x, _) => x.as_ptr(),
            MappedDecompressed(x) | MappedCompressed(x, _) => x.as_ptr(),
        }
    }

    #[must_use]
    pub fn from_borrowed(bytes: &'a [u8], decompressed_len: Option<usize>) -> Self {
        Self {
            inner: match decompressed_len {
                Some(len) => BorrowedCompressed(bytes, len),
                None => BorrowedDecompressed(bytes),
            },
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        match &self.inner {
            OwnedDecompressed(x) | OwnedCompressed(x, _) => x.is_empty(),
            BorrowedDecompressed(x) | BorrowedCompressed(x, _) => x.is_empty(),
            MappedDecompressed(x) | MappedCompressed(x, _) => x.is_empty(),
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        match &self.inner {
            OwnedDecompressed(x) | OwnedCompressed(x, _) => x.len(),
            BorrowedDecompressed(x) | BorrowedCompressed(x, _) => x.len(),
            MappedDecompressed(x) | MappedCompressed(x, _) => x.len(),
        }
    }

    #[must_use]
    pub fn into_owned(self) -> CompressableByteContainer<'static> {
        CompressableByteContainer {
            inner: match self.inner {
                OwnedDecompressed(x) => OwnedDecompressed(x),
                OwnedCompressed(x, y) => OwnedCompressed(x, y),
                BorrowedDecompressed(x) => OwnedDecompressed(x.to_vec()),
                BorrowedCompressed(x, y) => OwnedCompressed(x.to_vec(), y),
                MappedDecompressed(x) => MappedDecompressed(x),
                MappedCompressed(x, y) => MappedCompressed(x, y),
            },
        }
    }

    #[must_use]
    pub fn decompressed_len(&self) -> Option<usize> {
        match &self.inner {
            OwnedDecompressed(_) | BorrowedDecompressed(_) | MappedDecompressed(_) => None,
            OwnedCompressed(_, x) | BorrowedCompressed(_, x) | MappedCompressed(_, x) => Some(*x),
        }
    }

    #[must_use]
    pub fn is_compressed(&self) -> bool {
        match &self.inner {
            OwnedDecompressed(_) | BorrowedDecompressed(_) | MappedDecompressed(_) => false,
            OwnedCompressed(_, _) | BorrowedCompressed(_, _) | MappedCompressed(_, _) => true,
        }
    }
}

impl CompressableByteContainer<'static> {
    #[must_use]
    pub fn from_owned(bytes: Vec<u8>, decompressed_len: Option<usize>) -> Self {
        Self {
            inner: match decompressed_len {
                Some(len) => OwnedCompressed(bytes, len),
                None => OwnedDecompressed(bytes),
            },
        }
    }

    #[must_use]
    pub fn from_mapped(
        pos: usize,
        len: usize,
        mapping: Arc<Mmap>,
        decompressed_len: Option<usize>,
    ) -> Self {
        let mapping = Mapping { pos, len, mapping };
        Self {
            inner: match decompressed_len {
                Some(len) => MappedCompressed(mapping, len),
                None => MappedDecompressed(mapping),
            },
        }
    }
}

impl<'a> Default for CompressableByteContainer<'a> {
    fn default() -> Self {
        Self {
            inner: OwnedDecompressed(Vec::new()),
        }
    }
}
