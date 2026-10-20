//! The SCU (System Control Unit) at 0x05FE0000: the interrupt controller in front of the master SH-2,
//! three DMA levels (run by the machine, which owns the bus), the two timers, and the DSP's ports
//! (the DSP itself is not modelled: the survey has not seen it used). From the SCU user's manual;
//! where the manual is silent, the oracle.

/// The SCU's interrupt sources: (IST bit, vector, level).
pub const VBLANK_IN: u32 = 0;
pub const VBLANK_OUT: u32 = 1;
pub const HBLANK_IN: u32 = 2;
pub const TIMER0: u32 = 3;
pub const TIMER1: u32 = 4;
pub const DSP_END: u32 = 5;
pub const SOUND_REQUEST: u32 = 6;
pub const SMPC: u32 = 7;
pub const PAD: u32 = 8;
pub const DMA2_END: u32 = 9;
pub const DMA1_END: u32 = 10;
pub const DMA0_END: u32 = 11;
pub const DMA_ILLEGAL: u32 = 12;
pub const SPRITE_END: u32 = 13;

const LEVELS: [u32; 14] = [0xF, 0xE, 0xD, 0xC, 0xB, 0xA, 0x9, 0x8, 0x8, 0x6, 0x6, 0x5, 0x3, 0x2];

/// DMA start factors (DnMD bits 0-2).
pub const FACTOR_VBLANK_IN: u32 = 0;
pub const FACTOR_VBLANK_OUT: u32 = 1;
pub const FACTOR_HBLANK_IN: u32 = 2;
pub const FACTOR_TIMER0: u32 = 3;
pub const FACTOR_TIMER1: u32 = 4;
pub const FACTOR_SOUND: u32 = 5;
pub const FACTOR_SPRITE_END: u32 = 6;
pub const FACTOR_GO: u32 = 7;

#[derive(Clone, Copy, Debug, Default)]
pub struct DmaLevel {
    pub read: u32,
    pub write: u32,
    pub count: u32,
    pub add: u32,
    pub enable: u32,
    pub mode: u32,
    /// Set when the level should run (its start factor came, or GO was written).
    pub start: bool,
}

#[derive(Clone, Debug)]
pub struct Scu {
    pub dma: [DmaLevel; 3],
    pub ims: u32,
    pub ist: u32,
    pub t0c: u32,
    pub t1s: u32,
    pub t1md: u32,
    pub aiack: u32,
    pub asr: [u32; 2],
    pub aref: u32,
    pub rsel: u32,
    pub dsp: [u32; 4],
    /// Timer 0's line count since VBlank-OUT.
    pub t0_count: u32,
    pub t0_matched_line: bool,
}

impl Default for Scu {
    fn default() -> Scu {
        Scu {
            dma: [DmaLevel::default(); 3],
            ims: 0xBFFF,
            ist: 0,
            t0c: 0,
            t1s: 0,
            t1md: 0,
            aiack: 0,
            asr: [0; 2],
            aref: 0,
            rsel: 0,
            dsp: [0; 4],
            t0_count: 0,
            t0_matched_line: false,
        }
    }
}

impl Scu {
    pub fn raise(&mut self, source: u32) {
        self.ist |= 1 << source;
    }

    /// The level the SCU presents on the master's IRL lines: the highest unmasked pending source.
    pub fn pending(&self) -> Option<(u32, u32)> {
        let live = self.ist & !self.ims & 0x3FFF;
        if live == 0 {
            return None;
        }
        // the levels fall with the bit number, so the lowest pending bit wins (ties too)
        let bit = live.trailing_zeros();
        Some((LEVELS[bit as usize], bit))
    }

    /// The master fetches the vector for the interrupt it accepted: the source's pending bit clears.
    pub fn acknowledge(&mut self, source: u32) -> u32 {
        self.ist &= !(1 << source);
        0x40 + source
    }

    /// A start factor happened: levels waiting for it start.
    pub fn trigger(&mut self, factor: u32) {
        for l in &mut self.dma {
            if l.enable & 0x100 != 0 && l.mode & 7 == factor {
                l.start = true;
            }
        }
    }

    pub fn read32(&mut self, a: u32) -> u32 {
        let a = a & 0xFC;
        match a {
            0x00..=0x5C => {
                let l = &self.dma[(a / 0x20) as usize];
                match a & 0x1F {
                    0x00 => l.read,
                    0x04 => l.write,
                    0x08 => l.count,
                    0x0C => l.add,
                    0x10 => l.enable,
                    0x14 => l.mode,
                    _ => 0,
                }
            }
            0x7C => 0, // DSTA: transfers are instant, so never busy
            0x80 => self.dsp[0] & !0x0001_0000,
            0x84..=0x8C => self.dsp[((a - 0x80) / 4) as usize],
            0x90 => self.t0c,
            0x94 => self.t1s,
            0x98 => self.t1md,
            0xA0 => self.ims,
            0xA4 => self.ist,
            0xA8 => self.aiack,
            0xB0 => self.asr[0],
            0xB4 => self.asr[1],
            0xB8 => self.aref,
            0xC4 => self.rsel,
            0xC8 => 4, // VER
            _ => 0,
        }
    }

    pub fn write32(&mut self, a: u32, v: u32) {
        let a = a & 0xFC;
        match a {
            0x00..=0x5C => {
                let n = (a / 0x20) as usize;
                let l = &mut self.dma[n];
                match a & 0x1F {
                    0x00 => l.read = v & 0x07FF_FFFF,
                    0x04 => l.write = v & 0x07FF_FFFF,
                    0x08 => l.count = v & if n == 0 { 0xF_FFFF } else { 0xFFF },
                    0x0C => l.add = v & 0x107,
                    0x10 => {
                        l.enable = v & 0x101;
                        if v & 0x101 == 0x101 && l.mode & 7 == FACTOR_GO {
                            l.start = true;
                        }
                    }
                    0x14 => l.mode = v & 0x0101_0107,
                    _ => {}
                }
            }
            0x60 => {
                if v & 1 != 0 {
                    for l in &mut self.dma {
                        l.start = false;
                    }
                }
            }
            0x80..=0x8C => self.dsp[((a - 0x80) / 4) as usize] = v,
            0x90 => self.t0c = v & 0x3FF,
            0x94 => self.t1s = v & 0x1FF,
            0x98 => self.t1md = v & 0x101,
            0xA0 => self.ims = v & 0xBFFF,
            0xA4 => self.ist &= v,
            0xA8 => self.aiack = v & 1,
            0xB0 => self.asr[0] = v,
            0xB4 => self.asr[1] = v,
            0xB8 => self.aref = v & 0x1F,
            0xC4 => self.rsel = v & 1,
            _ => {}
        }
    }

    /// The next line: timer 0 counts lines and asks for its interrupt at the line T0C names; timer 1
    /// fires in every line (or, in mode 1, only in timer 0's line). `vblank_out` restarts the count.
    pub fn line(&mut self, vblank_out: bool) {
        if vblank_out {
            self.t0_count = 0;
        } else {
            self.t0_count = (self.t0_count + 1) & 0x3FF;
        }
        self.t0_matched_line = false;
        if self.t1md & 1 != 0 && self.t0_count == self.t0c {
            self.t0_matched_line = true;
            self.raise(TIMER0);
            self.trigger(FACTOR_TIMER0);
        }
    }

    /// Timer 1's point in the line.
    pub fn timer1(&mut self) {
        if self.t1md & 1 != 0 && (self.t1md & 0x100 == 0 || self.t0_matched_line) {
            self.raise(TIMER1);
            self.trigger(FACTOR_TIMER1);
        }
    }
}
