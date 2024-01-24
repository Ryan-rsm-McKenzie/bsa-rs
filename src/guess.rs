use crate::cc;
use core::mem;
use std::io::Read;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileFormat {
    TES3,
    TES4,
    FO4,
}

const BSA: u32 = cc::make_four(b"BSA");
const BTDX: u32 = cc::make_four(b"BTDX");

#[allow(clippy::module_name_repetitions)]
pub fn guess_format<In>(source: &mut In) -> Option<FileFormat>
where
    In: ?Sized + Read,
{
    let mut buf = [0u8; mem::size_of::<u32>()];
    source.read_exact(&mut buf).ok()?;
    let magic = u32::from_le_bytes(buf);
    match magic {
        0x100 => Some(FileFormat::TES3),
        BSA => Some(FileFormat::TES4),
        BTDX => Some(FileFormat::FO4),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crate::FileFormat;
    use anyhow::Context as _;
    use std::{fs::File, path::Path};

    #[test]
    fn guess() -> anyhow::Result<()> {
        let root = Path::new("data/common_guess_test");
        let tests = [
            (FileFormat::TES3, "tes3.bsa"),
            (FileFormat::TES4, "tes4.bsa"),
            (FileFormat::FO4, "fo4.ba2"),
        ];

        for (format, file_name) in tests {
            let mut file = File::open(root.join(file_name))
                .with_context(|| format!("failed to open file: {file_name}"))?;
            let guess = crate::guess_format(&mut file);
            assert_eq!(guess, Some(format));
        }

        Ok(())
    }
}
