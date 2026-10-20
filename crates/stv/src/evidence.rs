//! Evidence of what a program runs (development): the addresses each SH-2 executed, the targets of
//! its calls, computed jumps and returns, and where its interrupts entered: the map of the code a
//! port is made from. Code in a game's overlay window (as the client gives it) is told apart by a
//! fingerprint of the window's code, taken once a frame when code there runs.

use std::collections::HashMap;
use std::ops::Range;

/// A window of work RAM-H that holds different code from mode to mode (a game's overlays: a stage's
/// or a mode's code is loaded there), told apart by a fingerprint of `print`, the part of the window
/// that holds code (the rest may hold data). The client says where; the runtime knows no game.
#[derive(Clone, Debug)]
pub struct Overlay {
    pub window: Range<u32>,
    pub print: Range<u32>,
}

impl Overlay {
    /// Whether an address (any cache mirror) is in the window.
    #[inline]
    pub fn holds(&self, a: u32) -> bool {
        self.window.contains(&(a & 0x07FF_FFFF))
    }

    /// Whether a work RAM-H offset is in the code the fingerprint covers.
    #[inline]
    pub fn prints(&self, off: u32) -> bool {
        off.wrapping_sub(self.print.start & 0xF_FFFF) < self.print.end - self.print.start
    }

    /// The fingerprint of the window's code (`wram_h`: work RAM-H).
    pub fn fingerprint(&self, wram_h: &[u8]) -> u64 {
        crate::mem::fnv1a(&wram_h[(self.print.start & 0xF_FFFF) as usize..(self.print.end & 0xF_FFFF) as usize])
    }
}

#[derive(Default)]
pub struct Evidence {
    /// The game's overlay window, if it has one.
    pub overlay: Option<Overlay>,
    /// (CPU, overlay fingerprint (0 outside the window), address): times executed.
    pub executed: HashMap<(u8, u64, u32), u64>,
    /// (CPU, overlay fingerprint (0 unless the site or the target is in the window), kind, site,
    /// target): b'c' a call (BSR, BSRF, JSR), b'j' a computed jump (JMP, BRAF), b'r' a return (RTS,
    /// RTE).
    pub transfers: HashMap<(u8, u64, u8, u32, u32), u64>,
    /// (CPU, overlay fingerprint (0 unless the handler is in the window), vector, handler):
    /// interrupts taken.
    pub interrupts: HashMap<(u8, u64, u32, u32), u64>,
    /// The overlay fingerprints seen, with the first frame each was seen in, and the window as it
    /// was the last frame each was (its code, for the port's translation).
    pub overlays: HashMap<u64, u64>,
    pub code: HashMap<u64, Vec<u8>>,
    /// This frame's fingerprint, once taken.
    pub overlay_now: Option<u64>,
}

impl Evidence {
    /// The text form: one record a line (tools/port reads it).
    ///   x CPU OVERLAY ADDRESS COUNT, t CPU OVERLAY KIND SITE TARGET COUNT,
    ///   i CPU OVERLAY VECTOR HANDLER COUNT, o OVERLAY FIRST_FRAME (hex, but counts and frames)
    pub fn to_text(&self) -> String {
        let mut lines: Vec<String> = Vec::new();
        let mut o: Vec<_> = self.overlays.iter().collect();
        o.sort();
        lines.extend(o.iter().map(|(fp, f)| format!("o {fp:016x} {f}")));
        let mut x: Vec<_> = self.executed.iter().collect();
        x.sort();
        lines.extend(x.iter().map(|((c, fp, a), n)| format!("x {c} {fp:016x} {a:08x} {n}")));
        let mut t: Vec<_> = self.transfers.iter().collect();
        t.sort();
        lines.extend(t.iter().map(|((c, fp, k, s, d), n)| format!("t {c} {fp:016x} {} {s:08x} {d:08x} {n}", *k as char)));
        let mut i: Vec<_> = self.interrupts.iter().collect();
        i.sort();
        lines.extend(i.iter().map(|((c, fp, v, h), n)| format!("i {c} {fp:016x} {v:02x} {h:08x} {n}")));
        lines.join("\n") + "\n"
    }
}

/// An instruction's transfer, if it is one: (kind, target) from the CPU's registers before it runs.
pub fn transfer(op: u16, pc: u32, r: &[u32; 16], pr: u32) -> Option<(u8, u32)> {
    let n = ((op >> 8) & 0xF) as usize;
    match op {
        _ if op & 0xF000 == 0xB000 => {
            let d = (((op & 0xFFF) as i32) << 20) >> 20;
            Some((b'c', pc.wrapping_add(4).wrapping_add((d * 2) as u32)))
        }
        _ if op & 0xF0FF == 0x0003 => Some((b'c', pc.wrapping_add(4).wrapping_add(r[n]))),
        _ if op & 0xF0FF == 0x400B => Some((b'c', r[n])),
        _ if op & 0xF0FF == 0x402B => Some((b'j', r[n])),
        _ if op & 0xF0FF == 0x0023 => Some((b'j', pc.wrapping_add(4).wrapping_add(r[n]))),
        0x000B => Some((b'r', pr)),
        0x002B => Some((b'r', 0)),
        _ => None,
    }
}
