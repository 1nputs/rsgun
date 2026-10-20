//! Aiming: angles and distances between objects, and which player an enemy aims at (the nearer;
//! with a delay, where that player was some frames ago, from the director's record of the players'
//! positions).

use super::nested;
use super::{math, Cpu};

/// An object's position (x, y: 16.16), and its number (the player's, for a player object).
const X: u32 = 0x14;
const Y: u32 = 0x18;
const NUMBER: u32 = 0x32;
/// The players' objects (0 when out), and the director (director.rs): its own position, and the
/// players' positions over the last 32 frames (16 bytes a frame: x, y for each player).
const PLAYERS: u32 = 0x0607_40f0;
const GAME: u32 = 0x0607_46e0;
const HISTORY: u32 = 0xa8;
const HISTORY_FRAMES: i32 = 32;

/// The rewrites' SH-2 entries, for the calls between them (mod.rs's nested!).
const LENGTH: u32 = 0x0604_5e60;
const DISTANCE: u32 = 0x0604_5ee4;
const TARGET: u32 = 0x0604_0ccc;

/// SGL's (as far as can be told): the angle of a vector (x, y), and the sine and cosine of an
/// angle.
const SL_ATAN: u32 = 0x0606_780c;
const SL_SIN: u32 = 0x0606_9a0c;
const SL_COS: u32 = 0x0606_9a30;
/// The game's angles are SGL's turned a quarter.
const QUARTER: u32 = 0x4000;

fn angle(cpu: &mut Cpu, dx: u32, dy: u32) -> u32 {
    let a = cpu.call(SL_ATAN, &[dx, dy]);
    a.wrapping_add(QUARTER) as i16 as i32 as u32
}

/// The angle from `a` to `b`.
pub fn angle_to(cpu: &mut Cpu, a: u32, b: u32) -> u32 {
    let dy = cpu.read32(b + Y).wrapping_sub(cpu.read32(a + Y));
    let dx = cpu.read32(b + X).wrapping_sub(cpu.read32(a + X));
    angle(cpu, dx, dy)
}

/// The length of (dx, dy) (16.16), from its angle: the larger part over that angle's sine or
/// cosine.
fn length_of(cpu: &mut Cpu, dx: u32, dy: u32) -> u32 {
    let a = angle(cpu, dx, dy);
    let (x, y) = (dx as i32, dy as i32);
    let r = if x.wrapping_abs() > y.wrapping_abs() {
        let s = cpu.call(SL_SIN, &[a]);
        math::fixed_div(cpu, dx, s)
    } else {
        let c = cpu.call(SL_COS, &[a]);
        math::fixed_div(cpu, dy, c)
    } as i32;
    r.wrapping_abs() as u32
}

/// The length of (dx, dy); 0 for (0, 0).
pub fn length(cpu: &mut Cpu, dx: u32, dy: u32) -> u32 {
    if dx == 0 && dy == 0 {
        return 0;
    }
    length_of(cpu, dx, dy)
}

/// The distance from `a` to `b` (0 when neither is there).
pub fn distance(cpu: &mut Cpu, a: u32, b: u32) -> u32 {
    let dx = cpu.read32(b + X).wrapping_sub(cpu.read32(a + X));
    let dy = cpu.read32(b + Y).wrapping_sub(cpu.read32(a + Y));
    if a == 0 && b == 0 {
        return 0;
    }
    length_of(cpu, dx, dy)
}

/// The player `obj` aims at: the nearer of the two (the only one in; with neither, the director).
/// `out` (unless 0) gets where it is: its position, or with `delay` (1-32) where it was `delay`
/// frames ago.
pub fn target(cpu: &mut Cpu, obj: u32, delay: u32, out: u32) -> u32 {
    let p0 = cpu.read32(PLAYERS);
    let p1 = cpu.read32(PLAYERS + 4);
    if p0 == 0 && p1 == 0 {
        if out != 0 {
            let game = cpu.read32(GAME);
            let x = cpu.read32(game + X);
            cpu.write32(out, x);
            let game = cpu.read32(GAME);
            let y = cpu.read32(game + Y);
            cpu.write32(out + 4, y);
        }
        return cpu.read32(GAME);
    }
    let delay = (delay as i32).max(0);
    if delay == 0 {
        let t = if p0 == 0 {
            p1
        } else if p1 == 0 {
            p0
        } else {
            let d0 = nested!(cpu, DISTANCE, distance(cpu, obj, p0)) as i32;
            let d1 = nested!(cpu, DISTANCE, distance(cpu, obj, p1)) as i32;
            if d0 >= d1 { p1 } else { p0 }
        };
        if out != 0 {
            let x = cpu.read32(t + X);
            cpu.write32(out, x);
            let y = cpu.read32(t + Y);
            cpu.write32(out + 4, y);
        }
        return t;
    }
    let frame = delay.min(HISTORY_FRAMES) - 1;
    let t = if p0 == 0 {
        p1
    } else if p1 == 0 {
        p0
    } else {
        let (ox, oy) = (cpu.read32(obj + X), cpu.read32(obj + Y));
        let (x, y) = (cpu.read32(p0 + X), cpu.read32(p0 + Y));
        let a0 = nested!(cpu, LENGTH, length(cpu, x.wrapping_sub(ox), y.wrapping_sub(oy))) as i32;
        let (ox, oy) = (cpu.read32(obj + X), cpu.read32(obj + Y));
        let (x, y) = (cpu.read32(p1 + X), cpu.read32(p1 + Y));
        let a1 = nested!(cpu, LENGTH, length(cpu, x.wrapping_sub(ox), y.wrapping_sub(oy))) as i32;
        if a0 >= a1 { p1 } else { p0 }
    };
    if out != 0 {
        let n = cpu.read8(t + NUMBER) as i8 as i32;
        let at = (8 * n + 16 * frame) as u32 + HISTORY;
        let game = cpu.read32(GAME);
        let x = cpu.read32(game.wrapping_add(at));
        cpu.write32(out, x);
        let game = cpu.read32(GAME);
        let y = cpu.read32(game.wrapping_add(at + 4));
        cpu.write32(out + 4, y);
    }
    t
}

/// The angle from `obj` to where the player it aims at is (or was, `delay` frames ago).
pub fn aim(cpu: &mut Cpu, obj: u32, delay: u32) -> u32 {
    let out = cpu.stack();
    nested!(cpu, TARGET, target(cpu, obj, delay, out));
    let dy = cpu.read32(out + 4).wrapping_sub(cpu.read32(obj + Y));
    let dx = cpu.read32(out).wrapping_sub(cpu.read32(obj + X));
    angle(cpu, dx, dy)
}

/// The numbered objects (players first): their count, and the fields a search looks at: the side
/// (0 the players'), the mode byte (ending from 0x70), the flags (FLAGS_UNTARGETED), the life.
const NUMBERED: i32 = 63;
const SIDE: u32 = 0x54;
const OBJ_MODE: u32 = 3;
const ENDING: i8 = 0x70;
const FLAGS: u32 = 0x10;
const FLAGS_UNTARGETED: u32 = 0x010a_0000;
const LIFE: u32 = 0x50;
/// Farther than any target.
const FAR: i32 = 0x1000_0000;

/// The first free number from `from` on: it, or -1.
pub fn free_number(cpu: &mut Cpu, from: u32) -> u32 {
    let mut n = from as i8 as i32;
    while n < NUMBERED {
        if cpu.read32(PLAYERS + 4 * n as u32) == 0 {
            return n as u32;
        }
        n += 1;
    }
    u32::MAX
}

/// The nearest numbered object on the other side from `obj` that is not ending, may be aimed at
/// and is alive: its number, or -1. `out` (unless 0) gets its distance.
pub fn nearest(cpu: &mut Cpu, obj: u32, out: u32) -> u32 {
    let mut best = u32::MAX;
    let mut nearest = FAR;
    for n in 0..NUMBERED as u32 {
        let t = cpu.read32(PLAYERS + 4 * n);
        if t == 0
            || cpu.read8(obj + SIDE) == cpu.read8(t + SIDE)
            || cpu.read8(t + OBJ_MODE) as i8 >= ENDING
            || cpu.read32(t + FLAGS) & FLAGS_UNTARGETED != 0
            || cpu.read32(t + LIFE) as i32 <= 0
        {
            continue;
        }
        let d = nested!(cpu, DISTANCE, distance(cpu, obj, t)) as i32;
        if d < nearest {
            nearest = d;
            best = cpu.read8(t + NUMBER) as i8 as i32 as u32;
        }
    }
    if out != 0 {
        cpu.write32(out, nearest as u32);
    }
    best
}
