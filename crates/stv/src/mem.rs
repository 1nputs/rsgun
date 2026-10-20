//! Big-endian memory helpers: every memory the machine has is a byte array in the SH-2's order, so
//! dumps and hashes compare with the oracle's directly.

#[inline]
pub fn rd8(m: &[u8], a: usize) -> u8 {
    m[a]
}

#[inline]
pub fn rd16(m: &[u8], a: usize) -> u16 {
    u16::from_be_bytes([m[a], m[a + 1]])
}

#[inline]
pub fn rd32(m: &[u8], a: usize) -> u32 {
    u32::from_be_bytes([m[a], m[a + 1], m[a + 2], m[a + 3]])
}

#[inline]
pub fn wr8(m: &mut [u8], a: usize, v: u8) {
    m[a] = v;
}

#[inline]
pub fn wr16(m: &mut [u8], a: usize, v: u16) {
    m[a..a + 2].copy_from_slice(&v.to_be_bytes());
}

#[inline]
pub fn wr32(m: &mut [u8], a: usize, v: u32) {
    m[a..a + 4].copy_from_slice(&v.to_be_bytes());
}

/// 64-bit FNV-1a, as the oracle's hash lines use (tools/patches/mednafen/oracle.inc).
pub fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h = (h ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// A 16-bit register file written by byte or word: `mask` selects the bytes a write changes.
#[inline]
pub fn merge16(old: u16, v: u16, mask: u16) -> u16 {
    (old & !mask) | (v & mask)
}
