//! STG03.BIN's code (fingerprint fd6475daa2eebeb4), which game mode 9 runs in the overlay window:
//! stage 4, taken at STG01's end instead of stage 2 (the stage 4 route's evidence,
//! out/evidence/route4*).

use super::globals::{bios_setting_is_1, GAME_FRAMES, GAME_MODE};
use super::bullets::{self, AS_FLAGS, AS_POSITION, AS_SHOOTER, AS_SPEED};
use super::{clib, actors, aim, bg, camera, ground, hud, palette, parts, results, sgl, sprites, dma, effects, explosions, files, hits, kinds, math, models, nested, objects, screen, sound, states, stg02, timeline, Cpu};

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

// ---- the stage boss's event ----------------------------------------------------------------------

/// The stage boss's event object (gauge_make's, behaviour 0x06083b28): its state (+0x6d; the step
/// +0x6c before it), its place (+0x14, +0x18, +0x1c) moved each frame by its speed (+0x78, +0x7c,
/// +0x80) and its angle (+0x30) by the turn speed (+0x84, a word); the tilt (+0x2c, +0x2e, words);
/// the heading (+0x8a, a word) and the speed along it (+0x8c); the point it makes for (+0x90,
/// +0x94); which pod gets which side (+0x98, a byte); the results object (+0x9c); a count or
/// speed (+0x134); its four pods (+0xb8, a pointer each 8 on) and four bodies (+0xd8, +0xdc, +0xe0,
/// +0xe4: the boss_step objects, given their states).
const SB_VELOCITY: u32 = 0x78;
const SB_SPIN: u32 = 0x84;
const SB_TILT: u32 = 0x2c;
const SB_TILT_Y: u32 = 0x2e;
const SB_HEADING: u32 = 0x8a;
const SB_SPEED: u32 = 0x8c;
const SB_TARGET: u32 = 0x90;
const SB_WAVE: u32 = 0x98;
const SB_RESULTS: u32 = 0x9c;
const SB_WAIT: u32 = 0x134;
const SB_FADE: u32 = O_74;
const SB_PODS: u32 = 0xb8;
const SB_BODIES: [u32; 4] = [0xd8, 0xdc, 0xe0, 0xe4];
/// The states: 0 starts it (its models, its place high above), 2 brings it in (steps 0 to 6: a
/// colour fade, sparks, the bodies' moves, the place settled; 10 ends them), 10 waits for the
/// byte at 0x0600ed90 to clear, 15 sets the fight up, 16 and 17 are its two phases (the pods made
/// parts, shot at; steps 1 to 11 in the first, 0 to 12 in the second) and 100 its destruction.
const SB_ENTER: i8 = 2;
const SB_READY: i8 = 10;
const SB_BEGIN: i8 = 15;
const SB_FIRST: i8 = 16;
const SB_SECOND: i8 = 17;
const SB_DESTROYED: i8 = 100;
const SB_MODELS: u32 = 0x060a_b860;
const SB_FLAG: u32 = 0x0600_ed90;
const SB_PLACE: [u32; 3] = [0, 0xfc80_0000, 0x0400_0000];
const SB_COUNT: u32 = 0x2a30;
const SB_SPARK_FLAGS: u32 = 0x8_0400;
const SB_SPARK_BEHAVIOUR: u32 = 0x0608_38d4;
const SB_DRAW: u32 = 0x0608_3a44;
const SB_GAUGE: u32 = 100_000;
const SB_CHANGED: u32 = 2;
const COLOURS_TO: u32 = 0x0602_ee30;
const COLOURS_OUT: u32 = 0x0603_1e40;

fn signed_word(cpu: &mut Cpu, at: u32) -> u32 {
    cpu.read16(at) as i16 as i32 as u32
}

/// `x` divided by 2^n toward zero (the SH-2 code adds 2^n - 1 to a negative one and shifts it).
fn quotient(x: u32, n: u32) -> u32 {
    let x = x as i32;
    (x.wrapping_add(if x < 0 { (1 << n) - 1 } else { 0 }) >> n) as u32
}

fn absolute(x: u32) -> i32 {
    (x as i32).wrapping_abs()
}

/// Sets the state of the boss's four bodies (the word at their +0x6c, so their step 0).
fn bodies_state(cpu: &mut Cpu, o: u32, state: i8) {
    let bodies = SB_BODIES.map(|at| cpu.read32(o + at));
    for body in bodies.into_iter().rev() {
        cpu.write16(body + O_STEP, state as u16);
    }
}

fn pod(cpu: &mut Cpu, o: u32, k: u32) -> u32 {
    cpu.read32(o + SB_PODS + 8 * k)
}

/// The boss kind's 16 colours (at its first colour) blended from the colours at `from` toward the
/// stage's by `how_far` (the blend struct at `at`).
fn boss_blend(cpu: &mut Cpu, from: u32, at: u32, how_far: u32) {
    cpu.write16(at, how_far as u16);
    cpu.write16(at + 2, 16);
    cpu.write32(at + 4, from);
    let first = cpu.read16(kind(PART_KIND) + K_COLOUR) as u32;
    cpu.write32(at + 8, COLOURS_TO + 2 * first);
    cpu.write32(at + 12, COLOURS_OUT + 2 * first);
    nested!(cpu, 0x0600_5ba4, palette::colours_blend(cpu, at));
}

/// The parts made on the four pods, each with its side (the wave less its number, in 0 to 3).
fn pods_make_parts(cpu: &mut Cpu, o: u32) {
    for k in 0..4 {
        let wave = cpu.read8(o + SB_WAVE) as i8 as i32;
        let pod = pod(cpu, o, k);
        nested!(cpu, 0x0608_1e64, part_make(cpu, pod, (wave - k as i32) as u32 & 3));
    }
    let wave = cpu.read8(o + SB_WAVE).wrapping_add(1);
    cpu.write8(o + SB_WAVE, wave);
}

/// The pods' parts signalled (part_signal) one by one.
fn pods_signal(cpu: &mut Cpu, o: u32) {
    for k in 0..4 {
        let pod = pod(cpu, o, k);
        nested!(cpu, 0x0608_1dfc, part_signal(cpu, pod));
    }
}

/// Whether no pod has a part left (part_none of every one).
fn pods_bare(cpu: &mut Cpu, o: u32) -> bool {
    for k in 0..4 {
        let pod = pod(cpu, o, k);
        if nested!(cpu, 0x0608_1e48, part_none(cpu, pod)) == 0 {
            return false;
        }
    }
    true
}

/// The stage boss's event (see the fields above): the state's work, then the place and angle
/// moved by the speeds.
pub fn stage_boss_event(cpu: &mut Cpu, o: u32) {
    match cpu.read8(o + O_STATE) as i8 {
        0 => sb_start(cpu, o),
        SB_ENTER => sb_enter(cpu, o),
        SB_READY => sb_ready(cpu, o),
        SB_BEGIN => sb_begin(cpu, o),
        SB_FIRST => sb_first(cpu, o),
        SB_SECOND => sb_second(cpu, o),
        SB_DESTROYED => sb_destroyed(cpu, o),
        _ => {}
    }
    for k in 0..3 {
        let place = cpu.read32(o + O_POSITION + 4 * k).wrapping_add(cpu.read32(o + SB_VELOCITY + 4 * k));
        cpu.write32(o + O_POSITION + 4 * k, place);
    }
    let angle = cpu.read16(o + O_ANGLE).wrapping_add(cpu.read16(o + SB_SPIN));
    cpu.write16(o + O_ANGLE, angle);
}

/// State 0: the models set (hidden meanwhile; nothing more if that fails), the draw, the scores
/// cleared, the place, the speeds and fields cleared, the count 0x2a30 (the gauge's), state 2.
fn sb_start(cpu: &mut Cpu, o: u32) {
    let flags = cpu.read32(o + O_FLAGS) | 0x8000_0000;
    cpu.write32(o + O_FLAGS, flags);
    if nested!(cpu, 0x0605_2ff0, models::model_group_set(cpu, o, SB_MODELS)) == 0 {
        return;
    }
    let flags = cpu.read32(o + O_FLAGS) & 0x7fff_ffff;
    cpu.write32(o + O_FLAGS, flags);
    cpu.write32(o + O_DRAW, SB_DRAW);
    cpu.write32(TOTAL_SCORE, 0);
    cpu.write32(TOTAL_SCORE + 4, 0);
    for (k, p) in SB_PLACE.into_iter().enumerate() {
        cpu.write32(o + O_POSITION + 4 * k as u32, p);
    }
    cpu.write16(o + O_ANGLE, 0);
    cpu.write32(o + SB_VELOCITY + 8, 0);
    cpu.write32(o + SB_VELOCITY + 4, 0);
    cpu.write32(o + SB_VELOCITY, 0);
    cpu.write32(o + SB_SPIN, 0);
    cpu.write32(o + SB_WAVE, 0);
    cpu.write32(o + O_70, SB_COUNT);
    cpu.write16(o + O_STEP, SB_ENTER as u16);
}

/// State 2, the entrance. Step 0 sets the colours to fade (16 colours from black), the speed
/// down, the wait 30; 1 makes a spark every 20 frames until it is low enough; 2 slows its fall and
/// turns it, then sends the bodies to state 16; 3 and 4 slow it to a stop, tilting, and send them
/// to 17; 5 settles it, the colours fading with its depth, until it is in place; 6 turns it
/// round, then sends the bodies to 15 and the event on to 10.
fn sb_enter(cpu: &mut Cpu, o: u32) {
    match cpu.read8(o + O_STEP) {
        0 => {
            let f = cpu.stack();
            for i in 0..16 {
                cpu.write16(f + 2 * i, 0x8000);
            }
            cpu.write16(f, 0);
            boss_blend(cpu, f, f + 0x20, 0x500);
            cpu.write32(o + SB_VELOCITY + 4, 0x6_0000);
            cpu.write32(o + SB_WAIT, 30);
            cpu.write8(o + O_STEP, 1);
        }
        1 => {
            let wait = cpu.read32(o + SB_WAIT).wrapping_sub(1);
            cpu.write32(o + SB_WAIT, wait);
            if wait as i32 <= 0 {
                let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, SHOT_SIZE)) as i8 as i32 as u32;
                let spark = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, SHOT_EXEC));
                if spark != 0 {
                    nested!(cpu, 0x0604_62ac, actors::actor_setup(cpu, spark));
                    let flags = cpu.read32(spark + O_FLAGS) | SB_SPARK_FLAGS;
                    cpu.write32(spark + O_FLAGS, flags);
                    cpu.write32(spark + O_BEHAVIOUR, SB_SPARK_BEHAVIOUR);
                    let b = cpu.read8(o + O_NUMBER);
                    cpu.write8(spark + O_NUMBER, b);
                    let b = cpu.read8(o + O_54);
                    cpu.write8(spark + O_54, b);
                    cpu.write32(spark + SPARK_BOSS, o);
                }
                cpu.write32(o + SB_WAIT, 20);
            }
            if cpu.read32(o + O_POSITION + 4) as i32 > 0xff_ffff {
                cpu.write32(o + SB_WAIT, 0);
                cpu.write8(o + O_STEP, 2);
            }
        }
        2 => {
            let fall = cpu.read32(o + SB_VELOCITY + 4);
            let fall = nested!(cpu, 0x0604_6168, actors::approach(cpu, fall, 0, 0x1000));
            cpu.write32(o + SB_VELOCITY + 4, fall);
            sb_tilt_toward(cpu, o, 0xffff_c400, 0x200);
            if fall != 0 {
                return;
            }
            let wait = cpu.read32(o + SB_WAIT).wrapping_add(1);
            cpu.write32(o + SB_WAIT, wait);
            if wait as i32 > 29 {
                bodies_state(cpu, o, BS_LEAN);
                cpu.write8(o + O_STEP, 3);
            }
        }
        3 => {
            let fall = cpu.read32(o + SB_VELOCITY + 4);
            let fall = nested!(cpu, 0x0604_6168, actors::approach(cpu, fall, 0xfff8_0000, 0x800));
            cpu.write32(o + SB_VELOCITY + 4, fall);
            let spin = signed_word(cpu, o + SB_SPIN);
            let spin = nested!(cpu, 0x0604_6168, actors::approach(cpu, spin, 0x800, 8));
            cpu.write16(o + SB_SPIN, spin as u16);
            if cpu.read32(o + O_POSITION + 4) as i32 <= 0xc0_0000 {
                cpu.write32(o + SB_WAIT, 0);
                cpu.write8(o + O_STEP, 4);
            }
        }
        4 => {
            let fall = cpu.read32(o + SB_VELOCITY + 4);
            let fall = nested!(cpu, 0x0604_6168, actors::approach(cpu, fall, 0, 0x1000));
            cpu.write32(o + SB_VELOCITY + 4, fall);
            sb_tilt_toward(cpu, o, 0xffff_f000, 0x200);
            let wait = cpu.read32(o + SB_WAIT).wrapping_add(1);
            cpu.write32(o + SB_WAIT, wait);
            if wait as i32 > 89 {
                cpu.write32(o + SB_VELOCITY + 4, 0);
                bodies_state(cpu, o, BS_LEAN_LONG);
                cpu.write32(o + SB_WAIT, 0);
                cpu.write8(o + O_STEP, 5);
            }
        }
        5 => sb_settle(cpu, o),
        6 => sb_turn_round(cpu, o),
        10 => cpu.write16(o + O_STEP, SB_READY as u16),
        _ => {}
    }
}

/// The tilt (+0x2c) moved toward the angle `to` by a 32nd of the gap, held to `limit`.
fn sb_tilt_toward(cpu: &mut Cpu, o: u32, to: u32, limit: u32) {
    let tilt = signed_word(cpu, o + SB_TILT);
    let gap = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, tilt, to));
    let step = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, quotient(gap, 5), limit));
    let tilt = cpu.read16(o + SB_TILT).wrapping_add(step as u16);
    cpu.write16(o + SB_TILT, tilt);
}

/// Step 5: y moved toward -8.0 (a 64th of the way, held to 2.0) and z toward -0x40.0 (a 16th, held
/// to a speed kept in the wait, going up to 16.0 by 0x4000); the turn speed to 0x400 by 2. While z
/// is further than 0xffff from it the colours fade by its depth (a second 16 from black); once
/// near, the kind's colours are the stage's, z is set, the changed flags raised and step 6 begins.
fn sb_settle(cpu: &mut Cpu, o: u32) {
    let f = cpu.stack();
    let y = cpu.read32(o + O_POSITION + 4);
    let gap = 0xff80_0000u32.wrapping_sub(y) as i32;
    let share = share_of(cpu, gap, Share::Helper6) as u32;
    let by = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, share, 0x2_0000));
    let y = cpu.read32(o + O_POSITION + 4).wrapping_add(by);
    cpu.write32(o + O_POSITION + 4, y);
    let speed = cpu.read32(o + SB_WAIT);
    let speed = nested!(cpu, 0x0604_6168, actors::approach(cpu, speed, 0x10_0000, 0x4000));
    cpu.write32(o + SB_WAIT, speed);
    let gap = 0xffc0_0000u32.wrapping_sub(cpu.read32(o + O_POSITION + 8));
    let by = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, quotient(gap, 4), speed));
    let z = cpu.read32(o + O_POSITION + 8).wrapping_add(by);
    cpu.write32(o + O_POSITION + 8, z);
    let spin = signed_word(cpu, o + SB_SPIN);
    let spin = nested!(cpu, 0x0604_6168, actors::approach(cpu, spin, 0x400, 2));
    cpu.write16(o + SB_SPIN, spin as u16);
    let z = cpu.read32(o + O_POSITION + 8);
    if absolute(0xffc0_0000u32.wrapping_sub(z)) > 0xffff {
        for i in 0..16 {
            cpu.write16(f + 0x30 + 2 * i, 0x8000);
        }
        cpu.write16(f + 0x30, 0);
        let depth = nested!(cpu, 0x0600_6a8e, math::fixed_div(cpu, z, 0x0400_0000));
        let fade = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, 0x800, depth));
        boss_blend(cpu, f + 0x30, f + 0x50, 0x1000u32.wrapping_sub(fade));
        return;
    }
    for i in 0..10 {
        let first = cpu.read16(kind(PART_KIND) + K_COLOUR) as u32;
        let at = 2 * (first + i);
        let c = cpu.read16(COLOURS_TO + at);
        cpu.write16(COLOURS_OUT + at, c);
    }
    cpu.write32(o + O_POSITION + 8, 0xffc0_0000);
    let changed = cpu.read32(palette::CHANGED) | SB_CHANGED;
    cpu.write32(palette::CHANGED, changed);
    cpu.write32(o + SB_WAIT, 0);
    cpu.write8(o + O_STEP, 6);
}

/// Step 6: the turn speed taken to 0 by 8 while it is over 0x200, else toward the way round
/// (angle 0x8000: a 16th of the way ahead, held to 0x200); the tilt back to 0 (by at most 0x180),
/// y and z drawn to 0 by a 32nd, held to the speed (going to 1.0 by 0x200). Once it has stopped
/// turning, level and near 0 it sets the angle 0x8000, sends the bodies to 15 and goes to state 10.
fn sb_turn_round(cpu: &mut Cpu, o: u32) {
    let spin = signed_word(cpu, o + SB_SPIN);
    let spin = if absolute(spin) > 0x200 {
        nested!(cpu, 0x0604_6168, actors::approach(cpu, spin, 0, 8))
    } else {
        let angle = signed_word(cpu, o + O_ANGLE);
        let ahead = nested!(cpu, 0x0604_61cc, camera::angle_ahead(cpu, angle, 0xffff_8000));
        let spin = signed_word(cpu, o + SB_SPIN);
        let gap = ahead.wrapping_sub(spin);
        nested!(cpu, 0x0606_4f3c, math::clamp(cpu, quotient(gap, 4), 0x200))
    };
    cpu.write16(o + SB_SPIN, spin as u16);
    let tilt = signed_word(cpu, o + SB_TILT);
    let gap = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, tilt, 0));
    let by = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, gap, 0x180));
    let tilt = cpu.read16(o + SB_TILT).wrapping_add(by as u16);
    cpu.write16(o + SB_TILT, tilt);
    let speed = cpu.read32(o + SB_WAIT);
    let speed = nested!(cpu, 0x0604_6168, actors::approach(cpu, speed, 0x1_0000, 0x200));
    cpu.write32(o + SB_WAIT, speed);
    let y = cpu.read32(o + O_POSITION + 4);
    let by = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, quotient(y.wrapping_neg(), 5), speed));
    let y = cpu.read32(o + O_POSITION + 4).wrapping_add(by);
    cpu.write32(o + O_POSITION + 4, y);
    let speed = cpu.read32(o + SB_WAIT);
    let z = cpu.read32(o + O_POSITION + 8);
    let by = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, quotient(z.wrapping_neg(), 5), speed));
    let z = cpu.read32(o + O_POSITION + 8).wrapping_add(by);
    cpu.write32(o + O_POSITION + 8, z);
    if signed_word(cpu, o + SB_SPIN) != 0 || cpu.read16(o + SB_TILT) != 0 {
        return;
    }
    let y = cpu.read32(o + O_POSITION + 4);
    if absolute(y) > 0xffff || absolute(z) > 0xffff {
        return;
    }
    cpu.write32(o + O_POSITION + 4, 0);
    cpu.write32(o + O_POSITION + 8, 0);
    cpu.write16(o + O_ANGLE, 0x8000);
    bodies_state(cpu, o, BS_SWAY);
    cpu.write8(o + O_STEP, 10);
}

/// State 10: once the byte at 0x0600ed90 is clear the boss's gauge is set (to 100000), the wave
/// cleared and the state is 15.
fn sb_ready(cpu: &mut Cpu, o: u32) {
    if cpu.read8(SB_FLAG) != 0 {
        return;
    }
    nested!(cpu, 0x0604_64d8, hud::boss_gauge_set(cpu, SB_GAUGE));
    cpu.write32(o + SB_WAVE, 0);
    cpu.write16(o + O_STEP, SB_BEGIN as u16);
}

/// State 15: the gauge object's step run on it, the point it makes for set to (0, 0x400000), the
/// heading 0x8000 and speed 0, and state 16.
fn sb_begin(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_3aac, gauge_step(cpu, o));
    cpu.write32(o + SB_TARGET, 0);
    cpu.write32(o + SB_TARGET + 4, 0x40_0000);
    cpu.write16(o + SB_HEADING, 0x8000);
    cpu.write32(o + SB_SPEED, 0);
    cpu.write16(o + O_STEP, SB_FIRST as u16);
}

/// The heading (+0x8a) turned toward the direction of the point it makes for, at most `limit`;
/// that direction (the angle of the way to it plus 0x4000) is left as a word at `direction`.
fn sb_head_for_target(cpu: &mut Cpu, o: u32, direction: u32, limit: u32) {
    let dy = cpu.read32(o + SB_TARGET + 4).wrapping_sub(cpu.read32(o + O_POSITION + 4));
    let dx = cpu.read32(o + SB_TARGET).wrapping_sub(cpu.read32(o + O_POSITION));
    let way = nested!(cpu, 0x0606_780c, sgl::sl_atan(cpu, dx, dy));
    cpu.write16(direction, 0x4000u32.wrapping_add(way) as u16);
    let to = signed_word(cpu, direction);
    let heading = signed_word(cpu, o + SB_HEADING);
    let gap = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, heading, to));
    let by = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, gap, limit));
    let heading = cpu.read16(o + SB_HEADING).wrapping_add(by as u16);
    cpu.write16(o + SB_HEADING, heading);
}

/// The speed along the heading turned into a speed (+0x78, +0x7c) by turn_point on the vector at
/// `t` (x 0, y minus the speed or what `y` gives, scales 1.0, the heading as its angle at +0x18).
fn sb_set_velocity(cpu: &mut Cpu, o: u32, t: u32, y: u32) {
    cpu.write32(t, 0);
    cpu.write32(t + 4, y);
    cpu.write32(t + 0x10, 0x1_0000);
    cpu.write32(t + 0xc, 0x1_0000);
    let heading = cpu.read16(o + SB_HEADING);
    cpu.write16(t + 0x18, heading);
    cpu.write32(t + 0x14, 0);
    nested!(cpu, 0x0606_4eac, math::turn_point(cpu, t));
    let x = cpu.read32(t);
    cpu.write32(o + SB_VELOCITY, x);
    let y = cpu.read32(t + 4);
    cpu.write32(o + SB_VELOCITY + 4, y);
}

/// Whether the boss is within 0xfffff of the point it makes for.
fn sb_at_target(cpu: &mut Cpu, o: u32) -> bool {
    let dy = cpu.read32(o + SB_TARGET + 4).wrapping_sub(cpu.read32(o + O_POSITION + 4));
    let dx = cpu.read32(o + SB_TARGET).wrapping_sub(cpu.read32(o + O_POSITION));
    nested!(cpu, 0x0604_5e60, aim::length(cpu, dx, dy)) as i32 <= 0xf_ffff
}

/// State 16, the first phase. Step 0 sends the bodies to 18 (two of them at their step 10), step
/// 1 makes the parts on the pods (the wait 0x258 less 200 by the difficulty) and 2 waits it out,
/// 10 signals the parts and 11 waits 60 frames, then, with every pod bare, goes back to step 1 or
/// (the fade at +0x74 gone) to state 17. Meanwhile the boss heads for its point, at a speed that
/// is higher the nearer its heading is, taking a new random point (x 0 to 79, y 0 to 63 whole)
/// when it is near; and the gauge object's step runs on it.
fn sb_first(cpu: &mut Cpu, o: u32) {
    match cpu.read8(o + O_STEP) {
        0 => {
            bodies_state(cpu, o, BS_NOD);
            for at in [SB_BODIES[2], SB_BODIES[0]] {
                let body = cpu.read32(o + at);
                cpu.write8(body + O_STEP, 10);
            }
            cpu.write32(o + SB_FADE, 5);
            cpu.write8(o + O_STEP, 1);
        }
        1 => {
            pods_make_parts(cpu, o);
            let wait = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 0xc8));
            cpu.write32(o + SB_WAIT, 0x258u32.wrapping_sub(wait));
            cpu.write8(o + O_STEP, 2);
        }
        2 => {
            let wait = cpu.read32(o + SB_WAIT).wrapping_sub(1);
            cpu.write32(o + SB_WAIT, wait);
            if wait as i32 <= 0 {
                cpu.write8(o + O_STEP, 10);
            }
        }
        10 => {
            pods_signal(cpu, o);
            cpu.write32(o + SB_WAIT, 0);
            let fade = cpu.read32(o + SB_FADE).wrapping_sub(1);
            cpu.write32(o + SB_FADE, fade);
            cpu.write8(o + O_STEP, 11);
        }
        11 => {
            let wait = cpu.read32(o + SB_WAIT).wrapping_add(1);
            cpu.write32(o + SB_WAIT, wait);
            if wait as i32 > 59 && pods_bare(cpu, o) {
                if cpu.read32(o + SB_FADE) == 0 {
                    cpu.write16(o + O_STEP, SB_SECOND as u16);
                } else {
                    cpu.write8(o + O_STEP, 1);
                }
            }
        }
        _ => {}
    }
    let f = cpu.stack();
    sb_head_for_target(cpu, o, f + 0x78, 0x80);
    let to = signed_word(cpu, f + 0x78);
    let heading = signed_word(cpu, o + SB_HEADING);
    let gap = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, heading, to));
    let (base, more) = if absolute(gap) > 0x1000 {
        (nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 0x800)), 0x2000)
    } else {
        (nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 0x1000)), 0x4000)
    };
    let speed = cpu.read32(o + SB_SPEED);
    let speed = nested!(cpu, 0x0604_6168, actors::approach(cpu, speed, base.wrapping_add(more), 0x100));
    cpu.write32(o + SB_SPEED, speed);
    let speed = cpu.read32(o + SB_SPEED);
    sb_set_velocity(cpu, o, f + 0x60, speed.wrapping_neg());
    if sb_at_target(cpu, o) {
        let x = nested!(cpu, 0x0604_5d90, math::random(cpu, 80)) << 16;
        cpu.write32(o + SB_TARGET, x);
        let y = nested!(cpu, 0x0604_5d90, math::random(cpu, 64)) << 16;
        cpu.write32(o + SB_TARGET + 4, y);
    }
    nested!(cpu, 0x0608_3aac, gauge_step(cpu, o));
}

/// State 17, the second phase. Its turn speed goes to 0x400 by 2 until step 10; step 0 sends the
/// bodies to 19 and begins; 1 makes the parts (wait 0x4b0 less 400 by the difficulty); 2 waits;
/// 3 signals the parts or, with every pod gone (past 99), goes to step 5; 4 waits 60 frames for
/// the parts to be gone, then step 10; 5 takes the fade up to 2.0 by 0x400; 10 waits for the
/// angle to come round to 0x8000 (ahead) and sends the bodies to 15; 11 turns it the rest of the
/// way, 12 settles it there and goes back to state 16. It heads for its point as in the first
/// phase (faster: heading limit 0x100, speeds 0x4000 and 0x1000 over), stopped from step 6 on.
fn sb_second(cpu: &mut Cpu, o: u32) {
    if cpu.read8(o + O_STEP) as i8 <= 9 {
        let spin = signed_word(cpu, o + SB_SPIN);
        let spin = nested!(cpu, 0x0604_6168, actors::approach(cpu, spin, 0x400, 2));
        cpu.write16(o + SB_SPIN, spin as u16);
    }
    match cpu.read8(o + O_STEP) {
        0 => {
            cpu.write16(o + SB_HEADING, 0x8000);
            cpu.write32(o + SB_TARGET, 0);
            cpu.write32(o + SB_TARGET + 4, 0x40_0000);
            cpu.write32(o + SB_SPEED, 0);
            bodies_state(cpu, o, BS_NOD_ONCE);
            cpu.write32(o + SB_FADE, 0x1_0000);
            cpu.write8(o + O_STEP, 1);
        }
        1 => {
            pods_make_parts(cpu, o);
            let wait = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 0x190));
            cpu.write32(o + SB_WAIT, 0x4b0u32.wrapping_sub(wait));
            cpu.write8(o + O_STEP, 2);
        }
        2 => {
            let wait = cpu.read32(o + SB_WAIT).wrapping_sub(1);
            cpu.write32(o + SB_WAIT, wait);
            if wait as i32 <= 0 {
                cpu.write8(o + O_STEP, 3);
            }
        }
        3 => {
            let mut gone = 0;
            for k in 0..4 {
                let pod = pod(cpu, o, k);
                if cpu.read8(pod + O_STATE) as i8 > GONE {
                    gone += 1;
                }
            }
            if gone > 3 {
                cpu.write8(o + O_STEP, 5);
            } else {
                pods_signal(cpu, o);
                cpu.write32(o + SB_WAIT, 0);
                cpu.write8(o + O_STEP, 4);
            }
        }
        4 => {
            let wait = cpu.read32(o + SB_WAIT).wrapping_add(1);
            cpu.write32(o + SB_WAIT, wait);
            if wait as i32 > 59 && pods_bare(cpu, o) {
                cpu.write8(o + O_STEP, 10);
            }
        }
        5 => {
            let fade = cpu.read32(o + SB_FADE);
            let fade = nested!(cpu, 0x0604_6168, actors::approach(cpu, fade, 0x2_0000, 0x400));
            cpu.write32(o + SB_FADE, fade);
        }
        10 => {
            let angle = signed_word(cpu, o + O_ANGLE);
            let ahead = nested!(cpu, 0x0604_61cc, camera::angle_ahead(cpu, angle, 0x8000)) as i32;
            if ahead > 0x7fff {
                bodies_state(cpu, o, BS_SWAY);
                cpu.write8(o + O_STEP, 11);
            }
        }
        11 => {
            let angle = signed_word(cpu, o + O_ANGLE);
            let ahead = nested!(cpu, 0x0604_61cc, camera::angle_ahead(cpu, angle, 0x8000));
            let spin = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, quotient(ahead, 5), 0x400));
            cpu.write16(o + SB_SPIN, spin as u16);
            if spin as i16 as i32 > 32 {
                return sb_second_aim(cpu, o);
            }
            cpu.write16(o + SB_SPIN, 0);
            cpu.write8(o + O_STEP, 12);
            sb_settle_round(cpu, o);
        }
        12 => sb_settle_round(cpu, o),
        _ => {}
    }
    sb_second_aim(cpu, o);
}

/// Step 12: the angle taken to 0x8000 by at most 32 a frame; there, state 16.
fn sb_settle_round(cpu: &mut Cpu, o: u32) {
    let angle = signed_word(cpu, o + O_ANGLE);
    let ahead = nested!(cpu, 0x0604_61cc, camera::angle_ahead(cpu, angle, 0x8000));
    let by = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, ahead, 32));
    let angle = signed_word(cpu, o + O_ANGLE).wrapping_add(by);
    cpu.write16(o + O_ANGLE, angle as u16);
    if angle as i16 == 0x8000u16 as i16 {
        cpu.write16(o + O_ANGLE, 0x8000);
        cpu.write16(o + O_STEP, SB_FIRST as u16);
    }
}

/// The second phase's heading, speed and velocity (see sb_second) and, up to step 9, a new point
/// when it is near; the gauge object's step run on it.
fn sb_second_aim(cpu: &mut Cpu, o: u32) {
    let f = cpu.stack();
    sb_head_for_target(cpu, o, f + 0xb8, 0x100);
    let step = cpu.read8(o + O_STEP) as i8;
    let speed = if step > 5 {
        let speed = cpu.read32(o + SB_SPEED);
        nested!(cpu, 0x0604_6168, actors::approach(cpu, speed, 0, 0x400))
    } else {
        let to = signed_word(cpu, f + 0xb8);
        let heading = signed_word(cpu, o + SB_HEADING);
        let gap = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, heading, to));
        let (base, more) = if absolute(gap) > 0x1000 {
            (nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 0x1000)), 0x4000)
        } else {
            (nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 0x4000)), 0x1_0000)
        };
        let speed = cpu.read32(o + SB_SPEED);
        nested!(cpu, 0x0604_6168, actors::approach(cpu, speed, base.wrapping_add(more), 0x200))
    };
    cpu.write32(o + SB_SPEED, speed);
    let t = f + 0xa0;
    cpu.write32(t, 0);
    let mut y = cpu.read32(o + SB_SPEED).wrapping_neg();
    if cpu.read8(o + O_STEP) as i8 == 5 {
        let fade = cpu.read32(o + SB_FADE);
        y = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, y, fade));
    }
    sb_set_velocity(cpu, o, t, y);
    if cpu.read8(o + O_STEP) as i8 <= 9 && sb_at_target(cpu, o) {
        let out = o + SB_TARGET;
        nested!(cpu, 0x0604_0ccc, aim::target(cpu, o, 0, out));
    }
    nested!(cpu, 0x0608_3aac, gauge_step(cpu, o));
}

/// State 100, the destruction. Step 0 hides it, sends the bodies to 21, stops it and clears the
/// bullets; 1 sinks it (z toward 0x1000000 by 0x8000) for 60 frames and makes the results object
/// (its pointer at +0x9c), step 2; 2 waits for the results' byte (+3) to pass 111, then sets a
/// scroll and an explosion at its place and turns it; 3 waits 130 frames and ends it (its mode
/// 0x70, step 101). From step 0 on x and y are drawn to 0 by 0x4000 a frame, and from step 2 z
/// sinks and the tilt rises.
fn sb_destroyed(cpu: &mut Cpu, o: u32) {
    match cpu.read8(o + O_STEP) {
        0 => {
            let flags = cpu.read32(o + O_FLAGS) | F_HIDDEN;
            cpu.write32(o + O_FLAGS, flags);
            bodies_state(cpu, o, BS_RAISE);
            cpu.write32(o + SB_VELOCITY + 8, 0);
            cpu.write32(o + SB_VELOCITY + 4, 0);
            cpu.write32(o + SB_VELOCITY, 0);
            nested!(cpu, 0x0605_61e0, screen::bullets_cleared(cpu));
            cpu.write32(o + SB_FADE, 0);
            cpu.write8(o + O_STEP, 1);
        }
        1 => {
            let z = cpu.read32(o + O_POSITION + 8);
            let z = nested!(cpu, 0x0604_6168, actors::approach(cpu, z, 0x0100_0000, 0x8000));
            cpu.write32(o + O_POSITION + 8, z);
            let wait = cpu.read32(o + SB_FADE).wrapping_add(1);
            cpu.write32(o + SB_FADE, wait);
            if wait as i32 > 59 {
                let results = nested!(cpu, 0x0605_284c, results::results_set(cpu, 0));
                cpu.write32(o + SB_RESULTS, results);
                if results != 0 {
                    cpu.write8(o + O_STEP, 2);
                }
            }
        }
        2 => {
            let results = cpu.read32(o + SB_RESULTS);
            if cpu.read8(results + 3) as i8 > 111 {
                nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
                let p = cpu.stack() + 0xe0;
                for k in 0..3 {
                    let place = cpu.read32(o + O_POSITION + 4 * k);
                    cpu.write32(p + 4 * k, place);
                }
                cpu.write32(p + 0x3c, o);
                nested!(cpu, 0x0605_8ba4, explosions::explosion_set(cpu, p));
                cpu.write32(o + SB_FADE, 0);
                let spin = cpu.read16(o + SB_SPIN).wrapping_add(0x100);
                cpu.write16(o + SB_SPIN, spin);
                cpu.write8(o + O_STEP, 3);
            }
        }
        3 => {
            let wait = cpu.read32(o + SB_FADE).wrapping_add(1);
            cpu.write32(o + SB_FADE, wait);
            if wait as i32 > 0x81 {
                cpu.write8(o + O_MODE, MODE_DONE);
                cpu.write8(o + O_STEP, 101);
            }
        }
        _ => {}
    }
    for k in 0..2 {
        let v = cpu.read32(o + O_POSITION + 4 * k);
        let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0, 0x4000));
        cpu.write32(o + O_POSITION + 4 * k, v);
    }
    if cpu.read8(o + O_STEP) as i8 > 1 {
        let z = cpu.read32(o + O_POSITION + 8);
        let z = nested!(cpu, 0x0604_6168, actors::approach(cpu, z, 0x0100_0000, 0x8000));
        cpu.write32(o + O_POSITION + 8, z);
        let tilt = cpu.read16(o + SB_TILT_Y).wrapping_add(0x100);
        cpu.write16(o + SB_TILT_Y, tilt);
    }
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
    let r = nested!(cpu, PAIR_MAKER, slider_part_make(cpu, d));
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
        let o = nested!(cpu, SCENERY_ROUND, round_make(cpu, d));
        cpu.write32(SCENERY + 0x10 + 4 * i as u32, o);
    }
    for (i, (x, y)) in [(0x8_0000u32, 0xfe60_0000u32), (0xffa8_0000, 0xfec8_0000), (0x8_0000, 0xff30_0000)].into_iter().enumerate() {
        cpu.write32(d + G_PLACE, x);
        cpu.write32(d + G_PLACE + 4, y);
        cpu.write16(d + G_TYPE, 3 + i as u16);
        let o = nested!(cpu, SCENERY_PLATE, plate_make(cpu, d));
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

// ---- the orbiters and the route fliers -----------------------------------------------------------

/// An orbiter's fields: its parent (+0x6c, a pointer), its phase (+0x7c: 0 held to the parent, 1
/// loose), the side it is on (+0x88: a quarter turn each), its speed (+0x80, +0x84), the sprite
/// frame (+0x8a) and the frame count in cycle_frame (+0x8a, +0x8c).
const OR_PARENT: u32 = 0x6c;
const OR_PHASE: u32 = 0x7c;
const OR_SPEED: u32 = 0x80;
const OR_SIDE: u32 = 0x88;
const OR_FLAGS: u32 = 0x7e;
const OR_HIT_BITS: u32 = O_ARMS;
const OR_RADIUS: i32 = 24;
const OR_SPEED_STEP: u32 = 0xb00;
/// The area an orbiter lives in (top, bottom, left, right).
const OR_AREA: [u32; 4] = [0xff30_0000, 0x00d0_0000, 0xfef0_0000, 0x0110_0000];

/// An orbiter's step (behaviour 0x0608857c; the parent at +0x6c): done (blown up) when its power
/// (+0x50) is gone. Hit bit 0x200 raises its flag 1. In phase 0 it sits on the parent's circle
/// (radius 24, its side a quarter turn each from the parent's angle) turning with it, until the
/// parent's mode passes 111 or its flag 0x10 is up: then phase 1, its speed that of the way
/// round (the angle plus half a turn, less a eighth of a turn a side). In phase 1 it is done off the
/// area, and its speed eases toward four times the sine and cosine of its angle; its frame
/// cycles. Then its place moves by its speed and its draw-hide flag (0x80000) is the parent's.
pub fn orbiter_step(cpu: &mut Cpu, o: u32) {
    let f = cpu.stack();
    if cpu.read32(o + P_POWER) as i32 <= 0 {
        nested!(cpu, 0x0605_9e9c, ground::blown_up_1(cpu, o));
        cpu.write8(o + O_MODE, MODE_DONE);
        return;
    }
    if cpu.read16(o + OR_HIT_BITS) & 0x200 != 0 {
        let flags = cpu.read16(o + OR_FLAGS) | 1;
        cpu.write16(o + OR_FLAGS, flags);
    }
    let parent = cpu.read32(o + OR_PARENT);
    match cpu.read8(o + OR_PHASE) {
        0 => {
            cpu.write32(f, o + O_ANGLE);
            let angle = cpu.read16(parent + O_ANGLE);
            cpu.write16(o + O_ANGLE, angle);
            let side = cpu.read8(o + OR_SIDE) as i8 as i32;
            let a = (angle as i16 as i32).wrapping_add(side.wrapping_mul(0x4000)) as i16 as i32 as u32;
            let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a)) as i32;
            let x = cpu.read32(parent + O_POSITION).wrapping_add(sin.wrapping_mul(OR_RADIUS) as u32);
            cpu.write32(o + O_POSITION, x);
            let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a)) as i32;
            let y = cpu.read32(parent + O_POSITION + 4).wrapping_sub(cos.wrapping_mul(OR_RADIUS) as u32);
            cpu.write32(o + O_POSITION + 4, y);
            let z = cpu.read32(parent + O_POSITION + 8);
            cpu.write32(o + O_POSITION + 8, z);
            if cpu.read8(parent + O_MODE) as i8 > 111 || cpu.read16(parent + OR_FLAGS) & 0x10 != 0 {
                let phase = cpu.read8(o + OR_PHASE).wrapping_add(1);
                cpu.write8(o + OR_PHASE, phase);
                let p = cpu.read32(f);
                let side = cpu.read8(o + OR_SIDE) as i8 as i32;
                let a = (cpu.read16(p) as i16 as i32).wrapping_add(0xffff_8000u32 as i32).wrapping_sub(side.wrapping_mul(0x2000)) as i16 as i32 as u32;
                let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
                cpu.write32(o + OR_SPEED, sin);
                let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
                cpu.write32(o + OR_SPEED + 4, cos.wrapping_neg());
            }
        }
        1 => {
            let [top, bottom, left, right] = OR_AREA;
            let out = cpu.nested_with_stack(&aim::AREA_FLAGS_PROLOGUE, aim::AREA_FLAGS, &[right], |cpu| aim::area_flags(cpu, o + O_FLAGS, top, bottom, left, right)) as u8;
            if out != 0 {
                cpu.write8(o + O_MODE, MODE_DONE);
                return;
            }
            let a = cpu.read16(o + O_ANGLE) as i16 as i32 as u32;
            let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
            let v = cpu.read32(o + OR_SPEED);
            let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, sin << 2, OR_SPEED_STEP));
            cpu.write32(o + OR_SPEED, v);
            let a = cpu.read16(o + O_ANGLE) as i16 as i32 as u32;
            let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
            let v = cpu.read32(o + OR_SPEED + 4);
            let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, (cos << 2).wrapping_neg(), OR_SPEED_STEP));
            cpu.write32(o + OR_SPEED + 4, v);
            nested!(cpu, 0x0608_8548, cycle_frame(cpu, o));
        }
        _ => {}
    }
    for k in 0..2 {
        let v = cpu.read32(o + O_POSITION + 4 * k).wrapping_add(cpu.read32(o + OR_SPEED + 4 * k));
        cpu.write32(o + O_POSITION + 4 * k, v);
    }
    let flags = cpu.read32(o + O_FLAGS) & !F_HIDDEN;
    cpu.write32(o + O_FLAGS, flags);
    let hidden = cpu.read32(parent + O_FLAGS) & F_HIDDEN;
    cpu.write32(o + O_FLAGS, flags | hidden);
}

/// An orbiter's draw (sprite `s`; behaviour 0x06088764): its flag (+0x7e) 2 cleared when up, 2 raised
/// when 1 is; hidden with its flag 0x80000000. A sprite at its place from the model's (kind,
/// frame, lift, turn); the colour 16 flag when its side is a positive one; the colour on the
/// base (+0xe0 when flag 1 is up); and, while it is loose (phase), a second one in front of it
/// flickering (random 16 and, past frame 6, 256 flags) at 40 units about the angle.
pub fn orbiter_draw(cpu: &mut Cpu, s: u32) {
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
    if cpu.read32(o + O_FLAGS) & F_FLAGGED != 0 {
        return;
    }
    let d = cpu.stack();
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, d));
    let x = cpu.read32(o + O_POSITION);
    cpu.write32(d + S_POSITION, x);
    let z = cpu.read32(o + O_POSITION + 8);
    cpu.write32(d + S_POSITION + 8, z);
    let b = cpu.read8(s + S_LIFT);
    cpu.write8(d + S_LIFT, b);
    let turn = cpu.read16(s + S_TURN);
    cpu.write16(d + S_TURN, turn);
    if cpu.read8(s + OR_SIDE - 0x10) as i8 > 0 {
        cpu.write32(d + S_FLAGS, 16);
    }
    let kind = cpu.read8(s + S_KIND);
    cpu.write8(d + S_KIND, kind);
    let frame = cpu.read8(s + S_FRAME);
    cpu.write8(d + S_FRAME, frame);
    let y = cpu.read32(o + O_POSITION + 4);
    cpu.write32(d + S_POSITION + 4, y);
    let colour = if cpu.read16(at) & 1 != 0 {
        (cpu.read16(CHAR_BASE) as i16 as i32 as u32).wrapping_add(0xe0)
    } else {
        cpu.read16(s + S_COLOUR) as u32
    };
    cpu.write16(d + S_COLOUR, colour as u16);
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
    if cpu.read8(o + OR_PHASE) == 0 {
        return;
    }
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000)) & 16;
    cpu.write32(d + S_FLAGS, r);
    if cpu.read16(o + 0x8a) as i16 > 6 {
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000)) & 0x100;
        let flags = cpu.read32(d + S_FLAGS) | r;
        cpu.write32(d + S_FLAGS, flags);
    }
    cpu.write8(d + S_KIND, 23);
    let frame = cpu.read8(o + 0x8b);
    cpu.write8(d + S_FRAME, frame);
    let a = cpu.read16(o + O_ANGLE) as i16 as i32 as u32;
    let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a)) as i32;
    let x = cpu.read32(o + O_POSITION).wrapping_add(sin.wrapping_mul(-40) as u32);
    cpu.write32(d + S_POSITION, x);
    let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a)) as i32;
    let y = cpu.read32(o + O_POSITION + 4).wrapping_sub(cos.wrapping_mul(-40) as u32);
    cpu.write32(d + S_POSITION + 4, y);
    let base = cpu.read16(CHAR_BASE);
    let colour = if cpu.read32(LOOPS) & 1 != 0 { base.wrapping_add(16) } else { base };
    cpu.write16(d + S_COLOUR, colour);
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
}

const OR_SIZE: u32 = 0xbc;
const OR_BEHAVIOUR: u32 = 0x0608_857c;
const OR_DRAW: u32 = 0x0608_8764;
const OR_SCRIPT: u32 = 0x060a_c594;
const OR_FLAGS_SET: u32 = 0x00f8_0020;
const OR_POWER: u32 = 8000;

/// An orbiter made on `parent`'s circle at `side` (0x0608_888f4: an object of list 2, 0xbc bytes):
/// its behaviour and draw, flags 0xf80020, kind 23 frame 1, the parent's colour and number
/// bytes, power 8000, its hit bytes 1 and 18, its parent, its side and frame 2; its step run once.
pub fn orbiter_make(cpu: &mut Cpu, parent: u32, side: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, OR_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, SHOT_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + 0x3c));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + O_BEHAVIOUR, OR_BEHAVIOUR);
    cpu.write32(o + O_FLAGS, OR_FLAGS_SET);
    cpu.write32(o + O_DRAW, OR_DRAW);
    cpu.write8(o + O_KIND, 23);
    cpu.write8(o + O_FRAME, 1);
    let colour = cpu.read16(parent + O_COLOUR_A);
    cpu.write16(o + O_COLOUR_A, colour);
    cpu.write8(o + 0x33, 0xf8);
    let b = cpu.read8(parent + O_NUMBER);
    cpu.write8(o + O_NUMBER, b);
    cpu.write32(o + O_SCRIPT, OR_SCRIPT);
    cpu.write32(o + P_POWER, OR_POWER);
    cpu.write8(o + O_54, 1);
    cpu.write8(o + O_55, 18);
    cpu.write32(o + OR_PARENT, parent);
    cpu.write8(o + OR_SIDE, side as u8);
    cpu.write16(o + 0x8a, 2);
    nested!(cpu, 0x0608_857c, orbiter_step(cpu, o));
}

/// A route flier's fields: its route (+0x6c, as the diver's), the side times its x speed (+0x88,
/// +0x80), its y speed (+0x84), its height (+0xb0), the flags (+0x7e: 0x1000 asks for the area
/// check).
const RF_AREA: [u32; 4] = [0xff70_0000, 0x0090_0000, 0xff30_0000, 0x00d0_0000];
const RF_SCORE: u32 = 100;
const RF_ROUTE: u32 = 0x6c;
const RF_AREA_ASK: u16 = 0x1000;
const RF_LIFT: u32 = 0xb0;
const RF_FAR: i32 = 0x4_0000;
const RF_SHOT: u32 = 0x0607_30b0;
const RF_FIRE: u8 = 7;
const RF_BURST: u8 = 15;
const RF_FIRE_RADIUS: i32 = 18;
const RF_BURST_RADIUS: i32 = 12;
const RF_BURST_TURNS: [u16; 4] = [0x1800, 0xc00, 0xf400, 0xe800];

/// A route flier's step (behaviour 0x060889dc): done when it asks (+0x7e flag 0x1000) and is off
/// its area, or scored and blown up when its power (+0x50) is gone. Its hit bit 0x200 raises
/// the flag 1. It takes its route's steps (ground::route_step): on the route's 7 it raises the
/// flag 0x10 and shoots a pair of effects from two points a quarter turn either side of its
/// angle (radius 18, the shot at 0x060730b0); on 15 it plays a sound and puts the flame effect
/// (0x06054e08) out at its point (12 out) and four turns round it. Then its place moves by its
/// speed (x times the side), its z is its height and it is hidden (0x80000) when that is over 0x40000.
pub fn route_flier_step(cpu: &mut Cpu, o: u32) {
    let f = cpu.stack();
    if cpu.read16(o + OR_FLAGS) & RF_AREA_ASK != 0 {
        let [top, bottom, left, right] = RF_AREA;
        let out = cpu.nested_with_stack(&aim::AREA_FLAGS_PROLOGUE, aim::AREA_FLAGS, &[right], |cpu| aim::area_flags(cpu, o + O_FLAGS, top, bottom, left, right)) as u8;
        if out != 0 {
            cpu.write8(o + O_MODE, MODE_DONE);
            return;
        }
    }
    if cpu.read32(o + P_POWER) as i32 <= 0 {
        nested!(cpu, 0x0605_61ac, hits::score_hit_by(cpu, o, RF_SCORE));
        nested!(cpu, 0x0605_9e9c, ground::blown_up_1(cpu, o));
        cpu.write8(o + O_MODE, MODE_DONE);
        return;
    }
    if cpu.read16(o + OR_HIT_BITS) & 0x200 != 0 {
        let flags = cpu.read16(o + OR_FLAGS) | 1;
        cpu.write16(o + OR_FLAGS, flags);
    }
    let route = cpu.read32(o + RF_ROUTE);
    let r = nested!(cpu, 0x0605_a000, ground::route_step(cpu, o, route)) as u8;
    if r == RF_FIRE {
        let flags = cpu.read16(o + OR_FLAGS) | 0x10;
        cpu.write16(o + OR_FLAGS, flags);
        cpu.write32(f + 0x3c, RF_SHOT);
        let z = cpu.read32(o + O_POSITION + 8).wrapping_add(0xfffc_0000);
        cpu.write32(f + 8, z);
        cpu.write16(f + 0x20, 16);
        let angle = cpu.read16(o + O_ANGLE);
        cpu.write16(f + 0x18, angle);
        cpu.write32(f + 0x14, 0xffff_4000);
        cpu.write32(f + 0xc, 0xc000);
        cpu.write32(f + 0x10, 0xc000);
        for turn in [0x4000u32, 0xffff_c000] {
            let a = (cpu.read16(o + O_ANGLE) as u32).wrapping_add(turn) as i16 as i32 as u32;
            let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a)) as i32;
            let x = cpu.read32(o + O_POSITION).wrapping_add(sin.wrapping_mul(RF_FIRE_RADIUS) as u32);
            cpu.write32(f, x);
            let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a)) as i32;
            let y = cpu.read32(o + O_POSITION + 4).wrapping_sub(cos.wrapping_mul(RF_FIRE_RADIUS) as u32);
            cpu.write32(f + 4, y);
            nested!(cpu, 0x0605_7454, actors::drift_effect(cpu, f));
        }
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 61));
    } else if r == RF_BURST {
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 24));
        let b = cpu.read8(o + O_NUMBER);
        cpu.write8(f + 0x1e, b);
        let angle = cpu.read16(o + O_ANGLE);
        cpu.write16(f + 0x2c, angle);
        let a = cpu.read16(f + 0x2c) as i16 as i32 as u32;
        let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a)) as i32;
        let x = cpu.read32(o + O_POSITION).wrapping_add(sin.wrapping_mul(RF_BURST_RADIUS) as u32);
        cpu.write32(f, x);
        let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a)) as i32;
        let y = cpu.read32(o + O_POSITION + 4).wrapping_add(cos.wrapping_neg().wrapping_mul(RF_BURST_RADIUS) as u32);
        cpu.write32(f + 4, y);
        cpu.write32(f + 8, 0);
        cpu.call(0x0605_4e08, &[f]);
        for turn in RF_BURST_TURNS {
            let a = cpu.read16(o + O_ANGLE).wrapping_add(turn);
            cpu.write16(f + 0x2c, a);
            cpu.call(0x0605_4e08, &[f]);
        }
    }
    let side = cpu.read8(o + OR_SIDE) as i8 as i32;
    let x = cpu.read32(o + O_POSITION).wrapping_add(side.wrapping_mul(cpu.read32(o + OR_SPEED) as i32) as u32);
    cpu.write32(o + O_POSITION, x);
    let y = cpu.read32(o + O_POSITION + 4).wrapping_add(cpu.read32(o + OR_SPEED + 4));
    cpu.write32(o + O_POSITION + 4, y);
    let lift = cpu.read32(o + RF_LIFT);
    cpu.write32(o + O_POSITION + 8, lift);
    let flags = cpu.read32(o + O_FLAGS);
    let flags = if (lift as i32).wrapping_abs() > RF_FAR { flags | F_HIDDEN } else { flags & !F_HIDDEN };
    cpu.write32(o + O_FLAGS, flags);
}

/// A route flier's draw (sprite `s`; behaviour 0x06088c68): its flags (+0x7e) handled as the
/// orbiter's draw does, then a sprite of its kind and frame at its place (lift -8), the colour on
/// the base (+0xe0 when the flag 1 is up), and the thrust (ground::thrust) behind it.
pub fn route_flier_draw(cpu: &mut Cpu, s: u32) {
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
    if cpu.read32(o + O_FLAGS) & F_FLAGGED != 0 {
        return;
    }
    let d = cpu.stack();
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, d));
    let b = cpu.read8(s + S_KIND);
    cpu.write8(d + S_KIND, b);
    let b = cpu.read8(s + S_FRAME);
    cpu.write8(d + S_FRAME, b);
    cpu.write8(d + S_LIFT, 0xf8);
    for k in 0..3 {
        let v = cpu.read32(o + O_POSITION + 4 * k);
        cpu.write32(d + S_POSITION + 4 * k, v);
    }
    let turn = cpu.read16(s + S_TURN);
    cpu.write16(d + S_TURN, turn);
    let colour = if cpu.read16(at) & 1 != 0 {
        (cpu.read16(CHAR_BASE) as i16 as i32 as u32).wrapping_add(0xe0) as u16
    } else {
        cpu.read16(s + S_COLOUR)
    };
    cpu.write16(d + S_COLOUR, colour);
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
    nested!(cpu, 0x0605_9d24, ground::thrust(cpu, o, d, 7, 0xa000));
}

/// An event row's fields (as the other rows': its place +4, +6 in whole units, its side +0xb, its
/// route +8, its angle +0x14, its type +0x1b), the route flier's kind's first colour, its tables.
const RF_KIND: u32 = 23;
const RF_SIZE: u32 = 0xbc;
const RF_FLAGS_SET: u32 = 0x00c0_0020;
const RF_ROUTES: u32 = 0x060a_c584;
const RF_SCRIPT: u32 = 0x060a_c5a4;
const RF_POWER: u32 = 2000;
const RF_BEHAVIOUR: u32 = 0x0608_89dc;
const RF_DRAW: u32 = 0x0608_8c68;
const RF_OFFSET: u32 = 0xa8;

/// A route flier made from the event row `e` (an object of list 2, 0xbc bytes, flags 0xc00020,
/// kind 23, the kind's colour plus 16 times the type, its route from the table by +8, its angle
/// from +0x14, its side +0xb, power 2000, its place from +4 and +6, z 0), and, when the row's
/// +0xa byte is 0, two orbiters on it, one each side. Gives 1.
pub fn route_flier_make(cpu: &mut Cpu, e: u32) -> u32 {
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, 2)) as i8;
    if n == -1 {
        return 1;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, RF_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, PART_EXEC));
    if o == 0 {
        return 1;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + 0x3c));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + O_BEHAVIOUR, RF_BEHAVIOUR);
    cpu.write32(o + O_FLAGS, RF_FLAGS_SET);
    cpu.write32(o + O_DRAW, RF_DRAW);
    cpu.write8(o + O_KIND, RF_KIND as u8);
    cpu.write8(o + O_FRAME, 0);
    let ty = cpu.read8(e + E_TYPE) as u32;
    let base = cpu.read16(kind(RF_KIND) + K_COLOUR) as i16 as i32 as u32;
    cpu.write16(o + O_COLOUR_A, (ty << 4).wrapping_add(base) as u16);
    cpu.write8(o + O_33, 0xf8);
    let route = cpu.read8(e + E_SCRIPT) as u32;
    let route = cpu.read32(RF_ROUTES + 4 * route);
    cpu.write32(o + RF_ROUTE, route);
    let angle = cpu.read16(e + E_DIRECTION);
    cpu.write16(o + O_ANGLE, angle);
    let angle = cpu.read16(e + E_DIRECTION);
    cpu.write16(o + RF_OFFSET, angle);
    let side = cpu.read8(e + E_MIRROR);
    cpu.write8(o + OR_SIDE, side);
    cpu.write8(o + O_NUMBER, n as u8);
    let number = cpu.read8(o + O_NUMBER) as i8 as i32 as u32;
    cpu.write32(NUMBERED.wrapping_add(number << 2), o);
    cpu.write32(o + O_SCRIPT, RF_SCRIPT);
    cpu.write32(o + P_POWER, RF_POWER);
    cpu.write8(o + O_54, 1);
    cpu.write8(o + O_55, ty as u8);
    let x = (cpu.read16(e + E_PLACE) as i16 as i32 as u32) << 16;
    cpu.write32(o + O_POSITION, x);
    let y = (cpu.read16(e + E_PLACE + 2) as i16 as i32 as u32) << 16;
    cpu.write32(o + O_POSITION + 4, y);
    cpu.write32(o + O_POSITION + 8, 0);
    if cpu.read8(e + 0xa) == 0 {
        nested!(cpu, 0x0608_88f4, orbiter_make(cpu, o, 1));
        nested!(cpu, 0x0608_88f4, orbiter_make(cpu, o, -1i32 as u32));
    }
    1
}

/// A faller's step (behaviour 0x06088eb8; drawn as an orbiter): done (blown up) when its power is
/// gone, done when it is under the area (y over 0xd00000); else its frame cycles and it falls by +0x84.
pub fn faller_step(cpu: &mut Cpu, o: u32) {
    if cpu.read32(o + P_POWER) as i32 <= 0 {
        nested!(cpu, 0x0605_9e9c, ground::blown_up_1(cpu, o));
        cpu.write8(o + O_MODE, MODE_DONE);
        return;
    }
    if cpu.read32(o + O_POSITION + 4) as i32 > 0xd0_0000 {
        cpu.write8(o + O_MODE, MODE_DONE);
        return;
    }
    if cpu.read16(o + OR_HIT_BITS) & 0x200 != 0 {
        let flags = cpu.read16(o + OR_FLAGS) | 1;
        cpu.write16(o + OR_FLAGS, flags);
    }
    nested!(cpu, 0x0608_8548, cycle_frame(cpu, o));
    let y = cpu.read32(o + O_POSITION + 4).wrapping_add(cpu.read32(o + OR_SPEED + 4));
    cpu.write32(o + O_POSITION + 4, y);
}

/// The wave's fields in its event row `e`: the count (+0x1c), the clock (+0x16), the side flag
/// (+0x1e).
const FR_COUNT: u32 = 0x1c;
const FR_CLOCK: u32 = 0x16;
const FR_SIDE: u32 = 0x1e;
const FR_FIRST: i16 = 32;
const FR_EVERY: u32 = 10;
const FR_FALLER_SIZE: u32 = 0xbc;
const FR_FALLER_FLAGS: u32 = 0x00f0_0020;
const FR_FALLER_BEHAVIOUR: u32 = 0x0608_8eb8;
const FR_NEAR: i16 = 11;
const FR_WIDE: [u32; 2] = [0x60_0000, 0xc0_0000];

/// An event row of route fliers (as the other rows': it gives 1 done, 0 to go on). Past its count
/// of 32 it is a batch of route fliers: two pairs, each side (the side flag in +0x1e makes the
/// sides go the other way), made by route_flier_make with the row's place, side and type set per
/// flier (the places x +-96 and +-152, y -176, -216 and -192; the types 0 and 1; the route ids 2
/// and 3). Before it, every 10th clock tick it makes a faller (an object of list 2; its place x a random multiple of
/// 0x200000 in 0 to 0x600000 (0xc00000 once the count is past 11) either side, y -208.0; its
/// colour on the base, plus 16 on the side where it will fall in; a random side +-1, frame 2, a
/// fall speed 0x3c000 and a random 0-0x3fff, the angle 0x8000), counting up; the side flag is
/// picked at random with the first.
pub fn flier_row(cpu: &mut Cpu, e: u32) -> u32 {
    let f = cpu.stack();
    if cpu.read16(e + FR_COUNT) as i16 > FR_FIRST {
        let sign: i32 = if cpu.read16(e + FR_SIDE) & 0x100 == 0 { 1 } else { -1 };
        cpu.write8(e + 0xa, 1);
        cpu.write32(f, e + E_MIRROR);
        cpu.write8(e + E_MIRROR, 1);
        cpu.write16(e + E_DIRECTION, 0x8000);
        cpu.write32(f + 4, e + E_TYPE);
        cpu.write8(e + E_TYPE, 0);
        cpu.write8(e + E_SCRIPT, 2);
        let place = |cpu: &mut Cpu, x: i32, y: u32| {
            cpu.write16(e + E_PLACE, x.wrapping_mul(sign) as u16);
            cpu.write16(e + E_PLACE + 2, y as u16);
            nested!(cpu, 0x0608_8d48, route_flier_make(cpu, e));
        };
        place(cpu, 96, 0xff50);
        cpu.write16(e + E_PLACE + 2, 0xff28);
        nested!(cpu, 0x0608_8d48, route_flier_make(cpu, e));
        cpu.write8(e + E_SCRIPT, 3);
        let pair = cpu.read32(f);
        cpu.write8(pair, sign as u8);
        place(cpu, 0x98, 0xff40);
        let ty = cpu.read32(f + 4);
        cpu.write8(ty, 1);
        cpu.write8(e + E_SCRIPT, 2);
        place(cpu, -96, 0xff50);
        cpu.write16(e + E_PLACE + 2, 0xff28);
        nested!(cpu, 0x0608_8d48, route_flier_make(cpu, e));
        cpu.write8(e + E_SCRIPT, 3);
        let pair = cpu.read32(f);
        cpu.write8(pair, sign as u8);
        place(cpu, -0x98, 0xff40);
        return 1;
    }
    let clock = cpu.read16(e + FR_CLOCK).wrapping_add(1);
    cpu.write16(e + FR_CLOCK, clock);
    if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, clock as i16 as i32 as u32, FR_EVERY)) != 0 {
        return 0;
    }
    if cpu.read16(e + FR_COUNT) == 0 {
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000)) & 0x100;
        cpu.write16(e + FR_SIDE, r as u16);
    }
    let count = cpu.read16(e + FR_COUNT).wrapping_add(1);
    cpu.write16(e + FR_COUNT, count);
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, FR_FALLER_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, SHOT_EXEC));
    if o == 0 {
        return 1;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + 0x3c));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + O_BEHAVIOUR, FR_FALLER_BEHAVIOUR);
    cpu.write32(o + O_FLAGS, FR_FALLER_FLAGS);
    cpu.write32(o + O_DRAW, OR_DRAW);
    cpu.write8(o + O_KIND, 23);
    cpu.write8(o + O_FRAME, 1);
    cpu.write8(o + O_33, 0xf8);
    cpu.write32(o + O_SCRIPT, OR_SCRIPT);
    cpu.write32(o + P_POWER, OR_POWER);
    cpu.write8(o + O_54, 1);
    cpu.write8(o + O_55, 18);
    let wide = if cpu.read16(e + FR_COUNT) as i16 > FR_NEAR { FR_WIDE[1] } else { FR_WIDE[0] };
    let x = nested!(cpu, 0x0604_5d90, math::random(cpu, wide)) & 0xffe0_0000;
    cpu.write32(o + O_POSITION, x);
    cpu.write32(o + O_POSITION + 4, 0xff30_0000);
    cpu.write32(o + O_POSITION + 8, 0);
    let x = cpu.read32(o + O_POSITION);
    let base = cpu.read16(kind(RF_KIND) + K_COLOUR);
    let flip = cpu.read16(e + FR_SIDE) & 0x100 != 0;
    let in_screen = (x as i32).wrapping_abs() <= 0x5f_ffff;
    let colour = if in_screen {
        base
    } else if flip {
        if x as i32 > 95 { base.wrapping_add(16) } else { base }
    } else if x as i32 > 95 {
        base
    } else {
        base.wrapping_add(16)
    };
    cpu.write16(o + O_COLOUR_A, colour);
    let side = if nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000)) & 0x8000 == 0 { 1 } else { 0xff };
    cpu.write8(o + OR_SIDE, side);
    cpu.write16(o + 0x8a, 2);
    cpu.write8(o + OR_PHASE, 1);
    let speed = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x4000));
    cpu.write32(o + OR_SPEED + 4, speed.wrapping_add(0x3_c000));
    cpu.write16(o + O_ANGLE, 0x8000);
    0
}

/// Whether the nearer player (aim::nearer_player) is within 0x13ffff in x and y of the point 24 in
/// front of `o` (along its angle, +0x30). Gives 1 or 0.
pub fn player_in_front(cpu: &mut Cpu, o: u32) -> u32 {
    let player = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + O_FLAGS));
    let a = cpu.read16(o + O_ANGLE) as i16 as i32 as u32;
    let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a)) as i32;
    let x = (sin.wrapping_mul(24) as u32).wrapping_add(cpu.read32(o + O_POSITION));
    let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a)) as i32;
    let y = (cos.wrapping_mul(-24) as u32).wrapping_add(cpu.read32(o + O_POSITION + 4));
    let near = 0x13_ffff;
    if absolute(cpu.read32(player + O_POSITION).wrapping_sub(x)) > near {
        return 0;
    }
    (absolute(cpu.read32(player + O_POSITION + 4).wrapping_sub(y)) <= near) as u32
}

/// A slider's fields: its phase (+0x7c), the swing angle (+0x8c, a word) and its speed (+0xa6),
/// the countdown (+0x88), its limits along the way it slides (+0xac, +0xb0), the distance slid
/// (+0xb4), the trail's centre (+0x92, +0x94) and the trail's points (+0xbe on, 8 apart: x, y).
const SL_PHASE: u32 = 0x7c;
const SL_SWING: u32 = 0x8c;
const SL_SWING_SPEED: u32 = 0xa6;
const SL_COUNT: u32 = 0x88;
const SL_LOW: u32 = 0xac;
const SL_HIGH: u32 = 0xb0;
const SL_SLID: u32 = 0xb4;
const SL_TRAIL_AT: u32 = 0xbe;
const SL_CENTRE: u32 = 0x92;
const SL_BOTTOM: i32 = 0xb0_0000;
const SL_SCORE: u32 = 100;
const SL_SLIDE_STEP: u32 = 0x1200;
const SL_SPEED: u32 = 0x1_4000;
const SL_ALONG: u16 = 0x4000;
const SL_SWING_RELEASE: u16 = 0xc000;
const SL_SWING_BACK: u32 = 0xffff_f000;
const SL_SWING_BY: u32 = 0xa0;
const SL_RECOVER: u16 = 0x600;
const SL_SOUND: u32 = 60;

/// A slider's step (behaviour 0x06089288): done under the bottom (y over 0xb00000), or scored and
/// blown up when its power is gone; its flag 0x10 flips each frame; the trail's points (three, two
/// words each side, each further out by one on the swing angle +0x4000: sine on 8192ths for x,
/// cosine times 8 on 65536ths for y, about the centre) set; the hit bit 0x200 raises the flag 1.
/// Then by phase: 0 and past 3 aim (the speed 0x14000 toward the nearer player along its way, the
/// way being y when the angle's 0x4000 bit is up; once the player is in front the phase is 1, the
/// recover word 0x600), 1 swings back (the speed to -0x1000 by 0xa0 until the swing is under
/// -0x4000: then phase 2, the countdown 15, a sound), 2 counts down to phase 3, 3 swings forward
/// until it is level, to phase 0 (or 1 if the player is in front). The slide itself: along x or
/// y (by the angle's 0x4000 bit) the speed approached to the aim by 0x1200, the place kept between
/// the limits, the distance slid kept, the frame from it; along y the limits and place move
/// down with the ground.
pub fn swinger_step(cpu: &mut Cpu, o: u32) {
    if cpu.read32(o + O_POSITION + 4) as i32 > SL_BOTTOM {
        cpu.write8(o + O_MODE, MODE_DONE);
        return;
    }
    if cpu.read32(o + P_POWER) as i32 <= 0 {
        nested!(cpu, 0x0605_61ac, hits::score_hit_by(cpu, o, SL_SCORE));
        nested!(cpu, 0x0605_9e9c, ground::blown_up_1(cpu, o));
        cpu.write8(o + O_MODE, MODE_DONE);
        return;
    }
    let flags = cpu.read32(o + O_FLAGS) ^ 16;
    cpu.write32(o + O_FLAGS, flags);
    let mut k = 0u32;
    loop {
        let a = (cpu.read16(o + SL_SWING) as i16 as i32 as u32).wrapping_add(0x4000) as i16 as i32 as u32;
        let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a)) as i32;
        k += 1;
        let mut v = sin.wrapping_mul(k as i32);
        if v < 0 {
            v = v.wrapping_add(0x1fff);
        }
        nested!(cpu, 0x0600_815a, clib::ashiftrt_r4::<13>(cpu, v as u32));
        let x = ((v >> 13) as u32).wrapping_add(cpu.read16(o + SL_CENTRE) as i16 as i32 as u32);
        let a = (cpu.read16(o + SL_SWING) as i16 as i32 as u32).wrapping_add(0x4000) as i16 as i32 as u32;
        let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a)) as i32;
        let mut v = (cos.wrapping_mul(k as i32) << 3).wrapping_neg();
        if v < 0 {
            v = v.wrapping_add(0xffff);
        }
        let y = ((v >> 16) as u16).wrapping_add(cpu.read16(o + SL_CENTRE + 2));
        let at = o + SL_TRAIL_AT + 16 * (k - 1);
        cpu.write16(at, x as u16);
        cpu.write16(at + 2, y);
        cpu.write16(at + 8, x as u16);
        cpu.write16(at + 10, y);
        if k > 2 {
            break;
        }
    }
    if cpu.read16(o + O_ARMS) & 0x200 != 0 {
        let flags = cpu.read16(o + OR_FLAGS) | 1;
        cpu.write16(o + OR_FLAGS, flags);
    }
    let phase = cpu.read8(o + SL_PHASE);
    let mut drive = 0;
    match phase {
        2 => {
            let n = cpu.read8(o + SL_COUNT).wrapping_sub(1);
            cpu.write8(o + SL_COUNT, n);
            if (n as i8) < 0 {
                cpu.write8(o + SL_PHASE, phase + 1);
            }
        }
        1 => {
            let speed = signed_word(cpu, o + SL_SWING_SPEED);
            let speed = nested!(cpu, 0x0604_6168, actors::approach(cpu, speed, SL_SWING_BACK, SL_SWING_BY));
            cpu.write16(o + SL_SWING_SPEED, speed as u16);
            let swing = cpu.read16(o + SL_SWING).wrapping_add(speed as u16);
            cpu.write16(o + SL_SWING, swing);
            if (swing as i16) < SL_SWING_RELEASE as i16 {
                let phase = cpu.read8(o + SL_PHASE).wrapping_add(1);
                cpu.write8(o + SL_PHASE, phase);
                cpu.write8(o + SL_COUNT, 15);
                cpu.write16(o + SL_SWING, SL_SWING_RELEASE);
                nested!(cpu, 0x0600_50d4, sound::se_set(cpu, SL_SOUND));
            }
        }
        3 => {
            let swing = cpu.read16(o + SL_SWING).wrapping_add(0x400);
            cpu.write16(o + SL_SWING, swing);
            if swing as i16 >= 0 {
                cpu.write16(o + SL_SWING, 0);
                let in_front = cpu.holding(&[(8, o), (9, 3), (10, o + SL_PHASE), (11, 0)], |cpu| nested!(cpu, 0x0608_9208, player_in_front(cpu, o)));
                if in_front as u8 == 0 {
                    cpu.write8(o + SL_PHASE, 0);
                } else {
                    cpu.write8(o + SL_PHASE, 1);
                    cpu.write16(o + SL_SWING_SPEED, SL_RECOVER);
                }
            }
        }
        _ => {
            let player = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + O_FLAGS));
            let (own, theirs) = if cpu.read16(o + O_ANGLE) & SL_ALONG == 0 {
                (cpu.read32(o + O_POSITION), cpu.read32(player + O_POSITION))
            } else {
                (cpu.read32(o + O_POSITION + 4), cpu.read32(player + O_POSITION + 4))
            };
            drive = if own as i32 > theirs as i32 { SL_SPEED.wrapping_neg() } else { SL_SPEED };
            let in_front = cpu.holding(&[(8, o), (9, 3), (10, o + SL_PHASE), (11, drive)], |cpu| nested!(cpu, 0x0608_9208, player_in_front(cpu, o)));
            if in_front as u8 != 0 {
                cpu.write8(o + SL_PHASE, 1);
                drive = 0;
                cpu.write16(o + SL_SWING_SPEED, SL_RECOVER);
            }
        }
    }
    if cpu.read16(o + O_ANGLE) & SL_ALONG == 0 {
        let v = cpu.read32(o + OR_SPEED);
        let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, drive, SL_SLIDE_STEP));
        cpu.write32(o + OR_SPEED, v);
        let x = cpu.read32(o + O_POSITION).wrapping_add(v);
        cpu.write32(o + O_POSITION, x);
        let low = cpu.read32(o + SL_LOW);
        if (x as i32) < low as i32 {
            cpu.write32(o + O_POSITION, low);
            cpu.write32(o + OR_SPEED, 0);
        }
        let x = cpu.read32(o + O_POSITION);
        let high = cpu.read32(o + SL_HIGH);
        if x as i32 > high as i32 {
            cpu.write32(o + O_POSITION, high);
            cpu.write32(o + OR_SPEED, 0);
        }
        let slid = cpu.read32(o + SL_SLID).wrapping_add(cpu.read32(o + OR_SPEED));
        cpu.write32(o + SL_SLID, slid);
        let x = cpu.read32(o + O_POSITION);
        let mut v = x.wrapping_neg() as i32;
        if v < 0 {
            v = v.wrapping_add(0x3_ffff);
        }
        cpu.write8(o + O_FRAME, ((v >> 16) as i16 >> 2) as u8 & 3);
    } else {
        let v = cpu.read32(o + OR_SPEED + 4);
        let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, drive, SL_SLIDE_STEP));
        cpu.write32(o + OR_SPEED + 4, v);
        let y = cpu.read32(o + O_POSITION + 4).wrapping_add(v);
        cpu.write32(o + O_POSITION + 4, y);
        let at = cpu.read32(super::globals::GROUND_DY_AT);
        let dy = cpu.read32(at);
        let low = cpu.read32(o + SL_LOW).wrapping_sub(dy);
        cpu.write32(o + SL_LOW, low);
        let dy = cpu.read32(at);
        let high = cpu.read32(o + SL_HIGH).wrapping_sub(dy);
        cpu.write32(o + SL_HIGH, high);
        let y = cpu.read32(o + O_POSITION + 4);
        if (y as i32) < low as i32 {
            cpu.write32(o + O_POSITION + 4, low);
            cpu.write32(o + OR_SPEED + 4, 0);
        }
        let y = cpu.read32(o + O_POSITION + 4);
        if y as i32 > high as i32 {
            cpu.write32(o + O_POSITION + 4, high);
            cpu.write32(o + OR_SPEED + 4, 0);
        }
        let slid = cpu.read32(o + SL_SLID).wrapping_add(cpu.read32(o + OR_SPEED + 4));
        cpu.write32(o + SL_SLID, slid);
    }
    let slid = cpu.read32(o + SL_SLID);
    let mut v = slid.wrapping_neg() as i32;
    if v < 0 {
        v = v.wrapping_add(0x3_ffff);
    }
    cpu.write8(o + O_FRAME, ((v >> 16) as i16 >> 2) as u8 & 3);
    let at = cpu.read32(super::globals::GROUND_DY_AT);
    let dy = cpu.read32(at);
    let y = cpu.read32(o + O_POSITION + 4).wrapping_sub(dy);
    cpu.write32(o + O_POSITION + 4, y);
}

const SL_KIND: u32 = 73;
const SL_FLAGS_SET: u32 = 0x00c0_0000;
const SL_POWER: u32 = 3000;
const SL_STEP: u32 = 0x0608_9288;
const SL_DRAW: u32 = 0x0608_95d8;
const SL_BOXES: u32 = 0x060a_c5b4;
const SL_BOXES_PART: u32 = 0x060a_c5f4;
const SL_FALL_STEP: u32 = 0x0608_9998;
const SL_BOXES_AT: u32 = 0xbc;
const SL_TRAIL_TURN: u32 = 0x2000;

/// A slider's draw (sprite `s`; behaviour 0x060895d8): the flags (+0x7e) as the route flier's,
/// then three sprites in the kind 73: two wings (frames 4 and 5) at 6 out on the angle plus and
/// less 0x2000, turned by the angle plus and less the swing (+0x8c), then the body (its frame,
/// +0x2f) at its place turned by the angle.
pub fn slider_draw(cpu: &mut Cpu, s: u32) {
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
    let d = cpu.stack();
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, d));
    let b = cpu.read8(s + S_KIND);
    cpu.write8(d + S_KIND, b);
    cpu.write8(d + S_LIFT, 0xf8);
    let z = cpu.read32(o + O_POSITION + 8);
    cpu.write32(d + S_POSITION + 8, z);
    let colour = if cpu.read16(at) & 1 != 0 {
        (cpu.read16(CHAR_BASE) as i16 as i32 as u32).wrapping_add(0xe0) as u16
    } else {
        cpu.read16(s + S_COLOUR)
    };
    cpu.write16(d + S_COLOUR, colour);
    cpu.write32(d + 0x2c, d + S_FRAME);
    for (frame, turn, swing) in [(4u8, 0x2000u32, 1i32), (5, 0xffff_e000, -1)] {
        cpu.write8(d + S_FRAME, frame);
        let a = (cpu.read16(o + O_ANGLE) as i16 as i32 as u32).wrapping_add(turn) as i16 as i32 as u32;
        let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a)) as i32;
        let x = cpu.read32(o + O_POSITION).wrapping_add(sin.wrapping_mul(6) as u32);
        cpu.write32(d + S_POSITION, x);
        let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a)) as i32;
        let y = cpu.read32(o + O_POSITION + 4).wrapping_add(cos.wrapping_mul(-6) as u32);
        cpu.write32(d + S_POSITION + 4, y);
        let angle = cpu.read16(o + O_ANGLE);
        let swung = cpu.read16(o + SL_SWING);
        cpu.write16(d + S_TURN, if swing > 0 { angle.wrapping_add(swung) } else { angle.wrapping_sub(swung) });
        nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
    }
    let frame = cpu.read8(o + O_FRAME);
    cpu.write8(d + S_FRAME, frame);
    let x = cpu.read32(o + O_POSITION);
    cpu.write32(d + S_POSITION, x);
    let y = cpu.read32(o + O_POSITION + 4);
    cpu.write32(d + S_POSITION + 4, y);
    let angle = cpu.read16(o + O_ANGLE);
    cpu.write16(d + S_TURN, angle);
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
}

/// A slider's trail centre (+0x92, +0x94): the sine of 0x2000 times 6 and the cosine times -6, on
/// 65536ths.
fn slider_trail_centre(cpu: &mut Cpu, o: u32) {
    let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, SL_TRAIL_TURN)) as i32;
    let mut v = sin.wrapping_mul(6);
    if v < 0 {
        v = v.wrapping_add(0xffff);
    }
    cpu.write16(o + SL_CENTRE, (v >> 16) as u16);
    let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, SL_TRAIL_TURN)) as i32;
    let mut v = cos.wrapping_mul(-6);
    if v < 0 {
        v = v.wrapping_add(0xffff);
    }
    cpu.write16(o + SL_CENTRE + 2, (v >> 16) as u16);
}

/// A slider made from the event row `e` (an object of list 2, 0xfc bytes, flags 0xc00000, kind 73,
/// the colour by the type +0x1b, power 3000, its hit boxes copied in at +0xbc, its place from +4
/// and +6, the angle +0x14, its limits along its way by +0x1c and +0x1e from its place, the
/// trail centre). Gives 1.
pub fn slider_make(cpu: &mut Cpu, e: u32) -> u32 {
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, 2)) as i8;
    if n == -1 {
        return 1;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0xfc)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, PART_EXEC));
    if o == 0 {
        return 1;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + 0x3c));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + O_BEHAVIOUR, SL_STEP);
    cpu.write32(o + O_FLAGS, SL_FLAGS_SET);
    cpu.write32(o + O_DRAW, SL_DRAW);
    cpu.write8(o + O_KIND, SL_KIND as u8);
    cpu.write8(o + O_FRAME, 0);
    let ty = cpu.read8(e + E_TYPE) as u32;
    let base = cpu.read16(kind(SL_KIND) + K_COLOUR) as i16 as i32 as u32;
    cpu.write16(o + O_COLOUR_A, (ty << 4).wrapping_add(base) as u16);
    cpu.write8(o + O_NUMBER, n as u8);
    let number = cpu.read8(o + O_NUMBER) as i8 as i32 as u32;
    cpu.write32(NUMBERED.wrapping_add(number << 2), o);
    cpu.write32(o + P_POWER, SL_POWER);
    cpu.write8(o + O_54, 1);
    cpu.write8(o + O_55, ty as u8);
    nested!(cpu, 0x0605_6440, hits::boxes_copy(cpu, o + SL_BOXES_AT, SL_BOXES));
    cpu.write32(o + O_SCRIPT, o + SL_BOXES_AT);
    let x = (cpu.read16(e + E_PLACE) as i16 as i32 as u32) << 16;
    cpu.write32(o + O_POSITION, x);
    let y = (cpu.read16(e + E_PLACE + 2) as i16 as i32 as u32) << 16;
    cpu.write32(o + O_POSITION + 4, y);
    cpu.write32(o + O_POSITION + 8, 0);
    let b = cpu.read16(e + E_PLACE);
    cpu.write16(o + 0x8a, b);
    let angle = cpu.read16(e + E_DIRECTION);
    cpu.write16(o + O_ANGLE, angle);
    let along = if angle & SL_ALONG == 0 { o + O_POSITION } else { o + O_POSITION + 4 };
    let low = (cpu.read16(e + 0x1c) as i16 as i32 as u32) << 16;
    let low = cpu.read32(along).wrapping_add(low);
    cpu.write32(o + SL_LOW, low);
    let high = (cpu.read16(e + 0x1e) as i16 as i32 as u32) << 16;
    let high = cpu.read32(along).wrapping_add(high);
    cpu.write32(o + SL_HIGH, high);
    slider_trail_centre(cpu, o);
    1
}

/// A slider destroyed: scored (100), blown up and ended.
pub fn slider_burst(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0605_61ac, hits::score_hit_by(cpu, o, SL_SCORE));
    nested!(cpu, 0x0605_9e9c, ground::blown_up_1(cpu, o));
    cpu.write8(o + O_MODE, MODE_DONE);
}

/// A slider's swing (+0x8c) flickering: a random 0 to 255 less 0xc00; the hit bit 0x200 raises
/// the flag 1.
pub fn slider_flicker(cpu: &mut Cpu, o: u32) {
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x100));
    cpu.write16(o + SL_SWING, r.wrapping_add(0xffff_f400) as u16);
    if cpu.read16(o + O_ARMS) & 0x200 != 0 {
        let flags = cpu.read16(o + OR_FLAGS) | 1;
        cpu.write16(o + OR_FLAGS, flags);
    }
}

/// A falling slider's step (behaviour 0x06089998): done under 0xe00000, destroyed when its power is
/// gone, else flickering and carried up with the ground.
pub fn slider_fall_step(cpu: &mut Cpu, o: u32) {
    if cpu.read32(o + O_POSITION + 4) as i32 > 0xe0_0000 {
        cpu.write8(o + O_MODE, MODE_DONE);
        return;
    }
    if cpu.read32(o + P_POWER) as i32 <= 0 {
        nested!(cpu, 0x0608_991c, slider_burst(cpu, o));
        return;
    }
    nested!(cpu, 0x0608_994c, slider_flicker(cpu, o));
    let at = cpu.read32(super::globals::GROUND_DY_AT);
    let dy = cpu.read32(at);
    let y = cpu.read32(o + O_POSITION + 4).wrapping_sub(dy);
    cpu.write32(o + O_POSITION + 4, y);
}

/// A slider made falling from the description at `from` (its place, longs x and y at 0 and 4,
/// its type a word at +0x2c, a byte at +0x2d, its angle at +0x2e): as slider_make's, the behaviour
/// slider_fall_step, the hit boxes at 0x060ac5f4. The object, or 0.
pub fn slider_part_make(cpu: &mut Cpu, from: u32) -> u32 {
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, 2)) as i8;
    if n == -1 {
        return 0;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0xbc)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, PART_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + 0x3c));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + O_BEHAVIOUR, SL_FALL_STEP);
    cpu.write32(o + O_FLAGS, SL_FLAGS_SET);
    cpu.write32(o + O_DRAW, SL_DRAW);
    cpu.write8(o + O_KIND, SL_KIND as u8);
    cpu.write8(o + O_FRAME, 0);
    let ty = cpu.read16(from + 0x2c) as i16 as i32 as u32;
    let base = cpu.read16(kind(SL_KIND) + K_COLOUR) as i16 as i32 as u32;
    cpu.write16(o + O_COLOUR_A, (ty << 4).wrapping_add(base) as u16);
    cpu.write8(o + O_NUMBER, n as u8);
    let number = cpu.read8(o + O_NUMBER) as i8 as i32 as u32;
    cpu.write32(NUMBERED.wrapping_add(number << 2), o);
    cpu.write32(o + P_POWER, SL_POWER);
    cpu.write8(o + O_54, 1);
    let b = cpu.read8(from + 0x2d);
    cpu.write8(o + O_55, b);
    cpu.write32(o + O_SCRIPT, SL_BOXES_PART);
    let x = cpu.read32(from);
    cpu.write32(o + O_POSITION, x);
    let y = cpu.read32(from + 4);
    cpu.write32(o + O_POSITION + 4, y);
    cpu.write32(o + O_POSITION + 8, 0);
    let angle = cpu.read16(from + 0x2e);
    cpu.write16(o + O_ANGLE, angle);
    slider_trail_centre(cpu, o);
    o
}

/// A pair piece's fields: its side (+0x88: -1 or 1), its state (+0xa8: 0 waiting, 1 released), the
/// spread (+0xac, whose approach to 0 gives the speed) and its speed (+0x80); its parent is
/// +0x6c (the pair's platform).
const PP_STATE: u32 = 0xa8;
const PP_SPREAD: u32 = 0xac;
const PP_AREA: [u32; 4] = [0xff20_0000, 0x00e0_0000, 0xfee0_0000, 0x0120_0000];
const PP_SPREAD_BY: i32 = 0x7000;
const PP_PULL: i32 = -0x600;
const PP_REACH: i32 = 0x27_ffff;
const PP_OUT: i32 = 0x28_0000;
const PP_EASE: u32 = 0x2000;
const PP_POWER: u32 = 100;

/// A pair piece's step (behaviour 0x06089b58): done off the area. Once its parent's mode is
/// past 111 it is released (state 1) with the spread and speed 0x7000 times its side. Released,
/// the speed is pulled by 0x600 against the side while there is spread; within 0x27ffff of the
/// middle it is put at 0x280000 on its side and the spread eases to 0 (by 0x2000), which is its
/// speed too. It moves by its speed and the ground; its power is 100, or 0 on the frames of one
/// parity (by the side of the middle it is on): hit only on the others.
pub fn pair_step(cpu: &mut Cpu, o: u32) {
    let [top, bottom, left, right] = PP_AREA;
    let out = cpu.nested_with_stack(&aim::AREA_FLAGS_PROLOGUE, aim::AREA_FLAGS, &[right], |cpu| aim::area_flags(cpu, o + O_FLAGS, top, bottom, left, right)) as u8;
    if out != 0 {
        cpu.write8(o + O_MODE, MODE_DONE);
        return;
    }
    let state = cpu.read16(o + PP_STATE);
    let parent = cpu.read32(o + RF_ROUTE);
    if state as i16 == 0 && cpu.read8(parent + O_MODE) as i8 > 111 {
        cpu.write16(o + PP_STATE, state.wrapping_add(1));
        let side = cpu.read8(o + OR_SIDE) as i8 as i32;
        cpu.write32(o + OR_SPEED, side.wrapping_mul(PP_SPREAD_BY) as u32);
        let side = cpu.read8(o + OR_SIDE) as i8 as i32;
        cpu.write32(o + PP_SPREAD, side.wrapping_mul(PP_SPREAD_BY) as u32);
    }
    if cpu.read16(o + PP_STATE) != 0 {
        if cpu.read32(o + PP_SPREAD) != 0 {
            let side = cpu.read8(o + OR_SIDE) as i8 as i32;
            let v = cpu.read32(o + OR_SPEED).wrapping_add(side.wrapping_mul(PP_PULL) as u32);
            cpu.write32(o + OR_SPEED, v);
        }
        if absolute(cpu.read32(o + O_POSITION)) <= PP_REACH {
            let side = cpu.read8(o + OR_SIDE) as i8 as i32;
            cpu.write32(o + O_POSITION, side.wrapping_mul(PP_OUT) as u32);
            let spread = cpu.read32(o + PP_SPREAD);
            let spread = nested!(cpu, 0x0604_6168, actors::approach(cpu, spread, 0, PP_EASE));
            cpu.write32(o + PP_SPREAD, spread);
            cpu.write32(o + OR_SPEED, spread);
        }
    }
    let flags = cpu.read16(o + OR_FLAGS) | 0x10;
    cpu.write16(o + OR_FLAGS, flags);
    let x = cpu.read32(o + O_POSITION).wrapping_add(cpu.read32(o + OR_SPEED));
    cpu.write32(o + O_POSITION, x);
    let at = cpu.read32(super::globals::GROUND_DY_AT);
    let dy = cpu.read32(at);
    let y = cpu.read32(o + O_POSITION + 4).wrapping_sub(dy);
    cpu.write32(o + O_POSITION + 4, y);
    cpu.write32(o + P_POWER, PP_POWER);
    let odd = cpu.read32(GAME_FRAMES) & 1;
    if x as i32 > 0 {
        if odd == 0 {
            cpu.write32(o + P_POWER, 0);
        }
    } else if odd != 0 {
        cpu.write32(o + P_POWER, 0);
    }
}

/// A platform's fields: its phase (+0x7c), the heading (+0x8a, a word) it is turned to, its speed
/// along it (+0xac) and its (+0x80, +0x84) speed.
const PL_PHASE: u32 = 0x7c;
const PL_HEADING: u32 = 0x8a;
const PL_SPEED: u32 = 0xac;
const PL_AREA: [u32; 4] = [0xff6f_0000, 0x0090_0000, 0xff44_0000, 0x00bc_0000];
const PL_SHOT: u32 = 16;
const PL_LEVEL: u32 = 0x0600_ed84;

/// A platform's step (behaviour 0x06089cac): done off the area, blown up when its power is gone.
/// Phase 0 (rising: its y speed less 0x1800 a frame until it is under -0x8000) turns it to
/// face the nearer player (when the level byte 0x0600ed84 is past 1, else upside down) and goes to
/// phase 1; that lets the y speed rise by 0x1800 until it is up, then fires a plain bullet at the
/// player (its flags 0x40000 cleared, its frame 2, its hatch opened) and goes to phase 2, which
/// has it move along the heading at its speed (the sine and cosine times it), the speed rising to
/// 0x60000 by 0x4000. Every phase turns it toward the heading (a 32nd of the gap) and moves it.
pub fn platform_step(cpu: &mut Cpu, o: u32) {
    let f = cpu.stack();
    let [top, bottom, left, right] = PL_AREA;
    let out = cpu.nested_with_stack(&aim::AREA_FLAGS_PROLOGUE, aim::AREA_FLAGS, &[right], |cpu| aim::area_flags(cpu, o + O_FLAGS, top, bottom, left, right)) as u8;
    if out != 0 {
        cpu.write8(o + O_MODE, MODE_DONE);
        return;
    }
    if cpu.read32(o + P_POWER) as i32 <= 0 {
        nested!(cpu, 0x0605_9e4c, ground::blown_up(cpu, o));
        cpu.write8(o + O_MODE, MODE_DONE);
        return;
    }
    match cpu.read8(o + PL_PHASE) {
        0 => {
            let v = cpu.read32(o + OR_SPEED + 4).wrapping_add(0xffff_e800);
            cpu.write32(o + OR_SPEED + 4, v);
            if (v as i32) < -0x8000 {
                let phase = cpu.read8(o + PL_PHASE).wrapping_add(1);
                cpu.write8(o + PL_PHASE, phase);
                let heading = if cpu.read8(PL_LEVEL) as i8 > 1 {
                    nested!(cpu, 0x0605_6ad4, aim::angle_to_target(cpu, o, 2))
                } else {
                    0xffff_8000
                };
                cpu.write16(o + PL_HEADING, heading as u16);
            }
        }
        1 => {
            let v = cpu.read32(o + OR_SPEED + 4).wrapping_add(0x1800);
            cpu.write32(o + OR_SPEED + 4, v);
            if v as i32 > 0 {
                let phase = cpu.read8(o + PL_PHASE).wrapping_add(1);
                cpu.write8(o + PL_PHASE, phase);
                let b = cpu.read8(o + O_NUMBER);
                cpu.write8(f + 0x1e, b);
                for k in 0..3 {
                    let place = cpu.read32(o + O_POSITION + 4 * k);
                    cpu.write32(f + 4 * k, place);
                }
                let aim_at = nested!(cpu, 0x0605_6ad4, aim::angle_to_target(cpu, o, PL_SHOT));
                cpu.write16(f + 0x2c, aim_at as u16);
                nested!(cpu, 0x0605_3f48, bullets::bullet_set_plain(cpu, f));
                let flags = cpu.read32(o + O_FLAGS) & 0xfffb_ffff;
                cpu.write32(o + O_FLAGS, flags);
                cpu.write8(o + O_FRAME, 2);
                nested!(cpu, 0x0605_dd54, ground::hatch_opens(cpu, o));
            }
        }
        2 => {
            let speed = cpu.read32(o + PL_SPEED);
            let a = signed_word(cpu, o + PL_HEADING);
            let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
            let x = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, sin, speed));
            cpu.write32(o + OR_SPEED, x);
            let speed = cpu.read32(o + PL_SPEED);
            let a = signed_word(cpu, o + PL_HEADING);
            let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
            let y = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, cos, speed.wrapping_neg()));
            cpu.write32(o + OR_SPEED + 4, y);
            let speed = cpu.read32(o + PL_SPEED);
            let speed = nested!(cpu, 0x0604_6168, actors::approach(cpu, speed, 0x6_0000, 0x4000));
            cpu.write32(o + PL_SPEED, speed);
        }
        _ => {}
    }
    let heading = signed_word(cpu, o + PL_HEADING);
    let angle = signed_word(cpu, o + O_ANGLE);
    let gap = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, angle, heading));
    let share = nested!(cpu, 0x0600_6a78, math::quotient(cpu, gap, 32));
    let angle = cpu.read16(o + O_ANGLE).wrapping_add(share as u16);
    cpu.write16(o + O_ANGLE, angle);
    for k in 0..2 {
        let v = cpu.read32(o + O_POSITION + 4 * k).wrapping_add(cpu.read32(o + OR_SPEED + 4 * k));
        cpu.write32(o + O_POSITION + 4 * k, v);
    }
}

const PR_TABLE: u32 = 0x060a_c604;
const PR_EVERY: u32 = 16;
const PR_SIZE: u32 = 0xbc;
const PR_FLAGS_SET: u32 = 0x4_0000;
const PR_KIND: u32 = 27;
const PR_POWER: u32 = 200;
const PR_SCRIPT: u32 = 0x0607_3278;
const PR_START_Y: u32 = 0xff70_0000;
const PR_FALL: u32 = 0x3_8000;
const PR_CLOCK: u32 = 0x16;
const PR_AT: u32 = 0x1c;

/// An event row of platforms (gives 0, going on, or 1, finished): every 16th tick of its clock
/// (+0x16) it takes the next entry (+0x1c) of the table at 0x060ac604, a word each: -2 is skipped,
/// -1 the end; else a platform (an object of list 2, 0xbc bytes, flags 0x40000, kind 27, the kind's
/// colour (+0xcc of the kinds) by the row's type (+0x1b), power 200, heading and angle 0x8000, x
/// the entry on 65536ths, y -144.0, y speed 0x38000) is made; past the ninth entry the type counts
/// down (to 2 when it passes 0).
pub fn platform_row(cpu: &mut Cpu, e: u32) -> u32 {
    let clock = cpu.read16(e + PR_CLOCK) as i16 as i32 as u32;
    if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, clock, PR_EVERY)) != 0 {
        return platform_row_tick(cpu, e);
    }
    let at = cpu.read16(e + PR_AT);
    let entry = cpu.read16(PR_TABLE + 2 * at as u32) as i16;
    if entry == -2 {
        cpu.write16(e + PR_AT, at.wrapping_add(1));
        return 0;
    }
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, 2)) as i8;
    if n == -1 {
        return 0;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, PR_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, PART_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + 0x3c));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + O_BEHAVIOUR, 0x0608_9cac);
    cpu.write32(o + O_FLAGS, PR_FLAGS_SET);
    cpu.write8(o + O_KIND, PR_KIND as u8);
    cpu.write8(o + O_FRAME, 0);
    let ty = cpu.read8(e + E_TYPE) as u32;
    let base = cpu.read16(kind(PR_KIND) + K_COLOUR) as i16 as i32 as u32;
    let colour = base.wrapping_add(ty << 4) as u16;
    cpu.write16(o + O_COLOUR_A, colour);
    cpu.write16(o + O_COLOUR_B, colour);
    cpu.write8(o + O_NUMBER, n as u8);
    let number = cpu.read8(o + O_NUMBER) as i8 as i32 as u32;
    cpu.write32(NUMBERED.wrapping_add(number << 2), o);
    cpu.write32(o + O_SCRIPT, PR_SCRIPT);
    cpu.write32(o + P_POWER, PR_POWER);
    cpu.write8(o + O_54, 1);
    let ty = cpu.read8(e + E_TYPE);
    cpu.write8(o + O_55, ty);
    cpu.write16(o + O_ANGLE, 0x8000);
    cpu.write16(o + PL_HEADING, 0x8000);
    let at = cpu.read16(e + PR_AT);
    let x = (cpu.read16(PR_TABLE + 2 * at as u32) as i16 as i32 as u32) << 16;
    cpu.write32(o + O_POSITION, x);
    cpu.write32(o + O_POSITION + 4, PR_START_Y);
    cpu.write32(o + O_POSITION + 8, 0);
    cpu.write32(o + OR_SPEED + 4, PR_FALL);
    let at = cpu.read16(e + PR_AT).wrapping_add(1);
    cpu.write16(e + PR_AT, at);
    if cpu.read16(PR_TABLE + 2 * at as u32) as i16 == -1 {
        return 1;
    }
    if at as i16 > 8 {
        let ty = cpu.read8(e + E_TYPE).wrapping_sub(1);
        cpu.write8(e + E_TYPE, ty);
        if (ty as i8) < 0 {
            cpu.write8(e + E_TYPE, 2);
        }
    }
    platform_row_tick(cpu, e)
}

fn platform_row_tick(cpu: &mut Cpu, e: u32) -> u32 {
    let clock = cpu.read16(e + PR_CLOCK).wrapping_add(1);
    cpu.write16(e + PR_CLOCK, clock);
    0
}

// ---- the hangers: guns swung on a pendulum -------------------------------------------------------

/// A hanger's fields (its power is P_POWER, +0x50): the draw flags +0x2a (0x2000 while it is deep
/// into the scene), the flags word +0x7e (0x1000: it is lost outside the box below; 0x10: the
/// climb), the clock +0x8a and its end +0x8c, the swing's angle +0x90, the phase +0x92, the spin
/// +0x94, the lift +0xac (the swing's y), the drags +0xb0 (z) and +0xb4 (the radius's), the radius
/// +0xb8.
const HANG_DRAW_FLAGS: u32 = 0x2a;
const HANG_FLAGS: u32 = 0x7e;
const HANG_CLOCK: u32 = 0x8a;
const HANG_CLOCK_END: u32 = 0x8c;
const HANG_ANGLE: u32 = 0x90;
const HANG_PHASE: u32 = 0x92;
const HANG_SPIN: u32 = 0x94;
const HANG_LIFT: u32 = 0xac;
const HANG_DRAG: u32 = 0xb0;
const HANG_DRAG_R: u32 = 0xb4;
const HANG_RADIUS: u32 = 0xb8;
const HANG_EDGE: u16 = 0x1000;
const HANG_CLIMB: u16 = 0x10;
/// The box (top, bottom, left, right) it is lost outside of.
const HANG_AREA: [u32; 4] = [0xff40_0000, 0x00c0_0000, 0xff10_0000, 0x00f0_0000];
/// The swing's angle step per step.
const HANG_SWING_STEP: u16 = 0xc0;
/// z above which it is still deep (the phases turn there and at the next), and the far flag's z.
const HANG_DEEP: i32 = 0x5f_ffff;
const HANG_NEAR: i32 = 0x3_ffff;
const HANG_FAR: i32 = 0x7_ffff;
const HANG_FLAG_FAR: u32 = 0x8_0000;
const HANG_EFFECT_SCRIPT: u32 = 0x0607_3158;

/// A hanger's step (behaviour 0x0608a034). Power gone: blown up, done. Outside the box with its
/// edge flag (0x1000) on: done. Else `hanger_swing`. The clock (+0x8a) counts up each step.
pub fn hanger_step(cpu: &mut Cpu, o: u32) {
    let mut power = cpu.read32(o + P_POWER) as i32;
    let mut lost = false;
    if power > 0 && cpu.read16(o + HANG_FLAGS) & HANG_EDGE != 0 {
        let [top, bottom, left, right] = HANG_AREA;
        let out = cpu.nested_with_stack(&aim::AREA_FLAGS_PROLOGUE, aim::AREA_FLAGS, &[right], |cpu| aim::area_flags(cpu, o + O_FLAGS, top, bottom, left, right)) as u8;
        if out != 0 {
            power = cpu.read32(o + P_POWER) as i32;
            lost = true;
        }
    }
    if power <= 0 || lost {
        if power <= 0 {
            nested!(cpu, 0x0605_9e9c, ground::blown_up_1(cpu, o));
        }
        cpu.write8(o + O_MODE, MODE_DONE);
    } else {
        hanger_swing(cpu, o);
    }
    let clock = cpu.read16(o + HANG_CLOCK).wrapping_add(1);
    cpu.write16(o + HANG_CLOCK, clock);
}

/// The phases of a hanger's swing. Phase 0 holds it 10.0 above in z (the spin 0x7000) until the
/// clock passes its end; 1 lets it down until z is 0x5fffff and makes an effect there; 1 to 3
/// let the spin (by 0x1000) and the lift (by 0x2000) die down, 2 until z is 0x3ffff (the clock
/// restarted, the climb flag on, the phase on), 3 until the clock passes 120 (the climb flag
/// off, the edge flag on); 4 and the rest hold it. The radius's pull is -0x400000 (0, 5 and
/// more), -0x700000 (1 to 3), -0x2800000 (4). The angle turns by 0xc0 and the position follows it
/// (x the sine times the radius, y minus the cosine times it, plus the lift); the drags
/// approach what the depth (z, less 10.0 in phase 0) and the pull ask; the lift takes the spin, z
/// the drag, the radius its drag; the model turns by the angle and half a turn, the far flag (0x80000)
/// is on while z is more than 8.0 off, the draw flag 0x2000 while z is past 0x5fffff.
fn hanger_swing(cpu: &mut Cpu, o: u32) {
    let z = cpu.read32(o + O_POSITION + 8);
    let mut depth = z;
    let mut pull = cpu.read32(o + HANG_RADIUS).wrapping_add(0xffc0_0000);
    let phase = signed_word(cpu, o + HANG_PHASE);
    let next_phase = |cpu: &mut Cpu| {
        let next = cpu.read16(o + HANG_PHASE).wrapping_add(1);
        cpu.write16(o + HANG_PHASE, next);
    };
    let ease = |cpu: &mut Cpu| {
        let spin = signed_word(cpu, o + HANG_SPIN);
        let spin = nested!(cpu, 0x0604_6168, actors::approach(cpu, spin, 0, 0x1000));
        cpu.write16(o + HANG_SPIN, spin as u16);
        let lift = cpu.read32(o + HANG_LIFT);
        let lift = nested!(cpu, 0x0604_6168, actors::approach(cpu, lift, 0, 0x2000));
        cpu.write32(o + HANG_LIFT, lift);
    };
    match phase {
        0 => {
            depth = z.wrapping_add(0xff60_0000);
            cpu.write16(o + HANG_SPIN, 0x7000);
            if signed_word(cpu, o + HANG_CLOCK) as i32 > signed_word(cpu, o + HANG_CLOCK_END) as i32 {
                next_phase(cpu);
            }
        }
        1..=3 => {
            if phase == 1 && z as i32 <= HANG_DEEP {
                next_phase(cpu);
                hanger_effect(cpu, o);
            }
            if phase <= 2 {
                ease(cpu);
                if z as i32 <= HANG_NEAR {
                    next_phase(cpu);
                    cpu.write16(o + HANG_CLOCK, 0);
                    let flags = cpu.read16(o + HANG_FLAGS) | HANG_CLIMB;
                    cpu.write16(o + HANG_FLAGS, flags);
                }
            } else {
                ease(cpu);
                if signed_word(cpu, o + HANG_CLOCK) as i32 > 120 {
                    next_phase(cpu);
                    let flags = (cpu.read16(o + HANG_FLAGS) | HANG_EDGE) & !HANG_CLIMB;
                    cpu.write16(o + HANG_FLAGS, flags);
                }
            }
            pull = cpu.read32(o + HANG_RADIUS).wrapping_add(0xff90_0000);
        }
        4 => pull = cpu.read32(o + HANG_RADIUS).wrapping_add(0xfd80_0000),
        _ => {}
    }
    let angle = cpu.read16(o + HANG_ANGLE).wrapping_add(HANG_SWING_STEP);
    cpu.write16(o + HANG_ANGLE, angle);
    let radius = cpu.read32(o + HANG_RADIUS);
    let a = signed_word(cpu, o + HANG_ANGLE);
    let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    let x = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, sin, radius));
    cpu.write32(o + O_POSITION, x);
    let radius = cpu.read32(o + HANG_RADIUS);
    let a = signed_word(cpu, o + HANG_ANGLE);
    let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    let y = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, cos, radius.wrapping_neg()));
    let lift = cpu.read32(o + HANG_LIFT);
    cpu.write32(o + O_POSITION + 4, y.wrapping_add(lift));
    let q = nested!(cpu, 0x0600_6a78, math::quotient(cpu, depth, 0x300));
    let step = (q as i32).wrapping_abs() as u32;
    let to = nested!(cpu, 0x0600_6a78, math::quotient(cpu, depth.wrapping_neg(), 0x30));
    let from = cpu.read32(o + HANG_DRAG);
    let drag = nested!(cpu, 0x0604_6168, actors::approach(cpu, from, to, step));
    cpu.write32(o + HANG_DRAG, drag);
    let q = nested!(cpu, 0x0600_6a78, math::quotient(cpu, pull, 0x800));
    let step = (q as i32).wrapping_abs() as u32;
    let to = nested!(cpu, 0x0600_6a78, math::quotient(cpu, pull.wrapping_neg(), 0x80));
    let from = cpu.read32(o + HANG_DRAG_R);
    let drag = nested!(cpu, 0x0604_6168, actors::approach(cpu, from, to, step));
    cpu.write32(o + HANG_DRAG_R, drag);
    let spin = signed_word(cpu, o + HANG_SPIN);
    let lift = cpu.read32(o + HANG_LIFT).wrapping_add(spin);
    cpu.write32(o + HANG_LIFT, lift);
    let z = cpu.read32(o + O_POSITION + 8).wrapping_add(cpu.read32(o + HANG_DRAG));
    cpu.write32(o + O_POSITION + 8, z);
    let radius = cpu.read32(o + HANG_RADIUS).wrapping_add(cpu.read32(o + HANG_DRAG_R));
    cpu.write32(o + HANG_RADIUS, radius);
    let turn = cpu.read16(o + HANG_ANGLE).wrapping_add(0x8000);
    cpu.write16(o + O_ANGLE, turn);
    let flags = cpu.read32(o + O_FLAGS);
    let z = cpu.read32(o + O_POSITION + 8) as i32;
    cpu.write32(o + O_FLAGS, if z.wrapping_abs() > HANG_FAR { flags | HANG_FLAG_FAR } else { flags & !HANG_FLAG_FAR });
    let draw = cpu.read16(o + HANG_DRAW_FLAGS);
    cpu.write16(o + HANG_DRAW_FLAGS, if z > HANG_DEEP { draw | 0x2000 } else { draw & 0xdfff });
}

/// The effect a hanger makes as it drops into the scene (its description on the stack: the
/// position, 0x12000 sizes, 0x8030 and the script 0x06073158).
fn hanger_effect(cpu: &mut Cpu, o: u32) {
    let f = cpu.stack();
    cpu.write32(f + 0x3c, HANG_EFFECT_SCRIPT);
    for k in 0..3 {
        let v = cpu.read32(o + O_POSITION + 4 * k);
        cpu.write32(f + 4 * k, v);
    }
    cpu.write16(f + 32, 0x8030);
    cpu.write16(f + 24, 0);
    cpu.write32(f + 20, 0);
    cpu.write32(f + 12, 0x1_2000);
    cpu.write32(f + 16, 0x1_2000);
    nested!(cpu, 0x0605_7454, actors::drift_effect(cpu, f));
}

/// The hanger row's event (0x0608a428): three hangers made at the row's place (+4, +6 in 65536ths)
/// as ground guns (gun_set on a description built from the row: the type +0x1b, the direction
/// +0x14, the settings +0xa, the mirror +0xb, the kind +1 less 3), the scripts at 0x060ac6d4 and
/// 0x060ac6e0; each takes the behaviour 0x0608a034, z 10.0, the clock 0, the clock's end from the
/// row's +0x1c, and the angle 0x5555 times its number plus the row's direction. Gives 1.
pub fn hanger_row(cpu: &mut Cpu, e: u32) -> u32 {
    let d = cpu.stack();
    let x = (cpu.read16(e + E_PLACE) as i16 as i32 as u32) << 16;
    cpu.write32(d + G_PLACE, x);
    let y = (cpu.read16(e + E_PLACE + 2) as i16 as i32 as u32) << 16;
    cpu.write32(d + G_PLACE + 4, y);
    cpu.write32(d + G_PLACE + 8, 0);
    let t = cpu.read8(e + E_TYPE) as u16;
    cpu.write16(d + G_TYPE, t);
    let direction = cpu.read16(e + E_DIRECTION);
    cpu.write16(d + G_DIRECTION, direction);
    let settings = cpu.read8(e + E_SETTINGS) as u16;
    cpu.write16(d + G_SETTINGS, settings);
    let mirror = cpu.read8(e + E_MIRROR) as i8 as i32 as u32;
    cpu.write32(d + G_MIRROR, mirror);
    let kind = (cpu.read8(e + E_KIND) as i8 as i16).wrapping_sub(3) as u16;
    cpu.write16(d + G_KIND, kind);
    for k in 0..3u32 {
        let g = nested!(cpu, 0x0605_b168, ground::gun_set(cpu, d, HANG_SCRIPT, HANG_AIM));
        if g != 0 {
            cpu.write32(g + O_BEHAVIOUR, 0x0608_a034);
            cpu.write32(g + O_POSITION + 8, 0xa0_0000);
            cpu.write16(g + HANG_CLOCK, 0);
            let end = cpu.read16(e + E_1C);
            cpu.write16(g + HANG_CLOCK_END, end);
            let angle = cpu.read16(e + E_DIRECTION).wrapping_add((k * 0x5555) as u16);
            cpu.write16(g + HANG_ANGLE, angle);
            let y = cpu.read32(g + O_POSITION + 4);
            cpu.write32(g + HANG_LIFT, y);
        }
    }
    1
}

const HANG_SCRIPT: u32 = 0x060a_c6d4;
const HANG_AIM: u32 = 0x060a_c6e0;

// ---- the pods: a row of guns hung from the top, firing rockets ------------------------------------

/// A pod's rocket shot (called by the pod's step, 0x0608a514): a rocket shot described on the stack
/// at 4.0 right of and 16.0 below the pod (its number at +0x1e, the shot's colour 0x8000, its kind
/// word +0x30 65, +0x2e 20).
pub fn ceil_shot_make(cpu: &mut Cpu, o: u32) {
    let f = cpu.stack();
    cpu.write16(f + 0x30, 65);
    let n = cpu.read8(o + O_NUMBER);
    cpu.write8(f + 0x1e, n);
    cpu.write32(f + 8, 0);
    let x = cpu.read32(o + O_POSITION).wrapping_add(0x4_0000);
    cpu.write32(f, x);
    let y = cpu.read32(o + O_POSITION + 4).wrapping_add(0x10_0000);
    cpu.write32(f + 4, y);
    cpu.write16(f + 0x2c, 0x8000);
    cpu.write16(f + 0x2e, 20);
    nested!(cpu, 0x0605_5b5c, bullets::rocket_set(cpu, f));
}

/// A ceiling pod's fields: the phase byte +0x7c, the clock +0x8a (the horizontal gap to the
/// player, set at the start), the wait +0x8c (frames before it fires again), the x speed +0x80.
const CP_PHASE: u32 = 0x7c;
const CP_GAP: u32 = 0x8a;
const CP_WAIT: u32 = 0x8c;
const CP_SPEED: u32 = 0x80;
/// The pod's wait between shots, the fire's reach (x about 4.0 right of the player, y within 128.0
/// of it), the y gap that starts it moving (within 32.0), the speeds it slides to by the gap's
/// side, and the speed step.
const CP_WAIT_SHOTS: u16 = 20;
const CP_FIRE_X: i32 = 0xb_ffff;
const CP_FIRE_X_OFFSET: u32 = 0x4_0000;
const CP_FIRE_Y: i32 = 0x7f_ffff;
const CP_START_Y: i32 = 0x1f_ffff;
const CP_SPEED_TO: [u32; 2] = [0xfffe_1000, 0x1_f000];
const CP_SPEED_STEP: u32 = 0x2000;
/// The frame is the speed plus 0x22000 on 0x4000s.
const CP_FRAME_BASE: u32 = 0x2_2000;

/// A ceiling pod's step (behaviour 0x0608a574). Off the screen or out of power: done (blown up
/// for the power). Phase 0 waits until the player is within 32.0 in y, then notes the gap in x
/// (as whole units) and goes to phase 1; its wait counts down; it fires (ceil_shot_make, the wait
/// 20) when the player is within 12.0 in x of it (plus 4.0) and 128.0 in y. Phase 1 slides its
/// x speed (by 0x2000) toward 0x1f000 when the gap was right of it and -0x1ef000 otherwise. Then
/// the ground's scroll comes off its y, its frame follows its speed (over 0x4000s) and its x moves.
pub fn ceil_step(cpu: &mut Cpu, o: u32) {
    let s = o + O_FLAGS;
    let off = nested!(cpu, 0x0605_6a24, aim::off_screen(cpu, s)) as u8;
    if off != 0 || cpu.read32(o + P_POWER) as i32 <= 0 {
        if off == 0 {
            nested!(cpu, 0x0605_9e9c, ground::blown_up_1(cpu, o));
        }
        cpu.write8(o + O_MODE, MODE_DONE);
        return;
    }
    let p = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, s));
    let phase = cpu.read8(o + CP_PHASE);
    let gap_y = |cpu: &mut Cpu| (cpu.read32(o + O_POSITION + 4).wrapping_sub(cpu.read32(p + O_POSITION + 4)) as i32).wrapping_abs();
    if phase == 0 {
        if gap_y(cpu) <= CP_START_Y {
            cpu.write8(o + CP_PHASE, phase.wrapping_add(1));
            let mut dx = cpu.read32(o + O_POSITION).wrapping_sub(cpu.read32(p + O_POSITION)) as i32;
            if dx < 0 {
                dx = dx.wrapping_add(0xffff);
            }
            cpu.write16(o + CP_GAP, (dx >> 16) as u16);
        }
        let wait = cpu.read16(o + CP_WAIT);
        if wait as i16 != 0 {
            cpu.write16(o + CP_WAIT, wait.wrapping_sub(1));
        } else {
            let dx = cpu.read32(o + O_POSITION).wrapping_sub(cpu.read32(p + O_POSITION).wrapping_add(CP_FIRE_X_OFFSET)) as i32;
            if dx.wrapping_abs() <= CP_FIRE_X && gap_y(cpu) <= CP_FIRE_Y {
                cpu.write16(o + CP_WAIT, CP_WAIT_SHOTS);
                nested!(cpu, 0x0608_a514, ceil_shot_make(cpu, o));
            }
        }
    } else if phase == 1 {
        let to = CP_SPEED_TO[(signed_word(cpu, o + CP_GAP) as i32 > 0) as usize];
        let from = cpu.read32(o + CP_SPEED);
        let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, from, to, CP_SPEED_STEP));
        cpu.write32(o + CP_SPEED, v);
    }
    let scroll = ground_scroll(cpu);
    let y = cpu.read32(o + O_POSITION + 4).wrapping_sub(scroll);
    cpu.write32(o + O_POSITION + 4, y);
    let mut v = cpu.read32(o + CP_SPEED).wrapping_add(CP_FRAME_BASE) as i32;
    if v < 0 {
        v = v.wrapping_add(0x3fff);
    }
    cpu.call(0x0600_8158, &[v as u32]);
    let frame = cpu.reg(4);
    cpu.write8(o + O_FRAME, frame as u8);
    let x = cpu.read32(o + O_POSITION).wrapping_add(cpu.read32(o + CP_SPEED));
    cpu.write32(o + O_POSITION, x);
}

/// The ground's scroll this frame (the word the ground keeps its step at points to).
fn ground_scroll(cpu: &mut Cpu) -> u32 {
    let at = cpu.read32(super::globals::GROUND_DY_AT);
    cpu.read32(at)
}

const CP_SIZE: u32 = 0xbc;
const CP_KIND: u32 = 20;
const CP_FRAME: u8 = 8;
const CP_POWER: u32 = 0x578;
const CP_SCRIPT: u32 = 0x060a_c6e4;
const CP_START_Y_AT: u32 = 0xff80_0000;
/// The pod's 0xa8 word (the row's +1, its kind), and the row's step in x (40 on 65536ths).
const CP_KIND_WORD: u32 = 0xa8;
const CP_PITCH: u16 = 40;
const CP_START: u32 = 0x0608_a574;

/// An event row of ceiling pods (0x0608a6f0): while its count (+0x10) lasts, a pod (an object of list
/// 2, 0xbc bytes, kind 20 with the kind's colour plus the row's type (+0x1b) times 16, frame 8,
/// power 0x578) at the row's x (+4, in 65536ths; the row steps it by 40), y -128.0, z 0, its
/// +0xa8 the row's +1 and its +0xac its x. Gives 0 when there is no number or room, 1 when the
/// count is out.
pub fn ceil_row(cpu: &mut Cpu, e: u32) -> u32 {
    if cpu.read8(e + E_COUNT) == 0 {
        return 1;
    }
    loop {
        let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, 2)) as i8;
        if n == -1 {
            return 0;
        }
        let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, CP_SIZE)) as i8 as i32 as u32;
        let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, PART_EXEC));
        if o == 0 {
            return 0;
        }
        nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
        nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + 0x3c));
        nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
        cpu.write32(o + O_BEHAVIOUR, CP_START);
        cpu.write8(o + O_KIND, CP_KIND as u8);
        cpu.write8(o + O_FRAME, CP_FRAME);
        let ty = cpu.read8(e + E_TYPE) as u32;
        let base = cpu.read16(kind(CP_KIND) + K_COLOUR) as i16 as i32 as u32;
        let colour = base.wrapping_add(ty << 4) as u16;
        cpu.write16(o + O_COLOUR_A, colour);
        cpu.write16(o + O_COLOUR_B, colour);
        cpu.write8(o + O_NUMBER, n as u8);
        let number = cpu.read8(o + O_NUMBER) as i8 as i32 as u32;
        cpu.write32(NUMBERED.wrapping_add(number << 2), o);
        cpu.write32(o + O_SCRIPT, CP_SCRIPT);
        cpu.write32(o + P_POWER, CP_POWER);
        cpu.write8(o + O_54, 1);
        let ty = cpu.read8(e + E_TYPE);
        cpu.write8(o + O_55, ty);
        let x = (cpu.read16(e + E_PLACE) as i16 as i32 as u32) << 16;
        cpu.write32(o + O_POSITION, x);
        cpu.write32(o + O_POSITION + 4, CP_START_Y_AT);
        cpu.write32(o + O_POSITION + 8, 0);
        let at = cpu.read16(e + E_PLACE).wrapping_add(CP_PITCH);
        cpu.write16(e + E_PLACE, at);
        let kind_word = cpu.read8(e + E_KIND) as i8 as i16 as u16;
        cpu.write16(o + CP_KIND_WORD, kind_word);
        let x = cpu.read32(o + O_POSITION);
        cpu.write32(o + HANG_LIFT, x);
        let count = cpu.read8(e + E_COUNT).wrapping_sub(1);
        cpu.write8(e + E_COUNT, count);
        if count == 0 {
            return 1;
        }
    }
}

/// The chip's step (behaviour 0x0608a834): done once its y passes 216.0; else its turn drops by
/// 160, the ground's scroll comes off its y and its flags' 0x30 flip (a flicker).
pub fn chip_step(cpu: &mut Cpu, o: u32) {
    if cpu.read32(o + O_POSITION + 4) as i32 > 0xd8_0000 {
        cpu.write8(o + O_MODE, MODE_DONE);
        return;
    }
    let turn = cpu.read16(o + O_ANGLE).wrapping_add(0xff60);
    cpu.write16(o + O_ANGLE, turn);
    let scroll = ground_scroll(cpu);
    let y = cpu.read32(o + O_POSITION + 4).wrapping_sub(scroll);
    cpu.write32(o + O_POSITION + 4, y);
    let flags = cpu.read32(o + O_FLAGS) ^ 0x30;
    cpu.write32(o + O_FLAGS, flags);
}

// ---- the chips ---------------------------------------------------------------------------

const CHIP_KIND: u32 = 9;
const CHIP_FLAGS: u32 = 0xc6_0000;
const CHIP_POWER: u32 = 10;
const CHIP_SCRIPT: u32 = 0x060a_c6ec;
const CHIP_START_Y: u32 = 0xff40_0000;
const CHIP_DRAW: u32 = 0x0608_a878;
const CHIP_STEP: u32 = 0x0608_a834;
/// The four copies' ring: its radius 88.0 on the sine, and the angle step.
const CHIP_RING: i32 = 88;
const CHIP_TURN_STEP: u32 = 0x4000;

/// The chip's draw (0x0608a878): four copies of the sprite (the object's kind and frame, its colour,
/// z, the scales 1.0 and 0.5, the lift -8) round it, a quarter turn apart from the object's turn
/// (+0x30), each at 88 times the sine and cosine from its place.
pub fn chip_draw(cpu: &mut Cpu, s: u32) {
    let o = s.wrapping_sub(0x10);
    let d = cpu.stack();
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, d));
    let v = cpu.read8(s + S_KIND);
    cpu.write8(d + S_KIND, v);
    let v = cpu.read8(s + S_FRAME);
    cpu.write8(d + S_FRAME, v);
    cpu.write8(d + S_LIFT, 0xf8);
    let z = cpu.read32(o + O_POSITION + 8);
    cpu.write32(d + S_POSITION + 8, z);
    let colour = cpu.read16(s + S_COLOUR);
    cpu.write16(d + S_COLOUR, colour);
    cpu.write32(d + S_SCALES, 0x1_0000);
    cpu.write32(d + S_SCALES + 4, 0x8000);
    let mut step = 0;
    while step <= 0xffff {
        let turn = signed_word(cpu, o + O_ANGLE).wrapping_add(step);
        cpu.write16(d + S_TURN, turn as u16);
        let a = cpu.read16(d + S_TURN) as i16 as i32 as u32;
        let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a)) as i32;
        let x = cpu.read32(o + O_POSITION).wrapping_add(sin.wrapping_mul(CHIP_RING) as u32);
        cpu.write32(d + S_POSITION, x);
        let a = cpu.read16(d + S_TURN) as i16 as i32 as u32;
        let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a)) as i32;
        let y = cpu.read32(o + O_POSITION + 4).wrapping_sub(cos.wrapping_mul(CHIP_RING) as u32);
        cpu.write32(d + S_POSITION + 4, y);
        nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
        step += CHIP_TURN_STEP;
    }
}

/// The chip's event row (0x0608a940): one chip (an object of list 2, 0xbc bytes, kind and frame 9
/// with the kind's colour, power 10, flags 0xc60000, at x 0, y -192.0, z 0, its turn the row's
/// direction +0x14, step 0x0608a834, draw 0x0608a878). Gives 1 (also with no number or room).
pub fn chip_row(cpu: &mut Cpu, e: u32) -> u32 {
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, 2)) as i8;
    if n == -1 {
        return 1;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, CP_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, PART_EXEC));
    if o == 0 {
        return 1;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + 0x3c));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + O_BEHAVIOUR, CHIP_STEP);
    cpu.write32(o + O_FLAGS, CHIP_FLAGS);
    cpu.write32(o + O_DRAW, CHIP_DRAW);
    cpu.write8(o + O_KIND, CHIP_KIND as u8);
    cpu.write8(o + O_FRAME, CHIP_KIND as u8);
    let colour = cpu.read16(kind(CHIP_KIND) + K_COLOUR);
    cpu.write16(o + O_COLOUR_A, colour);
    cpu.write8(o + O_NUMBER, n as u8);
    let number = cpu.read8(o + O_NUMBER) as i8 as i32 as u32;
    cpu.write32(NUMBERED.wrapping_add(number << 2), o);
    cpu.write32(o + O_SCRIPT, CHIP_SCRIPT);
    cpu.write32(o + P_POWER, CHIP_POWER);
    cpu.write8(o + O_54, 1);
    let ty = cpu.read8(e + E_TYPE);
    cpu.write8(o + O_55, ty);
    cpu.write32(o + O_POSITION, 0);
    cpu.write32(o + O_POSITION + 4, CHIP_START_Y);
    cpu.write32(o + O_POSITION + 8, 0);
    let turn = cpu.read16(e + E_DIRECTION);
    cpu.write16(o + O_ANGLE, turn);
    1
}

// ---- the flaps: a hatch of four flaps that lets enemies out --------------------------------------

/// A flap hatch's fields (its power is P_POWER, its hit bits word +0x56, its side +0x88, its speed
/// +0x80 and y speed +0x84, its z +0xb0): the flags word +0x7e (0x1000: it is lost outside the box;
/// 1: hit; 0x10, 0x20, 0x40, 0x80: that flap opens), the four flaps' openings (words +0x9e top,
/// +0xa0 bottom, +0xa2 left, +0xa4 right, each 0 to 24), and what its release makes (+0xbc: started,
/// +0xbe: frames to the next, +0xc0: made so far, +0xc2 mirror, +0xc4 direction, +0xc6 type,
/// +0xc7 kind, +0xc8 settings, +0xc9 script number).
const FL_FLAGS: u32 = 0x7e;
const FL_SIDE: u32 = 0x88;
const FL_SPEED: u32 = 0x80;
const FL_SPEED_Y: u32 = 0x84;
const FL_Z: u32 = 0xb0;
const FL_HIT_BITS: u32 = 0x56;
const FL_ROUTE: u32 = 0x6c;
const FL_OPEN: [u32; 4] = [0x9e, 0xa0, 0xa2, 0xa4];
const FL_OPEN_FLAGS: [u16; 4] = [0x10, 0x20, 0x40, 0x80];
const FL_FULL: u32 = 24;
const FL_STARTED: u32 = 0xbc;
const FL_WAIT: u32 = 0xbe;
const FL_MADE: u32 = 0xc0;
const FL_MIRROR: u32 = 0xc2;
const FL_DIRECTION: u32 = 0xc4;
const FL_TYPE: u32 = 0xc6;
const FL_KIND: u32 = 0xc7;
const FL_SETTINGS: u32 = 0xc8;
const FL_SCRIPT_NO: u32 = 0xc9;
const FL_SCRIPTS: u32 = 0x060a_c7d0;
const FL_ROUTES: u32 = 0x060a_c78c;
const FL_SCRIPT: u32 = 0x060a_c7d8;
const FL_SIZE: u32 = 0xcc;
const FL_FLAGS_ROW: u32 = 0xc8_0000;
/// The kinds' colour at +0xcc (kind 12's) the hatch takes.
const FL_COLOURS: u32 = 0xcc;
const FL_POWER: u32 = 5000;
const FL_SCORE: u32 = 100;
const FL_START: u32 = 0x0608_af44;
const FL_DRAW: u32 = 0x0608_b1b4;
const FL_AREA: [u32; 4] = [0xff70_0000, 0x0090_0000, 0xff30_0000, 0x00d0_0000];
const FL_HATCH_KIND: u8 = 21;
const FL_MADE_MAX: i32 = 2;
const FL_PAUSE: u16 = 18;

/// A flap hatch's step (behaviour 0x0608af44). Lost outside its box (with 0x1000 on), or out of power
/// (scored 100 and blown up as a burster): done. Else a hit bit (0x200) sets its flag 1; its z (+0x1c)
/// past 4.0 sets the far flag (0x80000); the route's step (+0x6c) turns flags on: 7 flag 0x10, 9 off
/// all of 0xf0, 11 0x20, 13 0x40 (0x80 with the side positive), 15 0x80 (0x40 with the side
/// positive). It moves by its speed times its side (x), its y speed less the ground's scroll (y) and
/// takes z from +0xb0. Each flap's opening (words +0x9e, +0xa0, +0xa2, +0xa4) approaches 24 by 2 when
/// its flag is on, else 0; with any one full it releases (flap_release).
pub fn flap_step(cpu: &mut Cpu, o: u32) {
    let mut done = false;
    if cpu.read16(o + FL_FLAGS) & 0x1000 != 0 {
        let [top, bottom, left, right] = FL_AREA;
        let out = cpu.nested_with_stack(&aim::AREA_FLAGS_PROLOGUE, aim::AREA_FLAGS, &[right], |cpu| aim::area_flags(cpu, o + O_FLAGS, top, bottom, left, right)) as u8;
        done = out != 0;
    }
    if !done && cpu.read32(o + P_POWER) as i32 <= 0 {
        nested!(cpu, 0x0605_61ac, hits::score_hit_by(cpu, o, FL_SCORE));
        nested!(cpu, 0x0605_9fc4, ground::blown_up_burster(cpu, o));
        done = true;
    }
    if done {
        cpu.write8(o + O_MODE, MODE_DONE);
        return;
    }
    if cpu.read16(o + FL_HIT_BITS) & 0x200 != 0 {
        let flags = cpu.read16(o + FL_FLAGS) | 1;
        cpu.write16(o + FL_FLAGS, flags);
    }
    let z = (cpu.read32(o + O_POSITION + 8) as i32).wrapping_abs();
    let flags = cpu.read32(o + O_FLAGS);
    cpu.write32(o + O_FLAGS, if z > 0x4_0000 { flags | HANG_FLAG_FAR } else { flags & !HANG_FLAG_FAR });
    let route = cpu.read32(o + FL_ROUTE);
    let r = nested!(cpu, 0x0605_a000, ground::route_step(cpu, o, route)) as u8;
    let flags = cpu.read16(o + FL_FLAGS);
    let side_up = cpu.read8(o + FL_SIDE) as i8 > 0;
    match r {
        7 => cpu.write16(o + FL_FLAGS, flags | 0x10),
        9 => cpu.write16(o + FL_FLAGS, flags & 0xff0f),
        11 => cpu.write16(o + FL_FLAGS, flags | 0x20),
        13 => cpu.write16(o + FL_FLAGS, flags | if side_up { 0x40 } else { 0x80 }),
        15 => cpu.write16(o + FL_FLAGS, flags | if side_up { 0x80 } else { 0x40 }),
        _ => {}
    }
    let side = cpu.read8(o + FL_SIDE) as i8 as i32;
    let x = cpu.read32(o + O_POSITION).wrapping_add((cpu.read32(o + FL_SPEED) as i32).wrapping_mul(side) as u32);
    cpu.write32(o + O_POSITION, x);
    let scroll = ground_scroll(cpu);
    let dy = cpu.read32(o + FL_SPEED_Y).wrapping_sub(scroll);
    let y = cpu.read32(o + O_POSITION + 4).wrapping_add(dy);
    cpu.write32(o + O_POSITION + 4, y);
    let z = cpu.read32(o + FL_Z);
    cpu.write32(o + O_POSITION + 8, z);
    for (at, bit) in FL_OPEN.into_iter().zip(FL_OPEN_FLAGS) {
        let to = if cpu.read16(o + FL_FLAGS) & bit != 0 { FL_FULL } else { 0 };
        let from = signed_word(cpu, o + at);
        let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, from, to, 2));
        cpu.write16(o + at, v as u16);
    }
    if FL_OPEN.into_iter().any(|at| signed_word(cpu, o + at) == FL_FULL) {
        nested!(cpu, 0x0608_ae5c, flap_release(cpu, o));
    }
}

/// A flap hatch's release (0x0608ae5c): the first time it starts (+0xbc) with 16 frames to wait; then
/// each time its wait (+0xbe) is below 0 (until two were made: +0xc0 counts them) it makes an enemy
/// at its place (a hatch for kind 21, else a ground one, by its description +0xc2 to +0xc9, the
/// script by +0xc9 in the table 0x060ac7d0) and waits 18 frames; the wait counts down each time.
pub fn flap_release(cpu: &mut Cpu, o: u32) {
    if cpu.read8(o + FL_STARTED) == 0 {
        let v = cpu.read8(o + FL_STARTED).wrapping_add(1);
        cpu.write8(o + FL_STARTED, v);
        cpu.write16(o + FL_WAIT, 16);
        cpu.write16(o + FL_MADE, 0);
    } else if signed_word(cpu, o + FL_MADE) as i32 > FL_MADE_MAX {
        return;
    }
    if (cpu.read16(o + FL_WAIT) as i16) < 0 {
        cpu.write16(o + FL_WAIT, FL_PAUSE);
        let d = cpu.stack();
        let x = cpu.read32(o + O_POSITION);
        cpu.write32(d + G_PLACE, x);
        let y = cpu.read32(o + O_POSITION + 4);
        cpu.write32(d + G_PLACE + 4, y);
        cpu.write32(d + G_PLACE + 8, 0);
        let v = cpu.read8(o + FL_TYPE) as u16;
        cpu.write16(d + G_TYPE, v);
        let v = cpu.read16(o + FL_DIRECTION);
        cpu.write16(d + G_DIRECTION, v);
        let v = cpu.read8(o + FL_SETTINGS) as u16;
        cpu.write16(d + G_SETTINGS, v);
        let v = cpu.read8(o + FL_MIRROR) as i8 as i32 as u32;
        cpu.write32(d + G_MIRROR, v);
        let kind_byte = cpu.read8(o + FL_KIND);
        if kind_byte == FL_HATCH_KIND {
            let n = cpu.read8(o + FL_SCRIPT_NO) as u32;
            let s = cpu.read32(FL_SCRIPTS + 4 * n);
            nested!(cpu, 0x0605_e030, ground::hatch_set(cpu, d, s));
        } else {
            cpu.write16(d + G_KIND, kind_byte as u16);
            let n = cpu.read8(o + FL_SCRIPT_NO) as u32;
            let s = cpu.read32(FL_SCRIPTS + 4 * n);
            nested!(cpu, 0x0605_a780, ground::ground_set(cpu, d, s));
        }
        let made = cpu.read16(o + FL_MADE).wrapping_add(1);
        cpu.write16(o + FL_MADE, made);
    }
    let wait = cpu.read16(o + FL_WAIT).wrapping_sub(1);
    cpu.write16(o + FL_WAIT, wait);
}

/// A flap hatch's draw (0x0608b1b4): its sprite (flags 32, the object's kind and frame, lift -6,
/// its place; the colour of the first kind plus 0xe0 while its flag 1 is on, else the sprite's; flag 2
/// goes when it was on, set when flag 1 is on), then a flap sprite (frame 1, lift -9) for each
/// opening (+0x9e above by it, +0xa0 below with turn 0x8000, +0xa2 left with 0xc000, +0xa4 right
/// with 0x4000, by whole units) that is not 0.
pub fn flap_draw(cpu: &mut Cpu, s: u32) {
    let o = s.wrapping_sub(0x10);
    let d = cpu.stack();
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, d));
    let at = o + FL_FLAGS;
    let flags = cpu.read16(at);
    if flags & 2 != 0 {
        cpu.write16(at, flags & 0xfffc);
    }
    let flags = cpu.read16(at);
    if flags & 1 != 0 {
        cpu.write16(at, flags | 2);
    }
    cpu.write32(d + S_FLAGS, 32);
    let v = cpu.read8(s + S_KIND);
    cpu.write8(d + S_KIND, v);
    let v = cpu.read8(s + S_FRAME);
    cpu.write8(d + S_FRAME, v);
    cpu.write8(d + S_LIFT, 0xfa);
    for k in 0..3 {
        let v = cpu.read32(o + O_POSITION + 4 * k);
        cpu.write32(d + S_POSITION + 4 * k, v);
    }
    if cpu.read16(at) & 1 != 0 {
        let base = cpu.read16(kind(0) + K_COLOUR);
        cpu.write16(d + S_COLOUR, base.wrapping_add(0xe0));
    } else {
        let colour = cpu.read16(s + S_COLOUR);
        cpu.write16(d + S_COLOUR, colour);
    }
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
    cpu.write32(d + S_FLAGS, 0);
    cpu.write8(d + S_FRAME, 1);
    cpu.write8(d + S_LIFT, 0xf7);
    // (above, below, left, right: the turn each, and which way it moves)
    for (k, at) in FL_OPEN.into_iter().enumerate() {
        if cpu.read16(o + at) == 0 {
            continue;
        }
        let open = signed_word(cpu, o + at) << 16;
        let (x, y) = (cpu.read32(o + O_POSITION), cpu.read32(o + O_POSITION + 4));
        match k {
            0 => {
                cpu.write32(d + S_POSITION, x);
                cpu.write32(d + S_POSITION + 4, y.wrapping_sub(open));
            }
            1 => {
                cpu.write16(d + S_TURN, 0x8000);
                cpu.write32(d + S_POSITION, x);
                cpu.write32(d + S_POSITION + 4, y.wrapping_add(open));
            }
            2 => {
                cpu.write16(d + S_TURN, 0xc000);
                cpu.write32(d + S_POSITION, x.wrapping_sub(open));
                cpu.write32(d + S_POSITION + 4, y);
            }
            _ => {
                cpu.write16(d + S_TURN, 0x4000);
                cpu.write32(d + S_POSITION, x.wrapping_add(open));
                cpu.write32(d + S_POSITION + 4, y);
            }
        }
        nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
    }
}

/// The flap hatch's event row (0x0608b308): a hatch (an object of list 2, 0xcc bytes, kind 15, power
/// 5000, flags 0xc80000, step 0x0608af44, draw 0x0608b1b4, the kinds' colour at +0xcc plus the row's
/// type times 16) at the row's place, its route by the row's script number (+8, in the table at
/// 0x060ac78c), its side the row's +0xb, and its release's description from the row's +0x1c to
/// +0x1f, +0x11, +0x10 (the direction the +0x1c on 256ths). 1, or 0 with no number or room.
pub fn flap_row(cpu: &mut Cpu, e: u32) -> u32 {
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, 2)) as i8;
    if n == -1 {
        return 0;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, FL_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, PART_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + 0x3c));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + O_BEHAVIOUR, FL_START);
    cpu.write32(o + O_FLAGS, FL_FLAGS_ROW);
    cpu.write32(o + O_DRAW, FL_DRAW);
    cpu.write8(o + O_KIND, 15);
    cpu.write8(o + O_FRAME, 0);
    cpu.write8(o + O_NUMBER, n as u8);
    let number = cpu.read8(o + O_NUMBER) as i8 as i32 as u32;
    cpu.write32(NUMBERED.wrapping_add(number << 2), o);
    cpu.write32(o + O_SCRIPT, FL_SCRIPT);
    cpu.write32(o + P_POWER, FL_POWER);
    cpu.write8(o + O_54, 1);
    let ty = cpu.read8(e + E_TYPE);
    cpu.write8(o + O_55, ty);
    let base = cpu.read16(KINDS + FL_COLOURS) as i16 as i32 as u32;
    let number = cpu.read8(o + O_55) as i8 as i32 as u32;
    let colour = base.wrapping_add(number << 4) as u16;
    cpu.write16(o + O_COLOUR_A, colour);
    cpu.write16(o + O_COLOUR_B, colour);
    let x = (cpu.read16(e + E_PLACE) as i16 as i32 as u32) << 16;
    cpu.write32(o + O_POSITION, x);
    let y = (cpu.read16(e + E_PLACE + 2) as i16 as i32 as u32) << 16;
    cpu.write32(o + O_POSITION + 4, y);
    cpu.write32(o + O_POSITION + 8, 0);
    let script = cpu.read8(e + E_SCRIPT) as u32;
    let route = cpu.read32(FL_ROUTES + 4 * script);
    cpu.write32(o + FL_ROUTE, route);
    let v = cpu.read8(e + E_MIRROR);
    cpu.write8(o + FL_SIDE, v);
    cpu.write8(o + FL_STARTED, 0);
    let v = (cpu.read8(e + E_1C) as i8 as i16 as u16) << 8;
    cpu.write16(o + FL_DIRECTION, v);
    for (to, from) in [(FL_MIRROR, E_1D), (FL_TYPE, E_1E), (FL_KIND, E_1F), (FL_SETTINGS, E_DELAY), (FL_SCRIPT_NO, E_COUNT)] {
        let v = cpu.read8(e + from);
        cpu.write8(o + to, v);
    }
    1
}

/// A falling bounce (0x0608b49c): the lift (+0xac) takes the speed (+0xb0), y that and 0x7000; while
/// the lift is below `floor` the speed approaches 2.0 (by 0x400); else it turns round at a quarter of
/// its size and the lift is the floor (y takes no change, as the code's own sum is nil).
pub fn bounce_step(cpu: &mut Cpu, o: u32, floor: u32) {
    let lift = cpu.read32(o + HANG_LIFT).wrapping_add(cpu.read32(o + HANG_DRAG));
    cpu.write32(o + HANG_LIFT, lift);
    let y = cpu.read32(o + O_POSITION + 4).wrapping_add(0x7000).wrapping_add(cpu.read32(o + HANG_DRAG));
    cpu.write32(o + O_POSITION + 4, y);
    if (cpu.read32(o + HANG_LIFT) as i32) < floor as i32 {
        let speed = cpu.read32(o + HANG_DRAG);
        let speed = nested!(cpu, 0x0604_6168, actors::approach(cpu, speed, 0x2_0000, 0x400));
        cpu.write32(o + HANG_DRAG, speed);
    } else {
        let v = (cpu.read32(o + HANG_DRAG) as i32).wrapping_neg();
        let v = v.wrapping_add(if v < 0 { 3 } else { 0 }) >> 2;
        cpu.write32(o + HANG_DRAG, v as u32);
        cpu.write32(o + HANG_LIFT, floor);
        let y = cpu.read32(o + O_POSITION + 4);
        cpu.write32(o + O_POSITION + 4, y);
    }
}

// ---- the end's scenery, the volleys and the mounts ----------------------------------------------

/// The scenery's words: three heights the rounds raise (SCENERY, by their kind 0-2), and the six
/// objects (SCENERY + 0x10, by kind 0-5: rounds then plates); the kind of a round or plate is at +0x7c.
const SC_KIND: u32 = 0x7c;
const SC_OBJECTS: u32 = SCENERY + 0x10;
const SC_RAISE: u32 = 0x1a_0000;
const SC_GONE_Y: i32 = 0xe0_0000;
const SC_SLIDE: u32 = 0x7000;
const SC_STAGE: u32 = 0xa8;
const SC_SIZE: u32 = 0xbc;
const SC_SET_MOVE: u32 = 0x0608_b814;
const SC_PLATE_STEP: u32 = 0x0608_b518;
const SC_PLATE_DRAW: u32 = 0x0608_b664;
const SC_ROUND_STEP: u32 = 0x0608_b82c;
const SC_ROUND_DRAW: u32 = 0x0608_95d8;

/// A plate's step (behaviour 0x0608b518): past y 224.0 it is done and its place in the scenery's
/// objects (by kind, +0x7c) cleared. Its stage (+0xa8) begins at 0; the kinds below 4 add the
/// plates' pieces (mount_make) at -184.0 (stage 1, with a turn of 0x8000 for the first, 0 for the
/// others) and at -144.0 (stage 2: slots 3 to 6); kind 4 makes slots 1 and 2 (turn 0x8000), kind 5
/// slot 0. Then bounce_step over the floor (the scenery's heights summed: kind 5 the last, 4 the last
/// two, the rest all three).
pub fn plate_step(cpu: &mut Cpu, o: u32) {
    let y = cpu.read32(o + O_POSITION + 4) as i32;
    if y > SC_GONE_Y {
        cpu.write8(o + O_MODE, MODE_DONE);
        let kind = cpu.read8(o + SC_KIND) as u32;
        cpu.write32(SC_OBJECTS + 4 * kind, 0);
        return;
    }
    let kind = cpu.read8(o + SC_KIND);
    let [h0, h1, h2] = [0, 1, 2].map(|k| cpu.read32(SCENERY + 4 * k));
    let stage = |cpu: &mut Cpu| cpu.read16(o + SC_STAGE);
    let floor = match kind {
        5 => {
            if stage(cpu) == 0 && y > 0xff48_0000u32 as i32 {
                cpu.write16(o + SC_STAGE, 1);
                nested!(cpu, 0x0608_bcd0, mount_make(cpu, o, 0, 0x8000, 0));
            }
            h2
        }
        4 => {
            if stage(cpu) == 0 && y > 0xff48_0000u32 as i32 {
                cpu.write16(o + SC_STAGE, 1);
                nested!(cpu, 0x0608_bcd0, mount_make(cpu, o, 0, 0x8000, 1));
                nested!(cpu, 0x0608_bcd0, mount_make(cpu, o, 0, 0x8000, 2));
            }
            h2.wrapping_add(h1)
        }
        _ => {
            if stage(cpu) == 0 && y > 0xff48_0000u32 as i32 {
                cpu.write16(o + SC_STAGE, 1);
                nested!(cpu, 0x0608_bcd0, mount_make(cpu, o, 0, 0x8000, 0));
            }
            if stage(cpu) == 1 && cpu.read32(o + O_POSITION + 4) as i32 > 0xff70_0000u32 as i32 {
                cpu.write16(o + SC_STAGE, 2);
                for slot in 3..=6 {
                    nested!(cpu, 0x0608_bcd0, mount_make(cpu, o, 0, 0, slot));
                }
            }
            h2.wrapping_add(h1).wrapping_add(h0)
        }
    };
    nested!(cpu, 0x0608_b49c, bounce_step(cpu, o, floor));
}

/// A plate's draw (0x0608b664): the sprite (the object's kind, frame, colour and lift) at its place,
/// z 0, drawn at x, 5.0 further right and then (frame 1) 2.5 right of the first.
pub fn plate_draw(cpu: &mut Cpu, s: u32) {
    let o = s.wrapping_sub(0x10);
    let d = cpu.stack();
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, d));
    let v = cpu.read8(s + S_KIND);
    cpu.write8(d + S_KIND, v);
    let v = cpu.read8(s + S_FRAME);
    cpu.write8(d + S_FRAME, v);
    let v = cpu.read16(s + S_COLOUR);
    cpu.write16(d + S_COLOUR, v);
    let v = cpu.read8(s + S_LIFT);
    cpu.write8(d + S_LIFT, v);
    let x = cpu.read32(o + O_POSITION);
    cpu.write32(d + S_POSITION, x);
    let y = cpu.read32(o + O_POSITION + 4);
    cpu.write32(d + S_POSITION + 4, y);
    cpu.write32(d + S_POSITION + 8, 0);
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
    let x = cpu.read32(d + S_POSITION).wrapping_add(0x50_0000);
    cpu.write32(d + S_POSITION, x);
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
    cpu.write8(d + S_FRAME, 1);
    let x = cpu.read32(d + S_POSITION).wrapping_add(0xffd8_0000);
    cpu.write32(d + S_POSITION, x);
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
}

/// A plate (the scenery's, 0x0608b6fc) made from the description `d` (place, the kind in the type's
/// low byte): an object of list 2, 0xbc bytes, kind 11, power 10000, flags 0xc60000, step
/// 0x0608b518, draw 0x0608b664, the kind's colour, at the description's place, z 0 and lift -8. Gives it
/// (0 with no number or room).
pub fn plate_make(cpu: &mut Cpu, d: u32) -> u32 {
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, 2)) as i8;
    if n == -1 {
        return 0;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, SC_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, PART_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + 0x3c));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + O_BEHAVIOUR, SC_PLATE_STEP);
    cpu.write32(o + O_FLAGS, 0xc6_0000);
    cpu.write32(o + O_DRAW, SC_PLATE_DRAW);
    cpu.write8(o + O_NUMBER, n as u8);
    let number = cpu.read8(o + O_NUMBER) as i8 as i32 as u32;
    cpu.write32(NUMBERED.wrapping_add(number << 2), o);
    cpu.write8(o + O_KIND, 11);
    cpu.write8(o + O_FRAME, 0);
    let colour = cpu.read16(kind(11) + K_COLOUR);
    cpu.write16(o + O_COLOUR_B, colour);
    cpu.write16(o + O_COLOUR_A, colour);
    cpu.write32(o + P_POWER, 10000);
    cpu.write32(o + O_SCRIPT, 0x060a_c7e8);
    cpu.write8(o + O_54, 3);
    cpu.write8(o + O_55, 4);
    let x = cpu.read32(d + G_PLACE);
    cpu.write32(o + O_POSITION, x);
    let y = cpu.read32(d + G_PLACE + 4);
    cpu.write32(o + O_POSITION + 4, y);
    cpu.write32(o + O_POSITION + 8, 0);
    cpu.write8(o + O_LIFT_AT, 0xf8);
    let kind_byte = cpu.read8(d + G_TYPE + 1);
    cpu.write8(o + SC_KIND, kind_byte);
    o
}
const O_LIFT_AT: u32 = 0x33;

/// A round's step (behaviour 0x0608b82c, drawn by the slider's draw): past y 224.0 done and its place
/// in the scenery's objects cleared; with power left its flicker (slider_flicker), then kind 2
/// slides down by 0x7000, the others bounce (bounce_step) over the floor (kind 1 the last height,
/// 0 the last two); with the power gone the height of its kind (SCENERY) rises by 0x1a0000, its
/// place is cleared and the objects of the later plates are set moving (0x0608b814: kind 0 the
/// fourth, 1 the first, fourth and fifth, 2 the first, second and the three after), then it bursts.
pub fn round_step(cpu: &mut Cpu, o: u32) {
    if cpu.read32(o + O_POSITION + 4) as i32 > SC_GONE_Y {
        cpu.write8(o + O_MODE, MODE_DONE);
        let kind = cpu.read8(o + SC_KIND) as u32;
        cpu.write32(SC_OBJECTS + 4 * kind, 0);
        return;
    }
    if cpu.read32(o + P_POWER) as i32 > 0 {
        nested!(cpu, 0x0608_994c, slider_flicker(cpu, o));
        let kind_byte = cpu.read8(o + SC_KIND);
        if kind_byte == 2 {
            let y = cpu.read32(o + O_POSITION + 4).wrapping_add(SC_SLIDE);
            cpu.write32(o + O_POSITION + 4, y);
        } else {
            let floor = if kind_byte == 1 {
                cpu.read32(SCENERY + 8)
            } else {
                cpu.read32(SCENERY + 8).wrapping_add(cpu.read32(SCENERY + 4))
            };
            nested!(cpu, 0x0608_b49c, bounce_step(cpu, o, floor));
        }
        return;
    }
    let kind_byte = cpu.read8(o + SC_KIND) as u32;
    let raised = cpu.read32(SCENERY + 4 * kind_byte).wrapping_add(SC_RAISE);
    cpu.write32(SCENERY + 4 * kind_byte, raised);
    cpu.write32(SC_OBJECTS + 4 * kind_byte, 0);
    let moved: &[u32] = match kind_byte {
        0 => &[3],
        1 => &[0, 3, 4],
        2 => &[0, 1, 3, 4, 5],
        _ => &[],
    };
    for &k in moved {
        let object = cpu.read32(SC_OBJECTS + 4 * k);
        cpu.call(SC_SET_MOVE, &[object]);
    }
    nested!(cpu, 0x0608_991c, slider_burst(cpu, o));
}

/// A round (the scenery's, 0x0608b944) made from the description `d`: an object of list 2, 0xbc
/// bytes, kind 73, power 3000, flags 0xc00000, step 0x0608b82c, draw the slider's, colour the kind's
/// plus 16 times the description's type, at the description's place with its z 0; its kind (+0x7c) the
/// description's direction (low byte), its drift (+0x92, +0x94) 6 times the sine and minus 6 times
/// the cosine of 0x2000 on whole units. Gives it (0 with no number or room).
pub fn round_make(cpu: &mut Cpu, d: u32) -> u32 {
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, 2)) as i8;
    if n == -1 {
        return 0;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, SC_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, PART_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + 0x3c));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + O_BEHAVIOUR, SC_ROUND_STEP);
    cpu.write32(o + O_FLAGS, 0xc0_0000);
    cpu.write32(o + O_DRAW, SC_ROUND_DRAW);
    cpu.write8(o + O_KIND, 73);
    cpu.write8(o + O_FRAME, 0);
    let ty = signed_word(cpu, d + G_TYPE);
    let base = cpu.read16(kind(73) + K_COLOUR) as i16 as i32 as u32;
    cpu.write16(o + O_COLOUR_A, base.wrapping_add(ty << 4) as u16);
    cpu.write8(o + O_NUMBER, n as u8);
    let number = cpu.read8(o + O_NUMBER) as i8 as i32 as u32;
    cpu.write32(NUMBERED.wrapping_add(number << 2), o);
    cpu.write32(o + P_POWER, 3000);
    cpu.write8(o + O_54, 1);
    let v = cpu.read8(d + G_TYPE + 1);
    cpu.write8(o + O_55, v);
    cpu.write32(o + O_SCRIPT, 0x060a_c5f4);
    let x = cpu.read32(d + G_PLACE);
    cpu.write32(o + O_POSITION, x);
    let y = cpu.read32(d + G_PLACE + 4);
    cpu.write32(o + O_POSITION + 4, y);
    cpu.write32(o + O_POSITION + 8, 0);
    let v = cpu.read8(d + G_DIRECTION + 1);
    cpu.write8(o + SC_KIND, v);
    let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, 0x2000));
    let drift = sin.wrapping_add(sin).wrapping_add(sin);
    let drift = drift.wrapping_add(drift);
    let drift = drift.wrapping_add(if (drift as i32) < 0 { 0xffff } else { 0 });
    cpu.write16(o + SC_DRIFT, (drift >> 16) as u16);
    let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, 0x2000));
    let drift = (cos as i32).wrapping_mul(-6) as u32;
    let drift = drift.wrapping_add(if (drift as i32) < 0 { 0xffff } else { 0 });
    cpu.write16(o + SC_DRIFT + 2, (drift >> 16) as u16);
    o
}
const SC_DRIFT: u32 = 0x92;

/// A volley (behaviour 0x0608baa8): its clock (+0x8a) counts; in its 121st to 719th step, every 32nd,
/// with the nearer player within 64.0 across and below it, it fires a mount shot (0x0608c04c).
pub fn volley_down_step(cpu: &mut Cpu, o: u32) {
    let clock = cpu.read16(o + SC_CLOCK).wrapping_add(1);
    cpu.write16(o + SC_CLOCK, clock);
    if clock.wrapping_sub(121) > VOLLEY_SPAN {
        return;
    }
    let p = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + O_FLAGS));
    let gap = cpu.read32(o + O_POSITION).wrapping_sub(cpu.read32(p + O_POSITION)) as i32;
    if gap.wrapping_abs() > 0x3f_ffff || cpu.read16(o + SC_CLOCK) & 31 != 0 {
        return;
    }
    if (cpu.read32(o + O_POSITION + 4) as i32) < cpu.read32(p + O_POSITION + 4) as i32 {
        nested!(cpu, 0x0608_c04c, mount_shot(cpu, o));
    }
}
const SC_CLOCK: u32 = 0x8a;
const VOLLEY_SPAN: u16 = 0x256;

/// The other volley (0x0608bb18): the same without the width; it fires when the player is above it.
pub fn volley_up_step(cpu: &mut Cpu, o: u32) {
    let clock = cpu.read16(o + SC_CLOCK).wrapping_add(1);
    cpu.write16(o + SC_CLOCK, clock);
    if clock.wrapping_sub(121) > VOLLEY_SPAN {
        return;
    }
    let p = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + O_FLAGS));
    if cpu.read16(o + SC_CLOCK) & 31 != 0 {
        return;
    }
    if cpu.read32(o + O_POSITION + 4) as i32 > cpu.read32(p + O_POSITION + 4) as i32 {
        nested!(cpu, 0x0608_c04c, mount_shot(cpu, o));
    }
}

/// A mount's fields: its parent +0x6c, its slot +0x88 (0 to 6: where on the parent it sits, and
/// which routine the table at 0x060ac7f8 runs on it), its angle +0xa8.
const MT_PARENT: u32 = 0x6c;
const MT_SLOT: u32 = 0x88;
const MT_ANGLE: u32 = 0xa8;
const MT_ROUTINES: u32 = 0x060a_c7f8;
const MT_STEP: u32 = 0x0608_bb74;
const MT_DRAW: u32 = 0x0608_c290;
/// Where a mount sits against its parent by slot (x, y).
const MT_PLACE: [(u32, u32); 7] = [
    (0x6c_0000, 0x33_0000),
    (0xffe2_0000, 0x33_0000),
    (0, 0x33_0000),
    (0, 0xffcd_0000),
    (0x24_0000, 0xffcd_0000),
    (0x48_0000, 0xffcd_0000),
    (0x6c_0000, 0xffcd_0000),
];

/// A mount's step (behaviour 0x0608bb74): past y 144.0 done; with power left and the parent not yet
/// done (mode 0x70) it goes on; with the power gone it scores 10 and blows up; either way then done.
/// Going on: a hit bit (0x200) sets its flag 1 (+0x7e); the routine of its slot (the table 0x060ac7f8)
/// runs on it; it sits at its slot's place against the parent; its angle (+0xa8) turns by the
/// difference to the angle to its target (with the delay of 20) and its 0x80000 flag goes.
pub fn mount_step(cpu: &mut Cpu, o: u32) {
    let mut done = cpu.read32(o + O_POSITION + 4) as i32 > 0x90_0000;
    let parent = cpu.read32(o + MT_PARENT);
    if !done {
        let power = cpu.read32(o + P_POWER) as i32;
        if power > 0 {
            done = cpu.read8(parent + O_MODE) as i8 > 111;
        } else {
            nested!(cpu, 0x0605_61ac, hits::score_hit_by(cpu, o, 10));
            nested!(cpu, 0x0605_9e9c, ground::blown_up_1(cpu, o));
            done = true;
        }
    }
    if done {
        cpu.write8(o + O_MODE, MODE_DONE);
        return;
    }
    if cpu.read16(o + FL_HIT_BITS) & 0x200 != 0 {
        let flags = cpu.read16(o + FL_FLAGS) | 1;
        cpu.write16(o + FL_FLAGS, flags);
    }
    let slot = cpu.read8(o + MT_SLOT) as u32;
    let routine = cpu.read32(MT_ROUTINES + 4 * slot);
    cpu.call_returning(0x0608_bbec, routine, &[o]);
    let slot = cpu.read8(o + MT_SLOT) as usize;
    if let Some(&(dx, dy)) = MT_PLACE.get(slot) {
        let x = cpu.read32(parent + O_POSITION).wrapping_add(dx);
        cpu.write32(o + O_POSITION, x);
        let y = cpu.read32(parent + O_POSITION + 4).wrapping_add(dy);
        cpu.write32(o + O_POSITION + 4, y);
    }
    let aim_at = nested!(cpu, 0x0605_6ad4, aim::angle_to_target(cpu, o, 20)) & 0xffff;
    let from = signed_word(cpu, o + MT_ANGLE);
    let diff = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, from, aim_at));
    let angle = cpu.read16(o + MT_ANGLE).wrapping_add(diff as u16);
    cpu.write16(o + MT_ANGLE, angle);
    let flags = cpu.read32(o + O_FLAGS) & NOT_80000;
    cpu.write32(o + O_FLAGS, flags);
}

/// A mount (0x0608bcd0) made on `parent`, of `colour` (the kind's colour plus 16 times it), turned
/// `turn` and at `slot`: an object of list 2, 0xbc bytes, kind 74, power 2000, flags 0xc80000, step
/// 0x0608bb74, draw 0x0608c290, at x 0, y -512.0, z 0.
pub fn mount_make(cpu: &mut Cpu, parent: u32, colour: u32, turn: u32, slot: u32) {
    let (colour, turn, slot) = (colour & 0xff, turn & 0xffff, slot & 0xff);
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, 2)) as i8;
    if n == -1 {
        return;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, SC_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, PART_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + 0x3c));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + O_BEHAVIOUR, MT_STEP);
    cpu.write32(o + O_FLAGS, 0xc8_0000);
    cpu.write32(o + O_DRAW, MT_DRAW);
    cpu.write8(o + O_KIND, 74);
    cpu.write8(o + O_FRAME, 0);
    let base = cpu.read16(kind(73) + K_COLOUR) as i16 as i32 as u32;
    cpu.write16(o + O_COLOUR_A, base.wrapping_add(colour << 4) as u16);
    cpu.write8(o + O_NUMBER, n as u8);
    let number = cpu.read8(o + O_NUMBER) as i8 as i32 as u32;
    cpu.write32(NUMBERED.wrapping_add(number << 2), o);
    cpu.write32(o + P_POWER, 2000);
    cpu.write8(o + O_54, 1);
    cpu.write8(o + O_55, colour as u8);
    cpu.write32(o + O_SCRIPT, 0x060a_c88c);
    cpu.write32(o + MT_PARENT, parent);
    cpu.write8(o + MT_SLOT, slot as u8);
    cpu.write32(o + O_POSITION, 0);
    cpu.write32(o + O_POSITION + 4, 0xfe00_0000);
    cpu.write32(o + O_POSITION + 8, 0);
    cpu.write16(o + O_ANGLE, turn as u16);
}

/// A mount's shot (0x0608c04c): a rocket (0x06055b5c) from 10 units along its angle (+0xa8) from the
/// mount (x by the sine, y by minus the cosine), the angle given as the shot's (+0x2c), the sprite
/// word +0x30 48 and +0x2e 8.
pub fn mount_shot(cpu: &mut Cpu, o: u32) {
    let f = cpu.stack();
    cpu.write16(f + 0x30, 48);
    let n = cpu.read8(o + O_NUMBER);
    cpu.write8(f + 0x1e, n);
    cpu.write32(f + 8, 0);
    let angle = cpu.read16(o + MT_ANGLE);
    cpu.write16(f + 0x2c, angle);
    let a = cpu.read16(f + 0x2c) as i16 as i32 as u32;
    let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a)) as i32;
    let x = cpu.read32(o + O_POSITION).wrapping_add(sin.wrapping_mul(10) as u32);
    cpu.write32(f, x);
    let a = cpu.read16(f + 0x2c) as i16 as i32 as u32;
    let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a)) as i32;
    let y = cpu.read32(o + O_POSITION + 4).wrapping_sub(cos.wrapping_mul(10) as u32);
    cpu.write32(f + 4, y);
    cpu.write16(f + 0x2e, 8);
    nested!(cpu, 0x0605_5b5c, bullets::rocket_set(cpu, f));
}

/// A proxy's step (behaviour 0x0608bdf0): it follows a part's flags (the pointer at +0x6c), gone when
/// they say 0x40 (flag 0x80000 for 0x20 or 0x80), else takes the place at the pointer +0x74; while that
/// part's 0x10 flag is on its own flag 0x40000; else a hit bit sets the part's flag 1 and the health
/// at the pointer +0x70 takes its power less 1.0; the last kill is scored (10000) for the one
/// at +0x40; then its power is 1.0 again.
pub fn proxy_step(cpu: &mut Cpu, o: u32) {
    let at = cpu.read32(o + PX_FLAGS);
    let w = cpu.read16(at);
    if w & 0xe0 != 0 {
        if w & 0x40 != 0 {
            cpu.write8(o + O_MODE, MODE_DONE);
        } else {
            let flags = cpu.read32(o + O_FLAGS) | 0x8_0000;
            cpu.write32(o + O_FLAGS, flags);
        }
        return;
    }
    let place = cpu.read32(o + PX_PLACE);
    let x = cpu.read32(place);
    cpu.write32(o + O_POSITION, x);
    let place = cpu.read32(o + PX_PLACE);
    let y = cpu.read32(place + 4);
    cpu.write32(o + O_POSITION + 4, y);
    let at = cpu.read32(o + PX_FLAGS);
    if cpu.read16(at) & 0x10 != 0 {
        let flags = cpu.read32(o + O_FLAGS) | 0x4_0000;
        cpu.write32(o + O_FLAGS, flags);
    } else {
        let flags = cpu.read32(o + O_FLAGS) & 0xfffb_ffff;
        cpu.write32(o + O_FLAGS, flags);
        if cpu.read16(o + FL_HIT_BITS) & 0x200 != 0 {
            let at = cpu.read32(o + PX_FLAGS);
            let w = cpu.read16(at) | 1;
            cpu.write16(at, w);
        }
        let health = cpu.read32(o + PX_HEALTH);
        let mut life = cpu.read32(health);
        if life as i32 > 0 {
            life = life.wrapping_add(0xffff_0000).wrapping_add(cpu.read32(o + P_POWER));
            cpu.write32(health, life);
            let health = cpu.read32(o + PX_HEALTH);
            if cpu.read32(health) as i32 <= 0 {
                let who = cpu.read32(o + PX_WHO);
                let p = cpu.read8(who + O_NUMBER) as i8 as i32 as u32;
                let weapon = cpu.read8(who + O_55) as i8 as i32 as u32;
                nested!(cpu, 0x0604_3108, hits::score_kill(cpu, p, weapon, o, 10000));
            }
        }
    }
    let flags = cpu.read32(o + O_FLAGS) & NOT_80000;
    cpu.write32(o + O_FLAGS, flags);
    cpu.write32(o + P_POWER, 0x1_0000);
}
const PX_FLAGS: u32 = 0x6c;
const PX_HEALTH: u32 = 0x70;
const PX_PLACE: u32 = 0x74;
const PX_WHO: u32 = 0x40;

/// The proxy's maker (0x0608bed8): an object of list 2, 120 bytes, run by 0x0604d780, step 0x0608bdf0,
/// flags 0x80080000, power 1.0, script `script` (its argument), the pointers left to its user. Gives
/// it (0 with no room).
pub fn proxy_make(cpu: &mut Cpu, script: u32) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 120)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, 0x0604_d780));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + 0x3c));
    cpu.write32(o + O_BEHAVIOUR, 0x0608_bdf0);
    cpu.write32(o + O_FLAGS, 0x8008_0000);
    cpu.write32(o + P_POWER, 0x1_0000);
    cpu.write8(o + O_54, 1);
    cpu.write32(o + O_SCRIPT, script);
    cpu.write8(o + O_55, 0);
    o
}

/// A tracker's step (behaviour 0x0608bf60): gone when the flags (the pointer at +0x6c) say 0x40
/// (nothing for 0x80); else it sits at the place (the pointer at +0x70) with its 0x80000 flag off.
pub fn tracker_step(cpu: &mut Cpu, o: u32) {
    let at = cpu.read32(o + PX_FLAGS);
    let w = cpu.read16(at);
    if w & 0xc0 != 0 {
        if w & 0x40 != 0 {
            cpu.write8(o + O_MODE, MODE_DONE);
        }
        return;
    }
    let place = cpu.read32(o + PX_HEALTH);
    let x = cpu.read32(place);
    cpu.write32(o + O_POSITION, x);
    let y = cpu.read32(place + 4);
    cpu.write32(o + O_POSITION + 4, y);
    let flags = cpu.read32(o + O_FLAGS) & NOT_80000;
    cpu.write32(o + O_FLAGS, flags);
}

/// The tracker's maker (0x0608bfa4): a numbered object of list 2, 116 bytes, step 0x0608bf60, flags
/// 0x80080000, z 0, power 10, script 0x060ac884. Gives it (0 with no number or room).
pub fn tracker_make(cpu: &mut Cpu) -> u32 {
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, 2)) as i8;
    if n == -1 {
        return 0;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 116)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, PART_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + 0x3c));
    cpu.write32(o + O_BEHAVIOUR, 0x0608_bf60);
    cpu.write32(o + O_FLAGS, 0x8008_0000);
    cpu.write32(o + O_POSITION + 8, 0);
    cpu.write8(o + O_NUMBER, n as u8);
    let number = cpu.read8(o + O_NUMBER) as i8 as i32 as u32;
    cpu.write32(NUMBERED.wrapping_add(number << 2), o);
    cpu.write8(o + O_54, 1);
    cpu.write32(o + O_SCRIPT, 0x060a_c884);
    cpu.write32(o + P_POWER, 10);
    o
}


const SR_CLOCK_FROM: i32 = 0x1e0;
const SR_SPEED: u32 = 0x1_4000;
const SR_LEFT: u32 = 0xfffe_c000;
const SR_VOLLEY_AFTER: i32 = 120;
const SR_ANCHOR: u32 = 0xac;
const SR_ROUTINES: u32 = 0x060a_c89c;
const SR_APPROACH: u32 = 0x1200;
const SR_SPEED_Y: u32 = 0x84;

/// A strafing mount's route (the routine 0x0608c0d0 gives its speed for): 0x14000 once its phase
/// (+0x7c) has gone past 0; else its clock (+0x8a) counts (the phase goes on after 480 steps), the
/// speed -0x14000 while it is further down than the nearer player is below the anchor (+0xac), and
/// after 120 steps a shot (mount_shot) every 32nd and a stop (0) for the last 15 of each 32.
pub fn strafe_route(cpu: &mut Cpu, o: u32) -> u32 {
    let mut speed = SR_SPEED;
    if cpu.read8(o + SC_KIND) != 0 {
        return speed;
    }
    let clock = cpu.read16(o + SC_CLOCK).wrapping_add(1);
    cpu.write16(o + SC_CLOCK, clock);
    if clock as i16 as i32 > SR_CLOCK_FROM {
        let phase = cpu.read8(o + SC_KIND).wrapping_add(1);
        cpu.write8(o + SC_KIND, phase);
    }
    let p = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + O_FLAGS));
    let below = cpu.read32(o + O_POSITION + 4).wrapping_sub(cpu.read32(o + SR_ANCHOR)) as i32;
    if below > cpu.read32(p + O_POSITION + 4) as i32 {
        speed = SR_LEFT;
    }
    let clock = cpu.read16(o + SC_CLOCK);
    if clock as i16 as i32 > SR_VOLLEY_AFTER {
        let step = clock & 31;
        if step == 0 {
            nested!(cpu, 0x0608_c04c, mount_shot(cpu, o));
        } else if step > 16 {
            speed = 0;
        }
    }
    speed
}

/// A strafing mount's step (behaviour 0x0608c170): like the mount's (done past y 144.0; power gone,
/// or the parent done or out of power: scores 10 for the power and blows up, then done); a hit bit
/// sets its flag 1; the routine of its slot (the table 0x060ac89c) gives a speed that its y speed
/// (+0x84) approaches by 0x1200 and its y takes; its frame is bit 1 of its y over 4.0 on whole units
/// and its angle (+0xa8) turns toward its target as the mount's.
pub fn strafe_step(cpu: &mut Cpu, o: u32) {
    let mut done = cpu.read32(o + O_POSITION + 4) as i32 > 0x90_0000;
    if !done {
        let parent = cpu.read32(o + MT_PARENT);
        let mut dead = true;
        if cpu.read32(o + P_POWER) as i32 > 0 && cpu.read8(parent + O_MODE) as i8 <= 111 && cpu.read32(parent + P_POWER) as i32 > 0 {
            dead = false;
        }
        if dead {
            if cpu.read32(o + P_POWER) as i32 <= 0 {
                nested!(cpu, 0x0605_61ac, hits::score_hit_by(cpu, o, 10));
            }
            nested!(cpu, 0x0605_9e9c, ground::blown_up_1(cpu, o));
        }
        done = dead;
    }
    if done {
        cpu.write8(o + O_MODE, MODE_DONE);
        return;
    }
    if cpu.read16(o + FL_HIT_BITS) & 0x200 != 0 {
        let flags = cpu.read16(o + FL_FLAGS) | 1;
        cpu.write16(o + FL_FLAGS, flags);
    }
    let slot = cpu.read8(o + MT_SLOT) as u32;
    let routine = cpu.read32(SR_ROUTINES + 4 * slot);
    let f = cpu.stack();
    cpu.write32(f, routine);
    let to = cpu.call_returning(0x0608_c1fe, routine, &[o]);
    let from = cpu.read32(o + SR_SPEED_Y);
    let speed = nested!(cpu, 0x0604_6168, actors::approach(cpu, from, to, SR_APPROACH));
    cpu.write32(o + SR_SPEED_Y, speed);
    let y = cpu.read32(o + O_POSITION + 4).wrapping_add(speed);
    cpu.write32(o + O_POSITION + 4, y);
    let t = y.wrapping_add(if (y as i32) < 0 { 0x3_ffff } else { 0 });
    let frame = (((t >> 16) as i16 as i32) >> 2) & 1;
    cpu.write8(o + O_FRAME, frame as u8);
    let aim_at = nested!(cpu, 0x0605_6ad4, aim::angle_to_target(cpu, o, 20)) & 0xffff;
    let from = signed_word(cpu, o + MT_ANGLE);
    let diff = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, from, aim_at));
    let angle = cpu.read16(o + MT_ANGLE).wrapping_add(diff as u16);
    cpu.write16(o + MT_ANGLE, angle);
}

/// A mount's draw (0x0608c290): the sprite (the object's kind, lift -8, frame and turn (+0x30), its
/// place; the colour of the first kind plus 0xe0 while flag 1 is on, else the sprite's) and again as
/// frame 2 turned by its angle (+0xa8); flag 2 goes and is set with flag 1 as the flap hatch's.
pub fn mount_draw(cpu: &mut Cpu, s: u32) {
    let o = s.wrapping_sub(0x10);
    let d = cpu.stack();
    let at = o + FL_FLAGS;
    let w = cpu.read16(at);
    if w & 2 != 0 {
        cpu.write16(at, w & 0xfffc);
    }
    let w = cpu.read16(at);
    if w & 1 != 0 {
        cpu.write16(at, w | 2);
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, d));
    let v = cpu.read8(s + S_KIND);
    cpu.write8(d + S_KIND, v);
    cpu.write8(d + S_LIFT, 0xf8);
    let z = cpu.read32(o + O_POSITION + 8);
    cpu.write32(d + S_POSITION + 8, z);
    if cpu.read16(at) & 1 != 0 {
        let base = cpu.read16(kind(0) + K_COLOUR);
        cpu.write16(d + S_COLOUR, base.wrapping_add(0xe0));
    } else {
        let colour = cpu.read16(s + S_COLOUR);
        cpu.write16(d + S_COLOUR, colour);
    }
    let frame = cpu.read8(o + O_FRAME);
    cpu.write8(d + S_FRAME, frame);
    let x = cpu.read32(o + O_POSITION);
    cpu.write32(d + S_POSITION, x);
    let y = cpu.read32(o + O_POSITION + 4);
    cpu.write32(d + S_POSITION + 4, y);
    let turn = cpu.read16(o + O_ANGLE);
    cpu.write16(d + S_TURN, turn);
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
    cpu.write8(d + S_FRAME, 2);
    let turn = cpu.read16(o + MT_ANGLE);
    cpu.write16(d + S_TURN, turn);
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
}

// ---- the strafers' walker ------------------------------------------------------------------------

/// A strafing mount (0x0608c360) made on `parent`: an object of list 2, 0xbc bytes, kind 74 (draw
/// 0x0608c290, step 0x0608c170), the colour of the mount kind plus 16 times `colour`, turned
/// `turn`, at (`x`, `y`) in whole units, flags 0xc00000, power 2000, `side` at +0x88 and `anchor`
/// at +0xac, z 0.
#[allow(clippy::too_many_arguments)]
pub fn strafe_make(cpu: &mut Cpu, parent: u32, colour: u32, turn: u32, x: u32, y: u32, side: u32, anchor: u32) {
    let (colour, turn, side) = (colour & 0xff, turn & 0xffff, side & 0xff);
    let f = cpu.stack();
    cpu.write32(f, parent);
    cpu.write32(f + 4, turn);
    let y = y as i16 as i32 as u32;
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, 2)) as i8;
    cpu.write32(f + 8, side);
    if n == -1 {
        return;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, SC_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, PART_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + 0x3c));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + O_BEHAVIOUR, 0x0608_c170);
    cpu.write32(o + O_FLAGS, 0xc0_0000);
    cpu.write32(o + O_DRAW, 0x0608_c290);
    cpu.write8(o + O_KIND, 74);
    cpu.write8(o + O_FRAME, 0);
    let base = cpu.read16(kind(74) + K_COLOUR);
    cpu.write16(o + O_COLOUR_A, base.wrapping_add((colour << 4) as u16));
    cpu.write8(o + O_NUMBER, n as u8);
    let number = cpu.read8(o + O_NUMBER) as i8 as i32 as u32;
    cpu.write32(NUMBERED.wrapping_add(number << 2), o);
    cpu.write32(o + P_POWER, 2000);
    cpu.write8(o + O_54, 1);
    cpu.write8(o + O_55, colour as u8);
    cpu.write32(o + O_SCRIPT, 0x060a_c88c);
    cpu.write32(o + MT_PARENT, parent);
    cpu.write8(o + MT_SLOT, side as u8);
    cpu.write32(o + O_POSITION, (x as i16 as i32 as u32) << 16);
    cpu.write32(o + O_POSITION + 4, y << 16);
    cpu.write32(o + O_POSITION + 8, 0);
    cpu.write32(o + SR_ANCHOR, anchor);
    cpu.write16(o + O_ANGLE, turn as u16);
}

/// What the walker keeps (the boss of stage 4's second half): its mode (+0x150: 0-3 fighting, 4
/// done), its flags (+0x152: 0x40 sinks it), its hit and trigger bits (+0x94: 1 hit, 2 beaten, 4
/// and 8 a sound), its clock (+0x1b8), its stage (+0x1bc) and count (+0x1bd), the angle to the
/// player (+0x1a6), the place it shoots from (+0x154, +0x158), its sink speed (+0x1b0), its
/// joints' targets, turns, turn speeds and angles (+0x98, +0xd8, +0xf8, +0x118: 16 each).
const WK_MODE: u32 = 0x150;
const WK_FLAGS: u32 = 0x152;
const WK_BITS: u32 = 0x94;
const WK_CLOCK: u32 = 0x1b8;
const WK_STAGE: u32 = 0x1bc;
const WK_COUNT: u32 = 0x1bd;
const WK_ANGLE: u32 = 0x1a6;
const WK_AT: u32 = 0x154;
const WK_SINK: u32 = 0x1b0;
const WK_TURN: u32 = 0x1a0;
const WK_TURN_2: u32 = 0x1a2;
const WK_TURN_3: u32 = 0x1a4;
const WK_SPRING: u32 = 0x1a8;
const WK_FLAGS_2: u32 = 0x19c;
const WK_GAZE: u32 = 0x1ae;
const WK_DIFFICULTY: u32 = 0x0600_ed84;
const WK_TARGETS: u32 = 0x98;
const WK_ANGLES: u32 = 0xd8;
const WK_STEPS: u32 = 0xf8;
const WK_SPEEDS: u32 = 0x118;
const WK_STATES: u32 = 0x060a_cb80;
const WK_SCRIPTS: u32 = 0x060a_cb54;
const WK_FLASH_ANGLES: u32 = 0x060a_cb60;
const WK_FLASH_SCRIPTS: u32 = 0x060a_cb6c;

static WK_TURN_PROLOGUE: std::sync::OnceLock<super::Prologue> = std::sync::OnceLock::new();

/// A gun object put out from the walker (0x0608c4a8): a struct at the walker's shooting place
/// (+0x154, +0x158), the object's number (+0x1e of it a `side` of its own at +0x14), the turn
/// 0x8000 less than the walker's (+0x1a0) plus `dx`, `size` at +0x2c and a flag of 15; then a
/// hatch (kind 21) or a ground one (the kind at +0x1a) set from the script of `which`.
pub fn walker_gun(cpu: &mut Cpu, w: u32, kind: u32, which: u32, size: u32, dx: u32, side: u32) {
    let (kind, which, size) = (kind & 0xff, which & 0xff, size & 0xff);
    let side = side as u8 as i8 as i32 as u32;
    let dx = dx as i16 as i32 as u32;
    let d = cpu.stack();
    cpu.write32(d + 64, side);
    let at = cpu.read32(w + WK_AT);
    cpu.write32(d, at);
    let at = cpu.read32(w + WK_AT + 4);
    cpu.write32(d + 4, at);
    cpu.write32(d + 8, 0);
    cpu.write16(d + 44, size as u16);
    let turn = (cpu.read16(w + WK_TURN) as u32).wrapping_add(dx).wrapping_sub(0x8000);
    cpu.write16(d + 46, turn as u16);
    cpu.write16(d + 48, 15);
    cpu.write32(d + 20, side);
    let script = cpu.read32(WK_SCRIPTS + 4 * which);
    if kind == 21 {
        nested!(cpu, 0x0605_e030, ground::hatch_set(cpu, d, script));
    } else {
        cpu.write16(d + 26, kind as u16);
        nested!(cpu, 0x0605_a780, ground::ground_set(cpu, d, script));
    }
}

/// The walker's aim (0x0608c55c): its first turn (+0x1a0) sprung (aim::turn_toward) toward the
/// angle at +0x1a6 less 0x8000, the speed at +0x1a4, and moved by the turn (+0x1a2).
pub fn walker_aim(cpu: &mut Cpu, w: u32) {
    let to = (signed_word(cpu, w + WK_ANGLE).wrapping_add(0xffff_8000)) & 0xffff;
    let from = cpu.read16(w + WK_TURN) as u32;
    cpu.nested_with_stack(&WK_TURN_PROLOGUE, 0x0605_70d8, &[0x8000, 10, 64], |cpu| {
        aim::turn_toward(cpu, w + WK_TURN_2, w + WK_TURN_3, from, to, 0x8000, 10, 64)
    });
    let v = cpu.read16(w + WK_TURN).wrapping_add(cpu.read16(w + WK_TURN_2));
    cpu.write16(w + WK_TURN, v);
}

/// The walker's second turn (0x0608c5c0, +0x1a8 toward +0x1ae by the turn +0x1aa and speed
/// +0x1ac): once it passes 0 it is put to 0 and both its turn and its speed halved, reversed.
pub fn walker_aim_spring(cpu: &mut Cpu, w: u32) {
    let from = cpu.read16(w + WK_SPRING) as u32;
    let to = cpu.read16(w + WK_GAZE) as u32;
    cpu.nested_with_stack(&WK_TURN_PROLOGUE, 0x0605_70d8, &[0x8000, 10, 64], |cpu| {
        aim::turn_toward(cpu, w + 0x1aa, w + 0x1ac, from, to, 0x8000, 10, 64)
    });
    let v = cpu.read16(w + WK_SPRING).wrapping_add(cpu.read16(w + 0x1aa));
    cpu.write16(w + WK_SPRING, v);
    if v as i16 > 0 {
        cpu.write16(w + WK_SPRING, 0);
        for at in [w + 0x1aa, w + 0x1ac] {
            let v = -(signed_word(cpu, at) as i32);
            cpu.write16(at, (v / 2) as u16);
        }
    }
}

/// A flash from the walker (0x0608c64c, `dx` turned from its angle +0x1a0, the sign picks the
/// side): a struct at its shooting place with the object's number, the turn (+0x2c), a table
/// angle by the difficulty (0x060acb60) and script (0x060acb6c), 0x2000; a ribbon (kind 1 for
/// +-0x5000, else 0) and a flare from it.
pub fn walker_flash(cpu: &mut Cpu, w: u32, dx: u32) {
    let dx = dx as i16 as i32 as u32;
    let d = cpu.stack();
    let n = cpu.read8(w + O_NUMBER);
    cpu.write8(d + 30, n);
    let at = cpu.read32(w + WK_AT);
    cpu.write32(d, at);
    let at = cpu.read32(w + WK_AT + 4);
    cpu.write32(d + 4, at);
    cpu.write32(d + 8, 0);
    let turn = (cpu.read16(w + WK_TURN) as u32).wrapping_add(dx).wrapping_add(0xffff_8000);
    cpu.write16(d + 44, turn as u16);
    let level = cpu.read8(WK_DIFFICULTY) as i8 as i32 as u32;
    let v = cpu.read16(WK_FLASH_ANGLES.wrapping_add(level << 1));
    cpu.write16(d + 46, v);
    cpu.write16(d + 56, 0x2000);
    let v = cpu.read32(WK_FLASH_SCRIPTS.wrapping_add(level << 2));
    cpu.write32(d + 48, v);
    cpu.write32(d + 60, 0);
    let size = (dx as i32).unsigned_abs();
    let (ribbon, add) = if size == 0x5000 { (1, 32) } else { (0, 64) };
    let base = cpu.read16(CHAR_BASE);
    cpu.write16(d + 32, base.wrapping_add(add));
    nested!(cpu, 0x0606_42e0, super::ribbons::ribbon_set(cpu, ribbon, d, 0));
    nested!(cpu, 0x0606_43ec, effects::flare_set(cpu, d));
}

/// The walker's fall (0x0608c710): its y (+0x18) down by 0x7000; a hit bit (+0x94's 1) plays sound
/// 60; past y 176.0 it is done (mode 0x70) and its two flags (+0x19c, +0x19e) are 0x40.
pub fn walker_fall(cpu: &mut Cpu, w: u32) {
    let y = cpu.read32(w + 24).wrapping_add(0x7000);
    cpu.write32(w + 24, y);
    if cpu.read16(w + WK_BITS) & 1 != 0 {
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 60));
    }
    if cpu.read32(w + 24) as i32 > 0xb0_0000 {
        cpu.write8(w + O_MODE, MODE_DONE);
        cpu.write16(w + WK_FLAGS_2, 0x40);
        cpu.write16(w + 0x19e, 0x40);
    }
}

/// The walker's rise (0x0608c768): at the first count of its stage (+0x1b8's low byte 0) two
/// mounts of the strafe kind (0x0608c360) at x +-114 with the anchor from the clock's 0x300 bits;
/// the first hit bit lets the 0x40 flag go; the second (+4, with +8 for the side) plays a sound
/// 60; when beaten (+0x94's 2) it starts a run: below y -32.0 it turns to mode 2, its clock 0 and a
/// script (0x060acb18) at +0x6c, else it sinks (flag 0x40, sink speed 0); and a sinking one's y
/// moves by its sink speed.
pub fn walker_rise(cpu: &mut Cpu, w: u32) {
    let clock = cpu.read32(w + WK_CLOCK);
    if clock & 0xff == 0 {
        let anchor = (clock & 0x300).wrapping_sub(0x400).wrapping_mul(0x1000);
        cpu.push(&[0x90, 0, anchor]);
        nested!(cpu, 0x0608_c360, strafe_make(cpu, w, 0, 0xc000, 114, 0x90, 0, anchor));
        let clock = cpu.read32(w + WK_CLOCK);
        let anchor = (clock & 0x300).wrapping_sub(0x400).wrapping_mul(0x1000);
        cpu.push(&[0x90, 0, anchor]);
        nested!(cpu, 0x0608_c360, strafe_make(cpu, w, 0, 0x4000, (-114i32) as u32, 0x90, 0, anchor));
        cpu.pop(24);
    }
    if cpu.read16(w + WK_BITS) & 1 != 0 {
        let v = cpu.read16(w + WK_FLAGS) & 0xffbf;
        cpu.write16(w + WK_FLAGS, v);
    }
    let bits = cpu.read16(w + WK_BITS);
    if bits & 4 != 0 {
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 60));
    }
    if cpu.read16(w + WK_BITS) & 2 != 0 {
        if (cpu.read32(w + 24) as i32) < -0x20_0000 {
            cpu.write8(w + WK_MODE, 2);
            cpu.write32(w + WK_CLOCK, 0);
            nested!(cpu, 0x0605_6628, actors::script_start(cpu, w + 0x6c, 0x060a_cb18));
            cpu.write16(w + WK_ANGLE, 0x8000);
        } else {
            let v = cpu.read16(w + WK_FLAGS) | 0x40;
            cpu.write16(w + WK_FLAGS, v);
            cpu.write32(w + WK_SINK, 0);
        }
    }
    if cpu.read16(w + WK_FLAGS) & 0x40 != 0 {
        let y = cpu.read32(w + 24).wrapping_sub(cpu.read32(w + WK_SINK));
        cpu.write32(w + 24, y);
    }
}

/// A rocket from the walker (0x0608c8ac, `dx` added to the turn): a struct at its shooting place
/// with the object's number, the turn (+0x2c, the walker's +0x1a0 less 0x8000 plus `dx`), 8 and
/// 80, handed to the rocket maker (bullets::rocket_set).
pub fn walker_rocket(cpu: &mut Cpu, w: u32, dx: u32) {
    let dx = dx as i16 as i32 as u32;
    let d = cpu.stack();
    cpu.write16(d + 48, 80);
    let n = cpu.read8(w + O_NUMBER);
    cpu.write8(d + 30, n);
    let at = cpu.read32(w + WK_AT);
    cpu.write32(d, at);
    let at = cpu.read32(w + WK_AT + 4);
    cpu.write32(d + 4, at);
    cpu.write32(d + 8, 0);
    let turn = (cpu.read16(w + WK_TURN) as u32).wrapping_add(dx).wrapping_add(0xffff_8000);
    cpu.write16(d + 44, turn as u16);
    cpu.write16(d + 46, 8);
    nested!(cpu, 0x0605_5b5c, bullets::rocket_set(cpu, d));
}

/// A halfword constant as the SH-2 code loads it (`mov.w`: sign-extended).
fn sw(v: u16) -> u32 {
    v as i16 as i32 as u32
}

/// One gun put out with the arguments pushed as the SH-2 code leaves them (`dx` and `side` stay on
/// the stack until the caller takes them off).
fn walker_put(cpu: &mut Cpu, w: u32, kind: u32, which: u32, size: u32, dx: u32, side: u32) {
    cpu.push(&[dx, side]);
    nested!(cpu, 0x0608_c4a8, walker_gun(cpu, w, kind, which, size, dx, side));
}

/// The walker's fight (0x0608c914), its stage (+0x1bc 0-2) and count (+0x1bd). A stage ends when
/// the count passes 14, 44 or 84: mode 3, count and clock 0, gaze 0x1000. Past 120 on the clock:
/// stage 0 on every 32nd tick puts out a row of seven guns (the count mod 3 picks the row), stage 1
/// on every 8th aims at the nearer player and puts out one, stage 2 on every 20th aims, then (past
/// the first difficulty, on the 30th count) two strafers and a row of rockets by the count's
/// parity; each time the count goes up.
pub fn walker_fight(cpu: &mut Cpu, w: u32) {
    if cpu.read32(w + WK_CLOCK) == 60 {
        cpu.write16(w + WK_GAZE, 0xd000);
    }
    let stage = cpu.read8(w + WK_STAGE) as i8;
    let ends = match stage {
        0 => cpu.read8(w + WK_COUNT) > 14,
        1 => cpu.read8(w + WK_COUNT) > 44,
        2 => cpu.read8(w + WK_COUNT) > 84,
        _ => false,
    };
    if ends {
        cpu.write8(w + WK_MODE, 3);
        cpu.write8(w + WK_COUNT, 0);
        cpu.write32(w + WK_CLOCK, 0);
        cpu.write16(w + WK_GAZE, 0x1000);
        return;
    }
    let clock = cpu.read32(w + WK_CLOCK) as i32;
    if clock <= 119 {
        return;
    }
    if cpu.read8(w + WK_STAGE) as i8 == 0 && clock & 31 == 0 {
        if clock == 2 {
            cpu.write16(w + WK_ANGLE, 0x8000);
        }
        let count = cpu.read8(w + WK_COUNT) as u32;
        match nested!(cpu, 0x0600_6a6e, math::remainder(cpu, count, 3)) {
            1 => {
                walker_put(cpu, w, 0, 0, 1, sw(0xd000), 1);
                walker_put(cpu, w, 0, 0, 0, sw(0xe000), 1);
                walker_put(cpu, w, 0, 0, 0, sw(0xf000), 1);
                walker_put(cpu, w, 21, 0, 1, 0, 1);
                cpu.pop(32);
                walker_put(cpu, w, 0, 0, 0, 0x1000, 1);
                walker_put(cpu, w, 0, 0, 0, 0x2000, 1);
                walker_put(cpu, w, 0, 0, 1, 0x3000, 1);
            }
            2 => {
                walker_put(cpu, w, 21, 0, 2, sw(0xe000), 1);
                walker_put(cpu, w, 21, 0, 2, sw(0xe800), 1);
                walker_put(cpu, w, 21, 0, 2, sw(0xf800), 1);
                walker_put(cpu, w, 21, 0, 2, 0, 1);
                cpu.pop(32);
                walker_put(cpu, w, 21, 0, 2, 0x0800, 1);
                walker_put(cpu, w, 21, 0, 2, 0x1800, 1);
                walker_put(cpu, w, 21, 0, 2, 0x2000, 1);
            }
            _ => {
                walker_put(cpu, w, 21, 0, 0, sw(0xd000), 1);
                walker_put(cpu, w, 0, 0, 2, sw(0xe000), 1);
                walker_put(cpu, w, 0, 0, 1, sw(0xf000), 1);
                walker_put(cpu, w, 0, 0, 0, 0, 1);
                cpu.pop(32);
                walker_put(cpu, w, 0, 0, 1, 0x1000, 1);
                walker_put(cpu, w, 0, 0, 2, 0x2000, 1);
                walker_put(cpu, w, 21, 0, 0, 0x3000, 1);
            }
        }
        walker_count_up(cpu, w);
        return;
    }
    let stage = cpu.read8(w + WK_STAGE) as i8;
    if stage == 1 {
        let low = cpu.read32(w + WK_CLOCK) & 7;
        if low == 0 {
            walker_look(cpu, w);
            let count = cpu.read8(w + WK_COUNT) as u32;
            let r = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, count, 3));
            if r != 0 {
                walker_put(cpu, w, 0, 1, 0, low, 1);
            } else {
                walker_put(cpu, w, 21, 2, 1, low, 1);
            }
            walker_count_up(cpu, w);
            return;
        }
    }
    if cpu.read8(w + WK_STAGE) as i8 != 2 {
        return;
    }
    let clock = cpu.read32(w + WK_CLOCK);
    if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, clock, 20)) != 0 {
        return;
    }
    walker_look(cpu, w);
    if cpu.read8(WK_DIFFICULTY) != 0 {
        let count = cpu.read8(w + WK_COUNT) as u32;
        if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, count, 30)) == 0 {
            for turn in [0xc000u32, 0x4000] {
                let x = if turn == 0xc000 { 114u32 } else { (-114i32) as u32 };
                let count = cpu.read8(w + WK_COUNT) as u64;
                let colour = (((count * 0x8888_8889) >> 32) >> 4) as u32 & 0xff;
                cpu.push(&[0x90, 0, 0xffe0_0000]);
                nested!(cpu, 0x0608_c360, strafe_make(cpu, w, colour, turn, x, 0x90, 0, 0xffe0_0000));
            }
            cpu.pop(24);
        }
    }
    let rows: [u16; 7] = if cpu.read8(w + WK_COUNT) & 1 != 0 {
        [0xdc00, 0xe800, 0xf400, 0, 0x0c00, 0x1800, 0x2400]
    } else {
        [0xe200, 0xee00, 0xfa00, 0x0600, 0x1200, 0x1e00, 0]
    };
    let n = if cpu.read8(w + WK_COUNT) & 1 != 0 { 7 } else { 6 };
    for dx in &rows[..n] {
        nested!(cpu, 0x0608_c8ac, walker_rocket(cpu, w, sw(*dx)));
    }
    walker_count_up(cpu, w);
}

/// The angle to the nearer player (from the walker's place) kept at +0x1a6.
fn walker_look(cpu: &mut Cpu, w: u32) {
    let near = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, w + O_FLAGS));
    let a = nested!(cpu, 0x0604_5e30, aim::angle_to(cpu, w, near));
    cpu.write16(w + WK_ANGLE, a as u16);
}

fn walker_count_up(cpu: &mut Cpu, w: u32) {
    let count = cpu.read8(w + WK_COUNT).wrapping_add(1);
    cpu.write8(w + WK_COUNT, count);
}

/// The walker's salvo (0x0608cd38): past 120 on its clock it goes to mode 2, the clock 0, aims at
/// the nearer player (+0x1a6) and its stage goes up; at clock 60, 62, 64, 66, 68 and 70 flashes
/// (walker_flash) at +-0, 0x800, 0x1000, 0x1800, 0x2000 and 0x5000 turn.
pub fn walker_salvo(cpu: &mut Cpu, w: u32) {
    if cpu.read32(w + WK_CLOCK) as i32 > 120 {
        cpu.write8(w + WK_MODE, 2);
        cpu.write32(w + WK_CLOCK, 0);
        walker_look(cpu, w);
        let stage = cpu.read8(w + WK_STAGE).wrapping_add(1);
        cpu.write8(w + WK_STAGE, stage);
    }
    let flashes: [(u32, &[u16]); 6] = [
        (60, &[0]),
        (62, &[0x0800, 0xf800]),
        (64, &[0x1000, 0xf000]),
        (66, &[0x1800, 0xe800]),
        (68, &[0x2000, 0xe000]),
        (70, &[0x5000, 0xb000]),
    ];
    for (at, turns) in flashes {
        if cpu.read32(w + WK_CLOCK) == at {
            for &dx in turns {
                nested!(cpu, 0x0608_c64c, walker_flash(cpu, w, sw(dx)));
            }
        }
    }
}

/// The walker's blast (0x0608ce44): every 40th clock a sound (17 or 18 by a random bit) at its x;
/// a puff of smoke from a random place near it (x up to 80.0 around, y 32.0 above the place, the
/// z and a size of 1.0 less a random eighth) with a character that is the base plus 80 or its
/// own (+0x2a); past 180 it is done (mode 0x70), the scroll goes on and the timeline of the
/// stage's end (0x060afc24) starts.
pub fn walker_blast(cpu: &mut Cpu, w: u32) {
    let d = cpu.stack();
    let clock = cpu.read32(w + WK_CLOCK);
    if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, clock, 40)) == 0 {
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
        let se = if r & 1 == 0 { 18 } else { 17 };
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, se));
    }
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x50_0000));
    let x = r.wrapping_add(cpu.read32(w + 20));
    cpu.write32(d, x);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x20_0000));
    let y = r.wrapping_add(cpu.read32(w + 24)).wrapping_sub(0x20_0000);
    cpu.write32(d + 4, y);
    let z = cpu.read32(w + 28);
    cpu.write32(d + 8, z);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write32(d + 12, 0x1_0000u32.wrapping_sub((r & 7) << 11));
    nested!(cpu, 0x0605_77d0, explosions::smoke_puff(cpu, d));
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    if r & 0x800 != 0 {
        let base = cpu.read16(CHAR_BASE);
        cpu.write16(w + 40, base.wrapping_add(80));
    } else {
        let v = cpu.read16(w + 42);
        cpu.write16(w + 40, v);
    }
    if cpu.read32(w + WK_CLOCK) as i32 > 0xb4 {
        cpu.write8(w + O_MODE, MODE_DONE);
        nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
        nested!(cpu, 0x0605_7354, timeline::timeline_set(cpu, 0x060a_fc24));
    }
}

/// The walker's step (0x0608cf44). In modes 0-3 once its power (+0x50) is gone, or its stage is
/// past 2: mode 4, clock 0, its two flags 0x40, the fades stopped, the screen's colour offset
/// 0x80 and faded to 0 at speed 200, and with the power gone the stage's flags (0x0607405c) get
/// 0x80000400. In those modes the pose (+0x6c) steps; the routine of the mode (0x060acb80) runs
/// unless it has finished it; then the two turns, and its 16 joints (+0xd8) turn toward their
/// targets (+0x98) by their turns (+0xf8, speed +0x118); in the early modes the flag 0x10 of
/// +0x19c follows whether the second turn (+0x1a8) is near 0 (within 0x800); the clock goes on.
pub fn walker_step(cpu: &mut Cpu, w: u32) {
    if cpu.read8(w + WK_MODE) <= 3 {
        let power = cpu.read32(w + 0x50) as i32;
        if power <= 0 || cpu.read8(w + WK_STAGE) as i8 > 2 {
            cpu.write8(w + WK_MODE, 4);
            cpu.write32(w + WK_CLOCK, 0);
            cpu.write16(w + WK_FLAGS_2, 0x40);
            cpu.write16(w + 0x19e, 0x40);
            nested!(cpu, 0x0605_e9d8, screen::stop_fades(cpu));
            nested!(cpu, 0x0605_e6dc, screen::set_fade_offset(cpu, 0x80, 0x80, 0x80));
            nested!(cpu, 0x0605_e93c, screen::fade_to(cpu, 0, 0, 0, 200));
            if cpu.read32(w + 0x50) as i32 <= 0 {
                let flags = cpu.read32(actors::END_FLAGS) | 0x8000_0400;
                cpu.write32(actors::END_FLAGS, flags);
            }
        }
    }
    if cpu.read8(w + WK_MODE) <= 3 {
        nested!(cpu, 0x0605_6644, models::pose_step(cpu, w + 0x6c));
    }
    let mode = cpu.read8(w + WK_MODE) as u32;
    let routine = cpu.read32(WK_STATES + 4 * mode);
    cpu.call_returning(0x0608_cfe4, routine, &[w]);
    if cpu.read8(w + O_MODE) == MODE_DONE {
        return;
    }
    nested!(cpu, 0x0608_c55c, walker_aim(cpu, w));
    nested!(cpu, 0x0608_c5c0, walker_aim_spring(cpu, w));
    for i in 0..16u32 {
        let to = cpu.read16(w + WK_TARGETS + 2 * i) as u32;
        let from = cpu.read16(w + WK_ANGLES + 2 * i) as u32;
        cpu.nested_with_stack(&WK_TURN_PROLOGUE, 0x0605_70d8, &[0x8000, 6, 0x200], |cpu| {
            aim::turn_toward(cpu, w + WK_STEPS + 2 * i, w + WK_SPEEDS + 2 * i, from, to, 0x8000, 6, 0x200)
        });
        let v = cpu.read16(w + WK_ANGLES + 2 * i).wrapping_add(cpu.read16(w + WK_STEPS + 2 * i));
        cpu.write16(w + WK_ANGLES + 2 * i, v);
    }
    if cpu.read8(w + WK_MODE) <= 3 {
        let v = (cpu.read16(w + WK_SPRING) as u32).wrapping_add(0x800) & 0xf000;
        if v == 0 {
            let f = cpu.read16(w + WK_FLAGS_2) | 0x10;
            cpu.write16(w + WK_FLAGS_2, f);
        } else {
            let f = cpu.read16(w + WK_FLAGS_2) & 0xffef;
            cpu.write16(w + WK_FLAGS_2, f);
        }
    }
    let clock = cpu.read32(w + WK_CLOCK).wrapping_add(1);
    cpu.write32(w + WK_CLOCK, clock);
}

/// The turn of a walker's sprite (`d`) plus `add`, as the 16-bit angle the sine tables take.
fn wk_turn_at(cpu: &mut Cpu, d: u32, add: u16) -> u32 {
    cpu.read16(d + S_TURN).wrapping_add(add) as i16 as i32 as u32
}

/// `k` times the sine and the cosine of the angle `a`.
fn wk_trig(cpu: &mut Cpu, a: u32, k: i32) -> (u32, u32) {
    let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a)) as i32;
    let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a)) as i32;
    (sin.wrapping_mul(k) as u32, cos.wrapping_mul(k) as u32)
}

/// The sprite's place moved by `k` times the sine and the cosine of its turn plus `add` (x one
/// way, y the other), from where it stands.
fn wk_move(cpu: &mut Cpu, d: u32, add: u16, k: i32) {
    let a = wk_turn_at(cpu, d, add);
    let (dx, dy) = wk_trig(cpu, a, k);
    let x = cpu.read32(d + 4).wrapping_add(dx);
    cpu.write32(d + 4, x);
    let y = cpu.read32(d + 8).wrapping_sub(dy);
    cpu.write32(d + 8, y);
}

/// The sprite's place moved from its kept place (`WK_KEPT_X`, `WK_KEPT_Y`).
fn wk_move_kept(cpu: &mut Cpu, d: u32, add: u16, k: i32) {
    let a = wk_turn_at(cpu, d, add);
    let (dx, dy) = wk_trig(cpu, a, k);
    let x = cpu.read32(d + WK_KEPT_X).wrapping_add(dx);
    cpu.write32(d + 4, x);
    let y = cpu.read32(d + WK_KEPT_Y).wrapping_sub(dy);
    cpu.write32(d + 8, y);
}

/// The two word places the sprite is put out from (with the walker's feet kept in 16 units), a
/// pair of sprite fields the draw keeps: the kept x (+0x30) and y (+0x2c), the lift's place (+0x34).
const WK_KEPT_X: u32 = 48;
const WK_KEPT_Y: u32 = 44;
const WK_LIFT_AT: u32 = 52;

/// A joint's drawn length: 47 less the word at `at` (+0xdc and +0xea) over 256 (the sine times it
/// is moved along).
fn wk_reach(cpu: &mut Cpu, v: u32) -> u32 {
    let mut v = v as i16 as i32 as u32;
    if (v as i32) < 0 {
        v = v.wrapping_add(0xff);
    }
    nested!(cpu, 0x0600_8164, clib::ashiftrt_r4::<8>(cpu, v));
    47u32.wrapping_sub(((v as i32) >> 8) as u32)
}

/// The sprite's place moved by the sine and the cosine of its turn plus `add`, each times the reach
/// of the word at `at`.
fn wk_move_reach(cpu: &mut Cpu, d: u32, o: u32, add: u16, at: u32) {
    let a = wk_turn_at(cpu, d, add);
    let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    let v = signed_word(cpu, o + at);
    let k = wk_reach(cpu, v);
    let x = cpu.read32(d + 4).wrapping_add(sin.wrapping_mul(k));
    cpu.write32(d + 4, x);
    let a = wk_turn_at(cpu, d, add);
    let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    let v = signed_word(cpu, o + at);
    let k = wk_reach(cpu, v);
    let y = cpu.read32(d + 8).wrapping_sub(cos.wrapping_mul(k));
    cpu.write32(d + 8, y);
}

/// The sprite's place moved to (`at`, `at` + 4) from where it stands.
fn wk_move_to(cpu: &mut Cpu, d: u32, at: u32, add: u16, k: i32) {
    let a = wk_turn_at(cpu, d, add);
    let (dx, dy) = wk_trig(cpu, a, k);
    let x = cpu.read32(d + 4).wrapping_add(dx);
    cpu.write32(at, x);
    let y = cpu.read32(d + 8).wrapping_sub(dy);
    cpu.write32(at + 4, y);
}

/// The same from its kept place.
fn wk_move_kept_to(cpu: &mut Cpu, d: u32, at: u32, k: i32) {
    let a = wk_turn_at(cpu, d, 0);
    let (dx, dy) = wk_trig(cpu, a, k);
    let x = cpu.read32(d + WK_KEPT_X).wrapping_add(dx);
    cpu.write32(at, x);
    let y = cpu.read32(d + WK_KEPT_Y).wrapping_sub(dy);
    cpu.write32(at + 4, y);
}

/// The sprite's place from `x` and its kept y moved by the sine and cosine of its turn plus `add`,
/// each times `k`; the y is kept.
fn wk_move_from(cpu: &mut Cpu, d: u32, add: u16, k: i32, x: u32) {
    let a = wk_turn_at(cpu, d, add);
    let (dx, dy) = wk_trig(cpu, a, k);
    cpu.write32(d + 4, x.wrapping_add(dx));
    let y = cpu.read32(d + WK_KEPT_Y).wrapping_sub(dy);
    cpu.write32(d + WK_KEPT_Y, y);
    cpu.write32(d + 8, y);
}

fn wk_draw_one(cpu: &mut Cpu, d: u32) {
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
}

/// The sprite's turn plus (or minus) the word at `by`.
fn wk_turn_by(cpu: &mut Cpu, d: u32, by: u32, minus: bool) {
    let by = cpu.read16(by);
    let t = cpu.read16(d + S_TURN);
    let t = if minus { t.wrapping_sub(by) } else { t.wrapping_add(by) };
    cpu.write16(d + S_TURN, t);
}

/// The walker's draw (0x0608d0e8, its sprite `s`): the body (frame 0), then turned about the
/// walker's turn (+0x1a0) the shoulder guns and their arms in both halves: the left half is kept
/// (its places stored in the walker) on even frames, the right on odd, so each of the walker's
/// kept places (+0x154-+0x1b4) is where its last frame drew it.
pub fn walker_draw(cpu: &mut Cpu, s: u32) {
    let o = s.wrapping_sub(0x10);
    let d = cpu.stack();
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, d));
    let w = cpu.read16(o + WK_FLAGS_2);
    if w & 2 != 0 {
        cpu.write16(o + WK_FLAGS_2, w & 0xfffc);
    }
    let w = cpu.read16(o + WK_FLAGS_2);
    if w & 1 != 0 {
        cpu.write16(o + WK_FLAGS_2, w | 2);
    }
    let kind = cpu.read8(s + S_KIND);
    cpu.write8(d + S_KIND, kind);
    let x0 = signed_word(cpu, o + 0xf6) << 8;
    cpu.write32(d + 4, x0);
    cpu.write32(d + WK_KEPT_X, x0);
    let z = cpu.read32(o + 28);
    cpu.write32(d + 12, z);
    if cpu.read16(o + WK_FLAGS_2) & 1 != 0 {
        let base = cpu.read16(CHAR_BASE);
        cpu.write16(d + S_COLOUR, base.wrapping_add(0xe0));
    } else {
        let c = cpu.read16(s + 24);
        cpu.write16(d + S_COLOUR, c);
    }
    let y0 = cpu.read32(o + 24).wrapping_add(signed_word(cpu, o + 0xf4) << 8);
    cpu.write32(d + WK_KEPT_Y, y0);
    cpu.write32(d + 8, y0);
    cpu.write32(d, 0x20);
    cpu.write8(d + S_LIFT, 0xf6);
    cpu.write8(d + S_FRAME, 0);
    wk_draw_one(cpu, d);

    // the body's two guns and the middle one
    let t = cpu.read16(o + WK_TURN);
    cpu.write16(d + S_TURN, t);
    wk_move_kept(cpu, d, 0, -13);
    cpu.write8(d + S_LIFT, 0xf8);
    cpu.write8(d + S_FRAME, 1);
    let (x, y) = (cpu.read32(d + 4), cpu.read32(d + 8));
    cpu.write32(o + WK_AT, x);
    cpu.write32(o + WK_AT + 4, y);
    wk_draw_one(cpu, d);
    cpu.write8(d + S_LIFT, 0xfa);
    cpu.write8(d + S_FRAME, 3);
    wk_move_kept(cpu, d, 0, 1);
    let (x, y) = (cpu.read32(d + 4), cpu.read32(d + 8));
    cpu.write32(o + 0x15c, x);
    cpu.write32(o + 0x160, y);
    wk_draw_one(cpu, d);
    let v = (cpu.read16(o + WK_SPRING) as u32).wrapping_add(0x800) & 0xf000;
    let frame = if v == 0 {
        4
    } else if v == 0xf000 {
        5
    } else {
        6
    };
    cpu.write8(d + S_FRAME, frame);
    wk_move_kept(cpu, d, 0, 16);
    if frame == 4 {
        wk_move_kept_to(cpu, d, o + 0x164, -26);
    } else {
        let x = cpu.read32(d + WK_KEPT_X);
        cpu.write32(o + 0x164, x);
        let y = cpu.read32(d + WK_KEPT_Y);
        cpu.write32(o + 0x168, y);
    }
    if cpu.read8(o + WK_MODE) == 2 && cpu.read8(o + WK_STAGE) as i8 == 2 {
        cpu.write8(d + S_LIFT, 9);
    }
    wk_draw_one(cpu, d);
    cpu.write32(d + WK_LIFT_AT, d + S_LIFT);
    cpu.write8(d + S_LIFT, 0xf9);
    cpu.write8(d + S_FRAME, 2);
    let a = wk_turn_at(cpu, d, 0x4800);
    let (dx, dy) = wk_trig(cpu, a, 16);
    let x = cpu.read32(d + WK_KEPT_X).wrapping_add(dx);
    cpu.write32(d + 4, x);
    let y = cpu.read32(d + WK_KEPT_Y).wrapping_sub(dy);
    cpu.write32(d + 8, y);
    let r9 = signed_word(cpu, d + S_TURN);
    let t = (cpu.read16(o + WK_SPRING) as u32).wrapping_add(r9) as u16;
    cpu.write16(d + S_TURN, t);
    wk_draw_one(cpu, d);
    cpu.write32(d, 0x30);
    let a = r9.wrapping_add(0xb800) as i16 as i32 as u32;
    let (dx, dy) = wk_trig(cpu, a, 16);
    let x = cpu.read32(d + WK_KEPT_X).wrapping_add(dx);
    cpu.write32(d + 4, x);
    let y = cpu.read32(d + WK_KEPT_Y).wrapping_sub(dy);
    cpu.write32(d + WK_KEPT_Y, y);
    cpu.write32(d + 8, y);
    let t = r9.wrapping_sub(cpu.read16(o + WK_SPRING) as u32) as u16;
    cpu.write16(d + S_TURN, t);
    wk_draw_one(cpu, d);

    // the pods at either side
    cpu.write32(d, 0x20);
    cpu.write8(d + S_LIFT, 0xf7);
    cpu.write16(d + S_TURN, 0);
    let y = cpu.read32(o + 24);
    cpu.write32(d + 8, y);
    cpu.write8(d + S_FRAME, 7);
    let x = cpu.read32(d + WK_KEPT_X).wrapping_add(0x23_0000);
    cpu.write32(d + 4, x);
    wk_draw_one(cpu, d);
    cpu.write32(d, 0x30);
    let x = cpu.read32(d + WK_KEPT_X).wrapping_sub(0x23_0000);
    cpu.write32(d + 4, x);
    wk_draw_one(cpu, d);

    // the left arm, kept on even frames
    cpu.write8(d + S_LIFT, 0xf7);
    cpu.write32(d, 0x20);
    cpu.write8(d + S_FRAME, 8);
    let x = cpu.read32(d + WK_KEPT_X).wrapping_add(0x38_0000);
    cpu.write32(d + 4, x);
    let y = cpu.read32(o + 24).wrapping_add(signed_word(cpu, o + 0xe4) << 8);
    cpu.write32(d + 8, y);
    let even = cpu.read32(GAME_FRAMES) & 1 == 0;
    if even {
        cpu.write32(o + 0x16c, x);
        cpu.write32(o + 0x170, y);
    }
    wk_draw_one(cpu, d);
    cpu.write8(d + S_LIFT, 0xf6);
    cpu.write8(d + S_FRAME, 9);
    let t = cpu.read16(o + 0xd8);
    cpu.write16(d + S_TURN, t);
    wk_draw_one(cpu, d);
    cpu.write8(d + S_LIFT, 0xf5);
    cpu.write8(d + S_FRAME, 10);
    wk_move(cpu, d, 0x4000, 27);
    wk_turn_by(cpu, d, o + 0xda, false);
    if cpu.read32(GAME_FRAMES) & 1 == 0 {
        wk_move_to(cpu, d, o + 0x174, 0x4000, 16);
        let (x, y) = (cpu.read32(d + 4), cpu.read32(d + 8));
        cpu.write32(o + 0x17c, x);
        cpu.write32(o + 0x180, y);
    }
    wk_draw_one(cpu, d);
    cpu.write8(d + S_FRAME, 11);
    wk_move(cpu, d, 0, 3);
    wk_move_reach(cpu, d, o, 0x4000, 0xdc);
    wk_draw_one(cpu, d);
    cpu.write8(d + S_FRAME, 12);
    wk_turn_by(cpu, d, o + 0xde, false);
    if cpu.read32(GAME_FRAMES) & 1 == 0 {
        let (x, y) = (cpu.read32(d + 4), cpu.read32(d + 8));
        cpu.write32(o + 0x194, x);
        cpu.write32(o + 0x198, y);
    }
    wk_draw_one(cpu, d);
    wk_move(cpu, d, 0x4000, 5);
    let (x1, y1) = (cpu.read32(d + 4), cpu.read32(d + 8));
    cpu.write32(d + WK_KEPT_Y, y1);
    wk_move(cpu, d, 0x8000, 18);
    cpu.write8(d + S_FRAME, 14);
    if cpu.read32(GAME_FRAMES) & 1 == 0 {
        let (x, y) = (cpu.read32(d + 4), cpu.read32(d + 8));
        cpu.write32(o + 0x184, x);
        cpu.write32(o + 0x188, y);
    }
    wk_turn_by(cpu, d, o + 0xe0, false);
    let y2 = cpu.read32(d + 8);
    let rate = y2.wrapping_sub(cpu.read32(o + 0x1b4)).wrapping_add(cpu.read32(o + 0x1b0));
    cpu.write32(o + 0x1b0, rate);
    cpu.write32(o + 0x1b4, y2);
    wk_draw_one(cpu, d);
    wk_turn_by(cpu, d, o + 0xe0, true);
    wk_move_from(cpu, d, 0, 10, x1);
    cpu.write8(d + S_FRAME, 13);
    if cpu.read32(GAME_FRAMES) & 1 == 0 {
        let (x, y) = (cpu.read32(d + 4), cpu.read32(d + 8));
        cpu.write32(o + 0x18c, x);
        cpu.write32(o + 0x190, y);
    }
    wk_turn_by(cpu, d, o + 0xe2, false);
    wk_draw_one(cpu, d);

    // the right arm, kept on odd frames
    cpu.write8(d + S_LIFT, 0xf7);
    cpu.write32(d, 0x30);
    cpu.write16(d + S_TURN, 0);
    cpu.write8(d + S_FRAME, 15);
    let x = cpu.read32(d + WK_KEPT_X).wrapping_sub(0x38_0000);
    cpu.write32(d + 4, x);
    let y = cpu.read32(o + 24).wrapping_add(signed_word(cpu, o + 0xf2) << 8);
    cpu.write32(d + 8, y);
    if cpu.read32(GAME_FRAMES) & 1 != 0 {
        cpu.write32(o + 0x16c, x);
        cpu.write32(o + 0x170, y);
    }
    wk_draw_one(cpu, d);
    cpu.write8(d + S_LIFT, 0xf6);
    cpu.write8(d + S_FRAME, 9);
    let t = (cpu.read16(o + 0xe6) as i16).wrapping_neg() as u16;
    cpu.write16(d + S_TURN, t);
    wk_draw_one(cpu, d);
    cpu.write8(d + S_LIFT, 0xf5);
    cpu.write8(d + S_FRAME, 10);
    wk_move(cpu, d, 0xc000, 27);
    wk_turn_by(cpu, d, o + 0xe8, true);
    if cpu.read32(GAME_FRAMES) & 1 != 0 {
        wk_move_to(cpu, d, o + 0x174, 0xc000, 16);
        let (x, y) = (cpu.read32(d + 4), cpu.read32(d + 8));
        cpu.write32(o + 0x17c, x);
        cpu.write32(o + 0x180, y);
    }
    wk_draw_one(cpu, d);
    cpu.write8(d + S_FRAME, 11);
    wk_move(cpu, d, 0, 3);
    wk_move_reach(cpu, d, o, 0xc000, 0xea);
    wk_draw_one(cpu, d);
    cpu.write8(d + S_FRAME, 12);
    wk_turn_by(cpu, d, o + 0xec, true);
    if cpu.read32(GAME_FRAMES) & 1 != 0 {
        let (x, y) = (cpu.read32(d + 4), cpu.read32(d + 8));
        cpu.write32(o + 0x194, x);
        cpu.write32(o + 0x198, y);
    }
    wk_draw_one(cpu, d);
    wk_move(cpu, d, 0xc000, 5);
    let (x1, y1) = (cpu.read32(d + 4), cpu.read32(d + 8));
    cpu.write32(d + WK_KEPT_Y, y1);
    wk_move(cpu, d, 0x8000, 18);
    cpu.write8(d + S_FRAME, 14);
    if cpu.read32(GAME_FRAMES) & 1 != 0 {
        let (x, y) = (cpu.read32(d + 4), cpu.read32(d + 8));
        cpu.write32(o + 0x184, x);
        cpu.write32(o + 0x188, y);
    }
    wk_turn_by(cpu, d, o + 0xee, true);
    wk_draw_one(cpu, d);
    wk_turn_by(cpu, d, o + 0xee, false);
    wk_move_from(cpu, d, 0, 10, x1);
    cpu.write8(d + S_FRAME, 13);
    if cpu.read32(GAME_FRAMES) & 1 != 0 {
        let (x, y) = (cpu.read32(d + 4), cpu.read32(d + 8));
        cpu.write32(o + 0x18c, x);
        cpu.write32(o + 0x190, y);
    }
    wk_turn_by(cpu, d, o + 0xee, true);
    wk_draw_one(cpu, d);
}

/// The walker's maker (0x0608da14, from a timeline event `event` whose byte at +8 says which
/// side): an object of list 2, 0x1c0 bytes, step 0x0608cf44, draw 0x0608d0e8, kind 30, power
/// 50000, flags 0xc80000, its angles and turns set (+0x1a0 and +0x1a8 0x8000), the 9 proxies for its
/// weak points (0x0608bed8: each with the walker's number, the places +0x154-+0x194, the flags
/// +0x19c or +0x19e and the walker's power) and a tracker (0x0608bfa4); with the side byte 0 it
/// stands at y 192.0 on mode 1 and its pose and script go one way, else at y -192.0 and the
/// other. Gives 1 (0 with no number or room).
pub fn walker_make(cpu: &mut Cpu, event: u32) -> u32 {
    let f = cpu.stack();
    cpu.write32(f, event);
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, 2)) as i8;
    if n == -1 {
        return 0;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0x1c0)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, PART_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + 0x3c));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + O_BEHAVIOUR, 0x0608_cf44);
    cpu.write32(o + O_FLAGS, 0xc8_0000);
    cpu.write32(o + O_DRAW, 0x0608_d0e8);
    cpu.write8(o + O_KIND, 30);
    cpu.write8(o + O_FRAME, 0);
    cpu.write8(o + O_NUMBER, n as u8);
    let number = cpu.read8(o + O_NUMBER) as i8 as i32 as u32;
    cpu.write32(NUMBERED.wrapping_add(number << 2), o);
    cpu.write32(o + P_POWER, 50000);
    cpu.write8(o + O_54, 1);
    cpu.write8(o + O_55, 0);
    let colour = cpu.read16(kind(30) + K_COLOUR);
    cpu.write16(o + O_COLOUR_A, colour);
    cpu.write16(o + O_COLOUR_B, colour);
    cpu.write32(o + O_POSITION, 0);
    cpu.write32(o + O_POSITION + 8, 0);
    cpu.write16(o + WK_FLAGS, 0);
    cpu.write32(o + WK_CLOCK, 0);
    cpu.write8(o + WK_STAGE, 0);
    cpu.write8(o + WK_COUNT, 0);
    cpu.write16(o + WK_TURN, 0x8000);
    cpu.write16(o + WK_TURN_2, 0);
    cpu.write16(o + WK_TURN_3, 0);
    cpu.write16(o + WK_ANGLE, 0);
    cpu.write16(o + WK_SPRING, 0x8000);
    cpu.write16(o + 0x1aa, 0);
    cpu.write16(o + 0x1ac, 0);
    cpu.write16(o + WK_GAZE, 0);
    cpu.write32(o + WK_SINK, 0);
    let y = cpu.read32(o + 24);
    cpu.write32(o + 0x1b4, y);
    cpu.write16(o + WK_FLAGS_2, 0x10);
    cpu.write16(o + 0x19e, 0x10);
    // the weak points: (script, place, the flags they follow)
    let points: [(u32, u32, u32); 9] = [
        (0x060a_c814, 0x154, o + WK_FLAGS_2),
        (0x060a_c824, 0x15c, o + 0x19e),
        (0x060a_c834, 0x164, o + 0x19e),
        (0x060a_c844, 0x16c, o + 0x19e),
        (0x060a_c854, 0x174, o + 0x19e),
        (0x060a_c854, 0x17c, o + 0x19e),
        (0x060a_c864, 0x184, o + 0x19e),
        (0x060a_c864, 0x18c, o + 0x19e),
        (0x060a_c874, 0x194, o + 0x19e),
    ];
    for (script, place, flags) in points {
        let p = nested!(cpu, 0x0608_bed8, proxy_make(cpu, script));
        if p != 0 {
            let number = cpu.read8(o + O_NUMBER);
            cpu.write8(p + O_NUMBER, number);
            cpu.write32(p + PX_PLACE, o + place);
            cpu.write32(p + PX_FLAGS, flags);
            cpu.write32(p + PX_HEALTH, o + P_POWER);
        }
    }
    let t = nested!(cpu, 0x0608_bfa4, tracker_make(cpu));
    if t != 0 {
        cpu.write32(t + PX_HEALTH, o + WK_AT);
        cpu.write32(t + PX_FLAGS, o + 0x19e);
    }
    cpu.write16(o + 0x7c, 0x10);
    cpu.write32(o + 0x80, o + WK_TARGETS);
    cpu.write32(o + 0x84, o + 0xb8);
    cpu.write32(o + 0x88, 0x060a_ca60);
    let side_at = cpu.read32(f).wrapping_add(8);
    cpu.write32(f, side_at);
    if cpu.read8(side_at) == 0 {
        cpu.write8(o + WK_MODE, 1);
        cpu.write32(o + 24, 0xc0_0000);
        nested!(cpu, 0x0605_65f0, models::pose_set(cpu, o + 0x6c, 0x060a_c8a0));
        nested!(cpu, 0x0605_6628, actors::script_start(cpu, o + 0x6c, 0x060a_cab4));
        cpu.write16(o + WK_ANGLE, 0);
    } else {
        cpu.write8(o + WK_MODE, 0);
        cpu.write32(o + 24, 0xff40_0000);
        nested!(cpu, 0x0605_65f0, models::pose_set(cpu, o + 0x6c, 0x060a_ca20));
        nested!(cpu, 0x0605_6628, actors::script_start(cpu, o + 0x6c, 0x060a_ca98));
        cpu.write16(o + WK_ANGLE, 0x8000);
    }
    nested!(cpu, 0x0605_6644, models::pose_step(cpu, o + 0x6c));
    for i in 0..16u32 {
        cpu.write16(o + WK_STEPS + 2 * i, 0);
        cpu.write16(o + WK_SPEEDS + 2 * i, 0);
        let v = cpu.read16(o + WK_TARGETS + 2 * i);
        cpu.write16(o + WK_ANGLES + 2 * i, v);
    }
    1
}

// ---- the scrap and the orbiting pairs ------------------------------------------------------------

/// A scrap's step (0x0608de38): its speed down (+0x84) eased toward its target (+0xb0) by 0x2000
/// and its y moved by it; its scale (+0x20) a random 0x8000-0xffff and its second (+0x24) eased
/// toward +0xac by 0x1000; its flags' 0x10 (+0x10) from a random bit, its colour byte +0x33 126 on
/// even frames and 8 on odd, its frame (+0x2f) 8 or 9. Done when it ends (bullets::bullet_ends,
/// not at the back).
pub fn scrap_step(cpu: &mut Cpu, o: u32) {
    if nested!(cpu, 0x0605_3b10, bullets::bullet_ends(cpu, o, 1)) & 0xff != 0 {
        return;
    }
    let from = cpu.read32(o + 0x84);
    let to = cpu.read32(o + 0xb0);
    let speed = nested!(cpu, 0x0604_6168, actors::approach(cpu, from, to, 0x2000));
    cpu.write32(o + 0x84, speed);
    let y = cpu.read32(o + 24).wrapping_add(speed);
    cpu.write32(o + 24, y);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write32(o + 0x20, (r & 0x7fff).wrapping_add(0x8000));
    let from = cpu.read32(o + 0x24);
    let to = cpu.read32(o + 0xac);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, from, to, 0x1000));
    cpu.write32(o + 0x24, v);
    let shade = if cpu.read32(GAME_FRAMES) & 1 == 0 { 126 } else { 8 };
    cpu.write8(o + O_33, shade);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write32(o + O_FLAGS, (r & 16) | 0x420);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write8(o + O_FRAME, ((r & 1) + 8) as u8);
}

/// A scrap made (0x0608defc) at the place `at` (x, y), numbered `number`, of the kind `which`
/// (the table 0x060accfc gives its target +0xac, 0x060accf0 its +0xb0): an object of list 2, 0xbc
/// bytes, run by 0x0604d780, step 0x0608de38, kind 66, flags 0x420, script 0x060acce6; and a drift
/// effect (0x06057454) from its place with a random 0x2000 bit.
pub fn scrap_make(cpu: &mut Cpu, at: u32, number: u32, which: u32) {
    let number = number as u8 as i8 as i32 as u32;
    let which = which & 0xff;
    let f = cpu.stack();
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0xbc)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, 0x0604_d780));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + 0x3c));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + O_BEHAVIOUR, 0x0608_de38);
    cpu.write32(o + O_FLAGS, 0x420);
    cpu.write8(o + O_NUMBER, number as u8);
    let x = cpu.read32(at);
    cpu.write32(o + O_POSITION, x);
    let y = cpu.read32(at + 4);
    cpu.write32(o + O_POSITION + 4, y);
    cpu.write32(o + O_POSITION + 8, 0);
    cpu.write8(o + O_33, 0x7e);
    cpu.write8(o + O_FRAME, 8);
    cpu.write8(o + O_KIND, 0x42);
    cpu.write32(o + O_SCRIPT, 0x060a_cce6);
    cpu.write8(o + O_54, 1);
    cpu.write8(o + O_55, 5);
    let colour = cpu.read16(CHAR_BASE).wrapping_add(0x40);
    cpu.write16(o + O_COLOUR_A, colour);
    cpu.write16(o + O_COLOUR_B, colour);
    cpu.write32(o + O_SCALE, 0);
    cpu.write32(o + O_SCALE_2, 0);
    let v = cpu.read32(0x060a_ccfc + 4 * which);
    cpu.write32(o + 0xac, v);
    let v = cpu.read32(0x060a_ccf0 + 4 * which);
    cpu.write32(o + 0xb0, v);
    cpu.write32(f + 60, 0x0607_30d4);
    cpu.write16(f + 24, 0x8000);
    let (x, y, z) = (cpu.read32(o + 20), cpu.read32(o + 24), cpu.read32(o + 28));
    cpu.write32(f, x);
    cpu.write32(f + 4, y);
    cpu.write32(f + 8, z);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write16(f + 32, ((r & 0x2000) as u16).wrapping_add(0x80e0));
    cpu.write32(f + 12, 0xc000);
    cpu.write32(f + 16, 0xc000);
    cpu.write32(f + 20, 0xfffe_0000);
    nested!(cpu, 0x0605_7454, actors::drift_effect(cpu, f));
}

/// Smoke from an object's place (0x0608e210): a puff (explosions::smoking_set) at its place with
/// the character of its +0x30.
pub fn smoke_from(cpu: &mut Cpu, o: u32) {
    let f = cpu.stack();
    let (x, y, z) = (cpu.read32(o + 20), cpu.read32(o + 24), cpu.read32(o + 28));
    cpu.write32(f, x);
    cpu.write32(f + 4, y);
    cpu.write32(f + 8, z);
    let c = cpu.read16(o + 0x30);
    cpu.write16(f + 24, c);
    nested!(cpu, 0x0605_7b70, explosions::smoking_set(cpu, f));
}

/// The orbiting pair's fields (two objects, each the other's partner): the owner (+0x94, its
/// +0xbc says it still fires), the partner (+0x9c), the flags (+0x6e: 1 hit, 0x8000 ending), the
/// state (+0x6c: 0, 1 spiralling out, 16 wandering), the side (+0x72: positive and negative sides
/// turn about the owner), the two angles (+0x74, +0x76) and their speed (+0x78), the speed's
/// velocity (+0x7c, +0x80), the aim (+0x84, +0x88), the base y (+0x8c), the wander speed (+0x90),
/// the slot (+0x98) and the kind number (+0xa0).
const TW_OWNER: u32 = 0x94;
const TW_PARTNER: u32 = 0x9c;
const TW_FLAGS: u32 = 0x6e;
const TW_STATE: u32 = 0x6c;
const TW_SIDE: u32 = 0x72;
const TW_ANGLE_A: u32 = 0x74;
const TW_ANGLE_B: u32 = 0x76;
const TW_SPEED: u32 = 0x78;
const TW_AIM_X: u32 = 0x84;
const TW_AIM_Y: u32 = 0x88;
const TW_BASE_Y: u32 = 0x8c;
const TW_WANDER: u32 = 0x90;
const TW_SLOT: u32 = 0x98;
const TW_ENDING: u16 = 0x8000;

/// An orbiting piece's step (0x0608e244): the flags' 0x8000 comes on once the owner's table
/// (0x060accbb by its +0x96) says 1. Below y 160.0 with power left and the owner firing it carries
/// on; else it is done (mode 0x70, sound 58 if the owner fires, bursts of shots (0x06053db0) in
/// fives or fours by the side if its power is gone, smoke) and its partner's ending flag is set
/// if it has power. Carrying on: state 0 starts the spiral (the angles 0 and 0x4000, base y
/// -48.0, scales 0, speed 0x240); state 1 and 0 ease the speed (+0x78 by 0x100 / 4) and turn the
/// angles by it and twice it, place it by the slot (+0x98: 0-3 the sines and cosines of them
/// times 96, -96, -48 and 48, and 32 or 16 of y over the base) and turn it toward the partner's
/// turn (the other side) or the way it moved; state 16 every 50th frame picks a place to wander
/// to (+0x84, +0x88), turns toward it and moves by its sine and cosine times a speed eased
/// to 2.0; at the end the sprite turn is the kind and frame (+0x2e), a hit bit sets the flag 1, a
/// z beyond -8.0 .. 16.0 hides it, and its scales ease toward 1.0.
pub fn twin_step(cpu: &mut Cpu, o: u32) {
    let owner = cpu.read32(o + TW_OWNER);
    let flags = cpu.read16(o + TW_FLAGS);
    let partner = cpu.read32(o + TW_PARTNER);
    if flags & TW_ENDING == 0 {
        let at = cpu.read16(owner + 0x96) as u32;
        if cpu.read8(0x060a_ccbb + at) == 1 {
            cpu.write16(o + TW_FLAGS, flags | TW_ENDING);
        }
    }
    let y = cpu.read32(o + 24) as i32;
    let carry_on = y <= 0xa0_0000 && cpu.read32(o + 0x50) as i32 > 0 && cpu.read32(owner + 0xbc) as i32 > 0;
    if !carry_on {
        if cpu.read32(o + 24) as i32 <= 0xa0_0000 {
            if cpu.read32(owner + 0xbc) != 0 {
                nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 58));
            }
            if cpu.read32(o + 0x50) as i32 <= 0 {
                twin_burst(cpu, o);
            }
            nested!(cpu, 0x0608_e210, smoke_from(cpu, o));
        }
        cpu.write8(o + O_MODE, MODE_DONE);
        if cpu.read16(o + TW_FLAGS) & TW_ENDING == 0 && cpu.read32(partner + 0x50) as i32 > 0 {
            let v = cpu.read16(partner + TW_FLAGS) | TW_ENDING;
            cpu.write16(partner + TW_FLAGS, v);
        }
        return;
    }
    let state = cpu.read8(o + TW_STATE);
    match state {
        0 | 1 => {
            if state == 0 {
                cpu.write8(o + TW_STATE, 1);
                cpu.write16(o + TW_ANGLE_A, 0);
                cpu.write16(o + TW_ANGLE_B, 0x4000);
                cpu.write32(o + TW_BASE_Y, 0xffd0_0000);
                cpu.write32(o + O_SCALE, 0);
                cpu.write32(o + O_SCALE_2, 0);
                cpu.write16(o + TW_SPEED, 0x240);
            }
            twin_spiral(cpu, o, partner);
        }
        16 => twin_wander(cpu, o),
        _ => {}
    }
    // the end of the step
    let turn = cpu.read16(o + O_ANGLE);
    cpu.write16(o + O_KIND, turn);
    if cpu.read16(o + FL_HIT_BITS) & 0x200 != 0 {
        let v = cpu.read16(o + TW_FLAGS) | 1;
        cpu.write16(o + TW_FLAGS, v);
    }
    let z = cpu.read32(o + 28).wrapping_add(0x8_0000);
    if z > 0x10_0000 {
        let f = cpu.read32(o + O_FLAGS) | 0x8_0000;
        cpu.write32(o + O_FLAGS, f);
    } else {
        let f = cpu.read32(o + O_FLAGS) & 0xfff7_ffff;
        cpu.write32(o + O_FLAGS, f);
    }
    let from = cpu.read32(o + O_SCALE);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, from, 0x1_0000, 0x400));
    cpu.write32(o + O_SCALE, v);
    cpu.write32(o + O_SCALE_2, v);
}

/// The burst of shots (0x06053db0) a piece with no power left throws out: a struct at its place
/// with the object's number, two words by the difficulty (the tables 0x060acd18 and 0x060acd22
/// give the first angle and the step), five on the positive side and four on the other (half a
/// step ahead).
fn twin_burst(cpu: &mut Cpu, o: u32) {
    let f = cpu.stack();
    let n = cpu.read8(o + O_NUMBER);
    cpu.write8(f + 30, n);
    let x = cpu.read32(o + 20);
    cpu.write32(f, x);
    let y = cpu.read32(o + 24);
    cpu.write32(f + 4, y);
    cpu.write32(f + 8, 0);
    cpu.write16(f + 46, 0);
    cpu.write32(f + 48, 0x1_1000);
    let positive = cpu.read8(o + TW_SIDE) as i8 > 0;
    let shots = if positive { 5 } else { 4 };
    for i in 0..shots {
        let level = cpu.read8(WK_DIFFICULTY) as i8 as i32 as u32;
        let step = cpu.read16(0x060a_cd22u32.wrapping_add(level << 1)) as u32;
        let first = cpu.read16(0x060a_cd18u32.wrapping_add(level << 1)) as u32;
        let angle = if positive { first.wrapping_add(step.wrapping_mul(i)) } else { (step >> 1).wrapping_add(first).wrapping_add(step.wrapping_mul(i)) };
        cpu.write16(f + 44, angle as u16);
        nested!(cpu, 0x0605_3db0, bullets::bullet_set(cpu, f));
    }
}

/// An orbiting piece's spiral (states 0 and 1): the speed (+0x78) eased toward 0x100 by 4, the
/// angles turned by it and twice it, the place by the slot's sines and cosines (see twin_step);
/// the turn (+0x30) follows the partner's (the other side) or the way it moved; a flag of 0x8000
/// sends it wandering (state 16); the base y (+0x8c) goes down by 0x1200.
fn twin_spiral(cpu: &mut Cpu, o: u32, partner: u32) {
    let from = cpu.read16(o + TW_SPEED) as u32;
    let speed = nested!(cpu, 0x0604_6168, actors::approach(cpu, from, 0x100, 4));
    cpu.write16(o + TW_SPEED, speed as u16);
    let a = cpu.read16(o + TW_ANGLE_A).wrapping_add(speed as u16);
    cpu.write16(o + TW_ANGLE_A, a);
    let speed = cpu.read16(o + TW_SPEED) as u32;
    let b = cpu.read16(o + TW_ANGLE_B).wrapping_add((speed * 2) as u16);
    cpu.write16(o + TW_ANGLE_B, b);
    let (x, y) = (cpu.read32(o + 20), cpu.read32(o + 24));
    let slot = cpu.read8(o + TW_SLOT) as i8 as i32 as u32;
    if slot <= 3 {
        let a = signed_word(cpu, o + TW_ANGLE_A);
        let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
        let b = signed_word(cpu, o + TW_ANGLE_B);
        let (kx, ky): (i32, i32) = match slot {
            0 => (96, -32),
            1 => (-96, -32),
            2 => (-48, 16),
            _ => (48, 16),
        };
        cpu.write32(o + 20, sin.wrapping_mul(kx as u32));
        let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, b));
        let base = cpu.read32(o + TW_BASE_Y);
        cpu.write32(o + 24, cos.wrapping_mul(ky as u32).wrapping_add(base));
    }
    if (cpu.read8(o + TW_SIDE) as i8) < 0 {
        let t = cpu.read16(partner + O_ANGLE).wrapping_add(0x8000);
        cpu.write16(o + O_ANGLE, t);
    } else {
        let dy = y.wrapping_sub(cpu.read32(o + 24));
        let dx = x.wrapping_sub(cpu.read32(o + 20));
        let a = nested!(cpu, 0x0606_780c, sgl::sl_atan(cpu, dx, dy));
        cpu.write16(o + O_ANGLE, (a as u16).wrapping_add(0x8000));
    }
    if cpu.read16(o + TW_FLAGS) & TW_ENDING != 0 {
        cpu.write8(o + TW_STATE, 16);
        cpu.write32(o + TW_AIM_X, 0);
        cpu.write32(o + TW_AIM_Y, 0);
        cpu.write32(o + TW_WANDER, 0);
    }
    let base = cpu.read32(o + TW_BASE_Y).wrapping_add(0x1200);
    cpu.write32(o + TW_BASE_Y, base);
}

/// An orbiting piece's wander (state 16): every 50th frame a place to go to (+0x84, +0x88: random
/// up to 128.0 across and 64.0 down from the base y); its turn eased toward it (a 36th of the
/// difference); its speed (+0x90) eased toward 2.0 by 0xc00, its velocity (+0x7c, +0x80) that
/// along the turn, the place moved by it, the base y by 0x8000.
fn twin_wander(cpu: &mut Cpu, o: u32) {
    let frames = cpu.read32(GAME_FRAMES);
    if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, frames, 50)) == 0 {
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x80_0000));
        cpu.write32(o + TW_AIM_X, r);
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x40_0000));
        let base = cpu.read32(o + TW_BASE_Y);
        cpu.write32(o + TW_AIM_Y, r.wrapping_add(base));
    }
    let dy = cpu.read32(o + TW_AIM_Y).wrapping_sub(cpu.read32(o + 24));
    let dx = cpu.read32(o + TW_AIM_X).wrapping_sub(cpu.read32(o + 20));
    let a = nested!(cpu, 0x0606_780c, sgl::sl_atan(cpu, dx, dy));
    let to = (a as i16 as i32 as u32).wrapping_add(0x4000);
    let from = signed_word(cpu, o + O_ANGLE);
    let d = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, from, to));
    let q = nested!(cpu, 0x0600_6a78, math::quotient(cpu, d, 36));
    let t = (q as u16).wrapping_add(cpu.read16(o + O_ANGLE));
    cpu.write16(o + O_ANGLE, t);
    let from = cpu.read32(o + TW_WANDER);
    let speed = nested!(cpu, 0x0604_6168, actors::approach(cpu, from, 0x2_0000, 0xc00));
    cpu.write32(o + TW_WANDER, speed);
    let a = signed_word(cpu, o + O_ANGLE);
    let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    let vx = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, sin, speed));
    cpu.write32(o + 0x7c, vx);
    let speed = cpu.read32(o + TW_WANDER);
    let a = signed_word(cpu, o + O_ANGLE);
    let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    let vy = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, cos.wrapping_neg(), speed));
    cpu.write32(o + 0x80, vy);
    let x = cpu.read32(o + 20).wrapping_add(cpu.read32(o + 0x7c));
    cpu.write32(o + 20, x);
    let y = cpu.read32(o + 24).wrapping_add(cpu.read32(o + 0x80));
    cpu.write32(o + 24, y);
    let base = cpu.read32(o + TW_BASE_Y).wrapping_add(0x8000);
    cpu.write32(o + TW_BASE_Y, base);
}

/// An orbiting piece's draw (0x0608e728, its sprite `s`): the flags' 2 goes and is set again from
/// the 1 as the hatches'; the matrix pushed, moved to its place, scaled by +0x20, turned (z, y, x)
/// and its model put: the second kind-65 model list's entry by its number (+0xa0), the next one
/// unless the flag 0x8000 is on, and the one of it that the flag 1 picks.
pub fn twin_draw(cpu: &mut Cpu, s: u32) {
    let o = s.wrapping_sub(0x10);
    let at = s + 94;
    let w = cpu.read16(at);
    if w & 2 != 0 {
        cpu.write16(at, w & 0xfffc);
    }
    let w = cpu.read16(at);
    if w & 1 != 0 {
        cpu.write16(at, w | 2);
    }
    nested!(cpu, 0x0606_71cc, sgl::sl_push_matrix(cpu));
    let (x, y, z) = (cpu.read32(o + 20), cpu.read32(o + 24), cpu.read32(o + 28));
    nested!(cpu, 0x0606_721c, sgl::sl_translate(cpu, x, y, z));
    let scale = cpu.read32(o + 32);
    nested!(cpu, 0x0606_73ec, sgl::sl_scale(cpu, scale, scale, scale));
    let a = signed_word(cpu, s + 32);
    nested!(cpu, 0x0606_726c, sgl::sl_rot_z(cpu, a));
    let a = signed_word(cpu, s + 30);
    nested!(cpu, 0x0606_736c, sgl::sl_rot_y(cpu, a));
    let a = signed_word(cpu, s + 28);
    nested!(cpu, 0x0606_72ec, sgl::sl_rot_x(cpu, a));
    let w = cpu.read16(at) as u32;
    let list = cpu.read32(kind(65));
    let list = cpu.read32(list + 28);
    let n = cpu.read8(o + 0xa0) as u32;
    let entry = if w & 0x8000 != 0 { cpu.read32(list + 4 * n) } else { cpu.read32(list + 4 * n + 4) };
    let model = cpu.read32(entry + 4 * (w & 1));
    nested!(cpu, 0x0606_745c, sgl::sl_put_polygon(cpu, model));
    nested!(cpu, 0x0606_7200, sgl::sl_pop_matrix(cpu));
}

/// An orbiting piece set up (0x0608e828, on the object `o` from twin_pair): its step
/// 0x0608e244, draw 0x0608e728, flags 0xf80400, kind 65, script 0x060acd08, power 0x640, and its
/// state, angles, speeds, place and scales cleared.
pub fn twin_init(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + 0x3c));
    cpu.write32(o + O_BEHAVIOUR, 0x0608_e244);
    cpu.write32(o + O_DRAW, 0x0608_e728);
    cpu.write32(o + O_FLAGS, 0x00f8_0400);
    cpu.write8(o + O_KIND, 65);
    cpu.write8(o + O_FRAME, 0);
    cpu.write32(o + O_SCRIPT, 0x060a_cd08);
    cpu.write8(o + O_54, 1);
    cpu.write8(o + O_55, 18);
    cpu.write32(o + 0x50, 0x640);
    cpu.write8(o + TW_STATE, 0);
    cpu.write16(o + TW_FLAGS, 0);
    cpu.write16(o + 0x70, 0);
    cpu.write32(o + 20, 0);
    cpu.write32(o + 24, 0);
    cpu.write32(o + 28, 0);
    cpu.write32(o + 0x7c, 0);
    cpu.write32(o + 0x80, 0);
    cpu.write16(o + TW_ANGLE_A, 0);
    cpu.write16(o + TW_ANGLE_B, 0);
    cpu.write32(o + 32, 0);
    cpu.write32(o + 36, 0);
}

/// A pair of orbiting pieces (0x0608e8d4) for the owner `owner`, its number's byte `number` (+0x98)
/// and `which` (the kind number, 8 times it plus 5): two objects of list 2, 0xa4 bytes, both
/// registered by number, each the other's partner, the second on the other side (+0x72 -1) turned
/// half about (+0x30 0x8000). Done (mode 0x70) if there are not two numbers and two rooms.
pub fn twin_pair(cpu: &mut Cpu, owner: u32, number: u32, which: u32) {
    let (number, which) = (number as u8 as i8 as i32 as u32, which & 0xff);
    let f = cpu.stack();
    cpu.write32(f, owner);
    cpu.write32(f + 4, number);
    cpu.write32(f + 8, which);
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, 2)) as i8;
    if n == -1 {
        return;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0xa4)) as i8 as i32 as u32;
    let a = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, PART_EXEC));
    if a == 0 {
        return;
    }
    cpu.write8(a + O_NUMBER, n as u8);
    let at = cpu.read8(a + O_NUMBER) as i8 as i32 as u32;
    cpu.write32(NUMBERED.wrapping_add(at << 2), a);
    let m = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, 2)) as i8;
    if m == -1 {
        cpu.write8(a + O_MODE, MODE_DONE);
        return;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0xa4)) as i8 as i32 as u32;
    let b = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, PART_EXEC));
    if b == 0 {
        cpu.write8(a + O_MODE, MODE_DONE);
        return;
    }
    cpu.write8(b + O_NUMBER, m as u8);
    let at = cpu.read8(b + O_NUMBER) as i8 as i32 as u32;
    cpu.write32(NUMBERED.wrapping_add(at << 2), b);
    nested!(cpu, 0x0608_e828, twin_init(cpu, a));
    nested!(cpu, 0x0608_e828, twin_init(cpu, b));
    for p in [a, b] {
        let v = cpu.read32(f);
        cpu.write32(p + TW_OWNER, v);
        let v = cpu.read8(f + 7);
        cpu.write8(p + TW_SLOT, v);
    }
    cpu.write8(a + TW_SIDE, 1);
    cpu.write8(b + TW_SIDE, 0xff);
    cpu.write32(a + TW_PARTNER, b);
    cpu.write32(b + TW_PARTNER, a);
    cpu.write16(a + O_ANGLE, 0);
    cpu.write16(b + O_ANGLE, 0x8000);
    let v = (cpu.read32(f + 8).wrapping_mul(8).wrapping_add(5)) as u8;
    cpu.write8(a + 0xa0, v);
    cpu.write8(b + 0xa0, v);
}

/// What a kite keeps: its owner (+0x90, whose +0xbc says it still fires), its state (+0x6c: 0
/// waits, 1 swings, 2 chases), its wait (+0x70), the target and the swing's centre (+0x7c, +0x84),
/// the swing's angle and reach (+0x88, +0x8a), the side (+0x8c, a signed byte), its velocity
/// (+0x74, +0x78).
const KT_OWNER: u32 = 0x90;
const KT_STATE: u32 = 0x6c;
const KT_WAIT: u32 = 0x70;
const KT_AIM_X: u32 = 0x7c;
const KT_AIM_Y: u32 = 0x84;
const KT_SWING: u32 = 0x88;
const KT_REACH: u32 = 0x8a;
const KT_SIDE: u32 = 0x8c;

/// A kite's step (0x0608ea08). Below y 136.0 with power left and its owner firing it carries on;
/// else it is done (mode 0x70): sound 59 if the owner fires, with no power left and the
/// difficulty past 1 a rocket (bullets::rocket_set) from just above it (two, 10.0 either side, from the
/// third difficulty), smoke. Carrying on: state 0 counts its wait (+0x70) down, then places a
/// target 16.0 across (+0x7c) with the centre at y -80.0 and starts swinging; state 1 swings
/// (angle +0x88 by 0x180 times the side, reach +0x8a eased to 0x60), its place the centre plus
/// the sine times the reach and, up from the centre, the cosine times half the reach, and at
/// y 40.0 it ends the swing (state 2, its turn the swing's angle); state 2 every 80th frame picks
/// a target (+0x7c, random up to 160.0 across), turns toward it by a 32nd of the difference and
/// moves by its sine times 2.25 and cosine times 1.875, the centre y (+0x84) down by a random
/// 0x3000-0xafff; all add 0x4000 to the centre y, a hit bit sets the flag 1, a z out of the
/// range hides it, and its second scale follows its first.
pub fn kite_step(cpu: &mut Cpu, o: u32) {
    let f = cpu.stack();
    let owner = cpu.read32(o + KT_OWNER);
    let y = cpu.read32(o + 24) as i32;
    let mut carry_on = false;
    if y <= 0x88_0000 && cpu.read32(o + 0x50) as i32 > 0 && cpu.read32(owner + 0xbc) as i32 > 0 {
        carry_on = true;
    }
    if !carry_on {
        if cpu.read32(o + 24) as i32 <= 0x88_0000 {
            if cpu.read32(owner + 0xbc) != 0 {
                nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 59));
            }
            let level = cpu.read8(WK_DIFFICULTY) as i8;
            if cpu.read32(o + 0x50) as i32 <= 0 && level > 1 {
                let n = cpu.read8(o + O_NUMBER);
                cpu.write8(f + 30, n);
                cpu.write32(f + 8, 0);
                cpu.write16(f + 44, 0x8000);
                cpu.write16(f + 46, 8);
                cpu.write16(f + 48, 0x50);
                let y = cpu.read32(o + 24).wrapping_sub(0xc_0000);
                cpu.write32(f + 4, y);
                if level > 2 {
                    let x = cpu.read32(o + 20).wrapping_sub(0xa_0000);
                    cpu.write32(f, x);
                    nested!(cpu, 0x0605_5b5c, bullets::rocket_set(cpu, f));
                    let x = cpu.read32(o + 20).wrapping_add(0xa_0000);
                    cpu.write32(f, x);
                    nested!(cpu, 0x0605_5b5c, bullets::rocket_set(cpu, f));
                } else {
                    let x = cpu.read32(o + 20);
                    cpu.write32(f, x);
                    nested!(cpu, 0x0605_5b5c, bullets::rocket_set(cpu, f));
                }
            }
            nested!(cpu, 0x0608_e210, smoke_from(cpu, o));
        }
        cpu.write8(o + O_MODE, MODE_DONE);
        return;
    }
    match cpu.read8(o + KT_STATE) {
        0 => {
            let wait = cpu.read16(o + KT_WAIT).wrapping_sub(1);
            cpu.write16(o + KT_WAIT, wait);
            if (wait as i16) < 0 {
                let state = cpu.read8(o + KT_STATE).wrapping_add(1);
                cpu.write8(o + KT_STATE, state);
                cpu.write32(o + 0x74, 0);
                cpu.write32(o + 0x78, 0);
                cpu.write32(o + KT_AIM_Y, 0xffb0_0000);
                let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x10_0000));
                cpu.write32(o + KT_AIM_X, r);
                cpu.write32(o + 0x80, 0);
                cpu.write16(o + KT_SWING, 0);
                cpu.write16(o + KT_REACH, 0);
                cpu.write32(o + O_SCALE, 0);
            }
        }
        1 => {
            cpu.write32(f + 64, KT_SIDE);
            let side = cpu.read8(o + KT_SIDE) as i8 as i32 as u32;
            let swing = side.wrapping_mul(0x180).wrapping_add(cpu.read16(o + KT_SWING) as u32);
            cpu.write16(o + KT_SWING, swing as u16);
            let from = cpu.read16(o + KT_REACH) as u32;
            let reach = nested!(cpu, 0x0604_6168, actors::approach(cpu, from, 0x60, 1));
            cpu.write16(o + KT_REACH, reach as u16);
            let from = cpu.read32(o + O_SCALE);
            let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, from, 0x1_0000, 0x1000));
            cpu.write32(o + O_SCALE, v);
            let a = signed_word(cpu, o + KT_SWING);
            let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
            let reach = cpu.read16(o + KT_REACH) as u32;
            let x = sin.wrapping_mul(reach).wrapping_add(cpu.read32(o + KT_AIM_X));
            cpu.write32(o + 20, x);
            let a = signed_word(cpu, o + KT_SWING);
            let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
            let reach = (cpu.read16(o + KT_REACH) as u32) >> 1;
            let y = cos.wrapping_neg().wrapping_mul(reach).wrapping_add(cpu.read32(o + KT_AIM_Y));
            cpu.write32(o + 24, y);
            let side = cpu.read8(o + KT_SIDE) as i8 as i32 as u32;
            let turn = cpu.read16(o + KT_SWING).wrapping_add((side << 8 >> 2 << 8) as u16);
            cpu.write16(o + 0x2c, turn);
            if cpu.read32(o + 24) as i32 > 0x28_0000 {
                let state = cpu.read8(o + KT_STATE).wrapping_add(1);
                cpu.write8(o + KT_STATE, state);
                let t = cpu.read16(o + 0x2c);
                cpu.write16(o + O_ANGLE, t);
                cpu.write16(o + 0x2c, 0);
            }
        }
        2 => {
            let frames = cpu.read32(GAME_FRAMES);
            if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, frames, 80)) == 0 {
                let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0xa0_0000));
                cpu.write32(o + KT_AIM_X, r);
            }
            let dy = cpu.read32(o + KT_AIM_Y).wrapping_sub(cpu.read32(o + 24));
            let dx = cpu.read32(o + KT_AIM_X).wrapping_sub(cpu.read32(o + 20));
            let a = nested!(cpu, 0x0606_780c, sgl::sl_atan(cpu, dx, dy));
            let to = (a as i16 as i32 as u32).wrapping_add(0x4000);
            let from = signed_word(cpu, o + O_ANGLE);
            let d = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, from, to));
            let q = nested!(cpu, 0x0600_6a78, math::quotient(cpu, d, 32));
            let t = (q as u16).wrapping_add(cpu.read16(o + O_ANGLE));
            cpu.write16(o + O_ANGLE, t);
            let a = signed_word(cpu, o + O_ANGLE);
            let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
            let vx = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, sin, 0x2_4000));
            cpu.write32(o + 0x74, vx);
            let a = signed_word(cpu, o + O_ANGLE);
            let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
            let vy = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, cos.wrapping_neg(), 0x1_e000));
            cpu.write32(o + 0x78, vy);
            let x = cpu.read32(o + 20).wrapping_add(cpu.read32(o + 0x74));
            cpu.write32(o + 20, x);
            let y = cpu.read32(o + 24).wrapping_add(cpu.read32(o + 0x78));
            cpu.write32(o + 24, y);
            let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
            let v = cpu.read32(o + KT_AIM_Y).wrapping_add(0x3000).wrapping_add(r & 0x7fff);
            cpu.write32(o + KT_AIM_Y, v);
        }
        _ => {}
    }
    let v = cpu.read32(o + KT_AIM_Y).wrapping_add(0x4000);
    cpu.write32(o + KT_AIM_Y, v);
    if cpu.read16(o + FL_HIT_BITS) & 0x200 != 0 {
        let v = cpu.read16(o + 0x6e) | 1;
        cpu.write16(o + 0x6e, v);
    }
    let z = cpu.read32(o + 28).wrapping_add(0x8_0000);
    if z > 0x10_0000 {
        let f = cpu.read32(o + O_FLAGS) | 0x8_0000;
        cpu.write32(o + O_FLAGS, f);
    } else {
        let f = cpu.read32(o + O_FLAGS) & 0xfff7_ffff;
        cpu.write32(o + O_FLAGS, f);
    }
    let v = cpu.read32(o + O_SCALE);
    cpu.write32(o + O_SCALE_2, v);
}

/// A kite's draw (0x0608eda4, its sprite `s`): the flags' 2 goes and is set again from the 1; the
/// matrix pushed, moved to its place, scaled by +0x20, +0x24 and +0x20, turned about x by its
/// +0x2c and about z by its +0x30 half about, and the model of its kind-65 list by its number
/// (+0x95) and the flag 1 put.
pub fn kite_draw(cpu: &mut Cpu, s: u32) {
    let o = s.wrapping_sub(0x10);
    let at = s + 94;
    let w = cpu.read16(at);
    if w & 2 != 0 {
        cpu.write16(at, w & 0xfffc);
    }
    let w = cpu.read16(at);
    if w & 1 != 0 {
        cpu.write16(at, w | 2);
    }
    nested!(cpu, 0x0606_71cc, sgl::sl_push_matrix(cpu));
    let (x, y, z) = (cpu.read32(o + 20), cpu.read32(o + 24), cpu.read32(o + 28));
    nested!(cpu, 0x0606_721c, sgl::sl_translate(cpu, x, y, z));
    let (a, b) = (cpu.read32(o + 32), cpu.read32(o + 36));
    nested!(cpu, 0x0606_73ec, sgl::sl_scale(cpu, a, b, a));
    let a = signed_word(cpu, s + 28);
    nested!(cpu, 0x0606_72ec, sgl::sl_rot_x(cpu, a));
    let a = cpu.read16(s + 32).wrapping_add(0x8000) as i16 as i32 as u32;
    nested!(cpu, 0x0606_726c, sgl::sl_rot_z(cpu, a));
    let list = cpu.read32(kind(65));
    let list = cpu.read32(list + 28);
    let w = cpu.read16(at) as u32;
    let n = cpu.read8(o + 0x95) as u32;
    let entry = cpu.read32(list + 4 * n);
    let model = cpu.read32(entry + 4 * (w & 1));
    nested!(cpu, 0x0606_745c, sgl::sl_put_polygon(cpu, model));
    nested!(cpu, 0x0606_7200, sgl::sl_pop_matrix(cpu));
}

/// A kite made (0x0608ee74) for the owner `owner` as its `n`th (the signed byte `n`, at +0x94;
/// the wait +0x70 18 times it, the side +0x8c +1 on odd ones and -1 on even) with the kind
/// number `which` (+0x95 is 8 times it plus 7): an object of list 2, 0x98 bytes, step
/// 0x0608ea08, draw 0x0608eda4, kind 65, flags 0xf80400, script 0x060acd2c, power 200, at x 0,
/// y -256.0.
pub fn kite_make(cpu: &mut Cpu, owner: u32, n: u32, which: u32) {
    let (n, which) = (n as u8 as i8 as i32 as u32, which & 0xff);
    let number = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, 2)) as i8;
    if number == -1 {
        return;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0x98)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, PART_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + 0x3c));
    cpu.write32(o + O_BEHAVIOUR, 0x0608_ea08);
    cpu.write8(o + O_NUMBER, number as u8);
    let at = cpu.read8(o + O_NUMBER) as i8 as i32 as u32;
    cpu.write32(NUMBERED.wrapping_add(at << 2), o);
    cpu.write32(o + O_DRAW, 0x0608_eda4);
    cpu.write32(o + O_FLAGS, 0x00f8_0400);
    cpu.write8(o + O_KIND, 65);
    cpu.write8(o + O_FRAME, 0);
    cpu.write32(o + O_SCRIPT, 0x060a_cd2c);
    cpu.write8(o + O_54, 1);
    cpu.write8(o + O_55, 18);
    cpu.write32(o + 0x50, 200);
    cpu.write32(o + 20, 0);
    cpu.write32(o + 24, 0xff00_0000);
    cpu.write32(o + 28, 0);
    cpu.write32(o + KT_OWNER, owner);
    cpu.write8(o + 0x94, n as u8);
    cpu.write8(o + KT_STATE, 0);
    cpu.write16(o + 0x6e, 0);
    cpu.write16(o + KT_WAIT, n.wrapping_mul(18) as u16);
    cpu.write32(o + 0x74, 0);
    cpu.write32(o + 0x78, 0);
    cpu.write8(o + KT_SIDE, if n & 1 == 0 { 0xff } else { 1 });
    cpu.write8(o + 0x95, (which * 8 + 7) as u8);
}

/// What a swooper keeps: its owner (+0x8c, whose +0xbc says it still fires), its state (+0x6c: 0
/// waits, 1 and 4 grow in, 2 and 5 chase, 3 shrinks out), its wait (+0x70), its velocity (+0x74,
/// +0x78), the target (+0x7c, +0x80), the base y (+0x84) and its speed (+0x88).
const SW_OWNER: u32 = 0x8c;
const SW_STATE: u32 = 0x6c;
const SW_WAIT: u32 = 0x70;
const SW_VX: u32 = 0x74;
const SW_VY: u32 = 0x78;
const SW_AIM_X: u32 = 0x7c;
const SW_AIM_Y: u32 = 0x80;
const SW_BASE_Y: u32 = 0x84;
const SW_SPEED: u32 = 0x88;

/// A swooper put at a random place (up to 128.0 across, within 32.0 of y 0) with its base y at
/// -48.0, its velocity and aim cleared (the same in the first state and after the third).
fn swoop_respawn(cpu: &mut Cpu, o: u32) {
    cpu.write32(o + SW_VX, 0);
    cpu.write32(o + SW_VY, 0);
    cpu.write32(o + 0x7c, 0);
    cpu.write32(o + SW_AIM_Y, 0);
    cpu.write32(o + SW_SPEED, 0);
    cpu.write32(o + SW_BASE_Y, 0xffd0_0000);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x80_0000));
    cpu.write32(o + 20, r);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x20_0000));
    cpu.write32(o + 24, r.wrapping_sub(0x20_0000));
}

/// A swooper's step (0x0608efb4). Past y 160.0 it is done (mode 0x70); with its power gone or its
/// owner not firing it is done with a burst (0x060549f8, a struct at its place with its number,
/// 2.0, 180 and 0x2000) and sound 16 if the owner fires, and smoke. Otherwise by its state: 0
/// counts its wait (+0x70) down and then appears (swoop_respawn, scale 0, second scale 4.0); 1
/// and 4 grow (scales +0x1000 and -0x3000, turns +0x1000) until the scale is 1.0 (the state goes
/// on and its flag 0x80000 off); 2 and 5 every 50th frame pick a target (random up to 144.0 across
/// and 64.0 around the base y), turn toward it by a 16th of the difference, ease the speed
/// (+0x88) to 1.25 by 0x1000 and move by it (the y by 0.75 of the speed), the base y down by
/// 0x8000 (state 2 goes on at power 399 and below, flag 0x80000 on); 3 shrinks and, at scale 0,
/// appears again (state 4). Last the second turn (+0x2e) is minus twice the turn, a hit bit sets
/// the flag 1 and the velocity moves it.
pub fn swoop_step(cpu: &mut Cpu, o: u32) {
    let f = cpu.stack();
    let owner = cpu.read32(o + SW_OWNER);
    let y = cpu.read32(o + 24) as i32;
    let carry_on = y <= 0xa0_0000 && cpu.read32(o + 0x50) as i32 > 0 && cpu.read32(owner + 0xbc) as i32 > 0;
    if !carry_on {
        if cpu.read32(o + 24) as i32 <= 0xa0_0000 {
            if cpu.read32(owner + 0xbc) != 0 {
                let (x, y) = (cpu.read32(o + 20), cpu.read32(o + 24));
                cpu.write32(f, x);
                cpu.write32(f + 4, y);
                cpu.write32(f + 8, 0);
                let n = cpu.read8(o + O_NUMBER);
                cpu.write8(f + 30, n);
                cpu.write32(f + 56, 0x2_0000);
                cpu.write16(f + 46, 0xb4);
                cpu.write16(f + 50, 0x2000);
                cpu.write16(f + 44, 0);
                cpu.write16(f + 52, 0);
                nested!(cpu, 0x0605_49f8, bullets::missile_set(cpu, f));
                nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 16));
            }
            nested!(cpu, 0x0608_e210, smoke_from(cpu, o));
        }
        cpu.write8(o + O_MODE, MODE_DONE);
        return;
    }
    let state = cpu.read8(o + SW_STATE);
    match state {
        0 => {
            let wait = cpu.read16(o + SW_WAIT).wrapping_sub(1);
            cpu.write16(o + SW_WAIT, wait);
            if (wait as i16) < 0 {
                let s = cpu.read8(o + SW_STATE).wrapping_add(1);
                cpu.write8(o + SW_STATE, s);
                swoop_respawn(cpu, o);
                cpu.write32(o + O_SCALE, 0);
                cpu.write32(o + O_SCALE_2, 0x4_0000);
            }
        }
        1 | 4 => {
            let v = cpu.read32(o + O_SCALE).wrapping_add(0x1000);
            cpu.write32(o + O_SCALE, v);
            let v = cpu.read32(o + O_SCALE_2).wrapping_sub(0x3000);
            cpu.write32(o + O_SCALE_2, v);
            let v = cpu.read16(o + 0x2c).wrapping_add(0x1000);
            cpu.write16(o + 0x2c, v);
            let v = cpu.read16(o + O_ANGLE).wrapping_add(0x1000);
            cpu.write16(o + O_ANGLE, v);
            if cpu.read32(o + O_SCALE) == 0x1_0000 {
                let s = cpu.read8(o + SW_STATE).wrapping_add(1);
                cpu.write8(o + SW_STATE, s);
                let fl = cpu.read32(o + O_FLAGS) & 0xfff7_ffff;
                cpu.write32(o + O_FLAGS, fl);
            }
            let v = cpu.read32(o + SW_VY).wrapping_sub(0x100);
            cpu.write32(o + SW_VY, v);
        }
        2 | 5 => {
            let frames = cpu.read32(GAME_FRAMES);
            if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, frames, 50)) == 0 {
                let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x90_0000));
                cpu.write32(o + SW_AIM_X, r);
                let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x40_0000));
                let base = cpu.read32(o + SW_BASE_Y);
                cpu.write32(o + SW_AIM_Y, r.wrapping_add(base));
            }
            let dy = cpu.read32(o + SW_AIM_Y).wrapping_sub(cpu.read32(o + 24));
            let dx = cpu.read32(o + SW_AIM_X).wrapping_sub(cpu.read32(o + 20));
            let a = nested!(cpu, 0x0606_780c, sgl::sl_atan(cpu, dx, dy));
            let to = (a as i16 as i32 as u32).wrapping_add(0x4000);
            let from = signed_word(cpu, o + O_ANGLE);
            let d = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, from, to));
            let q = nested!(cpu, 0x0600_6a78, math::quotient(cpu, d, 16));
            let t = (q as u16).wrapping_add(cpu.read16(o + O_ANGLE));
            cpu.write16(o + O_ANGLE, t);
            let from = cpu.read32(o + SW_SPEED);
            let speed = nested!(cpu, 0x0604_6168, actors::approach(cpu, from, 0x1_4000, 0x1000));
            cpu.write32(o + SW_SPEED, speed);
            let a = signed_word(cpu, o + O_ANGLE);
            let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
            let vx = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, sin, speed));
            cpu.write32(o + SW_VX, vx);
            let speed = cpu.read32(o + SW_SPEED);
            let slow = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, speed, 0xc000));
            let a = signed_word(cpu, o + O_ANGLE);
            let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
            let vy = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, cos.wrapping_neg(), slow));
            cpu.write32(o + SW_VY, vy);
            let x = cpu.read32(o + 20).wrapping_add(cpu.read32(o + SW_VX));
            cpu.write32(o + 20, x);
            let y = cpu.read32(o + 24).wrapping_add(cpu.read32(o + SW_VY));
            cpu.write32(o + 24, y);
            let base = cpu.read32(o + SW_BASE_Y).wrapping_add(0x8000);
            cpu.write32(o + SW_BASE_Y, base);
            let s = cpu.read8(o + SW_STATE);
            if s == 2 && cpu.read32(o + 0x50) as i32 <= 399 {
                cpu.write8(o + SW_STATE, s + 1);
                let fl = cpu.read32(o + O_FLAGS) | 0x8_0000;
                cpu.write32(o + O_FLAGS, fl);
            }
        }
        3 => {
            let v = cpu.read32(o + O_SCALE).wrapping_sub(0x1000);
            cpu.write32(o + O_SCALE, v);
            let v = cpu.read32(o + O_SCALE_2).wrapping_add(0x3000);
            cpu.write32(o + O_SCALE_2, v);
            let v = cpu.read16(o + 0x2c).wrapping_sub(0x1000);
            cpu.write16(o + 0x2c, v);
            let v = cpu.read16(o + O_ANGLE).wrapping_sub(0x1000);
            cpu.write16(o + O_ANGLE, v);
            if cpu.read32(o + O_SCALE) == 0 {
                let s = cpu.read8(o + SW_STATE).wrapping_add(1);
                cpu.write8(o + SW_STATE, s);
                swoop_respawn(cpu, o);
            }
        }
        _ => {}
    }
    let turn = cpu.read16(o + O_ANGLE);
    cpu.write16(o + O_KIND, (turn as i16).wrapping_neg().wrapping_mul(2) as u16);
    if cpu.read16(o + FL_HIT_BITS) & 0x200 != 0 {
        let v = cpu.read16(o + 0x6e) | 1;
        cpu.write16(o + 0x6e, v);
    }
    let x = cpu.read32(o + 20).wrapping_add(cpu.read32(o + SW_VX));
    cpu.write32(o + 20, x);
    let y = cpu.read32(o + 24).wrapping_add(cpu.read32(o + SW_VY));
    cpu.write32(o + 24, y);
}

/// A swooper's draw (0x0608f380, its sprite `s`): as the kite's but turned about z (+0x30 half
/// about), y and x, and the model of its kind-65 list by its number (+0x91).
pub fn swoop_draw(cpu: &mut Cpu, s: u32) {
    let o = s.wrapping_sub(0x10);
    let at = s + 94;
    let w = cpu.read16(at);
    if w & 2 != 0 {
        cpu.write16(at, w & 0xfffc);
    }
    let w = cpu.read16(at);
    if w & 1 != 0 {
        cpu.write16(at, w | 2);
    }
    nested!(cpu, 0x0606_71cc, sgl::sl_push_matrix(cpu));
    let (x, y, z) = (cpu.read32(o + 20), cpu.read32(o + 24), cpu.read32(o + 28));
    nested!(cpu, 0x0606_721c, sgl::sl_translate(cpu, x, y, z));
    let (a, b) = (cpu.read32(o + 32), cpu.read32(o + 36));
    nested!(cpu, 0x0606_73ec, sgl::sl_scale(cpu, a, b, a));
    let a = cpu.read16(s + 32).wrapping_add(0x8000) as i16 as i32 as u32;
    nested!(cpu, 0x0606_726c, sgl::sl_rot_z(cpu, a));
    let a = signed_word(cpu, s + 30);
    nested!(cpu, 0x0606_736c, sgl::sl_rot_y(cpu, a));
    let a = signed_word(cpu, s + 28);
    nested!(cpu, 0x0606_72ec, sgl::sl_rot_x(cpu, a));
    let list = cpu.read32(kind(65));
    let list = cpu.read32(list + 28);
    let w = cpu.read16(at) as u32;
    let n = cpu.read8(o + 0x91) as u32;
    let entry = cpu.read32(list + 4 * n);
    let model = cpu.read32(entry + 4 * (w & 1));
    nested!(cpu, 0x0606_745c, sgl::sl_put_polygon(cpu, model));
    nested!(cpu, 0x0606_7200, sgl::sl_pop_matrix(cpu));
}

// ==== from 0x06094000 on ===========================================================================
// (A second STG03 worktree, wip-stg03b, writes the functions from 0x06094000 below this line while
// wip-stg03 writes the ones before it above.)

// ---- the thruster craft's draw -------------------------------------------------------------------

/// The object's frame flags (+0xaa: bit 0 picks the polygon data of a pair, bit 1 is cleared and
/// set again from it), its fin turn (+0xa4), its flame count (+0x94), its glow (+0x98), its model
/// corners' table (8 points, each of three halfwords with a halfword after, read from here) and
/// where their turned places go (+0xb4, 12 bytes each).
const CRAFT_FLAGS: u32 = 0xaa;
const CRAFT_FIN: u32 = 0xa4;
const CRAFT_FLAME: u32 = 0x94;
const CRAFT_GLOW: u32 = 0x98;
const CRAFT_CORNERS: u32 = 0x060a_ce20;
const CRAFT_POINTS: u32 = 0xb4;
/// The polygon data sets (the stage's data table, kept by the kinds, +0x1c's table: its entries
/// +0x2c to +0x3c, two a set by the flags' bit 0).
const CRAFT_DATA: u32 = 0x0607_41f0 + 0x430;
const CRAFT_SETS: [u32; 5] = [0x2c, 0x30, 0x34, 0x38, 0x3c];
/// The craft's sprite struct's own locals past the struct: two thruster places and the nose's.
const CRAFT_THRUSTER_1: u32 = 44;
const CRAFT_THRUSTER_2: u32 = 56;
const CRAFT_NOSE: u32 = 68;

fn craft_polygons(cpu: &mut Cpu, o: u32, set: u32) {
    let data = cpu.read32(CRAFT_DATA);
    let table = cpu.read32(data + 0x1c);
    let sets = cpu.read32(table + set);
    let flags = cpu.read16(o + CRAFT_FLAGS) as i16 as i32 as u32;
    let pd = cpu.read32(sets + ((flags & 1) << 2));
    nested!(cpu, 0x0606_745c, sgl::sl_put_polygon(cpu, pd));
}

/// A craft's draw (its sprite `s`): the frame flags' bit 1 cleared and set again where bit 0 is;
/// its body drawn under its place, scale 1.5 and turns; its eight model corners turned to the
/// places kept in the object; the two wings (a polygon set each, the second pair turned by the fin),
/// each with a thruster place (frame count mod 3 times 2, plus 16, ahead of the wing); a nose place
/// when flaming. Flaming (+0x94): a ring of four sprites once under 60 (frame 0x70 up by 20ths of
/// the count, size by it), a glow (+0x98) and ten trailing flames; then the thrusters' sprites
/// (frame by the loop count mod 3).
pub fn craft_draw(cpu: &mut Cpu, s: u32) {
    let o = s.wrapping_sub(0x10);
    let mut flags = cpu.read16(o + CRAFT_FLAGS) as i16 as i32 as u32;
    if flags & 2 != 0 {
        cpu.write16(o + CRAFT_FLAGS, (flags & 0xffff_fffc) as u16);
        flags = cpu.read16(o + CRAFT_FLAGS) as i16 as i32 as u32;
    }
    if flags & 1 != 0 {
        cpu.write16(o + CRAFT_FLAGS, (flags | 2) as u16);
    }
    nested!(cpu, 0x0606_71cc, sgl::sl_push_matrix(cpu));
    let (x, y, z) = (cpu.read32(s + 4), cpu.read32(s + 8), cpu.read32(s + 12));
    nested!(cpu, 0x0606_721c, sgl::sl_translate(cpu, x, y, z));
    nested!(cpu, 0x0606_73ec, sgl::sl_scale(cpu, 0x1_8000, 0x1_8000, 0x1_8000));
    let turn_z = (cpu.read16(s + 0x20) as u32).wrapping_add(0xffff_8000) as i16 as i32 as u32;
    nested!(cpu, 0x0606_726c, sgl::sl_rot_z(cpu, turn_z));
    let turn_x = cpu.read16(s + 0x1c) as i16 as i32 as u32;
    nested!(cpu, 0x0606_72ec, sgl::sl_rot_x(cpu, turn_x));
    let turn_y = cpu.read16(s + 0x1e) as i16 as i32 as u32;
    nested!(cpu, 0x0606_736c, sgl::sl_rot_y(cpu, turn_y));
    craft_polygons(cpu, o, CRAFT_SETS[0]);
    for k in 0..8 {
        let at = CRAFT_CORNERS + 8 * k;
        let x = (cpu.read16(at) as i16 as i32 as u32) << 16;
        let y = (cpu.read16(at + 2) as i16 as i32 as u32) << 16;
        let z = (cpu.read16(at + 4) as i16 as i32 as u32) << 16;
        nested!(cpu, 0x0606_db2c, sgl::sl_calc_point(cpu, x, y, z, o + CRAFT_POINTS + 12 * k));
    }
    let d = cpu.stack();
    // the wings
    nested!(cpu, 0x0606_71cc, sgl::sl_push_matrix(cpu));
    nested!(cpu, 0x0606_721c, sgl::sl_translate(cpu, 0xfffd_0000, 0, 0));
    craft_polygons(cpu, o, CRAFT_SETS[1]);
    nested!(cpu, 0x0606_7200, sgl::sl_pop_matrix(cpu));
    nested!(cpu, 0x0606_71cc, sgl::sl_push_matrix(cpu));
    nested!(cpu, 0x0606_721c, sgl::sl_translate(cpu, 0x3_0000, 0, 0));
    craft_polygons(cpu, o, CRAFT_SETS[2]);
    nested!(cpu, 0x0606_7200, sgl::sl_pop_matrix(cpu));
    // the fins, with a thruster place each
    nested!(cpu, 0x0606_71cc, sgl::sl_push_matrix(cpu));
    nested!(cpu, 0x0606_721c, sgl::sl_translate(cpu, 0x15_0000, 0, 0x2_0000));
    let fin = cpu.read16(o + CRAFT_FIN) as i16 as i32 as u32;
    nested!(cpu, 0x0606_72ec, sgl::sl_rot_x(cpu, fin));
    craft_polygons(cpu, o, CRAFT_SETS[3]);
    let m = craft_loops_mod_3(cpu);
    let z = (m << 16).wrapping_mul(2).wrapping_add(0x10_0000);
    nested!(cpu, 0x0606_db2c, sgl::sl_calc_point(cpu, 0xfffd_0000, 0, z, d + CRAFT_THRUSTER_1));
    nested!(cpu, 0x0606_7200, sgl::sl_pop_matrix(cpu));
    nested!(cpu, 0x0606_71cc, sgl::sl_push_matrix(cpu));
    nested!(cpu, 0x0606_721c, sgl::sl_translate(cpu, 0xffeb_0000, 0, 0x2_0000));
    let fin = cpu.read16(o + CRAFT_FIN) as i16 as i32 as u32;
    nested!(cpu, 0x0606_72ec, sgl::sl_rot_x(cpu, fin));
    craft_polygons(cpu, o, CRAFT_SETS[4]);
    let m = craft_loops_mod_3(cpu);
    let z = (m << 16).wrapping_mul(2).wrapping_add(0x10_0000);
    nested!(cpu, 0x0606_db2c, sgl::sl_calc_point(cpu, 0x3_0000, 0, z, d + CRAFT_THRUSTER_2));
    nested!(cpu, 0x0606_7200, sgl::sl_pop_matrix(cpu));
    let flame = cpu.read8(o + CRAFT_FLAME) as u32;
    if flame != 0 {
        nested!(cpu, 0x0606_db2c, sgl::sl_calc_point(cpu, 0, 0x78_0000, 0, d + CRAFT_NOSE));
    }
    nested!(cpu, 0x0606_7200, sgl::sl_pop_matrix(cpu));
    if flame != 0 {
        craft_flames(cpu, s, d);
    }
    craft_thrusters(cpu, d);
}

/// The flaming craft's sprites (see `craft_draw`; `d` the struct on the stack).
fn craft_flames(cpu: &mut Cpu, s: u32, d: u32) {
    let o = s.wrapping_sub(0x10);
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, d));
    cpu.write32(d + S_POSITION + 8, 0);
    cpu.write8(d + S_LIFT, 0x7e);
    cpu.write8(d + S_KIND, 0);
    let flame = cpu.read8(o + CRAFT_FLAME) as u32;
    if flame <= 59 {
        let base = cpu.read16(CHAR_BASE);
        cpu.write16(d + S_COLOUR, base);
        let left = 60 - flame;
        let q = nested!(cpu, 0x0600_6a78, math::quotient(cpu, left, 20));
        cpu.write8(d + S_FRAME, (q as u8).wrapping_add(0x70));
        cpu.write32(d + S_SCALES + 4, left.wrapping_mul(0x1000));
        let turn = cpu.read16(s + 0x20) as i16 as i32 as u32;
        let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, turn));
        let x = (sin as i32).wrapping_mul(24).wrapping_add(cpu.read32(s + 4) as i32) as u32;
        cpu.write32(d + S_POSITION, x);
        let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, turn));
        let y = (cos as i32).wrapping_neg().wrapping_mul(24).wrapping_add(cpu.read32(s + 8) as i32) as u32;
        cpu.write32(d + S_POSITION + 4, y);
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
        cpu.write16(d + CRAFT_SPIN, r as u16);
        for _ in 0..4 {
            nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
            let t = cpu.read16(d + CRAFT_SPIN).wrapping_add(0x4000);
            cpu.write16(d + CRAFT_SPIN, t);
        }
    }
    let glow = cpu.read32(o + CRAFT_GLOW);
    if glow == 0 {
        return;
    }
    cpu.write32(d + S_FLAGS, 0);
    if (cpu.read32(o + CRAFT_GLOW) as i32) < 0x3000 {
        cpu.write32(d + S_FLAGS, 0x100);
    }
    cpu.write8(d + S_FRAME, 0xaf);
    let base = cpu.read16(CHAR_BASE);
    let colour = if cpu.read32(LOOPS) & 2 == 0 { base.wrapping_add(0x10) } else { base };
    cpu.write16(d + S_COLOUR, colour);
    let size = cpu.read32(o + CRAFT_GLOW) as i32;
    let size = size.wrapping_add((cpu.read32(LOOPS) & 3) as i32 * ((if size < 0 { size + 3 } else { size }) >> 2)) as u32;
    cpu.write32(d + S_SCALES, size);
    cpu.write32(d + S_SCALES + 4, 0xd_0000);
    let turn = cpu.read16(s + 0x20);
    cpu.write16(d + CRAFT_SPIN, turn & 0xc000);
    for k in 0..2 {
        let v = cpu.read32(d + CRAFT_NOSE + 4 * k);
        cpu.write32(d + S_POSITION + 4 * k, v);
    }
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
    cpu.write8(d + S_FRAME, 1);
    let turn = cpu.read16(s + 0x20) as i16 as i32 as u32;
    let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, turn));
    let x = (sin as i32).wrapping_mul(24).wrapping_add(cpu.read32(s + 4) as i32) as u32;
    cpu.write32(d + S_POSITION, x);
    let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, turn));
    let y = (cos as i32).wrapping_neg().wrapping_mul(24).wrapping_add(cpu.read32(s + 8) as i32) as u32;
    cpu.write32(d + S_POSITION + 4, y);
    let size = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, size, 0xc000));
    cpu.write32(d + S_SCALES, size);
    cpu.write32(d + S_SCALES + 4, size);
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
    let size = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, size, 0x1_a000));
    cpu.write32(d + S_SCALES, size);
    cpu.write32(d + S_SCALES + 4, 0x1_4000);
    cpu.write32(d + S_FLAGS, 0x100);
    cpu.write8(d + S_FRAME, 0xc6);
    let mut behind = 0u32;
    for _ in 0..10 {
        let reach = ((cpu.read32(LOOPS).wrapping_mul(3)) & 0x1f).wrapping_add(behind).wrapping_add(0x20) as i32;
        let turn = cpu.read16(s + 0x20) as i16 as i32 as u32;
        let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, turn));
        let x = (sin as i32).wrapping_mul(reach).wrapping_add(cpu.read32(s + 4) as i32) as u32;
        cpu.write32(d + S_POSITION, x);
        let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, turn));
        let y = (cos as i32).wrapping_neg().wrapping_mul(reach).wrapping_add(cpu.read32(s + 8) as i32) as u32;
        cpu.write32(d + S_POSITION + 4, y);
        nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
        behind += 0x20;
    }
}

/// The thrusters' sprites: a fresh struct, size by the loop count (mod 3 times 0x2000, plus a
/// random 0-15 times 0x300 and 0x1000), kind 0, frame by the loop count mod 3 (0: colour
/// 0x30 on the character base; 1: 0x20, frame 1; 2: 0x100 flag, 0x70, frame 1), at each thruster
/// place and 0xb00000 forward.
fn craft_thrusters(cpu: &mut Cpu, d: u32) {
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, d));
    let m = craft_loops_mod_3(cpu);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    let size = (m << 13).wrapping_add((r & 15) * 0x300).wrapping_add(0x1000);
    cpu.write32(d + S_SCALES, size);
    cpu.write32(d + S_SCALES + 4, size);
    cpu.write8(d + S_KIND, 0);
    cpu.write8(d + S_LIFT, 0);
    let m = craft_loops_mod_3(cpu);
    let base = cpu.read16(CHAR_BASE);
    match m as i32 {
        0 => {
            cpu.write16(d + S_COLOUR, base.wrapping_add(0x30));
            cpu.write8(d + S_FRAME, 0);
        }
        1 => {
            cpu.write16(d + S_COLOUR, base.wrapping_add(0x20));
            cpu.write8(d + S_FRAME, 1);
        }
        2 => {
            let f = cpu.read32(d + S_FLAGS) | 0x100;
            cpu.write32(d + S_FLAGS, f);
            cpu.write16(d + S_COLOUR, base.wrapping_add(0x70));
            cpu.write8(d + S_FRAME, 1);
        }
        _ => {}
    }
    for at in [CRAFT_THRUSTER_1, CRAFT_THRUSTER_2] {
        for k in 0..3 {
            let v = cpu.read32(d + at + 4 * k);
            let v = if k == 2 { v.wrapping_add(0xff50_0000) } else { v };
            cpu.write32(d + S_POSITION + 4 * k, v);
        }
        nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
    }
}

const CRAFT_SPIN: u32 = 0x20;

/// The loop count mod 3 (the SH-2 code's call of the divider).
fn craft_loops_mod_3(cpu: &mut Cpu) -> u32 {
    let loops = cpu.read32(LOOPS);
    nested!(cpu, 0x0600_6a6e, math::remainder(cpu, loops, 3))
}

// ---- the thruster craft's maker ------------------------------------------------------------------

const CRAFT_SIZE: u32 = 0x120;
const CRAFT_FLAGS_SET: u32 = 0xc8_0000;
const CRAFT_STEP: u32 = 0x0609_3c08;
const CRAFT_KIND: u8 = 0x43;
const CRAFT_ROUTE_KIND: u8 = 6;
const CRAFT_ANGLE_STEP: u32 = 0xcccd;
const CRAFT_PARTS: u32 = 0x060a_ce20;
static CRAFT_PARTS_PROLOGUE: std::sync::OnceLock<super::Prologue> = std::sync::OnceLock::new();

/// A craft made (`link` kept at +0x8c, `kind` at +0x79): an object of list 2 (0x120 bytes) with
/// the number from the free ones, flags 0xc80000, kind 0x43, the step at 0x06093c08 and the draw
/// at 0x06094284. Kind 6 sits at (0, -0x500000, -0x800000) turned 0x8000; the others are put on a
/// circle of 0x80 round the origin, turned by 0xcccd a kind, and deeper (-0xc00000 less a
/// 0x100000 a kind). The eight parts set up from the table at 0x060ace20 under the flags +0xaa
/// and life 30000 at +0xb0. The number (0 if none was free or no object).
pub fn craft_make(cpu: &mut Cpu, link: u32, kind: u32) -> u32 {
    let kind = kind & 0xff;
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, 2)) as i8;
    if n == -1 {
        return 0;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, CRAFT_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, PART_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + O_SCRIPT));
    cpu.write32(o + O_BEHAVIOUR, CRAFT_STEP);
    let flags = cpu.read32(o + O_FLAGS) | CRAFT_FLAGS_SET;
    cpu.write32(o + O_FLAGS, flags);
    cpu.write32(o + O_DRAW, 0x0609_4284);
    cpu.write8(o + O_KIND, CRAFT_KIND);
    cpu.write8(o + O_NUMBER, n as u8);
    let at = cpu.read8(o + O_NUMBER) as i8 as i32 as u32;
    cpu.write32(NUMBERED.wrapping_add(at << 2), o);
    cpu.write8(o + O_KIND, CRAFT_KIND);
    cpu.write8(o + 0x78, 0);
    cpu.write8(o + 0x79, kind as u8);
    cpu.write16(o + 0x84, 0);
    if cpu.read8(o + 0x79) == CRAFT_ROUTE_KIND {
        cpu.write16(o + O_ANGLE, 0x8000);
        cpu.write32(o + O_POSITION, 0);
        cpu.write32(o + O_POSITION + 4, 0xffb0_0000);
        cpu.write32(o + O_POSITION + 8, 0xff80_0000);
    } else {
        cpu.write16(o + O_ANGLE, kind.wrapping_mul(CRAFT_ANGLE_STEP) as u16);
        let a = cpu.read16(o + O_ANGLE) as i16 as i32 as u32;
        let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a)) as i32;
        cpu.write32(o + O_POSITION, sin.wrapping_mul(0x80) as u32);
        let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a)) as i32;
        cpu.write32(o + O_POSITION + 4, cos.wrapping_mul(-0x80) as u32);
        cpu.write32(o + O_POSITION + 8, 0xff40_0000u32.wrapping_sub(kind << 20));
    }
    cpu.write32(o + 0x8c, link);
    cpu.write16(o + 0x90, 0);
    cpu.write16(o + 0x92, 0xf0);
    cpu.write8(o + CRAFT_FLAME, 0);
    cpu.write32(o + CRAFT_GLOW, 0);
    cpu.write32(o + 0x9c, 0);
    cpu.write32(o + 0xa0, 0);
    cpu.write32(o + 0x7c, 0);
    cpu.write32(o + 0x80, 0);
    cpu.write16(o + 0x88, 0);
    cpu.write16(o + CRAFT_FIN, 0);
    cpu.write16(o + 0xa6, 0);
    cpu.write16(o + 0xa8, 0);
    cpu.write32(o + 0x6c, 0);
    cpu.write32(o + 0x70, 0);
    cpu.write32(o + 0x74, 0);
    cpu.write16(o + CRAFT_FLAGS, 0);
    cpu.write8(o + 0xac, 0);
    cpu.write32(o + 0xb0, 30000);
    let number = cpu.read8(o + O_NUMBER) as u32;
    cpu.nested_with_stack(&CRAFT_PARTS_PROLOGUE, 0x0605_6ea4, &[CRAFT_PARTS, number], |cpu| {
        parts::parts_set(cpu, 8, o + CRAFT_POINTS, o + CRAFT_FLAGS, o + 0xb0, CRAFT_PARTS, number)
    });
    n as i32 as u32
}

// ---- the window ------------------------------------------------------------------------------------

const WIN_ANCHOR: u32 = 0x060b_2090;
const WIN_HURT: u32 = 0x56;
const WIN_HURT_BIT: u16 = 0x400;
const WIN_REACH_X: u32 = 0x74;
const WIN_REACH_Y: u32 = 0x78;
const WIN_LIFT: u32 = 0x7c;
const WIN_PARENT: u32 = 0x6c;
const WIN_PARENT_KIND: u32 = 0x126;
const WIN_KIND: u32 = 0x70;
const WIN_TARGET: u32 = 0x40;
const WIN_BIG: u32 = 0x8_0000;
const WIN_MARGIN: u32 = 0x8_0000;
const WIN_EASE: u32 = 0x4000;

/// The upper 16 bits of a fixed number, rounded toward zero, as a halfword (the SH-2 code adds
/// 0xffff to a negative one, then takes the upper half).
fn window_half(v: u32) -> u16 {
    let v = if (v as i32) < 0 { v.wrapping_add(0xffff) } else { v };
    (v >> 16) as u16
}

/// A window's step (the frame round a gap, widening toward the size its kind asks): its parent
/// ended (parent kind 4) ends it (mode 0x70). Else its place +0x18/+0x1c the anchor's plus its
/// lift and less 0xc0000; the hit boxes (+0x88-+0xa3: bytes and halfwords of the half sizes +0x74
/// and +0x78, signed as they go); where hurt (+0x56's bit 0x400) by one of the two first numbered
/// objects, that object is kept within the window's reach by random jumps; the flag 0x80000 by the
/// depth (past 0x7ffff either side); with it clear, the lift and the sizes eased 0x4000 a call toward
/// those of its kind (+0x70: 0, 1, 2, 3).
pub fn window_step(cpu: &mut Cpu, o: u32) {
    let parent = cpu.read32(o + WIN_PARENT);
    if cpu.read8(parent + WIN_PARENT_KIND) == 4 {
        cpu.write8(o + 3, 0x70);
        return;
    }
    let y = cpu.read32(WIN_ANCHOR + 16).wrapping_add(cpu.read32(o + WIN_LIFT));
    cpu.write32(o + 0x18, y);
    let z = cpu.read32(WIN_ANCHOR + 20).wrapping_add(0xfff4_0000);
    cpu.write32(o + 0x1c, z);
    let rx = cpu.read32(o + WIN_REACH_Y);
    let ry = cpu.read32(o + WIN_REACH_X);
    cpu.write16(o + 0x88, window_half(rx.wrapping_neg()));
    cpu.write8(o + 0x8a, window_half(ry) as u8);
    cpu.write16(o + 0x8e, window_half(ry));
    cpu.write8(o + 0x93, window_half(rx) as u8);
    cpu.write16(o + 0x98, window_half(rx));
    cpu.write8(o + 0x9a, window_half(ry) as u8);
    cpu.write16(o + 0x9e, window_half(ry.wrapping_neg()));
    cpu.write8(o + 0xa3, window_half(rx) as u8);
    if cpu.read16(o + WIN_HURT) & WIN_HURT_BIT != 0 {
        let t = cpu.read32(o + WIN_TARGET);
        if cpu.read32(NUMBERED) == t || cpu.read32(NUMBERED + 4) == t {
            window_keep_in(cpu, o, t);
        }
    }
    let depth = cpu.read32(o + 0x1c) as i32;
    let flags = cpu.read32(o + O_FLAGS);
    let flags = if depth.wrapping_abs() > 0x7_ffff { flags | WIN_BIG } else { flags & !WIN_BIG };
    cpu.write32(o + O_FLAGS, flags);
    if flags & WIN_BIG != 0 {
        return;
    }
    let (lift, x, y) = match cpu.read8(o + WIN_KIND) {
        2 => (0x30_0000, 0x50_0000, 0x38_0000),
        1 => (0x40_0000, 0x98_0000, 0x2c_0000),
        3 => (0, 0x60_0000, 0x40_0000),
        _ => (0x30_0000, 0x98_0000, 0x38_0000),
    };
    let v = cpu.read32(o + WIN_LIFT);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, lift, WIN_EASE));
    cpu.write32(o + WIN_LIFT, v);
    let v = cpu.read32(o + WIN_REACH_X);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, x, WIN_EASE));
    cpu.write32(o + WIN_REACH_X, v);
    let v = cpu.read32(o + WIN_REACH_Y);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, y, WIN_EASE));
    cpu.write32(o + WIN_REACH_Y, v);
}

/// The target `t` pushed back into the window's reach: each of its two places (+0x14, +0x18) set
/// to the edge less 0x80000 a random 0-0x1ffff past the edge, where it is out.
fn window_keep_in(cpu: &mut Cpu, o: u32, t: u32) {
    for (place, reach) in [(0x14, WIN_REACH_X), (0x18, WIN_REACH_Y)] {
        let edge = cpu.read32(o + place);
        let r = cpu.read32(o + reach);
        if (cpu.read32(t + place) as i32) > (edge.wrapping_add(r).wrapping_add(0xfff8_0000)) as i32 {
            let n = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x2_0000));
            let r = cpu.read32(o + reach);
            cpu.write32(t + place, edge.wrapping_add(r).wrapping_add(n.wrapping_add(0xfff6_0000)));
        }
        let r = cpu.read32(o + reach);
        let low = edge.wrapping_sub(r).wrapping_add(WIN_MARGIN) as i32;
        if (cpu.read32(t + place) as i32) < low {
            let n = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x2_0000));
            let r = cpu.read32(o + reach);
            cpu.write32(t + place, edge.wrapping_sub(r).wrapping_add(n.wrapping_add(0xa_0000)));
        }
    }
}

const WIN_MADE_SIZE: u32 = 0xa4;
const WIN_STEP: u32 = 0x0609_4b14;
const WIN_DRAW: u32 = 0x0609_4dd8;
const WIN_EXEC: u32 = 0x0604_d780;
const WIN_BOXES: u32 = 0x060a_d256;
const WIN_BOXES_AT: u32 = 0x84;
const WIN_FLAGS_SET: u32 = 0xc8_0000;
const WIN_START_REACH: [u32; 2] = [0xc0_0000, 0xa0_0000];
const WIN_START_DEPTH: u32 = 0x80_0000;

/// A window made round `parent` (its number copied): an object of list 2 (0xa4 bytes) with flags
/// 0xc80000, the step 0x06094b14 and the draw 0x06094dd8, kind of window 0, reach 0xc00000 by
/// 0xa00000, lift 0, place the origin, its hit boxes copied to +0x84 from 0x060ad256 and the
/// script pointing there, the hit-box numbers 1 and 5. It, or 0.
pub fn window_make(cpu: &mut Cpu, parent: u32) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, WIN_MADE_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, WIN_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + O_SCRIPT));
    cpu.write32(o + O_BEHAVIOUR, WIN_STEP);
    let flags = cpu.read32(o + O_FLAGS) | WIN_FLAGS_SET;
    cpu.write32(o + O_FLAGS, flags);
    cpu.write32(o + O_DRAW, WIN_DRAW);
    let number = cpu.read8(parent + O_NUMBER);
    cpu.write8(o + O_NUMBER, number);
    cpu.write32(o + WIN_PARENT, parent);
    cpu.write8(o + WIN_KIND, 0);
    cpu.write32(o + WIN_REACH_X, WIN_START_REACH[0]);
    cpu.write32(o + WIN_REACH_Y, WIN_START_REACH[1]);
    cpu.write32(o + 0x80, WIN_START_DEPTH);
    cpu.write32(o + WIN_LIFT, 0);
    for k in 0..3 {
        cpu.write32(o + O_POSITION + 4 * k, 0);
    }
    nested!(cpu, 0x0605_6440, hits::boxes_copy(cpu, o + WIN_BOXES_AT, WIN_BOXES));
    cpu.write32(o + O_SCRIPT, o + WIN_BOXES_AT);
    cpu.write8(o + O_54, 1);
    cpu.write8(o + O_55, 5);
    cpu.write16(o + 0x58, 0);
    o
}

// ---- the sparks --------------------------------------------------------------------------------------

const DEBRIS_SPARK_KIND: u8 = 0x27;
const DEBRIS_SPARK_FLAGS: u32 = 3;
const DEBRIS_SPARK_LINK: u32 = 0x060a_d280;
const DEBRIS_SPARK_MODE: u8 = 9;
const DEBRIS_SPARK_OBJ_FLAGS: u32 = 0x500;
const DEBRIS_SPARK_OFFSET: i32 = 24;

/// A spark thrown by the object `o` (its scale +0x20/+0x24 times 24 along its turn, a half turn
/// on, from its place, and its depth): an effect of kind 0x27 with the object's colour (+0x2a),
/// number and turn, its link 0x060ad280. Given mode 9, the flags 0x500, the object's scales, and
/// the push (+0x4c/+0x50) of an eighth of them along its turn.
pub fn debris_spark(cpu: &mut Cpu, o: u32) {
    use effects as fx;
    let e = cpu.stack();
    let x = cpu.read32(o + O_POSITION);
    cpu.write32(e + fx::X, x);
    let y = cpu.read32(o + O_POSITION + 4);
    cpu.write32(e + fx::X + 4, y);
    let side = (cpu.read32(o + O_SCALE) as i32).wrapping_mul(DEBRIS_SPARK_OFFSET) as u32;
    let half = (cpu.read16(o + O_ANGLE).wrapping_add(0x8000)) as i16 as i32 as u32;
    let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, half));
    let v = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, sin, side));
    let x = cpu.read32(e + fx::X).wrapping_add(v);
    cpu.write32(e + fx::X, x);
    let up = (cpu.read32(o + O_SCALE_2) as i32).wrapping_mul(DEBRIS_SPARK_OFFSET) as u32;
    let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, half));
    let v = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, cos.wrapping_neg(), up));
    let y = cpu.read32(e + fx::X + 4).wrapping_add(v);
    cpu.write32(e + fx::X + 4, y);
    let z = cpu.read32(o + O_POSITION + 8);
    cpu.write32(e + fx::X + 8, z);
    cpu.write32(e + fx::FLAGS_LONG, DEBRIS_SPARK_FLAGS);
    let turn = cpu.read16(o + O_ANGLE);
    cpu.write16(e + fx::ANGLE, turn);
    cpu.write16(e + fx::FRAMES, 0);
    cpu.write8(e + fx::FRAMES_2, 0);
    cpu.write8(e + fx::KIND, DEBRIS_SPARK_KIND);
    cpu.write8(e + fx::FRAME, 0);
    let colour = cpu.read16(o + O_COLOUR_B);
    cpu.write16(e + fx::COLOUR, colour);
    let player = cpu.read8(o + O_NUMBER);
    cpu.write8(e + fx::PLAYER, player);
    cpu.write32(e + fx::LINK, DEBRIS_SPARK_LINK);
    cpu.write32(e + fx::RELATIVE_3C, 0);
    let spark = nested!(cpu, 0x0604_e1cc, effects::effect_set(cpu, e));
    if spark == 0 {
        return;
    }
    cpu.write8(spark + 0x33, DEBRIS_SPARK_MODE);
    let flags = cpu.read32(spark + O_FLAGS) | DEBRIS_SPARK_OBJ_FLAGS;
    cpu.write32(spark + O_FLAGS, flags);
    let (sx, sy) = (cpu.read32(o + O_SCALE), cpu.read32(o + O_SCALE_2));
    cpu.write32(spark + O_SCALE, sx);
    cpu.write32(spark + O_SCALE_2, sy);
    let turn = cpu.read16(o + O_ANGLE) as i16 as i32 as u32;
    let eighth = ((sx as i32).wrapping_add(if (sx as i32) < 0 { 7 } else { 0 }) >> 3) as u32;
    let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, turn));
    let v = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, sin.wrapping_neg(), eighth));
    cpu.write32(spark + 0x4c, v);
    let eighth = ((sy as i32).wrapping_add(if (sy as i32) < 0 { 7 } else { 0 }) >> 3) as u32;
    let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, turn));
    let v = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, cos, eighth));
    cpu.write32(spark + 0x50, v);
}

// ---- the debris ----------------------------------------------------------------------------------------

const DIFFICULTY_AT: u32 = 0x0600_ed84;
const DEBRIS_AREA: [u32; 4] = [0xff40_0000, 0xc0_0000, 0xff10_0000, 0xf0_0000];
const DEBRIS_ROUTE_TURN: u32 = 0x8a;
const DEBRIS_SPEED: u32 = 0x8c;
const DEBRIS_SPEEDS: u32 = 0x060a_d294;
const DEBRIS_SPEED_MIN_LEVEL: i8 = 3;
const DEBRIS_SPEED_START: i16 = 4;
const DEBRIS_SCALE_END: u32 = 0x1_0000;
const DEBRIS_SCALE_STEP: u32 = 0x1000;
const DEBRIS_SPARK_EVERY: u32 = 5;
static DEBRIS_AREA_PROLOGUE: std::sync::OnceLock<super::Prologue> = std::sync::OnceLock::new();

/// A piece of debris's step: out of its area (top 0xff400000, bottom 0xc00000, left 0xff100000,
/// right 0xf00000) it ends (mode 0x70). Else, at difficulty 4 and past 4 steps of speed, its turn
/// eased toward the nearer player (the turn by the angle between them, over its turn rate
/// +0x8a, which rises 2 a call); its speed (+0x8c) rises by one to the difficulty's limit; it
/// moves along its turn by the speed; its frame is the game frames mod 3; its scales ease to
/// 0x10000 by 0x1000; its colour alternates; every fifth frame a spark.
pub fn debris_step(cpu: &mut Cpu, o: u32) {
    let [top, bottom, left, right] = DEBRIS_AREA;
    let out = cpu.nested_with_stack(&DEBRIS_AREA_PROLOGUE, 0x0605_69f8, &[right], |cpu| {
        aim::area_flags(cpu, o + O_FLAGS, top, bottom, left, right)
    });
    if out as u8 != 0 {
        cpu.write8(o + O_MODE, MODE_DONE);
        return;
    }
    let level = cpu.read8(DIFFICULTY_AT) as i8;
    if level > DEBRIS_SPEED_MIN_LEVEL && cpu.read16(o + DEBRIS_SPEED) as i16 > DEBRIS_SPEED_START {
        let rate = cpu.read16(o + DEBRIS_ROUTE_TURN) as i16 as i32 as u32;
        let near = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + O_FLAGS));
        let want = nested!(cpu, 0x0604_5e30, aim::angle_to(cpu, o, near)) as i16 as i32 as u32;
        let turn = cpu.read16(o + O_ANGLE) as i16 as i32 as u32;
        let diff = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, turn, want));
        let step = nested!(cpu, 0x0600_6a78, math::quotient(cpu, diff, rate));
        let turn = (step as u16).wrapping_add(cpu.read16(o + O_ANGLE));
        cpu.write16(o + O_ANGLE, turn);
        let rate = cpu.read16(o + DEBRIS_ROUTE_TURN).wrapping_add(2);
        cpu.write16(o + DEBRIS_ROUTE_TURN, rate);
    }
    let level = cpu.read8(DIFFICULTY_AT) as i8 as i32 as u32;
    let speed = cpu.read16(o + DEBRIS_SPEED) as i16;
    let limit = cpu.read8(DEBRIS_SPEEDS.wrapping_add(level)) as i16;
    if speed < limit {
        cpu.write16(o + DEBRIS_SPEED, (speed + 1) as u16);
    }
    let turn = cpu.read16(o + O_ANGLE) as i16 as i32 as u32;
    let speed = cpu.read16(o + DEBRIS_SPEED) as i16 as i32;
    let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, turn)) as i32;
    let x = cpu.read32(o + O_POSITION).wrapping_add(sin.wrapping_mul(speed) as u32);
    cpu.write32(o + O_POSITION, x);
    let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, turn)) as i32;
    let speed = cpu.read16(o + DEBRIS_SPEED) as i16 as i32;
    let y = cpu.read32(o + O_POSITION + 4).wrapping_sub(cos.wrapping_mul(speed) as u32);
    cpu.write32(o + O_POSITION + 4, y);
    let frames = cpu.read32(GAME_FRAMES);
    let m = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, frames, 3));
    cpu.write8(o + O_FRAME, m as u8);
    let scale = cpu.read32(o + O_SCALE);
    let scale = nested!(cpu, 0x0604_6168, actors::approach(cpu, scale, DEBRIS_SCALE_END, DEBRIS_SCALE_STEP));
    cpu.write32(o + O_SCALE, scale);
    cpu.write32(o + O_SCALE_2, scale);
    let base = cpu.read16(CHAR_BASE);
    let colour = if cpu.read32(GAME_FRAMES) & 1 == 0 { base.wrapping_add(0x20) } else { base.wrapping_add(0x30) };
    cpu.write16(o + O_COLOUR_B, colour);
    let frames = cpu.read32(GAME_FRAMES);
    if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, frames, DEBRIS_SPARK_EVERY)) == 0 {
        nested!(cpu, 0x0609_51e8, debris_spark(cpu, o));
    }
}

const DEBRIS_SIZE: u32 = 0xbc;
const DEBRIS_STEP: u32 = 0x0609_5328;
const DEBRIS_KIND: u8 = 0x27;
const DEBRIS_FRAME: u8 = 0xb;
const DEBRIS_BOXES: u32 = 0x060a_d276;
const DEBRIS_PLACE: u32 = 0x154;
const DEBRIS_SOUND: u32 = 0x55;
const DEBRIS_SCALE_START: u32 = 0x4000;

/// A piece of debris made from the object `p` (its turn a half turn on, and the place kept at
/// +0x154/+0x158): an object of list 2 (0xbc bytes) with the step 0x06095328, flags 0x400, mode
/// 8, kind 0x27 frame 11 and the colour 0x20 on the character base, scales 0x4000, turn rate 4;
/// the sound 0x55.
pub fn debris_make(cpu: &mut Cpu, p: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, DEBRIS_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, WIN_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + O_SCRIPT));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    let number = cpu.read8(p + O_NUMBER);
    cpu.write8(o + O_NUMBER, number);
    cpu.write32(o + O_BEHAVIOUR, DEBRIS_STEP);
    cpu.write32(o + O_FLAGS, 0x400);
    cpu.write32(o + 0x1c, 0);
    cpu.write8(o + O_33, 8);
    cpu.write8(o + O_FRAME, DEBRIS_FRAME);
    cpu.write8(o + O_KIND, DEBRIS_KIND);
    let colour = cpu.read16(CHAR_BASE).wrapping_add(0x20);
    cpu.write16(o + O_COLOUR_B, colour);
    cpu.write16(o + O_COLOUR_A, colour);
    cpu.write32(o + O_SCRIPT, DEBRIS_BOXES);
    cpu.write8(o + O_54, 1);
    cpu.write8(o + O_55, 5);
    let turn = cpu.read16(p + O_ANGLE).wrapping_add(0x8000);
    cpu.write16(o + O_ANGLE, turn);
    let x = cpu.read32(p + DEBRIS_PLACE);
    cpu.write32(o + O_POSITION, x);
    let y = cpu.read32(p + DEBRIS_PLACE + 4);
    cpu.write32(o + O_POSITION + 4, y);
    cpu.write32(o + O_SCALE, DEBRIS_SCALE_START);
    cpu.write32(o + O_SCALE_2, DEBRIS_SCALE_START);
    cpu.write16(o + DEBRIS_ROUTE_TURN, 4);
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, DEBRIS_SOUND));
}

// ---- the tail of fire ----------------------------------------------------------------------------------

const TAIL_SIZE: u32 = 0x74;
const TAIL_STEP: u32 = 0x0609_55b8;
const TAIL_FLAGS: u32 = 0x8000_0400;
const TAIL_BOXES: u32 = 0x060a_d29a;
const TAIL_PARENT: u32 = 0x6c;
const TAIL_BACK: u32 = 0x70;
const TAIL_BACK_START: u16 = 0x10;
const TAIL_BACK_STEP: i16 = -0x18;
const TAIL_SCALE_START: u32 = 0x4000;
const TAIL_BIG: i32 = 0x4000;
const TAIL_AREA: [u32; 4] = [0xff50_0000, 0xb0_0000, 0xff20_0000, 0xe0_0000];
const TAIL_SIZE_AT: u32 = 0x160;
static TAIL_AREA_PROLOGUE: std::sync::OnceLock<super::Prologue> = std::sync::OnceLock::new();

/// A tail piece's step: it hangs behind its parent (+0x6c) along the parent's turn, at its
/// distance (+0x70, signed) from the parent's place kept at +0x154/+0x158. While the parent's size
/// (+0x160) is over 0x4000 and the piece in the area (top 0xff500000, bottom 0xb00000, left
/// 0xff200000, right 0xe00000) its scales are that size and its distance shortens by 0x18; else it
/// ends (mode 0x70).
pub fn tail_step(cpu: &mut Cpu, o: u32) {
    let p = cpu.read32(o + TAIL_PARENT);
    let turn = cpu.read16(p + O_ANGLE) as i16 as i32 as u32;
    let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, turn)) as i32;
    let back = cpu.read16(o + TAIL_BACK) as i16 as i32;
    let x = sin.wrapping_mul(back).wrapping_add(cpu.read32(p + DEBRIS_PLACE) as i32) as u32;
    cpu.write32(o + O_POSITION, x);
    let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, turn)) as i32;
    let back = cpu.read16(o + TAIL_BACK) as i16 as i32;
    let y = cos.wrapping_neg().wrapping_mul(back).wrapping_add(cpu.read32(p + DEBRIS_PLACE + 4) as i32) as u32;
    cpu.write32(o + O_POSITION + 4, y);
    let size = cpu.read32(p + TAIL_SIZE_AT);
    if size as i32 > TAIL_BIG {
        let [top, bottom, left, right] = TAIL_AREA;
        let out = cpu.nested_with_stack(&TAIL_AREA_PROLOGUE, 0x0605_69f8, &[right], |cpu| {
            aim::area_flags(cpu, o + O_FLAGS, top, bottom, left, right)
        });
        if out as u8 == 0 {
            let size = cpu.read32(p + TAIL_SIZE_AT);
            cpu.write32(o + O_SCALE, size);
            cpu.write32(o + O_SCALE_2, size);
            let back = (cpu.read16(o + TAIL_BACK) as i16).wrapping_add(TAIL_BACK_STEP);
            cpu.write16(o + TAIL_BACK, back as u16);
            return;
        }
    }
    cpu.write8(o + O_MODE, MODE_DONE);
}

/// A tail piece made behind `parent`: an object of list 2 (0x74 bytes), flags 0x80000400, the step
/// 0x060955b8, distance 0x10, scale 0x4000; stepped once.
pub fn tail_make(cpu: &mut Cpu, parent: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, TAIL_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, WIN_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + O_SCRIPT));
    cpu.write32(o + O_BEHAVIOUR, TAIL_STEP);
    cpu.write32(o + O_FLAGS, TAIL_FLAGS);
    cpu.write32(o + O_SCRIPT, TAIL_BOXES);
    cpu.write8(o + O_54, 1);
    cpu.write8(o + O_55, 5);
    cpu.write32(o + TAIL_PARENT, parent);
    cpu.write16(o + TAIL_BACK, TAIL_BACK_START);
    cpu.write32(o + O_SCALE, TAIL_SCALE_START);
    nested!(cpu, 0x0609_55b8, tail_step(cpu, o));
}

// ---- the links ---------------------------------------------------------------------------------------------

/// The boss's links: eight object numbers from +0x13d (0 for none), the round counter +0x13c, eight
/// bytes from +0x145 and a halfword +0xea cleared with them, the phase +0x126 and the window made
/// +0x150.
const LINKS: u32 = 0x13d;
const LINKS_MOST: u32 = 8;
const LINKS_ROUND: u32 = 0x13c;
const LINKS_PHASE: u32 = 0x126;
const LINKS_WINDOW: u32 = 0x150;
const LINKS_FLAGS: u32 = 0x145;
const LINKS_HALF: u32 = 0xea;
const LINKS_LINK: u32 = 0x0609_384c;
const LINKS_MODE_GONE: i8 = 111;
/// Each wave's table of link data, time and count.
const LINKS_WAVES: [(u32, u32, u32); 3] = [(0x060a_d00e, 0x708, 7), (0x060a_d054, 0xe10, 6), (0x060a_d090, 0xe10, 8)];
const LINKS_TABLE_STEP: u32 = 10;

/// How many of the boss's links are still alive: a link whose object's mode (+3) has passed 111 is
/// forgotten (its number cleared). The count.
pub fn links_alive(cpu: &mut Cpu, boss: u32) -> u32 {
    let mut count: i8 = 0;
    for k in 0..LINKS_MOST {
        let n = cpu.read8(boss + LINKS + k) as i8;
        if n == 0 {
            continue;
        }
        let o = cpu.read32(NUMBERED.wrapping_add((n as i32 as u32) << 2));
        if cpu.read8(o + 3) as i8 > LINKS_MODE_GONE {
            cpu.write8(boss + LINKS + k, 0);
        } else {
            count += 1;
        }
    }
    count as i32 as u32
}

/// The links forgotten: the round counter up one, the numbers and the eight flag bytes and the
/// halfword cleared.
pub fn links_clear(cpu: &mut Cpu, boss: u32) {
    let round = cpu.read8(boss + LINKS_ROUND).wrapping_add(1);
    cpu.write8(boss + LINKS_ROUND, round);
    for k in 0..LINKS_MOST {
        cpu.write8(boss + LINKS + k, 0);
    }
    for k in 0..8 {
        cpu.write8(boss + LINKS_FLAGS + k, 0);
    }
    cpu.write16(boss + LINKS_HALF, 0);
}

/// Link `k` of a wave made (the code at 0x0609384c, given the wave's kind, time and the table's
/// entry) and its number kept.
fn links_link(cpu: &mut Cpu, boss: u32, k: u32, wave: usize, kind: u32) {
    let (table, time, _) = LINKS_WAVES[wave];
    let n = cpu.call(LINKS_LINK, &[boss, kind, time, table + LINKS_TABLE_STEP * k]);
    cpu.write8(boss + k + LINKS, n as u8);
}

/// The first wave: the links cleared, seven made (kind 0, time 0x708), each with a sound 0x50.
pub fn links_first_wave(cpu: &mut Cpu, boss: u32) {
    nested!(cpu, 0x0609_5764, links_clear(cpu, boss));
    for k in 0..LINKS_WAVES[0].2 {
        links_link(cpu, boss, k, 0, 0);
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 0x50));
    }
}

/// The first wave done once no link is alive: the stage scrolls on, the links cleared, the phase
/// up one.
pub fn links_first_done(cpu: &mut Cpu, boss: u32) {
    if nested!(cpu, 0x0609_5708, links_alive(cpu, boss)) as i8 == 0 {
        nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
        nested!(cpu, 0x0609_5764, links_clear(cpu, boss));
        let phase = cpu.read8(boss + LINKS_PHASE).wrapping_add(1);
        cpu.write8(boss + LINKS_PHASE, phase);
    }
}

/// At phase 3: the links cleared, six made (kind 1, time 0xe10), a craft of kind 6 made (kept as
/// the seventh), the window made (+0x150), the sound 0x51.
pub fn links_second_wave(cpu: &mut Cpu, boss: u32) {
    if cpu.read8(boss + LINKS_PHASE) != 3 {
        return;
    }
    nested!(cpu, 0x0609_5764, links_clear(cpu, boss));
    for k in 0..LINKS_WAVES[1].2 {
        links_link(cpu, boss, k, 1, 1);
    }
    let craft = nested!(cpu, 0x0609_4920, craft_make(cpu, boss, 6));
    cpu.write8(boss + 0x143, craft as u8);
    let window = nested!(cpu, 0x0609_50ec, window_make(cpu, boss));
    cpu.write32(boss + LINKS_WINDOW, window);
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 0x51));
}

/// The third wave once no link is alive: the links cleared, the window's kind up one, eight made
/// (kind 1, time 0xe10), the sound 0x52.
pub fn links_third_wave(cpu: &mut Cpu, boss: u32) {
    if nested!(cpu, 0x0609_5708, links_alive(cpu, boss)) as i8 != 0 {
        return;
    }
    nested!(cpu, 0x0609_5764, links_clear(cpu, boss));
    let window = cpu.read32(boss + LINKS_WINDOW);
    if window != 0 {
        let kind = cpu.read8(window + WIN_KIND).wrapping_add(1);
        cpu.write8(window + WIN_KIND, kind);
    }
    for k in 0..LINKS_WAVES[2].2 {
        links_link(cpu, boss, k, 2, 1);
    }
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 0x52));
}

/// The craft wave once no link is alive: the links cleared, the window's kind up two, five crafts
/// made (kinds 0-4), the sound 0x53.
pub fn links_crafts_wave(cpu: &mut Cpu, boss: u32) {
    if nested!(cpu, 0x0609_5708, links_alive(cpu, boss)) as i8 != 0 {
        return;
    }
    nested!(cpu, 0x0609_5764, links_clear(cpu, boss));
    let window = cpu.read32(boss + LINKS_WINDOW);
    if window != 0 {
        let kind = cpu.read8(window + WIN_KIND).wrapping_add(2);
        cpu.write8(window + WIN_KIND, kind);
    }
    for k in 0..5 {
        let n = nested!(cpu, 0x0609_4920, craft_make(cpu, boss, k));
        cpu.write8(boss + k + LINKS, n as u8);
    }
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 0x53));
}

/// The last wave done once no link is alive: the stage scrolls on, the round counter and the
/// phase up one.
pub fn links_last_done(cpu: &mut Cpu, boss: u32) {
    if nested!(cpu, 0x0609_5708, links_alive(cpu, boss)) as i8 == 0 {
        nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
        let round = cpu.read8(boss + LINKS_ROUND).wrapping_add(1);
        cpu.write8(boss + LINKS_ROUND, round);
        let phase = cpu.read8(boss + LINKS_PHASE).wrapping_add(1);
        cpu.write8(boss + LINKS_PHASE, phase);
    }
}

/// Phase 5: the links forgotten, the round (+0xe6) up one, the halfwords +0xea and +0xf2 cleared,
/// the flags 0xc80000, the boss gauge set to 60000, six links made (kind 2, time 0x708, each from
/// its own table), the sound 0x54.
pub fn links_last_wave(cpu: &mut Cpu, boss: u32) {
    if cpu.read8(boss + LINKS_PHASE) != 5 {
        return;
    }
    nested!(cpu, 0x0609_5764, links_clear(cpu, boss));
    let round = cpu.read8(boss + 0xe6).wrapping_add(1);
    cpu.write8(boss + 0xe6, round);
    cpu.write16(boss + LINKS_HALF, 0);
    cpu.write16(boss + 0xf2, 0);
    cpu.write32(boss + O_FLAGS, 0xc8_0000);
    nested!(cpu, 0x0604_64d8, hud::boss_gauge_set(cpu, 60000));
    for (k, (table, _)) in LAST_SLOTS.into_iter().enumerate() {
        let n = cpu.call(LINKS_LINK, &[boss, 2, 0x708, table]);
        cpu.write8(boss + LINKS + k as u32, n as u8);
    }
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 0x54));
}

/// The last wave's six slots: each its table of link data and how many it makes in all.
const LAST_SLOTS: [(u32, i8); 6] = [
    (0x060a_d0e0, 2),
    (0x060a_d0fe, 3),
    (0x060a_d126, 3),
    (0x060a_d14e, 3),
    (0x060a_d176, 3),
    (0x060a_d19e, 2),
];
const LAST_COUNTS: u32 = 0x145;
const LAST_ROUND_AT: u32 = 0xf8;

/// While the boss's life (+0xf8) is up, a slot of the last wave whose link has gone is filled
/// again from its table (the next entry; the slot's count +0x145... up one), until the count
/// reaches the slot's total.
pub fn links_refill(cpu: &mut Cpu, boss: u32) {
    if (cpu.read32(boss + LAST_ROUND_AT) as i32) <= 0 {
        return;
    }
    nested!(cpu, 0x0609_5708, links_alive(cpu, boss));
    for (k, (table, total)) in LAST_SLOTS.into_iter().enumerate() {
        let k = k as u32;
        if cpu.read8(boss + LINKS + k) != 0 {
            continue;
        }
        let count = cpu.read8(boss + LAST_COUNTS + k) as i8;
        if count >= total {
            continue;
        }
        cpu.write8(boss + LAST_COUNTS + k, (count + 1) as u8);
        let count = cpu.read8(boss + LAST_COUNTS + k) as i8 as i32 as u32;
        let n = cpu.call(LINKS_LINK, &[boss, 2, 0, table.wrapping_add(count * LINKS_TABLE_STEP)]);
        cpu.write8(boss + LINKS + k, n as u8);
    }
}

const BOSS_MOVES: u32 = 0x060a_d2a4;
const BOSS_MOVE_AT: u32 = 0x13c;

/// The boss's move: the code at the move's place in the table at 0x060ad2a4 (by the round counter
/// +0x13c), run for the boss.
pub fn boss_move(cpu: &mut Cpu, boss: u32) {
    let round = cpu.read8(boss + BOSS_MOVE_AT) as u32;
    let f = cpu.read32(BOSS_MOVES + 4 * round);
    cpu.call_returning(0x0609_5d38, f, &[boss]);
}

const CAM_X: u32 = 0x128;
const CAM_Y: u32 = 0x12c;
const CAM_VX: u32 = 0x130;
const CAM_VY: u32 = 0x134;
const CAM_DEPTH: u32 = 0x138;
const CAM_FAR: i32 = 0x8000;
const CAM_FASTEST: i32 = 0x20_0000;
const CAM_NEAR: i32 = 0x4_0000;
const CAM_EASE: u32 = 0x400;
/// Per phase (1-5): where the boss's x and y are brought to.
const CAM_AIMS: [(u32, u32); 5] = [
    (0x80_0000, 0x8c_0000),
    (0x100_0000, 0x8c_0000),
    (0x180_0000, 0x10c_0000),
    (0x200_0000, 0x10c_0000),
    (0x280_0000, 0x18c_0000),
];

/// The boss's place (+0x128, +0x12c) brought to the phase's aim: past phase 5, on, with aim 0;
/// phases 2 and 4 go on (the stage scrolls) once the x is within 0x40000. An axis far (over
/// 0x8000) from its aim has its speed eased toward a fifteenth-sixteenth of the gap (at most
/// 0x200000), by a hundred-and-twenty-eighth of it; near, its place and speed go to the aim and
/// rest by 0x400. Then the places move by the speeds, the scene's three anchors (0x060b2090: the
/// window's, the camera's and the boss's own) are set from them, and the boss's place is set.
pub fn boss_camera(cpu: &mut Cpu, boss: u32) {
    let phase = cpu.read8(boss + LINKS_PHASE);
    let (tx, ty) = match phase {
        1..=5 => CAM_AIMS[phase as usize - 1],
        _ => {
            cpu.write8(boss + LINKS_PHASE, phase.wrapping_add(1));
            (0, 0)
        }
    };
    if phase == 2 || phase == 4 {
        let gap = (cpu.read32(boss + CAM_X) as i32).wrapping_sub(tx as i32);
        if gap.wrapping_abs() < CAM_NEAR {
            nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
            let phase = cpu.read8(boss + LINKS_PHASE).wrapping_add(1);
            cpu.write8(boss + LINKS_PHASE, phase);
        }
    }
    boss_camera_axis(cpu, boss, CAM_X, CAM_VX, tx);
    boss_camera_axis(cpu, boss, CAM_Y, CAM_VY, ty);
    let x = cpu.read32(boss + CAM_X).wrapping_add(cpu.read32(boss + CAM_VX));
    cpu.write32(boss + CAM_X, x);
    let y = cpu.read32(boss + CAM_Y).wrapping_add(cpu.read32(boss + CAM_VY));
    cpu.write32(boss + CAM_Y, y);
    let a = WIN_ANCHOR;
    cpu.write32(a, 0);
    cpu.write32(a + 4, x.wrapping_add(0xff80_0000));
    cpu.write32(a + 8, y.wrapping_add(0xff80_0000));
    cpu.write32(a + 12, 0);
    cpu.write32(a + 16, x.wrapping_add(0xfe80_0000));
    cpu.write32(a + 20, y.wrapping_add(0xff00_0000));
    cpu.write32(a + 24, 0);
    cpu.write32(a + 28, x.wrapping_add(0xfd80_0000));
    cpu.write32(a + 32, y.wrapping_add(0xfe80_0000));
    cpu.write32(boss + O_POSITION, 0);
    let z = cpu.read32(a + 28).wrapping_add(0xffc0_0000);
    cpu.write32(boss + O_POSITION + 4, z);
    let depth = cpu.read32(a + 32).wrapping_add(cpu.read32(boss + CAM_DEPTH));
    cpu.write32(boss + O_POSITION + 8, depth);
}

fn boss_camera_axis(cpu: &mut Cpu, boss: u32, pos_at: u32, vel_at: u32, aim: u32) {
    let pos = cpu.read32(boss + pos_at);
    let gap = pos.wrapping_sub(aim) as i32;
    if gap.wrapping_abs() > CAM_FAR {
        let gap = gap.clamp(-CAM_FASTEST, CAM_FASTEST);
        let biased = if gap < 0 { gap + 127 } else { gap };
        shift_7(cpu, biased as u32);
        let step = (biased >> 7).wrapping_abs() as u32;
        let back = gap.wrapping_neg();
        let to = ((if back < 0 { back + 15 } else { back }) >> 4) as u32;
        let vel = cpu.read32(boss + vel_at);
        let vel = nested!(cpu, 0x0604_6168, actors::approach(cpu, vel, to, step));
        cpu.write32(boss + vel_at, vel);
    } else {
        let pos = nested!(cpu, 0x0604_6168, actors::approach(cpu, pos, aim, CAM_EASE));
        cpu.write32(boss + pos_at, pos);
        let vel = cpu.read32(boss + vel_at);
        let vel = nested!(cpu, 0x0604_6168, actors::approach(cpu, vel, 0, CAM_EASE));
        cpu.write32(boss + vel_at, vel);
    }
}

/// A move that does nothing.
pub fn boss_idle(_cpu: &mut Cpu) {}

/// A move that does nothing (its own frame of 16 bytes, unused).
pub fn boss_idle_framed(_cpu: &mut Cpu) {}

const PAUSE_TIME: u16 = 0x78;
const PAUSE_SCRIPT: u32 = 0x060a_cfa0;

/// Once the pause (+0xea) has passed 0x78: the round (+0xe6) is 2, the pause 0, the boss's script
/// (+0x6c) started from 0x060acfa0, the sound 0x58.
pub fn boss_pause_over(cpu: &mut Cpu, boss: u32) {
    if cpu.read16(boss + LINKS_HALF) <= PAUSE_TIME {
        return;
    }
    cpu.write8(boss + 0xe6, 2);
    cpu.write16(boss + LINKS_HALF, 0);
    nested!(cpu, 0x0605_6628, actors::script_start(cpu, boss + 0x6c, PAUSE_SCRIPT));
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 0x58));
}

const FACE_ANGLE: u32 = 0xe2;
const FACE_SPEED: u32 = 0xe4;
const FACE_TARGET: u32 = 0x178;
static FACE_PROLOGUE: std::sync::OnceLock<super::Prologue> = std::sync::OnceLock::new();

/// The boss's turn (+0x30) sprung (aim::turn_toward, its rate +0xe2, speed +0xe4) toward the
/// opposite of the angle kept at +0x178, and moved by the rate.
pub fn boss_face_away(cpu: &mut Cpu, boss: u32) {
    let from = cpu.read16(boss + O_ANGLE) as u32;
    let to = cpu.read16(boss + FACE_TARGET).wrapping_add(0x8000) as u32;
    cpu.nested_with_stack(&FACE_PROLOGUE, 0x0605_70d8, &[0x8000, 10, 0x40], |cpu| {
        aim::turn_toward(cpu, boss + FACE_ANGLE, boss + FACE_SPEED, from, to, 0x8000, 10, 0x40)
    });
    let turn = cpu.read16(boss + O_ANGLE).wrapping_add(cpu.read16(boss + FACE_ANGLE));
    cpu.write16(boss + O_ANGLE, turn);
}

/// The boss's round (+0xe6) set to 3, the angle to the nearer player kept at +0x178, the pause
/// (+0xea) cleared.
pub fn boss_face_player(cpu: &mut Cpu, boss: u32) {
    cpu.write8(boss + 0xe6, 3);
    let player = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, boss + O_FLAGS));
    let a = nested!(cpu, 0x0604_5e30, aim::angle_to(cpu, boss, player));
    cpu.write16(boss + FACE_TARGET, a as u16);
    cpu.write16(boss + LINKS_HALF, 0);
}

// ---- the window's draw -------------------------------------------------------------------------------------

const WIN_CORNERS: u32 = 44;
const WIN_CORNERS_AT: u32 = 36;
const WIN_FRAME_KIND: u8 = 0x27;
const WIN_FRAME_COLOUR: u32 = 0x0607_41f0 + 0x27c;
const WIN_LOOPS_LOW: u32 = 0x0600_ed8a;
const WIN_EDGE_SCALE: u32 = 0x7000;

/// A window's draw (its sprite `s`; the half sizes +0x64/+0x68 of the sprite, as halfwords): the
/// frame round the gap, four thin quads (a quad's four corners at +44 on the stack, its edge one
/// dot thick on the loop count's low bit) in colour 0x40 on the base plus the count's low bit and
/// its low three bits; then the four edge posts, sprites of kind 0x27 (frame by the loop count
/// 2 bits on, at the window's place less/plus the sizes).
pub fn window_draw(cpu: &mut Cpu, s: u32) {
    let o = s.wrapping_sub(0x10);
    let hx = window_half(cpu.read32(s + 0x64)) as i16;
    let nhx = hx.wrapping_neg();
    let hy = window_half(cpu.read32(s + 0x68)) as i16;
    let nhy = hy.wrapping_neg();
    let count = cpu.read16(WIN_LOOPS_LOW);
    let par = (count & 1) as i16;
    let d = cpu.stack();
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, d));
    cpu.write32(d + S_FLAGS, 1);
    cpu.write32(d + WIN_CORNERS_AT, d + WIN_CORNERS);
    cpu.write8(d + S_LIFT, 0xe0);
    let colour = cpu.read16(CHAR_BASE).wrapping_add(0x40).wrapping_add((par as u16) * 8).wrapping_add(count & 7);
    cpu.write16(d + S_COLOUR, colour);
    for k in 0..3 {
        let v = cpu.read32(o + O_POSITION + 4 * k);
        cpu.write32(d + S_POSITION + 4 * k, v);
    }
    let quads: [[i16; 8]; 4] = [
        [nhx, nhy.wrapping_sub(par), hx, nhy.wrapping_sub(par), hx, nhy, nhx, nhy],
        [nhx, hy, hx, hy, hx, hy.wrapping_add(par), nhx, hy.wrapping_add(par)],
        [nhx, nhy, nhx.wrapping_add(par), nhy, nhx.wrapping_add(par), hy, nhx, hy],
        [hx.wrapping_sub(par), nhy, hx, nhy, hx, hy, hx.wrapping_sub(par), hy],
    ];
    for quad in quads {
        for (k, v) in quad.into_iter().enumerate() {
            cpu.write16(d + WIN_CORNERS + 2 * k as u32, v as u16);
        }
        nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
    }
    cpu.write32(d + S_FLAGS, 0);
    let frame = ((cpu.read32(LOOPS) >> 2) & 3) as u8 + 0x12;
    cpu.write8(d + S_FRAME, frame);
    cpu.write8(d + S_KIND, WIN_FRAME_KIND);
    let post = cpu.read16(WIN_FRAME_COLOUR).wrapping_add(0x10);
    cpu.write16(d + 26, post);
    cpu.write16(d + S_COLOUR, post);
    cpu.write32(d + S_SCALES + 4, WIN_EDGE_SCALE);
    let (px, py) = (cpu.read32(s + 0x64), cpu.read32(s + 0x68));
    let (x, y) = (cpu.read32(o + O_POSITION), cpu.read32(o + O_POSITION + 4));
    cpu.write32(d + S_POSITION, x.wrapping_sub(px));
    cpu.write32(d + S_POSITION + 4, y.wrapping_sub(py));
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
    cpu.write32(d + S_POSITION, px.wrapping_add(x));
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
    cpu.write32(d + S_POSITION + 4, py.wrapping_add(y));
    cpu.write32(d + S_FLAGS, 0x20);
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
    cpu.write32(d + S_POSITION, x.wrapping_sub(px));
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
}

// ---- the head's steps (the boss' second phase) ------------------------------------------------------

/// The boss's round (+0xe6, a byte), its timer (+0xea, a halfword), its flags (+0x94, a
/// halfword: bit 0 a hit, bit 14 the move's end), its three tilts (+0x172, +0x174, +0x176,
/// halfwords eased by `approach`), its depth (+0x138), and the words the heads' moves keep:
/// +0x160, +0x164, +0x168, +0x16c (longs), +0x170 and +0x17f (bytes), +0x17c (a long).
const HEAD_ROUND: u32 = 0xe6;
const HEAD_TIME: u32 = 0xea;
const HEAD_FLAGS: u32 = 0x94;
const HEAD_TILT_A: u32 = 0x172;
const HEAD_TILT_B: u32 = 0x174;
const HEAD_TILT_C: u32 = 0x176;
const HEAD_DEPTH: u32 = 0x138;
const HEAD_K160: u32 = 0x160;
const HEAD_K164: u32 = 0x164;
const HEAD_K168: u32 = 0x168;
const HEAD_K16C: u32 = 0x16c;
const HEAD_COUNT: u32 = 0x170;
const HEAD_K17C: u32 = 0x17c;
const HEAD_K17F: u32 = 0x17f;
const HEAD_IDLE: u32 = 0x0609_6024;
const HEAD_FACE_AWAY: u32 = 0x0609_6034;
const HEAD_FACE_PLAYER: u32 = 0x0609_6098;
const HEAD_ARM_SCRIPT: u32 = 0x060a_cf86;

/// A halfword tilt of the head (a signed halfword) eased toward `to` by `step`.
fn head_tilt(cpu: &mut Cpu, o: u32, at: u32, to: u32, step: u32) {
    let v = cpu.read16(o + at) as i16 as i32 as u32;
    let r = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, to, step));
    cpu.write16(o + at, r as u16);
}

fn shift_8(cpu: &mut Cpu, v: u32) {
    nested!(cpu, 0x0600_8164, clib::ashiftrt_r4::<8>(cpu, v));
}

/// The turn (+0x30) eased toward the opposite of the angle to the nearer player, by a share of
/// the difference (`quotient` by `share`).
fn head_turn_toward_player(cpu: &mut Cpu, o: u32, share: u32) {
    let near = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + O_FLAGS));
    let a = nested!(cpu, 0x0604_5e30, aim::angle_to(cpu, o, near));
    let want = (a as i16 as i32 as u32).wrapping_add(0x8000);
    let turn = cpu.read16(o + O_ANGLE) as i16 as i32 as u32;
    let d = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, turn, want));
    let q = nested!(cpu, 0x0600_6a78, math::quotient(cpu, d, share));
    let t = q.wrapping_add(cpu.read16(o + O_ANGLE) as i16 as i32 as u32);
    cpu.write16(o + O_ANGLE, t as u16);
}

/// The depth eased to 0xc0000 by 0x1000 and the third tilt to 0 by 8; on a hit (flags bit 0) the
/// sound 0x59; at the move's end (bit 14) the pose script restarted; past 0xb4 frames the boss
/// turns to face the player.
pub fn head_close(cpu: &mut Cpu, o: u32) {
    let d = cpu.read32(o + HEAD_DEPTH);
    let d = nested!(cpu, 0x0604_6168, actors::approach(cpu, d, 0xc_0000, 0x1000));
    cpu.write32(o + HEAD_DEPTH, d);
    head_tilt(cpu, o, HEAD_TILT_C, 0, 8);
    nested!(cpu, HEAD_IDLE, boss_idle_framed(cpu));
    if cpu.read16(o + HEAD_FLAGS) & 1 != 0 {
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 0x59));
    }
    if cpu.read16(o + HEAD_FLAGS) & 0x4000 != 0 {
        nested!(cpu, 0x0605_6628, actors::script_start(cpu, o + 0x6c, HEAD_ARM_SCRIPT));
    }
    if cpu.read16(o + HEAD_TIME) > 0xb4 {
        nested!(cpu, HEAD_FACE_PLAYER, boss_face_player(cpu, o));
    }
}

/// The first two tilts eased to 0x40 by 3; after 50 frames the timer is cleared and the round
/// is 4; +0x16c rises by 0x200; the boss turns away.
pub fn head_wind(cpu: &mut Cpu, o: u32) {
    head_tilt(cpu, o, HEAD_TILT_A, 0x40, 3);
    head_tilt(cpu, o, HEAD_TILT_B, 0x40, 3);
    nested!(cpu, HEAD_IDLE, boss_idle_framed(cpu));
    if cpu.read16(o + HEAD_TIME) > 50 {
        cpu.write16(o + HEAD_TIME, 0);
        cpu.write8(o + HEAD_ROUND, 4);
    }
    let k = cpu.read32(o + HEAD_K16C).wrapping_add(0x200);
    cpu.write32(o + HEAD_K16C, k);
    nested!(cpu, HEAD_FACE_AWAY, boss_face_away(cpu, o));
}

/// At the timer's 10th frame: the round and the count up one, the tilts -128, debris made, the
/// pose script restarted (0x060acff4), +0x16c cleared, and a cross flash made at the place kept at
/// +0x154 (a description on the stack: its place +0x154, +0x158 and 0, a kind of 48, a size of
/// 0x18000 and the halfword at +0x30 of the boss); the boss turns away.
pub fn head_burst(cpu: &mut Cpu, o: u32) {
    nested!(cpu, HEAD_IDLE, boss_idle_framed(cpu));
    if cpu.read16(o + HEAD_TIME) == 10 {
        let r = cpu.read8(o + HEAD_ROUND).wrapping_add(1);
        cpu.write8(o + HEAD_ROUND, r);
        let c = cpu.read8(o + HEAD_COUNT).wrapping_add(1);
        cpu.write8(o + HEAD_COUNT, c);
        cpu.write16(o + HEAD_TILT_A, 0xff80);
        cpu.write16(o + HEAD_TILT_B, 0xff80);
        nested!(cpu, 0x0609_54b4, debris_make(cpu, o));
        nested!(cpu, 0x0605_6628, actors::script_start(cpu, o + 0x6c, 0x060a_cff4));
        cpu.write32(o + HEAD_K16C, 0);
        let d = cpu.stack();
        let (x, y) = (cpu.read32(o + 0x154), cpu.read32(o + 0x158));
        cpu.write32(d, x);
        cpu.write32(d + 4, y);
        cpu.write32(d + 8, 0);
        cpu.write16(d + 32, 48);
        cpu.write32(d + 16, 0x1_8000);
        let h = cpu.read16(o + O_ANGLE);
        cpu.write16(d + 24, h);
        nested!(cpu, 0x0605_7e38, explosions::cross_flash(cpu, d));
    }
    nested!(cpu, HEAD_FACE_AWAY, boss_face_away(cpu, o));
}

/// At the move's end (flags bit 14): past 11 on the count the round is 6, the timer cleared,
/// +0x168 0x30000 and the sound 0x46; else the boss faces the player; the pose script restarted.
/// The boss turns away.
pub fn head_open(cpu: &mut Cpu, o: u32) {
    nested!(cpu, HEAD_IDLE, boss_idle_framed(cpu));
    if cpu.read16(o + HEAD_FLAGS) & 0x4000 != 0 {
        if cpu.read8(o + HEAD_COUNT) as i8 > 11 {
            cpu.write8(o + HEAD_ROUND, 6);
            cpu.write16(o + HEAD_TIME, 0);
            cpu.write32(o + HEAD_K168, 0x3_0000);
            nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 0x46));
        } else {
            nested!(cpu, HEAD_FACE_PLAYER, boss_face_player(cpu, o));
        }
        nested!(cpu, 0x0605_6628, actors::script_start(cpu, o + 0x6c, HEAD_ARM_SCRIPT));
    }
    nested!(cpu, HEAD_FACE_AWAY, boss_face_away(cpu, o));
}

const HEAD_ORBIT_KIND: u32 = 44;
const HEAD_ORBIT_COLOUR: u32 = 32;
const HEAD_ORBIT_SCALE: u32 = 12;
const HEAD_ORBIT_ANGLE: u32 = 24;
const HEAD_ORBIT_RADIUS: u32 = 36;
const HEAD_ORBIT_OFFSET: u32 = 46;
const HEAD_ORBIT_TURN: u32 = 48;
const HEAD_ORBIT_LIFT: u32 = 50;
const HEAD_ORBIT_SCALE_SPEED: u32 = 52;
static HEAD_ORBIT_PROLOGUE: std::sync::OnceLock<super::Prologue> = std::sync::OnceLock::new();

/// An orbiter made around the boss's place (+0x154): kind 3, colours 2, scales 0x6000 (the
/// second one by +0x168 over -80), a random angle offset (0x3800 less 0x8000) and turn (0x80),
/// radii -0x80000 and -0x1000, watching the bits of 64 in +0xf2, its angle the turn (+0x30).
pub fn head_orbit_set(cpu: &mut Cpu, o: u32) {
    let d = cpu.stack();
    cpu.write16(d + HEAD_ORBIT_KIND, 3);
    cpu.write16(d + HEAD_ORBIT_COLOUR, 2);
    cpu.write32(d + HEAD_ORBIT_SCALE, 0x6000);
    let k = cpu.read32(o + HEAD_K168);
    cpu.write32(d + HEAD_ORBIT_SCALE + 4, k);
    cpu.write32(d + HEAD_ORBIT_SCALE_SPEED, 0);
    let k = cpu.read32(o + HEAD_K168);
    let q = nested!(cpu, 0x0600_6a78, math::quotient(cpu, k, 0xffff_ffb0));
    cpu.write32(d + HEAD_ORBIT_SCALE_SPEED + 4, q);
    cpu.write16(d + HEAD_ORBIT_ANGLE, 0);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x3800));
    cpu.write16(d + HEAD_ORBIT_OFFSET, r.wrapping_sub(0x8000) as u16);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x80));
    cpu.write16(d + HEAD_ORBIT_TURN, r as u16);
    cpu.write16(d + HEAD_ORBIT_LIFT, 0);
    cpu.write32(d + HEAD_ORBIT_RADIUS, 0xfff8_0000);
    cpu.write32(d + HEAD_ORBIT_RADIUS + 4, 0xffff_f000);
    let angle_from = o + O_ANGLE;
    cpu.nested_with_stack(&HEAD_ORBIT_PROLOGUE, 0x0605_9554, &[angle_from], |cpu| {
        super::orbits::orbit_set(cpu, d, o + 0xf2, 64, o + 0x154, angle_from)
    });
}

/// The tilts eased to 0x80 by 1; +0x168 down 0x100 and +0x16c up 0xa0; once +0x168 is over
/// 0x1000 on an even game frame an orbiter is made; past 0xf0 frames the round is up one, the
/// timer and +0x164 cleared and +0x17c the sound 0x5b's control; with a difficulty the boss's turn
/// is eased (a seventieth) toward the opposite of the angle to the nearer player.
pub fn head_rise(cpu: &mut Cpu, o: u32) {
    nested!(cpu, HEAD_IDLE, boss_idle_framed(cpu));
    head_tilt(cpu, o, HEAD_TILT_A, 0x80, 1);
    head_tilt(cpu, o, HEAD_TILT_B, 0x80, 1);
    let a = cpu.read32(o + HEAD_K168).wrapping_add(0xffff_ff00);
    cpu.write32(o + HEAD_K168, a);
    let b = cpu.read32(o + HEAD_K16C).wrapping_add(0xa0);
    cpu.write32(o + HEAD_K16C, b);
    if cpu.read32(o + HEAD_K168) as i32 > 0x1000 && cpu.read32(GAME_FRAMES) & 1 == 0 {
        nested!(cpu, 0x0609_6354, head_orbit_set(cpu, o));
    }
    if cpu.read16(o + HEAD_TIME) > 0xf0 {
        let r = cpu.read8(o + HEAD_ROUND).wrapping_add(1);
        cpu.write8(o + HEAD_ROUND, r);
        cpu.write16(o + HEAD_TIME, 0);
        cpu.write32(o + HEAD_K164, 0);
        let c = nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 0x5b));
        cpu.write32(o + HEAD_K17C, c as i8 as i32 as u32);
    }
    if cpu.read8(DIFFICULTY_AT) != 0 {
        head_turn_toward_player(cpu, o, 0x70);
    }
}

/// +0x16c eased to 0 by 0x200, the first two tilts to -256 by 12.
pub fn head_settle(cpu: &mut Cpu, o: u32) {
    let k = cpu.read32(o + HEAD_K16C);
    let k = nested!(cpu, 0x0604_6168, actors::approach(cpu, k, 0, 0x200));
    cpu.write32(o + HEAD_K16C, k);
    head_tilt(cpu, o, HEAD_TILT_A, 0xffff_ff00, 12);
    head_tilt(cpu, o, HEAD_TILT_B, 0xffff_ff00, 12);
}

/// The charge: while the timer is within 0xf0 frames +0x160 is brought down by 0x18000 a frame
/// (past it, once +0x160 is gone, the round goes up, +0x160 and +0x164 and the timer are cleared
/// and the sound 0x5b stopped on the control kept at +0x17f; before that it is settled, and
/// brought down by -0x1000); +0x164 eased to the share of the gap (a sixteenth) by its size over
/// 256; +0x160 rises by it, and past 0x8000 the tail is made. At difficulty 2 and up the turn
/// is eased (a three-hundredth) toward the opposite of the angle to the nearer player; the head
/// idles.
pub fn head_charge(cpu: &mut Cpu, o: u32) {
    nested!(cpu, HEAD_IDLE, boss_idle_framed(cpu));
    if cpu.read8(DIFFICULTY_AT) as i8 > 1 {
        head_turn_toward_player(cpu, o, 0x300);
    }
    let step: u32;
    if cpu.read16(o + HEAD_TIME) > 0xf0 {
        if cpu.read32(o + HEAD_K160) as i32 <= 0 {
            let r = cpu.read8(o + HEAD_ROUND).wrapping_add(1);
            cpu.write8(o + HEAD_ROUND, r);
            cpu.write32(o + HEAD_K160, 0);
            cpu.write32(o + HEAD_K164, 0);
            cpu.write16(o + HEAD_TIME, 0);
            let control = cpu.read8(o + HEAD_K17F) as i8 as i32 as u32;
            nested!(cpu, 0x0600_52e0, sound::se_stop(cpu, 0x5b, control));
            return;
        }
        step = 0xffff_f000;
        nested!(cpu, 0x0609_6528, head_settle(cpu, o));
    } else {
        step = 0x1_8000;
    }
    let gap = cpu.read32(o + HEAD_K160).wrapping_sub(step) as i32;
    let biased = if gap < 0 { gap.wrapping_add(0xff) } else { gap };
    shift_8(cpu, biased as u32);
    let size = (biased >> 8).wrapping_abs() as u32;
    let back = gap.wrapping_neg();
    let to = (if back < 0 { back.wrapping_add(15) } else { back } >> 4) as u32;
    let v = cpu.read32(o + HEAD_K164);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, to, size));
    cpu.write32(o + HEAD_K164, v);
    let k = cpu.read32(o + HEAD_K160).wrapping_add(v);
    cpu.write32(o + HEAD_K160, k);
    if k as i32 > 0x8000 {
        nested!(cpu, 0x0609_5674, tail_make(cpu, o));
    }
}

/// Settled; past 60 frames the boss faces the player, the count is cleared and +0xf0 rises.
pub fn head_recover(cpu: &mut Cpu, o: u32) {
    nested!(cpu, HEAD_IDLE, boss_idle_framed(cpu));
    nested!(cpu, 0x0609_6528, head_settle(cpu, o));
    if cpu.read16(o + HEAD_TIME) > 60 {
        nested!(cpu, HEAD_FACE_PLAYER, boss_face_player(cpu, o));
        cpu.write8(o + HEAD_COUNT, 0);
        let k = cpu.read16(o + 0xf0).wrapping_add(1);
        cpu.write16(o + 0xf0, k);
    }
}

/// The three tilts eased to -192 by 12; at the timer's 60th frame an explosion at the boss's
/// place (the boss at +0x3c of its description); past 0xd2 frames it ends.
pub fn head_end(cpu: &mut Cpu, o: u32) {
    nested!(cpu, HEAD_IDLE, boss_idle_framed(cpu));
    head_tilt(cpu, o, HEAD_TILT_A, 0xffff_ff40, 12);
    head_tilt(cpu, o, HEAD_TILT_B, 0xffff_ff40, 12);
    head_tilt(cpu, o, HEAD_TILT_C, 0xffff_ff40, 12);
    if cpu.read16(o + HEAD_TIME) == 60 {
        let d = cpu.stack();
        for k in 0..3 {
            let v = cpu.read32(o + O_POSITION + 4 * k);
            cpu.write32(d + 4 * k, v);
        }
        cpu.write32(d + 60, o);
        nested!(cpu, 0x0605_8ba4, explosions::explosion_set(cpu, d));
    }
    if cpu.read16(o + HEAD_TIME) > 0xd2 {
        cpu.write8(o + O_MODE, MODE_DONE);
    }
}

const HEAD_AIM_AT: u32 = 0x80;
const HEAD_SPIN: u32 = 0xdc;
const HEAD_SPIN_SPEED: u32 = 0xde;
const HEAD_SPIN_RATE: u32 = 0xe0;
const HEAD_LEAN: u32 = 0xd0;
const HEAD_LEAN_SPEED: u32 = 0xd2;
const HEAD_LEAN_RATE: u32 = 0xd4;
const HEAD_ROLL: u32 = 0xd6;
const HEAD_ROLL_SPEED: u32 = 0xd8;
const HEAD_ROLL_RATE: u32 = 0xda;
static HEAD_TURN_PROLOGUE: std::sync::OnceLock<super::Prologue> = std::sync::OnceLock::new();

/// The pose script stepped; three angles (the spin, lean and roll, each with a speed and a rate) are
/// sprung (aim::turn_toward, the arguments left pushed across the first two calls as the SH-2
/// code leaves them) toward the halfwords at +14, +0 and +6 of the table the boss keeps at +0x80;
/// each moves by its speed. The lean and the roll turn about (speed and rate negated) when they pass
/// 0xf000 or fall within 0x1ffe of 0x8000 (the lean) and past 0xfff or within 0xffe of 0xb001 (the
/// roll, which also jumps to 0xc000).
pub fn head_wobble(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0605_6644, models::pose_step(cpu, o + 0x6c));
    let table = cpu.read32(o + HEAD_AIM_AT);
    let from = cpu.read16(o + HEAD_SPIN) as u32;
    let to = cpu.read16(table.wrapping_add(14)) as u32;
    cpu.push(&[0x8000, 10, 0x100]);
    nested!(cpu, 0x0605_70d8, aim::turn_toward(cpu, o + HEAD_SPIN_SPEED, o + HEAD_SPIN_RATE, from, to, 0x8000, 10, 0x100));
    let spin = (cpu.read16(o + HEAD_SPIN) as i16).wrapping_add(cpu.read16(o + HEAD_SPIN_SPEED) as i16);
    cpu.write16(o + HEAD_SPIN, spin as u16);
    let from = cpu.read16(o + HEAD_LEAN) as u32;
    let table = cpu.read32(o + HEAD_AIM_AT);
    let to = cpu.read16(table) as u32;
    cpu.push(&[0x8000, 10, 0x10]);
    nested!(cpu, 0x0605_70d8, aim::turn_toward(cpu, o + HEAD_LEAN_SPEED, o + HEAD_LEAN_RATE, from, to, 0x8000, 10, 0x10));
    cpu.pop(24);
    let lean = (cpu.read16(o + HEAD_LEAN) as i16).wrapping_add(cpu.read16(o + HEAD_LEAN_SPEED) as i16);
    cpu.write16(o + HEAD_LEAN, lean as u16);
    if lean as u16 > 0xf000 {
        cpu.write16(o + HEAD_LEAN, 0);
        head_negate(cpu, o, HEAD_LEAN_SPEED, HEAD_LEAN_RATE);
    }
    let lean = cpu.read16(o + HEAD_LEAN).wrapping_add(0x7fff);
    if lean <= 0x1ffe {
        cpu.write16(o + HEAD_LEAN, 0x8000);
        head_negate(cpu, o, HEAD_LEAN_SPEED, HEAD_LEAN_RATE);
    }
    let table = cpu.read32(o + HEAD_AIM_AT);
    let from = cpu.read16(o + HEAD_ROLL) as u32;
    let to = cpu.read16(table.wrapping_add(6)) as u32;
    cpu.nested_with_stack(&HEAD_TURN_PROLOGUE, 0x0605_70d8, &[0x8000, 10, 0x40], |cpu| {
        aim::turn_toward(cpu, o + HEAD_ROLL_SPEED, o + HEAD_ROLL_RATE, from, to, 0x8000, 10, 0x40)
    });
    let roll = (cpu.read16(o + HEAD_ROLL) as i16).wrapping_add(cpu.read16(o + HEAD_ROLL_SPEED) as i16);
    cpu.write16(o + HEAD_ROLL, roll as u16);
    if roll as u16 <= 0xfff {
        cpu.write16(o + HEAD_ROLL, 0);
        head_negate(cpu, o, HEAD_ROLL_SPEED, HEAD_ROLL_RATE);
    }
    let roll = cpu.read16(o + HEAD_ROLL).wrapping_add(0x4fff);
    if roll <= 0xffe {
        cpu.write16(o + HEAD_ROLL, 0xc000);
    }
}

fn head_negate(cpu: &mut Cpu, o: u32, speed: u32, rate: u32) {
    let v = cpu.read16(o + speed).wrapping_neg();
    cpu.write16(o + speed, v);
    let v = cpu.read16(o + rate).wrapping_neg();
    cpu.write16(o + rate, v);
}

const HEAD_LIFE: u32 = 0xf8;
/// The object's flag that the boss has been hit.
const HEAD_HIT_FLAG: u32 = 0x80_0000;
const HEAD_HIT_COUNT: u32 = 0xf0;
const HEAD_DYING: u32 = 0xf2;
const HEAD_STEPS: u32 = 0x060a_d2c4;
const HEAD_DEAD_SCRIPT: u32 = 0x060a_cfce;
const STAGE_SCORE_AT: u32 = 0x0607_46f4;
const STAGE_RATE_AT: u32 = 0x0607_46f0;

/// The head's step. Once the boss has been hit (flags 0x800000): the gauge set to its life
/// (+0xf8); while the round is within 8, and the life is gone or the hit count (+0xf0) is over 3,
/// the round is 9, the bullets are cleared, the sound 0x5b stopped on its control, the bit of 64
/// set in +0xf2, the timer and the words +0x16c and +0x160 cleared; the stage's score is 0x4e20
/// and its rate up 2 if the life is gone (else the score cleared), the life cleared, the death
/// script started; the wobble. Then the round's step (the table at 0x060ad2c4, by the round), the
/// camera, the move, and the timer up one.
pub fn head_step(cpu: &mut Cpu, o: u32) {
    if cpu.read32(o + O_FLAGS) & HEAD_HIT_FLAG != 0 {
        let life = cpu.read32(o + HEAD_LIFE);
        nested!(cpu, 0x0604_64b8, hud::boss_gauge_life(cpu, life));
        let round = cpu.read8(o + HEAD_ROUND);
        let dying = round <= 8
            && (cpu.read32(o + HEAD_LIFE) as i32 <= 0 || cpu.read16(o + HEAD_HIT_COUNT) as i16 > 3);
        if dying {
            cpu.write8(o + HEAD_ROUND, 9);
            nested!(cpu, 0x0605_61e0, screen::bullets_cleared(cpu));
            let control = cpu.read8(o + HEAD_K17F) as i8 as i32 as u32;
            nested!(cpu, 0x0600_52e0, sound::se_stop(cpu, 0x5b, control));
            let d = cpu.read16(o + HEAD_DYING) | 64;
            cpu.write16(o + HEAD_DYING, d);
            cpu.write16(o + HEAD_TIME, 0);
            cpu.write32(o + HEAD_K16C, 0);
            cpu.write32(o + HEAD_K160, 0);
            if cpu.read32(o + HEAD_LIFE) as i32 > 0 {
                cpu.write32(STAGE_SCORE_AT, 0);
            } else {
                cpu.write32(STAGE_SCORE_AT, 0x4e20);
                let rate = cpu.read32(STAGE_RATE_AT).wrapping_add(2);
                cpu.write32(STAGE_RATE_AT, rate);
            }
            cpu.write32(o + HEAD_LIFE, 0);
            nested!(cpu, 0x0605_6628, actors::script_start(cpu, o + 0x6c, HEAD_DEAD_SCRIPT));
        }
        nested!(cpu, 0x0609_67c4, head_wobble(cpu, o));
    }
    let round = cpu.read8(o + HEAD_ROUND) as u32;
    let f = cpu.read32(HEAD_STEPS + 4 * round);
    cpu.call_returning(0x0609_69e2, f, &[o]);
    nested!(cpu, 0x0609_5d48, boss_camera(cpu, o));
    nested!(cpu, 0x0609_5d24, boss_move(cpu, o));
    let t = cpu.read16(o + HEAD_TIME).wrapping_add(1);
    cpu.write16(o + HEAD_TIME, t);
}

const HEAD_FORM: u32 = 0xf2;
const HEAD_MODELS: u32 = 0x430;
const HEAD_POINTS: u32 = 0x060a_ce08;
const HEAD_POINTS_OUT: u32 = 0xfc;
const HEAD_WORLD: u32 = 0x154;
const HEAD_GLOW: u32 = 0x160;
const HEAD_SPARK: u32 = 0x16c;
const HEAD_FLIP_BITS: u32 = 0x0604_5cdc;
const HEAD_SPARK_DRAW_SCALE: u32 = 0xfffb_c000;

/// The head's model set (the kinds' record at +0x430): its list's `k`th entry, a pair, the one
/// the form's bit 0 picks.
fn head_model(cpu: &mut Cpu, o: u32, k: u32) -> u32 {
    let x = cpu.read32(KINDS + HEAD_MODELS);
    let y = cpu.read32(x + 28);
    let z = cpu.read32(y + 4 * k);
    let flip = (cpu.read16(o + HEAD_FORM) & 1) as u32;
    cpu.read32(z + 4 * flip)
}

fn head_put(cpu: &mut Cpu, o: u32, k: u32) {
    let m = head_model(cpu, o, k);
    cpu.call(SL_PUT_POLYGON, &[m]);
}

fn head_half(cpu: &mut Cpu, at: u32) -> u32 {
    cpu.read16(at) as i16 as i32 as u32
}

/// The head's draw (its sprite `s`, at the object + 0x10). The form (+0xf2) loses its bits 0 and 1
/// if bit 1 is set, and bit 0 sets bit 1; the matrix pushed, moved to the head's place and
/// turned (x, y, z). Unless the form's bit 7 is set: the body put, three points of the table at
/// 0x060ace08 turned into the room at +0xfc; then the parts, each in a matrix of its own (the
/// jaw's and its hinges by the table the boss keeps at +0x80, the pods turned by 0x5555 and 0xaaaa
/// and by +0xd6, the nose), the world place of a point (0, 0x100000, 0) kept at +0x154, the glow
/// (+0x160: a flashing disc, turned by the game frames, scaled by it) while it lasts, the two
/// cheeks, the matrix popped. Then, while the sparks (+0x16c) last, a spark is drawn at +0x154,
/// flickering with the game frames.
pub fn head_draw(cpu: &mut Cpu, s: u32) {
    let o = s.wrapping_sub(0x10);
    let mut form = cpu.read16(o + HEAD_FORM);
    if form & 2 != 0 {
        cpu.write16(o + HEAD_FORM, form & 0xfffc);
        form = cpu.read16(o + HEAD_FORM);
    }
    if form & 1 != 0 {
        cpu.write16(o + HEAD_FORM, form | 2);
    }
    cpu.call(SL_PUSH_MATRIX, &[]);
    let p = [0, 1, 2].map(|i| cpu.read32(o + O_POSITION + 4 * i));
    cpu.call(SL_TRANSLATE, &p);
    let a = head_half(cpu, s + 28);
    cpu.call(SL_ROT_X, &[a]);
    let a = head_half(cpu, s + 30);
    cpu.call(SL_ROT_Y, &[a]);
    let a = head_half(cpu, s + 32);
    cpu.call(SL_ROT_Z, &[a]);
    if cpu.read16(o + HEAD_FORM) & 0x80 == 0 {
        head_parts(cpu, o);
    }
    cpu.call(SL_POP_MATRIX, &[]);
    if cpu.read32(o + HEAD_SPARK) != 0 {
        head_spark_draw(cpu, o);
    }
}

fn head_parts(cpu: &mut Cpu, o: u32) {
    head_put(cpu, o, 0);
    for i in 0..3u32 {
        let at = HEAD_POINTS + 8 * i;
        let x = cpu.read16(at) as u32;
        let y = cpu.read16(at + 2) as u32;
        let z = cpu.read16(at + 4) as u32;
        cpu.call(SL_CALC_POINT, &[x << 16, y << 16, z << 16, o + HEAD_POINTS_OUT + 12 * i]);
    }
    let table = |cpu: &mut Cpu| cpu.read32(o + HEAD_AIM_AT);
    cpu.call(SL_PUSH_MATRIX, &[]);
    cpu.call(SL_TRANSLATE, &[0, 0xffe5_0000, 0]);
    let t = table(cpu);
    let a = head_half(cpu, t + 12);
    cpu.call(SL_ROT_X, &[a]);
    head_put(cpu, o, 1);
    cpu.call(SL_POP_MATRIX, &[]);
    cpu.call(SL_PUSH_MATRIX, &[]);
    cpu.call(SL_TRANSLATE, &[0xffee_0000, 0, 0xffe2_0000]);
    let a = head_half(cpu, o + HEAD_LEAN);
    cpu.call(SL_ROT_Y, &[a]);
    head_put(cpu, o, 2);
    cpu.call(SL_POP_MATRIX, &[]);
    cpu.call(SL_PUSH_MATRIX, &[]);
    cpu.call(SL_TRANSLATE, &[0x12_0000, 0, 0xffe2_0000]);
    let a = head_half(cpu, o + HEAD_LEAN) as i32;
    cpu.call(SL_ROT_Y, &[a.wrapping_neg() as i16 as i32 as u32]);
    head_put(cpu, o, 3);
    cpu.call(SL_POP_MATRIX, &[]);
    cpu.call(SL_PUSH_MATRIX, &[]);
    let t = table(cpu);
    let z = head_half(cpu, t + 4).wrapping_mul(0x80).wrapping_add(0xfff2_0000);
    let y = head_half(cpu, t + 2).wrapping_mul(0x80).wrapping_add(0xfffb_0000);
    cpu.call(SL_TRANSLATE, &[0, y, z]);
    let a = head_half(cpu, o + HEAD_SPIN);
    cpu.call(SL_ROT_Y, &[a.wrapping_add(a) as i16 as i32 as u32]);
    head_put(cpu, o, 4);
    cpu.call(SL_PUSH_MATRIX, &[]);
    for (turn, k) in [(0, 5), (0x5555, 6), (0xffff_aaaa, 7)] {
        if turn != 0 {
            cpu.call(SL_ROT_Y, &[turn]);
        }
        cpu.call(SL_TRANSLATE, &[0, 0x12_0000, 0xfff3_0000]);
        let a = head_half(cpu, o + HEAD_ROLL);
        cpu.call(SL_ROT_X, &[a]);
        head_put(cpu, o, k);
        cpu.call(SL_POP_MATRIX, &[]);
        cpu.call(SL_PUSH_MATRIX, &[]);
    }
    let t = table(cpu);
    let y = head_half(cpu, t + 8).wrapping_mul(0x80);
    cpu.call(SL_TRANSLATE, &[0, y, 0x1_0000]);
    head_put(cpu, o, 10);
    cpu.call(SL_CALC_POINT, &[0, 0x10_0000, 0, o + HEAD_WORLD]);
    if cpu.read32(o + HEAD_GLOW) != 0 {
        let loops = cpu.read32(LOOPS);
        let bits = if loops & 1 != 0 { 0x100 } else { 0 };
        nested!(cpu, HEAD_FLIP_BITS, models::model_flip_bits(cpu, 1, 0, bits));
        cpu.call(SL_TRANSLATE, &[0, 0x4_0000, 0]);
        let loops = cpu.read32(LOOPS);
        cpu.call(SL_ROT_Y, &[loops.wrapping_mul(0x840) as i16 as i32 as u32]);
        cpu.call(SL_ROT_Z, &[0xffff_8000]);
        let g = cpu.read32(o + HEAD_GLOW);
        cpu.call(SL_SCALE, &[g, 0x1_8000, g]);
        let a = cpu.read32(KINDS + 16);
        let b = cpu.read32(a + 28);
        let c = cpu.read32(b);
        let m = cpu.read32(c);
        cpu.call(SL_PUT_POLYGON, &[m]);
    }
    cpu.call(SL_POP_MATRIX, &[]);
    cpu.call(SL_PUSH_MATRIX, &[]);
    cpu.call(SL_TRANSLATE, &[0, 0xffed_0000, 0xfff3_0000]);
    let t = table(cpu);
    let a = head_half(cpu, t + 10);
    cpu.call(SL_ROT_X, &[a]);
    head_put(cpu, o, 8);
    cpu.call(SL_POP_MATRIX, &[]);
    cpu.call(SL_PUSH_MATRIX, &[]);
    cpu.call(SL_TRANSLATE, &[0, 0xffed_0000, 0xd_0000]);
    let t = table(cpu);
    let a = head_half(cpu, t + 10) as i32;
    cpu.call(SL_ROT_X, &[a.wrapping_neg() as i16 as i32 as u32]);
    head_put(cpu, o, 9);
    cpu.call(SL_POP_MATRIX, &[]);
    cpu.call(SL_POP_MATRIX, &[]);
}

/// A spark at the place kept at +0x154/+0x158, its scales the spark counter (+0x16c) plus
/// a quarter of it for each of the game frame's low two bits, its kind flickering with the
/// frame (-44 less its bit 0), a random turn, its colour the stage base plus 48 or 112 (frame
/// bit 1).
fn head_spark_draw(cpu: &mut Cpu, o: u32) {
    let d = cpu.stack();
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, d));
    let loops = cpu.read32(LOOPS);
    if loops & 1 == 0 {
        cpu.write32(d + S_FLAGS, 0x100);
    }
    let low = cpu.read8(LOOPS + 3) as u32;
    cpu.write8(d + S_FRAME, (0u32.wrapping_sub(44).wrapping_sub(low & 1)) as u8);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write16(d + S_TURN, r as u16);
    cpu.write8(d + S_LIFT, 8);
    cpu.write8(d + S_KIND, 0);
    let base = cpu.read16(CHAR_BASE);
    let colour = if cpu.read32(LOOPS) & 2 != 0 { base.wrapping_add(48) } else { base.wrapping_add(112) };
    cpu.write16(d + S_COLOUR, colour);
    let spark = cpu.read32(o + HEAD_SPARK) as i32;
    let quarter = (if spark < 0 { spark + 3 } else { spark }) >> 2;
    let scale = quarter.wrapping_mul((cpu.read32(LOOPS) & 3) as i32).wrapping_add(spark) as u32;
    cpu.write32(d + S_SCALES, scale);
    cpu.write32(d + S_SCALES + 4, scale);
    let x = cpu.read32(o + HEAD_WORLD);
    cpu.write32(d + S_POSITION, x);
    let y = cpu.read32(o + HEAD_WORLD + 4);
    cpu.write32(d + S_POSITION + 4, y);
    cpu.write32(d + S_POSITION + 8, HEAD_SPARK_DRAW_SCALE);
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
}

const HEAD_EXEC: u32 = 0x0604_d600;
const HEAD_SIZE: u32 = 0x180;
const HEAD_PARTS: u32 = 0x060a_ce08;
const HEAD_MAKE_LIFE: u32 = 0xea60;
const HEAD_ANCHORS: [(u32, u32); 3] = [(0xff80_0000, 0xff80_0000), (0xfe80_0000, 0xff00_0000), (0xfd80_0000, 0xfe80_0000)];

/// Sets up the head (the object `o`, numbered `number`): its step (0x06096930), flags
/// 0x80080000 and draw (0x06096a44), its number's place in the numbered objects, the words the
/// moves keep cleared, the stage's score and rate cleared, its form 0xa0 and life 0xea60 with
/// the parts of the table at 0x060ace08 (3 of them, their points at +0xfc), the camera's place
/// cleared and the scene's anchors set from it, the stage scrolled on, the tilts, the pose
/// (script 0x060acf78 over the angles at 0x060ace60), the tilt words cleared and the colour of
/// model 1 set. The parts' arguments stay pushed, as in the SH-2 code.
pub fn head_init(cpu: &mut Cpu, o: u32, number: u32) {
    let number = number as i8 as i32 as u32;
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + O_SCRIPT));
    cpu.write32(o + O_BEHAVIOUR, 0x0609_6930);
    cpu.write32(o + O_FLAGS, 0x8008_0000);
    cpu.write32(o + O_DRAW, 0x0609_6a44);
    cpu.write8(o + O_NUMBER, number as u8);
    let n = cpu.read8(o + O_NUMBER) as i8 as i32 as u32;
    cpu.write32(NUMBERED.wrapping_add(n << 2), o);
    for k in 0..3 {
        cpu.write32(o + O_POSITION + 4 * k, 0);
    }
    cpu.write8(o + HEAD_ROUND, 0);
    for at in [0xec, 0xee, 0xe2, 0xe4, 0xea, 0xe8, 0xf0] {
        cpu.write16(o + at, 0);
    }
    cpu.write32(o + HEAD_K17C, 0);
    cpu.write32(STAGE_RATE_AT, 0);
    cpu.write32(STAGE_SCORE_AT, 0);
    cpu.write16(o + HEAD_FORM, 0xa0);
    cpu.write8(o + 0xf4, 0);
    cpu.write32(o + HEAD_LIFE, HEAD_MAKE_LIFE);
    let n = cpu.read8(o + O_NUMBER) as u32;
    cpu.push(&[HEAD_PARTS, n]);
    nested!(cpu, 0x0605_6ea4, parts::parts_set(cpu, 3, o + HEAD_POINTS_OUT, o + HEAD_FORM, o + HEAD_LIFE, HEAD_PARTS, n));
    cpu.write8(o + LINKS_PHASE, 0);
    for at in [CAM_X, CAM_Y, CAM_VX, CAM_VY] {
        cpu.write32(o + at, 0);
    }
    let (x, y) = (cpu.read32(o + CAM_X), cpu.read32(o + CAM_Y));
    for (k, (ax, ay)) in HEAD_ANCHORS.into_iter().enumerate() {
        cpu.write32(WIN_ANCHOR + 12 * k as u32 + 4, x.wrapping_add(ax));
        cpu.write32(WIN_ANCHOR + 12 * k as u32 + 8, y.wrapping_add(ay));
    }
    nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
    cpu.write8(o + LINKS_ROUND, 0);
    cpu.write32(o + CAM_DEPTH, 0);
    cpu.write32(o + HEAD_GLOW, 0);
    cpu.write32(o + HEAD_SPARK, 0);
    cpu.write8(o + HEAD_COUNT, 0);
    cpu.write16(o + FACE_TARGET, 0x8000);
    cpu.write16(o + HEAD_TILT_A, 0xff80);
    cpu.write16(o + HEAD_TILT_B, 0xff80);
    cpu.write16(o + HEAD_TILT_C, 0xff00);
    nested!(cpu, HEAD_IDLE, boss_idle_framed(cpu));
    cpu.write16(o + 124, 8);
    cpu.write32(o + HEAD_AIM_AT, o + 0x98);
    cpu.write32(o + HEAD_AIM_AT + 4, o + 0xa8);
    cpu.write32(o + HEAD_AIM_AT + 8, 0x060a_cf40);
    nested!(cpu, 0x0605_65f0, models::pose_set(cpu, o + 108, 0x060a_ce60));
    nested!(cpu, 0x0605_6628, actors::script_start(cpu, o + 108, 0x060a_cf78));
    nested!(cpu, 0x0605_6644, models::pose_step(cpu, o + 108));
    for at in [0xd0, 0xd2, 0xd4, 0xd6, 0xd8, 0xda, 0xdc, 0xde, 0xe0] {
        cpu.write16(o + at, 0);
    }
    let colour = cpu.read16(CHAR_BASE).wrapping_add(48) as u32;
    nested!(cpu, 0x0604_5b1c, models::model_colour(cpu, 1, 0, colour));
}

/// Makes the head: a free number in list 2, an object of 0x180 bytes of the exec 0x0604d600,
/// set up (the number given); the number, or 0.
pub fn head_spawn(cpu: &mut Cpu) -> u32 {
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, 2)) as i8;
    if n == -1 {
        return 0;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, HEAD_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, HEAD_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0609_6fc0, head_init(cpu, o, n as i32 as u32));
    n as i32 as u32
}

/// The angle toward the point `at` (x and y), 30 frames ahead (aim::angle_to_target of a stack
/// description with the point at +20 and +24), as a halfword.
pub fn head_aim_at(cpu: &mut Cpu, _from: u32, at: u32) -> u32 {
    let d = cpu.stack();
    let x = cpu.read32(at);
    cpu.write32(d + 20, x);
    let y = cpu.read32(at + 4);
    cpu.write32(d + 24, y);
    nested!(cpu, 0x0605_6ad4, aim::angle_to_target(cpu, d, 30)) & 0xffff
}

/// Whether the point `p` (its z moved by -0xb00000 meanwhile) is on the screen: SGL's convert of
/// it to (x, y), within 0xbfffff of the middle across and 0x6fffff down; then that place is
/// written at `out` and the result is 1 (else 0).
pub fn head_on_screen(cpu: &mut Cpu, _from: u32, p: u32, out: u32) -> u32 {
    let d = cpu.stack();
    let z = cpu.read32(p + 8).wrapping_add(0xff50_0000);
    cpu.write32(p + 8, z);
    cpu.call(0x0606_750c, &[p, d]);
    let z = cpu.read32(p + 8).wrapping_add(0x00b0_0000);
    cpu.write32(p + 8, z);
    let x = cpu.read32(d) as i32;
    if x.wrapping_abs() > 0xbf_ffff {
        return 0;
    }
    let y = cpu.read32(d + 4) as i32;
    if y.wrapping_abs() > 0x6f_ffff {
        return 0;
    }
    cpu.write32(out, x as u32);
    cpu.write32(out + 4, y as u32);
    1
}

/// The mean of a long's high word and the nine records' (24 bytes apart) high words, over ten,
/// as a 16.16 value.
fn head_mean(cpu: &mut Cpu, o: u32, first: u32, records: u32) -> u32 {
    let high = |v: u32| {
        let v = v as i32;
        (v.wrapping_add(if v < 0 { 0xffff } else { 0 }) >> 16) as i16 as i32
    };
    let mut sum = high(cpu.read32(o + first));
    for i in 0..9u32 {
        sum = sum.wrapping_add(high(cpu.read32(o + records + 24 * i)));
    }
    nested!(cpu, 0x0600_6a78, math::quotient(cpu, sum as u32, 10)) << 16
}

/// The middle of the head's nine records (x then y; the longs at +0x13c and +0x140 count in), at
/// `out`.
pub fn head_centre(cpu: &mut Cpu, o: u32, out: u32) {
    let x = head_mean(cpu, o, 0x13c, 0x728);
    cpu.write32(out, x);
    let y = head_mean(cpu, o, 0x140, 0x72c);
    cpu.write32(out + 4, y);
}

/// A drifting piece's step: unless the bullet check (bullets::bullet_ends) ends it, a count (+0x8c)
/// runs down and, while it lasts, the speeds +0x8e and +0x90 are added to the drift (+0x80,
/// +0x84); the place moves by the drift, the scales (+0x20, +0x24) ease to 0x10000 by 0x1800 and
/// the turn goes by 0x1000 a frame (the way the drift's x points). The sprite's lift (+0x33) flickers
/// 126 and 8 with the game frame.
pub fn head_shard_step(cpu: &mut Cpu, o: u32) {
    let ended = nested!(cpu, 0x0605_3b10, bullets::bullet_ends(cpu, o, 0)) as u8;
    if ended == 0 {
        let count = cpu.read16(o + 0x8c) as i16;
        if count > 0 {
            cpu.write16(o + 0x8c, (count - 1) as u16);
            let v = cpu.read32(o + 0x80).wrapping_add(cpu.read16(o + 0x8e) as i16 as i32 as u32);
            cpu.write32(o + 0x80, v);
            let v = cpu.read32(o + 0x84).wrapping_add(cpu.read16(o + 0x90) as i16 as i32 as u32);
            cpu.write32(o + 0x84, v);
        }
        let x = cpu.read32(o + 20).wrapping_add(cpu.read32(o + 0x80));
        cpu.write32(o + 20, x);
        let y = cpu.read32(o + 24).wrapping_add(cpu.read32(o + 0x84));
        cpu.write32(o + 24, y);
        let a = cpu.read32(o + 32);
        let a = nested!(cpu, 0x0604_6168, actors::approach(cpu, a, 0x1_0000, 0x1800));
        cpu.write32(o + 32, a);
        let b = cpu.read32(o + 36);
        let b = nested!(cpu, 0x0604_6168, actors::approach(cpu, b, 0x1_0000, 0x1800));
        cpu.write32(o + 36, b);
        let step = if cpu.read32(o + 0x80) as i32 > 0 { 0xffff_f000 } else { 0x1000 };
        let turn = (cpu.read16(o + 48) as i16 as i32 as u32).wrapping_add(step);
        cpu.write16(o + 48, turn as u16);
    }
    let lift = if cpu.read32(GAME_FRAMES) & 1 == 0 { 126 } else { 8 };
    cpu.write8(o + 51, lift);
}

const HEAD_SHARD_EXEC: u32 = 0x0604_d780;
const HEAD_SHARD_SIZE: u32 = 0xbc;

/// A shard of the head's break made at the place `p` (x, y; its number a byte at +30, its angle a
/// halfword at +44): an object of 0xbc bytes (exec 0x0604d780, step 0x060973d0, flags 0x400,
/// colour the base plus 64, script 0x060ad4a0), thrown along the angle (the sine and the cosine
/// over 14, as the drift speeds +0x8e and +0x90) for 24 frames, its turn the angle plus 0x800 cut to
/// the top four bits, scales 0x18000 and 0x2000.
pub fn head_shard_make(cpu: &mut Cpu, p: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, HEAD_SHARD_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, HEAD_SHARD_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + O_SCRIPT));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    let n = cpu.read8(p + 30);
    cpu.write8(o + O_NUMBER, n);
    cpu.write32(o + O_BEHAVIOUR, 0x0609_73d0);
    cpu.write32(o + O_FLAGS, 0x400);
    let x = cpu.read32(p);
    cpu.write32(o + O_POSITION, x);
    let y = cpu.read32(p + 4);
    cpu.write32(o + O_POSITION + 4, y);
    cpu.write32(o + O_POSITION + 8, 0);
    cpu.write8(o + 51, 8);
    cpu.write8(o + 47, 0xc9);
    cpu.write8(o + 46, 0);
    let c = cpu.read16(CHAR_BASE).wrapping_add(64);
    cpu.write16(o + 42, c);
    cpu.write16(o + 40, c);
    cpu.write32(o + O_SCRIPT, 0x060a_d4a0);
    cpu.write8(o + 84, 1);
    cpu.write8(o + 85, 5);
    let angle = (cpu.read16(p + 44) as i16 as i32 as u32).wrapping_add(0x800) & 0xffff_f000;
    cpu.write16(o + 48, angle as u16);
    cpu.write16(o + 0x8c, 24);
    let a = cpu.read16(o + 48) as i16 as i32 as u32;
    let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    let v = nested!(cpu, 0x0600_6a78, math::quotient(cpu, sin, 14));
    cpu.write16(o + 0x8e, v as u16);
    let a = cpu.read16(o + 48) as i16 as i32 as u32;
    let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    let v = nested!(cpu, 0x0600_6a78, math::quotient(cpu, cos.wrapping_neg(), 14));
    cpu.write16(o + 0x90, v as u16);
    cpu.write32(o + 32, 0x1_8000);
    cpu.write32(o + 36, 0x2000);
}

/// A spark's step: unless the bullet check ends it, the place moves by the drift (+0x80, +0x84)
/// and, on odd game frames, the frame (+0x2f) counts to 10 and starts over.
pub fn head_spark_step(cpu: &mut Cpu, o: u32) {
    let ended = nested!(cpu, 0x0605_3b10, bullets::bullet_ends(cpu, o, 0)) as u8;
    if ended != 0 {
        return;
    }
    let x = cpu.read32(o + 20).wrapping_add(cpu.read32(o + 0x80));
    cpu.write32(o + 20, x);
    let y = cpu.read32(o + 24).wrapping_add(cpu.read32(o + 0x84));
    cpu.write32(o + 24, y);
    if cpu.read32(GAME_FRAMES) & 1 != 0 {
        let f = cpu.read8(o + 47).wrapping_add(1);
        cpu.write8(o + 47, f);
        if f > 10 {
            cpu.write8(o + 47, 0);
        }
    }
}

/// A spark made at the place `p` (as the shard, but the drift is the angle's sine and cosine times
/// the speed at +48, the frame 0, the lift 7, the kind 69, the colour the base plus 80, the
/// script 0x060ad4a8, the step 0x060975e4 and the flags 0x410).
pub fn head_spark_make(cpu: &mut Cpu, p: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, HEAD_SHARD_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, HEAD_SHARD_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + O_SCRIPT));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    let n = cpu.read8(p + 30);
    cpu.write8(o + O_NUMBER, n);
    cpu.write32(o + O_BEHAVIOUR, 0x0609_75e4);
    cpu.write32(o + O_FLAGS, 0x410);
    let x = cpu.read32(p);
    cpu.write32(o + O_POSITION, x);
    let y = cpu.read32(p + 4);
    cpu.write32(o + O_POSITION + 4, y);
    cpu.write32(o + O_POSITION + 8, 0);
    cpu.write8(o + 51, 7);
    cpu.write8(o + 47, 0);
    cpu.write8(o + 46, 69);
    let c = cpu.read16(CHAR_BASE).wrapping_add(80);
    cpu.write16(o + 42, c);
    cpu.write16(o + 40, c);
    cpu.write32(o + O_SCRIPT, 0x060a_d4a8);
    cpu.write8(o + 84, 1);
    cpu.write8(o + 85, 5);
    let speed = cpu.read32(p + 48);
    let a = cpu.read16(p + 44) as i16 as i32 as u32;
    let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    let v = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, sin, speed));
    cpu.write32(o + 0x80, v);
    let speed = cpu.read32(p + 48);
    let a = cpu.read16(p + 44) as i16 as i32 as u32;
    let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    let v = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, cos.wrapping_neg(), speed));
    cpu.write32(o + 0x84, v);
}

const HEAD_FIRE_POINTS: u32 = 0xce8;
const HEAD_FIRE_STEPS: u32 = 0x060a_d4b0;

/// Fire from the head's nine places (the longs at +0x6f8 live, +0x128 plus four a place counted
/// against the difficulty's period): a description on the stack (the head's number, z 0, kind 2,
/// speed 0x12000) with the place's world point (two records of 12 bytes at +0xce8 + 24 a place,
/// one picked at random) when it is on the screen and its angle toward the players, a spark or
/// a shot by chance.
pub fn head_fire(cpu: &mut Cpu, p: u32) {
    let d = cpu.stack();
    for i in 0..9u32 {
        if (cpu.read32(p + 4 * i + 0x6f8) as i32) <= 0 {
            continue;
        }
        let level = cpu.read8(DIFFICULTY_AT) as i8 as i32 as u32;
        let period = cpu.read8(HEAD_FIRE_STEPS.wrapping_add(level)) as u32;
        let counter = cpu.read32(p + 0x128).wrapping_add(4 * i);
        if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, counter, period)) != 0 {
            continue;
        }
        let n = cpu.read8(p + 50);
        cpu.write8(d + 30, n);
        cpu.write32(d + 8, 0);
        cpu.write16(d + 46, 2);
        cpu.write32(d + 48, 0x1_2000);
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
        let at = p + 24 * i + HEAD_FIRE_POINTS + if r & 64 != 0 { 12 } else { 0 };
        if nested!(cpu, 0x0609_72a8, head_on_screen(cpu, p, at, d)) as u8 == 0 {
            continue;
        }
        let angle = nested!(cpu, 0x0609_7280, head_aim_at(cpu, p, at));
        cpu.write16(d + 44, angle as u16);
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
        if r & 1 != 0 {
            nested!(cpu, 0x0609_74b0, head_shard_make(cpu, d));
        } else {
            nested!(cpu, 0x0605_3db0, bullets::bullet_set(cpu, d));
        }
    }
}

// ---- the homing shards ---------------------------------------------------------------------------

/// A homing shard's heading (+0x8c), the heading it turns toward (+0x8e), the drift (+0x80, +0x84),
/// its speed (+0xac) and its life (+0x50).
const HOMING_HEADING: u32 = 0x8c;
const HOMING_TO: u32 = 0x8e;
const HOMING_DRIFT: u32 = 0x80;
const HOMING_SPEED: u32 = 0xac;
const HOMING_LIFE: u32 = 0x50;
/// The head's place the shards come out at (x, y, and z, +0xdd8) and the counters of its two
/// fire timers (+0xde4 the shot spacing set, +0xde8 the count it is taken mod).
const BOSS_FIRE_AT: u32 = 0xdd8;
const BOSS_FIRE_SPACING: u32 = 0xde4;
const BOSS_FIRE_COUNT: u32 = 0xde8;
/// The shot period by the difficulty (a byte table in the overlay).
const HOMING_PERIODS: u32 = 0x060a_d4c8;
const HOMING_SCRIPT: u32 = 0x060a_d4b6;
const HOMING_STEP: u32 = 0x0609_7898;
const HOMING_SOUND: u32 = 92;
const HOMING_HIT_SOUND: u32 = 12;
/// The ring an effect that dies in the air or on a wall makes: its colour (+0x20), width and height
/// (+0x2c, +0x2e) and speed (+0x30).
const RING_COLOUR: u32 = 0x20;
const RING_WIDTH: u32 = 0x2c;
const RING_HEIGHT: u32 = 0x2e;
const RING_SPEED: u32 = 0x30;
/// The sign of a 16-bit angle sum, its high word as a fixed-point whole part.
fn high_word(v: u32) -> i32 {
    let v = v as i32;
    (v.wrapping_add(if v < 0 { 0xffff } else { 0 }) >> 16) as i16 as i32
}

/// A homing shard's step. It ends in a ring when it is on a wall (the ground check sets flags
/// +0x56 to 0x400), below the screen's floor (y past 0xa00000) or out of life (+0x50): a ring
/// (sound 12) when a hit flag (0x600) is set and the mode (+3) to 0x70. Otherwise its heading
/// (+0x8c) turns a sixteenth of the way toward the target heading (+0x8e), the drift (+0x80, +0x84)
/// eases toward the speed (+0xac) along the heading (by the heading's sine and cosine over 16),
/// and the place moves by it.
pub fn homing_step(cpu: &mut Cpu, o: u32) {
    let d = cpu.stack();
    let (x, y) = (cpu.read32(o + 20), cpu.read32(o + 24));
    if nested!(cpu, 0x0605_ef90, bg::map_hit(cpu, x, y)) != 0 {
        cpu.write16(o + O_ARMS, 0x400);
    }
    let flags = cpu.read16(o + O_ARMS) as i16 as i32 as u32;
    let flying = flags & 0x400 == 0
        && cpu.read32(o + 24) as i32 <= 0x00a0_0000
        && cpu.read32(o + HOMING_LIFE) as i32 > 0;
    if !flying {
        let flags = cpu.read16(o + O_ARMS) as i16 as i32 as u32;
        if flags & 0x600 != 0 {
            nested!(cpu, 0x0600_50d4, sound::se_set(cpu, HOMING_HIT_SOUND));
            let v = cpu.read32(o + 20);
            cpu.write32(d, v);
            let v = cpu.read32(o + 24);
            cpu.write32(d + 4, v);
            let v = cpu.read32(o + 28);
            cpu.write32(d + 8, v);
            cpu.write16(d + RING_COLOUR, 1);
            cpu.write16(d + RING_WIDTH, 0x2000);
            cpu.write16(d + RING_HEIGHT, 0xc00);
            cpu.write32(d + RING_SPEED, 0);
            cpu.write32(d + RING_SPEED + 4, 0);
            cpu.write32(d + RING_SPEED + 8, 0);
            nested!(cpu, 0x0605_9144, explosions::ring_set(cpu, d));
        }
        cpu.write8(o + O_MODE, 0x70);
        return;
    }
    let heading = cpu.read16(o + HOMING_HEADING) as i16 as i32 as u32;
    let to = cpu.read16(o + HOMING_TO) as i16 as i32 as u32;
    let diff = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, heading, to)) as i32;
    let diff = diff.wrapping_add(if diff < 0 { 15 } else { 0 }) >> 4;
    let turned = diff.wrapping_add(cpu.read16(o + HOMING_HEADING) as i16 as i32);
    cpu.write16(o + HOMING_HEADING, turned as u16);
    cpu.write16(o + O_ANGLE, turned as u16);
    let a = cpu.read16(o + HOMING_HEADING) as i16 as i32 as u32;
    let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a)) as i32;
    let step = (sin.wrapping_add(if sin < 0 { 15 } else { 0 }) >> 4).wrapping_abs() as u32;
    let a = cpu.read16(o + HOMING_HEADING) as i16 as i32 as u32;
    let speed = cpu.read32(o + HOMING_SPEED);
    let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    let want = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, sin, speed));
    let from = cpu.read32(o + HOMING_DRIFT);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, from, want, step));
    cpu.write32(o + HOMING_DRIFT, v);
    let a = cpu.read16(o + HOMING_HEADING) as i16 as i32 as u32;
    let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a)) as i32;
    let up = cos.wrapping_neg();
    let step = (up.wrapping_add(if up < 0 { 15 } else { 0 }) >> 4).wrapping_abs() as u32;
    let speed = cpu.read32(o + HOMING_SPEED);
    let a = cpu.read16(o + HOMING_HEADING) as i16 as i32 as u32;
    let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    let want = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, cos.wrapping_neg(), speed));
    let from = cpu.read32(o + HOMING_DRIFT + 4);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, from, want, step));
    cpu.write32(o + HOMING_DRIFT + 4, v);
    let x = cpu.read32(o + 20).wrapping_add(cpu.read32(o + HOMING_DRIFT));
    cpu.write32(o + 20, x);
    let y = cpu.read32(o + 24).wrapping_add(cpu.read32(o + HOMING_DRIFT + 4));
    cpu.write32(o + 24, y);
}

/// A homing shard made from the head `p`'s fire place (the world point at +0xdd8 turned to the
/// screen by SGL with its z lowered by 0xb00000 meanwhile): an object of 0xbc bytes (exec
/// 0x0604d780, step 0x06097898, flags 0x300400, kind 17 with the lift 8, colour the base plus
/// 16, script 0x060ad4b6, life 0x320), thrown at a random heading (below 0x2000, the target
/// 0x1800 random less 0x8000) at speed 0x28000 with the drift its sine and cosine; a jet behind
/// it (reach 12, size 4) and the sound 92.
pub fn homing_make(cpu: &mut Cpu, p: u32) {
    let d = cpu.stack();
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, HEAD_SHARD_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, HEAD_SHARD_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + O_SCRIPT));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + O_BEHAVIOUR, HOMING_STEP);
    let n = cpu.read8(p + O_NUMBER);
    cpu.write8(o + O_NUMBER, n);
    cpu.write32(o + O_FLAGS, 0x30_0400);
    let z = cpu.read32(p + BOSS_FIRE_AT + 8).wrapping_add(0xff50_0000);
    cpu.write32(p + BOSS_FIRE_AT + 8, z);
    nested!(cpu, 0x0606_750c, sgl::sl_convert(cpu, p + BOSS_FIRE_AT, o + O_POSITION));
    let z = cpu.read32(p + BOSS_FIRE_AT + 8).wrapping_add(0x00b0_0000);
    cpu.write32(p + BOSS_FIRE_AT + 8, z);
    cpu.write32(o + O_POSITION + 8, 0);
    cpu.write8(o + 51, 8);
    cpu.write8(o + O_FRAME, 17);
    cpu.write8(o + O_KIND, 12);
    let c = (cpu.read16(KINDS + 0xcc) as i16 as i32 as u32).wrapping_add(16);
    cpu.write16(o + 42, c as u16);
    cpu.write16(o + 40, c as u16);
    cpu.write32(o + O_SCRIPT, HOMING_SCRIPT);
    cpu.write8(o + 84, 1);
    cpu.write32(o + HOMING_LIFE, 0x320);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x2000));
    cpu.write16(o + HOMING_HEADING, r as u16);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1800));
    cpu.write16(o + HOMING_TO, r.wrapping_add(0xffff_8000) as u16);
    cpu.write32(o + HOMING_SPEED, 0x2_8000);
    let a = cpu.read16(o + HOMING_HEADING) as i16 as i32 as u32;
    let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    cpu.write32(o + HOMING_DRIFT, sin);
    let a = cpu.read16(o + HOMING_HEADING) as i16 as i32 as u32;
    let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    cpu.write32(o + HOMING_DRIFT + 4, cos.wrapping_neg());
    cpu.write32(d + 60, o);
    cpu.write16(d + 44, 12);
    cpu.write16(d + 46, 4);
    nested!(cpu, 0x0605_7fe8, explosions::jet_set(cpu, d));
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, HOMING_SOUND));
}

/// The head's homing fire: each time its count (the halfword at +0xde8) is a multiple of the
/// difficulty's period it sets the spacing (+0xde4) to 0xe000 and makes a homing shard.
pub fn homing_fire(cpu: &mut Cpu, boss: u32) {
    let level = cpu.read8(DIFFICULTY_AT) as i8 as i32 as u32;
    let period = cpu.read8(HOMING_PERIODS.wrapping_add(level)) as u32;
    let count = cpu.read16(boss + BOSS_FIRE_COUNT) as i16 as i32 as u32;
    if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, count, period)) == 0 {
        cpu.write32(boss + BOSS_FIRE_SPACING, 0xe000);
        nested!(cpu, 0x0609_7a58, homing_make(cpu, boss));
    }
}

// ---- the worm ------------------------------------------------------------------------------------

/// The worm's fields: twelve links' places (x at +0x74, y at +0xa4, a long each), three byte trails
/// of 72 (the x and y offsets of a link from its neighbour at +0xd4 and +0x11c, the turn at +0x164),
/// the byte step of the turn (+0x26e), the halfword kind (+0x26c, 0x150 the fast one), the delay
/// (+0x26f), the turned total (+0x270) and the speed (+0x274); the dying count (+0x6e, a halfword)
/// and the two bytes shifted into the trails (+0x70, +0x71).
const WORM_LINKS_X: u32 = 0x74;
const WORM_LINKS_Y: u32 = 0xa4;
const WORM_TRAIL_X: u32 = 0xd4;
const WORM_TRAIL_Y: u32 = 0x11c;
const WORM_TRAIL_TURN: u32 = 0x164;
const WORM_TURN_STEP: u32 = 0x26e;
const WORM_KIND: u32 = 0x26c;
const WORM_DELAY: u32 = 0x26f;
const WORM_TURNED: u32 = 0x270;
const WORM_SPEED: u32 = 0x274;
const WORM_DYING: u32 = 0x6e;
const WORM_SHIFT_X: u32 = 0x70;
const WORM_SHIFT_Y: u32 = 0x71;
const WORM_TOGGLE: u32 = 108;
const WORM_FAST: u32 = 0x150;
const WORM_TRAIL: u32 = 72;
const WORM_STEP: u32 = 0x0609_7e6c;
const WORM_DRAW: u32 = 0x0609_7f74;
const WORM_SIZE: u32 = 0x278;
const WORM_SPARK_STEP: u32 = 0x0609_7c40;
const WORM_SPARK_SCRIPT: u32 = 0x060a_d4ce;

/// A spark of the worm's (its fields: the count of frames left, a halfword at +0x6c, which half of
/// the frames it shows on, a byte at +0x6e): the count runs down and at the end the mode (+3) is
/// 0x70; its flag bit 0x80000 (hidden) is set unless the game frame's bit 0 is the byte at +0x6e.
pub fn worm_spark_step(cpu: &mut Cpu, o: u32) {
    let left = (cpu.read16(o + 108) as i16 as i32).wrapping_sub(1);
    cpu.write16(o + 108, left as u16);
    if (left as i16) < 0 {
        cpu.write8(o + O_MODE, 0x70);
    }
    let frame = cpu.read32(GAME_FRAMES) & 1;
    let half = cpu.read8(o + 110) as u32;
    let flags = cpu.read32(o + O_FLAGS);
    let flags = if frame == half { flags | 0x8_0000 } else { flags & 0xfff7_ffff };
    cpu.write32(o + O_FLAGS, flags);
}

/// A spark made from the description `p` (the number a byte at +30, the place x and y, the count
/// a halfword at +44, the half a byte at +47) for the owner `owner` (kept at +0x3c): an object of
/// 0x70 bytes (exec 0x0604d780, step 0x06097c40, flags 0x80000400, the frames' hit boxes
/// +0x54 = 1 and +0x55 = 5).
pub fn worm_spark_make(cpu: &mut Cpu, p: u32, owner: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0x70)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, HEAD_SHARD_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + O_SCRIPT));
    cpu.write32(o + O_BEHAVIOUR, WORM_SPARK_STEP);
    cpu.write32(o + O_FLAGS, 0x8000_0400);
    let n = cpu.read8(p + 30);
    cpu.write8(o + O_NUMBER, n);
    let x = cpu.read32(p);
    cpu.write32(o + O_POSITION, x);
    let y = cpu.read32(p + 4);
    cpu.write32(o + O_POSITION + 4, y);
    cpu.write32(o + O_POSITION + 8, 0);
    cpu.write32(o + O_SCRIPT, owner);
    cpu.write8(o + 84, 1);
    cpu.write8(o + 85, 5);
    let count = cpu.read16(p + 44);
    cpu.write16(o + 108, count);
    let half = cpu.read8(p + 47);
    cpu.write8(o + 110, half);
}

/// The worm's move: while its delay (+0x26f) counts down nothing, then (until the turned total
/// +0x270 passes 0x7fff) its turn (+0x30) goes by the kind (+0x26c) times the step (+0x26e) and the
/// total by the kind; past it the turn is 0x8000. The speed (+0x274) eases to 0x28000 by 0x4000
/// and the place moves along the turn (x by the sine, y by minus the cosine, times the speed),
/// the bytes +0x70 and +0x71 the whole steps moved.
pub fn worm_move(cpu: &mut Cpu, o: u32) {
    let delay = cpu.read8(o + WORM_DELAY);
    if delay as i8 != 0 {
        cpu.write8(o + WORM_DELAY, delay.wrapping_sub(1));
    } else if cpu.read32(o + WORM_TURNED) as i32 > 0x7fff {
        cpu.write16(o + O_ANGLE, 0x8000);
    } else {
        let step = cpu.read8(o + WORM_TURN_STEP) as i8 as i32 as u32;
        let kind = cpu.read16(o + WORM_KIND) as u32;
        let turn = (cpu.read16(o + O_ANGLE) as i16 as i32 as u32).wrapping_add(kind.wrapping_mul(step));
        cpu.write16(o + O_ANGLE, turn as u16);
        let kind = cpu.read16(o + WORM_KIND) as u32;
        let total = cpu.read32(o + WORM_TURNED).wrapping_add(kind);
        cpu.write32(o + WORM_TURNED, total);
    }
    let speed = cpu.read32(o + WORM_SPEED);
    let speed = nested!(cpu, 0x0604_6168, actors::approach(cpu, speed, 0x2_8000, 0x4000));
    cpu.write32(o + WORM_SPEED, speed);
    let old_x = cpu.read32(o + 20);
    let a = cpu.read16(o + O_ANGLE) as i16 as i32 as u32;
    let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    let dx = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, sin, speed));
    let x = cpu.read32(o + 20).wrapping_add(dx);
    cpu.write32(o + 20, x);
    cpu.write8(o + WORM_SHIFT_X, (high_word(x) - high_word(old_x)) as u8);
    let old_y = cpu.read32(o + 24);
    let speed = cpu.read32(o + WORM_SPEED).wrapping_neg();
    let a = cpu.read16(o + O_ANGLE) as i16 as i32 as u32;
    let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    let dy = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, cos, speed));
    let y = cpu.read32(o + 24).wrapping_add(dy);
    cpu.write32(o + 24, y);
    cpu.write8(o + WORM_SHIFT_Y, (high_word(y) - high_word(old_y)) as u8);
}

/// The worm's step. Dying (the count +0x6e running): the count goes up and past 71 the mode (+3)
/// is 0x70. Else below the floor (y past 0xa00000) the count starts at 1 and the shifts are 0;
/// else it moves (`worm_move`) and every eighth game frame leaves a spark (the toggle +0x6c
/// flipped, the description holding the number, the place, the count 48 and the toggle) at its
/// place. Then the three trails take the shifts (+0x70, +0x71) and the turn over 256 as their
/// newest byte, unless it was just done.
pub fn worm_step(cpu: &mut Cpu, o: u32) {
    let d = cpu.stack();
    let dying = cpu.read16(o + WORM_DYING) as i16 as i32 as u32;
    if dying as u16 != 0 {
        let n = dying.wrapping_add(1);
        cpu.write16(o + WORM_DYING, n as u16);
        if n as u16 > 71 {
            cpu.write8(o + O_MODE, 0x70);
            return;
        }
    } else if cpu.read32(o + 24) as i32 > 0xa0_0000 {
        cpu.write16(o + WORM_DYING, 1);
        cpu.write8(o + WORM_SHIFT_X, 0);
        cpu.write8(o + WORM_SHIFT_Y, 0);
    } else {
        nested!(cpu, 0x0609_7d38, worm_move(cpu, o));
        if cpu.read32(GAME_FRAMES) & 7 == 0 {
            let n = cpu.read8(o + O_NUMBER);
            cpu.write8(d + 30, n);
            let v = cpu.read32(o + 20);
            cpu.write32(d, v);
            let v = cpu.read32(o + 24);
            cpu.write32(d + 4, v);
            cpu.write16(d + 44, 48);
            let toggle = cpu.read8(o + WORM_TOGGLE);
            cpu.write16(d + 46, toggle as u16);
            cpu.write8(o + WORM_TOGGLE, toggle ^ 1);
            nested!(cpu, 0x0609_7c90, worm_spark_make(cpu, d, WORM_SPARK_SCRIPT));
        }
    }
    let sx = cpu.read8(o + WORM_SHIFT_X) as i8 as i32 as u32;
    nested!(cpu, 0x0605_62b4, actors::shift_in_bytes(cpu, o + WORM_TRAIL_X, sx, WORM_TRAIL));
    let sy = cpu.read8(o + WORM_SHIFT_Y) as i8 as i32 as u32;
    nested!(cpu, 0x0605_62b4, actors::shift_in_bytes(cpu, o + WORM_TRAIL_Y, sy, WORM_TRAIL));
    let turn = cpu.read16(o + O_ANGLE) as i16 as i32 as u32;
    let turn = turn.wrapping_add(if (turn as i32) < 0 { 0xff } else { 0 });
    shift_8(cpu, turn);
    let r4 = cpu.reg(4);
    nested!(cpu, 0x0605_62b4, actors::shift_in_bytes(cpu, o + WORM_TRAIL_TURN, r4 as i8 as i32 as u32, WORM_TRAIL));
}

/// The worm's draw (its sprite `s`, the object 16 before it): the frame flags (+0x6e of the
/// object) cleared of bit 1 and set again from bit 0; the twelve links' places advanced by the
/// trails (a byte's whole step in the high word, unless the game is paused) and drawn, each a
/// sprite at the link with the turn's byte times 256; the head link (the first, flags +0x20)
/// and tail (the last) in their own colours.
pub fn worm_draw(cpu: &mut Cpu, s: u32) {
    let d = cpu.stack();
    let o = s.wrapping_sub(0x10);
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, d));
    cpu.write8(d + 30, 0);
    let loops = cpu.read32(LOOPS);
    cpu.write32(d + 16, ((loops & 3) << 12).wrapping_add(0x2_0000));
    cpu.write32(d + 12, 0);
    let fast = cpu.read16(o + WORM_KIND) == WORM_FAST as u16;
    let colour = if fast && cpu.read16(o + O_ANGLE) as i16 != 0 { 0xb8 } else { 8 };
    cpu.write8(d + 35, colour);
    let base = cpu.read16(CHAR_BASE) as i16 as i32 as u32;
    let odd = cpu.read32(LOOPS) & 1 != 0;
    cpu.write16(d + 24, base.wrapping_add(if odd { 32 } else { 48 }) as u16);
    if cpu.read32(GAME_FLAGS) & 0x8000_0000 == 0 {
        for i in 0..12u32 {
            let b = (cpu.read8(o + WORM_TRAIL_X + 5 * i) as i8 as i32 as u32) << 16;
            let v = cpu.read32(o + WORM_LINKS_X + 4 * i).wrapping_add(b);
            cpu.write32(o + WORM_LINKS_X + 4 * i, v);
            let b = (cpu.read8(o + WORM_TRAIL_Y + 5 * i) as i8 as i32 as u32) << 16;
            let v = cpu.read32(o + WORM_LINKS_Y + 4 * i).wrapping_add(b);
            cpu.write32(o + WORM_LINKS_Y + 4 * i, v);
        }
    }
    for i in 0..12u32 {
        let v = cpu.read32(o + WORM_LINKS_X + 4 * i);
        cpu.write32(d + 4, v);
        let v = cpu.read32(o + WORM_LINKS_Y + 4 * i);
        cpu.write32(d + 8, v);
        let turn = (cpu.read8(o + WORM_TRAIL_TURN + 5 * i) as i8 as i32 as u32) << 8;
        cpu.write16(d + 32, turn as u16);
        let eyes = if cpu.read32(LOOPS) & 1 == 0 { 0xb4 } else { 0xb3 };
        if i == 0 {
            cpu.write32(d + 20, 0xc000);
            let v = cpu.read32(d) | 0x20;
            cpu.write32(d, v);
            cpu.write8(d + 31, eyes);
        } else if i == 11 {
            cpu.write32(d + 20, 0xc000);
            let v = cpu.read32(d) & 0xffff_ffdf;
            cpu.write32(d, v);
            cpu.write8(d + 31, eyes);
        } else {
            cpu.write8(d + 31, 0xaf);
            cpu.write32(d + 20, 0x1_4000);
        }
        nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
    }
}

/// A worm made from `p` (its number copied): an object of 0x278 bytes (exec 0x0604d780, step
/// 0x06097e6c, draw 0x06097f74, flags 0xc80400, colour the base plus 48), at x 0 and y 0x80000,
/// the turn step `step` (a byte, +0x26e) and the kind `kind` (+0x26c; the delay 28 for the fast
/// kind 0x150, else 48), the speed 0x8000, its three trails cleared (72 bytes each, the turn's
/// from the turn over 256) and the twelve links all at the place.
pub fn worm_make(cpu: &mut Cpu, p: u32, step: u32, kind: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, WORM_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, HEAD_SHARD_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + O_SCRIPT));
    cpu.write32(o + O_BEHAVIOUR, WORM_STEP);
    let n = cpu.read8(p + O_NUMBER);
    cpu.write8(o + O_NUMBER, n);
    cpu.write32(o + O_FLAGS, 0xc8_0400);
    cpu.write32(o + O_DRAW, WORM_DRAW);
    let c = (cpu.read16(CHAR_BASE) as i16 as i32 as u32).wrapping_add(48);
    cpu.write16(o + 42, c as u16);
    cpu.write32(o + 20, 0);
    cpu.write32(o + 24, 0x8_0000);
    cpu.write32(o + 28, 0);
    cpu.write16(o + WORM_DYING, 0);
    cpu.write8(o + WORM_TOGGLE, 0);
    cpu.write8(o + WORM_TURN_STEP, step as u8);
    cpu.write32(o + WORM_TURNED, 0);
    cpu.write32(o + WORM_SPEED, 0x8000);
    cpu.write8(o + WORM_SHIFT_X, 0);
    cpu.write8(o + WORM_SHIFT_Y, 0);
    nested!(cpu, 0x0605_6294, actors::fill_bytes(cpu, o + WORM_TRAIL_X, 0, WORM_TRAIL));
    nested!(cpu, 0x0605_6294, actors::fill_bytes(cpu, o + WORM_TRAIL_Y, 0, WORM_TRAIL));
    let turn = cpu.read16(o + O_ANGLE) as i16 as i32 as u32;
    let turn = turn.wrapping_add(if (turn as i32) < 0 { 0xff } else { 0 });
    shift_8(cpu, turn);
    let r4 = cpu.reg(4);
    nested!(cpu, 0x0605_6294, actors::fill_bytes(cpu, o + WORM_TRAIL_TURN, r4 as i8 as i32 as u32, WORM_TRAIL));
    for i in 0..12u32 {
        let x = cpu.read32(o + 20);
        cpu.write32(o + WORM_LINKS_X + 4 * i, x);
        let y = cpu.read32(o + 24);
        cpu.write32(o + WORM_LINKS_Y + 4 * i, y);
    }
    cpu.write16(o + WORM_KIND, kind as u16);
    cpu.write8(o + WORM_DELAY, if kind as u16 == WORM_FAST as u16 { 28 } else { 48 });
}

// ---- the scatter, the orbiters, the fans and the rocks ---------------------------------------------

/// The scatter shots' fields: the count (+0x8c), the drift speeds (+0x8e, +0x90), the drift
/// (+0x80, +0x84), the frame counter (+0x88); the ring counter in the head (+0xdea, a byte) and
/// the base of the sixty degree steps.
const SCATTER_COUNT: u32 = 0x8c;
const SCATTER_SPEED: u32 = 0x8e;
const SCATTER_DRIFT: u32 = 0x80;
const SCATTER_FRAME: u32 = 0x88;
const SCATTER_STEP: u32 = 0x0609_8340;
const SCATTER_SCRIPT: u32 = 0x060a_d4d6;
const SCATTER_MAKE: u32 = 0x0609_8414;
const BOSS_RING_COUNT: u32 = 0xdea;
const BOSS_RING_SOUND: u32 = 94;

/// The head's scatter: six bullets (`bullet_set`) from the head's middle (`head_centre`, into
/// the stack at +0xb0), the angle toward the target (`angle_to_target`, 30 frames ahead) in the
/// description at +44, the speed 0x18000, and the extra turn (+54) going up by 60 degrees a bullet;
/// then a spark.
pub fn scatter_fire(cpu: &mut Cpu, o: u32) {
    let d = cpu.stack();
    let centre = d + 0xb0;
    nested!(cpu, 0x0609_731c, head_centre(cpu, o, centre));
    let n = cpu.read8(o + O_NUMBER);
    cpu.write8(d + 30, n);
    cpu.write32(d + 8, 0);
    let aim = d + 64;
    let v = cpu.read32(centre);
    cpu.write32(aim + 20, v);
    let v = cpu.read32(centre + 4);
    cpu.write32(aim + 24, v);
    let angle = nested!(cpu, 0x0605_6ad4, aim::angle_to_target(cpu, aim, 30));
    cpu.write16(d + 44, angle as u16);
    cpu.write16(d + 46, 1);
    cpu.write32(d + 48, 0x1_8000);
    cpu.write16(d + 54, 0);
    cpu.write16(d + 56, 0xfe00);
    cpu.write16(d + 58, 16);
    for _ in 0..6 {
        let v = cpu.read32(centre);
        cpu.write32(d, v);
        let v = cpu.read32(centre + 4);
        cpu.write32(d + 4, v);
        nested!(cpu, 0x0605_3db0, bullets::bullet_set(cpu, d));
        let a = cpu.read16(d + 54).wrapping_add(0x2aaa);
        cpu.write16(d + 54, a);
    }
    nested!(cpu, 0x0609_7644, head_spark_make(cpu, d));
}

/// A scatter shot's step: as the head's shard (`head_shard_step`) but the bullet check is the
/// one that turns it back (`bullet_ends` 1), the count (+0x8c) runs down while it adds the speeds
/// (+0x8e, +0x90) to the drift, and its frame (the sprite's byte +0x2f) is 91 plus the counter
/// (+0x88) over 4 mod 3.
pub fn scatter_step(cpu: &mut Cpu, o: u32) {
    let ended = nested!(cpu, 0x0605_3b10, bullets::bullet_ends(cpu, o, 1)) as u8;
    if ended != 0 {
        return;
    }
    let count = cpu.read16(o + SCATTER_COUNT) as i16 as i32;
    if count > 0 {
        cpu.write16(o + SCATTER_COUNT, (count - 1) as u16);
        let v = cpu.read32(o + SCATTER_DRIFT).wrapping_add(cpu.read16(o + SCATTER_SPEED) as i16 as i32 as u32);
        cpu.write32(o + SCATTER_DRIFT, v);
        let v = cpu.read32(o + SCATTER_DRIFT + 4).wrapping_add(cpu.read16(o + SCATTER_SPEED + 2) as i16 as i32 as u32);
        cpu.write32(o + SCATTER_DRIFT + 4, v);
    }
    let x = cpu.read32(o + 20).wrapping_add(cpu.read32(o + SCATTER_DRIFT));
    cpu.write32(o + 20, x);
    let y = cpu.read32(o + 24).wrapping_add(cpu.read32(o + SCATTER_DRIFT + 4));
    cpu.write32(o + 24, y);
    let a = cpu.read32(o + 32);
    let a = nested!(cpu, 0x0604_6168, actors::approach(cpu, a, 0x1_0000, 0x1800));
    cpu.write32(o + 32, a);
    let b = cpu.read32(o + 36);
    let b = nested!(cpu, 0x0604_6168, actors::approach(cpu, b, 0x1_0000, 0x1800));
    cpu.write32(o + 36, b);
    let k = cpu.read8(o + SCATTER_FRAME) as i8 as i32;
    let k = (k.wrapping_add(if k < 0 { 3 } else { 0 }) >> 2) as i8 as i32 as u32;
    let r = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, k, 3));
    cpu.write8(o + O_FRAME, r.wrapping_add(91) as u8);
    let n = cpu.read8(o + SCATTER_FRAME).wrapping_add(1);
    cpu.write8(o + SCATTER_FRAME, n);
}

/// A scatter shot made at the place `p` (x, y; its number a byte at +30) heading `angle`: an
/// object of 0xbc bytes (exec 0x0604d780, step 0x06098340, flags 0x400, sprite kind 0 and
/// frame 91 with the lift 8, colour the base plus 0x90, script 0x060ad4d6), the count 18, the
/// drift speeds the heading's sine and minus the cosine over 8, scales 0x18000 and 0x2000.
pub fn scatter_make(cpu: &mut Cpu, p: u32, angle: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, HEAD_SHARD_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, HEAD_SHARD_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + O_SCRIPT));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    let n = cpu.read8(p + 30);
    cpu.write8(o + O_NUMBER, n);
    cpu.write32(o + O_BEHAVIOUR, SCATTER_STEP);
    cpu.write32(o + O_FLAGS, 0x400);
    let x = cpu.read32(p);
    cpu.write32(o + O_POSITION, x);
    let y = cpu.read32(p + 4);
    cpu.write32(o + O_POSITION + 4, y);
    cpu.write32(o + O_POSITION + 8, 0);
    cpu.write8(o + 51, 8);
    cpu.write8(o + O_FRAME, 91);
    cpu.write8(o + O_KIND, 0);
    let c = (cpu.read16(CHAR_BASE) as i16 as i32 as u32).wrapping_add(0x90);
    cpu.write16(o + 42, c as u16);
    cpu.write16(o + 40, c as u16);
    cpu.write32(o + O_SCRIPT, SCATTER_SCRIPT);
    cpu.write8(o + 84, 1);
    cpu.write8(o + 85, 5);
    cpu.write16(o + O_ANGLE, angle as u16);
    cpu.write16(o + SCATTER_COUNT, 18);
    let a = cpu.read16(o + O_ANGLE) as i16 as i32 as u32;
    let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    let v = nested!(cpu, 0x0600_6a78, math::quotient(cpu, sin, 8));
    cpu.write16(o + SCATTER_SPEED, v as u16);
    let a = cpu.read16(o + O_ANGLE) as i16 as i32 as u32;
    let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    let v = nested!(cpu, 0x0600_6a78, math::quotient(cpu, cos.wrapping_neg(), 8));
    cpu.write16(o + SCATTER_SPEED + 2, v as u16);
    cpu.write32(o + 32, 0x1_8000);
    cpu.write32(o + 36, 0x2000);
}

/// A ring of scatter shots from the description `d` (the number at +30), the place already in it;
/// the head's counter (+0xdea) goes up and the shots go out at the head's turn (+0x30) plus the
/// offsets of the even set when the counter is even, else the odd.
fn scatter_ring(cpu: &mut Cpu, o: u32, d: u32, even: &[u32], odd: &[u32]) {
    let n = cpu.read8(o + BOSS_RING_COUNT).wrapping_add(1);
    cpu.write8(o + BOSS_RING_COUNT, n);
    let set = if n & 1 == 0 { even } else { odd };
    for &k in set {
        let turn = (cpu.read16(o + O_ANGLE) as i16 as i32 as u32).wrapping_add(k) & 0xffff;
        nested!(cpu, SCATTER_MAKE, scatter_make(cpu, d, turn));
    }
}

/// The scatter ring at the head's middle (`head_centre`, number from the head), the sound 94.
pub fn scatter_centre(cpu: &mut Cpu, o: u32) {
    let d = cpu.stack();
    nested!(cpu, 0x0609_731c, head_centre(cpu, o, d));
    let n = cpu.read8(o + O_NUMBER);
    cpu.write8(d + 30, n);
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, BOSS_RING_SOUND));
    scatter_ring(cpu, o, d, &[0x6000, 0x7000, 0x8000, 0x9000, 0xa000], &[0x6800, 0x7800, 0x8800, 0x9800]);
}

/// The scatter ring at the head's fire place (+0xdd8, +0xddc), the sound 94, the spacing
/// (+0xde4) 0xa000.
pub fn scatter_boss(cpu: &mut Cpu, o: u32) {
    let d = cpu.stack();
    let x = cpu.read32(o + BOSS_FIRE_AT);
    cpu.write32(d, x);
    let y = cpu.read32(o + BOSS_FIRE_AT + 4);
    cpu.write32(d + 4, y);
    let n = cpu.read8(o + O_NUMBER);
    cpu.write8(d + 30, n);
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, BOSS_RING_SOUND));
    scatter_ring(cpu, o, d, &[0xe000, 0xf000, 0, 0x1000, 0x2000], &[0xe800, 0xf800, 0x0800, 0x1800]);
    cpu.write32(o + BOSS_FIRE_SPACING, 0xa000);
}

/// An orbiter's fields: the owner (+0x6c), its angle around the owner (+0x8a), the spin it turns
/// by a step (a byte, +0x88), the distance (+0xac), the distance it eases to (+0xb4) and the
/// radial speed (+0xb0).
const ORBIT_OWNER: u32 = 108;
const ORBIT_SPIN: u32 = 0x88;
const ORBIT_ANGLE: u32 = 0x8a;
const ORBIT_DIST: u32 = 0xac;
const ORBIT_SPEED: u32 = 0xb0;
const ORBIT_TO: u32 = 0xb4;
const ORBIT_STEP: u32 = 0x0609_8708;
const ORBIT_DRAW: u32 = 0x0609_8948;
const ORBIT_SPEEDS: u32 = 0x060a_d520;
const ORBIT_SCRIPTS: [u32; 2] = [0x060a_d4de, 0x060a_d4fe];
const ORBIT_CHAR: u32 = KINDS + 0x45c;
const ORBIT_SOUND: u32 = 17;
const ORBIT_EFFECT: u32 = 0x0607_3044;

/// An orbiter's step. It ends when its owner (+0x6c) is past mode 111; or on a hit (0x400) or out of
/// life (+0x50): the sound 17 and an effect (drift_effect, the place, its turn, scale 2). Else once
/// its distance is within 4 of the one it eases to it turns toward the owner's side (the spin
/// times a difficulty's factor taken from the turn target +0x8a, the heading by a sixteenth of the
/// difference toward the turn plus or minus a quarter); then it sits at the owner's place plus its
/// distance along the angle (+0x8a), the distance easing to its aim by a share of the difference.
/// Its script alternates with the game frame, the hit flag 0x200 sets bit 0 of +0x7e, and every
/// fourth frame its frame (+0x2f) steps from 11 to 14 and back.
pub fn orbit_step(cpu: &mut Cpu, o: u32) {
    let d = cpu.stack();
    let owner = cpu.read32(o + ORBIT_OWNER);
    if cpu.read8(owner + O_MODE) as i8 > 111 {
        cpu.write8(o + O_MODE, 0x70);
        return;
    }
    let flags = cpu.read16(o + O_ARMS) as i16 as i32 as u32;
    if flags & 0x400 != 0 || cpu.read32(o + 80) as i32 <= 0 {
        cpu.write8(o + O_MODE, 0x70);
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, ORBIT_SOUND));
        cpu.write32(d + 60, ORBIT_EFFECT);
        cpu.write16(d + 32, 0x90);
        let v = cpu.read32(o + 20);
        cpu.write32(d, v);
        let v = cpu.read32(o + 24);
        cpu.write32(d + 4, v);
        let v = cpu.read32(o + 28);
        cpu.write32(d + 8, v);
        let turn = cpu.read16(o + O_ANGLE);
        cpu.write16(d + 24, turn);
        cpu.write32(d + 20, 0x2_0000);
        cpu.write32(d + 12, 0xa000);
        cpu.write32(d + 16, 0xe000);
        nested!(cpu, 0x0605_7454, actors::drift_effect(cpu, d));
        return;
    }
    if cpu.read32(o + ORBIT_DIST) as i32 >= cpu.read32(o + ORBIT_TO).wrapping_add(0xfffc_0000) as i32 {
        let level = cpu.read8(DIFFICULTY_AT) as i8 as i32 as u32;
        let factor = cpu.read8(ORBIT_SPEEDS.wrapping_add(level)) as u32;
        let spin = cpu.read8(o + ORBIT_SPIN) as i8 as i32 as u32;
        let angle = (cpu.read16(o + ORBIT_ANGLE) as i16 as i32 as u32).wrapping_sub(factor.wrapping_mul(spin));
        cpu.write16(o + ORBIT_ANGLE, angle as u16);
        let side = if cpu.read8(o + ORBIT_SPIN) as i8 > 0 { 0x4000 } else { 0xffff_c000 };
        let heading = (cpu.read16(o + O_ANGLE) as i16 as i32 as u32).wrapping_add(side);
        let at = cpu.read16(o + ORBIT_ANGLE) as i16 as i32 as u32;
        let diff = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, heading, at)) as i32;
        let diff = diff.wrapping_add(if diff < 0 { 15 } else { 0 }) >> 4;
        let turn = diff.wrapping_add(cpu.read16(o + O_ANGLE) as i16 as i32);
        cpu.write16(o + O_ANGLE, turn as u16);
    }
    let dist = cpu.read32(o + ORBIT_DIST);
    let a = cpu.read16(o + ORBIT_ANGLE) as i16 as i32 as u32;
    let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    let v = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, sin, dist));
    let x = cpu.read32(owner + 20).wrapping_add(v);
    cpu.write32(o + 20, x);
    let dist = cpu.read32(o + ORBIT_DIST).wrapping_neg();
    let a = cpu.read16(o + ORBIT_ANGLE) as i16 as i32 as u32;
    let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    let v = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, cos, dist));
    let y = cpu.read32(owner + 24).wrapping_add(v);
    cpu.write32(o + 24, y);
    let gap = cpu.read32(o + ORBIT_DIST).wrapping_sub(cpu.read32(o + ORBIT_TO));
    let step = nested!(cpu, 0x0600_6a78, math::quotient(cpu, gap, 0x100)) as i32;
    let step = step.wrapping_abs() as u32;
    let want = nested!(cpu, 0x0600_6a78, math::quotient(cpu, gap.wrapping_neg(), 16));
    let from = cpu.read32(o + ORBIT_SPEED);
    let speed = nested!(cpu, 0x0604_6168, actors::approach(cpu, from, want, step));
    cpu.write32(o + ORBIT_SPEED, speed);
    let dist = cpu.read32(o + ORBIT_DIST).wrapping_add(speed);
    cpu.write32(o + ORBIT_DIST, dist);
    let script = ORBIT_SCRIPTS[(cpu.read32(GAME_FRAMES) & 1 == 0) as usize];
    cpu.write32(o + O_SCRIPT, script);
    if cpu.read16(o + O_ARMS) as i16 as i32 as u32 & 0x200 != 0 {
        let f = cpu.read16(o + 126) as i16 as i32 as u32;
        cpu.write16(o + 126, (f | 1) as u16);
    }
    if cpu.read32(GAME_FRAMES) & 3 == 0 {
        let f = cpu.read8(o + O_FRAME).wrapping_add(1);
        cpu.write8(o + O_FRAME, f);
        if f > 13 {
            cpu.write8(o + O_FRAME, 11);
        }
    }
}

/// An orbiter's draw (its sprite `s`): the frame flags (+0x7e of the object) cleared of bit 1
/// and set again from bit 0; the sprite copied to a description (place, the sprite's bytes +31,
/// +30 and +35, the halfword +32), its halfword +24 the colour base plus 0xe0 when the flag bit
/// 0 is set, else the sprite's; drawn; then a flame at a random turn (a random 0x100 flag, the
/// frame 0x70 plus the frame counter mod 3, spread 0x7000 plus a random 0x1000, size 0x15800)
/// 16 times the unit vector of the turn (less 0x8000) from the object's place, in the colour
/// base plus 48.
pub fn orbit_draw(cpu: &mut Cpu, s: u32) {
    let d = cpu.stack();
    let o = s.wrapping_sub(0x10);
    let flags = s + 110;
    let f = cpu.read16(flags) as i16 as i32 as u32;
    if f & 2 != 0 {
        cpu.write16(flags, (f & 0xffff_fffc) as u16);
    }
    let f = cpu.read16(flags) as i16 as i32 as u32;
    if f & 1 != 0 {
        cpu.write16(flags, (f | 2) as u16);
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, d));
    let v = cpu.read32(o + 20);
    cpu.write32(d + 4, v);
    let v = cpu.read32(o + 24);
    cpu.write32(d + 8, v);
    let v = cpu.read32(o + 28);
    cpu.write32(d + 12, v);
    let v = cpu.read8(s + 31);
    cpu.write8(d + 31, v);
    let v = cpu.read8(s + 30);
    cpu.write8(d + 30, v);
    let v = cpu.read8(s + 35);
    cpu.write8(d + 35, v);
    let v = cpu.read16(s + 32);
    cpu.write16(d + 32, v);
    if cpu.read16(flags) & 1 != 0 {
        let c = (cpu.read16(CHAR_BASE) as i16 as i32 as u32).wrapping_add(0xe0);
        cpu.write16(d + 24, c as u16);
    } else {
        let v = cpu.read16(s + 24);
        cpu.write16(d + 24, v);
    }
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write32(d, r & 0x100);
    let loops = cpu.read32(LOOPS);
    let r = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, loops, 3));
    cpu.write8(d + 31, r.wrapping_add(0x70) as u8);
    cpu.write8(d + 30, 0);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1000));
    cpu.write32(d + 16, r.wrapping_add(0x7000));
    cpu.write32(d + 20, 0x1_5800);
    let turn = cpu.read16(d + 32).wrapping_add(0x8000);
    cpu.write16(d + 32, turn);
    let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, turn as i16 as i32 as u32));
    let x = cpu.read32(o + 20).wrapping_add(sin << 4);
    cpu.write32(d + 4, x);
    let turn = cpu.read16(d + 32) as i16 as i32 as u32;
    let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, turn));
    let y = cpu.read32(o + 24).wrapping_sub(cos << 4);
    cpu.write32(d + 8, y);
    let c = (cpu.read16(CHAR_BASE) as i16 as i32 as u32).wrapping_add(48);
    cpu.write16(d + 24, c as u16);
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
}

/// An orbiter made around the owner `p` at the angle `angle`, the distance `dist` and the spin
/// `spin` (a byte): an object of 0xbc bytes (exec 0x0604d780, step 0x06098708, draw 0x06098948,
/// flags 0xf00400, kind 69 with the frame 11 and the lift 7, colour the kind table's +0x45c,
/// script 0x060ad4de, life 10000) at the owner's place; the owner kept at +0x6c.
pub fn orbit_make(cpu: &mut Cpu, p: u32, angle: u32, dist: u32, spin: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, HEAD_SHARD_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, HEAD_SHARD_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + O_SCRIPT));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + O_BEHAVIOUR, ORBIT_STEP);
    let n = cpu.read8(p + O_NUMBER);
    cpu.write8(o + O_NUMBER, n);
    cpu.write32(o + O_FLAGS, 0xf0_0400);
    cpu.write32(o + O_DRAW, ORBIT_DRAW);
    let x = cpu.read32(p + 20);
    cpu.write32(o + O_POSITION, x);
    let y = cpu.read32(p + 24);
    cpu.write32(o + O_POSITION + 4, y);
    cpu.write32(o + O_POSITION + 8, 0);
    cpu.write8(o + 51, 7);
    cpu.write8(o + O_FRAME, 11);
    cpu.write8(o + O_KIND, 69);
    let c = cpu.read16(ORBIT_CHAR);
    cpu.write16(o + 42, c);
    cpu.write16(o + 40, c);
    cpu.write32(o + O_SCRIPT, ORBIT_SCRIPTS[0]);
    cpu.write8(o + 84, 1);
    cpu.write32(o + 80, 10000);
    cpu.write16(o + O_ANGLE, angle as u16);
    cpu.write16(o + ORBIT_ANGLE, angle as u16);
    cpu.write32(o + ORBIT_OWNER, p);
    cpu.write8(o + ORBIT_SPIN, spin as u8);
    cpu.write32(o + ORBIT_TO, dist);
}

/// A fan's fields: its radius (+0x6c, a long; negative until it is told to), the head it spreads
/// from (+0x70).
const SPREAD_RADIUS: u32 = 108;
const SPREAD_HEAD: u32 = 112;
const SPREAD_STEP: u32 = 0x0609_8bac;
const ORBIT_MAKE: u32 = 0x0609_8aa0;
const SPREAD_COUNTS: u32 = 0x060a_d528;
const SPREAD_SOUND: u32 = 95;

/// A fan's step. With a radius it goes out along its turn (the sine and cosine times the radius
/// added to the place, as `orbit_step` does) while the radius eases to 0xc000 by 0x1000; with a
/// negative one it sits at the head's middle (`head_centre`) and the radius eases to 0 by 0x300.
/// Past |x| 0xf80000 or |y| 0xc00000 the mode (+3) is 0x70.
pub fn fan_step(cpu: &mut Cpu, o: u32) {
    let radius = cpu.read32(o + SPREAD_RADIUS);
    let next = if radius as i32 >= 0 {
        let a = cpu.read16(o + O_ANGLE) as i16 as i32 as u32;
        let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
        let v = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, sin, radius));
        let x = cpu.read32(o + 20).wrapping_add(v);
        cpu.write32(o + 20, x);
        let radius = cpu.read32(o + SPREAD_RADIUS);
        let a = cpu.read16(o + O_ANGLE) as i16 as i32 as u32;
        let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
        let v = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, cos, radius));
        let y = cpu.read32(o + 24).wrapping_sub(v);
        cpu.write32(o + 24, y);
        let from = cpu.read32(o + SPREAD_RADIUS);
        nested!(cpu, 0x0604_6168, actors::approach(cpu, from, 0xc000, 0x1000))
    } else {
        let head = cpu.read32(o + SPREAD_HEAD);
        nested!(cpu, 0x0609_731c, head_centre(cpu, head, o + 20));
        let from = cpu.read32(o + SPREAD_RADIUS);
        nested!(cpu, 0x0604_6168, actors::approach(cpu, from, 0, 0x300))
    };
    cpu.write32(o + SPREAD_RADIUS, next);
    let x = cpu.read32(o + 20) as i32;
    let y = cpu.read32(o + 24) as i32;
    if x.wrapping_abs() > 0xf8_0000 || y.wrapping_abs() > 0xc0_0000 {
        cpu.write8(o + O_MODE, 0x70);
    }
}

/// A fan made at the head `p` with the spin `spin` (a byte): an object of 0x74 bytes (exec
/// 0x0604d780, step 0x06098bac, flags 0x80080400) at the head's middle, its turn the angle toward
/// the target, the radius -0x10000; and orbiters made around it (`orbit_make`) by a draw of four:
/// as many as the difficulty's count (0x060ad528) at steps of 0x2000 and the distance 0x380000;
/// four at 0x4000 steps, 0x300000 and twice the spin; six at fixed angles, 0x380000; or three at
/// 0x5555 steps, 0x200000 and three times the spin. The sound 95 at the end.
pub fn fan_make(cpu: &mut Cpu, p: u32, spin: u32) {
    let spin = spin as i8 as i32 as u32;
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0x74)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, HEAD_SHARD_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + O_SCRIPT));
    cpu.write32(o + O_BEHAVIOUR, SPREAD_STEP);
    let n = cpu.read8(p + O_NUMBER);
    cpu.write8(o + O_NUMBER, n);
    cpu.write32(o + O_FLAGS, 0x8008_0400);
    cpu.write32(o + SPREAD_HEAD, p);
    nested!(cpu, 0x0609_731c, head_centre(cpu, p, o + 20));
    cpu.write32(o + 28, 0);
    let a = nested!(cpu, 0x0605_6ad4, aim::angle_to_target(cpu, o, 30));
    cpu.write16(o + O_ANGLE, a as u16);
    cpu.write32(o + SPREAD_RADIUS, 0xffff_0000);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    match r & 3 {
        0 => {
            let level = cpu.read8(DIFFICULTY_AT) as i8 as i32 as u32;
            let count = cpu.read8(SPREAD_COUNTS.wrapping_add(level));
            let mut i = 0u8;
            while i < count {
                let angle = (u32::from(i) << 13) & 0xffff;
                nested!(cpu, ORBIT_MAKE, orbit_make(cpu, o, angle, 0x38_0000, spin));
                i = i.wrapping_add(1);
            }
        }
        1 => {
            for i in 0..4u32 {
                let angle = (i << 14) & 0xffff;
                let twice = spin.wrapping_add(spin) as i8 as i32 as u32;
                nested!(cpu, ORBIT_MAKE, orbit_make(cpu, o, angle, 0x30_0000, twice));
            }
        }
        2 => {
            for angle in [0, 0x2000, 0x4000, 0x8000, 0xa000, 0xc000] {
                nested!(cpu, ORBIT_MAKE, orbit_make(cpu, o, angle, 0x38_0000, spin));
            }
        }
        _ => {
            let triple = spin.wrapping_mul(3) as i8 as i32 as u32;
            for i in 0..3u32 {
                let angle = i.wrapping_mul(0x5555) & 0xffff;
                nested!(cpu, ORBIT_MAKE, orbit_make(cpu, o, angle, 0x20_0000, triple));
            }
        }
    }
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, SPREAD_SOUND));
}

/// A rock's fields: its state (a byte, +0x6c), the speeds x and y (+0x70, +0x74), the length of
/// its trail (+0x78, grows to 0x10000 and fades) and the hit boxes copied from the overlay's
/// table (from +0x7c; the trail's length goes into the first box's halfword +4 and bytes +6, +7).
const ROCK_STATE: u32 = 108;
const ROCK_SPEED: u32 = 112;
const ROCK_TAIL: u32 = 120;
const ROCK_BOXES_AT: u32 = 124;
const ROCK_BOXES: u32 = 0x060a_d52e;
const ROCK_STEP: u32 = 0x0609_8e5c;
const ROCK_DRAW: u32 = 0x0609_9068;
const ROCK_MAKE: u32 = 0x0609_924c;
const ROCK_SIZE: u32 = 0x94;
const ROCK_SOUND: u32 = 17;
const ROCK_BURST_SOUND: u32 = 93;

/// A rock's step. Past |x| 0xc00000 it ends (mode +3 to 0x70); out of life (+0x50) it ends in an
/// effect (the sound 17, `drift_effect`). Else by its state: 0, the x speed eases to 0 and the y
/// speed to 0x40000 by 0x1000, state 1 when that reaches 0; 1, the tail (+0x78) eases to 0x10000
/// as well and once the rock is past y 0x900000 the speeds are 0 and the state 2; 2, after 62
/// frames the tail eases to 0 by 0x200 and when it is 0 the rock ends. Then the place moves by
/// the speeds; the first hit box takes the trail's length over 0x8000 (as bytes, past that;
/// else 0); the hit flag 0x200 sets bit 0 of +0x6e.
pub fn rock_step(cpu: &mut Cpu, o: u32) {
    let d = cpu.stack();
    let x = cpu.read32(o + 20) as i32;
    if x.wrapping_abs() > 0xc0_0000 {
        cpu.write8(o + O_MODE, 0x70);
        return;
    }
    if cpu.read32(o + 80) as i32 <= 0 {
        cpu.write8(o + O_MODE, 0x70);
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, ROCK_SOUND));
        cpu.write32(d + 60, ORBIT_EFFECT);
        cpu.write16(d + 32, 0x90);
        let v = cpu.read32(o + 20);
        cpu.write32(d, v);
        let v = cpu.read32(o + 24);
        cpu.write32(d + 4, v);
        let v = cpu.read32(o + 28).wrapping_add(0xfff0_0000);
        cpu.write32(d + 8, v);
        let turn = cpu.read16(o + O_ANGLE);
        cpu.write16(d + 24, turn);
        cpu.write32(d + 20, 0x2_0000);
        cpu.write32(d + 12, 0xa000);
        cpu.write32(d + 16, 0xe000);
        nested!(cpu, 0x0605_7454, actors::drift_effect(cpu, d));
        return;
    }
    let state = cpu.read8(o + ROCK_STATE);
    match state {
        0 => {
            let from = cpu.read32(o + ROCK_SPEED);
            let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, from, 0, 0x1000));
            cpu.write32(o + ROCK_SPEED, v);
            let from = cpu.read32(o + ROCK_SPEED + 4);
            let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, from, 0x4_0000, 0x1000));
            cpu.write32(o + ROCK_SPEED + 4, v);
            if v == 0 {
                cpu.write8(o + ROCK_STATE, state.wrapping_add(1));
            }
        }
        1 => {
            let from = cpu.read32(o + ROCK_TAIL);
            let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, from, 0x1_0000, 0x1000));
            cpu.write32(o + ROCK_TAIL, v);
            let from = cpu.read32(o + ROCK_SPEED);
            let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, from, 0, 0x1000));
            cpu.write32(o + ROCK_SPEED, v);
            let from = cpu.read32(o + ROCK_SPEED + 4);
            let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, from, 0x4_0000, 0x1000));
            cpu.write32(o + ROCK_SPEED + 4, v);
            if cpu.read32(o + 24) as i32 > 0x90_0000 {
                cpu.write8(o + ROCK_STATE, state.wrapping_add(1));
                cpu.write32(o + ROCK_SPEED, 0);
                cpu.write32(o + ROCK_SPEED + 4, 0);
            }
        }
        _ => {
            let n = state.wrapping_add(1);
            cpu.write8(o + ROCK_STATE, n);
            if n > 62 {
                let from = cpu.read32(o + ROCK_TAIL);
                let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, from, 0, 0x200));
                cpu.write32(o + ROCK_TAIL, v);
            }
            if cpu.read32(o + ROCK_TAIL) == 0 {
                cpu.write8(o + O_MODE, 0x70);
                return;
            }
        }
    }
    let x = cpu.read32(o + 20).wrapping_add(cpu.read32(o + ROCK_SPEED));
    cpu.write32(o + 20, x);
    let y = cpu.read32(o + 24).wrapping_add(cpu.read32(o + ROCK_SPEED + 4));
    cpu.write32(o + 24, y);
    let tail = cpu.read32(o + ROCK_TAIL);
    let at = o + ROCK_BOXES_AT;
    if tail as i32 > 0x8000 {
        cpu.write16(at + 4, 0xff60);
        cpu.write8(at + 7, 0x90);
        let v = tail.wrapping_add(if (tail as i32) < 0 { 0x7fff } else { 0 });
        cpu.call(0x0600_8156, &[v]);
        let r4 = cpu.reg(4);
        cpu.write8(at + 6, r4 as u8);
    } else {
        cpu.write16(at + 4, 0);
        cpu.write8(at + 7, 0);
        cpu.write8(at + 6, 0);
    }
    if cpu.read16(o + O_ARMS) as i16 as i32 as u32 & 0x200 != 0 {
        let f = cpu.read16(o + 110) as i16 as i32 as u32;
        cpu.write16(o + 110, (f | 1) as u16);
    }
}

/// A rock's draw (its sprite `s`): the frame flags (+0x6e of the object) cleared of bit 1 and set
/// again from bit 0; a description from sprite_init with the place (x of the object, y below it),
/// the colour 24. With a trail (+0x78) a sprite for each step down it (the frame 94 plus the
/// counter's low bits, going up by 0x400000 a step, the speed the trail less a fraction by the
/// frame counter); then a dust at a random side, the rock's own picture.
pub fn rock_draw(cpu: &mut Cpu, s: u32) {
    let d = cpu.stack();
    let o = s.wrapping_sub(0x10);
    let flags = s + 94;
    let f = cpu.read16(flags) as i16 as i32 as u32;
    if f & 2 != 0 {
        cpu.write16(flags, (f & 0xffff_fffc) as u16);
    }
    let f = cpu.read16(flags) as i16 as i32 as u32;
    if f & 1 != 0 {
        cpu.write16(flags, (f | 2) as u16);
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, d));
    cpu.write8(d + 30, 0);
    let v = cpu.read32(o + 20);
    cpu.write32(d + 4, v);
    cpu.write32(d + 12, 0);
    cpu.write8(d + 35, 24);
    let tail = cpu.read32(o + ROCK_TAIL);
    if tail != 0 {
        cpu.write32(d, if tail as i32 > 0x7fff { 32 } else { 0x120 });
        let y = cpu.read32(o + 24).wrapping_add(0xffd8_0000);
        cpu.write32(d + 8, y);
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
        let base = cpu.read16(CHAR_BASE) as i16 as i32 as u32;
        cpu.write16(d + 24, base.wrapping_add(if r & 1 != 0 { 48 } else { 32 }) as u16);
        let loops = cpu.read32(LOOPS);
        let tail = cpu.read32(o + ROCK_TAIL);
        let cut = (loops & 3).wrapping_mul(tail) >> 4;
        cpu.write32(d + 16, tail.wrapping_sub(cut));
        cpu.write32(d + 20, 0x2_0000);
        let mut k = (loops >> 1) as u8;
        if cpu.read32(d + 8) as i32 > 0xff40_0000u32 as i32 {
            loop {
                cpu.write8(d + 31, 94 + (k & 3));
                nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
                let y = cpu.read32(d + 8).wrapping_add(0xffc0_0000);
                k = k.wrapping_add(1);
                cpu.write32(d + 8, y);
                if y as i32 <= 0xff40_0000u32 as i32 {
                    break;
                }
            }
        }
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
        cpu.write32(d, r & 0x100);
        let low = cpu.read8(LOOPS + 3) as u32 & 1;
        cpu.write8(d + 31, low.wrapping_sub(61) as u8);
        let tail = cpu.read32(o + ROCK_TAIL);
        let half = tail.wrapping_add(if (tail as i32) < 0 { 3 } else { 0 }) as i32 >> 2;
        let loops = cpu.read32(LOOPS);
        let more = (loops & 3).wrapping_mul(tail) >> 4;
        cpu.write32(d + 16, (half as u32).wrapping_add(more));
        cpu.write32(d + 20, 0x8000);
        let y = cpu.read32(o + 24).wrapping_add(0xffe8_0000);
        cpu.write32(d + 8, y);
        nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
    }
    cpu.write32(d, 32);
    let y = cpu.read32(o + 24);
    cpu.write32(d + 8, y);
    cpu.write8(d + 31, 14);
    cpu.write8(d + 30, 69);
    if cpu.read16(o + 110) & 1 != 0 {
        let c = (cpu.read16(CHAR_BASE) as i16 as i32 as u32).wrapping_add(0xe0);
        cpu.write16(d + 24, c as u16);
    } else {
        let c = cpu.read16(ORBIT_CHAR);
        cpu.write16(d + 24, c);
    }
    cpu.write32(d + 16, 0x1_0000);
    cpu.write32(d + 20, 0x1_0000);
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
}

/// A rock made at the head `p`'s fire place (+0xdd8, +0xddc) with the speeds `dx` and `dy`: an
/// object of 0x94 bytes (exec 0x0604d780, step 0x06098e5c, draw 0x06099068, flags 0xf00400,
/// kind 0, life 5000, the hit boxes at +0x7c from the overlay's table 0x060ad52e).
pub fn rock_make(cpu: &mut Cpu, p: u32, dx: u32, dy: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, ROCK_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, HEAD_SHARD_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + O_FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + O_SCRIPT));
    cpu.write32(o + O_BEHAVIOUR, ROCK_STEP);
    let n = cpu.read8(p + O_NUMBER);
    cpu.write8(o + O_NUMBER, n);
    cpu.write32(o + O_FLAGS, 0xf0_0400);
    cpu.write32(o + O_DRAW, ROCK_DRAW);
    cpu.write8(o + O_KIND, 0);
    let x = cpu.read32(p + BOSS_FIRE_AT);
    cpu.write32(o + O_POSITION, x);
    let y = cpu.read32(p + BOSS_FIRE_AT + 4);
    cpu.write32(o + O_POSITION + 4, y);
    cpu.write32(o + O_POSITION + 8, 0);
    cpu.write8(o + 84, 1);
    cpu.write32(o + 80, 5000);
    cpu.write8(o + ROCK_STATE, 0);
    cpu.write16(o + 110, 0);
    cpu.write32(o + ROCK_SPEED, dx);
    cpu.write32(o + ROCK_SPEED + 4, dy);
    cpu.write32(o + ROCK_TAIL, 0);
    cpu.write32(o + O_SCRIPT, o + ROCK_BOXES_AT);
    nested!(cpu, 0x0605_6440, hits::boxes_copy(cpu, o + ROCK_BOXES_AT, ROCK_BOXES));
}

/// A burst of rocks from the head `o`, the set by the low two bits of its byte +0xdeb (1 or 2:
/// four rocks; else three): the spacing (+0xde4) set to 0xe000 first, the byte stepped and the
/// sound 93 at the end. Each rock's speeds are `(dx, dy)`.
pub fn rock_burst(cpu: &mut Cpu, o: u32) {
    cpu.write32(o + BOSS_FIRE_SPACING, 0xe000);
    let rocks: &[(u32, u32)] = match cpu.read8(o + 0xdeb) & 3 {
        1 => &[(0x3_2000, 0xfffe_0000), (0x2_4000, 0xfffe_c000), (0xfffd_c000, 0xfffe_c000), (0xfffc_e000, 0xfffe_0000)],
        2 => &[(0x3_2000, 0xffff_0000), (0x1_8000, 0xfffd_8000), (0xfffe_8000, 0xfffd_8000), (0xfffc_e000, 0xffff_0000)],
        _ => &[(0x2_8000, 0xfffe_8000), (0, 0xfffd_e000), (0xfffd_8000, 0xfffe_8000)],
    };
    for &(dx, dy) in rocks {
        nested!(cpu, ROCK_MAKE, rock_make(cpu, o, dx, dy));
    }
    let n = cpu.read8(o + 0xdeb).wrapping_add(1);
    cpu.write8(o + 0xdeb, n);
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, ROCK_BURST_SOUND));
}

// ---- the head's parts moved with the view ----------------------------------------------------------

/// The head's parts moved by `d` in y: its place (+24), the nine parts' (the longs from +0x668), the
/// ten shots' (from +0xa54, where the halfword at +0xa18 + 2 n is not 0), and the limit at +0x9f8 less `d`.
pub fn parts_shift_y(cpu: &mut Cpu, o: u32, d: u32) {
    let y = cpu.read32(o + 24).wrapping_add(d);
    cpu.write32(o + 24, y);
    for i in 0..9u32 {
        let v = cpu.read32(o + 0x668 + 4 * i).wrapping_add(d);
        cpu.write32(o + 0x668 + 4 * i, v);
    }
    for i in 0..10u32 {
        if cpu.read16(o + 0xa18 + 2 * i) != 0 {
            let v = cpu.read32(o + 0xa54 + 4 * i).wrapping_add(d);
            cpu.write32(o + 0xa54 + 4 * i, v);
        }
    }
    let v = cpu.read32(o + 0x9f8).wrapping_sub(d);
    cpu.write32(o + 0x9f8, v);
}

/// The head's world kept near the middle: when its x passes 0x20000000 everything is moved by
/// -0x40000000 (0x06099420, the same as `parts_shift_y` in x), below -0x20000000 by 0x40000000;
/// when its y passes 0x04000000 by -0x08000000, below -0x04000000 by 0x08000000.
pub fn parts_wrap(cpu: &mut Cpu, o: u32) {
    if cpu.read32(o + 20) as i32 > 0x2000_0000 {
        cpu.call_returning(0x0609_9518, 0x0609_9420, &[o, 0xc000_0000]);
    }
    if (cpu.read32(o + 20) as i32) < -0x2000_0000 {
        cpu.call_returning(0x0609_9528, 0x0609_9420, &[o, 0x4000_0000]);
    }
    if cpu.read32(o + 24) as i32 > 0x0400_0000 {
        nested!(cpu, 0x0609_9490, parts_shift_y(cpu, o, 0xf800_0000));
    }
    if (cpu.read32(o + 24) as i32) < -0x0400_0000 {
        nested!(cpu, 0x0609_9490, parts_shift_y(cpu, o, 0x0800_0000));
    }
}
