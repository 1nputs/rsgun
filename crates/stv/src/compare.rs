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

use crate::sh2::{Bus, Sh2, SR_MASK};
use std::collections::HashMap;

/// One access the CPU made: b'R' or b'W', its size in bytes, the interrupt depth it was made at;
/// or b'P', an instruction's address (where the accesses after it were made).
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
    /// Where its accesses start in the CPU's log.
    start: usize,
    state: Sh2,
    ret: u32,
    sp: u32,
    depth: u8,
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
    pub verdicts: HashMap<u32, Verdict>,
}

/// The native run of a call against its log (entry, state at the call, the log, its interrupt
/// depth): the CPU's state at its end, or how it diverged.
pub type NativeRun<'a> = dyn FnMut(u32, &Sh2, &[Access], u8) -> Result<Sh2, String> + 'a;

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
    pub fn enter(&mut self, c: usize, entry: u32, cpu: &Sh2, log: &[Access]) {
        let (ret, sp) = self.taken[c].take().unwrap_or((cpu.pr, cpu.r[15]));
        self.calls[c].push(Call { entry, start: log.len(), state: cpu.clone(), ret, sp, depth: self.depth(c) });
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
        if cpu.delay.is_some() {
            return;
        }
        while let Some(call) = self.calls[c].last()
            && cpu.pc == call.ret
            && cpu.r[15] == call.sp
            && self.depth(c) == call.depth
        {
            let call = self.calls[c].pop().expect("a call");
            let v = self.verdicts.entry(call.entry).or_default();
            v.calls += 1;
            let accesses = &log[call.start..];
            let outcome = run(call.entry, &call.state, accesses, call.depth).and_then(|end| differences(&end, cpu));
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
                let v = self.verdicts.entry(call.entry).or_default();
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
    pub fn report(&self, name: &dyn Fn(u32) -> &'static str) -> String {
        let mut v: Vec<(&u32, &Verdict)> = self.verdicts.iter().collect();
        v.sort_by_key(|(e, v)| (std::cmp::Reverse(v.diverged > 0), std::cmp::Reverse(v.diverged), **e));
        let (calls, equal, diverged) = v.iter().fold((0, 0, 0), |a, (_, v)| (a.0 + v.calls, a.1 + v.equal, a.2 + v.diverged));
        let failing = v.iter().filter(|(_, v)| v.diverged > 0).count();
        let mut out = format!("{} functions called, {failing} diverging; {calls} calls: {equal} equal, {diverged} diverged\n", v.len());
        for (e, v) in &v {
            out += &format!("{e:08x} {:<12} calls {:>9} equal {:>9} diverged {:>7} too long {:>5}\n", name(**e), v.calls, v.equal, v.diverged, v.too_long);
        }
        for (e, v) in v.iter().filter(|(_, v)| v.diverged > 0) {
            for (frame, what) in &v.first {
                out += &format!("\n{e:08x} at frame {frame}: {what}\n");
            }
        }
        out
    }
}

/// How the native run's end differs from the interpreter's, if it does.
fn differences(native: &Sh2, sh2: &Sh2) -> Result<(), String> {
    let mut d = Vec::new();
    for i in 0..16 {
        if native.r[i] != sh2.r[i] {
            d.push(format!("r{i} {:08x} (SH-2 {:08x})", native.r[i], sh2.r[i]));
        }
    }
    let named = [("pc", native.pc, sh2.pc), ("pr", native.pr, sh2.pr), ("sr", native.sr & SR_MASK, sh2.sr & SR_MASK), ("gbr", native.gbr, sh2.gbr), ("vbr", native.vbr, sh2.vbr), ("mach", native.mach, sh2.mach), ("macl", native.macl, sh2.macl)];
    for (n, a, b) in named {
        if a != b {
            d.push(format!("{n} {a:08x} (SH-2 {b:08x})"));
        }
    }
    if d.is_empty() { Ok(()) } else { Err(format!("ends with {}", d.join(", "))) }
}

/// The bus a native run is checked on: reads answered from the call's log, writes checked against
/// it, code fetched from memory (`code`), functions found as the machine finds them.
pub struct Replay<'a> {
    pub log: &'a [Access],
    pub depth: u8,
    pub pos: usize,
    /// Cycles charged, and the most allowed before the run is taken to run on for ever.
    pub cycles: u64,
    pub limit: u64,
    pub code: &'a mut dyn Bus,
    /// The SH-2 instruction the log is at.
    pub pc: u32,
}

impl Replay<'_> {
    /// Moves to the call's next access (past instructions and handlers' accesses).
    fn skip(&mut self) {
        while let Some(e) = self.log.get(self.pos)
            && (e.depth != self.depth || e.kind == b'P')
        {
            if e.depth == self.depth {
                self.pc = e.addr;
            }
            self.pos += 1;
        }
    }

    fn next(&mut self, kind: u8, size: u8, addr: u32, value: u32) -> u32 {
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
                    None => Ok(cpu),
                }
            }
            Err(p) => Err(match p.downcast::<Diverged>() {
                Ok(d) => d.0,
                Err(p) => format!("panics: {}", p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default()),
            }),
        }
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
