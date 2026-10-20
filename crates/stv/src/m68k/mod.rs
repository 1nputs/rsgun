//! The Motorola 68000: the ST-V's sound CPU, which runs Sega's standard sound driver against the SCSP.
//!
//! Written from the 68000 programmer's reference manual. One call of [`M68k::step`] runs one
//! instruction (or takes a pending interrupt); cycle counts are approximate (the sound driver keeps
//! time by the SCSP's timers, not by instruction timing).

use crate::state::saved;

#[cfg(test)]
mod tests;

/// The sound CPU's bus: 8- and 16-bit accesses (a long is two words, high first, as the 68000
/// makes them), instruction fetches, and the interrupt acknowledge cycle.
pub trait Bus68 {
    fn read8(&mut self, addr: u32) -> u8;
    fn read16(&mut self, addr: u32) -> u16;
    fn write8(&mut self, addr: u32, value: u8);
    fn write16(&mut self, addr: u32, value: u16);
    fn fetch16(&mut self, addr: u32) -> u16;
    /// The vector for an interrupt at `level`, or None for the autovector.
    fn int_ack(&mut self, level: u8) -> Option<u8> {
        let _ = level;
        None
    }
}

pub const SR_C: u16 = 0x0001;
pub const SR_V: u16 = 0x0002;
pub const SR_Z: u16 = 0x0004;
pub const SR_N: u16 = 0x0008;
pub const SR_X: u16 = 0x0010;
pub const SR_S: u16 = 0x2000;
pub const SR_T: u16 = 0x8000;
const SR_MASK: u16 = 0xA71F;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Size {
    B,
    W,
    L,
}

impl Size {
    fn bytes(self) -> u32 {
        match self {
            Size::B => 1,
            Size::W => 2,
            Size::L => 4,
        }
    }
    fn mask(self) -> u32 {
        match self {
            Size::B => 0xFF,
            Size::W => 0xFFFF,
            Size::L => 0xFFFF_FFFF,
        }
    }
    fn msb(self) -> u32 {
        match self {
            Size::B => 0x80,
            Size::W => 0x8000,
            Size::L => 0x8000_0000,
        }
    }
    /// The usual size field (bits 7-6): 0 byte, 1 word, 2 long.
    fn from_bits(b: u16) -> Option<Size> {
        match b & 3 {
            0 => Some(Size::B),
            1 => Some(Size::W),
            2 => Some(Size::L),
            _ => None,
        }
    }
}

/// A resolved effective address.
#[derive(Clone, Copy, Debug)]
enum Ea {
    D(usize),
    A(usize),
    Mem(u32),
    Imm(u32),
}

/// Exception vectors.
const VEC_ILLEGAL: u32 = 4;
const VEC_ZERO_DIVIDE: u32 = 5;
const VEC_CHK: u32 = 6;
const VEC_TRAPV: u32 = 7;
const VEC_PRIVILEGE: u32 = 8;
const VEC_LINE_A: u32 = 10;
const VEC_LINE_F: u32 = 11;
const VEC_SPURIOUS: u32 = 24;
const VEC_AUTOVECTOR: u32 = 24;
const VEC_TRAP: u32 = 32;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct M68k {
    pub d: [u32; 8],
    /// A0-A7; A7 is the active stack pointer, the other one is kept in `other_sp`.
    pub a: [u32; 8],
    pub other_sp: u32,
    pub pc: u32,
    pub sr: u16,
    /// STOP: waiting for an interrupt.
    pub stopped: bool,
    /// The interrupt level the bus presents (0-7).
    pub ipl: u8,
    /// Cycles the last step took.
    pub cycles: u32,
    /// The address of the instruction being executed.
    pub op_pc: u32,
}

saved!(M68k { d, a, other_sp, pc, sr, stopped, ipl, cycles, op_pc });

#[inline]
fn sext8(v: u32) -> u32 {
    v as u8 as i8 as i32 as u32
}

#[inline]
fn sext16(v: u32) -> u32 {
    v as u16 as i16 as i32 as u32
}

impl M68k {
    /// Reset: supervisor mode, interrupts masked, SSP and PC from vectors 0 and 1.
    pub fn reset<B: Bus68>(&mut self, bus: &mut B) {
        self.sr = 0x2700;
        self.stopped = false;
        self.a[7] = self.read32(bus, 0);
        self.pc = self.read32(bus, 4);
    }

    fn supervisor(&self) -> bool {
        self.sr & SR_S != 0
    }

    /// Sets SR, swapping the stack pointers on a change of mode.
    pub fn set_sr(&mut self, v: u16) {
        let v = v & SR_MASK;
        if (v ^ self.sr) & SR_S != 0 {
            std::mem::swap(&mut self.a[7], &mut self.other_sp);
        }
        self.sr = v;
    }

    pub fn usp(&self) -> u32 {
        if self.supervisor() { self.other_sp } else { self.a[7] }
    }

    fn set_usp(&mut self, v: u32) {
        if self.supervisor() { self.other_sp = v } else { self.a[7] = v }
    }

    fn flag(&self, f: u16) -> bool {
        self.sr & f != 0
    }

    fn set_flag(&mut self, f: u16, on: bool) {
        if on { self.sr |= f } else { self.sr &= !f }
    }

    fn set_nz(&mut self, v: u32, s: Size) {
        let v = v & s.mask();
        self.set_flag(SR_N, v & s.msb() != 0);
        self.set_flag(SR_Z, v == 0);
    }

    /// Logic results: N and Z from the value, V and C cleared.
    fn set_logic(&mut self, v: u32, s: Size) {
        self.set_nz(v, s);
        self.sr &= !(SR_V | SR_C);
    }

    // ---- bus helpers ----------------------------------------------------------------------------

    fn read<B: Bus68>(&mut self, bus: &mut B, a: u32, s: Size) -> u32 {
        let a = a & 0xFF_FFFF;
        match s {
            Size::B => bus.read8(a) as u32,
            Size::W => bus.read16(a) as u32,
            Size::L => self.read32(bus, a),
        }
    }

    fn read32<B: Bus68>(&mut self, bus: &mut B, a: u32) -> u32 {
        let a = a & 0xFF_FFFF;
        ((bus.read16(a) as u32) << 16) | bus.read16(a.wrapping_add(2) & 0xFF_FFFF) as u32
    }

    fn write<B: Bus68>(&mut self, bus: &mut B, a: u32, s: Size, v: u32) {
        let a = a & 0xFF_FFFF;
        match s {
            Size::B => bus.write8(a, v as u8),
            Size::W => bus.write16(a, v as u16),
            Size::L => {
                bus.write16(a, (v >> 16) as u16);
                bus.write16(a.wrapping_add(2) & 0xFF_FFFF, v as u16);
            }
        }
    }

    fn fetch<B: Bus68>(&mut self, bus: &mut B) -> u32 {
        let w = bus.fetch16(self.pc & 0xFF_FFFF);
        self.pc = self.pc.wrapping_add(2);
        w as u32
    }

    fn fetch32<B: Bus68>(&mut self, bus: &mut B) -> u32 {
        let h = self.fetch(bus);
        (h << 16) | self.fetch(bus)
    }

    fn push32<B: Bus68>(&mut self, bus: &mut B, v: u32) {
        self.a[7] = self.a[7].wrapping_sub(4);
        let sp = self.a[7];
        self.write(bus, sp, Size::L, v);
    }

    fn push16<B: Bus68>(&mut self, bus: &mut B, v: u16) {
        self.a[7] = self.a[7].wrapping_sub(2);
        let sp = self.a[7];
        bus.write16(sp & 0xFF_FFFF, v);
    }

    fn pop32<B: Bus68>(&mut self, bus: &mut B) -> u32 {
        let v = self.read32(bus, self.a[7]);
        self.a[7] = self.a[7].wrapping_add(4);
        v
    }

    fn pop16<B: Bus68>(&mut self, bus: &mut B) -> u16 {
        let v = bus.read16(self.a[7] & 0xFF_FFFF);
        self.a[7] = self.a[7].wrapping_add(2);
        v
    }

    // ---- effective addresses --------------------------------------------------------------------

    /// The brief extension word's index: Dn or An, word or long, plus the 8-bit displacement.
    fn index<B: Bus68>(&mut self, bus: &mut B, base: u32) -> u32 {
        let ext = self.fetch(bus);
        let r = ((ext >> 12) & 7) as usize;
        let mut x = if ext & 0x8000 != 0 { self.a[r] } else { self.d[r] };
        if ext & 0x800 == 0 {
            x = sext16(x);
        }
        base.wrapping_add(x).wrapping_add(sext8(ext))
    }

    /// Resolves an effective address (applying (An)+ and -(An)); `None` for an invalid mode.
    fn ea<B: Bus68>(&mut self, bus: &mut B, mode: u16, reg: u16, s: Size) -> Option<Ea> {
        let r = reg as usize;
        let step = |r: usize, s: Size| if r == 7 && s == Size::B { 2 } else { s.bytes() };
        Some(match mode {
            0 => Ea::D(r),
            1 => Ea::A(r),
            2 => Ea::Mem(self.a[r]),
            3 => {
                let a = self.a[r];
                self.a[r] = a.wrapping_add(step(r, s));
                Ea::Mem(a)
            }
            4 => {
                self.a[r] = self.a[r].wrapping_sub(step(r, s));
                self.cycles += 2;
                Ea::Mem(self.a[r])
            }
            5 => {
                let d = sext16(self.fetch(bus));
                Ea::Mem(self.a[r].wrapping_add(d))
            }
            6 => {
                let base = self.a[r];
                self.cycles += 2;
                Ea::Mem(self.index(bus, base))
            }
            7 => match reg {
                0 => Ea::Mem(sext16(self.fetch(bus))),
                1 => Ea::Mem(self.fetch32(bus)),
                2 => {
                    let base = self.pc;
                    Ea::Mem(base.wrapping_add(sext16(self.fetch(bus))))
                }
                3 => {
                    let base = self.pc;
                    self.cycles += 2;
                    Ea::Mem(self.index(bus, base))
                }
                4 => Ea::Imm(match s {
                    Size::B => self.fetch(bus) & 0xFF,
                    Size::W => self.fetch(bus),
                    Size::L => self.fetch32(bus),
                }),
                _ => return None,
            },
            _ => return None,
        })
    }

    fn read_ea<B: Bus68>(&mut self, bus: &mut B, ea: Ea, s: Size) -> u32 {
        match ea {
            Ea::D(r) => self.d[r] & s.mask(),
            Ea::A(r) => self.a[r] & s.mask(),
            Ea::Mem(a) => self.read(bus, a, s),
            Ea::Imm(v) => v & s.mask(),
        }
    }

    fn write_ea<B: Bus68>(&mut self, bus: &mut B, ea: Ea, s: Size, v: u32) {
        match ea {
            Ea::D(r) => self.d[r] = (self.d[r] & !s.mask()) | (v & s.mask()),
            Ea::A(r) => self.a[r] = v,
            Ea::Mem(a) => self.write(bus, a, s, v),
            Ea::Imm(_) => {}
        }
    }

    /// The address of a control-mode effective address (for LEA, PEA, JMP, JSR, MOVEM).
    fn control_addr<B: Bus68>(&mut self, bus: &mut B, mode: u16, reg: u16) -> Option<u32> {
        match (mode, reg) {
            (2, _) | (5, _) | (6, _) | (7, 0..=3) => match self.ea(bus, mode, reg, Size::L)? {
                Ea::Mem(a) => Some(a),
                _ => None,
            },
            _ => None,
        }
    }

    // ---- exceptions -----------------------------------------------------------------------------

    fn exception<B: Bus68>(&mut self, bus: &mut B, vector: u32, saved_pc: u32) {
        let old_sr = self.sr;
        self.set_sr((self.sr | SR_S) & !SR_T);
        self.push32(bus, saved_pc);
        self.push16(bus, old_sr);
        self.pc = self.read32(bus, vector * 4);
        self.cycles += 34;
    }

    /// Takes the interrupt the bus presents if its level beats the mask (7 always does).
    fn take_interrupt<B: Bus68>(&mut self, bus: &mut B) -> bool {
        let level = self.ipl & 7;
        let mask = ((self.sr >> 8) & 7) as u8;
        if level == 0 || (level <= mask && level != 7) {
            return false;
        }
        self.stopped = false;
        let old_sr = self.sr;
        self.set_sr((self.sr | SR_S) & !SR_T);
        self.sr = (self.sr & !0x0700) | ((level as u16) << 8);
        let saved = self.pc;
        self.push32(bus, saved);
        let vector = match bus.int_ack(level) {
            Some(v) => v as u32,
            None => VEC_AUTOVECTOR + level as u32,
        };
        self.push16(bus, old_sr);
        self.pc = self.read32(bus, vector * 4);
        self.cycles += 44;
        let _ = VEC_SPURIOUS;
        true
    }

    // ---- condition codes ------------------------------------------------------------------------

    fn cond(&self, c: u16) -> bool {
        let (n, z, v, cf) = (self.flag(SR_N), self.flag(SR_Z), self.flag(SR_V), self.flag(SR_C));
        match c & 0xF {
            0 => true,
            1 => false,
            2 => !cf && !z,
            3 => cf || z,
            4 => !cf,
            5 => cf,
            6 => !z,
            7 => z,
            8 => !v,
            9 => v,
            10 => !n,
            11 => n,
            12 => n == v,
            13 => n != v,
            14 => !z && n == v,
            _ => z || n != v,
        }
    }

    // ---- arithmetic -------------------------------------------------------------------------------

    /// dst + src (+ X for ADDX); sets X N Z V C (Z only cleared, not set, for ADDX).
    fn add(&mut self, dst: u32, src: u32, s: Size, extend: bool) -> u32 {
        let x = if extend { self.flag(SR_X) as u64 } else { 0 };
        let m = s.mask() as u64;
        let r = (dst as u64 & m) + (src as u64 & m) + x;
        let res = r as u32 & s.mask();
        let carry = r > m;
        let ov = ((dst ^ res) & (src ^ res) & s.msb()) != 0;
        self.set_flag(SR_C, carry);
        self.set_flag(SR_X, carry);
        self.set_flag(SR_V, ov);
        self.set_flag(SR_N, res & s.msb() != 0);
        if extend {
            if res != 0 {
                self.sr &= !SR_Z;
            }
        } else {
            self.set_flag(SR_Z, res == 0);
        }
        res
    }

    /// dst - src (- X for SUBX); `cmp` leaves X alone.
    fn sub(&mut self, dst: u32, src: u32, s: Size, extend: bool, cmp: bool) -> u32 {
        let x = if extend { self.flag(SR_X) as u64 } else { 0 };
        let m = s.mask() as u64;
        let (d, sv) = (dst as u64 & m, src as u64 & m);
        let res = (d.wrapping_sub(sv).wrapping_sub(x) & m) as u32;
        let borrow = sv + x > d;
        let ov = ((dst ^ src) & (dst ^ res) & s.msb()) != 0;
        self.set_flag(SR_C, borrow);
        if !cmp {
            self.set_flag(SR_X, borrow);
        }
        self.set_flag(SR_V, ov);
        self.set_flag(SR_N, res & s.msb() != 0);
        if extend {
            if res != 0 {
                self.sr &= !SR_Z;
            }
        } else {
            self.set_flag(SR_Z, res == 0);
        }
        res
    }

    fn abcd(&mut self, dst: u32, src: u32) -> u32 {
        let x = self.flag(SR_X) as u32;
        let mut lo = (dst & 0xF) + (src & 0xF) + x;
        let mut res = (dst & 0xF0) + (src & 0xF0);
        if lo > 9 {
            lo += 6;
        }
        res += lo;
        let carry = res > 0x99;
        if carry {
            res += 0x60;
        }
        let res = res & 0xFF;
        self.set_flag(SR_C, carry);
        self.set_flag(SR_X, carry);
        if res != 0 {
            self.sr &= !SR_Z;
        }
        self.set_flag(SR_N, res & 0x80 != 0);
        res
    }

    fn sbcd(&mut self, dst: u32, src: u32) -> u32 {
        let x = self.flag(SR_X) as i32;
        let mut lo = (dst & 0xF) as i32 - (src & 0xF) as i32 - x;
        let mut hi = (dst & 0xF0) as i32 - (src & 0xF0) as i32;
        if lo < 0 {
            lo += 10;
            hi -= 0x10;
        }
        let mut res = hi + lo;
        let borrow = res < 0;
        if borrow {
            res -= 0x60;
        }
        let res = (res as u32) & 0xFF;
        self.set_flag(SR_C, borrow);
        self.set_flag(SR_X, borrow);
        if res != 0 {
            self.sr &= !SR_Z;
        }
        self.set_flag(SR_N, res & 0x80 != 0);
        res
    }

    // ---- the step ---------------------------------------------------------------------------------

    /// Runs one instruction, or takes a pending interrupt; returns the cycles taken.
    pub fn step<B: Bus68>(&mut self, bus: &mut B) -> u32 {
        self.cycles = 0;
        if self.take_interrupt(bus) {
            return self.cycles;
        }
        if self.stopped {
            return 4;
        }
        self.op_pc = self.pc;
        let op = self.fetch(bus) as u16;
        self.cycles = 4;
        if !self.execute(bus, op) {
            let pc = self.op_pc;
            let vec = match op >> 12 {
                0xA => VEC_LINE_A,
                0xF => VEC_LINE_F,
                _ => VEC_ILLEGAL,
            };
            self.exception(bus, vec, pc);
        }
        self.cycles
    }

    /// Executes `op`; false for an illegal instruction.
    fn execute<B: Bus68>(&mut self, bus: &mut B, op: u16) -> bool {
        let mode = (op >> 3) & 7;
        let reg = op & 7;
        let rx = ((op >> 9) & 7) as usize;
        match op >> 12 {
            0x0 => self.group0(bus, op, mode, reg, rx),
            0x1..=0x3 => self.op_move(bus, op, mode, reg),
            0x4 => self.group4(bus, op, mode, reg, rx),
            0x5 => self.group5(bus, op, mode, reg),
            0x6 => {
                let cond = (op >> 8) & 0xF;
                let base = self.pc;
                let mut disp = sext8(op as u32);
                if op & 0xFF == 0 {
                    disp = sext16(self.fetch(bus));
                }
                let target = base.wrapping_add(disp);
                match cond {
                    0 => self.pc = target,
                    1 => {
                        let ret = self.pc;
                        self.push32(bus, ret);
                        self.pc = target;
                    }
                    c => {
                        if self.cond(c) {
                            self.pc = target;
                        }
                    }
                }
                self.cycles += 6;
                true
            }
            0x7 => {
                if op & 0x100 != 0 {
                    return false;
                }
                let v = sext8(op as u32);
                self.d[rx] = v;
                self.set_logic(v, Size::L);
                true
            }
            0x8 => self.group8c(bus, op, mode, reg, rx, false),
            0xC => self.group8c(bus, op, mode, reg, rx, true),
            0x9 => self.group9d(bus, op, mode, reg, rx, false),
            0xD => self.group9d(bus, op, mode, reg, rx, true),
            0xB => self.group_b(bus, op, mode, reg, rx),
            0xE => self.group_e(bus, op, mode, reg, rx),
            _ => false,
        }
    }

    fn group0<B: Bus68>(&mut self, bus: &mut B, op: u16, mode: u16, reg: u16, rx: usize) -> bool {
        if op & 0x100 != 0 || (op >> 9) & 7 == 4 {
            // bit operations (dynamic: bit 8; static: 0x08xx), or MOVEP
            if op & 0x100 != 0 && mode == 1 {
                return self.movep(bus, op, reg, rx);
            }
            let bit = if op & 0x100 != 0 {
                self.d[rx]
            } else {
                if op & 0x0F00 != 0x0800 {
                    return false;
                }
                self.fetch(bus) & 0xFF
            };
            let kind = (op >> 6) & 3;
            if mode == 0 {
                let r = reg as usize;
                let mask = 1u32 << (bit & 31);
                self.set_flag(SR_Z, self.d[r] & mask == 0);
                match kind {
                    1 => self.d[r] ^= mask,
                    2 => self.d[r] &= !mask,
                    3 => self.d[r] |= mask,
                    _ => {}
                }
                self.cycles += 2;
                return true;
            }
            if mode == 1 {
                return false;
            }
            let Some(ea) = self.ea(bus, mode, reg, Size::B) else { return false };
            if kind != 0 && matches!(ea, Ea::Imm(_)) {
                return false;
            }
            let mask = 1u32 << (bit & 7);
            let v = self.read_ea(bus, ea, Size::B);
            self.set_flag(SR_Z, v & mask == 0);
            let nv = match kind {
                1 => v ^ mask,
                2 => v & !mask,
                3 => v | mask,
                _ => return true,
            };
            self.write_ea(bus, ea, Size::B, nv);
            return true;
        }
        // immediate operations: ORI ANDI SUBI ADDI EORI CMPI, and to CCR/SR
        let kind = (op >> 9) & 7;
        if matches!(kind, 0 | 1 | 5) && mode == 7 && reg == 4 {
            let size = (op >> 6) & 3;
            let imm = self.fetch(bus);
            let v = match size {
                0 => {
                    let ccr = self.sr & 0x1F;
                    let r = match kind {
                        0 => ccr | imm as u16,
                        1 => ccr & imm as u16,
                        _ => ccr ^ imm as u16,
                    };
                    self.sr = (self.sr & 0xFF00) | (r & 0x1F);
                    return true;
                }
                1 => {
                    if !self.supervisor() {
                        self.pc = self.op_pc;
                        self.exception(bus, VEC_PRIVILEGE, self.op_pc);
                        return true;
                    }
                    match kind {
                        0 => self.sr | imm as u16,
                        1 => self.sr & imm as u16,
                        _ => self.sr ^ imm as u16,
                    }
                }
                _ => return false,
            };
            self.set_sr(v);
            return true;
        }
        let Some(s) = Size::from_bits(op >> 6) else { return false };
        let imm = match s {
            Size::L => self.fetch32(bus),
            Size::W => self.fetch(bus),
            Size::B => self.fetch(bus) & 0xFF,
        };
        if mode == 1 {
            return false;
        }
        let Some(ea) = self.ea(bus, mode, reg, s) else { return false };
        if matches!(ea, Ea::Imm(_)) {
            return false;
        }
        let d = self.read_ea(bus, ea, s);
        let r = match kind {
            0 => d | imm,
            1 => d & imm,
            2 => self.sub(d, imm, s, false, false),
            3 => self.add(d, imm, s, false),
            5 => d ^ imm,
            6 => {
                self.sub(d, imm, s, false, true);
                return true;
            }
            _ => return false,
        };
        if matches!(kind, 0 | 1 | 5) {
            self.set_logic(r, s);
        }
        self.write_ea(bus, ea, s, r);
        true
    }

    fn movep<B: Bus68>(&mut self, bus: &mut B, op: u16, reg: u16, rx: usize) -> bool {
        let disp = sext16(self.fetch(bus));
        let addr = self.a[reg as usize].wrapping_add(disp);
        let long = op & 0x40 != 0;
        let n = if long { 4 } else { 2 };
        if op & 0x80 != 0 {
            let v = self.d[rx];
            for i in 0..n {
                let b = (v >> (8 * (n - 1 - i))) as u8;
                bus.write8(addr.wrapping_add(2 * i) & 0xFF_FFFF, b);
            }
        } else {
            let mut v = 0u32;
            for i in 0..n {
                v = (v << 8) | bus.read8(addr.wrapping_add(2 * i) & 0xFF_FFFF) as u32;
            }
            if long { self.d[rx] = v } else { self.d[rx] = (self.d[rx] & 0xFFFF_0000) | v }
        }
        true
    }

    fn op_move<B: Bus68>(&mut self, bus: &mut B, op: u16, mode: u16, reg: u16) -> bool {
        let s = match op >> 12 {
            1 => Size::B,
            3 => Size::W,
            _ => Size::L,
        };
        let dmode = (op >> 6) & 7;
        let dreg = (op >> 9) & 7;
        if s == Size::B && mode == 1 {
            return false;
        }
        let Some(src) = self.ea(bus, mode, reg, s) else { return false };
        let v = self.read_ea(bus, src, s);
        if dmode == 1 {
            // MOVEA: sign-extended, no flags
            if s == Size::B {
                return false;
            }
            self.a[dreg as usize] = if s == Size::W { sext16(v) } else { v };
            return true;
        }
        if dmode == 7 && dreg > 1 {
            return false;
        }
        let Some(dst) = self.ea(bus, dmode, dreg, s) else { return false };
        self.set_logic(v, s);
        self.write_ea(bus, dst, s, v);
        true
    }

    fn group4<B: Bus68>(&mut self, bus: &mut B, op: u16, mode: u16, reg: u16, rx: usize) -> bool {
        if op & 0x1C0 == 0x1C0 {
            // LEA
            let Some(a) = self.control_addr(bus, mode, reg) else { return false };
            self.a[rx] = a;
            return true;
        }
        if op & 0x1C0 == 0x180 {
            // CHK.W
            let Some(ea) = self.ea(bus, mode, reg, Size::W) else { return false };
            let bound = self.read_ea(bus, ea, Size::W) as i16;
            let v = self.d[rx] as i16;
            if v < 0 || v > bound {
                self.set_flag(SR_N, v < 0);
                let pc = self.pc;
                self.exception(bus, VEC_CHK, pc);
            }
            return true;
        }
        match op {
            0x4AFC => return false, // ILLEGAL
            0x4E70 => {
                // RESET: the bus's devices reset; the CPU goes on
                if !self.supervisor() {
                    self.privilege(bus);
                }
                self.cycles += 128;
                return true;
            }
            0x4E71 => return true,
            0x4E72 => {
                if !self.supervisor() {
                    self.privilege(bus);
                    return true;
                }
                let v = self.fetch(bus) as u16;
                self.set_sr(v);
                self.stopped = true;
                return true;
            }
            0x4E73 => {
                if !self.supervisor() {
                    self.privilege(bus);
                    return true;
                }
                let sr = self.pop16(bus);
                let pc = self.pop32(bus);
                self.set_sr(sr);
                self.pc = pc;
                self.cycles += 16;
                return true;
            }
            0x4E75 => {
                self.pc = self.pop32(bus);
                self.cycles += 12;
                return true;
            }
            0x4E76 => {
                if self.flag(SR_V) {
                    let pc = self.pc;
                    self.exception(bus, VEC_TRAPV, pc);
                }
                return true;
            }
            0x4E77 => {
                let ccr = self.pop16(bus);
                self.sr = (self.sr & 0xFF00) | (ccr & 0x1F);
                self.pc = self.pop32(bus);
                return true;
            }
            _ => {}
        }
        match op & 0xFFF0 {
            0x4E40 => {
                let pc = self.pc;
                self.exception(bus, VEC_TRAP + (op as u32 & 0xF), pc);
                return true;
            }
            0x4E50 => {
                if op & 8 == 0 {
                    // LINK
                    let disp = sext16(self.fetch(bus));
                    let v = self.a[reg as usize];
                    self.push32(bus, v);
                    self.a[reg as usize] = self.a[7];
                    self.a[7] = self.a[7].wrapping_add(disp);
                } else {
                    // UNLK
                    self.a[7] = self.a[reg as usize];
                    self.a[reg as usize] = self.pop32(bus);
                }
                return true;
            }
            0x4E60 => {
                if !self.supervisor() {
                    self.privilege(bus);
                    return true;
                }
                if op & 8 == 0 { self.set_usp(self.a[reg as usize]) } else { self.a[reg as usize] = self.usp() }
                return true;
            }
            _ => {}
        }
        match op & 0xFFC0 {
            0x4E80 | 0x4EC0 => {
                // JSR / JMP
                let Some(a) = self.control_addr(bus, mode, reg) else { return false };
                if op & 0x40 == 0 {
                    let ret = self.pc;
                    self.push32(bus, ret);
                }
                self.pc = a;
                self.cycles += 4;
                return true;
            }
            0x40C0 => {
                // MOVE from SR
                if mode == 1 {
                    return false;
                }
                let Some(ea) = self.ea(bus, mode, reg, Size::W) else { return false };
                if matches!(ea, Ea::Imm(_)) {
                    return false;
                }
                let _ = self.read_ea(bus, ea, Size::W); // the 68000 reads before it writes
                let sr = self.sr as u32;
                self.write_ea(bus, ea, Size::W, sr);
                return true;
            }
            0x44C0 => {
                // MOVE to CCR
                if mode == 1 {
                    return false;
                }
                let Some(ea) = self.ea(bus, mode, reg, Size::W) else { return false };
                let v = self.read_ea(bus, ea, Size::W) as u16;
                self.sr = (self.sr & 0xFF00) | (v & 0x1F);
                return true;
            }
            0x46C0 => {
                // MOVE to SR
                if !self.supervisor() {
                    self.privilege(bus);
                    return true;
                }
                if mode == 1 {
                    return false;
                }
                let Some(ea) = self.ea(bus, mode, reg, Size::W) else { return false };
                let v = self.read_ea(bus, ea, Size::W) as u16;
                self.set_sr(v);
                return true;
            }
            0x4800 => {
                // NBCD
                if mode == 1 {
                    return false;
                }
                let Some(ea) = self.ea(bus, mode, reg, Size::B) else { return false };
                let v = self.read_ea(bus, ea, Size::B);
                let r = self.sbcd(0, v);
                self.write_ea(bus, ea, Size::B, r);
                return true;
            }
            0x4840 => {
                if mode == 0 {
                    // SWAP
                    let v = self.d[reg as usize].rotate_left(16);
                    self.d[reg as usize] = v;
                    self.set_logic(v, Size::L);
                } else {
                    // PEA
                    let Some(a) = self.control_addr(bus, mode, reg) else { return false };
                    self.push32(bus, a);
                }
                return true;
            }
            0x4AC0 => {
                // TAS
                if mode == 1 {
                    return false;
                }
                let Some(ea) = self.ea(bus, mode, reg, Size::B) else { return false };
                let v = self.read_ea(bus, ea, Size::B);
                self.set_logic(v, Size::B);
                self.write_ea(bus, ea, Size::B, v | 0x80);
                return true;
            }
            _ => {}
        }
        // EXT and MOVEM
        if op & 0xFB80 == 0x4880 {
            if mode == 0 {
                let r = reg as usize;
                if op & 0x40 == 0 {
                    let v = (self.d[r] & 0xFFFF_0000) | (sext8(self.d[r]) & 0xFFFF);
                    self.d[r] = v;
                    self.set_logic(v, Size::W);
                } else {
                    let v = sext16(self.d[r]);
                    self.d[r] = v;
                    self.set_logic(v, Size::L);
                }
                return true;
            }
            return self.movem(bus, op, mode, reg);
        }
        // NEGX CLR NEG NOT TST: 0100 xxx0 ss
        let kind = (op >> 8) & 0xF;
        let Some(s) = Size::from_bits(op >> 6) else { return false };
        if mode == 1 {
            return false;
        }
        match kind {
            0x0 | 0x2 | 0x4 | 0x6 | 0xA => {
                let Some(ea) = self.ea(bus, mode, reg, s) else { return false };
                if kind != 0xA && matches!(ea, Ea::Imm(_)) {
                    return false;
                }
                let v = self.read_ea(bus, ea, s);
                let r = match kind {
                    0x0 => self.sub(0, v, s, true, false),
                    0x2 => {
                        // CLR (the read above is the 68000's: it reads before it writes)
                        self.sr = (self.sr & !(SR_N | SR_V | SR_C)) | SR_Z;
                        0
                    }
                    0x4 => self.sub(0, v, s, false, false),
                    0x6 => {
                        let r = !v & s.mask();
                        self.set_logic(r, s);
                        r
                    }
                    _ => {
                        self.set_logic(v, s);
                        return true;
                    }
                };
                self.write_ea(bus, ea, s, r);
                true
            }
            _ => false,
        }
    }

    fn privilege<B: Bus68>(&mut self, bus: &mut B) {
        let pc = self.op_pc;
        self.exception(bus, VEC_PRIVILEGE, pc);
    }

    fn movem<B: Bus68>(&mut self, bus: &mut B, op: u16, mode: u16, reg: u16) -> bool {
        let mask = self.fetch(bus);
        let s = if op & 0x40 != 0 { Size::L } else { Size::W };
        let to_regs = op & 0x400 != 0;
        let r = reg as usize;
        if mode == 4 && !to_regs {
            // registers to -(An): A7 first down to D0, mask reversed
            let mut addr = self.a[r];
            for i in 0..16 {
                if mask & (1 << i) != 0 {
                    let idx = 15 - i;
                    addr = addr.wrapping_sub(s.bytes());
                    let v = if idx >= 8 { self.a[idx - 8] } else { self.d[idx] };
                    self.write(bus, addr, s, v);
                }
            }
            self.a[r] = addr;
            return true;
        }
        let mut addr = if mode == 3 {
            self.a[r]
        } else {
            match self.control_addr(bus, mode, reg) {
                Some(a) => a,
                None => return false,
            }
        };
        for i in 0..16 {
            if mask & (1 << i) == 0 {
                continue;
            }
            if to_regs {
                let mut v = self.read(bus, addr, s);
                if s == Size::W {
                    v = sext16(v);
                }
                if i >= 8 { self.a[i - 8] = v } else { self.d[i] = v }
            } else {
                let v = if i >= 8 { self.a[i - 8] } else { self.d[i] };
                self.write(bus, addr, s, v);
            }
            addr = addr.wrapping_add(s.bytes());
        }
        if to_regs {
            // the 68000 reads one more word after MOVEM to registers
            let _ = bus.read16(addr & 0xFF_FFFF);
        }
        if mode == 3 {
            self.a[r] = addr;
        }
        true
    }

    fn group5<B: Bus68>(&mut self, bus: &mut B, op: u16, mode: u16, reg: u16) -> bool {
        if op & 0xC0 == 0xC0 {
            let cond = (op >> 8) & 0xF;
            if mode == 1 {
                // DBcc
                let disp = sext16(self.fetch(bus));
                if !self.cond(cond) {
                    let r = reg as usize;
                    let c = (self.d[r] as u16).wrapping_sub(1);
                    self.d[r] = (self.d[r] & 0xFFFF_0000) | c as u32;
                    if c != 0xFFFF {
                        self.pc = self.op_pc.wrapping_add(2).wrapping_add(disp);
                    }
                }
                self.cycles += 6;
                return true;
            }
            // Scc
            let Some(ea) = self.ea(bus, mode, reg, Size::B) else { return false };
            if matches!(ea, Ea::Imm(_)) {
                return false;
            }
            let v = if self.cond(cond) { 0xFF } else { 0 };
            if let Ea::Mem(a) = ea {
                let _ = bus.read8(a & 0xFF_FFFF); // read-modify-write on the 68000
            }
            self.write_ea(bus, ea, Size::B, v);
            return true;
        }
        // ADDQ / SUBQ
        let Some(s) = Size::from_bits(op >> 6) else { return false };
        let q = match (op >> 9) & 7 {
            0 => 8,
            n => n as u32,
        };
        if mode == 1 {
            if s == Size::B {
                return false;
            }
            let r = reg as usize;
            self.a[r] = if op & 0x100 == 0 { self.a[r].wrapping_add(q) } else { self.a[r].wrapping_sub(q) };
            return true;
        }
        let Some(ea) = self.ea(bus, mode, reg, s) else { return false };
        if matches!(ea, Ea::Imm(_)) {
            return false;
        }
        let v = self.read_ea(bus, ea, s);
        let r = if op & 0x100 == 0 { self.add(v, q, s, false) } else { self.sub(v, q, s, false, false) };
        self.write_ea(bus, ea, s, r);
        true
    }

    /// 0x8 (OR DIVU DIVS SBCD) and 0xC (AND MULU MULS ABCD EXG).
    fn group8c<B: Bus68>(&mut self, bus: &mut B, op: u16, mode: u16, reg: u16, rx: usize, and: bool) -> bool {
        let opmode = (op >> 6) & 7;
        if opmode == 3 || opmode == 7 {
            // DIVU/DIVS or MULU/MULS (word)
            if mode == 1 {
                return false;
            }
            let Some(ea) = self.ea(bus, mode, reg, Size::W) else { return false };
            let src = self.read_ea(bus, ea, Size::W);
            let signed = opmode == 7;
            if and {
                let r = if signed {
                    ((self.d[rx] as i16 as i32) * (src as i16 as i32)) as u32
                } else {
                    (self.d[rx] & 0xFFFF) * src
                };
                self.d[rx] = r;
                self.set_logic(r, Size::L);
                self.cycles += 66;
            } else {
                self.divide(bus, rx, src, signed);
            }
            return true;
        }
        if op & 0x1F0 == 0x100 {
            // SBCD / ABCD
            let r = reg as usize;
            let (src, dst) = if op & 8 == 0 {
                (self.d[r] & 0xFF, self.d[rx] & 0xFF)
            } else {
                self.a[r] = self.a[r].wrapping_sub(if r == 7 { 2 } else { 1 });
                let s = bus.read8(self.a[r] & 0xFF_FFFF) as u32;
                self.a[rx] = self.a[rx].wrapping_sub(if rx == 7 { 2 } else { 1 });
                let d = bus.read8(self.a[rx] & 0xFF_FFFF) as u32;
                (s, d)
            };
            let res = if and { self.abcd(dst, src) } else { self.sbcd(dst, src) };
            if op & 8 == 0 {
                self.d[rx] = (self.d[rx] & 0xFFFF_FF00) | res;
            } else {
                bus.write8(self.a[rx] & 0xFF_FFFF, res as u8);
            }
            return true;
        }
        if and && op & 0x130 == 0x100 {
            // EXG
            let r = reg as usize;
            match op & 0xF8 {
                0x40 => self.d.swap(rx, r),
                0x48 => self.a.swap(rx, r),
                0x88 => std::mem::swap(&mut self.d[rx], &mut self.a[r]),
                _ => return false,
            }
            return true;
        }
        let Some(s) = Size::from_bits(op >> 6) else { return false };
        let to_ea = op & 0x100 != 0;
        if mode == 1 {
            return false;
        }
        let Some(ea) = self.ea(bus, mode, reg, s) else { return false };
        if to_ea && matches!(ea, Ea::D(_) | Ea::Imm(_)) {
            return false;
        }
        let v = self.read_ea(bus, ea, s);
        let r = if and { v & self.d[rx] } else { v | self.d[rx] } & s.mask();
        self.set_logic(r, s);
        if to_ea {
            self.write_ea(bus, ea, s, r);
        } else {
            self.write_ea(bus, Ea::D(rx), s, r);
        }
        true
    }

    fn divide<B: Bus68>(&mut self, bus: &mut B, rx: usize, src: u32, signed: bool) {
        if src == 0 {
            let pc = self.pc;
            self.exception(bus, VEC_ZERO_DIVIDE, pc);
            return;
        }
        let dividend = self.d[rx];
        self.cycles += 136;
        if signed {
            let (n, d) = (dividend as i32 as i64, src as i16 as i64);
            let q = n / d;
            let r = n % d;
            if q > i16::MAX as i64 || q < i16::MIN as i64 {
                self.sr |= SR_V | SR_N;
                self.sr &= !SR_C;
                return;
            }
            self.d[rx] = ((r as u32 & 0xFFFF) << 16) | (q as u32 & 0xFFFF);
            self.set_nz(q as u32, Size::W);
            self.sr &= !(SR_V | SR_C);
        } else {
            let q = dividend / src;
            let r = dividend % src;
            if q > 0xFFFF {
                self.sr |= SR_V | SR_N;
                self.sr &= !SR_C;
                return;
            }
            self.d[rx] = (r << 16) | q;
            self.set_nz(q, Size::W);
            self.sr &= !(SR_V | SR_C);
        }
    }

    /// 0x9 (SUB SUBA SUBX) and 0xD (ADD ADDA ADDX).
    fn group9d<B: Bus68>(&mut self, bus: &mut B, op: u16, mode: u16, reg: u16, rx: usize, add: bool) -> bool {
        let opmode = (op >> 6) & 7;
        if opmode == 3 || opmode == 7 {
            // SUBA / ADDA
            let s = if opmode == 3 { Size::W } else { Size::L };
            let Some(ea) = self.ea(bus, mode, reg, s) else { return false };
            let mut v = self.read_ea(bus, ea, s);
            if s == Size::W {
                v = sext16(v);
            }
            self.a[rx] = if add { self.a[rx].wrapping_add(v) } else { self.a[rx].wrapping_sub(v) };
            return true;
        }
        let Some(s) = Size::from_bits(op >> 6) else { return false };
        if op & 0x130 == 0x100 {
            // SUBX / ADDX
            let r = reg as usize;
            if op & 8 == 0 {
                let (src, dst) = (self.d[r], self.d[rx]);
                let res = if add { self.add(dst, src, s, true) } else { self.sub(dst, src, s, true, false) };
                self.write_ea(bus, Ea::D(rx), s, res);
            } else {
                let step = |r: usize| if r == 7 && s == Size::B { 2 } else { s.bytes() };
                self.a[r] = self.a[r].wrapping_sub(step(r));
                let src = self.read(bus, self.a[r], s);
                self.a[rx] = self.a[rx].wrapping_sub(step(rx));
                let dst = self.read(bus, self.a[rx], s);
                let res = if add { self.add(dst, src, s, true) } else { self.sub(dst, src, s, true, false) };
                let a = self.a[rx];
                self.write(bus, a, s, res);
            }
            return true;
        }
        let to_ea = op & 0x100 != 0;
        if s == Size::B && mode == 1 {
            return false;
        }
        let Some(ea) = self.ea(bus, mode, reg, s) else { return false };
        if to_ea && matches!(ea, Ea::D(_) | Ea::A(_) | Ea::Imm(_)) {
            return false;
        }
        let v = self.read_ea(bus, ea, s);
        let d = self.d[rx];
        if to_ea {
            let r = if add { self.add(v, d, s, false) } else { self.sub(v, d, s, false, false) };
            self.write_ea(bus, ea, s, r);
        } else {
            let r = if add { self.add(d, v, s, false) } else { self.sub(d, v, s, false, false) };
            self.write_ea(bus, Ea::D(rx), s, r);
        }
        true
    }

    /// 0xB: CMP CMPA CMPM EOR.
    fn group_b<B: Bus68>(&mut self, bus: &mut B, op: u16, mode: u16, reg: u16, rx: usize) -> bool {
        let opmode = (op >> 6) & 7;
        if opmode == 3 || opmode == 7 {
            let s = if opmode == 3 { Size::W } else { Size::L };
            let Some(ea) = self.ea(bus, mode, reg, s) else { return false };
            let mut v = self.read_ea(bus, ea, s);
            if s == Size::W {
                v = sext16(v);
            }
            let d = self.a[rx];
            self.sub(d, v, Size::L, false, true);
            return true;
        }
        let Some(s) = Size::from_bits(op >> 6) else { return false };
        if op & 0x100 == 0 {
            if s == Size::B && mode == 1 {
                return false;
            }
            let Some(ea) = self.ea(bus, mode, reg, s) else { return false };
            let v = self.read_ea(bus, ea, s);
            let d = self.d[rx];
            self.sub(d, v, s, false, true);
            return true;
        }
        if mode == 1 {
            // CMPM (Ay)+,(Ax)+
            let r = reg as usize;
            let step = |r: usize| if r == 7 && s == Size::B { 2 } else { s.bytes() };
            let src = self.read(bus, self.a[r], s);
            self.a[r] = self.a[r].wrapping_add(step(r));
            let dst = self.read(bus, self.a[rx], s);
            self.a[rx] = self.a[rx].wrapping_add(step(rx));
            self.sub(dst, src, s, false, true);
            return true;
        }
        // EOR Dn,<ea>
        let Some(ea) = self.ea(bus, mode, reg, s) else { return false };
        if matches!(ea, Ea::Imm(_)) {
            return false;
        }
        let v = self.read_ea(bus, ea, s);
        let r = (v ^ self.d[rx]) & s.mask();
        self.set_logic(r, s);
        self.write_ea(bus, ea, s, r);
        true
    }

    /// 0xE: shifts and rotates.
    fn group_e<B: Bus68>(&mut self, bus: &mut B, op: u16, mode: u16, reg: u16, rx: usize) -> bool {
        if op & 0xC0 == 0xC0 {
            // memory: one bit, word size; the kind in bits 10-9
            if op & 0x800 != 0 || mode < 2 {
                return false;
            }
            let Some(ea) = self.ea(bus, mode, reg, Size::W) else { return false };
            if matches!(ea, Ea::Imm(_)) {
                return false;
            }
            let v = self.read_ea(bus, ea, Size::W);
            let r = self.shift((op >> 9) & 3, op & 0x100 != 0, v, 1, Size::W);
            self.write_ea(bus, ea, Size::W, r);
            return true;
        }
        let Some(s) = Size::from_bits(op >> 6) else { return false };
        let count = if op & 0x20 != 0 {
            self.d[rx] & 63
        } else {
            match rx {
                0 => 8,
                n => n as u32,
            }
        };
        let r = reg as usize;
        let v = self.d[r] & s.mask();
        let res = self.shift((op >> 3) & 3, op & 0x100 != 0, v, count, s);
        self.write_ea(bus, Ea::D(r), s, res);
        self.cycles += 2 * count;
        true
    }

    /// A shift or rotate: `kind` 0 arithmetic, 1 logical, 2 rotate through X, 3 rotate.
    fn shift(&mut self, kind: u16, left: bool, v: u32, count: u32, s: Size) -> u32 {
        let bits = s.bytes() * 8;
        let m = s.mask();
        let msb = s.msb();
        let mut v = v & m;
        let mut c = false;
        let mut overflow = false;
        let x0 = self.flag(SR_X);
        let mut x = x0;
        for _ in 0..count {
            match (kind, left) {
                (0, true) => {
                    c = v & msb != 0;
                    let nv = (v << 1) & m;
                    overflow |= (nv ^ v) & msb != 0;
                    v = nv;
                    x = c;
                }
                (0, false) => {
                    c = v & 1 != 0;
                    v = (v >> 1) | (v & msb);
                    x = c;
                }
                (1, true) => {
                    c = v & msb != 0;
                    v = (v << 1) & m;
                    x = c;
                }
                (1, false) => {
                    c = v & 1 != 0;
                    v >>= 1;
                    x = c;
                }
                (2, true) => {
                    c = v & msb != 0;
                    v = ((v << 1) | x as u32) & m;
                    x = c;
                }
                (2, false) => {
                    c = v & 1 != 0;
                    v = (v >> 1) | if x { msb } else { 0 };
                    x = c;
                }
                (_, true) => {
                    c = v & msb != 0;
                    v = ((v << 1) | c as u32) & m;
                }
                (_, false) => {
                    c = v & 1 != 0;
                    v = (v >> 1) | if c { msb } else { 0 };
                }
            }
        }
        let _ = bits;
        self.set_nz(v, s);
        self.set_flag(SR_V, kind == 0 && left && overflow);
        if count == 0 {
            // C is cleared, or for ROXL/ROXR set to X; X is unchanged
            self.set_flag(SR_C, kind == 2 && x0);
        } else {
            self.set_flag(SR_C, c);
            if kind != 3 {
                self.set_flag(SR_X, x);
            }
        }
        v
    }
}
