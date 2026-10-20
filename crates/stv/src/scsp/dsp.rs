//! The SCSP's effect DSP: 128 steps of microcode a sample over the slots' sends (MIXS), with a
//! ring buffer in sound RAM, writing the effect returns (EFREG). The step's fields and data paths
//! follow MAME's scspdsp.cpp (BSD-3-Clause; copyright-holders ElSemi, R. Belmont); where the
//! oracle's SCSP differs (tools/scsp/probes.py), the oracle's: the ring buffer is always 16-bit
//! floats (NOFL changes nothing), a read reaches MEMS two steps after its MRD, an IWT writes MEMS
//! after the step's INPUTS has read it, the down-counter wraps at the ring's current length, and
//! EWT sets EFREG, which keeps its value from sample to sample.

use crate::mem::{rd16, wr16};

#[derive(Clone, Debug)]
pub struct Dsp {
    /// The microprogram (MPRO), 128 steps of four words.
    pub mpro: [u16; 512],
    /// Coefficients (13 bits, kept as written: shifted up 3) and memory addresses.
    pub coef: [u16; 64],
    pub madrs: [u16; 32],
    /// TEMP (24-bit, addressed relative to the down-counter), MEMS (24-bit), MIXS (20-bit sums of
    /// the sends), EXTS (the external inputs), EFREG (the outputs).
    pub temp: [i32; 128],
    pub mems: [i32; 32],
    pub mixs: [i32; 16],
    pub exts: [i16; 2],
    pub efreg: [i16; 16],
    /// The down-counter, one a sample.
    pub dec: u32,
    /// The ring buffer: its base (RBP, in 4K words) and length (RBL: 8K << n words).
    pub rbp: u32,
    pub rbl: u32,
}

impl Default for Dsp {
    fn default() -> Dsp {
        Dsp {
            mpro: [0; 512],
            coef: [0; 64],
            madrs: [0; 32],
            temp: [0; 128],
            mems: [0; 32],
            mixs: [0; 16],
            exts: [0; 2],
            efreg: [0; 16],
            dec: 0,
            rbp: 0,
            rbl: 0,
        }
    }
}

#[inline]
fn sext(v: i32, bits: u32) -> i32 {
    (v << (32 - bits)) >> (32 - bits)
}

/// A 24-bit value as the ring buffer's 16-bit float: sign, 4-bit exponent, 11-bit mantissa.
fn pack(v: i32) -> u16 {
    let sign = (v >> 23) & 1;
    let mut t = ((v ^ (v << 1)) & 0xFF_FFFF) as u32;
    let mut exp = 0;
    while exp < 12 && t & 0x80_0000 == 0 {
        t <<= 1;
        exp += 1;
    }
    let mut m = if exp < 12 { (v << exp) & 0x3F_FFFF } else { v << 11 };
    m = (m >> 11) & 0x7FF;
    (m | (sign << 15) | (exp << 11)) as u16
}

fn unpack(v: u16) -> i32 {
    let sign = ((v >> 15) & 1) as i32;
    let mut exp = ((v >> 11) & 0xF) as i32;
    let mut u = ((v & 0x7FF) as i32) << 11;
    if exp > 11 {
        exp = 11;
        u |= sign << 22;
    } else {
        u |= (sign ^ 1) << 22;
    }
    u |= sign << 23;
    sext(u, 24) >> exp
}

impl Dsp {
    /// One sample's program.
    pub fn run(&mut self, ram: &mut [u8]) {
        let mut acc: i32 = 0;
        let mut memval: i32 = 0;
        // a read under way: the step its value arrives at, and the value
        let mut reading: Option<(usize, i32)> = None;
        let mut frc: i32 = 0;
        let mut yreg: i32 = 0;
        let mut adrs: u32 = 0;
        let ring = (8 * 1024u32) << self.rbl;
        for step in 0..128usize {
            let i = &self.mpro[step * 4..step * 4 + 4];
            let tra = ((i[0] >> 8) & 0x7F) as u32;
            let twt = (i[0] >> 7) & 1 != 0;
            let twa = (i[0] & 0x7F) as u32;
            let xsel = (i[1] >> 15) & 1 != 0;
            let ysel = (i[1] >> 13) & 3;
            let ira = ((i[1] >> 6) & 0x3F) as usize;
            let iwt = (i[1] >> 5) & 1 != 0;
            let iwa = (i[1] & 0x1F) as usize;
            let table = (i[2] >> 15) & 1 != 0;
            let mwt = (i[2] >> 14) & 1 != 0;
            let mrd = (i[2] >> 13) & 1 != 0;
            let ewt = (i[2] >> 12) & 1 != 0;
            let ewa = ((i[2] >> 8) & 0xF) as usize;
            let adrl = (i[2] >> 7) & 1 != 0;
            let frcl = (i[2] >> 6) & 1 != 0;
            let shift = (i[2] >> 4) & 3;
            let yrl = (i[2] >> 3) & 1 != 0;
            let negb = (i[2] >> 2) & 1 != 0;
            let zero = (i[2] >> 1) & 1 != 0;
            let bsel = i[2] & 1 != 0;
            let coef = ((i[3] >> 9) & 0x3F) as usize;
            let masa = ((i[3] >> 2) & 0x1F) as usize;
            let adreb = (i[3] >> 1) & 1 != 0;
            let nxadr = i[3] & 1 != 0;

            if let Some((at, v)) = reading
                && step >= at
            {
                memval = v;
                reading = None;
            }
            let inputs = match ira {
                0..=0x1F => self.mems[ira],
                0x20..=0x2F => self.mixs[ira - 0x20] << 4,
                0x30 | 0x31 => (self.exts[ira - 0x30] as i32) << 8,
                _ => 0,
            };
            let inputs = sext(inputs, 24);
            if iwt {
                // after INPUTS has read it (an IRA of the same MEMS sees the old value)
                self.mems[iwa] = memval;
            }
            let temp_in = sext(self.temp[((tra + self.dec) & 0x7F) as usize], 24);
            let b = if zero {
                0
            } else {
                let b = if bsel { acc } else { temp_in };
                if negb { b.wrapping_neg() } else { b }
            };
            let x = if xsel { inputs } else { temp_in };
            let y = match ysel {
                0 => frc,
                1 => (self.coef[coef] as i16 as i32) >> 3,
                2 => (yreg >> 11) & 0x1FFF,
                _ => (yreg >> 4) & 0x0FFF,
            };
            if yrl {
                yreg = inputs;
            }
            let shifted = match shift {
                0 => acc.clamp(-0x80_0000, 0x7F_FFFF),
                1 => acc.wrapping_mul(2).clamp(-0x80_0000, 0x7F_FFFF),
                2 => sext(acc.wrapping_mul(2), 24),
                _ => sext(acc, 24),
            };
            let y = sext(y, 13);
            acc = (((x as i64 * y as i64) >> 12) + b as i64) as i32;
            if twt {
                self.temp[((twa + self.dec) & 0x7F) as usize] = shifted;
            }
            if frcl {
                frc = if shift == 3 { shifted & 0x0FFF } else { (shifted >> 11) & 0x1FFF };
            }
            if (mrd || mwt) && step & 1 == 1 {
                let mut addr = self.madrs[masa] as u32;
                if !table {
                    addr = addr.wrapping_add(self.dec);
                }
                if adreb {
                    addr = addr.wrapping_add(adrs & 0x0FFF);
                }
                if nxadr {
                    addr = addr.wrapping_add(1);
                }
                addr &= if table { 0xFFFF } else { ring - 1 };
                let byte = (((addr + (self.rbp << 12)) << 1) & 0x7_FFFE) as usize;
                if mrd {
                    reading = Some((step + 2, unpack(rd16(ram, byte))));
                }
                if mwt {
                    wr16(ram, byte, pack(shifted));
                }
            }
            if adrl {
                adrs = if shift == 3 { ((shifted >> 12) & 0xFFF) as u32 } else { (inputs >> 16) as u32 };
            }
            if ewt {
                self.efreg[ewa] = (shifted >> 8) as i16;
            }
        }
        // the down-counter wraps at the ring buffer's length
        self.dec = self.dec.wrapping_sub(1) & (ring - 1);
    }
}
