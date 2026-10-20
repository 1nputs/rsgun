//! A boss's parts: actors that share their boss's life (each frame a part's own life, from one,
//! gives the damage it took to the shared life), follow a point of the boss, and are only hittable
//! within a depth of it. The boss's state word tells them when to end or to stop being hit, and
//! learns from them when one was hit.

use super::nested;
use super::{hits, Cpu};

/// The objects' mode byte's ending, and the actor fields a part uses: its flags (NO_HITS), what
/// hit it last, its own life (one, 16.16, each frame), its hit flags (WAS_HIT), the boss's state
/// word, the shared life, the point it follows (x, y, z), the depth it may be hit within (a byte,
/// whole units).
const OBJ_MODE: u32 = 3;
const ENDING: u8 = 0x70;
const FLAGS: u32 = 0x10;
const NO_HITS: u32 = 0x8_0000;
const POSITION: u32 = 0x14;
const HIT_OTHER: u32 = 0x40;
const LIFE: u32 = 0x50;
const ONE: u32 = 0x1_0000;
const HIT_FLAGS: u32 = 0x56;
const WAS_HIT: u16 = 0x200;
const BOSS_STATE: u32 = 0x6c;
const SHARED_LIFE: u32 = 0x70;
const FOLLOWS: u32 = 0x74;
const DEPTH: u32 = 0x78;
/// The boss's state word: bits that stop the parts (one of which ends them), and the bit a hit part
/// sets.
const STOPPED: u16 = 0xe0;
const ENDS: u16 = 0x40;
const PART_HIT: u16 = 1;
/// The followed point's depth is from this.
const Z_BASE: u32 = 0xb0_0000;
/// The killer's side that scores nothing (the third), its number and type, and the kill's points.
const SIDE: u32 = 0x54;
const THIRD_SIDE: u8 = 3;
const NUMBER: u32 = 0x32;
const TYPE: u32 = 0x55;
const KILL_POINTS: u32 = 10000;
const SCORE_KILL: u32 = 0x0604_3108;

/// A part's behaviour.
pub fn part_exec(cpu: &mut Cpu, part: u32) {
    let shared = cpu.read32(part + SHARED_LIFE);
    let life = cpu.read32(shared) as i32;
    if life > 0 {
        let taken = cpu.read32(part + LIFE);
        cpu.write32(shared, (life as u32).wrapping_sub(ONE).wrapping_add(taken));
        let shared = cpu.read32(part + SHARED_LIFE);
        if cpu.read32(shared) as i32 <= 0 {
            let killer = cpu.read32(part + HIT_OTHER);
            if cpu.read8(killer + SIDE) != THIRD_SIDE {
                let weapon = cpu.read8(killer + TYPE) as i8 as i32 as u32;
                let p = cpu.read8(killer + NUMBER) as i8 as i32 as u32;
                nested!(cpu, SCORE_KILL, hits::score_kill(cpu, p, weapon, part, KILL_POINTS));
            }
        }
    }
    cpu.write32(part + LIFE, ONE);
    let state = cpu.read32(part + BOSS_STATE);
    let s = cpu.read16(state);
    let hittable = if s & STOPPED != 0 {
        if s & ENDS != 0 {
            cpu.write8(part + OBJ_MODE, ENDING);
            return;
        }
        false
    } else {
        let at = cpu.read32(part + FOLLOWS);
        let x = cpu.read32(at);
        cpu.write32(part + POSITION, x);
        let at = cpu.read32(part + FOLLOWS);
        let y = cpu.read32(at + 4);
        cpu.write32(part + POSITION + 4, y);
        let at = cpu.read32(part + FOLLOWS);
        let z = cpu.read32(at + 8).wrapping_sub(Z_BASE) as i32;
        cpu.write32(part + POSITION + 8, z as u32);
        let depth = cpu.read8(part + DEPTH) as i8 as i32;
        z <= depth << 16 && z > (-depth) << 16
    };
    if hittable {
        let f = cpu.read32(part + FLAGS);
        cpu.write32(part + FLAGS, f & !NO_HITS);
        if cpu.read16(part + HIT_FLAGS) & WAS_HIT != 0 {
            let state = cpu.read32(part + BOSS_STATE);
            let s = cpu.read16(state);
            cpu.write16(state, s | PART_HIT);
        }
    } else {
        let f = cpu.read32(part + FLAGS);
        cpu.write32(part + FLAGS, f | NO_HITS);
    }
}

/// A boss's body (the actor its parts belong to): the boss's state word (as its parts'), its
/// points (x, y, z each: the parts' places) and its parts (8 bytes each: a word at +6 picks the
/// depth they show within), their count (a byte).
const BODY_POINTS: u32 = 0x70;
const BODY_PARTS: u32 = 0x74;
const BODY_COUNT: u32 = 0x78;
const PART_DEPTH_AT: u32 = 6;
const PART: u32 = 8;
const POINT: u32 = 12;
/// The depths parts show within, by their word (bytes, whole units).
const SHOW_DEPTHS: u32 = 0x0607_2fa4;
/// The state word's bit that holds the body still.
const HELD_STILL: u16 = 0x20;

/// A boss body's behaviour: no hits unless a part shows (within its depth of the screen's); the
/// body at the middle of the showing parts' points. Ended when its state says so; held when it
/// says to stop.
pub fn body_exec(cpu: &mut Cpu, body: u32) {
    let flags = cpu.read32(body + FLAGS) | NO_HITS;
    cpu.write32(body + FLAGS, flags);
    let state_at = cpu.read32(body + BOSS_STATE);
    let state = cpu.read16(state_at);
    if state & (STOPPED & !HELD_STILL) != 0 {
        if state & ENDS != 0 {
            cpu.write8(body + OBJ_MODE, ENDING);
        }
        return;
    }
    if state & HELD_STILL != 0 {
        return;
    }
    let (mut left, mut right) = (0x3fff_0000i32, 0xc001_0000u32 as i32);
    let (mut top, mut bottom) = (left, right);
    let mut point = cpu.read32(body + BODY_POINTS);
    let mut part = cpu.read32(body + BODY_PARTS);
    let mut i: u32 = 0;
    while (i as i32) < cpu.read8(body + BODY_COUNT) as i8 as i32 * 3 {
        let which = cpu.read16(part + PART_DEPTH_AT) as i16 as i32 as u32;
        let reach = (cpu.read8(SHOW_DEPTHS.wrapping_add(which)) as i32) << 16;
        let depth = cpu.read32(point + 8).wrapping_sub(Z_BASE) as i32;
        if depth <= reach && depth > -reach {
            let x = cpu.read32(point) as i32;
            left = left.min(x);
            right = right.max(x);
            let y = cpu.read32(point + 4) as i32;
            top = top.min(y);
            bottom = bottom.max(y);
            let flags = cpu.read32(body + FLAGS) & !NO_HITS;
            cpu.write32(body + FLAGS, flags);
        }
        point += POINT;
        part += PART;
        i = (i + 3) & 0xffff;
    }
    cpu.write32(body + POSITION, (left.wrapping_add(right) / 2) as u32);
    cpu.write32(body + POSITION + 4, (top.wrapping_add(bottom) / 2) as u32);
}

/// The hit flags' flash bit, and the flags' unhurt bit.
const FLASH: u16 = 1;
const UNHURT: u32 = 0x4_0000;

/// `part` flashes when `with` does (if there is one).
pub fn flash_with(cpu: &mut Cpu, part: u32, with: u32) {
    if with != 0 {
        let f = cpu.read16(with + HIT_FLAGS) & FLASH | cpu.read16(part + HIT_FLAGS);
        cpu.write16(part + HIT_FLAGS, f);
    }
}

/// `part` flashes while `with` was hit and is not flashing itself, and not while `with` is unhurt.
pub fn flash_against(cpu: &mut Cpu, part: u32, with: u32) {
    if with == 0 {
        return;
    }
    let w = cpu.read16(with + HIT_FLAGS);
    if w & WAS_HIT != 0 {
        let f = (w ^ FLASH) & FLASH | cpu.read16(part + HIT_FLAGS);
        cpu.write16(part + HIT_FLAGS, f);
    }
    if cpu.read32(with + FLAGS) & UNHURT != 0 {
        let f = cpu.read16(part + HIT_FLAGS) & !FLASH;
        cpu.write16(part + HIT_FLAGS, f);
    }
}

