mod detail {
    pub enum ByteContainer<'a> {
        Owned(Vec<u8>),
        Borrowed(&'a [u8]),
    }
}

use detail::ByteContainer::*;

pub struct ByteContainer<'a> {
    container: detail::ByteContainer<'a>,
}

impl<'a> ByteContainer<'a> {
    pub fn as_bytes(&self) -> &[u8] {
        match &self.container {
            Owned(x) => x,
            Borrowed(x) => x,
        }
    }

    pub fn as_ptr(&self) -> *const u8 {
        match &self.container {
            Owned(owner) => owner.as_ptr(),
            Borrowed(view) => view.as_ptr(),
        }
    }

    pub fn from_borrowed(bytes: &'a [u8]) -> Self {
        Self {
            container: Borrowed(bytes),
        }
    }

    pub fn from_owned(bytes: Vec<u8>) -> Self {
        Self {
            container: Owned(bytes),
        }
    }

    pub fn is_empty(&self) -> bool {
        match &self.container {
            Owned(x) => x.is_empty(),
            Borrowed(x) => x.is_empty(),
        }
    }

    pub fn len(&self) -> usize {
        match &self.container {
            Owned(x) => x.len(),
            Borrowed(x) => x.len(),
        }
    }

    pub fn into_owned<'b>(self) -> ByteContainer<'b> {
        ByteContainer {
            container: match self.container {
                Owned(x) => Owned(x),
                Borrowed(x) => Owned(x.to_owned()),
            },
        }
    }
}

impl<'a> Default for ByteContainer<'a> {
    fn default() -> Self {
        Self {
            container: Owned(Vec::new()),
        }
    }
}
