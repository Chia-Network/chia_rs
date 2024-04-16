pub fn u64_to_bytes(n: u64) -> Vec<u8> {
    let mut buf = Vec::<u8>::new();
    buf.extend_from_slice(&n.to_be_bytes());
    if (buf[0] & 0x80) != 0 {
        buf.insert(0, 0);
    } else {
        while buf.len() > 1 && buf[0] == 0 && (buf[1] & 0x80) == 0 {
            buf.remove(0);
        }
    }
    buf
}
