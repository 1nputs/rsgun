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

/// An explosion: what it follows (+0x3c, an object; dropped once that ends), its power (grows by
/// 0x100 a frame to 0xc000, then falls back), its state (a word: growing, burning down, fading;
/// also the word its orbiting sparks watch).
const FOLLOWS: u32 = 0x3c;
const POWER: u32 = 0x40;
const STATE: u32 = 0x58;
const GROWING: u16 = 0;
const BURNING: u16 = 0x10;
const BURNT_OUT: u16 = 0x20;
const FADED: u16 = 0x400;
const POWER_STEP: u32 = 0x100;
const POWER_MOST: i32 = 0xc000;
/// The flash at its height: the screen's colour offset to grey, faded back to none over 300
/// frames; the sound effects (the big one, and one of four at random every 16 steps of power).
const FLASH: u32 = 80;
const FLASH_FADE: u32 = 300;
const SE_BIG: u32 = 19;
const SE_SMALL: u32 = 0x0607_31c6;
/// The explosion's own flags bit (an attribute, at random) and colours.
const COLOUR_ALT: u16 = 0x10;

/// The explosion's description of its pieces (in its frame): their position, scales, angle (a
/// word), a word at +0x20 (the orbiter's colours' index in its low byte), the orbiter's radius and
/// the radius's speed, a word at +0x2c (an ember's growth as a long; the orbiter's kind in its low
/// byte), the orbiter's angle offset, turn and a word (its lift in the low byte), the orbiter's
/// scales' speeds, what an ember circles.
const D_SCALES: u32 = 0x0c;
const D_ANGLE: u32 = 0x18;
const D_20: u32 = 0x20;
const D_RADIUS: u32 = 0x24;
const D_2C: u32 = 0x2c;
const D_ANGLE_OFFSET: u32 = 0x2e;
const D_TURN: u32 = 0x30;
const D_32: u32 = 0x32;
const D_SCALE_SPEED: u32 = 0x34;
const D_AROUND: u32 = 0x3c;
const ORBIT_SET: u32 = 0x0605_9554;

/// An explosion's behaviour: ended once the screen is handed over; it follows what it follows (or
/// stays). Growing, its power goes up and it throws two pieces of debris a frame (and an ember
/// every other), a small piece and sound every 16th; at full power it flashes the screen, throws
/// a big piece with the big sound, and burns down, its power falling while it throws twin sparks
/// on even frames; burnt out, it fades for 64 frames and ends. Each frame (but the last) it also
/// makes a spark orbiting it and takes a random size, flip, frame and colour.
pub fn explosion_exec(cpu: &mut Cpu, x: u32) {
    if handed_over(cpu) {
        end(cpu, x);
        return;
    }
    let follows = cpu.read32(x + FOLLOWS);
    if follows != 0 && cpu.read8(follows + OBJ_MODE) as i8 >= ENDING as i8 {
        cpu.write32(x + FOLLOWS, 0);
    }
    let d = cpu.stack();
    if cpu.read32(x + FOLLOWS) != 0 {
        for k in 0..3 {
            let v = cpu.read32(follows + POSITION + 4 * k);
            cpu.write32(d + 4 * k, v);
        }
        for k in 0..3 {
            let v = cpu.read32(d + 4 * k);
            cpu.write32(x + POSITION + 4 * k, v);
        }
    } else {
        for k in 0..3 {
            let v = cpu.read32(x + POSITION + 4 * k);
            cpu.write32(d + 4 * k, v);
        }
    }
    match cpu.read16(x + STATE) {
        GROWING => {
            let power = cpu.read32(x + POWER).wrapping_add(POWER_STEP);
            cpu.write32(x + POWER, power);
            nested!(cpu, 0x0605_86e4, debris_set(cpu, d, power));
            let power = cpu.read32(x + POWER);
            nested!(cpu, 0x0605_86e4, debris_set(cpu, d, power));
            cpu.write32(d + D_AROUND, x);
            let power = cpu.read32(x + POWER);
            if power & POWER_STEP == 0 {
                cpu.write32(d + D_2C, power);
                nested!(cpu, 0x0605_8520, ember_set(cpu, d));
            }
            let power = cpu.read32(x + POWER);
            if power as i32 > POWER_MOST {
                cpu.write16(x + STATE, BURNING);
                nested!(cpu, 0x0605_e9d8, super::screen::stop_fades(cpu));
                nested!(cpu, 0x0605_e6dc, super::screen::set_fade_offset(cpu, FLASH, FLASH, FLASH));
                nested!(cpu, 0x0605_e93c, super::screen::fade_to(cpu, 0, 0, 0, FLASH_FADE));
                cpu.write16(d + D_20, 0);
                cpu.write32(d + D_2C, 0x1800);
                cpu.write32(d + D_TURN, -80i32 as u32);
                cpu.write32(d + D_SCALE_SPEED, 0x20);
                nested!(cpu, 0x0605_8e1c, blast_set(cpu, d));
                nested!(cpu, 0x0600_50d4, super::sound::se_set(cpu, SE_BIG));
            } else if power & 0xf00 == 0 {
                cpu.write16(d + D_20, 0);
                let power = cpu.read32(x + POWER);
                cpu.write32(d + D_SCALES + 4, power.wrapping_mul(6));
                let r = random(cpu, 0x10000);
                cpu.write16(d + D_ANGLE, r as u16);
                nested!(cpu, 0x0605_7da4, cross_set(cpu, d));
                let r = random(cpu, 0x10000) & 3;
                let se = cpu.read16(SE_SMALL + 4 * r) as i16 as i32 as u32;
                nested!(cpu, 0x0600_50d4, super::sound::se_set(cpu, se));
            }
        }
        BURNING => {
            if cpu.read32(GAME_FRAMES) & 1 == 0 {
                nested!(cpu, 0x0605_8278, twin_set(cpu, d));
            }
            let power = cpu.read32(x + POWER).wrapping_sub(POWER_STEP);
            cpu.write32(x + POWER, power);
            if (power as i32) < 0 {
                cpu.write16(x + STATE, BURNT_OUT);
            }
        }
        _ => {
            let state = cpu.read16(x + STATE).wrapping_add(0x10);
            cpu.write16(x + STATE, state);
            if state > FADED {
                cpu.write16(x + STATE, 1);
                end(cpu, x);
                return;
            }
        }
    }
    // the orbiting spark (orbits.rs: kind 0, colours 4), and the explosion's own look
    cpu.write16(d + D_2C, 0);
    cpu.write16(d + D_20, 4);
    let r = random(cpu, 0x10000);
    cpu.write32(d + D_SCALES, (r & 0x1fff) + 0x4000);
    let power = cpu.read32(x + POWER);
    let height = power.wrapping_mul(8);
    cpu.write32(d + D_SCALES + 4, height);
    cpu.write32(d + D_SCALE_SPEED, 0);
    cpu.write32(d + D_SCALE_SPEED + 4, shifted((height as i32).wrapping_neg(), 6));
    cpu.write16(d + D_ANGLE, 0);
    let r = random(cpu, 0x10000);
    cpu.write16(d + D_ANGLE_OFFSET, r as u16);
    let r = random(cpu, 0x100);
    cpu.write16(d + D_TURN, r as u16);
    cpu.write16(d + D_32, 0);
    let power = cpu.read32(x + POWER);
    let radius = power.wrapping_mul(0x80);
    cpu.write32(d + D_RADIUS, radius);
    cpu.write32(d + D_RADIUS + 4, shifted((radius as i32).wrapping_neg(), 6));
    let (watched, centre, angle) = (x + STATE, x + POSITION, x + ANGLE);
    cpu.nested_with_stack(&ORBIT_SET_PROLOGUE, ORBIT_SET, &[angle], |cpu| super::orbits::orbit_set(cpu, d, watched, 1, centre, angle));
    let flags = cpu.read32(x + FLAGS) & !ATTR_100;
    cpu.write32(x + FLAGS, flags);
    let r = random(cpu, 0x10000);
    let flags = cpu.read32(x + FLAGS) | (r & ATTR_100);
    cpu.write32(x + FLAGS, flags);
    let r = random(cpu, 0x2000);
    let size = cpu.read32(x + POWER).wrapping_sub(r).wrapping_mul(2);
    cpu.write32(x + SCALES, size);
    cpu.write32(x + SCALES + 4, size);
    let r = random(cpu, 0x10000);
    cpu.write8(x + FRAME, (r & 1) as u8);
    let r = random(cpu, 0x10000);
    let base = cpu.read16(CHAR_BASE);
    cpu.write16(x + COLOUR, if r & 7 != 0 { base } else { base + COLOUR_ALT });
}
static ORBIT_SET_PROLOGUE: std::sync::OnceLock<super::Prologue> = std::sync::OnceLock::new();

const EXPLOSION_SIZE: u32 = 0x5c;
const EXPLOSION_EXEC: u32 = 0x0605_88d4;
const POWER_0: u32 = 0x3000;

/// Makes an explosion at `p` (x, y, z; following the object at +0x3c, if any): in list 3, else 2.
pub fn explosion_set(cpu: &mut Cpu, p: u32) {
    let x = piece_in(cpu, &[3, 2], EXPLOSION_SIZE);
    if x == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, x + SPRITE));
    cpu.write32(x + BEHAVIOUR, EXPLOSION_EXEC);
    cpu.write32(x + FLAGS, PIECE_FLAGS);
    cpu.write16(x + STATE, GROWING);
    cpu.write8(x + FRAME, 1);
    cpu.write8(x + KIND, 0);
    let base = cpu.read16(CHAR_BASE);
    cpu.write16(x + COLOUR, base);
    for k in 0..3 {
        let v = cpu.read32(p + 4 * k);
        cpu.write32(x + POSITION + 4 * k, v);
    }
    let follows = cpu.read32(p + 0x3c);
    cpu.write32(x + FOLLOWS, follows);
    cpu.write32(x + POWER, POWER_0);
}

/// A blast (the explosion's flash at full power): a ring of shards drawn by its own draw, around
/// what it follows; its size grows by its growth, which changes by its change; once the growth
/// turns back it lasts its count of frames more (and draws as many shards).
const BLAST_FOLLOWS: u32 = 0x3c;
const BLAST_GROWTH: u32 = 0x40;
const BLAST_CHANGE: u32 = 0x44;
const BLAST_COUNT: u32 = 0x48;
const BLAST_SIZE: u32 = 0x4c;
const BLAST_EXEC: u32 = 0x0605_8c5c;
const BLAST_DRAW: u32 = 0x0605_8d00;
/// Its flags: a model drawn by its own function (the draw at +0x34).
const OWN_MODEL: u32 = 0xc0_0000;
const DRAW_AT: u32 = 0x34;
/// The colours it flickers between.
const BLAST_ALT: u16 = 8;

/// Makes a blast from `d`: its growth (+0x2c), the growth's change (+0x30), its count (+0x34),
/// what it follows (+0x3c); in list 3.
pub fn blast_set(cpu: &mut Cpu, d: u32) {
    let o = piece_in(cpu, &[3], BLAST_SIZE);
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + SPRITE));
    cpu.write32(o + BEHAVIOUR, BLAST_EXEC);
    cpu.write32(o + DRAW_AT, BLAST_DRAW);
    cpu.write32(o + FLAGS, OWN_MODEL);
    for k in 0..3 {
        let v = cpu.read32(d + 4 * k);
        cpu.write32(o + POSITION + 4 * k, v);
    }
    for (from, to) in [(0x3c, BLAST_FOLLOWS), (0x2c, BLAST_GROWTH), (0x30, BLAST_CHANGE), (0x34, BLAST_COUNT)] {
        let v = cpu.read32(d + from);
        cpu.write32(o + to, v);
    }
    cpu.write32(o + SCALES, 0);
    cpu.write32(o + SCALES + 4, 0);
}

/// A blast's behaviour: ended once the screen is handed over, or (its growth turned back) its
/// count runs out; else grown (both scales), turned at random, placed on what it follows, its
/// colour flickering.
pub fn blast_exec(cpu: &mut Cpu, o: u32) {
    if handed_over(cpu) {
        end(cpu, o);
        return;
    }
    if (cpu.read32(o + BLAST_GROWTH) as i32) < 0 {
        let n = cpu.read32(o + BLAST_COUNT) as i32;
        if n <= 0 {
            end(cpu, o);
            return;
        }
        cpu.write32(o + BLAST_COUNT, (n - 1) as u32);
    }
    let size = cpu.read32(o + SCALES).wrapping_add(cpu.read32(o + BLAST_GROWTH));
    cpu.write32(o + SCALES, size);
    let g = cpu.read32(o + BLAST_GROWTH).wrapping_add(cpu.read32(o + BLAST_CHANGE));
    cpu.write32(o + BLAST_GROWTH, g);
    let size = cpu.read32(o + SCALES);
    cpu.write32(o + SCALES + 4, size);
    let r = random(cpu, 0x10000);
    cpu.write16(o + ANGLE, r as u16);
    let follows = cpu.read32(o + BLAST_FOLLOWS);
    if follows != 0 {
        for k in 0..3 {
            let v = cpu.read32(follows + POSITION + 4 * k);
            cpu.write32(o + POSITION + 4 * k, v);
        }
    }
    let r = random(cpu, 0x10000);
    let colour = ((r as u16) & BLAST_ALT).wrapping_add(cpu.read16(CHAR_BASE));
    cpu.write16(o + COLOUR, colour);
}

/// The shard's sprite (in the draw's frame, sprites.rs's struct): its flags, position, colour,
/// angle, corners and lift; the corners after it (a thin quad, 3 pixels either side).
const S_FLAGS: u32 = 0x00;
const S_POSITION: u32 = 0x04;
const S_COLOUR: u32 = 0x18;
const S_ANGLE: u32 = 0x20;
const S_LIFT: u32 = 0x23;
const S_CORNERS_AT: u32 = 0x24;
const SHARD_CORNERS: u32 = 0x2c;
const SHARD: [u16; 8] = [0, 0xfffd, 0, 0xfffd, 0, 3, 0, 3];
const SHARD_POLYGON: u32 = 2;
const SHARD_LIFT: u8 = 8;
const SHARD_REACH: u32 = 32;

/// A blast's draw (sprite `s`): its count of shards, each at a random angle at 32 times its size
/// from its centre, pointing outward.
pub fn blast_draw(cpu: &mut Cpu, s: u32) {
    let o = s - SPRITE;
    let shard = cpu.stack();
    for (i, v) in SHARD.into_iter().enumerate() {
        cpu.write16(shard + SHARD_CORNERS + 2 * i as u32, v);
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, shard));
    let colour = cpu.read16(s + S_COLOUR);
    cpu.write16(shard + S_COLOUR, colour);
    let flags = cpu.read32(o + FLAGS) & ATTR_100 | SHARD_POLYGON;
    cpu.write32(shard + S_FLAGS, flags);
    cpu.write32(shard + S_CORNERS_AT, shard + SHARD_CORNERS);
    cpu.write8(shard + S_LIFT, SHARD_LIFT);
    let z = cpu.read32(o + POSITION + 8);
    cpu.write32(shard + S_POSITION + 8, z);
    let mut n: i32 = 0;
    while n < cpu.read32(o + BLAST_COUNT) as i32 {
        let a = random(cpu, 0x10000) as u16;
        let angle = a as i16 as i32 as u32;
        let reach = cpu.read32(o + SCALES).wrapping_mul(SHARD_REACH);
        let s_ = cpu.call(SL_SIN, &[angle]);
        let dx = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s_, reach));
        let x = cpu.read32(o + POSITION).wrapping_add(dx);
        cpu.write32(shard + S_POSITION, x);
        let reach = cpu.read32(o + SCALES + 4).wrapping_mul(SHARD_REACH);
        let c = cpu.call(SL_COS, &[angle]);
        let dy = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, c, reach));
        let y = cpu.read32(o + POSITION + 4).wrapping_sub(dy);
        cpu.write32(shard + S_POSITION + 4, y);
        cpu.write16(shard + S_ANGLE, a.wrapping_add(0x4000));
        nested!(cpu, 0x0604_0960, sprites::draw(cpu, shard));
        n = (n + 1) as i8 as i32;
    }
}

/// The drifting effect's description (actors.rs's E_*), and its maker.
use super::actors::{E_ANGLE, E_COLOUR, E_POSITION, E_SCALES, E_SCRIPT, E_SPEED};
const DRIFT_EFFECT: u32 = 0x0605_7454;

fn drift_effect(cpu: &mut Cpu, e: u32) -> u32 {
    nested!(cpu, DRIFT_EFFECT, super::actors::drift_effect(cpu, e))
}

/// The drifting effects' animations used here.
const SMOKE_A: u32 = 0x0607_3008;
const SMOKE_B: u32 = 0x0607_3044;
const SMOKE_C: u32 = 0x0607_305c;
const SPARK: u32 = 0x0607_30b0;
const FLAME: u32 = 0x0607_3074;
const JET_PUFF: u32 = 0x0607_3128;
const CROSS: u32 = 0x0607_3188;
const SMOKE_TRAIL: u32 = 0x0607_304a;
/// A description's size for glow_set (+0x2c).
const E_SIZE: u32 = 0x2c;
const GLOW_SET: u32 = 0x0605_8f34;

/// Makes an afterimage of `src`: a copy of its look (one lift lower) shrinking away over
/// `frames`, moving at `speed` in its direction; in list 3: it, or 0.
pub fn afterimage_set(cpu: &mut Cpu, src: u32, _unused: u32, frames: u32, speed: u32) -> u32 {
    let o = piece_in(cpu, &[3], 100);
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + SPRITE));
    cpu.write32(o + BEHAVIOUR, 0x0605_75e4);
    for k in 0..3 {
        let v = cpu.read32(src + POSITION + 4 * k);
        cpu.write32(o + POSITION + 4 * k, v);
    }
    let flags = cpu.read32(src + FLAGS) & ATTR_100 | PIECE_FLAGS;
    cpu.write32(o + FLAGS, flags);
    for field in [KIND, FRAME] {
        let v = cpu.read8(src + field);
        cpu.write8(o + field, v);
    }
    for field in [COLOUR, ANGLE] {
        let v = cpu.read16(src + field);
        cpu.write16(o + field, v);
    }
    for k in 0..2 {
        let v = cpu.read32(src + SCALES + 4 * k);
        cpu.write32(o + SCALES + 4 * k, v);
    }
    let lift = cpu.read8(src + LIFT).wrapping_sub(1);
    cpu.write8(o + LIFT, lift);
    let angle = cpu.read16(o + ANGLE) as i16 as i32 as u32;
    let s = cpu.call(SL_SIN, &[angle]);
    let dx = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, speed));
    cpu.write32(o + SPEED, dx);
    let angle = cpu.read16(o + ANGLE) as i16 as i32 as u32;
    let c = cpu.call(SL_COS, &[angle]);
    let dy = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, c.wrapping_neg(), speed));
    cpu.write32(o + SPEED + 4, dy);
    for k in 0..2 {
        let v = cpu.read32(src + SCALES + 4 * k);
        let q = nested!(cpu, 0x0600_6a78, math::quotient(cpu, v, frames));
        cpu.write32(o + SHRINK + 4 * k, q);
    }
    o
}

/// A glow and six smoke trails flung out around it from `d` (its position).
pub fn smoke_burst(cpu: &mut Cpu, d: u32) {
    cpu.write32(d + E_SIZE, 0x2_0000);
    nested!(cpu, GLOW_SET, glow_set(cpu, d));
    let r = random(cpu, 0x10000);
    cpu.write16(d + E_ANGLE, r as u16);
    for _ in 0..6 {
        let r = random(cpu, 0x10000);
        cpu.write32(d + E_SPEED, 0x2_0000u32.wrapping_sub(r));
        nested!(cpu, 0x0605_7998, smoke_trail_set(cpu, d));
        let a = cpu.read16(d + E_ANGLE).wrapping_add(0x2aaa);
        cpu.write16(d + E_ANGLE, a);
    }
}

/// A puff of smoke from `e` (its position and size): one of three smokes at random, maybe
/// flipped, in a random direction at a random speed up to 1 pixel a frame.
pub fn smoke_puff(cpu: &mut Cpu, e: u32) {
    let script = match random(cpu, 0x10000) & 0x300 {
        0x100 => SMOKE_B,
        0x200 => SMOKE_C,
        _ => SMOKE_A,
    };
    cpu.write32(e + E_SCRIPT, script);
    cpu.write16(e + E_COLOUR, 0x90);
    let r = random(cpu, 0x10000) as u16;
    let colour = r & 0x2000 | cpu.read16(e + E_COLOUR);
    cpu.write16(e + E_COLOUR, colour);
    let r = random(cpu, 0x10000);
    cpu.write16(e + E_ANGLE, r as u16);
    let r = random(cpu, 0x10000);
    cpu.write32(e + E_SPEED, r);
    let size = cpu.read32(e + E_SCALES);
    cpu.write32(e + E_SCALES + 4, size);
    drift_effect(cpu, e);
}

/// A burst of sparks from `e` (its position, direction and speed): a puff of smoke where it is,
/// a spark flung back the other way and two at random, and a glow.
pub fn spark_burst(cpu: &mut Cpu, e: u32) {
    cpu.write32(e + E_SCRIPT, SMOKE_C);
    cpu.write16(e + E_COLOUR, 0x90);
    cpu.write32(e + E_SCALES, 0x1_0000);
    cpu.write32(e + E_SCALES + 4, 0x1_0000);
    drift_effect(cpu, e);
    cpu.write32(e + E_SCRIPT, SPARK);
    cpu.write16(e + E_COLOUR, 0x8000);
    let r = random(cpu, 0x10000);
    cpu.write16(e + E_ANGLE, r as u16);
    let speed = cpu.read32(e + E_SPEED).wrapping_neg();
    cpu.write32(e + E_SPEED, speed);
    cpu.write32(e + E_SCALES + 4, 0xa000);
    drift_effect(cpu, e);
    for _ in 0..2 {
        let r = random(cpu, 0x10000);
        cpu.write16(e + E_ANGLE, r as u16);
        drift_effect(cpu, e);
    }
    cpu.write32(e + E_SIZE, 0xc000);
    nested!(cpu, GLOW_SET, glow_set(cpu, e));
}

/// A smoke trail (not drawn): its speed (x, y, z: rising faster each frame, ended past 6 pixels
/// a frame), and a count (its width's field) that puts out a puff each 6th frame.
const TRAIL_SPEED: u32 = 0x3c;
const TRAIL_COUNT: u32 = SCALES;
const TRAIL_RISE: u32 = 0x1000;
const TRAIL_LAST: i32 = 0x6_0000;
const NO_DRAW: u32 = 0x8000_0000;

/// A smoke trail's behaviour.
pub fn smoke_trail_exec(cpu: &mut Cpu, o: u32) {
    let vz = cpu.read32(o + TRAIL_SPEED + 8).wrapping_add(TRAIL_RISE);
    cpu.write32(o + TRAIL_SPEED + 8, vz);
    if vz as i32 > TRAIL_LAST {
        end(cpu, o);
        return;
    }
    let n = cpu.read32(o + TRAIL_COUNT).wrapping_add(1);
    cpu.write32(o + TRAIL_COUNT, n);
    if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, n, 6)) == 0 {
        let e = cpu.stack();
        for k in 0..3 {
            let v = cpu.read32(o + POSITION + 4 * k);
            cpu.write32(e + E_POSITION + 4 * k, v);
        }
        cpu.write32(e + E_SCALES, 0xc000);
        nested!(cpu, 0x0605_77d0, smoke_puff(cpu, e));
    }
    for k in 0..3 {
        let p = cpu.read32(o + POSITION + 4 * k).wrapping_add(cpu.read32(o + TRAIL_SPEED + 4 * k));
        cpu.write32(o + POSITION + 4 * k, p);
    }
}

/// Makes a smoke trail from `d` (its position, direction and speed), starting a pixel a frame
/// downward (toward the eye), its count at random; in list 3.
pub fn smoke_trail_set(cpu: &mut Cpu, d: u32) {
    let o = piece_in(cpu, &[3], 0x48);
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + SPRITE));
    cpu.write32(o + BEHAVIOUR, 0x0605_790c);
    cpu.write32(o + FLAGS, NO_DRAW);
    for k in 0..3 {
        let v = cpu.read32(d + E_POSITION + 4 * k);
        cpu.write32(o + POSITION + 4 * k, v);
    }
    let angle = cpu.read16(d + E_ANGLE);
    cpu.write16(o + ANGLE, angle);
    let angle = angle as i16 as i32 as u32;
    let speed = cpu.read32(d + E_SPEED);
    let s = cpu.call(SL_SIN, &[angle]);
    let dx = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, speed));
    cpu.write32(o + TRAIL_SPEED, dx);
    let speed = cpu.read32(d + E_SPEED);
    let angle = cpu.read16(d + E_ANGLE) as i16 as i32 as u32;
    let c = cpu.call(SL_COS, &[angle]);
    let dy = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, c.wrapping_neg(), speed));
    cpu.write32(o + TRAIL_SPEED + 4, dy);
    cpu.write32(o + TRAIL_SPEED + 8, 0xffff_0000);
    let r = random(cpu, 0x10000);
    cpu.write32(o + TRAIL_COUNT, r);
}

fn drift(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0605_73f0, super::actors::drift(cpu, o));
}

/// A smoking effect's behaviour: drifting; on odd frames a puff of smoke 20 behind it (back
/// along its direction, scattered by up to 16 pixels), drifting slowly back.
pub fn smoking_exec(cpu: &mut Cpu, o: u32) {
    drift(cpu, o);
    if cpu.read32(GAME_FRAMES) & 1 == 0 {
        return;
    }
    let e = cpu.stack();
    let back = cpu.read16(o + ANGLE).wrapping_add(0x8000) as i16 as i32 as u32;
    let s = cpu.call(SL_SIN, &[back]);
    let r = random(cpu, 0x10_0000);
    let x = cpu.read32(o + POSITION).wrapping_add(s.wrapping_mul(20)).wrapping_add(r);
    cpu.write32(e + E_POSITION, x);
    let back = cpu.read16(o + ANGLE).wrapping_add(0x8000) as i16 as i32 as u32;
    let c = cpu.call(SL_COS, &[back]);
    let r = random(cpu, 0x10_0000);
    let y = cpu.read32(o + POSITION + 4).wrapping_sub(c.wrapping_mul(20)).wrapping_add(r);
    cpu.write32(e + E_POSITION + 4, y);
    let r = random(cpu, 2);
    let z = cpu.read32(o + POSITION + 8).wrapping_sub(r);
    cpu.write32(e + E_POSITION + 8, z);
    let r = random(cpu, 0x10000);
    cpu.write32(e + E_SCRIPT, SMOKE_TRAIL + 2 * (r & 3));
    let r = random(cpu, 0x10000) as u16;
    cpu.write16(e + E_COLOUR, r & 0xa000 | 0x90);
    let r = random(cpu, 0x1000);
    cpu.write32(e + E_SCALES, r + 0x6000);
    cpu.write32(e + E_SCALES + 4, r + 0x6000);
    let r = random(cpu, 0x1000) as u16;
    let a = r.wrapping_add(cpu.read16(o + ANGLE));
    cpu.write16(e + E_ANGLE, a);
    let r = random(cpu, 0x2000);
    cpu.write32(e + E_SPEED, r.wrapping_add(0xffff_4000));
    drift_effect(cpu, e);
}

/// Makes a smoking effect from `e` (its position and direction: smoke flung out at 2 pixels a
/// frame), with a glow.
pub fn smoking_set(cpu: &mut Cpu, e: u32) {
    cpu.write32(e + E_SCRIPT, SMOKE_B);
    cpu.write16(e + E_COLOUR, 0x90);
    cpu.write32(e + E_SCALES, 0x1_0000);
    cpu.write32(e + E_SCALES + 4, 0x1_0000);
    cpu.write32(e + E_SPEED, 0x2_0000);
    let o = drift_effect(cpu, e);
    if o != 0 {
        cpu.write32(o + BEHAVIOUR, 0x0605_7a64);
        cpu.write32(e + E_SIZE, 0x1_0000);
        nested!(cpu, GLOW_SET, glow_set(cpu, e));
    }
}

/// A burning effect's behaviour: drifting; while it lasts, a twin spark on even frames, and on
/// odd ones a puff of smoke 32 out from it in a random direction.
pub fn burning_exec(cpu: &mut Cpu, o: u32) {
    drift(cpu, o);
    if cpu.read8(o + OBJ_MODE) as i8 >= ENDING as i8 {
        return;
    }
    let e = cpu.stack();
    if cpu.read32(GAME_FRAMES) & 1 == 0 {
        for k in 0..3 {
            let v = cpu.read32(o + POSITION + 4 * k);
            cpu.write32(e + 4 * k, v);
        }
        nested!(cpu, 0x0605_8278, twin_set(cpu, e));
        return;
    }
    cpu.write32(e + E_SCRIPT, SMOKE_B);
    let r = random(cpu, 0x10000) as u16;
    cpu.write16(e + E_COLOUR, r & 0x2000 | 0x90);
    let r = random(cpu, 0x2000).wrapping_add(0x1_0000);
    cpu.write32(e + E_SCALES, r);
    cpu.write32(e + E_SCALES + 4, r);
    let r = random(cpu, 0x10000);
    cpu.write16(e + E_ANGLE, r as u16);
    let r = random(cpu, 0x8000);
    cpu.write32(e + E_SPEED, r.wrapping_add(0x1_2000));
    let angle = cpu.read16(e + E_ANGLE) as i16 as i32 as u32;
    let s = cpu.call(SL_SIN, &[angle]);
    let x = cpu.read32(o + POSITION).wrapping_add(s.wrapping_mul(32));
    cpu.write32(e + E_POSITION, x);
    let angle = cpu.read16(e + E_ANGLE) as i16 as i32 as u32;
    let c = cpu.call(SL_COS, &[angle]);
    let y = cpu.read32(o + POSITION + 4).wrapping_sub(c.wrapping_mul(32));
    cpu.write32(e + E_POSITION + 4, y);
    let z = cpu.read32(o + POSITION + 8).wrapping_sub(2);
    cpu.write32(e + E_POSITION + 8, z);
    drift_effect(cpu, e);
}

/// The burning effect's orbiting rings (orbits.rs's description: kind 4, colours 4).
const ORBIT_D: u32 = 0;

/// Makes a burning effect from `e` (its position): a still flame with a glow, and five rings
/// orbiting it until its flags' high word has bit 8.
pub fn burning_set(cpu: &mut Cpu, e: u32) {
    cpu.write32(e + E_SCRIPT, FLAME);
    cpu.write16(e + E_COLOUR, 0);
    cpu.write32(e + E_SCALES, 0x1_0000);
    cpu.write32(e + E_SCALES + 4, 0x1_0000);
    cpu.write32(e + E_SPEED, 0);
    let o = drift_effect(cpu, e);
    if o == 0 {
        return;
    }
    cpu.write32(o + BEHAVIOUR, 0x0605_7bcc);
    cpu.write32(e + E_SIZE, 0x2_0000);
    nested!(cpu, GLOW_SET, glow_set(cpu, e));
    let d = cpu.stack() + ORBIT_D;
    cpu.write16(d + D_2C, 4);
    cpu.write16(d + D_20, 4);
    cpu.write32(d + D_SCALES, 0x3000);
    cpu.write32(d + D_SCALES + 4, 0x4000);
    cpu.write32(d + D_SCALE_SPEED, 0xffff_fc00);
    cpu.write32(d + D_SCALE_SPEED + 4, 0x2000);
    cpu.write32(d + D_RADIUS, 0x2_0000);
    cpu.write32(d + D_RADIUS + 4, 0);
    for _ in 0..5 {
        let r = random(cpu, 0x10000);
        cpu.write16(d + D_ANGLE, r as u16);
        let r = random(cpu, 0x10000);
        cpu.write16(d + D_ANGLE_OFFSET, r as u16);
        let r = random(cpu, 0x400);
        cpu.write16(d + D_TURN, r as u16);
        cpu.write16(d + D_32, 0xfff7);
        let (watched, centre, angle) = (o + FLAGS, o + POSITION, o + ANGLE);
        cpu.nested_with_stack(&ORBIT_SET_PROLOGUE, ORBIT_SET, &[angle], |cpu| super::orbits::orbit_set(cpu, d, watched, 0x100, centre, angle));
    }
}

/// Four sparks from `e` (its position, direction and size): a pair across its direction a
/// quarter turn round, flung at 4 times its size; then a pair along it, larger, half as fast.
pub fn cross_set(cpu: &mut Cpu, e: u32) {
    cpu.write32(e + E_SCRIPT, CROSS);
    cpu.write32(e + E_SCALES, 0x2000);
    let a = cpu.read16(e + E_ANGLE).wrapping_add(0x4000);
    cpu.write16(e + E_ANGLE, a);
    let size = cpu.read32(e + E_SCALES + 4);
    let speed = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, size, 0x4_0000));
    cpu.write32(e + E_SPEED, speed);
    drift_effect(cpu, e);
    let a = cpu.read16(e + E_ANGLE).wrapping_add(0x8000);
    cpu.write16(e + E_ANGLE, a);
    drift_effect(cpu, e);
    cpu.write32(e + E_SCALES, 0x4000);
    for field in [E_SCALES + 4, E_SPEED] {
        let v = cpu.read32(e + field) as i32;
        cpu.write32(e + field, (v / 2) as u32);
    }
    drift_effect(cpu, e);
    let a = cpu.read16(e + E_ANGLE).wrapping_add(0x8000);
    cpu.write16(e + E_ANGLE, a);
    drift_effect(cpu, e);
}

/// cross_set with a flash of the screen (grey, faded back over 512 frames).
pub fn cross_flash(cpu: &mut Cpu, e: u32) {
    nested!(cpu, 0x0605_7da4, cross_set(cpu, e));
    nested!(cpu, 0x0605_e9d8, super::screen::stop_fades(cpu));
    nested!(cpu, 0x0605_e6dc, super::screen::set_fade_offset(cpu, 40, 40, 40));
    nested!(cpu, 0x0605_e93c, super::screen::fade_to(cpu, 0, 0, 0, 0x200));
}

/// A jet (the flame behind something): what it is behind (+0x3c), its reach (a byte, pixels),
/// a count, its width's size (a byte).
const JET_OF: u32 = 0x3c;
const JET_REACH: u32 = 0x40;
const JET_COUNT: u32 = 0x41;
const JET_SIZE: u32 = 0x42;

/// A jet's behaviour: ended with what it is behind; else just above it, its width flickering,
/// put its reach (and a pixel at random) behind it, its frames flickering, flipped at random;
/// each third frame a puff of exhaust drifting back.
pub fn jet_exec(cpu: &mut Cpu, j: u32) {
    let of = cpu.read32(j + JET_OF);
    if cpu.read8(of + OBJ_MODE) as i8 >= ENDING as i8 {
        end(cpu, j);
        return;
    }
    let z = cpu.read32(of + POSITION + 8).wrapping_add(1);
    cpu.write32(j + POSITION + 8, z);
    let r = random(cpu, 0x10000);
    let w = (cpu.read8(j + JET_SIZE) as u32).wrapping_mul((r & 0x3ff) + 0xc00);
    cpu.write32(j + SCALES, w);
    let a = cpu.read16(of + ANGLE);
    cpu.write16(j + ANGLE, a);
    let r = random(cpu, 0x10000);
    let reach = ((r & 1) as i32 + cpu.read8(j + JET_REACH) as i8 as i32) as u32 & 0xffff;
    let n = cpu.read8(j + JET_COUNT).wrapping_add(1);
    cpu.write8(j + JET_COUNT, n);
    let frame = if n & 1 != 0 {
        (random(cpu, 0x10000) & 1) as u8 + 0xd3
    } else {
        (random(cpu, 0x10000) & 3) as u8 + 0xcf
    };
    cpu.write8(j + FRAME, frame);
    let back = cpu.read16(of + ANGLE).wrapping_add(0x8000) as i16 as i32 as u32;
    let s = cpu.call(SL_SIN, &[back]);
    let x = cpu.read32(of + POSITION).wrapping_add(s.wrapping_mul(reach));
    cpu.write32(j + POSITION, x);
    let back = cpu.read16(of + ANGLE).wrapping_add(0x8000) as i16 as i32 as u32;
    let c = cpu.call(SL_COS, &[back]);
    let y = cpu.read32(of + POSITION + 4).wrapping_add(c.wrapping_neg().wrapping_mul(reach));
    cpu.write32(j + POSITION + 4, y);
    let flags = cpu.read32(j + FLAGS) & !0x10;
    cpu.write32(j + FLAGS, flags);
    let r = random(cpu, 0x10000);
    let flags = cpu.read32(j + FLAGS) | r & 0x10;
    cpu.write32(j + FLAGS, flags);
    let w = cpu.read32(j + SCALES);
    cpu.write32(j + SCALES + 4, w);
    let n = cpu.read8(j + JET_COUNT) as u32;
    if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, n, 3)) == 0 {
        let e = cpu.stack();
        cpu.write32(e + E_SCRIPT, JET_PUFF);
        let r = random(cpu, 0x10000) as u16;
        cpu.write16(e + E_COLOUR, r & 0xa000 | 0xd0);
        for k in 0..3 {
            let v = cpu.read32(j + POSITION + 4 * k);
            cpu.write32(e + E_POSITION + 4 * k, v);
        }
        let a = cpu.read16(of + ANGLE);
        cpu.write16(e + E_ANGLE, a);
        cpu.write32(e + E_SPEED, 0xfffc_c000);
        cpu.write32(e + E_SCALES, 0x8000);
        cpu.write32(e + E_SCALES + 4, 0x8000);
        drift_effect(cpu, e);
    }
}

/// Makes a jet behind the object at `d`+0x3c, its reach and size from `d`'s bytes +0x2d and
/// +0x2f; in list 3, else 2: it, or 0.
pub fn jet_set(cpu: &mut Cpu, d: u32) -> u32 {
    let j = piece_in(cpu, &[3, 2], 0x44);
    if j == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, j + SPRITE));
    cpu.write32(j + BEHAVIOUR, 0x0605_7e78);
    cpu.write32(j + FLAGS, PIECE_FLAGS);
    cpu.write8(j + KIND, 0);
    cpu.write8(j + FRAME, 0x2c);
    let base = cpu.read16(CHAR_BASE);
    cpu.write16(j + COLOUR, base);
    cpu.write8(j + LIFT, 8);
    cpu.write8(j + JET_COUNT, 0);
    let of = cpu.read32(d + 0x3c);
    cpu.write32(j + JET_OF, of);
    for k in 0..3 {
        let v = cpu.read32(of + POSITION + 4 * k);
        cpu.write32(j + POSITION + 4 * k, v);
    }
    let reach = cpu.read8(d + 0x2d);
    cpu.write8(j + JET_REACH, reach);
    let size = cpu.read8(d + 0x2f);
    cpu.write8(j + JET_SIZE, size);
    cpu.write32(j + SCALES, 0);
    cpu.write32(j + SCALES + 4, 0);
    j
}

/// A glow: its animation (actors.rs's script), its size's step (whose low word is also its spin)
/// and that step's slowing, the size it grows to.
const GLOW_STEP: u32 = 0x54;
const GLOW_SPIN: u32 = 0x56;
const GLOW_SLOWING: u32 = 0x58;
const GLOW_SIZE_TO: u32 = 0x5c;
const SCRIPT: u32 = 0x4c;
const SHOWN_LEFT: u32 = 0x50;
const GLOW_ANIMATION: u32 = 0x0607_31a0;
/// Past this frame it takes the attribute bit 0x100.
const GLOW_LATE: u8 = 0xcd;

/// A glow's behaviour: ended at its animation's end; else spun, its size grown toward its size
/// (the step slowing), its frame late in the animation taking the attribute bit.
pub fn glow_exec(cpu: &mut Cpu, g: u32) {
    if nested!(cpu, 0x0605_73a0, super::actors::anim_step(cpu, g)) as u8 != 0 {
        end(cpu, g);
        return;
    }
    let a = cpu.read16(g + ANGLE).wrapping_add(cpu.read16(g + GLOW_SPIN));
    cpu.write16(g + ANGLE, a);
    let (w, to, step) = (cpu.read32(g + SCALES), cpu.read32(g + GLOW_SIZE_TO), cpu.read32(g + GLOW_STEP));
    let w = nested!(cpu, 0x0604_6168, super::actors::approach(cpu, w, to, step));
    cpu.write32(g + SCALES, w);
    let (step, slowing) = (cpu.read32(g + GLOW_STEP), cpu.read32(g + GLOW_SLOWING));
    let step = nested!(cpu, 0x0604_6168, super::actors::approach(cpu, step, 0, slowing));
    cpu.write32(g + GLOW_STEP, step);
    let w = cpu.read32(g + SCALES);
    cpu.write32(g + SCALES + 4, w);
    if cpu.read8(g + FRAME) > GLOW_LATE {
        let flags = cpu.read32(g + FLAGS) | ATTR_100;
        cpu.write32(g + FLAGS, flags);
    }
}

/// Makes a glow at `d` (its position) growing from half its size (+0x2c) to it: in list 3; it,
/// or 0.
pub fn glow_set(cpu: &mut Cpu, d: u32) -> u32 {
    let g = piece_in(cpu, &[3], 100);
    if g == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, g + SPRITE));
    cpu.write32(g + BEHAVIOUR, 0x0605_8eac);
    cpu.write32(g + FLAGS, PIECE_FLAGS);
    let base = cpu.read16(CHAR_BASE);
    cpu.write16(g + COLOUR, base);
    cpu.write8(g + LIFT, 8);
    cpu.write32(g + SCRIPT, GLOW_ANIMATION);
    let first = cpu.read8(GLOW_ANIMATION);
    cpu.write8(g + FRAME, first);
    let script = cpu.read32(g + SCRIPT);
    let time = cpu.read8(script + 1);
    cpu.write8(g + SHOWN_LEFT, time);
    for k in 0..3 {
        let v = cpu.read32(d + 4 * k);
        cpu.write32(g + POSITION + 4 * k, v);
    }
    let size = cpu.read32(d + E_SIZE) as i32;
    cpu.write32(g + SCALES, (size / 2) as u32);
    cpu.write32(g + SCALES + 4, (size / 2) as u32);
    cpu.write32(g + GLOW_SIZE_TO, size as u32);
    cpu.write32(g + GLOW_STEP, shifted(size, 4));
    cpu.write32(g + GLOW_SLOWING, shifted(size, 8));
    let r = random(cpu, 0x10000);
    cpu.write16(g + ANGLE, r as u16);
    g
}

/// A rising ring (drawn by ring_draw): its speed (x, y, z); its width grows by its height, which
/// falls by 0x100 a frame (its width below none ends it); its colour (+0x30, from the first
/// character's).
const RING_SPEED: u32 = 0x3c;
const RING_COLOUR: u32 = ANGLE;

/// A rising ring's behaviour.
pub fn ring_exec(cpu: &mut Cpu, o: u32) {
    let w = cpu.read32(o + SCALES).wrapping_add(cpu.read32(o + SCALES + 4));
    cpu.write32(o + SCALES, w);
    let h = cpu.read32(o + SCALES + 4).wrapping_add(0xffff_ff00);
    cpu.write32(o + SCALES + 4, h);
    if (cpu.read32(o + SCALES) as i32) < 0 {
        end(cpu, o);
        return;
    }
    for k in 0..3 {
        let p = cpu.read32(o + POSITION + 4 * k).wrapping_add(cpu.read32(o + RING_SPEED + 4 * k));
        cpu.write32(o + POSITION + 4 * k, p);
    }
}

/// The ring's sprite (in the draw's frame, sprites.rs's struct).
const R_FLAGS: u32 = 0x00;
const R_POSITION: u32 = 0x04;
const R_SCALES: u32 = 0x10;
const R_COLOUR: u32 = 0x18;
const R_KIND: u32 = 0x1e;
const R_FRAME: u32 = 0x1f;
const R_LIFT: u32 = 0x23;
const LOOPS: u32 = super::globals::LOOPS;
const COLOUR_TABLE: u32 = 0x0607_31b6;
const GAME_FRAMES_LOW: u32 = GAME_FRAMES + 3;

/// A rising ring's draw (sprite `s`): while its height is above none, an outer ring (frame 2, half
/// again as wide); then the ring (frame 0 or 1 by turns), in its colour with the colours of
/// orbits.rs's table by turns.
pub fn ring_draw(cpu: &mut Cpu, s: u32) {
    let o = s - SPRITE;
    let r = cpu.stack();
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, r));
    for k in 0..3 {
        let v = cpu.read32(o + POSITION + 4 * k);
        cpu.write32(r + R_POSITION + 4 * k, v);
    }
    let turn = (cpu.read32(LOOPS) & 1) * 2;
    let colour = cpu
        .read16(CHAR_BASE)
        .wrapping_add(cpu.read16(o + RING_COLOUR))
        .wrapping_add(cpu.read16(COLOUR_TABLE + turn));
    cpu.write16(r + R_COLOUR, colour);
    cpu.write8(r + R_LIFT, 0xf8);
    cpu.write8(r + R_KIND, 0);
    if cpu.read32(o + SCALES + 4) as i32 > 0 {
        cpu.write32(r + R_FLAGS, ATTR_100);
        cpu.write8(r + R_FRAME, 2);
        let w = cpu.read32(o + SCALES);
        let w = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, w, 0x1_8000));
        cpu.write32(r + R_SCALES, w);
        cpu.write32(r + R_SCALES + 4, w);
        nested!(cpu, 0x0604_0960, sprites::draw(cpu, r));
    }
    cpu.write32(r + R_FLAGS, 0);
    let f = cpu.read8(GAME_FRAMES_LOW) & 1;
    cpu.write8(r + R_FRAME, f);
    let w = cpu.read32(o + SCALES);
    cpu.write32(r + R_SCALES, w);
    cpu.write32(r + R_SCALES + 4, w);
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, r));
}

/// Makes a rising ring from `d`: its position, its width and height (words +0x2c, +0x2e), its
/// speed (+0x30), its colour (a word, +0x20); in list 3.
pub fn ring_set(cpu: &mut Cpu, d: u32) {
    let o = piece_in(cpu, &[3], 0x48);
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + SPRITE));
    cpu.write32(o + BEHAVIOUR, 0x0605_9034);
    cpu.write32(o + DRAW_AT, 0x0605_9080);
    cpu.write32(o + FLAGS, OWN_MODEL);
    for k in 0..3 {
        let v = cpu.read32(d + 4 * k);
        cpu.write32(o + POSITION + 4 * k, v);
    }
    for k in 0..3 {
        let v = cpu.read32(d + 0x30 + 4 * k);
        cpu.write32(o + RING_SPEED + 4 * k, v);
    }
    for k in 0..2 {
        let v = cpu.read16(d + 0x2c + 2 * k) as i16 as i32 as u32;
        cpu.write32(o + SCALES + 4 * k, v);
    }
    let colour = cpu.read16(d + 0x20);
    cpu.write16(o + RING_COLOUR, colour);
}

/// Smoke and sparks at `p` (x, y, z) every other game frame (its sound, one of four at random,
/// at most every 50 frames): a puff 176 nearer, of 3/4 size less up to 7 steps at random, and
/// `n`/2 twin sparks. Whether it made them.
const SMOKE_SOUND_LAST: u32 = 0x0607_320c;
const SMOKE_SOUNDS: u32 = 0x0607_31fe;
const SMOKE_SOUND_GAP: u32 = 50;
const SMOKE_NEARER: u32 = 0xff50_0000;
const SMOKE_SIZE: u32 = 0xc000;
const SMOKE_SIZE_STEP: u32 = 0x600;

pub fn smoke_and_sparks(cpu: &mut Cpu, p: u32, n: u32) -> u32 {
    let now = cpu.read32(super::globals::GAME_FRAMES);
    if now.wrapping_sub(cpu.read32(SMOKE_SOUND_LAST)) > SMOKE_SOUND_GAP {
        cpu.write32(SMOKE_SOUND_LAST, now);
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
        let se = cpu.read16(SMOKE_SOUNDS + (r & 3) * 4) as i16 as i32 as u32;
        nested!(cpu, 0x0600_50d4, super::sound::se_set(cpu, se));
    }
    if cpu.read32(super::globals::GAME_FRAMES) & 1 != 0 {
        return 0;
    }
    let f = cpu.stack();
    for k in 0..2 {
        let v = cpu.read32(p + 4 * k);
        cpu.write32(f + 4 * k, v);
    }
    let z = cpu.read32(p + 8).wrapping_add(SMOKE_NEARER);
    cpu.write32(f + 8, z);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write32(f + 12, SMOKE_SIZE.wrapping_sub((r & 7) * SMOKE_SIZE_STEP));
    nested!(cpu, 0x0605_77d0, smoke_puff(cpu, f));
    let mut m = (n as i8) / 2;
    while m > 0 {
        nested!(cpu, 0x0605_8278, twin_set(cpu, f));
        m -= 1;
    }
    1
}
