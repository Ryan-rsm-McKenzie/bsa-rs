macro_rules! container_wrapper {
    ($this:ident) => {
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

pub(crate) use container_wrapper;
