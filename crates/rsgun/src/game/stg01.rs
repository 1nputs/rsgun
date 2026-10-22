//! STG01.BIN's code, which the stage 3 mode runs in the overlay window (the stage played first;
//! its screen data is SCRDAT31 and SCRDAT32): its mode's code, its enemies and scenery, its
//! scrolls.

use super::globals::mode::DEMO10;
use super::globals::{BIOS_SETTING, GAME_FRAMES, GAME_MODE, GROUND_DY_AT};
use super::globals::LOOPS;
use super::bullets::{AS_ANGLE, AS_FLAGS, AS_POSITION, AS_SHOOTER, AS_SPEED};
use super::{actors, aim, bullets, camera, clib, dma, effects, explosions, files, ground, hits, hud, kinds, math, models, nested, objects, orbits, parts, results, screen, sgl, sound, sprites, states, timeline, Cpu};

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

