//! The rings: a flat model of 16 quads between two circles (32 points), each a frame growing by
//! its own rate and scaled, until the inner circle is wider than the outer. A ring may follow a
//! place (until what owns it ends), be drawn under a matrix of its own, take its own light (SGL's
//! 0x0606713c) and flicker (its polygons' mesh bit flipped each frame). The stage code makes them.

use super::globals::{GAME_FLAGS, HELD};
use super::{math, nested, objects, Cpu};

/// The ring's fields: its place (x, y, z), its scale (x, y), its turn (x, y, z: words), its own
/// flags (a byte: FLICKER, OWN_LIGHT), SGL's polygon data (the points, their count, the polygons,
/// their count, the attributes); the circles' radii, their rates and the scale's rates; the
/// points (32, 12 bytes each: an inner and an outer a quad), the polygons' attributes (16, 12
/// bytes each), what it follows (an object: its mode), the place it follows and its matrix.
const OBJ_MODE: u32 = 3;
const ENDING: u8 = 0x70;
const ENDED: u8 = 0x73;
const POSITION: u32 = 0x10;
const SCALE: u32 = 0x1c;
const TURN: u32 = 0x24;
const R_FLAGS: u32 = 0x2a;
const POLYGON_DATA: u32 = 0x2c;
const PD_POINT_COUNT: u32 = 0x30;
const PD_POLYGONS: u32 = 0x34;
const PD_POLYGON_COUNT: u32 = 0x38;
const PD_ATTRS: u32 = 0x3c;
const INNER: u32 = 0x40;
const OUTER: u32 = 0x44;
const INNER_RATE: u32 = 0x48;
const OUTER_RATE: u32 = 0x4c;
const SCALE_RATE: u32 = 0x50;
const POINTS: u32 = 0x58;
const ATTRS: u32 = 0x1d8;
const OWNER: u32 = 0x298;
const FOLLOWED: u32 = 0x29c;
const OWN_MATRIX: u32 = 0x2a0;
const RING_SIZE: u32 = 0x2a4;

const FLICKER: u8 = 1;
const OWN_LIGHT: u8 = 2;

const QUADS: u32 = 16;
const POINT_SIZE: u32 = 12;
const ATTR_SIZE: u32 = 12;
const ATTR_BITS: u32 = 4;
const MESH: u16 = 0x100;
/// The last mode of the owner's still followed.
const OWNER_LIVES: i8 = 0x6f;

const SL_0606713C: u32 = 0x0606_713c;
const SL_PUSH_MATRIX: u32 = 0x0606_71cc;
const SL_POP_MATRIX: u32 = 0x0606_7200;
const SL_LOAD_MATRIX: u32 = 0x0606_db7c;
const SL_TRANSLATE: u32 = 0x0606_721c;
const SL_ROT_X: u32 = 0x0606_72ec;
const SL_ROT_Y: u32 = 0x0606_736c;
const SL_ROT_Z: u32 = 0x0606_726c;
const SL_SCALE: u32 = 0x0606_73ec;
const SL_PUT_POLYGON: u32 = 0x0606_745c;

pub fn ring_draw(cpu: &mut Cpu, o: u32) {
    if cpu.read8(o + R_FLAGS) & OWN_LIGHT != 0 {
        cpu.call(SL_0606713C, &[0]);
    }
    cpu.call(SL_PUSH_MATRIX, &[]);
    let m = cpu.read32(o + OWN_MATRIX);
    if m != 0 {
        cpu.call(SL_LOAD_MATRIX, &[m]);
    }
    let p = [0, 1, 2].map(|i| cpu.read32(o + POSITION + 4 * i));
    cpu.call(SL_TRANSLATE, &p);
    for (k, rot) in [SL_ROT_X, SL_ROT_Y, SL_ROT_Z].into_iter().enumerate() {
        let a = cpu.read16(o + TURN + 2 * k as u32) as i16 as i32 as u32;
        cpu.call(rot, &[a]);
    }
    let (sx, sy) = (cpu.read32(o + SCALE), cpu.read32(o + SCALE + 4));
    cpu.call(SL_SCALE, &[sx, sy, 0x1_0000]);
    cpu.call(SL_PUT_POLYGON, &[o + POLYGON_DATA]);
    cpu.call(SL_POP_MATRIX, &[]);
    if cpu.read8(o + R_FLAGS) & OWN_LIGHT != 0 {
        cpu.call(SL_0606713C, &[1]);
    }
}

/// Places the points on the two circles: x by the sine, y by the cosine, a quad every 1/16 turn.
pub fn ring_points(cpu: &mut Cpu, o: u32) {
    let (inner, outer) = (cpu.read32(o + INNER), cpu.read32(o + OUTER));
    let mut a: i16 = 0;
    for k in 0..QUADS {
        let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a as i32 as u32));
        let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a as i32 as u32));
        let at = o + POINTS + 2 * POINT_SIZE * k;
        for (r, at) in [(inner, at), (outer, at + POINT_SIZE)] {
            let x = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, r, s));
            cpu.write32(at, x);
            let y = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, r, c));
            cpu.write32(at + 4, y);
        }
        a = a.wrapping_add(0x1000);
    }
}

/// A frame's growth: ends once the inner circle is wider than the outer.
pub fn ring_step(cpu: &mut Cpu, o: u32) {
    let inner = cpu.read32(o + INNER).wrapping_add(cpu.read32(o + INNER_RATE));
    cpu.write32(o + INNER, inner);
    let outer = cpu.read32(o + OUTER).wrapping_add(cpu.read32(o + OUTER_RATE));
    cpu.write32(o + OUTER, outer);
    if inner as i32 > outer as i32 {
        cpu.write8(o + OBJ_MODE, ENDING);
        return;
    }
    nested!(cpu, 0x0606_254c, ring_points(cpu, o));
    for k in 0..2 {
        let v = cpu.read32(o + SCALE + 4 * k).wrapping_add(cpu.read32(o + SCALE_RATE + 4 * k));
        cpu.write32(o + SCALE + 4 * k, v);
    }
    let owner = cpu.read32(o + OWNER);
    if owner != 0 {
        if cpu.read8(owner + OBJ_MODE) as i8 > OWNER_LIVES {
            cpu.write32(o + OWNER, 0);
        } else {
            let at = cpu.read32(o + FOLLOWED);
            for k in 0..3 {
                let v = cpu.read32(at + 4 * k);
                cpu.write32(o + POSITION + 4 * k, v);
            }
        }
    }
    if cpu.read8(o + R_FLAGS) & FLICKER != 0 {
        for k in 0..QUADS {
            let a = cpu.read32(o + PD_ATTRS) + ATTR_SIZE * k + ATTR_BITS;
            let v = cpu.read16(a) ^ MESH;
            cpu.write16(a, v);
        }
    }
}

pub fn ring_exec(cpu: &mut Cpu, o: u32) {
    if cpu.read32(GAME_FLAGS) & HELD != 0 {
        nested!(cpu, 0x0606_248c, ring_draw(cpu, o));
        return;
    }
    match cpu.read8(o + OBJ_MODE) {
        0 | 1 => {
            cpu.write8(o + OBJ_MODE, 1);
            nested!(cpu, 0x0606_25fc, ring_step(cpu, o));
            nested!(cpu, 0x0606_248c, ring_draw(cpu, o));
        }
        ENDING => cpu.write8(o + OBJ_MODE, ENDED),
        ENDED => nested!(cpu, 0x0600_579c, objects::obj_free(cpu, o)),
        _ => {}
    }
}

/// Makes a ring from description `d`: its own flags (a byte), the polygons' colour (a word), its
/// turn (x, y, z: words), place, scale, the scale's rates, the radii (inner, outer), their rates,
/// what it follows (its owner and the place) and its matrix. It, or 0.
const RING_LIST: u32 = 3;
const RING_EXEC: u32 = 0x0606_26bc;
/// The polygons (16 quads) and the attribute each starts from.
const RING_POLYGONS: u32 = 0x0607_39e8;
const RING_ATTR: u32 = 0x0607_3b28;
/// The polygons' texture: kind 0's first character and its frame 133's (sprites.rs's kinds).
const KINDS: u32 = 0x0607_41f0;
const KIND_FRAMES: u32 = 4;
const KIND_CHAR: u32 = 8;
const TEXTURE_FRAME: u32 = 133;
const ATTR_TEXTURE: u32 = 2;
const ATTR_COLOUR: u32 = 6;

pub fn ring_set(cpu: &mut Cpu, d: u32) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, RING_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, RING_LIST, blocks, RING_EXEC));
    if o == 0 {
        return 0;
    }
    for k in 0..3 {
        let v = cpu.read32(d + 12 + 4 * k);
        cpu.write32(o + POSITION + 4 * k, v);
        let t = cpu.read16(d + 4 + 2 * k);
        cpu.write16(o + TURN + 2 * k, t);
    }
    let copies = [
        (SCALE, 24),
        (SCALE + 4, 28),
        (SCALE_RATE, 32),
        (SCALE_RATE + 4, 36),
        (INNER, 40),
        (OUTER, 44),
        (INNER_RATE, 48),
        (OUTER_RATE, 52),
        (OWNER, 56),
        (FOLLOWED, 60),
        (OWN_MATRIX, 64),
    ];
    for (to, from) in copies {
        let v = cpu.read32(d + from);
        cpu.write32(o + to, v);
    }
    let flags = cpu.read8(d);
    cpu.write8(o + R_FLAGS, flags);
    cpu.write32(o + POLYGON_DATA, o + POINTS);
    cpu.write32(o + PD_POINT_COUNT, 2 * QUADS);
    cpu.write32(o + PD_POLYGONS, RING_POLYGONS);
    cpu.write32(o + PD_POLYGON_COUNT, QUADS);
    cpu.write32(o + PD_ATTRS, o + ATTRS);
    for k in 0..QUADS {
        let at = o + ATTRS + ATTR_SIZE * k;
        for w in (0..ATTR_SIZE).step_by(2) {
            let v = cpu.read16(RING_ATTR + w);
            cpu.write16(at + w, v);
        }
        let frames = cpu.read32(KINDS + KIND_FRAMES);
        let frame = cpu.read32(frames + 8 * TEXTURE_FRAME);
        let texture = cpu.read16(KINDS + KIND_CHAR).wrapping_add(cpu.read8(frame) as u16);
        cpu.write16(at + ATTR_TEXTURE, texture);
        let colour = cpu.read16(d + 2);
        cpu.write16(at + ATTR_COLOUR, colour);
    }
    for k in 0..2 * QUADS {
        cpu.write32(o + POINTS + POINT_SIZE * k + 8, 0);
    }
    nested!(cpu, 0x0606_254c, ring_points(cpu, o));
    o
}
