//! The game's arithmetic helpers (assembly beside its main program): 16.16 fixed-point multiply and
//! divide, and division on the SH-2's divide unit, which the game's division goes through.

use super::Cpu;

/// The divide unit: divisor, 32-bit dividend (writing it divides), and the 64-bit dividend's
/// halves (writing the low half divides), which hold the remainder and the quotient after.
const DVSR: u32 = 0xFFFF_FF00;
const DVDNT: u32 = 0xFFFF_FF04;
const DVDNTH: u32 = 0xFFFF_FF10;
const DVDNTL: u32 = 0xFFFF_FF14;

/// `a * b` in 16.16 fixed point (the 64-bit product's middle).
pub fn fixed_mul(_cpu: &mut Cpu, a: u32, b: u32) -> u32 {
    ((a as i32 as i64 * b as i32 as i64) >> 16) as u32
}

/// `a % b` (signed), on the divide unit.
pub fn remainder(cpu: &mut Cpu, a: u32, b: u32) -> u32 {
    cpu.write32(DVSR, b);
    cpu.write32(DVDNT, a);
    cpu.read32(DVDNTH)
}

/// `a / b` (signed), on the divide unit.
pub fn quotient(cpu: &mut Cpu, a: u32, b: u32) -> u32 {
    cpu.write32(DVSR, b);
    cpu.write32(DVDNT, a);
    cpu.read32(DVDNTL)
}

/// `a / b` in 16.16 fixed point, on the divide unit (`a << 16` divided as 64 bits).
pub fn fixed_div(cpu: &mut Cpu, a: u32, b: u32) -> u32 {
    cpu.write32(DVSR, b);
    cpu.write32(DVDNTH, (a >> 16) as i16 as i32 as u32);
    cpu.write32(DVDNTL, a << 16);
    cpu.read32(DVDNTL)
}
