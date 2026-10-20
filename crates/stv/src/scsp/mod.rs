//! The SCSP (YMF292): 512 KB of sound RAM, 32 slots (slot.rs), the effect DSP (dsp.rs), the mix,
//! timers A, B and C, the interrupt registers for the 68000 (SCIEB, SCIPD, SCIRE, SCILV0-2) and for
//! the main CPU (MCIEB, MCIPD, MCIRE, through the SCU's sound request), and the slot monitor. From
//! the SCSP user's manual, with the details measured on the oracle's SCSP (tools/scsp/probe.py) and
//! checked against its output (stv-check scsp). Registers at 0x100000 on the 68000's bus, 0x05B00000
//! on the SH-2s'.

mod dsp;
pub mod slot;

use crate::mem::{rd16, wr16};
use crate::state::saved;
use dsp::Dsp;
use slot::{Regs, Slot};

/// Interrupt sources (bits of SCIPD and MCIPD).
const INT_CPU: u16 = 1 << 5;
const INT_TIMER_A: u16 = 1 << 6;
const INT_SAMPLE: u16 = 1 << 10;

/// Common control registers, as offsets in `regs`.
const MVOL: usize = 0x400;
const RBL_RBP: usize = 0x402;
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
    slots: [Slot; 32],
    /// Samples made since the reset (the envelope clock counts every second one).
    samples: u64,
    /// KYONEX written: the slots key on and off after the next envelope clock.
    key_pending: bool,
    /// The sound stack: each slot's last two outputs, for frequency modulation.
    stack: [i16; 64],
    stack_pos: usize,
    dsp: Dsp,
    /// The slots' sends this sample, for the DSP's next.
    mixs_next: [i32; 16],
    /// The noise generator (a 17-bit LFSR).
    noise: u32,
    /// Samples counted towards each timer's next step (timers A, B, C).
    prescale: [u32; 3],
    /// The main CPU's sound request went from clear to pending (the SCU raises its interrupt).
    pub main_irq_edge: bool,
    /// The samples made (left, right) since the client last took them.
    pub out: Vec<[i16; 2]>,
}

saved!(Scsp { ram, regs, slots, samples, key_pending, stack, stack_pos, dsp, mixs_next, noise, prescale, main_irq_edge } skip { out });

impl Default for Scsp {
    fn default() -> Scsp {
        Scsp::new()
    }
}

impl Scsp {
    pub fn new() -> Scsp {
        Scsp {
            ram: vec![0; 0x8_0000],
            regs: vec![0; 0x1000],
            slots: [Slot::default(); 32],
            samples: 0,
            key_pending: false,
            stack: [0; 64],
            stack_pos: 0,
            dsp: Dsp::default(),
            mixs_next: [0; 16],
            noise: 1,
            prescale: [0; 3],
            main_irq_edge: false,
            out: Vec::new(),
        }
    }

    /// A reset (the SMPC's sound control): everything but sound RAM and the samples not yet taken.
    pub fn reset(&mut self) {
        let ram = std::mem::take(&mut self.ram);
        let out = std::mem::take(&mut self.out);
        *self = Scsp { ram, out, ..Scsp::new() };
    }

    /// The sounding slots' state, one line each (development).
    pub fn describe_slots(&self) -> String {
        let mut out = String::new();
        for (n, s) in self.slots.iter().enumerate() {
            if s.active {
                out += &format!("  slot {n:2}: {:?} eg {:#05x} phase {:#x} back {:?}\n", s.state, s.eg, s.phase, s.back);
            }
        }
        out
    }

    fn slot_regs(&self, n: usize) -> Regs {
        Regs::decode(&self.regs[0x20 * n..0x20 * n + 0x20])
    }

    /// KYONEX: every slot keys on or off as its KYONB says.
    fn key_execute(&mut self) {
        for n in 0..32 {
            let on = self.reg(0x20 * n) & 0x0800 != 0;
            let s = &mut self.slots[n];
            if on && (!s.active || s.state == slot::Eg::Release) {
                s.key_on();
            } else if !on {
                s.key_off();
            }
        }
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

    /// One sample (44.1 kHz): the slots, the DSP, the mix; then the timers step at their prescaled
    /// rates (overflows interrupt).
    pub fn sample(&mut self) {
        if self.samples & 1 == 0 {
            let count = (self.samples >> 1) as u32;
            for n in 0..32 {
                let regs = self.slot_regs(n);
                self.slots[n].clock(&regs, count);
            }
        }
        if std::mem::take(&mut self.key_pending) {
            self.key_execute();
        }
        // the noise generator steps once a sample
        let bit = (self.noise ^ (self.noise >> 5)) & 1;
        self.noise = (self.noise >> 1) | (bit << 16);
        // the DSP runs on the sends of the sample before
        self.dsp.mixs = std::mem::take(&mut self.mixs_next);
        self.dsp.run(&mut self.ram);
        let (mut l, mut r) = (0i32, 0i32);
        for n in 0..32 {
            let regs = self.slot_regs(n);
            let modulation = if regs.mdl >= 5 {
                let x = self.stack[(self.stack_pos + regs.mdxsl as usize) & 63] as i32;
                let y = self.stack[(self.stack_pos + regs.mdysl as usize) & 63] as i32;
                ((x + y) / 2) >> (16 - regs.mdl)
            } else {
                0
            };
            let o = self.slots[n].sample(&regs, &self.ram, modulation, self.noise);
            if !regs.stwinh {
                self.stack[self.stack_pos] = o as i16;
            }
            self.stack_pos = (self.stack_pos + 1) & 63;
            if o != 0 {
                let (dl, dr) = slot::send(o, regs.disdl, regs.dipan);
                l += dl;
                r += dr;
                if regs.imxl != 0 {
                    self.mixs_next[regs.isel as usize] += (o << 4) >> (7 - regs.imxl);
                }
            }
        }
        for i in 0..18 {
            let regs = self.slot_regs(i);
            let e = if i < 16 { self.dsp.efreg[i] } else { self.dsp.exts[i - 16] } as i32;
            if e != 0 {
                let (el, er) = slot::send(e, regs.efsdl, regs.efpan);
                l += el;
                r += er;
            }
        }
        let mvol = self.reg(MVOL) & 0xF;
        let master = |v: i32| -> i16 {
            if mvol == 0 { 0 } else { slot::half_octaves(v, 15 - mvol).clamp(-0x8000, 0x7FFF) as i16 }
        };
        if self.out.len() >= 1 << 15 {
            self.out.drain(..1 << 14); // a client not taking them: keep the latest
        }
        self.out.push([master(l), master(r)]);
        self.samples += 1;

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
        // what reads back, as the oracle's SCSP has it (tools/scsp/probe.py): slot registers mask
        // their unused bits, most of the common ones read as 0
        const SLOT_MASK: [u16; 16] =
            [0x0FFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0x0FFF, 0xFFFF, 0xFFFF, 0xFFFF, 0x00FF, 0xFFFF, 0, 0, 0, 0];
        match a {
            0x000..=0x3FF => self.reg(a) & SLOT_MASK[(a & 0x1F) >> 1],
            // the MIDI status: both buffers empty
            0x404 => 0x0900,
            // the slot monitor (MSLC itself does not read back): the slot's CA, SGC and EG
            MSLC => {
                let n = (self.reg(MSLC) >> 11) as usize & 0x1F;
                self.slots[n].monitor(&self.slot_regs(n))
            }
            SCIEB => self.reg(SCIEB) & 0x07FF,
            SCIPD | MCIPD => self.reg(a),
            0x400..=0x5FF => 0,
            0x600..=0x67F => self.stack[(a - 0x600) >> 1] as u16,
            0xC00..=0xDFF => self.dsp_word(self.dsp.temp[(a - 0xC00) >> 2], a),
            0xE00..=0xE7F => self.dsp_word(self.dsp.mems[(a - 0xE00) >> 2], a),
            0xE80..=0xEBF => self.dsp_word(self.dsp.mixs[(a - 0xE80) >> 2], a),
            0xEC0..=0xEDF => self.dsp.efreg[(a - 0xEC0) >> 1] as u16,
            0xEE0..=0xEE3 => self.dsp.exts[(a - 0xEE0) >> 1] as u16,
            _ => self.reg(a),
        }
    }

    /// A 24-bit DSP register as two words: the low 8 bits, then the top 16.
    fn dsp_word(&self, v: i32, a: usize) -> u16 {
        if a & 2 == 0 { (v & 0xFF) as u16 } else { (v >> 8) as u16 }
    }

    pub fn write16(&mut self, a: u32, v: u16, mask: u16) {
        let a = (a & 0xFFE) as usize;
        let old = self.reg(a);
        let nv = (old & !mask) | (v & mask);
        match a {
            0x000..=0x3FF if a & 0x1F == 0 => {
                // KYONEX executes (at the next sample, after its envelope clock) and does not stay
                self.set_reg(a, nv & 0x0FFF);
                if nv & 0x1000 != 0 {
                    self.key_pending = true;
                }
            }
            RBL_RBP => {
                self.set_reg(a, nv);
                self.dsp.rbp = (nv & 0x7F) as u32;
                self.dsp.rbl = ((nv >> 7) & 3) as u32;
            }
            0x600..=0x67F => self.stack[(a - 0x600) >> 1] = nv as i16,
            0x700..=0x77F => {
                self.set_reg(a, nv);
                self.dsp.coef[(a - 0x700) >> 1] = nv;
            }
            0x780..=0x7BF => {
                // (0x7C0-0x7FF is not a mirror of them)
                self.set_reg(a, nv);
                self.dsp.madrs[(a - 0x780) >> 1] = nv;
            }
            0x800..=0xBFF => {
                self.set_reg(a, nv);
                self.dsp.mpro[(a - 0x800) >> 1] = nv;
            }
            0xC00..=0xDFF => {
                let i = (a - 0xC00) >> 2;
                self.dsp.temp[i] = Self::dsp_set(self.dsp.temp[i], a, nv);
            }
            0xE00..=0xE7F => {
                let i = (a - 0xE00) >> 2;
                self.dsp.mems[i] = Self::dsp_set(self.dsp.mems[i], a, nv);
            }
            0xEC0..=0xEDF => self.dsp.efreg[(a - 0xEC0) >> 1] = nv as i16,
            0xEE0..=0xEE3 => self.dsp.exts[(a - 0xEE0) >> 1] = nv as i16,
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

    /// A word written into a 24-bit DSP register (the low 8 bits, or the top 16; sign-extended).
    fn dsp_set(old: i32, a: usize, v: u16) -> i32 {
        let u = if a & 2 == 0 { (old & !0xFF) | (v as i32 & 0xFF) } else { ((v as i32) << 8) | (old & 0xFF) };
        (u << 8) >> 8
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
