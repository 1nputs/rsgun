//! The camera: an object (list 0) that eases the eye toward where it is to be, a sixteenth of the
//! way a frame (at most 8 pixels), and its roll likewise, then hands SGL the view.

use super::nested;
use super::{math, objects, Cpu};

/// The camera (0 when there is none).
const CAMERA: u32 = 0x0607_46e4;
const CAMERA_SIZE: u32 = 0x44;
const CAMERA_EXEC: u32 = 0x0604_2e6c;
/// Its fields: the roll it is to have and the roll it has (words); where the eye is to be (x, y,
/// z: 16.16), where it is to look, where the eye is, where it looks.
const ROLL_TO: u32 = 0x10;
const ROLL: u32 = 0x12;
const EYE_TO: u32 = 0x14;
const AT_TO: u32 = 0x20;
const EYE: u32 = 0x2c;
const AT: u32 = 0x38;
/// The eye's height at the start, and how far below it the camera looks.
const EYE_Z: u32 = 0xff50_0000;
const AT_BELOW: u32 = 0x0400_0000;
/// The easing: a sixteenth a frame, at most 8 pixels; closer than 1/16 pixel, there.
const EASE_MOST: u32 = 0x8_0000;
const EASE_SNAP: i32 = 0x1000;
/// The roll's easing: a sixteenth a frame (at most 0x1000) when it is more than 15 away, else a step
/// of one.
const ROLL_EASE_MOST: u32 = 0x1000;
const ROLL_NEAR: i32 = 15;

/// The objects' mode byte (objects.rs): new, running, ending, ended.
const OBJ_MODE: u32 = 3;
const ENDING: u8 = 0x70;
const ENDED: u8 = 0x73;

/// angle_diff's SH-2 entry (for nested!).
const ANGLE_DIFF: u32 = 0x0604_6194;

/// SGL's: the near limit's level (7 as the camera starts), and the view set from an eye, a point
/// looked at and a roll.
const SL_ZDSP_LEVEL: u32 = 0x0606_990c;
const SL_LOOK_AT: u32 = 0x0606_993c;

/// `to - from` as angles (16 bits, the turn's shorter way: -0x7fff..=0x8000).
pub fn angle_diff(_cpu: &mut Cpu, from: u32, to: u32) -> u32 {
    let from = (from & 0xffff) as i32;
    let mut to = (to & 0xffff) as i32;
    if to < from {
        to += 0x1_0000;
    }
    let mut d = to - from;
    if d > 0x8000 {
        d -= 0x1_0000;
    }
    d as u32
}

/// `angle` kept within `range` of `centre` (all angles, 16 bits): the nearer limit if it is
/// further.
pub fn angle_within(cpu: &mut Cpu, angle: u32, centre: u32, range: u32) -> u32 {
    let (centre, range) = (centre as i16 as i32, range as i16 as i32);
    let d = nested!(cpu, 0x0604_6194, angle_diff(cpu, centre as u32, angle)) as i32;
    let a = if d > range {
        centre + range
    } else if d < -range {
        centre - range
    } else {
        angle as i32
    };
    a as i16 as i32 as u32
}

/// Makes the camera at the start (looking down from EYE_Z at 0, 0): it, or 0.
pub fn camera_set(cpu: &mut Cpu) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, CAMERA_SIZE));
    let cam = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 0, blocks as i8 as i32 as u32, CAMERA_EXEC));
    cpu.write32(CAMERA, cam);
    if cam != 0 {
        cpu.write16(cam + ROLL, 0);
        cpu.write16(cam + ROLL_TO, 0);
        cpu.write32(cam + EYE_TO + 4, 0);
        cpu.write32(cam + EYE_TO, 0);
        cpu.write32(cam + EYE_TO + 8, EYE_Z);
        for k in 0..2 {
            let v = cpu.read32(cam + EYE_TO + 4 * k);
            cpu.write32(cam + AT_TO + 4 * k, v);
        }
        let z = cpu.read32(cam + EYE_TO + 8).wrapping_add(AT_BELOW);
        for k in 0..3 {
            let v = cpu.read32(cam + EYE_TO + 4 * k);
            cpu.write32(cam + EYE + 4 * k, v);
        }
        cpu.write32(cam + AT_TO + 8, z);
        for k in 0..3 {
            let v = cpu.read32(cam + AT_TO + 4 * k);
            cpu.write32(cam + AT + 4 * k, v);
        }
    }
    cpu.read32(CAMERA)
}

/// The eye's coordinate at `field` eased toward the one at `to`.
fn ease(cpu: &mut Cpu, cam: u32, to: u32, field: u32) {
    let target = cpu.read32(cam + to) as i32;
    let at = cpu.read32(cam + field) as i32;
    if target == at {
        return;
    }
    let step = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, (target.wrapping_sub(at) / 16) as u32, EASE_MOST)) as i32;
    if step.abs() < EASE_SNAP {
        let target = cpu.read32(cam + to);
        cpu.write32(cam + field, target);
    } else {
        let at = cpu.read32(cam + field);
        cpu.write32(cam + field, at.wrapping_add(step as u32));
    }
}

/// The camera's frame: the eye eased (x, y), the roll eased, the view handed to SGL.
pub fn camera_exec(cpu: &mut Cpu, cam: u32) {
    match cpu.read8(cam + OBJ_MODE) {
        1 => {}
        0 => {
            cpu.call(SL_ZDSP_LEVEL, &[7]);
            cpu.write8(cam + OBJ_MODE, 1);
        }
        ENDING => {
            cpu.write8(cam + OBJ_MODE, ENDED);
            return;
        }
        ENDED => {
            nested!(cpu, 0x0600_579c, objects::obj_free(cpu, cam));
            cpu.write32(CAMERA, 0);
            return;
        }
        _ => return,
    }
    ease(cpu, cam, EYE_TO, EYE);
    ease(cpu, cam, EYE_TO + 4, EYE + 4);
    let to = cpu.read16(cam + ROLL_TO) as i16 as i32 as u32;
    let roll = cpu.read16(cam + ROLL) as i16 as i32 as u32;
    if to != roll {
        let d = nested!(cpu, ANGLE_DIFF, angle_diff(cpu, roll, to)) as i32;
        let step = if d.abs() > ROLL_NEAR {
            let d = nested!(cpu, ANGLE_DIFF, angle_diff(cpu, roll, to)) as i32 / 16;
            nested!(cpu, 0x0606_4f3c, math::clamp(cpu, d as u32, ROLL_EASE_MOST))
        } else {
            nested!(cpu, 0x0606_4f3c, math::clamp(cpu, d as u32, 1))
        } as i32;
        if step.abs() <= 0 {
            let to = cpu.read16(cam + ROLL_TO);
            cpu.write16(cam + ROLL, to);
        } else {
            let roll = cpu.read16(cam + ROLL);
            cpu.write16(cam + ROLL, roll.wrapping_add(step as u16));
        }
    }
    let roll = cpu.read16(cam + ROLL) as i16 as i32 as u32;
    cpu.call(SL_LOOK_AT, &[cam + EYE, cam + AT, roll]);
}

/// How far `b` is ahead of `a` (angles, 16 bits): 0 to 0xffff.
pub fn angle_ahead(_cpu: &mut Cpu, a: u32, b: u32) -> u32 {
    let (a, b) = ((a & 0xffff) as i32, (b & 0xffff) as i32);
    (if b >= a { b } else { b + 0x1_0000 }).wrapping_sub(a) as u32
}

/// How far `b` is behind `a` going back (angles, 16 bits): -0xffff to 0.
pub fn angle_behind(_cpu: &mut Cpu, a: u32, b: u32) -> u32 {
    let (a, b) = ((a & 0xffff) as i32, (b & 0xffff) as i32);
    (if b > a { b - 0x1_0000 } else { b }).wrapping_sub(a) as u32
}
