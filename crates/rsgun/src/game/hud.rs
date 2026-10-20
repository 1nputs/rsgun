//! The players' status lines (into each player's line buffers, text.rs, which the VBlank sends):
//! the score (its last digit the continues used: Radiant Silvergun's count), the lives, a
//! weapon's level with its icon (cycling through the weapons), and the gauge beside them.

use super::globals::{self, mode, CONTINUES, GAME_MODE, PLAY_DATA, PLAY_DATA_SIZE, SCORE, VBLANKS};
use super::{math, palette, text, Cpu};

/// What each player's status line shows now, to redraw only what changed: the score, the lives,
/// the weapon level, the weapon cycle's count, the gauge.
const LIVES_SHOWN: u32 = 0x0607_48a0;
const LEVEL_SHOWN: u32 = 0x0607_48a2;
const CYCLE: u32 = 0x0607_48a4;
const SCORE_SHOWN: u32 = 0x0607_48a8;
const GAUGE_SHOWN: u32 = 0x0607_48b0;
/// Each player's lives (a byte), and weapon levels (three bytes).
const LIVES: u32 = 0x0607_4062;
const WEAPON_LEVELS: u32 = 0x0607_4088;
/// The player objects (0 when there is none): their gauge's value (+0x50, from 10000) and
/// whether it is charging (+0x8b).
const PLAYERS: u32 = 0x0607_40f0;
const GAUGE_VALUE: u32 = 0x50;
const GAUGE_BASE: i32 = 10000;
const CHARGING: u32 = 0x8b;

/// Where things go in a status line buffer (bytes): the score's last digit, the lives' (or "--"),
/// the weapon's icon, its level's last digit.
const SCORE_AT: u32 = 0x10;
const LIVES_AT: u32 = 0x38;
const NO_LIVES_AT: u32 = 0x36;
const ICON_AT: u32 = 0x16;
const LEVEL_AT: u32 = 0x1a;
/// "--" (the trial ride's lives).
const NO_LIVES: u32 = 0x0606_0384;
/// Tiles: a digit's (digit + 1; the second player's palette 0x1000), the weapon icons' (0xb on),
/// the gauge's (full, and by eighths).
const DIGIT_BASE: u16 = 1;
const PLAYER_2: u16 = 0x1000;
const ICON_BASE: u16 = 0xb;
const GAUGE_FULL: u32 = 0x0607_3660;
const GAUGE_EIGHTHS: u32 = 0x0607_3650;
const GAUGE_TILES: i32 = 8;
/// The gauge's buffers (in the players' credit line buffers), and its icon's colours (2 of colour
/// RAM, by player; still, or three in turn while charging).
const GAUGE_AT: [u32; 2] = [0x0607_3634, 0x0607_3682];
const ICON_COLOURS_AT: [u32; 2] = [0x308, 0x318];
const ICON_STILL: u32 = 0x0607_3918;
const ICON_CHARGING: [u32; 3] = [0x0607_3914, 0x0607_391c, 0x0607_3920];
/// Moves a value toward a target by a step (not rewritten yet).
const APPROACH: u32 = 0x0604_6168;

fn attr(p: u32) -> u16 {
    if p != 0 { PLAYER_2 } else { 0 }
}

/// `digits` digits of `n` (unsigned) ending at `at`, in player `p`'s palette.
pub fn put_digits(cpu: &mut Cpu, n: u32, digits: u32, at: u32, p: u32) {
    let base = DIGIT_BASE | attr(p);
    let (mut n, mut at) = (n, at);
    for _ in 0..(digits as i32).max(0) {
        cpu.write16(at, (n % 10) as u16 + base);
        at = at.wrapping_sub(2);
        n /= 10;
    }
}

/// Player `p`'s status line, in the buffer at `line`: what changed redrawn.
pub fn status_line(cpu: &mut Cpu, p: u32, line: u32) {
    let pd = PLAY_DATA + PLAY_DATA_SIZE * p;
    let score = cpu.read32(pd + SCORE) as i32;
    let continues = (cpu.read8(pd + CONTINUES) as i8).min(9) as i32;
    let shown = score / 10 * 10 + continues;
    if cpu.read32(SCORE_SHOWN + 4 * p) != shown as u32 {
        let shown = shown.min(99_999_999) as u32;
        put_digits(cpu, shown, 8, line + SCORE_AT, p);
        cpu.write32(SCORE_SHOWN + 4 * p, shown);
    }
    if cpu.read8(LIVES_SHOWN + p) != cpu.read8(LIVES + p) {
        if cpu.read8(GAME_MODE) == mode::STG00 {
            text::put_string(cpu, NO_LIVES, line + NO_LIVES_AT, 0);
        } else {
            let lives = (cpu.read8(LIVES + p) as i8 as i32).min(99) as u32;
            put_digits(cpu, lives, 2, line + LIVES_AT, p);
            cpu.write8(LIVES_SHOWN + p, lives as u8);
        }
    }
    // the weapons in turn, 64 frames each: three (two without the BIOS's setting)
    let (period, step) = if globals::bios_setting_is_1(cpu) { (0xc0, 1) } else { (0x80, 2) };
    let count = cpu.read8(CYCLE + p).wrapping_add(1);
    cpu.write8(CYCLE + p, count);
    if count == period {
        cpu.write8(CYCLE + p, 0);
    }
    let weapon = (cpu.read8(CYCLE + p) >> 6) as u32;
    cpu.write16(line + ICON_AT, weapon as u16 + ICON_BASE + attr(p));
    let level = cpu.read8(WEAPON_LEVELS + 3 * p + step * weapon) as i8;
    if cpu.read8(LEVEL_SHOWN + p) as i8 != level {
        let level = (level as i32).min(99) as u32;
        put_digits(cpu, level, 2, line + LEVEL_AT, p);
        cpu.write8(LEVEL_SHOWN + p, level as u8);
    }
    gauge(cpu, p);
}

/// Player `p`'s gauge: its shown value moved toward the player's (an eighth of the way, and one),
/// drawn as eight tiles; its icon's colours (in turn while it charges).
pub fn gauge(cpu: &mut Cpu, p: u32) {
    let player = cpu.read32(PLAYERS + 4 * p);
    let (target, charging) = if player == 0 {
        (0, 0)
    } else {
        let v = (cpu.read32(player + GAUGE_VALUE) as i32).wrapping_sub(GAUGE_BASE);
        (v, cpu.read8(player + CHARGING))
    };
    let target = target.max(0);
    let shown = cpu.read32(GAUGE_SHOWN + 4 * p) as i32;
    let step = (target.wrapping_sub(shown).wrapping_abs() / 8).wrapping_add(1);
    let shown = cpu.call(APPROACH, &[shown as u32, target as u32, step as u32]) as i32;
    cpu.write32(GAUGE_SHOWN + 4 * p, shown as u32);
    let value = if shown > 9999 { 0 } else { shown };
    // 64ths: value × 64 / 10000
    let sixty_fourths = (value << 6) / 10000;
    let mut at = GAUGE_AT[p.min(1) as usize];
    let (full, eighths) = (sixty_fourths >> 3, sixty_fourths & 7);
    let a = attr(p);
    for _ in 0..full.max(0) {
        let t = cpu.read16(GAUGE_FULL);
        cpu.write16(at, t.wrapping_add(a));
        at += 2;
    }
    let mut i = full.max(0);
    let mut t = cpu.read16(GAUGE_EIGHTHS + 2 * eighths as u32);
    loop {
        cpu.write16(at, t.wrapping_add(a));
        at += 2;
        if i >= GAUGE_TILES - 1 {
            break;
        }
        i += 1;
        t = cpu.read16(GAUGE_EIGHTHS);
    }
    let index = ICON_COLOURS_AT[p.min(1) as usize];
    let colours = if charging == 0 {
        ICON_STILL
    } else {
        let vblanks = cpu.read32(VBLANKS);
        match math::remainder(cpu, vblanks, 3) as i32 {
            k @ 0..=2 => ICON_CHARGING[k as usize],
            _ => return,
        }
    };
    palette::set_colours(cpu, colours, index, 2);
}

/// Player `p`'s status line to be drawn whole next time.
pub fn status_reset(cpu: &mut Cpu, p: u32) {
    cpu.write32(SCORE_SHOWN + 4 * p, u32::MAX);
    cpu.write8(LIVES_SHOWN + p, 0xFF);
    cpu.write8(LEVEL_SHOWN + p, 0xFF);
    cpu.write8(CYCLE + p, 0);
    cpu.write32(GAUGE_SHOWN + 4 * p, 0);
}

/// Player `p`'s status line, in that player's own buffer.
pub fn draw_status(cpu: &mut Cpu, p: u32) {
    match p {
        0 => status_line(cpu, 0, STATUS_BUFFERS[0]),
        1 => status_line(cpu, 1, STATUS_BUFFERS[1]),
        _ => {}
    }
}

/// The players' own line buffers (text.rs's).
const STATUS_BUFFERS: [u32; 2] = [0x0607_3614, 0x0607_3662];
