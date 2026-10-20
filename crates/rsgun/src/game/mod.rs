//! The game, rewritten in Rust. Each function here replaces one of the game's SH-2 functions (as
//! the translation in build/port does, but written to be read: named, typed, its state named),
//! keeping its calling convention: arguments in r4-r7, the result in r0, what a function keeps
//! for its caller kept. Compare mode checks each against the SH-2 code by that convention
//! (stv::native::Contract::Abi; tools/port/check.sh). Ghidra's C for the function
//! (tools/ghidra/project.py) and the translation are where a rewrite starts from.
//!
//! A rewrite takes a [`Cpu`]: the bus the game's memory and hardware are on, and the registers it
//! needs to call code not rewritten yet ([`Cpu::call`]) and to wait ([`Cpu::wait`]).

mod dma;
mod files;
mod globals;
mod main_loop;
mod math;
mod objects;
mod pads;
mod palette;
mod system;
mod vblank;

use stv::native::{Contract, Natives};
use stv::sh2::{Bus, Sh2};

/// The CPU a rewritten function runs on.
pub struct Cpu<'a> {
    pub sh2: &'a mut Sh2,
    pub bus: &'a mut dyn Bus,
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

    /// One pass of a loop waiting for the hardware at `at` (the SH-2 code's loop, for the machine's
    /// watch on waits): the machine runs on as the SH-2 code's pass would let it.
    pub fn wait(&mut self, at: u32, cycles: u32) {
        stv::native::tick(self.sh2, self.bus, at, cycles);
    }
}

/// The return address a call from rewritten code gives (no code is there: odd).
const RETURN: u32 = 0xFFFF_FFF1;

/// The rewritten functions, each at its SH-2 function's entry: `entry => name[args] -> result`
/// (`-> ()` for none), arguments and result as u32, and for one that calls others the stack its
/// SH-2 function took (`frame N`, from its prologue): its callees get the stack where the SH-2
/// code had it (some hand the video chips' DMA a buffer of theirs on it). The SH-2 entry for each
/// moves the arguments from r4-r7 and the result to r0, and returns to PR.
macro_rules! rewrites {
    ($n:ident; $($entry:literal => $f:path [$($arg:ident),*] -> $ret:tt $(frame $frame:literal)?),* $(,)?) => {
        $(
            $n.register_as(
                $entry,
                |sh2: &mut Sh2, bus: &mut dyn Bus| {
                    let (ret, sp) = (sh2.pr, sh2.r[15]);
                    #[allow(unused_mut, unused_variables)]
                    let mut args = sh2.r[4..8].iter().copied();
                    $( let $arg = args.next().unwrap_or(0); )*
                    $( sh2.r[15] = sp - $frame; )?
                    let result = $f(&mut Cpu { sh2: &mut *sh2, bus }, $($arg),*);
                    rewrites!(@result sh2, result, $ret);
                    sh2.r[15] = sp;
                    sh2.pc = ret;
                },
                stringify!($f),
                None,
                rewrites!(@contract $ret),
            );
        )*
    };
    (@result $sh2:ident, $v:ident, ()) => { let () = $v; };
    (@result $sh2:ident, $v:ident, $t:tt) => { $sh2.r[0] = $v; };
    (@contract ()) => { Contract::AbiVoid };
    (@contract $t:tt) => { Contract::Abi };
}

/// Registers the rewritten functions (over the translations registered before).
pub fn register(n: &mut Natives) {
    rewrites! { n;
        0x0604_0000 => main_loop::main_loop[] -> u32 frame 32,
        0x0600_44a0 => dma::dma_set[src, dst, size] -> u32,
        0x0600_44e8 => dma::mode_dma_set[src, dst, size, mode] -> u32,
        0x0600_4530 => dma::divdma_set[src, dst, size] -> u32,
        0x0600_4580 => dma::dma_control[] -> () frame 32,
        0x0600_46a0 => dma::dma_control_init[] -> (),
        0x0600_46b8 => dma::dma_check[] -> u32,
        0x0600_6a58 => math::fixed_mul[a, b] -> u32,
        0x0600_6a6e => math::remainder[a, b] -> u32,
        0x0600_6a78 => math::quotient[a, b] -> u32,
        0x0600_6a8e => math::fixed_div[a, b] -> u32,
        0x0600_57f8 => objects::obj_size[bytes] -> u32 frame 20,
        0x0600_6aac => objects::find_free[area, blocks] -> u32,
        0x0600_6b00 => objects::obj_set[area, blocks, exec] -> u32 frame 16,
        0x0600_579c => objects::obj_free[obj] -> (),
        0x0600_6b64 => objects::obj_exec[] -> () frame 16,
        0x0600_5634 => objects::obj_init[] -> (),
        0x0600_584c => pads::pad_init[] -> (),
        0x0600_58a8 => pads::pad_read[] -> (),
        0x0600_5b18 => palette::set_colours[colours, index, count] -> (),
        0x0600_5b5c => palette::set_vdp1_colours[colours, index, count] -> (),
        0x0600_5a70 => palette::send_colours[] -> () frame 12,
        0x0600_5da0 => files::file_no[name] -> u32,
        0x0600_5e40 => files::load_file[name] -> u32 frame 16,
        0x0600_5fb0 => files::load_files[] -> (),
        0x0600_60c4 => files::clear_loads[] -> (),
        0x0600_5e10 => files::init_loads[] -> u32,
        0x0600_4468 => vblank::init_vblank[] -> () frame 8,
        0x0600_438c => vblank::run_callbacks[] -> () frame 24,
        0x0600_43d4 => vblank::add_callback[f] -> u32,
        0x0600_4408 => vblank::remove_callback[f] -> u32,
        0x0600_4378 => system::count_vblank[] -> (),
        0x0600_475c => system::sound_command_done[slot] -> u32,
        0x0604_0f44 => system::coin_byte3[slot] -> u32,
        0x0604_0f74 => system::credits[slot] -> u32,
        0x0604_0fac => system::coin_settings_low[] -> u32,
        0x0603_3a6a => system::purge_cache[] -> (),
        0x0603_64a0 => system::purge_cache[] -> (),
        0x0606_e24c => system::wait_dma0[] -> (),
        0x0603_5070 => system::set_interrupt_mask[mask] -> (),
        0x0600_136a => system::next_unless_end[n] -> u32,
    }
}
