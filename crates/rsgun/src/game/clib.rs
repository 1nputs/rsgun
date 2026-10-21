//! The C library's memset and memcpy as the game links them (Hitachi's for the SH): over a dozen
//! bytes they go by 32-bit words, and they write with the widths they choose, which matters when
//! the memory is a video chip's. And GCC's shift helpers (lib1funcs), which keep their own
//! convention: the value in and out in r4; and newlib's atexit.

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

/// GCC's __ashiftrt_r4_N: r4 shifted right arithmetically by N, left in r4. Up to 15 it is a run
/// of shar (T left the last bit shifted out); 24 is shlr16, shlr8, exts.b (T as it was).
pub fn ashiftrt_r4<const N: u32>(cpu: &mut Cpu, v: u32) {
    let v = v as i32;
    cpu.leave(&[(4, (v >> N) as u32)]);
    if N <= 15 {
        cpu.leave_t((v >> (N - 1)) & 1 != 0);
    }
}

/// The C library's signed division (Hitachi's, by the SH-2's division steps): `a` / `b` toward
/// zero, 0 when `b` is 0. Run as its code runs it (div0s, then 32 of rotcl and div1, on the CPU's
/// own registers), for what the steps leave: the partial remainder in r3, and SR's Q, M and T.
pub fn divide(cpu: &mut Cpu, a: u32, b: u32) -> u32 {
    let s = &mut *cpu.sh2;
    s.r[1] = a;
    if b == 0 {
        s.set_t(true);
        return 0;
    }
    s.r[2] = 0;
    // div0s r2,r1 (M the zero's sign); the dividend's sign into r3 and a one off a negative one
    s.set_q(a >> 31 != 0);
    s.set_m(false);
    s.set_t(a >> 31 != 0);
    s.r[3] = 0u32.wrapping_sub(s.t() as u32);
    let borrow = a < s.t() as u32;
    s.r[1] = a.wrapping_sub(s.t() as u32);
    s.set_t(borrow);
    // div0s r0,r3
    s.r[0] = b;
    s.set_q(s.r[3] >> 31 != 0);
    s.set_m(b >> 31 != 0);
    s.set_t(s.q() != s.m());
    let rotcl = |s: &mut stv::sh2::Sh2| {
        let out = s.r[1] >> 31 != 0;
        s.r[1] = (s.r[1] << 1) | s.t() as u32;
        s.set_t(out);
    };
    for _ in 0..32 {
        rotcl(s);
        s.div1(3, 0);
    }
    rotcl(s);
    // addc r2,r1
    let t = s.t() as u32;
    let q = s.r[1].wrapping_add(t);
    s.set_t(q < t);
    s.r[1] = q;
    q
}

/// atexit (newlib's): `f` added to the reentrancy struct's list of functions (its first block
/// inside the struct, made current on the first call; a block of 32 that is full gets a new one
/// from malloc ahead of it): 0, or -1 when malloc fails. Not seen run.
const IMPURE_PTR: u32 = 0x0600_a990;
const ATEXIT: u32 = 0x170;
const ATEXIT0: u32 = 0x174;
const ATEXIT_SIZE: u32 = 0x88;
const ATEXIT_MAX: i32 = 31;
const MALLOC: u32 = 0x0600_82e4;

pub fn atexit(cpu: &mut Cpu, f: u32) -> u32 {
    let reent = cpu.read32(IMPURE_PTR);
    let mut block = cpu.read32(reent + ATEXIT);
    if block == 0 {
        block = reent + ATEXIT0;
        cpu.write32(reent + ATEXIT, block);
    }
    if cpu.read32(block + 4) as i32 > ATEXIT_MAX {
        block = cpu.call(MALLOC, &[ATEXIT_SIZE]);
        if block == 0 {
            return u32::MAX;
        }
        cpu.write32(block + 4, 0);
        let reent = cpu.read32(IMPURE_PTR);
        let head = cpu.read32(reent + ATEXIT);
        cpu.write32(block, head);
        cpu.write32(reent + ATEXIT, block);
    }
    let n = cpu.read32(block + 4);
    cpu.write32(block + 4, n + 1);
    cpu.write32(block + 8 + 4 * n, f);
    0
}
