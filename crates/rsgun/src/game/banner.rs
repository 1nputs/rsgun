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

/// The other stages' banners' steps: each sets the shared plane (or not) and puts up pieces, each
/// a sprite of kind 5 (as banner_sprites's) placed by two bytes of a table: a behaviour (effects.rs:
/// lasting, widening, lengthening, falling), a frame, its scales (or the start's), its frames, and
/// what more it takes. A piece not made is left out.
const WIDEN_EXEC: u32 = 0x0604_eba0;
const LENGTHEN_EXEC: u32 = 0x0604_ebcc;
const LENGTHEN_WRAP_EXEC: u32 = 0x0604_ebf8;
const FALL_EXEC: u32 = 0x0604_ec34;
const SCALE_X: u32 = 0x20;
const SCALE_Y: u32 = 0x24;
const FALL_SPEED: u32 = 0x48;
const WIDEN_SPEED: u32 = 0x54;
const LENGTHEN_SPEED: u32 = 0x58;
/// The brighter pieces' colours (16 on) and their flip bits.
const BRIGHTER: u16 = 0x10;
const DROP: u32 = 0x4_0000;

#[derive(Clone, Copy)]
enum More {
    None,
    Brighter(u32),
    Widen(u32),
    Lengthen(u32),
    Drop,
}

struct Piece {
    exec: u32,
    frame: u8,
    at: u32,
    scales: Option<(u32, u32)>,
    frames: u16,
    more: More,
}

const fn last(frame: u8, at: u32, scale: Option<u32>, frames: u16) -> Piece {
    let scales = match scale {
        Some(x) => Some((x, 0x1_0000)),
        None => None,
    };
    Piece { exec: LAST_EXEC, frame, at, scales, frames, more: More::None }
}

const fn brighter(frame: u8, at: u32, scale: u32, frames: u16, flips: u32) -> Piece {
    Piece { exec: LAST_EXEC, frame, at, scales: Some((scale, 0x1_0000)), frames, more: More::Brighter(flips) }
}

const fn widen(frame: u8, at: u32, scale: u32, frames: u16, speed: u32) -> Piece {
    Piece { exec: WIDEN_EXEC, frame, at, scales: Some((scale, 0x1_0000)), frames, more: More::Widen(speed) }
}

const fn lengthen(frame: u8, at: u32, scales: (u32, u32), frames: u16, speed: u32) -> Piece {
    Piece { exec: LENGTHEN_EXEC, frame, at, scales: Some(scales), frames, more: More::Lengthen(speed) }
}

/// A piece made: it, or none.
fn piece(cpu: &mut Cpu, p: &Piece) -> Option<u32> {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, SPRITE_SIZE)) as i8 as i32 as u32;
    let a = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, SPRITE_LIST, blocks, PLAIN_EXEC));
    if a == 0 {
        return None;
    }
    nested!(cpu, 0x0604_dd50, effects::actor_init(cpu, a));
    cpu.write32(a + BEHAVIOUR, p.exec);
    cpu.write8(a + KIND, SPRITE_KIND);
    cpu.write8(a + FRAME, p.frame);
    let kind = cpu.read8(a + KIND) as i8 as i32;
    let bank = if let More::Brighter(_) = p.more { COLOUR_BANK + BRIGHTER } else { COLOUR_BANK };
    let colour = cpu.read16(CHAR_BASE.wrapping_add((16 * kind) as u32)).wrapping_add(bank);
    cpu.write16(a + COLOUR, colour);
    cpu.write8(a + NUMBER, 0xff);
    cpu.write8(a + LIFT, LIFTED);
    let x = (cpu.read8(p.at) as i8 as i32) << 16;
    cpu.write32(a + X, x as u32);
    let y = (cpu.read8(p.at + 1) as i8 as i32) << 16;
    cpu.write32(a + X + 4, y as u32);
    cpu.write32(a + X + 8, 0);
    if let Some((sx, sy)) = p.scales {
        cpu.write32(a + SCALE_X, sx);
        cpu.write32(a + SCALE_Y, sy);
    }
    match p.more {
        More::None => {}
        More::Brighter(flips) => {
            let flags = cpu.read32(a + FLAGS);
            cpu.write32(a + FLAGS, flags | flips);
        }
        More::Widen(speed) => cpu.write32(a + WIDEN_SPEED, speed),
        More::Lengthen(speed) => cpu.write32(a + LENGTHEN_SPEED, speed),
        More::Drop => {
            let y = cpu.read32(a + X + 4).wrapping_add(DROP);
            cpu.write32(a + X + 4, y);
            cpu.write32(a + FALL_SPEED, DROP.wrapping_neg());
        }
    }
    cpu.write16(a + WORD_40, p.frames);
    Some(a)
}

fn steps(cpu: &mut Cpu, plane: Option<u32>, pieces: &[Piece]) {
    if let Some(v) = plane {
        nested!(cpu, 0x0605_fdd8, screen::set_plane_shared_next(cpu, v));
    }
    for p in pieces {
        piece(cpu, p);
    }
}

/// The first: seven pairs of lengthening bars (a 49-frame wrap, one bar flat), a piece falling to
/// the table's height in 30 frames, two more.
const BARS: u32 = 0x0607_2c28;
const BARS_FLAT: u32 = 0x0607_2c38;
const BAR_SCALE: u32 = 0x1_e000;
const BAR_SPEED: u32 = 0x1300;
const BAR_FRAMES: u16 = 0x31;
const FALLING: u32 = 0x0607_2c48;
const FALL_FRAMES: i32 = 30;

pub fn banner_step_0(cpu: &mut Cpu) {
    nested!(cpu, 0x0605_fdd8, screen::set_plane_shared_next(cpu, 1));
    for i in 0..7 {
        let bars = [
            Piece { exec: LENGTHEN_WRAP_EXEC, frame: 0, at: BARS + 2 * i, scales: Some((BAR_SCALE, BAR_SCALE)), frames: BAR_FRAMES, more: More::Lengthen(BAR_SPEED) },
            Piece { exec: LENGTHEN_WRAP_EXEC, frame: 0, at: BARS_FLAT + 2 * i, scales: Some((BAR_SCALE, 0)), frames: BAR_FRAMES, more: More::Lengthen(BAR_SPEED) },
        ];
        if bars.iter().any(|p| piece(cpu, p).is_none()) {
            break;
        }
    }
    let fall = Piece { exec: FALL_EXEC, frame: 4, at: FALLING, scales: Some((0x6_0000, 0x1_0000)), frames: 0x1f, more: More::None };
    if let Some(a) = piece(cpu, &fall) {
        let to = ((cpu.read8(FALLING + 3) as i8 as i32) << 16).wrapping_sub(cpu.read32(a + X + 4) as i32);
        cpu.write32(a + FALL_SPEED, (to / FALL_FRAMES) as u32);
    }
    steps(cpu, None, &[last(1, 0x0607_2c4c, None, 3), last(6, 0x0607_2c4e, Some(0x3_4800), 3)]);
}

pub fn banner_step_1(cpu: &mut Cpu) {
    steps(cpu, Some(2), &[last(6, 0x0607_2c50, Some(0x3_4800), 3), last(6, 0x0607_2c52, Some(0x4_b800), 3)]);
}

pub fn banner_step_2(cpu: &mut Cpu) {
    steps(cpu, Some(0), &[
        last(1, 0x0607_2c54, None, 3),
        last(6, 0x0607_2c56, Some(0x1_9000), 3),
        last(6, 0x0607_2c58, Some(0x1_6900), 3),
        last(6, 0x0607_2c5a, Some(0x1_0000), 3),
    ]);
}

pub fn banner_step_3(cpu: &mut Cpu) {
    steps(cpu, Some(2), &[
        last(6, 0x0607_2c5c, Some(0x8800), 3),
        last(6, 0x0607_2c5e, Some(0x1_0000), 3),
        last(6, 0x0607_2c60, Some(0x1_8f00), 3),
    ]);
}

pub fn banner_step_4(cpu: &mut Cpu) {
    steps(cpu, Some(0), &[
        last(1, 0x0607_2c64, None, 3),
        last(6, 0x0607_2c66, Some(0x8800), 3),
        last(6, 0x0607_2c68, Some(0x8800), 3),
    ]);
}

pub fn banner_step_5(cpu: &mut Cpu) {
    steps(cpu, None, &[
        last(6, 0x0607_2c6c, Some(0x8800), 3),
        last(6, 0x0607_2c6e, Some(0x8800), 3),
        brighter(8, 0x0607_2c70, 0x2_0000, 3, 0),
        brighter(8, 0x0607_2c72, 0x2_0000, 3, 0x10),
        brighter(8, 0x0607_2c74, 0x2_0000, 3, 0x30),
        brighter(8, 0x0607_2c76, 0x2_0000, 3, 0x20),
    ]);
}

pub fn banner_step_6(cpu: &mut Cpu) {
    steps(cpu, Some(1), &[
        last(1, 0x0607_2c78, None, 3),
        last(6, 0x0607_2c7a, Some(0x1_8f00), 3),
        last(6, 0x0607_2c7c, Some(0x1_0000), 3),
    ]);
}

pub fn banner_step_7(cpu: &mut Cpu) {
    steps(cpu, Some(0), &[last(6, 0x0607_2c80, Some(0x8800), 3), last(6, 0x0607_2c82, Some(0x1_8f00), 3)]);
}

pub fn banner_step_8(cpu: &mut Cpu) {
    steps(cpu, None, &[
        last(6, 0x0607_2c84, Some(0x8800), 3),
        last(6, 0x0607_2c86, Some(0x1_0000), 3),
        last(3, 0x0607_2c88, Some(0x1_0000), 3),
        last(2, 0x0607_2c8a, Some(0x1_0000), 3),
    ]);
}

pub fn banner_step_9(cpu: &mut Cpu) {
    steps(cpu, Some(1), &[last(6, 0x0607_2c8c, Some(0x8800), 3), last(6, 0x0607_2c8e, Some(0x1_0000), 3)]);
}

pub fn banner_step_10(cpu: &mut Cpu) {
    steps(cpu, Some(0), &[
        widen(6, 0x0607_2c90, 0x2_0000, 5, 0x1_1c00),
        widen(6, 0x0607_2c92, 0, 5, 0x4000),
        brighter(8, 0x0607_2c94, 0x1_0000, 5, 0),
        brighter(8, 0x0607_2c96, 0x1_0000, 5, 0x10),
        brighter(8, 0x0607_2c98, 0x1_0000, 5, 0x30),
        brighter(8, 0x0607_2c9a, 0x1_0000, 5, 0x20),
    ]);
}

pub fn banner_step_11(cpu: &mut Cpu) {
    steps(cpu, Some(1), &[
        last(6, 0x0607_2c9c, Some(0x1_0000), 3),
        last(6, 0x0607_2c9e, Some(0x8800), 3),
        last(3, 0x0607_2ca0, Some(0x1_0000), 3),
    ]);
}

pub fn banner_step_12(cpu: &mut Cpu) {
    steps(cpu, Some(0), &[
        widen(6, 0x0607_2ca4, 0x2_0000, 5, 0xffff_c000),
        widen(6, 0x0607_2ca6, 0, 5, 0x63c0),
        last(3, 0x0607_2ca8, Some(0x1_0000), 5),
        last(2, 0x0607_2caa, Some(0x1_0000), 5),
    ]);
}

pub fn banner_step_13(cpu: &mut Cpu) {
    steps(cpu, None, &[last(6, 0x0607_2cac, Some(0xc800), 3), widen(7, 0x0607_2cae, 0x8000, 5, 0x6c00)]);
}

pub fn banner_step_14(cpu: &mut Cpu) {
    steps(cpu, None, &[
        widen(7, 0x0607_2cb0, 0x8000, 5, 0xffff_f000),
        widen(6, 0x0607_2cb2, 0x8000, 7, 0x1_22aa),
        last(3, 0x0607_2cb4, Some(0x1_0000), 5),
        last(2, 0x0607_2cb6, Some(0x1_0000), 5),
    ]);
}

pub fn banner_plane_1(cpu: &mut Cpu) {
    nested!(cpu, 0x0605_fdd8, screen::set_plane_shared_next(cpu, 1));
}

pub fn banner_plane_0(cpu: &mut Cpu) {
    nested!(cpu, 0x0605_fdd8, screen::set_plane_shared_next(cpu, 0));
}

pub fn banner_step_15(cpu: &mut Cpu) {
    steps(cpu, None, &[last(6, 0x0607_2cb8, Some(0x1_8f00), 3), last(7, 0x0607_2cba, Some(0x1_0000), 3)]);
}

pub fn banner_step_16(cpu: &mut Cpu) {
    steps(cpu, None, &[
        widen(6, 0x0607_2cbc, 0x1_a800, 5, 0xffff_c800),
        widen(7, 0x0607_2cbe, 0x3_0000, 5, 0xffff_7200),
        lengthen(3, 0x0607_2cc0, (0x1_0000, 0x4000), 5, 0x7000),
        lengthen(2, 0x0607_2cc2, (0x1_0000, 0x8000), 5, 0x6000),
    ]);
}

pub fn banner_step_17(cpu: &mut Cpu) {
    steps(cpu, None, &[
        last(6, 0x0607_2cc4, Some(0x1_0000), 3),
        last(7, 0x0607_2cc6, Some(0x2900), 3),
        last(3, 0x0607_2cc8, Some(0x1_0000), 5),
        Piece { exec: FALL_EXEC, frame: 2, at: 0x0607_2cca, scales: Some((0x1_0000, 0x1_0000)), frames: 5, more: More::Drop },
        last(5, 0x0607_2ccc, Some(0x6_0000), 3),
    ]);
}

/// A timeline event: the banner put up (done at once).
pub fn banner_start(cpu: &mut Cpu) -> u32 {
    nested!(cpu, 0x0605_1598, banner_set(cpu));
    1
}
