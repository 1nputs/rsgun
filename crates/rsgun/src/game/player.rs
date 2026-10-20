//! The players' ships (actors.rs's player_set makes them): each frame a ship takes its pad, picks
//! its state by the buttons held (a weapon: A, B, C and their combinations; the sword with all
//! three, the hyper sword once the gauge is full), dies on the ground, counts its weapons' level
//! ups and hit bonuses and its extends, runs its state's behaviour, and counts its time in each
//! state in its PLAY_DATA. While hurt it blinks.

use super::globals::{LOOPS, PLAY_DATA, PLAY_DATA_SIZE, SCORE};
use super::{bg, effects, hits, math, nested, objects, sound, Cpu};

/// A player ship's fields: its flags, position, colours, kind, frame, number (0, 1), lift, type
/// (its weapon), hit flags, gauge (its life field: up to 20000), state (a word: its low byte the
/// state), speed, the hyper sword's readiness, a byte cleared by every weapon but B+C and the
/// sword, the frames it stays hurt (blinking), its pad (PAD_INF: held, changed, pressed, let go).
const FLAGS: u32 = 0x10;
const POSITION: u32 = 0x14;
const NUMBER: u32 = 0x32;
const LIFT: u32 = 0x33;
const TYPE: u32 = 0x55;
const HIT_FLAGS: u32 = 0x56;
const GAUGE: u32 = 0x50;
const STATE: u32 = 0x6c;
const STATE_LOW: u32 = 0x6d;
const SPEED: u32 = 0x80;
const HYPER_READY: u32 = 0x8b;
const BYTE_8C: u32 = 0x8c;
const HURT: u32 = 0x90;
const PAD: u32 = 0xa0;
/// The flags: not drawn (blinking), not hurt.
const NO_DRAW: u32 = 0x8000_0000;
const UNHURT: u32 = 0x4_0000;
/// The hit flags: flashing, was hit.
const FLASH: u16 = 1;
const WAS_HIT: u16 = 0x200;

/// The states: 0-2 (coming in, ..., dead: no control), 3 no weapon, 4-9 the weapons by buttons,
/// 10 the sword, 11 the hyper sword.
const DEAD: u16 = 2;
const NO_WEAPON: u8 = 3;
const SWORD: u8 = 10;
const HYPER_SWORD: u8 = 11;
/// Each state's behaviour (SH-2 functions, by state).
const STATES: u32 = 0x0607_2768;

/// The players' pads (pads.rs's PAD_INF copies: the third and fourth).
const PADS: [u32; 2] = [0x0602_ee20, 0x0602_ee28];
/// While set, the pads are not taken (director.rs's FULL_SPEED: demos feed them).
const FULL_SPEED: u32 = 0x0600_ed90;
/// PLAY_DATA's frames played, and the frames in each state from 3 (longs).
const PD_FRAMES: u32 = 0x14;
const PD_STATE_FRAMES: u32 = 0x18;
const FIRST_COUNTED: u8 = 3;
const LAST_COUNTED: u8 = 11;
const BLINK_FROM: i16 = 16;

fn number(cpu: &mut Cpu, p: u32) -> i32 {
    cpu.read8(p + NUMBER) as i8 as i32
}

fn play_data(cpu: &mut Cpu, p: u32) -> u32 {
    PLAY_DATA.wrapping_add((number(cpu, p) * PLAY_DATA_SIZE as i32) as u32)
}

/// A player ship's behaviour.
pub fn player_exec(cpu: &mut Cpu, p: u32) {
    if cpu.read8(FULL_SPEED) == 0 {
        let pad = PADS[(number(cpu, p) != 0) as usize];
        for k in 0..4 {
            let v = cpu.read16(pad + 2 * k);
            cpu.write16(p + PAD + 2 * k, v);
        }
    }
    nested!(cpu, 0x0604_7b0c, control(cpu, p));
    nested!(cpu, 0x0604_77ec, weapons_frame(cpu, p));
    if cpu.read8(FULL_SPEED) == 0 {
        let pd = play_data(cpu, p);
        let n = cpu.read32(pd + PD_FRAMES).wrapping_add(1);
        cpu.write32(pd + PD_FRAMES, n);
        let state = cpu.read8(p + STATE_LOW);
        if (FIRST_COUNTED..=LAST_COUNTED).contains(&state) {
            let pd = play_data(cpu, p);
            let at = pd + PD_STATE_FRAMES + 4 * (state - FIRST_COUNTED) as u32;
            let n = cpu.read32(at).wrapping_add(1);
            cpu.write32(at, n);
        }
    }
    let hurt = cpu.read16(p + HURT) as i16;
    if hurt != 0 {
        let flags = cpu.read32(p + FLAGS);
        let flags = if hurt >= BLINK_FROM { flags ^ NO_DRAW } else { flags & !NO_DRAW };
        cpu.write32(p + FLAGS, flags);
        let h = cpu.read16(p + HIT_FLAGS) & !FLASH;
        cpu.write16(p + HIT_FLAGS, h);
        let hurt = cpu.read16(p + HURT).wrapping_sub(1);
        cpu.write16(p + HURT, hurt);
    }
}

/// The gauge's full, and the half it drops to after the hyper sword.
const GAUGE_FULL: i32 = 20000;
const GAUGE_HALF: u32 = 10000;
/// The hyper sword's aura (an effect following the ship) and its sound.
const AURA_SIZE: u32 = 0x80;
const AURA_EXEC: u32 = 0x0604_7a18;
const AURA_KIND: u8 = 3;
const AURA_FRAME: u8 = 60;
const AURA_COLOUR: u16 = 0x20;
const AURA_OF: u32 = 0x5c;
const SE_HYPER_READY: u32 = 0x44;
const PLAIN_EXEC: u32 = 0x0604_dcb4;
const CHAR_BASE: u32 = 0x0607_41fc;
const GAME_FRAMES: u32 = super::globals::GAME_FRAMES;
/// The corners of the ship tested against the ground in turn (x, y from its centre), the score
/// a scrape gives (as weapon 18), and the scrape's spark (polygon_set's struct: a line in a random
/// colour, slowing).
const CORNERS: [(u32, u32); 4] = [(0xe_0000, 0xe_0000), (0xfff2_0000, 0xe_0000), (0xe_0000, 0xfff6_0000), (0xfff2_0000, 0xfff6_0000)];
const SCRAPE_WEAPON: u32 = 18;
const SCRAPE_POINTS: u32 = 3;
const SCRAPE_FLAGS: u32 = 0x2b;
const SCRAPE_COLOUR: u16 = 0x10;
/// While set, the ground does not kill (backup.rs's flag that the settings were cleared).
const GROUND_SAFE: u32 = 0x0600_edbc;
/// The buttons: A, B, C (PAD_INF bits); the setting that tells three buttons from two.
const BUTTON_A: u16 = 0x400;
const BUTTON_B: u16 = 0x200;
const BUTTON_C: u16 = 0x100;
const SL_TURN: u32 = 0x0606_4eac;

fn random(cpu: &mut Cpu, n: u32) -> u32 {
    nested!(cpu, 0x0604_5d90, math::random(cpu, n))
}

/// A ship's control (from state 3 on): the hyper sword made ready once the gauge is full (with its
/// aura and sound; ready no more once the gauge is back at half); a scrape of one of its corners on
/// the ground (in turn, a corner a frame) scores and sparks; touching the ground with its centre
/// kills it (unless it is not to be hurt, still hurt, or the ground is safe); then its state by
/// the buttons held (not in the hyper sword).
pub fn control(cpu: &mut Cpu, p: u32) {
    if cpu.read8(p + STATE_LOW) as i8 <= DEAD as i8 {
        return;
    }
    if cpu.read8(p + HYPER_READY) == 0 {
        if cpu.read32(p + GAUGE) as i32 >= GAUGE_FULL {
            let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, AURA_SIZE)) as i8 as i32 as u32;
            let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 1, blocks, PLAIN_EXEC));
            if o != 0 {
                nested!(cpu, 0x0604_dd50, effects::actor_init(cpu, o));
                cpu.write32(o + 0x38, AURA_EXEC);
                cpu.write8(o + 0x2e, AURA_KIND);
                cpu.write8(o + 0x2f, AURA_FRAME);
                cpu.write8(o + LIFT, 4);
                let colour = cpu.read16(CHAR_BASE).wrapping_add(AURA_COLOUR);
                cpu.write16(o + 0x2a, colour);
                cpu.write16(o + 0x28, colour);
                let n = cpu.read8(p + NUMBER);
                cpu.write8(o + NUMBER, n);
                cpu.write32(o + AURA_OF, p);
            }
            nested!(cpu, 0x0600_50d4, sound::se_set(cpu, SE_HYPER_READY));
            cpu.write8(p + HYPER_READY, 1);
        }
    } else if cpu.read32(p + GAUGE) == GAUGE_HALF {
        cpu.write8(p + HYPER_READY, 0);
    }
    scrape(cpu, p);
    if cpu.read16(p + HIT_FLAGS) & WAS_HIT == 0 {
        let (x, y) = (cpu.read32(p + POSITION), cpu.read32(p + POSITION + 4));
        if nested!(cpu, 0x0605_ef90, bg::map_hit(cpu, x, y)) != 0 {
            let h = cpu.read16(p + HIT_FLAGS) | WAS_HIT;
            cpu.write16(p + HIT_FLAGS, h);
            if cpu.read32(LOOPS) & 1 != 0 {
                cpu.write16(p + HIT_FLAGS, h | FLASH);
            }
        }
    }
    if cpu.read16(p + HIT_FLAGS) & WAS_HIT != 0
        && cpu.read32(p + FLAGS) & UNHURT == 0
        && cpu.read16(p + HURT) == 0
        && cpu.read8(GROUND_SAFE) == 0
    {
        cpu.write32(p + GAUGE, 0);
        cpu.write16(p + STATE, DEAD);
        return;
    }
    if cpu.read8(p + STATE_LOW) == HYPER_SWORD {
        return;
    }
    let held = cpu.read16(p + PAD);
    let state = if super::globals::bios_setting_is_1(cpu) {
        match held & (BUTTON_A | BUTTON_B | BUTTON_C) {
            BUTTON_A => weapon(cpu, p, 4),
            BUTTON_C => weapon(cpu, p, 5),
            BUTTON_B => weapon(cpu, p, 6),
            0x300 => 9,
            0x500 => weapon(cpu, p, 7),
            0x600 => weapon(cpu, p, 8),
            0x700 => sword(cpu, p),
            _ => weapon(cpu, p, NO_WEAPON),
        }
    } else {
        match held & (BUTTON_A | BUTTON_C) {
            BUTTON_A => weapon(cpu, p, 4),
            BUTTON_C => weapon(cpu, p, 6),
            0x500 => sword(cpu, p),
            _ => weapon(cpu, p, NO_WEAPON),
        }
    };
    if cpu.read8(p + STATE_LOW) != state {
        cpu.write16(p + STATE, state as u16);
    }
}

fn weapon(cpu: &mut Cpu, p: u32, state: u8) -> u8 {
    cpu.write8(p + BYTE_8C, 0);
    state
}

/// The sword: kept if it is the sword already, else the hyper sword if ready.
fn sword(cpu: &mut Cpu, p: u32) -> u8 {
    let state = cpu.read8(p + STATE_LOW);
    if state == SWORD {
        return SWORD;
    }
    if cpu.read8(p + HYPER_READY) != 0 { HYPER_SWORD } else { SWORD }
}

/// The scrape test: one corner a frame (by the game frame) against the ground.
fn scrape(cpu: &mut Cpu, p: u32) {
    let f = cpu.stack();
    let (dx, dy) = CORNERS[(cpu.read32(GAME_FRAMES) & 3) as usize];
    let x = dx.wrapping_add(cpu.read32(p + POSITION));
    cpu.write32(f, x);
    let y = dy.wrapping_add(cpu.read32(p + POSITION + 4));
    cpu.write32(f + 4, y);
    if nested!(cpu, 0x0605_ef90, bg::map_hit(cpu, x, y)) == 0 {
        return;
    }
    let n = number(cpu, p) as u32;
    nested!(cpu, 0x0604_3108, hits::score_kill(cpu, n, SCRAPE_WEAPON, 0, SCRAPE_POINTS));
    let z = cpu.read32(p + POSITION + 8);
    cpu.write32(f + 8, z);
    cpu.write32(f + 0x10, 0x1_0000);
    cpu.write32(f + 0x0c, 0x1_0000);
    cpu.write32(f + 0x14, SCRAPE_FLAGS);
    let a = random(cpu, 0);
    cpu.write16(f + 0x18, a as u16);
    let frames = random(cpu, 5) as u16 + 15;
    cpu.write16(f + 0x24, frames);
    cpu.write8(f + 0x26, 0);
    let colour = cpu.read16(CHAR_BASE).wrapping_add(SCRAPE_COLOUR);
    cpu.write16(f + 0x20, colour);
    let n = cpu.read8(p + NUMBER);
    cpu.write8(f + 0x1e, n);
    let spark = nested!(cpu, 0x0604_e6d8, effects::polygon_set(cpu, f));
    if spark == 0 {
        return;
    }
    let lift = cpu.read8(p + LIFT);
    cpu.write8(spark + LIFT, lift);
    for k in 0..3 {
        cpu.write16(spark + 0x70 + 2 * k, 0);
    }
    let r = random(cpu, 4);
    cpu.write16(spark + 0x76, r as u16);
    let r = random(cpu, 0x2000);
    cpu.write16(spark + 0x42, r as u16);
    // its speed: up to 3 pixels a frame, turned up or down by a little
    cpu.write32(f, 0);
    let r = random(cpu, 0x2_0000);
    cpu.write32(f + 4, r.wrapping_add(0xfffd_0000));
    cpu.write32(f + 0x14, 0);
    let a = random(cpu, 0x800) as u16;
    cpu.write16(f + 0x18, a);
    let side = if random(cpu, 0) as i32 > 0 { 0x4800 } else { 0xb800 };
    let a = cpu.read16(f + 0x18).wrapping_add(side);
    cpu.write16(f + 0x18, a);
    cpu.call(SL_TURN, &[f]);
    let (vx, vy) = (cpu.read32(f), cpu.read32(f + 4));
    cpu.write32(spark + 0x44, vx);
    cpu.write32(spark + 0x48, vy);
}

/// Each player's weapons' levels (A, B, C bytes, 3 a player; at most 33), their experience (longs,
/// hits.rs's), and the experience each level up needs.
const LEVELS: u32 = 0x0607_4088;
const EXPERIENCE: u32 = 0x0607_4070;
const LEVEL_UP: u32 = 0x0607_2798;
const LEVEL_MOST: i8 = 32;
/// The weapons' hit counts (hits.rs's: 28 bytes a player; A, B, C, A+B, A+C's timer and count
/// (words), B+C, A+B+C) and the hits past which each gives a bonus of 10000 (the count cleared).
const WEAPON_HITS: u32 = 0x0607_40a0;
const HITS_SIZE: u32 = 28;
const BONUS: u32 = 10000;
const SCORE_KILL: u32 = 0x0604_3108;

/// A ship's weapons each frame: each weapon's level up once its experience reaches the next
/// level's; its extends; the hit bonuses; then its state's behaviour; its speed cleared.
pub fn weapons_frame(cpu: &mut Cpu, p: u32) {
    for w in 0..3 {
        let at = LEVELS.wrapping_add((number(cpu, p) * 3) as u32 + w);
        let level = cpu.read8(at) as i8;
        if level <= LEVEL_MOST {
            let need = cpu.read32(LEVEL_UP.wrapping_add((level as i32 * 4) as u32)) as i32;
            let n = number(cpu, p);
            let have = cpu.read32(EXPERIENCE.wrapping_add((n * 12) as u32 + 4 * w)) as i32;
            if have >= need {
                cpu.write8(at, (level + 1) as u8);
            }
        }
    }
    nested!(cpu, 0x0604_75d4, extend_check(cpu, p));
    // (the SH-2 code keeps the ship's position in its frame)
    let frame = cpu.stack();
    for k in 0..3 {
        let v = cpu.read32(p + POSITION + 4 * k);
        cpu.write32(frame + 4 * k, v);
    }
    let hits = |cpu: &mut Cpu| WEAPON_HITS.wrapping_add((number(cpu, p) * HITS_SIZE as i32) as u32);
    let bonus = |cpu: &mut Cpu| {
        let n = number(cpu, p) as u32;
        let weapon = cpu.read8(p + TYPE) as i8 as i32 as u32;
        nested!(cpu, SCORE_KILL, hits::score_kill(cpu, n, weapon, 0, BONUS));
    };
    for (field, past) in [(0, 199), (4, 299), (8, 15), (12, 599)] {
        let h = hits(cpu);
        if cpu.read32(h + field) as i32 > past {
            bonus(cpu);
            let h = hits(cpu);
            cpu.write32(h + field, 0);
        }
    }
    let h = hits(cpu);
    if cpu.read16(h + 18) as i16 > 299 {
        bonus(cpu);
        let h = hits(cpu);
        cpu.write16(h + 18, 0);
    }
    let h = hits(cpu);
    let timer = cpu.read16(h + 16) as i16;
    if timer > 0 {
        cpu.write16(h + 16, (timer - 1) as u16);
    } else {
        cpu.write16(h + 18, 0);
    }
    for (field, past) in [(20, 900), (24, 300)] {
        let h = hits(cpu);
        if cpu.read32(h + field) as i32 > past {
            bonus(cpu);
            let h = hits(cpu);
            cpu.write32(h + field, 0);
        }
    }
    let state = cpu.read8(p + STATE_LOW) as u32;
    let behaviour = cpu.read32(STATES + 4 * state);
    cpu.call(behaviour, &[p]);
    cpu.write32(p + SPEED + 4, 0);
    cpu.write32(p + SPEED, 0);
}

/// The extend setting (backup.rs's third), PLAY_DATA's score as last seen, each player's lives,
/// the extend's sound.
const EXTEND_SETTING: u32 = 0x0600_cc60;
const LAST_SCORE: u32 = 0x44;
const LIVES: u32 = 0x0607_4062;
const SE_EXTEND: u32 = 23;

/// A ship's extends: a life at each score the setting gives (0: every million, 1: every three
/// million, 2: one and three million, 3: one and five, 4: three, 5: five), when the score passes it.
pub fn extend_check(cpu: &mut Cpu, p: u32) {
    let pd = play_data(cpu, p);
    if cpu.read32(pd + SCORE) == cpu.read32(pd + LAST_SCORE) {
        return;
    }
    let passes = |cpu: &mut Cpu, at: i32| {
        let pd = play_data(cpu, p);
        cpu.read32(pd + LAST_SCORE) as i32 <= at && cpu.read32(pd + SCORE) as i32 > at
    };
    let every = |cpu: &mut Cpu, step: u32| {
        let pd = play_data(cpu, p);
        let score = cpu.read32(pd + SCORE);
        let now = nested!(cpu, 0x0600_6a78, math::quotient(cpu, score, step)) as i32;
        let pd = play_data(cpu, p);
        let last = cpu.read32(pd + LAST_SCORE);
        let before = nested!(cpu, 0x0600_6a78, math::quotient(cpu, last, step)) as i32;
        now > before
    };
    match cpu.read8(EXTEND_SETTING) {
        0 => {
            if every(cpu, 1_000_000) {
                extend(cpu, p);
            }
        }
        1 => {
            if every(cpu, 3_000_000) {
                extend(cpu, p);
            }
        }
        setting @ 2..=5 => {
            if setting <= 3 && passes(cpu, 999_999) {
                extend(cpu, p);
            }
            let at = if setting % 2 == 0 { 2_999_999 } else { 4_999_999 };
            if passes(cpu, at) {
                extend(cpu, p);
            }
        }
        _ => {}
    }
    let pd = play_data(cpu, p);
    let score = cpu.read32(pd + SCORE);
    cpu.write32(pd + LAST_SCORE, score);
}

fn extend(cpu: &mut Cpu, p: u32) {
    let at = LIVES.wrapping_add(number(cpu, p) as u32);
    let lives = cpu.read8(at).wrapping_add(1);
    cpu.write8(at, lives);
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, SE_EXTEND));
}
