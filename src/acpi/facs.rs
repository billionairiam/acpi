fn append_u32_le(buf: &mut Vec<u8>, value: u32) {
    buf.extend_from_slice(&value.to_le_bytes());
}

pub fn build_facs() -> Vec<u8> {
    let mut facs: Vec<u8> = Vec::with_capacity(64);
    facs.extend_from_slice(b"FACS");
    append_u32_le(&mut facs, 64); // Length
    append_u32_le(&mut facs, 0); // Hardware Signature
    append_u32_le(&mut facs, 0); // Firmware Waking Vector
    append_u32_le(&mut facs, 0); // Global Lock
    append_u32_le(&mut facs, 0); // Flags
    facs.extend_from_slice(&[0u8; 40]); // Reserved
    assert_eq!(facs.len(), 64);

    facs
}
