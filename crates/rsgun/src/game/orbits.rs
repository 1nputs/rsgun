//! The orbiters: effects (plain actors, list 3) that circle a centre (a position they follow) at a
//! radius and an angle that both move, until a word they watch has any of their mask's bits, the
//! screen is handed over, or their kind says they are done. Each kind has its own start and its
//! own frame (two tables by kind).

use super::globals::GAME_FRAMES;
use super::{director, math, nested, objects, sprites, Cpu};

/// The objects' mode byte (ending).
const OBJ_MODE: u32 = 3;
const ENDING: u8 = 0x70;

/// An orbiter: its sprite's flags, position, scales, colour, kind, frame, angle and lift; the word
/// it watches and the mask it watches for, its centre, where its angle comes from (a word, plus
/// its own offset; or its own angle), its kind, its colours' index, its scales' speeds, its angle
/// offset and turn (words), its radius and the radius's speed.
const SPRITE: u32 = 0x10;
const FLAGS: u32 = 0x10;
const POSITION: u32 = 0x14;
const SCALES: u32 = 0x20;
const COLOUR: u32 = 0x28;
const SPRITE_KIND: u32 = 0x2e;
const FRAME: u32 = 0x2f;
const ANGLE: u32 = 0x30;
const LIFT: u32 = 0x33;
const WATCHED: u32 = 0x3c;
const MASK: u32 = 0x40;
const CENTRE: u32 = 0x44;
const ANGLE_FROM: u32 = 0x48;
const BYTE_4C: u32 = 0x4c;
const KIND: u32 = 0x4d;
const COLOURS_AT: u32 = 0x4e;
const SCALE_SPEED: u32 = 0x50;
const ANGLE_OFFSET: u32 = 0x58;
const TURN: u32 = 0x5a;
const RADIUS: u32 = 0x5c;
const RADIUS_SPEED: u32 = 0x60;
/// The flags: not drawn (until its first frame), and the start's (an attribute, never culled).
const NO_DRAW: u32 = 0x8000_0000;
const START_FLAGS: u32 = 0x8000_0800;

/// The kinds' starts and frames (SH-2 functions, by kind).
const STARTS: u32 = 0x0607_31d4;
const FRAMES: u32 = 0x0607_31e8;
/// The colours by index (words, from the first character's): an orbiter takes the one at its
/// index and the next by turns.
const COLOUR_TABLE: u32 = 0x0607_31b6;
const CHAR_BASE: u32 = 0x0607_41fc;
/// The director's flags (director.rs) and the bit that hands the screen over.
const HANDED_OVER: u32 = 8;

/// SGL's sine and cosine.
const SL_SIN: u32 = 0x0606_9a0c;
const SL_COS: u32 = 0x0606_9a30;

/// The orbiter `o` placed: 1 (to end) once its watched word has a bit of its mask; else its angle
/// (from its source and offset, or its own) turned, and it put at its radius from its centre in
/// that direction (0 straight up), at the centre's depth: 0.
pub fn orbit_place(cpu: &mut Cpu, o: u32) -> u32 {
    let watched = cpu.read32(o + WATCHED);
    if cpu.read16(watched) & cpu.read16(o + MASK) != 0 {
        return 1;
    }
    let from = cpu.read32(o + ANGLE_FROM);
    let turned = if from != 0 {
        let a = cpu.read16(from).wrapping_add(cpu.read16(o + ANGLE_OFFSET));
        cpu.write16(o + ANGLE, a);
        o + ANGLE_OFFSET
    } else {
        o + ANGLE
    };
    let a = cpu.read16(turned).wrapping_add(cpu.read16(o + TURN));
    cpu.write16(turned, a);
    let radius = cpu.read32(o + RADIUS);
    let angle = cpu.read16(o + ANGLE) as i16 as i32 as u32;
    let s = cpu.call(SL_SIN, &[angle]);
    let dx = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, radius));
    let centre = cpu.read32(o + CENTRE);
    let x = cpu.read32(centre).wrapping_add(dx);
    cpu.write32(o + POSITION, x);
    let radius = cpu.read32(o + RADIUS);
    let angle = cpu.read16(o + ANGLE) as i16 as i32 as u32;
    let c = cpu.call(SL_COS, &[angle]);
    let dy = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, c, radius));
    let centre = cpu.read32(o + CENTRE);
    let y = cpu.read32(centre + 4).wrapping_sub(dy);
    cpu.write32(o + POSITION + 4, y);
    let z = cpu.read32(centre + 8);
    cpu.write32(o + POSITION + 8, z);
    0
}

/// An orbiter's behaviour: ended once the screen is handed over, its watched word says so or its
/// kind's frame says so; else shown, its scales and radius moved by their speeds.
pub fn orbit_exec(cpu: &mut Cpu, o: u32) {
    let ends = nested!(cpu, 0x0604_0cb4, director::director_flags(cpu)) & HANDED_OVER != 0
        || nested!(cpu, 0x0605_9418, orbit_place(cpu, o)) as u8 != 0
        || {
            let kind = cpu.read8(o + KIND) as u32;
            let frame = cpu.read32(FRAMES + 4 * kind);
            cpu.call(frame, &[o]) as u8 != 0
        };
    if ends {
        cpu.write8(o + OBJ_MODE, ENDING);
        return;
    }
    let flags = cpu.read32(o + FLAGS) & !NO_DRAW;
    cpu.write32(o + FLAGS, flags);
    for k in 0..2 {
        let v = cpu.read32(o + SCALES + 4 * k).wrapping_add(cpu.read32(o + SCALE_SPEED + 4 * k));
        cpu.write32(o + SCALES + 4 * k, v);
    }
    let r = cpu.read32(o + RADIUS).wrapping_add(cpu.read32(o + RADIUS_SPEED));
    cpu.write32(o + RADIUS, r);
}

/// An orbiter's description (its maker's): its scales, its angle (a word), its colours' index (a
/// byte), its radius and the radius's speed, its kind (a byte), its angle offset (or, without an
/// angle source, the angle's offset) and turn (words), its lift, its scales' speeds.
const D_SCALES: u32 = 0x0c;
const D_ANGLE: u32 = 0x18;
const D_COLOURS_AT: u32 = 0x21;
const D_RADIUS: u32 = 0x24;
const D_KIND: u32 = 0x2d;
const D_ANGLE_OFFSET: u32 = 0x2e;
const D_TURN: u32 = 0x30;
const D_LIFT: u32 = 0x33;
const D_SCALE_SPEED: u32 = 0x34;
const ORBIT_SIZE: u32 = 0x68;
const ORBIT_LIST: u32 = 3;
const PLAIN_EXEC: u32 = 0x0604_dcb4;
const ORBIT_EXEC: u32 = 0x0605_94cc;

/// Makes an orbiter from the description `d` around `centre` (its position, three longs), ending
/// once the word at `watched` has a bit of `mask`; its angle from the word at `angle_from` (0: its
/// own).
pub fn orbit_set(cpu: &mut Cpu, d: u32, watched: u32, mask: u32, centre: u32, angle_from: u32) {
    // the SH-2 code keeps the mask in its frame
    let frame = cpu.stack();
    cpu.write32(frame, mask & 0xffff);
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, ORBIT_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, ORBIT_LIST, blocks, PLAIN_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + SPRITE));
    cpu.write32(o + 0x38, ORBIT_EXEC);
    cpu.write32(o + FLAGS, START_FLAGS);
    cpu.write8(o + SPRITE_KIND, 0);
    cpu.write32(o + WATCHED, watched);
    cpu.write16(o + MASK, mask as u16);
    cpu.write32(o + CENTRE, centre);
    cpu.write32(o + ANGLE_FROM, angle_from);
    for k in 0..3 {
        let v = cpu.read32(centre + 4 * k);
        cpu.write32(o + POSITION + 4 * k, v);
    }
    let kind = cpu.read8(d + D_KIND);
    cpu.write8(o + KIND, kind);
    let start = cpu.read32(STARTS + 4 * kind as u32);
    cpu.call(start, &[o]);
    let colours = cpu.read8(d + D_COLOURS_AT);
    cpu.write8(o + COLOURS_AT, colours);
    for k in 0..2 {
        let v = cpu.read32(d + D_SCALES + 4 * k);
        cpu.write32(o + SCALES + 4 * k, v);
    }
    let angle = cpu.read16(d + D_ANGLE);
    cpu.write16(o + ANGLE, angle);
    if cpu.read32(o + ANGLE_FROM) != 0 {
        let v = cpu.read16(d + D_ANGLE_OFFSET);
        cpu.write16(o + ANGLE_OFFSET, v);
        let v = cpu.read16(d + D_TURN);
        cpu.write16(o + TURN, v);
    } else {
        let v = cpu.read16(d + D_ANGLE_OFFSET).wrapping_add(angle);
        cpu.write16(o + ANGLE, v);
    }
    cpu.write8(o + BYTE_4C, 0);
    for k in 0..2 {
        let v = cpu.read32(d + D_SCALE_SPEED + 4 * k);
        cpu.write32(o + SCALE_SPEED + 4 * k, v);
    }
    for k in 0..2 {
        let v = cpu.read32(d + D_RADIUS + 4 * k);
        cpu.write32(o + RADIUS + 4 * k, v);
    }
    let lift = cpu.read8(d + D_LIFT);
    cpu.write8(o + LIFT, lift);
}

/// The orbiter's colour this frame: its index's colour or the next, by turns.
fn colour(cpu: &mut Cpu, o: u32) {
    let at = cpu.read8(o + COLOURS_AT) as u32 + (cpu.read32(GAME_FRAMES) & 1);
    let c = cpu.read16(CHAR_BASE).wrapping_add(cpu.read16(COLOUR_TABLE + 2 * at));
    cpu.write16(o + COLOUR, c);
}

/// The sprite's flags an orbiter may take at random: flipped, an attribute bit.
const RANDOM_FLAGS: u32 = 0x110;

/// Kind 0's (and 3's) start: flipped at random, one of 7 frames from 0xa7 at random.
pub fn start_0(cpu: &mut Cpu, o: u32) {
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x10000));
    cpu.write32(o + FLAGS, r & RANDOM_FLAGS);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x10000)) & 0xffff;
    let f = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, r, 7));
    cpu.write8(o + FRAME, (f as u8).wrapping_add(0xa7));
}

/// Kind 1's start: flipped at random, frame 0x35.
pub fn start_1(cpu: &mut Cpu, o: u32) {
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x10000));
    cpu.write32(o + FLAGS, r & RANDOM_FLAGS);
    cpu.write8(o + FRAME, 0x35);
}

/// Kind 2's start: frame 0xd6.
pub fn start_2(cpu: &mut Cpu, o: u32) {
    cpu.write8(o + FRAME, 0xd6);
}

/// Kind 4's start: flags 0x20, frame 0xb4.
pub fn start_4(cpu: &mut Cpu, o: u32) {
    cpu.write32(o + FLAGS, 0x20);
    cpu.write8(o + FRAME, 0xb4);
}

/// Kind 0's frame: done once its height is below none; else coloured.
pub fn frame_0(cpu: &mut Cpu, o: u32) -> u32 {
    if (cpu.read32(o + SCALES + 4) as i32) < 0 {
        return 1;
    }
    colour(cpu, o);
    0
}

/// Kind 1's frame: its frame stepped every other frame; done once its width is below none or its
/// frame past 60; else coloured.
pub fn frame_1(cpu: &mut Cpu, o: u32) -> u32 {
    if cpu.read32(GAME_FRAMES) & 1 != 0 {
        let f = cpu.read8(o + FRAME).wrapping_add(1);
        cpu.write8(o + FRAME, f);
    }
    if (cpu.read32(o + SCALES) as i32) < 0 || cpu.read8(o + FRAME) > 60 {
        return 1;
    }
    colour(cpu, o);
    0
}

/// Kind 2's frame: frames 0xd6 and 0xd5 by turns; done once its width is below none; else
/// coloured and its depth less 0xb0.
pub fn frame_2(cpu: &mut Cpu, o: u32) -> u32 {
    let f = if cpu.read32(GAME_FRAMES) & 1 != 0 { 0xd5 } else { 0xd6 };
    cpu.write8(o + FRAME, f);
    if (cpu.read32(o + SCALES) as i32) < 0 {
        return 1;
    }
    colour(cpu, o);
    let z = cpu.read32(o + POSITION + 8).wrapping_add(0xff50_0000);
    cpu.write32(o + POSITION + 8, z);
    0
}

/// Kind 4's frame: done once its width is below none; else frames 0xb3 and 0xb4 by turns, and
/// coloured.
pub fn frame_4(cpu: &mut Cpu, o: u32) -> u32 {
    if (cpu.read32(o + SCALES) as i32) < 0 {
        return 1;
    }
    let f = (cpu.read32(GAME_FRAMES) & 1) as u8 + 0xb3;
    cpu.write8(o + FRAME, f);
    colour(cpu, o);
    0
}
