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

