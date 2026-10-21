//! The hits: each frame the actors register their boxes in the hit manager's seven lists
//! (register_boxes: by the actor's side and whether the box is a body or a shot), and the manager
//! (an object) checks the lists against each other once a frame (hits_exec). Each hit is handed to
//! a function chosen by the two boxes' kinds, which does what the hit does: damage (hit, hit_once),
//! a touch, sparks off the sword, a bullet absorbed; and a kill by the players' side scores, with
//! the colour chains and the weapons' experience (weapon_hit, score_kill).
//!
//! The lists: 0 the players' bodies, 1 the enemies', 2 a third side's (not known yet); 3 the
//! players' shots, 4 the enemies', 5 the third side's, 6 the enemies' small bullets (as points).
//! Checked: the players' shots against the enemies, the bullets (kinds above 2) and the enemies' and
//! third side's shots (kinds above 3); the players against the enemies' shots and bullets; the
//! third side's bodies against all shots and bullets; the third side's shots against the players
//! and the enemies.

use super::nested;
use super::globals::{GAME_FLAGS, HELD};
use super::{math, objects, system, Cpu};

/// The rewrites' SH-2 entries, for the calls between them (mod.rs's nested!).
const IN_HIT_SET: u32 = 0x0604_3874;
const TOUCH: u32 = 0x0604_3984;
const DAMAGE: u32 = 0x0604_3088;
const WEAPON_HIT: u32 = 0x0604_34e4;
const SCORE_KILL: u32 = 0x0604_3108;
const CHAIN_RESET: u32 = 0x0604_5090;
const HITS_RESET: u32 = 0x0604_50d4;
pub const REGISTER_BOXES: u32 = 0x0604_51b4;

/// The hit manager (0 when there is none).
const HITS: u32 = 0x0607_46e8;
/// The numbered objects (63; their number at NUMBER): the players first.
const PLAYERS: u32 = 0x0607_40f0;
/// Its lists (above): each list's count (words from +0x1e), last frame's counts (kept while the
/// game is not held, from +0x2c), and its boxes.
const COUNTS: u32 = 0x1e;
const LAST_COUNTS: u32 = 0x2c;
const LISTS: [u32; 7] = [0x3c, 0x7c, 0x43c, 0x53c, 0x93c, 0x113c, 0x123c];
/// The frames the sparks still have (each spark adds its own, one goes each frame; no new sparks
/// above SPARKS_MOST).
const SPARKS: u32 = 0x1a3c;

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
    let (bit, word) = if n > 31 { (nested!(cpu, 0x0600_69d8, system::shift_left(cpu, 1, (n - 32) as u32)), of + HIT_SET + 4) } else { (nested!(cpu, 0x0600_69d8, system::shift_left(cpu, 1, n as u32)), of + HIT_SET) };
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
            Gate::KindOrSet => cpu.read8(at + KIND) as i8 > 0 || nested!(cpu, IN_HIT_SET, in_hit_set(cpu, first_obj, second_obj)) != 0,
            Gate::Set => nested!(cpu, IN_HIT_SET, in_hit_set(cpu, first_obj, second_obj)) != 0,
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
        nested!(cpu, 0x0600_579c, objects::obj_free(cpu, hits));
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
    let n = cpu.read32(hits + SPARKS);
    if n as i32 > 0 {
        cpu.write32(hits + SPARKS, n - 1);
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
    nested!(cpu, SPRITE_INIT, super::sprites::sprite_init(cpu, sprite));
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
            nested!(cpu, SPRITE_DRAW, super::sprites::draw(cpu, sprite));
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
        nested!(cpu, SPRITE_DRAW, super::sprites::draw(cpu, sprite));
        i += 1;
    }
}

/// The fields of the objects that take part in hits: their flags (FLAG_*), what they hit last (or
/// what hit them), their life, their side (0 the players'), their type (a shot's: its weapon),
/// their hit flags (HIT_*), the damage they do, and what they give the object they hit (a push, as
/// far as can be told: two longs that object gets at PUSHED).
const FLAGS: u32 = 0x10;
const HIT_OTHER: u32 = 0x40;
const LIFE: u32 = 0x50;
const SIDE: u32 = 0x54;
const TYPE: u32 = 0x55;
const HIT_FLAGS: u32 = 0x56;
const POWER: u32 = 0x58;
const PUSH: u32 = 0x5c;
const PUSHED: u32 = 0x64;
/// The flags: not to be hurt.
const UNHURT: u32 = 0x4_0000;
/// The hit flags: touched, was hit, hit something.
const TOUCHED: u16 = 0x100;
const WAS_HIT: u16 = 0x200;
const HIT_SOMETHING: u16 = 0x400;
/// A type that only some shots hit (TYPE_PIERCING's), not known yet.
const TYPE_SHIELDED: u8 = 6;
const TYPE_PIERCING: u8 = 13;
/// With both players in, damage is three quarters (16.16) and one.
const TWO_PLAYERS_DAMAGE: u32 = 0xc000;

/// The hit functions that do nothing.
pub fn no_hit(_cpu: &mut Cpu) {}

fn set_hit_flags(cpu: &mut Cpu, obj: u32, bits: u16) {
    let f = cpu.read16(obj + HIT_FLAGS);
    cpu.write16(obj + HIT_FLAGS, f | bits);
}

/// `a` hits `b`: each knows the other; if `a` does damage, `b` is hit (and pushed), and a hit by the
/// players' side counts for the weapons.
pub fn hit(cpu: &mut Cpu, a: u32, b: u32) {
    cpu.write32(a + HIT_OTHER, b);
    set_hit_flags(cpu, a, HIT_SOMETHING);
    if cpu.read16(a + POWER) == 0 {
        return;
    }
    cpu.write32(b + HIT_OTHER, a);
    set_hit_flags(cpu, b, WAS_HIT);
    let push = cpu.read32(a + PUSH);
    cpu.write32(b + PUSHED, push);
    let push = cpu.read32(a + PUSH + 4);
    cpu.write32(b + PUSHED + 4, push);
    if cpu.read8(a + SIDE) == 0 {
        nested!(cpu, WEAPON_HIT, weapon_hit(cpu, a, b));
    }
}

/// `a` hits `b` once as it passes through (its hit set's bit for `b` turned over); a shielded type
/// only by a piercing one.
pub fn hit_once(cpu: &mut Cpu, a: u32, b: u32) {
    if cpu.read8(b + TYPE) == TYPE_SHIELDED && cpu.read8(a + TYPE) != TYPE_PIERCING {
        return;
    }
    let n = cpu.read8(b + NUMBER) as i8 as i32;
    let (bit, word) = if n > 31 { (nested!(cpu, 0x0600_69d8, system::shift_left(cpu, 1, (n - 32) as u32)), a + HIT_SET + 4) } else { (nested!(cpu, 0x0600_69d8, system::shift_left(cpu, 1, n as u32)), a + HIT_SET) };
    let set = cpu.read32(word);
    cpu.write32(word, set ^ bit);
    // inline: hit's code is the SH-2 function's own here, not a call
    hit(cpu, a, b);
}

/// `a` and `b` touch: each knows the other.
pub fn touch(cpu: &mut Cpu, a: u32, b: u32) {
    cpu.write32(a + HIT_OTHER, b);
    set_hit_flags(cpu, a, TOUCHED);
    cpu.write32(b + HIT_OTHER, a);
    set_hit_flags(cpu, b, TOUCHED);
}

/// `obj` takes `power` damage (three quarters and one with both players in), its life down to no
/// less than 0 unless it had none: its life, or -0x80000000 if it is not to be hurt.
pub fn damage(cpu: &mut Cpu, obj: u32, power: u32) -> u32 {
    if cpu.read32(obj + FLAGS) & UNHURT != 0 {
        return 0x8000_0000;
    }
    let mut p = power as i16 as i32;
    if cpu.read32(PLAYERS) != 0 && cpu.read32(PLAYERS + 4) != 0 {
        p = (nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, p as u32, TWO_PLAYERS_DAMAGE)) as i32 + 1) as i16 as i32;
    }
    if p != 0 {
        let life = cpu.read32(obj + LIFE) as i32;
        if life > 0 && life <= p {
            cpu.write32(obj + LIFE, 0);
        } else {
            let life = cpu.read32(obj + LIFE) as i32;
            cpu.write32(obj + LIFE, life.wrapping_sub(p) as u32);
        }
    }
    cpu.read32(obj + LIFE)
}

/// The flags that keep a kill from scoring, and that keep a hit from counting for the weapons.
const NO_SCORE: u32 = 0xd0_0000;
const NO_EXPERIENCE: u32 = 0x20_0000;
/// The points an object is worth by its kind (+0x2e; a word every 16 bytes).
const OBJ_KIND: u32 = 0x2e;
const POINTS: u32 = 0x0607_41fe;
/// Each player's score (PLAY_DATA's), the weapons' experience (A, B, C: longs, 12 bytes a player),
/// the hits counted by weapon (seven longs, 28 bytes a player; the fifth's two words for A+C: a
/// timer set to 300 and a count).
const SCORE: u32 = 0x0600_ec20;
const EXPERIENCE_A: u32 = 0x0607_4070;
const WEAPON_HITS: u32 = 0x0607_40a0;
const AC_TIMER: u32 = 0x0607_40b0;
const AC_TIME: u16 = 300;

/// `points` scored for `target` by the player whose shot hit it last (as weapon_hit scores a kill:
/// the shot's number and weapon), unless that was of side 3.
const SIDE_3: u8 = 3;

pub fn score_hit_by(cpu: &mut Cpu, target: u32, points: u32) {
    let shot = cpu.read32(target + HIT_OTHER);
    if cpu.read8(shot + SIDE) == SIDE_3 {
        return;
    }
    let weapon = cpu.read8(shot + TYPE) as i8 as i32 as u32;
    let p = cpu.read8(shot + NUMBER) as i8 as i32 as u32;
    nested!(cpu, SCORE_KILL, score_kill(cpu, p, weapon, target, points));
}

/// A shot of the players' hits `target`: the damage done; a kill scores; then by the shot's weapon
/// (its type, 7-16: A, B, C's two, A+B, A+C, B+C, one that counts nothing, A+B+C's two, as the
/// experience they add tells) points, the weapons' experience and the hit counted.
pub fn weapon_hit(cpu: &mut Cpu, shot: u32, target: u32) {
    let power = cpu.read16(shot + POWER) as i16 as i32 as u32;
    let life = nested!(cpu, DAMAGE, damage(cpu, target, power)) as i32;
    if life < 0 {
        return;
    }
    if life == 0 && cpu.read32(target + FLAGS) & NO_SCORE == 0 {
        let kind = cpu.read8(target + OBJ_KIND) as i8 as i32;
        let points = cpu.read16(POINTS.wrapping_add((16 * kind) as u32)) as i16 as i32 as u32;
        let weapon = cpu.read8(shot + TYPE) as i8 as i32 as u32;
        let p = cpu.read8(shot + NUMBER) as i8 as i32 as u32;
        nested!(cpu, SCORE_KILL, score_kill(cpu, p, weapon, target, points));
    }
    if cpu.read32(target + FLAGS) & NO_EXPERIENCE != 0 {
        return;
    }
    let weapon = cpu.read8(shot + TYPE) as i8 as i32;
    if !(7..=16).contains(&weapon) {
        return;
    }
    let p = cpu.read8(shot + NUMBER) as i8 as i32;
    let score = SCORE.wrapping_add((200 * p) as u32);
    let experience = EXPERIENCE_A.wrapping_add((12 * p) as u32);
    let hits = WEAPON_HITS.wrapping_add((28 * p) as u32);
    let add = |cpu: &mut Cpu, at: u32, n: u32| {
        let v = cpu.read32(at);
        cpu.write32(at, v.wrapping_add(n));
    };
    // (points, experience A, B, C, the hit counted)
    let (points, a, b, c, counted): (u32, u32, u32, u32, Option<u32>) = match weapon {
        7 => (10, 10, 0, 0, Some(0)),
        8 => (10, 0, 10, 0, Some(1)),
        9 => (1, 0, 0, 1, None),
        10 => (50, 0, 0, 50, Some(2)),
        11 => (2, 1, 1, 0, Some(3)),
        12 => (10, 5, 0, 5, None),
        13 => (2, 0, 1, 1, Some(5)),
        14 => return,
        15 => (3, 1, 1, 1, Some(6)),
        _ => (3, 1, 1, 1, None),
    };
    add(cpu, score, points);
    for (k, n) in [a, b, c].into_iter().enumerate() {
        if n != 0 {
            add(cpu, experience + 4 * k as u32, n);
        }
    }
    if let Some(k) = counted {
        add(cpu, hits + 4 * k, 1);
    }
    if weapon == 12 {
        let timer = AC_TIMER.wrapping_add((28 * p) as u32);
        cpu.write16(timer, AC_TIME);
        let n = cpu.read16(timer + 2);
        cpu.write16(timer + 2, n.wrapping_add(1));
    }
}

/// The chains (in the hit manager, by player): the colour of the last kill, the kills of one
/// colour in a row (a word), the chain's score so far (a word, up to 10000), the kills toward the
/// next three-of-a-colour bonus, and those bonuses taken.
const LAST_COLOUR: u32 = 0x10;
const CHAIN: u32 = 0x12;
const CHAIN_SCORE: u32 = 0x16;
const TRIPLE: u32 = 0x1a;
const TRIPLES: u32 = 0x1c;
/// The colours (an enemy's type 0-2), and each player's longest chain by colour (words in their
/// PLAY_DATA).
const COLOURS: i32 = 3;
const LONGEST_CHAIN: [u32; 3] = [0x0600_ebea, 0x0600_ebec, 0x0600_ebee];
/// The three-of-a-colour bonus, the chain score's cap, and the chain bonus's.
const TRIPLE_BONUS: u32 = 10000;
const CHAIN_SCORE_CAP: u32 = 10000;
const CHAIN_BONUS_CAP: u32 = 100000;
/// A score shown where it was made (an object, from a struct: x, y, z, the score at +0x14, what it
/// is at +0x1a (POPUP_*), the player at +0x1e, the object at +0x3c: effects.rs's popup).
const POPUP: u32 = 0x0604_ea48;
const POPUP_SCORE: u32 = 0x14;
const POPUP_WHAT: u32 = 0x1a;
const POPUP_PLAYER: u32 = 0x1e;
const POPUP_OBJ: u32 = 0x3c;
const POPUP_TRIPLE: u16 = 1;
const POPUP_CHAIN: u16 = 2;
const POPUP_POINTS: u16 = 3;
/// The weapon (TYPE) whose scores show no popup.
const NO_POPUP: i32 = 18;

/// A score popup for `obj` (its position) at the frame's struct.
fn popup(cpu: &mut Cpu, obj: u32, score: u32, what: u16, p: i32) {
    let s = cpu.stack();
    for k in 0..3 {
        let v = cpu.read32(obj + 0x14 + 4 * k);
        cpu.write32(s + 4 * k, v);
    }
    cpu.write32(s + POPUP_SCORE, score);
    cpu.write16(s + POPUP_WHAT, what);
    cpu.write8(s + POPUP_PLAYER, p as u8);
    cpu.write32(s + POPUP_OBJ, obj);
    nested!(cpu, POPUP, super::effects::popup(cpu, s));
}

/// Player `p` scores `points` with `weapon` for killing `target` (0: points not from a kill, shown
/// at the player): the score; a kill of a colour keeps the chains (three of a colour in a row: a
/// bonus of 10000; every third kill of a chain: the chain's score times its thirds, up to 100000,
/// and the longest chain kept); then the points count for the weapon's experience (A, B, C; shared
/// by the weapons that combine them).
pub fn score_kill(cpu: &mut Cpu, p: u32, weapon: u32, target: u32, points: u32) {
    let p = p as i8 as i32;
    let weapon = weapon as i8 as i32;
    let mut points = points;
    if points == 0 || !(0..=1).contains(&p) {
        return;
    }
    let score = SCORE + 200 * p as u32;
    let s = cpu.read32(score);
    cpu.write32(score, s.wrapping_add(points));
    if target == 0 {
        if weapon != NO_POPUP {
            let player = cpu.read32(PLAYERS + 4 * p as u32);
            if player != 0 {
                popup(cpu, player, points, POPUP_POINTS, p);
            }
        }
    } else if (cpu.read8(target + TYPE) as i8 as i32) < COLOURS {
        points = chain_kill(cpu, p, target, points, score);
    }
    let experience = EXPERIENCE_A + 12 * p as u32;
    let add = |cpu: &mut Cpu, k: u32, n: i32| {
        let v = cpu.read32(experience + 4 * k);
        cpu.write32(experience + 4 * k, v.wrapping_add(n as u32));
    };
    let points = points as i32;
    match weapon {
        7 => add(cpu, 0, points),
        8 => add(cpu, 1, points),
        9 | 10 => add(cpu, 2, points),
        11 => {
            add(cpu, 0, points / 2);
            add(cpu, 1, points / 2);
        }
        12 => {
            add(cpu, 0, points / 2);
            add(cpu, 2, points / 2);
        }
        13 => {
            add(cpu, 1, points / 2);
            add(cpu, 2, points / 2);
        }
        6 | 15 | 16 | 18 => {
            for k in 0..3 {
                add(cpu, k, points / 3);
            }
        }
        _ => {}
    }
}

/// The chains for a kill of a colour: the points with the bonuses made.
fn chain_kill(cpu: &mut Cpu, p: i32, target: u32, points: u32, score: u32) -> u32 {
    let mut points = points;
    let p = p as u32;
    let colour = cpu.read8(target + TYPE) as i8;
    let hits = cpu.read32(HITS);
    let triple = hits + p + TRIPLE;
    let t = cpu.read8(triple) as i8;
    if t == colour {
        cpu.write8(triple, t.wrapping_add(1) as u8);
        let triple = cpu.read32(HITS) + p + TRIPLE;
        if cpu.read8(triple) == 3 {
            cpu.write8(triple, 0);
            let triples = cpu.read32(HITS) + p + TRIPLES;
            let n = cpu.read8(triples);
            if n <= 0xfe {
                cpu.write8(triples, n + 1);
            }
            popup(cpu, target, TRIPLE_BONUS, POPUP_TRIPLE, p as i32);
            let s = cpu.read32(score);
            cpu.write32(score, s.wrapping_add(TRIPLE_BONUS));
            points = points.wrapping_add(TRIPLE_BONUS);
        }
    } else {
        cpu.write8(triple, (colour == 0) as u8);
        let triples = cpu.read32(HITS) + p + TRIPLES;
        cpu.write8(triples, 0);
    }
    let colour = cpu.read8(target + TYPE);
    let last = cpu.read32(HITS) + p + LAST_COLOUR;
    if cpu.read8(last) != colour {
        cpu.write8(last, colour);
        let h = cpu.read32(HITS) + 2 * p;
        cpu.write16(h + CHAIN, 0);
        cpu.write16(h + CHAIN_SCORE, 0);
    }
    let hits = cpu.read32(HITS);
    if cpu.read8(hits + p + LAST_COLOUR) != cpu.read8(target + TYPE) {
        return points;
    }
    let chain = hits + 2 * p + CHAIN;
    let n = cpu.read16(chain);
    if n <= 0xfffe {
        cpu.write16(chain, n.wrapping_add(1));
    }
    let hits = cpu.read32(HITS);
    let n = cpu.read16(hits + 2 * p + CHAIN);
    let colour = cpu.read8(hits + p + LAST_COLOUR) as i8;
    if (0..COLOURS as i8).contains(&colour) {
        let longest = LONGEST_CHAIN[colour as usize] + 200 * p;
        if cpu.read16(longest) < n {
            cpu.write16(longest, n);
        }
    }
    let chain_score = cpu.read32(HITS) + 2 * p + CHAIN_SCORE;
    let sum = (cpu.read16(chain_score) as i16 as i32).wrapping_add(points as i32);
    let capped = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, sum as u32, CHAIN_SCORE_CAP));
    let chain_score = cpu.read32(HITS) + 2 * p + CHAIN_SCORE;
    cpu.write16(chain_score, capped as u16);
    let hits = cpu.read32(HITS);
    let n = cpu.read16(hits + 2 * p + CHAIN) as u32;
    if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, n, 3)) != 0 {
        return points;
    }
    let hits = cpu.read32(HITS);
    let n = cpu.read16(hits + 2 * p + CHAIN) as u32;
    let thirds = nested!(cpu, 0x0600_6a78, math::quotient(cpu, n, 3)) as i32;
    let hits = cpu.read32(HITS);
    let chain_score = cpu.read16(hits + 2 * p + CHAIN_SCORE) as i16 as i32;
    let bonus = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, thirds.wrapping_mul(chain_score) as u32, CHAIN_BONUS_CAP));
    let s = cpu.read32(SCORE + 200 * p);
    cpu.write32(SCORE + 200 * p, s.wrapping_add(bonus));
    points = points.wrapping_add(bonus);
    popup(cpu, target, bonus, POPUP_CHAIN, p as i32);
    points
}

/// An object's position (x, y, z: 16.16 longs), its mode byte's neighbour (passed on to the effects
/// it makes, not known yet), and the sword's gauge (a player's, hud.rs: from 10000).
const POSITION: u32 = 0x14;
const OBJ_33: u32 = 0x33;
const GAUGE: u32 = 0x50;
/// The hit flags that turn over at each contact: the struck object's, then the striker's when the
/// first turned on (so a spark comes of one contact in four).
const STRUCK_TURN: u16 = 0x10;
const STRIKER_TURN: u16 = 0x20;
/// No new sparks while the sparks' frames are above this.
const SPARKS_MOST: i32 = 600;
/// The weapon a spark's points are scored with (no popup), and the points.
const SPARK_WEAPON: u32 = 18;
const SPARK_POINTS: u32 = 3;
/// A spark (made from a struct in the frame: x, y, z, two scales, a kind, an angle, the player, the
/// first character, its frames, and more after), its kind, its frames (5 at random, and 15), and
/// the spark maker; then how it flies: an angle from the struck object's push, turned a quarter
/// either way (the vector turned by TURN_POINT, its angle from SL_ATAN; as far as can be told).
const SPARK_SET: u32 = 0x0604_e6d8;
const SPARK_KIND: u32 = 0x2b;
const SPARK_FRAMES: u16 = 15;
const SL_ATAN: u32 = 0x0606_780c;
const TURN_POINT: u32 = 0x0606_4eac;
// (register_boxes calls it with the SH-2 code's return addresses: its push of PR, made before it
// calls SGL's cosine, is read later as an unset field.)
/// The effect's struct (in the frame): x, y, z, its scales (16.16), its kind, its angle (a word),
/// two bytes, the player, its first character (a word), its frames (a word), one byte more.
const FX_X: u32 = 0;
const FX_Y: u32 = 4;
const FX_Z: u32 = 8;
const FX_SCALE: u32 = 0xc;
const FX_KIND: u32 = 0x14;
const FX_ANGLE: u32 = 0x18;
const FX_1C: u32 = 0x1c;
const FX_PLAYER: u32 = 0x1e;
const FX_CHAR: u32 = 0x20;
const FX_FRAMES: u32 = 0x24;
const FX_26: u32 = 0x26;
/// What sparks fly with: their speed (x, y as words), its turn, and the speed given.
const SPARK_SPEED: u32 = 0x44;

/// Where the boxes `a` and `b` meet on one axis (16.16): between their facing edges.
fn meeting(cpu: &mut Cpu, a: u32, b: u32, offset: u32, half: u32) -> u32 {
    let ac = cpu.read16(a + offset) as i16 as i32;
    let bc = cpu.read16(b + offset) as i16 as i32;
    let sum = if ac < bc {
        let ah = cpu.read16(a + half) as i16 as i32;
        let bh = cpu.read16(b + half) as i16 as i32;
        ac + ah + (bc - bh)
    } else {
        let ah = cpu.read16(a + half) as i16 as i32;
        let bh = cpu.read16(b + half) as i16 as i32;
        (ac - ah) + (bc + bh)
    };
    sum.wrapping_mul(0x8000) as u32
}

/// `a` strikes `b` with sparks (a sword on a bullet, as far as can be told): they touch, `a`'s
/// player scores; one contact in four, while the sparks are few enough, a spark flies off where the
/// boxes meet.
pub fn sparks(cpu: &mut Cpu, a: u32, b: u32, a_box: u32, b_box: u32) {
    cpu.write32(a + HIT_OTHER, b);
    set_hit_flags(cpu, a, TOUCHED);
    let p = cpu.read8(a + NUMBER) as i8 as i32 as u32;
    nested!(cpu, SCORE_KILL, score_kill(cpu, p, SPARK_WEAPON, 0, SPARK_POINTS));
    let f = cpu.read16(b + HIT_FLAGS) ^ STRUCK_TURN;
    cpu.write16(b + HIT_FLAGS, f);
    if f & STRUCK_TURN == 0 {
        return;
    }
    let f = cpu.read16(a + HIT_FLAGS) ^ STRIKER_TURN;
    cpu.write16(a + HIT_FLAGS, f);
    if f & STRIKER_TURN == 0 {
        return;
    }
    let hits = cpu.read32(HITS);
    if cpu.read32(hits + SPARKS) as i32 > SPARKS_MOST {
        return;
    }
    let s = cpu.stack();
    let x = meeting(cpu, a_box, b_box, X, HALF_W);
    cpu.write32(s + FX_X, x);
    let y = meeting(cpu, a_box, b_box, Y, HALF_H);
    cpu.write32(s + FX_Y, y);
    for (field, k) in [(FX_X, 0), (FX_Y, 4)] {
        const STEP: i32 = 0x10_0000;
        let mut v = cpu.read32(s + field) as i32;
        let at = cpu.read32(a + POSITION + k) as i32;
        if v.wrapping_sub(at) < -STEP {
            v = at.wrapping_sub(STEP);
            cpu.write32(s + field, v as u32);
        }
        let v = cpu.read32(s + field) as i32;
        let at = cpu.read32(a + POSITION + k) as i32;
        if v.wrapping_sub(at) > STEP {
            cpu.write32(s + field, at.wrapping_add(STEP) as u32);
        }
    }
    let z = cpu.read32(a + POSITION + 8);
    cpu.write32(s + FX_Z, z);
    cpu.write32(s + FX_SCALE + 4, 0x1_0000);
    cpu.write32(s + FX_SCALE, 0x1_0000);
    cpu.write32(s + FX_KIND, SPARK_KIND);
    let angle = nested!(cpu, 0x0604_5d90, math::random(cpu, 0));
    cpu.write16(s + FX_ANGLE, angle as u16);
    let frames = nested!(cpu, 0x0604_5d90, math::random(cpu, 5)) as u16 + SPARK_FRAMES;
    cpu.write16(s + FX_FRAMES, frames);
    cpu.write8(s + FX_26, 0);
    let ch = cpu.read16(CHAR_BASE);
    cpu.write16(s + FX_CHAR, ch);
    let n = cpu.read8(a + NUMBER);
    cpu.write8(s + FX_PLAYER, n);
    let spark = nested!(cpu, SPARK_SET, super::effects::polygon_set(cpu, s));
    if spark == 0 {
        return;
    }
    let v = cpu.read8(a + OBJ_33);
    cpu.write8(spark + OBJ_33, v);
    for k in 0..3 {
        cpu.write16(spark + 0x70 + 2 * k, 0);
    }
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 4));
    cpu.write16(spark + 0x76, r as u16);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x2000));
    cpu.write16(spark + 0x42, r as u16);
    cpu.write32(s + FX_X, 0);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x2_0000));
    cpu.write32(s + FX_Y, r.wrapping_sub(0x3_0000));
    cpu.write32(s + FX_KIND, 0);
    let push_y = cpu.read32(b + PUSH + 4);
    let push_x = cpu.read32(b + PUSH);
    let away = cpu.call(SL_ATAN, &[push_x, push_y]);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x800));
    let mut angle = (r as u16).wrapping_add(0x4000).wrapping_add(away as u16);
    cpu.write16(s + FX_ANGLE, angle);
    let side = nested!(cpu, 0x0604_5d90, math::random(cpu, 0)) as i32;
    angle = cpu.read16(s + FX_ANGLE).wrapping_add(if side > 0 { 0x4800 } else { 0xb800 });
    cpu.write16(s + FX_ANGLE, angle);
    nested!(cpu, TURN_POINT, math::turn_point(cpu, s));
    let x = cpu.read32(s + FX_X);
    cpu.write32(spark + SPARK_SPEED, x);
    let y = cpu.read32(s + FX_Y);
    cpu.write32(spark + SPARK_SPEED + 4, y);
    let hits = cpu.read32(HITS);
    let n = cpu.read16(s + FX_FRAMES) as i16 as i32;
    let total = cpu.read32(hits + SPARKS);
    cpu.write32(hits + SPARKS, total.wrapping_add(n as u32));
}

/// A striker of type 15 charges its player's gauge as it absorbs (as far as can be told: the
/// sword), by 1000 up to 25000; the flash an absorbed bullet leaves.
const ABSORBER: u8 = 15;
const ABSORB_CHARGE: u32 = 1000;
const GAUGE_MOST: u32 = 25000;
const FLASH_SET: u32 = 0x0604_e1cc;
const FLASH_SCALE: u32 = 0x8000;
const FLASH_FRAMES: u16 = 8;
const FLASH_CHAR: u16 = 0x20;
const FLASH_33: u8 = 8;

/// `a` absorbs the bullet `b`: they touch, `a` may charge its player's gauge, the bullet ends with a
/// flash where it was.
pub fn absorb(cpu: &mut Cpu, a: u32, b: u32) {
    nested!(cpu, TOUCH, touch(cpu, a, b));
    if cpu.read8(a + TYPE) == ABSORBER {
        let n = cpu.read8(a + NUMBER) as i8 as i32;
        let player = cpu.read32(PLAYERS.wrapping_add((4 * n) as u32));
        if player != 0 {
            let g = cpu.read32(player + GAUGE).wrapping_add(ABSORB_CHARGE);
            let g = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, g, GAUGE_MOST));
            cpu.write32(player + GAUGE, g);
        }
    }
    cpu.write8(b + OBJ_MODE, ENDING);
    let s = cpu.stack();
    for k in 0..3 {
        let v = cpu.read32(b + POSITION + 4 * k);
        cpu.write32(s + 4 * k, v);
    }
    cpu.write32(s + FX_SCALE + 4, FLASH_SCALE);
    cpu.write32(s + FX_SCALE, FLASH_SCALE);
    cpu.write32(s + FX_KIND, 0);
    cpu.write16(s + FX_ANGLE, 0);
    cpu.write16(s + FX_FRAMES, FLASH_FRAMES);
    cpu.write8(s + FX_26, FLASH_FRAMES as u8);
    cpu.write8(s + FX_1C, 0);
    cpu.write8(s + FX_1C + 1, 2);
    let ch = cpu.read16(CHAR_BASE).wrapping_add(FLASH_CHAR);
    cpu.write16(s + FX_CHAR, ch);
    let n = cpu.read8(b + NUMBER);
    cpu.write8(s + FX_PLAYER, n);
    let flash = nested!(cpu, FLASH_SET, super::effects::effect_set(cpu, s));
    if flash != 0 {
        cpu.write8(flash + OBJ_33, FLASH_33);
    }
}

/// No colour yet (the last colour a chain is reset to).
const NO_COLOUR: u8 = 0x11;

/// Player `p`'s chains cleared.
pub fn chain_reset(cpu: &mut Cpu, p: u32) {
    let hits = cpu.read32(HITS);
    if hits == 0 {
        return;
    }
    let p = p as i8 as i32 as u32;
    cpu.write8(hits.wrapping_add(p) + LAST_COLOUR, NO_COLOUR);
    let hits = cpu.read32(HITS);
    cpu.write16(hits.wrapping_add(2 * p) + CHAIN, 0);
    cpu.write16(hits.wrapping_add(2 * p) + CHAIN_SCORE, 0);
    cpu.write8(hits.wrapping_add(p) + TRIPLE, 0);
    let hits = cpu.read32(HITS);
    cpu.write8(hits.wrapping_add(p) + TRIPLES, 0);
}

/// The hit lists emptied (this frame's counts and last frame's) and both players' chains cleared.
pub fn hits_reset(cpu: &mut Cpu) {
    let hits = cpu.read32(HITS);
    if hits == 0 {
        return;
    }
    for k in 0..7 {
        cpu.write16(hits + COUNTS + 2 * k, 0);
        cpu.write16(hits + LAST_COUNTS + 2 * k, 0);
    }
    nested!(cpu, CHAIN_RESET, chain_reset(cpu, 0));
    nested!(cpu, CHAIN_RESET, chain_reset(cpu, 1));
}

/// The hit manager's size and exec, and its list (4).
const HITS_SIZE: u32 = 0x1a40;
const HITS_EXEC: u32 = 0x0604_3d38;
const HITS_AREA: u32 = 4;

/// Makes the hit manager (object list 4), its lists empty: it, or 0.
pub fn hits_set(cpu: &mut Cpu) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, HITS_SIZE));
    let hits = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, HITS_AREA, blocks as i8 as i32 as u32, HITS_EXEC));
    cpu.write32(HITS, hits);
    if hits != 0 {
        nested!(cpu, HITS_RESET, hits_reset(cpu));
        let hits = cpu.read32(HITS);
        cpu.write32(hits + SPARKS, 0);
        cpu.write8(hits + OBJ_MODE, 1);
    }
    cpu.read32(HITS)
}

/// An object's boxes (8 bytes each, the last with LAST_BOX): what it is (BODY, or else a shot),
/// its kind, its offset (x, y: words, pixels), its half width and height (bytes, scaled by the
/// object's scales).
const BOXES: u32 = 0x3c;
const LAST_BOX: u8 = 0x80;
const BODY: u8 = 0x40;
/// The sprite fields the boxes follow: the object's scales (16.16) and angle.
const OBJ_SCALE_X: u32 = 0x20;
const OBJ_SCALE_Y: u32 = 0x24;
const OBJ_ANGLE: u32 = 0x30;
/// The object's flags: its boxes not registered (until one is), no hits, the boxes not flipped,
/// not turned (with the sprite's own unturned bit).
const UNREGISTERED: u32 = 0x0100_0000;
const NO_HITS: u32 = 0x8_0000;
const BOXES_UNFLIPPED: u32 = 0x1_0000;
const BOXES_UNTURNED: u32 = 0x1001_0000;
/// The sides: the players', a third (not known yet), the enemies' (any other).
const SIDE_PLAYERS: i8 = 0;
const SIDE_THIRD: i8 = 3;
/// A shot box of this kind on the enemies' side is a point (list 6: small bullets, as far as can be
/// told).
const POINT_KIND: i8 = 2;
/// A box further from the screen's centre than its half size and this (pixels) is not registered.
const SCREEN_X: i32 = 0xb0;
const SCREEN_Y: i32 = 0x70;

/// The list a box goes in, and its most boxes: bodies (players 0, the third side 2, enemies 1)
/// and shots (players 3, the third side 5, enemies' points 6, enemies 4).
fn box_list(cpu: &mut Cpu, obj: u32, desc: u32) -> (usize, i32) {
    let side = cpu.read8(obj + SIDE) as i8;
    if cpu.read8(desc) & 0x7f == BODY {
        match side {
            SIDE_PLAYERS => (0, 4),
            SIDE_THIRD => (2, 16),
            _ => (1, 60),
        }
    } else {
        match side {
            SIDE_PLAYERS => (3, 64),
            SIDE_THIRD => (5, 16),
            _ if cpu.read8(desc + 1) as i8 == POINT_KIND => (6, 128),
            _ => (4, 128),
        }
    }
}

/// A 16.16 value in whole pixels (toward 0).
fn pixels(v: u32) -> u16 {
    ((v as i32) / 0x1_0000) as u16
}

/// The turn struct (the frame's bottom, as draw_sprite's offset struct): x, y, scratch, the
/// scales, the flip bits, the angle; with the object's flips and angle unless its flags say not.
fn turn_struct(cpu: &mut Cpu, obj: u32, t: u32) {
    let flags = cpu.read32(obj + FLAGS);
    let flip = if flags & BOXES_UNFLIPPED != 0 { 0 } else { flags & 0x30 };
    cpu.write32(t + 0x14, flip);
    let angle = if cpu.read32(obj + FLAGS) & BOXES_UNTURNED != 0 { 0 } else { cpu.read16(obj + OBJ_ANGLE) };
    cpu.write16(t + 0x18, angle);
}

/// Registers `obj`'s boxes in the hit manager's lists (those on screen, while the lists have
/// room): 1, or (none registered) -5; -1 without a manager, -2 with NO_HITS, -3 without boxes, -4
/// with an empty hit set.
pub fn register_boxes(cpu: &mut Cpu, obj: u32) -> u32 {
    let flags = cpu.read32(obj + FLAGS) | UNREGISTERED;
    cpu.write32(obj + FLAGS, flags);
    if cpu.read32(HITS) == 0 {
        return -1i32 as u32;
    }
    if flags & NO_HITS != 0 {
        return -2i32 as u32;
    }
    let mut desc = cpu.read32(obj + BOXES);
    if desc == 0 {
        return -3i32 as u32;
    }
    if cpu.read32(obj + HIT_SET) == 0 && cpu.read32(obj + HIT_SET + 4) == 0 {
        return -4i32 as u32;
    }
    let t = cpu.stack();
    let mut registered = 0i8;
    let mut index = 0i8;
    loop {
        let (list, most) = box_list(cpu, obj, desc);
        let hits = cpu.read32(HITS);
        let count = hits + COUNTS + 2 * list as u32;
        let base = hits + LISTS[list];
        if cpu.read16(count) as i16 as i32 > most - 1 {
            break;
        }
        let entry = |cpu: &mut Cpu| base + BOX * cpu.read16(count) as i16 as i32 as u32;
        let kind = cpu.read8(desc + 1);
        let e = entry(cpu);
        cpu.write8(e + KIND, kind);
        let e = entry(cpu);
        cpu.write8(e + ATTR, index as u8);
        let (ox, oy) = (cpu.read16(desc + 2), cpu.read16(desc + 4));
        let (dx, dy) = if ox == 0 && oy == 0 {
            (0, 0)
        } else {
            cpu.write32(t, ((ox as i16 as i32) << 16) as u32);
            cpu.write32(t + 4, ((oy as i16 as i32) << 16) as u32);
            let sx = cpu.read32(obj + OBJ_SCALE_X);
            cpu.write32(t + 0xc, sx);
            let sy = cpu.read32(obj + OBJ_SCALE_Y);
            cpu.write32(t + 0x10, sy);
            turn_struct(cpu, obj, t);
            cpu.call_returning(0x0604_53c8, TURN_POINT, &[t]);
            (cpu.read32(t), cpu.read32(t + 4))
        };
        let e = entry(cpu);
        let x = cpu.read32(obj + POSITION).wrapping_add(dx);
        cpu.write16(e + X, pixels(x));
        let e = entry(cpu);
        let y = cpu.read32(obj + POSITION + 4).wrapping_add(dy);
        cpu.write16(e + Y, pixels(y));
        for (k, scale) in [(6, OBJ_SCALE_X), (7, OBJ_SCALE_Y)] {
            let half = cpu.read8(desc + k) as u32;
            let s = cpu.read32(obj + scale);
            let v = (nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, half, s)) as i32).wrapping_abs();
            let e = entry(cpu);
            cpu.write16(e + if k == 6 { HALF_W } else { HALF_H }, v as u16);
        }
        let e = entry(cpu);
        let (w, h) = (cpu.read16(e + HALF_W) as i16 as i32, cpu.read16(e + HALF_H) as i16 as i32);
        if w != h {
            cpu.write32(t + 0x10, 0x1_0000);
            cpu.write32(t + 0xc, 0x1_0000);
            turn_struct(cpu, obj, t);
            let e = entry(cpu);
            let (w, h) = (cpu.read16(e + HALF_W) as i16 as i32, cpu.read16(e + HALF_H) as i16 as i32);
            if w > h {
                let d = w - h;
                cpu.write32(t, d as u32);
                cpu.write32(t + 8, d as u32);
                cpu.write32(t + 4, 0);
                cpu.call_returning(0x0604_54ee, TURN_POINT, &[t]);
                let a = (cpu.read32(t) as i32).wrapping_abs().wrapping_sub(cpu.read32(t + 8) as i32);
                cpu.write32(t, a as u32);
                let b = (cpu.read32(t + 4) as i32).wrapping_abs();
                cpu.write32(t + 4, b as u32);
            } else {
                let d = h - w;
                cpu.write32(t + 4, d as u32);
                cpu.write32(t + 8, d as u32);
                cpu.write32(t, 0);
                cpu.call_returning(0x0604_5534, TURN_POINT, &[t]);
                let a = (cpu.read32(t) as i32).wrapping_abs();
                cpu.write32(t, a as u32);
                let b = (cpu.read32(t + 4) as i32).wrapping_abs().wrapping_sub(cpu.read32(t + 8) as i32);
                cpu.write32(t + 4, b as u32);
            }
            // The growth's low halves (as the SH-2 code adds them: words).
            let grow_w = cpu.read16(t + 2);
            let e = entry(cpu);
            let v = cpu.read16(e + HALF_W).wrapping_add(grow_w);
            cpu.write16(e + HALF_W, v);
            let grow_h = cpu.read16(t + 6);
            let e = entry(cpu);
            let v = cpu.read16(e + HALF_H).wrapping_add(grow_h);
            cpu.write16(e + HALF_H, v);
        }
        let e = entry(cpu);
        let x = (cpu.read16(e + X) as i16 as i32).abs();
        let on_screen = x - (cpu.read16(e + HALF_W) as i16 as i32 + SCREEN_X) < 0 && {
            let y = (cpu.read16(e + Y) as i16 as i32).abs();
            y - (cpu.read16(e + HALF_H) as i16 as i32 + SCREEN_Y) < 0
        };
        if on_screen {
            cpu.write32(e + OBJ, obj);
            let n = cpu.read16(count).wrapping_add(1);
            cpu.write16(count, n);
            registered = registered.wrapping_add(1);
        }
        if cpu.read8(desc) & LAST_BOX != 0 {
            break;
        }
        desc += 8;
        index = index.wrapping_add(1);
    }
    if registered == 0 {
        return -5i32 as u32;
    }
    let flags = cpu.read32(obj + FLAGS);
    cpu.write32(obj + FLAGS, flags & !UNREGISTERED);
    1
}

/// An actor's hit fields set up (from +0x3c, `h`): no boxes, a full hit set (hits every numbered
/// object), its type 4, no hit flags, a damage of 100.
pub fn hit_init(cpu: &mut Cpu, h: u32) {
    cpu.write32(h, 0);
    cpu.write32(h + 0x10, u32::MAX);
    cpu.write32(h + 0xc, u32::MAX);
    cpu.write8(h + 0x19, 4);
    cpu.write16(h + 0x1a, 0);
    cpu.write16(h + 0x1c, 100);
}

/// An object's box list (as at BOXES: 8 bytes a box, the last with LAST_BOX) copied from `from` to
/// `to`.
const LIST_BOX: u32 = 8;

pub fn boxes_copy(cpu: &mut Cpu, to: u32, from: u32) {
    let (mut to, mut from) = (to, from);
    loop {
        for at in [0, 1, 6, 7] {
            let v = cpu.read8(from + at);
            cpu.write8(to + at, v);
        }
        for at in [2, 4] {
            let v = cpu.read16(from + at);
            cpu.write16(to + at, v);
        }
        if cpu.read8(from) & LAST_BOX != 0 {
            return;
        }
        to += LIST_BOX;
        from += LIST_BOX;
    }
}
