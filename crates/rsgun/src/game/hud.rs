//! The players' status lines (into each player's line buffers, text.rs, which the VBlank sends):
//! the score (its last digit the continues used: Radiant Silvergun's count), the lives, a
//! weapon's level with its icon (cycling through the weapons), and the gauge beside them. And the
//! boss's life gauge (an object: a bar at the screen's bottom left).

use super::nested;
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
        nested!(cpu, 0x0605_fe34, put_digits(cpu, shown, 8, line + SCORE_AT, p));
        cpu.write32(SCORE_SHOWN + 4 * p, shown);
    }
    if cpu.read8(LIVES_SHOWN + p) != cpu.read8(LIVES + p) {
        if cpu.read8(GAME_MODE) == mode::STG00 {
            nested!(cpu, 0x0606_11b8, text::put_string(cpu, NO_LIVES, line + NO_LIVES_AT, 0));
        } else {
            let lives = (cpu.read8(LIVES + p) as i8 as i32).min(99) as u32;
            nested!(cpu, 0x0605_fe34, put_digits(cpu, lives, 2, line + LIVES_AT, p));
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
        nested!(cpu, 0x0605_fe34, put_digits(cpu, level, 2, line + LEVEL_AT, p));
        cpu.write8(LEVEL_SHOWN + p, level as u8);
    }
    nested!(cpu, 0x0606_0550, gauge(cpu, p));
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
        match nested!(cpu, 0x0600_6a6e, math::remainder(cpu, vblanks, 3)) as i32 {
            k @ 0..=2 => ICON_CHARGING[k as usize],
            _ => return,
        }
    };
    nested!(cpu, 0x0600_5b18, palette::set_colours(cpu, colours, index, 2));
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
        0 => nested!(cpu, 0x0606_0388, status_line(cpu, 0, STATUS_BUFFERS[0])),
        1 => nested!(cpu, 0x0606_0388, status_line(cpu, 1, STATUS_BUFFERS[1])),
        _ => {}
    }
}

/// The players' own line buffers (text.rs's).
const STATUS_BUFFERS: [u32; 2] = [0x0607_3614, 0x0607_3662];

/// The boss's life gauge (an object of list 4, its own exec; 0 when there is none): a line of
/// 224 pixels at (-112, 92), its length by the life shown over the most, in the first kind's colour
/// (0x40 on above half) shimmering with the passes; the life shown rises toward the life by a
/// sixteenth of the way (at least 1) and drops to it at once. It ends once the life shown is 0.
const BOSS_GAUGE: u32 = 0x0607_46ec;
const BOSS_GAUGE_EXEC: u32 = 0x0604_62dc;
const BOSS_GAUGE_SIZE: u32 = 0x80;
const BG_LIST: u32 = 4;
/// The gauge's mode (objects.rs's), sprite (flags: a line; its corners), phase, life, most life,
/// life shown, its line's corners.
const BG_MODE: u32 = 3;
const BG_SPRITE: u32 = 0x10;
const BG_FLAGS: u32 = 0x10;
const BG_POSITION: u32 = 0x14;
const BG_SCALE_X: u32 = 0x20;
const BG_COLOUR: u32 = 0x28;
const BG_NUMBER: u32 = 0x32;
const BG_LIFT: u32 = 0x33;
const BG_CORNERS_AT: u32 = 0x34;
const BG_PHASE: u32 = 0x3c;
const BG_LIFE: u32 = 0x60;
const BG_MOST: u32 = 0x64;
const BG_SHOWN: u32 = 0x68;
const BG_CORNERS: u32 = 0x70;
const LINE: u32 = 3;
const BG_X: u32 = 0xff90_0000;
const BG_Y: u32 = 0x5c_0000;
const BG_LENGTH: u16 = 0xe0;
const BG_HALF: i32 = 0x8000;
const BG_HIGH: u16 = 0x40;
const BG_RUNNING: u8 = 1;
const BG_ENDING: u8 = 0x70;
const BG_ENDED: u8 = 0x73;
const FIRST_COLOUR: u32 = 0x0607_41fc;

/// The boss's life gauge made, `most` its full life: it (also kept at BOSS_GAUGE), or 0.
pub fn boss_gauge_set(cpu: &mut Cpu, most: u32) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, super::objects::obj_size(cpu, BOSS_GAUGE_SIZE)) as i8 as i32 as u32;
    let g = nested!(cpu, 0x0600_6b00, super::objects::obj_set(cpu, BG_LIST, blocks, BOSS_GAUGE_EXEC));
    if g != 0 {
        nested!(cpu, 0x0604_dd50, super::effects::actor_init(cpu, g));
        cpu.write32(g + BG_MOST, most);
    }
    cpu.write32(BOSS_GAUGE, g);
    g
}

/// The boss's life given to its gauge (if there is one; not below 0).
pub fn boss_gauge_life(cpu: &mut Cpu, life: u32) {
    let g = cpu.read32(BOSS_GAUGE);
    if g != 0 {
        cpu.write32(g + BG_LIFE, (life as i32).max(0) as u32);
    }
}

pub fn boss_gauge_exec(cpu: &mut Cpu, g: u32) {
    match cpu.read8(g + BG_MODE) {
        BG_RUNNING => {}
        0 => {
            if cpu.read32(globals::GAME_FLAGS) & globals::HELD != 0 {
                return;
            }
            cpu.write8(g + BG_MODE, BG_RUNNING);
        }
        BG_ENDING => {
            cpu.write8(g + BG_MODE, BG_ENDED);
            return;
        }
        BG_ENDED => {
            nested!(cpu, 0x0600_579c, super::objects::obj_free(cpu, g));
            return;
        }
        _ => return,
    }
    let phase = cpu.read8(g + BG_PHASE);
    match phase {
        0 | 1 => {
            if phase == 0 {
                cpu.write32(g + BG_CORNERS_AT, g + BG_CORNERS);
                let flags = cpu.read32(g + BG_FLAGS) | LINE;
                cpu.write32(g + BG_FLAGS, flags);
                cpu.write8(g + BG_LIFT, 0x7f);
                cpu.write8(g + BG_NUMBER, 0xff);
                cpu.write32(g + BG_POSITION, BG_X);
                cpu.write32(g + BG_POSITION + 4, BG_Y);
                cpu.write32(g + BG_POSITION + 8, 0);
                for (k, v) in [(2, 0), (0, 0), (4, BG_LENGTH), (6, 0)] {
                    cpu.write16(g + BG_CORNERS + k, v);
                }
                cpu.write32(g + BG_SHOWN, 1);
                let most = cpu.read32(g + BG_MOST);
                cpu.write32(g + BG_LIFE, most);
                cpu.write8(g + BG_PHASE, 1);
            }
            let colour = cpu.read16(FIRST_COLOUR);
            cpu.write16(g + BG_COLOUR, colour);
            let (shown, life) = (cpu.read32(g + BG_SHOWN) as i32, cpu.read32(g + BG_LIFE) as i32);
            let shown = if shown >= life {
                life as u32
            } else {
                let step = nested!(cpu, 0x0606_4f50, math::at_least(cpu, (life.wrapping_sub(shown) / 16) as u32, 1)) as i32;
                let life = cpu.read32(g + BG_LIFE);
                nested!(cpu, 0x0604_6168, super::actors::approach(cpu, shown as u32, life, step.wrapping_abs() as u32))
            };
            cpu.write32(g + BG_SHOWN, shown);
            let most = cpu.read32(g + BG_MOST);
            let length = nested!(cpu, 0x0600_6a8e, math::fixed_div(cpu, shown, most));
            cpu.write32(g + BG_SCALE_X, length);
            let colour = cpu.read16(g + BG_COLOUR).wrapping_add(if length as i32 > BG_HALF { BG_HIGH } else { 0 });
            cpu.write16(g + BG_COLOUR, colour);
            let shimmer = (cpu.read32(globals::LOOPS) >> 2 & 0xf) as u16;
            let colour = shimmer.wrapping_add(cpu.read16(g + BG_COLOUR));
            cpu.write16(g + BG_COLOUR, colour);
            if cpu.read32(g + BG_SHOWN) == 0 {
                cpu.write8(g + BG_PHASE, 2);
            }
        }
        2 => {
            cpu.write32(BOSS_GAUGE, 0);
            cpu.write8(g + BG_MODE, BG_ENDING);
            cpu.write8(g + BG_PHASE, 3);
        }
        _ => {}
    }
    if (cpu.read8(g + BG_MODE) as i8) < BG_ENDING as i8 {
        nested!(cpu, 0x0604_0960, super::sprites::draw(cpu, g + BG_SPRITE));
    }
}

