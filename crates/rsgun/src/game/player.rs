//! The players' ships (actors.rs's player_set makes them): each frame a ship takes its pad, picks
//! its state by the buttons held (a weapon: A, B, C and their combinations; the sword with all
//! three, the hyper sword once the gauge is full), dies on the ground, counts its weapons' level
//! ups and hit bonuses and its extends, runs its state's behaviour, and counts its time in each
//! state in its PLAY_DATA. While hurt it blinks.

use super::globals::{LOOPS, PLAY_DATA, PLAY_DATA_SIZE, SCORE};
use super::{actors, bg, effects, hits, math, nested, objects, sound, sprites, Cpu};

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

/// The states: 0 start, 1 coming in, 2 dead, 3 no weapon, 4-9 the weapons by buttons (A, B, C,
/// A+B, A+C, B+C), 10 the sword (A+B+C), 11 the hyper sword.
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
const BUTTON_B: u16 = 0x100;
const BUTTON_C: u16 = 0x200;
const TURN_POINT: u32 = 0x0606_4eac;

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
            BUTTON_B => weapon(cpu, p, 5),
            BUTTON_C => weapon(cpu, p, 6),
            0x500 => weapon(cpu, p, 7),
            0x600 => weapon(cpu, p, 8),
            0x300 => 9,
            0x700 => sword(cpu, p),
            _ => weapon(cpu, p, NO_WEAPON),
        }
    } else {
        // two buttons: B gives C's weapon, A and B together the sword
        match held & (BUTTON_A | BUTTON_B) {
            BUTTON_A => weapon(cpu, p, 4),
            BUTTON_B => weapon(cpu, p, 6),
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
    nested!(cpu, TURN_POINT, super::math::turn_point(cpu, f));
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
    // the SH-2 code holds the ship, its number's address, the hits' size, their base, a field's
    // address and a limit in r8-r13 here: the state's callees push them (read later as an unset
    // field), and the call's return address, 0x060479e0
    let regs = [(8, p), (9, p + NUMBER), (10, HITS_SIZE), (11, WEAPON_HITS), (12, WEAPON_HITS + 18), (13, 299)];
    cpu.holding(&regs, |cpu| cpu.call_returning(0x0604_79e0, behaviour, &[p]));
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
        0 if every(cpu, 1_000_000) => extend(cpu, p),
        1 if every(cpu, 3_000_000) => extend(cpu, p),
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

/// The state's phase (the high byte of STATE: each weapon state starts at 0, waits a frame, then
/// fires (2) and waits its reload (3)), the shots fired in the state, the frames into the phase,
/// counts of each weapon's shots alive (bytes, the shots count themselves each frame: A, B, C,
/// A+C, B+C, the hyper sword, all), the shot limits' flags, the sideways and upward drift (banking)
/// and the tilt (a byte: its frame's index).
const PHASE: u32 = 0x6c;
const SHOTS: u32 = 0x89;
const PHASE_FRAMES: u32 = 0x88;
const ALIVE_A: u32 = 0x80;
const ALIVE_B: u32 = 0x81;
const ALIVE_C: u32 = 0x82;
const ALIVE_AC: u32 = 0x84;
const ALIVE_BC: u32 = 0x85;
const ALIVE_HYPER: u32 = 0x86;
const ALIVE_ALL: u32 = 0x87;
const LIMITS: u32 = 0xa8;
const LIMIT_ALL: u32 = 0x0800_0000;
const LIMIT_EACH: u32 = 0x0400_0000;
const MOVES_WITH_SWORD: u32 = 0x0070_0000;
const BANK_X: u32 = 0x94;
const BANK_Y: u32 = 0x98;
const TILT: u32 = 0x9f;
const ANGLE: u32 = 0x30;
const FRAME: u32 = 0x2f;
/// The frames by tilt (32 a row: level, moving up, faster up).
const TILT_FRAMES: u32 = 0x0607_2700;
/// The B+C laser's direction (by the stick when it starts) and whether it was taken.
const AIM: u32 = 0x8e;
const AIM_TAKEN: u32 = BYTE_8C;

/// The speeds: straight (1.875 pixels a frame) and diagonal (1.5).
const STRAIGHT: u32 = 0x1_e000;
const DIAGONAL: u32 = 0x1_8000;
const SCREEN_X: u32 = 0xa0_0000;
const SCREEN_Y: u32 = 0x60_0000;
/// The stick's bits in the pad.
const RIGHT: u16 = 0x8000;
const LEFT: u16 = 0x4000;
const DOWN: u16 = 0x2000;
const UP: u16 = 0x1000;

fn neg(v: u32) -> u32 {
    v.wrapping_neg()
}

/// A ship's move by its stick (kept on screen), its banking eased toward its motion and its frame
/// by the tilt: banked sideways by the sideways drift, a row on for moving up (and another for
/// moving up fast).
pub fn move_ship(cpu: &mut Cpu, p: u32) {
    let held = cpu.read16(p + PAD);
    let (mut dx, mut dy) = (0u32, 0u32);
    if held & (UP | DOWN) == 0 {
        if held & RIGHT != 0 {
            dx = STRAIGHT;
        }
        if held & LEFT != 0 {
            dx = neg(STRAIGHT);
        }
    } else {
        if held & RIGHT != 0 {
            dx = DIAGONAL;
        }
        if held & LEFT != 0 {
            dx = neg(DIAGONAL);
        }
        let speed = if dx == 0 { STRAIGHT } else { DIAGONAL };
        if held & UP != 0 {
            dy = neg(speed);
        }
        if held & DOWN != 0 {
            dy = speed;
        }
    }
    let x = cpu.read32(p + POSITION).wrapping_add(dx);
    cpu.write32(p + POSITION, x);
    let y = cpu.read32(p + POSITION + 4).wrapping_add(dy);
    cpu.write32(p + POSITION + 4, y);
    let x = cpu.read32(p + POSITION);
    let x = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, x, SCREEN_X));
    cpu.write32(p + POSITION, x);
    let y = cpu.read32(p + POSITION + 4);
    let y = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, y, SCREEN_Y));
    cpu.write32(p + POSITION + 4, y);
    let bank = cpu.read32(p + BANK_X);
    let bank = nested!(cpu, 0x0604_6168, super::actors::approach(cpu, bank, 0, 0x100)) as i32;
    cpu.write32(p + BANK_X, bank as u32);
    let bank = bank.wrapping_add((dx as i32).wrapping_sub(bank) / 8);
    cpu.write32(p + BANK_X, bank as u32);
    let bank_y = cpu.read32(p + BANK_Y);
    let bank_y = nested!(cpu, 0x0604_6168, super::actors::approach(cpu, bank_y, 0, 0x100));
    cpu.write32(p + BANK_Y, bank_y);
    let tilt = (cpu.read32(p + BANK_X) as i32 / 24576) as u8;
    cpu.write8(p + TILT, tilt);
    if (tilt as i8) < 0 {
        cpu.write8(p + TILT, tilt.wrapping_add(32));
    }
    for past in [0x1000, 0xc000] {
        let rising = (dy as i32).wrapping_sub(cpu.read32(p + BANK_Y) as i32);
        if rising > past {
            let t = cpu.read8(p + TILT).wrapping_add(32);
            cpu.write8(p + TILT, t);
        }
    }
    let bank_y = cpu.read32(p + BANK_Y) as i32;
    cpu.write32(p + BANK_Y, bank_y.wrapping_add((dy as i32).wrapping_sub(bank_y) / 16) as u32);
    let t = cpu.read8(p + TILT) as i8 as i32 as u32;
    let frame = cpu.read8(TILT_FRAMES.wrapping_add(t));
    cpu.write8(p + FRAME, frame);
}

fn phase(cpu: &mut Cpu, p: u32) -> u8 {
    cpu.read8(p + PHASE)
}

fn set_phase(cpu: &mut Cpu, p: u32, phase: u8) {
    cpu.write8(p + PHASE, phase);
}

/// The phase's frames counted: past `wait`, on to `next`.
fn wait(cpu: &mut Cpu, p: u32, wait: i8, next: u8) {
    let n = cpu.read8(p + PHASE_FRAMES).wrapping_add(1);
    cpu.write8(p + PHASE_FRAMES, n);
    if n as i8 > wait {
        set_phase(cpu, p, next);
    }
}

fn hits_at(cpu: &mut Cpu, p: u32) -> u32 {
    WEAPON_HITS.wrapping_add((number(cpu, p) * HITS_SIZE as i32) as u32)
}

/// A weapon state's start: the B+C and A+B+C hit counts cleared (`both`, else only A+B+C's), the
/// shots and frames cleared, on to the first wait.
fn start(cpu: &mut Cpu, p: u32, clear_bc: bool, clear_frames: bool) {
    let h = hits_at(cpu, p);
    cpu.write32(h + 24, 0);
    if clear_bc {
        cpu.write32(h + 20, 0);
    }
    cpu.write8(p + SHOTS, 0);
    if clear_frames {
        cpu.write8(p + PHASE_FRAMES, 0);
    }
    set_phase(cpu, p, 1);
}

/// Whether the shots alive are at a limit: all of them (`all` counted past `most`, with the
/// limit on all) or this weapon's (`each`).
fn at_limit(cpu: &mut Cpu, p: u32, each: u32, most: i8) -> bool {
    let limits = cpu.read32(p + LIMITS);
    (limits & LIMIT_ALL != 0 && cpu.read8(p + ALIVE_ALL) as i8 > most) || (limits & LIMIT_EACH != 0 && cpu.read8(p + each) as i8 > most)
}

/// A shot fired: its sound and its maker, counted; on to the reload.
fn fire(cpu: &mut Cpu, p: u32, se: u32, maker: u32) {
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, se));
    cpu.call(maker, &[p]);
    fired(cpu, p);
}

fn fired(cpu: &mut Cpu, p: u32) {
    let n = cpu.read8(p + SHOTS).wrapping_add(1);
    cpu.write8(p + SHOTS, n);
    cpu.write8(p + PHASE_FRAMES, 0);
    set_phase(cpu, p, 3);
}

/// The shots' makers (not rewritten yet).
const SHOT_A: u32 = 0x0604_860c;
const SHOT_B: u32 = 0x0604_8d84;
const SHOT_C: u32 = 0x0604_965c;
const SHOT_AB: u32 = 0x0604_a854;
const SHOT_AC: u32 = 0x0604_aa98;
const SHOT_BC: u32 = 0x0604_bdf8;
const SWORD_SET: u32 = 0x0604_c9a4;
const HYPER_SET: u32 = 0x0604_d444;

/// Weapon A (state 4): up to 15 shots alive, one each 5 frames.
pub fn weapon_a(cpu: &mut Cpu, p: u32) {
    nested!(cpu, 0x0604_6900, move_ship(cpu, p));
    match phase(cpu, p) {
        0 | 1 => {
            if phase(cpu, p) == 0 {
                start(cpu, p, true, true);
            }
            wait(cpu, p, 0, 2);
        }
        2 => {
            if !at_limit(cpu, p, ALIVE_A, 14) {
                fire(cpu, p, 2, SHOT_A);
            }
        }
        3 => wait(cpu, p, 4, 2),
        _ => {}
    }
}

/// Weapon B (state 5, homing): up to 13 shots alive, one each 7 frames.
pub fn weapon_b(cpu: &mut Cpu, p: u32) {
    nested!(cpu, 0x0604_6900, move_ship(cpu, p));
    match phase(cpu, p) {
        0 | 1 => {
            if phase(cpu, p) == 0 {
                start(cpu, p, true, true);
            }
            wait(cpu, p, 0, 2);
        }
        2 => {
            if !at_limit(cpu, p, ALIVE_B, 12) {
                fire(cpu, p, 4, SHOT_B);
            }
        }
        3 => wait(cpu, p, 6, 2),
        _ => {}
    }
}

/// Weapon C (state 6): up to 5 shots alive (with two buttons, under 7.5: the source's double),
/// one each 24 frames.
pub fn weapon_c(cpu: &mut Cpu, p: u32) {
    nested!(cpu, 0x0604_6900, move_ship(cpu, p));
    match phase(cpu, p) {
        0 | 1 => {
            if phase(cpu, p) == 0 {
                start(cpu, p, true, true);
            }
            wait(cpu, p, 0, 2);
        }
        2 => {
            let three_buttons = super::globals::bios_setting_is_1(cpu);
            let limits = cpu.read32(p + LIMITS);
            let over = |cpu: &mut Cpu, three: u32| {
                if three_buttons {
                    cpu.read8(p + three) as i8 > 4
                } else {
                    cpu.read8(p + ALIVE_ALL) as i8 as f64 >= 7.5
                }
            };
            if limits & LIMIT_ALL != 0 && over(cpu, ALIVE_ALL) {
                return;
            }
            if cpu.read32(p + LIMITS) & LIMIT_EACH != 0 && over(cpu, ALIVE_C) {
                return;
            }
            fire(cpu, p, 7, SHOT_C);
        }
        3 => wait(cpu, p, 23, 2),
        _ => {}
    }
}

/// A+B (state 7): its shot once (the shot keeps itself up while the buttons are held).
pub fn weapon_ab(cpu: &mut Cpu, p: u32) {
    nested!(cpu, 0x0604_6900, move_ship(cpu, p));
    match phase(cpu, p) {
        0 | 1 => {
            if phase(cpu, p) == 0 {
                start(cpu, p, true, false);
            }
            wait(cpu, p, 0, 2);
        }
        2 => {
            nested!(cpu, SHOT_AB, super::shots::shot_ab_set(cpu, p));
            set_phase(cpu, p, 3);
        }
        _ => {}
    }
}

/// C's weapon level (each player's three: A, B, C), which A+C's limit grows with.
const LEVEL_C: u32 = LEVELS + 2;

/// A+C (state 8): its shots one each 7 frames, up to 16 alive (24 from C's level 8, 32 from 24).
pub fn weapon_ac(cpu: &mut Cpu, p: u32) {
    nested!(cpu, 0x0604_6900, move_ship(cpu, p));
    match phase(cpu, p) {
        0 | 1 => {
            if phase(cpu, p) == 0 {
                start(cpu, p, true, true);
            }
            wait(cpu, p, 0, 2);
        }
        2 => {
            let n = number(cpu, p);
            let level = cpu.read8(LEVEL_C.wrapping_add((n * 3) as u32)) as i8;
            let most = if level <= 7 { 4 } else if level <= 23 { 6 } else { 8 } * 4;
            let limits = cpu.read32(p + LIMITS);
            if limits & LIMIT_ALL != 0 && cpu.read8(p + ALIVE_ALL) as i8 as i32 >= most {
                return;
            }
            if limits & LIMIT_EACH != 0 && cpu.read8(p + ALIVE_AC) as i8 as i32 >= most {
                return;
            }
            nested!(cpu, SHOT_AC, super::shots::shot_ac_set(cpu, p));
            nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 3));
            fired(cpu, p);
        }
        3 => wait(cpu, p, 6, 2),
        _ => {}
    }
}

/// B+C's directions by the stick (0 up, clockwise: a stick held as it starts; up alone keeps 0).
const AIMS: [(u16, u16); 7] = [(0x5000, 0xe000), (0x2000, 0x8000), (0x4000, 0xc000), (0x6000, 0xa000), (0x8000, 0x4000), (0x9000, 0x2000), (0xa000, 0x6000)];

/// B+C (state 9): its laser aimed by the stick as the state starts (unless it was aimed already),
/// one at a time, each 29 frames.
pub fn weapon_bc(cpu: &mut Cpu, p: u32) {
    nested!(cpu, 0x0604_6900, move_ship(cpu, p));
    match phase(cpu, p) {
        0 | 1 => {
            if phase(cpu, p) == 0 {
                let h = hits_at(cpu, p);
                cpu.write32(h + 24, 0);
                if cpu.read8(p + AIM_TAKEN) == 0 {
                    let stick = cpu.read16(p + PAD) & 0xf000;
                    let aim = AIMS.iter().find(|&&(s, _)| s == stick).map_or(0, |&(_, a)| a);
                    cpu.write16(p + AIM, aim);
                }
                cpu.write8(p + AIM_TAKEN, 1);
                cpu.write8(p + SHOTS, 0);
                cpu.write8(p + PHASE_FRAMES, 0);
                set_phase(cpu, p, 1);
            }
            wait(cpu, p, 0, 2);
        }
        2 => {
            if cpu.read8(p + ALIVE_BC) as i8 > 0 {
                return;
            }
            nested!(cpu, SHOT_BC, super::shots::shot_bc_set(cpu, p));
            nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 5));
            fired(cpu, p);
        }
        3 => wait(cpu, p, 28, 2),
        _ => {}
    }
}

/// The sword (state 10): moving only if the ship's limits allow; a swing once (when the sword
/// can be made), then held.
pub fn sword_state(cpu: &mut Cpu, p: u32) {
    if cpu.read32(p + LIMITS) & MOVES_WITH_SWORD != 0 {
        nested!(cpu, 0x0604_6900, move_ship(cpu, p));
    }
    match phase(cpu, p) {
        0 | 1 => {
            if phase(cpu, p) == 0 {
                let h = hits_at(cpu, p);
                cpu.write32(h + 20, 0);
                cpu.write8(p + SHOTS, 0);
                cpu.write8(p + PHASE_FRAMES, 0);
                set_phase(cpu, p, 1);
            }
            wait(cpu, p, 0, 2);
        }
        2 => {
            if nested!(cpu, SWORD_SET, super::shots::shot_sword_set(cpu, p)) == 0 {
                return;
            }
            nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 9));
            let n = cpu.read8(p + SHOTS).wrapping_add(1);
            cpu.write8(p + SHOTS, n);
            cpu.write8(p + PHASE_FRAMES, 0);
            set_phase(cpu, p, 4);
        }
        3 => wait(cpu, p, -1, 2),
        _ => {}
    }
}

/// The hyper sword's own aura.
const HYPER_AURA_EXEC: u32 = 0x0604_7228;
const HYPER_AURA_FRAME: u8 = 64;
/// The frames the ship stays hurt (blinking, not hit) after the hyper sword.
const AFTER_HYPER: u16 = 30;

/// The hyper sword (state 11): its aura, the ship not hurt, the gauge down to half; the hyper
/// sword once (when it can be made); when its shots are gone, the ship hurt (blinking) for 30
/// frames and back to no weapon.
pub fn hyper_state(cpu: &mut Cpu, p: u32) {
    if cpu.read32(p + LIMITS) & MOVES_WITH_SWORD != 0 {
        nested!(cpu, 0x0604_6900, move_ship(cpu, p));
    }
    match phase(cpu, p) {
        0 | 1 => {
            if phase(cpu, p) == 0 {
                let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, AURA_SIZE)) as i8 as i32 as u32;
                let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 1, blocks, PLAIN_EXEC));
                if o != 0 {
                    nested!(cpu, 0x0604_dd50, effects::actor_init(cpu, o));
                    cpu.write32(o + 0x38, HYPER_AURA_EXEC);
                    cpu.write8(o + 0x2e, AURA_KIND);
                    cpu.write8(o + 0x2f, HYPER_AURA_FRAME);
                    cpu.write8(o + LIFT, 4);
                    let colour = cpu.read16(CHAR_BASE).wrapping_add(AURA_COLOUR);
                    cpu.write16(o + 0x2a, colour);
                    cpu.write16(o + 0x28, colour);
                    let n = cpu.read8(p + NUMBER);
                    cpu.write8(o + NUMBER, n);
                    cpu.write32(o + AURA_OF, p);
                }
                let h = hits_at(cpu, p);
                cpu.write32(h + 20, 0);
                let flags = cpu.read32(p + FLAGS) | UNHURT;
                cpu.write32(p + FLAGS, flags);
                cpu.write32(p + GAUGE, GAUGE_HALF);
                cpu.write8(p + HYPER_READY, 0);
                cpu.write8(p + SHOTS, 0);
                cpu.write8(p + PHASE_FRAMES, 0);
                set_phase(cpu, p, 1);
            }
            wait(cpu, p, 0, 2);
        }
        2 => {
            if nested!(cpu, HYPER_SET, super::shots::hyper_set(cpu, p)) == 0 {
                return;
            }
            nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 10));
            fired(cpu, p);
        }
        3 => {
            let limits = cpu.read32(p + LIMITS);
            if limits & LIMIT_ALL != 0 && cpu.read8(p + ALIVE_ALL) != 0 {
                return;
            }
            if limits & LIMIT_EACH != 0 && cpu.read8(p + ALIVE_HYPER) != 0 {
                return;
            }
            let flags = cpu.read32(p + FLAGS) & !UNHURT;
            cpu.write32(p + FLAGS, flags);
            cpu.write16(p + HURT, AFTER_HYPER);
            cpu.write16(p + STATE, NO_WEAPON as u16);
        }
        _ => {}
    }
}

/// The hit flags' fields and the bytes a ship's start clears.
const HIT_58: u32 = 0x58;
const NO_HITS: u32 = 0x8_0000;

/// A ship's start (state 0): hittable, its banking and tilt none, its chains reset; no weapon.
pub fn ship_start(cpu: &mut Cpu, p: u32) {
    let flags = cpu.read32(p + FLAGS) & !NO_HITS;
    cpu.write32(p + FLAGS, flags);
    cpu.write16(p + HIT_58, 0);
    cpu.write8(p + HYPER_READY, 0);
    cpu.write32(p + BANK_Y, 0);
    cpu.write32(p + BANK_X, 0);
    for field in [0x9c, 0x9e, TILT] {
        cpu.write8(p + field, 0);
    }
    let n = number(cpu, p) as u32;
    nested!(cpu, 0x0604_5090, hits::chain_reset(cpu, n));
    cpu.write16(p + STATE, NO_WEAPON as u16);
}

/// The ship's entry: its frames left (a word) and its climb toward the screen (z per frame).
const ENTRY_LEFT: u32 = 0x6e;
const CLIMB: u32 = 0x70;
const ENTRY_FRAMES: u32 = 60;
const ENTRY_JET: u32 = 0x0606_23d4;
/// Where each player comes in from (x, and the sideways drift that brings it in), the depth and
/// height it starts at, its start upward drift; then it blinks for 180 frames.
const ENTRY_X: [(u32, u32); 2] = [(0x30_0000, 0xfffe_6667), (0xffd0_0000, 0x1_9999)];
const ENTRY_Y: u32 = 0x100_0000;
const ENTRY_Z: u32 = 0xff40_0000;
const ENTRY_BANK_Y: u32 = 0xfff1_0000;
const AFTER_ENTRY: u16 = 180;

/// A ship coming in (state 1): started below the screen and away from it (with its jet), it flies
/// up into place over 60 frames (sideways by the stick), then blinks and takes no weapon.
pub fn ship_entry(cpu: &mut Cpu, p: u32) {
    match phase(cpu, p) as i8 {
        0 => {
            nested!(cpu, 0x0604_66d4, ship_start(cpu, p));
            let flags = cpu.read32(p + FLAGS) | NO_HITS;
            cpu.write32(p + FLAGS, flags);
            let n = cpu.read8(p + NUMBER);
            if (n as usize) < ENTRY_X.len() {
                let (x, drift) = ENTRY_X[n as usize];
                cpu.write32(p + POSITION, x);
                cpu.write32(p + BANK_X, drift);
            }
            cpu.write32(p + POSITION + 4, ENTRY_Y);
            cpu.write32(p + POSITION + 8, ENTRY_Z);
            cpu.write32(p + BANK_Y, ENTRY_BANK_Y);
            let z = cpu.read32(p + POSITION + 8) as i32;
            cpu.write32(p + CLIMB, (z.wrapping_neg() / ENTRY_FRAMES as i32) as u32);
            nested!(cpu, ENTRY_JET, jet_set(cpu, p, ENTRY_FRAMES));
            cpu.write16(p + ENTRY_LEFT, ENTRY_FRAMES as u16);
            cpu.write16(p + HURT, 0);
            cpu.write8(p + STATE_LOW, 1);
            set_phase(cpu, p, 1);
        }
        1 => {
            let up = cpu.read32(p + BANK_Y);
            let up = nested!(cpu, 0x0604_6168, super::actors::approach(cpu, up, 0x8_0000, 0x6000));
            cpu.write32(p + BANK_Y, up);
            for (k, field) in [BANK_X, BANK_Y, CLIMB].into_iter().enumerate() {
                let v = cpu.read32(p + POSITION + 4 * k as u32).wrapping_add(cpu.read32(p + field));
                cpu.write32(p + POSITION + 4 * k as u32, v);
            }
            let held = cpu.read16(p + PAD);
            if held & RIGHT != 0 {
                let x = cpu.read32(p + POSITION).wrapping_add(STRAIGHT);
                cpu.write32(p + POSITION, x);
            }
            if held & LEFT != 0 {
                let x = cpu.read32(p + POSITION).wrapping_add(neg(STRAIGHT));
                cpu.write32(p + POSITION, x);
            }
            let x = cpu.read32(p + POSITION);
            let x = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, x, SCREEN_X));
            cpu.write32(p + POSITION, x);
            let left = cpu.read16(p + ENTRY_LEFT).wrapping_sub(1);
            cpu.write16(p + ENTRY_LEFT, left);
            let left = cpu.read16(p + ENTRY_LEFT) as i16 as i32 as u32;
            let i = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, left, 32));
            let frame = cpu.read8(TILT_FRAMES.wrapping_add(i));
            cpu.write8(p + FRAME, frame);
            if cpu.read16(p + ENTRY_LEFT) as i16 <= 0 {
                cpu.write32(p + BANK_Y, 0);
                cpu.write32(p + BANK_X, 0);
                cpu.write32(p + POSITION + 8, 0);
                cpu.write16(p + HURT, AFTER_ENTRY);
                let flags = cpu.read32(p + FLAGS) & !NO_HITS;
                cpu.write32(p + FLAGS, flags);
                cpu.write8(p + TILT, 0);
                cpu.write16(p + STATE, NO_WEAPON as u16);
            }
        }
        _ => {}
    }
}

/// The jet a ship comes in on: a flame (list 2) kept 12 pixels behind the ship along its angle. On
/// odd frames it takes its next stage: it grows over six (a puff of smoke at the sixth), then
/// flickers among three frames, narrowing to half and lengthening, trailing smoke; on even frames
/// only its colour is 16 on. It ends with the ship, when the ship is not drawn, or after its frames.
const JET_SIZE: u32 = 0x48;
const JET_LIST: u32 = 2;
const JET_EXEC: u32 = 0x0606_21f4;
const JET_FOLLOW: u32 = 0x0606_216c;
const JET_KIND: u8 = 3;
const JET_BEHIND: u32 = 0xc_0000;
const JET_LIT: u16 = 0x10;
/// SGL's slSin, slCos and slMulFX.
const SL_SIN: u32 = 0x0606_9a0c;
const SL_COS: u32 = 0x0606_9a30;
const SL_MUL_FX: u32 = 0x0606_e26c;
/// The jet's fields: the ship, its stage, its frames left.
const JET_SHIP: u32 = 0x3c;
const JET_STAGE: u32 = 0x40;
const JET_LEFT: u32 = 0x44;
/// The sprite's fields (sprite_init's, from 0x10): its colour and own colour, kind, scales.
const SPRITE: u32 = 0x10;
const BEHAVIOUR: u32 = 0x38;
const COLOUR: u32 = 0x28;
const OWN_COLOUR: u32 = 0x2a;
const KIND: u32 = 0x2e;
const SCALES: u32 = 0x20;
/// Each stage's frame and scales (x, y); from stage 8 on the flicker goes round 8, 9, 10; past the
/// grown stage (7) the scales are its own, x narrowing to half.
const JET_STAGES: [(u8, u32, u32); 11] = [
    (0x2b, 0x4000, 0x4000), (0x2b, 0x8000, 0x4000), (0x2b, 0xc000, 0x4000), (0x2b, 0x1_0000, 0x4000),
    (0x2b, 0x1_8000, 0x4000), (0x2b, 0x2_0000, 0x4000), (0x2b, 0x2_0000, 0x2_0000), (0x27, 0x1_0000, 0x1_0000),
    (0x25, 0x1_0000, 0x1_0000), (0x26, 0x1_0000, 0x1_0000), (0x27, 0x1_0000, 0x1_0000),
];
const JET_LAST: i32 = 10;
const JET_FLICKER: i32 = 8;
const JET_GROWN: i32 = 7;
const JET_PUFFS: i32 = 6;
const JET_NARROWEST: i32 = 0x8000;
const JET_NARROWING: u32 = 0x400;
/// The puff at the sixth stage: 8 pixels below the jet, wide and flat.
const PUFF_BELOW: u32 = 0x8_0000;
const PUFF_SCALES: [u32; 2] = [0x1_8000, 0x2000];
const PUFF_SET: u32 = 0x0606_20c0;
/// The trailing smoke (actors.rs's drift_effect): 8 pixels a frame backward, in one of two
/// animations and colours by the frame count; it lies one below the ship's lift, or two on
/// every other pair of frames of the second.
const SMOKE_SPEED: u32 = 0xfff8_0000;
const SMOKE_SCALE: u32 = 0x1_0000;
const SMOKES: [(u32, u16); 2] = [(0x0607_3128, 0x8090), (0x0607_310c, 0x80d0)];

/// Makes the jet of ship `ship`, lasting `frames` frames, in place behind it. The jet, or 0 when
/// there is no room.
pub fn jet_set(cpu: &mut Cpu, ship: u32, frames: u32) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, JET_SIZE)) as i8 as i32 as u32;
    let j = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, JET_LIST, blocks, PLAIN_EXEC));
    if j == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, j + SPRITE));
    cpu.write8(j + NUMBER, 0);
    cpu.write32(j + BEHAVIOUR, JET_EXEC);
    let (frame, x, y) = JET_STAGES[0];
    cpu.write8(j + FRAME, frame);
    cpu.write32(j + SCALES, x);
    cpu.write32(j + SCALES + 4, y);
    cpu.write8(j + KIND, JET_KIND);
    let colour = cpu.read16(CHAR_BASE);
    cpu.write16(j + OWN_COLOUR, colour);
    cpu.write16(j + COLOUR, colour);
    let lift = cpu.read8(ship + LIFT);
    cpu.write8(j + LIFT, lift);
    cpu.write32(j + JET_STAGE, 0);
    cpu.write32(j + JET_SHIP, ship);
    cpu.write32(j + JET_LEFT, frames);
    nested!(cpu, JET_FOLLOW, jet_follow(cpu, j));
    j
}

/// Puts the jet behind its ship: 12 pixels back along the ship's angle (taken when not 0), level.
pub fn jet_follow(cpu: &mut Cpu, j: u32) {
    let ship = cpu.read32(j + JET_SHIP);
    let a = cpu.read16(ship + ANGLE);
    if a != 0 {
        cpu.write16(j + ANGLE, a);
        let a = a as i16 as i32 as u32;
        let sin = cpu.call(SL_SIN, &[a]);
        let back = cpu.call(SL_MUL_FX, &[JET_BEHIND, sin]);
        let x = cpu.read32(ship + POSITION).wrapping_sub(back);
        cpu.write32(j + POSITION, x);
        let cos = cpu.call(SL_COS, &[a]);
        let back = cpu.call(SL_MUL_FX, &[JET_BEHIND, cos]);
        let y = cpu.read32(ship + POSITION + 4).wrapping_add(back);
        cpu.write32(j + POSITION + 4, y);
    } else {
        let x = cpu.read32(ship + POSITION);
        cpu.write32(j + POSITION, x);
        let y = cpu.read32(ship + POSITION + 4).wrapping_add(JET_BEHIND);
        cpu.write32(j + POSITION + 4, y);
    }
    let z = cpu.read32(ship + POSITION + 8);
    cpu.write32(j + POSITION + 8, z);
}

/// The jet's behaviour.
pub fn jet_exec(cpu: &mut Cpu, j: u32) {
    let ship = cpu.read32(j + JET_SHIP);
    if cpu.read8(ship + OBJ_MODE) as i8 >= ENDING as i8 || cpu.read32(ship + FLAGS) & NO_DRAW != 0 {
        cpu.write8(j + OBJ_MODE, ENDING);
        return;
    }
    let left = cpu.read32(j + JET_LEFT).wrapping_sub(1);
    cpu.write32(j + JET_LEFT, left);
    if left == 0 {
        cpu.write8(j + OBJ_MODE, ENDING);
        return;
    }
    nested!(cpu, JET_FOLLOW, jet_follow(cpu, j));
    if cpu.read32(GAME_FRAMES) & 1 == 0 {
        let colour = cpu.read16(CHAR_BASE).wrapping_add(JET_LIT);
        cpu.write16(j + OWN_COLOUR, colour);
        cpu.write16(j + COLOUR, colour);
        return;
    }
    let mut stage = cpu.read32(j + JET_STAGE) as i32 + 1;
    if stage > JET_LAST {
        stage = JET_FLICKER;
    }
    cpu.write32(j + JET_STAGE, stage as u32);
    let (frame, x, y) = JET_STAGES[stage as usize];
    cpu.write8(j + FRAME, frame);
    if stage <= JET_GROWN {
        cpu.write32(j + SCALES, x);
        cpu.write32(j + SCALES + 4, y);
    } else if cpu.read32(j + SCALES) as i32 > JET_NARROWEST {
        let x = cpu.read32(j + SCALES).wrapping_sub(JET_NARROWING);
        cpu.write32(j + SCALES, x);
        let y = cpu.read32(j + SCALES + 4).wrapping_add(JET_NARROWING);
        cpu.write32(j + SCALES + 4, y);
    }
    if stage >= JET_PUFFS {
        let f = cpu.stack();
        let below = if stage == JET_PUFFS { PUFF_BELOW } else { 0 };
        for k in 0..3 {
            let v = cpu.read32(j + POSITION + 4 * k).wrapping_add(if k == 1 { below } else { 0 });
            cpu.write32(f + actors::E_POSITION + 4 * k, v);
        }
        let a = cpu.read16(j + ANGLE);
        cpu.write16(f + actors::E_ANGLE, a);
        if stage == JET_PUFFS {
            cpu.write32(f + actors::E_SCALES, PUFF_SCALES[0]);
            cpu.write32(f + actors::E_SCALES + 4, PUFF_SCALES[1]);
            nested!(cpu, PUFF_SET, puff_set(cpu, f));
        } else {
            cpu.write32(f + actors::E_SPEED, SMOKE_SPEED);
            cpu.write32(f + actors::E_SCALES, SMOKE_SCALE);
            cpu.write32(f + actors::E_SCALES + 4, SMOKE_SCALE);
            let second = cpu.read32(GAME_FRAMES) & 2 != 0;
            let (script, colour) = SMOKES[second as usize];
            cpu.write32(f + actors::E_SCRIPT, script);
            cpu.write16(f + actors::E_COLOUR, colour);
            let e = nested!(cpu, 0x0605_7454, actors::drift_effect(cpu, f));
            if e != 0 {
                let down = if !second && cpu.read32(GAME_FRAMES) & 4 != 0 { 2 } else { 1 };
                let lift = cpu.read8(e + LIFT).wrapping_sub(down);
                cpu.write8(e + LIFT, lift);
            }
        }
    }
    let colour = cpu.read16(CHAR_BASE);
    cpu.write16(j + OWN_COLOUR, colour);
    cpu.write16(j + COLOUR, colour);
}

/// The puff: frames 0xcb on (one every other frame) until 0xff, widening by an eighth a frame.
const PUFF_SIZE: u32 = 0x40;
const PUFF_EXEC: u32 = 0x0606_2074;
const PUFF_LIFT: u8 = 0x7f;
const PUFF_FRAME: u8 = 0xcb;
const PUFF_FRAMES: u32 = 0x0607_3958;
const PUFF_LAST: u8 = 0xff;
const PUFF_WIDENING: u32 = 0x2000;
/// The puff's field: how far along its frames it is.
const PUFF_AT: u32 = 0x3c;

/// Makes a puff from the description at `f` (actors.rs's E_ layout: position, scales, angle, the
/// puff facing the other way).
pub fn puff_set(cpu: &mut Cpu, f: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, PUFF_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, JET_LIST, blocks, PLAIN_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + SPRITE));
    cpu.write32(o + BEHAVIOUR, PUFF_EXEC);
    cpu.write8(o + NUMBER, 0);
    cpu.write8(o + LIFT, PUFF_LIFT);
    for k in 0..2 {
        let v = cpu.read32(f + actors::E_SCALES + 4 * k);
        cpu.write32(o + SCALES + 4 * k, v);
    }
    for k in 0..3 {
        let v = cpu.read32(f + actors::E_POSITION + 4 * k);
        cpu.write32(o + POSITION + 4 * k, v);
    }
    let a = cpu.read16(f + actors::E_ANGLE).wrapping_add(0x8000);
    cpu.write16(o + ANGLE, a);
    cpu.write8(o + FRAME, PUFF_FRAME);
    cpu.write8(o + KIND, 0);
    let colour = cpu.read16(CHAR_BASE);
    cpu.write16(o + OWN_COLOUR, colour);
    cpu.write16(o + COLOUR, colour);
    cpu.write32(o + PUFF_AT, 0);
}

/// The puff's behaviour.
pub fn puff_exec(cpu: &mut Cpu, o: u32) {
    let at = cpu.read32(o + PUFF_AT);
    let frame = cpu.read8(PUFF_FRAMES.wrapping_add(at));
    cpu.write8(o + FRAME, frame);
    if frame == PUFF_LAST {
        cpu.write8(o + OBJ_MODE, ENDING);
        return;
    }
    let x = cpu.read32(o + SCALES).wrapping_add(PUFF_WIDENING);
    cpu.write32(o + SCALES, x);
    if cpu.read32(GAME_FRAMES) & 1 != 0 {
        cpu.write32(o + PUFF_AT, at.wrapping_add(1));
    }
}

/// The ship's death: its explosion (effect_set's struct, an animation linked), its sound, the
/// death counted in its PLAY_DATA; 20 frames later the ship ends.
const DEATH_FRAMES: u32 = 0x6e;
const EXPLOSION: u32 = 0x0607_2bbc;
const SE_DEATH: u32 = 56;
const PD_DEATHS: u32 = 0x10;
const DEATH_WAIT: i16 = 19;
const OBJ_MODE: u32 = 3;
const ENDING: u8 = 0x70;

/// A ship dead (state 2).
pub fn ship_dead(cpu: &mut Cpu, p: u32) {
    match phase(cpu, p) as i8 {
        0 => {
            let f = cpu.stack();
            for k in 0..3 {
                let v = cpu.read32(p + POSITION + 4 * k);
                cpu.write32(f + 4 * k, v);
            }
            cpu.write32(f + 0x10, 0x8000);
            cpu.write32(f + 0x0c, 0x8000);
            cpu.write32(f + 0x14, 3);
            let a = cpu.read16(p + ANGLE);
            cpu.write16(f + 0x18, a);
            cpu.write16(f + 0x24, 0);
            for field in [0x26, 0x1c, 0x1d] {
                cpu.write8(f + field, 0);
            }
            let colour = cpu.read16(CHAR_BASE).wrapping_add(0x90);
            cpu.write16(f + 0x20, colour);
            let n = cpu.read8(p + NUMBER);
            cpu.write8(f + 0x1e, n);
            cpu.write32(f + 0x28, EXPLOSION);
            cpu.write32(f + 0x3c, 0);
            nested!(cpu, 0x0604_e1cc, effects::effect_set(cpu, f));
            nested!(cpu, 0x0600_50d4, sound::se_set(cpu, SE_DEATH));
            let at = play_data(cpu, p) + PD_DEATHS;
            let d = cpu.read8(at).wrapping_add(1);
            cpu.write8(at, d);
            cpu.write16(p + DEATH_FRAMES, 0);
            set_phase(cpu, p, 1);
        }
        1 => {
            let n = cpu.read16(p + DEATH_FRAMES).wrapping_add(1);
            cpu.write16(p + DEATH_FRAMES, n);
            if n as i16 > DEATH_WAIT {
                cpu.write16(p + STATE, NO_WEAPON as u16);
                cpu.write8(p + OBJ_MODE, ENDING);
            }
        }
        _ => {}
    }
}

/// The players' objects (the numbered objects' first two; 0 when out).
const PLAYER_OBJECTS: [u32; 2] = [0x0607_40f0, 0x0607_40f4];
/// The ship's pad words (held, changed, pressed, let go: pads.rs's PAD_INF as the ship keeps it).
const SHIP_PAD: u32 = 0xa0;

/// The ships' pad words cleared (for the stages' scenes: the ships take no input).
pub fn ships_still(cpu: &mut Cpu) {
    for at in PLAYER_OBJECTS {
        let p = cpu.read32(at);
        if p != 0 {
            for k in 0..4 {
                cpu.write16(p + SHIP_PAD + 2 * k, 0);
            }
        }
    }
}

/// The ships hidden (not drawn).
pub fn ships_hidden(cpu: &mut Cpu) {
    for at in PLAYER_OBJECTS {
        let p = cpu.read32(at);
        if p != 0 {
            let flags = cpu.read32(p + FLAGS) | NO_DRAW;
            cpu.write32(p + FLAGS, flags);
        }
    }
}

/// A scene's flags (states.rs's: the pads not taken, and the two display flags) set, or cleared.
const SCENE_FLAGS: [u32; 3] = [0x0600_ed90, 0x0600_cc61, 0x0600_edcc];

pub fn scene_on(cpu: &mut Cpu) {
    for at in SCENE_FLAGS {
        cpu.write8(at, 1);
    }
}

pub fn scene_off(cpu: &mut Cpu) {
    for at in SCENE_FLAGS {
        cpu.write8(at, 0);
    }
}

/// Start pressed by a player who is in (pads.rs's PAD_INF pressed words; START), before the demo
/// modes: the bit, or 0.
const PRESSED: [u32; 2] = [0x0602_ee14, 0x0602_ee1c];
const START: u16 = 0x800;
const BEFORE_DEMOS: i8 = 12;

pub fn start_pressed(cpu: &mut Cpu) -> u32 {
    let mut pressed = 0;
    if (cpu.read8(super::globals::GAME_MODE) as i8) < BEFORE_DEMOS {
        for (at, pad) in PLAYER_OBJECTS.into_iter().zip(PRESSED) {
            if cpu.read32(at) != 0 {
                pressed |= cpu.read16(pad) & START;
            }
        }
    }
    pressed as u32
}

/// An aura's fields: its scales (x, y), its angle, its step (a byte), its spin (a word), its
/// growth, its link (effects.rs's); it follows its ship (AURA_OF) and ends with it.
const AU_SCALES: u32 = 0x20;
const AU_ANGLE: u32 = 0x30;
const AU_STEP: u32 = 0x3c;
const AU_SPIN: u32 = 0x42;
const AU_GROWTH: u32 = 0x54;
const AU_LINK: u32 = 0x70;
const SHIP_LIVES: i8 = 0x6f;

/// The aura follows its ship (ending with it): its place, its step.
fn aura_follow(cpu: &mut Cpu, o: u32) -> u8 {
    let ship = cpu.read32(o + AURA_OF);
    if cpu.read8(ship + OBJ_MODE) as i8 > SHIP_LIVES {
        cpu.write8(o + OBJ_MODE, ENDING);
    }
    for k in 0..3 {
        let v = cpu.read32(ship + POSITION + 4 * k);
        cpu.write32(o + POSITION + 4 * k, v);
    }
    cpu.read8(o + AU_STEP)
}

/// The hyper sword's readiness flash: it grows a 4th of one a frame (from a 16th) past one,
/// then slows to shrinking a 4th a frame, spinning ever slower; it ends once it would vanish.
const AU_START: u32 = 0x1000;
const AU_GROW: u32 = 0x4000;
const AU_SHRINK: u32 = 0xffff_c000;
const AU_GROWTH_STEP: u32 = 0x800;
const AU_SPIN_START: u16 = 0x800;
const AU_SPIN_END: u32 = 16;
const AU_SPIN_STEP: u32 = 32;

pub fn aura_exec(cpu: &mut Cpu, o: u32) {
    match aura_follow(cpu, o) as i8 {
        0 => {
            cpu.write32(o + AU_SCALES, AU_START);
            cpu.write32(o + AU_GROWTH, AU_GROW);
            cpu.write16(o + AU_SPIN, AU_SPIN_START);
            cpu.write8(o + AU_STEP, 1);
        }
        1 => {
            if cpu.read32(o + AU_SCALES) as i32 > 0xffff {
                cpu.write8(o + AU_STEP, 2);
            }
        }
        2 => {
            let g = cpu.read32(o + AU_GROWTH);
            let g = nested!(cpu, 0x0604_6168, super::actors::approach(cpu, g, AU_SHRINK, AU_GROWTH_STEP));
            cpu.write32(o + AU_GROWTH, g);
            let spin = cpu.read16(o + AU_SPIN) as i16 as i32 as u32;
            let spin = nested!(cpu, 0x0604_6168, super::actors::approach(cpu, spin, AU_SPIN_END, AU_SPIN_STEP));
            cpu.write16(o + AU_SPIN, spin as u16);
            let left = cpu.read32(o + AU_SCALES).wrapping_add(cpu.read32(o + AU_GROWTH));
            if left as i32 <= 0 {
                cpu.write8(o + OBJ_MODE, ENDING);
                cpu.write8(o + AU_STEP, 3);
            }
        }
        _ => {}
    }
    let a = cpu.read16(o + AU_ANGLE).wrapping_add(cpu.read16(o + AU_SPIN));
    cpu.write16(o + AU_ANGLE, a);
    let s = cpu.read32(o + AU_SCALES).wrapping_add(cpu.read32(o + AU_GROWTH));
    cpu.write32(o + AU_SCALES, s);
    cpu.write32(o + AU_SCALES + 4, s);
}

/// The hyper sword's aura: it grows an 8th of the way to one a frame (by at most a 16th), until
/// the ship is hurt again or has 10 or fewer frames of being hurt left; then it shrinks away by a
/// 16th a frame. Linked to its effect (the link at AU_LINK, from a table).
const HYPER_AURA_LINK: u32 = 0x0607_2760;
const HA_MOST: u32 = 0x1000;
const HA_LAST_HURT: i16 = 10;

pub fn hyper_aura_exec(cpu: &mut Cpu, o: u32) {
    match aura_follow(cpu, o) as i8 {
        0 => {
            nested!(cpu, 0x0604_6530, effects::link_init(cpu, o + AU_LINK, HYPER_AURA_LINK));
            cpu.write16(o + AU_ANGLE, 0);
            cpu.write32(o + AU_SCALES, AU_START);
            cpu.write8(o + AU_STEP, 1);
        }
        1 => {
            let s = cpu.read32(o + AU_SCALES) as i32;
            let d = 0x1_0000i32.wrapping_sub(s) / 8;
            let d = nested!(cpu, 0x0606_4f3c, super::math::clamp(cpu, d as u32, HA_MOST));
            let s = cpu.read32(o + AU_SCALES).wrapping_add(d);
            cpu.write32(o + AU_SCALES, s);
            let ship = cpu.read32(o + AURA_OF);
            if cpu.read32(ship + FLAGS) & UNHURT == 0 && cpu.read16(ship + HURT) as i16 <= HA_LAST_HURT {
                cpu.write8(o + AU_STEP, 2);
            }
        }
        2 => {
            let s = cpu.read32(o + AU_SCALES);
            let s = nested!(cpu, 0x0604_6168, super::actors::approach(cpu, s, 0, HA_MOST));
            cpu.write32(o + AU_SCALES, s);
            if s == 0 {
                cpu.write8(o + OBJ_MODE, ENDING);
                cpu.write8(o + AU_STEP, 3);
            }
        }
        _ => {}
    }
    let s = cpu.read32(o + AU_SCALES);
    cpu.write32(o + AU_SCALES + 4, s);
    nested!(cpu, 0x0604_6558, effects::link_step(cpu, o + 0x10, o + AU_LINK));
}
