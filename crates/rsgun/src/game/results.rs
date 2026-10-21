//! The results after a stage: a controller (an actor of its own) puts up its pieces one after
//! another (the labels fly in, the values slide in and count up, blinking once they are reached),
//! each waiting for the controller's step; a button press skips to the end. Once the controller
//! has added up the scores the pieces sink away.

use super::{actors, effects, hits, math, nested, objects, sound, sprites, Cpu};

/// An actor's fields as the pieces use them: its mode, flags (MODEL_DRAW), place (x, y, z),
/// scales (x, y), colour, kind and frame, lift, its own draw; its step and two bytes, a word
/// (its count, or the number drawn), what it wants (its rates; a target, a value), the
/// controller (an actor: its step) and a long more.
const OBJ_MODE: u32 = 3;
const ENDING: u8 = 0x70;
const FLAGS: u32 = 0x10;
const SPRITE: u32 = 0x10;
const X: u32 = 0x14;
const Y: u32 = 0x18;
const SCALE_X: u32 = 0x20;
const SCALE_Y: u32 = 0x24;
const COLOUR: u32 = 0x28;
const KIND: u32 = 0x2e;
const FRAME: u32 = 0x2f;
const LIFT: u32 = 0x33;
const OWN_DRAW: u32 = 0x34;
const STEP: u32 = 0x3c;
const BYTE_3D: u32 = 0x3d;
const BYTE_3E: u32 = 0x3e;
const COUNT: u32 = 0x40;
const CONTROLLER: u32 = 0x5c;
const WANT: u32 = 0x60;
const LONG_64: u32 = 0x64;
const LONG_6C: u32 = 0x6c;

/// The flags: drawn by its own draw (as a model).
const MODEL_DRAW: u32 = 0xc0_0000;
/// The pieces' kind and colour (the kind's colour word), and their lift while they move and once
/// in place.
const PIECE_KIND: u8 = 4;
const PIECE_COLOUR: u32 = 0x0607_423c;
const MOVING_LIFT: u8 = 16;
const PLACED_LIFT: u8 = 127;
/// The controller's steps the pieces watch: past SKIPPED the rest is shown at once, past ADDED
/// they sink away.
const SKIPPED: i8 = 17;
const ADDED: i8 = 18;
/// The last mode of the controller's still running.
const RUNNING: i8 = 0x6f;

const SE_ARRIVE: u32 = 0x2e;
const SE_NONE: u32 = 0x36;
const SE_SOME: u32 = 0x2f;
const SE_PERFECT: u32 = 0x37;

const SL_DIV: u32 = 0x0606_62d8;

/// The controller's step (a signed byte).
fn controller_step(cpu: &mut Cpu, o: u32) -> i8 {
    let c = cpu.read32(o + CONTROLLER);
    cpu.read8(c + STEP) as i8
}

/// Whether the controller ended: the piece ends with it.
fn controller_gone(cpu: &mut Cpu, o: u32) -> bool {
    let c = cpu.read32(o + CONTROLLER);
    if cpu.read8(c + OBJ_MODE) as i8 > RUNNING {
        cpu.write8(o + OBJ_MODE, ENDING);
        return true;
    }
    false
}

/// A piece's start: the pieces' kind and colour, lifted while it moves.
fn piece_start(cpu: &mut Cpu, o: u32) {
    cpu.write8(o + KIND, PIECE_KIND);
    let colour = cpu.read16(PIECE_COLOUR);
    cpu.write16(o + COLOUR, colour);
    cpu.write8(o + LIFT, MOVING_LIFT);
}

/// Sinking away: 4 pixels a frame, ending once 128 below.
const SINK: u32 = 0x4_0000;
const SUNK: i32 = -0x80_0000;
const SUNK_STEP: u8 = 100;

pub fn piece_sink(cpu: &mut Cpu, o: u32) {
    let y = cpu.read32(o + Y).wrapping_sub(SINK);
    cpu.write32(o + Y, y);
    if y as i32 <= SUNK {
        cpu.write8(o + OBJ_MODE, ENDING);
        cpu.write8(o + STEP, SUNK_STEP);
    }
}

/// A label flying in to its place (WANT: x, y, z) in a count of frames, arriving once the
/// controller has gone past the step it waits on (LONG_6C) or the count runs out.
const FLY_SPEED: u32 = 0x44;

pub fn piece_fly(cpu: &mut Cpu, o: u32) {
    if controller_gone(cpu, o) {
        return;
    }
    match cpu.read8(o + STEP) {
        0 => {
            piece_start(cpu, o);
            let n = cpu.read16(o + COUNT) as i16 as i32 as u32;
            for k in 0..3 {
                let d = cpu.read32(o + WANT + 4 * k).wrapping_sub(cpu.read32(o + X + 4 * k));
                let v = cpu.call(SL_DIV, &[d, n]);
                cpu.write32(o + FLY_SPEED + 4 * k, v);
            }
            cpu.write8(o + STEP, 1);
        }
        1 => {
            if controller_step(cpu, o) as i32 != cpu.read32(o + LONG_6C) as i32 {
                cpu.write8(o + STEP, 2);
            }
            for k in 0..3 {
                let v = cpu.read32(o + X + 4 * k).wrapping_add(cpu.read32(o + FLY_SPEED + 4 * k));
                cpu.write32(o + X + 4 * k, v);
            }
            let n = cpu.read16(o + COUNT).wrapping_sub(1);
            cpu.write16(o + COUNT, n);
            if n == 0 {
                nested!(cpu, 0x0600_50d4, sound::se_set(cpu, SE_ARRIVE));
                cpu.write8(o + STEP, 2);
            }
        }
        2 => {
            for k in 0..3 {
                let v = cpu.read32(o + WANT + 4 * k);
                cpu.write32(o + X + 4 * k, v);
            }
            cpu.write8(o + LIFT, PLACED_LIFT);
            cpu.write8(o + STEP, 3);
        }
        3 => {
            if controller_step(cpu, o) > ADDED {
                cpu.write8(o + STEP, 4);
            }
        }
        4 => nested!(cpu, 0x0605_1604, piece_sink(cpu, o)),
        _ => {}
    }
}

/// A value sliding in from the left to its x (WANT) at 16 pixels a frame, then bouncing back
/// past it slowing by a twentieth of its speed a frame, until it is back; its sound (by its
/// frame, FRAME_NONE or FRAME_SOME) once in place.
const SLIDE_SPEED: u32 = 0x44;
const SLIDE_SLOW: u32 = 0x4c;
const SLIDE_START: u32 = 0x10_0000;

pub fn piece_slide(cpu: &mut Cpu, o: u32) {
    if controller_gone(cpu, o) {
        return;
    }
    match cpu.read8(o + STEP) {
        0 => {
            piece_start(cpu, o);
            cpu.write32(o + SLIDE_SPEED, SLIDE_START);
            cpu.write8(o + STEP, 1);
        }
        1 => {
            if controller_step(cpu, o) > SKIPPED {
                cpu.write8(o + STEP, 3);
            }
            let speed = cpu.read32(o + SLIDE_SPEED);
            let x = cpu.read32(o + X).wrapping_add(speed);
            cpu.write32(o + X, x);
            if x as i32 >= cpu.read32(o + WANT) as i32 {
                let slow = (cpu.read32(o + SLIDE_SPEED) as i32 / 20).wrapping_neg();
                cpu.write32(o + SLIDE_SLOW, slow as u32);
                cpu.write8(o + STEP, 2);
            }
        }
        2 => {
            if controller_step(cpu, o) > SKIPPED {
                cpu.write8(o + STEP, 3);
            }
            let speed = cpu.read32(o + SLIDE_SPEED).wrapping_add(cpu.read32(o + SLIDE_SLOW));
            cpu.write32(o + SLIDE_SPEED, speed);
            let x = cpu.read32(o + X).wrapping_add(speed);
            cpu.write32(o + X, x);
            let want = cpu.read32(o + WANT);
            if x as i32 <= want as i32 {
                cpu.write32(o + X, want);
                cpu.write8(o + STEP, 3);
            }
        }
        3 => {
            if cpu.read8(o + FRAME) == FRAME_NONE {
                nested!(cpu, 0x0600_50d4, sound::se_set(cpu, SE_NONE));
            }
            if cpu.read8(o + FRAME) == FRAME_SOME {
                nested!(cpu, 0x0600_50d4, sound::se_set(cpu, SE_SOME));
            }
            cpu.write8(o + LIFT, PLACED_LIFT);
            let want = cpu.read32(o + WANT);
            cpu.write32(o + X, want);
            cpu.write8(o + STEP, 4);
        }
        4 => {
            if controller_step(cpu, o) > ADDED {
                cpu.write8(o + STEP, 5);
            }
        }
        5 => nested!(cpu, 0x0605_1604, piece_sink(cpu, o)),
        _ => {}
    }
}

/// The digits (frames 0-9 of the sprite's kind), 16 pixels apart, the last at the sprite's x and
/// the number's middle there; each a copy of the sprite at `t` (a struct on the stack).
const S_POS: u32 = 4;
const S_COLOUR: u32 = 0x18;
const S_KIND: u32 = 0x1e;
const S_FRAME: u32 = 0x1f;
const S_LIFT: u32 = 0x23;
const SPRITE_SIZE: u32 = 44;
const HALF_DIGIT: u32 = 0x8_0000;
const DIGIT: u32 = 0x10_0000;
const PERCENT: u8 = 10;

/// `t` set up as a copy of the actor's sprite (place, scales, kind, colour), lifted.
fn digit_sprite(cpu: &mut Cpu, actor: u32, t: u32) {
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, t));
    for k in 0..5 {
        let v = cpu.read32(actor + X + 4 * k);
        cpu.write32(t + S_POS + 4 * k, v);
    }
    let kind = cpu.read8(actor + KIND);
    cpu.write8(t + S_KIND, kind);
}

/// `n`'s digits less one: how far the number reaches right of its middle, in half digits.
fn widen(cpu: &mut Cpu, t: u32, n: i32) {
    let mut q = n / 10;
    while q != 0 {
        let x = cpu.read32(t + S_POS).wrapping_add(HALF_DIGIT);
        cpu.write32(t + S_POS, x);
        q /= 10;
    }
}

/// The percent piece's draw: its label (frame 17, once BYTE_3D says) and its number (COUNT, a
/// word, unless negative) with a percent sign.
pub fn percent_draw(cpu: &mut Cpu, s: u32) {
    let actor = s - SPRITE;
    let f = cpu.stack();
    if cpu.read8(actor + BYTE_3D) != 0 {
        digit_sprite(cpu, actor, f);
        let frame = cpu.read8(actor + FRAME);
        cpu.write8(f + S_FRAME, frame);
        let colour = cpu.read16(actor + COLOUR);
        cpu.write16(f + S_COLOUR, colour);
        cpu.write8(f + S_LIFT, MOVING_LIFT);
        nested!(cpu, 0x0604_0960, sprites::draw(cpu, f));
    }
    let n = cpu.read16(actor + COUNT) as i16 as i32;
    if n < 0 {
        return;
    }
    let t = f + SPRITE_SIZE;
    digit_sprite(cpu, actor, t);
    let colour = cpu.read16(actor + COLOUR);
    cpu.write16(t + S_COLOUR, colour);
    cpu.write8(t + S_LIFT, MOVING_LIFT);
    widen(cpu, t, n);
    let x = cpu.read32(t + S_POS).wrapping_add(DIGIT);
    cpu.write32(t + S_POS, x);
    cpu.write8(t + S_FRAME, PERCENT);
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, t));
    let mut n = cpu.read16(actor + COUNT) as i16 as i32;
    loop {
        let x = cpu.read32(t + S_POS).wrapping_sub(DIGIT);
        cpu.write32(t + S_POS, x);
        let d = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, n as u32, 10));
        cpu.write8(t + S_FRAME, d as u8);
        nested!(cpu, 0x0604_0960, sprites::draw(cpu, t));
        n /= 10;
        if n == 0 {
            break;
        }
    }
}

/// The hit rate: counts up (COUNT, a word from -1) a frame to the rate (WANT), then blinks it ten
/// times (BYTE_3E the frames, LONG_6C the blinks); a perfect 100 grows its label in (scales from
/// none, GROW_SPEED and GROW_RATE) and settles it.
const GROW_SPEED: u32 = 0x54;
const GROW_RATE: u32 = 0x58;
const GROW_ACCEL: u32 = 0x10;
const GROWN: i32 = 0x1_3fff;
const SETTLE: u32 = 0x4000;
const PERFECT: u32 = 100;
const PERFECT_FRAME: u8 = 17;
const BLINK_FRAMES: i8 = 3;
const BLINKS: i32 = 9;
const ONE: u32 = 0x1_0000;

/// The value's low word (WANT's), the number shown.
const WANT_LOW: u32 = WANT + 2;

pub fn percent_exec(cpu: &mut Cpu, o: u32) {
    if controller_gone(cpu, o) {
        return;
    }
    match cpu.read8(o + STEP) {
        0 => {
            let flags = cpu.read32(o + FLAGS);
            cpu.write32(o + FLAGS, flags | MODEL_DRAW);
            cpu.write32(o + OWN_DRAW, 0x0605_1928);
            piece_start(cpu, o);
            cpu.write8(o + BYTE_3D, 0);
            cpu.write16(o + COUNT, 0xffff);
            cpu.write8(o + STEP, 1);
        }
        1 => {
            let n = cpu.read16(o + COUNT).wrapping_add(1);
            cpu.write16(o + COUNT, n);
            if controller_step(cpu, o) > SKIPPED {
                cpu.write8(o + STEP, 5);
            } else if n as i16 as i32 >= cpu.read32(o + WANT) as i32 {
                cpu.write32(o + LONG_6C, 0);
                cpu.write8(o + BYTE_3E, 0);
                cpu.write8(o + STEP, 2);
            }
        }
        2 => {
            if controller_step(cpu, o) > SKIPPED {
                cpu.write8(o + STEP, 5);
                return;
            }
            let b = cpu.read8(o + BYTE_3E).wrapping_add(1);
            cpu.write8(o + BYTE_3E, b);
            if b as i8 <= BLINK_FRAMES {
                return;
            }
            let shown = if cpu.read16(o + COUNT) as i16 >= 0 { 0xffff } else { cpu.read16(o + WANT_LOW) };
            cpu.write16(o + COUNT, shown);
            cpu.write8(o + BYTE_3E, 0);
            let blinks = cpu.read32(o + LONG_6C).wrapping_add(1);
            cpu.write32(o + LONG_6C, blinks);
            if blinks as i32 <= BLINKS {
                return;
            }
            cpu.write8(o + STEP, 6);
            if cpu.read32(o + WANT) != PERFECT {
                return;
            }
            cpu.write16(o + COUNT, 0xffff);
            cpu.write8(o + FRAME, PERFECT_FRAME);
            cpu.write8(o + BYTE_3D, 1);
            for at in [SCALE_Y, SCALE_X, GROW_SPEED, GROW_RATE] {
                cpu.write32(o + at, 0);
            }
            cpu.write8(o + STEP, 3);
        }
        3 => {
            if controller_step(cpu, o) > SKIPPED {
                cpu.write8(o + STEP, 5);
                return;
            }
            let rate = cpu.read32(o + GROW_RATE).wrapping_add(GROW_ACCEL);
            cpu.write32(o + GROW_RATE, rate);
            let speed = cpu.read32(o + GROW_SPEED).wrapping_add(rate);
            cpu.write32(o + GROW_SPEED, speed);
            let scale = cpu.read32(o + SCALE_X).wrapping_add(speed);
            cpu.write32(o + SCALE_X, scale);
            cpu.write32(o + SCALE_Y, scale);
            if cpu.read32(o + SCALE_X) as i32 > GROWN {
                cpu.write8(o + STEP, 4);
            }
        }
        4 => {
            if controller_step(cpu, o) > SKIPPED {
                cpu.write8(o + STEP, 5);
            }
            let scale = cpu.read32(o + SCALE_X);
            let scale = nested!(cpu, 0x0604_6168, actors::approach(cpu, scale, ONE, SETTLE));
            cpu.write32(o + SCALE_X, scale);
            cpu.write32(o + SCALE_Y, scale);
            if cpu.read32(o + SCALE_X) == ONE {
                nested!(cpu, 0x0600_50d4, sound::se_set(cpu, SE_PERFECT));
                cpu.write8(o + STEP, 6);
            }
        }
        5 => {
            let shown = cpu.read16(o + WANT_LOW);
            cpu.write16(o + COUNT, shown);
            if cpu.read32(o + WANT) == PERFECT {
                cpu.write16(o + COUNT, 0xffff);
                cpu.write8(o + FRAME, PERFECT_FRAME);
                cpu.write8(o + BYTE_3D, 1);
                cpu.write32(o + SCALE_Y, ONE);
                cpu.write32(o + SCALE_X, ONE);
            }
            cpu.write8(o + LIFT, PLACED_LIFT);
            cpu.write8(o + STEP, 6);
        }
        6 => {
            if controller_step(cpu, o) > ADDED {
                cpu.write8(o + STEP, 7);
            }
        }
        7 => nested!(cpu, 0x0605_1604, piece_sink(cpu, o)),
        _ => {}
    }
}

/// The score piece's draw: its number (SHOWN, unless negative).
const SHOWN: u32 = LONG_64;

pub fn score_draw(cpu: &mut Cpu, s: u32) {
    let actor = s - SPRITE;
    if (cpu.read32(actor + SHOWN) as i32) < 0 {
        return;
    }
    let t = cpu.stack();
    digit_sprite(cpu, actor, t);
    let colour = cpu.read16(actor + COLOUR);
    cpu.write16(t + S_COLOUR, colour);
    cpu.write8(t + S_LIFT, MOVING_LIFT);
    let n = cpu.read32(actor + SHOWN) as i32;
    widen(cpu, t, n);
    let mut n = cpu.read32(actor + SHOWN) as i32;
    loop {
        let d = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, n as u32, 10));
        cpu.write8(t + S_FRAME, d as u8);
        nested!(cpu, 0x0604_0960, sprites::draw(cpu, t));
        n /= 10;
        let x = cpu.read32(t + S_POS).wrapping_sub(DIGIT);
        cpu.write32(t + S_POS, x);
        if n == 0 {
            break;
        }
    }
}

/// A score: counts up (SHOWN from -1) by a hundredth of the score (WANT) and a random part of a
/// five hundredth a frame, then blinks it ten times (BYTE_3E the frames, BYTE_3D the blinks).
pub fn score_exec(cpu: &mut Cpu, o: u32) {
    if controller_gone(cpu, o) {
        return;
    }
    match cpu.read8(o + STEP) {
        0 => {
            let flags = cpu.read32(o + FLAGS);
            cpu.write32(o + FLAGS, flags | MODEL_DRAW);
            cpu.write32(o + OWN_DRAW, 0x0605_1cf0);
            piece_start(cpu, o);
            cpu.write32(o + SHOWN, 0xffff_ffff);
            cpu.write8(o + STEP, 1);
        }
        1 => {
            if controller_step(cpu, o) > SKIPPED {
                cpu.write8(o + STEP, 3);
                return;
            }
            let want = cpu.read32(o + WANT) as i32;
            let r = nested!(cpu, 0x0604_5d90, math::random(cpu, (want / 500 + 1) as u32));
            let shown = (cpu.read32(o + SHOWN) as i32)
                .wrapping_add(1)
                .wrapping_add((cpu.read32(o + WANT) as i32 / 100).wrapping_add(r as i32));
            cpu.write32(o + SHOWN, shown as u32);
            let want = cpu.read32(o + WANT);
            if shown >= want as i32 {
                cpu.write32(o + SHOWN, want);
                cpu.write8(o + BYTE_3D, 0);
                cpu.write8(o + BYTE_3E, 0);
                cpu.write8(o + STEP, 2);
            }
        }
        2 => {
            if controller_step(cpu, o) > SKIPPED {
                cpu.write8(o + STEP, 3);
            }
            let b = cpu.read8(o + BYTE_3E).wrapping_add(1);
            cpu.write8(o + BYTE_3E, b);
            if b as i8 <= BLINK_FRAMES {
                return;
            }
            let shown = if cpu.read32(o + SHOWN) as i32 >= 0 { 0xffff_ffff } else { cpu.read32(o + WANT) };
            cpu.write32(o + SHOWN, shown);
            cpu.write8(o + BYTE_3E, 0);
            let blinks = cpu.read8(o + BYTE_3D).wrapping_add(1);
            cpu.write8(o + BYTE_3D, blinks);
            if blinks as u32 > BLINKS as u32 {
                cpu.write8(o + STEP, 4);
            }
        }
        3 => {
            let want = cpu.read32(o + WANT);
            cpu.write32(o + SHOWN, want);
            cpu.write8(o + LIFT, PLACED_LIFT);
            cpu.write8(o + STEP, 4);
        }
        4 => {
            if controller_step(cpu, o) > ADDED {
                cpu.write8(o + STEP, 5);
            }
        }
        5 => nested!(cpu, 0x0605_1604, piece_sink(cpu, o)),
        _ => {}
    }
}

/// The controller: waits (WANT frames), then puts up the pieces one after another, waiting
/// (COUNT) after each: the four labels flying in, the first score's label sliding in (frame 12,
/// or 13 when it is none), the rate counting up, the two scores counting up. A while after it
/// adds both scores to each player's (and keeps the stage's in the scores by stage), and ends a
/// second later. Holding A and B (and C, by the BIOS's setting) or START skips each wait;
/// pressing them skips the last one too.
const RATE: u32 = 0x64;
const SCORE_1: u32 = 0x68;
const SCORE_2: u32 = 0x6c;

const PLAYERS: u32 = 0x0607_40f0;
const PAD_INF: u32 = 0x0602_ee10;
const PAD_SIZE: u32 = 8;
const PAD_TRIG: u32 = 4;
const PAD_AB: u16 = 0x500;
const PAD_ABC: u16 = 0x700;
const PAD_START: u16 = 0x800;
/// What the pads say: a wait skipped (held), every wait skipped (pressed).
const SKIP: u32 = 1;
const SKIP_ALL: u32 = 0x8000;

/// The pieces: plain actors (list 3, or 2 when it is full), never culled.
const PIECE_SIZE: u32 = 0x80;
const PLAIN_EXEC: u32 = 0x0604_dcb4;
const UNCULLED: u32 = 0x2000_0000;
const NUMBER: u32 = 0x32;
const BEHAVIOUR: u32 = 0x38;
const Z: u32 = 0x1c;
const PIECE_FLY: u32 = 0x0605_1634;
const PIECE_SLIDE: u32 = 0x0605_17b0;
const PERCENT_EXEC: u32 = 0x0605_1a98;
const SCORE_EXEC: u32 = 0x0605_1de0;
const PIECE_FRAMES: u16 = 10;
/// The labels' start (16 above, 160 nearer) and the first score's label's (256 left, 36 above).
const LABEL_RISE: u32 = 0xfff0_0000;
const LABEL_NEAR: u32 = 0xff60_0000;
const SLIDE_LEFT: u32 = 0xff00_0000;
const SLIDE_RISE: u32 = 0xffdc_0000;
const FRAME_NONE: u8 = 13;
const FRAME_SOME: u8 = 12;
/// The tally's wait, the end's, the step after the end.
const TALLY_WAIT: u16 = 180;
const END_WAIT: u16 = 60;
const DONE: u8 = 20;
/// The scores' weapon number for score_kill.
const TALLY_WEAPON: u32 = 18;

use super::globals::{bios_setting_is_1, GAME_MODE, PLAY_DATA, PLAY_DATA_SIZE, SCORE};
/// Each player's scores by stage (5 a stage, by MODE_SUB, from the first stage mode) and the
/// score last kept.
const STAGE_SCORES: u32 = 0x48;
const KEPT_SCORE: u32 = 0xc0;
const STAGE_MODE_FIRST: u32 = 6;
const MODE_SUB: u32 = 0x0600_ebb1;

fn pads_skip(cpu: &mut Cpu) -> u32 {
    let both = if bios_setting_is_1(cpu) { PAD_ABC } else { PAD_AB };
    let mut skip = 0;
    for p in 0..2 {
        if cpu.read32(PLAYERS + 4 * p) == 0 {
            continue;
        }
        let held = cpu.read16(PAD_INF + PAD_SIZE * p);
        if held & both == both {
            skip = SKIP;
        }
        if held & PAD_START != 0 {
            skip = SKIP;
        }
        if cpu.read16(PAD_INF + PAD_SIZE * p + PAD_TRIG) & (both | PAD_START) != 0 {
            skip = SKIP_ALL;
        }
    }
    skip
}

/// A new piece with behaviour `behaviour`, or none.
fn piece_new(cpu: &mut Cpu, behaviour: u32) -> Option<u32> {
    let mut a = 0;
    for list in [3, 2] {
        let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, PIECE_SIZE)) as i8 as i32 as u32;
        a = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, list, blocks, PLAIN_EXEC));
        if a != 0 {
            break;
        }
    }
    if a == 0 {
        return None;
    }
    nested!(cpu, 0x0604_dd50, effects::actor_init(cpu, a));
    let flags = cpu.read32(a + FLAGS);
    cpu.write32(a + FLAGS, flags | UNCULLED);
    cpu.write8(a + NUMBER, 0xff);
    cpu.write32(a + BEHAVIOUR, behaviour);
    Some(a)
}

/// The piece told the controller's step it waits on (the next) and the controller's wait after it.
fn piece_link(cpu: &mut Cpu, o: u32, a: u32, waits: u32, wait: u16) {
    cpu.write32(a + LONG_6C, waits);
    cpu.write16(a + COUNT, PIECE_FRAMES);
    cpu.write32(a + CONTROLLER, o);
    cpu.write16(o + COUNT, wait);
}

/// A label flying in from above and nearer to `rise` below the controller.
fn label(cpu: &mut Cpu, o: u32, frame: u8, rise: u32, step: u8, wait: u16) {
    let Some(a) = piece_new(cpu, PIECE_FLY) else { return };
    cpu.write8(a + FRAME, frame);
    let (x, y, z) = (cpu.read32(o + X), cpu.read32(o + Y), cpu.read32(o + Z));
    cpu.write32(a + X, x);
    cpu.write32(a + Y, y.wrapping_add(LABEL_RISE));
    cpu.write32(a + Z, z.wrapping_add(LABEL_NEAR));
    cpu.write32(a + WANT, x);
    cpu.write32(a + WANT + 4, y.wrapping_add(rise));
    cpu.write32(a + WANT + 8, z);
    piece_link(cpu, o, a, step as u32, wait);
    cpu.write8(o + STEP, step);
}

/// A value counting up (behaviour `exec`) `drop` below the controller.
fn value(cpu: &mut Cpu, o: u32, exec: u32, drop: u32, from: u32, step: u8) {
    let Some(a) = piece_new(cpu, exec) else { return };
    let (x, y, z) = (cpu.read32(o + X), cpu.read32(o + Y), cpu.read32(o + Z));
    cpu.write32(a + X, x);
    cpu.write32(a + Y, y.wrapping_add(drop));
    cpu.write32(a + Z, z);
    let v = cpu.read32(o + from);
    cpu.write32(a + WANT, v);
    let waits = if exec == SCORE_EXEC { 14 } else { step as u32 };
    piece_link(cpu, o, a, waits, 30);
    cpu.write8(o + STEP, step);
}

pub fn results_exec(cpu: &mut Cpu, o: u32) {
    let skip = pads_skip(cpu);
    let step = cpu.read8(o + STEP);
    match step {
        0 => {
            let t = cpu.read32(o + WANT);
            if t != 0 {
                cpu.write32(o + WANT, t - 1);
            } else {
                cpu.write8(o + STEP, 1);
            }
        }
        1 => label(cpu, o, 11, 0xffbe_0000, 2, 60),
        3 => label(cpu, o, 14, 0xfff0_0000, 4, 10),
        5 => label(cpu, o, 15, 0x0018_0000, 6, 10),
        7 => label(cpu, o, 16, 0x0040_0000, 8, 10),
        9 => {
            let Some(a) = piece_new(cpu, PIECE_SLIDE) else { return };
            let frame = if cpu.read32(o + SCORE_1) == 0 { FRAME_NONE } else { FRAME_SOME };
            cpu.write8(a + FRAME, frame);
            let (x, y, z) = (cpu.read32(o + X), cpu.read32(o + Y), cpu.read32(o + Z));
            cpu.write32(a + X, x.wrapping_add(SLIDE_LEFT));
            cpu.write32(a + Y, y.wrapping_add(SLIDE_RISE));
            cpu.write32(a + Z, z);
            cpu.write32(a + WANT, x);
            piece_link(cpu, o, a, 10, 30);
            cpu.write8(o + STEP, 10);
        }
        11 => value(cpu, o, PERCENT_EXEC, 0, RATE, 12),
        13 => value(cpu, o, SCORE_EXEC, 0x0028_0000, SCORE_1, 14),
        15 => value(cpu, o, SCORE_EXEC, 0x0050_0000, SCORE_2, 16),
        2 | 4 | 6 | 8 | 10 | 12 | 14 | 16 => {
            let n = cpu.read16(o + COUNT).wrapping_sub(1);
            cpu.write16(o + COUNT, n);
            if skip != 0 {
                cpu.write16(o + COUNT, 0);
            }
            if cpu.read16(o + COUNT) != 0 {
                return;
            }
            if step == 16 {
                cpu.write16(o + COUNT, TALLY_WAIT);
            }
            cpu.write8(o + STEP, step + 1);
        }
        17 | 18 => {
            if step == 17 && skip != 0 {
                cpu.write8(o + STEP, 18);
            }
            let n = cpu.read16(o + COUNT).wrapping_sub(1);
            cpu.write16(o + COUNT, n);
            if skip == SKIP_ALL {
                cpu.write16(o + COUNT, 0);
            }
            if cpu.read16(o + COUNT) != 0 {
                return;
            }
            tally(cpu, o);
            cpu.write16(o + COUNT, END_WAIT);
            cpu.write8(o + STEP, 19);
        }
        19 => {
            let n = cpu.read16(o + COUNT).wrapping_sub(1);
            cpu.write16(o + COUNT, n);
            if n == 0 {
                cpu.write8(o + OBJ_MODE, ENDING);
                cpu.write8(o + STEP, DONE);
            }
        }
        _ => {}
    }
}

/// Both scores added to each player's; what the player made since the score last kept goes into
/// the stage's (and route's) place in the scores by stage, rounded down to tens.
fn tally(cpu: &mut Cpu, o: u32) {
    for p in 0..2 {
        if cpu.read32(PLAYERS + 4 * p) == 0 {
            continue;
        }
        let points = cpu.read32(o + SCORE_1).wrapping_add(cpu.read32(o + SCORE_2));
        nested!(cpu, 0x0604_3108, hits::score_kill(cpu, p, TALLY_WEAPON, 0, points));
        nested!(cpu, 0x0604_5090, hits::chain_reset(cpu, p));
        let data = PLAY_DATA + PLAY_DATA_SIZE * p;
        let at = stage_score(cpu, data);
        let made = cpu.read32(data + SCORE).wrapping_sub(cpu.read32(data + KEPT_SCORE));
        cpu.write32(at, made);
        let score = cpu.read32(data + SCORE);
        cpu.write32(data + KEPT_SCORE, score);
        let at = stage_score(cpu, data);
        let tens = cpu.read32(at) as i32 / 10;
        cpu.write32(at, tens as u32);
        let at = stage_score(cpu, data);
        let v = cpu.read32(at).wrapping_mul(10);
        cpu.write32(at, v);
    }
    let sub = cpu.read8(MODE_SUB).wrapping_add(1);
    cpu.write8(MODE_SUB, sub);
}

fn stage_score(cpu: &mut Cpu, data: u32) -> u32 {
    let mode = cpu.read8(GAME_MODE) as i8 as i32 as u32;
    let sub = cpu.read8(MODE_SUB) as i8 as i32 as u32;
    data.wrapping_add(STAGE_SCORES)
        .wrapping_add(20u32.wrapping_mul(mode.wrapping_sub(STAGE_MODE_FIRST)))
        .wrapping_add(4u32.wrapping_mul(sub))
}

/// Makes the results' controller, starting after `delay` frames: the rate (a percentage) and
/// the first score as the stage left them, the second the rate's worth 500 a point (a perfect
/// 100 counted twice). It, or 0.
const RESULTS_LIST: u32 = 4;
const RESULTS_EXEC: u32 = 0x0605_1f94;
const NO_DRAW: u32 = 0x8000_0000;
const STAGE_RATE: u32 = 0x0607_46f0;
const STAGE_SCORE: u32 = 0x0607_46f4;
const RATE_POINTS: u32 = 500;
const RESULTS_Y: u32 = 0x4_0000;

pub fn results_set(cpu: &mut Cpu, delay: u32) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, PIECE_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, RESULTS_LIST, blocks, PLAIN_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_dd50, effects::actor_init(cpu, o));
    cpu.write32(o + BEHAVIOUR, RESULTS_EXEC);
    let flags = cpu.read32(o + FLAGS);
    cpu.write32(o + FLAGS, flags | NO_DRAW);
    cpu.write8(o + KIND, PIECE_KIND);
    cpu.write8(o + NUMBER, 0xff);
    cpu.write32(o + X, 0);
    cpu.write32(o + Y, RESULTS_Y);
    cpu.write32(o + Z, 0);
    cpu.write32(o + WANT, delay);
    let rate = cpu.read32(STAGE_RATE);
    cpu.write32(o + RATE, rate);
    let score = cpu.read32(STAGE_SCORE);
    cpu.write32(o + SCORE_1, score);
    let counted = if rate == PERFECT { rate * 2 } else { rate };
    cpu.write32(o + SCORE_2, counted.wrapping_mul(RATE_POINTS));
    o
}
