#![warn(clippy::pedantic, clippy::std_instead_of_core)]
#![allow(clippy::enum_glob_use, clippy::missing_errors_doc)]

mod cc;
mod containers;
mod derive;
mod hashing;
mod io;
mod protocols;
pub mod tes3;
pub mod tes4;

pub struct Borrowed<'a>(pub &'a [u8]);

pub struct Copied<'a>(pub &'a [u8]);

mod private {
    pub trait Sealed {}
}

use private::Sealed;

pub trait Reader<T>: Sealed {
    type Error;
    type Item;

    fn read(source: T) -> core::result::Result<Self::Item, Self::Error>;
}

pub trait CompressableFrom<T>: Sealed {
    #[must_use]
    fn from_compressed(value: T, decompressed_len: usize) -> Self;

    #[must_use]
    fn from_decompressed(value: T) -> Self;
}

pub use bstr::{BStr, BString};

pub mod prelude {
    pub use crate::{CompressableFrom as _, Reader as _};
}
