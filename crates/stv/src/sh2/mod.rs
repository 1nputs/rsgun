//! The Hitachi SH-2 (SH7604) CPU core: the instruction set, delayed branches and exception entry.
//!
//! Written from the SH-1/SH-2 programming manual's instruction definitions. One call of [`Sh2::step`]
//! executes one instruction; a delayed branch's slot is the next step, as on the chip and in the
//! oracle's traces. The on-chip modules (divider, timers, DMA, interrupt controller, cache) are not
//! here: the CPU reaches them through the [`Bus`] like any other address.

#[cfg(test)]
mod tests;

/// The CPU's view of memory. Data accesses and instruction fetches are told apart: the oracle's
/// traces record data accesses only, and the caches treat them differently.
pub trait Bus {
    fn read8(&mut self, addr: u32) -> u8;
    fn read16(&mut self, addr: u32) -> u16;
    fn read32(&mut self, addr: u32) -> u32;
    fn write8(&mut self, addr: u32, value: u8);
    fn write16(&mut self, addr: u32, value: u16);
    fn write32(&mut self, addr: u32, value: u32);
    fn fetch16(&mut self, addr: u32) -> u16;
    /// Native code (crate::native): the Rust function registered at an entry address, if any.
    fn native_at(&mut self, _addr: u32) -> Option<crate::native::NativeFn> {
        None
    }
    /// Native code's cost in cycles, for the machine's clock (the machine may run on meanwhile).
    fn tick(&mut self, _cycles: u32) {}
    /// Native code ran the block that starts at `at` (and charged it): where it can tell it is
    /// going round a loop that waits.
    fn pass(&mut self, _cpu: &Sh2, _at: u32) {}
    /// Takes the interrupt due, if one is (between instructions): SR and the CPU's PC pushed, PC at
    /// the handler.
    fn take_interrupt(&mut self, _cpu: &mut Sh2) -> bool {
        false
    }
    /// Native code passed a call to the interpreter (counted).
    fn note_interpreted(&mut self) {}
}

pub const SR_T: u32 = 0x001;
pub const SR_S: u32 = 0x002;
pub const SR_I: u32 = 0x0F0;
pub const SR_Q: u32 = 0x100;
pub const SR_M: u32 = 0x200;
/// The bits of SR that exist.
pub const SR_MASK: u32 = 0x3F3;

/// Exception vector numbers the CPU raises itself.
pub const VEC_ILLEGAL: u32 = 4;
pub const VEC_SLOT_ILLEGAL: u32 = 6;
pub const VEC_CPU_ADDRESS_ERROR: u32 = 9;
pub const VEC_NMI: u32 = 11;

/// A delayed branch waiting for its slot: where it goes, and where the branch itself was (the PC a
/// slot-illegal exception saves).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Delay {
    pub target: u32,
    pub branch: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sh2 {
    pub r: [u32; 16],
    /// The address of the next instruction to execute (not the manual's PC, which reads as the
    /// executing instruction's address + 4).
    pub pc: u32,
    pub pr: u32,
    pub sr: u32,
    pub gbr: u32,
    pub vbr: u32,
    pub mach: u32,
    pub macl: u32,
    /// Set by a delayed branch; the next step executes the slot, then jumps.
    pub delay: Option<Delay>,
    /// Set by SLEEP until an interrupt is taken.
    pub sleeping: bool,
    /// Set when the instruction being executed raised an exception (so a slot does not jump).
    trapped: bool,
}

#[inline]
fn sext8(v: u32) -> u32 {
    v as u8 as i8 as i32 as u32
}

#[inline]
fn sext16(v: u32) -> u32 {
    v as u16 as i16 as i32 as u32
}

#[inline]
fn sext12(v: u32) -> u32 {
    (((v & 0xFFF) << 20) as i32 >> 20) as u32
}

/// Is `op` a branch, so illegal in a delay slot?
fn is_branch(op: u16) -> bool {
    let op = op as u32;
    match op >> 12 {
        0x0 => matches!(op & 0xFF, 0x03 | 0x23) || matches!(op, 0x000B | 0x002B),
        0x4 => matches!(op & 0xFF, 0x0B | 0x2B),
        0x8 => matches!((op >> 8) & 0xF, 0x9 | 0xB | 0xD | 0xF),
        0xA | 0xB => true,
        0xC => (op >> 8) & 0xF == 0x3,
        _ => false,
    }
}

impl Sh2 {
    pub fn t(&self) -> bool {
        self.sr & SR_T != 0
    }

    pub fn set_t(&mut self, t: bool) {
        self.sr = (self.sr & !SR_T) | t as u32;
    }

    pub fn q(&self) -> bool {
        self.sr & SR_Q != 0
    }

    pub fn m(&self) -> bool {
        self.sr & SR_M != 0
    }

    pub fn set_q(&mut self, q: bool) {
        self.sr = (self.sr & !SR_Q) | ((q as u32) << 8);
    }

    pub fn set_m(&mut self, m: bool) {
        self.sr = (self.sr & !SR_M) | ((m as u32) << 9);
    }

    /// The interrupt mask (SR.I0-I3).
    pub fn level(&self) -> u32 {
        (self.sr & SR_I) >> 4
    }

    /// Takes an exception: SR and `saved_pc` pushed, PC from the vector at VBR + 4 × `vector`.
    fn exception<B: Bus + ?Sized>(&mut self, bus: &mut B, vector: u32, saved_pc: u32) {
        self.r[15] = self.r[15].wrapping_sub(4);
        bus.write32(self.r[15], self.sr);
        self.r[15] = self.r[15].wrapping_sub(4);
        bus.write32(self.r[15], saved_pc);
        self.pc = bus.read32(self.vbr.wrapping_add(vector << 2));
        self.delay = None;
        self.sleeping = false;
        self.trapped = true;
    }

    /// A power-on or manual reset: PC and R15 from the vector pair at `vector` × 4 (0 power on, 2
    /// manual reset), VBR 0, interrupts masked.
    pub fn reset<B: Bus + ?Sized>(&mut self, bus: &mut B, vector: u32) {
        self.pc = bus.read32(vector << 2);
        self.r[15] = bus.read32((vector + 1) << 2);
        self.vbr = 0;
        self.sr = (self.sr & !SR_I) | SR_I;
        self.delay = None;
        self.sleeping = false;
    }

    /// Takes an interrupt between instructions: the machine calls this when one is pending at a
    /// higher level than the mask and no delayed branch is waiting for its slot.
    pub fn interrupt<B: Bus + ?Sized>(&mut self, bus: &mut B, vector: u32, level: u32) {
        debug_assert!(self.delay.is_none());
        let saved = self.pc;
        self.exception(bus, vector, saved);
        self.sr = (self.sr & !SR_I) | ((level & 0xF) << 4);
    }

    /// Takes an NMI (vector 11): not maskable; the mask becomes 15.
    pub fn nmi<B: Bus + ?Sized>(&mut self, bus: &mut B) {
        debug_assert!(self.delay.is_none());
        let saved = self.pc;
        self.exception(bus, VEC_NMI, saved);
        self.sr |= SR_I;
    }

    /// Executes one instruction (or, asleep, nothing).
    pub fn step<B: Bus + ?Sized>(&mut self, bus: &mut B) {
        if self.sleeping {
            return;
        }
        let addr = self.pc;
        let op = bus.fetch16(addr);
        let slot = self.delay.take();
        if let Some(d) = slot
            && is_branch(op)
        {
            self.exception(bus, VEC_SLOT_ILLEGAL, d.branch);
            return;
        }
        self.pc = addr.wrapping_add(2);
        self.trapped = false;
        self.execute(bus, op, addr, slot);
        if let Some(d) = slot
            && !self.sleeping
            && !self.trapped
        {
            self.pc = d.target;
        }
    }

    fn delayed(&mut self, target: u32, branch: u32) {
        self.delay = Some(Delay { target, branch });
    }

    fn illegal<B: Bus + ?Sized>(&mut self, bus: &mut B, addr: u32) {
        self.exception(bus, VEC_ILLEGAL, addr);
    }

    fn execute<B: Bus + ?Sized>(&mut self, bus: &mut B, op: u16, addr: u32, slot: Option<Delay>) {
        let op = op as u32;
        let n = ((op >> 8) & 0xF) as usize;
        let m = ((op >> 4) & 0xF) as usize;
        let d4 = op & 0xF;
        let d8 = op & 0xFF;
        // The manual's PC for PC-relative operands: the instruction's address + 4, or in a delay
        // slot the branch destination + 2.
        let pc4 = match slot {
            Some(d) => d.target.wrapping_add(2),
            None => addr.wrapping_add(4),
        };
        match op >> 12 {
            0x0 => match op & 0xF {
                0x2 => match m {
                    0 => self.r[n] = self.sr,
                    1 => self.r[n] = self.gbr,
                    2 => self.r[n] = self.vbr,
                    _ => self.illegal(bus, addr),
                },
                0x3 => match m {
                    0 => {
                        self.pr = addr.wrapping_add(4);
                        self.delayed(addr.wrapping_add(4).wrapping_add(self.r[n]), addr);
                    }
                    2 => self.delayed(addr.wrapping_add(4).wrapping_add(self.r[n]), addr),
                    _ => self.illegal(bus, addr),
                },
                0x4 => bus.write8(self.r[n].wrapping_add(self.r[0]), self.r[m] as u8),
                0x5 => bus.write16(self.r[n].wrapping_add(self.r[0]), self.r[m] as u16),
                0x6 => bus.write32(self.r[n].wrapping_add(self.r[0]), self.r[m]),
                0x7 => self.macl = self.r[n].wrapping_mul(self.r[m]),
                0x8 => match op {
                    0x0008 => self.set_t(false),
                    0x0018 => self.set_t(true),
                    0x0028 => {
                        self.mach = 0;
                        self.macl = 0;
                    }
                    _ => self.illegal(bus, addr),
                },
                0x9 => match op & 0xFF {
                    0x09 if n == 0 => {}
                    0x19 if n == 0 => {
                        self.set_m(false);
                        self.set_q(false);
                        self.set_t(false);
                    }
                    0x29 => self.r[n] = self.sr & SR_T,
                    _ => self.illegal(bus, addr),
                },
                0xA => match m {
                    0 => self.r[n] = self.mach,
                    1 => self.r[n] = self.macl,
                    2 => self.r[n] = self.pr,
                    _ => self.illegal(bus, addr),
                },
                0xB => match op {
                    0x000B => self.delayed(self.pr, addr),
                    0x001B => {
                        // SLEEP: the next instruction runs after the interrupt that wakes the CPU
                        self.sleeping = true;
                    }
                    0x002B => {
                        let pc = bus.read32(self.r[15]);
                        self.r[15] = self.r[15].wrapping_add(4);
                        self.sr = bus.read32(self.r[15]) & SR_MASK;
                        self.r[15] = self.r[15].wrapping_add(4);
                        self.delayed(pc, addr);
                    }
                    _ => self.illegal(bus, addr),
                },
                0xC => self.r[n] = sext8(bus.read8(self.r[m].wrapping_add(self.r[0])) as u32),
                0xD => self.r[n] = sext16(bus.read16(self.r[m].wrapping_add(self.r[0])) as u32),
                0xE => self.r[n] = bus.read32(self.r[m].wrapping_add(self.r[0])),
                0xF => self.mac_l(bus, n, m),
                _ => self.illegal(bus, addr),
            },
            0x1 => bus.write32(self.r[n].wrapping_add(d4 << 2), self.r[m]),
            0x2 => match op & 0xF {
                0x0 => bus.write8(self.r[n], self.r[m] as u8),
                0x1 => bus.write16(self.r[n], self.r[m] as u16),
                0x2 => bus.write32(self.r[n], self.r[m]),
                0x4 => {
                    let a = self.r[n].wrapping_sub(1);
                    bus.write8(a, self.r[m] as u8);
                    self.r[n] = a;
                }
                0x5 => {
                    let a = self.r[n].wrapping_sub(2);
                    bus.write16(a, self.r[m] as u16);
                    self.r[n] = a;
                }
                0x6 => {
                    let a = self.r[n].wrapping_sub(4);
                    bus.write32(a, self.r[m]);
                    self.r[n] = a;
                }
                0x7 => {
                    let q = self.r[n] >> 31 != 0;
                    let mm = self.r[m] >> 31 != 0;
                    self.set_q(q);
                    self.set_m(mm);
                    self.set_t(q != mm);
                }
                0x8 => self.set_t(self.r[n] & self.r[m] == 0),
                0x9 => self.r[n] &= self.r[m],
                0xA => self.r[n] ^= self.r[m],
                0xB => self.r[n] |= self.r[m],
                0xC => {
                    let x = self.r[n] ^ self.r[m];
                    let any_zero = x & 0xFF00_0000 == 0
                        || x & 0x00FF_0000 == 0
                        || x & 0x0000_FF00 == 0
                        || x & 0x0000_00FF == 0;
                    self.set_t(any_zero);
                }
                0xD => self.r[n] = (self.r[m] << 16) | (self.r[n] >> 16),
                0xE => self.macl = (self.r[n] & 0xFFFF).wrapping_mul(self.r[m] & 0xFFFF),
                0xF => {
                    let p = (self.r[n] as i16 as i32).wrapping_mul(self.r[m] as i16 as i32);
                    self.macl = p as u32;
                }
                _ => self.illegal(bus, addr),
            },
            0x3 => {
                let (a, b) = (self.r[n], self.r[m]);
                match op & 0xF {
                    0x0 => self.set_t(a == b),
                    0x2 => self.set_t(a >= b),
                    0x3 => self.set_t(a as i32 >= b as i32),
                    0x4 => self.div1(n, m),
                    0x5 => {
                        let p = (a as u64) * (b as u64);
                        self.mach = (p >> 32) as u32;
                        self.macl = p as u32;
                    }
                    0x6 => self.set_t(a > b),
                    0x7 => self.set_t(a as i32 > b as i32),
                    0x8 => self.r[n] = a.wrapping_sub(b),
                    0xA => {
                        let t1 = a.wrapping_sub(b);
                        let res = t1.wrapping_sub(self.t() as u32);
                        self.r[n] = res;
                        self.set_t(a < t1 || t1 < res);
                    }
                    0xB => {
                        let res = a.wrapping_sub(b);
                        self.r[n] = res;
                        self.set_t(((a ^ b) & (a ^ res)) >> 31 != 0);
                    }
                    0xC => self.r[n] = a.wrapping_add(b),
                    0xD => {
                        let p = (a as i32 as i64) * (b as i32 as i64);
                        self.mach = (p >> 32) as u32;
                        self.macl = p as u32;
                    }
                    0xE => {
                        let t1 = a.wrapping_add(b);
                        let res = t1.wrapping_add(self.t() as u32);
                        self.r[n] = res;
                        self.set_t(a > t1 || t1 > res);
                    }
                    0xF => {
                        let res = a.wrapping_add(b);
                        self.r[n] = res;
                        self.set_t((!(a ^ b) & (a ^ res)) >> 31 != 0);
                    }
                    _ => self.illegal(bus, addr),
                }
            }
            0x4 => self.op4(bus, op, n, m, addr),
            0x5 => self.r[n] = bus.read32(self.r[m].wrapping_add(d4 << 2)),
            0x6 => {
                let v = self.r[m];
                match op & 0xF {
                    0x0 => self.r[n] = sext8(bus.read8(v) as u32),
                    0x1 => self.r[n] = sext16(bus.read16(v) as u32),
                    0x2 => self.r[n] = bus.read32(v),
                    0x3 => self.r[n] = v,
                    0x4 => {
                        self.r[n] = sext8(bus.read8(v) as u32);
                        if n != m {
                            self.r[m] = v.wrapping_add(1);
                        }
                    }
                    0x5 => {
                        self.r[n] = sext16(bus.read16(v) as u32);
                        if n != m {
                            self.r[m] = v.wrapping_add(2);
                        }
                    }
                    0x6 => {
                        self.r[n] = bus.read32(v);
                        if n != m {
                            self.r[m] = v.wrapping_add(4);
                        }
                    }
                    0x7 => self.r[n] = !v,
                    0x8 => self.r[n] = (v & 0xFFFF_0000) | ((v & 0xFF) << 8) | ((v >> 8) & 0xFF),
                    0x9 => self.r[n] = v.rotate_left(16),
                    0xA => {
                        let t0 = 0u32.wrapping_sub(v);
                        let res = t0.wrapping_sub(self.t() as u32);
                        self.r[n] = res;
                        self.set_t(0 < t0 || t0 < res);
                    }
                    0xB => self.r[n] = 0u32.wrapping_sub(v),
                    0xC => self.r[n] = v & 0xFF,
                    0xD => self.r[n] = v & 0xFFFF,
                    0xE => self.r[n] = sext8(v),
                    _ => self.r[n] = sext16(v),
                }
            }
            0x7 => self.r[n] = self.r[n].wrapping_add(sext8(d8)),
            0x8 => match n {
                0x0 => bus.write8(self.r[m].wrapping_add(d4), self.r[0] as u8),
                0x1 => bus.write16(self.r[m].wrapping_add(d4 << 1), self.r[0] as u16),
                0x4 => self.r[0] = sext8(bus.read8(self.r[m].wrapping_add(d4)) as u32),
                0x5 => self.r[0] = sext16(bus.read16(self.r[m].wrapping_add(d4 << 1)) as u32),
                0x8 => self.set_t(self.r[0] == sext8(d8)),
                0x9 | 0xB | 0xD | 0xF => {
                    let target = addr.wrapping_add(4).wrapping_add(sext8(d8) << 1);
                    let taken = self.t() == (n == 0x9 || n == 0xD);
                    if taken {
                        if n >= 0xD {
                            self.delayed(target, addr);
                        } else {
                            self.pc = target;
                        }
                    }
                }
                _ => self.illegal(bus, addr),
            },
            0x9 => self.r[n] = sext16(bus.read16(pc4.wrapping_add(d8 << 1)) as u32),
            0xA => self.delayed(addr.wrapping_add(4).wrapping_add(sext12(op) << 1), addr),
            0xB => {
                self.pr = addr.wrapping_add(4);
                self.delayed(addr.wrapping_add(4).wrapping_add(sext12(op) << 1), addr);
            }
            0xC => self.op_c(bus, op, n, d8, pc4, addr),
            0xD => self.r[n] = bus.read32((pc4 & !3).wrapping_add(d8 << 2)),
            0xE => self.r[n] = sext8(d8),
            _ => self.illegal(bus, addr),
        }
    }

    fn op4<B: Bus + ?Sized>(&mut self, bus: &mut B, op: u32, n: usize, m: usize, addr: u32) {
        if op & 0xF == 0xF {
            self.mac_w(bus, n, m);
            return;
        }
        let v = self.r[n];
        match op & 0xFF {
            0x00 | 0x20 => {
                self.set_t(v >> 31 != 0);
                self.r[n] = v << 1;
            }
            0x10 => {
                let r = v.wrapping_sub(1);
                self.r[n] = r;
                self.set_t(r == 0);
            }
            0x01 => {
                self.set_t(v & 1 != 0);
                self.r[n] = v >> 1;
            }
            0x11 => self.set_t(v as i32 >= 0),
            0x21 => {
                self.set_t(v & 1 != 0);
                self.r[n] = ((v as i32) >> 1) as u32;
            }
            0x02 | 0x12 | 0x22 | 0x03 | 0x13 | 0x23 => {
                let value = match op & 0xFF {
                    0x02 => self.mach,
                    0x12 => self.macl,
                    0x22 => self.pr,
                    0x03 => self.sr,
                    0x13 => self.gbr,
                    _ => self.vbr,
                };
                let a = v.wrapping_sub(4);
                bus.write32(a, value);
                self.r[n] = a;
            }
            0x04 => {
                self.set_t(v >> 31 != 0);
                self.r[n] = v.rotate_left(1);
            }
            0x24 => {
                let t = self.t() as u32;
                self.set_t(v >> 31 != 0);
                self.r[n] = (v << 1) | t;
            }
            0x05 => {
                self.set_t(v & 1 != 0);
                self.r[n] = v.rotate_right(1);
            }
            0x15 => self.set_t(v as i32 > 0),
            0x25 => {
                let t = self.t() as u32;
                self.set_t(v & 1 != 0);
                self.r[n] = (v >> 1) | (t << 31);
            }
            0x06 | 0x16 | 0x26 | 0x07 | 0x17 | 0x27 => {
                let value = bus.read32(v);
                self.r[n] = v.wrapping_add(4);
                match op & 0xFF {
                    0x06 => self.mach = value,
                    0x16 => self.macl = value,
                    0x26 => self.pr = value,
                    0x07 => self.sr = value & SR_MASK,
                    0x17 => self.gbr = value,
                    _ => self.vbr = value,
                }
            }
            0x08 => self.r[n] = v << 2,
            0x18 => self.r[n] = v << 8,
            0x28 => self.r[n] = v << 16,
            0x09 => self.r[n] = v >> 2,
            0x19 => self.r[n] = v >> 8,
            0x29 => self.r[n] = v >> 16,
            0x0A => self.mach = v,
            0x1A => self.macl = v,
            0x2A => self.pr = v,
            0x0B => {
                self.pr = addr.wrapping_add(4);
                self.delayed(v, addr);
            }
            0x1B => {
                let b = bus.read8(v);
                self.set_t(b == 0);
                bus.write8(v, b | 0x80);
            }
            0x2B => self.delayed(v, addr),
            0x0E => self.sr = v & SR_MASK,
            0x1E => self.gbr = v,
            0x2E => self.vbr = v,
            _ => {
                let _ = m;
                self.illegal(bus, addr)
            }
        }
    }

    fn op_c<B: Bus + ?Sized>(&mut self, bus: &mut B, op: u32, n: usize, d8: u32, pc4: u32, addr: u32) {
        let gbr = self.gbr;
        match n {
            0x0 => bus.write8(gbr.wrapping_add(d8), self.r[0] as u8),
            0x1 => bus.write16(gbr.wrapping_add(d8 << 1), self.r[0] as u16),
            0x2 => bus.write32(gbr.wrapping_add(d8 << 2), self.r[0]),
            0x3 => {
                let next = addr.wrapping_add(2);
                self.exception(bus, d8, next);
            }
            0x4 => self.r[0] = sext8(bus.read8(gbr.wrapping_add(d8)) as u32),
            0x5 => self.r[0] = sext16(bus.read16(gbr.wrapping_add(d8 << 1)) as u32),
            0x6 => self.r[0] = bus.read32(gbr.wrapping_add(d8 << 2)),
            0x7 => self.r[0] = (pc4 & !3).wrapping_add(d8 << 2),
            0x8 => self.set_t(self.r[0] & d8 == 0),
            0x9 => self.r[0] &= d8,
            0xA => self.r[0] ^= d8,
            0xB => self.r[0] |= d8,
            _ => {
                let a = gbr.wrapping_add(self.r[0]);
                let b = bus.read8(a) as u32;
                match n {
                    0xC => self.set_t(b & d8 == 0),
                    0xD => bus.write8(a, (b & d8) as u8),
                    0xE => bus.write8(a, (b ^ d8) as u8),
                    _ => bus.write8(a, (b | d8) as u8),
                }
            }
        }
        let _ = op;
    }

    /// DIV1: one step of the non-restoring division, as the manual defines it.
    pub fn div1(&mut self, n: usize, m: usize) {
        let old_q = self.q();
        let mut q = self.r[n] >> 31 != 0;
        let divisor = self.r[m];
        let rn = (self.r[n] << 1) | self.t() as u32;
        let res;
        if old_q == self.m() {
            res = rn.wrapping_sub(divisor);
            let borrow = res > rn;
            q = if self.m() { !(q ^ borrow) } else { q ^ borrow };
            // M=0: Q = Q ^ borrow; M=1 (with old_q=1): Q = !(Q ^ borrow)
        } else {
            res = rn.wrapping_add(divisor);
            let carry = res < rn;
            q = if self.m() { !(q ^ carry) } else { q ^ carry };
            // M=1 (old_q=0): Q = !(Q ^ carry); M=0 (old_q=1): Q = Q ^ carry
        }
        self.r[n] = res;
        self.set_q(q);
        self.set_t(q == self.m());
    }

    /// MAC.L @Rm+,@Rn+. The oracle reads @Rm first (the manual's pseudo-code reads @Rn first).
    pub fn mac_l<B: Bus + ?Sized>(&mut self, bus: &mut B, n: usize, m: usize) {
        let b = bus.read32(self.r[m]) as i32 as i64;
        self.r[m] = self.r[m].wrapping_add(4);
        let a = bus.read32(self.r[n]) as i32 as i64;
        self.r[n] = self.r[n].wrapping_add(4);
        let mac = (((self.mach as u64) << 32) | self.macl as u64) as i64;
        let mut res = mac.wrapping_add(a.wrapping_mul(b));
        if self.sr & SR_S != 0 {
            res = res.clamp(-0x8000_0000_0000, 0x7FFF_FFFF_FFFF);
        }
        self.mach = (res >> 32) as u32;
        self.macl = res as u32;
    }

    /// MAC.W @Rm+,@Rn+, reading @Rm first as MAC.L does (not yet seen in a trace).
    pub fn mac_w<B: Bus + ?Sized>(&mut self, bus: &mut B, n: usize, m: usize) {
        let b = bus.read16(self.r[m]) as i16 as i64;
        self.r[m] = self.r[m].wrapping_add(2);
        let a = bus.read16(self.r[n]) as i16 as i64;
        self.r[n] = self.r[n].wrapping_add(2);
        let p = a * b;
        if self.sr & SR_S != 0 {
            let res = self.macl as i32 as i64 + p;
            if res > i32::MAX as i64 || res < i32::MIN as i64 {
                self.mach |= 1;
            }
            self.macl = res.clamp(i32::MIN as i64, i32::MAX as i64) as u32;
        } else {
            let mac = (((self.mach as u64) << 32) | self.macl as u64) as i64;
            let res = mac.wrapping_add(p);
            self.mach = (res >> 32) as u32;
            self.macl = res as u32;
        }
    }
}
