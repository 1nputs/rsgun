//! STG03.BIN's code (fingerprint fd6475daa2eebeb4), which game mode 9 runs in the overlay window:
//! stage 4, taken at STG01's end instead of stage 2 (the stage 4 route's evidence,
//! out/evidence/route4*).

use super::globals::{bios_setting_is_1, GAME_FRAMES, GAME_MODE};
use super::bullets::{self, AS_FLAGS, AS_POSITION, AS_SHOOTER, AS_SPEED};
use super::{clib, actors, aim, bg, camera, ground, hud, parts, sgl, sprites, dma, effects, explosions, files, hits, kinds, math, models, nested, objects, screen, sound, states, stg02, timeline, Cpu};

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

// ---- the stage boss's body part ------------------------------------------------------------------

/// The body part (the object part_make makes; behaviour 0x06080d34): its state (+0x6d) and, in
/// the states that have them, its phase (+0x6c, set apart from the state by a byte write; a word
/// write to +0x6c sets the state and clears the phase). Its fields besides the boss part's:
/// the speed it falls at (+0x78), the counts (+0x70, +0x74), a turning speed (+0x7c, a word), its
/// turn about z (+0x30, a word); the part_step's frame holds a word at +24 (the angle it aims,
/// aimed_set's AS_ANGLE too) and the shot description aimed_set takes.
const PS_HOLD: i8 = 1;
const PS_RISE: i8 = 2;
const PS_READY: i8 = 10;
const PS_HIT: i8 = 15;
const PS_AIMED: i8 = 16;
const PS_FAN: i8 = 17;
const PS_SWEEP: i8 = 18;
const PS_FALL: i8 = 99;
const PS_WRECK: i8 = 100;
const PS_RELEASE: i8 = 102;
const P_SPEED: u32 = O_78;
const P_COUNT: u32 = O_70;
const P_WAIT: u32 = O_74;
const P_SPIN: u32 = 0x7c;
const P_TURN: u32 = 0x30;
const P_TURN_X: u32 = 0x2c;
const P_TURN_Y: u32 = 0x2e;
/// The frame's word the aim is kept in (a shot description's angle).
const LOCAL: u32 = 24;
/// The scripts and powers of the two kinds of part: the guns (which 1-3: models 26-28) and the
/// body (any other: model 29).
const GUN_SCRIPT: u32 = 0x060a_b758;
const GUN_POWER: u32 = 10_000;
const GUN_MODELS: u32 = 25;
const BODY_MODEL: u32 = 29;
/// The offset (+0xdc) the body rises to in its state 2.
const RISE_TO: u32 = 0xfff8_0000;
const RISE_SPEED: u32 = 0x2_0000;
const RISE_STEP: u32 = 0x1000;
const RISE_NEAR: i32 = 0x7fff;
/// The hit flag that tells the body it was hit, and the damage's pass to the boss's first part.
const HIT: u16 = 0x200;
/// The fall (state 99): the offset past which it ends the phase, the speed it gains.
const FALL_SPEED: u32 = 0x4_0000;
const FALL_FROM: i32 = 0x1d_ffff;
/// The aimed pair (the guns' shots): the shot's speed, and its two places.
const PAIR_SPEED: u32 = 0x1_5800;
const PAIR_SOUND: u32 = 30;
const MUZZLE_SOUND: u32 = 119;

pub fn part_step(cpu: &mut Cpu, o: u32) {
    let boss = nested!(cpu, 0x0605_2970, models::follow_parent(cpu, o));
    if cpu.read8(o + O_STATE) == 0 {
        part_start(cpu, o);
    }
    if cpu.read8(o + O_STATE) as i8 <= GONE {
        part_check(cpu, o, boss);
    }
    match cpu.read8(o + O_STATE) as i8 {
        PS_HOLD => part_hold(cpu, o, boss),
        PS_RISE => part_rise(cpu, o),
        PS_READY => part_ready(cpu, o, boss),
        PS_HIT => part_hit(cpu, o, boss),
        PS_AIMED => part_aimed(cpu, o, boss),
        PS_FAN => part_fan(cpu, o, boss),
        PS_SWEEP => part_sweep(cpu, o, boss),
        PS_FALL => part_fall(cpu, o, boss),
        PS_WRECK => part_wreck(cpu, o),
        101 => {
            cpu.call(COVER_END, &[o]);
            part_release(cpu, o);
        }
        PS_RELEASE => part_release(cpu, o),
        _ => {}
    }
}

fn set_state(cpu: &mut Cpu, o: u32, state: i8) {
    cpu.write16(o + O_STEP, state as u16);
}

fn set_phase(cpu: &mut Cpu, o: u32, phase: u8) {
    cpu.write8(o + O_STEP, phase);
}

fn gun_model(cpu: &mut Cpu, n: u32) -> u32 {
    let file = cpu.read32(KINDS + K_SIZE * PART_KIND);
    let models = cpu.read32(file + KF_MODELS);
    cpu.read32(models + 4 * n)
}

/// State 0: the guns (which 1-3) and the body: script, power, model (the body goes straight to
/// state 2 when its parent is gone), then the model's colour by its side.
fn part_start(cpu: &mut Cpu, o: u32) {
    cpu.write32(o + P_SPEED, 0);
    set_state(cpu, o, PS_HOLD);
    let which = cpu.read8(o + P_WHICH) as i8 as i32;
    let frame = if (1..=3).contains(&which) {
        cpu.write32(o + P_SCRIPT, GUN_SCRIPT);
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 3)) as i32;
        cpu.write8(o + P_55, r.wrapping_abs() as u8);
        cpu.write32(o + P_POWER, GUN_POWER);
        GUN_MODELS + which as u32
    } else {
        cpu.write32(o + P_SCRIPT, PART_SCRIPT);
        cpu.write32(o + P_POWER, PART_POWER);
        let parent = cpu.read32(o + P_PARENT);
        if cpu.read8(parent + O_STATE) as i8 > GONE {
            set_state(cpu, o, PS_RISE);
        }
        BODY_MODEL
    };
    let model = gun_model(cpu, frame);
    cpu.write32(o + P_MODEL, model);
    let side = cpu.read8(o + P_55) as i8 as i32 as u32;
    nested!(cpu, 0x0608_0cc8, kind_side_colour(cpu, frame, side));
}

/// The first checks: the part is taken down (hidden, state 100) when the boss is gone, and, for
/// all but the first body part, when its power or its parent is; else, while the part of the boss
/// its parent stands for is in state 17, it falls (state 99).
fn part_check(cpu: &mut Cpu, o: u32, boss: u32) {
    let parent = cpu.read32(o + P_PARENT);
    let mut gone = cpu.read8(boss + O_STATE) as i8 > GONE;
    if cpu.read8(o + P_WHICH) != 0 {
        if cpu.read32(o + P_POWER) as i32 <= 0 {
            gone = true;
        }
        if cpu.read8(parent + O_STATE) as i8 > GONE {
            gone = true;
        }
    }
    if gone {
        let flags = cpu.read32(o + O_FLAGS) | F_HIDDEN;
        cpu.write32(o + O_FLAGS, flags);
        set_state(cpu, o, PS_WRECK);
        return;
    }
    if cpu.read8(o + O_STATE) as i8 > GONE - 1 {
        return;
    }
    let (_, part) = parents_part(cpu, o, boss);
    if cpu.read8(part + O_STATE) as i8 == PS_FAN {
        set_state(cpu, o, GONE);
    }
}

/// The boss's part its parent stands for.
fn parents_part(cpu: &mut Cpu, o: u32, boss: u32) -> (u32, u32) {
    let parent = cpu.read32(o + P_PARENT);
    let which = cpu.read8(parent + P_WHICH) as i8 as i32 as u32;
    (parent, cpu.read32(boss.wrapping_add(which << 2).wrapping_add(P_PARTS)))
}

/// State 1: phase 0 gives the boss's part state 16; phase 1 waits until that part has run 10
/// frames, its parent is gone or it is in state 15 (phase 10); phase 10 goes on to state 2.
fn part_hold(cpu: &mut Cpu, o: u32, boss: u32) {
    match cpu.read8(o + O_STEP) as i8 {
        0 => {
            let (_, part) = parents_part(cpu, o, boss);
            set_state(cpu, part, PS_AIMED);
            set_phase(cpu, o, 1);
        }
        1 => {
            let (parent, part) = parents_part(cpu, o, boss);
            if cpu.read8(part + O_STEP) as i8 > 9
                || cpu.read8(parent + O_STATE) as i8 > GONE
                || cpu.read8(part + O_STATE) as i8 == PS_HIT
            {
                set_phase(cpu, o, 10);
            }
        }
        10 => set_state(cpu, o, PS_RISE),
        _ => {}
    }
}

/// State 2: the body's offset (+0xdc) eased up toward its place (a sixteenth of the way each
/// frame, held to a speed that grows to 0x20000), and state 10 once it is there.
fn part_rise(cpu: &mut Cpu, o: u32) {
    let flags = cpu.read32(o + O_FLAGS) & !F_FLAGGED;
    cpu.write32(o + O_FLAGS, flags);
    let speed = cpu.read32(o + P_SPEED);
    let speed = nested!(cpu, 0x0604_6168, actors::approach(cpu, speed, RISE_SPEED, RISE_STEP));
    cpu.write32(o + P_SPEED, speed);
    let at = cpu.read32(o + P_OFFSET);
    let to_go = (RISE_TO.wrapping_sub(at) as i32) / 16;
    let step = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, to_go as u32, speed));
    let at = cpu.read32(o + P_OFFSET).wrapping_add(step);
    cpu.write32(o + P_OFFSET, at);
    if (RISE_TO.wrapping_sub(at) as i32).wrapping_abs() > RISE_NEAR {
        return;
    }
    cpu.write32(o + P_OFFSET, RISE_TO);
    set_state(cpu, o, PS_READY);
}

/// State 10: once the boss is past its state 14 the body is shown (no longer hidden from the
/// draw), its count cleared, and it goes to state 15 plus which part it is (15 the body, 16-18
/// the guns).
fn part_ready(cpu: &mut Cpu, o: u32, boss: u32) {
    if cpu.read8(boss + O_STATE) as i8 <= O_PARENT_STATE_MAX {
        return;
    }
    let flags = cpu.read32(o + O_FLAGS) & !O_HIDE_DRAW;
    cpu.write32(o + O_FLAGS, flags);
    cpu.write32(o + P_WAIT, 0);
    let which = cpu.read8(o + P_WHICH) as i8 as i32;
    set_state(cpu, o, (which + PS_HIT as i32) as i8);
}

/// State 15, the body: when it is hit (the flag 0x200) what it lost of its power (from 0x100000)
/// is taken from the boss's first part's, which takes whatever hit it, and its power is whole.
fn part_hit(cpu: &mut Cpu, o: u32, boss: u32) {
    if (cpu.read16(o + D_HIT_FLAGS) as i16 as i32 & HIT as i32) == 0 {
        return;
    }
    let first = cpu.read32(boss + P_PART);
    let power = cpu.read32(first + P_POWER).wrapping_sub(PART_POWER).wrapping_add(cpu.read32(o + P_POWER));
    cpu.write32(first + P_POWER, power);
    let first = cpu.read32(boss + P_PART);
    let by = cpu.read32(o + PART_HIT_BY);
    cpu.write32(first + PART_HIT_BY, by);
    cpu.write32(o + P_POWER, PART_POWER);
}

/// The idle phase of the guns' states: count down +0x74, then go to phase 30 while the boss is
/// in state 17, else to phase 10 or 20.
fn gun_idle(cpu: &mut Cpu, o: u32, boss: u32) {
    let wait = cpu.read32(o + P_WAIT);
    if wait as i32 > 0 {
        cpu.write32(o + P_WAIT, wait.wrapping_sub(1));
        return;
    }
    cpu.write32(o + P_WAIT, 0);
    if cpu.read8(boss + O_STATE) as i8 == PS_FAN {
        set_phase(cpu, o, 30);
        return;
    }
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 2)) as i32;
    set_phase(cpu, o, (r.wrapping_abs() + 1).wrapping_mul(10) as u8);
}

/// The gun's turning speed (+0x7c), a word.
fn spin(cpu: &mut Cpu, o: u32) -> u32 {
    cpu.read16(o + P_SPIN) as i16 as i32 as u32
}

fn spin_up(cpu: &mut Cpu, o: u32, to: u32, step: u32) {
    let s = spin(cpu, o);
    let s = nested!(cpu, 0x0604_6168, actors::approach(cpu, s, to, step));
    cpu.write16(o + P_SPIN, s as u16);
}

/// The gun turned (about z) toward `to`: `1/divisor` of the way, held to its turning speed.
fn gun_turn(cpu: &mut Cpu, o: u32, to: u32, divisor: i32) {
    let limit = spin(cpu, o);
    gun_turn_by(cpu, o, to, divisor, limit);
}

fn gun_turn_by(cpu: &mut Cpu, o: u32, to: u32, divisor: i32, limit: u32) {
    let at = cpu.read16(o + P_TURN) as i16 as i32 as u32;
    let d = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, at, to)) as i32;
    let step = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, (d / divisor) as u32, limit));
    let angle = cpu.read16(o + P_TURN).wrapping_add(step as u16);
    cpu.write16(o + P_TURN, angle);
}

fn count_down(cpu: &mut Cpu, o: u32, field: u32) -> i32 {
    let n = cpu.read32(o + field).wrapping_sub(1);
    cpu.write32(o + field, n);
    n as i32
}

/// The aim kept in the frame's word: the angle to the player (aim.rs, after `delay` frames) less
/// the parent's own, held within 0x2800 of straight on when `held`.
fn gun_aim(cpu: &mut Cpu, o: u32, delay: u32, held: bool) -> u32 {
    let aim = nested!(cpu, 0x0604_0df0, aim::aim(cpu, o, delay));
    let parent = cpu.read32(o + P_PARENT);
    let own = nested!(cpu, 0x0608_0860, stg02::model_screen_angle(cpu, parent));
    let local = cpu.stack() + LOCAL;
    cpu.write16(local, aim.wrapping_sub(own) as u16);
    if held {
        let a = cpu.read16(local) as i16 as i32 as u32;
        let a = nested!(cpu, 0x0604_6214, camera::angle_within(cpu, a, 0, 0x2800));
        cpu.write16(local, a as u16);
    }
    cpu.read16(local) as i16 as i32 as u32
}

/// The aimed pair: the shot description on the frame (its angle the part's own, no aim, speed
/// 0x15800, the part its shooter) at the two model points (-0x40000, -0x80000, 0) and (0x40000,
/// -0x80000, 0), a shot from each, and the sound 30.
fn aimed_pair(cpu: &mut Cpu, o: u32) {
    let shot = cpu.stack();
    let own = nested!(cpu, 0x0608_0860, stg02::model_screen_angle(cpu, o));
    cpu.write16(shot + LOCAL, own as u16);
    cpu.write32(shot + AS_FLAGS, 0);
    cpu.write32(shot + AS_SPEED, PAIR_SPEED);
    cpu.write32(shot + AS_SHOOTER, o);
    for x in [0xfffc_0000u32, 0x4_0000] {
        cpu.write32(shot + AS_POSITION, x);
        cpu.write32(shot + AS_POSITION + 4, 0xfff8_0000);
        cpu.write32(shot + AS_POSITION + 8, 0);
        nested!(cpu, 0x0605_2ca0, models::model_screen_point(cpu, o, shot));
        nested!(cpu, 0x0604_dbc4, bullets::aimed_set(cpu, shot));
    }
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, PAIR_SOUND));
}

/// State 16 (a gun that aims at the player): idle; phases 30-32 fire the aimed pair every third
/// frame for a count and wait; 10-12 turn toward the player (held within 0x2800 of straight on)
/// and fire at the end of a wait; 20-22 swing away to -0x2800 and back to 0x2800, firing as they
/// settle.
fn part_aimed(cpu: &mut Cpu, o: u32, boss: u32) {
    match cpu.read8(o + O_STEP) as i8 {
        0 => gun_idle(cpu, o, boss),
        30 => {
            let r = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 5));
            cpu.write32(o + P_COUNT, r.wrapping_add(10));
            cpu.write32(o + P_WAIT, 2);
            set_phase(cpu, o, 31);
        }
        31 => {
            if count_down(cpu, o, P_WAIT) > 0 {
                return;
            }
            aimed_pair(cpu, o);
            if count_down(cpu, o, P_COUNT) > 0 {
                set_phase(cpu, o, 30);
                return;
            }
            let r = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 10));
            cpu.write32(o + P_WAIT, 20u32.wrapping_sub(r));
            set_phase(cpu, o, 32);
        }
        32 => {
            if count_down(cpu, o, P_WAIT) <= 0 {
                set_phase(cpu, o, 30);
            }
        }
        10 => {
            cpu.write32(o + P_COUNT, 10);
            cpu.write16(o + P_SPIN, 0);
            set_phase(cpu, o, 11);
        }
        11 => {
            let r = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 20));
            cpu.write32(o + P_WAIT, 30u32.wrapping_sub(r));
            set_phase(cpu, o, 12);
        }
        12 => {
            let r = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 0x400));
            spin_up(cpu, o, r.wrapping_add(0x400), 16);
            let local = gun_aim(cpu, o, 30, true);
            gun_turn(cpu, o, local, 16);
            if count_down(cpu, o, P_WAIT) > 0 {
                return;
            }
            aimed_pair(cpu, o);
            if count_down(cpu, o, P_COUNT) > 0 {
                set_phase(cpu, o, 11);
            } else {
                set_phase(cpu, o, 0);
            }
        }
        20 => {
            cpu.write32(o + P_COUNT, 10);
            cpu.write16(o + P_SPIN, 0);
            let r = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 30));
            cpu.write32(o + P_WAIT, 90u32.wrapping_sub(r));
            set_phase(cpu, o, 21);
        }
        21 => {
            spin_up(cpu, o, 0x1000, 32);
            gun_turn(cpu, o, 0xffff_d800, 8);
            if count_down(cpu, o, P_WAIT) > 0 {
                return;
            }
            cpu.write16(o + P_SPIN, 0);
            cpu.write32(o + P_WAIT, 0);
            set_phase(cpu, o, 22);
        }
        22 => {
            spin_up(cpu, o, 0x100, 8);
            gun_turn(cpu, o, 0x2800, 16);
            let at = cpu.read16(o + P_TURN) as i16 as i32 as u32;
            let d = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, at, 0x2800)) as i32;
            if d.wrapping_abs() <= 0xff {
                let r = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 60));
                cpu.write32(o + P_COUNT, 0xb4u32.wrapping_sub(r));
                set_phase(cpu, o, 0);
                return;
            }
            let wait = cpu.read32(o + P_WAIT).wrapping_add(1);
            cpu.write32(o + P_WAIT, wait);
            let r = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 5));
            if wait as i32 >= 15i32.wrapping_sub(r as i32) {
                cpu.write32(o + P_WAIT, 0);
                aimed_pair(cpu, o);
            }
        }
        _ => {}
    }
}

/// State 17 (a gun that fires from its muzzle): idle; phases 10-11 sweep the turn, firing three
/// shots (straight and a spread either side) each time the wait is out, for a count; 30-31 fire
/// single shots every 20 frames.
fn part_fan(cpu: &mut Cpu, o: u32, boss: u32) {
    match cpu.read8(o + O_STEP) as i8 {
        0 => gun_idle(cpu, o, boss),
        10 | 20 => {
            let r = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 2));
            cpu.write32(o + P_COUNT, r.wrapping_add(5));
            cpu.write32(o + P_WAIT, 0);
            cpu.write16(o + P_SPIN, 0);
            set_phase(cpu, o, 11);
        }
        11 => {
            let local = cpu.stack() + LOCAL;
            cpu.write16(local, 0);
            let r = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 0x200));
            spin_up(cpu, o, r.wrapping_add(0x400), 16);
            gun_turn(cpu, o, 0, 16);
            if count_down(cpu, o, P_WAIT) > 0 {
                return;
            }
            cpu.write16(local, 0x2800);
            let r = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 2));
            let count = cpu.read32(o + P_COUNT);
            let share = nested!(cpu, 0x0600_6a8e, math::fixed_div(cpu, count, r.wrapping_add(5)));
            let spread = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, 0x2800, share));
            cpu.write16(local, spread as u16);
            let spread = spread as u16 as i16 as i32;
            nested!(cpu, 0x0608_0a54, muzzle_shot(cpu, o, 0));
            nested!(cpu, 0x0608_0a54, muzzle_shot(cpu, o, spread as u32));
            nested!(cpu, 0x0608_0a54, muzzle_shot(cpu, o, (spread.wrapping_neg() as i16) as i32 as u32));
            nested!(cpu, 0x0600_50d4, sound::se_set(cpu, MUZZLE_SOUND));
            if count_down(cpu, o, P_COUNT) > 0 {
                cpu.write32(o + P_WAIT, 30);
            } else {
                cpu.write32(o + P_WAIT, 60);
                set_phase(cpu, o, 0);
            }
        }
        30 => {
            let r = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 2));
            cpu.write32(o + P_COUNT, 4u32.wrapping_sub(r));
            cpu.write32(o + P_WAIT, 0);
            cpu.write16(o + P_SPIN, 0);
            set_phase(cpu, o, 31);
        }
        31 => {
            if count_down(cpu, o, P_WAIT) > 0 {
                return;
            }
            nested!(cpu, 0x0608_0a54, muzzle_shot(cpu, o, 0));
            nested!(cpu, 0x0600_50d4, sound::se_set(cpu, MUZZLE_SOUND));
            if count_down(cpu, o, P_COUNT) > 0 {
                cpu.write32(o + P_WAIT, 20);
            } else {
                cpu.write32(o + P_WAIT, 60);
                set_phase(cpu, o, 0);
            }
        }
        _ => {}
    }
}

/// How far from straight on the player may be for the sweeping gun to follow it.
const SWEEP_CONE: i32 = 0x2400;
const SWEEP_SOUND: u32 = 62;
/// A spark of the sweeping gun (spark_step's object).
const SWEEP_SPARK_UP: u32 = 0xfff8_0000;
const SWEEP_SMOKE_UP: u32 = 0xfffa_0000;
const SPARK_BEHAVIOUR: u32 = 0x0608_0b28;

/// State 18 (a gun that sweeps): idle; 10-12 follow the player within the cone, smoking at its
/// edge, then wait; 13 swings out to the aim and sounds; 14 sprays sparks every fourth frame
/// while the count runs; 15 waits for it and goes back to 11.
fn part_sweep(cpu: &mut Cpu, o: u32, boss: u32) {
    match cpu.read8(o + O_STEP) as i8 {
        0 => gun_idle(cpu, o, boss),
        30 => set_phase(cpu, o, 14),
        10 | 20 => {
            cpu.write32(o + P_WAIT, 0);
            cpu.write16(o + P_SPIN, 0);
            set_phase(cpu, o, 11);
        }
        11 => {
            gun_aim(cpu, o, 10, false);
            let local = cpu.stack() + LOCAL;
            let a = cpu.read16(local) as i16 as i32 as u32;
            let d = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, 0, a)) as i32;
            if d.wrapping_abs() > SWEEP_CONE {
                cpu.write16(local, 0);
                let wait = cpu.read32(o + P_WAIT).wrapping_add(1);
                cpu.write32(o + P_WAIT, wait);
                if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, wait, 8)) == 0 {
                    sweep_smoke(cpu, o);
                }
                spin_up(cpu, o, 64, 32);
            } else {
                let r = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 20));
                cpu.write32(o + P_WAIT, 60u32.wrapping_sub(r));
                set_phase(cpu, o, 12);
            }
            let a = cpu.read16(local) as i16 as i32 as u32;
            gun_turn(cpu, o, a, 16);
        }
        12 => {
            gun_aim(cpu, o, 10, false);
            let local = cpu.stack() + LOCAL;
            let a = cpu.read16(local) as i16 as i32 as u32;
            let d = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, 0, a)) as i32;
            if d.wrapping_abs() > SWEEP_CONE {
                set_phase(cpu, o, 11);
            } else {
                count_down(cpu, o, P_WAIT);
            }
            let r = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 0x200));
            spin_up(cpu, o, r.wrapping_add(0x400), 16);
            let a = cpu.read16(local) as i16 as i32 as u32;
            gun_turn(cpu, o, a, 16);
            if cpu.read32(o + P_WAIT) as i32 <= 0 {
                set_phase(cpu, o, 13);
            }
        }
        13 => {
            let local = cpu.stack() + LOCAL;
            gun_aim(cpu, o, 10, false);
            let r = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 16));
            let s = spin(cpu, o);
            let s = nested!(cpu, 0x0604_6168, actors::approach(cpu, s, 32, r.wrapping_add(32)));
            cpu.write16(o + P_SPIN, s as u16);
            let a = cpu.read16(local) as i16 as i32 as u32;
            let a = nested!(cpu, 0x0604_6214, camera::angle_within(cpu, a, 0, 0x2800));
            cpu.write16(local, a as u16);
            let a = cpu.read16(local) as i16 as i32 as u32;
            gun_turn(cpu, o, a, 16);
            if spin(cpu, o) == 32 {
                let r = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 60));
                cpu.write32(o + P_WAIT, r.wrapping_add(0xb4));
                nested!(cpu, 0x0600_50d4, sound::se_set(cpu, SWEEP_SOUND));
                set_phase(cpu, o, 14);
            }
        }
        14 => {
            gun_aim(cpu, o, 10, true);
            let local = cpu.stack() + LOCAL;
            let a = cpu.read16(local) as i16 as i32 as u32;
            gun_turn(cpu, o, a, 16);
            let wait = cpu.read32(o + P_WAIT);
            if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, wait, 4)) == 0 {
                sweep_spark(cpu, o);
            }
            count_down(cpu, o, P_WAIT);
        }
        15 if count_down(cpu, o, P_WAIT) <= 0 => set_phase(cpu, o, 11),
        _ => {}
    }
}

/// A smoke puff at the muzzle's model point (0, -0x60000, 0), scale 0x6000.
fn sweep_smoke(cpu: &mut Cpu, o: u32) {
    let e = cpu.stack();
    cpu.write32(e, 0);
    cpu.write32(e + 4, SWEEP_SMOKE_UP);
    cpu.write32(e + 8, 0);
    nested!(cpu, 0x0605_2ca0, models::model_screen_point(cpu, o, e));
    let number = cpu.read8(o + O_NUMBER);
    cpu.write8(e + 30, number);
    cpu.write32(e + 12, 0x6000);
    nested!(cpu, 0x0605_77d0, explosions::smoke_puff(cpu, e));
}

/// A spark (spark_step's object, list 2) at the muzzle's screen point, its angle the part's own
/// and its flicker's parity the count's.
fn sweep_spark(cpu: &mut Cpu, o: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, SHOT_SIZE)) as i8 as i32 as u32;
    let s = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, SHOT_EXEC));
    if s != 0 {
        nested!(cpu, 0x0604_62ac, actors::actor_setup(cpu, s));
        cpu.write32(s + O_POSITION, 0);
        cpu.write32(s + O_POSITION + 8, 0);
        let flags = cpu.read32(s + O_FLAGS) | 0x400;
        cpu.write32(s + O_FLAGS, flags);
        cpu.write32(s + O_BEHAVIOUR, SPARK_BEHAVIOUR);
        cpu.write32(s + O_POSITION + 4, SWEEP_SPARK_UP);
        nested!(cpu, 0x0605_2ca0, models::model_screen_point(cpu, o, s + O_POSITION));
        let angle = nested!(cpu, 0x0608_0860, stg02::model_screen_angle(cpu, o));
        cpu.write16(s + O_ANGLE, angle as u16);
        cpu.write8(s + O_33, 8);
        let number = cpu.read8(o + O_NUMBER);
        cpu.write8(s + O_NUMBER, number);
        let b = cpu.read8(o + O_54);
        cpu.write8(s + O_54, b);
        let b = cpu.read8(o + O_55);
        cpu.write8(s + O_55, b);
        let wait = cpu.read32(o + P_WAIT);
        let q = nested!(cpu, 0x0600_6a78, math::quotient(cpu, wait, 4));
        cpu.write16(s + SPARK_PARITY, (q & 1) as u16);
    }
}

/// State 99, the fall: phase 0 hides the part from the draw and starts it falling (+0x78
/// -0x8000); 1 speeds the fall up to 0x40000 and, once the offset (+0xdc) is past 0x1dffff, flags
/// it and goes to phase 10; 10 ends the part (mode 112) and frees its parent's slot when the
/// boss's part its parent stands for is gone or in state 15. Each frame its turns ease to 0 by a
/// sixteenth and the offset grows by the speed.
fn part_fall(cpu: &mut Cpu, o: u32, boss: u32) {
    match cpu.read8(o + O_STEP) as i8 {
        0 => {
            let flags = cpu.read32(o + O_FLAGS) | O_HIDE_DRAW;
            cpu.write32(o + O_FLAGS, flags);
            cpu.write32(o + P_SPEED, 0xffff_8000);
            set_phase(cpu, o, 1);
        }
        1 => {
            let speed = cpu.read32(o + P_SPEED);
            let speed = nested!(cpu, 0x0604_6168, actors::approach(cpu, speed, FALL_SPEED, 0x1000));
            cpu.write32(o + P_SPEED, speed);
            if cpu.read32(o + P_OFFSET) as i32 > FALL_FROM {
                let flags = cpu.read32(o + O_FLAGS) | F_FLAGGED;
                cpu.write32(o + O_FLAGS, flags);
                set_phase(cpu, o, 10);
            }
        }
        10 => {
            let (parent, part) = parents_part(cpu, o, boss);
            let state = cpu.read8(part + O_STATE) as i8;
            if state > GONE || state == PS_HIT {
                cpu.write32(parent + P_PART, 0);
                cpu.write8(o + O_MODE, 112);
            }
        }
        _ => {}
    }
    for turn in [P_TURN_X, P_TURN_Y, P_TURN] {
        let at = cpu.read16(o + turn) as i16 as i32 as u32;
        let d = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, at, 0)) as i32;
        let angle = cpu.read16(o + turn).wrapping_add((d / 16) as u16);
        cpu.write16(o + turn, angle);
    }
    let offset = cpu.read32(o + P_OFFSET).wrapping_add(cpu.read32(o + P_SPEED));
    cpu.write32(o + P_OFFSET, offset);
}

/// State 100, the wreck: phase 0 goes to state 102 at once if the part is flagged; else gives it a
/// count (90 and a random 0-29), a kill's score (2000 to what hit it) if its power is gone, and
/// phase 1. Then smoke bursts (every 5th frame, scattered by the part's model point) and a sound
/// (every 30th: 12-14) while the count runs down; at 0 the state 101.
fn part_wreck(cpu: &mut Cpu, o: u32) {
    if cpu.read8(o + O_STEP) == 0 {
        if cpu.read32(o + O_FLAGS) & F_FLAGGED != 0 {
            set_state(cpu, o, PS_RELEASE);
            return;
        }
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 30));
        cpu.write32(o + P_COUNT, r.wrapping_add(90));
        if cpu.read32(o + P_POWER) as i32 <= 0 {
            let by = cpu.read32(o + PART_HIT_BY);
            let player = cpu.read8(by + O_NUMBER) as i8 as i32 as u32;
            let weapon = cpu.read8(by + P_55) as i8 as i32 as u32;
            nested!(cpu, 0x0604_3108, hits::score_kill(cpu, player, weapon, o, 2000));
        }
        set_phase(cpu, o, 1);
    }
    let count = cpu.read32(o + P_COUNT);
    if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, count, 5)) == 0 {
        let p = cpu.stack() + 64;
        let x = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x18_0000));
        cpu.write32(p, x);
        let y = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x10_0000)).wrapping_add(0x10_0000);
        cpu.write32(p + 4, y);
        let z = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x10_0000)).wrapping_sub(0x10_0000);
        cpu.write32(p + 8, z);
        nested!(cpu, 0x0605_2c44, models::model_point(cpu, o, p));
        let scale = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x4000)).wrapping_add(0x8000);
        cpu.write32(p + 12, scale);
        nested!(cpu, 0x0605_77d0, explosions::smoke_puff(cpu, p));
    }
    let count = cpu.read32(o + P_COUNT);
    if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, count, 30)) == 0 {
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 3)) as i32;
        let se = r.wrapping_abs() + 12;
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, se as i16 as u32));
    }
    if count_down(cpu, o, P_COUNT) <= 0 {
        set_state(cpu, o, 101);
    }
}

/// States 101 (after the cover's part) and 102: the parent's part (+0xb4) cleared and the part
/// ended.
fn part_release(cpu: &mut Cpu, o: u32) {
    let parent = cpu.read32(o + P_PARENT);
    cpu.write32(parent + P_PART, 0);
    cpu.write8(o + O_MODE, 112);
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

// ---- the boss's body -----------------------------------------------------------------------------

/// The boss's fields: its kind (+0x6e: 9 to 12, which four-part model set and turn it starts with),
/// its step (+0x6c) within its state (+0x6d), a count (+0x70, a long; its low word +0x72 is the
/// sway's angle), a speed (+0x74, a word) and a rate (+0x78, a word). Its body and two of its
/// parts (the second and third, +0xb8 and +0xbc) are tilted about x (+0x2c, a word) each frame
/// toward an angle by a share of the gap held to the speed.
const B_KIND: u32 = 0x6e;
const B_COUNT: u32 = O_70;
const B_SWAY: u32 = 0x72;
const B_SPEED: u32 = O_74;
const B_RATE: u32 = O_78;
const B_DRAW: u32 = 0x34;
const B_TILT: u32 = 0x2c;
const B_PART_2: u32 = 0xb8;
const B_PART_3: u32 = 0xbc;
const B_PLACE_X: u32 = 0xd8;
const B_PLACE_Y: u32 = 0xdc;
const B_DRAW_CODE: u32 = 0x0608_24b4;
/// The kinds' part tables (0x78 apart, from 9) and the turns they start at.
const B_TABLES: [(u32, u32); 4] = [(0x060a_b9d4, 0x2000), (0x060a_ba4c, 0x6000), (0x060a_bac4, 0xa000), (0x060a_bb3c, 0xe000)];
const B_FIRST_KIND: i32 = 9;
/// The states: swaying (15, from the start), then four tilting moves (16 and 17 lean back, 18 nods
/// up and down, 19 nods once, 20 strikes: its steps 1, 2, 10 and 12) and 21 (a raise).
const BS_SWAY: i8 = 15;
const BS_LEAN: i8 = 16;
const BS_LEAN_LONG: i8 = 17;
const BS_NOD: i8 = 18;
const BS_NOD_ONCE: i8 = 19;
const BS_STRIKE: i8 = 20;
const BS_RAISE: i8 = 21;
/// A turn toward an angle is the plain gap, the way ahead or the way behind (camera.rs).
#[derive(Clone, Copy)]
enum Gap {
    Plain,
    Ahead,
    Behind,
}
/// A share of a gap: all of it, the quotient by 2^n (the SH-2 code adds 2^n - 1 to a negative
/// gap and shifts it) or by 64 through GCC's shift helper.
#[derive(Clone, Copy)]
enum Share {
    All,
    Shift(u32),
    Helper6,
}

fn gap_to(cpu: &mut Cpu, from: u32, gap: Gap, to: u32) -> i32 {
    (match gap {
        Gap::Plain => nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, from, to)),
        Gap::Ahead => nested!(cpu, 0x0604_61cc, camera::angle_ahead(cpu, from, to)),
        Gap::Behind => nested!(cpu, 0x0604_61f0, camera::angle_behind(cpu, from, to)),
    }) as i32
}

fn shift_6(cpu: &mut Cpu, v: u32) {
    nested!(cpu, 0x0600_8168, clib::ashiftrt_r4::<6>(cpu, v));
}

fn shift_7(cpu: &mut Cpu, v: u32) {
    nested!(cpu, 0x0600_8166, clib::ashiftrt_r4::<7>(cpu, v));
}

fn share_of(cpu: &mut Cpu, d: i32, share: Share) -> i32 {
    let bias = |n: u32| if d < 0 { (1 << n) - 1 } else { 0 };
    match share {
        Share::All => d,
        Share::Shift(n) => d.wrapping_add(bias(n)) >> n,
        Share::Helper6 => {
            let v = d.wrapping_add(bias(6));
            shift_6(cpu, v as u32);
            v >> 6
        }
    }
}

/// A tilt (the angle at `at` + 0x2c) moved toward `to`: a share of the gap, held to `limit` when
/// there is one.
fn tilt(cpu: &mut Cpu, at: u32, gap: Gap, to: u32, share: Share, limit: Option<u32>) {
    let angle = cpu.read16(at + B_TILT) as i16 as i32 as u32;
    let d = gap_to(cpu, angle, gap, to);
    let part = share_of(cpu, d, share) as u32;
    let step = match limit {
        Some(limit) => nested!(cpu, 0x0606_4f3c, math::clamp(cpu, part, limit)),
        None => part,
    };
    let angle = cpu.read16(at + B_TILT).wrapping_add(step as u16);
    cpu.write16(at + B_TILT, angle);
}

/// The three tilts (body, second part, third part) toward `targets`, each a share of the gap.
fn tilts(cpu: &mut Cpu, o: u32, targets: [(Gap, u32); 3], share: Share, limit: Option<u32>) {
    let at = [o, cpu.read32(o + B_PART_2), cpu.read32(o + B_PART_3)];
    for (at, (gap, to)) in at.into_iter().zip(targets) {
        tilt(cpu, at, gap, to, share, limit);
    }
}

fn speed(cpu: &mut Cpu, o: u32) -> u32 {
    cpu.read16(o + B_SPEED) as i16 as i32 as u32
}

/// The speed taken toward `to` by `step`.
fn speed_up(cpu: &mut Cpu, o: u32, to: u32, step: u32) {
    let v = speed(cpu, o);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, to, step));
    cpu.write16(o + B_SPEED, v as u16);
}

/// The step's count taken up by one; true once it is past `last`.
fn count_past(cpu: &mut Cpu, o: u32, last: i32) -> bool {
    let n = (cpu.read32(o + B_COUNT) as i32).wrapping_add(1);
    cpu.write32(o + B_COUNT, n as u32);
    n > last
}

/// The speed and count cleared and the rate set, as a nod's step begins; then step `next`.
fn begin_nod(cpu: &mut Cpu, o: u32, next: u8) {
    cpu.write16(o + B_SPEED, 0);
    cpu.write16(o + B_RATE, NOD_RATE);
    cpu.write32(o + B_COUNT, 0);
    cpu.write8(o + O_STEP, next);
}

/// The boss's body (behaviour 0x0608271c), the state machine on its state (+0x6d) and step
/// (+0x6c). 0 picks the part table and turn from its kind (nothing if the parts cannot be made),
/// sets its draw, turns its place by that and goes to 15; 15 sways the body and the second and
/// third parts about the sway angle (the count's low word, going up by 0x100); 16 to 21 are the
/// moves, see the states.
pub fn boss_step(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0605_2970, models::follow_parent(cpu, o));
    match cpu.read8(o + O_STATE) as i8 {
        0 => boss_start(cpu, o),
        BS_SWAY => boss_sway(cpu, o),
        state @ (BS_LEAN | BS_LEAN_LONG) => boss_lean(cpu, o, state),
        BS_NOD => boss_nod(cpu, o),
        BS_NOD_ONCE => boss_nod_once(cpu, o),
        BS_STRIKE => boss_strike(cpu, o),
        BS_RAISE => boss_raise(cpu, o),
        _ => {}
    }
}

fn boss_start(cpu: &mut Cpu, o: u32) {
    cpu.write16(o + O_ANGLE, 0);
    let which = (cpu.read8(o + B_KIND) as i8 as i32).wrapping_sub(B_FIRST_KIND) as u32;
    if let Some(&(table, turn)) = B_TABLES.get(which as usize)
        && nested!(cpu, 0x0605_332c, models::model_parts_set(cpu, o, table)) != 0
    {
        cpu.write16(o + O_ANGLE, turn as u16);
    }
    if cpu.read16(o + O_ANGLE) == 0 {
        return;
    }
    cpu.write32(o + B_DRAW, B_DRAW_CODE);
    let t = cpu.stack();
    let x = cpu.read32(o + B_PLACE_X);
    let y = cpu.read32(o + B_PLACE_Y);
    cpu.write32(t, x);
    cpu.write32(t + 4, y);
    cpu.write32(t + 16, 0x1_0000);
    cpu.write32(t + 12, 0x1_0000);
    cpu.write32(t + 20, 0);
    let turn = cpu.read16(o + O_ANGLE);
    cpu.write16(t + 24, turn);
    nested!(cpu, 0x0606_4eac, math::turn_point(cpu, t));
    let x = cpu.read32(t);
    cpu.write32(o + B_PLACE_X, x);
    let y = cpu.read32(t + 4);
    cpu.write32(o + B_PLACE_Y, y);
    cpu.write16(o + O_STEP, BS_SWAY as u16);
}

const SWAY_SPEED: u32 = 0x1000;
const SWAY_STEP: u32 = 2;
const SWAY_MIDDLE: u32 = 0x800;

/// State 15: the speed up to 0x1000 by 2; the body toward 0x800 less the sway's cosine times 0x800,
/// the second part toward the cosine times 0x800 less 0x800, the third toward 0; the count up by
/// 0x100.
fn boss_sway(cpu: &mut Cpu, o: u32) {
    if cpu.read8(o + O_STEP) == 0 {
        cpu.write16(o + B_SPEED, 0);
        cpu.write32(o + B_COUNT, 0);
        cpu.write8(o + O_STEP, 1);
    }
    speed_up(cpu, o, SWAY_SPEED, SWAY_STEP);
    let limit = speed(cpu, o);
    let sway = cpu.read16(o + B_SWAY) as i16 as i32 as u32;
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, sway));
    let m = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, SWAY_MIDDLE, c));
    tilt(cpu, o, Gap::Plain, SWAY_MIDDLE.wrapping_sub(m), Share::All, Some(limit));
    let part = cpu.read32(o + B_PART_2);
    let limit = speed(cpu, o);
    let sway = cpu.read16(o + B_SWAY) as i16 as i32 as u32;
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, sway));
    let m = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, SWAY_MIDDLE, c));
    tilt(cpu, part, Gap::Plain, m.wrapping_sub(SWAY_MIDDLE), Share::All, Some(limit));
    let part = cpu.read32(o + B_PART_3);
    let limit = speed(cpu, o);
    tilt(cpu, part, Gap::Plain, 0, Share::All, Some(limit));
    let n = cpu.read32(o + B_COUNT).wrapping_add(0x100);
    cpu.write32(o + B_COUNT, n);
}

const LEAN_SPEED: u32 = 0x1000;
const LEAN_BODY: u32 = 0xffff_c800;
const LEAN_PART: u32 = 0xffff_c000;
const LEAN_HOLD: [u32; 2] = [0x3c, 0x78];
const LEAN_START: [u32; 2] = [0x20, 0x40];
const LEAN_RISE: [(Gap, u32); 3] = [(Gap::Ahead, 0x1000), (Gap::Plain, 0xffff_e000), (Gap::Plain, 0)];
const LEAN_HOLDING: u8 = 2;
const STEP_RISE: u8 = 10;

/// States 16 and 17 (a short and a long lean): step 0 sets the speed 0 and the count 0x20 or 0x40;
/// step 1 takes the speed up to 0x1000 by the count and tilts the body and second part back
/// (the way behind) to 0xc800 and 0xc000 and the third to 0, and once the body is at 0xc800 cuts
/// the speed to a quarter and sets the count 0x3c or 0x78, going to step 2; step 2 counts down and
/// goes to step 10, which tilts them up again (a 64th of the gaps, to 0x1000, 0xe000 and 0).
fn boss_lean(cpu: &mut Cpu, o: u32, state: i8) {
    let long = (state == BS_LEAN_LONG) as usize;
    match cpu.read8(o + O_STEP) as i8 {
        0 => {
            cpu.write16(o + B_SPEED, 0);
            cpu.write32(o + B_COUNT, LEAN_START[long]);
            cpu.write8(o + O_STEP, 1);
        }
        1 => {
            let step = cpu.read32(o + B_COUNT);
            speed_up(cpu, o, LEAN_SPEED, step);
            let limit = speed(cpu, o);
            tilts(cpu, o, [(Gap::Behind, LEAN_BODY), (Gap::Behind, LEAN_PART), (Gap::Plain, 0)], Share::All, Some(limit));
            if cpu.read16(o + B_TILT) as i16 as i32 != LEAN_BODY as i32 {
                return;
            }
            // a logical shift, as the SH-2 code has it
            let v = cpu.read16(o + B_SPEED) as i16 as i32;
            let v = (v.wrapping_add(if v < 0 { 3 } else { 0 }) as u32) >> 2;
            cpu.write16(o + B_SPEED, v as u16);
            cpu.write32(o + B_COUNT, LEAN_HOLD[long]);
            cpu.write8(o + O_STEP, LEAN_HOLDING);
        }
        2 => {
            let n = (cpu.read32(o + B_COUNT) as i32).wrapping_sub(1);
            cpu.write32(o + B_COUNT, n as u32);
            if n <= 0 {
                cpu.write8(o + O_STEP, STEP_RISE);
            }
        }
        10 => {
            let limit = speed(cpu, o);
            tilts(cpu, o, LEAN_RISE, Share::Helper6, Some(limit));
        }
        _ => {}
    }
}

const NOD_SPEED: u32 = 0x100;
const NOD_RATE: u16 = 0x10;
const NOD_FRAMES: i32 = 119;
const NOD_DOWN: [(Gap, u32); 3] = [(Gap::Plain, 0x4000), (Gap::Plain, 0xffff_d000), (Gap::Plain, 0)];
const NOD_UP: [(Gap, u32); 3] = [(Gap::Plain, 0x800), (Gap::Plain, 0xffff_f800), (Gap::Plain, 0)];
const NOD_ONCE: [(Gap, u32); 3] = [(Gap::Plain, 0x4000), (Gap::Plain, 0xffff_c000), (Gap::Plain, 0)];
const NOD_TO_UP: u8 = 10;
const NOD_UP_STEP: u8 = 11;

/// The speed up to 0x100 by the rate (+0x78), then the three tilts a 32nd of their gaps toward
/// `targets`, held to the speed.
fn nod_tilts(cpu: &mut Cpu, o: u32, targets: [(Gap, u32); 3]) {
    let rate = cpu.read16(o + B_RATE) as i16 as i32 as u32;
    speed_up(cpu, o, NOD_SPEED, rate);
    let limit = speed(cpu, o);
    tilts(cpu, o, targets, Share::Shift(5), Some(limit));
}

/// State 18: step 0 starts the speed and count at 0 and the rate at 0x10; step 1 tilts them down
/// (to 0x4000, 0xd000 and 0) for 120 frames, then goes to 10, which starts again and goes to 11,
/// which tilts them up (to 0x800, 0xf800, 0) for 120 frames and goes back to 0.
fn boss_nod(cpu: &mut Cpu, o: u32) {
    match cpu.read8(o + O_STEP) as i8 {
        0 => begin_nod(cpu, o, 1),
        1 => {
            nod_tilts(cpu, o, NOD_DOWN);
            if count_past(cpu, o, NOD_FRAMES) {
                cpu.write8(o + O_STEP, NOD_TO_UP);
            }
        }
        10 => {
            begin_nod(cpu, o, NOD_UP_STEP);
        }
        11 => {
            nod_tilts(cpu, o, NOD_UP);
            if count_past(cpu, o, NOD_FRAMES) {
                cpu.write8(o + O_STEP, 0);
            }
        }
        _ => {}
    }
}

/// State 19: step 0 as 18's; step 1 tilts them down (to 0x4000, 0xc000 and 0) and stays there.
fn boss_nod_once(cpu: &mut Cpu, o: u32) {
    match cpu.read8(o + O_STEP) as i8 {
        0 => begin_nod(cpu, o, 1),
        1 => nod_tilts(cpu, o, NOD_ONCE),
        _ => {}
    }
}

const STRIKE_SPEED: u32 = 0x100;
const STRIKE_STEP: u32 = 10;
const STRIKE_WIND: [(Gap, u32); 3] = [(Gap::Ahead, 0x4000), (Gap::Plain, 0xffff_d000), (Gap::Plain, 0x1000)];
const STRIKE_SWING: [(Gap, u32); 3] = [(Gap::Ahead, 0x4000), (Gap::Plain, 0x1000), (Gap::Plain, 0x4000)];
const STRIKE_WIND_FRAMES: i32 = 39;
const STRIKE_SWING_FRAMES: i32 = 79;
const STRIKE_RETURN_SPEED: u32 = 0x800;
const STRIKE_RETURN_STEP: u32 = 8;
const STRIKE_RETURN_FRAMES: i32 = 89;
const STRIKE_SETTLE_FRAMES: i32 = 59;
const STRIKE_WINDING: u8 = 2;
const STRIKE_RETURN: u8 = 10;
const STRIKE_SETTLE: u8 = 12;

/// The speed up to 0x100 by 10, then the three tilts a 32nd of their gaps toward `targets`.
fn strike_tilts(cpu: &mut Cpu, o: u32, targets: [(Gap, u32); 3]) {
    speed_up(cpu, o, STRIKE_SPEED, STRIKE_STEP);
    let limit = speed(cpu, o);
    tilts(cpu, o, targets, Share::Shift(5), Some(limit));
}

/// State 20: step 0 clears the speed and count; step 1 winds up (40 frames: the body and second
/// part toward 0x4000 ahead and 0xd000, the third to 0x1000) and goes to 2, which swings (80
/// frames: toward 0x4000, 0x1000, 0x4000) and goes to 10, which returns them (90 frames: the body
/// and second part an eighth of the gap to 0x800, the third an eighth of the gap to 0x4000 held
/// to a 128th of the speed) and goes to 12, which settles them (a 64th of their gaps to 0x800, 0
/// and 0, 60 frames) and goes to state 15.
fn boss_strike(cpu: &mut Cpu, o: u32) {
    match cpu.read8(o + O_STEP) as i8 {
        0 => {
            cpu.write16(o + B_SPEED, 0);
            cpu.write32(o + B_COUNT, 0);
            cpu.write8(o + O_STEP, 1);
        }
        1 => {
            strike_tilts(cpu, o, STRIKE_WIND);
            if count_past(cpu, o, STRIKE_WIND_FRAMES) {
                cpu.write32(o + B_COUNT, 0);
                cpu.write8(o + O_STEP, STRIKE_WINDING);
            }
        }
        2 => {
            strike_tilts(cpu, o, STRIKE_SWING);
            if count_past(cpu, o, STRIKE_SWING_FRAMES) {
                cpu.write16(o + B_SPEED, 0);
                cpu.write32(o + B_COUNT, 0);
                cpu.write8(o + O_STEP, STRIKE_RETURN);
            }
        }
        10 => {
            speed_up(cpu, o, STRIKE_RETURN_SPEED, STRIKE_RETURN_STEP);
            let limit = speed(cpu, o);
            let part = cpu.read32(o + B_PART_2);
            tilt(cpu, o, Gap::Plain, 0x800, Share::Shift(3), Some(limit));
            tilt(cpu, part, Gap::Plain, 0x800, Share::Shift(3), Some(limit));
            let part = cpu.read32(o + B_PART_3);
            let v = speed(cpu, o) as i32;
            let v = v.wrapping_add(if v < 0 { 127 } else { 0 });
            shift_7(cpu, v as u32);
            let limit = (v >> 7) as i16 as i32 as u32;
            tilt(cpu, part, Gap::Plain, 0x4000, Share::Shift(3), Some(limit));
            if count_past(cpu, o, STRIKE_RETURN_FRAMES) {
                cpu.write32(o + B_COUNT, 0);
                cpu.write8(o + O_STEP, STRIKE_SETTLE);
                cpu.write16(o + O_STEP, BS_SWAY as u16);
            }
        }
        12 => {
            tilts(cpu, o, [(Gap::Plain, 0x800), (Gap::Plain, 0), (Gap::Plain, 0)], Share::Helper6, None);
            if count_past(cpu, o, STRIKE_SETTLE_FRAMES) {
                cpu.write16(o + O_STEP, BS_SWAY as u16);
            }
        }
        _ => {}
    }
}

const RAISE_SPEED: u32 = 0x80;
const RAISE_STEP: u32 = 1;

/// State 21: step 0 clears the speed and count; step 1 takes the speed up to 0x80 by 1 and tilts
/// the body and second part a 64th of their gaps to 0x4000 and 0xc000, the third all of its gap to
/// 0, held to the speed.
fn boss_raise(cpu: &mut Cpu, o: u32) {
    match cpu.read8(o + O_STEP) as i8 {
        0 => {
            cpu.write16(o + B_SPEED, 0);
            cpu.write32(o + B_COUNT, 0);
            cpu.write8(o + O_STEP, 1);
        }
        1 => {
            speed_up(cpu, o, RAISE_SPEED, RAISE_STEP);
            let limit = speed(cpu, o);
            let part = cpu.read32(o + B_PART_2);
            tilt(cpu, o, Gap::Plain, 0x4000, Share::Helper6, Some(limit));
            tilt(cpu, part, Gap::Plain, 0xffff_c000, Share::Helper6, Some(limit));
            let part = cpu.read32(o + B_PART_3);
            tilt(cpu, part, Gap::Plain, 0, Share::All, Some(limit));
        }
        _ => {}
    }
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

/// An enemy row's (enemy_row's event at `t`) motion before each is made: motion 3 puts its place
/// (+4, +6, words) on a circle of 0xd0 by 0xc0 about its direction (+0x14); motion 4 on its first
/// one made (+0x12 0) gives it a random type (+0x1b, 0-2) and side (+0xb, 1 or -1), on its second
/// turns the side round, and on its third sets its script (+8) to 25.
const E_MOTION: u32 = 9;
const E_PLACE: u32 = 4;
const E_DIRECTION: u32 = 0x14;
const E_MADE: u32 = 0x12;
const E_TYPE: u32 = 0x1b;
const E_MIRROR: u32 = 0xb;
const E_SCRIPT: u32 = 8;

pub fn row_arc(cpu: &mut Cpu, t: u32) {
    match cpu.read8(t + E_MOTION) {
        3 => {
            let a = cpu.read16(t + E_DIRECTION) as i16 as i32 as u32;
            let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
            let v = (s as i32).wrapping_mul(0xd0);
            let v = if v < 0 { v.wrapping_add(0xffff) } else { v };
            cpu.write16(t + E_PLACE, (v >> 16) as u16);
            let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
            let v = (c as i32).wrapping_neg().wrapping_mul(0xc0);
            let v = if v < 0 { v.wrapping_add(0xffff) } else { v };
            cpu.write16(t + E_PLACE + 2, (v >> 16) as u16);
        }
        4 => {
            if cpu.read8(t + E_MADE) == 0 {
                let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000)) as i32;
                let shade = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, r.wrapping_abs() as u32, 3));
                cpu.write8(t + E_TYPE, shade as u8);
                let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
                cpu.write8(t + E_MIRROR, if r & 0x100 != 0 { 1 } else { 0xff });
            }
            if cpu.read8(t + E_MADE) == 1 {
                let d = cpu.read8(t + E_MIRROR) as i8;
                cpu.write8(t + E_MIRROR, if d > 0 { 0xff } else { 1 });
            }
            if cpu.read8(t + E_MADE) == 2 {
                cpu.write8(t + E_SCRIPT, 25);
            }
        }
        _ => {}
    }
}

/// An enemy row's type (+0x1b) after each is made, by its motion (+9): 2 by how many are made
/// (+0x12), parity; 1 and 4 the next of three.
pub fn row_retype(cpu: &mut Cpu, t: u32) {
    match cpu.read8(t + E_MOTION) as i8 {
        2 => {
            let odd = cpu.read8(t + E_MADE) & 1 != 0;
            cpu.write8(t + E_TYPE, odd as u8);
        }
        1 | 4 => {
            let next = (cpu.read8(t + E_TYPE) as u32).wrapping_add(1);
            let r = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, next, 3));
            cpu.write8(t + E_TYPE, r as u8);
        }
        _ => {}
    }
}

// ---- the enemy rows -------------------------------------------------------------------------------

/// A row of enemies (a timeline event, as STG01's enemy_row, called each frame until it gives 1):
/// one made every `E_DELAY + 1` frames by its kind (0-2 ground_set's, 3-6 guns, 7-9 the wounded,
/// 10-13 platforms, 14-15 launchers, 20 a burster, 21 a hatch, 22 a pair of platforms about the
/// first made by 0x060899f4, 23 the six scenery pieces of the stage's end, 128 a hatch with the
/// behaviour 0x06084c3c, 129 a ground piece of kind 2 with ground_object_exec), each `E_STEP_X`,
/// `E_STEP_Y` and `E_TURN` on from the last, until `E_COUNT` are made. Its motion (`E_MOTION`)
/// places each round a circle first and changes the type after.
const E_KIND: u32 = 0x01;
const E_COUNT: u32 = 0x10;
const E_DELAY: u32 = 0x11;
const E_WAIT: u32 = 0x13;
const E_TURN: u32 = 0x16;
const E_STEP_X: u32 = 0x18;
const E_STEP_Y: u32 = 0x19;
const E_SETTINGS: u32 = 0x0a;
const E_1A: u32 = 0x1a;
const E_1C: u32 = 0x1c;
const E_1D: u32 = 0x1d;
const E_1E: u32 = 0x1e;
const E_1F: u32 = 0x1f;
/// A ground piece's description (ground.rs's, on the stack): its place, mirror, kind, type,
/// direction, settings, and the kinds' own words.
const G_PLACE: u32 = 0x00;
const G_MIRROR: u32 = 0x14;
const G_KIND: u32 = 0x1a;
const G_TYPE: u32 = 0x2c;
const G_DIRECTION: u32 = 0x2e;
const G_SETTINGS: u32 = 0x30;
const G_32: u32 = 0x32;
const G_34: u32 = 0x34;
const G_36: u32 = 0x36;
const G_38: u32 = 0x38;
const G_3A: u32 = 0x3a;
/// The kinds' scripts (by E_SCRIPT) and aims.
const GROUND_SCRIPTS: u32 = 0x060a_bee0;
const GUN_SCRIPTS: u32 = 0x060a_bfa4;
const GUN_AIMS: u32 = 0x060a_bfac;
const WOUNDED_SCRIPTS: u32 = 0x060a_c038;
const WOUNDED_AIMS: u32 = 0x060a_c06c;
const PLATFORM_SCRIPTS: u32 = 0x060a_c0e0;
/// The pair's maker, and the behaviour the pair's pieces take; the end's scenery makers and where
/// their objects are kept.
const PAIR_MAKER: u32 = 0x0608_99f4;
const PAIR_BEHAVIOUR: u32 = 0x0608_9b58;
const SCENERY_ROUND: u32 = 0x0608_b944;
const SCENERY_PLATE: u32 = 0x0608_b6fc;
const SCENERY: u32 = 0x060b_2060;
const HATCH_BEHAVIOUR: u32 = 0x0608_4c3c;
const GROUND_BEHAVIOUR: u32 = 0x0608_4c10;
const NOT_80000: u32 = 0xfff7_ffff;

pub fn enemy_row(cpu: &mut Cpu, event: u32) -> u32 {
    if cpu.read8(event + E_WAIT) == 0 {
        let d = cpu.stack();
        loop {
            if cpu.read8(event + E_MOTION) != 0 {
                nested!(cpu, 0x0608_4c68, row_arc(cpu, event));
            }
            let delay = cpu.read8(event + E_DELAY);
            cpu.write8(event + E_WAIT, delay);
            let x = (cpu.read16(event + E_PLACE) as i16 as i32 as u32) << 16;
            cpu.write32(d + G_PLACE, x);
            let y = (cpu.read16(event + E_PLACE + 2) as i16 as i32 as u32) << 16;
            cpu.write32(d + G_PLACE + 4, y);
            cpu.write32(d + G_PLACE + 8, 0);
            let t = cpu.read8(event + E_TYPE) as u16;
            cpu.write16(d + G_TYPE, t);
            let direction = cpu.read16(event + E_DIRECTION);
            cpu.write16(d + G_DIRECTION, direction);
            let settings = cpu.read8(event + E_SETTINGS) as u16;
            cpu.write16(d + G_SETTINGS, settings);
            let mirror = cpu.read8(event + E_MIRROR) as i8 as i32 as u32;
            cpu.write32(d + G_MIRROR, mirror);
            row_make(cpu, event, d);
            let made = cpu.read8(event + E_MADE).wrapping_add(1);
            cpu.write8(event + E_MADE, made);
            if made >= cpu.read8(event + E_COUNT) {
                return 1;
            }
            for (at, step) in [(E_PLACE, E_STEP_X), (E_PLACE + 2, E_STEP_Y)] {
                let v = cpu.read16(event + at).wrapping_add(cpu.read8(event + step) as i8 as i16 as u16);
                cpu.write16(event + at, v);
            }
            let direction = cpu.read16(event + E_DIRECTION).wrapping_add(cpu.read16(event + E_TURN));
            cpu.write16(event + E_DIRECTION, direction);
            if cpu.read8(event + E_MOTION) != 0 {
                nested!(cpu, 0x0608_4d68, row_retype(cpu, event));
            }
            if cpu.read8(event + E_WAIT) != 0 {
                break;
            }
        }
    }
    let wait = cpu.read8(event + E_WAIT).wrapping_sub(1);
    cpu.write8(event + E_WAIT, wait);
    0
}

/// One of the row made from the description `d`, by the event's kind.
fn row_make(cpu: &mut Cpu, event: u32, d: u32) {
    let kind = cpu.read8(event + E_KIND);
    let n = cpu.read8(event + E_SCRIPT) as u32;
    let script = |cpu: &mut Cpu, table: u32| cpu.read32(table + 4 * n);
    let byte = |cpu: &mut Cpu, at: u32| cpu.read8(event + at) as i8 as i16 as u16;
    let kind_word = |base: u8| (kind as i8 as i16).wrapping_sub(base as i16) as u16;
    match kind {
        0..=2 => {
            cpu.write16(d + G_KIND, kind as i8 as i16 as u16);
            let s = script(cpu, GROUND_SCRIPTS);
            nested!(cpu, 0x0605_a780, ground::ground_set(cpu, d, s));
        }
        3..=6 => {
            cpu.write16(d + G_KIND, kind_word(3));
            let (s, aim) = (script(cpu, GUN_SCRIPTS), script(cpu, GUN_AIMS));
            nested!(cpu, 0x0605_b168, ground::gun_set(cpu, d, s, aim));
        }
        7..=9 => {
            cpu.write16(d + G_KIND, kind_word(7));
            let v = byte(cpu, E_1E) << 8;
            cpu.write16(d + G_34, v);
            let v = byte(cpu, E_1F);
            cpu.write16(d + G_32, v);
            let at = cpu.read8(event + E_1C) as i8 as i32 as u32;
            let aim = cpu.read32(WOUNDED_AIMS.wrapping_add(4 * at));
            let s = script(cpu, WOUNDED_SCRIPTS);
            nested!(cpu, 0x0605_bbac, ground::wounded_set(cpu, d, s, aim));
        }
        10..=13 => {
            cpu.write16(d + G_KIND, kind_word(10));
            let v = cpu.read8(event + E_1A) as u16;
            cpu.write16(d + G_32, v);
            for (to, from) in [(G_34, E_1C), (G_36, E_1D), (G_38, E_1E), (G_3A, E_1F)] {
                let v = byte(cpu, from);
                cpu.write16(d + to, v);
            }
            let s = script(cpu, PLATFORM_SCRIPTS);
            nested!(cpu, 0x0605_c86c, ground::platform_set(cpu, d, s));
            if cpu.read8(event + E_1A) != 0 {
                cpu.write8(event + E_1A, 5);
            }
        }
        14 | 15 => {
            cpu.write16(d + G_KIND, kind_word(14));
            let v = cpu.read8(event + E_TYPE) as u16;
            cpu.write16(d + G_32, v);
            let v = cpu.read8(event + E_1A) as u16;
            cpu.write16(d + G_34, v);
            nested!(cpu, 0x0605_c464, ground::launcher_set(cpu, d, 0));
        }
        20 => {
            let s = script(cpu, GROUND_SCRIPTS);
            nested!(cpu, 0x0605_da58, ground::burster_set(cpu, d, s));
        }
        21 => {
            let s = script(cpu, GROUND_SCRIPTS);
            nested!(cpu, 0x0605_e030, ground::hatch_set(cpu, d, s));
        }
        22 => platform_pair(cpu, event, d),
        23 => scenery_end(cpu, d),
        128 => {
            let s = script(cpu, GROUND_SCRIPTS);
            let o = nested!(cpu, 0x0605_e030, ground::hatch_set(cpu, d, s));
            if o != 0 {
                cpu.write32(o + O_BEHAVIOUR, HATCH_BEHAVIOUR);
            }
        }
        129 => {
            cpu.write16(d + G_KIND, 2);
            let s = script(cpu, GROUND_SCRIPTS);
            let o = nested!(cpu, 0x0605_a780, ground::ground_set(cpu, d, s));
            if o != 0 {
                cpu.write32(o + O_BEHAVIOUR, GROUND_BEHAVIOUR);
            }
        }
        _ => {}
    }
}

/// Kind 22: the pair's maker (0x060899f4, given `d`) and, if it made something (`r`), two
/// platforms (the first platform script) of kind 1 and no direction, their type the event's
/// +0x18 or'ed with 4, then 8, and the settings in the description's words +0x34-+0x3a; each
/// takes the behaviour 0x06089b58, its x (-0x340000 or 0x340000), a side (-1 or 1), `r` (+0x6c)
/// and has the bit 0x80000 of its flags cleared.
fn platform_pair(cpu: &mut Cpu, event: u32, d: u32) {
    let r = cpu.call(PAIR_MAKER, &[d]);
    if r == 0 {
        return;
    }
    cpu.write16(d + G_KIND, 1);
    cpu.write16(d + G_32, 0);
    cpu.write16(d + G_DIRECTION, 0);
    for (to, from) in [(G_34, E_1C), (G_36, E_1D), (G_38, E_1E), (G_3A, E_1F)] {
        let v = cpu.read8(event + from) as i8 as i16 as u16;
        cpu.write16(d + to, v);
    }
    for (bit, x, side) in [(4u8, 0xffcc_0000u32, 0xffu8), (8, 0x34_0000, 1)] {
        cpu.write16(d + G_TYPE, bit as u16);
        let b = cpu.read8(event + E_STEP_X) | bit;
        cpu.write16(d + G_TYPE, b as i8 as i16 as u16);
        let script = cpu.read32(PLATFORM_SCRIPTS);
        let o = nested!(cpu, 0x0605_c86c, ground::platform_set(cpu, d, script));
        if o != 0 {
            cpu.write32(o + O_BEHAVIOUR, PAIR_BEHAVIOUR);
            cpu.write32(o + O_POSITION, x);
            cpu.write8(o + 0x88, side);
            cpu.write32(o + 0x6c, r);
            let flags = cpu.read32(o + O_FLAGS) & NOT_80000;
            cpu.write32(o + O_FLAGS, flags);
        }
    }
}

/// Kind 23: the end's scenery: three words cleared (SCENERY), then three rounds (type 0, direction
/// 0-2, down the screen; SCENERY_ROUND) and three plates (types 3-5, each at its place;
/// SCENERY_PLATE), their objects kept in the table from SCENERY + 0x10.
fn scenery_end(cpu: &mut Cpu, d: u32) {
    for k in 0..3 {
        cpu.write32(SCENERY + 4 * k, 0);
    }
    cpu.write32(d + G_PLACE, 0);
    cpu.write16(d + G_TYPE, 0);
    for (i, y) in [0xfe93_0000u32, 0xfefb_0000, 0xff63_0000].into_iter().enumerate() {
        cpu.write32(d + G_PLACE + 4, y);
        cpu.write16(d + G_DIRECTION, i as u16);
        let o = cpu.call(SCENERY_ROUND, &[d]);
        cpu.write32(SCENERY + 0x10 + 4 * i as u32, o);
    }
    for (i, (x, y)) in [(0x8_0000u32, 0xfe60_0000u32), (0xffa8_0000, 0xfec8_0000), (0x8_0000, 0xff30_0000)].into_iter().enumerate() {
        cpu.write32(d + G_PLACE, x);
        cpu.write32(d + G_PLACE + 4, y);
        cpu.write16(d + G_TYPE, 3 + i as u16);
        let o = cpu.call(SCENERY_PLATE, &[d]);
        cpu.write32(SCENERY + 0x1c + 4 * i as u32, o);
    }
}

// ---- the pieces ----------------------------------------------------------------------------------

/// A piece (a numbered object of list 2, 0xbc bytes: drawn as a model of kind 0x40, power 1200,
/// moving down with the ground): the model it shows by the enemy row's type (+0x8a, a word), its
/// hit flags (+0x56: 0x200 a hit; +0x7e, the sprite's: bit 0 shown hit, 1 and 2 cleared in the
/// draw).
const PIECE_SIZE: u32 = 0xbc;
const PIECE_NUMBERS: u32 = 2;
const PIECE_STEP: u32 = 0x0608_548c;
const PIECE_DRAW: u32 = 0x0608_54fc;
const PIECE_FLAGS: u32 = 0xc0_0000;
const PIECE_BOXES: u32 = 0x060a_c0ec;
const PIECE_KIND: u8 = 0x40;
const PIECE_POWER: u32 = 1200;
const PIECE_MODEL: u32 = 0x8a;
const PIECE_HIT_FLAGS: u32 = 0x7e;
const PIECE_OFF: i32 = 0x90_0000;
const PIECE_KILL_POINTS: u32 = 30;
const DIFFICULTY: u32 = 0x0600_ed84;
const BD_NUMBER: u32 = 0x1e;
const BD_GROWTH: u32 = 0x2c;
const BD_ACCEL: u32 = 0x2e;
const BD_SPEED: u32 = 0x30;
const BD_ACCEL_CHANGE: u32 = 0x38;
const BD_KIND: u32 = 0x3a;
const BLAST_SOUND: u32 = 16;

/// A piece blown apart at its place (x, y): on the harder difficulties (past 2) a burst (growth
/// 0x200, acceleration 0x60 changing by -8, speed (0, 0x8000), kind 0) and a glow (0xc000); on the
/// easier a burning effect; then the sound 16.
pub fn piece_blast(cpu: &mut Cpu, o: u32) {
    let f = cpu.stack();
    let number = cpu.read8(o + O_NUMBER);
    cpu.write8(f + BD_NUMBER, number);
    let x = cpu.read32(o + O_POSITION);
    cpu.write32(f, x);
    let y = cpu.read32(o + O_POSITION + 4);
    cpu.write32(f + 4, y);
    cpu.write32(f + 8, 0);
    if cpu.read8(DIFFICULTY) as i8 <= 2 {
        nested!(cpu, 0x0605_7cb0, explosions::burning_set(cpu, f));
    } else {
        cpu.write16(f + BD_GROWTH, 0x200);
        cpu.write16(f + BD_ACCEL, 0x60);
        cpu.write16(f + BD_ACCEL_CHANGE, 0xfff8);
        cpu.write16(f + BD_KIND, 0);
        cpu.write32(f + BD_SPEED, 0);
        cpu.write32(f + BD_SPEED + 4, 0x8000);
        nested!(cpu, 0x0605_5310, bullets::burst_set(cpu, f));
        cpu.write32(f + BD_GROWTH, 0xc000);
        nested!(cpu, 0x0605_8f34, explosions::glow_set(cpu, f));
    }
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, BLAST_SOUND));
}

/// The piece (behaviour 0x0608548c): past 0x900000 down it ends (mode 0x70); with its power gone
/// it is scored for what hit it (30 points), blown apart and ends; else it moves down with the
/// ground and takes a hit (bit 0 of +0x7e) when the hit flag 0x200 is set.
pub fn piece_step(cpu: &mut Cpu, o: u32) {
    let y = cpu.read32(o + O_POSITION + 4) as i32;
    if y <= PIECE_OFF {
        if cpu.read32(o + P_POWER) as i32 <= 0 {
            nested!(cpu, 0x0605_61ac, hits::score_hit_by(cpu, o, PIECE_KILL_POINTS));
            nested!(cpu, 0x0608_53ec, piece_blast(cpu, o));
        } else {
            let at = cpu.read32(super::globals::GROUND_DY_AT);
            let dy = cpu.read32(at);
            cpu.write32(o + O_POSITION + 4, (y as u32).wrapping_sub(dy));
            if cpu.read16(o + D_HIT_FLAGS) as i16 as i32 & 0x200 != 0 {
                let f = cpu.read16(o + PIECE_HIT_FLAGS) | 1;
                cpu.write16(o + PIECE_HIT_FLAGS, f);
            }
            return;
        }
    }
    cpu.write8(o + O_MODE, MODE_DONE);
}

/// The piece's draw (its sprite `s`): the hit mark in its flags (bit 1 cleared with bit 0 of
/// them; bit 0 on sets bit 1), the matrix pushed, moved to its place (a little in front), its model
/// of kind 0x40 by its type, the shown-hit flag choosing which of the pair; the matrix popped.
pub fn piece_draw(cpu: &mut Cpu, s: u32) {
    let at = s + 0x6e;
    let flags = cpu.read16(at) as i16 as i32 as u32;
    let mut flags = flags;
    if flags & 2 != 0 {
        flags &= 0xffff_fffc;
        cpu.write16(at, flags as u16);
        flags = cpu.read16(at) as i16 as i32 as u32;
    }
    if flags & 1 != 0 {
        cpu.write16(at, (flags | 2) as u16);
    }
    cpu.call(SL_PUSH_MATRIX, &[]);
    let z = cpu.read32(s + 12).wrapping_add(0x10_0000);
    let (x, y) = (cpu.read32(s + 4), cpu.read32(s + 8));
    cpu.call(SL_TRANSLATE, &[x, y, z]);
    let file = cpu.read32(KINDS + K_SIZE * PIECE_KIND as u32);
    let models = cpu.read32(file + KF_MODELS);
    let n = cpu.read16(s + 0x7a) as i16 as i32 as u32;
    let pair = cpu.read32(models + 4 * n);
    let hit = (cpu.read16(at) as i16 as i32 as u32) & 1;
    let model = cpu.read32(pair + 4 * hit);
    cpu.call(SL_PUT_POLYGON, &[model]);
    cpu.call(SL_POP_MATRIX, &[]);
}

/// A piece made, and what the pieces made from a row share: a free number from 2, the object (list 2,
/// 0xbc bytes), its sprite and boxes set up, its fields cleared, its behaviour, draw, flags
/// 0xc00000, kind 0x40, boxes, hit by player type 1, the row's type for its model (+0x55 and
/// +0x8a) and its power; the object, or 0.
fn new_piece(cpu: &mut Cpu, e: u32, behaviour: u32, power: u32) -> u32 {
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, PIECE_NUMBERS)) as i8;
    if n == -1 {
        return 0;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, PIECE_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, PART_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + O_SCRIPT));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + O_BEHAVIOUR, behaviour);
    cpu.write8(o + O_NUMBER, n as u8);
    let at = cpu.read8(o + O_NUMBER) as i8 as i32 as u32;
    cpu.write32(NUMBERED.wrapping_add(at << 2), o);
    cpu.write32(o + O_DRAW, PIECE_DRAW);
    cpu.write32(o + O_FLAGS, PIECE_FLAGS);
    cpu.write8(o + O_KIND, PIECE_KIND);
    cpu.write8(o + O_FRAME, 0);
    cpu.write32(o + O_SCRIPT, PIECE_BOXES);
    cpu.write8(o + P_54, 1);
    let t = cpu.read8(e + E_TYPE);
    cpu.write8(o + P_55, t);
    cpu.write32(o + P_POWER, power);
    cpu.write16(o + PIECE_MODEL, t as u16);
    o
}

/// A piece made from the enemy row's event `e` (its place +4, +6) with power 1200, at the row's
/// place (x, y, z 0). Gives 1 either way.
pub fn piece_make(cpu: &mut Cpu, e: u32) -> u32 {
    let o = new_piece(cpu, e, PIECE_STEP, PIECE_POWER);
    if o != 0 {
        let x = (cpu.read16(e + E_PLACE) as i16 as i32 as u32) << 16;
        cpu.write32(o + O_POSITION, x);
        let y = (cpu.read16(e + E_PLACE + 2) as i16 as i32 as u32) << 16;
        cpu.write32(o + O_POSITION + 4, y);
        cpu.write32(o + O_POSITION + 8, 0);
    }
    1
}

/// A sliding piece (behaviour 0x06085698): a piece that, with its side (+0x88: 1 goes right, -1
/// left), moves 0x18000 a frame across, ending past 0xe00000 out.
const SIDE: u32 = 0x88;
const SLIDER_SPEED: u32 = 0x1_8000;
const SLIDER_OUT: i32 = 0xe0_0000;

pub fn slider_step(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_548c, piece_step(cpu, o));
    if cpu.read8(o + O_MODE) == MODE_DONE {
        return;
    }
    let side = cpu.read8(o + SIDE) as i8 as i32;
    let x = cpu.read32(o + O_POSITION) as i32;
    if (side > 0 && x > SLIDER_OUT) || (side < 0 && x < -SLIDER_OUT) {
        cpu.write8(o + O_MODE, MODE_DONE);
        return;
    }
    cpu.write32(o + O_POSITION, x.wrapping_add(side.wrapping_mul(SLIDER_SPEED as i32)) as u32);
}

/// The row of sliding pieces (a timeline event, called each frame until it gives 1): its depth
/// (+0x1c) brought down with the ground (done past 0x800000), a count (+0x16, a word) on each
/// frame; every 60th a sliding piece (power 1000) at its place (x, its depth for y), its side
/// toward the middle.
pub fn slider_row(cpu: &mut Cpu, e: u32) -> u32 {
    let at = cpu.read32(super::globals::GROUND_DY_AT);
    let dy = cpu.read32(at);
    let depth = cpu.read32(e + 0x1c).wrapping_sub(dy);
    cpu.write32(e + 0x1c, depth);
    if depth as i32 > 0x80_0000 {
        return 1;
    }
    let count = cpu.read16(e + 0x16).wrapping_add(1);
    cpu.write16(e + 0x16, count);
    let r = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, count as i16 as i32 as u32, 60));
    if r != 0 {
        return 0;
    }
    let o = new_piece(cpu, e, 0x0608_5698, 1000);
    if o != 0 {
        let x = (cpu.read16(e + E_PLACE) as i16 as i32 as u32) << 16;
        cpu.write32(o + O_POSITION, x);
        cpu.write32(o + O_POSITION + 4, depth);
        cpu.write32(o + O_POSITION + 8, r);
        cpu.write8(o + SIDE, if x as i32 > 0 { 0xff } else { 1 });
    }
    0
}

// ---- the gate ------------------------------------------------------------------------------------

/// A gate (an object of list 2): two jaws (+0x114, +0x118; objects, jaw_step's, which hold it at
/// +0x6c) and six points (+0xcc, 12 bytes each: calc_point's outputs, which it takes from the
/// draw) whose offsets from it it keeps in words from +0x7e (x) and +0x80 (y), 8 bytes apart. Its
/// state (+0x6d) shares a hit between the jaws (0: either to both; 1: the first's to the second;
/// 2: the second's to the first); its count (+0x6c) grows each frame. Past 0x900000 down both
/// jaws, it and they end.
const GATE_JAWS: [u32; 2] = [0x114, 0x118];
const GATE_POINTS: u32 = 0xcc;
const GATE_OFFSETS: u32 = 0x7e;
const GATE_TURN: u32 = 0x6e;
const GATE_TURN_BY: u32 = 0x70;
const GATE_COUNT: u32 = 0x6c;
const GATE_OFF: i32 = 0x90_0000;
const HIT_WORD: u32 = 0x56;
const HIT_BIT: u16 = 0x200;

fn hit_bit(cpu: &mut Cpu, o: u32) -> bool {
    cpu.read16(o + HIT_WORD) & HIT_BIT != 0
}

fn set_hit(cpu: &mut Cpu, o: u32) {
    let h = cpu.read16(o + HIT_WORD) | HIT_BIT;
    cpu.write16(o + HIT_WORD, h);
}

pub fn gate_step(cpu: &mut Cpu, o: u32) {
    let a = cpu.read32(o + GATE_JAWS[0]);
    let b = cpu.read32(o + GATE_JAWS[1]);
    if cpu.read32(a + O_POSITION + 4) as i32 > GATE_OFF && cpu.read32(b + O_POSITION + 4) as i32 > GATE_OFF {
        for x in [o, a, b] {
            cpu.write8(x + O_MODE, MODE_DONE);
        }
        return;
    }
    let at = cpu.read32(super::globals::GROUND_DY_AT);
    let dy = cpu.read32(at);
    let y = cpu.read32(o + O_POSITION + 4).wrapping_sub(dy);
    cpu.write32(o + O_POSITION + 4, y);
    if cpu.read8(o + GATE_COUNT) != 0 {
        let flags = cpu.read32(o + O_FLAGS) & !F_HIDDEN;
        cpu.write32(o + O_FLAGS, flags);
        for i in 0..6u32 {
            for (k, from) in [(0, O_POSITION), (4, O_POSITION + 4)] {
                let mut d = cpu.read32(o + GATE_POINTS + 12 * i + k).wrapping_sub(cpu.read32(o + from)) as i32;
                if d < 0 {
                    d = d.wrapping_add(0xffff);
                }
                cpu.write16(o + GATE_OFFSETS + 8 * i + k / 2, (d >> 16) as u16);
            }
        }
    }
    let turn = cpu.read16(o + GATE_TURN).wrapping_add(cpu.read16(o + GATE_TURN_BY));
    cpu.write16(o + GATE_TURN, turn);
    match cpu.read8(o + O_STATE) {
        0 => {
            if hit_bit(cpu, a) {
                set_hit(cpu, b);
            }
            if hit_bit(cpu, b) {
                set_hit(cpu, a);
            }
        }
        1 | 2 => {
            let (from, to) = if cpu.read8(o + O_STATE) == 1 { (a, b) } else { (b, a) };
            if hit_bit(cpu, from) {
                set_hit(cpu, to);
            }
            let h = cpu.read16(from + HIT_WORD) & !HIT_BIT;
            cpu.write16(from + HIT_WORD, h);
        }
        _ => {}
    }
    let count = cpu.read8(o + GATE_COUNT).wrapping_add(1);
    cpu.write8(o + GATE_COUNT, count);
}

/// The gate's draw (its sprite `s`): the matrix pushed, moved to its place and turned (z); moved
/// down by +0x74; moved to the middle of its jaws' offsets (+0xac and +0xb0) and scaled up to
/// them (y by their gap over 0xd0), six points (the mid chain, each 0x200000 further down, shaken
/// by the frame) taken, turned by a random angle, a model of kind 0x11 (the group by LOOPS mod
/// 3) put and the matrix popped; then each jaw: moved to its offset, turned by it over 32 less
/// (or more, the second, scaled upside down) the frame, its point (+0xb4, +0xc0) taken and the
/// jaw model put.
const JAW_LOW: u32 = 0xac;
const JAW_HIGH: u32 = 0xb0;
const JAW_LOW_AT: u32 = 0xb4;
const JAW_HIGH_AT: u32 = 0xc0;
const GATE_KIND: u32 = 0x11;
const SL_CALC_POINT: u32 = 0x0606_db2c;

pub fn gate_draw(cpu: &mut Cpu, s: u32) {
    let o = s.wrapping_sub(0x10);
    cpu.call(SL_PUSH_MATRIX, &[]);
    let p = [0, 1, 2].map(|i| cpu.read32(o + O_POSITION + 4 * i));
    cpu.call(SL_TRANSLATE, &p);
    let turn = cpu.read16(s + 0x5e) as i16 as i32 as u32;
    cpu.call(SL_ROT_Z, &[turn]);
    cpu.call(SL_PUSH_MATRIX, &[]);
    let down = cpu.read32(s + 100);
    cpu.call(SL_TRANSLATE, &[0, down, 0]);
    cpu.call(SL_PUSH_MATRIX, &[]);
    let (low, high) = (cpu.read32(o + JAW_LOW), cpu.read32(o + JAW_HIGH));
    let gap = high.wrapping_sub(low) as i32;
    let middle = (gap.wrapping_add((gap < 0) as i32) >> 1).wrapping_add(low as i32);
    cpu.call(SL_TRANSLATE, &[0, middle as u32, 0]);
    let scale = nested!(cpu, 0x0600_6a78, math::quotient(cpu, gap as u32, 0xd0));
    cpu.call(SL_SCALE, &[0x1_0000, scale, 0x1_0000]);
    for i in 0..6u32 {
        let shake = (cpu.read32(GAME_FRAMES) & 3) << 19;
        let y = shake.wrapping_add(0xffa4_0000).wrapping_add(i * 0x20_0000);
        cpu.call(SL_CALC_POINT, &[0, y, 0, o + GATE_POINTS + 12 * i]);
    }
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.call(SL_ROT_Y, &[r as u16 as i16 as i32 as u32]);
    let loops = cpu.read32(LOOPS);
    let n = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, loops, 3));
    let file = cpu.read32(KINDS + K_SIZE * GATE_KIND);
    let models = cpu.read32(file + KF_MODELS);
    let group = cpu.read32(models.wrapping_add(4 * n).wrapping_add(4));
    let model = cpu.read32(group);
    cpu.call(SL_PUT_POLYGON, &[model]);
    cpu.call(SL_POP_MATRIX, &[]);
    let frames = cpu.read32(GAME_FRAMES);
    let jaw_model = |cpu: &mut Cpu| {
        let file = cpu.read32(KINDS + K_SIZE * GATE_KIND);
        let models = cpu.read32(file + KF_MODELS);
        let first = cpu.read32(models);
        cpu.read32(first)
    };
    cpu.call(SL_PUSH_MATRIX, &[]);
    cpu.call(SL_TRANSLATE, &[0, low, 0]);
    let turn = (((low as i32).wrapping_add(if (low as i32) < 0 { 31 } else { 0 })) >> 5).wrapping_sub((frames << 8) as i32);
    cpu.call(SL_ROT_Y, &[turn as i16 as i32 as u32]);
    cpu.call(SL_CALC_POINT, &[0, 0, 0, o + JAW_LOW_AT]);
    let m = jaw_model(cpu);
    cpu.call(SL_PUT_POLYGON, &[m]);
    cpu.call(SL_POP_MATRIX, &[]);
    cpu.call(SL_PUSH_MATRIX, &[]);
    cpu.call(SL_SCALE, &[0x1_0000, 0xffff_0000, 0x1_0000]);
    cpu.call(SL_TRANSLATE, &[0, high.wrapping_neg(), 0]);
    let turn = (((high as i32).wrapping_add(if (high as i32) < 0 { 31 } else { 0 })) >> 5).wrapping_add((frames << 8) as i32);
    cpu.call(SL_ROT_Y, &[turn as i16 as i32 as u32]);
    cpu.call(SL_CALC_POINT, &[0, 0, 0, o + JAW_HIGH_AT]);
    let m = jaw_model(cpu);
    cpu.call(SL_PUT_POLYGON, &[m]);
    for _ in 0..3 {
        cpu.call(SL_POP_MATRIX, &[]);
    }
}

/// A jaw (behaviour 0x06085bd8; the gate is at +0x6c; the first has +0x70 negative and is the low
/// jaw): it holds its speed (+0x74) toward 0x28000 by 0xc00 (toward -0x8000 by 0x8000 and its hit
/// count +0x78 set to 7 while hit, the count run down otherwise); a low jaw, once the count is
/// out and it not hit, closes by that speed to the gate's limit (-the gate's +0x78) and sounds
/// the first time it gets there (the gate's +0x11c); hit or counting it is pushed back 0xc000;
/// its place is the gate's point (+0xb4, +0xb8) and the gate's offset +0xac is held to its +0xb0
/// less 0x200000. The high jaw the same the other way (offset +0xb0, flag +0x11d, place
/// +0xc0, +0xc4, held to +0xac plus 0x200000).
const JAW_GATE: u32 = 0x6c;
const JAW_SPEED: u32 = 0x74;
const JAW_COUNT: u32 = 0x78;
const JAW_SIDE: u32 = 0x70;
const GATE_LIMIT: u32 = 0x78;
const GATE_CLOSED: [u32; 2] = [0x11c, 0x11d];
const JAW_SOUND: u32 = 79;

pub fn jaw_step(cpu: &mut Cpu, o: u32) {
    let gate = cpu.read32(o + JAW_GATE);
    let count = cpu.read16(o + JAW_COUNT) as i16;
    if count != 0 {
        cpu.write16(o + JAW_COUNT, (count - 1) as u16);
    }
    let (to, step) = if !hit_bit(cpu, o) {
        (0x2_8000, 0xc00)
    } else {
        cpu.write16(o + JAW_COUNT, 7);
        (0xffff_8000, 0x8000)
    };
    let speed = cpu.read32(o + JAW_SPEED);
    let speed = nested!(cpu, 0x0604_6168, actors::approach(cpu, speed, to, step));
    cpu.write32(o + JAW_SPEED, speed);
    let low = (cpu.read8(o + JAW_SIDE) as i8) < 0;
    let (mine, other, closed, place) = if low {
        (JAW_LOW, JAW_HIGH, GATE_CLOSED[0], JAW_LOW_AT)
    } else {
        (JAW_HIGH, JAW_LOW, GATE_CLOSED[1], JAW_HIGH_AT)
    };
    if cpu.read16(o + JAW_COUNT) != 0 || hit_bit(cpu, o) {
        let moved = cpu.read32(gate + mine);
        let moved = if low { moved.wrapping_add(0xc000) } else { moved.wrapping_sub(0xc000) };
        cpu.write32(gate + mine, moved);
    } else if speed as i32 > 0 {
        let limit = cpu.read32(gate + GATE_LIMIT);
        let to = if low { limit.wrapping_neg() } else { limit };
        let at = cpu.read32(gate + mine);
        let at = nested!(cpu, 0x0604_6168, actors::approach(cpu, at, to, speed));
        cpu.write32(gate + mine, at);
        if at == to {
            if cpu.read8(gate + closed) == 0 {
                cpu.write8(gate + closed, 1);
                nested!(cpu, 0x0600_50d4, sound::se_set(cpu, JAW_SOUND));
            }
        } else {
            cpu.write8(gate + closed, 0);
        }
    }
    let x = cpu.read32(gate + place);
    cpu.write32(o + O_POSITION, x);
    let y = cpu.read32(gate + place + 4);
    cpu.write32(o + O_POSITION + 4, y);
    let held = cpu.read32(gate + other);
    let held = if low { held.wrapping_sub(0x20_0000) } else { held.wrapping_add(0x20_0000) };
    let mine_at = cpu.read32(gate + mine) as i32;
    if low {
        if mine_at > held as i32 {
            cpu.write32(gate + mine, held);
        }
    } else if (mine_at) < held as i32 {
        cpu.write32(gate + mine, held);
    }
}

/// A jaw made for the gate `gate` (list 2, 0x7c bytes, behaviour jaw_step, y -0xc00000, power 10,
/// its boxes 0x060ac12c): `side` is -1 for the low jaw (the gate's +0x114) and 1 for the high
/// (+0x118). 1 if made, else 0.
const JAW_SIZE: u32 = 0x7c;
const JAW_STEP: u32 = 0x0608_5bd8;
const JAW_FLAGS: u32 = 0x8004_0000;
const JAW_Y: u32 = 0xff40_0000;
const JAW_BOXES: u32 = 0x060a_c12c;
const JAW_POWER: u32 = 10;

pub fn jaw_make(cpu: &mut Cpu, gate: u32, side: u32) -> u32 {
    let side = side as i8;
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, PIECE_NUMBERS)) as i8;
    if n == -1 {
        return 0;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, JAW_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, PART_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + O_SCRIPT));
    cpu.write32(o + O_BEHAVIOUR, JAW_STEP);
    cpu.write32(o + O_FLAGS, JAW_FLAGS);
    cpu.write8(o + O_NUMBER, n as u8);
    let at = cpu.read8(o + O_NUMBER) as i8 as i32 as u32;
    cpu.write32(NUMBERED.wrapping_add(at << 2), o);
    cpu.write32(o + O_POSITION + 4, JAW_Y);
    cpu.write32(o + O_POSITION + 8, 0);
    cpu.write8(o + P_54, 1);
    cpu.write32(o + O_SCRIPT, JAW_BOXES);
    cpu.write32(o + P_POWER, JAW_POWER);
    cpu.write32(o + JAW_GATE, gate);
    cpu.write8(o + JAW_SIDE, side as u8);
    cpu.write32(o + JAW_SPEED, 0);
    cpu.write16(o + JAW_COUNT, 0);
    let slot = if (cpu.read8(o + JAW_SIDE) as i8) < 0 { GATE_JAWS[0] } else { GATE_JAWS[1] };
    cpu.write32(gate + slot, o);
    1
}

/// A gate made from the row's event `e` (its place +4, +6; its state +1; its turn +0x14, speed of
/// turn +0x16; +0x1c the jaws' reach, +0x1e a drop): the object (list 2, 0x120 bytes, an actor
/// with boxes 0x060ac0fc copied into it), the jaws at -reach and reach, the closed marks set, and
/// the two jaws made; gives 1 and ends the gate if a jaw cannot be.
const GATE_SIZE: u32 = 0x120;
const GATE_STEP: u32 = 0x0608_5850;
const GATE_DRAW: u32 = 0x0608_59c4;
const GATE_FLAGS: u32 = 0xc8_0000;
const GATE_BOXES: u32 = 0x060a_c0fc;
const GATE_BOXES_AT: u32 = 0x7c;
const GATE_STATE: u32 = 0x6d;

pub fn gate_make(cpu: &mut Cpu, e: u32) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, GATE_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, 0x0604_d780));
    if o == 0 {
        return 1;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + O_SCRIPT));
    cpu.write32(o + O_BEHAVIOUR, GATE_STEP);
    cpu.write32(o + O_FLAGS, GATE_FLAGS);
    cpu.write32(o + O_DRAW, GATE_DRAW);
    cpu.write32(o + O_SCRIPT, o + GATE_BOXES_AT);
    cpu.write8(o + P_54, 1);
    cpu.write8(o + P_55, 5);
    let x = (cpu.read16(e + E_PLACE) as i16 as i32 as u32) << 16;
    cpu.write32(o + O_POSITION, x);
    let y = (cpu.read16(e + E_PLACE + 2) as i16 as i32 as u32) << 16;
    cpu.write32(o + O_POSITION + 4, y);
    cpu.write32(o + O_POSITION + 8, 0);
    nested!(cpu, 0x0605_6440, hits::boxes_copy(cpu, o + GATE_BOXES_AT, GATE_BOXES));
    cpu.write8(o + GATE_COUNT, 0);
    let state = cpu.read8(e + E_KIND);
    cpu.write8(o + GATE_STATE, state);
    let reach = cpu.read16(e + 0x1c) as i16 as i32;
    cpu.write32(o + JAW_LOW, (reach.wrapping_neg() << 16) as u32);
    cpu.write32(o + JAW_HIGH, (reach << 16) as u32);
    let turn = cpu.read16(e + E_DIRECTION);
    cpu.write16(o + GATE_TURN, turn);
    let turn_by = cpu.read16(e + E_TURN);
    cpu.write16(o + GATE_TURN_BY, turn_by);
    let drop = (cpu.read16(e + 0x1e) as i16 as i32) << 16;
    cpu.write32(o + 0x74, drop as u32);
    cpu.write32(o + GATE_LIMIT, (reach << 16) as u32);
    cpu.write8(o + GATE_CLOSED[0], 1);
    cpu.write8(o + GATE_CLOSED[1], 1);
    let low = nested!(cpu, 0x0608_5d9c, jaw_make(cpu, o, 0xffff_ffff)) as u8;
    if low == 0 || nested!(cpu, 0x0608_5d9c, jaw_make(cpu, o, 1)) as u8 == 0 {
        cpu.write8(o + O_MODE, MODE_DONE);
    }
    1
}

// ---- the swinging walls --------------------------------------------------------------------------

/// A wall (an object turned about z (+0x30) along a script, with 12-byte records of its box
/// points from +0x3c (x at +0, y at +4; the count at +0xa0)): its side (+0x9c: -1 or 1), the
/// point it holds to (+0xa1, +0xa4 its y), the script's place (+0xaa), wait (+0xac), turn (+0xae)
/// and turn's step (+0xb0), its script number (+0xa8), the turn's spring (+0xb2, +0xb4).
const W_SIDE: u32 = 0x9c;
const W_COUNT: u32 = 0xa0;
const W_HELD: u32 = 0xa1;
const W_HELD_Y: u32 = 0xa4;
const W_SCRIPT_NO: u32 = 0xa8;
const W_PLACE: u32 = 0xaa;
const W_WAIT: u32 = 0xac;
const W_TURN: u32 = 0xae;
const W_TURN_STEP: u32 = 0xb0;
const W_SPRING: u32 = 0xb2;
const W_SPRING_SPEED: u32 = 0xb4;
const W_POINTS: u32 = 0x3c;
const W_REACH: u32 = 0x68_0000;
const W_SCRIPTS: u32 = 0x060a_c1dc;
const WALL_SOUND: u32 = 25;

/// The wall's turn script `s` (a start turn, then (frames, turn) pairs; 0xff ends it; a pair whose
/// first byte has bit 0 set is skipped, 0x40 sending it back to the second): at the start of each
/// pair the turn to reach and its step a frame; the turn (+0xae) stepped each frame meanwhile; at
/// the end the wall ends (mode 0x70).
pub fn wall_script(cpu: &mut Cpu, o: u32, s: u32) {
    let place = cpu.read16(o + W_PLACE) as u32;
    if cpu.read8(s + place) == 0xff {
        return;
    }
    if cpu.read16(o + W_WAIT) as i16 != 0 {
        let turn = cpu.read16(o + W_TURN).wrapping_add(cpu.read16(o + W_TURN_STEP));
        cpu.write16(o + W_TURN, turn);
    } else {
        if place == 0 {
            let a = (cpu.read8(s) as u16) << 8;
            cpu.write16(o + W_TURN, a);
            let p = cpu.read16(o + W_PLACE).wrapping_add(1);
            cpu.write16(o + W_PLACE, p);
        } else {
            let a = (cpu.read8(s + place + 1) as u16) << 8;
            cpu.write16(o + W_TURN, a);
            let p = cpu.read16(o + W_PLACE).wrapping_add(2);
            cpu.write16(o + W_PLACE, p);
        }
        let at = s + cpu.read16(o + W_PLACE) as u32;
        let mut b = cpu.read8(at);
        if b == 0xff {
            cpu.write8(o + O_MODE, MODE_DONE);
        } else {
            while b & 1 != 0 {
                let p = cpu.read16(o + W_PLACE);
                if cpu.read8(s + p as u32) & 0x40 == 0 {
                    cpu.write16(o + W_PLACE, p.wrapping_add(1));
                } else {
                    cpu.write16(o + W_PLACE, 1);
                }
                let at = s + cpu.read16(o + W_PLACE) as u32;
                b = cpu.read8(at);
            }
            let at = s + cpu.read16(o + W_PLACE) as u32;
            let frames = cpu.read8(at);
            cpu.write16(o + W_WAIT, frames as u16);
            let to = (cpu.read8(at + 1) as u32) << 8;
            let from = cpu.read16(o + W_TURN) as u32;
            let wait = cpu.read16(o + W_WAIT) as i16 as i32 as u32;
            let d = nested!(cpu, 0x0605_6580, aim::angle_between(cpu, from, to)) as i16 as i32 as u32;
            let step = nested!(cpu, 0x0600_6a78, math::quotient(cpu, d, wait));
            cpu.write16(o + W_TURN_STEP, step as u16);
        }
    }
    let wait = cpu.read16(o + W_WAIT).wrapping_sub(1);
    cpu.write16(o + W_WAIT, wait);
}

/// The wall held at the edge: of its box points the one furthest toward its side (-1: the least
/// x; 1: the most) is found (the first on a tie), the wall's x moved so that point is 0x680000
/// out on its side; its y kept to that point's y (+0xa4 is the point's y when it was found); a
/// new furthest point sounds (25) if it is near the middle, and the spring's turn and speed
/// (+0xb2, +0xb4) are cut to an eighth.
pub fn wall_hold(cpu: &mut Cpu, o: u32) {
    let count = cpu.read8(o + W_COUNT) as u32;
    let side = cpu.read32(o + W_SIDE) as i32;
    let mut held = 0u32;
    let mut at = cpu.read32(o + W_POINTS) as i32;
    for i in 1..count {
        let v = cpu.read32(o + W_POINTS + 12 * i) as i32;
        if (side < 0 && at > v) || (side > 0 && at < v) {
            at = v;
            held = i;
        }
    }
    let out = side.wrapping_mul(W_REACH as i32);
    let x = cpu.read32(o + O_POSITION) as i32;
    cpu.write32(o + O_POSITION, out.wrapping_sub(at.wrapping_sub(x)) as u32);
    let point_y = cpu.read32(o + W_POINTS + 4 + 12 * held);
    if cpu.read8(o + W_HELD) as u32 == held {
        let y = cpu.read32(o + O_POSITION + 4);
        let y = cpu.read32(o + W_HELD_Y).wrapping_sub(point_y.wrapping_sub(y));
        cpu.write32(o + O_POSITION + 4, y);
        return;
    }
    cpu.write8(o + W_HELD, held as u8);
    cpu.write32(o + W_HELD_Y, point_y);
    if (point_y as i32).wrapping_abs() <= 0x7f_ffff {
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, WALL_SOUND));
    }
    for field in [W_SPRING, W_SPRING_SPEED] {
        let w = cpu.read16(o + field) as i16 as i32;
        cpu.write16(o + field, (w / 8) as u16);
    }
}

/// The wall (behaviour 0x060861d8): its script (by +0xa8) run; unless that ended it, its turn
/// (+0x30) sprung (aim::turn_toward) toward the script's turn times its side, and held at the edge.
pub fn wall_step(cpu: &mut Cpu, o: u32) {
    let no = cpu.read8(o + W_SCRIPT_NO) as u32;
    let s = cpu.read32(W_SCRIPTS + 4 * no);
    nested!(cpu, 0x0608_5fcc, wall_script(cpu, o, s));
    if cpu.read8(o + O_MODE) == MODE_DONE {
        return;
    }
    let from = cpu.read16(o + P_TURN) as u32;
    let to = (cpu.read16(o + W_TURN) as i16 as i32).wrapping_mul(cpu.read32(o + W_SIDE) as i32) as u32 & 0xffff;
    cpu.push(&[0x1000, 8, 5]);
    nested!(cpu, 0x0605_70d8, aim::turn_toward(cpu, o + W_SPRING, o + W_SPRING_SPEED, from, to, 0x1000, 8, 5));
    let turn = cpu.read16(o + P_TURN).wrapping_add(cpu.read16(o + W_SPRING));
    cpu.write16(o + P_TURN, turn);
    nested!(cpu, 0x0608_60e4, wall_hold(cpu, o));
    cpu.pop(12);
}

/// A box of the wall (behaviour 0x0608626c; the wall is at +0x6c, the point's number at +0x70,
/// the second set at +0x71): it ends with the wall (mode past 111), else takes the place of the
/// wall's point (12-byte records from +0x3c, or from +0x6c for the second set); the first set
/// takes one of four boxes (0x060ac220) by the frame.
const BOX_WALL: u32 = 0x6c;
const BOX_POINT: u32 = 0x70;
const BOX_SECOND: u32 = 0x71;
const BOX_BOXES: u32 = 0x060a_c220;

pub fn wall_box(cpu: &mut Cpu, o: u32) {
    let wall = cpu.read32(o + BOX_WALL);
    if cpu.read8(wall + O_MODE) as i8 > 111 {
        cpu.write8(o + O_MODE, MODE_DONE);
        return;
    }
    let i = cpu.read8(o + BOX_POINT) as u32;
    if cpu.read8(o + BOX_SECOND) == 0 {
        let x = cpu.read32(wall + W_POINTS + 12 * i);
        cpu.write32(o + O_POSITION, x);
        let y = cpu.read32(wall + W_POINTS + 4 + 12 * i);
        cpu.write32(o + O_POSITION + 4, y);
        let frames = cpu.read32(GAME_FRAMES);
        let boxes = cpu.read32(BOX_BOXES + 4 * (frames & 3));
        cpu.write32(o + O_SCRIPT, boxes);
    } else {
        let x = cpu.read32(wall + 0x6c + 12 * i);
        cpu.write32(o + O_POSITION, x);
        let y = cpu.read32(wall + 0x70 + 12 * i);
        cpu.write32(o + O_POSITION + 4, y);
    }
}

/// The wall's roll (called with the wall): about x by its turn less the frame times its side
/// (256ths of a turn).
pub fn wall_roll(cpu: &mut Cpu, o: u32) {
    let side = cpu.read32(o + W_SIDE);
    let frames = cpu.read32(GAME_FRAMES);
    let spin = (side << 8).wrapping_neg().wrapping_mul(frames);
    let angle = (spin.wrapping_add(cpu.read16(o + P_TURN) as i16 as i32 as u32)) as u16 as i16 as i32 as u32;
    cpu.call(SL_ROT_X, &[angle]);
}

/// The wall's draw (its sprite `s`), the wall of three rotors (kind 0x12's models: 0 the arm,
/// 1 and 2 the blade halves): the matrix pushed, moved to its place, turned about z by its
/// turn (+0x30) and about y by 0x4000; the middle rotor at (0, 0x180000, 0) turned about x by
/// 0x4000, and two more 0x2aab either side of it round a circle of 24, turned 0x6aaa and 0x1555;
/// each a point (taken from the frame's pick of two offsets, 0x060ac240) and the two blades,
/// 0xc0000 apart; then the arm at (0, -0x2f0000, 0) and two more at 47 round the circle
/// (0x5555 and -0x5556), each rolled (wall_roll) and put with a point taken.
const WALL_KIND: u32 = 0x12;
const WALL_OFFSETS: u32 = 0x060a_c240;

fn wall_model(cpu: &mut Cpu, n: u32) -> u32 {
    let file = cpu.read32(KINDS + K_SIZE * WALL_KIND);
    let models = cpu.read32(file + KF_MODELS);
    let m = cpu.read32(models + 4 * n);
    cpu.read32(m)
}

/// A rotor: a point taken at its place (the offset the frame picks), the two blades put.
fn wall_rotor(cpu: &mut Cpu, at: u32) {
    let frames = cpu.read32(GAME_FRAMES);
    let y = cpu.read32(WALL_OFFSETS + 4 * (frames & 1));
    cpu.call(SL_CALC_POINT, &[0, y, 0, at]);
    cpu.call(SL_TRANSLATE, &[0xfffa_0000, 0, 0]);
    let m = wall_model(cpu, 2);
    cpu.call(SL_PUT_POLYGON, &[m]);
    cpu.call(SL_TRANSLATE, &[0xc_0000, 0, 0]);
    let m = wall_model(cpu, 1);
    cpu.call(SL_PUT_POLYGON, &[m]);
}

/// The matrix pushed and moved round a circle of `radius` at `angle`.
fn wall_circle(cpu: &mut Cpu, angle: u32, radius: u32) {
    cpu.call(SL_PUSH_MATRIX, &[]);
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, angle));
    let up = s.wrapping_mul(radius);
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, angle));
    let across = c.wrapping_mul(radius.wrapping_neg());
    cpu.call(SL_TRANSLATE, &[0, across, up]);
}

pub fn wall_draw(cpu: &mut Cpu, s: u32) {
    let o = s.wrapping_sub(0x10);
    cpu.call(SL_PUSH_MATRIX, &[]);
    let p = [0, 1, 2].map(|i| cpu.read32(o + O_POSITION + 4 * i));
    cpu.call(SL_TRANSLATE, &p);
    let turn = cpu.read16(s + 0x20) as i16 as i32 as u32;
    cpu.call(SL_ROT_Z, &[turn]);
    cpu.call(SL_ROT_Y, &[0x4000]);
    cpu.call(SL_PUSH_MATRIX, &[]);
    cpu.call(SL_TRANSLATE, &[0, 0x18_0000, 0]);
    cpu.call(SL_ROT_X, &[0x4000]);
    wall_rotor(cpu, s + 0x5c);
    cpu.call(SL_POP_MATRIX, &[]);
    for (angle, roll, at) in [(0xffff_d555u32, 0x6aaau32, 0x68u32), (0x2aab, 0x1555, 0x74)] {
        wall_circle(cpu, angle, 24);
        cpu.call(SL_ROT_X, &[roll]);
        wall_rotor(cpu, s + at);
        cpu.call(SL_POP_MATRIX, &[]);
    }
    cpu.call(SL_PUSH_MATRIX, &[]);
    cpu.call(SL_TRANSLATE, &[0, 0xffd1_0000, 0]);
    nested!(cpu, 0x0608_6308, wall_roll(cpu, o));
    cpu.call(SL_CALC_POINT, &[0, 0, 0, s + 0x2c]);
    let m = wall_model(cpu, 0);
    cpu.call(SL_PUT_POLYGON, &[m]);
    cpu.call(SL_POP_MATRIX, &[]);
    for (angle, at) in [(0x5555u32, 0x38u32), (0xffff_aaaa, 0x44)] {
        wall_circle(cpu, angle, 47);
        nested!(cpu, 0x0608_6308, wall_roll(cpu, o));
        cpu.call(SL_CALC_POINT, &[0, 0, 0, s + at]);
        let m = wall_model(cpu, 0);
        cpu.call(SL_PUT_POLYGON, &[m]);
        cpu.call(SL_POP_MATRIX, &[]);
    }
    cpu.call(SL_POP_MATRIX, &[]);
}

/// A box of the wall `wall` made, for its point number `point` (`second` 1 for the second set):
/// a free number from 2, the object (list 2, 0x74 bytes, behaviour wall_box, power 100, hit by
/// type 3 and player 4), at x -0x2000000 and, for the first set, the point's y and the first
/// boxes (0x060ac1f0), else the second's (0x060ac230). 1 if made, else 0.
const WALL_BOX_SIZE: u32 = 0x74;
const WALL_BOX_STEP: u32 = 0x0608_626c;
const WALL_BOX_FLAGS: u32 = 0x8006_0000;

pub fn wall_box_make(cpu: &mut Cpu, wall: u32, point: u32, second: u32) -> u32 {
    let (point, second) = (point as u8, second as u8);
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, PIECE_NUMBERS)) as i8;
    if n == -1 {
        return 0;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, WALL_BOX_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, PART_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + O_SCRIPT));
    cpu.write32(o + O_BEHAVIOUR, WALL_BOX_STEP);
    cpu.write32(o + O_FLAGS, WALL_BOX_FLAGS);
    cpu.write8(o + O_NUMBER, n as u8);
    let at = cpu.read8(o + O_NUMBER) as i8 as i32 as u32;
    cpu.write32(NUMBERED.wrapping_add(at << 2), o);
    cpu.write8(o + P_54, 3);
    cpu.write8(o + P_55, 4);
    cpu.write32(o + P_POWER, 100);
    cpu.write32(o + BOX_WALL, wall);
    cpu.write8(o + BOX_POINT, point);
    cpu.write8(o + BOX_SECOND, second);
    cpu.write32(o + O_POSITION, 0xfe00_0000);
    cpu.write32(o + O_POSITION + 8, 0);
    if second == 0 {
        cpu.write32(o + O_SCRIPT, 0x060a_c1f0);
        let y = cpu.read32(wall + W_POINTS + 4 + 12 * point as u32);
        cpu.write32(o + O_POSITION + 4, y);
    } else {
        cpu.write32(o + O_SCRIPT, 0x060a_c230);
    }
    1
}

/// A wall made from the row's event `e` (its place +4, +6; its kind +1; its side +0xb; its script
/// number +0x1d): the object (list 2, 0xb8 bytes, an actor with the wall's step, no boxes of its
/// own), its fields set, its first script step run and its turn set from it; then the boxes: kind
/// 0 two of the first set and one of the second (the draw 0x06086340, called at once), kind 1
/// three of each (wall_draw, called at once); it ends if one cannot be made. Gives 1.
const WALL_SIZE: u32 = 0xb8;
const WALL_FLAGS: u32 = 0xc0_0000;
const WALL_DRAW_THREE: u32 = 0x0608_64a8;
const WALL_DRAW_TWO: u32 = 0x0608_6340;

pub fn wall_make(cpu: &mut Cpu, e: u32) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, WALL_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, 0x0604_dcb4));
    if o == 0 {
        return 1;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    cpu.write32(o + O_BEHAVIOUR, 0x0608_61d8);
    cpu.write32(o + O_FLAGS, WALL_FLAGS);
    let x = (cpu.read16(e + E_PLACE) as i16 as i32 as u32) << 16;
    cpu.write32(o + O_POSITION, x);
    let y = (cpu.read16(e + E_PLACE + 2) as i16 as i32 as u32) << 16;
    cpu.write32(o + O_POSITION + 4, y);
    cpu.write32(o + O_POSITION + 8, 0);
    let side = cpu.read8(e + E_MIRROR) as i8 as i32 as u32;
    cpu.write32(o + W_SIDE, side);
    let kind = cpu.read8(e + E_KIND);
    cpu.write8(o + W_COUNT, kind.wrapping_add(2));
    let no = cpu.read8(e + 0x1d);
    cpu.write8(o + W_SCRIPT_NO, no);
    for field in [W_PLACE, W_WAIT, W_SPRING, W_SPRING_SPEED] {
        cpu.write16(o + field, 0);
    }
    cpu.write8(o + W_HELD, 0);
    let y = cpu.read32(o + O_POSITION + 4);
    cpu.write32(o + W_HELD_Y, y);
    let s = cpu.read32(W_SCRIPTS + 4 * no as u32);
    nested!(cpu, 0x0608_5fcc, wall_script(cpu, o, s));
    let turn = (cpu.read16(o + W_TURN) as i16 as i32).wrapping_mul(cpu.read32(o + W_SIDE) as i32);
    cpu.write16(o + P_TURN, turn as u16);
    let kind = cpu.read8(e + E_KIND);
    let made = |cpu: &mut Cpu, point: u32, second: u32| nested!(cpu, 0x0608_67cc, wall_box_make(cpu, o, point, second)) as u8 != 0;
    match kind {
        0 => {
            if !made(cpu, 0, 0) || !made(cpu, 1, 0) {
                cpu.write8(o + O_MODE, MODE_DONE);
                return 1;
            }
            made(cpu, 0, 1);
            cpu.write32(o + O_DRAW, WALL_DRAW_TWO);
            cpu.call(WALL_DRAW_TWO, &[o + O_FLAGS]);
        }
        1 => {
            if !made(cpu, 0, 0) || !made(cpu, 1, 0) || !made(cpu, 2, 0) {
                cpu.write8(o + O_MODE, MODE_DONE);
                return 1;
            }
            made(cpu, 0, 1);
            made(cpu, 1, 1);
            made(cpu, 2, 1);
            cpu.write32(o + O_DRAW, WALL_DRAW_THREE);
            nested!(cpu, 0x0608_64a8, wall_draw(cpu, o + O_FLAGS));
        }
        _ => {}
    }
    nested!(cpu, 0x0608_60e4, wall_hold(cpu, o));
    1
}

/// A frame list stepped (the list at +0xc4: pairs of a frame and how many frames to show it, in
/// the object's +0x2f, ending at 0xff, when the pointer is cleared; the count at +0xc8).
const FRAME_LIST: u32 = 0xc4;
const FRAME_COUNT: u32 = 0xc8;

pub fn frame_list_step(cpu: &mut Cpu, o: u32) {
    let list = cpu.read32(o + FRAME_LIST);
    if list == 0 {
        return;
    }
    if cpu.read8(o + FRAME_COUNT) as i8 <= 0 {
        let frame = cpu.read8(list);
        cpu.write8(o + O_FRAME, frame);
        let count = cpu.read8(list + 1);
        cpu.write8(o + FRAME_COUNT, count);
        cpu.write32(o + FRAME_LIST, list.wrapping_add(2));
        if cpu.read8(list + 2) == 0xff {
            cpu.write32(o + FRAME_LIST, 0);
        }
    }
    let count = cpu.read8(o + FRAME_COUNT).wrapping_sub(1);
    cpu.write8(o + FRAME_COUNT, count);
}

// ---- the divers ----------------------------------------------------------------------------------

/// An object given a list of frames (frame_list_step's): the list at +0xc4, the count 0.
pub fn frame_list_set(cpu: &mut Cpu, o: u32, list: u32) {
    cpu.write32(o + FRAME_LIST, list);
    cpu.write8(o + FRAME_COUNT, 0);
}

const AIM_SPEED: u32 = 0x9a;
const AIM_TURN: u32 = 0x9e;
const AIM_MOST: u32 = 0x800;
const AIM_EASE: u32 = 8;
const AIM_ACCEL: u32 = 16;
static AIM_PROLOGUE: std::sync::OnceLock<super::Prologue> = std::sync::OnceLock::new();

/// An object's turn (+0x30) sprung toward `to` (aim::turn_toward, at most 0x800 a go, eased by 8
/// and accelerated by 16): its speed is +0x9a and its turn (+0x9e) what it adds each frame.
pub fn aim_turn(cpu: &mut Cpu, o: u32, to: u32) {
    let from = cpu.read16(o + P_TURN) as u32;
    let to = to & 0xffff;
    cpu.nested_with_stack(&AIM_PROLOGUE, 0x0605_70d8, &[AIM_MOST, AIM_EASE, AIM_ACCEL], |cpu| {
        aim::turn_toward(cpu, o + AIM_TURN, o + AIM_SPEED, from, to, AIM_MOST, AIM_EASE, AIM_ACCEL)
    });
}

/// An object aimed at the nearest player (aim::angle_to_target with no delay; kept at +0xa0, a
/// word) and its turn sprung toward it.
pub fn aim_at_target(cpu: &mut Cpu, o: u32) {
    let a = nested!(cpu, 0x0605_6ad4, aim::angle_to_target(cpu, o, 0));
    cpu.write16(o + D_AIM, a as u16);
    let a = cpu.read16(o + D_AIM) as i16 as i32 as u32;
    nested!(cpu, 0x0608_6ae8, aim_turn(cpu, o, a));
}

/// A diver's speed (+0x80, +0x84: longs) eased to rest by 0x1000.
pub fn diver_slow(cpu: &mut Cpu, o: u32) {
    for at in [D_VX, D_VY] {
        let v = cpu.read32(o + at);
        let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0, 0x1000));
        cpu.write32(o + at, v);
    }
}

/// A diver: an enemy that comes along a route (ground.rs's route_step, the script at +0x6c; its
/// flags +0x7e, a word; its side +0x88 and +0x89 the route's flag), waits out a count (+0xb4),
/// then dives at the nearest player. Its fields: the phase (+0x7c), its speed (+0x80, +0x84), the
/// aim (+0xa0), the swirl (+0xbc, +0xc0), its frame count (+0xb8) and the trail (+0xcc, seven
/// words).
const D_ROUTE: u32 = 0x6c;
const D_PHASE: u32 = 0x7c;
const D_FLAGS: u32 = 0x7e;
const D_VX: u32 = 0x80;
const D_VY: u32 = 0x84;
const D_SIDE: u32 = 0x88;
const D_ROUTE_END: u32 = 0x89;
const D_ROUTE_TURN: u32 = 0x8c;
const D_AIM: u32 = 0xa0;
const D_LIFT: u32 = 0xb0;
const D_WAIT: u32 = 0xb4;
const D_FRAMES: u32 = 0xb8;
const D_SWIRL: u32 = 0xbc;
const D_SWIRL_SPEED: u32 = 0xc0;
const D_TRAIL: u32 = 0xcc;
const D_TRAIL_N: i32 = 7;
const D_TRAIL_FLIP: u32 = 0x102;
const D_AREA: [u32; 4] = [0xff40_0000, 0x00c0_0000, 0xff00_0000, 0x0100_0000];
const D_SCORE: u32 = 100;
const D_FRAME_LIST: u32 = 0x060a_c348;
const D_ROUTE_FLIPPED: u8 = 7;
const D_DONE: u16 = 0x10;
const D_HIT: u16 = 0x200;
const D_SOUND_START: u32 = 60;
const D_SOUND_SWIRL: u32 = 62;
const D_SWIRL_AT: u32 = 0xc000;
const D_FAR: i32 = 0x4_0000;
const D_OFF: u32 = 0x8_0000;
const D_FRAME_SOUND: i32 = 10;
const D_FRAME_AIM: i32 = 40;
const D_RAM_GROUND_DY: u32 = super::globals::GROUND_DY_AT;
const D_DRIVE: u32 = 0x1_8000;
const D_KIND: u8 = 22;
const D_SIZE: u32 = 0x114;
const D_STEP: u32 = 0x0608_6ba8;
const D_DRAW: u32 = 0x0608_6f38;
const D_POWER: u32 = 0x157c;
const D_BOXES: u32 = 0xca;
const D_BOXES_FROM: u32 = 0x060a_c2f0;
const D_ROUTES: u32 = 0x060a_c2d8;
const S_TURN: u32 = 0x20;

/// The diver's step (behaviour 0x06086ba8). Done when off the area (a box around the screen), or
/// scored and blown up when its power (+0x50) is gone. On its route (script at +0x6c) it takes
/// the route's steps (its flag 0x10 when the route says 7; the route dropped then if +0x89 is 1),
/// its turn the side (+0x88) times +0x8c, its lift (+0x1c) +0xb0; with no route its wait
/// counts down. Phase 0 waits for the wait to run out or the route to end, then gives it its
/// frames and a sound (60) and goes to phase 1 (diver_dive). Then the frame count up, the turn on
/// by +0x9e, the frames, the trail, the position moved by the speed and the lift's far flag.
pub fn diver_step(cpu: &mut Cpu, o: u32) {
    let [top, bottom, left, right] = D_AREA;
    let out = cpu.nested_with_stack(&aim::AREA_FLAGS_PROLOGUE, aim::AREA_FLAGS, &[right], |cpu| aim::area_flags(cpu, o + O_FLAGS, top, bottom, left, right)) as u8;
    if out != 0 {
        cpu.write8(o + O_MODE, MODE_DONE);
        return;
    }
    if cpu.read32(o + P_POWER) as i32 <= 0 {
        nested!(cpu, 0x0605_61ac, hits::score_hit_by(cpu, o, D_SCORE));
        nested!(cpu, 0x0605_9fc4, ground::blown_up_burster(cpu, o));
        cpu.write8(o + O_MODE, MODE_DONE);
        return;
    }
    let route = cpu.read32(o + D_ROUTE);
    if route != 0 {
        let r = nested!(cpu, 0x0605_a000, ground::route_step(cpu, o, route)) as u8;
        if r == D_ROUTE_FLIPPED {
            let flags = cpu.read16(o + D_FLAGS) | D_DONE;
            cpu.write16(o + D_FLAGS, flags);
            if cpu.read8(o + D_ROUTE_END) as i8 == 1 {
                cpu.write32(o + D_ROUTE, out as u32);
            }
        }
        let side = cpu.read8(o + D_SIDE) as i8 as i32;
        let turn = cpu.read16(o + D_ROUTE_TURN) as i16 as i32;
        cpu.write16(o + P_TURN, side.wrapping_mul(turn) as u16);
        let lift = cpu.read32(o + D_LIFT);
        cpu.write32(o + O_POSITION + 8, lift);
    } else {
        let wait = cpu.read32(o + D_WAIT).wrapping_sub(1);
        cpu.write32(o + D_WAIT, wait);
    }
    match cpu.read8(o + D_PHASE) {
        0 => {
            if (cpu.read32(o + D_WAIT) as i32) < 0 || cpu.read16(o + D_FLAGS) & D_DONE != 0 {
                cpu.write8(o + D_PHASE, 1);
                cpu.write32(o + D_FRAMES, 0);
                nested!(cpu, 0x0608_6ad0, frame_list_set(cpu, o, D_FRAME_LIST));
                nested!(cpu, 0x0600_50d4, sound::se_set(cpu, D_SOUND_START));
            }
        }
        1 => diver_dive(cpu, o),
        _ => {}
    }
    let frames = cpu.read32(o + D_FRAMES).wrapping_add(1);
    cpu.write32(o + D_FRAMES, frames);
    let turn = cpu.read16(o + P_TURN).wrapping_add(cpu.read16(o + AIM_TURN));
    cpu.write16(o + P_TURN, turn);
    nested!(cpu, 0x0608_6a80, frame_list_step(cpu, o));
    for i in 0..D_TRAIL_N {
        let swirl = cpu.read32(o + D_SWIRL);
        let a = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, swirl, (i * 10) as u32));
        let b = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, swirl, 0x20));
        cpu.write16(o + D_TRAIL + 8 * i as u32, a.wrapping_add(b) as u16);
    }
    let shown = if cpu.read8(o + O_FRAME) == 3 { 0x10 } else { 0 };
    cpu.write16(o + D_TRAIL_FLIP + 2, shown);
    cpu.write16(o + D_TRAIL_FLIP + 10, shown);
    if cpu.read16(o + D_HIT_FLAGS) & D_HIT != 0 {
        let flags = cpu.read16(o + D_FLAGS) | 1;
        cpu.write16(o + D_FLAGS, flags);
    }
    let side = cpu.read8(o + D_SIDE) as i8 as i32;
    let x = cpu.read32(o + O_POSITION).wrapping_add((cpu.read32(o + D_VX) as i32).wrapping_mul(side) as u32);
    cpu.write32(o + O_POSITION, x);
    let dy_at = cpu.read32(D_RAM_GROUND_DY);
    let dy = cpu.read32(dy_at);
    let vy = cpu.read32(o + D_VY);
    let y = cpu.read32(o + O_POSITION + 4).wrapping_add(vy.wrapping_sub(dy));
    cpu.write32(o + O_POSITION + 4, y);
    let far = (cpu.read32(o + O_POSITION + 8) as i32).wrapping_abs() > D_FAR;
    let flags = cpu.read32(o + O_FLAGS);
    cpu.write32(o + O_FLAGS, if far { flags | D_OFF } else { flags & !D_OFF });
}

/// The diver's phase 1: with no frames left to run, the swirl (+0xbc) moved toward 0xc000 by its
/// speed (+0xc0, eased toward a 32nd of the way to it by a 512th of the way; sound 62 once); once
/// the route has ended (+0x89 is 1): at frame 10 it is aimed at the nearest player, to frame 40 it
/// slows, then it turns toward the aim and its speed eases to 0x18000 times the aim's sine and
/// -0x18000 times its cosine.
fn diver_dive(cpu: &mut Cpu, o: u32) {
    if cpu.read32(o + FRAME_LIST) == 0 {
        let a = cpu.read16(o + D_AIM) as i16;
        if a == 0 {
            cpu.write16(o + D_AIM, 1);
            nested!(cpu, 0x0600_50d4, sound::se_set(cpu, D_SOUND_SWIRL));
        }
        let off = cpu.read32(o + D_SWIRL).wrapping_sub(D_SWIRL_AT);
        let q = nested!(cpu, 0x0600_6a78, math::quotient(cpu, off, 0x200)) as i32;
        let to = nested!(cpu, 0x0600_6a78, math::quotient(cpu, off.wrapping_neg(), 0x20));
        let speed = cpu.read32(o + D_SWIRL_SPEED);
        let speed = nested!(cpu, 0x0604_6168, actors::approach(cpu, speed, to, q.wrapping_abs() as u32));
        cpu.write32(o + D_SWIRL_SPEED, speed);
        let swirl = cpu.read32(o + D_SWIRL).wrapping_add(speed);
        cpu.write32(o + D_SWIRL, swirl);
    }
    if cpu.read8(o + D_ROUTE_END) as i8 != 1 {
        return;
    }
    if cpu.read32(o + D_FRAMES) as i32 == D_FRAME_SOUND {
        nested!(cpu, 0x0608_6b24, aim_at_target(cpu, o));
    }
    if cpu.read32(o + D_FRAMES) as i32 <= D_FRAME_AIM {
        nested!(cpu, 0x0608_6b58, diver_slow(cpu, o));
        return;
    }
    let a = cpu.read16(o + D_AIM) as i16 as i32 as u32;
    nested!(cpu, 0x0608_6ae8, aim_turn(cpu, o, a));
    let a = cpu.read16(o + D_AIM) as i16 as i32 as u32;
    let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    drive(cpu, o, D_VX, sin, D_DRIVE);
    let a = cpu.read16(o + D_AIM) as i16 as i32 as u32;
    let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    drive(cpu, o, D_VY, cos, D_DRIVE.wrapping_neg());
}

/// A speed (at `at`) eased toward `trig` times `scale` by a 32nd of that.
fn drive(cpu: &mut Cpu, o: u32, at: u32, trig: u32, scale: u32) {
    let m = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, trig, scale));
    let m_abs = (m as i32).wrapping_abs();
    let step = m_abs.wrapping_add(if m_abs < 0 { 31 } else { 0 }) >> 5;
    let v = cpu.read32(o + at);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, m, step as u32));
    cpu.write32(o + at, v);
}

/// The diver's draw (its sprite `s`; the hit mark of the piece's draw): unless hidden (the top bit
/// of its flags), a sprite of kind 22 with its frame and turn, its colour by its type (+0x55) and
/// the frame count's parity, drawn; with a swirl (+0xbc) scaled by it and the loop count's low
/// bits; then two more, 0x4000 either side of its turn 0x18-ish out, and its thrust (0xa000 behind
/// by 13) with the flag 0x10 flipped.
pub fn diver_draw(cpu: &mut Cpu, s: u32) {
    let o = s.wrapping_sub(0x10);
    let at = s + 0x6e;
    let mut flags = cpu.read16(at) as i16 as i32 as u32;
    if flags & 2 != 0 {
        flags &= 0xffff_fffc;
        cpu.write16(at, flags as u16);
    }
    let flags = cpu.read16(at) as i16 as i32 as u32;
    if flags & 1 != 0 {
        cpu.write16(at, (flags | 2) as u16);
    }
    if cpu.read32(s) & F_FLAGGED != 0 {
        return;
    }
    let d = cpu.stack();
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, d));
    cpu.write8(d + S_KIND, D_KIND);
    let frame = cpu.read8(s + S_FRAME);
    cpu.write8(d + S_FRAME, frame);
    cpu.write8(d + S_LIFT, 0xf8);
    for k in 0..3 {
        let v = cpu.read32(s + S_POSITION + 4 * k);
        cpu.write32(d + S_POSITION + 4 * k, v);
    }
    let turn = cpu.read16(s + S_TURN);
    cpu.write16(d + S_TURN, turn);
    let base = cpu.read16(CHAR_BASE);
    let colour = if cpu.read16(at) & 1 == 0 { cpu.read16(s + S_COLOUR) } else { base.wrapping_add(0xe0) };
    cpu.write16(d + S_COLOUR, colour);
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
    let swirl = cpu.read32(o + D_SWIRL) as i32;
    if swirl != 0 {
        let loops = cpu.read32(LOOPS);
        let k = (loops & 3) as i32;
        let sx = swirl.wrapping_add((swirl.wrapping_add(if swirl < 0 { 7 } else { 0 }) >> 3).wrapping_mul(k));
        cpu.write32(d + S_SCALES, sx as u32);
        let q = swirl.wrapping_add(if swirl < 0 { 15 } else { 0 }) >> 4;
        let sy = q.wrapping_mul(k).wrapping_add(0x2_0000).wrapping_sub(swirl);
        cpu.write32(d + S_SCALES + 4, sy as u32);
        let odd = loops & 1 != 0;
        let add = match cpu.read8(o + P_55) {
            0 if odd => 0,
            0 => 0x10,
            2 => 0x40,
            _ if odd => 0x20,
            _ => 0x30,
        };
        cpu.write16(d + S_COLOUR, base.wrapping_add(add));
        cpu.write8(d + S_FRAME, 4);
        cpu.write8(d + S_LIFT, 0xf7);
        for (flags, quarter) in [(None, 0x4000u32), (Some(0x10), 0xc000)] {
            if let Some(f) = flags {
                cpu.write32(d + S_FLAGS, f);
            }
            let a = (cpu.read16(o + P_TURN).wrapping_add(quarter as u16)) as i16 as i32 as u32;
            let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
            let m = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, sin, 0x18_0000));
            let x = cpu.read32(o + O_POSITION).wrapping_add(m);
            cpu.write32(d + S_POSITION, x);
            let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
            let m = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, cos, 0xffe8_0000));
            let y = cpu.read32(o + O_POSITION + 4).wrapping_add(m);
            cpu.write32(d + S_POSITION + 4, y);
            nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
        }
    }
    let flags = cpu.read32(s) ^ 0x10;
    cpu.write32(s, flags);
    nested!(cpu, 0x0605_9d24, ground::thrust(cpu, o, d, 13, 0xa000));
}

/// A diver made from the event row `e` (its place +4, +6; its route +8; +10, +11 and +0x1b its
/// flag, side and type; +0x14 and +0x16 its turns; +0x1c its wait): object of 0x114 bytes in list
/// 2 on the free number, flags 0xc00000, kind 22 whose colour it takes plus 16 a type, power
/// 0x157c, its boxes copied from the row table's. Gives 1 either way.
pub fn diver_row(cpu: &mut Cpu, e: u32) -> u32 {
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, 2)) as i8;
    if n == -1 {
        return 1;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, D_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, PART_EXEC));
    if o == 0 {
        return 1;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + O_SCRIPT));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + O_BEHAVIOUR, D_STEP);
    cpu.write32(o + O_FLAGS, PIECE_FLAGS);
    cpu.write32(o + O_DRAW, D_DRAW);
    cpu.write8(o + O_KIND, D_KIND);
    cpu.write8(o + O_FRAME, 0);
    let t = cpu.read8(e + E_TYPE);
    let base = cpu.read16(kind(D_KIND as u32) + K_COLOUR);
    cpu.write16(o + O_COLOUR_A, base.wrapping_add((t as u16) << 4));
    cpu.write8(o + O_NUMBER, n as u8);
    let at = cpu.read8(o + O_NUMBER) as i8 as i32 as u32;
    cpu.write32(NUMBERED.wrapping_add(at << 2), o);
    cpu.write32(o + P_POWER, D_POWER);
    cpu.write8(o + P_54, 1);
    cpu.write8(o + P_55, t);
    let boxes = o + D_BOXES;
    nested!(cpu, 0x0605_6440, hits::boxes_copy(cpu, boxes, D_BOXES_FROM));
    cpu.write32(o + O_SCRIPT, boxes);
    let x = (cpu.read16(e + E_PLACE) as i16 as i32 as u32) << 16;
    cpu.write32(o + O_POSITION, x);
    let y = (cpu.read16(e + E_PLACE + 2) as i16 as i32 as u32) << 16;
    cpu.write32(o + O_POSITION + 4, y);
    cpu.write32(o + O_POSITION + 8, 0);
    cpu.write32(o + FRAME_LIST, 0);
    cpu.write8(o + FRAME_COUNT, 0);
    cpu.write32(o + D_SWIRL, 0);
    cpu.write32(o + D_SWIRL_SPEED, 0);
    let side = cpu.read8(e + E_MIRROR);
    cpu.write8(o + D_SIDE, side);
    let end = cpu.read8(e + E_SETTINGS);
    cpu.write8(o + D_ROUTE_END, end);
    let route = cpu.read8(e + E_SCRIPT) as u32;
    let script = cpu.read32(D_ROUTES + 4 * route);
    cpu.write32(o + D_ROUTE, script);
    let wait = cpu.read16(e + E_1C) as i16 as i32 as u32;
    cpu.write32(o + D_WAIT, wait);
    let turn = cpu.read16(e + E_DIRECTION);
    cpu.write16(o + P_TURN, turn);
    let turn = cpu.read16(e + E_TURN);
    cpu.write16(o + AIM_TURN, turn);
    1
}

// ---- the divers' fan shots and the carriers ------------------------------------------------------

const FAN_STEP: u32 = 0x0608_7314;
const FAN_SCRIPT: u32 = 0x060a_c3a8;
const FAN_SOUND: u32 = 30;
const FAN_SPEED: u32 = 0x1_8000;
const FAN_ANGLES: [u16; 3] = [0, 0x5555, 0xaaaa];
const FAN_NUMBER: u32 = 0x1e;
const FAN_SIDE: u32 = 0x2c;
const FAN_AIM: u32 = 0x78;
const FAN_SHOOTER: u32 = 0x70;
const FAN_SIZE: u32 = 0xbc;
const SHOT_SPRITE_FLAGS: u32 = 0x2000_0400;
const SHOT_COLOUR_ADD: u16 = 0x40;
const SHOT_OFF: u32 = 0x8_0000;

/// A fan shot (behaviour 0x06087314): unless it ends (bullets::bullet_ends), moved by its speed
/// (+0x80, +0x84) with its frame (+0x2f) 0x75 to 0x78 by the frame count's bits 2 and 3, and shown
/// every other frame by its count (+0x88, a byte going up each frame).
pub fn fan_shot_step(cpu: &mut Cpu, o: u32) {
    if nested!(cpu, 0x0605_3b10, bullets::bullet_ends(cpu, o, 0)) as u8 != 0 {
        return;
    }
    let x = cpu.read32(o + O_POSITION).wrapping_add(cpu.read32(o + D_VX));
    cpu.write32(o + O_POSITION, x);
    let y = cpu.read32(o + O_POSITION + 4).wrapping_add(cpu.read32(o + D_VY));
    cpu.write32(o + O_POSITION + 4, y);
    let frames = cpu.read32(GAME_FRAMES);
    cpu.write8(o + O_FRAME, (((frames >> 2) & 3) + 0x75) as u8);
    let count = cpu.read8(o + D_SIDE).wrapping_add(1);
    cpu.write8(o + D_SIDE, count);
    let flags = cpu.read32(o + O_FLAGS);
    cpu.write32(o + O_FLAGS, if count & 1 == 0 { flags & !SHOT_OFF } else { flags | SHOT_OFF });
}

/// A fan shot made at the place at `from` (x, y longs; the shooter's number at +0x1e; a word at
/// +0x2c whose sign picks its count's start) heading `angle`: an object of list 2 (0xbc bytes), its
/// speed 0x18000 along the angle (sine for x, minus cosine for y), its script 0x060ac3a8.
pub fn fan_shot_make(cpu: &mut Cpu, from: u32, angle: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, FAN_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, SHOT_EXEC));
    if o == 0 {
        return;
    }
    let angle = angle as u16 as i16 as i32 as u32;
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + O_SCRIPT));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    let number = cpu.read8(from + FAN_NUMBER);
    cpu.write8(o + O_NUMBER, number);
    cpu.write32(o + O_BEHAVIOUR, FAN_STEP);
    cpu.write32(o + O_FLAGS, SHOT_SPRITE_FLAGS);
    let x = cpu.read32(from);
    cpu.write32(o + O_POSITION, x);
    let y = cpu.read32(from + 4);
    cpu.write32(o + O_POSITION + 4, y);
    cpu.write32(o + O_POSITION + 8, 0);
    cpu.write8(o + O_33, 8);
    cpu.write8(o + O_FRAME, 0x73);
    cpu.write8(o + O_KIND, 0);
    let colour = cpu.read16(CHAR_BASE).wrapping_add(SHOT_COLOUR_ADD);
    cpu.write16(o + O_COLOUR_B, colour);
    cpu.write16(o + O_COLOUR_A, colour);
    cpu.write32(o + O_SCRIPT, FAN_SCRIPT);
    cpu.write8(o + P_54, 1);
    cpu.write8(o + P_55, 5);
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, angle));
    let vx = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, FAN_SPEED));
    cpu.write32(o + D_VX, vx);
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, angle));
    let vy = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, c, FAN_SPEED.wrapping_neg()));
    cpu.write32(o + D_VY, vy);
    let side = if cpu.read16(from + FAN_SIDE) as i16 > 0 { 0 } else { 1 };
    cpu.write8(o + D_SIDE, side);
}

/// A wing gun's burst: three fan shots from its place, a third of a turn apart from its aim (+0x78),
/// with sound 30. (The shooter's number (+0x32) and its side (+0x70, a byte as a word) are handed
/// over in the frame.)
pub fn fan_burst(cpu: &mut Cpu, o: u32) {
    let from = cpu.stack();
    let number = cpu.read8(o + O_NUMBER);
    cpu.write8(from + FAN_NUMBER, number);
    let x = cpu.read32(o + O_POSITION);
    cpu.write32(from, x);
    let y = cpu.read32(o + O_POSITION + 4);
    cpu.write32(from + 4, y);
    let side = cpu.read8(o + FAN_SHOOTER) as i8 as i16 as u16;
    cpu.write16(from + FAN_SIDE, side);
    for add in FAN_ANGLES {
        let aim = cpu.read16(o + FAN_AIM).wrapping_add(add) as u32;
        nested!(cpu, 0x0608_7394, fan_shot_make(cpu, from, aim));
    }
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, FAN_SOUND));
}

const WG_SIDE: u32 = 0x70;
const WG_PARENT: u32 = 0x6c;
const WG_SPREAD: u32 = 0x74;
const WG_AIM: u32 = 0x78;
const WG_COUNT: u32 = 0x7c;
const WG_BURSTS: u32 = 0x80;
const WG_OFFSET_X: i32 = 0x34_0000;
const WG_OFFSET_Y: i32 = 0xffcc_0000_u32 as i32;
const WG_COPIED: u32 = 0x8_0030;
const WG_KEPT: u32 = 0xfff7_ffcf;
const WG_RISE: u32 = 0x1_0000;
const WG_RISE_STEP: u32 = 0x800;
const WG_TURN_RATE: i32 = 0x80;
const WG_PACES: u32 = 0x060a_c3b8;
const WG_BURST_EVERY: u32 = 15;
const WG_BURST_LAST: i32 = 8;
const WG_GONE: i8 = 111;

/// A wing gun on a carrier or diver (behaviour 0x0608754c; its parent +0x6c, its side +0x70: 1 or
/// -1): once the parent has ended (past mode 111) it ends, with a burning if the parent's power is
/// gone. Otherwise its flags take the parent's (0x30 and 0x80000), its turn the parent's, its place
/// the parent's turned a quarter and set out by its side (0x340000 across, 0xffcc0000 down). While
/// the parent has not flagged 0x10 its spread (+0x74) eases to 0; once it does it eases up to
/// 0x10000, and at 0x10000 its aim (+0x78) turns by its side times the parent's times 0x80, and a
/// count (+0x7c) steps; at the count's pace by the difficulty, a count (+0x80) steps and, up to 8
/// of every 15, it fires a burst.
pub fn wing_gun_step(cpu: &mut Cpu, o: u32) {
    let parent = cpu.read32(o + WG_PARENT);
    if cpu.read8(parent + O_MODE) as i8 > WG_GONE {
        if cpu.read32(parent + P_POWER) as i32 <= 0 {
            let at = cpu.stack();
            for k in [0, 4, 8] {
                let v = cpu.read32(o + O_POSITION + k);
                cpu.write32(at + k, v);
            }
            nested!(cpu, 0x0605_7cb0, explosions::burning_set(cpu, at));
        }
        cpu.write8(o + O_MODE, MODE_DONE);
        return;
    }
    let flags = cpu.read32(o + O_FLAGS) & WG_KEPT;
    cpu.write32(o + O_FLAGS, flags);
    let flags = flags | (cpu.read32(parent + O_FLAGS) & WG_COPIED);
    cpu.write32(o + O_FLAGS, flags);
    let turn = cpu.read16(parent + P_TURN);
    cpu.write16(o + P_TURN, turn);
    let side = cpu.read8(o + WG_SIDE) as i8 as i32;
    let a = (turn.wrapping_add(0x4000)) as i16 as i32 as u32;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    let m = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, side.wrapping_mul(WG_OFFSET_X) as u32));
    let x = cpu.read32(parent + O_POSITION).wrapping_add(m);
    cpu.write32(o + O_POSITION, x);
    let a = (cpu.read16(o + P_TURN).wrapping_add(0x4000)) as i16 as i32 as u32;
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    let m = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, c, side.wrapping_mul(WG_OFFSET_Y) as u32));
    let y = cpu.read32(parent + O_POSITION + 4).wrapping_add(m);
    cpu.write32(o + O_POSITION + 4, y);
    let z = cpu.read32(parent + O_POSITION + 8);
    cpu.write32(o + O_POSITION + 8, z);
    let spread = cpu.read32(o + WG_SPREAD);
    if cpu.read16(parent + D_FLAGS) & D_DONE == 0 {
        let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, spread, 0, WG_RISE_STEP));
        cpu.write32(o + WG_SPREAD, v);
        return;
    }
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, spread, WG_RISE, WG_RISE_STEP));
    cpu.write32(o + WG_SPREAD, v);
    if v != WG_RISE {
        return;
    }
    let side = cpu.read8(o + WG_SIDE) as i8 as i32;
    let by = (cpu.read8(parent + D_SIDE) as i8 as i32).wrapping_mul(WG_TURN_RATE);
    let aim = (cpu.read16(o + WG_AIM) as i16 as i32).wrapping_sub(side.wrapping_mul(by));
    cpu.write16(o + WG_AIM, aim as u16);
    let n = cpu.read32(o + WG_COUNT).wrapping_add(1);
    cpu.write32(o + WG_COUNT, n);
    let at = cpu.read8(DIFFICULTY) as i8 as i32 as u32;
    let pace = cpu.read8(WG_PACES.wrapping_add(at)) as u32;
    if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, n, pace)) != 0 {
        return;
    }
    let bursts = cpu.read32(o + WG_BURSTS);
    let r = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, bursts, WG_BURST_EVERY)) as i32;
    if r <= WG_BURST_LAST {
        nested!(cpu, 0x0608_74d0, fan_burst(cpu, o));
    }
    let bursts = cpu.read32(o + WG_BURSTS).wrapping_add(1);
    cpu.write32(o + WG_BURSTS, bursts);
}

const WGD_KIND: u8 = 0;
const WGD_FRONT: u32 = 0x1c_0000;
const WGD_BACK: u32 = 0x5_0000;
const WGD_ACROSS: u32 = 0x2d_0000;
const WGD_FLAGS: u32 = 0x30;
const WGD_CORONA: u16 = 0x30;
const WGD_FLARE: u16 = 0x40;
const WGD_SCALE: u32 = 0xa000;
const WGD_FRAME: u8 = 0x70;
const WGD_FLARE_FRAME: u8 = 0x36;
const WGD_FLARE_SPREAD: u32 = 0x1800;
const WGD_LIFT: u8 = 0xf7;

/// A wing gun's draw (its sprite `s`): a sprite of the gun at the parent's place set out by the
/// parent's turn (a quarter, in quarters: the turn of 0, 0x4000, 0x8000 or 0xc000 puts it 0x1c0000
/// or -0x50000 along and 0x2d0000 across, the parent's flips (0x10 and 0x20) taking the signs), its
/// frame 0x70 to 0x72 by the loop count, drawn; and, while its spread (+0x74) is up, a flare at the
/// gun's place with a random size and turn.
pub fn wing_gun_draw(cpu: &mut Cpu, s: u32) {
    let o = s.wrapping_sub(0x10);
    let parent = cpu.read32(o + WG_PARENT);
    let d = cpu.stack();
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, d));
    let pflags = cpu.read32(parent + O_FLAGS);
    let sx: i32 = if pflags & 0x10 != 0 { -1 } else { 1 };
    let sy: i32 = if pflags & 0x20 != 0 { -1 } else { 1 };
    let turn = cpu.read16(s + S_TURN);
    let front = cpu.read8(o + WG_SIDE) as i8 > 0;
    let (px, py) = (cpu.read32(parent + O_POSITION), cpu.read32(parent + O_POSITION + 4));
    let off = if front { sx.wrapping_mul(WGD_BACK.wrapping_neg() as i32) } else { sx.wrapping_mul(WGD_FRONT as i32) } as u32;
    let across = sy.wrapping_mul(WGD_ACROSS as i32) as u32;
    match turn {
        0 => {
            cpu.write32(d + S_POSITION, px.wrapping_add(off));
            cpu.write32(d + S_POSITION + 4, py.wrapping_add(across));
        }
        0x4000 => {
            cpu.write32(d + S_POSITION + 4, py.wrapping_add(off));
            cpu.write32(d + S_POSITION, px.wrapping_sub(across));
        }
        0x8000 => {
            cpu.write32(d + S_POSITION, px.wrapping_sub(off));
            cpu.write32(d + S_POSITION + 4, py.wrapping_sub(across));
        }
        0xc000 => {
            cpu.write32(d + S_POSITION + 4, py.wrapping_sub(off));
            cpu.write32(d + S_POSITION, px.wrapping_add(across));
        }
        _ => {}
    }
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write32(d + S_FLAGS, (r & 0x100) | (pflags & WGD_FLAGS));
    cpu.write8(d + S_KIND, WGD_KIND);
    let loops = cpu.read32(LOOPS);
    let m = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, loops, 3));
    cpu.write8(d + S_FRAME, (m as u8).wrapping_add(WGD_FRAME));
    cpu.write32(d + S_SCALES, WGD_SCALE);
    let base = cpu.read16(CHAR_BASE);
    cpu.write16(d + S_COLOUR, base.wrapping_add(WGD_CORONA));
    let z = cpu.read32(parent + O_POSITION + 8);
    cpu.write32(d + S_POSITION + 8, z);
    cpu.write8(d + S_LIFT, WGD_LIFT);
    let v = if front { WGD_SCALE as i32 } else { 0x6000 };
    let spin = v.wrapping_mul(sy).wrapping_mul(sx).wrapping_add(cpu.read16(parent + P_TURN) as i16 as i32);
    cpu.write16(d + S_TURN, spin as u16);
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
    let spread = cpu.read32(o + WG_SPREAD);
    if spread == 0 {
        return;
    }
    let (x, y) = (cpu.read32(o + O_POSITION), cpu.read32(o + O_POSITION + 4));
    cpu.write32(d + S_POSITION, x);
    cpu.write32(d + S_POSITION + 4, y);
    let z = cpu.read32(parent + O_POSITION + 8).wrapping_sub(1);
    cpu.write32(d + S_POSITION + 8, z);
    cpu.write8(d + S_KIND, 3);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write8(d + S_FRAME, ((r & 1) as u8).wrapping_add(WGD_FLARE_FRAME));
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, WGD_FLARE_SPREAD));
    let size = spread.wrapping_add(r);
    cpu.write32(d + S_SCALES, size);
    cpu.write32(d + S_SCALES + 4, size);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write16(d + S_TURN, r as u16);
    let base = cpu.read16(CHAR_BASE);
    cpu.write16(d + S_COLOUR, base.wrapping_add(WGD_FLARE));
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
}

const WGM_SIZE: u32 = 0x84;
const WGM_STEP: u32 = 0x0608_754c;
const WGM_DRAW: u32 = 0x0608_7704;
const WGM_POWER: u32 = 10;
const WGM_BOXES: u32 = 0x060a_c3b0;

/// A wing gun made on the object `parent`, on `side` (1 or -1): an object of list 2 (0x84 bytes) with
/// the gun's behaviour and draw, flags 0xc00000, power 10 and its boxes, stepped once at once.
pub fn wing_gun_make(cpu: &mut Cpu, parent: u32, side: u32) {
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, 2)) as i8;
    if n == -1 {
        return;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, WGM_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, PART_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + O_SCRIPT));
    cpu.write32(o + O_BEHAVIOUR, WGM_STEP);
    cpu.write32(o + O_FLAGS, PIECE_FLAGS);
    cpu.write32(o + O_DRAW, WGM_DRAW);
    cpu.write32(o + O_POSITION + 8, 0);
    cpu.write8(o + O_NUMBER, n as u8);
    let at = cpu.read8(o + O_NUMBER) as i8 as i32 as u32;
    cpu.write32(NUMBERED.wrapping_add(at << 2), o);
    cpu.write8(o + P_54, 1);
    cpu.write32(o + P_POWER, WGM_POWER);
    cpu.write32(o + O_SCRIPT, WGM_BOXES);
    cpu.write32(o + WG_PARENT, parent);
    cpu.write8(o + WG_SIDE, side as u8);
    cpu.write32(o + WG_SPREAD, 0);
    cpu.write16(o + WG_AIM, 0);
    cpu.write32(o + WG_COUNT, 0);
    cpu.write32(o + WG_BURSTS, 0);
    nested!(cpu, 0x0608_754c, wing_gun_step(cpu, o));
}

const CR_BOX_WIDE: [u32; 4] = [0xff48_0000, 0x00b8_0000, 0xfee0_0000, 0x0120_0000];
const CR_BOX_NARROW: [u32; 4] = [0xff20_0000, 0x00e0_0000, 0xff08_0000, 0x00f8_0000];
const CR_NARROW: u16 = 0x4000;
const CR_CHECKED: u16 = 0x1000;
const CR_BOXES: [u32; 2] = [0x060a_c40e, 0x060a_c3be];
const CR_FRAMES: u32 = 0x060a_c460;
const CR_ROUTE_BEGUN: u8 = 7;
const CR_ROUTE_OVER: u8 = 9;
const CR_SCORE: u32 = 100;

/// A carrier (behaviour 0x06087a38): ends when off its box (the narrow one when its turn has 0x4000,
/// checked only once flagged 0x1000) or, its power gone, scored and blown up (ground.rs's
/// blown_up_wounded). Otherwise its boxes swap each frame, its frame (+0x2f) cycles with the frame
/// count's bits 2 and 3, it takes its route's steps (flag 0x10 set by the route's 7, cleared by its
/// 9), moves by its speed (the side +0x88 times x) and its lift (+0xb0) is its height, far when
/// past 0x40000.
pub fn carrier_step(cpu: &mut Cpu, o: u32) {
    let [top, bottom, left, right] = if cpu.read16(o + P_TURN) & CR_NARROW == 0 { CR_BOX_WIDE } else { CR_BOX_NARROW };
    if cpu.read16(o + D_FLAGS) & CR_CHECKED != 0 {
        let out = cpu.nested_with_stack(&aim::AREA_FLAGS_PROLOGUE, aim::AREA_FLAGS, &[right], |cpu| aim::area_flags(cpu, o + O_FLAGS, top, bottom, left, right)) as u8;
        if out != 0 {
            cpu.write8(o + O_MODE, MODE_DONE);
            return;
        }
    }
    if cpu.read32(o + P_POWER) as i32 <= 0 {
        nested!(cpu, 0x0605_61ac, hits::score_hit_by(cpu, o, CR_SCORE));
        nested!(cpu, 0x0605_9ee4, ground::blown_up_wounded(cpu, o));
        cpu.write8(o + O_MODE, MODE_DONE);
        return;
    }
    let frames = cpu.read32(GAME_FRAMES);
    cpu.write32(o + O_SCRIPT, CR_BOXES[(frames & 1) as usize]);
    let frame = cpu.read8(CR_FRAMES + ((frames >> 2) & 3));
    cpu.write8(o + O_FRAME, frame);
    let script = cpu.read32(o + D_ROUTE);
    match nested!(cpu, 0x0605_a000, ground::route_step(cpu, o, script)) as u8 {
        CR_ROUTE_BEGUN => {
            let flags = cpu.read16(o + D_FLAGS) | D_DONE;
            cpu.write16(o + D_FLAGS, flags);
        }
        CR_ROUTE_OVER => {
            let flags = cpu.read16(o + D_FLAGS) & !D_DONE;
            cpu.write16(o + D_FLAGS, flags);
        }
        _ => {}
    }
    let side = cpu.read8(o + D_SIDE) as i8 as i32;
    let x = cpu.read32(o + O_POSITION).wrapping_add((cpu.read32(o + D_VX) as i32).wrapping_mul(side) as u32);
    cpu.write32(o + O_POSITION, x);
    let y = cpu.read32(o + O_POSITION + 4).wrapping_add(cpu.read32(o + D_VY));
    cpu.write32(o + O_POSITION + 4, y);
    let lift = cpu.read32(o + D_LIFT);
    cpu.write32(o + O_POSITION + 8, lift);
    let far = (lift as i32).wrapping_abs() > D_FAR;
    let flags = cpu.read32(o + O_FLAGS);
    cpu.write32(o + O_FLAGS, if far { flags | D_OFF } else { flags & !D_OFF });
}

const CRR_SIZE: u32 = 0xbc;
const CRR_STEP: u32 = 0x0608_7a38;
const CRR_KIND: u8 = 24;
const CRR_POWER: u32 = 20000;
const CRR_FLAGS: u32 = 0x2000_0000;
const CRR_ROUTES: u32 = 0x060a_c3a0;
const CRR_NARROW_AT: u32 = 0x14;
const CRR_FLIP_X: u32 = 0x1c;
const CRR_FLIP_Y: u32 = 0x1e;
const CRR_WING_SIDES: [u32; 2] = [0xffff_ffff, 1];

/// A carrier made from the event row `e` (its place +4, +6; its route +8; its side +0xb; its turn
/// +0x14; flips +0x1c and +0x1e; its type +0x1b): an object of list 2 (0xbc bytes), kind 24 with its
/// colour 16 a type on, power 20000, then its two wing guns made, one each side. Gives 1.
pub fn carrier_row(cpu: &mut Cpu, e: u32) -> u32 {
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, 2)) as i8;
    if n == -1 {
        return 1;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, CRR_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, PART_EXEC));
    if o == 0 {
        return 1;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + O_SCRIPT));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + O_BEHAVIOUR, CRR_STEP);
    let flags = cpu.read32(o + O_FLAGS) | CRR_FLAGS;
    cpu.write32(o + O_FLAGS, flags);
    let turn = cpu.read16(e + CRR_NARROW_AT);
    cpu.write16(o + P_TURN, turn);
    if cpu.read16(e + CRR_FLIP_X) != 0 {
        let flags = cpu.read32(o + O_FLAGS) | 0x10;
        cpu.write32(o + O_FLAGS, flags);
    }
    if cpu.read16(e + CRR_FLIP_Y) != 0 {
        let flags = cpu.read32(o + O_FLAGS) | 0x20;
        cpu.write32(o + O_FLAGS, flags);
    }
    cpu.write8(o + O_KIND, CRR_KIND);
    cpu.write8(o + O_FRAME, 0);
    let t = cpu.read8(e + E_TYPE);
    let base = cpu.read16(kind(CRR_KIND as u32) + K_COLOUR);
    let colour = base.wrapping_add((t as u16) << 4);
    cpu.write16(o + O_COLOUR_A, colour);
    cpu.write16(o + O_COLOUR_B, colour);
    cpu.write8(o + O_33, 0xf7);
    cpu.write8(o + O_NUMBER, n as u8);
    let at = cpu.read8(o + O_NUMBER) as i8 as i32 as u32;
    cpu.write32(NUMBERED.wrapping_add(at << 2), o);
    cpu.write32(o + O_SCRIPT, CR_BOXES[1]);
    cpu.write32(o + P_POWER, CRR_POWER);
    cpu.write8(o + P_54, 1);
    cpu.write8(o + P_55, t);
    let x = (cpu.read16(e + E_PLACE) as i16 as i32 as u32) << 16;
    cpu.write32(o + O_POSITION, x);
    let y = (cpu.read16(e + E_PLACE + 2) as i16 as i32 as u32) << 16;
    cpu.write32(o + O_POSITION + 4, y);
    cpu.write32(o + O_POSITION + 8, 0);
    let side = cpu.read8(e + E_MIRROR);
    cpu.write8(o + D_SIDE, side);
    for side in CRR_WING_SIDES {
        nested!(cpu, 0x0608_7950, wing_gun_make(cpu, o, side));
    }
    let route = cpu.read8(e + E_SCRIPT) as u32;
    let script = cpu.read32(CRR_ROUTES + 4 * route);
    cpu.write32(o + D_ROUTE, script);
    1
}

// ---- the gunners ---------------------------------------------------------------------------------

const SPRAY_STEP: u32 = 0x0608_7d28;
const SPRAY_KIND: u8 = 25;
const SPRAY_SCRIPT: u32 = 0x060a_c494;
const SPRAY_SCALE_AT: u32 = 0x34;
const SPRAY_RANDOM_AT: u32 = 0x2c;
const SPRAY_SIDE_AT: u32 = 0x2d;
const SPRAY_BIG: i32 = 0xbfff;
const SPRAY_SMALL_FLAG: u32 = 0x100;
const SPRAY_SMALL_AT: u32 = 0x58;
const SPRAY_SPEED_SHIFT: u32 = 4;
const SPRAY_TURN: u32 = 0x1000;

/// A spray shot (behaviour 0x06087d28): unless it ends (bullets::bullet_ends, going back), moved
/// by its speed, and shown on every frame whose parity is its count's (+0x88).
pub fn spray_shot_step(cpu: &mut Cpu, o: u32) {
    if nested!(cpu, 0x0605_3b10, bullets::bullet_ends(cpu, o, 1)) as u8 != 0 {
        return;
    }
    let x = cpu.read32(o + O_POSITION).wrapping_add(cpu.read32(o + D_VX));
    cpu.write32(o + O_POSITION, x);
    let y = cpu.read32(o + O_POSITION + 4).wrapping_add(cpu.read32(o + D_VY));
    cpu.write32(o + O_POSITION + 4, y);
    let parity = cpu.read32(GAME_FRAMES) & 1;
    let shown = cpu.read8(o + D_SIDE) as i8 as i32 as u32 & 0xffff;
    let flags = cpu.read32(o + O_FLAGS);
    cpu.write32(o + O_FLAGS, if parity == shown { flags | D_OFF } else { flags & !D_OFF });
}

/// A spray shot made at the place at `from` (x, y longs; the shooter's number at +0x1e; its scale
/// at +0x34) heading `angle`: an object of list 2 (0xbc bytes), its speed 16 times the sine for x
/// and minus 16 times the cosine for y, put on by a random 0 to 15 times 0x1000 of it (kept at
/// +0x2c of `from`, a long, whose byte +0x2d the SH-2 code then reads for the side: always 0 as
/// that is); with the side 0 it is turned by the angle plus 0x1000, otherwise by 0xf000 less.
pub fn spray_shot_make(cpu: &mut Cpu, from: u32, angle: u32) {
    let angle = angle as u16;
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, FAN_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, SHOT_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + O_SCRIPT));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    let number = cpu.read8(from + FAN_NUMBER);
    cpu.write8(o + O_NUMBER, number);
    cpu.write32(o + O_BEHAVIOUR, SPRAY_STEP);
    cpu.write32(o + O_FLAGS, SHOT_SPRITE_FLAGS);
    let x = cpu.read32(from);
    cpu.write32(o + O_POSITION, x);
    let y = cpu.read32(from + 4);
    cpu.write32(o + O_POSITION + 4, y);
    cpu.write32(o + O_POSITION + 8, 0);
    cpu.write8(o + O_33, 7);
    cpu.write8(o + O_FRAME, 4);
    cpu.write8(o + O_KIND, SPRAY_KIND);
    let colour = cpu.read16(CHAR_BASE);
    cpu.write16(o + O_COLOUR_B, colour);
    cpu.write16(o + O_COLOUR_A, colour);
    cpu.write32(o + O_SCRIPT, SPRAY_SCRIPT);
    cpu.write8(o + P_54, 1);
    cpu.write8(o + P_55, 5);
    let scale = cpu.read32(from + SPRAY_SCALE_AT);
    cpu.write32(o + O_SCALE, scale);
    cpu.write32(o + O_SCALE_2, 0x1_0000);
    if cpu.read32(o + O_SCALE) as i32 <= SPRAY_BIG {
        cpu.write16(o + SPRAY_SMALL_AT, 0);
        let flags = cpu.read32(o + O_FLAGS) | SPRAY_SMALL_FLAG;
        cpu.write32(o + O_FLAGS, flags);
    }
    cpu.write16(o + P_TURN, angle);
    let a = angle as i16 as i32 as u32;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    cpu.write32(o + D_VX, s << SPRAY_SPEED_SHIFT);
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    cpu.write32(o + D_VY, (c << SPRAY_SPEED_SHIFT).wrapping_neg());
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000)) as i32;
    let by = (((r.wrapping_add((r < 0) as i32)) >> 1) & 15).wrapping_mul(0x1000) as u32;
    let local = cpu.stack();
    cpu.write32(local, SPRAY_TURN);
    cpu.write32(from + SPRAY_RANDOM_AT, by);
    let vx = cpu.read32(o + D_VX);
    let m = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, vx, by));
    let x = cpu.read32(o + O_POSITION).wrapping_add(m);
    cpu.write32(o + O_POSITION, x);
    let at = cpu.read32(from + SPRAY_RANDOM_AT);
    let vy = cpu.read32(o + D_VY);
    let m = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, vy, at));
    let y = cpu.read32(o + O_POSITION + 4).wrapping_add(m);
    cpu.write32(o + O_POSITION + 4, y);
    let side = cpu.read8(from + SPRAY_SIDE_AT);
    cpu.write8(o + D_SIDE, side);
    if side as i8 == 0 {
        let flags = cpu.read32(o + O_FLAGS) | 0x10;
        cpu.write32(o + O_FLAGS, flags);
        let sum = cpu.read32(local).wrapping_add(cpu.read16(o + P_TURN) as i16 as i32 as u32);
        cpu.write32(local, sum);
        cpu.write16(o + P_TURN, sum as u16);
    } else {
        let turn = cpu.read16(o + P_TURN).wrapping_add(0xf000);
        cpu.write16(o + P_TURN, turn);
    }
}

const GR_AREA: [u32; 4] = [0xff70_0000, 0x0090_0000, 0xff30_0000, 0x00d0_0000];
const GR_AIM: u32 = 0xa0;
const GR_SPIN: u32 = 0x9e;
const GR_AIM_MAX: u32 = 60;
const GR_AIM_FAR: i32 = 43;
const GR_SOUND: u32 = 0x23;
const GR_OFFSET: u32 = 24;
const GR_SIDES: [(u16, u16, u16); 2] = [(0x5800, 0x1000, 1), (0xa800, 0xf000, 0)];

/// A gunner (behaviour 0x06087f64): ends when off its box (checked once flagged 0x1000) or, its
/// power gone, scored and blown up; otherwise its hit mark (flag 1), its far flag (the lift
/// +0x1c past 0x40000), the flip (0x10), the route's steps (7 flags 0x10 and clears its aim, 9
/// clears the flag), moved by its speed, its turn the side times +0x8c, its spin (+0x9e) taken up by
/// the aim, its frame the spin in 128ths, 0 to 3. While not flagged its aim eases to 0, once flagged
/// to 60; every other frame it sprays a shot from each side (the scale by the aim: 0x10000 at 60,
/// 0x1000 up to 43, between), and the sound 35 every 8.
pub fn gunner_step(cpu: &mut Cpu, o: u32) {
    let [top, bottom, left, right] = GR_AREA;
    if cpu.read16(o + D_FLAGS) & CR_CHECKED != 0 {
        let out = cpu.nested_with_stack(&aim::AREA_FLAGS_PROLOGUE, aim::AREA_FLAGS, &[right], |cpu| aim::area_flags(cpu, o + O_FLAGS, top, bottom, left, right)) as u8;
        if out != 0 {
            cpu.write8(o + O_MODE, MODE_DONE);
            return;
        }
    }
    if cpu.read32(o + P_POWER) as i32 <= 0 {
        nested!(cpu, 0x0605_61ac, hits::score_hit_by(cpu, o, D_SCORE));
        nested!(cpu, 0x0605_9fc4, ground::blown_up_burster(cpu, o));
        cpu.write8(o + O_MODE, MODE_DONE);
        return;
    }
    if cpu.read16(o + D_HIT_FLAGS) & D_HIT != 0 {
        let flags = cpu.read16(o + D_FLAGS) | 1;
        cpu.write16(o + D_FLAGS, flags);
    }
    let far = (cpu.read32(o + O_POSITION + 8) as i32).wrapping_abs() > D_FAR;
    let flags = cpu.read32(o + O_FLAGS);
    cpu.write32(o + O_FLAGS, if far { flags | D_OFF } else { flags & !D_OFF });
    let flags = cpu.read32(o + O_FLAGS) ^ 0x10;
    cpu.write32(o + O_FLAGS, flags);
    let script = cpu.read32(o + D_ROUTE);
    match nested!(cpu, 0x0605_a000, ground::route_step(cpu, o, script)) as u8 {
        CR_ROUTE_BEGUN => {
            let flags = cpu.read16(o + D_FLAGS) | D_DONE;
            cpu.write16(o + D_FLAGS, flags);
            cpu.write16(o + GR_AIM, 0);
        }
        CR_ROUTE_OVER => {
            let flags = cpu.read16(o + D_FLAGS) & !D_DONE;
            cpu.write16(o + D_FLAGS, flags);
        }
        _ => {}
    }
    let side = cpu.read8(o + D_SIDE) as i8 as i32;
    let x = cpu.read32(o + O_POSITION).wrapping_add((cpu.read32(o + D_VX) as i32).wrapping_mul(side) as u32);
    cpu.write32(o + O_POSITION, x);
    let y = cpu.read32(o + O_POSITION + 4).wrapping_add(cpu.read32(o + D_VY));
    cpu.write32(o + O_POSITION + 4, y);
    let lift = cpu.read32(o + D_LIFT);
    cpu.write32(o + O_POSITION + 8, lift);
    let turn = side.wrapping_mul(cpu.read16(o + D_ROUTE_TURN) as i16 as i32);
    cpu.write16(o + P_TURN, turn as u16);
    let spin = cpu.read16(o + GR_SPIN).wrapping_add(cpu.read16(o + GR_AIM));
    cpu.write16(o + GR_SPIN, spin);
    let spin = spin as i16 as i32;
    let biased = spin.wrapping_add(if spin < 0 { 127 } else { 0 });
    shift_7(cpu, biased as u32);
    cpu.write8(o + O_FRAME, ((biased >> 7) & 3) as u8);
    let aim_now = cpu.read16(o + GR_AIM) as i16 as i32 as u32;
    if cpu.read16(o + D_FLAGS) & D_DONE == 0 {
        let a = nested!(cpu, 0x0604_6168, actors::approach(cpu, aim_now, 0, 1));
        cpu.write16(o + GR_AIM, a as u16);
        return;
    }
    let a = nested!(cpu, 0x0604_6168, actors::approach(cpu, aim_now, GR_AIM_MAX, 1));
    cpu.write16(o + GR_AIM, a as u16);
    let frames = cpu.read32(GAME_FRAMES);
    if frames & 1 == 0 {
        return;
    }
    let d = cpu.stack();
    cpu.write32(d + 8, 0);
    let aim_now = cpu.read16(o + GR_AIM) as i16 as i32;
    let scale = if aim_now == GR_AIM_MAX as i32 {
        0x1_0000
    } else if aim_now <= GR_AIM_FAR {
        0x1000
    } else {
        0x1_0000i32.wrapping_sub((GR_AIM_MAX as i32 - aim_now) << 12) as u32
    };
    cpu.write32(d + SPRAY_SCALE_AT, scale);
    for (place, extra, side) in GR_SIDES {
        cpu.write16(d + SPRAY_RANDOM_AT, side);
        let turn = cpu.read16(o + P_TURN);
        let a = turn.wrapping_add(place) as i16 as i32 as u32;
        let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
        let x = cpu.read32(o + O_POSITION).wrapping_add((s as i32).wrapping_mul(GR_OFFSET as i32) as u32);
        cpu.write32(d, x);
        let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
        let y = cpu.read32(o + O_POSITION + 4).wrapping_sub((c as i32).wrapping_mul(GR_OFFSET as i32) as u32);
        cpu.write32(d + 4, y);
        let angle = turn.wrapping_add(extra) as u32;
        nested!(cpu, 0x0608_7d98, spray_shot_make(cpu, d, angle));
    }
    if cpu.read32(GAME_FRAMES) & 7 == 0 {
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, GR_SOUND));
    }
}

const GRD_KIND: u8 = 25;
const GRD_FLARE: u16 = 0xe0;
const GRD_FLARE_KEEP: u16 = 0x2000;
const GRD_JET_SIZE: [u32; 2] = [0xb000, 0x1_a000];
const GRD_JETS: [(u16, u16); 2] = [(0xb000, 0xf000), (0x5000, 0x1000)];

/// A gunner's draw (its sprite `s`; the hit mark as the others'): unless hidden, a sprite of kind 25
/// with its frame and turn, its colour (with the hit mark the base plus 0xe0 and the colour's
/// 0x2000 bit), drawn; once flagged 0x10, a jet beside it (alternate loops each side, 0x18 out
/// at the turn 0x5000 or 0xb000, its size by the spin (+0xa0), its frame 0x70 to 0x72 by the frame
/// count), drawn.
pub fn gunner_draw(cpu: &mut Cpu, s: u32) {
    let o = s.wrapping_sub(0x10);
    let at = s + 0x6e;
    let mut flags = cpu.read16(at) as i16 as i32 as u32;
    if flags & 2 != 0 {
        flags &= 0xffff_fffc;
        cpu.write16(at, flags as u16);
    }
    let flags = cpu.read16(at) as i16 as i32 as u32;
    if flags & 1 != 0 {
        cpu.write16(at, (flags | 2) as u16);
    }
    let sprite_flags = cpu.read32(o + O_FLAGS) & F_FLAGGED;
    if sprite_flags != 0 {
        return;
    }
    let d = cpu.stack();
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, d));
    cpu.write32(d + S_FLAGS, sprite_flags);
    cpu.write8(d + S_KIND, GRD_KIND);
    let frame = cpu.read8(s + S_FRAME);
    cpu.write8(d + S_FRAME, frame);
    cpu.write8(d + S_LIFT, 8);
    for k in 0..3 {
        let v = cpu.read32(o + O_POSITION + 4 * k);
        cpu.write32(d + S_POSITION + 4 * k, v);
    }
    let turn = cpu.read16(s + S_TURN);
    cpu.write16(d + S_TURN, turn);
    let base = cpu.read16(CHAR_BASE);
    let own = cpu.read16(s + S_COLOUR);
    let colour = if cpu.read16(at) & 1 == 0 { own } else { (own & GRD_FLARE_KEEP | GRD_FLARE).wrapping_add(base) };
    cpu.write16(d + S_COLOUR, colour);
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
    if cpu.read16(o + D_FLAGS) & D_DONE == 0 {
        return;
    }
    cpu.write32(d + S_FLAGS, 0);
    cpu.write8(d + S_KIND, 0);
    let frames = cpu.read32(GAME_FRAMES);
    let m = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, frames, 3));
    cpu.write8(d + S_FRAME, (m as u8).wrapping_add(WGD_FRAME));
    let spin = (cpu.read16(o + GR_AIM) as i16 as i32 as u32) << 10;
    let v = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, spin, GRD_JET_SIZE[0]));
    cpu.write32(d + S_SCALES, v);
    let v = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, spin, GRD_JET_SIZE[1]));
    cpu.write32(d + S_SCALES + 4, v);
    cpu.write16(d + S_COLOUR, base);
    let (place, extra) = if cpu.read32(LOOPS) & 1 == 0 { GRD_JETS[0] } else { GRD_JETS[1] };
    let turn = cpu.read16(o + P_TURN);
    let a = turn.wrapping_add(place) as i16 as i32 as u32;
    let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    let x = cpu.read32(o + O_POSITION).wrapping_add((sin as i32).wrapping_mul(GR_OFFSET as i32) as u32);
    cpu.write32(d + S_POSITION, x);
    let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    let y = cpu.read32(o + O_POSITION + 4).wrapping_sub((cos as i32).wrapping_mul(GR_OFFSET as i32) as u32);
    cpu.write32(d + S_POSITION + 4, y);
    cpu.write16(d + S_TURN, turn.wrapping_add(extra));
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
}

const GRM_STEP: u32 = 0x0608_7f64;
const GRM_DRAW: u32 = 0x0608_822c;
const GRM_KIND: u8 = 25;
const GRM_FLAGS: u32 = 0xc8_0000;
const GRM_POWER: u32 = 6000;
const GRM_ROUTES: u32 = 0x060a_c48c;
const GRM_BOXES: u32 = 0x060a_c49c;

/// A gunner made from the event row `e` (its place +4, +6; its route +8; its side +0xb; its turn
/// +0x14; its type +0x1b): an object of list 2 (0xbc bytes), flags 0xc80000, kind 25 with its colour
/// 16 a type on, power 6000. Gives 1.
pub fn gunner_row(cpu: &mut Cpu, e: u32) -> u32 {
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, 2)) as i8;
    if n == -1 {
        return 1;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, CRR_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, PART_EXEC));
    if o == 0 {
        return 1;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + O_SCRIPT));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + O_BEHAVIOUR, GRM_STEP);
    cpu.write32(o + O_FLAGS, GRM_FLAGS);
    cpu.write32(o + O_DRAW, GRM_DRAW);
    let turn = cpu.read16(e + E_DIRECTION);
    cpu.write16(o + P_TURN, turn);
    let side = cpu.read8(e + E_MIRROR);
    cpu.write8(o + D_SIDE, side);
    cpu.write8(o + O_KIND, GRM_KIND);
    cpu.write8(o + O_FRAME, 0);
    let t = cpu.read8(e + E_TYPE);
    let base = cpu.read16(kind(GRM_KIND as u32) + K_COLOUR);
    cpu.write16(o + O_COLOUR_A, ((t as u16) << 4).wrapping_add(base));
    cpu.write8(o + O_33, 0xf8);
    cpu.write8(o + O_NUMBER, n as u8);
    let at = cpu.read8(o + O_NUMBER) as i8 as i32 as u32;
    cpu.write32(NUMBERED.wrapping_add(at << 2), o);
    cpu.write32(o + O_SCRIPT, GRM_BOXES);
    cpu.write32(o + P_POWER, GRM_POWER);
    cpu.write8(o + P_54, 1);
    cpu.write8(o + P_55, t);
    let x = (cpu.read16(e + E_PLACE) as i16 as i32 as u32) << 16;
    cpu.write32(o + O_POSITION, x);
    let y = (cpu.read16(e + E_PLACE + 2) as i16 as i32 as u32) << 16;
    cpu.write32(o + O_POSITION + 4, y);
    cpu.write32(o + O_POSITION + 8, 0);
    let route = cpu.read8(e + E_SCRIPT) as u32;
    let script = cpu.read32(GRM_ROUTES + 4 * route);
    cpu.write32(o + D_ROUTE, script);
    1
}

/// A frame counter (+0x8a, going 0 to 9 and then back to 7, by one plus the count's low bit; the
/// count +0x8c, a word, going up one a call).
pub fn cycle_frame(cpu: &mut Cpu, o: u32) {
    let count = cpu.read16(o + CYCLE_COUNT);
    let at = (count & 1).wrapping_add(cpu.read16(o + CYCLE_FRAME));
    cpu.write16(o + CYCLE_FRAME, at);
    if at as i16 > CYCLE_LAST {
        cpu.write16(o + CYCLE_FRAME, at.wrapping_sub(CYCLE_BACK));
    }
    let count = cpu.read16(o + CYCLE_COUNT).wrapping_add(1);
    cpu.write16(o + CYCLE_COUNT, count);
}

const CYCLE_FRAME: u32 = 0x8a;
const CYCLE_COUNT: u32 = 0x8c;
const CYCLE_LAST: i16 = 9;
const CYCLE_BACK: u16 = 3;

// ==== from 0x06094000 on ===========================================================================
// (A second STG03 worktree, wip-stg03b, writes the functions from 0x06094000 below this line while
// wip-stg03 writes the ones before it above.)
