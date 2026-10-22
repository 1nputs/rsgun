//! STG01.BIN's code, which the stage 3 mode runs in the overlay window (the stage played first;
//! its screen data is SCRDAT31 and SCRDAT32): its mode's code, its enemies and scenery, its
//! scrolls.

use super::globals::mode::DEMO10;
use super::globals::{BIOS_SETTING, GAME_FRAMES, GAME_MODE, GROUND_DY_AT};
use super::globals::{GAME_FLAGS, HELD, LOOPS, PLAY_DATA, PLAY_DATA_SIZE};
use super::bullets::{AS_ANGLE, AS_FLAGS, AS_POSITION, AS_SHOOTER, AS_SPEED};
use super::{actors, aim, bg, bullets, camera, clib, director, dma, effects, explosions, files, gouraud, ground, hits, hud, kinds, math, models, nested, objects, orbits, palette, parts, player, results, ribbons, rings, screen, sgl, sound, sprites, states, system, text, timeline, Cpu};

/// The director's slots the mode uses (states.rs's): its step, the load (a local written back
/// each time), the settings kept (states::keep_mode_settings: the buffer +0x6c, the room's top
/// +0x70, VDP1's next character place +0x74, its next character and colour +0x78 and +0x7c), the
/// second file's size (+0x80), the demo's input (+0x54), the mode's music (+0xa4).
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
/// The stages' screen data files (SCRDAT31.BIN, SCRDAT21.BIN ...): this stage's two.
const SCREEN_FILES: u32 = 0x0607_35b0;
const SCREEN_FILE_1: u32 = SCREEN_FILES;
const SCREEN_FILE_2: u32 = SCREEN_FILES + 0x1c;
/// The sound data files loaded at once (sound::load_sound_file), the defaults' set-up after the
/// first, and the music set after the third.
const SOUND_FILES: [u32; 5] = [1, 5, 4, 0x19, 0x18];
/// The kinds put in place (kinds.rs): three model kinds (the first's characters below 0x7fef8
/// by its file's size), then two sprite kinds that take kind 12's first colour.
const MODEL_KINDS: [u32; 3] = [0x1f, 0x21, 0x25];
const SPRITE_KINDS: [u32; 2] = [0x1b, 0x1c];
const KINDS: u32 = 0x0607_41f0;
const K_SIZE: u32 = 16;
const K_COLOUR: u32 = 0xc;
const COLOUR_KIND: u32 = 12;
const FILE_SIZE: u32 = 12;
const VDP1_TOP: u32 = 0x7_fef8;
/// The stage's scroll table, its timeline, its stage sound effects; the demo's input files
/// (by the BIOS's setting) and the modes after it.
const SCROLLS: u32 = 0x060a_36b8;
const EVENTS: u32 = 0x060a_1110;
const SOUND_EFFECTS: u32 = 0x060a_43e0;
const STAGE_SE_TABLE: u32 = 0x0600_edd4;
const NEXT_MODE: u32 = 0x0600_ed70;
const DEMO_FILES: [u32; 2] = [0x0600_8c24, 0x0600_8bf4]; // "DEMO1W.BIN", "DEMO1.BIN"
const AFTER_DEMO: u8 = 0;
const AFTER_PLAY: u8 = 8;

fn kind(k: u32) -> u32 {
    KINDS + K_SIZE * k
}

/// The mode's code (the director calls it each frame until it gives 1): the settings kept, the
/// sound data loaded, the kinds read into the buffer and put in place (each past the last), the
/// first screen data file loaded into the buffer; once DMA is idle the second, below the screen's
/// room off the place, and the screen started; once in, the stage data set from it; a frame; the
/// timeline made (in the demo, its input read after the buffer), the settings given back; 1.
pub fn stg01_mode(cpu: &mut Cpu, game: u32) -> u32 {
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
                    cpu.write16(game + MUSIC, 1);
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
                    let room = nested!(cpu, 0x0605_fe78, states::stage_table(cpu, 0));
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
            if cpu.read8(GAME_MODE) == DEMO10 {
                let buffer = cpu.read32(game + KEPT);
                cpu.write32(game + DEMO_INPUT, buffer);
                let name = DEMO_FILES[((cpu.read32(BIOS_SETTING) >> 12) & 0xF == 1) as usize];
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

/// Step 9's kinds: each read into the buffer and put in place in VDP1's VRAM (the first model
/// kind's characters below 0x7fef8 by its file's size, the rest at the place the last left), the
/// next character on by each kind's count (+6), the next colour by the third model kind's
/// (+0x16); the sprite kinds take kind 12's first colour; the buffer moved past each (its file's
/// first long).
fn install_kinds(cpu: &mut Cpu, game: u32) {
    for (i, k) in MODEL_KINDS.into_iter().enumerate() {
        let buffer = cpu.read32(game + KEPT);
        nested!(cpu, 0x0604_58cc, kinds::model_kind_read(cpu, k, buffer));
        let (first, colour) = next_char_colour(cpu, game);
        if i == 0 {
            let file = cpu.read32(kind(k));
            let vram = VDP1_TOP.wrapping_sub(cpu.read32(file + FILE_SIZE));
            nested!(cpu, 0x0604_5950, kinds::model_kind_install(cpu, k, vram, first, colour));
        } else {
            let vram = cpu.read32(game + VDP1_PLACE);
            let next = nested!(cpu, 0x0604_5950, kinds::model_kind_install(cpu, k, vram, first, colour));
            cpu.write32(game + VDP1_PLACE, next);
        }
        advance(cpu, game, k, i == 2);
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

// ---- the scenery's models ------------------------------------------------------------------------

/// SGL's matrix stack and its uses (called as the SH-2 code calls them: SGL's asm reads the
/// registers its callers load).
const SL_PUSH_MATRIX: u32 = 0x0606_71cc;
const SL_POP_MATRIX: u32 = 0x0606_7200;
const SL_TRANSLATE: u32 = 0x0606_721c;
const SL_ROT_X: u32 = 0x0606_72ec;
const SL_ROT_Y: u32 = 0x0606_736c;
const SL_ROT_Z: u32 = 0x0606_726c;
const SL_SCALE: u32 = 0x0606_73ec;
const SL_PUT_POLYGON: u32 = 0x0606_745c;
const SL_CALC_POINT: u32 = 0x0606_db2c;
const SL_GET_MATRIX: u32 = 0x0606_db9c;
/// The scenery's frame of reference (bg.rs's MOVER): its tilt about x (a word) and its place up
/// and along (+0x10, +0x14); the camera (camera.rs), whose eye's goal on z (+0x1c) points are
/// pushed along by.
const MOVER: u32 = 0x0607_4810;
const MOVER_TILT: u32 = 0x00;
const MOVER_Y: u32 = 0x10;
const MOVER_Z: u32 = 0x14;
const CAMERA: u32 = 0x0607_46e4;
const CAMERA_EYE_TO_Z: u32 = 0x1c;

/// The scenery's frame on the matrix stack: pushed, moved by the mover's place, tilted.
fn scenery_frame(cpu: &mut Cpu) {
    cpu.call(SL_PUSH_MATRIX, &[]);
    let (y, z) = (cpu.read32(MOVER + MOVER_Y), cpu.read32(MOVER + MOVER_Z));
    cpu.call(SL_TRANSLATE, &[0, y, z]);
    let tilt = cpu.read16(MOVER + MOVER_TILT) as i16 as i32 as u32;
    cpu.call(SL_ROT_X, &[tilt]);
}

/// The point `p` (x, y, z) in the scenery's frame to the screen's, in place, then on along z by
/// the camera's eye's goal.
pub fn scenery_point(cpu: &mut Cpu, p: u32) {
    scenery_frame(cpu);
    let [x, y, z] = [0, 4, 8].map(|k| cpu.read32(p + k));
    cpu.call(SL_CALC_POINT, &[x, y, z, p]);
    let camera = cpu.read32(CAMERA);
    let z = cpu.read32(p + 8).wrapping_add(cpu.read32(camera + CAMERA_EYE_TO_Z));
    cpu.write32(p + 8, z);
    cpu.call(SL_POP_MATRIX, &[]);
}

/// A scenery model actor's draw (its sprite `s`, the actor 0x10 before it): in the scenery's
/// frame, moved to its place in it (+0xd8), untilted, turned (z, x, y, each if set), scaled, its
/// model (the flashing one while its hit flags' bit 0 is set) put; its boxes; the matrix kept
/// (+0xe4). The matrix is not popped.
const ACTOR: u32 = 0x10;
const PLACE_IN_SCENERY: u32 = 0xd8;
const TURN_Z: u32 = 0x30;
const TURN_X: u32 = 0x2c;
const TURN_Y: u32 = 0x2e;
const SCALES: u32 = 0x20;
const MODELS: u32 = 0x44;
const HIT_FLAGS: u32 = 0x56;
const FLASH: u16 = 1;
const KEPT_MATRIX: u32 = 0xe4;

pub fn scenery_model_draw(cpu: &mut Cpu, s: u32) {
    let actor = s.wrapping_sub(ACTOR);
    scenery_frame(cpu);
    let p = [0, 4, 8].map(|k| cpu.read32(actor + PLACE_IN_SCENERY + k));
    cpu.call(SL_TRANSLATE, &p);
    let untilt = (cpu.read16(MOVER + MOVER_TILT) as i16).wrapping_neg() as i32 as u32;
    cpu.call(SL_ROT_X, &[untilt]);
    for (field, rot) in [(TURN_Z, SL_ROT_Z), (TURN_X, SL_ROT_X), (TURN_Y, SL_ROT_Y)] {
        let a = cpu.read16(actor + field) as i16;
        if a != 0 {
            cpu.call(rot, &[a as i32 as u32]);
        }
    }
    let scales = [0, 4, 8].map(|k| cpu.read32(actor + SCALES + k));
    cpu.call(SL_SCALE, &scales);
    let flashing = (cpu.read16(actor + HIT_FLAGS) & FLASH) as u32;
    let models = cpu.read32(actor + MODELS);
    let model = cpu.read32(models + 4 * flashing);
    cpu.call(SL_PUT_POLYGON, &[model]);
    nested!(cpu, 0x0605_29ac, models::model_boxes(cpu, actor));
    cpu.call(SL_GET_MATRIX, &[actor + KEPT_MATRIX]);
}

// ---- debris ---------------------------------------------------------------------------------------

/// A piece of debris (a plain model actor): its step (a byte), whether it ends in a burst (+0x3d),
/// a count of frames, its speed (x, y, z) and z's fall, its turns' speeds (words). Its position
/// (+0x14) and turns (+0x2c-0x30), number (+0x32).
const POSITION: u32 = 0x14;
const NUMBER: u32 = 0x32;
const BYTE_33: u32 = 0x33;
const MODEL: u32 = 0x34;
const BEHAVIOUR: u32 = 0x38;
const STEP: u32 = 0x3c;
const BURSTS: u32 = 0x3d;
const COUNT: u32 = 0x40;
const SPEED: u32 = 0x44;
const FALL: u32 = 0x50;
const SPIN: u32 = 0x60;
const OBJ_MODE: u32 = 3;
const TO_STOP: u8 = b'p';
const CHAR_BASE: u32 = 0x0607_41fc;
/// The smoke it leaves (an effect: colour 0x90 past the characters, flags 3, linked to these
/// animations), and its sounds (12-14 falling, 16-18 at the end).
const SMOKE_COLOUR: u16 = 0x90;
const SMOKE_FLAGS: u32 = 3;
const SMOKE_TRAIL: u32 = 0x0607_2bbc;
const SMOKE_END: u32 = 0x0607_2be0;
const FALL_SOUND: i16 = 12;
const END_SOUND: i16 = 16;

/// The effect `fx` (on the stack) of a smoke puff at (x, y, z), scaled `scale` both ways, at a
/// random angle, with `link`'s animation, of `number`'s.
#[allow(clippy::too_many_arguments)]
fn smoke(cpu: &mut Cpu, fx: u32, x: u32, y: u32, z: u32, scale: u32, number: u8, link: u32, rel: u32) {
    cpu.write32(fx + effects::X, x);
    cpu.write32(fx + effects::X + 4, y);
    cpu.write32(fx + effects::X + 8, z);
    cpu.write32(fx + effects::SCALE_Y, scale);
    cpu.write32(fx + effects::SCALE_X, scale);
    cpu.write32(fx + effects::FLAGS_LONG, SMOKE_FLAGS);
    let angle = nested!(cpu, 0x0604_5d90, math::random(cpu, 0));
    cpu.write16(fx + effects::ANGLE, angle as u16);
    cpu.write16(fx + effects::FRAMES, 0);
    cpu.write8(fx + effects::FRAMES_2, 0);
    cpu.write8(fx + effects::KIND, 0);
    let colour = cpu.read16(CHAR_BASE).wrapping_add(SMOKE_COLOUR);
    cpu.write16(fx + effects::COLOUR, colour);
    cpu.write8(fx + effects::PLAYER, number);
    cpu.write32(fx + effects::LINK, link);
    cpu.write32(fx + effects::RELATIVE_3C, rel);
    nested!(cpu, 0x0604_e1cc, effects::effect_set(cpu, fx));
}

/// One of the sounds `base` to `base + 2`, at random (the C code passes x too, which se_set does
/// not take).
fn debris_sound(cpu: &mut Cpu, base: i16) {
    let n = nested!(cpu, 0x0604_5d90, math::random(cpu, 3)) as i32;
    let se = (n.wrapping_abs() as i16).wrapping_add(base) as i32 as u32;
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, se));
}

/// Its behaviour. Step 0: a random speed (sideways, up, back) and fall, 180-209 frames. 1: smoke
/// every 10th frame and a sound every 40th; then a burst (2), or the end's smoke (3). 2: a smoke
/// burst; 3: the end's smoke; then 4: a sound and the end. Each frame it moves and turns.
pub fn debris_behaviour(cpu: &mut Cpu, actor: u32) {
    let fx = cpu.stack();
    match cpu.read8(actor + STEP) {
        0 => {
            let vx = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x4000));
            cpu.write32(actor + SPEED, vx);
            let vy = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x6000)).wrapping_add(0x1_0000);
            cpu.write32(actor + SPEED + 4, vy);
            let vz = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x4000)).wrapping_sub(0x1_0000);
            cpu.write32(actor + SPEED + 8, vz);
            cpu.write32(actor + FALL, 0x400);
            let frames = nested!(cpu, 0x0604_5d90, math::random(cpu, 30)).wrapping_add(0xb4);
            cpu.write16(actor + COUNT, frames as u16);
            cpu.write8(actor + STEP, 1);
        }
        1 => {
            let count = cpu.read16(actor + COUNT) as i16 as i32 as u32;
            let tenth = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, count, 10));
            if tenth == 0 {
                let x = cpu.read32(actor + POSITION).wrapping_add(nested!(cpu, 0x0604_5d90, math::random(cpu, 0x20_0000)));
                let y = cpu.read32(actor + POSITION + 4).wrapping_add(nested!(cpu, 0x0604_5d90, math::random(cpu, 0x20_0000)));
                let z = cpu.read32(actor + POSITION + 8).wrapping_add(nested!(cpu, 0x0604_5d90, math::random(cpu, 0x10_0000))).wrapping_sub(0x10_0000);
                let scale = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x4000)).wrapping_add(0x8000);
                let number = cpu.read8(actor + NUMBER);
                smoke(cpu, fx, x, y, z, scale, number, SMOKE_TRAIL, tenth);
            }
            let count = cpu.read16(actor + COUNT) as i16 as i32 as u32;
            if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, count, 40)) == 0 {
                debris_sound(cpu, FALL_SOUND);
            }
            let count = cpu.read16(actor + COUNT).wrapping_sub(1);
            cpu.write16(actor + COUNT, count);
            if count as i16 <= 0 {
                cpu.write8(actor + STEP, 2);
                if cpu.read8(actor + BURSTS) != 0 {
                    cpu.write8(actor + STEP, 3);
                }
            }
        }
        2 => {
            let at = fx + 0x40;
            for k in [0, 4, 8] {
                let v = cpu.read32(actor + POSITION + k);
                cpu.write32(at + k, v);
            }
            nested!(cpu, 0x0605_7754, explosions::smoke_burst(cpu, at));
            cpu.write8(actor + STEP, 4);
        }
        3 => {
            let (x, y) = (cpu.read32(actor + POSITION), cpu.read32(actor + POSITION + 4));
            let z = cpu.read32(actor + POSITION + 8).wrapping_sub(0x10_0000);
            let scale = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x4000)).wrapping_add(0xc000);
            let number = cpu.read8(actor + NUMBER);
            smoke(cpu, fx + 0x80, x, y, z, scale, number, SMOKE_END, 0);
            cpu.write8(actor + STEP, 4);
        }
        4 => {
            debris_sound(cpu, END_SOUND);
            cpu.write8(actor + OBJ_MODE, TO_STOP);
        }
        _ => {}
    }
    let vz = cpu.read32(actor + SPEED + 8).wrapping_add(cpu.read32(actor + FALL));
    cpu.write32(actor + SPEED + 8, vz);
    for k in [0, 4, 8] {
        let p = cpu.read32(actor + POSITION + k).wrapping_add(cpu.read32(actor + SPEED + k));
        cpu.write32(actor + POSITION + k, p);
    }
    for k in [0, 2, 4] {
        let t = cpu.read16(actor + TURN_X + k).wrapping_add(cpu.read16(actor + SPIN + k));
        cpu.write16(actor + TURN_X + k, t);
    }
}

/// A piece of debris broken off `parent` (list 3, 0x80 bytes): a model actor with the parent's
/// first model, at the parent's model point, with its scales, turns and number, turning at random
/// (each turn up to 0x400 a frame). It, or 0.
const PLAIN_EXEC: u32 = 0x0604_dcb4;
const IS_MODEL: u32 = 0x80_0000;
const FLAGS: u32 = 0x10;
/// The actor flags: no hit boxes (it neither hits nor is hit), not to be hurt (shielded).
const NO_HITS: u32 = 0x8_0000;
const UNHURT: u32 = 0x4_0000;
const DEBRIS_BEHAVIOUR: u32 = 0x0608_05c0;

pub fn debris_set(cpu: &mut Cpu, parent: u32) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0x80)) as i8 as i32 as u32;
    let actor = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 3, blocks, PLAIN_EXEC));
    if actor != 0 {
        nested!(cpu, 0x0604_dd50, effects::actor_init(cpu, actor));
        let flags = cpu.read32(actor + FLAGS) | IS_MODEL;
        cpu.write32(actor + FLAGS, flags);
        let models = cpu.read32(parent + MODELS);
        let model = cpu.read32(models);
        cpu.write32(actor + MODEL, model);
        cpu.write32(actor + BEHAVIOUR, DEBRIS_BEHAVIOUR);
        for k in [8, 4, 0] {
            cpu.write32(actor + POSITION + k, 0);
        }
        nested!(cpu, 0x0605_2c44, models::model_point(cpu, parent, actor + POSITION));
        for k in [0, 4, 8] {
            let v = cpu.read32(parent + SCALES + k);
            cpu.write32(actor + SCALES + k, v);
        }
        for k in [0, 2, 4] {
            let v = cpu.read16(parent + TURN_X + k);
            cpu.write16(actor + TURN_X + k, v);
        }
        for at in [NUMBER, BYTE_33] {
            let v = cpu.read8(parent + at);
            cpu.write8(actor + at, v);
        }
        for k in [0, 2, 4] {
            let spin = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x400));
            cpu.write16(actor + SPIN + k, spin as u16);
        }
    }
    actor
}

// ---- falling polygons -----------------------------------------------------------------------------

/// A polygon effect falling with the screen (effects.rs's polygon object): its own frame, its
/// place in the scenery (+0x64) moved by its speed (+0x44, +0x48; y also by the screen's scroll,
/// its work's +0xc), its position that place in the screen's frame.
const SCENERY_PLACE: u32 = 0x64;
const SCREEN_SCROLL_Y: u32 = 0x1c;

pub fn falling_polygon(cpu: &mut Cpu, obj: u32) {
    let s = cpu.read32(screen::SCREEN);
    nested!(cpu, 0x0604_e3a8, effects::polygon_exec(cpu, obj));
    let x = cpu.read32(obj + SCENERY_PLACE).wrapping_add(cpu.read32(obj + SPEED));
    cpu.write32(obj + SCENERY_PLACE, x);
    let dy = cpu.read32(obj + SPEED + 4).wrapping_add(cpu.read32(s + SCREEN_SCROLL_Y));
    let y = cpu.read32(obj + SCENERY_PLACE + 4).wrapping_add(dy);
    cpu.write32(obj + SCENERY_PLACE + 4, y);
    for k in [0, 4, 8] {
        let v = cpu.read32(obj + SCENERY_PLACE + k);
        cpu.write32(obj + POSITION + k, v);
    }
    nested!(cpu, 0x0608_0478, scenery_point(cpu, obj + POSITION));
}

/// A falling polygon that eases: its sideways speed a 128th of the way to none a frame, its
/// vertical speed toward 0x4000, its z on by its z speed (+0x60), which eases to none.
const Z_SPEED: u32 = 0x60;

pub fn easing_polygon(cpu: &mut Cpu, obj: u32) {
    let ease = |v: u32, to: u32| v.wrapping_add((to.wrapping_sub(v) as i32 / 128) as u32);
    let vx = cpu.read32(obj + SPEED);
    cpu.write32(obj + SPEED, ease(vx, 0));
    let vy = cpu.read32(obj + SPEED + 4);
    cpu.write32(obj + SPEED + 4, ease(vy, 0x4000));
    let z = cpu.read32(obj + POSITION + 8).wrapping_add(cpu.read32(obj + Z_SPEED));
    cpu.write32(obj + POSITION + 8, z);
    let vz = cpu.read32(obj + Z_SPEED);
    cpu.write32(obj + Z_SPEED, ease(vz, 0));
    nested!(cpu, 0x0608_09a0, falling_polygon(cpu, obj));
}

// ---- the risers: large models that rise from below the scenery ------------------------------------

/// A riser (a scenery model actor, its parts by model_parts_set's tables): its inner step and its
/// step (+0x6c, +0x6d: a word written there sets the step and clears the inner one), its variant
/// (+0x6e), a count (+0x70), its speed up the scenery (+0x90), something that holds its end (+0xb4),
/// its parts (+0xb8, +0xc0-0xc8), its place in the scenery (+0xd8: x, y up, z).
const INNER: u32 = 0x6c;
const RISER_STEP: u32 = 0x6d;
const VARIANT: u32 = 0x6e;
const RISER_COUNT: u32 = 0x70;
const RISE_SPEED: u32 = 0x90;
const HOLDS: u32 = 0xb4;
const PART_TOP: u32 = 0xb8;
const PARTS_TURNED: [(u32, u16); 3] = [(0xc0, 0x8000), (0xc4, 0x4000), (0xc8, 0xc000)];
const SCENERY_Y: u32 = 0xdc;
const SCENERY_Z: u32 = 0xe0;
const PART_FLAGS: u32 = 0x10;
const PART_HIDDEN: u32 = 0x10;
const PART_TURN: u32 = 0x2e;
const SCENERY_MODEL_DRAW: u32 = 0x0608_04e0;
/// Its parts' tables by step (2-9).
const RISER_PARTS: [u32; 8] = [0x0609_f744, 0x0609_f880, 0x0609_f920, 0x0609_f990, 0x0609_fa70, 0x0609_fae0, 0x0609_fb34, 0x0609_fba4];
/// Its place's limits up the scenery, and where it counts as risen.
const LOWEST: i32 = 0xfe80_0000u32 as i32;
const HIGHEST: i32 = 0x4c0_0000;
/// Its sounds: rising, risen, sinking.
const RISE_SOUND: u32 = 88;
const RISEN_SOUND: u32 = 89;
const SINK_SOUND: u32 = 91;

/// Sets the step (a word at +0x6c: the step's byte, its inner step cleared).
fn riser_step(cpu: &mut Cpu, obj: u32, step: u16) {
    cpu.write16(obj + INNER, step);
}

fn scroll_y(cpu: &mut Cpu) -> u32 {
    let s = cpu.read32(screen::SCREEN);
    cpu.read32(s + SCREEN_SCROLL_Y)
}

/// Its behaviour. Step 0: drawn by scenery_model_draw. 2-9: its parts made (by its table; 2-4
/// with its top part hidden in some variants, 5 with three parts turned), then 10: still with the
/// scroll. 11: a rise (fast up, easing to the scroll's speed once near its place; then up to
/// 0x8000 past it until in place: 15). 12: a wait of 60-119 frames, then up toward its place
/// (a 16th of the way a frame, at most 8 dots) until in place: 15. 100: sinking with the scroll
/// (1 once below and nothing holds it). Each frame its place moves by its speed and the scroll
/// (within its limits) and its depth eases back; one frame in 300 it sheds five polygons.
pub fn riser_behaviour(cpu: &mut Cpu, obj: u32) {
    nested!(cpu, 0x0605_2970, models::follow_parent(cpu, obj));
    match cpu.read8(obj + RISER_STEP) {
        0 => {
            cpu.write32(obj + MODEL, SCENERY_MODEL_DRAW);
            riser_step(cpu, obj, 1);
        }
        step @ 2..=9 => {
            let table = RISER_PARTS[step as usize - 2];
            if nested!(cpu, 0x0605_332c, models::model_parts_set(cpu, obj, table)) != 0 {
                let hide_top = match step {
                    2 => true,
                    3 => cpu.read8(obj + VARIANT) == 4,
                    4 => cpu.read8(obj + VARIANT) == 2,
                    _ => false,
                };
                if hide_top {
                    let part = cpu.read32(obj + PART_TOP);
                    let flags = cpu.read32(part + PART_FLAGS) | PART_HIDDEN;
                    cpu.write32(part + PART_FLAGS, flags);
                }
                if step == 5 {
                    for (at, turn) in PARTS_TURNED {
                        let part = cpu.read32(obj + at);
                        cpu.write16(part + PART_TURN, turn);
                    }
                }
                riser_step(cpu, obj, 10);
            }
        }
        10 => {
            let v = scroll_y(cpu).wrapping_neg();
            cpu.write32(obj + RISE_SPEED, v);
        }
        11 => match cpu.read8(obj + INNER) {
            0 => {
                cpu.write32(obj + RISE_SPEED, 0xffe8_0000);
                nested!(cpu, 0x0600_50d4, sound::se_set(cpu, RISE_SOUND));
                cpu.write8(obj + INNER, 1);
            }
            1 => {
                if cpu.read32(obj + SCENERY_Y) as i32 <= 0x9f_ffff {
                    let v = cpu.read32(obj + RISE_SPEED);
                    let to = scroll_y(cpu).wrapping_neg().wrapping_sub(v);
                    let v = ((to as i32 / 16) as u32).wrapping_add(v);
                    cpu.write32(obj + RISE_SPEED, v);
                    let off = scroll_y(cpu).wrapping_add(v) as i32;
                    let off = if off < 0 { off.wrapping_neg() } else { off };
                    if off <= 0x4000 {
                        cpu.write8(obj + INNER, 2);
                    }
                }
            }
            2 => {
                let v = cpu.read32(obj + RISE_SPEED);
                let to = 0x8000u32.wrapping_sub(scroll_y(cpu));
                let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, to, 0x400));
                cpu.write32(obj + RISE_SPEED, v);
                if cpu.read32(obj + SCENERY_Y) as i32 >= 0 {
                    in_place(cpu, obj, false);
                }
            }
            _ => {}
        },
        12 => match cpu.read8(obj + INNER) {
            1 => {
                let wait = nested!(cpu, 0x0604_5d90, math::random(cpu, 60)).wrapping_add(60);
                cpu.write32(obj + RISER_COUNT, wait);
                cpu.write8(obj + INNER, 2);
            }
            0 | 2 => {
                let count = cpu.read32(obj + RISER_COUNT);
                if cpu.read8(obj + INNER) == 2 && count as i32 > 0 {
                    cpu.write32(obj + RISER_COUNT, count.wrapping_sub(1));
                } else {
                    cpu.write8(obj + INNER, 9);
                }
            }
            9 => {
                let to = cpu.read32(obj + SCENERY_Y).wrapping_neg() as i32 / 16;
                let v = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, to as u32, 0x8_0000));
                let v = v.wrapping_sub(scroll_y(cpu));
                cpu.write32(obj + RISE_SPEED, v);
                let y = cpu.read32(obj + SCENERY_Y) as i32;
                let y = if y < 0 { y.wrapping_neg() } else { y };
                if y <= 0x8000 {
                    cpu.write8(obj + INNER, 10);
                }
            }
            10 if cpu.read32(obj + SCENERY_Y) as i32 <= 0 => in_place(cpu, obj, true),
            _ => {}
        },
        100 => {
            if cpu.read8(obj + INNER) == 0 {
                nested!(cpu, 0x0600_50d4, sound::se_set(cpu, SINK_SOUND));
                cpu.write8(obj + INNER, 1);
            }
            let v = cpu.read32(obj + RISE_SPEED);
            let scroll = scroll_y(cpu);
            let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, scroll, 0x800));
            cpu.write32(obj + RISE_SPEED, v);
            if cpu.read32(obj + HOLDS) == 0 && cpu.read32(obj + SCENERY_Y) as i32 >= HIGHEST {
                riser_step(cpu, obj, 1);
            }
        }
        _ => {}
    }
    riser_move(cpu, obj);
}

/// In its place: its place up the scenery 0, moving with the scroll (with the risen sound when
/// `sound`), step 15.
fn in_place(cpu: &mut Cpu, obj: u32, sound: bool) {
    cpu.write32(obj + SCENERY_Y, 0);
    let v = scroll_y(cpu).wrapping_neg();
    cpu.write32(obj + RISE_SPEED, v);
    if sound {
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, RISEN_SOUND));
    }
    riser_step(cpu, obj, 15);
}

/// The frame's move and shedding.
fn riser_move(cpu: &mut Cpu, obj: u32) {
    let y = cpu.read32(obj + SCENERY_Y).wrapping_add(cpu.read32(obj + RISE_SPEED));
    cpu.write32(obj + SCENERY_Y, y);
    let y = scroll_y(cpu).wrapping_add(y);
    cpu.write32(obj + SCENERY_Y, y);
    if (y as i32) < LOWEST {
        cpu.write32(obj + SCENERY_Y, LOWEST as u32);
    }
    if cpu.read32(obj + SCENERY_Y) as i32 > HIGHEST {
        cpu.write32(obj + SCENERY_Y, HIGHEST as u32);
    }
    let z = cpu.read32(obj + SCENERY_Z);
    let z = nested!(cpu, 0x0604_6168, actors::approach(cpu, z, 0, 0x8000));
    cpu.write32(obj + SCENERY_Z, z);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 300));
    if r == 0 {
        shed(cpu, obj, r);
    }
}

/// Five polygons shed from its model point (easing_polygon each: up to 2 dots back in z, up to
/// half a dot right, its height and depth, a random turn, sideways up to 1 dot, up at up to half
/// its speed), its depth pushed 2 dots back.
const SHED_FLAGS: u32 = 0x23;
const EASING_POLYGON: u32 = 0x0608_0a0c;
const POLY_TURN_SPEED: u32 = 0x70;
const POLY_SPIN: u32 = 0x76;

fn shed(cpu: &mut Cpu, obj: u32, zero: u32) {
    let fx = cpu.stack();
    let z = cpu.read32(obj + SCENERY_Z).wrapping_add(0xfffe_0000);
    cpu.write32(obj + SCENERY_Z, z);
    for k in [8, 4, 0] {
        cpu.write32(fx + effects::X + k, zero);
    }
    nested!(cpu, 0x0605_2c44, models::model_point(cpu, obj, fx));
    cpu.write32(fx + effects::SCALE_Y, 0x1_0000);
    cpu.write32(fx + effects::SCALE_X, 0x1_0000);
    cpu.write32(fx + effects::FLAGS_LONG, SHED_FLAGS);
    cpu.write8(fx + effects::FRAMES_2, 0);
    let colour = cpu.read16(CHAR_BASE);
    cpu.write16(fx + effects::COLOUR, colour);
    let number = cpu.read8(obj + NUMBER);
    cpu.write8(fx + effects::PLAYER, number);
    cpu.write32(fx + effects::RELATIVE_3C, obj);
    for _ in 0..5 {
        let angle = nested!(cpu, 0x0604_5d90, math::random(cpu, 0));
        cpu.write16(fx + effects::ANGLE, angle as u16);
        let frames = nested!(cpu, 0x0604_5d90, math::random(cpu, 60)).wrapping_add(90);
        cpu.write16(fx + effects::FRAMES, frames as u16);
        let p = nested!(cpu, 0x0604_e6d8, effects::polygon_set(cpu, fx));
        if p == 0 {
            return;
        }
        cpu.write32(p + BEHAVIOUR, EASING_POLYGON);
        let vz = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x10_0000)).wrapping_sub(0x10_0000);
        cpu.write32(p + Z_SPEED, vz);
        let dx = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x8_0000));
        let x = cpu.read32(obj + PLACE_IN_SCENERY).wrapping_add(dx);
        cpu.write32(p + SCENERY_PLACE, x);
        let y = cpu.read32(obj + SCENERY_Y);
        cpu.write32(p + SCENERY_PLACE + 4, y);
        let z = cpu.read32(obj + SCENERY_Z);
        cpu.write32(p + SCENERY_PLACE + 8, z);
        for k in [4, 2, 0] {
            cpu.write16(p + POLY_TURN_SPEED + k, 0);
        }
        let spin = nested!(cpu, 0x0604_5d90, math::random(cpu, 4));
        cpu.write16(p + POLY_SPIN, spin as u16);
        let vx = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
        cpu.write32(p + SPEED, vx);
        let half = (cpu.read32(obj + RISE_SPEED) as i32 / 2) as u32;
        let vy = nested!(cpu, 0x0604_5d90, math::random(cpu, half));
        cpu.write32(p + SPEED + 4, vy);
    }
}

/// A riser's smoking part (its parent the riser, +0xd4; its slot in the riser's +0xb4 list,
/// +0x6e): ends with the riser. Step 100: after 90-149 frames the riser sinks (its step 100) and
/// it goes on 150-209 frames (101), then frees its slot and ends (102). Smoke from its model point
/// each tenth frame, a sound each 120th.
const PARENT: u32 = 0xd4;
const SLOT: u32 = 0x6e;
const RISER_SLOTS: u32 = 0xb4;
const ENDING: u8 = 0x6f;

pub fn smoking_part(cpu: &mut Cpu, obj: u32) {
    let riser = cpu.read32(obj + PARENT);
    if cpu.read8(riser + OBJ_MODE) as i8 > ENDING as i8 {
        cpu.write8(obj + OBJ_MODE, TO_STOP);
    }
    match cpu.read8(obj + RISER_STEP) as i8 {
        0 => {
            riser_step(cpu, obj, 1);
            return;
        }
        100 => {}
        101 => {
            if cpu.read32(obj + RISER_COUNT) as i32 <= 0 {
                riser_step(cpu, obj, 102);
            }
        }
        102 => {
            let riser = cpu.read32(obj + PARENT);
            let slot = cpu.read8(obj + SLOT) as i8 as i32 as u32;
            cpu.write32(riser.wrapping_add(4 * slot).wrapping_add(RISER_SLOTS), 0);
            cpu.write8(obj + OBJ_MODE, TO_STOP);
            return;
        }
        _ => return,
    }
    let count = cpu.read32(obj + RISER_COUNT).wrapping_sub(1);
    cpu.write32(obj + RISER_COUNT, count);
    if cpu.read8(obj + RISER_STEP) == 100 {
        if cpu.read8(obj + INNER) == 0 {
            let frames = nested!(cpu, 0x0604_5d90, math::random(cpu, 60)).wrapping_add(90);
            cpu.write32(obj + RISER_COUNT, frames);
            cpu.write8(obj + INNER, 1);
        }
        if cpu.read32(obj + RISER_COUNT) as i32 <= 0 {
            let riser = cpu.read32(obj + PARENT);
            riser_step(cpu, riser, 100);
            let frames = nested!(cpu, 0x0604_5d90, math::random(cpu, 60)).wrapping_add(150);
            cpu.write32(obj + RISER_COUNT, frames);
            riser_step(cpu, obj, 101);
        }
    }
    let count = cpu.read32(obj + RISER_COUNT);
    if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, count, 10)) == 0 {
        let e = cpu.stack();
        let x = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x10_0000));
        cpu.write32(e, x);
        let y = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x10_0000));
        cpu.write32(e + 4, y);
        let z = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x10_0000)).wrapping_sub(0x10_0000);
        cpu.write32(e + 8, z);
        nested!(cpu, 0x0605_2c44, models::model_point(cpu, obj, e));
        let size = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x4000)).wrapping_add(0xc000);
        cpu.write32(e + 12, size);
        nested!(cpu, 0x0605_77d0, explosions::smoke_puff(cpu, e));
    }
    let count = cpu.read32(obj + RISER_COUNT);
    if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, count, 120)) == 0 {
        debris_sound(cpu, FALL_SOUND);
    }
}

/// A riser's part that flashes with another of its parts (the riser's slot 0's part +0xc0):
/// following its parent first.
pub fn flashing_part(cpu: &mut Cpu, obj: u32) {
    let parent = nested!(cpu, 0x0605_2970, models::follow_parent(cpu, obj));
    let slot = cpu.read32(parent + RISER_SLOTS);
    let with = cpu.read32(slot + 0xc0);
    nested!(cpu, 0x0604_668c, parts::flash_against(cpu, obj, with));
}

/// A spinning model's draw (its sprite `s`): moved to its place, turned (z, x, y, each if set),
/// scaled (flipped by the sprite's flags), its first model put; its kind 1's frame 0 flipped
/// (0x100) and coloured by the main loop's count (four colours by turns, 16 a step past the
/// characters); a 32nd of a turn more about y each frame.
const S_FLAGS: u32 = 0x00;
const S_TURN_Z: u32 = 0x20;
const S_TURN_X: u32 = 0x1c;
const S_TURN_Y: u32 = 0x1e;
const S_SCALES: u32 = 0x10;
const S_MODELS: u32 = 0x34;
const FLIP_X: u32 = 0x10;
const FLIP_Y: u32 = 0x20;
const SPIN_KIND: u32 = 1;
const SPIN_FLIP: u32 = 0x100;
const SPIN_COLOURS: u32 = 0x0609_fbec;
const SPIN_STEP: u16 = 0x800;

pub fn spinning_draw(cpu: &mut Cpu, s: u32) {
    let actor = s.wrapping_sub(ACTOR);
    cpu.call(SL_PUSH_MATRIX, &[]);
    let p = [0, 4, 8].map(|k| cpu.read32(actor + POSITION + k));
    cpu.call(SL_TRANSLATE, &p);
    for (field, rot) in [(S_TURN_Z, SL_ROT_Z), (S_TURN_X, SL_ROT_X), (S_TURN_Y, SL_ROT_Y)] {
        let a = cpu.read16(s + field) as i16;
        if a != 0 {
            cpu.call(rot, &[a as i32 as u32]);
        }
    }
    let flags = cpu.read32(s + S_FLAGS);
    let mut sx = cpu.read32(s + S_SCALES);
    let mut sy = cpu.read32(s + S_SCALES + 4);
    if flags & FLIP_X != 0 {
        sx = sx.wrapping_neg();
    }
    if flags & FLIP_Y != 0 {
        sy = sy.wrapping_neg();
    }
    let sz = cpu.read32(s + S_SCALES + 8);
    cpu.call(SL_SCALE, &[sx, sy, sz]);
    let models = cpu.read32(s + S_MODELS);
    let model = cpu.read32(models);
    cpu.call(SL_PUT_POLYGON, &[model]);
    cpu.call(SL_POP_MATRIX, &[]);
    nested!(cpu, 0x0604_5cdc, models::model_flip_bits(cpu, SPIN_KIND, 0, SPIN_FLIP));
    let n = (cpu.read32(LOOPS) >> 1) & 3;
    let colour = (cpu.read8(SPIN_COLOURS + n) as i8 as i32 as u32 * 16).wrapping_add(cpu.read16(CHAR_BASE) as i16 as i32 as u32) & 0xffff;
    nested!(cpu, 0x0604_5b1c, models::model_colour(cpu, SPIN_KIND, 0, colour));
    let t = cpu.read16(s + S_TURN_Y).wrapping_add(SPIN_STEP);
    cpu.write16(s + S_TURN_Y, t);
}

// ---- the beam ------------------------------------------------------------------------------------

/// A riser's beam (an actor, actor_exec's): it ends with its parent (+0xb0); placed down the
/// parent's model at +0x70, turned half round from it. Step 0: drawn spinning, its boxes, its
/// count 90 less by difficulty, its sound. 1: sparks each fourth frame while it charges; 2: 30
/// frames; 3: shown and hitting, it grows (its width a quarter of the way to one a frame, its
/// length to 32 dots), 60 frames; 4: no hits, it narrows to nothing and ends. Its width (+0x20) is kept at +0x28 too.
const BEAM_PARENT: u32 = 0xb0;
const BEAM_DOWN: u32 = 0x70;
const BEAM_COUNT: u32 = 0x78;
const BEAM_SOUND: u32 = 0x7c;
const BEAM_WIDTH: u32 = 0x20;
const BEAM_WIDTH_KEPT: u32 = 0x28;
const BEAM_GROW: u32 = 0x24;
const BEAM_BOXES: u32 = 0x0609_fbe4;
const BEAM_LIST: u32 = 0x3c;
const BEAM_LIST_AT: u32 = 0xb4;
const OBJ_FLAGS_BYTE: u32 = 2;
const OWN_MODEL_DRAW: u32 = 0xc0_0000;
const BEAM_HIT_FLAGS: u32 = 0x8008_0000;
const SPINNING_DRAW: u32 = 0x0608_11d4;
const KIND_1_FILE: u32 = 0x0607_4200;
const CHARGE_SOUND: u32 = 0x46;
const BEAM_SOUND_ID: u32 = 0x27;
const SPARK_FLAGS: u32 = 0x80;

pub fn beam_behaviour(cpu: &mut Cpu, obj: u32) {
    let parent = cpu.read32(obj + BEAM_PARENT);
    if cpu.read8(parent + OBJ_MODE) as i8 > ENDING as i8 {
        cpu.write8(obj + OBJ_MODE, TO_STOP);
        return;
    }
    cpu.write32(obj + POSITION, 0);
    let down = cpu.read32(obj + BEAM_DOWN);
    cpu.write32(obj + POSITION + 4, down);
    cpu.write32(obj + POSITION + 8, 0);
    nested!(cpu, 0x0605_2c44, models::model_point(cpu, parent, obj + POSITION));
    let turn = cpu.read16(parent + TURN_Z).wrapping_add(0x8000);
    cpu.write16(obj + TURN_Z, turn);
    let b = cpu.read8(obj + OBJ_FLAGS_BYTE) | 1;
    cpu.write8(obj + OBJ_FLAGS_BYTE, b);
    match cpu.read8(obj + RISER_STEP) {
        0 => {
            let flags = cpu.read32(obj + FLAGS) | OWN_MODEL_DRAW;
            cpu.write32(obj + FLAGS, flags);
            cpu.write32(obj + MODEL, SPINNING_DRAW);
            let file = cpu.read32(KIND_1_FILE);
            let models = cpu.read32(file + 0x1c);
            let model = cpu.read32(models);
            cpu.write32(obj + MODELS, model);
            nested!(cpu, 0x0605_2b08, models::beam_boxes(cpu, obj, BEAM_BOXES));
            cpu.write32(obj + BEAM_LIST, obj + BEAM_LIST_AT);
            for at in [0x60, 0x5c, 0x74, BEAM_GROW] {
                cpu.write32(obj + at, 0);
            }
            let flags = cpu.read32(obj + FLAGS) | BEAM_HIT_FLAGS;
            cpu.write32(obj + FLAGS, flags);
            let less = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 30));
            cpu.write32(obj + BEAM_COUNT, 90u32.wrapping_sub(less));
            let control = nested!(cpu, 0x0600_50d4, sound::se_set(cpu, CHARGE_SOUND));
            cpu.write8(obj + BEAM_SOUND, control as u8);
            riser_step(cpu, obj, 1);
        }
        1 => {
            let count = cpu.read32(obj + BEAM_COUNT);
            let fourth = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, count, 4));
            if fourth == 0 {
                spark(cpu, obj, fourth);
            }
            let count = cpu.read32(obj + BEAM_COUNT).wrapping_sub(1);
            cpu.write32(obj + BEAM_COUNT, count);
            if count as i32 <= 0 {
                let control = cpu.read8(obj + BEAM_SOUND) as i8 as i32 as u32;
                nested!(cpu, 0x0600_52e0, sound::se_stop(cpu, CHARGE_SOUND, control));
                cpu.write32(obj + BEAM_COUNT, 0);
                riser_step(cpu, obj, 2);
            }
        }
        2 => {
            let count = cpu.read32(obj + BEAM_COUNT).wrapping_add(1);
            cpu.write32(obj + BEAM_COUNT, count);
            if count as i32 > 0x1d {
                let flags = cpu.read32(obj + FLAGS) & 0x7ff7_ffff;
                cpu.write32(obj + FLAGS, flags);
                cpu.write32(obj + BEAM_COUNT, 0);
                let control = nested!(cpu, 0x0600_50d4, sound::se_set(cpu, BEAM_SOUND_ID));
                cpu.write8(obj + BEAM_SOUND, control as u8);
                riser_step(cpu, obj, 3);
            }
        }
        3 => {
            let w = cpu.read32(obj + BEAM_WIDTH);
            let w = w.wrapping_add((0x1_0000u32.wrapping_sub(w) as i32 / 4) as u32);
            cpu.write32(obj + BEAM_WIDTH, w);
            let g = cpu.read32(obj + BEAM_GROW);
            let g = nested!(cpu, 0x0604_6168, actors::approach(cpu, g, 0x1_0000, 0x1000));
            cpu.write32(obj + BEAM_GROW, g);
            let length = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, 0x200_0000, g));
            nested!(cpu, 0x0605_2b88, models::beam_length(cpu, obj, length));
            let count = cpu.read32(obj + BEAM_COUNT).wrapping_add(1);
            cpu.write32(obj + BEAM_COUNT, count);
            if count as i32 > 0x3b {
                let flags = cpu.read32(obj + FLAGS) | NO_HITS;
                cpu.write32(obj + FLAGS, flags);
                riser_step(cpu, obj, 4);
            }
        }
        4 => {
            let w = cpu.read32(obj + BEAM_WIDTH);
            let w = nested!(cpu, 0x0604_6168, actors::approach(cpu, w, 0, 0x800));
            cpu.write32(obj + BEAM_WIDTH, w);
            if w == 0 {
                let control = cpu.read8(obj + BEAM_SOUND) as i8 as i32 as u32;
                nested!(cpu, 0x0600_52e0, sound::se_stop(cpu, BEAM_SOUND_ID, control));
                cpu.write8(obj + OBJ_MODE, TO_STOP);
            }
        }
        _ => {}
    }
    let w = cpu.read32(obj + BEAM_WIDTH);
    cpu.write32(obj + BEAM_WIDTH_KEPT, w);
}

/// A spark of the charging beam (an effect relative to it): up to 4 dots across, 2 dots above
/// a 3-dot drop, scaled 2-2.5, at a small random angle, 15 frames, frames 0x37-0x39, one of two
/// colours.
fn spark(cpu: &mut Cpu, obj: u32, zero: u32) {
    let fx = cpu.stack();
    let x = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x4_0000));
    cpu.write32(fx + effects::X, x);
    let y = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x20_0000)).wrapping_sub(0x30_0000);
    cpu.write32(fx + effects::X + 4, y);
    cpu.write32(fx + effects::X + 8, zero);
    let scale = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x8000)).wrapping_add(0x2_0000);
    cpu.write32(fx + effects::SCALE_Y, scale);
    cpu.write32(fx + effects::SCALE_X, scale);
    cpu.write32(fx + effects::FLAGS_LONG, SPARK_FLAGS);
    let angle = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1800));
    cpu.write16(fx + effects::ANGLE, angle as u16);
    cpu.write16(fx + effects::FRAMES, 15);
    cpu.write8(fx + effects::FRAMES_2, 15);
    cpu.write8(fx + effects::KIND, 0);
    let frame = nested!(cpu, 0x0604_5d90, math::random(cpu, 3)).wrapping_add(0x37);
    cpu.write8(fx + effects::FRAME, frame as u8);
    let c = nested!(cpu, 0x0604_5d90, math::random(cpu, 2));
    let colour = (c as u16).wrapping_mul(16).wrapping_add(0x30).wrapping_add(cpu.read16(CHAR_BASE));
    cpu.write16(fx + effects::COLOUR, colour);
    let number = cpu.read8(obj + NUMBER);
    cpu.write8(fx + effects::PLAYER, number);
    cpu.write32(fx + effects::RELATIVE_3C, obj);
    nested!(cpu, 0x0604_e1cc, effects::effect_set(cpu, fx));
}

/// A riser's beam made (list 2, 0x174 bytes, actor_exec's), of `parent` (its number and the byte
/// +0x54), placed 3 dots down its model. It, or 0.
const ACTOR_EXEC: u32 = 0x0604_d780;
const SPRITE_FLAG_400: u32 = 0x400;
const BEAM_BEHAVIOUR: u32 = 0x0608_12cc;
const BYTE_54: u32 = 0x54;

pub fn beam_set(cpu: &mut Cpu, parent: u32) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0x174)) as i8 as i32 as u32;
    let obj = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if obj != 0 {
        nested!(cpu, 0x0604_62ac, actors::actor_setup(cpu, obj));
        let flags = cpu.read32(obj + FLAGS) | SPRITE_FLAG_400;
        cpu.write32(obj + FLAGS, flags);
        cpu.write32(obj + BEHAVIOUR, BEAM_BEHAVIOUR);
        let n = cpu.read8(parent + NUMBER);
        cpu.write8(obj + NUMBER, n);
        cpu.write8(obj + BYTE_33, 8);
        let b = cpu.read8(parent + BYTE_54);
        cpu.write8(obj + BYTE_54, b);
        cpu.write32(obj + BEAM_PARENT, parent);
        cpu.write32(obj + BEAM_DOWN, 0x30_0000);
    }
    obj
}

// ---- the riser's turrets ---------------------------------------------------------------------------

/// A turret on a riser's part (its parent; the part's parent, the riser, at +0xd4 of it): its
/// life (+0x50), its turns (x +0x2c, z +0x30: its aim), a count (+0x70), its shots' count
/// (+0x74; its low byte the inner step to go back to), the direction it fires its beam (+0x78), its
/// beam (+0x7c), the frames to its next fans (+0x80), who hit it last (+0x40: its player +0x32 and
/// weapon +0x55). Shot down, or its part sunk (past step 99), it is step 100.
const TURRET_LIFE: u32 = 0x50;
const TURRET_COUNT: u32 = 0x70;
const SHOTS: u32 = 0x74;
const SHOTS_LOW: u32 = 0x77;
const BEAM_AIM: u32 = 0x78;
const BEAM: u32 = 0x7c;
const FAN_WAIT: u32 = 0x80;
const HIT_BY: u32 = 0x40;
const HIT_PLAYER: u32 = 0x32;
const HIT_WEAPON: u32 = 0x55;
const TURRET_POINTS: u32 = 10000;
const FAN_SPEED: u32 = 0x1_5800;
const FAN_SOUND: u32 = 26;
const FAN_SIDES: [(u32, u16); 2] = [(0xffec_0000, 0x7400), (0x14_0000, 0x8c00)];
const RISER_PART_SLOTS: [u32; 4] = [0xb8, 0xbc, 0xc0, 0xc4];

/// Its aim turned toward the nearer player (a 12th less by difficulty of the way a frame, at
/// most 0x100).
fn turret_aim(cpu: &mut Cpu, obj: u32) {
    let player = nested!(cpu, 0x0604_5f70, aim::nearer_player_to(cpu, obj));
    let to = nested!(cpu, 0x0604_5e30, aim::angle_to(cpu, player, obj)) as i16 as i32 as u32;
    let turn = cpu.read16(obj + TURN_Z) as i16 as i32 as u32;
    let d = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, turn, to));
    let less = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 8));
    let step = nested!(cpu, 0x0606_62d8, clib::divide(cpu, d, 12u32.wrapping_sub(less)));
    let step = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, step, 0x100));
    let t = cpu.read16(obj + TURN_Z).wrapping_add(step as u16);
    cpu.write16(obj + TURN_Z, t);
}

/// The turret's beam made; fires along its aim.
fn turret_beam(cpu: &mut Cpu, obj: u32) {
    let beam = nested!(cpu, 0x0608_15b8, beam_set(cpu, obj));
    cpu.write32(obj + BEAM, beam);
}

/// Its beam is let go once it ends.
fn beam_ended(cpu: &mut Cpu, obj: u32, beam: u32) {
    if cpu.read8(beam + OBJ_MODE) as i8 > ENDING as i8 {
        cpu.write32(obj + BEAM, 0);
    }
}

/// Its behaviour. Steps 0-4: life 50000, raised (its x turn from a quarter turn) once the riser's
/// slot 0 part has sunk and the riser's +0xc4 is free, shielded until then. 15: it fights: fans of
/// aimed shots either side every 60 frames less by difficulty; aiming at the nearer player and
/// firing beams, three sweeps (inner 1-5: straight, left, right) or a run of 3-5 aimed (11-13), a
/// rest of 300-419 frames between (a quarter of it with all four parts of the riser up). 100: it
/// smokes (with the score for whoever shot it). 120: 30 frames, an explosion, 121.
pub fn turret_behaviour(cpu: &mut Cpu, obj: u32) {
    let part = nested!(cpu, 0x0605_2970, models::follow_parent(cpu, obj));
    let parent = cpu.read32(obj + PARENT);
    let riser = cpu.read32(parent + PARENT);
    shot_or_sunk(cpu, obj, part);
    match cpu.read8(obj + RISER_STEP) as i8 {
        0 => {
            cpu.write32(obj + TURRET_LIFE, 50000);
            cpu.write16(obj + TURN_X, 0x4000);
            riser_step(cpu, obj, 1);
        }
        1 => {
            let slot = cpu.read32(part + RISER_SLOTS);
            let watched = cpu.read32(slot + 0xc4);
            if cpu.read8(watched + RISER_STEP) as i8 >= 100 {
                riser_step(cpu, obj, 2);
            }
        }
        2 => {
            if cpu.read32(riser + 0xc4) == 0 {
                riser_step(cpu, obj, 3);
            }
        }
        3 => {
            let t = turn_toward(cpu, obj, TURN_X, 0xfc00, 0x80);
            if t == 0xfc00 {
                let flags = cpu.read32(obj + FLAGS) & !UNHURT;
                cpu.write32(obj + FLAGS, flags);
                riser_step(cpu, obj, 4);
            }
        }
        4 => {
            if turn_toward(cpu, obj, TURN_X, 0, 0x100) == 0 {
                for at in [TURRET_COUNT, BEAM, FAN_WAIT] {
                    cpu.write32(obj + at, 0);
                }
                riser_step(cpu, obj, 15);
            }
        }
        15 => turret_fight(cpu, obj, part),
        100 => {
            if cpu.read8(obj + INNER) == 0 {
                if cpu.read32(obj + TURRET_LIFE) as i32 <= 0 {
                    turret_score(cpu, obj);
                }
                cpu.write32(obj + TURRET_COUNT, 0);
                cpu.write8(obj + INNER, 1);
            }
            let count = cpu.read32(obj + TURRET_COUNT);
            let puff = Puff { at: cpu.stack() + 0x40, spread: (0x20_0000, 0x20_0000), lift: 0, size: 0x8000 };
            burning(cpu, obj, count, puff, 15);
            let count = cpu.read32(obj + TURRET_COUNT).wrapping_add(1);
            cpu.write32(obj + TURRET_COUNT, count);
        }
        120 => {
            if cpu.read8(obj + INNER) == 0 {
                cpu.write32(obj + TURRET_COUNT, 30);
                cpu.write8(obj + INNER, 1);
            }
            let count = cpu.read32(obj + TURRET_COUNT).wrapping_add(1);
            cpu.write32(obj + TURRET_COUNT, count);
            if count == 60 {
                let p = cpu.stack() + 0x80;
                for k in [0, 4, 8] {
                    let v = cpu.read32(obj + POSITION + k);
                    cpu.write32(p + k, v);
                }
                cpu.write32(p + 0x3c, obj);
                nested!(cpu, 0x0605_8ba4, explosions::explosion_set(cpu, p));
                cpu.write8(obj + INNER, 0);
                cpu.write8(obj + RISER_STEP, 121);
            }
        }
        _ => {}
    }
}

/// Shot down (its life gone) or its part sunk (past 99), a riser's piece goes to step 100, no
/// longer hitting.
fn shot_or_sunk(cpu: &mut Cpu, obj: u32, part: u32) {
    if (cpu.read8(obj + RISER_STEP) as i8) < 100
        && ((cpu.read32(obj + TURRET_LIFE) as i32) <= 0 || cpu.read8(part + RISER_STEP) as i8 > 99)
    {
        let flags = cpu.read32(obj + FLAGS) | NO_HITS;
        cpu.write32(obj + FLAGS, flags);
        riser_step(cpu, obj, 100);
    }
}

/// The score for whoever shot it down (by the hit that did).
fn turret_score(cpu: &mut Cpu, obj: u32) {
    let by = cpu.read32(obj + HIT_BY);
    let (player, weapon) = (cpu.read8(by + HIT_PLAYER) as i8 as i32 as u32, cpu.read8(by + HIT_WEAPON) as i8 as i32 as u32);
    nested!(cpu, 0x0604_3108, hits::score_kill(cpu, player, weapon, obj, TURRET_POINTS));
}

/// A shot-down piece's puffs of smoke: made at `at` (on the stack), at random up to `spread` (x, y)
/// from its model's centre (`lift` higher) and up to a dot in front, of size `size` and up to a
/// quarter more.
#[derive(Clone, Copy)]
struct Puff {
    at: u32,
    spread: (u32, u32),
    lift: u32,
    size: u32,
}

/// A shot-down piece's smoke, by its count: every 5th frame a puff, every `sound_every`th a debris
/// sound.
fn burning(cpu: &mut Cpu, obj: u32, count: u32, puff: Puff, sound_every: u32) {
    if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, count, 5)) == 0 {
        let e = puff.at;
        let x = nested!(cpu, 0x0604_5d90, math::random(cpu, puff.spread.0));
        cpu.write32(e, x);
        let y = nested!(cpu, 0x0604_5d90, math::random(cpu, puff.spread.1)).wrapping_add(puff.lift);
        cpu.write32(e + 4, y);
        let z = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x10_0000)).wrapping_sub(0x10_0000);
        cpu.write32(e + 8, z);
        nested!(cpu, 0x0605_2c44, models::model_point(cpu, obj, e));
        let size = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x4000)).wrapping_add(puff.size);
        cpu.write32(e + 12, size);
        nested!(cpu, 0x0605_77d0, explosions::smoke_puff(cpu, e));
    }
    if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, count, sound_every)) == 0 {
        debris_sound(cpu, FALL_SOUND);
    }
}

/// Its turn (z) moved an eighth of the way to `to` (at most 0x100): the new turn.
fn turn_eighth(cpu: &mut Cpu, obj: u32, to: u16) -> u16 {
    let turn = cpu.read16(obj + TURN_Z) as i16 as i32 as u32;
    let d = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, turn, to as i16 as i32 as u32)) as i32 / 8;
    let step = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, d as u32, 0x100));
    let t = cpu.read16(obj + TURN_Z).wrapping_add(step as u16);
    cpu.write16(obj + TURN_Z, t);
    t
}

/// Whether turn `t` is within 0x7f of `to`.
fn near(t: u16, to: u16) -> bool {
    (t as i16 as i32 - to as i16 as i32).wrapping_abs() < 0x80
}

/// The turn word at `field` moved toward `to` (by the angle between, at most `most`): the new turn.
fn turn_toward(cpu: &mut Cpu, obj: u32, field: u32, to: u32, most: u32) -> u16 {
    let turn = cpu.read16(obj + field) as i16 as i32 as u32;
    let d = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, turn, to));
    let step = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, d, most));
    let t = cpu.read16(obj + field).wrapping_add(step as u16);
    cpu.write16(obj + field, t);
    t
}

/// Step 15's inner steps.
fn turret_fight(cpu: &mut Cpu, obj: u32, part: u32) {
    match cpu.read8(obj + INNER) {
        0 => {
            let wait = cpu.read32(obj + FAN_WAIT).wrapping_add(1);
            cpu.write32(obj + FAN_WAIT, wait);
            let less = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 30));
            if cpu.read32(obj + FAN_WAIT) as i32 > 60u32.wrapping_sub(less) as i32 {
                fans(cpu, obj);
            }
            let beam = cpu.read32(obj + BEAM);
            if beam != 0 {
                beam_ended(cpu, obj, beam);
                return;
            }
            turret_aim(cpu, obj);
            let count = cpu.read32(obj + TURRET_COUNT);
            if count != 0 {
                cpu.write32(obj + TURRET_COUNT, count.wrapping_sub(1));
                return;
            }
            let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 2)) as i32;
            cpu.write8(obj + INNER, (r.wrapping_abs() as u8).wrapping_mul(10).wrapping_add(1));
        }
        1 | 21 | 31 | 41 => {
            cpu.write32(obj + SHOTS, 1);
            cpu.write16(obj + BEAM_AIM, 0);
            cpu.write8(obj + INNER, 4);
        }
        2 | 3 => {
            let aim = if cpu.read8(obj + INNER) == 2 { 0x2000 } else { 0xe000 };
            cpu.write16(obj + BEAM_AIM, aim);
            cpu.write8(obj + INNER, 4);
        }
        4 => {
            let aim = cpu.read16(obj + BEAM_AIM);
            let t = turn_eighth(cpu, obj, aim);
            if near(t, aim) {
                cpu.write16(obj + TURN_Z, aim);
                turret_beam(cpu, obj);
                cpu.write8(obj + INNER, 5);
            }
        }
        5 => {
            let beam = cpu.read32(obj + BEAM);
            if beam != 0 {
                beam_ended(cpu, obj, beam);
                return;
            }
            let shots = cpu.read32(obj + SHOTS).wrapping_add(1);
            cpu.write32(obj + SHOTS, shots);
            if (shots as i32) < 4 {
                let back = cpu.read8(obj + SHOTS_LOW);
                cpu.write8(obj + INNER, back);
                return;
            }
            let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 120)) as i32;
            let mut rest = r.wrapping_add(300);
            cpu.write32(obj + TURRET_COUNT, rest as u32);
            let all_up = RISER_PART_SLOTS.into_iter().all(|at| {
                let p = cpu.read32(part + at);
                cpu.read8(p + RISER_STEP) == 1
            });
            if all_up {
                rest /= 4;
                cpu.write32(obj + TURRET_COUNT, rest as u32);
            }
            cpu.write8(obj + INNER, 0);
        }
        11 => {
            cpu.write32(obj + TURRET_COUNT, 60);
            let n = nested!(cpu, 0x0604_5d90, math::random(cpu, 3)).wrapping_add(3);
            cpu.write32(obj + SHOTS, n);
            cpu.write8(obj + INNER, 12);
        }
        12 => {
            turret_aim(cpu, obj);
            let count = cpu.read32(obj + TURRET_COUNT).wrapping_sub(1);
            cpu.write32(obj + TURRET_COUNT, count);
            if count as i32 <= 0 {
                turret_beam(cpu, obj);
                cpu.write8(obj + INNER, 13);
            }
        }
        13 => {
            let beam = cpu.read32(obj + BEAM);
            if beam != 0 {
                beam_ended(cpu, obj, beam);
                return;
            }
            let shots = cpu.read32(obj + SHOTS).wrapping_sub(1);
            cpu.write32(obj + SHOTS, shots);
            if shots as i32 <= 0 {
                let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 120)).wrapping_add(300);
                cpu.write32(obj + TURRET_COUNT, r);
                cpu.write8(obj + INNER, 0);
            } else {
                cpu.write32(obj + TURRET_COUNT, 60);
                cpu.write8(obj + INNER, 12);
            }
        }
        _ => {}
    }
}

/// Two fans of aimed shots (5 more by difficulty each, from either side of its model at random
/// within a dot, outward from its aim), with sound 26.
fn fans(cpu: &mut Cpu, obj: u32) {
    let shot = cpu.stack();
    cpu.write32(shot + AS_FLAGS, 0);
    cpu.write32(shot + AS_SPEED, FAN_SPEED);
    cpu.write32(shot + AS_SHOOTER, obj);
    for (x, out) in FAN_SIDES {
        cpu.write32(shot + AS_POSITION, x);
        cpu.write32(shot + AS_POSITION + 4, 0x10_0000);
        cpu.write32(shot + AS_POSITION + 8, 0);
        nested!(cpu, 0x0605_2c44, models::model_point(cpu, obj, shot));
        cpu.write32(shot + AS_POSITION + 8, 0);
        let angle = cpu.read16(obj + TURN_Z).wrapping_add(out);
        cpu.write16(shot + AS_ANGLE, angle);
        let (bx, by) = (cpu.read32(shot + AS_POSITION), cpu.read32(shot + AS_POSITION + 4));
        let mut i = 0;
        while i < nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 3)).wrapping_add(5) as i32 {
            let x = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x10_0000)).wrapping_add(bx);
            cpu.write32(shot + AS_POSITION, x);
            let y = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x10_0000)).wrapping_add(by);
            cpu.write32(shot + AS_POSITION + 4, y);
            nested!(cpu, 0x0604_dbc4, bullets::aimed_set(cpu, shot));
            i += 1;
        }
    }
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, FAN_SOUND));
    cpu.write32(obj + FAN_WAIT, 0);
}

/// A twin-gunned turret on a riser's part (its parent; the riser at +0xd4 of that): its life
/// (+0x50), its turn (z +0x30), a count (+0x70), its volleys' count (+0x74; its low byte picks
/// the next sweep), the turn it sweeps to (+0x78). Its two guns' shots are made on the stack, from
/// its model's points (-16, 24, 0) and (16, 24, 0).
const TWIN_LIFE: u32 = 30000;
const SWEEP_TO: u32 = 0x78;
const GUNS: [u32; 2] = [0xfff0_0000, 0x10_0000];
const GUN_Y: u32 = 0x18_0000;
const TWIN_SHOT_SPEED: u32 = 0x1_5800;
const SHOT_SOUND: u32 = 26;
const SPRAY_SOUND: u32 = 27;
const SWEEP_WIDE: [u16; 9] = [0xa000, 0x9800, 0x9000, 0x8800, 0x6000, 0x6800, 0x7000, 0x7800, 0x8000];
const SWEEP_NARROW: [u16; 10] = [0xa400, 0x9c00, 0x9400, 0x8c00, 0x8400, 0x5c00, 0x6400, 0x6c00, 0x7400, 0x7c00];
const STAGE_RATE_AT: u32 = 0x0607_46f0;
const TWIN_RATE: u32 = 5;

/// Its behaviour. 0: life 30000; 1: waits for the riser to rise (its step 15), 2: for its part
/// (step 15 on), then unshielded. 15: it fights (see [`twin_fight`]). 100: shot down (5 more to
/// the stage's rate, and the score) or its part sunk, it smokes 150-199 frames; 102: debris, its
/// riser's slot freed, and it ends.
pub fn twin_turret_behaviour(cpu: &mut Cpu, obj: u32) {
    let part = nested!(cpu, 0x0605_2970, models::follow_parent(cpu, obj));
    let parent = cpu.read32(obj + PARENT);
    let riser = cpu.read32(parent + PARENT);
    shot_or_sunk(cpu, obj, part);
    match cpu.read8(obj + RISER_STEP) as i8 {
        0 => {
            cpu.write32(obj + TURRET_LIFE, TWIN_LIFE);
            riser_step(cpu, obj, 1);
        }
        1 => {
            if cpu.read8(riser + RISER_STEP) == 15 {
                cpu.write32(obj + TURRET_COUNT, 0);
                riser_step(cpu, obj, 2);
            }
        }
        2 => {
            if cpu.read8(part + RISER_STEP) as i8 >= 15 {
                let flags = cpu.read32(obj + FLAGS) & !UNHURT;
                cpu.write32(obj + FLAGS, flags);
                cpu.write32(obj + TURRET_COUNT, 0);
                riser_step(cpu, obj, 15);
            }
        }
        15 => twin_fight(cpu, obj),
        100 => {
            if cpu.read8(obj + INNER) == 0 {
                if cpu.read32(obj + TURRET_LIFE) as i32 <= 0 {
                    turret_score(cpu, obj);
                    let rate = cpu.read32(STAGE_RATE_AT).wrapping_add(TWIN_RATE);
                    cpu.write32(STAGE_RATE_AT, rate);
                }
                let frames = nested!(cpu, 0x0604_5d90, math::random(cpu, 50)).wrapping_add(150);
                cpu.write32(obj + TURRET_COUNT, frames);
                cpu.write8(obj + INNER, 1);
            }
            let count = cpu.read32(obj + TURRET_COUNT);
            let puff = Puff { at: cpu.stack() + 0x80, spread: (0x20_0000, 0x20_0000), lift: 0, size: 0x8000 };
            burning(cpu, obj, count, puff, 30);
            let count = cpu.read32(obj + TURRET_COUNT).wrapping_sub(1);
            cpu.write32(obj + TURRET_COUNT, count);
            if count as i32 <= 0 {
                riser_step(cpu, obj, 102);
            }
        }
        102 => {
            nested!(cpu, 0x0608_08b0, debris_set(cpu, obj));
            let slot = cpu.read8(obj + SLOT) as i8 as i32 as u32;
            cpu.write32(riser.wrapping_add(4 * slot).wrapping_add(RISER_SLOTS), 0);
            cpu.write8(obj + OBJ_MODE, TO_STOP);
        }
        _ => {}
    }
}

/// Step 15's inner steps, its guns' shots made first. 0: it turns back to straight ahead and
/// rests out its count, then picks one of four patterns. 1-5: five sweeps of shot pairs fanned
/// out behind either side (wide and narrow by turns), 30 frames less by difficulty apart. 11-13 and
/// 21-23: it turns a quarter one way (held 15 frames), then sweeps across to the other quarter,
/// firing a pair straight back every 10 frames less by difficulty. 31-34: three times, aiming at
/// the nearer player 60 frames, then a spray of 8 shots (more by difficulty) from between its guns.
fn twin_fight(cpu: &mut Cpu, obj: u32) {
    let (left, right) = (cpu.stack(), cpu.stack() + 0x40);
    for (shot, x) in [left, right].into_iter().zip(GUNS) {
        cpu.write32(shot + AS_POSITION, x);
        cpu.write32(shot + AS_POSITION + 4, GUN_Y);
        cpu.write32(shot + AS_POSITION + 8, 0);
        nested!(cpu, 0x0605_2c44, models::model_point(cpu, obj, shot));
        // The left's z cleared each time; the right's is left as its point's.
        cpu.write32(left + AS_POSITION + 8, 0);
        cpu.write32(shot + AS_FLAGS, 0);
        cpu.write32(shot + AS_SPEED, TWIN_SHOT_SPEED);
        cpu.write32(shot + AS_SHOOTER, obj);
    }
    // Steps 1, 11 and 21 set up and go straight on to the next.
    let mut inner = cpu.read8(obj + INNER);
    let sweep_to = match inner {
        1 => Some(0),
        11 => Some(0x4000),
        21 => Some(0xc000),
        _ => None,
    };
    if let Some(to) = sweep_to {
        cpu.write32(obj + TURRET_COUNT, 0);
        if inner == 1 {
            cpu.write32(obj + SHOTS, 0);
        }
        cpu.write16(obj + SWEEP_TO, to);
        inner += 1;
        cpu.write8(obj + INNER, inner);
    }
    match inner {
        0 => {
            let t = turn_eighth(cpu, obj, 0);
            if counting_down(cpu, obj) || !near(t, 0) {
                return;
            }
            cpu.write16(obj + TURN_Z, 0);
            let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 4)) as i32;
            cpu.write8(obj + INNER, (r.wrapping_abs() as u8).wrapping_mul(10).wrapping_add(1));
        }
        2 => {
            let to = cpu.read16(obj + SWEEP_TO);
            let t = turn_eighth(cpu, obj, to);
            if near(t, to) {
                cpu.write16(obj + TURN_Z, to);
                cpu.write8(obj + INNER, 3);
            }
        }
        3 | 4 => {
            if counting_down(cpu, obj) {
                return;
            }
            let sweep: &[u16] = if inner == 3 { &SWEEP_WIDE } else { &SWEEP_NARROW };
            for &out in sweep {
                pair(cpu, obj, left, right, out);
            }
            nested!(cpu, 0x0600_50d4, sound::se_set(cpu, SHOT_SOUND));
            let less = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 15));
            cpu.write32(obj + TURRET_COUNT, 30u32.wrapping_sub(less));
            cpu.write8(obj + INNER, 5);
        }
        5 => {
            let shots = cpu.read32(obj + SHOTS).wrapping_add(1);
            cpu.write32(obj + SHOTS, shots);
            if (shots as i32) < 5 {
                let next = (cpu.read8(obj + SHOTS_LOW) & 1) + 3;
                cpu.write8(obj + INNER, next);
                return;
            }
            let count = cpu.read32(obj + TURRET_COUNT).wrapping_add(60);
            cpu.write32(obj + TURRET_COUNT, count);
            cpu.write8(obj + INNER, 0);
        }
        12 | 22 => {
            let to = cpu.read16(obj + SWEEP_TO);
            let t = turn_eighth(cpu, obj, to);
            if !near(t, to) {
                return;
            }
            let count = cpu.read32(obj + TURRET_COUNT).wrapping_add(1);
            cpu.write32(obj + TURRET_COUNT, count);
            if (count as i32) < 15 {
                return;
            }
            cpu.write16(obj + TURN_Z, to);
            cpu.write16(obj + SWEEP_TO, to.wrapping_neg());
            cpu.write32(obj + TURRET_COUNT, 0);
            cpu.write8(obj + INNER, inner + 1);
        }
        13 | 23 => {
            let turn = cpu.read16(obj + TURN_Z) as i16 as i32 as u32;
            let to = cpu.read16(obj + SWEEP_TO) as i16 as i32 as u32;
            let d = if inner == 13 {
                nested!(cpu, 0x0604_61f0, camera::angle_behind(cpu, turn, to))
            } else {
                nested!(cpu, 0x0604_61cc, camera::angle_ahead(cpu, turn, to))
            };
            let step = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, d, 0x100));
            let t = cpu.read16(obj + TURN_Z).wrapping_add(step as u16);
            cpu.write16(obj + TURN_Z, t);
            if t == cpu.read16(obj + SWEEP_TO) {
                cpu.write8(obj + INNER, 0);
            }
            if counting_down(cpu, obj) {
                return;
            }
            pair(cpu, obj, left, right, 0x8000);
            nested!(cpu, 0x0600_50d4, sound::se_set(cpu, SHOT_SOUND));
            let less = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 5));
            cpu.write32(obj + TURRET_COUNT, 10u32.wrapping_sub(less));
        }
        31 => {
            cpu.write32(obj + TURRET_COUNT, 0);
            cpu.write32(obj + SHOTS, 0);
            cpu.write8(obj + INNER, 32);
        }
        32 => {
            turret_aim(cpu, obj);
            let count = cpu.read32(obj + TURRET_COUNT).wrapping_add(1);
            cpu.write32(obj + TURRET_COUNT, count);
            if count as i32 > 59 {
                cpu.write8(obj + INNER, 33);
            }
        }
        33 => {
            let angle = cpu.read16(obj + TURN_Z).wrapping_add(0x8000);
            cpu.write16(left + AS_ANGLE, angle);
            let between = |cpu: &mut Cpu, at: u32| {
                let sum = cpu.read32(left + at).wrapping_add(cpu.read32(right + at));
                (sum as i32 / 2) as u32
            };
            let (x, y) = (between(cpu, AS_POSITION), between(cpu, AS_POSITION + 4));
            let mut i = 0;
            while i < nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 4)).wrapping_add(8) as i32 {
                let dx = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x10_0000));
                cpu.write32(left + AS_POSITION, dx.wrapping_add(x));
                let dy = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x10_0000));
                cpu.write32(left + AS_POSITION + 4, dy.wrapping_add(y));
                nested!(cpu, 0x0604_dbc4, bullets::aimed_set(cpu, left));
                i += 1;
            }
            nested!(cpu, 0x0600_50d4, sound::se_set(cpu, SPRAY_SOUND));
            cpu.write8(obj + INNER, 34);
        }
        34 => {
            let shots = cpu.read32(obj + SHOTS).wrapping_add(1);
            cpu.write32(obj + SHOTS, shots);
            if (shots as i32) < 3 {
                cpu.write32(obj + TURRET_COUNT, 0);
                cpu.write8(obj + INNER, 32);
            } else {
                cpu.write32(obj + TURRET_COUNT, 60);
                cpu.write8(obj + INNER, 0);
            }
        }
        _ => {}
    }
}

/// Its count, if still running, taken down one: whether it was.
fn counting_down(cpu: &mut Cpu, obj: u32) -> bool {
    let count = cpu.read32(obj + TURRET_COUNT);
    if count as i32 > 0 {
        cpu.write32(obj + TURRET_COUNT, count.wrapping_sub(1));
        return true;
    }
    false
}

/// A shot from each gun, both at its turn and `out`.
fn pair(cpu: &mut Cpu, obj: u32, left: u32, right: u32, out: u16) {
    let angle = cpu.read16(obj + TURN_Z).wrapping_add(out);
    cpu.write16(left + AS_ANGLE, angle);
    cpu.write16(right + AS_ANGLE, angle);
    nested!(cpu, 0x0604_dbc4, bullets::aimed_set(cpu, left));
    nested!(cpu, 0x0604_dbc4, bullets::aimed_set(cpu, right));
}

// ---- the laser ------------------------------------------------------------------------------------

/// A laser (an actor, actor_exec's) and the glow at its far end (a plain object): each drawn as a quad
/// of its character (the sprite flag QUAD), its corners (x, y words, pixels) in itself, pointed to
/// by +0x34; its kind 0 and frame (the laser 0xaf, the glow 0xae), coloured as the kind.
const QUAD: u32 = 4;
const CORNERS: u32 = 0x34;
const SPRITE_KIND: u32 = 0x2e;
const SPRITE_FRAME: u32 = 0x2f;
const COLOUR: u32 = 0x28;
const OWN_COLOUR: u32 = 0x2a;
const LASER_FRAME: u8 = 0xaf;
const GLOW_FRAME: u8 = 0xae;

/// Its kind 0's frame `frame`, drawn as a quad with the corners at `corners`.
fn quad_sprite(cpu: &mut Cpu, obj: u32, frame: u8, corners: u32) {
    let flags = cpu.read32(obj + FLAGS) | QUAD;
    cpu.write32(obj + FLAGS, flags);
    cpu.write8(obj + SPRITE_KIND, 0);
    cpu.write8(obj + SPRITE_FRAME, frame);
    let kind = cpu.read8(obj + SPRITE_KIND) as i8 as i32 as u32;
    let colour = cpu.read16(CHAR_BASE.wrapping_add(16 * kind));
    cpu.write16(obj + OWN_COLOUR, colour);
    cpu.write16(obj + COLOUR, colour);
    cpu.write32(obj + CORNERS, obj + corners);
}

/// The corners' x: half `width` (16.16) either side, the first and last on the left.
fn quad_width(cpu: &mut Cpu, corners: u32, width: u32) {
    let left = (width.wrapping_neg() as i32 / 0x2_0000) as u16;
    cpu.write16(corners + 12, left);
    cpu.write16(corners, left);
    let right = (width as i32 / 0x2_0000) as u16;
    cpu.write16(corners + 8, right);
    cpu.write16(corners + 4, right);
}

/// The glow (its laser +0x5c; its width +0x60, from the laser's): it follows the laser (its place
/// and turn), ending with it. Step 0: set up; 1: it swells (its scales a 16th of the way to 3 and 2
/// a frame); 2: it shrinks back (x an eighth of the way to 1 a frame, y 1); the laser's step taken
/// each frame, its step 3 the glow's end. Its quad is 4 dots deep at the laser's far end.
const GLOW_LASER: u32 = 0x5c;
const GLOW_WIDTH: u32 = 0x60;
const GLOW_CORNERS: u32 = 0x70;
const GLOW_STEP: u32 = 0x3c;

pub fn laser_glow(cpu: &mut Cpu, obj: u32) {
    let laser = cpu.read32(obj + GLOW_LASER);
    if cpu.read8(laser + OBJ_MODE) as i8 > ENDING as i8 {
        cpu.write8(obj + OBJ_MODE, TO_STOP);
        return;
    }
    for k in [0, 4, 8] {
        let v = cpu.read32(laser + POSITION + k);
        cpu.write32(obj + POSITION + k, v);
    }
    let turn = cpu.read16(laser + TURN_Z);
    cpu.write16(obj + TURN_Z, turn);
    let (sx, sy) = (obj + SCALES, obj + SCALES + 4);
    match cpu.read8(obj + GLOW_STEP) {
        0 => {
            quad_sprite(cpu, obj, GLOW_FRAME, GLOW_CORNERS);
            let width = cpu.read32(laser + LASER_WIDTH);
            cpu.write32(obj + GLOW_WIDTH, width);
            cpu.write32(sy, 0);
            cpu.write32(sx, 0);
            cpu.write8(obj + GLOW_STEP, 1);
        }
        1 => {
            let x = cpu.read32(sx);
            cpu.write32(sx, x.wrapping_add((0x3_0000u32.wrapping_sub(x) as i32 / 16) as u32));
            let y = cpu.read32(sy);
            cpu.write32(sy, y.wrapping_add((0x2_0000u32.wrapping_sub(y) as i32 / 16) as u32));
            let step = cpu.read8(laser + RISER_STEP);
            cpu.write8(obj + GLOW_STEP, step);
        }
        2 => {
            let x = cpu.read32(sx);
            cpu.write32(sx, x.wrapping_add((0x1_0000u32.wrapping_sub(x) as i32 / 8) as u32));
            cpu.write32(sy, 0x1_0000);
            let step = cpu.read8(laser + RISER_STEP);
            cpu.write8(obj + GLOW_STEP, step);
        }
        3 => cpu.write8(obj + OBJ_MODE, TO_STOP),
        _ => {}
    }
    let corners = obj + GLOW_CORNERS;
    let width = cpu.read32(obj + GLOW_WIDTH);
    quad_width(cpu, corners, width);
    let far = laser + LASER_CORNERS;
    let (y0, y1) = (cpu.read16(far + 2), cpu.read16(far + 6));
    cpu.write16(corners + 10, y0);
    cpu.write16(corners + 14, y1);
    cpu.write16(corners + 2, y0.wrapping_sub(4));
    cpu.write16(corners + 6, y1.wrapping_sub(4));
}

/// The laser (its parent +0xb0; ends with it, or once the parent's step passes 99): placed at
/// +0x70 (x, y, z) on its parent's model (z 0), turned a quarter from it (back a quarter if the
/// parent is flipped), its sound's control (+0x8c), its width (+0x80) and length (+0x84, both
/// 16.16), a count (+0x88), the frames it stays at full length (+0x7c, a word). Step 0: set up
/// (sound 0x5c, 6 dots wide, its boxes, no hits); 1: 60 frames; 2: hitting, it grows a dot a
/// frame to 256 dots, held there for its frames; 3: no hits, it narrows (its x scale, to an
/// eighth), then its near end catches up with the far end. Its sound stops as it ends.
const LASER_PARENT: u32 = 0xb0;
const LASER_AT: u32 = 0x70;
const LASER_HELD: u32 = 0x7c;
const LASER_WIDTH: u32 = 0x80;
const LASER_LENGTH: u32 = 0x84;
const LASER_COUNT: u32 = 0x88;
const LASER_SOUND: u32 = 0x8c;
const LASER_CORNERS: u32 = 0x90;
const LASER_BOXES: u32 = 0x0609_fbf0;
const LASER_SOUND_ID: u32 = 0x5c;
const FULL_LENGTH: i32 = 0x100_0000;
const LASER_THIN: i32 = 0x2000;

pub fn laser_behaviour(cpu: &mut Cpu, obj: u32) {
    let parent = cpu.read32(obj + LASER_PARENT);
    if cpu.read8(parent + OBJ_MODE) as i8 > ENDING as i8 || cpu.read8(parent + RISER_STEP) as i8 > 99 {
        cpu.write8(obj + OBJ_MODE, TO_STOP);
    }
    for k in [0, 4, 8] {
        let v = cpu.read32(obj + LASER_AT + k);
        cpu.write32(obj + POSITION + k, v);
    }
    nested!(cpu, 0x0605_2c44, models::model_point(cpu, parent, obj + POSITION));
    cpu.write32(obj + POSITION + 8, 0);
    let parent = cpu.read32(obj + LASER_PARENT);
    let quarter: u16 = if cpu.read32(parent + FLAGS) & FLIP_X != 0 { 0xc000 } else { 0x4000 };
    let turn = cpu.read16(parent + TURN_Z).wrapping_add(quarter);
    cpu.write16(obj + TURN_Z, turn);
    let b = cpu.read8(obj + OBJ_FLAGS_BYTE) | 1;
    cpu.write8(obj + OBJ_FLAGS_BYTE, b);
    let corners = obj + LASER_CORNERS;
    match cpu.read8(obj + RISER_STEP) {
        0 => {
            let control = nested!(cpu, 0x0600_50d4, sound::se_set(cpu, LASER_SOUND_ID));
            cpu.write8(obj + LASER_SOUND, control as u8);
            quad_sprite(cpu, obj, LASER_FRAME, LASER_CORNERS);
            cpu.write32(obj + LASER_WIDTH, 0x6_0000);
            cpu.write32(obj + LASER_LENGTH, 0);
            cpu.write16(corners + 14, 0);
            cpu.write16(corners + 10, 0);
            nested!(cpu, 0x0605_2b08, models::beam_boxes(cpu, obj, LASER_BOXES));
            cpu.write32(obj + BEAM_LIST, obj + BEAM_LIST_AT);
            cpu.write32(obj + 0x60, 0);
            cpu.write32(obj + 0x5c, 0);
            let flags = cpu.read32(obj + FLAGS) | NO_HITS;
            cpu.write32(obj + FLAGS, flags);
            cpu.write32(obj + LASER_COUNT, 0);
            riser_step(cpu, obj, 1);
        }
        1 => {
            let count = cpu.read32(obj + LASER_COUNT).wrapping_add(1);
            cpu.write32(obj + LASER_COUNT, count);
            if count as i32 > 59 {
                let flags = cpu.read32(obj + FLAGS) & !NO_HITS;
                cpu.write32(obj + FLAGS, flags);
                riser_step(cpu, obj, 2);
            }
        }
        2 => {
            let length = cpu.read32(obj + LASER_LENGTH).wrapping_add(0x1_0000);
            cpu.write32(obj + LASER_LENGTH, length);
            nested!(cpu, 0x0605_2b88, models::beam_length(cpu, obj, length));
            if cpu.read32(obj + LASER_LENGTH) as i32 >= FULL_LENGTH {
                cpu.write32(obj + LASER_LENGTH, FULL_LENGTH as u32);
                let held = cpu.read16(obj + LASER_HELD).wrapping_sub(1);
                cpu.write16(obj + LASER_HELD, held);
                if held as i16 <= 0 {
                    let flags = cpu.read32(obj + FLAGS) | NO_HITS;
                    cpu.write32(obj + FLAGS, flags);
                    riser_step(cpu, obj, 3);
                }
            }
        }
        3 => {
            let w = cpu.read32(obj + SCALES);
            let w = nested!(cpu, 0x0604_6168, actors::approach(cpu, w, 0, 0x1000));
            cpu.write32(obj + SCALES, w);
            if w as i32 <= LASER_THIN {
                // Its near end moved toward the far (the step added to where it was: it speeds up).
                let (near, far) = (cpu.read16(corners + 10) as i16 as i32 as u32, cpu.read16(corners + 2) as i16 as i32 as u32);
                let step = nested!(cpu, 0x0604_6168, actors::approach(cpu, near, far, 16));
                let y = (step as u16).wrapping_add(cpu.read16(corners + 10));
                cpu.write16(corners + 10, y);
                cpu.write16(corners + 14, y);
                let gap = (y as i16 as i32 - cpu.read16(corners + 2) as i16 as i32).wrapping_abs();
                if gap <= 16 {
                    cpu.write8(obj + OBJ_MODE, TO_STOP);
                }
            }
        }
        _ => {}
    }
    let width = cpu.read32(obj + LASER_WIDTH);
    quad_width(cpu, corners, width);
    let far = (cpu.read32(obj + LASER_LENGTH).wrapping_neg() as i32 / 0x1_0000) as u16;
    cpu.write16(corners + 6, far);
    cpu.write16(corners + 2, far);
    if cpu.read8(obj + OBJ_MODE) as i8 > ENDING as i8 {
        let control = cpu.read8(obj + LASER_SOUND) as i8 as i32 as u32;
        nested!(cpu, 0x0600_52e0, sound::se_stop(cpu, LASER_SOUND_ID, control));
        cpu.write8(obj + LASER_SOUND, 0xff);
    }
}

/// A laser made (list 2, 0x174 bytes, actor_exec's) on `parent` (its number and the byte +0x54),
/// 20 dots across and `y` down its model, at full length 120 frames; with its glow (list 3, 0x80
/// bytes, plain). The laser, or 0.
const LASER_BEHAVIOUR: u32 = 0x0608_28d8;
const LASER_GLOW: u32 = 0x0608_273c;
const LASER_X: u32 = 0x14_0000;
const LASER_FRAMES: u16 = 120;

pub fn laser_set(cpu: &mut Cpu, parent: u32, y: u32) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0x174)) as i8 as i32 as u32;
    let obj = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if obj == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_62ac, actors::actor_setup(cpu, obj));
    let flags = cpu.read32(obj + FLAGS) | SPRITE_FLAG_400;
    cpu.write32(obj + FLAGS, flags);
    cpu.write32(obj + BEHAVIOUR, LASER_BEHAVIOUR);
    let n = cpu.read8(parent + NUMBER);
    cpu.write8(obj + NUMBER, n);
    cpu.write8(obj + BYTE_33, 8);
    let b = cpu.read8(parent + BYTE_54);
    cpu.write8(obj + BYTE_54, b);
    cpu.write32(obj + LASER_PARENT, parent);
    cpu.write32(obj + LASER_AT, LASER_X);
    cpu.write32(obj + LASER_AT + 4, y);
    cpu.write32(obj + LASER_AT + 8, 0);
    cpu.write16(obj + LASER_HELD, LASER_FRAMES);
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0x80)) as i8 as i32 as u32;
    let glow = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 3, blocks, PLAIN_EXEC));
    if glow != 0 {
        nested!(cpu, 0x0604_dd50, effects::actor_init(cpu, glow));
        let flags = cpu.read32(glow + FLAGS) | SPRITE_FLAG_400;
        cpu.write32(glow + FLAGS, flags);
        cpu.write32(glow + BEHAVIOUR, LASER_GLOW);
        let n = cpu.read8(parent + NUMBER);
        cpu.write8(glow + NUMBER, n);
        cpu.write8(glow + BYTE_33, 8);
        cpu.write32(glow + GLOW_LASER, obj);
    }
    obj
}

// ---- the laser cannons ----------------------------------------------------------------------------

/// A laser cannon on a riser's part, one of a pair (the riser's variant (+0x6e) picks the part's
/// slot holding the other: +0xbc's +0xb8 for 1, else +0xb8's): its life (+0x50), its turns (x
/// +0x2c, z +0x30), its lasers' timers (words: a laser made as one reaches 0, off below 0; the
/// middle one only at the hardest), a count (+0x78), its depth kept (+0x7c while it comes out),
/// then its aim (+0x7c, a word).
const CANNON_LIFE: u32 = 20000;
const LASER_TIMERS: [(u32, u32); 3] = [(0x70, 0xfffb_0000), (0x72, 0x22_0000), (0x74, 0x4a_0000)];
const MIDDLE_LASER: u32 = 0x72;
const LAST_LASER: u32 = 0x74;
const CANNON_COUNT: u32 = 0x78;
const KEPT_Z: u32 = 0x7c;
const CANNON_AIM: u32 = 0x7c;
const CANNON_RATE: u32 = 10;
const HARDEST: u32 = 4;

/// Its behaviour. 0: life 20000, turned down (x a quarter back), its lasers off. 1: when the
/// riser reaches step 12 its depth is kept; 2: at the riser's 15, 3: it comes 8 dots out (half a
/// dot a frame) and waits 30 frames, 4: it turns up (at most 0x80 a frame) and waits 10, 5: it goes
/// back (a dot a frame). 6: once its part is up (past 14) and the other cannon past 5, unshielded,
/// it leads (15, variant 1) or follows (16: turned opposite the other). With the other gone, 380
/// frames (17) and it fights alone (18). 100: shot down (10 more to the stage's rate, and the
/// score) or its part sunk, it smokes 90-139 frames, then its part sinks; 102: debris (tumbling
/// slowly) and the end.
pub fn laser_cannon_behaviour(cpu: &mut Cpu, obj: u32) {
    let part = nested!(cpu, 0x0605_2970, models::follow_parent(cpu, obj));
    let parent = cpu.read32(obj + PARENT);
    let riser = cpu.read32(parent + PARENT);
    let slot = if cpu.read8(riser + VARIANT) == 1 { 0xbc } else { 0xb8 };
    let holder = cpu.read32(part + slot);
    let other = cpu.read32(holder + 0xb8);
    shot_or_sunk(cpu, obj, part);
    match cpu.read8(obj + RISER_STEP) {
        0 => {
            cpu.write32(obj + TURRET_LIFE, CANNON_LIFE);
            cpu.write16(obj + TURN_X, 0xc000);
            cpu.write32(obj + 0x74, 0xffff_ffff);
            cpu.write32(obj + 0x70, 0xffff_ffff);
            riser_step(cpu, obj, 1);
        }
        1 => {
            if cpu.read8(riser + RISER_STEP) == 12 {
                let z = cpu.read32(obj + SCENERY_Z);
                cpu.write32(obj + KEPT_Z, z);
                riser_step(cpu, obj, 2);
            }
        }
        2 => {
            if cpu.read8(riser + RISER_STEP) == 15 {
                cpu.write32(obj + CANNON_COUNT, 0);
                riser_step(cpu, obj, 3);
            }
        }
        3 => {
            let (z, out) = (cpu.read32(obj + SCENERY_Z), cpu.read32(obj + KEPT_Z).wrapping_sub(0x8_0000));
            let z = nested!(cpu, 0x0604_6168, actors::approach(cpu, z, out, 0x8000));
            cpu.write32(obj + SCENERY_Z, z);
            if z == cpu.read32(obj + KEPT_Z).wrapping_sub(0x8_0000) {
                let count = cpu.read32(obj + CANNON_COUNT).wrapping_add(1);
                cpu.write32(obj + CANNON_COUNT, count);
                if count as i32 > 29 {
                    cpu.write32(obj + CANNON_COUNT, 0);
                    riser_step(cpu, obj, 4);
                }
            }
        }
        4 => {
            if turn_toward(cpu, obj, TURN_X, 0, 0x80) == 0 {
                let count = cpu.read32(obj + CANNON_COUNT).wrapping_add(1);
                cpu.write32(obj + CANNON_COUNT, count);
                if count as i32 > 9 {
                    riser_step(cpu, obj, 5);
                }
            }
        }
        5 => {
            let (z, back) = (cpu.read32(obj + SCENERY_Z), cpu.read32(obj + KEPT_Z));
            let z = nested!(cpu, 0x0604_6168, actors::approach(cpu, z, back, 0x1_0000));
            cpu.write32(obj + SCENERY_Z, z);
            if z == cpu.read32(obj + KEPT_Z) {
                riser_step(cpu, obj, 6);
            }
        }
        6 => {
            if cpu.read8(part + RISER_STEP) as i8 > 14 && cpu.read8(other + RISER_STEP) as i8 > 5 {
                let flags = cpu.read32(obj + FLAGS) & !UNHURT;
                cpu.write32(obj + FLAGS, flags);
                let leads = cpu.read8(riser + VARIANT) == 1;
                riser_step(cpu, obj, if leads { 15 } else { 16 });
            }
        }
        15 | 16 => {
            if cpu.read8(obj + RISER_STEP) == 15 {
                cannon_lead(cpu, obj, other);
            } else {
                let t = cpu.read16(other + TURN_Z).wrapping_neg();
                cpu.write16(obj + TURN_Z, t);
            }
            if cpu.read8(other + RISER_STEP) as i8 > 99 {
                riser_step(cpu, obj, 17);
            }
        }
        17 => {
            if cpu.read8(obj + INNER) == 0 {
                cpu.write32(obj + CANNON_COUNT, 380);
                cpu.write8(obj + INNER, 1);
            }
            let count = cpu.read32(obj + CANNON_COUNT).wrapping_sub(1);
            cpu.write32(obj + CANNON_COUNT, count);
            if count as i32 <= 0 {
                riser_step(cpu, obj, 18);
            }
        }
        18 => cannon_alone(cpu, obj, riser),
        100 => {
            let puff = Puff { at: cpu.stack(), spread: (0x20_0000, 0x20_0000), lift: 0x20_0000, size: 0xa000 };
            shot_down(cpu, obj, CANNON_COUNT, CANNON_RATE, puff);
        }
        102 => {
            let d = nested!(cpu, 0x0608_08b0, debris_set(cpu, obj));
            if d != 0 {
                for k in [0, 4] {
                    let spin = cpu.read16(d + SPIN + k) as i16 as i32 as u32;
                    let spin = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, spin, 16));
                    cpu.write16(d + SPIN + k, spin as u16);
                }
            }
            cpu.write8(obj + OBJ_MODE, TO_STOP);
        }
        _ => {}
    }
    for (at, y) in LASER_TIMERS {
        let t = cpu.read16(obj + at) as i16;
        if t < 0 {
            continue;
        }
        let now = if at == MIDDLE_LASER {
            nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 0)) == HARDEST && cpu.read16(obj + at) == 0
        } else {
            t == 0
        };
        if now {
            nested!(cpu, 0x0608_2bbc, laser_set(cpu, obj, y));
        }
        let t = cpu.read16(obj + at).wrapping_sub(1);
        cpu.write16(obj + at, t);
    }
}

/// The leader's fight (step 15's inner steps), firing for both. 0: it rests out its count, then
/// picks a pattern. 1, 21, 31: it turns to a random aim (0x2000-0x2f00), then both fire (their
/// lasers' timers 0) and it holds 400 frames after the other's last laser. 11: it turns to 0x2000,
/// both fire, it holds 180 frames after the other's last laser, then turns home (13). Its rest
/// after: 120 frames less by difficulty.
fn cannon_lead(cpu: &mut Cpu, obj: u32, other: u32) {
    let mut inner = cpu.read8(obj + INNER);
    if matches!(inner, 1 | 21 | 31) {
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 16));
        cpu.write16(obj + CANNON_AIM, (r << 8).wrapping_add(0x2000) as u16);
        inner = 2;
        cpu.write8(obj + INNER, inner);
    }
    match inner {
        0 => cannon_rest(cpu, obj),
        2 | 11 => {
            let (to, hold) = if inner == 2 { (cpu.read16(obj + CANNON_AIM), 400) } else { (0x2000, 180) };
            if cannon_turn(cpu, obj, to) {
                for at in [obj, other] {
                    for (timer, _) in LASER_TIMERS {
                        cpu.write16(at + timer, 0);
                    }
                }
                cpu.write32(obj + CANNON_COUNT, hold);
                cpu.write8(obj + INNER, inner + 1);
            }
        }
        3 | 12 => {
            if (cpu.read16(other + LAST_LASER) as i16) < 0 && cannon_held(cpu, obj) {
                cannon_hold_ended(cpu, obj, inner == 3);
            }
        }
        13 => turn_home(cpu, obj),
        _ => {}
    }
}

/// The lone cannon's fight (step 18's inner steps), as the leader's (variant 2 mirrored), firing
/// for itself: 11-14 its quarter turn, hold and turn home.
fn cannon_alone(cpu: &mut Cpu, obj: u32, riser: u32) {
    let mirrored = cpu.read8(riser + VARIANT) == 2;
    let mut inner = cpu.read8(obj + INNER);
    if matches!(inner, 1 | 11 | 21 | 31) {
        let aim = if inner == 11 {
            0x2000
        } else {
            let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 16));
            (r << 8).wrapping_add(0x2000) as u16
        };
        cpu.write16(obj + CANNON_AIM, aim);
        if mirrored {
            cpu.write16(obj + CANNON_AIM, aim.wrapping_neg());
        }
        inner = if inner == 11 { 12 } else { 2 };
        cpu.write8(obj + INNER, inner);
    }
    match inner {
        0 => cannon_rest(cpu, obj),
        2 | 12 => {
            let to = cpu.read16(obj + CANNON_AIM);
            if cannon_turn(cpu, obj, to) {
                for (timer, _) in LASER_TIMERS {
                    cpu.write16(obj + timer, 0);
                }
                cpu.write32(obj + CANNON_COUNT, if inner == 2 { 400 } else { 180 });
                cpu.write8(obj + INNER, inner + 1);
            }
        }
        3 | 13 => {
            if (cpu.read16(obj + LAST_LASER) as i16) < 0 && cannon_held(cpu, obj) {
                cannon_hold_ended(cpu, obj, inner == 3);
            }
        }
        14 => turn_home(cpu, obj),
        _ => {}
    }
}

/// Inner step 0: its count, if any, taken down; else a pattern picked (1, 11, 21 or 31).
fn cannon_rest(cpu: &mut Cpu, obj: u32) {
    let count = cpu.read32(obj + CANNON_COUNT);
    if count != 0 {
        cpu.write32(obj + CANNON_COUNT, count.wrapping_sub(1));
        return;
    }
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 4)) as i32;
    cpu.write8(obj + INNER, (r.wrapping_abs() as u8).wrapping_mul(10).wrapping_add(1));
}

/// Its turn (z) moved a 16th of the way to `to` (at most 0x60, 32 more by difficulty); within 0xff
/// of it, set to it: whether it got there.
fn cannon_turn(cpu: &mut Cpu, obj: u32, to: u16) -> bool {
    let most = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 32)).wrapping_add(0x60);
    let turn = cpu.read16(obj + TURN_Z) as i16 as i32 as u32;
    let d = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, turn, to as i16 as i32 as u32)) as i32 / 16;
    let step = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, d as u32, most));
    let t = cpu.read16(obj + TURN_Z).wrapping_add(step as u16);
    cpu.write16(obj + TURN_Z, t);
    if (t as i16 as i32 - to as i16 as i32).wrapping_abs() > 0xff {
        return false;
    }
    cpu.write16(obj + TURN_Z, to);
    true
}

/// Its count taken down: whether it ran out.
fn cannon_held(cpu: &mut Cpu, obj: u32) -> bool {
    let count = cpu.read32(obj + CANNON_COUNT).wrapping_sub(1);
    cpu.write32(obj + CANNON_COUNT, count);
    count as i32 <= 0
}

/// Its hold over: a rest and back to 0 (`rest`), else on to turn home.
fn cannon_hold_ended(cpu: &mut Cpu, obj: u32, rest: bool) {
    if rest {
        cannon_rested(cpu, obj);
    } else {
        let inner = cpu.read8(obj + INNER) + 1;
        cpu.write8(obj + INNER, inner);
    }
}

/// A rest of 120 frames less by difficulty, and back to inner step 0.
fn cannon_rested(cpu: &mut Cpu, obj: u32) {
    let less = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 30));
    cpu.write32(obj + CANNON_COUNT, 120u32.wrapping_sub(less));
    cpu.write8(obj + INNER, 0);
}

/// It turns back to straight (at most 16, 8 more by difficulty, a frame); there, its count taken
/// down, and at its end a rest.
fn turn_home(cpu: &mut Cpu, obj: u32) {
    let most = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 8)).wrapping_add(16);
    let turn = cpu.read16(obj + TURN_Z) as i16 as i32 as u32;
    let d = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, turn, 0));
    let step = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, d, most));
    let t = cpu.read16(obj + TURN_Z).wrapping_add(step as u16);
    cpu.write16(obj + TURN_Z, t);
    if (t as i16 as i32).wrapping_abs() > 0xff {
        return;
    }
    cpu.write16(obj + TURN_Z, 0);
    if cannon_held(cpu, obj) {
        cannon_rested(cpu, obj);
    }
}

/// Step 100 of a riser's piece (its count at `count`): shot down (`rate` more to the stage's rate,
/// and the score) or its part sunk, it smokes 90-139 frames, then its part sinks and it goes on to
/// 102.
fn shot_down(cpu: &mut Cpu, obj: u32, count_at: u32, rate: u32, puff: Puff) {
    if cpu.read8(obj + INNER) == 0 {
        let frames = nested!(cpu, 0x0604_5d90, math::random(cpu, 50)).wrapping_add(90);
        cpu.write32(obj + count_at, frames);
        if cpu.read32(obj + TURRET_LIFE) as i32 <= 0 {
            turret_score(cpu, obj);
            let r = cpu.read32(STAGE_RATE_AT).wrapping_add(rate);
            cpu.write32(STAGE_RATE_AT, r);
        }
        cpu.write8(obj + INNER, 1);
    }
    let count = cpu.read32(obj + count_at);
    burning(cpu, obj, count, puff, 30);
    let count = cpu.read32(obj + count_at).wrapping_sub(1);
    cpu.write32(obj + count_at, count);
    if count as i32 <= 0 {
        let parent = cpu.read32(obj + PARENT);
        riser_step(cpu, parent, 100);
        riser_step(cpu, obj, 102);
    }
}

// ---- the spinner ----------------------------------------------------------------------------------

/// A spinning piece on a riser's part: its life (+0x50), its turn (z) 0x400 more a frame while it
/// lives, a count (+0x7c), its spin (+0x78; its low word +0x7a added to its turn).
const SPINNER_LIFE: u32 = 2000;
const SPINNER_TIME: u32 = 1500;
const SPINNER_COUNT: u32 = 0x7c;
/// The count while it smokes (the spinner's and the aiming gun's).
const SMOKE_COUNT: u32 = 0x78;
const SPIN_UP: u32 = 0x78;
const SPIN_UP_LOW: u32 = 0x7a;

/// Its behaviour. 0: life 2000; 1: once its part is up (past 14), unshielded for 1500 frames (15);
/// 16: it spins up (16 more a frame) until past 0x7ff, then is done (no hits, 100). 100: shot
/// down (5 more to the stage's rate, and the score), timed out or its part sunk, it smokes 90-139
/// frames, then its part sinks; 102: debris that bursts.
pub fn spinner_behaviour(cpu: &mut Cpu, obj: u32) {
    let part = nested!(cpu, 0x0605_2970, models::follow_parent(cpu, obj));
    if (cpu.read8(obj + RISER_STEP) as i8) < 100 {
        shot_or_sunk(cpu, obj, part);
        let t = cpu.read16(obj + TURN_Z).wrapping_add(0x400);
        cpu.write16(obj + TURN_Z, t);
    }
    match cpu.read8(obj + RISER_STEP) as i8 {
        0 => {
            cpu.write32(obj + TURRET_LIFE, SPINNER_LIFE);
            riser_step(cpu, obj, 1);
        }
        1 => {
            if cpu.read8(part + RISER_STEP) as i8 > 14 {
                let flags = cpu.read32(obj + FLAGS) & !UNHURT;
                cpu.write32(obj + FLAGS, flags);
                cpu.write32(obj + SPINNER_COUNT, SPINNER_TIME);
                riser_step(cpu, obj, 15);
            }
        }
        15 => {
            let count = cpu.read32(obj + SPINNER_COUNT).wrapping_sub(1);
            cpu.write32(obj + SPINNER_COUNT, count);
            if count as i32 <= 0 {
                cpu.write32(obj + SPIN_UP, 0);
                riser_step(cpu, obj, 16);
            }
        }
        16 => {
            let spin = cpu.read32(obj + SPIN_UP).wrapping_add(16);
            cpu.write32(obj + SPIN_UP, spin);
            let t = cpu.read16(obj + TURN_Z).wrapping_add(cpu.read16(obj + SPIN_UP_LOW));
            cpu.write16(obj + TURN_Z, t);
            if cpu.read32(obj + SPIN_UP) as i32 > 0x7ff {
                let flags = cpu.read32(obj + FLAGS) | NO_HITS;
                cpu.write32(obj + FLAGS, flags);
                riser_step(cpu, obj, 100);
            }
        }
        100 => {
            let puff = Puff { at: cpu.stack(), spread: (0x20_0000, 0x20_0000), lift: 0, size: 0x8000 };
            shot_down(cpu, obj, SMOKE_COUNT, 5, puff);
        }
        102 => bursting_debris(cpu, obj),
        _ => {}
    }
}

// ---- the arc --------------------------------------------------------------------------------------

/// An electric arc between two points (the struct at `p`: from (x, y, z), its segments (+0x14),
/// its zig (+0x18, a word), to (x, y, z, +0x1c), its owner (+0x3c, its number the effects')):
/// one-frame line polygons (effects of flags 0x23: a line from the place to the second corner,
/// +0x74, in dots) laid from both ends at once. Each is a segment long (the distance over the
/// segments), less up to half at random, turned up to 0x2000 at random off the line between and by
/// the zig (the far end's the other way); the zig closes by its share (over half the segments and
/// one) a pair. Once the ends are within a segment, or past each other, the last two are joined.
/// The SH-2 code's frame: the two effects (+0, +0x40), the turn struct (+0x80, math.rs's
/// turn_point's) with the line's angle, the zig and its share after it.
const ARC_SEGMENTS: u32 = 0x14;
const ARC_ZIG: u32 = 0x18;
const ARC_TO: u32 = 0x1c;
const ARC_OWNER: u32 = 0x3c;
const ARC_FLAGS: u32 = 0x23;
const LINE: u32 = 0x70;
const T_SCALES: u32 = 0xc;
const T_FLAGS: u32 = 0x14;
const T_ANGLE: u32 = 0x18;
const T_LINE: u32 = 0x1c;
const T_ZIG: u32 = 0x1e;
const T_ZIG_SHARE: u32 = 0x20;

pub fn arc_draw(cpu: &mut Cpu, p: u32) {
    let (from, to, t) = (cpu.stack(), cpu.stack() + 0x40, cpu.stack() + 0x80);
    for k in [0, 4, 8] {
        let v = cpu.read32(p + k);
        cpu.write32(from + effects::X + k, v);
        let v = cpu.read32(p + ARC_TO + k);
        cpu.write32(to + effects::X + k, v);
    }
    let colour = cpu.read16(CHAR_BASE);
    let owner = cpu.read32(p + ARC_OWNER);
    let number = cpu.read8(owner + NUMBER);
    for fx in [from, to] {
        cpu.write32(fx + effects::SCALE_X, 0x1_0000);
        cpu.write32(fx + effects::SCALE_Y, 0x1_0000);
        cpu.write32(fx + effects::FLAGS_LONG, ARC_FLAGS);
        cpu.write16(fx + effects::ANGLE, 0);
        cpu.write16(fx + effects::FRAMES, 1);
        cpu.write8(fx + effects::FRAMES_2, 0);
        cpu.write16(fx + effects::COLOUR, colour);
        cpu.write8(fx + effects::PLAYER, number);
    }
    let (dx, dy) = apart(cpu, from, to);
    let length = nested!(cpu, 0x0604_5e60, aim::length(cpu, dx, dy));
    let segments = cpu.read32(p + ARC_SEGMENTS);
    let segment = nested!(cpu, 0x0600_6a78, math::quotient(cpu, length, segments));
    cpu.write32(t + T_SCALES + 4, 0x1_0000);
    cpu.write32(t + T_SCALES, 0x1_0000);
    cpu.write32(t + T_FLAGS, 0);
    let (dx, dy) = apart(cpu, from, to);
    let line = nested!(cpu, 0x0606_780c, sgl::sl_atan(cpu, dx, dy)) as u16;
    cpu.write16(t + T_LINE, line.wrapping_add(0x4000));
    let zig = cpu.read16(p + ARC_ZIG);
    cpu.write16(t + T_ZIG, zig);
    let zig = cpu.read16(p + ARC_ZIG) as i16 as i32 as u32;
    let shares = (cpu.read32(p + ARC_SEGMENTS) as i32 / 2 + 1) as u32;
    let share = nested!(cpu, 0x0600_6a78, math::quotient(cpu, zig, shares));
    cpu.write16(t + T_ZIG_SHARE, (share as u16).wrapping_neg());
    let (a, b) = loop {
        let a = nested!(cpu, 0x0604_e6d8, effects::polygon_set(cpu, from));
        if a == 0 {
            return;
        }
        let b = nested!(cpu, 0x0604_e6d8, effects::polygon_set(cpu, to));
        if b == 0 {
            cpu.write8(a + OBJ_MODE, TO_STOP);
            return;
        }
        let half = (segment as i32 / 2) as u32;
        arc_segment(cpu, t, from, a, half, segment, false);
        arc_segment(cpu, t, to, b, half, segment, true);
        let (dx, dy) = apart(cpu, from, to);
        if (nested!(cpu, 0x0604_5e60, aim::length(cpu, dx, dy)) as i32) < segment as i32 {
            break (a, b);
        }
        let (dx, dy) = apart(cpu, from, to);
        let now = nested!(cpu, 0x0606_780c, sgl::sl_atan(cpu, dx, dy)) as i16 as i32;
        let line = cpu.read16(t + T_LINE) as i16 as i32 as u32;
        let off = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, line, now.wrapping_add(0x4000) as u32));
        if (off as i32).wrapping_abs() > 0x4000 {
            break (a, b);
        }
        let zig = cpu.read16(t + T_ZIG).wrapping_add(cpu.read16(t + T_ZIG_SHARE));
        cpu.write16(t + T_ZIG, zig);
    };
    cpu.write32(t, 0);
    let (dx, dy) = apart(cpu, a + POSITION, b + POSITION);
    let gap = nested!(cpu, 0x0604_5e60, aim::length(cpu, dx, dy));
    cpu.write32(t + 4, gap.wrapping_neg());
    let (dx, dy) = apart(cpu, a + POSITION, b + POSITION);
    let line = nested!(cpu, 0x0606_780c, sgl::sl_atan(cpu, dx, dy)) as u16;
    cpu.write16(t + T_ANGLE, line.wrapping_add(0x4000));
    nested!(cpu, 0x0606_4eac, math::turn_point(cpu, t));
    let (x, y) = (cpu.read32(t), cpu.read32(t + 4));
    cpu.write16(a + LINE + 4, dots(x));
    cpu.write16(b + LINE + 4, dots(x.wrapping_neg()));
    cpu.write16(a + LINE + 6, dots(y));
    cpu.write16(b + LINE + 6, dots(y.wrapping_neg()));
}

/// The (x, y) from `p` to `q`.
fn apart(cpu: &mut Cpu, p: u32, q: u32) -> (u32, u32) {
    let dx = cpu.read32(q).wrapping_sub(cpu.read32(p));
    let dy = cpu.read32(q + 4).wrapping_sub(cpu.read32(p + 4));
    (dx, dy)
}

/// A 16.16 length in whole dots (toward 0).
fn dots(v: u32) -> u16 {
    (v as i32 / 0x1_0000) as u16
}

/// One segment of the arc: the line polygon `line` from the effect `fx`'s place, which moves on
/// to its end (the far end's turned back, its zig the other way).
fn arc_segment(cpu: &mut Cpu, t: u32, fx: u32, line: u32, half: u32, segment: u32, far: bool) {
    cpu.write32(t, 0);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, half));
    cpu.write32(t + 4, r.wrapping_sub(segment));
    let off = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x2000)) as u16;
    let (along, zig) = (cpu.read16(t + T_LINE), cpu.read16(t + T_ZIG));
    let angle = if far { along.wrapping_sub(zig).wrapping_add(off).wrapping_add(0x8000) } else { along.wrapping_add(zig).wrapping_add(off) };
    cpu.write16(t + T_ANGLE, angle);
    nested!(cpu, 0x0606_4eac, math::turn_point(cpu, t));
    cpu.write16(line + LINE + 2, 0);
    cpu.write16(line + LINE, 0);
    let (x, y) = (cpu.read32(t), cpu.read32(t + 4));
    cpu.write16(line + LINE + 4, dots(x));
    cpu.write16(line + LINE + 6, dots(y));
    let fx_x = cpu.read32(fx).wrapping_add(x);
    cpu.write32(fx, fx_x);
    let fx_y = cpu.read32(fx + 4).wrapping_add(y);
    cpu.write32(fx + 4, fx_y);
}

/// A square of electric arcs on a riser's part (its parent): one edge drawn a frame, by the game's
/// frames (the corners 9 dots out either way, 15 in front, on its model), each an arc of 3 segments
/// zigged 0x2000. 0: set up; 1: once its part is up (past 14), on (15; 16 and 17 back to 15). It
/// is off (100) once its part, or the part at its part's +0xc0's +0xb8, sinks; once the part at
/// +0xc0 sinks too, its parent sinks and it waits (101) for the parent to be 76 dots down, then
/// frees its slot and ends.
const SQUARE_EDGES: [((u32, u32), (u32, u32)); 4] = [
    ((0x9_0000, 0xfff7_0000), (0xfff7_0000, 0xfff7_0000)),
    ((0xfff7_0000, 0xfff7_0000), (0xfff7_0000, 0x9_0000)),
    ((0xfff7_0000, 0x9_0000), (0x9_0000, 0x9_0000)),
    ((0x9_0000, 0x9_0000), (0x9_0000, 0xfff7_0000)),
];
const SQUARE_Z: u32 = 0xfff1_0000;
const SQUARE_SEGMENTS: u32 = 3;
const SQUARE_ZIG: u16 = 0x2000;
const SQUARE_WORD: u32 = 0x74;
const SUNK_Y: i32 = 0x4bf_ffff;

pub fn arc_square_behaviour(cpu: &mut Cpu, obj: u32) {
    let part = nested!(cpu, 0x0605_2970, models::follow_parent(cpu, obj));
    let parent = cpu.read32(obj + PARENT);
    if (cpu.read8(obj + RISER_STEP) as i8) < 100 {
        let beside = cpu.read32(part + 0xc0);
        let other = cpu.read32(beside + 0xb8);
        if cpu.read8(other + RISER_STEP) as i8 > 99 || cpu.read8(part + RISER_STEP) as i8 > 99 {
            riser_step(cpu, obj, 100);
        }
    }
    match cpu.read8(obj + RISER_STEP) as i8 {
        0 => riser_step(cpu, obj, 1),
        1 => {
            if cpu.read8(obj + INNER) == 0 {
                cpu.write8(obj + INNER, 1);
            }
            if cpu.read8(part + RISER_STEP) as i8 > 14 {
                cpu.write16(obj + SQUARE_WORD, 0);
                riser_step(cpu, obj, 15);
            }
        }
        16 | 17 => riser_step(cpu, obj, 15),
        100 => {
            let beside = cpu.read32(part + 0xc0);
            if cpu.read8(beside + RISER_STEP) as i8 > 99 {
                riser_step(cpu, parent, 100);
                riser_step(cpu, obj, 101);
            }
        }
        101 if cpu.read32(parent + SCENERY_Y) as i32 > SUNK_Y => {
            let slot = cpu.read8(obj + SLOT) as i8 as i32 as u32;
            cpu.write32(parent.wrapping_add(4 * slot).wrapping_add(RISER_SLOTS), 0);
            cpu.write8(obj + OBJ_MODE, TO_STOP);
        }
        _ => {}
    }
    let step = cpu.read8(obj + RISER_STEP) as i8;
    if (step == 1 && cpu.read8(obj + INNER) == 0) || step > 99 {
        return;
    }
    let arc = cpu.stack();
    cpu.write32(arc + ARC_SEGMENTS, SQUARE_SEGMENTS);
    cpu.write16(arc + ARC_ZIG, SQUARE_ZIG);
    cpu.write32(arc + ARC_OWNER, obj);
    let edge = (cpu.read32(GAME_FRAMES) & 3) as usize;
    let ((x0, y0), (x1, y1)) = SQUARE_EDGES[edge];
    for (at, x, y) in [(arc, x0, y0), (arc + ARC_TO, x1, y1)] {
        cpu.write32(at, x);
        cpu.write32(at + 4, y);
        cpu.write32(at + 8, SQUARE_Z);
        nested!(cpu, 0x0605_2c44, models::model_point(cpu, obj, at));
    }
    nested!(cpu, 0x0608_3954, arc_draw(cpu, arc));
}

/// Its debris made (to burst), and its end.
fn bursting_debris(cpu: &mut Cpu, obj: u32) {
    let d = nested!(cpu, 0x0608_08b0, debris_set(cpu, obj));
    if d != 0 {
        cpu.write8(d + BURSTS, 1);
    }
    cpu.write8(obj + OBJ_MODE, TO_STOP);
}

// ---- the aiming gun -------------------------------------------------------------------------------

/// A gun on a riser's part: its life (+0x50), its turn (z +0x30), a count (+0x74), its bursts'
/// frame count (+0x70, a word) and how many are left (+0x72, a word).
const GUN_LIFE: u32 = 20000;
const GUN_COUNT: u32 = 0x74;
const BURST_WAIT: u32 = 0x70;
const BURSTS_LEFT: u32 = 0x72;
const GUN_SPREAD: [u16; 3] = [0x8000, 0x7000, 0x9000];

/// Its behaviour. 0: life 20000; 1: once the riser is up (15 on), unshielded, 15: on to 16: it
/// aims at the nearer player (a 16th of the way a frame) for 120 frames less up to 60 by
/// difficulty; 17: every 15th frame a burst of three aimed shots (straight back and 0x1000 either
/// side, from 16 dots down its model), 3 bursts and up to 2 more by difficulty, then back to 15.
/// 100: shot down (5 more to the stage's rate, and the score) or its part sunk, it smokes 90-139
/// frames, then its part sinks; 102: debris that bursts.
pub fn aiming_gun_behaviour(cpu: &mut Cpu, obj: u32) {
    let part = nested!(cpu, 0x0605_2970, models::follow_parent(cpu, obj));
    shot_or_sunk(cpu, obj, part);
    match cpu.read8(obj + RISER_STEP) as i8 {
        0 => {
            cpu.write32(obj + TURRET_LIFE, GUN_LIFE);
            riser_step(cpu, obj, 1);
        }
        1 => {
            let parent = cpu.read32(obj + PARENT);
            let riser = cpu.read32(parent + PARENT);
            if cpu.read8(riser + RISER_STEP) as i8 >= 15 {
                let flags = cpu.read32(obj + FLAGS) & !UNHURT;
                cpu.write32(obj + FLAGS, flags);
                riser_step(cpu, obj, 15);
            }
        }
        15 => riser_step(cpu, obj, 16),
        16 => {
            if cpu.read8(obj + INNER) == 0 {
                let less = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 60));
                cpu.write32(obj + GUN_COUNT, 120u32.wrapping_sub(less));
                cpu.write8(obj + INNER, 1);
            }
            let player = nested!(cpu, 0x0604_5f70, aim::nearer_player_to(cpu, obj));
            let from = nested!(cpu, 0x0604_5e30, aim::angle_to(cpu, obj, player)) as i16 as i32;
            let turn = cpu.read16(obj + TURN_Z) as i16 as i32 as u32;
            let d = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, turn, from.wrapping_add(0x8000) as u32)) as i32 / 16;
            let t = cpu.read16(obj + TURN_Z).wrapping_add(d as u16);
            cpu.write16(obj + TURN_Z, t);
            let count = cpu.read32(obj + GUN_COUNT).wrapping_sub(1);
            cpu.write32(obj + GUN_COUNT, count);
            if count as i32 > 0 {
                return;
            }
            cpu.write16(obj + BURST_WAIT, 0);
            let more = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 2));
            cpu.write16(obj + BURSTS_LEFT, more.wrapping_add(3) as u16);
            riser_step(cpu, obj, 17);
        }
        17 => {
            let wait = cpu.read16(obj + BURST_WAIT).wrapping_add(1);
            cpu.write16(obj + BURST_WAIT, wait);
            if wait as i16 <= 14 {
                return;
            }
            cpu.write16(obj + BURST_WAIT, 0);
            let left = cpu.read16(obj + BURSTS_LEFT).wrapping_sub(1);
            cpu.write16(obj + BURSTS_LEFT, left);
            if (left as i16) < 0 {
                riser_step(cpu, obj, 15);
                return;
            }
            let shot = cpu.stack();
            cpu.write32(shot + AS_POSITION, 0);
            cpu.write32(shot + AS_POSITION + 4, 0x10_0000);
            cpu.write32(shot + AS_POSITION + 8, 0);
            nested!(cpu, 0x0605_2c44, models::model_point(cpu, obj, shot));
            cpu.write32(shot + AS_FLAGS, 0);
            cpu.write32(shot + AS_SPEED, 0x1_5800);
            cpu.write32(shot + AS_SHOOTER, obj);
            for out in GUN_SPREAD {
                let angle = cpu.read16(obj + TURN_Z).wrapping_add(out);
                cpu.write16(shot + AS_ANGLE, angle);
                nested!(cpu, 0x0604_dbc4, bullets::aimed_set(cpu, shot));
            }
            nested!(cpu, 0x0600_50d4, sound::se_set(cpu, SHOT_SOUND));
        }
        100 => {
            let puff = Puff { at: cpu.stack() + 0x40, spread: (0x10_0000, 0x10_0000), lift: 0, size: 0x8000 };
            shot_down(cpu, obj, SMOKE_COUNT, 5, puff);
        }
        102 => bursting_debris(cpu, obj),
        _ => {}
    }
}

// ---- the burst seed -------------------------------------------------------------------------------

/// A seed (an actor) that flies to its target (+0xb0) and becomes a burst there (bullets.rs's):
/// its sprite (kind 0, frame 0, 16 colours past the kind's) scaled (+0x20, y kept the same), its
/// step's speed (x +0x5c, y +0x60), the size it grows to (+0x70), its shrink's speed (+0x74), its
/// burst (+0x78). Which objects it may hit (+0x48, two longs): none, or all once it lands.
const SEED_TARGET: u32 = 0xb0;
const SEED_SPEED: u32 = 0x5c;
const SEED_SIZE: u32 = 0x70;
const SEED_SHRINK: u32 = 0x74;
const SEED_BURST: u32 = 0x78;
const HIT_SET: u32 = 0x48;
const NO_DRAW: u32 = 0x8000_0000;
const SEED_LANDS: i32 = 0x4_0000;
const SEED_SOUND: u32 = 17;
/// The burst's description (bullets.rs's burst_set): its place, number, growth and acceleration
/// (words), speed (x, y), the acceleration's change, its kind (words).
const BD_NUMBER: u32 = 0x1e;
const BD_GROWTH: u32 = 0x2c;
const BD_ACCEL: u32 = 0x2e;
const BD_SPEED: u32 = 0x30;
const BD_ACCEL_CHANGE: u32 = 0x38;
const BD_KIND: u32 = 0x3a;

/// Its behaviour. It ends with its target (before step 15). 0: set up, a small dot (scale 1/8,
/// no hits). 1: a 16th of the way to its target a frame; within 4 dots, on it, hitting all, its
/// size 1-1.5 by difficulty; then its burst (growth 0x500, acceleration 0x60 changing by -6, still;
/// sound 17), it hidden, 16; no burst, it ends (101). 15: it grows (an eighth of the way a frame,
/// past its size by 2 256ths) to its size, then (its flag 0x100 set) hitting none, 100. 16: it
/// ends with its burst. 100: it shrinks away (ever faster, to 1/32 a frame).
pub fn burst_seed_behaviour(cpu: &mut Cpu, obj: u32) {
    let target = cpu.read32(obj + SEED_TARGET);
    if (cpu.read8(obj + RISER_STEP) as i8) < 15 && cpu.read8(target + OBJ_MODE) as i8 > ENDING as i8 {
        cpu.write8(obj + OBJ_MODE, TO_STOP);
        return;
    }
    match cpu.read8(obj + RISER_STEP) as i8 {
        0 => {
            let flags = cpu.read32(obj + FLAGS) | NO_HITS;
            cpu.write32(obj + FLAGS, flags);
            cpu.write8(obj + SPRITE_KIND, 0);
            cpu.write8(obj + SPRITE_FRAME, 0);
            let kind = cpu.read8(obj + SPRITE_KIND) as i8 as i32 as u32;
            let colour = cpu.read16(CHAR_BASE.wrapping_add(16 * kind)).wrapping_add(16);
            cpu.write16(obj + OWN_COLOUR, colour);
            cpu.write16(obj + COLOUR, colour);
            cpu.write32(obj + SCALES, 0x2000);
            cpu.write32(obj + HIT_SET, 0);
            cpu.write32(obj + HIT_SET + 4, 0);
            riser_step(cpu, obj, 1);
        }
        1 => {
            for k in [0, 4] {
                let d = cpu.read32(target + POSITION + k).wrapping_sub(cpu.read32(obj + POSITION + k)) as i32 / 16;
                cpu.write32(obj + SEED_SPEED + k, d as u32);
            }
            for k in [0, 4] {
                let p = cpu.read32(obj + POSITION + k).wrapping_add(cpu.read32(obj + SEED_SPEED + k));
                cpu.write32(obj + POSITION + k, p);
            }
            let far = nested!(cpu, 0x0604_5ee4, aim::distance(cpu, obj, target));
            if far as i32 >= SEED_LANDS {
                return seed_end(cpu, obj);
            }
            for k in [0, 4] {
                let p = cpu.read32(target + POSITION + k);
                cpu.write32(obj + POSITION + k, p);
            }
            cpu.write32(obj + SEED_SPEED + 4, 0);
            cpu.write32(obj + SEED_SPEED, 0);
            cpu.write32(obj + HIT_SET, 0xffff_ffff);
            cpu.write32(obj + HIT_SET + 4, 0xffff_ffff);
            let more = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 0x8000));
            cpu.write32(obj + SEED_SIZE, more.wrapping_add(0x1_0000));
            riser_step(cpu, obj, 15);
            let d = cpu.stack();
            for k in [0, 4, 8] {
                let v = cpu.read32(obj + POSITION + k);
                cpu.write32(d + k, v);
            }
            let number = cpu.read8(obj + NUMBER);
            cpu.write8(d + BD_NUMBER, number);
            cpu.write16(d + BD_GROWTH, 0x500);
            cpu.write16(d + BD_ACCEL, 0x60);
            cpu.write16(d + BD_ACCEL_CHANGE, 0xfffa);
            cpu.write32(d + BD_SPEED, 0);
            cpu.write32(d + BD_SPEED + 4, 0);
            cpu.write16(d + BD_KIND, 0);
            let burst = nested!(cpu, 0x0605_5310, bullets::burst_set(cpu, d));
            cpu.write32(obj + SEED_BURST, burst);
            if burst != 0 {
                nested!(cpu, 0x0600_50d4, sound::se_set(cpu, SEED_SOUND));
                let flags = cpu.read32(obj + FLAGS) | NO_DRAW;
                cpu.write32(obj + FLAGS, flags);
                riser_step(cpu, obj, 16);
            } else {
                cpu.write8(obj + OBJ_MODE, TO_STOP);
                riser_step(cpu, obj, 101);
            }
        }
        15 => {
            cpu.write32(obj + HIT_SET, 0xffff_ffff);
            cpu.write32(obj + HIT_SET + 4, 0xffff_ffff);
            let s = cpu.read32(obj + SCALES);
            let size = cpu.read32(obj + SEED_SIZE);
            let s = s.wrapping_add((size.wrapping_sub(s.wrapping_sub(0x200)) as i32 / 8) as u32);
            cpu.write32(obj + SCALES, s);
            if s as i32 >= cpu.read32(obj + SEED_SIZE) as i32 {
                let flags = cpu.read32(obj + FLAGS) | 0x100;
                cpu.write32(obj + FLAGS, flags);
                cpu.write32(obj + SEED_SHRINK, 0);
                cpu.write16(obj + SLOT, 0);
                cpu.write32(obj + HIT_SET, 0);
                cpu.write32(obj + HIT_SET + 4, 0);
                riser_step(cpu, obj, 100);
            }
        }
        16 => {
            let burst = cpu.read32(obj + SEED_BURST);
            if cpu.read8(burst + OBJ_MODE) as i8 > ENDING as i8 {
                cpu.write8(obj + OBJ_MODE, TO_STOP);
                riser_step(cpu, obj, 101);
            }
        }
        100 => {
            let v = cpu.read32(obj + SEED_SHRINK);
            let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0x800, 0x40));
            cpu.write32(obj + SEED_SHRINK, v);
            let s = cpu.read32(obj + SCALES);
            let s = nested!(cpu, 0x0604_6168, actors::approach(cpu, s, 0, v));
            cpu.write32(obj + SCALES, s);
            if s as i32 <= 0 {
                cpu.write8(obj + OBJ_MODE, TO_STOP);
            }
        }
        _ => {}
    }
    seed_end(cpu, obj);
}

/// Its y scale kept as its x.
fn seed_end(cpu: &mut Cpu, obj: u32) {
    let s = cpu.read32(obj + SCALES);
    cpu.write32(obj + SCALES + 4, s);
}

// ---- the tether -----------------------------------------------------------------------------------

/// A tether (a plain object): a line (sprite flags 3: two corners, its own at +0x70, in dots) from
/// one object (+0x5c, on which it is placed) to another (+0x60, to which it is turned), its far
/// corner its length (+0x64, 16.16) less 24 dots out (at least 8); its colour (+0x28) swinging up
/// to 4 either side of its own (+0x2a) by a frame count (+0x40, a word).
const TETHER_FROM: u32 = 0x5c;
const TETHER_TO: u32 = 0x60;
const TETHER_LENGTH: u32 = 0x64;
const TETHER_FRAME: u32 = 0x40;
const TETHER_CORNERS: u32 = 0x70;
const TWO_CORNERS: u32 = 3;
const TETHER_DIM: u16 = 42;
const TETHER_LIT: u16 = 24;

/// Its behaviour. It ends with the far object, or (before step 100) the near. 0: set up (dim);
/// 1: once the near object's step is past 14, lit (15); 15: once it is past 99, dim, and it shrinks
/// (100: its length an eighth less a frame) to a dot, and ends.
pub fn tether_behaviour(cpu: &mut Cpu, obj: u32) {
    let (from, to) = (cpu.read32(obj + TETHER_FROM), cpu.read32(obj + TETHER_TO));
    if cpu.read8(to + OBJ_MODE) as i8 > ENDING as i8
        || (cpu.read8(obj + STEP) as i8 <= 99 && cpu.read8(from + OBJ_MODE) as i8 > ENDING as i8)
    {
        cpu.write8(obj + OBJ_MODE, TO_STOP);
        return;
    }
    for k in [0, 4, 8] {
        let v = cpu.read32(from + POSITION + k);
        cpu.write32(obj + POSITION + k, v);
    }
    let corners = obj + TETHER_CORNERS;
    let mut step = cpu.read8(obj + STEP) as i8;
    if step == 0 {
        cpu.write32(obj + CORNERS, corners);
        let flags = cpu.read32(obj + FLAGS) | TWO_CORNERS;
        cpu.write32(obj + FLAGS, flags);
        for (at, v) in [(4, 0), (0, 0), (6, 0xfff8), (2, 0xfff8)] {
            cpu.write16(corners + at, v);
        }
        cpu.write16(obj + TETHER_FRAME, 0);
        let colour = cpu.read16(CHAR_BASE).wrapping_add(TETHER_DIM);
        cpu.write16(obj + OWN_COLOUR, colour);
        step = 1;
        cpu.write8(obj + STEP, 1);
    }
    match step {
        1 | 15 => {
            let length = nested!(cpu, 0x0604_5ee4, aim::distance(cpu, from, to));
            cpu.write32(obj + TETHER_LENGTH, length);
            let turn = nested!(cpu, 0x0604_5e30, aim::angle_to(cpu, from, to));
            cpu.write16(obj + TURN_Z, turn as u16);
            let near = cpu.read8(from + STEP) as i8;
            if step == 1 && near > 14 {
                let colour = cpu.read16(CHAR_BASE).wrapping_add(TETHER_LIT);
                cpu.write16(obj + OWN_COLOUR, colour);
                cpu.write8(obj + STEP, 15);
            } else if step == 15 && near > 99 {
                let colour = cpu.read16(CHAR_BASE).wrapping_add(TETHER_DIM);
                cpu.write16(obj + OWN_COLOUR, colour);
                cpu.write16(obj + TETHER_FRAME, 0);
                cpu.write8(obj + STEP, 100);
            }
        }
        100 => {
            cpu.write16(obj + TETHER_FRAME, 0);
            let length = cpu.read32(obj + TETHER_LENGTH);
            let length = length.wrapping_add((length.wrapping_neg() as i32 / 8) as u32);
            cpu.write32(obj + TETHER_LENGTH, length);
            if length as i32 <= 0xffff {
                cpu.write8(obj + OBJ_MODE, TO_STOP);
            }
        }
        _ => {}
    }
    let frame = cpu.read16(obj + TETHER_FRAME);
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, (frame << 11) as i16 as i32 as u32));
    let swing = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, 4, c));
    let colour = (swing as u16).wrapping_add(cpu.read16(obj + OWN_COLOUR));
    cpu.write16(obj + COLOUR, colour);
    let frame = cpu.read16(obj + TETHER_FRAME).wrapping_add(1);
    cpu.write16(obj + TETHER_FRAME, frame);
    let y = 24u16.wrapping_sub(cpu.read16(obj + TETHER_LENGTH));
    cpu.write16(corners + 6, y);
    if y as i16 > -8 {
        cpu.write16(corners + 6, 0xfff8);
    }
}

// ---- the tethered drone ---------------------------------------------------------------------------

/// A drone (a plain object, kind 3, unturned; its sprite, at +0x10, animated by its link at +0x70)
/// tethered to its parent (+0x5c): its sub-step (+0x3d: slowing or speeding), a count (+0x40, a
/// word), the place it dashes to (+0x44, +0x48), its speed (+0x60, along its turn), its seed
/// (+0x64).
const DRONE_PARENT: u32 = 0x5c;
const DRONE_SUB: u32 = 0x3d;
const DRONE_COUNT: u32 = 0x40;
const DRONE_TO: u32 = 0x44;
const DRONE_SPEED: u32 = 0x60;
const DRONE_SEED: u32 = 0x64;
const DRONE_LINK: u32 = 0x70;
const DRONE_KIND: u8 = 3;
const UNTURNED: u32 = 0x1000_0000;
const DRONE_FLYING: u32 = 0x0609_fc24;
const DRONE_DASHED: u32 = 0x0609_fc30;
const TETHER_BEHAVIOUR: u32 = 0x0608_55e8;
const SEED_BEHAVIOUR: u32 = 0x0608_5344;
const DRONE_NEAR: i32 = 0xf_ffff;

/// Its behaviour. It ends with its parent (before step 100). 0: its tether made (to the parent),
/// set up, flung out at 4 dots a frame; 1: slowing (a 16th a frame), 90 frames; then, turned to the
/// nearer player, 2: it chases it (slowing to half a dot a frame and holding there 90 frames less
/// up to 30 by difficulty, then speeding to 2.5 and holding up to 30 by difficulty, by turns),
/// turning toward it at most 0x8000 less its speed's 85th; within 16 dots, 15: it dashes (a
/// quarter of the way a frame, 16 frames) to where the player was. 16: a seed made from 24 dots
/// down its parent's model, flying to it; 17: once the seed is past its step 14 (or gone), it
/// fades (100: its scale a 16th less a frame) away.
pub fn tethered_drone_behaviour(cpu: &mut Cpu, obj: u32) {
    let parent = cpu.read32(obj + DRONE_PARENT);
    if (cpu.read8(obj + STEP) as i8) <= 99 && cpu.read8(parent + RISER_STEP) as i8 > 99 {
        cpu.write8(obj + OBJ_MODE, TO_STOP);
        return;
    }
    let step = cpu.read8(obj + STEP) as i8;
    match step {
        0 => {
            let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0x80)) as i8 as i32 as u32;
            let tether = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, PLAIN_EXEC));
            if tether != 0 {
                nested!(cpu, 0x0604_dd50, effects::actor_init(cpu, tether));
                let flags = cpu.read32(tether + FLAGS) | SPRITE_FLAG_400;
                cpu.write32(tether + FLAGS, flags);
                cpu.write32(tether + BEHAVIOUR, TETHER_BEHAVIOUR);
                cpu.write32(tether + TETHER_FROM, obj);
                cpu.write32(tether + TETHER_TO, parent);
                let flags = cpu.read32(obj + FLAGS) | UNTURNED;
                cpu.write32(obj + FLAGS, flags);
                cpu.write8(obj + SPRITE_KIND, DRONE_KIND);
                drone_colour(cpu, obj, 0xe0);
                nested!(cpu, 0x0604_6530, effects::link_init(cpu, obj + DRONE_LINK, DRONE_FLYING));
                cpu.write32(obj + DRONE_SPEED, 0x4_0000);
                cpu.write16(obj + DRONE_COUNT, 90);
                cpu.write8(obj + STEP, 1);
            }
        }
        1 => {
            let v = cpu.read32(obj + DRONE_SPEED);
            cpu.write32(obj + DRONE_SPEED, v.wrapping_add((v.wrapping_neg() as i32 / 16) as u32));
            let count = cpu.read16(obj + DRONE_COUNT).wrapping_sub(1);
            cpu.write16(obj + DRONE_COUNT, count);
            if count as i16 <= 0 {
                let player = nested!(cpu, 0x0604_5f70, aim::nearer_player_to(cpu, obj));
                let turn = nested!(cpu, 0x0604_5e30, aim::angle_to(cpu, obj, player));
                cpu.write16(obj + TURN_Z, turn as u16);
                cpu.write8(obj + DRONE_SUB, 0);
                cpu.write16(obj + DRONE_COUNT, 0);
                drone_colour(cpu, obj, 0x20);
                cpu.write8(obj + STEP, 2);
            }
        }
        2 => drone_chase(cpu, obj),
        15 => {
            let (dx, dy) = drone_apart(cpu, obj);
            let line = nested!(cpu, 0x0606_780c, sgl::sl_atan(cpu, dx, dy)) as u16;
            cpu.write16(obj + TURN_Z, line.wrapping_add(0x4000));
            let (dx, dy) = drone_apart(cpu, obj);
            let far = nested!(cpu, 0x0604_5e60, aim::length(cpu, dx, dy));
            cpu.write32(obj + DRONE_SPEED, (far as i32 / 4) as u32);
            let count = cpu.read16(obj + DRONE_COUNT).wrapping_add(1);
            cpu.write16(obj + DRONE_COUNT, count);
            if count as i16 > 15 {
                nested!(cpu, 0x0604_6530, effects::link_init(cpu, obj + DRONE_LINK, DRONE_DASHED));
                cpu.write32(obj + DRONE_SPEED, 0);
                for k in [0, 4] {
                    let v = cpu.read32(obj + DRONE_TO + k);
                    cpu.write32(obj + POSITION + k, v);
                }
                cpu.write8(obj + STEP, 16);
            }
        }
        16 => {
            let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0xb4)) as i8 as i32 as u32;
            let seed = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
            if seed != 0 {
                nested!(cpu, 0x0604_62ac, actors::actor_setup(cpu, seed));
                cpu.write32(seed + POSITION, 0);
                cpu.write32(seed + POSITION + 8, 0);
                let flags = cpu.read32(seed + FLAGS) | SPRITE_FLAG_400;
                cpu.write32(seed + FLAGS, flags);
                cpu.write32(seed + BEHAVIOUR, SEED_BEHAVIOUR);
                cpu.write32(seed + POSITION + 4, 0x18_0000);
                nested!(cpu, 0x0605_2c44, models::model_point(cpu, parent, seed + POSITION));
                cpu.write32(seed + POSITION + 8, 0);
                cpu.write8(seed + BYTE_33, 8);
                let n = cpu.read8(parent + NUMBER);
                cpu.write8(seed + NUMBER, n);
                let b = cpu.read8(parent + BYTE_54);
                cpu.write8(seed + BYTE_54, b);
                cpu.write32(seed + SEED_TARGET, obj);
                cpu.write32(obj + DRONE_SEED, seed);
                cpu.write16(obj + DRONE_COUNT, 0);
                cpu.write8(obj + STEP, 17);
            }
        }
        17 => {
            let seed = cpu.read32(obj + DRONE_SEED);
            if cpu.read8(seed + OBJ_MODE) as i8 > ENDING as i8 || cpu.read8(seed + RISER_STEP) as i8 > 14 {
                cpu.write8(obj + STEP, 100);
            }
        }
        100 => {
            let s = cpu.read32(obj + SCALES);
            let s = s.wrapping_add((s.wrapping_neg() as i32 / 16) as u32);
            cpu.write32(obj + SCALES, s);
            cpu.write32(obj + SCALES + 4, s);
            if cpu.read32(obj + SCALES) as i32 <= 0x7ff {
                cpu.write8(obj + OBJ_MODE, TO_STOP);
            }
        }
        _ => {}
    }
    let t = cpu.stack();
    cpu.write32(t, 0);
    let v = cpu.read32(obj + DRONE_SPEED).wrapping_neg();
    cpu.write32(t + 4, v);
    cpu.write32(t + T_SCALES + 4, 0x1_0000);
    cpu.write32(t + T_SCALES, 0x1_0000);
    cpu.write32(t + T_FLAGS, 0);
    let turn = cpu.read16(obj + TURN_Z);
    cpu.write16(t + T_ANGLE, turn);
    nested!(cpu, 0x0606_4eac, math::turn_point(cpu, t));
    for k in [0, 4] {
        let p = cpu.read32(obj + POSITION + k).wrapping_add(cpu.read32(t + k));
        cpu.write32(obj + POSITION + k, p);
    }
    nested!(cpu, 0x0604_6558, effects::link_step(cpu, obj + FLAGS, obj + DRONE_LINK));
}

/// Its colour (own and shown): `past` past the first character's.
fn drone_colour(cpu: &mut Cpu, obj: u32, past: u16) {
    let colour = cpu.read16(CHAR_BASE).wrapping_add(past);
    cpu.write16(obj + OWN_COLOUR, colour);
    cpu.write16(obj + COLOUR, colour);
}

/// From it to the place it dashes to.
fn drone_apart(cpu: &mut Cpu, obj: u32) -> (u32, u32) {
    let dx = cpu.read32(obj + DRONE_TO).wrapping_sub(cpu.read32(obj + POSITION));
    let dy = cpu.read32(obj + DRONE_TO + 4).wrapping_sub(cpu.read32(obj + POSITION + 4));
    (dx, dy)
}

/// Step 2: the chase.
fn drone_chase(cpu: &mut Cpu, obj: u32) {
    let player = nested!(cpu, 0x0604_5f70, aim::nearer_player_to(cpu, obj));
    let fast = cpu.read8(obj + DRONE_SUB) != 0;
    let to = if fast { 0x2_8000 } else { 0x8000 };
    let v = cpu.read32(obj + DRONE_SPEED);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, to, 0x800));
    cpu.write32(obj + DRONE_SPEED, v);
    if v == to {
        let count = cpu.read16(obj + DRONE_COUNT).wrapping_add(1);
        cpu.write16(obj + DRONE_COUNT, count);
        let held = cpu.read16(obj + DRONE_COUNT) as i16 as i32;
        let less = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 30)) as i32;
        let hold = if fast { less } else { 90 - less };
        if held >= hold {
            cpu.write16(obj + DRONE_COUNT, 0);
            cpu.write8(obj + DRONE_SUB, !fast as u8);
        }
    }
    let most = 0x8000u32.wrapping_sub((cpu.read32(obj + DRONE_SPEED) as i32 / 85) as u32);
    let to = nested!(cpu, 0x0604_5e30, aim::angle_to(cpu, obj, player)) as i16 as i32 as u32;
    let turn = cpu.read16(obj + TURN_Z) as i16 as i32 as u32;
    let d = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, turn, to));
    let step = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, d, most));
    let t = cpu.read16(obj + TURN_Z).wrapping_add(step as u16);
    cpu.write16(obj + TURN_Z, t);
    let far = nested!(cpu, 0x0604_5ee4, aim::distance(cpu, obj, player));
    if far as i32 > DRONE_NEAR {
        return;
    }
    for k in [0, 4] {
        let v = cpu.read32(player + POSITION + k);
        cpu.write32(obj + DRONE_TO + k, v);
    }
    cpu.write16(obj + DRONE_COUNT, 0);
    drone_colour(cpu, obj, 16);
    cpu.write8(obj + STEP, 15);
}

// ---- the drone launcher ---------------------------------------------------------------------------

/// A drone launcher on a riser's part: its life (+0x50), its turns (x +0x2c, z +0x30), its drone
/// (+0x70), a count (+0x74), its kick (+0x78, a word: added to its x turn, easing off by 0x100 a
/// frame).
const LAUNCHER_LIFE: u32 = 30000;
const LAUNCHED: u32 = 0x70;
const LAUNCHER_COUNT: u32 = 0x74;
const KICK: u32 = 0x78;
const DRONE_BEHAVIOUR: u32 = 0x0608_57b4;

/// Its behaviour. 0: life 30000, tilted (x 0xe000); 1-2: at the riser's steps 12 and 15; 3: it
/// turns up (at most 0x80 a frame); 4: unshielded, 15 after 60 frames. 15: straight ahead and with
/// its count run out, a drone launched (from 24 dots down its model, turned back from it) with a
/// kick, and another as the drone dashes (its step 16); it turns to face its drone while it lives,
/// then back straight (0x40 a frame) and rests 150 frames. 100: shot down (5 more to the stage's
/// rate, and the score) or its part sunk, it smokes 90-139 frames, then its part sinks; 102:
/// debris that bursts.
pub fn drone_launcher_behaviour(cpu: &mut Cpu, obj: u32) {
    let part = nested!(cpu, 0x0605_2970, models::follow_parent(cpu, obj));
    let parent = cpu.read32(obj + PARENT);
    let riser = cpu.read32(parent + PARENT);
    shot_or_sunk(cpu, obj, part);
    match cpu.read8(obj + RISER_STEP) as i8 {
        0 => {
            cpu.write32(obj + TURRET_LIFE, LAUNCHER_LIFE);
            cpu.write16(obj + TURN_X, 0xe000);
            cpu.write32(obj + LAUNCHED, 0);
            cpu.write16(obj + KICK, 0);
            riser_step(cpu, obj, 1);
        }
        1 => {
            if cpu.read8(riser + RISER_STEP) == 12 {
                riser_step(cpu, obj, 2);
            }
        }
        2 => {
            if cpu.read8(riser + RISER_STEP) == 15 {
                cpu.write32(obj + LAUNCHER_COUNT, 0);
                riser_step(cpu, obj, 3);
            }
        }
        3 => {
            if turn_toward(cpu, obj, TURN_X, 0, 0x80) == 0 {
                riser_step(cpu, obj, 4);
            }
        }
        4 => {
            let flags = cpu.read32(obj + FLAGS) & !UNHURT;
            cpu.write32(obj + FLAGS, flags);
            cpu.write32(obj + LAUNCHER_COUNT, 60);
            riser_step(cpu, obj, 15);
        }
        15 => launching(cpu, obj),
        100 => {
            let puff = Puff { at: cpu.stack(), spread: (0x18_0000, 0x18_0000), lift: 0x18_0000, size: 0xa000 };
            shot_down(cpu, obj, LAUNCHER_COUNT, 5, puff);
        }
        102 => bursting_debris(cpu, obj),
        _ => {}
    }
}

/// Step 15's inner steps (0: waiting; 1: launching; 2: its drone out; 3: its drone dashed), and
/// its turns.
fn launching(cpu: &mut Cpu, obj: u32) {
    let mut drone = cpu.read32(obj + LAUNCHED);
    match cpu.read8(obj + INNER) {
        0 => {
            if cpu.read16(obj + TURN_Z) == 0 {
                let count = cpu.read32(obj + LAUNCHER_COUNT);
                if count != 0 {
                    cpu.write32(obj + LAUNCHER_COUNT, count.wrapping_sub(1));
                } else {
                    cpu.write8(obj + INNER, 1);
                }
            }
        }
        1 => {
            let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0x80)) as i8 as i32 as u32;
            drone = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, PLAIN_EXEC));
            if drone != 0 {
                nested!(cpu, 0x0604_dd50, effects::actor_init(cpu, drone));
                cpu.write32(drone + POSITION, 0);
                cpu.write32(drone + POSITION + 8, 0);
                let flags = cpu.read32(drone + FLAGS) | SPRITE_FLAG_400;
                cpu.write32(drone + FLAGS, flags);
                cpu.write32(drone + BEHAVIOUR, DRONE_BEHAVIOUR);
                cpu.write32(drone + POSITION + 4, 0x18_0000);
                nested!(cpu, 0x0605_2c44, models::model_point(cpu, obj, drone + POSITION));
                cpu.write32(drone + POSITION + 8, 0);
                let n = cpu.read8(obj + NUMBER);
                cpu.write8(drone + NUMBER, n);
                cpu.write8(drone + BYTE_33, 9);
                let turn = cpu.read16(obj + TURN_Z).wrapping_add(0x8000);
                cpu.write16(drone + TURN_Z, turn);
                cpu.write32(drone + DRONE_PARENT, obj);
                cpu.write16(obj + KICK, 0xfc00);
                cpu.write32(obj + LAUNCHED, drone);
                cpu.write8(obj + INNER, 2);
            }
        }
        inner @ (2 | 3) => {
            if inner == 2 && cpu.read8(drone + STEP) == 16 {
                cpu.write16(obj + KICK, 0xf800);
                cpu.write8(obj + INNER, 3);
            }
            if drone == 0 {
                cpu.write32(obj + LAUNCHER_COUNT, 150);
                cpu.write8(obj + INNER, 0);
            } else if cpu.read8(drone + OBJ_MODE) as i8 > ENDING as i8 || cpu.read8(drone + STEP) as i8 > 99 {
                cpu.write32(obj + LAUNCHED, 0);
                drone = 0;
            }
        }
        _ => {}
    }
    let kick = cpu.read16(obj + KICK);
    if kick != 0 {
        let t = cpu.read16(obj + TURN_X).wrapping_add(kick);
        cpu.write16(obj + TURN_X, t);
        let kick = cpu.read16(obj + KICK) as i16 as i32 as u32;
        let kick = nested!(cpu, 0x0604_6168, actors::approach(cpu, kick, 0, 0x100));
        cpu.write16(obj + KICK, kick as u16);
    } else {
        turn_toward(cpu, obj, TURN_X, 0, 0x80);
    }
    if drone != 0 {
        let turn = nested!(cpu, 0x0604_5e30, aim::angle_to(cpu, drone, obj));
        cpu.write16(obj + TURN_Z, turn as u16);
    } else {
        turn_toward(cpu, obj, TURN_Z, 0, 0x40);
    }
}

// ---- the ring part --------------------------------------------------------------------------------

/// A part placed round its parent (+0xd4) in the scenery: out by the parent's +0x70 (scaled by its
/// x and z scales, +0x20 and +0x28), round by its y turn (+0x2e); flashing with the parent, ending
/// with it, debris that bursts once the parent sinks (its step past 99).
const RING_RADIUS: u32 = 0x70;
const SCALE_Z: u32 = 0x28;

pub fn ring_part_behaviour(cpu: &mut Cpu, obj: u32) {
    let t = cpu.stack();
    cpu.write32(t, 0);
    let parent = cpu.read32(obj + PARENT);
    let r = cpu.read32(parent + RING_RADIUS);
    cpu.write32(t + 4, r);
    let sx = cpu.read32(obj + SCALES);
    cpu.write32(t + T_SCALES, sx);
    let sz = cpu.read32(obj + SCALE_Z);
    cpu.write32(t + T_SCALES + 4, sz);
    cpu.write32(t + T_FLAGS, 0);
    let turn = cpu.read16(obj + TURN_Y);
    cpu.write16(t + T_ANGLE, turn);
    nested!(cpu, 0x0606_4eac, math::turn_point(cpu, t));
    let x = cpu.read32(t).wrapping_neg();
    cpu.write32(obj + PLACE_IN_SCENERY, x);
    let z = cpu.read32(t + 4);
    cpu.write32(obj + SCENERY_Z, z);
    let parent = cpu.read32(obj + PARENT);
    nested!(cpu, 0x0604_666c, parts::flash_with(cpu, obj, parent));
    let parent = cpu.read32(obj + PARENT);
    if cpu.read8(parent + OBJ_MODE) as i8 > ENDING as i8 {
        cpu.write8(obj + OBJ_MODE, TO_STOP);
    }
    let parent = cpu.read32(obj + PARENT);
    if cpu.read8(parent + RISER_STEP) as i8 > 99 {
        bursting_debris(cpu, obj);
    }
}

// ---- the glow -------------------------------------------------------------------------------------

/// A glow (an effect, relative to its parent +0x5c; effects.rs's): it ends with the parent; coloured
/// as the parent's own colour; at frame 0xb5, once down to its last frames (its frames, +0x40, within
/// those its scales shrink over, +0x3e), back to frame 0 at 3/8 scale, not shrinking.
const GLOW_PARENT: u32 = 0x5c;
const FX_SHRINK_FRAMES: u32 = 0x3e;
const FX_FRAMES_LEFT: u32 = 0x40;
const GLOW_LAST_FRAME: u8 = 0xb5;

pub fn glow_behaviour(cpu: &mut Cpu, obj: u32) {
    let parent = cpu.read32(obj + GLOW_PARENT);
    if cpu.read8(parent + OBJ_MODE) as i8 > ENDING as i8 {
        cpu.write8(obj + OBJ_MODE, TO_STOP);
        return;
    }
    if cpu.read8(obj + SPRITE_FRAME) == GLOW_LAST_FRAME
        && cpu.read16(obj + FX_FRAMES_LEFT) as i16 as i32 <= cpu.read8(obj + FX_SHRINK_FRAMES) as i8 as i32
    {
        cpu.write32(obj + SCALES + 4, 0x6000);
        cpu.write32(obj + SCALES, 0x6000);
        cpu.write8(obj + SPRITE_FRAME, 0);
        cpu.write8(obj + FX_SHRINK_FRAMES, 0);
    }
    let parent = cpu.read32(obj + GLOW_PARENT);
    let colour = cpu.read16(parent + OWN_COLOUR);
    cpu.write16(obj + COLOUR, colour);
    nested!(cpu, 0x0604_deb4, effects::effect_exec(cpu, obj));
}

// ---- the link beam --------------------------------------------------------------------------------

/// A beam (an actor; a quad of the laser's frame, its corners at +0x80) from itself to its parent
/// (+0xb0), with one box (at +0x90: a shot's, last; its offset y and half height following the
/// beam): the frames before its glow (+0x70; after, it moves to its end at its speed, +0x78, of
/// either sign), its length (+0x7c, 16.16), its end (+0x98, +0x9c).
const LINK_PARENT: u32 = 0xb0;
const LINK_DELAY: u32 = 0x70;
const LINK_SPEED: u32 = 0x78;
const LINK_LENGTH: u32 = 0x7c;
const LINK_CORNERS: u32 = 0x80;
const LINK_BOX: u32 = 0x90;
const LINK_END: u32 = 0x98;
const LINK_BOX_WHAT: u8 = 0xa0;
const GLOW_BEHAVIOUR: u32 = 0x0608_6198;
const GLOW_FLAGS: u32 = 0x80;
/// A box's offset (x, y: words), half width and height (bytes).
const BOX_X: u32 = 2;
const BOX_Y: u32 = 4;
const BOX_HALF_W: u32 = 6;
const BOX_HALF_H: u32 = 7;

/// Its behaviour. It ends with its parent; coloured as the parent. 0: set up (24 dots across, 3/4
/// scale); 1-2: hitting all, it reaches to the parent, taking the parent's step; 3-4: its end at the
/// parent, its near end comes on to it (the beam 9 dots short of it), and once there it ends.
pub fn link_beam_behaviour(cpu: &mut Cpu, obj: u32) {
    let parent = cpu.read32(obj + LINK_PARENT);
    if cpu.read8(parent + OBJ_MODE) as i8 > ENDING as i8 {
        cpu.write8(obj + OBJ_MODE, TO_STOP);
        return;
    }
    let delay = cpu.read32(obj + LINK_DELAY);
    if delay == 0 {
        for k in [0, 4] {
            let v = (cpu.read32(obj + LINK_SPEED) as i32).wrapping_abs() as u32;
            let (p, end) = (cpu.read32(obj + POSITION + k), cpu.read32(obj + LINK_END + k));
            let p = nested!(cpu, 0x0604_6168, actors::approach(cpu, p, end, v));
            cpu.write32(obj + POSITION + k, p);
        }
    } else {
        let delay = delay.wrapping_sub(1);
        cpu.write32(obj + LINK_DELAY, delay);
        if delay == 0 {
            link_glow(cpu, obj);
        }
    }
    let colour = cpu.read16(parent + OWN_COLOUR);
    cpu.write16(obj + OWN_COLOUR, colour);
    let (corners, b) = (obj + LINK_CORNERS, obj + LINK_BOX);
    match cpu.read8(obj + RISER_STEP) {
        0 => {
            cpu.write32(obj + CORNERS, corners);
            let flags = cpu.read32(obj + FLAGS) | QUAD;
            cpu.write32(obj + FLAGS, flags);
            cpu.write8(obj + SPRITE_KIND, 0);
            cpu.write8(obj + SPRITE_FRAME, LASER_FRAME);
            cpu.write32(obj + SCALES, 0xc000);
            for (at, v) in [(12, 0xfff4), (0, 0xfff4), (8, 12), (4, 12), (14, 0xfff7), (10, 0xfff7), (6, 0xfff7), (2, 0xfff7)] {
                cpu.write16(corners + at, v);
            }
            cpu.write8(b, LINK_BOX_WHAT);
            cpu.write8(b + 1, 0);
            cpu.write16(b + BOX_Y, 0);
            cpu.write16(b + BOX_X, 0);
            cpu.write8(b + BOX_HALF_W, 10);
            cpu.write8(b + BOX_HALF_H, 0);
            cpu.write32(obj + BEAM_LIST, b);
            cpu.write32(obj + LINK_LENGTH, 0);
            riser_step(cpu, obj, 1);
        }
        1 | 2 => {
            cpu.write32(obj + HIT_SET, 0xffff_ffff);
            cpu.write32(obj + HIT_SET + 4, 0xffff_ffff);
            let length = nested!(cpu, 0x0604_5ee4, aim::distance(cpu, obj, parent));
            cpu.write32(obj + LINK_LENGTH, length);
            for k in [0, 4] {
                let v = cpu.read32(parent + POSITION + k);
                cpu.write32(obj + LINK_END + k, v);
            }
            let far = cpu.read16(obj + LINK_LENGTH).wrapping_neg();
            cpu.write16(corners + 6, far);
            cpu.write16(corners + 2, far);
            let step = cpu.read8(parent + RISER_STEP) as i8 as i16;
            riser_step(cpu, obj, step as u16);
        }
        3 | 4 => {
            if cpu.read8(obj + INNER) == 0 {
                // Its end put 9 dots back along its turn, then on the parent after all.
                let t = cpu.stack();
                cpu.write32(t, 0);
                cpu.write32(t + 4, 0xfff7_0000);
                cpu.write32(t + T_SCALES + 4, 0x1_0000);
                cpu.write32(t + T_SCALES, 0x1_0000);
                cpu.write32(t + T_FLAGS, 0);
                let turn = cpu.read16(obj + TURN_Z);
                cpu.write16(t + T_ANGLE, turn);
                nested!(cpu, 0x0606_4eac, math::turn_point(cpu, t));
                for k in [0, 4] {
                    let v = cpu.read32(obj + LINK_END + k).wrapping_sub(cpu.read32(t + k));
                    cpu.write32(obj + LINK_END + k, v);
                }
                for k in [0, 4] {
                    let v = cpu.read32(parent + POSITION + k);
                    cpu.write32(obj + LINK_END + k, v);
                }
                cpu.write8(obj + INNER, 1);
            }
            let dx = cpu.read32(obj + LINK_END).wrapping_sub(cpu.read32(obj + POSITION));
            let dy = cpu.read32(obj + LINK_END + 4).wrapping_sub(cpu.read32(obj + POSITION + 4));
            let length = nested!(cpu, 0x0604_5e60, aim::length(cpu, dx, dy));
            cpu.write32(obj + LINK_LENGTH, length);
            let short = cpu.read16(obj + LINK_LENGTH).wrapping_sub(9);
            cpu.write16(obj + LINK_LENGTH, short);
            if cpu.read32(obj + LINK_LENGTH) as i32 > 0x8_ffff {
                cpu.write16(corners + 6, short.wrapping_neg());
                cpu.write16(corners + 2, short.wrapping_neg());
            } else {
                cpu.write32(obj + LINK_LENGTH, 0);
                if cpu.read32(obj + POSITION) == cpu.read32(obj + LINK_END)
                    && cpu.read32(obj + POSITION + 4) == cpu.read32(obj + LINK_END + 4)
                {
                    cpu.write8(obj + OBJ_MODE, TO_STOP);
                }
            }
        }
        _ => {}
    }
    let w = cpu.read16(obj + LINK_LENGTH) as i16 as i32;
    cpu.write16(b + BOX_Y, ((w + 9).wrapping_neg() / 2) as u16);
    let w = cpu.read16(obj + LINK_LENGTH) as i16 as i32;
    cpu.write8(b + BOX_HALF_H, (w / 2) as u8);
}

/// Its glow: an effect on it (glow_behaviour's), at 3/8 scale, of its colour.
fn link_glow(cpu: &mut Cpu, obj: u32) {
    let fx = cpu.stack();
    for k in [0, 4, 8] {
        cpu.write32(fx + effects::X + k, 0);
    }
    cpu.write32(fx + effects::SCALE_Y, 0x6000);
    cpu.write32(fx + effects::SCALE_X, 0x6000);
    cpu.write32(fx + effects::FLAGS_LONG, GLOW_FLAGS);
    cpu.write16(fx + effects::ANGLE, 0);
    cpu.write16(fx + effects::FRAMES, 0);
    cpu.write8(fx + effects::FRAMES_2, 0);
    cpu.write8(fx + effects::KIND, 0);
    cpu.write8(fx + effects::FRAME, 0);
    let colour = cpu.read16(obj + OWN_COLOUR);
    cpu.write16(fx + effects::COLOUR, colour);
    let number = cpu.read8(obj + NUMBER);
    cpu.write8(fx + effects::PLAYER, number);
    cpu.write32(fx + effects::RELATIVE_3C, obj);
    let glow = nested!(cpu, 0x0604_e1cc, effects::effect_set(cpu, fx));
    if glow != 0 {
        cpu.write32(glow + BEHAVIOUR, GLOW_BEHAVIOUR);
        let b = cpu.read8(obj + BYTE_33);
        cpu.write8(glow + BYTE_33, b);
    }
}

/// A link beam made (list 2, 0xb4 bytes, actor_exec's) from `parent`: on it (its place and turn),
/// its number, its +0x33 less one and +0x54, its delay and speed (+0x70, +0x78) and colour. It,
/// or 0.
const LINK_BEAM_BEHAVIOUR: u32 = 0x0608_6208;

pub fn link_beam_set(cpu: &mut Cpu, parent: u32) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0xb4)) as i8 as i32 as u32;
    let obj = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if obj == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_62ac, actors::actor_setup(cpu, obj));
    let flags = cpu.read32(obj + FLAGS) | SPRITE_FLAG_400;
    cpu.write32(obj + FLAGS, flags);
    cpu.write32(obj + BEHAVIOUR, LINK_BEAM_BEHAVIOUR);
    let n = cpu.read8(parent + NUMBER);
    cpu.write8(obj + NUMBER, n);
    let b = cpu.read8(parent + BYTE_33).wrapping_sub(1);
    cpu.write8(obj + BYTE_33, b);
    let b = cpu.read8(parent + BYTE_54);
    cpu.write8(obj + BYTE_54, b);
    cpu.write32(obj + LINK_PARENT, parent);
    for k in [0, 4, 8] {
        let v = cpu.read32(parent + POSITION + k);
        cpu.write32(obj + POSITION + k, v);
    }
    let turn = cpu.read16(parent + TURN_Z);
    cpu.write16(obj + TURN_Z, turn);
    for at in [LINK_DELAY, LINK_SPEED] {
        let v = cpu.read32(parent + at);
        cpu.write32(obj + at, v);
    }
    let colour = cpu.read16(parent + OWN_COLOUR);
    cpu.write16(obj + OWN_COLOUR, colour);
    cpu.write16(obj + COLOUR, colour);
    obj
}

// ---- the laser head -------------------------------------------------------------------------------

/// A laser head (an actor) that runs along its turn at its speed (+0x78), laying a link beam behind
/// each run, and turns square corners toward its player (+0x74: 0 or 1; the players' objects at
/// 0x060740f0): its count (+0x70: before it ends, and its beams' and glows' delay), its aim (+0x7c,
/// a word), frames (+0x7e, a word), whether it turned back (+0x80), the turn it started a corner at
/// (+0x84, a word), its sound's control (+0x86). Its riser's variant 4 turns it the other way.
const HEAD_PARENT: u32 = 0xb0;
const HEAD_COUNT: u32 = 0x70;
const HEAD_COUNT_LOW: u32 = 0x72;
const HEAD_PLAYER: u32 = 0x74;
const HEAD_SPEED: u32 = 0x78;
const HEAD_AIM: u32 = 0x7c;
const HEAD_FRAMES: u32 = 0x7e;
const HEAD_FRAMES_LOW: u32 = 0x7f;
const HEAD_BACK: u32 = 0x80;
const HEAD_CORNER: u32 = 0x84;
const HEAD_SOUND: u32 = 0x86;
const HEAD_SOUND_ID: u32 = 0x5d;
const PLAYERS: u32 = 0x0607_40f0;
const OFF_SCREEN: u32 = 0x0200_0000;
const CORNER_FRAME: u8 = 0xb5;
const LINK_BEAM_SET: u32 = 0x0608_64fc;

/// Its behaviour. It ends (5) once its parent's step passes 99; off screen (before step 4), 4: its
/// count run out, it ends. Its colour flickers (32 or 64 past its kind's, by the frame's oddness).
/// 0: set up (its sound, no hits, 3/8 scale, its colour 32 past or for player 1 16). 1 and 2: a
/// beam laid; running (1: until the player is within 8 dots of below it and on its side, 2: until
/// the player is above it and behind), 16 frames at least; then 3: it turns (0x300 a frame, 0x200
/// more by difficulty) to face the player's side (1) or up (2), a glow left at the corner (its
/// frames the turn's and its count's), and runs again (1 if now facing up, else 2).
pub fn laser_head_behaviour(cpu: &mut Cpu, obj: u32) {
    let parent = cpu.read32(obj + HEAD_PARENT);
    let kind = cpu.read8(obj + SPRITE_KIND) as i8 as i32 as u32;
    let base = cpu.read16(CHAR_BASE.wrapping_add(16 * kind));
    let past = if cpu.read32(GAME_FRAMES) & 1 != 0 { 0x20 } else { 0x40 };
    cpu.write16(obj + OWN_COLOUR, base.wrapping_add(past));
    let colour = cpu.read16(obj + OWN_COLOUR);
    cpu.write16(obj + COLOUR, colour);
    let part = cpu.read32(parent + PARENT);
    let riser = cpu.read32(part + PARENT);
    let mirrored = cpu.read8(riser + VARIANT) == 4;
    cpu.write8(obj + BYTE_33, if mirrored { 8 } else { 10 });
    if cpu.read8(parent + RISER_STEP) as i8 > 99 {
        cpu.write8(obj + OBJ_MODE, TO_STOP);
        riser_step(cpu, obj, 5);
    }
    if (cpu.read8(obj + RISER_STEP) as i8) < 4 && cpu.read32(obj + FLAGS) & OFF_SCREEN != 0 {
        riser_step(cpu, obj, 4);
    }
    let which = cpu.read8(obj + HEAD_PLAYER) as i8 as i32 as u32;
    let player = cpu.read32(PLAYERS.wrapping_add(4 * which));
    match cpu.read8(obj + RISER_STEP) {
        0 => {
            let control = nested!(cpu, 0x0600_50d4, sound::se_set(cpu, HEAD_SOUND_ID));
            cpu.write8(obj + HEAD_SOUND, control as u8);
            let flags = cpu.read32(obj + FLAGS) | NO_HITS;
            cpu.write32(obj + FLAGS, flags);
            cpu.write8(obj + SPRITE_KIND, 0);
            cpu.write8(obj + SPRITE_FRAME, 0);
            let past = if cpu.read8(obj + HEAD_PLAYER) == 0 { 0x20 } else { 0x10 };
            let kind = cpu.read8(obj + SPRITE_KIND) as i8 as i32 as u32;
            let colour = cpu.read16(CHAR_BASE.wrapping_add(16 * kind)).wrapping_add(past);
            cpu.write16(obj + OWN_COLOUR, colour);
            let colour = cpu.read16(obj + OWN_COLOUR);
            cpu.write16(obj + COLOUR, colour);
            let turn = cpu.read16(obj + TURN_Z);
            cpu.write16(obj + HEAD_AIM, turn);
            cpu.write32(obj + SCALES + 4, 0x6000);
            cpu.write32(obj + SCALES, 0x6000);
            riser_step(cpu, obj, 1);
        }
        step @ (1 | 2) => {
            if cpu.read8(obj + INNER) == 0 {
                nested!(cpu, LINK_BEAM_SET, link_beam_set(cpu, obj));
                cpu.write16(obj + HEAD_FRAMES, 16);
                cpu.write8(obj + INNER, 1);
            }
            if player != 0 && cpu.read16(obj + HEAD_FRAMES) == 0 {
                let (px, py) = (cpu.read32(player + POSITION) as i32, cpu.read32(player + POSITION + 4) as i32);
                let (x, y) = (cpu.read32(obj + POSITION) as i32, cpu.read32(obj + POSITION + 4) as i32);
                if step == 1 {
                    if py.wrapping_sub(0x8_0000) <= y && (if mirrored { px < x } else { px > x }) {
                        cpu.write16(obj + HEAD_AIM, if mirrored { 0xc000 } else { 0x4000 });
                        riser_step(cpu, obj, 3);
                        return head_end(cpu, obj);
                    }
                } else if py >= y && (if mirrored { px >= x } else { px <= x }) {
                    cpu.write16(obj + HEAD_AIM, 0x8000);
                    riser_step(cpu, obj, 3);
                }
            }
            head_run(cpu, obj);
        }
        3 => head_corner(cpu, obj),
        4 => {
            let count = cpu.read32(obj + HEAD_COUNT);
            if count == 0 {
                cpu.write8(obj + OBJ_MODE, TO_STOP);
            } else {
                cpu.write32(obj + HEAD_COUNT, count.wrapping_sub(1));
            }
        }
        _ => {}
    }
    head_end(cpu, obj);
}

/// Its sound stopped as it ends.
fn head_end(cpu: &mut Cpu, obj: u32) {
    if cpu.read8(obj + OBJ_MODE) as i8 > ENDING as i8 {
        let control = cpu.read8(obj + HEAD_SOUND) as i8 as i32 as u32;
        nested!(cpu, 0x0600_52e0, sound::se_stop(cpu, HEAD_SOUND_ID, control));
        cpu.write8(obj + HEAD_SOUND, 0xff);
    }
}

/// A frame's run along its turn, its frames taken down.
fn head_run(cpu: &mut Cpu, obj: u32) {
    let t = cpu.stack();
    cpu.write32(t, 0);
    let v = cpu.read32(obj + HEAD_SPEED);
    cpu.write32(t + 4, v);
    cpu.write32(t + T_SCALES + 4, 0x1_0000);
    cpu.write32(t + T_SCALES, 0x1_0000);
    cpu.write32(t + T_FLAGS, 0);
    let turn = cpu.read16(obj + TURN_Z);
    cpu.write16(t + T_ANGLE, turn);
    nested!(cpu, 0x0606_4eac, math::turn_point(cpu, t));
    for k in [0, 4] {
        let p = cpu.read32(obj + POSITION + k).wrapping_add(cpu.read32(t + k));
        cpu.write32(obj + POSITION + k, p);
    }
    let frames = cpu.read16(obj + HEAD_FRAMES);
    if frames != 0 {
        cpu.write16(obj + HEAD_FRAMES, frames.wrapping_sub(1));
    }
}

/// Step 3: the corner.
fn head_corner(cpu: &mut Cpu, obj: u32) {
    if cpu.read8(obj + INNER) == 0 {
        let turn = cpu.read16(obj + TURN_Z) as i16 as i32 as u32;
        let aim = cpu.read16(obj + HEAD_AIM) as i16 as i32 as u32;
        let d = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, turn, aim)) as i32;
        cpu.write32(obj + HEAD_BACK, (d < 0) as u32);
        let turn = cpu.read16(obj + TURN_Z);
        cpu.write16(obj + HEAD_CORNER, turn);
        cpu.write16(obj + HEAD_FRAMES, 0);
        cpu.write8(obj + INNER, 1);
    }
    let frames = cpu.read16(obj + HEAD_FRAMES).wrapping_add(1);
    cpu.write16(obj + HEAD_FRAMES, frames);
    let most = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 0x200)).wrapping_add(0x300);
    let turn = cpu.read16(obj + TURN_Z) as i16 as i32 as u32;
    let aim = cpu.read16(obj + HEAD_AIM) as i16 as i32 as u32;
    let d = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, turn, aim));
    let step = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, d, most));
    let t = cpu.read16(obj + TURN_Z).wrapping_add(step as u16);
    cpu.write16(obj + TURN_Z, t);
    if t != cpu.read16(obj + HEAD_AIM) {
        return;
    }
    riser_step(cpu, obj, if t == 0x8000 { 1 } else { 2 });
    let fx = cpu.stack();
    for k in [0, 4, 8] {
        let v = cpu.read32(obj + POSITION + k);
        cpu.write32(fx + effects::X + k, v);
    }
    cpu.write32(fx + effects::SCALE_Y, 0xc000);
    cpu.write32(fx + effects::SCALE_X, 0xc000);
    cpu.write32(fx + effects::FLAGS_LONG, 0);
    let corner = cpu.read16(obj + HEAD_CORNER);
    cpu.write16(fx + effects::ANGLE, corner);
    cpu.write8(fx + effects::KIND, 0);
    cpu.write8(fx + effects::FRAME, CORNER_FRAME);
    let colour = cpu.read16(obj + OWN_COLOUR);
    cpu.write16(fx + effects::COLOUR, colour);
    cpu.write8(fx + effects::PLAYER, 0xff);
    let rate = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 0x200)).wrapping_add(0x300);
    let turning = nested!(cpu, 0x0606_62d8, clib::divide(cpu, 0x4000, rate)) as u16;
    let frames = turning.wrapping_add(cpu.read16(obj + HEAD_COUNT_LOW)).wrapping_add(1);
    cpu.write16(fx + effects::FRAMES, frames);
    let turned = cpu.read16(obj + HEAD_FRAMES);
    cpu.write16(fx + effects::FRAMES, frames.wrapping_sub(turned));
    let shrink = cpu.read8(obj + HEAD_FRAMES_LOW).wrapping_add(1);
    cpu.write8(fx + effects::FRAMES_2, shrink);
    cpu.write32(fx + effects::RELATIVE_3C, 0);
    let glow = nested!(cpu, 0x0604_e1cc, effects::effect_set(cpu, fx));
    if glow == 0 {
        return;
    }
    cpu.write32(glow + BEHAVIOUR, GLOW_BEHAVIOUR);
    cpu.write32(glow + GLOW_PARENT, obj);
    let b = cpu.read8(obj + BYTE_33).wrapping_sub(1);
    cpu.write8(glow + BYTE_33, b);
    if cpu.read32(obj + HEAD_BACK) != 0 {
        let flags = cpu.read32(glow + FLAGS) | FLIP_X;
        cpu.write32(glow + FLAGS, flags);
    }
}

/// A laser head made (list 2, 0xb4 bytes, actor_exec's) on `parent` (its number and +0x54): from
/// 12 dots down its model, facing up, at 2 dots a frame, its count `count`; it chases the nearer
/// player (by number). It, or 0.
const LASER_HEAD_BEHAVIOUR: u32 = 0x0608_65c4;

pub fn laser_head_set(cpu: &mut Cpu, parent: u32, count: u32) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0xb4)) as i8 as i32 as u32;
    let obj = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if obj == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_62ac, actors::actor_setup(cpu, obj));
    let flags = cpu.read32(obj + FLAGS) | SPRITE_FLAG_400;
    cpu.write32(obj + FLAGS, flags);
    cpu.write32(obj + BEHAVIOUR, LASER_HEAD_BEHAVIOUR);
    let n = cpu.read8(parent + NUMBER);
    cpu.write8(obj + NUMBER, n);
    cpu.write8(obj + BYTE_33, 8);
    let b = cpu.read8(parent + BYTE_54);
    cpu.write8(obj + BYTE_54, b);
    cpu.write32(obj + HEAD_PARENT, parent);
    cpu.write32(obj + POSITION, 0);
    cpu.write32(obj + POSITION + 4, 0xc_0000);
    cpu.write32(obj + POSITION + 8, 0);
    nested!(cpu, 0x0605_2c44, models::model_point(cpu, parent, obj + POSITION));
    cpu.write16(obj + TURN_Z, 0x8000);
    cpu.write32(obj + HEAD_COUNT, count);
    cpu.write32(obj + HEAD_SPEED, 0xfffe_0000);
    let player = nested!(cpu, 0x0604_5f70, aim::nearer_player_to(cpu, obj));
    let which = cpu.read8(player + NUMBER);
    cpu.write8(obj + HEAD_PLAYER, which);
    obj
}

// ---- the laser head launcher ----------------------------------------------------------------------

/// A laser head launcher on a riser's part: its life (+0x50), its ring's radius (+0x70, 16.16: its
/// ring parts are placed by it), its spin (+0x74, a word: its y turn's speed, the other way for the
/// riser's variant 4), a count (+0x78), its head (+0x7c).
const HEAD_LAUNCHER_LIFE: u32 = 30000;
const RING_SPIN: u32 = 0x74;
const HEAD_WAIT: u32 = 0x78;
const LAUNCHED_HEAD: u32 = 0x7c;
const RING_OUT: u32 = 0xfff1_0000;

/// Its behaviour. 0: life 30000, its ring 6 dots out; 1-2: at the riser's step 15 its ring opens
/// to 15 dots (3/16 of a dot a frame); 3: once its part is up (past 14), unshielded, it spins up (to
/// 0x100); 15: after its count a laser head made (its count 300 and up to 120 more by difficulty),
/// 16: and once it ends, 250 frames less up to 100 by difficulty (15). 100: shot down (5 more to
/// the stage's rate, and the score) or its part sunk, it smokes 90-139 frames (no longer
/// spinning), then its part sinks; 102: debris.
pub fn head_launcher_behaviour(cpu: &mut Cpu, obj: u32) {
    let part = nested!(cpu, 0x0605_2970, models::follow_parent(cpu, obj));
    let parent = cpu.read32(obj + PARENT);
    let riser = cpu.read32(parent + PARENT);
    shot_or_sunk(cpu, obj, part);
    match cpu.read8(obj + RISER_STEP) as i8 {
        0 => {
            cpu.write32(obj + TURRET_LIFE, HEAD_LAUNCHER_LIFE);
            cpu.write32(obj + RING_RADIUS, 0xfffa_0000);
            cpu.write16(obj + RING_SPIN, 0);
            riser_step(cpu, obj, 1);
        }
        1 => {
            if cpu.read8(riser + RISER_STEP) == 15 {
                riser_step(cpu, obj, 2);
            }
        }
        2 => {
            let r = cpu.read32(obj + RING_RADIUS);
            let r = nested!(cpu, 0x0604_6168, actors::approach(cpu, r, RING_OUT, 0x3000));
            cpu.write32(obj + RING_RADIUS, r);
            if r == RING_OUT {
                riser_step(cpu, obj, 3);
            }
        }
        3 => {
            if cpu.read8(part + RISER_STEP) as i8 > 14 {
                let flags = cpu.read32(obj + FLAGS) & !UNHURT;
                cpu.write32(obj + FLAGS, flags);
                let spin = cpu.read16(obj + RING_SPIN) as i16 as i32 as u32;
                let spin = nested!(cpu, 0x0604_6168, actors::approach(cpu, spin, 0x100, 2)) as u16;
                cpu.write16(obj + RING_SPIN, spin);
                if spin == 0x100 {
                    cpu.write32(obj + HEAD_WAIT, 0);
                    riser_step(cpu, obj, 15);
                }
            }
        }
        15 => {
            let wait = cpu.read32(obj + HEAD_WAIT);
            if wait != 0 {
                cpu.write32(obj + HEAD_WAIT, wait.wrapping_sub(1));
            } else {
                let more = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 120)).wrapping_add(300);
                let head = nested!(cpu, 0x0608_6a6c, laser_head_set(cpu, obj, more));
                cpu.write32(obj + LAUNCHED_HEAD, head);
                riser_step(cpu, obj, 16);
            }
        }
        16 => {
            let head = cpu.read32(obj + LAUNCHED_HEAD);
            if head != 0 {
                if cpu.read8(head + OBJ_MODE) as i8 > ENDING as i8 {
                    cpu.write32(obj + LAUNCHED_HEAD, 0);
                }
            } else {
                let less = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 100));
                cpu.write32(obj + HEAD_WAIT, 250u32.wrapping_sub(less));
                riser_step(cpu, obj, 15);
            }
        }
        100 => {
            if cpu.read8(obj + INNER) == 0 {
                cpu.write16(obj + RING_SPIN, 0);
            }
            let puff = Puff { at: cpu.stack(), spread: (0x18_0000, 0x10_0000), lift: 0x10_0000, size: 0x8000 };
            shot_down(cpu, obj, HEAD_WAIT, 5, puff);
        }
        102 => {
            nested!(cpu, 0x0608_08b0, debris_set(cpu, obj));
            cpu.write8(obj + OBJ_MODE, TO_STOP);
        }
        _ => {}
    }
    let parent = cpu.read32(obj + PARENT);
    let riser = cpu.read32(parent + PARENT);
    let spin = cpu.read16(obj + RING_SPIN);
    let turn = cpu.read16(obj + TURN_Y);
    let turn = if cpu.read8(riser + VARIANT) == 4 { turn.wrapping_add(spin) } else { turn.wrapping_sub(spin) };
    cpu.write16(obj + TURN_Y, turn);
}

// ---- the boss -------------------------------------------------------------------------------------

/// The stage's boss: a group of five parts (made from the table at 0x0609f620; the main one at
/// +0xb4, the others at +0xb8, +0xbc, +0xc0, +0xc4), each raised through the steps its variant's
/// row of the table at 0x0609fc38 gives (3 a part; the others' places in their rows at +0x70-0x73),
/// the main one's +0xc0 its core (+0xc4 a second, its life added to the gauge's). Its time
/// (+0x7c: 3 minutes while a player is in), a count (+0x74), its results (+0x78).
const BOSS_PARTS: u32 = 0x0609_f620;
const BOSS_STEPS: u32 = 0x0609_fc38;
const BOSS_MAIN: u32 = 0xb4;
const BOSS_OTHERS: [(u32, u32); 4] = [(0xb8, 0x70), (0xbc, 0x71), (0xc0, 0x72), (0xc4, 0x73)];
const BOSS_ALL: [u32; 5] = [0xb4, 0xb8, 0xc0, 0xbc, 0xc4];
const BOSS_PLACES: u32 = 0x70;
const BOSS_COUNT: u32 = 0x74;
const BOSS_RESULTS: u32 = 0x78;
const BOSS_TIME: u32 = 0x7c;
const BOSS_OLD: u32 = 0xb0;
const CORE: u32 = 0xc0;
const SECOND_CORE: u32 = 0xc4;
const BOSS_TIME_LIMIT: u32 = 10800;
const BOSS_GAUGE: u32 = 80000;
const STAGE_SCORE_AT: u32 = 0x0607_46f4;
const BOSS_BONUS: u32 = 100_000;
const BOSS_RATE: u32 = 20;
const FULL_SPEED: u32 = 0x0600_ed90;
const CORE_ENDING: u16 = 120;
const CORE_BLAST: u32 = 0x0609_b820;

/// Its behaviour. 0: its parts made (tried every 5th frame until they are; hidden; the object at
/// +0xb0 ended; the stage's rate and score cleared). 1: once all five are at step 1, each set going
/// by its row's first step; 2: all at 10, 3: the main one to 11, the others to 12; 4: all at 15,
/// 5: once FULL_SPEED (director.rs's) is clear, its gauge made (80000), 15: the fight: each other
/// part back at 1 goes on by its row (3 steps each), back at 10 to 12; the gauge kept; its time
/// running out, or the core shot down (100000 and 20 more to the rate), 100: no hits, the bullets
/// cleared, 60 frames, the results; once they end, the core ends (120) with its blast, and 120
/// frames after, the boss.
pub fn boss_behaviour(cpu: &mut Cpu, obj: u32) {
    match cpu.read8(obj + RISER_STEP) as i8 {
        0 => {
            let wait = cpu.read8(obj + INNER);
            if wait != 0 {
                cpu.write8(obj + INNER, wait.wrapping_sub(1));
                return;
            }
            let flags = cpu.read32(obj + FLAGS) | NO_DRAW;
            cpu.write32(obj + FLAGS, flags);
            cpu.write8(obj + INNER, 4);
            if nested!(cpu, 0x0605_2ff0, models::model_group_set(cpu, obj, BOSS_PARTS)) == 0 {
                return;
            }
            cpu.write32(obj + MODEL, 0);
            let old = cpu.read32(obj + BOSS_OLD);
            cpu.write8(old + OBJ_MODE, TO_STOP);
            cpu.write32(STAGE_RATE_AT, 0);
            cpu.write32(STAGE_SCORE_AT, 0);
            cpu.write32(obj + BOSS_PLACES, 0);
            cpu.write32(obj + BOSS_TIME, BOSS_TIME_LIMIT);
            riser_step(cpu, obj, 1);
        }
        1 => {
            if !boss_parts_at(cpu, obj, 1) {
                return;
            }
            let main = cpu.read32(obj + BOSS_MAIN);
            let step = boss_row_step(cpu, main, 0);
            riser_step(cpu, main, step);
            for (slot, place) in BOSS_OTHERS {
                let part = cpu.read32(obj + slot);
                let at = cpu.read8(obj + place);
                let step = boss_row_step(cpu, part, at);
                riser_step(cpu, part, step);
            }
            for (_, place) in BOSS_OTHERS {
                let at = cpu.read8(obj + place).wrapping_add(1);
                cpu.write8(obj + place, at);
            }
            riser_step(cpu, obj, 2);
        }
        2 => {
            if boss_parts_at(cpu, obj, 10) {
                riser_step(cpu, obj, 3);
            }
        }
        3 => {
            let main = cpu.read32(obj + BOSS_MAIN);
            riser_step(cpu, main, 11);
            for slot in [0xb8, 0xc0, 0xbc, 0xc4] {
                let part = cpu.read32(obj + slot);
                cpu.write8(part + RISER_STEP, 12);
                cpu.write8(part + INNER, 1);
            }
            riser_step(cpu, obj, 4);
        }
        4 => {
            if boss_parts_at(cpu, obj, 15) {
                riser_step(cpu, obj, 5);
            }
        }
        5 => {
            if cpu.read8(FULL_SPEED) == 0 {
                nested!(cpu, 0x0604_64d8, hud::boss_gauge_set(cpu, BOSS_GAUGE));
                riser_step(cpu, obj, 15);
            }
        }
        15 => boss_fight(cpu, obj),
        100 => boss_ending(cpu, obj),
        _ => {}
    }
}

/// Whether all its parts are at step `step`.
fn boss_parts_at(cpu: &mut Cpu, obj: u32, step: i8) -> bool {
    BOSS_ALL.into_iter().all(|slot| {
        let part = cpu.read32(obj + slot);
        cpu.read8(part + RISER_STEP) as i8 == step
    })
}

/// The step at place `at` of a part's row (by its variant), as a step word (inner 0).
fn boss_row_step(cpu: &mut Cpu, part: u32, at: u8) -> u16 {
    let variant = cpu.read8(part + VARIANT) as i8 as i32;
    let row = BOSS_STEPS.wrapping_add((variant * 3 + at as i8 as i32) as u32);
    cpu.read8(row) as i8 as i16 as u16
}

/// Step 15: the fight.
fn boss_fight(cpu: &mut Cpu, obj: u32) {
    for (slot, place) in BOSS_OTHERS {
        let part = cpu.read32(obj + slot);
        if (cpu.read8(obj + place) as i8) <= 2 && cpu.read8(part + RISER_STEP) == 1 {
            let at = cpu.read8(obj + place);
            let step = boss_row_step(cpu, part, at);
            riser_step(cpu, part, step);
            cpu.write8(obj + place, at.wrapping_add(1));
        }
        if cpu.read8(part + RISER_STEP) == 10 {
            cpu.write8(part + RISER_STEP, 12);
            cpu.write8(part + INNER, 1);
        }
    }
    let main = cpu.read32(obj + BOSS_MAIN);
    let core = cpu.read32(main + CORE);
    let mut life = cpu.read32(core + TURRET_LIFE);
    let second = cpu.read32(main + SECOND_CORE);
    if second != 0 {
        life = life.wrapping_add(cpu.read32(second + TURRET_LIFE));
    }
    nested!(cpu, 0x0604_64b8, hud::boss_gauge_life(cpu, life));
    if cpu.read32(PLAYERS) != 0 || cpu.read32(PLAYERS + 4) != 0 {
        let time = cpu.read32(obj + BOSS_TIME).wrapping_sub(1);
        cpu.write32(obj + BOSS_TIME, time);
    }
    if cpu.read32(obj + BOSS_TIME) as i32 > 0 {
        let main = cpu.read32(obj + BOSS_MAIN);
        let core = cpu.read32(main + CORE);
        if cpu.read32(core + TURRET_LIFE) as i32 > 0 {
            return;
        }
        cpu.write32(STAGE_SCORE_AT, BOSS_BONUS);
        let rate = cpu.read32(STAGE_RATE_AT).wrapping_add(BOSS_RATE);
        cpu.write32(STAGE_RATE_AT, rate);
    }
    riser_step(cpu, obj, 100);
}

/// Step 100's inner steps.
fn boss_ending(cpu: &mut Cpu, obj: u32) {
    let main = cpu.read32(obj + BOSS_MAIN);
    match cpu.read8(obj + INNER) {
        0 => {
            let flags = cpu.read32(obj + FLAGS) | NO_HITS;
            cpu.write32(obj + FLAGS, flags);
            nested!(cpu, 0x0605_61e0, screen::bullets_cleared(cpu));
            cpu.write32(obj + BOSS_COUNT, 0);
            cpu.write8(obj + INNER, 2);
        }
        2 => {
            let count = cpu.read32(obj + BOSS_COUNT).wrapping_add(1);
            cpu.write32(obj + BOSS_COUNT, count);
            if count as i32 > 59 {
                let results = nested!(cpu, 0x0605_284c, results::results_set(cpu, 0));
                cpu.write32(obj + BOSS_RESULTS, results);
                if results != 0 {
                    cpu.write8(obj + INNER, 3);
                }
            }
        }
        3 => {
            let results = cpu.read32(obj + BOSS_RESULTS);
            if cpu.read8(results + OBJ_MODE) as i8 > ENDING as i8 {
                let core = cpu.read32(main + CORE);
                riser_step(cpu, core, CORE_ENDING);
                let core = cpu.read32(main + CORE);
                cpu.call(CORE_BLAST, &[core + POSITION]);
                cpu.write32(obj + BOSS_COUNT, 0);
                cpu.write8(obj + INNER, 4);
            }
        }
        4 => {
            let count = cpu.read32(obj + BOSS_COUNT).wrapping_add(1);
            cpu.write32(obj + BOSS_COUNT, count);
            if count as i32 > 119 {
                cpu.write8(obj + OBJ_MODE, TO_STOP);
            }
        }
        _ => {}
    }
}

/// The boss made (numbered: list 2, 0x138 bytes, numbered_exec's; an enemy's number): its sprite
/// and boxes set up, kind 0x2f, life 0x100000, side 2 and type 3, flag 0x20000, at the origin; its
/// number, or 0 (no number or no room).
const BOSS_SIZE: u32 = 0x138;
const NUMBERED_EXEC: u32 = 0x0604_d600;
const ENEMY_NUMBERS: u32 = 2;
const BOSS_KIND: u8 = 0x2f;
const BOSS_LIFE: u32 = 0x10_0000;
const SIDE: u32 = 0x54;
const TYPE: u32 = 0x55;
const BOSS_FLAG: u32 = 0x2_0000;
const BOSS_BEHAVIOUR: u32 = 0x0608_6e6c;

pub fn boss_set(cpu: &mut Cpu) -> u32 {
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, ENEMY_NUMBERS)) as i8;
    if n == -1 {
        return 0;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, BOSS_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, NUMBERED_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + BEAM_LIST));
    cpu.write8(o + SPRITE_KIND, BOSS_KIND);
    cpu.write8(o + SPRITE_FRAME, 0);
    cpu.write16(o + INNER, 0);
    cpu.write32(o + TURRET_LIFE, BOSS_LIFE);
    cpu.write8(o + SIDE, 2);
    cpu.write8(o + TYPE, 3);
    let flags = cpu.read32(o + FLAGS) | BOSS_FLAG;
    cpu.write32(o + FLAGS, flags);
    cpu.write32(o + BEHAVIOUR, BOSS_BEHAVIOUR);
    cpu.write8(o + NUMBER, n as u8);
    let n = cpu.read8(o + NUMBER) as i8 as i32;
    cpu.write32(PLAYERS.wrapping_add((4 * n) as u32), o);
    for k in [8, 4, 0] {
        cpu.write32(o + POSITION + k, 0);
    }
    cpu.read8(o + NUMBER) as i8 as i32 as u32
}

// ---- the depth-tinted ground ----------------------------------------------------------------------

/// Ground enemies whose colour bank (bits 11-13 of their own colour, +0x2a) follows their depth: a
/// bank every 8 dots past 64 dots (to 7), or past 32 dots (banks 4 to 7).
const BANK_BITS: u16 = 0x3800;

/// ground_exec's run, then its bank by its depth past 64 dots.
pub fn deep_ground_exec(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0605_a4cc, ground::ground_exec(cpu, o));
    let bank = depth_bank(cpu, o, 0x40_0000, 0x3800);
    let colour = cpu.read16(o + OWN_COLOUR) & !BANK_BITS | bank & BANK_BITS;
    cpu.write16(o + OWN_COLOUR, colour);
}

/// wounded_exec's run, then its bank (from 4) by its depth past 32 dots.
pub fn deep_wounded_exec(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0605_b568, ground::wounded_exec(cpu, o));
    let bank = depth_bank(cpu, o, 0x20_0000, 0x1800);
    let colour = cpu.read16(o + OWN_COLOUR) & !BANK_BITS | (bank & 0x1800).wrapping_add(0x2000);
    cpu.write16(o + OWN_COLOUR, colour);
}

/// Its depth past `from` in 256ths of a dot (toward 0), within 0 and `most`.
fn depth_bank(cpu: &mut Cpu, o: u32, from: u32, most: i32) -> u16 {
    let past = cpu.read32(o + POSITION + 8).wrapping_sub(from) as i32 / 0x100;
    past.clamp(0, most) as u16
}

// ---- the route flier ------------------------------------------------------------------------------

/// A flier along a route (ground.rs's route_step; its script +0x6c): its flags (+0x7e, a word:
/// 0x1000 ends it off screen, 0x20 turned over by the route's flag 0x20), its speed (x +0x80, by its
/// side +0x88: 1 or -1; y +0x84), its depth (+0xb0), its turn's step (+0x8c, a word, by its side).
/// Its frame runs through 32.
const ROUTE_SCRIPT: u32 = 0x6c;
const ROUTE_FLAGS: u32 = 0x7e;
const ROUTE_SPEED: u32 = 0x80;
const ROUTE_SIDE: u32 = 0x88;
const ROUTE_TURN: u32 = 0x8c;
const ROUTE_DEPTH: u32 = 0xb0;
const ENDS_OFF_SCREEN: u16 = 0x1000;
const ROUTE_FLIP: u16 = 0x20;

pub fn route_flier_behaviour(cpu: &mut Cpu, obj: u32) {
    if cpu.read16(obj + ROUTE_FLAGS) & ENDS_OFF_SCREEN != 0
        && nested!(cpu, 0x0605_6a24, aim::off_screen(cpu, obj + FLAGS)) as u8 != 0
    {
        cpu.write8(obj + OBJ_MODE, TO_STOP);
        return;
    }
    let script = cpu.read32(obj + ROUTE_SCRIPT);
    if nested!(cpu, 0x0605_a000, ground::route_step(cpu, obj, script)) as u8 == ROUTE_FLIP as u8 {
        let flags = cpu.read16(obj + ROUTE_FLAGS) ^ ROUTE_FLIP;
        cpu.write16(obj + ROUTE_FLAGS, flags);
    }
    let side = cpu.read8(obj + ROUTE_SIDE) as i8 as i32;
    let dx = (cpu.read32(obj + ROUTE_SPEED) as i32).wrapping_mul(side) as u32;
    let x = cpu.read32(obj + POSITION).wrapping_add(dx);
    cpu.write32(obj + POSITION, x);
    let y = cpu.read32(obj + POSITION + 4).wrapping_add(cpu.read32(obj + ROUTE_SPEED + 4));
    cpu.write32(obj + POSITION + 4, y);
    let z = cpu.read32(obj + ROUTE_DEPTH);
    cpu.write32(obj + POSITION + 8, z);
    let frame = cpu.read8(obj + SPRITE_FRAME).wrapping_add(1) & 31;
    cpu.write8(obj + SPRITE_FRAME, frame);
    let side = cpu.read8(obj + ROUTE_SIDE) as i8 as i16;
    let turn = side.wrapping_mul(cpu.read16(obj + ROUTE_TURN) as i16);
    cpu.write16(obj + TURN_Z, turn as u16);
}

/// A route flier made (list 3, 0xbc bytes, plain) from a ground description `d` (the spawner's):
/// kind 6, coloured by the description's type (16 colours a type past the kind's, bank 7), at its
/// place, its side by its mirror (flipped if negative), its script `script`, its direction kept at
/// +0xa8. It, or 0.
const FLIER_KIND: u8 = 6;
const FLIER_COLOUR: u32 = 0x0607_425c;
const ROUTE_FLIER: u32 = 0x0608_75cc;
const FLIER_DIRECTION: u32 = 0xa8;

pub fn route_flier_set(cpu: &mut Cpu, d: u32, script: u32) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0xbc)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 3, blocks, PLAIN_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + FLAGS));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + BEHAVIOUR, ROUTE_FLIER);
    cpu.write8(o + SPRITE_KIND, FLIER_KIND);
    cpu.write8(o + SPRITE_FRAME, 0);
    let colour = (cpu.read16(d + D_TYPE) << 4).wrapping_add(cpu.read16(FLIER_COLOUR)) | BANK_BITS;
    cpu.write16(o + OWN_COLOUR, colour);
    cpu.write16(o + COLOUR, colour);
    for k in [0, 4, 8] {
        let v = cpu.read32(d + D_PLACE + k);
        cpu.write32(o + POSITION + k, v);
    }
    let side = cpu.read8(d + D_MIRROR + 3);
    cpu.write8(o + ROUTE_SIDE, side);
    if (side as i8) < 0 {
        let flags = cpu.read32(o + FLAGS) | FLIP_X;
        cpu.write32(o + FLAGS, flags);
    }
    cpu.write32(o + ROUTE_SCRIPT, script);
    let direction = cpu.read16(d + D_DIRECTION);
    cpu.write16(o + FLIER_DIRECTION, direction);
    o
}

// ---- the enemy rows -------------------------------------------------------------------------------

/// A row of enemies (a timeline event, timeline.rs's: called each frame until it gives 1), as
/// STG00's ground_row with more kinds: one made every `E_DELAY + 1` frames by its kind (0-2
/// ground_set's, 3-6 guns, 7-9 the wounded, 10-13 platforms, 14-15 launchers, 16-17 spreaders,
/// 18 a carrier, 19 the sinking craft, 20 a burster, 21 a hatch; 128 a route flier, 129 and
/// 130 ground and wounded tinted by their depth), each `E_STEP_X`, `E_STEP_Y` and `E_TURN` on from
/// the last, until `E_COUNT` are made. Its motion (`E_MOTION`) places each round an arc first and
/// changes the type after.
const E_KIND: u32 = 0x01;
const E_PLACE: u32 = 0x04;
const E_SCRIPT: u32 = 0x08;
const E_MOTION: u32 = 0x09;
const E_SETTINGS: u32 = 0x0a;
const E_MIRROR: u32 = 0x0b;
const E_COUNT: u32 = 0x10;
const E_DELAY: u32 = 0x11;
const E_MADE: u32 = 0x12;
const E_WAIT: u32 = 0x13;
const E_DIRECTION: u32 = 0x14;
const E_TURN: u32 = 0x16;
const E_STEP_X: u32 = 0x18;
const E_STEP_Y: u32 = 0x19;
const E_1A: u32 = 0x1a;
const E_TYPE: u32 = 0x1b;
const E_1C: u32 = 0x1c;
const E_1D: u32 = 0x1d;
const E_1E: u32 = 0x1e;
const E_1F: u32 = 0x1f;
/// An arc's radius and angle (words, the motions'), over E_1C-E_1F.
const E_RADIUS: u32 = 0x1c;
const E_ANGLE: u32 = 0x1e;
/// A ground piece's description (ground.rs's, on the stack; stg00.rs's names): its place, mirror,
/// kind, type, direction, settings, and the kinds' own words.
const D_PLACE: u32 = 0x00;
const D_MIRROR: u32 = 0x14;
const D_KIND: u32 = 0x1a;
const D_TYPE: u32 = 0x2c;
const D_DIRECTION: u32 = 0x2e;
const D_SETTINGS: u32 = 0x30;
const D_32: u32 = 0x32;
const D_34: u32 = 0x34;
const D_36: u32 = 0x36;
const D_38: u32 = 0x38;
const D_3A: u32 = 0x3a;
/// The kinds' scripts (by E_SCRIPT) and aims.
const GROUND_SCRIPTS: u32 = 0x0609_fe9c;
const GUN_SCRIPTS: u32 = 0x0609_ffac;
const GUN_AIMS: u32 = 0x0609_ffc0;
const WOUNDED_SCRIPTS: u32 = 0x060a_00ec;
const WOUNDED_AIMS: u32 = 0x060a_0120;
const PLATFORM_SCRIPTS: u32 = 0x060a_0258;
const CRAFT_SET: u32 = 0x0608_862c;
const DEEP_GROUND_EXEC: u32 = 0x0608_7500;
const DEEP_WOUNDED_EXEC: u32 = 0x0608_7564;

pub fn enemy_row(cpu: &mut Cpu, event: u32) -> u32 {
    if cpu.read8(event + E_WAIT) == 0 {
        let d = cpu.stack();
        loop {
            if cpu.read8(event + E_MOTION) != 0 {
                nested!(cpu, 0x0608_7748, row_arc(cpu, event));
            }
            let delay = cpu.read8(event + E_DELAY);
            cpu.write8(event + E_WAIT, delay);
            let x = (cpu.read16(event + E_PLACE) as i16 as i32 as u32) << 16;
            cpu.write32(d + D_PLACE, x);
            let y = (cpu.read16(event + E_PLACE + 2) as i16 as i32 as u32) << 16;
            cpu.write32(d + D_PLACE + 4, y);
            cpu.write32(d + D_PLACE + 8, 0);
            let t = cpu.read8(event + E_TYPE) as u16;
            cpu.write16(d + D_TYPE, t);
            let direction = cpu.read16(event + E_DIRECTION);
            cpu.write16(d + D_DIRECTION, direction);
            let settings = cpu.read8(event + E_SETTINGS) as u16;
            cpu.write16(d + D_SETTINGS, settings);
            let mirror = cpu.read8(event + E_MIRROR) as i8 as i32 as u32;
            cpu.write32(d + D_MIRROR, mirror);
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
                nested!(cpu, 0x0608_7844, row_retype(cpu, event));
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
            cpu.write16(d + D_KIND, kind_word(0));
            let s = script(cpu, GROUND_SCRIPTS);
            nested!(cpu, 0x0605_a780, ground::ground_set(cpu, d, s));
        }
        3..=6 => {
            cpu.write16(d + D_KIND, kind_word(3));
            let (s, aim) = (script(cpu, GUN_SCRIPTS), script(cpu, GUN_AIMS));
            nested!(cpu, 0x0605_b168, ground::gun_set(cpu, d, s, aim));
        }
        7..=9 | 130 => {
            cpu.write16(d + D_KIND, if kind == 130 { 0 } else { kind_word(7) });
            let v = byte(cpu, E_1E) << 8;
            cpu.write16(d + D_34, v);
            let v = byte(cpu, E_1F);
            cpu.write16(d + D_32, v);
            let at = cpu.read8(event + E_1C) as i8 as i32 as u32;
            let aim = cpu.read32(WOUNDED_AIMS.wrapping_add(4 * at));
            let s = script(cpu, WOUNDED_SCRIPTS);
            let o = nested!(cpu, 0x0605_bbac, ground::wounded_set(cpu, d, s, aim));
            if kind == 130 && o != 0 {
                cpu.write32(o + BEHAVIOUR, DEEP_WOUNDED_EXEC);
            }
        }
        10..=13 => {
            cpu.write16(d + D_KIND, kind_word(10));
            let v = cpu.read8(event + E_1A) as u16;
            cpu.write16(d + D_32, v);
            for (to, from) in [(D_34, E_1C), (D_36, E_1D), (D_38, E_1E), (D_3A, E_1F)] {
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
            cpu.write16(d + D_KIND, kind_word(14));
            let v = cpu.read8(event + E_TYPE) as u16;
            cpu.write16(d + D_32, v);
            let v = cpu.read8(event + E_1A) as u16;
            cpu.write16(d + D_34, v);
            nested!(cpu, 0x0605_c464, ground::launcher_set(cpu, d, 0));
        }
        16 | 17 => {
            cpu.write16(d + D_KIND, kind_word(16));
            let s = script(cpu, PLATFORM_SCRIPTS);
            nested!(cpu, 0x0605_cee8, ground::spreader_set(cpu, d, s));
        }
        18 => {
            let v = cpu.read16(event + E_1E);
            cpu.write16(d + D_34, v);
            let carried = cpu.read16(event + E_1C) as u32;
            let s = script(cpu, PLATFORM_SCRIPTS);
            nested!(cpu, 0x0605_d51c, ground::carrier_set(cpu, d, s, carried));
        }
        19 => {
            let s = script(cpu, WOUNDED_SCRIPTS);
            nested!(cpu, CRAFT_SET, craft_set(cpu, d, s));
        }
        20 => {
            let s = script(cpu, GROUND_SCRIPTS);
            nested!(cpu, 0x0605_da58, ground::burster_set(cpu, d, s));
        }
        21 => {
            let s = script(cpu, GROUND_SCRIPTS);
            nested!(cpu, 0x0605_e030, ground::hatch_set(cpu, d, s));
        }
        128 => {
            cpu.write16(d + D_KIND, 0);
            let s = script(cpu, GROUND_SCRIPTS);
            nested!(cpu, 0x0608_7678, route_flier_set(cpu, d, s));
        }
        129 => {
            cpu.write16(d + D_KIND, 0);
            let s = script(cpu, GROUND_SCRIPTS);
            let o = nested!(cpu, 0x0605_a780, ground::ground_set(cpu, d, s));
            if o != 0 {
                cpu.write32(o + BEHAVIOUR, DEEP_GROUND_EXEC);
            }
        }
        _ => {}
    }
}

/// A row's place round its arc (before each is made): motion 1, the arc's radius out from 160 dots
/// above, its angle on by a sixth of a turn (by the side, E_MIRROR) each; 4, flattened to a quarter
/// high from 144 dots above, on by 1/32 of a turn.
pub fn row_arc(cpu: &mut Cpu, event: u32) {
    let motion = cpu.read8(event + E_MOTION);
    if motion != 1 && motion != 4 {
        return;
    }
    let angle = cpu.read16(event + E_ANGLE) as i16 as i32 as u32;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, angle)) as i32;
    let r = cpu.read16(event + E_RADIUS) as i16 as i32;
    cpu.write16(event + E_PLACE, (s.wrapping_mul(r) / 0x1_0000) as u16);
    let angle = cpu.read16(event + E_ANGLE) as i16 as i32 as u32;
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, angle)) as i32;
    let r = cpu.read16(event + E_RADIUS) as i16 as i32;
    let down = c.wrapping_neg().wrapping_mul(r);
    let side = cpu.read8(event + E_MIRROR) as i8 as i32;
    let (y, on) = if motion == 1 {
        (down.wrapping_sub(0xa0_0000) / 0x1_0000, side.wrapping_mul(0x2aaa))
    } else {
        (down / 0x4_0000 - 0x90, side << 11)
    };
    cpu.write16(event + E_PLACE + 2, y as u16);
    let angle = cpu.read16(event + E_ANGLE).wrapping_add(on as u16);
    cpu.write16(event + E_ANGLE, angle);
}

/// A row's type after each is made, by its motion: 1, by turns (1 after the even ones, 0 after the
/// odd); 2 and 3, on through 0-2 every 6th and 3rd; 5, its kind back to 0.
pub fn row_retype(cpu: &mut Cpu, event: u32) {
    match cpu.read8(event + E_MOTION) {
        1 => {
            let t = if cpu.read8(event + E_MADE) & 1 == 0 { 1 } else { 0 };
            cpu.write8(event + E_TYPE, t);
        }
        m @ (2 | 3) => {
            let made = cpu.read8(event + E_MADE) as u32;
            let every = if m == 2 { 6 } else { 3 };
            if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, made, every)) == 0 {
                let t = cpu.read8(event + E_TYPE) as u32 + 1;
                let t = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, t, 3));
                cpu.write8(event + E_TYPE, t as u8);
            }
        }
        5 => cpu.write8(event + E_KIND, 0),
        _ => {}
    }
}

// ---- the sinking craft ----------------------------------------------------------------------------

/// A big ground craft on a route (ground.rs's route_step and route_move; its script +0x6c, its
/// flags +0x7e, its side +0x88) with parts (+0x74 and +0x78 flashed with it): sinking (+0xa6, a
/// word), its sink's speed (+0xac), a count (+0xb0).
const SINKING: u32 = 0xa6;
const SINK_SPEED: u32 = 0xac;
const SINK_COUNT: u32 = 0xb0;
const CRAFT_PARTS: [u32; 2] = [0x74, 0x78];
const ROUTE_OUT: (u32, u32, u32, u32) = (0xfe80_0000, 0xc0_0000, 0xff00_0000, 0x100_0000);
const ROUTE_STOPS: u8 = 7;
const ROUTE_GOES: u8 = 9;
const ROUTE_STOPPED: u16 = 0x10;
const CRAFT_COLOUR: u32 = 0x0607_429c;
const SINK_SOUNDS: [u32; 2] = [18, 17];

/// Its behaviour. Leaving the area (-24 to 12 dots high, -16 to 16 across: flag 0x1000) it ends.
/// Shot down: no hits, the screen flashed (from 50 grey to black), it sinks (to 2 dots a frame)
/// smoking (every other frame, up to 3 dots off), a crash sound every 40 frames, flickering its
/// colour and turning (16 a frame by its side), blinking after 150 frames, gone after 180. Else
/// it runs its route (its flag 0x10 set at the route's 7, cleared at 9), its parts flashed with it.
pub fn sinking_craft_behaviour(cpu: &mut Cpu, o: u32) {
    if cpu.read16(o + SINKING) == 0 {
        let shot = cpu.read32(o + TURRET_LIFE) as i32 <= 0;
        let out = !shot && cpu.read16(o + ROUTE_FLAGS) & ENDS_OFF_SCREEN != 0 && {
            let (top, bottom, left, right) = ROUTE_OUT;
            cpu.nested_with_stack(&aim::AREA_FLAGS_PROLOGUE, aim::AREA_FLAGS, &[right], |cpu| {
                aim::area_flags(cpu, o + FLAGS, top, bottom, left, right)
            }) as u8
                != 0
        };
        if out {
            cpu.write8(o + OBJ_MODE, TO_STOP);
            return;
        }
        if shot {
            let sinking = cpu.read16(o + SINKING).wrapping_add(1);
            cpu.write16(o + SINKING, sinking);
            let flags = cpu.read32(o + FLAGS) | NO_HITS;
            cpu.write32(o + FLAGS, flags);
            cpu.write32(o + SINK_SPEED, 0);
            cpu.write32(o + SINK_COUNT, 0);
            nested!(cpu, 0x0605_e9d8, screen::stop_fades(cpu));
            nested!(cpu, 0x0605_e6dc, screen::set_fade_offset(cpu, 50, 50, 50));
            nested!(cpu, 0x0605_e93c, screen::fade_to(cpu, 0, 0, 0, 0x800));
            return;
        }
        let script = cpu.read32(o + ROUTE_SCRIPT);
        match nested!(cpu, 0x0605_a000, ground::route_step(cpu, o, script)) as u8 {
            ROUTE_STOPS => {
                let flags = cpu.read16(o + ROUTE_FLAGS) | ROUTE_STOPPED;
                cpu.write16(o + ROUTE_FLAGS, flags);
            }
            ROUTE_GOES => {
                let flags = cpu.read16(o + ROUTE_FLAGS) & !ROUTE_STOPPED;
                cpu.write16(o + ROUTE_FLAGS, flags);
            }
            _ => {}
        }
        nested!(cpu, 0x0605_a460, ground::route_move(cpu, o));
        for at in CRAFT_PARTS {
            let part = cpu.read32(o + at);
            if part != 0 {
                nested!(cpu, 0x0604_668c, parts::flash_against(cpu, o, part));
            }
        }
        return;
    }
    let v = cpu.read32(o + SINK_SPEED);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0x2_0000, 0x80));
    cpu.write32(o + SINK_SPEED, v);
    let z = cpu.read32(o + POSITION + 8).wrapping_add(v);
    cpu.write32(o + POSITION + 8, z);
    let count = cpu.read32(o + SINK_COUNT).wrapping_add(1);
    cpu.write32(o + SINK_COUNT, count);
    if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, count, 40)) == 0 {
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, SINK_SOUNDS[(r & 1) as usize]));
    }
    if cpu.read32(o + SINK_COUNT) & 1 == 0 {
        let e = cpu.stack();
        let x = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x30_0000)).wrapping_add(cpu.read32(o + POSITION));
        cpu.write32(e, x);
        let y = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x30_0000)).wrapping_add(cpu.read32(o + POSITION + 4));
        cpu.write32(e + 4, y);
        let z = cpu.read32(o + POSITION + 8);
        cpu.write32(e + 8, z);
        crash_puff(cpu, e);
    }
    crash_colour(cpu, o);
    let side = cpu.read8(o + ROUTE_SIDE) as i8 as i16 as u16;
    let turn = cpu.read16(o + TURN_Z).wrapping_add(side << 4);
    cpu.write16(o + TURN_Z, turn);
    let mut flags = cpu.read32(o + FLAGS) & !NO_DRAW;
    cpu.write32(o + FLAGS, flags);
    let count = cpu.read32(o + SINK_COUNT);
    if count as i32 > 150 && count & 1 != 0 {
        flags |= NO_DRAW;
        cpu.write32(o + FLAGS, flags);
    }
    if cpu.read32(o + SINK_COUNT) as i32 > 180 {
        cpu.write8(o + OBJ_MODE, TO_STOP);
    }
}

/// A crash's smoke puff (made at `e`, its place set): its size 3/4 less up to 7/32 at random.
fn crash_puff(cpu: &mut Cpu, e: u32) {
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write32(e + 12, 0xc000u32.wrapping_sub((r & 7) << 11));
    nested!(cpu, 0x0605_77d0, explosions::smoke_puff(cpu, e));
}

/// A crash's flicker: its colour at random the craft's (32 past) or 80 past the first character.
fn crash_colour(cpu: &mut Cpu, o: u32) {
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    let colour = if r & 1 != 0 {
        cpu.read16(CRAFT_COLOUR).wrapping_add(0x20)
    } else {
        cpu.read16(CHAR_BASE).wrapping_add(0x50)
    };
    cpu.write16(o + OWN_COLOUR, colour);
    let colour = cpu.read16(o + OWN_COLOUR);
    cpu.write16(o + COLOUR, colour);
}

/// A part of the craft (+0x6c; at its place, y 104 dots on for frame 1, else 200): following it and
/// flashing with it. While the craft lives, a hit on the part takes from the craft's life (the
/// part's own life, kept at 1, less one), with the score (by the part's kind) for the kill;
/// hitting as the craft does. Once the craft sinks, hidden as it is, no hits, smoking and
/// flickering, turning with it (frame 1) or against it.
const CRAFT: u32 = 0x6c;
const WAS_HIT_FLAG: u16 = 0x200;
const KIND_POINTS: u32 = 0x0607_41fe;

pub fn craft_part_behaviour(cpu: &mut Cpu, part: u32) {
    let craft = cpu.read32(part + CRAFT);
    if cpu.read8(craft + OBJ_MODE) as i8 > ENDING as i8 {
        cpu.write8(part + OBJ_MODE, TO_STOP);
        return;
    }
    let upper = cpu.read8(part + SPRITE_FRAME) == 1;
    if cpu.read16(craft + SINKING) != 0 {
        let flags = cpu.read32(part + FLAGS) & !NO_DRAW;
        cpu.write32(part + FLAGS, flags);
        let flags = flags | NO_HITS | cpu.read32(craft + FLAGS) & NO_DRAW;
        cpu.write32(part + FLAGS, flags);
        cpu.write16(part + SINKING, 1);
        let e = cpu.stack();
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
        cpu.write32(e + 12, 0xc000u32.wrapping_sub((r & 7) << 11));
        let z = cpu.read32(part + POSITION + 8);
        cpu.write32(e + 8, z);
        let count = cpu.read32(craft + SINK_COUNT);
        let (smokes, spread) = if upper { (count & 1 != 0, (0x8_0000, 0x70_0000)) } else { (count & 3 == 2, (0x10_0000, 0x10_0000)) };
        if smokes {
            let x = nested!(cpu, 0x0604_5d90, math::random(cpu, spread.0)).wrapping_add(cpu.read32(part + POSITION));
            cpu.write32(e, x);
            let y = nested!(cpu, 0x0604_5d90, math::random(cpu, spread.1)).wrapping_add(cpu.read32(part + POSITION + 4));
            cpu.write32(e + 4, y);
            nested!(cpu, 0x0605_77d0, explosions::smoke_puff(cpu, e));
        }
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
        let colour = if r & 1 != 0 {
            cpu.read16(CRAFT_COLOUR).wrapping_add(0x20)
        } else {
            cpu.read16(CHAR_BASE).wrapping_add(0x50)
        };
        cpu.write16(part + OWN_COLOUR, colour);
        let side = (cpu.read8(craft + ROUTE_SIDE) as i8 as i16 as u16) << 4;
        let turn = cpu.read16(part + TURN_Z);
        cpu.write16(part + TURN_Z, if upper { turn.wrapping_add(side) } else { turn.wrapping_sub(side) });
        let colour = cpu.read16(part + OWN_COLOUR);
        cpu.write16(part + COLOUR, colour);
    } else {
        let life = cpu.read32(craft + TURRET_LIFE);
        if life as i32 > 0 && cpu.read16(part + HIT_FLAGS) & WAS_HIT_FLAG != 0 {
            let life = life.wrapping_sub(0x1_0000).wrapping_add(cpu.read32(part + TURRET_LIFE));
            cpu.write32(craft + TURRET_LIFE, life);
            if life as i32 <= 0 {
                let kind = cpu.read8(part + SPRITE_KIND) as i8 as i32 as u32;
                let points = cpu.read16(KIND_POINTS.wrapping_add(16 * kind)) as i16 as i32 as u32;
                let by = cpu.read32(part + HIT_BY);
                let (player, weapon) = (cpu.read8(by + HIT_PLAYER) as i8 as i32 as u32, cpu.read8(by + HIT_WEAPON) as i8 as i32 as u32);
                nested!(cpu, 0x0604_3108, hits::score_kill(cpu, player, weapon, part, points));
            }
        }
        cpu.write32(part + TURRET_LIFE, 0x1_0000);
        let flags = cpu.read32(part + FLAGS) & !NO_HITS;
        cpu.write32(part + FLAGS, flags);
        let flags = flags | cpu.read32(craft + FLAGS) & NO_HITS;
        cpu.write32(part + FLAGS, flags);
    }
    let x = cpu.read32(craft + POSITION);
    cpu.write32(part + POSITION, x);
    let y = cpu.read32(craft + POSITION + 4).wrapping_add(if upper { 0x68_0000 } else { 0xc8_0000 });
    cpu.write32(part + POSITION + 4, y);
    let z = cpu.read32(craft + POSITION + 8);
    cpu.write32(part + POSITION + 8, z);
    nested!(cpu, 0x0604_666c, parts::flash_with(cpu, part, craft));
}

/// A gun on the craft (+0x6c; numbered): placed by its offset (+0xac, +0xb0) on the craft, its base
/// direction (+0x8a, a word), its cooldown (+0x8c) and rate (+0x8e: frames between pairs, 8 each
/// less by difficulty).
const GUN_OFFSET: u32 = 0xac;
const GUN_BASE: u32 = 0x8a;
const GUN_COOLDOWN: u32 = 0x8c;
const GUN_RATE: u32 = 0x8e;
const GUN_REACH: u16 = 0x2000;
const GUN_TURN: u16 = 0x60;
const DIFFICULTY: u32 = 0x0600_ed84;
const PARTING_SHOT: u32 = 0x0605_6070;
/// The plain shot's description (bullets.rs's): its place, number, direction.
const SHOT_NUMBER: u32 = 0x1e;
const SHOT_ANGLE: u32 = 0x2c;
const GUN_MUZZLES: [u16; 2] = [0xec00, 0x1400];

/// Its behaviour. Shot down, or the craft sinking or gone: it ends (blown off while the craft
/// lives; else, at difficulty above 2, a parting shot). Else it turns (0x60 a frame, within 0x2000
/// of its base) to the nearer player while the craft is stopped, the gun within 96 dots up or down
/// of the centre and the player within 0x2800 of its base, else back to its base; on the craft;
/// hitting as it does; when it may fire and its cooldown is out, a pair of plain shots 18 dots out,
/// 0x1400 either side of its turn.
pub fn craft_gun_behaviour(cpu: &mut Cpu, obj: u32) {
    let craft = cpu.read32(obj + CRAFT);
    if (cpu.read32(obj + TURRET_LIFE) as i32) <= 0
        || cpu.read16(craft + SINKING) != 0
        || cpu.read8(craft + OBJ_MODE) as i8 > ENDING as i8
    {
        if cpu.read8(craft + OBJ_MODE) as i8 > ENDING as i8 {
            cpu.write8(obj + OBJ_MODE, TO_STOP);
            if cpu.read8(DIFFICULTY) as i8 > 2 {
                cpu.call(PARTING_SHOT, &[obj, 0]);
            }
        } else {
            nested!(cpu, 0x0605_9f50, ground::gun_blown_off(cpu, obj));
            cpu.write8(obj + OBJ_MODE, TO_STOP);
        }
        return;
    }
    let player = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, obj + FLAGS));
    let to_player = nested!(cpu, 0x0604_5e30, aim::angle_to(cpu, obj, player)) as u16;
    let base = cpu.read16(obj + GUN_BASE);
    let off = nested!(cpu, 0x0605_6580, aim::angle_between(cpu, to_player as u32, base as u32)) as i16;
    let stopped = cpu.read16(craft + ROUTE_FLAGS) & ROUTE_STOPPED != 0;
    let fires = stopped
        && cpu.read32(obj + POSITION + 4).wrapping_add(0x60_0000) <= 0xc0_0000
        && (off as u16).wrapping_add(0x2800) <= 0x5000;
    let to = if fires { to_player } else { cpu.read16(obj + GUN_BASE) };
    let turn = cpu.read16(obj + TURN_Z);
    let e = nested!(cpu, 0x0605_6580, aim::angle_between(cpu, to as u32, turn as u32)) as i16;
    if (e as i32).abs() <= 95 {
        cpu.write16(obj + TURN_Z, to);
    } else {
        let t = cpu.read16(obj + TURN_Z);
        cpu.write16(obj + TURN_Z, if e > 0 { t.wrapping_sub(GUN_TURN) } else { t.wrapping_add(GUN_TURN) });
    }
    let t = cpu.read16(obj + TURN_Z);
    let off = t.wrapping_sub(cpu.read16(obj + GUN_BASE)) as i16;
    if off > GUN_REACH as i16 {
        cpu.write16(obj + TURN_Z, t.wrapping_add(GUN_REACH).wrapping_sub(off as u16));
    }
    if off < GUN_REACH.wrapping_neg() as i16 {
        let t = cpu.read16(obj + TURN_Z);
        cpu.write16(obj + TURN_Z, t.wrapping_add(GUN_REACH.wrapping_neg()).wrapping_sub(off as u16));
    }
    for k in [0, 4] {
        let v = cpu.read32(craft + POSITION + k).wrapping_add(cpu.read32(obj + GUN_OFFSET + k));
        cpu.write32(obj + POSITION + k, v);
    }
    let z = cpu.read32(craft + POSITION + 8);
    cpu.write32(obj + POSITION + 8, z);
    let flags = cpu.read32(obj + FLAGS) & !NO_HITS;
    cpu.write32(obj + FLAGS, flags);
    let flags = flags | cpu.read32(craft + FLAGS) & NO_HITS;
    cpu.write32(obj + FLAGS, flags);
    let cooldown = cpu.read16(obj + GUN_COOLDOWN) as i16 as i32 as u32;
    let cooldown = nested!(cpu, 0x0604_6168, actors::approach(cpu, cooldown, 0, 1)) as u16;
    cpu.write16(obj + GUN_COOLDOWN, cooldown);
    if !fires || cooldown != 0 {
        return;
    }
    let rate = (cpu.read16(obj + GUN_RATE) as i16 as i32).wrapping_sub(cpu.read8(DIFFICULTY) as i8 as i32);
    cpu.write16(obj + GUN_COOLDOWN, (rate * 8 + 8) as u16);
    let shot = cpu.stack();
    let number = cpu.read8(obj + NUMBER);
    cpu.write8(shot + SHOT_NUMBER, number);
    let z = cpu.read32(obj + POSITION + 8);
    cpu.write32(shot + 8, z);
    let turn = cpu.read16(obj + TURN_Z);
    cpu.write16(shot + SHOT_ANGLE, turn);
    for muzzle in GUN_MUZZLES {
        let a = cpu.read16(obj + TURN_Z).wrapping_add(muzzle) as i16 as i32 as u32;
        let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
        let x = s.wrapping_mul(18).wrapping_add(cpu.read32(obj + POSITION));
        cpu.write32(shot, x);
        let a = cpu.read16(obj + TURN_Z).wrapping_add(muzzle) as i16 as i32 as u32;
        let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
        let y = c.wrapping_neg().wrapping_mul(18).wrapping_add(cpu.read32(obj + POSITION + 4));
        cpu.write32(shot + 4, y);
        nested!(cpu, 0x0605_3f48, bullets::bullet_set_plain(cpu, shot));
    }
}

/// A craft's gun made (numbered: list 2, 0xbc bytes, numbered_exec's; an enemy's number) from its
/// description `g` (its base direction +0x2c, offset +0x30, +0x32 in dots, rate +0x34; words) on
/// `craft`: kind 10, frame 2, the craft's type (coloured by it: the craft's colour, 16 past for
/// type 2), its boxes, life 2000, side 1. 1, or 0.
const CRAFT_GUN: u32 = 0x0608_8258;
const CRAFT_GUN_BOXES: u32 = 0x060a_0358;

pub fn craft_gun_set(cpu: &mut Cpu, g: u32, craft: u32) -> u32 {
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, ENEMY_NUMBERS)) as i8;
    if n == -1 {
        return 0;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0xbc)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, NUMBERED_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + BEAM_LIST));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + BEHAVIOUR, CRAFT_GUN);
    cpu.write8(o + NUMBER, n as u8);
    let n = cpu.read8(o + NUMBER) as i8 as i32;
    cpu.write32(PLAYERS.wrapping_add((4 * n) as u32), o);
    cpu.write32(o + CRAFT, craft);
    cpu.write8(o + SPRITE_KIND, 10);
    cpu.write8(o + SPRITE_FRAME, 2);
    let t = cpu.read8(craft + TYPE);
    cpu.write8(o + TYPE, t);
    match cpu.read8(o + TYPE) {
        0 => {
            let colour = cpu.read16(CRAFT_COLOUR);
            cpu.write16(o + OWN_COLOUR, colour);
        }
        2 => {
            let colour = cpu.read16(CRAFT_COLOUR).wrapping_add(0x10);
            cpu.write16(o + OWN_COLOUR, colour);
        }
        _ => {}
    }
    let colour = cpu.read16(o + OWN_COLOUR);
    cpu.write16(o + COLOUR, colour);
    cpu.write32(o + BEAM_LIST, CRAFT_GUN_BOXES);
    cpu.write32(o + TURRET_LIFE, 2000);
    cpu.write8(o + SIDE, 1);
    cpu.write8(o + BYTE_33, 0xf9);
    let base = cpu.read16(g + 0x2c);
    cpu.write16(o + TURN_Z, base);
    cpu.write16(o + GUN_BASE, base);
    for k in [0, 2] {
        let v = (cpu.read16(g + 0x30 + k) as i16 as i32 as u32) << 16;
        cpu.write32(o + GUN_OFFSET + 2 * k, v);
    }
    let rate = cpu.read16(g + 0x34);
    cpu.write16(o + GUN_RATE, rate);
    for k in [0, 4] {
        let v = cpu.read32(craft + POSITION + k).wrapping_add(cpu.read32(o + GUN_OFFSET + k));
        cpu.write32(o + POSITION + k, v);
    }
    1
}

/// The sinking craft made (numbered: list 2, 0xbc bytes; an enemy's number) from a ground
/// description `d` (the enemy rows' kind 19) with its route `script`: kind 10, frame 3, never
/// culled and no hits (as its parts), life 45000, its type (1 taken as 2; coloured 32 past the
/// craft's colour, 48 for type 2), side 1, its boxes, its direction kept at +0xa8, its side by the
/// mirror, its guns' rate (+0x7c, from the settings); its two parts (frames 1 and 4: +0x74, +0x78)
/// and eight guns (in pairs facing out either side, 20 dots out, from 68 dots on, 32 apart). It,
/// or 0.
const CRAFT_FLAGS: u32 = 0x2008_0000;
const CRAFT_LIFE: u32 = 45000;
const CRAFT_BEHAVIOUR: u32 = 0x0608_7d80;
const CRAFT_PART: u32 = 0x0608_800c;
const CRAFT_BOXES: u32 = 0x060a_0288;
const CRAFT_PART_BOXES: [u32; 2] = [0x060a_02d8, 0x060a_0328];
const CRAFT_RATE: u32 = 0x7c;
const CRAFT_GUN_SET: u32 = 0x0608_84c8;

/// A numbered enemy made for the craft (its sprite, boxes and fields set up, its number taken):
/// it, or 0.
fn craft_numbered(cpu: &mut Cpu, behaviour: u32) -> u32 {
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, ENEMY_NUMBERS)) as i8;
    if n == -1 {
        return 0;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0xbc)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, NUMBERED_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + BEAM_LIST));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + BEHAVIOUR, behaviour);
    cpu.write32(o + FLAGS, CRAFT_FLAGS);
    cpu.write8(o + NUMBER, n as u8);
    let n = cpu.read8(o + NUMBER) as i8 as i32;
    cpu.write32(PLAYERS.wrapping_add((4 * n) as u32), o);
    o
}

/// Its own colour by its type: `past` past the craft's colour for type 0, 16 more for type 2,
/// else left.
fn craft_colour(cpu: &mut Cpu, o: u32, past: u16) {
    match cpu.read8(o + TYPE) {
        0 => {
            let colour = cpu.read16(CRAFT_COLOUR).wrapping_add(past);
            cpu.write16(o + OWN_COLOUR, colour);
        }
        2 => {
            let colour = cpu.read16(CRAFT_COLOUR).wrapping_add(past + 16);
            cpu.write16(o + OWN_COLOUR, colour);
        }
        _ => {}
    }
}

pub fn craft_set(cpu: &mut Cpu, d: u32, script: u32) -> u32 {
    let craft = craft_numbered(cpu, CRAFT_BEHAVIOUR);
    if craft == 0 {
        return 0;
    }
    cpu.write8(craft + SPRITE_KIND, 10);
    cpu.write8(craft + SPRITE_FRAME, 3);
    cpu.write32(craft + TURRET_LIFE, CRAFT_LIFE);
    if cpu.read16(d + D_TYPE) == 1 {
        cpu.write16(d + D_TYPE, 2);
    }
    let t = cpu.read8(d + D_TYPE + 1);
    cpu.write8(craft + TYPE, t);
    craft_colour(cpu, craft, 0x20);
    let colour = cpu.read16(craft + OWN_COLOUR);
    cpu.write16(craft + COLOUR, colour);
    cpu.write8(craft + SIDE, 1);
    cpu.write32(craft + BEAM_LIST, CRAFT_BOXES);
    cpu.write32(craft + ROUTE_SCRIPT, script);
    let direction = cpu.read16(d + D_DIRECTION);
    cpu.write16(craft + FLIER_DIRECTION, direction);
    let side = cpu.read8(d + D_MIRROR + 3);
    cpu.write8(craft + ROUTE_SIDE, side);
    for k in [0, 4, 8] {
        let v = cpu.read32(d + D_PLACE + k);
        cpu.write32(craft + POSITION + k, v);
    }
    cpu.write8(craft + BYTE_33, 0xf8);
    let rate = cpu.read8(d + D_SETTINGS + 1);
    cpu.write8(craft + CRAFT_RATE, rate);
    for (i, (slot, boxes)) in CRAFT_PARTS.into_iter().zip(CRAFT_PART_BOXES).enumerate() {
        let part = craft_numbered(cpu, CRAFT_PART);
        if part == 0 {
            return craft;
        }
        cpu.write32(part + CRAFT, craft);
        cpu.write8(part + SPRITE_KIND, 10);
        let t = cpu.read8(craft + TYPE);
        cpu.write8(part + TYPE, t);
        cpu.write32(part + TURRET_LIFE, 0x1_0000);
        cpu.write8(part + SIDE, 1);
        for k in [0, 4, 8] {
            let v = cpu.read32(d + D_PLACE + k);
            cpu.write32(part + POSITION + k, v);
        }
        cpu.write8(part + BYTE_33, 0xf8);
        cpu.write8(part + SPRITE_FRAME, if i == 0 { 1 } else { 4 });
        cpu.write32(part + BEAM_LIST, boxes);
        craft_colour(cpu, part, if i == 0 { 0 } else { 0x20 });
        cpu.write32(craft + slot, part);
        let colour = cpu.read16(part + OWN_COLOUR);
        cpu.write16(part + COLOUR, colour);
    }
    let g = cpu.stack();
    let rate = (cpu.read8(craft + CRAFT_RATE) as i8 as u16) & 15;
    cpu.write16(g + 0x34, rate);
    cpu.write16(g + 0x32, 68);
    for _ in 0..4 {
        for (direction, x) in [(0xc000, 0xffec), (0x4000, 20)] {
            cpu.write16(g + 0x2c, direction);
            cpu.write16(g + 0x30, x);
            nested!(cpu, CRAFT_GUN_SET, craft_gun_set(cpu, g, craft));
        }
        let y = cpu.read16(g + 0x32).wrapping_add(32);
        cpu.write16(g + 0x32, y);
    }
    craft
}

// ---- the dropped bomb -----------------------------------------------------------------------------

/// A bomb dropped down the screen (an actor: kind 3, four frames, its fall's speed +0x84): it ends
/// as a bullet does (bullets.rs's bullet_ends, a spark turned back). It falls faster (by 1/4 to
/// 1.625 dots a frame), its y scale growing to 7/16 (1/16 a frame); every 4th frame an afterimage
/// (20 frames, a dot a frame), made with its flag 0x100 set; its x scale 1.25 and up to 1/16 more,
/// flipped at random.
const BOMB_FALL: u32 = 0x84;
const BOMB_AFTERIMAGE_FLAG: u32 = 0x100;

pub fn dropped_bomb_behaviour(cpu: &mut Cpu, o: u32) {
    if nested!(cpu, 0x0605_3b10, bullets::bullet_ends(cpu, o, 1)) as u8 != 0 {
        return;
    }
    let v = cpu.read32(o + BOMB_FALL);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0x1_a000, 0x4000));
    cpu.write32(o + BOMB_FALL, v);
    let y = cpu.read32(o + POSITION + 4).wrapping_add(v);
    cpu.write32(o + POSITION + 4, y);
    let sy = cpu.read32(o + SCALES + 4);
    let sy = nested!(cpu, 0x0604_6168, actors::approach(cpu, sy, 0x7000, 0x1000));
    cpu.write32(o + SCALES + 4, sy);
    let frame = cpu.read8(o + SPRITE_FRAME).wrapping_add(1);
    cpu.write8(o + SPRITE_FRAME, frame);
    if frame > 3 {
        cpu.write8(o + SPRITE_FRAME, 0);
        let flags = cpu.read32(o + FLAGS) | BOMB_AFTERIMAGE_FLAG;
        cpu.write32(o + FLAGS, flags);
        nested!(cpu, 0x0605_7634, explosions::afterimage_set(cpu, o, 0, 20, 0x1_0000));
        let flags = cpu.read32(o + FLAGS) & !BOMB_AFTERIMAGE_FLAG;
        cpu.write32(o + FLAGS, flags);
    }
    let sx = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1000)).wrapping_add(0x1_4000);
    cpu.write32(o + SCALES, sx);
    let flags = cpu.read32(o + FLAGS) & !FLIP_X;
    cpu.write32(o + FLAGS, flags);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    let flags = cpu.read32(o + FLAGS) | r & FLIP_X;
    cpu.write32(o + FLAGS, flags);
}

/// A bomb made (list 2, 0xbc bytes, actor_exec's) from the description `p` (a place and the
/// dropper's number, +0x1e): flags 0x420, its boxes, side 1, type 5, scales 1.3125 by 1/16, 64
/// colours past the first character; then from the same description a puff drifting up (4 dots a
/// frame, slowing) from a little nearer, 3/4 by 1/2, of one of two colours.
const BOMB_FLAGS: u32 = 0x420;
const BOMB_BOXES: u32 = 0x060a_0360;
const BOMB_BEHAVIOUR: u32 = 0x0608_898c;
const BOMB_PUFF: u32 = 0x0607_30d4;

pub fn dropped_bomb_set(cpu: &mut Cpu, p: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0xbc)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + BEAM_LIST));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    let n = cpu.read8(p + SHOT_NUMBER);
    cpu.write8(o + NUMBER, n);
    cpu.write32(o + BEHAVIOUR, BOMB_BEHAVIOUR);
    cpu.write32(o + FLAGS, BOMB_FLAGS);
    for k in [0, 4, 8] {
        let v = cpu.read32(p + k);
        cpu.write32(o + POSITION + k, v);
    }
    cpu.write8(o + BYTE_33, 8);
    cpu.write8(o + SPRITE_FRAME, 0);
    cpu.write8(o + SPRITE_KIND, 3);
    cpu.write32(o + BEAM_LIST, BOMB_BOXES);
    cpu.write8(o + SIDE, 1);
    cpu.write8(o + TYPE, 5);
    cpu.write32(o + SCALES, 0x1_5000);
    cpu.write32(o + SCALES + 4, 0x1000);
    let colour = cpu.read16(CHAR_BASE).wrapping_add(0x40);
    cpu.write16(o + OWN_COLOUR, colour);
    cpu.write16(o + COLOUR, colour);
    cpu.write32(p + actors::E_SCRIPT, BOMB_PUFF);
    cpu.write16(p + actors::E_ANGLE, 0);
    let z = cpu.read32(p + actors::E_POSITION + 8).wrapping_sub(100);
    cpu.write32(p + actors::E_POSITION + 8, z);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write16(p + actors::E_COLOUR, ((r & 0x2000) as u16).wrapping_add(0x80e0));
    cpu.write32(p + actors::E_SCALES, 0xc000);
    cpu.write32(p + actors::E_SCALES + 4, 0x8000);
    cpu.write32(p + actors::E_SPEED, 0xfffc_0000);
    let puff = nested!(cpu, 0x0605_7454, actors::drift_effect(cpu, p));
    if puff != 0 {
        cpu.write8(puff + BYTE_33, 8);
    }
}

// ---- the bomber -----------------------------------------------------------------------------------

/// A bomber (numbered, drawn by its own function as a kind 16 sprite): its step (+0x7c), flags
/// (+0x7e, a word: 1 hit this frame, 2 its flash shown), its speed (y +0x84, z +0xac), its bombs
/// dropped (+0xa8, a word), its frames dropping (+0x8a, a word), a count (+0xb0).
const BOMBER_STEP: u32 = 0x7c;
const BOMBER_FLAGS: u32 = 0x7e;
const BOMBER_SPEED_Y: u32 = 0x84;
const BOMBER_SPEED_Z: u32 = 0xac;
const BOMBER_DROPS: u32 = 0xa8;
const BOMBER_DROPPING: u32 = 0x8a;
const BOMBER_COUNT: u32 = 0xb0;
const BOMBER_HIT: u16 = 1;
const BOMBER_FLASHED: u16 = 2;
const BOMB_EVERY: u32 = 0x060a_0398;
const BOMB_SOUND: u32 = 41;
const BOMBER_POINTS: u32 = 100;
const DROPPED_BOMB_SET: u32 = 0x0608_8a58;

/// Its behaviour. Shot down: 100 points to its hitter, blown up as the wounded are, gone. 0: it
/// comes up (to depth 0, half a dot a frame), then hittable; 1: easing to 1/8 dot a frame up, it
/// drops bombs (by turns one or a pair 20 dots apart, sound 41) every so many frames by difficulty,
/// for 240 frames; 2: no hits, it climbs away faster and faster, and ends once 128 dots up.
pub fn bomber_behaviour(cpu: &mut Cpu, o: u32) {
    if cpu.read32(o + TURRET_LIFE) as i32 <= 0 {
        nested!(cpu, 0x0605_61ac, hits::score_hit_by(cpu, o, BOMBER_POINTS));
        nested!(cpu, 0x0605_9ee4, ground::blown_up_wounded(cpu, o));
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    match cpu.read8(o + BOMBER_STEP) {
        0 => {
            let z = cpu.read32(o + POSITION + 8);
            let z = nested!(cpu, 0x0604_6168, actors::approach(cpu, z, 0, 0x8000));
            cpu.write32(o + POSITION + 8, z);
            if z == 0 {
                let step = cpu.read8(o + BOMBER_STEP).wrapping_add(1);
                cpu.write8(o + BOMBER_STEP, step);
                let flags = cpu.read32(o + FLAGS) & !NO_HITS;
                cpu.write32(o + FLAGS, flags);
            }
        }
        1 => {
            let v = cpu.read32(o + BOMBER_SPEED_Y);
            let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0xffff_e000, 0x400));
            cpu.write32(o + BOMBER_SPEED_Y, v);
            let difficulty = cpu.read8(DIFFICULTY) as i8 as i32 as u32;
            let every = cpu.read32(BOMB_EVERY.wrapping_add(4 * difficulty));
            let count = cpu.read32(o + BOMBER_COUNT);
            let r = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, count, every));
            if r == 0 {
                bomber_drop(cpu, o, r);
            }
            let dropping = cpu.read16(o + BOMBER_DROPPING).wrapping_add(1);
            cpu.write16(o + BOMBER_DROPPING, dropping);
            if dropping as i16 > 240 {
                let step = cpu.read8(o + BOMBER_STEP).wrapping_add(1);
                cpu.write8(o + BOMBER_STEP, step);
                let flags = cpu.read32(o + FLAGS) | NO_HITS;
                cpu.write32(o + FLAGS, flags);
            }
        }
        2 => {
            let vz = cpu.read32(o + BOMBER_SPEED_Z).wrapping_add(0xffff_e800);
            cpu.write32(o + BOMBER_SPEED_Z, vz);
            let vy = cpu.read32(o + BOMBER_SPEED_Y).wrapping_add(0xffff_f000);
            cpu.write32(o + BOMBER_SPEED_Y, vy);
            if (cpu.read32(o + POSITION + 4) as i32) < 0xff80_0000u32 as i32 {
                cpu.write8(o + OBJ_MODE, TO_STOP);
            }
        }
        _ => {}
    }
    let y = cpu.read32(o + POSITION + 4).wrapping_add(cpu.read32(o + BOMBER_SPEED_Y));
    cpu.write32(o + POSITION + 4, y);
    let z = cpu.read32(o + POSITION + 8).wrapping_add(cpu.read32(o + BOMBER_SPEED_Z));
    cpu.write32(o + POSITION + 8, z);
    if cpu.read16(o + HIT_FLAGS) & WAS_HIT_FLAG != 0 {
        let flags = cpu.read16(o + BOMBER_FLAGS) | BOMBER_HIT;
        cpu.write16(o + BOMBER_FLAGS, flags);
    }
    let count = cpu.read32(o + BOMBER_COUNT).wrapping_add(1);
    cpu.write32(o + BOMBER_COUNT, count);
}

/// A drop: one bomb from it, or (every other) a pair 20 dots either side; the description on the
/// stack (its depth `z`, the remainder that was 0), each bomb's maker reusing it.
fn bomber_drop(cpu: &mut Cpu, o: u32, z: u32) {
    let drops = cpu.read16(o + BOMBER_DROPS).wrapping_add(1);
    cpu.write16(o + BOMBER_DROPS, drops);
    let p = cpu.stack();
    let number = cpu.read8(o + NUMBER);
    cpu.write8(p + SHOT_NUMBER, number);
    cpu.write32(p + 8, z);
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, BOMB_SOUND));
    if cpu.read16(o + BOMBER_DROPS) & 1 != 0 {
        let x = cpu.read32(o + POSITION);
        cpu.write32(p, x);
        let y = cpu.read32(o + POSITION + 4);
        cpu.write32(p + 4, y);
        nested!(cpu, DROPPED_BOMB_SET, dropped_bomb_set(cpu, p));
    }
    if cpu.read16(o + BOMBER_DROPS) & 1 == 0 {
        let x = cpu.read32(o + POSITION).wrapping_add(0xffec_0000);
        cpu.write32(p, x);
        let y = cpu.read32(o + POSITION + 4);
        cpu.write32(p + 4, y);
        nested!(cpu, DROPPED_BOMB_SET, dropped_bomb_set(cpu, p));
        let x = cpu.read32(o + POSITION).wrapping_add(0x14_0000);
        cpu.write32(p, x);
        nested!(cpu, DROPPED_BOMB_SET, dropped_bomb_set(cpu, p));
    }
}

/// Its draw (its sprite `s`, at +0x10): a kind 16 sprite at its place, lifted 9, of the kind's
/// colour, or flashing (224 past the first character) the frame after a hit.
const BOMBER_KIND: u8 = 16;
const BOMBER_COLOUR: u32 = 0x0607_42fc;

pub fn bomber_draw(cpu: &mut Cpu, s: u32) {
    let o = s.wrapping_sub(FLAGS);
    let flags = cpu.read16(o + BOMBER_FLAGS);
    if flags & BOMBER_FLASHED != 0 {
        cpu.write16(o + BOMBER_FLAGS, flags & !(BOMBER_HIT | BOMBER_FLASHED));
    }
    let flags = cpu.read16(o + BOMBER_FLAGS);
    if flags & BOMBER_HIT != 0 {
        cpu.write16(o + BOMBER_FLAGS, flags | BOMBER_FLASHED);
    }
    let d = cpu.stack();
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, d));
    cpu.write8(d + 0x1f, 0);
    cpu.write8(d + 0x1e, BOMBER_KIND);
    cpu.write8(d + 0x23, 9);
    for k in [0, 4, 8] {
        let v = cpu.read32(o + POSITION + k);
        cpu.write32(d + 4 + k, v);
    }
    let colour = if cpu.read16(o + BOMBER_FLAGS) & BOMBER_HIT != 0 {
        cpu.read16(CHAR_BASE).wrapping_add(0xe0)
    } else {
        cpu.read16(BOMBER_COLOUR)
    };
    cpu.write16(d + 0x18, colour);
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
}

/// A bomber made (numbered: list 2, 0xbc bytes) at the event `e`'s place (+4, +8) and 96 dots back,
/// coming up at half a dot a frame: drawn by its own function, its boxes, life 4800, side 1. 1.
const BOMBER_FLAGS_START: u32 = 0xc8_0000;
const BOMBER_BEHAVIOUR: u32 = 0x0608_8b94;
const BOMBER_DRAW: u32 = 0x0608_8d7c;
const BOMBER_BOXES: u32 = 0x060a_0368;
const BOMBER_LIFE: u32 = 4800;

pub fn bomber_set(cpu: &mut Cpu, e: u32) -> u32 {
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, ENEMY_NUMBERS)) as i8;
    if n == -1 {
        return 1;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0xbc)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, NUMBERED_EXEC));
    if o == 0 {
        return 1;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + BEAM_LIST));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + BEHAVIOUR, BOMBER_BEHAVIOUR);
    cpu.write32(o + FLAGS, BOMBER_FLAGS_START);
    cpu.write32(o + MODEL, BOMBER_DRAW);
    cpu.write8(o + SPRITE_KIND, BOMBER_KIND);
    cpu.write8(o + NUMBER, n as u8);
    let n = cpu.read8(o + NUMBER) as i8 as i32;
    cpu.write32(PLAYERS.wrapping_add((4 * n) as u32), o);
    cpu.write32(o + BEAM_LIST, BOMBER_BOXES);
    cpu.write32(o + TURRET_LIFE, BOMBER_LIFE);
    cpu.write8(o + SIDE, 1);
    cpu.write8(o + TYPE, 0);
    let x = cpu.read32(e + 4);
    cpu.write32(o + POSITION, x);
    let y = cpu.read32(e + 8);
    cpu.write32(o + POSITION + 4, y);
    cpu.write32(o + POSITION + 8, 0xffa0_0000);
    cpu.write32(o + BOMBER_SPEED_Y, 0x8000);
    1
}

/// Whether the nearer player is within 24 dots (both ways) of the point 28 dots out along its turn.
pub fn player_ahead(cpu: &mut Cpu, o: u32) -> u32 {
    let player = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + FLAGS));
    let turn = cpu.read16(o + TURN_Z) as i16 as i32 as u32;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, turn));
    let x = s.wrapping_mul(28).wrapping_add(cpu.read32(o + POSITION));
    let turn = cpu.read16(o + TURN_Z) as i16 as i32 as u32;
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, turn));
    let y = c.wrapping_mul(0xffff_ffe4).wrapping_add(cpu.read32(o + POSITION + 4));
    let dx = (cpu.read32(player + POSITION).wrapping_sub(x) as i32).wrapping_abs();
    if dx > 0x17_ffff {
        return 0;
    }
    let dy = (cpu.read32(player + POSITION + 4).wrapping_sub(y) as i32).wrapping_abs();
    (dy <= 0x17_ffff) as u32
}

// ---- the rail claw --------------------------------------------------------------------------------

/// A claw on a rail (numbered; along x, or along y if its turn has 0x4000): its step (+0x7c), flags
/// (+0x7e, a word: 1 hit), its speed along the rail (x +0x80 or y +0x84), the rail's ends (+0xac,
/// +0xb0; a y rail's moving with the ground), how far it has gone (+0xb4: its frame, a 4-dot step
/// of 4), its jaw's turn (+0x8c, a word) and speed (+0xa6, a word), its hold (+0x88), its jaw's
/// boxes (six at +0xbc, in pairs 8, 16 and 24 dots along the jaw from +0x92, +0x94).
const CLAW_STEP: u32 = 0x7c;
const CLAW_FLAGS: u32 = 0x7e;
const CLAW_SPEED: u32 = 0x80;
const CLAW_ENDS: u32 = 0xac;
const CLAW_GONE: u32 = 0xb4;
const JAW: u32 = 0x8c;
const JAW_SPEED: u32 = 0xa6;
const CLAW_HOLD: u32 = 0x88;
const JAW_BOXES: u32 = 0xbc;
const JAW_FROM: u32 = 0x92;
const ALONG_Y: u16 = 0x4000;
const CLAW_SNAP_SOUND: u32 = 60;
const CLAW_POINTS: u32 = 100;
const CLAW_GONE_BELOW: i32 = 0xb0_0000;

/// Its behaviour. Below 176 dots it ends; shot down, 100 points to its hitter, blown up, gone.
/// Flipped every frame. 0: it slides along its rail toward the nearer player (to 1.25 dots a frame,
/// by 0x1200), stopping at the ends; once the player is close ahead (player_ahead), 1: it snaps its
/// jaw shut (faster and faster, from 0x600 a frame, sound 60 as it closes at a quarter back), 2:
/// held 15 frames, 3: it opens (0x400 a frame), and snaps again if the player is still there.
pub fn rail_claw_behaviour(cpu: &mut Cpu, o: u32) {
    if cpu.read32(o + POSITION + 4) as i32 > CLAW_GONE_BELOW {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    if cpu.read32(o + TURRET_LIFE) as i32 <= 0 {
        nested!(cpu, 0x0605_61ac, hits::score_hit_by(cpu, o, CLAW_POINTS));
        nested!(cpu, 0x0605_9e9c, ground::blown_up_1(cpu, o));
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    let flags = cpu.read32(o + FLAGS) ^ FLIP_X;
    cpu.write32(o + FLAGS, flags);
    let boxes = o + JAW_BOXES;
    for i in 1..=3u32 {
        let a = cpu.read16(o + JAW).wrapping_add(0x4000) as i16 as i32 as u32;
        let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a)) as i32;
        let x = (s.wrapping_mul(i as i32) / 0x2000) as u16;
        let x = x.wrapping_add(cpu.read16(o + JAW_FROM));
        let a = cpu.read16(o + JAW).wrapping_add(0x4000) as i16 as i32 as u32;
        let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a)) as i32;
        let y = (c.wrapping_mul(i as i32).wrapping_mul(8).wrapping_neg() / 0x1_0000) as u16;
        let y = y.wrapping_add(cpu.read16(o + JAW_FROM + 2));
        for b in [boxes + 16 * (i - 1), boxes + 16 * (i - 1) + 8] {
            cpu.write16(b + BOX_X, x);
            cpu.write16(b + BOX_Y, y);
        }
    }
    if cpu.read16(o + HIT_FLAGS) & WAS_HIT_FLAG != 0 {
        let flags = cpu.read16(o + CLAW_FLAGS) | 1;
        cpu.write16(o + CLAW_FLAGS, flags);
    }
    let mut toward = 0;
    match cpu.read8(o + CLAW_STEP) {
        1 => {
            let v = cpu.read16(o + JAW_SPEED) as i16 as i32 as u32;
            let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0xffff_f000, 0xa0)) as u16;
            cpu.write16(o + JAW_SPEED, v);
            let jaw = cpu.read16(o + JAW).wrapping_add(v);
            cpu.write16(o + JAW, jaw);
            if (jaw as i16) < 0xc000u16 as i16 {
                let step = cpu.read8(o + CLAW_STEP).wrapping_add(1);
                cpu.write8(o + CLAW_STEP, step);
                cpu.write8(o + CLAW_HOLD, 15);
                cpu.write16(o + JAW, 0xc000);
                nested!(cpu, 0x0600_50d4, sound::se_set(cpu, CLAW_SNAP_SOUND));
            }
        }
        2 => {
            let hold = cpu.read8(o + CLAW_HOLD).wrapping_sub(1);
            cpu.write8(o + CLAW_HOLD, hold);
            if (hold as i8) < 0 {
                let step = cpu.read8(o + CLAW_STEP).wrapping_add(1);
                cpu.write8(o + CLAW_STEP, step);
            }
        }
        3 => {
            let jaw = cpu.read16(o + JAW).wrapping_add(0x400);
            cpu.write16(o + JAW, jaw);
            if jaw as i16 >= 0 {
                cpu.write16(o + JAW, 0);
                if nested!(cpu, 0x0608_8f14, player_ahead(cpu, o)) as u8 != 0 {
                    cpu.write8(o + CLAW_STEP, 1);
                    cpu.write16(o + JAW_SPEED, 0x600);
                } else {
                    cpu.write8(o + CLAW_STEP, 0);
                }
            }
        }
        _ => {
            let player = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + FLAGS));
            let k = if cpu.read16(o + TURN_Z) & ALONG_Y != 0 { 4 } else { 0 };
            let (mine, theirs) = (cpu.read32(o + POSITION + k) as i32, cpu.read32(player + POSITION + k) as i32);
            toward = if mine > theirs { 0xfffe_c000 } else { 0x1_4000 };
            if nested!(cpu, 0x0608_8f14, player_ahead(cpu, o)) as u8 != 0 {
                cpu.write8(o + CLAW_STEP, 1);
                toward = 0;
                cpu.write16(o + JAW_SPEED, 0x600);
            }
        }
    }
    let along_y = cpu.read16(o + TURN_Z) & ALONG_Y != 0;
    let k = if along_y { 4 } else { 0 };
    let v = cpu.read32(o + CLAW_SPEED + k);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, toward, 0x1200));
    cpu.write32(o + CLAW_SPEED + k, v);
    let p = cpu.read32(o + POSITION + k).wrapping_add(v);
    cpu.write32(o + POSITION + k, p);
    if along_y {
        let at = cpu.read32(GROUND_DY_AT);
        let dy = cpu.read32(at);
        for end in [0, 4] {
            let e = cpu.read32(o + CLAW_ENDS + end).wrapping_sub(dy);
            cpu.write32(o + CLAW_ENDS + end, e);
        }
    }
    if (cpu.read32(o + POSITION + k) as i32) < cpu.read32(o + CLAW_ENDS) as i32 {
        let end = cpu.read32(o + CLAW_ENDS);
        cpu.write32(o + POSITION + k, end);
        cpu.write32(o + CLAW_SPEED + k, 0);
    }
    if (cpu.read32(o + POSITION + k) as i32) > cpu.read32(o + CLAW_ENDS + 4) as i32 {
        let end = cpu.read32(o + CLAW_ENDS + 4);
        cpu.write32(o + POSITION + k, end);
        cpu.write32(o + CLAW_SPEED + k, 0);
    }
    let gone = cpu.read32(o + CLAW_GONE).wrapping_add(cpu.read32(o + CLAW_SPEED + k));
    cpu.write32(o + CLAW_GONE, gone);
    if !along_y {
        let frame = (cpu.read32(o + POSITION).wrapping_neg() as i32 / 0x4_0000) as u8 & 3;
        cpu.write8(o + SPRITE_FRAME, frame);
    }
    let frame = (cpu.read32(o + CLAW_GONE).wrapping_neg() as i32 / 0x4_0000) as u8 & 3;
    cpu.write8(o + SPRITE_FRAME, frame);
    let at = cpu.read32(GROUND_DY_AT);
    let dy = cpu.read32(at);
    let y = cpu.read32(o + POSITION + 4).wrapping_sub(dy);
    cpu.write32(o + POSITION + 4, y);
}

/// The claw's draw (its sprite `s`, at +0x10): its two jaws (frames 4 and 5, 6 dots out at an
/// eighth of a turn either side, turned by the jaw's turn either way) and its body (its own frame),
/// lifted 8 toward the eye, its colour or flashing the frame after a hit.
const SPRITE_STRUCT_KIND: u32 = 0x1e;
const SPRITE_STRUCT_FRAME: u32 = 0x1f;
const SPRITE_STRUCT_COLOUR: u32 = 0x18;
const SPRITE_STRUCT_ANGLE: u32 = 0x20;
const SPRITE_STRUCT_LIFT: u32 = 0x23;

pub fn rail_claw_draw(cpu: &mut Cpu, s: u32) {
    let o = s.wrapping_sub(FLAGS);
    let flags = cpu.read16(o + CLAW_FLAGS);
    if flags & 2 != 0 {
        cpu.write16(o + CLAW_FLAGS, flags & !3);
    }
    let flags = cpu.read16(o + CLAW_FLAGS);
    if flags & 1 != 0 {
        cpu.write16(o + CLAW_FLAGS, flags | 2);
    }
    let d = cpu.stack();
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, d));
    let kind = cpu.read8(o + SPRITE_KIND);
    cpu.write8(d + SPRITE_STRUCT_KIND, kind);
    cpu.write8(d + SPRITE_STRUCT_LIFT, 0xf8);
    let z = cpu.read32(o + POSITION + 8);
    cpu.write32(d + 4 + 8, z);
    let colour = if cpu.read16(o + CLAW_FLAGS) & 1 != 0 {
        cpu.read16(CHAR_BASE).wrapping_add(0xe0)
    } else {
        cpu.read16(o + COLOUR)
    };
    cpu.write16(d + SPRITE_STRUCT_COLOUR, colour);
    for (frame, side, opens) in [(4u8, 0x2000u16, true), (5, 0xe000, false)] {
        cpu.write8(d + SPRITE_STRUCT_FRAME, frame);
        let a = cpu.read16(o + TURN_Z).wrapping_add(side) as i16 as i32 as u32;
        let sn = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
        let x = cpu.read32(o + POSITION).wrapping_add(sn.wrapping_mul(6));
        cpu.write32(d + 4, x);
        let a = cpu.read16(o + TURN_Z).wrapping_add(side) as i16 as i32 as u32;
        let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
        let y = cpu.read32(o + POSITION + 4).wrapping_add(c.wrapping_mul(0xffff_fffa));
        cpu.write32(d + 4 + 4, y);
        let (turn, jaw) = (cpu.read16(o + TURN_Z), cpu.read16(o + JAW));
        cpu.write16(d + SPRITE_STRUCT_ANGLE, if opens { turn.wrapping_add(jaw) } else { turn.wrapping_sub(jaw) });
        nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
    }
    let frame = cpu.read8(o + SPRITE_FRAME);
    cpu.write8(d + SPRITE_STRUCT_FRAME, frame);
    for k in [0, 4] {
        let v = cpu.read32(o + POSITION + k);
        cpu.write32(d + 4 + k, v);
    }
    let turn = cpu.read16(o + TURN_Z);
    cpu.write16(d + SPRITE_STRUCT_ANGLE, turn);
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
}

/// A claw made (numbered: list 2, 0xfc bytes) from the event `e` (an enemy row's: its place, type
/// +0x1b, direction +0x14 (its rail along y with 0x4000), the rail's ends +0x1c and +0x1e in dots
/// from the place): kind 0x49, coloured by its type, life 3000, side 1, its own boxes (copied from
/// 0x060a03ac), its jaw from 6 dots out at an eighth of a turn. 1.
const CLAW_SIZE: u32 = 0xfc;
const CLAW_FLAGS_START: u32 = 0xc0_0000;
const CLAW_BEHAVIOUR: u32 = 0x0608_8f94;
const CLAW_DRAW: u32 = 0x0608_92e4;
const CLAW_KIND: u8 = 0x49;
const CLAW_COLOUR: u32 = 0x0607_468c;
const CLAW_LIFE: u32 = 3000;
const CLAW_BOXES: u32 = 0x060a_03ac;
const CLAW_START_X: u32 = 0x8a;

pub fn rail_claw_set(cpu: &mut Cpu, e: u32) -> u32 {
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, ENEMY_NUMBERS)) as i8;
    if n == -1 {
        return 1;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, CLAW_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, NUMBERED_EXEC));
    if o == 0 {
        return 1;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + BEAM_LIST));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + BEHAVIOUR, CLAW_BEHAVIOUR);
    cpu.write32(o + FLAGS, CLAW_FLAGS_START);
    cpu.write32(o + MODEL, CLAW_DRAW);
    cpu.write8(o + SPRITE_KIND, CLAW_KIND);
    cpu.write8(o + SPRITE_FRAME, 0);
    let t = cpu.read8(e + E_TYPE) as u16;
    let colour = (t << 4).wrapping_add(cpu.read16(CLAW_COLOUR));
    cpu.write16(o + COLOUR, colour);
    cpu.write8(o + NUMBER, n as u8);
    let n = cpu.read8(o + NUMBER) as i8 as i32;
    cpu.write32(PLAYERS.wrapping_add((4 * n) as u32), o);
    cpu.write32(o + TURRET_LIFE, CLAW_LIFE);
    cpu.write8(o + SIDE, 1);
    let t = cpu.read8(e + E_TYPE);
    cpu.write8(o + TYPE, t);
    let boxes = o + JAW_BOXES;
    nested!(cpu, 0x0605_6440, hits::boxes_copy(cpu, boxes, CLAW_BOXES));
    cpu.write32(o + BEAM_LIST, boxes);
    for k in [0, 1] {
        let v = (cpu.read16(e + E_PLACE + 2 * k) as i16 as i32 as u32) << 16;
        cpu.write32(o + POSITION + 4 * k, v);
    }
    cpu.write32(o + POSITION + 8, 0);
    let x = cpu.read16(e + E_PLACE);
    cpu.write16(o + CLAW_START_X, x);
    let direction = cpu.read16(e + E_DIRECTION);
    cpu.write16(o + TURN_Z, direction);
    let k = if direction & ALONG_Y != 0 { 4 } else { 0 };
    for (end, at) in [(0, E_1C), (4, E_1E)] {
        let off = (cpu.read16(e + at) as i16 as i32 as u32) << 16;
        let v = cpu.read32(o + POSITION + k).wrapping_add(off);
        cpu.write32(o + CLAW_ENDS + end, v);
    }
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, 0x2000)) as i32;
    cpu.write16(o + JAW_FROM, (s.wrapping_mul(6) / 0x1_0000) as u16);
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, 0x2000)) as i32;
    cpu.write16(o + JAW_FROM + 2, (c.wrapping_mul(-6) / 0x1_0000) as u16);
    1
}

// ---- the ring sparks ------------------------------------------------------------------------------

/// A spark spiralling out from a centre (a plain object; its centre +0x3c): its x scale shrinking
/// (1/256 a frame) to nothing, its distance (its y scale +0x24, 32 dots a unit) growing by its speed
/// (+0x44) that grows by +0x48, its turn by its spin (+0x40, a word); its frame by the frame (0xd3
/// on, of 4). It ends as it vanishes or with its centre.
const SPARK_CENTRE: u32 = 0x3c;
const SPARK_SPIN: u32 = 0x40;
const SPARK_SPEED: u32 = 0x44;
const SPARK_ACCEL: u32 = 0x48;
const GAME_FRAMES_LOW: u32 = 0x0600_ebd7;

pub fn ring_spark_behaviour(cpu: &mut Cpu, o: u32) {
    let centre = cpu.read32(o + SPARK_CENTRE);
    let s = cpu.read32(o + SCALES);
    let s = nested!(cpu, 0x0604_6168, actors::approach(cpu, s, 0, 0x80));
    cpu.write32(o + SCALES, s);
    if s == 0 || cpu.read8(centre + OBJ_MODE) as i8 > ENDING as i8 {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    let r = cpu.read32(o + SCALES + 4).wrapping_add(cpu.read32(o + SPARK_SPEED));
    cpu.write32(o + SCALES + 4, r);
    let v = cpu.read32(o + SPARK_SPEED).wrapping_add(cpu.read32(o + SPARK_ACCEL));
    cpu.write32(o + SPARK_SPEED, v);
    let turn = cpu.read16(o + TURN_Z).wrapping_add(cpu.read16(o + SPARK_SPIN));
    cpu.write16(o + TURN_Z, turn);
    let reach = cpu.read32(o + SCALES + 4) << 5;
    ring_place(cpu, o, centre, reach);
    let frame = (cpu.read8(GAME_FRAMES_LOW) & 3).wrapping_sub(45);
    cpu.write8(o + SPRITE_FRAME, frame);
}

/// It placed at `reach` (16.16 times the unit) from its centre along its turn, at the centre's
/// depth.
fn ring_place(cpu: &mut Cpu, o: u32, centre: u32, reach: u32) {
    let turn = cpu.read16(o + TURN_Z) as i16 as i32 as u32;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, turn));
    let dx = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, reach));
    let x = dx.wrapping_add(cpu.read32(centre + POSITION));
    cpu.write32(o + POSITION, x);
    let turn = cpu.read16(o + TURN_Z) as i16 as i32 as u32;
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, turn));
    let dy = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, c.wrapping_neg(), reach));
    let y = dy.wrapping_add(cpu.read32(centre + POSITION + 4));
    cpu.write32(o + POSITION + 4, y);
    let z = cpu.read32(centre + POSITION + 8);
    cpu.write32(o + POSITION + 8, z);
}

/// A ring spark made (list 3, 0x4c bytes, plain) from the description `p` (a place; its centre
/// +0x3c, its speed +0x2c): frame 0xd6, at the place, lifted 1 away, spinning up to 0x200 either
/// way at random, at a random turn, 1/32 wide, its speed a quarter of the given, slowing by a 256th
/// of it a frame.
const RING_SPARK: u32 = 0x0608_9628;
const RING_FLARE: u32 = 0x0608_97f4;

pub fn ring_spark_set(cpu: &mut Cpu, p: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0x4c)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 3, blocks, PLAIN_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + FLAGS));
    cpu.write32(o + BEHAVIOUR, RING_SPARK);
    cpu.write8(o + SPRITE_FRAME, 0xd6);
    cpu.write8(o + SPRITE_KIND, 0);
    let colour = cpu.read16(CHAR_BASE);
    cpu.write16(o + OWN_COLOUR, colour);
    cpu.write16(o + COLOUR, colour);
    cpu.write8(o + BYTE_33, 0xff);
    for k in [0, 4, 8] {
        let v = cpu.read32(p + k);
        cpu.write32(o + POSITION + k, v);
    }
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000)) as u16;
    cpu.write16(o + SPARK_SPIN, 0x200u16.wrapping_sub(r & 0x3ff));
    let centre = cpu.read32(p + SPARK_CENTRE);
    cpu.write32(o + SPARK_CENTRE, centre);
    let turn = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write16(o + TURN_Z, turn as u16);
    cpu.write32(o + SCALES, 0x800);
    cpu.write32(o + SCALES + 4, 0);
    let speed = cpu.read32(p + 0x2c) as i32;
    cpu.write32(o + SPARK_SPEED, (speed / 4) as u32);
    cpu.write32(o + SPARK_ACCEL, (speed.wrapping_neg() / 0x100) as u32);
}

/// A flare circling a centre (a plain object; its centre +0x3c): 56 dots a unit of the centre's y
/// scale out along its turn; its frame counting down every other frame, ending below 22 or with
/// its centre.
pub fn ring_flare_behaviour(cpu: &mut Cpu, o: u32) {
    let centre = cpu.read32(o + SPARK_CENTRE);
    if cpu.read32(GAME_FRAMES) & 1 != 0 {
        let frame = cpu.read8(o + SPRITE_FRAME).wrapping_sub(1);
        cpu.write8(o + SPRITE_FRAME, frame);
    }
    if cpu.read8(o + SPRITE_FRAME) <= 21 || cpu.read8(centre + OBJ_MODE) as i8 > ENDING as i8 {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    let reach = cpu.read32(centre + SCALES + 4).wrapping_mul(56);
    ring_place(cpu, o, centre, reach);
}

/// A ring flare made (list 3, 0x4c bytes, plain) from the description `p` (a place; its centre
/// +0x3c): kind 0x27 (its colour), frame 0x1c, at a random turn, its scales 1 less up to 3/16 by
/// 1 more up to 3/2, at random.
const FLARE_KIND: u8 = 0x27;
const FLARE_COLOUR: u32 = 0x0607_446c;

pub fn ring_flare_set(cpu: &mut Cpu, p: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0x4c)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 3, blocks, PLAIN_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + FLAGS));
    cpu.write32(o + BEHAVIOUR, RING_FLARE);
    cpu.write8(o + SPRITE_KIND, FLARE_KIND);
    cpu.write8(o + SPRITE_FRAME, 0x1c);
    let colour = cpu.read16(FLARE_COLOUR);
    cpu.write16(o + OWN_COLOUR, colour);
    cpu.write16(o + COLOUR, colour);
    for k in [0, 4, 8] {
        let v = cpu.read32(p + k);
        cpu.write32(o + POSITION + k, v);
    }
    cpu.write8(o + BYTE_33, 0);
    let centre = cpu.read32(p + SPARK_CENTRE);
    cpu.write32(o + SPARK_CENTRE, centre);
    let turn = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write16(o + TURN_Z, turn as u16);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write32(o + SCALES, 0x1_0000u32.wrapping_sub((r & 3) << 12));
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write32(o + SCALES + 4, ((r & 3) << 15).wrapping_add(0x1_0000));
}

/// An effect (effects.rs's) kept at the place +0x5c points to, 176 dots nearer.
const NEARER: u32 = 0xff50_0000;

pub fn nearer_effect_behaviour(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0604_deb4, effects::effect_exec(cpu, o));
    let place = cpu.read32(o + GLOW_PARENT);
    for k in [0, 4] {
        let v = cpu.read32(place + k);
        cpu.write32(o + POSITION + k, v);
    }
    let z = cpu.read32(place + 8).wrapping_add(NEARER);
    cpu.write32(o + POSITION + 8, z);
}

// ---- the gunship's guns ---------------------------------------------------------------------------

/// A muzzle flash on `o`'s gun at `at` (a place in `o`): an effect (flags 3, relative: its offset
/// and scales whatever the stack held, as the game leaves them), turned back from `o`, its link
/// 0x060a0800, kept 176 dots nearer than the gun (nearer_effect_behaviour).
const FLASH_LINK: u32 = 0x060a_0800;
const NEARER_EFFECT: u32 = 0x0608_996c;

pub fn muzzle_flash(cpu: &mut Cpu, o: u32, at: u32) {
    let fx = cpu.stack();
    cpu.write32(fx + effects::FLAGS_LONG, 3);
    let angle = cpu.read16(o + TURN_Z).wrapping_add(0x8000);
    cpu.write16(fx + effects::ANGLE, angle);
    cpu.write16(fx + effects::FRAMES, 0);
    cpu.write8(fx + effects::FRAMES_2, 0);
    cpu.write8(fx + effects::KIND, 0);
    cpu.write8(fx + effects::FRAME, 0);
    let colour = cpu.read16(CHAR_BASE);
    cpu.write16(fx + effects::COLOUR, colour);
    let number = cpu.read8(o + NUMBER);
    cpu.write8(fx + effects::PLAYER, number);
    cpu.write32(fx + effects::LINK, FLASH_LINK);
    cpu.write32(fx + effects::RELATIVE_3C, 0);
    let e = nested!(cpu, 0x0604_e1cc, effects::effect_set(cpu, fx));
    if e != 0 {
        cpu.write32(e + BEHAVIOUR, NEARER_EFFECT);
        cpu.write32(e + GLOW_PARENT, at);
    }
}

/// A gunship (a big model enemy): its gun places (the spray gun at +0x4f0; the twin guns at +0x514
/// and +0x520), its parts' hit flags (words at +0x10a, +0x10e, +0x116: 0x40 shot off), a count
/// (+0xe4, a word), its recoil (+0xfc, a word).
const SPRAY_GUN: u32 = 0x4f0;
const TWIN_GUNS: [u32; 2] = [0x514, 0x520];
const SHIP_PART_A: u32 = 0x10a;
const SHIP_PART_B: u32 = 0x10e;
const SHIP_PART_C: u32 = 0x116;
const PART_OFF: u16 = 0x40;
const SHIP_COUNT: u32 = 0xe4;
const SHIP_RECOIL: u32 = 0xfc;
const SPRAY_SPREADS: u32 = 0x060a_07e6;
const SPRAY_GUN_SOUND: u32 = 28;
const TWIN_SOUND: u32 = 24;
/// The bullets' description (bullets.rs's): place, number, direction, a word after it, speed.
const SHOT_SPEED: u32 = 0x30;
const MUZZLE_FLASH: u32 = 0x0608_99a4;

/// The spray gun: while its part is on and the ship's tilt (x turn) within a quarter of 0x6000,
/// every 18th frame 12 bullets straight back spread by difficulty (in 0x800 steps) at 1.375 dots
/// a frame and up to half a dot more, with its recoil and a flash.
pub fn spray_gun(cpu: &mut Cpu, o: u32) {
    if cpu.read16(o + SHIP_PART_A) & PART_OFF != 0 {
        return;
    }
    let count = cpu.read16(o + SHIP_COUNT) as u32;
    if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, count, 18)) != 0 {
        return;
    }
    if cpu.read16(o + TURN_X).wrapping_add(0xdfff) > 0x3ffe {
        return;
    }
    cpu.write16(o + SHIP_RECOIL, 0xfd00);
    let difficulty = cpu.read8(DIFFICULTY) as i8 as i32 as u32;
    let spread = cpu.read16(SPRAY_SPREADS.wrapping_add(2 * difficulty));
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, SPRAY_GUN_SOUND));
    let shot = cpu.stack();
    for _ in 0..12 {
        let number = cpu.read8(o + NUMBER);
        cpu.write8(shot + SHOT_NUMBER, number);
        for k in [0, 4] {
            let v = cpu.read32(o + SPRAY_GUN + k);
            cpu.write32(shot + k, v);
        }
        cpu.write32(shot + 8, 0);
        cpu.write16(shot + SHOT_ANGLE + 2, 0);
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000)) as u16;
        let off = spread.wrapping_sub(r & spread.wrapping_mul(2).wrapping_sub(1)) & 0xf800;
        let angle = off.wrapping_add(cpu.read16(o + TURN_Z)).wrapping_add(0x8000);
        cpu.write16(shot + SHOT_ANGLE, angle);
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
        cpu.write32(shot + SHOT_SPEED, (r & 0x7fff).wrapping_add(0x1_6000));
        nested!(cpu, 0x0605_3db0, bullets::bullet_set(cpu, shot));
    }
    nested!(cpu, MUZZLE_FLASH, muzzle_flash(cpu, o, o + SPRAY_GUN));
}

/// The twin guns: while the ship's tilt is within 0x2000 of 0x3001 on: with either of two parts
/// off, every 10th frame a plain rocket from each straight back (sound 24) and their flashes; with
/// two others off, every 10th frame 8 pairs of plain bullets, back-left and back at random up to
/// 0x3800 aside.
pub fn twin_guns(cpu: &mut Cpu, o: u32) {
    if cpu.read16(o + TURN_X).wrapping_add(0xcfff) > 0x1ffe {
        return;
    }
    let shot = cpu.stack();
    let number = cpu.read8(o + NUMBER);
    cpu.write8(shot + SHOT_NUMBER, number);
    cpu.write32(shot + 8, 0);
    if cpu.read16(o + SHIP_PART_B) & PART_OFF != 0 || cpu.read16(o + SHIP_PART_A) & PART_OFF != 0 {
        let count = cpu.read16(o + SHIP_COUNT) as u32;
        if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, count, 10)) == 0 {
            nested!(cpu, 0x0600_50d4, sound::se_set(cpu, TWIN_SOUND));
            let angle = cpu.read16(o + TURN_Z).wrapping_add(0x8000);
            cpu.write16(shot + SHOT_ANGLE, angle);
            for gun in TWIN_GUNS {
                for k in [0, 4] {
                    let v = cpu.read32(o + gun + k);
                    cpu.write32(shot + k, v);
                }
                nested!(cpu, 0x0605_5d84, bullets::rocket_set_plain(cpu, shot));
            }
            for gun in TWIN_GUNS {
                nested!(cpu, MUZZLE_FLASH, muzzle_flash(cpu, o, o + gun));
            }
        }
    }
    if cpu.read16(o + SHIP_PART_A) & PART_OFF == 0 || cpu.read16(o + SHIP_PART_C) & PART_OFF == 0 {
        return;
    }
    let count = cpu.read16(o + SHIP_COUNT) as u32;
    if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, count, 10)) != 0 {
        return;
    }
    for _ in 0..8 {
        for (gun, back) in TWIN_GUNS.into_iter().zip([0xc000u16, 0x8000]) {
            let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000)) as u16;
            let angle = cpu.read16(o + TURN_Z).wrapping_add(back).wrapping_sub(r & 0x3800);
            cpu.write16(shot + SHOT_ANGLE, angle);
            for k in [0, 4] {
                let v = cpu.read32(o + gun + k);
                cpu.write32(shot + k, v);
            }
            nested!(cpu, 0x0605_3f48, bullets::bullet_set_plain(cpu, shot));
        }
    }
}

// ---- the homing rocket ----------------------------------------------------------------------------

/// A homing rocket (an actor): its speed (x +0x80, y +0x84), its knock (+0xb4, +0xb8: easing off by
/// the words +0xa2, +0xa4 a frame), its homing frames left (+0x9e, a word), its frames before it
/// thrusts (+0x8a, a word), its thrust (+0xac).
const ROCKET_SPEED: u32 = 0x80;
const ROCKET_KNOCK: u32 = 0xb4;
const ROCKET_KNOCK_EASE: u32 = 0xa2;
const ROCKET_HOMING: u32 = 0x9e;
const ROCKET_WAIT: u32 = 0x8a;
const ROCKET_THRUST: u32 = 0xac;
const HIT_WALL: u16 = 0x400;
const ROCKET_EXHAUST: u32 = 0x0607_3144;
const ROCKET_BLAST: u32 = 0x0607_3044;
const ROCKET_BLAST_SOUND: u32 = 17;

/// Its behaviour. Into a wall (bg.rs's map) or shot down: sound 17, a blast drifting on, gone. Off
/// screen: gone. Else it turns to the nearer player (by a share of the angle growing as its homing
/// runs out), its knock eases off, and once its wait is out it thrusts along its turn (its speed
/// easing a 32nd of the way a frame), trailing exhaust every 7th frame from 28 dots behind.
pub fn homing_rocket_behaviour(cpu: &mut Cpu, o: u32) {
    let (x, y) = (cpu.read32(o + POSITION), cpu.read32(o + POSITION + 4));
    if nested!(cpu, 0x0605_ef90, super::bg::map_hit(cpu, x, y)) != 0 {
        cpu.write16(o + HIT_FLAGS, HIT_WALL);
    }
    let alive = |cpu: &mut Cpu| cpu.read32(o + TURRET_LIFE) as i32 > 0 && cpu.read16(o + HIT_FLAGS) & HIT_WALL == 0;
    if !alive(cpu) || nested!(cpu, 0x0605_6a24, aim::off_screen(cpu, o + FLAGS)) as u8 != 0 {
        if !alive(cpu) {
            nested!(cpu, 0x0600_50d4, sound::se_set(cpu, ROCKET_BLAST_SOUND));
            let e = cpu.stack();
            cpu.write32(e + actors::E_SCRIPT, ROCKET_BLAST);
            cpu.write16(e + actors::E_COLOUR, 0x90);
            for k in [0, 4, 8] {
                let v = cpu.read32(o + POSITION + k);
                cpu.write32(e + actors::E_POSITION + k, v);
            }
            let turn = cpu.read16(o + TURN_Z);
            cpu.write16(e + actors::E_ANGLE, turn);
            cpu.write32(e + actors::E_SPEED, 0x2_0000);
            cpu.write32(e + actors::E_SCALES, 0xa000);
            cpu.write32(e + actors::E_SCALES + 4, 0xe000);
            nested!(cpu, 0x0605_7454, actors::drift_effect(cpu, e));
        }
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    let homing = cpu.read16(o + ROCKET_HOMING);
    if homing != 0 {
        cpu.write16(o + ROCKET_HOMING, homing.wrapping_sub(1));
        let share = 64u32.wrapping_sub(cpu.read16(o + ROCKET_HOMING) as i16 as i32 as u32);
        let player = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + FLAGS));
        let to = nested!(cpu, 0x0604_5e30, aim::angle_to(cpu, o, player)) as i16 as i32 as u32;
        let turn = cpu.read16(o + TURN_Z) as i16 as i32 as u32;
        let d = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, turn, to));
        let step = nested!(cpu, 0x0600_6a78, math::quotient(cpu, d, share));
        let t = cpu.read16(o + TURN_Z).wrapping_add(step as u16);
        cpu.write16(o + TURN_Z, t);
    }
    for k in [0, 4] {
        let ease = cpu.read16(o + ROCKET_KNOCK_EASE + 2 * k / 4) as i16 as i32 as u32;
        let v = cpu.read32(o + ROCKET_KNOCK + k);
        let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0, ease));
        cpu.write32(o + ROCKET_KNOCK + k, v);
    }
    let wait = cpu.read16(o + ROCKET_WAIT);
    if wait as i16 >= 0 {
        cpu.write16(o + ROCKET_WAIT, wait.wrapping_sub(1));
    } else {
        for k in [0, 4] {
            let thrust = cpu.read32(o + ROCKET_THRUST);
            let turn = cpu.read16(o + TURN_Z) as i16 as i32 as u32;
            let to = if k == 0 {
                let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, turn));
                nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, thrust))
            } else {
                let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, turn));
                nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, c.wrapping_neg(), thrust))
            };
            let step = (to as i32 / 32).wrapping_abs() as u32;
            let v = cpu.read32(o + ROCKET_SPEED + k);
            let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, to, step));
            cpu.write32(o + ROCKET_SPEED + k, v);
        }
        let frames = cpu.read32(GAME_FRAMES);
        if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, frames, 7)) == 0 {
            let e = cpu.stack();
            cpu.write32(e + actors::E_SCRIPT, ROCKET_EXHAUST);
            cpu.write16(e + actors::E_COLOUR, 0x90);
            let back = cpu.read16(o + TURN_Z).wrapping_add(0x8000) as i16 as i32 as u32;
            let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, back));
            let x = cpu.read32(o + POSITION).wrapping_add(s.wrapping_mul(28));
            cpu.write32(e + actors::E_POSITION, x);
            let back = cpu.read16(o + TURN_Z).wrapping_add(0x8000) as i16 as i32 as u32;
            let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, back));
            let y = cpu.read32(o + POSITION + 4).wrapping_sub(c.wrapping_mul(28));
            cpu.write32(e + actors::E_POSITION + 4, y);
            let z = cpu.read32(o + POSITION + 8);
            cpu.write32(e + actors::E_POSITION + 8, z);
            let turn = cpu.read16(o + TURN_Z);
            cpu.write16(e + actors::E_ANGLE, turn);
            cpu.write32(e + actors::E_SPEED, 0xfffe_0000);
            cpu.write32(e + actors::E_SCALES, 0x8000);
            cpu.write32(e + actors::E_SCALES + 4, 0xc000);
            nested!(cpu, 0x0605_7454, actors::drift_effect(cpu, e));
        }
    }
    for k in [0, 4] {
        let v = cpu.read32(o + ROCKET_SPEED + k).wrapping_add(cpu.read32(o + ROCKET_KNOCK + k));
        let p = cpu.read32(o + POSITION + k).wrapping_add(v);
        cpu.write32(o + POSITION + k, p);
    }
}

/// A homing rocket made (list 2, 0xbc bytes, actor_exec's) from the description `p` (its place
/// and the shooter's number +0x1e; its turn +0x2c, the knock's direction +0x2e (words) and size
/// +0x34, its thrust +0x30, its homing frames +0x38): kind 12 frame 0x18 (the kind's colour), its
/// boxes, side 1, life 500, 3/4 scale, thrusting after 14 frames, its knock easing off over 32.
/// It, or 0.
const ROCKET_FLAGS: u32 = 0x30_0400;
const ROCKET_COLOUR: u32 = 0x0607_42bc;
const ROCKET_BOXES: u32 = 0x060a_0810;
const ROCKET_BEHAVIOUR: u32 = 0x0608_9d04;

pub fn homing_rocket_set(cpu: &mut Cpu, p: u32) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0xbc)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + BEAM_LIST));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + BEHAVIOUR, ROCKET_BEHAVIOUR);
    let n = cpu.read8(p + SHOT_NUMBER);
    cpu.write8(o + NUMBER, n);
    cpu.write32(o + FLAGS, ROCKET_FLAGS);
    for k in [0, 4, 8] {
        let v = cpu.read32(p + k);
        cpu.write32(o + POSITION + k, v);
    }
    cpu.write8(o + BYTE_33, 8);
    cpu.write8(o + SPRITE_FRAME, 0x18);
    cpu.write8(o + SPRITE_KIND, 12);
    let colour = cpu.read16(ROCKET_COLOUR);
    cpu.write16(o + OWN_COLOUR, colour);
    cpu.write16(o + COLOUR, colour);
    cpu.write32(o + BEAM_LIST, ROCKET_BOXES);
    cpu.write8(o + SIDE, 1);
    cpu.write32(o + TURRET_LIFE, 500);
    let turn = cpu.read16(p + 0x2c);
    cpu.write16(o + TURN_Z, turn);
    for k in [0, 4] {
        let size = cpu.read32(p + 0x34);
        let a = cpu.read16(p + 0x2e) as i16 as i32 as u32;
        let knock = if k == 0 {
            let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
            nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, size))
        } else {
            let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
            nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, c.wrapping_neg(), size))
        };
        cpu.write32(o + ROCKET_KNOCK + k, knock);
    }
    for k in [0, 4] {
        let ease = (cpu.read32(o + ROCKET_KNOCK + k) as i32 / 32).wrapping_abs();
        cpu.write16(o + ROCKET_KNOCK_EASE + k / 2, ease as u16);
    }
    cpu.write16(o + ROCKET_WAIT, 14);
    let homing = cpu.read16(p + 0x38);
    cpu.write16(o + ROCKET_HOMING, homing);
    let thrust = cpu.read32(p + 0x30);
    cpu.write32(o + ROCKET_THRUST, thrust);
    cpu.write32(o + SCALES, 0xc000);
    cpu.write32(o + SCALES + 4, 0xc000);
    o
}

/// A homing rocket rising from below (hit only once at depth 0, 1.5 dots a frame up), opening as it
/// starts to thrust: frames 16 and 17 two frames each, then 18-21 by turns, ever faster (8 frames
/// down to 1).
const RISE_STEP: u32 = 0x7c;
const RISE_HOLD: u32 = 0x88;
const RISE_HOLD_NEXT: u32 = 0x89;
const RISE_CYCLE: u32 = 0xa8;

pub fn rising_rocket_behaviour(cpu: &mut Cpu, o: u32) {
    nested!(cpu, ROCKET_BEHAVIOUR, homing_rocket_behaviour(cpu, o));
    let z = cpu.read32(o + POSITION + 8);
    let z = nested!(cpu, 0x0604_6168, actors::approach(cpu, z, 0, 0x1_8000));
    cpu.write32(o + POSITION + 8, z);
    let flags = cpu.read32(o + FLAGS);
    cpu.write32(o + FLAGS, if z == 0 { flags & !NO_HITS } else { flags | NO_HITS });
    let held = |cpu: &mut Cpu| {
        let h = cpu.read8(o + RISE_HOLD).wrapping_sub(1);
        cpu.write8(o + RISE_HOLD, h);
        h == 0
    };
    let next = |cpu: &mut Cpu| {
        let s = cpu.read8(o + RISE_STEP).wrapping_add(1);
        cpu.write8(o + RISE_STEP, s);
    };
    let frame = match cpu.read8(o + RISE_STEP) {
        0 => {
            if cpu.read16(o + ROCKET_WAIT) != 0 {
                return;
            }
            next(cpu);
            cpu.write8(o + RISE_HOLD, 2);
            16
        }
        1 => {
            if !held(cpu) {
                return;
            }
            next(cpu);
            cpu.write8(o + RISE_HOLD, 2);
            17
        }
        2 => {
            if !held(cpu) {
                return;
            }
            next(cpu);
            cpu.write8(o + RISE_HOLD_NEXT, 8);
            cpu.write8(o + RISE_HOLD, 8);
            cpu.write16(o + RISE_CYCLE, 0);
            18
        }
        3 => {
            if !held(cpu) {
                return;
            }
            let cycle = cpu.read16(o + RISE_CYCLE).wrapping_add(1) & 3;
            cpu.write16(o + RISE_CYCLE, cycle);
            let next_hold = cpu.read8(o + RISE_HOLD_NEXT);
            cpu.write8(o + RISE_HOLD, next_hold);
            if next_hold as i8 > 1 {
                cpu.write8(o + RISE_HOLD_NEXT, next_hold - 1);
            }
            cpu.read8(o + RISE_CYCLE + 1).wrapping_add(18)
        }
        _ => return,
    };
    cpu.write8(o + SPRITE_FRAME, frame);
}

/// The rocket launcher (its place +0x4fc): while its part is on and the ship's tilt is within 0x2000
/// of 0x3001, every so many frames by difficulty a rising homing rocket from it (sound 32, its
/// recoil +0x100), homing for frames by difficulty: kind 0x27 frame 15, 16 colours past the kind's,
/// full size; and its flash.
const LAUNCHER: u32 = 0x4fc;
const SHIP_RECOIL_2: u32 = 0x100;
const ROCKET_EVERY: u32 = 0x060a_07f0;
const ROCKET_HOMING_FRAMES: u32 = 0x060a_07f8;
const LAUNCH_SOUND: u32 = 32;
const ROCKET_SET: u32 = 0x0608_9fd8;
const RISING_ROCKET: u32 = 0x0608_a17c;

pub fn rocket_launcher(cpu: &mut Cpu, o: u32) {
    if cpu.read16(o + SHIP_PART_C) & PART_OFF != 0 {
        return;
    }
    let difficulty = cpu.read8(DIFFICULTY) as i8 as i32 as u32;
    let every = cpu.read8(ROCKET_EVERY.wrapping_add(difficulty)) as u32;
    let count = cpu.read16(o + SHIP_COUNT) as u32;
    if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, count, every)) != 0 {
        return;
    }
    if cpu.read16(o + TURN_X).wrapping_add(0xcfff) > 0x1ffe {
        return;
    }
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, LAUNCH_SOUND));
    cpu.write16(o + SHIP_RECOIL_2, 0xfe80);
    let p = cpu.stack();
    let number = cpu.read8(o + NUMBER);
    cpu.write8(p + SHOT_NUMBER, number);
    for k in [0, 4] {
        let v = cpu.read32(o + LAUNCHER + k);
        cpu.write32(p + k, v);
    }
    let z = cpu.read32(o + LAUNCHER + 8).wrapping_add(NEARER);
    cpu.write32(p + 8, z);
    let back = cpu.read16(o + TURN_Z).wrapping_add(0x8000);
    cpu.write16(p + 0x2c, back);
    cpu.write32(p + 0x30, 0x2_a000);
    cpu.write16(p + 0x2e, back);
    cpu.write32(p + 0x34, 0x1_0000);
    let difficulty = cpu.read8(DIFFICULTY) as i8 as i32 as u32;
    let homing = cpu.read8(ROCKET_HOMING_FRAMES.wrapping_add(difficulty)) as u16;
    cpu.write16(p + 0x38, homing);
    let r = nested!(cpu, ROCKET_SET, homing_rocket_set(cpu, p));
    if r != 0 {
        cpu.write32(r + BEHAVIOUR, RISING_ROCKET);
        cpu.write8(r + SPRITE_FRAME, 15);
        cpu.write8(r + SPRITE_KIND, FLARE_KIND);
        let colour = cpu.read16(FLARE_COLOUR).wrapping_add(16);
        cpu.write16(r + OWN_COLOUR, colour);
        cpu.write16(r + COLOUR, colour);
        cpu.write32(r + BEAM_LIST, ROCKET_BOXES);
        cpu.write32(r + SCALES, 0x1_0000);
        cpu.write32(r + SCALES + 4, 0x1_0000);
    }
    nested!(cpu, MUZZLE_FLASH, muzzle_flash(cpu, o, o + LAUNCHER));
}

/// An afterburn behind `o` (24 dots a unit of its scales back from it): an effect (flags 3,
/// relative; kind 0x27, its colour; link 0x060a0850), its flags 0x500, `o`'s scales, drifting back
/// an eighth of them a frame.
const AFTERBURN_LINK: u32 = 0x060a_0850;

pub fn afterburn(cpu: &mut Cpu, o: u32) {
    let fx = cpu.stack();
    for k in [0, 4] {
        let scale = cpu.read32(o + SCALES + k);
        let back = cpu.read16(o + TURN_Z).wrapping_add(0x8000) as i16 as i32 as u32;
        let along = if k == 0 {
            let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, back));
            nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, scale.wrapping_mul(24)))
        } else {
            let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, back));
            nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, c.wrapping_neg(), scale.wrapping_mul(24)))
        };
        let v = cpu.read32(o + POSITION + k).wrapping_add(along);
        cpu.write32(fx + effects::X + k, v);
    }
    let z = cpu.read32(o + POSITION + 8);
    cpu.write32(fx + effects::X + 8, z);
    cpu.write32(fx + effects::FLAGS_LONG, 3);
    let turn = cpu.read16(o + TURN_Z);
    cpu.write16(fx + effects::ANGLE, turn);
    cpu.write16(fx + effects::FRAMES, 0);
    cpu.write8(fx + effects::FRAMES_2, 0);
    cpu.write8(fx + effects::KIND, FLARE_KIND);
    cpu.write8(fx + effects::FRAME, 0);
    let colour = cpu.read16(FLARE_COLOUR);
    cpu.write16(fx + effects::COLOUR, colour);
    let number = cpu.read8(o + NUMBER);
    cpu.write8(fx + effects::PLAYER, number);
    cpu.write32(fx + effects::LINK, AFTERBURN_LINK);
    cpu.write32(fx + effects::RELATIVE_3C, 0);
    let e = nested!(cpu, 0x0604_e1cc, effects::effect_set(cpu, fx));
    if e == 0 {
        return;
    }
    let flags = cpu.read32(e + FLAGS) | 0x500;
    cpu.write32(e + FLAGS, flags);
    for k in [0, 4] {
        let v = cpu.read32(o + SCALES + k);
        cpu.write32(e + SCALES + k, v);
    }
    for (k, at) in [(0, 0x4c), (4, 0x50)] {
        let eighth = (cpu.read32(o + SCALES + k) as i32 / 8) as u32;
        let back = cpu.read16(o + TURN_Z).wrapping_add(0x8000) as i16 as i32 as u32;
        let v = if k == 0 {
            let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, back));
            nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, eighth))
        } else {
            let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, back));
            nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, c.wrapping_neg(), eighth))
        };
        cpu.write32(e + at, v);
    }
}

// ---- the fireball ---------------------------------------------------------------------------------

/// A fireball thrown from the gunship (an actor, kind 0x27): its speed (x +0x80, y +0x84), its
/// pull (words +0x8e, +0x90, easing off by +0x92, +0x94), its growth count (+0x88).
const BALL_SPEED: u32 = 0x80;
const BALL_PULL: u32 = 0x8e;
const BALL_PULL_EASE: u32 = 0x92;
const BALL_GROWTH: u32 = 0x88;
const BALL_SIZES: u32 = 0x060a_07cc;
const BALL_FRAMES: u32 = 0x060a_0863;
const BALL_AREA: (u32, u32, u32, u32) = (0xff30_0000, 0xd0_0000, 0xff00_0000, 0x100_0000);

/// Its behaviour. Leaving the area (-52 to 52 dots high, -16 to 16 across) it ends. It moves, its
/// speed pulled (the pull easing off), growing to its size by difficulty (by 1/8 wide, 3/64 high a
/// frame); its first frames from a table, then three by turns every other frame; an afterburn
/// every 10th frame.
pub fn fireball_behaviour(cpu: &mut Cpu, o: u32) {
    let (top, bottom, left, right) = BALL_AREA;
    let out = cpu.nested_with_stack(&aim::AREA_FLAGS_PROLOGUE, aim::AREA_FLAGS, &[right], |cpu| {
        aim::area_flags(cpu, o + FLAGS, top, bottom, left, right)
    }) as u8;
    if out != 0 {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    for k in [0, 4] {
        let p = cpu.read32(o + POSITION + k).wrapping_add(cpu.read32(o + BALL_SPEED + k));
        cpu.write32(o + POSITION + k, p);
    }
    for k in [0, 4] {
        let pull = cpu.read16(o + BALL_PULL + k / 2) as i16 as i32 as u32;
        let v = cpu.read32(o + BALL_SPEED + k).wrapping_add(pull);
        cpu.write32(o + BALL_SPEED + k, v);
    }
    for k in [0, 2] {
        let (pull, ease) = (cpu.read16(o + BALL_PULL + k) as i16 as i32 as u32, cpu.read16(o + BALL_PULL_EASE + k) as i16 as i32 as u32);
        let pull = nested!(cpu, 0x0604_6168, actors::approach(cpu, pull, 0, ease));
        cpu.write16(o + BALL_PULL + k, pull as u16);
    }
    let difficulty = cpu.read8(DIFFICULTY) as i8 as i32 as u32;
    let size = cpu.read32(BALL_SIZES.wrapping_add(4 * difficulty));
    for (k, rate) in [(0, 0x2000), (4, 0xc00)] {
        let s = cpu.read32(o + SCALES + k);
        let s = nested!(cpu, 0x0604_6168, actors::approach(cpu, s, size, rate));
        cpu.write32(o + SCALES + k, s);
    }
    let growth = cpu.read8(o + BALL_GROWTH);
    if growth as i8 <= 7 {
        let growth = growth.wrapping_add(1);
        cpu.write8(o + BALL_GROWTH, growth);
        let at = ((growth as i8 as i32 / 2) * 2) as u32;
        let frame = cpu.read8(BALL_FRAMES.wrapping_add(at));
        cpu.write8(o + SPRITE_FRAME, frame);
    } else {
        let frames = cpu.read32(GAME_FRAMES) >> 1;
        let frame = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, frames, 3));
        cpu.write8(o + SPRITE_FRAME, frame as u8);
    }
    let frames = cpu.read32(GAME_FRAMES);
    if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, frames, 10)) == 0 {
        nested!(cpu, 0x0608_a3f8, afterburn(cpu, o));
    }
}

/// A fireball made (list 2, 0xbc bytes, actor_exec's) by the gunship `ship` from its mouth (+0x508,
/// +0x50c): kind 0x27 frame 11 (the kind's colour), its boxes, side 1, type 5, thrown along the
/// ship's turn at 5 dots a frame, turned back, pulled back half a dot a frame (easing off over
/// frames by difficulty). It, or 0.
const SHIP_MOUTH: u32 = 0x508;
const BALL_BOXES: u32 = 0x060a_0830;
const BALL_EASE_FRAMES: u32 = 0x060a_07e0;
const FIREBALL: u32 = 0x0608_a53c;

pub fn fireball_set(cpu: &mut Cpu, ship: u32) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0xbc)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + BEAM_LIST));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    let n = cpu.read8(ship + NUMBER);
    cpu.write8(o + NUMBER, n);
    cpu.write32(o + BEHAVIOUR, FIREBALL);
    cpu.write32(o + FLAGS, 0x400);
    for k in [0, 4] {
        let v = cpu.read32(ship + SHIP_MOUTH + k);
        cpu.write32(o + POSITION + k, v);
    }
    cpu.write32(o + POSITION + 8, 0);
    cpu.write8(o + BYTE_33, 6);
    cpu.write8(o + SPRITE_FRAME, 11);
    cpu.write8(o + SPRITE_KIND, FLARE_KIND);
    let colour = cpu.read16(FLARE_COLOUR);
    cpu.write16(o + OWN_COLOUR, colour);
    cpu.write16(o + COLOUR, colour);
    cpu.write32(o + BEAM_LIST, BALL_BOXES);
    cpu.write8(o + SIDE, 1);
    cpu.write8(o + TYPE, 5);
    let back = cpu.read16(ship + TURN_Z).wrapping_add(0x8000);
    cpu.write16(o + TURN_Z, back);
    let turn = cpu.read16(ship + TURN_Z) as i16 as i32 as u32;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, turn));
    cpu.write32(o + BALL_SPEED, s.wrapping_mul(5));
    let turn = cpu.read16(ship + TURN_Z) as i16 as i32 as u32;
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, turn));
    cpu.write32(o + BALL_SPEED + 4, c.wrapping_neg().wrapping_mul(5));
    let back = cpu.read16(o + TURN_Z) as i16 as i32 as u32;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, back));
    let pull = nested!(cpu, 0x0600_6a78, math::quotient(cpu, s, 2));
    cpu.write16(o + BALL_PULL, pull as u16);
    let back = cpu.read16(o + TURN_Z) as i16 as i32 as u32;
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, back));
    let pull = nested!(cpu, 0x0600_6a78, math::quotient(cpu, c.wrapping_neg(), 2));
    cpu.write16(o + BALL_PULL + 2, pull as u16);
    let difficulty = cpu.read8(DIFFICULTY) as i8 as i32 as u32;
    let frames = cpu.read8(BALL_EASE_FRAMES.wrapping_add(difficulty)) as u32;
    for k in [0, 2] {
        let pull = cpu.read16(o + BALL_PULL + k) as i16 as i32 as u32;
        let ease = nested!(cpu, 0x0600_6a78, math::quotient(cpu, pull, frames)) as i32;
        cpu.write16(o + BALL_PULL_EASE + k, ease.wrapping_abs() as u16);
    }
    cpu.write8(o + BALL_GROWTH, 0);
    o
}

// ---- the gunship's charge -------------------------------------------------------------------------

/// The charge glow at the gunship's mouth (a plain object: its ship +0x3c, the place it keeps to
/// +0x40, 176 dots nearer): it ends with the ship's part (+0x10e) or once the ship's +0xe0 passes 4.
/// It grows to 3/4 (1/128 a frame), flickers (flag 0x100 by turns, four frames from 0xd3), and
/// every other frame throws a ring spark, a ring flare (from its centre, as fast as it is wide) and
/// a twin flash (explosions.rs's).
const CHARGE_SHIP: u32 = 0x3c;
const CHARGE_AT: u32 = 0x40;
const SHIP_CHARGE: u32 = 0xe0;
const CHARGE_FLICKER: u32 = 0x100;
const RING_SPARK_SET: u32 = 0x0608_96f8;
const RING_FLARE_SET: u32 = 0x0608_98a0;

pub fn charge_glow_behaviour(cpu: &mut Cpu, o: u32) {
    let ship = cpu.read32(o + CHARGE_SHIP);
    if cpu.read16(ship + SHIP_PART_B) & PART_OFF != 0 || cpu.read8(ship + SHIP_CHARGE) > 4 {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    let at = cpu.read32(o + CHARGE_AT);
    for k in [0, 4] {
        let v = cpu.read32(at + k);
        cpu.write32(o + POSITION + k, v);
    }
    let z = cpu.read32(at + 8).wrapping_add(NEARER);
    cpu.write32(o + POSITION + 8, z);
    for k in [0, 4] {
        let s = cpu.read32(o + SCALES + k);
        let s = nested!(cpu, 0x0604_6168, actors::approach(cpu, s, 0xc000, 0x200));
        cpu.write32(o + SCALES + k, s);
    }
    let flags = cpu.read32(o + FLAGS) ^ CHARGE_FLICKER;
    cpu.write32(o + FLAGS, flags);
    let frame = (cpu.read8(GAME_FRAMES + 3) & 3).wrapping_sub(45);
    cpu.write8(o + SPRITE_FRAME, frame);
    if cpu.read32(GAME_FRAMES) & 1 != 0 {
        return;
    }
    let p = cpu.stack();
    for k in [0, 4, 8] {
        let v = cpu.read32(o + POSITION + k);
        cpu.write32(p + k, v);
    }
    cpu.write32(p + SPARK_CENTRE, o);
    let speed = cpu.read32(o + SCALES);
    cpu.write32(p + 0x2c, speed);
    nested!(cpu, RING_SPARK_SET, ring_spark_set(cpu, p));
    nested!(cpu, RING_FLARE_SET, ring_flare_set(cpu, p));
    nested!(cpu, 0x0605_8278, explosions::twin_set(cpu, p));
}

/// The charge glow made (list 3, 0x44 bytes, plain) at the gunship's mouth, from nothing, with sound
/// 70: whether it was.
const CHARGE_GLOW: u32 = 0x0608_a838;

pub fn charge_glow_set(cpu: &mut Cpu, ship: u32) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0x44)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 3, blocks, PLAIN_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + FLAGS));
    cpu.write32(o + BEHAVIOUR, CHARGE_GLOW);
    cpu.write32(o + CHARGE_SHIP, ship);
    cpu.write32(o + CHARGE_AT, ship + SHIP_MOUTH);
    cpu.write8(o + SPRITE_FRAME, 0);
    cpu.write8(o + SPRITE_KIND, 0);
    let colour = cpu.read16(CHAR_BASE);
    cpu.write16(o + COLOUR, colour);
    cpu.write8(o + BYTE_33, 0);
    cpu.write32(o + SCALES, 0);
    cpu.write32(o + SCALES + 4, 0);
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, CHARGE_SOUND));
    1
}

/// The screen object's (screen.rs's SCREEN, a pointer) +0x14 and +0x18 (not known yet: they ease and
/// shrink as the gunship's fight goes).
const SCREEN_AT: u32 = 0x0607_4740;
const SCREEN_14: u32 = 0x14;
const SCREEN_18: u32 = 0x18;

/// The screen's +0x18 less 0x106, not below 1.
pub fn screen_18_shrink(cpu: &mut Cpu) {
    let s = cpu.read32(SCREEN_AT);
    let v = cpu.read32(s + SCREEN_18).wrapping_add(0xffff_fefa);
    cpu.write32(s + SCREEN_18, v);
    if v as i32 <= 0xffff {
        cpu.write32(s + SCREEN_18, 0x1_0000);
    }
}

/// The screen's +0x14 eased toward `to` (by 1/16 a frame).
pub fn screen_14_ease(cpu: &mut Cpu, to: u32) {
    let s = cpu.read32(SCREEN_AT);
    let v = cpu.read32(s + SCREEN_14);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, to, 0x1000));
    cpu.write32(s + SCREEN_14, v);
}

/// 24 dots less 176 dots a unit the screen's +0x18 is short of 1 (whole dots).
pub fn screen_18_depth(cpu: &mut Cpu) -> u32 {
    let s = cpu.read32(SCREEN_AT);
    let short = 0x1_0000u32.wrapping_sub(cpu.read32(s + SCREEN_18));
    let d = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, short, 0xb0));
    0x18_0000u32.wrapping_sub(d << 16)
}

/// The mover (bg.rs's) carried along z by the stage's push (0x060a4480: it eases to a stop under 48
/// dots, to back up 1.25 dots a frame past them; z kept from going below 0), its tilt (+4, a
/// word) a 1024th of z back, its x and y lean (+0, +2) from the tilt.
const STAGE_PUSH: u32 = 0x060a_4480;
const MOVER_LEAN: u32 = 0x00;
const MOVER_ANGLE: u32 = 0x04;

pub fn mover_push(cpu: &mut Cpu) {
    let push = cpu.read32(STAGE_PUSH);
    let z = cpu.read32(MOVER + MOVER_Z).wrapping_add(push);
    cpu.write32(MOVER + MOVER_Z, z);
    let (to, rate) = if z as i32 > 0x2f_ffff { (0xfffe_c000, 0x140) } else { (0, 0x2f0) };
    let push = nested!(cpu, 0x0604_6168, actors::approach(cpu, push, to, rate));
    cpu.write32(STAGE_PUSH, push);
    if cpu.read32(MOVER + MOVER_Z) as i32 <= 0 {
        cpu.write32(MOVER + MOVER_Z, 0);
    }
    let z = cpu.read32(MOVER + MOVER_Z) as i32 / 0x100;
    let angle = nested!(cpu, 0x0600_6a78, math::quotient(cpu, (z << 2).wrapping_neg() as u32, 16));
    cpu.write16(MOVER + MOVER_ANGLE, angle as u16);
    for (at, by) in [(MOVER_LEAN, 0x1800), (MOVER_LEAN + 2, 0xffff_f000)] {
        let a = cpu.read16(MOVER + MOVER_ANGLE) as i16 as i32 as u32;
        let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
        let v = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, by));
        cpu.write16(MOVER + at, v as u16);
    }
}

// ---- the gunship's flight -------------------------------------------------------------------------

/// The gunship's turn's speed (+0xf4, a word, at most the angle left, to 0x800) and the speed's goal
/// (+0xf6: a tenth of that angle, toward the turn's side; eased by 4 a frame), its roll's speed
/// (+0xf8, a word) and roll (+0x52c, 16.16 of a turn) and its rolls (+0x530), its flags (+0xe2: 1
/// once it has rolled twice).
const SHIP_TURN_SPEED: u32 = 0xf4;
const SHIP_TURN_GOAL: u32 = 0xf6;
const SHIP_ROLL_SPEED: u32 = 0xf8;
const SHIP_ROLL: u32 = 0x52c;
const SHIP_ROLLS: u32 = 0x530;
const SHIP_FLAGS: u32 = 0xe2;

/// Its turn's speed set going toward `to`.
pub fn ship_turn_toward(cpu: &mut Cpu, o: u32, to: u32) {
    let to = to as i16 as i32 as u32;
    let turn = cpu.read16(o + TURN_Z) as u32;
    let d = nested!(cpu, 0x0605_6580, aim::angle_between(cpu, turn, to & 0xffff)) as i16 as i32;
    let left = (d.wrapping_abs() as u16).min(0x800) as u32;
    let turn = cpu.read16(o + TURN_Z) as i16 as i32 as u32;
    let side = nested!(cpu, 0x0605_6554, aim::turn_sign(cpu, to, turn)) as i32;
    let goal = nested!(cpu, 0x0600_6a78, math::quotient(cpu, left, if side > 0 { 10 } else { 0xffff_fff6 }));
    let speed = cpu.read16(o + SHIP_TURN_GOAL) as i16 as i32 as u32;
    let speed = nested!(cpu, 0x0604_6168, actors::approach(cpu, speed, goal, 4));
    cpu.write16(o + SHIP_TURN_GOAL, speed as u16);
    let (now, goal) = (cpu.read16(o + SHIP_TURN_SPEED) as i16 as i32 as u32, cpu.read16(o + SHIP_TURN_GOAL) as i16 as i32 as u32);
    let now = nested!(cpu, 0x0604_6168, actors::approach(cpu, now, goal, left));
    cpu.write16(o + SHIP_TURN_SPEED, now as u16);
}

/// It turns to face away from the nearer player (straight, at the easiest difficulty).
pub fn ship_turn_from_player(cpu: &mut Cpu, o: u32) {
    let to = if cpu.read8(DIFFICULTY) == 0 {
        0
    } else {
        let player = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + FLAGS));
        let a = nested!(cpu, 0x0604_5e30, aim::angle_to(cpu, o, player));
        a.wrapping_add(0x8000) as i16 as i32 as u32
    };
    nested!(cpu, 0x0608_ab14, ship_turn_toward(cpu, o, to));
}

/// It rolls (its tilt back by its roll's speed, easing to 0x180 by 8), counting its rolls.
pub fn ship_roll(cpu: &mut Cpu, o: u32) {
    let v = cpu.read16(o + SHIP_ROLL_SPEED) as i16 as i32 as u32;
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0x180, 8)) as u16;
    cpu.write16(o + SHIP_ROLL_SPEED, v);
    let tilt = cpu.read16(o + TURN_X).wrapping_sub(v);
    cpu.write16(o + TURN_X, tilt);
    let roll = cpu.read32(o + SHIP_ROLL).wrapping_sub(cpu.read16(o + SHIP_ROLL_SPEED) as i16 as i32 as u32);
    cpu.write32(o + SHIP_ROLL, roll);
    if (roll as i32) < 0 {
        cpu.write32(o + SHIP_ROLL, roll.wrapping_add(0x1_0000));
        let rolls = cpu.read8(o + SHIP_ROLLS).wrapping_add(1);
        cpu.write8(o + SHIP_ROLLS, rolls);
        if rolls > 1 {
            let flags = cpu.read16(o + SHIP_FLAGS) | 1;
            cpu.write16(o + SHIP_FLAGS, flags);
        }
    }
}

/// Its drift (+0xe8, +0xec), eased a 32nd of the way a frame toward 5/8 of a dot a frame, a quarter
/// turn off the way to a point jittered up to 16 dots about (0, `dy` dots) from it.
const SHIP_DRIFT: u32 = 0xe8;

pub fn ship_drift(cpu: &mut Cpu, o: u32, dy: u32) {
    let dy = dy as i16 as i32;
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000)) as i32;
    let off_y = ((dy + 16 - (r & 31)) << 16) as u32;
    let y = cpu.read32(o + POSITION + 4).wrapping_sub(off_y);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000)) as i32;
    let off_x = ((16 - (r & 31)) << 16) as u32;
    let x = cpu.read32(o + POSITION).wrapping_sub(off_x);
    let a = nested!(cpu, 0x0606_780c, sgl::sl_atan(cpu, x, y));
    let a = (a as u16).wrapping_add(0xc000) as i16 as i32 as u32;
    for k in [0, 4] {
        let to = if k == 0 {
            let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
            nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, 0xa000))
        } else {
            let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
            nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, c.wrapping_neg(), 0xa000))
        };
        let step = (to as i32 / 32).wrapping_abs() as u32;
        let v = cpu.read32(o + SHIP_DRIFT + k);
        let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, to, step));
        cpu.write32(o + SHIP_DRIFT + k, v);
    }
}

/// Its drift about 48 dots up from it.
pub fn ship_drift_up(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_ac7c, ship_drift(cpu, o, 0xffff_ffd0));
}

/// The gunship's step (+0xe0), its script (actors.rs's, at +0x6c; its flags +0x94), its parts' hit
/// flags (eight words from +0x108: 0x20 not yet hittable).
const SHIP_STEP: u32 = 0xe0;
const SHIP_SCRIPT: u32 = 0x6c;
const SHIP_SCRIPT_FLAGS: u32 = 0x94;
const SHIP_PARTS_HIT: u32 = 0x108;
const PART_NOT_YET: u16 = 0x20;
const SHIP_SCRIPT_1: u32 = 0x060a_0762;
const SHIP_SCRIPT_2: u32 = 0x060a_0740;

/// Step 0: it comes in (2 dots a frame from far, tilting back 0x300 a frame, the screen's +0x18
/// shrinking, its +0x14 easing to -4 dots) until past the screen's depth; then upright (a quarter
/// tilt), its parts made hittable, its first script, 1.
pub fn ship_come_in(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_a9b8, screen_18_shrink(cpu));
    nested!(cpu, 0x0608_a9e8, screen_14_ease(cpu, 0xfffc_0000));
    let tilt = cpu.read16(o + TURN_X).wrapping_add(0xfd00);
    cpu.write16(o + TURN_X, tilt);
    let z = cpu.read32(o + POSITION + 8).wrapping_add(0x2_0000);
    cpu.write32(o + POSITION + 8, z);
    let depth = nested!(cpu, 0x0608_aadc, screen_18_depth(cpu));
    if cpu.read32(o + POSITION + 8) as i32 <= depth as i32 {
        return;
    }
    cpu.write8(o + SHIP_STEP, 1);
    nested!(cpu, 0x0605_6628, actors::script_start(cpu, o + SHIP_SCRIPT, SHIP_SCRIPT_1));
    cpu.write16(o + TURN_X, 0x4000);
    cpu.write16(o + TURN_Y, 0);
    for k in 0..8 {
        let at = o + SHIP_PARTS_HIT + 2 * k;
        let v = cpu.read16(at) & !PART_NOT_YET;
        cpu.write16(at, v);
    }
}

/// Step 1: held at the screen's depth (its +0x14 easing to -6 dots), drifting about 48 dots up
/// (pushed down by half a dot a frame while its script's flag 1 or 2 is set); its script's flag
/// 0x4000 starts its second script (2).
pub fn ship_hold(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_a9b8, screen_18_shrink(cpu));
    nested!(cpu, 0x0608_a9e8, screen_14_ease(cpu, 0xfffa_0000));
    let depth = nested!(cpu, 0x0608_aadc, screen_18_depth(cpu));
    cpu.write32(o + POSITION + 8, depth);
    nested!(cpu, 0x0608_ad78, ship_drift_up(cpu, o));
    let flags = cpu.read16(o + SHIP_SCRIPT_FLAGS);
    if flags & 1 != 0 || flags & 2 != 0 {
        let v = cpu.read32(o + SHIP_DRIFT + 4).wrapping_add(0x8000);
        cpu.write32(o + SHIP_DRIFT + 4, v);
    }
    if cpu.read16(o + SHIP_SCRIPT_FLAGS) & 0x4000 != 0 {
        cpu.write8(o + SHIP_STEP, 2);
        nested!(cpu, 0x0605_6628, actors::script_start(cpu, o + SHIP_SCRIPT, SHIP_SCRIPT_2));
    }
}

/// The puff of smoke the gunship trails while it waits (drift_effect's script), how far the stage's
/// ground map has come (0x060a4840's place, 16.16 dots) when it moves on, the count at which it
/// starts charging.
const SHIP_SMOKE: u32 = 0x0607_30c4;
const MAP_PLACE: u32 = 0x060a_4848;
const SHIP_MOVES_ON: i32 = 0x0900_0000;
const SHIP_CHARGES_AT: u16 = 0x180;

/// Step 2: held at the screen's depth (its +0x14 easing to -6 dots), drifting about 64 dots up and
/// leaning with its drift, a puff of smoke rising from one side or the other (by its count) up to
/// 63 dots above it and 24 dots nearer; moving on (3) once the ground map has come 2304 dots, its
/// charge glow started at its count's 0x180th.
pub fn ship_smoke(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_a9b8, screen_18_shrink(cpu));
    nested!(cpu, 0x0608_a9e8, screen_14_ease(cpu, 0xfffa_0000));
    let depth = nested!(cpu, 0x0608_aadc, screen_18_depth(cpu));
    cpu.write32(o + POSITION + 8, depth);
    nested!(cpu, 0x0608_ac7c, ship_drift(cpu, o, 0xffff_ffc0));
    let dx = cpu.read32(o + SHIP_DRIFT).wrapping_neg();
    let a = nested!(cpu, 0x0606_780c, sgl::sl_atan(cpu, dx, 0x1_0000));
    let to = (a as u16).wrapping_add(0xc000) as i16 as i32 as u32;
    nested!(cpu, 0x0608_ab14, ship_turn_toward(cpu, o, to));
    let e = cpu.stack();
    cpu.write32(e + actors::E_SCRIPT, SHIP_SMOKE);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write16(e + actors::E_COLOUR, r as u16 & 0x8000);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000)) as i32;
    let side = if cpu.read16(o + SHIP_COUNT) & 1 != 0 { 0x30_0000 } else { 0xffd0_0000 };
    let x = cpu.read32(o + POSITION).wrapping_add(((16 - (r & 31)) << 16) as u32).wrapping_add(side);
    cpu.write32(e + actors::E_POSITION, x);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    let y = cpu.read32(o + POSITION + 4).wrapping_sub((r & 63) << 16);
    cpu.write32(e + actors::E_POSITION + 4, y);
    let z = cpu.read32(o + POSITION + 8).wrapping_add(0x18_0000);
    cpu.write32(e + actors::E_POSITION + 8, z);
    let back = cpu.read16(o + TURN_Z).wrapping_add(0x8000);
    cpu.write16(e + actors::E_ANGLE, back);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write32(e + actors::E_SPEED, ((r & 7) << 16).wrapping_add(0x4_0000));
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write32(e + actors::E_SCALES, 0x1_0000u32.wrapping_sub((r & 7) << 15));
    cpu.write32(e + actors::E_SCALES + 4, 0x2_0000);
    nested!(cpu, 0x0605_7454, actors::drift_effect(cpu, e));
    if cpu.read32(MAP_PLACE) as i32 > SHIP_MOVES_ON {
        cpu.write8(o + SHIP_STEP, 3);
    }
    if cpu.read16(o + SHIP_COUNT) == SHIP_CHARGES_AT {
        nested!(cpu, 0x0608_a91c, charge_glow_set(cpu, o));
    }
}

/// Its depth eased back to 8 dots (by a dot a frame), the stage's push carrying the mover, it turns
/// from the player, rolls and drifts about 48 dots up.
fn ship_fly(cpu: &mut Cpu, o: u32) {
    let z = cpu.read32(o + POSITION + 8);
    let z = nested!(cpu, 0x0604_6168, actors::approach(cpu, z, 0x8_0000, 0x1_0000));
    cpu.write32(o + POSITION + 8, z);
    nested!(cpu, 0x0608_aa18, mover_push(cpu));
    nested!(cpu, 0x0608_abb0, ship_turn_from_player(cpu, o));
    nested!(cpu, 0x0608_ac04, ship_roll(cpu, o));
    nested!(cpu, 0x0608_ad78, ship_drift_up(cpu, o));
}

/// Its guns: the spray gun, the rocket launcher, the twin guns.
fn ship_guns(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_9a2c, spray_gun(cpu, o));
    nested!(cpu, 0x0608_a2a8, rocket_launcher(cpu, o));
    nested!(cpu, 0x0608_9b74, twin_guns(cpu, o));
}

/// Step 3: flying and firing; while its mouth's part is on, on its every third roll (always at the
/// hardest difficulty) it starts charging (4).
pub fn ship_fight(cpu: &mut Cpu, o: u32) {
    ship_fly(cpu, o);
    if cpu.read16(o + SHIP_PART_B) & PART_OFF == 0 {
        let charge = cpu.read8(DIFFICULTY) == 4 || {
            let rolls = cpu.read8(o + SHIP_ROLLS) as u32;
            nested!(cpu, 0x0600_6a6e, math::remainder(cpu, rolls, 3)) == 0
        };
        if charge {
            cpu.write8(o + SHIP_STEP, 4);
            nested!(cpu, 0x0608_a91c, charge_glow_set(cpu, o));
        }
    }
    ship_guns(cpu, o);
}

/// The fireball's sound, the gunship's script once it has fired one.
const FIREBALL_SOUND: u32 = 42;
const SHIP_SCRIPT_FIRED: u32 = 0x060a_07aa;

/// Step 4: flying and firing while it charges (back to 3 if its mouth's part is shot off); once its
/// tilt is between 0x2c01 and 0x3eff its fireball (5): thrown back along its way (its drift its
/// direction, a dot a frame), rolling fast (0x600), not yet rolled twice, a flash from its mouth.
pub fn ship_charge(cpu: &mut Cpu, o: u32) {
    ship_fly(cpu, o);
    ship_guns(cpu, o);
    if cpu.read16(o + SHIP_PART_B) & PART_OFF != 0 {
        cpu.write8(o + SHIP_STEP, 3);
        return;
    }
    if cpu.read16(o + TURN_X).wrapping_sub(0x2c01) > 0x12fe {
        return;
    }
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, FIREBALL_SOUND));
    cpu.write8(o + SHIP_STEP, 5);
    let flags = cpu.read16(o + SHIP_FLAGS) & !1;
    cpu.write16(o + SHIP_FLAGS, flags);
    let turn = cpu.read16(o + TURN_Z) as i16 as i32 as u32;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, turn));
    cpu.write32(o + SHIP_DRIFT, s);
    let turn = cpu.read16(o + TURN_Z) as i16 as i32 as u32;
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, turn));
    cpu.write32(o + SHIP_DRIFT + 4, c.wrapping_neg());
    cpu.write16(o + SHIP_ROLL_SPEED, 0x600);
    nested!(cpu, 0x0608_a684, fireball_set(cpu, o));
    nested!(cpu, 0x0605_6628, actors::script_start(cpu, o + SHIP_SCRIPT, SHIP_SCRIPT_FIRED));
    let e = cpu.stack();
    for k in [0, 4] {
        let v = cpu.read32(o + SHIP_MOUTH + k);
        cpu.write32(e + actors::E_POSITION + k, v);
    }
    cpu.write32(e + actors::E_POSITION + 8, 0);
    cpu.write16(e + actors::E_COLOUR, 16);
    cpu.write32(e + actors::E_SCALES + 4, 0x4_0000);
    let turn = cpu.read16(o + TURN_Z);
    cpu.write16(e + actors::E_ANGLE, turn);
    nested!(cpu, 0x0605_7e38, explosions::cross_flash(cpu, e));
}

/// Step 5: thrown back by its fireball (drifting, rolling, its turn slowing to a stop by 64 a frame,
/// its depth easing back to 8 dots), its spray gun and rocket launcher firing; its script's flag
/// 0x4000 starts its second script again (3).
pub fn ship_thrown_back(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_aa18, mover_push(cpu));
    nested!(cpu, 0x0608_ac04, ship_roll(cpu, o));
    nested!(cpu, 0x0608_ad78, ship_drift_up(cpu, o));
    let z = cpu.read32(o + POSITION + 8);
    let z = nested!(cpu, 0x0604_6168, actors::approach(cpu, z, 0x8_0000, 0x1_0000));
    cpu.write32(o + POSITION + 8, z);
    let speed = cpu.read16(o + SHIP_TURN_SPEED) as i16 as i32 as u32;
    let speed = nested!(cpu, 0x0604_6168, actors::approach(cpu, speed, 0, 64));
    cpu.write16(o + SHIP_TURN_SPEED, speed as u16);
    nested!(cpu, 0x0608_9a2c, spray_gun(cpu, o));
    nested!(cpu, 0x0608_a2a8, rocket_launcher(cpu, o));
    if cpu.read16(o + SHIP_SCRIPT_FLAGS) & 0x4000 != 0 {
        cpu.write8(o + SHIP_STEP, 3);
        nested!(cpu, 0x0605_6628, actors::script_start(cpu, o + SHIP_SCRIPT, SHIP_SCRIPT_2));
    }
}

/// The gunship's count since it was downed (+0x102, a word), then its count to its next events
/// (+0x104) and which they are (+0x106); its parts' hit flag once it is gone.
const SHIP_DOWN_COUNT: u32 = 0x102;
const SHIP_EVENTS_COUNT: u32 = 0x104;
const SHIP_EVENTS_NEXT: u32 = 0x106;
const PART_GONE: u16 = 0x80;

/// Step 6: downed, its depth easing back to 8 dots and the stage's push carrying the mover; held
/// no deeper than the mover (falling ever faster, tilting forward 0x500 a frame, turning straight),
/// else rolling; drifting about 48 dots up. An explosion at its 60th frame; at its 210th it is gone
/// (7): its drift stopped, its parts all gone, its events due in 120 frames. Its script's flag
/// 0x4000 starts its second script again.
pub fn ship_sink(cpu: &mut Cpu, o: u32) {
    let z = cpu.read32(o + POSITION + 8);
    let z = nested!(cpu, 0x0604_6168, actors::approach(cpu, z, 0x8_0000, 0x1_0000));
    cpu.write32(o + POSITION + 8, z);
    nested!(cpu, 0x0608_aa18, mover_push(cpu));
    let floor = cpu.read32(MOVER + MOVER_Z);
    if (floor as i32) < cpu.read32(o + POSITION + 8) as i32 {
        cpu.write32(o + POSITION + 8, floor);
        let v = cpu.read32(o + SHIP_DRIFT + 4).wrapping_add(0x600);
        cpu.write32(o + SHIP_DRIFT + 4, v);
        let tilt = cpu.read16(o + TURN_X).wrapping_add(0x500);
        cpu.write16(o + TURN_X, tilt);
        nested!(cpu, 0x0608_ad78, ship_drift_up(cpu, o));
        nested!(cpu, 0x0608_ab14, ship_turn_toward(cpu, o, 0));
    } else {
        nested!(cpu, 0x0608_ac04, ship_roll(cpu, o));
        nested!(cpu, 0x0608_ad78, ship_drift_up(cpu, o));
    }
    let n = cpu.read16(o + SHIP_DOWN_COUNT).wrapping_add(1);
    cpu.write16(o + SHIP_DOWN_COUNT, n);
    if n == 60 {
        let p = cpu.stack();
        for k in [0, 4, 8] {
            let v = cpu.read32(o + POSITION + k);
            cpu.write32(p + k, v);
        }
        cpu.write32(p + 0x3c, o);
        nested!(cpu, 0x0605_8ba4, explosions::explosion_set(cpu, p));
    }
    if cpu.read16(o + SHIP_DOWN_COUNT) == 210 {
        cpu.write8(o + SHIP_STEP, 7);
        cpu.write32(o + SHIP_DRIFT, 0);
        cpu.write32(o + SHIP_DRIFT + 4, 0);
        cpu.write16(o + SHIP_EVENTS_COUNT, 120);
        cpu.write16(o + SHIP_EVENTS_NEXT, 0);
        for k in 0..8 {
            let at = o + SHIP_PARTS_HIT + 2 * k;
            let v = cpu.read16(at) | PART_GONE;
            cpu.write16(at, v);
        }
    }
    if cpu.read16(o + SHIP_SCRIPT_FLAGS) & 0x4000 != 0 {
        nested!(cpu, 0x0605_6628, actors::script_start(cpu, o + SHIP_SCRIPT, SHIP_SCRIPT_2));
    }
}

/// The events (timeline.rs's) set going after the gunship is gone, in turn.
const SHIP_GONE_EVENTS: [u32; 3] = [0x060a_16d0, 0x060a_1730, 0x060a_1790];

/// Step 7: gone. Once the mover has come to rest (no lean or tilt, at 0, no push) it is stopped and
/// the screen's scroll moves on; while the mover is over 128 dots along, every 240 frames the next
/// of its events.
pub fn ship_gone(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_aa18, mover_push(cpu));
    if cpu.read16(MOVER + MOVER_LEAN) == 0
        && cpu.read32(MOVER + MOVER_Z) == 0
        && cpu.read16(MOVER + MOVER_ANGLE) == 0
        && cpu.read32(STAGE_PUSH) == 0
    {
        cpu.write16(MOVER + MOVER_LEAN + 2, 0);
        cpu.write8(o + OBJ_MODE, TO_STOP);
        nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
    }
    let n = cpu.read16(o + SHIP_EVENTS_COUNT).wrapping_sub(1);
    cpu.write16(o + SHIP_EVENTS_COUNT, n);
    if cpu.read32(MOVER + MOVER_Z) as i32 > 0x80_0000 && (n as i16) < 0 {
        cpu.write16(o + SHIP_EVENTS_COUNT, 240);
        let next = cpu.read16(o + SHIP_EVENTS_NEXT) as i16;
        if (0..3).contains(&next) {
            let events = SHIP_GONE_EVENTS[next as usize];
            nested!(cpu, 0x0605_7354, timeline::timeline_set(cpu, events));
        }
        let next = cpu.read16(o + SHIP_EVENTS_NEXT).wrapping_add(1);
        cpu.write16(o + SHIP_EVENTS_NEXT, next);
    }
}

/// The gunship's life (+0x120, the boss gauge's) and its parts' (longs: +0x118, +0x11c, +0x124,
/// +0x12c for two parts, +0x130); its turn's lean (+0xf2, a word), its guns' kicks (words: the spray
/// gun's +0xfa, its recoil +0xfc; the rocket launcher's +0xfe, its recoil +0x100), its counts (words:
/// +0x536; +0x532 and +0x538 while their parts are off).
const SHIP_LIFE: u32 = 0x120;
const SHIP_LEAN: u32 = 0xf2;
const SHIP_KICKS: [(u32, u32, u16); 2] = [(0xfa, SHIP_RECOIL, 0x90), (0xfe, SHIP_RECOIL_2, 0x40)];
const SHIP_COUNT_536: u32 = 0x536;

/// Its parts as they are knocked off (their hit flags, the life that does it and how low, the
/// flags then set, their smoke's steps started, the count run down once off).
struct ShipPart {
    flags: u32,
    life: u32,
    low: i32,
    off: u16,
    smokes: &'static [u32],
    count: Option<u32>,
}

const SHIP_PART_LIVES: [ShipPart; 6] = [
    ShipPart { flags: 0x108, life: 0x118, low: 0, off: 0x40, smokes: &[0x134], count: None },
    ShipPart { flags: 0x10a, life: 0x11c, low: 0, off: 0xc0, smokes: &[0x135], count: Some(0x532) },
    ShipPart { flags: 0x10e, life: 0x124, low: 0, off: 0x40, smokes: &[0x137], count: None },
    ShipPart { flags: 0x112, life: 0x12c, low: 750, off: 0xc0, smokes: &[0x139], count: None },
    ShipPart { flags: 0x114, life: 0x12c, low: 0, off: 0x40, smokes: &[0x13a], count: None },
    ShipPart { flags: 0x116, life: 0x130, low: 0, off: 0xc0, smokes: &[0x13b, 0x138], count: Some(0x538) },
];

/// What a downed gunship adds to the stage's rate (its parts' lives, each when at 0: 5 more if the
/// fifth's part is off though its life is not out), the flags its parts are then given, and their
/// smoke steps started.
const SHIP_DOWN_SCORE: u32 = 5000;
const SHIP_DOWN_RATE: u32 = 30;
const SHIP_PART_RATES: [(u32, u32); 3] = [(0x118, 10), (0x11c, 10), (0x124, 30)];
const SHIP_DOWNED_FLAGS: [(u32, u16); 7] =
    [(0x108, 0x40), (0x10a, 0xc0), (0x10c, 0x40), (0x10e, 0x40), (0x112, 0x40), (0x114, 0x40), (0x116, 0xc0)];

/// Its parts' smoke: each a step (a byte, 0 none) along points (12 bytes each) where smoke and
/// sparks (n) are made; the step moves on only when they were made, or (steady) every frame.
const SHIP_SMOKES: [(u32, u32, u8, u32, bool); 7] = [
    (0x134, 0x13c, 16, 4, true),
    (0x135, 0x208, 13, 4, false),
    (0x137, 0x334, 6, 6, false),
    (0x13a, 0x3dc, 16, 4, true),
    (0x139, 0x3b8, 3, 4, false),
    (0x13b, 0x4a8, 5, 6, false),
    (0x138, 0x388, 3, 4, true),
];

/// The gunship's behaviour: past step 2 carried along by the stage's push (and 3/4 of a dot a frame
/// more); its life on the boss gauge. Before step 6, downed once its life is out or it is past the
/// mover: the bullets cleared, the stage's score (5000 at rate 30 if shot down, else none) and its
/// rate for its parts shot off, its parts all off and smoking; else its parts knocked off as their
/// lives run out. Then its step; its pose; its turn (and lean), its guns' kicks easing back, its
/// drift; its parts' smoke; its count.
pub fn ship_behaviour(cpu: &mut Cpu, o: u32) {
    if cpu.read8(o + SHIP_STEP) > 2 {
        let z = cpu.read32(o + POSITION + 8).wrapping_add(0xc000).wrapping_add(cpu.read32(STAGE_PUSH));
        cpu.write32(o + POSITION + 8, z);
    }
    let life = cpu.read32(o + SHIP_LIFE);
    nested!(cpu, 0x0604_64b8, hud::boss_gauge_life(cpu, life));
    let downed = cpu.read8(o + SHIP_STEP) <= 5
        && (cpu.read32(o + SHIP_LIFE) as i32 <= 0
            || (cpu.read32(MOVER + MOVER_Z) as i32) < cpu.read32(o + POSITION + 8) as i32);
    if downed {
        cpu.write8(o + SHIP_STEP, 6);
        nested!(cpu, 0x0605_61e0, screen::bullets_cleared(cpu));
        cpu.write32(STAGE_RATE_AT, 0);
        if cpu.read32(o + SHIP_LIFE) as i32 <= 0 {
            cpu.write32(STAGE_SCORE_AT, SHIP_DOWN_SCORE);
            cpu.write32(STAGE_RATE_AT, SHIP_DOWN_RATE);
        } else {
            cpu.write32(STAGE_SCORE_AT, 0);
        }
        let add = |cpu: &mut Cpu, n: u32| {
            let rate = cpu.read32(STAGE_RATE_AT).wrapping_add(n);
            cpu.write32(STAGE_RATE_AT, rate);
        };
        for (life, n) in SHIP_PART_RATES {
            if cpu.read32(o + life) as i32 <= 0 {
                add(cpu, n);
            }
        }
        if cpu.read32(o + 0x12c) as i32 <= 0 {
            add(cpu, 10);
        } else if cpu.read8(o + 0x139) != 0 {
            add(cpu, 5);
        }
        if cpu.read32(o + 0x130) as i32 <= 0 {
            add(cpu, 10);
        }
        cpu.write32(o + SHIP_LIFE, 0);
        for (at, bits) in SHIP_DOWNED_FLAGS {
            let v = cpu.read16(o + at) | bits;
            cpu.write16(o + at, v);
        }
        for at in [0x134, 0x135, 0x137, 0x139, 0x13a] {
            if cpu.read8(o + at) == 0 {
                cpu.write8(o + at, 1);
            }
        }
        if cpu.read8(o + 0x13b) == 0 {
            cpu.write8(o + 0x138, 1);
            cpu.write8(o + 0x13b, 1);
        }
    } else {
        count_down(cpu, o + SHIP_COUNT_536);
        for part in &SHIP_PART_LIVES {
            let f = cpu.read16(o + part.flags);
            if f & PART_OFF == 0 {
                if cpu.read32(o + part.life) as i32 <= part.low {
                    cpu.write16(o + part.flags, f | part.off);
                    for &at in part.smokes {
                        cpu.write8(o + at, 1);
                    }
                }
            } else if let Some(count) = part.count {
                count_down(cpu, o + count);
            }
        }
    }
    match cpu.read8(o + SHIP_STEP) {
        0 => nested!(cpu, 0x0608_ad90, ship_come_in(cpu, o)),
        1 => nested!(cpu, 0x0608_ae64, ship_hold(cpu, o)),
        2 => nested!(cpu, 0x0608_aef4, ship_smoke(cpu, o)),
        3 => nested!(cpu, 0x0608_b058, ship_fight(cpu, o)),
        4 => nested!(cpu, 0x0608_b10c, ship_charge(cpu, o)),
        5 => nested!(cpu, 0x0608_b264, ship_thrown_back(cpu, o)),
        6 => nested!(cpu, 0x0608_b30c, ship_sink(cpu, o)),
        7 => nested!(cpu, 0x0608_b490, ship_gone(cpu, o)),
        step => unreachable!("gunship step {step}"),
    }
    nested!(cpu, 0x0605_6644, models::pose_step(cpu, o + SHIP_SCRIPT));
    let turn = cpu.read16(o + TURN_Z).wrapping_add(cpu.read16(o + SHIP_TURN_SPEED));
    cpu.write16(o + TURN_Z, turn);
    let to = (cpu.read16(o + SHIP_TURN_SPEED) as i16 as i32).wrapping_neg().wrapping_mul(24) as u32;
    let lean = cpu.read16(o + SHIP_LEAN) as i16 as i32 as u32;
    let lean = nested!(cpu, 0x0604_6168, actors::approach(cpu, lean, to, 32));
    cpu.write16(o + SHIP_LEAN, lean as u16);
    for (kick, recoil, ease) in SHIP_KICKS {
        let k = cpu.read16(o + kick).wrapping_add(cpu.read16(o + recoil));
        cpu.write16(o + kick, k);
        if (k as i16) < 0 {
            let r = cpu.read16(o + recoil).wrapping_add(ease);
            cpu.write16(o + recoil, r);
        } else {
            cpu.write16(o + kick, 0);
            cpu.write16(o + recoil, 0);
        }
    }
    for k in [0, 4] {
        let v = cpu.read32(o + POSITION + k).wrapping_add(cpu.read32(o + SHIP_DRIFT + k));
        cpu.write32(o + POSITION + k, v);
    }
    for (step, points, n, sparks, steady) in SHIP_SMOKES {
        let s = cpu.read8(o + step);
        if s == 0 || s > n {
            continue;
        }
        let p = o + points + 12 * (s as u32 - 1);
        let made = nested!(cpu, 0x0605_968c, explosions::smoke_and_sparks(cpu, p, sparks)) as u8 != 0;
        if steady || made {
            let s = cpu.read8(o + step).wrapping_add(1);
            cpu.write8(o + step, s);
        }
    }
    let count = cpu.read16(o + SHIP_COUNT).wrapping_add(1);
    cpu.write16(o + SHIP_COUNT, count);
}

/// A word count run down to 0.
fn count_down(cpu: &mut Cpu, at: u32) {
    let n = cpu.read16(at);
    if n as i16 > 0 {
        cpu.write16(at, n - 1);
    }
}

/// A big model's shadow (the gunship's, or another's): every other pass of the main loop while its
/// step is under 3, five dark quads (the colour 0x1f0 past the first character's) about its place,
/// turned with it and lifted 8 toward the eye; 12 dots past the screen's depth for the gunship (only
/// while within 48 dots of it), else at the mover's place.
const SPRITE_STRUCT_CORNERS: u32 = 0x24;
const SHADOW_CORNERS: u32 = 0x2c;
const SHADOW_COLOUR: u16 = 0x1f0;
const SHIP_BEHAVIOUR: u32 = 0x0608_b560;
const SHADOW_QUADS: [[i16; 8]; 5] = [
    [-32, -32, 32, -32, 24, 24, -24, 24],
    [-60, -8, 0, -8, -32, 52, -44, 52],
    [-44, -44, -32, -44, 0, -8, -60, -8],
    [60, -8, 0, -8, 32, 56, 44, 56],
    [44, -44, 32, -44, 0, -8, 60, -8],
];

pub fn ship_shadow(cpu: &mut Cpu, o: u32) {
    if cpu.read32(LOOPS) & 1 != 0 || cpu.read8(o + SHIP_STEP) > 2 {
        return;
    }
    if cpu.read32(o + BEHAVIOUR) == SHIP_BEHAVIOUR {
        let depth = nested!(cpu, 0x0608_aadc, screen_18_depth(cpu));
        let off = (cpu.read32(o + POSITION + 8).wrapping_sub(depth) as i32).wrapping_abs();
        if off > 0x30_0000 {
            return;
        }
    }
    let d = cpu.stack();
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, d));
    cpu.write32(d, 0x101);
    cpu.write32(d + SPRITE_STRUCT_CORNERS, d + SHADOW_CORNERS);
    cpu.write8(d + SPRITE_STRUCT_LIFT, 0xf8);
    let colour = cpu.read16(CHAR_BASE).wrapping_add(SHADOW_COLOUR);
    cpu.write16(d + SPRITE_STRUCT_COLOUR, colour);
    for k in [0, 4] {
        let v = cpu.read32(o + POSITION + k);
        cpu.write32(d + 4 + k, v);
    }
    let turn = cpu.read16(o + TURN_Z);
    cpu.write16(d + SPRITE_STRUCT_ANGLE, turn);
    let z = if cpu.read32(o + BEHAVIOUR) == SHIP_BEHAVIOUR {
        nested!(cpu, 0x0608_aadc, screen_18_depth(cpu)).wrapping_add(0xc_0000)
    } else {
        cpu.read32(MOVER + MOVER_Z)
    };
    cpu.write32(d + 4 + 8, z);
    for quad in SHADOW_QUADS {
        for (k, v) in quad.into_iter().enumerate() {
            cpu.write16(d + SHADOW_CORNERS + 2 * k as u32, v as u16);
        }
        nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
    }
}

/// The gunship's models (in the stage's model set, 0x06074450's +0x1c: by part a pair, plain and
/// flashing, at +0x00 to +0x1c; the shot-off ones' at +0x20, +0x24, +0x28), its joints' angles
/// (its pose script's: the body's tilt; the hull's lift and depth (256ths of a dot) and its turn;
/// the head's tilt; the spray gun's turn; the launcher's turn; the mouth's depth; the wing's tilt;
/// the nose's tilt).
const STAGE_MODELS: u32 = 0x0607_4450;
const SHIP_MODELS: u32 = 0x1c;
const SHIP_JOINTS: u32 = SHIP_SCRIPT + 0x14;
const PART_FLASHED: u16 = 2;
const PART_FLASH: u16 = 1;

/// A part's model put (unless it is gone: its shot-off one if it has one and is off, else its
/// plain or flashing one).
fn ship_part(cpu: &mut Cpu, flags: u32, pair: u32, off: Option<u32>) {
    let f = cpu.read16(flags);
    if f & PART_GONE != 0 {
        return;
    }
    let model = match off {
        Some(off) if f & PART_OFF != 0 => ship_model(cpu, off, 0),
        _ => ship_model(cpu, pair, f),
    };
    cpu.call(SL_PUT_POLYGON, &[model]);
}

/// The model at the gunship's models' `at` (the flashing one of a pair by `f`).
fn ship_model(cpu: &mut Cpu, at: u32, f: u16) -> u32 {
    let set = cpu.read32(STAGE_MODELS);
    let models = cpu.read32(set + SHIP_MODELS);
    let list = cpu.read32(models + at);
    cpu.read32(list + 4 * (f & PART_FLASH) as u32)
}

/// A part's points (a table of whole-dot x, y, z, 8 bytes each) brought into the world at `out`
/// (12 bytes each).
fn ship_points(cpu: &mut Cpu, table: u32, n: u32, out: u32) {
    for i in 0..n {
        let [x, y, z] = [0, 2, 4].map(|k| (cpu.read16(table + 8 * i + k) as u32) << 16);
        cpu.call(SL_CALC_POINT, &[x, y, z, out + 12 * i]);
    }
}

/// The gunship's draw (its sprite `s`): its parts' flashes moved on; at its place, turned (z, y, x
/// with the body's tilt) and scaled, 24 dots on: the nose, the wing (with its tip while that is on)
/// and, unless the hull is gone, the hull turned with its lean, carrying the head with the mouth,
/// the spray gun and the launcher (pushed back by their kicks) and the tail; each part's smoke
/// points and gun places brought into the world. Then its shadow.
pub fn ship_draw(cpu: &mut Cpu, s: u32) {
    let o = s.wrapping_sub(FLAGS);
    for at in [0x108, 0x10a, 0x10c, 0x10e, 0x110, 0x114, 0x116] {
        let f = cpu.read16(o + at);
        if f & PART_FLASHED != 0 {
            cpu.write16(o + at, f & !(PART_FLASH | PART_FLASHED));
        }
        let f = cpu.read16(o + at);
        if f & PART_FLASH != 0 {
            cpu.write16(o + at, f | PART_FLASHED);
        }
    }
    let joint = |cpu: &mut Cpu, k: u32| {
        let joints = cpu.read32(o + SHIP_JOINTS);
        cpu.read16(joints + 2 * k)
    };
    let angle = |a: u16| a as i16 as i32 as u32;
    let push = |cpu: &mut Cpu| {
        cpu.call(SL_PUSH_MATRIX, &[]);
    };
    let pop = |cpu: &mut Cpu| {
        cpu.call(SL_POP_MATRIX, &[]);
    };
    push(cpu);
    let p = [0, 4, 8].map(|k| cpu.read32(o + POSITION + k));
    cpu.call(SL_TRANSLATE, &p);
    let a = cpu.read16(o + TURN_Z);
    cpu.call(SL_ROT_Z, &[angle(a)]);
    let a = cpu.read16(o + TURN_Y);
    cpu.call(SL_ROT_Y, &[angle(a)]);
    let a = cpu.read16(o + TURN_X).wrapping_add(joint(cpu, 0));
    cpu.call(SL_ROT_X, &[angle(a)]);
    let scales = [0, 4, 8].map(|k| cpu.read32(o + SCALES + k));
    cpu.call(SL_SCALE, &scales);
    cpu.call(SL_TRANSLATE, &[0, 0, 0x18_0000]);

    push(cpu);
    cpu.call(SL_TRANSLATE, &[0xfff4_0000, 0xc_0000, 0xfff8_0000]);
    let a = joint(cpu, 9);
    cpu.call(SL_ROT_X, &[angle(a)]);
    ship_part(cpu, o + 0x108, 0x10, Some(0x20));
    ship_points(cpu, 0x060a_03ec, 17, o + 0x13c);
    pop(cpu);

    push(cpu);
    cpu.call(SL_TRANSLATE, &[0xc_0000, 0x8_0000, 0xfff4_0000]);
    let a = joint(cpu, 8);
    cpu.call(SL_ROT_X, &[angle(a)]);
    let f = cpu.read16(o + 0x114);
    if f & PART_GONE == 0 {
        ship_part(cpu, o + 0x114, 0x18, Some(0x28));
        if f & PART_OFF == 0 {
            if cpu.read16(o + 0x112) & PART_GONE == 0 {
                push(cpu);
                cpu.call(SL_TRANSLATE, &[0xc_0000, 0xfff7_0000, 0]);
                let f = cpu.read16(o + 0x114);
                let model = ship_model(cpu, 0x14, f);
                cpu.call(SL_PUT_POLYGON, &[model]);
                pop(cpu);
            }
            for (x, z, out) in [(0x8_0000, 0xfff0_0000, 0x3b8), (0x38_0000, 0xfff8_0000, 0x3c4), (0x58_0000, 0xfffc_0000, 0x3d0)] {
                cpu.call(SL_CALC_POINT, &[x, 0xfff0_0000, z, o + out]);
            }
        }
    }
    ship_points(cpu, 0x060a_0594, 17, o + 0x3dc);
    pop(cpu);

    push(cpu);
    if cpu.read16(o + 0x10c) & PART_GONE == 0 {
        let (lift, depth) = (joint(cpu, 1), joint(cpu, 2));
        cpu.call(SL_TRANSLATE, &[0, angle(lift) << 8, angle(depth) << 8]);
        let a = joint(cpu, 3).wrapping_add(cpu.read16(o + SHIP_LEAN));
        cpu.call(SL_ROT_Y, &[angle(a)]);
        ship_part(cpu, o + 0x10c, 0x08, None);
        ship_points(cpu, 0x060a_04e4, 11, o + 0x2b0);
        cpu.call(SL_CALC_POINT, &[0xffee_0000, 0xfff6_0000, 0xffec_0000, o + TWIN_GUNS[0]]);
        cpu.call(SL_CALC_POINT, &[0x12_0000, 0xfff6_0000, 0xffec_0000, o + TWIN_GUNS[1]]);
        push(cpu);
        cpu.call(SL_TRANSLATE, &[0, 0xfff8_0000, 0]);

        push(cpu);
        cpu.call(SL_TRANSLATE, &[0, 0, 0]);
        let a = joint(cpu, 4);
        cpu.call(SL_ROT_X, &[angle(a)]);
        push(cpu);
        let depth = joint(cpu, 7);
        cpu.call(SL_TRANSLATE, &[0, 0, angle(depth) << 8]);
        ship_part(cpu, o + 0x10e, 0x0c, Some(0x24));
        ship_points(cpu, 0x060a_053c, 7, o + 0x334);
        cpu.call(SL_CALC_POINT, &[0, 0xfff4_0000, 0xffe0_0000, o + SHIP_MOUTH]);
        pop(cpu);
        pop(cpu);

        push(cpu);
        let kick = angle(cpu.read16(o + 0xfa)) << 8;
        cpu.call(SL_TRANSLATE, &[0xfff0_0000, 0xfff0_0000, 0xfffc_0000u32.wrapping_sub(kick)]);
        let a = joint(cpu, 5);
        cpu.call(SL_ROT_Z, &[angle(a)]);
        ship_part(cpu, o + SHIP_PART_A, 0x04, None);
        ship_points(cpu, 0x060a_0474, 14, o + 0x208);
        cpu.call(SL_CALC_POINT, &[0xffdc_0000, 0xfff8_0000, 0xffc0_0000, o + SPRAY_GUN]);
        pop(cpu);

        push(cpu);
        let kick = angle(cpu.read16(o + 0xfe)) << 8;
        cpu.call(SL_TRANSLATE, &[0x28_0000, 0xffe0_0000, kick.wrapping_neg()]);
        let a = joint(cpu, 6);
        cpu.call(SL_ROT_Z, &[angle(a)]);
        let a = cpu.read16(o + 0xfe).wrapping_mul(2);
        cpu.call(SL_ROT_X, &[angle(a)]);
        ship_part(cpu, o + SHIP_PART_C, 0x1c, None);
        ship_points(cpu, 0x060a_061c, 6, o + 0x4a8);
        cpu.call(SL_CALC_POINT, &[0xc_0000, 0xfff0_0000, 0xffe8_0000, o + LAUNCHER]);
        pop(cpu);

        push(cpu);
        let kick = angle(cpu.read16(o + 0xfe)) << 7;
        cpu.call(SL_TRANSLATE, &[0, 0xfff0_0000, 0xfff4_0000u32.wrapping_sub(kick)]);
        ship_part(cpu, o + SHIP_PART_C, 0x00, None);
        ship_points(cpu, 0x060a_0574, 4, o + 0x388);
        pop(cpu);
        pop(cpu);
    }
    pop(cpu);
    pop(cpu);
    nested!(cpu, 0x0608_bb1c, ship_shadow(cpu, o));
}

/// The gunship's parts (parts.rs's): each its count, its points (and the table of their places
/// and kinds), its hit flags and the life they share.
const SHIP_PART_SETS: [(u32, u32, u32, u32, u32); 7] = [
    (17, 0x13c, 0x108, 0x118, 0x060a_03ec),
    (14, 0x208, 0x10a, 0x11c, 0x060a_0474),
    (11, 0x2b0, 0x10c, 0x120, 0x060a_04e4),
    (7, 0x334, 0x10e, 0x124, 0x060a_053c),
    (4, 0x388, 0x116, 0x130, 0x060a_0574),
    (17, 0x3dc, 0x114, 0x12c, 0x060a_0594),
    (6, 0x4a8, 0x116, 0x130, 0x060a_061c),
];
const SHIP_LIVES: [(u32, u32); 7] =
    [(0x118, 1500), (0x11c, 3000), (0x120, 5500), (0x124, 5500), (0x128, 3000), (0x12c, 1500), (0x130, 3000)];
/// Its flags (FLAGS'), its pose (10 joints, their angles at +0x98 and turns at +0xac, its poses,
/// the first set), its first script, the sound it comes in with.
const SHIP_FLAGS_SET: u32 = 0xc8_0000;
const SHIP_JOINT_COUNT: u16 = 10;
const SHIP_POSES: u32 = 0x060a_0714;
const SHIP_FIRST_POSE: u32 = 0x060a_064c;
const SHIP_SCRIPT_0: u32 = 0x060a_0754;
const SHIP_SOUND: u32 = 77;
const SHIP_DRAW: u32 = 0x0608_bd3c;
static PARTS_SET_PROLOGUE: std::sync::OnceLock<super::Prologue> = std::sync::OnceLock::new();

/// The gunship `o` set up with the number `n`: its parts all not yet hittable, their lives, their
/// smoke none; its count at 0x80; its behaviour and draw; numbered; still, unturned, unrolled (an
/// eighth of a roll in), at 3/4 size; its parts made; its pose and first script; step 0, 64 dots
/// up and 192 back; the stage's push stopped; the boss gauge for its life (5500); its sound.
pub fn ship_set(cpu: &mut Cpu, o: u32, n: u32) {
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + BEAM_LIST));
    for k in (0..8).rev() {
        cpu.write16(o + SHIP_PARTS_HIT + 2 * k, PART_NOT_YET);
    }
    for (at, life) in SHIP_LIVES {
        cpu.write32(o + at, life);
    }
    for at in 0x134..0x13c {
        cpu.write8(o + at, 0);
    }
    for at in [0x532, 0x534, SHIP_COUNT_536, 0x538] {
        cpu.write16(o + at, 0);
    }
    cpu.write16(o + SHIP_COUNT, 0x80);
    cpu.write32(o + BEHAVIOUR, SHIP_BEHAVIOUR);
    let flags = cpu.read32(o + FLAGS) | SHIP_FLAGS_SET;
    cpu.write32(o + FLAGS, flags);
    cpu.write32(o + MODEL, SHIP_DRAW);
    cpu.write8(o + NUMBER, n as u8);
    let n = cpu.read8(o + NUMBER) as i8 as i32;
    cpu.write32(PLAYERS.wrapping_add((4 * n) as u32), o);
    for at in [SHIP_FLAGS, SHIP_LEAN, SHIP_TURN_SPEED, SHIP_TURN_GOAL, SHIP_ROLL_SPEED, 0xfa, SHIP_RECOIL, 0xfe, SHIP_RECOIL_2, SHIP_DOWN_COUNT] {
        cpu.write16(o + at, 0);
    }
    cpu.write32(o + SHIP_ROLL, 0x2000);
    cpu.write8(o + SHIP_ROLLS, 0);
    for k in [0, 4, 8] {
        cpu.write32(o + POSITION + k, 0);
    }
    for k in [0, 4] {
        cpu.write32(o + SHIP_DRIFT + k, 0);
    }
    for k in [0, 4, 8] {
        cpu.write32(o + SCALES + k, 0xc000);
    }
    for (count, points, state, life, table) in SHIP_PART_SETS {
        let number = cpu.read8(o + NUMBER) as u32;
        let (points, state, life) = (o + points, o + state, o + life);
        cpu.nested_with_stack(&PARTS_SET_PROLOGUE, 0x0605_6ea4, &[table, number], |cpu| {
            parts::parts_set(cpu, count, points, state, life, table, number)
        });
    }
    cpu.write16(o + SHIP_SCRIPT + 0x10, SHIP_JOINT_COUNT);
    cpu.write32(o + SHIP_JOINTS, o + 0x98);
    cpu.write32(o + SHIP_SCRIPT + 0x18, o + 0xac);
    cpu.write32(o + SHIP_SCRIPT + 0x1c, SHIP_POSES);
    nested!(cpu, 0x0605_65f0, models::pose_set(cpu, o + SHIP_SCRIPT, SHIP_FIRST_POSE));
    nested!(cpu, 0x0605_6628, actors::script_start(cpu, o + SHIP_SCRIPT, SHIP_SCRIPT_0));
    cpu.write8(o + SHIP_STEP, 0);
    cpu.write32(o + POSITION + 4, 0xffc0_0000);
    cpu.write32(o + POSITION + 8, 0xff40_0000);
    cpu.write32(STAGE_PUSH, 0);
    nested!(cpu, 0x0604_64d8, hud::boss_gauge_set(cpu, 5500));
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, SHIP_SOUND));
}

/// A gunship made (numbered: list 2, 0x53c bytes, numbered_exec's; an enemy's number): its
/// number, or 0 if none was free.
const SHIP_SIZE: u32 = 0x53c;

pub fn ship_make(cpu: &mut Cpu) -> u32 {
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, ENEMY_NUMBERS)) as i8;
    if n == -1 {
        return 0;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, SHIP_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, NUMBERED_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0608_c574, ship_set(cpu, o, n as i32 as u32));
    n as i32 as u32
}

/// The gunship as it first passes over (a gunship's model, untouchable): carried with the mover,
/// 512 dots ahead of it, 28 dots below and 24 dots behind it and rising 7/16 of a dot a frame
/// (its +0xf4 counting down); stopped once within 48 dots of the eye. Upright, its hull turning a
/// 48th of the way a frame to the nearer player (its lean), its pose moving on, its count.
const PASS_RISE: u32 = 0xf4;

pub fn passing_ship_behaviour(cpu: &mut Cpu, o: u32) {
    let x = cpu.read32(MOVER + 0xc).wrapping_add(0x200_0000);
    cpu.write32(o + POSITION, x);
    let rise = (cpu.read16(o + PASS_RISE) as i16 as i32).wrapping_mul(0x7000) as u32;
    let y = cpu.read32(MOVER + MOVER_Y).wrapping_add(rise.wrapping_add(0xfe40_0000));
    cpu.write32(o + POSITION + 4, y);
    let z = cpu.read32(MOVER + MOVER_Z).wrapping_add(0xffe8_0000);
    cpu.write32(o + POSITION + 8, z);
    if z as i32 <= 0x2f_ffff {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    cpu.write16(o + TURN_X, 0x4000);
    let player = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + FLAGS));
    let a = nested!(cpu, 0x0604_5e30, aim::angle_to(cpu, o, player));
    let lean = cpu.read16(o + SHIP_LEAN) as i16 as i32 as u32;
    let d = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, lean, a as i16 as i32 as u32));
    let step = nested!(cpu, 0x0600_6a78, math::quotient(cpu, d, 48));
    let lean = cpu.read16(o + SHIP_LEAN).wrapping_add(step as u16);
    cpu.write16(o + SHIP_LEAN, lean);
    let rise = cpu.read16(o + PASS_RISE).wrapping_sub(1);
    cpu.write16(o + PASS_RISE, rise);
    nested!(cpu, 0x0605_6644, models::pose_step(cpu, o + SHIP_SCRIPT));
    let count = cpu.read16(o + SHIP_COUNT).wrapping_add(1);
    cpu.write16(o + SHIP_COUNT, count);
}

/// Its draw (its sprite `s`): the gunship's, its parts all plain, its hull turned back to front by
/// its lean and its head tilted 0x1000 less; no points brought out. Then its shadow.
pub fn passing_ship_draw(cpu: &mut Cpu, s: u32) {
    let o = s.wrapping_sub(FLAGS);
    let joint = |cpu: &mut Cpu, k: u32| {
        let joints = cpu.read32(o + SHIP_JOINTS);
        cpu.read16(joints + 2 * k)
    };
    let angle = |a: u16| a as i16 as i32 as u32;
    let push = |cpu: &mut Cpu| {
        cpu.call(SL_PUSH_MATRIX, &[]);
    };
    let pop = |cpu: &mut Cpu| {
        cpu.call(SL_POP_MATRIX, &[]);
    };
    let put = |cpu: &mut Cpu, at: u32| {
        let model = ship_model(cpu, at, 0);
        cpu.call(SL_PUT_POLYGON, &[model]);
    };
    push(cpu);
    let p = [0, 4, 8].map(|k| cpu.read32(o + POSITION + k));
    cpu.call(SL_TRANSLATE, &p);
    let a = cpu.read16(o + TURN_Z);
    cpu.call(SL_ROT_Z, &[angle(a)]);
    let a = cpu.read16(o + TURN_Y);
    cpu.call(SL_ROT_Y, &[angle(a)]);
    let a = cpu.read16(o + TURN_X).wrapping_add(joint(cpu, 0));
    cpu.call(SL_ROT_X, &[angle(a)]);
    let scales = [0, 4, 8].map(|k| cpu.read32(o + SCALES + k));
    cpu.call(SL_SCALE, &scales);
    cpu.call(SL_TRANSLATE, &[0, 0, 0x18_0000]);

    push(cpu);
    cpu.call(SL_TRANSLATE, &[0xfff4_0000, 0xc_0000, 0xfff8_0000]);
    let a = joint(cpu, 9);
    cpu.call(SL_ROT_X, &[angle(a)]);
    put(cpu, 0x10);
    pop(cpu);

    push(cpu);
    cpu.call(SL_TRANSLATE, &[0xc_0000, 0x8_0000, 0xfff4_0000]);
    let a = joint(cpu, 8);
    cpu.call(SL_ROT_X, &[angle(a)]);
    put(cpu, 0x18);
    push(cpu);
    cpu.call(SL_TRANSLATE, &[0xc_0000, 0xfff7_0000, 0]);
    put(cpu, 0x14);
    pop(cpu);
    pop(cpu);

    push(cpu);
    let (lift, depth) = (joint(cpu, 1), joint(cpu, 2));
    cpu.call(SL_TRANSLATE, &[0, angle(lift) << 8, angle(depth) << 8]);
    let a = joint(cpu, 3).wrapping_add(cpu.read16(o + SHIP_LEAN)).wrapping_add(0x8000);
    cpu.call(SL_ROT_Y, &[angle(a)]);
    put(cpu, 0x08);
    push(cpu);
    cpu.call(SL_TRANSLATE, &[0, 0xfff8_0000, 0]);

    push(cpu);
    cpu.call(SL_TRANSLATE, &[0, 0, 0]);
    let a = joint(cpu, 4).wrapping_add(0xf000);
    cpu.call(SL_ROT_X, &[angle(a)]);
    push(cpu);
    let depth = joint(cpu, 7);
    cpu.call(SL_TRANSLATE, &[0, 0, angle(depth) << 8]);
    put(cpu, 0x0c);
    pop(cpu);
    pop(cpu);

    push(cpu);
    cpu.call(SL_TRANSLATE, &[0xfff0_0000, 0xfff0_0000, 0xfffc_0000]);
    let a = joint(cpu, 5);
    cpu.call(SL_ROT_Z, &[angle(a)]);
    put(cpu, 0x04);
    pop(cpu);

    push(cpu);
    cpu.call(SL_TRANSLATE, &[0x28_0000, 0xffe0_0000, 0]);
    let a = joint(cpu, 6);
    cpu.call(SL_ROT_Z, &[angle(a)]);
    put(cpu, 0x1c);
    pop(cpu);

    push(cpu);
    cpu.call(SL_TRANSLATE, &[0, 0xfff0_0000, 0xfff4_0000]);
    put(cpu, 0x00);
    pop(cpu);
    pop(cpu);
    pop(cpu);
    pop(cpu);
    nested!(cpu, 0x0608_bb1c, ship_shadow(cpu, o));
}

/// The passing gunship `o` set up with the number `n`: its behaviour and draw, numbered, step 0,
/// at 3/4 size, no lean or rise; its pose and its second script; at 0.
const PASSING_SHIP_BEHAVIOUR: u32 = 0x0608_c910;
const PASSING_SHIP_DRAW: u32 = 0x0608_c9e8;

pub fn passing_ship_set(cpu: &mut Cpu, o: u32, n: u32) {
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + BEAM_LIST));
    cpu.write32(o + BEHAVIOUR, PASSING_SHIP_BEHAVIOUR);
    let flags = cpu.read32(o + FLAGS) | SHIP_FLAGS_SET;
    cpu.write32(o + FLAGS, flags);
    cpu.write32(o + MODEL, PASSING_SHIP_DRAW);
    cpu.write8(o + NUMBER, n as u8);
    let n = cpu.read8(o + NUMBER) as i8 as i32;
    cpu.write32(PLAYERS.wrapping_add((4 * n) as u32), o);
    cpu.write8(o + SHIP_STEP, 0);
    for k in [0, 4, 8] {
        cpu.write32(o + SCALES + k, 0xc000);
    }
    cpu.write16(o + SHIP_LEAN, 0);
    cpu.write16(o + PASS_RISE, 0);
    cpu.write16(o + SHIP_SCRIPT + 0x10, SHIP_JOINT_COUNT);
    cpu.write32(o + SHIP_JOINTS, o + 0x98);
    cpu.write32(o + SHIP_SCRIPT + 0x18, o + 0xac);
    cpu.write32(o + SHIP_SCRIPT + 0x1c, SHIP_POSES);
    nested!(cpu, 0x0605_65f0, models::pose_set(cpu, o + SHIP_SCRIPT, SHIP_FIRST_POSE));
    nested!(cpu, 0x0605_6628, actors::script_start(cpu, o + SHIP_SCRIPT, SHIP_SCRIPT_2));
    cpu.write8(o + SHIP_STEP, 0);
    for k in [0, 4, 8] {
        cpu.write32(o + POSITION + k, 0);
    }
}

/// A passing gunship made (as the gunship is): its number, or 0 if none was free.
pub fn passing_ship_make(cpu: &mut Cpu) -> u32 {
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, ENEMY_NUMBERS)) as i8;
    if n == -1 {
        return 0;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, SHIP_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, NUMBERED_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0608_cc4c, passing_ship_set(cpu, o, n as i32 as u32));
    n as i32 as u32
}

// ---- the winged boss ------------------------------------------------------------------------------

/// An orbiter's description (orbits.rs's): its scales, its angle (a word), its colours' index (a
/// word, the index its low byte), its radius and the radius's speed, its kind (a word, the kind
/// its low byte), its angle offset and turn, its lift (a word, the lift its low byte), its scales'
/// speeds.
const D_SCALES: u32 = 0x0c;
const D_ANGLE: u32 = 0x18;
const D_COLOURS: u32 = 0x20;
const D_RADIUS: u32 = 0x24;
const D_ORBIT_KIND: u32 = 0x2c;
const D_ANGLE_OFFSET: u32 = 0x2e;
const D_TURN: u32 = 0x30;
const D_LIFT: u32 = 0x32;
const D_SCALE_SPEED: u32 = 0x34;
const ORBIT_SET: u32 = 0x0605_9554;
static ORBIT_SET_PROLOGUE: std::sync::OnceLock<super::Prologue> = std::sync::OnceLock::new();

/// A glow about a gun at `centre` (kind 2, colours 1: half size, shrinking a 32nd a frame, turned
/// by the angle at `angle_from` and `offset`, spinning 0x400 a frame, lifted 12), ending once the
/// word at `watched` has its part's off bit.
pub fn gun_glow(cpu: &mut Cpu, watched: u32, angle_from: u32, centre: u32, offset: u32) {
    let d = cpu.stack();
    cpu.write16(d + D_ORBIT_KIND, 2);
    cpu.write16(d + D_COLOURS, 1);
    for k in [0, 4] {
        cpu.write32(d + D_SCALES + k, 0x8000);
        cpu.write32(d + D_SCALE_SPEED + k, 0xffff_f800);
    }
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write16(d + D_ANGLE, r as u16);
    cpu.write16(d + D_ANGLE_OFFSET, offset as u16);
    cpu.write16(d + D_TURN, 0x400);
    cpu.write16(d + D_LIFT, 12);
    for k in [0, 4] {
        cpu.write32(d + D_RADIUS + k, 0);
    }
    cpu.nested_with_stack(&ORBIT_SET_PROLOGUE, ORBIT_SET, &[angle_from], |cpu| {
        orbits::orbit_set(cpu, d, watched, PART_OFF as u32, centre, angle_from)
    });
}

/// The winged boss's count (+0x78, a word), its wings' hit flags (the left's +0x288, the right's
/// +0x1f0: their part off bit), their guns' places (+0x2a4, +0x2b0) and, once they are off, their
/// stumps' (+0x2bc, +0x2c8).
const WB_COUNT: u32 = 0x78;
const WB_LEFT: u32 = 0x288;
const WB_RIGHT: u32 = 0x1f0;
const WB_LEFT_GUN: u32 = 0x2a4;
const WB_RIGHT_GUN: u32 = 0x2b0;
const WB_LEFT_STUMP: u32 = 0x2bc;
const WB_RIGHT_STUMP: u32 = 0x2c8;
/// The bullets' description (bullets.rs's): swirling (flag 1) from its start angle (a word), turning
/// (a word) out to a radius (a word).
const SHOT_FLAGS: u32 = 0x2e;
const SHOT_SWIRL_ANGLE: u32 = 0x36;
const SHOT_SWIRL_TURN: u32 = 0x38;
const SHOT_SWIRL_RADIUS: u32 = 0x3a;
const WB_SWIRL_RADII: u32 = 0x060a_08dc;
const WB_SHOT_SOUND: u32 = 27;

/// Every 30th frame its volley (sound 27): from each wing's gun still on three bullets back along
/// it, 1.5 dots a frame, swirling a third of a turn apart (the left's one way, the right's the
/// other) out to a radius by difficulty; each with a glow.
pub fn wb_volley(cpu: &mut Cpu, o: u32) {
    let count = cpu.read16(o + WB_COUNT) as u32;
    if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, count, 30)) != 0 {
        return;
    }
    let shot = cpu.stack();
    let number = cpu.read8(o + NUMBER);
    cpu.write8(shot + SHOT_NUMBER, number);
    cpu.write32(shot + 8, 0);
    let back = cpu.read16(o + TURN_Z).wrapping_add(0x8000);
    cpu.write16(shot + SHOT_ANGLE, back);
    cpu.write16(shot + SHOT_FLAGS, 1);
    cpu.write32(shot + SHOT_SPEED, 0x1_8000);
    cpu.write16(shot + SHOT_SWIRL_ANGLE, 0);
    let difficulty = cpu.read8(DIFFICULTY) as i8 as i32 as u32;
    let radius = cpu.read8(WB_SWIRL_RADII.wrapping_add(difficulty)) as u16;
    cpu.write16(shot + SHOT_SWIRL_RADIUS, radius);
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, WB_SHOT_SOUND));
    for _ in 0..3 {
        for (wing, gun, turn) in [(WB_LEFT, WB_LEFT_GUN, 0xfd00u16), (WB_RIGHT, WB_RIGHT_GUN, 0x300)] {
            if cpu.read16(o + wing) & PART_OFF != 0 {
                continue;
            }
            cpu.write16(shot + SHOT_SWIRL_TURN, turn);
            for k in [0, 4] {
                let v = cpu.read32(o + gun + k);
                cpu.write32(shot + k, v);
            }
            nested!(cpu, 0x0605_3db0, bullets::bullet_set(cpu, shot));
            nested!(cpu, 0x0608_cd98, gun_glow(cpu, o + wing, o + TURN_Z, o + gun, 0x8000));
        }
        let a = cpu.read16(shot + SHOT_SWIRL_ANGLE).wrapping_add(0x5555);
        cpu.write16(shot + SHOT_SWIRL_ANGLE, a);
    }
}

/// Its core's hit flags (+0x1a0). The missiles' description (bullets.rs's): homing frames (by
/// difficulty), the aim's offset, the launch's direction (words), the speed.
const WB_CORE: u32 = 0x1a0;
const MISSILE_HOMING: u32 = 0x2e;
const MISSILE_AIM: u32 = 0x32;
const MISSILE_LAUNCH: u32 = 0x34;
const MISSILE_SPEED: u32 = 0x38;
const WB_HOMING_FRAMES: u32 = 0x060a_08b0;
const WB_MISSILE_SOUND: u32 = 33;

/// Its missiles: from each wing's stump (the wing shot off) a missile (sound 33) heading back and
/// out (0x5000 to that side of straight back), launched further out, homing for frames by
/// difficulty, 2 dots a frame; each with a glow that ends with the core.
pub fn wb_missiles(cpu: &mut Cpu, o: u32) {
    let d = cpu.stack();
    let number = cpu.read8(o + NUMBER);
    cpu.write8(d + SHOT_NUMBER, number);
    cpu.write32(d + 8, 0);
    cpu.write32(d + MISSILE_SPEED, 0x2_0000);
    let difficulty = cpu.read8(DIFFICULTY) as i8 as i32 as u32;
    let homing = cpu.read16(WB_HOMING_FRAMES.wrapping_add(2 * difficulty));
    cpu.write16(d + MISSILE_HOMING, homing);
    cpu.write16(d + MISSILE_AIM, 0x800);
    for (wing, stump, heading, launch, offset) in
        [(WB_LEFT, WB_LEFT_STUMP, 0xb000u16, 0xe000u16, 0xc000), (WB_RIGHT, WB_RIGHT_STUMP, 0x5000, 0x2000, 0x4000)]
    {
        if cpu.read16(o + wing) & PART_OFF == 0 {
            continue;
        }
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, WB_MISSILE_SOUND));
        let a = cpu.read16(o + TURN_Z).wrapping_add(heading);
        cpu.write16(d + SHOT_ANGLE, a);
        cpu.write16(d + MISSILE_LAUNCH, a.wrapping_add(launch));
        for k in [0, 4] {
            let v = cpu.read32(o + stump + k);
            cpu.write32(d + k, v);
        }
        nested!(cpu, 0x0605_49f8, bullets::missile_set(cpu, d));
        nested!(cpu, 0x0608_cd98, gun_glow(cpu, o + WB_CORE, o + TURN_Z, o + stump, offset));
    }
}

/// A flame (an actor_exec bullet from the winged boss): its speed (+0x80, +0x84; its push the same).
/// It flies on, its frame flickering (2, 3), flipped every frame, its colour cycling; ended off the
/// screen.
const FLAME_SPEED: u32 = 0x80;
const FLAME_PUSH: u32 = 0x5c;
const FLAME_BEHAVIOUR: u32 = 0x0608_d0a8;
const FLAME_FLAGS: u32 = 0x2000_0400;
const FLAME_KIND: u8 = 42;
const FLAME_BOXES: u32 = 0x060a_0b7c;
const WB_FLAME_SPEEDS: u32 = 0x060a_089c;

pub fn flame_behaviour(cpu: &mut Cpu, o: u32) {
    if nested!(cpu, 0x0605_6a24, aim::off_screen(cpu, o + FLAGS)) as u8 != 0 {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    for k in [0, 4] {
        let v = cpu.read32(o + POSITION + k).wrapping_add(cpu.read32(o + FLAME_SPEED + k));
        cpu.write32(o + POSITION + k, v);
    }
    let frame = ((cpu.read32(GAME_FRAMES) >> 1) & 1) as u8 + 2;
    cpu.write8(o + SPRITE_FRAME, frame);
    let flags = cpu.read32(o + FLAGS) ^ FLIP_X;
    cpu.write32(o + FLAGS, flags);
    let colour = match cpu.read32(GAME_FRAMES) & 3 {
        0 => cpu.read16(CHAR_BASE).wrapping_add(0x40),
        3 => cpu.read16(CHAR_BASE).wrapping_add(0x10),
        _ => cpu.read16(CHAR_BASE),
    };
    cpu.write16(o + OWN_COLOUR, colour);
}

/// A flame made (list 2, 0xbc bytes) from the shot description `d` (its place, number, direction
/// and speed): kind 42 frame 2, lifted 8, the first character's colour, its boxes, side 1, type 5,
/// facing back along its way. It, or 0.
pub fn flame_set(cpu: &mut Cpu, d: u32) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0xbc)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + BEAM_LIST));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + BEHAVIOUR, FLAME_BEHAVIOUR);
    let n = cpu.read8(d + SHOT_NUMBER);
    cpu.write8(o + NUMBER, n);
    cpu.write32(o + FLAGS, FLAME_FLAGS);
    for k in [0, 4, 8] {
        let v = cpu.read32(d + k);
        cpu.write32(o + POSITION + k, v);
    }
    cpu.write8(o + BYTE_33, 8);
    cpu.write8(o + SPRITE_FRAME, 2);
    cpu.write8(o + SPRITE_KIND, FLAME_KIND);
    let colour = cpu.read16(CHAR_BASE);
    cpu.write16(o + OWN_COLOUR, colour);
    cpu.write16(o + COLOUR, colour);
    cpu.write32(o + BEAM_LIST, FLAME_BOXES);
    cpu.write8(o + SIDE, 1);
    cpu.write8(o + TYPE, 5);
    for k in [0, 4] {
        let speed = cpu.read32(d + SHOT_SPEED);
        let a = cpu.read16(d + SHOT_ANGLE) as i16 as i32 as u32;
        let v = if k == 0 {
            let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
            nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, speed))
        } else {
            let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
            nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, c.wrapping_neg(), speed))
        };
        cpu.write32(o + FLAME_SPEED + k, v);
    }
    for k in [0, 4] {
        let v = cpu.read32(o + FLAME_SPEED + k);
        cpu.write32(o + FLAME_PUSH + k, v);
    }
    let back = cpu.read16(d + SHOT_ANGLE).wrapping_add(0x8000);
    cpu.write16(o + TURN_Z, back);
    o
}

/// A flame from `d` at the winged boss's flame speed for the difficulty.
pub fn wb_flame(cpu: &mut Cpu, d: u32) -> u32 {
    let difficulty = cpu.read8(DIFFICULTY) as i8 as i32 as u32;
    let speed = cpu.read32(WB_FLAME_SPEEDS.wrapping_add(4 * difficulty));
    cpu.write32(d + SHOT_SPEED, speed);
    nested!(cpu, 0x0608_d130, flame_set(cpu, d))
}

/// Its mouth's place (+0x298), which of its fans is next (+0xda: its low bit), the fans' directions
/// (two rows of nine words, from straight back out either way in turn: whole and half steps).
const WB_MOUTH: u32 = 0x298;
const WB_FAN: u32 = 0xda;
const WB_FAN_ANGLES: u32 = 0x060a_0b84;
/// The fans' last directions by difficulty (the easiest first; the even fan's, the odd's).
const WB_FAN_LASTS: [(u32, u32); 5] = [(2, 1), (2, 3), (4, 3), (4, 5), (6, 5)];

/// A fan of flames from its mouth (sound 27): by difficulty and which fan is next, 2 to 7 of its row
/// of directions; a glow at its mouth that ends with the core; the other fan next.
pub fn wb_flame_fan(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, WB_SHOT_SOUND));
    let (even, odd) = WB_FAN_LASTS[match cpu.read8(DIFFICULTY) {
        d @ 1..=4 => d as usize,
        _ => 0,
    }];
    let last = if cpu.read8(o + WB_FAN) & 1 != 0 { odd } else { even };
    let d = cpu.stack();
    for i in 0..=last {
        let number = cpu.read8(o + NUMBER);
        cpu.write8(d + SHOT_NUMBER, number);
        let row = cpu.read8(o + WB_FAN) as u32;
        let a = cpu.read16(WB_FAN_ANGLES + 2 * (9 * row + i));
        let a = a.wrapping_add(0x8000).wrapping_add(cpu.read16(o + TURN_Z));
        cpu.write16(d + SHOT_ANGLE, a);
        for k in [0, 4] {
            let v = cpu.read32(o + WB_MOUTH + k);
            cpu.write32(d + k, v);
        }
        cpu.write32(d + 8, 0);
        nested!(cpu, 0x0608_d274, wb_flame(cpu, d));
    }
    nested!(cpu, 0x0608_cd98, gun_glow(cpu, o + WB_CORE, o + TURN_Z, o + WB_MOUTH, 0x8000));
    let fan = cpu.read8(o + WB_FAN) ^ 1;
    cpu.write8(o + WB_FAN, fan);
}

// ---- the winged boss's breath ---------------------------------------------------------------------

/// The breath (an actor_exec bullet from one of the winged boss's arms, +0x6c): its reach (+0xac,
/// 16.16 of its full length) and how fast it grows (+0x8c, a word), its width (+0xb0, what is left
/// of 1); six glows (+0x6c their breath) strung along it, each at a place (+0x8a, a word: dots at
/// full reach), in steps (+0x7c: 0 strung, 1 spent). A spent piece hits nothing (and has 0x100).
const BREATH_OF: u32 = 0x6c;
const BREATH_REACH: u32 = 0xac;
const BREATH_GROWTH: u32 = 0x8c;
const BREATH_WIDTH: u32 = 0xb0;
const BREATH_AT: u32 = 0x8a;
const BREATH_STEP: u32 = 0x7c;
const BREATH_SPENT: u32 = NO_HITS | 0x100;
const BREATH_SPENT_FRAME: u8 = 90;
/// The arm's life (+0x160) and its mouth (+0x168).
const ARM_LIFE: u32 = 0x160;
const ARM_MOUTH: u32 = 0x168;

/// A glow on the breath: ended with it; turned with it. Strung: at its place along the breath, as
/// wide as the breath less a 2048th for each dot of its place, three times its reach high (the
/// last glow, at frame 1, six); once under 1/8 wide it is spent (frame 90, a quarter wide, twice as
/// high), flipped at random and a pixel about its place. Coloured as the breath.
pub fn breath_glow_behaviour(cpu: &mut Cpu, o: u32) {
    let b = cpu.read32(o + BREATH_OF);
    if cpu.read8(b + OBJ_MODE) as i8 > 111 {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    let turn = cpu.read16(b + TURN_Z);
    cpu.write16(o + TURN_Z, turn);
    let along = |cpu: &mut Cpu| (cpu.read16(o + BREATH_AT) as i16 as i32).wrapping_mul(cpu.read32(b + BREATH_REACH) as i32) as u32;
    let back = |cpu: &mut Cpu| cpu.read16(b + TURN_Z).wrapping_add(0x8000) as i16 as i32 as u32;
    if cpu.read8(o + BREATH_STEP) == 0 {
        if cpu.read32(o + SCALES) as i32 <= 0x1fff {
            cpu.write8(o + BREATH_STEP, 1);
            let flags = cpu.read32(o + FLAGS) | BREATH_SPENT;
            cpu.write32(o + FLAGS, flags);
            cpu.write8(o + SPRITE_KIND, 0);
            cpu.write8(o + SPRITE_FRAME, BREATH_SPENT_FRAME);
            cpu.write32(o + SCALES, 0x4000);
            let h = cpu.read32(o + SCALES + 4).wrapping_mul(2);
            cpu.write32(o + SCALES + 4, h);
        } else {
            let at = cpu.read16(o + BREATH_AT) as i16 as i32;
            let w = cpu.read32(b + BREATH_WIDTH).wrapping_sub((at * 32) as u32);
            cpu.write32(o + SCALES, w);
            let h = if cpu.read16(o + BREATH_AT) == 0x180 {
                cpu.write8(o + SPRITE_FRAME, 1);
                cpu.read32(b + BREATH_REACH).wrapping_mul(6)
            } else {
                cpu.read32(b + BREATH_REACH).wrapping_mul(3)
            };
            cpu.write32(o + SCALES + 4, h);
        }
        let m = along(cpu);
        let a = back(cpu);
        let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
        let dx = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, m));
        let x = cpu.read32(b + POSITION).wrapping_add(dx);
        cpu.write32(o + POSITION, x);
        let a = back(cpu);
        let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
        let dy = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, c.wrapping_neg(), m));
        let y = cpu.read32(b + POSITION + 4).wrapping_add(dy);
        cpu.write32(o + POSITION + 4, y);
    } else {
        let flags = cpu.read32(o + FLAGS) & !0x30;
        cpu.write32(o + FLAGS, flags);
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
        let flags = cpu.read32(o + FLAGS) | (r & 0x30);
        cpu.write32(o + FLAGS, flags);
        let m = along(cpu);
        let a = back(cpu);
        let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
        let dx = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, m));
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
        let x = cpu.read32(b + POSITION).wrapping_add(dx).wrapping_add(0x8000).wrapping_sub((r & 1) << 16);
        cpu.write32(o + POSITION, x);
        let a = back(cpu);
        let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
        let dy = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, c.wrapping_neg(), m));
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
        let y = cpu.read32(b + POSITION + 4).wrapping_add(dy).wrapping_add(0x8000).wrapping_sub((r & 1) << 16);
        cpu.write32(o + POSITION + 4, y);
    }
    let colour = cpu.read16(b + COLOUR);
    cpu.write16(o + COLOUR, colour);
    let colour = cpu.read16(b + OWN_COLOUR);
    cpu.write16(o + OWN_COLOUR, colour);
}

/// A glow made (list 2, 0xbc bytes) on the breath `b` at `at`: kind 42 frame 0, lifted 7, never hit
/// (as it is drawn), its colour 0xe0 past the first character's.
const BREATH_GLOW_BEHAVIOUR: u32 = 0x0608_d3d4;
const BREATH_GLOW_FLAGS: u32 = 0x2008_0400;
const BREATH_COLOUR: u16 = 0xe0;

pub fn breath_glow_set(cpu: &mut Cpu, b: u32, at: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0xbc)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + BEAM_LIST));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + BEHAVIOUR, BREATH_GLOW_BEHAVIOUR);
    let n = cpu.read8(b + NUMBER);
    cpu.write8(o + NUMBER, n);
    cpu.write32(o + FLAGS, BREATH_GLOW_FLAGS);
    cpu.write32(o + POSITION + 8, 0);
    cpu.write8(o + BYTE_33, 7);
    cpu.write8(o + SPRITE_FRAME, 0);
    cpu.write8(o + SPRITE_KIND, FLAME_KIND);
    let colour = cpu.read16(CHAR_BASE).wrapping_add(BREATH_COLOUR);
    cpu.write16(o + OWN_COLOUR, colour);
    cpu.write16(o + COLOUR, colour);
    cpu.write32(o + BREATH_OF, b);
    cpu.write16(o + BREATH_AT, at as u16);
}

/// The breath: ended with its arm (or the arm's life) or once at full reach. Its boxes by the game's
/// frame (four in turn); at its arm's mouth, turned with it; growing (its growth easing to 0x240
/// by a 16th of a step by difficulty), as wide as what its reach leaves of 1 and three times its
/// reach high (past 0x24000 high it is spent), its colour flickering; once under 1/8 wide, frame 90.
const BREATH_BOXES: u32 = 0x060a_0ccc;
const BREATH_GROWTH_STEPS: u32 = 0x060a_08d0;

pub fn breath_behaviour(cpu: &mut Cpu, o: u32) {
    let arm = cpu.read32(o + BREATH_OF);
    if cpu.read8(arm + OBJ_MODE) as i8 > 111
        || cpu.read32(arm + ARM_LIFE) as i32 <= 0
        || cpu.read32(o + BREATH_REACH) as i32 > 0x1_0000
    {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    let turn = cpu.read32(GAME_FRAMES) & 3;
    let boxes = cpu.read32(BREATH_BOXES + 4 * turn);
    cpu.write32(o + BEAM_LIST, boxes);
    for k in [0, 4] {
        let v = cpu.read32(arm + ARM_MOUTH + k);
        cpu.write32(o + POSITION + k, v);
    }
    let turn = cpu.read16(arm + TURN_Z);
    cpu.write16(o + TURN_Z, turn);
    let difficulty = cpu.read8(DIFFICULTY) as i8 as i32 as u32;
    let step = cpu.read16(BREATH_GROWTH_STEPS.wrapping_add(2 * difficulty)) as i16 as i32 / 16;
    let growth = cpu.read16(o + BREATH_GROWTH) as i16 as i32 as u32;
    let growth = nested!(cpu, 0x0604_6168, actors::approach(cpu, growth, 0x240, step as i16 as i32 as u32));
    cpu.write16(o + BREATH_GROWTH, growth as u16);
    let reach = cpu.read32(o + BREATH_REACH).wrapping_add(cpu.read16(o + BREATH_GROWTH) as i16 as i32 as u32);
    cpu.write32(o + BREATH_REACH, reach);
    let width = 0x1_0000u32.wrapping_sub(reach);
    cpu.write32(o + BREATH_WIDTH, width);
    cpu.write32(o + SCALES, width);
    let height = cpu.read32(o + BREATH_REACH).wrapping_mul(3);
    cpu.write32(o + SCALES + 4, height);
    if height as i32 > 0x2_4000 {
        let flags = cpu.read32(o + FLAGS) | BREATH_SPENT;
        cpu.write32(o + FLAGS, flags);
    }
    let colour = cpu.read16(CHAR_BASE).wrapping_add(if cpu.read32(GAME_FRAMES) & 1 != 0 { 0x20 } else { 0x30 });
    cpu.write16(o + OWN_COLOUR, colour);
    if cpu.read32(o + SCALES) as i32 <= 0x1fff {
        cpu.write8(o + SPRITE_KIND, 0);
        cpu.write8(o + SPRITE_FRAME, BREATH_SPENT_FRAME);
        cpu.write8(o + SPRITE_KIND, 0);
    }
}

/// A breath made (list 2, 0xbc bytes) from the arm `arm` (sound 43): kind 42 frame 1, its boxes,
/// side 1, type 5, half size, lifted 8, its colour 0xe0 past the first character's; its six glows.
const BREATH_BEHAVIOUR: u32 = 0x0608_d688;
const BREATH_FLAGS: u32 = 0x2000_0420;
const BREATH_FIRST_BOXES: u32 = 0x060a_0baa;
const BREATH_SOUND: u32 = 43;

pub fn breath_set(cpu: &mut Cpu, arm: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0xbc)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, BREATH_SOUND));
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + BEAM_LIST));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + BEHAVIOUR, BREATH_BEHAVIOUR);
    let n = cpu.read8(arm + NUMBER);
    cpu.write8(o + NUMBER, n);
    cpu.write32(o + FLAGS, BREATH_FLAGS);
    cpu.write32(o + BEAM_LIST, BREATH_FIRST_BOXES);
    cpu.write8(o + SIDE, 1);
    cpu.write8(o + TYPE, 5);
    cpu.write8(o + SPRITE_FRAME, 1);
    cpu.write8(o + SPRITE_KIND, FLAME_KIND);
    let colour = cpu.read16(CHAR_BASE).wrapping_add(BREATH_COLOUR);
    cpu.write16(o + OWN_COLOUR, colour);
    cpu.write16(o + COLOUR, colour);
    for k in [0, 4] {
        cpu.write32(o + SCALES + k, 0x8000);
    }
    cpu.write32(o + POSITION + 8, 0);
    cpu.write8(o + BYTE_33, 8);
    cpu.write32(o + BREATH_OF, arm);
    for i in 1..=6 {
        nested!(cpu, 0x0608_d5c4, breath_glow_set(cpu, o, 0x40 * i));
    }
}

// ---- the winged boss's laser ----------------------------------------------------------------------

/// The laser (an actor_exec model from the winged boss, +0x6c; its count +0x8a, a word; its size
/// +0xac, 16.16) and the boxes it sends out along itself (+0x70 their laser; how far out +0xac).
const WB_LASER_OF: u32 = 0x6c;
const WB_LASER_COUNT: u32 = 0x8a;
const WB_LASER_SIZE: u32 = 0xac;
const WB_LASER_BOX_LASER: u32 = 0x70;
const WB_LASER_BOX_OUT: u32 = 0xac;
/// The winged boss's core's life (+0x1a4).
const WB_CORE_LIFE: u32 = 0x1a4;

/// A box sent along the laser: ended with the boss, its core's life or past 192 dots out; half the
/// laser's size, out from the boss's mouth along the boss's way back, going 16 dots a frame.
pub fn wb_laser_box_behaviour(cpu: &mut Cpu, o: u32) {
    let boss = cpu.read32(o + WB_LASER_OF);
    if cpu.read8(boss + OBJ_MODE) as i8 > 111
        || cpu.read32(boss + WB_CORE_LIFE) as i32 <= 0
        || cpu.read32(o + WB_LASER_BOX_OUT) as i32 > 0xc0_0000
    {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    let laser = cpu.read32(o + WB_LASER_BOX_LASER);
    for k in [0, 4] {
        let v = cpu.read32(laser + SCALES + k) as i32 / 2;
        cpu.write32(o + SCALES + k, v as u32);
    }
    for k in [0, 4] {
        let out = cpu.read32(o + WB_LASER_BOX_OUT);
        let a = cpu.read16(boss + TURN_Z).wrapping_add(0x8000) as i16 as i32 as u32;
        let d = if k == 0 {
            let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
            nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, out))
        } else {
            let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
            nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, c.wrapping_neg(), out))
        };
        let v = cpu.read32(boss + WB_MOUTH + k).wrapping_add(d);
        cpu.write32(o + POSITION + k, v);
    }
    let out = cpu.read32(o + WB_LASER_BOX_OUT).wrapping_add(0x10_0000);
    cpu.write32(o + WB_LASER_BOX_OUT, out);
}

/// A box made (list 2, 0xbc bytes, unseen) by the laser `laser` at its place: lifted 7, its boxes,
/// side 1, type 5.
const WB_LASER_BOX_BEHAVIOUR: u32 = 0x0608_d8d4;
const WB_LASER_BOX_FLAGS: u32 = 0x8000_0400;
const WB_LASER_BOX_BOXES: u32 = 0x060a_0cdc;

pub fn wb_laser_box_set(cpu: &mut Cpu, laser: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0xbc)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + BEAM_LIST));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + BEHAVIOUR, WB_LASER_BOX_BEHAVIOUR);
    let n = cpu.read8(laser + NUMBER);
    cpu.write8(o + NUMBER, n);
    cpu.write32(o + FLAGS, WB_LASER_BOX_FLAGS);
    for k in [0, 4] {
        let v = cpu.read32(laser + POSITION + k);
        cpu.write32(o + POSITION + k, v);
    }
    cpu.write32(o + POSITION + 8, 0);
    cpu.write8(o + BYTE_33, 7);
    cpu.write32(o + BEAM_LIST, WB_LASER_BOX_BOXES);
    cpu.write8(o + SIDE, 1);
    cpu.write8(o + TYPE, 5);
    let boss = cpu.read32(laser + WB_LASER_OF);
    cpu.write32(o + WB_LASER_OF, boss);
    cpu.write32(o + WB_LASER_BOX_LASER, laser);
}

/// The control the laser's sound plays on (the boss's +0x2d8, a byte; 0 none yet) and the sound.
const WB_LASER_CONTROL: u32 = 0x2d8;
const WB_LASER_SOUND: u32 = 39;

/// The laser: ended (its sound stopped) with the boss, its core's life or once shrunk away. Its
/// count: at 60 a flash of the screen (orange, faded back over 256 frames); until then it swells to
/// a 256th (by 1 a frame), from then to full size (by 1/64; its sound started once full) and from
/// 270 shrinks (by 1/32). Its width flickering a 16th smaller every other frame, its depth 1; ahead
/// of the boss's mouth (a dot for each 16th of its size), turned with it and tilted half as far the
/// other way; past 3/8 size sending boxes along itself.
pub fn wb_laser_behaviour(cpu: &mut Cpu, o: u32) {
    let boss = cpu.read32(o + WB_LASER_OF);
    if cpu.read8(boss + OBJ_MODE) as i8 > 111
        || cpu.read32(boss + WB_CORE_LIFE) as i32 <= 0
        || cpu.read32(o + WB_LASER_SIZE) == 0
    {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        let control = cpu.read8(boss + WB_LASER_CONTROL) as i8 as i32 as u32;
        nested!(cpu, 0x0600_52e0, sound::se_stop(cpu, WB_LASER_SOUND, control));
        return;
    }
    let n = cpu.read16(o + WB_LASER_COUNT).wrapping_add(1);
    cpu.write16(o + WB_LASER_COUNT, n);
    if n == 60 {
        nested!(cpu, 0x0605_e6dc, screen::set_fade_offset(cpu, 100, 80, 40));
        nested!(cpu, 0x0605_e93c, screen::fade_to(cpu, 0, 0, 0, 0x100));
    }
    let n = cpu.read16(o + WB_LASER_COUNT) as i16;
    let size = cpu.read32(o + WB_LASER_SIZE);
    let (to, by) = if n > 269 {
        (0, 0x800)
    } else if n > 59 {
        if cpu.read8(boss + WB_LASER_CONTROL) == 0 && size == 0x1_0000 {
            let control = nested!(cpu, 0x0600_50d4, sound::se_set(cpu, WB_LASER_SOUND));
            cpu.write8(boss + WB_LASER_CONTROL, control as u8);
        }
        (0x1_0000, 0x400)
    } else {
        (0x100, 1)
    };
    let size = cpu.read32(o + WB_LASER_SIZE);
    let size = nested!(cpu, 0x0604_6168, actors::approach(cpu, size, to, by));
    cpu.write32(o + WB_LASER_SIZE, size);
    let flicker = (cpu.read32(GAME_FRAMES) & 1).wrapping_mul(0x1000);
    let size = cpu.read32(o + WB_LASER_SIZE);
    let less = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, flicker, size));
    let width = cpu.read32(o + WB_LASER_SIZE).wrapping_sub(less);
    cpu.write32(o + SCALES, width);
    cpu.write32(o + SCALES + 8, 0x1_0000);
    cpu.write32(o + SCALES + 4, width);
    let turn = cpu.read16(boss + TURN_Z);
    cpu.write16(o + TURN_Z, turn);
    for k in [0, 4] {
        let back = (cpu.read32(o + SCALES) as i32 / 0x1000).wrapping_mul(cpu.read32(o + SCALES + 8) as i32) as u32;
        let a = cpu.read16(o + TURN_Z).wrapping_add(0x8000) as i16 as i32 as u32;
        let d = if k == 0 {
            let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
            nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, back))
        } else {
            let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
            nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, c.wrapping_neg(), back))
        };
        let v = cpu.read32(boss + WB_MOUTH + k).wrapping_sub(d);
        cpu.write32(o + POSITION + k, v);
    }
    let tilt = (cpu.read16(boss + TURN_Y) as i16 as i32).wrapping_neg() / 2;
    cpu.write16(o + TURN_Y, tilt as u16);
    if cpu.read32(o + SCALES) as i32 > 0x6000 {
        nested!(cpu, 0x0608_d9c4, wb_laser_box_set(cpu, o));
    }
}

/// The winged boss's model set (0x06074470: its +0x1c its models; the laser's the eighth's plain
/// one) and the laser's glows' colours (past the first character's, by the main loop's pass).
const WB_MODEL_SET: u32 = 0x0607_4470;
const WB_LASER_MODEL: u32 = 0x1c;
const WB_LASER_COLOURS: u32 = 0x060a_0ce4;

/// The laser's draw (its sprite `s`), while it has any width: its model at its place, turned (z, y,
/// x) and scaled; then sprites: a flare where it ends (96 dots out, 176 nearer; twice its size
/// wide and jittering, 8 high; flipped by turns, 0x100 once under a 16th wide), a glow at the
/// mouth (a quarter of 1.5 less its size, jittering), and two sparks up to 144 dots out along it
/// (frames 61 to 76), all kind 42 by the main loop's colours.
pub fn wb_laser_draw(cpu: &mut Cpu, s: u32) {
    let o = s.wrapping_sub(FLAGS);
    if cpu.read32(o + SCALES) as i32 <= 0 {
        return;
    }
    let d = cpu.stack();
    let p = d + 0x2c;
    cpu.call(SL_PUSH_MATRIX, &[]);
    let at = [0, 4, 8].map(|k| cpu.read32(o + POSITION + k));
    cpu.call(SL_TRANSLATE, &at);
    for (turn, rot) in [(TURN_Z, SL_ROT_Z), (TURN_Y, SL_ROT_Y), (TURN_X, SL_ROT_X)] {
        let a = cpu.read16(o + turn) as i16 as i32 as u32;
        cpu.call(rot, &[a]);
    }
    cpu.call(SL_CALC_POINT, &[0, 0, 0xffa0_0000, p]);
    let scales = [0, 4, 8].map(|k| cpu.read32(o + SCALES + k));
    cpu.call(SL_SCALE, &scales);
    let set = cpu.read32(WB_MODEL_SET);
    let models = cpu.read32(set + SHIP_MODELS);
    let pair = cpu.read32(models + WB_LASER_MODEL);
    let model = cpu.read32(pair);
    cpu.call(SL_PUT_POLYGON, &[model]);
    cpu.call(SL_POP_MATRIX, &[]);
    let colour = |cpu: &mut Cpu| {
        let which = cpu.read32(LOOPS) & 1;
        let base = cpu.read16(CHAR_BASE);
        base.wrapping_add(cpu.read16(WB_LASER_COLOURS + 2 * which))
    };

    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, d));
    let mut flags = if cpu.read32(LOOPS) & 2 != 0 { 0x30 } else { 0x20 };
    if cpu.read32(o + SCALES) as i32 <= 0xfff {
        flags |= 0x100;
    }
    cpu.write32(d, flags);
    for k in [0, 4] {
        let v = cpu.read32(p + k);
        cpu.write32(d + 4 + k, v);
    }
    let z = cpu.read32(p + 8).wrapping_add(0xff50_0000);
    cpu.write32(d + 4 + 8, z);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    let w = cpu.read32(o + WB_LASER_SIZE).wrapping_mul(2).wrapping_add((r & 31) << 9);
    cpu.write32(d + 0x10, w);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write32(d + 0x14, ((r & 31) << 8).wrapping_add(0x8_0000));
    let turn = cpu.read16(o + TURN_Z);
    cpu.write16(d + SPRITE_STRUCT_ANGLE, turn);
    cpu.write8(d + SPRITE_STRUCT_LIFT, 0xf8);
    cpu.write8(d + SPRITE_STRUCT_FRAME, 1);
    cpu.write8(d + SPRITE_STRUCT_KIND, FLAME_KIND);
    let c = colour(cpu);
    cpu.write16(d + SPRITE_STRUCT_COLOUR, c);
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));

    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, d));
    cpu.write32(d, 0);
    for k in [0, 4, 8] {
        let v = cpu.read32(o + POSITION + k);
        cpu.write32(d + 4 + k, v);
    }
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    let size = ((((r & 31) << 10).wrapping_add(0x1_8000).wrapping_sub(cpu.read32(o + WB_LASER_SIZE))) as i32 / 4) as u32;
    cpu.write32(d + 0x10, size);
    cpu.write32(d + 0x14, size);
    let turn = cpu.read16(o + TURN_Z);
    cpu.write16(d + SPRITE_STRUCT_ANGLE, turn);
    cpu.write8(d + SPRITE_STRUCT_LIFT, 0xf7);
    cpu.write8(d + SPRITE_STRUCT_FRAME, 1);
    cpu.write8(d + SPRITE_STRUCT_KIND, 0);
    let c = colour(cpu);
    cpu.write16(d + SPRITE_STRUCT_COLOUR, c);
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));

    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, d));
    for (n, more) in [(0x80, 16), (0x90, 0)] {
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, n)) as i32;
        let out = (r.wrapping_abs().wrapping_add(more) as u32) & 0xffff;
        let a = cpu.read16(o + TURN_Z).wrapping_add(0x8000) as i16 as i32 as u32;
        let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
        let x = cpu.read32(o + POSITION).wrapping_add(s.wrapping_mul(out));
        cpu.write32(d + 4, x);
        let a = cpu.read16(o + TURN_Z).wrapping_add(0x8000) as i16 as i32 as u32;
        let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
        if more != 0 {
            cpu.write32(d + 4 + 8, 0);
        }
        let y = cpu.read32(o + POSITION + 4).wrapping_sub(c.wrapping_mul(out));
        cpu.write32(d + 4 + 4, y);
        if more != 0 {
            let turn = cpu.read16(o + TURN_Z);
            cpu.write16(d + SPRITE_STRUCT_ANGLE, turn);
            cpu.write8(d + SPRITE_STRUCT_LIFT, 16);
            let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
            cpu.write8(d + SPRITE_STRUCT_FRAME, (r as u8 & 15) + 61);
            cpu.write8(d + SPRITE_STRUCT_KIND, 0);
            let c = colour(cpu);
            cpu.write16(d + SPRITE_STRUCT_COLOUR, c);
            let w = cpu.read32(o + SCALES).wrapping_add(0x3000);
            cpu.write32(d + 0x10, w);
            cpu.write32(d + 0x14, 0xc000);
        }
        nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
    }
}

/// The laser made (list 2, 0xbc bytes) by the boss `boss`: drawn by its own function, lifted 8, a
/// 16th of its size, upright.
const WB_LASER_BEHAVIOUR: u32 = 0x0608_da7c;
const WB_LASER_DRAW: u32 = 0x0608_dc7c;
const WB_LASER_FLAGS: u32 = 0xc8_0000;

pub fn wb_laser_set(cpu: &mut Cpu, boss: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0xbc)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + BEAM_LIST));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + BEHAVIOUR, WB_LASER_BEHAVIOUR);
    let n = cpu.read8(boss + NUMBER);
    cpu.write8(o + NUMBER, n);
    cpu.write32(o + FLAGS, WB_LASER_FLAGS);
    cpu.write32(o + MODEL, WB_LASER_DRAW);
    cpu.write32(o + POSITION + 8, 0);
    cpu.write8(o + BYTE_33, 8);
    cpu.write32(o + WB_LASER_SIZE, 0x1000);
    cpu.write32(o + WB_LASER_OF, boss);
    cpu.write16(o + TURN_X, 0x4000);
}

// ---- the winged boss's flight ---------------------------------------------------------------------

/// Turns the word at `rate` from `from` toward `to` as aim.rs's turn_toward does (the turn left at
/// most 0x800, the goal at `goal` a tenth of it eased by 4 a frame), with no frame of its own.
pub fn wb_turn_toward(cpu: &mut Cpu, rate: u32, goal: u32, from: u32, to: u32) {
    let (from, to) = (from & 0xffff, to & 0xffff);
    let d = nested!(cpu, 0x0605_6580, aim::angle_between(cpu, from, to)) as i16 as i32;
    let left = (d.unsigned_abs() & 0xffff).min(0x800);
    let side = nested!(cpu, 0x0605_6554, aim::turn_sign(cpu, to, from)) as i32;
    let g = nested!(cpu, 0x0600_6a78, math::quotient(cpu, left, if side > 0 { 10 } else { 0xffff_fff6 }));
    let v = cpu.read16(goal) as i16 as i32 as u32;
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, g, 4));
    cpu.write16(goal, v as u16);
    let r = cpu.read16(rate) as i16 as i32 as u32;
    let g = cpu.read16(goal) as i16 as i32 as u32;
    let r = nested!(cpu, 0x0604_6168, actors::approach(cpu, r, g, left));
    cpu.write16(rate, r as u16);
}

/// The winged boss's step (+0x74, a byte), its turns' rates and their goals (x: +0xcc, +0xce; y:
/// +0xd0, +0xd2; z: +0xd4, +0xd6), its flags (+0x76, a word: 1 not yet fighting), its wait (+0x7a),
/// its wings' opening (+0x9a, a word: 0x4000 open), its script's place (+0x7c).
const WB_STEP: u32 = 0x74;
const WB_TURN_X: u32 = 0xcc;
const WB_TURN_Y: u32 = 0xd0;
const WB_TURN_Z: u32 = 0xd4;
const WB_FLAGS: u32 = 0x76;
const WB_WAIT: u32 = 0x7a;
const WB_OPENING: u32 = 0x9a;
const WB_SCRIPT_AT: u32 = 0x7c;
const WB_ARRIVES_SOUND: u32 = 79;
const WB_SETTLES_SOUND: u32 = 80;
static TURN_TOWARD_PROLOGUE: std::sync::OnceLock<super::Prologue> = std::sync::OnceLock::new();

/// Its z turn's rate set going to face away from the nearer player.
pub fn wb_turn_from_player(cpu: &mut Cpu, o: u32) {
    let player = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + FLAGS));
    let a = nested!(cpu, 0x0604_5e30, aim::angle_to(cpu, o, player));
    let (from, to) = (cpu.read16(o + TURN_Z) as u32, a.wrapping_add(0x8000) & 0xffff);
    nested!(cpu, 0x0608_e100, wb_turn_toward(cpu, o + WB_TURN_Z, o + WB_TURN_Z + 2, from, to));
}

/// Step 0: it comes down (2 dots a frame) to 0; then 1 (sound 79).
pub fn wb_come_down(cpu: &mut Cpu, o: u32) {
    let y = cpu.read32(o + POSITION + 4);
    let y = nested!(cpu, 0x0604_6168, actors::approach(cpu, y, 0, 0x2_0000));
    cpu.write32(o + POSITION + 4, y);
    if y == 0 {
        cpu.write8(o + WB_STEP, 1);
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, WB_ARRIVES_SOUND));
    }
}

/// Step 1: on up to 80 dots up, levelling (its y turn to 0, its x turn to upright: at most 0x800 a
/// frame, a 16th of the way, the rate easing by 2), its depth by its y turn; once upright and its x
/// turn's goal is none, 2 (sound 80).
pub fn wb_settle(cpu: &mut Cpu, o: u32) {
    let y = cpu.read32(o + POSITION + 4);
    let y = nested!(cpu, 0x0604_6168, actors::approach(cpu, y, 0xffb0_0000, 0x2_0000));
    cpu.write32(o + POSITION + 4, y);
    for (rate, turn, to) in [(WB_TURN_Y, TURN_Y, 0), (WB_TURN_X, TURN_X, 0x4000)] {
        let from = cpu.read16(o + turn) as u32;
        let (rate, goal) = (o + rate, o + rate + 2);
        cpu.nested_with_stack(&TURN_TOWARD_PROLOGUE, 0x0605_70d8, &[0x800, 16, 2], |cpu| {
            aim::turn_toward(cpu, rate, goal, from, to, 0x800, 16, 2)
        });
    }
    let half = (cpu.read16(o + TURN_Y) as i16 as i32 / 2) as u32;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, half));
    cpu.write32(o + POSITION + 8, (s << 8).wrapping_neg());
    if cpu.read16(o + WB_TURN_X + 2) == 0 && cpu.read16(o + TURN_X) == 0x4000 {
        cpu.write8(o + WB_STEP, 2);
        cpu.write16(o + TURN_Y, 0);
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, WB_SETTLES_SOUND));
    }
}

/// Step 2: a wait of 11 frames; then 3.
pub fn wb_wait(cpu: &mut Cpu, o: u32) {
    let n = cpu.read16(o + WB_WAIT).wrapping_add(1);
    cpu.write16(o + WB_WAIT, n);
    if n as i16 > 10 {
        cpu.write8(o + WB_STEP, 3);
        cpu.write16(o + WB_WAIT, 0);
    }
}

/// Step 3: its wings open (0x80 a frame); once open, 4: fighting, its count and script from the
/// start.
pub fn wb_open(cpu: &mut Cpu, o: u32) {
    let v = cpu.read16(o + WB_OPENING).wrapping_add(0x80);
    cpu.write16(o + WB_OPENING, v);
    if v == 0x4000 {
        cpu.write8(o + WB_STEP, 4);
        let flags = cpu.read16(o + WB_FLAGS) & !1;
        cpu.write16(o + WB_FLAGS, flags);
        cpu.write16(o + WB_COUNT, 0);
        cpu.write16(o + WB_SCRIPT_AT, 0);
    }
}

/// Its script (words in pairs, from 0x060a0ce8: the count to act at, the act), how many times it
/// has run through (+0xde), its slots (words from +0x7e: an act under 0x80 is kept in the slot of
/// its bits above the low four; the first slot its attack), its missiles' count (+0xdc).
const WB_SCRIPT: u32 = 0x060a_0ce8;
const WB_LAPS: u32 = 0xde;
const WB_SLOTS: u32 = 0x7e;
const WB_MISSILES: u32 = 0xdc;

/// Its script's acts due at its count: 0xffff starts it over (and the count), 0x80 a volley (unless
/// its y turn is moving), 0x81 sixty frames of missiles; others without 0x80 kept in their slots.
pub fn wb_script(cpu: &mut Cpu, o: u32) {
    loop {
        let at = cpu.read16(o + WB_SCRIPT_AT) as u32;
        if cpu.read16(o + WB_COUNT) != cpu.read16(WB_SCRIPT + 2 * at) {
            return;
        }
        let act = cpu.read16(WB_SCRIPT + 2 * at + 2);
        if act == 0xffff {
            cpu.write16(o + WB_COUNT, 0);
            cpu.write16(o + WB_SCRIPT_AT, 0);
            let laps = cpu.read16(o + WB_LAPS).wrapping_add(1);
            cpu.write16(o + WB_LAPS, laps);
            continue;
        }
        cpu.write16(o + WB_SCRIPT_AT, (at + 2) as u16);
        if act & 0x80 == 0 {
            cpu.write16(o + WB_SLOTS + 2 * (act as u32 >> 4), act);
        } else if act == 0x80 {
            if cpu.read16(o + WB_TURN_Y) == 0 {
                nested!(cpu, 0x0608_ce2c, wb_volley(cpu, o));
            }
        } else if act == 0x81 {
            cpu.write16(o + WB_MISSILES, 60);
        }
    }
}

/// Its fans' frames by difficulty (bytes), its laser's time (+0xd8), how fast it falls once downed
/// (+0x94), the laser's sound.
const WB_FAN_EVERY: u32 = 0x060a_0894;
const WB_LASER_TIME: u32 = 0xd8;
const WB_FALL: u32 = 0x94;
const WB_LASER_START_SOUND: u32 = 70;
static WB_TURN_TOWARD_PROLOGUE: std::sync::OnceLock<super::Prologue> = std::sync::OnceLock::new();

/// Its own copy of aim.rs's turn_toward (at 0x0608e044).
#[allow(clippy::too_many_arguments)]
fn wb_turn(cpu: &mut Cpu, rate: u32, from: u32, to: u32, most: u32, ease: u32, accel: u32) {
    let goal = rate + 2;
    cpu.nested_with_stack(&WB_TURN_TOWARD_PROLOGUE, 0x0608_e044, &[most, ease, accel], |cpu| {
        aim::turn_toward(cpu, rate, goal, from, to, most, ease, accel)
    });
}

/// Step 4: fighting, held 72 dots up and 5 more for each difficulty below the hardest, its back to
/// the player, levelling (y to 0: at most 0x1000 a frame, a tenth, by 24; x upright: 0x800, a
/// quarter, by 2). While its y turn is still, its missiles every 20th of their frames; then its
/// attack: 1 fans of flames (every so many frames by difficulty), 2 volleys, 3 both, 8 its laser
/// (5: sound 70, 300 frames).
pub fn wb_fight(cpu: &mut Cpu, o: u32) {
    let below = (5 - cpu.read8(DIFFICULTY) as i8 as i32) * 5;
    let to = 0xffb8_0000u32.wrapping_sub((below << 16) as u32);
    let y = cpu.read32(o + POSITION + 4);
    let y = nested!(cpu, 0x0604_6168, actors::approach(cpu, y, to, 0x2000));
    cpu.write32(o + POSITION + 4, y);
    nested!(cpu, 0x0608_e190, wb_turn_from_player(cpu, o));
    let from = cpu.read16(o + TURN_Y) as u32;
    wb_turn(cpu, o + WB_TURN_Y, from, 0, 0x1000, 10, 24);
    let from = cpu.read16(o + TURN_X) as u32;
    wb_turn(cpu, o + WB_TURN_X, from, 0x4000, 0x800, 4, 2);
    if cpu.read16(o + WB_TURN_Y) == 0 && cpu.read16(o + WB_MISSILES) as i16 > 0 {
        let n = cpu.read16(o + WB_MISSILES) as i16 as i32 as u32;
        if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, n, 20)) == 0 {
            nested!(cpu, 0x0608_cf78, wb_missiles(cpu, o));
        }
        let n = cpu.read16(o + WB_MISSILES).wrapping_sub(1);
        cpu.write16(o + WB_MISSILES, n);
    }
    let fan_due = |cpu: &mut Cpu| {
        let difficulty = cpu.read8(DIFFICULTY) as i8 as i32 as u32;
        let every = cpu.read8(WB_FAN_EVERY.wrapping_add(difficulty)) as u32;
        let count = cpu.read16(o + WB_COUNT) as u32;
        nested!(cpu, 0x0600_6a6e, math::remainder(cpu, count, every)) == 0
    };
    match cpu.read16(o + WB_SLOTS) {
        1 => {
            if fan_due(cpu) {
                nested!(cpu, 0x0608_d2a0, wb_flame_fan(cpu, o));
            }
        }
        2 => nested!(cpu, 0x0608_ce2c, wb_volley(cpu, o)),
        3 => {
            if fan_due(cpu) {
                nested!(cpu, 0x0608_d2a0, wb_flame_fan(cpu, o));
            }
            nested!(cpu, 0x0608_ce2c, wb_volley(cpu, o));
        }
        8 => {
            cpu.write16(o + WB_SLOTS, 0);
            cpu.write8(o + WB_STEP, 5);
            cpu.write16(o + WB_TURN_Z + 2, 0);
            cpu.write16(o + WB_LASER_TIME, 300);
            nested!(cpu, 0x0608_df9c, wb_laser_set(cpu, o));
            nested!(cpu, 0x0600_50d4, sound::se_set(cpu, WB_LASER_START_SOUND));
        }
        _ => {}
    }
}

/// Step 5: its laser, 80 dots up, rolling (its y turn toward a further 0x6000: at most 0x4000, an
/// eighth, by 32) and slowly turning its back to the player (at most 0x100, a twentieth, by 4); its
/// laser's time out, 4 again, its x turn's rate 0xc00 and goal none.
pub fn wb_laser_phase(cpu: &mut Cpu, o: u32) {
    let y = cpu.read32(o + POSITION + 4);
    let y = nested!(cpu, 0x0604_6168, actors::approach(cpu, y, 0xffb0_0000, 0x2000));
    cpu.write32(o + POSITION + 4, y);
    let turn = cpu.read16(o + TURN_Y);
    wb_turn(cpu, o + WB_TURN_Y, turn as u32, turn.wrapping_add(0x6000) as u32, 0x4000, 8, 32);
    let player = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + FLAGS));
    let a = nested!(cpu, 0x0604_5e30, aim::angle_to(cpu, o, player));
    let from = cpu.read16(o + TURN_Z) as u32;
    wb_turn(cpu, o + WB_TURN_Z, from, a.wrapping_add(0x8000) & 0xffff, 0x100, 20, 4);
    let t = cpu.read16(o + WB_LASER_TIME).wrapping_sub(1);
    cpu.write16(o + WB_LASER_TIME, t);
    if (t as i16) < 0 {
        cpu.write8(o + WB_STEP, 4);
        cpu.write16(o + WB_TURN_X, 0xc00);
        cpu.write16(o + WB_TURN_X + 2, 0);
    }
}

/// Step 6: downed, levelling its y turn (0x4000, an eighth, by 32), its x turn's rate easing to
/// -0x600 and its z turn's to none (by 8); an explosion at its 60th frame; falling back ever faster
/// (to 5 dots a frame, by 3/256); ended past its 210th.
pub fn wb_downed(cpu: &mut Cpu, o: u32) {
    let from = cpu.read16(o + TURN_Y) as u32;
    wb_turn(cpu, o + WB_TURN_Y, from, 0, 0x4000, 8, 32);
    for (rate, to) in [(WB_TURN_X, 0xffff_fa00), (WB_TURN_Z, 0)] {
        let v = cpu.read16(o + rate) as i16 as i32 as u32;
        let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, to, 8));
        cpu.write16(o + rate, v as u16);
    }
    let n = cpu.read16(o + WB_WAIT).wrapping_add(1);
    cpu.write16(o + WB_WAIT, n);
    if n == 60 {
        let p = cpu.stack();
        for k in [0, 4, 8] {
            let v = cpu.read32(o + POSITION + k);
            cpu.write32(p + k, v);
        }
        cpu.write32(p + 0x3c, o);
        nested!(cpu, 0x0605_8ba4, explosions::explosion_set(cpu, p));
    }
    let z = cpu.read32(o + POSITION + 8).wrapping_add(cpu.read32(o + WB_FALL));
    cpu.write32(o + POSITION + 8, z);
    let fall = cpu.read32(o + WB_FALL);
    let fall = nested!(cpu, 0x0604_6168, actors::approach(cpu, fall, 0x5_0000, 0x300));
    cpu.write32(o + WB_FALL, fall);
    if cpu.read16(o + WB_WAIT) as i16 > 210 {
        cpu.write8(o + OBJ_MODE, TO_STOP);
    }
}

// ---- the winged boss's arms -----------------------------------------------------------------------

/// An arm (its own object; +0x6c the boss): which arm (+0x70, a byte: 0 and 3 the outer pair, which
/// spit fire, 1 and 2 the inner, which breathe), its step (+0x71, a byte), its turns' rates and their
/// goals (x: +0x84, +0x86; y: +0x88, +0x8a; z: +0x8c, +0x8e), its fall once downed (+0x80), a count
/// (+0x164, a word), its hit flags (+0x15c). Each arm's place on the boss (12 bytes from +0x9c),
/// its down flag on the boss (a byte from +0x88), and the boss's slots its attacks are kept in.
const ARM_OF: u32 = 0x6c;
const ARM_WHICH: u32 = 0x70;
const ARM_STEP: u32 = 0x71;
const ARM_TURN_X: u32 = 0x84;
const ARM_TURN_Y: u32 = 0x88;
const ARM_TURN_Z: u32 = 0x8c;
const ARM_FALL: u32 = 0x80;
const ARM_COUNT: u32 = 0x164;
const ARM_HIT_FLAGS: u32 = 0x15c;
const WB_ARM_PLACES: u32 = 0x9c;
const WB_ARM_DOWN: u32 = 0x88;
const WB_ARM_BELOW: u32 = 0xb0_0000;
const WB_SLOT_1: u32 = 0x80;
const WB_SLOT_2: u32 = 0x82;
const WB_SLOT_3: u32 = 0x84;
const WB_SLOT_4: u32 = 0x86;
/// The acts that set each arm going (in slots 1-4: 0x11, 0x21, 0x31, 0x41).
const ARM_ACTS: [u16; 4] = [0x11, 0x21, 0x31, 0x41];
/// The inner arms' breath: the turn they rear back to (either way), the count it lasts.
const ARM_REAR: [u32; 2] = [0x600, 0xffff_fa00];
const ARM_BREATH_TIME: u16 = 160;
/// The outer arms' fireballs (shells: bullets.rs's shell_set, kind 42 with frames 4 on): their
/// sound, every so many frames by difficulty (bytes), their speed by difficulty, their flags.
const ARM_SHOT_SOUND: u32 = 28;
const ARM_SHOT_EVERY: u32 = 0x060a_088c;
const ARM_SHOT_SPEEDS: u32 = 0x060a_08bc;
const ARM_SHOT_FLAGS: u32 = 0x4_0000;
const ARM_SHOT_KIND: u8 = 42;
const ARM_SHOT_FRAME: u8 = 4;
const ARM_SHOT_COLOUR: u16 = 0x30;
const SHELL_LAUNCH: u32 = 0x2e;
const SHELL_SPEED: u32 = 0x30;
const SHELL_LAUNCH_SPEED: u32 = 0x34;
const SHELL_KIND: u32 = 0x38;
const SHELL_FUSE: u32 = 0x7c;
const SHELL_BEHAVIOUR: u32 = 0x0608_e798;
const SHELL_COLOURS: u32 = 0x28;
/// The sounds of an arm and of a wing knocked off.
const ARM_DOWN_SOUND: u32 = 17;

/// Step 0: held until the boss fights (its step 4); then 1.
pub fn arm_wait(cpu: &mut Cpu, o: u32) {
    let boss = cpu.read32(o + ARM_OF);
    if cpu.read8(boss + WB_STEP) == 4 {
        cpu.write8(o + ARM_STEP, 1);
    }
}

/// An outer arm's fireball: a shell whose fuse is never lit, cycling frames 4-35.
pub fn arm_shot_behaviour(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0605_547c, bullets::shell_exec(cpu, o));
    let frame = (cpu.read8(GAME_FRAMES_LOW) & 31) + ARM_SHOT_FRAME;
    cpu.write8(o + SPRITE_FRAME, frame);
    cpu.write8(o + SHELL_FUSE, 0);
}

/// Step 1: fighting, its back to the player. An inner arm rears back (its y turn's rate toward
/// 0x600 either way, by 16); once its act is due (0x21 or 0x31), it is cleared, the arm breathes
/// (step 2, its z turn's goal none, 160 frames). An outer arm turns its x toward 0x4800 less up to
/// 0xfff at random, its y toward 0x1000 (arm 3: 0x2000), and sways (its z toward 4 dots, arm 3 -3,
/// by 1/32); while its act (0x11 or 0x41) is kept, or the inner arm beside it is down, every so
/// many frames a fireball from its mouth, about its back at random within 0x1000 either way, with a
/// glow.
pub fn arm_fight(cpu: &mut Cpu, o: u32) {
    let boss = cpu.read32(o + ARM_OF);
    let which = cpu.read8(o + ARM_WHICH);
    if which > 3 {
        return;
    }
    let player = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + FLAGS));
    let a = nested!(cpu, 0x0604_5e30, aim::angle_to(cpu, o, player));
    let from = cpu.read16(o + TURN_Z) as u32;
    let to = a.wrapping_add(0x8000) & 0xffff;
    nested!(cpu, 0x0608_e100, wb_turn_toward(cpu, o + ARM_TURN_Z, o + ARM_TURN_Z + 2, from, to));
    if which == 1 || which == 2 {
        let acts = [cpu.read16(boss + WB_SLOT_2), cpu.read16(boss + WB_SLOT_3)];
        let to = ARM_REAR[which as usize - 1];
        let v = cpu.read16(o + ARM_TURN_Y) as i16 as i32 as u32;
        let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, to, 16));
        cpu.write16(o + ARM_TURN_Y, v as u16);
        let which = cpu.read8(o + ARM_WHICH);
        if (which == 1 && acts[0] == ARM_ACTS[1]) || (which == 2 && acts[1] == ARM_ACTS[2]) {
            cpu.write16(boss + WB_SLOT_1 + 2 * which as u32, 0);
            cpu.write8(o + ARM_STEP, 2);
            cpu.write16(o + ARM_TURN_Z + 2, 0);
            cpu.write16(o + ARM_COUNT, ARM_BREATH_TIME);
            nested!(cpu, 0x0608_d7c0, breath_set(cpu, o));
        }
        return;
    }
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    let from = cpu.read16(o + TURN_X) as u32;
    let to = 0x4800u32.wrapping_sub(r & 0xfff) & 0xffff;
    nested!(cpu, 0x0608_e100, wb_turn_toward(cpu, o + ARM_TURN_X, o + ARM_TURN_X + 2, from, to));
    let acts = [cpu.read16(boss + WB_SLOT_1), cpu.read16(boss + WB_SLOT_4)];
    let third = cpu.read8(o + ARM_WHICH) == 3;
    let (to, sway) = if third { (0x2000, 0xfffd_0000) } else { (0x1000, 0x4_0000) };
    let from = cpu.read16(o + TURN_Y) as u32;
    nested!(cpu, 0x0608_e100, wb_turn_toward(cpu, o + ARM_TURN_Y, o + ARM_TURN_Y + 2, from, to));
    let z = cpu.read32(o + POSITION + 8);
    let z = nested!(cpu, 0x0604_6168, actors::approach(cpu, z, sway, 0x800));
    cpu.write32(o + POSITION + 8, z);
    let firing = match cpu.read8(o + ARM_WHICH) {
        0 => acts[0] == ARM_ACTS[0] || cpu.read8(boss + WB_ARM_DOWN + 1) != 0,
        3 => acts[1] == ARM_ACTS[3] || cpu.read8(boss + WB_ARM_DOWN + 2) != 0,
        _ => false,
    };
    if !firing {
        return;
    }
    let n = cpu.read16(o + ARM_COUNT).wrapping_sub(1);
    cpu.write16(o + ARM_COUNT, n);
    if n as i16 > 0 {
        return;
    }
    let difficulty = cpu.read8(DIFFICULTY) as i8 as i32 as u32;
    let every = cpu.read8(ARM_SHOT_EVERY.wrapping_add(difficulty));
    cpu.write16(o + ARM_COUNT, every as u16);
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, ARM_SHOT_SOUND));
    let d = cpu.stack();
    let number = cpu.read8(o + NUMBER);
    cpu.write8(d + bullets::S_NUMBER, number);
    for k in [0, 4] {
        let v = cpu.read32(o + ARM_MOUTH + k);
        cpu.write32(d + bullets::S_POSITION + k, v);
    }
    cpu.write32(d + 8, 0);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    let angle = cpu.read16(o + TURN_Z).wrapping_add(0x1000).wrapping_sub((r & 0x1fff) as u16).wrapping_add(0x8000);
    cpu.write16(d + bullets::S_ANGLE, angle);
    cpu.write32(d + SHELL_SPEED, 0x1_0000);
    cpu.write16(d + SHELL_LAUNCH, angle);
    cpu.write16(d + SHELL_KIND, 0);
    let difficulty = cpu.read8(DIFFICULTY) as i8 as i32 as u32;
    let speed = cpu.read32(ARM_SHOT_SPEEDS.wrapping_add(difficulty << 2));
    cpu.write32(d + SHELL_LAUNCH_SPEED, speed);
    let shot = nested!(cpu, 0x0605_587c, bullets::shell_set(cpu, d));
    if shot != 0 {
        cpu.write32(shot + BEHAVIOUR, SHELL_BEHAVIOUR);
        let flags = cpu.read32(shot + FLAGS) | ARM_SHOT_FLAGS;
        cpu.write32(shot + FLAGS, flags);
        cpu.write8(shot + SPRITE_FRAME, ARM_SHOT_FRAME);
        cpu.write8(shot + SPRITE_KIND, ARM_SHOT_KIND);
        let colour = cpu.read16(CHAR_BASE).wrapping_add(ARM_SHOT_COLOUR);
        cpu.write16(shot + SHELL_COLOURS + 2, colour);
        cpu.write16(shot + SHELL_COLOURS, colour);
    }
    nested!(cpu, 0x0608_cd98, gun_glow(cpu, o + ARM_HIT_FLAGS, o + TURN_Z, o + ARM_MOUTH, 0x8000));
}

/// Step 2: an inner arm breathing: its z turn's rate easing off (by a 16th of it), its y turn's to
/// none (by 10); its count out, 1 again.
pub fn arm_breathe(cpu: &mut Cpu, o: u32) {
    let v = cpu.read16(o + ARM_TURN_Z) as i16 as i32;
    let step = (v / 16) as i16 as i32;
    let v = if step == 0 {
        0
    } else {
        nested!(cpu, 0x0604_6168, actors::approach(cpu, v as u32, 0, step.unsigned_abs()))
    };
    cpu.write16(o + ARM_TURN_Z, v as u16);
    let v = cpu.read16(o + ARM_TURN_Y) as i16 as i32 as u32;
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0, 10));
    cpu.write16(o + ARM_TURN_Y, v as u16);
    let n = cpu.read16(o + ARM_COUNT).wrapping_sub(1);
    cpu.write16(o + ARM_COUNT, n);
    if (n as i16) < 0 {
        cpu.write8(o + ARM_STEP, 1);
    }
}

/// Step 3: downed. Every third frame a smoke puff within 32 dots of it each way (3/4 to 1/2 as
/// big at random); its y turn levelling (0x4000, an eighth, by 32), its x turn's rate easing to
/// -0x300 (by 8) and its z turn's to none (by 16); falling back ever faster (to 4 dots a frame, by
/// 1/256); ended 256 dots back.
pub fn arm_downed(cpu: &mut Cpu, o: u32) {
    let now = cpu.read32(GAME_FRAMES);
    if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, now, 3)) == 0 {
        let e = cpu.stack();
        for k in [0, 4, 8] {
            let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
            let off = 0x40u32.wrapping_sub(r & 0x7f).wrapping_mul(0x8000);
            let v = cpu.read32(o + POSITION + k).wrapping_add(off);
            cpu.write32(e + k, v);
        }
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
        cpu.write32(e + 12, 0xa000u32.wrapping_sub((r & 7) << 11));
        nested!(cpu, 0x0605_77d0, explosions::smoke_puff(cpu, e));
        nested!(cpu, 0x0605_8278, explosions::twin_set(cpu, e));
    }
    let from = cpu.read16(o + TURN_Y) as u32;
    wb_turn(cpu, o + ARM_TURN_Y, from, 0, 0x4000, 8, 32);
    let v = cpu.read16(o + ARM_TURN_X) as i16 as i32 as u32;
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0xffff_fd00, 8));
    cpu.write16(o + ARM_TURN_X, v as u16);
    let v = cpu.read16(o + ARM_TURN_Z) as i16 as i32 as u32;
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0, 16));
    cpu.write16(o + ARM_TURN_Z, v as u16);
    let z = cpu.read32(o + POSITION + 8).wrapping_add(cpu.read32(o + ARM_FALL));
    cpu.write32(o + POSITION + 8, z);
    let fall = cpu.read32(o + ARM_FALL);
    let fall = nested!(cpu, 0x0604_6168, actors::approach(cpu, fall, 0x4_0000, 0x100));
    cpu.write32(o + ARM_FALL, fall);
    if cpu.read32(o + POSITION + 8) as i32 > 0x100_0000 {
        cpu.write8(o + OBJ_MODE, TO_STOP);
    }
}

/// Hit flags' flash: set the frame after a hit (bit 0 to bit 1), cleared the frame after that.
fn flash_step(cpu: &mut Cpu, at: u32) {
    let flags = cpu.read16(at);
    if flags & 2 != 0 {
        cpu.write16(at, flags & !3);
    }
    let flags = cpu.read16(at);
    if flags & 1 != 0 {
        cpu.write16(at, flags | 2);
    }
}

/// The winged boss's wings' knocked-off steps (words, 0 on: the left's +0x2d6, the right's +0x2d4)
/// and their smoke's points (12 bytes each: the right's 5 from +0x1a8, the left's 11 from +0x1f8);
/// the wings' lives (the right's +0x1f4, the left's +0x28c).
const WB_RIGHT_OFF: u32 = 0x2d4;
const WB_LEFT_OFF: u32 = 0x2d6;
const WB_RIGHT_SMOKE: u32 = 0x1a8;
const WB_LEFT_SMOKE: u32 = 0x1f8;
const WB_RIGHT_LIFE: u32 = 0x1f4;
const WB_LEFT_LIFE: u32 = 0x28c;
const WB_DOWN_RATE: u32 = 40;
const WB_DOWN_SCORE: u32 = 10_000;
const WB_LAST_LAP: i16 = 7;
const WING_DOWN_SOUND: u32 = 19;

/// The winged boss's behaviour: its life on the boss gauge. Before step 6, downed once its life is
/// out or its script has run through eight times: the bullets cleared, its laser's sound stopped,
/// the stage's score (10000 and 40 more to the rate if shot down, else none), its core and both
/// wings off (10 more to the rate for each wing already off). Each wing knocked off as its life
/// runs out (sound 19). Fighting (steps 4 and 5), its script; then its step, its turns, its
/// flashes, its wings' smoke along their points, its count.
pub fn wb_behaviour(cpu: &mut Cpu, o: u32) {
    let life = cpu.read32(o + WB_CORE_LIFE);
    nested!(cpu, 0x0604_64b8, hud::boss_gauge_life(cpu, life));
    let downed = cpu.read8(o + WB_STEP) <= 5
        && (cpu.read32(o + WB_CORE_LIFE) as i32 <= 0 || cpu.read16(o + WB_LAPS) as i16 > WB_LAST_LAP);
    if downed {
        cpu.write8(o + WB_STEP, 6);
        nested!(cpu, 0x0605_61e0, screen::bullets_cleared(cpu));
        let control = cpu.read8(o + WB_LASER_CONTROL) as i8 as i32 as u32;
        nested!(cpu, 0x0600_52e0, sound::se_stop(cpu, WB_LASER_SOUND, control));
        let add = |cpu: &mut Cpu, n: u32| {
            let rate = cpu.read32(STAGE_RATE_AT).wrapping_add(n);
            cpu.write32(STAGE_RATE_AT, rate);
        };
        if cpu.read32(o + WB_CORE_LIFE) as i32 <= 0 {
            add(cpu, WB_DOWN_RATE);
            cpu.write32(STAGE_SCORE_AT, WB_DOWN_SCORE);
        } else {
            cpu.write32(STAGE_SCORE_AT, 0);
        }
        cpu.write32(o + WB_CORE_LIFE, 0);
        let flags = cpu.read16(o + WB_CORE) | PART_OFF;
        cpu.write16(o + WB_CORE, flags);
        cpu.write16(o + WB_WAIT, 0);
        for (off, wing) in [(WB_RIGHT_OFF, WB_RIGHT), (WB_LEFT_OFF, WB_LEFT)] {
            let step = cpu.read16(o + off);
            if step == 0 {
                cpu.write16(o + off, step.wrapping_add(1));
                let flags = cpu.read16(o + wing) | PART_OFF;
                cpu.write16(o + wing, flags);
            } else {
                add(cpu, 10);
            }
        }
    }
    for (off, wing, life) in [(WB_RIGHT_OFF, WB_RIGHT, WB_RIGHT_LIFE), (WB_LEFT_OFF, WB_LEFT, WB_LEFT_LIFE)] {
        let step = cpu.read16(o + off);
        if step == 0 && cpu.read32(o + life) as i32 <= 0 {
            cpu.write16(o + off, step.wrapping_add(1));
            let flags = cpu.read16(o + wing) | PART_OFF;
            cpu.write16(o + wing, flags);
            nested!(cpu, 0x0600_50d4, sound::se_set(cpu, WING_DOWN_SOUND));
        }
    }
    if cpu.read8(o + WB_STEP).wrapping_sub(4) <= 1 {
        nested!(cpu, 0x0608_e36c, wb_script(cpu, o));
    }
    match cpu.read8(o + WB_STEP) {
        0 => nested!(cpu, 0x0608_e1e4, wb_come_down(cpu, o)),
        1 => nested!(cpu, 0x0608_e224, wb_settle(cpu, o)),
        2 => nested!(cpu, 0x0608_e300, wb_wait(cpu, o)),
        3 => nested!(cpu, 0x0608_e328, wb_open(cpu, o)),
        4 => nested!(cpu, 0x0608_e424, wb_fight(cpu, o)),
        5 => nested!(cpu, 0x0608_e5c4, wb_laser_phase(cpu, o)),
        6 => nested!(cpu, 0x0608_e698, wb_downed(cpu, o)),
        step => unreachable!("winged boss step {step}"),
    }
    for (turn, rate) in [(TURN_X, WB_TURN_X), (TURN_Y, WB_TURN_Y), (TURN_Z, WB_TURN_Z)] {
        let v = cpu.read16(o + turn).wrapping_add(cpu.read16(o + rate));
        cpu.write16(o + turn, v);
    }
    for at in [WB_CORE, WB_RIGHT, WB_LEFT] {
        flash_step(cpu, o + at);
    }
    for (off, points, last) in [(WB_RIGHT_OFF, WB_RIGHT_SMOKE, 4), (WB_LEFT_OFF, WB_LEFT_SMOKE, 10)] {
        let step = cpu.read16(o + off) as i16 as i32;
        if step == 0 || step - 1 > last {
            continue;
        }
        let p = o + points + 12u32.wrapping_mul((step - 1) as u32);
        if nested!(cpu, 0x0605_968c, explosions::smoke_and_sparks(cpu, p, 4)) as u8 != 0 {
            let step = cpu.read16(o + off).wrapping_add(1);
            cpu.write16(o + off, step);
        }
    }
    let count = cpu.read16(o + WB_COUNT).wrapping_add(1);
    cpu.write16(o + WB_COUNT, count);
}

/// An arm's behaviour: ended with the boss. While the boss is not yet fighting and the arm is up,
/// held at its place on the boss (0xb0 dots below it) and turned as the boss is. Downed once its
/// life or the boss's is out (step 3, off; 10 more to the stage's rate if shot down; its down flag
/// on the boss; sound 17). Then its step, its turns, its flash.
pub fn arm_behaviour(cpu: &mut Cpu, o: u32) {
    let boss = cpu.read32(o + ARM_OF);
    if cpu.read8(boss + OBJ_MODE) as i8 > 111 {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    if cpu.read16(boss + WB_FLAGS) & 1 != 0 && cpu.read8(o + ARM_STEP) <= 2 {
        let place = |cpu: &mut Cpu| boss + WB_ARM_PLACES + 12 * cpu.read8(o + ARM_WHICH) as u32;
        for k in [0, 4] {
            let at = place(cpu);
            let v = cpu.read32(at + k);
            cpu.write32(o + POSITION + k, v);
        }
        let at = place(cpu);
        let z = cpu.read32(at + 8).wrapping_sub(WB_ARM_BELOW);
        cpu.write32(o + POSITION + 8, z);
        for turn in [TURN_X, TURN_Y, TURN_Z] {
            let v = cpu.read16(boss + turn);
            cpu.write16(o + turn, v);
        }
    }
    if cpu.read8(o + ARM_STEP) <= 2
        && (cpu.read32(o + ARM_LIFE) as i32 <= 0 || cpu.read32(boss + WB_CORE_LIFE) as i32 <= 0)
    {
        cpu.write8(o + ARM_STEP, 3);
        let flags = cpu.read16(o + ARM_HIT_FLAGS) | PART_OFF;
        cpu.write16(o + ARM_HIT_FLAGS, flags);
        if cpu.read32(o + ARM_LIFE) as i32 <= 0 {
            let rate = cpu.read32(STAGE_RATE_AT).wrapping_add(10);
            cpu.write32(STAGE_RATE_AT, rate);
        }
        let which = cpu.read8(o + ARM_WHICH) as u32;
        cpu.write8(boss + WB_ARM_DOWN + which, 1);
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, ARM_DOWN_SOUND));
    }
    match cpu.read8(o + ARM_STEP) {
        0 => nested!(cpu, 0x0608_e778, arm_wait(cpu, o)),
        1 => nested!(cpu, 0x0608_e7cc, arm_fight(cpu, o)),
        2 => nested!(cpu, 0x0608_eb18, arm_breathe(cpu, o)),
        3 => nested!(cpu, 0x0608_eb94, arm_downed(cpu, o)),
        step => unreachable!("winged boss arm step {step}"),
    }
    for (turn, rate) in [(TURN_X, ARM_TURN_X), (TURN_Y, ARM_TURN_Y), (TURN_Z, ARM_TURN_Z)] {
        let v = cpu.read16(o + turn).wrapping_add(cpu.read16(o + rate));
        cpu.write16(o + turn, v);
    }
    flash_step(cpu, o + ARM_HIT_FLAGS);
}

/// The winged boss's parts' points (their tables, as parts_set's: the core's 16 into +0xe0, the
/// right wing's 6 into +0x1a8, the left's 12 into +0x1f8), its models (in the boss's model set's
/// list: the core 4, the left wing 0x18, the right 0x14), its wings' guns' and stumps' places, its
/// arms' places on it (whole-dot x, y, z, 6 bytes each, spread with its wings' opening by the
/// signed bytes at 0x060a08fc for x and 0x060a0900 for z).
const WB_CORE_POINTS: (u32, u32, u32) = (0x060a_0904, 16, 0xe0);
const WB_RIGHT_POINTS: (u32, u32, u32) = (0x060a_0984, 6, 0x1a8);
const WB_LEFT_POINTS: (u32, u32, u32) = (0x060a_09b4, 12, 0x1f8);
const WB_CORE_MODEL: u32 = 0x04;
const WB_LEFT_MODEL: u32 = 0x18;
const WB_RIGHT_MODEL: u32 = 0x14;
const WB_ARM_SPOTS: u32 = 0x060a_08e2;
const WB_ARM_SPREAD_X: u32 = 0x060a_08fc;
const WB_ARM_SPREAD_Z: u32 = 0x060a_0900;

/// The model at the winged boss's models' `at` (the flashing one of a pair by its flags `f`).
fn wb_model(cpu: &mut Cpu, at: u32, f: u16) -> u32 {
    let set = cpu.read32(WB_MODEL_SET);
    let models = cpu.read32(set + SHIP_MODELS);
    let list = cpu.read32(models + at);
    cpu.read32(list + 4 * (f & PART_FLASH) as u32)
}

/// At an object's place, turned (z, y, x): the matrix pushed.
fn wb_frame(cpu: &mut Cpu, o: u32) {
    cpu.call(SL_PUSH_MATRIX, &[]);
    let p = [0, 4, 8].map(|k| cpu.read32(o + POSITION + k));
    cpu.call(SL_TRANSLATE, &p);
    for (turn, rot) in [(TURN_Z, SL_ROT_Z), (TURN_Y, SL_ROT_Y), (TURN_X, SL_ROT_X)] {
        let a = cpu.read16(o + turn) as i16 as i32 as u32;
        cpu.call(rot, &[a]);
    }
}

/// The winged boss's draw (its sprite `s`): in its frame, its parts' points brought into the world;
/// unless its core is gone, the core (and its mouth 76 dots on), each wing while on (and its gun's
/// place) or its stump's place once off. While it is not yet fighting, its arms' places, spread as
/// its wings open.
pub fn wb_draw(cpu: &mut Cpu, s: u32) {
    let o = s.wrapping_sub(FLAGS);
    wb_frame(cpu, o);
    for (table, n, out) in [WB_CORE_POINTS, WB_RIGHT_POINTS, WB_LEFT_POINTS] {
        ship_points(cpu, table, n, o + out);
    }
    let f = cpu.read16(o + WB_CORE);
    if f & PART_GONE == 0 {
        let model = wb_model(cpu, WB_CORE_MODEL, f);
        cpu.call(SL_PUT_POLYGON, &[model]);
        cpu.call(SL_CALC_POINT, &[0, 0, 0xffb4_0000, o + WB_MOUTH]);
        let f = cpu.read16(o + WB_LEFT);
        if f & PART_OFF == 0 {
            let model = wb_model(cpu, WB_LEFT_MODEL, f);
            cpu.call(SL_PUT_POLYGON, &[model]);
            cpu.call(SL_CALC_POINT, &[0xffd2_0000, 0xffe8_0000, 0xfff0_0000, o + WB_LEFT_GUN]);
        } else {
            cpu.call(SL_CALC_POINT, &[0xfff4_0000, 0, 0xffe0_0000, o + WB_LEFT_STUMP]);
        }
        let f = cpu.read16(o + WB_RIGHT);
        if f & PART_OFF == 0 {
            let model = wb_model(cpu, WB_RIGHT_MODEL, f);
            cpu.call(SL_PUT_POLYGON, &[model]);
            cpu.call(SL_CALC_POINT, &[0x2e_0000, 0xffe8_0000, 0xfff0_0000, o + WB_RIGHT_GUN]);
        } else {
            cpu.call(SL_CALC_POINT, &[0xc_0000, 0, 0xffe0_0000, o + WB_RIGHT_STUMP]);
        }
    }
    if cpu.read16(o + WB_FLAGS) & 1 != 0 {
        for i in 0..4 {
            let opening = cpu.read16(o + WB_OPENING) as i16 as i32 as u32;
            let spread = |cpu: &mut Cpu, reach: u32, table: u32| {
                let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, opening));
                let v = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, reach));
                let k = nested!(cpu, 0x0600_6a78, math::quotient(cpu, 3 * i, 3));
                let c = cpu.read8(table.wrapping_add(k)) as i8 as i32 as u32;
                v.wrapping_mul(c)
            };
            let dx = spread(cpu, 0x10_0000, WB_ARM_SPREAD_X);
            let dz = spread(cpu, 0x20_0000, WB_ARM_SPREAD_Z);
            let [x, y, z] = [0, 2, 4].map(|k| (cpu.read16(WB_ARM_SPOTS + 6 * i + k) as u32) << 16);
            cpu.call(SL_CALC_POINT, &[x.wrapping_add(dx), y, z.wrapping_add(dz), o + WB_ARM_PLACES + 12 * i]);
        }
    }
    cpu.call(SL_POP_MATRIX, &[]);
}

/// The arms' points (as parts_set's, into +0x90) and models, by arm: the left outer, the left
/// inner, the right inner, the right outer.
const ARM_POINTS: [(u32, u32); 4] = [(0x060a_0a14, 11), (0x060a_0a64, 10), (0x060a_0ab4, 9), (0x060a_0af4, 17)];
const ARM_MODELS: [u32; 4] = [0x0c, 0x08, 0x00, 0x10];
const ARM_PARTS: u32 = 0x90;

/// An arm's draw: in its frame, its points brought into the world; unless it is gone, its model
/// and its mouth's place (and the right outer arm's second, +0x174).
fn arm_draw(cpu: &mut Cpu, s: u32, which: usize, mouths: &[[u32; 4]]) {
    let o = s.wrapping_sub(FLAGS);
    wb_frame(cpu, o);
    let (table, n) = ARM_POINTS[which];
    ship_points(cpu, table, n, o + ARM_PARTS);
    let f = cpu.read16(o + ARM_HIT_FLAGS);
    if f & PART_GONE == 0 {
        let model = wb_model(cpu, ARM_MODELS[which], f);
        cpu.call(SL_PUT_POLYGON, &[model]);
        for &[x, y, z, out] in mouths {
            cpu.call(SL_CALC_POINT, &[x, y, z, o + out]);
        }
    }
    cpu.call(SL_POP_MATRIX, &[]);
}

/// The left outer arm's draw.
pub fn arm_draw_0(cpu: &mut Cpu, s: u32) {
    arm_draw(cpu, s, 0, &[[0, 0xfff8_0000, 0xffe8_0000, ARM_MOUTH]]);
}

/// The left inner arm's draw.
pub fn arm_draw_1(cpu: &mut Cpu, s: u32) {
    arm_draw(cpu, s, 1, &[[0, 0, 0xffe0_0000, ARM_MOUTH]]);
}

/// The right inner arm's draw.
pub fn arm_draw_2(cpu: &mut Cpu, s: u32) {
    arm_draw(cpu, s, 2, &[[0, 0, 0xffe0_0000, ARM_MOUTH]]);
}

/// The right outer arm's draw.
pub fn arm_draw_3(cpu: &mut Cpu, s: u32) {
    arm_draw(cpu, s, 3, &[[0x16_0000, 0x18_0000, 0xffdc_0000, ARM_MOUTH], [0x10_0000, 0xffe3_0000, 0xffe0_0000, 0x174]]);
}

/// The winged boss's set-up: model kind 40's frame 7 flipped; 384 dots up and 256 back, upside
/// down and back to front, not yet fighting with its wings shut; its slots 0x10-0x40, its core
/// 28000 and wings 8000 each, the stage's rate cleared; then its four arms (list 2, 0x180 bytes,
/// numbered_exec's, each taking an enemy number but marking the boss's in the players' table): at
/// their places across it, the outer pair 18000, the inner 16000. The boss gauge at 28000, sound
/// 78. Stops where no number or object is free.
const WB_BEHAVIOUR: u32 = 0x0608_ecd4;
const WB_DRAW: u32 = 0x0608_f15c;
const WB_SET_FLAGS: u32 = 0xc8_0000;
const WB_FLIP_KIND: u32 = 40;
const WB_FLIP_FRAME: u32 = 7;
const WB_FLIP_BITS: u32 = 0x100;
const WB_CORE_START: u32 = 28000;
const WB_WING_START: u32 = 8000;
const WB_SOUND: u32 = 78;
const ARM_SIZE: u32 = 0x180;
const ARM_BEHAVIOUR: u32 = 0x0608_efb8;
const ARM_DRAWS: [u32; 4] = [0x0608_f450, 0x0608_f554, 0x0608_f654, 0x0608_f754];
const ARM_XS: [u32; 4] = [0xff8e_0000, 0xffb2_0000, 0x4e_0000, 0x72_0000];
const ARM_LIVES: [u32; 4] = [18000, 16000, 16000, 18000];

pub fn wb_set(cpu: &mut Cpu, o: u32, n: u32) {
    nested!(cpu, 0x0604_5cdc, models::model_flip_bits(cpu, WB_FLIP_KIND, WB_FLIP_FRAME, WB_FLIP_BITS));
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + BEAM_LIST));
    cpu.write16(o + WB_COUNT, 0);
    cpu.write16(o + WB_WAIT, 0);
    cpu.write32(o + BEHAVIOUR, WB_BEHAVIOUR);
    let flags = cpu.read32(o + FLAGS) | WB_SET_FLAGS;
    cpu.write32(o + FLAGS, flags);
    cpu.write32(o + MODEL, WB_DRAW);
    cpu.write8(o + NUMBER, n as u8);
    let number = cpu.read8(o + NUMBER) as i8 as i32;
    cpu.write32(PLAYERS.wrapping_add((4 * number) as u32), o);
    cpu.write16(o + WB_FLAGS, 1);
    cpu.write16(o + WB_OPENING, 0);
    cpu.write8(o + WB_STEP, 0);
    for (k, v) in [(0, 0), (4, 0x180_0000), (8, 0x100_0000)] {
        cpu.write32(o + POSITION + k, v);
    }
    for at in [0x8c, 0x90, WB_FALL] {
        cpu.write32(o + at, 0);
    }
    cpu.write16(o + TURN_Y, 0x8000);
    cpu.write16(o + TURN_X, 0xc001);
    for at in [WB_TURN_X, WB_TURN_Y, WB_TURN_Z] {
        cpu.write16(o + at, 0);
        cpu.write16(o + at + 2, 0);
    }
    cpu.write16(o + WB_SCRIPT_AT, 0);
    cpu.write16(o + WB_SLOTS, 0);
    for (k, at) in [WB_SLOT_1, WB_SLOT_2, WB_SLOT_3, WB_SLOT_4].into_iter().enumerate() {
        cpu.write16(o + at, 0x10 * (k as u16 + 1));
    }
    for k in 0..4 {
        cpu.write8(o + WB_ARM_DOWN + k, 0);
    }
    for at in [WB_LAPS, WB_LASER_TIME, WB_MISSILES] {
        cpu.write16(o + at, 0);
    }
    cpu.write8(o + WB_FAN, 0);
    cpu.write32(STAGE_RATE_AT, 0);
    cpu.write8(o + WB_LASER_CONTROL, 0);
    cpu.write16(o + 0x290, 0);
    cpu.write16(o + 0x292, 0);
    let parts = [
        (WB_CORE_POINTS, WB_CORE, WB_CORE_LIFE, WB_CORE_START, None),
        (WB_RIGHT_POINTS, WB_RIGHT, WB_RIGHT_LIFE, WB_WING_START, Some(WB_RIGHT_OFF)),
        (WB_LEFT_POINTS, WB_LEFT, WB_LEFT_LIFE, WB_WING_START, Some(WB_LEFT_OFF)),
    ];
    for ((table, count, points), state, life, start, off) in parts {
        cpu.write16(o + state, 0);
        cpu.write32(o + life, start);
        if let Some(off) = off {
            cpu.write16(o + off, 0);
        }
        let number = cpu.read8(o + NUMBER) as u32;
        let (points, state, life) = (o + points, o + state, o + life);
        cpu.nested_with_stack(&PARTS_SET_PROLOGUE, 0x0605_6ea4, &[table, number], |cpu| {
            parts::parts_set(cpu, count, points, state, life, table, number)
        });
    }
    for which in 0..4 {
        let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, ENEMY_NUMBERS)) as i8;
        if n == -1 {
            return;
        }
        let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, ARM_SIZE)) as i8 as i32 as u32;
        let a = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, NUMBERED_EXEC));
        if a == 0 {
            return;
        }
        nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, a + FLAGS));
        nested!(cpu, 0x0604_6284, hits::hit_init(cpu, a + BEAM_LIST));
        cpu.write32(a + BEHAVIOUR, ARM_BEHAVIOUR);
        let flags = cpu.read32(a + FLAGS) | WB_SET_FLAGS;
        cpu.write32(a + FLAGS, flags);
        cpu.write8(a + NUMBER, n as u8);
        let number = cpu.read8(a + NUMBER) as i8 as i32;
        cpu.write32(PLAYERS.wrapping_add((4 * number) as u32), o);
        cpu.write8(a + ARM_WHICH, which as u8);
        cpu.write32(a + ARM_OF, o);
        cpu.write32(a + ARM_FALL, 0);
        for at in [ARM_TURN_X, ARM_TURN_Y, ARM_TURN_Z] {
            cpu.write16(a + at, 0);
            cpu.write16(a + at + 2, 0);
        }
        cpu.write16(a + 0x74, 0x80);
        cpu.write16(a + ARM_HIT_FLAGS, 0);
        cpu.write16(a + ARM_COUNT, 0);
        cpu.write16(a + ARM_COUNT + 2, 0);
        cpu.write8(a + ARM_STEP, 0);
        cpu.write32(a + MODEL, ARM_DRAWS[which]);
        for (k, v) in [(0, ARM_XS[which]), (4, 0), (8, 0)] {
            cpu.write32(a + POSITION + k, v);
        }
        cpu.write32(a + ARM_LIFE, ARM_LIVES[which]);
        let (table, count) = ARM_POINTS[which];
        let number = cpu.read8(o + NUMBER) as u32;
        let (points, state, life) = (a + ARM_PARTS, a + ARM_HIT_FLAGS, a + ARM_LIFE);
        cpu.nested_with_stack(&PARTS_SET_PROLOGUE, 0x0605_6ea4, &[table, number], |cpu| {
            parts::parts_set(cpu, count, points, state, life, table, number)
        });
        cpu.write16(a + 0x72, 0);
        cpu.write32(a + POSITION + 8, 0);
        cpu.write32(a + 0x78, 0);
        cpu.write32(a + 0x7c, 0);
    }
    nested!(cpu, 0x0604_64d8, hud::boss_gauge_set(cpu, WB_CORE_START));
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, WB_SOUND));
}

/// The winged boss made (numbered: list 2, 0x2dc bytes, numbered_exec's; an enemy's number): its
/// number, or 0 if none was free.
const WB_SIZE: u32 = 0x2dc;

pub fn wb_make(cpu: &mut Cpu) -> u32 {
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, ENEMY_NUMBERS)) as i8;
    if n == -1 {
        return 0;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, WB_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, NUMBERED_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0608_f878, wb_set(cpu, o, n as i32 as u32));
    n as i32 as u32
}

// ---- the trails ----------------------------------------------------------------------------------

/// A sprite's description (sprites.rs's, on the stack): its flags (0x100 flipped), its place (x, y,
/// z), its scales (x, y), its colours (two words), its kind and frame, its angle (a word), its lift.
const SD_FLAGS: u32 = 0x00;
const SD_X: u32 = 0x04;
const SD_Y: u32 = 0x08;
const SD_Z: u32 = 0x0c;
const SD_SCALE_X: u32 = 0x10;
const SD_SCALE_Y: u32 = 0x14;
const SD_COLOUR: u32 = 0x18;
const SD_COLOUR_2: u32 = 0x1a;
const SD_KIND: u32 = 0x1e;
const SD_FRAME: u32 = 0x1f;
const SD_ANGLE: u32 = 0x20;
const SD_LIFT: u32 = 0x23;
const SD_FLIPPED: u32 = 0x100;

/// The sprite `d` drawn once more, flipped, a dot nearer, `grow` bigger both ways, in colour
/// `colour` past the first character's; then put back (unflipped).
pub fn sprite_again(cpu: &mut Cpu, d: u32, colour: u32, grow: u32) {
    let old = cpu.read16(d + SD_COLOUR);
    let flags = cpu.read32(d + SD_FLAGS) | SD_FLIPPED;
    let z = cpu.read32(d + SD_Z).wrapping_add(1);
    cpu.write32(d + SD_Z, z);
    for at in [SD_SCALE_X, SD_SCALE_Y] {
        let v = cpu.read32(d + at).wrapping_add(grow);
        cpu.write32(d + at, v);
    }
    cpu.write32(d + SD_FLAGS, flags);
    let c = (colour as u16).wrapping_add(cpu.read16(CHAR_BASE));
    cpu.write16(d + SD_COLOUR, c);
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
    let flags = cpu.read32(d + SD_FLAGS) & !SD_FLIPPED;
    let z = cpu.read32(d + SD_Z).wrapping_sub(1);
    cpu.write32(d + SD_Z, z);
    for at in [SD_SCALE_X, SD_SCALE_Y] {
        let v = cpu.read32(d + at).wrapping_sub(grow);
        cpu.write32(d + at, v);
    }
    cpu.write32(d + SD_FLAGS, flags);
    cpu.write16(d + SD_COLOUR, old);
}

/// A trail's touch (an actor_exec hit box left along it, unseen): its life (+0x6c, a word) and the
/// frames' parity it is hittable on (+0x6e, a byte: on the others it has 0x80000).
const TOUCH_LIFE: u32 = 0x6c;
const TOUCH_PARITY: u32 = 0x6e;
const TOUCH_OFF: u32 = 0x8_0000;

/// A touch: ended once its life runs out; hittable every other frame.
pub fn touch_behaviour(cpu: &mut Cpu, o: u32) {
    let n = cpu.read16(o + TOUCH_LIFE).wrapping_sub(1);
    cpu.write16(o + TOUCH_LIFE, n);
    if (n as i16) < 0 {
        cpu.write8(o + OBJ_MODE, TO_STOP);
    }
    let parity = cpu.read32(GAME_FRAMES) & 1;
    let flags = cpu.read32(o + FLAGS);
    let flags = if parity == cpu.read8(o + TOUCH_PARITY) as u32 { flags | TOUCH_OFF } else { flags & !TOUCH_OFF };
    cpu.write32(o + FLAGS, flags);
}

/// A touch made (list 2, 0x70 bytes, actor_exec's) from the description `d` (its place, number,
/// life at +0x2c and parity at +0x2f) with the boxes `boxes`.
const TOUCH_SIZE: u32 = 0x70;
const TOUCH_BEHAVIOUR: u32 = 0x0608_fdd4;
const TOUCH_FLAGS: u32 = 0x8000_0400;
/// Its boxes, its side (1: the enemies') and type.
const HIT_BOXES: u32 = 0x3c;
const HIT_SIDE: u32 = 0x54;
const HIT_TYPE: u32 = 0x55;

pub fn touch_set(cpu: &mut Cpu, d: u32, boxes: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, TOUCH_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + BEAM_LIST));
    cpu.write32(o + BEHAVIOUR, TOUCH_BEHAVIOUR);
    cpu.write32(o + FLAGS, TOUCH_FLAGS);
    let number = cpu.read8(d + bullets::S_NUMBER);
    cpu.write8(o + NUMBER, number);
    for k in [0, 4] {
        let v = cpu.read32(d + k);
        cpu.write32(o + POSITION + k, v);
    }
    cpu.write32(o + POSITION + 8, 0);
    cpu.write32(o + HIT_BOXES, boxes);
    cpu.write8(o + HIT_SIDE, 1);
    cpu.write8(o + HIT_TYPE, 5);
    let life = cpu.read16(d + bullets::S_ANGLE);
    cpu.write16(o + TOUCH_LIFE, life);
    let parity = cpu.read8(d + bullets::S_ANGLE + 3);
    cpu.write8(o + TOUCH_PARITY, parity);
}

/// The waving trail (an actor_exec sprite chain): its touches' parity (+0x6d, a byte, turned each
/// time), its count once ended (+0x6e, a word), its wave (+0x70, a word), the whole dots it moved
/// this frame (+0x72 x, +0x73 y, bytes), the places of its twelve links (x from +0x74, y from
/// +0xa4) and its last 72 frames' moves (x from +0xd4, y from +0x11c, its turn's high byte from
/// +0x164).
const TRAIL_PARITY: u32 = 0x6d;
const TRAIL_ENDED: u32 = 0x6e;
const TRAIL_WAVE: u32 = 0x70;
const TRAIL_DX: u32 = 0x72;
const TRAIL_DY: u32 = 0x73;
const TRAIL_LINK_X: u32 = 0x74;
const TRAIL_LINK_Y: u32 = 0xa4;
const TRAIL_MOVES_X: u32 = 0xd4;
const TRAIL_MOVES_Y: u32 = 0x11c;
const TRAIL_TURNS: u32 = 0x164;
const TRAIL_LINKS: u32 = 12;
const TRAIL_MOVES: u32 = 72;

/// Whole dots of a 16.16 value, toward zero.
fn whole(v: u32) -> i16 {
    ((v as i32 + if (v as i32) < 0 { 0xffff } else { 0 }) >> 16) as i16
}

/// The waving trail's move: on the hardest two difficulties waving (its heading 0x4000 off its wave,
/// mirrored once the wave is past half a turn; the wave 0x300 on a frame) at 2.5 dots a frame, else
/// straight along its wave turned a quarter at 1.5; the whole dots it moved kept.
pub fn trail_wave(cpu: &mut Cpu, o: u32) {
    let speed = if cpu.read8(DIFFICULTY) as i8 > 1 {
        let w = cpu.read16(o + TRAIL_WAVE);
        let heading = if w & 0x8000 != 0 { 0x4000u16.wrapping_sub(w) } else { w.wrapping_add(0x4000) };
        cpu.write16(o + TURN_Z, heading);
        let w = cpu.read16(o + TRAIL_WAVE).wrapping_add(0x300);
        cpu.write16(o + TRAIL_WAVE, w);
        0x2_8000
    } else {
        let w = cpu.read16(o + TRAIL_WAVE).wrapping_add(0x4000);
        cpu.write16(o + TURN_Z, w);
        0x1_8000
    };
    for (at, d, quarter) in [(POSITION, TRAIL_DX, 0), (POSITION + 4, TRAIL_DY, 0x4000)] {
        let was = cpu.read32(o + at);
        let turn = (cpu.read16(o + TURN_Z) as i16 as i32 + quarter) as u32;
        let turn = if quarter == 0 { turn } else { turn & 0x7fff };
        let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, turn));
        let v = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, speed));
        let now = cpu.read32(o + at).wrapping_add(v);
        cpu.write32(o + at, now);
        cpu.write8(o + d, whole(now).wrapping_sub(whole(was)) as u8);
    }
}

/// A trail's moves kept (the whole dots it moved, at `d` and `d + 1`; `n` frames of them at `x`,
/// `y` and `turns`), its turn's high byte too.
fn trail_keep(cpu: &mut Cpu, o: u32, d: u32, x: u32, y: u32, turns: u32, n: u32) {
    let dx = cpu.read8(o + d) as i8 as i32 as u32;
    nested!(cpu, 0x0605_62b4, actors::shift_in_bytes(cpu, o + x, dx, n));
    let dy = cpu.read8(o + d + 1) as i8 as i32 as u32;
    nested!(cpu, 0x0605_62b4, actors::shift_in_bytes(cpu, o + y, dy, n));
    let turn = cpu.read16(o + TURN_Z) as i16 as i32;
    nested!(cpu, 0x0600_8164, clib::ashiftrt_r4::<8>(cpu, (turn + if turn < 0 { 0xff } else { 0 }) as u32));
    let high = (((turn + if turn < 0 { 0xff } else { 0 }) >> 8) as i8) as i32 as u32;
    nested!(cpu, 0x0605_62b4, actors::shift_in_bytes(cpu, o + turns, high, n));
}

/// The waving trail's touches: their boxes, their life.
const TRAIL_TOUCH_BOXES: u32 = 0x060a_0e1a;
const TRAIL_TOUCH_LIFE: u16 = 48;

/// The waving trail: once off screen it stops (the rest of it runs out, ended 72 frames on);
/// before, it moves and every fourth frame leaves a touch where it is. Its moves kept.
pub fn trail_behaviour(cpu: &mut Cpu, o: u32) {
    let ended = cpu.read16(o + TRAIL_ENDED);
    if ended != 0 {
        let n = ended.wrapping_add(1);
        cpu.write16(o + TRAIL_ENDED, n);
        if n > 71 {
            cpu.write8(o + OBJ_MODE, TO_STOP);
            return;
        }
    } else if nested!(cpu, 0x0605_6a24, aim::off_screen(cpu, o + FLAGS)) as u8 != 0 {
        let n = cpu.read16(o + TRAIL_ENDED).wrapping_add(1);
        cpu.write16(o + TRAIL_ENDED, n);
        cpu.write8(o + TRAIL_DX, 0);
        cpu.write8(o + TRAIL_DY, 0);
    } else {
        nested!(cpu, 0x0608_fecc, trail_wave(cpu, o));
        if cpu.read32(GAME_FRAMES) & 3 == 0 {
            trail_touch(cpu, o, TRAIL_PARITY, TRAIL_TOUCH_LIFE, TRAIL_TOUCH_BOXES);
        }
    }
    trail_keep(cpu, o, TRAIL_DX, TRAIL_MOVES_X, TRAIL_MOVES_Y, TRAIL_TURNS, TRAIL_MOVES);
}

/// A touch left where a trail is (its parity, at `parity`, turned each time).
fn trail_touch(cpu: &mut Cpu, o: u32, parity_at: u32, life: u16, boxes: u32) {
    let d = cpu.stack();
    for k in [0, 4] {
        let v = cpu.read32(o + POSITION + k);
        cpu.write32(d + k, v);
    }
    let number = cpu.read8(o + NUMBER);
    cpu.write8(d + bullets::S_NUMBER, number);
    cpu.write16(d + bullets::S_ANGLE, life);
    let parity = cpu.read8(o + parity_at);
    cpu.write16(d + bullets::S_ANGLE + 2, parity as u16);
    cpu.write8(o + parity_at, parity ^ 1);
    nested!(cpu, 0x0608_fe24, touch_set(cpu, d, boxes));
}

/// The waving trail's draw: unless the game is paused, its links moved on by their moves (link k by
/// the move 5k frames back); then each link a sprite (kind 46, lift 10; colour 0x20 past the first
/// character's on odd passes of the main loop, else 0x30) at its place turned by its turn then,
/// shrinking along it from its head (by 0xe00 a link from 1 and a quarter, or 0x200 from a quarter,
/// on the easier difficulties; more by the main loop's pass), its frame cycling with the game's
/// frames (the head 0-3, then 4-7, the ninth 8-11, the last three 12-17 flickering and flipped at
/// random); its draw count.
const TRAIL_KIND: u8 = 46;
const TRAIL_LIFT: u8 = 10;
const TRAIL_COLOURS: [u16; 2] = [0x30, 0x20];
const TRAIL_COUNT: u32 = 0x6c;

pub fn trail_draw(cpu: &mut Cpu, s: u32) {
    let o = s.wrapping_sub(FLAGS);
    let d = cpu.stack();
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, d));
    cpu.write32(d + SD_Z, 0);
    cpu.write8(d + SD_LIFT, TRAIL_LIFT);
    cpu.write8(d + SD_KIND, TRAIL_KIND);
    let odd = cpu.read32(LOOPS) & 1;
    let colour = cpu.read16(CHAR_BASE).wrapping_add(TRAIL_COLOURS[odd as usize]);
    cpu.write16(d + SD_COLOUR, colour);
    cpu.write16(d + SD_COLOUR_2, colour);
    if cpu.read32(GAME_FLAGS) & HELD == 0 {
        for i in 0..TRAIL_LINKS {
            for (at, moves) in [(TRAIL_LINK_X, TRAIL_MOVES_X), (TRAIL_LINK_Y, TRAIL_MOVES_Y)] {
                let m = cpu.read8(o + moves + 5 * i) as i8 as i32 as u32;
                let v = cpu.read32(o + at + 4 * i).wrapping_add(m << 16);
                cpu.write32(o + at + 4 * i, v);
            }
        }
    }
    let mut frame = cpu.read16(GAME_FRAMES + 2);
    let pass = cpu.read32(LOOPS) & 3;
    let (step, mut scale) = if cpu.read8(DIFFICULTY) as i8 > 1 {
        (0xffff_f200u32, (pass << 12) + 0x1_0000)
    } else {
        (0xffff_fe00, (pass << 10) + 0x4000)
    };
    cpu.write32(d + SD_SCALE_X, scale);
    for i in 0..TRAIL_LINKS {
        scale = scale.wrapping_add(step);
        cpu.write32(d + SD_SCALE_X, scale);
        let x = cpu.read32(o + TRAIL_LINK_X + 4 * i);
        cpu.write32(d + SD_X, x);
        let y = cpu.read32(o + TRAIL_LINK_Y + 4 * i);
        cpu.write32(d + SD_Y, y);
        frame = frame.wrapping_add(1) & 3;
        let turn = (cpu.read8(o + TRAIL_TURNS + 5 * i) as i8 as i16) << 8;
        cpu.write16(d + SD_ANGLE, turn as u16);
        let flicker = |cpu: &mut Cpu, from: u8| (cpu.read8(GAME_FRAMES_LOW) & 1) + from;
        let f = match i {
            0 => frame as u8,
            1..=6 => {
                cpu.write32(d + SD_SCALE_Y, 0xa000);
                frame as u8 + 4
            }
            7 => {
                cpu.write32(d + SD_SCALE_Y, 0x8000);
                frame as u8 + 4
            }
            8 => {
                cpu.write32(d + SD_SCALE_Y, 0xa000);
                frame as u8 + 8
            }
            9 => flicker(cpu, 12),
            10 => flicker(cpu, 14),
            _ => flicker(cpu, 16),
        };
        cpu.write8(d + SD_FRAME, f);
        if i > 8 {
            let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
            cpu.write32(d + SD_FLAGS, r & SD_FLIPPED);
        }
        nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
    }
    let n = cpu.read8(o + TRAIL_COUNT).wrapping_add(1);
    cpu.write8(o + TRAIL_COUNT, n);
}

/// A waving trail made (list 2, 0x270 bytes, actor_exec's; sound 43) from the description `d` (its
/// place, number; its direction, which on the easier difficulties only sets it 10 dots left or
/// right and it goes straight up; its flags, any of which make it colour 0x10 rather than 0x30).
const TRAIL_SIZE: u32 = 0x270;
const TRAIL_BEHAVIOUR: u32 = 0x0608_ffe4;
const TRAIL_DRAW: u32 = 0x0609_00f0;
const TRAIL_FLAGS: u32 = 0xc8_0400;
const TRAIL_SOUND: u32 = 43;
const OBJ_COLOUR: u32 = 0x2a;

pub fn trail_set(cpu: &mut Cpu, d: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, TRAIL_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + BEAM_LIST));
    cpu.write32(o + BEHAVIOUR, TRAIL_BEHAVIOUR);
    let number = cpu.read8(d + bullets::S_NUMBER);
    cpu.write8(o + NUMBER, number);
    cpu.write32(o + FLAGS, TRAIL_FLAGS);
    cpu.write32(o + MODEL, TRAIL_DRAW);
    for k in [0, 4] {
        let v = cpu.read32(d + k);
        cpu.write32(o + POSITION + k, v);
    }
    cpu.write32(o + POSITION + 8, 0);
    cpu.write16(o + TRAIL_ENDED, 0);
    cpu.write8(o + TRAIL_PARITY, 0);
    let colour = if cpu.read16(d + SHOT_FLAGS) != 0 { 0x10 } else { 0x30 };
    let colour = cpu.read16(CHAR_BASE).wrapping_add(colour);
    cpu.write16(o + OBJ_COLOUR, colour);
    let wave = if cpu.read8(DIFFICULTY) as i8 > 1 {
        cpu.read16(d + bullets::S_ANGLE)
    } else {
        let side = if cpu.read16(d + bullets::S_ANGLE) != 0 { 0xfff6_0000 } else { 0xa_0000 };
        let x = cpu.read32(o + POSITION).wrapping_add(side);
        cpu.write32(o + POSITION, x);
        0x4000
    };
    cpu.write16(o + TRAIL_WAVE, wave);
    cpu.write8(o + TRAIL_DX, 0);
    cpu.write8(o + TRAIL_DY, 0);
    trail_fill(cpu, o, TRAIL_MOVES_X, TRAIL_MOVES_Y, TRAIL_TURNS, TRAIL_MOVES);
    for i in 0..TRAIL_LINKS {
        let x = cpu.read32(o + POSITION);
        cpu.write32(o + TRAIL_LINK_X + 4 * i, x);
        let y = cpu.read32(o + POSITION + 4);
        cpu.write32(o + TRAIL_LINK_Y + 4 * i, y);
    }
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, TRAIL_SOUND));
}

/// A trail's kept moves cleared, its turns filled with its turn's high byte.
fn trail_fill(cpu: &mut Cpu, o: u32, x: u32, y: u32, turns: u32, n: u32) {
    nested!(cpu, 0x0605_6294, actors::fill_bytes(cpu, o + x, 0, n));
    nested!(cpu, 0x0605_6294, actors::fill_bytes(cpu, o + y, 0, n));
    let turn = cpu.read16(o + TURN_Z) as i16 as i32;
    let turn = turn + if turn < 0 { 0xff } else { 0 };
    nested!(cpu, 0x0600_8164, clib::ashiftrt_r4::<8>(cpu, turn as u32));
    let high = ((turn >> 8) as i8) as i32 as u32;
    nested!(cpu, 0x0605_6294, actors::fill_bytes(cpu, o + turns, high, n));
}

/// The rebounding trail (an actor_exec sprite chain going straight): its touches' parity (+0x6c),
/// its count once ended (+0x6e, a word), the whole dots it moved this frame (+0x70 x, +0x71 y), the
/// places of its eight links (x from +0x74, y from +0x94) and its last 96 frames' moves (x from
/// +0xb4, y from +0x114, its turn's high byte from +0x174).
const REBOUND_PARITY: u32 = 0x6c;
const REBOUND_ENDED: u32 = 0x6e;
const REBOUND_DX: u32 = 0x70;
const REBOUND_DY: u32 = 0x71;
const REBOUND_LINK_X: u32 = 0x74;
const REBOUND_LINK_Y: u32 = 0x94;
const REBOUND_MOVES_X: u32 = 0xb4;
const REBOUND_MOVES_Y: u32 = 0x114;
const REBOUND_TURNS: u32 = 0x174;
const REBOUND_LINKS: u32 = 8;
const REBOUND_MOVES: u32 = 96;
/// The sides it rebounds from (112 dots either way), its speed (1.5 dots a frame and half a dot
/// more for each difficulty), the bottom it ends past (128 dots down).
const REBOUND_SIDE: i32 = 0x70_0000;
const REBOUND_BOTTOM: i32 = 0x80_0000;
const REBOUND_TOUCH_BOXES: u32 = 0x060a_0e22;
const REBOUND_TOUCH_LIFE: u16 = 72;

/// The rebounding trail's move: once past the side it heads for, it ends there (its count on) and
/// a new one starts from where it is, mirrored; along its heading (x by its sine, y back by its
/// cosine), the whole dots it moved kept (none once it has ended).
pub fn rebound_move(cpu: &mut Cpu, o: u32) {
    let (x0, y0) = (cpu.read32(o + POSITION), cpu.read32(o + POSITION + 4));
    let heading = cpu.read16(o + TURN_Z) as i16;
    let past = if heading > 0 { x0 as i32 > REBOUND_SIDE } else { (x0 as i32) < -REBOUND_SIDE };
    if past {
        let n = cpu.read16(o + REBOUND_ENDED).wrapping_add(1);
        cpu.write16(o + REBOUND_ENDED, n);
    }
    let difficulty = cpu.read8(DIFFICULTY) as i8 as i32 as u32;
    let speed = (((difficulty << 8) >> 1) << 8).wrapping_add(0x1_8000);
    let turn = cpu.read16(o + TURN_Z) as i16 as i32 as u32;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, turn));
    let v = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, speed));
    let x = cpu.read32(o + POSITION).wrapping_add(v);
    cpu.write32(o + POSITION, x);
    cpu.write8(o + REBOUND_DX, whole(x).wrapping_sub(whole(x0)) as u8);
    let turn = cpu.read16(o + TURN_Z) as i16 as i32 as u32;
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, turn));
    let v = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, c, speed));
    let y = cpu.read32(o + POSITION + 4).wrapping_sub(v);
    cpu.write32(o + POSITION + 4, y);
    cpu.write8(o + REBOUND_DY, whole(y).wrapping_sub(whole(y0)) as u8);
    if cpu.read16(o + REBOUND_ENDED) != 0 {
        let d = cpu.stack();
        let number = cpu.read8(o + NUMBER);
        cpu.write8(d + bullets::S_NUMBER, number);
        for k in [0, 4] {
            let v = cpu.read32(o + POSITION + k);
            cpu.write32(d + k, v);
        }
        cpu.write32(d + 8, 0);
        let mirrored = cpu.read16(o + TURN_Z).wrapping_neg();
        cpu.write16(d + bullets::S_ANGLE, mirrored);
        nested!(cpu, 0x0609_0e04, rebound_set(cpu, d));
        cpu.write8(o + REBOUND_DX, 0);
        cpu.write8(o + REBOUND_DY, 0);
    }
}

/// The rebounding trail: once past the bottom it stops (the rest of it runs out, ended 72 frames
/// on); before, it moves and every fourth frame leaves a touch where it is. Its moves kept.
pub fn rebound_behaviour(cpu: &mut Cpu, o: u32) {
    let ended = cpu.read16(o + REBOUND_ENDED);
    if ended != 0 {
        let n = ended.wrapping_add(1);
        cpu.write16(o + REBOUND_ENDED, n);
        if n > 71 {
            cpu.write8(o + OBJ_MODE, TO_STOP);
            return;
        }
    } else if cpu.read32(o + POSITION + 4) as i32 > REBOUND_BOTTOM {
        cpu.write16(o + REBOUND_ENDED, ended.wrapping_add(1));
        cpu.write8(o + REBOUND_DX, 0);
        cpu.write8(o + REBOUND_DY, 0);
    } else {
        nested!(cpu, 0x0609_049c, rebound_move(cpu, o));
        if cpu.read32(GAME_FRAMES) & 3 == 0 {
            trail_touch(cpu, o, REBOUND_PARITY, REBOUND_TOUCH_LIFE, REBOUND_TOUCH_BOXES);
        }
    }
    trail_keep(cpu, o, REBOUND_DX, REBOUND_MOVES_X, REBOUND_MOVES_Y, REBOUND_TURNS, REBOUND_MOVES);
}

/// A rebounding trail made (list 2, 0x204 bytes, actor_exec's) from the description `d` (its
/// place, number and heading); its links all at its place's whole dots.
const REBOUND_SIZE: u32 = 0x204;
const REBOUND_BEHAVIOUR: u32 = 0x0609_05dc;
const REBOUND_DRAW: u32 = 0x0609_06e4;

pub fn rebound_set(cpu: &mut Cpu, d: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, REBOUND_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + BEAM_LIST));
    cpu.write32(o + BEHAVIOUR, REBOUND_BEHAVIOUR);
    let number = cpu.read8(d + bullets::S_NUMBER);
    cpu.write8(o + NUMBER, number);
    cpu.write32(o + FLAGS, TRAIL_FLAGS);
    cpu.write32(o + MODEL, REBOUND_DRAW);
    for k in [0, 4] {
        let v = cpu.read32(d + k);
        cpu.write32(o + POSITION + k, v);
    }
    cpu.write32(o + POSITION + 8, 0);
    cpu.write16(o + REBOUND_ENDED, 0);
    cpu.write8(o + REBOUND_PARITY, 0);
    let heading = cpu.read16(d + bullets::S_ANGLE);
    cpu.write16(o + TURN_Z, heading);
    cpu.write8(o + REBOUND_DX, 0);
    cpu.write8(o + REBOUND_DY, 0);
    trail_fill(cpu, o, REBOUND_MOVES_X, REBOUND_MOVES_Y, REBOUND_TURNS, REBOUND_MOVES);
    for i in 0..REBOUND_LINKS {
        let x = cpu.read32(o + POSITION) & 0xffff_0000;
        cpu.write32(o + REBOUND_LINK_X + 4 * i, x);
        let y = cpu.read32(o + POSITION + 4) & 0xffff_0000;
        cpu.write32(o + REBOUND_LINK_Y + 4 * i, y);
    }
}

/// The rebounding trail's sprites' corners (four x, y words each: the head's +0x1d4, the body's
/// +0x1e4, the tail's +0x1f4), the turn its tail is drawn at (the turn kept 84 frames back), its
/// sprite (kind 46, distorted, lift 16) and its frames (cycling every other frame through six from
/// 24 for the head, 30 the tail, 18 the body); once ended above the bottom, a flash where it went
/// off (frames 42 and 43, 12 dots outside it, shaking in size, drawn again bigger).
const REBOUND_HEAD: u32 = 0x1d4;
const REBOUND_BODY: u32 = 0x1e4;
const REBOUND_TAIL: u32 = 0x1f4;
const REBOUND_TAIL_TURN: u32 = REBOUND_TURNS + 84;
const REBOUND_SPRITE_FLAGS: u32 = 0x34;
const REBOUND_FRAMES: [u8; 3] = [24, 30, 18];
const REBOUND_LIFT: u8 = 16;

/// A corner `angle` out from the middle, `reach` / 4 dots (toward zero).
fn rebound_corner(cpu: &mut Cpu, at: u32, angle: i32, reach: u32) {
    let dots = |v: u32| ((v as i32 + if (v as i32) < 0 { 0x3_ffff } else { 0 }) >> 18) as u16;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, angle as u32));
    cpu.write16(at, dots(s.wrapping_mul(reach)));
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, angle as u32));
    cpu.write16(at + 2, dots(c.wrapping_neg().wrapping_mul(reach)));
}

/// A corner reaching back from the head at `from` to the trail's last link, and `angle` out from
/// there, `reach` / 4 dots more.
fn rebound_back_corner(cpu: &mut Cpu, o: u32, at: u32, from: u32, angle: i32, reach: u32) {
    let quarter = |v: u32| ((v as i32 + if (v as i32) < 0 { 3 } else { 0 }) >> 2) as u32;
    let last = REBOUND_LINKS - 1;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, angle as u32));
    let x = cpu.read32(from + SD_X).wrapping_sub(cpu.read32(o + REBOUND_LINK_X + 4 * last) & 0xffff_0000);
    cpu.write16(o + at, (x.wrapping_add(quarter(s.wrapping_mul(reach))) >> 16) as u16);
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, angle as u32));
    let y = cpu.read32(from + SD_Y).wrapping_sub(cpu.read32(o + REBOUND_LINK_Y + 4 * last) & 0xffff_0000);
    cpu.write16(o + at + 2, (y.wrapping_add(quarter(c.wrapping_neg().wrapping_mul(reach))) >> 16) as u16);
}

/// The rebounding trail's draw: unless the game is paused, its links moved on by their moves (link
/// k by the move 11k frames back). A distorted sprite from its head reaching back to its last
/// link, one at its last link, and (until it ends) one at its head; its corners a quarter turn
/// either side of its heading (the tail's of its turn then), 32 (or 48 on the hardest two
/// difficulties) dots out, and more on odd passes of the main loop.
pub fn rebound_draw(cpu: &mut Cpu, s: u32) {
    let o = s.wrapping_sub(FLAGS);
    if cpu.read32(GAME_FLAGS) & HELD == 0 {
        for i in 0..REBOUND_LINKS {
            let back = 11 * i;
            for (at, moves) in [(REBOUND_LINK_X, REBOUND_MOVES_X), (REBOUND_LINK_Y, REBOUND_MOVES_Y)] {
                let m = cpu.read8(o + moves + back) as i8 as i32 as u32;
                let v = cpu.read32(o + at + 4 * i).wrapping_add(m << 16);
                cpu.write32(o + at + 4 * i, v);
            }
        }
    }
    let d = cpu.stack();
    cpu.write32(d + SD_FLAGS, REBOUND_SPRITE_FLAGS);
    cpu.write32(d + SD_Z, 0);
    cpu.write32(d + SD_SCALE_X, 0x1_0000);
    cpu.write32(d + SD_SCALE_Y, 0x1_0000);
    cpu.write16(d + SD_ANGLE, 0);
    cpu.write8(d + SD_LIFT, REBOUND_LIFT);
    let cycle = |cpu: &mut Cpu, from: u8| {
        let now = cpu.read32(GAME_FRAMES) >> 1;
        let f = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, now, 6)) as u8;
        cpu.write8(d + SD_FRAME, f.wrapping_add(from));
    };
    cycle(cpu, REBOUND_FRAMES[0]);
    cpu.write8(d + SD_KIND, TRAIL_KIND);
    let colour = cpu.read16(CHAR_BASE);
    cpu.write16(d + SD_COLOUR_2, colour);
    cpu.write16(d + SD_COLOUR, colour);
    let odd = cpu.read32(LOOPS) & 1;
    let reach = if cpu.read8(DIFFICULTY) as i8 > 1 { odd * 8 + 48 } else { odd * 4 + 32 };
    let place = |cpu: &mut Cpu, link: u32| {
        let x = cpu.read32(o + REBOUND_LINK_X + 4 * link) & 0xffff_0000;
        cpu.write32(d + SD_X, x);
        let y = cpu.read32(o + REBOUND_LINK_Y + 4 * link) & 0xffff_0000;
        cpu.write32(d + SD_Y, y);
    };
    place(cpu, 0);
    let tail = ((cpu.read8(o + REBOUND_TAIL_TURN) as i8 as i32) << 8) & 0xffff;
    let heading = |cpu: &mut Cpu, by: i32| (cpu.read16(o + TURN_Z) as i32).wrapping_add(by) as i16 as i32;
    let a = heading(cpu, 0xe000);
    rebound_corner(cpu, o + REBOUND_HEAD, a, reach);
    let a = heading(cpu, 0x2000);
    rebound_corner(cpu, o + REBOUND_HEAD + 4, a, reach);
    let (left, right) = ((tail + 0x6000) as i16 as i32, (tail + 0xa000) as i16 as i32);
    rebound_back_corner(cpu, o, REBOUND_HEAD + 8, d, left, reach);
    rebound_back_corner(cpu, o, REBOUND_HEAD + 12, d, right, reach);
    cpu.write32(d + SPRITE_STRUCT_CORNERS, o + REBOUND_HEAD);
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));

    place(cpu, REBOUND_LINKS - 1);
    cycle(cpu, REBOUND_FRAMES[1]);
    for (k, by) in [0xe000, 0x2000].into_iter().enumerate() {
        rebound_corner(cpu, o + REBOUND_TAIL + 4 * k as u32, (tail + by) as i16 as i32, reach);
    }
    rebound_corner(cpu, o + REBOUND_TAIL + 8, left, reach);
    rebound_corner(cpu, o + REBOUND_TAIL + 12, right, reach);
    cpu.write32(d + SPRITE_STRUCT_CORNERS, o + REBOUND_TAIL);
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));

    place(cpu, 0);
    if cpu.read16(o + REBOUND_ENDED) == 0 {
        cpu.write32(d + SD_FLAGS, REBOUND_SPRITE_FLAGS);
        cycle(cpu, REBOUND_FRAMES[2]);
        for (k, by) in [0xe000, 0x2000, 0x6000, 0xa000].into_iter().enumerate() {
            let a = heading(cpu, by);
            rebound_corner(cpu, o + REBOUND_BODY + 4 * k as u32, a, reach);
        }
        cpu.write32(d + SPRITE_STRUCT_CORNERS, o + REBOUND_BODY);
        nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
        return;
    }
    if cpu.read32(o + POSITION + 4) as i32 > REBOUND_BOTTOM - 1 {
        return;
    }
    cpu.write32(d + SD_FLAGS, 0);
    let lift = cpu.read8(d + SD_LIFT).wrapping_add(1);
    cpu.write8(d + SD_LIFT, lift);
    let frame = if cpu.read32(GAME_FRAMES) & 1 != 0 { 42 } else { 43 };
    cpu.write8(d + SD_FRAME, frame);
    cpu.write8(d + SD_KIND, TRAIL_KIND);
    let colour = cpu.read16(CHAR_BASE);
    cpu.write16(d + SD_COLOUR, colour);
    for at in [SD_SCALE_X, SD_SCALE_Y] {
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
        cpu.write32(d + at, 0xf000u32.wrapping_sub((r & 7).wrapping_mul(0xc00)));
    }
    let x = cpu.read32(d + SD_X);
    let (x, facing) = if x as i32 > 0 { (x.wrapping_add(0xc_0000), 0xc000) } else { (x.wrapping_sub(0xc_0000), 0x4000) };
    cpu.write32(d + SD_X, x);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1000));
    cpu.write16(d + SD_ANGLE, r.wrapping_add(facing) as u16);
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
    nested!(cpu, 0x0608_fd58, sprite_again(cpu, d, 0, 0x3000));
}

// ---- the darts -----------------------------------------------------------------------------------

/// A dart (an actor_exec sprite thrown out sideways): its step (+0x6c: 0 flying out, 1 turned at
/// the side), its speed (x +0x70, y +0x74), its growth and the growth's change (+0x78, +0x7c), the
/// way it heads once turned (+0x80, a byte: -1 up, 1 down); its size (+0x20, its height 1.25 of
/// it at +0x24).
const DART_STEP: u32 = 0x6c;
const DART_VX: u32 = 0x70;
const DART_VY: u32 = 0x74;
const DART_GROWTH: u32 = 0x78;
const DART_GROWTH_CHANGE: u32 = 0x7c;
const DART_WAY: u32 = 0x80;
const DART_SIDE: u32 = 0x70_0000;
const DART_GROWTHS: u32 = 0x060a_0e3a;
const DART_TURNED_BOXES: u32 = 0x060a_0e32;
const SPRITE_COLOUR: u32 = 0x28;

/// A dart's move: flying out, its frame cycling 44-49; once past a side (112 dots), stopped there,
/// a quarter size, growing (by the difficulty's rate, a 64th less each frame), its boxes the
/// turned dart's, heading toward the nearer player's height. Turned, its frame cycling 50-53 and
/// its colour flashing, it speeds up or down (to 1.5 dots a frame, by 1/8). Its size and place
/// moved on.
pub fn dart_move(cpu: &mut Cpu, o: u32) {
    if cpu.read8(o + DART_STEP) == 0 {
        let now = cpu.read32(GAME_FRAMES);
        let f = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, now, 6)) as u8;
        cpu.write8(o + SPRITE_FRAME, f.wrapping_add(44));
        if cpu.read32(o + POSITION).wrapping_add(DART_SIDE) > 2 * DART_SIDE {
            let step = cpu.read8(o + DART_STEP).wrapping_add(1);
            cpu.write8(o + DART_STEP, step);
            cpu.write32(o + DART_VX, 0);
            cpu.write32(o + SCALES, 0x4000);
            let difficulty = cpu.read8(DIFFICULTY) as i8 as i32 as u32;
            let growth = cpu.read16(DART_GROWTHS.wrapping_add(2 * difficulty)) as u32;
            cpu.write32(o + DART_GROWTH, growth);
            nested!(cpu, 0x0600_8168, clib::ashiftrt_r4::<6>(cpu, growth));
            cpu.write32(o + DART_GROWTH_CHANGE, growth >> 6);
            cpu.write32(o + HIT_BOXES, DART_TURNED_BOXES);
            let x = if cpu.read32(o + POSITION) as i32 > 0 { DART_SIDE } else { DART_SIDE.wrapping_neg() };
            cpu.write32(o + POSITION, x);
            let player = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + FLAGS));
            let below = cpu.read32(o + POSITION + 4) as i32 > cpu.read32(player + POSITION + 4) as i32;
            cpu.write8(o + DART_WAY, if below { 0xff } else { 1 });
        }
    } else {
        let f = (cpu.read8(GAME_FRAMES_LOW) & 3) + 50;
        cpu.write8(o + SPRITE_FRAME, f);
        let colour = if cpu.read32(GAME_FRAMES) & 1 != 0 { 0 } else { 0x10 };
        let colour = cpu.read16(CHAR_BASE).wrapping_add(colour);
        cpu.write16(o + SPRITE_COLOUR, colour);
        let change = cpu.read32(o + DART_GROWTH_CHANGE).wrapping_sub(1);
        cpu.write32(o + DART_GROWTH_CHANGE, change);
        let way = cpu.read8(o + DART_WAY) as i8 as i32 as u32;
        let vy = cpu.read32(o + DART_VY);
        let vy = nested!(cpu, 0x0604_6168, actors::approach(cpu, vy, way.wrapping_mul(0x1_8000), 0x2000));
        cpu.write32(o + DART_VY, vy);
    }
    let growth = cpu.read32(o + DART_GROWTH).wrapping_add(cpu.read32(o + DART_GROWTH_CHANGE));
    cpu.write32(o + DART_GROWTH, growth);
    let size = cpu.read32(o + SCALES).wrapping_add(growth);
    cpu.write32(o + SCALES, size);
    let height = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, size, 0x1_4000));
    cpu.write32(o + SCALES + 4, height);
    for (k, v) in [(0, DART_VX), (4, DART_VY)] {
        let p = cpu.read32(o + POSITION + k).wrapping_add(cpu.read32(o + v));
        cpu.write32(o + POSITION + k, p);
    }
}

/// A dart: ended off screen or once shrunk away; else it moves.
pub fn dart_behaviour(cpu: &mut Cpu, o: u32) {
    let off = nested!(cpu, 0x0605_6a24, aim::off_screen(cpu, o + FLAGS)) as u8 != 0;
    if off || (cpu.read32(o + SCALES) as i32) < 0 {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    nested!(cpu, 0x0609_0f28, dart_move(cpu, o));
}

/// A dart made (list 2, 0x84 bytes, actor_exec's; sound 28) from the description `d` (its place,
/// number, speed at +0x2c and +0x30): unlifted, flipped when thrown right, kind 46 frame 44, its
/// boxes, hurting.
const DART_SIZE: u32 = 0x84;
const DART_BEHAVIOUR: u32 = 0x0609_109c;
const DART_SOUND: u32 = 28;
const DART_FLAGS: u32 = 0x400;
const DART_FLIPPED: u32 = 0x10;
const DART_KIND: u8 = 46;
const DART_FRAME: u8 = 44;
const DART_BOXES: u32 = 0x060a_0e2a;
const D_SPEED: u32 = 0x2c;

pub fn dart_set(cpu: &mut Cpu, d: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, DART_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, DART_SOUND));
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + BEAM_LIST));
    cpu.write32(o + BEHAVIOUR, DART_BEHAVIOUR);
    let number = cpu.read8(d + bullets::S_NUMBER);
    cpu.write8(o + NUMBER, number);
    for k in [0, 4] {
        let v = cpu.read32(d + k);
        cpu.write32(o + POSITION + k, v);
    }
    cpu.write32(o + POSITION + 8, 0);
    cpu.write8(o + BYTE_33, 0);
    for k in [0, 4] {
        let v = cpu.read32(d + D_SPEED + k);
        cpu.write32(o + DART_VX + k, v);
    }
    let flags = if cpu.read32(o + DART_VX) as i32 > 0 { DART_FLAGS | DART_FLIPPED } else { DART_FLAGS };
    cpu.write32(o + FLAGS, flags);
    cpu.write8(o + SPRITE_FRAME, DART_FRAME);
    cpu.write8(o + SPRITE_KIND, DART_KIND);
    let colour = cpu.read16(CHAR_BASE);
    cpu.write16(o + OBJ_COLOUR, colour);
    cpu.write16(o + SPRITE_COLOUR, colour);
    cpu.write32(o + HIT_BOXES, DART_BOXES);
    cpu.write8(o + HIT_SIDE, 1);
    cpu.write8(o + HIT_TYPE, 5);
    cpu.write8(o + DART_STEP, 0);
    cpu.write32(o + DART_GROWTH, 0);
    cpu.write32(o + DART_GROWTH_CHANGE, 0);
}

// ---- the plumes ----------------------------------------------------------------------------------

/// A plume (an actor_exec sprite): its widening's change (+0x6c), its speed down (+0x70); its
/// width and height (+0x20, +0x24).
const PLUME_WIDENING: u32 = 0x6c;
const PLUME_SPEED: u32 = 0x70;

/// A plume: ended once under 1/32 wide. It falls ever faster (by 1/16 a frame), growing taller by
/// a 56th of its speed, wider by its widening (which falls by 1/1024 a frame), throbbing a 16th
/// either way every other 8 frames of its speed.
pub fn plume_behaviour(cpu: &mut Cpu, o: u32) {
    if cpu.read32(o + SCALES) as i32 <= 0x7ff {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    let speed = cpu.read32(o + PLUME_SPEED).wrapping_add(0x1000);
    cpu.write32(o + PLUME_SPEED, speed);
    let y = cpu.read32(o + POSITION + 4).wrapping_add(speed);
    cpu.write32(o + POSITION + 4, y);
    let speed = cpu.read32(o + PLUME_SPEED);
    let h = cpu.read32(o + SCALES + 4).wrapping_add(((speed as i32) / 56) as u32);
    cpu.write32(o + SCALES + 4, h);
    let widening = cpu.read32(o + PLUME_WIDENING).wrapping_sub(0x40);
    cpu.write32(o + PLUME_WIDENING, widening);
    let w = cpu.read32(o + SCALES).wrapping_add(widening);
    cpu.write32(o + SCALES, w);
    let throb = if cpu.read32(o + PLUME_SPEED) & 0x2000 != 0 { 0xffff_f000 } else { 0x1000 };
    let h = cpu.read32(o + SCALES + 4);
    cpu.write32(o + SCALES, w.wrapping_add(throb));
    cpu.write32(o + SCALES + 4, h.wrapping_add(throb));
}

/// A plume's draw (its sprite `s`): itself unculled (colour 0x20 past the first character's on
/// odd passes of the main loop, else 0x30), its flags then put back; a glow over it (frame 54, an
/// eighth bigger, lift 8); once taller than 2, three sparks (frames 61-76 at random, flipped, lift
/// 9, twice its width) 32 to 128 dots below at random.
const PLUME_FLAGS: u32 = 0xc0_0400;
const PLUME_DRAWN_FLAGS: u32 = 0x2000_0020;
const PLUME_GLOW_FLAGS: u32 = 0x2000_0120;
const PLUME_FRAME: u8 = 54;

pub fn plume_draw(cpu: &mut Cpu, s: u32) {
    let o = s.wrapping_sub(FLAGS);
    cpu.write32(o + FLAGS, PLUME_DRAWN_FLAGS);
    let odd = cpu.read32(LOOPS) & 1;
    let colour = cpu.read16(CHAR_BASE).wrapping_add(TRAIL_COLOURS[odd as usize]);
    cpu.write16(o + SPRITE_COLOUR, colour);
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, o + FLAGS));
    cpu.write32(o + FLAGS, PLUME_FLAGS);
    let d = cpu.stack();
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, d));
    cpu.write32(d + SD_FLAGS, PLUME_GLOW_FLAGS);
    for k in [0, 4] {
        let v = cpu.read32(o + POSITION + k);
        cpu.write32(d + SD_X + k, v);
    }
    cpu.write32(d + SD_Z, 1);
    cpu.write8(d + SD_LIFT, 8);
    cpu.write8(d + SD_FRAME, PLUME_FRAME);
    cpu.write8(d + SD_KIND, TRAIL_KIND);
    let colour = cpu.read16(CHAR_BASE).wrapping_add(0x30);
    cpu.write16(d + SD_COLOUR, colour);
    for k in [0, 4] {
        let v = cpu.read32(o + SCALES + k).wrapping_add(0x2000);
        cpu.write32(d + SD_SCALE_X + k, v);
    }
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
    if cpu.read32(o + SCALES + 4) as i32 <= 0x2_0000 {
        return;
    }
    cpu.write32(d + SD_FLAGS, SD_FLIPPED);
    let x = cpu.read32(o + POSITION);
    cpu.write32(d + SD_X, x);
    cpu.write32(d + SD_Z, 1);
    cpu.write8(d + SD_LIFT, 9);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write8(d + SD_FRAME, ((r & 15) + 61) as u8);
    cpu.write8(d + SD_KIND, 0);
    let w = cpu.read32(o + SCALES).wrapping_mul(2);
    cpu.write32(d + SD_SCALE_X, w);
    cpu.write32(d + SD_SCALE_Y, 0x1_0000);
    for _ in 0..3 {
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x60_0000));
        cpu.write32(d + SD_Y, r.wrapping_add(0x20_0000));
        nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
    }
}

/// A plume made (list 2, 0x74 bytes, actor_exec's; sound 83 first) from the description `d` (its
/// place, number): lift 8, kind 46 frame 54, hurting, half wide and an eighth high, widening by
/// 1/32 and falling at 4 dots a frame.
const PLUME_SIZE: u32 = 0x74;
const PLUME_BEHAVIOUR: u32 = 0x0609_1684;
const PLUME_DRAW: u32 = 0x0609_1708;
const PLUME_SOUND: u32 = 83;
const PLUME_BOXES: u32 = 0x060a_0e4c;

pub fn plume_set(cpu: &mut Cpu, d: u32) {
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, PLUME_SOUND));
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, PLUME_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + BEAM_LIST));
    cpu.write32(o + BEHAVIOUR, PLUME_BEHAVIOUR);
    cpu.write32(o + FLAGS, PLUME_FLAGS);
    cpu.write32(o + MODEL, PLUME_DRAW);
    let number = cpu.read8(d + bullets::S_NUMBER);
    cpu.write8(o + NUMBER, number);
    for k in [0, 4] {
        let v = cpu.read32(d + k);
        cpu.write32(o + POSITION + k, v);
    }
    cpu.write32(o + POSITION + 8, 0);
    cpu.write8(o + BYTE_33, 8);
    cpu.write8(o + SPRITE_FRAME, PLUME_FRAME);
    cpu.write8(o + SPRITE_KIND, TRAIL_KIND);
    let colour = cpu.read16(CHAR_BASE).wrapping_add(0x30);
    cpu.write16(o + OBJ_COLOUR, colour);
    cpu.write16(o + SPRITE_COLOUR, colour);
    cpu.write32(o + HIT_BOXES, PLUME_BOXES);
    cpu.write8(o + HIT_SIDE, 1);
    cpu.write8(o + HIT_TYPE, 5);
    cpu.write32(o + SCALES + 4, 0x2000);
    cpu.write32(o + SCALES, 0x8000);
    cpu.write32(o + PLUME_WIDENING, 0x800);
    cpu.write32(o + PLUME_SPEED, 0x4_0000);
}

// ---- the fire boss -------------------------------------------------------------------------------

/// The fire boss's sounds: its shot (30), its spray (26), its missiles (31).
const FB_SHOT_SOUND: u32 = 30;
const FB_SPRAY_SOUND: u32 = 26;
const FB_MISSILE_SOUND: u32 = 31;
/// Its spray's angles (words about its aim, +0x2e): the hardest two difficulties' four, then the
/// easier ones' two.
const FB_SPRAY_ANGLES: u32 = 0x060a_0eec;
const FB_AIM: u32 = 0x2e;
/// Its missiles' homing by difficulty (words).
const FB_MISSILE_HOMING: u32 = 0x060a_0df4;
const MS_HOMING: u32 = 0x2e;
const MS_AIM: u32 = 0x32;
const MS_LAUNCH: u32 = 0x34;
const MS_SPEED: u32 = 0x38;

/// A bullet straight down at 2 dots a frame from 8 dots below it (sound 30).
pub fn fb_shot(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, FB_SHOT_SOUND));
    let d = cpu.stack();
    let number = cpu.read8(o + NUMBER);
    cpu.write8(d + bullets::S_NUMBER, number);
    let x = cpu.read32(o + POSITION);
    cpu.write32(d, x);
    let y = cpu.read32(o + POSITION + 4).wrapping_add(0x8_0000);
    cpu.write32(d + 4, y);
    cpu.write32(d + 8, 0);
    cpu.write16(d + bullets::S_ANGLE, 0x8000);
    cpu.write32(d + SHOT_SPEED, 0x2_0000);
    cpu.write16(d + SHOT_FLAGS, 0);
    nested!(cpu, 0x0605_3db0, bullets::bullet_set(cpu, d));
}

/// Its spray (sound 26): bullets (rockets on the hardest difficulty) at 1.5 dots a frame from 8
/// dots out along its aim, four of them on the hardest two difficulties, else two.
pub fn fb_spray(cpu: &mut Cpu, o: u32) {
    let (n, from) = if cpu.read8(DIFFICULTY) as i8 > 1 { (4, 0) } else { (2, 4) };
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, FB_SPRAY_SOUND));
    let d = cpu.stack();
    for i in 0..n {
        let a = cpu.read16(o + FB_AIM).wrapping_add(cpu.read16(FB_SPRAY_ANGLES + 2 * (i + from)));
        let number = cpu.read8(o + NUMBER);
        cpu.write8(d + bullets::S_NUMBER, number);
        let angle = a as i16 as i32 as u32;
        let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, angle));
        let x = cpu.read32(o + POSITION).wrapping_add(s << 3);
        cpu.write32(d, x);
        let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, angle));
        let y = cpu.read32(o + POSITION + 4).wrapping_sub(c << 3);
        cpu.write32(d + 4, y);
        cpu.write32(d + 8, 0);
        cpu.write16(d + bullets::S_ANGLE, a);
        cpu.write32(d + SHOT_SPEED, 0x1_8000);
        cpu.write16(d + SHOT_FLAGS, 0);
        if cpu.read8(DIFFICULTY) as i8 > 2 {
            nested!(cpu, 0x0605_5d84, bullets::rocket_set_plain(cpu, d));
        } else {
            nested!(cpu, 0x0605_3db0, bullets::bullet_set(cpu, d));
        }
    }
}

/// Its missiles (sound 31): two from 12 dots either side and 8 below, launched back along its
/// heading 0x800 apart, homing by the difficulty, aiming 0x800 a frame, 2 dots a frame.
pub fn fb_missiles(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, FB_MISSILE_SOUND));
    let d = cpu.stack();
    let number = cpu.read8(o + NUMBER);
    cpu.write8(d + bullets::S_NUMBER, number);
    cpu.write32(d + 8, 0);
    cpu.write32(d + MS_SPEED, 0x2_0000);
    let difficulty = cpu.read8(DIFFICULTY) as i8 as i32 as u32;
    let homing = cpu.read16(FB_MISSILE_HOMING.wrapping_add(2 * difficulty));
    cpu.write16(d + MS_HOMING, homing);
    cpu.write16(d + MS_AIM, 0x800);
    let back = cpu.read16(o + TURN_Z).wrapping_add(0x8000);
    cpu.write16(d + bullets::S_ANGLE, back);
    let y = cpu.read32(o + POSITION + 4).wrapping_add(0x8_0000);
    cpu.write32(d + 4, y);
    for (dx, launch) in [(0xfff4_0000u32, 0x800u16), (0xc_0000, 0xf800)] {
        let back = cpu.read16(d + bullets::S_ANGLE);
        cpu.write16(d + MS_LAUNCH, back.wrapping_add(launch));
        let x = cpu.read32(o + POSITION).wrapping_add(dx);
        cpu.write32(d, x);
        nested!(cpu, 0x0605_49f8, bullets::missile_set(cpu, d));
    }
}

/// Its heading's turn and goal (+0xa0, +0xa2), its speed across (+0x8c), its flags (+0x6e, a word:
/// 0x100 just turned back at a side), its parts' hit flags (+0xaa and +0xae the right side's,
/// +0xac and +0xb0 the left's), its two guns' turns (+0x1a4, +0x1a6), their goals (+0x1d8, +0x1da)
/// and their turns' rates and goals (+0x1dc, +0x1de; +0x1e0, +0x1e2).
const FB_TURN_RATE: u32 = 0xa0;
const FB_VX: u32 = 0x8c;
const FB_FLAGS: u32 = 0x6e;
const FB_TURNED_BACK: u16 = 0x100;
const FB_GUN_TURNS: [u32; 2] = [0x1a4, 0x1a6];
const FB_GUN_GOALS: [u32; 2] = [0x1d8, 0x1da];
const FB_GUN_RATES: [u32; 2] = [0x1dc, 0x1e0];

/// Its glide toward the heading `to` (at most 0x800 a frame, a sixth, by 32): across at 6 times
/// the heading's sine (none under a 256th), turned back at the sides (80 dots right and left,
/// 104 once both its parts on that side are off); its guns turned toward their goals (0xa00, a
/// sixth, by 32).
pub fn fb_glide(cpu: &mut Cpu, o: u32, to: u32) {
    let from = cpu.read16(o + TURN_Z) as u32;
    let (rate, goal, to) = (o + FB_TURN_RATE, o + FB_TURN_RATE + 2, to & 0xffff);
    cpu.nested_with_stack(&TURN_TOWARD_PROLOGUE, 0x0605_70d8, &[0x800, 6, 32], |cpu| {
        aim::turn_toward(cpu, rate, goal, from, to, 0x800, 6, 32)
    });
    let heading = cpu.read16(o + TURN_Z) as i16 as i32 as u32;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, heading));
    let vx = s.wrapping_mul(6);
    cpu.write32(o + FB_VX, vx);
    if vx.wrapping_add(0xff) <= 0x1fe {
        cpu.write32(o + FB_VX, 0);
    }
    let both_off = |cpu: &mut Cpu, a: u32, b: u32| {
        cpu.read16(o + a) & PART_OFF != 0 && cpu.read16(o + b) & PART_OFF != 0
    };
    let right = if both_off(cpu, 0xaa, 0xae) { 0x68_0000 } else { 0x50_0000 };
    let left = if both_off(cpu, 0xac, 0xb0) { 0xff98_0000u32 } else { 0xffb0_0000 };
    let flags = cpu.read16(o + FB_FLAGS) & !FB_TURNED_BACK;
    cpu.write16(o + FB_FLAGS, flags);
    let vx = cpu.read32(o + FB_VX) as i32;
    let x = cpu.read32(o + POSITION) as i32;
    if (vx > 0 && x > right) || (vx < 0 && x < left as i32) {
        cpu.write32(o + FB_VX, vx.wrapping_neg() as u32);
        let flags = cpu.read16(o + FB_FLAGS) | FB_TURNED_BACK;
        cpu.write16(o + FB_FLAGS, flags);
    }
    for k in 0..2 {
        let (rate, goal) = (o + FB_GUN_RATES[k], o + FB_GUN_RATES[k] + 2);
        let from = cpu.read16(o + FB_GUN_TURNS[k]) as u32;
        let to = cpu.read16(o + FB_GUN_GOALS[k]) as u32;
        cpu.nested_with_stack(&TURN_TOWARD_PROLOGUE, 0x0605_70d8, &[0xa00, 6, 32], |cpu| {
            aim::turn_toward(cpu, rate, goal, from, to, 0xa00, 6, 32)
        });
    }
    for k in 0..2 {
        let v = cpu.read16(o + FB_GUN_TURNS[k]).wrapping_add(cpu.read16(o + FB_GUN_RATES[k]));
        cpu.write16(o + FB_GUN_TURNS[k], v);
    }
}

/// Its core's attack: its count (+0x84, a word: 100 ends it), whether it is open (+0x7c, a byte),
/// its core's hit flags (+0xa8) and mouth (+0x198), a flash (+0x1e4, a byte: 3 lit); its flags'
/// 0x10 (a pulse of its guns, every 12th count past 60 on the hardest two difficulties).
const FB_CORE_COUNT: u32 = 0x84;
const FB_CORE_OPEN: u32 = 0x7c;
const FB_CORE: u32 = 0xa8;
const FB_MOUTH: u32 = 0x198;
const FB_FLASH: u32 = 0x1e4;
const FB_PULSE: u16 = 0x10;
const FB_SHELL_SPEEDS: u32 = 0x060a_0e00;

/// The core's attack, a count at a time: at 100 it ends (closed). Past 60 its flash lit on odd
/// frames. While its core is on, every 8th count past 20 a glow about its mouth (kind 2, colours
/// 2), and at 60 two waving trails from its mouth (one each way). Once its core is off, every
/// fourth count past 60 a shell (kind 3) thrown down within an eighth of a turn at random.
pub fn fb_core_attack(cpu: &mut Cpu, o: u32) {
    let flags = cpu.read16(o + FB_FLAGS) & !FB_PULSE;
    cpu.write16(o + FB_FLAGS, flags);
    let count = cpu.read16(o + FB_CORE_COUNT) as i16;
    if count == 100 {
        cpu.write8(o + FB_CORE_OPEN, 0);
        cpu.write16(o + FB_CORE_COUNT, 0);
        return;
    }
    if count > 60 && cpu.read32(GAME_FRAMES) & 1 != 0 {
        cpu.write8(o + FB_FLASH, 3);
    }
    let d = cpu.stack();
    let number = cpu.read8(o + NUMBER);
    cpu.write8(d + bullets::S_NUMBER, number);
    if cpu.read8(DIFFICULTY) as i8 > 1 {
        let count = cpu.read16(o + FB_CORE_COUNT) as i16;
        if count > 60 && nested!(cpu, 0x0600_6a6e, math::remainder(cpu, count as i32 as u32, 12)) == 0 {
            let flags = cpu.read16(o + FB_FLAGS) | FB_PULSE;
            cpu.write16(o + FB_FLAGS, flags);
        }
    }
    if cpu.read16(o + FB_CORE) & (PART_OFF | PART_GONE) == 0 {
        let count = cpu.read16(o + FB_CORE_COUNT) as i16;
        if count > 20 && nested!(cpu, 0x0600_6a6e, math::remainder(cpu, count as i32 as u32, 8)) == 0 {
            cpu.write16(d + D_ORBIT_KIND, 2);
            cpu.write16(d + D_COLOURS, 2);
            for k in [0, 4] {
                cpu.write32(d + D_SCALES + k, 0x8000);
                cpu.write32(d + D_SCALE_SPEED + k, 0xffff_f800);
            }
            let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
            cpu.write16(d + D_ANGLE, r as u16);
            cpu.write16(d + D_ANGLE_OFFSET, 0);
            cpu.write16(d + D_TURN, 0x400);
            cpu.write16(d + D_LIFT, 12);
            for k in [0, 4] {
                cpu.write32(d + D_RADIUS + k, 0);
            }
            let (watched, centre, angle_from) = (o + FB_CORE, o + FB_MOUTH, o + TURN_Z);
            cpu.nested_with_stack(&ORBIT_SET_PROLOGUE, ORBIT_SET, &[angle_from], |cpu| {
                orbits::orbit_set(cpu, d, watched, PART_OFF as u32, centre, angle_from)
            });
        }
        if cpu.read16(o + FB_CORE_COUNT) == 60 {
            for k in [0, 4] {
                let v = cpu.read32(o + FB_MOUTH + k);
                cpu.write32(d + k, v);
            }
            cpu.write32(d + 8, 0);
            for (angle, flags) in [(0, 0), (0x8000, 1)] {
                cpu.write16(d + bullets::S_ANGLE, angle);
                cpu.write16(d + SHOT_FLAGS, flags);
                nested!(cpu, 0x0609_0310, trail_set(cpu, d));
            }
        }
    } else {
        let count = cpu.read16(o + FB_CORE_COUNT);
        if count as i16 > 60 && count & 3 == 0 {
            for k in [0, 4] {
                let v = cpu.read32(o + POSITION + k);
                cpu.write32(d + k, v);
            }
            cpu.write32(d + 8, 0);
            let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
            let angle = 0x9000u16.wrapping_sub((r & 0x1fff) as u16);
            cpu.write16(d + bullets::S_ANGLE, angle);
            cpu.write16(d + SHELL_KIND, 3);
            cpu.write32(d + SHELL_SPEED, 0x1_0000);
            cpu.write16(d + SHELL_LAUNCH, angle);
            let difficulty = cpu.read8(DIFFICULTY) as i8 as i32 as u32;
            let speed = cpu.read32(FB_SHELL_SPEEDS.wrapping_add(difficulty << 2));
            cpu.write32(d + SHELL_LAUNCH_SPEED, speed);
            nested!(cpu, 0x0605_587c, bullets::shell_set(cpu, d));
        }
    }
    let count = cpu.read16(o + FB_CORE_COUNT).wrapping_add(1);
    cpu.write16(o + FB_CORE_COUNT, count);
}

/// An orbiter's description at `d` for the fire boss's sparks: kind 2, colours 0, half size wide
/// and up to 3/16 less high at random, shrinking a 32nd a frame, at a random angle, turning up to
/// 0x800 a frame at random, lifted 24, still.
pub fn fb_spark_desc(cpu: &mut Cpu, d: u32) {
    cpu.write16(d + D_ORBIT_KIND, 2);
    cpu.write16(d + D_COLOURS, 0);
    cpu.write32(d + D_SCALES, 0x8000);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write32(d + D_SCALES + 4, 0x8000u32.wrapping_sub((r & 3) << 11));
    for k in [0, 4] {
        cpu.write32(d + D_SCALE_SPEED + k, 0xffff_f800);
    }
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write16(d + D_ANGLE, r as u16);
    cpu.write16(d + D_ANGLE_OFFSET, 0);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x800));
    cpu.write16(d + D_TURN, r as u16);
    cpu.write16(d + D_LIFT, 24);
    for k in [0, 4] {
        cpu.write32(d + D_RADIUS + k, 0);
    }
}

/// Its guns' attack: its count (+0x86, a word: 90 ends it), whether it is on (+0x7d, a byte); its
/// guns (the right's hit flags +0xaa, muzzle +0x1a8, flash +0x1e5; the left's +0xac, +0x1b4,
/// +0x1e6).
const FB_GUN_COUNT: u32 = 0x86;
const FB_GUNS_ON: u32 = 0x7d;
const FB_GUN_PARTS: [u32; 2] = [0xaa, 0xac];
const FB_MUZZLES: [u32; 2] = [0x1a8, 0x1b4];
const FB_GUN_FLASHES: [u32; 2] = [0x1e5, 0x1e6];
const FB_REBOUND_SOUND: u32 = 43;
/// What a gun shot off throws instead (not seen run: left to the SH-2 code).
const FB_STUMP_SHOT: u32 = 0x0609_158c;

/// The guns' attack, a count at a time once both guns are still: at 90 it ends (their goals
/// none). Past 10, on even counts, each gun still on flashes, with a spark about its muzzle every
/// 8th; at 9 each gun still on fires a rebounding trail back along its aim (sound 43), and a gun
/// shot off throws from 8 dots its side of the boss instead.
pub fn fb_gun_attack(cpu: &mut Cpu, o: u32) {
    let count = cpu.read16(o + FB_GUN_COUNT);
    if count as i16 == 90 {
        cpu.write8(o + FB_GUNS_ON, 0);
        cpu.write16(o + FB_GUN_COUNT, 0);
        for at in FB_GUN_GOALS {
            cpu.write16(o + at, 0);
        }
        return;
    }
    if cpu.read16(o + FB_GUN_RATES[1]) == 0 && cpu.read16(o + FB_GUN_RATES[0]) == 0 {
        cpu.write16(o + FB_GUN_COUNT, count.wrapping_add(1));
    }
    let d = cpu.stack();
    let count = cpu.read16(o + FB_GUN_COUNT);
    if count as i16 > 10 && count & 1 == 0 {
        nested!(cpu, 0x0609_245c, fb_spark_desc(cpu, d));
        for k in 0..2 {
            if cpu.read16(o + FB_GUN_PARTS[k]) & (PART_OFF | PART_GONE) != 0 {
                continue;
            }
            cpu.write8(o + FB_GUN_FLASHES[k], 3);
            let count = cpu.read16(o + FB_GUN_COUNT) as i16 as i32 as u32;
            if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, count, 8)) == 0 {
                let (watched, centre, angle_from) = (o + FB_GUN_PARTS[k], o + FB_MUZZLES[k], o + TURN_Z);
                cpu.nested_with_stack(&ORBIT_SET_PROLOGUE, ORBIT_SET, &[angle_from], |cpu| {
                    orbits::orbit_set(cpu, d, watched, PART_OFF as u32, centre, angle_from)
                });
            }
        }
    }
    if cpu.read16(o + FB_GUN_COUNT) != 9 {
        return;
    }
    let number = cpu.read8(o + NUMBER);
    cpu.write8(d + bullets::S_NUMBER, number);
    cpu.write32(d + 8, 0);
    for (k, (side, way)) in [(0xfff8_0000u32, 1u16), (0x8_0000, 0xffff)].into_iter().enumerate() {
        if cpu.read16(o + FB_GUN_PARTS[k]) & (PART_OFF | PART_GONE) == 0 {
            for j in [0, 4] {
                let v = cpu.read32(o + FB_MUZZLES[k] + j);
                cpu.write32(d + j, v);
            }
            let back = cpu.read16(o + FB_GUN_GOALS[k]).wrapping_add(0x8000);
            cpu.write16(d + bullets::S_ANGLE, back);
            nested!(cpu, 0x0609_0e04, rebound_set(cpu, d));
            nested!(cpu, 0x0600_50d4, sound::se_set(cpu, FB_REBOUND_SOUND));
        } else {
            let x = cpu.read32(o + POSITION).wrapping_add(side);
            cpu.write32(d, x);
            let y = cpu.read32(o + POSITION + 4);
            cpu.write32(d + 4, y);
            cpu.write16(d + bullets::S_ANGLE, 0x8000);
            cpu.write16(d + SHOT_FLAGS, 0);
            cpu.write16(d + SHOT_SPEED, way);
            cpu.call(FB_STUMP_SHOT, &[d]);
        }
    }
}

/// Its dart launchers (the right's count +0x88, on +0x7e, hit flags +0xae, flash +0x1e7, muzzle
/// +0x1c0; the left's +0x8a, +0x7f, +0xb0, +0x1e8, +0x1cc), and what a launcher shot off does at
/// its 80th count (not seen run: left to the SH-2 code).
struct FbLauncher {
    count: u32,
    on: u32,
    part: u32,
    flash: u32,
    muzzle: u32,
    way: u32,
}
const FB_LAUNCHERS: [FbLauncher; 2] = [
    FbLauncher { count: 0x88, on: 0x7e, part: 0xae, flash: 0x1e7, muzzle: 0x1c0, way: 1 },
    FbLauncher { count: 0x8a, on: 0x7f, part: 0xb0, flash: 0x1e8, muzzle: 0x1cc, way: 0xffff_ffff },
];
const FB_LAUNCHER_OFF: u32 = 0x0609_1dac;

/// A launcher's attack, a count at a time: at 110 it ends. Shot off, at 80 it does what a shot-off
/// launcher does; else past 30 its flash lit on odd frames, and every 10th count a dart thrown out
/// its side (2 dots a frame, half a dot down).
fn fb_launch(cpu: &mut Cpu, o: u32, l: &FbLauncher) {
    let count = cpu.read16(o + l.count) as i16;
    if count == 110 {
        cpu.write8(o + l.on, 0);
        cpu.write16(o + l.count, 0);
        return;
    }
    if cpu.read16(o + l.part) & (PART_OFF | PART_GONE) != 0 {
        if count == 80 {
            cpu.call(FB_LAUNCHER_OFF, &[o, l.way]);
        }
    } else {
        if count > 30 && cpu.read32(GAME_FRAMES) & 1 != 0 {
            cpu.write8(o + l.flash, 2);
        }
        let count = cpu.read16(o + l.count) as i16;
        if count > 30 && nested!(cpu, 0x0600_6a6e, math::remainder(cpu, count as i32 as u32, 10)) == 0 {
            let d = cpu.stack();
            let number = cpu.read8(o + NUMBER);
            cpu.write8(d + bullets::S_NUMBER, number);
            cpu.write32(d + D_SPEED + 4, 0x8000);
            for k in [0, 4] {
                let v = cpu.read32(o + l.muzzle + k);
                cpu.write32(d + k, v);
            }
            cpu.write32(d + D_SPEED, l.way.wrapping_mul(0x2_0000));
            nested!(cpu, 0x0609_10d8, dart_set(cpu, d));
        }
    }
    let count = cpu.read16(o + l.count).wrapping_add(1);
    cpu.write16(o + l.count, count);
}

/// The right launcher's attack.
pub fn fb_launch_right(cpu: &mut Cpu, o: u32) {
    fb_launch(cpu, o, &FB_LAUNCHERS[0]);
}

/// The left launcher's attack.
pub fn fb_launch_left(cpu: &mut Cpu, o: u32) {
    fb_launch(cpu, o, &FB_LAUNCHERS[1]);
}

/// It rises to 64 dots up (a 16th of a dot a frame).
pub fn fb_rise(cpu: &mut Cpu, o: u32) {
    let y = cpu.read32(o + POSITION + 4);
    let y = nested!(cpu, 0x0604_6168, actors::approach(cpu, y, 0xffc0_0000, 0x1000));
    cpu.write32(o + POSITION + 4, y);
}

/// Its speed while flying in (+0x1f4), its roll's rate (+0x1f2, a word), its step (+0x6c, a byte)
/// and the frames in it (+0x70, a word).
const FB_FLY_SPEED: u32 = 0x1f4;
const FB_ROLL: u32 = 0x1f2;
const FB_STEP: u32 = 0x6c;
const FB_STEP_COUNT: u32 = 0x70;

/// It flies on at its speed along its turns (across and down by its y turn back to front, back by
/// its x turn less a quarter).
pub fn fb_fly(cpu: &mut Cpu, o: u32) {
    let turn = |cpu: &mut Cpu, at: u32, by: u32| (cpu.read16(o + at) as i32 as u32).wrapping_add(by) as i16 as i32 as u32;
    let a = turn(cpu, TURN_Y, 0x8000);
    let speed = cpu.read32(o + FB_FLY_SPEED);
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    let v = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, speed));
    let x = cpu.read32(o + POSITION).wrapping_add(v);
    cpu.write32(o + POSITION, x);
    let a = turn(cpu, TURN_Y, 0x8000);
    let speed = cpu.read32(o + FB_FLY_SPEED);
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    let v = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, c, speed));
    let y = cpu.read32(o + POSITION + 4).wrapping_sub(v);
    cpu.write32(o + POSITION + 4, y);
    let a = turn(cpu, TURN_X, 0xc000);
    let speed = cpu.read32(o + FB_FLY_SPEED);
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    let v = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, speed));
    let z = cpu.read32(o + POSITION + 8).wrapping_sub(v);
    cpu.write32(o + POSITION + 8, z);
}

/// Step 0: past its 64th frame, step 1 (its roll none); it flies at 2 dots a frame, its heading's
/// turn 0x100.
pub fn fb_fly_in(cpu: &mut Cpu, o: u32) {
    if cpu.read16(o + FB_STEP_COUNT) > 64 {
        let step = cpu.read8(o + FB_STEP).wrapping_add(1);
        cpu.write8(o + FB_STEP, step);
        cpu.write16(o + FB_ROLL, 0);
    }
    cpu.write16(o + FB_TURN_RATE, 0x100);
    cpu.write32(o + FB_FLY_SPEED, 0x2_0000);
    nested!(cpu, 0x0609_28cc, fb_fly(cpu, o));
}

/// Step 1: its roll speeding to 96 (by 1), slowing (by 1/128 a frame), pitching up by its roll and
/// turning by five times it (its heading's turn 20 times it more); once pitched past upright, step
/// 2: in the middle at its height, level and still.
pub fn fb_pull_up(cpu: &mut Cpu, o: u32) {
    let roll = cpu.read16(o + FB_ROLL) as i16 as i32 as u32;
    let roll = nested!(cpu, 0x0604_6168, actors::approach(cpu, roll, 96, 1));
    cpu.write16(o + FB_ROLL, roll as u16);
    let speed = cpu.read32(o + FB_FLY_SPEED).wrapping_add(0xffff_fe00);
    cpu.write32(o + FB_FLY_SPEED, speed);
    let roll = cpu.read16(o + FB_ROLL);
    let x = cpu.read16(o + TURN_X).wrapping_add(roll);
    cpu.write16(o + TURN_X, x);
    let roll = cpu.read16(o + FB_ROLL) as i16;
    let y = cpu.read16(o + TURN_Y).wrapping_sub(roll.wrapping_mul(5) as u16);
    cpu.write16(o + TURN_Y, y);
    let roll = cpu.read16(o + FB_ROLL) as i16;
    cpu.write16(o + FB_TURN_RATE, roll.wrapping_mul(20).wrapping_add(0x100) as u16);
    nested!(cpu, 0x0609_28cc, fb_fly(cpu, o));
    if cpu.read16(o + TURN_X) as i16 > 0x4000 {
        let step = cpu.read8(o + FB_STEP).wrapping_add(1);
        cpu.write8(o + FB_STEP, step);
        cpu.write16(o + FB_TURN_RATE + 2, 0);
        cpu.write32(o + POSITION, 0);
        cpu.write32(o + POSITION + 8, 0);
        cpu.write16(o + 0xa4, 0);
        cpu.write16(o + FB_STEP_COUNT, 0);
        cpu.write16(o + TURN_X, 0x4000);
        cpu.write16(o + TURN_Y, 0);
    }
}

/// Its heading turn's limit while settling (+0xa4, a word), the sound once it has settled.
const FB_TURN_LIMIT: u32 = 0xa4;
const FB_SETTLED_SOUND: u32 = 82;

/// Step 2: rising, its heading turned back to straight (by at most its limit, easing to 0x800 by 2,
/// all at once, by 16); past 120 frames step 3 (its heading's turn and goal none), and above the
/// easiest difficulty its wing pieces either side (sound 82).
pub fn fb_settle(cpu: &mut Cpu, o: u32) {
    let from = cpu.read16(o + TURN_Z) as u32;
    let most = cpu.read16(o + FB_TURN_LIMIT) as u32;
    let (rate, goal) = (o + FB_TURN_RATE, o + FB_TURN_RATE + 2);
    cpu.nested_with_stack(&TURN_TOWARD_PROLOGUE, 0x0605_70d8, &[most, 1, 16], |cpu| {
        aim::turn_toward(cpu, rate, goal, from, 0, most, 1, 16)
    });
    let limit = cpu.read16(o + FB_TURN_LIMIT) as i16 as i32 as u32;
    let limit = nested!(cpu, 0x0604_6168, actors::approach(cpu, limit, 0x800, 2));
    cpu.write16(o + FB_TURN_LIMIT, limit as u16);
    if cpu.read16(o + FB_STEP_COUNT) > 120 {
        let step = cpu.read8(o + FB_STEP).wrapping_add(1);
        cpu.write8(o + FB_STEP, step);
        cpu.write16(o + FB_TURN_RATE, 0);
        cpu.write16(o + FB_TURN_RATE + 2, 0);
        if cpu.read8(DIFFICULTY) as i8 > 0 {
            nested!(cpu, 0x0609_4aec, wing_set(cpu, o, 1));
            nested!(cpu, 0x0609_4aec, wing_set(cpu, o, 0xffff_ffff));
            nested!(cpu, 0x0600_50d4, sound::se_set(cpu, FB_SETTLED_SOUND));
        }
    }
    nested!(cpu, 0x0609_28a0, fb_rise(cpu, o));
}

/// Its script (byte pairs from 0x060a0f08: a parameter, an act; its place +0x74, a word; act 13
/// starts it over from the tenth pair, its laps +0x78), the wait between acts by difficulty (bytes
/// at 0x060a0ef8, into +0x76), and what each act's parameter is kept in (the core's +0x80, the
/// guns' +0x81, the launchers' +0x82, +0x83). Its wait for its sweep (+0x7a).
const FB_SCRIPT: u32 = 0x060a_0f08;
const FB_SCRIPT_AT: u32 = 0x74;
const FB_SCRIPT_AGAIN: u16 = 18;
const FB_LAPS: u32 = 0x78;
const FB_WAIT: u32 = 0x76;
const FB_WAITS: u32 = 0x060a_0ef8;
const FB_ON: [u32; 4] = [0x7c, 0x7d, 0x7e, 0x7f];
const FB_PARAMS: [u32; 4] = [0x80, 0x81, 0x82, 0x83];
const FB_SWEEP_WAIT: u32 = 0x7a;
const FB_FLAG_HOLD: u16 = 2;

/// Its next act (step 5, its attacks, its wait by the difficulty): 0 the core, 1 the guns, 4 both
/// launchers, 5 the guns (parameter none) and the core, 6 the guns and both launchers, 7 the guns
/// and the right launcher, 8 the guns and the left launcher (each parameter the frames before that
/// attack starts, but the guns' in 5-8); 9 its sweep (back to step 3, 360 frames), 11 its lunge
/// (step 4, 120 frames), 12 a hold (120 frames).
pub fn fb_next_act(cpu: &mut Cpu, o: u32) {
    let script = FB_SCRIPT + 1;
    let at = cpu.read16(o + FB_SCRIPT_AT) as u32;
    let mut act = cpu.read8(script + at);
    if act == 13 {
        cpu.write16(o + FB_SCRIPT_AT, FB_SCRIPT_AGAIN);
        let laps = cpu.read16(o + FB_LAPS).wrapping_add(1);
        cpu.write16(o + FB_LAPS, laps);
        let at = cpu.read16(o + FB_SCRIPT_AT) as u32;
        act = cpu.read8(script + at);
    }
    cpu.write8(o + FB_STEP, 5);
    let difficulty = cpu.read8(DIFFICULTY) as i8 as i32 as u32;
    let wait = cpu.read8(FB_WAITS.wrapping_add(difficulty)) as u16;
    cpu.write16(o + FB_WAIT, wait);
    let at = cpu.read16(o + FB_SCRIPT_AT);
    let param = cpu.read8(FB_SCRIPT + at as u32);
    cpu.write16(o + FB_SCRIPT_AT, at.wrapping_add(2));
    let start = |cpu: &mut Cpu, k: usize, param: u8| {
        cpu.write8(o + FB_ON[k], 1);
        cpu.write8(o + FB_PARAMS[k], param);
    };
    match act {
        0 => start(cpu, 0, param),
        1 => start(cpu, 1, param),
        4 => {
            start(cpu, 2, param);
            start(cpu, 3, param);
        }
        5 => {
            start(cpu, 1, 0);
            start(cpu, 0, param);
        }
        6 => {
            start(cpu, 1, 0);
            start(cpu, 2, param);
            start(cpu, 3, param);
        }
        7 => {
            start(cpu, 1, 0);
            start(cpu, 2, param);
        }
        8 => {
            start(cpu, 1, 0);
            start(cpu, 3, param);
        }
        9 => {
            cpu.write8(o + FB_STEP, 3);
            cpu.write16(o + FB_SWEEP_WAIT, 360);
        }
        11 => {
            cpu.write8(o + FB_STEP, 4);
            cpu.write16(o + FB_WAIT, 120);
        }
        12 => {
            cpu.write16(o + FB_WAIT, 120);
            let flags = cpu.read16(o + FB_FLAGS) | FB_FLAG_HOLD;
            cpu.write16(o + FB_FLAGS, flags);
        }
        _ => {}
    }
}

/// Its sweep's lead-in by difficulty (bytes at 0x060a0e14: frames, with 180, the sweep follows no
/// one), how far off the player it lets itself be (22 dots), the player's x it follows (within 88
/// dots), its core's hit flags for its sweep's glows (+0xa6) and life (+0xb8); its pitch's rate
/// and goal (+0x98, +0x9a), its drift back (+0x90). A plume of the other kind (the hard
/// difficulties once the core is under 20000; not seen run: left to the SH-2 code).
const FB_SWEEP_LEADS: u32 = 0x060a_0e14;
const FB_SLACK: i32 = 0x16_0000;
const FB_FOLLOW: i32 = 0x58_0000;
const FB_BODY: u32 = 0xa6;
const FB_LIFE: u32 = 0xb8;
const FB_PITCH_RATE: u32 = 0x98;
const FB_DRIFT: u32 = 0x90;
const FB_HOT_PLUME: u32 = 0x0609_1a80;
const FB_RING_SOUND: u32 = 69;

/// Step 3, the fight. Its waits run down (the sweep's every other frame from 139 on). It glides
/// after the nearer player (or holds its place in its sweep's lead-in), turned toward it once 22
/// dots off; within that its next act once its waits are out (and its missiles above the easiest
/// difficulty), its guns aimed at random while on. Past 120 of its sweep, glows streaming from its
/// body (and a second spray of them), its guns swung apart and its pitch raised (0x6000 under
/// 141), and a shot every 10th frame of the back half of every 40 once under 260; at 120 a ring of
/// eight glows (sound 69). It pitches toward its goal, its drift easing off, rising. At 40 its
/// guns straightened; at 100 on an odd frame a plume, a flash, its pitch and drift thrown back and
/// its guns swung out.
pub fn fb_fight(cpu: &mut Cpu, o: u32) {
    let mut pitch_to = 0x4000u32;
    let wait = cpu.read16(o + FB_WAIT);
    if wait != 0 {
        cpu.write16(o + FB_WAIT, wait.wrapping_sub(1));
    }
    let sweep = cpu.read16(o + FB_SWEEP_WAIT);
    if sweep != 0 && (sweep > 0x8b || cpu.read32(GAME_FRAMES) & 1 == 0) {
        cpu.write16(o + FB_SWEEP_WAIT, sweep.wrapping_sub(1));
    }
    let player = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + FLAGS));
    let mut tx = (cpu.read32(player + POSITION) as i32).clamp(-FB_FOLLOW, FB_FOLLOW) as u32;
    let sweep = cpu.read16(o + FB_SWEEP_WAIT);
    if sweep != 0 {
        let difficulty = cpu.read8(DIFFICULTY) as i8 as i32 as u32;
        let lead = cpu.read8(FB_SWEEP_LEADS.wrapping_add(difficulty)) as u16 + 180;
        if sweep < lead {
            tx = cpu.read32(o + POSITION);
        }
    }
    let dx = cpu.read32(o + POSITION).wrapping_sub(tx) as i32;
    let turned_back = |cpu: &mut Cpu| cpu.read16(o + FB_FLAGS) & FB_TURNED_BACK != 0;
    let to = if dx > FB_SLACK && !turned_back(cpu) {
        0xf000
    } else if dx < -FB_SLACK && !turned_back(cpu) {
        0x1000
    } else {
        if cpu.read16(o + FB_WAIT) == 0 && cpu.read16(o + FB_SWEEP_WAIT) == 0 {
            nested!(cpu, 0x0609_2b3c, fb_next_act(cpu, o));
            if cpu.read8(DIFFICULTY) as i8 > 0 {
                nested!(cpu, 0x0609_2010, fb_missiles(cpu, o));
            }
        }
        if cpu.read8(o + FB_GUNS_ON) != 0 {
            let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
            let a = ((r & 0x1700) + 0x1a00) as u16;
            cpu.write16(o + FB_GUN_GOALS[0], a.wrapping_neg());
            if cpu.read8(DIFFICULTY) as i8 > 2 {
                let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
                cpu.write16(o + FB_GUN_GOALS[1], ((r & 0x1700) + 0x1a00) as u16);
            } else {
                cpu.write16(o + FB_GUN_GOALS[1], a);
            }
            for at in FB_GUN_RATES {
                cpu.write16(o + at, 0);
                cpu.write16(o + at + 2, 0);
            }
        }
        0
    };
    nested!(cpu, 0x0609_20d0, fb_glide(cpu, o, to));
    let d = cpu.stack();
    let orbit = |cpu: &mut Cpu, angle_from: u32| {
        let (watched, centre) = (o + FB_BODY, o + POSITION);
        cpu.nested_with_stack(&ORBIT_SET_PROLOGUE, ORBIT_SET, &[angle_from], |cpu| {
            orbits::orbit_set(cpu, d, watched, PART_OFF as u32, centre, angle_from)
        });
    };
    let sweep = cpu.read16(o + FB_SWEEP_WAIT) as u32;
    if sweep > 120 {
        let left = 0x168u32.wrapping_sub(sweep);
        cpu.write16(d + D_ORBIT_KIND, 0);
        cpu.write16(d + D_COLOURS, 2);
        cpu.write32(d + D_SCALES, 0x4000);
        cpu.write32(d + D_SCALES + 4, (left << 9).wrapping_add(0x2_0000));
        cpu.write32(d + D_SCALE_SPEED, 0);
        cpu.write32(d + D_SCALE_SPEED + 4, 0xffff_f000);
        cpu.write16(d + D_ANGLE, 0);
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
        cpu.write16(d + D_ANGLE_OFFSET, (r & 0x7fff) as u16);
        cpu.write16(d + D_TURN, 0);
        cpu.write16(d + D_LIFT, 0);
        cpu.write32(d + D_RADIUS, 0x8_0000);
        cpu.write32(d + D_RADIUS + 4, 0xffff_f000);
        orbit(cpu, o + TURN_X);
        let sweep = cpu.read16(o + FB_SWEEP_WAIT) as u32;
        let rem = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, sweep, 1));
        if rem == 0 {
            cpu.write16(d + D_ORBIT_KIND, 1);
            cpu.write16(d + D_COLOURS, 2);
            cpu.write32(d + D_SCALES, 0x1_0000);
            let left = 0x168u32.wrapping_sub(cpu.read16(o + FB_SWEEP_WAIT) as u32);
            cpu.write32(d + D_SCALES + 4, left.wrapping_mul(0xc0));
            cpu.write32(d + D_SCALE_SPEED, 0xffff_f800);
            let left = 0x168u32.wrapping_sub(cpu.read16(o + FB_SWEEP_WAIT) as u32);
            cpu.write32(d + D_SCALE_SPEED + 4, left.wrapping_mul(3));
            cpu.write16(d + D_ANGLE, 0);
            let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
            cpu.write16(d + D_ANGLE_OFFSET, ((r & 0x3fff) + 0x2000) as u16);
            let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
            cpu.write16(d + D_TURN, 0x100u32.wrapping_sub(r & 0x3ff) as u16);
            cpu.write16(d + D_LIFT, 0);
            cpu.write32(d + D_RADIUS, rem);
            cpu.write32(d + D_RADIUS + 4, 0x2_0000);
            orbit(cpu, o + TURN_X);
        }
        let sweep = cpu.read16(o + FB_SWEEP_WAIT) as u32;
        if sweep > 0x8c {
            let a = 0x2000u32.wrapping_sub((0x154u32.wrapping_sub(sweep)).wrapping_mul(96)) as u16;
            cpu.write16(o + FB_GUN_GOALS[0], a);
            cpu.write16(o + FB_GUN_GOALS[1], a.wrapping_neg());
        } else {
            pitch_to = 0x6000;
            cpu.write16(o + FB_GUN_GOALS[0], 0x2000);
            cpu.write16(o + FB_GUN_GOALS[1], 0xe000);
        }
        if cpu.read16(o + FB_SWEEP_WAIT) <= 0x103 {
            let n = cpu.read16(o + FB_STEP_COUNT) as u32;
            if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, n, 40)) as i32 > 19 {
                let n = cpu.read16(o + FB_STEP_COUNT) as u32;
                if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, n, 10)) == 0 {
                    nested!(cpu, 0x0609_1eb4, fb_shot(cpu, o));
                }
            }
        }
    }
    if cpu.read16(o + FB_SWEEP_WAIT) == 120 {
        cpu.write16(d + D_ORBIT_KIND, 1);
        cpu.write16(d + D_COLOURS, 2);
        cpu.write32(d + D_SCALES, 0x8000);
        cpu.write32(d + D_SCALES + 4, 0x4000);
        cpu.write32(d + D_SCALE_SPEED, 0);
        cpu.write32(d + D_SCALE_SPEED + 4, 0x3000);
        cpu.write16(d + D_ANGLE, 0);
        cpu.write16(d + D_TURN, 0x80);
        cpu.write16(d + D_LIFT, 0);
        cpu.write32(d + D_RADIUS, 0);
        cpu.write32(d + D_RADIUS + 4, 0);
        for i in 0..8u16 {
            cpu.write16(d + D_ANGLE_OFFSET, i << 13);
            orbit(cpu, o + TURN_Z);
        }
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, FB_RING_SOUND));
    }
    let from = cpu.read16(o + TURN_X) as u32;
    let (rate, goal) = (o + FB_PITCH_RATE, o + FB_PITCH_RATE + 2);
    cpu.nested_with_stack(&TURN_TOWARD_PROLOGUE, 0x0605_70d8, &[0x800, 12, 4], |cpu| {
        aim::turn_toward(cpu, rate, goal, from, pitch_to, 0x800, 12, 4)
    });
    let drift = cpu.read32(o + FB_DRIFT);
    let drift = nested!(cpu, 0x0604_6168, actors::approach(cpu, drift, 0, 0x800));
    cpu.write32(o + FB_DRIFT, drift);
    nested!(cpu, 0x0609_28a0, fb_rise(cpu, o));
    if cpu.read16(o + FB_SWEEP_WAIT) == 40 {
        for at in FB_GUN_GOALS {
            cpu.write16(o + at, 0);
        }
    }
    if cpu.read16(o + FB_SWEEP_WAIT) == 100 && cpu.read32(GAME_FRAMES) & 1 != 0 {
        for k in [0, 4, 8] {
            let v = cpu.read32(o + POSITION + k);
            cpu.write32(d + k, v);
        }
        let number = cpu.read8(o + NUMBER);
        cpu.write8(d + bullets::S_NUMBER, number);
        if cpu.read8(DIFFICULTY) as i8 > 1 && cpu.read32(o + FB_LIFE) as i32 <= 19999 {
            cpu.call(FB_HOT_PLUME, &[d]);
        } else {
            nested!(cpu, 0x0609_1834, plume_set(cpu, d));
        }
        cpu.write16(d + actors::E_COLOUR, 48);
        cpu.write32(d + actors::E_SCALES + 4, 0x2_0000);
        cpu.write16(d + actors::E_ANGLE, 0x8000);
        nested!(cpu, 0x0605_7e38, explosions::cross_flash(cpu, d));
        cpu.write16(o + FB_PITCH_RATE, 0xf200);
        cpu.write16(o + FB_PITCH_RATE + 2, 0xff40);
        cpu.write32(o + FB_DRIFT, 0xffff_0000);
        cpu.write16(o + FB_GUN_GOALS[0], 0xb000);
        cpu.write16(o + FB_GUN_GOALS[1], 0x5000);
    }
}

/// Its flags' 0x1 (its lunge's strike), its pitch's goal while lunging, its fall once downed
/// (+0x94), its count once downed (+0x72, a word).
const FB_STRIKE: u16 = 1;
const FB_FALL: u32 = 0x94;
const FB_DOWN_COUNT: u32 = 0x72;

/// Step 4, its lunge: its wait out, back to step 3 (its guns straightened); holding its place. At
/// 45 its guns swung out, at 40 its strike; above 50 pitched back (0x2800), its guns swung in and
/// rising 3/16 of a dot a frame; at 50 its pitch and drift thrown forward. It pitches toward its
/// goal (0x800, a 12th, by 8), its drift easing off, rising.
pub fn fb_lunge(cpu: &mut Cpu, o: u32) {
    let wait = cpu.read16(o + FB_WAIT);
    if wait != 0 {
        cpu.write16(o + FB_WAIT, wait.wrapping_sub(1));
    } else {
        cpu.write8(o + FB_STEP, 3);
        for at in FB_GUN_GOALS {
            cpu.write16(o + at, 0);
        }
    }
    nested!(cpu, 0x0609_20d0, fb_glide(cpu, o, 0));
    if cpu.read16(o + FB_WAIT) == 45 {
        cpu.write16(o + FB_GUN_GOALS[0], 0xc000);
        cpu.write16(o + FB_GUN_GOALS[1], 0x4000);
    }
    if cpu.read16(o + FB_WAIT) == 40 {
        let flags = cpu.read16(o + FB_FLAGS) | FB_STRIKE;
        cpu.write16(o + FB_FLAGS, flags);
    }
    let pitch_to = if cpu.read16(o + FB_WAIT) as i16 > 50 {
        cpu.write16(o + FB_GUN_GOALS[0], 0x1000);
        cpu.write16(o + FB_GUN_GOALS[1], 0xf000);
        let y = cpu.read32(o + POSITION + 4).wrapping_add(0xffff_d000);
        cpu.write32(o + POSITION + 4, y);
        0x2800
    } else {
        0x4000
    };
    if cpu.read16(o + FB_WAIT) == 50 {
        cpu.write16(o + FB_PITCH_RATE, 0x1000);
        cpu.write16(o + FB_PITCH_RATE + 2, 0x1c0);
        cpu.write32(o + FB_DRIFT, 0x1_8000);
    }
    let from = cpu.read16(o + TURN_X) as u32;
    let (rate, goal) = (o + FB_PITCH_RATE, o + FB_PITCH_RATE + 2);
    cpu.nested_with_stack(&TURN_TOWARD_PROLOGUE, 0x0605_70d8, &[0x800, 12, 8], |cpu| {
        aim::turn_toward(cpu, rate, goal, from, pitch_to, 0x800, 12, 8)
    });
    let drift = cpu.read32(o + FB_DRIFT);
    let drift = nested!(cpu, 0x0604_6168, actors::approach(cpu, drift, 0, 0x800));
    cpu.write32(o + FB_DRIFT, drift);
    let y = cpu.read32(o + POSITION + 4);
    let y = nested!(cpu, 0x0604_6168, actors::approach(cpu, y, 0xffc0_0000, 0x1000));
    cpu.write32(o + POSITION + 4, y);
}

/// Step 5, its attacks: holding its place; once nearly still, each attack that is on (the core's,
/// the guns', the right launcher's and the left's) runs once its parameter's frames are out; when
/// none is on, back to step 3.
pub fn fb_attack(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0609_20d0, fb_glide(cpu, o, 0));
    if (cpu.read32(o + FB_VX) as i32).unsigned_abs() > 0x7ff {
        return;
    }
    for k in 0..4 {
        if cpu.read8(o + FB_ON[k]) == 0 {
            continue;
        }
        let param = cpu.read8(o + FB_PARAMS[k]);
        if param != 0 {
            cpu.write8(o + FB_PARAMS[k], param.wrapping_sub(1));
            continue;
        }
        match k {
            0 => nested!(cpu, 0x0609_2254, fb_core_attack(cpu, o)),
            1 => nested!(cpu, 0x0609_24ec, fb_gun_attack(cpu, o)),
            2 => nested!(cpu, 0x0609_2700, fb_launch_right(cpu, o)),
            _ => nested!(cpu, 0x0609_27d0, fb_launch_left(cpu, o)),
        }
    }
    if cpu.read32(o + FB_ON[0]) == 0 {
        cpu.write8(o + FB_STEP, 3);
    }
}

/// Step 6, downed: levelling its y turn (0x4000, an eighth, by 32), its pitch's rate easing to
/// -0x600 and its heading's to none (by 8), turned back at the sides (88 dots), its speed across
/// and drift easing off (by 3/16); an explosion at its 60th frame; falling back ever faster (to 4
/// dots a frame, by 3/512); ended past its 210th.
pub fn fb_downed(cpu: &mut Cpu, o: u32) {
    let from = cpu.read16(o + TURN_Y) as u32;
    let (rate, goal) = (o + 0x9c, o + 0x9e);
    cpu.nested_with_stack(&TURN_TOWARD_PROLOGUE, 0x0605_70d8, &[0x4000, 8, 32], |cpu| {
        aim::turn_toward(cpu, rate, goal, from, 0, 0x4000, 8, 32)
    });
    for (at, to) in [(FB_PITCH_RATE, 0xffff_fa00), (FB_TURN_RATE, 0)] {
        let v = cpu.read16(o + at) as i16 as i32 as u32;
        let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, to, 8));
        cpu.write16(o + at, v as u16);
    }
    let vx = cpu.read32(o + FB_VX) as i32;
    let x = cpu.read32(o + POSITION) as i32;
    if (vx > 0 && x > FB_FOLLOW) || (vx < 0 && x < -FB_FOLLOW) {
        let vx = cpu.read32(o + FB_VX).wrapping_neg();
        cpu.write32(o + FB_VX, vx);
    }
    for at in [FB_VX, FB_DRIFT] {
        let v = cpu.read32(o + at);
        let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0, 0x3000));
        cpu.write32(o + at, v);
    }
    let n = cpu.read16(o + FB_DOWN_COUNT).wrapping_add(1);
    cpu.write16(o + FB_DOWN_COUNT, n);
    if n == 60 {
        let p = cpu.stack();
        for k in [0, 4, 8] {
            let v = cpu.read32(o + POSITION + k);
            cpu.write32(p + k, v);
        }
        cpu.write32(p + 0x3c, o);
        nested!(cpu, 0x0605_8ba4, explosions::explosion_set(cpu, p));
    }
    let z = cpu.read32(o + POSITION + 8).wrapping_add(cpu.read32(o + FB_FALL));
    cpu.write32(o + POSITION + 8, z);
    let fall = cpu.read32(o + FB_FALL);
    let fall = nested!(cpu, 0x0604_6168, actors::approach(cpu, fall, 0x4_0000, 0x180));
    cpu.write32(o + FB_FALL, fall);
    if cpu.read16(o + FB_DOWN_COUNT) as i16 > 210 {
        cpu.write8(o + OBJ_MODE, TO_STOP);
    }
}

// ---- the fire boss's gondola and wing pieces --------------------------------------------------------

/// The gondola (its own object hanging under the fire boss, +0x6c the boss): its count (+0x70, a
/// word), step (+0x72, a byte), lean (+0x73, a byte: -1, 0, 1), speed across and down (+0x74,
/// +0x78), its turns' rates and goals (x: +0x80, +0x82; y: +0x84; z: +0x88, +0x8a). Its place on
/// the boss (+0x18c, 176 dots nearer), the boss's speed across (+0x8c).
const GONDOLA_OF: u32 = 0x6c;
const GONDOLA_COUNT: u32 = 0x70;
const GONDOLA_STEP: u32 = 0x72;
const GONDOLA_LEAN: u32 = 0x73;
const GONDOLA_VX: u32 = 0x74;
const GONDOLA_VY: u32 = 0x78;
const GONDOLA_TURN_X: u32 = 0x80;
const GONDOLA_TURN_Y: u32 = 0x84;
const GONDOLA_TURN_Z: u32 = 0x88;
const FB_GONDOLA_PLACE: u32 = 0x18c;
const GONDOLA_BELOW: u32 = 0xb0_0000;

/// The gondola leans (its z turn toward its lean's 0x6000: at most 0x1000, half the way, by 64).
pub fn gondola_lean(cpu: &mut Cpu, o: u32) {
    let lean = cpu.read8(o + GONDOLA_LEAN) as i8 as i32;
    let from = cpu.read16(o + TURN_Z);
    let to = (from as i16 as i32).wrapping_add(lean.wrapping_mul(0x6000)) as u32 & 0xffff;
    let (rate, goal) = (o + GONDOLA_TURN_Z, o + GONDOLA_TURN_Z + 2);
    cpu.nested_with_stack(&TURN_TOWARD_PROLOGUE, 0x0605_70d8, &[0x1000, 2, 64], |cpu| {
        aim::turn_toward(cpu, rate, goal, from as u32, to, 0x1000, 2, 64)
    });
}

/// Step 0: carried at its place on the boss, turned as it is, leaning with it (once it goes more
/// than a 16th of a dot a frame either way); at the boss's strike, step 1 (dropping at 3 dots a
/// frame, level).
pub fn gondola_ride(cpu: &mut Cpu, o: u32) {
    let boss = cpu.read32(o + GONDOLA_OF);
    for k in [0, 4] {
        let v = cpu.read32(boss + FB_GONDOLA_PLACE + k);
        cpu.write32(o + POSITION + k, v);
    }
    let z = cpu.read32(boss + FB_GONDOLA_PLACE + 8).wrapping_sub(GONDOLA_BELOW);
    cpu.write32(o + POSITION + 8, z);
    for turn in [TURN_X, TURN_Y] {
        let v = cpu.read16(boss + turn);
        cpu.write16(o + turn, v);
    }
    if cpu.read32(boss + FB_VX) as i32 > 0x1000 {
        cpu.write8(o + GONDOLA_LEAN, 1);
    }
    if (cpu.read32(boss + FB_VX) as i32) < -0x1000 {
        cpu.write8(o + GONDOLA_LEAN, 0xff);
    }
    nested!(cpu, 0x0609_35ec, gondola_lean(cpu, o));
    let flags = cpu.read16(boss + FB_FLAGS);
    if flags & FB_STRIKE != 0 {
        cpu.write16(boss + FB_FLAGS, flags & !FB_STRIKE);
        cpu.write8(o + GONDOLA_STEP, 1);
        cpu.write32(o + POSITION + 8, 0);
        cpu.write32(o + GONDOLA_VX, 0);
        cpu.write32(o + GONDOLA_VY, 0x3_0000);
        cpu.write16(o + GONDOLA_TURN_X, 0);
        cpu.write16(o + GONDOLA_TURN_X + 2, 0);
    }
}

/// Step 1: dropping, slowing by 3/64 a frame; stopped, step 2 (its count from 0).
pub fn gondola_drop(cpu: &mut Cpu, o: u32) {
    let vy = cpu.read32(o + GONDOLA_VY).wrapping_add(0xffff_f440);
    cpu.write32(o + GONDOLA_VY, vy);
    if (vy as i32) < 0 {
        cpu.write8(o + GONDOLA_STEP, 2);
        cpu.write32(o + GONDOLA_VY, 0);
        cpu.write16(o + GONDOLA_COUNT, 0);
    }
    nested!(cpu, 0x0609_35ec, gondola_lean(cpu, o));
}

/// Step 2: hunting. Above the easiest difficulty, from its 121st frame to its 359th, every 16th
/// frame in the back half of every 64 its spray. It drifts toward the nearer player (a dot a frame,
/// by 1/64, once 16 dots off; leaning that way), turned face down; at the boss's hold, step 3.
pub fn gondola_hunt(cpu: &mut Cpu, o: u32) {
    let boss = cpu.read32(o + GONDOLA_OF);
    if cpu.read8(DIFFICULTY) != 0 {
        let count = cpu.read16(o + GONDOLA_COUNT);
        if count.wrapping_sub(121) <= 0xee
            && nested!(cpu, 0x0600_6a6e, math::remainder(cpu, count as u32, 16)) == 0
        {
            let count = cpu.read16(o + GONDOLA_COUNT) as u32;
            if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, count, 64)) as i32 > 31 {
                nested!(cpu, 0x0609_1f1c, fb_spray(cpu, o));
            }
        }
    }
    let player = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + FLAGS));
    let dx = cpu.read32(o + POSITION).wrapping_sub(cpu.read32(player + POSITION)) as i32;
    let to = if dx > 0x10_0000 {
        cpu.write8(o + GONDOLA_LEAN, 1);
        0xffff_0000
    } else if dx < -0x10_0000 {
        cpu.write8(o + GONDOLA_LEAN, 0xff);
        0x1_0000
    } else {
        0
    };
    let vx = cpu.read32(o + GONDOLA_VX);
    let vx = nested!(cpu, 0x0604_6168, actors::approach(cpu, vx, to, 0x400));
    cpu.write32(o + GONDOLA_VX, vx);
    let flags = cpu.read16(boss + FB_FLAGS);
    if flags & FB_FLAG_HOLD != 0 {
        cpu.write16(boss + FB_FLAGS, flags & !FB_FLAG_HOLD);
        cpu.write8(o + GONDOLA_STEP, 3);
    }
    nested!(cpu, 0x0609_35ec, gondola_lean(cpu, o));
    gondola_pitch(cpu, o, 0xc000);
}

/// Its x turn toward `to` (at most 0x1000, a quarter, by 32).
fn gondola_pitch(cpu: &mut Cpu, o: u32, to: u32) {
    let from = cpu.read16(o + TURN_X) as u32;
    let (rate, goal) = (o + GONDOLA_TURN_X, o + GONDOLA_TURN_X + 2);
    cpu.nested_with_stack(&TURN_TOWARD_PROLOGUE, 0x0605_70d8, &[0x1000, 4, 32], |cpu| {
        aim::turn_toward(cpu, rate, goal, from, to, 0x1000, 4, 32)
    });
}

/// Step 3: back up to the boss (3/4 of a dot a frame) and across under it (3/4, by 1/16, once 4
/// dots off; leaning that way); there, step 0. Turned upright.
pub fn gondola_return(cpu: &mut Cpu, o: u32) {
    let boss = cpu.read32(o + GONDOLA_OF);
    let y = cpu.read32(o + POSITION + 4);
    let goal = cpu.read32(boss + FB_GONDOLA_PLACE + 4);
    let y = nested!(cpu, 0x0604_6168, actors::approach(cpu, y, goal, 0xc000));
    let x = cpu.read32(o + POSITION);
    cpu.write32(o + POSITION + 4, y);
    let dx = x.wrapping_sub(cpu.read32(boss + POSITION)) as i32;
    let to = if dx > 0x4_0000 {
        cpu.write8(o + GONDOLA_LEAN, 1);
        0xffff_4000
    } else if dx < -0x4_0000 {
        cpu.write8(o + GONDOLA_LEAN, 0xff);
        0xc000
    } else {
        0
    };
    let vx = cpu.read32(o + GONDOLA_VX);
    let vx = nested!(cpu, 0x0604_6168, actors::approach(cpu, vx, to, 0x1000));
    cpu.write32(o + GONDOLA_VX, vx);
    if cpu.read32(o + POSITION + 4) == cpu.read32(boss + FB_GONDOLA_PLACE + 4)
        && (dx as u32).wrapping_add(0x3_ffff) <= 0x7_fffe
    {
        cpu.write8(o + GONDOLA_STEP, 0);
    }
    nested!(cpu, 0x0609_35ec, gondola_lean(cpu, o));
    gondola_pitch(cpu, o, 0x4000);
}

/// Hidden while more than 8 dots nearer or further.
fn hidden_off_depth(cpu: &mut Cpu, o: u32) {
    let flags = cpu.read32(o + FLAGS);
    let off = cpu.read32(o + POSITION + 8).wrapping_add(0x7_ffff) > 0xf_ffff;
    cpu.write32(o + FLAGS, if off { flags | TOUCH_OFF } else { flags & !TOUCH_OFF });
}

/// The gondola: ended once the boss is downed. Its step, then moved (turned back at the sides, 88
/// dots), turned; its count; hidden away from the plane.
pub fn gondola_behaviour(cpu: &mut Cpu, o: u32) {
    let boss = cpu.read32(o + GONDOLA_OF);
    if cpu.read8(boss + FB_STEP) > 5 {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    match cpu.read8(o + GONDOLA_STEP) {
        0 => nested!(cpu, 0x0609_3638, gondola_ride(cpu, o)),
        1 => nested!(cpu, 0x0609_3704, gondola_drop(cpu, o)),
        2 => nested!(cpu, 0x0609_3744, gondola_hunt(cpu, o)),
        3 => nested!(cpu, 0x0609_3868, gondola_return(cpu, o)),
        step => unreachable!("gondola step {step}"),
    }
    for (k, v) in [(0, GONDOLA_VX), (4, GONDOLA_VY)] {
        let p = cpu.read32(o + POSITION + k).wrapping_add(cpu.read32(o + v));
        cpu.write32(o + POSITION + k, p);
    }
    let vx = cpu.read32(o + GONDOLA_VX) as i32;
    let x = cpu.read32(o + POSITION) as i32;
    if (vx > 0 && x > FB_FOLLOW) || (vx < 0 && x < -FB_FOLLOW) {
        let vx = cpu.read32(o + GONDOLA_VX).wrapping_neg();
        cpu.write32(o + GONDOLA_VX, vx);
    }
    for (turn, rate) in [(TURN_X, GONDOLA_TURN_X), (TURN_Y, GONDOLA_TURN_Y), (TURN_Z, GONDOLA_TURN_Z)] {
        let v = cpu.read16(o + turn).wrapping_add(cpu.read16(o + rate));
        cpu.write16(o + turn, v);
    }
    let n = cpu.read16(o + GONDOLA_COUNT).wrapping_add(1);
    cpu.write16(o + GONDOLA_COUNT, n);
    hidden_off_depth(cpu, o);
}

/// A wing piece (its own object, +0x6c the boss): its side (+0x70, a byte: 1 right, -1 left), the
/// boss's last 12 places (whole dots, words: x from +0x72, y from +0x8a), the oldest at +0x88 and
/// +0xa0 its goal. Its rockets' sound.
const WING_SIDE: u32 = 0x70;
const WING_TRAIL_X: u32 = 0x72;
const WING_TRAIL_Y: u32 = 0x8a;
const WING_TRAIL: u32 = 12;
const WING_SOUND: u32 = 25;

/// A wing piece: ended once the boss is downed; hidden away from the plane. Spinning its side's
/// way; trailing the boss (50 dots its side, 12 frames behind, at 3 dots a frame; within 112 dots
/// of the middle). Its aim on the nearer player within a quarter turn of its side's (straight
/// down beyond), turned toward it 3/16 at a time (all at once within 127), kept within a quarter
/// of its side's; at the boss's pulse a rocket along it (sound 25).
pub fn wing_behaviour(cpu: &mut Cpu, o: u32) {
    let boss = cpu.read32(o + GONDOLA_OF);
    if cpu.read8(boss + FB_STEP) > 5 {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    hidden_off_depth(cpu, o);
    let side = cpu.read8(o + WING_SIDE) as i8 as i32;
    let spin = cpu.read16(o + TURN_Z).wrapping_add((side << 10) as u16);
    cpu.write16(o + TURN_Z, spin);
    let side = cpu.read8(o + WING_SIDE) as i8 as i32 as u32;
    let x = cpu.read32(boss + POSITION).wrapping_add(side.wrapping_mul(0x32_0000));
    nested!(cpu, 0x0605_62fc, actors::shift_in_words(cpu, o + WING_TRAIL_X, whole(x) as i32 as u32, WING_TRAIL));
    let y = cpu.read32(boss + POSITION + 4);
    nested!(cpu, 0x0605_62fc, actors::shift_in_words(cpu, o + WING_TRAIL_Y, whole(y) as i32 as u32, WING_TRAIL));
    for (k, trail) in [(0, WING_TRAIL_X), (4, WING_TRAIL_Y)] {
        let goal = (cpu.read16(o + trail + 2 * (WING_TRAIL - 1)) as u32) << 16;
        let v = cpu.read32(o + POSITION + k);
        let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, goal, 0x3_0000));
        cpu.write32(o + POSITION + k, v);
    }
    if cpu.read32(o + POSITION) as i32 > DART_SIDE as i32 {
        cpu.write32(o + POSITION, DART_SIDE);
    }
    if (cpu.read32(o + POSITION) as i32) < -(DART_SIDE as i32) {
        cpu.write32(o + POSITION, DART_SIDE.wrapping_neg());
    }
    let side = cpu.read8(o + WING_SIDE) as i8 as i32 as u32;
    let facing = ((((side << 8) >> 2) << 8) & 0xffff) as u16;
    let player = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + FLAGS));
    let mut aim_at = nested!(cpu, 0x0604_5e30, aim::angle_to(cpu, o, player)) & 0xffff;
    let off = nested!(cpu, 0x0605_6580, aim::angle_between(cpu, aim_at, facing as u32)) as i16;
    if off.wrapping_add(0x4100) as u16 > 0x8200 {
        aim_at = 0x8000;
    }
    let turn = cpu.read16(o + TURN_Y);
    let d = nested!(cpu, 0x0605_6580, aim::angle_between(cpu, aim_at, turn as u32)) as i16;
    if d.unsigned_abs() > 127 {
        let turn = cpu.read16(o + TURN_Y).wrapping_add(if d > 0 { 0xff40 } else { 0xc0 });
        cpu.write16(o + TURN_Y, turn);
    } else {
        cpu.write16(o + TURN_Y, aim_at as u16);
    }
    let turn = cpu.read16(o + TURN_Y);
    let off = turn.wrapping_sub(facing) as i16 as i32;
    if off > 0x4000 {
        cpu.write16(o + TURN_Y, (turn as i32 + 0x4000 - off) as u16);
    }
    if off < -0x4000 {
        let turn = cpu.read16(o + TURN_Y) as i32;
        cpu.write16(o + TURN_Y, (turn - 0x4000 - off) as u16);
    }
    if cpu.read16(boss + FB_FLAGS) & FB_PULSE != 0 {
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, WING_SOUND));
        let d = cpu.stack();
        let number = cpu.read8(o + NUMBER);
        cpu.write8(d + bullets::S_NUMBER, number);
        let aim = cpu.read16(o + TURN_Y);
        cpu.write16(d + bullets::S_ANGLE, aim);
        for k in [0, 4, 8] {
            let v = cpu.read32(o + POSITION + k);
            cpu.write32(d + k, v);
        }
        cpu.write16(d + SHOT_FLAGS, 13);
        cpu.write16(d + SHOT_SPEED, 1);
        nested!(cpu, 0x0605_5b5c, bullets::rocket_set(cpu, d));
    }
}

/// The fire boss's parts: each one's knocked-off step (a byte, 0 on), life, hit flags and smoke's
/// points (12 bytes each) with the last step that smokes: the core, the right and left guns, the
/// right and left launchers. Its body's hit flags are +0xa6 (its life +0xb8 on the gauge).
struct FbPart {
    step: u32,
    life: u32,
    flags: u32,
    smoke: u32,
    last: i32,
}
const FB_PARTS: [FbPart; 5] = [
    FbPart { step: 0xb3, life: 0xd4, flags: 0xa8, smoke: 0xd8, last: 5 },
    FbPart { step: 0xb4, life: 0x120, flags: 0xaa, smoke: 0x124, last: 1 },
    FbPart { step: 0xb5, life: 0x13c, flags: 0xac, smoke: 0x140, last: 1 },
    FbPart { step: 0xb6, life: 0x158, flags: 0xae, smoke: 0x15c, last: 0 },
    FbPart { step: 0xb7, life: 0x168, flags: 0xb0, smoke: 0x16c, last: 0 },
];
const FB_DOWN_SCORE: u32 = 30_000;
const FB_DOWN_RATE: u32 = 50;
const FB_LAST_LAP: i16 = 3;
/// Its turn's high byte over its last 8 frames (+0x1e9).
const FB_TURNS: u32 = 0x1e9;

/// The fire boss's behaviour: its life on the boss gauge. Before step 6, downed once its life is
/// out or its script has run through four times: the bullets cleared, its body off, the scenery's
/// scroll moved on, the stage's score (30000 at rate 50 if shot down, else none) and 10 more to
/// the rate for each part already off, every life out. Each part knocked off as its life runs out.
/// Its step; moved and turned; its parts' smoke along their points; its turns kept; its count.
pub fn fb_behaviour(cpu: &mut Cpu, o: u32) {
    let life = cpu.read32(o + FB_LIFE);
    nested!(cpu, 0x0604_64b8, hud::boss_gauge_life(cpu, life));
    let downed = cpu.read8(o + FB_STEP) <= 5
        && (cpu.read32(o + FB_LIFE) as i32 <= 0 || cpu.read16(o + FB_LAPS) as i16 > FB_LAST_LAP);
    if downed {
        cpu.write8(o + FB_STEP, 6);
        nested!(cpu, 0x0605_61e0, screen::bullets_cleared(cpu));
        let flags = cpu.read16(o + FB_BODY) | PART_OFF;
        cpu.write16(o + FB_BODY, flags);
        cpu.write16(o + FB_DOWN_COUNT, 0);
        nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
        cpu.write32(STAGE_RATE_AT, 0);
        if cpu.read32(o + FB_LIFE) as i32 <= 0 {
            cpu.write32(STAGE_SCORE_AT, FB_DOWN_SCORE);
            cpu.write32(STAGE_RATE_AT, FB_DOWN_RATE);
        } else {
            cpu.write32(STAGE_SCORE_AT, 0);
        }
        for part in &FB_PARTS {
            if cpu.read32(o + part.life) as i32 <= 0 {
                let rate = cpu.read32(STAGE_RATE_AT).wrapping_add(10);
                cpu.write32(STAGE_RATE_AT, rate);
            }
        }
        cpu.write32(o + FB_LIFE, 0);
        for part in &FB_PARTS {
            cpu.write32(o + part.life, 0);
        }
    }
    for part in &FB_PARTS {
        let step = cpu.read8(o + part.step);
        if step == 0 && cpu.read32(o + part.life) as i32 <= 0 {
            cpu.write8(o + part.step, step.wrapping_add(1));
            let flags = cpu.read16(o + part.flags) | PART_OFF;
            cpu.write16(o + part.flags, flags);
        }
    }
    match cpu.read8(o + FB_STEP) {
        0 => nested!(cpu, 0x0609_2970, fb_fly_in(cpu, o)),
        1 => nested!(cpu, 0x0609_29c0, fb_pull_up(cpu, o)),
        2 => nested!(cpu, 0x0609_2a78, fb_settle(cpu, o)),
        3 => nested!(cpu, 0x0609_2ca0, fb_fight(cpu, o)),
        4 => nested!(cpu, 0x0609_3298, fb_lunge(cpu, o)),
        5 => nested!(cpu, 0x0609_33bc, fb_attack(cpu, o)),
        6 => nested!(cpu, 0x0609_34b8, fb_downed(cpu, o)),
        step => unreachable!("fire boss step {step}"),
    }
    for (k, v) in [(0, FB_VX), (4, FB_DRIFT)] {
        let p = cpu.read32(o + POSITION + k).wrapping_add(cpu.read32(o + v));
        cpu.write32(o + POSITION + k, p);
    }
    for (turn, rate) in [(TURN_X, FB_PITCH_RATE), (TURN_Y, 0x9c), (TURN_Z, FB_TURN_RATE)] {
        let v = cpu.read16(o + turn).wrapping_add(cpu.read16(o + rate));
        cpu.write16(o + turn, v);
    }
    for part in &FB_PARTS {
        let step = cpu.read8(o + part.step) as i32;
        if step == 0 || step - 1 > part.last {
            continue;
        }
        let p = o + part.smoke + 12 * (step - 1) as u32;
        if nested!(cpu, 0x0605_968c, explosions::smoke_and_sparks(cpu, p, 12)) as u8 != 0 {
            let step = cpu.read8(o + part.step).wrapping_add(1);
            cpu.write8(o + part.step, step);
        }
    }
    let turn = cpu.read16(o + TURN_Z) as i16 as i32;
    let turn = turn + if turn < 0 { 0xff } else { 0 };
    nested!(cpu, 0x0600_8164, clib::ashiftrt_r4::<8>(cpu, turn as u32));
    let high = ((turn >> 8) as i8) as i32 as u32;
    nested!(cpu, 0x0605_62b4, actors::shift_in_bytes(cpu, o + FB_TURNS, high, 8));
    let n = cpu.read16(o + FB_STEP_COUNT).wrapping_add(1);
    cpu.write16(o + FB_STEP_COUNT, n);
}

/// The fire boss's model set (0x060744c0: its +0x1c its models), its parts' flashes (bytes: the
/// core's +0x1e4 pushing it out, the guns' +0x1e5 and +0x1e6 lowering them, the launchers' +0x1e7
/// and +0x1e8 pulling them in), its turn of 7 frames back (+0x1f0), its points' tables (whole-dot
/// x, y, z, 8 bytes each, as parts_set's) and its glow's place (+0x178).
const FB_MODEL_SET: u32 = 0x0607_44c0;
const FB_OLD_TURN: u32 = FB_TURNS + 7;
const FB_GLOW_AT: u32 = 0x178;
const FB_HIDDEN: [u32; 6] = [FB_BODY, FB_CORE, 0xaa, 0xac, 0xae, 0xb0];

/// The model at the fire boss's models' `at` (the flashing one of a pair by its flags `f`).
fn fb_model(cpu: &mut Cpu, at: u32, f: u16) -> u32 {
    let set = cpu.read32(FB_MODEL_SET);
    let models = cpu.read32(set + SHIP_MODELS);
    let list = cpu.read32(models + at);
    cpu.read32(list + 4 * (f & PART_FLASH) as u32)
}

/// Points from `table` (whole dots, 8 bytes each) moved by `by`, brought into the world at `out`.
fn fb_points(cpu: &mut Cpu, table: u32, n: u32, by: [u32; 3], out: u32) {
    for i in 0..n {
        let [x, y, z] = [0, 2, 4].map(|k| (cpu.read16(table + 8 * i + k) as u32) << 16);
        cpu.call(SL_CALC_POINT, &[x.wrapping_add(by[0]), y.wrapping_add(by[1]), z.wrapping_add(by[2]), out + 12 * i]);
    }
}

/// The fire boss's draw (its sprite `s`): its parts' flashes moved on. At its place, turned (x, y,
/// z): its gondola's place 48 dots on, its glow's 24 (32 on odd passes) back, its body's points, its
/// body. The core (pushed out by its flash; once fighting, turned by its turn of 7 frames back
/// against its own), its mouth's place, its points. Each gun (lowered by its flash, turned by its
/// own turn; its shot-off model once off) with its muzzle and points; each launcher (pulled in by
/// its flash, turned as the core) with its muzzle and points. Its two fins, kept level. Then,
/// unless its body is gone, a glow at its glow's place (frames 0 and 1 flipped by the main loop's
/// pass), throbbing at random; its flashes cleared.
pub fn fb_draw(cpu: &mut Cpu, s: u32) {
    let o = s.wrapping_sub(FLAGS);
    let lean = (cpu.read16(o + TURN_Z) as i16 as i32).wrapping_neg() as u32 & 0xff00;
    for at in FB_HIDDEN {
        flash_step(cpu, o + at);
    }
    let angle = |a: u16| a as i16 as i32 as u32;
    let push = |cpu: &mut Cpu| {
        cpu.call(SL_PUSH_MATRIX, &[]);
    };
    let pop = |cpu: &mut Cpu| {
        cpu.call(SL_POP_MATRIX, &[]);
    };
    let old_turn = |cpu: &mut Cpu| angle(((cpu.read8(o + FB_OLD_TURN) as u32) << 8).wrapping_add(lean) as u16);
    let flash = |cpu: &mut Cpu, at: u32| (cpu.read8(o + at) as u32) << 16;
    push(cpu);
    let p = [0, 4, 8].map(|k| cpu.read32(o + POSITION + k));
    cpu.call(SL_TRANSLATE, &p);
    for (turn, rot) in [(TURN_X, SL_ROT_X), (TURN_Y, SL_ROT_Y), (TURN_Z, SL_ROT_Z)] {
        let a = cpu.read16(o + turn);
        cpu.call(rot, &[angle(a)]);
    }
    cpu.call(SL_CALC_POINT, &[0, 0, 0xffd0_0000, o + FB_GONDOLA_PLACE]);
    let back = ((cpu.read32(LOOPS) & 1) * 8 + 24) << 16;
    cpu.call(SL_CALC_POINT, &[0, 0, back, o + FB_GLOW_AT]);
    fb_points(cpu, 0x060a_0d84, 2, [0, 0, 0], o + 0xbc);
    let f = cpu.read16(o + FB_BODY);
    if f & PART_GONE == 0 {
        let model = fb_model(cpu, 0x24, f);
        cpu.call(SL_PUT_POLYGON, &[model]);
    }

    push(cpu);
    let out = 0xfff6_0000u32.wrapping_sub(flash(cpu, FB_FLASH));
    cpu.call(SL_TRANSLATE, &[0, 0x2_0000, out]);
    if cpu.read8(o + FB_STEP) > 2 {
        let a = old_turn(cpu);
        cpu.call(SL_ROT_Y, &[a]);
    }
    let a = old_turn(cpu);
    cpu.call(SL_ROT_Z, &[a]);
    let f = cpu.read16(o + FB_CORE);
    if f & (PART_OFF | PART_GONE) == 0 {
        let model = fb_model(cpu, 0x20, f);
        cpu.call(SL_PUT_POLYGON, &[model]);
        cpu.call(SL_CALC_POINT, &[0, 0, 0xffe0_0000, o + FB_MOUTH]);
    }
    fb_points(cpu, 0x060a_0d94, 6, [0, 0, 0xa_0000], o + 0xd8);
    pop(cpu);

    for (k, x, models, table, by, out) in [
        (0, 0x19_0000u32, (0x10, 0x14), 0x060a_0dc4, 0xffe8_0000u32, 0x124),
        (1, 0xffeb_0000, (0x08, 0x0c), 0x060a_0dd4, 0x18_0000, 0x140),
    ] {
        push(cpu);
        let y = flash(cpu, FB_GUN_FLASHES[k]).wrapping_add(0x3_0000);
        cpu.call(SL_TRANSLATE, &[x, y, 0xfffa_0000]);
        for rot in [SL_ROT_Y, SL_ROT_Z] {
            let a = cpu.read16(o + FB_GUN_TURNS[k]);
            cpu.call(rot, &[angle(a)]);
        }
        let f = cpu.read16(o + FB_GUN_PARTS[k]);
        if f & PART_GONE == 0 {
            let model = if f & PART_OFF == 0 { fb_model(cpu, models.0, f) } else { fb_model(cpu, models.1, 0) };
            cpu.call(SL_PUT_POLYGON, &[model]);
            cpu.call(SL_CALC_POINT, &[0, 0, 0xffe0_0000, o + FB_MUZZLES[k]]);
        }
        fb_points(cpu, table, 2, [by, 0, 0x6_0000], o + out);
        pop(cpu);
    }

    for (k, model, muzzle, table, by, out) in [
        (0, 0x1c, 0x20_0000u32, 0x060a_0de4, 0xfff4_0000u32, 0x15c),
        (1, 0x18, 0xffe0_0000, 0x060a_0dec, 0xc_0000, 0x16c),
    ] {
        let l = &FB_LAUNCHERS[k];
        push(cpu);
        let x = if k == 0 {
            0xc_0000u32.wrapping_sub(flash(cpu, l.flash))
        } else {
            flash(cpu, l.flash).wrapping_add(0xfff4_0000)
        };
        cpu.call(SL_TRANSLATE, &[x, 0, 0xd_0000]);
        let a = old_turn(cpu);
        cpu.call(SL_ROT_Z, &[a]);
        if cpu.read8(o + FB_STEP) > 2 {
            let a = old_turn(cpu);
            cpu.call(SL_ROT_Y, &[a]);
        }
        let f = cpu.read16(o + l.part);
        if f & (PART_OFF | PART_GONE) == 0 {
            let m = fb_model(cpu, model, f);
            cpu.call(SL_PUT_POLYGON, &[m]);
            cpu.call(SL_CALC_POINT, &[muzzle, 0, 0, o + l.muzzle]);
        }
        fb_points(cpu, table, 1, [by, 0, 0xfff0_0000], o + out);
        pop(cpu);
    }

    push(cpu);
    cpu.call(SL_TRANSLATE, &[0xc_0000, 0xc_0000, 0]);
    let a = cpu.read16(o + TURN_Z).wrapping_neg();
    cpu.call(SL_ROT_Z, &[angle(a)]);
    let f = cpu.read16(o + FB_BODY);
    if f & PART_GONE == 0 {
        let model = fb_model(cpu, 0x04, f);
        cpu.call(SL_PUT_POLYGON, &[model]);
    }
    pop(cpu);
    push(cpu);
    let a = cpu.read16(o + TURN_Z).wrapping_neg();
    cpu.call(SL_ROT_X, &[angle(a)]);
    cpu.call(SL_TRANSLATE, &[0xfff4_0000, 0xc_0000, 0]);
    let f = cpu.read16(o + FB_BODY);
    if f & PART_GONE == 0 {
        let model = fb_model(cpu, 0x00, f);
        cpu.call(SL_PUT_POLYGON, &[model]);
    }
    pop(cpu);
    pop(cpu);

    if cpu.read16(o + FB_BODY) & PART_GONE == 0 {
        let d = cpu.stack();
        nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, d));
        for k in [0, 4] {
            let v = cpu.read32(o + FB_GLOW_AT + k);
            cpu.write32(d + SD_X + k, v);
        }
        let z = cpu.read32(o + FB_GLOW_AT + 8).wrapping_add(0xff50_0000);
        cpu.write32(d + SD_Z, z);
        cpu.write8(d + SD_LIFT, 0);
        let size = if cpu.read32(LOOPS) & 1 == 0 {
            let colour = cpu.read16(CHAR_BASE).wrapping_add(0x30);
            cpu.write16(d + SD_COLOUR, colour);
            cpu.write8(d + SD_FRAME, 0);
            0x5000
        } else {
            let flags = cpu.read32(d + SD_FLAGS) | SD_FLIPPED;
            cpu.write32(d + SD_FLAGS, flags);
            let colour = cpu.read16(CHAR_BASE).wrapping_add(0x70);
            cpu.write16(d + SD_COLOUR, colour);
            cpu.write8(d + SD_FRAME, 1);
            0x7000
        };
        cpu.write8(d + SD_KIND, 0);
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
        let size = (((r & 15) * 3) << 8).wrapping_add(size);
        cpu.write32(d + SD_SCALE_X, size);
        cpu.write32(d + SD_SCALE_Y, size);
        nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
    }
    for at in [FB_FLASH, FB_GUN_FLASHES[0], FB_GUN_FLASHES[1], FB_LAUNCHERS[0].flash, FB_LAUNCHERS[1].flash] {
        cpu.write8(o + at, 0);
    }
}

/// The gondola's draw: at its place, turned (x, y, z), its body; four fins about it, 4 dots back and 8
/// out each way (turned to face out).
pub fn gondola_draw(cpu: &mut Cpu, s: u32) {
    let o = s.wrapping_sub(FLAGS);
    cpu.call(SL_PUSH_MATRIX, &[]);
    let p = [0, 4, 8].map(|k| cpu.read32(o + POSITION + k));
    cpu.call(SL_TRANSLATE, &p);
    for (turn, rot) in [(TURN_X, SL_ROT_X), (TURN_Y, SL_ROT_Y), (TURN_Z, SL_ROT_Z)] {
        let a = cpu.read16(o + turn) as i16 as i32 as u32;
        cpu.call(rot, &[a]);
    }
    let model = fb_model(cpu, 0x2c, 0);
    cpu.call(SL_PUT_POLYGON, &[model]);
    for (at, turn) in [
        ([0, 0x8_0000, 0xfffc_0000], Some(0xffff_c000)),
        ([0xfff8_0000, 0, 0xfffc_0000], None),
        ([0, 0xfff8_0000, 0xfffc_0000], Some(0x4000)),
        ([0x8_0000, 0, 0xfffc_0000], Some(0xffff_8000)),
    ] {
        cpu.call(SL_PUSH_MATRIX, &[]);
        cpu.call(SL_TRANSLATE, &at);
        if let Some(turn) = turn {
            cpu.call(SL_ROT_Z, &[turn]);
        }
        let model = fb_model(cpu, 0x30, 0);
        cpu.call(SL_PUT_POLYGON, &[model]);
        cpu.call(SL_POP_MATRIX, &[]);
    }
    cpu.call(SL_POP_MATRIX, &[]);
}

/// The gondola's and wing pieces' size, boxes, life.
const GONDOLA_SIZE: u32 = 0x8c;
const GONDOLA_BEHAVIOUR: u32 = 0x0609_394c;
const GONDOLA_DRAW: u32 = 0x0609_47fc;
const GONDOLA_FLAGS: u32 = 0xcc_0000;
const GONDOLA_BOXES: u32 = 0x060a_0f5c;
const GONDOLA_LIFE: u32 = 10_000;
const HIT_LIFE: u32 = 0x50;
const WING_SIZE: u32 = 0xa4;
const WING_BEHAVIOUR: u32 = 0x0609_3a40;
const WING_DRAW: u32 = 0x0609_4a60;
const WING_BOXES: u32 = 0x060a_0f6c;

/// The fire boss's gondola made (numbered: list 2, 0x8c bytes, numbered_exec's; an enemy's number) for
/// the boss `boss`: 72 dots up in the middle, face down, leaning right, 10000 life.
pub fn gondola_set(cpu: &mut Cpu, boss: u32) {
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, ENEMY_NUMBERS)) as i8;
    if n == -1 {
        return;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, GONDOLA_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, NUMBERED_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + BEAM_LIST));
    cpu.write16(o + GONDOLA_COUNT, 0);
    cpu.write32(o + GONDOLA_OF, boss);
    cpu.write32(o + BEHAVIOUR, GONDOLA_BEHAVIOUR);
    cpu.write32(o + FLAGS, GONDOLA_FLAGS);
    cpu.write32(o + MODEL, GONDOLA_DRAW);
    cpu.write8(o + NUMBER, n as u8);
    let number = cpu.read8(o + NUMBER) as i8 as i32;
    cpu.write32(PLAYERS.wrapping_add((4 * number) as u32), o);
    cpu.write32(o + HIT_BOXES, GONDOLA_BOXES);
    cpu.write32(o + HIT_LIFE, GONDOLA_LIFE);
    cpu.write8(o + HIT_SIDE, 1);
    cpu.write8(o + HIT_TYPE, 4);
    cpu.write8(o + GONDOLA_STEP, 0);
    for (k, v) in [(0, 0), (4, 0xffb8_0000), (8, 0)] {
        cpu.write32(o + POSITION + k, v);
    }
    for at in [GONDOLA_VX, GONDOLA_VY, 0x7c] {
        cpu.write32(o + at, 0);
    }
    cpu.write16(o + TURN_X, 0x4000);
    for at in [GONDOLA_TURN_X, GONDOLA_TURN_Y, GONDOLA_TURN_Z] {
        cpu.write16(o + at, 0);
        cpu.write16(o + at + 2, 0);
    }
    cpu.write8(o + GONDOLA_LEAN, 1);
}

/// A wing piece's draw: at its place, turned (x, y back to front, z), its model.
pub fn wing_draw(cpu: &mut Cpu, s: u32) {
    let o = s.wrapping_sub(FLAGS);
    cpu.call(SL_PUSH_MATRIX, &[]);
    let p = [0, 4, 8].map(|k| cpu.read32(o + POSITION + k));
    cpu.call(SL_TRANSLATE, &p);
    let a = cpu.read16(o + TURN_X) as i16 as i32 as u32;
    cpu.call(SL_ROT_X, &[a]);
    let a = cpu.read16(o + TURN_Y).wrapping_add(0x8000) as i16 as i32 as u32;
    cpu.call(SL_ROT_Y, &[a]);
    let a = cpu.read16(o + TURN_Z) as i16 as i32 as u32;
    cpu.call(SL_ROT_Z, &[a]);
    let model = fb_model(cpu, 0x28, 0);
    cpu.call(SL_PUT_POLYGON, &[model]);
    cpu.call(SL_POP_MATRIX, &[]);
}

/// A wing piece made (numbered: list 2, 0xa4 bytes) for the boss `boss` on its side `side` (1 the
/// right, -1 the left): 72 dots up in the middle, aimed its side's way, 10000 life, its trail of
/// the boss's places filled with where it is now.
pub fn wing_set(cpu: &mut Cpu, boss: u32, side: u32) {
    let side = side as i8;
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, ENEMY_NUMBERS)) as i8;
    if n == -1 {
        return;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, WING_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, NUMBERED_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + BEAM_LIST));
    cpu.write32(o + GONDOLA_OF, boss);
    cpu.write32(o + BEHAVIOUR, WING_BEHAVIOUR);
    let flags = cpu.read32(o + FLAGS) | GONDOLA_FLAGS;
    cpu.write32(o + FLAGS, flags);
    cpu.write32(o + MODEL, WING_DRAW);
    cpu.write8(o + NUMBER, n as u8);
    let number = cpu.read8(o + NUMBER) as i8 as i32;
    cpu.write32(PLAYERS.wrapping_add((4 * number) as u32), o);
    cpu.write32(o + HIT_BOXES, WING_BOXES);
    cpu.write32(o + HIT_LIFE, GONDOLA_LIFE);
    cpu.write8(o + HIT_SIDE, 1);
    cpu.write8(o + HIT_TYPE, 4);
    for (k, v) in [(0, 0), (4, 0xffb8_0000), (8, 0)] {
        cpu.write32(o + POSITION + k, v);
    }
    cpu.write8(o + WING_SIDE, side as u8);
    cpu.write16(o + TURN_X, 0x4000);
    let side = cpu.read8(o + WING_SIDE) as i8 as i32 as u32;
    cpu.write16(o + TURN_Y, (((side << 8) >> 2) << 8) as u16);
    let side = cpu.read8(o + WING_SIDE) as i8 as i32 as u32;
    let x = cpu.read32(boss + POSITION).wrapping_add(side.wrapping_mul(0x32_0000));
    nested!(cpu, 0x0605_62dc, actors::fill_words(cpu, o + WING_TRAIL_X, whole(x) as i32 as u32, WING_TRAIL));
    let y = cpu.read32(boss + POSITION + 4);
    nested!(cpu, 0x0605_62dc, actors::fill_words(cpu, o + WING_TRAIL_Y, whole(y) as i32 as u32, WING_TRAIL));
}

/// The fire boss's set-up: 86 dots left, 128 down and 192 back, pitched, turned and rolled to fly
/// in; its script's start by difficulty (bytes at 0x060a0f00), its first wait 30; its body 40000
/// (on the gauge) with a depth box over its glow, its core 15000, its guns 25000 and launchers
/// 18000 each, its gondola; sound 81.
const FB_BEHAVIOUR: u32 = 0x0609_3c58;
const FB_DRAW: u32 = 0x0609_4030;
const FB_SET_FLAGS: u32 = 0xc8_0000;
const FB_SCRIPT_STARTS: u32 = 0x060a_0f00;
const FB_START_LIFE: u32 = 40_000;
const FB_SOUND: u32 = 81;
const DEPTH_BOX_WATCHED: u32 = 0x6c;
const DEPTH_BOX_AT: u32 = 0x74;
/// Its parts' tables and lives, in the order they are set up (the body's first).
const FB_PART_SETS: [(u32, u32, u32, u32, u32, Option<u32>); 5] = [
    (0x060a_0d94, 6, 0xd8, 0xa8, 0xd4, Some(15_000)),
    (0x060a_0dc4, 2, 0x124, 0xaa, 0x120, Some(25_000)),
    (0x060a_0dd4, 2, 0x140, 0xac, 0x13c, Some(25_000)),
    (0x060a_0de4, 1, 0x15c, 0xae, 0x158, Some(18_000)),
    (0x060a_0dec, 1, 0x16c, 0xb0, 0x168, Some(18_000)),
];

pub fn fb_set(cpu: &mut Cpu, o: u32, n: u32) {
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + BEAM_LIST));
    cpu.write32(o + BEHAVIOUR, FB_BEHAVIOUR);
    cpu.write32(o + FLAGS, FB_SET_FLAGS);
    cpu.write32(o + MODEL, FB_DRAW);
    cpu.write8(o + NUMBER, n as u8);
    let number = cpu.read8(o + NUMBER) as i8 as i32;
    cpu.write32(PLAYERS.wrapping_add((4 * number) as u32), o);
    cpu.write8(o + FB_STEP, 0);
    for (k, v) in [(0, 0xffaa_0000), (4, 0x80_0000), (8, 0xff40_0000)] {
        cpu.write32(o + POSITION + k, v);
    }
    cpu.write16(o + TURN_X, 0x2000);
    cpu.write16(o + TURN_Z, 0xc000);
    cpu.write16(o + TURN_Y, 0xa000);
    for at in [FB_VX, FB_DRIFT, FB_FALL] {
        cpu.write32(o + at, 0);
    }
    for at in (0x98..=0xa4).step_by(2) {
        cpu.write16(o + at, 0);
    }
    for at in [FB_STEP_COUNT, FB_DOWN_COUNT, FB_FLAGS] {
        cpu.write16(o + at, 0);
    }
    let difficulty = cpu.read8(DIFFICULTY) as i8 as i32 as u32;
    let start = cpu.read8(FB_SCRIPT_STARTS.wrapping_add(difficulty));
    cpu.write16(o + FB_SCRIPT_AT, start as u16);
    cpu.write16(o + FB_WAIT, 30);
    cpu.write16(o + FB_LAPS, 0);
    for k in 0..2 {
        cpu.write16(o + FB_GUN_TURNS[k], 0);
        cpu.write16(o + FB_GUN_GOALS[k], 0);
        cpu.write16(o + FB_GUN_RATES[k], 0);
        cpu.write16(o + FB_GUN_RATES[k] + 2, 0);
    }
    cpu.write16(o + FB_SWEEP_WAIT, 0);
    for at in FB_ON.into_iter().chain(FB_PARAMS) {
        cpu.write8(o + at, 0);
    }
    for at in [FB_CORE_COUNT, FB_GUN_COUNT, FB_LAUNCHERS[0].count, FB_LAUNCHERS[1].count] {
        cpu.write16(o + at, 0);
    }
    for at in FB_FLASH..=FB_LAUNCHERS[1].flash {
        cpu.write8(o + at, 0);
    }
    cpu.write16(o + FB_ROLL, 0);
    cpu.write32(o + FB_FLY_SPEED, 0);
    nested!(cpu, 0x0605_6294, actors::fill_bytes(cpu, o + FB_TURNS, 0, 8));
    for k in [0, 4, 8] {
        cpu.write32(o + FB_GONDOLA_PLACE + k, 0);
    }
    cpu.write16(o + FB_BODY, 0);
    cpu.write8(o + 0xb2, 0);
    cpu.write16(o + 0x184, 0);
    cpu.write16(o + 0x186, 0);
    cpu.write32(o + FB_LIFE, FB_START_LIFE);
    let parts_set = |cpu: &mut Cpu, table: u32, count: u32, points: u32, state: u32, life: u32| {
        let number = cpu.read8(o + NUMBER) as u32;
        let (points, state, life) = (o + points, o + state, o + life);
        cpu.nested_with_stack(&PARTS_SET_PROLOGUE, 0x0605_6ea4, &[table, number], |cpu| {
            parts::parts_set(cpu, count, points, state, life, table, number)
        });
    };
    parts_set(cpu, 0x060a_0d84, 2, 0xbc, FB_BODY, FB_LIFE);
    let glow = nested!(cpu, 0x0605_6ff4, actors::depth_box_set(cpu, 2));
    if glow != 0 {
        let number = cpu.read8(o + NUMBER);
        cpu.write8(glow + NUMBER, number);
        cpu.write32(glow + DEPTH_BOX_AT, o + FB_GLOW_AT);
        cpu.write32(glow + DEPTH_BOX_WATCHED, o + FB_BODY);
    }
    for (k, (table, count, points, state, life, start)) in FB_PART_SETS.into_iter().enumerate() {
        cpu.write16(o + state, 0);
        cpu.write8(o + FB_PARTS[k].step, 0);
        if let Some(start) = start {
            cpu.write32(o + life, start);
        }
        parts_set(cpu, table, count, points, state, life);
    }
    nested!(cpu, 0x0609_493c, gondola_set(cpu, o));
    nested!(cpu, 0x0604_64d8, hud::boss_gauge_set(cpu, FB_START_LIFE));
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, FB_SOUND));
}

/// The fire boss made (numbered: list 2, 0x1f8 bytes, numbered_exec's; an enemy's number): its
/// number, or 0 if none was free.
const FB_SIZE: u32 = 0x1f8;

pub fn fb_make(cpu: &mut Cpu) -> u32 {
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, ENEMY_NUMBERS)) as i8;
    if n == -1 {
        return 0;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, FB_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, NUMBERED_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0609_4c2c, fb_set(cpu, o, n as i32 as u32));
    n as i32 as u32
}

// ---- the stage's events --------------------------------------------------------------------------

/// A timeline event's step (+1, a byte), a frame count (+4), and its load's handle, place and size
/// (+0x10, +0x14, +0x18); the opening message's lines (by the event's step: four from 0x060a10bc,
/// 0x060a10d0), its wait (+6, a word).
const EV_STEP: u32 = 1;
const EV_FRAMES: u32 = 4;
const EV_WAIT: u32 = 6;
const EV_LOAD: u32 = 0x10;
const EV_AT: u32 = 0x14;
const EV_SIZE: u32 = 0x18;
const MESSAGE_FIRST_LINES: u32 = 0x060a_10bc;
const MESSAGE_LINES: u32 = 0x060a_10d0;
static OPENING_SET_PROLOGUE: std::sync::OnceLock<super::Prologue> = std::sync::OnceLock::new();

/// The stage's message event: its four lines (by its step) after its wait. Done at once.
pub fn message_event(cpu: &mut Cpu, e: u32) -> u32 {
    let k = cpu.read8(e + EV_STEP) as u32;
    let [l2, l3, l4] = [0, 4, 8].map(|j| cpu.read32(MESSAGE_LINES + 12 * k + j));
    let l1 = cpu.read32(MESSAGE_FIRST_LINES + 4 * k);
    let wait = cpu.read16(e + EV_WAIT) as i16 as i32 as u32;
    cpu.nested_with_stack(&OPENING_SET_PROLOGUE, 0x0605_99c8, &[wait], |cpu| {
        text::opening_set(cpu, l1, l2, l3, l4, wait)
    });
    1
}

/// The director's flag bit 0, and the request its next events wait on; their timeline (kept at
/// 0x06074734).
const DIRECTOR_READY: u32 = 1;
const REQUESTED: u32 = 0x0607_4804;
const NEXT_EVENTS: u32 = 0x060a_1150;
const NEXT_TIMELINE: u32 = 0x0607_4734;

/// Waits for the director's flag bit 0, then for a request; then starts the next events' timeline
/// (-1: done).
pub fn next_events_event(cpu: &mut Cpu, e: u32) -> u32 {
    match cpu.read8(e + EV_STEP) {
        0 => {
            if nested!(cpu, 0x0604_0cb4, director::director_flags(cpu)) & DIRECTOR_READY != 0 {
                let step = cpu.read8(e + EV_STEP).wrapping_add(1);
                cpu.write8(e + EV_STEP, step);
            }
            0
        }
        1 => {
            if cpu.read32(REQUESTED) == 0 {
                return 0;
            }
            let t = nested!(cpu, 0x0605_7354, timeline::timeline_set(cpu, NEXT_EVENTS));
            cpu.write32(NEXT_TIMELINE, t);
            0xffff_ffff
        }
        _ => 0,
    }
}

/// A sound data file's load (file `n`), a step a frame: its place 0x7540 below the room's top, the
/// load started, its size once it has one, its end, its copy into the file's area; then done (1).
/// The event's frames counted down each time.
pub fn sound_load_event(cpu: &mut Cpu, e: u32, n: u32) -> u32 {
    let n = n & 0xff;
    let mut load = cpu.read32(e + EV_LOAD);
    let mut done = 0;
    match cpu.read8(e + EV_STEP) {
        0 => {
            let at = cpu.read32(ROOM_TOP).wrapping_add(0xffff_8ac0);
            cpu.write32(e + EV_AT, at);
            cpu.write8(e + EV_STEP, 1);
        }
        1 => {
            let at = cpu.read32(e + EV_AT);
            load = nested!(cpu, 0x0600_46d8, sound::load_sound_data(cpu, n, at));
            if load != 0 {
                let size = nested!(cpu, 0x0600_5edc, files::load_size(cpu, load));
                cpu.write32(e + EV_SIZE, size);
                cpu.write8(e + EV_STEP, 2);
            }
        }
        2 => {
            if nested!(cpu, 0x0600_6074, files::load_done(cpu, load)) != 0 {
                cpu.write8(e + EV_STEP, 3);
            }
        }
        3 => {
            let (at, size) = (cpu.read32(e + EV_AT), cpu.read32(e + EV_SIZE));
            nested!(cpu, 0x0600_4adc, sound::copy_to_area(cpu, n, at, size));
            cpu.write8(e + EV_STEP, 4);
        }
        4 => done = 1,
        _ => {}
    }
    cpu.write32(e + EV_LOAD, load);
    let frames = cpu.read32(e + EV_FRAMES).wrapping_sub(1);
    cpu.write32(e + EV_FRAMES, frames);
    done
}

/// Sound data file 26's load.
pub fn sound_load_26(cpu: &mut Cpu, e: u32) -> u32 {
    nested!(cpu, 0x0609_50e0, sound_load_event(cpu, e, 26))
}

/// Sound data file 27's load.
pub fn sound_load_27(cpu: &mut Cpu, e: u32) -> u32 {
    nested!(cpu, 0x0609_50e0, sound_load_event(cpu, e, 27))
}

/// Sound data file 28's load.
pub fn sound_load_28(cpu: &mut Cpu, e: u32) -> u32 {
    nested!(cpu, 0x0609_50e0, sound_load_event(cpu, e, 28))
}

/// Sound data file 29's load.
pub fn sound_load_29(cpu: &mut Cpu, e: u32) -> u32 {
    nested!(cpu, 0x0609_50e0, sound_load_event(cpu, e, 29))
}

/// Sound data file 30's load.
pub fn sound_load_30(cpu: &mut Cpu, e: u32) -> u32 {
    nested!(cpu, 0x0609_50e0, sound_load_event(cpu, e, 30))
}

/// The music change: its second load's size (+0x1c), its wait's base (+8); the sound manager's
/// music playing (+0x62), the game's music (0x060746e0's +0xa4).
const EV_SIZE_2: u32 = 0x1c;
const EV_BASE: u32 = 8;
const SOUND_MANAGER: u32 = 0x0600_edd0;
const MANAGER_PLAYING: u32 = 0x62;
const GAME_OBJECT: u32 = 0x0607_46e0;
const MUSIC_FADE: u32 = 200;
const NEXT_MUSIC: u32 = 2;

/// The music change, a step a frame (its frames counted down each time): sound data files 7 and 6
/// loaded one after the other into the room's top; once its frames are out the music faded over
/// 200 frames; once it has stopped, two frames on, both copied into their areas, and a wait of a
/// frame for every 8 KB copied and 4 more (or the base, if longer); then the game's music 2 asked
/// for. Then done (1).
pub fn music_event(cpu: &mut Cpu, e: u32) -> u32 {
    let mut load = cpu.read32(e + EV_LOAD);
    let mut done = 0;
    let frames_out = |cpu: &mut Cpu| cpu.read32(e + EV_FRAMES) as i32 <= 0;
    match cpu.read8(e + EV_STEP) {
        0 => {
            let at = cpu.read32(ROOM_TOP).wrapping_sub(ROOM);
            cpu.write32(e + EV_AT, at);
            cpu.write8(e + EV_STEP, 1);
        }
        1 | 3 => {
            let first = cpu.read8(e + EV_STEP) == 1;
            let at = if first { cpu.read32(e + EV_AT) } else { cpu.read32(e + EV_AT).wrapping_add(cpu.read32(e + EV_SIZE)) };
            load = nested!(cpu, 0x0600_46d8, sound::load_sound_data(cpu, if first { 7 } else { 6 }, at));
            if load != 0 {
                let size = nested!(cpu, 0x0600_5edc, files::load_size(cpu, load));
                cpu.write32(e + if first { EV_SIZE } else { EV_SIZE_2 }, size);
                cpu.write8(e + EV_STEP, if first { 2 } else { 4 });
            }
        }
        2 | 4 => {
            if nested!(cpu, 0x0600_6074, files::load_done(cpu, load)) != 0 {
                let step = cpu.read8(e + EV_STEP) + 1;
                cpu.write8(e + EV_STEP, step);
            }
        }
        5 => {
            if frames_out(cpu) {
                nested!(cpu, 0x0600_4f0c, sound::music_fade_out(cpu, MUSIC_FADE));
                cpu.write8(e + EV_STEP, 6);
            }
        }
        6 => {
            let manager = cpu.read32(SOUND_MANAGER);
            if cpu.read16(manager + MANAGER_PLAYING) == 0 {
                cpu.write32(e + EV_FRAMES, 2);
                cpu.write8(e + EV_STEP, 7);
            }
        }
        7 => {
            if frames_out(cpu) {
                let (at, size) = (cpu.read32(e + EV_AT), cpu.read32(e + EV_SIZE));
                nested!(cpu, 0x0600_4adc, sound::copy_to_area(cpu, 7, at, size));
                let at = cpu.read32(e + EV_AT).wrapping_add(cpu.read32(e + EV_SIZE));
                let size = cpu.read32(e + EV_SIZE_2);
                nested!(cpu, 0x0600_4adc, sound::copy_to_area(cpu, 6, at, size));
                let total = cpu.read32(e + EV_SIZE).wrapping_add(cpu.read32(e + EV_SIZE_2)) as i32;
                let total = total + if total < 0 { 0x1fff } else { 0 };
                nested!(cpu, 0x0600_815a, clib::ashiftrt_r4::<13>(cpu, total as u32));
                let base = cpu.read32(e + EV_BASE);
                let wait = nested!(cpu, 0x0606_4f50, math::at_least(cpu, base, ((total >> 13) + 4) as u32));
                cpu.write32(e + EV_FRAMES, wait);
                cpu.write8(e + EV_STEP, 9);
            }
        }
        9 => {
            if frames_out(cpu) {
                let game = cpu.read32(GAME_OBJECT);
                cpu.write16(game + MUSIC, NEXT_MUSIC as u16);
                nested!(cpu, 0x0600_4e98, sound::request_music(cpu, NEXT_MUSIC));
                cpu.write8(e + EV_STEP, 10);
            }
        }
        10 => done = 1,
        _ => {}
    }
    cpu.write32(e + EV_LOAD, load);
    let frames = cpu.read32(e + EV_FRAMES).wrapping_sub(1);
    cpu.write32(e + EV_FRAMES, frames);
    done
}

/// The play's own places for more kinds (states.rs's SETTINGS' current ones: where the next file
/// loads, VDP1's VRAM's next place, the next character and colour), copied into an event as it
/// loads (+0x14, +0x18, +0x1c and +0x1e, words).
const PLAY_LOAD_AT: u32 = 0x0607_40e4;
const PLAY_VRAM: u32 = 0x0607_4058;
const PLAY_CHAR: u32 = 0x0607_40ea;
const PLAY_COLOUR: u32 = 0x0607_40e8;
const EV_VRAM: u32 = 0x18;
const EV_CHAR: u32 = 0x1c;
const EV_COLOUR: u32 = 0x1e;
/// A kind's file's characters (its +6, a word), colours (+0x16) and sizes (+0, where the next
/// file loads); a kind's attributes' mode bits (8: its colours shared).
const KF_CHARS: u32 = 6;
const KF_COLOURS: u32 = 0x16;
const K_ATTRS: u32 = 0xa;
const K_MODE_BITS: u16 = 0x38;
const K_SHARED: u16 = 8;

/// The event's load places taken from the play's.
fn ev_places(cpu: &mut Cpu, e: u32) {
    let at = cpu.read32(PLAY_LOAD_AT);
    cpu.write32(e + EV_AT, at);
    let vram = cpu.read32(PLAY_VRAM);
    cpu.write32(e + EV_VRAM, vram);
    let ch = cpu.read16(PLAY_CHAR);
    cpu.write16(e + EV_CHAR, ch);
    let colour = cpu.read16(PLAY_COLOUR);
    cpu.write16(e + EV_COLOUR, colour);
}

/// Kind `k`'s file's load started at the event's place (a model's, or a sprite's): the load.
fn ev_kind_load(cpu: &mut Cpu, e: u32, k: u32, model: bool) -> u32 {
    let at = cpu.read32(e + EV_AT);
    if model {
        nested!(cpu, 0x0604_5910, kinds::kind_load(cpu, k, at))
    } else {
        nested!(cpu, 0x0604_56cc, kinds::kind_load(cpu, k, at))
    }
}

/// Whether a kind put in place moves the event's next colour on past its colours.
#[derive(Clone, Copy, PartialEq)]
enum Colours {
    Always,
    UnlessShared,
    Never,
}

/// Kind `k` (a model's, or a sprite's) fixed and put in place at the event's places, which move on
/// past it (its colours as `colours` says).
fn ev_kind_install(cpu: &mut Cpu, e: u32, k: u32, model: bool, colours: Colours) {
    let at = cpu.read32(e + EV_AT);
    if model {
        nested!(cpu, 0x0604_583c, kinds::model_kind_fix(cpu, k, at));
    } else {
        nested!(cpu, 0x0604_5638, kinds::sprite_kind_fix(cpu, k, at));
    }
    let vram = cpu.read32(e + EV_VRAM);
    let first = cpu.read16(e + EV_CHAR) as i16 as i32 as u32;
    let colour = cpu.read16(e + EV_COLOUR) as u32;
    let next = if model {
        nested!(cpu, 0x0604_5950, kinds::model_kind_install(cpu, k, vram, first, colour))
    } else {
        nested!(cpu, 0x0604_570c, kinds::sprite_kind_install(cpu, k, vram, first, colour))
    };
    cpu.write32(e + EV_VRAM, next);
    let kind = KINDS + K_SIZE * k;
    let file = cpu.read32(kind);
    let ch = cpu.read16(e + EV_CHAR).wrapping_add(cpu.read16(file + KF_CHARS));
    cpu.write16(e + EV_CHAR, ch);
    let counted = match colours {
        Colours::Always => true,
        Colours::UnlessShared => cpu.read16(kind + K_ATTRS) & K_MODE_BITS != K_SHARED,
        Colours::Never => false,
    };
    if counted {
        let file = cpu.read32(kind);
        let c = cpu.read16(e + EV_COLOUR).wrapping_add(cpu.read16(file + KF_COLOURS));
        cpu.write16(e + EV_COLOUR, c);
    }
    let file = cpu.read32(kind);
    let next = cpu.read32(file);
    cpu.write32(e + EV_AT, next);
}

/// The second scenery's kinds' event, a step a frame: model kind 38 loaded and put in place (its
/// polygons shaded and lit by 11 lights' worth, the Gouraud tables' ramp 24 and floor 13, the
/// light turned -0x2000), then sprite kind 39. Then done (1).
pub fn scenery_kinds_event(cpu: &mut Cpu, e: u32) -> u32 {
    let mut load = cpu.read32(e + EV_LOAD);
    let mut done = 0;
    match cpu.read8(e + EV_STEP) {
        0 => {
            ev_places(cpu, e);
            cpu.write8(e + EV_STEP, 1);
        }
        1 => {
            load = ev_kind_load(cpu, e, 38, true);
            if load != 0 {
                cpu.write8(e + EV_STEP, 2);
            }
        }
        2 => {
            if nested!(cpu, 0x0600_6074, files::load_done(cpu, load)) != 0 {
                ev_kind_install(cpu, e, 38, true, Colours::UnlessShared);
                nested!(cpu, 0x0606_2f74, gouraud::models_hss(cpu, 38, 11));
                nested!(cpu, 0x0606_3380, gouraud::models_lit(cpu, 38, 11));
                nested!(cpu, 0x0606_3168, gouraud::table_ramp(cpu, 24, 24, 24));
                nested!(cpu, 0x0606_30e0, gouraud::table_floor(cpu, 13, 13, 13));
                nested!(cpu, 0x0606_2fe8, gouraud::light_turned(cpu, 0xffff_e000, 0, 0));
                cpu.write8(e + EV_STEP, 3);
            }
        }
        3 => cpu.write8(e + EV_STEP, 4),
        4 => {
            load = ev_kind_load(cpu, e, 39, false);
            if load != 0 {
                cpu.write8(e + EV_STEP, 5);
            }
        }
        5 => {
            if nested!(cpu, 0x0600_6074, files::load_done(cpu, load)) != 0 {
                ev_kind_install(cpu, e, 39, false, Colours::Always);
                cpu.write8(e + EV_STEP, 6);
            }
        }
        6 => cpu.write8(e + EV_STEP, 7),
        7 => done = 1,
        _ => {}
    }
    cpu.write32(e + EV_LOAD, load);
    done
}

/// The passing gunship's event: made; done (1).
pub fn passing_ship_event(cpu: &mut Cpu, _e: u32) -> u32 {
    nested!(cpu, 0x0608_cd38, passing_ship_make(cpu));
    1
}

/// The gunship's event: made (its number kept at +8, over again until one is free); once its
/// number's slot is empty again, the events after it started (-1: done).
const EV_NUMBER: u32 = 8;
const AFTER_SHIP_EVENTS: u32 = 0x060a_17f0;

pub fn ship_event(cpu: &mut Cpu, e: u32) -> u32 {
    match cpu.read8(e + EV_STEP) {
        0 => {
            let n = nested!(cpu, 0x0608_c8b0, ship_make(cpu)) as i8 as i32 as u32;
            if n != 0 {
                let step = cpu.read8(e + EV_STEP).wrapping_add(1);
                cpu.write8(e + EV_STEP, step);
                cpu.write32(e + EV_NUMBER, n);
            }
            0
        }
        1 => {
            let n = cpu.read32(e + EV_NUMBER);
            if cpu.read32(PLAYERS.wrapping_add(n << 2)) != 0 {
                return 0;
            }
            let t = nested!(cpu, 0x0605_7354, timeline::timeline_set(cpu, AFTER_SHIP_EVENTS));
            cpu.write32(NEXT_TIMELINE, t);
            0xffff_ffff
        }
        _ => 0,
    }
}

/// The results it waits on (kept at +4 while they last), the events after it.
const EV_RESULTS: u32 = 4;
const AFTER_RESULTS_EVENTS: u32 = 0x060a_1830;

/// The area's change, a step a frame: the stage's results (60 frames on; forgotten once they
/// end); sprite kind 10, model kind 35 (shaded by 2 lights' worth), model kind 40 (shaded and lit
/// by 8, the Gouraud tables' ramp 26 and floor 15, the light turned 0x2000 and -0x2000) and sprite
/// kind 42 (its colours not counted) loaded and put in place; once the results are over, the
/// scenery's scroll moved on; then at a request the events after it (1: done).
pub fn area_change_event(cpu: &mut Cpu, e: u32) -> u32 {
    let mut load = cpu.read32(e + EV_LOAD);
    if cpu.read8(e + EV_STEP) != 0 {
        let results = cpu.read32(e + EV_RESULTS);
        if results != 0 && cpu.read8(results + OBJ_MODE) as i8 > 111 {
            cpu.write32(e + EV_RESULTS, 0);
        }
    }
    let next = |cpu: &mut Cpu| {
        let step = cpu.read8(e + EV_STEP).wrapping_add(1);
        cpu.write8(e + EV_STEP, step);
    };
    let loaded = |cpu: &mut Cpu, load: u32| nested!(cpu, 0x0600_6074, files::load_done(cpu, load)) != 0;
    let mut done = 0;
    match cpu.read8(e + EV_STEP) {
        0 => {
            let results = nested!(cpu, 0x0605_284c, results::results_set(cpu, 60));
            if results != 0 {
                next(cpu);
                cpu.write32(e + EV_RESULTS, results);
                ev_places(cpu, e);
            }
        }
        1 | 3 | 5 | 7 => {
            let (k, model) = match cpu.read8(e + EV_STEP) {
                1 => (10, false),
                3 => (35, true),
                5 => (40, true),
                _ => (42, false),
            };
            load = ev_kind_load(cpu, e, k, model);
            if load != 0 {
                next(cpu);
            }
        }
        2 | 8 => {
            if loaded(cpu, load) {
                let (k, colours) =
                    if cpu.read8(e + EV_STEP) == 2 { (10, Colours::Always) } else { (42, Colours::Never) };
                ev_kind_install(cpu, e, k, false, colours);
                next(cpu);
            }
        }
        4 => {
            if loaded(cpu, load) {
                ev_kind_install(cpu, e, 35, true, Colours::UnlessShared);
                nested!(cpu, 0x0606_2f74, gouraud::models_hss(cpu, 35, 2));
                next(cpu);
            }
        }
        6 => {
            if loaded(cpu, load) {
                ev_kind_install(cpu, e, 40, true, Colours::UnlessShared);
                nested!(cpu, 0x0606_2f74, gouraud::models_hss(cpu, 40, 8));
                nested!(cpu, 0x0606_3380, gouraud::models_lit(cpu, 40, 8));
                nested!(cpu, 0x0606_3168, gouraud::table_ramp(cpu, 26, 26, 26));
                nested!(cpu, 0x0606_30e0, gouraud::table_floor(cpu, 15, 15, 15));
                nested!(cpu, 0x0606_2fe8, gouraud::light_turned(cpu, 0x2000, 0xffff_e000, 0));
                next(cpu);
            }
        }
        9 => {
            if cpu.read32(e + EV_RESULTS) == 0 {
                next(cpu);
                nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
            }
        }
        10 if cpu.read32(REQUESTED) != 0 => {
            done = 1;
            let t = nested!(cpu, 0x0605_7354, timeline::timeline_set(cpu, AFTER_RESULTS_EVENTS));
            cpu.write32(NEXT_TIMELINE, t);
        }
        _ => {}
    }
    cpu.write32(e + EV_LOAD, load);
    done
}

/// The ground's motion functions (globals.rs's GROUND_DX_AT, GROUND_DY_AT, GROUND_DZ_AT) set to the
/// second area's. Done (1).
const AREA_2_GROUND: [(u32, u32); 3] = [
    (super::globals::GROUND_DX_AT, 0x0607_2f5c),
    (GROUND_DY_AT, 0x060a_110c),
    (super::globals::GROUND_DZ_AT, 0x0607_2f64),
];

pub fn ground_motion_event(cpu: &mut Cpu, _e: u32) -> u32 {
    for (at, f) in AREA_2_GROUND {
        cpu.write32(at, f);
    }
    1
}

/// The ground's motion functions set to the third area's (its own rise). Done (1).
const AREA_3_GROUND: [(u32, u32); 3] = [
    (super::globals::GROUND_DX_AT, 0x0607_2f5c),
    (GROUND_DY_AT, 0x0607_47ec),
    (super::globals::GROUND_DZ_AT, 0x0607_2f64),
];

pub fn ground_motion_3_event(cpu: &mut Cpu, _e: u32) -> u32 {
    for (at, f) in AREA_3_GROUND {
        cpu.write32(at, f);
    }
    1
}

/// The winged boss's event: made (its number kept at +8, over again until one is free); once its
/// number's slot is empty again, the events after it started (-1: done).
const AFTER_WB_EVENTS: u32 = 0x060a_1f70;

pub fn wb_event(cpu: &mut Cpu, e: u32) -> u32 {
    match cpu.read8(e + EV_STEP) {
        0 => {
            let n = nested!(cpu, 0x0608_fcf8, wb_make(cpu)) as i8 as i32 as u32;
            if n != 0 {
                let step = cpu.read8(e + EV_STEP).wrapping_add(1);
                cpu.write8(e + EV_STEP, step);
                cpu.write32(e + EV_NUMBER, n);
            }
            0
        }
        1 => {
            let n = cpu.read32(e + EV_NUMBER);
            if cpu.read32(PLAYERS.wrapping_add(n << 2)) != 0 {
                return 0;
            }
            let t = nested!(cpu, 0x0605_7354, timeline::timeline_set(cpu, AFTER_WB_EVENTS));
            cpu.write32(NEXT_TIMELINE, t);
            0xffff_ffff
        }
        _ => 0,
    }
}

/// The events after the second area's change.
const AFTER_AREA_2_EVENTS: u32 = 0x060a_1fb0;

/// The second area's change, a step a frame: the stage's results (60 frames on; forgotten once
/// they end); sprite kinds 16, 10 and 73, model kind 45 (shaded and lit by 13 lights' worth, the
/// Gouraud tables' ramp 24, 22, 20 and floor 10, 9, 8, the light turned 0x2800 and 0x2000), sprite
/// kind 46 (its colours not counted) and model kind 36 (shaded by 2) loaded and put in place; once
/// the results are over, the scenery's scroll moved on; then at a request the events after it (1:
/// done).
pub fn area_2_change_event(cpu: &mut Cpu, e: u32) -> u32 {
    if cpu.read8(e + EV_STEP) != 0 {
        let results = cpu.read32(e + EV_RESULTS);
        if results != 0 && cpu.read8(results + OBJ_MODE) as i8 > 111 {
            cpu.write32(e + EV_RESULTS, 0);
        }
    }
    let mut load = cpu.read32(e + EV_LOAD);
    let next = |cpu: &mut Cpu| {
        let step = cpu.read8(e + EV_STEP).wrapping_add(1);
        cpu.write8(e + EV_STEP, step);
    };
    let loaded = |cpu: &mut Cpu, load: u32| nested!(cpu, 0x0600_6074, files::load_done(cpu, load)) != 0;
    let mut done = 0;
    match cpu.read8(e + EV_STEP) {
        0 => {
            let results = nested!(cpu, 0x0605_284c, results::results_set(cpu, 60));
            if results != 0 {
                next(cpu);
                cpu.write32(e + EV_RESULTS, results);
                ev_places(cpu, e);
            }
        }
        step @ (1 | 3 | 5 | 7 | 10 | 12) => {
            let (k, model) = match step {
                1 => (16, false),
                3 => (10, false),
                5 => (73, false),
                7 => (45, true),
                10 => (46, false),
                _ => (36, true),
            };
            load = ev_kind_load(cpu, e, k, model);
            if load != 0 {
                next(cpu);
            }
        }
        step @ (2 | 4 | 6 | 11) => {
            if loaded(cpu, load) {
                let (k, colours) = match step {
                    2 => (16, Colours::Always),
                    4 => (10, Colours::Always),
                    6 => (73, Colours::Always),
                    _ => (46, Colours::Never),
                };
                ev_kind_install(cpu, e, k, false, colours);
                next(cpu);
            }
        }
        8 => {
            if loaded(cpu, load) {
                ev_kind_install(cpu, e, 45, true, Colours::UnlessShared);
                nested!(cpu, 0x0606_2f74, gouraud::models_hss(cpu, 45, 13));
                nested!(cpu, 0x0606_3380, gouraud::models_lit(cpu, 45, 13));
                nested!(cpu, 0x0606_3168, gouraud::table_ramp(cpu, 24, 22, 20));
                nested!(cpu, 0x0606_30e0, gouraud::table_floor(cpu, 10, 9, 8));
                nested!(cpu, 0x0606_2fe8, gouraud::light_turned(cpu, 0x2800, 0x2000, 0));
                next(cpu);
            }
        }
        9 => next(cpu),
        13 => {
            if loaded(cpu, load) {
                ev_kind_install(cpu, e, 36, true, Colours::UnlessShared);
                nested!(cpu, 0x0606_2f74, gouraud::models_hss(cpu, 36, 2));
                next(cpu);
            }
        }
        14 => {
            if cpu.read32(e + EV_RESULTS) == 0 {
                next(cpu);
                nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
            }
        }
        15 if cpu.read32(REQUESTED) != 0 => {
            done = 1;
            let t = nested!(cpu, 0x0605_7354, timeline::timeline_set(cpu, AFTER_AREA_2_EVENTS));
            cpu.write32(NEXT_TIMELINE, t);
        }
        _ => {}
    }
    cpu.write32(e + EV_LOAD, load);
    done
}

/// The events after the fire boss.
const AFTER_FB_EVENTS: u32 = 0x060a_2370;

/// The fire boss's event: made (its number kept at +8); once its number's slot is empty, the
/// stage's results (60 frames on); once they have ended, the scenery's scroll moved on; then at a
/// request the events after it (-1: done).
pub fn fb_event(cpu: &mut Cpu, e: u32) -> u32 {
    let next = |cpu: &mut Cpu| {
        let step = cpu.read8(e + EV_STEP).wrapping_add(1);
        cpu.write8(e + EV_STEP, step);
    };
    match cpu.read8(e + EV_STEP) {
        0 => {
            let n = nested!(cpu, 0x0609_4fc0, fb_make(cpu)) as i8 as i32 as u32;
            if n != 0 {
                next(cpu);
                cpu.write32(e + EV_NUMBER, n);
            }
        }
        1 => {
            let n = cpu.read32(e + EV_NUMBER);
            if cpu.read32(PLAYERS.wrapping_add(n << 2)) != 0 {
                return 0;
            }
            let results = nested!(cpu, 0x0605_284c, results::results_set(cpu, 60));
            if results != 0 {
                next(cpu);
                cpu.write32(e + EV_RESULTS, results);
            }
        }
        2 => {
            let results = cpu.read32(e + EV_RESULTS);
            if cpu.read8(results + OBJ_MODE) as i8 > 111 {
                next(cpu);
                nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
            }
        }
        3 if cpu.read32(REQUESTED) != 0 => {
            let t = nested!(cpu, 0x0605_7354, timeline::timeline_set(cpu, AFTER_FB_EVENTS));
            cpu.write32(NEXT_TIMELINE, t);
            return 0xffff_ffff;
        }
        _ => {}
    }
    0
}

/// The third area's kinds' event, a step a frame: model kind 43 (shaded and lit by 10 lights'
/// worth) and sprite kind 44 (its colours not counted) loaded and put in place. Then done (1).
pub fn area_3_kinds_event(cpu: &mut Cpu, e: u32) -> u32 {
    let mut load = cpu.read32(e + EV_LOAD);
    let next = |cpu: &mut Cpu| {
        let step = cpu.read8(e + EV_STEP).wrapping_add(1);
        cpu.write8(e + EV_STEP, step);
    };
    let loaded = |cpu: &mut Cpu, load: u32| nested!(cpu, 0x0600_6074, files::load_done(cpu, load)) != 0;
    let mut done = 0;
    match cpu.read8(e + EV_STEP) {
        0 => {
            ev_places(cpu, e);
            next(cpu);
        }
        1 => {
            load = ev_kind_load(cpu, e, 43, true);
            if load != 0 {
                cpu.write8(e + EV_STEP, 2);
            }
        }
        2 => {
            if loaded(cpu, load) {
                ev_kind_install(cpu, e, 43, true, Colours::UnlessShared);
                nested!(cpu, 0x0606_2f74, gouraud::models_hss(cpu, 43, 10));
                nested!(cpu, 0x0606_3380, gouraud::models_lit(cpu, 43, 10));
                next(cpu);
            }
        }
        3 => {
            load = ev_kind_load(cpu, e, 44, false);
            if load != 0 {
                next(cpu);
            }
        }
        4 => {
            if loaded(cpu, load) {
                ev_kind_install(cpu, e, 44, false, Colours::Never);
                next(cpu);
            }
        }
        5 => done = 1,
        _ => {}
    }
    cpu.write32(e + EV_LOAD, load);
    done
}

/// The events after the stage boss.
const AFTER_STAGE_BOSS_EVENTS: u32 = 0x060a_2510;

/// The stage boss's event: made (its number kept at +8); once its number's slot is empty, the
/// stage's results (60 frames on); once they have ended, at a request the events after it (-1:
/// done).
pub fn stage_boss_event(cpu: &mut Cpu, e: u32) -> u32 {
    let next = |cpu: &mut Cpu| {
        let step = cpu.read8(e + EV_STEP).wrapping_add(1);
        cpu.write8(e + EV_STEP, step);
    };
    let step = cpu.read8(e + EV_STEP);
    match step {
        0 => {
            let n = nested!(cpu, 0x0609_f028, sb_make(cpu)) as i8 as i32 as u32;
            if n != 0 {
                next(cpu);
                cpu.write32(e + EV_NUMBER, n);
            }
            return 0;
        }
        1 => {
            let n = cpu.read32(e + EV_NUMBER);
            if cpu.read32(PLAYERS.wrapping_add(n << 2)) != 0 {
                return 0;
            }
            let results = nested!(cpu, 0x0605_284c, results::results_set(cpu, 60));
            if results != 0 {
                next(cpu);
                cpu.write32(e + EV_RESULTS, results);
            }
            return 0;
        }
        2 => {
            let results = cpu.read32(e + EV_RESULTS);
            if cpu.read8(results + OBJ_MODE) as i8 > 111 {
                next(cpu);
            }
        }
        3 => {}
        _ => return 0,
    }
    if cpu.read32(REQUESTED) == 0 {
        return 0;
    }
    let t = nested!(cpu, 0x0605_7354, timeline::timeline_set(cpu, AFTER_STAGE_BOSS_EVENTS));
    cpu.write32(NEXT_TIMELINE, t);
    0xffff_ffff
}

/// The fourth area's kinds' event, a step a frame: sprite kind 10, model kind 47 (shaded and lit
/// by 22 lights' worth, the Gouraud tables' ramp 22, 24, 28 and floor 12, the light turned 0x6000
/// and -0x1000), sprite kind 48 (its colours not counted) and model kind 32 loaded and put in
/// place. Then done (1).
pub fn area_4_kinds_event(cpu: &mut Cpu, e: u32) -> u32 {
    let mut load = cpu.read32(e + EV_LOAD);
    let next = |cpu: &mut Cpu| {
        let step = cpu.read8(e + EV_STEP).wrapping_add(1);
        cpu.write8(e + EV_STEP, step);
    };
    let loaded = |cpu: &mut Cpu, load: u32| nested!(cpu, 0x0600_6074, files::load_done(cpu, load)) != 0;
    let mut done = 0;
    match cpu.read8(e + EV_STEP) {
        0 => {
            ev_places(cpu, e);
            next(cpu);
        }
        step @ (1 | 3 | 5 | 7) => {
            let (k, model) = match step {
                1 => (10, false),
                3 => (47, true),
                5 => (48, false),
                _ => (32, true),
            };
            load = ev_kind_load(cpu, e, k, model);
            if load != 0 {
                next(cpu);
            }
        }
        2 | 6 => {
            if loaded(cpu, load) {
                let (k, colours) = if cpu.read8(e + EV_STEP) == 2 { (10, Colours::Always) } else { (48, Colours::Never) };
                ev_kind_install(cpu, e, k, false, colours);
                next(cpu);
            }
        }
        4 => {
            if loaded(cpu, load) {
                ev_kind_install(cpu, e, 47, true, Colours::UnlessShared);
                nested!(cpu, 0x0606_2f74, gouraud::models_hss(cpu, 47, 22));
                nested!(cpu, 0x0606_3380, gouraud::models_lit(cpu, 47, 22));
                nested!(cpu, 0x0606_3168, gouraud::table_ramp(cpu, 22, 24, 28));
                nested!(cpu, 0x0606_30e0, gouraud::table_floor(cpu, 12, 12, 12));
                nested!(cpu, 0x0606_2fe8, gouraud::light_turned(cpu, 0x6000, 0xffff_f000, 0));
                next(cpu);
            }
        }
        8 => {
            if loaded(cpu, load) {
                ev_kind_install(cpu, e, 32, true, Colours::UnlessShared);
                next(cpu);
            }
        }
        9 => done = 1,
        _ => {}
    }
    cpu.write32(e + EV_LOAD, load);
    done
}

/// The boss's event: made; done (1).
pub fn boss_event(cpu: &mut Cpu, _e: u32) -> u32 {
    nested!(cpu, 0x0608_7430, boss_set(cpu));
    1
}

/// Where a piece of the stage's screens goes: the screen room's offset, or a VRAM address.
#[derive(Clone, Copy)]
enum ScreenAt {
    Room(u32),
    At(u32),
}
use ScreenAt::{At, Room};

/// A step of the stage's screens' set-up: a map put (the stage data's map, where, its width and
/// height in cells, its character offset and pattern name add: put_cells's two pushed arguments,
/// which stay pushed until a Pop), cells copied (the stage data's cells, where, how many bytes), a
/// stretch cleared (SGL's DMA copy of the zero block), the stage data's first colours, or the
/// last four maps' pushed arguments let go.
#[derive(Clone, Copy)]
enum ScreenStep {
    Put(u32, ScreenAt, u32, u32, u32, u32),
    Cells(u32, ScreenAt, u32),
    Clear(ScreenAt, u32),
    Palette,
    Pop,
}
use ScreenStep::{Cells, Clear, Palette, Pop, Put};

const STAGE_SCREENS: [ScreenStep; 191] = [
    Cells(2, At(0x25e1_0000), 0xca00),
    Put(13, Room(0x24000), 32, 32, 0, 0x800),
    Put(20, Room(0x24040), 32, 32, 0, 0x800),
    Put(23, Room(0x25000), 32, 32, 0, 0x800),
    Put(24, Room(0x25040), 32, 32, 0, 0x800),
    Pop,
    Put(21, Room(0x26000), 32, 32, 0, 0x800),
    Put(22, Room(0x26040), 32, 32, 0, 0x800),
    Put(25, Room(0x27000), 32, 32, 0, 0x800),
    Put(26, Room(0x27040), 32, 32, 0, 0x800),
    Pop,
    Put(27, Room(0x28000), 32, 32, 0, 0x800),
    Put(12, Room(0x28040), 32, 32, 0, 0x800),
    Put(16, Room(0x29000), 32, 32, 0, 0x800),
    Put(17, Room(0x29040), 32, 32, 0, 0x800),
    Pop,
    Put(14, Room(0x2a000), 32, 32, 0, 0x800),
    Put(15, Room(0x2a040), 32, 32, 0, 0x800),
    Put(18, Room(0x2b000), 32, 32, 0, 0x800),
    Put(19, Room(0x2b040), 32, 32, 0, 0x800),
    Pop,
    Put(32, Room(0x2c000), 32, 32, 0, 0x800),
    Put(31, Room(0x2c040), 32, 32, 0, 0x800),
    Put(33, Room(0x2d000), 32, 32, 0, 0x800),
    Put(34, Room(0x2d040), 32, 32, 0, 0x800),
    Pop,
    Put(32, At(0x25e4_a000), 32, 32, 0, 0x800),
    Put(31, At(0x25e4_a040), 32, 32, 0, 0x800),
    Put(30, At(0x25e4_b000), 32, 32, 0, 0x800),
    Put(29, At(0x25e4_b040), 32, 32, 0, 0x800),
    Pop,
    Put(28, At(0x25e4_c000), 32, 32, 0, 0x800),
    Put(29, At(0x25e4_c040), 32, 32, 0, 0x800),
    Put(28, At(0x25e4_d000), 32, 32, 0, 0x800),
    Put(29, At(0x25e4_d040), 32, 32, 0, 0x800),
    Pop,
    Cells(3, At(0x25e0_0000), 0x5b80),
    Put(70, At(0x25e4_e00e), 30, 32, 0, 0x0),
    Put(71, At(0x25e4_f00e), 30, 32, 0, 0x0),
    Put(72, At(0x25e5_0000), 32, 32, 0, 0x0),
    Put(72, At(0x25e5_1000), 32, 32, 0, 0x0),
    Pop,
    Put(73, At(0x25e5_0040), 32, 32, 0, 0x0),
    Put(73, At(0x25e5_1040), 32, 32, 0, 0x0),
    Put(74, At(0x25e5_2000), 32, 32, 0, 0x0),
    Put(74, At(0x25e5_3000), 32, 32, 0, 0x0),
    Pop,
    Put(75, At(0x25e5_2040), 32, 32, 0, 0x0),
    Put(75, At(0x25e5_3040), 32, 32, 0, 0x0),
    Put(80, At(0x25e5_4000), 32, 32, 0, 0x400),
    Put(81, At(0x25e5_4040), 32, 32, 0, 0x400),
    Pop,
    Put(82, At(0x25e5_5000), 32, 32, 0, 0x400),
    Put(81, At(0x25e5_5040), 32, 32, 0, 0x400),
    Put(82, At(0x25e5_6000), 32, 32, 0, 0x400),
    Put(79, At(0x25e5_6040), 32, 32, 0, 0x400),
    Pop,
    Put(82, At(0x25e5_7000), 32, 32, 0, 0x400),
    Put(81, At(0x25e5_7040), 32, 32, 0, 0x400),
    Put(82, At(0x25e5_8000), 32, 32, 0, 0x400),
    Put(81, At(0x25e5_8040), 32, 32, 0, 0x400),
    Pop,
    Put(78, At(0x25e5_9000), 32, 32, 0, 0x400),
    Put(81, At(0x25e5_9040), 32, 32, 0, 0x400),
    Put(82, At(0x25e5_a000), 32, 32, 0, 0x400),
    Put(81, At(0x25e5_a040), 32, 32, 0, 0x400),
    Pop,
    Put(82, At(0x25e5_b000), 32, 32, 0, 0x400),
    Put(77, At(0x25e5_b040), 32, 32, 0, 0x400),
    Put(82, At(0x25e5_c000), 32, 32, 0, 0x400),
    Put(81, At(0x25e5_c040), 32, 32, 0, 0x400),
    Pop,
    Put(82, At(0x25e5_d000), 32, 32, 0, 0x400),
    Put(81, At(0x25e5_d040), 32, 32, 0, 0x400),
    Cells(0, At(0x25e0_8000), 0x4520),
    Clear(At(0x25e4_0000), 0x2000),
    Put(10, At(0x25e4_2000), 32, 32, 0, 0x0),
    Put(4, At(0x25e4_2040), 32, 32, 0, 0x0),
    Pop,
    Put(8, At(0x25e4_3000), 32, 32, 0, 0x0),
    Put(2, At(0x25e4_3040), 32, 32, 0, 0x0),
    Put(5, At(0x25e4_4000), 32, 32, 0, 0x0),
    Put(11, At(0x25e4_4040), 32, 32, 0, 0x0),
    Pop,
    Put(3, At(0x25e4_5000), 32, 32, 0, 0x0),
    Put(9, At(0x25e4_5040), 32, 32, 0, 0x0),
    Put(10, At(0x25e4_6000), 32, 32, 0, 0x0),
    Put(4, At(0x25e4_6040), 32, 32, 0, 0x0),
    Pop,
    Put(10, At(0x25e4_7000), 32, 32, 0, 0x0),
    Put(4, At(0x25e4_7040), 32, 32, 0, 0x0),
    Put(5, At(0x25e4_8000), 32, 32, 0, 0x0),
    Put(11, At(0x25e4_8040), 32, 32, 0, 0x0),
    Pop,
    Put(5, At(0x25e4_9000), 32, 32, 0, 0x0),
    Put(11, At(0x25e4_9040), 32, 32, 0, 0x0),
    Cells(0, At(0x25e6_0000), 0x4520),
    Clear(At(0x25e7_2000), 0x2000),
    Clear(At(0x25e7_4000), 0x2000),
    Clear(At(0x25e7_0000), 0x2000),
    Clear(Room(0x7000), 0x1000),
    Put(2, Room(0xb000), 32, 32, 0, 0x0),
    Put(3, Room(0xb040), 32, 32, 0, 0x0),
    Pop,
    Put(4, Room(0xa000), 32, 32, 0, 0x0),
    Put(5, Room(0xa040), 32, 32, 0, 0x0),
    Put(4, Room(0x9000), 32, 32, 0, 0x0),
    Put(5, Room(0x9040), 32, 32, 0, 0x0),
    Pop,
    Put(6, Room(0x8000), 32, 32, 0, 0x0),
    Put(7, Room(0x8040), 32, 32, 0, 0x0),
    Put(11, Room(0x4000), 32, 32, 0, 0x0),
    Put(10, Room(0x4040), 32, 32, 0, 0x0),
    Pop,
    Put(9, Room(0x5000), 32, 32, 0, 0x0),
    Put(8, Room(0x5040), 32, 32, 0, 0x0),
    Put(35, Room(0x2000), 22, 32, 1, 0x0),
    Put(36, Room(0x3000), 22, 32, 1, 0x0),
    Pop,
    Put(0, Room(0), 32, 32, 0, 0x288),
    Put(0, Room(0x1040), 32, 32, 0, 0x288),
    Put(1, Room(0x40), 32, 32, 0, 0x288),
    Put(1, Room(0x1000), 32, 32, 0, 0x288),
    Pop,
    Cells(1, At(0x25e6_4520), 0xb480),
    Put(0, At(0x25e7_6000), 32, 32, 0, 0x229),
    Put(0, At(0x25e7_7040), 32, 32, 0, 0x229),
    Put(1, At(0x25e7_6040), 32, 32, 0, 0x229),
    Put(1, At(0x25e7_7000), 32, 32, 0, 0x229),
    Pop,
    Palette,
    Put(61, Room(0x1100e), 30, 32, 0, 0x0),
    Put(61, Room(0x1200e), 30, 32, 0, 0x0),
    Put(59, Room(0xc000), 32, 32, 0, 0x0),
    Put(60, Room(0xc040), 32, 32, 0, 0x0),
    Pop,
    Put(57, Room(0xd000), 32, 32, 0, 0x0),
    Put(58, Room(0xd040), 32, 32, 0, 0x0),
    Put(57, Room(0xe000), 32, 32, 0, 0x0),
    Put(58, Room(0xe040), 32, 32, 0, 0x0),
    Pop,
    Put(55, Room(0xf000), 32, 32, 0, 0x0),
    Put(56, Room(0xf040), 32, 32, 0, 0x0),
    Put(53, Room(0x10000), 32, 32, 0, 0x0),
    Put(54, Room(0x10040), 32, 32, 0, 0x0),
    Pop,
    Put(64, Room(0x1c000), 32, 32, 0, 0x0),
    Put(65, Room(0x1c040), 32, 32, 0, 0x0),
    Put(62, Room(0x1d000), 32, 32, 0, 0x0),
    Put(63, Room(0x1d040), 32, 32, 0, 0x0),
    Pop,
    Put(68, Room(0x1e000), 32, 32, 0, 0x0),
    Put(69, Room(0x1e040), 32, 32, 0, 0x0),
    Put(66, Room(0x1f000), 32, 32, 0, 0x0),
    Put(67, Room(0x1f040), 32, 32, 0, 0x0),
    Pop,
    Put(57, Room(0x20000), 32, 32, 0, 0x0),
    Put(58, Room(0x20040), 32, 32, 0, 0x0),
    Put(53, Room(0x21000), 32, 32, 0, 0x0),
    Put(54, Room(0x21040), 32, 32, 0, 0x0),
    Pop,
    Put(47, Room(0x13000), 32, 32, 0, 0x0),
    Put(48, Room(0x13040), 12, 32, 0, 0x0),
    Put(45, Room(0x14000), 32, 32, 0, 0x0),
    Put(46, Room(0x14040), 12, 32, 0, 0x0),
    Pop,
    Put(49, Room(0x15000), 32, 32, 0, 0x0),
    Put(50, Room(0x15040), 12, 32, 0, 0x0),
    Put(43, Room(0x16000), 32, 32, 0, 0x0),
    Put(44, Room(0x16040), 12, 32, 0, 0x0),
    Pop,
    Put(41, Room(0x17000), 32, 32, 0, 0x0),
    Put(42, Room(0x17040), 12, 32, 0, 0x0),
    Put(51, Room(0x18000), 32, 32, 0, 0x0),
    Put(52, Room(0x18040), 12, 32, 0, 0x0),
    Pop,
    Put(39, Room(0x19000), 32, 32, 0, 0x0),
    Put(40, Room(0x19040), 12, 32, 0, 0x0),
    Put(39, Room(0x1a000), 32, 32, 0, 0x0),
    Put(40, Room(0x1a040), 12, 32, 0, 0x0),
    Pop,
    Put(37, Room(0x1b000), 32, 32, 0, 0x0),
    Put(38, Room(0x1b040), 12, 32, 0, 0x0),
    Put(83, Room(0x22000), 32, 32, 0, 0x0),
    Put(84, Room(0x22040), 32, 32, 0, 0x0),
    Pop,
    Put(85, Room(0x23000), 32, 32, 0, 0x0),
    Put(86, Room(0x23040), 32, 32, 0, 0x0),
    Put(76, Room(0x6000), 32, 32, 0, 0x0),
    Put(76, Room(0x6040), 32, 32, 0, 0x0),
    Pop,
];

/// The stage's screens' start (screen.rs's scroll table): the screen room kept; the first time
/// (its count at 0x060a2b78, counted each time) the overlay's lists of screen addresses moved into
/// the room; the stage data set; RBG0's rotation parameters and coefficient table at the end of
/// VRAM's first half, its cells, both parameters (A's maps from a table, both coefficient tables
/// mode 0x61); the back colour; NBG0 and NBG1's cells and maps over the same VRAM, each scroll plane
/// at its place (NBG0 1.5 times, NBG1 a half its zoom); the screens shown and their priorities, the
/// sprites' type and colour calculation; then the maps, cells and colours (STAGE_SCREENS), the DMA
/// waited out, and the scroll marks.
const SCREEN_ROOM: u32 = 0x0607_47a0;
const SCREEN_STARTS: u32 = 0x060a_2b78;
const ROOM_LISTS: [(u32, u32); 7] = [
    (0x060a_2a98, 5),
    (0x060a_2aac, 2),
    (0x060a_2ab4, 7),
    (0x060a_2ad0, 8),
    (0x060a_2b08, 4),
    (0x060a_2af0, 6),
    (0x060a_2b18, 24),
];
const RPARA_TABLES: u32 = 0x25e3_fe00;
const KTABLE: u32 = 0x25e2_0000;
const KTABLE_MODE: u32 = 0x61;
const RA_MAPS: u32 = 0x060a_2d50;
const ZERO_BLOCK: u32 = 0x060a_2d60;
const ZERO_COPY_MODE: u32 = 0x11;
const SCROLL_MARKS: u32 = 0x54;
const MARKS: u32 = 0x060a_2a90;
const PRIORITIES: [(u32, u32); 5] = [(1, 3), (0, 4), (5, 5), (-7i32 as u32, 5), (-8i32 as u32, 4)];

pub fn stg01_start(cpu: &mut Cpu, data: u32, room: u32) {
    cpu.write32(SCREEN_ROOM, room);
    let starts = cpu.read32(SCREEN_STARTS);
    cpu.write32(SCREEN_STARTS, starts.wrapping_add(1));
    if starts == 0 {
        for (list, n) in ROOM_LISTS {
            let room = cpu.read32(SCREEN_ROOM);
            nested!(cpu, 0x0605_ef20, screen::offset_list(cpu, room, list, n));
        }
    }
    nested!(cpu, 0x0605_e17c, states::stage_data_set(cpu, data));
    nested!(cpu, 0x0609_f22c, sgl::sl_rpara_init_set_stg01(cpu, RPARA_TABLES));
    nested!(cpu, 0x0609_f29c, sgl::sl_make_ktable(cpu, KTABLE));
    nested!(cpu, 0x0606_e27c, sgl::sl_char_rbg0(cpu, 0, 0));
    nested!(cpu, 0x0606_e2ce, sgl::sl_page_rbg0(cpu, 0x25e0_8000, 0x200, 0x8000));
    nested!(cpu, 0x0609_f31c, sgl::sl_plane_ra(cpu, 0));
    nested!(cpu, 0x0609_f344, sgl::sl_map_ra_table(cpu, RA_MAPS));
    nested!(cpu, 0x0609_f39c, sgl::sl_over_ra(cpu, 0));
    nested!(cpu, 0x0609_f3ac, sgl::sl_ktable_ra(cpu, KTABLE, KTABLE_MODE));
    nested!(cpu, 0x0609_f42c, sgl::sl_rpara_mode(cpu, 0));
    nested!(cpu, 0x0609_f43c, sgl::sl_cur_rpara(cpu, 0));
    nested!(cpu, 0x0607_0dec, sgl::sl_back1_col_set(cpu, 0x25e3_fffe, 0));
    nested!(cpu, 0x0606_e288, sgl::sl_char_nbg0(cpu, 0, 0));
    nested!(cpu, 0x0606_e300, sgl::sl_page_nbg0(cpu, 0x25e6_0000, 0, 0xc000));
    nested!(cpu, 0x0607_0c3c, sgl::sl_plane_nbg0(cpu, 0));
    nested!(cpu, 0x0607_0c4c, sgl::sl_map_nbg0(cpu, 0x25e7_6000, 0x25e7_6000, 0x25e7_6000, 0x25e7_6000));
    nested!(cpu, 0x0607_0cfc, sgl::sl_zoom_mode(cpu, 1, 1));
    for (field, v) in [(bg::X, 0x100_0000), (bg::Y, 0), (bg::SPEED_Y, 0), (bg::SPEED_X, 0), (bg::ZOOM_Y, 0x1_8000), (bg::ZOOM_X, 0x1_8000)] {
        cpu.write32(bg::PLANE_A + field, v);
    }
    nested!(cpu, 0x0605_f040, bg::plane_a_frame(cpu));
    nested!(cpu, 0x0606_e294, sgl::sl_char_nbg1(cpu, 0, 0));
    nested!(cpu, 0x0606_e2e4, sgl::sl_page_nbg1(cpu, 0x25e6_0000, 0, 0xc000));
    nested!(cpu, 0x0607_0d0c, sgl::sl_plane_nbg1(cpu, 0));
    nested!(cpu, 0x0607_0d1c, sgl::sl_map_nbg1(cpu, 0x25e7_6000, 0x25e7_6000, 0x25e7_6000, 0x25e7_6000));
    nested!(cpu, 0x0607_0cfc, sgl::sl_zoom_mode(cpu, 0, 0));
    for (field, v) in [(bg::X, 0x100_0000), (bg::Y, 0x200_0000), (bg::SPEED_Y, 0), (bg::SPEED_X, 0), (bg::ZOOM_Y, 0x8000), (bg::ZOOM_X, 0x8000)] {
        cpu.write32(bg::PLANE_B + field, v);
    }
    nested!(cpu, 0x0605_f08c, bg::plane_b_frame(cpu));
    nested!(cpu, 0x0606_dd6c, sgl::sl_scr_auto_disp(cpu, 0x17));
    nested!(cpu, 0x0606_e23c, sgl::sl_bg_on(cpu, 0x17));
    for (screen, pri) in PRIORITIES {
        nested!(cpu, 0x0606_98dc, sgl::sl_priority(cpu, screen, pri));
    }
    nested!(cpu, 0x0609_f44c, sgl::sl_sprite_type(cpu, 3));
    nested!(cpu, 0x0607_0ddc, sgl::sl_color_calc(cpu, 0x53));
    nested!(cpu, 0x0609_f45c, sgl::sl_sprite_cc_condition(cpu, 1));
    nested!(cpu, 0x0609_f47c, sgl::sl_sprite_cc_number(cpu, 4));
    nested!(cpu, 0x0606_713c, sgl::sl_cur_window(cpu, 0));
    nested!(cpu, 0x0606_990c, sgl::sl_zdsp_level(cpu, 1));
    nested!(cpu, 0x0606_713c, sgl::sl_cur_window(cpu, 1));
    let place = |cpu: &mut Cpu, at: ScreenAt| match at {
        Room(off) => cpu.read32(SCREEN_ROOM).wrapping_add(off),
        At(a) => a,
    };
    for step in STAGE_SCREENS {
        match step {
            Put(map, at, w, h, ch, add) => {
                let src = states::stage_part(cpu, states::MAPS, map);
                let dst = place(cpu, at);
                cpu.push(&[ch, add]);
                nested!(cpu, 0x0605_ee3c, screen::put_cells(cpu, src, dst, w, h, ch, add));
            }
            Cells(k, at, bytes) => {
                let src = states::stage_part(cpu, states::CELLS, k);
                let dst = place(cpu, at);
                nested!(cpu, 0x0605_edf8, screen::copy_words(cpu, src, dst, bytes));
            }
            Clear(at, bytes) => {
                let dst = place(cpu, at);
                nested!(cpu, 0x0603_3aa0, sgl::sl_dmax_copy(cpu, ZERO_BLOCK, dst, bytes, ZERO_COPY_MODE));
            }
            Palette => {
                let colours = states::stage_part(cpu, states::COLOURS, 0);
                nested!(cpu, 0x0600_5b18, palette::set_colours(cpu, colours, 0, 0x10));
            }
            Pop => cpu.pop(32),
        }
    }
    nested!(cpu, 0x0606_e24c, system::wait_dma0(cpu));
    let s = cpu.read32(screen::SCREEN);
    cpu.write32(s + SCROLL_MARKS, MARKS);
}

// ---- the stage's end -----------------------------------------------------------------------------

/// The last scroll's start: once the director's flag bit 0 is set, the stage data's first 15 maps'
/// offsets made addresses, and the scenery's scroll moved on.
const STAGE_MAPS_LIST: u32 = 0x0607_47d0;
const STAGE_DATA_AT: u32 = 0x0607_4800;
const RELOCATED_MAPS: u32 = 15;

pub fn last_scroll_start(cpu: &mut Cpu) {
    if nested!(cpu, 0x0604_0cb4, director::director_flags(cpu)) & DIRECTOR_READY == 0 {
        return;
    }
    let (list, base) = (cpu.read32(STAGE_MAPS_LIST), cpu.read32(STAGE_DATA_AT));
    for k in 0..RELOCATED_MAPS {
        let v = cpu.read32(list + 4 * k).wrapping_add(base);
        cpu.write32(list + 4 * k, v);
    }
    nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
}

/// The stage's end's rumble (an actor): its step (+0x3c, a byte), its count (+0x40), its blend
/// (+0x3e, a word), its sound's control (+0x44, -1 none; the control's byte +0x47).
const RUMBLE_STEP: u32 = 0x3c;
const RUMBLE_COUNT: u32 = 0x40;
const RUMBLE_BLEND: u32 = 0x3e;
const RUMBLE_CONTROL: u32 = 0x44;
const RUMBLE_SOUND: u32 = 74;

/// The rumble: once the game is no longer at full speed, its sound stopped (if it has one) and it
/// ends. Else at the middle, back to front; after 32 frames its sound (74); then its blend from 0
/// to 0x3800 over 128 frames.
pub fn rumble_behaviour(cpu: &mut Cpu, o: u32) {
    if cpu.read8(FULL_SPEED) == 0 {
        if cpu.read32(o + RUMBLE_CONTROL) != 0xffff_ffff {
            let control = cpu.read8(o + RUMBLE_CONTROL + 3) as i8 as i32 as u32;
            nested!(cpu, 0x0600_52e0, sound::se_stop(cpu, RUMBLE_SOUND, control));
        }
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    let next = |cpu: &mut Cpu| {
        let step = cpu.read8(o + RUMBLE_STEP).wrapping_add(1);
        cpu.write8(o + RUMBLE_STEP, step);
    };
    let wait = |cpu: &mut Cpu| {
        let n = cpu.read32(o + RUMBLE_COUNT).wrapping_add(1);
        cpu.write32(o + RUMBLE_COUNT, n);
        if n == 32 {
            let control = nested!(cpu, 0x0600_50d4, sound::se_set(cpu, RUMBLE_SOUND)) as i8 as i32 as u32;
            cpu.write32(o + RUMBLE_CONTROL, control);
            cpu.write32(o + RUMBLE_COUNT, 0);
            next(cpu);
        }
    };
    match cpu.read8(o + RUMBLE_STEP) {
        0 => {
            for k in [0, 4, 8] {
                cpu.write32(o + POSITION + k, 0);
            }
            cpu.write32(o + RUMBLE_COUNT, 0);
            cpu.write16(o + TURN_Z, 0x8000);
            cpu.write16(o + RUMBLE_BLEND, 0);
            cpu.write32(o + RUMBLE_CONTROL, 0xffff_ffff);
            next(cpu);
            wait(cpu);
        }
        1 => wait(cpu),
        2 => {
            let t = cpu.read32(o + RUMBLE_COUNT).wrapping_add(0x200);
            cpu.write32(o + RUMBLE_COUNT, t);
            let blend = nested!(cpu, 0x0606_2ebc, math::ease(cpu, 0, 0x3800, t));
            cpu.write16(o + RUMBLE_BLEND, blend as u16);
            if cpu.read32(o + RUMBLE_COUNT) == 0x1_0000 {
                next(cpu);
            }
        }
        _ => {}
    }
}

/// A player's craft leaving at the stage's end (an actor): its step (+0x3c, a byte), a flag
/// (+0x3d), its count (+0x44), its speeds (+0x48 x, +0x4c y, +0x50 z), its turn's rate (+0x54),
/// its wobble (+0x40, a word; +0x5c), its drift (+0x78, +0x7c, +0x80), its trail's count (+0x68) and
/// sound (+0x6c, -1 none).
const LEAVE_STEP: u32 = 0x3c;
const LEAVE_FLAG: u32 = 0x3d;
const LEAVE_COUNT: u32 = 0x44;
const LEAVE_SPEED: u32 = 0x48;
const LEAVE_TURN_RATE: u32 = 0x54;

/// Its start: at the middle 10 dots back, still, upright and back to front, its turn's rate 0x2000,
/// its drift (-0x91eb, 0x91eb, 0x91eb), no sound; its next step.
pub fn leave_start(cpu: &mut Cpu, o: u32) {
    for (k, v) in [(0, 0), (4, 0), (8, 0xa_0000)] {
        cpu.write32(o + POSITION + k, v);
    }
    for k in [0, 4, 8] {
        cpu.write32(o + LEAVE_SPEED + k, 0);
    }
    cpu.write16(o + TURN_X, 0);
    cpu.write16(o + TURN_Y, 0);
    cpu.write16(o + TURN_Z, 0x8000);
    cpu.write32(o + 0x5c, 0);
    cpu.write16(o + 0x40, 0);
    cpu.write32(o + LEAVE_TURN_RATE, 0x2000);
    cpu.write32(o + LEAVE_COUNT, 0);
    cpu.write8(o + LEAVE_FLAG, 1);
    cpu.write32(o + 0x78, 0xffff_6e15);
    cpu.write32(o + 0x7c, 0x91eb);
    cpu.write32(o + 0x80, 0x91eb);
    cpu.write32(o + 0x68, 0);
    cpu.write32(o + 0x6c, 0xffff_ffff);
    let step = cpu.read8(o + LEAVE_STEP).wrapping_add(1);
    cpu.write8(o + LEAVE_STEP, step);
}

/// A wait of `frames` frames: then rising half a dot a frame and going 2 dots a frame further, its
/// next step (1); else 0.
pub fn leave_wait(cpu: &mut Cpu, o: u32, frames: u32) -> u32 {
    let n = cpu.read32(o + LEAVE_COUNT).wrapping_add(1);
    cpu.write32(o + LEAVE_COUNT, n);
    if n != frames {
        return 0;
    }
    cpu.write32(o + LEAVE_COUNT, 0);
    cpu.write32(o + LEAVE_SPEED + 4, 0x8000);
    cpu.write32(o + LEAVE_SPEED + 8, 0x2_0000);
    let step = cpu.read8(o + LEAVE_STEP).wrapping_add(1);
    cpu.write8(o + LEAVE_STEP, step);
    1
}

/// What differs between the two players' craft leaving: the player's slot, where the player's own
/// craft is put back (x), the wait before it sets off, its speed across then, its wobble's start
/// and the way it swings (to 0x400 either way, 8 a frame), where it goes off to without the player,
/// and whether its wobble and turn are cleared as it ends.
struct Leaver {
    player: u32,
    home_x: u32,
    wait: u32,
    vx: u32,
    wobble_flag: u32,
    wobble: u32,
    swing: i32,
    out_x: u32,
    off_x: u32,
    clears: bool,
}
const LEAVERS: [Leaver; 2] = [
    Leaver {
        player: PLAYERS,
        home_x: 0xffc0_0000,
        wait: 0xb0,
        vx: 0x8000,
        wobble_flag: 1,
        wobble: 0xffff_fe80,
        swing: 8,
        out_x: 0xffc0_0000,
        off_x: 0xff40_0000,
        clears: true,
    },
    Leaver {
        player: PLAYERS + 4,
        home_x: 0x40_0000,
        wait: 0xa0,
        vx: 0xffff_8000,
        wobble_flag: 0,
        wobble: 0x180,
        swing: -8,
        out_x: 0x40_0000,
        off_x: 0xc0_0000,
        clears: false,
    },
];
/// Its trail (+0x68: ended with it) and sound (+0x6c, its control's byte +0x6f); the leaving
/// steps not seen run (left to the SH-2 code); its wobble's speed (+0x58), wobble (+0x5c; its low
/// word the turn's rate), where it set off from (+0x60 x, +0x64 z).
const LEAVE_TRAIL: u32 = 0x68;
const LEAVE_CONTROL: u32 = 0x6c;
const LEAVE_SOUND: u32 = 76;
const LEAVE_STEPS: [u32; 3] = [0x0609_7c40, 0x0609_7c6c, 0x0609_7d38];
const LEAVE_WOBBLE_FLAG: u32 = 0x58;
const LEAVE_WOBBLE: u32 = 0x5c;
const LEAVE_FROM: u32 = 0x60;
/// A player's state's low byte (+0x6d) and hurt count (+0x90, a word), as player.rs names them.
const PLAYER_STATE_LOW: u32 = 0x6d;
const PLAYER_HURT: u32 = 0x90;

fn leave(cpu: &mut Cpu, o: u32, l: &Leaver) {
    if cpu.read8(o + LEAVE_STEP) == 0 {
        nested!(cpu, 0x0609_7b68, leave_start(cpu, o));
    }
    let trail = cpu.read32(o + LEAVE_TRAIL);
    if trail != 0 && cpu.read8(trail + OBJ_MODE) as i8 > 111 {
        cpu.write32(o + LEAVE_TRAIL, 0);
    }
    let end = |cpu: &mut Cpu| cpu.write8(o + OBJ_MODE, TO_STOP);
    if cpu.read8(FULL_SPEED) == 0 || cpu.read8(o + LEAVE_STEP) == 7 {
        let trail = cpu.read32(o + LEAVE_TRAIL);
        if trail != 0 {
            cpu.write8(trail + OBJ_MODE, TO_STOP);
        }
        if cpu.read32(o + LEAVE_CONTROL) != 0xffff_ffff {
            let control = cpu.read8(o + LEAVE_CONTROL + 3) as i8 as i32 as u32;
            nested!(cpu, 0x0600_52e0, sound::se_stop(cpu, LEAVE_SOUND, control));
        }
        let p = cpu.read32(l.player);
        if p != 0 {
            let flags = cpu.read32(p + FLAGS) & !NO_DRAW;
            cpu.write32(p + FLAGS, flags);
            cpu.write8(p + PLAYER_STATE_LOW, 0);
            cpu.write16(p + PLAYER_HURT, 0);
            for (k, v) in [(0, l.home_x), (4, 0x34_0000), (8, 0)] {
                cpu.write32(p + POSITION + k, v);
            }
        }
        end(cpu);
        return;
    }
    let next = |cpu: &mut Cpu| {
        let step = cpu.read8(o + LEAVE_STEP).wrapping_add(1);
        cpu.write8(o + LEAVE_STEP, step);
    };
    match cpu.read8(o + LEAVE_STEP) as i8 {
        1 => {
            if nested!(cpu, 0x0609_7c00, leave_wait(cpu, o, l.wait)) != 0 {
                cpu.write32(o + LEAVE_SPEED, l.vx);
                cpu.write32(o + LEAVE_WOBBLE_FLAG, l.wobble_flag);
                cpu.write32(o + LEAVE_WOBBLE, l.wobble);
            }
        }
        2 => {
            cpu.call(LEAVE_STEPS[0], &[o]);
        }
        3 => {
            cpu.call(LEAVE_STEPS[1], &[o]);
        }
        4 => {
            let w = cpu.read32(o + LEAVE_WOBBLE) as i32;
            if l.swing < 0 && w > -0x400 || l.swing > 0 && w <= 0x3ff {
                cpu.write32(o + LEAVE_WOBBLE, w.wrapping_add(l.swing) as u32);
            }
            cpu.call(LEAVE_STEPS[2], &[o]);
            let n = cpu.read32(o + LEAVE_COUNT).wrapping_add(1);
            cpu.write32(o + LEAVE_COUNT, n);
            if n == 0x100 {
                cpu.write32(o + LEAVE_COUNT, 0);
                for k in [8, 4, 0] {
                    cpu.write32(o + LEAVE_SPEED + k, 0);
                }
                let x = cpu.read32(o + POSITION);
                cpu.write32(o + LEAVE_FROM, x);
                let z = cpu.read32(o + POSITION + 8);
                cpu.write32(o + LEAVE_FROM + 4, z);
                next(cpu);
            }
        }
        5 => {
            let t = cpu.read32(o + LEAVE_COUNT).wrapping_add(0x200);
            cpu.write32(o + LEAVE_COUNT, t);
            let to = if cpu.read32(l.player) != 0 { l.out_x } else { l.off_x };
            let from = cpu.read32(o + LEAVE_FROM);
            let x = nested!(cpu, 0x0606_2ebc, math::ease(cpu, from, to, t));
            cpu.write32(o + POSITION, x);
            let from = cpu.read32(o + LEAVE_FROM + 4);
            let t = cpu.read32(o + LEAVE_COUNT);
            let z = nested!(cpu, 0x0606_2ebc, math::ease(cpu, from, 0, t));
            cpu.write32(o + POSITION + 8, z);
            let rate = nested!(cpu, 0x0606_2ebc, math::ease(cpu, 0x4000, 0, t));
            cpu.write32(o + LEAVE_TURN_RATE, rate);
            if cpu.read32(o + LEAVE_COUNT) == 0x8000 {
                cpu.write8(o + LEAVE_FLAG, 3);
            }
            if cpu.read32(o + LEAVE_COUNT) == 0x1_0000 {
                if cpu.read32(l.player) == 0 {
                    if l.clears {
                        cpu.write32(o + LEAVE_WOBBLE, 0);
                        cpu.write16(o + TURN_Y, 0);
                    }
                    end(cpu);
                    return;
                }
                cpu.write32(o + LEAVE_COUNT, 0);
                next(cpu);
            }
        }
        6 => {
            let v = (cpu.read16(o + TURN_Y) as i16 as i32).wrapping_add(0x8000);
            nested!(cpu, 0x0600_815e, clib::ashiftrt_r4::<11>(cpu, v as u32));
            if v == 16 {
                cpu.write8(o + LEAVE_STEP, 7);
                return;
            }
        }
        _ => {}
    }
    let turn = cpu.read16(o + TURN_Y).wrapping_add(cpu.read16(o + LEAVE_WOBBLE + 2));
    cpu.write16(o + TURN_Y, turn);
    for k in [0, 4, 8] {
        let p = cpu.read32(o + POSITION + k).wrapping_add(cpu.read32(o + LEAVE_SPEED + k));
        cpu.write32(o + POSITION + k, p);
    }
}

/// The second player's craft leaving.
pub fn leave_2p(cpu: &mut Cpu, o: u32) {
    leave(cpu, o, &LEAVERS[1]);
}

/// The first player's craft leaving.
pub fn leave_1p(cpu: &mut Cpu, o: u32) {
    leave(cpu, o, &LEAVERS[0]);
}

/// The stage's end's scroll work (a scroll's work): its step (+0, a byte), the planes' slide
/// (+0x1c, 16.16 of a plane) and its speed (+0x20), the planes' speed down (+0x24), a count
/// (+0x28), the view's angles kept (+0x10, +0x12, +0x14, words).
const END_STEP: u32 = 0;
const END_SLIDE: u32 = 0x1c;
const END_SLIDE_SPEED: u32 = 0x20;
const END_DOWN: u32 = 0x24;
const END_COUNT: u32 = 0x28;
const END_ANGLES: u32 = 0x10;
/// The screen object's (SCREEN's pointer) matrix, the view's kept there each frame.
const SCREEN_MATRIX: u32 = 0x70;

/// The planes slide on (a plane's height at a time: at each whole one NBG0 and NBG1 change places,
/// each zoomed one more, the screens shown changed the first time, and the second time the
/// screens changed and the scroll moved on instead); NBG1's colour rate by the slide (0 up to 15
/// over its first half, on to 31 over its last quarter); both planes scrolled down by the work's
/// speed, their zoom less the slide's speed.
pub fn planes_slide(cpu: &mut Cpu, w: u32) {
    let t = cpu.read32(w + END_SLIDE).wrapping_add(cpu.read32(w + END_SLIDE_SPEED));
    cpu.write32(w + END_SLIDE, t);
    if t as i32 > 0xffff {
        let step = cpu.read8(w + END_STEP) as i8;
        let swap = match step {
            1 => {
                nested!(cpu, 0x0606_e23c, sgl::sl_bg_on(cpu, 0x1_0016));
                cpu.write8(w + END_STEP, 2);
                true
            }
            2 => {
                nested!(cpu, 0x0606_e23c, sgl::sl_bg_on(cpu, 0x3_0014));
                nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
                return;
            }
            0 => true,
            _ => false,
        };
        if swap {
            let t = cpu.read32(w + END_SLIDE).wrapping_add(0xffff_0000);
            cpu.write32(w + END_SLIDE, t);
            for plane in [bg::PLANE_A, bg::PLANE_B] {
                let z = cpu.read32(plane + bg::ZOOM_X).wrapping_add(0x1_0000);
                cpu.write32(plane + bg::ZOOM_X, z);
            }
            let (ax, bx) = (cpu.read32(bg::PLANE_A + bg::X), cpu.read32(bg::PLANE_B + bg::X));
            let ay = cpu.read32(bg::PLANE_A + bg::Y);
            cpu.write32(bg::PLANE_A + bg::X, bx);
            let by = cpu.read32(bg::PLANE_B + bg::Y);
            cpu.write32(bg::PLANE_A + bg::Y, by);
            cpu.write32(bg::PLANE_B + bg::X, ax);
            cpu.write32(bg::PLANE_B + bg::Y, ay);
        }
    }
    let t = cpu.read32(w + END_SLIDE) as i32;
    let rate = if t <= 0x7fff {
        let v = 0x7fff - t;
        nested!(cpu, 0x0600_815e, clib::ashiftrt_r4::<11>(cpu, v as u32));
        15 - (v >> 11)
    } else if t > 0xc000 {
        let v = t - 0xc000;
        nested!(cpu, 0x0600_8160, clib::ashiftrt_r4::<10>(cpu, v as u32));
        (v >> 10) + 15
    } else {
        15
    };
    nested!(cpu, 0x0606_e45c, sgl::sl_col_rate(cpu, 0, rate as u32 & 0xffff));
    let down = cpu.read32(w + END_DOWN);
    nested!(cpu, 0x0605_efd4, bg::plane_a_speed(cpu, 0, down));
    for plane in [bg::PLANE_A, bg::PLANE_B] {
        let z = cpu.read32(plane + bg::ZOOM_X).wrapping_sub(cpu.read32(w + END_SLIDE_SPEED));
        cpu.write32(plane + bg::ZOOM_Y, z);
        cpu.write32(plane + bg::ZOOM_X, z);
        if plane == bg::PLANE_A {
            nested!(cpu, 0x0605_eff8, bg::plane_b_speed(cpu, 0, down));
        }
    }
    nested!(cpu, 0x0605_f040, bg::plane_a_frame(cpu));
    nested!(cpu, 0x0605_f08c, bg::plane_b_frame(cpu));
}

/// The stage's end's view (its place on its path, its position at +4 and its angles at +0x10) and
/// its path (keys from 0x060a2d88, the first's position taken); the rumble's model and both
/// players' craft leaving (0x060a4860, 0x06076840 and +4); the shading's Gouraud colour.
const END_VIEW: u32 = 0x0607_4750;
const END_VIEW_POSITION: u32 = 0x04;
const END_VIEW_ANGLES: u32 = 0x10;
/// The view's roll (the low word of its z angle).
const END_VIEW_ROLL: u32 = 0x1a;
/// The end's path's keys: the view's position (three longs).
const END_KEY: u32 = 12;
const END_PATH: u32 = 0x060a_2d88;
const END_RUMBLE: u32 = 0x060a_4860;
const END_CRAFT: u32 = 0x0607_6840;
const END_GOURAUD_COLOUR: u32 = 0xf39c;

/// The stage's end's start: the view at its path's first key, still; the planes' slide from a
/// half at 1/16 a frame, sliding down half a dot a frame; the sprites' priority and colour rate at
/// -7 (slPriority's numbering) 4 and 8; the shading set up (the Gouraud colour 0xf39c), the rumble and both players' craft leaving made;
/// the scene on; both players' own craft hidden (their state's low byte and hurt count cleared).
pub fn end_start(cpu: &mut Cpu, w: u32) {
    for k in [0, 4, 8] {
        let v = cpu.read32(END_PATH + k);
        cpu.write32(END_VIEW + END_VIEW_POSITION + k, v);
    }
    for k in [0, 4, 8] {
        cpu.write32(END_VIEW + END_VIEW_ANGLES + k, 0);
    }
    cpu.write32(END_VIEW, 0);
    cpu.write32(w + END_SLIDE_SPEED, 0);
    cpu.write32(w + END_DOWN, 0x8_0000);
    cpu.write32(w + END_SLIDE, 0x8000);
    cpu.write32(w + END_COUNT, 0);
    cpu.write8(w + END_STEP, 0);
    nested!(cpu, 0x0609_89c4, planes_slide(cpu, w));
    nested!(cpu, 0x0606_98dc, sgl::sl_priority(cpu, -7i32 as u32, 4));
    nested!(cpu, 0x0606_e45c, sgl::sl_col_rate(cpu, -7i32 as u32, 8));
    nested!(cpu, 0x0606_3248, gouraud::shading_init(cpu));
    nested!(cpu, 0x0607_1154, sgl::sl_set_gouraud_color(cpu, END_GOURAUD_COLOUR));
    nested!(cpu, 0x0606_4e54, models::pods_shading(cpu));
    let rumble = nested!(cpu, 0x0606_2df4, models::model31_set(cpu, 0x0609_7a6c));
    cpu.write32(END_RUMBLE, rumble);
    let craft = nested!(cpu, 0x0606_4d34, models::pod_set(cpu, 0x0609_80bc, 0));
    cpu.write32(END_CRAFT, craft);
    let craft = nested!(cpu, 0x0606_4d34, models::pod_set(cpu, 0x0609_7e28, 1));
    cpu.write32(END_CRAFT + 4, craft);
    nested!(cpu, 0x0606_1738, player::scene_on(cpu));
    for slot in [PLAYERS, PLAYERS + 4] {
        let p = cpu.read32(slot);
        if p != 0 {
            cpu.write8(p + PLAYER_STATE_LOW, 0);
            cpu.write16(p + PLAYER_HURT, 0);
            let flags = cpu.read32(p + FLAGS) | NO_DRAW;
            cpu.write32(p + FLAGS, flags);
        }
    }
}

/// The view's position at its place on the end's path, eased from its key to the next (`_w`, the
/// end's work, not used).
pub fn end_view_along(cpu: &mut Cpu, _w: u32) {
    let place = cpu.read32(END_VIEW);
    let key = (place >> 16) as i16 as i32;
    let t = place & 0xffff;
    let from = END_PATH.wrapping_add(key.wrapping_mul(END_KEY as i32) as u32);
    let to = from.wrapping_add(END_KEY);
    for k in [0, 4, 8] {
        let (a, b) = (cpu.read32(from + k), cpu.read32(to + k));
        let v = nested!(cpu, 0x0606_2ebc, math::ease(cpu, a, b, t));
        cpu.write32(END_VIEW + END_VIEW_POSITION + k, v);
    }
}

/// The stage's end, a frame (its steps: 0 and 1 the view along its path's first stretch, 2 a
/// wait of 0x80 frames, the rumble let go and the view's angles kept, 3 the view on along its
/// second stretch, its angles eased to none, 4 until the 1P craft's is past 111 (+3, a byte), 5
/// (or start pressed) the scene off and the scroll on): the view turned toward the 1P craft until
/// step 3, its matrix kept in the screen's; both planes scrolled down the screen as the view's
/// roll turns it; the ships still.
pub fn end_steps(cpu: &mut Cpu, w: u32) {
    if nested!(cpu, 0x0606_1780, player::start_pressed(cpu)) != 0 {
        cpu.write8(w + END_STEP, 5);
    }
    let craft = cpu.read32(END_CRAFT);
    if craft != 0 && cpu.read8(w + END_STEP) as i8 <= 2 {
        nested!(cpu, 0x0605_f40c, bg::view_toward(cpu, craft));
    }
    nested!(cpu, 0x0606_71cc, sgl::sl_push_matrix(cpu));
    nested!(cpu, 0x0605_f468, bg::view_matrix(cpu));
    let matrix = cpu.read32(screen::SCREEN) + SCREEN_MATRIX;
    nested!(cpu, 0x0606_db9c, sgl::sl_get_matrix(cpu, matrix));
    nested!(cpu, 0x0606_7200, sgl::sl_pop_matrix(cpu));
    let roll = cpu.read16(END_VIEW + END_VIEW_ROLL) as i16 as u32;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, roll));
    let down = cpu.read32(w + END_DOWN);
    let dx = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, down, s));
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, roll));
    let down = cpu.read32(w + END_DOWN);
    let dy = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, down, c)).wrapping_neg();
    nested!(cpu, 0x0605_efd4, bg::plane_a_speed(cpu, dx, dy));
    nested!(cpu, 0x0605_eff8, bg::plane_b_speed(cpu, dx, dy));
    nested!(cpu, 0x0605_f08c, bg::plane_b_frame(cpu));
    nested!(cpu, 0x0605_f040, bg::plane_a_frame(cpu));
    let step = cpu.read8(w + END_STEP) as i8;
    let next = match step {
        0 | 1 => {
            if step == 0 {
                cpu.write8(w + END_STEP, 1);
            }
            end_view_on(cpu, w, 0x1_0000)
        }
        2 => {
            let n = cpu.read32(w + END_COUNT).wrapping_add(1);
            cpu.write32(w + END_COUNT, n);
            if n == 0x80 {
                let rumble = cpu.read32(END_RUMBLE);
                if rumble != 0 {
                    cpu.write8(rumble + 3, 0x70);
                    cpu.write32(END_RUMBLE, 0);
                }
                cpu.write32(w + END_COUNT, 0);
                for k in [0, 2, 4] {
                    let a = cpu.read16(END_VIEW + END_VIEW_ANGLES + 2 + 2 * k);
                    cpu.write16(w + END_ANGLES + k, a);
                }
            }
            n == 0x80
        }
        3 => {
            if end_view_on(cpu, w, 0x2_0000) {
                for k in [0, 4, 8] {
                    cpu.write32(END_VIEW + END_VIEW_ANGLES + k, 0);
                }
                true
            } else {
                let t = cpu.read32(END_VIEW) & 0xffff;
                for k in [0, 2, 4] {
                    let a = cpu.read16(w + END_ANGLES + k) as i16 as u32;
                    let v = nested!(cpu, 0x0606_2ebc, math::ease(cpu, a, 0, t));
                    cpu.write32(END_VIEW + END_VIEW_ANGLES + 2 * k, v);
                }
                false
            }
        }
        4 => {
            let craft = cpu.read32(END_CRAFT);
            cpu.read8(craft + 3) as i8 > 111
        }
        5 => {
            nested!(cpu, 0x0606_175c, player::scene_off(cpu));
            nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
            false
        }
        _ => false,
    };
    if next {
        let step = cpu.read8(w + END_STEP).wrapping_add(1);
        cpu.write8(w + END_STEP, step);
    }
    nested!(cpu, 0x0606_16b8, player::ships_still(cpu));
}

/// The view on along the end's path, 1/256 of a key a frame, up to `end`: whether it is there.
fn end_view_on(cpu: &mut Cpu, w: u32, end: u32) -> bool {
    let place = cpu.read32(END_VIEW).wrapping_add(0x100);
    cpu.write32(END_VIEW, if place as i32 > end as i32 - 1 { end } else { place });
    nested!(cpu, 0x0609_847c, end_view_along(cpu, w));
    cpu.read32(END_VIEW) == end
}

/// The rotation scroll's plane (MOVER, bg.rs's: its angles x, y, z at +0 and three more words, its
/// position at +0x0c, its speed at +0x18, its scale at +0x24) as the sweep sets it out.
const SWEEP_PLANE: u32 = MOVER;
const SWEEP_POSITION: u32 = 0x0c;
const SWEEP_SPEED: u32 = 0x18;
const SWEEP_SCALE: u32 = 0x24;
/// Where it sets out from (x, y, z) and its speed down; past x 0xa00000 it is done.
const SWEEP_FROM: [u32; 3] = [0xfd00_0000, 0xff00_0000, 0x00cd_0000];
const SWEEP_DOWN: u32 = 0x2_0000;
const SWEEP_END_X: i32 = 0x00a0_0000;
/// The sweep (an object): its mode (+3: 0 setting out, 1 on, 112 done, 115 freed), its step
/// (+0x10), its colour rate (+0x14, 16.16) and its wait (+0x18).
const SWEEP_MODE: u32 = 3;
const SWEEP_STEP: u32 = 0x10;
const SWEEP_RATE: u32 = 0x14;
const SWEEP_WAIT: u32 = 0x18;
const SWEEP_SIZE: u32 = 0x1c;
/// The rotation scroll's (RBG0's) colour rate, slColRate's screen 5.
const SWEEP_SCREEN: u32 = 5;

/// The rotation scroll's plane swept across (while the game is not held): set out (still, at
/// rate 28) and then a step at a time: a wait of 180 frames; until it is 0x600000 down, its
/// coming forward (z) turned back; its rate down to 12, a 16th a frame; then down to none, 16th
/// by 16th, and colour calculation set to 67. Once on, it speeds up along x (to a whole a frame,
/// 1/32 more each), falls by its speed, comes forward by its own (its turn back eased off by 1/256
/// a frame until it is past 0x203fff), its matrix made and its screen's colour rate set; done once
/// past x 0xa00000.
pub fn sweep_behaviour(cpu: &mut Cpu, o: u32) {
    if cpu.read32(GAME_FLAGS) & HELD != 0 {
        return;
    }
    match cpu.read8(o + SWEEP_MODE) as i8 {
        0 => {
            for (k, v) in SWEEP_FROM.into_iter().enumerate() {
                cpu.write32(SWEEP_PLANE + SWEEP_POSITION + 4 * k as u32, v);
            }
            for (k, v) in [0, SWEEP_DOWN, 0].into_iter().enumerate() {
                cpu.write32(SWEEP_PLANE + SWEEP_SPEED + 4 * k as u32, v);
            }
            for k in 0..6 {
                cpu.write16(SWEEP_PLANE + 2 * k, 0);
            }
            for k in 0..3 {
                cpu.write32(SWEEP_PLANE + SWEEP_SCALE + 4 * k, 0x1_0000);
            }
            cpu.write32(o + SWEEP_RATE, 0x1c_0000);
            cpu.write32(o + SWEEP_STEP, 0);
            cpu.write32(o + SWEEP_WAIT, 180);
            cpu.write8(o + SWEEP_MODE, 1);
        }
        1 => {}
        112 => {
            cpu.write8(o + SWEEP_MODE, 115);
            return;
        }
        115 => {
            nested!(cpu, 0x0600_579c, objects::obj_free(cpu, o));
            return;
        }
        _ => return,
    }
    let next = match cpu.read32(o + SWEEP_STEP) {
        0 => {
            let wait = cpu.read32(o + SWEEP_WAIT).wrapping_sub(1);
            cpu.write32(o + SWEEP_WAIT, wait);
            wait == 0
        }
        1 => {
            let far = cpu.read32(SWEEP_PLANE + SWEEP_POSITION + 4) as i32 > 0x5f_ffff;
            if far {
                cpu.write32(SWEEP_PLANE + SWEEP_SPEED + 8, 0xffff_a000);
            }
            far
        }
        2 => {
            let rate = cpu.read32(o + SWEEP_RATE).wrapping_sub(0x1_0000);
            cpu.write32(o + SWEEP_RATE, rate);
            rate as i32 <= 0xc_0000
        }
        3 => {
            let rate = cpu.read32(o + SWEEP_RATE).wrapping_sub(0x1000);
            cpu.write32(o + SWEEP_RATE, rate);
            let none = (rate as i32) < 0;
            if none {
                cpu.write32(o + SWEEP_RATE, 0);
                nested!(cpu, 0x0607_0ddc, sgl::sl_color_calc(cpu, 67));
            }
            none
        }
        _ => false,
    };
    if next {
        let step = cpu.read32(o + SWEEP_STEP).wrapping_add(1);
        cpu.write32(o + SWEEP_STEP, step);
    }
    if cpu.read32(o + SWEEP_STEP) == 0 {
        return;
    }
    let speed = cpu.read32(SWEEP_PLANE + SWEEP_SPEED).wrapping_add(0x800);
    let speed = (speed as i32).min(0x1_0000) as u32;
    cpu.write32(SWEEP_PLANE + SWEEP_SPEED, speed);
    let x = cpu.read32(SWEEP_PLANE + SWEEP_POSITION).wrapping_add(speed);
    cpu.write32(SWEEP_PLANE + SWEEP_POSITION, x);
    if x as i32 > SWEEP_END_X {
        cpu.write8(o + SWEEP_MODE, 112);
    }
    let dy = cpu.read32(SWEEP_PLANE + SWEEP_SPEED + 4);
    let y = cpu.read32(SWEEP_PLANE + SWEEP_POSITION + 4).wrapping_add(dy);
    cpu.write32(SWEEP_PLANE + SWEEP_POSITION + 4, y);
    let dz = cpu.read32(SWEEP_PLANE + SWEEP_SPEED + 8);
    let z = cpu.read32(SWEEP_PLANE + SWEEP_POSITION + 8).wrapping_add(dz);
    cpu.write32(SWEEP_PLANE + SWEEP_POSITION + 8, z);
    if z as i32 <= 0x20_3fff {
        let dz = dz.wrapping_add(0x100);
        cpu.write32(SWEEP_PLANE + SWEEP_SPEED + 8, if dz as i32 > 0 { 0 } else { dz });
    }
    nested!(cpu, 0x0605_f530, bg::rotation_matrix(cpu, SWEEP_PLANE));
    let rate = cpu.read32(o + SWEEP_RATE) >> 16;
    nested!(cpu, 0x0606_e45c, sgl::sl_col_rate(cpu, SWEEP_SCREEN, rate));
}

/// The stage's colour lists (states.rs's STAGE_PARTS[COLOURS]) and data block: the second list's
/// 256 colours are the clear's.
const STAGE_COLOURS_LIST: u32 = 0x0607_47f8;
const CLEAR_COLOURS: u32 = 0x100;
/// The clear's message.
const CLEAR_MESSAGE: u32 = 0x060a_2d00;

/// The stage's clear's start: the shading ended, the fades stopped and the colour offset none; the
/// clear's colours set; the planes' slide from a half, still, sliding up half a dot a frame; the
/// sweep made; the sprites' priority at -7 (slPriority's numbering) 5, their colour rates at -7,
/// -8, -5 and -6 0, 8, 16 and 24; the clear's message shown.
pub fn clear_start(cpu: &mut Cpu, w: u32) {
    nested!(cpu, 0x0606_3294, gouraud::shading_end(cpu));
    nested!(cpu, 0x0605_e9d8, screen::stop_fades(cpu));
    nested!(cpu, 0x0605_e6dc, screen::set_fade_offset(cpu, 0, 0, 0));
    let list = cpu.read32(STAGE_COLOURS_LIST);
    let colours = cpu.read32(list + 4).wrapping_add(cpu.read32(STAGE_DATA_AT));
    nested!(cpu, 0x0600_5b18, palette::set_colours(cpu, colours, CLEAR_COLOURS, CLEAR_COLOURS));
    cpu.write32(w + END_SLIDE_SPEED, 0);
    cpu.write32(w + END_DOWN, 0xfff8_0000);
    cpu.write32(w + END_SLIDE, 0x8000);
    cpu.write8(w + END_STEP, 0);
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, SWEEP_SIZE)) as i8 as i32 as u32;
    nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 0, blocks, 0x0609_8724));
    nested!(cpu, 0x0606_98dc, sgl::sl_priority(cpu, -7i32 as u32, 5));
    for (screen, rate) in [(-7i32, 0), (-8, 8), (-5, 16), (-6, 24)] {
        nested!(cpu, 0x0606_e45c, sgl::sl_col_rate(cpu, screen as u32, rate));
    }
    nested!(cpu, 0x0606_1620, text::message_set(cpu, CLEAR_MESSAGE));
}

/// The clear's slide, its sliding up eased off to a quarter dot a frame.
pub fn slide_easing(cpu: &mut Cpu, w: u32) {
    let down = cpu.read32(w + END_DOWN) as i32;
    if down < -0x2_0000 {
        cpu.write32(w + END_DOWN, (down + 0x51e) as u32);
    }
    nested!(cpu, 0x0609_89c4, planes_slide(cpu, w));
}

/// The clear's slide, sliding faster (to 0x300 a frame, 16 more each) and up faster (to about
/// 0x4999a, 0xccc more each).
pub fn slide_up(cpu: &mut Cpu, w: u32) {
    let speed = cpu.read32(w + END_SLIDE_SPEED) as i32;
    if speed <= 0x2ff {
        cpu.write32(w + END_SLIDE_SPEED, (speed + 16) as u32);
    }
    let down = cpu.read32(w + END_DOWN) as i32;
    if down > 0xfffb_6667u32 as i32 {
        cpu.write32(w + END_DOWN, (down - 0xccc) as u32);
    }
    nested!(cpu, 0x0609_89c4, planes_slide(cpu, w));
}

/// The clear's slide, sliding faster still (to 0x5c0 a frame, 8 more each) and turned down (to
/// about 0x73333, 0x1999 more each).
pub fn slide_down(cpu: &mut Cpu, w: u32) {
    let speed = cpu.read32(w + END_SLIDE_SPEED) as i32;
    if speed <= 0x5bf {
        cpu.write32(w + END_SLIDE_SPEED, (speed + 8) as u32);
    }
    let down = cpu.read32(w + END_DOWN) as i32;
    if down <= 0x7_3332 {
        cpu.write32(w + END_DOWN, (down + 0x1999) as u32);
    }
    nested!(cpu, 0x0609_89c4, planes_slide(cpu, w));
}

/// The clear's rotation plane: its maps' table (for slMapRA), where it sets out (x, y, z), its
/// cells (five pieces of 0x2000 bytes from the screen's room at +0x24000 to VRAM at 0x25e40000,
/// queued 0x1000 at a time), its 80 colours (the stage's first colour list's) at 0x100.
const CLEAR_RA_MAPS: u32 = 0x060a_2dac;
const CLEAR_PLANE_AT: [u32; 3] = [0x00b0_0000, 0, 0x0400_0000];
const CLEAR_CELLS: u32 = 0x2_4000;
const CLEAR_CELLS_VRAM: u32 = 0x25e4_0000;
const CLEAR_CELL_PIECE: u32 = 0x2000;
const CLEAR_CELL_PIECES: u32 = 5;
const CLEAR_PLANE_COLOURS: u32 = 80;
/// The clear's scroll work's rotation plane speed down (+0x28, END_COUNT's place in the end's), as
/// the plane's start leaves it.
const CLEAR_PLANE_SPEED: u32 = 0x28;
const CLEAR_PLANE_DOWN: u32 = 0x2_0000;

/// The clear's rotation plane set up: the scroll's step on; RBG0's page (its cells at 0x25e00000,
/// 512 colours, 0xc000) and its maps; the plane at its start, unturned, still and unscaled; its
/// colours set and its cells queued; RBG0 (slPriority's 5) at priority 2; its speed down 2.
pub fn clear_plane_start(cpu: &mut Cpu, w: u32) {
    let step = cpu.read8(w + END_STEP).wrapping_add(1);
    cpu.write8(w + END_STEP, step);
    nested!(cpu, 0x0606_e2ce, sgl::sl_page_rbg0(cpu, 0x25e0_0000, 0x200, 0xc000));
    nested!(cpu, 0x0609_f344, sgl::sl_map_ra_table(cpu, CLEAR_RA_MAPS));
    for (k, v) in CLEAR_PLANE_AT.into_iter().enumerate() {
        cpu.write32(SWEEP_PLANE + SWEEP_POSITION + 4 * k as u32, v);
    }
    for k in 0..3 {
        cpu.write32(SWEEP_PLANE + SWEEP_SPEED + 4 * k, 0);
    }
    for k in 0..6 {
        cpu.write16(SWEEP_PLANE + 2 * k, 0);
    }
    for k in 0..3 {
        cpu.write32(SWEEP_PLANE + SWEEP_SCALE + 4 * k, 0x1_0000);
    }
    let list = cpu.read32(STAGE_COLOURS_LIST);
    let colours = cpu.read32(list).wrapping_add(cpu.read32(STAGE_DATA_AT));
    nested!(cpu, 0x0600_5b18, palette::set_colours(cpu, colours, CLEAR_COLOURS, CLEAR_PLANE_COLOURS));
    for k in 0..CLEAR_CELL_PIECES {
        let src = cpu.read32(SCREEN_ROOM) + CLEAR_CELLS + k * CLEAR_CELL_PIECE;
        let dst = CLEAR_CELLS_VRAM + k * CLEAR_CELL_PIECE;
        nested!(cpu, 0x0605_ed2c, screen::queue_long_copy(cpu, src, dst, CLEAR_CELL_PIECE, 0x1000));
    }
    nested!(cpu, 0x0606_98dc, sgl::sl_priority(cpu, 5, 2));
    cpu.write32(w + CLEAR_PLANE_SPEED, CLEAR_PLANE_DOWN);
}

/// The clear's rotation plane, a frame: its matrix (moved to its position, y turned up, then
/// turned about y, x and z) made RBG0's rotation parameters; then moved down by its speed.
pub fn clear_plane_frame(cpu: &mut Cpu, w: u32) {
    nested!(cpu, 0x0606_71cc, sgl::sl_push_matrix(cpu));
    let [x, y, z] = [0, 4, 8].map(|k| cpu.read32(SWEEP_PLANE + SWEEP_POSITION + k));
    nested!(cpu, 0x0606_721c, sgl::sl_translate(cpu, x, y.wrapping_neg(), z));
    let [ax, ay, az] = [0, 2, 4].map(|k| cpu.read16(SWEEP_PLANE + k) as i16 as u32);
    nested!(cpu, 0x0606_736c, sgl::sl_rot_y(cpu, ay));
    nested!(cpu, 0x0606_72ec, sgl::sl_rot_x(cpu, ax));
    nested!(cpu, 0x0606_726c, sgl::sl_rot_z(cpu, az));
    nested!(cpu, 0x0606_e50c, sgl::rotation_parameters(cpu));
    nested!(cpu, 0x0606_7200, sgl::sl_pop_matrix(cpu));
    let dy = cpu.read32(w + CLEAR_PLANE_SPEED);
    nested!(cpu, 0x0605_f3a4, bg::mover_move(cpu, 0, dy, 0));
}

/// The clear's step with both: the planes' slide and the rotation plane.
pub fn clear_slide_and_plane(cpu: &mut Cpu, w: u32) {
    nested!(cpu, 0x0609_89c4, planes_slide(cpu, w));
    nested!(cpu, 0x0609_8d64, clear_plane_frame(cpu, w));
}

/// The clear's step that does nothing.
pub fn clear_idle(_cpu: &mut Cpu, _w: u32) {}

/// The clear's step with the rotation plane alone.
pub fn clear_plane_only(cpu: &mut Cpu, w: u32) {
    nested!(cpu, 0x0609_8d64, clear_plane_frame(cpu, w));
}

/// NBG1 scrolled up by the work's speed (+0x24) as the rotation plane's z angle turns it.
pub fn plane_b_turned(cpu: &mut Cpu, w: u32) {
    let angle = cpu.read16(SWEEP_PLANE + 4) as i16 as u32;
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, angle));
    let up = cpu.read32(w + END_DOWN).wrapping_neg();
    let dy = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, up, c));
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, angle));
    let up = cpu.read32(w + END_DOWN).wrapping_neg();
    let dx = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, up, s));
    nested!(cpu, 0x0605_eff8, bg::plane_b_speed(cpu, dx, dy));
    nested!(cpu, 0x0605_f08c, bg::plane_b_frame(cpu));
}

/// The clear's two ground maps (bg.rs's ground map: its length in blocks (a byte), all loaded (a
/// byte), the lines it keeps ahead (a word), where the next rows go and the plane's size less one
/// (words), its place and how far it is loaded (16.16 lines), the plane's cells, its blocks'
/// table, its empty cell and its shapes), both in NBG0's plane.
const CLEAR_MAPS: [u32; 2] = [0x060a_4840, 0x060a_48c0];
const MAP_BLOCKS: u32 = 0x00;
const MAP_LOADED_ALL: u32 = 0x01;
const MAP_AHEAD: u32 = 0x02;
const MAP_PLANE_AT: u32 = 0x04;
const MAP_PLANE_MASK: u32 = 0x06;
const MAP_AT: u32 = 0x08;
const MAP_LOADED: u32 = 0x0c;
const MAP_PLANE_CELLS: u32 = 0x10;
const MAP_BLOCK_TABLE: u32 = 0x14;
const MAP_SHAPES: u32 = 0x1c;
/// A ground map set out: its length, its blocks' table (screen offset lists made addresses), its
/// plane's cells, where it starts (16.16 lines), its plane's size less one, the lines it keeps
/// ahead and its shapes; nothing of it in the plane yet, and loaded to 0x2000000.
struct MapSet {
    blocks: u8,
    table: u32,
    cells: u32,
    at: u32,
    mask: u16,
    ahead: u16,
    shapes: u32,
}

const MAP_LOADED_FROM: u32 = 0x0200_0000;

fn map_set(cpu: &mut Cpu, map: u32, m: &MapSet) {
    cpu.write8(map + MAP_BLOCKS, m.blocks);
    cpu.write8(map + MAP_LOADED_ALL, 0);
    cpu.write32(map + MAP_BLOCK_TABLE, m.table);
    cpu.write32(map + MAP_PLANE_CELLS, m.cells);
    cpu.write32(map + MAP_AT, m.at);
    cpu.write32(map + MAP_LOADED, MAP_LOADED_FROM);
    cpu.write16(map + MAP_PLANE_AT, 0);
    cpu.write16(map + MAP_PLANE_MASK, m.mask);
    cpu.write16(map + MAP_AHEAD, m.ahead);
    cpu.write32(map + MAP_SHAPES, m.shapes);
}

const CLEAR_MAP_SET: [MapSet; 2] = [
    MapSet {
        blocks: 5,
        table: 0x060a_2a98,
        cells: 0x25e7_2000,
        at: 0x00e0_0000,
        mask: 0x3fff,
        ahead: 0x120,
        shapes: 0,
    },
    MapSet {
        blocks: 2,
        table: 0x060a_2aac,
        cells: 0x25e7_0000,
        at: 0x00e0_0000,
        mask: 0x1fff,
        ahead: 0x90,
        shapes: 0,
    },
];
/// NBG0's cells (0x25e60000, 0x8000) and its four maps.
const CLEAR_NBG0_CELLS: u32 = 0x25e6_0000;
const CLEAR_NBG0_MAPS: [u32; 4] = [0x25e7_2000, 0x25e7_0000, 0x25e7_4000, 0x25e7_0000];
/// NBG0's plane's centre and zoom (two: half size).
const CLEAR_NBG0_AT: [u32; 2] = [0x0100_0000, 0x0190_0000];
const CLEAR_NBG0_ZOOM: u32 = 0x2_0000;
/// The clear's ground scroll's work: its step (+1, a byte), NBG0's speed down (+4) and its zoom
/// (+8); NBG1's speed (+0x24, END_DOWN).
const GROUND_STEP: u32 = 1;
const GROUND_SPEED: u32 = 4;
const GROUND_ZOOM: u32 = 8;

/// The clear's ground scroll's start: the clear's colours at 0; NBG0's page and maps; the two
/// ground maps set out; NBG0's plane at its place, half size; colour offset B on 18; colour
/// calculation 0x142; NBG0, NBG2 and the rotation plane shown (0x20015); sprite type 5; the
/// priorities (sprites at -8 4, NBG1 and NBG0 4, RBG0 3); colour offset B none; NBG0 moving up 2
/// dots a frame at zoom 2, NBG1's speed 2.
pub fn ground_start(cpu: &mut Cpu, w: u32) {
    let list = cpu.read32(STAGE_COLOURS_LIST);
    let colours = cpu.read32(list + 4).wrapping_add(cpu.read32(STAGE_DATA_AT));
    nested!(cpu, 0x0600_5b18, palette::set_colours(cpu, colours, 0, CLEAR_COLOURS));
    nested!(cpu, 0x0606_e300, sgl::sl_page_nbg0(cpu, CLEAR_NBG0_CELLS, 0, 0x8000));
    let [a, b, c, d] = CLEAR_NBG0_MAPS;
    nested!(cpu, 0x0607_0c4c, sgl::sl_map_nbg0(cpu, a, b, c, d));
    for (map, m) in CLEAR_MAPS.into_iter().zip(&CLEAR_MAP_SET) {
        map_set(cpu, map, m);
    }
    cpu.write32(bg::PLANE_A + bg::X, CLEAR_NBG0_AT[0]);
    cpu.write32(bg::PLANE_A + bg::Y, CLEAR_NBG0_AT[1]);
    cpu.write32(bg::PLANE_A + bg::SPEED_Y, 0);
    cpu.write32(bg::PLANE_A + bg::SPEED_X, 0);
    cpu.write32(bg::PLANE_A + bg::ZOOM_Y, CLEAR_NBG0_ZOOM);
    cpu.write32(bg::PLANE_A + bg::ZOOM_X, CLEAR_NBG0_ZOOM);
    nested!(cpu, 0x0609_f48c, sgl::sl_col_offset_b_on(cpu, 18));
    nested!(cpu, 0x0607_0ddc, sgl::sl_color_calc(cpu, 0x142));
    nested!(cpu, 0x0606_e23c, sgl::sl_bg_on(cpu, 0x2_0015));
    nested!(cpu, 0x0609_f44c, sgl::sl_sprite_type(cpu, 5));
    for (screen, pri) in [(-8i32, 4), (0, 4), (1, 4), (5, 3)] {
        nested!(cpu, 0x0606_98dc, sgl::sl_priority(cpu, screen as u32, pri));
    }
    nested!(cpu, 0x0605_e71c, screen::set_offset_b(cpu, 0, 0, 0));
    cpu.write32(w + GROUND_SPEED, 0xfffe_0000);
    cpu.write32(w + GROUND_ZOOM, CLEAR_NBG0_ZOOM);
    cpu.write8(w + GROUND_STEP, 0);
    cpu.write32(w + END_DOWN, 0x2_0000);
}

/// NBG1's new cells (the stage's first cell list's, 0xb480 bytes to 0x25e65100, 0x800 at a
/// time), page and map (0x2000 bytes of the screen's room to 0x25e76000); its 16 colours (at 0x200)
/// blended halfway (the blend at 0x060a2e1c: 0x060a2dbc to 0x060a2ddc into 0x060a2dfc).
const NBG1_CELLS_VRAM: u32 = 0x25e6_5100;
const NBG1_CELLS_SIZE: u32 = 0xb480;
const NBG1_MAP_VRAM: u32 = 0x25e7_6000;
const NBG1_MAP_SIZE: u32 = 0x2000;
const NBG1_BLEND: u32 = 0x060a_2e1c;
const NBG1_BLENDED: u32 = 0x060a_2dfc;
const NBG1_COLOURS: u32 = 0x200;
const NBG1_COLOUR_COUNT: u32 = 16;
const HALFWAY: u16 = 0x800;
/// The stage's cell lists (states.rs's STAGE_PARTS[CELLS]).
const STAGE_CELLS_LIST: u32 = 0x0607_47fc;
/// How far the first ground map comes before NBG1 is brought in.
const NBG1_IN_AT: i32 = 0x05a0_0000;

/// The clear's ground scroll, a frame: NBG0 at its zoom and speed, both ground maps streamed (the
/// scroll moved on when the first ends); once the first has come 0x5a00000, NBG1 brought in (its
/// cells queued, and once they started its page, map and colours, its plane at its place at full
/// size, the screens shown 0x17) and then turned with the rotation plane; NBG0's frame, and the
/// rotation plane's.
pub fn ground_steps(cpu: &mut Cpu, w: u32) {
    let zoom = cpu.read32(w + GROUND_ZOOM);
    cpu.write32(bg::PLANE_A + bg::ZOOM_Y, zoom);
    cpu.write32(bg::PLANE_A + bg::ZOOM_X, zoom);
    let speed = cpu.read32(w + GROUND_SPEED);
    nested!(cpu, 0x0605_efd4, bg::plane_a_speed(cpu, 0, speed));
    if nested!(cpu, 0x0605_f170, bg::map_stream(cpu, bg::PLANE_A, CLEAR_MAPS[0])) == u32::MAX {
        nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
    }
    nested!(cpu, 0x0605_f170, bg::map_stream(cpu, bg::PLANE_A, CLEAR_MAPS[1]));
    if cpu.read32(CLEAR_MAPS[0] + MAP_AT) as i32 > NBG1_IN_AT {
        match cpu.read8(w + GROUND_STEP) {
            0 => {
                let list = cpu.read32(STAGE_CELLS_LIST);
                let cells = cpu.read32(list).wrapping_add(cpu.read32(STAGE_DATA_AT));
                let queued = nested!(
                    cpu,
                    0x0605_ed2c,
                    screen::queue_long_copy(cpu, cells, NBG1_CELLS_VRAM, NBG1_CELLS_SIZE, 0x800)
                );
                if queued == 0 {
                    nested!(cpu, 0x0606_e2e4, sgl::sl_page_nbg1(cpu, CLEAR_NBG0_CELLS, 0x400, 0xc000));
                    let room = cpu.read32(SCREEN_ROOM);
                    nested!(cpu, 0x0605_ecf0, screen::queue_copy(cpu, room, NBG1_MAP_VRAM, NBG1_MAP_SIZE));
                    cpu.write16(NBG1_BLEND, HALFWAY);
                    nested!(cpu, 0x0600_5ba4, palette::colours_blend(cpu, NBG1_BLEND));
                    nested!(
                        cpu,
                        0x0600_5b18,
                        palette::set_colours(cpu, NBG1_BLENDED, NBG1_COLOURS, NBG1_COLOUR_COUNT)
                    );
                    cpu.write32(bg::PLANE_B + bg::X, 0x0200_0000);
                    cpu.write32(bg::PLANE_B + bg::Y, 0x0200_0000);
                    cpu.write32(bg::PLANE_B + bg::SPEED_Y, 0);
                    cpu.write32(bg::PLANE_B + bg::SPEED_X, 0);
                    cpu.write32(bg::PLANE_B + bg::ZOOM_Y, 0x1_0000);
                    cpu.write32(bg::PLANE_B + bg::ZOOM_X, 0x1_0000);
                    nested!(cpu, 0x0605_f08c, bg::plane_b_frame(cpu));
                    nested!(cpu, 0x0606_e23c, sgl::sl_bg_on(cpu, 0x17));
                    let step = cpu.read8(w + GROUND_STEP).wrapping_add(1);
                    cpu.write8(w + GROUND_STEP, step);
                }
            }
            1 => nested!(cpu, 0x0609_8e30, plane_b_turned(cpu, w)),
            _ => {}
        }
    }
    nested!(cpu, 0x0605_f040, bg::plane_a_frame(cpu));
    nested!(cpu, 0x0609_8d64, clear_plane_frame(cpu, w));
}

/// NBG0's new map (0x2000 bytes of the screen's room from +0x2000, to 0x25e74000: all four of its
/// maps), its plane's place; 128 colours of the stage's fourth list at 16; its new cells (the
/// stage's second cell list's, 0x5100 bytes to 0x25e60000, 0x800 a VBlank).
const NEW_NBG0_MAP: u32 = 0x25e7_4000;
const NEW_NBG0_MAP_FROM: u32 = 0x2000;
const NEW_NBG0_X: u32 = 0x0058_0000;
const NEW_NBG0_COLOURS: (u32, u32) = (16, 0x80);
const NEW_NBG0_CELLS_SIZE: u32 = 0x5100;
/// The darkening's work: its wait (+0x1c, eight frames a step) and colour offset B (+0x10, +0x12,
/// +0x14: words r, g, b).
const DARKEN_WAIT: u32 = 0x1c;
const DARKEN_EVERY: u32 = 8;
const DARKEN_OFFSET: u32 = 0x10;
/// How dark each of r, g and b goes.
const DARKEST: [i16; 3] = [-32, -48, -64];

/// NBG0's new screen: its map queued and set, its plane at its place at full size and its frame;
/// its colours set; the darkening's wait and colour offset B none; NBG0, NBG1, NBG2 and the
/// rotation plane shown (0x10016); its cells queued.
pub fn new_nbg0_start(cpu: &mut Cpu, w: u32) {
    let room = cpu.read32(SCREEN_ROOM) + NEW_NBG0_MAP_FROM;
    nested!(cpu, 0x0605_ecf0, screen::queue_copy(cpu, room, NEW_NBG0_MAP, NBG1_MAP_SIZE));
    let m = NEW_NBG0_MAP;
    nested!(cpu, 0x0607_0c4c, sgl::sl_map_nbg0(cpu, m, m, m, m));
    cpu.write32(bg::PLANE_A + bg::X, NEW_NBG0_X);
    cpu.write32(bg::PLANE_A + bg::Y, 0);
    cpu.write32(bg::PLANE_A + bg::SPEED_Y, 0);
    cpu.write32(bg::PLANE_A + bg::SPEED_X, 0);
    cpu.write32(bg::PLANE_A + bg::ZOOM_Y, 0x1_0000);
    cpu.write32(bg::PLANE_A + bg::ZOOM_X, 0x1_0000);
    nested!(cpu, 0x0605_f040, bg::plane_a_frame(cpu));
    let list = cpu.read32(STAGE_COLOURS_LIST);
    let colours = cpu.read32(list + 12).wrapping_add(cpu.read32(STAGE_DATA_AT));
    let (at, n) = NEW_NBG0_COLOURS;
    nested!(cpu, 0x0600_5b18, palette::set_colours(cpu, colours, at, n));
    cpu.write32(w + DARKEN_WAIT, DARKEN_EVERY);
    for k in 0..3 {
        cpu.write16(w + DARKEN_OFFSET + 2 * k, 0);
    }
    nested!(cpu, 0x0606_e23c, sgl::sl_bg_on(cpu, 0x1_0016));
    let list = cpu.read32(STAGE_CELLS_LIST);
    let cells = cpu.read32(list + 4).wrapping_add(cpu.read32(STAGE_DATA_AT));
    nested!(
        cpu,
        0x0605_ed2c,
        screen::queue_long_copy(cpu, cells, CLEAR_NBG0_CELLS, NEW_NBG0_CELLS_SIZE, 0x800)
    );
}

/// The darkening, a frame: the rotation plane's frame; every eight frames colour offset B a step
/// darker (to -32, -48, -64); NBG1's colours blended on toward the second set (1/512 a frame, to
/// whole); NBG1's speed up to twice the plane's (1/256 a frame more); NBG1 turned.
pub fn darken_steps(cpu: &mut Cpu, w: u32) {
    nested!(cpu, 0x0609_8d64, clear_plane_frame(cpu, w));
    let wait = cpu.read32(w + DARKEN_WAIT).wrapping_sub(1);
    cpu.write32(w + DARKEN_WAIT, wait);
    if wait == 0 {
        cpu.write32(w + DARKEN_WAIT, DARKEN_EVERY);
        for (k, darkest) in DARKEST.into_iter().enumerate() {
            let at = w + DARKEN_OFFSET + 2 * k as u32;
            let v = (cpu.read16(at) as i16).wrapping_sub(1);
            cpu.write16(at, v.max(darkest) as u16);
        }
    }
    let [r, g, b] = [0, 2, 4].map(|k| cpu.read16(w + DARKEN_OFFSET + k) as i16 as u32);
    nested!(cpu, 0x0605_e71c, screen::set_offset_b(cpu, r, g, b));
    let how_far = cpu.read16(NBG1_BLEND);
    if how_far as i16 <= 0xfff {
        cpu.write16(NBG1_BLEND, how_far.wrapping_add(8));
        nested!(cpu, 0x0600_5ba4, palette::colours_blend(cpu, NBG1_BLEND));
        nested!(
            cpu,
            0x0600_5b18,
            palette::set_colours(cpu, NBG1_BLENDED, NBG1_COLOURS, NBG1_COLOUR_COUNT)
        );
    }
    let twice = cpu.read32(w + CLEAR_PLANE_SPEED).wrapping_add(cpu.read32(w + CLEAR_PLANE_SPEED));
    let speed = cpu.read32(w + END_DOWN);
    if (speed as i32) < twice as i32 {
        cpu.write32(w + END_DOWN, speed.wrapping_add(0x100));
    }
    nested!(cpu, 0x0609_8e30, plane_b_turned(cpu, w));
}

/// Both planes scrolled up as the rotation plane moves (NBG0 at half its speed, NBG1 at twice).
pub fn planes_up(cpu: &mut Cpu, w: u32) {
    let speed = cpu.read32(w + CLEAR_PLANE_SPEED) as i32;
    nested!(cpu, 0x0605_efd4, bg::plane_a_speed(cpu, 0, (speed >> 1).wrapping_neg() as u32));
    let speed = cpu.read32(w + CLEAR_PLANE_SPEED);
    nested!(cpu, 0x0605_eff8, bg::plane_b_speed(cpu, 0, speed.wrapping_add(speed).wrapping_neg()));
    nested!(cpu, 0x0605_f040, bg::plane_a_frame(cpu));
    nested!(cpu, 0x0605_f08c, bg::plane_b_frame(cpu));
}

/// The view for the plane's turn: at (0xb00000, 0xb00000, the plane's height turned up) looking
/// down a quarter turn about x; the rotation plane at the origin, still, turned a quarter about x,
/// at scale 0xffff; colour offset B none and off; the scroll's step 0.
const TURN_VIEW_AT: u32 = 0x00b0_0000;
const TURN_VIEW_ANGLE: u32 = 0xc000;
const TURN_PLANE_ANGLE: u16 = 0x4000;
const TURN_PLANE_SCALE: u32 = 0xffff;

pub fn plane_turn_start(cpu: &mut Cpu, w: u32) {
    let height = cpu.read32(SWEEP_PLANE + SWEEP_POSITION + 4).wrapping_neg();
    cpu.write32(END_VIEW + END_VIEW_POSITION, TURN_VIEW_AT);
    cpu.write32(END_VIEW + END_VIEW_POSITION + 4, TURN_VIEW_AT);
    cpu.write32(END_VIEW + END_VIEW_POSITION + 8, height);
    cpu.write32(END_VIEW + END_VIEW_ANGLES, TURN_VIEW_ANGLE);
    cpu.write32(END_VIEW + END_VIEW_ANGLES + 4, 0);
    cpu.write32(END_VIEW + END_VIEW_ANGLES + 8, 0);
    for k in 0..3 {
        cpu.write32(SWEEP_PLANE + SWEEP_POSITION + 4 * k, 0);
        cpu.write32(SWEEP_PLANE + SWEEP_SPEED + 4 * k, 0);
    }
    cpu.write16(SWEEP_PLANE, TURN_PLANE_ANGLE);
    for k in 1..6 {
        cpu.write16(SWEEP_PLANE + 2 * k, 0);
    }
    for k in 0..3 {
        cpu.write32(SWEEP_PLANE + SWEEP_SCALE + 4 * k, TURN_PLANE_SCALE);
    }
    nested!(cpu, 0x0605_e71c, screen::set_offset_b(cpu, 0, 0, 0));
    nested!(cpu, 0x0609_f48c, sgl::sl_col_offset_b_on(cpu, 0));
    cpu.write8(w + END_STEP, 0);
}

/// The plane's turn's colours (80 of the stage's third list, at 0x100).
const TURN_COLOURS: u32 = 80;

/// The view's matrix with the rotation plane's (RBG0's parameters) made from it.
fn view_and_plane(cpu: &mut Cpu) {
    nested!(cpu, 0x0606_71cc, sgl::sl_push_matrix(cpu));
    nested!(cpu, 0x0605_f468, bg::view_matrix(cpu));
    nested!(cpu, 0x0605_f530, bg::rotation_matrix(cpu, SWEEP_PLANE));
    nested!(cpu, 0x0606_7200, sgl::sl_pop_matrix(cpu));
}

/// The view's height less the plane's speed (the view comes down to it): the height left.
fn view_down(cpu: &mut Cpu, w: u32) -> u32 {
    let at = END_VIEW + END_VIEW_POSITION + 8;
    let height = cpu.read32(at).wrapping_sub(cpu.read32(w + CLEAR_PLANE_SPEED));
    cpu.write32(at, height);
    height
}

/// The plane's turn, a frame: its colours set the first frame; both planes up; the view and the
/// plane; the view coming down.
pub fn plane_turn_steps(cpu: &mut Cpu, w: u32) {
    if cpu.read8(w + END_STEP) == 0 {
        let list = cpu.read32(STAGE_COLOURS_LIST);
        let colours = cpu.read32(list + 8).wrapping_add(cpu.read32(STAGE_DATA_AT));
        nested!(cpu, 0x0600_5b18, palette::set_colours(cpu, colours, CLEAR_COLOURS, TURN_COLOURS));
        let step = cpu.read8(w + END_STEP).wrapping_add(1);
        cpu.write8(w + END_STEP, step);
    }
    nested!(cpu, 0x0609_9348, planes_up(cpu, w));
    view_and_plane(cpu);
    view_down(cpu, w);
}

/// The view's height kept within 0x4000000 (and above 0x800000); NBG0's plane at its place at full
/// size; NBG0 (slPriority's 1) at priority 2; NBG0, NBG1, NBG2 and the rotation plane shown; the
/// step 0; the landing's message shown.
const VIEW_HEIGHTS: u32 = 0x03ff_ffff;
const VIEW_LOWEST: i32 = 0x007f_ffff;
const LANDING_MESSAGE: u32 = 0x060a_2d10;

pub fn landing_start(cpu: &mut Cpu, w: u32) {
    let at = END_VIEW + END_VIEW_POSITION + 8;
    let height = cpu.read32(at) & VIEW_HEIGHTS;
    cpu.write32(at, height);
    if height as i32 <= VIEW_LOWEST {
        cpu.write32(at, height.wrapping_add(VIEW_HEIGHTS + 1));
    }
    cpu.write32(bg::PLANE_A + bg::X, NEW_NBG0_X);
    cpu.write32(bg::PLANE_A + bg::Y, 0);
    cpu.write32(bg::PLANE_A + bg::SPEED_Y, 0);
    cpu.write32(bg::PLANE_A + bg::SPEED_X, 0);
    cpu.write32(bg::PLANE_A + bg::ZOOM_Y, 0x1_0000);
    cpu.write32(bg::PLANE_A + bg::ZOOM_X, 0x1_0000);
    nested!(cpu, 0x0606_98dc, sgl::sl_priority(cpu, 1, 2));
    nested!(cpu, 0x0606_e23c, sgl::sl_bg_on(cpu, 0x17));
    cpu.write8(w + END_STEP, 0);
    nested!(cpu, 0x0606_1620, text::message_set(cpu, LANDING_MESSAGE));
}

/// The landing's rotation plane maps (for slMapRA), and the heights at which they change and the
/// scroll moves on.
const LANDING_RA_MAPS: u32 = 0x060a_2e2c;
const LANDING_MAPS_AT: i32 = 0x00ff_ffff;
const LANDED_AT: i32 = 0x0001_ffff;

/// The landing, a frame: both planes up; the view and the plane; the view coming down: the
/// rotation plane's maps changed at 0x1000000, and the scroll moved on at 0x20000.
pub fn landing_steps(cpu: &mut Cpu, w: u32) {
    nested!(cpu, 0x0609_9348, planes_up(cpu, w));
    view_and_plane(cpu);
    let height = view_down(cpu, w) as i32;
    match cpu.read8(w + END_STEP) {
        0 if height <= LANDING_MAPS_AT => {
            nested!(cpu, 0x0609_f344, sgl::sl_map_ra_table(cpu, LANDING_RA_MAPS));
            let step = cpu.read8(w + END_STEP).wrapping_add(1);
            cpu.write8(w + END_STEP, step);
        }
        1 if height <= LANDED_AT => nested!(cpu, 0x0605_e648, screen::scroll_next(cpu)),
        _ => {}
    }
}

/// The models placed about the landing's view (bg.rs's draw_models_near's table).
const LANDING_MODELS: u32 = 0x060a_2ffc;

/// The landing's draw (the screen's hook): the view's matrix, the models near it drawn, and the
/// rotation plane's parameters from it (the plane moved, tilted about x and scaled, inverted).
pub fn landing_draw(cpu: &mut Cpu) {
    nested!(cpu, 0x0606_71cc, sgl::sl_push_matrix(cpu));
    nested!(cpu, 0x0605_f468, bg::view_matrix(cpu));
    nested!(cpu, 0x0605_f6e0, bg::draw_models_near(cpu, LANDING_MODELS));
    tilted_plane(cpu);
}

/// The rotation plane's parameters from the matrix (it moved, tilted about x and scaled, then
/// inverted), and the matrix popped.
fn tilted_plane(cpu: &mut Cpu) {
    let [x, y, z] = [0, 4, 8].map(|k| cpu.read32(SWEEP_PLANE + SWEEP_POSITION + k));
    nested!(cpu, 0x0606_721c, sgl::sl_translate(cpu, x, y, z));
    let tilt = cpu.read16(SWEEP_PLANE) as i16 as u32;
    nested!(cpu, 0x0606_72ec, sgl::sl_rot_x(cpu, tilt));
    let [sx, sy, sz] = [0, 4, 8].map(|k| cpu.read32(SWEEP_PLANE + SWEEP_SCALE + k));
    nested!(cpu, 0x0606_73ec, sgl::sl_scale(cpu, sx, sy, sz));
    nested!(cpu, 0x0606_e4dc, sgl::sl_invers_matrix(cpu));
    nested!(cpu, 0x0606_e50c, sgl::rotation_parameters(cpu));
    nested!(cpu, 0x0606_7200, sgl::sl_pop_matrix(cpu));
}

/// The landing's path (bg.rs's view_along: keys of the view's position and angles); the screen's
/// draw hook (+0x58, screen.rs's HELD_HOOK); kind 35's colour bank (its colour, K_COLOUR, ORed
/// with 0x1000) and its file's models (+0x1c: a list of pointers to each model's first long),
/// the first two put in the landing's placed models (0x060a2f44 and 0x060a2f4c, their model at
/// +4).
const LANDING_PATH: u32 = 0x060a_2e3c;
const SCREEN_HOOK: u32 = 0x58;
const LANDING_KIND: u32 = 35;
const LANDING_COLOUR_BANK: u32 = 0x1000;
const KF_MODELS: u32 = 0x1c;
const LANDING_PLACED: [u32; 2] = [0x060a_2f44, 0x060a_2f4c];
const PLACED_MODEL_AT: u32 = 4;

/// The landing's set-up: the view at its path's start; the step 0; kind 35's models given its
/// colour in bank 0x1000; its first two models placed; the landing's draw made the screen's hook.
pub fn landing_set(cpu: &mut Cpu, w: u32) {
    cpu.write32(END_VIEW, 0);
    nested!(cpu, 0x0605_f5b0, bg::view_along(cpu, LANDING_PATH));
    cpu.write8(w + END_STEP, 0);
    let kind = KINDS + K_SIZE * LANDING_KIND;
    let colour = (cpu.read16(kind + K_COLOUR) | LANDING_COLOUR_BANK as u16) as u32;
    nested!(cpu, 0x0604_5b60, models::models_colour(cpu, LANDING_KIND, colour));
    for (k, placed) in LANDING_PLACED.into_iter().enumerate() {
        let file = cpu.read32(kind);
        let models = cpu.read32(file + KF_MODELS);
        let first = cpu.read32(models + 4 * k as u32);
        let model = cpu.read32(first);
        cpu.write32(placed + PLACED_MODEL_AT, model);
    }
    let hook = cpu.read32(screen::SCREEN) + SCREEN_HOOK;
    cpu.write32(hook, 0x0609_95d4);
}

/// The touchdown, a frame: both planes up; the view along the landing's path (on by 1/256 of the
/// plane's speed) and the landing drawn; its steps: past half the first key, the plane slowed
/// (1/64 a frame, to an eighth, held at half a dot); past four keys, the plane moved 512 dots
/// back; past nine, the scroll moved on.
pub fn touchdown_steps(cpu: &mut Cpu, w: u32) {
    nested!(cpu, 0x0609_9348, planes_up(cpu, w));
    nested!(cpu, 0x0605_f5b0, bg::view_along(cpu, LANDING_PATH));
    nested!(cpu, 0x0609_95d4, landing_draw(cpu));
    let speed = cpu.read32(w + CLEAR_PLANE_SPEED);
    nested!(cpu, 0x0600_8164, clib::ashiftrt_r4::<8>(cpu, speed));
    let place = cpu.read32(END_VIEW).wrapping_add((speed as i32 >> 8) as u32);
    cpu.write32(END_VIEW, place);
    let place = place as i32;
    let next = match cpu.read8(w + END_STEP) {
        0 => place > 0x8000,
        1 => {
            let speed = cpu.read32(w + CLEAR_PLANE_SPEED).wrapping_sub(0x400);
            cpu.write32(w + CLEAR_PLANE_SPEED, speed);
            let slow = speed as i32 <= 0x7fff;
            if slow {
                cpu.write32(w + CLEAR_PLANE_SPEED, 0x8000);
            }
            slow
        }
        2 => {
            let past = place > 0x4_0000;
            if past {
                plane_back(cpu);
            }
            past
        }
        3 => {
            if place > 0x8_ffff {
                nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
            }
            false
        }
        _ => false,
    };
    if next {
        let step = cpu.read8(w + END_STEP).wrapping_add(1);
        cpu.write8(w + END_STEP, step);
    }
}

/// The rotation plane moved 512 dots back along x.
fn plane_back(cpu: &mut Cpu) {
    let x = cpu.read32(SWEEP_PLANE + SWEEP_POSITION).wrapping_add(0xfe00_0000);
    cpu.write32(SWEEP_PLANE + SWEEP_POSITION, x);
}

/// The landing's end: the screen's hook off, the rotation plane moved 512 dots back.
pub fn landing_end(cpu: &mut Cpu) {
    let hook = cpu.read32(screen::SCREEN) + SCREEN_HOOK;
    cpu.write32(hook, 0);
    plane_back(cpu);
}

/// Climbing away, a frame: the view's height less the plane's speed, the rotation plane's
/// parameters from the view; both planes up.
pub fn climb_steps(cpu: &mut Cpu, w: u32) {
    view_down(cpu, w);
    nested!(cpu, 0x0606_71cc, sgl::sl_push_matrix(cpu));
    nested!(cpu, 0x0605_f468, bg::view_matrix(cpu));
    tilted_plane(cpu);
    nested!(cpu, 0x0609_9348, planes_up(cpu, w));
}

/// The blend back (palette.rs's blend: 0x060a2ddc toward 0x060a2dbc, into NBG1_BLENDED) and the
/// fly-off's message.
const BLEND_BACK: u32 = 0x060a_3054;
const FLY_OFF_MESSAGE: u32 = 0x060a_2d20;

/// The fly-off's start: the plane's speed a dot a frame; the blend back from none; its message
/// shown.
pub fn fly_off_start(cpu: &mut Cpu, w: u32) {
    cpu.write32(w + CLEAR_PLANE_SPEED, 0x1_0000);
    cpu.write16(BLEND_BACK, 0);
    nested!(cpu, 0x0606_1620, text::message_set(cpu, FLY_OFF_MESSAGE));
}

/// The fly-off, a frame: the view on along y (0x4ccc a frame); NBG0 zoomed out (96 a frame); the
/// plane slowed by as much (to half a dot); the view's height less its speed, the rotation plane's
/// parameters from the view; NBG0 up at half the plane's speed.
pub fn fly_off(cpu: &mut Cpu, w: u32) {
    let at = END_VIEW + END_VIEW_POSITION + 4;
    let y = cpu.read32(at).wrapping_add(0x4ccc);
    cpu.write32(at, y);
    for zoom in [bg::ZOOM_X, bg::ZOOM_Y] {
        let z = cpu.read32(bg::PLANE_A + zoom).wrapping_add(96);
        cpu.write32(bg::PLANE_A + zoom, z);
    }
    let speed = cpu.read32(w + CLEAR_PLANE_SPEED).wrapping_sub(96);
    cpu.write32(w + CLEAR_PLANE_SPEED, speed);
    if speed as i32 <= 0x7fff {
        cpu.write32(w + CLEAR_PLANE_SPEED, 0x8000);
    }
    view_down(cpu, w);
    nested!(cpu, 0x0606_71cc, sgl::sl_push_matrix(cpu));
    nested!(cpu, 0x0605_f468, bg::view_matrix(cpu));
    tilted_plane(cpu);
    let speed = cpu.read32(w + CLEAR_PLANE_SPEED) as i32;
    nested!(cpu, 0x0605_efd4, bg::plane_a_speed(cpu, 0, (speed >> 1).wrapping_neg() as u32));
    nested!(cpu, 0x0605_f040, bg::plane_a_frame(cpu));
}

/// A plane's zoom-in (an object: its mode +3, its zoom at `at`): from 0x9999 up by 0x83 a frame
/// (while the game is not held) to whole, then done.
const ZOOMER_MODE: u32 = 3;

fn zoom_in(cpu: &mut Cpu, o: u32, at: u32, plane: u32) {
    if cpu.read32(GAME_FLAGS) & HELD != 0 {
        return;
    }
    match cpu.read8(o + ZOOMER_MODE) as i8 {
        0 => {
            cpu.write32(o + at, 0x9999);
            cpu.write8(o + ZOOMER_MODE, 1);
        }
        1 => {}
        112 => {
            cpu.write8(o + ZOOMER_MODE, 115);
            return;
        }
        115 => {
            nested!(cpu, 0x0600_579c, objects::obj_free(cpu, o));
            return;
        }
        _ => return,
    }
    let zoom = cpu.read32(o + at).wrapping_add(0x83);
    cpu.write32(o + at, zoom);
    if zoom as i32 > 0x1_0000 {
        cpu.write32(o + at, 0x1_0000);
        cpu.write8(o + ZOOMER_MODE, 112);
    }
    let zoom = cpu.read32(o + at);
    cpu.write32(plane + bg::ZOOM_Y, zoom);
    cpu.write32(plane + bg::ZOOM_X, zoom);
}

/// NBG1's zoom-in (its zoom at +0x18).
pub fn nbg1_zoomer(cpu: &mut Cpu, o: u32) {
    zoom_in(cpu, o, 0x18, bg::PLANE_B);
}

/// The fly-off with NBG1: NBG1 up at twice the plane's speed; the fly-off; NBG1's colours blended
/// back (1/256 a frame), and once whole the scroll moved on.
pub fn fly_off_steps(cpu: &mut Cpu, w: u32) {
    let speed = cpu.read32(w + CLEAR_PLANE_SPEED);
    nested!(cpu, 0x0605_eff8, bg::plane_b_speed(cpu, 0, speed.wrapping_add(speed).wrapping_neg()));
    nested!(cpu, 0x0605_f08c, bg::plane_b_frame(cpu));
    nested!(cpu, 0x0609_9894, fly_off(cpu, w));
    let how_far = cpu.read16(BLEND_BACK);
    if how_far as i16 > 0xfff {
        nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
    } else {
        cpu.write16(BLEND_BACK, how_far.wrapping_add(16));
        nested!(cpu, 0x0600_5ba4, palette::colours_blend(cpu, BLEND_BACK));
        nested!(
            cpu,
            0x0600_5b18,
            palette::set_colours(cpu, NBG1_BLENDED, NBG1_COLOURS, NBG1_COLOUR_COUNT)
        );
    }
}

/// NBG1's last screen: 256 colours of the stage's sixth list at 0x200; its page (cells at
/// 0x25e68000, 0x400, 0x8000) and its one map (0x25e76000); its cells (the stage's fourth cell
/// list's, 0x4220 bytes, 0x800 a VBlank) queued; NBG0, NBG2 and the rotation plane shown
/// (0x20015).
const LAST_NBG1_CELLS: u32 = 0x25e6_8000;
const LAST_NBG1_CELLS_SIZE: u32 = 0x4220;

pub fn last_nbg1_start(cpu: &mut Cpu) {
    let list = cpu.read32(STAGE_COLOURS_LIST);
    let colours = cpu.read32(list + 20).wrapping_add(cpu.read32(STAGE_DATA_AT));
    nested!(cpu, 0x0600_5b18, palette::set_colours(cpu, colours, NBG1_COLOURS, CLEAR_COLOURS));
    nested!(cpu, 0x0606_e2e4, sgl::sl_page_nbg1(cpu, LAST_NBG1_CELLS, 0x400, 0x8000));
    let m = NBG1_MAP_VRAM;
    nested!(cpu, 0x0607_0d1c, sgl::sl_map_nbg1(cpu, m, m, m, m));
    let list = cpu.read32(STAGE_CELLS_LIST);
    let cells = cpu.read32(list + 12).wrapping_add(cpu.read32(STAGE_DATA_AT));
    nested!(
        cpu,
        0x0605_ed2c,
        screen::queue_long_copy(cpu, cells, LAST_NBG1_CELLS, LAST_NBG1_CELLS_SIZE, 0x800)
    );
    nested!(cpu, 0x0606_e23c, sgl::sl_bg_on(cpu, 0x2_0015));
}

/// The fly-off's last frame: the fly-off, and the scroll moved on.
pub fn fly_off_end(cpu: &mut Cpu, w: u32) {
    nested!(cpu, 0x0609_9894, fly_off(cpu, w));
    nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
}

/// NBG1's ground map for the fly-off (as CLEAR_MAPS) and its empty cell; NBG1's place;
/// the fly-off's work's NBG1 speed (+0x1c); the fly-off ends when the map has come 0x3000000.
const FLY_MAP: u32 = 0x060a_4890;
const FLY_MAP_SET: MapSet = MapSet {
    blocks: 7,
    table: 0x060a_2ab4,
    cells: NBG1_MAP_VRAM,
    at: 0x01c4_0000,
    mask: 0x1fff,
    ahead: 0x90,
    shapes: 0,
};
const MAP_EMPTY: u32 = 0x18;
const EMPTY_CELLS: u32 = 0x0400_0400;
const FLY_NBG1_AT: [u32; 2] = [0x00b0_0000, 0x0070_0000];
const FLY_NBG1_SPEED: u32 = 0x1c;
const FLY_END_AT: i32 = 0x0300_0000;

/// NBG1's ground for the fly-off: its map set out and its plane cleared; NBG1 at its place, its
/// frame; NBG1 moving up two dots a frame; NBG0, NBG1, NBG2 and the rotation plane shown; colour
/// calculation 0x140; NBG1's zoom made.
pub fn fly_ground_start(cpu: &mut Cpu, w: u32) {
    map_set(cpu, FLY_MAP, &FLY_MAP_SET);
    cpu.write32(FLY_MAP + MAP_EMPTY, EMPTY_CELLS);
    nested!(cpu, 0x0605_f14c, bg::map_clear(cpu, FLY_MAP));
    cpu.write32(bg::PLANE_B + bg::X, FLY_NBG1_AT[0]);
    cpu.write32(bg::PLANE_B + bg::Y, FLY_NBG1_AT[1]);
    cpu.write32(bg::PLANE_B + bg::SPEED_Y, 0);
    cpu.write32(bg::PLANE_B + bg::SPEED_X, 0);
    nested!(cpu, 0x0605_f08c, bg::plane_b_frame(cpu));
    cpu.write32(w + FLY_NBG1_SPEED, 0xfffe_0000);
    nested!(cpu, 0x0606_e23c, sgl::sl_bg_on(cpu, 0x17));
    nested!(cpu, 0x0607_0ddc, sgl::sl_color_calc(cpu, 0x140));
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, SWEEP_SIZE)) as i8 as i32 as u32;
    nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 0, blocks, 0x0609_9964));
}

/// The fly-off over NBG1's ground, a frame: NBG1 at its speed, its map streamed, its frame; the
/// fly-off; the scroll moved on once the map has come 0x3000000.
pub fn fly_ground_steps(cpu: &mut Cpu, w: u32) {
    let speed = cpu.read32(w + FLY_NBG1_SPEED);
    nested!(cpu, 0x0605_eff8, bg::plane_b_speed(cpu, 0, speed));
    nested!(cpu, 0x0605_f170, bg::map_stream(cpu, bg::PLANE_B, FLY_MAP));
    nested!(cpu, 0x0605_f08c, bg::plane_b_frame(cpu));
    nested!(cpu, 0x0609_9894, fly_off(cpu, w));
    if cpu.read32(FLY_MAP + MAP_AT) as i32 > FLY_END_AT {
        nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
    }
}

/// The last ground's map (in NBG0's plane: CLEAR_MAPS's fields) and the models placed along it
/// (bg.rs's draw_models_near's table).
const LAST_MAP: u32 = 0x060a_4870;
const LAST_MODELS: u32 = 0x060a_3338;

/// The last ground's draw (the screen's hook): the view's place the map's (in 512-line keys), the
/// models near it drawn moved down by the map's place.
pub fn last_ground_draw(cpu: &mut Cpu) {
    let at = cpu.read32(LAST_MAP + MAP_AT);
    nested!(cpu, 0x0600_8162, clib::ashiftrt_r4::<9>(cpu, at));
    cpu.write32(END_VIEW, (at as i32 >> 9) as u32);
    nested!(cpu, 0x0606_71cc, sgl::sl_push_matrix(cpu));
    let at = cpu.read32(LAST_MAP + MAP_AT);
    nested!(cpu, 0x0606_721c, sgl::sl_translate(cpu, 0, at, 0));
    nested!(cpu, 0x0605_f6e0, bg::draw_models_near(cpu, LAST_MODELS));
    nested!(cpu, 0x0606_7200, sgl::sl_pop_matrix(cpu));
}

/// The last ground: NBG0's page (cells at 0x25e60000, 0x8000) and its one map (0x25e74000); 64
/// colours of the stage's fifth list at 0; its map (eight blocks of 0x060a2ad0's, with shapes),
/// NBG0's place, its cells (the stage's third cell list's, 0x2ac0 bytes, 0x2000 a VBlank); kind
/// 36's first two models placed along it (0x060a3238 and 0x060a3240); RBG0's page (0x25e00000,
/// 512 colours, 0x8000) and maps (0x060a3228's); the rotation plane 176 dots back; 96 colours of
/// the stage's seventh list at 0x100.
const LAST_MAP_SET: MapSet = MapSet {
    blocks: 8,
    table: 0x060a_2ad0,
    cells: NEW_NBG0_MAP,
    at: 0x00e0_0000,
    mask: 0x1fff,
    ahead: 0x90,
    shapes: 0x060a_321c,
};
const LAST_NBG0_AT: [u32; 2] = [0x00b0_0000, 0x0190_0000];
const LAST_NBG0_COLOURS: u32 = 64;
const LAST_NBG0_CELLS_SIZE: u32 = 0x2ac0;
const LAST_KIND: u32 = 36;
const LAST_PLACED: [u32; 2] = [0x060a_3238, 0x060a_3240];
const LAST_RA_MAPS: u32 = 0x060a_3228;
const LAST_PLANE_X: u32 = 0xff50_0000;
const LAST_PLANE_COLOURS: u32 = 96;

/// The last ground's start: as above; the last ground's draw the screen's hook; NBG0, NBG1, NBG2
/// and the rotation plane shown; NBG0 (slPriority's 1) at priority 5, NBG1 4, RBG0 3.
pub fn last_ground_start(cpu: &mut Cpu) {
    nested!(cpu, 0x0606_e300, sgl::sl_page_nbg0(cpu, CLEAR_NBG0_CELLS, 0, 0x8000));
    let m = NEW_NBG0_MAP;
    nested!(cpu, 0x0607_0c4c, sgl::sl_map_nbg0(cpu, m, m, m, m));
    let list = cpu.read32(STAGE_COLOURS_LIST);
    let colours = cpu.read32(list + 16).wrapping_add(cpu.read32(STAGE_DATA_AT));
    nested!(cpu, 0x0600_5b18, palette::set_colours(cpu, colours, 0, LAST_NBG0_COLOURS));
    map_set(cpu, LAST_MAP, &LAST_MAP_SET);
    cpu.write32(LAST_MAP + MAP_EMPTY, 0);
    nested!(cpu, 0x0605_f14c, bg::map_clear(cpu, LAST_MAP));
    cpu.write32(bg::PLANE_A + bg::X, LAST_NBG0_AT[0]);
    cpu.write32(bg::PLANE_A + bg::Y, LAST_NBG0_AT[1]);
    cpu.write32(bg::PLANE_A + bg::SPEED_Y, 0);
    cpu.write32(bg::PLANE_A + bg::SPEED_X, 0);
    cpu.write32(bg::PLANE_A + bg::ZOOM_Y, 0x1_0000);
    cpu.write32(bg::PLANE_A + bg::ZOOM_X, 0x1_0000);
    let list = cpu.read32(STAGE_CELLS_LIST);
    let cells = cpu.read32(list + 8).wrapping_add(cpu.read32(STAGE_DATA_AT));
    nested!(
        cpu,
        0x0605_ed2c,
        screen::queue_long_copy(cpu, cells, CLEAR_NBG0_CELLS, LAST_NBG0_CELLS_SIZE, 0x2000)
    );
    let file = cpu.read32(KINDS + K_SIZE * LAST_KIND);
    for (k, placed) in LAST_PLACED.into_iter().enumerate() {
        let models = cpu.read32(file + KF_MODELS);
        let first = cpu.read32(models + 4 * k as u32);
        let model = cpu.read32(first);
        cpu.write32(placed + PLACED_MODEL_AT, model);
    }
    let hook = cpu.read32(screen::SCREEN) + SCREEN_HOOK;
    cpu.write32(hook, 0x0609_9c6c);
    nested!(cpu, 0x0606_e2ce, sgl::sl_page_rbg0(cpu, 0x25e0_0000, 0x200, 0x8000));
    nested!(cpu, 0x0609_f344, sgl::sl_map_ra_table(cpu, LAST_RA_MAPS));
    cpu.write32(SWEEP_PLANE + SWEEP_POSITION, LAST_PLANE_X);
    for k in 1..3 {
        cpu.write32(SWEEP_PLANE + SWEEP_POSITION + 4 * k, 0);
    }
    for k in 0..3 {
        cpu.write32(SWEEP_PLANE + SWEEP_SPEED + 4 * k, 0);
    }
    for k in 0..6 {
        cpu.write16(SWEEP_PLANE + 2 * k, 0);
    }
    for k in 0..3 {
        cpu.write32(SWEEP_PLANE + SWEEP_SCALE + 4 * k, 0x1_0000);
    }
    let list = cpu.read32(STAGE_COLOURS_LIST);
    let colours = cpu.read32(list + 24).wrapping_add(cpu.read32(STAGE_DATA_AT));
    nested!(cpu, 0x0600_5b18, palette::set_colours(cpu, colours, CLEAR_COLOURS, LAST_PLANE_COLOURS));
    nested!(cpu, 0x0606_e23c, sgl::sl_bg_on(cpu, 0x17));
    for (screen, pri) in [(1, 5), (0, 4), (5, 3)] {
        nested!(cpu, 0x0606_98dc, sgl::sl_priority(cpu, screen, pri));
    }
}

/// The rotation plane down with the last ground (by half NBG1's speed), its parameters from the
/// matrix moved to it, inverted.
pub fn last_plane_frame(cpu: &mut Cpu, w: u32) {
    let speed = cpu.read32(w + FLY_NBG1_SPEED) as i32;
    let at = SWEEP_PLANE + SWEEP_POSITION + 4;
    let y = cpu.read32(at).wrapping_sub((speed >> 1) as u32);
    cpu.write32(at, y);
    nested!(cpu, 0x0606_71cc, sgl::sl_push_matrix(cpu));
    let [x, y, z] = [0, 4, 8].map(|k| cpu.read32(SWEEP_PLANE + SWEEP_POSITION + k));
    nested!(cpu, 0x0606_721c, sgl::sl_translate(cpu, x, y, z));
    nested!(cpu, 0x0606_e4dc, sgl::sl_invers_matrix(cpu));
    nested!(cpu, 0x0606_e50c, sgl::rotation_parameters(cpu));
    nested!(cpu, 0x0606_7200, sgl::sl_pop_matrix(cpu));
}

/// NBG1 at its speed, its ground map streamed, its frame: the map's end (map_stream's).
pub fn last_nbg1(cpu: &mut Cpu, w: u32) -> u32 {
    let speed = cpu.read32(w + FLY_NBG1_SPEED);
    nested!(cpu, 0x0605_eff8, bg::plane_b_speed(cpu, 0, speed));
    let end = nested!(cpu, 0x0605_f170, bg::map_stream(cpu, bg::PLANE_B, FLY_MAP));
    nested!(cpu, 0x0605_f08c, bg::plane_b_frame(cpu));
    end
}

/// NBG0 at twice NBG1's speed, the last ground's map streamed, its frame; the ground hit by
/// points (the map scrolled by NBG0's place, 176 dots back): the map's end.
pub fn last_nbg0(cpu: &mut Cpu, w: u32) -> u32 {
    let speed = cpu.read32(w + FLY_NBG1_SPEED);
    nested!(cpu, 0x0605_efd4, bg::plane_a_speed(cpu, 0, speed.wrapping_add(speed)));
    let end = nested!(cpu, 0x0605_f170, bg::map_stream(cpu, bg::PLANE_A, LAST_MAP));
    nested!(cpu, 0x0605_f040, bg::plane_a_frame(cpu));
    let x = cpu.read32(bg::PLANE_A + bg::X).wrapping_add(LAST_PLANE_X);
    nested!(cpu, 0x0605_ef58, bg::map_hit_on(cpu, x, LAST_MAP));
    end
}

/// The last ground, a frame: the rotation plane, both grounds and the models along the last; the
/// scroll's speed by the map's place (past 0xa000000 sped up 1/128 a frame to two dots a frame;
/// else eased off as much to a quarter dot past 0x3e00000, a dot before); the scroll moved on past
/// 0xc000000.
pub fn last_ground_steps(cpu: &mut Cpu, w: u32) {
    nested!(cpu, 0x0609_9eb0, last_plane_frame(cpu, w));
    nested!(cpu, 0x0609_9f48, last_nbg0(cpu, w));
    nested!(cpu, 0x0609_9f08, last_nbg1(cpu, w));
    nested!(cpu, 0x0609_9c6c, last_ground_draw(cpu));
    let at = cpu.read32(LAST_MAP + MAP_AT) as i32;
    let speed = cpu.read32(w + FLY_NBG1_SPEED) as i32;
    if at > 0x0a00_0000 {
        let speed = speed - 0x200;
        cpu.write32(w + FLY_NBG1_SPEED, speed.max(-0x2_0000) as u32);
    } else {
        let slowest = if at > 0x03e0_0000 { -0x4000 } else { -0x1_0000 };
        let speed = speed + 0x200;
        cpu.write32(w + FLY_NBG1_SPEED, speed.min(slowest) as u32);
    }
    if cpu.read32(LAST_MAP + MAP_AT) as i32 > 0x0c00_0000 {
        nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
    }
}

/// The screen's draw hook off.
pub fn hook_off(cpu: &mut Cpu) {
    let hook = cpu.read32(screen::SCREEN) + SCREEN_HOOK;
    cpu.write32(hook, 0);
}

/// NBG1's speed sped up 1/128 a frame to `fastest`.
fn speed_up(cpu: &mut Cpu, w: u32, fastest: i32) {
    let speed = cpu.read32(w + FLY_NBG1_SPEED) as i32;
    if speed > fastest {
        cpu.write32(w + FLY_NBG1_SPEED, (speed - 0x200).max(fastest) as u32);
    }
}

/// A ground map taken back four blocks once past `past` (its place and how far it is loaded).
fn map_back(cpu: &mut Cpu, map: u32, past: i32) {
    let at = cpu.read32(map + MAP_AT);
    if at as i32 > past {
        let loaded = cpu.read32(map + MAP_LOADED);
        cpu.write32(map + MAP_AT, at.wrapping_add(0xfe00_0000));
        cpu.write32(map + MAP_LOADED, loaded.wrapping_add(0xfe00_0000));
    }
}

/// Both grounds on, round and round: sped up to two dots a frame; the rotation plane and both
/// grounds; the last ground's map taken back past 0xe000000, NBG1's past 0xa000000.
pub fn grounds_looping(cpu: &mut Cpu, w: u32) {
    speed_up(cpu, w, -0x2_0000);
    nested!(cpu, 0x0609_9eb0, last_plane_frame(cpu, w));
    nested!(cpu, 0x0609_9f48, last_nbg0(cpu, w));
    nested!(cpu, 0x0609_9f08, last_nbg1(cpu, w));
    map_back(cpu, LAST_MAP, 0x0e00_0000);
    map_back(cpu, FLY_MAP, 0x0a00_0000);
}

/// The scroll's step that does nothing.
pub fn ground_idle(_cpu: &mut Cpu) {}

/// The grounds faster still: sped up to three dots a frame; the rotation plane and both grounds;
/// the last ground's map taken back past 0xe000000.
pub fn grounds_faster(cpu: &mut Cpu, w: u32) {
    speed_up(cpu, w, -0x3_0000);
    nested!(cpu, 0x0609_9eb0, last_plane_frame(cpu, w));
    nested!(cpu, 0x0609_9f48, last_nbg0(cpu, w));
    nested!(cpu, 0x0609_9f08, last_nbg1(cpu, w));
    map_back(cpu, LAST_MAP, 0x0e00_0000);
}

/// The grounds changed: NBG1's map (six blocks of 0x060a2af0's) set out and its plane cleared,
/// NBG1 at 0x1540000 down; the last ground's map changed (four blocks of 0x060a2b08's, as far on
/// in its first block as it was ahead); the step 0; the message shown.
const SEA_MAP_SET: MapSet = MapSet {
    blocks: 6,
    table: 0x060a_2af0,
    cells: NBG1_MAP_VRAM,
    at: 0x00e0_0000,
    mask: 0x1fff,
    ahead: 0x90,
    shapes: 0,
};
const SEA_NBG1_Y: u32 = 0x0154_0000;
const SHORE_MAP_BLOCKS: u8 = 4;
const SHORE_MAP_TABLE: u32 = 0x060a_2b08;
const SHORE_MESSAGE: u32 = 0x060a_2d30;

pub fn grounds_change(cpu: &mut Cpu, w: u32) {
    map_set(cpu, FLY_MAP, &SEA_MAP_SET);
    cpu.write32(FLY_MAP + MAP_EMPTY, EMPTY_CELLS);
    nested!(cpu, 0x0605_f14c, bg::map_clear(cpu, FLY_MAP));
    cpu.write32(bg::PLANE_B + bg::Y, SEA_NBG1_Y);
    cpu.write8(LAST_MAP + MAP_BLOCKS, SHORE_MAP_BLOCKS);
    cpu.write8(LAST_MAP + MAP_LOADED_ALL, 0);
    cpu.write32(LAST_MAP + MAP_BLOCK_TABLE, SHORE_MAP_TABLE);
    cpu.write32(LAST_MAP + MAP_PLANE_CELLS, NEW_NBG0_MAP);
    let ahead = cpu.read32(LAST_MAP + MAP_LOADED).wrapping_sub(cpu.read32(LAST_MAP + MAP_AT));
    cpu.write32(LAST_MAP + MAP_AT, MAP_LOADED_FROM.wrapping_sub(ahead));
    cpu.write32(LAST_MAP + MAP_LOADED, MAP_LOADED_FROM);
    cpu.write8(w + END_STEP, 0);
    nested!(cpu, 0x0606_1620, text::message_set(cpu, SHORE_MESSAGE));
}

/// The grounds on until NBG1's map has come 0x9000000, then the scroll moved on.
pub fn grounds_to_shore(cpu: &mut Cpu, w: u32) {
    nested!(cpu, 0x0609_9eb0, last_plane_frame(cpu, w));
    nested!(cpu, 0x0609_9f48, last_nbg0(cpu, w));
    nested!(cpu, 0x0609_9f08, last_nbg1(cpu, w));
    if cpu.read32(FLY_MAP + MAP_AT) as i32 > 0x0900_0000 {
        nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
    }
}

/// A target (an unseen actor, its own draw nothing): its hits on while its word at +0x6c is set
/// (its long there), off while not; its place in the targets' table (+0x70).
const TARGET_ON: u32 = 0x6c;
const TARGET_SLOT: u32 = 0x70;

pub fn target_behaviour(cpu: &mut Cpu, o: u32) {
    let flags = cpu.read32(o + FLAGS);
    let flags = if cpu.read32(o + TARGET_ON) != 0 { flags & !NO_HITS } else { flags | NO_HITS };
    cpu.write32(o + FLAGS, flags);
}

/// A target's draw: nothing.
pub fn target_draw(_cpu: &mut Cpu, _o: u32) {}

/// A target set up: its sprite and hits; unturned; its behaviour and draw (a model drawn by its own
/// function, not to be hurt); number 0, side 3; its boxes (0x060a3448), its life 0x1000; off.
const TARGET_TURN: u32 = 0x1c;
const TARGET_BEHAVIOUR: u32 = 0x0609_a288;
const TARGET_DRAW: u32 = 0x0609_a2b4;
const OWN_DRAW: u32 = 0x40_0000;
const TARGET_SIDE: u8 = 3;
const TARGET_BOXES: u32 = 0x060a_3448;
const TARGET_LIFE: u32 = 0x1000;

pub fn target_init(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HIT_BOXES));
    cpu.write32(o + TARGET_TURN, 0);
    cpu.write32(o + BEHAVIOUR, TARGET_BEHAVIOUR);
    let flags = cpu.read32(o + FLAGS) | IS_MODEL | OWN_DRAW | UNHURT;
    cpu.write32(o + FLAGS, flags);
    cpu.write32(o + MODEL, TARGET_DRAW);
    cpu.write8(o + NUMBER, 0);
    cpu.write8(o + HIT_SIDE, TARGET_SIDE);
    cpu.write32(o + HIT_BOXES, TARGET_BOXES);
    cpu.write32(o + HIT_LIFE, TARGET_LIFE);
    cpu.write32(o + TARGET_ON, 0);
}

/// The targets: two tables of five (20 bytes each, the target first), the second made only when
/// the work's difficulty (+3, the setting's copy) is above the easiest (else emptied). Each made (list 2, 0x74 bytes, actor_exec's), set up and
/// given its entry. Done (the last entry's offset, 80, left as the result).
const TARGET_TABLES: [u32; 2] = [0x060a_3380, 0x060a_33e4];
const TARGET_ENTRY: u32 = 20;
const TARGETS: u32 = 5;
const TARGET_SIZE: u32 = 0x74;

pub fn targets_set(cpu: &mut Cpu, e: u32) -> u32 {
    let both = cpu.read8(e + TARGETS_DIFFICULTY) != 0;
    for (n, table) in TARGET_TABLES.into_iter().enumerate() {
        if n == 1 && !both {
            for i in 0..TARGETS {
                cpu.write32(table + TARGET_ENTRY * i, 0);
            }
            break;
        }
        for i in 0..TARGETS {
            let entry = table + TARGET_ENTRY * i;
            let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, TARGET_SIZE)) as i8 as i32 as u32;
            let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
            if o != 0 {
                nested!(cpu, 0x0609_a2c0, target_init(cpu, o));
                cpu.write32(o + TARGET_SLOT, entry);
            }
            cpu.write32(entry, o);
        }
    }
    TARGET_ENTRY * (TARGETS - 1)
}

/// The targets' frame: the second rotation scroll's (RBG1's: its parameters at 0x06074770, as
/// MOVER's) plane; a target's polygon (kind 37's first model), the point checked on screen (and
/// its size), and the point the view's centre on screen is kept at (0x060a3464 to 0x060a48b0); a
/// target's entry: its target, its place (x, y, z) and its turn about z (a word at +0x10).
const TARGET_PLANE_B: u32 = 0x0607_4770;
const TARGET_KIND: u32 = 37;
const TARGET_POINT: u32 = 0x060a_3458;
const TARGET_POINT_SIZE: u32 = 0x0100_0000;
const VIEW_CENTRE: u32 = 0x060a_3464;
const VIEW_CENTRE_ON_SCREEN: u32 = 0x060a_48b0;
const ENTRY_AT: u32 = 4;
const ENTRY_TURN: u32 = 0x10;
/// Where a target's point goes on screen (its +0x14).
const TARGET_ON_SCREEN: u32 = 0x14;

/// The targets' frame: in window 0, the view's centre on screen and the view's matrix kept; RBG0's
/// parameters from the rotation plane, and the first table's targets placed on it (turned with it
/// about z); RBG1's from the second plane, and above the easiest the second table's on it; window
/// 1.
pub fn targets_frame(cpu: &mut Cpu, w: u32) {
    nested!(cpu, 0x0606_713c, sgl::sl_cur_window(cpu, 0));
    nested!(cpu, 0x0606_71cc, sgl::sl_push_matrix(cpu));
    nested!(cpu, 0x0605_f4d0, bg::view_matrix_moved_first(cpu));
    nested!(cpu, 0x0606_76fc, sgl::sl_convert_pixels(cpu, VIEW_CENTRE, VIEW_CENTRE_ON_SCREEN));
    let matrix = cpu.read32(screen::SCREEN) + SCREEN_MATRIX;
    nested!(cpu, 0x0606_db9c, sgl::sl_get_matrix(cpu, matrix));
    nested!(cpu, 0x0609_f43c, sgl::sl_cur_rpara(cpu, 0));
    nested!(cpu, 0x0605_f530, bg::rotation_matrix(cpu, SWEEP_PLANE));
    targets_on(cpu, SWEEP_PLANE, TARGET_TABLES[0]);
    nested!(cpu, 0x0609_f43c, sgl::sl_cur_rpara(cpu, 1));
    // the SH-2 code holds the plane and the first table's loop's registers here (its count,
    // sl_pop_matrix, the table, the point): the matrix's callees push them, and a hatch's set-up
    // reads the point back as a field its caller never set
    let held = [
        (8, TARGET_PLANE_B),
        (9, TARGET_ENTRY * TARGETS),
        (10, 0x0606_7200),
        (11, TARGETS),
        (12, TARGET_TABLES[0]),
        (13, TARGET_POINT),
    ];
    cpu.holding(&held, |cpu| nested!(cpu, 0x0605_f530, bg::rotation_matrix(cpu, TARGET_PLANE_B)));
    if cpu.read8(w + TARGETS_DIFFICULTY) != 0 {
        targets_on(cpu, TARGET_PLANE_B, TARGET_TABLES[1]);
    }
    nested!(cpu, 0x0606_7200, sgl::sl_pop_matrix(cpu));
    nested!(cpu, 0x0606_713c, sgl::sl_cur_window(cpu, 1));
}

/// A table's targets on `plane` (turned with it about z): each placed and turned, on (its hits) if
/// its point is on screen, its polygon drawn and the point's place on screen kept; else off.
fn targets_on(cpu: &mut Cpu, plane: u32, table: u32) {
    nested!(cpu, 0x0606_71cc, sgl::sl_push_matrix(cpu));
    let turn = cpu.read16(plane + 4) as i16 as u32;
    nested!(cpu, 0x0606_726c, sgl::sl_rot_z(cpu, turn));
    for i in 0..TARGETS {
        let entry = table + TARGET_ENTRY * i;
        nested!(cpu, 0x0606_71cc, sgl::sl_push_matrix(cpu));
        let [x, y, z] = [0, 4, 8].map(|k| cpu.read32(entry + ENTRY_AT + k));
        nested!(cpu, 0x0606_721c, sgl::sl_translate(cpu, x, y, z));
        let turn = cpu.read16(entry + ENTRY_TURN) as i16 as u32;
        nested!(cpu, 0x0606_726c, sgl::sl_rot_z(cpu, turn));
        let seen = nested!(cpu, 0x0606_e9fc, sgl::sl_check_on_screen(cpu, TARGET_POINT, TARGET_POINT_SIZE));
        let target = cpu.read32(entry);
        if (seen as i32) >= 0 {
            cpu.write32(target + TARGET_ON, 1);
            let file = cpu.read32(KINDS + K_SIZE * TARGET_KIND);
            let models = cpu.read32(file + KF_MODELS);
            let first = cpu.read32(models);
            let model = cpu.read32(first);
            nested!(cpu, 0x0606_745c, sgl::sl_put_polygon(cpu, model));
            let target = cpu.read32(entry);
            if target != 0 {
                nested!(cpu, 0x0606_750c, sgl::sl_convert(cpu, TARGET_POINT, target + TARGET_ON_SCREEN));
            }
        } else {
            cpu.write32(target + TARGET_ON, 0);
        }
        nested!(cpu, 0x0606_7200, sgl::sl_pop_matrix(cpu));
    }
    nested!(cpu, 0x0606_7200, sgl::sl_pop_matrix(cpu));
}

/// The targets' screens: the perspective (0x2000) in window 0; RBG0's page (0x25e00000, 512
/// colours, 0xc000); both rotation planes' maps (0x060a3470's), RBG1's plane and over modes, its
/// coefficient table (0x25e20000, mode 97); rotation parameter mode 3; window 0 over the screen
/// (352 by 224) with its line table (0xa5e3fa80); the windows' control at 7 3; the planes' cells
/// (the stage's seventh cell list's, 0x10a80 bytes to 0x25e08000, 0x2000 a VBlank); 112 colours of
/// the stage's tenth list at 0x100.
const TARGETS_RA_MAPS: u32 = 0x060a_3470;
const TARGETS_KTABLE: u32 = 0x25e2_0000;
const TARGETS_KTABLE_MODE: u32 = 97;
const TARGETS_LINE_WINDOW: u32 = 0xa5e3_fa80;
const TARGETS_CELLS_VRAM: u32 = 0x25e0_8000;
const TARGETS_CELLS_SIZE: u32 = 0x1_0a80;
const TARGETS_COLOURS: u32 = 112;
/// The view for the targets: at (0, 0xa00000, -0x3a70000), turned half about y.
const TARGETS_VIEW_AT: [u32; 3] = [0, 0x00a0_0000, 0xfc59_0000];
const TARGETS_VIEW_TURN: u32 = 0x8000;
/// The work: its step (+0, a byte) and two more bytes, its copy of the difficulty (+3); its
/// words at +0x10 to +0x16, and +0x20 to +0x2c (the plane's speed -1/4 at +0x28).
const TARGETS_FIRST_STEP: u8 = 5;
const TARGETS_DIFFICULTY: u32 = 3;
/// Kind 37's colour, one on (0x060a4810), and its models' colour in bank 0x1000.
const TARGET_COLOUR: u32 = 0x060a_4810;

/// The targets' start: their screens and view; both rotation planes at the origin, still,
/// unturned (the second's angles alone, not their speeds) and unscaled; the work set out; kind 37's models shaded by one light and coloured;
/// sprite type 5, the sprites at -8 (slPriority's) at priority 3; both target tables emptied; the
/// targets' frame the screen's hook.
pub fn targets_start(cpu: &mut Cpu, w: u32) {
    nested!(cpu, 0x0606_713c, sgl::sl_cur_window(cpu, 0));
    nested!(cpu, 0x0606_e1cc, sgl::sl_perspective(cpu, 0x2000));
    nested!(cpu, 0x0606_713c, sgl::sl_cur_window(cpu, 1));
    nested!(cpu, 0x0606_e2ce, sgl::sl_page_rbg0(cpu, 0x25e0_0000, 0x200, 0xc000));
    nested!(cpu, 0x0609_f344, sgl::sl_map_ra_table(cpu, TARGETS_RA_MAPS));
    nested!(cpu, 0x0609_f49c, sgl::sl_plane_rb(cpu, 0));
    nested!(cpu, 0x0609_f32c, sgl::sl_map_rb_table(cpu, TARGETS_RA_MAPS));
    nested!(cpu, 0x0609_f4ac, sgl::sl_over_rb(cpu, 0));
    nested!(cpu, 0x0609_f4bc, sgl::sl_ktable_rb(cpu, TARGETS_KTABLE, TARGETS_KTABLE_MODE));
    nested!(cpu, 0x0609_f42c, sgl::sl_rpara_mode(cpu, 3));
    nested!(cpu, 0x0609_f51c, sgl::sl_scr_window0(cpu, 0, 0, 0x160, 0xe0));
    nested!(cpu, 0x0606_e17c, sgl::sl_window_control(cpu, 7, 3));
    nested!(cpu, 0x0609_f53c, sgl::sl_line_window0(cpu, TARGETS_LINE_WINDOW));
    let list = cpu.read32(STAGE_CELLS_LIST);
    let cells = cpu.read32(list + 24).wrapping_add(cpu.read32(STAGE_DATA_AT));
    nested!(
        cpu,
        0x0605_ed2c,
        screen::queue_long_copy(cpu, cells, TARGETS_CELLS_VRAM, TARGETS_CELLS_SIZE, 0x2000)
    );
    let list = cpu.read32(STAGE_COLOURS_LIST);
    let colours = cpu.read32(list + 36).wrapping_add(cpu.read32(STAGE_DATA_AT));
    nested!(cpu, 0x0600_5b18, palette::set_colours(cpu, colours, CLEAR_COLOURS, TARGETS_COLOURS));
    nested!(cpu, 0x0606_e23c, sgl::sl_bg_on(cpu, 0x1_0016));
    for (k, v) in TARGETS_VIEW_AT.into_iter().enumerate() {
        cpu.write32(END_VIEW + END_VIEW_POSITION + 4 * k as u32, v);
    }
    cpu.write32(END_VIEW + END_VIEW_ANGLES, 0);
    cpu.write32(END_VIEW + END_VIEW_ANGLES + 4, TARGETS_VIEW_TURN);
    cpu.write32(END_VIEW + END_VIEW_ANGLES + 8, 0);
    for (plane, words) in [(SWEEP_PLANE, 6), (TARGET_PLANE_B, 3)] {
        for k in 0..3 {
            cpu.write32(plane + SWEEP_POSITION + 4 * k, 0);
            cpu.write32(plane + SWEEP_SPEED + 4 * k, 0);
        }
        for k in 0..words {
            cpu.write16(plane + 2 * k, 0);
        }
        for k in 0..3 {
            cpu.write32(plane + SWEEP_SCALE + 4 * k, 0x1_0000);
        }
    }
    cpu.write8(w + END_STEP, TARGETS_FIRST_STEP);
    cpu.write8(w + 1, 0);
    cpu.write8(w + 2, 0);
    let difficulty = cpu.read8(DIFFICULTY);
    cpu.write8(w + TARGETS_DIFFICULTY, difficulty);
    for k in 0..4 {
        cpu.write16(w + 0x10 + 2 * k, 0);
    }
    cpu.write32(w + 0x20, 0);
    cpu.write32(w + 0x24, 0);
    cpu.write32(w + CLEAR_PLANE_SPEED, 0xffff_c000);
    cpu.write32(w + 0x2c, 0);
    nested!(cpu, 0x0606_2f74, gouraud::models_hss(cpu, TARGET_KIND, 1));
    let colour = cpu.read16(KINDS + K_SIZE * TARGET_KIND + K_COLOUR);
    cpu.write16(TARGET_COLOUR, colour.wrapping_add(1));
    nested!(cpu, 0x0604_5b60, models::models_colour(cpu, TARGET_KIND, (colour | 0x1000) as u32));
    nested!(cpu, 0x0609_f44c, sgl::sl_sprite_type(cpu, 5));
    nested!(cpu, 0x0606_98dc, sgl::sl_priority(cpu, -8i32 as u32, 3));
    for table in TARGET_TABLES {
        for i in 0..TARGETS {
            cpu.write32(table + TARGET_ENTRY * i, 0);
        }
    }
    let hook = cpu.read32(screen::SCREEN) + SCREEN_HOOK;
    cpu.write32(hook, 0x0609_a40c);
}

/// The planes' sway (the targets' work): its flags (+0, a byte: 0x20 both planes righted at once,
/// 0x80 the first righted, 0x40 the second, each flag cleared once its plane is level), each
/// plane's turn speed about z (+0x10, +0x12: words) and what the speeds ease toward (+0x14,
/// +0x16); the stage boss's phase (past 2 the sway less: by a half in phase 4, else three
/// quarters).
const SWAY_FLAGS: u32 = 0;
const SWAY_BOTH: i8 = 0x20;
const SWAY_FIRST: i8 = -0x80;
const SWAY_SECOND: i8 = 0x40;
const SWAY_SPEEDS: u32 = 0x10;
const SWAY_TO: u32 = 0x14;
const BOSS_PHASE: u32 = 0x060a_48e0;

/// A plane's turn back toward level: its z angle's opposite, within `most`, a 16th of it.
fn righting(cpu: &mut Cpu, plane: u32, most: i32) -> i16 {
    let back = (cpu.read16(plane + 4) as i16).wrapping_neg() as i32;
    (back.clamp(-most, most) >> 4) as i16
}

/// The planes' sway, a frame: righted (both at once, a 16th of their tilt up to 0x60 a frame; or
/// each flagged one up to 0x40, its flag cleared once it is level), or each speed a step toward
/// its target.
pub fn planes_sway(cpu: &mut Cpu, w: u32) {
    let flags = cpu.read8(w + SWAY_FLAGS) as i8;
    if flags & SWAY_BOTH != 0 {
        for (k, plane) in [SWEEP_PLANE, TARGET_PLANE_B].into_iter().enumerate() {
            let v = righting(cpu, plane, 0x600);
            cpu.write16(w + SWAY_SPEEDS + 2 * k as u32, v as u16);
        }
        return;
    }
    if flags & (SWAY_FIRST | SWAY_SECOND) != 0 {
        if flags & SWAY_FIRST != 0 {
            let v = righting(cpu, SWEEP_PLANE, 0x400);
            if v == 0 {
                cpu.write8(w + SWAY_FLAGS, (flags & !SWAY_FIRST) as u8);
            }
            cpu.write16(w + SWAY_SPEEDS, v as u16);
        }
        let flags = cpu.read8(w + SWAY_FLAGS) as i8;
        if flags & SWAY_SECOND != 0 {
            let v = righting(cpu, TARGET_PLANE_B, 0x400);
            if v == 0 {
                cpu.write8(w + SWAY_FLAGS, (flags & !SWAY_SECOND) as u8);
            }
            cpu.write16(w + SWAY_SPEEDS + 2, v as u16);
        }
        return;
    }
    let mut to = [0, 2].map(|k| cpu.read16(w + SWAY_TO + k) as i16 as i32 as u32);
    let phase = cpu.read8(BOSS_PHASE) as i8;
    if phase > 2 {
        let by = if phase == 4 { 0x8000 } else { 0xc000 };
        for t in &mut to {
            *t = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, by, *t));
        }
    }
    for (k, to) in to.into_iter().enumerate() {
        let at = w + SWAY_SPEEDS + 2 * k as u32;
        let v = cpu.read16(at) as i16 as i32;
        let to = to as i32;
        if v > to {
            cpu.write16(at, (v - 1) as u16);
        } else if v < to {
            cpu.write16(at, (v + 1) as u16);
        }
    }
}

/// The targets' glow (the work's +0x2c, 0x400 a frame round 0x20000, up and back down): their
/// colour (TARGET_COLOUR's one) from (3, 29, 16) at none to (0, 12, 10) at whole (r, g, b).
const GLOW: u32 = 0x2c;
const GLOW_FROM: [u32; 3] = [0x3_0000, 0x1d_0000, 0x10_0000];
const GLOW_TO: [u32; 3] = [0, 0xc_0000, 0xa_0000];

pub fn targets_glow(cpu: &mut Cpu, w: u32) {
    let t = cpu.read32(w + GLOW).wrapping_add(0x400);
    cpu.write32(w + GLOW, t);
    if t as i32 > 0x2_0000 {
        cpu.write32(w + GLOW, 0);
    }
    let t = cpu.read32(w + GLOW);
    let t = if t as i32 > 0x1_0000 { 0x2_0000 - t } else { t };
    let [r, g, b] = [0, 1, 2].map(|k| {
        let v = nested!(cpu, 0x0606_2e7c, math::lerp(cpu, GLOW_FROM[k], GLOW_TO[k], t));
        (v >> 16) as i16 as i32 as u32
    });
    let colour = (b << 10) | (g << 5) | r;
    let at = cpu.stack();
    cpu.write16(at, colour as u16);
    let index = cpu.read16(TARGET_COLOUR) as u32;
    nested!(cpu, 0x0600_5b18, palette::set_colours(cpu, at, index, 1));
}

/// The targets' steps (the work's step's low four bits): the view moved down (0: 1/4 dot a frame,
/// 2: 1/8) to -64 dots, or up (1, 3) to 64, or (4) back to the middle 1/4 a frame; the planes'
/// sway, each plane turned by its speed (kept at +0xa); the targets' frame and glow; window 0's
/// lines; with NBG1 following (the work's +2), NBG1's place by the view's centre on screen.
const TARGETS_STEP_MASK: u8 = 15;
const TARGETS_FOLLOW: u32 = 2;

pub fn targets_steps(cpu: &mut Cpu, w: u32) {
    let at = END_VIEW + END_VIEW_POSITION + 4;
    let y = cpu.read32(at) as i32;
    let moved = match cpu.read8(w + END_STEP) & TARGETS_STEP_MASK {
        0 if y > -0x40_0000 => Some(y - 0x4000),
        1 if y <= 0x3f_ffff => Some(y + 0x4000),
        2 if y > -0x40_0000 => Some(y - 0x2000),
        3 if y <= 0x3f_ffff => Some(y + 0x2000),
        4 => {
            if y.wrapping_abs() <= 0x3fff {
                Some(0)
            } else if y < 0 {
                Some(y + 0x4000)
            } else {
                Some(y - 0x4000)
            }
        }
        _ => None,
    };
    if let Some(y) = moved {
        cpu.write32(at, y as u32);
    }
    nested!(cpu, 0x0609_a8d8, planes_sway(cpu, w));
    for (k, plane) in [SWEEP_PLANE, TARGET_PLANE_B].into_iter().enumerate() {
        let speed = cpu.read16(w + SWAY_SPEEDS + 2 * k as u32);
        cpu.write16(plane + 10, speed);
        let turn = cpu.read16(plane + 4).wrapping_add(speed);
        cpu.write16(plane + 4, turn);
    }
    nested!(cpu, 0x0609_a40c, targets_frame(cpu, w));
    nested!(cpu, 0x0609_aa64, targets_glow(cpu, w));
    nested!(cpu, 0x0609_ac50, window_lines(cpu));
    if cpu.read8(w + TARGETS_FOLLOW) != 0 {
        let y = cpu.read32(VIEW_CENTRE_ON_SCREEN + 4) << 16;
        cpu.write32(bg::PLANE_B + bg::Y, 0x0100_0000u32.wrapping_sub(y));
        nested!(cpu, 0x0605_f08c, bg::plane_b_frame(cpu));
    }
}

/// Window 0's lines (224, each its left and right edges, sent to VDP2's line table at 0x25e3fa80):
/// round the view's centre on screen, each line's half width from 0x060a3480's table by its
/// distance from the centre's line (none: the line shut, 0x2c00000).
const WINDOW_LINES: u32 = 0x060a_4490;
const WINDOW_HALF_WIDTHS: u32 = 0x060a_3480;
const WINDOW_LINES_VRAM: u32 = 0x25e3_fa80;
const WINDOW_SHUT: u32 = 0x02c0_0000;
const SCREEN_LINES: i32 = 224;

pub fn window_lines(cpu: &mut Cpu) {
    let centre = (cpu.read32(VIEW_CENTRE_ON_SCREEN).wrapping_add(0xb0)).wrapping_mul(2);
    let line = cpu.read32(VIEW_CENTRE_ON_SCREEN + 4).wrapping_add(112) as i32;
    for i in 0..SCREEN_LINES {
        let d = (i - line).wrapping_abs().min(SCREEN_LINES - 1);
        let half = cpu.read16(WINDOW_HALF_WIDTHS + 2 * d as u32) as u32;
        let edges = if half != 0 {
            (centre.wrapping_sub(half) << 16) | half.wrapping_add(centre)
        } else {
            WINDOW_SHUT
        };
        cpu.write32(WINDOW_LINES + 4 * i as u32, edges);
    }
    let size = 4 * SCREEN_LINES as u32;
    nested!(cpu, 0x0603_4a50, sgl::sl_transfer_entry(cpu, WINDOW_LINES, WINDOW_LINES_VRAM, size));
}

/// The view's turn (an object: its mode +3, its step +0x10, its wait +0x14, its turn speed +0x18
/// and how far it has turned +0x1c).
const TURNER_MODE: u32 = 3;
const TURNER_STEP: u32 = 0x10;
const TURNER_WAIT: u32 = 0x14;
const TURNER_SPEED: u32 = 0x18;
const TURNER_TURNED: u32 = 0x1c;
const TURNER_SIZE: u32 = 0x20;

/// The view turned about y (while the game is not held): after 30 frames, its speed up a step a
/// frame to 64; once turned past 0x77df, slowed a step a frame; stopped (its y angle none) and
/// done.
pub fn view_turner(cpu: &mut Cpu, o: u32) {
    if cpu.read32(GAME_FLAGS) & HELD != 0 {
        return;
    }
    match cpu.read8(o + TURNER_MODE) as i8 {
        0 => {
            cpu.write32(o + TURNER_TURNED, 0);
            cpu.write32(o + TURNER_SPEED, 0);
            cpu.write32(o + TURNER_STEP, 0);
            cpu.write32(o + TURNER_WAIT, 30);
            cpu.write8(o + TURNER_MODE, 1);
        }
        1 => {}
        112 => {
            cpu.write8(o + TURNER_MODE, 115);
            return;
        }
        115 => {
            nested!(cpu, 0x0600_579c, objects::obj_free(cpu, o));
            return;
        }
        _ => return,
    }
    let next = match cpu.read32(o + TURNER_STEP) {
        0 => {
            let wait = cpu.read32(o + TURNER_WAIT).wrapping_sub(1);
            cpu.write32(o + TURNER_WAIT, wait);
            wait == 0
        }
        1 => {
            let speed = cpu.read32(o + TURNER_SPEED).wrapping_add(1);
            cpu.write32(o + TURNER_SPEED, speed);
            let fastest = speed as i32 > 64;
            if fastest {
                cpu.write32(o + TURNER_SPEED, 64);
            }
            fastest
        }
        2 => cpu.read32(o + TURNER_TURNED) as i32 > 0x77df,
        3 => {
            let speed = cpu.read32(o + TURNER_SPEED).wrapping_sub(1);
            cpu.write32(o + TURNER_SPEED, speed);
            if (speed as i32) < 0 {
                cpu.write32(o + TURNER_SPEED, 0);
                cpu.write32(END_VIEW + END_VIEW_ANGLES + 4, 0);
                cpu.write8(o + TURNER_MODE, 112);
            }
            false
        }
        _ => false,
    };
    if next {
        let step = cpu.read32(o + TURNER_STEP).wrapping_add(1);
        cpu.write32(o + TURNER_STEP, step);
    }
    let speed = cpu.read32(o + TURNER_SPEED);
    let at = END_VIEW + END_VIEW_ANGLES + 4;
    let turn = cpu.read32(at).wrapping_add(speed);
    cpu.write32(at, turn);
    let turned = cpu.read32(o + TURNER_TURNED).wrapping_add(cpu.read32(o + TURNER_SPEED));
    cpu.write32(o + TURNER_TURNED, turned);
}

/// The targets with the view rising (by the plane's speed) and NBG1's ground on: once its map ends
/// the view's turn made and the scroll moved on.
pub fn targets_rising(cpu: &mut Cpu, w: u32) {
    let at = END_VIEW + END_VIEW_POSITION + 4;
    let y = cpu.read32(at).wrapping_add(cpu.read32(w + CLEAR_PLANE_SPEED));
    cpu.write32(at, y);
    nested!(cpu, 0x0609_ab14, targets_steps(cpu, w));
    if nested!(cpu, 0x0609_9f08, last_nbg1(cpu, w)) != 0 {
        let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, TURNER_SIZE)) as i8 as i32 as u32;
        nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 0, blocks, 0x0609_acdc));
        nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
    }
}

/// NBG1's sea: its page (cells at 0x25e60000, 0x8000) and its one map (0x25e74000); its cells (the
/// stage's eighth cell list's, 0x1280 bytes, 0x2000 a VBlank) and map (the screen's room from
/// +0x22000) queued; its colours' blend (0x060a2dbc toward 0x060a3640, into NBG1_BLENDED) from
/// none, and 16 of 0x060a2dbc's at 0; NBG1 at its place at full size.
const SEA_CELLS_SIZE: u32 = 0x1280;
const SEA_MAP_FROM: u32 = 0x2_2000;
const SEA_BLEND: u32 = 0x060a_3660;
const SEA_COLOURS: u32 = 0x060a_2dbc;
const SEA_NBG1_AT: u32 = 0x0100_0000;

/// NBG1's sea's start: as above; NBG1 followed (the work's +2 a step on); NBG1 (slPriority's 0)
/// at priority 3; colour calculation 0x110.
pub fn sea_start(cpu: &mut Cpu, w: u32) {
    nested!(cpu, 0x0606_e2e4, sgl::sl_page_nbg1(cpu, CLEAR_NBG0_CELLS, 0, 0x8000));
    let m = NEW_NBG0_MAP;
    nested!(cpu, 0x0607_0d1c, sgl::sl_map_nbg1(cpu, m, m, m, m));
    let list = cpu.read32(STAGE_CELLS_LIST);
    let cells = cpu.read32(list + 28).wrapping_add(cpu.read32(STAGE_DATA_AT));
    nested!(
        cpu,
        0x0605_ed2c,
        screen::queue_long_copy(cpu, cells, CLEAR_NBG0_CELLS, SEA_CELLS_SIZE, 0x2000)
    );
    let room = cpu.read32(SCREEN_ROOM) + SEA_MAP_FROM;
    nested!(cpu, 0x0605_ecf0, screen::queue_copy(cpu, room, NEW_NBG0_MAP, NBG1_MAP_SIZE));
    cpu.write16(SEA_BLEND, 0);
    nested!(cpu, 0x0600_5b18, palette::set_colours(cpu, SEA_COLOURS, 0, NBG1_COLOUR_COUNT));
    cpu.write32(bg::PLANE_B + bg::X, SEA_NBG1_AT);
    cpu.write32(bg::PLANE_B + bg::Y, SEA_NBG1_AT);
    cpu.write32(bg::PLANE_B + bg::SPEED_Y, 0);
    cpu.write32(bg::PLANE_B + bg::SPEED_X, 0);
    cpu.write32(bg::PLANE_B + bg::ZOOM_Y, 0x1_0000);
    cpu.write32(bg::PLANE_B + bg::ZOOM_X, 0x1_0000);
    nested!(cpu, 0x0605_f08c, bg::plane_b_frame(cpu));
    let follow = cpu.read8(w + TARGETS_FOLLOW).wrapping_add(1);
    cpu.write8(w + TARGETS_FOLLOW, follow);
    nested!(cpu, 0x0606_98dc, sgl::sl_priority(cpu, 0, 3));
    nested!(cpu, 0x0607_0ddc, sgl::sl_color_calc(cpu, 0x110));
}

/// The view's pitch speed (the targets' work's +0x24).
const PITCH_SPEED: u32 = 0x24;

/// The view's pitch on by its speed: the pitch.
fn pitch_on(cpu: &mut Cpu, w: u32) -> u32 {
    let pitch = cpu.read32(END_VIEW + END_VIEW_ANGLES).wrapping_add(cpu.read32(w + PITCH_SPEED));
    cpu.write32(END_VIEW + END_VIEW_ANGLES, pitch);
    pitch
}

/// The view pitched down a step a frame (to 32 a frame) until its x angle is -0x1000, then eased
/// back toward still.
fn pitch_down(cpu: &mut Cpu, w: u32) {
    let speed = cpu.read32(w + PITCH_SPEED) as i32;
    let pitch = cpu.read32(END_VIEW + END_VIEW_ANGLES) as i32;
    let speed = if pitch > -0x1000 { (speed - 1).max(-32) } else { (speed + 1).min(0) };
    cpu.write32(w + PITCH_SPEED, speed as u32);
    pitch_on(cpu, w);
}

/// The view's pitch speed back up (a step a frame to 32, slowed to still once past -0x200).
fn pitch_up_speed(cpu: &mut Cpu, w: u32) {
    let speed = cpu.read32(w + PITCH_SPEED) as i32;
    let pitch = cpu.read32(END_VIEW + END_VIEW_ANGLES) as i32;
    let speed = if pitch >= -0x200 { (speed - 1).max(0) } else { (speed + 1).min(32) };
    cpu.write32(w + PITCH_SPEED, speed as u32);
}

/// The dive, a frame: the targets' steps; the view down by the plane's speed (at the bottom held
/// there and the scroll moved on); pitched down a step a frame (to 32 a frame) until its x angle
/// is -0x1000, then eased back toward level; on 3/4 dot a frame.
pub fn targets_dive(cpu: &mut Cpu, w: u32) {
    nested!(cpu, 0x0609_ab14, targets_steps(cpu, w));
    let at = END_VIEW + END_VIEW_POSITION + 4;
    let y = cpu.read32(at).wrapping_add(cpu.read32(w + CLEAR_PLANE_SPEED));
    cpu.write32(at, y);
    if y as i32 <= 0 {
        cpu.write32(at, 0);
        nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
    }
    pitch_down(cpu, w);
    let z = cpu.read32(END_VIEW + END_VIEW_POSITION + 8).wrapping_add(0xc000);
    cpu.write32(END_VIEW + END_VIEW_POSITION + 8, z);
}

/// The level-out, a frame: the targets' steps; until the view is on to -0x2270000, its pitch eased
/// back up (a step a frame to 32, slowed once it is past -0x200); on 3/4 dot a frame to
/// -0x1a70000, where once level the scroll moves on.
pub fn targets_level(cpu: &mut Cpu, w: u32) {
    nested!(cpu, 0x0609_ab14, targets_steps(cpu, w));
    if cpu.read32(END_VIEW + END_VIEW_POSITION + 8) as i32 > 0xfdd9_0000u32 as i32 {
        pitch_up_speed(cpu, w);
    }
    let pitch = pitch_on(cpu, w);
    let at = END_VIEW + END_VIEW_POSITION + 8;
    let z = cpu.read32(at).wrapping_add(0xc000);
    cpu.write32(at, z);
    let last = 0xfe59_0000u32;
    if z as i32 > last as i32 {
        cpu.write32(at, last);
        if pitch == 0 {
            nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
        }
    }
}

/// The targets' event: the targets made (their result passed on).
pub fn targets_event(cpu: &mut Cpu, e: u32) -> u32 {
    nested!(cpu, 0x0609_a330, targets_set(cpu, e))
}

/// The targets cleared: the work's step 4 with both planes righted; the sway's targets none, its
/// count, pitch speed none and the plane's speed -1/2; the targets' frame the screen's hook; every
/// target done (mode 112) and its entry emptied; the sea's blend back (0x060a3640 toward
/// 0x060a2dbc, into NBG1_BLENDED) from none.
const TARGETS_CLEARED: u8 = 0x24;
const TARGETS_COUNT: u32 = 0x20;
const SEA_BLEND_BACK: u32 = 0x060a_3670;

pub fn targets_clear(cpu: &mut Cpu, w: u32) {
    cpu.write8(w + END_STEP, TARGETS_CLEARED);
    cpu.write16(w + SWAY_TO, 0);
    cpu.write16(w + SWAY_TO + 2, 0);
    cpu.write32(w + TARGETS_COUNT, 0);
    cpu.write32(w + PITCH_SPEED, 0);
    cpu.write32(w + CLEAR_PLANE_SPEED, 0xffff_8000);
    let hook = cpu.read32(screen::SCREEN) + SCREEN_HOOK;
    cpu.write32(hook, 0x0609_a40c);
    for table in TARGET_TABLES {
        for i in 0..TARGETS {
            let entry = table + TARGET_ENTRY * i;
            let target = cpu.read32(entry);
            if target != 0 {
                cpu.write8(target + OBJ_MODE, 112);
                cpu.write32(entry, 0);
            }
        }
    }
    cpu.write16(SEA_BLEND_BACK, 0);
}

/// The climb out, a frame: after 300 frames the view pitched down and on (y and z each half a
/// dot a frame less); the targets' steps; the sea's colours blended back (1/128 a frame).
pub fn climb_out(cpu: &mut Cpu, w: u32) {
    let count = cpu.read32(w + TARGETS_COUNT);
    cpu.write32(w + TARGETS_COUNT, count.wrapping_add(1));
    if count as i32 > 300 {
        pitch_down(cpu, w);
        for k in [4, 8] {
            let at = END_VIEW + END_VIEW_POSITION + k;
            let v = cpu.read32(at).wrapping_add(0xffff_8000);
            cpu.write32(at, v);
        }
    }
    nested!(cpu, 0x0609_ab14, targets_steps(cpu, w));
    let how_far = cpu.read16(SEA_BLEND_BACK);
    if how_far as i16 <= 0xfff {
        cpu.write16(SEA_BLEND_BACK, how_far.wrapping_add(32));
        nested!(cpu, 0x0600_5ba4, palette::colours_blend(cpu, SEA_BLEND_BACK));
        nested!(cpu, 0x0600_5b18, palette::set_colours(cpu, NBG1_BLENDED, 0, NBG1_COLOUR_COUNT));
    }
}

/// The climb out until the view is on past -0x2600000, then the scroll moved on.
pub fn climb_out_steps(cpu: &mut Cpu, w: u32) {
    nested!(cpu, 0x0609_b0c0, climb_out(cpu, w));
    if (cpu.read32(END_VIEW + END_VIEW_POSITION + 8) as i32) < 0xfda0_0000u32 as i32 {
        nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
    }
}

/// The pull-up, a frame: the pitch eased back up; falling faster (1/16 dot a frame more, to 4
/// dots a frame); on half a dot a frame; the targets' steps.
pub fn pull_up(cpu: &mut Cpu, w: u32) {
    pitch_up_speed(cpu, w);
    pitch_on(cpu, w);
    let fall = (cpu.read32(w + CLEAR_PLANE_SPEED) as i32 - 0x1000).max(-0x4_0000);
    cpu.write32(w + CLEAR_PLANE_SPEED, fall as u32);
    let at = END_VIEW + END_VIEW_POSITION + 4;
    let y = cpu.read32(at).wrapping_add(cpu.read32(w + CLEAR_PLANE_SPEED));
    cpu.write32(at, y);
    let at = END_VIEW + END_VIEW_POSITION + 8;
    let z = cpu.read32(at).wrapping_add(0xffff_8000);
    cpu.write32(at, z);
    nested!(cpu, 0x0609_ab14, targets_steps(cpu, w));
}

/// The pull-up until the view is level, then the scroll moved on.
pub fn pull_up_steps(cpu: &mut Cpu, w: u32) {
    nested!(cpu, 0x0609_b198, pull_up(cpu, w));
    if cpu.read32(END_VIEW + END_VIEW_ANGLES) == 0 {
        nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
    }
}

/// NBG0's zoom-in (its zoom at +0x10).
pub fn nbg0_zoomer(cpu: &mut Cpu, o: u32) {
    zoom_in(cpu, o, 0x10, bg::PLANE_A);
}

/// The run-out's ground: NBG0's page (cells at 0x25e68000, 0x400, 0x8000) and its one map
/// (0x25e76000); 240 colours of the stage's eleventh list at 0x200; its ground map (24 blocks of
/// 0x060a2b18's, from 0x1c40000) set out and its plane cleared; NBG0 at its place; the scroll's
/// NBG1 speed three dots a frame up; NBG0, NBG2 and the rotation plane shown; NBG0's zoom-in made;
/// the run-out's message.
const RUN_MAP: u32 = 0x060a_4820;
const RUN_MAP_SET: MapSet = MapSet {
    blocks: 24,
    table: 0x060a_2b18,
    cells: NBG1_MAP_VRAM,
    at: 0x01c4_0000,
    mask: 0x1fff,
    ahead: 0x90,
    shapes: 0,
};
const RUN_COLOURS: u32 = 0xf0;
const RUN_MESSAGE: u32 = 0x060a_2d40;
const ZOOMER_SIZE: u32 = 20;

pub fn run_out_start(cpu: &mut Cpu, w: u32) {
    nested!(cpu, 0x0606_e300, sgl::sl_page_nbg0(cpu, LAST_NBG1_CELLS, 0x400, 0x8000));
    let m = NBG1_MAP_VRAM;
    nested!(cpu, 0x0607_0c4c, sgl::sl_map_nbg0(cpu, m, m, m, m));
    let list = cpu.read32(STAGE_COLOURS_LIST);
    let colours = cpu.read32(list + 40).wrapping_add(cpu.read32(STAGE_DATA_AT));
    nested!(cpu, 0x0600_5b18, palette::set_colours(cpu, colours, NBG1_COLOURS, RUN_COLOURS));
    map_set(cpu, RUN_MAP, &RUN_MAP_SET);
    cpu.write32(RUN_MAP + MAP_EMPTY, EMPTY_CELLS);
    nested!(cpu, 0x0605_f14c, bg::map_clear(cpu, RUN_MAP));
    cpu.write32(bg::PLANE_A + bg::X, FLY_NBG1_AT[0]);
    cpu.write32(bg::PLANE_A + bg::Y, FLY_NBG1_AT[1]);
    cpu.write32(bg::PLANE_A + bg::SPEED_Y, 0);
    cpu.write32(bg::PLANE_A + bg::SPEED_X, 0);
    cpu.write32(w + FLY_NBG1_SPEED, 0xfffd_0000);
    nested!(cpu, 0x0606_e23c, sgl::sl_bg_on(cpu, 0x2_0015));
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, ZOOMER_SIZE)) as i8 as i32 as u32;
    nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 0, blocks, 0x0609_b23c));
    nested!(cpu, 0x0606_1620, text::message_set(cpu, RUN_MESSAGE));
}

/// NBG0 at the scroll's speed (+0x1c), the run-out's map streamed, its frame.
pub fn run_nbg0(cpu: &mut Cpu, w: u32) {
    let speed = cpu.read32(w + FLY_NBG1_SPEED);
    nested!(cpu, 0x0605_efd4, bg::plane_a_speed(cpu, 0, speed));
    nested!(cpu, 0x0605_f170, bg::map_stream(cpu, bg::PLANE_A, RUN_MAP));
    nested!(cpu, 0x0605_f040, bg::plane_a_frame(cpu));
}

/// The run-out, a frame: NBG0's ground, the pull-up; the scroll moved on once the map has come
/// 0x3000000.
pub fn run_out_steps(cpu: &mut Cpu, w: u32) {
    nested!(cpu, 0x0609_b3f8, run_nbg0(cpu, w));
    nested!(cpu, 0x0609_b198, pull_up(cpu, w));
    if cpu.read32(RUN_MAP + MAP_AT) as i32 > 0x0300_0000 {
        nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
    }
}

/// The approach's screens: the perspective 0x4000 in window 0; RBG0's page (0x25e00000, 512
/// colours, 0x8000) and maps (0x060a3680's), rotation parameter mode 0, parameter A current, the
/// windows' control at 7 none; 96 colours of the stage's eighth list at 0x100; the rotation plane
/// at (-176, -368, 56) dots, still, an eighth turned about x; its cells (the stage's fifth cell
/// list's, 0x2a20 bytes, 0x800 a VBlank); NBG1's page (0x25e60000, 0x8000) and one map
/// (0x25e74000), its cells (the sixth list's, 0x2a0 bytes) and map (the screen's room from
/// +0x6000, 0x1000 bytes), 16 colours of the ninth list at 0; NBG1 at its place at full size.
const APPROACH_RA_MAPS: u32 = 0x060a_3680;
const APPROACH_PLANE_COLOURS: u32 = 96;
const APPROACH_PLANE_AT: [u32; 3] = [0xff50_0000, 0xfe90_0000, 0x0038_0000];
const APPROACH_PLANE_TILT: u16 = 0x2000;
const APPROACH_PLANE_CELLS_SIZE: u32 = 0x2a20;
const APPROACH_NBG1_CELLS_SIZE: u32 = 0x2a0;
const APPROACH_NBG1_MAP_FROM: u32 = 0x6000;
const APPROACH_NBG1_MAP_SIZE: u32 = 0x1000;

/// The approach's start: as above; NBG0 (slPriority's 1) at priority 5, NBG1 2, RBG0 3; colour
/// calculation 0x110; NBG0, NBG1, NBG2 and the rotation plane shown; the screen's hook off; the
/// plane's scroll still at its start, its rise 4 dots a frame, the plane's speed none.
const APPROACH_SPEED: u32 = 0x0c;
const APPROACH_AT: u32 = 0x20;
const APPROACH_RISE: u32 = 0x24;

pub fn approach_start(cpu: &mut Cpu, w: u32) {
    nested!(cpu, 0x0606_713c, sgl::sl_cur_window(cpu, 0));
    nested!(cpu, 0x0606_e1cc, sgl::sl_perspective(cpu, 0x4000));
    nested!(cpu, 0x0606_713c, sgl::sl_cur_window(cpu, 1));
    nested!(cpu, 0x0606_e2ce, sgl::sl_page_rbg0(cpu, 0x25e0_0000, 0x200, 0x8000));
    nested!(cpu, 0x0609_f344, sgl::sl_map_ra_table(cpu, APPROACH_RA_MAPS));
    nested!(cpu, 0x0609_f42c, sgl::sl_rpara_mode(cpu, 0));
    nested!(cpu, 0x0609_f43c, sgl::sl_cur_rpara(cpu, 0));
    nested!(cpu, 0x0606_e17c, sgl::sl_window_control(cpu, 7, 0));
    let list = cpu.read32(STAGE_COLOURS_LIST);
    let colours = cpu.read32(list + 28).wrapping_add(cpu.read32(STAGE_DATA_AT));
    nested!(cpu, 0x0600_5b18, palette::set_colours(cpu, colours, CLEAR_COLOURS, APPROACH_PLANE_COLOURS));
    for (k, v) in APPROACH_PLANE_AT.into_iter().enumerate() {
        cpu.write32(SWEEP_PLANE + SWEEP_POSITION + 4 * k as u32, v);
    }
    for k in 0..3 {
        cpu.write32(SWEEP_PLANE + SWEEP_SPEED + 4 * k, 0);
    }
    cpu.write16(SWEEP_PLANE, APPROACH_PLANE_TILT);
    for k in 1..6 {
        cpu.write16(SWEEP_PLANE + 2 * k, 0);
    }
    let list = cpu.read32(STAGE_CELLS_LIST);
    let cells = cpu.read32(list + 16).wrapping_add(cpu.read32(STAGE_DATA_AT));
    nested!(
        cpu,
        0x0605_ed2c,
        screen::queue_long_copy(cpu, cells, 0x25e0_0000, APPROACH_PLANE_CELLS_SIZE, 0x800)
    );
    nested!(cpu, 0x0606_e2e4, sgl::sl_page_nbg1(cpu, CLEAR_NBG0_CELLS, 0, 0x8000));
    let m = NEW_NBG0_MAP;
    nested!(cpu, 0x0607_0d1c, sgl::sl_map_nbg1(cpu, m, m, m, m));
    let list = cpu.read32(STAGE_CELLS_LIST);
    let cells = cpu.read32(list + 20).wrapping_add(cpu.read32(STAGE_DATA_AT));
    nested!(
        cpu,
        0x0605_ed2c,
        screen::queue_long_copy(cpu, cells, CLEAR_NBG0_CELLS, APPROACH_NBG1_CELLS_SIZE, 0x2000)
    );
    let room = cpu.read32(SCREEN_ROOM) + APPROACH_NBG1_MAP_FROM;
    nested!(cpu, 0x0605_ecf0, screen::queue_copy(cpu, room, NEW_NBG0_MAP, APPROACH_NBG1_MAP_SIZE));
    let list = cpu.read32(STAGE_COLOURS_LIST);
    let colours = cpu.read32(list + 32).wrapping_add(cpu.read32(STAGE_DATA_AT));
    nested!(cpu, 0x0600_5b18, palette::set_colours(cpu, colours, 0, NBG1_COLOUR_COUNT));
    cpu.write32(bg::PLANE_B + bg::X, FLY_NBG1_AT[0]);
    cpu.write32(bg::PLANE_B + bg::Y, FLY_NBG1_AT[1]);
    cpu.write32(bg::PLANE_B + bg::SPEED_Y, 0);
    cpu.write32(bg::PLANE_B + bg::SPEED_X, 0);
    cpu.write32(bg::PLANE_B + bg::ZOOM_Y, 0x1_0000);
    cpu.write32(bg::PLANE_B + bg::ZOOM_X, 0x1_0000);
    nested!(cpu, 0x0605_f08c, bg::plane_b_frame(cpu));
    for (screen, pri) in [(1, 5), (0, 2), (5, 3)] {
        nested!(cpu, 0x0606_98dc, sgl::sl_priority(cpu, screen, pri));
    }
    nested!(cpu, 0x0607_0ddc, sgl::sl_color_calc(cpu, 0x110));
    nested!(cpu, 0x0606_e23c, sgl::sl_bg_on(cpu, 0x17));
    let hook = cpu.read32(screen::SCREEN) + SCREEN_HOOK;
    cpu.write32(hook, 0);
    cpu.write32(w + APPROACH_SPEED, 0);
    cpu.write32(w + APPROACH_AT, 0);
    cpu.write32(w + APPROACH_RISE, 0x4_0000);
    cpu.write32(w + CLEAR_PLANE_SPEED, 0);
}

/// The scroll's step that does nothing.
pub fn approach_idle(_cpu: &mut Cpu) {}

/// The approach's plane, a frame: its scroll on by its speed; its parameters from the matrix moved
/// to it, tilted about x, scrolled along its y, inverted.
pub fn approach_plane(cpu: &mut Cpu, w: u32) {
    let at = cpu.read32(w + APPROACH_AT).wrapping_add(cpu.read32(w + APPROACH_SPEED));
    cpu.write32(w + APPROACH_AT, at);
    nested!(cpu, 0x0606_71cc, sgl::sl_push_matrix(cpu));
    let [x, y, z] = [0, 4, 8].map(|k| cpu.read32(SWEEP_PLANE + SWEEP_POSITION + k));
    nested!(cpu, 0x0606_721c, sgl::sl_translate(cpu, x, y, z));
    let tilt = cpu.read16(SWEEP_PLANE) as i16 as u32;
    nested!(cpu, 0x0606_72ec, sgl::sl_rot_x(cpu, tilt));
    let at = cpu.read32(w + APPROACH_AT);
    nested!(cpu, 0x0606_721c, sgl::sl_translate(cpu, 0, at, 0));
    nested!(cpu, 0x0606_e4dc, sgl::sl_invers_matrix(cpu));
    nested!(cpu, 0x0606_e50c, sgl::rotation_parameters(cpu));
    nested!(cpu, 0x0606_7200, sgl::sl_pop_matrix(cpu));
}

/// The approach, a frame: NBG0's ground; past 0x2c000000 NBG0 zoomed out (0x80 a frame); past
/// 0x2d000000 the plane's scroll sped up (0xa3d a frame, to half a dot); the plane; past
/// 0x2d800000 the plane rising (its rise slowed 1/16 a frame, to half a dot) to -0x700000, where
/// the scroll moves on.
pub fn approach_steps(cpu: &mut Cpu, w: u32) {
    nested!(cpu, 0x0609_b3f8, run_nbg0(cpu, w));
    if cpu.read32(RUN_MAP + MAP_AT) as i32 > 0x2c00_0000 {
        for zoom in [bg::ZOOM_X, bg::ZOOM_Y] {
            let z = cpu.read32(bg::PLANE_A + zoom).wrapping_add(0x80);
            cpu.write32(bg::PLANE_A + zoom, z);
        }
    }
    if cpu.read32(RUN_MAP + MAP_AT) as i32 > 0x2d00_0000 {
        let speed = (cpu.read32(w + APPROACH_SPEED) as i32 + 0xa3d).min(0x8_0000);
        cpu.write32(w + APPROACH_SPEED, speed as u32);
    }
    nested!(cpu, 0x0609_b688, approach_plane(cpu, w));
    if cpu.read32(RUN_MAP + MAP_AT) as i32 > 0x2d80_0000 {
        let rise = cpu.read32(w + APPROACH_RISE).wrapping_add(0xffff_f000);
        cpu.write32(w + APPROACH_RISE, if rise as i32 > 0x7fff { rise } else { 0x8000 });
        let at = SWEEP_PLANE + SWEEP_POSITION + 4;
        let y = cpu.read32(at).wrapping_add(cpu.read32(w + APPROACH_RISE));
        cpu.write32(at, y);
        let top = 0xff90_0000u32;
        if y as i32 > top as i32 {
            cpu.write32(at, top);
            nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
        }
    }
}

/// The end's spinner (models.rs's: its own byte +0x3c, its count +0x40): unseen (its draw SGL's empty
/// wait) until 150 frames on, then drawn as a spinner; the director's flag 8 ends it.
const END_SPINNER_SHOWN: u32 = 0x3c;
const END_SPINNER_COUNT: u32 = 0x40;
const END_SPINNER_SHOWS_AT: u32 = 150;
const END_SPINNER_DRAW: u32 = 0x0606_29fc;
const DIRECTOR_ENDS: u32 = 8;

/// The end's spinner, a frame: done once the director's flag 8 is set; else turned, and after 150
/// frames shown (and the scroll moved on).
pub fn end_spinner_behaviour(cpu: &mut Cpu, o: u32) {
    if nested!(cpu, 0x0604_0cb4, director::director_flags(cpu)) & DIRECTOR_ENDS != 0 {
        cpu.write8(o + OBJ_MODE, 112);
        return;
    }
    if cpu.read8(o + END_SPINNER_SHOWN) == 0 {
        let n = cpu.read32(o + END_SPINNER_COUNT).wrapping_add(1);
        cpu.write32(o + END_SPINNER_COUNT, n);
        if n == END_SPINNER_SHOWS_AT {
            cpu.write32(o + MODEL, END_SPINNER_DRAW);
            cpu.write32(o + END_SPINNER_COUNT, 0);
            nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
            let shown = cpu.read8(o + END_SPINNER_SHOWN).wrapping_add(1);
            cpu.write8(o + END_SPINNER_SHOWN, shown);
        }
    }
    nested!(cpu, 0x0606_2aa0, models::spinner_turn(cpu, o));
}

/// The end's spinner made at `p` (x, y, z) (list 2, 0x48 bytes): unseen at first; the view's matrix kept;
/// kind 32's models lit by one light. It, or 0.
const END_SPINNER_SIZE: u32 = 0x48;
const END_SPINNER_BEHAVIOUR: u32 = 0x0609_b7b4;
const EMPTY_DRAW: u32 = 0x0603_3654;
const END_SPINNER_KIND: u32 = 32;

pub fn end_spinner_set(cpu: &mut Cpu, p: u32) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, END_SPINNER_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, PLAIN_EXEC));
    if o != 0 {
        nested!(cpu, 0x0606_2af8, models::spinner_init(cpu, o));
        cpu.write32(o + BEHAVIOUR, END_SPINNER_BEHAVIOUR);
        cpu.write32(o + MODEL, EMPTY_DRAW);
        for k in [0, 4, 8] {
            let v = cpu.read32(p + k);
            cpu.write32(o + POSITION + k, v);
        }
    }
    let matrix = cpu.read32(screen::SCREEN) + SCREEN_MATRIX;
    nested!(cpu, 0x0606_db9c, sgl::sl_get_matrix(cpu, matrix));
    nested!(cpu, 0x0606_3380, gouraud::models_lit(cpu, END_SPINNER_KIND, 1));
    o
}

/// The next stage's choice: its step (the work's +0, a byte) and the choice (+1: 0 stage 2, 1
/// stage 4), its count (+0x28).
const CHOICE: u32 = 1;
const CHOICE_COUNT: u32 = 0x28;

/// The choice's start: its step and choice 0; the scene on.
pub fn choice_start(cpu: &mut Cpu, w: u32) {
    cpu.write8(w + END_STEP, 0);
    cpu.write8(w + CHOICE, 0);
    nested!(cpu, 0x0606_1738, player::scene_on(cpu));
}

/// The choice's lines (the overlay's strings: "SELECT NEXT STAGE", "  STAGE 2", "  STAGE 4", and
/// blanks to clear them) and where they go (VDP2's map), the cursor (the large font's '_', a space
/// where it is not); a flag (a byte at 0x06075d30, not known yet) the third step waits on; the
/// players' state's low byte (past 2 they are playing) and pads' pressed words (A, B or C 0x0f00,
/// up or down 0x3000); each player's route (PLAY_DATA's +0x3c: 1 stage 4) and the mode after
/// (NEXT_MODE: 8 stage 2, 9 stage 4).
const CHOICE_TITLE: (u32, u32) = (0x0609_b9b4, 0x25e7_e50a);
const CHOICE_LINES: [(u32, u32); 2] = [(0x0609_b9c8, 0x25e7_e71a), (0x0609_b9d4, 0x25e7_e89a)];
const CHOICE_TITLE_BLANK: u32 = 0x0609_b9e0;
const CHOICE_LINE_BLANK: u32 = 0x0609_b9f4;
const CHOICE_CURSOR: u32 = b'_' as u32;
const CHOICE_NO_CURSOR: u32 = b' ' as u32;
const CHOICE_FLAG: u32 = 0x0607_5d30;
const PADS_PRESSED: [u32; 2] = [0x0602_ee14, 0x0602_ee1c];
const PAD_UP_DOWN: u16 = 0x3000;
const PAD_BUTTONS: u16 = 0x0f00;
const PLAY_ROUTE: u32 = 0x3c;
const NEXT_STAGE_MODES: [u8; 2] = [8, 9];
/// How long the title waits (300 frames), how long before the choice is taken (30), and how
/// long it may last (300).
const CHOICE_WAIT: u32 = 300;
const CHOICE_FROM: i32 = 29;
const CHOICE_LAST: i32 = 300;

/// A player in and playing (their state's low byte past 2).
fn choice_player(cpu: &mut Cpu, k: usize) -> bool {
    let p = cpu.read32(PLAYERS + 4 * k as u32);
    p != 0 && cpu.read8(p + PLAYER_STATE_LOW) as i8 > 2
}

/// The choice, a frame: after 300 frames the fades stopped and its lines typed; after 30 more,
/// up or down by a playing player turns the choice (a sound), the cursor and the route set by it;
/// a button (or 300 frames) takes it: the lines cleared and the scroll moved on. The approach's
/// plane each frame, the ships still.
pub fn next_stage_choice(cpu: &mut Cpu, w: u32) {
    let next = |cpu: &mut Cpu| {
        let step = cpu.read8(w + END_STEP).wrapping_add(1);
        cpu.write8(w + END_STEP, step);
    };
    match cpu.read8(w + END_STEP) {
        0 => {
            cpu.write32(w + CHOICE_COUNT, 0);
            next(cpu);
        }
        1 => {
            let n = cpu.read32(w + CHOICE_COUNT).wrapping_add(1);
            cpu.write32(w + CHOICE_COUNT, n);
            if n == CHOICE_WAIT {
                next(cpu);
                nested!(cpu, 0x0605_e9d8, screen::stop_fades(cpu));
                cpu.write32(w + CHOICE_COUNT, 0);
                next(cpu);
                let (s, at) = CHOICE_TITLE;
                nested!(cpu, 0x0606_14b0, text::typewriter_set(cpu, s, at, 1, 0));
                for (s, at) in CHOICE_LINES {
                    nested!(cpu, 0x0606_14b0, text::typewriter_set(cpu, s, at, 1, 0));
                }
            }
        }
        2 => {
            if cpu.read8(CHOICE_FLAG) != 0 {
                next(cpu);
            }
        }
        3 => {
            let n = cpu.read32(w + CHOICE_COUNT).wrapping_add(1);
            cpu.write32(w + CHOICE_COUNT, n);
            if n as i32 > CHOICE_FROM {
                next_stage_choosing(cpu, w);
            }
        }
        _ => {}
    }
    nested!(cpu, 0x0609_b688, approach_plane(cpu, w));
    nested!(cpu, 0x0606_16b8, player::ships_still(cpu));
}

fn next_stage_choosing(cpu: &mut Cpu, w: u32) {
    for (k, pad) in PADS_PRESSED.into_iter().enumerate() {
        if choice_player(cpu, k) && cpu.read16(pad) & PAD_UP_DOWN != 0 {
            let choice = cpu.read8(w + CHOICE) ^ 1;
            cpu.write8(w + CHOICE, choice);
            nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 0));
        }
    }
    let choice = (cpu.read8(w + CHOICE) != 0) as usize;
    let mut cursors = [CHOICE_NO_CURSOR; 2];
    cursors[choice] = CHOICE_CURSOR;
    for ((_, at), c) in CHOICE_LINES.into_iter().zip(cursors) {
        nested!(cpu, 0x0606_111c, text::put_large_char(cpu, c, at, 1));
    }
    for p in 0..2 {
        cpu.write8(PLAY_DATA + PLAY_DATA_SIZE * p + PLAY_ROUTE, choice as u8);
    }
    cpu.write8(NEXT_MODE, NEXT_STAGE_MODES[choice]);
    let mut taken = false;
    for (k, pad) in PADS_PRESSED.into_iter().enumerate() {
        if choice_player(cpu, k) && cpu.read16(pad) & PAD_BUTTONS != 0 {
            taken = true;
        }
    }
    if cpu.read32(w + CHOICE_COUNT) as i32 > CHOICE_LAST {
        taken = true;
    }
    if taken {
        let at = CHOICE_TITLE.1;
        nested!(cpu, 0x0606_14b0, text::typewriter_set(cpu, CHOICE_TITLE_BLANK, at, 1, 0));
        for (_, at) in CHOICE_LINES {
            nested!(cpu, 0x0606_14b0, text::typewriter_set(cpu, CHOICE_LINE_BLANK, at, 1, 0));
        }
        nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
    }
}

/// The stage's mode done (the director's).
pub fn stage_done(cpu: &mut Cpu) {
    nested!(cpu, 0x0604_0c64, director::mode_done(cpu));
}

/// After the choice, a frame: the approach's plane, the ships still.
pub fn after_choice(cpu: &mut Cpu, w: u32) {
    nested!(cpu, 0x0609_b688, approach_plane(cpu, w));
    nested!(cpu, 0x0606_16b8, player::ships_still(cpu));
}

/// The scroll moved on.
pub fn scroll_on(cpu: &mut Cpu) {
    nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
}

// ---- the stage boss ---------------------------------------------------------------------------------

/// The numbered objects (actors.rs's NUMBERED: an object by its number).
const NUMBERED: u32 = 0x0607_40f0;
/// The screen's scroll (screen.rs's SCROLL_INDEX, a byte) the stage boss waits for.
const SCREEN_SCROLL_INDEX: u32 = 0x51;
const SB_SCROLL: i8 = 23;
/// The stage boss: its parts' numbers (+0x70 to +0x75: four arms (0x0609d2b0's) and two guns
/// (0x0609e3f0's)), a byte at +0x6c, its attack script (+0xb4: one of 0x060a4088's five, by
/// 0x060a4484's byte), its rise (+0xa8, 16.16 of the way) and its height (+0xc8).
const SB_PARTS: u32 = 0x70;
const SB_6C: u32 = 0x6c;
const SB_SCRIPT: u32 = 0xb4;
const SB_RISE: u32 = 0xa8;
const SB_HEIGHT: u32 = 0xc8;
const SB_SCRIPTS: u32 = 0x060a_4088;
const SB_SCRIPT_INDEX: u32 = 0x060a_4484;
const SB_BEHAVIOUR: u32 = 0x0609_be58;
const SB_DRAW: u32 = 0x0609_c40c;
const SB_KIND: u32 = 43;
/// Each arm's place (x, y), turn (z) and its model (kind 43's), each gun's (x, y, z) and model.
const SB_ARMS: [(u32, u32, u32, u32); 4] = [
    (0xffd8_0000, 0x4_0000, 0xffdc_0000, 1),
    (0xffd8_0000, 0x4_0000, 0x000c_0000, 2),
    (0x0030_0000, 0x4_0000, 0xffdc_0000, 0),
    (0x0030_0000, 0x4_0000, 0, 4),
];
const SB_GUNS: [(u32, u32, u32, u32); 2] =
    [(0xffe0_0000, 0xffe8_0000, 0x0010_0000, 5), (0x0020_0000, 0xffe8_0000, 0x0010_0000, 6)];
/// The screen's byte +0x10 (set as the boss comes), the boss's sound.
const SCREEN_10: u32 = 0x10;
const SB_SOUND: u32 = 84;

/// The stage boss waiting, a frame: once the screen's scroll is 23, made: its behaviour (rising)
/// and draw, its arms and guns made (their numbers kept), its byte +0x6c -1, its attack script,
/// its rise from none; the screen's byte +0x10 set; its sound.
pub fn sb_wait(cpu: &mut Cpu, o: u32) {
    let s = cpu.read32(screen::SCREEN);
    if cpu.read8(s + SCREEN_SCROLL_INDEX) as i8 != SB_SCROLL {
        return;
    }
    let number = cpu.read8(o + NUMBER) as i8 as i32 as u32;
    cpu.write32(o + BEHAVIOUR, SB_BEHAVIOUR);
    cpu.write32(o + MODEL, SB_DRAW);
    let models = |cpu: &mut Cpu, k: u32| {
        let file = cpu.read32(KINDS + K_SIZE * SB_KIND);
        let list = cpu.read32(file + KF_MODELS);
        cpu.read32(list + 4 * k)
    };
    for (k, (x, y, z, model)) in SB_ARMS.into_iter().enumerate() {
        let model = models(cpu, model);
        cpu.push(&[z, model]);
        let n = nested!(cpu, 0x0609_d2b0, sb_arm_set(cpu, number, k as u32, x, y, z, model));
        cpu.write8(o + SB_PARTS + k as u32, n as u8);
    }
    cpu.pop(32);
    for (k, (x, y, z, model)) in SB_GUNS.into_iter().enumerate() {
        let model = models(cpu, model);
        cpu.push(&[z, model]);
        let n = nested!(cpu, 0x0609_e3f0, sb_gun_set(cpu, number, k as u32, x, y, z, model));
        cpu.write8(o + SB_PARTS + 4 + k as u32, n as u8);
    }
    cpu.write8(o + SB_6C, 0xff);
    let index = cpu.read8(SB_SCRIPT_INDEX) as i8 as i32 as u32;
    let script = cpu.read32(SB_SCRIPTS + 4 * index);
    cpu.write32(o + SB_SCRIPT, script);
    cpu.write32(o + SB_RISE, 0);
    let s = cpu.read32(screen::SCREEN);
    cpu.write8(s + SCREEN_10, 1);
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, SB_SOUND));
}

/// Where the boss rises from and to (its height), and its gauge's most.
const SB_FROM: u32 = 0x0800_0000;
const SB_TO: u32 = 0xffd0_0000;
const SB_LIFE: u32 = 90_000;
const SB_FIGHT: u32 = 0x0609_bf88;

/// The stage boss rising, a frame (1/128 of the way): its height eased up, the sea's colours
/// blended with it; at the top its behaviour the fight's, it and its parts hit, its gauge set
/// (90000) and its first move.
pub fn sb_rise(cpu: &mut Cpu, o: u32) {
    let t = cpu.read32(o + SB_RISE).wrapping_add(0x200);
    cpu.write32(o + SB_RISE, t);
    let h = nested!(cpu, 0x0606_2ebc, math::ease(cpu, SB_FROM, SB_TO, t));
    cpu.write32(o + SB_HEIGHT, h);
    let t = cpu.read32(o + SB_RISE) as i32;
    cpu.write16(SEA_BLEND, (t >> 4) as u16);
    nested!(cpu, 0x0600_5ba4, palette::colours_blend(cpu, SEA_BLEND));
    nested!(cpu, 0x0600_5b18, palette::set_colours(cpu, NBG1_BLENDED, 0, NBG1_COLOUR_COUNT));
    if cpu.read32(o + SB_RISE) != 0x1_0000 {
        return;
    }
    cpu.write32(o + SB_RISE, 0);
    cpu.write32(o + SB_HEIGHT, SB_TO);
    cpu.write32(o + BEHAVIOUR, SB_FIGHT);
    let flags = cpu.read32(o + FLAGS) & !NO_HITS;
    cpu.write32(o + FLAGS, flags);
    for k in 0..6 {
        let n = cpu.read8(o + SB_PARTS + k) as i8 as i32 as u32;
        let part = cpu.read32(NUMBERED + 4 * n);
        let flags = cpu.read32(part + FLAGS) & !NO_HITS;
        cpu.write32(part + FLAGS, flags);
    }
    nested!(cpu, 0x0604_64d8, hud::boss_gauge_set(cpu, SB_LIFE));
    nested!(cpu, 0x0609_c564, sb_next_move(cpu, o));
}

/// The stage boss's fight fields: its move (+0x6d: an index in 0x060a429c's moves), a burst's
/// state (+0x6f), a byte kept from the frame before (+0x76 from +0x77, which each frame starts
/// -1), its turn about z's last 16 frames (+0x7e, words, the next at +0x78) and the oldest of them
/// (+0x7c), its lean (TURN_Y) following its turn's change, its bob (+0xb8, from the phase at
/// +0xa2), the frames left of its move (+0xa8: -1 none), and two fields its burst drives (+0xbc,
/// +0xd8: up to 0x100000).
const SB_MOVE: u32 = 0x6d;
const SB_BURST: u32 = 0x6f;
const SB_KEPT: u32 = 0x76;
const SB_THIS_FRAME: u32 = 0x77;
const SB_TURNS_AT: u32 = 0x78;
const SB_TURN_WAS: u32 = 0x7c;
const SB_TURNS: u32 = 0x7e;
const SB_BOB_PHASE: u32 = 0xa2;
const SB_BOB: u32 = 0xb8;
const SB_MOVE_LEFT: u32 = 0xa8;
const SB_BC: u32 = 0xbc;
const SB_D8: u32 = 0xd8;
const SB_MOVES: u32 = 0x060a_429c;
const SB_FIRST_SCRIPTS: u32 = 0x060a_422c;
const SB_DOWNED: u32 = 0x0609_c868;
const SB_UNSEEN: u32 = 0x0609_a888;
/// Its downing's bonus, the rate it adds and the points to whoever downed it.
const SB_BONUS: u32 = 50_000;
const SB_RATE: u32 = 50;
const SB_POINTS: u32 = 10_000;
const SB_BURST_SOUND: u32 = 83;

/// The stage boss's fight, a frame: its gauge; downed once its life is gone (its bonus 50000 and
/// rate 50, 10000 points to whoever downed it); else, in the boss's first phase, its first script
/// (one of 0x060a422c's) and its first move; its move; its bob; its lean toward its turn's change
/// (64 a frame one way, 32 the other, to 0x800; else back to none); its turns kept; its burst (3:
/// the sound and +0xd8 0x20000 more; 2: 0x1000 more; 1: +0xbc 0x1000 less to none; then +0xd8
/// 0x1000 less to none); its move's frames counted down, the next move at none.
pub fn sb_fight(cpu: &mut Cpu, o: u32) {
    let life = cpu.read32(o + HIT_LIFE);
    nested!(cpu, 0x0604_64b8, hud::boss_gauge_life(cpu, life));
    if cpu.read32(o + HIT_LIFE) as i32 <= 0 {
        cpu.call(SB_DOWNED, &[o]);
        cpu.write32(STAGE_SCORE_AT, SB_BONUS);
        let rate = cpu.read32(STAGE_RATE_AT).wrapping_add(SB_RATE);
        cpu.write32(STAGE_RATE_AT, rate);
        let by = cpu.read32(o + HIT_BY);
        let player = cpu.read8(by + HIT_PLAYER) as i8 as i32 as u32;
        let weapon = cpu.read8(by + HIT_WEAPON) as i8 as i32 as u32;
        nested!(cpu, 0x0604_3108, hits::score_kill(cpu, player, weapon, o, SB_POINTS));
        return;
    }
    if cpu.read8(BOSS_PHASE) == 0 {
        cpu.write8(o + SB_6C, 0xff);
        let index = cpu.read8(SB_SCRIPT_INDEX) as i8 as i32 as u32;
        let script = cpu.read32(SB_FIRST_SCRIPTS + 4 * index);
        cpu.write32(o + SB_SCRIPT, script);
        cpu.call(SB_UNSEEN, &[]);
        nested!(cpu, 0x0609_c564, sb_next_move(cpu, o));
        let phase = cpu.read8(BOSS_PHASE).wrapping_sub(1);
        cpu.write8(BOSS_PHASE, phase);
    }
    let mv = cpu.read8(o + SB_MOVE) as i8 as i32 as u32;
    let f = cpu.read32(SB_MOVES + 4 * mv);
    cpu.call(f, &[o]);
    let phase = cpu.read16(o + SB_BOB_PHASE).wrapping_add(0xc0);
    cpu.write16(o + SB_BOB_PHASE, phase);
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, phase as i16 as u32));
    let bob = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, s, 0x10_0000));
    cpu.write32(o + SB_BOB, bob);
    let at = cpu.read8(o + SB_TURNS_AT) as i8 as i32 as u32;
    let change = cpu.read16(o + SB_TURNS + 2 * at).wrapping_sub(cpu.read16(o + SB_TURN_WAS)) as i16;
    let lean = cpu.read16(o + TURN_Y) as i16;
    let lean = match change.signum() {
        1 => lean.wrapping_sub(64).max(-0x800),
        -1 => lean.wrapping_add(32).min(0x800),
        _ => lean.wrapping_sub(lean.signum() * 32),
    };
    cpu.write16(o + TURN_Y, lean as u16);
    let at = cpu.read8(o + SB_TURNS_AT) as i8 as i32 as u32;
    let oldest = cpu.read16(o + SB_TURNS + 2 * at);
    cpu.write16(o + SB_TURN_WAS, oldest);
    let turn = cpu.read16(o + TURN_Z);
    cpu.write16(o + SB_TURNS + 2 * at, turn);
    cpu.write8(o + SB_TURNS_AT, ((at + 1) & 15) as u8);
    let this = cpu.read8(o + SB_THIS_FRAME);
    cpu.write8(o + SB_KEPT, this);
    cpu.write8(o + SB_THIS_FRAME, 0xff);
    let burst_down = |cpu: &mut Cpu| {
        let b = cpu.read8(o + SB_BURST).wrapping_sub(1);
        cpu.write8(o + SB_BURST, b);
    };
    let ease_d8 = |cpu: &mut Cpu| {
        let v = cpu.read32(o + SB_D8);
        if v != 0 {
            cpu.write32(o + SB_D8, (v as i32 - 0x1000).max(0) as u32);
        }
    };
    match cpu.read8(o + SB_BURST) {
        3 => {
            nested!(cpu, 0x0600_50d4, sound::se_set(cpu, SB_BURST_SOUND));
            let v = cpu.read32(o + SB_D8).wrapping_add(0x2_0000);
            cpu.write32(o + SB_D8, v);
            nested!(cpu, 0x0609_e9ec, sb_burst_step(cpu, o));
            burst_down(cpu);
        }
        2 => {
            nested!(cpu, 0x0609_e9ec, sb_burst_step(cpu, o));
            let v = cpu.read32(o + SB_D8).wrapping_add(0x1000);
            cpu.write32(o + SB_D8, v);
        }
        1 => {
            nested!(cpu, 0x0609_e9ec, sb_burst_step(cpu, o));
            let v = cpu.read32(o + SB_BC).wrapping_add(0xffff_f000);
            cpu.write32(o + SB_BC, v);
            if (v as i32) < 0 {
                cpu.write32(o + SB_BC, 0);
                burst_down(cpu);
            }
            ease_d8(cpu);
        }
        0 => ease_d8(cpu),
        _ => {}
    }
    if cpu.read32(o + SB_D8) as i32 > 0x10_0000 {
        cpu.write32(o + SB_D8, 0x10_0000);
    }
    let left = cpu.read32(o + SB_MOVE_LEFT);
    if left != u32::MAX {
        let left = left.wrapping_sub(1);
        cpu.write32(o + SB_MOVE_LEFT, left);
        if left == 0 {
            nested!(cpu, 0x0609_c564, sb_next_move(cpu, o));
        }
    }
}

/// Where the boss's sinking's blasts go (within 80 dots of 40 dots up and left of it), the big one's frame and its end's.
const SB_BLAST_SPREAD: u32 = 0x50_0000;
const SB_BLAST_OFFSET: u32 = 0xffd8_0000;
const SB_BLOWN_AT: u32 = 150;
const SB_GONE_AT: u32 = 300;

/// The stage boss sinking, a frame: down 2 dots a frame and back (z), turning; for its first 150
/// frames an explosion about it every fourth frame (by the frame count: 0x0609ef14's, or
/// 0x0609edac's of one kind or the other); at 150 a big explosion; at 300 it and its parts done.
pub fn sb_sinking(cpu: &mut Cpu, o: u32) {
    let h = cpu.read32(o + SB_HEIGHT).wrapping_add(0x2_0000);
    cpu.write32(o + SB_HEIGHT, h);
    let z = cpu.read32(o + POSITION + 8).wrapping_add(0x28f);
    cpu.write32(o + POSITION + 8, z);
    for at in [TURN_Y, TURN_Z, SB_TURN_WAS] {
        let t = cpu.read16(o + at).wrapping_add(0x100);
        cpu.write16(o + at, t);
    }
    let p = cpu.stack();
    if cpu.read32(o + SB_MOVE_LEFT) as i32 <= 0x95 && cpu.read32(GAME_FRAMES) & 3 == 0 {
        for k in [0, 4] {
            let r = nested!(cpu, 0x0603_4dd4, sgl::sl_random(cpu));
            let d = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, SB_BLAST_SPREAD, r));
            let v = cpu.read32(o + POSITION + k).wrapping_add(SB_BLAST_OFFSET).wrapping_add(d);
            cpu.write32(p + k, v);
        }
        let z = cpu.read32(o + POSITION + 8);
        cpu.write32(p + 8, z);
        match (cpu.read32(GAME_FRAMES) & 12) >> 2 {
            1 => nested!(cpu, 0x0609_edac, blast_small(cpu, p, 1)),
            3 => nested!(cpu, 0x0609_edac, blast_small(cpu, p, 0)),
            _ => nested!(cpu, 0x0609_ef14, blast_small_quiet(cpu, p)),
        };
    }
    if cpu.read32(o + SB_MOVE_LEFT) == SB_BLOWN_AT {
        for k in [0, 4, 8] {
            let v = cpu.read32(o + POSITION + k);
            cpu.write32(p + k, v);
        }
        cpu.write32(p + 0x3c, o);
        nested!(cpu, 0x0605_8ba4, explosions::explosion_set(cpu, p));
    }
    let n = cpu.read32(o + SB_MOVE_LEFT);
    cpu.write32(o + SB_MOVE_LEFT, n.wrapping_add(1));
    if n == SB_GONE_AT {
        cpu.write8(o + OBJ_MODE, 112);
        for k in 0..6 {
            let part = cpu.read8(o + SB_PARTS + k) as i8 as i32 as u32;
            let part = cpu.read32(NUMBERED + 4 * part);
            cpu.write8(part + OBJ_MODE, 112);
        }
    }
}

/// The boss's draw's fields: its place (+0xc0, +0xc4, with its height +0xc8 and bob), its body's
/// matrix kept (+0xdc) and its head's (+0x10c), the points it keeps on screen (its body's centre
/// 0x060a427c to its position, its head's 0x060a4270 to +0xcc); its body (kind 43's eighth model,
/// at 13/16 size), its head (the fourth, flashing when hit).
const SB_AT: u32 = 0xc0;
const SB_BODY_MATRIX: u32 = 0xdc;
const SB_HEAD_MATRIX: u32 = 0x10c;
const SB_HEAD_AT: u32 = 0xcc;
const SB_BODY_POINT: u32 = 0x060a_427c;
const SB_HEAD_POINT: u32 = 0x060a_4270;
const SB_BODY_MODEL: u32 = 7;
const SB_HEAD_MODELS: u32 = 3;
const SB_SIZE: u32 = 0xd000;

/// The stage boss drawn (given its sprite, its +0x10): in window 0, from the screen's matrix, at
/// its place (its height bobbing), a quarter turned about x, turned about y by its turn kept and
/// leaning about z, forward by +0xd8, at 13/16 size: its body drawn, its centre and matrix kept;
/// then turned about y by its turn since, its head drawn (flashing when hit), its matrix and point
/// kept.
pub fn sb_draw(cpu: &mut Cpu, s: u32) {
    let o = s.wrapping_sub(FLAGS);
    nested!(cpu, 0x0606_713c, sgl::sl_cur_window(cpu, 0));
    nested!(cpu, 0x0606_71cc, sgl::sl_push_matrix(cpu));
    let screen = cpu.read32(screen::SCREEN) + SCREEN_MATRIX;
    nested!(cpu, 0x0606_db7c, sgl::sl_load_matrix(cpu, screen));
    let x = cpu.read32(o + SB_AT);
    let y = cpu.read32(o + SB_AT + 4);
    let z = cpu.read32(o + SB_HEIGHT).wrapping_add(cpu.read32(o + SB_BOB));
    nested!(cpu, 0x0606_721c, sgl::sl_translate(cpu, x, y, z));
    nested!(cpu, 0x0606_72ec, sgl::sl_rot_x(cpu, 0x4000));
    let turn = cpu.read16(o + SB_TURN_WAS) as i16 as u32;
    nested!(cpu, 0x0606_736c, sgl::sl_rot_y(cpu, turn));
    let lean = cpu.read16(o + TURN_Y) as i16 as u32;
    nested!(cpu, 0x0606_726c, sgl::sl_rot_z(cpu, lean));
    let forward = cpu.read32(o + SB_D8);
    nested!(cpu, 0x0606_721c, sgl::sl_translate(cpu, 0, 0, forward));
    nested!(cpu, 0x0606_73ec, sgl::sl_scale(cpu, SB_SIZE, SB_SIZE, SB_SIZE));
    let models = |cpu: &mut Cpu, k: u32| {
        let file = cpu.read32(KINDS + K_SIZE * SB_KIND);
        let list = cpu.read32(file + KF_MODELS);
        cpu.read32(list + 4 * k)
    };
    let body = models(cpu, SB_BODY_MODEL);
    let body = cpu.read32(body);
    nested!(cpu, 0x0606_745c, sgl::sl_put_polygon(cpu, body));
    nested!(cpu, 0x0606_750c, sgl::sl_convert(cpu, SB_BODY_POINT, o + POSITION));
    nested!(cpu, 0x0606_db9c, sgl::sl_get_matrix(cpu, o + SB_BODY_MATRIX));
    nested!(cpu, 0x0606_71cc, sgl::sl_push_matrix(cpu));
    let flash = (cpu.read16(o + HIT_FLAGS) & FLASH) as u32;
    let since = cpu.read16(o + TURN_Z).wrapping_sub(cpu.read16(o + SB_TURN_WAS)) as i16 as u32;
    nested!(cpu, 0x0606_736c, sgl::sl_rot_y(cpu, since));
    let heads = models(cpu, SB_HEAD_MODELS);
    let head = cpu.read32(heads + 4 * flash);
    nested!(cpu, 0x0606_745c, sgl::sl_put_polygon(cpu, head));
    nested!(cpu, 0x0606_db9c, sgl::sl_get_matrix(cpu, o + SB_HEAD_MATRIX));
    nested!(cpu, 0x0606_750c, sgl::sl_convert(cpu, SB_HEAD_POINT, o + SB_HEAD_AT));
    nested!(cpu, 0x0606_7200, sgl::sl_pop_matrix(cpu));
    nested!(cpu, 0x0606_7200, sgl::sl_pop_matrix(cpu));
    nested!(cpu, 0x0606_713c, sgl::sl_cur_window(cpu, 1));
}

/// The boss's script (+0xb4: entries of 20 bytes, +0x6c the one it is on): its move (+0, a byte:
/// -1 the script's end), the screen's byte +0x10 and words +0x24, +0x26 (+1, +2, +4: unless +1 is
/// -1), each arm pair's step (+6: the first and third arms, +7: the second and fourth; -1 left as
/// they are) and word (+8, +10), the boss's word +0xa0 (+12) and its move's frames (+16).
const SB_ENTRY: u32 = 20;
const SB_E_MOVE: u32 = 0;
const SB_E_SCREEN: u32 = 1;
const SB_E_SCREEN_24: u32 = 2;
const SB_E_SCREEN_26: u32 = 4;
const SB_E_ARMS: u32 = 6;
const SB_E_ARMS_WORD: u32 = 8;
const SB_E_A0: u32 = 12;
const SB_E_FRAMES: u32 = 16;
/// The boss's fields a move starts afresh: +0x6e (a byte), +0x7a (a word), +0xa0 (the entry's
/// word), +0xa4 (a word, -0x2000), +0xac and +0xb0.
const SB_6E: u32 = 0x6e;
const SB_7A: u32 = 0x7a;
const SB_A0: u32 = 0xa0;
const SB_A4: u32 = 0xa4;
const SB_AC: u32 = 0xac;
const SB_B0: u32 = 0xb0;
/// An arm's step (+0x6e), its inner step (+0x6f), a byte +0x70, its word +0x8e and its long
/// +0x94 (the arms' 0x0609d2b0 objects).
const SB_ARM_STEP: u32 = 0x6e;
const SB_ARM_INNER: u32 = 0x6f;
const SB_ARM_70: u32 = 0x70;
const SB_ARM_WORD: u32 = 0x8e;
const SB_ARM_94: u32 = 0x94;
const SCREEN_24: u32 = 0x24;
const SCREEN_26: u32 = 0x26;

fn sb_part(cpu: &mut Cpu, o: u32, k: u32) -> u32 {
    let n = cpu.read8(o + SB_PARTS + k) as i8 as i32 as u32;
    cpu.read32(NUMBERED + 4 * n)
}

/// The boss's next move from its script: at its end, downed; else the move, its fields afresh,
/// each arm pair set going (their steps and words) unless left, all four arms' +0x94 none, the
/// screen's settings unless left.
pub fn sb_next_move(cpu: &mut Cpu, o: u32) {
    let at = cpu.read8(o + SB_6C).wrapping_add(1);
    cpu.write8(o + SB_6C, at);
    let e = cpu.read32(o + SB_SCRIPT).wrapping_add(SB_ENTRY.wrapping_mul(at as i8 as i32 as u32));
    let mv = cpu.read8(e + SB_E_MOVE);
    if mv == 0xff {
        cpu.call(SB_DOWNED, &[o]);
        return;
    }
    cpu.write8(o + SB_MOVE, mv);
    let word = cpu.read16(e + SB_E_A0);
    cpu.write16(o + SB_A0, word);
    let frames = cpu.read32(e + SB_E_FRAMES);
    cpu.write32(o + SB_MOVE_LEFT, frames);
    cpu.write32(o + SB_AC, 0);
    cpu.write32(o + SB_B0, 0);
    cpu.write8(o + SB_6E, 0);
    cpu.write16(o + SB_7A, 0);
    cpu.write8(o + SB_BURST, 0);
    cpu.write32(o + SB_BC, 0);
    cpu.write8(o + SB_THIS_FRAME, 0);
    let arms = [0, 1, 2, 3].map(|k| sb_part(cpu, o, k));
    for pair in 0..2 {
        let step = cpu.read8(e + SB_E_ARMS + pair);
        if step == 0xff {
            continue;
        }
        let (first, second) = (arms[pair as usize], arms[pair as usize + 2]);
        cpu.write8(first + SB_ARM_STEP, step);
        cpu.write8(second + SB_ARM_STEP, step);
        cpu.write8(second + SB_ARM_INNER, 0);
        cpu.write8(first + SB_ARM_INNER, 0);
        let word = cpu.read16(e + SB_E_ARMS_WORD + 2 * pair);
        cpu.write16(first + SB_ARM_WORD, word);
        cpu.write16(second + SB_ARM_WORD, word);
        cpu.write8(second + SB_ARM_70, 0);
        cpu.write8(first + SB_ARM_70, 0);
    }
    for k in [3, 2, 1, 0] {
        cpu.write32(arms[k] + SB_ARM_94, 0);
    }
    cpu.write16(o + SB_A4, 0xe000);
    let b = cpu.read8(e + SB_E_SCREEN);
    if b != 0xff {
        let s = cpu.read32(screen::SCREEN);
        cpu.write8(s + SCREEN_10, b);
        let w = cpu.read16(e + SB_E_SCREEN_24);
        cpu.write16(s + SCREEN_24, w);
        let w = cpu.read16(e + SB_E_SCREEN_26);
        cpu.write16(s + SCREEN_26, w);
    }
}

/// The boss's downed parts' behaviours (the arms', the guns'), and its own (sinking).
const SB_ARM_DOWNED: u32 = 0x0609_d654;
const SB_GUN_DOWNED: u32 = 0x0609_e5c8;
const SB_SINKING: u32 = 0x0609_c220;

/// The stage boss downed: its move's frames, burst and +0xbc none; the bullets cleared; its parts'
/// behaviours their downed ones; it and its parts not hit; its behaviour sinking; the scroll moved
/// on.
pub fn sb_downed(cpu: &mut Cpu, o: u32) {
    cpu.write32(o + SB_MOVE_LEFT, 0);
    cpu.write8(o + SB_BURST, 0);
    cpu.write32(o + SB_BC, 0);
    nested!(cpu, 0x0605_61e0, screen::bullets_cleared(cpu));
    for k in 0..6 {
        let part = sb_part(cpu, o, k);
        cpu.write32(part + BEHAVIOUR, if k < 4 { SB_ARM_DOWNED } else { SB_GUN_DOWNED });
    }
    let flags = cpu.read32(o + FLAGS) | NO_HITS;
    cpu.write32(o + FLAGS, flags);
    for k in 0..6 {
        let part = sb_part(cpu, o, k);
        let flags = cpu.read32(part + FLAGS) | NO_HITS;
        cpu.write32(part + FLAGS, flags);
    }
    cpu.write32(o + BEHAVIOUR, SB_SINKING);
    nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
}

/// The boss turned toward `t` (an object: by their places' bearing): 1 once it snapped round.
pub fn sb_turn_toward(cpu: &mut Cpu, o: u32, t: u32) -> u32 {
    let dx = cpu.read32(t + POSITION).wrapping_sub(cpu.read32(o + POSITION));
    let dy = cpu.read32(t + POSITION + 4).wrapping_sub(cpu.read32(o + POSITION + 4));
    let a = nested!(cpu, 0x0606_780c, sgl::sl_atan(cpu, dx, dy)) as i16;
    let a = a.wrapping_add(0x4000) as i32 as u32;
    nested!(cpu, 0x0609_c9c0, sb_turn_to(cpu, o, a))
}

/// The boss's turn about z toward `a` by its turn speed (+0x7a: 4 a frame more, to 0xb0 either
/// way); when it is the other way round (more than 0x7c00 off) its speed first eased off (16 a
/// frame), and once still and more than 0x7ff0 off it snaps a half turn round (1); else 0.
pub fn sb_turn_to(cpu: &mut Cpu, o: u32, a: u32) -> u32 {
    let a = a as i16;
    let d = a.wrapping_sub(cpu.read16(o + TURN_Z) as i16);
    let speed = cpu.read16(o + SB_7A) as i16;
    if (d as i32).abs() > 0x7c00 {
        if (speed as i32).abs() > 16 {
            cpu.write16(o + SB_7A, if speed > 0 { speed - 16 } else { speed + 16 } as u16);
        } else if (d as i32).abs() > 0x7ff0 {
            cpu.write16(o + TURN_Z, a.wrapping_add(i16::MIN) as u16);
            cpu.write16(o + SB_7A, 0);
            return 1;
        }
    }
    let speed = cpu.read16(o + SB_7A) as i16;
    let speed = if d >= 0 { speed.wrapping_sub(4).max(-0xb0) } else { speed.wrapping_add(4).min(0xb0) };
    cpu.write16(o + SB_7A, speed as u16);
    let turn = cpu.read16(o + TURN_Z).wrapping_add(cpu.read16(o + SB_7A));
    cpu.write16(o + TURN_Z, turn);
    0
}

/// A move: turned to its word (+0xa0); the next move once it snapped round.
pub fn sb_turn_move(cpu: &mut Cpu, o: u32) {
    let a = cpu.read16(o + SB_A0) as i16 as i32 as u32;
    if nested!(cpu, 0x0609_c9c0, sb_turn_to(cpu, o, a)) != 0 {
        nested!(cpu, 0x0609_c564, sb_next_move(cpu, o));
    }
}

/// The spin's count (+0xb0).
const SB_COUNT: u32 = SB_B0;

/// The spin's steps (+0x6e): 0 the beam started, then after 45 frames a burst (3); 1 the turn speed down 2
/// a frame to -128; 2 128 frames; 3 back up 2 a frame to none, then a burst (1); 4 15 frames, then
/// the next move.
pub fn sb_spin(cpu: &mut Cpu, o: u32) {
    let next = |cpu: &mut Cpu| {
        let step = cpu.read8(o + SB_6E).wrapping_add(1);
        cpu.write8(o + SB_6E, step);
    };
    match cpu.read8(o + SB_6E) {
        0 => {
            if cpu.read32(o + SB_COUNT) == 0 {
                nested!(cpu, 0x0609_ed4c, sb_beam_start(cpu, o));
            }
            let n = cpu.read32(o + SB_COUNT).wrapping_add(1);
            cpu.write32(o + SB_COUNT, n);
            if n == 45 {
                cpu.write8(o + SB_BURST, 3);
                cpu.write32(o + SB_COUNT, 0);
                next(cpu);
            }
        }
        1 => {
            let s = (cpu.read16(o + SB_7A) as i16).wrapping_sub(2);
            cpu.write16(o + SB_7A, s as u16);
            if s < -128 {
                cpu.write16(o + SB_7A, -128i16 as u16);
                next(cpu);
            }
        }
        2 => {
            let n = cpu.read32(o + SB_COUNT);
            cpu.write32(o + SB_COUNT, n.wrapping_add(1));
            if n == 128 {
                next(cpu);
            }
        }
        3 => {
            let s = (cpu.read16(o + SB_7A) as i16).wrapping_add(2);
            cpu.write16(o + SB_7A, s as u16);
            if s > 0 {
                cpu.write16(o + SB_7A, 0);
                cpu.write32(o + SB_COUNT, 0);
                next(cpu);
                cpu.write8(o + SB_BURST, 1);
            }
        }
        4 => {
            let n = cpu.read32(o + SB_COUNT).wrapping_add(1);
            cpu.write32(o + SB_COUNT, n);
            if n == 15 {
                nested!(cpu, 0x0609_c564, sb_next_move(cpu, o));
            }
        }
        _ => {}
    }
}

/// A move: the spin, turning one way (its turn speed added).
pub fn sb_spin_left(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0609_cab0, sb_spin(cpu, o));
    let turn = cpu.read16(o + TURN_Z).wrapping_add(cpu.read16(o + SB_7A));
    cpu.write16(o + TURN_Z, turn);
}

/// A move: the spin, turning the other way.
pub fn sb_spin_right(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0609_cab0, sb_spin(cpu, o));
    let turn = cpu.read16(o + TURN_Z).wrapping_sub(cpu.read16(o + SB_7A));
    cpu.write16(o + TURN_Z, turn);
}

/// A move, the long spin (its steps +0x6e): 0 the beam started and after 45 frames a burst (3); three
/// spins (1, 5, 9: the turn speed down to -128; 2, 6, 10: 128 frames; 3, 7, 11: back to none, the
/// screen's words +0x24 and +0x26 turned over but after the last), with waits of 180 frames
/// between (4, 8: after the second, back to the first spin while the boss's phase is -1); 12 a
/// burst (1) and 15 frames, then the next move. Turned by its speed, the other way in steps 5-8.
pub fn sb_long_spin(cpu: &mut Cpu, o: u32) {
    let step = cpu.read8(o + SB_6E);
    let next = |cpu: &mut Cpu| {
        let step = cpu.read8(o + SB_6E).wrapping_add(1);
        cpu.write8(o + SB_6E, step);
    };
    let count = |cpu: &mut Cpu| {
        let n = cpu.read32(o + SB_COUNT);
        cpu.write32(o + SB_COUNT, n.wrapping_add(1));
        n
    };
    match step {
        0 => {
            if cpu.read32(o + SB_COUNT) == 0 {
                nested!(cpu, 0x0609_ed4c, sb_beam_start(cpu, o));
            }
            let n = cpu.read32(o + SB_COUNT).wrapping_add(1);
            cpu.write32(o + SB_COUNT, n);
            if n == 45 {
                cpu.write8(o + SB_BURST, 3);
                cpu.write32(o + SB_COUNT, 0);
                next(cpu);
            }
        }
        1 | 5 | 9 => {
            let s = (cpu.read16(o + SB_7A) as i16).wrapping_sub(2);
            cpu.write16(o + SB_7A, s as u16);
            if s < -128 {
                cpu.write16(o + SB_7A, -128i16 as u16);
                next(cpu);
            }
        }
        2 | 6 | 10 => {
            if count(cpu) == 128 {
                next(cpu);
            }
        }
        3 | 7 | 11 => {
            let s = (cpu.read16(o + SB_7A) as i16).wrapping_add(2);
            cpu.write16(o + SB_7A, s as u16);
            if s > 0 {
                cpu.write16(o + SB_7A, 0);
                cpu.write32(o + SB_COUNT, 0);
                if step != 11 {
                    let sc = cpu.read32(screen::SCREEN);
                    for at in [SCREEN_24, SCREEN_26] {
                        let w = cpu.read16(sc + at).wrapping_neg();
                        cpu.write16(sc + at, w);
                    }
                }
                next(cpu);
            }
        }
        4 | 8 => {
            if step == 4 && count(cpu) == 180 {
                next(cpu);
                cpu.write32(o + SB_COUNT, 0);
            }
            if count(cpu) == 180 {
                if cpu.read8(BOSS_PHASE) == 0xff {
                    cpu.write8(o + SB_6E, 1);
                } else {
                    next(cpu);
                }
                cpu.write32(o + SB_COUNT, 0);
            }
        }
        12 => {
            if cpu.read32(o + SB_COUNT) == 0 {
                cpu.write8(o + SB_BURST, 1);
            }
            let n = cpu.read32(o + SB_COUNT).wrapping_add(1);
            cpu.write32(o + SB_COUNT, n);
            if n == 15 {
                nested!(cpu, 0x0609_c564, sb_next_move(cpu, o));
            }
        }
        _ => {}
    }
    let turn = cpu.read16(o + TURN_Z);
    let speed = cpu.read16(o + SB_7A);
    let backwards = cpu.read8(o + SB_6E).wrapping_sub(5) <= 3;
    cpu.write16(o + TURN_Z, if backwards { turn.wrapping_sub(speed) } else { turn.wrapping_add(speed) });
}

/// The fast spin (its steps +0x6e): 0 the beam started and after 45 frames a burst (3), its speed
/// none; 1 the speed down to -0xc0 (2 a frame), or with the script index 3 or 4 to -0x100 (4 a
/// frame); 2 786 frames (585 with the index 3 or 4); 3 back up 6 a frame to none, then a burst
/// (1); 4 16 frames, then the next move.
pub fn sb_fast_spin(cpu: &mut Cpu, o: u32) {
    let next = |cpu: &mut Cpu| {
        let step = cpu.read8(o + SB_6E).wrapping_add(1);
        cpu.write8(o + SB_6E, step);
    };
    let hard = |cpu: &mut Cpu| (3..=4).contains(&(cpu.read8(SB_SCRIPT_INDEX) as i8));
    match cpu.read8(o + SB_6E) {
        0 => {
            if cpu.read32(o + SB_COUNT) == 0 {
                nested!(cpu, 0x0609_ed4c, sb_beam_start(cpu, o));
            }
            let n = cpu.read32(o + SB_COUNT).wrapping_add(1);
            cpu.write32(o + SB_COUNT, n);
            if n == 45 {
                cpu.write8(o + SB_BURST, 3);
                cpu.write32(o + SB_COUNT, 0);
                cpu.write16(o + SB_7A, 0);
                next(cpu);
            }
        }
        1 => {
            let (by, most) = if hard(cpu) { (4, -0x100) } else { (2, -0xc0) };
            let s = (cpu.read16(o + SB_7A) as i16).wrapping_sub(by);
            cpu.write16(o + SB_7A, s as u16);
            if s < most {
                cpu.write16(o + SB_7A, most as u16);
                next(cpu);
            }
        }
        2 => {
            let frames = if hard(cpu) { 0x249 } else { 0x312 };
            let n = cpu.read32(o + SB_COUNT);
            cpu.write32(o + SB_COUNT, n.wrapping_add(1));
            if n == frames {
                next(cpu);
            }
        }
        3 => {
            let s = (cpu.read16(o + SB_7A) as i16).wrapping_add(6);
            cpu.write16(o + SB_7A, s as u16);
            if s > 0 {
                cpu.write16(o + SB_7A, 0);
                cpu.write32(o + SB_COUNT, 0);
                next(cpu);
                cpu.write8(o + SB_BURST, 1);
            }
        }
        4 => {
            let n = cpu.read32(o + SB_COUNT);
            cpu.write32(o + SB_COUNT, n.wrapping_add(1));
            if n == 15 {
                nested!(cpu, 0x0609_c564, sb_next_move(cpu, o));
            }
        }
        _ => {}
    }
}

/// A move: the fast spin, turning one way.
pub fn sb_fast_spin_left(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0609_cdb4, sb_fast_spin(cpu, o));
    let turn = cpu.read16(o + TURN_Z).wrapping_add(cpu.read16(o + SB_7A));
    cpu.write16(o + TURN_Z, turn);
}

/// A move: the fast spin, turning the other way.
pub fn sb_fast_spin_right(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0609_cdb4, sb_fast_spin(cpu, o));
    let turn = cpu.read16(o + TURN_Z).wrapping_sub(cpu.read16(o + SB_7A));
    cpu.write16(o + TURN_Z, turn);
}

/// The chase's check (0x0609d124's: whether to turn, given a count), by script index.
const SB_CHASE_CHECK: u32 = 0x0609_d124;

/// A move, the chase: turned toward the nearer player; while the boss's phase is 2 or less, at
/// script index 2 only when 0x0609d124 says so for 200, at 3 or 4 for 300.
pub fn sb_chase(cpu: &mut Cpu, o: u32) {
    let mut turn = true;
    if cpu.read8(BOSS_PHASE) as i8 <= 2 {
        let count = match cpu.read8(SB_SCRIPT_INDEX) as i8 {
            2 => Some(200),
            3 | 4 => Some(300),
            _ => None,
        };
        if let Some(count) = count {
            turn = cpu.call(SB_CHASE_CHECK, &[o, count]) != 0;
        }
    }
    if turn {
        let player = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + FLAGS));
        nested!(cpu, 0x0609_c984, sb_turn_toward(cpu, o, player));
    }
}

/// An arm of the stage boss (a numbered object, 0x9c bytes): its behaviour and draw, unseen and
/// not hit at first (a model drawn by its own function); its boss's number (+0x6c) and its own
/// place among the arms (+0x6d); where it is on the boss (+0x74 x, +0x78 y, +0x7c z), its model
/// (+0x98: kind 43's), its turn (+0x8c, a word, none; +0x90 the boss's turn kept with it), its
/// steps (+0x6e, +0x6f, +0x70) and +0x88, +0x8e none; side 1, its boxes (0x060a42c4) and life
/// 25000.
const SB_ARM_SIZE: u32 = 0x9c;
const SB_ARM_BEHAVIOUR: u32 = 0x0609_d408;
const SB_ARM_DRAW: u32 = 0x0609_d660;
const SB_ARM_FLAGS: u32 = IS_MODEL | OWN_DRAW | NO_HITS;
const SB_ARM_BOSS: u32 = 0x6c;
const SB_ARM_PLACE: u32 = 0x6d;
const SB_ARM_AT: u32 = 0x74;
const SB_ARM_MODEL: u32 = 0x98;
const SB_ARM_TURN: u32 = 0x8c;
const SB_ARM_BOSS_TURN: u32 = 0x90;
const SB_ARM_88: u32 = 0x88;
const SB_ARM_BOXES: u32 = 0x060a_42c4;
const SB_ARM_LIFE: u32 = 25_000;
const LIFT: u32 = 0x33;

/// Arm `k` of the boss numbered `boss` made at (x, y, z) on it with `model`: its number, or 0.
pub fn sb_arm_set(cpu: &mut Cpu, boss: u32, k: u32, x: u32, y: u32, z: u32, model: u32) -> u32 {
    let (boss, k) = (boss as i8, k as u8);
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, ENEMY_NUMBERS)) as i8;
    if n == -1 {
        return 0;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, SB_ARM_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, NUMBERED_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HIT_BOXES));
    cpu.write32(o + BEHAVIOUR, SB_ARM_BEHAVIOUR);
    for k in [8, 4, 0] {
        cpu.write32(o + POSITION + k, 0);
    }
    let flags = cpu.read32(o + FLAGS) | SB_ARM_FLAGS;
    cpu.write32(o + FLAGS, flags);
    cpu.write32(o + MODEL, SB_ARM_DRAW);
    cpu.write8(o + LIFT, 0);
    for (at, v) in [(0, x), (4, y), (8, z)] {
        cpu.write32(o + SB_ARM_AT + at, v);
    }
    cpu.write8(o + SB_ARM_PLACE, k);
    cpu.write8(o + SB_ARM_BOSS, boss as u8);
    cpu.write32(o + SB_ARM_MODEL, model);
    cpu.write16(o + SB_ARM_TURN, 0);
    cpu.write8(o + SB_ARM_INNER, 0);
    cpu.write16(o + SB_ARM_WORD, 0);
    cpu.write8(o + SB_ARM_70, 0);
    cpu.write8(o + SB_ARM_STEP, 0);
    let parent = cpu.read8(o + SB_ARM_BOSS) as i8 as i32 as u32;
    let parent = cpu.read32(NUMBERED + 4 * parent);
    let turn = cpu.read16(parent + SB_TURN_WAS).wrapping_add(cpu.read16(o + SB_ARM_TURN));
    cpu.write16(o + SB_ARM_BOSS_TURN, turn);
    cpu.write32(o + SB_ARM_88, 0);
    cpu.write8(o + HIT_SIDE, 1);
    cpu.write32(o + HIT_BOXES, SB_ARM_BOXES);
    cpu.write32(o + HIT_LIFE, SB_ARM_LIFE);
    cpu.write8(o + NUMBER, n as u8);
    let n = cpu.read8(o + NUMBER) as i8 as i32;
    cpu.write32(NUMBERED.wrapping_add((4 * n) as u32), o);
    n as u32
}

/// The arm's steps (0x060a4314's, by its step +0x6e), its dying behaviour, the rate and points
/// its downing gives.
const SB_ARM_STEPS: u32 = 0x060a_4314;
const SB_ARM_DYING: u32 = 0x0609_d524;
const SB_ARM_GONE: u32 = 0x0609_d654;
const SB_ARM_RATE: u32 = 10;
const SB_ARM_POINTS: u32 = 10_000;
/// The arm's place's bit for the far side (its lean the other way).
const SB_ARM_FAR: u8 = 2;

/// An arm, a frame: downed once its life is gone (dying, not hit, +0x94 none, its rate and 10000
/// points to whoever downed it); else its step (given the boss); then its lean: from its own turn (+0x8c, the other
/// way on the far side) while +0x70 is none, else from its turn kept less the boss's (its own turn
/// following); its turn kept the boss's plus its lean.
pub fn sb_arm_behaviour(cpu: &mut Cpu, o: u32) {
    if cpu.read32(o + HIT_LIFE) as i32 <= 0 {
        cpu.write32(o + BEHAVIOUR, SB_ARM_DYING);
        let flags = cpu.read32(o + FLAGS) | NO_HITS;
        cpu.write32(o + FLAGS, flags);
        cpu.write32(o + SB_ARM_94, 0);
        let rate = cpu.read32(STAGE_RATE_AT).wrapping_add(SB_ARM_RATE);
        cpu.write32(STAGE_RATE_AT, rate);
        let by = cpu.read32(o + HIT_BY);
        let player = cpu.read8(by + HIT_PLAYER) as i8 as i32 as u32;
        let weapon = cpu.read8(by + HIT_WEAPON) as i8 as i32 as u32;
        nested!(cpu, 0x0604_3108, hits::score_kill(cpu, player, weapon, o, SB_ARM_POINTS));
        return;
    }
    let step = cpu.read8(o + SB_ARM_STEP) as i8 as i32 as u32;
    let f = cpu.read32(SB_ARM_STEPS + 4 * step);
    let n = cpu.read8(o + SB_ARM_BOSS) as i8 as i32 as u32;
    let boss = cpu.read32(NUMBERED + 4 * n);
    cpu.call(f, &[o, boss]);
    let far = cpu.read8(o + SB_ARM_PLACE) & SB_ARM_FAR != 0;
    let boss = |cpu: &mut Cpu| {
        let n = cpu.read8(o + SB_ARM_BOSS) as i8 as i32 as u32;
        cpu.read32(NUMBERED + 4 * n)
    };
    if cpu.read8(o + SB_ARM_70) == 0 {
        let turn = cpu.read16(o + SB_ARM_TURN);
        cpu.write16(o + TURN_Y, if far { turn.wrapping_neg() } else { turn });
    } else {
        let b = boss(cpu);
        let lean = cpu.read16(o + SB_ARM_BOSS_TURN).wrapping_sub(cpu.read16(b + SB_TURN_WAS));
        cpu.write16(o + TURN_Y, lean);
        cpu.write16(o + SB_ARM_TURN, if far { lean.wrapping_neg() } else { lean });
    }
    let b = boss(cpu);
    let turn = cpu.read16(b + SB_TURN_WAS).wrapping_add(cpu.read16(o + TURN_Y));
    cpu.write16(o + SB_ARM_BOSS_TURN, turn);
}

/// An arm dying, a frame: a blast within 24 dots of it every fourth frame (by the frame count's
/// kind); at 60 frames a big one within 32 dots, the boss's phase a step on, and it gone.
pub fn sb_arm_dying(cpu: &mut Cpu, o: u32) {
    let p = cpu.stack();
    let place = |cpu: &mut Cpu, spread: u32, offset: u32| {
        for k in [0, 4] {
            let r = nested!(cpu, 0x0603_4dd4, sgl::sl_random(cpu));
            let d = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, spread, r));
            let v = cpu.read32(o + POSITION + k).wrapping_add(offset).wrapping_add(d);
            cpu.write32(p + k, v);
        }
        let z = cpu.read32(o + POSITION + 8);
        cpu.write32(p + 8, z);
    };
    if cpu.read32(GAME_FRAMES) & 3 == 0 {
        place(cpu, 0x18_0000, 0xfff4_0000);
        match (cpu.read32(GAME_FRAMES) & 12) >> 2 {
            1 => nested!(cpu, 0x0609_ee9c, blast_large(cpu, p, 1)),
            3 => nested!(cpu, 0x0609_ee9c, blast_large(cpu, p, 0)),
            _ => nested!(cpu, 0x0609_efcc, blast_large_quiet(cpu, p)),
        };
    }
    let n = cpu.read32(o + SB_ARM_94);
    cpu.write32(o + SB_ARM_94, n.wrapping_add(1));
    if n == 60 {
        place(cpu, 0x20_0000, 0xfff0_0000);
        nested!(cpu, 0x0609_edac, blast_small(cpu, p, 1));
        let phase = cpu.read8(BOSS_PHASE).wrapping_sub(1);
        cpu.write8(BOSS_PHASE, phase);
        cpu.write32(o + BEHAVIOUR, SB_ARM_GONE);
    }
}

/// A gone arm's behaviour: nothing.
pub fn sb_arm_gone(_cpu: &mut Cpu, _o: u32) {}

/// The arm's draw: its muzzle's point (0x060a42d4, kept at +0x80 x, +0x84 y), its broken model
/// (kind 43's tenth).
const SB_ARM_MUZZLE_POINT: u32 = 0x060a_42d4;
const SB_ARM_MUZZLE: u32 = 0x80;
const SB_ARM_BROKEN_MODEL: u32 = 9;

/// An arm drawn (given its sprite): in window 0, from the boss's body's matrix, its place on
/// screen kept; moved to its place on the boss, turned about y by its lean, at 13/16 size; broken
/// once its life is gone, else its model (flashing when hit) and its muzzle's place kept.
pub fn sb_arm_draw(cpu: &mut Cpu, s: u32) {
    let o = s.wrapping_sub(FLAGS);
    let boss = cpu.read8(o + SB_ARM_BOSS) as i8 as i32 as u32;
    let boss = cpu.read32(NUMBERED + 4 * boss);
    nested!(cpu, 0x0606_713c, sgl::sl_cur_window(cpu, 0));
    nested!(cpu, 0x0606_71cc, sgl::sl_push_matrix(cpu));
    nested!(cpu, 0x0606_db7c, sgl::sl_load_matrix(cpu, boss + SB_BODY_MATRIX));
    nested!(cpu, 0x0606_750c, sgl::sl_convert(cpu, o + SB_ARM_AT, o + POSITION));
    let [x, y, z] = [0, 4, 8].map(|k| cpu.read32(o + SB_ARM_AT + k));
    nested!(cpu, 0x0606_721c, sgl::sl_translate(cpu, x, y, z));
    let lean = cpu.read16(o + TURN_Y) as i16 as u32;
    nested!(cpu, 0x0606_736c, sgl::sl_rot_y(cpu, lean));
    nested!(cpu, 0x0606_73ec, sgl::sl_scale(cpu, SB_SIZE, SB_SIZE, SB_SIZE));
    if cpu.read32(o + HIT_LIFE) as i32 <= 0 {
        let file = cpu.read32(KINDS + K_SIZE * SB_KIND);
        let list = cpu.read32(file + KF_MODELS);
        let broken = cpu.read32(list + 4 * SB_ARM_BROKEN_MODEL);
        let model = cpu.read32(broken);
        nested!(cpu, 0x0606_745c, sgl::sl_put_polygon(cpu, model));
    } else {
        let flash = (cpu.read16(o + HIT_FLAGS) & FLASH) as u32;
        let models = cpu.read32(o + SB_ARM_MODEL);
        let model = cpu.read32(models + 4 * flash);
        nested!(cpu, 0x0606_745c, sgl::sl_put_polygon(cpu, model));
        nested!(cpu, 0x0606_750c, sgl::sl_convert(cpu, SB_ARM_MUZZLE_POINT, o + SB_ARM_MUZZLE));
    }
    nested!(cpu, 0x0606_7200, sgl::sl_pop_matrix(cpu));
    nested!(cpu, 0x0606_713c, sgl::sl_cur_window(cpu, 1));
}

/// An arm's turn (+0x8c) toward `a` by 0x100 a frame: 1 once there.
pub fn sb_arm_turn_to(cpu: &mut Cpu, o: u32, a: u32) -> u32 {
    let turn = cpu.read16(o + SB_ARM_TURN) as i16;
    let d = (a as i16).wrapping_sub(turn);
    if d < -0x100 {
        cpu.write16(o + SB_ARM_TURN, turn.wrapping_sub(0x100) as u16);
        0
    } else if d > 0x100 {
        cpu.write16(o + SB_ARM_TURN, turn.wrapping_add(0x100) as u16);
        0
    } else {
        cpu.write16(o + SB_ARM_TURN, d.wrapping_add(turn) as u16);
        1
    }
}

/// An arm turned toward the player it aims at (16 frames ahead: aim.rs's target), by its bearing
/// from the arm less the boss's turn (a quarter turn on, mirrored on the far side): 1 once there.
pub fn sb_arm_aim(cpu: &mut Cpu, o: u32) -> u32 {
    let t = cpu.stack();
    nested!(cpu, 0x0604_0ccc, aim::target(cpu, o, 16, t));
    let dx = cpu.read32(t).wrapping_sub(cpu.read32(o + POSITION));
    cpu.write32(t, dx);
    let dy = cpu.read32(t + 4).wrapping_sub(cpu.read32(o + POSITION + 4));
    cpu.write32(t + 4, dy);
    let a = nested!(cpu, 0x0606_780c, sgl::sl_atan(cpu, dx, dy)) as i16;
    let boss = cpu.read8(o + SB_ARM_BOSS) as i8 as i32 as u32;
    let boss = cpu.read32(NUMBERED + 4 * boss);
    let a = a.wrapping_sub(cpu.read16(boss + SB_TURN_WAS) as i16);
    let far = cpu.read8(o + SB_ARM_PLACE) & SB_ARM_FAR != 0;
    let a = if far { 0x4000i16.wrapping_sub(a) } else { a.wrapping_sub(0x4000) };
    nested!(cpu, 0x0609_d760, sb_arm_turn_to(cpu, o, a as i32 as u32))
}

/// A shot from an arm (bullets.rs's description `d`): its number, from its muzzle.
pub fn sb_arm_shot_from(cpu: &mut Cpu, d: u32, o: u32) {
    let n = cpu.read8(o + NUMBER);
    cpu.write8(d + SHOT_NUMBER, n);
    let x = cpu.read32(o + SB_ARM_MUZZLE);
    cpu.write32(d, x);
    let y = cpu.read32(o + SB_ARM_MUZZLE + 4);
    cpu.write32(d + 4, y);
    cpu.write32(d + 8, 0);
}

/// An arm's spread: four shots from its muzzle, about its turn kept (half turned round), 40 and
/// 13 degrees either side.
const SB_ARM_SPREAD: [u16; 4] = [0xe38f, 0xf68b, 0x0975, 0x1c71];

pub fn sb_arm_spread(cpu: &mut Cpu, o: u32) {
    let d = cpu.stack();
    nested!(cpu, 0x0609_d814, sb_arm_shot_from(cpu, d, o));
    let a = cpu.read16(o + SB_ARM_BOSS_TURN).wrapping_add(0x8000);
    for off in SB_ARM_SPREAD {
        cpu.write16(d + SHOT_ANGLE, a.wrapping_add(off));
        nested!(cpu, 0x0605_3f48, bullets::bullet_set_plain(cpu, d));
    }
}


/// An arm's stray shot: from its muzzle, half turned round from its turn kept and up to 0x3000
/// either way (random, in steps of 0x400), at 1 to 1.75 dots a frame (random, in quarters).
pub fn sb_arm_stray_shot(cpu: &mut Cpu, o: u32) {
    let d = cpu.stack();
    nested!(cpu, 0x0609_d814, sb_arm_shot_from(cpu, d, o));
    let r = nested!(cpu, 0x0603_4dd4, sgl::sl_random(cpu));
    let v = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, r, 0x6000));
    let a = (v.wrapping_add(0xffff_d000) & 0xffff_fc00)
        .wrapping_add(0xffff_8000)
        .wrapping_add(cpu.read16(o + SB_ARM_BOSS_TURN) as i16 as i32 as u32);
    cpu.write16(d + SHOT_ANGLE, a as u16);
    cpu.write16(d + SHOT_FLAGS, 0);
    let r = nested!(cpu, 0x0603_4dd4, sgl::sl_random(cpu));
    cpu.write32(d + SHOT_SPEED, (r & 0xc000).wrapping_add(0x1_0000));
    nested!(cpu, 0x0605_3db0, bullets::bullet_set(cpu, d));
}

/// The arm's ribbons (ribbons.rs's description: its colour +0x20, heading +0x2c, turn +0x2e, speed
/// up +0x30, turn's reach +0x38, the place it follows +0x3c): their speed up and the homing one's
/// turn by the script index (0x060a42e0's longs, 0x060a42f4's words), the homing one's reach,
/// their colours (16 apart past the first character), their sound.
const RIBBON_COLOUR: u32 = 0x20;
const RIBBON_HEADING: u32 = 0x2c;
const RIBBON_TURN: u32 = 0x2e;
const RIBBON_ACCEL: u32 = 0x30;
const RIBBON_REACH: u32 = 0x38;
const RIBBON_SOURCE: u32 = 0x3c;
const SB_RIBBON_ACCELS: u32 = 0x060a_42e0;
const SB_RIBBON_TURNS: u32 = 0x060a_42f4;
const SB_RIBBON_REACH: u16 = 0x1000;
const SB_RIBBON_SOUND: u32 = 41;

/// An arm's ribbon (mover `kind`, colour `colour` past the first character, its own `flags`) from
/// its muzzle, half turned round from its turn kept, with its flare and sound.
fn sb_arm_ribbon(cpu: &mut Cpu, o: u32, kind: u32, colour: u16, flags: u32) {
    let d = cpu.stack();
    nested!(cpu, 0x0609_d814, sb_arm_shot_from(cpu, d, o));
    let heading = cpu.read16(o + SB_ARM_BOSS_TURN).wrapping_add(0x8000);
    cpu.write16(d + RIBBON_HEADING, heading);
    let index = cpu.read8(SB_SCRIPT_INDEX) as i8 as i32 as u32;
    if kind == 1 {
        let turn = cpu.read16(SB_RIBBON_TURNS + 2 * index);
        cpu.write16(d + RIBBON_TURN, turn);
        cpu.write16(d + RIBBON_REACH, SB_RIBBON_REACH);
    }
    let accel = cpu.read32(SB_RIBBON_ACCELS + 4 * index);
    cpu.write32(d + RIBBON_ACCEL, accel);
    cpu.write32(d + RIBBON_SOURCE, o + SB_ARM_MUZZLE);
    let c = cpu.read16(CHAR_BASE).wrapping_add(colour);
    cpu.write16(d + RIBBON_COLOUR, c);
    nested!(cpu, 0x0606_42e0, ribbons::ribbon_set(cpu, kind, d, flags));
    nested!(cpu, 0x0606_43ec, effects::flare_set(cpu, d));
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, SB_RIBBON_SOUND));
}

/// An arm's straight ribbon.
pub fn sb_arm_straight_ribbon(cpu: &mut Cpu, o: u32) {
    sb_arm_ribbon(cpu, o, 0, 48, 0);
}

/// An arm's homing ribbon.
pub fn sb_arm_homing_ribbon(cpu: &mut Cpu, o: u32) {
    sb_arm_ribbon(cpu, o, 1, 64, 0);
}

/// An arm's straight ribbon of the second colour (its flags 1: ending in a burst).
pub fn sb_arm_straight_ribbon_2(cpu: &mut Cpu, o: u32) {
    sb_arm_ribbon(cpu, o, 0, 16, 1);
}

/// An arm's step, pointing: the near arms (places 0 and 2) turned to their word (+0x8e), the far
/// ones aimed at the player: 1 once there.
pub fn sb_arm_point(cpu: &mut Cpu, o: u32) -> u32 {
    match cpu.read8(o + SB_ARM_PLACE) {
        0 | 2 => {
            let a = cpu.read16(o + SB_ARM_WORD) as i16 as i32 as u32;
            nested!(cpu, 0x0609_d760, sb_arm_turn_to(cpu, o, a))
        }
        _ => nested!(cpu, 0x0609_d798, sb_arm_aim(cpu, o)),
    }
}

/// An arm's step, the volley: turned to its word, its spread every 90 frames (60 past the first
/// script).
pub fn sb_arm_volley(cpu: &mut Cpu, o: u32, _boss: u32) {
    let a = cpu.read16(o + SB_ARM_WORD) as i16 as i32 as u32;
    nested!(cpu, 0x0609_d760, sb_arm_turn_to(cpu, o, a));
    let every = if cpu.read8(SB_SCRIPT_INDEX) == 0 { 90 } else { 60 };
    let n = cpu.read32(o + SB_ARM_94).wrapping_add(1);
    cpu.write32(o + SB_ARM_94, n);
    if n == every {
        nested!(cpu, 0x0609_d87c, sb_arm_spread(cpu, o));
        cpu.write32(o + SB_ARM_94, 0);
    }
}

/// An arm's step, strays: turned to its word, three stray shots every 15 frames.
pub fn sb_arm_strays(cpu: &mut Cpu, o: u32, _boss: u32) {
    let a = cpu.read16(o + SB_ARM_WORD) as i16 as i32 as u32;
    nested!(cpu, 0x0609_d760, sb_arm_turn_to(cpu, o, a));
    let n = cpu.read32(o + SB_ARM_94).wrapping_add(1);
    cpu.write32(o + SB_ARM_94, n);
    if n == 15 {
        for _ in 0..3 {
            nested!(cpu, 0x0609_d8f0, sb_arm_stray_shot(cpu, o));
        }
        cpu.write32(o + SB_ARM_94, 0);
    }
}

/// The ribbons' interval by the script index (0x060a4300's).
const SB_RIBBON_EVERY: u32 = 0x060a_4300;

/// An arm's ribbon cycle (its inner step +0x6f): 0 aiming, until its first ribbon's time (30
/// frames for the far arms, half the interval and 54 more for the near ones); 1 aiming, its ribbon
/// (`fire`) at the interval, then held (+0x70); 2 48 frames, then let go and back to 1.
fn sb_arm_ribbon_cycle(cpu: &mut Cpu, o: u32, boss: u32, fire: fn(&mut Cpu, u32)) {
    let count = |cpu: &mut Cpu| {
        let n = cpu.read32(o + SB_ARM_94).wrapping_add(1);
        cpu.write32(o + SB_ARM_94, n);
        n
    };
    let every = |cpu: &mut Cpu| {
        let index = cpu.read8(SB_SCRIPT_INDEX) as i8 as i32 as u32;
        cpu.read32(SB_RIBBON_EVERY + 4 * index)
    };
    let next = |cpu: &mut Cpu| {
        let step = cpu.read8(o + SB_ARM_INNER).wrapping_add(1);
        cpu.write8(o + SB_ARM_INNER, step);
    };
    // the SH-2 code holds the inner step, its address and the boss in r9-r11 across the aim:
    // aim.rs's target pushes them, and a straight ribbon reads one back as its turn's reach
    let inner = cpu.read8(o + SB_ARM_INNER);
    let held = [(9, inner as i8 as i32 as u32), (10, o + SB_ARM_INNER), (11, boss)];
    let aim = |cpu: &mut Cpu| cpu.holding(&held, |cpu| nested!(cpu, 0x0609_d798, sb_arm_aim(cpu, o)));
    match inner {
        0 => {
            aim(cpu);
            let first = if cpu.read8(o + SB_ARM_PLACE) & SB_ARM_FAR != 0 {
                30
            } else {
                (((every(cpu) as i32 + 48) >> 1) + 30) as u32
            };
            if count(cpu) == first {
                cpu.write32(o + SB_ARM_94, 0);
                next(cpu);
            }
        }
        1 => {
            aim(cpu);
            let n = count(cpu);
            if n == every(cpu) {
                fire(cpu, o);
                cpu.write32(o + SB_ARM_94, 0);
                cpu.write8(o + SB_ARM_70, 1);
                next(cpu);
            }
        }
        2 if count(cpu) == 48 => {
            cpu.write32(o + SB_ARM_94, 0);
            cpu.write8(o + SB_ARM_70, 0);
            cpu.write8(o + SB_ARM_INNER, 1);
        }
        _ => {}
    }
}

/// An arm's step, straight ribbons.
pub fn sb_arm_ribbons(cpu: &mut Cpu, o: u32, boss: u32) {
    sb_arm_ribbon_cycle(cpu, o, boss, |cpu, o| nested!(cpu, 0x0609_da1c, sb_arm_straight_ribbon(cpu, o)));
}

/// An arm's step, homing ribbons.
pub fn sb_arm_homing_ribbons(cpu: &mut Cpu, o: u32, boss: u32) {
    sb_arm_ribbon_cycle(cpu, o, boss, |cpu, o| nested!(cpu, 0x0609_daa4, sb_arm_homing_ribbon(cpu, o)));
}

/// An arm's step, straight ribbons of the second colour.
pub fn sb_arm_ribbons_2(cpu: &mut Cpu, o: u32, boss: u32) {
    sb_arm_ribbon_cycle(cpu, o, boss, |cpu, o| nested!(cpu, 0x0609_dbd0, sb_arm_straight_ribbon_2(cpu, o)));
}

/// A gun of the stage boss (a numbered object, 0x80 bytes): its behaviour and draw, unseen and not
/// hit at first (a model drawn by its own function); its boss's number (+0x6c) and its side
/// (+0x6d: 0 the left), where it is on the boss (+0x70 x, +0x74 y, +0x78 z), its model (+0x7c),
/// tilted 0x2aaa about x and turned out 0x1800 about y (the left one the other way); side 1, its
/// boxes (0x060a433c) and life 10000.
const SB_GUN_SIZE: u32 = 0x80;
const SB_GUN_BEHAVIOUR: u32 = 0x0609_e51c;
const SB_GUN_DRAW: u32 = 0x0609_e5d4;
const SB_GUN_BOSS: u32 = 0x6c;
const SB_GUN_SIDE: u32 = 0x6d;
const SB_GUN_AT: u32 = 0x70;
const SB_GUN_MODEL: u32 = 0x7c;
const SB_GUN_TILT: u16 = 0x2aaa;
const SB_GUN_TURN: u16 = 0x1800;
const SB_GUN_BOXES: u32 = 0x060a_433c;
const SB_GUN_LIFE: u32 = 10_000;

/// Gun `side` of the boss numbered `boss` made at (x, y, z) on it with `model`: its number, or 0.
pub fn sb_gun_set(cpu: &mut Cpu, boss: u32, side: u32, x: u32, y: u32, z: u32, model: u32) -> u32 {
    let (boss, side) = (boss as i8, side as i8);
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, ENEMY_NUMBERS)) as i8;
    if n == -1 {
        return 0;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, SB_GUN_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, NUMBERED_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HIT_BOXES));
    cpu.write32(o + BEHAVIOUR, SB_GUN_BEHAVIOUR);
    let flags = cpu.read32(o + FLAGS) | SB_ARM_FLAGS;
    cpu.write32(o + FLAGS, flags);
    cpu.write32(o + MODEL, SB_GUN_DRAW);
    for k in [8, 4, 0] {
        cpu.write32(o + POSITION + k, 0);
    }
    cpu.write8(o + LIFT, 0);
    for (at, v) in [(0, x), (4, y), (8, z)] {
        cpu.write32(o + SB_GUN_AT + at, v);
    }
    cpu.write8(o + SB_GUN_SIDE, side as u8);
    cpu.write8(o + SB_GUN_BOSS, boss as u8);
    cpu.write32(o + SB_GUN_MODEL, model);
    cpu.write16(o + TURN_X, SB_GUN_TILT);
    let turn = if side == 0 { SB_GUN_TURN.wrapping_neg() } else { SB_GUN_TURN };
    cpu.write16(o + TURN_Y, turn);
    cpu.write8(o + HIT_SIDE, 1);
    cpu.write32(o + HIT_BOXES, SB_GUN_BOXES);
    cpu.write32(o + HIT_LIFE, SB_GUN_LIFE);
    cpu.write8(o + NUMBER, n as u8);
    let n = cpu.read8(o + NUMBER) as i8 as i32;
    cpu.write32(NUMBERED.wrapping_add((4 * n) as u32), o);
    n as u32
}

/// The gun's breaking: its puffs (five about it, by its side's points: 0x060a4388's for the left,
/// 0x060a434c's for the right, turned with the boss), its gone behaviour and draw (nothing), the
/// rate and points it gives, its sound.
const SB_GUN_PUFFS: [u32; 2] = [0x060a_4388, 0x060a_434c];
const SB_GUN_GONE: u32 = 0x0609_e5c8;
const SB_GUN_NO_DRAW: u32 = 0x0609_c400;
const SB_GUN_RATE: u32 = 5;
const SB_GUN_POINTS: u32 = 10_000;
const SB_GUN_SOUND: u32 = 19;

/// A gun, a frame: once its life is gone, broken (puffs, not hit, gone, its rate and 10000 points
/// to whoever broke it, its sound).
pub fn sb_gun_behaviour(cpu: &mut Cpu, o: u32) {
    if cpu.read32(o + HIT_LIFE) as i32 > 0 {
        return;
    }
    let boss = cpu.read8(o + SB_GUN_BOSS) as i8 as i32 as u32;
    let boss = cpu.read32(NUMBERED + 4 * boss);
    let side = (cpu.read8(o + SB_GUN_SIDE) != 0) as usize;
    let turn = cpu.read16(boss + TURN_Z) as i16 as i32 as u32;
    nested!(cpu, 0x0606_2940, actors::puffs_around(cpu, o + POSITION, turn, SB_GUN_PUFFS[side], 5));
    let flags = cpu.read32(o + FLAGS) | NO_HITS;
    cpu.write32(o + FLAGS, flags);
    cpu.write32(o + BEHAVIOUR, SB_GUN_GONE);
    cpu.write32(o + MODEL, SB_GUN_NO_DRAW);
    let rate = cpu.read32(STAGE_RATE_AT).wrapping_add(SB_GUN_RATE);
    cpu.write32(STAGE_RATE_AT, rate);
    let by = cpu.read32(o + HIT_BY);
    let player = cpu.read8(by + HIT_PLAYER) as i8 as i32 as u32;
    let weapon = cpu.read8(by + HIT_WEAPON) as i8 as i32 as u32;
    nested!(cpu, 0x0604_3108, hits::score_kill(cpu, player, weapon, o, SB_GUN_POINTS));
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, SB_GUN_SOUND));
}

/// A broken gun's behaviour: nothing.
pub fn sb_gun_gone(_cpu: &mut Cpu, _o: u32) {}

/// A gun drawn (given its sprite): in window 0, from the boss's head's matrix, its place on screen
/// kept; moved to its place on the boss, tilted about x and turned about y; its model (flashing
/// when hit) unless its life is gone.
pub fn sb_gun_draw(cpu: &mut Cpu, s: u32) {
    let o = s.wrapping_sub(FLAGS);
    let boss = cpu.read8(o + SB_GUN_BOSS) as i8 as i32 as u32;
    let boss = cpu.read32(NUMBERED + 4 * boss);
    nested!(cpu, 0x0606_713c, sgl::sl_cur_window(cpu, 0));
    nested!(cpu, 0x0606_71cc, sgl::sl_push_matrix(cpu));
    nested!(cpu, 0x0606_db7c, sgl::sl_load_matrix(cpu, boss + SB_HEAD_MATRIX));
    nested!(cpu, 0x0606_750c, sgl::sl_convert(cpu, o + SB_GUN_AT, o + POSITION));
    let [x, y, z] = [0, 4, 8].map(|k| cpu.read32(o + SB_GUN_AT + k));
    nested!(cpu, 0x0606_721c, sgl::sl_translate(cpu, x, y, z));
    let tilt = cpu.read16(o + TURN_X) as i16 as u32;
    nested!(cpu, 0x0606_72ec, sgl::sl_rot_x(cpu, tilt));
    let turn = cpu.read16(o + TURN_Y) as i16 as u32;
    nested!(cpu, 0x0606_736c, sgl::sl_rot_y(cpu, turn));
    if cpu.read32(o + HIT_LIFE) as i32 > 0 {
        let flash = (cpu.read16(o + HIT_FLAGS) & FLASH) as u32;
        let models = cpu.read32(o + SB_GUN_MODEL);
        let model = cpu.read32(models + 4 * flash);
        nested!(cpu, 0x0606_745c, sgl::sl_put_polygon(cpu, model));
    }
    nested!(cpu, 0x0606_7200, sgl::sl_pop_matrix(cpu));
    nested!(cpu, 0x0606_713c, sgl::sl_cur_window(cpu, 1));
}

/// A beam's segment's colours (its sprite's colour and own colour: the first character's, or 16
/// past it, on alternate frames) and its frame (0x060a43dc's two moving or stopped (+0x75),
/// changing every other frame).
const SEG_STOPPED: u32 = 0x75;
const SEG_FRAMES: u32 = 0x060a_43dc;

pub fn segment_look(cpu: &mut Cpu, o: u32) {
    let odd = cpu.read32(GAME_FRAMES) & 1 != 0;
    let c = cpu.read16(CHAR_BASE).wrapping_add(if odd { 0 } else { 16 });
    cpu.write16(o + OWN_COLOUR, c);
    cpu.write16(o + COLOUR, c);
    let step = cpu.read8(o + SEG_STOPPED) as i8 as i32 as u32;
    let half = (cpu.read32(GAME_FRAMES) & 2) >> 1;
    let frame = cpu.read8(SEG_FRAMES.wrapping_add(2 * step).wrapping_add(half));
    cpu.write8(o + SPRITE_FRAME, frame);
}

/// A beam's segment (the boss's burst): its boss (+0x6c, a pointer), how far out along the boss's
/// turn it is (+0x70) and its age (+0x74, a byte: 0 its first frame); it stops (+0x75) where the
/// beam was hit, its boxes then 0x060a43d4's and its lift -8.
const SEG_BOSS: u32 = 0x6c;
const SEG_OUT: u32 = 0x70;
const SEG_AGE: u32 = 0x74;
const SEG_STOPPED_BOXES: u32 = 0x060a_43d4;
const SEG_FIRST_FLAGS: u32 = 32;
const SEG_FIRST_TURN: u8 = 44;
/// Its hit flags' bit for touching (hits.rs's).
const TOUCHING: u16 = 0x400;

/// A segment, a frame: ended past the segment the beam was hit at last frame, or off screen; else
/// out along the beam (8 dots its first frame, 12 its second with its flags and turn set, then 24 a
/// frame until it reaches where the beam was hit, where it stops); touching something, the beam's
/// hit kept at the nearest segment, and while the beam is strong every eighth frame a blast at it;
/// placed out from the boss's head along the boss's turn, turned with it, sized by the beam; aged,
/// and past its first frame its colours and frame.
pub fn segment_behaviour(cpu: &mut Cpu, o: u32) {
    let boss = cpu.read32(o + SEG_BOSS);
    let hit_at = cpu.read8(boss + SB_KEPT) as i8;
    let past = hit_at != -1 && cpu.read8(o + SEG_AGE) as i8 > hit_at;
    if past || nested!(cpu, 0x0605_6a24, aim::off_screen(cpu, o + FLAGS)) as u8 != 0 {
        cpu.write8(o + OBJ_MODE, 112);
        return;
    }
    let out = |cpu: &mut Cpu, by: u32| {
        let v = cpu.read32(o + SEG_OUT).wrapping_add(by);
        cpu.write32(o + SEG_OUT, v);
    };
    match cpu.read8(o + SEG_AGE) {
        0 => out(cpu, 0x8_0000),
        1 => {
            cpu.write32(o + FLAGS, SEG_FIRST_FLAGS);
            cpu.write8(o + TURN_Y, SEG_FIRST_TURN);
            out(cpu, 0xc_0000);
        }
        age => {
            if cpu.read8(o + SEG_STOPPED) == 1 {
                if age != 2 {
                    out(cpu, 0x18_0000);
                }
            } else if age == {
                let boss = cpu.read32(o + SEG_BOSS);
                cpu.read8(boss + SB_KEPT)
            } {
                cpu.write8(o + SEG_STOPPED, 1);
                cpu.write32(o + HIT_BOXES, SEG_STOPPED_BOXES);
                cpu.write8(o + LIFT, 0xf8);
            } else {
                out(cpu, 0x18_0000);
            }
        }
    }
    if cpu.read16(o + HIT_FLAGS) & TOUCHING != 0 {
        let frames = cpu.read32(GAME_FRAMES);
        let boss = cpu.read32(o + SEG_BOSS);
        if frames & 7 == 0 && cpu.read32(boss + SB_BC) as i32 > 0x7fff {
            let p = cpu.stack();
            for k in [0, 4, 8] {
                let v = cpu.read32(o + POSITION + k);
                cpu.write32(p + k, v);
            }
            if frames & 8 != 0 {
                nested!(cpu, 0x0609_ef14, blast_small_quiet(cpu, p));
            } else {
                nested!(cpu, 0x0609_edac, blast_small(cpu, p, 1));
            }
        }
        let boss = cpu.read32(o + SEG_BOSS);
        let nearest = cpu.read8(boss + SB_THIS_FRAME) as i8;
        let age = cpu.read8(o + SEG_AGE);
        if nearest == -1 || nearest > age as i8 {
            cpu.write8(boss + SB_THIS_FRAME, age);
        }
    }
    let boss = cpu.read32(o + SEG_BOSS);
    let turn = cpu.read16(boss + TURN_Z) as i16 as i32 as u32;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, turn));
    let reach = cpu.read32(o + SEG_OUT);
    let dx = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, reach.wrapping_neg(), s));
    let boss = cpu.read32(o + SEG_BOSS);
    let x = cpu.read32(boss + SB_HEAD_AT).wrapping_add(dx);
    cpu.write32(o + POSITION, x);
    let turn = cpu.read16(boss + TURN_Z) as i16 as i32 as u32;
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, turn));
    let reach = cpu.read32(o + SEG_OUT);
    let dy = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, reach, c));
    let boss = cpu.read32(o + SEG_BOSS);
    let y = cpu.read32(boss + SB_HEAD_AT + 4).wrapping_add(dy);
    cpu.write32(o + POSITION + 4, y);
    let turn = cpu.read16(boss + TURN_Z);
    cpu.write16(o + TURN_Z, turn);
    let size = cpu.read32(boss + SB_BC);
    cpu.write32(o + SCALES, size);
    let age = cpu.read8(o + SEG_AGE);
    cpu.write8(o + SEG_AGE, age.wrapping_add(1));
    if age != 0 {
        nested!(cpu, 0x0609_e6a0, segment_look(cpu, o));
    }
}

/// A segment's flags (drawn over, ...), its first frame (-82), its side and type (1, 5), its
/// boxes and lift while moving (0x060a43c4's, -16).
const SEG_FLAGS: u32 = 0x2000_0400;
const SEG_FIRST_FRAME: u8 = 0xae;
const SEG_TYPE: u8 = 5;
const SEG_BOXES: u32 = 0x060a_43c4;

/// A segment `o` set up for `boss`'s beam: its sprite and hits, its behaviour, the boss's number,
/// at the boss's head, its first frame, its colours by the frame count, side 1 type 5, sized by
/// the beam, not out and new; its boxes and lift as it moves or is stopped.
pub fn segment_init(cpu: &mut Cpu, o: u32, boss: u32) {
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HIT_BOXES));
    let flags = cpu.read32(o + FLAGS) | SEG_FLAGS;
    cpu.write32(o + FLAGS, flags);
    cpu.write32(o + BEHAVIOUR, 0x0609_e6fc);
    let n = cpu.read8(boss + NUMBER);
    cpu.write8(o + NUMBER, n);
    let x = cpu.read32(boss + SB_HEAD_AT);
    cpu.write32(o + POSITION, x);
    let y = cpu.read32(boss + SB_HEAD_AT + 4);
    cpu.write32(o + POSITION + 4, y);
    cpu.write32(o + POSITION + 8, 0);
    cpu.write8(o + SPRITE_FRAME, SEG_FIRST_FRAME);
    cpu.write8(o + TURN_Y, 0);
    let odd = cpu.read32(GAME_FRAMES) & 1 != 0;
    let c = cpu.read16(CHAR_BASE).wrapping_add(if odd { 0 } else { 16 });
    cpu.write16(o + OWN_COLOUR, c);
    cpu.write16(o + COLOUR, c);
    cpu.write8(o + HIT_SIDE, 1);
    cpu.write8(o + HIT_TYPE, SEG_TYPE);
    let size = cpu.read32(boss + SB_BC);
    cpu.write32(o + SCALES, size);
    cpu.write32(o + SEG_BOSS, boss);
    cpu.write32(o + SEG_OUT, 0);
    cpu.write8(o + SEG_AGE, 0);
    let stopped = cpu.read8(o + SEG_STOPPED) != 0;
    let (boxes, lift) = if stopped { (SEG_STOPPED_BOXES, 0xf8) } else { (SEG_BOXES, 0xf0) };
    cpu.write32(o + HIT_BOXES, boxes);
    cpu.write8(o + LIFT, lift);
}

/// The beam's next segment (list 2, 0x78 bytes, actor_exec's): stopped from the start while the
/// boss's burst is starting (3).
const SEG_SIZE: u32 = 0x78;

pub fn sb_burst_step(cpu: &mut Cpu, boss: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, SEG_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return;
    }
    let stopped = (cpu.read8(boss + SB_BURST) == 3) as u8;
    cpu.write8(o + SEG_STOPPED, stopped);
    nested!(cpu, 0x0609_e8f4, segment_init(cpu, o, boss));
}

/// A ring's description (rings.rs's): its flags (a byte), colour (a word), turn (three words),
/// place (three longs), scale (two), the scale's rates (two), its radii (inner, outer), their
/// rates (+0x30, +0x34, +0x38), what it follows (+0x3c) and its matrix (+0x40).
const RD_FLAGS: u32 = 0x00;
const RD_COLOUR: u32 = 0x02;
const RD_TURN: u32 = 0x04;
const RD_PLACE: u32 = 0x0c;
const RD_SCALE: u32 = 0x18;
const RD_SCALE_RATE: u32 = 0x20;
const RD_RADII: u32 = 0x28;
const RD_RATES: u32 = 0x30;
const RD_FOLLOWS: u32 = 0x3c;
const RD_MATRIX: u32 = 0x40;

/// A ring about the boss's head (as its beam starts): flags 1, the first character's colour,
/// unturned, at the head, whole and growing 1/16 a frame, 48 to 80 dots across, the first rate
/// whole.
pub fn sb_head_ring(cpu: &mut Cpu, boss: u32) {
    let d = cpu.stack();
    let x = cpu.read32(boss + SB_HEAD_AT);
    cpu.write32(d + RD_PLACE, x);
    let y = cpu.read32(boss + SB_HEAD_AT + 4);
    cpu.write32(d + RD_PLACE + 4, y);
    cpu.write32(d + RD_PLACE + 8, 0);
    for k in [0, 2, 4] {
        cpu.write16(d + RD_TURN + k, 0);
    }
    for k in [0, 4] {
        cpu.write32(d + RD_SCALE_RATE + k, 0x1000);
    }
    let c = cpu.read16(CHAR_BASE);
    cpu.write16(d + RD_COLOUR, c);
    cpu.write8(d + RD_FLAGS, 1);
    for k in [0, 4] {
        cpu.write32(d + RD_SCALE + k, 0x1_0000);
    }
    cpu.write32(d + RD_RADII + 4, 0x50_0000);
    cpu.write32(d + RD_RADII, 0x30_0000);
    cpu.write32(d + RD_RATES, 0x1_0000);
    for k in [4, 8] {
        cpu.write32(d + RD_RATES + k, 0);
    }
    cpu.write32(d + RD_FOLLOWS, 0);
    cpu.write32(d + RD_MATRIX, 0);
    nested!(cpu, 0x0606_2738, rings::ring_set(cpu, d));
}

/// The head's glow (an object: its boss +0x3c, its step +0x40, its size x and y +0x48, +0x4c):
/// ended once the beam is gone.
const HG_BOSS: u32 = 0x3c;
const HG_STEP: u32 = 0x40;
const HG_SIZE: u32 = 0x48;
const HG_FRAME: u8 = 0xd3;
const HG_FLIP: u32 = 0x100;

/// The head's glow, a frame: 0 growing (0x5b0 a frame) until the burst is in its second step, then
/// wide and flat, its frame, turned against the boss, a ring at the head, the screen flashed and
/// fading back; 1 turned against the boss, narrowing (0x3000 a frame) and lengthening (0x800) to
/// whole, then on; 2 until the burst is in its last step; 3 sized with the beam. At the head,
/// jittering (up to 0x2d00 more, and 0x2000 on alternate frames, flipped), its colours by the frame
/// count.
pub fn head_glow(cpu: &mut Cpu, o: u32) {
    let boss = cpu.read32(o + HG_BOSS);
    if cpu.read32(boss + SB_BC) == 0 {
        cpu.write8(o + OBJ_MODE, 112);
        return;
    }
    let next = |cpu: &mut Cpu| {
        let step = cpu.read32(o + HG_STEP).wrapping_add(1);
        cpu.write32(o + HG_STEP, step);
    };
    let burst = |cpu: &mut Cpu| {
        let boss = cpu.read32(o + HG_BOSS);
        cpu.read8(boss + SB_BURST)
    };
    let against = |cpu: &mut Cpu| {
        let boss = cpu.read32(o + HG_BOSS);
        let turn = cpu.read16(boss + TURN_Z).wrapping_add(0x8000);
        cpu.write16(o + TURN_Z, turn);
    };
    match cpu.read32(o + HG_STEP) {
        0 => {
            for k in [0, 4] {
                let v = cpu.read32(o + HG_SIZE + k).wrapping_add(0x5b0);
                cpu.write32(o + HG_SIZE + k, v);
            }
            if burst(cpu) == 2 {
                cpu.write32(o + HG_SIZE, 0x4_0000);
                cpu.write32(o + HG_SIZE + 4, 0x8000);
                cpu.write8(o + SPRITE_FRAME, HG_FRAME);
                against(cpu);
                let boss = cpu.read32(o + HG_BOSS);
                nested!(cpu, 0x0609_ea4c, sb_head_ring(cpu, boss));
                nested!(cpu, 0x0605_e6dc, screen::set_fade_offset(cpu, 60, 40, 40));
                nested!(cpu, 0x0605_e93c, screen::fade_to(cpu, 0, 0, 0, 0x200));
                next(cpu);
            }
        }
        1 => {
            against(cpu);
            let x = cpu.read32(o + HG_SIZE).wrapping_add(0xffff_d000);
            cpu.write32(o + HG_SIZE, x);
            let y = cpu.read32(o + HG_SIZE + 4).wrapping_add(0x800);
            cpu.write32(o + HG_SIZE + 4, y);
            if y == 0x1_0000 {
                next(cpu);
                cpu.write16(o + TURN_Z, 0);
            }
            if burst(cpu) == 1 {
                next(cpu);
            }
        }
        2 => {
            if burst(cpu) == 1 {
                next(cpu);
            }
        }
        3 => {
            let boss = cpu.read32(o + HG_BOSS);
            let size = cpu.read32(boss + SB_BC);
            let v = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, 0x1_0000, size));
            cpu.write32(o + HG_SIZE + 4, v);
            cpu.write32(o + HG_SIZE, v);
        }
        _ => {}
    }
    let boss = cpu.read32(o + HG_BOSS);
    let x = cpu.read32(boss + SB_HEAD_AT);
    cpu.write32(o + POSITION, x);
    let y = cpu.read32(boss + SB_HEAD_AT + 4);
    cpu.write32(o + POSITION + 4, y);
    let r = nested!(cpu, 0x0603_4dd4, sgl::sl_random(cpu)) & 15;
    let mut jitter = (r * 3) << 8;
    let flags = cpu.read32(o + FLAGS);
    if cpu.read32(GAME_FRAMES) & 1 != 0 {
        cpu.write32(o + FLAGS, flags | HG_FLIP);
        jitter += 0x2000;
    } else {
        cpu.write32(o + FLAGS, flags & !HG_FLIP);
    }
    let even = cpu.read32(GAME_FRAMES) & 2 == 0;
    let c = cpu.read16(CHAR_BASE).wrapping_add(if even { 16 } else { 0 });
    cpu.write16(o + OWN_COLOUR, c);
    cpu.write16(o + COLOUR, c);
    for k in [0, 4] {
        let v = cpu.read32(o + HG_SIZE + k).wrapping_add(jitter);
        cpu.write32(o + SCALES + k, v);
    }
}

/// The head's glow's flags (drawn over, ...), its +0x44.
const HG_FLAGS: u32 = 0x2000_0400;
const HG_44: u32 = 0x44;

/// A head's glow `o` set up for `boss`: its sprite, flags and behaviour, the boss's number, at the
/// boss's head, no lift, its frame, unturned, 16 past the first character's colours, half size,
/// following the boss, at its first step.
pub fn head_glow_init(cpu: &mut Cpu, o: u32, boss: u32) {
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + FLAGS));
    let flags = cpu.read32(o + FLAGS) | HG_FLAGS;
    cpu.write32(o + FLAGS, flags);
    cpu.write32(o + BEHAVIOUR, 0x0609_ead4);
    let n = cpu.read8(boss + NUMBER);
    cpu.write8(o + NUMBER, n);
    let x = cpu.read32(boss + SB_HEAD_AT);
    cpu.write32(o + POSITION, x);
    let y = cpu.read32(boss + SB_HEAD_AT + 4);
    cpu.write32(o + POSITION + 4, y);
    cpu.write32(o + POSITION + 8, 0);
    cpu.write8(o + LIFT, 0);
    cpu.write8(o + SPRITE_FRAME, HG_FRAME);
    cpu.write8(o + TURN_Y, 0);
    let c = cpu.read16(CHAR_BASE).wrapping_add(16);
    cpu.write16(o + OWN_COLOUR, c);
    cpu.write16(o + COLOUR, c);
    for k in [0, 4] {
        cpu.write32(o + HG_SIZE + k, 0x8000);
    }
    cpu.write32(o + HG_BOSS, boss);
    cpu.write32(o + HG_44, 0);
    cpu.write32(o + HG_STEP, 0);
}

/// The beam's start (list 2, 0x50 bytes, plain_exec's): the head's glow made, the beam whole, its
/// sound.
const HG_SIZE_BYTES: u32 = 0x50;
const SB_BEAM_SOUND: u32 = 69;

pub fn sb_beam_start(cpu: &mut Cpu, boss: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, HG_SIZE_BYTES)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, PLAIN_EXEC));
    if o != 0 {
        nested!(cpu, 0x0609_eca8, head_glow_init(cpu, o, boss));
    }
    cpu.write32(boss + SB_BC, 0x1_0000);
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, SB_BEAM_SOUND));
}

/// The blasts' scripts (the effects' drift_effect: 0x06073008 small, 0x0607305c large), their
/// colours (0x90; the silent ones 0x8090) and sounds.
const BLAST_SMALL: u32 = 0x0607_3008;
const BLAST_LARGE: u32 = 0x0607_305c;
const BLAST_COLOUR: u16 = 0x90;
const BLAST_QUIET_COLOUR: u16 = 0x8090;
const BLAST_SMALL_SOUND: u32 = 18;
const BLAST_LARGE_SOUND: u32 = 12;
/// A blast's random flip (its flags' bits 4 and 5).
const BLAST_FLIP: u32 = 0x30;

/// A blast at `p` (an effect's description: the place there): its script and colour, unturned and
/// still, whole; flipped at random.
fn blast(cpu: &mut Cpu, p: u32, script: u32, colour: u16) {
    cpu.write32(p + actors::E_SCRIPT, script);
    cpu.write16(p + actors::E_COLOUR, colour);
    cpu.write16(p + actors::E_ANGLE, 0);
    cpu.write32(p + actors::E_SPEED, 0);
    cpu.write32(p + actors::E_SCALES, 0x1_0000);
    cpu.write32(p + actors::E_SCALES + 4, 0x1_0000);
    let e = nested!(cpu, 0x0605_7454, actors::drift_effect(cpu, p));
    if e != 0 {
        let r = nested!(cpu, 0x0603_4dd4, sgl::sl_random(cpu));
        let flags = cpu.read32(e + FLAGS) | (r & BLAST_FLIP);
        cpu.write32(e + FLAGS, flags);
    }
}

/// A small blast at `p`, with its sound when `loud`.
pub fn blast_small(cpu: &mut Cpu, p: u32, loud: u32) {
    blast(cpu, p, BLAST_SMALL, BLAST_COLOUR);
    if loud != 0 {
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, BLAST_SMALL_SOUND));
    }
}

/// A large blast at `p`, with its sound when `loud`.
pub fn blast_large(cpu: &mut Cpu, p: u32, loud: u32) {
    blast(cpu, p, BLAST_LARGE, BLAST_COLOUR);
    if loud != 0 {
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, BLAST_LARGE_SOUND));
    }
}

/// A small blast at `p`, silent.
pub fn blast_small_quiet(cpu: &mut Cpu, p: u32) {
    blast(cpu, p, BLAST_SMALL, BLAST_QUIET_COLOUR);
}

/// A large blast at `p`, silent.
pub fn blast_large_quiet(cpu: &mut Cpu, p: u32) {
    blast(cpu, p, BLAST_LARGE, BLAST_QUIET_COLOUR);
}

/// The stage boss made (a numbered object, 0x13c bytes): its script index the difficulty; waiting,
/// unseen and not hit (a model drawn by its own function, its draw at first nothing), a quarter
/// tilted about x, no lift; at its place, 0x8000000 down; its fields none, its turns kept its turn;
/// its phase 4; side 1, its boxes (0x060a4240) and life 90000; the stage's rate and score none;
/// kind 43's models sorted (the eighth by 2, the first, second, third and fifth by 1), the Gouraud
/// tables' ramp (24, 24, 26) and floor 10; the light set. Its number, or 0.
const SB_SIZE_BYTES: u32 = 0x13c;
const SB_FLAGS: u32 = IS_MODEL | OWN_DRAW | NO_HITS;
const SB_TILT: u16 = 0x4000;
const SB_9E: u32 = 0x9e;
const SB_BOXES: u32 = 0x060a_4240;
const SB_FIRST_PHASE: u8 = 4;
const SB_LIGHT: [u32; 3] = [0xffff_4a3e, 0xb5c2, 0xb5c2];
const SB_SORTS: [(u32, u32); 5] = [(7, 2), (0, 1), (1, 1), (2, 1), (4, 1)];

pub fn sb_make(cpu: &mut Cpu) -> u32 {
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, ENEMY_NUMBERS)) as i8;
    if n == -1 {
        return 0;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, SB_SIZE_BYTES)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, NUMBERED_EXEC));
    if o == 0 {
        return 0;
    }
    let difficulty = cpu.read8(DIFFICULTY);
    cpu.write8(SB_SCRIPT_INDEX, difficulty);
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HIT_BOXES));
    cpu.write32(o + BEHAVIOUR, 0x0609_bcc4);
    for k in [0, 4, 8] {
        cpu.write32(o + POSITION + k, 0);
    }
    let flags = cpu.read32(o + FLAGS) | SB_FLAGS;
    cpu.write32(o + FLAGS, flags);
    cpu.write32(o + MODEL, SB_GUN_NO_DRAW);
    cpu.write16(o + TURN_X, SB_TILT);
    cpu.write8(o + LIFT, 0);
    cpu.write8(o + NUMBER, n as u8);
    let n = cpu.read8(o + NUMBER) as i8 as i32;
    cpu.write32(NUMBERED.wrapping_add((4 * n) as u32), o);
    cpu.write32(o + SB_AT, 0);
    cpu.write32(o + SB_AT + 4, 0);
    cpu.write32(o + SB_HEIGHT, SB_FROM);
    cpu.write16(o + SB_BOB_PHASE, 0);
    cpu.write16(o + SB_9E, 0);
    cpu.write8(o + SB_TURNS_AT, 0);
    cpu.write32(o + SB_D8, 0);
    cpu.write8(o + SB_BURST, 0);
    for at in [SB_B0, SB_AC, SB_MOVE_LEFT] {
        cpu.write32(o + at, 0);
    }
    cpu.write16(o + SB_7A, 0);
    cpu.write16(o + SB_TURN_WAS, 0);
    cpu.write32(o + SB_BC, 0);
    cpu.write32(o + SB_BOB, 0);
    cpu.write8(BOSS_PHASE, SB_FIRST_PHASE);
    for k in 0..16 {
        let turn = cpu.read16(o + TURN_Z);
        cpu.write16(o + SB_TURNS + 2 * k, turn);
    }
    cpu.write8(o + HIT_SIDE, 1);
    cpu.write32(o + HIT_BOXES, SB_BOXES);
    cpu.write32(o + HIT_LIFE, SB_LIFE);
    cpu.write32(STAGE_RATE_AT, 0);
    cpu.write32(STAGE_SCORE_AT, 0);
    for (frame, sort) in SB_SORTS {
        nested!(cpu, 0x0606_3428, gouraud::model_sort(cpu, SB_KIND, frame, sort));
    }
    nested!(cpu, 0x0606_3168, gouraud::table_ramp(cpu, 24, 24, 26));
    nested!(cpu, 0x0606_30e0, gouraud::table_floor(cpu, 10, 10, 10));
    let v = cpu.stack();
    for (k, c) in SB_LIGHT.into_iter().enumerate() {
        cpu.write32(v + 4 * k as u32, c);
    }
    nested!(cpu, 0x0607_0f8c, sgl::sl_light(cpu, v));
    cpu.read8(o + NUMBER) as i8 as i32 as u32
}
