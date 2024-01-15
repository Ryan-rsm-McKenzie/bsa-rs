use bstr::BString;

#[must_use]
const fn build_lookup_table() -> [u8; 256] {
    let mut table = [0u8; u8::MAX as usize + 1];
    let mut i: u8 = 0;
    loop {
        table[i as usize] = i;
        match i {
            u8::MAX => break,
            _ => i += 1,
        };
    }

    table['/' as usize] = b'\\';

    let offset = b'a' - b'A';
    let mut i = b'A';
    loop {
        table[i as usize] = i + offset;
        match i {
            b'Z' => break,
            _ => i += 1,
        };
    }

    table
}

#[must_use]
fn map_byte(b: u8) -> u8 {
    const LUT: [u8; 256] = build_lookup_table();
    LUT[b as usize]
}

pub fn normalize_path(path: &mut BString) {
    for b in path.iter_mut() {
        *b = map_byte(*b);
    }

    while path.last().is_some_and(|&x| x == b'\\') {
        path.pop();
    }

    while path.first().is_some_and(|&x| x == b'\\') {
        path.remove(0);
    }

    if path.is_empty() || path.len() >= 260 {
        path.clear();
        path.push(b'.');
    }
}

#[cfg(test)]
mod tests {
    use super::map_byte;

    #[test]
    fn test_mapping() {
        macro_rules! test {
            ($l:literal, $r:literal) => {
                assert_eq!(map_byte($l as u8), $r as u8);
            };
        }

        test!('A', 'a');
        test!('a', 'a');
        test!('Z', 'z');
        test!('z', 'z');
        test!('/', '\\');
        test!('\\', '\\');
        test!('.', '.');
        test!(255, 255);
    }
}
