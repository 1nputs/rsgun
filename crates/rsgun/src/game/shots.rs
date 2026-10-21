//! The players' shots (player.rs's weapon states make them): actors on the players' side (list 1)
//! with the weapon as their type (hits.rs counts their hits and experience by it), each counting
//! itself alive in its ship each frame (the ship's limits), ended off screen or where they hit
//! something or the ground (with a spark).

use super::{actors, bg, effects, math, nested, objects, sprites, Cpu};

/// The objects' mode byte (ending).
const OBJ_MODE: u32 = 3;
const ENDING: u8 = 0x70;
/// A shot's sprite (flags, position, scales, colours, kind, frame, angle, number, lift), boxes,
/// behaviour, side and type, hit flags, power (a word), speed, its phase (a word: the low byte),
/// its link (effects.rs's), its ship.
const SPRITE: u32 = 0x10;
const FLAGS: u32 = 0x10;
const POSITION: u32 = 0x14;
const SCALES: u32 = 0x20;
const COLOUR: u32 = 0x28;
const OWN_COLOUR: u32 = 0x2a;
const KIND: u32 = 0x2e;
const FRAME: u32 = 0x2f;
const ANGLE: u32 = 0x30;
const NUMBER: u32 = 0x32;
const LIFT: u32 = 0x33;
const BOXES: u32 = 0x3c;
const BEHAVIOUR: u32 = 0x38;
const SIDE: u32 = 0x54;
const TYPE: u32 = 0x55;
const HIT_FLAGS: u32 = 0x56;
const POWER: u32 = 0x58;
const SPEED: u32 = 0x5c;
const PHASE: u32 = 0x6c;
const PHASE_LOW: u32 = 0x6d;
const SHIP: u32 = 0xb0;
/// The hit flags: hit something.
const HIT_SOMETHING: u16 = 0x400;
/// The flags: no hits.
const NO_HITS: u32 = 0x8_0000;
/// The kinds' table (sprites.rs's): a kind's colour at +0xc (16 bytes a kind).
const KIND_COLOURS: u32 = 0x0607_41fc;
const SHOT_SIZE: u32 = 0xb4;
const ACTOR_EXEC: u32 = 0x0604_d780;
/// The screen's edges for shots (from its centre).
const EDGE_X: i32 = 0xa7_ffff;
const EDGE_Y: i32 = 0x7f_ffff;
/// The weapons' hit counts (hits.rs's: 28 bytes a player; A's first).
const WEAPON_HITS: u32 = 0x0607_40a0;
/// The weapons' levels (A, B, C bytes; 3 a player).
const LEVELS: u32 = 0x0607_4088;
/// The ship's counts of shots alive (player.rs's): A's, all.
const ALIVE_A: u32 = 0x80;
const ALIVE_ALL: u32 = 0x87;
/// SGL's: a vector turned (sprites.rs's SL_TURN).
const SL_TURN: u32 = 0x0606_4eac;

fn end(cpu: &mut Cpu, s: u32) {
    cpu.write8(s + OBJ_MODE, ENDING);
}

fn number(cpu: &mut Cpu, s: u32) -> i32 {
    cpu.read8(s + NUMBER) as i8 as i32
}

/// A shot in list 1 run by the actors' frame, set up: it, or 0.
fn shot_in(cpu: &mut Cpu) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, SHOT_SIZE)) as i8 as i32 as u32;
    let s = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 1, blocks, ACTOR_EXEC));
    if s != 0 {
        nested!(cpu, 0x0604_62ac, actors::actor_setup(cpu, s));
    }
    s
}

/// The spark where a shot hits (effects.rs's effect_set, linked to an animation), at its place and
/// angle, its width and three quarters of its height, its own colour.
const HIT_SPARK: u32 = 0x0607_2920;

pub fn shot_spark(cpu: &mut Cpu, s: u32) {
    let f = cpu.stack();
    for k in 0..3 {
        let v = cpu.read32(s + POSITION + 4 * k);
        cpu.write32(f + 4 * k, v);
    }
    let w = cpu.read32(s + SCALES);
    let w = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, w, 0x1_0000));
    cpu.write32(f + 0x0c, w);
    let h = cpu.read32(s + SCALES + 4);
    let h = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, h, 0xc000));
    cpu.write32(f + 0x10, h);
    cpu.write32(f + 0x14, 3);
    let a = cpu.read16(s + ANGLE);
    cpu.write16(f + 0x18, a);
    cpu.write16(f + 0x24, 0);
    for field in [0x26, 0x1c, 0x1d] {
        cpu.write8(f + field, 0);
    }
    let colour = cpu.read16(s + OWN_COLOUR);
    cpu.write16(f + 0x20, colour);
    let n = cpu.read8(s + NUMBER);
    cpu.write8(f + 0x1e, n);
    cpu.write32(f + 0x28, HIT_SPARK);
    cpu.write32(f + 0x3c, 0);
    let fx = nested!(cpu, 0x0604_e1cc, effects::effect_set(cpu, f));
    // the SH-2 code keeps the effect in its struct's last long
    cpu.write32(f + 0x3c, fx);
}

/// A's shot: kind 3, its animation, the kind's colour, side 0, type 7 (A).
const A_KIND: u8 = 3;
const A_ANIMATION: u32 = 0x0607_28ac;
const A_TYPE: u8 = 7;
const A_BOXES: u32 = 0x0607_2824;
const A_SPEED: u32 = 0xfff8_0000;

fn a_start(cpu: &mut Cpu, s: u32, link: u32) {
    cpu.write8(s + KIND, A_KIND);
    nested!(cpu, 0x0604_6530, effects::link_init(cpu, s + link, A_ANIMATION));
    let kind = cpu.read8(s + KIND) as i8 as i32;
    let colour = cpu.read16(KIND_COLOURS.wrapping_add((kind * 16) as u32));
    cpu.write16(s + OWN_COLOUR, colour);
    cpu.write16(s + COLOUR, colour);
    cpu.write8(s + LIFT, 0);
    cpu.write8(s + SIDE, 0);
    cpu.write8(s + TYPE, A_TYPE);
    cpu.write32(s + BOXES, A_BOXES);
}

/// Whether the shot is on screen (else its player's A hit count is cleared).
fn on_screen(cpu: &mut Cpu, s: u32) -> bool {
    let (x, y) = (cpu.read32(s + POSITION) as i32, cpu.read32(s + POSITION + 4) as i32);
    if x.wrapping_abs() > EDGE_X || y.wrapping_abs() > EDGE_Y {
        let n = number(cpu, s);
        cpu.write32(WEAPON_HITS.wrapping_add((n * 28) as u32), 0);
        return false;
    }
    true
}

/// Whether the shot hit something or the ground (then its spark).
fn hit_here(cpu: &mut Cpu, s: u32) -> bool {
    let hit = cpu.read16(s + HIT_FLAGS) & HIT_SOMETHING != 0 || {
        let (x, y) = (cpu.read32(s + POSITION), cpu.read32(s + POSITION + 4));
        nested!(cpu, 0x0605_ef90, bg::map_hit(cpu, x, y)) != 0
    };
    if hit {
        nested!(cpu, 0x0604_8278, shot_spark(cpu, s));
    }
    hit
}

/// A's main shot: straight up at 8 pixels a frame, counting itself in its ship; ended off screen
/// or where it hits.
pub fn shot_a_exec(cpu: &mut Cpu, s: u32) {
    match cpu.read8(s + PHASE_LOW) {
        0 => {
            a_start(cpu, s, 0x70);
            cpu.write32(s + SPEED, 0);
            cpu.write32(s + SPEED + 4, A_SPEED);
            cpu.write16(s + PHASE, 1);
        }
        1 => {}
        _ => return,
    }
    if !on_screen(cpu, s) || hit_here(cpu, s) {
        end(cpu, s);
        return;
    }
    let ship = cpu.read32(s + SHIP);
    if cpu.read8(ship + OBJ_MODE) as i8 >= ENDING as i8 {
        cpu.write32(s + SHIP, 0);
    }
    count_alive(cpu, s, ALIVE_A);
    let y = cpu.read32(s + POSITION + 4).wrapping_add(cpu.read32(s + SPEED + 4));
    cpu.write32(s + POSITION + 4, y);
    nested!(cpu, 0x0604_6558, effects::link_step(cpu, s + SPRITE, s + 0x70));
}

/// A side shot's frame count (its hits only on odd frames).
const SIDE_FRAMES: u32 = 0x70;

/// A's side shot: 8 pixels a frame along its angle, hittable every other frame; ended off screen or
/// where it hits.
pub fn shot_a_side_exec(cpu: &mut Cpu, s: u32) {
    match cpu.read8(s + PHASE_LOW) {
        0 => {
            a_start(cpu, s, 0x74);
            let f = cpu.stack();
            cpu.write32(f, 0);
            cpu.write32(f + 4, A_SPEED);
            cpu.write32(f + 0x10, 0x1_0000);
            cpu.write32(f + 0x0c, 0x1_0000);
            cpu.write32(f + 0x14, 0);
            let a = cpu.read16(s + ANGLE);
            cpu.write16(f + 0x18, a);
            cpu.call(SL_TURN, &[f]);
            let (vx, vy) = (cpu.read32(f), cpu.read32(f + 4));
            cpu.write32(s + SPEED, vx);
            cpu.write32(s + SPEED + 4, vy);
            cpu.write16(s + PHASE, 1);
        }
        1 => {}
        _ => return,
    }
    if !on_screen(cpu, s) || hit_here(cpu, s) {
        end(cpu, s);
        return;
    }
    for k in 0..2 {
        let p = cpu.read32(s + POSITION + 4 * k).wrapping_add(cpu.read32(s + SPEED + 4 * k));
        cpu.write32(s + POSITION + 4 * k, p);
    }
    let n = cpu.read32(s + SIDE_FRAMES).wrapping_add(1);
    cpu.write32(s + SIDE_FRAMES, n);
    let flags = cpu.read32(s + FLAGS);
    let flags = if n & 1 != 0 { flags & !NO_HITS } else { flags | NO_HITS };
    cpu.write32(s + FLAGS, flags);
    nested!(cpu, 0x0604_6558, effects::link_step(cpu, s + SPRITE, s + 0x74));
}

/// B's shot (homing): kind 3 in the kind's colour plus 0x40, type 8 (B), growing to its size
/// by B's level; its target's number (a byte), its turn limit (a word, growing while it homes; C's
/// shots clear it too).
const B_ANIMATION: u32 = 0x0607_28b4;
const B_TYPE: u8 = 8;
const B_BOXES: u32 = 0x0607_283c;
const B_COLOUR: u16 = 0x40;
const TARGET: u32 = 0x70;
const TURN: u32 = 0x6e;
const TURN_START: u16 = 0x800;
const TURN_MOST: u32 = 0x1800;
const TURN_GROWTH: u32 = 0x80;
const B_LINK: u32 = 0x74;
/// The wider edge for B's and C's shots (x).
const WIDE_EDGE_X: i32 = 0xbf_ffff;
const B_SPEED: u32 = 0xfffa_0000;
const SEEKING: u16 = 1;
const HOMING: u16 = 2;
/// The ship's B shots alive.
const ALIVE_B: u32 = 0x81;
/// The numbered objects (actors.rs's), the flags that keep one from being homed on, its life.
const NUMBERED: u32 = 0x0607_40f0;
const UNHOMED: u32 = 0x010a_0000;
const LIFE: u32 = 0x50;
const B_LEVEL: u32 = LEVELS + 1;

/// B's shot: it seeks the nearest thing to hit (on the other side, alive, not ending) and homes on
/// it, turning toward it by at most its turn limit a frame (which grows while it homes), at 6
/// pixels a frame; it grows to its size; ended off screen (seeking) or where it hits.
pub fn shot_b_exec(cpu: &mut Cpu, s: u32) {
    if cpu.read8(s + PHASE_LOW) == 0 {
        cpu.write32(s + SCALES + 4, 0);
        cpu.write32(s + SCALES, 0);
        cpu.write8(s + KIND, A_KIND);
        nested!(cpu, 0x0604_6530, effects::link_init(cpu, s + B_LINK, B_ANIMATION));
        let kind = cpu.read8(s + KIND) as i8 as i32;
        let colour = cpu.read16(KIND_COLOURS.wrapping_add((kind * 16) as u32)).wrapping_add(B_COLOUR);
        cpu.write16(s + OWN_COLOUR, colour);
        cpu.write16(s + COLOUR, colour);
        cpu.write8(s + LIFT, 0);
        cpu.write8(s + SIDE, 0);
        cpu.write8(s + TYPE, B_TYPE);
        cpu.write32(s + BOXES, B_BOXES);
        cpu.write16(s + TURN, 0);
        cpu.write16(s + PHASE, SEEKING);
    }
    if hit_here(cpu, s) {
        end(cpu, s);
        return;
    }
    let ship = cpu.read32(s + SHIP);
    if cpu.read8(ship + OBJ_MODE) as i8 >= ENDING as i8 {
        cpu.write32(s + SHIP, 0);
    }
    count_alive(cpu, s, ALIVE_B);
    match cpu.read8(s + PHASE_LOW) as u16 {
        SEEKING => {
            let (x, y) = (cpu.read32(s + POSITION) as i32, cpu.read32(s + POSITION + 4) as i32);
            if x.wrapping_abs() > WIDE_EDGE_X || y.wrapping_abs() > EDGE_Y {
                let n = number(cpu, s);
                cpu.write32(WEAPON_HITS.wrapping_add((n * 28) as u32 + 4), 0);
                end(cpu, s);
                return;
            }
            let t = nested!(cpu, 0x0604_5fd8, super::aim::nearest(cpu, s, 0));
            cpu.write8(s + TARGET, t as u8);
            if t as u8 as i8 != -1 {
                cpu.write16(s + PHASE, HOMING);
            }
        }
        HOMING => home(cpu, s),
        _ => {}
    }
    let f = cpu.stack();
    cpu.write32(f, 0);
    cpu.write32(f + 4, B_SPEED);
    cpu.write32(f + 0x10, 0x1_0000);
    cpu.write32(f + 0x0c, 0x1_0000);
    cpu.write32(f + 0x14, 0);
    let a = cpu.read16(s + ANGLE);
    cpu.write16(f + 0x18, a);
    cpu.call(SL_TURN, &[f]);
    let (vx, vy) = (cpu.read32(f), cpu.read32(f + 4));
    let x = cpu.read32(s + POSITION).wrapping_add(vx);
    cpu.write32(s + POSITION, x);
    let y = cpu.read32(s + POSITION + 4).wrapping_add(vy);
    cpu.write32(s + POSITION + 4, y);
    let (vx, vy) = (cpu.read32(f), cpu.read32(f + 4));
    cpu.write32(s + SPEED, vx);
    cpu.write32(s + SPEED + 4, vy);
    let n = number(cpu, s);
    let level = cpu.read8(B_LEVEL.wrapping_add((n * 3) as u32)) as i8 as i32;
    let size = (level * 0x1f0 + 0xe000) as u32;
    let w = cpu.read32(s + SCALES);
    let w = nested!(cpu, 0x0604_6168, actors::approach(cpu, w, size, 0x2000));
    cpu.write32(s + SCALES, w);
    cpu.write32(s + SCALES + 4, w);
    nested!(cpu, 0x0604_6558, effects::link_step(cpu, s + SPRITE, s + B_LINK));
}

/// One frame homing: the nearest target again (lost, or ending, not to be homed on or dead: back
/// to seeking, the turn limit to its start); else turned toward it.
fn home(cpu: &mut Cpu, s: u32) {
    let t = nested!(cpu, 0x0604_5fd8, super::aim::nearest(cpu, s, 0));
    cpu.write8(s + TARGET, t as u8);
    let lost = |cpu: &mut Cpu| {
        cpu.write16(s + TURN, TURN_START);
        cpu.write16(s + PHASE, SEEKING);
    };
    if t as u8 as i8 == -1 {
        lost(cpu);
        return;
    }
    let n = cpu.read8(s + TARGET) as i8 as i32;
    let target = cpu.read32(NUMBERED.wrapping_add((n * 4) as u32));
    if cpu.read8(target + OBJ_MODE) as i8 >= ENDING as i8 || cpu.read32(target + FLAGS) & UNHOMED != 0 || cpu.read32(target + LIFE) as i32 <= 0 {
        lost(cpu);
        return;
    }
    let limit = (cpu.read16(s + TURN) as i16 as i32 + TURN_START as i32) as u32;
    let to = nested!(cpu, 0x0604_5e30, super::aim::angle_to(cpu, s, target)) as i16 as i32 as u32;
    let from = cpu.read16(s + ANGLE) as i16 as i32 as u32;
    let d = nested!(cpu, 0x0604_6194, super::camera::angle_diff(cpu, from, to));
    let d = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, d, limit));
    let a = cpu.read16(s + ANGLE).wrapping_add(d as u16);
    cpu.write16(s + ANGLE, a);
    let turn = cpu.read16(s + TURN) as i16 as i32 as u32;
    let turn = nested!(cpu, 0x0604_6168, actors::approach(cpu, turn, TURN_MOST, TURN_GROWTH));
    cpu.write16(s + TURN, turn as u16);
}

const B_EXEC: u32 = 0x0604_8acc;

/// B's shots from ship `p`: a pair at 5 pixels either side, turned out by an eighth of a turn,
/// their power by B's level; a muzzle flash. The second shot (or 0).
pub fn shot_b_set(cpu: &mut Cpu, p: u32) -> u32 {
    let mut last = 0;
    for (dx, angle) in [(0xfffb_0000u32, 0xe000u16), (0x5_0000, 0x2000)] {
        let s = shot_in(cpu);
        if s == 0 {
            return 0;
        }
        cpu.write32(s + BEHAVIOUR, B_EXEC);
        let x = cpu.read32(p + POSITION).wrapping_add(dx);
        cpu.write32(s + POSITION, x);
        let y = cpu.read32(p + POSITION + 4).wrapping_add(0xfff7_0000);
        cpu.write32(s + POSITION + 4, y);
        let z = cpu.read32(p + POSITION + 8);
        cpu.write32(s + POSITION + 8, z);
        cpu.write16(s + ANGLE, angle);
        let n = cpu.read8(p + NUMBER);
        cpu.write8(s + NUMBER, n);
        let level = cpu.read8(B_LEVEL.wrapping_add((n as i8 as i32 * 3) as u32)) as i8 as i32;
        cpu.write16(s + POWER, (level * 6 + 60) as u16);
        cpu.write32(s + SHIP, p);
        last = s;
    }
    muzzle(cpu, p, B_COLOUR, MUZZLE, 0);
    last
}

/// A muzzle flash at the ship (its scales, its angle turned by `turn`), kind 3's colour plus
/// `colour`, `animation` (effect_set's).
fn muzzle(cpu: &mut Cpu, p: u32, colour: u16, animation: u32, turn: u16) {
    let f = cpu.stack();
    for k in 0..3 {
        let v = cpu.read32(p + POSITION + 4 * k);
        cpu.write32(f + 4 * k, v);
    }
    for k in 0..2 {
        let v = cpu.read32(p + SCALES + 4 * k);
        cpu.write32(f + 0x0c + 4 * k, v);
    }
    cpu.write32(f + 0x14, 3);
    let a = cpu.read16(p + ANGLE).wrapping_add(turn);
    cpu.write16(f + 0x18, a);
    cpu.write16(f + 0x24, 0);
    cpu.write8(f + 0x26, 0);
    cpu.write8(f + 0x1c, A_KIND);
    cpu.write8(f + 0x1d, 0);
    let c = cpu.read16(MUZZLE_COLOUR).wrapping_add(colour);
    cpu.write16(f + 0x20, c);
    let n = cpu.read8(p + NUMBER);
    cpu.write8(f + 0x1e, n);
    cpu.write32(f + 0x28, animation);
    cpu.write32(f + 0x3c, 0);
    nested!(cpu, 0x0604_e1cc, effects::effect_set(cpu, f));
}

/// The ship's fields the makers read: shots fired in the state, scales, angle.
const SHOTS: u32 = 0x89;
/// A's side shots' angle tables (by count: two, four).
const SIDES_2: u32 = 0x0607_2932;
const SIDES_4: u32 = 0x0607_2936;
/// The muzzle flash (effect_set's: kind 3's colour, its animation).
const MUZZLE: u32 = 0x0607_2918;
const MUZZLE_COLOUR: u32 = 0x0607_422c;
const A_EXEC: u32 = 0x0604_8318;
const A_SIDE_EXEC: u32 = 0x0604_8480;

fn level_a(cpu: &mut Cpu, p: u32) -> i32 {
    let n = number(cpu, p);
    cpu.read8(LEVELS.wrapping_add((n * 3) as u32)) as i8 as i32
}

/// A main shot from ship `p` at `dx` from it: its size and power by A's level.
fn main_shot(cpu: &mut Cpu, p: u32, dx: u32) -> u32 {
    let s = shot_in(cpu);
    if s == 0 {
        return 0;
    }
    cpu.write32(s + BEHAVIOUR, A_EXEC);
    let x = cpu.read32(p + POSITION).wrapping_add(dx);
    cpu.write32(s + POSITION, x);
    let y = cpu.read32(p + POSITION + 4).wrapping_add(0xfff7_0000);
    cpu.write32(s + POSITION + 4, y);
    let z = cpu.read32(p + POSITION + 8);
    cpu.write32(s + POSITION + 8, z);
    let n = cpu.read8(p + NUMBER);
    cpu.write8(s + NUMBER, n);
    for k in 0..2 {
        let size = (level_a(cpu, p) * 0x1f0 + 0xe000) as u32;
        cpu.write32(s + SCALES + 4 * k, size);
    }
    let power = level_a(cpu, p) * 10 + 100;
    cpu.write16(s + POWER, power as u16);
    s
}

/// A's shot from ship `p`: the first of a burst one main shot of double power; the others a pair
/// of main shots and, by A's level, side shots (two each 4th, 3rd or 2nd shot from level 3, 5, 7;
/// two always from 9; then four each 4th, 3rd, 2nd from 12, 15, 18, always from 21), and a muzzle
/// flash. The last side shot made (or the main shot, or 0).
pub fn shot_a_set(cpu: &mut Cpu, p: u32) -> u32 {
    if cpu.read8(p + SHOTS) == 0 {
        let s = main_shot(cpu, p, 0);
        if s == 0 {
            return 0;
        }
        let power = cpu.read16(s + POWER).wrapping_mul(2);
        cpu.write16(s + POWER, power);
        cpu.write32(s + SHIP, p);
        return s;
    }
    let frame = cpu.stack();
    let s = main_shot(cpu, p, 0xfffb_0000);
    if s == 0 {
        return 0;
    }
    // the SH-2 code keeps the ship field's offset in its frame
    cpu.write32(frame + 0x40, SHIP);
    cpu.write32(s + SHIP, p);
    let s = main_shot(cpu, p, 0x5_0000);
    if s == 0 {
        return 0;
    }
    cpu.write32(s + SHIP, p);
    let level = level_a(cpu, p);
    let every = |cpu: &mut Cpu, n: u32| {
        let fired = cpu.read8(p + SHOTS) as u32;
        nested!(cpu, 0x0600_6a6e, math::remainder(cpu, fired, n)) == 1
    };
    let (count, table) = match level {
        ..=2 => (0, 0),
        3..=4 => (if every(cpu, 4) { 2 } else { 0 }, SIDES_2),
        5..=6 => (if every(cpu, 3) { 2 } else { 0 }, SIDES_2),
        7..=8 => (if every(cpu, 2) { 2 } else { 0 }, SIDES_2),
        9..=11 => (2, SIDES_2),
        12..=14 => (if every(cpu, 4) { 4 } else { 2 }, SIDES_4),
        15..=17 => (if every(cpu, 3) { 4 } else { 2 }, SIDES_4),
        18..=20 => (if every(cpu, 2) { 4 } else { 2 }, SIDES_4),
        _ => (4, SIDES_4),
    };
    let mut last = 0;
    for i in 0..count {
        let s = shot_in(cpu);
        last = s;
        if s == 0 {
            break;
        }
        cpu.write32(s + BEHAVIOUR, A_SIDE_EXEC);
        cpu.write32(s + SIDE_FRAMES, i as u32);
        let angle = cpu.read16(table + 2 * i as u32);
        cpu.write16(s + ANGLE, angle);
        let out = ((i / 2 + 1) * 0x8_0000 + 0x5_0000) as u32;
        let x = cpu.read32(p + POSITION);
        let x = if (angle as i16) < 0 { x.wrapping_sub(out) } else { x.wrapping_add(out) };
        cpu.write32(s + POSITION, x);
        cpu.write16(s + ANGLE, 0);
        let y = cpu.read32(p + POSITION + 4).wrapping_add(0xfff7_0000).wrapping_add(((i / 2 + 1) * 0x8_0000) as u32);
        cpu.write32(s + POSITION + 4, y);
        let z = cpu.read32(p + POSITION + 8);
        cpu.write32(s + POSITION + 8, z);
        let n = cpu.read8(p + NUMBER);
        cpu.write8(s + NUMBER, n);
        let power = (level_a(cpu, p) * 10 + 100) as i16;
        cpu.write16(s + POWER, power as u16);
        let power = cpu.read16(s + POWER) as i16;
        cpu.write16(s + POWER, (power / 2) as u16);
        let w = (level_a(cpu, p) * 0xf8 + 0x8000) as u32;
        cpu.write32(s + SCALES, w);
        let h = (level_a(cpu, p) * 0xf8 + 0xc000) as u32;
        cpu.write32(s + SCALES + 4, h);
        cpu.write32(s + SHIP, p);
    }
    muzzle(cpu, p, 0, MUZZLE, 0);
    last
}

/// C's shot (where a bolt bursts): kind 3 in its own colour, type 9 (C), from an eighth of the
/// size growing to its size by C's level (hitting everything again each frame), then at half
/// power shrinking ever faster away (hitting everything again each time its sizes summed reach
/// twice its size; counted in its ship while bigger than 0x6000); its lift flickers.
const C_ANIMATION: u32 = 0x0607_28e0;
const C_TYPE: u8 = 9;
const C_BOXES: u32 = 0x0607_282c;
const C_START: u32 = 0x2000;
const C_LINK: u32 = 0x90;
const C_SIZE: u32 = 0x70;
const C_SHRINK: u32 = 0x74;
const C_SUMMED: u32 = 0x78;
const C_SHRINK_MOST: u32 = 0x800;
const C_SHRINK_GROWTH: u32 = 0x40;
const C_COUNTED: i32 = 0x6000;
const GROWING: u16 = 1;
const SHRINKING: u16 = 2;
/// The set of things a shot may hit (hits.rs's: two longs of bits).
const HIT_SET: u32 = 0x48;
/// The ship's C shots alive.
const ALIVE_C: u32 = 0x82;
const LEVEL_C: u32 = LEVELS + 2;
const GAME_FRAMES: u32 = super::globals::GAME_FRAMES;

fn hit_set(cpu: &mut Cpu, s: u32, set: u32) {
    cpu.write32(s + HIT_SET, set);
    cpu.write32(s + HIT_SET + 4, set);
}

/// Counts the shot alive in its ship (if it has one): in `field` and in all.
fn count_alive(cpu: &mut Cpu, s: u32, field: u32) {
    let ship = cpu.read32(s + SHIP);
    if ship != 0 {
        for field in [field, ALIVE_ALL] {
            let n = cpu.read8(ship + field).wrapping_add(1);
            cpu.write8(ship + field, n);
        }
    }
}

/// A burst growing (C's shot, B+C's): hitting everything again, an eighth of the way (and 0x200) to
/// its size; there, its sizes summed (at `summed`) and shrink cleared, its power halved: true.
fn burst_grown(cpu: &mut Cpu, s: u32, summed: u32) -> bool {
    hit_set(cpu, s, u32::MAX);
    let size = cpu.read32(s + SCALES) as i32;
    let to = cpu.read32(s + C_SIZE) as i32;
    let size = size.wrapping_add(to.wrapping_sub(size.wrapping_sub(0x200)) / 8);
    cpu.write32(s + SCALES, size as u32);
    if size < cpu.read32(s + C_SIZE) as i32 {
        return false;
    }
    cpu.write32(s + summed, 0);
    cpu.write32(s + C_SHRINK, 0);
    let power = cpu.read16(s + POWER) as i16;
    cpu.write16(s + POWER, (power / 2) as u16);
    true
}

/// A burst shrinking: its sizes summed (hitting everything again each time they reach twice its
/// size), shrinking ever faster away; ended once gone.
fn burst_shrinks(cpu: &mut Cpu, s: u32, summed: u32) {
    let sum = cpu.read32(s + summed).wrapping_add(cpu.read32(s + SCALES));
    cpu.write32(s + summed, sum);
    let twice = cpu.read32(s + C_SIZE).wrapping_mul(2);
    if sum as i32 >= twice as i32 {
        hit_set(cpu, s, u32::MAX);
        cpu.write32(s + summed, sum.wrapping_sub(twice));
    }
    let step = cpu.read32(s + C_SHRINK);
    let step = nested!(cpu, 0x0604_6168, actors::approach(cpu, step, C_SHRINK_MOST, C_SHRINK_GROWTH));
    cpu.write32(s + C_SHRINK, step);
    let size = cpu.read32(s + SCALES);
    let size = nested!(cpu, 0x0604_6168, actors::approach(cpu, size, 0, step));
    cpu.write32(s + SCALES, size);
    if size as i32 <= 0 {
        end(cpu, s);
    }
}

pub fn shot_c_exec(cpu: &mut Cpu, s: u32) {
    let ship = cpu.read32(s + SHIP);
    if cpu.read8(ship + OBJ_MODE) as i8 >= ENDING as i8 {
        cpu.write32(s + SHIP, 0);
    }
    let phase = cpu.read8(s + PHASE_LOW) as i8;
    match phase {
        0 | 1 => {
            if phase == 0 {
                cpu.write8(s + LIFT, 0);
                cpu.write32(s + SCALES + 4, C_START);
                cpu.write32(s + SCALES, C_START);
                cpu.write8(s + KIND, A_KIND);
                nested!(cpu, 0x0604_6530, effects::link_init(cpu, s + C_LINK, C_ANIMATION));
                let colour = cpu.read16(s + OWN_COLOUR);
                cpu.write16(s + COLOUR, colour);
                cpu.write8(s + SIDE, 0);
                cpu.write8(s + TYPE, C_TYPE);
                cpu.write32(s + BOXES, C_BOXES);
                cpu.write32(s + SPEED + 4, 0);
                cpu.write32(s + SPEED, 0);
                let n = number(cpu, s);
                let level = cpu.read8(LEVEL_C.wrapping_add((n * 3) as u32)) as i8 as i32;
                cpu.write32(s + C_SIZE, (level * 0x2e8 + 0xc000) as u32);
                cpu.write16(s + PHASE, GROWING);
            }
            count_alive(cpu, s, ALIVE_C);
            if burst_grown(cpu, s, C_SUMMED) {
                cpu.write16(s + TURN, 0);
                cpu.write16(s + PHASE, SHRINKING);
            }
        }
        2 => {
            if cpu.read32(s + SCALES) as i32 > C_COUNTED {
                count_alive(cpu, s, ALIVE_C);
            }
            burst_shrinks(cpu, s, C_SUMMED);
        }
        _ => {}
    }
    let lift = if cpu.read32(GAME_FRAMES) & 1 != 0 { 0 } else { 0xf7 };
    cpu.write8(s + LIFT, lift);
    let w = cpu.read32(s + SCALES);
    cpu.write32(s + SCALES + 4, w);
    nested!(cpu, 0x0604_6558, effects::link_step(cpu, s + SPRITE, s + C_LINK));
}

/// C's bolt: kind 3 in the kind's colour plus 0x20, type 10, 6 pixels a frame along its angle,
/// growing to full size, hitting nothing (its set empty) and counted in its ship; it bursts (into
/// C's shot, then ends) where it hits something or the ground, or once its ship lets go of C (its
/// state not C's: C's hit count cleared); ended off screen (C's hit count cleared). It leaves a
/// trail.
const BOLT_ANIMATION: u32 = 0x0607_28d0;
const BOLT_TYPE: u8 = 10;
const BOLT_BOXES: u32 = 0x0607_2834;
const BOLT_COLOUR: u16 = 0x20;
const BOLT_LINK: u32 = 0x90;
const FLYING: u16 = 1;
const BURSTING: u16 = 2;
/// The ship's state (player.rs's), C's.
const SHIP_STATE: u32 = 0x6d;
const STATE_C: u8 = 6;
/// The flags' flip bits.
const FLIP: u32 = 0x30;
const C_EXEC: u32 = 0x0604_8f30;

fn clear_c_hits(cpu: &mut Cpu, s: u32) {
    let n = number(cpu, s);
    cpu.write32(WEAPON_HITS.wrapping_add((n * 28) as u32 + 8), 0);
}

pub fn bolt_exec(cpu: &mut Cpu, s: u32) {
    let ship = cpu.read32(s + SHIP);
    if cpu.read8(ship + SHIP_STATE) != STATE_C {
        clear_c_hits(cpu, s);
        cpu.write16(s + PHASE, BURSTING);
    }
    let ship = cpu.read32(s + SHIP);
    if cpu.read8(ship + OBJ_MODE) as i8 >= ENDING as i8 {
        cpu.write32(s + SHIP, 0);
    }
    let phase = cpu.read8(s + PHASE_LOW) as i8;
    match phase {
        0 | 1 => {
            if phase == 0 {
                bolt_start(cpu, s);
            }
            let (x, y) = (cpu.read32(s + POSITION) as i32, cpu.read32(s + POSITION + 4) as i32);
            if x.wrapping_abs() > WIDE_EDGE_X || y.wrapping_abs() > EDGE_Y {
                clear_c_hits(cpu, s);
                end(cpu, s);
            } else {
                let hit = cpu.read16(s + HIT_FLAGS) & HIT_SOMETHING != 0 || {
                    let (x, y) = (cpu.read32(s + POSITION), cpu.read32(s + POSITION + 4));
                    nested!(cpu, 0x0605_ef90, bg::map_hit(cpu, x, y)) != 0
                };
                if hit {
                    hit_set(cpu, s, 0);
                    cpu.write16(s + PHASE, BURSTING);
                }
                count_alive(cpu, s, ALIVE_C);
                for k in 0..2 {
                    let p = cpu.read32(s + POSITION + 4 * k).wrapping_add(cpu.read32(s + SPEED + 4 * k));
                    cpu.write32(s + POSITION + 4 * k, p);
                }
                let w = cpu.read32(s + SCALES) as i32;
                let w = w.wrapping_add(0x1_0000i32.wrapping_sub(w) / 8);
                cpu.write32(s + SCALES, w as u32);
                cpu.write32(s + SCALES + 4, w as u32);
            }
        }
        2 => {
            let c = shot_in(cpu);
            if c != 0 {
                cpu.write32(c + BEHAVIOUR, C_EXEC);
                for k in 0..3 {
                    let v = cpu.read32(s + POSITION + 4 * k);
                    cpu.write32(c + POSITION + 4 * k, v);
                }
                let n = cpu.read8(s + NUMBER);
                cpu.write8(c + NUMBER, n);
                let colour = cpu.read16(s + OWN_COLOUR);
                cpu.write16(c + OWN_COLOUR, colour);
                let level = cpu.read8(LEVEL_C.wrapping_add((n as i8 as i32 * 3) as u32)) as i8 as i32;
                cpu.write16(c + POWER, (level * 2 + 20) as u16);
                let ship = cpu.read32(s + SHIP);
                cpu.write32(c + SHIP, ship);
                end(cpu, s);
            }
        }
        _ => {}
    }
    nested!(cpu, 0x0604_6558, effects::link_step(cpu, s + SPRITE, s + BOLT_LINK));
}

/// The bolt's start: its look, its speed (turned to its angle and flip), its trail.
fn bolt_start(cpu: &mut Cpu, s: u32) {
    cpu.write32(s + SCALES + 4, 0);
    cpu.write32(s + SCALES, 0);
    cpu.write8(s + KIND, A_KIND);
    nested!(cpu, 0x0604_6530, effects::link_init(cpu, s + BOLT_LINK, BOLT_ANIMATION));
    let kind = cpu.read8(s + KIND) as i8 as i32;
    let colour = cpu.read16(KIND_COLOURS.wrapping_add((kind * 16) as u32)).wrapping_add(BOLT_COLOUR);
    cpu.write16(s + OWN_COLOUR, colour);
    cpu.write16(s + COLOUR, colour);
    cpu.write8(s + LIFT, 0);
    cpu.write8(s + SIDE, 0);
    cpu.write8(s + TYPE, BOLT_TYPE);
    cpu.write32(s + BOXES, BOLT_BOXES);
    let f = cpu.stack();
    cpu.write32(f, 0);
    cpu.write32(f + 4, B_SPEED);
    cpu.write32(f + 0x10, 0x1_0000);
    cpu.write32(f + 0x0c, 0x1_0000);
    let flip = cpu.read32(s + FLAGS) & FLIP;
    cpu.write32(f + 0x14, flip);
    let a = cpu.read16(s + ANGLE);
    cpu.write16(f + 0x18, a);
    cpu.call(SL_TURN, &[f]);
    let (vx, vy) = (cpu.read32(f), cpu.read32(f + 4));
    cpu.write32(s + SPEED, vx);
    cpu.write32(s + SPEED + 4, vy);
    cpu.write16(s + TURN, 0);
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, TRAIL_SIZE)) as i8 as i32 as u32;
    let t = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 1, blocks, PLAIN_EXEC));
    if t != 0 {
        nested!(cpu, 0x0604_dd50, effects::actor_init(cpu, t));
        let flags = cpu.read32(t + FLAGS) | SPRITE_POLYGON;
        cpu.write32(t + FLAGS, flags);
        for k in 0..3 {
            let v = cpu.read32(s + POSITION + 4 * k);
            cpu.write32(t + POSITION + 4 * k, v);
        }
        let a = cpu.read16(s + ANGLE);
        cpu.write16(t + ANGLE, a);
        cpu.write32(t + BEHAVIOUR, TRAIL_EXEC);
        cpu.write8(t + KIND, A_KIND);
        let colour = cpu.read16(s + OWN_COLOUR);
        cpu.write16(t + COLOUR, colour);
        let n = cpu.read8(s + NUMBER);
        cpu.write8(t + NUMBER, n);
        cpu.write8(t + LIFT, 0xf6);
        cpu.write32(t + CORNERS_AT, t + CORNERS);
        cpu.write32(t + TRAIL_OF, s);
    }
    cpu.write16(s + PHASE, FLYING);
}

/// The bolt's trail (a plain actor): a polygon of its character's own size (sprites.rs's) from the
/// bolt back the way it came, as long as the distance it has come, shortening by a sixteenth each
/// frame; at the bolt (and its width) while it flies, shortening away once it has ended.
const TRAIL_SIZE: u32 = 0x80;
const PLAIN_EXEC: u32 = 0x0604_dcb4;
const TRAIL_EXEC: u32 = 0x0604_9160;
const TRAIL_ANIMATION: u32 = 0x0607_28d8;
const SPRITE_POLYGON: u32 = 4;
const CORNERS_AT: u32 = 0x34;
const TRAIL_PHASE: u32 = 0x3c;
const TRAIL_40: u32 = 0x40;
const TRAIL_LENGTH: u32 = 0x48;
const TRAIL_OF: u32 = 0x5c;
const TRAIL_LINK: u32 = 0x60;
/// Its four corners (x, y words: pixels): the top's two, the bottom's two.
const CORNERS: u32 = 0x70;
/// Its length while it shortens away (16.16), below which it ends.
const TRAIL_GONE: i32 = 0xf_ffff;
/// The kinds (sprites.rs's: their frames, their first character) and each character's size.
const KINDS: u32 = 0x0607_41f0;
const CHAR_SIZES: u32 = 0x0600_cc70;

fn shorten(cpu: &mut Cpu, o: u32) -> i32 {
    let len = cpu.read32(o + TRAIL_LENGTH) as i32;
    let len = len.wrapping_add(len.wrapping_neg() / 16);
    cpu.write32(o + TRAIL_LENGTH, len as u32);
    len
}

pub fn trail_exec(cpu: &mut Cpu, o: u32) {
    let bolt = cpu.read32(o + TRAIL_OF);
    if cpu.read8(o + TRAIL_PHASE) as i8 <= 1 && cpu.read8(bolt + OBJ_MODE) as i8 >= ENDING as i8 {
        cpu.write16(o + TRAIL_40, 0);
        cpu.write8(o + TRAIL_PHASE, 2);
    }
    let phase = cpu.read8(o + TRAIL_PHASE) as i8;
    match phase {
        0 | 1 => {
            if phase == 0 {
                nested!(cpu, 0x0604_6530, effects::link_init(cpu, o + TRAIL_LINK, TRAIL_ANIMATION));
                for field in [2, 6, 0xe, 0xa] {
                    cpu.write16(o + CORNERS + field, 0);
                }
                cpu.write32(o + TRAIL_LENGTH, 0);
                cpu.write8(o + TRAIL_PHASE, 1);
            }
            let len = shorten(cpu, o);
            let d = nested!(cpu, 0x0604_5ee4, super::aim::distance(cpu, o, bolt));
            cpu.write32(o + TRAIL_LENGTH, (len as u32).wrapping_add(d));
            for field in [POSITION, POSITION + 4, POSITION + 8, SCALES] {
                let v = cpu.read32(bolt + field);
                cpu.write32(o + field, v);
            }
            let a = cpu.read16(bolt + ANGLE);
            cpu.write16(o + ANGLE, a);
        }
        2 => {
            if shorten(cpu, o) <= TRAIL_GONE {
                end(cpu, o);
            }
        }
        _ => {}
    }
    nested!(cpu, 0x0604_6558, effects::link_step(cpu, o + SPRITE, o + TRAIL_LINK));
    let kind = KINDS.wrapping_add((cpu.read8(o + KIND) as i8 as i32 * 16) as u32);
    let frames = cpu.read32(kind + 4);
    let n = cpu.read8(o + FRAME) as u32;
    let frame = cpu.read32(frames + 8 * n);
    let ch = cpu.read16(kind + 8).wrapping_add(cpu.read8(frame) as u16) as i16 as i32;
    let w = cpu.read16(CHAR_SIZES.wrapping_add((ch * 8) as u32)) as i32;
    let left = (-w / 2) as u16;
    cpu.write16(o + CORNERS + 0xc, left);
    cpu.write16(o + CORNERS, left);
    let right = ((w >> 1) - 1) as u16;
    cpu.write16(o + CORNERS + 8, right);
    cpu.write16(o + CORNERS + 4, right);
    let len = (cpu.read32(o + TRAIL_LENGTH) as i32 / 0x1_0000) as u16;
    cpu.write16(o + CORNERS + 0xe, len);
    cpu.write16(o + CORNERS + 0xa, len);
}

const BOLT_EXEC: u32 = 0x0604_9318;
/// The bolts from the ship (x, y, the angle turned by).
const BOLTS: [(u32, u32, u16); 3] = [(0xfffb_0000, 0xfff7_0000, 0xd000), (0x5_0000, 0xfff7_0000, 0x3000), (0, 0x9_0000, 0x8000)];
/// C's muzzle flashes (effect_set's: kind 3's colour plus 0x20, their animation).
const C_MUZZLE: u32 = 0x0607_2910;

/// C's bolts from ship `p`: a pair at 5 pixels either side turned out by 3/16 of a turn, and a
/// third straight back unless the BIOS's setting is 1 (with it the pads keep C: pads.rs); their
/// power by C's level; a muzzle flash 3 pixels either side, turned out the same. The last bolt (or
/// 0).
pub fn shot_c_set(cpu: &mut Cpu, p: u32) -> u32 {
    let count = if super::globals::bios_setting_is_1(cpu) { 2 } else { 3 };
    let mut last = 0;
    for &(dx, dy, angle) in &BOLTS[..count] {
        let s = shot_in(cpu);
        if s == 0 {
            return 0;
        }
        cpu.write32(s + BEHAVIOUR, BOLT_EXEC);
        let x = cpu.read32(p + POSITION).wrapping_add(dx);
        cpu.write32(s + POSITION, x);
        let y = cpu.read32(p + POSITION + 4).wrapping_add(dy);
        cpu.write32(s + POSITION + 4, y);
        let z = cpu.read32(p + POSITION + 8);
        cpu.write32(s + POSITION + 8, z);
        let a = cpu.read16(p + ANGLE).wrapping_add(angle);
        cpu.write16(s + ANGLE, a);
        let n = cpu.read8(p + NUMBER);
        cpu.write8(s + NUMBER, n);
        let level = cpu.read8(LEVEL_C.wrapping_add((n as i8 as i32 * 3) as u32)) as i8 as i32;
        cpu.write16(s + POWER, (level * 2 + 20) as u16);
        cpu.write32(s + SHIP, p);
        last = s;
    }
    let f = cpu.stack();
    for k in 0..3 {
        let v = cpu.read32(p + POSITION + 4 * k);
        cpu.write32(f + 4 * k, v);
    }
    let y = cpu.read32(p + POSITION + 4).wrapping_add(0xfffb_0000);
    cpu.write32(f + 4, y);
    for k in 0..2 {
        let v = cpu.read32(p + SCALES + 4 * k);
        cpu.write32(f + 0x0c + 4 * k, v);
    }
    cpu.write32(f + 0x14, 3);
    cpu.write32(f + 0x3c, 0);
    cpu.write16(f + 0x24, 0);
    cpu.write8(f + 0x26, 0);
    cpu.write8(f + 0x1c, A_KIND);
    cpu.write8(f + 0x1d, 0);
    let c = cpu.read16(MUZZLE_COLOUR).wrapping_add(BOLT_COLOUR);
    cpu.write16(f + 0x20, c);
    let n = cpu.read8(p + NUMBER);
    cpu.write8(f + 0x1e, n);
    cpu.write32(f + 0x28, C_MUZZLE);
    for (dx, angle) in [(0x3_0000u32, 0xd000u16), (0xfffd_0000, 0x3000)] {
        let x = cpu.read32(p + POSITION).wrapping_add(dx);
        cpu.write32(f, x);
        let a = cpu.read16(p + ANGLE).wrapping_add(angle);
        cpu.write16(f + 0x18, a);
        nested!(cpu, 0x0604_e1cc, effects::effect_set(cpu, f));
    }
    last
}

/// A+B's shots (the lock-on plasma): two seekers (plain actors), each held at the ship's nose 5
/// pixels to its side (the second's flags mirror it) and drawn as a line ahead, never culled,
/// whose reach grows toward 256 pixels. After 20 frames it sweeps out to its side and back (by up
/// to 0x800 and 0x174 more each B level), its colour pulsing, until something to hit lies within
/// 8 pixels of the line and nearer than its reach (on the other side from the ship, alive, not
/// ending, not to be homed on): then it locks on (a lock ring there, the line's colour steady,
/// aimed at it within the sweep) and holds the nearest along the line no further than 16 pixels
/// beyond it (another: A+B's hit count cleared and a new ring); lost, it seeks again. Ended once
/// its ship is not in A+B's state.
const SEEKER_SIZE: u32 = 0x80;
const SEEKER_EXEC: u32 = 0x0604_a29c;
/// The seeker's phase, frames before it sweeps, its ship, target's number (a byte), sweep's step,
/// reach (16.16), lock ring; the line's corners (x, y words: the base, the end).
const SEEKER_PHASE: u32 = 0x3c;
const SEEKER_WAIT: u32 = 0x3e;
const SEEKER_SHIP: u32 = 0x5c;
const SEEKER_TARGET: u32 = 0x60;
const SEEKER_STEP: u32 = 0x61;
const SEEKER_REACH: u32 = 0x64;
const SEEKER_LOCK: u32 = 0x68;
const SEEKER_CORNERS: u32 = 0x70;
/// The flags: a line (two corners), never culled; mirrored (flipped).
const LINE_FLAGS: u32 = 0x2000_0003;
const MIRRORED: u32 = 0x10;
const SEEKER_WAITS: u8 = 20;
const SEEKER_REACH_MOST: i32 = 0x100_0000;
const SEEKER_LOCK_BEYOND: u32 = 0x10_0000;
const LINE_HALF_WIDTH: i32 = 0x8_0000;
/// The first kind's colour (the line's: plus 0x2a pulsing by 4 while it seeks, 0x18 locked on).
const SEEKING_COLOUR: u16 = 0x2a;
const LOCKED_COLOUR: u16 = 0x18;
const STATE_AB: u8 = 7;
const LOCK_EXEC: u32 = 0x0604_a0f8;
/// SGL's.
const SL_COS: u32 = 0x0606_9a30;

/// The frame's spills: the angle to the nearest thing found, the sweep, the ship.
const F_FOUND_ANGLE: u32 = 0x40;
const F_SWEEP: u32 = 0x44;
const F_SHIP: u32 = 0x48;

pub fn seeker_exec(cpu: &mut Cpu, o: u32) {
    let f = cpu.stack();
    let ship = cpu.read32(o + SEEKER_SHIP);
    cpu.write32(f + F_SHIP, ship);
    if cpu.read8(ship + SHIP_STATE) != STATE_AB || cpu.read8(ship + OBJ_MODE) as i8 >= ENDING as i8 {
        end(cpu, o);
        return;
    }
    cpu.write32(f, 0x5_0000);
    cpu.write32(f + 4, 0xfff7_0000);
    for k in 0..2 {
        let v = cpu.read32(ship + SCALES + 4 * k);
        cpu.write32(f + 0x0c + 4 * k, v);
    }
    let flip = (cpu.read32(o + FLAGS) ^ cpu.read32(ship + FLAGS)) & FLIP;
    cpu.write32(f + 0x14, flip);
    let a = cpu.read16(ship + ANGLE);
    cpu.write16(f + 0x18, a);
    cpu.call(SL_TURN, &[f]);
    for k in 0..2 {
        let p = cpu.read32(ship + POSITION + 4 * k).wrapping_add(cpu.read32(f + 4 * k));
        cpu.write32(o + POSITION + 4 * k, p);
    }
    let z = cpu.read32(ship + POSITION + 8);
    cpu.write32(o + POSITION + 8, z);
    let n = number(cpu, o);
    let level = cpu.read8(B_LEVEL.wrapping_add((n * 3) as u32)) as i8 as i32;
    let sweep = if cpu.read32(o + FLAGS) & MIRRORED == 0 { level * 0x174 + 0x800 } else { -0x800 - level * 0x174 } as i16 as i32;
    cpu.write32(f + F_SWEEP, sweep as u32);
    let phase = cpu.read8(o + SEEKER_PHASE);
    if phase <= 2 {
        if phase == 0 {
            let flags = cpu.read32(o + FLAGS) | LINE_FLAGS;
            cpu.write32(o + FLAGS, flags);
            cpu.write8(o + LIFT, 0);
            for k in [6, 4, 2, 0] {
                cpu.write16(o + SEEKER_CORNERS + k, 0);
            }
            cpu.write32(o + CORNERS_AT, o + SEEKER_CORNERS);
            cpu.write8(o + SEEKER_TARGET, 0xff);
            cpu.write8(o + SEEKER_PHASE, 1);
        }
        if phase <= 1 {
            cpu.write8(o + SEEKER_WAIT, SEEKER_WAITS);
            cpu.write32(o + SEEKER_REACH, 0);
            cpu.write8(o + SEEKER_STEP, 0);
            cpu.write8(o + SEEKER_PHASE, 2);
        }
        seek(cpu, o, ship, sweep);
    } else if phase <= 4 {
        if phase == 3 {
            let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, SEEKER_SIZE)) as i8 as i32 as u32;
            let r = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 1, blocks, PLAIN_EXEC));
            if r != 0 {
                nested!(cpu, 0x0604_dd50, effects::actor_init(cpu, r));
                let flags = cpu.read32(r + FLAGS) | cpu.read32(o + FLAGS) & FLIP;
                cpu.write32(r + FLAGS, flags);
                cpu.write32(r + BEHAVIOUR, LOCK_EXEC);
                for k in 0..3 {
                    let v = cpu.read32(o + POSITION + 4 * k);
                    cpu.write32(r + POSITION + 4 * k, v);
                }
                let n = cpu.read8(o + NUMBER);
                cpu.write8(r + NUMBER, n);
                cpu.write32(r + LOCK_OF, o);
                let ship = cpu.read32(o + SEEKER_SHIP);
                cpu.write32(r + SEEKER_SHIP, ship);
                cpu.write8(o + SEEKER_PHASE, 4);
            }
            let colour = cpu.read16(KIND_COLOURS).wrapping_add(LOCKED_COLOUR);
            cpu.write16(o + COLOUR, colour);
            cpu.write32(o + SEEKER_LOCK, r);
        }
        hold(cpu, o, ship, sweep);
    }
    let reach = cpu.read32(o + SEEKER_REACH) as i32;
    cpu.write16(o + SEEKER_CORNERS + 6, (reach.wrapping_neg() / 0x1_0000) as u16);
}

/// Seeking: the reach grown, the sweep (after the wait), and the nearest within reach locked on.
fn seek(cpu: &mut Cpu, o: u32, ship: u32, sweep: i32) {
    let reach = cpu.read32(o + SEEKER_REACH) as i32;
    let reach = reach.wrapping_add(SEEKER_REACH_MOST.wrapping_sub(reach) / 8);
    cpu.write32(o + SEEKER_REACH, reach as u32);
    let wait = cpu.read8(o + SEEKER_WAIT) as i8;
    if wait > 0 {
        cpu.write8(o + SEEKER_WAIT, (wait - 1) as u8);
    } else {
        let step = cpu.read8(o + SEEKER_STEP).wrapping_add(1);
        cpu.write8(o + SEEKER_STEP, step);
    }
    let step = cpu.read8(o + SEEKER_STEP) as i8 as i32;
    let c = cpu.call(SL_COS, &[(step * 0x800) as i16 as i32 as u32]);
    let pulse = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, 4, c));
    let colour = (pulse as u16).wrapping_add(SEEKING_COLOUR).wrapping_add(cpu.read16(KIND_COLOURS));
    cpu.write16(o + COLOUR, colour);
    let f = cpu.stack();
    cpu.write16(f + 0x18, sweep as u16);
    let c = cpu.call(SL_COS, &[(step * 0x200) as i16 as i32 as u32]);
    let swing = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, sweep as u32, c));
    cpu.write16(o + ANGLE, (sweep as u16).wrapping_sub(swing as u16));
    let reach = cpu.read32(o + SEEKER_REACH) as i32;
    let (t, d) = nearest_ahead(cpu, o, ship, reach, false);
    if t != -1 {
        cpu.write32(o + SEEKER_REACH, d as u32);
        aim_within(cpu, o, sweep);
        cpu.write8(o + SEEKER_TARGET, t as u8);
        cpu.write8(o + SEEKER_PHASE, 3);
    }
}

/// Locked on: the nearest along the line no further than just beyond the target held (another:
/// A+B's hit count cleared, a new ring); none, seeking again.
fn hold(cpu: &mut Cpu, o: u32, ship: u32, sweep: i32) {
    let n = cpu.read8(o + SEEKER_TARGET) as i8 as i32;
    let target = cpu.read32(NUMBERED.wrapping_add((n * 4) as u32));
    let d = nested!(cpu, 0x0604_5ee4, super::aim::distance(cpu, o, target));
    let (t, d) = nearest_ahead(cpu, o, ship, d.wrapping_add(SEEKER_LOCK_BEYOND) as i32, true);
    if t == -1 {
        cpu.write32(o + SEEKER_LOCK, 0);
        cpu.write8(o + SEEKER_PHASE, 1);
    } else {
        cpu.write32(o + SEEKER_REACH, d as u32);
        aim_within(cpu, o, sweep);
        if t != cpu.read8(o + SEEKER_TARGET) as i8 {
            let n = number(cpu, o);
            cpu.write32(WEAPON_HITS.wrapping_add((n * 28) as u32 + 0xc), 0);
            cpu.write8(o + SEEKER_PHASE, 3);
        }
    }
    cpu.write8(o + SEEKER_TARGET, t as u8);
}

/// The nearest thing to hit along the seeker's line (within 8 pixels of it, ahead, on the other
/// side from `ship`, alive, not ending, not to be homed on) nearer than `reach` (or as near,
/// `or_as_near`): its number (or -1) and how far ahead; the angle to it spilled in the frame.
fn nearest_ahead(cpu: &mut Cpu, o: u32, ship: u32, mut reach: i32, or_as_near: bool) -> (i8, i32) {
    let f = cpu.stack();
    cpu.write32(f + 0x10, 0x1_0000);
    cpu.write32(f + 0x0c, 0x1_0000);
    cpu.write32(f + 0x14, 0);
    let a = cpu.read16(o + ANGLE).wrapping_neg();
    cpu.write16(f + 0x18, a);
    cpu.write32(f + F_FOUND_ANGLE, 0);
    let mut found = -1;
    for i in 2..=62u32 {
        let t = cpu.read32(NUMBERED + 4 * i);
        if t == 0
            || cpu.read8(t + SIDE) == cpu.read8(ship + SIDE)
            || cpu.read8(t + OBJ_MODE) as i8 >= ENDING as i8
            || cpu.read32(t + FLAGS) & UNHOMED != 0
            || cpu.read32(t + LIFE) as i32 <= 0
        {
            continue;
        }
        for k in 0..2 {
            let d = cpu.read32(t + POSITION + 4 * k).wrapping_sub(cpu.read32(o + POSITION + 4 * k));
            cpu.write32(f + 4 * k, d);
        }
        cpu.call(SL_TURN, &[f]);
        let (x, y) = (cpu.read32(f) as i32, cpu.read32(f + 4) as i32);
        if x.wrapping_abs() > LINE_HALF_WIDTH || y >= 0 {
            continue;
        }
        let ahead = y.wrapping_neg();
        if if or_as_near { ahead > reach } else { ahead >= reach } {
            continue;
        }
        let to = nested!(cpu, 0x0604_5e30, super::aim::angle_to(cpu, o, t)) as i16 as i32 as u32;
        cpu.write32(f + F_FOUND_ANGLE, to);
        reach = ahead;
        found = cpu.read8(t + NUMBER) as i8;
    }
    (found, reach)
}

/// The seeker turned to what it found, kept within its sweep.
fn aim_within(cpu: &mut Cpu, o: u32, sweep: i32) {
    let f = cpu.stack();
    let to = cpu.read32(f + F_FOUND_ANGLE);
    let range = sweep.wrapping_abs() as i16 as i32 as u32;
    let a = nested!(cpu, 0x0604_6214, super::camera::angle_within(cpu, to, sweep as u32, range));
    cpu.write16(o + ANGLE, a as u16);
}

/// The lock ring (a plain actor; kind 3, the kind's colour plus 0x20, unturned, lifted 4): it
/// grows to full size at its seeker's target while the seeker holds it with this ring, making the
/// plasma beam (until one is made); then it shrinks away.
const LOCK_ANIMATION: u32 = 0x0607_28fc;
const LOCK_PHASE: u32 = 0x3c;
const LOCK_OF: u32 = 0x64;
const LOCK_COLOUR: u16 = 0x20;
const LOCK_FADING: u8 = 100;
const LOCK_GONE: i32 = 0xff;
const UNTURNED: u32 = 0x1000_0000;
/// The plasma beam (an actor list's actor, with hits: below).
const BEAM_SIZE: u32 = 0x198;
const BEAM_EXEC: u32 = 0x0604_9c54;

pub fn lock_exec(cpu: &mut Cpu, r: u32) {
    let phase = cpu.read8(r + LOCK_PHASE) as i8;
    let seeker = cpu.read32(r + LOCK_OF);
    match phase {
        0 | 1 | 2 => {
            if phase == 0 {
                cpu.write32(r + SCALES, 0);
                let flags = cpu.read32(r + FLAGS) | UNTURNED;
                cpu.write32(r + FLAGS, flags);
                cpu.write8(r + KIND, A_KIND);
                nested!(cpu, 0x0604_6530, effects::link_init(cpu, r + 0x70, LOCK_ANIMATION));
                let kind = cpu.read8(r + KIND) as i8 as i32;
                let colour = cpu.read16(KIND_COLOURS.wrapping_add((kind * 16) as u32)).wrapping_add(LOCK_COLOUR);
                cpu.write16(r + COLOUR, colour);
                cpu.write8(r + LIFT, 4);
                cpu.write8(r + LOCK_PHASE, 1);
            }
            if phase <= 1 {
                let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, BEAM_SIZE)) as i8 as i32 as u32;
                let b = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 1, blocks, ACTOR_EXEC));
                if b != 0 {
                    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, b + SPRITE));
                    nested!(cpu, 0x0604_6284, super::hits::hit_init(cpu, b + BEAM_HITS));
                    cpu.write8(b + BEAM_PHASE, 0);
                    cpu.write32(b + BEHAVIOUR, BEAM_EXEC);
                    for k in 0..3 {
                        let v = cpu.read32(r + POSITION + 4 * k);
                        cpu.write32(b + POSITION + 4 * k, v);
                    }
                    let n = cpu.read8(r + NUMBER);
                    cpu.write8(b + NUMBER, n);
                    let seeker = cpu.read32(r + LOCK_OF);
                    cpu.write32(b + BEAM_SEEKER, seeker);
                    cpu.write32(b + BEAM_LOCK, r);
                    cpu.write32(b + BEAM_TARGET, 0);
                    cpu.write8(r + LOCK_PHASE, 2);
                }
            }
            let size = cpu.read32(r + SCALES) as i32;
            cpu.write32(r + SCALES, size.wrapping_add(0x1_0000i32.wrapping_sub(size) / 8) as u32);
            if cpu.read8(seeker + OBJ_MODE) as i8 >= ENDING as i8 || cpu.read32(seeker + SEEKER_LOCK) != r {
                cpu.write8(r + LOCK_PHASE, LOCK_FADING);
            } else {
                for k in 0..3 {
                    let v = cpu.read32(seeker + POSITION + 4 * k);
                    cpu.write32(r + POSITION + 4 * k, v);
                }
            }
        }
        p if p == LOCK_FADING as i8 => {
            let size = cpu.read32(r + SCALES) as i32;
            let size = size.wrapping_add(size.wrapping_neg() / 8);
            cpu.write32(r + SCALES, size as u32);
            if size <= LOCK_GONE {
                end(cpu, r);
            }
        }
        _ => {}
    }
    let w = cpu.read32(r + SCALES);
    cpu.write32(r + SCALES + 4, w);
    nested!(cpu, 0x0604_6558, effects::link_step(cpu, r + SPRITE, r + 0x70));
}

/// A+B's shot from ship `p`: its two seekers (the second mirrored). The second (or 0).
pub fn shot_ab_set(cpu: &mut Cpu, p: u32) -> u32 {
    let mut last = 0;
    for k in 0..2 {
        let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, SEEKER_SIZE)) as i8 as i32 as u32;
        let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 1, blocks, PLAIN_EXEC));
        if o == 0 {
            return 0;
        }
        nested!(cpu, 0x0604_dd50, effects::actor_init(cpu, o));
        cpu.write32(o + SEEKER_SHIP, p);
        cpu.write32(o + BEHAVIOUR, SEEKER_EXEC);
        for k in 0..3 {
            let v = cpu.read32(p + POSITION + 4 * k);
            cpu.write32(o + POSITION + 4 * k, v);
        }
        let n = cpu.read8(p + NUMBER);
        cpu.write8(o + NUMBER, n);
        if k == 1 {
            let flags = cpu.read32(o + FLAGS) | MIRRORED;
            cpu.write32(o + FLAGS, flags);
        }
        last = o;
    }
    last
}

/// The plasma beam (made by the lock ring): 16 segments of 32 pixels from its seeker toward the
/// target, drawn by its own draw. It stretches out (16 pixels a frame) to the seeker's reach, then
/// gets its head (an animation at the end) and holds, widening (0x800 to 0x1800, 0x20 a frame;
/// sound 8 each 15 frames). Its power: half A's and B's levels summed plus 10, times one and up to
/// two more by its width. Each segment turns toward the target: those more than 40 pixels short of
/// it within its width of the aim (player 1's swaying toward a second angle by at most 0x300 a
/// frame, a new one at random once there; player 2's at random), the rest straight at it, their
/// length to reach it. A hit box each segment (at its end, or on odd frames its middle). Once the
/// lock ring fades (before it is made: at once) it is drawn in from its base, 8 pixels a frame, and
/// ends.
const BEAM_HITS: u32 = 0x3c;
const BEAM_DRAW: u32 = 0x34;
const BEAM_LOCK: u32 = 0x6c;
const BEAM_BOXES_AT: u32 = 0x70;
const BEAM_TARGET: u32 = 0xf0;
const BEAM_SEEKER: u32 = 0xf4;
const BEAM_HEAD: u32 = 0xf8;
const BEAM_PHASE: u32 = 0x108;
const BEAM_SOUNDS: u32 = 0x109;
const BEAM_WIDTH: u32 = 0x10a;
const BEAM_LENGTH: u32 = 0x10c;
const BEAM_DRAWN_IN: u32 = 0x110;
/// Each segment's angle, the angle it sways toward (words), its length (16.16 of 32 pixels; the
/// last 0).
const BEAM_ANGLES: u32 = 0x114;
const BEAM_SWAYS: u32 = 0x134;
const BEAM_SEGMENTS: u32 = 0x154;
const STRETCHING: u8 = 1;
const HELD: u8 = 2;
const DRAWING_IN: u8 = 3;
const BEAM_GONE: u8 = 4;
/// The flags: a model drawn by its own draw.
const OWN_MODEL: u32 = 0xc0_0000;
const BEAM_DRAW_FN: u32 = 0x0604_98d4;
const BEAM_TYPE: u8 = 11;
const BEAM_COLOUR: u16 = 0x20;
const BEAM_HEAD_ANIMATION: u32 = 0x0607_2904;
const BEAM_STRETCH: u32 = 0x10_0000;
const BEAM_WIDTH_START: u16 = 0x800;
const BEAM_WIDEST: u32 = 0x1800;
const BEAM_WIDENING: u32 = 0x20;
const SE_BEAM: u32 = 8;
const SEGMENT: u32 = 0x20_0000;
const SEGMENT_STEP: u32 = 0xffe0_0000;
const SEGMENTS: u32 = 16;
const SWAY_MOST: u32 = 0x300;
const AIM_SHORT: u32 = 0x28_0000;
const DRAW_IN: u32 = 0x8_0000;
const DRAWN_IN_SHORT: u32 = 0x10_0000;
/// The beam's hit box (attribute, kind, size: bytes 0, 1, 6, 7).
const BEAM_BOX: u32 = 0x0607_2844;
const LAST_BOX: u8 = 0x80;
const SL_ATAN: u32 = 0x0606_780c;

/// The frame's spills: the seeker, target, place along the beam (x, y), how far along, the
/// segment (then its address), the width's and sways' offsets.
const F_SEEKER: u32 = 0x54;
const F_TARGET: u32 = 0x50;
const F_X: u32 = 0x48;
const F_Y: u32 = 0x4c;
const F_ALONG: u32 = 0x40;
const F_SEGMENT: u32 = 0x44;

pub fn beam_exec(cpu: &mut Cpu, b: u32) {
    let f = cpu.stack();
    let seeker = cpu.read32(b + BEAM_SEEKER);
    cpu.write32(f + F_SEEKER, seeker);
    let phase = cpu.read8(b + BEAM_PHASE) as i8;
    if phase <= HELD as i8 {
        let lock = cpu.read32(b + BEAM_LOCK);
        if cpu.read8(lock + OBJ_MODE) as i8 >= ENDING as i8 || cpu.read8(lock + LOCK_PHASE) as i8 >= LOCK_FADING as i8 {
            cpu.write8(b + BEAM_PHASE, if phase == 0 { BEAM_GONE } else { DRAWING_IN });
        }
    }
    let phase = cpu.read8(b + BEAM_PHASE);
    match phase {
        0 | STRETCHING => {
            if phase == 0 {
                let flags = cpu.read32(b + FLAGS) | OWN_MODEL;
                cpu.write32(b + FLAGS, flags);
                cpu.write32(b + BEAM_DRAW, BEAM_DRAW_FN);
                cpu.write16(b + ANGLE, 0);
                cpu.write8(b + SIDE, 0);
                cpu.write8(b + TYPE, BEAM_TYPE);
                cpu.write32(b + BEAM_LENGTH, 0);
                cpu.write16(b + BEAM_WIDTH, BEAM_WIDTH_START);
                let colour = cpu.read16(KIND_COLOURS).wrapping_add(BEAM_COLOUR);
                cpu.write16(b + OWN_COLOUR, colour);
                cpu.write16(b + COLOUR, colour);
                cpu.write32(b + BEAM_DRAWN_IN, 0);
                let t = cpu.read8(seeker + SEEKER_TARGET);
                cpu.write8(b + BEAM_TARGET, t);
                cpu.write8(b + BEAM_SOUNDS, 0);
                cpu.write8(b + BEAM_PHASE, STRETCHING);
            }
            let (len, reach) = (cpu.read32(b + BEAM_LENGTH), cpu.read32(seeker + SEEKER_REACH));
            let len = nested!(cpu, 0x0604_6168, actors::approach(cpu, len, reach, BEAM_STRETCH));
            cpu.write32(b + BEAM_LENGTH, len);
            if len as i32 >= cpu.read32(seeker + SEEKER_REACH) as i32 {
                nested!(cpu, 0x0604_6530, effects::link_init(cpu, b + BEAM_HEAD, BEAM_HEAD_ANIMATION));
                cpu.write8(b + BEAM_PHASE, HELD);
            }
        }
        HELD => {}
        DRAWING_IN => {
            let drawn_in = cpu.read32(b + BEAM_DRAWN_IN).wrapping_add(DRAW_IN);
            cpu.write32(b + BEAM_DRAWN_IN, drawn_in);
            if (drawn_in as i32) < cpu.read32(b + BEAM_LENGTH).wrapping_sub(DRAWN_IN_SHORT) as i32 {
                nested!(cpu, 0x0604_9aa0, beam_boxes(cpu, b));
            } else {
                end(cpu, b);
            }
            return;
        }
        BEAM_GONE => {
            end(cpu, b);
            return;
        }
        _ => return,
    }
    for k in 0..3 {
        let v = cpu.read32(seeker + POSITION + 4 * k);
        cpu.write32(b + POSITION + 4 * k, v);
    }
    let levels = LEVELS.wrapping_add((number(cpu, b) * 3) as u32);
    let sum = cpu.read8(levels) as i8 as i32 + cpu.read8(levels + 1) as i8 as i32;
    cpu.write16(b + POWER, (sum / 2 + 10) as u16);
    let width = cpu.read16(b + BEAM_WIDTH) as i16 as i32 as u32;
    let k = nested!(cpu, 0x0600_6a8e, math::fixed_div(cpu, width, BEAM_WIDEST));
    let power = cpu.read16(b + POWER) as i16 as i32;
    let more = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, (power * 2) as u32, k));
    let power = (more as u16).wrapping_add(cpu.read16(b + POWER));
    cpu.write16(b + POWER, power);
    if cpu.read8(b + BEAM_PHASE) == HELD {
        let reach = cpu.read32(seeker + SEEKER_REACH);
        cpu.write32(b + BEAM_LENGTH, reach);
        let width = cpu.read16(b + BEAM_WIDTH) as i16 as i32 as u32;
        let width = nested!(cpu, 0x0604_6168, actors::approach(cpu, width, BEAM_WIDEST, BEAM_WIDENING));
        cpu.write16(b + BEAM_WIDTH, width as u16);
        let sounds = cpu.read8(b + BEAM_SOUNDS) as i8 as i32 as u32;
        if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, sounds, 15)) == 0 {
            nested!(cpu, 0x0600_50d4, super::sound::se_set(cpu, SE_BEAM));
        }
        let sounds = cpu.read8(b + BEAM_SOUNDS).wrapping_add(1);
        cpu.write8(b + BEAM_SOUNDS, sounds);
    }
    turn_segments(cpu, b, seeker);
    // the SH-2 code holds a pointer into its frame in r12 (beam_boxes pushes it; read later as
    // an unset field)
    cpu.holding(12, f + 0x18, |cpu| nested!(cpu, 0x0604_9aa0, beam_boxes(cpu, b)));
}

/// The segments turned toward the target, out to the beam's length.
fn turn_segments(cpu: &mut Cpu, b: u32, seeker: u32) {
    let f = cpu.stack();
    let n = cpu.read8(b + BEAM_TARGET) as i8 as i32;
    let target = cpu.read32(NUMBERED.wrapping_add((n * 4) as u32));
    cpu.write32(f + F_TARGET, target);
    let (mut x, mut y) = (cpu.read32(b + POSITION), cpu.read32(b + POSITION + 4));
    cpu.write32(f + F_X, x);
    cpu.write32(f + F_Y, y);
    cpu.write32(f + 0x10, 0x1_0000);
    cpu.write32(f + 0x0c, 0x1_0000);
    cpu.write32(f + 0x14, 0);
    let mut along = 0u32;
    cpu.write32(f + F_ALONG, along);
    cpu.write32(f + F_SEGMENT, 0);
    cpu.write32(f + 0x58, BEAM_WIDTH);
    cpu.write32(f + 0x5c, BEAM_SWAYS);
    let width = |cpu: &mut Cpu| cpu.read16(b + BEAM_WIDTH) as i16 as i32 as u32;
    let mut i = 0;
    while i < SEGMENTS {
        if along as i32 >= cpu.read32(b + BEAM_LENGTH) as i32 {
            break;
        }
        let dx = cpu.read32(target + POSITION).wrapping_sub(x);
        let dy = cpu.read32(target + POSITION + 4).wrapping_sub(y);
        let aim = cpu.call(SL_ATAN, &[dx, dy]).wrapping_add(0x4000);
        cpu.write16(f + 0x18, aim as u16);
        let (angle, sway, segment) = (b + BEAM_ANGLES + 2 * i, b + BEAM_SWAYS + 2 * i, b + BEAM_SEGMENTS + 4 * i);
        if (along.wrapping_add(AIM_SHORT) as i32) < cpu.read32(seeker + SEEKER_REACH) as i32 {
            if cpu.read8(b + NUMBER) != 0 {
                let w = width(cpu);
                let r = nested!(cpu, 0x0604_5d90, math::random(cpu, w));
                let a = (r as u16).wrapping_add(cpu.read16(f + 0x18));
                cpu.write16(angle, a);
            } else {
                let (a, aim, w) = (cpu.read16(angle) as i16 as i32 as u32, cpu.read16(f + 0x18) as i16 as i32 as u32, width(cpu));
                let a = nested!(cpu, 0x0604_6214, super::camera::angle_within(cpu, a, aim, w));
                cpu.write16(angle, a as u16);
                let (s, aim, w) = (cpu.read16(sway) as i16 as i32 as u32, cpu.read16(f + 0x18) as i16 as i32 as u32, width(cpu));
                let s = nested!(cpu, 0x0604_6214, super::camera::angle_within(cpu, s, aim, w));
                cpu.write16(sway, s as u16);
                if cpu.read16(angle) == s as u16 {
                    let w = width(cpu);
                    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, w));
                    let aim = (r as u16).wrapping_add(cpu.read16(f + 0x18));
                    cpu.write16(f + 0x18, aim);
                    cpu.write16(sway, aim);
                }
                let (a, s) = (cpu.read16(angle) as i16 as i32 as u32, cpu.read16(sway) as i16 as i32 as u32);
                let d = nested!(cpu, 0x0604_6194, super::camera::angle_diff(cpu, a, s));
                let d = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, d, SWAY_MOST));
                let a = (d as u16).wrapping_add(cpu.read16(angle));
                cpu.write16(angle, a);
            }
            cpu.write32(segment, 0x1_0000);
        } else {
            cpu.write16(angle, aim as u16);
            let target = cpu.read32(f + F_TARGET);
            let dx = cpu.read32(target + POSITION).wrapping_sub(cpu.read32(f + F_X));
            let dy = cpu.read32(target + POSITION + 4).wrapping_sub(cpu.read32(f + F_Y));
            let len = nested!(cpu, 0x0604_5e60, super::aim::length(cpu, dx, dy));
            let scale = nested!(cpu, 0x0600_6a8e, math::fixed_div(cpu, len, SEGMENT));
            cpu.write32(f + 0x10, scale);
            cpu.write32(segment, scale);
        }
        cpu.write32(f, 0);
        cpu.write32(f + 4, SEGMENT_STEP);
        let a = cpu.read16(angle);
        cpu.write16(f + 0x18, a);
        cpu.call(SL_TURN, &[f]);
        x = x.wrapping_add(cpu.read32(f));
        cpu.write32(f + F_X, x);
        y = y.wrapping_add(cpu.read32(f + 4));
        cpu.write32(f + F_Y, y);
        along = along.wrapping_add(SEGMENT);
        cpu.write32(f + F_ALONG, along);
        i += 1;
        cpu.write32(f + F_SEGMENT, i);
    }
    cpu.write32(f + F_SEGMENT, b + 4 * i);
    cpu.write32(b + BEAM_SEGMENTS + 4 * i, 0);
}

fn pixels(v: u32) -> u16 {
    (v as i32 / 0x1_0000) as u16
}

/// The beam's hit boxes (in the actor: one each segment past the drawn-in part, hitting
/// everything again each frame), or none.
pub fn beam_boxes(cpu: &mut Cpu, b: u32) {
    let f = cpu.stack();
    let mut drawn_in = cpu.read32(b + BEAM_DRAWN_IN) as i32;
    cpu.write32(f + 0x14, 0);
    cpu.write32(f + 0x50, 0);
    cpu.write32(f + 0x40, BEAM_SEGMENTS);
    let (mut x, mut y) = (0u32, 0u32);
    let (mut count, mut last) = (0, 0);
    for i in 0..SEGMENTS {
        let segment = cpu.read32(b + BEAM_SEGMENTS + 4 * i);
        if segment == 0 {
            break;
        }
        cpu.write32(f, 0);
        cpu.write32(f + 4, SEGMENT_STEP);
        let a = cpu.read16(b + BEAM_ANGLES + 2 * i);
        cpu.write16(f + 0x18, a);
        cpu.write32(f + 0x10, segment);
        cpu.write32(f + 0x48, y);
        cpu.write32(f + 0x4c, x);
        cpu.call(SL_TURN, &[f]);
        let (dx, dy) = (cpu.read32(f), cpu.read32(f + 4));
        if drawn_in == 0 {
            let at = b + BEAM_BOXES_AT + 8 * count;
            last = at;
            let attr = cpu.read8(BEAM_BOX) & !LAST_BOX;
            cpu.write8(at, attr);
            for k in [1, 6, 7] {
                let v = cpu.read8(BEAM_BOX + k);
                cpu.write8(at + k, v);
            }
            let (ox, oy) = if cpu.read32(GAME_FRAMES) & 1 != 0 { ((dx as i32 / 2) as u32, (dy as i32 / 2) as u32) } else { (dx, dy) };
            cpu.write16(at + 2, pixels(ox.wrapping_add(x)));
            cpu.write16(at + 4, pixels(oy.wrapping_add(y)));
            count += 1;
            cpu.write32(f + 0x50, count);
        } else {
            drawn_in = (drawn_in - SEGMENT as i32).max(0);
        }
        x = x.wrapping_add(dx);
        y = y.wrapping_add(dy);
    }
    if count == 0 {
        cpu.write32(b + BOXES, 0);
    } else {
        hit_set(cpu, b, u32::MAX);
        cpu.write32(b + BOXES, b + BEAM_BOXES_AT);
        let attr = cpu.read8(last) | LAST_BOX;
        cpu.write8(last, attr);
    }
}

/// The beam's own draw (sprite `s`): each segment past the drawn-in part a sprite (kind 3, frames
/// 10-12 at random, flipped at random, its width by the beam's), the one partly drawn in shortened
/// from its base; then the head while it holds.
pub fn beam_draw(cpu: &mut Cpu, s: u32) {
    let b = s.wrapping_sub(SPRITE);
    let f = cpu.stack();
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, f));
    for k in 0..3 {
        let v = cpu.read32(b + POSITION + 4 * k);
        cpu.write32(f + 4 + 4 * k, v);
    }
    cpu.write8(f + 0x1e, A_KIND);
    let colour = cpu.read16(b + OWN_COLOUR);
    cpu.write16(f + 0x18, colour);
    cpu.write8(f + 0x23, 0);
    let width = cpu.read16(b + BEAM_WIDTH) as i16 as i32 as u32;
    let width = nested!(cpu, 0x0600_6a8e, math::fixed_div(cpu, width, BEAM_WIDEST));
    cpu.write32(f + 0x10, width);
    let t = f + 0x2c;
    cpu.write32(t + 0x10, 0x1_0000);
    cpu.write32(t + 0x0c, 0x1_0000);
    cpu.write32(t + 0x14, 0);
    let mut drawn_in = cpu.read32(b + BEAM_DRAWN_IN) as i32;
    for i in 0..SEGMENTS {
        let segment = cpu.read32(b + BEAM_SEGMENTS + 4 * i);
        if segment == 0 {
            break;
        }
        let a = cpu.read16(b + BEAM_ANGLES + 2 * i);
        cpu.write16(f + 0x20, a);
        cpu.write16(t + 0x18, a);
        cpu.write32(f + 0x14, segment);
        cpu.write32(t + 0x10, segment);
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 3)) as i32;
        cpu.write8(f + 0x1f, r.wrapping_abs().wrapping_add(10) as u8);
        if drawn_in == 0 {
            let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0));
            let flags = cpu.read32(f) | r & MIRRORED;
            cpu.write32(f, flags);
            nested!(cpu, 0x0604_0960, sprites::draw(cpu, f));
        } else if drawn_in < SEGMENT as i32 {
            for (k, field) in [(0x1c, 4), (0x20, 8), (0x24, 0x14)] {
                let v = cpu.read32(f + field);
                cpu.write32(t + k, v);
            }
            cpu.write32(t, 0);
            cpu.write32(t + 4, (drawn_in as u32).wrapping_neg());
            cpu.call(SL_TURN, &[t]);
            for k in 0..2 {
                let p = cpu.read32(f + 4 + 4 * k).wrapping_add(cpu.read32(t + 4 * k));
                cpu.write32(f + 4 + 4 * k, p);
            }
            let sy = nested!(cpu, 0x0600_6a8e, math::fixed_div(cpu, SEGMENT.wrapping_sub(drawn_in as u32), SEGMENT));
            cpu.write32(f + 0x14, sy);
            nested!(cpu, 0x0604_0960, sprites::draw(cpu, f));
            for (k, field) in [(0x1c, 4), (0x20, 8), (0x24, 0x14)] {
                let v = cpu.read32(t + k);
                cpu.write32(f + field, v);
            }
            drawn_in = 0;
        } else {
            drawn_in -= SEGMENT as i32;
        }
        cpu.write32(t, 0);
        cpu.write32(t + 4, SEGMENT_STEP);
        cpu.call(SL_TURN, &[t]);
        for k in 0..2 {
            let p = cpu.read32(f + 4 + 4 * k).wrapping_add(cpu.read32(t + 4 * k));
            cpu.write32(f + 4 + 4 * k, p);
        }
    }
    if cpu.read8(b + BEAM_PHASE) == HELD {
        nested!(cpu, 0x0604_6558, effects::link_step(cpu, f, b + BEAM_HEAD));
        cpu.write8(f + 0x23, 4);
        let w = cpu.read32(f + 0x10);
        cpu.write32(f + 0x14, w);
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0));
        cpu.write16(f + 0x20, r as u16);
        nested!(cpu, 0x0604_0960, sprites::draw(cpu, f));
    }
}

/// A+C's shot: kind 3 in the kind's colour, type 12, 8 pixels a frame along its angle (counted in
/// its ship); ended off screen or where it hits (with a spark).
const AC_ANIMATION: u32 = 0x0607_28e8;
const AC_TYPE: u8 = 12;
const AC_BOXES: u32 = 0x0607_284c;
const AC_LINK: u32 = 0x70;
/// The ship's A+C shots alive.
const ALIVE_AC: u32 = 0x84;

pub fn shot_ac_exec(cpu: &mut Cpu, s: u32) {
    if cpu.read8(s + PHASE_LOW) == 0 {
        cpu.write8(s + KIND, A_KIND);
        nested!(cpu, 0x0604_6530, effects::link_init(cpu, s + AC_LINK, AC_ANIMATION));
        let kind = cpu.read8(s + KIND) as i8 as i32;
        let colour = cpu.read16(KIND_COLOURS.wrapping_add((kind * 16) as u32));
        cpu.write16(s + OWN_COLOUR, colour);
        cpu.write16(s + COLOUR, colour);
        cpu.write8(s + LIFT, 0);
        cpu.write8(s + SIDE, 0);
        cpu.write8(s + TYPE, AC_TYPE);
        cpu.write32(s + BOXES, AC_BOXES);
        let f = cpu.stack();
        cpu.write32(f, 0);
        cpu.write32(f + 4, A_SPEED);
        cpu.write32(f + 0x10, 0x1_0000);
        cpu.write32(f + 0x0c, 0x1_0000);
        cpu.write32(f + 0x14, 0);
        let a = cpu.read16(s + ANGLE);
        cpu.write16(f + 0x18, a);
        cpu.call(SL_TURN, &[f]);
        let (vx, vy) = (cpu.read32(f), cpu.read32(f + 4));
        cpu.write32(s + SPEED, vx);
        cpu.write32(s + SPEED + 4, vy);
        cpu.write16(s + PHASE, 1);
    }
    let (x, y) = (cpu.read32(s + POSITION) as i32, cpu.read32(s + POSITION + 4) as i32);
    if x.wrapping_abs() > WIDE_EDGE_X || y.wrapping_abs() > EDGE_Y || hit_here(cpu, s) {
        end(cpu, s);
        return;
    }
    let ship = cpu.read32(s + SHIP);
    if cpu.read8(ship + OBJ_MODE) as i8 >= ENDING as i8 {
        cpu.write32(s + SHIP, 0);
    }
    count_alive(cpu, s, ALIVE_AC);
    if cpu.read8(s + PHASE_LOW) == 1 {
        for k in 0..2 {
            let p = cpu.read32(s + POSITION + 4 * k).wrapping_add(cpu.read32(s + SPEED + 4 * k));
            cpu.write32(s + POSITION + 4 * k, p);
        }
        nested!(cpu, 0x0604_6558, effects::link_step(cpu, s + SPRITE, s + AC_LINK));
    }
}

const AC_EXEC: u32 = 0x0604_a914;
/// A+C's spreads (by C's level: 4 shots below 8, 6 below 24, else 8): each shot's angle and its
/// place from the ship (x, y: words, 16.16).
const AC_SPREADS: [(i8, u32, u32); 3] = [(7, 4, 0x0607_293e), (23, 6, 0x0607_2956), (i8::MAX, 8, 0x0607_297a)];

/// A+C's shots from ship `p`: the spread, their size by A's level and power by A's and C's; a
/// muzzle flash ahead and one behind. The last shot (or 0).
pub fn shot_ac_set(cpu: &mut Cpu, p: u32) -> u32 {
    let f = cpu.stack();
    let n = number(cpu, p);
    let level_c = cpu.read8(LEVEL_C.wrapping_add((n * 3) as u32)) as i8;
    let &(_, count, table) = AC_SPREADS.iter().find(|&&(most, _, _)| level_c <= most).unwrap();
    cpu.write32(f + 0x40, count);
    let mut s = 0;
    for i in 0..count {
        s = shot_in(cpu);
        if s == 0 {
            break;
        }
        cpu.write32(s + BEHAVIOUR, AC_EXEC);
        let e = table + 6 * i;
        for k in 0..2 {
            let v = cpu.read32(p + POSITION + 4 * k).wrapping_add(cpu.read16(e + 2 + 2 * k) as i16 as i32 as u32);
            cpu.write32(s + POSITION + 4 * k, v);
        }
        let z = cpu.read32(p + POSITION + 8);
        cpu.write32(s + POSITION + 8, z);
        let a = cpu.read16(e);
        cpu.write16(s + ANGLE, a);
        let n = cpu.read8(p + NUMBER);
        cpu.write8(s + NUMBER, n);
        for k in 0..2 {
            let size = (level_a(cpu, p) * 0x1f0 + 0xe000) as u32;
            cpu.write32(s + SCALES + 4 * k, size);
        }
        let levels = LEVELS.wrapping_add((number(cpu, p) * 3) as u32);
        let sum = cpu.read8(levels) as i8 as i32 + cpu.read8(levels + 2) as i8 as i32;
        cpu.write16(s + POWER, (sum * 4 + 80) as u16);
        cpu.write32(s + SHIP, p);
    }
    muzzle(cpu, p, 0, C_MUZZLE, 0);
    muzzle(cpu, p, 0, MUZZLE, 0x8000);
    s
}

/// B+C's shot (the ring): drawn by its own draw (eight sprites around it, kind 3 in the kind's
/// colour plus 0x30, frame 30), type 14, held at the ship and turning (0x200 a frame), its radius
/// growing toward 96 pixels and 1 more each B level. It takes the things to hit within it (on the
/// other side, alive, not ending, not to be homed on; at most a fifth of C's level plus 4) as its
/// targets, each keeping its wait from the last frame, and sends a bolt at each whose wait is over
/// (then 19 frames) unless as many bolts are out. Once the ship lets go (not in B+C's state) or
/// ends it shrinks away.
const RING_EXEC: u32 = 0x0604_b98c;
const RING_DRAW: u32 = 0x0604_b830;
const RING_COLOUR: u16 = 0x30;
const RING_FRAME: u8 = 30;
const RING_TYPE: u8 = 14;
const RING_TURN: u16 = 0x200;
const RING_REACH: i32 = -0x60_0000;
const RING_WAIT: u8 = 19;
const RING_FADING: u16 = 5;
const RING_SHRINK: u32 = 0x2000;
/// The ring's bolts out (a byte: each counts itself each frame, cleared after the ring's frame), its
/// radius (16.16, negative: up from it), its targets' numbers (10 bytes, the last -1) and their
/// waits.
const RING_BOLTS: u32 = 0x70;
const RING_RADIUS: u32 = 0x60;
const RING_5C: u32 = 0x5c;
const RING_TARGETS: u32 = 0x74;
const RING_WAITS: u32 = 0x80;
const RING_MOST: u32 = 10;
const STATE_BC: u8 = 9;
/// The ship's B+C shots alive.
const ALIVE_BC: u32 = 0x85;
const BC_BOLT_EXEC: u32 = 0x0604_b510;
/// A B+C bolt's target, ring, and its place from the target.
const BC_TARGET: u32 = 0x70;
const BC_RING: u32 = 0x78;
const BC_FROM: u32 = 0x5c;
/// The polygon flag: four corners.
const FOUR_CORNERS: u32 = 2;

/// The most targets (and bolts a frame) by C's level.
fn ring_most(cpu: &mut Cpu, s: u32) -> i32 {
    let n = number(cpu, s);
    cpu.read8(LEVEL_C.wrapping_add((n * 3) as u32)) as i8 as i32 / 5 + 4
}

pub fn ring_exec(cpu: &mut Cpu, s: u32) {
    let ship = cpu.read32(s + SHIP);
    if cpu.read8(ship + SHIP_STATE) != STATE_BC {
        cpu.write16(s + PHASE, RING_FADING);
    }
    if cpu.read8(ship + OBJ_MODE) as i8 >= ENDING as i8 {
        cpu.write32(s + SHIP, 0);
        cpu.write16(s + PHASE, RING_FADING);
    }
    if cpu.read32(s + SHIP) != 0 {
        count_alive(cpu, s, ALIVE_BC);
        for k in 0..3 {
            let v = cpu.read32(ship + POSITION + 4 * k);
            cpu.write32(s + POSITION + 4 * k, v);
        }
        let a = cpu.read16(s + ANGLE).wrapping_add(RING_TURN);
        cpu.write16(s + ANGLE, a);
    }
    let phase = cpu.read8(s + PHASE_LOW) as i8;
    match phase {
        0 | 1 => {
            if phase == 0 {
                let flags = cpu.read32(s + FLAGS) | OWN_MODEL;
                cpu.write32(s + FLAGS, flags);
                cpu.write32(s + BEAM_DRAW, RING_DRAW);
                cpu.write8(s + KIND, A_KIND);
                cpu.write8(s + FRAME, RING_FRAME);
                let kind = cpu.read8(s + KIND) as i8 as i32;
                let colour = cpu.read16(KIND_COLOURS.wrapping_add((kind * 16) as u32)).wrapping_add(RING_COLOUR);
                cpu.write16(s + OWN_COLOUR, colour);
                cpu.write16(s + COLOUR, colour);
                cpu.write8(s + LIFT, 4);
                cpu.write8(s + SIDE, 0);
                cpu.write8(s + TYPE, RING_TYPE);
                cpu.write8(s + RING_BOLTS, 0);
                cpu.write32(s + RING_RADIUS, 0);
                cpu.write32(s + RING_5C, 0);
                cpu.write16(s + TURN, 0);
                cpu.write8(s + RING_TARGETS, 0xff);
                cpu.write16(s + PHASE, 1);
            }
            ring_targets(cpu, s);
        }
        p if p == RING_FADING as i8 => {
            let w = cpu.read32(s + SCALES);
            let w = nested!(cpu, 0x0604_6168, actors::approach(cpu, w, 0, RING_SHRINK));
            cpu.write32(s + SCALES, w);
            cpu.write32(s + SCALES + 4, w);
            if cpu.read32(s + SCALES) == 0 {
                end(cpu, s);
            }
        }
        _ => {}
    }
    cpu.write8(s + RING_BOLTS, 0);
    let lift = if cpu.read32(GAME_FRAMES) & 1 != 0 { 0 } else { 0x7f };
    cpu.write8(s + LIFT, lift);
}

/// The ring grown, its targets taken (the waits of those still cooling kept), and the bolts sent.
fn ring_targets(cpu: &mut Cpu, s: u32) {
    let f = cpu.stack();
    let r = cpu.read32(s + RING_RADIUS) as i32;
    let n = number(cpu, s);
    let level_b = cpu.read8(B_LEVEL.wrapping_add((n * 3) as u32)) as i8 as i32;
    let to = RING_REACH.wrapping_sub(r).wrapping_sub(level_b << 16);
    cpu.write32(s + RING_RADIUS, r.wrapping_add(to / 16) as u32);
    for i in 0..RING_MOST {
        if cpu.read8(s + RING_TARGETS + i) as i8 == -1 {
            break;
        }
        let wait = cpu.read8(s + RING_WAITS + i);
        if wait != 0 {
            cpu.write8(s + RING_WAITS + i, wait.wrapping_sub(1));
        }
    }
    // the targets still cooling and their waits, in the frame
    let mut cooling = 0;
    for i in 0..RING_MOST {
        let t = cpu.read8(s + RING_TARGETS + i);
        cpu.write8(f + cooling, t);
        if t as i8 == -1 {
            break;
        }
        let wait = cpu.read8(s + RING_WAITS + i);
        if wait != 0 {
            cpu.write8(f + 0xc + cooling, wait);
            cooling += 1;
        }
    }
    let radius = (cpu.read32(s + RING_RADIUS) as i32).wrapping_abs();
    let mut count = 0;
    cpu.write32(f + 0x18, count);
    cpu.write8(s + RING_TARGETS, 0xff);
    cpu.write32(f + 0x1c, f + 0xc);
    for i in 2..64u32 {
        let t = cpu.read32(NUMBERED + 4 * i);
        if t == 0
            || cpu.read8(s + SIDE) == cpu.read8(t + SIDE)
            || cpu.read8(t + OBJ_MODE) as i8 >= ENDING as i8
            || cpu.read32(t + FLAGS) & UNHOMED != 0
            || cpu.read32(t + LIFE) as i32 <= 0
            || nested!(cpu, 0x0604_5ee4, super::aim::distance(cpu, s, t)) as i32 > radius
        {
            continue;
        }
        let at = s + count;
        let n = cpu.read8(t + NUMBER);
        cpu.write8(at + RING_TARGETS, n);
        cpu.write8(at + RING_WAITS, 0);
        for j in 0..cooling {
            if cpu.read8(f + j) == cpu.read8(t + NUMBER) {
                let wait = cpu.read8(f + 0xc + j);
                cpu.write8(at + RING_WAITS, wait);
                break;
            }
        }
        count += 1;
        cpu.write32(f + 0x18, count);
        if count as i32 >= ring_most(cpu, s) {
            break;
        }
        cpu.write8(s + count + RING_TARGETS, 0xff);
    }
    if cpu.read8(s + RING_BOLTS) as i8 as i32 >= ring_most(cpu, s) || count == 0 {
        return;
    }
    cpu.write32(f + 0x20, RING_WAITS);
    for j in 0..count {
        let at = s + j;
        if cpu.read8(at + RING_TARGETS) as i8 == -1 {
            break;
        }
        if cpu.read8(at + RING_WAITS) != 0 {
            continue;
        }
        let b = shot_in(cpu);
        if b == 0 {
            continue;
        }
        let n = cpu.read8(at + RING_TARGETS) as i8 as i32;
        let t = cpu.read32(NUMBERED.wrapping_add((n * 4) as u32));
        cpu.write32(b + BEHAVIOUR, BC_BOLT_EXEC);
        for k in 0..3 {
            let v = cpu.read32(s + POSITION + 4 * k);
            cpu.write32(b + POSITION + 4 * k, v);
        }
        let n = cpu.read8(s + NUMBER);
        cpu.write8(b + NUMBER, n);
        cpu.write16(b + POWER, 0);
        let ship = cpu.read32(s + SHIP);
        cpu.write32(b + SHIP, ship);
        cpu.write32(b + BC_RING, s);
        cpu.write32(b + BC_TARGET, t);
        for k in 0..2 {
            let d = cpu.read32(s + POSITION + 4 * k).wrapping_sub(cpu.read32(t + POSITION + 4 * k));
            cpu.write32(b + BC_FROM + 4 * k, d);
        }
        hit_set(cpu, b, 0);
        cpu.write8(at + RING_WAITS, RING_WAIT);
    }
}

/// The ring's own draw (sprite `s`): eight four-cornered polygons of its kind around it at its
/// radius, turned with it (the last four mirrored, turning the other way), flickering in colour (its
/// colour less 0x20 while it has targets).
pub fn ring_draw(cpu: &mut Cpu, sp: u32) {
    let s = sp.wrapping_sub(SPRITE);
    let f = cpu.stack();
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, f));
    let corners = f + 0x6c;
    cpu.write32(f + 0x24, corners);
    let flags = cpu.read32(f) | FOUR_CORNERS;
    cpu.write32(f, flags);
    for (k, v) in [(8, 0u16), (2, 0), (0, 0), (0xa, 0xfffc), (4, 0xfffc), (6, 4), (0xc, 4), (0xe, 4)] {
        cpu.write16(corners + k, v);
    }
    let z = cpu.read32(s + POSITION + 8);
    cpu.write32(f + 0x0c, z);
    let kind = cpu.read8(s + KIND);
    cpu.write8(f + 0x1e, kind);
    let lift = cpu.read8(s + LIFT);
    cpu.write8(f + 0x23, lift);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 4)) as u16;
    let flicker = if cpu.read8(s + RING_TARGETS) as i8 == -1 { 0x36 } else { 0x16 };
    let kind = cpu.read8(f + 0x1e) as i8 as i32;
    let colour = r.wrapping_add(flicker).wrapping_add(cpu.read16(KIND_COLOURS.wrapping_add((kind * 16) as u32)));
    cpu.write16(f + 0x18, colour);
    let t = f + 0x2c;
    for k in 0..2 {
        let v = cpu.read32(s + SCALES + 4 * k);
        cpu.write32(t + 0x0c + 4 * k, v);
    }
    cpu.write32(t + 0x14, 0);
    let a = cpu.read16(s + ANGLE);
    cpu.write16(t + 0x18, a);
    for i in 0..8 {
        if i == 4 {
            let flip = cpu.read32(t + 0x14) | MIRRORED;
            cpu.write32(t + 0x14, flip);
            let a = cpu.read16(s + ANGLE).wrapping_neg();
            cpu.write16(t + 0x18, a);
        }
        cpu.write32(t, 0);
        let r = cpu.read32(s + RING_RADIUS);
        cpu.write32(t + 4, r);
        cpu.call(SL_TURN, &[t]);
        for k in 0..2 {
            let p = cpu.read32(s + POSITION + 4 * k).wrapping_add(cpu.read32(t + 4 * k));
            cpu.write32(f + 4 + 4 * k, p);
        }
        let a = cpu.read16(t + 0x18);
        cpu.write16(f + 0x20, a);
        nested!(cpu, 0x0604_0960, sprites::draw(cpu, f));
        let a = cpu.read16(t + 0x18).wrapping_add(0x4000);
        cpu.write16(t + 0x18, a);
    }
}

/// B+C's shot from ship `p`: the ring (its power 0). It, or 0.
pub fn shot_bc_set(cpu: &mut Cpu, p: u32) -> u32 {
    let s = shot_in(cpu);
    if s == 0 {
        return 0;
    }
    cpu.write32(s + BEHAVIOUR, RING_EXEC);
    for k in 0..3 {
        let v = cpu.read32(p + POSITION + 4 * k);
        cpu.write32(s + POSITION + 4 * k, v);
    }
    let n = cpu.read8(p + NUMBER);
    cpu.write8(s + NUMBER, n);
    cpu.write16(s + POWER, 0);
    cpu.write32(s + SHIP, p);
    s
}

/// B+C's bolt (the ring's, at a target): kind 3 in the kind's colour plus 0x30, frame 30, unturned,
/// type 14, counted in its ring each frame. It flies in from the ring onto its target (a quarter of
/// the way a frame, for 16 frames) and stays on it (while it is alive, not ending and not to be
/// homed on), calls a strike down from the ship (sound 6), waits while the strike lives, then
/// shrinks away; it shrinks at once if the ship ends. Its lift flickers.
const MARK_ANIMATION: u32 = 0x0607_28f4;
const MARK_LINK: u32 = 0x90;
/// The bolt's frames flying in (a word), whether a strike counted itself this frame, frames
/// between strikes, strikes called (bytes); the phase's high byte (between strikes).
const MARK_FRAMES: u32 = 0x6e;
const MARK_STRUCK: u32 = 0x74;
const MARK_GAP: u32 = 0x75;
const MARK_STRIKES: u32 = 0x76;
const PHASE_HIGH: u32 = 0x6c;
const MARK_IN: i16 = 15;
const MARK_GAPS: i8 = 6;
const MARK_SHRINK: u32 = 0x1000;
const MARK_SHRINKING: u16 = 4;
const SE_STRIKE: u32 = 6;
const STRIKE_SIZE: u32 = 0x47c;
const STRIKE_EXEC: u32 = 0x0604_b0e4;

pub fn bc_bolt_exec(cpu: &mut Cpu, s: u32) {
    let mut target = cpu.read32(s + BC_TARGET);
    let ship = cpu.read32(s + SHIP);
    if cpu.read8(ship + OBJ_MODE) as i8 >= ENDING as i8 {
        cpu.write16(s + PHASE, MARK_SHRINKING);
    }
    let ring = cpu.read32(s + BC_RING);
    if ring != 0 {
        let n = cpu.read8(ring + RING_BOLTS).wrapping_add(1);
        cpu.write8(ring + RING_BOLTS, n);
        if cpu.read8(ring + OBJ_MODE) as i8 >= ENDING as i8 {
            cpu.write32(s + BC_RING, 0);
        }
    }
    if target != 0
        && (cpu.read8(target + OBJ_MODE) as i8 >= ENDING as i8 || cpu.read32(target + FLAGS) & UNHOMED != 0 || cpu.read32(target + LIFE) as i32 <= 0)
    {
        cpu.write32(s + BC_TARGET, 0);
        target = 0;
    }
    let phase = cpu.read8(s + PHASE_LOW);
    match phase {
        0 | 1 => {
            if phase == 0 {
                let flags = cpu.read32(s + FLAGS) | UNTURNED;
                cpu.write32(s + FLAGS, flags);
                cpu.write8(s + KIND, A_KIND);
                cpu.write8(s + FRAME, RING_FRAME);
                let kind = cpu.read8(s + KIND) as i8 as i32;
                let colour = cpu.read16(KIND_COLOURS.wrapping_add((kind * 16) as u32)).wrapping_add(RING_COLOUR);
                cpu.write16(s + OWN_COLOUR, colour);
                cpu.write16(s + COLOUR, colour);
                cpu.write8(s + LIFT, 4);
                cpu.write8(s + SIDE, 0);
                cpu.write8(s + TYPE, RING_TYPE);
                cpu.write8(s + MARK_STRUCK, 0);
                cpu.write8(s + MARK_STRIKES, 0);
                nested!(cpu, 0x0604_6530, effects::link_init(cpu, s + MARK_LINK, MARK_ANIMATION));
                cpu.write16(s + MARK_FRAMES, 0);
                cpu.write16(s + PHASE, 1);
            }
            for k in 0..2 {
                let d = cpu.read32(s + BC_FROM + 4 * k) as i32;
                cpu.write32(s + BC_FROM + 4 * k, d.wrapping_add(d.wrapping_neg() / 4) as u32);
            }
            if target != 0 {
                for k in 0..2 {
                    let p = cpu.read32(target + POSITION + 4 * k).wrapping_add(cpu.read32(s + BC_FROM + 4 * k));
                    cpu.write32(s + POSITION + 4 * k, p);
                }
            }
            let n = cpu.read16(s + MARK_FRAMES).wrapping_add(1);
            cpu.write16(s + MARK_FRAMES, n);
            if n as i16 > MARK_IN {
                cpu.write16(s + MARK_FRAMES, 0);
                cpu.write16(s + PHASE, 2);
            }
        }
        2 => {
            if target != 0 {
                for k in 0..2 {
                    let v = cpu.read32(target + POSITION + 4 * k);
                    cpu.write32(s + POSITION + 4 * k, v);
                }
            }
            if cpu.read8(s + PHASE_HIGH) == 0 {
                strike(cpu, s, ship);
            } else {
                let gap = cpu.read8(s + MARK_GAP).wrapping_add(1);
                cpu.write8(s + MARK_GAP, gap);
                if gap as i8 > MARK_GAPS {
                    cpu.write8(s + MARK_GAP, 0);
                    cpu.write8(s + PHASE_HIGH, 0);
                }
            }
        }
        3 => {
            let target = cpu.read32(s + BC_TARGET);
            if target != 0 {
                for k in 0..2 {
                    let v = cpu.read32(target + POSITION + 4 * k);
                    cpu.write32(s + POSITION + 4 * k, v);
                }
            }
            if cpu.read8(s + MARK_STRUCK) == 0 {
                cpu.write16(s + PHASE, MARK_SHRINKING);
            }
        }
        4 => {
            let w = cpu.read32(s + SCALES);
            let w = nested!(cpu, 0x0604_6168, actors::approach(cpu, w, 0, MARK_SHRINK));
            cpu.write32(s + SCALES, w);
            cpu.write32(s + SCALES + 4, w);
            if cpu.read32(s + SCALES) == 0 {
                end(cpu, s);
            }
        }
        _ => {}
    }
    cpu.write8(s + MARK_STRUCK, 0);
    nested!(cpu, 0x0604_6558, effects::link_step(cpu, s + SPRITE, s + MARK_LINK));
    let lift = if cpu.read32(GAME_FRAMES) & 1 != 0 { 0 } else { 0xf7 };
    cpu.write8(s + LIFT, lift);
}

/// The strike called down from `ship`'s nose (its power half B's and C's levels summed plus 10);
/// then on to waiting for it (after a gap, another, while its count stays below 1).
fn strike(cpu: &mut Cpu, s: u32, ship: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, STRIKE_SIZE)) as i8 as i32 as u32;
    let t = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 1, blocks, ACTOR_EXEC));
    if t == 0 {
        return;
    }
    nested!(cpu, 0x0604_62ac, actors::actor_setup(cpu, t));
    cpu.write32(t + BEHAVIOUR, STRIKE_EXEC);
    let x = cpu.read32(ship + POSITION);
    cpu.write32(t + POSITION, x);
    let y = cpu.read32(ship + POSITION + 4).wrapping_add(0xfff7_0000);
    cpu.write32(t + POSITION + 4, y);
    let z = cpu.read32(ship + POSITION + 8);
    cpu.write32(t + POSITION + 8, z);
    nested!(cpu, 0x0600_50d4, super::sound::se_set(cpu, SE_STRIKE));
    let n = cpu.read8(ship + NUMBER);
    cpu.write8(t + NUMBER, n);
    let levels = LEVELS.wrapping_add((n as i8 as i32 * 3) as u32);
    let sum = cpu.read8(levels + 1) as i8 as i32 + cpu.read8(levels + 2) as i8 as i32;
    cpu.write16(t + POWER, (sum / 2 + 10) as u16);
    cpu.write32(t + SHIP, ship);
    cpu.write32(t + STRIKE_MARK, s);
    cpu.write8(s + MARK_GAP, 0);
    let strikes = cpu.read8(s + MARK_STRIKES).wrapping_add(1);
    cpu.write8(s + MARK_STRIKES, strikes);
    if strikes as i8 > 0 {
        cpu.write16(s + PHASE, 3);
    } else {
        cpu.write8(s + PHASE_HIGH, 1);
    }
}

/// The strike (a trail: 5 pieces 3 places apart, drawn by its own draw, kind 0 in its colour, type
/// 13, its own boxes; no hits of its own). Its speed (turned by its angle): a random drift to the
/// side and 4 pixels back, slowing for 18 frames; then gaining toward 24 pixels a frame ahead as it
/// widens, steering toward its bolt (a share of the turn a frame, growing over 32 frames, while
/// more than 16 pixels away; then all of it) until the bolt lies within reach (its distance less
/// than twice the speed, with 24 pixels and up to 8 at random): there it bursts and fades. Its bolt
/// gone, it flies on straight and fades off screen. It counts itself in its bolt each frame.
const STRIKE_BOXES: u32 = 0x0607_2854;
const STRIKE_FLAGS: u32 = 0x8_0004;
const STRIKE_DRAW: u32 = 0x0604_aef8;
/// A sprite's flags for a polygon of its character, four corners.
const TEXTURED_QUAD: u32 = 5;
const STRIKE_TYPE: u8 = 13;
const STRIKE_START: u32 = 0x1000;
const STRIKE_PIECES: u32 = 5;
const STRIKE_SPACING: u32 = 3;
/// The strike's bolt, speed (x, y), frames launching then steering (a word), frames left fading.
const STRIKE_MARK: u32 = 0x78;
const STRIKE_SPEED: u32 = 0x5c;
const STRIKE_FRAMES: u32 = 0x6e;
const STRIKE_LEFT: u32 = 0x70;
const STRIKE_DRIFT: u32 = 0x4_0000;
const STRIKE_LAUNCH: u16 = 18;
const STRIKE_FASTEST: u32 = 0xffe8_0000;
const STRIKE_ACCEL: u32 = 0x8000;
const STRIKE_WIDEST: u32 = 0x4000;
const STRIKE_WIDENING: u32 = 0x400;
const STRIKE_STEERS: i16 = 31;
const STRIKE_CLOSE: i32 = 0xf_ffff;
const STRIKE_REACH: u32 = 0x18_0000;
const STRIKE_REACH_RANDOM: u32 = 0x8_0000;
const BURST_EXEC: u32 = 0x0604_ad2c;
/// SGL's (as far as can be told): a 16.16 division.
const SL_DIV: u32 = 0x0606_62d8;

pub fn strike_exec(cpu: &mut Cpu, t: u32) {
    let f = cpu.stack();
    let mark = cpu.read32(t + STRIKE_MARK);
    let record = |cpu: &mut Cpu| {
        let a = cpu.read16(t + ANGLE) as i16 as i32 as u32;
        nested!(cpu, 0x0604_da00, actors::trail_record(cpu, t, a));
    };
    let phase = cpu.read8(t + PHASE_LOW);
    match phase {
        0 | 1 => {
            if phase == 0 {
                nested!(cpu, 0x0604_d95c, actors::trail_init(cpu, t, STRIKE_PIECES, STRIKE_SPACING, STRIKE_BOXES));
                let flags = cpu.read32(t + FLAGS) | STRIKE_FLAGS;
                cpu.write32(t + FLAGS, flags);
                cpu.write32(t + BEAM_DRAW, STRIKE_DRAW);
                cpu.write8(t + LIFT, 0);
                cpu.write8(t + KIND, 0);
                cpu.write8(t + FRAME, 0);
                cpu.write32(t + SCALES, STRIKE_START);
                cpu.write32(t + SCALES + 4, STRIKE_START);
                let kind = cpu.read8(t + KIND) as i8 as i32;
                let colour = cpu.read16(KIND_COLOURS.wrapping_add((kind * 16) as u32));
                cpu.write16(t + OWN_COLOUR, colour);
                cpu.write16(t + COLOUR, colour);
                cpu.write8(t + SIDE, 0);
                cpu.write8(t + TYPE, STRIKE_TYPE);
                cpu.write32(t + BOXES, STRIKE_BOXES);
                hit_set(cpu, t, 0);
                let r = nested!(cpu, 0x0604_5d90, math::random(cpu, STRIKE_DRIFT));
                cpu.write32(t + STRIKE_SPEED, r);
                cpu.write32(t + STRIKE_SPEED + 4, STRIKE_DRIFT);
                cpu.write16(t + STRIKE_FRAMES, STRIKE_LAUNCH);
                cpu.write16(t + PHASE, 1);
            }
            record(cpu);
            for k in 0..2 {
                let v = cpu.read32(t + STRIKE_SPEED + 4 * k) as i32;
                cpu.write32(t + STRIKE_SPEED + 4 * k, v.wrapping_add(v.wrapping_neg() / 8) as u32);
            }
            let n = cpu.read16(t + STRIKE_FRAMES).wrapping_sub(1);
            cpu.write16(t + STRIKE_FRAMES, n);
            if n as i16 <= 0 {
                cpu.write32(t + STRIKE_SPEED + 4, 0);
                cpu.write32(t + STRIKE_SPEED, 0);
                cpu.write16(t + STRIKE_FRAMES, 0);
                cpu.write16(t + PHASE, 2);
            }
        }
        2 => {
            record(cpu);
            let to = nested!(cpu, 0x0604_5e30, super::aim::angle_to(cpu, t, mark)) as i16 as i32 as u32;
            let from = cpu.read16(t + ANGLE) as i16 as i32 as u32;
            let d = nested!(cpu, 0x0604_6194, super::camera::angle_diff(cpu, from, to));
            cpu.write32(f + 0x14, d);
            let v = cpu.read32(t + STRIKE_SPEED + 4);
            let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, STRIKE_FASTEST, STRIKE_ACCEL));
            cpu.write32(t + STRIKE_SPEED + 4, v);
            let w = cpu.read32(t + SCALES + 4);
            let w = nested!(cpu, 0x0604_6168, actors::approach(cpu, w, STRIKE_WIDEST, STRIKE_WIDENING));
            cpu.write32(t + SCALES + 4, w);
            let steered = cpu.read16(t + STRIKE_FRAMES) as i16;
            if steered <= STRIKE_STEERS && nested!(cpu, 0x0604_5ee4, super::aim::distance(cpu, t, mark)) as i32 > STRIKE_CLOSE {
                let share = 8 - (cpu.read16(t + STRIKE_FRAMES) as i16 / 4) as i32;
                let d = cpu.read32(f + 0x14);
                let turn = cpu.call(SL_DIV, &[d, share as u32]);
                let a = cpu.read16(t + ANGLE).wrapping_add(turn as u16);
                cpu.write16(t + ANGLE, a);
                let n = cpu.read16(t + STRIKE_FRAMES).wrapping_add(1);
                cpu.write16(t + STRIKE_FRAMES, n);
            } else {
                let a = cpu.read16(t + ANGLE).wrapping_add(cpu.read16(f + 0x16));
                cpu.write16(t + ANGLE, a);
            }
            let r = nested!(cpu, 0x0604_5d90, math::random(cpu, STRIKE_REACH_RANDOM));
            let d = nested!(cpu, 0x0604_5ee4, super::aim::distance(cpu, t, mark));
            let reach = d.wrapping_add(r).wrapping_add(STRIKE_REACH) as i32;
            let speed = (cpu.read32(t + STRIKE_SPEED + 4) as i32).wrapping_mul(2).wrapping_abs();
            if reach <= speed {
                cpu.write16(t + PHASE, 4);
            }
        }
        3 => {
            record(cpu);
            let v = cpu.read32(t + STRIKE_SPEED + 4);
            let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, STRIKE_FASTEST, STRIKE_ACCEL));
            cpu.write32(t + STRIKE_SPEED + 4, v);
            let (x, y) = (cpu.read32(t + POSITION) as i32, cpu.read32(t + POSITION + 4) as i32);
            if x.wrapping_abs() > WIDE_EDGE_X || y.wrapping_abs() > EDGE_Y {
                cpu.write16(t + PHASE, 5);
            }
        }
        4 => {
            record(cpu);
            let b = shot_in(cpu);
            if b != 0 {
                cpu.write32(b + BEHAVIOUR, BURST_EXEC);
                for k in 0..3 {
                    let v = cpu.read32(t + POSITION + 4 * k);
                    cpu.write32(b + POSITION + 4 * k, v);
                }
                let n = cpu.read8(t + NUMBER);
                cpu.write8(b + NUMBER, n);
                let level = cpu.read8(LEVEL_C.wrapping_add((n as i8 as i32 * 3) as u32)) as i8 as i32;
                cpu.write16(b + POWER, (level * 2 + 20) as u16);
                let ship = cpu.read32(t + SHIP);
                cpu.write32(b + SHIP, ship);
                let mark = cpu.read32(t + STRIKE_MARK);
                cpu.write32(b + BURST_MARK, mark);
                cpu.write16(t + PHASE, 5);
            }
        }
        5 => {
            record(cpu);
            if cpu.read8(t + PHASE_HIGH) == 0 {
                let left = cpu.read8(t + actors::TRAIL_PIECES) as u32 * cpu.read8(t + actors::TRAIL_SPACING) as u32;
                cpu.write32(t + STRIKE_LEFT, left);
                cpu.write8(t + PHASE_HIGH, 1);
            }
            let left = cpu.read32(t + STRIKE_LEFT).wrapping_sub(1);
            cpu.write32(t + STRIKE_LEFT, left);
            if left as i32 <= 0 {
                end(cpu, t);
                cpu.write16(t + PHASE, 6);
            }
        }
        _ => {}
    }
    if cpu.read8(t + PHASE_LOW) as i8 > 3 {
        return;
    }
    if mark != 0 {
        let n = cpu.read8(mark + MARK_STRUCK).wrapping_add(1);
        cpu.write8(mark + MARK_STRUCK, n);
        if cpu.read8(mark + OBJ_MODE) as i8 >= ENDING as i8 {
            cpu.write32(t + STRIKE_MARK, 0);
            if cpu.read8(t + PHASE_LOW) as i8 <= 2 {
                cpu.write16(t + PHASE, 3);
            }
        }
    }
    for k in 0..2 {
        let v = cpu.read32(t + STRIKE_SPEED + 4 * k);
        cpu.write32(f + 4 * k, v);
    }
    cpu.write32(f + 0x10, 0x1_0000);
    cpu.write32(f + 0x0c, 0x1_0000);
    cpu.write32(f + 0x14, 0);
    let a = cpu.read16(t + ANGLE);
    cpu.write16(f + 0x18, a);
    cpu.call(SL_TURN, &[f]);
    for k in 0..2 {
        let p = cpu.read32(t + POSITION + 4 * k).wrapping_add(cpu.read32(f + 4 * k));
        cpu.write32(t + POSITION + 4 * k, p);
    }
}

/// The strike's own draw (sprite `sp`): its pieces back along its kept places (each a 4-pixel-wide
/// textured polygon from one place to the next, its colour brighter by up to 15 along it, the last
/// a different frame).
pub fn strike_draw(cpu: &mut Cpu, sp: u32) {
    let t = sp.wrapping_sub(SPRITE);
    let f = cpu.stack();
    let c = f + 0x2c;
    for (k, v) in [(0xc, 0xfffeu16), (0, 0xfffe), (8, 2), (4, 2), (6, 0), (2, 0)] {
        cpu.write16(c + k, v);
    }
    cpu.write32(f + 0x48, c + 0xa);
    cpu.write16(c + 0xe, 8);
    cpu.write16(c + 0xa, 8);
    cpu.write32(f, TEXTURED_QUAD);
    cpu.write8(f + 0x1e, 0);
    cpu.write32(f + 0x24, c);
    cpu.write32(f + 0x14, 0x1_0000);
    cpu.write32(f + 0x10, 0x1_0000);
    let lift = cpu.read8(sp + 0x23);
    cpu.write8(f + 0x23, lift);
    let own = cpu.read16(sp + 0x1a);
    cpu.write16(f + 0x1a, own);
    cpu.write16(f + 0x18, own);
    let mut kept = cpu.read16(t + actors::TRAIL_KEPT) as i16 as i32;
    let mut next = cpu.read16(t + actors::TRAIL_NEXT) as i16 as i32;
    for k in 0..3 {
        let v = cpu.read32(t + POSITION + 4 * k);
        cpu.write32(f + 4 + 4 * k, v);
    }
    cpu.write8(f + 0x1f, 0xae);
    let pieces = |cpu: &mut Cpu| cpu.read8(t + actors::TRAIL_PIECES) as i32;
    let mut i = 0;
    if pieces(cpu) <= 0 {
        return;
    }
    cpu.write32(f + 0x3c, c + 0xa);
    cpu.write32(f + 0x40, c + 0xe);
    cpu.write32(f + 0x44, actors::TRAIL_PIECES);
    while i < pieces(cpu) {
        if kept <= 0 {
            return;
        }
        let spacing = cpu.read8(t + actors::TRAIL_SPACING) as i32;
        if kept >= spacing {
            next -= spacing;
            kept -= spacing;
        } else {
            next -= kept;
            kept = 0;
        }
        if next < 0 {
            next += 0x80;
        }
        let px = (cpu.read16(t + actors::TRAIL_X + 2 * next as u32) as u32) << 16;
        let py = (cpu.read16(t + actors::TRAIL_Y + 2 * next as u32) as u32) << 16;
        let (dx, dy) = (cpu.read32(f + 4).wrapping_sub(px), cpu.read32(f + 8).wrapping_sub(py));
        let len = nested!(cpu, 0x0604_5e60, super::aim::length(cpu, dx, dy));
        let len = (len as i32 / 0x1_0000) as u16;
        cpu.write16(c + 0xe, len);
        cpu.write16(c + 0xa, len);
        let (dx, dy) = (cpu.read32(f + 4).wrapping_sub(px), cpu.read32(f + 8).wrapping_sub(py));
        let a = cpu.call(SL_ATAN, &[dx, dy]).wrapping_add(0x4000);
        cpu.write16(f + 0x20, a as u16);
        let frame = if kept > 0 && i + 1 < pieces(cpu) { 0xaf } else { 0xb3 };
        cpu.write8(f + 0x1f, frame);
        nested!(cpu, 0x0604_0960, sprites::draw(cpu, f));
        cpu.write32(f + 4, px);
        cpu.write32(f + 8, py);
        i += 1;
        let n = pieces(cpu) as u32;
        let along = nested!(cpu, 0x0600_6a8e, math::fixed_div(cpu, i as u32, n));
        let shade = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, 15, along));
        let colour = (shade as u16).wrapping_add(cpu.read16(f + 0x1a));
        cpu.write16(f + 0x18, colour);
    }
}

/// B+C's burst (where a strike lands): drawn by its own draw (kind 3 frame 56, the first kind's
/// colour, spinning, its lift flickering), type 13, its own boxes; grows and shrinks away as C's
/// shot does (its size by C's level), keeping its bolt (until grown, or the bolt ends).
const BURST_DRAW: u32 = 0x0604_ac8c;
const BURST_BOXES: u32 = 0x0607_285c;
const BURST_TYPE: u8 = 13;
const BURST_COLOUR: u16 = 0x20;
const BURST_START: u32 = 0x1000;
const BURST_5C: u32 = 0x3_0000;
const BURST_MARK: u32 = 0x78;
const BURST_SUMMED: u32 = 0x7c;
const BURST_FRAME: u8 = 56;
const BURST_SPIN: u16 = 0x1000;
const VBLANKS: u32 = super::globals::VBLANKS;

pub fn burst_exec(cpu: &mut Cpu, b: u32) {
    let mark = cpu.read32(b + BURST_MARK);
    let phase = cpu.read8(b + PHASE_LOW) as i8;
    match phase {
        0 | 1 => {
            if phase == 0 {
                cpu.write8(b + LIFT, 0);
                cpu.write8(b + KIND, A_KIND);
                let flags = cpu.read32(b + FLAGS) | OWN_MODEL;
                cpu.write32(b + FLAGS, flags);
                cpu.write32(b + BEAM_DRAW, BURST_DRAW);
                let kind = cpu.read8(b + KIND) as i8 as i32;
                let colour = cpu.read16(KIND_COLOURS.wrapping_add((kind * 16) as u32)).wrapping_add(BURST_COLOUR);
                cpu.write16(b + OWN_COLOUR, colour);
                cpu.write16(b + COLOUR, colour);
                cpu.write8(b + SIDE, 0);
                cpu.write8(b + TYPE, BURST_TYPE);
                cpu.write32(b + BOXES, BURST_BOXES);
                cpu.write32(b + SPEED + 4, BURST_5C);
                cpu.write32(b + SPEED, BURST_5C);
                cpu.write32(b + SCALES + 4, BURST_START);
                cpu.write32(b + SCALES, BURST_START);
                let n = number(cpu, b);
                let level = cpu.read8(LEVEL_C.wrapping_add((n * 3) as u32)) as i8 as i32;
                cpu.write32(b + C_SIZE, (level * 0x2e8 + 0xc000) as u32);
                cpu.write32(b + BURST_SUMMED, 0);
                cpu.write16(b + PHASE, GROWING);
            }
            if burst_grown(cpu, b, BURST_SUMMED) {
                cpu.write32(b + BURST_MARK, 0);
                cpu.write16(b + PHASE, SHRINKING);
            }
        }
        2 => burst_shrinks(cpu, b, BURST_SUMMED),
        _ => {}
    }
    let w = cpu.read32(b + SCALES);
    cpu.write32(b + SCALES + 4, w);
    if mark != 0 && cpu.read8(mark + OBJ_MODE) as i8 >= ENDING as i8 {
        cpu.write32(b + BURST_MARK, 0);
    }
}

/// The burst's own draw (sprite `sp`): one sprite, turned on a sixteenth of a turn each frame.
pub fn burst_draw(cpu: &mut Cpu, sp: u32) {
    let b = sp.wrapping_sub(SPRITE);
    let f = cpu.stack();
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, f));
    for k in 0..5 {
        let v = cpu.read32(b + POSITION + 4 * k);
        cpu.write32(f + 4 + 4 * k, v);
    }
    let a = cpu.read16(sp + 0x20);
    cpu.write16(f + 0x20, a);
    let kind = cpu.read8(sp + 0x1e);
    cpu.write8(f + 0x1e, kind);
    cpu.write8(f + 0x1f, BURST_FRAME);
    let lift = cpu.read8(sp + 0x23);
    cpu.write8(f + 0x23, lift);
    let colour = cpu.read16(KIND_COLOURS);
    cpu.write16(f + 0x18, colour);
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, f));
    let a = cpu.read16(sp + 0x20).wrapping_sub(BURST_SPIN);
    cpu.write16(sp + 0x20, a);
    let lift = if cpu.read32(VBLANKS) & 1 != 0 { 0 } else { 0xf7 };
    cpu.write8(sp + 0x23, lift);
}

/// The sword (A+B+C: a model drawn by its own draw, type 15, its power a third of ten times all
/// three levels plus 100), held at the ship's nose (9.6 pixels out along where it points),
/// spinning about its length and counted in its ship. It starts pointing up and swings round once
/// (16 frames: the way the stick was pushed as it started, right unless left); then, by the ship's
/// settings, it turns toward the stick's direction (a quarter of the way, at most 0x1000 a frame),
/// spins, or turns away from it. Its angle eases toward where it points (a quarter of the way), or
/// leads it while turning fast; it hits everything again once it has swept far enough (at once
/// while turning fast). While it moves it keeps its afterimages (up to 16 of the last 30 frames:
/// scales, angle, place). Once the ship lets go (not in the sword's state) it shrinks away and its
/// afterimages fade, and it ends.
const SWORD_SIZE: u32 = 0x184;
const SWORD_EXEC: u32 = 0x0604_c2e8;
const SWORD_DRAW: u32 = 0x0604_be74;
const SWORD_MODEL: u32 = 0x0607_2ba0;
const SWORD_BOXES: u32 = 0x0607_2864;
const SWORD_TYPE: u8 = 15;
const HYPER_TYPE: u8 = 16;
/// The flags: a model.
const IS_MODEL: u32 = 0x80_0000;
/// The sword's model (its address: SWORD_MODEL), ship, frames into the swing or pause (a word),
/// where it points, the stick's direction, its swing a frame (words), how far it has swept since
/// it last hit everything; its afterimages (30, 8 bytes each: scales in 1024ths of SWORD_UNIT,
/// the angle, the place in pixels), how many it keeps at most, from which they fade, the last's
/// index and how many are kept, how far they have faded (bytes), its blade's half width and length
/// (pixel words), the unit of the afterimages' scales.
const SWORD_MODEL_AT: u32 = 0x44;
const SWORD_SHIP: u32 = 0x80;
const SWORD_FRAMES: u32 = 0x6e;
const SWORD_POINTS: u32 = 0x70;
const SWORD_AIM: u32 = 0x72;
const SWORD_SWING: u32 = 0x76;
const SWORD_SWEPT: u32 = 0x7c;
const SWORD_IMAGES: u32 = 0x84;
const SWORD_MOST: u32 = 0x174;
const SWORD_FADE_FROM: u32 = 0x175;
const SWORD_LAST: u32 = 0x176;
const SWORD_KEPT: u32 = 0x177;
const SWORD_FADED: u32 = 0x178;
const SWORD_HALF_WIDTH: u32 = 0x17a;
const SWORD_LENGTH: u32 = 0x17c;
const SWORD_UNIT: u32 = 0x180;
const SWORD_IMAGE_COUNT: i32 = 30;
/// Its spin (a model's turn about Y: a word).
const SPIN: u32 = 0x2e;
const STATE_SWORD: u8 = 10;
/// The ship's sword shots alive, its pad, its settings (the sword's turning: toward the stick,
/// spinning, away from it) and the way its sword last swung (left: 1).
const ALIVE_SWORD: u32 = 0x86;
const SHIP_PAD: u32 = 0xa0;
const SHIP_LIMITS: u32 = 0xa8;
const SWORD_TOWARD: u32 = 0xa0_0000;
const SWORD_SPINS: u32 = 0x40_0000;
const SWORD_AWAY: u32 = 0x10_0000;
const SWING_LEFT: u32 = 0x8a;
/// The stick's directions as angles (0 up, clockwise).
const STICK_ANGLES: [(u32, u16); 8] =
    [(0x5000, 0xe000), (0x2000, 0x8000), (0x1000, 0), (0x4000, 0xc000), (0x8000, 0x4000), (0x6000, 0xa000), (0x9000, 0x2000), (0xa000, 0x6000)];
const STICK_RIGHT: u32 = 0x8000;
const STICK_LEFT: u32 = 0x4000;
const SWORD_REACH: u32 = 0xfff6_199a;
const SWORD_SPIN: u16 = 0x400;
const SWORD_TURN_MOST: u32 = 0x1000;
const FAST_TURN: i32 = 0x100;
const SWEEP_HITS: i32 = 63;
const SWORD_LEAD: u32 = 0x1_8000;
const SETTLED: i32 = 0x400;
const SWORD_FADING: u16 = 4;

/// The frame's spills: the stick's bits, its direction (a word: -1 none), the turn this frame.
const F_STICK: u32 = 0x14;
const F_DIRECTION: u32 = 0x18;
const F_TURN: u32 = 8;

pub fn sword_exec(cpu: &mut Cpu, s: u32) {
    let f = cpu.stack();
    let ship = cpu.read32(s + SWORD_SHIP);
    if cpu.read8(s + PHASE_LOW) as i8 <= 3 {
        if cpu.read8(ship + OBJ_MODE) as i8 >= ENDING as i8 {
            cpu.write32(s + SWORD_SHIP, 0);
        }
        let p = cpu.read32(s + SWORD_SHIP);
        if p != 0 {
            for field in [ALIVE_SWORD, ALIVE_ALL] {
                let n = cpu.read8(p + field).wrapping_add(1);
                cpu.write8(p + field, n);
            }
        }
        if cpu.read8(ship + SHIP_STATE) != STATE_SWORD {
            cpu.write16(s + PHASE, SWORD_FADING);
        }
    }
    let stick = cpu.read16(ship + SHIP_PAD) as i16 as i32 as u32 & 0xf000;
    cpu.write32(f + F_STICK, stick);
    let direction = STICK_ANGLES.iter().find(|&&(bits, _)| bits == stick).map_or(0xffff, |&(_, a)| a);
    cpu.write16(f + F_DIRECTION, direction);
    if direction != 0xffff {
        cpu.write16(s + SWORD_AIM, direction);
    }
    let mut turn = 0i32;
    cpu.write32(f + F_TURN, 0);
    match cpu.read8(s + PHASE_LOW) {
        0 => {
            for (field, v) in [(SWORD_LAST, 0xffu8), (SWORD_KEPT, 0), (SWORD_MOST, 16), (SWORD_FADE_FROM, 0), (SWORD_FADED, 0)] {
                cpu.write8(s + field, v);
            }
            cpu.write16(s + SWORD_HALF_WIDTH, 0);
            cpu.write16(s + SWORD_LENGTH, 0xffda);
            cpu.write32(s + SWORD_UNIT, 0x0400_0000);
            let flags = cpu.read32(s + FLAGS) | OWN_MODEL;
            cpu.write32(s + FLAGS, flags);
            cpu.write32(s + BEAM_DRAW, SWORD_DRAW);
            // (the SH-2 code first points it by the stick, or by the settings, then overwrites that)
            cpu.write16(s + SWORD_FRAMES, 0);
            cpu.write32(s + SWORD_SWEPT, 0);
            cpu.write16(s + SWORD_AIM, 0x8000);
            cpu.write16(s + ANGLE, 0);
            cpu.write16(s + SWORD_POINTS, 0);
            if stick & STICK_RIGHT != 0 {
                cpu.write8(ship + SWING_LEFT, 0);
            }
            if stick & STICK_LEFT != 0 {
                cpu.write8(ship + SWING_LEFT, 1);
            }
            let swing = if cpu.read8(ship + SWING_LEFT) == 0 { 0x1000 } else { 0xf000 };
            cpu.write16(s + SWORD_SWING, swing);
            cpu.write16(s + PHASE, 2);
        }
        1 => {
            let limits = |cpu: &mut Cpu| cpu.read32(ship + SHIP_LIMITS);
            let toward = |cpu: &mut Cpu, away: u32| {
                let (from, to) = (cpu.read16(s + SWORD_POINTS) as i16 as i32 as u32, cpu.read16(s + SWORD_AIM) as i16 as i32 as u32);
                let d = nested!(cpu, 0x0604_6194, super::camera::angle_diff(cpu, from, to.wrapping_add(away))) as i32;
                nested!(cpu, 0x0606_4f3c, math::clamp(cpu, (d / 4) as u32, SWORD_TURN_MOST)) as i32
            };
            if limits(cpu) & SWORD_TOWARD != 0 {
                turn = toward(cpu, 0);
                cpu.write32(f + F_TURN, turn as u32);
                let t = toward(cpu, 0);
                let a = cpu.read16(s + SWORD_POINTS).wrapping_add(t as u16);
                cpu.write16(s + SWORD_POINTS, a);
            }
            if limits(cpu) & SWORD_SPINS != 0 {
                turn = SWORD_TURN_MOST as i32;
                cpu.write32(f + F_TURN, turn as u32);
                let a = cpu.read16(s + SWORD_POINTS).wrapping_add(SWORD_TURN_MOST as u16);
                cpu.write16(s + SWORD_POINTS, a);
            }
            if limits(cpu) & SWORD_AWAY != 0 {
                turn = toward(cpu, 0x8000);
                cpu.write32(f + F_TURN, turn as u32);
                let t = toward(cpu, 0x8000);
                let a = cpu.read16(s + SWORD_POINTS).wrapping_add(t as u16);
                cpu.write16(s + SWORD_POINTS, a);
            }
        }
        2 => {
            turn = cpu.read16(s + SWORD_SWING) as i16 as i32;
            cpu.write32(f + F_TURN, turn as u32);
            let a = cpu.read16(s + SWORD_POINTS).wrapping_add(cpu.read16(s + SWORD_SWING));
            cpu.write16(s + SWORD_POINTS, a);
            let n = cpu.read16(s + SWORD_FRAMES).wrapping_add(1);
            cpu.write16(s + SWORD_FRAMES, n);
            if n as i16 > 15 {
                cpu.write16(s + SWORD_FRAMES, 0);
                cpu.write16(s + PHASE, 1);
            }
        }
        3 => {
            let n = cpu.read16(s + SWORD_FRAMES).wrapping_add(1);
            cpu.write16(s + SWORD_FRAMES, n);
            if n as i16 > 7 {
                cpu.write16(s + PHASE, 1);
            }
        }
        4 => {
            hit_set(cpu, s, 0);
            cpu.write32(s + SWORD_SWEPT, 0xffff_fff8);
            let w = cpu.read32(s + SCALES) as i32;
            let w = w.wrapping_add((-0x1000i32).wrapping_sub(w) / 4);
            cpu.write32(s + SCALES, w as u32);
            if w < 0 {
                cpu.write32(s + SCALES, 0);
            }
            let w = cpu.read32(s + SCALES);
            cpu.write32(s + SCALES + 4, w);
        }
        _ => {}
    }
    let all_set = |cpu: &mut Cpu| cpu.read32(s + HIT_SET) == u32::MAX && cpu.read32(s + HIT_SET + 4) == u32::MAX;
    if turn.wrapping_abs() > FAST_TURN {
        if !all_set(cpu) {
            let n = cpu.read32(s + SWORD_SWEPT).wrapping_add(1);
            cpu.write32(s + SWORD_SWEPT, n);
        }
        if cpu.read32(s + SWORD_SWEPT) as i32 > 0 {
            hit_set(cpu, s, u32::MAX);
            cpu.write32(s + SWORD_SWEPT, 0);
        }
        let lead = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, turn as u32, SWORD_LEAD));
        let a = cpu.read16(s + SWORD_POINTS).wrapping_sub(lead as u16);
        cpu.write16(s + ANGLE, a);
    } else {
        if !all_set(cpu) {
            let n = cpu.read32(s + SWORD_SWEPT).wrapping_add(0x20).wrapping_add(turn.wrapping_abs() as u32);
            cpu.write32(s + SWORD_SWEPT, n);
        }
        if cpu.read32(s + SWORD_SWEPT) as i32 > SWEEP_HITS {
            hit_set(cpu, s, u32::MAX);
            cpu.write32(s + SWORD_SWEPT, 0);
        }
        let (from, to) = (cpu.read16(s + ANGLE) as i16 as i32 as u32, cpu.read16(s + SWORD_POINTS) as i16 as i32 as u32);
        let d = nested!(cpu, 0x0604_6194, super::camera::angle_diff(cpu, from, to)) as i32;
        let a = cpu.read16(s + ANGLE).wrapping_add((d / 4) as u16);
        cpu.write16(s + ANGLE, a);
    }
    cpu.write32(f, 0);
    cpu.write32(f + 4, SWORD_REACH);
    for k in 0..2 {
        let v = cpu.read32(ship + SCALES + 4 * k);
        cpu.write32(f + 0x0c + 4 * k, v);
    }
    let flip = cpu.read32(s + FLAGS) & FLIP;
    cpu.write32(f + 0x14, flip);
    let a = cpu.read16(s + SWORD_POINTS);
    cpu.write16(f + 0x18, a);
    cpu.call(SL_TURN, &[f]);
    for k in 0..2 {
        let p = cpu.read32(ship + POSITION + 4 * k).wrapping_add(cpu.read32(f + 4 * k));
        cpu.write32(s + POSITION + 4 * k, p);
    }
    let z = cpu.read32(ship + POSITION + 8);
    cpu.write32(s + POSITION + 8, z);
    let spin = cpu.read16(s + SPIN).wrapping_add(SWORD_SPIN);
    cpu.write16(s + SPIN, spin);
    sword_images(cpu, s, turn);
}

/// The afterimages: fading once the sword fades (it ends once they are gone and it has shrunk);
/// else one kept while it moves (turning, or its angle not yet within 0x400 of where it points),
/// and lost while it is still.
fn sword_images(cpu: &mut Cpu, s: u32, turn: i32) {
    let fade = |cpu: &mut Cpu| {
        let faded = cpu.read8(s + SWORD_FADED);
        if (faded as i8) < cpu.read8(s + SWORD_MOST) as i8 {
            cpu.write8(s + SWORD_FADED, faded.wrapping_add(1));
        }
    };
    if cpu.read8(s + PHASE_LOW) as u16 == SWORD_FADING {
        fade(cpu);
        let kept = cpu.read8(s + SWORD_KEPT).wrapping_sub(1);
        cpu.write8(s + SWORD_KEPT, kept);
        if kept as i8 <= 0 && cpu.read32(s + SCALES) as i32 <= 0 {
            end(cpu, s);
        }
        return;
    }
    if turn == 0 {
        let (from, to) = (cpu.read16(s + ANGLE) as i16 as i32 as u32, cpu.read16(s + SWORD_POINTS) as i16 as i32 as u32);
        let d = nested!(cpu, 0x0604_6194, super::camera::angle_diff(cpu, from, to)) as i32;
        if d.wrapping_abs() <= SETTLED {
            let kept = cpu.read8(s + SWORD_KEPT);
            if kept as i8 > 0 {
                cpu.write8(s + SWORD_KEPT, kept - 1);
            }
            fade(cpu);
            return;
        }
    }
    let last = cpu.read8(s + SWORD_LAST).wrapping_add(1);
    cpu.write8(s + SWORD_LAST, last);
    let last = (cpu.read8(s + SWORD_LAST) as i8 as i32 % SWORD_IMAGE_COUNT) as u8;
    cpu.write8(s + SWORD_LAST, last);
    let image = |cpu: &mut Cpu| s.wrapping_add((cpu.read8(s + SWORD_LAST) as i8 as i32 * 8) as u32) + SWORD_IMAGES;
    let x = pixels(cpu.read32(s + POSITION));
    let at = image(cpu);
    cpu.write16(at + 4, x);
    let y = pixels(cpu.read32(s + POSITION + 4));
    let at = image(cpu);
    cpu.write16(at + 6, y);
    let a = cpu.read16(s + ANGLE);
    let at = image(cpu);
    cpu.write16(at + 2, a);
    for k in 0..2 {
        let (scale, unit) = (cpu.read32(s + SCALES + 4 * k), cpu.read32(s + SWORD_UNIT));
        let v = nested!(cpu, 0x0600_6a8e, math::fixed_div(cpu, scale, unit));
        let at = image(cpu);
        cpu.write8(at + k, v as u8);
    }
    let (kept, most) = (cpu.read8(s + SWORD_KEPT) as i8 as i32 + 1, cpu.read8(s + SWORD_MOST) as i8 as i32 as u32);
    let kept = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, kept as u32, most));
    cpu.write8(s + SWORD_KEPT, kept as u8);
    cpu.write8(s + SWORD_FADED, 0);
}

/// The sword's maker: the sword at ship `p` (its sprite and hits set up as actor_setup does). It,
/// or 0.
pub fn shot_sword_set(cpu: &mut Cpu, p: u32) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, SWORD_SIZE)) as i8 as i32 as u32;
    let s = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 1, blocks, ACTOR_EXEC));
    if s == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, s + SPRITE));
    nested!(cpu, 0x0604_6284, super::hits::hit_init(cpu, s + BOXES));
    cpu.write16(s + PHASE, 0);
    cpu.write32(s + BEHAVIOUR, SWORD_EXEC);
    let flags = cpu.read32(s + FLAGS) | IS_MODEL;
    cpu.write32(s + FLAGS, flags);
    cpu.write32(s + SWORD_MODEL_AT, SWORD_MODEL);
    let model = cpu.read32(SWORD_MODEL);
    cpu.write32(s + BEAM_DRAW, model);
    for k in 0..3 {
        let v = cpu.read32(p + POSITION + 4 * k);
        cpu.write32(s + POSITION + 4 * k, v);
    }
    cpu.write8(s + LIFT, 0);
    let n = cpu.read8(p + NUMBER);
    cpu.write8(s + NUMBER, n);
    cpu.write8(s + SIDE, 0);
    cpu.write8(s + TYPE, SWORD_TYPE);
    let levels = LEVELS.wrapping_add((n as i8 as i32 * 3) as u32);
    let sum = (0..3).map(|k| cpu.read8(levels + k) as i8 as i32).sum::<i32>();
    cpu.write16(s + POWER, (sum * 10 / 3 + 100) as u16);
    cpu.write32(s + BOXES, SWORD_BOXES);
    cpu.write32(s + SWORD_SHIP, p);
    s
}

/// SGL's matrix calls (sprites.rs's).
const SL_PUSH_MATRIX: u32 = 0x0606_71cc;
const SL_TRANSLATE: u32 = 0x0606_721c;
const SL_ROT_Z: u32 = 0x0606_726c;
const SL_ROT_Y: u32 = 0x0606_736c;
const SL_SCALE: u32 = 0x0606_73ec;
const SL_PUT_POLYGON: u32 = 0x0606_745c;
const SL_POP_MATRIX: u32 = 0x0606_7200;
const LOOPS: u32 = super::globals::LOOPS;
/// The afterimages' band: four-cornered polygons, half-lit on odd passes (attribute 0x100), the
/// first kind's colour plus 0xc0 (player 2: 0xb0), brighter by up to 16 and shorter (the hyper
/// sword) as they fade.
const BAND_FLAGS: u32 = 1;
const BAND_HALF: u32 = 0x100;

/// The sword's own draw (sprite `sp`): its model (turned, spun, scaled: mirrored with its flip),
/// then its afterimages as a band of polygons, each from one afterimage's blade to the one before.
pub fn sword_draw(cpu: &mut Cpu, sp: u32) {
    let s = sp.wrapping_sub(SPRITE);
    let f = cpu.stack();
    cpu.call(SL_PUSH_MATRIX, &[]);
    let (x, y, z) = (cpu.read32(s + POSITION), cpu.read32(s + POSITION + 4), cpu.read32(s + POSITION + 8));
    cpu.call(SL_TRANSLATE, &[x, y, z]);
    let a = cpu.read16(s + ANGLE) as i16 as i32 as u32;
    cpu.call(SL_ROT_Z, &[a]);
    let a = cpu.read16(s + SPIN) as i16 as i32 as u32;
    cpu.call(SL_ROT_Y, &[a]);
    let sx = cpu.read32(s + SCALES);
    let sx = if cpu.read32(s + FLAGS) & MIRRORED != 0 { sx.wrapping_neg() } else { sx };
    let (sy, sz) = (cpu.read32(s + SCALES + 4), cpu.read32(s + SCALES + 8));
    cpu.call(SL_SCALE, &[sx, sy, sz]);
    let model = cpu.read32(s + SWORD_MODEL_AT);
    let model = cpu.read32(model);
    cpu.call(SL_PUT_POLYGON, &[model]);
    cpu.call(SL_POP_MATRIX, &[]);
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, f));
    let flags = cpu.read32(f) | BAND_FLAGS;
    cpu.write32(f, flags);
    if cpu.read32(LOOPS) & 1 != 0 {
        cpu.write32(f, flags | BAND_HALF);
    }
    let c = f + 0x6c;
    cpu.write32(f + 0x24, c);
    for k in [0xc, 8, 4] {
        cpu.write32(f + k, 0);
    }
    let base = if cpu.read8(s + NUMBER) == 0 { 0xc0 } else { 0xb0 };
    let colour = cpu.read16(KIND_COLOURS).wrapping_add(base);
    cpu.write16(f + 0x1a, colour);
    cpu.write8(f + 0x23, 0);
    let mut i = cpu.read8(s + SWORD_LAST) as i8 as i32;
    cpu.write32(f + 0x7c, i as u32);
    cpu.write32(f + 0x80, 0x1_0000);
    if cpu.read8(s + SWORD_KEPT) as i8 <= 1 {
        return;
    }
    cpu.write32(f + 0x84, SWORD_HALF_WIDTH);
    cpu.write32(f + 0x88, SWORD_UNIT);
    let mut j = 0;
    loop {
        if j == 0 {
            blade(cpu, s, f, i, c, c + 4, false);
        } else {
            for (to, from) in [(0, 0xc), (2, 0xe), (4, 8), (6, 0xa)] {
                let v = cpu.read16(c + from);
                cpu.write16(c + to, v);
            }
        }
        if i == 0 {
            i = SWORD_IMAGE_COUNT;
        }
        i = (i - 1) as i8 as i32;
        cpu.write32(f + 0x7c, i as u32);
        blade(cpu, s, f, i, c + 0xc, c + 8, true);
        let colour = cpu.read16(f + 0x1a);
        cpu.write16(f + 0x18, colour);
        let faded = cpu.read8(s + SWORD_FADED) as i8 as i32 + j;
        if faded > cpu.read8(s + SWORD_FADE_FROM) as i8 as i32 {
            let most = cpu.read8(s + SWORD_MOST) as i8 as i32 as u32;
            let k = nested!(cpu, 0x0600_6a8e, math::fixed_div(cpu, faded as u32, most));
            let more = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, 16, k));
            let colour = (more as u16).wrapping_add(cpu.read16(f + 0x18));
            cpu.write16(f + 0x18, colour);
            cpu.write32(f + 0x80, 0x1_0000u32.wrapping_sub(k));
        }
        nested!(cpu, 0x0604_0960, sprites::draw(cpu, f));
        j += 1;
        if j + 1 >= cpu.read8(s + SWORD_KEPT) as i8 as i32 {
            return;
        }
    }
}

/// Afterimage `i`'s blade: its base (at `base`, x and y words) and tip (`tip`), turned and scaled
/// as the afterimage was (the hyper sword's shortened as it fades, `fading`).
fn blade(cpu: &mut Cpu, s: u32, f: u32, i: i32, base: u32, tip: u32, fading: bool) {
    let t = f + 0x2c;
    let image = s.wrapping_add((i * 8) as u32) + SWORD_IMAGES;
    let half = cpu.read16(s + SWORD_HALF_WIDTH) as i16 as i32;
    cpu.write32(t, half.wrapping_mul(0x8000) as u32);
    cpu.write32(t + 4, 0);
    for k in 0..2 {
        let (v, unit) = (cpu.read8(image + k) as u32, cpu.read32(s + SWORD_UNIT));
        let v = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, v, unit));
        cpu.write32(t + 0x0c + 4 * k, v);
    }
    if fading && cpu.read8(s + TYPE) == HYPER_TYPE {
        let (v, k) = (cpu.read32(t + 0x10), cpu.read32(f + 0x80));
        let v = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, v, k));
        cpu.write32(t + 0x10, v);
    }
    cpu.write32(t + 0x14, 0);
    let a = cpu.read16(image + 2);
    cpu.write16(t + 0x18, a);
    cpu.write32(f + 0x90, SL_TURN);
    for (at, x, y) in [(base, half, 0), (tip, half.wrapping_neg(), (cpu.read16(s + SWORD_LENGTH) as i16 as i32) << 16)] {
        if at == tip {
            cpu.write32(t, x.wrapping_mul(0x8000) as u32);
            cpu.write32(t + 4, y as u32);
        }
        cpu.call(SL_TURN, &[t]);
        let px = pixels(cpu.read32(t)).wrapping_add(cpu.read16(image + 4));
        cpu.write16(at, px);
        let py = pixels(cpu.read32(t + 4)).wrapping_add(cpu.read16(image + 6));
        cpu.write16(at + 2, py);
    }
}
