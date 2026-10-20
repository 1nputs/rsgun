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
    let ship = cpu.read32(s + SHIP);
    if ship != 0 {
        for field in [ALIVE_A, ALIVE_ALL] {
            let n = cpu.read8(ship + field).wrapping_add(1);
            cpu.write8(ship + field, n);
        }
    }
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
    let f = frame;
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
    let colour = cpu.read16(MUZZLE_COLOUR);
    cpu.write16(f + 0x20, colour);
    let n = cpu.read8(p + NUMBER);
    cpu.write8(f + 0x1e, n);
    cpu.write32(f + 0x28, MUZZLE);
    cpu.write32(f + 0x3c, 0);
    nested!(cpu, 0x0604_e1cc, effects::effect_set(cpu, f));
    last
}
