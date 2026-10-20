//! The ST-V: two SH-2s on one bus with the SCU, SMPC, VDP1, VDP2, SCSP, the I/O chip and the cart,
//! run a line at a time. The client (or a check) owns the clock: `run_frame` runs to the end of the
//! next frame, where the oracle's frames end, so the two can be compared frame by frame.

use crate::m68k::M68k;
use crate::mem::{fnv1a, rd16, rd32, wr16, wr32};
use crate::onchip::OnChip;
use crate::scu::{self, Scu};
use crate::sh2::{Bus, Sh2};
use crate::smpc::{Effect, Smpc};
use crate::scsp::{Scsp, SoundBus};
use crate::video::{Vdp1, Vdp2};

/// NTSC: 263 lines a frame.
const LINES: u32 = 263;
/// The sound CPU's clock: the SCSP's 22.5792 MHz halved; a sample every 256 of its cycles.
const SOUND_HZ: u64 = 11_289_600;

pub struct Sys {
    pub bios: Vec<u8>,
    pub wram_l: Vec<u8>,
    pub wram_h: Vec<u8>,
    pub backup: Vec<u8>,
    pub cart: Vec<u8>,
    pub prot: [u8; 16],
    /// The cart's protection (the 315-5881's game-start check, as the oracle answers it): armed by
    /// writing 1 to 0x04FFFFF0; each 16-bit read at 0x04FFFFFC then gives the next pair of a pattern
    /// (byte i: i & 0xF0 for the first 256, i & 0x0F for the next).
    prot_on: bool,
    prot_counter: u8,
    pub io: [u8; 0x80],
    pub onchip: [OnChip; 2],
    /// Each CPU's cache data array, reachable at 0xC0000000 (4 KB).
    pub cache_data: [Vec<u8>; 2],
    pub scu: Scu,
    pub smpc: Smpc,
    pub vdp1: Vdp1,
    pub vdp2: Vdp2,
    pub scsp: Scsp,
    pub m68k: M68k,
    pub sound_on: bool,
    sound_reset_pending: bool,
    /// The sound CPU's clock: SH-2 cycles owed to it (in units of SOUND_HZ / SH-2 clock), and
    /// its cycles into the current sample.
    sound_owed: u64,
    sound_in_sample: u32,
    pub sound_steps: u64,
    pub slave_on: bool,
    pub clock352: bool,
    /// The line the beam is on (VDP2's V counter).
    pub line: u32,
    /// Frames finished, as the oracle counts them.
    pub frame: u64,
    /// SYSRES, waiting for the machine to carry it out between instructions.
    reset_pending: bool,
    /// An NMI for each CPU (the SMPC wakes the master with one after a clock change).
    nmi_pending: [bool; 2],
    slave_reset_pending: bool,
    /// Cycles of bus waiting the current instruction has added.
    wait: u32,
    /// When on, each SH-2's records in the oracle's trace format (version 3; development).
    pub trace: Option<[Vec<u8>; 2]>,
}

impl Sys {
    fn record(&mut self, cpu: usize, tag: u8, size: u8, a: u32, v: u32) {
        if let Some(t) = &mut self.trace {
            let b = &mut t[cpu];
            b.extend_from_slice(&[tag, size, 0, 0]);
            b.extend_from_slice(&a.to_be_bytes());
            b.extend_from_slice(&v.to_be_bytes());
        }
    }
}

impl Sys {
    fn cycles_per_line(&self) -> u32 {
        if self.clock352 { 1820 } else { 1708 }
    }

    fn cycles_per_second(&self) -> u64 {
        self.cycles_per_line() as u64 * LINES as u64 * 5994 / 100
    }

    /// The SH-2s' clock: 28.6364 MHz for the 352-dot modes, 26.8466 MHz for 320.
    fn sh2_hz(&self) -> u64 {
        if self.clock352 { 28_636_363 } else { 26_846_590 }
    }

    /// Runs the sound CPU and the SCSP for `sh2_cycles` of the main clock.
    fn run_sound(&mut self, sh2_cycles: u32) {
        self.sound_owed += sh2_cycles as u64 * SOUND_HZ;
        let cycles = (self.sound_owed / self.sh2_hz()) as u32;
        self.sound_owed %= self.sh2_hz();
        let mut left = cycles;
        while left > 0 {
            let chunk = left.min(256 - self.sound_in_sample);
            if self.sound_on {
                let mut done = 0;
                while done < chunk {
                    self.m68k.ipl = self.scsp.ipl();
                    let mut bus = SoundBus { scsp: &mut self.scsp };
                    done += self.m68k.step(&mut bus).max(4);
                    self.sound_steps += 1;
                }
            }
            self.sound_in_sample += chunk;
            left -= chunk;
            if self.sound_in_sample >= 256 {
                self.sound_in_sample = 0;
                self.scsp.sample();
            }
        }
        if self.scsp.main_irq_edge {
            self.scsp.main_irq_edge = false;
            self.scu.raise(scu::SOUND_REQUEST);
            self.scu.trigger(scu::FACTOR_SOUND);
        }
    }

    // ---- the external bus (also what DMA reaches) ----------------------------------------------

    pub fn ext_read8(&mut self, a: u32) -> u8 {
        let p = a & 0x07FF_FFFF;
        match p {
            0x0000_0000..=0x000F_FFFF => self.bios[(p & 0x7_FFFF) as usize],
            0x0010_0000..=0x0017_FFFF => {
                self.wait += 4;
                if p & 1 != 0 {
                    let v = self.smpc.read(p);
                    self.smpc_ports();
                    v
                } else {
                    0xFF
                }
            }
            0x0018_0000..=0x001F_FFFF => self.backup[(p & 0xFFFF) as usize],
            0x0020_0000..=0x002F_FFFF => {
                self.wait += 4;
                self.wram_l[(p & 0xF_FFFF) as usize]
            }
            0x0040_0000..=0x004F_FFFF => {
                if p & 1 != 0 { self.io_read((p & 0x7F) as usize) } else { 0xFF }
            }
            0x0200_0000..=0x04FF_FFEF => self.cart.get((p - 0x0200_0000) as usize).copied().unwrap_or(0xFF),
            0x04FF_FFF0..=0x04FF_FFFF => {
                let w = self.ext_read16(p & !1);
                if p & 1 == 0 { (w >> 8) as u8 } else { w as u8 }
            }
            0x05A0_0000..=0x05FF_FFFF => {
                let w = self.ext_read16(p & !1);
                if p & 1 == 0 { (w >> 8) as u8 } else { w as u8 }
            }
            0x0600_0000..=0x07FF_FFFF => self.wram_h[(p & 0xF_FFFF) as usize],
            _ => 0,
        }
    }

    pub fn ext_read16(&mut self, a: u32) -> u16 {
        let p = a & 0x07FF_FFFE;
        match p {
            0x05A0_0000..=0x05AF_FFFF => self.scsp.read16_bus(p & 0x7_FFFF),
            0x05B0_0000..=0x05BF_FFFF => self.scsp.read16_bus(0x10_0000 | (p & 0xFFF)),
            0x05C0_0000..=0x05C7_FFFF => rd16(&self.vdp1.vram, (p & 0x7_FFFF) as usize),
            0x05C8_0000..=0x05CF_FFFF => rd16(&self.vdp1.fb[self.vdp1.draw], (p & 0x3_FFFF) as usize),
            0x05D0_0000..=0x05D7_FFFF => self.vdp1.read_reg(p),
            0x05E0_0000..=0x05EF_FFFF => rd16(&self.vdp2.vram, (p & 0x7_FFFF) as usize),
            0x05F0_0000..=0x05F7_FFFF => rd16(&self.vdp2.cram, (p & 0xFFF) as usize),
            0x05F8_0000..=0x05FB_FFFF => self.vdp2.read_reg(p),
            0x05FE_0000..=0x05FE_FFFF => {
                let l = self.scu.read32(p & !3);
                if p & 2 == 0 { (l >> 16) as u16 } else { l as u16 }
            }
            0x0600_0000..=0x07FF_FFFF => rd16(&self.wram_h, (p & 0xF_FFFF) as usize),
            0x04FF_FFF0..=0x04FF_FFFF => self.prot_read(p),
            _ if (0x05A0_0000..0x0600_0000).contains(&p) => 0,
            _ => ((self.ext_read8(p) as u16) << 8) | self.ext_read8(p + 1) as u16,
        }
    }

    pub fn ext_read32(&mut self, a: u32) -> u32 {
        let p = a & 0x07FF_FFFC;
        match p {
            0x05FE_0000..=0x05FE_FFFF => self.scu.read32(p),
            0x0600_0000..=0x07FF_FFFF => rd32(&self.wram_h, (p & 0xF_FFFF) as usize),
            _ => ((self.ext_read16(p) as u32) << 16) | self.ext_read16(p + 2) as u32,
        }
    }

    pub fn ext_write8(&mut self, a: u32, v: u8) {
        let p = a & 0x07FF_FFFF;
        match p {
            0x0010_0000..=0x0017_FFFF => {
                if p & 1 != 0 {
                    let per_ms = (self.cycles_per_second() / 1000) as u32;
                    self.smpc.write(p, v, per_ms);
                    self.smpc_ports();
                }
            }
            0x0018_0000..=0x001F_FFFF => self.backup[(p & 0xFFFF) as usize] = v,
            0x0020_0000..=0x002F_FFFF => self.wram_l[(p & 0xF_FFFF) as usize] = v,
            0x0040_0000..=0x004F_FFFF => self.io[(p & 0x7F) as usize] = v,
            0x0100_0000..=0x01FF_FFFF => self.signal(p),
            0x04FF_FFF0..=0x04FF_FFFF => {
                self.prot[(p & 0xF) as usize] = v;
                if p == 0x04FF_FFF1 {
                    self.prot_on = v & 1 != 0;
                    self.prot_counter = 0;
                }
            }
            0x05A0_0000..=0x05AF_FFFF => self.scsp.write8(p & 0x7_FFFF, v),
            0x05B0_0000..=0x05BF_FFFF => self.scsp.write8(0x10_0000 | (p & 0xFFF), v),
            0x05C0_0000..=0x05C7_FFFF => self.vdp1.vram[(p & 0x7_FFFF) as usize] = v,
            0x05C8_0000..=0x05CF_FFFF => {
                let fb = self.vdp1.draw;
                self.vdp1.fb[fb][(p & 0x3_FFFF) as usize] = v;
            }
            0x05E0_0000..=0x05EF_FFFF => self.vdp2.vram[(p & 0x7_FFFF) as usize] = v,
            0x05F0_0000..=0x05F7_FFFF => self.vdp2.cram[(p & 0xFFF) as usize] = v,
            0x05F8_0000..=0x05FB_FFFF => {
                let (v16, mask) = if p & 1 == 0 { ((v as u16) << 8, 0xFF00) } else { (v as u16, 0x00FF) };
                self.vdp2.write_reg(p, v16, mask);
            }
            0x05D0_0000..=0x05D7_FFFF | 0x05FE_0000..=0x05FE_FFFF => {
                // byte writes to word or long registers: not expected
            }
            0x0600_0000..=0x07FF_FFFF => self.wram_h[(p & 0xF_FFFF) as usize] = v,
            _ => {}
        }
    }

    pub fn ext_write16(&mut self, a: u32, v: u16) {
        let p = a & 0x07FF_FFFE;
        match p {
            0x05A0_0000..=0x05AF_FFFF => self.scsp.write16_bus(p & 0x7_FFFF, v),
            0x05B0_0000..=0x05BF_FFFF => self.scsp.write16_bus(0x10_0000 | (p & 0xFFF), v),
            0x05C0_0000..=0x05C7_FFFF => wr16(&mut self.vdp1.vram, (p & 0x7_FFFF) as usize, v),
            0x05C8_0000..=0x05CF_FFFF => {
                let fb = self.vdp1.draw;
                wr16(&mut self.vdp1.fb[fb], (p & 0x3_FFFF) as usize, v);
            }
            0x05D0_0000..=0x05D7_FFFF => {
                if self.vdp1.write_reg(p, v) {
                    self.vdp1_draw();
                }
            }
            0x05E0_0000..=0x05EF_FFFF => wr16(&mut self.vdp2.vram, (p & 0x7_FFFF) as usize, v),
            0x05F0_0000..=0x05F7_FFFF => wr16(&mut self.vdp2.cram, (p & 0xFFF) as usize, v),
            0x05F8_0000..=0x05FB_FFFF => self.vdp2.write_reg(p, v, 0xFFFF),
            0x05FE_0000..=0x05FE_FFFF => {
                let l = self.scu.read32(p & !3);
                let nl = if p & 2 == 0 { (l & 0xFFFF) | ((v as u32) << 16) } else { (l & 0xFFFF_0000) | v as u32 };
                self.ext_write32(p & !3, nl);
            }
            0x0600_0000..=0x07FF_FFFF => wr16(&mut self.wram_h, (p & 0xF_FFFF) as usize, v),
            _ => {
                self.ext_write8(p, (v >> 8) as u8);
                self.ext_write8(p + 1, v as u8);
            }
        }
    }

    pub fn ext_write32(&mut self, a: u32, v: u32) {
        let p = a & 0x07FF_FFFC;
        match p {
            0x05FE_0000..=0x05FE_FFFF => {
                self.scu.write32(p, v);
                self.run_scu_dma();
            }
            0x0600_0000..=0x07FF_FFFF => wr32(&mut self.wram_h, (p & 0xF_FFFF) as usize, v),
            _ => {
                self.ext_write16(p, (v >> 16) as u16);
                self.ext_write16(p + 2, v as u16);
            }
        }
    }

    fn prot_read(&mut self, p: u32) -> u16 {
        if p >= 0x04FF_FFFC && self.prot_on {
            let c = self.prot_counter;
            self.prot_counter = c.wrapping_add(1);
            let i = ((c & 0x7F) as u16) << 1;
            let mask = if c & 0x80 == 0 { 0xF0F0 } else { 0x0F0F };
            return ((i << 8) | (i + 1)) & mask;
        }
        ((self.prot[(p & 0xE) as usize] as u16) << 8) | self.prot[(p & 0xF | 1) as usize] as u16
    }

    fn io_read(&mut self, r: usize) -> u8 {
        match r {
            0x07 => self.io[r], // the output port reads back
            _ => 0xFF,          // inputs, active low: nothing pressed
        }
    }

    /// A write to MINIT (0x01000000) or SINIT (0x01800000): the slave's or the master's FRT input
    /// capture.
    fn signal(&mut self, p: u32) {
        let target = if p >= 0x0180_0000 { 0 } else { 1 };
        self.onchip[target].input_capture();
    }

    // ---- VDP1 ---------------------------------------------------------------------------------

    fn vdp1_draw(&mut self) {
        let fb = self.vdp1.draw;
        let tvmr = self.vdp1.tvmr;
        let (vram, fbs) = (&self.vdp1.vram, &mut self.vdp1.fb);
        crate::vdp1render::Vdp1Draw::new(vram, &mut fbs[fb], tvmr).run();
        self.vdp1.run_draw();
        self.scu.raise(scu::SPRITE_END);
        self.scu.trigger(scu::FACTOR_SPRITE_END);
        self.run_scu_dma();
    }

    // ---- DMA ----------------------------------------------------------------------------------

    /// Runs every SCU DMA level that has been started.
    fn run_scu_dma(&mut self) {
        for n in 0..3 {
            if !self.scu.dma[n].start {
                continue;
            }
            self.scu.dma[n].start = false;
            let l = self.scu.dma[n];
            let radd = if l.add & 0x100 != 0 { 4 } else { 0 };
            let wadd = if l.add & 7 == 0 { 0 } else { 1u32 << (l.add & 7) };
            let max = if n == 0 { 0x10_0000 } else { 0x1000 };
            if l.mode & 0x0100_0000 == 0 {
                let count = if l.count == 0 { max } else { l.count };
                let (r, w) = self.scu_transfer(l.read, l.write, count, radd, wadd);
                if l.mode & 0x1_0000 != 0 {
                    self.scu.dma[n].read = r;
                }
                if l.mode & 0x100 != 0 {
                    self.scu.dma[n].write = w;
                }
            } else {
                let mut table = l.write;
                for _ in 0..0x1_0000 {
                    let count = self.ext_read32(table);
                    let dst = self.ext_read32(table + 4);
                    let src = self.ext_read32(table + 8);
                    let count = if count == 0 { max } else { count & 0xF_FFFF };
                    self.scu_transfer(src & 0x07FF_FFFF, dst & 0x07FF_FFFF, count, radd, wadd);
                    table += 12;
                    if src & 0x8000_0000 != 0 {
                        break;
                    }
                }
                if l.mode & 0x100 != 0 {
                    self.scu.dma[n].write = table;
                }
            }
            self.scu.raise([scu::DMA0_END, scu::DMA1_END, scu::DMA2_END][n]);
        }
    }

    /// One SCU transfer of `count` bytes: the source as a byte stream (`radd` 4: sequential, 0: the
    /// same long word), the destination in 16-bit units `wadd` apart (`wadd` 4 to work RAM or the
    /// A-bus: sequential). Returns the read and write addresses after it.
    fn scu_transfer(&mut self, src: u32, dst: u32, count: u32, radd: u32, wadd: u32) -> (u32, u32) {
        let b_bus = (0x05A0_0000..0x0600_0000).contains(&(dst & 0x07FF_FFFF));
        let mut w = dst;
        let mut i = 0;
        while i < count {
            let ra = if radd == 4 { src + i } else { src + (i & 3) };
            if b_bus || wadd != 4 {
                let hi = self.ext_read8(ra);
                let lo = self.ext_read8(ra + 1);
                self.ext_write16(w, ((hi as u16) << 8) | lo as u16);
                w = w.wrapping_add(wadd);
                i += 2;
            } else {
                let b = self.ext_read8(ra);
                self.ext_write8(w, b);
                w = w.wrapping_add(1);
                i += 1;
            }
        }
        (src + if radd == 4 { count } else { 0 }, w)
    }

    /// Runs an SH-2 DMAC channel that is ready.
    fn run_sh2_dma(&mut self, cpu: usize) {
        for ch in 0..2 {
            if !self.onchip[cpu].dma_ready(ch) {
                continue;
            }
            let c = self.onchip[cpu].dma[ch];
            let unit: u32 = [1, 2, 4, 16][((c.chcr >> 10) & 3) as usize];
            let tcr = if c.tcr == 0 { 0x100_0000 } else { c.tcr };
            let units = if unit == 16 { tcr.div_ceil(4) } else { tcr };
            let step = |mode: u32| -> i64 {
                match mode {
                    1 => unit as i64,
                    2 => -(unit as i64),
                    _ => 0,
                }
            };
            let (sstep, dstep) = (step((c.chcr >> 12) & 3), step((c.chcr >> 14) & 3));
            let (mut s, mut d) = (c.sar, c.dar);
            for _ in 0..units {
                match unit {
                    1 => {
                        let v = self.ext_read8(s);
                        self.ext_write8(d, v);
                    }
                    2 => {
                        let v = self.ext_read16(s);
                        self.ext_write16(d, v);
                    }
                    _ => {
                        for k in 0..unit / 4 {
                            let v = self.ext_read32(s + 4 * k);
                            self.ext_write32(d + 4 * k, v);
                        }
                    }
                }
                s = (s as i64 + sstep) as u32;
                d = (d as i64 + dstep) as u32;
            }
            let o = &mut self.onchip[cpu].dma[ch];
            o.sar = s;
            o.dar = d;
            o.tcr = 0;
            o.chcr |= 0x2;
        }
    }

    // ---- the CPUs' view ------------------------------------------------------------------------

    fn cpu_read8(&mut self, cpu: usize, a: u32) -> u8 {
        match a >> 29 {
            0 | 1 => self.ext_read8(a),
            6 => self.cache_data[cpu][(a & 0xFFF) as usize],
            7 if a >= 0xFFFF_FE00 => self.onchip[cpu].read8(a),
            _ => 0,
        }
    }

    fn cpu_read16(&mut self, cpu: usize, a: u32) -> u16 {
        match a >> 29 {
            0 | 1 => self.ext_read16(a),
            6 => rd16(&self.cache_data[cpu], (a & 0xFFE) as usize),
            7 if a >= 0xFFFF_FE00 => self.onchip[cpu].read16(a),
            _ => 0,
        }
    }

    fn cpu_read32(&mut self, cpu: usize, a: u32) -> u32 {
        match a >> 29 {
            0 | 1 => self.ext_read32(a),
            6 => rd32(&self.cache_data[cpu], (a & 0xFFC) as usize),
            7 if a >= 0xFFFF_FE00 => self.onchip[cpu].read32(a),
            _ => 0,
        }
    }

    fn cpu_write8(&mut self, cpu: usize, a: u32, v: u8) {
        match a >> 29 {
            0 | 1 => self.ext_write8(a, v),
            6 => self.cache_data[cpu][(a & 0xFFF) as usize] = v,
            7 if a >= 0xFFFF_FE00 => self.onchip[cpu].write8(a, v),
            _ => {}
        }
    }

    fn cpu_write16(&mut self, cpu: usize, a: u32, v: u16) {
        match a >> 29 {
            0 | 1 => self.ext_write16(a, v),
            6 => wr16(&mut self.cache_data[cpu], (a & 0xFFE) as usize, v),
            7 if a >= 0xFFFF_FE00 => self.onchip[cpu].write16(a, v),
            _ => {}
        }
    }

    fn cpu_write32(&mut self, cpu: usize, a: u32, v: u32) {
        match a >> 29 {
            0 | 1 => self.ext_write32(a, v),
            6 => wr32(&mut self.cache_data[cpu], (a & 0xFFC) as usize, v),
            7 if a >= 0xFFFF_FE00 => {
                self.onchip[cpu].write32(a, v);
                self.run_sh2_dma(cpu);
            }
            _ => {}
        }
    }

    // ---- SMPC effects ---------------------------------------------------------------------------

    /// The SMPC's port 2 changed the sound control lines: bit 4 resets the sound CPU, bit 3 the
    /// SCSP; the sound CPU runs while both are low.
    fn smpc_ports(&mut self) {
        for (prev, cur) in std::mem::take(&mut self.smpc.sctrl_events) {
            if (prev ^ cur) & 0x10 != 0 {
                self.sound_reset_pending = true;
            }
            if (prev ^ cur) & 0x08 != 0 {
                let ram = std::mem::take(&mut self.scsp.ram);
                self.scsp = Scsp::new();
                self.scsp.ram = ram;
            }
            self.sound_on = cur == 0;
        }
    }

    fn smpc_effect(&mut self, e: Effect) {
        match e {
            Effect::None => {}
            Effect::SoundOn => {
                self.sound_on = true;
                self.sound_reset_pending = true;
            }
            Effect::SoundOff => self.sound_on = false,
            Effect::Nmi => self.nmi_pending[0] = true,
            Effect::SystemReset => self.reset_pending = true,
            Effect::SlaveOn => {
                self.slave_on = true;
                self.slave_reset_pending = true;
            }
            Effect::SlaveOff => self.slave_on = false,
            Effect::Clock352 | Effect::Clock320 => {
                self.clock352 = e == Effect::Clock352;
                self.slave_on = false;
                self.nmi_pending[0] = true;
            }
        }
    }
}

/// One CPU's bus: the on-chip modules of that CPU, then the shared external bus.
struct CpuBus<'a> {
    sys: &'a mut Sys,
    cpu: usize,
}

impl Bus for CpuBus<'_> {
    fn read8(&mut self, a: u32) -> u8 {
        let v = self.sys.cpu_read8(self.cpu, a);
        self.sys.record(self.cpu, b'R', 1, a, v as u32);
        v
    }
    fn read16(&mut self, a: u32) -> u16 {
        let v = self.sys.cpu_read16(self.cpu, a);
        self.sys.record(self.cpu, b'R', 2, a, v as u32);
        v
    }
    fn read32(&mut self, a: u32) -> u32 {
        let v = self.sys.cpu_read32(self.cpu, a);
        self.sys.record(self.cpu, b'R', 4, a, v);
        v
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.sys.record(self.cpu, b'W', 1, a, v as u32);
        self.sys.cpu_write8(self.cpu, a, v)
    }
    fn write16(&mut self, a: u32, v: u16) {
        self.sys.record(self.cpu, b'W', 2, a, v as u32);
        self.sys.cpu_write16(self.cpu, a, v)
    }
    fn write32(&mut self, a: u32, v: u32) {
        self.sys.record(self.cpu, b'W', 4, a, v);
        self.sys.cpu_write32(self.cpu, a, v)
    }
    fn fetch16(&mut self, a: u32) -> u16 {
        match a >> 29 {
            0 | 1 => {
                let p = a & 0x07FF_FFFE;
                match p {
                    0x0000_0000..=0x000F_FFFF => rd16(&self.sys.bios, (p & 0x7_FFFF) as usize),
                    0x0020_0000..=0x002F_FFFF => rd16(&self.sys.wram_l, (p & 0xF_FFFF) as usize),
                    0x0600_0000..=0x07FF_FFFF => rd16(&self.sys.wram_h, (p & 0xF_FFFF) as usize),
                    _ => self.sys.ext_read16(p),
                }
            }
            6 => rd16(&self.sys.cache_data[self.cpu], (a & 0xFFE) as usize),
            _ => 0,
        }
    }
}

/// The memory hashes the oracle writes each frame (tools/patches/mednafen/oracle.inc), in order:
/// work RAM-L, work RAM-H, VDP1 VRAM, VDP2 VRAM, CRAM, VDP2 registers, sound RAM.
pub struct FrameHashes(pub [u64; 7]);

pub struct Machine {
    pub cpu: [Sh2; 2],
    pub sys: Sys,
    /// Cycles each CPU has run into the current line.
    time: [u32; 2],
    pub steps: [u64; 2],
    /// The last frame's picture: width, height, RGB.
    pub picture: (usize, usize, Vec<u8>),
    /// When on, the addresses each SH-2 has executed (development).
    pub coverage: Option<[std::collections::HashSet<u32>; 2]>,
}

impl Machine {
    /// `eeprom`: the settings EEPROM's contents at power on (the oracle's: tools/oracle/eeprom.py).
    pub fn new(bios: Vec<u8>, cart: Vec<u8>, area: u8, eeprom: Option<&[u8]>) -> Machine {
        let sys = Sys {
            bios,
            wram_l: vec![0; 0x10_0000],
            wram_h: vec![0; 0x10_0000],
            backup: vec![0; 0x1_0000],
            cart,
            prot: [0; 16],
            prot_on: false,
            prot_counter: 0,
            io: [0; 0x80],
            onchip: [OnChip::new(false), OnChip::new(true)],
            cache_data: [vec![0; 0x1000], vec![0; 0x1000]],
            scu: Scu::default(),
            smpc: Smpc::new(area, eeprom),
            vdp1: Vdp1::new(),
            vdp2: Vdp2::new(),
            scsp: Scsp::new(),
            m68k: M68k::default(),
            sound_on: false,
            sound_reset_pending: false,
            sound_owed: 0,
            sound_in_sample: 0,
            sound_steps: 0,
            slave_on: false,
            clock352: false,
            line: 0,
            frame: 0,
            reset_pending: false,
            nmi_pending: [false; 2],
            slave_reset_pending: false,
            wait: 0,
            trace: None,
        };
        let mut m = Machine { cpu: [Sh2::default(), Sh2::default()], sys, time: [0; 2], steps: [0; 2], picture: (0, 0, Vec::new()), coverage: None };
        let mut bus = CpuBus { sys: &mut m.sys, cpu: 0 };
        m.cpu[0].reset(&mut bus, 0);
        m
    }

    /// The line the frame ends on, where the oracle ends its frames: the top blanking's last line
    /// but one (254 with 224 lines of display, 262 with 240 or 256).
    fn frame_end_line(&self) -> u32 {
        if self.sys.vdp2.height() == 224 { 254 } else { 262 }
    }

    /// Runs to the end of the next frame.
    pub fn run_frame(&mut self) {
        loop {
            self.run_line();
            self.sys.line = (self.sys.line + 1) % LINES;
            if self.sys.line == self.frame_end_line() {
                self.sys.frame += 1;
                return;
            }
        }
    }

    fn run_line(&mut self) {
        let line = self.sys.line;
        let height = self.sys.vdp2.height();
        // the vertical events at the line's start
        if line == height {
            self.compose();
            self.sys.vdp2.vblank = true;
            self.sys.scu.raise(scu::VBLANK_IN);
            self.sys.scu.trigger(scu::FACTOR_VBLANK_IN);
        }
        if line == LINES - 1 {
            self.sys.vdp2.vblank = self.sys.vdp2.tvmd() & 0x8000 == 0;
            self.sys.scu.raise(scu::VBLANK_OUT);
            self.sys.scu.trigger(scu::FACTOR_VBLANK_OUT);
            // VDP1's frame change comes with VBlank-OUT (as the oracle has it)
            if self.sys.vdp1.frame_change() {
                self.sys.vdp1_draw();
            }
        }
        self.sys.scu.line(line == LINES - 1);
        self.sys.run_scu_dma();
        let per_line = self.sys.cycles_per_line();
        let width = if self.sys.clock352 { 352 } else { 320 };
        let hblank_at = width * 4;
        let t1_at = (self.sys.scu.t1s * 4).min(per_line - 1);
        self.sys.vdp2.hblank = false;
        // run to timer 1 and HBlank-IN, then to the end of the line
        let mut points = [(t1_at, 1u8), (hblank_at, 2u8)];
        points.sort();
        for (at, what) in points {
            self.run_cpus(at);
            match what {
                1 => self.sys.scu.timer1(),
                _ => {
                    self.sys.vdp2.hblank = true;
                    self.sys.scu.raise(scu::HBLANK_IN);
                    self.sys.scu.trigger(scu::FACTOR_HBLANK_IN);
                }
            }
            self.sys.run_scu_dma();
        }
        self.run_cpus(per_line);
        self.time = [0; 2];
        if self.sys.sound_reset_pending {
            self.sys.sound_reset_pending = false;
            let mut bus = SoundBus { scsp: &mut self.sys.scsp };
            self.sys.m68k.reset(&mut bus);
        }
        self.sys.run_sound(per_line);
        // the SMPC and its clock, once a line
        let (effect, irq) = self.sys.smpc.clock(per_line, self.sys.cycles_per_second());
        if irq {
            self.sys.scu.raise(scu::SMPC);
        }
        self.sys.smpc_effect(effect);
        self.carry_out_resets();
    }

    fn carry_out_resets(&mut self) {
        if self.sys.reset_pending {
            self.sys.reset_pending = false;
            self.sys.scu = Scu::default();
            self.sys.vdp1.reset();
            self.sys.vdp2.reset();
            self.sys.onchip[0].reset();
            self.sys.onchip[1].reset();
            self.sys.slave_on = false;
            let mut bus = CpuBus { sys: &mut self.sys, cpu: 0 };
            self.cpu[0].reset(&mut bus, 2);
        }
        if self.sys.slave_reset_pending {
            self.sys.slave_reset_pending = false;
            self.sys.onchip[1].reset();
            let mut bus = CpuBus { sys: &mut self.sys, cpu: 1 };
            self.cpu[1] = Sh2::default();
            self.cpu[1].reset(&mut bus, 0);
        }
    }

    /// Runs both CPUs to `until` cycles into the line, the slave following the master closely.
    fn run_cpus(&mut self, until: u32) {
        const SLICE: u32 = 32;
        while self.time[0] < until {
            let slice_end = (self.time[0] + SLICE).min(until);
            while self.time[0] < slice_end {
                self.time[0] += self.step(0);
            }
            if self.sys.slave_on {
                while self.time[1] < self.time[0] {
                    self.time[1] += self.step(1);
                }
            } else {
                self.time[1] = self.time[0];
            }
            if self.sys.reset_pending || self.sys.slave_reset_pending {
                self.carry_out_resets();
            }
        }
    }

    /// One instruction (or an interrupt taken) on a CPU; returns the cycles it took.
    fn step(&mut self, c: usize) -> u32 {
        self.sys.wait = 0;
        let cpu = &mut self.cpu[c];
        if cpu.delay.is_none() && self.sys.nmi_pending[c] {
            self.sys.nmi_pending[c] = false;
            let mut bus = CpuBus { sys: &mut self.sys, cpu: c };
            cpu.nmi(&mut bus);
            return 4;
        }
        if cpu.delay.is_none() {
            let onchip = self.sys.onchip[c].pending_irq();
            let external = if c == 0 { self.sys.scu.pending() } else { None };
            let take_external = match (external, onchip) {
                (Some((le, _)), Some(o)) => le >= o.level,
                (Some(_), None) => true,
                _ => false,
            };
            if take_external {
                let (level, source) = external.unwrap_or_default();
                if level > cpu.level() {
                    let vector = self.sys.scu.acknowledge(source);
                    let pc = cpu.pc;
                    self.sys.record(c, b'X', vector as u8, pc, 9);
                    let mut bus = CpuBus { sys: &mut self.sys, cpu: c };
                    cpu.interrupt(&mut bus, vector, level);
                    return 4;
                }
            } else if let Some(o) = onchip
                && o.level > cpu.level()
            {
                let mut bus = CpuBus { sys: &mut self.sys, cpu: c };
                cpu.interrupt(&mut bus, o.vector, o.level);
                return 4;
            }
        }
        if cpu.sleeping {
            self.sys.onchip[c].clock_frt(8);
            return 8;
        }
        if let Some(cov) = &mut self.coverage {
            cov[c].insert(cpu.pc);
        }
        let pc = cpu.pc;
        self.sys.record(c, b'P', 0, pc, 0);
        let mut bus = CpuBus { sys: &mut self.sys, cpu: c };
        cpu.step(&mut bus);
        self.steps[c] += 1;
        let cycles = 1 + self.sys.wait;
        self.sys.onchip[c].clock_frt(cycles);
        cycles
    }

    /// Writes the memories as the oracle's dumps name them (tools/patches/mednafen/oracle.inc).
    pub fn dump(&self, dir: &std::path::Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        let s = &self.sys;
        for (name, data) in [
            ("wram-l.bin", &s.wram_l),
            ("wram-h.bin", &s.wram_h),
            ("vdp1-vram.bin", &s.vdp1.vram),
            ("vdp2-vram.bin", &s.vdp2.vram),
            ("vdp2-cram.bin", &s.vdp2.cram),
            ("vdp2-regs.bin", &s.vdp2.regs),
            ("sound-ram.bin", &s.scsp.ram),
        ] {
            std::fs::write(dir.join(name), data)?;
        }
        // the display state, as the oracle's display/ dumps have it
        let d = dir.join("display");
        std::fs::create_dir_all(&d)?;
        std::fs::write(d.join("vdp2-regs.bin"), &s.vdp2.regs)?;
        std::fs::write(d.join("vdp2-vram.bin"), &s.vdp2.vram)?;
        std::fs::write(d.join("vdp2-cram.bin"), &s.vdp2.cram)?;
        std::fs::write(d.join("vdp1-fb.bin"), &s.vdp1.fb[s.vdp1.draw ^ 1])?;
        std::fs::write(d.join("vdp1-regs.bin"), [s.vdp1.tvmr as u8, s.vdp1.fbcr as u8])?;
        Ok(())
    }

    /// VDP2 composes the frame just displayed (at VBlank-IN, from the state then).
    fn compose(&mut self) {
        let s = &self.sys;
        let frame = crate::vdp2render::Vdp2Frame {
            regs: &s.vdp2.regs,
            vram: &s.vdp2.vram,
            cram: &s.vdp2.cram,
            fb: &s.vdp1.fb[s.vdp1.draw ^ 1],
            tvmr: s.vdp1.tvmr as u8,
        };
        self.picture = (frame.width(), frame.height(), frame.render());
    }

    pub fn hashes(&self) -> FrameHashes {
        let s = &self.sys;
        FrameHashes([
            fnv1a(&s.wram_l),
            fnv1a(&s.wram_h),
            fnv1a(&s.vdp1.vram),
            fnv1a(&s.vdp2.vram),
            fnv1a(&s.vdp2.cram),
            fnv1a(&s.vdp2.regs),
            fnv1a(&s.scsp.ram),
        ])
    }
}
