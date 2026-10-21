//! Native code: a game's functions ported to Rust run in place of its SH-2 code, behind the same bus.
//! The client registers each one at its entry address; when an SH-2 reaches the address (not in a
//! delay slot), the machine calls the Rust function instead, with the CPU's registers, and goes on at
//! PR as RTS would. A native function calls another through [`call`]: the registered Rust function if
//! there is one, else the SH-2 code, interpreted until it returns.
//!
//! Native code runs on a stack of its own (the machine's fibers), so it can wait as the SH-2 code
//! did: each block charges its cycles through [`tick`], and when the CPU's turn is spent the machine
//! runs the rest of the system before going on with it; interrupts are taken there too, between
//! blocks.
//!
//! A game's overlay window (stv::evidence::Overlay, as the client gives it) holds different code
//! from mode to mode: a function there is registered with the window's fingerprint as its code was
//! loaded, and runs only while that is what the window holds.

use crate::evidence::Overlay;
use crate::sh2::{Bus, Sh2};
use std::collections::HashMap;

pub type NativeFn = fn(&mut Sh2, &mut dyn Bus);

#[derive(Clone, Copy)]
pub struct Native {
    pub f: NativeFn,
    pub name: &'static str,
    /// For code in the overlay window: the window's fingerprint the function belongs to.
    pub overlay: Option<u64>,
    pub contract: Contract,
}

/// What a native function keeps of the SH-2 code it replaces (what compare mode checks).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Contract {
    /// A translation: every access the SH-2 code made, in order, and every register at its end.
    #[default]
    Exact,
    /// A rewrite keeping GCC's calling convention for the SH: the accesses outside its stack frame,
    /// in order, but for the literals the SH-2 code loads from beside itself; at its end its result
    /// (r0) and what a function keeps for its caller (r8-r15, GBR, VBR, SR's interrupt mask).
    Abi,
    /// The same for a function with no result (r0 is whatever it was left as).
    AbiVoid,
    /// A rewrite of code with a convention of its own: the accesses as for `Abi`, and at its end
    /// only the registers in the mask (bit n: rn) its caller relies on, GBR, VBR and SR's
    /// interrupt mask.
    Keeps(u16),
}

impl Contract {
    /// The registers kept for the caller (bit n: rn), for a rewrite.
    pub fn kept(self) -> u16 {
        match self {
            Contract::Exact => 0xffff,
            Contract::Abi => 0xff01,
            Contract::AbiVoid => 0xff00,
            Contract::Keeps(mask) => mask,
        }
    }
}

/// The registered functions.
#[derive(Default)]
pub struct Natives {
    map: HashMap<u32, Vec<Native>>,
    /// Work RAM-H's halfwords that start a function (a quick no).
    entries: Vec<u64>,
    /// The game's overlay window, if it has one, and its fingerprint while nothing has written its
    /// code since.
    pub overlay: Option<Overlay>,
    pub overlay_now: Option<u64>,
    /// Calls made natively, and calls native code passed to the interpreter.
    pub native_calls: u64,
    pub interpreted_calls: u64,
    /// The calls passed to the interpreter by their target: how many, and the instructions the
    /// interpreter ran for them (with what they called).
    pub interpreted_by: HashMap<u32, (u64, u64)>,
}

/// Folds the cache mirrors: the address as work RAM-H's offset, if it is in work RAM-H.
#[inline]
fn wram_h_offset(a: u32) -> Option<u32> {
    let p = a & 0x07FF_FFFF;
    (0x0600_0000..0x0800_0000).contains(&p).then_some(p & 0xF_FFFF)
}

impl Natives {
    /// Registers a translation (Contract::Exact) at `addr`, in place of what was registered there
    /// for the same overlay.
    pub fn register(&mut self, addr: u32, f: NativeFn, name: &'static str, overlay: Option<u64>) {
        self.register_as(addr, f, name, overlay, Contract::Exact);
    }

    /// Registers a native function with its contract, in place of what was registered at `addr` for
    /// the same overlay.
    pub fn register_as(&mut self, addr: u32, f: NativeFn, name: &'static str, overlay: Option<u64>, contract: Contract) {
        let Some(off) = wram_h_offset(addr) else { panic!("{name}: native code must be in work RAM-H") };
        if self.entries.is_empty() {
            self.entries = vec![0; 0x10_0000 / 2 / 64];
        }
        let i = (off >> 1) as usize;
        self.entries[i / 64] |= 1 << (i % 64);
        let list = self.map.entry(off).or_default();
        list.retain(|n| n.overlay != overlay);
        list.push(Native { f, name, overlay, contract });
    }

    /// The name of a function registered at `addr` (any of them, for overlay code).
    pub fn name(&self, addr: u32) -> &'static str {
        wram_h_offset(addr).and_then(|off| self.map.get(&off)).and_then(|l| l.first()).map_or("", |n| n.name)
    }

    pub fn len(&self) -> usize {
        self.map.values().map(Vec::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Whether a function may start here (the quick test before `get`).
    #[inline]
    pub fn maybe(&self, pc: u32) -> bool {
        match wram_h_offset(pc) {
            Some(off) if !self.entries.is_empty() => {
                let i = (off >> 1) as usize;
                self.entries[i / 64] >> (i % 64) & 1 != 0
            }
            _ => false,
        }
    }

    /// The function registered at `pc` for the code there now (`wram_h`: work RAM-H, for the
    /// overlay window's fingerprint).
    pub fn get(&mut self, pc: u32, wram_h: &[u8]) -> Option<Native> {
        if !self.maybe(pc) {
            return None;
        }
        let off = wram_h_offset(pc)?;
        let list = self.map.get(&off)?;
        let Some(o) = self.overlay.as_ref().filter(|o| o.holds(0x0600_0000 | off)) else {
            return list.first().copied();
        };
        let fp = *self.overlay_now.get_or_insert_with(|| o.fingerprint(wram_h));
        list.iter().find(|n| n.overlay == Some(fp)).copied()
    }

    /// A write to work RAM-H (offset `off`): the overlay's fingerprint is taken again if it may
    /// have changed.
    #[inline]
    pub fn wrote(&mut self, off: u32) {
        if self.overlay.as_ref().is_some_and(|o| o.prints(off)) {
            self.overlay_now = None;
        }
    }
}

/// A call from native code (the caller has set PR and run the delay slot): the registered Rust
/// function at `target` if there is one, else the SH-2 code until it returns.
pub fn call(cpu: &mut Sh2, bus: &mut dyn Bus, target: u32) {
    match bus.native_at(target) {
        Some(f) => f(cpu, bus),
        None => interpret_call(cpu, bus, target),
    }
}

/// The PC an interrupt taken in native code saves (odd: no code is there); the handler's RTE comes
/// back to it, and the native code goes on.
pub const NATIVE_PC: u32 = 0xFFFF_FFFF;

/// Native code's cost: the block starting at `at` ran, `cycles` more on the machine's clock (the
/// machine runs on when the CPU's turn is spent), then the interrupt due, if one is, its handler run
/// to its return.
pub fn tick(cpu: &mut Sh2, bus: &mut dyn Bus, at: u32, cycles: u32) {
    bus.tick(cycles);
    bus.pass(cpu, at);
    let (pc, sp) = (cpu.pc, cpu.r[15]);
    cpu.pc = NATIVE_PC;
    if bus.take_interrupt(cpu) {
        run_until(cpu, bus, NATIVE_PC, sp);
    }
    cpu.pc = pc;
}

/// A computed jump native code has no block for: the SH-2 code from `target` until the function
/// returns to its caller (`ret`, with the stack at `sp`, as they were on entry).
pub fn goto(cpu: &mut Sh2, bus: &mut dyn Bus, target: u32, ret: u32, sp: u32) {
    let start = bus.interpreted();
    cpu.pc = target;
    run_until(cpu, bus, ret, sp);
    let steps = bus.interpreted() - start;
    bus.note_interpreted(target, steps);
}

/// Runs SH-2 code from `target` until it returns to PR with the stack where it was (natives it
/// reaches run natively).
pub fn interpret_call(cpu: &mut Sh2, bus: &mut dyn Bus, target: u32) {
    let start = bus.interpreted();
    let (ret, sp) = (cpu.pr, cpu.r[15]);
    cpu.pc = target;
    run_until(cpu, bus, ret, sp);
    let steps = bus.interpreted() - start;
    bus.note_interpreted(target, steps);
}

/// Runs the CPU as the machine would, taking interrupts, until it reaches `ret` with the stack at
/// `sp`.
fn run_until(cpu: &mut Sh2, bus: &mut dyn Bus, ret: u32, sp: u32) {
    loop {
        if cpu.delay.is_none() && bus.take_interrupt(cpu) {
            bus.tick(4);
        } else if cpu.sleeping {
            bus.tick(8);
        } else if cpu.delay.is_none()
            && let Some(f) = bus.native_at(cpu.pc)
        {
            f(cpu, bus);
            bus.tick(1);
        } else {
            cpu.step(bus);
            bus.tick(1);
        }
        if cpu.pc == ret && cpu.r[15] == sp && cpu.delay.is_none() {
            return;
        }
    }
}
