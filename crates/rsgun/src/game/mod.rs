//! The game, rewritten in Rust. Each function here replaces one of the game's SH-2 functions (as
//! the translation in build/port does, but written to be read: named, typed, its state named),
//! keeping its calling convention: arguments in r4-r7, the result in r0, what a function keeps
//! for its caller kept. Compare mode checks each against the SH-2 code by that convention
//! (stv::native::Contract::Abi; tools/port/check.sh). Ghidra's C for the function
//! (tools/ghidra/project.py) and the translation are where a rewrite starts from.
//!
//! A rewrite takes a [`Cpu`]: the bus the game's memory and hardware are on, and the registers it
//! needs to call code not rewritten yet ([`Cpu::call`]) and to wait ([`Cpu::wait`]).

mod backup;
mod clib;
mod director;
mod dma;
mod files;
mod globals;
mod hits;
mod hud;
mod main_loop;
mod math;
mod objects;
mod pads;
mod palette;
mod screen;
mod sound;
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

    /// The stack the function's callees get: the bottom of its SH-2 function's frame (`frame N`),
    /// where a struct it hands a callee by address goes.
    pub fn stack(&self) -> u32 {
        self.sh2.r[15]
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
        0x0600_49e8 => sound::sound_load[] -> () frame 36,
        0x0600_4ddc => sound::stop_effects[] -> () frame 32,
        0x0600_4e98 => sound::request_music[music] -> (),
        0x0600_4f0c => sound::music_fade_out[frames] -> (),
        0x0600_4f3c => sound::se_control[se] -> u32 frame 36,
        0x0600_50d4 => sound::se_set[se] -> u32 frame 24,
        0x0600_52e0 => sound::se_stop[se, control] -> u32 frame 20,
        0x0600_53dc => sound::sound_exec[m] -> () frame 28,
        0x0600_5598 => sound::sound_manager_set[] -> u32 frame 16,
        0x0600_817c => clib::memset[dst, c, n] -> u32,
        0x0600_81d0 => clib::memcpy[dst, src, n] -> u32,
        0x0600_79a8 => sound::seq_status[buf, control] -> (),
        0x0600_46d8 => sound::load_sound_data[n, dst] -> u32,
        0x0600_8058 => system::set_scu_mask[mask, bits] -> (),
        0x0600_69d8 => system::shift_left[x, bits] -> u32,
        0x0600_6364 => backup::backup_byte[i] -> u32,
        0x0600_637c => backup::backup_long[i] -> u32 frame 8,
        0x0600_63a4 => backup::set_backup_byte[i, v] -> (),
        0x0600_63bc => backup::set_backup_long[i, v] -> u32 frame 8,
        0x0600_63e4 => backup::load_backup[] -> u32 frame 40,
        0x0600_66ec => backup::format_backup[why] -> () frame 24,
        0x0600_65fc => backup::save_scores[] -> u32 frame 32,
        0x0604_2db8 => director::game_set[] -> u32 frame 16,
        0x0604_2b14 => director::game_exec[game] -> () frame 80,
        0x0604_0c64 => director::mode_done[] -> (),
        0x0604_0cb4 => director::director_flags[] -> u32,
        0x0604_02c4 => states::load_mode_file[mode] -> u32 frame 12,
        0x0604_12e8 => states::state_reset[game] -> () frame 20,
        0x0604_13b0 => states::state_load_mode[game] -> () frame 16,
        0x0604_17a4 => states::state_run_mode[game] -> () frame 16,
        0x0604_1578 => states::state_screen[game] -> () frame 16,
        0x0604_1848 => states::state_title[game] -> () frame 32,
        0x0604_1df0 => states::state_stage[game] -> () frame 132,
        0x0604_2734 => states::state_stage_end[game] -> () frame 36,
        0x0604_1010 => states::clear_mode[game] -> () frame 24,
        0x0605_e6dc => screen::set_fade_offset[r, g, b] -> (),
        0x0605_e8dc => screen::fade_new[r, g, b, speed] -> u32,
        0x0605_e93c => screen::fade_to[r, g, b, speed] -> (),
        0x0605_e9d8 => screen::stop_fades[] -> (),
        0x0605_e73c => screen::fade_exec[fade] -> () frame 28,
        0x0605_e1b0 => screen::screen_set[] -> () frame 16,
        0x0605_e49c => screen::screen_exec[s] -> () frame 16,
        0x0605_e2dc => screen::screen_fields[] -> (),
        0x0605_ea98 => screen::screen_vblank[] -> () frame 32,
        0x0605_e6c0 => screen::offsets_clear[] -> () frame 8,
        0x0605_fdc0 => screen::take_plane_shared[] -> (),
        0x0605_fde8 => screen::request_clear_text_plane[] -> (),
        0x0605_fdfc => screen::clear_text_plane[] -> (),
        0x0605_ef44 => screen::clear_scroll_word[] -> (),
        0x0605_edf8 => screen::copy_words[src, dst, bytes] -> (),
        0x0605_ed84 => screen::long_copies_on[] -> u32,
        0x0605_ef20 => screen::offset_list[delta, list, n] -> (),
        0x0605_e24c => screen::screen_off[] -> () frame 8,
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
        0x0604_3874 => hits::in_hit_set[of, obj] -> u32 frame 12,
        0x0604_3d38 => hits::hits_exec[hits] -> () frame 228,
        0x0604_5d78 => math::random_seed[seed] -> (),
        0x0604_5d90 => math::random[n] -> u32 frame 8,
        0x0604_5dcc => math::by_difficulty[x] -> u32,
        0x0606_14b0 => text::typewriter_set[s, at, flags, blink] -> u32 frame 28,
        0x0606_131c => text::typewriter_exec[obj] -> () frame 16,
        0x0606_1504 => text::message_exec[obj] -> () frame 12,
        0x0606_12bc => text::stop_text_objects[] -> () frame 8,
        0x0605_fe34 => hud::put_digits[n, digits, at, p] -> (),
        0x0606_0388 => hud::status_line[p, line] -> () frame 24,
        0x0606_0550 => hud::gauge[p] -> () frame 20,
        0x0606_0354 => hud::status_reset[p] -> (),
        0x0606_0a0c => hud::draw_status[p] -> () frame 8,
        0x0600_4378 => system::count_vblank[] -> (),
        0x0604_0f44 => system::coin_byte3[slot] -> u32,
        0x0604_0f5c => system::use_credit[slot] -> u32 frame 8,
        0x0604_0f74 => system::credits[slot] -> u32,
        0x0604_0fac => system::coin_settings_low[] -> u32,
        0x0603_3a6a => system::purge_cache[] -> (),
        0x0603_64a0 => system::purge_cache[] -> (),
        0x0606_e24c => system::wait_dma0[] -> (),
        0x0603_5070 => system::set_interrupt_mask[mask] -> (),
        0x0600_136a => system::next_unless_end[n] -> u32,
    }
}
