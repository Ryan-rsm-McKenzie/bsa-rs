use bstr::BString;

const fn build_lookup_table() -> [u8; 256] {
    let mut table = [0u8; u8::MAX as usize + 1];
    let mut i: usize = 0;
    loop {
        table[i] = i as u8;
        if i >= u8::MAX as usize {
            break;
        } else {
            i += 1;
        }
    }

    table['/' as usize] = '\\' as u8;

    let offset = 'a' as u8 - 'A' as u8;
    let mut i = 'A' as usize;
    loop {
        table[i] = (i as u8) + offset;
        if i >= 'Z' as usize {
            break;
        } else {
            i += 1;
        }
    }

    table
}

fn map_byte(b: u8) -> u8 {
    const LUT: [u8; 256] = build_lookup_table();
    LUT[b as usize]
}

pub fn normalize_path(path: &mut BString) {
    for b in path.iter_mut() {
        *b = map_byte(*b);
    }

    while path.last().is_some_and(|x| *x == '\\' as u8) {
        path.pop();
    }

    while path.first().is_some_and(|x| *x == '\\' as u8) {
        path.remove(0);
    }

    if path.is_empty() || path.len() >= 260 {
        path.clear();
        path.push('\\' as u8);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
