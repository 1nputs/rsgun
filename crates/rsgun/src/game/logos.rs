//! TITLE.BIN's code, which the logo mode runs in the overlay window: the logos before the title.
//! Where the BIOS's setting is 4 the FBI's warning (FBI.BIN) comes first (the US's, as far as can
//! be told), then the logo (LOGOBG.BIN), then the title's picture (kinds.rs's loader brings it),
//! each faded in and out. A player with coins in (or, on free play, pressing start) skips to the
//! end. An object runs it; the mode's code makes the object, and is done the frame after.

use super::director::{self, GAME};
use super::globals::BIOS_SETTING;
use super::{files, kinds, nested, objects, pads, screen, system, Cpu};

/// The director's flags byte (bit 0: a mode runs), its loaded flags (the logo's file in behind
/// the warning; kinds.rs's: the title's picture in; set here: the title's screen set up), where
/// the pictures' files are (kinds.rs's), and its slots the mode uses (states.rs's: the first's
/// byte the mode's step, the second's long kept as it was across the mode's code).
const FLAGS: u32 = 0x3e;
const MODE_RUNS: u8 = 1;
const LOADED: u32 = 0x44;
const LOGO_IN: u32 = 0x10_0000;
const PICTURE_READY: u32 = 0x1000_0000;
const TITLE_SET_UP: u32 = 0x2000_0000;
const BG_AT: u32 = 0x48;
const MODE_STEP: u32 = 0x64;
const SLOT_2: u32 = 0x68;
const MADE: u8 = 2;

/// The pictures' files go off the top of RAM-L's room (kinds.rs's), loaded at a rate set for
/// the next load (files.rs).
const BG_LOAD_AT: u32 = 0x0607_4050;
const NEXT_RATE: u32 = 0x0603_2e40;
const LOGO_RATE: u32 = 0x2000;
const FBI: u32 = 0x0600_9000; // "FBI.BIN"
const LOGOBG: u32 = 0x0600_9018; // "LOGOBG.BIN"

/// Each picture's cells (bytes) and colours, as screen::picture_load takes them: the warning's,
/// the logo's, the title's (by the BIOS's setting).
const FBI_CELLS: u32 = 0x1_aa80;
const LOGO_CELLS: u32 = 0x3d80;
const TITLE_CELLS: [u32; 2] = [0x2_9080, 0x2_8a80];
const COLOURS: u32 = 0x100;

/// Fades: to black and back, at one speed.
const BLACK: u32 = -255i32 as u32;
const FADE_SPEED: u32 = 0x888;

/// The logos' object: its step (a word; its low byte read), a count of frames, its file's load,
/// the file's size.
const STEP: u32 = 0x3c;
const STEP_LOW: u32 = 0x3d;
const COUNT: u32 = 0x40;
const LOAD: u32 = 0x44;
const SIZE: u32 = 0x4c;
const LAST_STEP: u16 = 14;

/// The object: list 2, 0x80 bytes.
const LIST: u32 = 2;
const OBJECT_BYTES: u32 = 0x80;
const LOGOS_EXEC: u32 = 0x0608_0054;

/// Whether the BIOS's setting is 4 (the FBI's warning first).
fn fbi_first(cpu: &mut Cpu) -> bool {
    (cpu.read32(BIOS_SETTING) >> 12) & 0xF == 4
}

/// The logo mode's code (the director calls it each frame until it gives 1): the logos' object
/// made on the first frame, 1 on the next.
pub fn logo_mode(cpu: &mut Cpu, game: u32) -> u32 {
    let step = cpu.read8(game + MODE_STEP);
    let kept = cpu.read32(game + SLOT_2);
    let done = match step {
        0 => {
            nested!(cpu, 0x0608_04a0, logos_set(cpu));
            cpu.write8(game + MODE_STEP, MADE);
            0
        }
        MADE => 1,
        _ => 0,
    };
    cpu.write32(game + SLOT_2, kept);
    done
}

/// The logos' object made, at its first step.
pub fn logos_set(cpu: &mut Cpu) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, OBJECT_BYTES)) as i8 as i32 as u32;
    let obj = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, LIST, blocks, LOGOS_EXEC));
    if obj != 0 {
        cpu.write16(obj + STEP, 0);
    }
}

fn set_step(cpu: &mut Cpu, obj: u32, step: u16) {
    cpu.write16(obj + STEP, step);
}

/// One frame off the object's count: whether it has run out.
fn count_down(cpu: &mut Cpu, obj: u32) -> bool {
    let count = cpu.read32(obj + COUNT).wrapping_sub(1);
    cpu.write32(obj + COUNT, count);
    count as i32 <= 0
}

/// Whether either player pressed start this frame.
fn start_pressed(cpu: &mut Cpu) -> bool {
    (pads::pressed(cpu, 0) | pads::pressed(cpu, 1)) & pads::START != 0
}

/// The picture whose file is at the director's BG_AT shown on NBG1 (NBG0 and NBG1 on).
fn show_picture(cpu: &mut Cpu, cells: u32) {
    let game = cpu.read32(GAME);
    let at = cpu.read32(game + BG_AT);
    nested!(cpu, 0x0606_001c, screen::picture_load(cpu, at, cells, COLOURS));
    nested!(cpu, 0x0606_00f0, screen::planes_ab(cpu));
}

/// The logos' object, each frame. Its steps: 0, the picture screen set up and the first file read
/// (the warning's, or the logo's) off the top of the room; 1, once the mode runs, the warning (2)
/// or the logo (7); 2-6, the warning shown while the logo's file loads behind it, then faded out
/// and back; 7-9, the logo shown, the kinds' loader made, 180 frames (start cuts them short);
/// 10, faded out; 11, once the title's picture is in, it shown and faded in; 13, 30 frames; 14,
/// the mode done and the object freed. Step 12 does nothing.
pub fn logos_exec(cpu: &mut Cpu, obj: u32) {
    // (the load is kept in a local and written back at the end, as the C code has it)
    let mut load = cpu.read32(obj + LOAD);
    let skip = if !system::free_play(cpu) {
        nested!(cpu, 0x0604_0f44, system::coin_byte3(cpu, 0)) != 0 || nested!(cpu, 0x0604_0f44, system::coin_byte3(cpu, 1)) != 0
    } else {
        start_pressed(cpu)
    };
    if skip {
        set_step(cpu, obj, LAST_STEP);
    }
    match cpu.read8(obj + STEP_LOW) {
        0 => {
            nested!(cpu, 0x0605_fe8c, screen::picture_screen_set(cpu));
            nested!(cpu, 0x0606_00d4, screen::plane_a(cpu));
            let name = if fbi_first(cpu) { FBI } else { LOGOBG };
            let size = nested!(cpu, 0x0600_5e9c, files::file_size(cpu, name));
            cpu.write32(obj + SIZE, size);
            let at = cpu.read32(BG_LOAD_AT).wrapping_sub(size);
            cpu.write32(BG_LOAD_AT, at);
            let game = cpu.read32(GAME);
            cpu.write32(game + BG_AT, at);
            nested!(cpu, 0x0600_5f10, files::read_file(cpu, name, at));
            let top = cpu.read32(BG_LOAD_AT).wrapping_add(cpu.read32(obj + SIZE));
            cpu.write32(BG_LOAD_AT, top);
            set_step(cpu, obj, 1);
        }
        1 => {
            let game = cpu.read32(GAME);
            if cpu.read8(game + FLAGS) & MODE_RUNS != 0 {
                let next = if fbi_first(cpu) { 2 } else { 7 };
                set_step(cpu, obj, next);
            }
        }
        2 => {
            show_picture(cpu, FBI_CELLS);
            cpu.write32(obj + COUNT, 8);
            set_step(cpu, obj, 3);
        }
        3 => {
            if count_down(cpu, obj) {
                load = nested!(cpu, 0x0600_5e40, files::load_file(cpu, LOGOBG));
                if load != 0 {
                    let size = nested!(cpu, 0x0600_5edc, files::load_size(cpu, load));
                    cpu.write32(obj + SIZE, size);
                    let at = cpu.read32(BG_LOAD_AT).wrapping_sub(size);
                    cpu.write32(BG_LOAD_AT, at);
                    cpu.write32(NEXT_RATE, LOGO_RATE);
                    nested!(cpu, 0x0600_603c, files::start_load(cpu, load, at));
                    set_step(cpu, obj, 4);
                }
            }
        }
        4 => {
            let count = cpu.read32(obj + COUNT).wrapping_sub(1);
            cpu.write32(obj + COUNT, count);
            if nested!(cpu, 0x0600_6074, files::load_done(cpu, load)) != 0 {
                let game = cpu.read32(GAME);
                let flags = cpu.read32(game + LOADED);
                cpu.write32(game + LOADED, flags | LOGO_IN);
                let at = cpu.read32(BG_LOAD_AT);
                cpu.write32(game + BG_AT, at);
                let top = at.wrapping_add(cpu.read32(obj + SIZE));
                cpu.write32(BG_LOAD_AT, top);
                let count = cpu.read32(obj + COUNT).wrapping_add(300);
                cpu.write32(obj + COUNT, count);
                set_step(cpu, obj, 5);
            }
        }
        5 => {
            if count_down(cpu, obj) {
                nested!(cpu, 0x0605_e9d8, screen::stop_fades(cpu));
                nested!(cpu, 0x0605_e93c, screen::fade_to(cpu, BLACK, BLACK, BLACK, FADE_SPEED));
                cpu.write32(obj + COUNT, 30);
                set_step(cpu, obj, 6);
            }
        }
        6 => {
            if count_down(cpu, obj) {
                nested!(cpu, 0x0605_e9d8, screen::stop_fades(cpu));
                nested!(cpu, 0x0605_e93c, screen::fade_to(cpu, 0, 0, 0, FADE_SPEED));
                set_step(cpu, obj, 7);
            }
        }
        7 => {
            show_picture(cpu, LOGO_CELLS);
            cpu.write32(obj + COUNT, 4);
            set_step(cpu, obj, 8);
        }
        8 => {
            if count_down(cpu, obj) {
                nested!(cpu, 0x0605_3aa0, kinds::kinds_set(cpu));
                cpu.write32(obj + COUNT, 180);
                set_step(cpu, obj, 9);
            }
        }
        9 => {
            let count = cpu.read32(obj + COUNT).wrapping_sub(1);
            cpu.write32(obj + COUNT, count);
            if start_pressed(cpu) {
                cpu.write32(obj + COUNT, 0);
            }
            if cpu.read32(obj + COUNT) as i32 <= 0 {
                nested!(cpu, 0x0605_e9d8, screen::stop_fades(cpu));
                nested!(cpu, 0x0605_e93c, screen::fade_to(cpu, BLACK, BLACK, BLACK, FADE_SPEED));
                cpu.write32(obj + COUNT, 30);
                set_step(cpu, obj, 10);
            }
        }
        10 => {
            let count = cpu.read32(obj + COUNT);
            if count as i32 <= 0 {
                set_step(cpu, obj, 11);
            } else {
                cpu.write32(obj + COUNT, count.wrapping_sub(1));
            }
        }
        11 => {
            let game = cpu.read32(GAME);
            if cpu.read32(game + LOADED) & PICTURE_READY != 0 {
                let cells = TITLE_CELLS[fbi_first(cpu) as usize];
                show_picture(cpu, cells);
                let game = cpu.read32(GAME);
                let flags = cpu.read32(game + LOADED);
                cpu.write32(game + LOADED, flags | TITLE_SET_UP);
                nested!(cpu, 0x0605_e93c, screen::fade_to(cpu, 0, 0, 0, FADE_SPEED));
                cpu.write32(obj + COUNT, 30);
                set_step(cpu, obj, 13);
            }
        }
        13 => {
            if count_down(cpu, obj) {
                set_step(cpu, obj, 14);
            }
        }
        14 => {
            nested!(cpu, 0x0604_0c64, director::mode_done(cpu));
            nested!(cpu, 0x0600_579c, objects::obj_free(cpu, obj));
        }
        _ => {}
    }
    cpu.write32(obj + LOAD, load);
}
