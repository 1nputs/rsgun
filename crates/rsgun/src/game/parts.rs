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
