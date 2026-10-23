//! The SH7604's on-chip modules, one set per CPU, at 0xFFFFFE00-0xFFFFFFFF: the free-running timer
//! (FRT, whose input capture carries the master/slave signal), the divider (DIVU), the DMA controller
//! (DMAC: its transfers are run by the machine, which owns the bus), the interrupt controller's
//! priority and vector registers, the watchdog, the bus controller and the cache control register.
//! Written from the SH7604 hardware manual; behaviour the manual leaves open follows the oracle.

use crate::state::saved;

/// An interrupt an on-chip module asks for: its level and vector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Irq {
    pub level: u32,
    pub vector: u32,
}

#[derive(Clone, Debug, Default)]
pub struct Frt {
    pub tier: u8,
    pub ftcsr: u8,
    pub frc: u16,
    pub ocra: u16,
    pub ocrb: u16,
    pub tcr: u8,
    pub tocr: u8,
    pub icr: u16,
    /// The byte a read of FRC's or ICR's high half latches for the low half.
    temp: u8,
    /// Cycles counted towards the next FRC tick.
    pub sub: u32,
}

saved!(Frt { tier, ftcsr, frc, ocra, ocrb, tcr, tocr, icr, temp, sub });

#[derive(Clone, Debug, Default)]
pub struct Divu {
    pub dvsr: u32,
    pub dvdnth: u32,
    pub dvdntl: u32,
    pub dvcr: u32,
    pub vcrdiv: u32,
}

saved!(Divu { dvsr, dvdnth, dvdntl, dvcr, vcrdiv });

#[derive(Clone, Copy, Debug, Default)]
pub struct DmaChannel {
    pub sar: u32,
    pub dar: u32,
    pub tcr: u32,
    pub chcr: u32,
    pub vcr: u32,
    pub drcr: u8,
}

saved!(DmaChannel { sar, dar, tcr, chcr, vcr, drcr });

#[derive(Clone, Debug, Default)]
pub struct OnChip {
    pub frt: Frt,
    pub divu: Divu,
    pub dma: [DmaChannel; 2],
    pub dmaor: u32,
    pub ipra: u16,
    pub iprb: u16,
    pub vcra: u16,
    pub vcrb: u16,
    pub vcrc: u16,
    pub vcrd: u16,
    pub vcrwdt: u16,
    pub icr: u16,
    pub wtcsr: u8,
    pub wtcnt: u8,
    pub rstcsr: u8,
    pub sbycr: u8,
    pub ccr: u8,
    pub bsc: [u32; 7],
    /// The slave CPU: BCR1's MASTER bit (bit 15, from a pin) reads 1.
    pub slave: bool,
    /// Registers with no model yet (SCI, the rest): kept so reads return what was written.
    other: Vec<(u32, u8)>,
}

saved!(OnChip { frt, divu, dma, dmaor, ipra, iprb, vcra, vcrb, vcrc, vcrd, vcrwdt, icr, wtcsr, wtcnt, rstcsr, sbycr, ccr, bsc, slave, other });

/// FTCSR flags.
const ICF: u8 = 0x80;
const OCFA: u8 = 0x08;
const OCFB: u8 = 0x04;
const OVF: u8 = 0x02;
const CCLRA: u8 = 0x01;

impl OnChip {
    pub fn new(slave: bool) -> OnChip {
        let mut o = OnChip { slave, ..OnChip::default() };
        o.reset();
        o
    }

    /// Power-on values (the hardware manual's register tables).
    pub fn reset(&mut self) {
        *self = OnChip { other: std::mem::take(&mut self.other), slave: self.slave, ..OnChip::default() };
        self.other.clear();
        self.frt.tier = 0x01;
        self.frt.ocra = 0xFFFF;
        self.frt.ocrb = 0xFFFF;
        self.frt.tocr = 0xE0;
        self.wtcsr = 0x18;
        self.rstcsr = 0x1F;
        self.ccr = 0;
        self.bsc = [0x03F0 | (self.slave as u32) << 15, 0x00FC, 0xAAFF, 0x0000, 0x0000, 0x0000, 0x0000];
    }

    // ---- FRT ----------------------------------------------------------------------------------

    /// The FRT's clock divisor as a power of two (8, 32, 128), or None for the external clock (not
    /// connected).
    fn frt_divisor_log2(&self) -> Option<u32> {
        match self.frt.tcr & 3 {
            0 => Some(3),
            1 => Some(5),
            2 => Some(7),
            _ => None,
        }
    }

    /// Advances the FRT by `cycles` CPU cycles.
    pub fn clock_frt(&mut self, cycles: u32) {
        let Some(shift) = self.frt_divisor_log2() else { return };
        self.frt.sub += cycles;
        let ticks = self.frt.sub >> shift;
        self.frt.sub &= (1 << shift) - 1;
        for _ in 0..ticks {
            // compare match is checked on the count reached
            let next = self.frt.frc.wrapping_add(1);
            if self.frt.frc == 0xFFFF {
                self.frt.ftcsr |= OVF;
            }
            self.frt.frc = next;
            if self.frt.frc == self.frt.ocra {
                self.frt.ftcsr |= OCFA;
                if self.frt.ftcsr & CCLRA != 0 {
                    self.frt.frc = 0;
                }
            }
            if self.frt.frc == self.frt.ocrb {
                self.frt.ftcsr |= OCFB;
            }
        }
    }

    /// CPU cycles before the FRT next sets a flag (overflow, compare match A or B); u64::MAX when
    /// its clock is not connected.
    pub fn frt_cycles_to_event(&self) -> u64 {
        let Some(shift) = self.frt_divisor_log2() else { return u64::MAX };
        let f = &self.frt;
        let ticks_to = |target: u16| match target.wrapping_sub(f.frc) {
            0 => 0x1_0000,
            d => d as u64,
        };
        let ticks = ticks_to(f.ocra).min(ticks_to(f.ocrb)).min(0x1_0000 - f.frc as u64);
        (ticks << shift) - f.sub as u64
    }

    /// Some FRT interrupt is enabled.
    pub fn frt_interrupts_on(&self) -> bool {
        self.frt.tier & (ICF | OCFA | OCFB | OVF) != 0
    }

    /// The other CPU wrote MINIT/SINIT: an input capture.
    pub fn input_capture(&mut self) {
        self.frt.icr = self.frt.frc;
        self.frt.ftcsr |= ICF;
    }

    // ---- DIVU ---------------------------------------------------------------------------------

    /// A 64/32 signed division of DVDNTH:DVDNTL by DVSR: the quotient to DVDNTL, the remainder to
    /// DVDNTH; on overflow (a zero divisor, or a quotient outside 32 bits) OVF is set and the
    /// quotient saturates.
    fn divide(&mut self) {
        let d = &mut self.divu;
        let dividend = (((d.dvdnth as u64) << 32) | d.dvdntl as u64) as i64;
        let divisor = d.dvsr as i32 as i64;
        let q = if divisor == 0 { None } else { dividend.checked_div(divisor) };
        match q {
            Some(q) if q >= i32::MIN as i64 && q <= i32::MAX as i64 => {
                d.dvdntl = q as i32 as u32;
                d.dvdnth = (dividend - q * divisor) as i32 as u32;
            }
            _ => {
                d.dvcr |= 1;
                let negative = (dividend < 0) != (divisor < 0);
                d.dvdntl = if negative { 0x8000_0000 } else { 0x7FFF_FFFF };
            }
        }
    }

    // ---- interrupts ---------------------------------------------------------------------------

    /// The highest-priority interrupt the modules ask for, if any.
    pub fn pending_irq(&self) -> Option<Irq> {
        // TIER's enable bits sit where FTCSR's flags do
        if self.frt.tier & self.frt.ftcsr & (ICF | OCFA | OCFB | OVF) == 0
            && self.divu.dvcr & 3 != 3
            && self.dma.iter().all(|ch| ch.chcr & 0x6 != 0x6)
        {
            return None;
        }
        let mut best: Option<Irq> = None;
        let mut offer = |level: u32, vector: u32| {
            if level > 0 && best.is_none_or(|b| level > b.level) {
                best = Some(Irq { level, vector });
            }
        };
        let f = &self.frt;
        let frt_level = ((self.iprb >> 8) & 0xF) as u32;
        if f.tier & 0x80 != 0 && f.ftcsr & ICF != 0 {
            offer(frt_level, ((self.vcrc >> 8) & 0x7F) as u32);
        }
        if (f.tier & 0x08 != 0 && f.ftcsr & OCFA != 0) || (f.tier & 0x04 != 0 && f.ftcsr & OCFB != 0) {
            offer(frt_level, (self.vcrc & 0x7F) as u32);
        }
        if f.tier & 0x02 != 0 && f.ftcsr & OVF != 0 {
            offer(frt_level, ((self.vcrd >> 8) & 0x7F) as u32);
        }
        if self.divu.dvcr & 3 == 3 {
            offer(((self.ipra >> 12) & 0xF) as u32, self.divu.vcrdiv & 0x7F);
        }
        for ch in &self.dma {
            if ch.chcr & 0x6 == 0x6 {
                offer(((self.ipra >> 8) & 0xF) as u32, ch.vcr & 0x7F);
            }
        }
        best
    }

    // ---- DMAC ---------------------------------------------------------------------------------

    /// A channel ready to transfer: enabled, auto-requested, the DMAC on and not halted.
    pub fn dma_ready(&self, ch: usize) -> bool {
        let c = &self.dma[ch];
        c.chcr & 0x1 != 0 && c.chcr & 0x2 == 0 && c.chcr & 0x200 != 0 && self.dmaor & 0x7 == 0x1
    }

    // ---- registers ----------------------------------------------------------------------------

    pub fn read8(&mut self, a: u32) -> u8 {
        let a = a & 0x1FF;
        match a {
            0x010 => self.frt.tier | 0x01,
            0x011 => self.frt.ftcsr,
            0x012 => {
                self.frt.temp = self.frt.frc as u8;
                (self.frt.frc >> 8) as u8
            }
            0x013 => self.frt.temp,
            0x014 => (self.ocr() >> 8) as u8,
            0x015 => self.ocr() as u8,
            0x016 => self.frt.tcr,
            0x017 => self.frt.tocr | 0xE0,
            0x018 => {
                self.frt.temp = self.frt.icr as u8;
                (self.frt.icr >> 8) as u8
            }
            0x019 => self.frt.temp,
            0x071 => self.dma[0].drcr,
            0x072 => self.dma[1].drcr,
            0x080 => self.wtcsr,
            0x081 => self.wtcnt,
            0x083 => self.rstcsr,
            0x091 => self.sbycr,
            0x092 => self.ccr & !0x10,
            0x060..=0x069 | 0x0E0..=0x0E5 => {
                let w = self.read16(a & !1);
                if a & 1 == 0 { (w >> 8) as u8 } else { w as u8 }
            }
            0x100.. => {
                let l = self.read32(a & !3);
                (l >> (8 * (3 - (a & 3)))) as u8
            }
            _ => self.other.iter().find(|(o, _)| *o == a).map_or(0, |(_, v)| *v),
        }
    }

    fn ocr(&self) -> u16 {
        if self.frt.tocr & 0x10 == 0 { self.frt.ocra } else { self.frt.ocrb }
    }

    pub fn read16(&mut self, a: u32) -> u16 {
        let a = a & 0x1FE;
        match a {
            0x060 => self.iprb,
            0x062 => self.vcra,
            0x064 => self.vcrb,
            0x066 => self.vcrc,
            0x068 => self.vcrd,
            0x0E0 => self.icr,
            0x0E2 => self.ipra,
            0x0E4 => self.vcrwdt,
            0x100.. => {
                let l = self.read32(a & !3);
                if a & 2 == 0 { (l >> 16) as u16 } else { l as u16 }
            }
            _ => ((self.read8(a) as u16) << 8) | self.read8(a + 1) as u16,
        }
    }

    pub fn read32(&mut self, a: u32) -> u32 {
        let a = a & 0x1FC;
        match a {
            0x100..=0x13F => {
                let d = &self.divu;
                match a & 0x1C {
                    0x00 => d.dvsr,
                    0x04 | 0x14 | 0x1C => d.dvdntl,
                    0x08 => d.dvcr,
                    0x0C => d.vcrdiv,
                    _ => d.dvdnth, // 0x10, 0x18
                }
            }
            0x180 => self.dma[0].sar,
            0x184 => self.dma[0].dar,
            0x188 => self.dma[0].tcr,
            0x18C => self.dma[0].chcr,
            0x190 => self.dma[1].sar,
            0x194 => self.dma[1].dar,
            0x198 => self.dma[1].tcr,
            0x19C => self.dma[1].chcr,
            0x1A0 => self.dma[0].vcr,
            0x1A8 => self.dma[1].vcr,
            0x1B0 => self.dmaor,
            0x1E0..=0x1F8 => self.bsc[((a - 0x1E0) / 4) as usize],
            _ => ((self.read16(a) as u32) << 16) | self.read16(a + 2) as u32,
        }
    }

    pub fn write8(&mut self, a: u32, v: u8) {
        let a = a & 0x1FF;
        match a {
            0x010 => self.frt.tier = v | 0x01,
            0x011 => self.frt.ftcsr = (self.frt.ftcsr & v & (ICF | OCFA | OCFB | OVF)) | (v & CCLRA),
            0x012 => self.frt.temp = v,
            0x013 => self.frt.frc = ((self.frt.temp as u16) << 8) | v as u16,
            0x014 => self.frt.temp = v,
            0x015 => {
                let w = ((self.frt.temp as u16) << 8) | v as u16;
                if self.frt.tocr & 0x10 == 0 { self.frt.ocra = w } else { self.frt.ocrb = w }
            }
            0x016 => self.frt.tcr = v,
            0x017 => self.frt.tocr = v | 0xE0,
            0x071 => self.dma[0].drcr = v,
            0x072 => self.dma[1].drcr = v,
            0x091 => self.sbycr = v,
            0x092 => self.ccr = v & !0x10, // CP (purge) reads back 0: there is no cache to purge
            0x060..=0x069 | 0x0E0..=0x0E5 => {
                let w = self.read16(a & !1);
                let nw = if a & 1 == 0 { (w & 0x00FF) | ((v as u16) << 8) } else { (w & 0xFF00) | v as u16 };
                self.write16(a & !1, nw);
            }
            0x100.. => {
                // byte writes to the 32-bit area: not expected; keep what the register holds
            }
            _ => match self.other.iter_mut().find(|(o, _)| *o == a) {
                Some(e) => e.1 = v,
                None => self.other.push((a, v)),
            },
        }
    }

    pub fn write16(&mut self, a: u32, v: u16) {
        let a = a & 0x1FE;
        match a {
            0x060 => self.iprb = v,
            0x062 => self.vcra = v,
            0x064 => self.vcrb = v,
            0x066 => self.vcrc = v,
            0x068 => self.vcrd = v,
            0x080 => match v >> 8 {
                0x5A => self.wtcnt = v as u8,
                0xA5 => self.wtcsr = (v as u8) | 0x18,
                _ => {}
            },
            0x082 => {
                if v >> 8 == 0x5A {
                    self.rstcsr = (v as u8) | 0x1F;
                }
            }
            0x0E0 => self.icr = v,
            0x0E2 => self.ipra = v,
            0x0E4 => self.vcrwdt = v,
            0x100.. => {
                let l = self.read32(a & !3);
                let nl = if a & 2 == 0 { (l & 0xFFFF) | ((v as u32) << 16) } else { (l & 0xFFFF_0000) | v as u32 };
                self.write32(a & !3, nl);
            }
            _ => {
                self.write8(a, (v >> 8) as u8);
                self.write8(a + 1, v as u8);
            }
        }
    }

    pub fn write32(&mut self, a: u32, v: u32) {
        let a = a & 0x1FC;
        match a {
            0x100..=0x13F => match a & 0x1C {
                0x00 => self.divu.dvsr = v,
                0x04 => {
                    self.divu.dvdntl = v;
                    self.divu.dvdnth = ((v as i32) >> 31) as u32;
                    self.divide();
                }
                0x08 => self.divu.dvcr = v & 3,
                0x0C => self.divu.vcrdiv = v & 0xFFFF,
                0x10 | 0x18 => self.divu.dvdnth = v,
                _ => {
                    self.divu.dvdntl = v;
                    self.divide();
                }
            },
            0x180 => self.dma[0].sar = v,
            0x184 => self.dma[0].dar = v,
            0x188 => self.dma[0].tcr = v & 0xFF_FFFF,
            0x18C => self.dma[0].chcr = self.chcr_write(0, v),
            0x190 => self.dma[1].sar = v,
            0x194 => self.dma[1].dar = v,
            0x198 => self.dma[1].tcr = v & 0xFF_FFFF,
            0x19C => self.dma[1].chcr = self.chcr_write(1, v),
            0x1A0 => self.dma[0].vcr = v & 0x7F,
            0x1A8 => self.dma[1].vcr = v & 0x7F,
            0x1B0 => self.dmaor = (self.dmaor & v & 0x6) | (v & 0x9),
            0x1E0..=0x1F8 => {
                if v >> 16 == 0xA55A {
                    let i = ((a - 0x1E0) / 4) as usize;
                    self.bsc[i] = if i == 0 { (self.bsc[0] & 0x8000) | (v & 0x1FF7) } else { v & 0xFFFF };
                }
            }
            _ => {
                self.write16(a, (v >> 16) as u16);
                self.write16(a + 2, v as u16);
            }
        }
    }

    /// CHCR: TE (bit 1) is cleared by writing 0 after reading 1.
    fn chcr_write(&self, ch: usize, v: u32) -> u32 {
        let old = self.dma[ch].chcr;
        (v & 0xFFFD) | (old & v & 0x2)
    }
}
