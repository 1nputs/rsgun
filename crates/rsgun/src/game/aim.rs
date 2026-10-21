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
const TURN_SIGN: u32 = 0x0605_6554;
const ANGLE_BETWEEN: u32 = 0x0605_6580;
const QUOTIENT: u32 = 0x0600_6a78;
const APPROACH: u32 = 0x0604_6168;
pub(super) const AREA_FLAGS: u32 = 0x0605_69f8;

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
        nested!(cpu, 0x0600_6a8e, math::fixed_div(cpu, dx, s))
    } else {
        let c = cpu.call(SL_COS, &[a]);
        nested!(cpu, 0x0600_6a8e, math::fixed_div(cpu, dy, c))
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

/// The player nearer `obj` (the second when both are as near; the only one in; with neither,
/// the director).
pub fn nearer_player_to(cpu: &mut Cpu, obj: u32) -> u32 {
    let p0 = cpu.read32(PLAYERS);
    let p1 = cpu.read32(PLAYERS + 4);
    match (p0, p1) {
        (0, 0) => cpu.read32(GAME),
        (0, p) | (p, 0) => p,
        _ => {
            let d0 = nested!(cpu, DISTANCE, distance(cpu, obj, p0)) as i32;
            let d1 = nested!(cpu, DISTANCE, distance(cpu, obj, p1)) as i32;
            if d0 >= d1 { p1 } else { p0 }
        }
    }
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

/// Which way from `b` to `a` is the shorter turn (angles, 16 bits): 1 one way, -1 the other.
pub fn turn_sign(_cpu: &mut Cpu, a: u32, b: u32) -> u32 {
    let (a, b) = (a as i32, b as i32);
    let d = a.wrapping_sub(b);
    let sign = if a >= b { if d > 0x7fff { -1 } else { 1 } } else if d > -0x8000 { -1 } else { 1 };
    sign as u32
}

/// `b - a` as angles (16 bits), wrapped by which side of `a` a quarter turn lies on.
pub fn angle_between(cpu: &mut Cpu, a: u32, b: u32) -> u32 {
    let (a, b) = ((a & 0xffff) as i32, (b & 0xffff) as i32);
    let mut d = b - a;
    let quarter = if nested!(cpu, TURN_SIGN, turn_sign(cpu, b as u32, a as u32)) as i32 > 0 { a - 0x4000 } else { a + 0x4000 };
    if d > 0 {
        if quarter < b && quarter > a {
            d -= 0x1_0000;
        }
    } else if d < 0 && quarter > b && quarter < a {
        d += 0x1_0000;
    }
    d as i16 as i32 as u32
}

/// Where a sprite's position (x, y at +4, +8 of `s`) lies against a box: bit 0 above `top`, 1 below
/// `bottom`, 2 left of `left`, 3 right of `right` (16.16).
pub fn area_flags(cpu: &mut Cpu, s: u32, top: u32, bottom: u32, left: u32, right: u32) -> u32 {
    let y = cpu.read32(s + 8) as i32;
    let mut f = (y < top as i32) as u32;
    if y > bottom as i32 {
        f |= 2;
    }
    let x = cpu.read32(s + 4) as i32;
    if x < left as i32 {
        f |= 4;
    }
    if x > right as i32 {
        f |= 8;
    }
    f
}

/// Where the point `p` (x, y: 16.16) lies against the screen's edges, 128 pixels up and down and
/// 176 across (as area_flags: bit 0 above, 1 below, 2 left, 3 right).
pub fn point_area_wide(cpu: &mut Cpu, p: u32) -> u32 {
    point_area_in(cpu, p, [0xff80_0000, 0x0080_0000, 0xff50_0000, 0x00b0_0000])
}

/// point_area_wide against edges 8 units inside 112 pixels up and down and 176 across.
pub fn point_area(cpu: &mut Cpu, p: u32) -> u32 {
    point_area_in(cpu, p, [0xff90_0008, 0x006f_fff8, 0xff50_0008, 0x00af_fff8])
}

fn point_area_in(cpu: &mut Cpu, p: u32, [top, bottom, left, right]: [u32; 4]) -> u32 {
    let y = cpu.read32(p + 4) as i32;
    let mut f = (y < top as i32) as u32;
    if y > bottom as i32 {
        f |= 2;
    }
    let x = cpu.read32(p) as i32;
    if x < left as i32 {
        f |= 4;
    }
    if x > right as i32 {
        f |= 8;
    }
    f
}

/// The screen's edges (16.16 from its centre: top, bottom, left, right).
const SCREEN: [u32; 4] = [0xff80_0000, 0x0080_0000, 0xff50_0000, 0x00b0_0000];

/// area_flags against the screen.
pub fn off_screen(cpu: &mut Cpu, s: u32) -> u32 {
    let f = cpu.nested_with_stack(&AREA_FLAGS_PROLOGUE, AREA_FLAGS, &[SCREEN[3]], |cpu| area_flags(cpu, s, SCREEN[0], SCREEN[1], SCREEN[2], SCREEN[3]));
    f & 0xff
}
pub(super) static AREA_FLAGS_PROLOGUE: std::sync::OnceLock<super::Prologue> = std::sync::OnceLock::new();

/// Turns the angle at `angle` from `from` toward `to` (16-bit angles; words at `angle` and `speed`):
/// the turn left (at most `most`), the turning speed eased toward that over `ease` (its sign the
/// turn's) by at most `accel`, the angle moved by the speed but not past the turn.
#[allow(clippy::too_many_arguments)]
pub fn turn_toward(cpu: &mut Cpu, angle: u32, speed: u32, from: u32, to: u32, most: u32, ease: u32, accel: u32) {
    let (from, to) = (from & 0xffff, to & 0xffff);
    let most = most & 0xffff;
    let ease = ease as i16 as i32;
    let accel = accel as i16 as i32;
    let d = nested!(cpu, ANGLE_BETWEEN, angle_between(cpu, from, to)) as i16 as i32;
    let left = (d.unsigned_abs() & 0xffff).min(most);
    let sign = nested!(cpu, TURN_SIGN, turn_sign(cpu, to, from)) as i32;
    let by = if sign > 0 { ease } else { -ease };
    let target = nested!(cpu, QUOTIENT, math::quotient(cpu, left, by as u32));
    let s = cpu.read16(speed) as i16 as i32 as u32;
    let s = nested!(cpu, APPROACH, super::actors::approach(cpu, s, target, accel as u32));
    cpu.write16(speed, s as u16);
    let a = cpu.read16(angle) as i16 as i32 as u32;
    let s = cpu.read16(speed) as i16 as i32 as u32;
    let a = nested!(cpu, APPROACH, super::actors::approach(cpu, a, s, left));
    cpu.write16(angle, a as u16);
}

/// The numbered objects: the two players first.
const PLAYER_1: u32 = 0x0607_40f0;
const PLAYER_2: u32 = 0x0607_40f4;
/// The director (what is aimed at with no player in).
const DIRECTOR: u32 = 0x0607_46e0;

/// The player nearer the point `p` (x at +4, y at +8; the second on a tie), the only one in, or
/// with neither the director.
pub fn nearer_player(cpu: &mut Cpu, p: u32) -> u32 {
    let (p1, p2) = (cpu.read32(PLAYER_1), cpu.read32(PLAYER_2));
    if p1 != 0 && p2 != 0 {
        let d1 = player_distance(cpu, p1, p);
        let d2 = player_distance(cpu, p2, p);
        return if d1 as i32 >= d2 as i32 { p2 } else { p1 };
    }
    if p1 != 0 {
        p1
    } else if p2 != 0 {
        p2
    } else {
        cpu.read32(DIRECTOR)
    }
}

fn player_distance(cpu: &mut Cpu, player: u32, p: u32) -> u32 {
    let dx = cpu.read32(player + X).wrapping_sub(cpu.read32(p + 4));
    let dy = cpu.read32(player + X + 4).wrapping_sub(cpu.read32(p + 8));
    nested!(cpu, 0x0604_5e60, length(cpu, dx, dy))
}

/// The angle (16-bit, 0 straight up) from `obj` to where its target is (target's, with `delay`).
pub fn angle_to_target(cpu: &mut Cpu, obj: u32, delay: u32) -> u32 {
    let at = cpu.stack();
    nested!(cpu, 0x0604_0ccc, target(cpu, obj, delay, at));
    let dx = cpu.read32(at).wrapping_sub(cpu.read32(obj + X));
    let dy = cpu.read32(at + 4).wrapping_sub(cpu.read32(obj + X + 4));
    let a = cpu.call(SL_ATAN, &[dx, dy]);
    (a as u16).wrapping_add(0x4000) as u32
}
