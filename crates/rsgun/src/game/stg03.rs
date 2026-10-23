//! STG03.BIN's code (fingerprint fd6475daa2eebeb4), which game mode 9 runs in the overlay window:
//! stage 4, taken at STG01's end instead of stage 2 (the stage 4 route's evidence,
//! out/evidence/route4*).

use super::globals::{bios_setting_is_1, GAME_FRAMES, GAME_MODE};
use super::{actors, aim, bg, camera, ground, hud, parts, sgl, sprites, dma, effects, explosions, files, hits, kinds, math, models, nested, objects, screen, sound, states, stg02, timeline, Cpu};

// ---- the stage's mode ----------------------------------------------------------------------------

/// The director's slots the mode uses (as STG02's mode uses them): its step, the load (a local
/// written back each time), the settings kept (the buffer +0x6c, the room's top +0x70, VDP1's next
/// character place +0x74, its next character and colour +0x78 and +0x7c), the second file's size
/// (+0x80), the demo's input (+0x54), the mode's music (+0xa4).
const MODE_STEP: u32 = 0x64;
const LOAD: u32 = 0x68;
const KEPT: u32 = 0x6c;
const PLACE: u32 = 0x70;
const VDP1_PLACE: u32 = 0x74;
const NEXT_CHAR: u32 = 0x78;
const NEXT_COLOUR: u32 = 0x7c;
const SIZE: u32 = 0x80;
const DEMO_INPUT: u32 = 0x54;
const MUSIC: u32 = 0xa4;
const ROOM: u32 = 0x4_4040;
const ROOM_TOP: u32 = 0x0607_4050;
const NEXT_RATE: u32 = 0x0603_2e40;
const STAGE_RATE: u32 = 0x2_0000;
/// The stages' screen data files: this stage's two (the second and the seventh entries of the
/// table).
const SCREEN_FILE_1: u32 = 0x0607_35b0 + 8;
const SCREEN_FILE_2: u32 = 0x0607_35cc + 8;
/// Its screen's room in the stage table (states::stage_table's 2).
const SCREEN_ROOM: u32 = 2;
/// The sound data files loaded at once (sound::load_sound_file), the defaults' set-up after the
/// first, and the music set after the third.
const SOUND_FILES: [u32; 5] = [1, 7, 6, 0x26, 0x25];
const STAGE_MUSIC: u16 = 2;
/// The kinds put in place (kinds.rs): three model kinds, then two sprite kinds that take kind
/// 12's first colour.
const MODEL_KINDS: [u32; 3] = [0x20, 0x3f, 0x32];
const SPRITE_KINDS: [u32; 2] = [0x1b, 0x1c];
const KINDS: u32 = 0x0607_41f0;
const K_SIZE: u32 = 16;
const K_COLOUR: u32 = 0xc;
const COLOUR_KIND: u32 = 12;
/// The stage's scroll table, its timeline, its stage sound effects; the demo's input files
/// (by the BIOS's setting) and the modes after it. The demo of this stage is game mode 14.
const SCROLLS: u32 = 0x060b_098c;
const EVENTS: u32 = 0x060a_d764;
const SOUND_EFFECTS: u32 = 0x060b_1768;
const STAGE_SE_TABLE: u32 = 0x0600_edd4;
const NEXT_MODE: u32 = 0x0600_ed70;
const DEMO: u8 = 14;
const DEMO_FILES: [u32; 2] = [0x0600_8c3c, 0x0600_8c0c];
const AFTER_DEMO: u8 = 0;
const AFTER_PLAY: u8 = 10;

fn kind(k: u32) -> u32 {
    KINDS + K_SIZE * k
}

/// The mode's code (the director calls it each frame until it gives 1): the settings kept, the
/// sound data loaded, the kinds read into the buffer and put in place (each past the last), the
/// first screen data file loaded into the buffer; once DMA is idle the second, below the screen's
/// room off the place, and the screen started; once in, the stage data set from it; a frame; the
/// timeline made (in the demo, its input read after the buffer), the settings given back; 1.
pub fn stg03_mode(cpu: &mut Cpu, game: u32) -> u32 {
    let mut load = cpu.read32(game + LOAD);
    let mut done = 0;
    match cpu.read8(game + MODE_STEP) {
        0 => {
            states::keep_mode_settings(cpu, game, ROOM);
            cpu.write8(game + MODE_STEP, 1);
        }
        1 => {
            for (k, n) in SOUND_FILES.into_iter().enumerate() {
                let buffer = cpu.read32(game + KEPT);
                nested!(cpu, 0x0600_4b18, sound::load_sound_file(cpu, n, buffer));
                if k == 0 {
                    nested!(cpu, 0x0600_49cc, sound::sound_setup_default(cpu));
                }
                if k == 2 {
                    cpu.write16(game + MUSIC, STAGE_MUSIC);
                }
            }
            let top = cpu.read32(ROOM_TOP);
            cpu.write32(game + PLACE, top);
            cpu.write8(game + MODE_STEP, 9);
        }
        9 => {
            install_kinds(cpu, game);
            let name = cpu.read32(SCREEN_FILE_1);
            load = nested!(cpu, 0x0600_5e40, files::load_file(cpu, name));
            if load != 0 {
                cpu.write32(NEXT_RATE, STAGE_RATE);
                let buffer = cpu.read32(game + KEPT);
                nested!(cpu, 0x0600_603c, files::start_load(cpu, load, buffer));
                cpu.write8(game + MODE_STEP, 10);
            }
        }
        10 => {
            if nested!(cpu, 0x0600_6074, files::load_done(cpu, load)) != 0 {
                cpu.write8(game + MODE_STEP, 11);
            }
        }
        11 => {
            if nested!(cpu, 0x0600_46b8, dma::dma_check(cpu)) == 0 {
                let name = cpu.read32(SCREEN_FILE_2);
                load = nested!(cpu, 0x0600_5e40, files::load_file(cpu, name));
                if load != 0 {
                    let room = nested!(cpu, 0x0605_fe78, states::stage_table(cpu, SCREEN_ROOM));
                    let screen_room = cpu.read32(game + PLACE).wrapping_sub(room);
                    cpu.write32(game + PLACE, screen_room);
                    let size = nested!(cpu, 0x0600_5edc, files::load_size(cpu, load));
                    cpu.write32(game + SIZE, size);
                    let place = cpu.read32(game + PLACE).wrapping_sub(size);
                    cpu.write32(game + PLACE, place);
                    cpu.write32(NEXT_RATE, STAGE_RATE);
                    nested!(cpu, 0x0600_603c, files::start_load(cpu, load, place));
                    let buffer = cpu.read32(game + KEPT);
                    nested!(cpu, 0x0605_e3e0, screen::screen_start(cpu, SCROLLS, buffer, screen_room));
                    cpu.write8(game + MODE_STEP, 12);
                }
            }
        }
        12 => {
            if nested!(cpu, 0x0600_6074, files::load_done(cpu, load)) != 0 {
                let place = cpu.read32(game + PLACE);
                nested!(cpu, 0x0605_e17c, states::stage_data_set(cpu, place));
                cpu.write8(game + MODE_STEP, 13);
            }
        }
        13 => cpu.write8(game + MODE_STEP, 14),
        14 => {
            nested!(cpu, 0x0605_7354, timeline::timeline_set(cpu, EVENTS));
            cpu.write32(STAGE_SE_TABLE, SOUND_EFFECTS);
            if cpu.read8(GAME_MODE) == DEMO {
                let buffer = cpu.read32(game + KEPT);
                cpu.write32(game + DEMO_INPUT, buffer);
                let name = DEMO_FILES[bios_setting_is_1(cpu) as usize];
                let buffer = cpu.read32(game + KEPT);
                let size = nested!(cpu, 0x0600_5f10, files::read_file(cpu, name, buffer));
                let past = cpu.read32(game + KEPT).wrapping_add(size);
                cpu.write32(game + KEPT, past);
                cpu.write8(NEXT_MODE, AFTER_DEMO);
            } else {
                cpu.write32(game + DEMO_INPUT, 0);
                cpu.write8(NEXT_MODE, AFTER_PLAY);
            }
            states::give_back_mode_settings(cpu, game);
            cpu.write8(game + MODE_STEP, 15);
        }
        15 => done = 1,
        _ => {}
    }
    cpu.write32(game + LOAD, load);
    done
}

/// Step 9's kinds: each read into the buffer and put in place in VDP1's VRAM (at the place the
/// last left), the next character on by each kind's count (+6), the next colour by the last two
/// model kinds' (+0x16); the sprite kinds take kind 12's first colour; the buffer moved past each
/// (its file's first long).
fn install_kinds(cpu: &mut Cpu, game: u32) {
    for (i, k) in MODEL_KINDS.into_iter().enumerate() {
        let buffer = cpu.read32(game + KEPT);
        nested!(cpu, 0x0604_58cc, kinds::model_kind_read(cpu, k, buffer));
        let (first, colour) = next_char_colour(cpu, game);
        let vram = cpu.read32(game + VDP1_PLACE);
        let next = nested!(cpu, 0x0604_5950, kinds::model_kind_install(cpu, k, vram, first, colour));
        cpu.write32(game + VDP1_PLACE, next);
        advance(cpu, game, k, i >= 1);
        let file = cpu.read32(kind(k));
        let past = cpu.read32(file);
        cpu.write32(game + KEPT, past);
    }
    for k in SPRITE_KINDS {
        let buffer = cpu.read32(game + KEPT);
        nested!(cpu, 0x0604_5688, kinds::sprite_kind_read(cpu, k, buffer));
        let (first, colour) = next_char_colour(cpu, game);
        let vram = cpu.read32(game + VDP1_PLACE);
        let next = nested!(cpu, 0x0604_570c, kinds::sprite_kind_install(cpu, k, vram, first, colour));
        cpu.write32(game + VDP1_PLACE, next);
        advance(cpu, game, k, false);
        let c = cpu.read16(kind(COLOUR_KIND) + K_COLOUR);
        cpu.write16(kind(k) + K_COLOUR, c);
        let file = cpu.read32(kind(k));
        let past = cpu.read32(file);
        cpu.write32(game + KEPT, past);
    }
}

fn next_char_colour(cpu: &mut Cpu, game: u32) -> (u32, u32) {
    (cpu.read16(game + NEXT_CHAR) as i16 as i32 as u32, cpu.read16(game + NEXT_COLOUR) as u32)
}

/// The next character on by kind `k`'s count (its file's +6 word); with `colour_too` the next
/// colour by its +0x16 word too.
fn advance(cpu: &mut Cpu, game: u32, k: u32, colour_too: bool) {
    let file = cpu.read32(kind(k));
    let next = cpu.read16(game + NEXT_CHAR).wrapping_add(cpu.read16(file + 6));
    cpu.write16(game + NEXT_CHAR, next);
    if colour_too {
        let colour = cpu.read16(game + NEXT_COLOUR).wrapping_add(cpu.read16(file + 0x16));
        cpu.write16(game + NEXT_COLOUR, colour);
    }
}

// ---- the gun's shots and sparks ------------------------------------------------------------------

/// An object's fields (the same in these shots and sparks): its mode (+3: 112 done), its flags
/// (+0x10), its position (+0x14: x, y, z), its scales (+0x20, +0x24), the colour (+0x28 and +0x2a),
/// its angle (+0x30), its number (+0x32), its script (+0x3c), the step (+0x6c: a byte of its own,
/// the next the state), its counters (+0x70.., +0x74, +0x78).
const O_MODE: u32 = 3;
const O_FLAGS: u32 = 0x10;
const O_POSITION: u32 = 0x14;
const O_SCALE: u32 = 0x20;
const O_SCALE_2: u32 = 0x24;
const O_COLOUR_A: u32 = 0x28;
const O_COLOUR_B: u32 = 0x2a;
const O_ANGLE: u32 = 0x30;
const O_NUMBER: u32 = 0x32;
const O_BEHAVIOUR: u32 = 0x38;
const O_SCRIPT: u32 = 0x3c;
const O_STEP: u32 = 0x6c;
const O_STATE: u32 = 0x6d;
const O_70: u32 = 0x70;
const O_74: u32 = 0x74;
const O_78: u32 = 0x78;
const O_ARMS: u32 = 0x56;
/// The first colour of kind 0 (every stage's sprites' base).
const CHAR_BASE: u32 = 0x0607_41fc;
/// An object's mode that ends it.
const MODE_DONE: u8 = 0x70;
/// An object's flags: the sprite's.
const F_NOT_CULLED: u32 = 0x2000_0400;
const F_HIDDEN: u32 = 0x8_0000;

/// The shot made by `muzzle_shot`: it flies in its angle, its speed 0x18000 at most; on the ground
/// (or with +0x56's bit 0x400 set, a hit) it makes a spark effect (its angle turned round, its
/// object's own speeds a quarter) and ends; off the screen it ends.
const SHOT_SPEED: u32 = 0x1_8000;
const SHOT_SPEED_FIRST: u32 = 0x1000;
const SHOT_SPEED_STEP: u32 = 0x2000;
const SHOT_SCALE_END: u32 = 0x1_0000;
const SHOT_SCALE_STEP: u32 = 0x1000;
const SHOT_SPEED_AT: u32 = 0x70;
const SHOT_HIT: u16 = 0x400;
const SHOT_SPARK_SCALE: u32 = 0xc000;
const SHOT_SPARK_FLAGS: u32 = 11;
/// A spark's description (effects.rs's struct)'s script, the shot's: SPARK_LINK.
const SHOT_SPARK_LINK: u32 = 0x0607_2c14;
const SHOT_LIMIT_X: i32 = 0xbf_ffff;
const SHOT_LIMIT_Y: i32 = 0x7f_ffff;
const SHOT_SPEEDS: [u32; 2] = [0x5c, 0x60];
const SPARK_SPEEDS: [u32; 2] = [0x44, 0x48];

pub fn shot_step(cpu: &mut Cpu, o: u32) {
    use effects as fx;
    let mut hit = cpu.read16(o + O_ARMS) & SHOT_HIT != 0;
    if !hit {
        let (x, y) = (cpu.read32(o + O_POSITION), cpu.read32(o + O_POSITION + 4));
        hit = nested!(cpu, 0x0605_ef90, bg::map_hit(cpu, x, y)) != 0;
    }
    if hit {
        let e = cpu.stack();
        for k in 0..3 {
            let v = cpu.read32(o + O_POSITION + 4 * k);
            cpu.write32(e + fx::X + 4 * k, v);
        }
        cpu.write32(e + fx::SCALE_X, SHOT_SPARK_SCALE);
        cpu.write32(e + fx::SCALE_Y, SHOT_SPARK_SCALE);
        cpu.write32(e + fx::FLAGS_LONG, SHOT_SPARK_FLAGS);
        let angle = cpu.read16(o + O_ANGLE).wrapping_add(0x8000);
        cpu.write16(e + fx::ANGLE, angle);
        cpu.write16(e + fx::FRAMES, 0);
        cpu.write8(e + fx::FRAMES_2, 0);
        cpu.write8(e + fx::KIND, 0);
        let colour = cpu.read16(o + O_COLOUR_B);
        cpu.write16(e + fx::COLOUR, colour);
        cpu.write8(e + fx::FRAME, colour as u8);
        let player = cpu.read8(o + O_NUMBER);
        cpu.write8(e + fx::PLAYER, player);
        cpu.write32(e + fx::LINK, SHOT_SPARK_LINK);
        cpu.write32(e + fx::RELATIVE_3C, 0);
        let spark = nested!(cpu, 0x0604_e1cc, effects::effect_set(cpu, e));
        if spark != 0 {
            for (from, to) in SHOT_SPEEDS.into_iter().zip(SPARK_SPEEDS) {
                let v = cpu.read32(o + from) as i32;
                cpu.write32(spark + to, (v / 4) as u32);
            }
        }
        return;
    }
    match cpu.read8(o + O_STATE) {
        0 => {
            cpu.write32(o + SHOT_SPEED_AT, SHOT_SPEED_FIRST);
            cpu.write16(o + O_STEP, 1);
            shot_speed_up(cpu, o);
        }
        1 => shot_speed_up(cpu, o),
        _ => {}
    }
    let base = cpu.read16(CHAR_BASE);
    let colour = base.wrapping_add(if cpu.read32(GAME_FRAMES) >> 1 & 1 == 0 { 0x30 } else { 0x20 });
    cpu.write16(o + O_COLOUR_B, colour);
    cpu.write16(o + O_COLOUR_A, colour);
    let speed = cpu.read32(o + SHOT_SPEED_AT);
    let angle = cpu.read16(o + O_ANGLE) as i16 as i32 as u32;
    nested!(cpu, 0x0604_d8d8, actors::move_by_angle(cpu, o, speed, angle));
    let x = (cpu.read32(o + O_POSITION) as i32).wrapping_abs();
    let y = (cpu.read32(o + O_POSITION + 4) as i32).wrapping_abs();
    if x > SHOT_LIMIT_X || y > SHOT_LIMIT_Y {
        cpu.write8(o + O_MODE, MODE_DONE);
    }
}

/// The shot's speed brought up to 0x18000 by 0x2000, its second scale to 0x10000 by 0x1000, and
/// its first scale given a random 0xc000-0x10000.
fn shot_speed_up(cpu: &mut Cpu, o: u32) {
    let speed = cpu.read32(o + SHOT_SPEED_AT);
    let speed = nested!(cpu, 0x0604_6168, actors::approach(cpu, speed, SHOT_SPEED, SHOT_SPEED_STEP));
    cpu.write32(o + SHOT_SPEED_AT, speed);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x4000));
    cpu.write32(o + O_SCALE, r.wrapping_add(0xc000));
    let s = cpu.read32(o + O_SCALE_2);
    let s = nested!(cpu, 0x0604_6168, actors::approach(cpu, s, SHOT_SCALE_END, SHOT_SCALE_STEP));
    cpu.write32(o + O_SCALE_2, s);
}

/// A shot made at the muzzle of the object `gun` (its screen point of the point (0, -0x120000, 0)
/// in its model's frame), turned `turn` from the gun's own angle (its model_screen_angle): an
/// object of list 2 (0xb4 bytes) with the shot's behaviour, hidden by the mode 8 (+0x33), its
/// scale 0x1000, its script and colour kind 0 / 200, the gun's number and the byte at +0x54.
const SHOT_SIZE: u32 = 0xb4;
const SHOT_EXEC: u32 = 0x0604_d780;
const SHOT_BEHAVIOUR: u32 = 0x0608_0890;
const SHOT_SCRIPT: u32 = 0x060a_bbb4;
const SHOT_BEHIND: u32 = 0xffee_0000;
const O_33: u32 = 0x33;
const O_KIND: u32 = 0x2e;
const O_FRAME: u32 = 0x2f;
const O_54: u32 = 0x54;
const O_55: u32 = 0x55;

pub fn muzzle_shot(cpu: &mut Cpu, gun: u32, turn: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, SHOT_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, SHOT_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_62ac, actors::actor_setup(cpu, o));
    let flags = cpu.read32(o + O_FLAGS) | F_NOT_CULLED;
    cpu.write32(o + O_FLAGS, flags);
    cpu.write32(o + O_BEHAVIOUR, SHOT_BEHAVIOUR);
    cpu.write8(o + O_33, 8);
    cpu.write32(o + O_POSITION, 0);
    cpu.write32(o + O_POSITION + 4, SHOT_BEHIND);
    cpu.write32(o + O_POSITION + 8, 0);
    nested!(cpu, 0x0605_2ca0, models::model_screen_point(cpu, gun, o + O_POSITION));
    cpu.write32(o + O_SCALE_2, 0x1000);
    let angle = nested!(cpu, 0x0608_0860, stg02::model_screen_angle(cpu, gun));
    cpu.write16(o + O_ANGLE, (turn as u16).wrapping_add(angle as u16));
    let number = cpu.read8(gun + O_NUMBER);
    cpu.write8(o + O_NUMBER, number);
    cpu.write8(o + O_KIND, 0);
    cpu.write8(o + O_FRAME, 200);
    cpu.write32(o + O_SCRIPT, SHOT_SCRIPT);
    let b = cpu.read8(gun + O_54);
    cpu.write8(o + O_54, b);
    cpu.write8(o + O_55, 4);
}

/// The gun's spark (a debris object): a state machine on +0x6d. 0 sets it up (colour kind 0 / 0x90
/// of the sprites' base, script, scale 0x6000, a link with the sprite's flags, flipped half the
/// time, its speed 0x12000 up to 0x14000 by a random, its heading its angle plus a random and its
/// angle random); 1 flickers (every other frame hidden by +0x6e's parity), slows, counts down,
/// then 2 slows faster; each step it moves, plays its link, and ends when the link has ended.
const SPARK_SCRIPT: u32 = 0x060a_bbbc;
const SPARK_LINK_SCRIPT: u32 = 0x060a_bbc4;
const SPARK_COLOUR_ADD: u16 = 0x90;
const SPARK_SCALE: u32 = 0x6000;
const SPARK_LINK: u32 = 0x7c;
const SPARK_LINK_FLAGS: u32 = SPARK_LINK + 12;
const SPARK_LINK_FLIP: u32 = SPARK_LINK + 13;
const SPARK_LINK_SPEED: u32 = SPARK_LINK + 8;
const SPARK_PARITY: u32 = 0x6e;
const SPARK_SPEED: u32 = O_74;
const SPARK_HEADING: u32 = O_78;
const SPARK_LIFE: u32 = O_70;

pub fn spark_step(cpu: &mut Cpu, o: u32) {
    match cpu.read8(o + O_STATE) as i8 {
        0 => {
            cpu.write8(o + O_KIND, 0);
            let colour = cpu.read16(CHAR_BASE).wrapping_add(SPARK_COLOUR_ADD);
            cpu.write16(o + O_COLOUR_B, colour);
            cpu.write16(o + O_COLOUR_A, colour);
            cpu.write32(o + O_SCRIPT, SPARK_SCRIPT);
            cpu.write32(o + O_SCALE, SPARK_SCALE);
            nested!(cpu, 0x0604_6530, effects::link_init(cpu, o + SPARK_LINK, SPARK_LINK_SCRIPT));
            let f = cpu.read8(o + SPARK_LINK_FLAGS) | 0xc0;
            cpu.write8(o + SPARK_LINK_FLAGS, f);
            let flip = nested!(cpu, 0x0604_5d90, math::random(cpu, 0)) as u8 & 1;
            cpu.write8(o + SPARK_LINK_FLIP, flip);
            cpu.write32(o + SPARK_LINK_SPEED, if flip as i8 == 0 { 0x6000 } else { 0x4000 });
            let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x2000));
            cpu.write32(o + SPARK_SPEED, r.wrapping_add(0x1_2000));
            let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x100));
            let heading = r.wrapping_add(cpu.read16(o + O_ANGLE) as u32);
            cpu.write16(o + SPARK_HEADING, heading as u16);
            let angle = nested!(cpu, 0x0604_5d90, math::random(cpu, 0));
            cpu.write16(o + O_ANGLE, angle as u16);
            cpu.write32(o + SPARK_LIFE, 60);
            cpu.write16(o + O_STEP, 1);
        }
        1 => {
            let parity = cpu.read16(o + SPARK_PARITY).wrapping_add(cpu.read32(GAME_FRAMES) as u16) as u32;
            let flags = cpu.read32(o + O_FLAGS);
            cpu.write32(o + O_FLAGS, if parity & 1 == 0 { flags & !F_HIDDEN } else { flags | F_HIDDEN });
            let speed = cpu.read32(o + SPARK_SPEED);
            let speed = nested!(cpu, 0x0604_6168, actors::approach(cpu, speed, 0, 0x100));
            cpu.write32(o + SPARK_SPEED, speed);
            let life = cpu.read32(o + SPARK_LIFE).wrapping_sub(1);
            cpu.write32(o + SPARK_LIFE, life);
            if life as i32 <= 0 {
                let flags = cpu.read32(o + O_FLAGS) | F_HIDDEN;
                cpu.write32(o + O_FLAGS, flags);
                cpu.write16(o + O_STEP, 2);
            }
        }
        2 => {
            let speed = cpu.read32(o + SPARK_SPEED);
            let speed = nested!(cpu, 0x0604_6168, actors::approach(cpu, speed, 0, 0x400));
            cpu.write32(o + SPARK_SPEED, speed);
        }
        _ => {}
    }
    let speed = cpu.read32(o + SPARK_SPEED);
    let heading = cpu.read16(o + SPARK_HEADING) as i16 as i32 as u32;
    nested!(cpu, 0x0604_d8d8, actors::move_by_angle(cpu, o, speed, heading));
    nested!(cpu, 0x0604_6558, effects::link_step(cpu, o + O_FLAGS, o + SPARK_LINK));
    let scale = cpu.read32(o + O_SCALE);
    cpu.write32(o + O_SCALE_2, scale);
    if cpu.read8(o + SPARK_LINK_FLAGS) & 1 != 0 {
        cpu.write8(o + O_MODE, MODE_DONE);
    }
}

/// A model colour of kind 0x48 (frame `frame`): its kind's colour bank moved on by 8, 12 or 16 by
/// `side` (0, 1, 2; 4 for any other), a word.
const SIDE_KIND: u32 = 0x48;

pub fn kind_side_colour(cpu: &mut Cpu, frame: u32, side: u32) {
    let (frame, side) = (frame as i8 as i32, side as i8 as i32);
    let bank = nested!(cpu, 0x0604_5af8, kinds::kind_colour_bank(cpu, SIDE_KIND)) as u16;
    let add = match side {
        0 => 8,
        1 => 12,
        2 => 16,
        _ => 4,
    };
    nested!(cpu, 0x0604_5b1c, models::model_colour(cpu, SIDE_KIND, frame as u32, bank.wrapping_add(add) as u32));
}

// ---- the stage boss's parts ----------------------------------------------------------------------

/// The numbered objects (a long each, by their number +0x32).
const NUMBERED: u32 = 0x0607_40f0;
/// A boss part's fields: its state (+0x6d; the byte before it the step, +0x6c), which of its
/// parent's parts it is (+0x6e) and the parent's group (+0x6f), its part (+0xb4: the next one it
/// made), its parent (+0xd4), its numbers' share (+0xb8: the parent's parts by +0x6e), the
/// models' pair (+0x44), the weapon's power (+0x50), the byte pair at +0x54/+0x55.
const P_PART: u32 = 0xb4;
const P_PARTS: u32 = 0xb8;
const P_WHICH: u32 = 0x6e;
const P_GROUP: u32 = 0x6f;
const P_PARENT: u32 = 0xd4;
const P_MODEL: u32 = 0x44;
const P_POWER: u32 = 0x50;
const P_54: u32 = 0x54;
const P_55: u32 = 0x55;
/// A state past which a part is gone (99 is the one it is given to be taken down, 100 and up
/// ones that follow).
const GONE: i8 = 99;

/// The part of `o`'s numbered parent given the step 17, or, if that is gone, `o`'s own part (if it
/// has one) the step 99.
pub fn part_signal(cpu: &mut Cpu, o: u32) {
    let group = cpu.read8(o + P_GROUP) as i8 as i32 as u32;
    let list = cpu.read32(NUMBERED.wrapping_add(group << 2));
    let which = cpu.read8(o + P_WHICH) as i8 as i32 as u32;
    let target = cpu.read32(list.wrapping_add(which << 2).wrapping_add(P_PARTS));
    if cpu.read8(target + O_STATE) as i8 > GONE {
        let part = cpu.read32(o + P_PART);
        if part == 0 {
            return;
        }
        cpu.write16(part + O_STEP, GONE as u16);
    } else {
        cpu.write16(target + O_STEP, 17);
    }
}

/// Whether `o` has no part.
pub fn part_none(cpu: &mut Cpu, o: u32) -> u32 {
    (cpu.read32(o + P_PART) == 0) as u32
}

/// The boss part's object (list 2, 0x178 bytes, after `parent` in it): its flags 0x80c00000 on, its
/// behaviour 0x06080d34 (the part_step), its power 0x100000, the parent's two bytes +0x54/+0x55,
/// its draw, the model pair 29 of kind 0x48, its parent, its script, its offset 0x1e0000, its
/// number (a free one from 16; in the numbered table), which part it is (`side`) and the parent's
/// group; the parent's part is it. Made unless `side` is not 0 and the parent is past 99, or no
/// number is free; the object or 0.
const PART_SIZE: u32 = 0x178;
const PART_NUMBERS: u32 = 0x10;
const PART_EXEC: u32 = 0x0604_d600;
const PART_BEHAVIOUR: u32 = 0x0608_0d34;
const PART_DRAW: u32 = 0x0605_2e50;
const PART_SCRIPT: u32 = 0x060a_b728;
const PART_FLAGS: u32 = 0x80c0_0000;
const PART_POWER: u32 = 0x10_0000;
const PART_OFFSET: u32 = 0x1e_0000;
const PART_KIND: u32 = 0x48;
const PART_MODEL: u32 = 29;
const KF_MODELS: u32 = 0x1c;
const O_DRAW: u32 = 0x34;
const P_SCRIPT: u32 = 0x114;
const P_OFFSET: u32 = 0xdc;

pub fn part_make(cpu: &mut Cpu, parent: u32, side: u32) -> u32 {
    let side = side as i8 as i32 as u32;
    if side != 0 && (cpu.read8(parent + O_STATE) as i8) > GONE {
        return 0;
    }
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, PART_NUMBERS)) as i8;
    if n == -1 {
        return 0;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, PART_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_5694, objects::obj_set_after(cpu, 2, blocks, PART_EXEC, parent));
    if o != 0 {
        nested!(cpu, 0x0605_2908, models::model_init(cpu, o));
        let flags = cpu.read32(o + O_FLAGS) | PART_FLAGS;
        cpu.write32(o + O_FLAGS, flags);
        cpu.write32(o + O_BEHAVIOUR, PART_BEHAVIOUR);
        cpu.write32(o + P_POWER, PART_POWER);
        let b = cpu.read8(parent + P_54);
        cpu.write8(o + P_54, b);
        let b = cpu.read8(parent + P_55);
        cpu.write8(o + P_55, b);
        cpu.write32(o + O_DRAW, PART_DRAW);
        let file = cpu.read32(KINDS + K_SIZE * PART_KIND);
        let models = cpu.read32(file + KF_MODELS);
        let model = cpu.read32(models + 4 * PART_MODEL);
        cpu.write32(o + P_MODEL, model);
        cpu.write32(o + P_PARENT, parent);
        cpu.write32(o + P_SCRIPT, PART_SCRIPT);
        cpu.write32(o + P_OFFSET, PART_OFFSET);
        cpu.write32(NUMBERED + 4 * n as i32 as u32, o);
        cpu.write8(o + O_NUMBER, n as u8);
        cpu.write8(o + P_WHICH, side as u8);
        let group = cpu.read8(parent + P_GROUP);
        cpu.write8(o + P_GROUP, group);
    }
    cpu.write32(parent + P_PART, o);
    o
}

// ---- the boss's pods -----------------------------------------------------------------------------

/// A pod's states (+0x6d): 0 starts it (its power, its angle by which pod it is), 1 waits a frame,
/// 10 until its parent's state passes 14, 15 doing nothing; 100 is its destruction (smoke puffs
/// about it and a sound, for 0x78 less a random 0-29 frames), 101 its end (its model chosen by
/// which pod it is), 102 nothing.
const POD_START: i8 = 0;
const POD_WAIT: i8 = 1;
const POD_TO_RUN: i8 = 10;
const POD_RUNNING: i8 = 15;
const POD_DESTROY: i8 = 100;
const POD_END: i8 = 101;
const POD_DONE: u16 = 102;
const POD_POWER: u32 = 100_000;
const POD_SCORE: u32 = 10_000;
const POD_DESTROY_FRAMES: u32 = 0x78;
const TOTAL_SCORE: u32 = 0x0607_46f0;
const POD_TURNS: [(u32, u16); 3] = [(3, 0x4000), (5, 0x8000), (7, 0xc000)];
const POD_MODELS: [(u32, u32); 4] = [(1, 2), (3, 4), (5, 8), (7, 6)];
const PART_HIT_BY: u32 = 0x40;
const O_HIDE_DRAW: u32 = 0x4_0000;
const O_PARENT_STATE_MAX: i8 = 14;
const LIFE_COUNT: u32 = O_70;

/// The pod (a boss part: behaviour 0x06081f8c): the state machine above, after the first check
/// that sends it to its destruction (state 100, hidden) when its power (+0x50) is gone or its
/// parent (models::follow_parent) is past 99.
pub fn pod_step(cpu: &mut Cpu, o: u32) {
    let parent = nested!(cpu, 0x0605_2970, models::follow_parent(cpu, o));
    let state = cpu.read8(o + O_STATE) as i8;
    if state <= GONE {
        let power = cpu.read32(o + P_POWER) as i32;
        if power <= 0 || cpu.read8(parent + O_STATE) as i8 > GONE {
            let flags = cpu.read32(o + O_FLAGS) | F_HIDDEN;
            cpu.write32(o + O_FLAGS, flags);
            cpu.write16(o + O_STEP, POD_DESTROY as u16);
        }
    }
    match cpu.read8(o + O_STATE) as i8 {
        POD_START => {
            cpu.write32(o + P_POWER, POD_POWER);
            let which = cpu.read8(o + P_WHICH) as i8 as u32;
            for (n, turn) in POD_TURNS {
                if which == n {
                    cpu.write16(o + O_ANGLE, turn);
                }
            }
            cpu.write32(o + P_PART, 0);
            cpu.write16(o + O_STEP, 1);
        }
        POD_WAIT => cpu.write16(o + O_STEP, POD_TO_RUN as u16),
        POD_TO_RUN => {
            if cpu.read8(parent + O_STATE) as i8 > O_PARENT_STATE_MAX {
                let flags = cpu.read32(o + O_FLAGS) & !O_HIDE_DRAW;
                cpu.write32(o + O_FLAGS, flags);
                cpu.write16(o + O_STEP, POD_RUNNING as u16);
            }
        }
        POD_DESTROY => pod_destroy(cpu, o),
        POD_END => {
            let which = cpu.read8(o + P_WHICH) as i8 as u32;
            for (n, model) in POD_MODELS {
                if which == n {
                    let file = cpu.read32(KINDS + K_SIZE * PART_KIND);
                    let models = cpu.read32(file + KF_MODELS);
                    let m = cpu.read32(models + 4 * model);
                    cpu.write32(o + P_MODEL, m);
                }
            }
            cpu.write16(o + O_STEP, POD_DONE);
        }
        _ => {}
    }
}

/// A destroyed pod: once (step 0) the kill scored for what hit it (10000 points to its player,
/// the total 10 on) if its power is gone, its frames set; then each frame a smoke puff at its
/// model's point (on odd frames a twin), every 5th frame scattered with a random, every 15th a
/// sound (12-14); at the end the state 101.
fn pod_destroy(cpu: &mut Cpu, o: u32) {
    if cpu.read8(o + O_STEP) == 0 {
        if cpu.read32(o + P_POWER) as i32 <= 0 {
            let by = cpu.read32(o + PART_HIT_BY);
            let player = cpu.read8(by + O_NUMBER) as i8 as i32 as u32;
            let weapon = cpu.read8(by + P_55) as i8 as i32 as u32;
            nested!(cpu, 0x0604_3108, hits::score_kill(cpu, player, weapon, o, POD_SCORE));
            let total = cpu.read32(TOTAL_SCORE).wrapping_add(10);
            cpu.write32(TOTAL_SCORE, total);
        }
        let r = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 30));
        cpu.write32(o + LIFE_COUNT, POD_DESTROY_FRAMES.wrapping_sub(r));
        cpu.write8(o + O_STEP, 1);
    }
    let p = cpu.stack();
    for k in 0..3 {
        cpu.write32(p + 4 * k, 0);
    }
    nested!(cpu, 0x0605_2c44, models::model_point(cpu, o, p));
    if cpu.read32(o + LIFE_COUNT) & 1 != 0 {
        nested!(cpu, 0x0605_8278, explosions::twin_set(cpu, p));
    }
    let life = cpu.read32(o + LIFE_COUNT);
    if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, life, 5)) == 0 {
        for (k, (reach, back)) in [(0x20_0000u32, 0u32), (0x20_0000, 0), (0x10_0000, 0x10_0000)].into_iter().enumerate() {
            let r = nested!(cpu, 0x0604_5d90, math::random(cpu, reach));
            let v = cpu.read32(p + 4 * k as u32).wrapping_sub(back).wrapping_add(r);
            cpu.write32(p + 4 * k as u32, v);
        }
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x4000));
        cpu.write32(p + 12, r.wrapping_add(0x8000));
        nested!(cpu, 0x0605_77d0, explosions::smoke_puff(cpu, p));
    }
    let life = cpu.read32(o + LIFE_COUNT);
    if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, life, 15)) == 0 {
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 3)) as i32;
        let se = r.wrapping_abs() + 12;
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, se as i16 as u32));
    }
    let life = cpu.read32(o + LIFE_COUNT).wrapping_sub(1);
    cpu.write32(o + LIFE_COUNT, life);
    if life as i32 <= 0 {
        cpu.write16(o + O_STEP, POD_END as u16);
    }
}

// ---- the boss's arms -----------------------------------------------------------------------------

/// An arm's fields: its angle (+0x2c, a word), its swing speed (+0x74, a word), its frame count
/// (+0x70). The boss (models::follow_parent) keeps its parts in a list from +0xb0, by which part
/// the arm is (+0x6e).
const A_ANGLE: u32 = 0x2c;
const A_SWING: u32 = O_74;
const A_FRAMES: u32 = O_70;
const BOSS_PARTS: u32 = 0xb0;
const ARM_SWING_MAX: u32 = 0x400;
const ARM_SWING_STEP: u32 = 0x10;
const ARM_CLOSE: u32 = 0xffff_c000;
const ARM_OPEN_FRAMES: i32 = 59;
const ARM_NEAR: i32 = 15;
const ARM_MODEL: u32 = 10;
const ARM_RAISE_STATE: i8 = 16;
const ARM_LOWER_STATE: i8 = 17;

/// An arm (a boss part: behaviour 0x06082250), the state machine: 16 swings it to the closed
/// angle 0xc000 (its step 1 until it is within 0x2000 of it, then 10, the speed up to 0x400 by
/// 0x10, the angle a quarter of the way round each frame, its step 100 once within 15 of it),
/// 17 swings it to 0 the same way (after 60 frames of its step 1, the speed 0 again); 100 gives
/// it its model and 101; 101 goes on to 102; 0 to 15. After the first check, as the pod's, that
/// sends it to 100 when its parent or the boss is past 99.
pub fn arm_step(cpu: &mut Cpu, o: u32) {
    let boss = nested!(cpu, 0x0605_2970, models::follow_parent(cpu, o));
    let which = cpu.read8(o + P_WHICH) as i8 as i32 as u32;
    let with = cpu.read32(boss.wrapping_add(which << 2).wrapping_add(BOSS_PARTS));
    if cpu.read8(o + O_STATE) as i8 <= GONE {
        nested!(cpu, 0x0604_666c, parts::flash_with(cpu, o, with));
        let parent = cpu.read32(o + P_PARENT);
        if cpu.read8(parent + O_STATE) as i8 > GONE || cpu.read8(boss + O_STATE) as i8 > GONE {
            let flags = cpu.read32(o + O_FLAGS) | F_HIDDEN;
            cpu.write32(o + O_FLAGS, flags);
            cpu.write16(o + O_STEP, POD_DESTROY as u16);
        }
    }
    let state = cpu.read8(o + O_STATE) as i8;
    match state {
        ARM_LOWER_STATE | ARM_RAISE_STATE => {
            let step = cpu.read8(o + O_STEP) as i8;
            let lower = state == ARM_LOWER_STATE;
            match step {
                0 if lower => {
                    cpu.write32(o + A_FRAMES, 0);
                    cpu.write8(o + O_STEP, 1);
                }
                0 => {
                    cpu.write32(o + A_SWING, 0);
                    cpu.write8(o + O_STEP, 1);
                }
                1 if lower => {
                    let frames = cpu.read32(o + A_FRAMES).wrapping_add(1);
                    cpu.write32(o + A_FRAMES, frames);
                    if frames as i32 > ARM_OPEN_FRAMES {
                        cpu.write32(o + A_SWING, 0);
                        cpu.write8(o + O_STEP, 2);
                    }
                }
                1 | 10 if !lower => {
                    if step == 1 && cpu.read16(o + A_ANGLE) as i16 as i32 <= 0x2000 {
                        cpu.write8(o + O_STEP, 10);
                    }
                    arm_swing(cpu, o, ARM_CLOSE, false);
                }
                2 if lower => arm_swing(cpu, o, 0, true),
                100 => cpu.write16(o + O_STEP, POD_RUNNING as u16),
                _ => {}
            }
        }
        POD_START => cpu.write16(o + O_STEP, POD_RUNNING as u16),
        POD_DESTROY => {
            let file = cpu.read32(KINDS + K_SIZE * PART_KIND);
            let models = cpu.read32(file + KF_MODELS);
            let m = cpu.read32(models + 4 * ARM_MODEL);
            cpu.write32(o + P_MODEL, m);
            cpu.write16(o + O_STEP, POD_END as u16);
        }
        POD_END => cpu.write16(o + O_STEP, POD_DONE),
        _ => {}
    }
}

/// An arm turned toward `to`: its swing speed up toward 0x400 by 0x10, its angle moved a quarter
/// of the way (by the angle behind it for the closing swing (`ahead` false), ahead of it for the
/// other), held to the speed; once within 15 of `to` it is there and its step is 100.
fn arm_swing(cpu: &mut Cpu, o: u32, to: u32, ahead: bool) {
    let swing = cpu.read16(o + A_SWING) as i16 as i32 as u32;
    let swing = nested!(cpu, 0x0604_6168, actors::approach(cpu, swing, ARM_SWING_MAX, ARM_SWING_STEP));
    cpu.write16(o + A_SWING, swing as u16);
    let limit = cpu.read16(o + A_SWING) as i16 as i32 as u32;
    let angle = cpu.read16(o + A_ANGLE) as i16 as i32 as u32;
    let d = if ahead {
        nested!(cpu, 0x0604_61cc, camera::angle_ahead(cpu, angle, to))
    } else {
        nested!(cpu, 0x0604_61f0, camera::angle_behind(cpu, angle, to))
    } as i32;
    let quarter = (d / 4) as u32;
    let step = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, quarter, limit));
    let angle = cpu.read16(o + A_ANGLE).wrapping_add(step as u16);
    cpu.write16(o + A_ANGLE, angle);
    let a = angle as i16 as i32 as u32;
    let diff = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, a, to)) as i32;
    if diff.wrapping_abs() <= ARM_NEAR {
        cpu.write16(o + A_ANGLE, to as u16);
        cpu.write8(o + O_STEP, 100);
    }
}

// ---- the boss's draw -----------------------------------------------------------------------------

const SL_PUSH_MATRIX: u32 = 0x0606_71cc;
const SL_POP_MATRIX: u32 = 0x0606_7200;
const SL_TRANSLATE: u32 = 0x0606_721c;
const SL_ROT_Z: u32 = 0x0606_726c;
const SL_ROT_X: u32 = 0x0606_72ec;
const SL_ROT_Y: u32 = 0x0606_736c;
const SL_SCALE: u32 = 0x0606_73ec;
const SL_PUT_POLYGON: u32 = 0x0606_745c;
const SL_GET_MATRIX: u32 = 0x0606_db9c;
/// A drawn object's fields: its place in its group (+0xd8: x, y, z), its turns (+0x30 about z,
/// +0x2c about x, +0x2e about y: words), its scales (+0x20: x, y, z; the flags' 0x10 and 0x20
/// flip x and y), its models (+0x44: the flashing one by the hit flags' bit 0 at +0x56), its
/// matrix saved at +0xe4. The boss has four parts (+0xb4, a long each) drawn inside it.
const D_PLACE: u32 = 0xd8;
const D_TURN_Z: u32 = 0x30;
const D_TURN_X: u32 = 0x2c;
const D_TURN_Y: u32 = 0x2e;
const D_SCALES: u32 = 0x20;
const D_HIT_FLAGS: u32 = 0x56;
const D_MATRIX: u32 = 0xe4;
const FLIP_X: u32 = 0x10;
const FLIP_Y: u32 = 0x20;
const HELD: u32 = super::globals::HELD;
const GAME_FLAGS: u32 = super::globals::GAME_FLAGS;

fn place_and_turn(cpu: &mut Cpu, o: u32) {
    let p = [0, 1, 2].map(|i| cpu.read32(o + D_PLACE + 4 * i));
    cpu.call(SL_TRANSLATE, &p);
    let z = cpu.read16(o + D_TURN_Z) as i16;
    if z != 0 {
        cpu.call(SL_ROT_Z, &[z as i32 as u32]);
    }
}

fn put_and_save(cpu: &mut Cpu, o: u32) {
    let y = cpu.read16(o + D_TURN_Y) as i16;
    if y != 0 {
        cpu.call(SL_ROT_Y, &[y as i32 as u32]);
    }
    let models = cpu.read32(o + P_MODEL);
    let flashing = (cpu.read16(o + D_HIT_FLAGS) & 1) as u32;
    let model = cpu.read32(models + 4 * flashing);
    cpu.call(SL_PUT_POLYGON, &[model]);
    nested!(cpu, 0x0605_29ac, models::model_boxes(cpu, o));
    if cpu.read32(GAME_FLAGS) & HELD == 0 {
        cpu.call(SL_GET_MATRIX, &[o + D_MATRIX]);
    }
}

/// The boss's draw (its sprite `s`, at the object + 0x10): the matrix pushed, moved to its place,
/// turned (z, x, y), scaled (flipped by the flags), its model put, its boxes made and its matrix
/// saved (not while held); then each of its four parts the same, its x turn set from the body's
/// (the first follows the body's, the second the body's and the first's, the third the second's
/// and the fourth is its own); the matrix popped.
pub fn boss_draw(cpu: &mut Cpu, s: u32) {
    let o = s.wrapping_sub(0x10);
    cpu.call(SL_PUSH_MATRIX, &[]);
    place_and_turn(cpu, o);
    let x = cpu.read16(o + D_TURN_X) as i16;
    if x != 0 {
        cpu.call(SL_ROT_X, &[x as i32 as u32]);
    }
    let y = cpu.read16(o + D_TURN_Y) as i16;
    if y != 0 {
        cpu.call(SL_ROT_Y, &[y as i32 as u32]);
    }
    let flags = cpu.read32(o + O_FLAGS);
    let (mut sx, mut sy, sz) = (cpu.read32(o + D_SCALES), cpu.read32(o + D_SCALES + 4), cpu.read32(o + D_SCALES + 8));
    if flags & FLIP_X != 0 {
        sx = sx.wrapping_neg();
    }
    if flags & FLIP_Y != 0 {
        sy = sy.wrapping_neg();
    }
    cpu.call(SL_SCALE, &[sx, sy, sz]);
    let models = cpu.read32(o + P_MODEL);
    let flashing = (cpu.read16(o + D_HIT_FLAGS) & 1) as u32;
    let model = cpu.read32(models + 4 * flashing);
    cpu.call(SL_PUT_POLYGON, &[model]);
    nested!(cpu, 0x0605_29ac, models::model_boxes(cpu, o));
    if cpu.read32(GAME_FLAGS) & HELD == 0 {
        cpu.call(SL_GET_MATRIX, &[o + D_MATRIX]);
    }
    for i in 0..4u32 {
        let part = cpu.read32(o + P_PART + 4 * i);
        if part == 0 {
            continue;
        }
        place_and_turn(cpu, part);
        let body = cpu.read16(o + D_TURN_X) as i16 as i32 as u32;
        let own = cpu.read16(part + D_TURN_X) as i16 as i32 as u32;
        let turn_x = match i {
            0 => {
                let next = cpu.read32(o + P_PART + 4);
                let next_x = cpu.read16(next + D_TURN_X) as i16 as i32 as u32;
                let d = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, body, next_x)) as i32;
                if d > 0 {
                    cpu.write16(next + D_TURN_X, body as u16);
                }
                let next_x = cpu.read16(next + D_TURN_X) as i16 as i32 as u32;
                let t = nested!(cpu, 0x0604_61f0, camera::angle_behind(cpu, body, next_x));
                let q = (t as i32).wrapping_add(if (t as i32) < 0 { 3 } else { 0 }) >> 2;
                cpu.write16(part + D_TURN_X, q as u16);
                cpu.read16(part + D_TURN_X) as i16 as i32 as u32
            }
            1 => {
                let first = cpu.read32(o + P_PART);
                let first_x = cpu.read16(first + D_TURN_X) as i16 as i32 as u32;
                nested!(cpu, 0x0604_61f0, camera::angle_behind(cpu, body.wrapping_add(first_x), own)) as i16 as i32 as u32
            }
            2 => {
                let next = cpu.read32(o + P_PART + 4);
                let next_x = cpu.read16(next + D_TURN_X) as i16 as i32 as u32;
                nested!(cpu, 0x0604_61cc, camera::angle_ahead(cpu, next_x, own)) as i16 as i32 as u32
            }
            _ => own,
        };
        if turn_x as i16 != 0 || i == 1 || i == 2 {
            cpu.call(SL_ROT_X, &[turn_x]);
        }
        put_and_save(cpu, part);
    }
    cpu.call(SL_POP_MATRIX, &[]);
}

// ---- small parts ---------------------------------------------------------------------------------

/// A part that waits for the boss: state 0 hides nothing but is flagged (+0x10's top bit) and goes
/// to 1; state 1 to 15.
const F_FLAGGED: u32 = 0x8000_0000;

pub fn wake_step(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0605_2970, models::follow_parent(cpu, o));
    match cpu.read8(o + O_STATE) as i8 {
        0 => {
            let flags = cpu.read32(o + O_FLAGS) | F_FLAGGED;
            cpu.write32(o + O_FLAGS, flags);
            cpu.write16(o + O_STEP, 1);
        }
        1 => cpu.write16(o + O_STEP, POD_RUNNING as u16),
        _ => {}
    }
}

/// A glow's draw (its sprite `s`): one sprite at the object's model point (0, -0x140000, 0x300000)
/// scaled by the object's +0x20 (0xc000 on even passes, 0x10000 and the 0x100 flag on odd) plus a
/// random 0-15 times 0x200, frame 0 or 1 and the colour 0x30 or 0x70 on the character base; put
/// in the camera's view (the matrix unit, moved to the camera's place).
const CAMERA: u32 = 0x0607_46e4;
const LOOPS: u32 = super::globals::LOOPS;
const S_FLAGS: u32 = 0;
const S_POSITION: u32 = 4;
const S_SCALES: u32 = 0x10;
const S_COLOUR: u32 = 0x18;
const S_KIND: u32 = 0x1e;
const S_FRAME: u32 = 0x1f;
const S_LIFT: u32 = 0x23;

pub fn glow_draw(cpu: &mut Cpu, s: u32) {
    let o = s.wrapping_sub(0x10);
    let d = cpu.stack();
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, d));
    cpu.write32(d + S_POSITION, 0);
    cpu.write32(d + S_POSITION + 4, 0xffec_0000);
    cpu.write32(d + S_POSITION + 8, 0x30_0000);
    nested!(cpu, 0x0605_2c44, models::model_point(cpu, o, d + S_POSITION));
    cpu.write8(d + S_KIND, 0);
    cpu.write8(d + S_LIFT, 0);
    let odd = cpu.read32(LOOPS) & 1 != 0;
    let size = cpu.read32(o + O_SCALE);
    let base = cpu.read16(CHAR_BASE);
    let scale = if odd {
        let f = cpu.read32(d + S_FLAGS) | 0x100;
        let scale = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, 0x1_0000, size));
        cpu.write32(d + S_SCALES, scale);
        cpu.write16(d + S_COLOUR, base.wrapping_add(0x70));
        cpu.write32(d + S_FLAGS, f);
        cpu.write8(d + S_FRAME, 1);
        scale
    } else {
        let scale = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, 0xc000, size));
        cpu.write32(d + S_SCALES, scale);
        cpu.write16(d + S_COLOUR, base.wrapping_add(0x30));
        cpu.write8(d + S_FRAME, 0);
        scale
    };
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 16));
    let scale = scale.wrapping_add(r.wrapping_mul(0x200));
    cpu.write32(d + S_SCALES, scale);
    cpu.write32(d + S_SCALES + 4, scale);
    nested!(cpu, 0x0606_db5c, sgl::sl_push_no_copy(cpu));
    nested!(cpu, 0x0603_512e, sgl::sl_unit_matrix(cpu, 0));
    let cam = cpu.read32(CAMERA);
    let (x, y, z) = (cpu.read32(cam + 0x14), cpu.read32(cam + 0x18), cpu.read32(cam + 0x1c));
    cpu.call(SL_TRANSLATE, &[x, y, z.wrapping_neg()]);
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
    cpu.call(SL_POP_MATRIX, &[]);
}

// ---- the boss's lamps ----------------------------------------------------------------------------

/// A lamp (a boss part, behaviour 0x0608342c): the pod's destruction (state 100) and end (101,
/// its model by which of the boss's parts it follows: 9, 10, 11, 12), but its glow draw
/// (0x0608331c) and a scale (+0x20) it brings toward 0x20000 (by 0x1000) while its parent is in
/// its first 9 steps of the state 16 or 17, toward 0xc000 (by 0x400) after them, else toward
/// 0x10000.
const LAMP_DRAW: u32 = 0x0608_331c;
const LAMP_MODELS: [(u32, u32); 4] = [(9, 18), (10, 20), (11, 24), (12, 22)];
const LAMP_FIRST_STEPS: i8 = 9;

pub fn lamp_step(cpu: &mut Cpu, o: u32) {
    let boss = nested!(cpu, 0x0605_2970, models::follow_parent(cpu, o));
    let parent = cpu.read32(o + P_PARENT);
    if cpu.read8(o + O_STATE) as i8 <= GONE {
        let power = cpu.read32(o + P_POWER) as i32;
        if power <= 0 || cpu.read8(boss + O_STATE) as i8 > GONE {
            let flags = cpu.read32(o + O_FLAGS) | O_HIDE_DRAW;
            cpu.write32(o + O_FLAGS, flags);
            cpu.write16(o + O_STEP, POD_DESTROY as u16);
        }
    }
    let (to, step) = match cpu.read8(parent + O_STATE) as i8 {
        16 | 17 => {
            if cpu.read8(parent + O_STEP) as i8 > LAMP_FIRST_STEPS {
                (0xc000, 0x400)
            } else {
                (0x2_0000, 0x1000)
            }
        }
        _ => (0x1_0000, 0x400),
    };
    let scale = cpu.read32(o + O_SCALE);
    let scale = nested!(cpu, 0x0604_6168, actors::approach(cpu, scale, to, step));
    cpu.write32(o + O_SCALE, scale);
    match cpu.read8(o + O_STATE) as i8 {
        POD_START => {
            cpu.write32(o + O_DRAW, LAMP_DRAW);
            cpu.write32(o + P_POWER, POD_POWER);
            cpu.write16(o + O_STEP, 1);
        }
        POD_WAIT => {
            if cpu.read8(boss + O_STATE) as i8 > O_PARENT_STATE_MAX {
                let flags = cpu.read32(o + O_FLAGS) & !O_HIDE_DRAW;
                cpu.write32(o + O_FLAGS, flags);
                cpu.write16(o + O_STEP, POD_RUNNING as u16);
            }
        }
        POD_DESTROY => pod_destroy(cpu, o),
        POD_END => {
            let which = cpu.read8(parent + P_WHICH) as i8 as u32;
            for (n, model) in LAMP_MODELS {
                if which == n {
                    let file = cpu.read32(KINDS + K_SIZE * PART_KIND);
                    let models = cpu.read32(file + KF_MODELS);
                    let m = cpu.read32(models + 4 * model);
                    cpu.write32(o + P_MODEL, m);
                }
            }
            cpu.write16(o + O_STEP, POD_DONE);
        }
        _ => {}
    }
}

// ---- the boss's covers, sparks and gauge -----------------------------------------------------------

/// The cover (a boss part, behaviour 0x06083708): flashing with its parent's part at +0xbc; its
/// state 100 goes on to 101, which runs the part at 0x0608076c (not seen run), clears the parent's
/// slot for it and ends it (112).
const COVER_PARENT_PART: u32 = 0xbc;
const COVER_END: u32 = 0x0608_076c;

pub fn cover_step(cpu: &mut Cpu, o: u32) {
    let boss = nested!(cpu, 0x0605_2970, models::follow_parent(cpu, o));
    if cpu.read8(o + O_STATE) as i8 <= GONE {
        let parent = cpu.read32(o + P_PARENT);
        let with = cpu.read32(parent + COVER_PARENT_PART);
        nested!(cpu, 0x0604_666c, parts::flash_with(cpu, o, with));
        let parent = cpu.read32(o + P_PARENT);
        let with = cpu.read32(parent + COVER_PARENT_PART);
        if cpu.read8(with + O_STATE) as i8 > GONE || cpu.read8(boss + O_STATE) as i8 > GONE {
            let flags = cpu.read32(o + O_FLAGS) | F_HIDDEN;
            cpu.write32(o + O_FLAGS, flags);
            cpu.write16(o + O_STEP, POD_DESTROY as u16);
        }
    }
    match cpu.read8(o + O_STATE) as i8 {
        POD_START => {
            let flags = cpu.read32(o + O_FLAGS) | F_FLAGGED;
            cpu.write32(o + O_FLAGS, flags);
            cpu.write16(o + O_STEP, 1);
        }
        POD_WAIT => cpu.write16(o + O_STEP, POD_TO_RUN as u16),
        POD_DESTROY => cpu.write16(o + O_STEP, POD_END as u16),
        POD_END => {
            cpu.call(COVER_END, &[o]);
            let parent = cpu.read32(o + P_PARENT);
            let which = cpu.read8(o + P_WHICH) as i8 as i32 as u32;
            cpu.write32(parent.wrapping_add(which << 2).wrapping_add(P_PART), 0);
            cpu.write8(o + O_MODE, 112);
        }
        _ => {}
    }
}

/// The bay (a boss part, behaviour 0x0608380c): state 0 gives it its power and the scales 0x10000,
/// 1 goes on, 10 waits for the boss's state to pass 14; 15 looks for one of the boss's parts 1, 3,
/// 5, 7 whose own part is the first (+0x6e 0), and flashes against it.
pub fn bay_step(cpu: &mut Cpu, o: u32) {
    let boss = nested!(cpu, 0x0605_2970, models::follow_parent(cpu, o));
    match cpu.read8(o + O_STATE) as i8 {
        10 => {
            if cpu.read8(boss + O_STATE) as i8 > O_PARENT_STATE_MAX {
                cpu.write16(o + O_STEP, POD_RUNNING as u16);
            }
        }
        POD_START => {
            cpu.write32(o + P_POWER, POD_POWER);
            for k in [0x28, 0x24, 0x20] {
                cpu.write32(o + k, 0x1_0000);
            }
            cpu.write16(o + O_STEP, 1);
        }
        POD_WAIT => cpu.write16(o + O_STEP, POD_TO_RUN as u16),
        15 => {
            let mut i = 1u32;
            while i <= 7 {
                let part = cpu.read32(boss.wrapping_add(4 * i).wrapping_add(P_PART));
                let inner = cpu.read32(part + P_PART);
                if inner != 0 && cpu.read8(inner + P_WHICH) == 0 {
                    nested!(cpu, 0x0604_668c, parts::flash_against(cpu, o, inner));
                    return;
                }
                i += 2;
            }
        }
        _ => {}
    }
}

/// A spark of the boss (behaviour 0x060838d4): 0 starts it at the model point (0, 0, 0) of the
/// 0xc0 object of one of the boss's parts 9-11 (9 plus a random 0-3), the sprite frame 0xd6 and
/// the colour 0xa0 on the sprites' base; 1 and 2 scale it (0xc000, then 0x20000, a frame 0x38 of
/// kind 3, a speed in x of a 300th of its place); 3 moves it, turning, 0x80000 down each frame
/// until it is 0x800000 under, when it ends.
const SPARK_BOSS: u32 = 0xb0;
const SPARK_PART_OBJECT: u32 = 0xc0;
const O_X_SPEED: u32 = 0x5c;
const O_Y_SPEED: u32 = 0x60;
const SPARK_DIVISOR: i64 = 0x1b4e_81b5;

pub fn boss_spark_step(cpu: &mut Cpu, o: u32) {
    match cpu.read8(o + O_STATE) as i8 {
        0 => {
            let boss = cpu.read32(o + SPARK_BOSS);
            let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 3)) as i32;
            let n = r.wrapping_abs().wrapping_add(9) as i8 as i32 as u32;
            let part = cpu.read32(boss.wrapping_add(n << 2).wrapping_add(P_PART));
            let target = cpu.read32(part + SPARK_PART_OBJECT);
            for k in 0..3 {
                cpu.write32(o + O_POSITION + 4 * k, 0);
            }
            nested!(cpu, 0x0605_2c44, models::model_point(cpu, target, o + O_POSITION));
            cpu.write8(o + O_KIND, 0);
            cpu.write8(o + O_FRAME, 0xd6);
            let colour = cpu.read16(CHAR_BASE).wrapping_add(0xa0);
            cpu.write16(o + O_COLOUR_B, colour);
            cpu.write16(o + O_COLOUR_A, colour);
            cpu.write8(o + O_33, 8);
            cpu.write8(o + P_55, 4);
            cpu.write16(o + O_STEP, 1);
        }
        1 => {
            cpu.write32(o + O_SCALE_2, 0xc000);
            cpu.write32(o + O_SCALE, 0xc000);
            cpu.write16(o + O_STEP, 2);
        }
        2 => {
            cpu.write32(o + O_SCALE_2, 0x2_0000);
            cpu.write32(o + O_SCALE, 0x2_0000);
            cpu.write8(o + O_KIND, 3);
            cpu.write8(o + O_FRAME, 0x38);
            let x = cpu.read32(o + O_POSITION) as i32;
            let high = ((x as i64 * SPARK_DIVISOR) >> 32) as i32;
            let speed = (high >> 5).wrapping_sub(if x < 0 { -1 } else { 0 });
            cpu.write32(o + O_X_SPEED, speed as u32);
            cpu.write32(o + O_Y_SPEED, 0);
            cpu.write16(o + O_STEP, 3);
        }
        3 => {
            let x = cpu.read32(o + O_POSITION).wrapping_add(cpu.read32(o + O_X_SPEED));
            cpu.write32(o + O_POSITION, x);
            let y = cpu.read32(o + O_POSITION + 4).wrapping_add(cpu.read32(o + O_Y_SPEED));
            cpu.write32(o + O_POSITION + 4, y);
            let z = cpu.read32(o + O_POSITION + 8).wrapping_add(0xfff8_0000);
            cpu.write32(o + O_POSITION + 8, z);
            let a = cpu.read16(o + O_ANGLE).wrapping_add(0x1000);
            cpu.write16(o + O_ANGLE, a);
            if z as i32 <= -0x80_0000 {
                cpu.write8(o + O_MODE, MODE_DONE);
                cpu.write16(o + O_STEP, 4);
            }
        }
        _ => {}
    }
}

/// A group's draw (sprite `s`: its place at +4, its turns at +0x1c about x, +0x1e about y and
/// +0x20 about z, words): the matrix pushed, moved and turned (a turn of 0 not made), left open.
pub fn place_draw(cpu: &mut Cpu, s: u32) {
    cpu.call(SL_PUSH_MATRIX, &[]);
    let p = [1, 2, 3].map(|i| cpu.read32(s + 4 * i));
    cpu.call(SL_TRANSLATE, &p);
    for (field, rot) in [(0x1c, SL_ROT_X), (0x1e, SL_ROT_Y), (0x20, SL_ROT_Z)] {
        let a = cpu.read16(s + field) as i16;
        if a != 0 {
            cpu.call(rot, &[a as i32 as u32]);
        }
    }
}

/// The boss's gauge object (behaviour 0x06083aac; the boss is the part at +0xb4): the gauge
/// shows its power (+0x50); a count (+0x70) runs down while the first or second numbered object
/// exists; when it is out, or the power is, it ends (state 100), scoring the rate 100000 and 20
/// more if the power is gone.
pub fn gauge_step(cpu: &mut Cpu, o: u32) {
    let boss = cpu.read32(o + P_PART);
    let power = cpu.read32(boss + P_POWER);
    nested!(cpu, 0x0604_64b8, hud::boss_gauge_life(cpu, power));
    if cpu.read32(NUMBERED) != 0 || cpu.read32(NUMBERED + 4) != 0 {
        let c = cpu.read32(o + O_70).wrapping_sub(1);
        cpu.write32(o + O_70, c);
    }
    if cpu.read32(o + O_70) as i32 > 0 {
        if cpu.read32(boss + P_POWER) as i32 > 0 {
            return;
        }
        cpu.write32(0x0607_46f4, 100_000);
        let total = cpu.read32(TOTAL_SCORE).wrapping_add(0x14);
        cpu.write32(TOTAL_SCORE, total);
    }
    cpu.write16(o + O_STEP, POD_DESTROY as u16);
}

// ---- the stage's gauge object, ground scenery and map tiles --------------------------------------

/// The boss's gauge object made (list 2, 0x138 bytes): a free number from 2 (made unless it is
/// past 46), its power 0x100000, its bytes +0x54/+0x55 2 and 3, flagged 0x20000, its behaviour the
/// gauge's (gauge_step's 0x06083b28 is the stage's boss event), numbered; its number, or 0.
const GAUGE_SIZE: u32 = 0x138;
const GAUGE_FIRST: u32 = 2;
const GAUGE_LAST: i8 = 46;
const GAUGE_BEHAVIOUR: u32 = 0x0608_3b28;
const GAUGE_FLAGS: u32 = 0x2_0000;

pub fn gauge_make(cpu: &mut Cpu) -> u32 {
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, GAUGE_FIRST)) as i8;
    if n > GAUGE_LAST {
        return 0;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, GAUGE_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, PART_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + 0x3c));
    cpu.write16(o + O_STEP, 0);
    cpu.write32(o + P_POWER, 0x10_0000);
    cpu.write8(o + P_54, 2);
    cpu.write8(o + P_55, 3);
    let flags = cpu.read32(o + O_FLAGS) | GAUGE_FLAGS;
    cpu.write32(o + O_FLAGS, flags);
    cpu.write32(o + O_BEHAVIOUR, GAUGE_BEHAVIOUR);
    cpu.write8(o + O_NUMBER, n as u8);
    cpu.write32(NUMBERED.wrapping_add((n as i32 as u32) << 2), o);
    n as i32 as u32
}

/// A ground object's exec (ground::ground_exec) and the ground's step added to its y.
pub fn ground_object_exec(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0605_a4cc, ground::ground_exec(cpu, o));
    let at = cpu.read32(super::globals::GROUND_DY_AT);
    let dy = cpu.read32(at);
    let y = cpu.read32(o + 0x18).wrapping_add(dy);
    cpu.write32(o + 0x18, y);
}

/// A map tile's (the struct at `t`: its mode +9) update: mode 3 puts its place (+4, +6, words) on
/// a circle of 0xd0 by 0xc0 about its angle (+0x14); mode 4 on its first frame (+0x12 0) gives it
/// a random shade (+0x1b, 0-2) and direction (+0xb, 1 or -1), on its second turns the direction
/// round, and on its third sets +8 to 25.
const T_MODE: u32 = 9;
const T_X: u32 = 4;
const T_Y: u32 = 6;
const T_ANGLE: u32 = 0x14;
const T_FRAME: u32 = 0x12;
const T_SHADE: u32 = 0x1b;
const T_DIRECTION: u32 = 0xb;
const T_8: u32 = 8;

pub fn tile_step(cpu: &mut Cpu, t: u32) {
    match cpu.read8(t + T_MODE) {
        3 => {
            let a = cpu.read16(t + T_ANGLE) as i16 as i32 as u32;
            let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
            let v = (s as i32).wrapping_mul(0xd0);
            let v = if v < 0 { v.wrapping_add(0xffff) } else { v };
            cpu.write16(t + T_X, (v >> 16) as u16);
            let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
            let v = (c as i32).wrapping_neg().wrapping_mul(0xc0);
            let v = if v < 0 { v.wrapping_add(0xffff) } else { v };
            cpu.write16(t + T_Y, (v >> 16) as u16);
        }
        4 => {
            if cpu.read8(t + T_FRAME) == 0 {
                let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000)) as i32;
                let shade = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, r.wrapping_abs() as u32, 3));
                cpu.write8(t + T_SHADE, shade as u8);
                let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
                cpu.write8(t + T_DIRECTION, if r & 0x100 != 0 { 1 } else { 0xff });
            }
            if cpu.read8(t + T_FRAME) == 1 {
                let d = cpu.read8(t + T_DIRECTION) as i8;
                cpu.write8(t + T_DIRECTION, if d > 0 { 0xff } else { 1 });
            }
            if cpu.read8(t + T_FRAME) == 2 {
                cpu.write8(t + T_8, 25);
            }
        }
        _ => {}
    }
}

/// A map tile's shade (+0x1b) by its mode (+9): mode 2 by its frame's parity, modes 1 and 4 the
/// next of three.
pub fn tile_shade(cpu: &mut Cpu, t: u32) {
    match cpu.read8(t + T_MODE) as i8 {
        2 => {
            let odd = cpu.read8(t + T_FRAME) & 1 != 0;
            cpu.write8(t + T_SHADE, odd as u8);
        }
        1 | 4 => {
            let next = (cpu.read8(t + T_SHADE) as u32).wrapping_add(1);
            let r = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, next, 3));
            cpu.write8(t + T_SHADE, r as u8);
        }
        _ => {}
    }
}
