//! The kinds' loading (as a mode loads): an object that loads the background's file, runs three
//! loaders (not rewritten yet), then the mode's kinds one a step: each kind's file (by the kinds'
//! file names) loads where the next file goes, its offsets are made pointers, and it is put in
//! place: its characters' sizes into the table of characters, its characters into VDP1's VRAM,
//! its colours into colour RAM or VDP1's table, and the kind's entry (sprites.rs's kinds) set.
//! The sprites' kinds come first in the list (to a 1), then the models' (to a 79).

use super::director::GAME;
use super::globals::bios_setting_is_1;
use super::{files, nested, objects, palette, sprites, system, Cpu};

/// A kind's file: its characters' sizes (an offset: 8 bytes each, a width, height, place in VRAM
/// in 8 bytes and a word more) and their count, its characters (an offset) and their size, its
/// colours (an offset) and their count (a word, +0x16), its colour mode (bits 3-5: 8 for VDP1's
/// table), its attributes (a word, +0x1a), its frames or models (an offset) and their count.
const F_SIZES: u32 = 0;
const F_CHARS: u32 = 4;
const F_DATA: u32 = 8;
const F_DATA_SIZE: u32 = 0xc;
const F_COLOURS: u32 = 0x10;
const F_COLOUR_COUNT: u32 = 0x16;
const F_MODE: u32 = 0x18;
const F_ATTRS: u32 = 0x1a;
const F_FRAMES: u32 = 0x1c;
const F_FRAME_COUNT: u32 = 0x20;
const MODE_BITS: u32 = 0x38;
const VDP1_TABLE: u32 = 8;

/// The kinds (16 bytes each: the file, the frames or models, the first character, the
/// attributes, the first colour), each kind's file name (8 bytes each), the characters' sizes.
const KINDS: u32 = 0x0607_41f0;
const K_FILE: u32 = 0;
const K_FRAMES: u32 = 4;
const K_CHAR: u32 = 8;
const K_ATTRS: u32 = 0xa;
const K_COLOUR: u32 = 0xc;
const KIND_NAMES: u32 = 0x0607_2478;
const CHAR_SIZES: u32 = 0x0600_cc70;
const VDP1_VRAM: u32 = 0x25c0_0000;
/// The next VDP1 table colour (a word) and its default.
const VDP1_NEXT: u32 = 0x0607_4054;
const VDP1_NEXT_DEFAULT: u32 = 0x0607_4060;
/// SGL's: a copy (src, dst, bytes).
const SL_COPY: u32 = 0x0603_3a10;

fn kind(k: u32) -> u32 {
    KINDS.wrapping_add(k.wrapping_mul(16))
}

/// Kind `k`'s file name.
fn name(cpu: &mut Cpu, k: u32) -> u32 {
    cpu.read32(KIND_NAMES.wrapping_add(k.wrapping_mul(8)))
}

fn add_base(cpu: &mut Cpu, at: u32, base: u32) {
    let v = cpu.read32(at).wrapping_add(base);
    cpu.write32(at, v);
}

/// A sprite kind's file at `p` made kind `k`'s: its offsets made pointers (its frames' too).
pub fn sprite_kind_fix(cpu: &mut Cpu, k: u32, p: u32) {
    cpu.write32(kind(k) + K_FILE, p);
    for at in [F_SIZES, F_DATA, F_COLOURS, F_FRAMES] {
        add_base(cpu, p + at, p);
    }
    let mut i = 0;
    while (i as i32) < cpu.read32(p + F_FRAME_COUNT) as i32 {
        let frames = cpu.read32(p + F_FRAMES);
        add_base(cpu, frames + 8 * i, p);
        i += 1;
    }
}

/// Kind `k`'s file read to `dst` and fixed: the read's result.
pub fn sprite_kind_read(cpu: &mut Cpu, k: u32, dst: u32) -> u32 {
    let name = name(cpu, k);
    let r = nested!(cpu, 0x0600_5f10, files::read_file(cpu, name, dst));
    nested!(cpu, 0x0604_5638, sprite_kind_fix(cpu, k, dst));
    r
}

/// Kind `k`'s file's load started to `dst`: the load, or 0.
pub fn kind_load(cpu: &mut Cpu, k: u32, dst: u32) -> u32 {
    let name = name(cpu, k);
    let load = nested!(cpu, 0x0600_5e40, files::load_file(cpu, name));
    nested!(cpu, 0x0600_603c, files::start_load(cpu, load, dst));
    load
}

/// The characters' sizes from the file, their places moved to `vram`, from character `first`;
/// the characters copied there.
fn put_chars(cpu: &mut Cpu, p: u32, vram: u32, first: u32) {
    let mut i = 0u32;
    while (i as i32) < cpu.read32(p + F_CHARS) as i32 {
        let at = CHAR_SIZES.wrapping_add(first.wrapping_add(i).wrapping_mul(8));
        let src = cpu.read32(p + F_SIZES) + 8 * i;
        let w = cpu.read16(src);
        cpu.write16(at, w);
        let sizes = cpu.read32(p + F_SIZES);
        let h = cpu.read16(sizes + 8 * i + 2);
        cpu.write16(at + 2, h);
        let sizes = cpu.read32(p + F_SIZES);
        let place = cpu.read16(sizes + 8 * i + 4).wrapping_add((vram >> 3) as u16);
        cpu.write16(at + 4, place);
        let sizes = cpu.read32(p + F_SIZES);
        let size = cpu.read16(sizes + 8 * i + 6);
        cpu.write16(at + 6, size);
        i += 1;
    }
    let (data, size) = (cpu.read32(p + F_DATA), cpu.read32(p + F_DATA_SIZE));
    cpu.call(SL_COPY, &[data, VDP1_VRAM.wrapping_add(vram), size]);
}

fn colours(cpu: &mut Cpu, p: u32, index: u32) {
    let at = cpu.read32(p + F_COLOURS);
    let count = cpu.read16(p + F_COLOUR_COUNT) as u32;
    nested!(cpu, 0x0600_5b18, palette::set_colours(cpu, at, index, count));
}

fn vdp1_colours(cpu: &mut Cpu, p: u32) {
    let at = cpu.read32(p + F_COLOURS);
    let count = cpu.read16(p + F_COLOUR_COUNT) as u32;
    let next = cpu.read16(VDP1_NEXT) as u32;
    nested!(cpu, 0x0600_5b5c, palette::set_vdp1_colours(cpu, at, next, count));
}

fn set_kind(cpu: &mut Cpu, k: u32, p: u32, first: u32, colour: u16) {
    let at = kind(k);
    let frames = cpu.read32(p + F_FRAMES);
    cpu.write32(at + K_FRAMES, frames);
    cpu.write16(at + K_CHAR, first as u16);
    let attrs = cpu.read16(p + F_ATTRS);
    cpu.write16(at + K_ATTRS, attrs);
    cpu.write16(at + K_COLOUR, colour);
}

/// Sprite kind `k` put in place: its characters at `vram` (from character `first`), its colours
/// at `colour` (colour RAM) or the next of VDP1's table. Where the next characters go.
pub fn sprite_kind_install(cpu: &mut Cpu, k: u32, vram: u32, first: u32, colour: u32) -> u32 {
    let p = cpu.read32(kind(k) + K_FILE);
    let mut colour = colour as u16;
    put_chars(cpu, p, vram, first);
    if cpu.read32(p + F_MODE) & MODE_BITS == VDP1_TABLE {
        vdp1_colours(cpu, p);
        let next = cpu.read16(VDP1_NEXT);
        colour = next.wrapping_mul(2).wrapping_add(0xe000) >> 3;
        let count = cpu.read16(p + F_COLOUR_COUNT);
        cpu.write16(VDP1_NEXT, next.wrapping_add(count));
    } else {
        colours(cpu, p, colour as u32);
    }
    set_kind(cpu, k, p, first, colour);
    vram.wrapping_add(cpu.read32(p + F_DATA_SIZE))
}

/// A model kind's file at `p` made kind `k`'s: its offsets made pointers, its models' (two each,
/// plain and flashing: SGL's polygon data, its points, polygons and attributes) too.
const PD_POINTS: u32 = 0;
const PD_POLYGONS: u32 = 8;
const PD_ATTRS: u32 = 0x10;
const PD_COUNT: u32 = 0xc;

/// The file's model `i` (its plain and flashing polygon data).
fn model_pair(cpu: &mut Cpu, p: u32, i: u32) -> u32 {
    let models = cpu.read32(p + F_FRAMES);
    cpu.read32(models + 4 * i)
}

pub fn model_kind_fix(cpu: &mut Cpu, k: u32, p: u32) {
    cpu.write32(kind(k) + K_FILE, p);
    for at in [F_SIZES, F_DATA, F_COLOURS, F_FRAMES] {
        add_base(cpu, p + at, p);
    }
    let mut i = 0;
    while (i as i32) < cpu.read32(p + F_FRAME_COUNT) as i32 {
        let models = cpu.read32(p + F_FRAMES);
        add_base(cpu, models + 4 * i, p);
        let pair = model_pair(cpu, p, i);
        add_base(cpu, pair, p);
        let pair = model_pair(cpu, p, i);
        add_base(cpu, pair + 4, p);
        let pair = model_pair(cpu, p, i);
        let (plain, flashing) = (cpu.read32(pair), cpu.read32(pair + 4));
        for pd in [plain, flashing] {
            for at in [PD_POINTS, PD_POLYGONS, PD_ATTRS] {
                add_base(cpu, pd + at, p);
            }
        }
        i += 1;
    }
}

pub fn model_kind_read(cpu: &mut Cpu, k: u32, dst: u32) -> u32 {
    let name = name(cpu, k);
    let r = nested!(cpu, 0x0600_5f10, files::read_file(cpu, name, dst));
    nested!(cpu, 0x0604_583c, model_kind_fix(cpu, k, dst));
    r
}

/// Model kind `k` put in place: its polygons' characters moved by `first`, their colours by the
/// kind's (the flashing ones' all the flash colour), then as a sprite kind.
const ATTR_SIZE: u32 = 12;
const ATTR_TEXTURE: u32 = 2;
const ATTR_COLOUR: u32 = 6;
const VDP1_BANK: u16 = 0x1c00;
const VDP1_FLASH: u16 = 0x1dfc;
const FLASH_COLOURS: u16 = 0xe0;

pub fn model_kind_install(cpu: &mut Cpu, k: u32, vram: u32, first: u32, colour: u32) -> u32 {
    let p = cpu.read32(kind(k) + K_FILE);
    let mut colour = colour as u16;
    let flash = if cpu.read32(p + F_MODE) & MODE_BITS == VDP1_TABLE {
        colour = VDP1_BANK.wrapping_add(cpu.read16(VDP1_NEXT) >> 2);
        VDP1_FLASH
    } else {
        cpu.read16(KINDS + K_COLOUR).wrapping_add(FLASH_COLOURS)
    };
    let mut i = 0;
    while (i as i32) < cpu.read32(p + F_FRAME_COUNT) as i32 {
        let pair = model_pair(cpu, p, i);
        let (plain, flashing) = (cpu.read32(pair), cpu.read32(pair + 4));
        let mut j = 0;
        while j < cpu.read32(plain + PD_COUNT) {
            let a = cpu.read32(plain + PD_ATTRS) + ATTR_SIZE * j;
            let b = cpu.read32(flashing + PD_ATTRS) + ATTR_SIZE * j;
            let t = cpu.read16(a + ATTR_TEXTURE).wrapping_add(first as u16);
            cpu.write16(a + ATTR_TEXTURE, t);
            let t = cpu.read16(b + ATTR_TEXTURE).wrapping_add(first as u16);
            cpu.write16(b + ATTR_TEXTURE, t);
            let c = cpu.read16(a + ATTR_COLOUR).wrapping_add(colour);
            cpu.write16(a + ATTR_COLOUR, c);
            cpu.write16(b + ATTR_COLOUR, flash);
            j += 1;
        }
        i += 1;
    }
    put_chars(cpu, p, vram, first);
    if cpu.read32(p + F_MODE) & MODE_BITS == VDP1_TABLE {
        vdp1_colours(cpu, p);
        let next = cpu.read16(VDP1_NEXT);
        colour = next;
        let count = cpu.read16(p + F_COLOUR_COUNT);
        cpu.write16(VDP1_NEXT, next.wrapping_add(count));
    } else {
        colours(cpu, p, colour as u32);
    }
    set_kind(cpu, k, p, first, colour);
    vram.wrapping_add(cpu.read32(p + F_DATA_SIZE))
}

/// The loader's fields: its step (a word: a byte saying a load is under way, then the step), the
/// load, the place in the kinds' list.
const STEP: u32 = 0x3c;
const LOADING: u32 = 0x3c;
const STEP_LOW: u32 = 0x3d;
const LOAD: u32 = 0x40;
const PLACE: u32 = 0x44;
const OBJ_MODE: u32 = 3;
const ENDING: u8 = 0x70;

/// The game's loaded flags (director's +0x44: each kind's in the list by its place, the three
/// loaders', the background's) and where the background's file went.
const LOADED: u32 = 0x44;
const BG_AT: u32 = 0x48;
const LOADED_BG: u32 = 0x1000_0000;
const LOADED_A: u32 = 0x8000;
const LOADED_B: u32 = 0x2000;
const LOADED_C: u32 = 0x4000;
const ALL_KINDS: u32 = 0xffff;
/// The background's file names (by the BIOS's setting), where its file goes (taken off the top
/// of RAM-L's room), the rate of its load (files.rs).
const BG_NAME: u32 = 0x0600_9008;
const BG_NAME_1: u32 = 0x0600_8ff4;
const BG_LOAD_AT: u32 = 0x0607_4050;
const NEXT_RATE: u32 = 0x0603_2e40;
const BG_RATE: u32 = 0x4000;
/// Where the next file loads, VDP1's VRAM's next place, the next character and colour (words).
const LOAD_AT: u32 = 0x0607_40d8;
const VRAM_NEXT: u32 = 0x0607_4094;
const CHAR_NEXT: u32 = 0x0607_40e0;
const COLOUR_NEXT: u32 = 0x0607_40ec;
/// The three loaders (each loads at LOAD_AT and gives its size).
const LOADER_A: u32 = 0x0606_0260;
const LOADER_B: u32 = 0x0605_fb00;
const LOADER_C: u32 = 0x0606_165c;
/// The mode's kinds, the end of the sprites' and the models'; the sprite kinds whose colours are
/// not counted; kind 0's colours that go to VDP1's table too.
const KIND_LIST: u32 = 0x0607_2ed4;
const SPRITES_END: i8 = 1;
const MODELS_END: i8 = 79;
const SHARED_COLOURS: [u32; 3] = [8, 13, 3];
const KIND_0_VDP1: (u32, u32, u32) = (0x1c0, 0x7f0, 16);

pub fn kinds_exec(cpu: &mut Cpu, o: u32) {
    let f = cpu.stack();
    let load = cpu.read32(o + LOAD);
    cpu.write32(f + 4, load);
    let game = cpu.read32(GAME);
    match cpu.read8(o + STEP_LOW) {
        0 => {
            let v = cpu.read16(VDP1_NEXT_DEFAULT);
            cpu.write16(VDP1_NEXT, v);
            cpu.write16(o + STEP, 1);
        }
        1 => {
            if cpu.read8(o + LOADING) == 0 {
                if cpu.read32(game + LOADED) & LOADED_BG != 0 {
                    cpu.write16(o + STEP, 2);
                } else {
                    let name = if bios_setting_is_1(cpu) { BG_NAME_1 } else { BG_NAME };
                    let load = nested!(cpu, 0x0600_5e40, files::load_file(cpu, name));
                    cpu.write32(f + 4, load);
                    if load != 0 {
                        let size = nested!(cpu, 0x0600_5edc, files::load_size(cpu, load));
                        let at = cpu.read32(BG_LOAD_AT).wrapping_sub(size);
                        cpu.write32(BG_LOAD_AT, at);
                        cpu.write32(NEXT_RATE, BG_RATE);
                        nested!(cpu, 0x0600_603c, files::start_load(cpu, load, at));
                        cpu.write8(o + LOADING, 1);
                    }
                }
            } else if nested!(cpu, 0x0600_6074, files::load_done(cpu, load)) != 0 {
                let game = cpu.read32(GAME);
                let flags = cpu.read32(game + LOADED);
                cpu.write32(game + LOADED, flags | LOADED_BG);
                let at = cpu.read32(BG_LOAD_AT);
                cpu.write32(game + BG_AT, at);
                cpu.write16(o + STEP, 2);
            }
        }
        2 => cpu.write16(o + STEP, 3),
        3 => {
            let flags = cpu.read32(game + LOADED);
            if flags & ALL_KINDS == ALL_KINDS {
                cpu.write16(o + STEP, 8);
            } else {
                run_loader(cpu, LOADER_A, LOADED_A);
                cpu.write16(o + STEP, 4);
            }
        }
        4 => {
            run_loader(cpu, LOADER_B, LOADED_B);
            cpu.write16(o + STEP, 5);
        }
        5 => {
            run_loader(cpu, LOADER_C, LOADED_C);
            cpu.write32(o + PLACE, 0);
            cpu.write16(o + STEP, 6);
        }
        6 => load_kind(cpu, o, false),
        7 => load_kind(cpu, o, true),
        8 => {
            let v = cpu.read16(VDP1_NEXT);
            cpu.write16(VDP1_NEXT_DEFAULT, v);
            cpu.write16(o + STEP, 9);
            cpu.write8(o + OBJ_MODE, ENDING);
            nested!(cpu, 0x0600_579c, objects::obj_free(cpu, o));
        }
        _ => {}
    }
    let load = cpu.read32(f + 4);
    cpu.write32(o + LOAD, load);
}

/// A loader run (its size taken off LOAD_AT) unless its flag says it ran.
fn run_loader(cpu: &mut Cpu, loader: u32, flag: u32) {
    let game = cpu.read32(GAME);
    if cpu.read32(game + LOADED) & flag != 0 {
        return;
    }
    let at = cpu.read32(LOAD_AT);
    let size = cpu.call(loader, &[at]);
    let at = cpu.read32(LOAD_AT).wrapping_add(size);
    cpu.write32(LOAD_AT, at);
    let game = cpu.read32(GAME);
    let flags = cpu.read32(game + LOADED);
    cpu.write32(game + LOADED, flags | flag);
}

/// The list's next kind (a sprite's, or a model's): its load started (unless the game has it),
/// and once loaded, fixed and put in place.
fn load_kind(cpu: &mut Cpu, o: u32, models: bool) {
    let f = cpu.stack();
    let place = cpu.read32(o + PLACE);
    let k = cpu.read8(KIND_LIST.wrapping_add(place)) as i8;
    if k == if models { MODELS_END } else { SPRITES_END } {
        cpu.write16(o + STEP, if models { 8 } else { 7 });
        return;
    }
    let k = k as i32 as u32;
    let bit = nested!(cpu, 0x0600_69d8, system::shift_left(cpu, 1, place));
    cpu.write32(f, bit);
    if models {
        cpu.write32(f + 8, o + LOADING);
    }
    let game = cpu.read32(GAME);
    if cpu.read8(o + LOADING) == 0 {
        if cpu.read32(game + LOADED) & bit != 0 {
            cpu.write32(o + PLACE, place.wrapping_add(1));
            return;
        }
        let at = cpu.read32(LOAD_AT);
        let load = if models {
            nested!(cpu, 0x0604_5910, kind_load(cpu, k, at))
        } else {
            nested!(cpu, 0x0604_56cc, kind_load(cpu, k, at))
        };
        cpu.write32(f + 4, load);
        if load != 0 {
            cpu.write8(o + LOADING, 1);
        }
        return;
    }
    let load = cpu.read32(f + 4);
    if nested!(cpu, 0x0600_6074, files::load_done(cpu, load)) == 0 {
        return;
    }
    let at = cpu.read32(LOAD_AT);
    let (vram, first) = (cpu.read32(VRAM_NEXT), cpu.read16(CHAR_NEXT) as i16 as i32 as u32);
    let colour = cpu.read16(COLOUR_NEXT) as u32;
    let next = if models {
        nested!(cpu, 0x0604_583c, model_kind_fix(cpu, k, at));
        nested!(cpu, 0x0604_5950, model_kind_install(cpu, k, vram, first, colour))
    } else {
        nested!(cpu, 0x0604_5638, sprite_kind_fix(cpu, k, at));
        nested!(cpu, 0x0604_570c, sprite_kind_install(cpu, k, vram, first, colour))
    };
    cpu.write32(VRAM_NEXT, next);
    let p = cpu.read32(kind(k) + K_FILE);
    let chars = cpu.read16(CHAR_NEXT).wrapping_add(cpu.read16(p + F_CHARS + 2));
    cpu.write16(CHAR_NEXT, chars);
    if models || !SHARED_COLOURS.contains(&k) {
        let p = cpu.read32(kind(k) + K_FILE);
        let c = cpu.read16(COLOUR_NEXT).wrapping_add(cpu.read16(p + F_COLOUR_COUNT));
        cpu.write16(COLOUR_NEXT, c);
    }
    let p = cpu.read32(kind(k) + K_FILE);
    let sizes = cpu.read32(p + F_SIZES);
    cpu.write32(LOAD_AT, sizes);
    if !models && k == 0 {
        let p = cpu.read32(KINDS + K_FILE);
        let (off, index, count) = KIND_0_VDP1;
        let at = cpu.read32(p + F_COLOURS).wrapping_add(off);
        nested!(cpu, 0x0600_5b5c, palette::set_vdp1_colours(cpu, at, index, count));
    }
    let game = cpu.read32(GAME);
    let flags = cpu.read32(game + LOADED) | cpu.read32(f);
    cpu.write32(game + LOADED, flags);
    let place = cpu.read32(o + PLACE).wrapping_add(1);
    cpu.write32(o + PLACE, place);
    cpu.write8(o + LOADING, 0);
}

/// The kinds' loader, made unless one is in list 4 already: it.
const LOADER_LIST: u32 = 4;
const LOADER_SIZE: u32 = 0x80;
const KINDS_EXEC: u32 = 0x0605_3620;

pub fn kinds_set(cpu: &mut Cpu) -> u32 {
    let mut o = cpu.read32(objects::OBJ_INF + 8 * LOADER_LIST);
    while o != 0 {
        if cpu.read32(o + objects::EXEC) == KINDS_EXEC {
            return o;
        }
        o = cpu.read32(o + objects::NEXT);
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, LOADER_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, LOADER_LIST, blocks, KINDS_EXEC));
    if o != 0 {
        nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + 0x10));
        cpu.write16(o + STEP, 0);
    }
    o
}
