//! The hits: each frame the objects register boxes in the hit manager's seven lists, and the
//! manager (an object) checks the lists against each other once a frame. Each hit is handed to a
//! function chosen by the two boxes' kinds, which does what the hit does (damage, a pickup, the
//! weapons' experience ...). Which list holds what is not known yet beyond what the checks tell:
//! lists 0, 2 and 5 are checked against the others, list 3 against 1, 4, 5 and 6; list 6 holds
//! points.

use super::globals::{GAME_FLAGS, HELD};
use super::{objects, system, Cpu};

/// The hit manager (0 when there is none).
const HITS: u32 = 0x0607_46e8;
/// Its lists: each list's count (words from +0x1e), last frame's counts (kept while the game is not
/// held, from +0x2c), and its boxes.
const COUNTS: u32 = 0x1e;
const LAST_COUNTS: u32 = 0x2c;
const LISTS: [u32; 7] = [0x3c, 0x7c, 0x43c, 0x53c, 0x93c, 0x113c, 0x123c];
/// A frame count the manager counts down (not known yet).
const COUNTDOWN: u32 = 0x1a3c;

/// A box (16 bytes): its kind, its attribute, its centre (x, y), its half width and height, its
/// object.
const BOX: u32 = 16;
const KIND: u32 = 0;
const ATTR: u32 = 1;
const X: u32 = 2;
const Y: u32 = 4;
const HALF_W: u32 = 6;
const HALF_H: u32 = 8;
const OBJ: u32 = 0xc;

/// An object's fields a hit sets: the first box's attribute and the second's (both objects get
/// both).
const HIT_FIRST: u32 = 0x5a;
const HIT_SECOND: u32 = 0x5b;
/// An object's number among those a hit set covers (below 32 in the set's first long, else its
/// second), and an object's set of objects it may hit (two longs).
const NUMBER: u32 = 0x32;
const HIT_SET: u32 = 0x48;

/// The hit functions: by the first box's kind and the second's, two kinds to a row
/// (`f(first, second)`), or five (`f(first, second, first box, second box)`).
const HIT_PAIR: u32 = 0x0607_2428;
const HIT_FIVE: u32 = 0x0607_2414;
/// A point (list 6) hit by a box of list 3 takes HIT_FIVE's column 2.
const POINT_COLUMN: u32 = 2;

/// The objects' mode byte: set to end.
const OBJ_MODE: u32 = 3;
const ENDING: u8 = 0x70;
const ENDED: u8 = 0x73;

/// Whether `obj` is in `of`'s hit set (the set's bit, nonzero, or 0).
pub fn in_hit_set(cpu: &mut Cpu, of: u32, obj: u32) -> u32 {
    let n = cpu.read8(obj + NUMBER) as i8 as i32;
    let (bit, word) = if n > 31 { (system::shift_left(cpu, 1, (n - 32) as u32), of + HIT_SET + 4) } else { (system::shift_left(cpu, 1, n as u32), of + HIT_SET) };
    cpu.read32(word) & bit
}

/// A box as the pass keeps it (the outer loop's box is read once).
struct HitBox {
    at: u32,
    kind: i32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    obj: u32,
}

fn read_box(cpu: &mut Cpu, at: u32) -> HitBox {
    HitBox {
        at,
        kind: cpu.read8(at + KIND) as i8 as i32,
        x: cpu.read16(at + X) as i16 as i32,
        y: cpu.read16(at + Y) as i16 as i32,
        w: cpu.read16(at + HALF_W) as i16 as i32,
        h: cpu.read16(at + HALF_H) as i16 as i32,
        obj: cpu.read32(at + OBJ),
    }
}

fn count(cpu: &mut Cpu, hits: u32, list: usize) -> i32 {
    cpu.read16(hits + COUNTS + 2 * list as u32) as i16 as i32
}

fn box_at(hits: u32, list: usize, i: i32) -> u32 {
    hits + LISTS[list] + BOX * i as u32
}

/// What a check of a box against a list asks before the boxes are compared.
#[derive(Clone, Copy, PartialEq)]
enum Gate {
    /// Nothing.
    None,
    /// The inner box's kind above 0, or the second object in the first's hit set.
    KindOrSet,
    /// The second object in the first's hit set.
    Set,
    /// The inner box has an object (a point).
    Object,
}

/// How a hit is handed on.
#[derive(Clone, Copy, PartialEq)]
enum Call {
    /// HIT_PAIR by the two kinds.
    Pair,
    /// HIT_FIVE by the two kinds, the boxes passed too.
    Five,
    /// HIT_FIVE's POINT_COLUMN, the boxes passed too; a point whose object ends is taken out.
    Point,
}

/// Box `a` (from an outer loop) against each box of `list`: the first of the two is `a` if
/// `a_first`, else the inner box. Overlapping boxes (or a point in `a`) hit.
fn check(cpu: &mut Cpu, hits: u32, a: &HitBox, list: usize, a_first: bool, gate: Gate, call: Call) {
    let mut i = 0;
    while i < count(cpu, hits, list) {
        let at = box_at(hits, list, i);
        i += 1;
        let obj = cpu.read32(at + OBJ);
        let (first_obj, second_obj) = if a_first { (a.obj, obj) } else { (obj, a.obj) };
        let pass = match gate {
            Gate::None => true,
            Gate::KindOrSet => cpu.read8(at + KIND) as i8 > 0 || in_hit_set(cpu, first_obj, second_obj) != 0,
            Gate::Set => in_hit_set(cpu, first_obj, second_obj) != 0,
            Gate::Object => obj != 0,
        };
        if !pass {
            continue;
        }
        let point = gate == Gate::Object;
        let (w, h) = if point {
            (a.w, a.h)
        } else {
            (cpu.read16(at + HALF_W) as i16 as i32 + a.w, cpu.read16(at + HALF_H) as i16 as i32 + a.h)
        };
        let x = cpu.read16(at + X) as i16 as i32;
        if (x - a.x).abs() > w {
            continue;
        }
        let y = cpu.read16(at + Y) as i16 as i32;
        if (y - a.y).abs() > h {
            continue;
        }
        let (first_at, second_at) = if a_first { (a.at, at) } else { (at, a.at) };
        let attr = cpu.read8(first_at + ATTR);
        cpu.write8(second_obj + HIT_FIRST, attr);
        cpu.write8(first_obj + HIT_FIRST, attr);
        let attr = cpu.read8(second_at + ATTR);
        cpu.write8(second_obj + HIT_SECOND, attr);
        cpu.write8(first_obj + HIT_SECOND, attr);
        let first_kind = if a_first { a.kind } else { cpu.read8(at + KIND) as i8 as i32 };
        let second_kind = if a_first { cpu.read8(at + KIND) as i8 as i32 } else { a.kind };
        match call {
            Call::Pair => {
                let f = cpu.read32(HIT_PAIR.wrapping_add((4 * (2 * first_kind + second_kind)) as u32));
                cpu.call(f, &[first_obj, second_obj]);
            }
            Call::Five | Call::Point => {
                let column = if call == Call::Point { POINT_COLUMN as i32 } else { second_kind };
                let f = cpu.read32(HIT_FIVE.wrapping_add((4 * (5 * first_kind + column)) as u32));
                cpu.call(f, &[first_obj, second_obj, first_at, second_at]);
                if call == Call::Point && cpu.read8(obj + OBJ_MODE) == ENDING {
                    cpu.write32(at + OBJ, 0);
                }
            }
        }
    }
}

/// Each of `list`'s boxes against the lists in `against`.
fn pass(cpu: &mut Cpu, hits: u32, list: usize, against: &[(usize, bool, Gate, Call, i32)]) {
    let mut i = 0;
    while i < count(cpu, hits, list) {
        let a = read_box(cpu, box_at(hits, list, i));
        for &(other, a_first, gate, call, above) in against {
            if a.kind > above {
                check(cpu, hits, &a, other, a_first, gate, call);
            }
        }
        i += 1;
    }
}

/// The hit manager's frame (its exec): the lists checked, last frame's counts kept (and drawn in the
/// debug mode), the lists emptied for the next frame.
pub fn hits_exec(cpu: &mut Cpu, hits: u32) {
    if cpu.read8(hits + OBJ_MODE) == ENDING {
        cpu.write8(hits + OBJ_MODE, ENDED);
        cpu.write32(HITS, 0);
        objects::obj_free(cpu, hits);
        return;
    }
    const ANY: i32 = i32::MIN;
    pass(cpu, hits, 3, &[
        (1, true, Gate::KindOrSet, Call::Pair, ANY),
        (6, true, Gate::Object, Call::Point, 2),
        (4, true, Gate::None, Call::Five, 3),
        (5, true, Gate::None, Call::Five, 3),
    ]);
    pass(cpu, hits, 0, &[(4, false, Gate::Set, Call::Pair, ANY), (6, false, Gate::Object, Call::Pair, ANY)]);
    pass(cpu, hits, 2, &[
        (3, false, Gate::Set, Call::Pair, ANY),
        (4, false, Gate::Set, Call::Pair, ANY),
        (6, false, Gate::Object, Call::Pair, ANY),
    ]);
    pass(cpu, hits, 5, &[(0, true, Gate::KindOrSet, Call::Pair, ANY), (1, true, Gate::KindOrSet, Call::Pair, ANY)]);
    if cpu.read32(GAME_FLAGS) & HELD == 0 {
        for k in 0..7 {
            let n = cpu.read16(hits + COUNTS + 2 * k);
            cpu.write16(hits + LAST_COUNTS + 2 * k, n);
        }
    }
    if cpu.read32(DEBUG_FLAGS) & SHOW_HITS != 0 {
        draw_hits(cpu, hits);
    }
    for k in (0..7).rev() {
        cpu.write16(hits + COUNTS + 2 * k, 0);
    }
    let n = cpu.read32(hits + COUNTDOWN);
    if n as i32 > 0 {
        cpu.write32(hits + COUNTDOWN, n - 1);
    }
}

/// The debug flags (director.rs), and the bit that has the boxes drawn.
const DEBUG_FLAGS: u32 = 0x0600_ed8c;
const SHOW_HITS: u32 = 2;
/// The sprite drawn for a box: its struct (in the frame, from +0x10; set up by SPRITE_INIT), its
/// corners (four x, y pairs from the frame's bottom), and the sprite draw.
const SPRITE: u32 = 0x10;
const SPRITE_FLAGS: u32 = 0;
const SPRITE_X: u32 = 4;
const SPRITE_Y: u32 = 8;
const SPRITE_Z: u32 = 0xc;
const SPRITE_CHAR: u32 = 0x18;
const SPRITE_PRIORITY: u32 = 0x23;
const SPRITE_CORNERS: u32 = 0x24;
const SPRITE_INIT: u32 = 0x0604_6264;
const SPRITE_DRAW: u32 = 0x0604_0960;
/// The sprites' first character (as the game keeps it), each list's colour (an offset from it, in
/// the order drawn), and the points' (by the VBlank count's low bit: they flicker).
const CHAR_BASE: u32 = 0x0607_41fc;
const DRAWN: [(usize, i16); 6] = [(0, 0x36), (3, 0x16), (1, 0x26), (4, 0x1d), (2, 0x2e), (5, 0x14)];
const VBLANK_LOW: u32 = 0x0600_ebb6;
const POINT_CHAR: u16 = 0x90;

/// The debug mode's boxes: each of last frame's boxes drawn as a sprite of its list's colour.
fn draw_hits(cpu: &mut Cpu, hits: u32) {
    let corners = cpu.stack();
    let sprite = corners + SPRITE;
    cpu.call(SPRITE_INIT, &[sprite]);
    let flags = cpu.read32(sprite + SPRITE_FLAGS);
    cpu.write32(sprite + SPRITE_FLAGS, flags | 2);
    cpu.write8(sprite + SPRITE_PRIORITY, 0x7f);
    for (list, colour) in DRAWN {
        let base = cpu.read16(CHAR_BASE) as i16;
        cpu.write16(sprite + SPRITE_CHAR, base.wrapping_add(colour) as u16);
        let mut i = 0;
        while i < cpu.read16(hits + LAST_COUNTS + 2 * list as u32) as i16 as i32 {
            let at = box_at(hits, list, i);
            let x = cpu.read16(at + X) as i16 as i32;
            cpu.write32(sprite + SPRITE_X, (x << 16) as u32);
            let y = cpu.read16(at + Y) as i16 as i32;
            cpu.write32(sprite + SPRITE_Y, (y << 16) as u32);
            cpu.write32(sprite + SPRITE_Z, 0);
            let w = cpu.read16(at + HALF_W) as i16;
            let h = cpu.read16(at + HALF_H) as i16;
            for (k, v) in [-w, -h, w, -h, w, h, -w, h].into_iter().enumerate() {
                cpu.write16(corners + 2 * k as u32, v as u16);
            }
            cpu.write32(sprite + SPRITE_CORNERS, corners);
            cpu.call(SPRITE_DRAW, &[sprite]);
            i += 1;
        }
    }
    let flags = cpu.read32(sprite + SPRITE_FLAGS);
    cpu.write32(sprite + SPRITE_FLAGS, flags | 3);
    let flicker = cpu.read16(VBLANK_LOW) & 1;
    let base = cpu.read16(CHAR_BASE);
    cpu.write16(sprite + SPRITE_CHAR, (flicker | POINT_CHAR).wrapping_add(base));
    let mut i = 0;
    while i < cpu.read16(hits + LAST_COUNTS + 2 * 6) as i16 as i32 {
        let at = box_at(hits, 6, i);
        let x = cpu.read16(at + X) as i16 as i32;
        cpu.write32(sprite + SPRITE_X, (x << 16) as u32);
        let y = cpu.read16(at + Y) as i16 as i32;
        cpu.write32(sprite + SPRITE_Y, (y << 16) as u32);
        cpu.write32(sprite + SPRITE_Z, 0);
        for k in (0..4).rev() {
            cpu.write16(corners + 2 * k, 0);
        }
        cpu.write32(sprite + SPRITE_CORNERS, corners);
        cpu.call(SPRITE_DRAW, &[sprite]);
        i += 1;
    }
}
