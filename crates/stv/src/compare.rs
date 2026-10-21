//! Compare mode (development): the interpreter runs the game, and each call it makes to a function
//! the port has (stv::native) is checked against the port. The call's accesses are logged as the
//! interpreter makes them; when it returns, the native function runs from the CPU's state at the
//! call against the log: its reads are answered from the log and must be the same reads in the
//! same order, its writes must be the writes the SH-2 code made, and it must end with the same
//! registers. The native run touches nothing: the machine goes on from the interpreter's run.
//!
//! An interrupt taken during the call is the handler's business: its accesses are logged a level
//! deeper and left out of the call's (what it changed, the call's reads see). Calls inside calls
//! are checked each on its own, and in their caller's run natively.
//!
//! A rewrite (native::Contract::Abi), and any call with one inside it, is checked by what it does
//! instead of by each access. Its stack frame (below the stack pointer it was called with, as deep
//! as the SH-2 code went) is its own business, and so are the SH-2 code's loads of its literals.
//! Memory (the RAMs and video and sound memories) is checked by value: reads are answered from what
//! the memory held (as the SH-2 code read it, as the native run wrote it, or, for bytes the SH-2
//! code did not touch, as the machine holds them still) and the bytes written must end the same, in
//! any order and number. The hardware's registers, and memory that changed
//! under the call (written by an interrupt, the other CPU or the sound CPU, as the SH-2 code's reads
//! show), keep their accesses in order. At the end, only the result and the registers kept for the
//! caller must be the same.

use crate::native::Contract;
use crate::sh2::{Bus, Sh2, SR_MASK};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ops::Range;

/// One access the CPU made: b'R' or b'W', its size in bytes, the interrupt depth it was made at;
/// b'P', an instruction's address (where the accesses after it were made); b'O', before a write to
/// work RAM, what it overwrites.
#[derive(Clone, Copy, Debug)]
pub struct Access {
    pub kind: u8,
    pub size: u8,
    pub depth: u8,
    pub addr: u32,
    pub value: u32,
}

/// A call being logged.
struct Call {
    entry: u32,
    /// The native function's name (overlay code has one at an entry for each of its codes).
    name: &'static str,
    /// Where its accesses start in the CPU's log.
    start: usize,
    state: Sh2,
    ret: u32,
    sp: u32,
    depth: u8,
    /// The deepest the stack went during the call.
    min_sp: u32,
    /// Checked by the calling convention (a rewrite is in it), and the registers it keeps then
    /// (a translation's are taken as GCC's with no result).
    relaxed: bool,
    kept: u16,
}

/// What compare mode found for one function.
#[derive(Default, Debug)]
pub struct Verdict {
    pub calls: u64,
    pub equal: u64,
    pub diverged: u64,
    /// Calls not checked: their log grew past the cap.
    pub too_long: u64,
    /// The first divergences (frame, what).
    pub first: Vec<(u64, String)>,
}

/// The accesses kept at most (a call that is still running when the log grows past this is not
/// checked).
const CAP: usize = 4 << 20;
/// How many divergences each function keeps.
const KEEP: usize = 3;

#[derive(Default)]
pub struct Compare {
    calls: [Vec<Call>; 2],
    /// The interrupts each CPU is in: where each returns to (PC, stack).
    irqs: [Vec<(u32, u32)>; 2],
    /// An interrupt just taken, until the handler's first instruction: a call starting there is the
    /// handler, which returns by RTE to the interrupted code.
    taken: [Option<(u32, u32)>; 2],
    /// By entry and the native function's name.
    pub verdicts: HashMap<(u32, &'static str), Verdict>,
}

/// The native run of a call against its log (entry, state at the call, the log, its interrupt
/// depth, and its stack frame when it is checked by the calling convention): the CPU's state at
/// its end, or how it diverged.
pub type NativeRun<'a> = dyn FnMut(u32, &Sh2, &[Access], u8, Option<Range<u32>>) -> Result<Sh2, String> + 'a;

/// A native run's divergence from the log (unwinds the run).
struct Diverged(String);

impl Compare {
    /// Whether CPU `c` is in a call being logged.
    pub fn logging(&self, c: usize) -> bool {
        !self.calls[c].is_empty()
    }

    /// The interrupt depth CPU `c` is at.
    pub fn depth(&self, c: usize) -> u8 {
        self.irqs[c].len().min(255) as u8
    }

    /// CPU `c` reached a function the port has: its call is logged from here (`log`: the CPU's
    /// accesses so far).
    pub fn enter(&mut self, c: usize, entry: u32, name: &'static str, contract: Contract, cpu: &Sh2, log: &[Access]) {
        let (ret, sp) = self.taken[c].take().unwrap_or((cpu.pr, cpu.r[15]));
        let relaxed = contract != Contract::Exact;
        if relaxed {
            for k in &mut self.calls[c] {
                k.relaxed = true;
            }
        }
        let kept = if contract == Contract::Exact { Contract::AbiVoid.kept() } else { contract.kept() };
        let call = Call { entry, name, start: log.len(), state: cpu.clone(), ret, sp, depth: self.depth(c), min_sp: cpu.r[15], relaxed, kept };
        self.calls[c].push(call);
    }

    /// CPU `c` took an interrupt that returns to `pc` with the stack at `sp`.
    pub fn interrupted(&mut self, c: usize, pc: u32, sp: u32) {
        if self.irqs[c].len() < 64 {
            self.irqs[c].push((pc, sp));
        }
        self.taken[c] = Some((pc, sp));
    }

    /// After an interpreted step on CPU `c`: handlers returned from, and calls returned from, each
    /// checked natively (`run`: the native run of one call against its log, returning the CPU's
    /// state or the divergence). Returns whether the log can be cleared (no call is being logged).
    pub fn stepped(&mut self, c: usize, cpu: &Sh2, frame: u64, log: &mut Vec<Access>, run: &mut NativeRun) {
        self.taken[c] = None;
        for k in &mut self.calls[c] {
            k.min_sp = k.min_sp.min(cpu.r[15]);
        }
        if cpu.delay.is_some() {
            return;
        }
        while let Some(call) = self.calls[c].last()
            && cpu.pc == call.ret
            && cpu.r[15] == call.sp
            && self.depth(c) == call.depth
        {
            let call = self.calls[c].pop().expect("a call");
            let v = self.verdicts.entry((call.entry, call.name)).or_default();
            v.calls += 1;
            let accesses = &log[call.start..];
            let stack = call.relaxed.then_some(call.min_sp..call.sp);
            let outcome = run(call.entry, &call.state, accesses, call.depth, stack).and_then(|end| differences(&end, cpu, call.relaxed.then_some(call.kept)));
            match outcome {
                Ok(()) => v.equal += 1,
                Err(what) => {
                    v.diverged += 1;
                    if v.first.len() < KEEP {
                        v.first.push((frame, what));
                    }
                }
            }
        }
        // a handler's RTE (after the handler's own call, if it was one)
        while let Some(&(pc, sp)) = self.irqs[c].last()
            && cpu.pc == pc
            && cpu.r[15] == sp
        {
            self.irqs[c].pop();
        }
        if self.calls[c].is_empty() {
            log.clear();
        } else if log.len() > CAP {
            // the outermost calls are let go until the log is short enough
            while log.len() - self.calls[c][0].start > CAP / 2 {
                let call = self.calls[c].remove(0);
                let v = self.verdicts.entry((call.entry, call.name)).or_default();
                v.calls += 1;
                v.too_long += 1;
                if self.calls[c].is_empty() {
                    break;
                }
            }
            let keep = self.calls[c].first().map_or(log.len(), |k| k.start);
            log.drain(..keep);
            for k in &mut self.calls[c] {
                k.start -= keep;
            }
        }
    }

    /// Lets go of CPU `c`'s calls and interrupts (a reset).
    pub fn reset(&mut self, c: usize) {
        self.calls[c].clear();
        self.irqs[c].clear();
        self.taken[c] = None;
    }

    /// The report: one line a function (diverging first), then the divergences.
    pub fn report(&self) -> String {
        let mut v: Vec<(&(u32, &str), &Verdict)> = self.verdicts.iter().collect();
        v.sort_by_key(|(k, v)| (std::cmp::Reverse(v.diverged > 0), std::cmp::Reverse(v.diverged), **k));
        let (calls, equal, diverged) = v.iter().fold((0, 0, 0), |a, (_, v)| (a.0 + v.calls, a.1 + v.equal, a.2 + v.diverged));
        let failing = v.iter().filter(|(_, v)| v.diverged > 0).count();
        let mut out = format!("{} functions called, {failing} diverging; {calls} calls: {equal} equal, {diverged} diverged\n", v.len());
        for ((e, name), v) in &v {
            out += &format!("{e:08x} {name:<12} calls {:>9} equal {:>9} diverged {:>7} too long {:>5}\n", v.calls, v.equal, v.diverged, v.too_long);
        }
        for ((e, _), v) in v.iter().filter(|(_, v)| v.diverged > 0) {
            for (frame, what) in &v.first {
                out += &format!("\n{e:08x} at frame {frame}: {what}\n");
            }
        }
        out
    }
}

/// How the native run's end differs from the interpreter's, if it does (`relaxed`: in the
/// registers the calling convention keeps, bit n rn).
fn differences(native: &Sh2, sh2: &Sh2, relaxed: Option<u16>) -> Result<(), String> {
    let mut d = Vec::new();
    for i in 0..16 {
        let kept = relaxed.is_none_or(|mask| mask >> i & 1 != 0);
        if native.r[i] != sh2.r[i] && kept {
            d.push(format!("r{i} {:08x} (SH-2 {:08x})", native.r[i], sh2.r[i]));
        }
    }
    // the calling convention keeps SR's interrupt mask
    let sr = if relaxed.is_some() { 0xF0 } else { SR_MASK };
    let named = [("pc", native.pc, sh2.pc), ("pr", native.pr, sh2.pr), ("sr", native.sr & sr, sh2.sr & sr), ("gbr", native.gbr, sh2.gbr), ("vbr", native.vbr, sh2.vbr), ("mach", native.mach, sh2.mach), ("macl", native.macl, sh2.macl)];
    for (n, a, b) in named {
        if a != b && !(relaxed.is_some() && matches!(n, "pr" | "mach" | "macl")) {
            d.push(format!("{n} {a:08x} (SH-2 {b:08x})"));
        }
    }
    if d.is_empty() { Ok(()) } else { Err(format!("ends with {}", d.join(", "))) }
}

/// The bus a native run is checked on: reads answered from the call's log, writes checked against
/// it, code fetched from memory (`code`), functions found as the machine finds them.
pub struct Replay<'a> {
    log: &'a [Access],
    depth: u8,
    pos: usize,
    /// Cycles charged, and the most allowed before the run is taken to run on for ever.
    cycles: u64,
    limit: u64,
    code: &'a mut dyn Bus,
    /// The SH-2 instruction the log is at.
    pc: u32,
    relaxed: Option<Relaxed>,
    /// The native run's accesses, when they are kept to look at (`keep_trace`).
    pub trace: Option<Vec<Access>>,
}

/// A call checked by what it does (a rewrite is in it).
struct Relaxed {
    /// Its stack frame, the native run's bytes there, and what was there before the call wrote
    /// (an interrupt handler's frame: the exception's).
    frame: Range<u32>,
    scratch: HashMap<u32, u8>,
    frame_initial: HashMap<u32, u8>,
    /// The log's accesses checked in order: the hardware's, memory that changed under the call.
    ordered: Vec<bool>,
    /// The literals the SH-2 code loaded (address: size, value).
    literals: HashMap<u32, (u8, u32)>,
    /// Memory by byte (canonical addresses): what it held as the SH-2 code first read it, what the
    /// SH-2 code left written, what the native run has written; and the bytes that changed under
    /// the call.
    initial: HashMap<u32, u8>,
    expected: BTreeMap<u32, u8>,
    written: BTreeMap<u32, u8>,
    volatile: HashSet<u32>,
}

/// Memory (no access to it does anything but read or write it): the work RAMs, VDP1's VRAM and
/// framebuffer, VDP2's VRAM and CRAM, sound RAM; a canonical address (the cached and cache-through
/// areas folded; the CPU's own areas above them are not memory).
fn memory(a: u32) -> Option<u32> {
    if a >= 0x4000_0000 {
        return None;
    }
    let a = a & 0x07FF_FFFF;
    let m = matches!(a, 0x0020_0000..=0x002F_FFFF | 0x05A0_0000..=0x05AF_FFFF | 0x05C0_0000..=0x05CB_FFFF | 0x05E0_0000..=0x05F0_0FFF | 0x0600_0000..=0x07FF_FFFF);
    m.then_some(a)
}

/// An access's bytes: (address, value) from the first.
fn bytes(addr: u32, size: u8, value: u32) -> impl Iterator<Item = (u32, u8)> {
    (0..size as u32).map(move |i| (addr.wrapping_add(i), (value >> (8 * (size as u32 - 1 - i))) as u8))
}

impl<'a> Replay<'a> {
    /// The replay of a call from `entry` against its `log` at interrupt depth `depth`, `frame`
    /// when it is checked by what it does; the run may charge `limit` cycles at most.
    pub fn new(log: &'a [Access], depth: u8, code: &'a mut dyn Bus, entry: u32, limit: u64, frame: Option<Range<u32>>) -> Replay<'a> {
        let relaxed = frame.map(|frame| Relaxed::new(log, depth, &mut *code, frame));
        Replay { log, depth, pos: 0, cycles: 0, limit, code, pc: entry, relaxed, trace: None }
    }

    /// Moves to the call's next access (past instructions, handlers' accesses and what is left out).
    fn skip(&mut self) {
        while let Some(e) = self.log.get(self.pos)
            && (e.depth != self.depth || matches!(e.kind, b'P' | b'O') || self.relaxed.as_ref().is_some_and(|r| !r.ordered[self.pos]))
        {
            if e.depth == self.depth {
                self.pc = e.addr;
            }
            self.pos += 1;
        }
    }

    fn next(&mut self, kind: u8, size: u8, addr: u32, value: u32) -> u32 {
        if let Some(t) = &mut self.trace {
            t.push(Access { kind, size, depth: self.depth, addr, value });
        }
        let code = &mut self.code;
        if let Some(r) = &mut self.relaxed
            && let Some(v) = r.access(kind, size, addr, value, &mut |a| code.read8(a))
        {
            return v.unwrap_or_else(|what| std::panic::resume_unwind(Box::new(Diverged(what))));
        }
        self.skip();
        let what = |k: u8, s: u8, a: u32, v: u32| match k {
            b'R' => format!("read{} {a:08x}", s * 8),
            _ => format!("write{} {a:08x} = {v:0w$x}", s * 8, w = s as usize * 2),
        };
        let Some(&e) = self.log.get(self.pos) else {
            std::panic::resume_unwind(Box::new(Diverged(format!("{} after the SH-2 code's last access", what(kind, size, addr, value)))));
        };
        let at = self.pos;
        self.pos += 1;
        if e.kind != kind || e.size != size || e.addr != addr || (kind == b'W' && e.value != value) {
            std::panic::resume_unwind(Box::new(Diverged(format!("access {at}: {} where the SH-2 code made {} at {:08x}", what(kind, size, addr, value), what(e.kind, e.size, e.addr, e.value), self.pc))));
        }
        e.value
    }

    /// Runs `f` from `state` against the log: the CPU's state at its end, or how it diverged.
    pub fn run(&mut self, f: crate::native::NativeFn, state: &Sh2) -> Result<Sh2, String> {
        let mut cpu = state.clone();
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&mut cpu, self)));
        match r {
            Ok(()) => {
                let last = self.pc;
                self.skip();
                match self.log.get(self.pos) {
                    Some(e) => Err(format!("returns (after {last:08x}) with the SH-2 code's access {} to come ({} {:08x} at {:08x})", self.pos, e.kind as char, e.addr, self.pc)),
                    None => match &self.relaxed {
                        Some(r) => r.finished().map(|()| cpu),
                        None => Ok(cpu),
                    },
                }
            }
            Err(p) => Err(match p.downcast::<Diverged>() {
                Ok(d) => d.0,
                Err(p) => format!("panics: {}", p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default()),
            }),
        }
    }
}

impl Relaxed {
    fn new(log: &[Access], depth: u8, code: &mut dyn Bus, frame: Range<u32>) -> Relaxed {
        let mut r = Relaxed {
            frame,
            scratch: HashMap::new(),
            frame_initial: HashMap::new(),
            ordered: vec![false; log.len()],
            literals: HashMap::new(),
            initial: HashMap::new(),
            expected: BTreeMap::new(),
            written: BTreeMap::new(),
            volatile: HashSet::new(),
        };
        // what memory held, as the SH-2 code went: a read finding other than that, the byte
        // changed under the call
        let mut known: HashMap<u32, u8> = HashMap::new();
        let mut frame_written = HashSet::new();
        let mut literal_op = false;
        let mut memory_accesses = Vec::new();
        for (i, e) in log.iter().enumerate() {
            if e.depth != depth {
                continue;
            }
            if e.kind == b'P' {
                // the SH-2 code's loads PC-relative (MOV.W, MOV.L @(disp,PC)) are of its literals
                literal_op = matches!(code.fetch16(e.addr) & 0xF000, 0x9000 | 0xD000);
                continue;
            }
            if e.kind == b'O' {
                // what a write overwrites: what the memory held, if nothing told before
                if !r.frame.contains(&e.addr)
                    && let Some(a) = memory(e.addr)
                {
                    for (b, v) in bytes(a, e.size, e.value) {
                        if let std::collections::hash_map::Entry::Vacant(k) = known.entry(b) {
                            k.insert(v);
                            r.initial.insert(b, v);
                        }
                    }
                }
                continue;
            }
            if r.frame.contains(&e.addr) {
                for (b, v) in bytes(e.addr, e.size, e.value) {
                    if e.kind == b'W' {
                        frame_written.insert(b);
                    } else if !frame_written.contains(&b) {
                        r.frame_initial.entry(b).or_insert(v);
                    }
                }
                continue;
            }
            if e.kind == b'R' && literal_op {
                r.literals.insert(e.addr, (e.size, e.value));
                continue;
            }
            let Some(a) = memory(e.addr) else {
                r.ordered[i] = true;
                continue;
            };
            memory_accesses.push(i);
            for (b, v) in bytes(a, e.size, e.value) {
                if e.kind == b'W' {
                    known.insert(b, v);
                    r.expected.insert(b, v);
                } else if let Some(&k) = known.get(&b) {
                    if k != v {
                        r.volatile.insert(b);
                    }
                } else {
                    known.insert(b, v);
                    r.initial.insert(b, v);
                }
            }
        }
        for i in memory_accesses {
            let e = &log[i];
            let a = memory(e.addr).expect("memory");
            r.ordered[i] = bytes(a, e.size, 0).any(|(b, _)| r.volatile.contains(&b));
        }
        r.expected.retain(|b, _| !r.volatile.contains(b));
        r
    }

    /// A native access answered here (its frame, a literal, memory; `peek` reads what the machine
    /// holds): Some(the value read, or how it diverged); None for one checked in order.
    fn access(&mut self, kind: u8, size: u8, addr: u32, value: u32, peek: &mut dyn FnMut(u32) -> u8) -> Option<Result<u32, String>> {
        if self.frame.contains(&addr) {
            let mut v = 0;
            for (b, w) in bytes(addr, size, value) {
                let s = self.scratch.entry(b).or_insert_with(|| self.frame_initial.get(&b).copied().unwrap_or(0));
                if kind == b'W' {
                    *s = w;
                }
                v = v << 8 | *s as u32;
            }
            return Some(Ok(v));
        }
        if kind == b'R'
            && let Some(&(s, v)) = self.literals.get(&addr)
            && s == size
        {
            return Some(Ok(v));
        }
        let a = memory(addr)?;
        if bytes(a, size, 0).any(|(b, _)| self.volatile.contains(&b)) {
            // checked in order; what it writes to the bytes that did not change under the call
            // is memory all the same
            if kind == b'W' {
                let volatile = &self.volatile;
                self.written.extend(bytes(a, size, value).filter(|(b, _)| !volatile.contains(b)));
            }
            return None;
        }
        if kind == b'W' {
            self.written.extend(bytes(a, size, value));
            return Some(Ok(value));
        }
        let mut v = 0;
        for (b, _) in bytes(a, size, 0) {
            let x = match self.written.get(&b).or_else(|| self.initial.get(&b)) {
                Some(&x) => x,
                // untouched by the SH-2 code: as it was
                None if !self.expected.contains_key(&b) => peek(b),
                None => return Some(Err(format!("read{} {addr:08x}: memory the SH-2 code wrote before reading", size * 8))),
            };
            v = v << 8 | x as u32;
        }
        Some(Ok(v))
    }

    /// At the native run's return: memory is as the SH-2 code left it (each byte either wrote).
    fn finished(&self) -> Result<(), String> {
        let end = |w: &BTreeMap<u32, u8>, b: &u32| w.get(b).or_else(|| self.initial.get(b)).copied();
        for b in self.written.keys().chain(self.expected.keys()) {
            let (native, sh2) = (end(&self.written, b), end(&self.expected, b));
            if native != sh2 {
                let show = |v: Option<u8>| v.map_or("as it was".to_string(), |v| format!("{v:02x}"));
                return Err(format!("leaves memory otherwise: {b:08x} {} (SH-2 {})", show(native), show(sh2)));
            }
        }
        Ok(())
    }
}

impl Bus for Replay<'_> {
    fn read8(&mut self, a: u32) -> u8 {
        self.next(b'R', 1, a, 0) as u8
    }
    fn read16(&mut self, a: u32) -> u16 {
        self.next(b'R', 2, a, 0) as u16
    }
    fn read32(&mut self, a: u32) -> u32 {
        self.next(b'R', 4, a, 0)
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.next(b'W', 1, a, v as u32);
    }
    fn write16(&mut self, a: u32, v: u16) {
        self.next(b'W', 2, a, v as u32);
    }
    fn write32(&mut self, a: u32, v: u32) {
        self.next(b'W', 4, a, v);
    }
    fn fetch16(&mut self, a: u32) -> u16 {
        self.code.fetch16(a)
    }
    fn native_at(&mut self, a: u32) -> Option<crate::native::NativeFn> {
        self.code.native_at(a)
    }
    fn tick(&mut self, cycles: u32) {
        self.cycles += cycles as u64;
        if self.cycles > self.limit {
            std::panic::resume_unwind(Box::new(Diverged(format!("runs on past {} cycles", self.limit))));
        }
    }
}
