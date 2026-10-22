//! The game, rewritten in Rust. Each function here replaces one of the game's SH-2 functions (as
//! the translation in build/port does, but written to be read: named, typed, its state named),
//! keeping its calling convention: arguments in r4-r7, the result in r0, what a function keeps
//! for its caller kept. Compare mode checks each against the SH-2 code by that convention
//! (stv::native::Contract::Abi; tools/port/check.sh). Ghidra's C for the function
//! (tools/ghidra/project.py) and the translation are where a rewrite starts from.
//!
//! A rewrite takes a [`Cpu`]: the bus the game's memory and hardware are on, and the registers it
//! needs to call code not rewritten yet ([`Cpu::call`]) and to wait ([`Cpu::wait`]).

mod actors;
mod aim;
mod backup;
mod banner;
mod bg;
mod bullets;
mod camera;
mod clib;
mod director;
mod dma;
mod effects;
mod explosions;
mod files;
mod globals;
mod gouraud;
mod ground;
mod hits;
mod hud;
mod kinds;
mod logos;
mod main_loop;
mod modes;
mod math;
mod models;
mod objects;
mod opening;
mod orbits;
mod pads;
mod player;
mod play;
mod palette;
mod parts;
mod ribbons;
mod results;
mod rings;
mod screen;
mod shots;
mod sgl;
mod sound;
mod sprites;
mod start;
mod states;
mod stg00;
mod stg01;
mod stg02;
mod stg04;
mod stg05;
mod system;
mod text;
mod timeline;
mod vblank;

use stv::native::{Contract, Natives};
use stv::sh2::{Bus, Sh2};

/// The CPU a rewritten function runs on.
pub struct Cpu<'a> {
    pub sh2: &'a mut Sh2,
    pub bus: &'a mut dyn Bus,
    /// The r14 the rewrite was called with (the registration sets r14 to the rewrite's frame;
    /// SGL's assembly that pushes r14 pushes this).
    pub caller_r14: u32,
    /// The r14 the rewrite leaves (`leave_r14`: SGL's slave handlers use it without keeping it),
    /// else the caller's again.
    r14_out: Option<u32>,
}

impl Cpu<'_> {
    pub fn read8(&mut self, a: u32) -> u8 {
        self.bus.read8(a)
    }
    pub fn read16(&mut self, a: u32) -> u16 {
        self.bus.read16(a)
    }
    pub fn read32(&mut self, a: u32) -> u32 {
        self.bus.read32(a)
    }
    pub fn write8(&mut self, a: u32, v: u8) {
        self.bus.write8(a, v)
    }
    pub fn write16(&mut self, a: u32, v: u16) {
        self.bus.write16(a, v)
    }
    pub fn write32(&mut self, a: u32, v: u32) {
        self.bus.write32(a, v)
    }

    /// Calls the game's function at `f` (rewritten, translated or interpreted) with `args` in
    /// r4-r7: its result (r0).
    pub fn call(&mut self, f: u32, args: &[u32]) -> u32 {
        for (r, &a) in self.sh2.r[4..8].iter_mut().zip(args) {
            *r = a;
        }
        let pr = self.sh2.pr;
        self.sh2.pr = RETURN;
        stv::native::call(self.sh2, self.bus, f);
        self.sh2.pr = pr;
        self.sh2.r[0]
    }

    /// `call` with PR `pr`, the SH-2 call's return address, instead of RETURN, so the callee's
    /// pushes of it leave what the SH-2 code's do (the game reads stale stack; the interpreter
    /// knows the return by PR and the stack pointer both). Used where such a read was seen.
    pub fn call_returning(&mut self, pr: u32, f: u32, args: &[u32]) -> u32 {
        for (r, &a) in self.sh2.r[4..8].iter_mut().zip(args) {
            *r = a;
        }
        let kept = self.sh2.pr;
        self.sh2.pr = pr;
        stv::native::call(self.sh2, self.bus, f);
        self.sh2.pr = kept;
        self.sh2.r[0]
    }

    /// `call` with arguments past the fourth on the stack (the fifth at the stack pointer, as the
    /// SH-2 code pushes them just before the call).
    #[allow(dead_code)]
    pub fn call_with_stack(&mut self, f: u32, args: &[u32], stack_args: &[u32]) -> u32 {
        let sp = self.sh2.r[15];
        let top = sp.wrapping_sub(4 * stack_args.len() as u32);
        for (i, &a) in stack_args.iter().enumerate().rev() {
            self.bus.write32(top + 4 * i as u32, a);
        }
        self.sh2.r[15] = top;
        let r = self.call(f, args);
        self.sh2.r[15] = sp;
        r
    }

    /// The stack the function's callees get: the bottom of its SH-2 function's frame (its
    /// prologue's, see [`Prologue`]), where a struct it hands a callee by address goes.
    pub fn stack(&self) -> u32 {
        self.sh2.r[15]
    }

    /// Runs `f` with registers (r8-r13) holding values (`(n, value)`), as the SH-2 code holds them
    /// across a call: the callees' prologues push them, and the game reads the stack they leave
    /// (a struct field never set). Used where such a read was seen.
    pub fn holding<R>(&mut self, regs: &[(usize, u32)], f: impl FnOnce(&mut Cpu) -> R) -> R {
        let kept = self.sh2.r;
        for &(n, value) in regs {
            self.sh2.r[n] = value;
        }
        let r = f(self);
        for &(n, _) in regs {
            self.sh2.r[n] = kept[n];
        }
        r
    }

    /// Pushes `values` (the first at the new stack pointer), as the SH-2 code pushes a call's
    /// arguments past the fourth where it leaves them until a later [`Cpu::pop`] (or its return):
    /// the next call's go below them.
    pub fn push(&mut self, values: &[u32]) {
        let top = self.sh2.r[15].wrapping_sub(4 * values.len() as u32);
        for (i, &v) in values.iter().enumerate() {
            self.bus.write32(top.wrapping_add(4 * i as u32), v);
        }
        self.sh2.r[15] = top;
    }

    /// Register `n`'s value (r0-r15), and PR's: for SH-2 code that pushes them where the rewrite
    /// does not otherwise know them.
    pub fn reg(&self, n: usize) -> u32 {
        self.sh2.r[n]
    }

    pub fn pr(&self) -> u32 {
        self.sh2.pr
    }

    /// GBR: SGL's work area, which its library code addresses from it.
    pub fn gbr(&self) -> u32 {
        self.sh2.gbr
    }

    /// Registers left as SGL's code leaves them (`(n, value)`, r1-r7): its callers' code passes
    /// what is left in them on, and some of SGL's own code reads them before it sets them.
    pub fn leave(&mut self, regs: &[(usize, u32)]) {
        for &(n, v) in regs {
            self.sh2.r[n] = v;
        }
    }

    /// MACH and MACL left as SGL's code leaves them (the 64-bit sum).
    pub fn leave_mac(&mut self, sum: i64) {
        self.sh2.mach = (sum >> 32) as u32;
        self.sh2.macl = sum as u32;
    }

    /// r14 left as SGL's code leaves it (a top-level rewrite's; the registration gives the caller's
    /// back otherwise).
    pub fn leave_r14(&mut self, v: u32) {
        self.r14_out = Some(v);
    }

    /// SR's T bit left as SGL's code leaves it.
    pub fn leave_t(&mut self, t: bool) {
        self.sh2.sr = (self.sh2.sr & !1) | t as u32;
    }

    /// Takes `bytes` of pushed arguments off the stack.
    pub fn pop(&mut self, bytes: u32) {
        self.sh2.r[15] = self.sh2.r[15].wrapping_add(bytes);
    }

    /// `nested` for a callee that takes arguments past the fourth: `stack_args` pushed first (the
    /// fifth at the stack pointer), as the SH-2 code pushes them before the call.
    pub fn nested_with_stack<R>(&mut self, prologue: &std::sync::OnceLock<Prologue>, entry: u32, stack_args: &[u32], f: impl FnOnce(&mut Cpu) -> R) -> R {
        let sp = self.sh2.r[15];
        let top = sp.wrapping_sub(4 * stack_args.len() as u32);
        for (i, &a) in stack_args.iter().enumerate() {
            self.bus.write32(top.wrapping_add(4 * i as u32), a);
        }
        self.sh2.r[15] = top;
        let r = self.nested(prologue, entry, f);
        self.sh2.r[15] = sp;
        r
    }

    /// Runs `f` (a rewrite called by another) as the SH-2 function at `entry` runs when called:
    /// its prologue's pushes made and its frame below them, as [`Prologue`] reads them from its
    /// code (through `prologue`, a cache for the call site; see [`nested!`]), r14 its frame
    /// pointer meanwhile.
    pub fn nested<R>(&mut self, prologue: &std::sync::OnceLock<Prologue>, entry: u32, f: impl FnOnce(&mut Cpu) -> R) -> R {
        let p = prologue.get_or_init(|| Prologue::read(&mut *self.bus, entry));
        let (sp, pr, fp) = (self.sh2.r[15], self.sh2.pr, self.sh2.r[14]);
        self.sh2.pr = RETURN;
        self.sh2.r[15] = p.enter(self.sh2, &mut *self.bus);
        self.sh2.r[14] = self.sh2.r[15];
        self.sh2.pr = pr;
        let r = f(self);
        self.sh2.r[15] = sp;
        self.sh2.r[14] = fp;
        r
    }

    /// One pass of a loop waiting for the hardware at `at` (the SH-2 code's loop, for the machine's
    /// watch on waits): the machine runs on as the SH-2 code's pass would let it.
    pub fn wait(&mut self, at: u32, cycles: u32) {
        stv::native::tick(self.sh2, self.bus, at, cycles);
    }

    /// The SH-2 code's time for its block at `at` (`cycles`: an instruction a cycle, as the
    /// translations count it), where other hardware relies on how long the code takes: the
    /// machine runs on as the SH-2 code would let it.
    pub fn spend(&mut self, at: u32, cycles: u32) {
        stv::native::tick(self.sh2, self.bus, at, cycles);
    }
}

/// The return address a call from rewritten code gives (no code is there: odd).
const RETURN: u32 = 0xFFFF_FFF1;

/// A function's prologue as its SH-2 code has it: the registers it pushes, in order (PR_PUSH for
/// the procedure register), and the room it makes below them for its locals. A rewrite makes the
/// same pushes, with the registers it is called with, and takes the same frame, r14 its frame
/// pointer while it runs (the game's code always keeps its frame pointer there, so the r14 its
/// callees push is the SH-2 code's): the stack then holds what the SH-2 code's would (the game reads stale stack: a struct field it left unset gets
/// what a call before left there), and a struct a rewrite hands a callee on the stack is where the
/// SH-2 code's was.
pub struct Prologue {
    pushes: Vec<u8>,
    locals: u32,
}

const PR_PUSH: u8 = 16;
/// The control and MAC registers a prologue pushes (interrupt handlers' `stc.l GBR` and
/// `sts.l MACH`/`MACL`, `stc.l SR`): only in PROLOGUES.
const GBR_PUSH: u8 = 17;
const MACH_PUSH: u8 = 18;
const MACL_PUSH: u8 = 19;
const SR_PUSH: u8 = 20;

/// Prologues the reader cannot take from the code: SGL's on-screen check pushes r0 for a while
/// after its two pushes, before its first branch (it is popped again: not part of its frame);
/// slPutPolygonX pushes PR first, then r9 and r8; the rotation parameters' push r8 first, then PR
/// and the others (and GBR, which the rewrite writes itself); slScrAutoDisp pushes PR first, then
/// the others, and takes its frame by a subtraction; SGL's interrupt handlers save what they use,
/// GBR, MACH, MACL and SR among it; SGL's routines run block by block (asm2sm.py) make their own
/// pushes.
const PROLOGUES: [(u32, &[u8], u32); 13] = [
    (0x0606_e9fc, &[6, 7], 0),
    (0x0607_10b0, &[PR_PUSH, 9, 8], 0),
    (0x0606_e50c, &[8, PR_PUSH, 14, 13, 12, 11, 10, 9], 0),
    (0x0606_dd6c, &[PR_PUSH, 13, 12, 11, 10, 9, 8], 0xa4),
    (0x0603_32a0, &[0, GBR_PUSH], 0),
    (0x0603_32b2, &[0, GBR_PUSH], 0),
    (0x0603_32c4, &[0, GBR_PUSH], 0),
    (0x0603_4c1a, &[0, 4, 3, 2, 1, PR_PUSH, MACH_PUSH, MACL_PUSH], 0),
    (0x0603_32d6, &[0, 7, 6, 5, 4, 3, 2, PR_PUSH, 1, GBR_PUSH, MACH_PUSH, MACL_PUSH, SR_PUSH], 0),
    (0x0603_3430, &[0, 1, GBR_PUSH, PR_PUSH, 7, 6, 5, 4, 3, 2, MACH_PUSH, MACL_PUSH, SR_PUSH], 0),
    (0x0603_3658, &[], 0),
    (0x0603_3884, &[], 0),
    (0x0603_4cb0, &[PR_PUSH], 0),
];

impl Prologue {
    /// Read from the code at `entry` (but PROLOGUES), up to its first branch: the pushes (`mov.l Rm,@-R15`, then
    /// `sts.l PR,@-R15`, after which a push is an argument's, and `stc.l GBR,@-R15`), and the locals (`add #imm,R15`, or
    /// `mov.w @(disp,PC),Rn` then `add Rn,R15`). Other instructions may be scheduled among them.
    pub fn read(bus: &mut dyn Bus, entry: u32) -> Prologue {
        if let Some(&(_, pushes, locals)) = PROLOGUES.iter().find(|(e, ..)| *e == entry) {
            return Prologue { pushes: pushes.to_vec(), locals };
        }
        let mut p = Prologue { pushes: Vec::new(), locals: 0 };
        let mut words = [None; 16];
        let mut pc = entry;
        for _ in 0..16 {
            let op = bus.read16(pc);
            let n = ((op >> 8) & 0xF) as usize;
            let m = ((op >> 4) & 0xF) as u8;
            let branch = matches!(op, 0x000B | 0x002B)
                || matches!(op & 0xF0FF, 0x400B | 0x402B | 0x0003 | 0x0023)
                || matches!(op & 0xF000, 0xA000 | 0xB000)
                || matches!(op & 0xFF00, 0x8900 | 0x8B00 | 0x8D00 | 0x8F00);
            if branch {
                break;
            }
            if op & 0xFF0F == 0x2F06 {
                if p.pushes.last() == Some(&PR_PUSH) {
                    break;
                }
                p.pushes.push(m);
            } else if op == 0x4F22 {
                p.pushes.push(PR_PUSH);
            } else if op == 0x4F13 {
                p.pushes.push(GBR_PUSH);
            } else if op & 0xFF00 == 0x7F00 {
                p.locals = (op as u8 as i8 as i32).wrapping_neg() as u32;
                break;
            } else if op & 0xF000 == 0x9000 {
                let at = pc + 4 + 2 * (op & 0xFF) as u32;
                words[n] = Some(bus.read16(at) as i16 as i32);
            } else if op & 0xFF0F == 0x3F0C {
                if let Some(w) = words[m as usize] {
                    p.locals = w.wrapping_neg() as u32;
                }
                break;
            }
            pc += 2;
        }
        p
    }

    /// The pushes made on `sh2`'s stack with its registers, and the room below: the stack pointer
    /// after.
    fn enter(&self, sh2: &mut Sh2, bus: &mut dyn Bus) -> u32 {
        let mut sp = sh2.r[15];
        for &r in &self.pushes {
            sp = sp.wrapping_sub(4);
            let v = match r {
                PR_PUSH => sh2.pr,
                GBR_PUSH => sh2.gbr,
                MACH_PUSH => sh2.mach,
                MACL_PUSH => sh2.macl,
                SR_PUSH => sh2.sr,
                _ => sh2.r[r as usize],
            };
            bus.write32(sp, v);
        }
        sp.wrapping_sub(self.locals)
    }
}

/// `cpu.nested` for a call from one rewrite to another: `nested!(cpu, ENTRY, f(cpu, ...))`, the
/// callee's SH-2 entry and the call.
macro_rules! nested {
    ($cpu:ident, $entry:expr, $call:expr) => {{
        static PROLOGUE: std::sync::OnceLock<$crate::game::Prologue> = std::sync::OnceLock::new();
        $cpu.nested(&PROLOGUE, $entry, |$cpu| $call)
    }};
}
pub(crate) use nested;

/// The rewritten functions, each at its SH-2 function's entry: `entry => name[args] -> result`
/// (`-> ()` for none), arguments and result as u32. The SH-2 entry for each makes its SH-2
/// function's prologue (its pushes and frame, [`Prologue`]: its callees get the stack where the
/// SH-2 code had it; some hand the video chips' DMA a buffer of theirs on it), moves the
/// arguments from r4-r7 and the result to r0, and returns to PR.
macro_rules! rewrites {
    // the overlay window's code the client names $o (states::overlay_name)
    ($n:ident; overlay $o:literal; $($entry:literal => $f:path [$($arg:ident),* $(; $($sarg:ident),+)?] -> $ret:tt),* $(,)?) => {
        rewrites!(@once $($entry),*);
        $(
            $n.register_in($o, $entry, rewrites!(@native $entry, $f, [$($arg),* $(; $($sarg),+)?], $ret), stringify!($f), rewrites!(@contract $ret));
        )*
    };
    ($n:ident; $($entry:literal => $f:path [$($arg:ident),* $(; $($sarg:ident),+)?] -> $ret:tt),* $(,)?) => {
        rewrites!(@once $($entry),*);
        $(
            $n.register_as($entry, rewrites!(@native $entry, $f, [$($arg),* $(; $($sarg),+)?], $ret), stringify!($f), None, rewrites!(@contract $ret));
        )*
    };
    // a second entry at an address would silently replace the first
    (@once $($entry:literal),*) => {
        let mut entries = [$($entry),*];
        entries.sort_unstable();
        if let Some(w) = entries.windows(2).find(|w| w[0] == w[1]) {
            panic!("{:#010x} is rewritten twice", w[0]);
        }
    };
    (@native $entry:literal, $f:path, [$($arg:ident),* $(; $($sarg:ident),+)?], $ret:tt) => {
        |sh2: &mut Sh2, bus: &mut dyn Bus| {
            static PROLOGUE: std::sync::OnceLock<Prologue> = std::sync::OnceLock::new();
            let (ret, sp) = (sh2.pr, sh2.r[15]);
            #[allow(unused_mut, unused_variables)]
            let mut args = sh2.r[4..8].iter().copied();
            $( let $arg = args.next().unwrap_or(0); )*
            // arguments past the fourth: on the stack, the fifth at the stack pointer
            #[allow(unused_mut, unused_variables)]
            let mut at = sp;
            $($( #[allow(unused_assignments)] let $sarg = { let v = bus.read32(at); at = at.wrapping_add(4); v }; )+)?
            let prologue = PROLOGUE.get_or_init(|| Prologue::read(&mut *bus, $entry));
            sh2.r[15] = prologue.enter(sh2, &mut *bus);
            let fp = sh2.r[14];
            sh2.r[14] = sh2.r[15];
            let mut cpu = Cpu { sh2: &mut *sh2, bus, caller_r14: fp, r14_out: None };
            let result = $f(&mut cpu, $($arg),* $(, $($sarg),+)?);
            let r14 = cpu.r14_out.unwrap_or(fp);
            rewrites!(@result sh2, result, $ret);
            sh2.r[15] = sp;
            sh2.r[14] = r14;
            sh2.pc = ret;
        }
    };
    (@result $sh2:ident, $v:ident, ()) => { let () = $v; };
    (@result $sh2:ident, $v:ident, slave) => { let () = $v; };
    (@result $sh2:ident, $v:ident, $t:tt) => { $sh2.r[0] = $v; };
    (@contract ()) => { Contract::AbiVoid };
    (@contract slave) => { Contract::Keeps(SLAVE_KEEPS) };
    (@contract $t:tt) => { Contract::Abi };
}

/// What SGL's slave command handlers (`-> slave`) keep: the place in the commands (r4, past the
/// words they took) and the stack. The slave's command loop sets r0-r2 and r4 for each handler,
/// and no handler reads what another left in the rest: a native run with them filled with
/// garbage before each handler ended play1 and the attract mode on the interpreter's pictures
/// (HANDOFF section 6, the readable pass).
const SLAVE_KEEPS: u16 = 1 << 4 | 1 << 15;

/// Registers the rewritten functions (over the translations registered before).
pub fn register(n: &mut Natives) {
    n.namer = Some(states::overlay_name);
    rewrites! { n;
        0x0604_0000 => main_loop::main_loop[] -> u32,
        0x0600_44a0 => dma::dma_set[src, dst, size] -> u32,
        0x0600_44e8 => dma::mode_dma_set[src, dst, size, mode] -> u32,
        0x0600_4530 => dma::divdma_set[src, dst, size] -> u32,
        0x0600_4580 => dma::dma_control[] -> (),
        0x0600_46a0 => dma::dma_control_init[] -> (),
        0x0600_46b8 => dma::dma_check[] -> u32,
        0x0600_6a58 => math::fixed_mul[a, b] -> u32,
        0x0600_6a6e => math::remainder[a, b] -> u32,
        0x0600_6a78 => math::quotient[a, b] -> u32,
        0x0600_6a8e => math::fixed_div[a, b] -> u32,
        0x0600_57f8 => objects::obj_size[bytes] -> u32,
        0x0600_6aac => objects::find_free[area, blocks] -> u32,
        0x0600_5694 => objects::obj_set_after[area, blocks, exec, after] -> u32,
        0x0600_5714 => objects::obj_set_before[area, blocks, exec, before] -> u32,
        0x0600_6b00 => objects::obj_set[area, blocks, exec] -> u32,
        0x0600_579c => objects::obj_free[obj] -> (),
        0x0600_6b64 => objects::obj_exec[] -> (),
        0x0600_5634 => objects::obj_init[] -> (),
        0x0600_584c => pads::pad_init[] -> (),
        0x0600_58a8 => pads::pad_read[] -> (),
        0x0600_5b18 => palette::set_colours[colours, index, count] -> (),
        0x0600_5b5c => palette::set_vdp1_colours[colours, index, count] -> (),
        0x0600_5ba4 => palette::colours_blend[b] -> (),
        0x0600_5c90 => palette::colours_shift[s] -> (),
        0x0600_5a70 => palette::send_colours[] -> (),
        0x0600_5da0 => files::file_no[name] -> u32,
        0x0600_5e40 => files::load_file[name] -> u32,
        0x0600_5fb0 => files::load_files[] -> (),
        0x0600_60c4 => files::clear_loads[] -> (),
        0x0600_5e10 => files::init_loads[] -> u32,
        0x0600_4468 => vblank::init_vblank[] -> (),
        0x0600_438c => vblank::run_callbacks[] -> (),
        0x0600_43d4 => vblank::add_callback[f] -> u32,
        0x0600_4408 => vblank::remove_callback[f] -> u32,
        0x0600_5f10 => files::read_file[name, dst] -> u32,
        0x0600_5e9c => files::file_size[name] -> u32,
        0x0600_5edc => files::load_size[load] -> u32,
        0x0600_603c => files::start_load[load, dst] -> u32,
        0x0600_6074 => files::load_done[load] -> u32,
        0x0600_60b0 => files::free_load[load] -> (),
        0x0600_475c => sound::command_done[slot] -> u32,
        0x0600_4718 => sound::send_to[slot, command, len] -> u32,
        0x0600_47bc => sound::send[command, len] -> u32,
        0x0600_4780 => sound::driver_idle[] -> u32,
        0x0600_4804 => sound::set_flags[bits] -> (),
        0x0600_48b8 => sound::total_volume[volume] -> (),
        0x0600_48ec => sound::mixer[bank, mixer] -> (),
        0x0600_4928 => sound::mixer_param[which, level, pan] -> (),
        0x0600_4974 => sound::sound_setup[pan1, pan0] -> (),
        0x0600_49cc => sound::sound_setup_default[] -> (),
        0x0600_4b88 => sound::start_seq[control, song, priority] -> (),
        0x0600_4bc4 => sound::seq_volume[control, volume, rate] -> (),
        0x0600_4c00 => sound::stop_seq[control] -> (),
        0x0600_4a64 => sound::map_area[n] -> u32,
        0x0600_4adc => sound::copy_to_area[n, src, size] -> (),
        0x0600_4b18 => sound::load_sound_file[n, buffer] -> u32,
        0x0600_49e8 => sound::sound_load[] -> (),
        0x0600_4ddc => sound::stop_effects[] -> (),
        0x0600_4e98 => sound::request_music[music] -> (),
        0x0600_4f0c => sound::music_fade_out[frames] -> (),
        0x0600_4edc => sound::music_fade_to[volume, frames] -> (),
        0x0600_4f3c => sound::se_control[se] -> u32,
        0x0600_50d4 => sound::se_set[se] -> u32,
        0x0600_52e0 => sound::se_stop[se, control] -> u32,
        0x0600_53dc => sound::sound_exec[m] -> (),
        0x0600_5598 => sound::sound_manager_set[] -> u32,
        0x0600_813a => clib::ashiftrt_r4::<24>[v] -> (),
        0x0600_815a => clib::ashiftrt_r4::<13>[v] -> (),
        0x0600_815c => clib::ashiftrt_r4::<12>[v] -> (),
        0x0600_815e => clib::ashiftrt_r4::<11>[v] -> (),
        0x0600_8160 => clib::ashiftrt_r4::<10>[v] -> (),
        0x0600_8162 => clib::ashiftrt_r4::<9>[v] -> (),
        0x0600_8164 => clib::ashiftrt_r4::<8>[v] -> (),
        0x0600_8166 => clib::ashiftrt_r4::<7>[v] -> (),
        0x0600_8168 => clib::ashiftrt_r4::<6>[v] -> (),
        0x0600_817c => clib::memset[dst, c, n] -> u32,
        0x0606_62d8 => clib::divide[a, b] -> u32,
        0x0600_81d0 => clib::memcpy[dst, src, n] -> u32,
        0x0600_79a8 => sound::seq_status[buf, control] -> (),
        0x0600_46d8 => sound::load_sound_data[n, dst] -> u32,
        0x0600_8058 => system::set_scu_mask[mask, bits] -> (),
        0x0600_69d8 => system::shift_left[x, bits] -> u32,
        0x0600_68d8 => system::player_started[p] -> (),
        0x0600_6928 => system::player_ended[p] -> (),
        0x0600_6364 => backup::backup_byte[i] -> u32,
        0x0600_637c => backup::backup_long[i] -> u32,
        0x0600_63a4 => backup::set_backup_byte[i, v] -> (),
        0x0600_63bc => backup::set_backup_long[i, v] -> u32,
        0x0600_63e4 => backup::load_backup[] -> u32,
        0x0600_66ec => backup::format_backup[why] -> (),
        0x0600_65fc => backup::save_scores[] -> u32,
        0x0604_2db8 => director::game_set[] -> u32,
        0x0604_2b14 => director::game_exec[game] -> (),
        0x0604_0c64 => director::mode_done[] -> (),
        0x0604_0c8c => director::mode_skipped[] -> (),
        0x0604_0cb4 => director::director_flags[] -> u32,
        0x0604_02c4 => states::load_mode_file[mode] -> u32,
        0x0604_12e8 => states::state_reset[game] -> (),
        0x0604_13b0 => states::state_load_mode[game] -> (),
        0x0604_17a4 => states::state_run_mode[game] -> (),
        0x0604_1578 => states::state_screen[game] -> (),
        0x0604_1848 => states::state_title[game] -> (),
        0x0604_1df0 => states::state_stage[game] -> (),
        0x0604_2734 => states::state_stage_end[game] -> (),
        0x0604_1010 => states::clear_mode[game] -> (),
        0x0605_e6dc => screen::set_fade_offset[r, g, b] -> (),
        0x0605_e8dc => screen::fade_new[r, g, b, speed] -> u32,
        0x0605_e93c => screen::fade_to[r, g, b, speed] -> (),
        0x0605_e9d8 => screen::stop_fades[] -> (),
        0x0605_61e0 => screen::bullets_cleared[] -> (),
        0x0605_e73c => screen::fade_exec[fade] -> (),
        0x0605_e1b0 => screen::screen_set[] -> (),
        0x0605_e49c => screen::screen_exec[s] -> (),
        0x0605_e2dc => screen::screen_fields[] -> (),
        0x0605_ea98 => screen::screen_vblank[] -> (),
        0x0605_e6c0 => screen::offsets_clear[] -> (),
        0x0605_fdc0 => screen::take_plane_shared[] -> (),
        0x0605_fde8 => screen::request_clear_text_plane[] -> (),
        0x0605_fdfc => screen::clear_text_plane[] -> (),
        0x0605_ef44 => bg::map_hit_off[] -> (),
        0x0605_ef58 => bg::map_hit_on[x, map] -> (),
        0x0605_ef74 => bg::map_hit_by[f] -> (),
        0x0605_ef90 => bg::map_hit[x, y] -> u32,
        0x0605_f28c => bg::map_point[x, y, scroll, map] -> u32,
        0x0605_f14c => bg::map_clear[map] -> (),
        0x0605_f170 => bg::map_stream[plane, map] -> u32,
        0x0605_efd4 => bg::plane_a_speed[dx, dy] -> (),
        0x0605_eff8 => bg::plane_b_speed[dx, dy] -> (),
        0x0605_f01c => bg::plane_speed[plane, dx, dy] -> (),
        0x0605_f028 => bg::plane_step[plane] -> (),
        0x0605_f040 => bg::plane_a_frame[] -> (),
        0x0605_f08c => bg::plane_b_frame[] -> (),
        0x0605_f0d8 => bg::plane_corner[plane, out] -> (),
        0x0605_f3a4 => bg::mover_move[dx, dy, dz] -> (),
        0x0605_f3ec => bg::move_by[s, dx, dy, dz] -> (),
        0x0605_f40c => bg::view_toward[target] -> (),
        0x0605_f468 => bg::view_matrix[] -> (),
        0x0605_f4d0 => bg::view_matrix_moved_first[] -> (),
        0x0605_f530 => bg::rotation_matrix[o] -> (),
        0x0605_f5b0 => bg::view_along[path] -> (),
        0x0605_f648 => bg::view_along_eased[path] -> (),
        0x0605_f6e0 => bg::draw_models_near[table] -> (),
        0x0605_f794 => bg::draw_models[n, list] -> (),
        0x0605_f860 => bg::edge_line[angle] -> u32,
        0x0605_f8c4 => bg::edge_window[table, y, angle] -> (),
        0x0604_e3a8 => effects::polygon_exec[obj] -> (),
        0x0604_e6d8 => effects::polygon_set[fx] -> u32,
        0x0604_dd88 => effects::grow_exec[obj] -> (),
        0x0604_ddf4 => effects::grow_set[obj, size, frames] -> (),
        0x0604_de5c => effects::accel_exec[obj] -> (),
        0x0604_de98 => effects::accel_set[obj, ax, ay] -> (),
        0x0604_eb80 => effects::last_exec[obj] -> (),
        0x0604_eba0 => effects::widen_exec[obj] -> (),
        0x0604_ebcc => effects::lengthen_exec[obj] -> (),
        0x0604_ebf8 => effects::lengthen_wrap_exec[obj] -> (),
        0x0604_ec34 => effects::fall_exec[obj] -> (),
        0x0604_ea48 => effects::popup[p] -> (),
        0x0604_e948 => effects::digit_set[p] -> (),
        0x0604_e898 => effects::digit_exec[obj] -> (),
        0x0604_daa8 => bullets::aimed_exec[b] -> (),
        0x0604_dbc4 => bullets::aimed_set[shot] -> u32,
        0x0604_d8d8 => actors::move_by_angle[actor, speed, angle] -> (),
        0x0604_d95c => actors::trail_init[actor, pieces, spacing, boxes] -> (),
        0x0604_da00 => actors::trail_record[actor, angle] -> (),
        0x0605_75e4 => explosions::shrink_exec[o] -> (),
        0x0605_88d4 => explosions::explosion_exec[x] -> (),
        0x0605_7634 => explosions::afterimage_set[src, unused, frames, speed] -> u32,
        0x0605_7754 => explosions::smoke_burst[d] -> (),
        0x0605_77d0 => explosions::smoke_puff[e] -> (),
        0x0605_786c => explosions::spark_burst[e] -> (),
        0x0605_790c => explosions::smoke_trail_exec[o] -> (),
        0x0605_7998 => explosions::smoke_trail_set[d] -> (),
        0x0605_7a64 => explosions::smoking_exec[o] -> (),
        0x0605_7b70 => explosions::smoking_set[e] -> (),
        0x0605_7bcc => explosions::burning_exec[o] -> (),
        0x0605_7cb0 => explosions::burning_set[e] -> (),
        0x0605_7da4 => explosions::cross_set[e] -> (),
        0x0605_7e38 => explosions::cross_flash[e] -> (),
        0x0605_7e78 => explosions::jet_exec[j] -> (),
        0x0605_7fe8 => explosions::jet_set[d] -> u32,
        0x0605_8eac => explosions::glow_exec[g] -> (),
        0x0605_8f34 => explosions::glow_set[d] -> u32,
        0x0605_9034 => explosions::ring_exec[o] -> (),
        0x0605_9080 => explosions::ring_draw[s] -> (),
        0x0605_9144 => explosions::ring_set[d] -> (),
        0x0605_8ba4 => explosions::explosion_set[p] -> (),
        0x0605_8e1c => explosions::blast_set[d] -> (),
        0x0605_8c5c => explosions::blast_exec[o] -> (),
        0x0605_8d00 => explosions::blast_draw[s] -> (),
        0x0605_80bc => explosions::twin_exec[o] -> (),
        0x0605_8278 => explosions::twin_set[p] -> (),
        0x0605_968c => explosions::smoke_and_sparks[p, n] -> u32,
        0x0605_8444 => explosions::ember_exec[o] -> (),
        0x0605_8520 => explosions::ember_set[d] -> (),
        0x0605_8638 => explosions::debris_exec[o] -> (),
        0x0605_86e4 => explosions::debris_set[p, spread] -> (),
        0x0604_8278 => shots::shot_spark[s] -> (),
        0x0604_8318 => shots::shot_a_exec[s] -> (),
        0x0604_8480 => shots::shot_a_side_exec[s] -> (),
        0x0604_860c => shots::shot_a_set[p] -> u32,
        0x0604_8acc => shots::shot_b_exec[s] -> (),
        0x0604_8d84 => shots::shot_b_set[p] -> u32,
        0x0604_8f30 => shots::shot_c_exec[s] -> (),
        0x0604_9160 => shots::trail_exec[o] -> (),
        0x0604_9318 => shots::bolt_exec[s] -> (),
        0x0604_965c => shots::shot_c_set[p] -> u32,
        0x0604_98d4 => shots::beam_draw[s] -> (),
        0x0604_9aa0 => shots::beam_boxes[b] -> (),
        0x0604_9c54 => shots::beam_exec[b] -> (),
        0x0604_a0f8 => shots::lock_exec[r] -> (),
        0x0604_a29c => shots::seeker_exec[o] -> (),
        0x0604_a854 => shots::shot_ab_set[p] -> u32,
        0x0604_a914 => shots::shot_ac_exec[s] -> (),
        0x0604_aa98 => shots::shot_ac_set[p] -> u32,
        0x0604_ac8c => shots::burst_draw[sp] -> (),
        0x0604_ad2c => shots::burst_exec[b] -> (),
        0x0604_aef8 => shots::strike_draw[sp] -> (),
        0x0604_b0e4 => shots::strike_exec[t] -> (),
        0x0604_b510 => shots::bc_bolt_exec[s] -> (),
        0x0604_b830 => shots::ring_draw[sp] -> (),
        0x0604_b98c => shots::ring_exec[s] -> (),
        0x0604_bdf8 => shots::shot_bc_set[p] -> u32,
        0x0604_be74 => shots::sword_draw[sp] -> (),
        0x0604_c2e8 => shots::sword_exec[s] -> (),
        0x0604_c9a4 => shots::shot_sword_set[p] -> u32,
        0x0604_caa8 => shots::hyper_piece_exec[o] -> (),
        0x0604_cd28 => shots::hyper_exec[o] -> (),
        0x0604_d444 => shots::hyper_set[p] -> u32,
        0x0604_5638 => kinds::sprite_kind_fix[k, p] -> (),
        0x0604_5688 => kinds::sprite_kind_read[k, dst] -> u32,
        0x0604_56cc => kinds::kind_load[k, dst] -> u32,
        0x0604_570c => kinds::sprite_kind_install[k, vram, first, colour] -> u32,
        0x0604_583c => kinds::model_kind_fix[k, p] -> (),
        0x0604_58cc => kinds::model_kind_read[k, dst] -> u32,
        0x0604_5910 => kinds::kind_load[k, dst] -> u32,
        0x0604_5950 => kinds::model_kind_install[k, vram, first, colour] -> u32,
        0x0605_3620 => kinds::kinds_exec[o] -> (),
        0x0605_3aa0 => kinds::kinds_set[] -> u32,
        0x0604_7f60 => player::player_exec[p] -> (),
        0x0604_7b0c => player::control[p] -> (),
        0x0604_77ec => player::weapons_frame[p] -> (),
        0x0604_75d4 => player::extend_check[p] -> (),
        0x0604_6900 => player::move_ship[p] -> (),
        0x0604_6ac0 => player::weapon_a[p] -> (),
        0x0604_6bb4 => player::weapon_b[p] -> (),
        0x0604_6cb0 => player::weapon_c[p] -> (),
        0x0604_6e10 => player::weapon_ab[p] -> (),
        0x0604_6ea0 => player::weapon_ac[p] -> (),
        0x0604_6fc0 => player::weapon_bc[p] -> (),
        0x0604_7154 => player::sword_state[p] -> (),
        0x0604_7a18 => player::aura_exec[o] -> (),
        0x0604_7228 => player::hyper_aura_exec[o] -> (),
        0x0604_732c => player::hyper_state[p] -> (),
        0x0604_66d4 => player::ship_start[p] -> (),
        0x0604_673c => player::ship_entry[p] -> (),
        0x0604_74dc => player::ship_dead[p] -> (),
        0x0606_16b8 => player::ships_still[] -> (),
        0x0606_1704 => player::ships_hidden[] -> (),
        0x0606_1738 => player::scene_on[] -> (),
        0x0606_175c => player::scene_off[] -> (),
        0x0606_1780 => player::start_pressed[] -> u32,
        0x0606_23d4 => player::jet_set[ship, frames] -> u32,
        0x0606_216c => player::jet_follow[j] -> (),
        0x0606_21f4 => player::jet_exec[j] -> (),
        0x0606_20c0 => player::puff_set[f] -> (),
        0x0606_2074 => player::puff_exec[o] -> (),
        0x0606_248c => rings::ring_draw[o] -> (),
        0x0606_254c => rings::ring_points[o] -> (),
        0x0606_25fc => rings::ring_step[o] -> (),
        0x0606_26bc => rings::ring_exec[o] -> (),
        0x0606_2738 => rings::ring_set[d] -> u32,
        0x0605_1604 => results::piece_sink[o] -> (),
        0x0605_1634 => results::piece_fly[o] -> (),
        0x0605_17b0 => results::piece_slide[o] -> (),
        0x0605_1928 => results::percent_draw[s] -> (),
        0x0605_1a98 => results::percent_exec[o] -> (),
        0x0605_1cf0 => results::score_draw[s] -> (),
        0x0605_1de0 => results::score_exec[o] -> (),
        0x0605_1f94 => results::results_exec[o] -> (),
        0x0605_284c => results::results_set[delay] -> u32,
        0x0605_1250 => banner::banner_idle[] -> (),
        0x0605_125c => banner::banner_sprites[] -> (),
        0x0605_1394 => banner::banner_plane_4[] -> (),
        0x0605_13ac => banner::banner_plane_3[] -> (),
        0x0605_13c4 => banner::banner_idle[] -> (),
        0x0605_13d0 => banner::banner_plane_4[] -> (),
        0x0605_13e8 => banner::banner_exec[o] -> (),
        0x0605_1598 => banner::banner_set[] -> u32,
        0x0604_ec60 => banner::banner_step_0[] -> (),
        0x0604_ef84 => banner::banner_step_1[] -> (),
        0x0604_f0d0 => banner::banner_step_2[] -> (),
        0x0604_f30c => banner::banner_step_3[] -> (),
        0x0604_f4cc => banner::banner_step_4[] -> (),
        0x0604_f684 => banner::banner_step_5[] -> (),
        0x0604_f9dc => banner::banner_step_6[] -> (),
        0x0604_fb90 => banner::banner_step_7[] -> (),
        0x0604_fcdc => banner::banner_step_8[] -> (),
        0x0604_ff10 => banner::banner_step_9[] -> (),
        0x0605_0054 => banner::banner_step_10[] -> (),
        0x0605_03bc => banner::banner_idle[] -> (),
        0x0605_03c8 => banner::banner_step_11[] -> (),
        0x0605_0584 => banner::banner_step_12[] -> (),
        0x0605_07d8 => banner::banner_idle[] -> (),
        0x0605_07e4 => banner::banner_step_13[] -> (),
        0x0605_0934 => banner::banner_idle[] -> (),
        0x0605_0940 => banner::banner_step_14[] -> (),
        0x0605_0b90 => banner::banner_plane_1[] -> (),
        0x0605_0ba8 => banner::banner_plane_0[] -> (),
        0x0605_0bc0 => banner::banner_step_15[] -> (),
        0x0605_0cfc => banner::banner_step_16[] -> (),
        0x0605_0f70 => banner::banner_idle[] -> (),
        0x0605_0f7c => banner::banner_step_17[] -> (),
        0x0605_72ec => timeline::timeline_exec[t] -> (),
        0x0605_7354 => timeline::timeline_set[events] -> u32,
        0x0605_7210 => timeline::timeline_run[t] -> (),
        0x0605_64f8 => timeline::scroll_event[] -> u32,
        0x0605_6b14 => parts::body_exec[body] -> (),
        0x0605_6a58 => aim::nearer_player[p] -> u32,
        0x0605_6ad4 => aim::angle_to_target[obj, delay] -> u32,
        0x0605_9418 => orbits::orbit_place[o] -> u32,
        0x0605_94cc => orbits::orbit_exec[o] -> (),
        0x0605_9554 => orbits::orbit_set[d, watched, mask, centre; angle_from] -> (),
        0x0605_91dc => orbits::start_0[o] -> (),
        0x0605_9294 => orbits::start_1[o] -> (),
        0x0605_932c => orbits::start_2[o] -> (),
        0x0605_93ac => orbits::start_4[o] -> (),
        0x0605_9228 => orbits::frame_0[o] -> u32,
        0x0605_92c4 => orbits::frame_1[o] -> u32,
        0x0605_933c => orbits::frame_2[o] -> u32,
        0x0605_93c0 => orbits::frame_4[o] -> u32,
        0x0605_3b10 => bullets::bullet_ends[b, back] -> u32,
        0x0605_4050 => bullets::heavy_set[d] -> u32,
        0x0605_41d0 => bullets::heavy_bullet[d] -> u32,
        0x0605_41f0 => bullets::laser_segment_exec[o] -> (),
        0x0605_4354 => bullets::laser_segment_set[d, muzzle, hits, angle_at; from] -> (),
        0x0605_4504 => bullets::muzzle_exec[o] -> (),
        0x0605_46ac => bullets::laser_set[d, angle_at] -> (),
        0x0605_47ec => bullets::missile_exec[o] -> (),
        0x0605_49f8 => bullets::missile_set[d] -> u32,
        0x0605_4b60 => bullets::missile_set_plain[d] -> u32,
        0x0605_4b9c => bullets::pulse_exec[b] -> (),
        0x0605_4c8c => bullets::pulse_set[shot] -> u32,
        0x0605_4e28 => bullets::burst_exec[o] -> (),
        0x0605_4f64 => bullets::burst_ball_draw[s] -> (),
        0x0605_5154 => bullets::burst_star_draw[s] -> (),
        0x0605_5310 => bullets::burst_set[d] -> u32,
        0x0605_547c => bullets::shell_exec[o] -> (),
        0x0605_587c => bullets::shell_set[d] -> u32,
        0x0605_5a68 => bullets::rocket_exec[b] -> (),
        0x0605_5b5c => bullets::rocket_set[shot] -> u32,
        0x0605_5d84 => bullets::rocket_set_plain[shot] -> u32,
        0x0605_6260 => bg::ground_still[] -> u32,
        0x0605_a000 => ground::route_step[o, script] -> u32,
        0x0605_a340 => ground::aim_step[o, script, keep] -> u32,
        0x0605_a460 => ground::route_move[o] -> (),
        0x0605_a4cc => ground::ground_exec[o] -> (),
        0x0605_a780 => ground::ground_set[d, script] -> u32,
        0x0605_a970 => ground::gun_exec[o] -> (),
        0x0605_aae8 => ground::barrel_exec[o] -> (),
        0x0605_b0d4 => ground::barrel_init[o, gun] -> (),
        0x0605_b168 => ground::gun_set[d, script, aim] -> u32,
        0x0605_b568 => ground::wounded_exec[o] -> (),
        0x0605_b748 => ground::on_body[o] -> u32,
        0x0605_b7d4 => ground::big_gun_exec[o] -> (),
        0x0605_bbac => ground::wounded_set[d, script, aim] -> u32,
        0x0605_be90 => ground::route_exec[o] -> (),
        0x0605_bf28 => ground::launcher_exec[o] -> (),
        0x0605_c1d4 => ground::turret_exec[o] -> (),
        0x0605_c464 => ground::launcher_set[d, carrier] -> u32,
        0x0605_cc54 => ground::spreader_exec[o] -> (),
        0x0605_cee8 => ground::spreader_set[d, script] -> u32,
        0x0605_d028 => ground::side_laser_exec[o] -> (),
        0x0605_d228 => ground::side_laser_set[d, carrier] -> u32,
        0x0605_d35c => ground::carrier_exec[o] -> (),
        0x0605_d51c => ground::carrier_set[d, script, kind] -> u32,
        0x0605_d780 => ground::burster_exec[o] -> (),
        0x0605_d988 => ground::burster_draw[s] -> (),
        0x0605_da58 => ground::burster_set[d, script] -> u32,
        0x0605_dba4 => ground::shard_exec[o] -> (),
        0x0605_dc18 => ground::shard_set[d, from] -> (),
        0x0605_de54 => ground::hatch_exec[o] -> (),
        0x0605_dd54 => ground::hatch_opens[o] -> (),
        0x0605_e030 => ground::hatch_set[d, script] -> u32,
        0x0605_c700 => ground::ride[o] -> (),
        0x0605_c760 => ground::rider_set[d, on] -> u32,
        0x0605_c86c => ground::platform_set[d, script] -> u32,
        0x0606_38c0 => aim::point_area_wide[p] -> u32,
        0x0606_3620 => ribbons::ribbon_init[o, d, mover] -> u32,
        0x0606_3884 => ribbons::ribbon_draw[s] -> (),
        0x0606_3940 => ribbons::ribbon_end[o] -> (),
        0x0606_39e0 => ribbons::ribbon_exec[o] -> (),
        0x0606_3b94 => ribbons::ribbon_fade[o] -> (),
        0x0606_3c20 => ribbons::ribbon_move[o] -> (),
        0x0606_3cec => ribbons::ribbon_straight[o] -> (),
        0x0606_3dc0 => ribbons::ribbon_homing[o] -> (),
        0x0606_3e30 => ribbons::ribbon_wave[o] -> (),
        0x0606_3ef4 => ribbons::ribbon_home[o] -> (),
        0x0606_3f70 => ribbons::ribbon_edges[o, i] -> (),
        0x0606_4194 => ribbons::ribbon_edge[o, x0, y0, x1; y1, width, quad] -> (),
        0x0606_4244 => ribbons::ribbon_boxes[o, i] -> (),
        0x0606_42e0 => ribbons::ribbon_set[kind, d, flags] -> (),
        0x0606_4494 => ribbons::ribbon_flash[o] -> (),
        0x0606_4358 => effects::flare_exec[o] -> (),
        0x0606_43ec => effects::flare_set[d] -> u32,
        0x0606_4a28 => models::pod_draw[s] -> (),
        0x0606_46c4 => models::pod_model[o] -> (),
        0x0606_48e4 => models::pod_sprite[o] -> (),
        0x0606_4520 => models::pod_glow[o] -> (),
        0x0606_4a7c => models::pod_init[o, n] -> (),
        0x0606_4d34 => models::pod_set[exec, n] -> u32,
        0x0606_4d98 => models::pod_normals[] -> (),
        0x0606_2b54 => models::model31_draw[s] -> (),
        0x0606_2db0 => models::model31_init[o] -> (),
        0x0606_2df4 => models::model31_set[exec] -> u32,
        0x0606_4e54 => models::pods_shading[] -> (),
        0x0606_3900 => aim::point_area[p] -> u32,
        0x0605_3bfc => bullets::bullet_frame[b] -> (),
        0x0605_3ccc => bullets::bullet_move[b] -> (),
        0x0605_3cfc => bullets::bullet_swirl[b] -> (),
        0x0605_3db0 => bullets::bullet_set[shot] -> u32,
        0x0605_3f48 => bullets::bullet_set_plain[shot] -> u32,
        0x0605_3f70 => bullets::shootable_bullet[b] -> (),
        0x0605_7454 => actors::drift_effect[e] -> u32,
        0x0605_6168 => actors::flap_and_face[actor, v] -> (),
        0x0605_62b4 => actors::shift_in_bytes[at, v, n] -> (),
        0x0605_62fc => actors::shift_in_words[at, v, n] -> (),
        0x0605_681c => actors::enemy_clear[enemy] -> (),
        0x0605_2908 => models::model_init[obj] -> (),
        0x0605_2970 => models::follow_parent[obj] -> u32,
        0x0605_29ac => models::model_boxes[obj] -> (),
        0x0605_2b08 => models::beam_boxes[obj, src] -> (),
        0x0605_2b88 => models::beam_length[obj, length] -> (),
        0x0605_2c44 => models::model_point[obj, p] -> (),
        0x0605_2ca0 => models::model_screen_point[obj, p] -> (),
        0x0605_2d04 => models::group_draw[s] -> (),
        0x0605_2d6c => models::part_draw[s] -> (),
        0x0605_2e50 => models::model_draw[s] -> (),
        0x0605_2f3c => models::group_end[s] -> (),
        0x0605_2f54 => models::end_with_parent[obj] -> (),
        0x0605_2f78 => models::group_end_set[head] -> u32,
        0x0605_332c => models::model_parts_set[root, table] -> u32,
        0x0605_2ff0 => models::model_group_set[root, table] -> u32,
        0x0605_ecb4 => screen::queue_fill[src, dst, size] -> u32,
        0x0605_ecf0 => screen::queue_copy[src, dst, size] -> u32,
        0x0605_ed2c => screen::queue_long_copy[src, dst, size, piece] -> u32,
        0x0605_ed94 => screen::queue_cells[src, dst, w, h; char_offset, add] -> u32,
        0x0605_fcdc => screen::plane_picture[picture] -> (),
        0x0605_ee3c => screen::put_cells[src, dst, w, h; char_offset, add] -> (),
        0x0605_ee94 => screen::put_long_cells[src, dst, w, h; char_offset, add] -> (),
        0x0605_eee4 => screen::fill_cells[cell, dst, w, h] -> (),
        0x0605_edf8 => screen::copy_words[src, dst, bytes] -> (),
        0x0605_ed84 => screen::long_copies_on[] -> u32,
        0x0605_ef20 => screen::offset_list[delta, list, n] -> (),
        0x0605_e24c => screen::screen_off[] -> (),
        0x0604_0f24 => system::coin_settings_shown[] -> u32,
        0x0606_0ff8 => text::char_index[c] -> u32,
        0x0606_10e4 => text::put_char[c, at, attr] -> u32,
        0x0606_11b8 => text::put_string[s, at, attr] -> (),
        0x0606_0c58 => text::put_number[n, at] -> u32,
        0x0606_111c => text::put_large_char[c, at, attr] -> u32,
        0x0606_1204 => text::put_large_string[s, at, attr] -> (),
        0x0606_0894 => text::set_mode[mode] -> (),
        0x0606_0b34 => text::hide_line[p] -> (),
        0x0606_0b78 => text::show_message[p, message] -> (),
        0x0606_0a48 => text::show_buffers[p] -> (),
        0x0606_0c08 => text::credits_shown[] -> u32,
        0x0606_0c2c => text::credits_hidden[] -> u32,
        0x0606_0fc8 => text::credits_set[] -> u32,
        0x0606_06dc => text::clear_line[p] -> (),
        0x0606_0790 => text::clear_credit_line[p] -> (),
        0x0606_07c4 => text::send_line[src, dst, tiles] -> (),
        0x0606_0824 => text::clear_hidden_lines[] -> (),
        0x0606_08bc => text::send_lines[] -> (),
        0x0604_0fc0 => system::coins[slot] -> u32,
        0x0606_0cd8 => text::credit_text[d, p, credits, coins] -> (),
        0x0606_0dac => text::draw_credits[d, p, at] -> (),
        0x0606_0e7c => text::credits_exec[d] -> (),
        0x0604_3874 => hits::in_hit_set[of, obj] -> u32,
        0x0604_37fc => hits::no_hit[] -> (),
        0x0604_3978 => hits::no_hit[] -> (),
        0x0604_3808 => hits::hit[a, b] -> (),
        0x0604_38b8 => hits::hit_once[a, b] -> (),
        0x0604_3984 => hits::touch[a, b] -> (),
        0x0604_3088 => hits::damage[obj, power] -> u32,
        0x0604_34e4 => hits::weapon_hit[shot, target] -> (),
        0x0604_3108 => hits::score_kill[p, weapon, target, points] -> (),
        0x0605_61ac => hits::score_hit_by[target, points] -> (),
        0x0606_4f3c => math::clamp[x, limit] -> u32,
        0x0606_4f50 => math::at_least[x, least] -> u32,
        0x0606_2e7c => math::lerp[a, b, t] -> u32,
        0x0606_2ebc => math::ease[a, b, t] -> u32,
        0x0604_39b0 => hits::sparks[a, b, a_box, b_box] -> (),
        0x0604_3c64 => hits::absorb[a, b] -> (),
        0x0604_5090 => hits::chain_reset[p] -> (),
        0x0604_50d4 => hits::hits_reset[] -> (),
        0x0604_5154 => hits::hits_set[] -> u32,
        0x0604_51b4 => hits::register_boxes[obj] -> u32,
        0x0604_6194 => camera::angle_diff[from, to] -> u32,
        0x0604_6214 => camera::angle_within[angle, centre, range] -> u32,
        0x0604_62dc => hud::boss_gauge_exec[g] -> (),
        0x0604_64b8 => hud::boss_gauge_life[life] -> (),
        0x0604_64d8 => hud::boss_gauge_set[most] -> u32,
        0x0604_666c => parts::flash_with[part, with] -> (),
        0x0604_668c => parts::flash_against[part, with] -> (),
        0x0604_2ff8 => camera::camera_set[] -> u32,
        0x0604_2e6c => camera::camera_exec[cam] -> (),
        0x0604_114c => play::player_reset[p] -> (),
        0x0604_129c => play::game_start[] -> (),
        0x0604_016c => main_loop::init_game[] -> (),
        0x0604_5e30 => aim::angle_to[a, b] -> u32,
        0x0604_5e60 => aim::length[dx, dy] -> u32,
        0x0604_5ee4 => aim::distance[a, b] -> u32,
        0x0604_5f70 => aim::nearer_player_to[obj] -> u32,
        0x0604_5b1c => models::model_colour[kind, frame, colour] -> (),
        0x0604_5b60 => models::models_colour[kind, colour] -> (),
        0x0604_5c50 => models::model_mesh[kind, frame, on] -> (),
        0x0604_5cdc => models::model_flip_bits[kind, frame, bits] -> (),
        0x0604_61cc => camera::angle_ahead[a, b] -> u32,
        0x0604_61f0 => camera::angle_behind[a, b] -> u32,
        0x0605_6294 => actors::fill_bytes[at, v, n] -> (),
        0x0605_62dc => actors::fill_words[at, v, n] -> (),
        0x0605_6628 => actors::script_start[s, script] -> (),
        0x0605_fdd8 => screen::set_plane_shared_next[v] -> (),
        0x0605_e71c => screen::set_offset_b[r, g, b] -> (),
        0x0605_6f7c => actors::depth_box_exec[o] -> (),
        0x0605_9d24 => ground::thrust[o, s, behind, scale] -> (),
        0x0605_9e4c => ground::blown_up[o] -> (),
        0x0605_9e9c => ground::blown_up_1[o] -> (),
        0x0605_9ee4 => ground::blown_up_wounded[o] -> (),
        0x0605_9f50 => ground::gun_blown_off[o] -> (),
        0x0605_9fc4 => ground::blown_up_burster[o] -> (),
        0x0605_9a84 => actors::bonus_exec[o] -> (),
        0x0605_9bd8 => actors::bonus_set[d] -> u32,
        0x0605_6ff4 => actors::depth_box_set[kind] -> u32,
        0x0606_2e40 => math::unwrap_angles[a, b] -> (),
        0x0606_71cc => sgl::sl_push_matrix[] -> u32,
        0x0606_7200 => sgl::sl_pop_matrix[] -> u32,
        0x0606_721c => sgl::sl_translate[x, y, z] -> (),
        0x0606_73ec => sgl::sl_scale[x, y, z] -> (),
        0x0606_745c => sgl::sl_put_polygon[pd] -> u32,
        0x0606_750c => sgl::sl_convert[p, out] -> u32,
        0x0606_76fc => sgl::sl_convert_pixels[p, out] -> u32,
        0x0606_780c => sgl::sl_atan[x, y] -> u32,
        0x0606_713c => sgl::sl_cur_window[win] -> u32,
        0x0606_db2c => sgl::sl_calc_point[x, y, z, out] -> (),
        0x0606_db5c => sgl::sl_push_no_copy[] -> u32,
        0x0606_db7c => sgl::sl_load_matrix[src] -> (),
        0x0606_db9c => sgl::sl_get_matrix[dst] -> (),
        0x0606_9a54 => sgl::sl_sin_cos[a] -> (),
        0x0606_75ac => sgl::sl_put_sprite[pos, attr, angle] -> u32,
        0x0606_76ac => sgl::sl_put_sprite_hv[pos, attr, angle] -> u32,
        0x0607_0e22 => sgl::sl_disp_sprite[pos, attr, angle] -> u32,
        0x0606_778c => sgl::sl_put_quad[cmd, z] -> u32,
        0x0606_e9fc => sgl::sl_check_on_screen[p, size] -> u32,
        0x0606_98dc => sgl::sl_priority[screen, pri] -> (),
        0x0606_e17c => sgl::sl_window_control[screen, v] -> (),
        0x0606_e1bc => sgl::sl_line_scroll_control[screen, v] -> (),
        0x0606_e45c => sgl::sl_col_rate[screen, v] -> (),
        0x0606_e47c => sgl::sl_scr_pos_nbg0[x, y] -> (),
        0x0606_e46c => sgl::sl_zoom_nbg0[x, y] -> (),
        0x0606_e4cc => sgl::sl_scr_pos_nbg1[x, y] -> (),
        0x0606_e4bc => sgl::sl_zoom_nbg1[x, y] -> (),
        0x0606_e25c => sgl::sl_col_offset_on[on, b] -> (),
        0x0607_0ddc => sgl::sl_color_calc[v] -> (),
        0x0606_e18c => sgl::sl_special_mode_nbg0[v] -> (),
        0x0606_e190 => sgl::sl_special_mode_nbg1[v] -> (),
        0x0606_e1a6 => sgl::sl_special_mode_nbg2[v] -> (),
        0x0606_e194 => sgl::sl_special_mode_nbg3[v] -> (),
        0x0606_e19e => sgl::sl_special_mode_high[v] -> (),
        0x0607_147c => sgl::sl_isqrt[v] -> u32,
        0x0607_14a8 => sgl::sl_sqrt[v] -> u32,
        0x0607_14bc => sgl::sl_div_fx[a, b] -> u32,
        0x0606_eadc => sgl::sl_tan[a] -> u32,
        0x0607_1d2c => sgl::sl_inverse[] -> (),
        0x0606_e4dc => sgl::sl_invers_matrix[] -> (),
        0x0606_98ec => sgl::sl_col_offset_a[r, g, b] -> (),
        0x0606_98fc => sgl::sl_col_offset_b[r, g, b] -> (),
        0x0607_1c0c => sgl::sl_sqrt64[hi, lo] -> u32,
        0x0606_993c => sgl::sl_look_at[cam, target, twist] -> (),
        0x0606_72ec => sgl::sl_rot_x[a] -> (),
        0x0606_736c => sgl::sl_rot_y[a] -> (),
        0x0606_726c => sgl::sl_rot_z[a] -> (),
        0x0606_72f6 => sgl::sl_rot_x_sc[s, c] -> (),
        0x0606_7376 => sgl::sl_rot_y_sc[s, c] -> (),
        0x0606_7276 => sgl::sl_rot_z_sc[s, c] -> (),
        0x0606_e23c => sgl::sl_bg_on[bits] -> (),
        0x0603_511a => sgl::sl_push_unit_matrix[] -> u32,
        0x0607_10b0 => sgl::sl_put_polygon_x[pd, light] -> u32,
        0x0607_20fc => sgl::turn_undone[] -> (),
        0x0607_212c => sgl::turn_vector[v, out] -> (),
        0x0607_0ff0 => sgl::gouraud_make[pd] -> (),
        0x0607_0fbc => sgl::sl_init_gouraud[tables, count, first, shades] -> (),
        0x0607_0f8c => sgl::sl_light[v] -> (),
        0x0606_e288 => sgl::sl_char_nbg0[kind, size] -> (),
        0x0606_e294 => sgl::sl_char_nbg1[kind, size] -> (),
        0x0606_e2a0 => sgl::sl_char_nbg2[kind, size] -> (),
        0x0606_e2b6 => sgl::sl_char_nbg3[kind, size] -> (),
        0x0606_e27c => sgl::sl_char_rbg0[kind, size] -> (),
        0x0606_e300 => sgl::sl_page_nbg0[cells, palette, kind] -> (),
        0x0606_e2e4 => sgl::sl_page_nbg1[cells, palette, kind] -> (),
        0x0606_e2ec => sgl::sl_page_nbg2[cells, palette, kind] -> (),
        0x0606_e2f4 => sgl::sl_page_nbg3[cells, palette, kind] -> (),
        0x0606_e2ce => sgl::sl_page_rbg0[cells, palette, kind] -> (),
        0x0607_1cde => sgl::rotation_bank[at, use_] -> (),
        0x0607_0c4c => sgl::sl_map_nbg0[a, b, c, d] -> (),
        0x0607_0d1c => sgl::sl_map_nbg1[a, b, c, d] -> (),
        0x0606_e3ac => sgl::sl_map_nbg2[a, b, c, d] -> (),
        0x0607_1c4c => sgl::sl_map_rbg0[a] -> (),
        0x0607_0c3c => sgl::sl_plane_nbg0[size] -> (),
        0x0607_0d0c => sgl::sl_plane_nbg1[size] -> (),
        0x0606_e39c => sgl::sl_plane_nbg2[size] -> (),
        0x0607_0cfc => sgl::sl_zoom_mode[screen, mode] -> (),
        0x0607_0dec => sgl::sl_back1_col_set[at, colour] -> (),
        0x0606_dd4c => sgl::vram_split[split] -> (),
        0x0606_dd5c => sgl::sl_scr_transparent[bits] -> (),
        0x0606_990c => sgl::sl_zdsp_level[level] -> u32,
        0x0606_98cc => sgl::dma0_busy[] -> u32,
        0x0607_1154 => sgl::sl_set_gouraud_color[rgb] -> (),
        0x0606_e1cc => sgl::sl_perspective[angle] -> (),
        0x0606_e50c => sgl::rotation_parameters[] -> u32,
        0x0606_dc8a => sgl::tv_mode[mode] -> u32,
        0x0606_705c => sgl::slave_setting_60[v] -> (),
        0x0606_7088 => sgl::slave_setting_68[v] -> (),
        0x0606_70ac => sgl::screen_size[width, height] -> (),
        0x0606_70fc => sgl::sprite_colour_mode[mode] -> (),
        0x0606_711c => sgl::sl_tv_on[] -> (),
        0x0606_7126 => sgl::sl_tv_off[] -> (),
        0x0607_215c => sgl::scu_copy[dst, src, bytes] -> u32,
        0x0607_10f6 => sgl::sl_gouraud_tbl_copy[] -> (),
        0x0606_dd6c => sgl::sl_scr_auto_disp[screens] -> u32,
        0x0606_dcec => sgl::frame_mode[on] -> u32,
        0x0607_14dc => sgl::put_polygons[pd, each] -> u32,
        0x0603_4dd4 => sgl::sl_random[] -> u32,
        0x0603_512e => sgl::sl_unit_matrix[m] -> u32,
        0x0603_5110 => sgl::sl_init_matrix[] -> u32,
        0x0603_3a00 => sgl::sl_int_function[f] -> (),
        0x0603_4a50 => sgl::sl_transfer_entry[src, dst, size] -> u32,
        0x0603_3a10 => sgl::sl_dma_copy[src, dst, size] -> (),
        0x0603_3aa0 => sgl::sl_dmax_copy[src, dst, size, mode] -> u32,
        0x0603_4cca => sgl::sl_synch[] -> u32,
        0x0603_4e4a => sgl::sound_flush[] -> (),
        0x0603_4df2 => sgl::pcm_step[] -> (),
        0x0603_32a0 => sgl::dma_level0_end[] -> (),
        0x0603_32b2 => sgl::dma_level1_end[] -> (),
        0x0603_32c4 => sgl::dma_level2_end[] -> (),
        0x0603_4c1a => sgl::smpc_interrupt[] -> (),
        0x0603_3b40 => sgl::smpc_lock[] -> u32,
        0x0603_3b4a => sgl::smpc_unlock[v] -> u32,
        0x0603_3ef6 => sgl::smpc_wanted[] -> u32,
        0x0603_3f02 => sgl::smpc_going[] -> u32,
        0x0603_3f2e => sgl::smpc_mark_going[] -> (),
        0x0603_3f68 => sgl::smpc_busy_count[] -> u32,
        0x0603_3b68 => sgl::smpc_port_setting_a[v] -> u32,
        0x0603_3b8e => sgl::smpc_port_setting_b[v] -> u32,
        0x0603_3c04 => sgl::smpc_port_bit_c0[v] -> u32,
        0x0603_3c34 => sgl::smpc_port_bit_c1[v] -> u32,
        0x0603_3c80 => sgl::smpc_port_bit_d0[v] -> u32,
        0x0603_3cb0 => sgl::smpc_port_bit_d1[v] -> u32,
        0x0603_3ce8 => sgl::smpc_command_entry[c] -> u32,
        0x0603_3dbc => sgl::peripherals_clear[] -> (),
        0x0603_3b58 => sgl::smpc_setting_a[] -> u32,
        0x0603_3b60 => sgl::smpc_setting_b[] -> u32,
        0x0603_3bf0 => sgl::smpc_setting_c0_clear[] -> u32,
        0x0603_3bfa => sgl::smpc_setting_c1_clear[] -> u32,
        0x0603_3c6c => sgl::smpc_setting_d0_clear[] -> u32,
        0x0603_3c76 => sgl::smpc_setting_d1_clear[] -> u32,
        0x0603_3d3c => sgl::smpc_ports_readable[] -> u32,
        0x0603_3d7e => sgl::smpc_any_port[] -> u32,
        0x0603_3d9c => sgl::smpc_port_modes[v] -> u32,
        0x0603_42ba => sgl::smpc_mode_bit[v] -> u32,
        0x0603_42fa => sgl::smpc_port1_mode[v] -> u32,
        0x0603_4338 => sgl::smpc_port2_mode[v] -> u32,
        0x0603_6a50 => sgl::intback_parameters[] -> (),
        0x0603_4258 => sgl::smpc_issue[entry, wait] -> u32,
        0x0603_44c4 => sgl::sl_request_command[c] -> u32,
        0x0603_4378 => sgl::smpc_request_plain[entry] -> u32,
        0x0603_43e8 => sgl::smpc_request_intback_status[entry] -> u32,
        0x0603_4420 => sgl::smpc_request_intback_pads[entry] -> u32,
        0x0603_6620 => sgl::pad_clear[data, record] -> (),
        0x0603_662a => sgl::pad_buttons[now, record] -> u32,
        0x0603_6950 => sgl::device_none[data, record] -> u32,
        0x0603_669c => sgl::device_pad[data, record] -> u32,
        0x0603_3f90 => sgl::parse_port[data_at, records_at] -> u32,
        0x0603_4048 => sgl::parse_ports[] -> (),
        0x0603_4120 => sgl::smpc_frame_in[] -> (),
        0x0603_41d4 => sgl::smpc_frame_out[] -> (),
        0x0603_3404 => sgl::dma1_copy[src, dst, words] -> (),
        0x0603_32d6 => sgl::vblank_in[] -> (),
        0x0603_3430 => sgl::vblank_out[] -> (),
        0x0603_3590 => sgl::synch_init[mode, frames] -> (),
        0x0603_3dd4 => sgl::smpc_init[] -> (),
        0x0603_61c0 => sgl::slave_call[] -> (),
        0x0603_6254 => sgl::slave_zdsp_level[] -> (),
        0x0603_6268 => sgl::slave_perspective[] -> (),
        0x0603_627a => sgl::slave_light[] -> (),
        0x0603_6286 => sgl::slave_keep_60[] -> (),
        0x0603_628a => sgl::slave_keep_64[] -> (),
        0x0603_628e => sgl::slave_keep_68[] -> (),
        0x0603_6430 => sgl::slave_window[] -> (),
        0x0603_6412 => sgl::slave_frame_end[] -> (),
        0x0603_8330 => sgl::slave_quads[] -> (),
        0x0603_7f0e => sgl::slave_sprite[] -> (),
        0x0603_7f74 => sgl::slave_turned_sprite[] -> slave,
        0x0603_6dd8 => sgl::slave_polygons[] -> slave,
        0x0603_6ab0 => sgl::slave_frame_draw[] -> (),
        0x0603_6292 => sgl::slave_screen[] -> (),
        0x0603_61ec => sgl::slave_clear_pairs[] -> (),
        0x0603_61fe => sgl::slave_shade_tables[] -> (),
        0x0603_64e0 => sgl::draw_init[] -> (),
        0x0603_3658 => sgl::pcm_streams[] -> (),
        0x0603_3884 => sgl::pcm_copy[] -> (),
        0x0603_38da => sgl::pcm_wait[] -> (),
        0x0603_5540 => sgl::vdp_init[] -> (),
        0x0603_5660 => sgl::font_cells[] -> (),
        0x0603_5690 => sgl::font_cells_wide[] -> (),
        0x0603_56c0 => sgl::rotation_table_init[] -> (),
        0x0603_5724 => sgl::dma0_copy_words[] -> (),
        0x0603_59c0 => sgl::default_colours[] -> (),
        0x0600_7ef0 => sgl::sl_init_system[mode, textures, frames] -> (),
        0x0600_7e8c => sgl::sound_restart[] -> (),
        0x0600_7e70 => sgl::fill_vectors[at, v, mask] -> (),
        0x0603_5090 => sgl::slave_start[] -> u32,
        0x0603_4cb0 => sgl::synch_first[] -> u32,
        0x0603_50b0 => sgl::slave_main[] -> (),
        0x0603_6110 => sgl::slave_commands[] -> (),
        0x0600_4000 => start::start[] -> (),
        0x0600_7e50 => start::restart[f] -> (),
        0x0600_4070 => start::top_level[] -> (),
        0x0600_4288 => start::clear_state[] -> (),
        0x0600_4304 => start::reset_counters[] -> (),
        0x0600_4340 => start::run_game[] -> (),
        0x0600_6978 => start::protection_read[buf] -> (),
        0x0600_8100 => start::gcc_main[] -> (),
        0x0600_80bc => start::global_constructors[] -> (),
        0x0600_826c => clib::atexit[f] -> u32,
        0x0600_6ba0 => sound::snd_wait[] -> (),
        0x0600_6be4 => sound::snd_init[init] -> (),
        0x0600_6d7c => sound::snd_change_map[map] -> u32,
        0x0600_6e0c => sound::snd_command_82[v] -> u32,
        0x0600_7cd4 => sound::snd_clear[at, n] -> (),
        0x0600_7da0 => sound::snd_copy[dst, src, n] -> (),
        0x0600_7db8 => sound::snd_next_slot[] -> u32,
        0x0606_9a0c => math::sl_sin[a] -> u32,
        0x0606_9a30 => math::sl_cos[a] -> u32,
        0x0606_e26c => math::sl_mul_fx[a, b] -> u32,
        0x0606_4eac => math::turn_point[t] -> (),
        0x0606_29fc => models::spinner_draw[s] -> (),
        0x0606_2aa0 => models::spinner_turn[o] -> (),
        0x0606_2af8 => models::spinner_init[o] -> (),
        0x0606_2940 => actors::puffs_around[centre, angle, points, count] -> (),
        0x0604_0ccc => aim::target[obj, delay, out] -> u32,
        0x0604_0df0 => aim::aim[obj, delay] -> u32,
        0x0604_5df8 => aim::free_number[from] -> u32,
        0x0604_5fd8 => aim::nearest[obj, out] -> u32,
        0x0604_03c4 => sprites::draw_sprite[s] -> (),
        0x0604_030c => sprites::draw_model[s] -> (),
        0x0604_0960 => sprites::draw[s] -> (),
        0x0604_0604 => sprites::draw_distorted[s] -> (),
        0x0604_6138 => actors::end_by_flags[actor] -> u32,
        0x0604_6168 => actors::approach[from, to, step] -> u32,
        0x0604_dcb4 => actors::plain_exec[actor] -> (),
        0x0604_d600 => actors::numbered_exec[actor] -> (),
        0x0604_6264 => sprites::sprite_init[s] -> (),
        0x0605_6554 => aim::turn_sign[a, b] -> u32,
        0x0605_6580 => aim::angle_between[a, b] -> u32,
        0x0605_6644 => models::pose_step[p] -> (),
        0x0605_65f0 => models::pose_set[p, angles] -> (),
        0x0606_28cc => actors::puff_at[p] -> (),
        0x0605_69f8 => aim::area_flags[s, top, bottom, left; right] -> u32,
        0x0605_6a24 => aim::off_screen[s] -> u32,
        0x0605_70d8 => aim::turn_toward[angle, speed, from, to; most, ease, accel] -> (),
        0x0605_73a0 => actors::anim_step[actor] -> u32,
        0x0605_73f0 => actors::drift[actor] -> (),
        0x0605_6cc8 => parts::part_exec[part] -> (),
        0x0605_6dbc => parts::part_set[kind] -> u32,
        0x0605_6114 => parts::part_look[o, colour] -> (),
        0x0605_6c20 => parts::body_set[] -> u32,
        0x0605_6ea4 => parts::parts_set[count, points, state, life; parts, number] -> (),
        0x0606_32b0 => models::model_gouraud[kind, frame] -> (),
        0x0606_2f74 => gouraud::models_hss[kind, count] -> (),
        0x0606_2fe8 => gouraud::light_turned[x, y, z] -> (),
        0x0606_3060 => gouraud::table_set[colours] -> (),
        0x0606_30e0 => gouraud::table_floor[r, g, b] -> (),
        0x0606_3168 => gouraud::table_ramp[r, g, b] -> (),
        0x0606_320c => gouraud::shading_default[] -> (),
        0x0606_3248 => gouraud::shading_init[] -> (),
        0x0606_3294 => gouraud::shading_end[] -> (),
        0x0606_3380 => gouraud::models_lit[kind, count] -> (),
        0x0606_3428 => gouraud::model_sort[kind, frame, sort] -> (),
        0x0606_34e0 => gouraud::point_normals[pd, out] -> (),
        0x0606_35e8 => gouraud::gouraud_tables[pd, at] -> (),
        0x0605_e6fc => screen::set_offset_a[r, g, b] -> (),
        0x0605_e648 => screen::scroll_next[] -> (),
        0x0605_e630 => screen::screen_alt[] -> (),
        0x0605_e27c => screen::play_screen_set[] -> (),
        0x0605_e34c => screen::screen_reset[] -> (),
        0x0605_e3e0 => screen::screen_start[table, a, b] -> (),
        0x0605_fe8c => screen::picture_screen_set[] -> (),
        0x0605_fb00 => screen::pictures_load[p] -> u32,
        0x0605_ea18 => screen::text_plane_set[] -> (),
        0x0606_0260 => text::fonts_load[p] -> u32,
        0x0606_165c => kinds::demo_load[p] -> u32,
        0x0606_001c => screen::picture_load[p, bytes, count] -> (),
        0x0606_00f0 => screen::planes_ab[] -> (),
        0x0606_00d4 => screen::plane_a[] -> (),
        0x0604_5af8 => kinds::kind_colour_bank[k] -> u32,
        0x0605_64e0 => banner::banner_start[] -> u32,
        0x0605_fc38 => banner::banner_load[] -> (),
        0x0605_e17c => states::stage_data_set[p] -> (),
        0x0605_fe78 => states::stage_table[i] -> u32,
        0x0606_333c => models::model_lit[kind, frame] -> (),
        0x0604_dd50 => effects::actor_init[actor] -> (),
        0x0604_6530 => effects::link_init[link, to] -> (),
        0x0604_e1cc => effects::effect_set[fx] -> u32,
        0x0604_6558 => effects::link_step[s, link] -> (),
        0x0604_6284 => hits::hit_init[h] -> (),
        0x0605_6440 => hits::boxes_copy[to, from] -> (),
        0x0604_62ac => actors::actor_setup[actor] -> (),
        0x0604_816c => actors::player_set[st] -> u32,
        0x0604_deb4 => effects::effect_exec[obj] -> (),
        0x0604_d780 => actors::actor_exec[actor] -> (),
        0x0604_3d38 => hits::hits_exec[hits] -> (),
        0x0604_5d78 => math::random_seed[seed] -> (),
        0x0604_5d90 => math::random[n] -> u32,
        0x0604_5dcc => math::by_difficulty[x] -> u32,
        0x0606_14b0 => text::typewriter_set[s, at, flags, blink] -> u32,
        0x0605_975c => text::opening_exec[o] -> (),
        0x0605_9920 => text::opening_bar_draw[s] -> (),
        0x0605_99c8 => text::opening_set[l1, l2, l3, l4; wait] -> (),
        0x0606_131c => text::typewriter_exec[obj] -> (),
        0x0606_1504 => text::message_exec[obj] -> (),
        0x0606_1620 => text::message_set[lines] -> (),
        0x0606_12bc => text::stop_text_objects[] -> (),
        0x0605_fe34 => hud::put_digits[n, digits, at, p] -> (),
        0x0606_0388 => hud::status_line[p, line] -> (),
        0x0606_0550 => hud::gauge[p] -> (),
        0x0606_0354 => hud::status_reset[p] -> (),
        0x0606_0a0c => hud::draw_status[p] -> (),
        0x0600_4378 => system::count_vblank[] -> (),
        0x0604_0f44 => system::coin_byte3[slot] -> u32,
        0x0604_0f5c => system::use_credit[slot] -> u32,
        0x0604_0f74 => system::credits[slot] -> u32,
        0x0604_0fac => system::coin_settings_low[] -> u32,
        0x0603_3a6a => system::purge_cache[] -> (),
        0x0603_64a0 => system::purge_cache[] -> (),
        0x0606_e24c => system::wait_dma0[] -> (),
        0x0603_5070 => system::set_interrupt_mask[mask] -> (),
        0x0600_136a => system::next_unless_end[n] -> u32,
    }
    // TITLE.BIN's code (the logo mode's)
    rewrites! { n; overlay "TITLE";
        0x0608_0000 => logos::logo_mode[game] -> u32,
        0x0608_0054 => logos::logos_exec[obj] -> (),
        0x0608_04a0 => logos::logos_set[] -> (),
    }
    // OPENING.BIN's code (the opening mode's), with its copies of SGL's NBG3 and sprite setters
    rewrites! { n; overlay "OPENING";
        0x0608_0000 => opening::opening_mode[game] -> u32,
        0x0608_0328 => opening::opening_start[data, room] -> (),
        0x0608_03b4 => modes::mode_wait[] -> (),
        0x0608_03e0 => opening::text_cell[x, y] -> u32,
        0x0608_083c => opening::typer_exec[obj] -> (),
        0x0608_099c => opening::typer_set[text, at, width, blinks] -> (),
        0x0608_0a20 => opening::story_exec[obj] -> (),
        0x0608_0b60 => opening::story_set[] -> (),
        0x0608_0b90 => opening::pictures_start[work] -> (),
        0x0608_0d90 => opening::picture_put[work, p] -> (),
        0x0608_0e6c => opening::pictures_exec[work] -> (),
        0x0608_1164 => opening::opening_end[] -> (),
        0x0608_1184 => opening::opening_idle[] -> (),
        0x0608_1190 => sgl::sl_sprite_cc_condition[condition] -> (),
        0x0608_11b0 => sgl::sl_sprite_cc_number[n] -> (),
        0x0608_11c0 => sgl::sl_scr_move_nbg3[dx, dy] -> (),
        0x0608_11e0 => sgl::sl_plane_nbg3[size] -> (),
        0x0608_11f0 => sgl::sl_map_nbg3[a, b, c, d] -> (),
        0x0608_12b0 => sgl::sl_scr_pos_nbg3[x, y] -> (),
    }
    // STG00.BIN's code (the first stage mode's), with its copies of SGL's rotation scroll set-up
    rewrites! { n; overlay "STG00";
        0x0608_0000 => stg00::stg00_mode[game] -> u32,
        0x0608_0244 => stg00::cycling_behaviour[actor] -> (),
        0x0608_029c => stg00::cycling_set[] -> u32,
        0x0608_04b4 => stg00::ground_row[event] -> u32,
        0x0608_064c => stg00::wounded_shooting[o] -> (),
        0x0608_0720 => stg00::wounded_event[event] -> u32,
        0x0608_0828 => stg00::stg00_start[data, room] -> (),
        0x0608_0ba4 => modes::mode_wait[] -> (),
        0x0608_0bd0 => stg00::countdown_start[work] -> (),
        0x0608_0c48 => stg00::plane_b_scroll[] -> (),
        0x0608_0c74 => stg00::countdown_show[work] -> (),
        0x0608_0d54 => stg00::skip_text[work] -> (),
        0x0608_0d9c => stg00::countdown_exec[work] -> (),
        0x0608_0e48 => stg00::stg00_end[] -> (),
        0x0608_0e74 => stg00::stg00_idle[] -> (),
        0x0608_0e94 => sgl::sl_rpara_init_set[tables] -> (),
        0x0608_0ec8 => sgl::rpara_init[] -> (),
        0x0608_0f04 => sgl::sl_make_ktable[table] -> (),
        0x0608_0f84 => sgl::sl_plane_ra[size] -> (),
        0x0608_0f94 => sgl::sl_over_ra[mode] -> (),
        0x0608_0fa4 => sgl::sl_ktable_ra[table, mode] -> (),
        0x0608_1024 => sgl::sl_rpara_mode[mode] -> (),
        0x0608_1034 => sgl::sl_cur_rpara[b] -> (),
        0x0608_1044 => sgl::sl_plane_rb[size] -> (),
        0x0608_1054 => sgl::sl_map_rb[a] -> (),
        0x0608_10f4 => sgl::sl_over_rb[mode] -> (),
        0x0608_1104 => sgl::sl_ktable_rb[table, mode] -> (),
        0x0608_1164 => sgl::sl_sprite_type[t] -> (),
        0x0608_1174 => sgl::sl_sprite_cc_condition[condition] -> (),
        0x0608_1194 => sgl::sl_sprite_cc_number[n] -> (),
    }
    // STG02.BIN's code (stage 2's)
    rewrites! { n; overlay "STG02";
        0x0608_0000 => stg02::stg02_mode[game] -> u32,
        0x0608_0450 => stg02::model_attrs_set[k, m] -> (),
        0x0608_04d0 => stg02::kind_attrs_set[k] -> (),
        0x0608_051c => stg02::model_tilt_angle[o] -> u32,
        0x0608_054c => stg02::model_screen_angle[o] -> u32,
        0x0608_057c => stg02::line_effect[o, length] -> (),
        0x0608_0610 => stg02::burst[o, size] -> u32,
        0x0608_06b4 => stg02::destroyed_hook[o] -> u32,
        0x0608_0c20 => stg02::aim_line_behaviour[o] -> (),
        0x0608_0d88 => stg02::circling_missile_behaviour[o] -> (),
        0x0608_1124 => stg02::core_behaviour[o] -> (),
        0x0608_1a00 => stg02::rocket_behaviour[o] -> (),
        0x0608_1bc8 => stg02::launcher_behaviour[o] -> (),
        0x0608_1f80 => stg02::laser_behaviour[o] -> (),
        0x0608_22c4 => stg02::laser_gun_behaviour[o] -> (),
        0x0608_260c => stg02::pod_behaviour[o] -> (),
        0x0608_2814 => stg02::fin_7_behaviour[o] -> (),
        0x0608_2970 => stg02::fin_8_behaviour[o] -> (),
        0x0608_2ad4 => stg02::fin_9_behaviour[o] -> (),
        0x0608_2c38 => stg02::spinner_behaviour[o] -> (),
        0x0608_2f1c => stg02::wing_behaviour[o] -> (),
        0x0608_30c8 => stg02::hatch_behaviour[o] -> (),
        0x0608_349c => stg02::hatch_pod_behaviour[o] -> (),
        0x0608_37dc => stg02::outline_line_behaviour[o] -> (),
        0x0608_3b7c => stg02::outline_spark_behaviour[o] -> (),
        0x0608_3e14 => stg02::tracer_behaviour[o] -> (),
        0x0608_4f30 => stg02::heart_behaviour[o] -> (),
        0x0608_590c => stg02::tracers_set[o] -> (),
        0x0608_5974 => stg02::boss_decide[o] -> (),
        0x0608_5a44 => stg02::boss_move[o] -> (),
        0x0608_5d3c => stg02::boss_gun_b_attack[o] -> (),
        0x0608_5db0 => stg02::boss_gun_c_attack[o] -> (),
        0x0608_5e24 => stg02::boss_rise[o] -> (),
        0x0608_6038 => stg02::boss_rise_missiles[o] -> (),
        0x0608_6250 => stg02::boss_behaviour[o] -> (),
        0x0608_66b8 => stg02::boss_set[] -> u32,
        0x0608_67cc => stg02::row_arc[event] -> (),
        0x0608_6844 => stg02::row_rekind[event] -> (),
        0x0608_6860 => stg02::enemy_row[event] -> u32,
        0x0608_6d3c => stg02::spiral_flier_behaviour[o] -> (),
        0x0608_6f00 => stg02::spiral_flier_set[d] -> (),
        0x0608_701c => stg02::arc_spark_behaviour[o] -> (),
        0x0608_7258 => stg02::arc_spark_set[owner, from, to, index; mask] -> (),
        0x0608_7350 => stg02::arc_a_set[o] -> (),
        0x0608_73a0 => stg02::arc_b_set[o] -> (),
        0x0608_73ec => stg02::great_ship_turns[o] -> (),
        0x0608_751c => stg02::great_ship_come[o] -> (),
        0x0608_758c => stg02::great_ship_open[o] -> (),
        0x0608_760c => stg02::great_ship_fight[o] -> (),
        0x0608_7940 => stg02::great_ship_leave[o] -> (),
        0x0608_79a4 => stg02::great_ship_step[o] -> (),
        0x0608_79e4 => stg02::great_ship_draw[s] -> (),
        0x0608_8018 => stg02::great_ship_set[o] -> (),
        0x0608_8270 => stg02::great_ship_make[] -> u32,
        0x0608_82e4 => stg02::mine_behaviour[o] -> (),
        0x0608_848c => stg02::mine_set[d] -> (),
        0x0608_85d0 => stg02::mine_throw[o] -> (),
        0x0608_860c => stg02::mine_layer_salvos[o] -> (),
        0x0608_86e4 => stg02::mine_layer_behaviour[o] -> (),
        0x0608_8a2c => stg02::mine_layer_draw[s] -> (),
        0x0608_8b04 => stg02::layer_gun_behaviour[o] -> (),
        0x0608_8e18 => stg02::layer_gun_set[d, layer] -> u32,
        0x0608_8f80 => stg02::mine_layer_event[e] -> u32,
        0x0608_90dc => stg02::ring_carrier_behaviour[o] -> (),
        0x0608_9228 => stg02::jet_flame_behaviour[o] -> (),
        0x0608_9334 => stg02::jet_flame_set[d, owner, reach_at] -> (),
        0x0608_9498 => stg02::blast_behaviour[o] -> (),
        0x0608_95bc => stg02::blast_set[number, turn, flags_at, place_at; angle_at] -> (),
        0x0608_96c8 => stg02::pulse_shot[o, angle] -> (),
        0x0608_9734 => stg02::push_beam_behaviour[o] -> (),
        0x0608_97d4 => stg02::push_beam_draw[s] -> (),
        0x0608_98cc => stg02::push_beam_set[number, flags_at, place_at] -> (),
        0x0608_998c => stg02::snake_steer[o] -> (),
        0x0608_9ac0 => stg02::snake_piece_end[o] -> (),
        0x0608_9b70 => stg02::snake_head_behaviour[o] -> (),
        0x0608_9ca0 => stg02::snake_body_behaviour[o] -> (),
        0x0608_9d2c => stg02::snake_set[e] -> (),
        0x0608_9f54 => stg02::fan_shots[o] -> (),
        0x0608_a07c => stg02::mb_dive[o] -> (),
        0x0608_a104 => stg02::mb_to_wander[o] -> (),
        0x0608_a12c => stg02::mb_open_one[o] -> (),
        0x0608_a184 => stg02::mb_jet[o] -> (),
        0x0608_a324 => stg02::mb_to_left[o] -> (),
        0x0608_a34c => stg02::mb_to_right[o] -> (),
        0x0608_a374 => stg02::mb_volleys_start[o] -> (),
        0x0608_a3b4 => stg02::mb_gun_back[o] -> (),
        0x0608_a3f0 => stg02::mb_gun_aim[o] -> (),
        0x0608_a454 => stg02::mb_stays[o] -> u32,
        0x0608_a4e8 => stg02::mb_next_action[o] -> (),
        0x0608_a5cc => stg02::mb_rising[o] -> (),
        0x0608_a640 => stg02::mb_risen[o] -> (),
        0x0608_a694 => stg02::mb_idle[o] -> (),
        0x0608_a714 => stg02::mb_diving[o] -> (),
        0x0608_a794 => stg02::mb_sinking[o] -> (),
        0x0608_a97c => stg02::mb_stopping[o] -> (),
        0x0608_a9e4 => stg02::mb_wandering[o] -> (),
        0x0608_aa54 => stg02::mb_going_left[o] -> (),
        0x0608_aaac => stg02::mb_going_right[o] -> (),
        0x0608_ab04 => stg02::mb_halting[o] -> (),
        0x0608_ab50 => stg02::mb_volleys[o] -> (),
        0x0608_aca4 => stg02::mb_volleys_aim[o] -> (),
        0x0608_acf8 => stg02::mb_volleys_fire[o] -> (),
        0x0608_b150 => stg02::mb_one_opening[o] -> (),
        0x0608_b1d8 => stg02::mb_one_open[o] -> (),
        0x0608_b530 => stg02::mb_jet_lighting[o] -> (),
        0x0608_b654 => stg02::mb_circling[o] -> (),
        0x0608_b7e8 => stg02::mb_beaten[o] -> (),
        0x0608_b8c4 => stg02::mb_behaviour[o] -> (),
        0x0608_be60 => stg02::mb_draw[s] -> (),
        0x0608_ca20 => stg02::mb_set[o, n] -> (),
        0x0608_cdb0 => stg02::mb_make[] -> u32,
        0x0608_ce10 => stg02::flame_behaviour[o] -> (),
        0x0608_cee4 => stg02::flame_set[d, owner, flags_at] -> (),
        0x0608_d044 => stg02::spark_fade[o] -> u32,
        0x0608_d074 => stg02::spark_behaviour[o] -> (),
        0x0608_d0c4 => stg02::spark_set[p] -> (),
        0x0608_d1d8 => stg02::charge_grow[o] -> u32,
        0x0608_d268 => stg02::charge_behaviour[o] -> (),
        0x0608_d2d8 => stg02::charge_set[watched, mask, centre, angle_from] -> (),
        0x0608_d3bc => stg02::ss_flash[o] -> (),
        0x0608_d424 => stg02::fireball_behaviour[o] -> (),
        0x0608_d52c => stg02::fireball_set[d] -> (),
        0x0608_d684 => stg02::ss_fireball[o, angle] -> (),
        0x0608_d6c8 => stg02::ss_glow[o] -> (),
        0x0608_d74c => stg02::ss_fires_clear[o] -> (),
        0x0608_d770 => stg02::ss_coil[o] -> (),
        0x0608_d7bc => stg02::ss_breath[o] -> (),
        0x0608_d854 => stg02::ss_lunge[o] -> (),
        0x0608_d8a4 => stg02::ss_sweep[o] -> (),
        0x0608_d944 => stg02::ss_sweep_fast[o] -> (),
        0x0608_d9c0 => stg02::ss_dive_in[o] -> (),
        0x0608_dae4 => stg02::ss_next_action[o] -> (),
        0x0608_db7c => stg02::ss_tilt[o, to] -> (),
        0x0608_dbb8 => stg02::ss_neck[o, to] -> (),
        0x0608_dbf4 => stg02::ss_coming[o] -> (),
        0x0608_dd70 => stg02::ss_hovering[o] -> (),
        0x0608_dec0 => stg02::ss_lunging[o] -> (),
        0x0608_e14c => stg02::ss_sweeping[o] -> (),
        0x0608_e52c => stg02::ss_fast_sweeping[o] -> (),
        0x0608_e7c8 => stg02::ss_fireballs[o] -> (),
        0x0608_e9ac => stg02::ss_breathing[o] -> (),
        0x0608_eb9c => stg02::ss_breath_right[o] -> (),
        0x0608_ec20 => stg02::ss_breath_left[o] -> (),
        0x0608_eca4 => stg02::ss_coiling[o] -> (),
        0x0608_edd0 => stg02::ss_beaten[o] -> (),
        0x0608_edf0 => stg02::ss_behaviour[o] -> (),
        0x0608_f0bc => stg02::sg_glow[s] -> (),
        0x0608_f12c => stg02::sg_shot_behaviour[b] -> (),
        0x0608_f164 => stg02::sg_behaviour[s] -> (),
        0x0608_f510 => stg02::ss_draw[s] -> (),
        0x0608_f74c => stg02::sg_draw[sp] -> (),
        0x0608_f9d0 => stg02::ss_set[o, n] -> (),
        0x0608_fe24 => stg02::ss_make[] -> u32,
        0x0608_fe84 => stg02::thruster_behaviour[o] -> (),
        0x0608_febc => stg02::thruster_draw[s] -> (),
        0x0609_004c => stg02::thruster_set[owner, glows] -> (),
        0x0609_00e4 => stg02::escort_behaviour[o] -> (),
        0x0609_03ac => stg02::escort_draw[s] -> (),
        0x0609_0464 => stg02::escort_set[d] -> (),
        0x0609_0564 => stg02::spinner_pod_behaviour[o] -> (),
        0x0609_06f4 => stg02::spinner_pod_draw[s] -> (),
        0x0609_08b0 => stg02::spinner_pod_set[d] -> (),
        0x0609_0a2c => stg02::carrier_beam_behaviour[o] -> (),
        0x0609_0b6c => stg02::carrier_beam_draw[s] -> (),
        0x0609_0c7c => stg02::carrier_beam_set[carrier, gun] -> (),
        0x0609_0d8c => stg02::spark_shot_behaviour[o] -> (),
        0x0609_0e60 => stg02::spark_shot_set[d] -> (),
        0x0609_0f8c => stg02::flare_behaviour[o] -> (),
        0x0609_1010 => stg02::flare_draw[s] -> (),
        0x0609_10dc => stg02::flare_set[d] -> (),
        0x0609_11d4 => stg02::big_charge_trail_fade[o] -> u32,
        0x0609_1210 => stg02::big_charge_trail_behaviour[o] -> (),
        0x0609_1268 => stg02::big_charge_trail_set[p] -> (),
        0x0609_1384 => stg02::big_charge_grow[o] -> u32,
        0x0609_1428 => stg02::big_charge_behaviour[o] -> (),
        0x0609_1494 => stg02::big_charge_set[watched, mask, centre, angle_from; colours] -> (),
        0x0609_157c => stg02::barrier_place[o] -> (),
        0x0609_15d4 => stg02::barrier_behaviour[o] -> (),
        0x0609_1604 => stg02::barrier_set[on_at, points, size_at] -> (),
        0x0609_16c8 => stg02::cross_flash_at[p, colours, angle] -> (),
        0x0609_1710 => stg02::lunge_shot_behaviour[o] -> (),
        0x0609_189c => stg02::lunge_shot_set[d, owner] -> u32,
        0x0609_1a78 => stg02::bomb_glow_behaviour[g] -> (),
        0x0609_1aa0 => stg02::bomb_behaviour[o] -> (),
        0x0609_1c90 => stg02::bomb_set[d] -> (),
        0x0609_1d68 => stg02::ground_pod_behaviour[o] -> (),
        0x0609_1dc0 => stg02::ground_pod_set[p] -> (),
        0x0609_1eb0 => stg02::hover_mine_behaviour[o] -> (),
        0x0609_1ef8 => stg02::hover_mine_draw[s] -> (),
        0x0609_1f80 => stg02::hover_mine_set[x, y, carrier, drops] -> (),
        0x0609_20d4 => stg02::carrier_target[o] -> u32,
        0x0609_20ec => stg02::carrier_to_5[o] -> (),
        0x0609_2130 => stg02::carrier_hum[o, v] -> (),
        0x0609_2154 => stg02::carrier_lunge_fan[o] -> (),
        0x0609_21ec => stg02::carrier_events[o] -> (),
        0x0609_2878 => stg02::carrier_turns[o] -> (),
        0x0609_28f8 => stg02::carrier_cruise[o] -> (),
        0x0609_29b0 => stg02::carrier_rising[o] -> (),
        0x0609_2ac0 => stg02::carrier_swinging[o] -> (),
        0x0609_2b94 => stg02::carrier_settling[o] -> (),
        0x0609_2c0c => stg02::carrier_guarding[o] -> (),
        0x0609_2fcc => stg02::carrier_diving[o] -> (),
        0x0609_30bc => stg02::carrier_fighting[o] -> (),
        0x0609_34b4 => stg02::carrier_beaten[o] -> (),
        0x0609_3524 => stg02::carrier_results[o] -> (),
        0x0609_3594 => stg02::carrier_parts_open[o] -> (),
        0x0609_35e8 => stg02::carrier_parts_shut[o] -> (),
        0x0609_3654 => stg02::carrier_mirror_waiting[o] -> (),
        0x0609_36ec => stg02::carrier_mirroring[o] -> (),
        0x0609_3804 => stg02::carrier_mirror_leaving[o] -> (),
        0x0609_38c0 => stg02::carrier_last_waiting[o] -> (),
        0x0609_3974 => stg02::carrier_last_rising[o] -> (),
        0x0609_3a68 => stg02::carrier_last_settling[o] -> (),
        0x0609_3abc => stg02::carrier_last_next[o] -> (),
        0x0609_3c28 => stg02::carrier_last_hover[o] -> (),
        0x0609_3d34 => stg02::carrier_last_main[o] -> (),
        0x0609_3e88 => stg02::carrier_last_turning[o] -> (),
        0x0609_3ed4 => stg02::carrier_barrage[o] -> (),
        0x0609_41b4 => stg02::carrier_pods[o] -> (),
        0x0609_42fc => stg02::carrier_barriers[o] -> (),
        0x0609_46ac => stg02::carrier_sinking[o] -> (),
        0x0609_477c => stg02::carrier_behaviour[o] -> (),
        0x0609_4c50 => stg02::carrier_draw[s] -> (),
        0x0609_5e38 => stg02::carrier_lead_start[o] -> (),
        0x0609_5ef8 => stg02::carrier_tail_start[o] -> (),
        0x0609_5f20 => stg02::carrier_parts_set[o] -> (),
        0x0609_6114 => stg02::carrier_set[o, partner, tail] -> (),
        0x0609_6278 => stg02::carriers_make[] -> u32,
        0x0609_6378 => stg02::opening_event[e] -> u32,
        0x0609_63cc => stg02::sound_load_event[e, n] -> u32,
        0x0609_6490 => stg02::sound_load_32[e] -> u32,
        0x0609_64a8 => stg02::sound_load_33[e] -> u32,
        0x0609_64c0 => stg02::sound_load_34[e] -> u32,
        0x0609_64d8 => stg02::sound_load_35[e] -> u32,
        0x0609_64f0 => stg02::sound_load_36[e] -> u32,
        0x0609_6508 => stg02::boss_music_event[e] -> u32,
        0x0609_6724 => stg02::boss_kinds_event[e] -> u32,
        0x0609_6934 => stg02::mid_boss_event[e] -> u32,
        0x0609_6a04 => stg02::carrier_kinds_event[e] -> u32,
        0x0609_6c14 => stg02::carrier_event[e] -> u32,
        0x0609_6cfc => stg02::scenery_kinds_event[e] -> u32,
        0x0609_6f1c => stg02::great_ship_event[e] -> u32,
        0x0609_6f34 => stg02::serpent_event[e] -> u32,
        0x0609_7004 => stg02::last_kinds_event[e] -> u32,
        0x0609_71e4 => stg02::last_boss_event[e] -> u32,
        0x0609_72b4 => stg02::ground_moves_event[e] -> u32,
        0x0609_72e8 => stg02::late_kinds_event[e] -> u32,
        0x0609_7628 => stg02::last_boss_behaviour[o] -> (),
        0x0609_76b4 => stg02::last_part_behaviour[o] -> (),
        0x0609_7950 => stg02::last_part_slowing[o] -> (),
        0x0609_7970 => stg02::last_part_next[o] -> (),
        0x0609_79fc => stg02::last_part_turning[o] -> (),
        0x0609_7ab8 => stg02::last_part_spin_on[o] -> (),
        0x0609_7b68 => stg02::last_part_shots[o, glow] -> (),
        0x0609_7ccc => stg02::last_part_volley[o] -> (),
        0x0609_7d24 => stg02::last_part_fan_rocket[o, k, turn] -> (),
        0x0609_7dd8 => stg02::last_part_side_rocket[o, k, turn] -> (),
        0x0609_7e8c => stg02::last_part_side_bullet[o, k, turn] -> (),
        0x0609_7f40 => stg02::last_part_fan[o] -> (),
        0x0609_7fc0 => stg02::last_part_sweep[o] -> (),
        0x0609_8068 => stg02::last_part_sweep_again[o] -> (),
        0x0609_8080 => stg02::last_part_side_rockets[o] -> (),
        0x0609_8118 => stg02::last_part_sweep_third[o] -> (),
        0x0609_8130 => stg02::off_field[p] -> u32,
        0x0609_8174 => stg02::bouncing_shot_behaviour[o] -> (),
        0x0609_8224 => stg02::ring_shot_behaviour[o] -> (),
        0x0609_8324 => stg02::last_shot_make[d] -> u32,
        0x0609_841c => stg02::bouncing_shot_set[d] -> u32,
        0x0609_8440 => stg02::ring_shot_set[d] -> u32,
        0x0609_8464 => stg02::last_boss_fighting[o] -> (),
        0x0609_870c => stg02::last_script_next[o] -> (),
        0x0609_879c => stg02::last_boss_matrix[o] -> (),
        0x0609_87e4 => stg02::last_pose_blend[o, a, b, c; d] -> (),
        0x0609_8934 => stg02::last_pose_first[o] -> (),
        0x0609_8968 => stg02::last_pose_second[o] -> (),
        0x0609_899c => stg02::last_pose_third[o] -> (),
        0x0609_89d0 => stg02::last_pose_fourth[o] -> (),
        0x0609_8a30 => stg02::last_boss_missile[o] -> (),
        0x0609_8aac => stg02::last_ribbon_behaviour[o] -> (),
        0x0609_8b74 => stg02::last_ribbon_make[d, bounces] -> u32,
        0x0609_8bd0 => stg02::last_arm_ribbon[o, heading] -> (),
        0x0609_8cb4 => stg02::last_arm_shots[o, i] -> (),
        0x0609_8df0 => stg02::last_arms_fire[o] -> (),
        0x0609_8e54 => stg02::last_arms_volley[o] -> (),
        0x0609_8e8c => stg02::last_arm_behaviour[o] -> (),
        0x0609_9024 => stg02::last_arm_draw[o] -> (),
        0x0609_9030 => stg02::last_arm_make[boss, i] -> u32,
        0x0609_918c => stg02::last_follower_behaviour[o] -> (),
        0x0609_935c => stg02::last_arm_turn[i] -> u32,
        0x0609_9230 => stg02::last_follower_make[boss, i, hits, turn] -> u32,
        0x0609_93a0 => stg02::last_ring_place[o] -> (),
        0x0609_941c => stg02::last_ring_behaviour[o] -> (),
        0x0609_95b0 => stg02::last_mark_behaviour[o] -> (),
        0x0609_96a4 => stg02::last_ring_set[arm] -> (),
        0x0609_97d4 => stg02::last_mark_make[ring] -> (),
        0x0609_98f4 => stg02::last_flash_ring[p] -> (),
        0x0609_997c => stg02::last_flash_behaviour[o] -> (),
        0x0609_9ba0 => stg02::last_ring_flash_make[ring] -> (),
    }
    // STG01.BIN's code (stage 3's)
    rewrites! { n; overlay "STG01";
        0x0608_0000 => stg01::stg01_mode[game] -> u32,
        0x0608_0478 => stg01::scenery_point[p] -> (),
        0x0608_04e0 => stg01::scenery_model_draw[s] -> (),
        0x0608_05c0 => stg01::debris_behaviour[actor] -> (),
        0x0608_08b0 => stg01::debris_set[parent] -> u32,
        0x0608_09a0 => stg01::falling_polygon[obj] -> (),
        0x0608_0a0c => stg01::easing_polygon[obj] -> (),
        0x0608_0a78 => stg01::riser_behaviour[obj] -> (),
        0x0608_1024 => stg01::smoking_part[obj] -> (),
        0x0608_11a0 => stg01::flashing_part[obj] -> (),
        0x0608_11d4 => stg01::spinning_draw[s] -> (),
        0x0608_12cc => stg01::beam_behaviour[obj] -> (),
        0x0608_15b8 => stg01::beam_set[parent] -> u32,
        0x0608_1648 => stg01::turret_behaviour[obj] -> (),
        0x0608_1d60 => stg01::twin_turret_behaviour[obj] -> (),
        0x0608_273c => stg01::laser_glow[obj] -> (),
        0x0608_28d8 => stg01::laser_behaviour[obj] -> (),
        0x0608_2bbc => stg01::laser_set[parent, y] -> u32,
        0x0608_2cbc => stg01::laser_cannon_behaviour[obj] -> (),
        0x0608_36f0 => stg01::spinner_behaviour[obj] -> (),
        0x0608_3954 => stg01::arc_draw[p] -> (),
        0x0608_3d68 => stg01::arc_square_behaviour[obj] -> (),
        0x0608_3f70 => stg01::aiming_gun_behaviour[obj] -> (),
        0x0608_5344 => stg01::burst_seed_behaviour[obj] -> (),
        0x0608_55e8 => stg01::tether_behaviour[obj] -> (),
        0x0608_57b4 => stg01::tethered_drone_behaviour[obj] -> (),
        0x0608_5c84 => stg01::drone_launcher_behaviour[obj] -> (),
        0x0608_60e8 => stg01::ring_part_behaviour[obj] -> (),
        0x0608_6198 => stg01::glow_behaviour[obj] -> (),
        0x0608_6208 => stg01::link_beam_behaviour[obj] -> (),
        0x0608_64fc => stg01::link_beam_set[parent] -> u32,
        0x0608_65c4 => stg01::laser_head_behaviour[obj] -> (),
        0x0608_6a6c => stg01::laser_head_set[parent, count] -> u32,
        0x0608_6b40 => stg01::head_launcher_behaviour[obj] -> (),
        0x0608_6e6c => stg01::boss_behaviour[obj] -> (),
        0x0608_7430 => stg01::boss_set[] -> u32,
        0x0608_7500 => stg01::deep_ground_exec[o] -> (),
        0x0608_7564 => stg01::deep_wounded_exec[o] -> (),
        0x0608_75cc => stg01::route_flier_behaviour[obj] -> (),
        0x0608_7678 => stg01::route_flier_set[d, script] -> u32,
        0x0608_7748 => stg01::row_arc[event] -> (),
        0x0608_7844 => stg01::row_retype[event] -> (),
        0x0608_78d4 => stg01::enemy_row[event] -> u32,
        0x0608_7d80 => stg01::sinking_craft_behaviour[o] -> (),
        0x0608_800c => stg01::craft_part_behaviour[part] -> (),
        0x0608_8258 => stg01::craft_gun_behaviour[obj] -> (),
        0x0608_84c8 => stg01::craft_gun_set[g, craft] -> u32,
        0x0608_862c => stg01::craft_set[d, script] -> u32,
        0x0608_898c => stg01::dropped_bomb_behaviour[o] -> (),
        0x0608_8a58 => stg01::dropped_bomb_set[p] -> (),
        0x0608_8b94 => stg01::bomber_behaviour[o] -> (),
        0x0608_8d7c => stg01::bomber_draw[s] -> (),
        0x0608_8e28 => stg01::bomber_set[e] -> u32,
        0x0608_8f14 => stg01::player_ahead[o] -> u32,
        0x0608_8f94 => stg01::rail_claw_behaviour[o] -> (),
        0x0608_92e4 => stg01::rail_claw_draw[s] -> (),
        0x0608_9450 => stg01::rail_claw_set[e] -> u32,
        0x0608_9628 => stg01::ring_spark_behaviour[o] -> (),
        0x0608_96f8 => stg01::ring_spark_set[p] -> (),
        0x0608_97f4 => stg01::ring_flare_behaviour[o] -> (),
        0x0608_98a0 => stg01::ring_flare_set[p] -> (),
        0x0608_996c => stg01::nearer_effect_behaviour[o] -> (),
        0x0608_99a4 => stg01::muzzle_flash[o, at] -> (),
        0x0608_9a2c => stg01::spray_gun[o] -> (),
        0x0608_9b74 => stg01::twin_guns[o] -> (),
        0x0608_9d04 => stg01::homing_rocket_behaviour[o] -> (),
        0x0608_9fd8 => stg01::homing_rocket_set[p] -> u32,
        0x0608_a17c => stg01::rising_rocket_behaviour[o] -> (),
        0x0608_a2a8 => stg01::rocket_launcher[o] -> (),
        0x0608_a3f8 => stg01::afterburn[o] -> (),
        0x0608_a53c => stg01::fireball_behaviour[o] -> (),
        0x0608_a684 => stg01::fireball_set[ship] -> u32,
        0x0608_a838 => stg01::charge_glow_behaviour[o] -> (),
        0x0608_a91c => stg01::charge_glow_set[ship] -> u32,
        0x0608_a9b8 => stg01::screen_18_shrink[] -> (),
        0x0608_a9e8 => stg01::screen_14_ease[to] -> (),
        0x0608_aa18 => stg01::mover_push[] -> (),
        0x0608_aadc => stg01::screen_18_depth[] -> u32,
        0x0608_ab14 => stg01::ship_turn_toward[o, to] -> (),
        0x0608_abb0 => stg01::ship_turn_from_player[o] -> (),
        0x0608_ac04 => stg01::ship_roll[o] -> (),
        0x0608_ac7c => stg01::ship_drift[o, dy] -> (),
        0x0608_ad78 => stg01::ship_drift_up[o] -> (),
        0x0608_ad90 => stg01::ship_come_in[o] -> (),
        0x0608_ae64 => stg01::ship_hold[o] -> (),
        0x0608_aef4 => stg01::ship_smoke[o] -> (),
        0x0608_b058 => stg01::ship_fight[o] -> (),
        0x0608_b10c => stg01::ship_charge[o] -> (),
        0x0608_b264 => stg01::ship_thrown_back[o] -> (),
        0x0608_b30c => stg01::ship_sink[o] -> (),
        0x0608_b490 => stg01::ship_gone[o] -> (),
        0x0608_b560 => stg01::ship_behaviour[o] -> (),
        0x0608_bb1c => stg01::ship_shadow[o] -> (),
        0x0608_bd3c => stg01::ship_draw[s] -> (),
        0x0608_c574 => stg01::ship_set[o, n] -> (),
        0x0608_c8b0 => stg01::ship_make[] -> u32,
        0x0608_c910 => stg01::passing_ship_behaviour[o] -> (),
        0x0608_c9e8 => stg01::passing_ship_draw[s] -> (),
        0x0608_cc4c => stg01::passing_ship_set[o, n] -> (),
        0x0608_cd38 => stg01::passing_ship_make[] -> u32,
        0x0608_cd98 => stg01::gun_glow[watched, angle_from, centre, offset] -> (),
        0x0608_ce2c => stg01::wb_volley[o] -> (),
        0x0608_cf78 => stg01::wb_missiles[o] -> (),
        0x0608_d0a8 => stg01::flame_behaviour[o] -> (),
        0x0608_d130 => stg01::flame_set[d] -> u32,
        0x0608_d274 => stg01::wb_flame[d] -> u32,
        0x0608_d2a0 => stg01::wb_flame_fan[o] -> (),
        0x0608_d3d4 => stg01::breath_glow_behaviour[o] -> (),
        0x0608_d5c4 => stg01::breath_glow_set[b, at] -> (),
        0x0608_d688 => stg01::breath_behaviour[o] -> (),
        0x0608_d7c0 => stg01::breath_set[arm] -> (),
        0x0608_d8d4 => stg01::wb_laser_box_behaviour[o] -> (),
        0x0608_d9c4 => stg01::wb_laser_box_set[laser] -> (),
        0x0608_da7c => stg01::wb_laser_behaviour[o] -> (),
        0x0608_dc7c => stg01::wb_laser_draw[s] -> (),
        0x0608_df9c => stg01::wb_laser_set[boss] -> (),
        0x0608_e044 => aim::turn_toward[angle, speed, from, to; most, ease, accel] -> (),
        0x0608_e100 => stg01::wb_turn_toward[rate, goal, from, to] -> (),
        0x0608_e190 => stg01::wb_turn_from_player[o] -> (),
        0x0608_e1e4 => stg01::wb_come_down[o] -> (),
        0x0608_e224 => stg01::wb_settle[o] -> (),
        0x0608_e300 => stg01::wb_wait[o] -> (),
        0x0608_e328 => stg01::wb_open[o] -> (),
        0x0608_e36c => stg01::wb_script[o] -> (),
        0x0608_e424 => stg01::wb_fight[o] -> (),
        0x0608_e5c4 => stg01::wb_laser_phase[o] -> (),
        0x0608_e698 => stg01::wb_downed[o] -> (),
        0x0608_e778 => stg01::arm_wait[o] -> (),
        0x0608_e798 => stg01::arm_shot_behaviour[o] -> (),
        0x0608_e7cc => stg01::arm_fight[o] -> (),
        0x0608_eb18 => stg01::arm_breathe[o] -> (),
        0x0608_eb94 => stg01::arm_downed[o] -> (),
        0x0608_ecd4 => stg01::wb_behaviour[o] -> (),
        0x0608_efb8 => stg01::arm_behaviour[o] -> (),
        0x0608_f15c => stg01::wb_draw[s] -> (),
        0x0608_f450 => stg01::arm_draw_0[s] -> (),
        0x0608_f554 => stg01::arm_draw_1[s] -> (),
        0x0608_f654 => stg01::arm_draw_2[s] -> (),
        0x0608_f754 => stg01::arm_draw_3[s] -> (),
        0x0608_f878 => stg01::wb_set[o, n] -> (),
        0x0608_fcf8 => stg01::wb_make[] -> u32,
        0x0608_fd58 => stg01::sprite_again[d, colour, grow] -> (),
        0x0608_fdd4 => stg01::touch_behaviour[o] -> (),
        0x0608_fe24 => stg01::touch_set[d, boxes] -> (),
        0x0608_fecc => stg01::trail_wave[o] -> (),
        0x0608_ffe4 => stg01::trail_behaviour[o] -> (),
        0x0609_00f0 => stg01::trail_draw[s] -> (),
        0x0609_0310 => stg01::trail_set[d] -> (),
        0x0609_049c => stg01::rebound_move[o] -> (),
        0x0609_05dc => stg01::rebound_behaviour[o] -> (),
        0x0609_06e4 => stg01::rebound_draw[s] -> (),
        0x0609_0e04 => stg01::rebound_set[d] -> (),
        0x0609_0f28 => stg01::dart_move[o] -> (),
        0x0609_109c => stg01::dart_behaviour[o] -> (),
        0x0609_10d8 => stg01::dart_set[d] -> (),
        0x0609_1684 => stg01::plume_behaviour[o] -> (),
        0x0609_1708 => stg01::plume_draw[s] -> (),
        0x0609_1834 => stg01::plume_set[d] -> (),
        0x0609_1eb4 => stg01::fb_shot[o] -> (),
        0x0609_1f1c => stg01::fb_spray[o] -> (),
        0x0609_2010 => stg01::fb_missiles[o] -> (),
        0x0609_20d0 => stg01::fb_glide[o, to] -> (),
        0x0609_2254 => stg01::fb_core_attack[o] -> (),
        0x0609_245c => stg01::fb_spark_desc[d] -> (),
        0x0609_24ec => stg01::fb_gun_attack[o] -> (),
        0x0609_2700 => stg01::fb_launch_right[o] -> (),
        0x0609_27d0 => stg01::fb_launch_left[o] -> (),
        0x0609_28a0 => stg01::fb_rise[o] -> (),
        0x0609_28cc => stg01::fb_fly[o] -> (),
        0x0609_2970 => stg01::fb_fly_in[o] -> (),
        0x0609_29c0 => stg01::fb_pull_up[o] -> (),
        0x0609_2a78 => stg01::fb_settle[o] -> (),
        0x0609_2b3c => stg01::fb_next_act[o] -> (),
        0x0609_2ca0 => stg01::fb_fight[o] -> (),
        0x0609_3298 => stg01::fb_lunge[o] -> (),
        0x0609_33bc => stg01::fb_attack[o] -> (),
        0x0609_34b8 => stg01::fb_downed[o] -> (),
        0x0609_35ec => stg01::gondola_lean[o] -> (),
        0x0609_3638 => stg01::gondola_ride[o] -> (),
        0x0609_3704 => stg01::gondola_drop[o] -> (),
        0x0609_3744 => stg01::gondola_hunt[o] -> (),
        0x0609_3868 => stg01::gondola_return[o] -> (),
        0x0609_394c => stg01::gondola_behaviour[o] -> (),
        0x0609_3a40 => stg01::wing_behaviour[o] -> (),
        0x0609_3c58 => stg01::fb_behaviour[o] -> (),
        0x0609_4030 => stg01::fb_draw[s] -> (),
        0x0609_47fc => stg01::gondola_draw[s] -> (),
        0x0609_493c => stg01::gondola_set[boss] -> (),
        0x0609_4a60 => stg01::wing_draw[s] -> (),
        0x0609_4aec => stg01::wing_set[boss, side] -> (),
        0x0609_4c2c => stg01::fb_set[o, n] -> (),
        0x0609_4fc0 => stg01::fb_make[] -> u32,
        0x0609_5020 => stg01::message_event[e] -> u32,
        0x0609_5074 => stg01::next_events_event[e] -> u32,
        0x0609_50e0 => stg01::sound_load_event[e, n] -> u32,
        0x0609_51a4 => stg01::sound_load_26[e] -> u32,
        0x0609_51bc => stg01::sound_load_27[e] -> u32,
        0x0609_51d4 => stg01::sound_load_28[e] -> u32,
        0x0609_51ec => stg01::sound_load_29[e] -> u32,
        0x0609_5204 => stg01::sound_load_30[e] -> u32,
        0x0609_521c => stg01::music_event[e] -> u32,
        0x0609_53d0 => stg01::scenery_kinds_event[e] -> u32,
        0x0609_55e0 => stg01::passing_ship_event[e] -> u32,
        0x0609_55f8 => stg01::ship_event[e] -> u32,
        0x0609_566c => stg01::area_change_event[e] -> u32,
        0x0609_59e4 => stg01::ground_motion_event[e] -> u32,
        0x0609_5a18 => stg01::wb_event[e] -> u32,
        0x0609_5a8c => stg01::area_2_change_event[e] -> u32,
        0x0609_5f20 => stg01::fb_event[e] -> u32,
        0x0609_5ff0 => stg01::ground_motion_3_event[e] -> u32,
        0x0609_6024 => stg01::area_3_kinds_event[e] -> u32,
        0x0609_61d0 => stg01::stage_boss_event[e] -> u32,
        0x0609_6294 => stg01::area_4_kinds_event[e] -> u32,
        0x0609_6578 => stg01::boss_event[e] -> u32,
        0x0609_6590 => stg01::stg01_start[data, room] -> (),
        0x0609_7a24 => stg01::last_scroll_start[] -> (),
        0x0609_7a6c => stg01::rumble_behaviour[o] -> (),
        0x0609_7b68 => stg01::leave_start[o] -> (),
        0x0609_7c00 => stg01::leave_wait[o, frames] -> u32,
        0x0609_7e28 => stg01::leave_2p[o] -> (),
        0x0609_80bc => stg01::leave_1p[o] -> (),
        0x0609_8360 => stg01::end_start[w] -> (),
        0x0609_847c => stg01::end_view_along[w] -> (),
        0x0609_84fc => stg01::end_steps[w] -> (),
        0x0609_8724 => stg01::sweep_behaviour[o] -> (),
        0x0609_88fc => stg01::clear_start[w] -> (),
        0x0609_89c4 => stg01::planes_slide[w] -> (),
        0x0609_8af8 => stg01::slide_easing[w] -> (),
        0x0609_8b24 => stg01::slide_up[w] -> (),
        0x0609_8b60 => stg01::slide_down[w] -> (),
        0x0609_8c0c => stg01::clear_plane_start[w] -> (),
        0x0609_8d64 => stg01::clear_plane_frame[w] -> (),
        0x0609_8de4 => stg01::clear_slide_and_plane[w] -> (),
        0x0609_8e0c => stg01::clear_idle[w] -> (),
        0x0609_8e18 => stg01::clear_plane_only[w] -> (),
        0x0609_8e30 => stg01::plane_b_turned[w] -> (),
        0x0609_8e98 => stg01::ground_start[w] -> (),
        0x0609_902c => stg01::ground_steps[w] -> (),
        0x0609_9190 => stg01::new_nbg0_start[w] -> (),
        0x0609_926c => stg01::darken_steps[w] -> (),
        0x0609_9348 => stg01::planes_up[w] -> (),
        0x0609_9390 => stg01::plane_turn_start[w] -> (),
        0x0609_9428 => stg01::plane_turn_steps[w] -> (),
        0x0609_94b0 => stg01::landing_start[w] -> (),
        0x0609_9534 => stg01::landing_steps[w] -> (),
        0x0609_95d4 => stg01::landing_draw[] -> (),
        0x0609_9654 => stg01::landing_set[w] -> (),
        0x0609_96d8 => stg01::touchdown_steps[w] -> (),
        0x0609_97b0 => stg01::landing_end[] -> (),
        0x0609_97d8 => stg01::climb_steps[w] -> (),
        0x0609_9864 => stg01::fly_off_start[w] -> (),
        0x0609_9894 => stg01::fly_off[w] -> (),
        0x0609_9964 => stg01::nbg1_zoomer[o] -> (),
        0x0609_99f8 => stg01::fly_off_steps[w] -> (),
        0x0609_9a74 => stg01::last_nbg1_start[] -> (),
        0x0609_9b0c => stg01::fly_off_end[w] -> (),
        0x0609_9b2c => stg01::fly_ground_start[w] -> (),
        0x0609_9c08 => stg01::fly_ground_steps[w] -> (),
        0x0609_9c6c => stg01::last_ground_draw[] -> (),
        0x0609_9cc8 => stg01::last_ground_start[] -> (),
        0x0609_9eb0 => stg01::last_plane_frame[w] -> (),
        0x0609_9f08 => stg01::last_nbg1[w] -> u32,
        0x0609_9f48 => stg01::last_nbg0[w] -> u32,
        0x0609_9fa8 => stg01::last_ground_steps[w] -> (),
        0x0609_a050 => stg01::hook_off[] -> (),
        0x0609_a068 => stg01::grounds_looping[w] -> (),
        0x0609_a0f4 => stg01::ground_idle[] -> (),
        0x0609_a100 => stg01::grounds_faster[w] -> (),
        0x0609_a170 => stg01::grounds_change[w] -> (),
        0x0609_a23c => stg01::grounds_to_shore[w] -> (),
        0x0609_a288 => stg01::target_behaviour[o] -> (),
        0x0609_a2b4 => stg01::target_draw[o] -> (),
        0x0609_a2c0 => stg01::target_init[o] -> (),
        0x0609_a330 => stg01::targets_set[e] -> u32,
        0x0609_a40c => stg01::targets_frame[w] -> (),
        0x0609_a604 => stg01::targets_start[w] -> (),
        0x0609_a8d8 => stg01::planes_sway[w] -> (),
        0x0609_aa64 => stg01::targets_glow[w] -> (),
        0x0609_ab14 => stg01::targets_steps[w] -> (),
        0x0609_ac50 => stg01::window_lines[] -> (),
        0x0609_acdc => stg01::view_turner[o] -> (),
        0x0609_adc4 => stg01::targets_rising[w] -> (),
        0x0609_ae24 => stg01::sea_start[w] -> (),
        0x0609_af1c => stg01::targets_dive[w] -> (),
        0x0609_af98 => stg01::targets_level[w] -> (),
        0x0609_b01c => stg01::targets_event[e] -> u32,
        0x0609_b034 => stg01::targets_clear[w] -> (),
        0x0609_b0c0 => stg01::climb_out[w] -> (),
        0x0609_b164 => stg01::climb_out_steps[w] -> (),
        0x0609_b198 => stg01::pull_up[w] -> (),
        0x0609_b210 => stg01::pull_up_steps[w] -> (),
        0x0609_b23c => stg01::nbg0_zoomer[o] -> (),
        0x0609_b2d0 => stg01::run_out_start[w] -> (),
        0x0609_b3f8 => stg01::run_nbg0[w] -> (),
        0x0609_b434 => stg01::run_out_steps[w] -> (),
        0x0609_b478 => stg01::approach_start[w] -> (),
        0x0609_b67c => stg01::approach_idle[] -> (),
        0x0609_b688 => stg01::approach_plane[w] -> (),
        0x0609_b6f8 => stg01::approach_steps[w] -> (),
        0x0609_b7b4 => stg01::end_spinner_behaviour[o] -> (),
        0x0609_b820 => stg01::end_spinner_set[p] -> u32,
        0x0609_b994 => stg01::choice_start[w] -> (),
        0x0609_ba00 => stg01::next_stage_choice[w] -> (),
        0x0609_bc74 => stg01::stage_done[] -> (),
        0x0609_bc8c => stg01::after_choice[w] -> (),
        0x0609_bcac => stg01::scroll_on[] -> (),
        0x0609_bcc4 => stg01::sb_wait[o] -> (),
        0x0609_be58 => stg01::sb_rise[o] -> (),
        0x0609_bf88 => stg01::sb_fight[o] -> (),
        0x0609_c220 => stg01::sb_sinking[o] -> (),
        0x0609_c40c => stg01::sb_draw[s] -> (),
        0x0609_c564 => stg01::sb_next_move[o] -> (),
        0x0609_c868 => stg01::sb_downed[o] -> (),
        0x0609_c984 => stg01::sb_turn_toward[o, t] -> u32,
        0x0609_c9c0 => stg01::sb_turn_to[o, a] -> u32,
        0x0609_ca80 => stg01::sb_turn_move[o] -> (),
        0x0609_cab0 => stg01::sb_spin[o] -> (),
        0x0609_cba0 => stg01::sb_spin_left[o] -> (),
        0x0609_cbcc => stg01::sb_spin_right[o] -> (),
        0x0609_cbf8 => stg01::sb_long_spin[o] -> (),
        0x0609_cdb4 => stg01::sb_fast_spin[o] -> (),
        0x0609_cf08 => stg01::sb_fast_spin_left[o] -> (),
        0x0609_cf34 => stg01::sb_fast_spin_right[o] -> (),
        0x0609_d1dc => stg01::sb_chase[o] -> (),
        0x0609_d2b0 => stg01::sb_arm_set[boss, k, x, y; z, model] -> u32,
        0x0609_d408 => stg01::sb_arm_behaviour[o] -> (),
        0x0609_d524 => stg01::sb_arm_dying[o] -> (),
        0x0609_d654 => stg01::sb_arm_gone[o] -> (),
        0x0609_d660 => stg01::sb_arm_draw[s] -> (),
        0x0609_d760 => stg01::sb_arm_turn_to[o, a] -> u32,
        0x0609_d798 => stg01::sb_arm_aim[o] -> u32,
        0x0609_d814 => stg01::sb_arm_shot_from[d, o] -> (),
        0x0609_d87c => stg01::sb_arm_spread[o] -> (),
        0x0609_d8f0 => stg01::sb_arm_stray_shot[o] -> (),
        0x0609_da1c => stg01::sb_arm_straight_ribbon[o] -> (),
        0x0609_daa4 => stg01::sb_arm_homing_ribbon[o] -> (),
        0x0609_dbd0 => stg01::sb_arm_straight_ribbon_2[o] -> (),
        0x0609_dcf4 => stg01::sb_arm_point[o] -> u32,
        0x0609_dd30 => stg01::sb_arm_volley[o, boss] -> (),
        0x0609_de10 => stg01::sb_arm_strays[o, boss] -> (),
        0x0609_dec8 => stg01::sb_arm_ribbons[o, boss] -> (),
        0x0609_dfd0 => stg01::sb_arm_homing_ribbons[o, boss] -> (),
        0x0609_e1e0 => stg01::sb_arm_ribbons_2[o, boss] -> (),
        0x0609_e3f0 => stg01::sb_gun_set[boss, side, x, y; z, model] -> u32,
        0x0609_e51c => stg01::sb_gun_behaviour[o] -> (),
        0x0609_e5c8 => stg01::sb_gun_gone[o] -> (),
        0x0609_e5d4 => stg01::sb_gun_draw[s] -> (),
        0x0609_e6a0 => stg01::segment_look[o] -> (),
        0x0609_e6fc => stg01::segment_behaviour[o] -> (),
        0x0609_e8f4 => stg01::segment_init[o, boss] -> (),
        0x0609_e9ec => stg01::sb_burst_step[boss] -> (),
        0x0609_ea4c => stg01::sb_head_ring[boss] -> (),
        0x0609_ead4 => stg01::head_glow[o] -> (),
        0x0609_eca8 => stg01::head_glow_init[o, boss] -> (),
        0x0609_ed4c => stg01::sb_beam_start[boss] -> (),
        0x0609_edac => stg01::blast_small[p, loud] -> (),
        0x0609_ee9c => stg01::blast_large[p, loud] -> (),
        0x0609_ef14 => stg01::blast_small_quiet[p] -> (),
        0x0609_efcc => stg01::blast_large_quiet[p] -> (),
        0x0609_f028 => stg01::sb_make[] -> u32,
        0x0609_f22c => sgl::sl_rpara_init_set_stg01[tables] -> (),
        0x0609_f260 => sgl::rpara_init[] -> (),
        0x0609_f29c => sgl::sl_make_ktable[table] -> (),
        0x0609_f31c => sgl::sl_plane_ra[size] -> (),
        0x0609_f32c => sgl::sl_map_rb_table[map] -> (),
        0x0609_f344 => sgl::sl_map_ra_table[map] -> (),
        0x0609_f39c => sgl::sl_over_ra[mode] -> (),
        0x0609_f3ac => sgl::sl_ktable_ra[table, mode] -> (),
        0x0609_f42c => sgl::sl_rpara_mode[mode] -> (),
        0x0609_f43c => sgl::sl_cur_rpara[b] -> (),
        0x0609_f44c => sgl::sl_sprite_type[t] -> (),
        0x0609_f45c => sgl::sl_sprite_cc_condition[condition] -> (),
        0x0609_f47c => sgl::sl_sprite_cc_number[n] -> (),
        0x0609_f48c => sgl::sl_col_offset_b_on[screens] -> (),
        0x0609_f49c => sgl::sl_plane_rb[size] -> (),
        0x0609_f4ac => sgl::sl_over_rb[mode] -> (),
        0x0609_f4bc => sgl::sl_ktable_rb[table, mode] -> (),
        0x0609_f51c => sgl::sl_scr_window0[left, top, right, bottom] -> (),
        0x0609_f53c => sgl::sl_line_window0[table] -> (),
        0x0609_f54c => sgl::sl_map_rb[a] -> (),
    }
    // STG04.BIN's code (fingerprint 5c18364ee98aacb1): `rewrites! { n; overlay "STG04"; ... }` here, from its first rewrite
    // STG05.BIN's code (fingerprint 2adefae5540dd556, the last stage)
    rewrites! { n; overlay "STG05";
        0x0608_0000 => stg05::stg05_mode[game] -> u32,
        0x0608_0290 => stg05::pose_step[anim, figure] -> (),
        0x0608_09e4 => stg05::falling_piece[o] -> (),
        0x0608_0a80 => stg05::falling_piece_set[] -> (),
        0x0608_0b74 => stg05::glint_released[o] -> u32,
        0x0608_0bc0 => stg05::glint_fade[o] -> u32,
        0x0608_0c2c => stg05::glint_trail_exec[o] -> (),
        0x0608_0c9c => stg05::glint_behaviour[o] -> (),
        0x0608_0d44 => stg05::glint_set[watched, mask, place, spare; number] -> (),
        0x0608_0e58 => stg05::glow_behaviour[o] -> (),
        0x0608_0ea8 => stg05::glow_draw[s] -> (),
        0x0608_0f38 => stg05::glow_set[] -> (),
        0x0608_0fc0 => stg05::spark_behaviour[o] -> (),
        0x0608_1054 => stg05::spark_draw[s] -> (),
        0x0608_10fc => stg05::spark_set[] -> (),
        0x0608_11c4 => stg05::follower_behaviour[o] -> (),
        0x0608_1210 => stg05::follower_set[parent] -> u32,
        0x0608_1290 => stg05::ember_behaviour[o] -> (),
        0x0608_1374 => stg05::ember_set[side] -> (),
        0x0608_1490 => stg05::shot_from[o, angle, at, reach] -> (),
    }
}
