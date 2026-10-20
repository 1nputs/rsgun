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
mod hits;
mod hud;
mod main_loop;
mod math;
mod models;
mod objects;
mod orbits;
mod pads;
mod play;
mod palette;
mod parts;
mod screen;
mod sound;
mod sprites;
mod states;
mod system;
mod text;
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

impl Prologue {
    /// Read from the code at `entry`, up to its first branch: the pushes (`mov.l Rm,@-R15`, then
    /// `sts.l PR,@-R15`, after which a push is an argument's), and the locals (`add #imm,R15`, or
    /// `mov.w @(disp,PC),Rn` then `add Rn,R15`). Other instructions may be scheduled among them.
    pub fn read(bus: &mut dyn Bus, entry: u32) -> Prologue {
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
            let v = if r == PR_PUSH { sh2.pr } else { sh2.r[r as usize] };
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
    ($n:ident; $($entry:literal => $f:path [$($arg:ident),* $(; $($sarg:ident),+)?] -> $ret:tt),* $(,)?) => {
        $(
            $n.register_as(
                $entry,
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
                    let result = $f(&mut Cpu { sh2: &mut *sh2, bus }, $($arg),* $(, $($sarg),+)?);
                    rewrites!(@result sh2, result, $ret);
                    sh2.r[15] = sp;
                    sh2.r[14] = fp;
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
        0x0600_6b00 => objects::obj_set[area, blocks, exec] -> u32,
        0x0600_579c => objects::obj_free[obj] -> (),
        0x0600_6b64 => objects::obj_exec[] -> (),
        0x0600_5634 => objects::obj_init[] -> (),
        0x0600_584c => pads::pad_init[] -> (),
        0x0600_58a8 => pads::pad_read[] -> (),
        0x0600_5b18 => palette::set_colours[colours, index, count] -> (),
        0x0600_5b5c => palette::set_vdp1_colours[colours, index, count] -> (),
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
        0x0600_4f3c => sound::se_control[se] -> u32,
        0x0600_50d4 => sound::se_set[se] -> u32,
        0x0600_52e0 => sound::se_stop[se, control] -> u32,
        0x0600_53dc => sound::sound_exec[m] -> (),
        0x0600_5598 => sound::sound_manager_set[] -> u32,
        0x0600_817c => clib::memset[dst, c, n] -> u32,
        0x0600_81d0 => clib::memcpy[dst, src, n] -> u32,
        0x0600_79a8 => sound::seq_status[buf, control] -> (),
        0x0600_46d8 => sound::load_sound_data[n, dst] -> u32,
        0x0600_8058 => system::set_scu_mask[mask, bits] -> (),
        0x0600_69d8 => system::shift_left[x, bits] -> u32,
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
        0x0604_d95c => actors::trail_init[actor, a, b, boxes] -> (),
        0x0604_da00 => actors::trail_record[actor, angle] -> (),
        0x0605_75e4 => explosions::shrink_exec[o] -> (),
        0x0605_80bc => explosions::twin_exec[o] -> (),
        0x0605_8278 => explosions::twin_set[p] -> (),
        0x0605_8444 => explosions::ember_exec[o] -> (),
        0x0605_8520 => explosions::ember_set[d] -> (),
        0x0605_8638 => explosions::debris_exec[o] -> (),
        0x0605_86e4 => explosions::debris_set[p, spread] -> (),
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
        0x0605_3bfc => bullets::bullet_frame[b] -> (),
        0x0605_3ccc => bullets::bullet_move[b] -> (),
        0x0605_3cfc => bullets::bullet_swirl[b] -> (),
        0x0605_3db0 => bullets::bullet_set[shot] -> u32,
        0x0605_3f48 => bullets::bullet_set_plain[shot] -> u32,
        0x0605_3f70 => bullets::shootable_bullet[b] -> (),
        0x0605_7454 => actors::drift_effect[e] -> u32,
        0x0605_681c => actors::enemy_clear[enemy] -> (),
        0x0605_2908 => models::model_init[obj] -> (),
        0x0605_2970 => models::follow_parent[obj] -> (),
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
        0x0605_ecb4 => screen::queue_fill[src, dst, size] -> u32,
        0x0605_ecf0 => screen::queue_copy[src, dst, size] -> u32,
        0x0605_ed2c => screen::queue_long_copy[src, dst, size, piece] -> u32,
        0x0605_ed94 => screen::queue_cells[src, dst, w, h; char_offset, add] -> u32,
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
        0x0606_4f3c => math::clamp[x, limit] -> u32,
        0x0606_2e7c => math::lerp[a, b, t] -> u32,
        0x0606_2ebc => math::ease[a, b, t] -> u32,
        0x0604_39b0 => hits::sparks[a, b, a_box, b_box] -> (),
        0x0604_3c64 => hits::absorb[a, b] -> (),
        0x0604_5090 => hits::chain_reset[p] -> (),
        0x0604_50d4 => hits::hits_reset[] -> (),
        0x0604_5154 => hits::hits_set[] -> u32,
        0x0604_51b4 => hits::register_boxes[obj] -> u32,
        0x0604_6194 => camera::angle_diff[from, to] -> u32,
        0x0604_2ff8 => camera::camera_set[] -> u32,
        0x0604_2e6c => camera::camera_exec[cam] -> (),
        0x0604_114c => play::player_reset[p] -> (),
        0x0604_129c => play::game_start[] -> (),
        0x0604_016c => main_loop::init_game[] -> (),
        0x0604_5e30 => aim::angle_to[a, b] -> u32,
        0x0604_5e60 => aim::length[dx, dy] -> u32,
        0x0604_5ee4 => aim::distance[a, b] -> u32,
        0x0604_0ccc => aim::target[obj, delay, out] -> u32,
        0x0604_0df0 => aim::aim[obj, delay] -> u32,
        0x0604_5df8 => aim::free_number[from] -> u32,
        0x0604_5fd8 => aim::nearest[obj, out] -> u32,
        0x0604_03c4 => sprites::draw_sprite[s] -> (),
        0x0604_030c => sprites::draw_model[s] -> (),
        0x0604_0960 => sprites::draw[s] -> (),
        0x0604_6138 => actors::end_by_flags[actor] -> u32,
        0x0604_6168 => actors::approach[from, to, step] -> u32,
        0x0604_dcb4 => actors::plain_exec[actor] -> (),
        0x0604_d600 => actors::numbered_exec[actor] -> (),
        0x0604_6264 => sprites::sprite_init[s] -> (),
        0x0605_6554 => aim::turn_sign[a, b] -> u32,
        0x0605_6580 => aim::angle_between[a, b] -> u32,
        0x0605_69f8 => aim::area_flags[s, top, bottom, left; right] -> u32,
        0x0605_6a24 => aim::off_screen[s] -> u32,
        0x0605_70d8 => aim::turn_toward[angle, speed, from, to; most, ease, accel] -> (),
        0x0605_73a0 => actors::anim_step[actor] -> u32,
        0x0605_73f0 => actors::drift[actor] -> (),
        0x0605_6cc8 => parts::part_exec[part] -> (),
        0x0604_dd50 => effects::actor_init[actor] -> (),
        0x0604_6530 => effects::link_init[link, to] -> (),
        0x0604_e1cc => effects::effect_set[fx] -> u32,
        0x0604_6558 => effects::link_step[s, link] -> (),
        0x0604_6284 => hits::hit_init[h] -> (),
        0x0604_62ac => actors::actor_setup[actor] -> (),
        0x0604_816c => actors::player_set[st] -> u32,
        0x0604_deb4 => effects::effect_exec[obj] -> (),
        0x0604_d780 => actors::actor_exec[actor] -> (),
        0x0604_3d38 => hits::hits_exec[hits] -> (),
        0x0604_5d78 => math::random_seed[seed] -> (),
        0x0604_5d90 => math::random[n] -> u32,
        0x0604_5dcc => math::by_difficulty[x] -> u32,
        0x0606_14b0 => text::typewriter_set[s, at, flags, blink] -> u32,
        0x0606_131c => text::typewriter_exec[obj] -> (),
        0x0606_1504 => text::message_exec[obj] -> (),
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
}
