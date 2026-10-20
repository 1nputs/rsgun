//! The C library's memset and memcpy as the game links them (Hitachi's for the SH): over a dozen
//! bytes they go by 32-bit words, and they write with the widths they choose, which matters when
//! the memory is a video chip's.

use super::Cpu;

/// memset: `n` bytes at `dst` set to `c`: dst. Over 11 bytes, after the bytes up to `dst`'s next
/// word, by pairs of words.
pub fn memset(cpu: &mut Cpu, dst: u32, c: u32, n: u32) -> u32 {
    let (mut p, mut n) = (dst, n);
    if n == 0 {
        return dst;
    }
    if n as i32 > 11 {
        for _ in 0..(4u32.wrapping_sub(dst) & 3) {
            cpu.write8(p, c as u8);
            n -= 1;
            p += 1;
        }
        let byte = c & 0xFF;
        let word = byte | byte << 8 | byte << 16 | byte << 24;
        for _ in 0..n >> 3 {
            cpu.write32(p, word);
            cpu.write32(p + 4, word);
            p += 8;
        }
        n &= 7;
    }
    for _ in 0..n {
        cpu.write8(p, c as u8);
        p += 1;
    }
    dst
}

/// memcpy: `n` bytes from `src` to `dst`: dst. Over 12 bytes, after the bytes up to `src`'s next
/// word, by words read whole and written as `dst`'s alignment allows (a word, two halves, four
/// bytes).
pub fn memcpy(cpu: &mut Cpu, dst: u32, src: u32, n: u32) -> u32 {
    let (mut d, mut s, mut n) = (dst, src, n);
    if n == 0 {
        return dst;
    }
    let copy_byte = |cpu: &mut Cpu, d: &mut u32, s: &mut u32| {
        let b = cpu.read8(*s);
        cpu.write8(*d, b);
        *d += 1;
        *s += 1;
    };
    if n as i32 > 12 {
        for _ in 0..(4u32.wrapping_sub(src) & 3) {
            copy_byte(cpu, &mut d, &mut s);
            n -= 1;
        }
        for _ in 0..n >> 2 {
            let w = cpu.read32(s);
            match d & 3 {
                0 => cpu.write32(d, w),
                2 => {
                    cpu.write16(d + 2, w as u16);
                    cpu.write16(d, (w >> 16) as u16);
                }
                _ => {
                    for i in (0..4).rev() {
                        cpu.write8(d + i, (w >> (8 * (3 - i))) as u8);
                    }
                }
            }
            s += 4;
            d += 4;
        }
        n &= 3;
    }
    for _ in 0..n {
        copy_byte(cpu, &mut d, &mut s);
    }
    dst
}
