//! A banner the stages put up (what it says is not known yet): its graphics loaded and a sound,
//! then a script of steps from a table, one every other frame (its two sprites of kind 5, shown
//! for three frames each time, and the shared plane switched), while kind 5's colours 1-13
//! cycle through its set colours; it ends at the script's end, or soon once the game's state is 9.

use super::director::GAME;
use super::{effects, nested, objects, screen, sound, Cpu};

/// Its fields: its mode, kind; its step, the script's place, a word (cleared), the colour
/// cycle's frame (two to a phase) and its phase (0-24).
const OBJ_MODE: u32 = 3;
const ENDING: u8 = 0x70;
const FLAGS: u32 = 0x10;
const KIND: u32 = 0x2e;
const NUMBER: u32 = 0x32;
const BEHAVIOUR: u32 = 0x38;
const STEP: u32 = 0x3c;
const PLACE: u32 = 0x3d;
const WORD_40: u32 = 0x40;
const CYCLE_FRAME: u32 = 0x60;
const PHASE: u32 = 0x64;

/// The game's state index (director.rs's) that cuts it short, the step that ends it.
const GAME_STATE_INDEX: u32 = 0x3d;
const CUT_SHORT: u8 = 9;
const END_STEP: u8 = 4;

/// The script (a function a step, none at its end), the graphics' loader, the sound.
const SCRIPT: u32 = 0x0607_2cd4;
const LOAD: u32 = 0x0605_fc38;
const SE_BANNER: u32 = 0x2d;

/// The colours: kind `KIND`'s first colour (the kinds' word +0xc), colour RAM's image and its
/// colours as set (palette.rs), the image changed.
const KINDS: u32 = 0x0607_41f0;
const KIND_COLOUR: u32 = 0xc;
const CRAM_IMAGE: u32 = 0x0603_0e40;
const CRAM_SET: u32 = 0x0602_fe30;
const CHANGED: u32 = 0x0603_0e30;
const CRAM_CHANGED: u32 = 1;
const PHASES: u32 = 24;
const TOP: i32 = 11;

pub fn banner_exec(cpu: &mut Cpu, o: u32) {
    let step = cpu.read8(o + STEP) as i8;
    if step > 0 && step <= 3 {
        let game = cpu.read32(GAME);
        if cpu.read8(game + GAME_STATE_INDEX) == CUT_SHORT {
            cpu.write8(o + STEP, END_STEP);
        }
    }
    match cpu.read8(o + STEP) {
        0 => {
            cpu.call(LOAD, &[]);
            nested!(cpu, 0x0600_50d4, sound::se_set(cpu, SE_BANNER));
            cpu.write32(o + CYCLE_FRAME, 0);
            cpu.write32(o + PHASE, 0);
            cpu.write8(o + STEP, 1);
        }
        1 => {
            let place = cpu.read8(o + PLACE) as u32;
            let f = cpu.read32(SCRIPT + 4 * place);
            if f == 0 {
                cpu.write8(o + STEP, END_STEP);
            } else {
                cpu.call(f, &[]);
                cpu.write8(o + STEP, 2);
            }
        }
        2 => {
            cpu.write16(o + WORD_40, 0);
            let place = cpu.read8(o + PLACE).wrapping_add(1);
            cpu.write8(o + PLACE, place);
            cpu.write8(o + STEP, 1);
        }
        3 => {
            let n = cpu.read16(o + WORD_40).wrapping_add(1);
            cpu.write16(o + WORD_40, n);
            if n as i16 > 59 {
                cpu.write8(o + STEP, 0);
            }
        }
        END_STEP => {
            nested!(cpu, 0x0605_fdd8, screen::set_plane_shared_next(cpu, 0));
            cpu.write8(o + OBJ_MODE, ENDING);
        }
        _ => {}
    }
    cycle(cpu, o);
}

/// Colours 1-13 set from a run of the set colours that climbs to 12 and falls back, starting at
/// the phase (folded: past 11 it starts falling).
fn cycle(cpu: &mut Cpu, o: u32) {
    let mut c = cpu.read32(o + PHASE) as i32;
    let mut falling = false;
    if c > TOP {
        c = 12 - (c - 12);
        falling = c != 0;
    }
    for k in 1..=13u32 {
        let kind = cpu.read8(o + KIND) as i8 as i32;
        let first = cpu.read16(KINDS.wrapping_add((16 * kind) as u32) + KIND_COLOUR) as u32;
        let v = cpu.read16(CRAM_SET.wrapping_add(2 * (first as i32 + c + 1) as u32));
        cpu.write16(CRAM_IMAGE + 2 * (first + k), v);
        let changed = cpu.read32(CHANGED);
        cpu.write32(CHANGED, changed | CRAM_CHANGED);
        if falling {
            c -= 1;
            if c <= 0 {
                falling = false;
            }
        } else {
            c += 1;
            if c > TOP {
                falling = true;
            }
        }
    }
    let n = cpu.read32(o + CYCLE_FRAME).wrapping_add(1);
    cpu.write32(o + CYCLE_FRAME, n);
    if n as i32 > 1 {
        cpu.write32(o + CYCLE_FRAME, 0);
        let p = cpu.read32(o + PHASE).wrapping_sub(1);
        cpu.write32(o + PHASE, p);
        if (p as i32) < 0 {
            cpu.write32(o + PHASE, PHASES);
        }
    }
}

/// The script's steps: nothing; the sprites; the shared plane 4 or 3.
pub fn banner_idle(_cpu: &mut Cpu) {}

/// Its two sprites (kind 5, frames 3 and 2, at the places in a table of bytes, the first at
/// 7/8 size), each lasting three frames.
const PLAIN_EXEC: u32 = 0x0604_dcb4;
const LAST_EXEC: u32 = 0x0604_eb80;
const SPRITE_SIZE: u32 = 0x80;
const SPRITE_LIST: u32 = 3;
const SPRITE_KIND: u8 = 5;
const SPRITE_PLACES: u32 = 0x0607_2cd0;
const CHAR_BASE: u32 = 0x0607_41fc;
const COLOUR_BANK: u16 = 0x4000;
const X: u32 = 0x14;
const COLOUR: u32 = 0x28;
const FRAME: u32 = 0x2f;
const LIFT: u32 = 0x33;
const LIFTED: u8 = 16;
const FRAMES: u16 = 3;

pub fn banner_sprites(cpu: &mut Cpu) {
    for (k, (frame, scale)) in [(3u8, 0xe000u32), (2, 0x1_0000)].into_iter().enumerate() {
        let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, SPRITE_SIZE)) as i8 as i32 as u32;
        let a = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, SPRITE_LIST, blocks, PLAIN_EXEC));
        if a == 0 {
            continue;
        }
        nested!(cpu, 0x0604_dd50, effects::actor_init(cpu, a));
        cpu.write32(a + BEHAVIOUR, LAST_EXEC);
        cpu.write8(a + KIND, SPRITE_KIND);
        cpu.write8(a + FRAME, frame);
        let kind = cpu.read8(a + KIND) as i8 as i32;
        let colour = cpu.read16(CHAR_BASE.wrapping_add((16 * kind) as u32)).wrapping_add(COLOUR_BANK);
        cpu.write16(a + COLOUR, colour);
        cpu.write8(a + NUMBER, 0xff);
        cpu.write8(a + LIFT, LIFTED);
        let at = SPRITE_PLACES + 2 * k as u32;
        let x = (cpu.read8(at) as i8 as i32) << 16;
        cpu.write32(a + X, x as u32);
        let y = (cpu.read8(at + 1) as i8 as i32) << 16;
        cpu.write32(a + X + 4, y as u32);
        cpu.write32(a + X + 8, 0);
        cpu.write32(a + X + 12, scale);
        cpu.write32(a + X + 16, scale);
        cpu.write16(a + WORD_40, FRAMES);
    }
}

pub fn banner_plane_4(cpu: &mut Cpu) {
    nested!(cpu, 0x0605_fdd8, screen::set_plane_shared_next(cpu, 4));
}

pub fn banner_plane_3(cpu: &mut Cpu) {
    nested!(cpu, 0x0605_fdd8, screen::set_plane_shared_next(cpu, 3));
}

/// Makes the banner (list 4, not drawn, kind 5). It, or 0.
const BANNER_SIZE: u32 = 0x80;
const BANNER_LIST: u32 = 4;
const BANNER_EXEC: u32 = 0x0605_13e8;
const NO_DRAW: u32 = 0x8000_0000;

pub fn banner_set(cpu: &mut Cpu) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, BANNER_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, BANNER_LIST, blocks, PLAIN_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_dd50, effects::actor_init(cpu, o));
    cpu.write32(o + BEHAVIOUR, BANNER_EXEC);
    let flags = cpu.read32(o + FLAGS);
    cpu.write32(o + FLAGS, flags | NO_DRAW);
    cpu.write8(o + KIND, SPRITE_KIND);
    cpu.write8(o + NUMBER, 0xff);
    o
}
