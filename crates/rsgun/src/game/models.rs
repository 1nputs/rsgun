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

/// A model's pose script (the struct at `p`): the joints' count (a word), their angles and turns
/// a frame (word arrays), the poses (each an array of the joints' angles), the script (words,
/// three a step: the pose, the frames to reach it, the frames held after it; a negative word is
/// flags: SCRIPT_AGAIN starts the script over, SCRIPT_STOP stops it), the step (in words), its
/// frames left, and the flags. Each frame the joints turn; a new step sets each joint's turn to
/// reach its pose over the step's frames.
const P_COUNT: u32 = 0x10;
const P_ANGLES: u32 = 0x14;
const P_TURNS: u32 = 0x18;
const P_POSES: u32 = 0x1c;
const P_SCRIPT: u32 = 0x20;
const P_AT: u32 = 0x24;
const P_LEFT: u32 = 0x26;
const P_FLAGS: u32 = 0x28;
const STEP: u16 = 3;
const SCRIPT_AGAIN: u16 = 0x2000;
const SCRIPT_STOP: u16 = 0x4000;

pub fn pose_step(cpu: &mut Cpu, p: u32) {
    if cpu.read16(p + P_FLAGS) & SCRIPT_STOP != 0 {
        return;
    }
    cpu.write16(p + P_FLAGS, 0);
    let (angles, turns) = (cpu.read32(p + P_ANGLES), cpu.read32(p + P_TURNS));
    for i in 0..cpu.read16(p + P_COUNT) as u32 {
        let a = cpu.read16(angles + 2 * i).wrapping_add(cpu.read16(turns + 2 * i));
        cpu.write16(angles + 2 * i, a);
    }
    if cpu.read16(p + P_LEFT) == 0 {
        let script = cpu.read32(p + P_SCRIPT);
        let word = |cpu: &mut Cpu, k: u32| {
            let at = cpu.read16(p + P_AT) as u32;
            cpu.read16(script + 2 * (at + k))
        };
        let at = cpu.read16(p + P_AT).wrapping_add(STEP);
        cpu.write16(p + P_AT, at);
        let w = word(cpu, 0);
        if (w as i16) < 0 {
            cpu.write16(p + P_FLAGS, w);
            cpu.write16(p + P_AT, at.wrapping_add(1));
            if w & SCRIPT_AGAIN != 0 {
                cpu.write16(p + P_AT, 0);
            }
            if w & SCRIPT_STOP != 0 {
                return;
            }
        }
        let frames = word(cpu, 1);
        cpu.write16(p + P_LEFT, frames);
        let pose = word(cpu, 0) as i16 as i32;
        let poses = cpu.read32(p + P_POSES);
        let mut target = cpu.read32(poses.wrapping_add((4 * pose) as u32));
        let (angles, turns) = (cpu.read32(p + P_ANGLES), cpu.read32(p + P_TURNS));
        for i in 0..cpu.read16(p + P_COUNT) as u32 {
            let frames = cpu.read16(p + P_LEFT) as i16 as i32 as u32;
            let to = cpu.read16(target) as u32;
            target += 2;
            let from = cpu.read16(angles + 2 * i) as u32;
            let d = nested!(cpu, 0x0605_6580, super::aim::angle_between(cpu, from, to)) as i16 as i32 as u32;
            let turn = nested!(cpu, 0x0600_6a78, super::math::quotient(cpu, d, frames));
            cpu.write16(turns + 2 * i, turn as u16);
        }
        let held = word(cpu, 2);
        let left = cpu.read16(p + P_LEFT).wrapping_add(held);
        cpu.write16(p + P_LEFT, left);
    }
    let left = cpu.read16(p + P_LEFT).wrapping_sub(1);
    cpu.write16(p + P_LEFT, left);
}

/// A spinner's draw (`s` its sprite): its model (sprite kind 32's first, through its table) at
/// its place, turned by its angle about y and a quarter about x, under the screen's light;
/// SGL's 0x0606713c set to 0 around it when its byte +0x2d says.
const SPINNER_MODELS: u32 = 0x0607_43f0;
const SPINNER_OWN: u32 = 0x2d;
const SPINNER_TURN: u32 = 0x1e;
const SL_0606713C: u32 = 0x0606_713c;
const SCREEN_LIGHT: u32 = 0x70;

pub fn spinner_draw(cpu: &mut Cpu, s: u32) {
    let own = cpu.read8(s + SPINNER_OWN) != 0;
    if own {
        cpu.call(SL_0606713C, &[0]);
    }
    cpu.call(SL_PUSH_MATRIX, &[]);
    let screen = cpu.read32(super::screen::SCREEN);
    cpu.call(SL_LOAD_MATRIX, &[screen + SCREEN_LIGHT]);
    let p = [1, 2, 3].map(|i| cpu.read32(s + 4 * i));
    cpu.call(SL_TRANSLATE, &p);
    let a = cpu.read16(s + SPINNER_TURN) as i16 as i32 as u32;
    cpu.call(SL_ROT_Y, &[a]);
    cpu.call(SL_ROT_X, &[0x4000]);
    let table = cpu.read32(SPINNER_MODELS);
    let at = cpu.read32(table + 0x1c);
    let at = cpu.read32(at);
    let model = cpu.read32(at);
    cpu.call(SL_PUT_POLYGON, &[model]);
    cpu.call(SL_POP_MATRIX, &[]);
    if cpu.read8(s + SPINNER_OWN) != 0 {
        cpu.call(SL_0606713C, &[1]);
    }
}

/// A spinner's turn a frame (at +0x2e, a word): 0x100 and up to 0x200 more back, by a sine whose
/// phase (+0x44) runs 0x80 a frame.
const SPIN_PHASE: u32 = 0x44;
const SPIN_ANGLE: u32 = 0x2e;
const SPIN_SWING: u32 = 0x200_0000;
const SL_SIN_M: u32 = 0x0606_9a0c;
const SL_MUL_FX_M: u32 = 0x0606_e26c;

pub fn spinner_turn(cpu: &mut Cpu, o: u32) {
    let phase = cpu.read16(o + SPIN_PHASE).wrapping_add(0x80);
    cpu.write16(o + SPIN_PHASE, phase);
    let s = cpu.call(SL_SIN_M, &[phase as i16 as i32 as u32]);
    let m = (cpu.call(SL_MUL_FX_M, &[SPIN_SWING, s]) >> 16) as i16;
    let a = cpu.read16(o + SPIN_ANGLE).wrapping_add(0xff00).wrapping_sub(m.unsigned_abs());
    cpu.write16(o + SPIN_ANGLE, a);
}

/// Sprite kind `kind`'s models (sprites.rs's kinds: each kind's table, its +0x1c the models: for
/// each frame a pointer to its plain model and its flashing one; SGL's polygon data: +0xc the
/// polygons' count, +0x10 their attributes, 12 bytes each).
const MODEL_KINDS: u32 = 0x0607_41f0;
const KIND_MODELS: u32 = 0x1c;
const PD_COUNT: u32 = 0xc;
const PD_ATTRS: u32 = 0x10;
const ATTR_SIZE: u32 = 12;
const ATTR_BITS: u32 = 4;
const ATTR_COLOUR: u32 = 6;

fn kind_model(cpu: &mut Cpu, kind: u32, frame: u32) -> u32 {
    let table = cpu.read32(MODEL_KINDS.wrapping_add(kind << 4));
    let models = cpu.read32(table + KIND_MODELS);
    cpu.read32(models.wrapping_add(frame << 2))
}

/// The plain model's polygons all given colour `colour`.
pub fn model_colour(cpu: &mut Cpu, kind: u32, frame: u32, colour: u32) {
    let at = kind_model(cpu, kind, frame);
    let pd = cpu.read32(at);
    let mut k = 0;
    while k < cpu.read32(pd + PD_COUNT) {
        let attrs = cpu.read32(pd + PD_ATTRS);
        cpu.write16(attrs + ATTR_SIZE * k + ATTR_COLOUR, colour as u16);
        k += 1;
    }
}

/// The plain model's polygons all lit (SGL's attribute sort byte's UseLight).
const ATTR_SORT: u32 = 1;
const USE_LIGHT: u8 = 8;

pub fn model_lit(cpu: &mut Cpu, kind: u32, frame: u32) {
    let at = kind_model(cpu, kind, frame);
    let pd = cpu.read32(at);
    let mut k = 0;
    while (k as i32) < cpu.read32(pd + PD_COUNT) as i32 {
        let a = cpu.read32(pd + PD_ATTRS) + ATTR_SIZE * k + ATTR_SORT;
        let v = cpu.read8(a) | USE_LIGHT;
        cpu.write8(a, v);
        k += 1;
    }
}

/// Both models' polygons' attribute bits xored with `bits`.
pub fn model_flip_bits(cpu: &mut Cpu, kind: u32, frame: u32, bits: u32) {
    let at = kind_model(cpu, kind, frame);
    let (plain, flashing) = (cpu.read32(at), cpu.read32(at + 4));
    let mut k = 0;
    while k < cpu.read32(plain + PD_COUNT) {
        for pd in [plain, flashing] {
            let a = cpu.read32(pd + PD_ATTRS) + ATTR_SIZE * k + ATTR_BITS;
            let v = cpu.read16(a) ^ bits as u16;
            cpu.write16(a, v);
        }
        k += 1;
    }
}

/// The pods (made by the stages' code): an actor drawn by its own draw, by its byte +0x3d: 1 a
/// model of seven parts (a body, four legs and the four struts that share one part; XPDATA
/// structs in the actor, its light at +0x78), 3 a sprite of kind 2 (its frame by its turn about
/// y, 32 a turn), each turned by the actor's turns and with a glow under it.
const POD_DRAW_KIND: u32 = 0x2d;
const POD_X: u32 = 0x14;
const POD_TURNS: u32 = 0x2c;
const POD_SPRITE_COLOUR: u32 = 0x3e;
const POD_GLOW: u32 = 0x54;
const POD_LIGHT: u32 = 0x78;
const POD_BODY: u32 = 0x84;
const POD_LEGS: [(u32, [i32; 3]); 4] = [(0x9c, [-3, 8, -3]), (0xb4, [-3, 8, 3]), (0xcc, [3, 8, 3]), (0xe4, [3, 8, -3])];
const POD_STRUT: u32 = 0xfc;
const POD_STRUTS: [([i32; 3], i16, i16); 4] = [
    ([-3, 1, -3], 0x0e38, 0xe0b7u16 as i16),
    ([-3, 1, 3], 0xf1c8u16 as i16, 0xe0b7u16 as i16),
    ([3, 1, 3], 0xf1c8u16 as i16, 0x1f49),
    ([3, 1, -3], 0x0e38, 0x1f49),
];
const POD_DROP: u32 = 0xfffd_0000;
const SL_PUT_SPRITE: u32 = 0x0607_0e22;
const SL_PUT_POLYGON_X: u32 = 0x0607_10b0;

pub fn pod_draw(cpu: &mut Cpu, s: u32) {
    let o = s - 0x10;
    match cpu.read8(s + POD_DRAW_KIND) {
        1 => nested!(cpu, 0x0606_46c4, pod_model(cpu, o)),
        2 => {
            cpu.call(0x0606_4620, &[o]);
        }
        3 => nested!(cpu, 0x0606_48e4, pod_sprite(cpu, o)),
        _ => {}
    }
}

/// Under the screen's light at the actor's place, turned by its turns.
fn pod_place(cpu: &mut Cpu, o: u32) {
    let screen = cpu.read32(super::screen::SCREEN);
    cpu.call(SL_LOAD_MATRIX, &[screen + SCREEN_LIGHT]);
    let p = [0, 1, 2].map(|k| cpu.read32(o + POD_X + 4 * k));
    cpu.call(SL_TRANSLATE, &p);
    pod_turn(cpu, o);
}

fn pod_turn(cpu: &mut Cpu, o: u32) {
    for (k, rot) in [SL_ROT_X, SL_ROT_Y, SL_ROT_Z].into_iter().enumerate() {
        let a = cpu.read16(o + POD_TURNS + 2 * k as u32) as i16 as i32 as u32;
        cpu.call(rot, &[a]);
    }
}

fn units(p: [i32; 3]) -> [u32; 3] {
    p.map(|v| (v << 16) as u32)
}

pub fn pod_model(cpu: &mut Cpu, o: u32) {
    cpu.call(SL_PUSH_MATRIX, &[]);
    pod_place(cpu, o);
    cpu.call(SL_TRANSLATE, &[0, POD_DROP, 0]);
    let light = o + POD_LIGHT;
    cpu.call(SL_PUT_POLYGON_X, &[o + POD_BODY, light]);
    for (part, at) in POD_LEGS {
        cpu.call(SL_PUSH_MATRIX, &[]);
        cpu.call(SL_TRANSLATE, &units(at));
        cpu.call(SL_PUT_POLYGON_X, &[o + part, light]);
        cpu.call(SL_POP_MATRIX, &[]);
    }
    for (at, x, z) in POD_STRUTS {
        cpu.call(SL_PUSH_MATRIX, &[]);
        cpu.call(SL_TRANSLATE, &units(at));
        cpu.call(SL_ROT_X, &[x as i32 as u32]);
        cpu.call(SL_ROT_Z, &[z as i32 as u32]);
        cpu.call(SL_PUT_POLYGON_X, &[o + POD_STRUT, light]);
        cpu.call(SL_POP_MATRIX, &[]);
    }
    nested!(cpu, 0x0606_4520, pod_glow(cpu, o));
    cpu.call(SL_POP_MATRIX, &[]);
}

/// The sprite's frames by its turn about y (from a table of bytes, 32 a turn; the second half
/// flipped), kind 2's; its colour bank by its byte +0x3e.
const POD_FRAMES: u32 = 0x0607_3c44;
const KIND_2: u32 = MODEL_KINDS + 2 * 16;
const SPRITE_KIND_FRAMES: u32 = 4;
const SPRITE_KIND_CHAR: u32 = 8;
const SPRITE_KIND_COLOUR: u32 = 0xc;
const SA_TEXTURE: u32 = 0;
const SA_ATTRS: u32 = 2;
const SA_COLOUR: u32 = 4;
const SA_GOURAUD: u32 = 6;
const SA_DIR: u32 = 8;
const POD_SPRITE_ATTRS: u16 = 0x80;
const POD_SPRITE_DIR: u16 = 2;
const POD_SPRITE_FLIPPED: u16 = 0x12;
const POD_SPRITE_SCALE: u32 = 0xffff;

pub fn pod_sprite(cpu: &mut Cpu, o: u32) {
    let f = cpu.stack();
    for k in 0..3 {
        let v = cpu.read32(o + POD_X + 4 * k);
        cpu.write32(f + 4 * k, v);
    }
    cpu.write32(f + 12, POD_SPRITE_SCALE);
    let a = f + 16;
    let turn = cpu.read16(o + POD_TURNS + 2) as i16 as i32;
    let n = ((turn + 0x8400) as u32 & 0xffff) >> 11;
    let frame = cpu.read8(POD_FRAMES + n) as u32;
    let frames = cpu.read32(KIND_2 + SPRITE_KIND_FRAMES);
    let at = cpu.read32(frames + 8 * frame);
    let texture = cpu.read16(KIND_2 + SPRITE_KIND_CHAR).wrapping_add(cpu.read8(at) as u16);
    cpu.write16(a + SA_TEXTURE, texture);
    let bank = (cpu.read8(o + POD_SPRITE_COLOUR) as i8 as i32 * 16) as u16;
    let colour = cpu.read16(KIND_2 + SPRITE_KIND_COLOUR).wrapping_add(bank);
    cpu.write16(a + SA_COLOUR, colour);
    cpu.write16(a + SA_GOURAUD, 0);
    cpu.write16(a + SA_ATTRS, POD_SPRITE_ATTRS);
    cpu.write16(a + SA_DIR, if n > 15 { POD_SPRITE_FLIPPED } else { POD_SPRITE_DIR });
    cpu.call(SL_PUSH_MATRIX, &[]);
    let screen = cpu.read32(super::screen::SCREEN);
    cpu.call(SL_LOAD_MATRIX, &[screen + SCREEN_LIGHT]);
    let z = 0x8000u16.wrapping_sub(cpu.read16(o + POD_TURNS + 4)) as i16 as i32 as u32;
    cpu.call(SL_PUT_SPRITE, &[f, a, z]);
    let p = [0, 1, 2].map(|k| cpu.read32(o + POD_X + 4 * k));
    cpu.call(SL_TRANSLATE, &p);
    pod_turn(cpu, o);
    nested!(cpu, 0x0606_4520, pod_glow(cpu, o));
    cpu.call(SL_POP_MATRIX, &[]);
}

/// The glow under a pod (unless its size, +0x54, is none): a sprite of kind 0 flickering a little
/// in size; on even loops frame 213 in colours 0x30 on, 12 below; on odd ones frame 1 in colours
/// 0x70 on (meshed) and 1.5 times as large, 16 below.
const GLOW_EVEN: (u32, u16, u16, u32) = (0x6a8, 0x30, 0, 0xfff4_0000);
const GLOW_ODD: (u32, u16, u16, u32) = (8, 0x70, 0x100, 0xfff0_0000);
const GLOW_FLICKER: u32 = 0x800;
const GLOW_ODD_SCALE: u32 = 0x1_8000;
const KIND_0: u32 = MODEL_KINDS;

pub fn pod_glow(cpu: &mut Cpu, o: u32) {
    let size = cpu.read32(o + POD_GLOW);
    if size == 0 {
        return;
    }
    let f = cpu.stack();
    cpu.write32(f, 0);
    cpu.write32(f + 8, 0);
    let r = nested!(cpu, 0x0604_5d90, super::math::random(cpu, 0)) & 0xffff;
    let r = nested!(cpu, 0x0606_e26c, super::math::sl_mul_fx(cpu, r, GLOW_FLICKER));
    let scale = cpu.read32(o + POD_GLOW).wrapping_add(r);
    cpu.write32(f + 12, scale);
    let odd = cpu.read32(super::globals::LOOPS) & 1 != 0;
    let (frame, colours, attrs, y) = if odd { GLOW_ODD } else { GLOW_EVEN };
    let a = f + 16;
    let colour = cpu.read16(KIND_0 + SPRITE_KIND_COLOUR).wrapping_add(colours);
    cpu.write16(a + SA_COLOUR, colour);
    cpu.write16(a + SA_ATTRS, attrs);
    let frames = cpu.read32(KIND_0 + SPRITE_KIND_FRAMES);
    let at = cpu.read32(frames + frame);
    let texture = cpu.read16(KIND_0 + SPRITE_KIND_CHAR).wrapping_add(cpu.read8(at) as u16);
    cpu.write16(a + SA_TEXTURE, texture);
    cpu.write32(f + 4, y);
    if odd {
        let s = nested!(cpu, 0x0606_e26c, super::math::sl_mul_fx(cpu, GLOW_ODD_SCALE, scale));
        cpu.write32(f + 12, s);
    }
    cpu.write16(a + SA_GOURAUD, 0);
    cpu.write16(a + SA_DIR, POD_SPRITE_DIR);
    cpu.call(SL_PUT_SPRITE, &[f, a, 0]);
}

/// Kind 31's model (an actor's own draw, at its sprite `s`): its body placed and turned as the
/// sprite, two side parts, two parts turned about y by the actor's word +0x3e (the second the
/// other way), and four glows under its corners (the pods' glow, its size flickering by SGL's
/// random number).
const KIND_31_MODELS: u32 = MODEL_KINDS + 31 * 16;
const M31_TURN: u32 = 0x2e;
const M31_SIDES: [(u32, [u32; 3]); 2] = [(3, [0x4f_0000, 0xffcf_0000, 0x25_0000]), (4, [0xffb1_0000, 0xffcf_0000, 0x25_0000])];
const M31_TURNED: [(u32, u32, bool); 2] = [(1, 0xa_0000, false), (2, 0xfff6_0000, true)];
const M31_TURNED_Z: u32 = 0x9_0000;
const M31_GLOW_EVEN: (u32, u16, u16, u32, u32) = (0x6a8, 0x30, 0, 0x5000, 0xff90_0000);
const M31_GLOW_ODD: (u32, u16, u16, u32, u32) = (8, 0x70, 0x100, 0x7000, 0xff86_0000);
const M31_GLOW_STEP: u32 = 0x300;
const M31_GLOWS: [(u32, u32); 4] = [(0x10_0000, 0xfff0_0000), (0x10_0000, 0x10_0000), (0xfff0_0000, 0xfff0_0000), (0xfff0_0000, 0x10_0000)];
const SL_RANDOM: u32 = 0x0603_4dd4;

fn model_31(cpu: &mut Cpu, part: u32) -> u32 {
    let table = cpu.read32(KIND_31_MODELS);
    let models = cpu.read32(table + KIND_MODELS);
    let at = cpu.read32(models + 4 * part);
    cpu.read32(at)
}

pub fn model31_draw(cpu: &mut Cpu, s: u32) {
    cpu.call(SL_PUSH_MATRIX, &[]);
    let screen = cpu.read32(super::screen::SCREEN);
    cpu.call(SL_LOAD_MATRIX, &[screen + SCREEN_LIGHT]);
    let p = [1, 2, 3].map(|i| cpu.read32(s + 4 * i));
    cpu.call(SL_TRANSLATE, &p);
    for (at, rot) in [(S_TURN_X, SL_ROT_X), (S_TURN_Y, SL_ROT_Y), (S_TURN_Z, SL_ROT_Z)] {
        let a = cpu.read16(s + at) as i16 as i32 as u32;
        cpu.call(rot, &[a]);
    }
    let body = model_31(cpu, 0);
    cpu.call(SL_PUT_POLYGON, &[body]);
    for (part, at) in M31_SIDES {
        cpu.call(SL_PUSH_MATRIX, &[]);
        cpu.call(SL_TRANSLATE, &at);
        let m = model_31(cpu, part);
        cpu.call(SL_PUT_POLYGON, &[m]);
        cpu.call(SL_POP_MATRIX, &[]);
    }
    for (part, x, other_way) in M31_TURNED {
        cpu.call(SL_PUSH_MATRIX, &[]);
        cpu.call(SL_TRANSLATE, &[x, 0, M31_TURNED_Z]);
        let a = cpu.read16(s + M31_TURN) as i16;
        let a = if other_way { a.wrapping_neg() } else { a };
        cpu.call(SL_ROT_Y, &[a as i32 as u32]);
        let m = model_31(cpu, part);
        cpu.call(SL_PUT_POLYGON, &[m]);
        cpu.call(SL_POP_MATRIX, &[]);
    }
    cpu.call(SL_PUSH_MATRIX, &[]);
    let f = cpu.stack();
    let a = f + 16;
    let odd = cpu.read32(super::globals::LOOPS) & 1 != 0;
    let (frame, colours, attrs, size, y) = if odd { M31_GLOW_ODD } else { M31_GLOW_EVEN };
    let colour = cpu.read16(KIND_0 + SPRITE_KIND_COLOUR).wrapping_add(colours);
    cpu.write16(a + SA_COLOUR, colour);
    cpu.write16(a + SA_ATTRS, attrs);
    let frames = cpu.read32(KIND_0 + SPRITE_KIND_FRAMES);
    let at = cpu.read32(frames + frame);
    let texture = cpu.read16(KIND_0 + SPRITE_KIND_CHAR).wrapping_add(cpu.read8(at) as u16);
    cpu.write16(a + SA_TEXTURE, texture);
    cpu.write32(f + 4, y);
    let r = cpu.call(SL_RANDOM, &[]) & 0xf;
    cpu.write32(f + 12, size.wrapping_add(r * M31_GLOW_STEP));
    cpu.write16(a + SA_GOURAUD, 0);
    cpu.write16(a + SA_DIR, POD_SPRITE_DIR);
    for (x, z) in M31_GLOWS {
        cpu.write32(f, x);
        cpu.write32(f + 8, z);
        cpu.call(SL_PUT_SPRITE, &[f, a, 0]);
    }
    cpu.call(SL_POP_MATRIX, &[]);
    cpu.call(SL_POP_MATRIX, &[]);
}
