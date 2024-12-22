use core::ops::Range;
use memmap2::Mmap;
use std::sync::Arc;

#[derive(Clone, Debug)]
struct Mapping {
    pos: usize,
    len: usize,
    mapping: Arc<Mmap>,
}

impl Mapping {
    #[must_use]
    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.mapping[self.pos..self.pos + self.len]
    }

    #[must_use]
    pub(crate) fn as_ptr(&self) -> *const u8 {
        self.as_bytes().as_ptr()
    }

    #[must_use]
    pub(crate) fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[must_use]
    pub(crate) fn len(&self) -> usize {
        self.len
    }
}

#[derive(Clone, Debug)]
enum BytesInner<'bytes> {
    Owned(Box<[u8]>),
    Borrowed(&'bytes [u8]),
    Mapped(Mapping),
}

use BytesInner::*;

impl From<Mapping> for BytesInner<'static> {
    fn from(value: Mapping) -> Self {
        Mapped(value)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Bytes<'bytes> {
    inner: BytesInner<'bytes>,
}

impl<'bytes> Bytes<'bytes> {
    #[must_use]
    pub(crate) fn as_bytes(&self) -> &[u8] {
        match &self.inner {
            Owned(x) => x,
            Borrowed(x) => x,
            Mapped(x) => x.as_bytes(),
        }
    }

    #[must_use]
    pub(crate) fn as_ptr(&self) -> *const u8 {
        match &self.inner {
            Owned(x) => x.as_ptr(),
            Borrowed(x) => x.as_ptr(),
            Mapped(x) => x.as_ptr(),
        }
    }

    #[must_use]
    pub(crate) fn from_borrowed(bytes: &'bytes [u8]) -> Self {
        Self {
            inner: Borrowed(bytes),
        }
    }

    #[must_use]
    pub(crate) fn is_empty(&self) -> bool {
        match &self.inner {
            Owned(x) => x.is_empty(),
            Borrowed(x) => x.is_empty(),
            Mapped(x) => x.is_empty(),
        }
    }

    #[must_use]
    pub(crate) fn len(&self) -> usize {
        match &self.inner {
            Owned(x) => x.len(),
            Borrowed(x) => x.len(),
            Mapped(x) => x.len(),
        }
    }

    #[must_use]
    pub(crate) fn into_owned(self) -> Bytes<'static> {
        Bytes {
            inner: match self.inner {
                Owned(x) => Owned(x),
                Borrowed(x) => Owned(x.into()),
                Mapped(x) => Mapped(x),
            },
        }
    }

    #[must_use]
    pub(crate) fn into_compressable(
        self,
        decompressed_len: Option<usize>,
    ) -> CompressableBytes<'bytes> {
        CompressableBytes {
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

    #[must_use]
    pub(crate) fn copy_slice(&self, slice: Range<usize>) -> Self {
        match &self.inner {
            Owned(x) => Self {
                inner: Owned(x[slice].into()),
            },
            Borrowed(x) => Self {
                inner: Borrowed(&x[slice]),
            },
            Mapped(x) => Self {
                inner: Mapping {
                    pos: x.pos + slice.start,
                    len: slice.len(),
                    mapping: x.mapping.clone(),
                }
                .into(),
            },
        }
    }
}

impl Bytes<'static> {
    #[must_use]
    pub(crate) fn from_owned(bytes: Box<[u8]>) -> Self {
        Self {
            inner: Owned(bytes),
        }
    }

    #[must_use]
    pub(crate) fn from_mapped(pos: usize, len: usize, mapping: Arc<Mmap>) -> Self {
        Self {
            inner: Mapping { pos, len, mapping }.into(),
        }
    }
}

impl Default for Bytes<'_> {
    fn default() -> Self {
        Self {
            inner: Owned(Box::default()),
        }
    }
}

#[derive(Clone, Debug)]
enum CompressableBytesInner<'bytes> {
    OwnedDecompressed(Box<[u8]>),
    OwnedCompressed(Box<[u8]>, usize),
    BorrowedDecompressed(&'bytes [u8]),
    BorrowedCompressed(&'bytes [u8], usize),
    MappedDecompressed(Mapping),
    MappedCompressed(Mapping, usize),
}

use CompressableBytesInner::*;

#[derive(Clone, Debug)]
pub(crate) struct CompressableBytes<'bytes> {
    inner: CompressableBytesInner<'bytes>,
}

impl<'bytes> CompressableBytes<'bytes> {
    #[must_use]
    pub(crate) fn as_bytes(&self) -> &[u8] {
        match &self.inner {
            OwnedDecompressed(x) | OwnedCompressed(x, _) => x,
            BorrowedDecompressed(x) | BorrowedCompressed(x, _) => x,
            MappedDecompressed(x) | MappedCompressed(x, _) => x.as_bytes(),
        }
    }

    #[must_use]
    pub(crate) fn as_ptr(&self) -> *const u8 {
        match &self.inner {
            OwnedDecompressed(x) | OwnedCompressed(x, _) => x.as_ptr(),
            BorrowedDecompressed(x) | BorrowedCompressed(x, _) => x.as_ptr(),
            MappedDecompressed(x) | MappedCompressed(x, _) => x.as_ptr(),
        }
    }

    #[must_use]
    pub(crate) fn from_borrowed(bytes: &'bytes [u8], decompressed_len: Option<usize>) -> Self {
        Self {
            inner: match decompressed_len {
                Some(len) => BorrowedCompressed(bytes, len),
                None => BorrowedDecompressed(bytes),
            },
        }
    }

    #[must_use]
    pub(crate) fn is_empty(&self) -> bool {
        match &self.inner {
            OwnedDecompressed(x) | OwnedCompressed(x, _) => x.is_empty(),
            BorrowedDecompressed(x) | BorrowedCompressed(x, _) => x.is_empty(),
            MappedDecompressed(x) | MappedCompressed(x, _) => x.is_empty(),
        }
    }

    #[must_use]
    pub(crate) fn len(&self) -> usize {
        match &self.inner {
            OwnedDecompressed(x) | OwnedCompressed(x, _) => x.len(),
            BorrowedDecompressed(x) | BorrowedCompressed(x, _) => x.len(),
            MappedDecompressed(x) | MappedCompressed(x, _) => x.len(),
        }
    }

    #[must_use]
    pub(crate) fn into_owned(self) -> CompressableBytes<'static> {
        CompressableBytes {
            inner: match self.inner {
                OwnedDecompressed(x) => OwnedDecompressed(x),
                OwnedCompressed(x, y) => OwnedCompressed(x, y),
                BorrowedDecompressed(x) => OwnedDecompressed(x.into()),
                BorrowedCompressed(x, y) => OwnedCompressed(x.into(), y),
                MappedDecompressed(x) => MappedDecompressed(x),
                MappedCompressed(x, y) => MappedCompressed(x, y),
            },
        }
    }

    #[must_use]
    pub(crate) fn decompressed_len(&self) -> Option<usize> {
        match &self.inner {
            OwnedDecompressed(_) | BorrowedDecompressed(_) | MappedDecompressed(_) => None,
            OwnedCompressed(_, x) | BorrowedCompressed(_, x) | MappedCompressed(_, x) => Some(*x),
        }
    }

    #[must_use]
    pub(crate) fn is_compressed(&self) -> bool {
        match &self.inner {
            OwnedDecompressed(_) | BorrowedDecompressed(_) | MappedDecompressed(_) => false,
            OwnedCompressed(_, _) | BorrowedCompressed(_, _) | MappedCompressed(_, _) => true,
        }
    }
}

impl CompressableBytes<'static> {
    #[must_use]
    pub(crate) fn from_owned(bytes: Box<[u8]>, decompressed_len: Option<usize>) -> Self {
        Self {
            inner: match decompressed_len {
                Some(len) => OwnedCompressed(bytes, len),
                None => OwnedDecompressed(bytes),
            },
        }
    }
}

impl Default for CompressableBytes<'_> {
    fn default() -> Self {
        Self {
            inner: OwnedDecompressed(Box::default()),
        }
    }
}
