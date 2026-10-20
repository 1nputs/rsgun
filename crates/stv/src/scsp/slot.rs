//! One of the SCSP's 32 slots: a sample played from sound RAM at a pitch, looped, shaped by an
//! envelope, and levelled. What the oracle's SCSP showed, probe by probe (tools/scsp/probe.py):
//!
//! - The phase is a position in samples, relative to SA, with 14 fraction bits; each sample adds
//!   (0x400 + FNS, 11 bits) shifted by OCT + 4 (so FNS's low bits vanish at the lowest octaves).
//! - Two neighbouring samples are mixed by the fraction's top 6 bits (linear interpolation).
//! - Played backwards (reverse and alternating loops) the read position is LEA·2^14 + NOT(q), q
//!   counting up from where the backward run began.
//! - The envelope is an attenuation, 0 (loudest) to 0x3FF, clocked at the start of every even
//!   sample since the reset, with the Yamaha rate tables: attack `a += (!a * inc) >> 4`, decays
//!   and release `a += inc`. Rates are 2R + 2(KRS + OCT) when that is positive. A sample's writes
//!   come before its clock (rates act at once), its KYONEX after: key on starts the attack at
//!   0x280, key off the release, from the next clock.
//! - Levels: the attenuation a (envelope + 4·TL) scales by (127 - a % 64) / 128, halved for each
//!   64; send levels and pan are 6 dB and 3 dB steps (a 3 dB step is ×3/4).

use crate::mem::rd16;
use crate::state::saved;

/// The envelope's phases (SGC, as the slot monitor shows them).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Eg {
    Attack = 0,
    Decay1 = 1,
    Decay2 = 2,
    #[default]
    Release = 3,
}

impl crate::state::State for Eg {
    fn save(&self, w: &mut Vec<u8>) {
        w.push(*self as u8);
    }
    fn load(&mut self, r: &mut crate::state::Reader) -> Result<(), String> {
        *self = match r.take(1)?[0] {
            0 => Eg::Attack,
            1 => Eg::Decay1,
            2 => Eg::Decay2,
            3 => Eg::Release,
            b => return Err(format!("{b} where an envelope phase was saved")),
        };
        Ok(())
    }
}

/// A slot's registers (0x20 bytes at 0x100000 + 0x20·n), decoded.
#[derive(Clone, Copy, Debug, Default)]
pub struct Regs {
    pub key_on: bool,
    pub sbctl: u16,
    pub ssctl: u16,
    pub lpctl: u16,
    pub pcm8: bool,
    pub sa: u32,
    pub lsa: u32,
    pub lea: u32,
    pub d2r: u16,
    pub d1r: u16,
    pub eghold: bool,
    pub ar: u16,
    pub lpslnk: bool,
    pub krs: u16,
    pub dl: u16,
    pub rr: u16,
    pub stwinh: bool,
    pub sdir: bool,
    pub tl: u16,
    pub mdl: u16,
    pub mdxsl: u16,
    pub mdysl: u16,
    pub oct: i32,
    pub fns: u16,
    pub lfore: bool,
    pub lfof: u16,
    pub plfows: u16,
    pub plfos: u16,
    pub alfows: u16,
    pub alfos: u16,
    pub isel: u16,
    pub imxl: u16,
    pub disdl: u16,
    pub dipan: u16,
    pub efsdl: u16,
    pub efpan: u16,
}

impl Regs {
    pub fn decode(r: &[u8]) -> Regs {
        let w = |i: usize| rd16(r, 2 * i);
        let (w0, w4, w5, w6, w7, w8, w9, w10, w11) = (w(0), w(4), w(5), w(6), w(7), w(8), w(9), w(10), w(11));
        Regs {
            key_on: w0 & 0x0800 != 0,
            sbctl: (w0 >> 9) & 3,
            ssctl: (w0 >> 7) & 3,
            lpctl: (w0 >> 5) & 3,
            pcm8: w0 & 0x10 != 0,
            sa: (((w0 & 0xF) as u32) << 16) | w(1) as u32,
            lsa: w(2) as u32,
            lea: w(3) as u32,
            d2r: (w4 >> 11) & 0x1F,
            d1r: (w4 >> 6) & 0x1F,
            eghold: w4 & 0x20 != 0,
            ar: w4 & 0x1F,
            lpslnk: w5 & 0x4000 != 0,
            krs: (w5 >> 10) & 0xF,
            dl: (w5 >> 5) & 0x1F,
            rr: w5 & 0x1F,
            stwinh: w6 & 0x200 != 0,
            sdir: w6 & 0x100 != 0,
            tl: w6 & 0xFF,
            mdl: (w7 >> 12) & 0xF,
            mdxsl: (w7 >> 6) & 0x3F,
            mdysl: w7 & 0x3F,
            oct: (((w8 >> 11) & 0xF) as i32 ^ 8) - 8,
            fns: w8 & 0x7FF,
            lfore: w9 & 0x8000 != 0,
            lfof: (w9 >> 10) & 0x1F,
            plfows: (w9 >> 8) & 3,
            plfos: (w9 >> 5) & 7,
            alfows: (w9 >> 3) & 3,
            alfos: w9 & 7,
            isel: (w10 >> 3) & 0xF,
            imxl: w10 & 7,
            disdl: (w11 >> 13) & 7,
            dipan: (w11 >> 8) & 0x1F,
            efsdl: (w11 >> 5) & 7,
            efpan: w11 & 0x1F,
        }
    }

    /// The phase's step a sample (14 fraction bits). FNS is kept with the register's bit 10, which
    /// the oracle adds in: the mantissa is those 11 bits XOR 0x400.
    fn step(&self) -> u32 {
        let f = (self.fns ^ 0x400) as u32;
        let s = self.oct + 4;
        if s >= 0 { f << s } else { f >> -s }
    }

    /// An envelope rate's effective rate (0-63): twice the register, plus the key scaling (twice
    /// KRS + OCT when that is positive; KRS 0xF is none).
    fn rate(&self, r: u16) -> u32 {
        if r == 0 {
            return 0;
        }
        let mut rate = 2 * r as i32;
        if self.krs != 0xF {
            rate += (2 * (self.krs as i32 + self.oct)).max(0);
        }
        rate.min(63) as u32
    }
}

/// The envelope's step for an effective rate at an envelope clock count (0 = no step this clock).
fn eg_inc(rate: u32, count: u32) -> u32 {
    const LOW: [[u32; 8]; 4] = [[0, 1, 0, 1, 0, 1, 0, 1], [0, 1, 0, 1, 1, 1, 0, 1], [0, 1, 1, 1, 0, 1, 1, 1], [0, 1, 1, 1, 1, 1, 1, 1]];
    // (the Yamaha tables, the high rates' a step on: the larger step comes on even counts)
    const HIGH: [[u32; 8]; 4] = [[1, 1, 1, 1, 1, 1, 1, 1], [1, 1, 2, 1, 1, 1, 2, 1], [2, 1, 2, 1, 2, 1, 2, 1], [2, 2, 2, 1, 2, 2, 2, 1]];
    match rate {
        0 | 1 => 0,
        2..=47 => {
            let shift = 11 - (rate >> 2);
            if count & ((1 << shift) - 1) != 0 {
                return 0;
            }
            LOW[(rate & 3) as usize][((count >> shift) & 7) as usize]
        }
        48..=59 => HIGH[(rate & 3) as usize][(count & 7) as usize] << ((rate - 48) >> 2),
        _ => 8,
    }
}

/// A level: `v` scaled by attenuation `a` (64 to an octave, (127 - a % 64) / 128 within it).
#[inline]
pub fn attenuate(v: i32, a: u32) -> i32 {
    if a > 0x3FF {
        return 0;
    }
    (v * (127 - (a & 63) as i32)) >> (7 + (a >> 6))
}

/// An attenuation of `steps` 3 dB steps: halved for each two, ×3/4 for an odd one, in one rounding.
#[inline]
pub fn half_octaves(v: i32, steps: u16) -> i32 {
    let shift = (steps >> 1) as u32;
    if shift >= 31 {
        return if v < 0 { -1 } else { 0 };
    }
    if steps & 1 != 0 { (v * 3) >> (shift + 2) } else { v >> shift }
}

/// A send level and pan, together (SDL: 0 off, 1-7 from -36 dB to 0 dB in 6 dB steps; PAN: bit 4
/// picks the side attenuated, the low bits by how many 3 dB steps, 15 silencing it), applied as one
/// attenuation: (left, right).
#[inline]
pub fn send(v: i32, sdl: u16, p: u16) -> (i32, i32) {
    if sdl == 0 {
        return (0, 0);
    }
    let base = 2 * (7 - sdl);
    let full = half_octaves(v, base);
    let low = p & 0xF;
    let att = if low == 0xF { 0 } else { half_octaves(v, base + low) };
    if p & 0x10 == 0 { (att, full) } else { (full, att) }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Slot {
    pub active: bool,
    /// The forward position (relative to SA, 14 fraction bits).
    pub phase: u32,
    /// Playing backwards: how far since the backward run began (14 fraction bits).
    pub back: Option<u32>,
    /// The envelope's attenuation (0 loudest, 0x3FF silent) and phase.
    pub eg: u32,
    pub state: Eg,
    /// The LFO: its 8-bit position and the samples counted towards its next step.
    pub lfo_pos: u32,
    pub lfo_count: u32,
    /// The release has ended: the slot stops after this sample.
    pub ending: bool,
}

saved!(Slot { active, phase, back, eg, state, lfo_pos, lfo_count, ending });

/// Samples between the LFO's steps, by LFOF.
const LFO_PERIOD: [u32; 32] = [
    1020, 892, 764, 636, 508, 444, 380, 316, 252, 220, 188, 156, 124, 108, 92, 76, 60, 52, 44, 36, 28, 24, 20, 16, 12, 10, 8, 6, 4,
    3, 2, 1,
];

impl Slot {
    pub fn key_on(&mut self) {
        self.active = true;
        self.phase = 0;
        self.back = None;
        self.eg = 0x280;
        self.state = Eg::Attack;
        self.ending = false;
    }

    pub fn key_off(&mut self) {
        if self.active {
            self.state = Eg::Release;
        }
    }

    /// The sample at the current position (before the envelope), with `modulation` samples added
    /// to the position.
    fn wave(&self, r: &Regs, ram: &[u8], modulation: i32, noise: i32) -> i32 {
        match r.ssctl {
            1 => return noise,
            2 | 3 => return 0,
            _ => {}
        }
        let pos = match self.back {
            Some(q) => (r.lea << 14).wrapping_add(!q),
            None => self.phase,
        };
        let i = ((pos >> 14) as i32).wrapping_add(modulation) as u32;
        let f = ((pos >> 8) & 63) as i32;
        let read = |i: u32| -> i32 {
            let v = if r.pcm8 {
                let b = ram[(r.sa.wrapping_add(i) & 0x7_FFFF) as usize];
                (b as u16) << 8
            } else {
                rd16(ram, (r.sa.wrapping_add(i.wrapping_mul(2)) & 0x7_FFFE) as usize)
            };
            let v = if r.sbctl & 1 != 0 { v ^ 0x7FFF } else { v };
            let v = if r.sbctl & 2 != 0 { v ^ 0x8000 } else { v };
            v as i16 as i32
        };
        let (s0, s1) = (read(i), read(i.wrapping_add(1)));
        s0 + (((s1 - s0) * f) >> 6)
    }

    /// The LFO's value this sample: (pitch, amplitude) offsets.
    fn lfo(&mut self, r: &Regs, noise: u32) -> (i32, u32) {
        if r.lfore {
            self.lfo_pos = 0;
            self.lfo_count = 0;
        } else {
            self.lfo_count += 1;
            if self.lfo_count >= LFO_PERIOD[r.lfof as usize] {
                self.lfo_count = 0;
                self.lfo_pos = (self.lfo_pos + 1) & 0xFF;
            }
        }
        let p = self.lfo_pos as i32;
        let wave = |ws: u16| -> i32 {
            match ws {
                0 => p - 128,                              // saw
                1 => if p < 128 { 127 } else { -128 },     // square
                2 => if p < 128 { 2 * p - 128 } else { 383 - 2 * p }, // triangle
                _ => (noise & 0xFF) as i32 - 128,          // noise
            }
        };
        let pitch = if r.plfos != 0 { (wave(r.plfows) << r.plfos) >> 7 } else { 0 };
        let amp = if r.alfos != 0 { (((wave(r.alfows) + 128) as u32) << r.alfos) >> 6 } else { 0 };
        (pitch, amp)
    }

    /// The envelope's clock (`count`: the clock's count).
    pub fn clock(&mut self, r: &Regs, count: u32) {
        if self.active {
            self.envelope(r, count);
            // LPSLNK: the attack ends (after this clock) once the sample has reached the loop
            if r.lpslnk && self.state == Eg::Attack && (self.back.is_some() || self.phase >> 14 >= r.lsa) {
                self.state = Eg::Decay1;
            }
        }
    }

    /// One sample: the slot's output (after the envelope and TL), then the position moves on.
    pub fn sample(&mut self, r: &Regs, ram: &[u8], modulation: i32, noise: u32) -> i32 {
        if !self.active {
            return 0;
        }
        self.wrap(r);
        if !self.active {
            return 0;
        }
        let (pitch_lfo, amp_lfo) = self.lfo(r, noise);
        let v = self.wave(r, ram, modulation, (noise as u16) as i16 as i32);
        let out = if r.sdir {
            v
        } else {
            let eg = if r.eghold && self.state == Eg::Attack { 0 } else { self.eg };
            attenuate(v, (eg + ((r.tl as u32) << 2) + amp_lfo).min(0x3FF))
        };
        self.advance(r, pitch_lfo);
        if self.ending {
            self.ending = false;
            self.active = false;
        }
        out
    }

    /// The loop, before the sample is read: where the position has reached LEA (LSA for a reverse
    /// loop) it goes round, turns back, or (no loop) the slot stops. Checked on the position as it
    /// is, so moving LEA under a playing slot acts at once.
    fn wrap(&mut self, r: &Regs) {
        let len = r.lea.wrapping_sub(r.lsa) << 14;
        if let Some(q) = self.back {
            if q >= len && len > 0 {
                if r.lpctl == 3 {
                    self.back = None;
                    self.phase = (r.lsa << 14) + (q - len);
                } else {
                    self.back = Some(q - len);
                }
            }
            return;
        }
        let pos = self.phase >> 14;
        match r.lpctl {
            0 => {
                if pos >= r.lea {
                    self.active = false;
                    self.eg = 0x3FF;
                    self.state = Eg::Release;
                }
            }
            1 => {
                if pos >= r.lea && len > 0 {
                    self.phase -= len;
                }
            }
            2 => {
                if pos >= r.lsa {
                    self.back = Some(self.phase - (r.lsa << 14));
                }
            }
            _ => {
                if pos >= r.lea {
                    self.back = Some(self.phase - (r.lea << 14));
                }
            }
        }
    }

    /// The position moves on by the pitch's step.
    fn advance(&mut self, r: &Regs, pitch_lfo: i32) {
        let step = r.step();
        let step = if pitch_lfo != 0 { ((step as i64 * (1024 + pitch_lfo) as i64) >> 10) as u32 } else { step };
        match &mut self.back {
            Some(q) => *q += step,
            None => self.phase = self.phase.wrapping_add(step),
        }
    }

    fn envelope(&mut self, r: &Regs, count: u32) {
        match self.state {
            Eg::Attack => {
                let inc = eg_inc(r.rate(r.ar), count) as i32;
                let a = self.eg as i32 + ((!(self.eg as i32) * inc) >> 4);
                if a <= 0 {
                    self.eg = 0;
                    if !r.lpslnk {
                        self.state = Eg::Decay1;
                    }
                } else {
                    self.eg = a as u32;
                }
            }
            Eg::Decay1 => {
                self.eg = (self.eg + eg_inc(r.rate(r.d1r), count)).min(0x3FF);
                // DL is met, not passed: a decay starting beyond it (LPSLNK) stays in decay 1
                if (self.eg >> 5) == r.dl as u32 {
                    self.state = Eg::Decay2;
                }
            }
            Eg::Decay2 => self.eg = (self.eg + eg_inc(r.rate(r.d2r), count)).min(0x3FF),
            Eg::Release => {
                self.eg = (self.eg + eg_inc(r.rate(r.rr), count)).min(0x3FF);
                // the slot ends when its release reaches 0x3C0 (after this sample)
                if self.eg >= 0x3C0 {
                    self.eg = 0x3FF;
                    self.ending = true;
                }
            }
        }
    }

    /// The slot monitor: CA (the position's bits 12-15), SGC, EG (the attenuation's top 5 bits).
    pub fn monitor(&self, r: &Regs) -> u16 {
        let pos = match self.back {
            Some(q) => r.lea.wrapping_sub(q >> 14),
            None => self.phase >> 14,
        };
        let eg = if self.active { self.eg } else { 0x3FF };
        ((((pos >> 12) & 0xF) as u16) << 7) | ((self.state as u16) << 5) | ((eg >> 5) & 0x1F) as u16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_match_the_oracle() {
        // a constant 0x4000 through TL (tools/scsp/probe.py)
        let tl = |t: u32| attenuate(0x4000, t << 2);
        assert_eq!([tl(0), tl(1), tl(4), tl(16), tl(64), tl(128)], [16256, 15744, 14208, 8128, 1016, 63]);
        assert_eq!(attenuate(-0x8000, 0), -32512);
        assert_eq!(send(16256, 7, 1), (12192, 16256));
        assert_eq!(send(16256, 7, 0x13), (16256, 6096));
        assert_eq!(send(22571, 6, 1), (8464, 11285));
        assert_eq!(half_octaves(16256, 14), 127);
    }

    #[test]
    fn attack_from_0x280() {
        let r = Regs { ar: 31, krs: 15, dl: 0, ..Default::default() };
        let mut s = Slot::default();
        s.key_on();
        let mut seen = vec![s.eg];
        for c in 1..10 {
            s.envelope(&r, c);
            seen.push(s.eg);
        }
        assert_eq!(seen, [640, 319, 159, 79, 39, 19, 9, 4, 1, 0]);
    }
}
