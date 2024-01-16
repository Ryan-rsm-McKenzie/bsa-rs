#[must_use]
pub(crate) const fn make_four(cc: &[u8]) -> u32 {
    let buffer = match cc.len() {
        0 => [0, 0, 0, 0],
        1 => [cc[0], 0, 0, 0],
        2 => [cc[0], cc[1], 0, 0],
        3 => [cc[0], cc[1], cc[2], 0],
        _ => [cc[0], cc[1], cc[2], cc[3]],
    };
    u32::from_le_bytes(buffer)
}

#[test]
fn test() {
    assert_eq!(make_four(b""), 0x00000000);
    assert_eq!(make_four(b"A"), 0x00000041);
    assert_eq!(make_four(b"AB"), 0x00004241);
    assert_eq!(make_four(b"ABC"), 0x00434241);
    assert_eq!(make_four(b"ABCD"), 0x44434241);
    assert_eq!(make_four(b"ABCDE"), 0x44434241);
}
