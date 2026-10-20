//! The players' shots (player.rs's weapon states make them): actors on the players' side (list 1)
//! with the weapon as their type (hits.rs counts their hits and experience by it), each counting
//! itself alive in its ship each frame (the ship's limits), ended off screen or where they hit
//! something or the ground (with a spark).

use super::{actors, bg, effects, math, nested, objects, Cpu};

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
    muzzle(cpu, p, 0x40);
    last
}

/// The muzzle flash at the ship (its scales and angle), kind 3's colour plus `colour`.
fn muzzle(cpu: &mut Cpu, p: u32, colour: u16) {
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
    let a = cpu.read16(p + ANGLE);
    cpu.write16(f + 0x18, a);
    cpu.write16(f + 0x24, 0);
    cpu.write8(f + 0x26, 0);
    cpu.write8(f + 0x1c, A_KIND);
    cpu.write8(f + 0x1d, 0);
    let c = cpu.read16(MUZZLE_COLOUR).wrapping_add(colour);
    cpu.write16(f + 0x20, c);
    let n = cpu.read8(p + NUMBER);
    cpu.write8(f + 0x1e, n);
    cpu.write32(f + 0x28, MUZZLE);
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
    muzzle(cpu, p, 0);
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
            hit_set(cpu, s, u32::MAX);
            let size = cpu.read32(s + SCALES) as i32;
            let to = cpu.read32(s + C_SIZE) as i32;
            let size = size.wrapping_add(to.wrapping_sub(size.wrapping_sub(0x200)) / 8);
            cpu.write32(s + SCALES, size as u32);
            if size >= cpu.read32(s + C_SIZE) as i32 {
                cpu.write32(s + C_SUMMED, 0);
                cpu.write32(s + C_SHRINK, 0);
                cpu.write16(s + TURN, 0);
                let power = cpu.read16(s + POWER) as i16;
                cpu.write16(s + POWER, (power / 2) as u16);
                cpu.write16(s + PHASE, SHRINKING);
            }
        }
        2 => {
            if cpu.read32(s + SCALES) as i32 > C_COUNTED {
                count_alive(cpu, s, ALIVE_C);
            }
            let summed = cpu.read32(s + C_SUMMED).wrapping_add(cpu.read32(s + SCALES));
            cpu.write32(s + C_SUMMED, summed);
            let twice = cpu.read32(s + C_SIZE).wrapping_mul(2);
            if summed as i32 >= twice as i32 {
                hit_set(cpu, s, u32::MAX);
                cpu.write32(s + C_SUMMED, summed.wrapping_sub(twice));
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
    let n = cpu.read8(o + 0x2f) as u32;
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
