//! Checks the runtime against the oracle's records.
//!
//!     stv-check trace DIR [--cpu m|s] [--rom build/rom]
//!
//! `trace`: replays an oracle trace (DIR = out/oracle/.../tFFFFFF, made by tools/oracle/run.py --trace)
//! through the SH-2 core. The core starts from the registers in DIR/start, fetches instructions from
//! the memory there (and the BIOS and cart images), and has every data read answered by the next
//! record; it must step through the same addresses, make the same reads and writes in the same
//! order, and take the same interrupts. The first difference stops it, with the records around it.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use stv::sh2::{Bus, Sh2};

const EXCEPTION_INT: u32 = 9;

/// Mednafen's pipeline pseudo-ops that run no instruction: standby, and a stall for a DMA burst or an
/// external halt. (0x7F/0xFF, an exception pending, either takes one or runs the instruction.)
const OP_STANDBY: u8 = 0x7E;
const OP_STALL: u8 = 0xFE;

#[derive(Clone, Copy, Debug)]
struct Rec {
    tag: u8,
    size: u8,
    a: u32,
    v: u32,
}

fn load_trace(path: &Path) -> Result<Vec<Rec>, String> {
    let data = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let recs: Vec<Rec> = data
        .as_chunks::<12>()
        .0
        .iter()
        .map(|c| Rec {
            tag: c[0],
            size: c[1],
            a: u32::from_be_bytes([c[4], c[5], c[6], c[7]]),
            v: u32::from_be_bytes([c[8], c[9], c[10], c[11]]),
        })
        .collect();
    match recs.first() {
        Some(h) if h.tag == b'H' && h.a == 0x5354_5654 && h.size == 3 => Ok(recs),
        Some(h) if h.tag == b'H' => Err(format!("{}: trace version {}, this reads 3", path.display(), h.size)),
        _ => Err(format!("{}: not an oracle trace", path.display())),
    }
}

/// What the CPU fetches instructions from: the dump's work RAM, the BIOS and the cart.
struct Memory {
    bios: Vec<u8>,
    wram_l: Vec<u8>,
    wram_h: Vec<u8>,
    cart: Vec<u8>,
}

impl Memory {
    fn slot(&mut self, addr: u32) -> Option<&mut [u8]> {
        let a = (addr & 0x07FF_FFFF) as usize;
        match a {
            0x0000_0000..=0x000F_FFFF => Some(&mut self.bios[a & 0x7FFFF..]),
            0x0020_0000..=0x002F_FFFF => Some(&mut self.wram_l[a & 0xFFFFF..]),
            0x0200_0000..=0x04FF_FFFF => self.cart.get_mut(a - 0x0200_0000..),
            0x0600_0000..=0x07FF_FFFF => Some(&mut self.wram_h[a & 0xFFFFF..]),
            _ => None,
        }
    }

    fn fetch16(&mut self, addr: u32) -> Option<u16> {
        if addr >> 29 > 1 {
            return None;
        }
        self.slot(addr).and_then(|s| s.get(..2)).map(|b| u16::from_be_bytes([b[0], b[1]]))
    }

    /// Applies the CPU's own write, so code it writes and then runs is fetched as written.
    fn store(&mut self, addr: u32, size: u32, value: u32) {
        if addr >> 29 > 1 {
            return;
        }
        if let Some(s) = self.slot(addr) {
            for i in 0..size as usize {
                if let Some(b) = s.get_mut(i) {
                    *b = (value >> (8 * (size as usize - 1 - i))) as u8;
                }
            }
        }
    }
}

/// The CPU's bus during a replay: data accesses are checked against, and answered by, the records.
struct Replay {
    recs: Vec<Rec>,
    pos: usize,
    mem: Memory,
    error: Option<String>,
    fetch_error: Option<u32>,
    /// The next fetch's word, when the replay starts in a delay slot whose address it does not know.
    fetch_override: Option<u16>,
}

impl Replay {
    fn next_access(&mut self, kind: u8, size: u32, addr: u32, value: u32) -> u32 {
        // An exception the instruction raised itself (TRAPA, an illegal instruction) is noted in the
        // stream before its stack accesses: step over it.
        while self.pos < self.recs.len() && self.recs[self.pos].tag == b'X' && self.recs[self.pos].v != EXCEPTION_INT {
            self.pos += 1;
        }
        let r = self.recs.get(self.pos).copied();
        let got = match r {
            Some(r) if r.tag == kind && r.size as u32 == size && r.a == addr && (kind == b'R' || r.v == value) => {
                self.pos += 1;
                return r.v;
            }
            Some(r) => format!("{}{} [{:08x}] {:x}", r.tag as char, r.size, r.a, r.v),
            None => "the end of the trace".into(),
        };
        if self.error.is_none() {
            let want = if kind == b'R' { format!("R{size} [{addr:08x}]") } else { format!("W{size} [{addr:08x}] {value:x}") };
            self.error = Some(format!("the core made {want}; the oracle made {got}"));
        }
        r.map(|r| r.v).unwrap_or(0)
    }
}

impl Bus for Replay {
    fn read8(&mut self, a: u32) -> u8 {
        self.next_access(b'R', 1, a, 0) as u8
    }
    fn read16(&mut self, a: u32) -> u16 {
        self.next_access(b'R', 2, a, 0) as u16
    }
    fn read32(&mut self, a: u32) -> u32 {
        self.next_access(b'R', 4, a, 0)
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.next_access(b'W', 1, a, v as u32);
        self.mem.store(a, 1, v as u32);
    }
    fn write16(&mut self, a: u32, v: u16) {
        self.next_access(b'W', 2, a, v as u32);
        self.mem.store(a, 2, v as u32);
    }
    fn write32(&mut self, a: u32, v: u32) {
        self.next_access(b'W', 4, a, v);
        self.mem.store(a, 4, v);
    }
    fn fetch16(&mut self, a: u32) -> u16 {
        if let Some(w) = self.fetch_override.take() {
            return w;
        }
        self.mem.fetch16(a).unwrap_or_else(|| {
            self.fetch_error.get_or_insert(a);
            0x0009
        })
    }
}

fn read_regs(path: &Path, cpu: usize) -> Result<Sh2, String> {
    let data = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let w: Vec<u32> = data[cpu * 92..cpu * 92 + 92].as_chunks::<4>().0.iter().map(|c| u32::from_be_bytes(*c)).collect();
    let mut s = Sh2::default();
    s.r.copy_from_slice(&w[..16]);
    // The oracle's PC reads as the next instruction + 4, as the manual's PC does.
    s.pc = w[16].wrapping_sub(4);
    s.pr = w[17];
    s.sr = w[18];
    s.gbr = w[19];
    s.vbr = w[20];
    s.mach = w[21];
    s.macl = w[22];
    Ok(s)
}

fn show_regs(c: &Sh2) -> String {
    let mut s = format!("  pc={:08x} pr={:08x} sr={:03x} gbr={:08x} vbr={:08x} mach={:08x} macl={:08x}{}\n ",
        c.pc, c.pr, c.sr, c.gbr, c.vbr, c.mach, c.macl, if c.delay.is_some() { " (slot next)" } else { "" });
    for (i, r) in c.r.iter().enumerate() {
        s += &format!(" r{i}={r:08x}");
        if i == 7 {
            s += "\n ";
        }
    }
    s
}

fn show_recs(recs: &[Rec], from: usize, to: usize, mark: usize) -> String {
    let mut s = String::new();
    for (k, r) in recs.iter().enumerate().take(to.min(recs.len())).skip(from) {
        let m = if k == mark { ">>" } else { "  " };
        s += &match r.tag {
            b'P' => format!("{m} {k:8} P {:08x}  op {:02x} {:04x}\n", r.a, r.size, r.v),
            b'R' | b'W' => format!("{m} {k:8}     {}{} [{:08x}] {:x}\n", r.tag as char, r.size, r.a, r.v),
            b'X' => format!("{m} {k:8} X vector {:#x} saved pc {:08x} (exception {})\n", r.size, r.a, r.v),
            b'I' => format!("{m} {k:8} I level {}\n", r.size),
            t => format!("{m} {k:8} {} {:08x} {:08x}\n", t as char, r.a, r.v),
        };
    }
    s
}

struct Outcome {
    steps: u64,
    code_fixes: u64,
    interrupts: u64,
    frames: u64,
    error: Option<String>,
    cpu: Sh2,
}

fn replay(recs: Vec<Rec>, mem: Memory, mut cpu: Sh2) -> Outcome {
    let mut bus = Replay { recs, pos: 1, mem, error: None, fetch_error: None, fetch_override: None };
    // Where the CPU is: the oracle's PC is the next instruction + 4, except between a delayed branch
    // and its slot, where it is the target + 2 (the first instruction is then a slot: op bit 7).
    if let Some(first) = bus.recs.iter().find(|r| r.tag == b'P' && r.size != OP_STALL && r.size != OP_STANDBY)
        && first.size & 0x80 != 0
        && (first.size & 0x7F) != 0x7F
    {
        let target = cpu.pc.wrapping_add(4).wrapping_sub(2);
        cpu.delay = Some(stv::sh2::Delay { target, branch: 0 });
        // The slot's own address is not in the registers; PR - 2 after a jsr or bsr, else unknown.
        let slot = cpu.pr.wrapping_sub(2);
        if bus.mem.fetch16(slot) == Some(first.v as u16) {
            cpu.pc = slot;
        } else {
            bus.fetch_override = Some(first.v as u16);
        }
    }
    // The debugger's PC lags for the first steps of a window: trust it once it has agreed with the
    // core's; until then the instruction words alone must agree.
    let mut synced = false;
    let (mut steps, mut interrupts, mut frames, mut code_fixes) = (0u64, 0u64, 0u64, 0u64);
    let fail = |bus: &Replay, cpu: &Sh2, at: usize, what: String| -> Option<String> {
        Some(format!("{what}\nrecords:\n{}core:\n{}", show_recs(&bus.recs, at.saturating_sub(16), at + 8, at), show_regs(cpu)))
    };
    while bus.pos < bus.recs.len() {
        let at = bus.pos;
        let r = bus.recs[at];
        match r.tag {
            b'F' => {
                frames += 1;
                bus.pos += 1;
            }
            b'P' => {
                if cpu.sleeping || r.size == OP_STANDBY || r.size == OP_STALL {
                    bus.pos += 1;
                    continue;
                }
                if synced && r.a != 0 && r.a != cpu.pc {
                    let e = fail(&bus, &cpu, at, format!("step {steps}: the oracle is at {:08x}, the core at {:08x}", r.a, cpu.pc));
                    return Outcome { steps, interrupts, frames, code_fixes, error: e, cpu };
                }
                synced |= r.a == cpu.pc && bus.fetch_override.is_none();
                bus.pos += 1;
                // An interrupt accepted before this instruction runs.
                if matches!(bus.recs.get(bus.pos), Some(x) if x.tag == b'X' && (x.v == EXCEPTION_INT || x.v <= 1)) {
                    continue;
                }
                // An exception-pending step that began a DMA burst stall runs nothing; the instruction
                // runs on the step after the stall.
                if (r.size & 0x7F) == 0x7F && matches!(bus.recs.get(bus.pos), Some(x) if x.tag == b'P' && x.size == OP_STALL) {
                    continue;
                }
                // The oracle's instruction word: where it differs from the memory the replay fetches
                // from (code a DMA wrote during the trace), the oracle's is what ran.
                if bus.fetch_override.is_none() && bus.mem.fetch16(cpu.pc) != Some(r.v as u16) {
                    if !synced {
                        let held = bus.mem.fetch16(cpu.pc);
                        let e = fail(&bus, &cpu, at, format!("step {steps}: the oracle runs {:04x}; the core is at {:08x}, which holds {held:04x?}",
                            r.v, cpu.pc));
                        return Outcome { steps, interrupts, frames, code_fixes, error: e, cpu };
                    }
                    bus.mem.store(cpu.pc, 2, r.v);
                    code_fixes += 1;
                }
                cpu.step(&mut bus);
                steps += 1;
                if let Some(a) = bus.fetch_error {
                    let e = fail(&bus, &cpu, at, format!("step {steps}: fetch from {a:08x}, outside the memory the replay has"));
                    return Outcome { steps, interrupts, frames, code_fixes, error: e, cpu };
                }
                if let Some(e) = bus.error.take() {
                    let e = fail(&bus, &cpu, at, format!("step {steps} (the instruction at {:08x}): {e}", r.a));
                    return Outcome { steps, interrupts, frames, code_fixes, error: e, cpu };
                }
                if let Some(n) = bus.recs.get(bus.pos)
                    && matches!(n.tag, b'R' | b'W')
                {
                    let e = fail(&bus, &cpu, bus.pos, format!("step {steps} (the instruction at {:08x}): the oracle made an access the core did not", r.a));
                    return Outcome { steps, interrupts, frames, code_fixes, error: e, cpu };
                }
            }
            b'X' if r.v <= 1 => {
                bus.pos += 1;
                cpu.reset(&mut bus, r.size as u32);
                synced = false;
                if let Some(e) = bus.error.take() {
                    let e = fail(&bus, &cpu, at, format!("reset: {e}"));
                    return Outcome { steps, interrupts, frames, code_fixes, error: e, cpu };
                }
            }
            b'X' if r.v == EXCEPTION_INT => {
                if r.a != cpu.pc || cpu.delay.is_some() {
                    let e = fail(&bus, &cpu, at, format!("interrupt {:#x}: the oracle saved pc {:08x}, the core is at {:08x}", r.size, r.a, cpu.pc));
                    return Outcome { steps, interrupts, frames, code_fixes, error: e, cpu };
                }
                let level = bus.recs[at..].iter().take(8).find(|x| x.tag == b'I').map(|x| x.size as u32);
                let Some(level) = level else {
                    let e = fail(&bus, &cpu, at, "an interrupt with no level record".into());
                    return Outcome { steps, interrupts, frames, code_fixes, error: e, cpu };
                };
                bus.pos += 1;
                cpu.interrupt(&mut bus, r.size as u32, level);
                interrupts += 1;
                if let Some(e) = bus.error.take() {
                    let e = fail(&bus, &cpu, at, format!("interrupt {:#x}: {e}", r.size));
                    return Outcome { steps, interrupts, frames, code_fixes, error: e, cpu };
                }
                match bus.recs.get(bus.pos) {
                    Some(i) if i.tag == b'I' => bus.pos += 1,
                    _ => {
                        let e = fail(&bus, &cpu, bus.pos, "expected the interrupt's level record".into());
                        return Outcome { steps, interrupts, frames, code_fixes, error: e, cpu };
                    }
                }
            }
            _ => {
                let e = fail(&bus, &cpu, at, format!("step {steps}: an unexpected record"));
                return Outcome { steps, interrupts, frames, code_fixes, error: e, cpu };
            }
        }
    }
    Outcome { steps, interrupts, frames, code_fixes, error: None, cpu }
}

fn trace(dir: &Path, cpus: &[usize], rom: &Path) -> Result<bool, String> {
    let start = dir.join("start");
    let read = |p: PathBuf| fs::read(&p).map_err(|e| format!("{}: {e}", p.display()));
    let mut ok = true;
    for &c in cpus {
        let mem = Memory {
            bios: read(rom.join("bios-epr20091.bin"))?,
            wram_l: read(start.join("wram-l.bin"))?,
            wram_h: read(start.join("wram-h.bin"))?,
            cart: read(rom.join("rsgun-cart.bin"))?,
        };
        let name = if c == 0 { "master" } else { "slave" };
        let recs = load_trace(&dir.join(if c == 0 { "m.trace" } else { "s.trace" }))?;
        let n = recs.len();
        let cpu = read_regs(&start.join("regs.bin"), c)?;
        let out = replay(recs, mem, cpu);
        match &out.error {
            None => {
                println!("{name}: {} steps, {} interrupts, {} frames: all {n} records match ({} instruction words taken from the trace)",
                    out.steps, out.interrupts, out.frames, out.code_fixes);
                // The dump after the last traced frame, if there is one, holds the registers to end on.
                if let Some(end) = end_dump(dir, out.frames) {
                    let want = read_regs(&end.join("regs.bin"), c)?;
                    // The oracle's PC is the next instruction + 4, but between a delayed branch and
                    // its slot it is the branch target + 2.
                    let oracle_pc = |c: &Sh2| match c.delay {
                        Some(d) => d.target.wrapping_add(2),
                        None => c.pc.wrapping_add(4),
                    };
                    let regs = |c: &Sh2, pc: u32| (c.r, pc, c.pr, c.sr, c.gbr, c.vbr, c.mach, c.macl);
                    if regs(&want, want.pc.wrapping_add(4)) != regs(&out.cpu, oracle_pc(&out.cpu)) {
                        println!("{name}: but the registers differ from {}:\n oracle\n{}\n core\n{}",
                            end.display(), show_regs(&want), show_regs(&out.cpu));
                        ok = false;
                    } else {
                        println!("{name}: registers equal to {}", end.display());
                    }
                }
            }
            Some(e) => {
                println!("{name}: {} steps and {} interrupts matched, then:\n{e}", out.steps, out.interrupts);
                ok = false;
            }
        }
    }
    Ok(ok)
}

/// The oracle dump taken at the end of the trace (DIR/../fFFFFFF for the trace's last frame).
fn end_dump(dir: &Path, frames: u64) -> Option<PathBuf> {
    let first: u64 = dir.file_name()?.to_str()?.strip_prefix('t')?.parse().ok()?;
    let p = dir.parent()?.join(format!("f{:06}", first + frames - 1));
    p.join("regs.bin").exists().then_some(p)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let usage = "usage: stv-check trace DIR [--cpu m|s] [--rom DIR]";
    let Some(cmd) = args.first() else {
        eprintln!("{usage}");
        return ExitCode::from(2);
    };
    let mut cpus = vec![0, 1];
    let mut rom = PathBuf::from("build/rom");
    let mut dir = None;
    let mut it = args[1..].iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--cpu" => cpus = match it.next().map(String::as_str) { Some("m") => vec![0], Some("s") => vec![1], _ => vec![0, 1] },
            "--rom" => rom = it.next().map(PathBuf::from).unwrap_or(rom),
            _ => dir = Some(PathBuf::from(a)),
        }
    }
    let result = match (cmd.as_str(), dir) {
        ("trace", Some(d)) => trace(&d, &cpus, &rom),
        _ => Err(usage.into()),
    };
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("stv-check: {e}");
            ExitCode::from(2)
        }
    }
}
