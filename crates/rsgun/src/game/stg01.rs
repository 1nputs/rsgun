//! STG01.BIN's code, which the stage 3 mode runs in the overlay window (the stage played first;
//! its screen data is SCRDAT31 and SCRDAT32): its mode's code, its enemies and scenery, its
//! scrolls.

use super::globals::mode::DEMO10;
use super::globals::{BIOS_SETTING, GAME_FRAMES, GAME_MODE, GROUND_DY_AT};
use super::globals::LOOPS;
use super::bullets::{AS_ANGLE, AS_FLAGS, AS_POSITION, AS_SHOOTER, AS_SPEED};
use super::{actors, aim, bullets, camera, clib, dma, effects, explosions, files, ground, hits, hud, kinds, math, models, nested, objects, parts, results, screen, sgl, sound, sprites, states, timeline, Cpu};

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

