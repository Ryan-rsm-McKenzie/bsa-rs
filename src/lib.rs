#![warn(clippy::pedantic, clippy::std_instead_of_core)]
#![allow(clippy::enum_glob_use, clippy::missing_errors_doc)]

mod containers;
mod hashing;
mod io;
mod strings;
pub mod tes3;

pub struct Borrowed<'a>(pub &'a [u8]);

pub struct Copied<'a>(pub &'a [u8]);

pub trait Read<T>
where
    Self: Sized,
{
    type Error;

    fn read(source: T) -> core::result::Result<Self, Self::Error>;
}

pub use bstr::{BStr, BString};

pub mod prelude {
    pub use crate::Read as _;
}
