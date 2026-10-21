//! The ground's enemies (tanks, boats, guns): actors that move along a route script, riding the
//! ground's movement, only hittable while low enough.
//!
//! A route script (bytes): first its flags (a word, low byte first), the first direction (its high
//! byte: the actor's base direction is added), height (signed: in 16ths of the depth unit) and
//! speed (its high byte, in 16ths of a pixel a frame); then segments: an optional flag byte (odd: its
//! low 4 bits returned as the segment starts; 0x21 in its top bits sets flag 0x1000), the frames the
//! segment lasts, and the direction, height and speed it turns, rises and speeds toward over them.
//! 0x81 ends it.

use super::{actors, aim, math, nested, Cpu};

/// The objects' mode byte (ending).
const OBJ_MODE: u32 = 3;
const ENDING: u8 = 0x70;

/// An actor's flags, position, its route flags (a word), speed (x, y), direction and speed (words:
/// 16-bit angle, speed by 256ths), frames left in the segment, turn and speed change a frame
/// (words), the script's place (a word), its base direction (a word), height change a frame and
/// height (16.16 by 16ths: the depth), its side's mirror (a byte, ±1).
const FLAGS: u32 = 0x10;
const POSITION: u32 = 0x14;
const ROUTE_FLAGS: u32 = 0x7e;
const SPEED: u32 = 0x80;
const MIRROR: u32 = 0x88;
const DIRECTION: u32 = 0x8c;
const PACE: u32 = 0x8e;
const FRAMES_LEFT: u32 = 0x90;
const TURN: u32 = 0x92;
const PACE_CHANGE: u32 = 0x94;
const ROUTE_AT: u32 = 0x96;
const BASE_DIRECTION: u32 = 0xa8;
const CLIMB: u32 = 0xac;
const HEIGHT: u32 = 0xb0;
/// The script's end, a flag byte's mark, the flag bits it may set.
const END: u8 = 0x81;
const FLAG_BYTE: u8 = 1;
const FLAG_TOP: u8 = 0xf1;
const FLAG_1000: u8 = 0x21;
const ROUTE_1000: u16 = 0x1000;
/// The flags: no hits (while higher than LOW).
const NO_HITS: u32 = 0x8_0000;
const LOW: i32 = 0x4_0000;
/// SGL's sine and cosine.
const SL_SIN: u32 = 0x0606_9a0c;
const SL_COS: u32 = 0x0606_9a30;

/// The speed (x, y) for the actor's direction and pace.
fn heading(cpu: &mut Cpu, o: u32) {
    let pace = (cpu.read16(o + PACE) as i16 as i32) << 4;
    let d = cpu.read16(o + DIRECTION) as i16 as i32 as u32;
    let s = cpu.call(SL_SIN, &[d]);
    let vx = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, pace as u32));
    cpu.write32(o + SPEED, vx);
    let pace = (cpu.read16(o + PACE) as i16 as i32) << 4;
    let d = cpu.read16(o + DIRECTION) as i16 as i32 as u32;
    let c = cpu.call(SL_COS, &[d]);
    let vy = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, c.wrapping_neg(), pace as u32));
    cpu.write32(o + SPEED + 4, vy);
}

/// One frame of the actor's route `script`: a new segment taken when the last is over (its
/// heading set; the change a frame toward its direction, height and speed worked out), else the
/// segment's changes made and the speed eased toward the new heading (by a sixteenth of the pace
/// times the heading's sine or cosine). The new segment's flag byte's low 4 bits, else 0.
pub fn route_step(cpu: &mut Cpu, o: u32, script: u32) -> u32 {
    let f = cpu.stack();
    cpu.write32(f, 0);
    let at = cpu.read16(o + ROUTE_AT) as i16 as i32 as u32;
    if cpu.read8(script.wrapping_add(at)) == END {
        return 0;
    }
    if cpu.read16(o + FRAMES_LEFT) != 0 {
        follow(cpu, o);
    } else {
        start_segment(cpu, o, script, at);
    }
    let n = cpu.read16(o + FRAMES_LEFT).wrapping_sub(1);
    cpu.write16(o + FRAMES_LEFT, n);
    cpu.read32(f)
}

fn start_segment(cpu: &mut Cpu, o: u32, script: u32, at: u32) {
    let f = cpu.stack();
    let byte = |cpu: &mut Cpu, k: u32| cpu.read8(script.wrapping_add(k)) as u16;
    if at == 0 {
        let flags = byte(cpu, 0) | byte(cpu, 1) << 8;
        cpu.write16(o + ROUTE_FLAGS, flags);
        let d = (byte(cpu, 2) << 8).wrapping_add(cpu.read16(o + BASE_DIRECTION));
        cpu.write16(o + DIRECTION, d);
        let h = (cpu.read8(script + 3) as i8 as i32).wrapping_mul(0x10_0000);
        cpu.write32(o + HEIGHT, h as u32);
        let pace = byte(cpu, 4) << 8;
        cpu.write16(o + PACE, pace);
        let at = cpu.read16(o + ROUTE_AT).wrapping_add(1);
        cpu.write16(o + ROUTE_AT, at);
    } else {
        let d = (byte(cpu, at + 1) << 8).wrapping_add(cpu.read16(o + BASE_DIRECTION));
        cpu.write16(o + DIRECTION, d);
        let at = cpu.read16(o + ROUTE_AT) as i16 as i32 as u32;
        let h = (cpu.read8(script.wrapping_add(at + 2)) as i8 as i32).wrapping_mul(0x10_0000);
        cpu.write32(o + HEIGHT, h as u32);
        let at = cpu.read16(o + ROUTE_AT) as i16 as i32 as u32;
        let pace = byte(cpu, at + 3) << 8;
        cpu.write16(o + PACE, pace);
    }
    heading(cpu, o);
    let at = cpu.read16(o + ROUTE_AT).wrapping_add(4);
    cpu.write16(o + ROUTE_AT, at);
    let mut p = script.wrapping_add(at as i16 as i32 as u32);
    let b = cpu.read8(p);
    if b == END {
        return;
    }
    if b & FLAG_BYTE != 0 {
        cpu.write16(o + ROUTE_AT, at.wrapping_add(1));
        cpu.write32(f, b as u32);
        p += 1;
        if b & FLAG_TOP == FLAG_1000 {
            let flags = cpu.read16(o + ROUTE_FLAGS) | ROUTE_1000;
            cpu.write16(o + ROUTE_FLAGS, flags);
        }
        let v = cpu.read32(f) & 0xf;
        cpu.write32(f, v);
    }
    let frames = cpu.read8(p) as u16;
    cpu.write16(o + FRAMES_LEFT, frames);
    let frames = frames as i16 as i32 as u32;
    let to = ((cpu.read8(p + 1) as u16) << 8).wrapping_add(cpu.read16(o + BASE_DIRECTION)) as u32;
    let from = cpu.read16(o + DIRECTION) as u32;
    let d = nested!(cpu, 0x0605_6580, aim::angle_between(cpu, from, to)) as i16 as i32 as u32;
    let turn = nested!(cpu, 0x0600_6a78, math::quotient(cpu, d, frames));
    cpu.write16(o + TURN, turn as u16);
    let h = cpu.read32(o + HEIGHT) as i32;
    let h = (if h < 0 { h + 0xff } else { h }) >> 8;
    let target = (cpu.read8(p + 2) as i8 as i32) << 12;
    let frames = cpu.read16(o + FRAMES_LEFT) as i16 as i32 as u32;
    let climb = nested!(cpu, 0x0600_6a78, math::quotient(cpu, h.wrapping_sub(target) as u32, frames));
    cpu.write32(o + CLIMB, (climb << 8).wrapping_neg());
    let frames = cpu.read16(o + FRAMES_LEFT) as i16 as i32 as u32;
    let to = (cpu.read8(p + 3) as u32) << 8;
    let from = cpu.read16(o + PACE) as u32;
    let d = nested!(cpu, 0x0605_6580, aim::angle_between(cpu, from, to)) as i16 as i32 as u32;
    let change = nested!(cpu, 0x0600_6a78, math::quotient(cpu, d, frames));
    cpu.write16(o + PACE_CHANGE, change as u16);
}

/// The segment's changes made; the speed eased toward the heading.
fn follow(cpu: &mut Cpu, o: u32) {
    let d = cpu.read16(o + DIRECTION).wrapping_add(cpu.read16(o + TURN));
    cpu.write16(o + DIRECTION, d);
    let pace = cpu.read16(o + PACE).wrapping_add(cpu.read16(o + PACE_CHANGE));
    cpu.write16(o + PACE, pace);
    let h = cpu.read32(o + HEIGHT).wrapping_add(cpu.read32(o + CLIMB));
    cpu.write32(o + HEIGHT, h);
    for (k, trig, neg) in [(0, SL_SIN, false), (4, SL_COS, true)] {
        let d = cpu.read16(o + DIRECTION) as i16 as i32 as u32;
        let t = cpu.call(trig, &[d]) as i32;
        let t = if neg { t.wrapping_neg() } else { t };
        let pace = cpu.read16(o + PACE) as i16 as i32;
        let step = t.wrapping_mul((pace / 16) as i16 as i32).wrapping_abs();
        let d = cpu.read16(o + DIRECTION) as i16 as i32 as u32;
        let t = cpu.call(trig, &[d]);
        let t = if neg { t.wrapping_neg() } else { t };
        let target = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, t, (pace << 4) as u32));
        let v = cpu.read32(o + SPEED + k);
        let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, target, step as u32));
        cpu.write32(o + SPEED + k, v);
    }
}

/// The actor moved by its speed (x mirrored by its side) and with the ground (y), at its height;
/// hittable only while low.
pub fn route_move(cpu: &mut Cpu, o: u32) {
    let mirror = cpu.read8(o + MIRROR) as i8 as i32;
    let x = cpu.read32(o + POSITION).wrapping_add((cpu.read32(o + SPEED) as i32).wrapping_mul(mirror) as u32);
    cpu.write32(o + POSITION, x);
    let ground = cpu.read32(super::globals::GROUND_DY_AT);
    let dy = cpu.read32(o + SPEED + 4).wrapping_sub(cpu.read32(ground));
    let y = cpu.read32(o + POSITION + 4).wrapping_add(dy);
    cpu.write32(o + POSITION + 4, y);
    let h = cpu.read32(o + HEIGHT);
    cpu.write32(o + POSITION + 8, h);
    let flags = cpu.read32(o + FLAGS);
    let flags = if (h as i32).wrapping_abs() > LOW { flags | NO_HITS } else { flags & !NO_HITS };
    cpu.write32(o + FLAGS, flags);
}

/// A gun's aim script (bytes): the first aim (its high byte); then steps: flag bytes (odd: their
/// low 4 bits returned as the step starts; with 0x40 the next byte jumps back that far and one
/// more), the frames the step lasts and the aim it turns to over them (its high byte). 0x81 ends
/// it.
const AIM: u32 = 0x8a;
const AIM_TURN: u32 = 0x98;
const AIM_FRAMES_LEFT: u32 = 0x9a;
const AIM_AT: u32 = 0x9c;
const JUMP_BACK: u8 = 0x40;

/// One frame of the actor's aim `script`: a new step taken when the last is over (its aim set
/// from the script's unless `keep`), else the aim turned by the step's turn. The new step's last
/// flag byte's low 4 bits, else 0.
pub fn aim_step(cpu: &mut Cpu, o: u32, script: u32, keep: u32) -> u32 {
    let keep = keep as u8;
    let at = cpu.read16(o + AIM_AT) as i16 as i32 as u32;
    if cpu.read8(script.wrapping_add(at)) == END {
        return 0;
    }
    let mut flags = 0;
    if cpu.read16(o + AIM_FRAMES_LEFT) != 0 {
        let a = cpu.read16(o + AIM).wrapping_add(cpu.read16(o + AIM_TURN));
        cpu.write16(o + AIM, a);
    } else {
        let next = if at == 0 {
            let a = (cpu.read8(script) as u16) << 8;
            cpu.write16(o + AIM, a);
            cpu.read16(o + AIM_AT).wrapping_add(1)
        } else {
            if keep == 0 {
                let a = (cpu.read8(script.wrapping_add(at + 1)) as u16) << 8;
                cpu.write16(o + AIM, a);
            }
            cpu.read16(o + AIM_AT).wrapping_add(2)
        };
        cpu.write16(o + AIM_AT, next);
        let byte = |cpu: &mut Cpu| {
            let at = cpu.read16(o + AIM_AT) as i16 as i32 as u32;
            cpu.read8(script.wrapping_add(at))
        };
        if byte(cpu) != END {
            while byte(cpu) & FLAG_BYTE != 0 {
                let b = byte(cpu);
                let at = cpu.read16(o + AIM_AT).wrapping_add(1);
                cpu.write16(o + AIM_AT, at);
                if b & JUMP_BACK != 0 {
                    let back = byte(cpu) as u16 + 1;
                    cpu.write16(o + AIM_AT, at.wrapping_sub(back));
                }
                flags = (b & 0xf) as u32;
            }
            let p = script.wrapping_add(cpu.read16(o + AIM_AT) as i16 as i32 as u32);
            let frames = cpu.read8(p) as u16;
            cpu.write16(o + AIM_FRAMES_LEFT, frames);
            let to = (cpu.read8(p + 1) as u32) << 8;
            let from = cpu.read16(o + AIM) as u32;
            let d = nested!(cpu, 0x0605_6580, aim::angle_between(cpu, from, to)) as i16 as i32 as u32;
            let frames = cpu.read16(o + AIM_FRAMES_LEFT) as i16 as i32 as u32;
            let turn = nested!(cpu, 0x0600_6a78, math::quotient(cpu, d, frames));
            cpu.write16(o + AIM_TURN, turn as u16);
        }
    }
    let n = cpu.read16(o + AIM_FRAMES_LEFT).wrapping_sub(1);
    cpu.write16(o + AIM_FRAMES_LEFT, n);
    flags
}

/// The route flags: ends once outside its area (with 0x1000), firing (0x10). The flag bytes that
/// start and stop firing.
const FIRING: u16 = 0x10;
const START_FIRING: u32 = 7;
const STOP_FIRING: u32 = 9;
/// The area a route that ends leaving it keeps to (top, bottom, left, right: 16.16).
const ROUTE_AREA: [u32; 4] = [0xff20_0000, 0x00e0_0000, 0xfee0_0000, 0x0120_0000];
const SPRITE: u32 = 0x10;
/// The route script's address.
const SCRIPT: u32 = 0x6c;

/// A thing moving along its route script (ending once outside its area if its route says so),
/// told by the script when to fire.
pub fn route_exec(cpu: &mut Cpu, o: u32) {
    if cpu.read16(o + ROUTE_FLAGS) & ROUTE_1000 != 0 {
        let [top, bottom, left, right] = ROUTE_AREA;
        let f = cpu.nested_with_stack(&aim::AREA_FLAGS_PROLOGUE, aim::AREA_FLAGS, &[right], |cpu| aim::area_flags(cpu, o + SPRITE, top, bottom, left, right));
        if f as u8 != 0 {
            cpu.write8(o + OBJ_MODE, ENDING);
            return;
        }
    }
    let script = cpu.read32(o + SCRIPT);
    match nested!(cpu, 0x0605_a000, route_step(cpu, o, script)) & 0xff {
        START_FIRING => {
            let f = cpu.read16(o + ROUTE_FLAGS) | FIRING;
            cpu.write16(o + ROUTE_FLAGS, f);
        }
        STOP_FIRING => {
            let f = cpu.read16(o + ROUTE_FLAGS) & !FIRING;
            cpu.write16(o + ROUTE_FLAGS, f);
        }
        _ => {}
    }
    nested!(cpu, 0x0605_a460, route_move(cpu, o));
}

/// A thing riding on another (at +0x6c), at its offset from it (pixel words at +0x8a, +0x8c), not
/// hittable while that one is not; ended once that one ends.
const RIDES_ON: u32 = 0x6c;
const RIDE_OFFSET: u32 = 0x8a;

pub fn ride(cpu: &mut Cpu, o: u32) {
    let on = cpu.read32(o + RIDES_ON);
    if cpu.read8(on + OBJ_MODE) as i8 >= ENDING as i8 {
        cpu.write8(o + OBJ_MODE, ENDING);
        return;
    }
    let flags = cpu.read32(o + FLAGS) & !NO_HITS;
    cpu.write32(o + FLAGS, flags);
    let hits = cpu.read32(on + FLAGS) & NO_HITS;
    cpu.write32(o + FLAGS, flags | hits);
    for k in 0..2 {
        let p = cpu.read32(on + POSITION + 4 * k).wrapping_add((cpu.read16(o + RIDE_OFFSET + 2 * k) as i16 as i32 as u32) << 16);
        cpu.write32(o + POSITION + 4 * k, p);
    }
    let z = cpu.read32(on + POSITION + 8);
    cpu.write32(o + POSITION + 8, z);
}


/// A ground enemy (a tank or the like) moving along its route script: it dies where the ground's
/// map says it cannot be (its life -1) or when its life runs out (an explosion by its kind; the
/// second kind, above the middle difficulty, a parting shot first), ends off its area if its
/// route says so; its frame follows its direction (by its kind and settings: the first kind's
/// turning on by a frame each frame or picked from its direction by 32nds, 16ths or 8ths; the
/// second's and third's from their tables), its angle its direction (mirrored by its side), and
/// while firing it fires a plain bullet at the player every so often (by its settings).
const LIFE: u32 = 0x50;
const KIND_OF: u32 = 0xa6;
const SETTINGS: u32 = 0x7c;
const FIRE_WAIT: u32 = 0x8a;
const FRAME: u32 = 0x2f;
const ANGLE: u32 = 0x30;
const NUMBER: u32 = 0x32;
/// The settings: frames between shots (by 8s, less 8: the low 4 bits), how the first kind's frame
/// follows its direction (bits 4-5).
const FIRE_EVERY: u8 = 0xf;
const FRAME_BY: u8 = 0x30;
/// The area a ground enemy that ends off it keeps to (top, bottom, left, right).
const GROUND_AREA: [u32; 4] = [0xff80_0000, 0x0080_0000, 0xff44_0000, 0x00bc_0000];
/// The second and third kinds' frames by direction (every other byte).
const KIND_1_FRAMES: u32 = 0x0607_3481;
const KIND_2_FRAMES: u32 = 0x0607_34c1;
const FLIPPED_Y: u32 = 0x20;
/// Not rewritten yet: a parting shot, the kinds' explosions.
const PARTING_SHOT: u32 = 0x0605_6070;
const BLOWN_UP_1: u32 = 0x0605_9e9c;
const BLOWN_UP: u32 = 0x0605_9e4c;
const DIFFICULTY: u32 = 0x0600_ed84;
/// The shot's description (bullets.rs's): its place, number, direction.
const S_NUMBER: u32 = 0x1e;
const S_ANGLE: u32 = 0x2c;
const AIM_DELAY: u32 = 16;

pub fn ground_exec(cpu: &mut Cpu, o: u32) {
    let f = cpu.stack();
    let (x, y) = (cpu.read32(o + POSITION), cpu.read32(o + POSITION + 4));
    if nested!(cpu, 0x0605_ef90, super::bg::map_hit(cpu, x, y)) != 0 {
        cpu.write32(o + LIFE, u32::MAX);
    }
    let alive = |cpu: &mut Cpu| cpu.read32(o + LIFE) as i32 > 0;
    let leaves = alive(cpu)
        && cpu.read16(o + ROUTE_FLAGS) & ROUTE_1000 != 0
        && {
            let [top, bottom, left, right] = GROUND_AREA;
            cpu.nested_with_stack(&aim::AREA_FLAGS_PROLOGUE, aim::AREA_FLAGS, &[right], |cpu| aim::area_flags(cpu, o + SPRITE, top, bottom, left, right)) as u8 != 0
        };
    if !alive(cpu) || leaves {
        if !alive(cpu) {
            let kind = cpu.read16(o + KIND_OF) as i16;
            if kind == 2 && cpu.read8(DIFFICULTY) as i8 > 2 {
                cpu.call(PARTING_SHOT, &[o, 0]);
            }
            let blown = if cpu.read16(o + KIND_OF) == 1 { BLOWN_UP_1 } else { BLOWN_UP };
            cpu.call(blown, &[o]);
        }
        cpu.write8(o + OBJ_MODE, ENDING);
        return;
    }
    let script = cpu.read32(o + SCRIPT);
    match nested!(cpu, 0x0605_a000, route_step(cpu, o, script)) & 0xff {
        START_FIRING => {
            let fl = cpu.read16(o + ROUTE_FLAGS) | FIRING;
            cpu.write16(o + ROUTE_FLAGS, fl);
        }
        STOP_FIRING => {
            let fl = cpu.read16(o + ROUTE_FLAGS) & !FIRING;
            cpu.write16(o + ROUTE_FLAGS, fl);
            cpu.write16(o + FIRE_WAIT, 0);
        }
        _ => {}
    }
    nested!(cpu, 0x0605_a460, route_move(cpu, o));
    let direction = |cpu: &mut Cpu| cpu.read16(o + DIRECTION) as i16 as i32;
    match cpu.read16(o + KIND_OF) as i16 {
        0 => {
            let frame = match cpu.read8(o + SETTINGS) & FRAME_BY {
                0 => Some(cpu.read8(o + FRAME) as i32 + 1),
                0x10 => Some(direction(cpu) / 0x400),
                0x20 => Some(direction(cpu) / 0x200),
                0x30 => Some(direction(cpu) / 0x100),
                _ => None,
            };
            if let Some(frame) = frame {
                cpu.write8(o + FRAME, (frame & 31) as u8);
            }
        }
        1 => {
            let flags = cpu.read32(o + FLAGS) | FLIPPED_Y;
            cpu.write32(o + FLAGS, flags);
            let i = (direction(cpu).wrapping_neg() / 0x800) & 31;
            let frame = cpu.read8(KIND_1_FRAMES + 2 * i as u32);
            cpu.write8(o + FRAME, frame);
        }
        2 => {
            let i = (direction(cpu) / 0x1000) & 15;
            let frame = cpu.read8(KIND_2_FRAMES + 2 * i as u32);
            cpu.write8(o + FRAME, frame);
        }
        _ => {}
    }
    let mirror = cpu.read8(o + MIRROR) as i8 as i16;
    let d = cpu.read16(o + DIRECTION) as i16;
    cpu.write16(o + ANGLE, mirror.wrapping_mul(d) as u16);
    if cpu.read16(o + ROUTE_FLAGS) & FIRING == 0 {
        return;
    }
    let wait = cpu.read16(o + FIRE_WAIT).wrapping_sub(1);
    cpu.write16(o + FIRE_WAIT, wait);
    if wait as i16 >= 0 {
        return;
    }
    let n = cpu.read8(o + NUMBER);
    cpu.write8(f + S_NUMBER, n);
    let every = (cpu.read8(o + SETTINGS) & FIRE_EVERY) as u16;
    cpu.write16(o + FIRE_WAIT, every.wrapping_mul(8).wrapping_sub(8));
    for k in 0..3 {
        let v = cpu.read32(o + POSITION + 4 * k);
        cpu.write32(f + 4 * k, v);
    }
    let a = nested!(cpu, 0x0605_6ad4, aim::angle_to_target(cpu, o, AIM_DELAY));
    cpu.write16(f + S_ANGLE, a as u16);
    nested!(cpu, 0x0605_3f48, super::bullets::bullet_set_plain(cpu, f));
}

/// A ground gun (on a tank or the like): along its route script as ground_exec, its gun turned by
/// its aim script; while firing, if its route flags say so, it tracks the player instead (turning
/// toward where the player was 10 frames ago, at most 0x1000 a frame, easing over 10, by 0x18 at
/// most). Off its area (if its route says so) it ends; out of life it blows up.
const TRACKS: u16 = 0x20;
const TRACKING: u16 = 0x40;
const GUN_AREA: [u32; 4] = [0xff40_0000, 0x00c0_0000, 0xff10_0000, 0x00f0_0000];
/// The gun's aim script, and its turn and turning speed while tracking (words).
const AIM_SCRIPT: u32 = 0x70;
const TRACK_TURN: u32 = 0xa2;
const TRACK_SPEED: u32 = 0xa4;
const TRACK_DELAY: u32 = 10;
const TRACK_MOST: u32 = 0x1000;
const TRACK_EASE: u32 = 10;
const TRACK_ACCEL: u32 = 0x18;
const TURN_TOWARD: u32 = 0x0605_70d8;
static TURN_TOWARD_PROLOGUE: std::sync::OnceLock<super::Prologue> = std::sync::OnceLock::new();

pub fn gun_exec(cpu: &mut Cpu, o: u32) {
    let alive = |cpu: &mut Cpu| cpu.read32(o + LIFE) as i32 > 0;
    let leaves = alive(cpu)
        && cpu.read16(o + ROUTE_FLAGS) & ROUTE_1000 != 0
        && {
            let [top, bottom, left, right] = GUN_AREA;
            cpu.nested_with_stack(&aim::AREA_FLAGS_PROLOGUE, aim::AREA_FLAGS, &[right], |cpu| aim::area_flags(cpu, o + SPRITE, top, bottom, left, right)) as u8 != 0
        };
    if !alive(cpu) || leaves {
        if !alive(cpu) {
            cpu.call(BLOWN_UP_1, &[o]);
        }
        cpu.write8(o + OBJ_MODE, ENDING);
        return;
    }
    let script = cpu.read32(o + SCRIPT);
    match nested!(cpu, 0x0605_a000, route_step(cpu, o, script)) & 0xff {
        START_FIRING => {
            let fl = cpu.read16(o + ROUTE_FLAGS) | FIRING;
            cpu.write16(o + ROUTE_FLAGS, fl);
            if fl & TRACKS != 0 {
                cpu.write16(o + ROUTE_FLAGS, fl | TRACKING);
            }
        }
        STOP_FIRING => {
            let fl = cpu.read16(o + ROUTE_FLAGS) & !FIRING & !TRACKING;
            cpu.write16(o + ROUTE_FLAGS, fl);
        }
        _ => {}
    }
    nested!(cpu, 0x0605_a460, route_move(cpu, o));
    let mirror = |cpu: &mut Cpu| cpu.read8(o + MIRROR) as i8 as i16;
    if cpu.read16(o + ROUTE_FLAGS) & TRACKING != 0 {
        let to = nested!(cpu, 0x0605_6ad4, aim::angle_to_target(cpu, o, TRACK_DELAY)) & 0xffff;
        let from = (mirror(cpu).wrapping_mul(cpu.read16(o + AIM) as i16) as u16) as u32;
        let (turn, speed) = (o + TRACK_TURN, o + TRACK_SPEED);
        cpu.nested_with_stack(&TURN_TOWARD_PROLOGUE, TURN_TOWARD, &[TRACK_MOST, TRACK_EASE, TRACK_ACCEL], |cpu| aim::turn_toward(cpu, turn, speed, from, to, TRACK_MOST, TRACK_EASE, TRACK_ACCEL));
        let by = mirror(cpu).wrapping_mul(cpu.read16(o + TRACK_TURN) as i16) as u16;
        let a = by.wrapping_add(cpu.read16(o + AIM));
        cpu.write16(o + AIM, a);
    } else {
        cpu.write16(o + TRACK_TURN, 0);
        cpu.write16(o + TRACK_SPEED, 0);
        let script = cpu.read32(o + AIM_SCRIPT);
        nested!(cpu, 0x0605_a340, aim_step(cpu, o, script, 1));
    }
    let a = mirror(cpu).wrapping_mul(cpu.read16(o + AIM) as i16) as u16;
    cpu.write16(o + ANGLE, a);
}

/// A ground enemy that shows its wounds: along its route script as route_exec does; once below
/// 2500 life it smokes (a puff each 8 frames, near it, flipped at random) and, while firing,
/// unless it is the plain kind (its boxes), it fires pairs of shots 24 pixels either side, twice
/// the difficulty's number of pairs, every 120 frames less 12 for each step of difficulty. Out of
/// life it blows up (by its kind).
const BOXES: u32 = 0x3c;
const PLAIN_BOXES: u32 = 0x0607_3338;
const WOUNDED: i32 = 0x9c3;
const WOUND_WAIT: u32 = 0xa2;
const SIDE_SHOT: u32 = 0x0605_b4c0;
const SIDE_SHOT_X: [u32; 2] = [0xffe8_0000, 0x18_0000];
/// Not rewritten yet: the wounded kind's explosion.
const BLOWN_UP_WOUNDED: u32 = 0x0605_9ee4;
/// The smoke (actors.rs's drifting effect): its animation, colour (attribute bits at random), how
/// far out it starts (up to 4 pixels each way, 4 units nearer), its scales and speed.
const SMOKE_SCRIPT: u32 = 0x0607_310c;
const SMOKE_COLOUR: u16 = 0xd0;
const SMOKE_FLIPS: u32 = 0xa000;
const SMOKE_SPREAD: u32 = 0x4_0000;
const SMOKE_SCALE: u32 = 0x8000;
const SMOKE_SPEED: u32 = 0xc000;
const GAME_FRAMES: u32 = super::globals::GAME_FRAMES;

pub fn wounded_exec(cpu: &mut Cpu, o: u32) {
    let f = cpu.stack();
    let alive = |cpu: &mut Cpu| cpu.read32(o + LIFE) as i32 > 0;
    let leaves = alive(cpu)
        && cpu.read16(o + ROUTE_FLAGS) & ROUTE_1000 != 0
        && {
            let [top, bottom, left, right] = ROUTE_AREA;
            cpu.nested_with_stack(&aim::AREA_FLAGS_PROLOGUE, aim::AREA_FLAGS, &[right], |cpu| aim::area_flags(cpu, o + SPRITE, top, bottom, left, right)) as u8 != 0
        };
    if !alive(cpu) || leaves {
        if !alive(cpu) {
            let blown = if cpu.read32(o + BOXES) == PLAIN_BOXES { BLOWN_UP_1 } else { BLOWN_UP_WOUNDED };
            cpu.call(blown, &[o]);
        }
        cpu.write8(o + OBJ_MODE, ENDING);
        return;
    }
    let script = cpu.read32(o + SCRIPT);
    match nested!(cpu, 0x0605_a000, route_step(cpu, o, script)) & 0xff {
        START_FIRING => {
            let fl = cpu.read16(o + ROUTE_FLAGS) | FIRING;
            cpu.write16(o + ROUTE_FLAGS, fl);
        }
        STOP_FIRING => {
            let fl = cpu.read16(o + ROUTE_FLAGS) & !FIRING;
            cpu.write16(o + ROUTE_FLAGS, fl);
        }
        _ => {}
    }
    nested!(cpu, 0x0605_a460, route_move(cpu, o));
    let wounded = |cpu: &mut Cpu| cpu.read32(o + LIFE) as i32 <= WOUNDED;
    if wounded(cpu) && cpu.read32(GAME_FRAMES) & 7 == 0 {
        use super::actors::{E_ANGLE, E_COLOUR, E_POSITION, E_SCALES, E_SCRIPT, E_SPEED};
        cpu.write32(f + E_SCRIPT, SMOKE_SCRIPT);
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
        cpu.write16(f + E_COLOUR, (r & SMOKE_FLIPS) as u16 | SMOKE_COLOUR);
        for k in 0..2 {
            let r = nested!(cpu, 0x0604_5d90, math::random(cpu, SMOKE_SPREAD));
            let p = cpu.read32(o + POSITION + 4 * k).wrapping_add(r);
            cpu.write32(f + E_POSITION + 4 * k, p);
        }
        let z = cpu.read32(o + POSITION + 8).wrapping_sub(SMOKE_SPREAD);
        cpu.write32(f + E_POSITION + 8, z);
        cpu.write16(f + E_ANGLE, 0);
        cpu.write32(f + E_SPEED, SMOKE_SPEED);
        cpu.write32(f + E_SCALES, SMOKE_SCALE);
        cpu.write32(f + E_SCALES + 4, SMOKE_SCALE);
        nested!(cpu, 0x0605_7454, actors::drift_effect(cpu, f));
    }
    if cpu.read16(o + ROUTE_FLAGS) & FIRING == 0 || cpu.read32(o + BOXES) == PLAIN_BOXES || !wounded(cpu) {
        return;
    }
    let wait = cpu.read16(o + WOUND_WAIT).wrapping_add(1);
    cpu.write16(o + WOUND_WAIT, wait);
    let d = cpu.read8(DIFFICULTY) as i8 as i32;
    if (wait as i16 as i32) <= 120 - d * 12 {
        return;
    }
    cpu.write16(o + WOUND_WAIT, 0);
    let mut i = 0;
    while i < cpu.read8(DIFFICULTY) as i8 as i32 * 2 {
        for x in SIDE_SHOT_X {
            cpu.call(SIDE_SHOT, &[o, x]);
        }
        i = (i + 1) & 0xff;
    }
}

/// A rocket launcher: mounted on a carrier (at its offset, or 48 pixels out along the carrier's
/// angle plus its own turn if the carrier turns its parts; hittable as the carrier is, firing when
/// it fires) or standing on the ground (moving with it, firing while within 104 pixels of the
/// middle up and down). It flashes with what it is told to (if anything) instead of firing; else
/// it fires a rocket (from 6 pixels out along its angle) each so many frames (by its rate, less
/// for each step of difficulty). Off screen it ends (mounted: if its carrier's route says so;
/// standing: once it has been on screen); with its carrier gone it ends; out of life it blows up
/// (sound 12, a burst; above difficulty 3 a parting shot).
const L_CARRIER: u32 = 0x6c;
const L_FLASH_WITH: u32 = 0x74;
const L_SEEN: u32 = 0x88;
const L_OFFSET: u32 = 0x8a;
const L_RATE: u32 = 0x8e;
const L_WAIT: u32 = 0xa6;
const L_TURN: u32 = 0xa8;
/// The carrier's flags: it turns its parts.
const TURNS_PARTS: u32 = 0x1000_0000;
const L_OUT: i32 = 0x30;
const L_MUZZLE: i32 = 6;
const L_FIRING_BAND: u32 = 0x67_ffff;
const L_FIRING_SPAN: u32 = 0xcf_fffe;
/// The rocket the launcher fires (bullets.rs's shot description: its frames speeding up, its
/// flags: the second colour pair, from the ground).
const S_FLAGS: u32 = 0x2e;
const S_ROCKET_FLAGS: u32 = 0x30;
const L_ROCKET_FRAMES: u16 = 8;
const L_ROCKET_FLAGS: u16 = 9;
const SE_BLOWN: u32 = 12;
const L_BURST: u32 = 0x0607_3044;
const L_BURST_COLOUR: u16 = 0x90;

pub fn launcher_exec(cpu: &mut Cpu, o: u32) {
    let f = cpu.stack();
    let c = cpu.read32(o + L_CARRIER);
    let off = |cpu: &mut Cpu| nested!(cpu, 0x0605_6a24, aim::off_screen(cpu, o + SPRITE)) as u8 != 0;
    if c == 0 && !off(cpu) {
        cpu.write8(o + L_SEEN, 1);
    }
    let gone = if c != 0 {
        off(cpu) && cpu.read16(c + ROUTE_FLAGS) & ROUTE_1000 != 0
    } else {
        off(cpu) && cpu.read8(o + L_SEEN) == 1
    };
    if gone {
        cpu.write8(o + OBJ_MODE, ENDING);
        return;
    }
    if cpu.read32(o + LIFE) as i32 > 0 {
        if cpu.read8(c + OBJ_MODE) as i8 >= ENDING as i8 {
            cpu.write8(o + OBJ_MODE, ENDING);
            return;
        }
    } else {
        if cpu.read8(DIFFICULTY) as i8 > 3 {
            cpu.call(PARTING_SHOT, &[o, 0]);
        }
        nested!(cpu, 0x0600_50d4, super::sound::se_set(cpu, SE_BLOWN));
        use super::actors::{E_ANGLE, E_COLOUR, E_POSITION, E_SCALES, E_SCRIPT, E_SPEED};
        cpu.write32(f + E_SCRIPT, L_BURST);
        cpu.write16(f + E_COLOUR, L_BURST_COLOUR);
        for k in 0..3 {
            let v = cpu.read32(o + POSITION + 4 * k);
            cpu.write32(f + E_POSITION + 4 * k, v);
        }
        let a = cpu.read16(o + ANGLE);
        cpu.write16(f + E_ANGLE, a);
        cpu.write32(f + E_SPEED, 0x1_0000);
        cpu.write32(f + E_SCALES, 0x1_0000);
        cpu.write32(f + E_SCALES + 4, 0x1_0000);
        nested!(cpu, 0x0605_7454, actors::drift_effect(cpu, f));
        cpu.write8(o + OBJ_MODE, ENDING);
        return;
    }
    if c != 0 {
        let flags = cpu.read32(o + FLAGS) & !NO_HITS;
        cpu.write32(o + FLAGS, flags);
        let hits = cpu.read32(c + FLAGS) & NO_HITS;
        cpu.write32(o + FLAGS, flags | hits);
        if cpu.read32(c + FLAGS) & TURNS_PARTS != 0 {
            let a = cpu.read16(o + L_TURN).wrapping_add(cpu.read16(c + ANGLE));
            cpu.write16(o + ANGLE, a);
            let s = cpu.call(SL_SIN, &[a as i16 as i32 as u32]) as i32;
            let x = (s.wrapping_mul(L_OUT) as u32).wrapping_add(cpu.read32(c + POSITION));
            cpu.write32(o + POSITION, x);
            let a = cpu.read16(o + ANGLE) as i16 as i32 as u32;
            let co = cpu.call(SL_COS, &[a]) as i32;
            let y = (co.wrapping_mul(-L_OUT) as u32).wrapping_add(cpu.read32(c + POSITION + 4));
            cpu.write32(o + POSITION + 4, y);
        } else {
            for k in 0..2 {
                let p = cpu.read32(c + POSITION + 4 * k).wrapping_add((cpu.read16(o + L_OFFSET + 2 * k) as i16 as i32 as u32) << 16);
                cpu.write32(o + POSITION + 4 * k, p);
            }
        }
        let z = cpu.read32(c + POSITION + 8);
        cpu.write32(o + POSITION + 8, z);
        let fl = cpu.read16(c + ROUTE_FLAGS);
        cpu.write16(o + ROUTE_FLAGS, fl);
    } else {
        let y = cpu.read32(o + POSITION + 4);
        let firing = if y.wrapping_add(L_FIRING_BAND) > L_FIRING_SPAN { 0 } else { FIRING };
        cpu.write16(o + ROUTE_FLAGS, firing);
        for (k, at) in [(0, super::globals::GROUND_DX_AT), (4, super::globals::GROUND_DY_AT)] {
            let g = cpu.read32(at);
            let p = cpu.read32(o + POSITION + k).wrapping_sub(cpu.read32(g));
            cpu.write32(o + POSITION + k, p);
        }
    }
    let with = cpu.read32(o + L_FLASH_WITH);
    if with != 0 {
        nested!(cpu, 0x0604_668c, super::parts::flash_against(cpu, o, with));
        return;
    }
    let wait = cpu.read16(o + L_WAIT) as i16 as i32 as u32;
    let wait = nested!(cpu, 0x0604_6168, actors::approach(cpu, wait, 0, 1)) as i16;
    cpu.write16(o + L_WAIT, wait as u16);
    if cpu.read16(o + ROUTE_FLAGS) & FIRING == 0 || wait != 0 {
        return;
    }
    let d = cpu.read8(DIFFICULTY) as i8 as i16;
    let rate = (cpu.read16(o + L_RATE) as i16).wrapping_sub(d);
    cpu.write16(o + L_WAIT, rate.wrapping_mul(8).wrapping_add(8) as u16);
    let n = cpu.read8(o + NUMBER);
    cpu.write8(f + S_NUMBER, n);
    let a = cpu.read16(o + ANGLE);
    cpu.write16(f + S_ANGLE, a);
    cpu.write16(f + S_FLAGS, L_ROCKET_FRAMES);
    cpu.write16(f + S_ROCKET_FLAGS, L_ROCKET_FLAGS);
    let s = cpu.call(SL_SIN, &[a as i16 as i32 as u32]) as i32;
    let x = cpu.read32(o + POSITION).wrapping_add(s.wrapping_mul(L_MUZZLE) as u32);
    cpu.write32(f, x);
    let a = cpu.read16(o + ANGLE) as i16 as i32 as u32;
    let co = cpu.call(SL_COS, &[a]) as i32;
    let y = cpu.read32(o + POSITION + 4).wrapping_add(co.wrapping_mul(-L_MUZZLE) as u32);
    cpu.write32(f + 4, y);
    cpu.write32(f + 8, 0);
    nested!(cpu, 0x0605_5b5c, super::bullets::rocket_set(cpu, f));
}

/// A turret on a vehicle (at +0x6c): while the vehicle fires and the player (as 12 frames ago) is
/// within 0x4800 of the vehicle's facing it turns toward the player (0x80 a frame) and fires a
/// plain bullet every so often (by its rate, less each step of difficulty), recoiling a pixel in
/// turn; else it swings 0x800 either side of the vehicle's facing (32 frames each way). It keeps
/// within a quarter turn of that facing, sits on the vehicle, is hittable as it is, flashes with
/// it, and gives it its damage (scoring the kill by its kind's points if that ends it).
const T_FACING: u32 = 0x8a;
const T_WAIT: u32 = 0x8c;
const T_RATE: u32 = 0x8e;
const HIT_OTHER: u32 = 0x40;
const HIT_FLAGS: u32 = 0x56;
const WAS_HIT: u16 = 0x200;
const KIND: u32 = 0x2e;
const TYPE: u32 = 0x55;
const T_DELAY: u32 = 12;
const T_CONE: u16 = 0x4800;
const T_CONE_SPAN: u16 = 0x9000;
const T_SWING: u16 = 0x800;
const T_TURN: u16 = 0x80;
const T_LIMIT: i16 = 0x4000;
const T_MUZZLE: i32 = 10;
const ONE: u32 = 0x1_0000;
/// The kinds' points (the kinds' table, sprites.rs's: a word at +0xe).
const KIND_POINTS: u32 = 0x0607_41fe;

pub fn turret_exec(cpu: &mut Cpu, o: u32) {
    let f = cpu.stack();
    let v = cpu.read32(o + RIDES_ON);
    if cpu.read8(v + OBJ_MODE) as i8 >= ENDING as i8 {
        cpu.write8(o + OBJ_MODE, ENDING);
        return;
    }
    let facing = cpu.read16(v + ANGLE);
    cpu.write16(o + T_FACING, facing);
    let aim = nested!(cpu, 0x0605_6ad4, aim::angle_to_target(cpu, o, T_DELAY)) & 0xffff;
    let facing = cpu.read16(o + T_FACING) as u32;
    let d = nested!(cpu, 0x0605_6580, aim::angle_between(cpu, aim, facing)) as u16;
    let shooting = cpu.read16(v + ROUTE_FLAGS) & FIRING != 0 && d.wrapping_add(T_CONE) <= T_CONE_SPAN;
    let target = if shooting {
        aim
    } else {
        let swing = ((cpu.read32(GAME_FRAMES) & 0x20) << 7) as u16;
        (cpu.read16(o + T_FACING).wrapping_add(T_SWING).wrapping_sub(swing)) as u32
    };
    let angle = cpu.read16(o + ANGLE) as u32;
    let d = nested!(cpu, 0x0605_6580, aim::angle_between(cpu, target, angle)) as i16;
    let a = if d.wrapping_abs() <= 0x7f {
        target as u16
    } else if d > 0 {
        cpu.read16(o + ANGLE).wrapping_sub(T_TURN)
    } else {
        cpu.read16(o + ANGLE).wrapping_add(T_TURN)
    };
    cpu.write16(o + ANGLE, a);
    let off = cpu.read16(o + ANGLE).wrapping_sub(cpu.read16(o + T_FACING)) as i16;
    if off > T_LIMIT {
        let a = cpu.read16(o + ANGLE).wrapping_add(T_LIMIT as u16).wrapping_sub(off as u16);
        cpu.write16(o + ANGLE, a);
    }
    if off < -T_LIMIT {
        let a = cpu.read16(o + ANGLE).wrapping_add((-T_LIMIT) as u16).wrapping_sub(off as u16);
        cpu.write16(o + ANGLE, a);
    }
    let recoil = (cpu.read32(GAME_FRAMES) >> 2) & shooting as u32;
    let a = cpu.read16(o + ANGLE) as i16 as i32 as u32;
    let s = cpu.call(SL_SIN, &[a]);
    let x = cpu.read32(v + POSITION).wrapping_add(s.wrapping_mul(recoil));
    cpu.write32(o + POSITION, x);
    let a = cpu.read16(o + ANGLE) as i16 as i32 as u32;
    let c = cpu.call(SL_COS, &[a]);
    let y = cpu.read32(v + POSITION + 4).wrapping_add(c.wrapping_neg().wrapping_mul(recoil));
    cpu.write32(o + POSITION + 4, y);
    let z = cpu.read32(v + POSITION + 8);
    cpu.write32(o + POSITION + 8, z);
    let flags = cpu.read32(o + FLAGS) & !NO_HITS;
    cpu.write32(o + FLAGS, flags);
    let hits = cpu.read32(v + FLAGS) & NO_HITS;
    cpu.write32(o + FLAGS, flags | hits);
    let wait = cpu.read16(o + T_WAIT) as i16 as i32 as u32;
    let wait = nested!(cpu, 0x0604_6168, actors::approach(cpu, wait, 0, 1));
    cpu.write16(o + T_WAIT, wait as u16);
    if shooting && wait as i16 == 0 {
        let d = cpu.read8(DIFFICULTY) as i8 as i16;
        let rate = (cpu.read16(o + T_RATE) as i16).wrapping_sub(d);
        cpu.write16(o + T_WAIT, rate.wrapping_mul(8).wrapping_add(8) as u16);
        let n = cpu.read8(o + NUMBER);
        cpu.write8(f + S_NUMBER, n);
        let a = cpu.read16(o + ANGLE) as i16 as i32 as u32;
        let s = cpu.call(SL_SIN, &[a]) as i32;
        let x = cpu.read32(o + POSITION).wrapping_add(s.wrapping_mul(T_MUZZLE) as u32);
        cpu.write32(f, x);
        let a = cpu.read16(o + ANGLE) as i16 as i32 as u32;
        let c = cpu.call(SL_COS, &[a]) as i32;
        let y = cpu.read32(o + POSITION + 4).wrapping_add(c.wrapping_neg().wrapping_mul(T_MUZZLE) as u32);
        cpu.write32(f + 4, y);
        let z = cpu.read32(o + POSITION + 8);
        cpu.write32(f + 8, z);
        let a = cpu.read16(o + ANGLE);
        cpu.write16(f + S_ANGLE, a);
        nested!(cpu, 0x0605_3f48, super::bullets::bullet_set_plain(cpu, f));
    }
    let life = cpu.read32(v + LIFE);
    if life as i32 > 0 && cpu.read16(o + HIT_FLAGS) & WAS_HIT != 0 {
        let life = life.wrapping_sub(ONE).wrapping_add(cpu.read32(o + LIFE));
        cpu.write32(v + LIFE, life);
        if life as i32 <= 0 {
            let kind = cpu.read8(o + KIND) as i8 as i32;
            let points = cpu.read16(KIND_POINTS.wrapping_add((kind * 16) as u32)) as i16 as i32 as u32;
            let killer = cpu.read32(o + HIT_OTHER);
            let weapon = cpu.read8(killer + TYPE) as i8 as i32 as u32;
            let p = cpu.read8(killer + NUMBER) as i8 as i32 as u32;
            nested!(cpu, 0x0604_3108, super::hits::score_kill(cpu, p, weapon, o, points));
        }
    }
    cpu.write32(o + LIFE, ONE);
    nested!(cpu, 0x0604_666c, super::parts::flash_with(cpu, o, v));
}

/// A big gun's body check: the gun ends with its body (the numbered object of its number), blown
/// off once the body is below 2500 life (above difficulty 3 with a parting shot); else it sits on
/// the body in its colour. 1 when it ends, else 0.
const GUN_BLOWN_OFF: u32 = 0x0605_9f50;
const NUMBERED: u32 = 0x0607_40f0;
const OWN_COLOUR: u32 = 0x2a;

pub fn on_body(cpu: &mut Cpu, o: u32) -> u32 {
    let n = cpu.read8(o + NUMBER) as i8 as i32;
    let body = cpu.read32(NUMBERED.wrapping_add((n * 4) as u32));
    if cpu.read32(body + LIFE) as i32 <= WOUNDED {
        cpu.call(GUN_BLOWN_OFF, &[o]);
        cpu.write8(o + OBJ_MODE, ENDING);
        if cpu.read8(DIFFICULTY) == 4 {
            cpu.call(PARTING_SHOT, &[o, 0]);
        }
        return 1;
    }
    if cpu.read8(body + OBJ_MODE) as i8 >= ENDING as i8 {
        cpu.write8(o + OBJ_MODE, ENDING);
        return 1;
    }
    for k in 0..3 {
        let v = cpu.read32(body + POSITION + 4 * k);
        cpu.write32(o + POSITION + 4 * k, v);
    }
    let c = cpu.read16(body + OWN_COLOUR);
    cpu.write16(o + OWN_COLOUR, c);
    0
}

/// A big gun on a body (on_body): flashing with it; while the body fires it aims at the nearer
/// player and fires when its timer passes its wait, by its mode: two rockets 15 pixels out either
/// side of its aim (mode 0; sound 25; volleys of so many by the difficulty, then a longer wait),
/// a heavy bullet (mode 1; sound 29; to a side at random, volleys of 8 times its count), or a
/// laser (mode 2: the gun turning slowly while it lasts). It turns by its aim script if it has one
/// (with its own offset, mirrored), else toward its aim at a speed easing to its mode's.
const G_SCRIPT: u32 = 0x6c;
const G_AIM_OFFSET: u32 = 0x8c;
const G_MODE: u32 = 0x90;
const G_COUNT: u32 = 0x92;
const G_WAIT: u32 = 0x94;
const G_VOLLEYS: u32 = 0x96;
const G_TURN: u32 = 0x98;
const G_TURN_SPEED: u32 = 0x9a;
const G_SLOW: u32 = 0xa0;
const G_TIMER: u32 = 0xa2;
const G_SPEED: u32 = 0xac;
const G_SPEED_WORD: u32 = 0xae;
const G_SPEEDS: u32 = 0x0607_34ec;
const G_SLOW_SPEED: u32 = 0x200;
const G_SPEED_STEP: u32 = 0x80;
const G_EASE: u32 = 12;
const G_ACCEL: u32 = 6;
const G_MUZZLE: i32 = 12;
const G_ROCKET_OUT: i32 = 15;
const G_ROCKET_SPREAD: u16 = 0x2000;
const G_BULLET_ASIDE: i32 = 5;
const G_ROCKET_FRAMES: u16 = 10;
const G_ROCKET_FLAGS: u16 = 0x21;
const SE_ROCKETS: u32 = 25;
const SE_HEAVY: u32 = 29;
const FLASH: u16 = 1;
/// Not rewritten yet: the heavy bullet and the laser.
const HEAVY_BULLET: u32 = 0x0605_41d0;
const LASER: u32 = 0x0605_46ac;
static BIG_GUN_TURN_PROLOGUE: std::sync::OnceLock<super::Prologue> = std::sync::OnceLock::new();

/// The shot description at `f` placed `out` along `angle` from the gun.
fn shot_out(cpu: &mut Cpu, o: u32, f: u32, angle: u16, out: i32) {
    let s = cpu.call(SL_SIN, &[angle as i16 as i32 as u32]) as i32;
    let x = (s.wrapping_mul(out) as u32).wrapping_add(cpu.read32(o + POSITION));
    cpu.write32(f, x);
    let c = cpu.call(SL_COS, &[angle as i16 as i32 as u32]) as i32;
    let y = (c.wrapping_neg().wrapping_mul(out) as u32).wrapping_add(cpu.read32(o + POSITION + 4));
    cpu.write32(f + 4, y);
}

pub fn big_gun_exec(cpu: &mut Cpu, o: u32) {
    let f = cpu.stack();
    if nested!(cpu, 0x0605_b748, on_body(cpu, o)) as u8 != 0 {
        return;
    }
    let n = cpu.read8(o + NUMBER) as i8 as i32;
    let body = cpu.read32(NUMBERED.wrapping_add((n * 4) as u32));
    let h = cpu.read16(o + HIT_FLAGS) & !FLASH;
    cpu.write16(o + HIT_FLAGS, h);
    let flash = cpu.read16(body + HIT_FLAGS) & FLASH;
    cpu.write16(o + HIT_FLAGS, h | flash);
    let aim = if cpu.read16(body + ROUTE_FLAGS) & FIRING != 0 {
        let t = cpu.read16(o + G_TIMER).wrapping_add(1);
        cpu.write16(o + G_TIMER, t);
        let p = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + SPRITE));
        nested!(cpu, 0x0604_5e30, aim::angle_to(cpu, o, p))
    } else {
        cpu.read16(o + ANGLE) as u32
    } & 0xffff;
    cpu.write32(f + 0x40, aim);
    if cpu.read16(o + G_TIMER) as i16 > cpu.read16(o + G_WAIT) as i16 {
        let v = cpu.read16(o + G_VOLLEYS).wrapping_add(1);
        cpu.write16(o + G_VOLLEYS, v);
        let n = cpu.read8(o + NUMBER);
        cpu.write8(f + S_NUMBER, n);
        let a = cpu.read16(o + ANGLE);
        shot_out(cpu, o, f, a, G_MUZZLE);
        let z = cpu.read32(o + POSITION + 8);
        cpu.write32(f + 8, z);
        let a = cpu.read16(o + ANGLE);
        cpu.write16(f + S_ANGLE, a);
        let difficulty = |cpu: &mut Cpu| cpu.read8(DIFFICULTY) as i8 as i16;
        match cpu.read16(o + G_MODE) as i16 {
            0 => {
                cpu.write16(o + G_TIMER, 0);
                let d = difficulty(cpu);
                let n = ((cpu.read16(o + G_COUNT) as i16).wrapping_sub(4).wrapping_add(d)).max(1);
                let wait = if cpu.read16(o + G_VOLLEYS) as i16 >= n {
                    cpu.write16(o + G_VOLLEYS, 0);
                    96 - difficulty(cpu) * 8
                } else {
                    30 - difficulty(cpu) * 4
                };
                cpu.write16(o + G_WAIT, wait as u16);
                nested!(cpu, 0x0600_50d4, super::sound::se_set(cpu, SE_ROCKETS));
                cpu.write16(f + S_FLAGS, G_ROCKET_FRAMES);
                cpu.write16(f + S_ROCKET_FLAGS, G_ROCKET_FLAGS);
                for spread in [G_ROCKET_SPREAD.wrapping_neg(), G_ROCKET_SPREAD] {
                    let a = cpu.read16(o + ANGLE).wrapping_add(spread);
                    shot_out(cpu, o, f, a, G_ROCKET_OUT);
                    nested!(cpu, 0x0605_5b5c, super::bullets::rocket_set(cpu, f));
                }
            }
            1 => {
                nested!(cpu, 0x0600_50d4, super::sound::se_set(cpu, SE_HEAVY));
                cpu.write16(o + G_TIMER, 0);
                let wait = if cpu.read16(o + G_VOLLEYS) as i16 >= (cpu.read16(o + G_COUNT) as i16).wrapping_mul(8) {
                    cpu.write16(o + G_VOLLEYS, 0);
                    96 - difficulty(cpu) * 8
                } else {
                    6
                };
                cpu.write16(o + G_WAIT, wait as u16);
                let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000)) & 3;
                let aside = match r {
                    2 => Some(0x4000u16),
                    3 => Some(0xc000),
                    _ => None,
                };
                if let Some(aside) = aside {
                    let a = cpu.read16(o + ANGLE).wrapping_add(aside) as i16 as i32 as u32;
                    let s = cpu.call(SL_SIN, &[a]) as i32;
                    let x = cpu.read32(f).wrapping_add(s.wrapping_mul(G_BULLET_ASIDE) as u32);
                    cpu.write32(f, x);
                    let a = cpu.read16(o + ANGLE).wrapping_add(aside) as i16 as i32 as u32;
                    let c = cpu.call(SL_COS, &[a]) as i32;
                    let y = cpu.read32(f + 4).wrapping_sub(c.wrapping_mul(G_BULLET_ASIDE) as u32);
                    cpu.write32(f + 4, y);
                }
                cpu.call(HEAVY_BULLET, &[f]);
            }
            2 => {
                cpu.write16(o + G_TIMER, 0);
                let lasts = (cpu.read16(o + G_COUNT) as i16).wrapping_mul(30);
                cpu.write16(o + G_WAIT, lasts.wrapping_add(80) as u16);
                cpu.write16(o + G_SLOW, lasts.wrapping_add(40) as u16);
                cpu.write32(f + 0x3c, o);
                cpu.write16(f + S_ANGLE, 0);
                cpu.write16(f + S_FLAGS, 8);
                let w = cpu.read16(o + G_WAIT).wrapping_sub(80);
                cpu.write16(f + S_ROCKET_FLAGS, w);
                cpu.write16(f + 0x32, 0);
                cpu.call(LASER, &[f, o + ANGLE]);
            }
            _ => {}
        }
    }
    let script = cpu.read32(o + G_SCRIPT);
    if script != 0 {
        nested!(cpu, 0x0605_a340, aim_step(cpu, o, script, 0));
        let a = cpu.read16(o + AIM).wrapping_add(cpu.read16(o + G_AIM_OFFSET)) as i16 as i32;
        let mirror = cpu.read8(o + MIRROR) as i8 as i32;
        cpu.write16(o + ANGLE, a.wrapping_mul(mirror) as u16);
        return;
    }
    let to = if cpu.read16(o + G_SLOW) as i16 != 0 {
        let s = cpu.read16(o + G_SLOW).wrapping_sub(1);
        cpu.write16(o + G_SLOW, s);
        G_SLOW_SPEED
    } else {
        let m = cpu.read16(o + G_MODE) as i16 as i32;
        cpu.read16(G_SPEEDS.wrapping_add((m * 2) as u32)) as u32
    };
    let s = cpu.read32(o + G_SPEED);
    let s = nested!(cpu, 0x0604_6168, actors::approach(cpu, s, to, G_SPEED_STEP));
    cpu.write32(o + G_SPEED, s);
    let (turn, speed) = (o + G_TURN, o + G_TURN_SPEED);
    let from = cpu.read16(o + ANGLE) as u32;
    let most = cpu.read16(o + G_SPEED_WORD) as u32;
    let aim = cpu.read32(f + 0x40);
    cpu.nested_with_stack(&BIG_GUN_TURN_PROLOGUE, TURN_TOWARD, &[most, G_EASE, G_ACCEL], |cpu| aim::turn_toward(cpu, turn, speed, from, aim, most, G_EASE, G_ACCEL));
    let a = cpu.read16(o + ANGLE).wrapping_add(cpu.read16(o + G_TURN));
    cpu.write16(o + ANGLE, a);
}

/// A ground enemy with a hatch: as ground_exec it dies, leaves and follows its route; when its
/// route starts it firing its hatch opens (frame 1 for three frames, then its hurt flag cleared
/// with its opening's effect, then frame 2); while firing it fires a plain bullet every so often
/// (by its settings, less each step of difficulty), straight ahead if its route says so, else at
/// the player.
const HATCH: u32 = 0x9a;
const HATCH_OPENS: u32 = 0x0605_dd54;
const UNHURT: u32 = 0x4_0000;
const STRAIGHT: u16 = 8;

pub fn hatch_exec(cpu: &mut Cpu, o: u32) {
    let f = cpu.stack();
    let (x, y) = (cpu.read32(o + POSITION), cpu.read32(o + POSITION + 4));
    if nested!(cpu, 0x0605_ef90, super::bg::map_hit(cpu, x, y)) != 0 {
        cpu.write32(o + LIFE, u32::MAX);
    }
    let alive = |cpu: &mut Cpu| cpu.read32(o + LIFE) as i32 > 0;
    let leaves = alive(cpu)
        && cpu.read16(o + ROUTE_FLAGS) & ROUTE_1000 != 0
        && {
            let [top, bottom, left, right] = GROUND_AREA;
            cpu.nested_with_stack(&aim::AREA_FLAGS_PROLOGUE, aim::AREA_FLAGS, &[right], |cpu| aim::area_flags(cpu, o + SPRITE, top, bottom, left, right)) as u8 != 0
        };
    if !alive(cpu) || leaves {
        if !alive(cpu) {
            cpu.call(BLOWN_UP, &[o]);
        }
        cpu.write8(o + OBJ_MODE, ENDING);
        return;
    }
    let script = cpu.read32(o + SCRIPT);
    match nested!(cpu, 0x0605_a000, route_step(cpu, o, script)) & 0xff {
        START_FIRING => {
            let fl = cpu.read16(o + ROUTE_FLAGS) | FIRING;
            cpu.write16(o + ROUTE_FLAGS, fl);
            cpu.write16(o + HATCH, 1);
        }
        STOP_FIRING => {
            let fl = cpu.read16(o + ROUTE_FLAGS) & !FIRING;
            cpu.write16(o + ROUTE_FLAGS, fl);
            cpu.write16(o + FIRE_WAIT, 0);
        }
        _ => {}
    }
    nested!(cpu, 0x0605_a460, route_move(cpu, o));
    let mirror = cpu.read8(o + MIRROR) as i8 as i16;
    let d = cpu.read16(o + DIRECTION) as i16;
    cpu.write16(o + ANGLE, mirror.wrapping_mul(d) as u16);
    let hatch = cpu.read16(o + HATCH);
    let frame = match hatch {
        1..=4 => {
            if hatch == 4 {
                let flags = cpu.read32(o + FLAGS) & !UNHURT;
                cpu.write32(o + FLAGS, flags);
                cpu.call(HATCH_OPENS, &[o]);
            }
            let h = cpu.read16(o + HATCH).wrapping_add(1);
            cpu.write16(o + HATCH, h);
            Some(1)
        }
        5 => Some(2),
        _ => None,
    };
    if let Some(frame) = frame {
        cpu.write8(o + FRAME, frame);
    }
    if cpu.read16(o + ROUTE_FLAGS) & FIRING == 0 {
        return;
    }
    let wait = cpu.read16(o + FIRE_WAIT).wrapping_sub(1);
    cpu.write16(o + FIRE_WAIT, wait);
    if wait as i16 >= 0 {
        return;
    }
    let n = cpu.read8(o + NUMBER);
    cpu.write8(f + S_NUMBER, n);
    let every = (cpu.read8(o + SETTINGS) & FIRE_EVERY) as i16;
    let d = cpu.read8(DIFFICULTY) as i8 as i16;
    cpu.write16(o + FIRE_WAIT, every.wrapping_sub(d).wrapping_mul(8).wrapping_add(8) as u16);
    for k in 0..3 {
        let v = cpu.read32(o + POSITION + 4 * k);
        cpu.write32(f + 4 * k, v);
    }
    let a = if cpu.read16(o + ROUTE_FLAGS) & STRAIGHT != 0 {
        cpu.read16(o + ANGLE)
    } else {
        nested!(cpu, 0x0605_6ad4, aim::angle_to_target(cpu, o, AIM_DELAY)) as u16
    };
    cpu.write16(f + S_ANGLE, a);
    nested!(cpu, 0x0605_3f48, super::bullets::bullet_set_plain(cpu, f));
}

/// A shard (a plain actor thrown off a ground enemy): it flies with its speed (x, y), sped by its
/// acceleration, turning (by the word at +0x42), blinking after 15 frames; ended after 30.
const SHARD_FRAMES: u32 = 0x3c;
const SHARD_TURN: u32 = 0x42;
const SHARD_SPEED: u32 = 0x54;
const SHARD_ACCEL: u32 = 0x5c;
const BLINK: u32 = 0x100;

pub fn shard_exec(cpu: &mut Cpu, o: u32) {
    let t = cpu.read8(o + SHARD_FRAMES).wrapping_add(1);
    cpu.write8(o + SHARD_FRAMES, t);
    if t > 30 {
        cpu.write8(o + OBJ_MODE, ENDING);
        return;
    }
    if t > 15 {
        let flags = cpu.read32(o + FLAGS) ^ BLINK;
        cpu.write32(o + FLAGS, flags);
    }
    for k in 0..2 {
        let p = cpu.read32(o + POSITION + 4 * k).wrapping_add(cpu.read32(o + SHARD_SPEED + 4 * k));
        cpu.write32(o + POSITION + 4 * k, p);
    }
    for k in 0..2 {
        let v = cpu.read32(o + SHARD_SPEED + 4 * k).wrapping_add(cpu.read32(o + SHARD_ACCEL + 4 * k));
        cpu.write32(o + SHARD_SPEED + 4 * k, v);
    }
    let a = cpu.read16(o + ANGLE).wrapping_add(cpu.read16(o + SHARD_TURN));
    cpu.write16(o + ANGLE, a);
}
