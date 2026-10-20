//! The SCSP (YMF292) as the CPUs see it: 512 KB of sound RAM and the registers, with the parts the
//! sound driver keeps time and talks by: timers A, B and C, the interrupt registers for the 68000
//! (SCIEB, SCIPD, SCIRE, SCILV0-2) and for the main CPU (MCIEB, MCIPD, MCIRE, through the SCU's sound
//! request), and the slot monitor. No sound is generated yet: every slot reads as released.
//! From the SCSP user's manual; registers at 0x100000 on the 68000's bus, 0x05B00000 on the SH-2s'.

use crate::mem::{rd16, wr16};

/// Interrupt sources (bits of SCIPD and MCIPD).
const INT_CPU: u16 = 1 << 5;
const INT_TIMER_A: u16 = 1 << 6;
const INT_SAMPLE: u16 = 1 << 10;

/// Common control registers, as offsets in `regs`.
const MSLC: usize = 0x408;
const TIMA: usize = 0x418;
const SCIEB: usize = 0x41E;
const SCIPD: usize = 0x420;
const SCIRE: usize = 0x422;
const SCILV0: usize = 0x424;
const MCIEB: usize = 0x42A;
const MCIPD: usize = 0x42C;
const MCIRE: usize = 0x42E;

pub struct Scsp {
    pub ram: Vec<u8>,
    pub regs: Vec<u8>,
    /// Samples counted towards each timer's next step (timers A, B, C).
    prescale: [u32; 3],
    /// The main CPU's sound request went from clear to pending (the SCU raises its interrupt).
    pub main_irq_edge: bool,
}

impl Default for Scsp {
    fn default() -> Scsp {
        Scsp::new()
    }
}

impl Scsp {
    pub fn new() -> Scsp {
        Scsp { ram: vec![0; 0x8_0000], regs: vec![0; 0x1000], prescale: [0; 3], main_irq_edge: false }
    }

    fn reg(&self, a: usize) -> u16 {
        rd16(&self.regs, a)
    }

    fn set_reg(&mut self, a: usize, v: u16) {
        wr16(&mut self.regs, a, v);
    }

    /// Sets interrupt sources pending, for the 68000 and the main CPU.
    fn raise(&mut self, bits: u16) {
        let before = self.reg(MCIPD) & self.reg(MCIEB);
        self.set_reg(SCIPD, self.reg(SCIPD) | bits);
        self.set_reg(MCIPD, self.reg(MCIPD) | bits);
        let after = self.reg(MCIPD) & self.reg(MCIEB);
        if after & !before != 0 {
            self.main_irq_edge = true;
        }
    }

    /// The level the SCSP presents to the 68000: the highest SCILV level of the enabled, pending
    /// sources (sources 7 and up share source 7's level bits).
    pub fn ipl(&self) -> u8 {
        let live = self.reg(SCIPD) & self.reg(SCIEB);
        let mut level = 0;
        for src in 0..11 {
            if live & (1 << src) != 0 {
                let bit = src.min(7);
                let l = (0..3).fold(0u8, |l, k| l | ((((self.reg(SCILV0 + 2 * k) >> bit) & 1) as u8) << k));
                level = level.max(l);
            }
        }
        level
    }

    /// One sample (44.1 kHz): the timers step at their prescaled rates; overflows interrupt.
    pub fn sample(&mut self) {
        let mut bits = INT_SAMPLE;
        for t in 0..3 {
            let a = TIMA + 2 * t;
            let r = self.reg(a);
            let div = 1u32 << ((r >> 8) & 7);
            self.prescale[t] += 1;
            if self.prescale[t] >= div {
                self.prescale[t] = 0;
                let count = (r & 0xFF) + 1;
                if count > 0xFF {
                    bits |= INT_TIMER_A << t;
                }
                self.set_reg(a, (r & 0xFF00) | (count & 0xFF));
            }
        }
        self.raise(bits);
    }

    pub fn read16(&mut self, a: u32) -> u16 {
        let a = (a & 0xFFE) as usize;
        match a {
            // the slot monitor: MSLC as written; CA 0, SGC release, EG silent
            MSLC => (self.reg(MSLC) & 0xF800) | 0x7F,
            0x400 => (self.reg(0x400) & 0xFF0F) | 0x10, // VER 1
            _ => self.reg(a),
        }
    }

    pub fn write16(&mut self, a: u32, v: u16, mask: u16) {
        let a = (a & 0xFFE) as usize;
        let old = self.reg(a);
        let nv = (old & !mask) | (v & mask);
        match a {
            SCIPD => {
                // only the CPU source can be set by writing
                if nv & INT_CPU != 0 {
                    self.raise(INT_CPU);
                }
            }
            MCIPD => {
                if nv & INT_CPU != 0 {
                    self.raise(INT_CPU);
                }
            }
            SCIRE => {
                let p = self.reg(SCIPD) & !(v & mask);
                self.set_reg(SCIPD, p);
            }
            MCIRE => {
                let p = self.reg(MCIPD) & !(v & mask);
                self.set_reg(MCIPD, p);
            }
            _ => self.set_reg(a, nv),
        }
    }

    // ---- the buses: the 68000's (0x000000 sound RAM, 0x100000 registers) and the SH-2s' ----------

    pub fn read8(&mut self, a: u32) -> u8 {
        if a & 0x10_0000 == 0 {
            self.ram[(a & 0x7_FFFF) as usize]
        } else {
            let w = self.read16(a & !1);
            if a & 1 == 0 { (w >> 8) as u8 } else { w as u8 }
        }
    }

    pub fn read16_bus(&mut self, a: u32) -> u16 {
        if a & 0x10_0000 == 0 { rd16(&self.ram, (a & 0x7_FFFE) as usize) } else { self.read16(a) }
    }

    pub fn write8(&mut self, a: u32, v: u8) {
        if a & 0x10_0000 == 0 {
            self.ram[(a & 0x7_FFFF) as usize] = v;
        } else if a & 1 == 0 {
            self.write16(a & !1, (v as u16) << 8, 0xFF00);
        } else {
            self.write16(a & !1, v as u16, 0x00FF);
        }
    }

    pub fn write16_bus(&mut self, a: u32, v: u16) {
        if a & 0x10_0000 == 0 {
            wr16(&mut self.ram, (a & 0x7_FFFE) as usize, v);
        } else {
            self.write16(a, v, 0xFFFF);
        }
    }
}

/// The 68000's bus: the SCSP, and the interrupt acknowledge (autovectored).
pub struct SoundBus<'a> {
    pub scsp: &'a mut Scsp,
}

impl crate::m68k::Bus68 for SoundBus<'_> {
    fn read8(&mut self, a: u32) -> u8 {
        self.scsp.read8(a)
    }
    fn read16(&mut self, a: u32) -> u16 {
        self.scsp.read16_bus(a)
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.scsp.write8(a, v)
    }
    fn write16(&mut self, a: u32, v: u16) {
        self.scsp.write16_bus(a, v)
    }
    fn fetch16(&mut self, a: u32) -> u16 {
        rd16(&self.scsp.ram, (a & 0x7_FFFE) as usize)
    }
}
