//! The explosions' pieces: plain actors (actors.rs's plain_exec) that the explosions throw out:
//! twin sparks (a polygon between two points drifting apart, flickering), embers circling what
//! burns, debris flung out in a random direction (in 3D, through SGL's matrix) slowing to a stop,
//! and pieces that shrink away. All end once the screen is handed over.

use super::{director, math, nested, objects, sprites, Cpu};

/// The objects' mode byte (ending).
const OBJ_MODE: u32 = 3;
const ENDING: u8 = 0x70;
/// An actor's sprite (its flags first), position, scales, colour, kind, frame, angle, lift, its
/// corners (polygons), behaviour.
const SPRITE: u32 = 0x10;
const FLAGS: u32 = 0x10;
const POSITION: u32 = 0x14;
const SCALES: u32 = 0x20;
const COLOUR: u32 = 0x28;
const KIND: u32 = 0x2e;
const FRAME: u32 = 0x2f;
const ANGLE: u32 = 0x30;
const LIFT: u32 = 0x33;
const CORNERS_AT: u32 = 0x34;
const BEHAVIOUR: u32 = 0x38;
/// The flags: never culled with an attribute bit (0x1000), the attribute bit 0x100 (some pieces
/// take it at random), a polygon of four corners.
const PIECE_FLAGS: u32 = 0x2000_1000;
const ATTR_100: u32 = 0x100;
const FOUR_CORNERS: u32 = 1;

/// The first character's colour (the colours are from it), the game's frames.
const CHAR_BASE: u32 = 0x0607_41fc;
const GAME_FRAMES: u32 = super::globals::GAME_FRAMES;
/// The director's flags (director.rs) and the bit that hands the screen over.
const HANDED_OVER: u32 = 8;
const PLAIN_EXEC: u32 = 0x0604_dcb4;

/// SGL's.
const SL_SIN: u32 = 0x0606_9a0c;
const SL_COS: u32 = 0x0606_9a30;
const SL_ATAN: u32 = 0x0606_780c;
const SL_PUSH_MATRIX: u32 = 0x0606_71cc;
const SL_POP_MATRIX: u32 = 0x0606_7200;
const SL_TRANSLATE: u32 = 0x0606_721c;
const SL_ROT_X: u32 = 0x0606_72ec;
const SL_ROT_Z: u32 = 0x0606_726c;
const SL_CALC_POINT: u32 = 0x0606_db2c;

fn end(cpu: &mut Cpu, o: u32) {
    cpu.write8(o + OBJ_MODE, ENDING);
}

fn handed_over(cpu: &mut Cpu) -> bool {
    nested!(cpu, 0x0604_0cb4, director::director_flags(cpu)) & HANDED_OVER != 0
}

fn random(cpu: &mut Cpu, n: u32) -> u32 {
    nested!(cpu, 0x0604_5d90, math::random(cpu, n))
}

/// A piece in list 3 (else 2) of `size` bytes: it, or 0.
fn piece_in(cpu: &mut Cpu, lists: &[u32], size: u32) -> u32 {
    for &list in lists {
        let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, size)) as i8 as i32 as u32;
        let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, list, blocks, PLAIN_EXEC));
        if o != 0 {
            return o;
        }
    }
    0
}

/// `v` over 2^`shift`, rounded toward zero.
fn shifted(v: i32, shift: u32) -> u32 {
    let rounded = if v < 0 { v.wrapping_add((1 << shift) - 1) } else { v };
    (rounded >> shift) as u32
}

/// A shrinking piece's speeds: its scales' (x, y, taken off), its own (x, y).
const SHRINK: u32 = 0x5c;
const SPEED: u32 = 0x54;

/// A shrinking piece's behaviour: its scales shrunk (ended once either is gone), moved by its
/// speed.
pub fn shrink_exec(cpu: &mut Cpu, o: u32) {
    for k in 0..2 {
        let v = cpu.read32(o + SCALES + 4 * k).wrapping_sub(cpu.read32(o + SHRINK + 4 * k));
        cpu.write32(o + SCALES + 4 * k, v);
    }
    if cpu.read32(o + SCALES) as i32 <= 0 || cpu.read32(o + SCALES + 4) as i32 <= 0 {
        end(cpu, o);
    }
    for k in 0..2 {
        let p = cpu.read32(o + POSITION + 4 * k).wrapping_add(cpu.read32(o + SPEED + 4 * k));
        cpu.write32(o + POSITION + 4 * k, p);
    }
}

/// A twin spark: its two points (x, y each), their speeds and their accelerations (each a drag of
/// a 128th of its speed), its rise and the rise's change, its age (a byte; past 100 it ends),
/// which frames show the first point (a bit mask with frames' bit 0), its corners (four, x and y
/// words, jittered by a pixel).
const TWIN_A: u32 = 0x3c;
const TWIN_B: u32 = 0x44;
const TWIN_A_SPEED: u32 = 0x4c;
const TWIN_A_ACCEL: u32 = 0x54;
const TWIN_B_SPEED: u32 = 0x5c;
const TWIN_B_ACCEL: u32 = 0x64;
const TWIN_RISE: u32 = 0x6c;
const TWIN_RISE_CHANGE: u32 = 0x70;
const TWIN_AGE: u32 = 0x74;
const TWIN_SHOWS_A: u32 = 0x75;
const TWIN_CORNERS: u32 = 0x76;
const TWIN_SIZE: u32 = 0x88;
const TWIN_EXEC: u32 = 0x0605_80bc;
const TWIN_LAST_AGE: i8 = 100;
const TWIN_SPIN: u16 = 0x1000;
/// Its colours: a tenth of its age from the first character's, and 0x50 further on even frames.
const TWIN_EVEN_COLOURS: u16 = 0x50;

/// A twin spark's behaviour: ended once the screen is handed over or past its last age; else
/// spun, risen, its points moved, aged by 1 or 2, coloured by its age, placed at one point or the
/// other, its corners jittered.
pub fn twin_exec(cpu: &mut Cpu, o: u32) {
    if handed_over(cpu) || cpu.read8(o + TWIN_AGE) as i8 > TWIN_LAST_AGE {
        end(cpu, o);
        return;
    }
    let a = cpu.read16(o + ANGLE).wrapping_add(TWIN_SPIN);
    cpu.write16(o + ANGLE, a);
    let z = cpu.read32(o + POSITION + 8).wrapping_add(cpu.read32(o + TWIN_RISE));
    cpu.write32(o + POSITION + 8, z);
    let rise = cpu.read32(o + TWIN_RISE).wrapping_add(cpu.read32(o + TWIN_RISE_CHANGE));
    cpu.write32(o + TWIN_RISE, rise);
    for (point, speed, accel) in [(TWIN_A, TWIN_A_SPEED, TWIN_A_ACCEL), (TWIN_B, TWIN_B_SPEED, TWIN_B_ACCEL)] {
        for k in 0..2 {
            let p = cpu.read32(o + point + 4 * k).wrapping_add(cpu.read32(o + speed + 4 * k));
            cpu.write32(o + point + 4 * k, p);
        }
        for k in 0..2 {
            let v = cpu.read32(o + speed + 4 * k).wrapping_add(cpu.read32(o + accel + 4 * k));
            cpu.write32(o + speed + 4 * k, v);
        }
    }
    let r = random(cpu, 0x10000);
    let age = cpu.read8(o + TWIN_AGE).wrapping_add(1).wrapping_add((r & 1) as u8);
    cpu.write8(o + TWIN_AGE, age);
    let even = cpu.read32(GAME_FRAMES) & 1 == 0;
    let age = cpu.read8(o + TWIN_AGE) as i8 as i32 as u32;
    let tenth = nested!(cpu, 0x0600_6a78, math::quotient(cpu, age, 10)) as u16;
    let colour = if even { tenth + TWIN_EVEN_COLOURS } else { tenth };
    let colour = colour.wrapping_add(cpu.read16(CHAR_BASE));
    cpu.write16(o + COLOUR, colour);
    let frames = cpu.read32(GAME_FRAMES);
    let point = if cpu.read8(o + TWIN_SHOWS_A) as u32 & frames != 0 && frames & 1 != 0 { TWIN_A } else { TWIN_B };
    for k in 0..2 {
        let v = cpu.read32(o + point + 4 * k);
        cpu.write32(o + POSITION + 4 * k, v);
    }
    for corner in 0..4 {
        let at = o + TWIN_CORNERS + 4 * corner;
        if corner % 2 == 0 {
            for k in 0..2 {
                let r = random(cpu, 0x10000);
                cpu.write16(at + 2 * k, (r & 1) as u16);
            }
        } else {
            cpu.write16(at, 0);
            cpu.write16(at + 2, 0);
        }
    }
}

/// A twin spark's start: its points flung out at random (up to 4 pixels a frame each, slowed by a
/// 128th a frame), rising at half a pixel a frame, less a 16th each frame.
const TWIN_FLING: u32 = 0x4_0000;
const TWIN_RISE_0: u32 = 0x8000;
const TWIN_RISE_CHANGE_0: u32 = 0xffff_f000;

/// Makes a twin spark at `p` (x, y, z): in list 3, else 2.
pub fn twin_set(cpu: &mut Cpu, p: u32) {
    let o = piece_in(cpu, &[3, 2], TWIN_SIZE);
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + SPRITE));
    cpu.write32(o + BEHAVIOUR, TWIN_EXEC);
    cpu.write32(o + CORNERS_AT, o + TWIN_CORNERS);
    let flags = cpu.read32(o + FLAGS) | FOUR_CORNERS;
    cpu.write32(o + FLAGS, flags);
    cpu.write8(o + LIFT, 0);
    for k in 0..3 {
        let v = cpu.read32(p + 4 * k);
        cpu.write32(o + POSITION + 4 * k, v);
    }
    for point in [TWIN_A, TWIN_B] {
        for k in 0..2 {
            let v = cpu.read32(o + POSITION + 4 * k);
            cpu.write32(o + point + 4 * k, v);
        }
    }
    cpu.write8(o + TWIN_AGE, 0);
    for (speed, accel) in [(TWIN_A_SPEED, TWIN_A_ACCEL), (TWIN_B_SPEED, TWIN_B_ACCEL)] {
        let angle = random(cpu, 0x10000) as u16 as i16 as i32 as u32;
        let fling = random(cpu, TWIN_FLING);
        let s = cpu.call(SL_SIN, &[angle]);
        let dx = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, fling));
        cpu.write32(o + speed, dx);
        let fling = random(cpu, TWIN_FLING);
        let c = cpu.call(SL_COS, &[angle]);
        let dy = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, c.wrapping_neg(), fling));
        cpu.write32(o + speed + 4, dy);
        for k in 0..2 {
            let v = (cpu.read32(o + speed + 4 * k) as i32).wrapping_neg();
            cpu.write32(o + accel + 4 * k, shifted(v, 7));
        }
    }
    let r = random(cpu, 0x10000);
    cpu.write8(o + TWIN_SHOWS_A, (r & 1) as u8);
    cpu.write32(o + TWIN_RISE, TWIN_RISE_0);
    cpu.write32(o + TWIN_RISE_CHANGE, TWIN_RISE_CHANGE_0);
}

/// An ember: what it circles (+0x3c, an object), its turn (a word), the speed its distance (its
/// height, +0x24) grows by and that speed's change.
const EMBER_AROUND: u32 = 0x3c;
const EMBER_TURN: u32 = 0x40;
const EMBER_GROWTH: u32 = 0x44;
const EMBER_GROWTH_CHANGE: u32 = 0x48;
const EMBER_SIZE: u32 = 76;
const EMBER_EXEC: u32 = 0x0605_8444;
/// Its width shrinks by this a frame; its frames are 58 and 56 at random.
const EMBER_SHRINK: u32 = 0x1c0;
const EMBER_FRAME: u8 = 58;

/// An ember's behaviour: ended once the screen is handed over, its width shrunk away, or what it
/// circles ends; else its distance grown, it turned and placed at that distance from what it
/// circles, its frame flickering.
pub fn ember_exec(cpu: &mut Cpu, o: u32) {
    if handed_over(cpu) {
        end(cpu, o);
        return;
    }
    let w = cpu.read32(o + SCALES);
    let w = nested!(cpu, 0x0604_6168, super::actors::approach(cpu, w, 0, EMBER_SHRINK));
    cpu.write32(o + SCALES, w);
    let around = cpu.read32(o + EMBER_AROUND);
    if w == 0 || cpu.read8(around + OBJ_MODE) as i8 >= ENDING as i8 {
        end(cpu, o);
        return;
    }
    let d = cpu.read32(o + SCALES + 4).wrapping_add(cpu.read32(o + EMBER_GROWTH));
    cpu.write32(o + SCALES + 4, d);
    let g = cpu.read32(o + EMBER_GROWTH).wrapping_add(cpu.read32(o + EMBER_GROWTH_CHANGE));
    cpu.write32(o + EMBER_GROWTH, g);
    let a = cpu.read16(o + ANGLE).wrapping_add(cpu.read16(o + EMBER_TURN));
    cpu.write16(o + ANGLE, a);
    let angle = a as i16 as i32 as u32;
    let d = cpu.read32(o + SCALES + 4);
    let s = cpu.call(SL_SIN, &[angle]);
    let dx = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, d));
    let x = dx.wrapping_add(cpu.read32(around + POSITION));
    cpu.write32(o + POSITION, x);
    let d = cpu.read32(o + SCALES + 4);
    let angle = cpu.read16(o + ANGLE) as i16 as i32 as u32;
    let c = cpu.call(SL_COS, &[angle]);
    let dy = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, c.wrapping_neg(), d));
    let y = dy.wrapping_add(cpu.read32(around + POSITION + 4));
    cpu.write32(o + POSITION + 4, y);
    let z = cpu.read32(around + POSITION + 8);
    cpu.write32(o + POSITION + 8, z);
    let r = random(cpu, 0x10000);
    cpu.write8(o + FRAME, EMBER_FRAME - (r & 2) as u8);
}

/// An ember's description: its position (x, y, z), its distance's speed (+0x2c), what it circles
/// (+0x3c).
const ED_GROWTH: u32 = 0x2c;
const ED_AROUND: u32 = 0x3c;
const EMBER_TURN_MOST: u32 = 0x140;

/// Makes an ember from `d`: in list 2; flipped by attribute at random, frame 57 or 56, a random
/// width (a quarter and up to 7 eighths more), turning at random, its distance growing at a
/// quarter of the speed given, slowed by a 256th of it a frame.
pub fn ember_set(cpu: &mut Cpu, d: u32) {
    let o = piece_in(cpu, &[2], EMBER_SIZE);
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + SPRITE));
    cpu.write32(o + BEHAVIOUR, EMBER_EXEC);
    let r = random(cpu, 0x10000);
    let flags = cpu.read32(o + FLAGS) | PIECE_FLAGS | (r & ATTR_100);
    cpu.write32(o + FLAGS, flags);
    for k in 0..3 {
        let v = cpu.read32(d + 4 * k);
        cpu.write32(o + POSITION + 4 * k, v);
    }
    cpu.write8(o + KIND, 0);
    let base = cpu.read16(CHAR_BASE);
    cpu.write16(o + COLOUR, base);
    cpu.write8(o + LIFT, 0);
    let around = cpu.read32(d + ED_AROUND);
    cpu.write32(o + EMBER_AROUND, around);
    let r = random(cpu, 0x10000);
    cpu.write8(o + FRAME, EMBER_FRAME - 1 - (r & 1) as u8);
    let r = random(cpu, 0x10000);
    cpu.write32(o + SCALES, ((r & 7) << 11) + 0x4000);
    cpu.write32(o + SCALES + 4, 0);
    let r = random(cpu, 0x10000);
    cpu.write16(o + ANGLE, r as u16);
    let r = random(cpu, EMBER_TURN_MOST);
    cpu.write16(o + EMBER_TURN, r as u16);
    let g = cpu.read32(d + ED_GROWTH) as i32;
    cpu.write32(o + EMBER_GROWTH, shifted(g, 2));
    let g = (cpu.read32(d + ED_GROWTH) as i32).wrapping_neg();
    cpu.write32(o + EMBER_GROWTH_CHANGE, shifted(g, 8));
}

/// A piece of debris: its speed (x, y, z), its drag (a 32nd of the speed), its kind (0-3; 2 a
/// streak along its direction, the others turned at random quarters).
const DEBRIS_SPEED: u32 = 0x40;
const DEBRIS_DRAG: u32 = 0x4c;
const DEBRIS_58: u32 = 0x58;
const DEBRIS_KIND: u32 = 0x5a;
const STREAK: u8 = 2;
const DEBRIS_SIZE: u32 = 92;
const DEBRIS_EXEC: u32 = 0x0605_8638;
const DEBRIS_COLOUR: u16 = 0x90;
const DEBRIS_LIFT: u8 = 0xf8;
/// Its frames: a streak's from 0x1f, the others' from 4; it ends past 18 (not a streak) or 40.
const STREAK_FRAME: u8 = 0x1f;
const PIECE_FRAME: u8 = 4;
const PIECE_LAST: u8 = 18;
const LAST_FRAME: u8 = 40;
/// It is flung out from 0xb0 nearer the eye, at least a quarter pixel and up to 15 times `spread`
/// more.
const DEBRIS_NEARER: u32 = 0xff50_0000;
const DEBRIS_FLING: i32 = -0x4000;

/// A debris piece's behaviour: ended once the screen is handed over or its frames run out (its
/// frame stepped on every other pair of frames); else moved by its speed, which its drag slows.
pub fn debris_exec(cpu: &mut Cpu, o: u32) {
    if !handed_over(cpu) {
        let step = ((cpu.read32(GAME_FRAMES) & 2) >> 1) as u8;
        let frame = cpu.read8(o + FRAME).wrapping_add(step);
        cpu.write8(o + FRAME, frame);
        if (cpu.read8(o + DEBRIS_KIND) == STREAK || frame <= PIECE_LAST) && cpu.read8(o + FRAME) <= LAST_FRAME {
            for k in 0..3 {
                let p = cpu.read32(o + POSITION + 4 * k).wrapping_add(cpu.read32(o + DEBRIS_SPEED + 4 * k));
                cpu.write32(o + POSITION + 4 * k, p);
            }
            for k in 0..3 {
                let v = cpu.read32(o + DEBRIS_SPEED + 4 * k).wrapping_add(cpu.read32(o + DEBRIS_DRAG + 4 * k));
                cpu.write32(o + DEBRIS_SPEED + 4 * k, v);
            }
            return;
        }
    }
    end(cpu, o);
}

/// Makes a piece of debris at `p` (x, y, z) flung `spread` apart: in list 2; its speed a random
/// direction in 3D (turned about x and z at random), always toward the eye.
pub fn debris_set(cpu: &mut Cpu, p: u32, spread: u32) {
    // the SH-2 code keeps the spread in its frame, and later the kind's address
    let frame = cpu.stack();
    cpu.write32(frame, spread);
    let o = piece_in(cpu, &[2], DEBRIS_SIZE);
    if o == 0 {
        return;
    }
    cpu.call(SL_PUSH_MATRIX, &[]);
    cpu.call(SL_TRANSLATE, &[0, 0, DEBRIS_NEARER]);
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + SPRITE));
    cpu.write32(o + BEHAVIOUR, DEBRIS_EXEC);
    cpu.write32(o + FLAGS, PIECE_FLAGS);
    cpu.write8(o + KIND, 0);
    let colour = cpu.read16(CHAR_BASE).wrapping_add(DEBRIS_COLOUR);
    cpu.write16(o + COLOUR, colour);
    cpu.write8(o + LIFT, DEBRIS_LIFT);
    cpu.write16(o + DEBRIS_58, 0);
    let r = random(cpu, 0x10000);
    cpu.write32(frame + 4, o + DEBRIS_KIND);
    cpu.write8(o + DEBRIS_KIND, (r & 3) as u8);
    for k in 0..3 {
        let v = cpu.read32(p + 4 * k);
        cpu.write32(o + POSITION + 4 * k, v);
    }
    if random(cpu, 0x10000) & 1 != 0 {
        let flags = cpu.read32(o + FLAGS) | ATTR_100;
        cpu.write32(o + FLAGS, flags);
    }
    cpu.call(SL_PUSH_MATRIX, &[]);
    let r = random(cpu, 0x10000);
    cpu.call(SL_ROT_X, &[r as u16 as i16 as i32 as u32]);
    let r = random(cpu, 0x10000);
    cpu.call(SL_ROT_Z, &[r as u16 as i16 as i32 as u32]);
    let r = random(cpu, 0x10000) & 15;
    let spread = cpu.read32(frame);
    cpu.call(SL_TRANSLATE, &[0, (DEBRIS_FLING as u32).wrapping_sub(r.wrapping_mul(spread)), 0]);
    cpu.call(SL_CALC_POINT, &[0, 0, 0, o + DEBRIS_SPEED]);
    cpu.call(SL_POP_MATRIX, &[]);
    let vz = (cpu.read32(o + DEBRIS_SPEED + 8) as i32).wrapping_abs().wrapping_neg();
    cpu.write32(o + DEBRIS_SPEED + 8, vz as u32);
    for k in 0..3 {
        let v = (cpu.read32(o + DEBRIS_SPEED + 4 * k) as i32).wrapping_neg();
        cpu.write32(o + DEBRIS_DRAG + 4 * k, shifted(v, 5));
    }
    if cpu.read8(o + DEBRIS_KIND) == STREAK {
        cpu.write8(o + FRAME, STREAK_FRAME);
        let (vx, vy) = (cpu.read32(o + DEBRIS_SPEED), cpu.read32(o + DEBRIS_SPEED + 4));
        let a = cpu.call(SL_ATAN, &[vx, vy]);
        cpu.write16(o + ANGLE, (a as u16).wrapping_add(0x4000));
    } else {
        cpu.write8(o + FRAME, PIECE_FRAME);
        let r = random(cpu, 0x10000);
        cpu.write16(o + ANGLE, (r & 0xc000) as u16);
    }
    cpu.call(SL_POP_MATRIX, &[]);
}
