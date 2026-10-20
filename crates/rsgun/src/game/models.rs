//! The model actors: actors drawn as 3D models by draws of their own (sprites.rs's OWN_DRAW), in
//! groups that keep SGL's matrix stack open for the parts that follow (a group's draw pushes the
//! group's place, its parts' draws inside it, the group's end pops it). A part keeps the matrix
//! it was drawn with, so points can be placed on it later, and brings its boxes from model space
//! to the screen each frame. A beam's boxes are a column of one box repeated.

use super::globals::{GAME_FLAGS, HELD};
use super::{hits, nested, sprites, Cpu};

/// SGL's.
const SL_PUSH_MATRIX: u32 = 0x0606_71cc;
const SL_POP_MATRIX: u32 = 0x0606_7200;
const SL_TRANSLATE: u32 = 0x0606_721c;
const SL_ROT_X: u32 = 0x0606_72ec;
const SL_ROT_Y: u32 = 0x0606_736c;
const SL_ROT_Z: u32 = 0x0606_726c;
const SL_SCALE: u32 = 0x0606_73ec;
const SL_PUT_POLYGON: u32 = 0x0606_745c;
const SL_UNIT_MATRIX: u32 = 0x0603_512e;
/// A point through the current matrix (x, y, z, where to).
const SL_CALC_POINT: u32 = 0x0606_db2c;
/// The current matrix saved (where to), a matrix loaded, and (as far as can be told) the matrix
/// stack pushed without a copy.
const SL_GET_MATRIX: u32 = 0x0606_db9c;
const SL_LOAD_MATRIX: u32 = 0x0606_db7c;
const SL_PUSH_NO_COPY: u32 = 0x0606_db5c;
/// A point brought into view space (sprites.rs's).
const SL_CONVERT: u32 = 0x0606_750c;

/// The objects' mode byte: ending at and past ENDING.
const OBJ_MODE: u32 = 3;
const ENDING: u8 = 0x70;
/// The numbered objects (actors.rs's).
const NUMBERED: u32 = 0x0607_40f0;

/// An actor's sprite (sprites.rs's struct: its flags, position, scales, turns; the models from
/// +0x34 there), its hit fields and boxes, its hit flags.
const SPRITE: u32 = 0x10;
const FLAGS: u32 = 0x10;
const POSITION: u32 = 0x14;
const SCALES: u32 = 0x20;
const BOXES: u32 = 0x3c;
const MODELS: u32 = 0x44;
const HIT_FLAGS: u32 = 0x56;
/// The sprite's fields, from the sprite: its position, its turns (words: x, y, z) and its scales.
const S_POSITION: u32 = 0x04;
const S_TURN_X: u32 = 0x1c;
const S_TURN_Y: u32 = 0x1e;
const S_TURN_Z: u32 = 0x20;
/// The flags: flipped (x, y), the boxes not flipped, not hurt, no hits.
const FLIP_X: u32 = 0x10;
const FLIP_Y: u32 = 0x20;
const BOXES_UNFLIPPED: u32 = 0x1_0000;
const UNHURT: u32 = 0x4_0000;
const NO_HITS: u32 = 0x8_0000;
/// The hit flags' flash (the flashing model drawn).
const FLASH: u16 = 1;

/// A model actor's own fields: the parent's number (numbered parts), its place in its group
/// (x, y, z), the matrix it was last drawn with (saved, 12 longs), its model boxes (none: 0) and the
/// boxes made from them (BOXES points there).
const PARENT: u32 = 0x6f;
const PLACE: u32 = 0xd8;
const MATRIX: u32 = 0xe4;
const MODEL_BOXES: u32 = 0x114;
const MADE_BOXES: u32 = 0x118;
/// Cleared with the rest (not known yet).
const FIELD_6C: u32 = 0x6c;
const FIELD_6E: u32 = 0x6e;
const FIELD_B0: u32 = 0xb0;

/// The camera (camera.rs's) and the eye's depth it is to have: the model's depths are from it.
const CAMERA: u32 = 0x0607_46e4;
const EYE_TO_Z: u32 = 0x1c;

/// A model actor's fields set up: its sprite and hit fields, its boxes not flipped and not hurt,
/// its place nothing, no model boxes.
pub fn model_init(cpu: &mut Cpu, obj: u32) {
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, obj + SPRITE));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, obj + BOXES));
    let flags = cpu.read32(obj + FLAGS) | UNHURT | BOXES_UNFLIPPED;
    cpu.write32(obj + FLAGS, flags);
    for field in [FIELD_B0, PLACE + 8, PLACE + 4, PLACE, MODEL_BOXES] {
        cpu.write32(obj + field, 0);
    }
    cpu.write8(obj + FIELD_6E, 0);
    cpu.write16(obj + FIELD_6C, 0);
}

/// A part follows its parent (by number): no hits while the parent has none, ending as it ends.
/// The parent (the stages' code takes it).
pub fn follow_parent(cpu: &mut Cpu, obj: u32) -> u32 {
    let n = cpu.read8(obj + PARENT) as i8 as i32;
    let parent = cpu.read32(NUMBERED.wrapping_add((n * 4) as u32));
    let flags = cpu.read32(obj + FLAGS) | cpu.read32(parent + FLAGS) & NO_HITS;
    cpu.write32(obj + FLAGS, flags);
    if cpu.read8(parent + OBJ_MODE) as i8 >= ENDING as i8 {
        cpu.write8(obj + OBJ_MODE, ENDING);
    }
    parent
}

/// An actor that ends with the object at +0x3c (once that is freed or ends).
pub fn end_with_parent(cpu: &mut Cpu, obj: u32) {
    let parent = cpu.read32(obj + BOXES);
    if cpu.read8(parent) == 0 || cpu.read8(parent + OBJ_MODE) as i8 >= ENDING as i8 {
        cpu.write8(obj + OBJ_MODE, ENDING);
    }
}

/// A model box (12 bytes, the last with LAST_BOX): what it is and its kind (as a box's), its point
/// in model space (x, y, z: words), its half width and height (bytes) and how far from the eye's
/// depth it counts (a byte, whole units).
const MODEL_BOX: u32 = 12;
const MB_POINT: u32 = 2;
const MB_HALF: u32 = 8;
const MB_REACH: u32 = 10;
/// A box (hits.rs's, 8 bytes): what, kind, offset (x, y: words), half width and height.
const BOX: u32 = 8;
const B_OFFSET: u32 = 2;
const B_HALF: u32 = 6;
const LAST_BOX: u8 = 0x80;

fn camera_depth(cpu: &mut Cpu) -> u32 {
    let camera = cpu.read32(CAMERA);
    cpu.read32(camera + EYE_TO_Z)
}

/// The whole units of a 16.16 value, rounded toward zero.
fn whole(v: i32) -> u16 {
    ((if v < 0 { v.wrapping_add(0xffff) } else { v }) >> 16) as u16
}

/// The model actor's position and boxes for this frame (with hits, and model boxes): its position
/// the model's origin through the current matrix (its depth from the eye's), and a box made of
/// each model box whose point is within its reach of the eye's depth (offset from the position).
pub fn model_boxes(cpu: &mut Cpu, obj: u32) {
    if cpu.read32(obj + FLAGS) & NO_HITS != 0 {
        return;
    }
    if cpu.read32(obj + MODEL_BOXES) == 0 {
        return;
    }
    cpu.call(SL_CALC_POINT, &[0, 0, 0, obj + POSITION]);
    let z = cpu.read32(obj + POSITION + 8).wrapping_add(camera_depth(cpu));
    cpu.write32(obj + POSITION + 8, z);
    cpu.write32(obj + BOXES, obj + MADE_BOXES);
    let point = cpu.stack();
    let (mut made, mut last) = (0, 0);
    let mut at = 0;
    loop {
        let src = cpu.read32(obj + MODEL_BOXES) + at;
        let p = [0, 1, 2].map(|i| ((cpu.read16(src + MB_POINT + 2 * i) as i16 as i32) << 16) as u32);
        cpu.call(SL_CALC_POINT, &[p[0], p[1], p[2], point]);
        let depth = (cpu.read32(point + 8).wrapping_add(camera_depth(cpu)) as i32).wrapping_abs();
        if depth <= (cpu.read8(src + MB_REACH) as i32) << 16 {
            let dst = cpu.read32(obj + BOXES) + BOX * made;
            last = dst;
            for i in 0..2 {
                let v = cpu.read8(src + i);
                cpu.write8(dst + i, v);
            }
            for i in 0..2 {
                let v = cpu.read32(point + 4 * i).wrapping_sub(cpu.read32(obj + POSITION + 4 * i)) as i32;
                cpu.write16(dst + B_OFFSET + 2 * i, whole(v));
            }
            made += 1;
            for i in 0..2 {
                let v = cpu.read8(src + MB_HALF + i);
                cpu.write8(dst + B_HALF + i, v);
            }
        }
        at += MODEL_BOX;
        if cpu.read8(src) & LAST_BOX != 0 {
            break;
        }
    }
    if made == 0 {
        cpu.write32(obj + BOXES, 0);
    } else {
        let v = cpu.read8(last) | LAST_BOX;
        cpu.write8(last, v);
    }
}

/// A beam's boxes (24 from +0xb4, the column's length set by beam_length).
const BEAM: u32 = 0xb4;
const BEAM_BOXES: u32 = 24;

/// The beam's boxes: `src` repeated up a column (each its height and 4 pixels above the last).
pub fn beam_boxes(cpu: &mut Cpu, obj: u32, src: u32) {
    let mut y: i32 = 0;
    for k in 0..BEAM_BOXES {
        let dst = obj + BEAM + BOX * k;
        let what = cpu.read8(src);
        cpu.write8(dst, what);
        let kind = cpu.read8(src + 1);
        cpu.write8(dst + 1, kind);
        let x = cpu.read16(src + B_OFFSET);
        cpu.write16(dst + B_OFFSET, x);
        let sy = cpu.read16(src + B_OFFSET + 2);
        cpu.write16(dst + B_OFFSET + 2, sy.wrapping_add(y as u16));
        let w = cpu.read8(src + B_HALF);
        cpu.write8(dst + B_HALF, w);
        let h = cpu.read8(src + B_HALF + 1);
        cpu.write8(dst + B_HALF + 1, h);
        y = y - 4 - 2 * h as i32;
    }
}

/// The beam's boxes to `length` (16.16 pixels): its first box a step back by the game frame
/// (0, 2, 1, 3 steps by frames modulo 4: the column scrolls; no further back than the length),
/// each box after it a box's height (twice its half height and 4) further, the last the one that
/// reaches the length (at most the 24th).
pub fn beam_length(cpu: &mut Cpu, obj: u32, length: u32) {
    let length = length as i32;
    let step = cpu.read8(obj + BEAM + B_HALF + 1) as i32 * 2 + 4;
    let phase = match cpu.read32(super::globals::GAME_FRAMES) & 3 {
        1 => 2,
        2 => 1,
        p => p as i32,
    };
    let mut y = (-step).wrapping_mul(phase);
    if y.wrapping_abs() << 16 > length {
        y = whole(length.wrapping_neg()) as i16 as i32;
    }
    let mut k = 0;
    loop {
        let dst = obj + BEAM + BOX * k;
        cpu.write16(dst + B_OFFSET + 2, y as u16);
        let reach = ((cpu.read16(dst + B_OFFSET + 2) as i16 as i32).wrapping_abs() + step * 4) << 16;
        if length <= reach {
            break;
        }
        let what = cpu.read8(dst) & !LAST_BOX;
        k += 1;
        y -= step * 4;
        cpu.write8(dst, what);
        if k > BEAM_BOXES - 2 {
            break;
        }
    }
    let last = obj + BEAM + BOX * k;
    let what = cpu.read8(last) | LAST_BOX;
    cpu.write8(last, what);
}

/// The point at `p` (x, y, z: in the model actor's space when it was last drawn) placed in the
/// world, in place (its depth from the eye's).
pub fn model_point(cpu: &mut Cpu, obj: u32, p: u32) {
    point_through_matrix(cpu, obj, p);
    let z = cpu.read32(p + 8).wrapping_add(camera_depth(cpu));
    cpu.write32(p + 8, z);
    cpu.call(SL_POP_MATRIX, &[]);
}

/// model_point brought into view space (its depth then 0).
pub fn model_screen_point(cpu: &mut Cpu, obj: u32, p: u32) {
    point_through_matrix(cpu, obj, p);
    cpu.call(SL_UNIT_MATRIX, &[0]);
    cpu.call(SL_CONVERT, &[p, p]);
    cpu.write32(p + 8, 0);
    cpu.call(SL_POP_MATRIX, &[]);
}

fn point_through_matrix(cpu: &mut Cpu, obj: u32, p: u32) {
    cpu.call(SL_PUSH_NO_COPY, &[]);
    cpu.call(SL_LOAD_MATRIX, &[obj + MATRIX]);
    let v = [0, 1, 2].map(|i| cpu.read32(p + 4 * i));
    cpu.call(SL_CALC_POINT, &[v[0], v[1], v[2], p]);
}

/// A group's draw (sprite `s`): the matrix pushed, moved to its position and turned (z, x, y; a
/// turn of 0 not made), left open for its parts.
pub fn group_draw(cpu: &mut Cpu, s: u32) {
    cpu.call(SL_PUSH_MATRIX, &[]);
    let p = [0, 1, 2].map(|i| cpu.read32(s + S_POSITION + 4 * i));
    cpu.call(SL_TRANSLATE, &p);
    turn(cpu, s);
}

fn turn(cpu: &mut Cpu, s: u32) {
    for (field, rot) in [(S_TURN_Z, SL_ROT_Z), (S_TURN_X, SL_ROT_X), (S_TURN_Y, SL_ROT_Y)] {
        let a = cpu.read16(s + field) as i16;
        if a != 0 {
            cpu.call(rot, &[a as i32 as u32]);
        }
    }
}

/// A group's end: its matrix popped.
pub fn group_end(cpu: &mut Cpu, _s: u32) {
    cpu.call(SL_POP_MATRIX, &[]);
}

/// A part's draw (sprite `s`), left open for the parts inside it: the matrix pushed, moved to the
/// part's place in its group, turned (z, x, y), scaled (flipped by the flags), the model (the
/// flashing one while it flashes) put; its boxes made, and its matrix saved (not while the game is
/// held).
pub fn part_draw(cpu: &mut Cpu, s: u32) {
    let obj = s - SPRITE;
    cpu.call(SL_PUSH_MATRIX, &[]);
    let p = [0, 1, 2].map(|i| cpu.read32(obj + PLACE + 4 * i));
    cpu.call(SL_TRANSLATE, &p);
    turn(cpu, s);
    let mut sx = cpu.read32(obj + SCALES);
    let flags = cpu.read32(s);
    let mut sy = cpu.read32(obj + SCALES + 4);
    if flags & FLIP_X != 0 {
        sx = sx.wrapping_neg();
    }
    if flags & FLIP_Y != 0 {
        sy = sy.wrapping_neg();
    }
    let sz = cpu.read32(obj + SCALES + 8);
    cpu.call(SL_SCALE, &[sx, sy, sz]);
    let flashing = cpu.read16(obj + HIT_FLAGS) & FLASH;
    let models = cpu.read32(obj + MODELS);
    let model = cpu.read32(models + 4 * flashing as u32);
    cpu.call(SL_PUT_POLYGON, &[model]);
    nested!(cpu, 0x0605_29ac, model_boxes(cpu, obj));
    if cpu.read32(GAME_FLAGS) & HELD == 0 {
        cpu.call(SL_GET_MATRIX, &[obj + MATRIX]);
    }
}

/// A part's draw, closed (its matrix popped after).
pub fn model_draw(cpu: &mut Cpu, s: u32) {
    // inline
    part_draw(cpu, s);
    cpu.call(SL_POP_MATRIX, &[]);
}
