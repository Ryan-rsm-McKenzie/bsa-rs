//! Archives come in various flavors, and the specific variant you'll need to use depends on which game you're working with. Learn more by choosing one of [`tes3`], [`tes4`], or [`fo4`].

#![warn(
    clippy::pedantic,
    clippy::single_char_lifetime_names,
    clippy::std_instead_of_core
)]
#![allow(
    unknown_lints,
    clippy::enum_glob_use,
    clippy::missing_errors_doc,
    clippy::struct_field_names
)]

mod cc;
mod containers;
mod derive;
pub mod fo4;
mod guess;
mod hashing;
mod io;
mod protocols;
pub mod tes3;
pub mod tes4;

pub use guess::{guess_format, FileFormat};

/// Makes a shallow copy of the input.
///
/// The lifetime of the result is tied to the input buffer.
pub struct Borrowed<'borrow>(pub &'borrow [u8]);

/// Makes a deep copy of the input.
///
/// The lifetime of the result is independent of the input buffer.
pub struct Copied<'copy>(pub &'copy [u8]);

mod private {
    pub trait Sealed {}
}

use private::Sealed;

/// A trait that enables reading from various sources.
pub trait Reader<T>: Sealed {
    type Error;
    type Item;

    /// Reads an instance of `Self::Item` from the given source.
    fn read(source: T) -> core::result::Result<Self::Item, Self::Error>;
}

/// A trait that creates an optionally compressed container using the given value.
pub trait CompressableFrom<T>: Sealed {
    /// Makes a compressed instance of `Self` using the given data.
    #[must_use]
    fn from_compressed(value: T, decompressed_len: usize) -> Self;

    /// Makes a decompressed instance of `Self` using the given data.
    #[must_use]
    fn from_decompressed(value: T) -> Self;
}

/// Indicates whether the operation should finish by compressing the data or not.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CompressionResult {
    /// The data will finish in a compressed state.
    Compressed,
    /// The data will finish in a decompressed state.
    #[default]
    Decompressed,
}

/// A trait that enables reading from various sources, with configuration options.
pub trait ReaderWithOptions<T>: Sealed {
    type Error;
    type Item;
    type Options;

    /// Reads an instance of `Self::Item` from the given source, using the given options.
    fn read(source: T, options: &Self::Options) -> core::result::Result<Self::Item, Self::Error>;
}

pub use bstr::{BStr, BString, ByteSlice, ByteVec};

/// Convenience using statements for traits that are needed to work with the library.
pub mod prelude {
    pub use crate::{CompressableFrom as _, Reader as _, ReaderWithOptions as _};
}
