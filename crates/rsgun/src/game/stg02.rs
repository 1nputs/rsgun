//! STG02.BIN's code, which the stage 2 mode runs in the overlay window (its screen data is
//! SCRDAT21 and SCRDAT22): its mode's code, its enemies and scenery, its scrolls.

use super::globals::{BIOS_SETTING, GAME_FLAGS, GAME_MODE, LOOPS, PLAY_DATA, PLAY_DATA_SIZE};
use super::bullets::{AS_ANGLE, AS_FLAGS, AS_POSITION, AS_SHOOTER, AS_SPEED, S_ANGLE, S_NUMBER, S_POSITION};
use super::{actors, aim, bg, bullets, camera, clib, dma, effects, explosions, files, gouraud, ground, hits, kinds, hud, math, models, nested, objects, orbits, palette, parts, results, ribbons, rings, screen, sgl, sound, sprites, states, text, timeline, Cpu};

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
const SCREEN_FILE_1: u32 = SCREEN_FILES + 4;
const SCREEN_FILE_2: u32 = SCREEN_FILES + 0x20;
/// Its screen's room in the stage table (states::stage_table's 1).
const SCREEN_ROOM: u32 = 1;
/// The sound data files loaded at once (sound::load_sound_file), the defaults' set-up after the
/// first, and the music set after the third.
const SOUND_FILES: [u32; 5] = [1, 9, 8, 0x20, 0x1f];
const STAGE_MUSIC: u16 = 3;
/// The kinds put in place (kinds.rs): two model kinds (the first's characters below 0x7fef8 by
/// its file's size), then two sprite kinds that take kind 12's first colour.
const MODEL_KINDS: [u32; 2] = [0x31, 0x32];
const SPRITE_KINDS: [u32; 2] = [0x1b, 0x1c];
const KINDS: u32 = 0x0607_41f0;
const K_SIZE: u32 = 16;
const K_COLOUR: u32 = 0xc;
const COLOUR_KIND: u32 = 12;
const FILE_SIZE: u32 = 12;
const VDP1_TOP: u32 = 0x7_fef8;
/// The stage's scroll table, its timeline, its stage sound effects; the demo's input files
/// (by the BIOS's setting) and the modes after it. The demo of this stage is game mode 13.
const SCROLLS: u32 = 0x060a_85ac;
const EVENTS: u32 = 0x060a_4c20;
const SOUND_EFFECTS: u32 = 0x060a_86dc;
const STAGE_SE_TABLE: u32 = 0x0600_edd4;
const NEXT_MODE: u32 = 0x0600_ed70;
const DEMO: u8 = 13;
const DEMO_FILES: [u32; 2] = [0x0600_8c30, 0x0600_8c00]; // "DEMO2W.BIN", "DEMO2.BIN"
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
pub fn stg02_mode(cpu: &mut Cpu, game: u32) -> u32 {
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
/// next character on by each kind's count (+6), the next colour by the second model kind's
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
        advance(cpu, game, k, i == 1);
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

// ---- models' attributes, angles, small effects ------------------------------------------------

/// A model kind's file (kinds.rs's): its models (an offset made a pointer, +0x1c: two `PDATA`s
/// each, the model and its flashing copy) and their count (+0x20). A `PDATA` (SGL's): its
/// polygons' count (+0xc) and attributes (+0x10, 12 bytes each: the attribute word +4, the
/// direction word +10).
const F_MODELS: u32 = 0x1c;
const F_MODEL_COUNT: u32 = 0x20;
const PD_POLYGONS: u32 = 0xc;
const PD_ATTRS: u32 = 0x10;
const ATTR_SIZE: u32 = 12;
const AT_ATRB: u32 = 4;
const AT_DIR: u32 = 10;
/// What each polygon gets: the attribute word, and the direction's low bits (its 0x30 kept).
const ATRB: u16 = 0xc0;
const DIR_KEPT: u16 = 0x30;
const DIR_SET: u16 = 5;

/// Model `n` of kind `k`: every polygon of both its `PDATA`s given the attribute and direction
/// above.
pub fn model_attrs_set(cpu: &mut Cpu, k: u32, n: u32) {
    let file = cpu.read32(kind(k));
    let models = cpu.read32(file + F_MODELS);
    let pair = cpu.read32(models.wrapping_add(4 * n));
    let model = cpu.read32(pair);
    let flashing = cpu.read32(pair + 4);
    let mut i = 0;
    while i < cpu.read32(model + PD_POLYGONS) {
        for p in [flashing, model] {
            let a = cpu.read32(p + PD_ATTRS) + ATTR_SIZE * i;
            cpu.write16(a + AT_ATRB, ATRB);
            let dir = cpu.read16(a + AT_DIR) & DIR_KEPT | DIR_SET;
            cpu.write16(a + AT_DIR, dir);
        }
        i += 1;
    }
}

/// Every model of kind `k` so.
pub fn kind_attrs_set(cpu: &mut Cpu, k: u32) {
    let file = cpu.read32(kind(k));
    let mut n = 0;
    while (n as i32) < cpu.read32(file + F_MODEL_COUNT) as i32 {
        nested!(cpu, 0x0608_0450, model_attrs_set(cpu, k, n));
        n += 1;
    }
}

/// An object's angle (a word, sign-extended) towards the point at minus its longs `x_at` and
/// `x_at + 4` (slAtan's), a quarter turn on.
fn quarter_on_angle(cpu: &mut Cpu, o: u32, x_at: u32) -> u32 {
    let y = cpu.read32(o + x_at + 4).wrapping_neg();
    let x = cpu.read32(o + x_at).wrapping_neg();
    let a = nested!(cpu, 0x0606_780c, sgl::sl_atan(cpu, x, y));
    (a as u16).wrapping_add(0x4000) as i16 as i32 as u32
}

/// The angle by the object's longs +0x100 and +0x104.
const AT_100: u32 = 0x100;
/// The angle by its longs +0xf0 and +0xf4.
const AT_F0: u32 = 0xf0;

pub fn model_tilt_angle(cpu: &mut Cpu, o: u32) -> u32 {
    quarter_on_angle(cpu, o, AT_100)
}

pub fn model_screen_angle(cpu: &mut Cpu, o: u32) -> u32 {
    quarter_on_angle(cpu, o, AT_F0)
}

/// An object's position (x, y, z), its angle (a word), number (a byte).
const POSITION: u32 = 0x14;
const ANGLE: u32 = 0x30;
const NUMBER: u32 = 0x32;
/// Kind 61: the stage's effects' sprites (their colour, the effects').
const EFFECT_KIND: u32 = 61;

/// A polygon effect at the object `o` (polygon_set's: flags 59, unscaled, 20 frames, at its angle,
/// of its number, kind 61's colour), its corners' words (+0x70) a line `length` long, 4 shorter
/// at its end.
const LINE_FLAGS: u32 = 59;
const LINE_FRAMES: u16 = 20;
const P_CORNERS: u32 = 0x70;

pub fn line_effect(cpu: &mut Cpu, o: u32, length: u32) {
    let length = length as u16;
    let fx = cpu.stack();
    for k in [0, 4, 8] {
        let v = cpu.read32(o + POSITION + k);
        cpu.write32(fx + effects::X + k, v);
    }
    cpu.write32(fx + effects::SCALE_Y, 0x1_0000);
    cpu.write32(fx + effects::SCALE_X, 0x1_0000);
    cpu.write32(fx + effects::FLAGS_LONG, LINE_FLAGS);
    cpu.write16(fx + effects::FRAMES, LINE_FRAMES);
    cpu.write8(fx + effects::FRAMES_2, 0);
    let angle = cpu.read16(o + ANGLE);
    cpu.write16(fx + effects::ANGLE, angle);
    let colour = cpu.read16(kind(EFFECT_KIND) + K_COLOUR);
    cpu.write16(fx + effects::COLOUR, colour);
    let n = cpu.read8(o + NUMBER);
    cpu.write8(fx + effects::PLAYER, n);
    let p = nested!(cpu, 0x0604_e6d8, effects::polygon_set(cpu, fx));
    if p != 0 {
        cpu.write16(p + P_CORNERS + 4, 0);
        cpu.write16(p + P_CORNERS, 0);
        cpu.write16(p + P_CORNERS + 2, length);
        cpu.write16(p + P_CORNERS + 6, length.wrapping_sub(4));
    }
}

/// A burst at the object `o` (effect_set's: flags 11, scaled `size` both ways, kind 61's first
/// frame and colour, with its animation), growing by half its size over 60 frames; sound 104
/// (the C code passes x too, which se_set does not take). The effect, or 0.
const BURST_FLAGS: u32 = 11;
const BURST_ANIMATION: u32 = 0x060a_2ea4;
const BURST_GROWTH_FRAMES: u32 = 60;
const BURST_SOUND: u32 = 104;

pub fn burst(cpu: &mut Cpu, o: u32, size: u32) -> u32 {
    let fx = cpu.stack();
    for k in [0, 4, 8] {
        let v = cpu.read32(o + POSITION + k);
        cpu.write32(fx + effects::X + k, v);
    }
    cpu.write32(fx + effects::SCALE_Y, size);
    cpu.write32(fx + effects::SCALE_X, size);
    cpu.write32(fx + effects::FLAGS_LONG, BURST_FLAGS);
    cpu.write16(fx + effects::FRAMES, 0);
    cpu.write8(fx + effects::FRAMES_2, 0);
    cpu.write8(fx + effects::KIND, EFFECT_KIND as u8);
    cpu.write32(fx + effects::LINK, BURST_ANIMATION);
    let colour = cpu.read16(kind(EFFECT_KIND) + K_COLOUR);
    cpu.write16(fx + effects::COLOUR, colour);
    cpu.write16(fx + effects::ANGLE, 0);
    // (held as the SH-2 code holds them: r10 the object, r9 the size then its half, r8 the
    // effect; se_set's callee pushes r9, and an effect made later takes that slot as its player)
    let e = cpu.holding(&[(9, size), (10, o)], |cpu| nested!(cpu, 0x0604_e1cc, effects::effect_set(cpu, fx)));
    let mut held = size;
    if e != 0 {
        held = (size as i32 / 2) as u32;
        cpu.holding(&[(8, e), (9, held), (10, o)], |cpu| {
            nested!(cpu, 0x0604_ddf4, effects::grow_set(cpu, e, held, BURST_GROWTH_FRAMES))
        });
    }
    cpu.holding(&[(8, e), (9, held), (10, o)], |cpu| nested!(cpu, 0x0600_50d4, sound::se_set(cpu, BURST_SOUND)));
    e
}

/// A hook the stage's enemies call as they are destroyed (before their burst): does nothing; 0.
pub fn destroyed_hook(_cpu: &mut Cpu, _o: u32) -> u32 {
    0
}

// ---- the circling missile and its aiming line ---------------------------------------------------

/// The objects' mode byte (ending), flags, the sprite's colours (a word each), kind, frame; the
/// behaviour; the game's frames; the plain actors' exec (actors.rs's), their size here.
const OBJ_MODE: u32 = 3;
const TO_STOP: u8 = b'p';
const ENDING: i8 = 0x6f;
const FLAGS: u32 = 0x10;
const COLOUR: u32 = 0x28;
const OWN_COLOUR: u32 = 0x2a;
const SPRITE_KIND: u32 = 0x2e;
const SPRITE_FRAME: u32 = 0x2f;
const CORNERS_AT: u32 = 0x34;
const BYTE_33: u32 = 0x33;
const BEHAVIOUR: u32 = 0x38;
const PLAIN_EXEC: u32 = 0x0604_dcb4;
const GAME_FRAMES: u32 = 0x0600_ebd4;
/// The flags: drawn as a polygon of its own corners (sprites.rs's POLYGON_BITS 3), and 0x400.
const POLYGON_LINE: u32 = 3;
const FLAG_400: u32 = 0x400;
/// The step of the stage's enemies (a word, read by its low byte, +0x6d), and the step at which
/// they are done for (100 on).
const STEP_WORD: u32 = 0x6c;
const STEP: u32 = 0x6d;
const DONE_FOR: i8 = 99;

/// A turned point (math::turn_point's, on the stack): x, y, scales, flags, angle.
const T_X: u32 = 0;
const T_Y: u32 = 4;
const T_8: u32 = 8;
const T_SCALES: u32 = 0xc;
const T_FLAGS: u32 = 0x14;
const T_ANGLE: u32 = 0x18;

/// The line's step (a byte), its count (a word), the missile it starts at (+0x5c) and the target
/// it points at (+0x60); its corners (words, +0x70: the second point's y its length, +0x76).
const L_STEP: u32 = 0x3c;
const L_COUNT: u32 = 0x40;
const L_FROM: u32 = 0x5c;
const L_TO: u32 = 0x60;
const L_CORNERS: u32 = 0x70;
const L_LENGTH: u32 = 0x76;
/// It shrinks once the missile's step is past 14, and ends 30 frames on.
const MISSILE_LEAVING: i8 = 14;
const LINE_SHRINK_FRAMES: i16 = 29;
/// Its length: 32 dots less the distance to the target (in dots).
const LINE_REACH: u32 = 0x20_0000;

/// The stage effects' colour, pulsing over 64 frames by 4 either way about 8 on.
fn pulse_colour(cpu: &mut Cpu, o: u32) {
    let frames = cpu.read32(GAME_FRAMES);
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, (frames << 10) as i16 as i32 as u32));
    let m = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, 4, c)) as u16;
    let colour = cpu.read16(kind(EFFECT_KIND) + K_COLOUR).wrapping_add(m.wrapping_add(8));
    cpu.write16(o + OWN_COLOUR, colour);
    cpu.write16(o + COLOUR, colour);
}

/// The line's place at its missile, its angle to the target, its length.
fn line_follow(cpu: &mut Cpu, o: u32, from: u32) {
    let to = cpu.read32(o + L_TO);
    for k in [0, 4] {
        let v = cpu.read32(from + POSITION + k);
        cpu.write32(o + POSITION + k, v);
    }
    let a = nested!(cpu, 0x0604_5e30, aim::angle_to(cpu, o, to));
    cpu.write16(o + ANGLE, a as u16);
    let d = nested!(cpu, 0x0604_5ee4, aim::distance(cpu, o, to));
    let v = LINE_REACH.wrapping_sub(d) as i32;
    let v = if v < 0 { v + 0xffff } else { v };
    cpu.write16(o + L_LENGTH, (v >> 16) as u16);
}

/// The circling missile's aiming line (a plain actor's behaviour): it ends with its missile. Step
/// 0: a polygon line of its own corners, all 0; 1: from the missile toward its target, longer as
/// it nears it; 2 (once the missile leaves): it shrinks by an eighth a frame, and ends after 30.
/// Its colour pulses.
pub fn aim_line_behaviour(cpu: &mut Cpu, o: u32) {
    let from = cpu.read32(o + L_FROM);
    if cpu.read8(from + OBJ_MODE) as i8 > ENDING {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    if (cpu.read8(o + L_STEP) as i8) < 2 && cpu.read8(from + STEP) as i8 > MISSILE_LEAVING {
        cpu.write8(o + L_STEP, 2);
    }
    match cpu.read8(o + L_STEP) {
        0 => {
            cpu.write32(o + POSITION + 8, 0);
            cpu.write32(o + CORNERS_AT, o + L_CORNERS);
            let flags = cpu.read32(o + FLAGS) | POLYGON_LINE;
            cpu.write32(o + FLAGS, flags);
            for k in [4, 0, 6, 2] {
                cpu.write16(o + L_CORNERS + k, 0);
            }
            cpu.write16(o + L_COUNT, 0);
            cpu.write8(o + L_STEP, 1);
            line_follow(cpu, o, from);
        }
        1 => line_follow(cpu, o, from),
        2 => {
            for k in [0, 4] {
                let v = cpu.read32(from + POSITION + k);
                cpu.write32(o + POSITION + k, v);
            }
            let length = cpu.read16(o + L_LENGTH) as i16;
            let cut = (length as i32).wrapping_neg() / 8;
            cpu.write16(o + L_LENGTH, length.wrapping_add(cut as i16) as u16);
            let count = (cpu.read16(o + L_COUNT) as i16).wrapping_add(1);
            cpu.write16(o + L_COUNT, count as u16);
            if count > LINE_SHRINK_FRAMES {
                cpu.write8(o + OBJ_MODE, TO_STOP);
            }
        }
        _ => {}
    }
    pulse_colour(cpu, o);
}

/// The circling missile (a stage enemy's shot: its step word at +0x6c): its behaviour. Step 0:
/// its sprite set, its aiming line made. 1: it circles its target (at its turn from the target's
/// angle), closing to its radius, until it leaves (15): straight on along its angle, faster.
/// Its frame cycles; it leaves a trail. Its fields: the turn about its
/// target (+0x70, a word), its count (a word: its frame), the frames it has circled, its radius;
/// its target (+0xb0); its sprite's animation (+0x3c).
const M_ANIMATION: u32 = 0x3c;
const M_TURN: u32 = 0x70;
const M_COUNT: u32 = 0x72;
const M_CIRCLED: u32 = 0x74;
const M_RADIUS: u32 = 0x78;
const M_TARGET: u32 = 0xb0;
const MISSILE_ANIMATION: u32 = 0x060a_2ecc;
const MISSILE_FRAME: u8 = 0x1f;
const START_RADIUS: u32 = 0x20_0000;
const AIM_LINE_SIZE: u32 = 0x80;
const AIM_LINE_BEHAVIOUR: u32 = 0x0608_0c20;
const AIM_LINE_33: u8 = 0xf8;
/// The radius it closes to (by an 128th of the gap a frame): 144 dots, 48 more or less by the
/// difficulty; it leaves (step 15) after 500 frames, or once within 32 dots of it and facing within
/// 0x1000 of a quarter turn from where it aims at the player.
const RADIUS: u32 = 0x90_0000;
const RADIUS_BY_DIFFICULTY: u32 = 0x30_0000;
const CIRCLE_FRAMES: i32 = 500;
const CLOSE: i32 = 0x20_0000;
const AIM_DELAY: u32 = 20;
const FACING: i32 = 0x1000;
const LEAVING: u16 = 15;
/// Leaving: it speeds up to 4 dots a frame (by 1/16 a frame) and ends off the screen.
const LEAVE_SPEED: u32 = 0x4_0000;
const LEAVE_ACCEL: u32 = 0x1000;
const OFF_X: i32 = 0xbf_ffff;
const OFF_Y: i32 = 0x7f_ffff;
/// Its trail: every fourth frame, a puff (effect_set's: flags 3, scaled 0x6000, of kind 61's
/// first frame with its animation, placed with it), shrinking to nothing over 60 frames.
const TRAIL_SCALE: u32 = 0x6000;
const TRAIL_FLAGS: u32 = 3;
const TRAIL_GROWTH: u32 = 0x800;
const E_SCALE_SPEED: u32 = 0x54;
const E_SCALES: u32 = 0x20;

pub fn circling_missile_behaviour(cpu: &mut Cpu, o: u32) {
    let t = cpu.stack();
    let step = cpu.read8(o + STEP) as i8;
    let mut circle = step == 1;
    if step == 0 {
        cpu.write32(o + POSITION + 8, 0);
        cpu.write8(o + SPRITE_KIND, EFFECT_KIND as u8);
        cpu.write8(o + SPRITE_FRAME, MISSILE_FRAME);
        let colour = cpu.read16(kind(EFFECT_KIND) + K_COLOUR);
        cpu.write16(o + OWN_COLOUR, colour);
        cpu.write16(o + COLOUR, colour);
        cpu.write32(o + M_ANIMATION, MISSILE_ANIMATION);
        cpu.write32(o + M_RADIUS, START_RADIUS);
        cpu.write32(o + M_CIRCLED, 0);
        cpu.write16(o + M_COUNT, 0);
        let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, AIM_LINE_SIZE)) as i8 as i32 as u32;
        let line = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 3, blocks, PLAIN_EXEC));
        if line != 0 {
            nested!(cpu, 0x0604_dd50, effects::actor_init(cpu, line));
            let flags = cpu.read32(line + FLAGS) | FLAG_400;
            cpu.write32(line + FLAGS, flags);
            cpu.write32(line + BEHAVIOUR, AIM_LINE_BEHAVIOUR);
            let n = cpu.read8(o + NUMBER);
            cpu.write8(line + NUMBER, n);
            cpu.write8(line + BYTE_33, AIM_LINE_33);
            let target = cpu.read32(o + M_TARGET);
            cpu.write32(line + L_TO, target);
            cpu.write32(line + L_FROM, o);
        }
        cpu.write16(o + STEP_WORD, 1);
        circle = true;
    }
    if circle {
        let target = cpu.read32(o + M_TARGET);
        if cpu.read8(target + STEP) as i8 > DONE_FOR || cpu.read8(target + OBJ_MODE) as i8 > ENDING {
            cpu.write8(o + OBJ_MODE, TO_STOP);
        } else {
            let goal = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, RADIUS_BY_DIFFICULTY)).wrapping_add(RADIUS);
            cpu.write32(t + T_8, goal);
            let radius = cpu.read32(o + M_RADIUS);
            let gap = goal.wrapping_sub(radius) as i32;
            let gap = if gap < 0 { gap + 0x7f } else { gap } as u32;
            nested!(cpu, 0x0600_8166, clib::ashiftrt_r4::<7>(cpu, gap));
            cpu.write32(o + M_RADIUS, radius.wrapping_add((gap as i32 >> 7) as u32));
            cpu.write32(t + T_X, 0);
            let r = cpu.read32(o + M_RADIUS).wrapping_neg();
            cpu.write32(t + T_Y, r);
            cpu.write32(t + T_SCALES + 4, 0x1_0000);
            cpu.write32(t + T_SCALES, 0x1_0000);
            cpu.write32(t + T_FLAGS, 0);
            let a = nested!(cpu, 0x0608_054c, model_screen_angle(cpu, target)) as u16;
            let a = cpu.read16(o + M_TURN).wrapping_add(a);
            cpu.write16(t + T_ANGLE, a);
            nested!(cpu, 0x0606_4eac, math::turn_point(cpu, t));
            let x = cpu.read32(target + POSITION).wrapping_add(cpu.read32(t + T_X));
            cpu.write32(o + POSITION, x);
            let y = cpu.read32(target + POSITION + 4).wrapping_add(cpu.read32(t + T_Y));
            cpu.write32(o + POSITION + 4, y);
            let a = cpu.read16(t + T_ANGLE).wrapping_add(0x4000);
            cpu.write16(o + ANGLE, a);
            let circled = cpu.read32(o + M_CIRCLED) as i32;
            let leave = if circled < CIRCLE_FRAMES {
                cpu.write32(o + M_CIRCLED, (circled + 1) as u32);
                let gap = cpu.read32(t + T_8).wrapping_sub(cpu.read32(o + M_RADIUS)) as i32;
                gap.wrapping_abs() <= CLOSE && {
                    let aim = nested!(cpu, 0x0604_0df0, aim::aim(cpu, o, AIM_DELAY)) as i16 as i32 as u32;
                    let facing = (cpu.read16(o + ANGLE) as i16 as i32).wrapping_sub(0x1000) as u32;
                    let off = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, facing, aim)) as i32;
                    off.wrapping_abs() <= FACING
                }
            } else {
                true
            };
            if leave {
                cpu.write32(o + M_RADIUS, 0);
                cpu.write16(o + STEP_WORD, LEAVING);
            }
        }
    } else if step == LEAVING as i8 {
        let r = cpu.read32(o + M_RADIUS);
        let r = nested!(cpu, 0x0604_6168, actors::approach(cpu, r, LEAVE_SPEED, LEAVE_ACCEL));
        cpu.write32(o + M_RADIUS, r);
        cpu.write32(t + T_X, 0);
        let r = cpu.read32(o + M_RADIUS).wrapping_neg();
        cpu.write32(t + T_Y, r);
        cpu.write32(t + T_SCALES + 4, 0x1_0000);
        cpu.write32(t + T_SCALES, 0x1_0000);
        cpu.write32(t + T_FLAGS, 0);
        let a = cpu.read16(o + ANGLE);
        cpu.write16(t + T_ANGLE, a);
        nested!(cpu, 0x0606_4eac, math::turn_point(cpu, t));
        let x = cpu.read32(o + POSITION).wrapping_add(cpu.read32(t + T_X));
        cpu.write32(o + POSITION, x);
        let y = cpu.read32(o + POSITION + 4).wrapping_add(cpu.read32(t + T_Y));
        cpu.write32(o + POSITION + 4, y);
        if (x as i32).wrapping_abs() > OFF_X || (y as i32).wrapping_abs() > OFF_Y {
            cpu.write8(o + OBJ_MODE, TO_STOP);
        }
    }
    let count = cpu.read16(o + M_COUNT) as i16 as i32 as u32;
    let frame = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, count, 8)) as u8;
    cpu.write8(o + SPRITE_FRAME, MISSILE_FRAME.wrapping_sub(frame));
    let count = cpu.read16(o + M_COUNT).wrapping_add(1);
    cpu.write16(o + M_COUNT, count);
    let frames = cpu.read32(GAME_FRAMES);
    if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, frames, 4)) == 0 {
        let fx = t;
        for k in [0, 4, 8] {
            let v = cpu.read32(o + POSITION + k);
            cpu.write32(fx + effects::X + k, v);
        }
        cpu.write32(fx + effects::SCALE_Y, TRAIL_SCALE);
        cpu.write32(fx + effects::SCALE_X, TRAIL_SCALE);
        cpu.write32(fx + effects::FLAGS_LONG, TRAIL_FLAGS);
        cpu.write16(fx + effects::ANGLE, 0);
        cpu.write16(fx + effects::FRAMES, 0);
        cpu.write8(fx + effects::FRAMES_2, 0);
        cpu.write8(fx + effects::KIND, EFFECT_KIND as u8);
        cpu.write8(fx + effects::FRAME, 0);
        let colour = cpu.read16(kind(EFFECT_KIND) + K_COLOUR);
        cpu.write16(fx + effects::COLOUR, colour);
        let n = cpu.read8(o + NUMBER);
        cpu.write8(fx + effects::PLAYER, n);
        cpu.write32(fx + effects::LINK, BURST_ANIMATION);
        cpu.write32(fx + effects::RELATIVE_3C, o);
        let e = nested!(cpu, 0x0604_e1cc, effects::effect_set(cpu, fx));
        if e != 0 {
            cpu.write32(e + E_SCALE_SPEED + 4, TRAIL_GROWTH);
            cpu.write32(e + E_SCALE_SPEED, TRAIL_GROWTH);
            let ay = (cpu.read32(e + E_SCALES + 4) as i32 / 60).wrapping_neg() as u32;
            let ax = (cpu.read32(e + E_SCALES) as i32 / 60).wrapping_neg() as u32;
            nested!(cpu, 0x0604_de98, effects::accel_set(cpu, e, ax, ay));
        }
    }
}

// ---- the boss's core ---------------------------------------------------------------------------

/// A model part's (models.rs's) fields the boss's parts use: who hit it last (+0x40: its player
/// +0x32 and weapon +0x55), its life, the byte its shots and pieces copy (+0x54), its turns (y
/// +0x2e, z +0x30), its step word's high byte (an inner step, +0x6c), its slot in its parent's
/// parts (+0x6e; the parent's at +0xb4).
const HIT_BY: u32 = 0x40;
const HIT_PLAYER: u32 = 0x32;
const HIT_WEAPON: u32 = 0x55;
const LIFE: u32 = 0x50;
const SIDE: u32 = 0x54;
const TURN_Y: u32 = 0x2e;
const TURN_Z: u32 = 0x30;
const INNER: u32 = 0x6c;
const SLOT: u32 = 0x6e;
const PART_SLOTS: u32 = 0xb4;
/// The flags: no hits; shielded (hits do no harm).
const NO_HITS: u32 = 0x8_0000;
const UNHURT: u32 = 0x4_0000;
/// The steps every part has: shot down (or its parent done for), then gone.
const SHOT_DOWN: u16 = 100;
const GONE: u16 = 101;
const FIGHTING: u16 = 15;
/// The stage's rate (a percentage, results.rs's), and what a part's points add to it.
const STAGE_RATE_AT: u32 = 0x0607_46f0;
/// The actors' exec (actors.rs's) for the boss's shots and pieces.
const ACTOR_EXEC: u32 = 0x0604_d780;

/// Shot down (its life gone) or the part it watches (its parent, or the boss) done for (past
/// step 99), a part goes to step 100, no longer hit.
fn part_shot_or_parent_gone(cpu: &mut Cpu, o: u32, watched: u32) {
    if cpu.read8(o + STEP) as i8 <= DONE_FOR
        && ((cpu.read32(o + LIFE) as i32) <= 0 || cpu.read8(watched + STEP) as i8 > DONE_FOR)
    {
        let flags = cpu.read32(o + FLAGS) | NO_HITS;
        cpu.write32(o + FLAGS, flags);
        cpu.write16(o + STEP_WORD, SHOT_DOWN);
    }
}

/// Step 100: the score for whoever shot it down (if it was: `points`, and `rate` more to the
/// stage's rate), its parent's slot for it freed; 101.
fn part_shot_down(cpu: &mut Cpu, o: u32, parent: u32, points: u32, rate: u32) {
    if cpu.read32(o + LIFE) as i32 <= 0 {
        let by = cpu.read32(o + HIT_BY);
        let (player, weapon) = (cpu.read8(by + HIT_PLAYER) as i8 as i32 as u32, cpu.read8(by + HIT_WEAPON) as i8 as i32 as u32);
        nested!(cpu, 0x0604_3108, hits::score_kill(cpu, player, weapon, o, points));
        let r = cpu.read32(STAGE_RATE_AT).wrapping_add(rate);
        cpu.write32(STAGE_RATE_AT, r);
    }
    let slot = cpu.read8(o + SLOT) as i8 as i32 as u32;
    cpu.write32(parent.wrapping_add(slot << 2) + PART_SLOTS, 0);
    cpu.write16(o + STEP_WORD, GONE);
}

/// Step 101: the hook, a burst, the end.
fn part_gone(cpu: &mut Cpu, o: u32, size: u32) {
    nested!(cpu, 0x0608_06b4, destroyed_hook(cpu, o));
    nested!(cpu, 0x0608_0610, burst(cpu, o, size));
    cpu.write8(o + OBJ_MODE, TO_STOP);
}

/// A turn word moved a 16th of the way to `to`.
fn ease_turn(cpu: &mut Cpu, at: u32, to: u32) {
    let from = cpu.read16(at) as i16 as i32 as u32;
    let d = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, from, to)) as i32 / 16;
    let t = cpu.read16(at).wrapping_add(d as u16);
    cpu.write16(at, t);
}

/// The core's count (+0x70), a tally (+0x74), its sway (+0x78: a long whose low word, +0x7a, is
/// an angle; for the circling missiles, its turn).
const C_COUNT: u32 = 0x70;
const C_TALLY: u32 = 0x74;
const C_SWAY: u32 = 0x78;
const C_SWAY_ANGLE: u32 = 0x7a;
const CORE_LIFE: u32 = 100_000;
const CORE_POINTS: u32 = 10000;
const CORE_RATE: u32 = 50;
/// It unshields once its parent's step is past 14.
const PARENT_FIGHTING: i8 = 14;
/// Step 16 picks 19-21 at random.
const ATTACKS: u16 = 19;
/// Step 17's ring: 7 shots and up to 3 more by difficulty, from 28 dots off its middle, aimed
/// out, at 1.34 dots a frame.
const RING_SHOTS: u32 = 7;
const RING_RADIUS: u32 = 0xffe4_0000;
const SHOT_SPEED: u32 = 0x1_5800;
/// Step 18's circling missiles: each a turn on (by difficulty: half, 5/8, 3/4, 3/4, 4/5 of a
/// turn), every 200, 150, 100, 100 or 80 frames.
const MISSILE_SIZE: u32 = 0xb4;
const CIRCLING_MISSILE: u32 = 0x0608_0d88;
const MISSILE_33: u8 = 8;
const MISSILE_TURNS: [(u32, u16); 5] = [(200, 0x8000), (150, 0xa000), (100, 0xc000), (100, 0xc000), (80, 0xcccd)];
/// Step 19's twin shots: 50-69 pairs; 60 frames less by difficulty (up to 30) before the
/// first, 6 less by difficulty (up to 3) between; from either side (28 dots, by the sway's
/// cosine) 16 dots up, swaying about its screen angle by an eighth of a turn's worth of its sine.
const TWIN_SIDE: u32 = 0x1c_0000;
const TWIN_UP: u32 = 0xfff0_0000;
const TWIN_SWAY: u32 = 0x2000;
const SWAY_STEP: u32 = 0x200;
/// Steps 20-22's pieces (actors of 0x47c bytes, the behaviour 0x060809b4): four, 30 frames
/// apart, each from its place (x, the turn it starts at), mirrored as the core is tilted, 8 dots
/// down.
const PIECE_SIZE: u32 = 0x47c;
const PIECE_BEHAVIOUR: u32 = 0x0608_09b4;
const PIECES: [(u32, u16); 4] = [(0x1c_0000, 0x5000), (0x10_0000, 0x6000), (0xfff0_0000, 0xa000), (0xffe4_0000, 0xb000)];
const PIECE_DOWN: u32 = 0x8_0000;
/// Step 23's sway: its turns (z, y) eased to (0x2800, 0x2000), (-0x2800, -0x2000), then (0, 0),
/// 60 frames each.
const SWAY_TURNS: [(u32, u32); 3] = [(0x2800, 0x2000), (0xffff_d800, 0xffff_e000), (0, 0)];

/// The core's behaviour (part 0 of the boss): shielded with 100000 life until its parent fights
/// (10), then 15. Its parent sets its attacks: 16, one of 19-21 at random; 17, a ring of shots;
/// 18, circling missiles; 19, twin swaying shots; 20-22, its four pieces; 23, a sway. 100: shot
/// down (10000 points, 50 to the rate); 101: a burst.
pub fn core_behaviour(cpu: &mut Cpu, o: u32) {
    let t = cpu.stack();
    let parent = nested!(cpu, 0x0605_2970, models::follow_parent(cpu, o));
    part_shot_or_parent_gone(cpu, o, parent);
    let inner = cpu.read8(o + INNER);
    match cpu.read8(o + STEP) {
        0 => {
            cpu.write32(o + LIFE, CORE_LIFE);
            let flags = cpu.read32(o + FLAGS) & !UNHURT;
            cpu.write32(o + FLAGS, flags);
            cpu.write16(o + STEP_WORD, 10);
        }
        10 => {
            if cpu.read8(parent + STEP) as i8 > PARENT_FIGHTING {
                let flags = cpu.read32(o + FLAGS) & !UNHURT;
                cpu.write32(o + FLAGS, flags);
                cpu.write16(o + STEP_WORD, FIGHTING);
            }
        }
        16 => {
            let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 3)) as i32;
            cpu.write16(o + STEP_WORD, (r.wrapping_abs() as u16).wrapping_add(ATTACKS));
        }
        17 => {
            // the count (+0x1a), the angle (+0x1c), its step (+0x1e): words after the turn's
            let (count_at, angle_at, step_at) = (t + 0x1a, t + 0x1c, t + 0x1e);
            let n = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 3)).wrapping_add(RING_SHOTS);
            cpu.write32(t + 0x44, count_at);
            cpu.write16(count_at, n as u16);
            cpu.write16(angle_at, 0);
            let n = cpu.read16(count_at) as i16 as i32 as u32;
            let step = nested!(cpu, 0x0606_62d8, clib::divide(cpu, 0x1_0000, n));
            cpu.write16(step_at, step as u16);
            cpu.write32(t + T_SCALES + 4, 0x1_0000);
            cpu.write32(t + T_SCALES, 0x1_0000);
            cpu.write32(t + T_FLAGS, 0);
            cpu.write32(t + AS_SPEED, SHOT_SPEED);
            cpu.write32(t + AS_SHOOTER, o);
            let mut i = 0;
            while i < cpu.read16(count_at) as i16 as i32 {
                cpu.write32(t + T_X, 0);
                cpu.write32(t + T_Y, RING_RADIUS);
                cpu.write32(t + T_8, 0);
                let a = cpu.read16(angle_at);
                cpu.write16(t + T_ANGLE, a);
                nested!(cpu, 0x0606_4eac, math::turn_point(cpu, t));
                nested!(cpu, 0x0605_2ca0, models::model_screen_point(cpu, o, t));
                let dx = cpu.read32(t + T_X).wrapping_sub(cpu.read32(o + POSITION));
                let dy = cpu.read32(t + T_Y).wrapping_sub(cpu.read32(o + POSITION + 4));
                let a = nested!(cpu, 0x0606_780c, sgl::sl_atan(cpu, dx, dy)) as u16;
                cpu.write16(t + T_ANGLE, a.wrapping_add(0x4000));
                nested!(cpu, 0x0604_dbc4, bullets::aimed_set(cpu, t));
                let a = cpu.read16(angle_at).wrapping_add(cpu.read16(step_at));
                cpu.write16(angle_at, a);
                i += 1;
            }
            cpu.write16(o + STEP_WORD, FIGHTING);
        }
        18 => match inner {
            0 => {
                cpu.write32(o + C_TALLY, 0);
                cpu.write16(o + C_SWAY, 0);
                cpu.write8(o + INNER, 1);
            }
            1 => {
                let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, MISSILE_SIZE)) as i8 as i32 as u32;
                let m = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
                if m != 0 {
                    nested!(cpu, 0x0604_62ac, actors::actor_setup(cpu, m));
                    let flags = cpu.read32(m + FLAGS) | FLAG_400;
                    cpu.write32(m + FLAGS, flags);
                    cpu.write32(m + BEHAVIOUR, CIRCLING_MISSILE);
                    let n = cpu.read8(o + NUMBER);
                    cpu.write8(m + NUMBER, n);
                    cpu.write8(m + BYTE_33, MISSILE_33);
                    let side = cpu.read8(o + SIDE);
                    cpu.write8(m + SIDE, side);
                    cpu.write32(m + M_TARGET, o);
                    let turn = cpu.read16(o + C_SWAY);
                    cpu.write16(m + M_TURN, turn);
                }
                let d = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 0));
                if let Some(&(wait, turn)) = MISSILE_TURNS.get(d as usize) {
                    cpu.write32(o + C_COUNT, wait);
                    let t = cpu.read16(o + C_SWAY).wrapping_add(turn);
                    cpu.write16(o + C_SWAY, t);
                }
                let tally = cpu.read32(o + C_TALLY).wrapping_add(1);
                cpu.write32(o + C_TALLY, tally);
                cpu.write8(o + INNER, 2);
            }
            2 => {
                let count = cpu.read32(o + C_COUNT).wrapping_sub(1);
                cpu.write32(o + C_COUNT, count);
                if count as i32 <= 0 {
                    cpu.write8(o + INNER, 1);
                }
            }
            _ => {}
        },
        19 => {
            cpu.write32(t + 0x40, o + INNER);
            match inner {
                0 => {
                    let d = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 30));
                    cpu.write32(o + C_COUNT, 60u32.wrapping_sub(d));
                    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 20));
                    cpu.write32(o + C_TALLY, r.wrapping_add(50));
                    cpu.write32(o + C_SWAY, 0);
                    cpu.write8(o + INNER, 1);
                }
                1 => {
                    let sway = cpu.read32(o + C_SWAY).wrapping_add(SWAY_STEP);
                    cpu.write32(o + C_SWAY, sway);
                    let count = cpu.read32(o + C_COUNT).wrapping_sub(1);
                    cpu.write32(o + C_COUNT, count);
                    if count as i32 <= 0 {
                        twin_shots(cpu, o, t);
                        cpu.write8(o + INNER, 2);
                    }
                }
                2 => {
                    let tally = cpu.read32(o + C_TALLY).wrapping_sub(1);
                    cpu.write32(o + C_TALLY, tally);
                    if tally as i32 > 0 {
                        let d = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 3));
                        cpu.write32(o + C_COUNT, 6u32.wrapping_sub(d));
                        cpu.write8(o + INNER, 1);
                    } else {
                        cpu.write16(o + STEP_WORD, FIGHTING);
                    }
                }
                _ => {}
            }
        }
        20..=22 => match inner {
            0 => {
                let d = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 30));
                cpu.write32(o + C_COUNT, 60u32.wrapping_sub(d));
                cpu.write32(o + C_TALLY, 0);
                cpu.write8(o + INNER, 1);
            }
            1 => {
                let count = cpu.read32(o + C_COUNT).wrapping_sub(1);
                cpu.write32(o + C_COUNT, count);
                if count as i32 <= 0 {
                    core_piece(cpu, o);
                    cpu.write8(o + INNER, 2);
                }
            }
            2 => {
                let tally = cpu.read32(o + C_TALLY).wrapping_add(1);
                cpu.write32(o + C_TALLY, tally);
                if tally as i32 > 3 {
                    cpu.write16(o + STEP_WORD, FIGHTING);
                } else {
                    cpu.write32(o + C_COUNT, 30);
                    cpu.write8(o + INNER, 1);
                }
            }
            _ => {}
        },
        23 => match inner {
            0 => {
                cpu.write32(o + C_COUNT, 0);
                cpu.write8(o + INNER, 1);
            }
            k @ 1..=3 => {
                let (z, y) = SWAY_TURNS[k as usize - 1];
                ease_turn(cpu, o + TURN_Z, z);
                ease_turn(cpu, o + TURN_Y, y);
                let count = cpu.read32(o + C_COUNT).wrapping_add(1);
                cpu.write32(o + C_COUNT, count);
                if count as i32 > 59 {
                    if k == 3 {
                        cpu.write16(o + STEP_WORD, FIGHTING);
                    } else {
                        cpu.write32(o + C_COUNT, 0);
                        cpu.write8(o + INNER, k + 1);
                    }
                }
            }
            _ => {}
        },
        100 => part_shot_down(cpu, o, parent, CORE_POINTS, CORE_RATE),
        101 => part_gone(cpu, o, 0x2_0000),
        _ => {}
    }
}

/// Step 19's pair (aimed_set's shot on the stack at `t`): one from each side (by the sway's
/// cosine) placed on screen, the right swaying one way, the left the other (by its sine).
fn twin_shots(cpu: &mut Cpu, o: u32, t: u32) {
    for side in [TWIN_SIDE, TWIN_SIDE.wrapping_neg()] {
        let sway = cpu.read16(o + C_SWAY_ANGLE) as i16 as i32 as u32;
        let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, sway));
        let x = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, side, c));
        cpu.write32(t + T_X, x);
        cpu.write32(t + T_Y, TWIN_UP);
        cpu.write32(t + T_8, 0);
        nested!(cpu, 0x0605_2ca0, models::model_screen_point(cpu, o, t));
        let a = nested!(cpu, 0x0608_054c, model_screen_angle(cpu, o));
        let sway = cpu.read16(o + C_SWAY_ANGLE) as i16 as i32 as u32;
        let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, sway));
        let m = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, TWIN_SWAY, s));
        cpu.write32(t + 0x48, t + AS_ANGLE);
        if side == TWIN_SIDE {
            cpu.write16(t + AS_ANGLE, m.wrapping_add(a) as u16);
            cpu.write32(t + AS_FLAGS, 0);
            cpu.write32(t + AS_SPEED, SHOT_SPEED);
            cpu.write32(t + AS_SHOOTER, o);
        } else {
            cpu.write16(t + AS_ANGLE, a.wrapping_sub(m) as u16);
        }
        nested!(cpu, 0x0604_dbc4, bullets::aimed_set(cpu, t));
    }
}

/// Steps 20-22's piece (by the tally): at its place in the core's model, its turn on by the
/// core's screen angle.
fn core_piece(cpu: &mut Cpu, o: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, PIECE_SIZE)) as i8 as i32 as u32;
    let p = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if p == 0 {
        return;
    }
    nested!(cpu, 0x0604_62ac, actors::actor_setup(cpu, p));
    let flags = cpu.read32(p + FLAGS) | FLAG_400;
    cpu.write32(p + FLAGS, flags);
    cpu.write32(p + BEHAVIOUR, PIECE_BEHAVIOUR);
    if let Some(&(x, turn)) = PIECES.get(cpu.read32(o + C_TALLY) as usize) {
        cpu.write32(p + POSITION, x);
        cpu.write16(p + TURN_Z, turn);
    }
    let tilt = nested!(cpu, 0x0608_051c, model_tilt_angle(cpu, o)) as i16 as i32;
    if tilt.wrapping_abs() > 0x3fff {
        let x = cpu.read32(p + POSITION).wrapping_neg();
        cpu.write32(p + POSITION, x);
    }
    cpu.write32(p + POSITION + 4, PIECE_DOWN);
    cpu.write32(p + POSITION + 8, 0);
    nested!(cpu, 0x0605_2ca0, models::model_screen_point(cpu, o, p + POSITION));
    let a = nested!(cpu, 0x0608_054c, model_screen_angle(cpu, o)) as u16;
    let turn = cpu.read16(p + TURN_Z).wrapping_add(a);
    cpu.write16(p + TURN_Z, turn);
    let n = cpu.read8(o + NUMBER);
    cpu.write8(p + NUMBER, n);
    let side = cpu.read8(o + SIDE);
    cpu.write8(p + SIDE, side);
}

// ---- the rocket ----------------------------------------------------------------------------------

/// A shot's hit flags (a word): it has hit something; the flags: off the screen.
const HIT_FLAGS: u32 = 0x56;
const HIT_SOMETHING: u16 = 0x400;
const OFF_SCREEN: u32 = 0x0200_0000;
/// The rocket (an actor's behaviour): its count, its speed, the distance it has gone (a long:
/// 16 dots a line of its trail; its high word the line's length), its corners (words, +0x7c);
/// the part it is launched from (+0xb0).
const R_COUNT: u32 = 0x70;
const R_SPEED: u32 = 0x74;
const R_GONE: u32 = 0x78;
const R_CORNERS: u32 = 0x7c;
const R_FROM: u32 = 0xb0;
/// A polygon of its own corners (sprites.rs's POLYGON_BITS 2): a thin dart.
const DART: u32 = 2;
const DART_CORNERS: [(u32, u16); 8] =
    [(0, 0xfffe), (4, 2), (6, 0xfff8), (2, 0xfff8), (8, 4), (0xc, 0xfffc), (0xa, 4), (0xe, 4)];
const ROCKET_START_Y: u32 = 0xffe5_0000;
const ROCKET_ANIMATION: u32 = 0x060a_2ed4;
/// It speeds up to 2 dots a frame (half a dot more or less by difficulty), by 3/64 of a dot (a
/// 64th more or less by difficulty) a frame.
const ROCKET_SPEED: u32 = 0x2_0000;
const ROCKET_SPEED_BY_DIFFICULTY: u32 = 0x8000;
const ROCKET_ACCEL: u32 = 0xc00;
const ROCKET_ACCEL_BY_DIFFICULTY: u32 = 0x400;
const TRAIL_EVERY: i32 = 0xf_ffff;
const TRAIL_STEP: u32 = 0x10_0000;

/// The rocket's behaviour: ended once it has hit something, hits the ground or leaves the
/// screen. Step 0: a dart 27 dots up its part's model, on screen. 1: on along its angle,
/// speeding up. A line of its trail each 16 dots; its colour flickers.
pub fn rocket_behaviour(cpu: &mut Cpu, o: u32) {
    let hit = cpu.read16(o + HIT_FLAGS) & HIT_SOMETHING != 0;
    let (x, y) = (cpu.read32(o + POSITION), cpu.read32(o + POSITION + 4));
    if hit || nested!(cpu, 0x0605_ef90, bg::map_hit(cpu, x, y)) != 0 || cpu.read32(o + FLAGS) & OFF_SCREEN != 0 {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    match cpu.read8(o + STEP) {
        0 => {
            let flags = cpu.read32(o + FLAGS) | DART;
            cpu.write32(o + FLAGS, flags);
            cpu.write32(o + CORNERS_AT, o + R_CORNERS);
            for (k, v) in DART_CORNERS {
                cpu.write16(o + R_CORNERS + k, v);
            }
            let colour = cpu.read16(kind(EFFECT_KIND) + K_COLOUR);
            cpu.write16(o + OWN_COLOUR, colour);
            cpu.write16(o + COLOUR, colour);
            cpu.write32(o + POSITION, 0);
            cpu.write32(o + POSITION + 4, ROCKET_START_Y);
            cpu.write32(o + POSITION + 8, 0);
            let from = cpu.read32(o + R_FROM);
            nested!(cpu, 0x0605_2ca0, models::model_screen_point(cpu, from, o + POSITION));
            cpu.write32(o + POSITION + 8, 0);
            cpu.write32(o + M_ANIMATION, ROCKET_ANIMATION);
            for at in [R_COUNT, R_SPEED, R_GONE] {
                cpu.write32(o + at, 0);
            }
            cpu.write16(o + STEP_WORD, 1);
        }
        1 => {
            let accel = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, ROCKET_ACCEL_BY_DIFFICULTY)).wrapping_add(ROCKET_ACCEL);
            let to = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, ROCKET_SPEED_BY_DIFFICULTY)).wrapping_add(ROCKET_SPEED);
            let speed = cpu.read32(o + R_SPEED);
            let speed = nested!(cpu, 0x0604_6168, actors::approach(cpu, speed, to, accel));
            cpu.write32(o + R_SPEED, speed);
            let angle = cpu.read16(o + ANGLE) as i16 as i32 as u32;
            nested!(cpu, 0x0604_d8d8, actors::move_by_angle(cpu, o, speed, angle));
        }
        _ => {}
    }
    let gone = cpu.read32(o + R_GONE).wrapping_add(cpu.read32(o + R_SPEED));
    cpu.write32(o + R_GONE, gone);
    while cpu.read32(o + R_GONE) as i32 > TRAIL_EVERY {
        let gone = cpu.read32(o + R_GONE).wrapping_sub(TRAIL_STEP);
        cpu.write32(o + R_GONE, gone);
        let length = cpu.read16(o + R_GONE) as i16 as i32 as u32;
        nested!(cpu, 0x0608_057c, line_effect(cpu, o, length));
    }
    let count = cpu.read32(o + R_COUNT).wrapping_add(1);
    cpu.write32(o + R_COUNT, count);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 6)) as u16;
    let colour = cpu.read16(kind(EFFECT_KIND) + K_COLOUR).wrapping_add(r.wrapping_add(6));
    cpu.write16(o + OWN_COLOUR, colour);
    cpu.write16(o + COLOUR, colour);
}

// ---- the boss's rocket launcher ------------------------------------------------------------------

/// The boss the parts belong to (+0xd4: the top of their parents).
const BOSS: u32 = 0xd4;
/// The launcher's count (+0x70) and volleys left (+0x74).
const LAUNCHER_COUNT: u32 = 0x70;
const VOLLEYS: u32 = 0x74;
const LAUNCHER_LIFE: u32 = 40000;
const LAUNCHER_POINTS: u32 = 10000;
const LAUNCHER_RATE: u32 = 10;
/// Its turn back toward 0 at most 0x40 a frame (step 15); aiming, up to an eighth of a turn
/// either way of the boss's screen angle toward the nearer player (mirrored as the boss is
/// tilted), a 16th of the way a frame.
const TURN_BACK: u32 = 0x40;
const LAUNCHER_REACH: u32 = 0x2000;
/// Five volleys, 60 frames to the first and 30 between: each a fan of 6 rockets and 2 more by
/// difficulty (7 and 2 more in odd volleys), over a quarter turn about its screen angle less an
/// eighth.
const VOLLEY_COUNT: u32 = 5;
const VOLLEY_ROCKETS: u32 = 6;
const VOLLEY_FAN: u32 = 0x4000;
const VOLLEY_FROM: u16 = 0xe000;
const ROCKET: u32 = 0x0608_1a00;
const ROCKET_SIZE: u32 = 0xb4;
const ROCKET_33: u8 = 8;
/// Volleys done, the inner step 100 makes its step 22 (back to 15).
const VOLLEYS_DONE: u8 = 100;

/// The launcher's behaviour (part 1 of the boss): 40000 life, shielded until its parent fights
/// (10), then 15 (its turn back toward 0). 16: 17 (a random number taken). 17-21: five volleys
/// of rockets, aimed between; then 22, 15. 100: shot down (10000 points, 10 to the rate); 101: a
/// burst.
pub fn launcher_behaviour(cpu: &mut Cpu, o: u32) {
    let t = cpu.stack();
    let parent = nested!(cpu, 0x0605_2970, models::follow_parent(cpu, o));
    let boss = cpu.read32(o + BOSS);
    part_shot_or_parent_gone(cpu, o, boss);
    match cpu.read8(o + STEP) {
        0 => {
            cpu.write32(o + LIFE, LAUNCHER_LIFE);
            cpu.write16(o + STEP_WORD, 10);
        }
        10 => {
            if cpu.read8(parent + STEP) as i8 > PARENT_FIGHTING {
                let flags = cpu.read32(o + FLAGS) & !UNHURT;
                cpu.write32(o + FLAGS, flags);
                cpu.write16(o + STEP_WORD, FIGHTING);
            }
        }
        15 => turn_back(cpu, o),
        16 => {
            nested!(cpu, 0x0604_5d90, math::random(cpu, 4));
            cpu.write16(o + STEP_WORD, 17);
        }
        17..=21 => match cpu.read8(o + INNER) {
            0 => {
                cpu.write32(o + LAUNCHER_COUNT, 60);
                cpu.write32(o + VOLLEYS, VOLLEY_COUNT);
                cpu.write8(o + INNER, 1);
            }
            1 => {
                aim_within(cpu, o, LAUNCHER_REACH, 16);
                let count = cpu.read32(o + LAUNCHER_COUNT).wrapping_sub(1);
                cpu.write32(o + LAUNCHER_COUNT, count);
                if count as i32 <= 0 {
                    cpu.write8(o + INNER, 2);
                }
            }
            2 => {
                volley(cpu, o, t);
                let left = cpu.read32(o + VOLLEYS).wrapping_sub(1);
                cpu.write32(o + VOLLEYS, left);
                if left as i32 > 0 {
                    cpu.write32(o + LAUNCHER_COUNT, 30);
                    cpu.write8(o + INNER, 1);
                } else {
                    cpu.write8(o + INNER, VOLLEYS_DONE);
                }
            }
            VOLLEYS_DONE => cpu.write16(o + STEP_WORD, 22),
            _ => {}
        },
        22 => cpu.write16(o + STEP_WORD, FIGHTING),
        100 => part_shot_down(cpu, o, parent, LAUNCHER_POINTS, LAUNCHER_RATE),
        101 => part_gone(cpu, o, 0x1_0000),
        _ => {}
    }
}

/// Its turn (z) moved a `ease`th of the way toward the nearer player, kept within `reach` either
/// way of the boss's screen angle (mirrored as the boss is tilted).
fn aim_within(cpu: &mut Cpu, o: u32, reach: u32, ease: i32) {
    let player = nested!(cpu, 0x0604_5f70, aim::nearer_player_to(cpu, o));
    let to = nested!(cpu, 0x0604_5e30, aim::angle_to(cpu, o, player)) as i16 as i32;
    let boss = cpu.read32(o + BOSS);
    let screen = nested!(cpu, 0x0608_054c, model_screen_angle(cpu, boss)) as i16 as i32;
    let mut off = to.wrapping_sub(screen);
    let boss = cpu.read32(o + BOSS);
    let tilt = nested!(cpu, 0x0608_051c, model_tilt_angle(cpu, boss)) as i16 as i32;
    if tilt.wrapping_abs() > 0x3fff {
        off = off.wrapping_neg();
    }
    let d = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, 0, off as u32));
    let goal = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, d, reach)) as i32;
    let a = cpu.read16(o + TURN_Z) as i16;
    let a = a.wrapping_add((goal.wrapping_sub(a as i32) / ease) as i16);
    cpu.write16(o + TURN_Z, a as u16);
}

/// Its turn (z) back toward 0, at most 0x40 a frame.
fn turn_back(cpu: &mut Cpu, o: u32) {
    let a = cpu.read16(o + TURN_Z) as i16 as i32 as u32;
    let d = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, a, 0));
    let d = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, d, TURN_BACK));
    let a = cpu.read16(o + TURN_Z).wrapping_add(d as u16);
    cpu.write16(o + TURN_Z, a);
}

/// A volley (the angle and its step, words on the stack at +0x1c, +0x1e): the rockets fanned
/// out (as many as there is room for).
fn volley(cpu: &mut Cpu, o: u32, t: u32) {
    let (angle_at, step_at) = (t + 0x1c, t + 0x1e);
    let extra = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 2)).wrapping_mul(2);
    let mut n = extra.wrapping_add(VOLLEY_ROCKETS) as i32;
    let a = nested!(cpu, 0x0608_054c, model_screen_angle(cpu, o)) as u16;
    let a = a.wrapping_add(VOLLEY_FROM);
    cpu.write16(angle_at, a);
    let step = nested!(cpu, 0x0606_62d8, clib::divide(cpu, VOLLEY_FAN, n as u32));
    cpu.write16(step_at, step as u16);
    if cpu.read32(o + VOLLEYS) & 1 != 0 {
        n = extra.wrapping_add(VOLLEY_ROCKETS + 1) as i32;
    } else {
        let half = (cpu.read16(step_at) as i16 as i32 / 2) as u16;
        cpu.write16(angle_at, a.wrapping_add(half));
    }
    let mut i = 0;
    while i < n {
        let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, ROCKET_SIZE)) as i8 as i32 as u32;
        let r = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
        if r == 0 {
            break;
        }
        nested!(cpu, 0x0604_62ac, actors::actor_setup(cpu, r));
        i += 1;
        let flags = cpu.read32(r + FLAGS) | FLAG_400;
        cpu.write32(r + FLAGS, flags);
        cpu.write32(r + BEHAVIOUR, ROCKET);
        let n = cpu.read8(o + NUMBER);
        cpu.write8(r + NUMBER, n);
        cpu.write8(r + BYTE_33, ROCKET_33);
        let side = cpu.read8(o + SIDE);
        cpu.write8(r + SIDE, side);
        let w = cpu.read8(o + HIT_WEAPON);
        cpu.write8(r + HIT_WEAPON, w);
        cpu.write32(r + R_FROM, o);
        let a = cpu.read16(angle_at);
        cpu.write16(r + ANGLE, a);
        let a = cpu.read16(angle_at).wrapping_add(cpu.read16(step_at));
        cpu.write16(angle_at, a);
    }
}

// ---- the laser ---------------------------------------------------------------------------------

/// The laser (an actor of the gun that fires it, +0xb0): its count, its length (16.16 dots), its
/// corners (words, +0x78), its boxes (models.rs's beam boxes, +0xb4; pointed at by +0x3c), its
/// width (the sprite's x scale, +0x20).
const LZ_GUN: u32 = 0xb0;
const LZ_COUNT: u32 = 0x70;
const LZ_LENGTH: u32 = 0x74;
const LZ_CORNERS: u32 = 0x78;
const LZ_BOXES: u32 = 0xb4;
const LZ_BOXES_AT: u32 = 0x3c;
const LZ_WIDTH: u32 = 0x20;
const LZ_HIT_5C: u32 = 0x5c;
const LZ_HIT_60: u32 = 0x60;
/// It starts 32 dots up the gun's model, along its screen angle; hidden, unhit and a polygon
/// while it charges (the flags 0x8008_0001).
const LZ_PLACE: u32 = 0xffe0_0000;
const CHARGING: u32 = 0x8008_0001;
const LZ_SHAPE: [(u32, u16); 8] = [(0, 0xfff8), (4, 8), (0xc, 0xfffd), (8, 3), (0xe, 0), (0xa, 0), (6, 0), (2, 0)];
/// Charging: 16 dots more a frame (4 more or less by difficulty), a spark each 16 dots up to
/// 256 (polygon_set's: flags 0xb2, 30 frames, a square 4 dots wide, placed with it); then 8 less
/// by difficulty (up to 5) frames.
const CHARGE_STEP: u32 = 0x10_0000;
const CHARGE_BY_DIFFICULTY: u32 = 0x4_0000;
const CHARGED: i32 = 0xff_ffff;
const SPARK_FLAGS: u32 = 0xb2;
const SPARK_FRAMES: u16 = 30;
const SPARK_SQUARE: [(u32, u16); 8] = [(0xc, 0xfffe), (0, 0xfffe), (8, 2), (4, 2), (6, 0xfffe), (2, 0xfffe), (0xe, 2), (0xa, 2)];
/// Firing: its boxes (from 0x060a2edc), its width 1 and up to 3/4 more by difficulty (2 at the
/// hardest), sound 105; it grows 16 dots a frame to 320, then (unhit) narrows by an eighth a
/// frame over 16 frames, its colour on.
const LZ_BOX: u32 = 0x060a_2edc;
const LZ_WIDE: u32 = 0x1_0000;
const LZ_WIDE_BY_DIFFICULTY: u32 = 0xc000;
const LZ_WIDEST: u32 = 0x2_0000;
const HARDEST: u32 = 4;
const LZ_SOUND: u32 = 105;
const LZ_REACH: i32 = 0x13f_ffff;
const LZ_FADE_FRAMES: u32 = 16;

/// The laser's behaviour: ended with its gun. Kept at its gun's muzzle and angle. 0: its shape;
/// 1: it charges; 2: it waits; 15: it fires, growing; 16: it fades and ends.
pub fn laser_behaviour(cpu: &mut Cpu, o: u32) {
    let gun = cpu.read32(o + LZ_GUN);
    if cpu.read8(gun + STEP) as i8 > DONE_FOR || cpu.read8(gun + OBJ_MODE) as i8 > ENDING {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    cpu.write32(o + POSITION, 0);
    cpu.write32(o + POSITION + 4, LZ_PLACE);
    cpu.write32(o + POSITION + 8, 0);
    nested!(cpu, 0x0605_2ca0, models::model_screen_point(cpu, gun, o + POSITION));
    let a = nested!(cpu, 0x0608_054c, model_screen_angle(cpu, gun));
    cpu.write16(o + ANGLE, a as u16);
    match cpu.read8(o + STEP) {
        0 => {
            let flags = cpu.read32(o + FLAGS) | CHARGING;
            cpu.write32(o + FLAGS, flags);
            cpu.write32(o + CORNERS_AT, o + LZ_CORNERS);
            cpu.write32(o + LZ_BOXES_AT, o + LZ_BOXES);
            for (k, v) in LZ_SHAPE {
                cpu.write16(o + LZ_CORNERS + k, v);
            }
            let colour = cpu.read16(kind(EFFECT_KIND) + K_COLOUR);
            cpu.write16(o + OWN_COLOUR, colour);
            cpu.write16(o + COLOUR, colour);
            cpu.write32(o + LZ_LENGTH, 0);
            cpu.write32(o + LZ_COUNT, 0);
            cpu.write16(o + STEP_WORD, 1);
        }
        1 => {
            let more = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, CHARGE_BY_DIFFICULTY));
            let length = cpu.read32(o + LZ_LENGTH).wrapping_add(CHARGE_STEP).wrapping_add(more);
            cpu.write32(o + LZ_LENGTH, length);
            let fx = cpu.stack();
            loop {
                let length = cpu.read32(o + LZ_LENGTH);
                let sparks = nested!(cpu, 0x0600_6a78, math::quotient(cpu, length, CHARGE_STEP)) as i32;
                let n = cpu.read32(o + LZ_COUNT);
                if n as i32 > sparks {
                    break;
                }
                cpu.write32(fx + effects::X, 0);
                cpu.write32(fx + effects::X + 8, 0);
                cpu.write32(fx + effects::X + 4, n.wrapping_neg().wrapping_mul(CHARGE_STEP));
                cpu.write32(fx + effects::SCALE_Y, 0x1_0000);
                cpu.write32(fx + effects::SCALE_X, 0x1_0000);
                cpu.write32(fx + effects::FLAGS_LONG, SPARK_FLAGS);
                cpu.write16(fx + effects::ANGLE, 0);
                cpu.write16(fx + effects::FRAMES, SPARK_FRAMES);
                cpu.write8(fx + effects::FRAMES_2, 0);
                let colour = cpu.read16(kind(EFFECT_KIND) + K_COLOUR);
                cpu.write16(fx + effects::COLOUR, colour);
                let number = cpu.read8(o + NUMBER);
                cpu.write8(fx + effects::PLAYER, number);
                cpu.write32(fx + effects::RELATIVE_3C, o);
                let p = nested!(cpu, 0x0604_e6d8, effects::polygon_set(cpu, fx));
                if p != 0 {
                    for (k, v) in SPARK_SQUARE {
                        cpu.write16(p + P_CORNERS + k, v);
                    }
                }
                let n = cpu.read32(o + LZ_COUNT).wrapping_add(1);
                cpu.write32(o + LZ_COUNT, n);
            }
            if cpu.read32(o + LZ_LENGTH) as i32 > CHARGED {
                let d = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 5));
                cpu.write32(o + LZ_COUNT, 8u32.wrapping_sub(d));
                cpu.write16(o + STEP_WORD, 2);
            }
        }
        2 => {
            let count = cpu.read32(o + LZ_COUNT).wrapping_sub(1);
            cpu.write32(o + LZ_COUNT, count);
            if count as i32 <= 0 {
                let flags = cpu.read32(o + FLAGS) & !(CHARGING & !1);
                cpu.write32(o + FLAGS, flags);
                nested!(cpu, 0x0605_2b08, models::beam_boxes(cpu, o, LZ_BOX));
                cpu.write32(o + LZ_HIT_60, 0);
                cpu.write32(o + LZ_HIT_5C, 0);
                cpu.write32(o + LZ_LENGTH, 0);
                let w = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, LZ_WIDE_BY_DIFFICULTY));
                cpu.write32(o + LZ_WIDTH, w.wrapping_add(LZ_WIDE));
                if nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 0)) == HARDEST {
                    cpu.write32(o + LZ_WIDTH, LZ_WIDEST);
                }
                nested!(cpu, 0x0600_50d4, sound::se_set(cpu, LZ_SOUND));
                cpu.write16(o + STEP_WORD, FIGHTING);
            }
        }
        15 => {
            let length = cpu.read32(o + LZ_LENGTH).wrapping_add(CHARGE_STEP);
            cpu.write32(o + LZ_LENGTH, length);
            let tip = (cpu.read16(o + LZ_LENGTH) as i16).wrapping_neg() as u16;
            cpu.write16(o + LZ_CORNERS + 6, tip);
            cpu.write16(o + LZ_CORNERS + 2, tip);
            let length = cpu.read32(o + LZ_LENGTH);
            nested!(cpu, 0x0605_2b88, models::beam_length(cpu, o, length));
            if cpu.read32(o + LZ_LENGTH) as i32 > LZ_REACH {
                let flags = cpu.read32(o + FLAGS) | NO_HITS;
                cpu.write32(o + FLAGS, flags);
                cpu.write32(o + LZ_COUNT, LZ_FADE_FRAMES);
                cpu.write16(o + STEP_WORD, 16);
            }
        }
        16 => {
            let colour = cpu.read16(o + OWN_COLOUR).wrapping_add(1);
            cpu.write16(o + OWN_COLOUR, colour);
            let w = cpu.read32(o + LZ_WIDTH) as i32;
            cpu.write32(o + LZ_WIDTH, w.wrapping_add(w.wrapping_neg() / 8) as u32);
            let count = cpu.read32(o + LZ_COUNT).wrapping_sub(1);
            cpu.write32(o + LZ_COUNT, count);
            if count as i32 <= 0 {
                cpu.write8(o + OBJ_MODE, TO_STOP);
            }
        }
        _ => {}
    }
}

// ---- the boss's laser gun ---------------------------------------------------------------------

/// The laser gun's count (+0x70), shots left (+0x74), its laser (+0x78).
const GUN_COUNT: u32 = 0x70;
const GUN_SHOTS: u32 = 0x74;
const GUN_LASER: u32 = 0x78;
const GUN_LIFE: u32 = 40000;
const GUN_POINTS: u32 = 10000;
const GUN_RATE: u32 = 10;
/// Three lasers, each aimed for 15 frames first (within 3/16 of a turn, a quarter of the way a
/// frame), the next once the last has ended.
const GUN_SHOT_COUNT: u32 = 3;
const GUN_AIM_FRAMES: u32 = 15;
const GUN_REACH: u32 = 0x3000;
const LASER: u32 = 0x0608_1f80;
const LASER_SIZE: u32 = 0x174;
const LASER_33: u8 = 8;
const GUN_DONE: u16 = 20;

/// The laser gun's behaviour (part 2 of the boss): 40000 life, shielded until its parent fights
/// (10), then 15 (its turn back toward 0). 16: 17 (a random number taken). 17-19: three lasers;
/// then 20, 15. 100: shot down (10000 points, 10 to the rate); 101: a burst.
pub fn laser_gun_behaviour(cpu: &mut Cpu, o: u32) {
    let parent = nested!(cpu, 0x0605_2970, models::follow_parent(cpu, o));
    let boss = cpu.read32(o + BOSS);
    part_shot_or_parent_gone(cpu, o, boss);
    match cpu.read8(o + STEP) {
        0 => {
            cpu.write32(o + LIFE, GUN_LIFE);
            cpu.write16(o + STEP_WORD, 10);
        }
        10 => {
            if cpu.read8(parent + STEP) as i8 > PARENT_FIGHTING {
                let flags = cpu.read32(o + FLAGS) & !UNHURT;
                cpu.write32(o + FLAGS, flags);
                cpu.write16(o + STEP_WORD, FIGHTING);
            }
        }
        15 => turn_back(cpu, o),
        16 => {
            nested!(cpu, 0x0604_5d90, math::random(cpu, 2));
            cpu.write16(o + STEP_WORD, 17);
        }
        17..=19 => match cpu.read8(o + INNER) {
            0 => {
                cpu.write32(o + GUN_COUNT, GUN_AIM_FRAMES);
                cpu.write32(o + GUN_SHOTS, GUN_SHOT_COUNT);
                cpu.write8(o + INNER, 1);
            }
            1 => {
                aim_within(cpu, o, GUN_REACH, 4);
                let count = cpu.read32(o + GUN_COUNT).wrapping_sub(1);
                cpu.write32(o + GUN_COUNT, count);
                if count as i32 <= 0 {
                    cpu.write8(o + INNER, 2);
                }
            }
            2 => {
                let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, LASER_SIZE)) as i8 as i32 as u32;
                let l = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
                if l != 0 {
                    nested!(cpu, 0x0604_62ac, actors::actor_setup(cpu, l));
                    let flags = cpu.read32(l + FLAGS) | FLAG_400;
                    cpu.write32(l + FLAGS, flags);
                    cpu.write32(l + BEHAVIOUR, LASER);
                    let n = cpu.read8(o + NUMBER);
                    cpu.write8(l + NUMBER, n);
                    cpu.write8(l + BYTE_33, LASER_33);
                    let side = cpu.read8(o + SIDE);
                    cpu.write8(l + SIDE, side);
                    let w = cpu.read8(o + HIT_WEAPON);
                    cpu.write8(l + HIT_WEAPON, w);
                    cpu.write32(l + LZ_GUN, o);
                }
                cpu.write32(o + GUN_LASER, l);
                cpu.write8(o + INNER, 3);
                if l == 0 {
                    cpu.write16(o + STEP_WORD, GUN_DONE);
                }
            }
            3 => {
                let laser = cpu.read32(o + GUN_LASER);
                if cpu.read8(laser + OBJ_MODE) as i8 > ENDING {
                    let left = cpu.read32(o + GUN_SHOTS).wrapping_sub(1);
                    cpu.write32(o + GUN_SHOTS, left);
                    if left as i32 > 0 {
                        cpu.write32(o + GUN_COUNT, GUN_AIM_FRAMES);
                        cpu.write8(o + INNER, 1);
                    } else {
                        cpu.write16(o + STEP_WORD, GUN_DONE);
                    }
                }
            }
            _ => {}
        },
        20 => cpu.write16(o + STEP_WORD, FIGHTING),
        100 => part_shot_down(cpu, o, parent, GUN_POINTS, GUN_RATE),
        101 => part_gone(cpu, o, 0x1_0000),
        _ => {}
    }
}

// ---- the boss's pods ---------------------------------------------------------------------------

/// A pod's bob (a word counting frames), its scales (+0x20, +0x24), its place in its group
/// (models.rs's, +0xd8: x, y).
const BOB: u32 = 0x76;
const SCALES: u32 = 0x20;
const PLACE_IN_GROUP: u32 = 0xd8;
const POD_LIFE: u32 = 50000;
const POD_POINTS: u32 = 100_000;
const POD_SCALE: u32 = 0x1_8000;
/// The pods are parts 3-6, set about the boss a quarter turn apart.
const FIRST_POD: u8 = 3;
const POD_TURNS: [u16; 4] = [0xe000, 0x2000, 0x6000, 0xa000];
/// Each bobs 6 dots either way of 32 dots out (once in 512 frames), along its turn.
const POD_BOB: u32 = 0x6_0000;
const POD_OUT: u32 = 0xffe0_0000;

/// A pod's behaviour (parts 3-6 of the boss): 50000 life, its turn by its slot, scaled 1.5,
/// shielded until its parent fights (10), then 15. 100: shot down (100000 points); 101: a burst.
/// Each frame its place in its group bobs out and in along its turn.
pub fn pod_behaviour(cpu: &mut Cpu, o: u32) {
    let parent = nested!(cpu, 0x0605_2970, models::follow_parent(cpu, o));
    let boss = cpu.read32(o + BOSS);
    part_shot_or_parent_gone(cpu, o, boss);
    match cpu.read8(o + STEP) {
        0 => {
            cpu.write32(o + LIFE, POD_LIFE);
            let k = cpu.read8(o + SLOT).wrapping_sub(FIRST_POD);
            if let Some(&turn) = POD_TURNS.get(k as usize) {
                cpu.write16(o + TURN_Z, turn);
            }
            cpu.write32(o + SCALES + 4, POD_SCALE);
            cpu.write32(o + SCALES, POD_SCALE);
            cpu.write16(o + BOB, 0);
            cpu.write16(o + STEP_WORD, 10);
        }
        10 => {
            let bob = cpu.read16(o + BOB).wrapping_add(1);
            cpu.write16(o + BOB, bob);
            if cpu.read8(parent + STEP) as i8 > PARENT_FIGHTING {
                let flags = cpu.read32(o + FLAGS) & !UNHURT;
                cpu.write32(o + FLAGS, flags);
                cpu.write16(o + STEP_WORD, FIGHTING);
            }
        }
        15 => {
            let bob = cpu.read16(o + BOB).wrapping_add(1);
            cpu.write16(o + BOB, bob);
        }
        100 => {
            if cpu.read32(o + LIFE) as i32 <= 0 {
                let by = cpu.read32(o + HIT_BY);
                let (player, weapon) = (cpu.read8(by + HIT_PLAYER) as i8 as i32 as u32, cpu.read8(by + HIT_WEAPON) as i8 as i32 as u32);
                nested!(cpu, 0x0604_3108, hits::score_kill(cpu, player, weapon, o, POD_POINTS));
            }
            let slot = cpu.read8(o + SLOT) as i8 as i32 as u32;
            cpu.write32(parent.wrapping_add(slot << 2) + PART_SLOTS, 0);
            cpu.write16(o + STEP_WORD, GONE);
        }
        101 => part_gone(cpu, o, 0x1_0000),
        _ => {}
    }
    let t = cpu.stack();
    cpu.write32(t + T_X, 0);
    let a = ((cpu.read16(o + BOB) as u32) << 7) as i16 as i32 as u32;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    let y = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, POD_BOB, s)).wrapping_add(POD_OUT);
    cpu.write32(t + T_Y, y);
    let (sx, sy) = (cpu.read32(o + SCALES), cpu.read32(o + SCALES + 4));
    cpu.write32(t + T_SCALES, sx);
    cpu.write32(t + T_SCALES + 4, sy);
    cpu.write32(t + T_FLAGS, 0);
    let turn = cpu.read16(o + TURN_Z);
    cpu.write16(t + T_ANGLE, turn);
    nested!(cpu, 0x0606_4eac, math::turn_point(cpu, t));
    let (x, y) = (cpu.read32(t + T_X), cpu.read32(t + T_Y));
    cpu.write32(o + PLACE_IN_GROUP, x);
    cpu.write32(o + PLACE_IN_GROUP + 4, y);
}

// ---- the boss's fins ---------------------------------------------------------------------------

/// A fin's turns (x +0x2c, z +0x30).
const TURN_X: u32 = 0x2c;
const FIN_LIFE: u32 = 10000;

/// A fin (parts 7-9 of the boss): its set turns (x, z), its points, how far it sways (its turn x
/// by the cosine of the game's frames, once in 256 frames), and whether it sways only while it
/// fights (else always).
struct Fin {
    turn_x: u16,
    turn_z: u16,
    points: u32,
    sway: u32,
    fighting_only: bool,
}

/// Its turn x swayed about its set turn.
fn fin_sway(cpu: &mut Cpu, o: u32, fin: &Fin) {
    let frames = cpu.read32(GAME_FRAMES);
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, (frames << 8) as i16 as i32 as u32));
    let s = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, fin.sway, c)) as u16;
    cpu.write16(o + TURN_X, s.wrapping_add(fin.turn_x));
}

/// A fin's behaviour: 10000 life, its turns set, shielded until its parent fights (10), then 15.
/// 100: shot down (its points); 101: a burst.
fn fin_behaviour(cpu: &mut Cpu, o: u32, fin: Fin) {
    let parent = nested!(cpu, 0x0605_2970, models::follow_parent(cpu, o));
    let boss = cpu.read32(o + BOSS);
    part_shot_or_parent_gone(cpu, o, boss);
    match cpu.read8(o + STEP) {
        0 => {
            cpu.write32(o + LIFE, FIN_LIFE);
            cpu.write16(o + TURN_X, fin.turn_x);
            cpu.write16(o + TURN_Z, fin.turn_z);
            cpu.write16(o + STEP_WORD, 10);
        }
        10 => {
            if cpu.read8(parent + STEP) as i8 > PARENT_FIGHTING {
                let flags = cpu.read32(o + FLAGS) & !UNHURT;
                cpu.write32(o + FLAGS, flags);
                cpu.write16(o + STEP_WORD, FIGHTING);
            }
        }
        15 if fin.fighting_only => fin_sway(cpu, o, &fin),
        100 => {
            if cpu.read32(o + LIFE) as i32 <= 0 {
                let by = cpu.read32(o + HIT_BY);
                let (player, weapon) = (cpu.read8(by + HIT_PLAYER) as i8 as i32 as u32, cpu.read8(by + HIT_WEAPON) as i8 as i32 as u32);
                nested!(cpu, 0x0604_3108, hits::score_kill(cpu, player, weapon, o, fin.points));
            }
            let slot = cpu.read8(o + SLOT) as i8 as i32 as u32;
            cpu.write32(parent.wrapping_add(slot << 2) + PART_SLOTS, 0);
            cpu.write16(o + STEP_WORD, GONE);
        }
        101 => part_gone(cpu, o, 0x1_0000),
        _ => {}
    }
    if !fin.fighting_only {
        fin_sway(cpu, o, &fin);
    }
}

/// The fins (parts 7, 8 and 9).
pub fn fin_7_behaviour(cpu: &mut Cpu, o: u32) {
    fin_behaviour(cpu, o, Fin { turn_x: 0xe000, turn_z: 0xa38f, points: 10000, sway: 0xc00, fighting_only: true });
}

pub fn fin_8_behaviour(cpu: &mut Cpu, o: u32) {
    fin_behaviour(cpu, o, Fin { turn_x: 0x1c71, turn_z: 0x5c76, points: 5000, sway: 0x800, fighting_only: false });
}

pub fn fin_9_behaviour(cpu: &mut Cpu, o: u32) {
    fin_behaviour(cpu, o, Fin { turn_x: 0x0e38, turn_z: 0x871d, points: 5000, sway: 0x800, fighting_only: false });
}

// ---- the boss's spinner ------------------------------------------------------------------------

/// The spinner's wobble (a word, +0x72), its spin (a word: its turn y on by it a frame, +0x74),
/// a word set to 4 (+0x76), its shots' count (+0x78); the parts it flashes with (its parent's
/// +0xe0 and +0xe4).
const WOBBLE: u32 = 0x72;
const SPIN: u32 = 0x74;
const SPIN_76: u32 = 0x76;
const SPIN_SHOTS: u32 = 0x78;
const FLASHES_WITH: [u32; 2] = [0xe0, 0xe4];
const SPINNER_LIFE: u32 = 20000;
const SPINNER_POINTS: u32 = 10000;
const SPINNER_RATE: u32 = 10;
/// Step 16: it spins up (by 16 a frame) to 0x800, wobbling up to 0x40 past 0x5ff; at full spin
/// a shot every 20 frames less by difficulty (up to 5), 29 dots up its model, swaying a 16th of
/// a turn either way about its screen angle; 300 frames, then 17: it spins down (by 16) and
/// stops wobbling, and once below 0x400 eases its turn y to the next whole turn (a 16th of the
/// way, at most 0x400 a frame), then 15.
const SPIN_UP: u32 = 16;
const SPIN_FULL: u16 = 0x800;
const WOBBLE_FROM: i16 = 0x5ff;
const WOBBLE_MOST: u32 = 0x40;
const SPIN_SHOT_EVERY: u32 = 20;
const SPIN_SHOT_Y: u32 = 0xffe3_0000;
const SPIN_SWAY: u32 = 0x1000;
const SPIN_FRAMES: i32 = 299;
const SPIN_SLOW: i16 = 0x3ff;
const SPIN_SETTLE: u32 = 0x400;

/// The spinner's behaviour (part 10 of the boss): 20000 life, shielded until its parent fights
/// (10), then 15; 16-17 as above. 100: shot down (10000 points, 10 to the rate); 101: a burst.
/// Each frame it flashes with its parent's two parts and turns by its spin.
pub fn spinner_behaviour(cpu: &mut Cpu, o: u32) {
    let parent = nested!(cpu, 0x0605_2970, models::follow_parent(cpu, o));
    let boss = cpu.read32(o + BOSS);
    part_shot_or_parent_gone(cpu, o, boss);
    match cpu.read8(o + STEP) {
        0 => {
            cpu.write32(o + LIFE, SPINNER_LIFE);
            cpu.write16(o + WOBBLE, 0);
            cpu.write16(o + SPIN, 0);
            cpu.write16(o + SPIN_76, 4);
            cpu.write16(o + STEP_WORD, 10);
        }
        10 => {
            if cpu.read8(parent + STEP) as i8 > PARENT_FIGHTING {
                let flags = cpu.read32(o + FLAGS) & !UNHURT;
                cpu.write32(o + FLAGS, flags);
                cpu.write16(o + STEP_WORD, FIGHTING);
            }
        }
        16 => {
            let spin = cpu.read16(o + SPIN) as i16 as i32 as u32;
            let spin = nested!(cpu, 0x0604_6168, actors::approach(cpu, spin, SPIN_FULL as u32, SPIN_UP));
            cpu.write16(o + SPIN, spin as u16);
            if spin as i16 > WOBBLE_FROM {
                let w = cpu.read16(o + WOBBLE) as i16 as i32 as u32;
                let w = nested!(cpu, 0x0604_6168, actors::approach(cpu, w, WOBBLE_MOST, 1));
                cpu.write16(o + WOBBLE, w as u16);
            }
            if cpu.read16(o + SPIN) == SPIN_FULL {
                let less = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 5));
                let n = cpu.read32(o + SPIN_SHOTS);
                if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, n, SPIN_SHOT_EVERY.wrapping_sub(less))) == 0 {
                    let t = cpu.stack();
                    cpu.write32(t + AS_FLAGS, 0);
                    cpu.write32(t + AS_SPEED, SHOT_SPEED);
                    cpu.write32(t + AS_SHOOTER, o);
                    cpu.write32(t + T_X, 0);
                    cpu.write32(t + T_Y, SPIN_SHOT_Y);
                    cpu.write32(t + T_8, 0);
                    nested!(cpu, 0x0605_2ca0, models::model_screen_point(cpu, o, t));
                    let a = nested!(cpu, 0x0608_054c, model_screen_angle(cpu, o));
                    let n = cpu.read32(o + SPIN_SHOTS);
                    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, (n << 8) as i16 as i32 as u32));
                    let m = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, SPIN_SWAY, s));
                    cpu.write16(t + AS_ANGLE, m.wrapping_add(a) as u16);
                    nested!(cpu, 0x0604_dbc4, bullets::aimed_set(cpu, t));
                }
                let n = cpu.read32(o + SPIN_SHOTS).wrapping_add(1);
                cpu.write32(o + SPIN_SHOTS, n);
                if n as i32 > SPIN_FRAMES {
                    cpu.write16(o + STEP_WORD, 17);
                }
            }
        }
        17 => {
            let w = cpu.read16(o + WOBBLE) as i16 as i32 as u32;
            let w = nested!(cpu, 0x0604_6168, actors::approach(cpu, w, 0, 1));
            cpu.write16(o + WOBBLE, w as u16);
            let spin = cpu.read16(o + SPIN) as i16 as i32 as u32;
            let spin = nested!(cpu, 0x0604_6168, actors::approach(cpu, spin, 0, SPIN_UP));
            cpu.write16(o + SPIN, spin as u16);
            if spin as i16 <= SPIN_SLOW {
                let turn = cpu.read16(o + TURN_Y) as i16 as i32 as u32;
                let ahead = nested!(cpu, 0x0604_61cc, camera::angle_ahead(cpu, turn, 0)) as i32 / 16;
                let spin = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, ahead as u32, SPIN_SETTLE));
                cpu.write16(o + SPIN, spin as u16);
                if spin as i16 == 0 {
                    cpu.write16(o + STEP_WORD, FIGHTING);
                }
            }
        }
        100 => part_shot_down(cpu, o, parent, SPINNER_POINTS, SPINNER_RATE),
        101 => part_gone(cpu, o, 0x1_0000),
        _ => {}
    }
    for at in FLASHES_WITH {
        let with = cpu.read32(parent + at);
        nested!(cpu, 0x0604_668c, parts::flash_against(cpu, o, with));
    }
    let turn = cpu.read16(o + TURN_Y).wrapping_add(cpu.read16(o + SPIN));
    cpu.write16(o + TURN_Y, turn);
}

// ---- the boss's wings --------------------------------------------------------------------------

/// The hit flags: hit (this frame) by a shot that harms.
const HIT_HARMED: u16 = 0x200;
/// A wing's life (a million: what it takes it passes on to the boss, with who hit it), the
/// boss's count (a word, +0x72) it flaps by.
const WING_LIFE: u32 = 0x10_0000;
const BOSS_FLAP: u32 = 0x72;
/// The second wing (slot 11) is mirrored.
const MIRRORED_WING: u8 = 11;
const FLIP_X: u32 = 0x10;
/// Its turn z: an eighth of a turn, less an eighth of a turn by the cosine of the boss's count
/// (once in 128 of it); the mirrored wing's negated.
const WING_TURN: u16 = 0x2000;
const WING_FLAP: u32 = 0xffff_e000;

/// A wing's behaviour (parts 11 and 12 of the boss): ended with the boss. Until done for, it
/// flashes with the boss, passes the harm it takes to the boss's life, and is done for once the
/// boss is. 0: (the second mirrored) 10, shielded until its parent fights, then 15. 100: its
/// slot freed (no points); 101: a burst. Each frame it flaps.
pub fn wing_behaviour(cpu: &mut Cpu, o: u32) {
    let parent = nested!(cpu, 0x0605_2970, models::follow_parent(cpu, o));
    let boss = cpu.read32(o + BOSS);
    if cpu.read8(boss + OBJ_MODE) as i8 > ENDING {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    if cpu.read8(o + STEP) as i8 <= DONE_FOR {
        nested!(cpu, 0x0604_666c, parts::flash_with(cpu, o, boss));
        if cpu.read16(o + HIT_FLAGS) & HIT_HARMED != 0 {
            let boss = cpu.read32(o + BOSS);
            let life = cpu.read32(boss + LIFE).wrapping_sub(WING_LIFE).wrapping_add(cpu.read32(o + LIFE));
            cpu.write32(boss + LIFE, life);
            let by = cpu.read32(o + HIT_BY);
            cpu.write32(boss + HIT_BY, by);
            cpu.write32(o + LIFE, WING_LIFE);
        }
        let boss = cpu.read32(o + BOSS);
        if cpu.read8(boss + STEP) as i8 > DONE_FOR {
            let flags = cpu.read32(o + FLAGS) | NO_HITS;
            cpu.write32(o + FLAGS, flags);
            cpu.write16(o + STEP_WORD, SHOT_DOWN);
        }
    }
    match cpu.read8(o + STEP) {
        0 => {
            if cpu.read8(o + SLOT) == MIRRORED_WING {
                let flags = cpu.read32(o + FLAGS) | FLIP_X;
                cpu.write32(o + FLAGS, flags);
            }
            cpu.write16(o + STEP_WORD, 10);
        }
        10 => {
            if cpu.read8(parent + STEP) as i8 > PARENT_FIGHTING {
                let flags = cpu.read32(o + FLAGS) & !UNHURT;
                cpu.write32(o + FLAGS, flags);
                cpu.write16(o + STEP_WORD, FIGHTING);
            }
        }
        100 => {
            let slot = cpu.read8(o + SLOT) as i8 as i32 as u32;
            cpu.write32(parent.wrapping_add(slot << 2) + PART_SLOTS, 0);
            cpu.write16(o + STEP_WORD, GONE);
        }
        101 => part_gone(cpu, o, 0x1_0000),
        _ => {}
    }
    cpu.write16(o + TURN_Z, WING_TURN);
    let boss = cpu.read32(o + BOSS);
    let a = ((cpu.read16(boss + BOSS_FLAP) as u32) << 9) as i16 as i32 as u32;
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    let m = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, WING_FLAP, c)) as u16;
    let turn = m.wrapping_add(cpu.read16(o + TURN_Z));
    cpu.write16(o + TURN_Z, turn);
    if cpu.read32(o + FLAGS) & FLIP_X != 0 {
        cpu.write16(o + TURN_Z, turn.wrapping_neg());
    }
}

// ---- the boss's hatch --------------------------------------------------------------------------

/// The hatch's count (+0x70), its own three parts (slots +0xb4, +0xb8, +0xbc), made from the
/// table at 0x060a2e08 (models.rs's model_parts_set).
const HATCH_COUNT: u32 = 0x70;
const HATCH_SLOTS: [u32; 3] = [0xb4, 0xb8, 0xbc];
const HATCH_PARTS: u32 = 0x060a_2e08;
const HATCH_LIFE: u32 = 20000;
const HATCH_POINTS: u32 = 10000;
const HATCH_RATE: u32 = 10;
/// Step 16 waits 120 frames once its parts are all gone.
const HATCH_WAIT: i32 = 119;

fn hatch_empty(cpu: &mut Cpu, o: u32) -> bool {
    HATCH_SLOTS.into_iter().all(|at| cpu.read32(o + at) == 0)
}

/// The hatch's behaviour (part 13 of the boss): 0: its parts made (each at its step 1), 20000
/// life, 10: shielded until its parent fights, then 15: counting while it has parts; with none
/// left, 17: they are made again, 15. 16: with none left, 120 frames, then 17. 100: shot down
/// (10000 points, 10 to the rate); 101: a burst.
pub fn hatch_behaviour(cpu: &mut Cpu, o: u32) {
    let parent = nested!(cpu, 0x0605_2970, models::follow_parent(cpu, o));
    let boss = cpu.read32(o + BOSS);
    part_shot_or_parent_gone(cpu, o, boss);
    match cpu.read8(o + STEP) {
        0 => {
            if nested!(cpu, 0x0605_332c, models::model_parts_set(cpu, o, HATCH_PARTS)) != 0 {
                for at in HATCH_SLOTS.into_iter().rev() {
                    let p = cpu.read32(o + at);
                    cpu.write8(p + STEP, 1);
                }
                cpu.write32(o + LIFE, HATCH_LIFE);
                cpu.write16(o + STEP_WORD, 10);
            }
        }
        10 => {
            if cpu.read8(parent + STEP) as i8 > PARENT_FIGHTING {
                let flags = cpu.read32(o + FLAGS) & !UNHURT;
                cpu.write32(o + FLAGS, flags);
                cpu.write32(o + HATCH_COUNT, 0);
                cpu.write16(o + STEP_WORD, FIGHTING);
            }
        }
        15 => {
            if hatch_empty(cpu, o) {
                cpu.write16(o + STEP_WORD, 17);
            } else {
                let n = cpu.read32(o + HATCH_COUNT).wrapping_add(1);
                cpu.write32(o + HATCH_COUNT, n);
            }
        }
        16 => {
            if hatch_empty(cpu, o) {
                let n = cpu.read32(o + HATCH_COUNT).wrapping_add(1);
                cpu.write32(o + HATCH_COUNT, n);
                if n as i32 > HATCH_WAIT {
                    cpu.write16(o + STEP_WORD, 17);
                }
            }
        }
        17 => {
            if nested!(cpu, 0x0605_332c, models::model_parts_set(cpu, o, HATCH_PARTS)) != 0 {
                cpu.write32(o + HATCH_COUNT, 0);
                cpu.write16(o + STEP_WORD, FIGHTING);
            }
        }
        100 => part_shot_down(cpu, o, parent, HATCH_POINTS, HATCH_RATE),
        101 => part_gone(cpu, o, 0x1_0000),
        _ => {}
    }
}

// ---- the hatch's pods --------------------------------------------------------------------------

/// A hatch pod's flag set (by others, +0x70: it may launch), its count before launching (+0x74),
/// its sway (a word counting frames, +0x7a); its models (+0x44).
const HP_LAUNCH: u32 = 0x70;
const HP_WAIT: u32 = 0x74;
const HP_SWAY: u32 = 0x7a;
const MODELS: u32 = 0x44;
const HP_LIFE: u32 = 1000;
const HP_POINTS: u32 = 1000;
/// Slots 1 and 2 are turned (an eighth, a quarter of a turn back).
const HP_TURNS: [(u8, u16); 2] = [(1, 0xe000), (2, 0xc000)];
/// It grows in (0x400 a frame) from nothing.
const HP_GROW: u32 = 0x400;
/// Allowed to, it launches each time its sway reaches 64 of 128: 30 frames, then it leaves its
/// hatch as a free pod (an actor of 0xb4 bytes with kind 60's model 10, its behaviour
/// 0x060832d8 not seen run) at its place, its life with it.
const LAUNCH_AT: u16 = 0x40;
const LAUNCH_WAIT: u32 = 30;
const FREE_POD_SIZE: u32 = 0xb4;
const FREE_POD_FLAGS: u32 = 0x80_0400;
const FREE_POD_KIND: u32 = 60;
const FREE_POD_MODEL: u32 = 0x28;
const FREE_POD: u32 = 0x0608_32d8;
/// What a launched piece came from (+0xb0).
const OWNER: u32 = 0xb0;
/// Its turn x sways: an eighth of a turn back, less an eighth by the cosine of its count.
const HP_TILT: u16 = 0xe000;
const HP_SWAY_SIZE: u32 = 0xffff_e000;

/// A hatch pod's behaviour (the hatch's parts): 0-1: 1000 life, turned by its slot (0: made
/// unscaled, 2: it grows in), 10: shielded until its parent fights, then 15: it sways, and may
/// launch (17). 100: shot down (1000 points), its slot freed; 101: a small burst.
pub fn hatch_pod_behaviour(cpu: &mut Cpu, o: u32) {
    let parent = nested!(cpu, 0x0605_2970, models::follow_parent(cpu, o));
    let boss = cpu.read32(o + BOSS);
    part_shot_or_parent_gone(cpu, o, boss);
    match cpu.read8(o + STEP) {
        step @ (0 | 1) => {
            cpu.write32(o + LIFE, HP_LIFE);
            let slot = cpu.read8(o + SLOT);
            if let Some(&(_, turn)) = HP_TURNS.iter().find(|(s, _)| *s == slot) {
                cpu.write16(o + TURN_Z, turn);
            }
            cpu.write16(o + HP_SWAY, 0);
            if step == 0 {
                for k in [8, 4, 0] {
                    cpu.write32(o + SCALES + k, 0);
                }
                cpu.write16(o + STEP_WORD, 2);
            } else {
                cpu.write16(o + STEP_WORD, 10);
            }
        }
        2 => {
            let s = cpu.read32(o + SCALES);
            let s = nested!(cpu, 0x0604_6168, actors::approach(cpu, s, 0x1_0000, HP_GROW));
            cpu.write32(o + SCALES, s);
            cpu.write32(o + SCALES + 8, s);
            cpu.write32(o + SCALES + 4, s);
            if cpu.read32(o + SCALES) == 0x1_0000 {
                cpu.write16(o + STEP_WORD, 10);
            }
        }
        10 => {
            let sway = cpu.read16(o + HP_SWAY).wrapping_add(1);
            cpu.write16(o + HP_SWAY, sway);
            if cpu.read8(parent + STEP) as i8 > PARENT_FIGHTING {
                let flags = cpu.read32(o + FLAGS) & !UNHURT;
                cpu.write32(o + FLAGS, flags);
                cpu.write32(o + HP_LAUNCH, 0);
                cpu.write16(o + STEP_WORD, FIGHTING);
            }
        }
        15 => {
            let sway = cpu.read16(o + HP_SWAY).wrapping_add(1);
            cpu.write16(o + HP_SWAY, sway);
            if cpu.read32(o + HP_LAUNCH) != 0 && sway & 0x7f == LAUNCH_AT {
                cpu.write32(o + HP_WAIT, LAUNCH_WAIT);
                cpu.write16(o + STEP_WORD, 17);
            }
        }
        17 => {
            let wait = cpu.read32(o + HP_WAIT);
            if wait as i32 > 0 {
                cpu.write32(o + HP_WAIT, wait.wrapping_sub(1));
            } else {
                free_pod(cpu, o);
                let boss = cpu.read32(o + BOSS);
                let slot = cpu.read8(o + SLOT) as i8 as i32 as u32;
                cpu.write32(boss.wrapping_add(slot << 2) + PART_SLOTS, 0);
                cpu.write8(o + OBJ_MODE, TO_STOP);
            }
        }
        100 => {
            if cpu.read32(o + LIFE) as i32 <= 0 {
                let by = cpu.read32(o + HIT_BY);
                let (player, weapon) = (cpu.read8(by + HIT_PLAYER) as i8 as i32 as u32, cpu.read8(by + HIT_WEAPON) as i8 as i32 as u32);
                nested!(cpu, 0x0604_3108, hits::score_kill(cpu, player, weapon, o, HP_POINTS));
            }
            let boss = cpu.read32(o + BOSS);
            if boss != 0 {
                let slot = cpu.read8(o + SLOT) as i8 as i32 as u32;
                cpu.write32(boss.wrapping_add(slot << 2) + PART_SLOTS, 0);
            }
            cpu.write16(o + STEP_WORD, GONE);
        }
        101 => part_gone(cpu, o, 0x4000),
        _ => {}
    }
    let a = ((cpu.read16(o + HP_SWAY) as u32) << 9) as i16 as i32 as u32;
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    let m = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, HP_SWAY_SIZE, c)) as u16;
    cpu.write16(o + TURN_X, m.wrapping_add(HP_TILT));
}

/// The free pod a hatch pod leaves as.
fn free_pod(cpu: &mut Cpu, o: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, FREE_POD_SIZE)) as i8 as i32 as u32;
    let p = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if p == 0 {
        return;
    }
    nested!(cpu, 0x0604_62ac, actors::actor_setup(cpu, p));
    let flags = cpu.read32(p + FLAGS) | FREE_POD_FLAGS;
    cpu.write32(p + FLAGS, flags);
    let file = cpu.read32(kind(FREE_POD_KIND));
    let list = cpu.read32(file + F_MODELS);
    let models = cpu.read32(list + FREE_POD_MODEL);
    cpu.write32(p + MODELS, models);
    cpu.write8(p + BYTE_33, MISSILE_33);
    for k in [8, 4, 0] {
        cpu.write32(p + POSITION + k, 0);
    }
    nested!(cpu, 0x0605_2c44, models::model_point(cpu, o, p + POSITION));
    let a = nested!(cpu, 0x0608_054c, model_screen_angle(cpu, o));
    cpu.write16(p + ANGLE, a as u16);
    cpu.write32(p + BEHAVIOUR, FREE_POD);
    let n = cpu.read8(o + NUMBER);
    cpu.write8(p + NUMBER, n);
    let side = cpu.read8(o + SIDE);
    cpu.write8(p + SIDE, side);
    let w = cpu.read8(o + HIT_WEAPON);
    cpu.write8(p + HIT_WEAPON, w);
    let life = cpu.read32(o + LIFE);
    cpu.write32(p + LIFE, life);
    cpu.write32(p + OWNER, o);
}

// ---- the outline's lines -----------------------------------------------------------------------

/// A 16.16 value in whole dots, toward zero (a word).
fn dots(v: u32) -> u16 {
    let v = v as i32;
    ((if v < 0 { v + 0xffff } else { v }) >> 16) as u16
}

/// An outline line (a plain actor drawn as a polygon line of its own corners): its step (a byte,
/// +0x3c), its index in the outline (a word, +0x40), its place from the line before it (16.16,
/// +0x44 and +0x48: in that line's turn), the object it outlines (+0x5c), the lines before and
/// after it (+0x60, +0x64), its run (words, +0x68 and +0x6a: from its point to the next), how much
/// of it is drawn (16.16, +0x6c), its corners (words, +0x70: its start, +0x74: its end).
const OL_STEP: u32 = 0x3c;
const OL_INDEX: u32 = 0x40;
const OL_FROM: u32 = 0x44;
const OL_OWNER: u32 = 0x5c;
const OL_BEFORE: u32 = 0x60;
const OL_AFTER: u32 = 0x64;
const OL_RUN: u32 = 0x68;
const OL_DRAWN: u32 = 0x6c;
const OL_START: u32 = 0x70;
const OL_END: u32 = 0x74;
/// The outlines' points (by the owner's outline number, its word +0x74): each a word, then the
/// points (x, y words).
const OUTLINES: u32 = 0x060a_2fec;
const OWNER_OUTLINE: u32 = 0x74;
/// The flags: hidden (until the line before it is drawn), a polygon line, 0x2000_0000.
const HIDDEN: u32 = 0x8000_0000;
const OL_FLAGS: u32 = 0xa000_0003;
const OL_COLOUR: u16 = 11;
const DRAW_SPEED: u32 = 0x10_0000;
/// Its steps: 0 set up, 1 waiting for the line before, 15 drawing, 99 drawn, 100 being erased
/// (its start on toward its end; the line after then erased too), 120 erased.
const OL_WAITING: u8 = 1;
const OL_DRAWING: u8 = 15;
const OL_DRAWN_STEP: u8 = 99;
const OL_ERASING: u8 = 100;
const OL_ERASED: u8 = 120;

/// How far along its run (`drawn` of it, from its start): the point (whole dots), and the run's
/// length. The turned point is on the stack at `t`.
fn along_run(cpu: &mut Cpu, o: u32, t: u32) -> (u16, u16, u32) {
    let x = (cpu.read16(o + OL_RUN) as i16 as i32 as u32) << 16;
    cpu.write32(t + T_X, x);
    let y = (cpu.read16(o + OL_RUN + 2) as i16 as i32 as u32) << 16;
    cpu.write32(t + T_Y, y);
    let length = nested!(cpu, 0x0604_5e60, aim::length(cpu, x, y));
    cpu.write32(t + T_8, length);
    let (x, y) = (cpu.read32(t + T_X), cpu.read32(t + T_Y));
    let a = nested!(cpu, 0x0606_780c, sgl::sl_atan(cpu, x, y)) as u16;
    cpu.write16(t + T_ANGLE, a.wrapping_add(0x4000));
    cpu.write32(t + T_X, 0);
    let drawn = cpu.read32(o + OL_DRAWN).wrapping_neg();
    cpu.write32(t + T_Y, drawn);
    cpu.write32(t + T_SCALES + 4, 0x1_0000);
    cpu.write32(t + T_SCALES, 0x1_0000);
    cpu.write32(t + T_FLAGS, 0);
    nested!(cpu, 0x0606_4eac, math::turn_point(cpu, t));
    (dots(cpu.read32(t + T_X)), dots(cpu.read32(t + T_Y)), cpu.read32(t + T_8))
}

/// An outline line's behaviour: ended once its owner ends, is at step 15 or is done for. 0: its
/// run and place from the outline's points; 1: shown once the line before it is drawn; 15: it
/// draws out 16 dots a frame; 100: it is erased from its start the same way, then the line after
/// it. Each frame it is placed at the end of the line before (the first at its owner).
pub fn outline_line_behaviour(cpu: &mut Cpu, o: u32) {
    let t = cpu.stack();
    let owner = cpu.read32(o + OL_OWNER);
    cpu.write32(t + 0x40, owner);
    let owner_step = cpu.read8(owner + STEP) as i8;
    if cpu.read8(owner + OBJ_MODE) as i8 > ENDING || owner_step == FIGHTING as i8 || owner_step > DONE_FOR {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    let n = cpu.read16(owner + OWNER_OUTLINE) as i16 as i32 as u32;
    let points = cpu.read32(OUTLINES.wrapping_add(n << 2)).wrapping_add(2);
    let step = cpu.read8(o + OL_STEP);
    let mut next = None;
    match step {
        0 => {
            cpu.write32(o + CORNERS_AT, o + OL_START);
            let flags = cpu.read32(o + FLAGS) | OL_FLAGS;
            cpu.write32(o + FLAGS, flags);
            for k in [2, 0, 6, 4] {
                cpu.write16(o + OL_START + k, 0);
            }
            let k = cpu.read16(o + OL_INDEX) as i16 as i32 as u32;
            let p = points.wrapping_add(k << 2);
            let dx = cpu.read16(p + 4).wrapping_sub(cpu.read16(p));
            cpu.write16(o + OL_RUN, dx);
            let dy = cpu.read16(p + 6).wrapping_sub(cpu.read16(p + 2));
            cpu.write16(o + OL_RUN + 2, dy);
            cpu.write32(o + OL_DRAWN, 0);
            if k == 0 {
                cpu.write32(o + OL_FROM + 4, 0);
                cpu.write32(o + OL_FROM, 0);
            } else {
                let dx = (cpu.read16(p) as i16 as i32).wrapping_sub(cpu.read16(p.wrapping_sub(4)) as i16 as i32);
                cpu.write32(o + OL_FROM, (dx as u32) << 16);
                let dy = (cpu.read16(p + 2) as i16 as i32).wrapping_sub(cpu.read16(p.wrapping_sub(2)) as i16 as i32);
                cpu.write32(o + OL_FROM + 4, (dy as u32) << 16);
            }
            let colour = cpu.read16(kind(EFFECT_KIND) + K_COLOUR).wrapping_add(OL_COLOUR);
            cpu.write16(o + OWN_COLOUR, colour);
            cpu.write16(o + COLOUR, colour);
            next = Some(OL_WAITING);
        }
        OL_WAITING => {
            let before = cpu.read32(o + OL_BEFORE);
            if before == 0 || cpu.read8(before + OL_STEP) as i8 > 98 {
                let flags = cpu.read32(o + FLAGS) & !HIDDEN;
                cpu.write32(o + FLAGS, flags);
                next = Some(OL_DRAWING);
            }
        }
        OL_DRAWING => {
            let drawn = cpu.read32(o + OL_DRAWN).wrapping_add(DRAW_SPEED);
            cpu.write32(o + OL_DRAWN, drawn);
            cpu.write32(t + 0x44, o + OL_RUN + 2);
            let (x, y, length) = along_run(cpu, o, t);
            cpu.write16(o + OL_END, x);
            cpu.write16(o + OL_END + 2, y);
            if cpu.read32(o + OL_DRAWN) as i32 >= length as i32 {
                let run = cpu.read16(o + OL_RUN);
                cpu.write16(o + OL_END, run);
                let run = cpu.read16(o + OL_RUN + 2);
                cpu.write16(o + OL_END + 2, run);
                cpu.write32(o + OL_DRAWN, 0);
                next = Some(OL_DRAWN_STEP);
            }
        }
        OL_ERASING => {
            let drawn = cpu.read32(o + OL_DRAWN).wrapping_add(DRAW_SPEED);
            cpu.write32(o + OL_DRAWN, drawn);
            let (x, y, length) = along_run(cpu, o, t);
            cpu.write16(o + OL_START, x);
            cpu.write16(o + OL_START + 2, y);
            if cpu.read32(o + OL_DRAWN) as i32 >= length as i32 {
                let after = cpu.read32(o + OL_AFTER);
                if after != 0 {
                    cpu.write8(after + OL_STEP, OL_ERASING);
                }
                let flags = cpu.read32(o + FLAGS) | HIDDEN;
                cpu.write32(o + FLAGS, flags);
                next = Some(OL_ERASED);
            }
        }
        _ => {}
    }
    if let Some(s) = next {
        cpu.write8(o + OL_STEP, s);
    }
    if cpu.read16(o + OL_INDEX) == 0 {
        let owner = cpu.read32(t + 0x40);
        for k in [0, 4, 8] {
            let v = cpu.read32(owner + POSITION + k);
            cpu.write32(o + POSITION + k, v);
        }
        cpu.write16(o + ANGLE, 0);
    } else {
        let before = cpu.read32(o + OL_BEFORE);
        let (x, y) = (cpu.read32(o + OL_FROM), cpu.read32(o + OL_FROM + 4));
        cpu.write32(t + T_X, x);
        cpu.write32(t + T_Y, y);
        cpu.write32(t + T_SCALES + 4, 0x1_0000);
        cpu.write32(t + T_SCALES, 0x1_0000);
        cpu.write32(t + T_FLAGS, 0);
        let a = cpu.read16(before + ANGLE);
        cpu.write16(t + T_ANGLE, a);
        nested!(cpu, 0x0606_4eac, math::turn_point(cpu, t));
        let x = cpu.read32(before + POSITION).wrapping_add(cpu.read32(t + T_X));
        cpu.write32(o + POSITION, x);
        let y = cpu.read32(before + POSITION + 4).wrapping_add(cpu.read32(t + T_Y));
        cpu.write32(o + POSITION + 4, y);
        let z = cpu.read32(before + POSITION + 8);
        cpu.write32(o + POSITION + 8, z);
        let a = cpu.read16(before + ANGLE);
        cpu.write16(o + ANGLE, a);
    }
}

// ---- the outline's spark -----------------------------------------------------------------------

/// The spark (an actor drawn as a dart of its own corners, +0x7c) runs along the outline's
/// lines: the line it is on (+0xb0), its speed (+0x74), how far along the line (+0x78); two longs
/// set to -1 each frame (+0x48, +0x4c).
const SP_LINE: u32 = 0xb0;
const SP_SPEED: u32 = 0x74;
const SP_ALONG: u32 = 0x78;
const SP_CORNERS: u32 = 0x7c;
const SP_48: u32 = 0x48;
const SPARK_SHAPE: [(u32, u16); 8] = [(0, 0xfffd), (4, 3), (6, 0xfffb), (2, 0xfffb), (8, 5), (0xc, 0xfffb), (0xe, 5), (0xa, 5)];
const SPARK_ANIMATION: u32 = 0x060a_3004;
/// It speeds up to 6 dots a frame (2 more or less by difficulty), by 1/16 a frame.
const SPARK_SPEED: u32 = 0x6_0000;
const SPARK_SPEED_BY_DIFFICULTY: u32 = 0x2_0000;
const SPARK_ACCEL: u32 = 0x1000;
/// Its trail: every fourth frame its shape again (polygon_set's: flags 50, scaled 3/4, 20 frames).
const SPARK_TRAIL_SCALE: u32 = 0xc000;
const SPARK_TRAIL_FLAGS: u32 = 50;
const SPARK_TRAIL_FRAMES: u16 = 20;

/// The spark's behaviour: ended with its line, or past the last. 0: its shape, 15. 15: along its
/// line's drawn part (from its start to its end corner), on to the line after at its end. Its
/// colour flickers; a trail every fourth frame.
pub fn outline_spark_behaviour(cpu: &mut Cpu, o: u32) {
    let line = cpu.read32(o + SP_LINE);
    if cpu.read8(line + OBJ_MODE) as i8 > ENDING {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    let t = cpu.stack();
    match cpu.read8(o + STEP) {
        0 => {
            cpu.write32(o + CORNERS_AT, o + SP_CORNERS);
            let flags = cpu.read32(o + FLAGS) | 1;
            cpu.write32(o + FLAGS, flags);
            for (k, v) in SPARK_SHAPE {
                cpu.write16(o + SP_CORNERS + k, v);
            }
            let colour = cpu.read16(kind(EFFECT_KIND) + K_COLOUR).wrapping_add(6);
            cpu.write16(o + OWN_COLOUR, colour);
            cpu.write16(o + COLOUR, colour);
            cpu.write32(o + M_ANIMATION, SPARK_ANIMATION);
            cpu.write32(o + SP_SPEED, 0);
            cpu.write32(o + SP_ALONG, 0);
            cpu.write16(o + STEP_WORD, FIGHTING);
        }
        15 => {
            cpu.write32(o + SP_48, u32::MAX);
            cpu.write32(o + SP_48 + 4, u32::MAX);
            let x = (cpu.read16(line + OL_END) as i16 as i32 as u32) << 16;
            cpu.write32(t + T_X, x);
            let y = (cpu.read16(line + OL_END + 2) as i16 as i32 as u32) << 16;
            cpu.write32(t + T_Y, y);
            let length = nested!(cpu, 0x0604_5e60, aim::length(cpu, x, y));
            cpu.write32(t + T_8, length);
            let (x, y) = (cpu.read32(t + T_X), cpu.read32(t + T_Y));
            let a = (nested!(cpu, 0x0606_780c, sgl::sl_atan(cpu, x, y)) as u16).wrapping_add(0x4000);
            cpu.write16(o + ANGLE, a);
            let a = a.wrapping_add(cpu.read16(line + ANGLE));
            cpu.write16(o + ANGLE, a);
            cpu.write32(t + T_X, 0);
            let along = cpu.read32(o + SP_ALONG).wrapping_neg();
            cpu.write32(t + T_Y, along);
            cpu.write32(t + T_SCALES + 4, 0x1_0000);
            cpu.write32(t + T_SCALES, 0x1_0000);
            cpu.write32(t + T_FLAGS, 0);
            cpu.write16(t + T_ANGLE, a);
            nested!(cpu, 0x0606_4eac, math::turn_point(cpu, t));
            let x = cpu.read32(line + POSITION).wrapping_add(cpu.read32(t + T_X));
            cpu.write32(o + POSITION, x);
            let y = cpu.read32(line + POSITION + 4).wrapping_add(cpu.read32(t + T_Y));
            cpu.write32(o + POSITION + 4, y);
            let z = cpu.read32(line + POSITION + 8);
            cpu.write32(o + POSITION + 8, z);
            let to = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, SPARK_SPEED_BY_DIFFICULTY)).wrapping_add(SPARK_SPEED);
            let speed = cpu.read32(o + SP_SPEED);
            let speed = nested!(cpu, 0x0604_6168, actors::approach(cpu, speed, to, SPARK_ACCEL));
            cpu.write32(o + SP_SPEED, speed);
            let along = cpu.read32(o + SP_ALONG).wrapping_add(speed);
            cpu.write32(o + SP_ALONG, along);
            let length = cpu.read32(t + T_8);
            if along as i32 >= length as i32 {
                cpu.write32(o + SP_ALONG, along.wrapping_sub(length));
                let after = cpu.read32(line + OL_AFTER);
                cpu.write32(o + SP_LINE, after);
                if after == 0 {
                    cpu.write8(o + OBJ_MODE, TO_STOP);
                }
            }
        }
        _ => {}
    }
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 5)) as u16;
    let colour = cpu.read16(kind(EFFECT_KIND) + K_COLOUR).wrapping_add(r.wrapping_add(5));
    cpu.write16(o + OWN_COLOUR, colour);
    cpu.write16(o + COLOUR, colour);
    let frames = cpu.read32(GAME_FRAMES);
    if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, frames, 4)) == 0 {
        let fx = t;
        for k in [0, 4, 8] {
            let v = cpu.read32(o + POSITION + k);
            cpu.write32(fx + effects::X + k, v);
        }
        cpu.write32(fx + effects::SCALE_Y, SPARK_TRAIL_SCALE);
        cpu.write32(fx + effects::SCALE_X, SPARK_TRAIL_SCALE);
        cpu.write32(fx + effects::FLAGS_LONG, SPARK_TRAIL_FLAGS);
        let a = cpu.read16(o + ANGLE);
        cpu.write16(fx + effects::ANGLE, a);
        cpu.write16(fx + effects::FRAMES, SPARK_TRAIL_FRAMES);
        cpu.write8(fx + effects::FRAMES_2, 0);
        let colour = cpu.read16(kind(EFFECT_KIND) + K_COLOUR);
        cpu.write16(fx + effects::COLOUR, colour);
        let n = cpu.read8(o + NUMBER);
        cpu.write8(fx + effects::PLAYER, n);
        cpu.write32(fx + effects::RELATIVE_3C, o);
        let p = nested!(cpu, 0x0604_e6d8, effects::polygon_set(cpu, fx));
        if p != 0 {
            for (k, v) in SPARK_SHAPE {
                cpu.write16(p + P_CORNERS + k, v);
            }
        }
    }
}

// ---- the tracers -------------------------------------------------------------------------------

/// A tracer (a boss part that traces an outline): the tracer it follows (+0x70; a leader has
/// itself or another), the outline it traces (a word, +0x74: OUTLINES's), its spin (a word: its
/// turn y on by it each frame, +0x76), a count (+0x78), its outline's first line and its spark
/// (+0x7c, +0x80), where it heads (16.16, +0x84 and +0x88: its place in its group eases there),
/// its heading (a word, +0x8c: its turn z), the ease (a word, +0x8e: a 256th of it divides).
const TR_LEADER: u32 = 0x70;
const TR_OUTLINE: u32 = 0x74;
const TR_SPIN: u32 = 0x76;
const TR_COUNT: u32 = 0x78;
const TR_LINES: u32 = 0x7c;
const TR_SPARK: u32 = 0x80;
const TR_GOAL: u32 = 0x84;
const TR_HEADING: u32 = 0x8c;
const TR_EASE: u32 = 0x8e;
const TR_LIFE: u32 = 5000;
const TR_POINTS: u32 = 3000;
const TR_EASE_START: u16 = 0x800;
/// Hidden and unhit as it appears (an effect: kind 61's frame 46 with its animation, its scales
/// shrinking 0x444 a frame), for 50 frames.
const TR_HIDDEN: u32 = 0x8008_0000;
const TR_APPEAR_FRAME: u8 = 46;
const TR_APPEAR_ANIMATION: u32 = 0x060a_300c;
const TR_APPEAR_SHRINK: u32 = 0xffff_fbbc;
const TR_APPEAR_FRAMES: u32 = 50;
/// The leader (slot 0) picks one of five outlines; the others follow it (17).
const TR_OUTLINES: u32 = 5;
/// Tracing: spinning up (by 16 a frame) to 0x400, heading (at most 0x400 a frame, a quarter
/// turn behind its run) to the outline's first point, turning (at most 0x200) to face the
/// second, then at 0x800 spin its lines made one after another; once all are drawn, its spark;
/// once that is gone, the lines erased; once all are, they end and it fights again (15).
const TR_SPIN_TRACE: u32 = 0x400;
const TR_SPIN_FULL: u16 = 0x800;
const TR_SPIN_STEP: u32 = 16;
const TR_HEAD: u32 = 0xc000;
const TR_FACE: u32 = 0x4000;
const TR_HEAD_MOST: u32 = 0x400;
const TR_FACE_MOST: u32 = 0x200;
const TR_THERE: i32 = 0xffff;
const OUTLINE_LINE: u32 = 0x0608_37dc;
const OUTLINE_SPARK: u32 = 0x0608_3b7c;
const LINE_SIZE: u32 = 0x80;
const SPARK_SIZE: u32 = 0xb4;
/// Its place in its group eases toward where it heads: by 16 (256 once tracing) of the ease a
/// frame, by 32 a frame.
const TR_EASE_FIGHTING: u32 = 0x1000;
const TR_EASE_TRACING: u32 = 0x100;
const TR_EASE_STEP: u32 = 32;

/// The outline's points (`points`: x, y words) and their count.
fn outline_points(cpu: &mut Cpu, n: u32) -> (u32, i32) {
    let table = cpu.read32(OUTLINES.wrapping_add(n << 2));
    (table + 2, cpu.read16(table) as i16 as i32)
}

/// Where it heads moved a 16th of the way to the point (`px`, `py`: whole dots), its heading
/// turned (at most 0x400) toward a quarter turn behind its run there; there (within a dot, by
/// the sum of the run's sizes), it heads there exactly and is at inner step 1.
fn head_for(cpu: &mut Cpu, o: u32, t: u32, px: u16, py: u16) {
    let dx = ((px as i16 as i32 as u32) << 16).wrapping_sub(cpu.read32(o + TR_GOAL));
    cpu.write32(t + T_X, dx);
    let dy = ((py as i16 as i32 as u32) << 16).wrapping_sub(cpu.read32(o + TR_GOAL + 4));
    cpu.write32(t + T_Y, dy);
    let gx = cpu.read32(o + TR_GOAL).wrapping_add((dx as i32 / 16) as u32);
    cpu.write32(o + TR_GOAL, gx);
    let gy = cpu.read32(o + TR_GOAL + 4).wrapping_add((dy as i32 / 16) as u32);
    cpu.write32(o + TR_GOAL + 4, gy);
    let a = nested!(cpu, 0x0606_780c, sgl::sl_atan(cpu, dx, dy)) as i16 as i32 as u32;
    let heading = cpu.read16(o + TR_HEADING) as i16 as i32 as u32;
    let d = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, heading, a.wrapping_add(TR_HEAD)));
    let c = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, d, TR_HEAD_MOST)) as u16;
    let h = c.wrapping_add(cpu.read16(o + TR_HEADING));
    cpu.write16(o + TR_HEADING, h);
    let size = (dx as i32).wrapping_abs().wrapping_add((dy as i32).wrapping_abs());
    if size <= TR_THERE {
        cpu.write32(o + TR_GOAL, (px as i16 as i32 as u32) << 16);
        cpu.write32(o + TR_GOAL + 4, (py as i16 as i32 as u32) << 16);
        cpu.write8(o + INNER, 1);
    }
}

/// Its heading turned (at most 0x200) toward a quarter turn on from its run to (`px`, `py`);
/// once facing it, set exactly, and at inner step 2.
fn face(cpu: &mut Cpu, o: u32, t: u32, px: u16, py: u16) {
    let dx = ((px as i16 as i32 as u32) << 16).wrapping_sub(cpu.read32(o + TR_GOAL));
    cpu.write32(t + T_X, dx);
    let dy = ((py as i16 as i32 as u32) << 16).wrapping_sub(cpu.read32(o + TR_GOAL + 4));
    cpu.write32(t + T_Y, dy);
    let c = turn_to_face(cpu, o, dx, dy);
    if c == 0 {
        let (x, y) = (cpu.read32(t + T_X), cpu.read32(t + T_Y));
        let a = nested!(cpu, 0x0606_780c, sgl::sl_atan(cpu, x, y)) as u16;
        cpu.write16(o + TR_HEADING, a.wrapping_add(TR_FACE as u16));
        cpu.write8(o + INNER, 2);
    }
}

/// The heading turned (at most 0x200) toward a quarter turn on from the run (`dx`, `dy`): the
/// turn.
fn turn_to_face(cpu: &mut Cpu, o: u32, dx: u32, dy: u32) -> u32 {
    let a = nested!(cpu, 0x0606_780c, sgl::sl_atan(cpu, dx, dy)) as i16 as i32 as u32;
    let heading = cpu.read16(o + TR_HEADING) as i16 as i32 as u32;
    let d = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, heading, a.wrapping_add(TR_FACE)));
    let c = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, d, TR_FACE_MOST));
    let h = cpu.read16(o + TR_HEADING).wrapping_add(c as u16);
    cpu.write16(o + TR_HEADING, h);
    c
}

/// Its spin moved toward `to`.
fn spin_toward(cpu: &mut Cpu, o: u32, to: u32) -> u16 {
    let spin = cpu.read16(o + TR_SPIN) as i16 as i32 as u32;
    let spin = nested!(cpu, 0x0604_6168, actors::approach(cpu, spin, to, TR_SPIN_STEP)) as u16;
    cpu.write16(o + TR_SPIN, spin);
    spin
}

/// The tracer's behaviour (part 14 of the boss, and parts of others): 0: 5000 life, hidden; 1:
/// it appears (50 frames); 10: shielded until its parent fights, then 15: the leader picks an
/// outline (16), the others follow (17). 16: it traces its outline (above). 17: it follows its
/// leader's outline back from its end, spinning as it does, until the leader fights again. 18:
/// 15. 100: shot down (3000 points), its slot freed; 101: a small burst. Each frame it spins,
/// and its place in its group eases toward where it heads, turned by its heading.
pub fn tracer_behaviour(cpu: &mut Cpu, o: u32) {
    let t = cpu.stack();
    let parent = nested!(cpu, 0x0605_2970, models::follow_parent(cpu, o));
    let leader = cpu.read32(o + TR_LEADER);
    cpu.write32(t + 0x8c, leader);
    if cpu.read8(o + STEP) as i8 <= DONE_FOR
        && ((cpu.read32(o + LIFE) as i32) <= 0
            || cpu.read8(parent + STEP) as i8 > DONE_FOR
            || { let l = cpu.read32(t + 0x8c); cpu.read8(l + STEP) as i8 > DONE_FOR })
    {
        let flags = cpu.read32(o + FLAGS) | NO_HITS;
        cpu.write32(o + FLAGS, flags);
        cpu.write16(o + STEP_WORD, SHOT_DOWN);
    }
    match cpu.read8(o + STEP) {
        0 => {
            cpu.write32(o + LIFE, TR_LIFE);
            cpu.write16(o + TR_SPIN, 0);
            let (x, y) = (cpu.read32(o + PLACE_IN_GROUP), cpu.read32(o + PLACE_IN_GROUP + 4));
            cpu.write32(o + TR_GOAL, x);
            cpu.write32(o + TR_GOAL + 4, y);
            let a = cpu.read16(o + ANGLE);
            cpu.write16(o + TR_HEADING, a);
            cpu.write16(o + TR_EASE, TR_EASE_START);
            let flags = cpu.read32(o + FLAGS) | TR_HIDDEN;
            cpu.write32(o + FLAGS, flags);
            cpu.write16(o + STEP_WORD, 1);
        }
        1 => match cpu.read8(o + INNER) {
            0 => {
                let fx = t;
                for k in [0, 4, 8] {
                    let v = cpu.read32(o + PLACE_IN_GROUP + k);
                    cpu.write32(fx + effects::X + k, v);
                }
                cpu.write32(fx + effects::SCALE_Y, 0x1_0000);
                cpu.write32(fx + effects::SCALE_X, 0x1_0000);
                cpu.write32(fx + effects::FLAGS_LONG, 3);
                cpu.write16(fx + effects::ANGLE, 0);
                cpu.write8(fx + effects::FRAMES_2, 0);
                cpu.write16(fx + effects::FRAMES, 0);
                cpu.write8(fx + effects::KIND, EFFECT_KIND as u8);
                cpu.write8(fx + effects::FRAME, TR_APPEAR_FRAME);
                let colour = cpu.read16(kind(EFFECT_KIND) + K_COLOUR);
                cpu.write16(fx + effects::COLOUR, colour);
                let n = cpu.read8(o + NUMBER);
                cpu.write8(fx + effects::PLAYER, n);
                cpu.write32(fx + effects::LINK, TR_APPEAR_ANIMATION);
                cpu.write32(fx + effects::RELATIVE_3C, o);
                let e = nested!(cpu, 0x0604_e1cc, effects::effect_set(cpu, fx));
                if e != 0 {
                    cpu.write32(e + E_SCALE_SPEED + 4, TR_APPEAR_SHRINK);
                    cpu.write32(e + E_SCALE_SPEED, TR_APPEAR_SHRINK);
                }
                cpu.write32(o + TR_COUNT, TR_APPEAR_FRAMES);
                cpu.write8(o + INNER, 1);
            }
            1 => {
                let n = cpu.read32(o + TR_COUNT).wrapping_sub(1);
                cpu.write32(o + TR_COUNT, n);
                if n as i32 <= 0 {
                    cpu.write8(o + INNER, 2);
                }
            }
            2 => {
                let flags = cpu.read32(o + FLAGS) & !TR_HIDDEN;
                cpu.write32(o + FLAGS, flags);
                cpu.write16(o + STEP_WORD, 10);
            }
            _ => {}
        },
        10 => {
            if cpu.read8(parent + STEP) as i8 > PARENT_FIGHTING {
                let flags = cpu.read32(o + FLAGS) & !UNHURT;
                cpu.write32(o + FLAGS, flags);
                cpu.write16(o + STEP_WORD, FIGHTING);
            }
        }
        15 => {
            if cpu.read8(o + SLOT) == 0 {
                let r = nested!(cpu, 0x0604_5d90, math::random(cpu, TR_OUTLINES)) as i32;
                cpu.write16(o + TR_OUTLINE, r.wrapping_abs() as u16);
                cpu.write16(o + STEP_WORD, 16);
            } else {
                cpu.write16(o + STEP_WORD, 17);
            }
        }
        16 => trace(cpu, o, t),
        17 => follow_trace(cpu, o, t),
        18 => cpu.write16(o + STEP_WORD, FIGHTING),
        100 => {
            if cpu.read32(o + LIFE) as i32 <= 0 {
                let by = cpu.read32(o + HIT_BY);
                let (player, weapon) = (cpu.read8(by + HIT_PLAYER) as i8 as i32 as u32, cpu.read8(by + HIT_WEAPON) as i8 as i32 as u32);
                nested!(cpu, 0x0604_3108, hits::score_kill(cpu, player, weapon, o, TR_POINTS));
            }
            let boss = cpu.read32(o + BOSS);
            let slot = cpu.read8(o + SLOT) as i8 as i32 as u32;
            cpu.write32(boss.wrapping_add(slot << 2) + PART_SLOTS, 0);
            cpu.write16(o + STEP_WORD, GONE);
        }
        101 => part_gone(cpu, o, 0x4000),
        _ => {}
    }
    let turn = cpu.read16(o + TURN_Y).wrapping_add(cpu.read16(o + TR_SPIN));
    cpu.write16(o + TURN_Y, turn);
    let p = t + 0x40;
    let (gx, gy) = (cpu.read32(o + TR_GOAL), cpu.read32(o + TR_GOAL + 4));
    cpu.write32(p + T_X, gx);
    cpu.write32(p + T_Y, gy);
    cpu.write32(p + T_SCALES + 4, 0x1_0000);
    cpu.write32(p + T_SCALES, 0x1_0000);
    cpu.write32(p + T_FLAGS, 0);
    cpu.write16(p + T_ANGLE, 0);
    nested!(cpu, 0x0606_4eac, math::turn_point(cpu, p));
    let to = if cpu.read8(o + STEP) as i8 > 15 { TR_EASE_TRACING } else { TR_EASE_FIGHTING };
    let ease = cpu.read16(o + TR_EASE) as i16 as i32 as u32;
    let ease = nested!(cpu, 0x0604_6168, actors::approach(cpu, ease, to, TR_EASE_STEP));
    cpu.write16(o + TR_EASE, ease as u16);
    let e = cpu.read16(o + TR_EASE) as i16 as i32;
    let e = if e < 0 { e + 0xff } else { e } as u32;
    nested!(cpu, 0x0600_8164, clib::ashiftrt_r4::<8>(cpu, e));
    let by = (e as i32 >> 8) as i16 as i32 as u32;
    cpu.write32(p + T_FLAGS, by);
    let x = cpu.read32(o + PLACE_IN_GROUP);
    let run = cpu.read32(p + T_X).wrapping_sub(x);
    let dx = nested!(cpu, 0x0606_62d8, clib::divide(cpu, run, by));
    cpu.write32(o + PLACE_IN_GROUP, x.wrapping_add(dx));
    let y = cpu.read32(o + PLACE_IN_GROUP + 4);
    let by = cpu.read32(p + T_FLAGS);
    let run = cpu.read32(p + T_Y).wrapping_sub(y);
    let dy = nested!(cpu, 0x0606_62d8, clib::divide(cpu, run, by));
    cpu.write32(o + PLACE_IN_GROUP + 4, y.wrapping_add(dy));
    let h = cpu.read16(o + TR_HEADING);
    cpu.write16(o + ANGLE, h);
}

/// Step 16: the leader traces its outline (inner steps 0-5).
fn trace(cpu: &mut Cpu, o: u32, t: u32) {
    let n = cpu.read16(o + TR_OUTLINE) as i16 as i32 as u32;
    let (points, count) = outline_points(cpu, n);
    let point = |cpu: &mut Cpu, k: u32| (cpu.read16(points + 4 * k), cpu.read16(points + 4 * k + 2));
    match cpu.read8(o + INNER) {
        0 => {
            spin_toward(cpu, o, TR_SPIN_TRACE);
            cpu.write32(t + 0x8c, TR_GOAL);
            let (x, y) = point(cpu, 0);
            head_for(cpu, o, t, x, y);
        }
        1 => {
            spin_toward(cpu, o, TR_SPIN_TRACE);
            let (x, y) = point(cpu, 1);
            face(cpu, o, t, x, y);
        }
        2 => {
            if spin_toward(cpu, o, TR_SPIN_FULL as u32) == TR_SPIN_FULL {
                let mut before = 0;
                let mut i = 0;
                while i < count {
                    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, LINE_SIZE)) as i8 as i32 as u32;
                    let line = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, PLAIN_EXEC));
                    if line == 0 {
                        break;
                    }
                    nested!(cpu, 0x0604_dd50, effects::actor_init(cpu, line));
                    let flags = cpu.read32(line + FLAGS) | FLAG_400;
                    cpu.write32(line + FLAGS, flags);
                    cpu.write32(line + BEHAVIOUR, OUTLINE_LINE);
                    cpu.write16(line + OL_INDEX, i as u16);
                    cpu.write32(line + OL_OWNER, o);
                    cpu.write32(line + OL_BEFORE, before);
                    cpu.write32(line + OL_AFTER, 0);
                    if before == 0 {
                        cpu.write32(o + TR_LINES, line);
                    } else {
                        cpu.write32(before + OL_AFTER, line);
                    }
                    i += 1;
                    before = line;
                }
                cpu.write8(o + INNER, 3);
            }
        }
        3 => {
            let mut line = cpu.read32(o + TR_LINES);
            let mut i = 0;
            while i < count {
                if (cpu.read8(line + OL_STEP) as i8) <= 98 {
                    break;
                }
                i += 1;
                line = cpu.read32(line + OL_AFTER);
            }
            if i == count {
                let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, SPARK_SIZE)) as i8 as i32 as u32;
                let s = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
                if s != 0 {
                    nested!(cpu, 0x0604_62ac, actors::actor_setup(cpu, s));
                    let flags = cpu.read32(s + FLAGS) | FLAG_400;
                    cpu.write32(s + FLAGS, flags);
                    cpu.write32(s + BEHAVIOUR, OUTLINE_SPARK);
                    for k in [8, 4, 0] {
                        cpu.write32(s + POSITION + k, 0);
                    }
                    nested!(cpu, 0x0605_2ca0, models::model_screen_point(cpu, o, s + POSITION));
                    let n = cpu.read8(o + NUMBER);
                    cpu.write8(s + NUMBER, n);
                    let side = cpu.read8(o + SIDE);
                    cpu.write8(s + SIDE, side);
                    let w = cpu.read8(o + HIT_WEAPON);
                    cpu.write8(s + HIT_WEAPON, w);
                    let life = cpu.read32(o + LIFE);
                    cpu.write32(s + LIFE, life);
                    let first = cpu.read32(o + TR_LINES);
                    cpu.write32(s + SP_LINE, first);
                    cpu.write32(o + TR_SPARK, s);
                    cpu.write8(o + INNER, 4);
                }
            }
        }
        4 => {
            let spark = cpu.read32(o + TR_SPARK);
            if cpu.read8(spark + OBJ_MODE) as i8 > ENDING {
                let first = cpu.read32(o + TR_LINES);
                cpu.write8(first + OL_STEP, OL_ERASING);
                cpu.write8(o + INNER, 5);
            }
        }
        5 => {
            let mut line = cpu.read32(o + TR_LINES);
            while cpu.read32(line + OL_AFTER) != 0 {
                line = cpu.read32(line + OL_AFTER);
            }
            if cpu.read8(line + OL_STEP) as i8 > 119 {
                let mut line = cpu.read32(o + TR_LINES);
                while line != 0 {
                    cpu.write8(line + OBJ_MODE, TO_STOP);
                    line = cpu.read32(line + OL_AFTER);
                }
                cpu.write16(o + STEP_WORD, FIGHTING);
            }
        }
        _ => {}
    }
}

/// Step 17: a follower heads to its leader's outline's end (its last point: inner 0), faces
/// the point before (1), then keeps heading there while facing back (2) until the leader
/// fights again (15).
fn follow_trace(cpu: &mut Cpu, o: u32, t: u32) {
    let leader = cpu.read32(t + 0x8c);
    let spin = cpu.read16(leader + TR_SPIN);
    cpu.write16(o + TR_SPIN, spin);
    let leader = cpu.read32(t + 0x8c);
    let n = cpu.read16(leader + TR_OUTLINE) as i16 as i32 as u32;
    let (points, count) = outline_points(cpu, n);
    let count = count as u32;
    cpu.write32(t + 0x88, o + INNER);
    let point = |cpu: &mut Cpu, k: u32| (cpu.read16(points.wrapping_add(4 * k)), cpu.read16(points.wrapping_add(4 * k + 2)));
    match cpu.read8(o + INNER) {
        0 => {
            cpu.write32(t + 0x80, count << 2);
            cpu.write32(t + 0x84, points.wrapping_add(count << 2) + 2);
            cpu.write32(t + 0x8c, TR_GOAL);
            let (x, y) = point(cpu, count);
            head_for(cpu, o, t, x, y);
        }
        1 => {
            let (x, y) = point(cpu, count.wrapping_sub(1));
            face(cpu, o, t, x, y);
        }
        2 => {
            let (x, y) = point(cpu, count);
            let dx = ((x as i16 as i32 as u32) << 16).wrapping_sub(cpu.read32(o + TR_GOAL));
            cpu.write32(t + T_X, dx);
            let dy = ((y as i16 as i32 as u32) << 16).wrapping_sub(cpu.read32(o + TR_GOAL + 4));
            cpu.write32(t + T_Y, dy);
            let gx = cpu.read32(o + TR_GOAL).wrapping_add((dx as i32 / 16) as u32);
            cpu.write32(o + TR_GOAL, gx);
            let gy = cpu.read32(o + TR_GOAL + 4).wrapping_add((dy as i32 / 16) as u32);
            cpu.write32(o + TR_GOAL + 4, gy);
            let (x, y) = point(cpu, count.wrapping_sub(1));
            let dx = ((x as i16 as i32 as u32) << 16).wrapping_sub(cpu.read32(o + TR_GOAL));
            cpu.write32(t + T_X, dx);
            let dy = ((y as i16 as i32 as u32) << 16).wrapping_sub(cpu.read32(o + TR_GOAL + 4));
            cpu.write32(t + T_Y, dy);
            turn_to_face(cpu, o, dx, dy);
            let at = cpu.read32(t + 0x8c) + STEP;
            cpu.write32(t + 0x8c, at);
            if cpu.read8(at) == FIGHTING as u8 {
                cpu.write16(o + STEP_WORD, FIGHTING);
            }
        }
        _ => {}
    }
}

// ---- the boss's heart --------------------------------------------------------------------------

/// The heart (part 14 of the boss: hidden behind the core until it is gone): its wobble (a word
/// added to its three turns each frame, +0x74) and the wobble's speed (+0x76), a count (a long,
/// +0x78; in its attacks its low words count shots and frames), its sway (a long, +0x7c: its high
/// word an angle), its drift (16.16, +0x80, +0x84) and the frames until it may dodge (+0x88), its
/// place in the core's model (+0x8c), the core (+0x98).
const HT_WOBBLE: u32 = 0x74;
const HT_WOBBLE_SPEED: u32 = 0x76;
const HT_COUNT: u32 = 0x78;
const HT_WAIT: u32 = 0x7a;
const HT_SWAY: u32 = 0x7c;
const HT_SWAY_ANGLE: u32 = 0x7e;
const HT_DRIFT: u32 = 0x80;
const HT_DODGE_WAIT: u32 = 0x88;
const HT_AT: u32 = 0x8c;
const HT_CORE: u32 = 0x98;
const HT_70: u32 = 0x70;
/// The kinds' first colours (kinds.rs's table +0xc).
const CHAR_BASE: u32 = 0x0607_41fc;
const HT_LIFE: u32 = 50000;
const HT_POINTS: u32 = 10000;
const HT_RATE: u32 = 10;
/// The stage's score (results.rs's): set to 100000 as the heart is shot down.
const STAGE_SCORE_AT: u32 = 0x0607_46f4;
const HT_STAGE_SCORE: u32 = 100_000;
const HT_WOBBLE_START: u16 = 8;
const HT_WOBBLE_MOST: i32 = 0x7ff;
/// Fighting behind the core: it sways along a circle 35 dots across (its z and y).
const HT_SWAY_STEP: u32 = 0x200;
const HT_SWAY_RADIUS: u32 = 0xffdd_0000;
/// Freed (16): its model, the place it had in the core's model as its place in its group.
const HT_FREE_MODEL: u32 = 0x0608_4e2c;
const MODEL_AT: u32 = 0x34;
/// Free (18): every 30 frames or so an attack, picked at random (none, or inner 10-40):
/// 10, a stream of up to 384 aimed shots, one each 6 frames (2 fewer by difficulty), turning;
/// 20, 12 (8 more by difficulty) seekers 30 frames apart; 30, 8 (4 more) of the big ones; 40,
/// rings of 16 (8 more) shots; then 60 frames (90, 30 less by difficulty, after the stream).
const HT_STREAM_EVERY: u32 = 6;
const HT_STREAM_LENGTH: i32 = 0x17f;
const HT_REST: u32 = 60;
const HT_SEEKER: u32 = 0x0608_493c;
const HT_SEEKER_ANIMATION: u32 = 0x060a_302c;
const HT_SEEKER_LIFE: u32 = 500;
const HT_BIG: u32 = 0x0608_4bec;
const HT_BIG_SIZE: u32 = 0x47c;
const HT_RING_SHOT: u32 = 0x0608_485c;
const HT_RING_FRAME: u8 = 200;
const HT_RING_ANIMATION: u32 = 0x060a_303c;
/// The weapon byte its shots carry (+0x55).
const HT_SHOT_WEAPON: u8 = 4;
const HT_GAP: u16 = 30;
/// Kept within 176 by 128 dots; past 144 by 96 it drifts back toward the middle (4 to 6 dots,
/// turned), or, hit (and no drift for 30 frames), it dodges: a random way, turned away from the
/// target player if within 80 dots and facing within 0x1800 of it. The drift fades by a 16th a
/// frame.
const HT_BOUND_X: u32 = 0xb0_0000;
const HT_BOUND_Y: u32 = 0x80_0000;
const HT_BACK_X: i32 = 0x90_0000;
const HT_BACK_Y: i32 = 0x60_0000;
const HT_DRIFT_RANDOM: u32 = 0x2_0000;
const HT_DRIFT_FROM: u32 = 0xfffa_0000;
const HT_DRIFT_FRAMES: u32 = 30;
const HT_DODGE_NEAR: i32 = 0x4f_ffff;
const HT_DODGE_FACING: i32 = 0x17ff;

/// Its wobble on by its speed, the speed reversed past 0x7ff (the wobble held at 0x800).
fn heart_wobble(cpu: &mut Cpu, o: u32) {
    let w = cpu.read16(o + HT_WOBBLE).wrapping_add(cpu.read16(o + HT_WOBBLE_SPEED));
    cpu.write16(o + HT_WOBBLE, w);
    if (cpu.read16(o + HT_WOBBLE) as i16 as i32).wrapping_abs() > HT_WOBBLE_MOST {
        let s = cpu.read16(o + HT_WOBBLE_SPEED).wrapping_neg();
        cpu.write16(o + HT_WOBBLE_SPEED, s);
        let w = cpu.read16(o + HT_WOBBLE) as i16 as i32 as u32;
        let w = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, w, 0x800));
        cpu.write16(o + HT_WOBBLE, w as u16);
    }
}

/// A shot of its own (an actor of `size` bytes with `behaviour`) at its place, at a random
/// angle (or `angle`): the shot (0 if there was no room).
fn heart_piece(cpu: &mut Cpu, o: u32, size: u32, behaviour: u32, angle: Option<u16>) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, size)) as i8 as i32 as u32;
    let p = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if p == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_62ac, actors::actor_setup(cpu, p));
    let flags = cpu.read32(p + FLAGS) | FLAG_400;
    cpu.write32(p + FLAGS, flags);
    cpu.write32(p + BEHAVIOUR, behaviour);
    let (x, y) = (cpu.read32(o + POSITION), cpu.read32(o + POSITION + 4));
    cpu.write32(p + POSITION, x);
    cpu.write32(p + POSITION + 4, y);
    cpu.write32(p + POSITION + 8, 0);
    let a = match angle {
        Some(a) => a,
        None => nested!(cpu, 0x0604_5d90, math::random(cpu, 0)) as u16,
    };
    cpu.write16(p + ANGLE, a);
    p
}

/// After a piece: its count of them down; more to come, the gap and `next`; else rest (60).
fn heart_next(cpu: &mut Cpu, o: u32, next: u8) {
    let n = cpu.read16(o + HT_COUNT).wrapping_sub(1);
    cpu.write16(o + HT_COUNT, n);
    if n as i16 > 0 {
        cpu.write16(o + HT_WAIT, HT_GAP);
        cpu.write8(o + INNER, next);
    } else {
        cpu.write32(o + HT_COUNT, HT_REST);
        cpu.write8(o + INNER, 0);
    }
}

/// Waiting out the gap; then `next`.
fn heart_wait(cpu: &mut Cpu, o: u32, next: u8) {
    let w = cpu.read16(o + HT_WAIT);
    if w as i16 > 0 {
        cpu.write16(o + HT_WAIT, w.wrapping_sub(1));
    } else {
        cpu.write8(o + INNER, next);
    }
}

/// The heart's behaviour: 0: 50000 life, wobbling; 10: shielded until its parent fights; 15:
/// it sways behind the core; once the core ends, 16: freed, 17, 18: it fights alone (above).
/// 100: shot down (10000 points, 10 to the rate, the stage's score 100000); 101: a small burst.
/// Each frame its three turns wobble.
pub fn heart_behaviour(cpu: &mut Cpu, o: u32) {
    let t = cpu.stack();
    let parent = nested!(cpu, 0x0605_2970, models::follow_parent(cpu, o));
    part_shot_or_parent_gone(cpu, o, parent);
    if cpu.read8(o + STEP) as i8 <= 15 {
        let core = cpu.read32(parent + PART_SLOTS);
        if core != 0 {
            cpu.write32(o + HT_CORE, core);
        }
        let core = cpu.read32(o + HT_CORE);
        if cpu.read8(core + OBJ_MODE) as i8 > ENDING {
            cpu.write16(o + STEP_WORD, 16);
        } else {
            for k in [8, 4, 0] {
                cpu.write32(o + HT_AT + k, 0);
            }
            nested!(cpu, 0x0605_2c44, models::model_point(cpu, o, o + HT_AT));
        }
    }
    match cpu.read8(o + STEP) {
        0 => {
            cpu.write32(o + LIFE, HT_LIFE);
            cpu.write16(o + HT_70, 0);
            cpu.write16(o + HT_WOBBLE, 0);
            cpu.write16(o + HT_WOBBLE_SPEED, HT_WOBBLE_START);
            cpu.write32(o + HT_SWAY, 0);
            cpu.write32(o + PART_SLOTS + 4, 0);
            cpu.write32(o + PART_SLOTS, 0);
            let core = cpu.read32(parent + PART_SLOTS);
            cpu.write32(o + HT_CORE, core);
            cpu.write16(o + STEP_WORD, 10);
        }
        10 => {
            heart_wobble(cpu, o);
            if cpu.read8(parent + STEP) as i8 > PARENT_FIGHTING {
                let flags = cpu.read32(o + FLAGS) & !UNHURT;
                cpu.write32(o + FLAGS, flags);
                cpu.write16(o + STEP_WORD, FIGHTING);
            }
        }
        15 => {
            let sway = cpu.read32(o + HT_SWAY).wrapping_add(HT_SWAY_STEP);
            cpu.write32(o + HT_SWAY, sway);
            cpu.write32(t + T_X, 0);
            cpu.write32(t + T_Y, HT_SWAY_RADIUS);
            cpu.write32(t + T_SCALES + 4, 0x1_0000);
            cpu.write32(t + T_SCALES, 0x1_0000);
            cpu.write32(t + T_FLAGS, 0);
            let a = cpu.read16(o + HT_SWAY_ANGLE);
            cpu.write16(t + T_ANGLE, a);
            nested!(cpu, 0x0606_4eac, math::turn_point(cpu, t));
            let (x, y) = (cpu.read32(t + T_X), cpu.read32(t + T_Y));
            cpu.write32(o + PLACE_IN_GROUP + 8, x);
            cpu.write32(o + PLACE_IN_GROUP + 4, y);
            heart_wobble(cpu, o);
        }
        16 => {
            cpu.write32(o + MODEL_AT, HT_FREE_MODEL);
            let (x, y) = (cpu.read32(o + HT_AT), cpu.read32(o + HT_AT + 4));
            cpu.write32(o + PLACE_IN_GROUP, x);
            cpu.write32(o + PLACE_IN_GROUP + 4, y);
            cpu.write32(o + PLACE_IN_GROUP + 8, 0);
            cpu.write16(o + STEP_WORD, 17);
        }
        17 => {
            cpu.write32(o + HT_COUNT, 30);
            cpu.write32(o + HT_DODGE_WAIT, 0);
            cpu.write16(o + STEP_WORD, 18);
        }
        18 => {
            heart_wobble(cpu, o);
            heart_attack(cpu, o, t);
            heart_drift(cpu, o, t);
        }
        100 => {
            if cpu.read32(o + LIFE) as i32 <= 0 {
                let by = cpu.read32(o + HIT_BY);
                let (player, weapon) = (cpu.read8(by + HIT_PLAYER) as i8 as i32 as u32, cpu.read8(by + HIT_WEAPON) as i8 as i32 as u32);
                nested!(cpu, 0x0604_3108, hits::score_kill(cpu, player, weapon, o, HT_POINTS));
                let r = cpu.read32(STAGE_RATE_AT).wrapping_add(HT_RATE);
                cpu.write32(STAGE_RATE_AT, r);
                cpu.write32(STAGE_SCORE_AT, HT_STAGE_SCORE);
            }
            let slot = cpu.read8(o + SLOT) as i8 as i32 as u32;
            cpu.write32(parent.wrapping_add(slot << 2) + PART_SLOTS, 0);
            cpu.write16(o + STEP_WORD, GONE);
        }
        101 => part_gone(cpu, o, 0x4000),
        _ => {}
    }
    for at in [TURN_X, TURN_Y, TURN_Z] {
        let v = cpu.read16(o + at).wrapping_add(cpu.read16(o + HT_WOBBLE));
        cpu.write16(o + at, v);
    }
}

/// Step 18's attacks (inner steps).
fn heart_attack(cpu: &mut Cpu, o: u32, t: u32) {
    if cpu.read8(o + INNER) == 0 {
        let n = cpu.read32(o + HT_COUNT);
        if n as i32 > 0 {
            cpu.write32(o + HT_COUNT, n.wrapping_sub(1));
        } else {
            cpu.write32(o + HT_COUNT, 0);
            let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 5)) as i32;
            cpu.write8(o + INNER, (r.wrapping_abs() as u8).wrapping_mul(10));
        }
    }
    match cpu.read8(o + INNER) {
        10 => cpu.write8(o + INNER, 11),
        11 => {
            let less = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 2));
            let n = cpu.read32(o + HT_COUNT);
            let r = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, n, HT_STREAM_EVERY.wrapping_sub(less)));
            if r == 0 {
                cpu.write32(t + AS_FLAGS, 0);
                cpu.write32(t + AS_SPEED, SHOT_SPEED);
                cpu.write32(t + AS_SHOOTER, o);
                let (x, y) = (cpu.read32(o + POSITION), cpu.read32(o + POSITION + 4));
                cpu.write32(t + AS_POSITION, x);
                cpu.write32(t + AS_POSITION + 4, y);
                cpu.write32(t + AS_POSITION + 8, 0);
                let n = cpu.read32(o + HT_COUNT);
                cpu.write16(t + AS_ANGLE, (n << 8) as u16);
                nested!(cpu, 0x0604_dbc4, bullets::aimed_set(cpu, t));
            }
            let n = cpu.read32(o + HT_COUNT).wrapping_add(1);
            cpu.write32(o + HT_COUNT, n);
            if n as i32 > HT_STREAM_LENGTH {
                let less = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 30));
                cpu.write32(o + HT_COUNT, 90u32.wrapping_sub(less));
                cpu.write8(o + INNER, 0);
            }
        }
        20 => {
            let n = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 8)).wrapping_add(12);
            cpu.write16(o + HT_COUNT, n as u16);
            cpu.write8(o + INNER, 21);
        }
        21 => heart_wait(cpu, o, 22),
        22 => {
            let p = heart_piece(cpu, o, SPARK_SIZE, HT_SEEKER, None);
            if p != 0 {
                cpu.write8(p + BYTE_33, MISSILE_33);
                let n = cpu.read8(o + NUMBER);
                cpu.write8(p + NUMBER, n);
                cpu.write8(p + SPRITE_KIND, EFFECT_KIND as u8);
                cpu.write8(p + SPRITE_FRAME, 0);
                let k = cpu.read8(p + SPRITE_KIND) as i8 as i32 as u32;
                let colour = cpu.read16((CHAR_BASE).wrapping_add(k << 4));
                cpu.write16(p + OWN_COLOUR, colour);
                cpu.write16(p + COLOUR, colour);
                cpu.write32(p + M_ANIMATION, HT_SEEKER_ANIMATION);
                cpu.write32(p + LIFE, HT_SEEKER_LIFE);
                let side = cpu.read8(o + SIDE);
                cpu.write8(p + SIDE, side);
                cpu.write8(p + HIT_WEAPON, HT_SHOT_WEAPON);
            }
            heart_next(cpu, o, 21);
        }
        30 => {
            let n = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 4)).wrapping_add(8);
            cpu.write16(o + HT_COUNT, n as u16);
            cpu.write8(o + INNER, 31);
        }
        31 => heart_wait(cpu, o, 32),
        32 => {
            let p = heart_piece(cpu, o, HT_BIG_SIZE, HT_BIG, None);
            if p != 0 {
                let n = cpu.read8(o + NUMBER);
                cpu.write8(p + NUMBER, n);
                let side = cpu.read8(o + SIDE);
                cpu.write8(p + SIDE, side);
            }
            heart_next(cpu, o, 31);
        }
        40 => {
            let n = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 4)).wrapping_add(8);
            cpu.write16(o + HT_COUNT, n as u16);
            cpu.write8(o + INNER, 41);
        }
        41 => heart_wait(cpu, o, 42),
        42 => {
            heart_ring(cpu, o, t);
            heart_next(cpu, o, 41);
        }
        _ => {}
    }
}

/// A ring of shots (the count, angle and step words on the stack at +0x1a, +0x1c, +0x1e; the
/// shots made a long at +0x14), every other ring half a step turned.
fn heart_ring(cpu: &mut Cpu, o: u32, t: u32) {
    let (count_at, angle_at, step_at) = (t + 0x1a, t + 0x1c, t + 0x1e);
    let n = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 8)).wrapping_add(16);
    cpu.write16(count_at, n as u16);
    let n = cpu.read16(count_at) as i16 as i32 as u32;
    let step = nested!(cpu, 0x0606_62d8, clib::divide(cpu, 0x1_0000, n));
    cpu.write16(step_at, step as u16);
    if cpu.read16(o + HT_COUNT) & 1 != 0 {
        cpu.write16(angle_at, 0);
    } else {
        let half = (cpu.read16(step_at) as i16 as i32 / 2) as u16;
        cpu.write16(angle_at, half);
    }
    cpu.write32(t + 0x14, 0);
    while (cpu.read32(t + 0x14) as i32) < cpu.read16(count_at) as i16 as i32 {
        let a = cpu.read16(angle_at);
        let p = heart_piece(cpu, o, SPARK_SIZE, HT_RING_SHOT, Some(a));
        if p == 0 {
            break;
        }
        cpu.write8(p + BYTE_33, MISSILE_33);
        let n = cpu.read8(o + NUMBER);
        cpu.write8(p + NUMBER, n);
        cpu.write8(p + SPRITE_KIND, 0);
        cpu.write8(p + SPRITE_FRAME, HT_RING_FRAME);
        let colour = cpu.read16(kind(EFFECT_KIND) + K_COLOUR);
        cpu.write16(p + OWN_COLOUR, colour);
        cpu.write16(p + COLOUR, colour);
        cpu.write32(p + M_ANIMATION, HT_RING_ANIMATION);
        let side = cpu.read8(o + SIDE);
        cpu.write8(p + SIDE, side);
        cpu.write8(p + HIT_WEAPON, HT_SHOT_WEAPON);
        let a = cpu.read16(angle_at).wrapping_add(cpu.read16(step_at));
        cpu.write16(angle_at, a);
        let i = cpu.read32(t + 0x14).wrapping_add(1);
        cpu.write32(t + 0x14, i);
    }
}

/// A drift (on the stack at `t`: 4 to 6 dots, at `angle`) set, for 30 frames.
fn heart_set_drift(cpu: &mut Cpu, o: u32, t: u32) {
    nested!(cpu, 0x0606_4eac, math::turn_point(cpu, t));
    let (x, y) = (cpu.read32(t + T_X), cpu.read32(t + T_Y));
    cpu.write32(o + HT_DRIFT, x);
    cpu.write32(o + HT_DRIFT + 4, y);
    cpu.write32(o + HT_DODGE_WAIT, HT_DRIFT_FRAMES);
}

/// Step 18's movement: kept in bounds, drifting back or dodging, the drift fading.
fn heart_drift(cpu: &mut Cpu, o: u32, t: u32) {
    let x = cpu.read32(o + PLACE_IN_GROUP);
    let x = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, x, HT_BOUND_X));
    cpu.write32(o + PLACE_IN_GROUP, x);
    let y = cpu.read32(o + PLACE_IN_GROUP + 4);
    let y = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, y, HT_BOUND_Y));
    cpu.write32(o + PLACE_IN_GROUP + 4, y);
    if (x as i32).wrapping_abs() > HT_BACK_X || (y as i32).wrapping_abs() > HT_BACK_Y {
        cpu.write32(t + T_X, 0);
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, HT_DRIFT_RANDOM));
        cpu.write32(t + T_Y, r.wrapping_add(HT_DRIFT_FROM));
        cpu.write32(t + T_SCALES + 4, 0x1_0000);
        cpu.write32(t + T_SCALES, 0x1_0000);
        cpu.write32(t + T_FLAGS, 0);
        let (x, y) = (cpu.read32(o + PLACE_IN_GROUP), cpu.read32(o + PLACE_IN_GROUP + 4));
        let a = nested!(cpu, 0x0606_780c, sgl::sl_atan(cpu, x.wrapping_neg(), y.wrapping_neg())) as u16;
        cpu.write16(t + T_ANGLE, a.wrapping_add(0x4000));
        heart_set_drift(cpu, o, t);
    }
    let wait = cpu.read32(o + HT_DODGE_WAIT);
    if wait as i32 > 0 {
        cpu.write32(o + HT_DODGE_WAIT, wait.wrapping_sub(1));
    } else if cpu.read16(o + HIT_FLAGS) & HIT_HARMED != 0 {
        cpu.write32(t + T_X, 0);
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, HT_DRIFT_RANDOM));
        cpu.write32(t + T_Y, r.wrapping_add(HT_DRIFT_FROM));
        cpu.write32(t + T_SCALES + 4, 0x1_0000);
        cpu.write32(t + T_SCALES, 0x1_0000);
        cpu.write32(t + T_FLAGS, 0);
        let a = nested!(cpu, 0x0604_5d90, math::random(cpu, 0));
        cpu.write16(t + T_ANGLE, a as u16);
        let player = nested!(cpu, 0x0604_0ccc, aim::target(cpu, o, 0, 0));
        let d = nested!(cpu, 0x0604_5ee4, aim::distance(cpu, o, player)) as i32;
        if d <= HT_DODGE_NEAR {
            let way = cpu.read16(t + T_ANGLE) as i16 as i32 as u32;
            let to = nested!(cpu, 0x0604_5e30, aim::angle_to(cpu, o, player)) as i16 as i32 as u32;
            let off = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, to, way)) as i32;
            if off.wrapping_abs() <= HT_DODGE_FACING {
                let a = cpu.read16(t + T_ANGLE).wrapping_add(0x8000);
                cpu.write16(t + T_ANGLE, a);
            }
        }
        heart_set_drift(cpu, o, t);
    }
    for k in [0, 4] {
        let v = cpu.read32(o + PLACE_IN_GROUP + k).wrapping_add(cpu.read32(o + HT_DRIFT + k));
        cpu.write32(o + PLACE_IN_GROUP + k, v);
    }
    for k in [0, 4] {
        let d = cpu.read32(o + HT_DRIFT + k) as i32;
        cpu.write32(o + HT_DRIFT + k, d.wrapping_add(d.wrapping_neg() / 16) as u32);
    }
}

// ---- the boss's tracers made -------------------------------------------------------------------

/// The part a boss's tracers are made in (+0xec), and its table of them (two tracers, each the
/// other's leader).
const TRACER_HOST: u32 = 0xec;
const TRACER_PAIR: u32 = 0x060a_2e64;

/// A pair of tracers made in the boss's part (when it has none left).
pub fn tracers_set(cpu: &mut Cpu, o: u32) {
    let host = cpu.read32(o + TRACER_HOST);
    let alive = (0..2).filter(|&k| cpu.read32(host + PART_SLOTS + 4 * k) != 0).count();
    if alive == 0 && nested!(cpu, 0x0605_332c, models::model_parts_set(cpu, host, TRACER_PAIR)) != 0 {
        let (a, b) = (cpu.read32(host + PART_SLOTS), cpu.read32(host + PART_SLOTS + 4));
        cpu.write32(a + TR_LEADER, b);
        let (a, b) = (cpu.read32(host + PART_SLOTS), cpu.read32(host + PART_SLOTS + 4));
        cpu.write32(b + TR_LEADER, a);
    }
}

// ---- the boss's actions --------------------------------------------------------------------------

/// The boss's action (a byte, +0x81: the table at 0x060a304c) and its inner step (+0x80; the
/// two written as a word, +0x80), whether its body is turned over (+0x82), its attacks in a row
/// (+0x83), its body (+0xb4) and the action's locals (+0x88-0x97).
const ACTION_WORD: u32 = 0x80;
const ACTION_INNER: u32 = 0x80;
const ACTION: u32 = 0x81;
const TURNED_OVER: u32 = 0x82;
const ATTACKS_IN_A_ROW: u32 = 0x83;
const BODY: u32 = 0xb4;
const ACTIONS: u32 = 0x060a_304c;
/// The actions: 0 choose, 1 move under the player, 2 (not seen run), 3 and 4 the body's two
/// guns' attacks, 5 and 6 rise and fall.
const DECIDE: u16 = 0;
const MOVE: u16 = 1;
/// It moves to stay within 48 dots of the player (kept within 112 dots of the middle), unless
/// already past 96 dots that way; else it attacks: 3 or 4 by which way its body is turned,
/// up to 4 in a row (1 fewer by difficulty), then 5 or 6 at random.
const DECIDE_NEAR: i32 = 0x30_0000;
const DECIDE_BOUND: u32 = 0x70_0000;
const DECIDE_EDGE: i32 = 0x5f_ffff;
const DECIDE_DELAY: u32 = 30;

/// Action 0: what next.
pub fn boss_decide(cpu: &mut Cpu, o: u32) {
    let at = cpu.stack();
    nested!(cpu, 0x0604_0ccc, aim::target(cpu, o, DECIDE_DELAY, at));
    let tx = cpu.read32(at);
    let tx = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, tx, DECIDE_BOUND));
    cpu.write32(at, tx);
    let x = cpu.read32(o + POSITION) as i32;
    let d = (tx as i32).wrapping_sub(x);
    let attack = d.wrapping_abs() <= DECIDE_NEAR || if d < 0 { x <= -0x60_0000 } else { x > DECIDE_EDGE };
    if !attack {
        cpu.write16(o + ACTION_WORD, MOVE);
        return;
    }
    let row = cpu.read8(o + ATTACKS_IN_A_ROW) as i8 as i32;
    let less = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 1)) as i32;
    if row < 4i32.wrapping_sub(less) {
        let action = if cpu.read8(o + TURNED_OVER) == 0 { 3 } else { 4 };
        cpu.write16(o + ACTION_WORD, action);
        let row = cpu.read8(o + ATTACKS_IN_A_ROW).wrapping_add(1);
        cpu.write8(o + ATTACKS_IN_A_ROW, row);
    } else {
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0)) as i32;
        cpu.write16(o + ACTION_WORD, if r > 0 { 5 } else { 6 });
        cpu.write8(o + ATTACKS_IN_A_ROW, 0);
    }
}

/// Action 1's locals: a count (+0x88), its shots' count (+0x8c), its speed (a word, +0x90: 64ths
/// of a dot a frame, and its body's turn y's change), the body's turn y to settle at (+0x92), the
/// x it heads for (+0x94).
const MV_88: u32 = 0x88;
const MV_SHOTS: u32 = 0x8c;
const MV_SPEED: u32 = 0x90;
const MV_TURN_TO: u32 = 0x92;
const MV_GOAL: u32 = 0x94;
/// It speeds (by 1/2 a frame up to 20 dots) toward the player, firing a pair from its body
/// every 6 frames (2 fewer by difficulty): 32 dots either side, 16 up, spread an eighth of a turn
/// (and 1/32 more or less by difficulty) about its body's screen angle. Within 32 dots it turns
/// its body over (by an eighth of the way, at most 0x500) to face where it goes.
const MV_SHOT_EVERY: u32 = 6;
const MV_SIDE: u32 = 0x20_0000;
const MV_UP: u32 = 0xfff0_0000;
const MV_SPREAD: u32 = 0x800;
const MV_SPEED_MOST: u32 = 0x500;
const MV_ACCEL: u32 = 32;
const MV_THERE: i32 = 0x1f_ffff;

/// Action 1: move under the player.
pub fn boss_move(cpu: &mut Cpu, o: u32) {
    let t = cpu.stack();
    let body = cpu.read32(o + BODY);
    cpu.write32(t + 0x40, body);
    match cpu.read8(o + ACTION_INNER) {
        0 => {
            cpu.write32(o + MV_88, 0);
            cpu.write32(o + MV_SHOTS, 0);
            cpu.write16(o + MV_SPEED, 0);
            cpu.write8(o + ACTION_INNER, 1);
        }
        1 => {
            let less = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, 2));
            cpu.write32(t + 0x44, MV_SHOTS);
            let n = cpu.read32(o + MV_SHOTS);
            let r = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, n, MV_SHOT_EVERY.wrapping_sub(less)));
            if r == 0 {
                for side in [MV_SIDE.wrapping_neg(), MV_SIDE] {
                    cpu.write32(t + T_X, side);
                    cpu.write32(t + T_Y, MV_UP);
                    cpu.write32(t + T_8, 0);
                    let body = cpu.read32(t + 0x40);
                    nested!(cpu, 0x0605_2ca0, models::model_screen_point(cpu, body, t));
                    let body = cpu.read32(t + 0x40);
                    let a = nested!(cpu, 0x0608_054c, model_screen_angle(cpu, body)) as u16;
                    let s = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, MV_SPREAD)).wrapping_add(MV_SPREAD) as u16;
                    if side == MV_SIDE {
                        cpu.write16(t + AS_ANGLE, s.wrapping_add(a));
                    } else {
                        cpu.write16(t + AS_ANGLE, a.wrapping_sub(s));
                        cpu.write32(t + AS_FLAGS, 0);
                        cpu.write32(t + AS_SPEED, SHOT_SPEED);
                        cpu.write32(t + AS_SHOOTER, o);
                    }
                    nested!(cpu, 0x0604_dbc4, bullets::aimed_set(cpu, t));
                }
            }
            let n = cpu.read32(o + MV_SHOTS).wrapping_add(1);
            cpu.write32(o + MV_SHOTS, n);
            let player = nested!(cpu, 0x0604_0ccc, aim::target(cpu, o, 0, 0));
            let px = cpu.read32(player + POSITION);
            let goal = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, px, DECIDE_BOUND));
            cpu.write32(o + MV_GOAL, goal);
            let to = if (goal as i32) > cpu.read32(o + POSITION) as i32 { MV_SPEED_MOST } else { MV_SPEED_MOST.wrapping_neg() };
            let speed = cpu.read16(o + MV_SPEED) as i16 as i32 as u32;
            let speed = nested!(cpu, 0x0604_6168, actors::approach(cpu, speed, to, MV_ACCEL));
            cpu.write16(o + MV_SPEED, speed as u16);
            let gap = cpu.read32(o + MV_GOAL).wrapping_sub(cpu.read32(o + POSITION)) as i32;
            if gap.wrapping_abs() <= MV_THERE {
                let body = cpu.read32(t + 0x40);
                let going_right = cpu.read16(o + MV_SPEED) as i16 > 0;
                let body_turned = cpu.read16(body + TURN_Y) as i16 > 0;
                cpu.write16(o + MV_TURN_TO, if going_right == body_turned { 0x8000 } else { 0 });
                cpu.write8(o + ACTION_INNER, 2);
            }
        }
        2 => {
            let body = cpu.read32(t + 0x40);
            let turn = cpu.read16(body + TURN_Y) as i16 as i32 as u32;
            let to = cpu.read16(o + MV_TURN_TO) as i16 as i32 as u32;
            let d = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, turn, to)) as i32 / 8;
            let speed = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, d as u32, MV_SPEED_MOST)) as u16;
            cpu.write16(o + MV_SPEED, speed);
            if (cpu.read16(o + MV_SPEED) as i16 as i32).wrapping_abs() <= 15 {
                let to = cpu.read16(o + MV_TURN_TO);
                cpu.write16(body + TURN_Y, to);
                cpu.write16(o + MV_SPEED, 0);
                cpu.write16(o + ACTION_WORD, DECIDE);
            }
        }
        _ => {}
    }
    let speed = cpu.read16(o + MV_SPEED) as i16 as i32 as u32;
    let x = cpu.read32(o + POSITION).wrapping_add(speed << 6);
    cpu.write32(o + POSITION, x);
    let body = cpu.read32(t + 0x40);
    let turn = cpu.read16(body + TURN_Y).wrapping_add(cpu.read16(o + MV_SPEED));
    cpu.write16(body + TURN_Y, turn);
    let over = (cpu.read16(body + TURN_Y) as i16 as i32).wrapping_abs() > 0x4000;
    cpu.write8(o + TURNED_OVER, over as u8);
}

/// Actions 3 and 4: the gun in the body's slot (+0xb8 or +0xbc) set to attack (its step 16)
/// once it fights (15), and waited on until it fights again; with the gun gone, action 2.
fn gun_attack(cpu: &mut Cpu, o: u32, slot: u32) {
    let gun = cpu.read32(o + slot);
    if gun == 0 || cpu.read8(gun + OBJ_MODE) as i8 > ENDING {
        cpu.write16(o + ACTION_WORD, 2);
        return;
    }
    match cpu.read8(o + ACTION_INNER) {
        0 => cpu.write8(o + ACTION_INNER, 1),
        1 if cpu.read8(gun + STEP) == FIGHTING as u8 => {
            cpu.write16(gun + STEP_WORD, 16);
            cpu.write8(o + ACTION_INNER, 2);
        }
        2 if cpu.read8(gun + STEP) == FIGHTING as u8 => cpu.write16(o + ACTION_WORD, DECIDE),
        _ => {}
    }
}

pub fn boss_gun_b_attack(cpu: &mut Cpu, o: u32) {
    gun_attack(cpu, o, BODY + 4);
}

pub fn boss_gun_c_attack(cpu: &mut Cpu, o: u32) {
    gun_attack(cpu, o, BODY + 8);
}

/// A rise and fall (actions 5 and 6): the locals' places (the angle kept, its spin, the y kept,
/// the rise's speed, a count).
struct Rise {
    angle: u32,
    spin: u32,
    y: u32,
    speed: u32,
    count: u32,
}

const RISE_5: Rise = Rise { angle: 0x88, spin: 0x8a, y: 0x8c, speed: 0x90, count: 0x94 };
const RISE_6: Rise = Rise { angle: 0x8c, spin: 0x8e, y: 0x90, speed: 0x94, count: 0x88 };
/// It spins up (by 2 a frame) to 0x80, then (the body told to attack, in 6) rises (its spin to
/// `top_spin`, its speed by 1/256 a frame to 1/4 dot) until past 80 dots down, falls back the
/// same (in 6 the body told to fight again) to 16 dots above where it was, then settles its
/// spin (a 16th of the way to the angle it had, at most 0x100) and its place (by 1/4 dot), and
/// chooses again.
const RISE_SPIN: u16 = 0x80;
const RISE_SPIN_STEP: u32 = 2;
const RISE_SPEED: u32 = 0x4000;
const RISE_ACCEL: u32 = 0x100;
const RISE_LOW: i32 = 0x4f_ffff;
const RISE_BACK: u32 = 0x10_0000;
const RISE_SETTLE: u32 = 0x100;

fn rise_and_fall(cpu: &mut Cpu, o: u32, r: &Rise, top_spin: u32, body_steps: Option<(u16, u16)>) {
    match cpu.read8(o + ACTION_INNER) {
        0 => {
            let a = cpu.read16(o + ANGLE);
            cpu.write16(o + r.angle, a);
            cpu.write16(o + r.spin, 0);
            let y = cpu.read32(o + POSITION + 4);
            cpu.write32(o + r.y, y);
            cpu.write32(o + r.speed, 0);
            cpu.write32(o + r.count, 0);
            cpu.write8(o + ACTION_INNER, 1);
        }
        1 => {
            let spin = cpu.read16(o + r.spin) as i16 as i32 as u32;
            let spin = nested!(cpu, 0x0604_6168, actors::approach(cpu, spin, RISE_SPIN as u32, RISE_SPIN_STEP)) as u16;
            cpu.write16(o + r.spin, spin);
            if spin == RISE_SPIN {
                if let Some((attack, _)) = body_steps {
                    let body = cpu.read32(o + BODY);
                    cpu.write16(body + STEP_WORD, attack);
                }
                cpu.write8(o + ACTION_INNER, 2);
            }
        }
        2 => {
            let spin = cpu.read16(o + r.spin) as i16 as i32 as u32;
            let spin = nested!(cpu, 0x0604_6168, actors::approach(cpu, spin, top_spin, RISE_SPIN_STEP));
            cpu.write16(o + r.spin, spin as u16);
            let v = cpu.read32(o + r.speed);
            let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, RISE_SPEED, RISE_ACCEL));
            cpu.write32(o + r.speed, v);
            if cpu.read32(o + POSITION + 4) as i32 > RISE_LOW {
                cpu.write8(o + ACTION_INNER, 3);
            }
        }
        3 => {
            let v = cpu.read32(o + r.speed);
            let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, RISE_SPEED.wrapping_neg(), RISE_ACCEL));
            cpu.write32(o + r.speed, v);
            if let Some((_, fight)) = body_steps {
                let body = cpu.read32(o + BODY);
                cpu.write16(body + STEP_WORD, fight);
            }
            if cpu.read32(o + POSITION + 4) as i32 <= cpu.read32(o + r.y).wrapping_add(RISE_BACK) as i32 {
                cpu.write8(o + ACTION_INNER, 4);
            }
        }
        4 => {
            let a = cpu.read16(o + ANGLE) as i16 as i32 as u32;
            let kept = cpu.read16(o + r.angle) as i16 as i32 as u32;
            let ahead = nested!(cpu, 0x0604_61cc, camera::angle_ahead(cpu, a, kept)) as i32 / 16;
            let spin = nested!(cpu, 0x0606_4f3c, math::clamp(cpu, ahead as u32, RISE_SETTLE));
            cpu.write16(o + r.spin, spin as u16);
            if (spin as i16) < 4 {
                let kept = cpu.read16(o + r.angle);
                cpu.write16(o + ANGLE, kept);
                cpu.write16(o + r.spin, 0);
            }
            let v = cpu.read32(o + r.speed);
            let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0, RISE_ACCEL));
            cpu.write32(o + r.speed, v);
            let settled = if v == 0 {
                let y = cpu.read32(o + POSITION + 4);
                let to = cpu.read32(o + r.y);
                let y = nested!(cpu, 0x0604_6168, actors::approach(cpu, y, to, RISE_SPEED));
                cpu.write32(o + POSITION + 4, y);
                true
            } else if cpu.read32(o + POSITION + 4) as i32 > cpu.read32(o + r.y) as i32 {
                false
            } else {
                let to = cpu.read32(o + r.y);
                cpu.write32(o + POSITION + 4, to);
                cpu.write32(o + r.speed, 0);
                true
            };
            if settled && cpu.read32(o + POSITION + 4) == cpu.read32(o + r.y) && cpu.read16(o + r.spin) == 0 {
                cpu.write16(o + ACTION_WORD, DECIDE);
            }
        }
        _ => {}
    }
    let a = cpu.read16(o + ANGLE).wrapping_add(cpu.read16(o + r.spin));
    cpu.write16(o + ANGLE, a);
    let y = cpu.read32(o + POSITION + 4).wrapping_add(cpu.read32(o + r.speed));
    cpu.write32(o + POSITION + 4, y);
}

/// Action 5: a rise and fall, the body attacking (its step 17) every 6 frames of it.
const RISE_5_TOP_SPIN: u32 = 0x200;
const RISE_5_EVERY: u32 = 6;
const BODY_RING: u16 = 17;

pub fn boss_rise(cpu: &mut Cpu, o: u32) {
    rise_and_fall(cpu, o, &RISE_5, RISE_5_TOP_SPIN, None);
    if cpu.read8(o + ACTION_INNER).wrapping_sub(2) <= 1 {
        let n = cpu.read32(o + RISE_5.count);
        if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, n, RISE_5_EVERY)) == 0 {
            let body = cpu.read32(o + BODY);
            cpu.write16(body + STEP_WORD, BODY_RING);
        }
        let n = cpu.read32(o + RISE_5.count).wrapping_add(1);
        cpu.write32(o + RISE_5.count, n);
    }
}

/// Action 6: a rise and fall with the body's circling missiles (its step 18) while it rises.
const RISE_6_TOP_SPIN: u32 = 0x180;
const BODY_MISSILES: u16 = 18;

pub fn boss_rise_missiles(cpu: &mut Cpu, o: u32) {
    rise_and_fall(cpu, o, &RISE_6, RISE_6_TOP_SPIN, Some((BODY_MISSILES, FIGHTING)));
    if cpu.read8(o + ACTION_INNER) == 2 {
        let n = cpu.read32(o + RISE_6.count);
        nested!(cpu, 0x0600_6a6e, math::remainder(cpu, n, 120));
        let n = cpu.read32(o + RISE_6.count).wrapping_add(1);
        cpu.write32(o + RISE_6.count, n);
    }
}

// ---- the boss --------------------------------------------------------------------------------------

/// The boss (a numbered actor, the root of its parts' group: the table at 0x060a2c5c): its time
/// (+0x70: three minutes, counted while a player is in), the control its sound plays on (+0x74),
/// a count (+0x78), the results (+0x7c), its tracers' count (a word, +0x86); its model root
/// (+0xb0) and body (+0xb4), the part whose life it shows (+0xec).
const BS_TIME: u32 = 0x70;
const BS_SOUND: u32 = 0x74;
const BS_COUNT: u32 = 0x78;
const BS_RESULTS: u32 = 0x7c;
const BS_TRACERS: u32 = 0x86;
const BS_ROOT: u32 = 0xb0;
const BS_PARTS: u32 = 0x060a_2c5c;
const BS_TIME_FRAMES: u32 = 0x2a30;
const BS_PLACE: (u32, u32) = (0xffc0_0000, 0x8_0000);
/// The players (the numbered objects' first two).
const PLAYERS: [u32; 2] = [0x0607_40f0, 0x0607_40f4];
/// The players' ships' kind and colour while it fights (kind 62's), and after (kind 2's).
const SHIP_KIND_FIGHT: u8 = 62;
const SHIP_KIND_AFTER: u8 = 2;
/// Its colours (kind 60's, 16 of them in the colour table at 0x06030e40) set to 0xae, then
/// faded in (colours_blend: by 1/0x78 of the way a frame) from 0x060a3068 to 0x060a3088.
const COLOUR_TABLE: u32 = 0x0603_0e40;
const BOSS_KIND: u32 = 60;
const BOSS_COLOURS_START: u16 = 0xae;
const FADE_FROM: u32 = 0x060a_3068;
const FADE_TO: u32 = 0x060a_3088;
const FADE_DONE: u32 = 0x1000_0000;
const FADE_STEP: u32 = 0x22_2222;
/// Its music: faded to half over 300 frames, sound 103; at the end faded back up over 60, 103
/// stopped.
const BS_MUSIC_FADE: (u32, u32) = (0x8000, 300);
const BS_MUSIC_BACK: (u32, u32) = (0x1_0000, 60);
const BS_SOUND_ID: u32 = 103;
const BS_GAUGE: u32 = 50000;
const FULL_SPEED: u32 = 0x0600_ed90;
/// Tracers every 900 frames (300 fewer by difficulty).
const BS_TRACER_EVERY: i32 = 900;
const BS_TRACER_BY_DIFFICULTY: u32 = 300;

/// The players' ships' sprite kind and colour (the second's 16 on).
fn ships_kind(cpu: &mut Cpu, k: u8) {
    for (n, at) in PLAYERS.into_iter().enumerate() {
        let ship = cpu.read32(at);
        if ship != 0 {
            cpu.write8(ship + SPRITE_KIND, k);
            let colour = cpu.read16(kind(k as u32) + K_COLOUR).wrapping_add(16 * n as u16);
            cpu.write16(ship + OWN_COLOUR, colour);
            cpu.write16(ship + COLOUR, colour);
        }
    }
}

/// The boss's behaviour. 0: its parts made (hidden until then), placed, its colours set; 1: the
/// music down, its sound; 2: its colours fade in; 10: (at full speed) its gauge, its locals
/// cleared, 15: it fights: its actions, tracers now and then; once its time is up or its life
/// gone, 100: the end (the ships' colours back, unhit, the music up, the bullets cleared; 60
/// frames, the results, once they end the next scroll, 120 frames, and it ends). From 2 to 99 the
/// ships take kind 62's colours.
pub fn boss_behaviour(cpu: &mut Cpu, o: u32) {
    let t = cpu.stack();
    match cpu.read8(o + STEP) {
        0 => {
            let flags = cpu.read32(o + FLAGS) | HIDDEN;
            cpu.write32(o + FLAGS, flags);
            if nested!(cpu, 0x0605_2ff0, models::model_group_set(cpu, o, BS_PARTS)) != 0 {
                let flags = cpu.read32(o + FLAGS) & !HIDDEN;
                cpu.write32(o + FLAGS, flags);
                cpu.write32(STAGE_RATE_AT, 0);
                cpu.write32(STAGE_SCORE_AT, 0);
                cpu.write32(o + POSITION, 0);
                cpu.write32(o + POSITION + 4, BS_PLACE.0);
                cpu.write32(o + POSITION + 8, BS_PLACE.1);
                cpu.write16(o + ANGLE, 0x8000);
                cpu.write32(o + BS_COUNT, 0);
                cpu.write32(o + BS_TIME, BS_TIME_FRAMES);
                for i in 0..16u16 {
                    let c = cpu.read16(kind(BOSS_KIND) + K_COLOUR).wrapping_add(i) as u32;
                    cpu.write16(COLOUR_TABLE + 2 * c, BOSS_COLOURS_START);
                }
                cpu.write16(o + STEP_WORD, 1);
            }
        }
        1 => {
            cpu.write32(o + BS_COUNT, 0);
            nested!(cpu, 0x0600_4edc, sound::music_fade_to(cpu, BS_MUSIC_FADE.0, BS_MUSIC_FADE.1));
            let control = nested!(cpu, 0x0600_50d4, sound::se_set(cpu, BS_SOUND_ID));
            cpu.write8(o + BS_SOUND, control as u8);
            cpu.write16(o + STEP_WORD, 2);
        }
        2 => {
            let v = cpu.read32(o + BS_COUNT);
            let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, FADE_DONE, FADE_STEP));
            cpu.write32(o + BS_COUNT, v);
            cpu.write16(t, dots(v));
            cpu.write16(t + 2, 16);
            cpu.write32(t + 4, FADE_FROM);
            cpu.write32(t + 8, FADE_TO);
            let c = cpu.read16(kind(BOSS_KIND) + K_COLOUR) as u32;
            cpu.write32(t + 12, COLOUR_TABLE + 2 * c);
            nested!(cpu, 0x0600_5ba4, palette::colours_blend(cpu, t));
            if cpu.read32(o + BS_COUNT) == FADE_DONE {
                cpu.write16(o + STEP_WORD, 10);
            }
        }
        10 => {
            if cpu.read8(FULL_SPEED) == 0 {
                nested!(cpu, 0x0604_64d8, hud::boss_gauge_set(cpu, BS_GAUGE));
                for k in 0..14 {
                    cpu.write32(o + BS_COUNT + 4 * k, 0);
                }
                cpu.write16(o + STEP_WORD, FIGHTING);
            }
        }
        15 => boss_fight(cpu, o),
        // (held as the SH-2 code holds them: the step, the boss, 15 and 2; the fade under
        // bullets_cleared pushes them, and an effect made later takes one as its player)
        100 => cpu.holding(&[(8, 100), (9, o), (10, 15), (11, 2)], |cpu| boss_end(cpu, o)),
        _ => {}
    }
    if cpu.read8(o + STEP).wrapping_sub(2) <= 97 {
        ships_kind(cpu, SHIP_KIND_FIGHT);
    }
}

/// Step 15.
fn boss_fight(cpu: &mut Cpu, o: u32) {
    let n = cpu.read16(o + BS_TRACERS).wrapping_add(1);
    cpu.write16(o + BS_TRACERS, n);
    let n = cpu.read16(o + BS_TRACERS) as i16 as i32;
    let less = nested!(cpu, 0x0604_5dcc, math::by_difficulty(cpu, BS_TRACER_BY_DIFFICULTY)) as i32;
    if n >= BS_TRACER_EVERY.wrapping_sub(less) {
        nested!(cpu, 0x0608_590c, tracers_set(cpu, o));
        cpu.write16(o + BS_TRACERS, 0);
    }
    if cpu.read32(o + BS_ROOT) != 0 {
        let body = cpu.read32(o + BODY);
        if body == 0 {
            for k in [8, 4, 0] {
                cpu.write32(o + POSITION + k, 0);
            }
            let flags = cpu.read32(o + FLAGS) | HIDDEN;
            cpu.write32(o + FLAGS, flags);
            cpu.write16(o + ANGLE, 0);
            let root = cpu.read32(o + BS_ROOT);
            cpu.write8(root + OBJ_MODE, TO_STOP);
            cpu.write32(o + BS_ROOT, 0);
        } else if cpu.read8(body + OBJ_MODE) as i8 <= ENDING {
            match cpu.read8(o + ACTION) as i8 {
                0 => nested!(cpu, 0x0608_5974, boss_decide(cpu, o)),
                1 => nested!(cpu, 0x0608_5a44, boss_move(cpu, o)),
                3 => nested!(cpu, 0x0608_5d3c, boss_gun_b_attack(cpu, o)),
                4 => nested!(cpu, 0x0608_5db0, boss_gun_c_attack(cpu, o)),
                5 => nested!(cpu, 0x0608_5e24, boss_rise(cpu, o)),
                6 => nested!(cpu, 0x0608_6038, boss_rise_missiles(cpu, o)),
                a => {
                    let f = cpu.read32(ACTIONS.wrapping_add((a as i32 as u32) << 2));
                    cpu.call(f, &[o]);
                }
            }
        }
    }
    let shown = cpu.read32(o + TRACER_HOST);
    let life = cpu.read32(shown + LIFE);
    nested!(cpu, 0x0604_64b8, hud::boss_gauge_life(cpu, life));
    if cpu.read32(PLAYERS[0]) != 0 || cpu.read32(PLAYERS[1]) != 0 {
        let time = cpu.read32(o + BS_TIME).wrapping_sub(1);
        cpu.write32(o + BS_TIME, time);
    }
    let shown = cpu.read32(o + TRACER_HOST);
    if cpu.read32(o + BS_TIME) as i32 <= 0 || cpu.read32(shown + LIFE) as i32 <= 0 {
        cpu.write16(o + STEP_WORD, SHOT_DOWN);
    }
}

/// Step 100 (inner steps 0-4).
fn boss_end(cpu: &mut Cpu, o: u32) {
    match cpu.read8(o + INNER) {
        0 => {
            ships_kind(cpu, SHIP_KIND_AFTER);
            let flags = cpu.read32(o + FLAGS) | NO_HITS;
            cpu.write32(o + FLAGS, flags);
            nested!(cpu, 0x0600_4edc, sound::music_fade_to(cpu, BS_MUSIC_BACK.0, BS_MUSIC_BACK.1));
            let control = cpu.read8(o + BS_SOUND) as i8 as i32 as u32;
            nested!(cpu, 0x0600_52e0, sound::se_stop(cpu, BS_SOUND_ID, control));
            nested!(cpu, 0x0605_61e0, screen::bullets_cleared(cpu));
            cpu.write32(o + BS_COUNT, 0);
            cpu.write8(o + INNER, 2);
        }
        2 => {
            let n = cpu.read32(o + BS_COUNT).wrapping_add(1);
            cpu.write32(o + BS_COUNT, n);
            if n as i32 > 59 {
                let r = nested!(cpu, 0x0605_284c, results::results_set(cpu, 0));
                cpu.write32(o + BS_RESULTS, r);
                if r != 0 {
                    cpu.write8(o + INNER, 3);
                }
            }
        }
        3 => {
            let r = cpu.read32(o + BS_RESULTS);
            if cpu.read8(r + OBJ_MODE) as i8 > ENDING {
                nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
                cpu.write32(o + BS_COUNT, 0);
                cpu.write8(o + INNER, 4);
            }
        }
        4 => {
            let n = cpu.read32(o + BS_COUNT).wrapping_add(1);
            cpu.write32(o + BS_COUNT, n);
            if n as i32 > 119 {
                cpu.write8(o + OBJ_MODE, TO_STOP);
                cpu.write8(o + INNER, GONE as u8);
            }
        }
        _ => {}
    }
}

/// The boss made (a numbered actor of 0x138 bytes, numbered from 2, if one is free below 47):
/// its sprite and hit fields, life a million (its parts' harm is passed to it), 0x20000 flagged,
/// numbered; kind 60's models' attributes and colours (13 models), its colours: its number, or
/// 0.
const BS_SIZE: u32 = 0x138;
const NUMBERED_EXEC: u32 = 0x0604_d600;
const NUMBERED: u32 = 0x0607_40f0;
const BS_LIFE: u32 = 0x10_0000;
const BS_FLAG: u32 = 0x2_0000;
const BOSS_BEHAVIOUR: u32 = 0x0608_6250;
const BS_MODELS: u32 = 13;
const BS_COLOURS: u32 = 0x060a_30a8;
const HITS_AT: u32 = 0x3c;

pub fn boss_set(cpu: &mut Cpu) -> u32 {
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, 2)) as i8 as i32;
    if n > 46 {
        return 0;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, BS_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, NUMBERED_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + 0x10));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HITS_AT));
    cpu.write16(o + STEP_WORD, 0);
    cpu.write32(o + LIFE, BS_LIFE);
    cpu.write8(o + SIDE, 2);
    cpu.write8(o + HIT_WEAPON, 3);
    let flags = cpu.read32(o + FLAGS) | BS_FLAG;
    cpu.write32(o + FLAGS, flags);
    cpu.write32(o + BEHAVIOUR, BOSS_BEHAVIOUR);
    cpu.write8(o + NUMBER, n as u8);
    let n = cpu.read8(o + NUMBER) as i8 as i32 as u32;
    cpu.write32(NUMBERED.wrapping_add(n << 2), o);
    nested!(cpu, 0x0608_04d0, kind_attrs_set(cpu, BOSS_KIND));
    for i in 0..BS_MODELS {
        let c = (cpu.read16(kind(BOSS_KIND) + K_COLOUR) as u32).wrapping_add(i) & 0xffff;
        nested!(cpu, 0x0604_5b1c, models::model_colour(cpu, BOSS_KIND, i, c));
    }
    let c = cpu.read16(kind(BOSS_KIND) + K_COLOUR) as u32;
    nested!(cpu, 0x0600_5b18, palette::set_colours(cpu, BS_COLOURS, c, 16));
    cpu.read8(o + NUMBER) as i8 as i32 as u32
}

// ---- the enemy rows ----------------------------------------------------------------------------

/// A row of enemies (a timeline event, timeline.rs's; stg01.rs's enemy_row with this stage's
/// kinds: 0-2 ground_set's, 3-6 guns, 7-9 the wounded, 10-13 platforms, 14-15 launchers, 16
/// spreaders, 18 a carrier, 21 a hatch, 128 two rings of three carriers circling).
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
/// Motion 1 places each round an arc (its radius a word, +0x1c; its angle the direction);
/// motion 2 makes the kind 0 after the first.
const E_RADIUS: u32 = 0x1c;
/// A ground piece's description (ground.rs's, on the stack).
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
const GROUND_SCRIPTS: u32 = 0x060a_31cc;
const GUN_SCRIPTS: u32 = 0x060a_326c;
const GUN_AIMS: u32 = 0x060a_327c;
const WOUNDED_SCRIPTS: u32 = 0x060a_3320;
const WOUNDED_AIMS: u32 = 0x060a_3340;
const PLATFORM_SCRIPTS: u32 = 0x060a_3378;
/// Kind 128's carriers: type 5, kind 2, circling (their behaviour 0x060890dc) a third of a turn
/// apart (the second ring a sixth on), at speeds 80 and 160, their +0xa2 32, +0xac -192 dots.
const RING_CARRIER: u32 = 0x0608_90dc;
const RC_ANGLE: u32 = 0x8a;
const RC_SPEED: u32 = 0x8e;
const RC_A2: u32 = 0xa2;
const RC_AC: u32 = 0xac;
const RC_7C: u32 = 0x7c;
const RINGS: [(u16, u16); 2] = [(0, 0x50), (0x2aaa, 0xa0)];

/// A row's place round its arc (before each is made, motion 1).
pub fn row_arc(cpu: &mut Cpu, event: u32) {
    if cpu.read8(event + E_MOTION) != 1 {
        return;
    }
    let angle = cpu.read16(event + E_DIRECTION) as i16 as i32 as u32;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, angle)) as i32;
    let r = cpu.read16(event + E_RADIUS) as i16 as i32;
    cpu.write16(event + E_PLACE, (s.wrapping_mul(r) / 0x1_0000) as u16);
    let angle = cpu.read16(event + E_DIRECTION) as i16 as i32 as u32;
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, angle)) as i32;
    let r = cpu.read16(event + E_RADIUS) as i16 as i32;
    cpu.write16(event + E_PLACE + 2, (c.wrapping_neg().wrapping_mul(r) / 0x1_0000) as u16);
}

/// After each is made (motion 2): the kind 0.
pub fn row_rekind(cpu: &mut Cpu, event: u32) {
    if cpu.read8(event + E_MOTION) == 2 {
        cpu.write8(event + E_KIND, 0);
    }
}

pub fn enemy_row(cpu: &mut Cpu, event: u32) -> u32 {
    if cpu.read8(event + E_WAIT) == 0 {
        let d = cpu.stack();
        cpu.write32(d + 0x40, d + D_TYPE);
        loop {
            if cpu.read8(event + E_MOTION) != 0 {
                nested!(cpu, 0x0608_67cc, row_arc(cpu, event));
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
                nested!(cpu, 0x0608_6844, row_rekind(cpu, event));
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
        7..=9 => {
            cpu.write16(d + D_KIND, kind_word(7));
            let v = byte(cpu, E_1E) << 8;
            cpu.write16(d + D_34, v);
            let v = byte(cpu, E_1F);
            cpu.write16(d + D_32, v);
            let at = cpu.read8(event + E_1C) as i8 as i32 as u32;
            let aim = cpu.read32(WOUNDED_AIMS.wrapping_add(4 * at));
            let s = script(cpu, WOUNDED_SCRIPTS);
            nested!(cpu, 0x0605_bbac, ground::wounded_set(cpu, d, s, aim));
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
        16 => {
            let s = script(cpu, GROUND_SCRIPTS);
            nested!(cpu, 0x0605_cee8, ground::spreader_set(cpu, d, s));
        }
        18 => {
            let v = cpu.read16(event + E_1E);
            cpu.write16(d + D_34, v);
            let carried = cpu.read16(event + E_1C) as u32;
            let s = script(cpu, PLATFORM_SCRIPTS);
            nested!(cpu, 0x0605_d51c, ground::carrier_set(cpu, d, s, carried));
        }
        21 => {
            let s = script(cpu, GROUND_SCRIPTS);
            nested!(cpu, 0x0605_e030, ground::hatch_set(cpu, d, s));
        }
        128 => {
            cpu.write16(d + D_34, 0);
            cpu.write16(d + D_KIND, 2);
            for (offset, speed) in RINGS {
                let mut i: i8 = 0;
                while i <= 2 {
                    let at = cpu.read32(d + 0x40);
                    cpu.write16(at, 5);
                    cpu.write32(d + 0x44, RC_ANGLE);
                    let s = script(cpu, PLATFORM_SCRIPTS);
                    let o = nested!(cpu, 0x0605_d51c, ground::carrier_set(cpu, d, s, 0));
                    if o != 0 {
                        cpu.write32(o + BEHAVIOUR, RING_CARRIER);
                        let a = (i as u16).wrapping_mul(0x5555).wrapping_add(offset);
                        cpu.write16(o + RC_ANGLE, a);
                        cpu.write16(o + RC_SPEED, speed);
                        cpu.write16(o + RC_A2, 0x20);
                        cpu.write32(o + RC_AC, 0xff40_0000);
                        cpu.write8(o + RC_7C, 0);
                    }
                    i = i.wrapping_add(1);
                }
            }
        }
        _ => {}
    }
}

// ---- the spiral flier ------------------------------------------------------------------------------

/// The spiral flier (a numbered enemy actor): its count (a word, +0x8c), its shot's wait (a word,
/// +0x8a), its turn from its parent's angle (a word, +0x92), its distance out (a word, +0xa8:
/// growing two dots a frame from -192), its parent (+0x6c).
const SF_COUNT: u32 = 0x8c;
const SF_WAIT: u32 = 0x8a;
const SF_TURN: u32 = 0x92;
const SF_OUT: u32 = 0xa8;
const SF_PARENT: u32 = 0x6c;
/// It ends off the screen after a second; shot down (the game's rank past 2 adds something
/// not rewritten yet: 0x06056070), blown up.
const SF_GRACE: i16 = 60;
const SF_LATE: u32 = 0x0605_6070;
const RANK: u32 = 0x0600_ed84;
/// Its place: out along its angle (its turn from the parent's, and 0x6400 more), and 90 dots to
/// the side.
const SF_ANGLE_ON: u16 = 0x6400;
const SF_SIDE: i32 = 90;
/// Its shot (bullet_set's): while on the screen it waits (8 - the rank) 20ths of a second, then
/// fires at the nearer player (flags 2, speed 15/16).
const SF_SHOT_FLAGS: u16 = 2;
const SF_SHOT_SPEED: u32 = 0xf000;
const S_FLAGS: u32 = 0x2e;
const S_SPEED: u32 = 0x30;

pub fn spiral_flier_behaviour(cpu: &mut Cpu, o: u32) {
    let t = cpu.stack();
    let count = cpu.read16(o + SF_COUNT).wrapping_add(1);
    cpu.write16(o + SF_COUNT, count);
    let alive = cpu.read32(o + LIFE) as i32 > 0;
    if alive && (count as i16 > SF_GRACE && nested!(cpu, 0x0605_6a24, aim::off_screen(cpu, o + 0x10)) as u8 != 0) {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    if !alive {
        if cpu.read8(RANK) as i8 > 2 {
            cpu.call(SF_LATE, &[o, 0]);
        }
        nested!(cpu, 0x0605_9e4c, ground::blown_up(cpu, o));
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    let frame = cpu.read8(o + SPRITE_FRAME).wrapping_add(1) & 0x1f;
    cpu.write8(o + SPRITE_FRAME, frame);
    let parent = cpu.read32(o + SF_PARENT);
    let a = cpu.read16(o + SF_TURN).wrapping_add(SF_ANGLE_ON).wrapping_add(cpu.read16(parent + ANGLE));
    cpu.write16(o + ANGLE, a);
    let a = cpu.read16(o + ANGLE) as i16 as i32 as u32;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a)) as i32;
    let r = cpu.read16(o + SF_OUT) as i16 as i32;
    cpu.write32(o + POSITION, s.wrapping_mul(r) as u32);
    let a = cpu.read16(o + ANGLE) as i16 as i32 as u32;
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a)) as i32;
    let r = cpu.read16(o + SF_OUT) as i16 as i32;
    cpu.write32(o + POSITION + 4, c.wrapping_mul(r).wrapping_neg() as u32);
    let a = (cpu.read16(o + ANGLE).wrapping_add(0x4000)) as i16 as i32 as u32;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a)) as i32;
    let x = cpu.read32(o + POSITION).wrapping_add(s.wrapping_mul(SF_SIDE) as u32);
    cpu.write32(o + POSITION, x);
    let a = (cpu.read16(o + ANGLE).wrapping_add(0x4000)) as i16 as i32 as u32;
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a)) as i32;
    let y = cpu.read32(o + POSITION + 4).wrapping_sub(c.wrapping_mul(SF_SIDE) as u32);
    cpu.write32(o + POSITION + 4, y);
    let out = cpu.read16(o + SF_OUT).wrapping_add(2);
    cpu.write16(o + SF_OUT, out);
    if cpu.read8(o + HIT_WEAPON) == 1 && nested!(cpu, 0x0605_6a24, aim::off_screen(cpu, o + 0x10)) as u8 == 0 {
        let w = cpu.read16(o + SF_WAIT).wrapping_sub(1);
        cpu.write16(o + SF_WAIT, w);
    }
    if (cpu.read16(o + SF_WAIT) as i16) < 0 {
        let rank = cpu.read8(RANK);
        cpu.write16(o + SF_WAIT, (8u8.wrapping_sub(rank) as i8 as i16).wrapping_mul(20) as u16);
        let n = cpu.read8(o + NUMBER);
        cpu.write8(t + S_NUMBER, n);
        for k in [0, 4, 8] {
            let v = cpu.read32(o + POSITION + k);
            cpu.write32(t + S_POSITION + k, v);
        }
        let player = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + 0x10));
        let a = nested!(cpu, 0x0604_5e30, aim::angle_to(cpu, o, player));
        cpu.write16(t + S_ANGLE, a as u16);
        cpu.write16(t + S_FLAGS, SF_SHOT_FLAGS);
        cpu.write32(t + S_SPEED, SF_SHOT_SPEED);
        nested!(cpu, 0x0605_3db0, bullets::bullet_set(cpu, t));
    }
}

/// The spiral flier made from the description `d` (ground.rs's; its parent at +0x3c, its turn
/// +0x34), numbered from 2 if one is free: its sprite (kind 6, its colour by its type), 200 life,
/// its weapon byte the type's low byte.
const SF_SIZE: u32 = 0xbc;
const SF_BEHAVIOUR: u32 = 0x0608_6d3c;
const SF_KIND: u8 = 6;
const SF_ANIMATION: u32 = 0x0607_3278;
const SF_LIFE: u32 = 200;
const SF_START_OUT: u16 = 0xff40;
const D_PARENT: u32 = 0x3c;

pub fn spiral_flier_set(cpu: &mut Cpu, d: u32) {
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, 2)) as i8;
    if n == -1 {
        return;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, SF_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, NUMBERED_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + 0x10));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HITS_AT));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + BEHAVIOUR, SF_BEHAVIOUR);
    cpu.write8(o + NUMBER, n as u8);
    let k = cpu.read8(o + NUMBER) as i8 as i32 as u32;
    cpu.write32(NUMBERED.wrapping_add(k << 2), o);
    cpu.write8(o + SPRITE_KIND, SF_KIND);
    cpu.write8(o + SPRITE_FRAME, 0);
    cpu.write32(o + M_ANIMATION, SF_ANIMATION);
    cpu.write32(o + LIFE, SF_LIFE);
    let colour = (cpu.read16(d + D_TYPE) << 4).wrapping_add(cpu.read16(kind(SF_KIND as u32) + K_COLOUR));
    cpu.write16(o + COLOUR, colour);
    cpu.write16(o + OWN_COLOUR, colour);
    cpu.write8(o + SIDE, 1);
    let w = cpu.read8(d + D_TYPE + 1);
    cpu.write8(o + HIT_WEAPON, w);
    let (x, y) = (cpu.read32(d + D_PLACE), cpu.read32(d + D_PLACE + 4));
    cpu.write32(o + POSITION, x);
    cpu.write32(o + POSITION + 4, y);
    cpu.write32(o + POSITION + 8, 0);
    let parent = cpu.read32(d + D_PARENT);
    cpu.write32(o + SF_PARENT, parent);
    cpu.write16(o + SF_WAIT, 10);
    let turn = cpu.read16(d + D_34);
    cpu.write16(o + SF_TURN, turn);
    cpu.write16(o + SF_OUT, SF_START_OUT);
}

// ---- the electric arcs --------------------------------------------------------------------------

/// An arc's spark (an actor, one of 16 along an arc between two points): its owner (+0x6c), the
/// two points (pointers to x, y, z: +0x70 the far end, +0x74 the near), its index along (a word,
/// +0x78), its size (16.16, +0x7c), the bit of its owner's step it goes out with (+0x80).
const ARC_OWNER: u32 = 0x6c;
const ARC_FROM: u32 = 0x70;
const ARC_TO: u32 = 0x74;
const ARC_INDEX: u32 = 0x78;
const ARC_SIZE: u32 = 0x7c;
const ARC_MASK: u32 = 0x80;
const ARC_SPARKS: u16 = 16;
/// It grows to full size (by 1/128 a frame), shrinks (by 1/64) and ends while its owner's step
/// has its bit; its scale by its place along (smaller near the ends) and at random; hidden and
/// unhit (and dimmer) while small or far off in depth; every other one flickers.
const ARC_GROW: u32 = 0x200;
const ARC_SHRINK: u32 = 0x400;
const ARC_SCALES: [u32; 3] = [0x3000, 0x4000, 0x5400];
const ARC_SCALE_MID: u32 = 0x6000;
const ARC_SMALL: i32 = 0xbfff;
const ARC_DEPTH: u32 = 0x8_0000;
const ARC_HIDE: u32 = 0x8_0100;
const ARC_FRAME: u8 = 54;
const ARC_COLOURS: (u16, u16, u16) = (0x40, 0x20, 0x30);
const CHAR_BASE_0: u32 = 0x0607_41fc;

/// The arc's spark's behaviour.
pub fn arc_spark_behaviour(cpu: &mut Cpu, o: u32) {
    let owner = cpu.read32(o + ARC_OWNER);
    if cpu.read8(owner + STEP) & cpu.read8(o + ARC_MASK) == 0 {
        let s = cpu.read32(o + ARC_SIZE);
        let s = nested!(cpu, 0x0604_6168, actors::approach(cpu, s, 0x1_0000, ARC_GROW));
        cpu.write32(o + ARC_SIZE, s);
    } else {
        let s = cpu.read32(o + ARC_SIZE);
        let s = nested!(cpu, 0x0604_6168, actors::approach(cpu, s, 0, ARC_SHRINK));
        cpu.write32(o + ARC_SIZE, s);
        if s == 0 {
            cpu.write8(o + OBJ_MODE, TO_STOP);
            return;
        }
    }
    let index = cpu.read16(o + ARC_INDEX);
    let base = match index {
        0 | 15 => ARC_SCALES[0],
        1 | 14 => ARC_SCALES[1],
        2 | 13 => ARC_SCALES[2],
        _ => ARC_SCALE_MID,
    };
    let size = cpu.read32(o + ARC_SIZE);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1fff));
    let s = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, (base * 2).wrapping_add(r), size));
    cpu.write32(o + SCALES, s);
    cpu.write32(o + SCALES + 4, s);
    let a = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write16(o + ANGLE, a as u16);
    let (from, to) = (cpu.read32(o + ARC_FROM), cpu.read32(o + ARC_TO));
    for k in [0, 4, 8] {
        let (f, t) = (cpu.read32(from + k), cpu.read32(to + k));
        let step = (f.wrapping_sub(t) as i32 / 15) as u32;
        let index = cpu.read16(o + ARC_INDEX) as u32;
        cpu.write32(o + POSITION + k, step.wrapping_mul(index).wrapping_add(t));
    }
    let z = cpu.read32(o + POSITION + 8);
    let colour = if (cpu.read32(o + ARC_SIZE) as i32) <= ARC_SMALL || z.wrapping_add(ARC_DEPTH) > 0x10_0000 {
        let flags = cpu.read32(o + FLAGS) | ARC_HIDE;
        cpu.write32(o + FLAGS, flags);
        cpu.read16(CHAR_BASE_0).wrapping_add(ARC_COLOURS.0)
    } else {
        let flags = cpu.read32(o + FLAGS) & !ARC_HIDE;
        cpu.write32(o + FLAGS, flags);
        let odd = cpu.read32(GAME_FRAMES) & 1 != 0;
        cpu.read16(CHAR_BASE_0).wrapping_add(if odd { ARC_COLOURS.1 } else { ARC_COLOURS.2 })
    };
    cpu.write16(o + OWN_COLOUR, colour);
    if cpu.read32(GAME_FRAMES) & 1 == (cpu.read16(o + ARC_INDEX) & 1) as u32 {
        let flags = cpu.read32(o + FLAGS) | NO_HITS;
        cpu.write32(o + FLAGS, flags);
    }
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write8(o + SPRITE_FRAME, (((r & 0x10) >> 4) as u8).wrapping_add(ARC_FRAME));
    let colour = cpu.read16(o + OWN_COLOUR);
    cpu.write16(o + COLOUR, colour);
}

/// An arc's spark made (an actor of 0xbc bytes, kind 3's frame 40), between `from` and `to`.
const ARC_SPARK_SIZE: u32 = 0xbc;
const ARC_SPARK: u32 = 0x0608_701c;
const ARC_SPARK_ANIMATION: u32 = 0x060a_3410;

pub fn arc_spark_set(cpu: &mut Cpu, owner: u32, from: u32, to: u32, index: u32, mask: u32) {
    let index = index as u16;
    let mask = mask as u8;
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, ARC_SPARK_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + 0x10));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HITS_AT));
    cpu.write32(o + ARC_OWNER, owner);
    cpu.write32(o + ARC_FROM, from);
    cpu.write32(o + ARC_TO, to);
    cpu.write16(o + ARC_INDEX, index);
    cpu.write32(o + ARC_SIZE, 0);
    cpu.write8(o + ARC_MASK, mask);
    let n = cpu.read8(owner + NUMBER);
    cpu.write8(o + NUMBER, n);
    cpu.write32(o + BEHAVIOUR, ARC_SPARK);
    cpu.write32(o + FLAGS, FLAG_400);
    for k in [0, 4, 8] {
        cpu.write32(o + POSITION + k, 0);
    }
    cpu.write8(o + BYTE_33, 8);
    cpu.write8(o + SPRITE_FRAME, 0x28);
    cpu.write8(o + SPRITE_KIND, 3);
    let colour = cpu.read16(CHAR_BASE_0).wrapping_add(ARC_COLOURS.2);
    cpu.write16(o + OWN_COLOUR, colour);
    cpu.write16(o + COLOUR, colour);
    cpu.write32(o + M_ANIMATION, ARC_SPARK_ANIMATION);
    cpu.write8(o + SIDE, 1);
    cpu.write8(o + HIT_WEAPON, 5);
}

/// An arc's 16 sparks, between the owner's two points at `from` and `to`, going out with `mask`.
fn arc_set(cpu: &mut Cpu, o: u32, from: u32, to: u32, mask: u32) {
    static PROLOGUE: std::sync::OnceLock<super::Prologue> = std::sync::OnceLock::new();
    let mut i: i8 = 0;
    while i < ARC_SPARKS as i8 {
        let index = i as i32 as u32 & 0xffff;
        cpu.nested_with_stack(&PROLOGUE, 0x0608_7258, &[mask], |cpu| arc_spark_set(cpu, o, o + from, o + to, index, mask));
        i = i.wrapping_add(1);
    }
}

/// The two arcs of the generator (the third of its pieces): between +0x314 and +0x320 (going out
/// with its step's bit 7), and +0x32c and +0x338 (bit 6).
pub fn arc_a_set(cpu: &mut Cpu, o: u32) {
    arc_set(cpu, o, 0x314, 0x320, 0x80);
}

pub fn arc_b_set(cpu: &mut Cpu, o: u32) {
    arc_set(cpu, o, 0x32c, 0x338, 0x40);
}

// ---- the great ship --------------------------------------------------------------------------------

/// The great ship (a big model actor that passes over the stage): its step (a byte, +0x6c: the
/// table at 0x060a3418), the flags its arcs go out by (+0x6d: bit 7 the first, bit 6 the
/// second), its speed down and back (+0x70, +0x74), its count (a word, +0x78); its parts' turns
/// and their eased speeds (words, +0x2e2 to +0x308), two depths (+0x30c, +0x310).
const GS_STEP: u32 = 0x6c;
const GS_ARCS: u32 = 0x6d;
const GS_SPEED_Y: u32 = 0x70;
const GS_SPEED_Z: u32 = 0x74;
const GS_COUNT: u32 = 0x78;
const GS_STEPS: u32 = 0x060a_3418;
/// The parts' words the steps move (their meaning by the model).
const GS_2E2: u32 = 0x2e2;
const GS_2E4: u32 = 0x2e4;
const GS_2E6: u32 = 0x2e6;
const GS_2E8: u32 = 0x2e8;
const GS_2EA: u32 = 0x2ea;
const GS_2EC: u32 = 0x2ec;
const GS_2EE: u32 = 0x2ee;
const GS_2F0: u32 = 0x2f0;
const GS_2F6: u32 = 0x2f6;
const GS_2F8: u32 = 0x2f8;
const GS_2FE: u32 = 0x2fe;
const GS_304: u32 = 0x304;
const GS_DEPTH_A: u32 = 0x30c;
const GS_DEPTH_B: u32 = 0x310;
/// Its four eased turns (aim::turn_toward's): the angle at `angle`, its speed at `angle + 2`
/// (both of the ship), turned from the word at `from` toward the word at `to`, at most `most`
/// (eased by 32, 1 a frame); each turned word then on by its speed.
const GS_TURNS: [(u32, u32, u32, u32); 4] =
    [(0x300, TURN_X, 0x2fe, 0x780), (0x306, TURN_Z, 0x304, 0x780), (0x2f2, 0x2ee, 0x2f0, 0xa00), (0x2fa, 0x2f6, 0x2f8, 0xa00)];

/// The ship's turns eased (each step's first).
pub fn great_ship_turns(cpu: &mut Cpu, o: u32) {
    let count = cpu.read16(o + GS_COUNT).wrapping_add(1);
    cpu.write16(o + GS_COUNT, count);
    let t = cpu.stack();
    cpu.write32(t, GS_TURNS[0].0);
    for (k, &(speed, turned, to, most)) in GS_TURNS.iter().enumerate() {
        if k == 1 {
            cpu.write32(t + 4, o + TURN_Z);
            cpu.write32(t + 8, speed);
        }
        let from = cpu.read16(o + turned) as u32;
        let goal = cpu.read16(o + to) as u32;
        cpu.push(&[most, 0x20, 1]);
        nested!(cpu, 0x0605_70d8, aim::turn_toward(cpu, o + speed, o + speed + 2, from, goal, most, 0x20, 1));
        if k == 2 {
            cpu.pop(36);
        }
    }
    for &(speed, turned, _, _) in &GS_TURNS {
        let v = cpu.read16(o + turned).wrapping_add(cpu.read16(o + speed));
        cpu.write16(o + turned, v);
    }
}

/// Step 0: it comes down (its speed easing to 1/256 dot by 1/... a frame) until at the middle
/// of the screen's depth, then on to step 1 (sound 91).
const GS_COME_SPEED: u32 = 0x80;
const GS_COME_ACCEL: u32 = 0x310;
const GS_COME_FROM: i32 = 0xfff0_0000u32 as i32;
const GS_SOUND: u32 = 91;

pub fn great_ship_come(cpu: &mut Cpu, o: u32) {
    if cpu.read32(o + POSITION + 4) as i32 >= GS_COME_FROM {
        let v = cpu.read32(o + GS_SPEED_Y);
        let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, GS_COME_SPEED, GS_COME_ACCEL));
        cpu.write32(o + GS_SPEED_Y, v);
    }
    let v = cpu.read32(o + GS_SPEED_Y);
    cpu.write32(o + GS_SPEED_Z, v);
    if cpu.read32(o + POSITION + 4) as i32 >= 0 {
        let s = cpu.read8(o + GS_STEP).wrapping_add(1);
        cpu.write8(o + GS_STEP, s);
        for at in [POSITION + 4, POSITION + 8, GS_SPEED_Y, GS_SPEED_Z] {
            cpu.write32(o + at, 0);
        }
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, GS_SOUND));
    }
}

/// Step 1: its parts open (until the +0x2ee word comes round to 0), then step 2 with its two
/// arcs.
pub fn great_ship_open(cpu: &mut Cpu, o: u32) {
    let v = cpu.read16(o + GS_2E4).wrapping_sub(0x40);
    cpu.write16(o + GS_2E4, v);
    cpu.write16(o + GS_2EA, v);
    let d = cpu.read32(o + GS_DEPTH_A).wrapping_add(0x4000);
    cpu.write32(o + GS_DEPTH_A, d);
    for at in [GS_2EE, GS_2F6] {
        let v = cpu.read16(o + at).wrapping_add(0x20);
        cpu.write16(o + at, v);
    }
    let v = cpu.read16(o + TURN_X).wrapping_add(0x40);
    cpu.write16(o + TURN_X, v);
    if cpu.read16(o + GS_2EE) == 0 {
        cpu.write8(o + GS_STEP, 2);
        cpu.write16(o + GS_COUNT, 0);
        nested!(cpu, 0x0608_7350, arc_a_set(cpu, o));
        nested!(cpu, 0x0608_73a0, arc_b_set(cpu, o));
    }
}

/// Step 2: it fights by its count (in frames): it moves down and back, its parts swing, spiral
/// fliers leave it every 16 frames (from 0x565 to 0x924), its arcs go out and are made again (at
/// the lowest ranks); from 0xa51 it closes and leaves (step 3).
const GS_FLIER_OUT: u32 = 0xc0;
const GS_FLIER_TURN: u16 = 0xc800;

pub fn great_ship_fight(cpu: &mut Cpu, o: u32) {
    let t = cpu.stack();
    nested!(cpu, 0x0608_73ec, great_ship_turns(cpu, o));
    if cpu.read16(o + GS_2E2) == 0xc000 {
        cpu.write8(o + GS_STEP, 3);
        let a = cpu.read8(o + GS_ARCS) | 0xc0;
        cpu.write8(o + GS_ARCS, a);
        return;
    }
    let count = |cpu: &mut Cpu| cpu.read16(o + GS_COUNT) as i16;
    if count(cpu) > 0xa50 {
        let v = cpu.read16(o + GS_2E2).wrapping_add(0x80);
        cpu.write16(o + GS_2E2, v);
        cpu.write16(o + GS_2E8, v.wrapping_neg());
        let d = cpu.read32(o + GS_DEPTH_B);
        let d = nested!(cpu, 0x0604_6168, actors::approach(cpu, d, 0, 0x3800));
        cpu.write32(o + GS_DEPTH_B, d);
        let d = cpu.read32(o + GS_DEPTH_A);
        let d = nested!(cpu, 0x0604_6168, actors::approach(cpu, d, 0x4c_0000, 0x4000));
        cpu.write32(o + GS_DEPTH_A, d);
        cpu.write16(o + GS_304, 0x4000);
        return;
    }
    if count(cpu) > 0x99c {
        cpu.write16(o + GS_2F0, 0);
        cpu.write16(o + GS_2F8, 0);
        cpu.write16(o + GS_2FE, 0xc000);
    }
    if count(cpu) == 0x960 {
        cpu.write16(o + GS_2FE, 0xc000);
    }
    if count(cpu) > 0x924 {
        cpu.write16(o + GS_304, 0);
        let d = cpu.read32(o + GS_DEPTH_B);
        let d = nested!(cpu, 0x0604_6168, actors::approach(cpu, d, 0, 0x3800));
        cpu.write32(o + GS_DEPTH_B, d);
        return;
    }
    if count(cpu) > 0x564 {
        let a = cpu.read16(o + TURN_Z).wrapping_add(0xf800);
        cpu.write16(o + GS_304, a);
        if cpu.read8(RANK) as i8 <= 2 {
            if count(cpu) == 0x618 {
                let a = cpu.read8(o + GS_ARCS) | 0x80;
                cpu.write8(o + GS_ARCS, a);
            }
            if count(cpu) == 0x744 {
                let a = (cpu.read8(o + GS_ARCS) | 0x40) & 0x7f;
                cpu.write8(o + GS_ARCS, a);
                nested!(cpu, 0x0608_7350, arc_a_set(cpu, o));
            }
            if count(cpu) == 0x870 {
                let a = cpu.read8(o + GS_ARCS) & 0xbf;
                cpu.write8(o + GS_ARCS, a);
                nested!(cpu, 0x0608_73a0, arc_b_set(cpu, o));
            }
        }
        if cpu.read16(o + GS_COUNT) & 0xf == 0 {
            cpu.write32(t + D_PARENT, o);
            let late = count(cpu) > 0x744;
            if cpu.read16(o + GS_COUNT) & 0x10 == 0 {
                cpu.write16(t + D_34, 0x8000);
                cpu.write16(t + D_TYPE, late as u16);
            } else {
                cpu.write16(t + D_34, 0);
                cpu.write16(t + D_TYPE, !late as u16);
            }
            let a = (cpu.read16(o + TURN_Z).wrapping_add(GS_FLIER_TURN)) as i16 as i32 as u32;
            let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
            cpu.write32(t + D_PLACE, s.wrapping_mul(GS_FLIER_OUT));
            let a = (cpu.read16(o + TURN_Z).wrapping_add(GS_FLIER_TURN)) as i16 as i32 as u32;
            let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
            cpu.write32(t + D_PLACE + 4, c.wrapping_mul(GS_FLIER_OUT.wrapping_neg()));
            nested!(cpu, 0x0608_6f00, spiral_flier_set(cpu, t));
        }
    }
    let quarter = |cpu: &mut Cpu, v: u32| {
        let q = ((v as i32).wrapping_neg() / 4) as u16;
        cpu.write16(o + GS_2E6, q);
        cpu.write16(o + GS_2EC, q);
    };
    let c = count(cpu);
    if c > 0x4b0 {
        let d = cpu.read32(o + GS_DEPTH_A);
        let d = nested!(cpu, 0x0604_6168, actors::approach(cpu, d, 0x38_0000, 0x3000));
        cpu.write32(o + GS_DEPTH_A, d);
        let d = cpu.read32(o + GS_DEPTH_B);
        let d = nested!(cpu, 0x0604_6168, actors::approach(cpu, d, 0x40_0000, 0x3000));
        cpu.write32(o + GS_DEPTH_B, d);
        cpu.write16(o + GS_2F0, 0xe400);
        cpu.write16(o + GS_2F8, 0xe400);
    } else if c > 0x384 {
        let v = cpu.read32(o + GS_SPEED_Y);
        let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0, 0x100));
        cpu.write32(o + GS_SPEED_Y, v);
        if v == 0 {
            let y = cpu.read32(o + POSITION + 4);
            let y = nested!(cpu, 0x0604_6168, actors::approach(cpu, y, 0, 0x2000));
            cpu.write32(o + POSITION + 4, y);
        }
        let v = cpu.read32(o + GS_SPEED_Y);
        quarter(cpu, v);
    } else if c > 0x1e0 {
        let v = cpu.read32(o + GS_SPEED_Y);
        let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0xffff_ce00, 0x100));
        cpu.write32(o + GS_SPEED_Y, v);
        quarter(cpu, v);
        cpu.write16(o + GS_2F0, 0xd200);
        cpu.write16(o + GS_2F8, 0xd200);
    } else if c > 0xf0 {
        let v = cpu.read32(o + GS_SPEED_Y);
        let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0x3200, 0x100));
        cpu.write32(o + GS_SPEED_Y, v);
        quarter(cpu, v);
    } else if c <= 119 {
        cpu.write16(o + GS_2F0, 0xda00);
        cpu.write16(o + GS_2F8, 0xda00);
        let d = cpu.read32(o + GS_DEPTH_A).wrapping_sub(0x3000);
        cpu.write32(o + GS_DEPTH_A, d);
    }
}

/// Step 3: it leaves (back, faster), its parts closing; past 128 dots back it ends.
const GS_LEAVE_SPEED: u32 = 0xffff_6000;
const GS_LEAVE_ACCEL: u32 = 0x800;
const GS_GONE_Z: i32 = 0xff80_0000u32 as i32;

pub fn great_ship_leave(cpu: &mut Cpu, o: u32) {
    let v = cpu.read32(o + GS_SPEED_Z);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, GS_LEAVE_SPEED, GS_LEAVE_ACCEL));
    cpu.write32(o + GS_SPEED_Z, v);
    let a = cpu.read16(o + GS_2E4).wrapping_add(0x50);
    cpu.write16(o + GS_2E4, a);
    let b = cpu.read16(o + GS_2EA).wrapping_sub(0x50);
    cpu.write16(o + GS_2EA, b);
    if (cpu.read32(o + POSITION + 8) as i32) < GS_GONE_Z {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        cpu.write16(o + ACTION_WORD, 0x40);
    }
}

/// The ship's step (by its table), then its speeds added to its place.
pub fn great_ship_step(cpu: &mut Cpu, o: u32) {
    match cpu.read8(o + GS_STEP) {
        0 => nested!(cpu, 0x0608_751c, great_ship_come(cpu, o)),
        1 => nested!(cpu, 0x0608_758c, great_ship_open(cpu, o)),
        2 => nested!(cpu, 0x0608_760c, great_ship_fight(cpu, o)),
        3 => nested!(cpu, 0x0608_7940, great_ship_leave(cpu, o)),
        s => {
            let f = cpu.read32(GS_STEPS + 4 * s as u32);
            cpu.call(f, &[o]);
        }
    }
    let y = cpu.read32(o + POSITION + 4).wrapping_add(cpu.read32(o + GS_SPEED_Y));
    cpu.write32(o + POSITION + 4, y);
    let z = cpu.read32(o + POSITION + 8).wrapping_add(cpu.read32(o + GS_SPEED_Z));
    cpu.write32(o + POSITION + 8, z);
}

/// SGL's matrix stack and its uses (called as the SH-2 code calls them).
const SL_PUSH_MATRIX: u32 = 0x0606_71cc;
const SL_POP_MATRIX: u32 = 0x0606_7200;
const SL_TRANSLATE: u32 = 0x0606_721c;
const SL_ROT_X: u32 = 0x0606_72ec;
const SL_ROT_Y: u32 = 0x0606_736c;
const SL_ROT_Z: u32 = 0x0606_726c;
const SL_SCALE: u32 = 0x0606_73ec;
const SL_PUT_POLYGON: u32 = 0x0606_745c;
const SL_CALC_POINT: u32 = 0x0606_db2c;
/// The great ship's models (kind 52's: 0 the arc's post, 1 the arm, 2 the forearm, 3 the
/// other forearm) and the points of each (x, y, z words, 8 bytes each) worked out into the ship
/// (12 bytes each) as they are drawn.
const GS_KIND: u32 = 52;
const GS_ARM_POINTS: (u32, u32) = (0x060a_3380, 5);
const GS_FORE_POINTS: (u32, u32) = (0x060a_33a8, 3);
const GS_FORE_B_POINTS: (u32, u32) = (0x060a_33c0, 3);
const GS_POST_POINTS: (u32, u32) = (0x060a_33d8, 7);
/// Each arm's posts' tips (the arcs' ends), and how far into the screen they are put after.
const GS_TIP: u32 = 0xfff4_0000;
const GS_TIPS_IN: u32 = 0xb0_0000;

/// Kind 52's model `n` put.
fn gs_put(cpu: &mut Cpu, n: u32) {
    let file = cpu.read32(kind(GS_KIND));
    let models = cpu.read32(file + F_MODELS);
    let pair = cpu.read32(models + 4 * n);
    let model = cpu.read32(pair);
    cpu.call(SL_PUT_POLYGON, &[model]);
}

/// The points of a table worked out (each 12 bytes on from `at`).
fn gs_points(cpu: &mut Cpu, (table, count): (u32, u32), at: u32) {
    for i in 0..count {
        let p = table + 8 * i;
        let [x, y, z] = [0, 2, 4].map(|k| (cpu.read16(p + k) as i16 as i32 as u32) << 16);
        cpu.call(SL_CALC_POINT, &[x, y, z, at + 12 * i]);
    }
}

/// One arm (the two drawn mirror images of each other): its place (`y` the second word's sign
/// by the side), its turns (`turns`: x, y, z words), its forearms' swing (the word at `swing`);
/// its points at `at` (the arm, forearm, post, other forearm and other post's), the posts' tips
/// at `tips`.
struct Arm {
    flip: bool,
    turns: u32,
    swing: u32,
    at: [u32; 5],
    tips: [u32; 2],
}

const GS_ARMS: [Arm; 2] = [
    Arm { flip: false, turns: 0x2e2, swing: 0x2ee, at: [0x84, 0xc0, 0x108, 0xe4, 0x15c], tips: [0x314, 0x32c] },
    Arm { flip: true, turns: 0x2e8, swing: 0x2f6, at: [0x1b0, 0x1ec, 0x234, 0x210, 0x288], tips: [0x320, 0x338] },
];

fn gs_arm(cpu: &mut Cpu, o: u32, arm: &Arm) {
    cpu.call(SL_PUSH_MATRIX, &[]);
    let swing = cpu.read16(o + arm.swing) as i16 as i32 as u32;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, swing));
    let x = cpu.read32(o + GS_DEPTH_A);
    let y = cpu.read32(o + GS_DEPTH_B);
    let y = if arm.flip { y.wrapping_neg() } else { y };
    cpu.call(SL_TRANSLATE, &[x, y, s.wrapping_mul(69)]);
    for (k, rot) in [(0, SL_ROT_X), (2, SL_ROT_Y), (4, SL_ROT_Z)] {
        let a = cpu.read16(o + arm.turns + k) as i16 as i32 as u32;
        cpu.call(rot, &[a]);
    }
    gs_put(cpu, 1);
    gs_points(cpu, GS_ARM_POINTS, o + arm.at[0]);
    for (side, fore_at, post_at, tip) in [(1u32, arm.at[1], arm.at[2], arm.tips[0]), (0xffff_ffff, arm.at[3], arm.at[4], arm.tips[1])] {
        cpu.call(SL_PUSH_MATRIX, &[]);
        cpu.call(SL_TRANSLATE, &[0x6_0000u32.wrapping_mul(side), 0, 0]);
        let swing = cpu.read16(o + arm.swing) as i16;
        let swing = if side == 1 { swing } else { swing.wrapping_neg() };
        cpu.call(SL_ROT_X, &[swing as i32 as u32]);
        gs_put(cpu, if side == 1 { 2 } else { 3 });
        gs_points(cpu, if side == 1 { GS_FORE_POINTS } else { GS_FORE_B_POINTS }, o + fore_at);
        cpu.call(SL_PUSH_MATRIX, &[]);
        let up = if side == 1 { 0xffbb_0000 } else { 0x45_0000 };
        cpu.call(SL_TRANSLATE, &[0xfffa_0000u32.wrapping_mul(side), up, 0]);
        let frames = cpu.read32(GAME_FRAMES);
        cpu.call(SL_ROT_X, &[(frames << 10) as i16 as i32 as u32]);
        gs_put(cpu, 0);
        gs_points(cpu, GS_POST_POINTS, o + post_at);
        cpu.call(SL_CALC_POINT, &[GS_TIP, 0, 0, o + tip]);
        cpu.call(SL_POP_MATRIX, &[]);
        cpu.call(SL_POP_MATRIX, &[]);
    }
}

/// The great ship's draw (its sprite `s`, the actor 0x10 before it): in its place and turns,
/// its arm and the mirror image of it, their points worked out; the posts' tips put 176 dots
/// into the screen.
pub fn great_ship_draw(cpu: &mut Cpu, s: u32) {
    let o = s.wrapping_sub(0x10);
    cpu.call(SL_PUSH_MATRIX, &[]);
    let p = [0, 4, 8].map(|k| cpu.read32(o + POSITION + k));
    cpu.call(SL_TRANSLATE, &p);
    for (at, rot) in [(TURN_Z, SL_ROT_Z), (TURN_X, SL_ROT_X), (TURN_Y, SL_ROT_Y)] {
        let a = cpu.read16(o + at) as i16 as i32 as u32;
        cpu.call(rot, &[a]);
    }
    gs_arm(cpu, o, &GS_ARMS[0]);
    cpu.call(SL_POP_MATRIX, &[]);
    cpu.call(SL_SCALE, &[0xffff_0000, 0x1_0000, 0x1_0000]);
    gs_arm(cpu, o, &GS_ARMS[1]);
    cpu.call(SL_POP_MATRIX, &[]);
    cpu.call(SL_POP_MATRIX, &[]);
    for at in [0x328, 0x31c, 0x340, 0x334] {
        let z = cpu.read32(o + at).wrapping_sub(GS_TIPS_IN);
        cpu.write32(o + at, z);
    }
}

/// The great ship set up (its actor, numbered): its sprite and hit fields, its step and draw,
/// flags 0xc8_0000, its place (128 dots up and back), turned over (x a quarter turn back); its
/// parts' turns, speeds and depths as they start; its parts' hit points (parts.rs's parts_set:
/// each table's points into the ship, its parts' state at +0x80, their life at +0x7c); sound 90.
const GS_STEP_FN: u32 = 0x0608_79a4;
const GS_DRAW: u32 = 0x0608_79e4;
const GS_FLAGS: u32 = 0xc8_0000;
const GS_START: u32 = 0xff80_0000;
const GS_PARTS_STATE: u32 = 0x80;
const GS_PARTS_LIFE: u32 = 0x7c;
const GS_START_SOUND: u32 = 90;
const MODEL_DRAW: u32 = 0x34;
/// Its words set as it starts (the rest of +0x2e2-0x308 and +0x2dc, +0x2de cleared).
const GS_START_WORDS: [(u32, u16); 6] = [(0x2c, 0xc000), (0x2ee, 0xe000), (0x2f6, 0xe000), (0x2e4, 0x4000), (0x2ea, 0x4000), (0x80, 0x10)];
const GS_CLEAR_WORDS: [u32; 18] =
    [0x2fe, 0x300, 0x302, 0x304, 0x306, 0x308, 0x2f0, 0x2f2, 0x2f4, 0x2f8, 0x2fa, 0x2fc, 0x2e2, 0x2e6, 0x2e8, 0x2ec, 0x2dc, 0x2de];
/// Its parts' hit points: (count, where in the ship, table).
const GS_PART_POINTS: [(u32, u32, u32); 10] = [
    (5, 0x84, 0x060a_3380),
    (3, 0xc0, 0x060a_33a8),
    (3, 0xe4, 0x060a_33c0),
    (7, 0x108, 0x060a_33d8),
    (7, 0x15c, 0x060a_33d8),
    (5, 0x1b0, 0x060a_3380),
    (3, 0x1ec, 0x060a_33a8),
    (3, 0x210, 0x060a_33c0),
    (7, 0x234, 0x060a_33d8),
    (7, 0x288, 0x060a_33d8),
];

pub fn great_ship_set(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + 0x10));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HITS_AT));
    cpu.write32(o + BEHAVIOUR, GS_STEP_FN);
    cpu.write32(o + FLAGS, GS_FLAGS);
    cpu.write32(o + MODEL_DRAW, GS_DRAW);
    cpu.write32(o + POSITION, 0);
    cpu.write32(o + POSITION + 4, GS_START);
    cpu.write32(o + POSITION + 8, GS_START);
    for at in GS_CLEAR_WORDS {
        cpu.write16(o + at, 0);
    }
    for (at, v) in GS_START_WORDS {
        cpu.write16(o + at, v);
    }
    cpu.write8(o + GS_STEP, 0);
    cpu.write8(o + GS_ARCS, 0);
    cpu.write32(o + GS_SPEED_Y, 0xa000);
    cpu.write32(o + GS_SPEED_Z, 0xa000);
    cpu.write32(o + GS_DEPTH_A, 0x20_0000);
    cpu.write32(o + GS_DEPTH_B, 0);
    for (k, &(count, at, table)) in GS_PART_POINTS.iter().enumerate() {
        let number = cpu.read8(o + NUMBER) as u32;
        cpu.push(&[table, number]);
        nested!(cpu, 0x0605_6ea4, parts::parts_set(cpu, count, o + at, o + GS_PARTS_STATE, o + GS_PARTS_LIFE, table, number));
        if k == 3 || k == 7 {
            cpu.pop(32);
        }
    }
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, GS_START_SOUND));
}

/// The great ship made (numbered from 2): its number, or 0.
const GS_SIZE: u32 = 0x344;

pub fn great_ship_make(cpu: &mut Cpu) -> u32 {
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, 2)) as i8;
    if n == -1 {
        return 0;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, GS_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, NUMBERED_EXEC));
    if o == 0 {
        return 0;
    }
    cpu.write8(o + NUMBER, n as u8);
    let k = cpu.read8(o + NUMBER) as i8 as i32 as u32;
    cpu.write32(NUMBERED.wrapping_add(k << 2), o);
    nested!(cpu, 0x0608_8018, great_ship_set(cpu, o));
    n as i32 as u32
}

// ---- the burst mine ------------------------------------------------------------------------------

/// The burst mine (a plain enemy actor thrown out): its count (a word, +0x8c), its bursts (a
/// word, +0x8e), the next ring's angle (a word, +0x90), its growth and the growth's change
/// (words, +0x92, +0x94), its speed (+0x80, +0x84) and the speed's change (+0xac, +0xb0); its
/// scales (+0x20, +0x24).
const MN_COUNT: u32 = 0x8c;
const MN_BURSTS: u32 = 0x8e;
const MN_RING_ANGLE: u32 = 0x90;
const MN_GROW: u32 = 0x92;
const MN_GROW_CHANGE: u32 = 0x94;
const MN_SPEED: u32 = 0x80;
const MN_DRAG: u32 = 0xac;
/// Thrown for 30 frames (slowing to a stop), it then swells (flags 0x100, growing by 0x380 and
/// 0x40 less... a frame) and every 12 frames bursts into a ring of 16 pulses (sound 102), the
/// rings 0x600 on from a random start; after three, once small again, it ends. Its frame cycles
/// by the table at 0x060a3458.
const MN_THROWN: i16 = 30;
const MN_SWELL_FLAG: u32 = 0x100;
const MN_GROW_START: u16 = 0x380;
const MN_GROW_CHANGE_START: u16 = 0x40;
const MN_BURST_EVERY: u32 = 12;
const MN_RING_STEP: u16 = 0x600;
const MN_RING_SHOTS: i8 = 16;
const MN_RING_SPACING: u16 = 0x1000;
const MN_SOUND: u32 = 102;
const MN_SHOT_FLAGS: u16 = 8;
const MN_PULSE_33: u8 = 7;
const MN_GONE_SIZE: i32 = 0x1fff;
const MN_FRAMES: u32 = 0x060a_3458;

pub fn mine_behaviour(cpu: &mut Cpu, o: u32) {
    let count = cpu.read16(o + MN_COUNT).wrapping_add(1);
    cpu.write16(o + MN_COUNT, count);
    if count as i16 == MN_THROWN {
        let flags = cpu.read32(o + FLAGS) | MN_SWELL_FLAG;
        cpu.write32(o + FLAGS, flags);
        cpu.write16(o + MN_GROW, MN_GROW_START);
        cpu.write16(o + MN_GROW_CHANGE, MN_GROW_CHANGE_START);
    }
    if cpu.read16(o + MN_COUNT) as i16 > MN_THROWN {
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000)) as u16;
        cpu.write16(o + ANGLE, r & 0xc000);
        let s = cpu.read32(o + SCALES).wrapping_add(cpu.read16(o + MN_GROW) as i16 as i32 as u32);
        cpu.write32(o + SCALES, s);
        let g = cpu.read16(o + MN_GROW).wrapping_add(cpu.read16(o + MN_GROW_CHANGE));
        cpu.write16(o + MN_GROW, g);
        let c = cpu.read16(o + MN_GROW_CHANGE).wrapping_sub(2);
        cpu.write16(o + MN_GROW_CHANGE, c);
        let count = cpu.read16(o + MN_COUNT) as i16 as i32 as u32;
        if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, count, MN_BURST_EVERY)) == 0 {
            let b = cpu.read16(o + MN_BURSTS).wrapping_add(1);
            cpu.write16(o + MN_BURSTS, b);
            if b as i16 == 1 {
                let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
                cpu.write16(o + MN_RING_ANGLE, r as u16);
            } else {
                let a = cpu.read16(o + MN_RING_ANGLE).wrapping_add(MN_RING_STEP);
                cpu.write16(o + MN_RING_ANGLE, a);
            }
            if (cpu.read16(o + MN_BURSTS) as i16) <= 3 {
                let t = cpu.stack();
                let n = cpu.read8(o + NUMBER);
                cpu.write8(t + S_NUMBER, n);
                let a = cpu.read16(o + MN_RING_ANGLE);
                cpu.write16(t + S_ANGLE, a);
                cpu.write16(t + S_FLAGS, MN_SHOT_FLAGS);
                let (x, y) = (cpu.read32(o + POSITION), cpu.read32(o + POSITION + 4));
                cpu.write32(t + S_POSITION, x);
                cpu.write32(t + S_POSITION + 4, y);
                cpu.write32(t + S_POSITION + 8, 0);
                nested!(cpu, 0x0600_50d4, sound::se_set(cpu, MN_SOUND));
                let mut i: i8 = 0;
                while i < MN_RING_SHOTS {
                    let a = cpu.read16(t + S_ANGLE).wrapping_add(MN_RING_SPACING);
                    cpu.write16(t + S_ANGLE, a);
                    let p = nested!(cpu, 0x0605_4c8c, bullets::pulse_set(cpu, t));
                    if p != 0 {
                        cpu.write8(p + BYTE_33, MN_PULSE_33);
                    }
                    i = i.wrapping_add(1);
                }
            } else if (cpu.read32(o + SCALES) as i32) <= MN_GONE_SIZE {
                cpu.write8(o + OBJ_MODE, TO_STOP);
            }
        }
    } else {
        for k in [0, 4] {
            let p = cpu.read32(o + POSITION + k).wrapping_add(cpu.read32(o + MN_SPEED + k));
            cpu.write32(o + POSITION + k, p);
        }
        for k in [0, 4] {
            let v = cpu.read32(o + MN_SPEED + k).wrapping_add(cpu.read32(o + MN_DRAG + k));
            cpu.write32(o + MN_SPEED + k, v);
        }
    }
    let s = cpu.read32(o + SCALES);
    cpu.write32(o + SCALES + 4, s);
    let k = cpu.read32(GAME_FRAMES) & 3;
    let f = cpu.read8(MN_FRAMES + k);
    cpu.write8(o + SPRITE_FRAME, f);
}

/// A burst mine thrown from the shot description `d` (its place, its number +0x1e, its angle
/// +0x2c): 8 dots a frame along the angle, slowing to a stop over 25 frames; kind 0's frame 0,
/// colour 0x90 on; sound 41.
const MN_SIZE: u32 = 0xbc;
const MN_BEHAVIOUR: u32 = 0x0608_82e4;
const MN_FLAGS: u32 = 0x8_0400;
const MN_COLOUR: u16 = 0x90;
const MN_THROW: i32 = 8;
const MN_SLOW_FRAMES: u32 = 25;
const MN_SCALE: u32 = 0x4000;
const MN_THROW_SOUND: u32 = 41;

pub fn mine_set(cpu: &mut Cpu, d: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, MN_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + 0x10));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HITS_AT));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    let n = cpu.read8(d + S_NUMBER);
    cpu.write8(o + NUMBER, n);
    cpu.write32(o + BEHAVIOUR, MN_BEHAVIOUR);
    cpu.write32(o + FLAGS, MN_FLAGS);
    cpu.write8(o + SPRITE_FRAME, 0);
    cpu.write8(o + SPRITE_KIND, 0);
    cpu.write8(o + BYTE_33, 8);
    let colour = cpu.read16(CHAR_BASE_0).wrapping_add(MN_COLOUR);
    cpu.write16(o + OWN_COLOUR, colour);
    cpu.write16(o + COLOUR, colour);
    let (x, y) = (cpu.read32(d + S_POSITION), cpu.read32(d + S_POSITION + 4));
    cpu.write32(o + POSITION, x);
    cpu.write32(o + POSITION + 4, y);
    cpu.write32(o + POSITION + 8, 0);
    let a = cpu.read16(d + S_ANGLE) as i16 as i32 as u32;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a)) as i32;
    cpu.write32(o + MN_SPEED, s.wrapping_mul(MN_THROW) as u32);
    let a = cpu.read16(d + S_ANGLE) as i16 as i32 as u32;
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a)) as i32;
    cpu.write32(o + MN_SPEED + 4, c.wrapping_mul(MN_THROW).wrapping_neg() as u32);
    for k in [0, 4] {
        let v = cpu.read32(o + MN_SPEED + k).wrapping_neg();
        let q = nested!(cpu, 0x0600_6a78, math::quotient(cpu, v, MN_SLOW_FRAMES));
        cpu.write32(o + MN_DRAG + k, q);
    }
    cpu.write8(o + SIDE, 1);
    cpu.write8(o + HIT_WEAPON, 5);
    cpu.write32(o + SCALES, MN_SCALE);
    cpu.write32(o + SCALES + 4, MN_SCALE);
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, MN_THROW_SOUND));
}

/// A burst mine thrown from `o` (at its place, along its angle).
pub fn mine_throw(cpu: &mut Cpu, o: u32) {
    let d = cpu.stack();
    let (x, y) = (cpu.read32(o + POSITION), cpu.read32(o + POSITION + 4));
    cpu.write32(d + S_POSITION, x);
    cpu.write32(d + S_POSITION + 4, y);
    let n = cpu.read8(o + NUMBER);
    cpu.write8(d + S_NUMBER, n);
    let a = cpu.read16(o + ANGLE);
    cpu.write16(d + S_ANGLE, a);
    nested!(cpu, 0x0608_848c, mine_set(cpu, d));
}

// ---- the mine layer ------------------------------------------------------------------------------

/// The mine layer (a big enemy): its step (a byte, +0x7c), flags (a word, +0x7e: bit 0 hit), its
/// speed (+0x80, +0x84), its count (a word, +0xa2), its rounds (a word, +0xa4), its count once
/// shot down (+0xb0) and its fall then (+0xb8).
const ML_STEP: u32 = 0x7c;
const ML_FLAGS: u32 = 0x7e;
const ML_SPEED: u32 = 0x80;
const ML_COUNT: u32 = 0xa2;
const ML_ROUNDS: u32 = 0xa4;
const ML_AC: u32 = 0xac;
const ML_DOWN_COUNT: u32 = 0xb0;
const ML_FALL: u32 = 0xb8;
const ML_DOWN: u8 = 7;

/// Its salvos of mines by its count: step 2, one after 60 frames (3); 3, after 180 frames (4); 4,
/// one after 30 (5); 5, two more (at 180 and 420), and after 600 frames back to 2 (twice; then
/// 6).
pub fn mine_layer_salvos(cpu: &mut Cpu, o: u32) {
    let count = |cpu: &mut Cpu| cpu.read16(o + ML_COUNT) as i16;
    let throw = |cpu: &mut Cpu, next: u8| {
        cpu.write8(o + ML_STEP, next);
        cpu.write16(o + ML_COUNT, 0);
        nested!(cpu, 0x0608_85d0, mine_throw(cpu, o));
    };
    match cpu.read8(o + ML_STEP) {
        2 if count(cpu) > 60 => throw(cpu, 3),
        3 if count(cpu) > 180 => {
            cpu.write8(o + ML_STEP, 4);
            cpu.write16(o + ML_COUNT, 0);
        }
        4 if count(cpu) > 30 => throw(cpu, 5),
        5 => {
            let c = count(cpu);
            if c == 180 || c == 420 {
                nested!(cpu, 0x0608_85d0, mine_throw(cpu, o));
            }
            if count(cpu) > 600 {
                cpu.write8(o + ML_STEP, 2);
                cpu.write16(o + ML_COUNT, 0);
                let r = cpu.read16(o + ML_ROUNDS).wrapping_add(1);
                cpu.write16(o + ML_ROUNDS, r);
                if r as i16 == 2 {
                    cpu.write8(o + ML_STEP, 6);
                }
            }
        }
        _ => {}
    }
    let c = cpu.read16(o + ML_COUNT).wrapping_add(1);
    cpu.write16(o + ML_COUNT, c);
}

/// Shot down (unless already): 100 points to who hit it, a white flash fading (by 0x800).
const ML_POINTS: u32 = 100;
const ML_FLASH: u32 = 50;
const ML_FADE: u32 = 0x800;
const ML_GONE_Y: i32 = 0xd8_0000;
/// It comes in (0: 5/8 dot a frame down to 64 dots up; 1: its speed down and up out of the
/// ground until level, then hittable), then turns toward the nearer player (a 48th of the way a
/// frame; in its last round away, a half turn) and speeds (by round: to 1/8 dot by 1/64, 3/8 by
/// 1/2048, 1/16 by 1/256) laying mines. Down (7): it drifts to a stop and sinks, smoking, a
/// sound every 40 frames, its colour flickering; it flickers past 150 and ends past 180.
const ML_COME_SPEED: u32 = 0xa000;
const ML_COME_TO: i32 = 0xffc0_0000u32 as i32;
const ML_RISE: u32 = 0xfe00;
const ML_RISE_Z: u32 = 0x8000;
const ML_DOWN_SMOKE: u32 = 0x50_0000;

pub fn mine_layer_behaviour(cpu: &mut Cpu, o: u32) {
    if cpu.read8(o + ML_STEP) != ML_DOWN && cpu.read32(o + POSITION + 4) as i32 > ML_GONE_Y {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    if cpu.read8(o + ML_STEP) != ML_DOWN && cpu.read32(o + LIFE) as i32 <= 0 {
        nested!(cpu, 0x0605_61ac, hits::score_hit_by(cpu, o, ML_POINTS));
        cpu.write8(o + ML_STEP, ML_DOWN);
        let flags = cpu.read32(o + FLAGS) | NO_HITS;
        cpu.write32(o + FLAGS, flags);
        cpu.write32(o + ML_AC, 0);
        cpu.write32(o + ML_DOWN_COUNT, 0);
        cpu.write32(o + ML_FALL, 0xffff_d000);
        nested!(cpu, 0x0605_e6dc, screen::set_fade_offset(cpu, ML_FLASH, ML_FLASH, ML_FLASH));
        nested!(cpu, 0x0605_e93c, screen::fade_to(cpu, 0, 0, 0, ML_FADE));
    }
    match cpu.read8(o + ML_STEP) {
        0 => {
            cpu.write32(o + ML_SPEED + 4, ML_COME_SPEED);
            if cpu.read32(o + POSITION + 4) as i32 > ML_COME_TO {
                cpu.write8(o + ML_STEP, 1);
            }
        }
        1 => {
            let v = cpu.read32(o + ML_SPEED + 4).wrapping_add(ML_RISE | 0xffff_0000);
            cpu.write32(o + ML_SPEED + 4, v);
            let z = cpu.read32(o + POSITION + 8).wrapping_add(ML_RISE_Z | 0xffff_0000);
            cpu.write32(o + POSITION + 8, z);
            if z == 0 {
                cpu.write8(o + ML_STEP, 2);
                let flags = cpu.read32(o + FLAGS) & !NO_HITS;
                cpu.write32(o + FLAGS, flags);
            }
        }
        ML_DOWN => {
            for k in [0, 4] {
                let v = cpu.read32(o + ML_SPEED + k);
                let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0, 0x100));
                cpu.write32(o + ML_SPEED + k, v);
            }
            let f = cpu.read32(o + ML_FALL);
            let f = nested!(cpu, 0x0604_6168, actors::approach(cpu, f, 0x8000, 0x100));
            cpu.write32(o + ML_FALL, f);
            let z = cpu.read32(o + POSITION + 8).wrapping_add(f);
            cpu.write32(o + POSITION + 8, z);
            let n = cpu.read32(o + ML_DOWN_COUNT).wrapping_add(1);
            cpu.write32(o + ML_DOWN_COUNT, n);
            if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, n, 40)) == 0 {
                let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
                let se = if r & 1 == 0 { 18 } else { 17 };
                nested!(cpu, 0x0600_50d4, sound::se_set(cpu, se));
            }
            let t = cpu.stack();
            let dx = nested!(cpu, 0x0604_5d90, math::random(cpu, ML_DOWN_SMOKE));
            let x = cpu.read32(o + POSITION).wrapping_add(dx);
            cpu.write32(t, x);
            let dy = nested!(cpu, 0x0604_5d90, math::random(cpu, ML_DOWN_SMOKE));
            let y = cpu.read32(o + POSITION + 4).wrapping_add(dy);
            cpu.write32(t + 4, y);
            let z = cpu.read32(o + POSITION + 8);
            cpu.write32(t + 8, z);
            let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
            cpu.write32(t + 12, 0xc000u32.wrapping_sub((r & 7) << 11));
            nested!(cpu, 0x0605_77d0, explosions::smoke_puff(cpu, t));
            let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
            let colour = cpu.read16(CHAR_BASE_0).wrapping_add(if r & 1 == 0 { 0xb0 } else { 0x50 });
            cpu.write16(o + COLOUR, colour);
            let flags = cpu.read32(o + FLAGS) & !HIDDEN;
            cpu.write32(o + FLAGS, flags);
            let n = cpu.read32(o + ML_DOWN_COUNT) as i32;
            if n > 150 && n & 1 != 0 {
                cpu.write32(o + FLAGS, flags | HIDDEN);
            }
            if cpu.read32(o + ML_DOWN_COUNT) as i32 > 180 {
                cpu.write8(o + OBJ_MODE, TO_STOP);
            }
        }
        _ => {
            let (to, step) = match cpu.read16(o + ML_ROUNDS) as i16 {
                0 => (0x800, 0x400),
                2 => (0x6000, 0x80),
                _ => (0x1000, 0x100),
            };
            let v = cpu.read32(o + ML_SPEED + 4);
            let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, to, step));
            cpu.write32(o + ML_SPEED + 4, v);
            let goal = if cpu.read16(o + ML_ROUNDS) as i16 == 2 {
                0x8000
            } else {
                let p = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + 0x10));
                nested!(cpu, 0x0604_5e30, aim::angle_to(cpu, o, p)) & 0xffff
            };
            let a = cpu.read16(o + ANGLE) as i16 as i32 as u32;
            let d = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, a, goal)) as i32 / 48;
            let a = cpu.read16(o + ANGLE).wrapping_add(d as u16);
            cpu.write16(o + ANGLE, a);
            nested!(cpu, 0x0608_860c, mine_layer_salvos(cpu, o));
        }
    }
    if cpu.read16(o + HIT_FLAGS) & HIT_HARMED != 0 {
        let f = cpu.read16(o + ML_FLAGS) | 1;
        cpu.write16(o + ML_FLAGS, f);
    }
    for k in [0, 4] {
        let p = cpu.read32(o + POSITION + k).wrapping_add(cpu.read32(o + ML_SPEED + k));
        cpu.write32(o + POSITION + k, p);
    }
}

/// The mine layer's draw (its sprite `s`, the actor 0x10 before it): its flags' hit bit (bit 0)
/// shown for a frame (bit 1); unless hidden, its hull (kind 19's frame 0, white while hit) and
/// its turret (frame 1, at its angle) drawn from a sprite on the stack.
const SP_FLAGS: u32 = 0x00;
const SP_POSITION: u32 = 0x04;
const SP_COLOUR: u32 = 0x18;
const SP_KIND: u32 = 0x1e;
const SP_FRAME: u32 = 0x1f;
const SP_ANGLE: u32 = 0x20;
const SP_LIFT: u32 = 0x23;
const ML_KIND: u8 = 19;
const ML_HIT_COLOUR: u16 = 0xe0;
const ML_TURRET_FLAGS: u32 = 0x20;

pub fn mine_layer_draw(cpu: &mut Cpu, s: u32) {
    let o = s.wrapping_sub(0x10);
    let f = cpu.read16(o + ML_FLAGS);
    if f & 2 != 0 {
        cpu.write16(o + ML_FLAGS, f & 0xfffc);
    }
    let f = cpu.read16(o + ML_FLAGS);
    if f & 1 != 0 {
        cpu.write16(o + ML_FLAGS, f | 2);
    }
    if cpu.read32(o + FLAGS) & HIDDEN != 0 {
        return;
    }
    let t = cpu.stack();
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, t));
    cpu.write8(t + SP_KIND, ML_KIND);
    cpu.write8(t + SP_FRAME, 0);
    cpu.write8(t + SP_LIFT, 0xf7);
    for k in [0, 4, 8] {
        let v = cpu.read32(o + POSITION + k);
        cpu.write32(t + SP_POSITION + k, v);
    }
    let colour = if cpu.read16(o + ML_FLAGS) & 1 != 0 {
        cpu.read16(CHAR_BASE_0).wrapping_add(ML_HIT_COLOUR)
    } else {
        cpu.read16(o + COLOUR)
    };
    cpu.write16(t + SP_COLOUR, colour);
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, t));
    cpu.write32(t + SP_FLAGS, ML_TURRET_FLAGS);
    cpu.write8(t + SP_FRAME, 1);
    let a = cpu.read16(o + ANGLE);
    cpu.write16(t + SP_ANGLE, a);
    cpu.write8(t + SP_LIFT, 0xf8);
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, t));
}

/// The mine layer's guns (numbered enemies riding it, +0x6c): their kind (a word, +0xa6: 1 a
/// spray gun turning toward the nearer player, 0 a rocket gun swinging out), their place on the
/// layer (16.16, +0xac, +0xb0), their wait (a word, +0x8c).
const GN_LAYER: u32 = 0x6c;
const GN_KIND: u32 = 0xa6;
const GN_PLACE: u32 = 0xac;
const GN_WAIT: u32 = 0x8c;
/// The spray gun turns 0x60 a frame (straight up in the layer's last round) and, while the
/// layer is at step 3, fires every 26 frames: a plain shot 20 dots out, and by rank pairs more
/// either side (0xc00, 0x1800, 0x2400 apart).
const GN_TURN: i32 = 0x60;
const GN_SPRAY_WAIT: u16 = 26;
const GN_SPRAY_OUT: i32 = 20;
const GN_SPRAY_SPREAD: [(i8, [u16; 2]); 3] = [(0, [0xf400, 0x0c00]), (1, [0xe800, 0x1800]), (2, [0xdc00, 0x2400])];
/// The rocket gun swings outward (0x200 a frame) to straight down, all the way round while the
/// layer is at step 5 (counting down its wait), and fires a rocket (12 dots out, flags 9, speed
/// 96) when its wait is out, its wait then by rank (the table at 0x060a345c).
const GN_SWING: u16 = 0x200;
const GN_ROCKET_OUT: i32 = 12;
const GN_ROCKET_FLAGS: u16 = 9;
const GN_ROCKET_SPEED: u16 = 96;
const GN_ROCKET_WAITS: u32 = 0x060a_345c;

pub fn layer_gun_behaviour(cpu: &mut Cpu, o: u32) {
    let layer = cpu.read32(o + GN_LAYER);
    let t = cpu.stack();
    if (cpu.read32(o + LIFE) as i32) <= 0 || cpu.read8(layer + ML_STEP) == ML_DOWN || cpu.read8(layer + OBJ_MODE) as i8 > ENDING {
        if cpu.read8(layer + OBJ_MODE) as i8 <= ENDING {
            nested!(cpu, 0x0605_9f50, ground::gun_blown_off(cpu, o));
        }
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    let x = cpu.read32(layer + POSITION).wrapping_add(cpu.read32(o + GN_PLACE));
    cpu.write32(o + POSITION, x);
    let y = cpu.read32(layer + POSITION + 4).wrapping_add(cpu.read32(o + GN_PLACE + 4));
    cpu.write32(o + POSITION + 4, y);
    let z = cpu.read32(layer + POSITION + 8);
    cpu.write32(o + POSITION + 8, z);
    let flags = cpu.read32(o + FLAGS) & !NO_HITS;
    cpu.write32(o + FLAGS, flags);
    let flags = (cpu.read32(layer + FLAGS) & NO_HITS) | flags;
    cpu.write32(o + FLAGS, flags);
    if flags & NO_HITS != 0 {
        return;
    }
    let n = cpu.read8(o + NUMBER);
    cpu.write8(t + S_NUMBER, n);
    let z = cpu.read32(o + POSITION + 8);
    cpu.write32(t + S_POSITION + 8, z);
    if cpu.read16(o + GN_KIND) == 1 {
        let goal = if cpu.read16(layer + ML_ROUNDS) == 2 {
            0
        } else {
            let p = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + 0x10));
            nested!(cpu, 0x0604_5e30, aim::angle_to(cpu, o, p)) & 0xffff
        };
        let a = cpu.read16(o + ANGLE) as u32;
        let d = nested!(cpu, 0x0605_6580, aim::angle_between(cpu, goal, a)) as i16 as i32;
        if d.wrapping_abs() < GN_TURN {
            cpu.write16(o + ANGLE, goal as u16);
        } else {
            let turn = if d > 0 { -GN_TURN } else { GN_TURN };
            let a = cpu.read16(o + ANGLE).wrapping_add(turn as u16);
            cpu.write16(o + ANGLE, a);
        }
        let w = cpu.read16(o + GN_WAIT) as i16 as i32 as u32;
        let w = nested!(cpu, 0x0604_6168, actors::approach(cpu, w, 0, 1));
        cpu.write16(o + GN_WAIT, w as u16);
        if w as i16 != 0 || cpu.read8(layer + ML_STEP) != 3 {
            return;
        }
        cpu.write16(o + GN_WAIT, GN_SPRAY_WAIT);
        let a = cpu.read16(o + ANGLE) as i16 as i32 as u32;
        let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a)) as i32;
        let x = cpu.read32(o + POSITION).wrapping_add(s.wrapping_mul(GN_SPRAY_OUT) as u32);
        cpu.write32(t + S_POSITION, x);
        let a = cpu.read16(o + ANGLE) as i16 as i32 as u32;
        let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a)) as i32;
        let y = cpu.read32(o + POSITION + 4).wrapping_add(c.wrapping_neg().wrapping_mul(GN_SPRAY_OUT) as u32);
        cpu.write32(t + S_POSITION + 4, y);
        let a = cpu.read16(o + ANGLE);
        cpu.write16(t + S_ANGLE, a);
        nested!(cpu, 0x0605_3f48, bullets::bullet_set_plain(cpu, t));
        for (above, offsets) in GN_SPRAY_SPREAD {
            if cpu.read8(RANK) as i8 > above {
                for off in offsets {
                    let a = cpu.read16(o + ANGLE).wrapping_add(off);
                    cpu.write16(t + S_ANGLE, a);
                    nested!(cpu, 0x0605_3f48, bullets::bullet_set_plain(cpu, t));
                }
            }
        }
    } else {
        let out = cpu.read32(o + GN_PLACE) as i32 > 0;
        let swing = if out { GN_SWING } else { GN_SWING.wrapping_neg() };
        if cpu.read8(layer + ML_STEP) == 5 || cpu.read16(o + ANGLE) != 0x8000 {
            let a = cpu.read16(o + ANGLE).wrapping_add(swing);
            cpu.write16(o + ANGLE, a);
        }
        if cpu.read8(layer + ML_STEP) == 5 {
            let w = cpu.read16(o + GN_WAIT) as i16 as i32 as u32;
            let w = nested!(cpu, 0x0604_6168, actors::approach(cpu, w, 0, 1));
            cpu.write16(o + GN_WAIT, w as u16);
        }
        if cpu.read16(o + GN_WAIT) != 0 {
            return;
        }
        let rank = cpu.read8(RANK) as i8 as i32 as u32;
        let w = cpu.read8(GN_ROCKET_WAITS.wrapping_add(rank)) as u16;
        cpu.write16(o + GN_WAIT, w);
        let a = cpu.read16(o + ANGLE) as i16 as i32 as u32;
        let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a)) as i32;
        let x = cpu.read32(o + POSITION).wrapping_add(s.wrapping_mul(GN_ROCKET_OUT) as u32);
        cpu.write32(t + S_POSITION, x);
        let a = cpu.read16(o + ANGLE) as i16 as i32 as u32;
        let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a)) as i32;
        let y = cpu.read32(o + POSITION + 4).wrapping_add(c.wrapping_neg().wrapping_mul(GN_ROCKET_OUT) as u32);
        cpu.write32(t + S_POSITION + 4, y);
        cpu.write16(t + S_SPEED, GN_ROCKET_SPEED);
        cpu.write16(t + S_FLAGS, GN_ROCKET_FLAGS);
        let a = cpu.read16(o + ANGLE);
        cpu.write16(t + S_ANGLE, a);
        nested!(cpu, 0x0605_5b5c, bullets::rocket_set(cpu, t));
    }
}

/// A gun on the mine layer made from `d` (its kind a word at +0x2c, its place on the layer, words
/// in dots, +0x30 and +0x32), numbered from 2: kind 19's frame by its kind (0x060a3471's), its
/// animation and life by its kind (0x060a3464's, 0x060a346c's), pointing down, its first wait
/// 10 (5 on the right): 1, or 0 if it could not be made.
const GN_SIZE: u32 = 0xbc;
const GN_BEHAVIOUR: u32 = 0x0608_8b04;
const GN_FLAGS: u32 = 0x8_0020;
const GN_FRAMES: u32 = 0x060a_3471;
const GN_ANIMATIONS: u32 = 0x060a_3464;
const GN_LIVES: u32 = 0x060a_346c;
const D_GUN_KIND: u32 = 0x2c;
const D_GUN_PLACE: u32 = 0x30;

pub fn layer_gun_set(cpu: &mut Cpu, d: u32, layer: u32) -> u32 {
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, 2)) as i8;
    if n == -1 {
        return 0;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, GN_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, NUMBERED_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + 0x10));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HITS_AT));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + BEHAVIOUR, GN_BEHAVIOUR);
    cpu.write8(o + NUMBER, n as u8);
    let k = cpu.read8(o + NUMBER) as i8 as i32 as u32;
    cpu.write32(NUMBERED.wrapping_add(k << 2), o);
    cpu.write32(o + GN_LAYER, layer);
    cpu.write32(o + FLAGS, GN_FLAGS);
    cpu.write8(o + SPRITE_KIND, ML_KIND);
    let kind_n = cpu.read16(d + D_GUN_KIND) as i16 as i32 as u32;
    let f = cpu.read8(GN_FRAMES.wrapping_add(kind_n << 1));
    cpu.write8(o + SPRITE_FRAME, f);
    let colour = cpu.read16(kind(ML_KIND as u32) + K_COLOUR);
    cpu.write16(o + COLOUR, colour);
    cpu.write16(o + OWN_COLOUR, colour);
    let kind_n = cpu.read16(d + D_GUN_KIND) as i16 as i32 as u32;
    let anim = cpu.read32(GN_ANIMATIONS.wrapping_add(kind_n << 2));
    cpu.write32(o + M_ANIMATION, anim);
    let kind_n = cpu.read16(d + D_GUN_KIND) as i16 as i32 as u32;
    let life = cpu.read16(GN_LIVES.wrapping_add(kind_n << 1)) as u32;
    cpu.write32(o + LIFE, life);
    cpu.write8(o + SIDE, 1);
    cpu.write8(o + HIT_WEAPON, 0);
    cpu.write8(o + BYTE_33, 0xf8);
    cpu.write16(o + ANGLE, 0x8000);
    let k = cpu.read16(d + D_GUN_KIND);
    cpu.write16(o + GN_KIND, k);
    let px = (cpu.read16(d + D_GUN_PLACE) as i16 as i32 as u32) << 16;
    cpu.write32(o + GN_PLACE, px);
    let py = (cpu.read16(d + D_GUN_PLACE + 2) as i16 as i32 as u32) << 16;
    cpu.write32(o + GN_PLACE + 4, py);
    cpu.write16(o + GN_WAIT, 10);
    if cpu.read32(o + GN_PLACE) as i32 > 0 {
        cpu.write16(o + GN_WAIT, 5);
    }
    1
}

/// The mine layer's event (a timeline event, `e`'s +4 long its x): it made, numbered from 2,
/// 104 dots... below the screen, sunk 5 dots, 50000 life; its four guns: two spray guns (48 dots
/// either side, 2 up), two rocket guns (26 either side, 33 down). 1.
const ML_SIZE: u32 = 0xbc;
const ML_BEHAVIOUR: u32 = 0x0608_86e4;
const ML_DRAW: u32 = 0x0608_8a2c;
const ML_LIFE: u32 = 50000;
const ML_ANIMATION: u32 = 0x060a_3428;
const ML_START: (u32, u32) = (0xfee0_0000, 0x50_0000);
const ML_GUNS: [(u16, u16, u16); 4] = [(1, 0xffd0, 0xfffe), (1, 0x30, 0xfffe), (0, 0xffe6, 0x21), (0, 0x1a, 0x21)];

pub fn mine_layer_event(cpu: &mut Cpu, e: u32) -> u32 {
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, 2)) as i8;
    if n == -1 {
        return 1;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, ML_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, NUMBERED_EXEC));
    if o == 0 {
        return 1;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + 0x10));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HITS_AT));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + BEHAVIOUR, ML_BEHAVIOUR);
    cpu.write32(o + FLAGS, GS_FLAGS);
    cpu.write32(o + MODEL_DRAW, ML_DRAW);
    cpu.write8(o + SPRITE_KIND, ML_KIND);
    cpu.write8(o + NUMBER, n as u8);
    let k = cpu.read8(o + NUMBER) as i8 as i32 as u32;
    cpu.write32(NUMBERED.wrapping_add(k << 2), o);
    let colour = cpu.read16(kind(ML_KIND as u32) + K_COLOUR);
    cpu.write16(o + COLOUR, colour);
    cpu.write32(o + LIFE, ML_LIFE);
    cpu.write32(o + M_ANIMATION, ML_ANIMATION);
    cpu.write8(o + SIDE, 1);
    cpu.write8(o + HIT_WEAPON, 0);
    let x = cpu.read32(e + 4);
    cpu.write32(o + POSITION, x);
    cpu.write32(o + POSITION + 4, ML_START.0);
    cpu.write32(o + POSITION + 8, ML_START.1);
    cpu.write32(o + ML_SPEED, 0);
    cpu.write32(o + ML_SPEED + 4, 0);
    let d = cpu.stack();
    for (i, (kind_n, px, py)) in ML_GUNS.into_iter().enumerate() {
        if i == 0 || i == 2 {
            cpu.write16(d + D_GUN_KIND, kind_n);
            cpu.write16(d + D_GUN_PLACE + 2, py);
        }
        cpu.write16(d + D_GUN_PLACE, px);
        nested!(cpu, 0x0608_8e18, layer_gun_set(cpu, d, o));
    }
    1
}

// ---- the ring carriers -----------------------------------------------------------------------------

/// A ring carrier (enemy_row's kind 128, a carrier (ground.rs's) given this behaviour): its
/// step (a byte, +0x7c), a word (+0x7e: 16 while near the screen), its angle round the ring (a
/// word, RC_ANGLE) and the ring's radius (RC_SPEED) and turn a frame (RC_A2), the ring's place
/// down (16.16, RC_AC: from 320 dots up, falling faster by 7/16 dot a frame each frame).
const RC_STEP: u32 = 0x7c;
const RC_NEAR: u32 = 0x7e;
const RC_NEAR_SPAN: u32 = 0x120_0000;
const RC_NEAR_FROM: u32 = 0x90_0000;
const RC_START: u32 = 0xfec0_0000;
const RC_FALL: u32 = 0x7000;
const RC_GONE_Y: i32 = 0xd0_0000;
const RC_ANIMATIONS: [u32; 3] = [0x060a_3474, 0x060a_3484, 0x060a_3494];

pub fn ring_carrier_behaviour(cpu: &mut Cpu, o: u32) {
    if (cpu.read32(o + LIFE) as i32) <= 0 {
        nested!(cpu, 0x0605_9ee4, ground::blown_up_wounded(cpu, o));
        cpu.write8(o + OBJ_MODE, TO_STOP);
    } else {
        let y = cpu.read32(o + POSITION + 4).wrapping_add(RC_NEAR_FROM);
        cpu.write16(o + RC_NEAR, if y > RC_NEAR_SPAN { 0 } else { 16 });
        let step = cpu.read8(o + RC_STEP);
        if step <= 1 {
            if step == 0 {
                cpu.write8(o + RC_STEP, 1);
                cpu.write32(o + RC_AC, RC_START);
            }
            if cpu.read32(o + POSITION + 4) as i32 > RC_GONE_Y {
                cpu.write8(o + OBJ_MODE, TO_STOP);
                return;
            }
        }
        let flags = cpu.read32(o + FLAGS) & !NO_HITS;
        cpu.write32(o + FLAGS, flags);
        let down = cpu.read32(o + RC_AC).wrapping_add(RC_FALL);
        cpu.write32(o + RC_AC, down);
        let a = cpu.read16(o + RC_ANGLE).wrapping_add(cpu.read16(o + RC_A2));
        cpu.write16(o + RC_ANGLE, a);
        let a = cpu.read16(o + RC_ANGLE) as i16 as i32 as u32;
        let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a)) as i32;
        let r = cpu.read16(o + RC_SPEED) as i16 as i32;
        cpu.write32(o + POSITION, s.wrapping_mul(r) as u32);
        let a = cpu.read16(o + RC_ANGLE) as i16 as i32 as u32;
        let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a)) as i32;
        let r = cpu.read16(o + RC_SPEED) as i16 as i32;
        let y = c.wrapping_mul(r.wrapping_neg()) as u32;
        cpu.write32(o + POSITION + 4, y);
        let down = cpu.read32(o + RC_AC);
        cpu.write32(o + POSITION + 4, y.wrapping_add(down));
        let turn = cpu.read16(o + ANGLE).wrapping_add(cpu.read16(o + RC_A2) << 2);
        cpu.write16(o + ANGLE, turn);
    }
    let f = cpu.read32(GAME_FRAMES) & 3;
    let anim = match f {
        1 => RC_ANIMATIONS[1],
        3 => RC_ANIMATIONS[2],
        _ => RC_ANIMATIONS[0],
    };
    cpu.write32(o + M_ANIMATION, anim);
}

// ---- the jet flames ----------------------------------------------------------------------------------

/// A jet flame (a plain enemy actor streaming back from its owner, +0x6c): the owner's word
/// (+0x70: a pointer whose +2 word, if set, is how far the flame reaches), how far back it is (a
/// word, +0x8a: 16 more each frame), its growth a frame (a word, +0xa8: its scale grows by it,
/// and it wavers by a quarter of it).
const JF_OWNER: u32 = 0x6c;
const JF_REACH_AT: u32 = 0x70;
const JF_BACK: u32 = 0x8a;
const JF_GROW: u32 = 0xa8;
const JF_ANIMATIONS: [u32; 2] = [0x060a_3f58, 0x060a_3f60];

pub fn jet_flame_behaviour(cpu: &mut Cpu, o: u32) {
    let owner = cpu.read32(o + JF_OWNER);
    let reach_at = cpu.read32(o + JF_REACH_AT);
    let off = nested!(cpu, 0x0605_6a24, aim::off_screen(cpu, o + 0x10)) as u8 != 0;
    let gone = off || cpu.read8(owner + OBJ_MODE) as i8 > ENDING || {
        let reach = cpu.read16(reach_at + 2) as i16;
        reach != 0 && reach < cpu.read16(o + JF_BACK) as i16
    };
    if gone {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    let waver = (cpu.read16(o + JF_GROW) as i16 as i32 / 4) as i16 as i32 as u32;
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, waver)) as u16;
    let a = r.wrapping_add(cpu.read16(owner + ANGLE)).wrapping_add(0x8000);
    cpu.write16(o + ANGLE, a);
    let back = cpu.read16(owner + ANGLE).wrapping_add(0x8000) as i16 as i32 as u32;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, back)) as i32;
    let d = cpu.read16(o + JF_BACK) as i16 as i32;
    let x = cpu.read32(owner + POSITION).wrapping_add(s.wrapping_mul(d) as u32);
    cpu.write32(o + POSITION, x);
    let back = cpu.read16(owner + ANGLE).wrapping_add(0x8000) as i16 as i32 as u32;
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, back)) as i32;
    let d = cpu.read16(o + JF_BACK) as i16 as i32;
    let y = cpu.read32(owner + POSITION + 4).wrapping_add(c.wrapping_neg().wrapping_mul(d) as u32);
    cpu.write32(o + POSITION + 4, y);
    let z = cpu.read32(owner + POSITION + 8);
    cpu.write32(o + POSITION + 8, z);
    let b = cpu.read16(o + JF_BACK).wrapping_add(16);
    cpu.write16(o + JF_BACK, b);
    let anim = JF_ANIMATIONS[(cpu.read32(GAME_FRAMES) & 1) as usize];
    cpu.write32(o + M_ANIMATION, anim);
    let s = cpu.read32(o + SCALES).wrapping_add(cpu.read16(o + JF_GROW) as i16 as i32 as u32);
    cpu.write32(o + SCALES, s);
}

/// A jet flame made from the description `d` (its number +0x1e, its distance back a word +0x2e,
/// its size a word +0x30) behind `owner` (one dot back along its angle), reaching as far as the
/// word at `reach_at` + 2 says: kind 0's frame 100 on (by the frame of 7), colour 0x40 on,
/// flipped at random; small ones hidden and unhit.
const JF_SIZE: u32 = 0xbc;
const JF_BEHAVIOUR: u32 = 0x0608_9228;
const JF_FRAME: u8 = 100;
const JF_COLOUR: u16 = 0x40;
const JF_SMALL: i32 = 0x400;
const JF_HIDE: u32 = 0x8_0100;
const D_BACK: u32 = 0x2e;
const D_SIZE: u32 = 0x30;

pub fn jet_flame_set(cpu: &mut Cpu, d: u32, owner: u32, reach_at: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, JF_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + 0x10));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HITS_AT));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + BEHAVIOUR, JF_BEHAVIOUR);
    let n = cpu.read8(d + S_NUMBER);
    cpu.write8(o + NUMBER, n);
    cpu.write32(o + JF_OWNER, owner);
    cpu.write32(o + JF_REACH_AT, reach_at);
    cpu.write32(o + M_ANIMATION, JF_ANIMATIONS[0]);
    cpu.write8(o + SIDE, 1);
    cpu.write8(o + SPRITE_KIND, 0);
    let colour = cpu.read16(CHAR_BASE_0).wrapping_add(JF_COLOUR);
    cpu.write16(o + COLOUR, colour);
    cpu.write16(o + OWN_COLOUR, colour);
    let frames = cpu.read32(GAME_FRAMES);
    let f = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, frames, 7)) as u8;
    cpu.write8(o + SPRITE_FRAME, f.wrapping_add(JF_FRAME));
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write32(o + FLAGS, r & FLIP_X | FLAG_400);
    let a = cpu.read16(owner + ANGLE);
    cpu.write16(o + ANGLE, a);
    let a = cpu.read16(owner + ANGLE) as i16 as i32 as u32;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    let x = cpu.read32(owner + POSITION).wrapping_add(s);
    cpu.write32(o + POSITION, x);
    let a = cpu.read16(owner + ANGLE) as i16 as i32 as u32;
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    let y = cpu.read32(owner + POSITION + 4).wrapping_sub(c);
    cpu.write32(o + POSITION + 4, y);
    cpu.write32(o + POSITION + 8, 0);
    cpu.write8(o + BYTE_33, 8);
    let b = cpu.read16(d + D_BACK);
    cpu.write16(o + JF_BACK, b);
    let size = cpu.read16(d + D_SIZE) as i16 as i32;
    cpu.write32(o + SCALES, (size << 4) as u32);
    cpu.write16(o + JF_GROW, (size << 6) as u16);
    if (cpu.read32(o + SCALES) as i32) < JF_SMALL {
        let flags = cpu.read32(o + FLAGS) | JF_HIDE;
        cpu.write32(o + FLAGS, flags);
    }
}

// ---- the blast ---------------------------------------------------------------------------------------

/// A muzzle blast (a plain actor held at its gun, kind 0x36's frames 1-4): its step (a byte,
/// +0x6c: 1 once fading), pointers into its gun: its flags word (+0x70: bit 6 ends it), its place
/// (+0x74: x, y) and angle word (+0x78); its turn from that angle (a word, +0x7c). It stretches
/// (its scale x 0x580 more a frame) while its y scale fades (by 1/256 a frame, from 1/4, once
/// past 1/2 stretched), small: hidden, unhit and pulled back.
const BL_STEP: u32 = 0x6c;
const BL_FLAGS_AT: u32 = 0x70;
const BL_PLACE_AT: u32 = 0x74;
const BL_ANGLE_AT: u32 = 0x78;
const BL_TURN: u32 = 0x7c;
const BL_ENDED: u16 = 0x40;
const BL_STRETCH: u32 = 0x580;
const BL_FADE_FROM: i32 = 0x8000;
const BL_FADE: u32 = 0x100;
const BL_SMALL: i32 = 0x17ff;
const BL_HIDE: u32 = 0x8_0100;
const FLIP_Y: u32 = 0x20;

pub fn blast_behaviour(cpu: &mut Cpu, o: u32) {
    if cpu.read32(o + SCALES) as i32 > BL_FADE_FROM || cpu.read8(o + BL_STEP) != 0 {
        let v = cpu.read32(o + SCALES + 4);
        let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0, BL_FADE));
        cpu.write32(o + SCALES + 4, v);
    }
    let flags_at = cpu.read32(o + BL_FLAGS_AT);
    if cpu.read16(flags_at) & BL_ENDED != 0 || cpu.read32(o + SCALES + 4) == 0 {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    let s = cpu.read32(o + SCALES).wrapping_add(BL_STRETCH);
    cpu.write32(o + SCALES, s);
    let angle_at = cpu.read32(o + BL_ANGLE_AT);
    let a = cpu.read16(angle_at).wrapping_add(cpu.read16(o + BL_TURN));
    cpu.write16(o + ANGLE, a);
    let place = cpu.read32(o + BL_PLACE_AT);
    let x = cpu.read32(place);
    cpu.write32(o + POSITION, x);
    let place = cpu.read32(o + BL_PLACE_AT);
    let y = cpu.read32(place + 4);
    cpu.write32(o + POSITION + 4, y);
    cpu.write32(o + POSITION + 8, 0);
    let f = (cpu.read8(GAME_FRAMES + 3) & 3) + 1;
    cpu.write8(o + SPRITE_FRAME, f);
    let fade = cpu.read32(o + SCALES + 4) as i32;
    if fade <= BL_SMALL {
        cpu.write8(o + BL_STEP, 1);
        let flags = cpu.read32(o + FLAGS) | BL_HIDE;
        cpu.write32(o + FLAGS, flags);
        let pull = (0x1800i32.wrapping_sub(cpu.read32(o + SCALES + 4) as i32)).wrapping_mul(3);
        let s = (cpu.read32(o + SCALES) as i32).wrapping_sub(pull);
        cpu.write32(o + SCALES, s as u32);
    }
    let colour = if cpu.read32(GAME_FRAMES) & 3 != 0 { 0x30 } else { 0x20 };
    let colour = cpu.read16(CHAR_BASE_0).wrapping_add(colour);
    cpu.write16(o + COLOUR, colour);
    let c = cpu.read16(o + COLOUR);
    cpu.write16(o + OWN_COLOUR, c);
    let flags = cpu.read32(o + FLAGS) & !FLIP_Y;
    cpu.write32(o + FLAGS, flags);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    let flags = cpu.read32(o + FLAGS) | (r & FLIP_Y);
    cpu.write32(o + FLAGS, flags);
}

/// A muzzle blast made (sound 76) for player-side `number`, turned `turn` from its gun's angle
/// (flipped when a quarter turn), held at the gun by the pointers.
const BL_SIZE: u32 = 0x80;
const BL_BEHAVIOUR: u32 = 0x0608_9498;
const BL_SOUND: u32 = 76;
const BL_KIND: u8 = 0x36;
const BL_ANIMATION: u32 = 0x060a_3f68;

pub fn blast_set(cpu: &mut Cpu, number: u32, turn: u32, flags_at: u32, place_at: u32, angle_at: u32) {
    let number = number as u8;
    let turn = turn as u16;
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, BL_SOUND));
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, BL_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + 0x10));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HITS_AT));
    cpu.write32(o + BEHAVIOUR, BL_BEHAVIOUR);
    cpu.write8(o + NUMBER, number);
    cpu.write8(o + BL_STEP, 0);
    cpu.write32(o + BL_FLAGS_AT, flags_at);
    cpu.write32(o + BL_PLACE_AT, place_at);
    cpu.write32(o + BL_ANGLE_AT, angle_at);
    cpu.write16(o + BL_TURN, turn);
    cpu.write8(o + BYTE_33, 0);
    cpu.write32(o + FLAGS, if turn == 0x4000 { FLAG_400 | FLIP_X } else { FLAG_400 });
    cpu.write8(o + SPRITE_FRAME, 1);
    cpu.write8(o + SPRITE_KIND, BL_KIND);
    let colour = cpu.read16(CHAR_BASE_0).wrapping_add(0x30);
    cpu.write16(o + OWN_COLOUR, colour);
    cpu.write16(o + COLOUR, colour);
    cpu.write32(o + M_ANIMATION, BL_ANIMATION);
    cpu.write8(o + SIDE, 1);
    cpu.write8(o + HIT_WEAPON, 5);
    cpu.write32(o + SCALES, 0);
    cpu.write32(o + SCALES + 4, 0x4000);
}

/// A pulse shot from `o` at `angle` (16 dots out, flags 0x29).
const PS_OUT: i32 = 16;
const PS_FLAGS: u16 = 0x29;

pub fn pulse_shot(cpu: &mut Cpu, o: u32, angle: u32) {
    let t = cpu.stack();
    let n = cpu.read8(o + NUMBER);
    cpu.write8(t + S_NUMBER, n);
    cpu.write16(t + S_FLAGS, PS_FLAGS);
    cpu.write16(t + S_ANGLE, angle as u16);
    let a = angle as i16 as i32 as u32;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a)) as i32;
    let x = cpu.read32(o + POSITION).wrapping_add(s.wrapping_mul(PS_OUT) as u32);
    cpu.write32(t + S_POSITION, x);
    let a = cpu.read16(t + S_ANGLE) as i16 as i32 as u32;
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a)) as i32;
    let y = cpu.read32(o + POSITION + 4).wrapping_add(c.wrapping_mul(-PS_OUT) as u32);
    cpu.write32(t + S_POSITION + 4, y);
    cpu.write32(t + S_POSITION + 8, 0);
    nested!(cpu, 0x0605_4c8c, bullets::pulse_set(cpu, t));
}

// ---- the push beam -----------------------------------------------------------------------------------

/// A push beam (a plain actor drawn as a ragged polygon, out from its gun 16 dots a frame): how far
/// out (16.16, +0x6c: from 192 dots back, by the frame), pointers into its gun: its flags word
/// (+0x70: bit 6 ends it) and its place (+0x74). A player it hits is pushed along (by rank, the
/// table at 0x060a3fa0, in 16ths of a dot).
const PB_OUT: u32 = 0x6c;
const PB_FLAGS_AT: u32 = 0x70;
const PB_PLACE_AT: u32 = 0x74;
const PB_STEP: u32 = 0x10_0000;
const PB_REACH: i32 = 0xf_ffff;
const PB_PUSHES: u32 = 0x060a_3fa0;
const HIT_BY_AT: u32 = 0x40;

pub fn push_beam_behaviour(cpu: &mut Cpu, o: u32) {
    let out = cpu.read32(o + PB_OUT).wrapping_add(PB_STEP);
    cpu.write32(o + PB_OUT, out);
    let flags_at = cpu.read32(o + PB_FLAGS_AT);
    if cpu.read16(flags_at) & BL_ENDED != 0 || out as i32 > PB_REACH {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    if cpu.read16(o + HIT_FLAGS) & HIT_SOMETHING != 0 {
        let by = cpu.read32(o + HIT_BY_AT);
        if cpu.read32(PLAYERS[0]) == by || cpu.read32(PLAYERS[1]) == by {
            let rank = cpu.read8(RANK) as i8 as i32 as u32;
            let push = (cpu.read8(PB_PUSHES.wrapping_add(rank)) as u32) << 12;
            let x = cpu.read32(by + POSITION).wrapping_add(push);
            cpu.write32(by + POSITION, x);
        }
    }
    let place = cpu.read32(o + PB_PLACE_AT);
    let x = cpu.read32(place).wrapping_add(cpu.read32(o + PB_OUT));
    cpu.write32(o + POSITION, x);
    let place = cpu.read32(o + PB_PLACE_AT);
    let y = cpu.read32(place + 4);
    cpu.write32(o + POSITION + 4, y);
    cpu.write32(o + POSITION + 8, 0);
}

/// The push beam's draw (its sprite `s`): a sprite on the stack (a polygon of its own ragged
/// corners at +0x2c, flipped at random), at its place give or take two dots, colour one of 4.
const SP_CORNERS_AT: u32 = 0x24;
const SPR_CORNERS: u32 = 0x2c;

pub fn push_beam_draw(cpu: &mut Cpu, s: u32) {
    let o = s.wrapping_sub(0x10);
    let t = cpu.stack();
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, t));
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write32(t + SP_FLAGS, r & 0x100 | 1);
    let dx = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x4_0000));
    let x = cpu.read32(o + POSITION).wrapping_add(dx);
    cpu.write32(t + SP_POSITION, x);
    let dy = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x4_0000));
    let y = cpu.read32(o + POSITION + 4).wrapping_add(dy);
    cpu.write32(t + SP_POSITION + 4, y);
    cpu.write32(t + SP_POSITION + 8, 0);
    cpu.write8(t + SP_LIFT, 16);
    let a = cpu.read16(o + ANGLE);
    cpu.write16(t + SP_ANGLE, a);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000)) as u16;
    let colour = (r & 3).wrapping_add(cpu.read16(CHAR_BASE_0));
    cpu.write16(t + SP_COLOUR, colour);
    cpu.write32(t + SP_CORNERS_AT, t + SPR_CORNERS);
    let c = t + SPR_CORNERS;
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000)) as u16;
    cpu.write16(c, (r & 3).wrapping_neg());
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000)) as u16;
    cpu.write16(c + 2, (r & 7).wrapping_neg().wrapping_sub(16));
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000)) as u16;
    cpu.write16(c + 4, r & 3);
    let v = cpu.read16(c + 2);
    cpu.write16(c + 6, v);
    cpu.write16(c + 8, r & 3);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000)) as u16;
    let low = (r & 7).wrapping_add(16);
    cpu.write16(c + 10, low);
    let v = cpu.read16(c);
    cpu.write16(c + 12, v);
    cpu.write16(c + 14, low);
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, t));
}

/// A push beam made for player-side `number` at the gun (its flags word and place).
const PB_SIZE: u32 = 0x78;
const PB_BEHAVIOUR: u32 = 0x0608_9734;
const PB_DRAW: u32 = 0x0608_97d4;
const PB_FLAGS: u32 = 0xc0_0400;
const PB_ANIMATION: u32 = 0x060a_3f98;

pub fn push_beam_set(cpu: &mut Cpu, number: u32, flags_at: u32, place_at: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, PB_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + 0x10));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HITS_AT));
    cpu.write32(o + BEHAVIOUR, PB_BEHAVIOUR);
    cpu.write32(o + FLAGS, PB_FLAGS);
    cpu.write32(o + MODEL_DRAW, PB_DRAW);
    cpu.write8(o + NUMBER, number as u8);
    cpu.write32(o + PB_FLAGS_AT, flags_at);
    cpu.write32(o + PB_PLACE_AT, place_at);
    cpu.write32(o + M_ANIMATION, PB_ANIMATION);
    cpu.write8(o + SIDE, 1);
    cpu.write8(o + HIT_WEAPON, 5);
    cpu.write16(o + 0x58, 0);
    let f = cpu.read32(GAME_FRAMES) & 0xf;
    cpu.write32(o + PB_OUT, f.wrapping_sub(0xc0) << 16);
}

// ---- the snake -----------------------------------------------------------------------------------------

/// The snake's head (an enemy actor whose body pieces follow its trail): its count of frames
/// left (a word, +0x6c), its turning (a word, +0x6e: the turn a frame is 1/this of the way to
/// where it aims), its last move in whole dots (bytes, +0x70, +0x71), its trail (96 bytes each of
/// moves x +0x72, y +0xd2, and angle +0x132 in 256ths).
const SN_LEFT: u32 = 0x6c;
const SN_TURNING: u32 = 0x6e;
const SN_MOVE: u32 = 0x70;
const SN_TRAIL_X: u32 = 0x72;
const SN_TRAIL_Y: u32 = 0xd2;
const SN_TRAIL_ANGLE: u32 = 0x132;
const SN_TRAIL: u32 = 96;
/// By rank: its turning (0x060a3fb8's, approached a step a frame) and its speed (0x060a3fc0's,
/// 16ths of a dot; across at 3/4 of it), aiming 32 dots above where the player was 10 frames
/// ago.
const SN_TURNINGS: u32 = 0x060a_3fb8;
const SN_SPEEDS: u32 = 0x060a_3fc0;
const SN_AIM_UP: u32 = 0x20_0000;
const SN_AIM_DELAY: u32 = 10;

/// The head's steering and move.
pub fn snake_steer(cpu: &mut Cpu, o: u32) {
    let rank = cpu.read8(RANK) as i8 as i32 as u32;
    let to = cpu.read8(SN_TURNINGS.wrapping_add(rank)) as u32;
    let t = cpu.read16(o + SN_TURNING) as i16 as i32 as u32;
    let t = nested!(cpu, 0x0604_6168, actors::approach(cpu, t, to, 1));
    cpu.write16(o + SN_TURNING, t as u16);
    let y = cpu.read32(o + POSITION + 4).wrapping_add(SN_AIM_UP);
    cpu.write32(o + POSITION + 4, y);
    let turning = cpu.read16(o + SN_TURNING) as i16 as i32 as u32;
    let goal = nested!(cpu, 0x0605_6ad4, aim::angle_to_target(cpu, o, SN_AIM_DELAY)) & 0xffff;
    let a = cpu.read16(o + ANGLE) as i16 as i32 as u32;
    let d = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, a, goal));
    let q = nested!(cpu, 0x0600_6a78, math::quotient(cpu, d, turning)) as u16;
    let a = q.wrapping_add(cpu.read16(o + ANGLE));
    cpu.write16(o + ANGLE, a);
    let y = cpu.read32(o + POSITION + 4).wrapping_sub(SN_AIM_UP);
    cpu.write32(o + POSITION + 4, y);
    let rank = cpu.read8(RANK) as i8 as i32 as u32;
    let speed = (cpu.read8(SN_SPEEDS.wrapping_add(rank)) as u32).wrapping_mul(0x1000);
    let old = cpu.read32(o + POSITION);
    let across = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, speed, 0xc000));
    let a = cpu.read16(o + ANGLE) as i16 as i32 as u32;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    let dx = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, across));
    let x = cpu.read32(o + POSITION).wrapping_add(dx);
    cpu.write32(o + POSITION, x);
    cpu.write8(o + SN_MOVE, dots(x).wrapping_sub(dots(old)) as u8);
    let old = cpu.read32(o + POSITION + 4);
    let a = cpu.read16(o + ANGLE) as i16 as i32 as u32;
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    let dy = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, c, speed));
    let y = cpu.read32(o + POSITION + 4).wrapping_sub(dy);
    cpu.write32(o + POSITION + 4, y);
    cpu.write8(o + SN_MOVE + 1, dots(y).wrapping_sub(dots(old)) as u8);
}

/// A snake piece's end: smoke (drift_effect's: flags 2, colour 0x90, with its animation at its
/// place and angle), and at rank 2 on a shot at the nearer player (flags 2, speed 1 1/8).
const SN_SMOKE_ANIMATION: u32 = 0x0607_305c;
const SN_SMOKE_COLOUR: u16 = 0x90;
const SN_SHOT_SPEED: u32 = 0x1_2000;

pub fn snake_piece_end(cpu: &mut Cpu, o: u32) {
    let t = cpu.stack();
    cpu.write32(t + effects::RELATIVE_3C, SN_SMOKE_ANIMATION);
    cpu.write16(t + effects::COLOUR, SN_SMOKE_COLOUR);
    for k in [0, 4, 8] {
        let v = cpu.read32(o + POSITION + k);
        cpu.write32(t + effects::X + k, v);
    }
    let a = cpu.read16(o + ANGLE);
    cpu.write16(t + effects::ANGLE, a);
    cpu.write32(t + effects::FLAGS_LONG, 2);
    cpu.write32(t + effects::SCALE_X, 0x1_0000);
    cpu.write32(t + effects::SCALE_Y, 0x1_0000);
    nested!(cpu, 0x0605_7454, actors::drift_effect(cpu, t));
    if cpu.read8(RANK) as i8 > 1 {
        let n = cpu.read8(o + NUMBER);
        cpu.write8(t + S_NUMBER, n);
        let p = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + 0x10));
        let a = nested!(cpu, 0x0604_5e30, aim::angle_to(cpu, o, p));
        cpu.write16(t + S_ANGLE, a as u16);
        cpu.write16(t + S_FLAGS, 2);
        cpu.write32(t + S_SPEED, SN_SHOT_SPEED);
        nested!(cpu, 0x0605_3db0, bullets::bullet_set(cpu, t));
    }
}

/// The head's behaviour: its count down (out: it ends); it steers, its moves and angle into its
/// trail; its colour flashes in its last 30 frames, before that cycles by the count; its lift
/// alternates.
pub fn snake_head_behaviour(cpu: &mut Cpu, o: u32) {
    let left = cpu.read16(o + SN_LEFT).wrapping_sub(1);
    cpu.write16(o + SN_LEFT, left);
    if (left as i16) < 0 {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        nested!(cpu, 0x0608_9ac0, snake_piece_end(cpu, o));
        return;
    }
    nested!(cpu, 0x0608_998c, snake_steer(cpu, o));
    let m = cpu.read8(o + SN_MOVE) as i8 as i32 as u32;
    nested!(cpu, 0x0605_62b4, actors::shift_in_bytes(cpu, o + SN_TRAIL_X, m, SN_TRAIL));
    let m = cpu.read8(o + SN_MOVE + 1) as i8 as i32 as u32;
    nested!(cpu, 0x0605_62b4, actors::shift_in_bytes(cpu, o + SN_TRAIL_Y, m, SN_TRAIL));
    let a = cpu.read16(o + ANGLE) as i16 as i32;
    let a = if a < 0 { a + 0xff } else { a } as u32;
    nested!(cpu, 0x0600_8164, clib::ashiftrt_r4::<8>(cpu, a));
    let a = (a as i32 >> 8) as i8 as i32 as u32;
    nested!(cpu, 0x0605_62b4, actors::shift_in_bytes(cpu, o + SN_TRAIL_ANGLE, a, SN_TRAIL));
    let left = cpu.read16(o + SN_LEFT) as i16;
    let kc = cpu.read16(CHAR_BASE_0);
    let colour = if left < 30 && cpu.read32(GAME_FRAMES) & 2 != 0 {
        kc.wrapping_add(0xe0)
    } else if left > 29 {
        match ((left as i32 / 4) as u32) & 3 {
            0 => kc.wrapping_add(0x50),
            2 => kc,
            _ => kc.wrapping_add(0x10),
        }
    } else {
        kc.wrapping_add(0xb0)
    };
    cpu.write16(o + OWN_COLOUR, colour);
    let lift = if cpu.read32(GAME_FRAMES) & 1 != 0 { 0x7e } else { 8 };
    cpu.write8(o + BYTE_33, lift);
}

/// A body piece of the snake (its head +0x6c, how far back along its trail, a word +0x70): it
/// moves by the head's trail there, takes its angle, colour and lift; it ends with the head.
const SB_HEAD: u32 = 0x6c;
const SB_BACK: u32 = 0x70;

pub fn snake_body_behaviour(cpu: &mut Cpu, o: u32) {
    let head = cpu.read32(o + SB_HEAD);
    if cpu.read8(head + OBJ_MODE) as i8 > ENDING {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        nested!(cpu, 0x0608_9ac0, snake_piece_end(cpu, o));
        return;
    }
    let back = cpu.read16(o + SB_BACK) as u32;
    let dx = cpu.read8(head + SN_TRAIL_X + back) as i8 as i32 as u32;
    let x = cpu.read32(o + POSITION).wrapping_add(dx << 16);
    cpu.write32(o + POSITION, x);
    let back = cpu.read16(o + SB_BACK) as u32;
    let dy = cpu.read8(head + SN_TRAIL_Y + back) as i8 as i32 as u32;
    let y = cpu.read32(o + POSITION + 4).wrapping_add(dy << 16);
    cpu.write32(o + POSITION + 4, y);
    let back = cpu.read16(o + SB_BACK) as u32;
    let a = cpu.read8(head + SN_TRAIL_ANGLE + back) as i8 as i32 as u32;
    cpu.write16(o + ANGLE, (a << 8) as u16);
    let c = cpu.read16(head + OWN_COLOUR);
    cpu.write16(o + OWN_COLOUR, c);
    let l = cpu.read8(head + BYTE_33);
    cpu.write8(o + BYTE_33, l);
}

/// The snake made from the event `e` (its place: x, y at +0, +4; its count of frames, a word at
/// +0x2c): its head (0x194 bytes) and 16 body pieces (0x74 bytes) behind it, each smaller by 3/128
/// and 6 frames further back along the head's trail; all numbered enemies, sprite 98 of kind 0
/// (colours 0x50 on), hit by weapon 4 on side 1. The trail starts still.
const SN_HEAD_SIZE: u32 = 0x194;
const SB_SIZE: u32 = 0x74;
const SN_HEAD_BEHAVIOUR: u32 = 0x0608_9b70;
const SB_BEHAVIOUR: u32 = 0x0608_9ca0;
const SN_FLAGS: u32 = 0x4_0400;
const SN_FRAME: u8 = 98;
const SN_COLOURS: u16 = 0x50;
const SN_ANIMATION: u32 = 0x060a_3fa6;
const SN_WEAPON: u8 = 4;
const SN_START_TURNING: u16 = 100;
const SN_E_LEFT: u32 = 0x2c;
const SN_PIECES: i8 = 16;
const SB_BACK_STEP: u16 = 6;
const SB_BACK_FIRST: u16 = 5;
const SB_SHRINK: u32 = 0x600;

/// What the head and each body piece share: number, flags, place, angle 0, sprite, colours,
/// animation, side and weapon.
fn snake_piece_init(cpu: &mut Cpu, o: u32, behaviour: u32, n: i8, e: u32) {
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + 0x10));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HITS_AT));
    cpu.write32(o + BEHAVIOUR, behaviour);
    cpu.write8(o + NUMBER, n as u8);
    let k = cpu.read8(o + NUMBER) as i8 as i32 as u32;
    cpu.write32(NUMBERED.wrapping_add(k << 2), o);
    cpu.write32(o + FLAGS, SN_FLAGS);
    let (x, y) = (cpu.read32(e), cpu.read32(e + 4));
    cpu.write32(o + POSITION, x);
    cpu.write32(o + POSITION + 4, y);
    cpu.write32(o + POSITION + 8, 0);
    cpu.write16(o + ANGLE, 0);
    cpu.write8(o + SPRITE_FRAME, SN_FRAME);
    cpu.write8(o + SPRITE_KIND, 0);
    let c = cpu.read16(CHAR_BASE_0).wrapping_add(SN_COLOURS);
    cpu.write16(o + OWN_COLOUR, c);
    cpu.write16(o + COLOUR, c);
    cpu.write32(o + M_ANIMATION, SN_ANIMATION);
    cpu.write8(o + SIDE, 1);
    cpu.write8(o + HIT_WEAPON, SN_WEAPON);
}

pub fn snake_set(cpu: &mut Cpu, e: u32) {
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, 2)) as i8;
    if n == -1 {
        return;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, SN_HEAD_SIZE)) as i8 as i32 as u32;
    let head = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, NUMBERED_EXEC));
    if head == 0 {
        return;
    }
    snake_piece_init(cpu, head, SN_HEAD_BEHAVIOUR, n, e);
    cpu.write8(head + SN_MOVE, 0);
    cpu.write8(head + SN_MOVE + 1, 0);
    nested!(cpu, 0x0605_6294, actors::fill_bytes(cpu, head + SN_TRAIL_X, 0, SN_TRAIL));
    nested!(cpu, 0x0605_6294, actors::fill_bytes(cpu, head + SN_TRAIL_Y, 0, SN_TRAIL));
    let a = cpu.read16(head + ANGLE) as i16 as i32;
    let a = if a < 0 { a + 0xff } else { a } as u32;
    nested!(cpu, 0x0600_8164, clib::ashiftrt_r4::<8>(cpu, a));
    let a = (a as i32 >> 8) as i8 as i32 as u32;
    nested!(cpu, 0x0605_6294, actors::fill_bytes(cpu, head + SN_TRAIL_ANGLE, a, SN_TRAIL));
    let left = cpu.read16(e + SN_E_LEFT);
    cpu.write16(head + SN_LEFT, left);
    cpu.write16(head + SN_TURNING, SN_START_TURNING);
    for i in 0..SN_PIECES {
        let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, 2)) as i8;
        if n == -1 {
            return;
        }
        let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, SB_SIZE)) as i8 as i32 as u32;
        let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, NUMBERED_EXEC));
        if o == 0 {
            return;
        }
        snake_piece_init(cpu, o, SB_BEHAVIOUR, n, e);
        cpu.write32(o + SB_HEAD, head);
        cpu.write16(o + SB_BACK, (i as u16) * SB_BACK_STEP + SB_BACK_FIRST);
        let s = 0x1_0000u32.wrapping_sub((i as u32).wrapping_mul(SB_SHRINK));
        cpu.write32(o + SCALES + 4, s);
        cpu.write32(o + SCALES, s);
    }
}

/// A spread of 10 shots from `o` backwards along its aim (its angle and its aim's turn, a word at
/// +0x4c8): each turned at random (by the rank's spread, the words at 0x060a3fc6), 16 dots out,
/// flags 2, speed 1 1/8 to 1 5/8 at random; from rank 2 on a pulse shot along the same line too.
const FAN_TURN: u32 = 0x4c8;
const FAN_SPREADS: u32 = 0x060a_3fc6;
const FAN_SHOTS: i8 = 10;
const FAN_OUT: i32 = 16;
const FAN_SPEED: u32 = 0x1_2000;
const FAN_SPEED_RANDOM: u32 = 0x7fff;

pub fn fan_shots(cpu: &mut Cpu, o: u32) {
    let t = cpu.stack();
    let mut left = FAN_SHOTS;
    loop {
        let rank = cpu.read8(RANK) as i8 as i32 as u32;
        let spread = cpu.read16(FAN_SPREADS.wrapping_add(rank << 1)) as u32;
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, spread));
        let a = r
            .wrapping_add(cpu.read16(o + ANGLE) as i16 as u32)
            .wrapping_add(cpu.read16(o + FAN_TURN) as i16 as u32)
            .wrapping_add(0x8000);
        cpu.write16(t + S_ANGLE, a as u16);
        let n = cpu.read8(o + NUMBER);
        cpu.write8(t + S_NUMBER, n);
        let a = cpu.read16(t + S_ANGLE) as i16 as i32 as u32;
        let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a)) as i32;
        let x = cpu.read32(o + POSITION).wrapping_add(s.wrapping_mul(FAN_OUT) as u32);
        cpu.write32(t + S_POSITION, x);
        let a = cpu.read16(t + S_ANGLE) as i16 as i32 as u32;
        let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a)) as i32;
        let y = cpu.read32(o + POSITION + 4).wrapping_sub(c.wrapping_mul(FAN_OUT) as u32);
        cpu.write32(t + S_POSITION + 4, y);
        cpu.write32(t + S_POSITION + 8, 0);
        cpu.write16(t + S_FLAGS, 2);
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
        cpu.write32(t + S_SPEED, (r & FAN_SPEED_RANDOM).wrapping_add(FAN_SPEED));
        nested!(cpu, 0x0605_3db0, bullets::bullet_set(cpu, t));
        left -= 1;
        if left <= 0 {
            break;
        }
    }
    if cpu.read8(RANK) as i8 > 1 {
        let a = cpu
            .read16(o + ANGLE)
            .wrapping_add(cpu.read16(o + FAN_TURN))
            .wrapping_add(0x8000);
        nested!(cpu, 0x0608_96c8, pulse_shot(cpu, o, a as u32));
    }
}

// ---- the mid-boss ---------------------------------------------------------------------------------

/// The mid-boss (a large actor that works out its parts' points as it draws): its pose and script
/// (+0x6c, models::pose_step's; the script's flags at +0x94: bit 0 a beat, bit 1 another, bit 14
/// its end), its state (a byte, +0xf0: which of the functions at 0x060a4000 runs), its counts
/// (words: frames in the state +0xf4, frames since it was beaten +0xf6), its plan (the actions at
/// 0x060a3fd0: where it is +0xf8, frames to the next +0xfa, done +0xfc).
const MB_POSE: u32 = 0x6c;
const MB_SCRIPT_FLAGS: u32 = 0x94;
const MB_BEAT: u16 = 1;
const MB_BEAT_2: u16 = 2;
const MB_SCRIPT_END: u16 = 0x4000;
const MB_STATE: u32 = 0xf0;
const MB_COUNT: u32 = 0xf4;
const MB_BEATEN_FOR: u32 = 0xf6;
const MB_PLAN_AT: u32 = 0xf8;
const MB_WAIT: u32 = 0xfa;
const MB_PLAN_DONE: u32 = 0xfc;
const MB_PLAN: u32 = 0x060a_3fd0;
const MB_STATES: u32 = 0x060a_4000;
/// Its speed (x, y, z: +0x100), its turns' speeds (words: TURN_X's +0x10c, eased by +0x10e;
/// TURN_Y's +0x110; TURN_Z's +0x114).
const MB_SPEED: u32 = 0x100;
const MB_TURN_X_SPEED: u32 = 0x10c;
const MB_TURN_X_EASE: u32 = 0x10e;
const MB_TURN_Y_SPEED: u32 = 0x110;
const MB_TURN_Z_SPEED: u32 = 0x114;
/// Its 8 parts (parts::parts_set's): their flags (words from +0x11a: bit 5 not hit, bit 6
/// broken), how far each broken part's smoke has got (bytes from +0x12a), their lives (from
/// +0x134; parts 2 and 3 share one).
const MB_PART_FLAGS: u32 = 0x11a;
const MB_PART_SMOKE: u32 = 0x12a;
const MB_LIVES: u32 = 0x134;
const MB_UNHIT: u16 = 0x20;
const MB_BROKEN: u16 = 0x40;
/// The part flags by part.
const fn mb_part(k: u32) -> u32 {
    MB_PART_FLAGS + 2 * k
}
/// Its gun's turn (words: the turn +0x4c8, its speed +0x4ca, that eased by +0x4cc, the least and
/// most it turns +0x4ce, +0x4d0; drawn turned by +0x4c2, turning by +0x4c4).
const MB_GUN: u32 = 0x4c8;
const MB_GUN_SPEED: u32 = 0x4ca;
const MB_GUN_EASE: u32 = 0x4cc;
const MB_GUN_LEAST: u32 = 0x4ce;
const MB_GUN_MOST: u32 = 0x4d0;
const MB_SPIN: u32 = 0x4c2;
const MB_SPIN_SPEED: u32 = 0x4c4;
/// Which of its sides it opens (a byte, +0x4d2: the count of openings, or bit 0 the second side),
/// how many blasts it has fired (+0x4d3), and which sides are open (+0x4d4, +0x4d5).
const MB_SIDE: u32 = 0x4d2;
const MB_BLASTS: u32 = 0x4d3;
const MB_OPEN_A: u32 = 0x4d4;
const MB_OPEN_B: u32 = 0x4d5;
/// Its jet (jet_flame_set's): the reach (+0xdd8, and its last, +0xdda), its size (+0xddc), on
/// (+0xdde), how far it has gone round (16.16, +0xde0), and its nozzle's place (x, y, z: +0xde4).
const MB_JET_REACH: u32 = 0xdd8;
const MB_JET_LAST_REACH: u32 = 0xdda;
const MB_JET_SIZE: u32 = 0xddc;
const MB_JET_ON: u32 = 0xdde;
const MB_ROUND: u32 = 0xde0;
const MB_NOZZLE: u32 = 0xde4;
/// Its pulse volleys: their spread (a word, +0xdf0), their count (+0xdf2), frames to the next
/// (+0xdf3).
const MB_SPREAD: u32 = 0xdf0;
const MB_VOLLEYS: u32 = 0xdf2;
const MB_VOLLEY_WAIT: u32 = 0xdf3;
/// The scripts its actions start.
const MB_SCRIPT_IDLE: u32 = 0x060a_3d16;
const MB_SCRIPT_DIVE: u32 = 0x060a_3d90;
const MB_SCRIPT_OPEN_A: u32 = 0x060a_3e50;
const MB_SCRIPT_OPEN_B: u32 = 0x060a_3e86;
const MB_SCRIPT_JET: u32 = 0x060a_3f2e;
const MB_SCRIPT_LEFT: u32 = 0x060a_3d64;
const MB_SCRIPT_RIGHT: u32 = 0x060a_3d7a;
const MB_SCRIPT_VOLLEYS: u32 = 0x060a_3dd4;
const MB_SCRIPT_VOLLEYS_2: u32 = 0x060a_3de8;
const MB_SCRIPT_BACK: u32 = 0x060a_3d08;
const MB_SCRIPT_CIRCLE: u32 = 0x060a_3f4a;
const MB_SCRIPT_RISEN: u32 = 0x060a_3d30;
const MB_POSE_RISEN: u32 = 0x060a_3a8c;
/// Its snakes: 720 frames long.
const MB_SNAKE_FRAMES: u16 = 720;

fn mb_parts_and(cpu: &mut Cpu, o: u32, parts: &[u32], mask: u16) {
    for &k in parts {
        let f = cpu.read16(o + mb_part(k)) & mask;
        cpu.write16(o + mb_part(k), f);
    }
}

fn mb_parts_or(cpu: &mut Cpu, o: u32, parts: &[u32], bits: u16) {
    for &k in parts {
        let f = cpu.read16(o + mb_part(k)) | bits;
        cpu.write16(o + mb_part(k), f);
    }
}

fn mb_script(cpu: &mut Cpu, o: u32, script: u32) {
    nested!(cpu, 0x0605_6628, actors::script_start(cpu, o + MB_POSE, script));
}

fn mb_flags(cpu: &mut Cpu, o: u32) -> u16 {
    cpu.read16(o + MB_SCRIPT_FLAGS)
}

/// A snake from its place (snake_set's event on the stack: its place, 720 frames).
fn mb_snake_out(cpu: &mut Cpu, o: u32) {
    let t = cpu.stack();
    let (x, y) = (cpu.read32(o + POSITION), cpu.read32(o + POSITION + 4));
    cpu.write32(t, x);
    cpu.write32(t + 4, y);
    cpu.write16(t + SN_E_LEFT, MB_SNAKE_FRAMES);
    nested!(cpu, 0x0608_9d2c, snake_set(cpu, t));
}

/// Action 3: it dives (state 3).
pub fn mb_dive(cpu: &mut Cpu, o: u32) {
    cpu.write8(o + MB_STATE, 3);
    mb_script(cpu, o, MB_SCRIPT_DIVE);
    cpu.write16(o + MB_COUNT, 0);
}

/// To wandering (state 13).
pub fn mb_to_wander(cpu: &mut Cpu, o: u32) {
    cpu.write8(o + MB_STATE, 13);
    mb_script(cpu, o, MB_SCRIPT_IDLE);
}

/// Action 5: one side opens (state 17), which by the count of openings.
pub fn mb_open_one(cpu: &mut Cpu, o: u32) {
    cpu.write8(o + MB_STATE, 17);
    if cpu.read8(o + MB_SIDE) & 1 != 0 {
        mb_script(cpu, o, MB_SCRIPT_OPEN_B);
        cpu.write8(o + MB_OPEN_B, 1);
    } else {
        mb_script(cpu, o, MB_SCRIPT_OPEN_A);
        cpu.write8(o + MB_OPEN_A, 1);
    }
}

/// Action 7: the jet (state 21), with a snake and a sound.
pub fn mb_jet(cpu: &mut Cpu, o: u32) {
    cpu.write8(o + MB_STATE, 21);
    cpu.write16(o + MB_COUNT, 0);
    mb_script(cpu, o, MB_SCRIPT_JET);
    mb_snake_out(cpu, o);
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 70));
}

/// Off to the left (state 14).
pub fn mb_to_left(cpu: &mut Cpu, o: u32) {
    cpu.write8(o + MB_STATE, 14);
    mb_script(cpu, o, MB_SCRIPT_LEFT);
}

/// Off to the right (state 15).
pub fn mb_to_right(cpu: &mut Cpu, o: u32) {
    cpu.write8(o + MB_STATE, 15);
    mb_script(cpu, o, MB_SCRIPT_RIGHT);
}

/// Action 6: pulse volleys (state 19), spread 0x1c00 at first, the first in 30 frames.
pub fn mb_volleys_start(cpu: &mut Cpu, o: u32) {
    cpu.write8(o + MB_STATE, 19);
    cpu.write16(o + MB_SPREAD, 0x1c00);
    cpu.write8(o + MB_VOLLEYS, 0);
    cpu.write8(o + MB_VOLLEY_WAIT, 30);
    mb_script(cpu, o, MB_SCRIPT_VOLLEYS);
}

/// Its gun turned (aim::turn_toward's: at most 0x800, eased by 6, by 32 a frame) toward `to`.
fn mb_gun_turn(cpu: &mut Cpu, o: u32, to: u32) {
    static PROLOGUE: std::sync::OnceLock<super::Prologue> = std::sync::OnceLock::new();
    let from = cpu.read16(o + MB_GUN) as u32;
    cpu.nested_with_stack(&PROLOGUE, 0x0605_70d8, &[0x800, 6, 32], |cpu| {
        aim::turn_toward(cpu, o + MB_GUN_SPEED, o + MB_GUN_EASE, from, to, 0x800, 6, 32)
    });
}

/// Its gun turned back straight.
pub fn mb_gun_back(cpu: &mut Cpu, o: u32) {
    mb_gun_turn(cpu, o, 0);
}

/// Its gun turned toward the nearer player.
pub fn mb_gun_aim(cpu: &mut Cpu, o: u32) {
    let p = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + 0x10));
    let a = nested!(cpu, 0x0604_5e30, aim::angle_to(cpu, o, p));
    mb_gun_turn(cpu, o, a.wrapping_add(0x8000) & 0xffff);
}

/// Whether it stays where it is (1), or goes off after the nearer player, more than 92 dots off
/// to a side (0) — for action 7 by its own side of the screen instead; not while it waits or for
/// action 4.
const MB_FAR: i32 = 0x5c_0000;

pub fn mb_stays(cpu: &mut Cpu, o: u32) -> u32 {
    if cpu.read16(o + MB_WAIT) != 0 {
        return 1;
    }
    let at = cpu.read16(o + MB_PLAN_AT) as u32;
    let mut action = cpu.read8(MB_PLAN + at);
    if action == 9 {
        action = cpu.read8(MB_PLAN);
    }
    if action == 4 {
        return 1;
    }
    let left = if action == 7 {
        let x = cpu.read32(o + POSITION) as i32;
        if x > 0 {
            true
        } else if x >= 0 {
            return 1;
        } else {
            false
        }
    } else {
        let p = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + 0x10));
        let d = (cpu.read32(p + POSITION) as i32).wrapping_sub(cpu.read32(o + POSITION) as i32);
        if d > MB_FAR {
            false
        } else if d >= -MB_FAR {
            return 1;
        } else {
            true
        }
    };
    if left {
        nested!(cpu, 0x0608_a324, mb_to_left(cpu, o));
    } else {
        nested!(cpu, 0x0608_a34c, mb_to_right(cpu, o));
    }
    0
}

/// The plan's next action (8: the plan done; 9: back to its start); waits 60 frames after, 30
/// after actions 5 and 6.
pub fn mb_next_action(cpu: &mut Cpu, o: u32) {
    if cpu.read16(o + MB_WAIT) != 0 {
        return;
    }
    let at = cpu.read16(o + MB_PLAN_AT) as u32;
    let mut action = cpu.read8(MB_PLAN + at);
    if action == 8 {
        cpu.write16(o + MB_PLAN_DONE, 1);
        return;
    }
    if action == 9 {
        cpu.write16(o + MB_PLAN_AT, 0);
        action = cpu.read8(MB_PLAN);
    }
    let at = cpu.read16(o + MB_PLAN_AT).wrapping_add(1);
    cpu.write16(o + MB_PLAN_AT, at);
    let wait = match action {
        0 => {
            cpu.call(0x0608_a1e4, &[o]);
            60
        }
        1 => {
            cpu.call(0x0608_a27c, &[o]);
            60
        }
        2 => {
            cpu.call(0x0608_a2c4, &[o]);
            60
        }
        3 => {
            nested!(cpu, 0x0608_a07c, mb_dive(cpu, o));
            60
        }
        4 => {
            cpu.call(0x0608_a0b0, &[o]);
            60
        }
        5 => {
            nested!(cpu, 0x0608_a12c, mb_open_one(cpu, o));
            30
        }
        6 => {
            nested!(cpu, 0x0608_a374, mb_volleys_start(cpu, o));
            30
        }
        7 => {
            nested!(cpu, 0x0608_a184, mb_jet(cpu, o));
            60
        }
        _ => return,
    };
    cpu.write16(o + MB_WAIT, wait);
}

/// State 0: it rises from below; once past 0x300 dots up it stands (state 1) at the middle,
/// 100 dots deep, posed, with its script and sound 71.
pub fn mb_rising(cpu: &mut Cpu, o: u32) {
    if cpu.read32(o + POSITION + 4) as i32 > 0x300_0000 {
        let s = cpu.read8(o + MB_STATE).wrapping_add(1);
        cpu.write8(o + MB_STATE, s);
        cpu.write32(o + POSITION, 0);
        cpu.write32(o + POSITION + 4, 0);
        cpu.write32(o + POSITION + 8, 0x64_0000);
        cpu.write32(o + MB_SPEED + 4, 0);
        cpu.write32(o + MB_SPEED + 8, 0);
        nested!(cpu, 0x0605_65f0, models::pose_set(cpu, o + MB_POSE, MB_POSE_RISEN));
        mb_script(cpu, o, MB_SCRIPT_RISEN);
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 71));
    }
}

/// State 1: sound 72 on its script's beats; at its end, idling.
pub fn mb_risen(cpu: &mut Cpu, o: u32) {
    if mb_flags(cpu, o) & MB_BEAT != 0 {
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 72));
    }
    if mb_flags(cpu, o) & MB_SCRIPT_END != 0 {
        cpu.write8(o + MB_STATE, 2);
        cpu.write16(o + MB_WAIT, 0);
    }
}

/// State 2: it tilts up (TURN_X toward 0xc000), its gun back; the plan's next action once its
/// wait is over.
pub fn mb_idle(cpu: &mut Cpu, o: u32) {
    let from = cpu.read16(o + TURN_X) as u32;
    cpu.push(&[0x800, 6, 32]);
    nested!(cpu, 0x0605_70d8, aim::turn_toward(cpu, o + MB_TURN_X_SPEED, o + MB_TURN_X_EASE, from, 0xc000, 0x800, 6, 32));
    nested!(cpu, 0x0608_a3b4, mb_gun_back(cpu, o));
    cpu.pop(12);
    let wait = cpu.read16(o + MB_WAIT);
    if wait != 0 {
        let w = nested!(cpu, 0x0604_6168, actors::approach(cpu, wait as i16 as u32, 0, 1));
        cpu.write16(o + MB_WAIT, w as u16);
    } else {
        nested!(cpu, 0x0608_a4e8, mb_next_action(cpu, o));
    }
}

/// State 3: at its script's beat it starts down (state 4), every part open to hits.
pub fn mb_diving(cpu: &mut Cpu, o: u32) {
    if mb_flags(cpu, o) & MB_BEAT != 0 {
        let s = cpu.read8(o + MB_STATE).wrapping_add(1);
        cpu.write8(o + MB_STATE, s);
        cpu.write32(o + MB_SPEED + 4, 0xfffc_5000);
        mb_parts_and(cpu, o, &[0, 1, 2, 3, 4, 5, 6, 7], !MB_UNHIT);
    }
}

/// Sound 73 every 30 frames (from its 25th), at its place.
fn mb_swish(cpu: &mut Cpu, o: u32) {
    let n = (cpu.read16(o + MB_COUNT) as u32).wrapping_sub(25);
    if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, n, 30)) == 0 {
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 73));
    }
}

/// State 4: it dives, turning over, nearer by 2 dots a frame; past the front it waits above the
/// screen (state 12), level.
pub fn mb_sinking(cpu: &mut Cpu, o: u32) {
    mb_swish(cpu, o);
    let v = cpu.read32(o + MB_SPEED + 4).wrapping_add(0x1800);
    cpu.write32(o + MB_SPEED + 4, v);
    let z = cpu.read32(o + POSITION + 8).wrapping_add(0xfffe_0000);
    cpu.write32(o + POSITION + 8, z);
    let t = cpu.read16(o + TURN_X).wrapping_add(0x680);
    cpu.write16(o + TURN_X, t);
    if (cpu.read32(o + POSITION + 8) as i32) < 0 {
        cpu.write32(o + POSITION + 4, 0xffc0_0000);
        cpu.write32(o + POSITION + 8, 0);
        cpu.write32(o + MB_SPEED + 4, 0);
        cpu.write16(o + TURN_X, 0);
        cpu.write16(o + MB_TURN_X_SPEED, 0);
        cpu.write8(o + MB_STATE, 12);
    }
}

/// State 12: its gun at the player, stopping; at its script's end, wandering.
pub fn mb_stopping(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_a3f0, mb_gun_aim(cpu, o));
    let v = cpu.read32(o + MB_SPEED);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0, 0x2000));
    cpu.write32(o + MB_SPEED, v);
    if mb_flags(cpu, o) & MB_SCRIPT_END != 0 {
        cpu.write8(o + MB_STATE, 13);
        mb_script(cpu, o, MB_SCRIPT_IDLE);
    }
}

/// State 13: its gun at the player, 64 dots up the screen; it follows the player, or takes the
/// plan's next action.
pub fn mb_wandering(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_a3f0, mb_gun_aim(cpu, o));
    let w = cpu.read16(o + MB_WAIT) as i16 as u32;
    let w = nested!(cpu, 0x0604_6168, actors::approach(cpu, w, 0, 1));
    cpu.write16(o + MB_WAIT, w as u16);
    let y = cpu.read32(o + POSITION + 4);
    let y = nested!(cpu, 0x0604_6168, actors::approach(cpu, y, 0xffc0_0000, 0x2_0000));
    cpu.write32(o + POSITION + 4, y);
    if nested!(cpu, 0x0608_a454, mb_stays(cpu, o)) as u8 != 0 {
        nested!(cpu, 0x0608_a4e8, mb_next_action(cpu, o));
    }
}

/// States 14 and 15: off to a side at up to 8 dots a frame, to state 16 at the script's beat.
fn mb_going(cpu: &mut Cpu, o: u32, to: u32) {
    nested!(cpu, 0x0608_a3f0, mb_gun_aim(cpu, o));
    let v = cpu.read32(o + MB_SPEED);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, to, 0x4000));
    cpu.write32(o + MB_SPEED, v);
    if mb_flags(cpu, o) & MB_BEAT != 0 {
        cpu.write8(o + MB_STATE, 16);
    }
}

pub fn mb_going_left(cpu: &mut Cpu, o: u32) {
    mb_going(cpu, o, 0xfff8_0000);
}

pub fn mb_going_right(cpu: &mut Cpu, o: u32) {
    mb_going(cpu, o, 0x8_0000);
}

/// State 16: it slows; stopped, it wanders again.
pub fn mb_halting(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_a3f0, mb_gun_aim(cpu, o));
    let v = cpu.read32(o + MB_SPEED);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0, 0x4000));
    cpu.write32(o + MB_SPEED, v);
    if v == 0 {
        nested!(cpu, 0x0608_a104, mb_to_wander(cpu, o));
    }
}

/// Its pulse volleys: every 16 frames (sound 78) pulse shots along its gun, spread apart by the
/// spread: odd volleys 3 (5 every fourth), even ones 2 (4); the spread eased toward the rank's
/// (the words at 0x060a3fde) by 4 a frame.
const MB_SPREADS: u32 = 0x060a_3fde;

pub fn mb_volleys(cpu: &mut Cpu, o: u32) {
    let w = cpu.read8(o + MB_VOLLEY_WAIT).wrapping_sub(1);
    cpu.write8(o + MB_VOLLEY_WAIT, w);
    if (w as i8) < 0 {
        cpu.write8(o + MB_VOLLEY_WAIT, 15);
        let n = cpu.read8(o + MB_VOLLEYS).wrapping_add(1);
        cpu.write8(o + MB_VOLLEYS, n);
        let a = cpu.read16(o + TURN_Z).wrapping_add(cpu.read16(o + MB_GUN)).wrapping_add(0x8000) as u32;
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 78));
        let n = cpu.read8(o + MB_VOLLEYS);
        let shot = |cpu: &mut Cpu, angle: u32| {
            nested!(cpu, 0x0608_96c8, pulse_shot(cpu, o, angle & 0xffff));
        };
        if n & 1 != 0 {
            if n & 2 != 0 {
                let s = cpu.read16(o + MB_SPREAD) as u32;
                shot(cpu, a.wrapping_sub(2 * s));
                let s = cpu.read16(o + MB_SPREAD) as u32;
                shot(cpu, a.wrapping_add(2 * s));
            }
            let s = cpu.read16(o + MB_SPREAD) as u32;
            shot(cpu, a.wrapping_sub(s));
            shot(cpu, a);
            let s = cpu.read16(o + MB_SPREAD) as u32;
            shot(cpu, a.wrapping_add(s));
        } else {
            if n & 2 != 0 {
                let s = cpu.read16(o + MB_SPREAD) as u32;
                shot(cpu, a.wrapping_sub(s / 2).wrapping_sub(s));
                let s = cpu.read16(o + MB_SPREAD) as u32;
                shot(cpu, a.wrapping_add(s / 2).wrapping_add(s));
            }
            let s = cpu.read16(o + MB_SPREAD) as u32;
            shot(cpu, a.wrapping_sub(s / 2));
            let s = cpu.read16(o + MB_SPREAD) as u32;
            shot(cpu, a.wrapping_add(s / 2));
        }
    }
    let rank = cpu.read8(RANK) as i8 as i32 as u32;
    let to = cpu.read16(MB_SPREADS.wrapping_add(rank << 1)) as u32;
    let s = cpu.read16(o + MB_SPREAD) as u32;
    let s = nested!(cpu, 0x0604_6168, actors::approach(cpu, s, to, 4));
    cpu.write16(o + MB_SPREAD, s as u16);
}

/// State 19: its gun at the player, volleys; at its script's end, state 20.
pub fn mb_volleys_aim(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_a3f0, mb_gun_aim(cpu, o));
    if mb_flags(cpu, o) & MB_SCRIPT_END != 0 {
        let s = cpu.read8(o + MB_STATE).wrapping_add(1);
        cpu.write8(o + MB_STATE, s);
        mb_script(cpu, o, MB_SCRIPT_VOLLEYS_2);
    }
    nested!(cpu, 0x0608_ab50, mb_volleys(cpu, o));
}

/// State 20: volleys until past 60 of them, then back (state 12).
pub fn mb_volleys_fire(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_a3f0, mb_gun_aim(cpu, o));
    if cpu.read8(o + MB_VOLLEYS) > 60 {
        cpu.write8(o + MB_STATE, 12);
        mb_script(cpu, o, MB_SCRIPT_BACK);
    } else {
        nested!(cpu, 0x0608_ab50, mb_volleys(cpu, o));
    }
}

/// State 17: its gun back, stopping; sound 74 at the second beat; at the first, state 18.
pub fn mb_one_opening(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_a3b4, mb_gun_back(cpu, o));
    let v = cpu.read32(o + MB_SPEED);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0, 0x2000));
    cpu.write32(o + MB_SPEED, v);
    if mb_flags(cpu, o) & MB_BEAT_2 != 0 {
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 74));
    }
    if mb_flags(cpu, o) & MB_BEAT != 0 {
        cpu.write8(o + MB_STATE, 18);
        cpu.write8(o + MB_BLASTS, 0);
        cpu.write16(o + MB_COUNT, 0);
        cpu.write8(o + MB_OPEN_A, 0);
        cpu.write8(o + MB_OPEN_B, 0);
    }
}

/// State 18: one side open. Its steam (drift effects, colours 0x30 on, the script at
/// 0x060730b0, stretched by the frame: up over 14 frames, down after 128) from the side's
/// points; the first side swings its gun's least turn out and fires 8 blasts (by the points
/// at 0x060a3fe8), the second its most and a push beam for 175 frames, then 8 blasts (by those at
/// 0x060a3ff8) turned half round; every 80 frames a spread of shots (at 40) and pulse shots
/// (70 on, by the rank); it drifts up; at its script's end, back (state 12), the side for next
/// time at random.
const MB_STEAM_SCRIPT: u32 = 0x0607_30b0;
const MB_STEAM_COLOUR: u16 = 0x8030;
const MB_STEAM_Z: u32 = 0xfff8_8000;
const MB_POINTS_A: u32 = 0x3fc;
const MB_POINTS_B: u32 = 0x45c;
const MB_BLASTS_A: u32 = 0x060a_3fe8;
const MB_BLASTS_B: u32 = 0x060a_3ff8;
const MB_PUSH_AT: u32 = 0x48c;

pub fn mb_one_open(cpu: &mut Cpu, o: u32) {
    static PROLOGUE: std::sync::OnceLock<super::Prologue> = std::sync::OnceLock::new();
    nested!(cpu, 0x0608_a3f0, mb_gun_aim(cpu, o));
    let t = cpu.stack();
    cpu.write32(t + actors::E_SCRIPT, MB_STEAM_SCRIPT);
    cpu.write16(t + actors::E_COLOUR, MB_STEAM_COLOUR);
    cpu.write32(t + actors::E_SPEED, 0);
    cpu.write32(t + actors::E_POSITION + 8, MB_STEAM_Z);
    let count = cpu.read16(o + MB_COUNT) as u32;
    let stretch = if count <= 14 {
        count * 0x500
    } else if count > 128 {
        0x5000u32.wrapping_sub((count - 128).wrapping_mul(0x180))
    } else {
        0x5000
    };
    cpu.write32(t + actors::E_SCALES + 4, stretch);
    cpu.write32(t + actors::E_SCALES, 0x1_0000);
    let steam = |cpu: &mut Cpu, part: u32, k: u32, points: u32, turn: u16| {
        if cpu.read32(t + actors::E_SCALES + 4) as i32 > 0 && cpu.read16(o + mb_part(part)) & MB_BROKEN == 0 {
            let at = o + k * 12;
            let a = cpu.read16(o + TURN_Z).wrapping_add(turn);
            cpu.write16(t + actors::E_ANGLE, a);
            let x = cpu.read32(at + points);
            cpu.write32(t + actors::E_POSITION, x);
            let y = cpu.read32(at + points + 4);
            cpu.write32(t + actors::E_POSITION + 4, y);
            nested!(cpu, 0x0605_7454, actors::drift_effect(cpu, t));
        }
    };
    let second = cpu.read8(o + MB_SIDE) & 1 != 0;
    if !second {
        let l = cpu.read16(o + MB_GUN_LEAST) as i16 as u32;
        let l = nested!(cpu, 0x0604_6168, actors::approach(cpu, l, 0xffff_c000, 0x80));
        cpu.write16(o + MB_GUN_LEAST, l as u16);
        let k = (cpu.read16(o + MB_COUNT) & 7) as u32;
        steam(cpu, 2, k, MB_POINTS_A, 0x4000);
    } else {
        let m = cpu.read16(o + MB_GUN_MOST) as i16 as u32;
        let m = nested!(cpu, 0x0604_6168, actors::approach(cpu, m, 0x4000, 0x80));
        cpu.write16(o + MB_GUN_MOST, m as u16);
        if cpu.read16(o + MB_COUNT) <= 0xaf {
            let n = cpu.read8(o + NUMBER) as u32;
            nested!(cpu, 0x0608_98cc, push_beam_set(cpu, n, o + mb_part(7), o + MB_PUSH_AT));
        }
        let c = cpu.read16(o + MB_COUNT) as u32;
        if c & 1 != 0 {
            steam(cpu, 6, (c >> 1) & 3, MB_POINTS_B, 0xc000);
        }
    }
    let y = cpu.read32(o + POSITION + 4).wrapping_add(0xffff_e800);
    cpu.write32(o + POSITION + 4, y);
    let b = cpu.read8(o + MB_BLASTS) as u32;
    if b <= 7 && cpu.read16(o + MB_COUNT) & 15 == 0 {
        let (part, table, points, turn) =
            if cpu.read8(o + MB_SIDE) & 1 == 0 { (2, MB_BLASTS_A, MB_POINTS_A, 0) } else { (6, MB_BLASTS_B, MB_POINTS_B, 0x8000) };
        if cpu.read16(o + mb_part(part)) & MB_BROKEN == 0 {
            let k = cpu.read8(table + b) as u32;
            let place = k * 12 + points + o;
            let n = cpu.read8(o + NUMBER) as u32;
            cpu.nested_with_stack(&PROLOGUE, 0x0608_95bc, &[o + TURN_Z], |cpu| {
                blast_set(cpu, n, turn, o + mb_part(part), place, o + TURN_Z)
            });
        }
        let b = cpu.read8(o + MB_BLASTS).wrapping_add(1);
        cpu.write8(o + MB_BLASTS, b);
    }
    let c = cpu.read16(o + MB_COUNT) as u32;
    let m = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, c, 80)) & 0xffff;
    if m > 69 {
        let a = cpu.read16(o + TURN_Z).wrapping_add(cpu.read16(o + MB_GUN)).wrapping_add(0x8000) as u32;
        let rank = cpu.read8(RANK);
        if rank != 0 {
            let shot = |cpu: &mut Cpu, angle: u32| {
                nested!(cpu, 0x0608_96c8, pulse_shot(cpu, o, angle & 0xffff));
            };
            match m {
                71 => shot(cpu, a),
                75 => {
                    shot(cpu, a.wrapping_add(0xffff_f400));
                    shot(cpu, a.wrapping_add(0xc00));
                }
                79 if rank as i8 > 1 => {
                    shot(cpu, a.wrapping_add(0xffff_e800));
                    shot(cpu, a.wrapping_add(0x1800));
                }
                _ => {}
            }
        }
    } else if m == 40 {
        nested!(cpu, 0x0608_9f54, fan_shots(cpu, o));
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 78));
    }
    if mb_flags(cpu, o) & MB_SCRIPT_END != 0 {
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
        cpu.write8(o + MB_SIDE, (r & 1) as u8);
        cpu.write8(o + MB_STATE, 12);
        mb_script(cpu, o, MB_SCRIPT_BACK);
    }
}

/// State 21: the jet lights: its gun back; smoke from the nozzle (drift effects, colours 0x40
/// on (flipped at random), the script at 0x06073144, 184 dots nearer, at random angles, speed
/// 8/65536, growing with the frame); at its script's beat the jet is on (state 22) with a cross
/// flash at the nozzle.
const MB_SMOKE_SCRIPT: u32 = 0x0607_3144;
const MB_SMOKE_COLOUR: u16 = 64;
const MB_SMOKE_NEARER: u32 = 0xff48_0000;
const MB_JET_START_SIZE: u16 = 0x200;

pub fn mb_jet_lighting(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_a3b4, mb_gun_back(cpu, o));
    let t = cpu.stack();
    cpu.write32(t + actors::E_SCRIPT, MB_SMOKE_SCRIPT);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write16(t + actors::E_COLOUR, ((r & 0x8000) + MB_SMOKE_COLOUR as u32) as u16);
    let x = cpu.read32(o + MB_NOZZLE);
    cpu.write32(t + actors::E_POSITION, x);
    let y = cpu.read32(o + MB_NOZZLE + 4);
    cpu.write32(t + actors::E_POSITION + 4, y);
    let z = cpu.read32(o + MB_NOZZLE + 8).wrapping_add(MB_SMOKE_NEARER);
    cpu.write32(t + actors::E_POSITION + 8, z);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write16(t + actors::E_ANGLE, r as u16);
    cpu.write32(t + actors::E_SPEED, 8);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    let c = cpu.read16(o + MB_COUNT) as u32;
    if r & 3 != 0 {
        cpu.write32(t + actors::E_SCALES, c << 9);
        cpu.write32(t + actors::E_SCALES + 4, c << 8);
    } else {
        cpu.write32(t + actors::E_SCALES, c.wrapping_mul(0x80));
        cpu.write32(t + actors::E_SCALES + 4, c * 0x1c0);
    }
    nested!(cpu, 0x0605_7454, actors::drift_effect(cpu, t));
    if mb_flags(cpu, o) & MB_BEAT != 0 {
        let s = cpu.read8(o + MB_STATE).wrapping_add(1);
        cpu.write8(o + MB_STATE, s);
        cpu.write8(o + MB_JET_ON, 1);
        cpu.write32(o + MB_ROUND, 0);
        cpu.write16(o + MB_JET_SIZE, MB_JET_START_SIZE);
        let x = cpu.read32(o + MB_NOZZLE);
        cpu.write32(t + actors::E_POSITION, x);
        let y = cpu.read32(o + MB_NOZZLE + 4);
        cpu.write32(t + actors::E_POSITION + 4, y);
        cpu.write32(t + actors::E_POSITION + 8, 0);
        cpu.write16(t + actors::E_COLOUR, MB_SMOKE_COLOUR);
        cpu.write32(t + actors::E_SCALES + 4, 0x2_0000);
        cpu.write16(t + actors::E_ANGLE, 0x8000);
        nested!(cpu, 0x0605_7e38, explosions::cross_flash(cpu, t));
    }
}

/// State 22: round it goes on its jet (sound 77 every 15 frames), on an ellipse 116 dots across
/// and 64 down by its turn: the turn speeding to 1/128 of a turn a frame and the jet growing to
/// 0x100 for most of the way round, the last eighth slowing and the jet dying; once round it
/// wanders again. The jet flame (jet_flame_set's: 24 dots back) while it has a size.
const MB_ROUND_SLOWS: u32 = 0xe000;
const MB_ROUND_X: i32 = 116;
const MB_ROUND_Y: i32 = 64;

pub fn mb_circling(cpu: &mut Cpu, o: u32) {
    let c = cpu.read16(o + MB_COUNT) as u32;
    if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, c, 15)) == 0 {
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 77));
    }
    if mb_flags(cpu, o) & MB_SCRIPT_END != 0 {
        mb_script(cpu, o, MB_SCRIPT_CIRCLE);
    }
    let p = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + 0x10));
    nested!(cpu, 0x0604_5e30, aim::angle_to(cpu, o, p));
    let (speed_to, size_to, size_step) = if cpu.read32(o + MB_ROUND) > MB_ROUND_SLOWS { (1, 0, 3) } else { (0x80, 0x100, 8) };
    let v = cpu.read16(o + MB_TURN_Z_SPEED) as i16 as u32;
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, speed_to, 1));
    cpu.write16(o + MB_TURN_Z_SPEED, v as u16);
    let s = cpu.read16(o + MB_JET_SIZE) as i16 as u32;
    let s = nested!(cpu, 0x0604_6168, actors::approach(cpu, s, size_to, size_step));
    cpu.write16(o + MB_JET_SIZE, s as u16);
    let round = cpu.read32(o + MB_ROUND).wrapping_add(cpu.read16(o + MB_TURN_Z_SPEED) as i16 as u32);
    cpu.write32(o + MB_ROUND, round);
    if round > 0x1_0000 {
        cpu.write8(o + MB_JET_ON, 0);
        cpu.write16(o + MB_TURN_Z_SPEED, 0);
        cpu.write16(o + TURN_Z, 0);
        nested!(cpu, 0x0608_a104, mb_to_wander(cpu, o));
    }
    let a = cpu.read16(o + TURN_Z) as i16 as i32 as u32;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a)) as i32;
    cpu.write32(o + POSITION, s.wrapping_mul(MB_ROUND_X) as u32);
    let a = cpu.read16(o + TURN_Z) as i16 as i32 as u32;
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a)) as i32;
    cpu.write32(o + POSITION + 4, c.wrapping_neg().wrapping_mul(MB_ROUND_Y) as u32);
    let reach = cpu.read16(o + MB_JET_REACH);
    if reach as i16 != 0 {
        cpu.write16(o + MB_JET_LAST_REACH, reach);
        cpu.write16(o + MB_JET_REACH, 0);
    } else {
        cpu.write16(o + MB_JET_LAST_REACH, 0);
    }
    let t = cpu.stack();
    let n = cpu.read8(o + NUMBER);
    cpu.write8(t + S_NUMBER, n);
    cpu.write16(t + D_BACK, 24);
    let size = cpu.read16(o + MB_JET_SIZE);
    cpu.write16(t + D_SIZE, size);
    if cpu.read16(o + MB_JET_SIZE) != 0 {
        nested!(cpu, 0x0608_9334, jet_flame_set(cpu, t, o, o + MB_JET_REACH));
    }
}

/// State 23: beaten. It slows and tips over, an explosion after 60 frames, sinks away into the
/// distance and is gone after 210.
pub fn mb_beaten(cpu: &mut Cpu, o: u32) {
    let v = cpu.read16(o + MB_TURN_X_SPEED) as i16 as u32;
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0xffff_f400, 8));
    cpu.write16(o + MB_TURN_X_SPEED, v as u16);
    let v = cpu.read16(o + MB_TURN_Z_SPEED) as i16 as u32;
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0x200, 1));
    cpu.write16(o + MB_TURN_Z_SPEED, v as u16);
    for k in [0, 4] {
        let v = cpu.read32(o + MB_SPEED + k);
        let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0, 0x1000));
        cpu.write32(o + MB_SPEED + k, v);
    }
    let n = cpu.read16(o + MB_BEATEN_FOR).wrapping_add(1);
    cpu.write16(o + MB_BEATEN_FOR, n);
    if n as i16 == 60 {
        let t = cpu.stack();
        for k in [0, 4, 8] {
            let v = cpu.read32(o + POSITION + k);
            cpu.write32(t + k, v);
        }
        cpu.write32(t + 60, o);
        nested!(cpu, 0x0605_8ba4, explosions::explosion_set(cpu, t));
    }
    let z = cpu.read32(o + POSITION + 8).wrapping_add(cpu.read32(o + MB_SPEED + 8));
    cpu.write32(o + POSITION + 8, z);
    let v = cpu.read32(o + MB_SPEED + 8);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0x4_0000, 0x100));
    cpu.write32(o + MB_SPEED + 8, v);
    if cpu.read16(o + MB_BEATEN_FOR) as i16 > 210 {
        cpu.write8(o + OBJ_MODE, TO_STOP);
    }
}

/// The mid-boss's behaviour. Its life on the boss gauge; beaten (its life gone, or its plan done)
/// before state 23: state 23, the bullets cleared, its core broken, the stage's score (10000 and
/// rate 40 if shot down) and rate (10 for each part broken, 6 and 4 for the middle ones), every
/// life cleared. Each part whose life is gone is broken (shutting the parts it carries); its
/// state's function; its pose; its moves and turns; its gun's turn kept between its least and
/// most, those eased back to a quarter turn; broken parts smoke and spark at their points until
/// done; its count.
const MB_GUN_LIMIT: u32 = 0x4000;
const MB_HALF_LIFE: i32 = 30000;
/// Each broken part's smoke: the part, the points it smokes at, how many points (past the
/// first); 6 sparks at each (the SH-2 code passes a third argument, 4 or 8, that is not read).
const MB_SMOKES: [(u32, u32, u32); 7] =
    [(1, 0x234, 2), (2, 0x258, 18), (3, 0x33c, 4), (4, 0x378, 2), (5, 0x39c, 2), (6, 0x3c0, 1), (7, 0x3d8, 2)];

fn mb_break(cpu: &mut Cpu, o: u32, part: u32) {
    let s = cpu.read8(o + MB_PART_SMOKE + part).wrapping_add(1);
    cpu.write8(o + MB_PART_SMOKE + part, s);
    mb_parts_or(cpu, o, &[part], MB_BROKEN);
}

fn mb_life(cpu: &mut Cpu, o: u32, k: u32) -> i32 {
    cpu.read32(o + MB_LIVES + 4 * k) as i32
}

pub fn mb_behaviour(cpu: &mut Cpu, o: u32) {
    let life = cpu.read32(o + MB_LIVES);
    nested!(cpu, 0x0604_64b8, hud::boss_gauge_life(cpu, life));
    if cpu.read8(o + MB_STATE) <= 22 && (mb_life(cpu, o, 0) <= 0 || cpu.read16(o + MB_PLAN_DONE) != 0) {
        cpu.write8(o + MB_STATE, 23);
        nested!(cpu, 0x0605_61e0, screen::bullets_cleared(cpu));
        mb_parts_or(cpu, o, &[0], MB_BROKEN);
        cpu.write16(o + MB_BEATEN_FOR, 0);
        cpu.write32(STAGE_RATE_AT, 0);
        if mb_life(cpu, o, 0) <= 0 {
            cpu.write32(STAGE_SCORE_AT, 10000);
            cpu.write32(STAGE_RATE_AT, 40);
        } else {
            cpu.write32(STAGE_SCORE_AT, 0);
        }
        let add = |cpu: &mut Cpu, n: u32| {
            let r = cpu.read32(STAGE_RATE_AT).wrapping_add(n);
            cpu.write32(STAGE_RATE_AT, r);
        };
        if mb_life(cpu, o, 1) <= 0 {
            add(cpu, 10);
        }
        if mb_life(cpu, o, 2) <= 0 {
            add(cpu, 6);
        }
        if mb_life(cpu, o, 2) <= MB_HALF_LIFE {
            add(cpu, 4);
        }
        for k in 3..7 {
            if mb_life(cpu, o, k) <= 0 {
                add(cpu, 10);
            }
        }
        for k in 0..7 {
            cpu.write32(o + MB_LIVES + 4 * k, 0);
        }
    }
    let unbroken = |cpu: &mut Cpu, part: u32| cpu.read8(o + MB_PART_SMOKE + part) == 0;
    if unbroken(cpu, 1) && mb_life(cpu, o, 1) <= 0 {
        mb_break(cpu, o, 1);
    }
    if unbroken(cpu, 2) && mb_life(cpu, o, 2) <= 0 {
        mb_break(cpu, o, 2);
        if unbroken(cpu, 3) {
            mb_break(cpu, o, 3);
        }
    }
    if unbroken(cpu, 3) && mb_life(cpu, o, 2) <= MB_HALF_LIFE {
        mb_break(cpu, o, 3);
    }
    if unbroken(cpu, 4) && mb_life(cpu, o, 3) <= 0 {
        mb_break(cpu, o, 4);
    }
    if unbroken(cpu, 5) && mb_life(cpu, o, 4) <= 0 {
        mb_break(cpu, o, 5);
        if unbroken(cpu, 6) {
            mb_break(cpu, o, 6);
        }
        if unbroken(cpu, 7) {
            mb_break(cpu, o, 7);
        }
    }
    if unbroken(cpu, 6) && mb_life(cpu, o, 5) <= 0 {
        mb_break(cpu, o, 6);
        if unbroken(cpu, 7) {
            mb_break(cpu, o, 7);
        }
    }
    if unbroken(cpu, 7) && mb_life(cpu, o, 6) <= 0 {
        mb_break(cpu, o, 7);
    }
    let state = cpu.read8(o + MB_STATE) as u32;
    match state {
        0 => nested!(cpu, 0x0608_a5cc, mb_rising(cpu, o)),
        1 => nested!(cpu, 0x0608_a640, mb_risen(cpu, o)),
        2 => nested!(cpu, 0x0608_a694, mb_idle(cpu, o)),
        3 => nested!(cpu, 0x0608_a714, mb_diving(cpu, o)),
        4 => nested!(cpu, 0x0608_a794, mb_sinking(cpu, o)),
        12 => nested!(cpu, 0x0608_a97c, mb_stopping(cpu, o)),
        13 => nested!(cpu, 0x0608_a9e4, mb_wandering(cpu, o)),
        14 => nested!(cpu, 0x0608_aa54, mb_going_left(cpu, o)),
        15 => nested!(cpu, 0x0608_aaac, mb_going_right(cpu, o)),
        16 => nested!(cpu, 0x0608_ab04, mb_halting(cpu, o)),
        17 => nested!(cpu, 0x0608_b150, mb_one_opening(cpu, o)),
        18 => nested!(cpu, 0x0608_b1d8, mb_one_open(cpu, o)),
        19 => nested!(cpu, 0x0608_aca4, mb_volleys_aim(cpu, o)),
        20 => nested!(cpu, 0x0608_acf8, mb_volleys_fire(cpu, o)),
        21 => nested!(cpu, 0x0608_b530, mb_jet_lighting(cpu, o)),
        22 => nested!(cpu, 0x0608_b654, mb_circling(cpu, o)),
        23 => nested!(cpu, 0x0608_b7e8, mb_beaten(cpu, o)),
        _ => {
            let f = cpu.read32(MB_STATES + 4 * state);
            cpu.call(f, &[o]);
        }
    }
    nested!(cpu, 0x0605_6644, models::pose_step(cpu, o + MB_POSE));
    for k in [0, 4] {
        let p = cpu.read32(o + POSITION + k).wrapping_add(cpu.read32(o + MB_SPEED + k));
        cpu.write32(o + POSITION + k, p);
    }
    for (turn, speed) in [(TURN_X, MB_TURN_X_SPEED), (TURN_X + 2, MB_TURN_Y_SPEED), (TURN_Z, MB_TURN_Z_SPEED)] {
        let t = cpu.read16(o + turn).wrapping_add(cpu.read16(o + speed));
        cpu.write16(o + turn, t);
    }
    let s = cpu.read16(o + MB_SPIN).wrapping_add(cpu.read16(o + MB_SPIN_SPEED));
    cpu.write16(o + MB_SPIN, s);
    let g = cpu.read16(o + MB_GUN).wrapping_add(cpu.read16(o + MB_GUN_SPEED));
    cpu.write16(o + MB_GUN, g);
    let least = cpu.read16(o + MB_GUN_LEAST);
    if (g as i16) < least as i16 {
        cpu.write16(o + MB_GUN, least);
    }
    let most = cpu.read16(o + MB_GUN_MOST);
    if cpu.read16(o + MB_GUN) as i16 > most as i16 {
        cpu.write16(o + MB_GUN, most);
    }
    let l = cpu.read16(o + MB_GUN_LEAST) as i16 as u32;
    let l = nested!(cpu, 0x0604_6168, actors::approach(cpu, l, MB_GUN_LIMIT.wrapping_neg(), 0x40));
    cpu.write16(o + MB_GUN_LEAST, l as u16);
    let m = cpu.read16(o + MB_GUN_MOST) as i16 as u32;
    let m = nested!(cpu, 0x0604_6168, actors::approach(cpu, m, MB_GUN_LIMIT, 0x40));
    cpu.write16(o + MB_GUN_MOST, m as u16);
    for &(part, points, last) in &MB_SMOKES {
        let s = cpu.read8(o + MB_PART_SMOKE + part) as u32;
        if s != 0 && s - 1 <= last {
            let at = (s - 1) * 12 + points + o;
            let made = nested!(cpu, 0x0605_968c, explosions::smoke_and_sparks(cpu, at, 12));
            if made as u8 != 0 {
                let s = cpu.read8(o + MB_PART_SMOKE + part).wrapping_add(1);
                cpu.write8(o + MB_PART_SMOKE + part, s);
            }
        }
    }
    let c = cpu.read16(o + MB_COUNT).wrapping_add(1);
    cpu.write16(o + MB_COUNT, c);
}

/// The mid-boss set up as `n`: drawn by its own code, 320 dots down the screen and 64 deep,
/// tilted up a quarter turn, rising at 5.5 dots a frame (state 0); its counts, turns, gun and jet
/// cleared; its 8 parts (parts::parts_set's: their point counts, points and tables) shut to hits,
/// with their lives (part 3 sharing part 2's); its pose (16 joints, at +0x98, their speeds at
/// +0xb8, its model's joints at 0x060a3c4c), posed and idling; the boss gauge for its life.
const MB_BEHAVIOUR: u32 = 0x0608_b8c4;
const MB_DRAW: u32 = 0x0608_be60;
const MB_FLAGS: u32 = 0xc8_0000;
const MB_START_Y: u32 = 0xfec0_0000;
const MB_START_Z: u32 = 0x40_0000;
const MB_RISE: u32 = 0x5_8000;
const MB_LIFE: u32 = 70000;
/// Two words it clears (+0x4bc, +0x4be).
const MB_4BC: u32 = 0x4bc;
const MB_POSE_JOINTS: u16 = 16;
const MB_POSE_MODEL: u32 = 0x060a_3c4c;
const MB_POSE_START: u32 = 0x060a_368c;
/// Each part: its point count, its points, its life (and where), its table.
type MbPart = (u32, u32, Option<(u32, u32)>, u32);
const MB_PARTS: [MbPart; 8] = [
    (19, 0x150, Some((0x134, 70000)), 0x060a_34a4),
    (3, 0x234, Some((0x138, 35000)), 0x060a_353c),
    (19, 0x258, Some((0x13c, 40000)), 0x060a_3554),
    (5, 0x33c, None, 0x060a_35ec),
    (3, 0x378, Some((0x140, 35000)), 0x060a_3614),
    (3, 0x39c, Some((0x144, 50000)), 0x060a_362c),
    (2, 0x3c0, Some((0x148, 45000)), 0x060a_3644),
    (3, 0x3d8, Some((0x14c, 30000)), 0x060a_3654),
];

pub fn mb_set(cpu: &mut Cpu, o: u32, n: u32) {
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + 0x10));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HITS_AT));
    cpu.write32(o + BEHAVIOUR, MB_BEHAVIOUR);
    let f = cpu.read32(o + FLAGS) | MB_FLAGS;
    cpu.write32(o + FLAGS, f);
    cpu.write32(o + MODEL_DRAW, MB_DRAW);
    cpu.write8(o + NUMBER, n as u8);
    let k = cpu.read8(o + NUMBER) as i8 as i32 as u32;
    cpu.write32(NUMBERED.wrapping_add(k << 2), o);
    cpu.write8(o + MB_STATE, 0);
    cpu.write32(o + POSITION, 0);
    cpu.write32(o + POSITION + 4, MB_START_Y);
    cpu.write32(o + POSITION + 8, MB_START_Z);
    cpu.write16(o + TURN_X, 0xc000);
    cpu.write32(o + MB_SPEED, 0);
    cpu.write32(o + MB_SPEED + 4, MB_RISE);
    cpu.write32(o + MB_SPEED + 8, 0);
    for at in (MB_TURN_X_SPEED..=0x118).step_by(2) {
        cpu.write16(o + at, 0);
    }
    for at in [MB_COUNT, MB_BEATEN_FOR, 0xf2, MB_PLAN_AT, MB_WAIT, MB_PLAN_DONE] {
        cpu.write16(o + at, 0);
    }
    for at in (MB_SPIN..=MB_GUN_MOST).step_by(2) {
        cpu.write16(o + at, 0);
    }
    cpu.write8(o + MB_SIDE, 0);
    for at in [MB_JET_REACH, MB_JET_LAST_REACH, MB_JET_SIZE] {
        cpu.write16(o + at, 0);
    }
    cpu.write8(o + MB_JET_ON, 0);
    cpu.write8(o + MB_OPEN_A, 0);
    cpu.write8(o + MB_OPEN_B, 0);
    // the SH-2 code keeps the shared life's address in its frame
    let t = cpu.stack();
    let mut life_at = 0;
    for (k, &(count, points, life, table)) in MB_PARTS.iter().enumerate() {
        let k = k as u32;
        cpu.write16(o + mb_part(k), MB_UNHIT);
        cpu.write8(o + MB_PART_SMOKE + k, 0);
        if k == 0 {
            cpu.write16(o + MB_4BC, 0);
            cpu.write16(o + MB_4BC + 2, 0);
        }
        if let Some((at, value)) = life {
            life_at = o + at;
            if k == 2 {
                cpu.write32(t, life_at);
            }
            cpu.write32(life_at, value);
        }
        let number = cpu.read8(o + NUMBER) as u32;
        cpu.push(&[table, number]);
        nested!(cpu, 0x0605_6ea4, parts::parts_set(cpu, count, o + points, o + mb_part(k), life_at, table, number));
        if k == 3 {
            cpu.pop(32);
        }
    }
    cpu.write16(o + MB_POSE + 0x10, MB_POSE_JOINTS);
    cpu.write32(o + MB_POSE + 0x14, o + 0x98);
    cpu.write32(o + MB_POSE + 0x18, o + 0xb8);
    cpu.write32(o + MB_POSE + 0x1c, MB_POSE_MODEL);
    cpu.pop(32);
    nested!(cpu, 0x0605_65f0, models::pose_set(cpu, o + MB_POSE, MB_POSE_START));
    mb_script(cpu, o, MB_SCRIPT_IDLE);
    nested!(cpu, 0x0605_6644, models::pose_step(cpu, o + MB_POSE));
    nested!(cpu, 0x0604_64d8, hud::boss_gauge_set(cpu, MB_LIFE));
}

/// The mid-boss made (its number, or 0 when there is no room).
const MB_SIZE: u32 = 0xdf4;

pub fn mb_make(cpu: &mut Cpu) -> u32 {
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, 2)) as i8;
    if n == -1 {
        return 0;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, MB_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, NUMBERED_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0608_ca20, mb_set(cpu, o, n as i32 as u32));
    n as i32 as u32
}

/// The mid-boss's draw (`s` its sprite part, the actor 0x10 before). Its parts' flags first:
/// bit 1 (drawn hit last time) clears bit 0 and itself, bit 0 (hit) sets bit 1; part 3's hit
/// shows on part 2. Then its model (kind 53's) by its pose (the joints' angles at +0x80): the
/// body and its points, the jet's nozzle, the two arms of parts 1 to 3 and the gun's points,
/// the other arm of parts 4 to 7 with the blasts' points; a part drawn hit (bit 0), broken
/// (bit 6: some drawn as wreckage) or not at all (bit 7). The matrices of parts 3, 6 and 7 are
/// kept for 16 frames (a ring by the frame, at +0x4d8, +0x7d8, +0xad8). Glows (kind 54's sprite)
/// at the joints, 176 dots nearer; and on alternate loops, the open side's parts drawn again
/// where they were 2, 4 and 6 frames before (an after-image).
const MB_MODEL_KIND: u32 = 53;
const MB_GLOW_KIND: u8 = 54;
const MB_GLOW_NEARER: u32 = 0xff50_0000;
const MB_DRAWN_HIT: u16 = 2;
const MB_HIT: u16 = 1;
const MB_HIDDEN: u16 = 0x80;
const MB_TRAIL_3: u32 = 0x4d8;
const MB_TRAIL_6: u32 = 0x7d8;
const MB_TRAIL_7: u32 = 0xad8;
const SL_GET_MATRIX: u32 = 0x0606_db9c;
const SL_LOAD_MATRIX: u32 = 0x0606_db7c;
/// The points of each part (their tables of x, y, z words and counts).
const MB_BODY_POINTS: (u32, u32) = (0x060a_34a4, 19);
const MB_ARM_POINTS: (u32, u32) = (0x060a_353c, 3);
const MB_SHOULDER_POINTS: (u32, u32) = (0x060a_3554, 19);
const MB_GUN_POINTS: (u32, u32) = (0x060a_35ec, 5);
const MB_ARM_B_POINTS: (u32, u32) = (0x060a_3614, 3);
const MB_ELBOW_POINTS: (u32, u32) = (0x060a_362c, 3);
const MB_HAND_A_POINTS: (u32, u32) = (0x060a_3644, 2);
const MB_HAND_B_POINTS: (u32, u32) = (0x060a_3654, 3);

/// Kind 53's model `n`, its hit or plain one by `flags`' bit 0.
fn mb_put(cpu: &mut Cpu, n: u32, flags: u16) {
    let file = cpu.read32(kind(MB_MODEL_KIND));
    let models = cpu.read32(file + F_MODELS);
    let pair = cpu.read32(models + 4 * n);
    let model = cpu.read32(pair + 4 * (flags & 1) as u32);
    cpu.call(SL_PUT_POLYGON, &[model]);
}

/// Kind 53's model `n` (its first).
fn mb_put_first(cpu: &mut Cpu, n: u32) {
    let file = cpu.read32(kind(MB_MODEL_KIND));
    let models = cpu.read32(file + F_MODELS);
    let pair = cpu.read32(models + 4 * n);
    let model = cpu.read32(pair);
    cpu.call(SL_PUT_POLYGON, &[model]);
}

/// A part drawn unless hidden: model `n` (`broken`'s when broken, if it has one).
fn mb_put_part(cpu: &mut Cpu, flags: u16, n: u32, broken: u32) {
    if flags & MB_HIDDEN == 0 {
        if flags & MB_BROKEN == 0 {
            mb_put(cpu, n, flags);
        } else {
            mb_put_first(cpu, broken);
        }
    }
}

/// A joint's angle (the pose's word `k`).
fn mb_joint(cpu: &mut Cpu, o: u32, k: u32) -> u32 {
    let angles = cpu.read32(o + MB_POSE + 0x14);
    cpu.read16(angles + 2 * k) as i16 as i32 as u32
}

fn mb_trail_at(cpu: &mut Cpu, o: u32, ring: u32, back: u32) -> u32 {
    let f = cpu.read32(GAME_FRAMES).wrapping_sub(back) & 15;
    o + ring + f * 48
}

pub fn mb_draw(cpu: &mut Cpu, s: u32) {
    let o = s - 0x10;
    for k in 0..8 {
        let at = o + mb_part(k);
        let mut f = cpu.read16(at);
        if f & MB_DRAWN_HIT != 0 {
            cpu.write16(at, f & 0xfffc);
            f = cpu.read16(at);
        }
        if f & MB_HIT != 0 {
            cpu.write16(at, f | MB_DRAWN_HIT);
        }
    }
    let f = (cpu.read16(o + mb_part(3)) & 1) | cpu.read16(o + mb_part(2));
    cpu.write16(o + mb_part(2), f);
    let t = cpu.stack();
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, t));
    cpu.write8(t + 0x1f, 0);
    cpu.write8(t + 0x1e, MB_GLOW_KIND);
    let c = cpu.read16(kind(MB_GLOW_KIND as u32) + K_COLOUR);
    cpu.write16(t + 0x18, c);
    cpu.write8(t + 0x23, 0);
    cpu.call(SL_PUSH_MATRIX, &[]);
    let p = [0, 4, 8].map(|k| cpu.read32(o + POSITION + k));
    cpu.call(SL_TRANSLATE, &p);
    let turn = |cpu: &mut Cpu, field: u32, joint: u32| {
        let a = cpu.read16(o + field).wrapping_add(mb_joint(cpu, o, joint) as u16);
        a as i16 as i32 as u32
    };
    if cpu.read8(o + MB_JET_ON) == 0 {
        let a = turn(cpu, TURN_X, 3);
        cpu.call(SL_ROT_X, &[a]);
        let a = turn(cpu, TURN_Z, 5);
        cpu.call(SL_ROT_Z, &[a]);
    } else {
        let a = turn(cpu, TURN_Z, 5);
        cpu.call(SL_ROT_Z, &[a]);
        let a = turn(cpu, TURN_X, 3);
        cpu.call(SL_ROT_X, &[a]);
    }
    let a = turn(cpu, TURN_X + 2, 4);
    cpu.call(SL_ROT_Y, &[a]);
    cpu.call(SL_PUSH_MATRIX, &[]);
    let place = [0, 1, 2].map(|k| mb_joint(cpu, o, k) << 8);
    cpu.call(SL_TRANSLATE, &place);
    cpu.call(SL_PUSH_MATRIX, &[]);
    let a = cpu.read16(o + MB_SPIN) as i16 as i32 as u32;
    cpu.call(SL_ROT_X, &[a]);
    let a = cpu.read16(o + MB_GUN) as i16 as i32 as u32;
    cpu.call(SL_ROT_Z, &[a]);
    gs_points(cpu, MB_BODY_POINTS, o + 0x150);
    let f = cpu.read16(o + mb_part(0));
    if f & MB_HIDDEN == 0 {
        mb_put(cpu, 8, f);
    }
    cpu.call(SL_CALC_POINT, &[0, 0x18_0000, 0, o + MB_NOZZLE]);
    cpu.call(SL_POP_MATRIX, &[]);
    // the first arm: parts 1, 2 and 3
    cpu.call(SL_PUSH_MATRIX, &[]);
    cpu.call(SL_CALC_POINT, &[0xffee_0000, 0xfff2_0000, 0, t + 44]);
    cpu.call(SL_TRANSLATE, &[0xffe6_0000, 0xfff6_0000, 0]);
    let a = mb_joint(cpu, o, 6);
    cpu.call(SL_ROT_X, &[a]);
    let a = mb_joint(cpu, o, 7);
    cpu.call(SL_ROT_Z, &[a]);
    cpu.call(SL_PUSH_MATRIX, &[]);
    cpu.call(SL_TRANSLATE, &[0xffe4_0000, 0, 0]);
    gs_points(cpu, MB_ARM_POINTS, o + 0x234);
    let f = cpu.read16(o + mb_part(1));
    mb_put_part(cpu, f, 6, 12);
    cpu.call(SL_CALC_POINT, &[0xffe0_0000, 0, 0, t + 56]);
    cpu.call(SL_PUSH_MATRIX, &[]);
    cpu.call(SL_TRANSLATE, &[0xffe4_0000, 0, 0]);
    let a = mb_joint(cpu, o, 9);
    cpu.call(SL_ROT_Z, &[a]);
    let a = mb_joint(cpu, o, 8);
    cpu.call(SL_ROT_X, &[a]);
    cpu.call(SL_PUSH_MATRIX, &[]);
    cpu.call(SL_TRANSLATE, &[0xfff0_0000, 0x40_0000, 0]);
    gs_points(cpu, MB_SHOULDER_POINTS, o + 0x258);
    let f2 = cpu.read16(o + mb_part(2));
    if f2 & MB_HIDDEN == 0 {
        if cpu.read16(o + mb_part(3)) & (MB_BROKEN | MB_HIDDEN) == 0 {
            mb_put(cpu, 0, f2);
            gs_points(cpu, MB_GUN_POINTS, o + 0x33c);
            let m = mb_trail_at(cpu, o, MB_TRAIL_3, 0);
            cpu.call(SL_GET_MATRIX, &[m]);
        } else if f2 & MB_BROKEN == 0 {
            mb_put(cpu, 9, f2);
            let m = mb_trail_at(cpu, o, MB_TRAIL_3, 0);
            cpu.call(SL_GET_MATRIX, &[m]);
        } else {
            mb_put_first(cpu, 10);
        }
        if cpu.read16(o + mb_part(2)) & MB_BROKEN == 0 {
            for i in 0..8u32 {
                let y = (i * 3) << 18;
                cpu.call(SL_CALC_POINT, &[0x10_0000, y.wrapping_add(0xffe0_0000), 0xfff8_0000, o + MB_POINTS_A + 12 * i]);
            }
            cpu.call(SL_PUSH_MATRIX, &[]);
            cpu.call(SL_TRANSLATE, &[0xffdc_0000, 0xffcc_0000, 0xffee_0000]);
            // the SH-2 code keeps the kind's and the part's offsets in its frame
            cpu.write32(t + 116, MB_MODEL_KIND * 16);
            cpu.write32(t + 120, mb_part(2));
            let f = cpu.read16(o + mb_part(2));
            mb_put(cpu, 1, f);
            cpu.call(SL_POP_MATRIX, &[]);
            cpu.call(SL_PUSH_MATRIX, &[]);
            cpu.call(SL_TRANSLATE, &[0xffdc_0000, 0xffcc_0000, 0x12_0000]);
            let f = cpu.read16(o + mb_part(2));
            mb_put(cpu, 2, f);
            cpu.call(SL_POP_MATRIX, &[]);
        }
    }
    for _ in 0..4 {
        cpu.call(SL_POP_MATRIX, &[]);
    }
    // the other arm: parts 4 to 7
    cpu.call(SL_CALC_POINT, &[0x12_0000, 0xfff2_0000, 0, t + 68]);
    cpu.call(SL_PUSH_MATRIX, &[]);
    cpu.call(SL_TRANSLATE, &[0x1a_0000, 0xfff6_0000, 0]);
    let a = mb_joint(cpu, o, 10);
    cpu.call(SL_ROT_X, &[a]);
    let a = mb_joint(cpu, o, 11);
    cpu.call(SL_ROT_Z, &[a]);
    cpu.call(SL_PUSH_MATRIX, &[]);
    cpu.call(SL_TRANSLATE, &[0x1c_0000, 0, 0]);
    gs_points(cpu, MB_ARM_B_POINTS, o + 0x378);
    let f = cpu.read16(o + mb_part(4));
    mb_put_part(cpu, f, 7, 13);
    cpu.call(SL_CALC_POINT, &[0x20_0000, 0, 0, t + 80]);
    cpu.call(SL_PUSH_MATRIX, &[]);
    cpu.call(SL_TRANSLATE, &[0x1c_0000, 0, 0]);
    let a = mb_joint(cpu, o, 13);
    cpu.call(SL_ROT_Z, &[a]);
    let a = mb_joint(cpu, o, 12);
    cpu.call(SL_ROT_X, &[a]);
    cpu.call(SL_PUSH_MATRIX, &[]);
    cpu.call(SL_TRANSLATE, &[0x24_0000, 0, 0]);
    gs_points(cpu, MB_ELBOW_POINTS, o + 0x39c);
    let f = cpu.read16(o + mb_part(5));
    mb_put_part(cpu, f, 5, 11);
    cpu.call(SL_CALC_POINT, &[0xfff4_0000, 0x1c_0000, 0, t + 92]);
    cpu.call(SL_PUSH_MATRIX, &[]);
    cpu.call(SL_TRANSLATE, &[0xfff4_0000, 0x1c_0000, 0]);
    let a = mb_joint(cpu, o, 14);
    cpu.call(SL_ROT_Z, &[a]);
    cpu.call(SL_PUSH_MATRIX, &[]);
    cpu.call(SL_TRANSLATE, &[0, 0x1e_0000, 0]);
    gs_points(cpu, MB_HAND_A_POINTS, o + 0x3c0);
    let f = cpu.read16(o + mb_part(6));
    if f & (MB_BROKEN | MB_HIDDEN) == 0 {
        mb_put(cpu, 4, f);
        let m = mb_trail_at(cpu, o, MB_TRAIL_6, 0);
        cpu.call(SL_GET_MATRIX, &[m]);
        for i in 0..4u32 {
            let y = (i * 3) << 18;
            cpu.call(SL_CALC_POINT, &[0xffe4_0000, y.wrapping_add(0xfff0_0000), 0xfff8_0000, o + MB_POINTS_B + 12 * i]);
        }
    }
    cpu.call(SL_CALC_POINT, &[0, 0x1c_0000, 0, t + 104]);
    cpu.call(SL_PUSH_MATRIX, &[]);
    cpu.call(SL_TRANSLATE, &[0, 0x1c_0000, 0]);
    let a = mb_joint(cpu, o, 15);
    cpu.call(SL_ROT_Z, &[a]);
    cpu.call(SL_PUSH_MATRIX, &[]);
    cpu.call(SL_TRANSLATE, &[0xfffc_0000, 0x1e_0000, 0]);
    gs_points(cpu, MB_HAND_B_POINTS, o + 0x3d8);
    let f = cpu.read16(o + mb_part(7));
    if f & (MB_BROKEN | MB_HIDDEN) == 0 {
        mb_put(cpu, 3, f);
        let m = mb_trail_at(cpu, o, MB_TRAIL_7, 0);
        cpu.call(SL_GET_MATRIX, &[m]);
        for i in 0..4u32 {
            cpu.call(SL_CALC_POINT, &[0xffe0_0000, 0, 0xfff8_0000, o + MB_PUSH_AT + 12 * i]);
        }
    }
    for _ in 0..10 {
        cpu.call(SL_POP_MATRIX, &[]);
    }
    // the glows
    let glow = |cpu: &mut Cpu, at: u32| {
        let (x, y, z) = (cpu.read32(t + at), cpu.read32(t + at + 4), cpu.read32(t + at + 8));
        cpu.write32(t + 4, x);
        cpu.write32(t + 8, y);
        cpu.write32(t + 12, z.wrapping_add(MB_GLOW_NEARER));
        nested!(cpu, 0x0604_0960, sprites::draw(cpu, t));
    };
    for at in [44, 56, 68, 80] {
        glow(cpu, at);
    }
    if cpu.read16(o + mb_part(6)) & (MB_BROKEN | MB_HIDDEN) == 0 {
        glow(cpu, 92);
    }
    if cpu.read16(o + mb_part(7)) & (MB_BROKEN | MB_HIDDEN) == 0 {
        glow(cpu, 104);
    }
    // the after-images
    let image = |cpu: &mut Cpu, ring: u32, n: u32| {
        for k in 1..=3u32 {
            cpu.call(SL_PUSH_MATRIX, &[]);
            let m = mb_trail_at(cpu, o, ring, 2 * k);
            cpu.call(SL_LOAD_MATRIX, &[m]);
            mb_put_first(cpu, n);
            cpu.call(SL_POP_MATRIX, &[]);
        }
    };
    if cpu.read32(LOOPS) & 1 != 0 {
        if cpu.read8(o + MB_OPEN_A) != 0 && cpu.read16(o + mb_part(2)) & (MB_BROKEN | MB_HIDDEN) == 0 {
            let n = if cpu.read16(o + mb_part(3)) & MB_BROKEN != 0 { 9 } else { 0 };
            image(cpu, MB_TRAIL_3, n);
        }
    } else if cpu.read8(o + MB_OPEN_B) != 0 {
        if cpu.read16(o + mb_part(7)) & (MB_BROKEN | MB_HIDDEN) == 0 {
            image(cpu, MB_TRAIL_7, 3);
        }
        if cpu.read16(o + mb_part(6)) & (MB_BROKEN | MB_HIDDEN) == 0 {
            image(cpu, MB_TRAIL_6, 4);
        }
    }
}

// ---- the flame thrower --------------------------------------------------------------------------------

/// A flame (an enemy actor, one of a stream out of its owner along the owner's angle): its owner
/// (+0x6c), the owner's flags word it ends with (+0x70: bit 6), how far out it is (a word, +0x8a)
/// and how far further each frame (+0x8c), its growth a frame (a word, +0xa8). It ends past 352
/// dots out; its animation flickers by the frame.
const FL_OWNER: u32 = 0x6c;
const FL_FLAGS_AT: u32 = 0x70;
const FL_OUT: u32 = 0x8a;
const FL_STEP: u32 = 0x8c;
const FL_GROWTH: u32 = 0xa8;
const FL_REACH: i32 = 0x160;
const FL_ENDS: u16 = 0x40;
const FL_ANIMATIONS: [u32; 2] = [0x060a_42ce, 0x060a_42d6];

pub fn flame_behaviour(cpu: &mut Cpu, o: u32) {
    let owner = cpu.read32(o + FL_OWNER);
    let out = cpu.read16(o + FL_OUT) as i16 as i32;
    let flags_at = cpu.read32(o + FL_FLAGS_AT);
    if out.abs() > FL_REACH || cpu.read16(flags_at) & FL_ENDS != 0 {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    let a = cpu.read16(owner + TURN_Z);
    cpu.write16(o + TURN_Z, a);
    let a = a as i16 as i32 as u32;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a)) as i32;
    let out = cpu.read16(o + FL_OUT) as i16 as i32;
    let v = cpu.read32(owner + POSITION).wrapping_add(s.wrapping_mul(out) as u32);
    cpu.write32(o + POSITION, v);
    let a = cpu.read16(owner + TURN_Z) as i16 as i32 as u32;
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a)) as i32;
    let out = cpu.read16(o + FL_OUT) as i16 as i32;
    let v = cpu.read32(owner + POSITION + 4).wrapping_add(c.wrapping_neg().wrapping_mul(out) as u32);
    cpu.write32(o + POSITION + 4, v);
    let z = cpu.read32(owner + POSITION + 8);
    cpu.write32(o + POSITION + 8, z);
    let out = cpu.read16(o + FL_OUT).wrapping_add(cpu.read16(o + FL_STEP));
    cpu.write16(o + FL_OUT, out);
    let animation = FL_ANIMATIONS[(cpu.read32(GAME_FRAMES) & 1) as usize];
    cpu.write32(o + M_ANIMATION, animation);
    let s = cpu.read32(o + SCALES).wrapping_add(cpu.read16(o + FL_GROWTH) as i16 as u32);
    cpu.write32(o + SCALES, s);
}

/// A flame made from the description `d` (its number +0x1e, how far out it starts a word +0x2e,
/// its size a word +0x30, its step a word +0x32) for `owner`, ending with the flags word at
/// `flags_at`: one dot out along the owner's angle, kind 0's frames 94 to 97 (by the frame),
/// colours 0x30 on, side 1; its size 16 times the description's, growing by 64 times it a frame;
/// small ones hidden and unhit.
const FL_SIZE: u32 = 0xbc;
const FL_BEHAVIOUR: u32 = 0x0608_ce10;
const FL_FRAME: u8 = 94;
const FL_COLOURS: u16 = 0x30;
const FL_SPRITE_FLAGS: u32 = 0x400;
const FL_SMALL: i32 = 0x3ff;
const FL_HIDE: u32 = 0x8_0100;
const FL_LIFT: u8 = 4;
const D_STEP: u32 = 0x32;

pub fn flame_set(cpu: &mut Cpu, d: u32, owner: u32, flags_at: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, FL_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + 0x10));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HITS_AT));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + BEHAVIOUR, FL_BEHAVIOUR);
    let n = cpu.read8(d + S_NUMBER);
    cpu.write8(o + NUMBER, n);
    cpu.write32(o + FL_OWNER, owner);
    cpu.write32(o + FL_FLAGS_AT, flags_at);
    cpu.write32(o + M_ANIMATION, FL_ANIMATIONS[0]);
    cpu.write8(o + SIDE, 1);
    cpu.write8(o + SPRITE_KIND, 0);
    let c = cpu.read16(CHAR_BASE_0).wrapping_add(FL_COLOURS);
    cpu.write16(o + COLOUR, c);
    cpu.write16(o + OWN_COLOUR, c);
    let frames = cpu.read32(GAME_FRAMES);
    let k = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, frames, 4));
    cpu.write8(o + SPRITE_FRAME, (k as u8).wrapping_add(FL_FRAME));
    cpu.write32(o + FLAGS, FL_SPRITE_FLAGS);
    let a = cpu.read16(owner + TURN_Z);
    cpu.write16(o + TURN_Z, a);
    let a = a as i16 as i32 as u32;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    let v = cpu.read32(owner + POSITION).wrapping_add(s);
    cpu.write32(o + POSITION, v);
    let a = cpu.read16(owner + TURN_Z) as i16 as i32 as u32;
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    let v = cpu.read32(owner + POSITION + 4).wrapping_sub(c);
    cpu.write32(o + POSITION + 4, v);
    cpu.write32(o + POSITION + 8, 0);
    cpu.write8(o + BYTE_33, FL_LIFT);
    let out = cpu.read16(d + D_BACK);
    cpu.write16(o + FL_OUT, out);
    let step = cpu.read16(d + D_STEP);
    cpu.write16(o + FL_STEP, step);
    let size = cpu.read16(d + D_SIZE) as i16 as i32;
    cpu.write32(o + SCALES, (size << 4) as u32);
    let size = cpu.read16(d + D_SIZE);
    cpu.write16(o + FL_GROWTH, size << 6);
    if cpu.read32(o + SCALES) as i32 <= FL_SMALL {
        let f = cpu.read32(o + FLAGS) | FL_HIDE;
        cpu.write32(o + FLAGS, f);
    }
}

// ---- the charge sparks ---------------------------------------------------------------------------------

/// A charge spark (orbits.rs's orbiter, its frame its own): its count (a byte, +0x4c). The spark
/// thrown off a charge: frames 61 on by its count, shrinking in height by 1/16 a frame; it ends
/// once flat. Whether it ends.
const CS_COUNT: u32 = 0x4c;
const CS_FRAME: u8 = 61;
const CS_SHRINK: u32 = 0x1000;

pub fn spark_fade(cpu: &mut Cpu, o: u32) -> u32 {
    let k = cpu.read8(o + CS_COUNT) & 15;
    cpu.write8(o + SPRITE_FRAME, k.wrapping_add(CS_FRAME));
    let h = cpu.read32(o + SCALES + 4).wrapping_sub(CS_SHRINK);
    cpu.write32(o + SCALES + 4, h);
    ((h as i32) < 0) as u32
}

/// A thrown spark's behaviour: placed on its orbit (ending with it) and faded; counted, 176 dots
/// nearer than its centre.
const CS_NEARER: u32 = 0xff50_0000;

pub fn spark_behaviour(cpu: &mut Cpu, o: u32) {
    if nested!(cpu, 0x0605_9418, orbits::orbit_place(cpu, o)) as u8 != 0
        || nested!(cpu, 0x0608_d044, spark_fade(cpu, o)) as u8 != 0
    {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    let n = cpu.read8(o + CS_COUNT).wrapping_add(1);
    cpu.write8(o + CS_COUNT, n);
    let z = cpu.read32(o + POSITION + 8).wrapping_add(CS_NEARER);
    cpu.write32(o + POSITION + 8, z);
}

/// A spark thrown off the charge `p`: the same orbit (watched word, mask, centre and angle
/// source) at a random angle and turn, at the centre; frame 61 of kind 0, colours 0x20 on;
/// 11/8 the charge's width, 2 high.
const CS_SIZE: u32 = 0x68;
const CS_BEHAVIOUR: u32 = 0x0608_d074;
const CS_SPRITE_FLAGS: u32 = 0x100;
const CS_COLOURS: u16 = 0x20;
const CS_WIDER: u32 = 0x1_6000;

pub fn spark_set(cpu: &mut Cpu, p: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, CS_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 3, blocks, PLAIN_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + 0x10));
    cpu.write32(o + BEHAVIOUR, CS_BEHAVIOUR);
    cpu.write32(o + FLAGS, CS_SPRITE_FLAGS);
    cpu.write8(o + CS_COUNT, 0);
    for (at, long) in [(0x3c, true), (0x40, false), (0x44, true), (0x48, true)] {
        if long {
            let v = cpu.read32(p + at);
            cpu.write32(o + at, v);
        } else {
            let v = cpu.read16(p + at);
            cpu.write16(o + at, v);
        }
    }
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write16(o + 0x58, r as u16);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x100));
    cpu.write16(o + 0x5a, r as u16);
    cpu.write32(o + 0x5c, 0);
    cpu.write32(o + 0x60, 0);
    let centre = cpu.read32(o + 0x44);
    for k in [0, 4, 8] {
        let v = cpu.read32(centre + k);
        cpu.write32(o + POSITION + k, v);
    }
    cpu.write8(o + SPRITE_FRAME, CS_FRAME);
    cpu.write8(o + SPRITE_KIND, 0);
    let c = cpu.read16(CHAR_BASE_0).wrapping_add(CS_COLOURS);
    cpu.write16(o + COLOUR, c);
    cpu.write8(o + BYTE_33, 0);
    let w = cpu.read32(p + SCALES);
    let w = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, w, CS_WIDER));
    cpu.write32(o + SCALES, w);
    cpu.write32(o + SCALES + 4, 0x2_0000);
}

/// A charge (an orbiter, its frame its own) growing: for its first 60 frames flipped at random,
/// frames 211 to 214 at random, growing to 5/8 by 1/16 a frame; then shrinking; it ends gone.
/// Its height its width. Whether it ends.
const CG_GROWS: u8 = 60;
const CG_FRAME: u8 = 211;
const CG_FLIP: u32 = 0x100;
const CG_FULL: u32 = 0xa000;
const CG_STEP: u32 = 0x1000;

pub fn charge_grow(cpu: &mut Cpu, o: u32) -> u32 {
    if cpu.read8(o + CS_COUNT) > CG_GROWS {
        let w = cpu.read32(o + SCALES);
        let w = nested!(cpu, 0x0604_6168, actors::approach(cpu, w, 0, CG_STEP));
        cpu.write32(o + SCALES, w);
        if w == 0 {
            return 1;
        }
    } else {
        let f = cpu.read32(o + FLAGS) & !CG_FLIP;
        cpu.write32(o + FLAGS, f);
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
        let f = cpu.read32(o + FLAGS) | (r & CG_FLIP);
        cpu.write32(o + FLAGS, f);
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
        cpu.write8(o + SPRITE_FRAME, ((r & 3) as u8).wrapping_add(CG_FRAME));
        let w = cpu.read32(o + SCALES);
        let w = nested!(cpu, 0x0604_6168, actors::approach(cpu, w, CG_FULL, CG_STEP));
        cpu.write32(o + SCALES, w);
    }
    let w = cpu.read32(o + SCALES);
    cpu.write32(o + SCALES + 4, w);
    0
}

/// A charge's behaviour: placed on its orbit (ending with it) and grown; counted, 176 dots nearer
/// than its centre; a spark thrown off every fifth frame.
pub fn charge_behaviour(cpu: &mut Cpu, o: u32) {
    if nested!(cpu, 0x0605_9418, orbits::orbit_place(cpu, o)) as u8 != 0
        || nested!(cpu, 0x0608_d1d8, charge_grow(cpu, o)) as u8 != 0
    {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    let n = cpu.read8(o + CS_COUNT).wrapping_add(1);
    cpu.write8(o + CS_COUNT, n);
    let z = cpu.read32(o + POSITION + 8).wrapping_add(CS_NEARER);
    cpu.write32(o + POSITION + 8, z);
    let frames = cpu.read32(GAME_FRAMES);
    if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, frames, 5)) == 0 {
        nested!(cpu, 0x0608_d0c4, spark_set(cpu, o));
    }
}

/// A charge made (sound 70) at the centre `centre` (x, y, z), its angle from the word at
/// `angle_from`, ending once the word at `watched` has a bit of `mask`: turning 1/16 of a turn
/// a frame, no radius, frame 214 of kind 0, colours 0x20 on, no size at first.
const CG_BEHAVIOUR: u32 = 0x0608_d268;
const CG_TURN: u16 = 0x1000;
const CG_START_FRAME: u8 = 214;

pub fn charge_set(cpu: &mut Cpu, watched: u32, mask: u32, centre: u32, angle_from: u32) {
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 70));
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, CS_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 3, blocks, PLAIN_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + 0x10));
    cpu.write32(o + BEHAVIOUR, CG_BEHAVIOUR);
    cpu.write32(o + FLAGS, CS_SPRITE_FLAGS);
    cpu.write8(o + CS_COUNT, 0);
    cpu.write32(o + 0x3c, watched);
    cpu.write16(o + 0x40, mask as u16);
    cpu.write32(o + 0x44, centre);
    cpu.write32(o + 0x48, angle_from);
    cpu.write16(o + 0x58, 0);
    cpu.write16(o + 0x5a, CG_TURN);
    cpu.write32(o + 0x5c, 0);
    cpu.write32(o + 0x60, 0);
    for k in [0, 4, 8] {
        let v = cpu.read32(centre + k);
        cpu.write32(o + POSITION + k, v);
    }
    cpu.write8(o + SPRITE_FRAME, CG_START_FRAME);
    cpu.write8(o + SPRITE_KIND, 0);
    let c = cpu.read16(CHAR_BASE_0).wrapping_add(CS_COLOURS);
    cpu.write16(o + COLOUR, c);
    cpu.write8(o + BYTE_33, 0);
    cpu.write32(o + SCALES, 0);
    cpu.write32(o + SCALES + 4, 0);
}

// ---- the serpent ----------------------------------------------------------------------------------------

/// The serpent (the stage's second large enemy: a head drawn by its own code with 14 body
/// segments following its trail). Its state (a byte, +0x6c), frames in it (a word, +0x70), its
/// plan (the actions at 0x060a42f0: where it is +0x72, frames to the next +0x74, times through
/// +0x76), its speed (x, y, z: +0x78), its turns' speeds (words: TURN_X's +0x84 eased by +0x86,
/// TURN_Y's +0x88 eased by +0x8a), its pose (+0x90: the joints' angles at +0xa4, the script's
/// flags at +0xb8), its parts (flags from +0xe0), its trail (210 frames each of x, y, z and the
/// three turns' high bytes, from +0x191), its drift (x, y: +0x68c), its segments (+0x69c), their
/// fire flags (bytes, +0x6d4), the side it works from (a byte, +0x6e2: 1 or -1), its flame (a
/// word, +0x6e4; its sweep +0x6e6, +0x6e8, +0x6ec), its neck's turn (words: +0x6f2, its speed
/// +0x6f4 eased by +0x6f6), its mouth (x, y, z: +0x6f8) and its turning slowness (a word,
/// +0x704: the turn toward the player divided by it).
const SS_STATE: u32 = 0x6c;
const SS_COUNT: u32 = 0x70;
const SS_PLAN_AT: u32 = 0x72;
const SS_WAIT: u32 = 0x74;
const SS_LOOPS: u32 = 0x76;
const SS_SPEED: u32 = 0x78;
const SS_TURN_X_SPEED: u32 = 0x84;
const SS_TURN_X_EASE: u32 = 0x86;
const SS_TURN_Y_SPEED: u32 = 0x88;
const SS_TURN_Y_EASE: u32 = 0x8a;
const SS_POSE: u32 = 0x90;
const SS_JOINTS: u32 = 0xa4;
const SS_SCRIPT_FLAGS: u32 = 0xb8;
const SS_PARTS: u32 = 0xe0;
const SS_TRAIL_X: u32 = 0x191;
const SS_TRAIL_Y: u32 = 0x263;
const SS_TRAIL_TURN_X: u32 = 0x407;
const SS_TRAIL_TURN_Y: u32 = 0x4d9;
const SS_TRAIL_TURN_Z: u32 = 0x5ab;
const SS_TRAIL: u32 = 210;
const SS_DRIFT: u32 = 0x68c;
const SS_SEGMENTS: u32 = 0x69c;
const SS_SEGMENT_COUNT: u32 = 14;
const SS_FIRES: u32 = 0x6d4;
const SS_SIDE: u32 = 0x6e2;
const SS_FLAME: u32 = 0x6e4;
const SS_SWEEP: u32 = 0x6e6;
const SS_SWEEP_SPEED: u32 = 0x6e8;
const SS_SWEEP_2: u32 = 0x6ec;
const SS_NECK: u32 = 0x6f2;
const SS_NECK_SPEED: u32 = 0x6f4;
const SS_NECK_EASE: u32 = 0x6f6;
const SS_MOUTH: u32 = 0x6f8;
const SS_SLOWNESS: u32 = 0x704;
const SS_PLAN: u32 = 0x060a_42f0;
const SS_BEAT: u16 = 1;
const SS_SCRIPT_END: u16 = 0x4000;

/// A cross flash (sound 93) at its mouth, 176 dots nearer: colour 0x30, 3 high, along its angle.
const SS_FLASH_NEARER: u32 = 0xff50_0000;

pub fn ss_flash(cpu: &mut Cpu, o: u32) {
    let t = cpu.stack();
    let x = cpu.read32(o + SS_MOUTH);
    cpu.write32(t + actors::E_POSITION, x);
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 93));
    let y = cpu.read32(o + SS_MOUTH + 4);
    cpu.write32(t + actors::E_POSITION + 4, y);
    let z = cpu.read32(o + SS_MOUTH + 8).wrapping_add(SS_FLASH_NEARER);
    cpu.write32(t + actors::E_POSITION + 8, z);
    cpu.write16(t + actors::E_COLOUR, 48);
    cpu.write32(t + actors::E_SCALES + 4, 0x3_0000);
    let a = cpu.read16(o + TURN_Z);
    cpu.write16(t + actors::E_ANGLE, a);
    nested!(cpu, 0x0605_7e38, explosions::cross_flash(cpu, t));
}

/// A fireball (an enemy actor): its speed (x, y: +0x80), frames it speeds up (a word, +0x8c) by
/// (words: +0x8e, +0x90), its count (a byte, +0x88), its angle (a word, +0x92). It ends off the
/// screen; it flickers (flipped by turns) at its angle and a little more at random; every 11th
/// frame it leaves smoke behind (a drift effect: colours 0x30 on, the script at 0x06073146,
/// half as high).
const FB_SPEED: u32 = 0x80;
const FB_SPEEDING: u32 = 0x8c;
const FB_SPEED_UP: u32 = 0x8e;
const FB_COUNT: u32 = 0x88;
const FB_ANGLE: u32 = 0x92;
const FB_FLIP: u32 = 0x10;
const FB_WOBBLE: u32 = 0x600;
const FB_SMOKE_SCRIPT: u32 = 0x0607_3146;
const FB_SMOKE_COLOUR: u16 = 0x8030;

pub fn fireball_behaviour(cpu: &mut Cpu, o: u32) {
    if nested!(cpu, 0x0605_6a24, aim::off_screen(cpu, o + 0x10)) as u8 != 0 {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    let n = cpu.read16(o + FB_SPEEDING);
    if n as i16 > 0 {
        cpu.write16(o + FB_SPEEDING, n.wrapping_sub(1));
        for k in [0, 4] {
            let v = cpu.read32(o + FB_SPEED + k).wrapping_add(cpu.read16(o + FB_SPEED_UP + k / 2) as i16 as u32);
            cpu.write32(o + FB_SPEED + k, v);
        }
    }
    for k in [0, 4] {
        let p = cpu.read32(o + POSITION + k).wrapping_add(cpu.read32(o + FB_SPEED + k));
        cpu.write32(o + POSITION + k, p);
    }
    let f = cpu.read32(o + FLAGS) ^ FB_FLIP;
    cpu.write32(o + FLAGS, f);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, FB_WOBBLE));
    let a = (r as u16).wrapping_add(cpu.read16(o + FB_ANGLE));
    cpu.write16(o + TURN_Z, a);
    let n = cpu.read8(o + FB_COUNT).wrapping_add(1);
    cpu.write8(o + FB_COUNT, n);
    let n = cpu.read8(o + FB_COUNT) as i8 as i32 as u32;
    if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, n, 11)) == 0 {
        let t = cpu.stack();
        cpu.write32(t + actors::E_SCRIPT, FB_SMOKE_SCRIPT);
        cpu.write16(t + actors::E_COLOUR, FB_SMOKE_COLOUR);
        for k in [0, 4, 8] {
            let v = cpu.read32(o + POSITION + k);
            cpu.write32(t + actors::E_POSITION + k, v);
        }
        let a = cpu.read16(o + TURN_Z).wrapping_add(0x8000);
        cpu.write16(t + actors::E_ANGLE, a);
        cpu.write32(t + actors::E_SPEED, 0x8000);
        let w = cpu.read32(o + SCALES);
        cpu.write32(t + actors::E_SCALES, w);
        let h = cpu.read32(o + SCALES + 4) as i32;
        cpu.write32(t + actors::E_SCALES + 4, (h / 2) as u32);
        nested!(cpu, 0x0605_7454, actors::drift_effect(cpu, t));
    }
}

/// A fireball made from the description `d` (its place, its number +0x1e, its angle a word
/// +0x2c): kind 0's frame 100, colours 0x30 on, side 1, hit by weapon 5; speeding up over 15
/// frames by a ninth of its direction each; half wide, 7/8 high.
const FB_SIZE: u32 = 0xbc;
const FB_BEHAVIOUR: u32 = 0x0608_d424;
const FB_SPRITE_FLAGS: u32 = 0x420;
const FB_LIFT: u8 = 8;
const FB_FRAME: u8 = 100;
const FB_COLOURS: u16 = 0x30;
const FB_ANIMATION: u32 = 0x060a_42e6;
const FB_WEAPON: u8 = 5;
const FB_SPEEDS_FOR: u16 = 15;
const D_ANGLE_2C: u32 = 0x2c;

pub fn fireball_set(cpu: &mut Cpu, d: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, FB_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + 0x10));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HITS_AT));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    let n = cpu.read8(d + S_NUMBER);
    cpu.write8(o + NUMBER, n);
    cpu.write32(o + BEHAVIOUR, FB_BEHAVIOUR);
    cpu.write32(o + FLAGS, FB_SPRITE_FLAGS);
    for k in [0, 4, 8] {
        let v = cpu.read32(d + k);
        cpu.write32(o + POSITION + k, v);
    }
    cpu.write8(o + BYTE_33, FB_LIFT);
    cpu.write8(o + SPRITE_FRAME, FB_FRAME);
    cpu.write8(o + SPRITE_KIND, 0);
    let c = cpu.read16(CHAR_BASE_0).wrapping_add(FB_COLOURS);
    cpu.write16(o + OWN_COLOUR, c);
    cpu.write16(o + COLOUR, c);
    cpu.write32(o + M_ANIMATION, FB_ANIMATION);
    cpu.write8(o + SIDE, 1);
    cpu.write8(o + HIT_WEAPON, FB_WEAPON);
    let a = cpu.read16(d + D_ANGLE_2C);
    cpu.write16(o + TURN_Z, a);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write8(o + FB_COUNT, r as u8);
    let a = cpu.read16(o + TURN_Z);
    cpu.write16(o + FB_ANGLE, a);
    cpu.write16(o + FB_SPEEDING, FB_SPEEDS_FOR);
    let a = cpu.read16(d + D_ANGLE_2C) as i16 as i32 as u32;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    let q = nested!(cpu, 0x0600_6a78, math::quotient(cpu, s, 9));
    cpu.write16(o + FB_SPEED_UP, q as u16);
    let a = cpu.read16(d + D_ANGLE_2C) as i16 as i32 as u32;
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    let q = nested!(cpu, 0x0600_6a78, math::quotient(cpu, c, 9));
    cpu.write16(o + FB_SPEED_UP + 2, (q as u16).wrapping_neg());
    cpu.write32(o + SCALES, 0x8000);
    cpu.write32(o + SCALES + 4, 0xe000);
}

/// A fireball from its mouth at `angle`.
pub fn ss_fireball(cpu: &mut Cpu, o: u32, angle: u32) {
    let t = cpu.stack();
    let n = cpu.read8(o + NUMBER);
    cpu.write8(t + S_NUMBER, n);
    cpu.write16(t + D_ANGLE_2C, angle as u16);
    let x = cpu.read32(o + SS_MOUTH);
    cpu.write32(t, x);
    let y = cpu.read32(o + SS_MOUTH + 4);
    cpu.write32(t + 4, y);
    cpu.write32(t + 8, 0);
    nested!(cpu, 0x0608_d52c, fireball_set(cpu, t));
}

/// A ring of glow round its mouth (an orbiter: kind 2, colours 1, half size and shrinking by
/// 0xa00 a frame, turning 1/64 of a turn a frame at its angle, its radius growing a dot a frame,
/// lift -6), until its head part's bit 6.
const OD_SCALES: u32 = 0x0c;
const OD_ANGLE: u32 = 0x18;
const OD_COLOURS: u32 = 0x20;
const OD_RADIUS: u32 = 0x24;
const OD_KIND: u32 = 0x2c;
const OD_ANGLE_OFFSET: u32 = 0x2e;
const OD_TURN: u32 = 0x30;
const OD_LIFT: u32 = 0x32;
const OD_SCALE_SPEED: u32 = 0x34;

pub fn ss_glow(cpu: &mut Cpu, o: u32) {
    static PROLOGUE: std::sync::OnceLock<super::Prologue> = std::sync::OnceLock::new();
    let t = cpu.stack();
    cpu.write16(t + OD_KIND, 2);
    cpu.write16(t + OD_COLOURS, 1);
    cpu.write32(t + OD_SCALES, 0x8000);
    cpu.write32(t + OD_SCALES + 4, 0x8000);
    cpu.write32(t + OD_SCALE_SPEED, 0xffff_f600);
    cpu.write32(t + OD_SCALE_SPEED + 4, 0xffff_f600);
    let a = cpu.read16(o + TURN_Z);
    cpu.write16(t + OD_ANGLE, a);
    cpu.write16(t + OD_ANGLE_OFFSET, 0);
    cpu.write16(t + OD_TURN, 0x400);
    cpu.write16(t + OD_LIFT, 0xfffa);
    cpu.write32(t + OD_RADIUS, 0);
    cpu.write32(t + OD_RADIUS + 4, 0x1_0000);
    let angle_from = o + TURN_Z;
    cpu.nested_with_stack(&PROLOGUE, 0x0605_9554, &[angle_from], |cpu| {
        orbits::orbit_set(cpu, t, o + SS_PARTS, 0x40, o + SS_MOUTH, angle_from)
    });
}

/// Its segments' fire flags cleared.
pub fn ss_fires_clear(cpu: &mut Cpu, o: u32) {
    let mut i: i8 = 0;
    loop {
        let at = o.wrapping_add(i as i32 as u32);
        i = i.wrapping_add(1);
        cpu.write8(at + SS_FIRES, 0);
        if i > 13 {
            break;
        }
    }
}

fn ss_script(cpu: &mut Cpu, o: u32, script: u32) {
    nested!(cpu, 0x0605_6628, actors::script_start(cpu, o + SS_POSE, script));
}

/// Action 4: it coils (state 11).
const SS_POSE_COIL: u32 = 0x060a_4180;
const SS_SCRIPT_COIL: u32 = 0x060a_42c0;

pub fn ss_coil(cpu: &mut Cpu, o: u32) {
    cpu.write8(o + SS_STATE, 11);
    nested!(cpu, 0x0605_65f0, models::pose_set(cpu, o + SS_POSE, SS_POSE_COIL));
    ss_script(cpu, o, SS_SCRIPT_COIL);
    nested!(cpu, 0x0605_6644, models::pose_step(cpu, o + SS_POSE));
}

/// Action 0: it breathes fire (state 7 from the right, 9 from the left); at rank 3 on, half the
/// time with its longer script.
const SS_POSE_BREATH: u32 = 0x060a_4168;
const SS_SCRIPT_BREATH: u32 = 0x060a_426c;
const SS_SCRIPT_BREATH_LONG: u32 = 0x060a_4296;

pub fn ss_breath(cpu: &mut Cpu, o: u32) {
    let s = if cpu.read32(o + POSITION) as i32 > 0 { 7 } else { 9 };
    cpu.write8(o + SS_STATE, s);
    nested!(cpu, 0x0605_65f0, models::pose_set(cpu, o + SS_POSE, SS_POSE_BREATH));
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000)) & 0x80;
    if r == 0 || (r == 0x80 && (cpu.read8(RANK) as i8) <= 2) {
        ss_script(cpu, o, SS_SCRIPT_BREATH);
    } else if r == 0x80 {
        ss_script(cpu, o, SS_SCRIPT_BREATH_LONG);
    }
    nested!(cpu, 0x0605_6644, models::pose_step(cpu, o + SS_POSE));
}

/// Action 3: it lunges (state 3), from a side at random, turning faster.
pub fn ss_lunge(cpu: &mut Cpu, o: u32) {
    cpu.write8(o + SS_STATE, 3);
    cpu.write16(o + SS_COUNT, 0);
    cpu.write16(o + SS_FLAME, 0);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write8(o + SS_SIDE, (r & 1) as u8);
    cpu.write16(o + SS_SLOWNESS, 16);
}

/// Action 1: it sweeps its flame (state 4) from the side away from it; its segments fire by the
/// rank (more at the lower ranks).
pub fn ss_sweep(cpu: &mut Cpu, o: u32) {
    cpu.write8(o + SS_STATE, 4);
    cpu.write16(o + SS_COUNT, 0);
    cpu.write16(o + SS_FLAME, 0x80);
    cpu.write16(o + SS_SWEEP, 0);
    cpu.write32(o + SS_SWEEP_SPEED, 0x1_0000);
    cpu.write32(o + SS_SWEEP_2, 0);
    let side = if cpu.read32(o + POSITION) as i32 > 0 { 0xff } else { 1 };
    cpu.write8(o + SS_SIDE, side);
    let rank = cpu.read8(RANK) as i8;
    let fires: &[u32] = match rank {
        0 => &[1, 11, 7, 5, 3, 9],
        1 => &[7, 5, 3, 9],
        2 => &[3, 9],
        _ => &[],
    };
    for &k in fires {
        cpu.write8(o + SS_FIRES + k, 1);
    }
}

/// Action 2: it sweeps its flame faster (state 5), every other segment firing toward it.
pub fn ss_sweep_fast(cpu: &mut Cpu, o: u32) {
    cpu.write8(o + SS_STATE, 5);
    cpu.write16(o + SS_COUNT, 0);
    cpu.write16(o + SS_FLAME, 0x80);
    cpu.write16(o + SS_SWEEP, 0);
    cpu.write32(o + SS_SWEEP_SPEED, 0x2_0000);
    let (side, fire) = if cpu.read32(o + POSITION) as i32 > 0 { (0xff, 65) } else { (1, 0xc1) };
    cpu.write8(o + SS_SIDE, side);
    for k in (0..14).step_by(2) {
        cpu.write8(o + SS_FIRES + k, fire);
    }
}

/// State 6 set up: it dives in from 240 dots up, its turns from its pose, its trail and
/// segments at its place.
pub fn ss_dive_in(cpu: &mut Cpu, o: u32) {
    cpu.write8(o + SS_STATE, 6);
    // the SH-2 code keeps the angle's address in its frame
    let t = cpu.stack();
    cpu.write32(t, o + TURN_Z);
    let joints = cpu.read32(o + SS_JOINTS);
    let a = cpu.read16(joints);
    cpu.write16(o + TURN_Z, a);
    cpu.write16(o + TURN_Y, a.wrapping_neg());
    for at in (0x84..=0x8e).step_by(2) {
        cpu.write16(o + at, 0);
    }
    cpu.write32(o + POSITION, 0);
    nested!(cpu, 0x0605_6294, actors::fill_bytes(cpu, o + SS_TRAIL_X, 0, SS_TRAIL));
    cpu.write32(o + POSITION + 4, 0xff10_0000);
    nested!(cpu, 0x0605_6294, actors::fill_bytes(cpu, o + SS_TRAIL_Y, 0, SS_TRAIL));
    for (turn, trail) in [(TURN_Y, SS_TRAIL_TURN_Y), (TURN_Z, SS_TRAIL_TURN_Z)] {
        let a = cpu.read16(o + turn) as i16 as i32;
        let a = if a < 0 { a + 0xff } else { a } as u32;
        nested!(cpu, 0x0600_8164, clib::ashiftrt_r4::<8>(cpu, a));
        let a = (a as i32 >> 8) as i8 as i32 as u32;
        nested!(cpu, 0x0605_6294, actors::fill_bytes(cpu, o + trail, a, SS_TRAIL));
    }
    cpu.write16(o + SS_COUNT, 0);
    cpu.write32(o + SS_SPEED, 0);
    cpu.write32(o + SS_SPEED + 4, 0);
    cpu.write32(o + SS_DRIFT, 0);
    cpu.write32(o + SS_DRIFT + 4, 0);
    for i in 0..SS_SEGMENT_COUNT {
        let s = cpu.read32(o + SS_SEGMENTS + 4 * i);
        for k in [0, 4, 8] {
            let v = cpu.read32(o + POSITION + k);
            cpu.write32(s + POSITION + k, v);
        }
    }
}

/// The plan's next action (5: back to its start, counted); none past action 4.
pub fn ss_next_action(cpu: &mut Cpu, o: u32) {
    if cpu.read16(o + SS_WAIT) != 0 {
        return;
    }
    let at = cpu.read16(o + SS_PLAN_AT) as u32;
    let mut action = cpu.read8(SS_PLAN + at);
    if action == 5 {
        cpu.write16(o + SS_PLAN_AT, 0);
        action = cpu.read8(SS_PLAN);
        let n = cpu.read16(o + SS_LOOPS).wrapping_add(1);
        cpu.write16(o + SS_LOOPS, n);
    }
    let at = cpu.read16(o + SS_PLAN_AT).wrapping_add(1);
    cpu.write16(o + SS_PLAN_AT, at);
    match action {
        0 => nested!(cpu, 0x0608_d7bc, ss_breath(cpu, o)),
        1 => nested!(cpu, 0x0608_d8a4, ss_sweep(cpu, o)),
        2 => nested!(cpu, 0x0608_d944, ss_sweep_fast(cpu, o)),
        3 => nested!(cpu, 0x0608_d854, ss_lunge(cpu, o)),
        4 => nested!(cpu, 0x0608_d770, ss_coil(cpu, o)),
        _ => {}
    }
}

/// Its tilt (TURN_Y) eased toward `to` (aim::turn_toward's: at most 0x800, eased by 6, by 32).
pub fn ss_tilt(cpu: &mut Cpu, o: u32, to: u32) {
    static PROLOGUE: std::sync::OnceLock<super::Prologue> = std::sync::OnceLock::new();
    let from = cpu.read16(o + TURN_Y) as u32;
    let to = to & 0xffff;
    cpu.nested_with_stack(&PROLOGUE, 0x0605_70d8, &[0x800, 6, 32], |cpu| {
        aim::turn_toward(cpu, o + SS_TURN_Y_SPEED, o + SS_TURN_Y_EASE, from, to, 0x800, 6, 32)
    });
}

/// Its neck eased toward `to` (at most 0x800, eased by 4, by 16).
pub fn ss_neck(cpu: &mut Cpu, o: u32, to: u32) {
    static PROLOGUE: std::sync::OnceLock<super::Prologue> = std::sync::OnceLock::new();
    let from = cpu.read16(o + SS_NECK) as u32;
    let to = to & 0xffff;
    cpu.nested_with_stack(&PROLOGUE, 0x0605_70d8, &[0x800, 4, 16], |cpu| {
        aim::turn_toward(cpu, o + SS_NECK_SPEED, o + SS_NECK_EASE, from, to, 0x800, 4, 16)
    });
}

/// States 0 and 1: it comes in by its pose: its tilt from joint 2, its heading a sixteenth of the
/// way to joint 0's a frame (TURN_Y the other way), moving at joint 1's speed along its heading
/// (sideways) and its tilt; its neck straight, then (from the script's beat, sound 95) back; at
/// the script's end, hovering (state 2) for 120 frames, turning slower.
pub fn ss_coming(cpu: &mut Cpu, o: u32) {
    // the SH-2 code keeps the tilt's address in its frame
    let t = cpu.stack();
    cpu.write32(t, o + TURN_X);
    let joints = cpu.read32(o + SS_JOINTS);
    let tilt = cpu.read16(joints + 4);
    cpu.write16(o + TURN_X, tilt);
    let joints = cpu.read32(o + SS_JOINTS);
    let from = cpu.read16(o + TURN_Z) as i16 as i32 as u32;
    let to = cpu.read16(joints) as i16 as i32 as u32;
    let d = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, from, to)) as i32;
    let d = if d < 0 { d + 15 } else { d } >> 4;
    let a = (d as u16).wrapping_add(cpu.read16(o + TURN_Z));
    cpu.write16(o + TURN_Z, a);
    cpu.write16(o + TURN_Y, 0x8000u16.wrapping_sub(a));
    let speed = |cpu: &mut Cpu| {
        let joints = cpu.read32(o + SS_JOINTS);
        ((cpu.read16(joints + 2) as i16 as i32) << 4) as u32
    };
    let v = speed(cpu);
    let a = cpu.read16(o + TURN_Z) as i16 as i32 as u32;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    let x = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, v));
    cpu.write32(o + SS_SPEED, x);
    let v = speed(cpu);
    let a = cpu.read16(o + TURN_X).wrapping_add(0xc000) as i16 as i32 as u32;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    let y = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, v));
    cpu.write32(o + SS_SPEED + 4, y);
    let v = speed(cpu);
    let a = cpu.read16(o + TURN_X).wrapping_add(0xc000) as i16 as i32 as u32;
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    let z = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, c.wrapping_neg(), v));
    cpu.write32(o + SS_SPEED + 8, z);
    nested!(cpu, 0x0605_6644, models::pose_step(cpu, o + SS_POSE));
    let to = if cpu.read8(o + SS_STATE) == 0 {
        if cpu.read16(o + SS_SCRIPT_FLAGS) & SS_BEAT != 0 {
            cpu.write8(o + SS_STATE, 1);
            nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 95));
        }
        0x800
    } else {
        0xf000
    };
    nested!(cpu, 0x0608_dbb8, ss_neck(cpu, o, to));
    if cpu.read16(o + SS_SCRIPT_FLAGS) & SS_SCRIPT_END != 0 {
        cpu.write8(o + SS_STATE, 2);
        cpu.write16(o + SS_SLOWNESS, 48);
        cpu.write16(o + SS_WAIT, 120);
        cpu.write32(o + SS_SPEED + 8, 0);
    }
}

/// State 2: it hovers, its neck forward and level, heading toward the nearer player (48 dots
/// above the middle), by the angle left divided by its slowness (which eases down to 40), tilted
/// with it; moving 2 dots a frame along its heading, its drift easing out; the plan's next action
/// once its wait is over.
const SS_AIM_ABOVE: u32 = 0x30_0000;

pub fn ss_hovering(cpu: &mut Cpu, o: u32) {
    let p = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + 0x10));
    nested!(cpu, 0x0608_dbb8, ss_neck(cpu, o, 0x800));
    let from = cpu.read16(o + TURN_X) as u32;
    cpu.push(&[0x800, 6, 32]);
    nested!(cpu, 0x0605_70d8, aim::turn_toward(cpu, o + SS_TURN_X_SPEED, o + SS_TURN_X_EASE, from, 0, 0x800, 6, 32));
    let dx = cpu.read32(p + POSITION).wrapping_sub(cpu.read32(o + POSITION));
    let dy = SS_AIM_ABOVE.wrapping_sub(cpu.read32(o + POSITION + 4));
    let a = nested!(cpu, 0x0606_780c, sgl::sl_atan(cpu, dx, dy));
    let to = (a as i16 as i32 as u32).wrapping_add(0x4000);
    let from = cpu.read16(o + TURN_Z) as i16 as i32 as u32;
    let d = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, from, to));
    let slow = cpu.read16(o + SS_SLOWNESS) as u32;
    let q = nested!(cpu, 0x0606_62d8, clib::divide(cpu, d, slow));
    let a = (q as u16).wrapping_add(cpu.read16(o + TURN_Z));
    cpu.write16(o + TURN_Z, a);
    let slow = cpu.read16(o + SS_SLOWNESS) as u32;
    let slow = nested!(cpu, 0x0604_6168, actors::approach(cpu, slow, 40, 1));
    cpu.write16(o + SS_SLOWNESS, slow as u16);
    let a = (cpu.read16(o + TURN_Z).wrapping_neg()) as u32;
    nested!(cpu, 0x0608_db7c, ss_tilt(cpu, o, a));
    let a = cpu.read16(o + TURN_Z) as i16 as i32 as u32;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    cpu.write32(o + SS_SPEED, s.wrapping_mul(2));
    let a = cpu.read16(o + TURN_Z) as i16 as i32 as u32;
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    cpu.write32(o + SS_SPEED + 4, c.wrapping_neg().wrapping_mul(2));
    for k in [0, 4] {
        let d = cpu.read32(o + SS_DRIFT + k);
        let d = nested!(cpu, 0x0604_6168, actors::approach(cpu, d, 0, 0x400));
        cpu.write32(o + SS_DRIFT + k, d);
    }
    let wait = cpu.read16(o + SS_WAIT);
    cpu.pop(12);
    if wait != 0 {
        let w = nested!(cpu, 0x0604_6168, actors::approach(cpu, wait as i16 as u32, 0, 1));
        cpu.write16(o + SS_WAIT, w as u16);
    } else {
        nested!(cpu, 0x0608_dae4, ss_next_action(cpu, o));
    }
}

/// Its flame from its mouth (flame_set's: its number, 24 dots out, `size`, 32 a frame), ending
/// with its head part.
fn ss_flame_out(cpu: &mut Cpu, o: u32, size: u16) {
    let t = cpu.stack();
    let n = cpu.read8(o + NUMBER);
    cpu.write8(t + S_NUMBER, n);
    cpu.write16(t + D_BACK, 24);
    cpu.write16(t + D_SIZE, size);
    cpu.write16(t + D_STEP, 32);
    nested!(cpu, 0x0608_cee4, flame_set(cpu, t, o, o + SS_PARTS));
}

/// Its turn toward `to`: the angle left divided by its slowness, added to its heading.
fn ss_turn_to(cpu: &mut Cpu, o: u32, dx: u32, dy: u32) -> u16 {
    let a = nested!(cpu, 0x0606_780c, sgl::sl_atan(cpu, dx, dy));
    let to = (a as i16 as i32 as u32).wrapping_add(0x4000);
    let from = cpu.read16(o + TURN_Z) as i16 as i32 as u32;
    let d = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, from, to));
    let slow = cpu.read16(o + SS_SLOWNESS) as u32;
    let q = nested!(cpu, 0x0606_62d8, clib::divide(cpu, d, slow));
    let a = (q as u16).wrapping_add(cpu.read16(o + TURN_Z));
    cpu.write16(o + TURN_Z, a);
    a
}

/// Its speed 2 dots a frame along its heading.
fn ss_ahead(cpu: &mut Cpu, o: u32) {
    let a = cpu.read16(o + TURN_Z) as i16 as i32 as u32;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    cpu.write32(o + SS_SPEED, s.wrapping_mul(2));
    let a = cpu.read16(o + TURN_Z) as i16 as i32 as u32;
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    cpu.write32(o + SS_SPEED + 4, c.wrapping_neg().wrapping_mul(2));
}

/// State 3: the lunge. It chases the nearer player (from rank 3, a point circling 64 dots round
/// the player), tilted over by its side, drifting back against its speed; its turning slows to the
/// rank's (the words at 0x060a42fa) over 480 frames, then its flame grows to 0x100 (to frame
/// 900) and dies (its turning slower, to 96); its neck back from frame 391 to 959; a charge at
/// its mouth at frame 420, a flash at 480; the flame out while it burns; after 1200 frames,
/// hovering (120 frames).
const SS_SLOWNESSES: u32 = 0x060a_42fa;

pub fn ss_lunging(cpu: &mut Cpu, o: u32) {
    let p = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + 0x10));
    let (dx, dy) = if cpu.read8(RANK) as i8 > 2 {
        let a = (cpu.read32(GAME_FRAMES) << 8) as i16 as i32 as u32;
        let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
        let dx = cpu.read32(p + POSITION).wrapping_add(s << 6).wrapping_sub(cpu.read32(o + POSITION));
        let a = (cpu.read32(GAME_FRAMES) << 8) as i16 as i32 as u32;
        let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
        let dy = cpu.read32(p + POSITION + 4).wrapping_sub(c << 6).wrapping_sub(cpu.read32(o + POSITION + 4));
        (dx, dy)
    } else {
        let dx = cpu.read32(p + POSITION).wrapping_sub(cpu.read32(o + POSITION));
        let dy = cpu.read32(p + POSITION + 4).wrapping_sub(cpu.read32(o + POSITION + 4));
        (dx, dy)
    };
    let a = ss_turn_to(cpu, o, dx, dy) as i16 as i32 as u32;
    let side = cpu.read8(o + SS_SIDE) as i8 as i32 as u32;
    let over = ((side << 8) >> 1) << 8;
    nested!(cpu, 0x0608_db7c, ss_tilt(cpu, o, over.wrapping_sub(a) & 0xffff));
    ss_ahead(cpu, o);
    for k in [0, 4] {
        let v = cpu.read32(o + SS_SPEED + k).wrapping_neg() as i32;
        let d = cpu.read32(o + SS_DRIFT + k);
        let d = nested!(cpu, 0x0604_6168, actors::approach(cpu, d, (v / 2) as u32, 0x400));
        cpu.write32(o + SS_DRIFT + k, d);
    }
    let c = cpu.read16(o + SS_COUNT);
    if c <= 0x1e0 {
        let rank = cpu.read8(RANK) as i8 as i32 as u32;
        let to = cpu.read16(SS_SLOWNESSES.wrapping_add(rank << 1)) as u32;
        let s = cpu.read16(o + SS_SLOWNESS) as u32;
        let s = nested!(cpu, 0x0604_6168, actors::approach(cpu, s, to, 1));
        cpu.write16(o + SS_SLOWNESS, s as u16);
    } else if c <= 0x384 {
        let f = cpu.read16(o + SS_FLAME) as i16 as u32;
        let f = nested!(cpu, 0x0604_6168, actors::approach(cpu, f, 0x100, 8));
        cpu.write16(o + SS_FLAME, f as u16);
    } else {
        let f = cpu.read16(o + SS_FLAME) as i16 as u32;
        let f = nested!(cpu, 0x0604_6168, actors::approach(cpu, f, 0, 8));
        cpu.write16(o + SS_FLAME, f as u16);
        let s = cpu.read16(o + SS_SLOWNESS) as u32;
        let s = nested!(cpu, 0x0604_6168, actors::approach(cpu, s, 96, 1));
        cpu.write16(o + SS_SLOWNESS, s as u16);
    }
    let c = cpu.read16(o + SS_COUNT);
    let neck = if c.wrapping_sub(0x187) <= 0x238 { 0xf000 } else { 0x800 };
    nested!(cpu, 0x0608_dbb8, ss_neck(cpu, o, neck));
    let c = cpu.read16(o + SS_COUNT);
    if c == 0x1a4 {
        nested!(cpu, 0x0608_d2d8, charge_set(cpu, o + SS_PARTS, 64, o + SS_MOUTH, o + TURN_Z));
    }
    if cpu.read16(o + SS_COUNT) == 0x1e0 {
        nested!(cpu, 0x0608_d3bc, ss_flash(cpu, o));
    }
    let f = cpu.read16(o + SS_FLAME);
    if f != 0 {
        ss_flame_out(cpu, o, f);
    }
    if cpu.read16(o + SS_COUNT) > 0x4b0 {
        cpu.write8(o + SS_STATE, 2);
        cpu.write16(o + SS_WAIT, 120);
    }
}

/// State 4: the sweep. Its neck bobs (by the frame); its flame grows to 0xd0 and sweeps it round
/// (its heading turned by the flame times its side), moving along its heading faster as its neck
/// turns, then the flame dies after frame 1020; its drift eased toward its aim at the nearer
/// player (a sixteenth a frame), by the flame; tilted by its side (over from frame 481 to 1019);
/// while its tail is whole, from rank 2 the tail charges and flashes (frames 420 to 900) and
/// flames; after 1200 frames, hovering (240 frames), its segments' fire cleared.
const SS_AIM: u32 = 0x698;
const SS_TAIL: u32 = SS_SEGMENTS + 4 * 13;
/// The tail's own flags (bit 6: broken), its part, its tip.
const SS_TAIL_FLAGS: u32 = 0x6c;
const SS_BROKEN: u16 = 0x40;
const SS_TAIL_PARTS: u32 = 0x70;
const SS_TAIL_TIP: u32 = 0x158;
const SS_TAIL_FLAME: u32 = SS_SWEEP_2;

pub fn ss_sweeping(cpu: &mut Cpu, o: u32) {
    let t = cpu.stack();
    let neck = if cpu.read32(GAME_FRAMES) & 32 != 0 { 0 } else { 0x800 };
    nested!(cpu, 0x0608_dbb8, ss_neck(cpu, o, neck));
    let c = cpu.read16(o + SS_COUNT);
    let approach = |cpu: &mut Cpu, at: u32, word: bool, to: u32, step: u32| {
        let v = if word { cpu.read16(o + at) as i16 as u32 } else { cpu.read32(o + at) };
        let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, to, step));
        if word {
            cpu.write16(o + at, v as u16);
        } else {
            cpu.write32(o + at, v);
        }
    };
    let (sweep_to, sweep_step) = if c > 0x3fc {
        approach(cpu, SS_FLAME, true, 0, 1);
        approach(cpu, SS_SWEEP_SPEED, false, 0x2_0000, 0x200);
        (0xffff_ff80, 8)
    } else {
        if c > 0xb4 {
            let n = cpu.read16(o + SS_NECK) as u32;
            approach(cpu, SS_SWEEP_SPEED, false, 0xf000u32.wrapping_sub(n << 3), 0x800);
        } else {
            approach(cpu, SS_SWEEP_SPEED, false, 0x1_8000, 0x100);
        }
        approach(cpu, SS_FLAME, true, 0xd0, 1);
        (0x1000, 8)
    };
    let f = cpu.read16(o + SS_FLAME) as i16 as u32;
    let s = nested!(cpu, 0x0604_6168, actors::approach(cpu, f, sweep_to, sweep_step));
    cpu.write16(o + SS_SWEEP, s as u16);
    // the SH-2 code keeps the side's offset in its frame
    cpu.write32(t + 64, SS_SIDE);
    let side = cpu.read8(o + SS_SIDE) as i8 as i16;
    let f = cpu.read16(o + SS_FLAME) as i16;
    let a = (side.wrapping_mul(f) as u16).wrapping_add(cpu.read16(o + TURN_Z));
    cpu.write16(o + TURN_Z, a);
    for (k, cos) in [(0, false), (4, true)] {
        let v = cpu.read32(o + SS_SWEEP_SPEED);
        let a = cpu.read16(o + TURN_Z) as i16 as i32 as u32;
        let s = if cos {
            nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a)).wrapping_neg()
        } else {
            nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a))
        };
        let m = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, v));
        cpu.write32(o + SS_SPEED + k, m);
    }
    let to = nested!(cpu, 0x0605_6ad4, aim::angle_to_target(cpu, o, 20)) & 0xffff;
    let from = cpu.read16(o + SS_AIM) as u32;
    let d = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, from, to)) as i32;
    let d = if d < 0 { d + 15 } else { d } >> 4;
    let a = (d as u16).wrapping_add(cpu.read16(o + SS_AIM));
    cpu.write16(o + SS_AIM, a);
    cpu.write32(t + 68, SS_SWEEP);
    for (k, cos) in [(0, false), (4, true)] {
        let step = cpu.read16(o + SS_SWEEP) as i16 as u32;
        let a = cpu.read16(o + SS_AIM) as i16 as i32 as u32;
        let s = if cos {
            nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a)).wrapping_neg()
        } else {
            nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a))
        };
        let d = cpu.read32(o + SS_DRIFT + k);
        let d = nested!(cpu, 0x0604_6168, actors::approach(cpu, d, (s as i32 / 2) as u32, step));
        cpu.write32(o + SS_DRIFT + k, d);
    }
    let c = cpu.read16(o + SS_COUNT);
    let side = cpu.read8(o + SS_SIDE) as i8 as i32 as u32;
    let to = if c.wrapping_sub(0x1e1) <= 0x21a {
        side.wrapping_mul(0xc000)
    } else {
        (side << 12).wrapping_add(cpu.read16(o + TURN_Y) as i16 as i32 as u32)
    };
    nested!(cpu, 0x0608_db7c, ss_tilt(cpu, o, to & 0xffff));
    let tail = cpu.read32(o + SS_TAIL);
    if cpu.read16(tail + SS_TAIL_FLAGS) & SS_BROKEN == 0 && cpu.read8(RANK) as i8 > 1 {
        let v = cpu.read32(o + SS_TAIL_FLAME);
        let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0, 12));
        cpu.write32(o + SS_TAIL_FLAME, v);
        let c = cpu.read16(o + SS_COUNT);
        if [0x1a4, 0x21c, 0x294, 0x30c, 0x384].contains(&c) {
            nested!(cpu, 0x0608_d2d8, charge_set(cpu, tail + SS_TAIL_PARTS, 64, tail + SS_TAIL_TIP, tail + TURN_Z));
        }
        let c = cpu.read16(o + SS_COUNT);
        if [0x1e0, 0x258, 0x2d0, 0x348, 0x3c0].contains(&c) {
            cpu.write32(o + SS_TAIL_FLAME, 0x100);
            let x = cpu.read32(tail + POSITION);
            cpu.write32(t + actors::E_POSITION, x);
            nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 93));
            for k in [4, 8] {
                let v = cpu.read32(tail + POSITION + k);
                cpu.write32(t + actors::E_POSITION + k, v);
            }
            cpu.write16(t + actors::E_COLOUR, 48);
            cpu.write32(t + actors::E_SCALES + 4, 0x2_0000);
            let a = cpu.read16(tail + TURN_Z);
            cpu.write16(t + actors::E_ANGLE, a);
            nested!(cpu, 0x0605_7e38, explosions::cross_flash(cpu, t));
        }
        if cpu.read32(o + SS_TAIL_FLAME) != 0 {
            let n = cpu.read8(o + NUMBER);
            cpu.write8(t + S_NUMBER, n);
            cpu.write16(t + D_BACK, 0xffe0);
            let size = cpu.read16(o + SS_TAIL_FLAME + 2);
            cpu.write16(t + D_SIZE, size);
            cpu.write16(t + D_STEP, 0xffe0);
            nested!(cpu, 0x0608_cee4, flame_set(cpu, t, tail, tail + SS_TAIL_PARTS));
        }
    }
    if cpu.read16(o + SS_COUNT) > 0x4b0 {
        cpu.write8(o + SS_STATE, 2);
        cpu.write16(o + SS_SLOWNESS, 64);
        cpu.write16(o + SS_WAIT, 240);
        nested!(cpu, 0x0608_d74c, ss_fires_clear(cpu, o));
    }
}

/// Its aim eased a sixteenth of the way to `to`, and its drift toward half a dot along it (by
/// its sweep word a frame).
fn ss_aim_drift(cpu: &mut Cpu, o: u32, to: u32) {
    let from = cpu.read16(o + SS_AIM) as u32;
    let d = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, from, to)) as i32;
    let d = if d < 0 { d + 15 } else { d } >> 4;
    let a = (d as u16).wrapping_add(cpu.read16(o + SS_AIM));
    cpu.write16(o + SS_AIM, a);
    for (k, cos) in [(0, false), (4, true)] {
        let step = cpu.read16(o + SS_SWEEP) as i16 as u32;
        let a = cpu.read16(o + SS_AIM) as i16 as i32 as u32;
        let s = if cos {
            nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a)).wrapping_neg()
        } else {
            nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a))
        };
        let d = cpu.read32(o + SS_DRIFT + k);
        let d = nested!(cpu, 0x0604_6168, actors::approach(cpu, d, (s as i32 / 2) as u32, step));
        cpu.write32(o + SS_DRIFT + k, d);
    }
}

/// State 5: the fast sweep. Its neck bobs the other way; its flame grows to 0xe8 and sweeps it
/// round faster (2 dots a frame); at frame 240 its even segments are told when to fire (20 frames
/// apart), its jaw open after; the flame dies after frame 1320; its drift eased toward the middle;
/// after 1500 frames, hovering (240 frames), its segments' fire cleared.
const SS_JAW: u32 = 0x6f0;
const SS_FIRE_AT: u32 = 0x154;

pub fn ss_fast_sweeping(cpu: &mut Cpu, o: u32) {
    let neck = if cpu.read32(GAME_FRAMES) & 32 != 0 { 0xf000 } else { 0x800 };
    nested!(cpu, 0x0608_dbb8, ss_neck(cpu, o, neck));
    let approach = |cpu: &mut Cpu, at: u32, word: bool, to: u32, step: u32| {
        let v = if word { cpu.read16(o + at) as i16 as u32 } else { cpu.read32(o + at) };
        let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, to, step));
        if word {
            cpu.write16(o + at, v as u16);
        } else {
            cpu.write32(o + at, v);
        }
        v
    };
    if cpu.read16(o + SS_COUNT) > 0x528 {
        approach(cpu, SS_FLAME, true, 0, 1);
        approach(cpu, SS_SWEEP_SPEED, false, 0x2_0000, 0x100);
        let f = cpu.read16(o + SS_FLAME) as i16 as u32;
        let s = nested!(cpu, 0x0604_6168, actors::approach(cpu, f, 0xffff_ff80, 8));
        cpu.write16(o + SS_SWEEP, s as u16);
        cpu.write16(o + SS_JAW, 0);
    } else {
        approach(cpu, SS_FLAME, true, 0xe8, 1);
        approach(cpu, SS_SWEEP_SPEED, false, 0x2_0000, 0x100);
        let f = cpu.read16(o + SS_FLAME) as i16 as u32;
        let s = nested!(cpu, 0x0604_6168, actors::approach(cpu, f, 0x800, 8));
        cpu.write16(o + SS_SWEEP, s as u16);
        if cpu.read16(o + SS_COUNT) == 0xf0 {
            for i in (0..14u32).step_by(2) {
                let s = cpu.read32(o + SS_SEGMENTS + 4 * i);
                cpu.write16(s + SS_FIRE_AT, (20 * i) as u16);
            }
        }
        let jaw = if cpu.read16(o + SS_COUNT) > 0xf0 {
            (cpu.read8(o + SS_SIDE) as i8 as i32 as u32).wrapping_mul(0xc000)
        } else {
            0
        };
        cpu.write16(o + SS_JAW, jaw as u16);
    }
    let side = cpu.read8(o + SS_SIDE) as i8 as i16;
    let f = cpu.read16(o + SS_FLAME) as i16;
    let a = (side.wrapping_mul(f) as u16).wrapping_add(cpu.read16(o + TURN_Z));
    cpu.write16(o + TURN_Z, a);
    let side = cpu.read8(o + SS_SIDE) as i8 as i32 as u32;
    let to = (side << 12).wrapping_add(cpu.read16(o + TURN_Y) as i16 as i32 as u32);
    nested!(cpu, 0x0608_db7c, ss_tilt(cpu, o, to & 0xffff));
    for (k, cos) in [(0, false), (4, true)] {
        let v = cpu.read32(o + SS_SWEEP_SPEED);
        let a = cpu.read16(o + TURN_Z) as i16 as i32 as u32;
        let s = if cos {
            nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a)).wrapping_neg()
        } else {
            nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a))
        };
        let m = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, v));
        cpu.write32(o + SS_SPEED + k, m);
    }
    let x = cpu.read32(o + POSITION).wrapping_neg();
    let y = cpu.read32(o + POSITION + 4).wrapping_neg();
    let a = nested!(cpu, 0x0606_780c, sgl::sl_atan(cpu, x, y));
    ss_aim_drift(cpu, o, (a as i16 as i32 as u32).wrapping_add(0x4000));
    if cpu.read16(o + SS_COUNT) > 0x5dc {
        cpu.write8(o + SS_STATE, 2);
        cpu.write16(o + SS_SLOWNESS, 64);
        cpu.write16(o + SS_WAIT, 240);
        nested!(cpu, 0x0608_d74c, ss_fires_clear(cpu, o));
    }
}

/// Its heading eased a sixteenth of the way toward the point (`dx`, `dy`) off; its tilt the
/// other way.
fn ss_head_to(cpu: &mut Cpu, o: u32, dx: u32, dy: u32) {
    let a = nested!(cpu, 0x0606_780c, sgl::sl_atan(cpu, dx, dy));
    let to = (a as i16 as i32 as u32).wrapping_add(0x4000);
    let from = cpu.read16(o + TURN_Z) as i16 as i32 as u32;
    let d = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, from, to)) as i32;
    let d = if d < 0 { d + 15 } else { d } >> 4;
    let a = (d as u16).wrapping_add(cpu.read16(o + TURN_Z));
    cpu.write16(o + TURN_Z, a);
    nested!(cpu, 0x0608_db7c, ss_tilt(cpu, o, 0x8000u16.wrapping_sub(a) as u32));
}

/// A point circling the nearer player (`across` and `down` dots out, by the frame), less its
/// place.
fn ss_round_player(cpu: &mut Cpu, o: u32, p: u32, across: u32, down_shift: u32) -> (u32, u32) {
    let a = (cpu.read32(GAME_FRAMES) << 8) as i16 as i32 as u32;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    let dx = cpu.read32(p + POSITION).wrapping_add(s.wrapping_mul(across)).wrapping_sub(cpu.read32(o + POSITION));
    let a = (cpu.read32(GAME_FRAMES) << 8) as i16 as i32 as u32;
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    let dy = cpu.read32(p + POSITION + 4).wrapping_sub(c << down_shift).wrapping_sub(cpu.read32(o + POSITION + 4));
    (dx, dy)
}

/// State 6 (after it dives in): the fireballs. It heads for a point swinging 96 dots across the nearer player (16
/// up and down), 1 3/4 dots a frame across and 1 1/2 down, its drift keeping its tenth segment at
/// the top; its neck back from frame 211 to 808; from frame 241 to 779, every so many frames by
/// the rank (the bytes at 0x060a42e0) four fireballs (sound 94) fanned out from its mouth with a
/// glow; after 900 frames, hovering (240 frames), turning faster.
const SS_FIREBALL_GAPS: u32 = 0x060a_42e0;
const SS_FAN: [u16; 4] = [0xe800, 0xf800, 0x800, 0x1800];
const SS_SEGMENT_10: u32 = SS_SEGMENTS + 4 * 10;

pub fn ss_fireballs(cpu: &mut Cpu, o: u32) {
    let p = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + 0x10));
    let (dx, dy) = ss_round_player(cpu, o, p, 96, 4);
    ss_head_to(cpu, o, dx, dy);
    for (k, cos, v) in [(0, false, 0x1_c000), (4, true, 0x1_8000)] {
        let a = cpu.read16(o + TURN_Z) as i16 as i32 as u32;
        let s = if cos {
            nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a)).wrapping_neg()
        } else {
            nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a))
        };
        let m = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, v));
        cpu.write32(o + SS_SPEED + k, m);
    }
    let s = cpu.read32(o + SS_SEGMENT_10);
    let d = 0xff10_0000u32.wrapping_sub(cpu.read32(s + POSITION + 4));
    cpu.write32(o + SS_DRIFT + 4, d);
    let c = cpu.read16(o + SS_COUNT);
    let neck = if c.wrapping_sub(0xd3) <= 0x256 { 0xf000 } else { 0x800 };
    nested!(cpu, 0x0608_dbb8, ss_neck(cpu, o, neck));
    let c = cpu.read16(o + SS_COUNT);
    if c.wrapping_sub(0xf1) <= 0x21a {
        let rank = cpu.read8(RANK) as i8 as i32 as u32;
        let gap = cpu.read8(SS_FIREBALL_GAPS.wrapping_add(rank)) as u32;
        if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, c as u32, gap)) == 0 {
            nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 94));
            for turn in SS_FAN {
                let a = cpu.read16(o + TURN_Z).wrapping_add(turn) as u32;
                nested!(cpu, 0x0608_d684, ss_fireball(cpu, o, a));
            }
            nested!(cpu, 0x0608_d6c8, ss_glow(cpu, o));
        }
    }
    if cpu.read16(o + SS_COUNT) > 0x384 {
        cpu.write8(o + SS_STATE, 2);
        cpu.write16(o + SS_SLOWNESS, 16);
        cpu.write16(o + SS_WAIT, 240);
    }
}

/// Its speed along its heading at joint 1's speed, its drift easing out, its pose stepped.
fn ss_cruise(cpu: &mut Cpu, o: u32) {
    for (k, cos) in [(0, false), (4, true)] {
        let joints = cpu.read32(o + SS_JOINTS);
        let v = ((cpu.read16(joints + 2) as i16 as i32) << 4) as u32;
        let a = cpu.read16(o + TURN_Z) as i16 as i32 as u32;
        let s = if cos {
            nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a)).wrapping_neg()
        } else {
            nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a))
        };
        let m = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, v));
        cpu.write32(o + SS_SPEED + k, m);
    }
    for k in [0, 4] {
        let d = cpu.read32(o + SS_DRIFT + k);
        let d = nested!(cpu, 0x0604_6168, actors::approach(cpu, d, 0, 0x400));
        cpu.write32(o + SS_DRIFT + k, d);
    }
    nested!(cpu, 0x0605_6644, models::pose_step(cpu, o + SS_POSE));
}

/// The breath (states 7 to 10) by its pose: rearing (7, 9) its neck forward until the script's
/// beat (then the next state, the flame cleared); breathing (8, 10) its neck back, a charge at
/// the second beat (the count restarted), at frame 50 a flash and a flame of 0x400 dying by 64 a
/// frame; at the script's end, near enough the middle, hovering (80 frames), turning faster.
pub fn ss_breathing(cpu: &mut Cpu, o: u32) {
    let t = cpu.stack();
    ss_cruise(cpu, o);
    let s = cpu.read8(o + SS_STATE);
    if s == 7 || s == 9 {
        if cpu.read16(o + SS_SCRIPT_FLAGS) & SS_BEAT != 0 {
            cpu.write8(o + SS_STATE, s.wrapping_add(1));
            cpu.write16(o + SS_COUNT, 0);
            cpu.write16(o + SS_FLAME, 0);
        }
        nested!(cpu, 0x0608_dbb8, ss_neck(cpu, o, 0x800));
    } else {
        nested!(cpu, 0x0608_dbb8, ss_neck(cpu, o, 0xf000));
        if cpu.read16(o + SS_SCRIPT_FLAGS) & 2 != 0 {
            nested!(cpu, 0x0608_d2d8, charge_set(cpu, o + SS_PARTS, 64, o + SS_MOUTH, o + TURN_Z));
            cpu.write16(o + SS_COUNT, 0);
        }
        if cpu.read16(o + SS_COUNT) == 50 {
            nested!(cpu, 0x0608_d3bc, ss_flash(cpu, o));
            cpu.write16(o + SS_FLAME, 0x400);
        }
        let f = cpu.read16(o + SS_FLAME) as i16 as u32;
        let f = nested!(cpu, 0x0604_6168, actors::approach(cpu, f, 0, 64)) as u16;
        cpu.write16(o + SS_FLAME, f);
        if f != 0 {
            let n = cpu.read8(o + NUMBER);
            cpu.write8(t + S_NUMBER, n);
            cpu.write16(t + D_BACK, 24);
            let size = cpu.read16(o + SS_FLAME);
            cpu.write16(t + D_SIZE, size);
            cpu.write16(t + D_STEP, 32);
            nested!(cpu, 0x0608_cee4, flame_set(cpu, t, o, o + SS_PARTS));
        }
    }
    if cpu.read16(o + SS_SCRIPT_FLAGS) & SS_SCRIPT_END != 0
        && (cpu.read32(o + POSITION + 4) as i32 > 0xa0_0000 || cpu.read32(o + POSITION).wrapping_add(0xc0_0000) > 0x180_0000)
    {
        cpu.write8(o + SS_STATE, 2);
        cpu.write16(o + SS_SLOWNESS, 16);
        cpu.write16(o + SS_WAIT, 80);
    }
}

/// Its heading eased a fortieth of the way to `to` (at most 0x180 a frame), tilted to `tilt`.
fn ss_rear_turn(cpu: &mut Cpu, o: u32, to: u32, tilt: u32) {
    let from = cpu.read16(o + TURN_Z) as i16 as i32 as u32;
    let d = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, from, to)) as i32;
    let q = ((d / 40) as i16).clamp(-0x180, 0x180);
    let a = (q as u16).wrapping_add(cpu.read16(o + TURN_Z));
    cpu.write16(o + TURN_Z, a);
    nested!(cpu, 0x0608_db7c, ss_tilt(cpu, o, tilt));
}

/// States 7 and 8: the breath from the right, heading by joint 0.
pub fn ss_breath_right(cpu: &mut Cpu, o: u32) {
    let joints = cpu.read32(o + SS_JOINTS);
    let to = cpu.read16(joints) as i16 as i32 as u32;
    ss_rear_turn(cpu, o, to, 0x2000);
    nested!(cpu, 0x0608_e9ac, ss_breathing(cpu, o));
}

/// States 9 and 10: the breath from the left, heading against joint 0.
pub fn ss_breath_left(cpu: &mut Cpu, o: u32) {
    let joints = cpu.read32(o + SS_JOINTS);
    let to = (cpu.read16(joints) as i16 as i32 as u32).wrapping_neg();
    ss_rear_turn(cpu, o, to, 0x6000);
    nested!(cpu, 0x0608_e9ac, ss_breathing(cpu, o));
}

/// State 11: the coil. Its neck forward, heading a thirty-second of the way against joint 0,
/// tilted by its side of the screen, cruising by its pose; once more than 640 dots up, it dives
/// in again (state 6).
const SS_DIVE_AT: i32 = -0x280_0000;

pub fn ss_coiling(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_dbb8, ss_neck(cpu, o, 0x800));
    let joints = cpu.read32(o + SS_JOINTS);
    let from = cpu.read16(o + TURN_Z) as i16 as i32 as u32;
    let to = (cpu.read16(joints) as i16 as i32 as u32).wrapping_neg();
    let d = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, from, to)) as i32;
    let d = if d < 0 { d + 31 } else { d } >> 5;
    let a = (d as u16).wrapping_add(cpu.read16(o + TURN_Z));
    cpu.write16(o + TURN_Z, a);
    let tilt = if cpu.read32(o + POSITION) as i32 > 0 { 0x4000 } else { 0xc000 };
    nested!(cpu, 0x0608_db7c, ss_tilt(cpu, o, tilt));
    ss_cruise(cpu, o);
    if (cpu.read32(o + POSITION + 4) as i32) < SS_DIVE_AT {
        nested!(cpu, 0x0608_d9c0, ss_dive_in(cpu, o));
    }
}

/// State 12: beaten; gone after 210 frames.
pub fn ss_beaten(cpu: &mut Cpu, o: u32) {
    if cpu.read16(o + SS_COUNT) > 0xd2 {
        cpu.write8(o + OBJ_MODE, TO_STOP);
    }
}

/// The serpent's behaviour. Its life on the boss gauge; beaten (its life gone, or twice through
/// its plan) before state 12: state 12 (sound 19), the bullets cleared, its head and tail parts
/// shut, still, the stage's score (30000, and 30 more to the rate, if shot down). Its state's
/// function; then it moves by its speed and drift, its speed's whole dots each frame into its
/// trail (with its turns), its turns and neck turned (its neck at most 0x800 forward); counted.
const SS_LIFE: u32 = 0xe4;
const SS_TAIL_PART: u32 = 0x13c;
const SS_PARTS_SHUT: u16 = 0xe0;
const SS_STATES: u32 = 0x060a_4304;
const SS_MOVE_LEFT: u32 = 0x680;
const SS_MOVED: u32 = 0x18e;

pub fn ss_behaviour(cpu: &mut Cpu, o: u32) {
    let life = cpu.read32(o + SS_LIFE);
    nested!(cpu, 0x0604_64b8, hud::boss_gauge_life(cpu, life));
    if cpu.read8(o + SS_STATE) <= 11
        && ((cpu.read32(o + SS_LIFE) as i32) <= 0 || cpu.read16(o + SS_LOOPS) as i16 > 1)
    {
        cpu.write8(o + SS_STATE, 12);
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 19));
        nested!(cpu, 0x0605_61e0, screen::bullets_cleared(cpu));
        for at in [SS_PARTS, SS_TAIL_PART] {
            let f = cpu.read16(o + at) | SS_PARTS_SHUT;
            cpu.write16(o + at, f);
        }
        cpu.write16(o + SS_COUNT, 0);
        for k in [0, 4, 8] {
            cpu.write32(o + SS_SPEED + k, 0);
        }
        if (cpu.read32(o + SS_LIFE) as i32) <= 0 {
            cpu.write32(STAGE_SCORE_AT, 30000);
            let r = cpu.read32(STAGE_RATE_AT).wrapping_add(30);
            cpu.write32(STAGE_RATE_AT, r);
        } else {
            cpu.write32(STAGE_SCORE_AT, 0);
        }
        cpu.write32(o + SS_LIFE, 0);
    }
    let state = cpu.read8(o + SS_STATE) as u32;
    match state {
        0 | 1 => nested!(cpu, 0x0608_dbf4, ss_coming(cpu, o)),
        2 => nested!(cpu, 0x0608_dd70, ss_hovering(cpu, o)),
        3 => nested!(cpu, 0x0608_dec0, ss_lunging(cpu, o)),
        4 => nested!(cpu, 0x0608_e14c, ss_sweeping(cpu, o)),
        5 => nested!(cpu, 0x0608_e52c, ss_fast_sweeping(cpu, o)),
        6 => nested!(cpu, 0x0608_e7c8, ss_fireballs(cpu, o)),
        7 | 8 => nested!(cpu, 0x0608_eb9c, ss_breath_right(cpu, o)),
        9 | 10 => nested!(cpu, 0x0608_ec20, ss_breath_left(cpu, o)),
        11 => nested!(cpu, 0x0608_eca4, ss_coiling(cpu, o)),
        12 => nested!(cpu, 0x0608_edd0, ss_beaten(cpu, o)),
        _ => {
            let f = cpu.read32(SS_STATES + 4 * state);
            cpu.call(f, &[o]);
        }
    }
    let dots = |v: u32| -> i32 { (v as i32) / 0x1_0000 };
    for k in 0..3u32 {
        let left = cpu.read32(o + SS_MOVE_LEFT + 4 * k);
        let now = left.wrapping_add(cpu.read32(o + SS_SPEED + 4 * k));
        cpu.write32(o + SS_MOVE_LEFT + 4 * k, now);
        cpu.write8(o + SS_MOVED + k, dots(now).wrapping_sub(dots(left)) as u8);
        let step = cpu.read32(o + SS_MOVE_LEFT + 4 * k).wrapping_sub(left).wrapping_add(cpu.read32(o + SS_DRIFT + 4 * k));
        let p = cpu.read32(o + POSITION + 4 * k).wrapping_add(step);
        cpu.write32(o + POSITION + 4 * k, p);
    }
    for k in 0..3u32 {
        let m = cpu.read8(o + SS_MOVED + k) as i8 as i32 as u32;
        nested!(cpu, 0x0605_62b4, actors::shift_in_bytes(cpu, o + SS_TRAIL_X + SS_TRAIL * k, m, SS_TRAIL));
    }
    for (turn, trail) in [(TURN_X, SS_TRAIL_TURN_X), (TURN_Y, SS_TRAIL_TURN_Y), (TURN_Z, SS_TRAIL_TURN_Z)] {
        let a = cpu.read16(o + turn) as i16 as i32;
        let a = if a < 0 { a + 0xff } else { a } as u32;
        nested!(cpu, 0x0600_8164, clib::ashiftrt_r4::<8>(cpu, a));
        let a = (a as i32 >> 8) as i8 as i32 as u32;
        nested!(cpu, 0x0605_62b4, actors::shift_in_bytes(cpu, o + trail, a, SS_TRAIL));
    }
    for (turn, speed) in [(TURN_X, SS_TURN_X_SPEED), (TURN_Y, SS_TURN_Y_SPEED), (TURN_Z, 0x8c)] {
        let a = cpu.read16(o + turn).wrapping_add(cpu.read16(o + speed));
        cpu.write16(o + turn, a);
    }
    let n = cpu.read16(o + SS_NECK).wrapping_add(cpu.read16(o + SS_NECK_SPEED));
    cpu.write16(o + SS_NECK, n);
    if (n as i16) >= 0 && n > 0x800 {
        cpu.write16(o + SS_NECK, 0x800);
    }
    let c = cpu.read16(o + SS_COUNT).wrapping_add(1);
    cpu.write16(o + SS_COUNT, c);
}

// ---- the serpent's segments ---------------------------------------------------------------------------

/// A segment of the serpent (an actor drawn by the serpent's code): its part's flags (a word,
/// +0x70: bit 6 broken), how far its smoke has got (+0x72), its life (+0x74), its smoke's points
/// (12 bytes each, from +0x78), its head (+0x140), how far back along the head's trail it is (a
/// word, +0x144), its place in the body (a byte, +0x146), frames until it fires (a word, +0x154),
/// its gun's place (x, y: +0x158), its tilt's turn (words: +0x164, its speed +0x166).
const SG_FLAGS: u32 = 0x70;
const SG_SMOKE: u32 = 0x72;
const SG_LIFE: u32 = 0x74;
const SG_SMOKE_POINTS: u32 = 0x78;
const SG_HEAD: u32 = 0x140;
const SG_BACK: u32 = 0x144;
const SG_INDEX: u32 = 0x146;
const SG_GUN: u32 = 0x158;
const SG_TURN: u32 = 0x164;
const SG_TURN_SPEED: u32 = 0x166;
const SG_BROKEN: u16 = 0x40;

/// A glow round its gun (an orbiter: kind 2, colours 1, 5/16 size shrinking by 0x800 a frame,
/// at its angle), until its part is broken.
pub fn sg_glow(cpu: &mut Cpu, s: u32) {
    static PROLOGUE: std::sync::OnceLock<super::Prologue> = std::sync::OnceLock::new();
    let t = cpu.stack();
    cpu.write16(t + OD_KIND, 2);
    cpu.write16(t + OD_COLOURS, 1);
    cpu.write32(t + OD_SCALES, 0x5000);
    cpu.write32(t + OD_SCALES + 4, 0x5000);
    cpu.write32(t + OD_SCALE_SPEED, 0xffff_f800);
    cpu.write32(t + OD_SCALE_SPEED + 4, 0xffff_f800);
    let a = cpu.read16(s + TURN_Z);
    cpu.write16(t + OD_ANGLE, a);
    cpu.write16(t + OD_ANGLE_OFFSET, 0);
    cpu.write16(t + OD_TURN, 0);
    cpu.write16(t + OD_LIFT, 0);
    cpu.write32(t + OD_RADIUS, 0);
    cpu.write32(t + OD_RADIUS + 4, 0);
    let angle_from = s + TURN_Z;
    cpu.nested_with_stack(&PROLOGUE, 0x0605_9554, &[angle_from], |cpu| {
        orbits::orbit_set(cpu, t, s + SG_FLAGS, 0x40, s + SG_GUN, angle_from)
    });
}

/// A segment's shot (a bullet): after 120 frames it no longer hits (bit 10 of its hit flags).
const SG_SHOT_AGE: u32 = 0xa8;
const SG_SHOT_HARMLESS: u16 = 0x400;

pub fn sg_shot_behaviour(cpu: &mut Cpu, b: u32) {
    let n = cpu.read16(b + SG_SHOT_AGE).wrapping_add(1);
    cpu.write16(b + SG_SHOT_AGE, n);
    if n as i16 > 120 {
        let f = cpu.read16(b + HIT_FLAGS) | SG_SHOT_HARMLESS;
        cpu.write16(b + HIT_FLAGS, f);
    }
    nested!(cpu, 0x0605_3ccc, bullets::bullet_move(cpu, b));
}

/// A segment's behaviour. Its life gone, or the head's: broken (5 more to the stage's rate if
/// shot). With the head beaten it falls away turning, smoking, until off the screen or the head
/// ends; else it follows the head's trail (its place and turns from as far back as it is; its
/// tilt eased toward the trail's, or toward its fire flag's when that is set), drifting with the
/// head. Even segments with the head's jaw open fire (a glow and a shot along its heading, 1 1/8
/// dots a frame) when their tilt is near the jaw's, every 120 frames; broken ones smoke.
const SG_FALL_AREA: [u32; 4] = [0xff40_0000, 0x00c0_0000, 0xff10_0000, 0x00f0_0000];
const SG_SMOKE_SPREAD: u32 = 0x28_0000;
const SG_SHOT_GAP: u16 = 120;
const SG_SHOT_SPEED: u32 = 0x1_2000;
const SG_SHOT_BEHAVIOUR: u32 = 0x0608_f12c;

pub fn sg_behaviour(cpu: &mut Cpu, s: u32) {
    let t = cpu.stack();
    let head = cpu.read32(s + SG_HEAD);
    if cpu.read8(s + SG_SMOKE) == 0
        && ((cpu.read32(s + SG_LIFE) as i32) <= 0 || (cpu.read32(head + SS_LIFE) as i32) <= 0)
    {
        cpu.write8(s + SG_SMOKE, 1);
        let f = cpu.read16(s + SG_FLAGS) | SG_BROKEN;
        cpu.write16(s + SG_FLAGS, f);
        if (cpu.read32(s + SG_LIFE) as i32) <= 0 {
            let r = cpu.read32(STAGE_RATE_AT).wrapping_add(5);
            cpu.write32(STAGE_RATE_AT, r);
        }
    }
    if cpu.read8(head + SS_STATE) > 11 {
        if cpu.read8(head + OBJ_MODE) as i8 > ENDING
            || cpu.nested_with_stack(&aim::AREA_FLAGS_PROLOGUE, 0x0605_69f8, &[SG_FALL_AREA[3]], |cpu| {
                aim::area_flags(cpu, s + 0x10, SG_FALL_AREA[0], SG_FALL_AREA[1], SG_FALL_AREA[2], SG_FALL_AREA[3])
            }) as u8
                != 0
        {
            cpu.write8(s + OBJ_MODE, TO_STOP);
            return;
        }
        let a = cpu.read16(s + TURN_Z).wrapping_add(0x4000) as i16 as i32 as u32;
        let v = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
        let x = cpu.read32(s + POSITION).wrapping_add(v.wrapping_mul(2));
        cpu.write32(s + POSITION, x);
        let a = cpu.read16(s + TURN_Z).wrapping_add(0x4000) as i16 as i32 as u32;
        let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
        let y = cpu.read32(s + POSITION + 4).wrapping_sub(c.wrapping_mul(2));
        cpu.write32(s + POSITION + 4, y);
        let z = cpu.read16(s + TURN_Z).wrapping_add(16);
        cpu.write16(s + TURN_Z, z);
        let y = cpu.read16(s + TURN_Y).wrapping_add(0x400);
        cpu.write16(s + TURN_Y, y);
        let a = cpu.read16(s + TURN_Y) as i16 as i32;
        let a = if a < 0 { a + 0x3ff } else { a } as u32;
        nested!(cpu, 0x0600_8160, clib::ashiftrt_r4::<10>(cpu, a));
        if (a as i32 >> 10) & 3 == 0 {
            for k in [0, 4, 8] {
                let r = nested!(cpu, 0x0604_5d90, math::random(cpu, SG_SMOKE_SPREAD));
                let p = cpu.read32(s + POSITION + k).wrapping_add(r);
                cpu.write32(t + k, p);
            }
            let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
            cpu.write32(t + 12, 0xc000u32.wrapping_sub((r & 7) * 0x600));
            nested!(cpu, 0x0605_77d0, explosions::smoke_puff(cpu, t));
        }
    } else {
        let back = |cpu: &mut Cpu| head.wrapping_add(cpu.read16(s + SG_BACK) as u32);
        for (k, trail) in [(0, SS_TRAIL_X), (4, SS_TRAIL_Y), (8, SS_TRAIL_X + 2 * SS_TRAIL)] {
            let at = back(cpu);
            let d = (cpu.read8(at + trail) as i8 as i32 as u32) << 16;
            let p = cpu.read32(s + POSITION + k).wrapping_add(d);
            cpu.write32(s + POSITION + k, p);
        }
        let at = back(cpu);
        let a = (cpu.read8(at + SS_TRAIL_TURN_X) as i8 as i32 as u32) << 8;
        cpu.write16(s + TURN_X, a as u16);
        let index = cpu.read8(s + SG_INDEX) as u32;
        let fire = cpu.read8(head + index + SS_FIRES) as i8 as i32 as u32;
        let to = if fire & 1 == 0 {
            let at = back(cpu);
            cpu.read8(at + SS_TRAIL_TURN_Y) as i8 as i32 as u32
        } else {
            fire & 0xf0
        };
        let to = (to << 8) & 0xffff;
        let from = cpu.read16(s + TURN_Y) as u32;
        cpu.push(&[0x800, 4, 16]);
        nested!(cpu, 0x0605_70d8, aim::turn_toward(cpu, s + SG_TURN, s + SG_TURN_SPEED, from, to, 0x800, 4, 16));
        let a = cpu.read16(s + TURN_Y).wrapping_add(cpu.read16(s + SG_TURN));
        cpu.write16(s + TURN_Y, a);
        let at = back(cpu);
        let a = (cpu.read8(at + SS_TRAIL_TURN_Z) as i8 as i32 as u32) << 8;
        cpu.write16(s + TURN_Z, a as u16);
        for k in [0, 4] {
            let p = cpu.read32(s + POSITION + k).wrapping_add(cpu.read32(head + SS_DRIFT + 4 * (k / 4)));
            cpu.write32(s + POSITION + k, p);
        }
        cpu.pop(12);
        let p = cpu.read32(s + POSITION + 8).wrapping_add(cpu.read32(head + SS_DRIFT + 8));
        cpu.write32(s + POSITION + 8, p);
    }
    let w = cpu.read16(s + SS_FIRE_AT) as i16 as u32;
    let w = nested!(cpu, 0x0604_6168, actors::approach(cpu, w, 0, 1)) as u16;
    cpu.write16(s + SS_FIRE_AT, w);
    let index = cpu.read8(s + SG_INDEX);
    let smokes = if index == 13 {
        12
    } else if index & 1 != 0 {
        6
    } else {
        let jaw = cpu.read16(head + SS_JAW);
        if jaw != 0
            && cpu.read16(s + SG_FLAGS) & SG_BROKEN == 0
            && w == 0
            && jaw.wrapping_sub(cpu.read16(s + TURN_Y)).wrapping_add(15) <= 30
        {
            nested!(cpu, 0x0608_f0bc, sg_glow(cpu, s));
            cpu.write16(s + SS_FIRE_AT, SG_SHOT_GAP);
            let n = cpu.read8(s + NUMBER);
            cpu.write8(t + S_NUMBER, n);
            let x = cpu.read32(s + SG_GUN);
            cpu.write32(t + S_POSITION, x);
            let y = cpu.read32(s + SG_GUN + 4);
            cpu.write32(t + S_POSITION + 4, y);
            cpu.write32(t + S_POSITION + 8, 0);
            let a = cpu.read16(s + TURN_Z).wrapping_sub(cpu.read16(s + TURN_Y));
            cpu.write16(t + S_ANGLE, a);
            cpu.write16(t + S_FLAGS, 2);
            cpu.write32(t + S_SPEED, SG_SHOT_SPEED);
            let b = nested!(cpu, 0x0605_3db0, bullets::bullet_set(cpu, t));
            if b != 0 {
                cpu.write32(b + BEHAVIOUR, SG_SHOT_BEHAVIOUR);
            }
        }
        2
    };
    let k = cpu.read8(s + SG_SMOKE) as u32;
    if k != 0 && ((k - 1) as i32) < smokes {
        let at = s + (k - 1) * 12 + SG_SMOKE_POINTS;
        let made = nested!(cpu, 0x0605_968c, explosions::smoke_and_sparks(cpu, at, 4));
        if made as u8 != 0 {
            let k = cpu.read8(s + SG_SMOKE).wrapping_add(1);
            cpu.write8(s + SG_SMOKE, k);
        }
    }
}

/// Kind `kind`'s model `n`: its hit one by `flags`' bit 0 (or its first, `flags` None).
fn put_kind_model(cpu: &mut Cpu, kind_n: u32, n: u32, flags: Option<u16>) {
    let file = cpu.read32(kind(kind_n));
    let models = cpu.read32(file + F_MODELS);
    let pair = cpu.read32(models + 4 * n);
    let at = flags.map_or(0, |f| 4 * (f & 1) as u32);
    let model = cpu.read32(pair + at);
    cpu.call(SL_PUT_POLYGON, &[model]);
}

/// A part's flags for drawing: drawn hit last time (bit 1) clears its hit bits; hit (bit 0)
/// marks it drawn hit.
fn hit_flags_drawn(cpu: &mut Cpu, at: u32) {
    let mut f = cpu.read16(at);
    if f & 2 != 0 {
        cpu.write16(at, f & 0xfffc);
        f = cpu.read16(at);
    }
    if f & 1 != 0 {
        cpu.write16(at, f | 2);
    }
}

/// The serpent's head drawn (`s` its sprite part): kind 55's model by its turns (heading over by
/// half a turn; tilted, then turned, while coming in) and its neck, with its points and mouth,
/// unless hidden; its jaw opened by twice its neck the other way. Its tail's hits show on it.
const SS_MODEL_KIND: u32 = 55;
const SS_HEAD_POINTS: (u32, u32) = (0x060a_4060, 7);
const SS_JAW_POINTS: (u32, u32) = (0x060a_4098, 6);
const SS_HIDDEN: u16 = 0x80;

pub fn ss_draw(cpu: &mut Cpu, s: u32) {
    let o = s - 0x10;
    hit_flags_drawn(cpu, o + SS_PARTS);
    hit_flags_drawn(cpu, o + SS_TAIL_PART);
    let f = (cpu.read16(o + SS_TAIL_PART) & 1) | cpu.read16(o + SS_PARTS);
    cpu.write16(o + SS_PARTS, f);
    cpu.call(SL_PUSH_MATRIX, &[]);
    let p = [0, 4, 8].map(|k| cpu.read32(o + POSITION + k));
    cpu.call(SL_TRANSLATE, &p);
    let heading = |cpu: &mut Cpu| cpu.read16(o + TURN_Z).wrapping_add(0x8000) as i16 as i32 as u32;
    let tilt = |cpu: &mut Cpu| cpu.read16(o + TURN_X).wrapping_add(cpu.read16(o + SS_NECK)) as i16 as i32 as u32;
    if cpu.read8(o + SS_STATE) > 1 {
        let a = heading(cpu);
        cpu.call(SL_ROT_Z, &[a]);
        let a = cpu.read16(o + TURN_Y) as i16 as i32 as u32;
        cpu.call(SL_ROT_Y, &[a]);
        let a = tilt(cpu);
        cpu.call(SL_ROT_X, &[a]);
    } else {
        let a = tilt(cpu);
        cpu.call(SL_ROT_X, &[a]);
        let a = heading(cpu);
        cpu.call(SL_ROT_Z, &[a]);
        let a = cpu.read16(o + TURN_Y) as i16 as i32 as u32;
        cpu.call(SL_ROT_Y, &[a]);
    }
    gs_points(cpu, SS_HEAD_POINTS, o + 0xe8);
    cpu.call(SL_CALC_POINT, &[0, 0x16_0000, 0x14_0000, o + SS_MOUTH]);
    let f = cpu.read16(o + SS_PARTS);
    if f & SS_HIDDEN == 0 {
        put_kind_model(cpu, SS_MODEL_KIND, 0, Some(f));
        cpu.call(SL_PUSH_MATRIX, &[]);
        cpu.call(SL_TRANSLATE, &[0, 0, 0]);
        let n = cpu.read16(o + SS_NECK) as u32;
        let a = (n.wrapping_neg().wrapping_mul(2)) as i16 as i32 as u32;
        cpu.call(SL_ROT_X, &[a]);
        gs_points(cpu, SS_JAW_POINTS, o + 0x140);
        let f = cpu.read16(o + SS_PARTS);
        put_kind_model(cpu, SS_MODEL_KIND, 1, Some(f));
        cpu.call(SL_POP_MATRIX, &[]);
    }
    cpu.call(SL_POP_MATRIX, &[]);
}

/// A segment drawn (`s` its sprite part): kind 55's model by its turns, unless hidden: the tail
/// (moved 21.5 dots up, its points and gun), the even segments (with a gun), the odd ones (two
/// kinds by turns); broken ones as wreckage.
const SG_TAIL_POINTS: (u32, u32) = (0x060a_4108, 12);
const SG_EVEN_POINTS: (u32, u32) = (0x060a_40c8, 2);
const SG_ODD_POINTS: (u32, u32) = (0x060a_40d8, 6);

pub fn sg_draw(cpu: &mut Cpu, sp: u32) {
    let s = sp - 0x10;
    hit_flags_drawn(cpu, s + SG_FLAGS);
    cpu.call(SL_PUSH_MATRIX, &[]);
    let p = [0, 4, 8].map(|k| cpu.read32(s + POSITION + k));
    cpu.call(SL_TRANSLATE, &p);
    let a = cpu.read16(s + TURN_X) as i16 as i32 as u32;
    cpu.call(SL_ROT_X, &[a]);
    let a = cpu.read16(s + TURN_Z).wrapping_add(0x8000) as i16 as i32 as u32;
    cpu.call(SL_ROT_Z, &[a]);
    let a = cpu.read16(s + TURN_Y) as i16 as i32 as u32;
    cpu.call(SL_ROT_Y, &[a]);
    if cpu.read16(s + SG_FLAGS) & SS_HIDDEN == 0 {
        let index = cpu.read8(s + SG_INDEX);
        let (n, broken) = if index == 13 {
            cpu.call(SL_PUSH_MATRIX, &[]);
            cpu.call(SL_TRANSLATE, &[0, 0xffea_8000, 0]);
            gs_points(cpu, SG_TAIL_POINTS, s + SG_SMOKE_POINTS);
            cpu.call(SL_CALC_POINT, &[0, 0, 0, s + SG_GUN]);
            (5, 8)
        } else if index & 1 == 0 {
            gs_points(cpu, SG_EVEN_POINTS, s + SG_SMOKE_POINTS);
            cpu.call(SL_CALC_POINT, &[0, 0, 0x1a_0000, s + SG_GUN]);
            (2, 6)
        } else {
            gs_points(cpu, SG_ODD_POINTS, s + SG_SMOKE_POINTS);
            (if cpu.read8(s + SG_INDEX) & 2 == 0 { 3 } else { 4 }, 7)
        };
        let f = cpu.read16(s + SG_FLAGS);
        if f & SG_BROKEN == 0 {
            put_kind_model(cpu, SS_MODEL_KIND, n, Some(f));
        } else {
            put_kind_model(cpu, SS_MODEL_KIND, broken, None);
        }
        if index == 13 {
            cpu.call(SL_POP_MATRIX, &[]);
        }
    }
    cpu.call(SL_POP_MATRIX, &[]);
}

/// The serpent set up as `n` (its own draw), 256 dots below the screen: its state, counts,
/// speeds, turns and trail cleared (the turns' trails at its turns), its life 85000 on its head
/// and tail parts (parts::parts_set's: 7 and 6 points); its 14 segments made behind it (each
/// numbered, though the numbered table points at the head; as far back along the trail as 15 a
/// segment, 14 more; the tail 75000 life, even segments 45000 less 2000 each along, odd ones 65000
/// less 3000 each), its pose (3 joints at +0xbc, their speeds at +0xc2, its model's at
/// 0x060a41d0), its script, and the boss gauge. No room for a segment: no more.
const SS_BEHAVIOUR: u32 = 0x0608_edf0;
const SS_DRAW: u32 = 0x0608_f510;
const SS_FLAGS: u32 = 0xc8_0000;
const SS_START_Y: u32 = 0x100_0000;
const SS_START_Z: u32 = 0x73_0e80;
const SS_START_LIFE: u32 = 85000;
const SS_START_SLOWNESS: u16 = 64;
const SS_SEGMENT_SIZE: u32 = 0x168;
const SG_BEHAVIOUR: u32 = 0x0608_f164;
const SG_DRAW: u32 = 0x0608_f74c;
const SS_POSE_START: u32 = 0x060a_41a4;
const SS_SCRIPT_START: u32 = 0x060a_4214;
const SS_POSE_MODEL: u32 = 0x060a_41d0;
/// Each segment's points, count, life (the first's, less each along), by kind.
const SG_TAIL: (u32, u32, u32, u32) = (0x060a_4108, 12, 0x1_24f8, 0);
const SG_EVEN: (u32, u32, u32, u32) = (0x060a_40c8, 2, 0xafc8, 0x7d0);
const SG_ODD: (u32, u32, u32, u32) = (0x060a_40d8, 6, 0xfde8, 0xbb8);

pub fn ss_set(cpu: &mut Cpu, o: u32, n: u32) {
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + 0x10));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HITS_AT));
    cpu.write32(o + BEHAVIOUR, SS_BEHAVIOUR);
    // the SH-2 code keeps the number's address in its frame
    let t = cpu.stack();
    cpu.write32(t, o + NUMBER);
    let f = cpu.read32(o + FLAGS) | SS_FLAGS;
    cpu.write32(o + FLAGS, f);
    cpu.write32(o + MODEL_DRAW, SS_DRAW);
    cpu.write8(o + NUMBER, n as u8);
    let k = cpu.read8(o + NUMBER) as i8 as i32 as u32;
    cpu.write32(NUMBERED.wrapping_add(k << 2), o);
    cpu.write8(o + SS_STATE, 0);
    cpu.write32(o + POSITION, 0);
    cpu.write32(o + POSITION + 4, SS_START_Y);
    cpu.write32(o + POSITION + 8, SS_START_Z);
    for k in [0, 4, 8] {
        cpu.write32(o + SS_SPEED + k, 0);
    }
    for at in (SS_TURN_X_SPEED..=0x8e).step_by(2) {
        cpu.write16(o + at, 0);
    }
    for at in [SS_COUNT, 0x6e, SS_PLAN_AT, SS_WAIT, SS_LOOPS] {
        cpu.write16(o + at, 0);
    }
    for k in 0..3 {
        cpu.write8(o + SS_MOVED + k, 0);
    }
    for k in 0..3 {
        nested!(cpu, 0x0605_6294, actors::fill_bytes(cpu, o + SS_TRAIL_X + SS_TRAIL * k, 0, SS_TRAIL));
    }
    for (turn, trail) in [(TURN_X, SS_TRAIL_TURN_X), (TURN_Y, SS_TRAIL_TURN_Y), (TURN_Z, SS_TRAIL_TURN_Z)] {
        let a = cpu.read16(o + turn) as i16 as i32;
        let a = if a < 0 { a + 0xff } else { a } as u32;
        nested!(cpu, 0x0600_8164, clib::ashiftrt_r4::<8>(cpu, a));
        let a = (a as i32 >> 8) as i8 as i32 as u32;
        nested!(cpu, 0x0605_6294, actors::fill_bytes(cpu, o + trail, a, SS_TRAIL));
    }
    for at in (SS_MOVE_LEFT..=SS_DRIFT + 8).step_by(4) {
        cpu.write32(o + at, 0);
    }
    cpu.write16(o + SS_AIM, 0);
    cpu.write32(STAGE_RATE_AT, 0);
    for at in (SS_JAW..=SS_NECK_EASE).step_by(2) {
        cpu.write16(o + at, 0);
    }
    cpu.write16(o + SS_SLOWNESS, SS_START_SLOWNESS);
    cpu.write16(o + SS_PARTS, 0);
    cpu.write8(o + SS_PARTS + 2, 0);
    cpu.write16(o + 0x188, 0);
    cpu.write16(o + 0x18a, 0);
    cpu.write32(o + SS_LIFE, SS_START_LIFE);
    for (count, points, flags, table) in [(7, 0xe8, SS_PARTS, SS_HEAD_POINTS.0), (6, 0x140, SS_TAIL_PART, SS_JAW_POINTS.0)] {
        let number = cpu.read8(o + NUMBER) as u32;
        cpu.push(&[table, number]);
        nested!(cpu, 0x0605_6ea4, parts::parts_set(cpu, count, o + points, o + flags, o + SS_LIFE, table, number));
    }
    cpu.pop(16);
    for i in 0..SS_SEGMENT_COUNT {
        let sn = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, 2)) as i8;
        if sn == -1 {
            return;
        }
        let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, SS_SEGMENT_SIZE)) as i8 as i32 as u32;
        let s = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, NUMBERED_EXEC));
        if s == 0 {
            return;
        }
        cpu.write32(o + SS_SEGMENTS + 4 * i, s);
        cpu.write8(o + SS_FIRES + i, 0);
        nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, s + 0x10));
        nested!(cpu, 0x0604_6284, hits::hit_init(cpu, s + HITS_AT));
        cpu.write32(s + BEHAVIOUR, SG_BEHAVIOUR);
        let f = cpu.read32(s + FLAGS) | SS_FLAGS;
        cpu.write32(s + FLAGS, f);
        cpu.write32(s + MODEL_DRAW, SG_DRAW);
        cpu.write8(s + NUMBER, sn as u8);
        let k = cpu.read8(s + NUMBER) as i8 as i32 as u32;
        cpu.write32(NUMBERED.wrapping_add(k << 2), o);
        cpu.write32(s + SG_HEAD, o);
        for k in [0, 4, 8] {
            let v = cpu.read32(o + POSITION + k);
            cpu.write32(s + POSITION + k, v);
        }
        cpu.write16(s + 0x6e, 0);
        cpu.write16(s + 0x6c, 0);
        cpu.write16(s + SG_FLAGS, 0);
        cpu.write8(s + SG_SMOKE, 0);
        cpu.write16(s + SG_BACK, (15 * i + 14) as u16);
        cpu.write8(s + SG_INDEX, i as u8);
        for at in [SS_FIRE_AT, SG_TURN, SG_TURN_SPEED] {
            cpu.write16(s + at, 0);
        }
        let index = cpu.read8(s + SG_INDEX) as u32;
        let (table, count, life, less) = if index == 13 {
            SG_TAIL
        } else if index & 1 == 0 {
            SG_EVEN
        } else {
            SG_ODD
        };
        cpu.write32(s + SG_LIFE, life.wrapping_sub(less.wrapping_mul(i)));
        let number = cpu.read8(s + NUMBER) as u32;
        cpu.push(&[table, number]);
        nested!(cpu, 0x0605_6ea4, parts::parts_set(cpu, count, s + SG_SMOKE_POINTS, s + SG_FLAGS, s + SG_LIFE, table, number));
        cpu.pop(8);
    }
    cpu.write16(o + SS_POSE + 0x10, 3);
    cpu.write32(o + SS_POSE + 0x14, o + 0xbc);
    cpu.write32(o + SS_POSE + 0x18, o + 0xc2);
    cpu.write32(o + SS_POSE + 0x1c, SS_POSE_MODEL);
    nested!(cpu, 0x0605_65f0, models::pose_set(cpu, o + SS_POSE, SS_POSE_START));
    ss_script(cpu, o, SS_SCRIPT_START);
    nested!(cpu, 0x0605_6644, models::pose_step(cpu, o + SS_POSE));
    nested!(cpu, 0x0604_64d8, hud::boss_gauge_set(cpu, SS_START_LIFE));
}

/// The serpent made (its number, or 0 when there is no room).
const SS_SIZE: u32 = 0x708;

pub fn ss_make(cpu: &mut Cpu) -> u32 {
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, 2)) as i8;
    if n == -1 {
        return 0;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, SS_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, NUMBERED_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0608_f9d0, ss_set(cpu, o, n as i32 as u32));
    n as i32 as u32
}

// ---- the thruster glows ---------------------------------------------------------------------------------

/// A thruster glow (a plain actor drawn as sprites, below the ranks' third): its owner (+0x6c),
/// which glows (a word, +0x8a). It rises 3 1/4 dots a frame and ends past 176 dots down or with
/// its owner.
const TG_OWNER: u32 = 0x6c;
const TG_GLOWS: u32 = 0x8a;
const TG_RISE: u32 = 0x3_4000;
const TG_GONE: i32 = 0xaf_ffff;

pub fn thruster_behaviour(cpu: &mut Cpu, o: u32) {
    let y = cpu.read32(o + POSITION + 4).wrapping_add(TG_RISE);
    cpu.write32(o + POSITION + 4, y);
    let owner = cpu.read32(o + TG_OWNER);
    if y as i32 > TG_GONE || cpu.read8(owner + OBJ_MODE) as i8 > ENDING {
        cpu.write8(o + OBJ_MODE, TO_STOP);
    }
}

/// Its draw (`s` its sprite part): sprites of kind 0 (colours 0x90 on, nearer by 12, flickering
/// on alternate loops) at its height, 60 dots to either side: glows 0x100 both sides (frames 0x9c
/// to 0x9e, the right flipped), 0x200 the same the other way round, 0x300 frames 0x9f to 0xa2 on
/// the right (flipped), 0x400 on the left, 0x500 frames 0x9c to 0x9e on the right.
const TG_SIDE: u32 = 0x3c_0000;
const TG_FLIP: u32 = 0x10;
const TG_COLOURS: u16 = 0x90;
const TG_LIFT: u8 = 0xf4;

pub fn thruster_draw(cpu: &mut Cpu, s: u32) {
    let o = s - 0x10;
    let t = cpu.stack();
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, t));
    if cpu.read32(LOOPS) & 1 != 0 {
        cpu.write32(t, 0x100);
    }
    cpu.write32(t + 4, 0);
    let y = cpu.read32(o + POSITION + 4);
    cpu.write32(t + 8, y);
    cpu.write32(t + 12, 0);
    let c = cpu.read16(CHAR_BASE_0).wrapping_add(TG_COLOURS);
    cpu.write16(t + 0x18, c);
    cpu.write8(t + 0x23, TG_LIFT);
    cpu.write8(t + 0x1e, 0);
    let draws = |cpu: &mut Cpu, x: u32, flip: Option<bool>, frames: &[u8]| {
        cpu.write32(t + 4, x);
        if let Some(flip) = flip {
            let f = cpu.read32(t);
            cpu.write32(t, if flip { f | TG_FLIP } else { f & !TG_FLIP });
        }
        for &frame in frames {
            cpu.write8(t + 0x1f, frame);
            nested!(cpu, 0x0604_0960, sprites::draw(cpu, t));
        }
    };
    const JET: [u8; 3] = [0x9c, 0x9d, 0x9e];
    const BLAZE: [u8; 4] = [0x9f, 0xa0, 0xa1, 0xa2];
    match cpu.read16(o + TG_GLOWS) {
        0x100 => {
            draws(cpu, TG_SIDE, Some(true), &JET);
            draws(cpu, TG_SIDE.wrapping_neg(), Some(false), &JET);
        }
        0x200 => {
            draws(cpu, TG_SIDE.wrapping_neg(), Some(true), &JET);
            draws(cpu, TG_SIDE, Some(false), &JET);
        }
        0x300 => draws(cpu, TG_SIDE, Some(true), &BLAZE),
        0x400 => draws(cpu, TG_SIDE.wrapping_neg(), None, &BLAZE),
        0x500 => draws(cpu, TG_SIDE, Some(false), &JET),
        _ => {}
    }
}

/// A thruster glow made for `owner` (`glows` which), 176 dots up, below rank 3.
const TG_SIZE: u32 = 0xbc;
const TG_BEHAVIOUR: u32 = 0x0608_fe84;
const TG_DRAW: u32 = 0x0608_febc;
const TG_FLAGS: u32 = 0xc8_0000;
const TG_START_Y: u32 = 0xff50_0000;

pub fn thruster_set(cpu: &mut Cpu, owner: u32, glows: u32) {
    if cpu.read8(RANK) as i8 > 2 {
        return;
    }
    // its return address pushed where the SH-2 code's is: the carrier's lunging shots read the
    // stale slot as their heading
    let blocks = cpu.call_returning(0x0609_0070, 0x0600_57f8, &[TG_SIZE]) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 3, blocks, PLAIN_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + 0x10));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HITS_AT));
    cpu.write32(o + BEHAVIOUR, TG_BEHAVIOUR);
    cpu.write32(o + MODEL_DRAW, TG_DRAW);
    cpu.write32(o + FLAGS, TG_FLAGS);
    cpu.write32(o + POSITION + 4, TG_START_Y);
    cpu.write32(o + TG_OWNER, owner);
    cpu.write16(o + TG_GLOWS, glows as u16);
}

// ---- the escort fighters ------------------------------------------------------------------------------

/// An escort fighter (a numbered enemy drawn as a model): its carrier (+0x6c), its part flags (a
/// word, +0x70: bit 0 hit), its speed (x, y: +0x74), its side (a byte, +0x7c: 1 the right, -1
/// the left, 0 neither).
const EF_CARRIER: u32 = 0x6c;
const EF_HIT: u32 = 0x70;
const EF_SPEED: u32 = 0x74;
const EF_SIDE: u32 = 0x7c;
/// Shot down: a burst (growth 0x600, acceleration 64 changing by -4, kind 1) and a glow
/// (1 1/4 size) at its place.
const EF_BURST_GROWTH: u16 = 0x600;
const EF_BURST_ACCEL: u16 = 64;
const EF_BURST_ACCEL_CHANGE: u16 = 0xfffc;
const EF_GLOW_SIZE: u32 = 0x1_4000;
const BD_GROWTH: u32 = 0x2c;
const BD_ACCEL: u32 = 0x2e;
const BD_SPEED: u32 = 0x30;
const BD_ACCEL_CHANGE: u32 = 0x38;
const BD_KIND: u32 = 0x3a;
/// Its moves: across toward its side's edge (1 dot a frame, eased by 1/4), back once past 144
/// dots out; up or down to stay 64 dots above the nearer player (1 1/4 dots a frame, eased by
/// 1/16); not past 88 dots from the top. It turns an eighth of the way to the player a frame,
/// marks its carrier with its side (bit 14 the right, 15 the left) and passes its hits on to its
/// flags. Half the time (by its side, the frame in 120), every 15 frames while its carrier
/// attacks (steps 16 and 20) and the player is far enough (80 dots less 8 a rank), the rank's
/// number of shots at random along its heading (spread by the rank, in 2048ths).
const EF_EDGE: i32 = 0x90_0000;
const EF_ABOVE: u32 = 0x40_0000;
const EF_TOP: i32 = -0x58_0000;
const EF_ATTACKS: [u8; 2] = [16, 20];
/// The carrier's step (a byte) and its escorts' marks (a word).
const CARRIER_STEP: u32 = 0x6c;
const CARRIER_ESCORTS: u32 = 0x6e;

pub fn escort_behaviour(cpu: &mut Cpu, o: u32) {
    let t = cpu.stack();
    if cpu.read32(o + LIFE) as i32 <= 0 {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        for k in [0, 4, 8] {
            let v = cpu.read32(o + POSITION + k);
            cpu.write32(t + k, v);
        }
        let n = cpu.read8(o + NUMBER);
        cpu.write8(t + S_NUMBER, n);
        cpu.write16(t + BD_GROWTH, EF_BURST_GROWTH);
        cpu.write16(t + BD_ACCEL, EF_BURST_ACCEL);
        cpu.write16(t + BD_ACCEL_CHANGE, EF_BURST_ACCEL_CHANGE);
        cpu.write16(t + BD_KIND, 1);
        cpu.write32(t + BD_SPEED, 0);
        cpu.write32(t + BD_SPEED + 4, 0);
        nested!(cpu, 0x0605_5310, bullets::burst_set(cpu, t));
        cpu.write32(t + BD_GROWTH, EF_GLOW_SIZE);
        nested!(cpu, 0x0605_8f34, explosions::glow_set(cpu, t));
        return;
    }
    let side = cpu.read8(o + EF_SIDE) as i8 as i32 as u32;
    let x = cpu.read32(o + POSITION) as i32;
    let to = if x.abs() > EF_EDGE { (side << 16).wrapping_neg() } else { side << 16 };
    let v = cpu.read32(o + EF_SPEED);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, to, 0x4000));
    cpu.write32(o + EF_SPEED, v);
    let p = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + 0x10));
    let above = cpu.read32(p + POSITION + 4).wrapping_sub(EF_ABOVE) as i32;
    let to = if cpu.read32(o + POSITION + 4) as i32 > above { 0xfffe_c000 } else { 0x1_4000 };
    let v = cpu.read32(o + EF_SPEED + 4);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, to, 0x1000));
    cpu.write32(o + EF_SPEED + 4, v);
    for k in [0, 4] {
        let q = cpu.read32(o + POSITION + k).wrapping_add(cpu.read32(o + EF_SPEED + k));
        cpu.write32(o + POSITION + k, q);
    }
    let carrier = cpu.read32(o + EF_CARRIER);
    let mark = if (cpu.read8(o + EF_SIDE) as i8) > 0 { 0x4000 } else { 0x8000 };
    let f = cpu.read16(carrier + CARRIER_ESCORTS) | mark;
    cpu.write16(carrier + CARRIER_ESCORTS, f);
    if cpu.read16(o + HIT_FLAGS) & 0x200 != 0 {
        let f = cpu.read16(o + EF_HIT) | 1;
        cpu.write16(o + EF_HIT, f);
    }
    let q = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + 0x10));
    let a = nested!(cpu, 0x0604_5e30, aim::angle_to(cpu, o, q)) as i16 as i32 as u32;
    let from = cpu.read16(o + TURN_Z) as i16 as i32 as u32;
    let d = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, from, a)) as i32;
    let d = if d < 0 { d + 7 } else { d } >> 3;
    let z = (d as u16).wrapping_add(cpu.read16(o + TURN_Z));
    cpu.write16(o + TURN_Z, z);
    let side = cpu.read8(o + EF_SIDE) as i8;
    let firing = if side > 0 {
        let f = cpu.read32(GAME_FRAMES);
        nested!(cpu, 0x0600_6a6e, math::remainder(cpu, f, 120)) as i32 > 59
    } else if side < 0 {
        let f = cpu.read32(GAME_FRAMES).wrapping_add(60);
        nested!(cpu, 0x0600_6a6e, math::remainder(cpu, f, 120)) as i32 > 59
    } else {
        false
    };
    if firing {
        let f = cpu.read32(GAME_FRAMES);
        let m = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, f, 15));
        if m == 0 && EF_ATTACKS.contains(&cpu.read8(carrier + CARRIER_STEP)) {
            let dx = cpu.read32(p + POSITION).wrapping_sub(cpu.read32(o + POSITION));
            let dy = cpu.read32(p + POSITION + 4).wrapping_sub(cpu.read32(o + POSITION + 4));
            let far = nested!(cpu, 0x0604_5e60, aim::length(cpu, dx, dy)) as i32;
            let rank = cpu.read8(RANK) as i8 as i32;
            if far > (80 - 8 * rank) << 16 {
                let n = cpu.read8(o + NUMBER);
                cpu.write8(t + S_NUMBER, n);
                let x = cpu.read32(o + POSITION);
                cpu.write32(t + S_POSITION, x);
                let y = cpu.read32(o + POSITION + 4);
                cpu.write32(t + S_POSITION + 4, y);
                cpu.write32(t + S_POSITION + 8, m);
                cpu.write16(t + S_FLAGS, 0);
                let mut left = cpu.read8(RANK) as i8;
                while left > 0 {
                    let rank = cpu.read8(RANK) as i8 as i32 as u32;
                    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, rank.wrapping_mul(0x600)));
                    let a = r.wrapping_add(cpu.read16(o + TURN_Z) as i16 as u32).wrapping_add(0x400) & 0xffff_f800;
                    cpu.write16(t + S_ANGLE, a as u16);
                    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
                    cpu.write32(t + S_SPEED, (r & 0x7fff).wrapping_add(0x1_2000));
                    nested!(cpu, 0x0605_3db0, bullets::bullet_set(cpu, t));
                    left -= 1;
                }
            }
        }
    }
    if (cpu.read32(o + POSITION + 4) as i32) < EF_TOP {
        cpu.write32(o + POSITION + 4, EF_TOP as u32);
        cpu.write32(o + EF_SPEED + 4, 0x1_0000);
    }
}

/// Its draw (`s` its sprite part): kind 56's model 16 (its hit one when hit) at its place,
/// turned half round from its heading and banked by its height.
const EF_MODEL_KIND: u32 = 56;

pub fn escort_draw(cpu: &mut Cpu, s: u32) {
    let o = s - 0x10;
    hit_flags_drawn(cpu, o + EF_HIT);
    cpu.call(SL_PUSH_MATRIX, &[]);
    let p = [0, 4, 8].map(|k| cpu.read32(o + POSITION + k));
    cpu.call(SL_TRANSLATE, &p);
    let a = cpu.read16(o + TURN_Z).wrapping_add(0x8000) as i16 as i32 as u32;
    cpu.call(SL_ROT_Z, &[a]);
    let y = cpu.read32(o + POSITION + 4) as i32;
    let y = if y < 0 { y + 127 } else { y } as u32;
    nested!(cpu, 0x0600_8166, clib::ashiftrt_r4::<7>(cpu, y));
    let bank = (y as i32 >> 7) as i16 as i32 as u32;
    cpu.call(SL_ROT_Y, &[bank]);
    let f = cpu.read16(o + EF_HIT);
    put_kind_model(cpu, EF_MODEL_KIND, 16, Some(f));
    cpu.call(SL_POP_MATRIX, &[]);
}

/// An escort fighter made from the description `d` (its place; its carrier +0x3c, its side a
/// byte +0x2d): numbered, life 10000, side 1, hit by weapon 4, its own draw; its count at random.
const EF_SIZE: u32 = 0x80;
const EF_BEHAVIOUR: u32 = 0x0609_00e4;
const EF_DRAW: u32 = 0x0609_03ac;
const EF_FLAGS: u32 = 0xf0_0400;
const EF_KIND: u8 = 56;
const EF_HITS: u32 = 0x060a_49b0;
const EF_LIFE: u32 = 10000;
const EF_COUNT: u32 = 0x7d;

pub fn escort_set(cpu: &mut Cpu, d: u32) {
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, 2)) as i8;
    if n == -1 {
        return;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, EF_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, NUMBERED_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + 0x10));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HITS_AT));
    cpu.write32(o + BEHAVIOUR, EF_BEHAVIOUR);
    cpu.write8(o + NUMBER, n as u8);
    let k = cpu.read8(o + NUMBER) as i8 as i32 as u32;
    cpu.write32(NUMBERED.wrapping_add(k << 2), o);
    cpu.write32(o + MODEL_DRAW, EF_DRAW);
    cpu.write32(o + FLAGS, EF_FLAGS);
    cpu.write8(o + SPRITE_KIND, EF_KIND);
    cpu.write32(o + HITS_AT, EF_HITS);
    cpu.write8(o + SIDE, 1);
    cpu.write8(o + HIT_WEAPON, 4);
    cpu.write32(o + LIFE, EF_LIFE);
    let (x, y) = (cpu.read32(d), cpu.read32(d + 4));
    cpu.write32(o + POSITION, x);
    cpu.write32(o + POSITION + 4, y);
    cpu.write32(o + POSITION + 8, 0);
    let carrier = cpu.read32(d + 0x3c);
    cpu.write32(o + EF_CARRIER, carrier);
    cpu.write32(o + EF_SPEED, 0);
    cpu.write32(o + EF_SPEED + 4, 0);
    let side = cpu.read8(d + 0x2d);
    cpu.write8(o + EF_SIDE, side);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write8(o + EF_COUNT, r as u8);
}

// ---- the spinning pods ------------------------------------------------------------------------------------

/// A spinning pod (an enemy actor drawn as two sprites on a tumbling axis): its hits for drawing
/// (a word, +0x7e), its speed (x, y: +0x80), frames it slows (a byte, +0x88) by (words: +0x92,
/// +0x94), its heading (a word, +0x8c), its spin (a word, +0xa6), which frames it is hidden on
/// (a word, +0xa8: odd or even).
const PO_HITS: u32 = 0x7e;
const PO_SPEED: u32 = 0x80;
const PO_SLOWING: u32 = 0x88;
const PO_HEADING: u32 = 0x8c;
const PO_SLOW_BY: u32 = 0x92;
const PO_SPIN: u32 = 0xa6;
const PO_HIDDEN_ON: u32 = 0xa8;
const PO_HIDE: u32 = 0x8_0000;
/// Shot down: smoke (a drift effect, the script at 0x0607305c, colours 0x90 on) and a rocket
/// at the nearer player (spread 3/64 of a turn at random, kind 7 or 9).
const PO_SMOKE_SCRIPT: u32 = 0x0607_305c;

pub fn spinner_pod_behaviour(cpu: &mut Cpu, o: u32) {
    let t = cpu.stack();
    if nested!(cpu, 0x0605_6a24, aim::off_screen(cpu, o + 0x10)) as u8 != 0 || cpu.read32(o + LIFE) as i32 <= 0 {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        if cpu.read32(o + LIFE) as i32 <= 0 {
            let x = cpu.read32(o + POSITION);
            cpu.write32(t + actors::E_POSITION, x);
            let y = cpu.read32(o + POSITION + 4);
            cpu.write32(t + actors::E_POSITION + 4, y);
            cpu.write32(t + actors::E_POSITION + 8, 0);
            cpu.write32(t + actors::E_SCRIPT, PO_SMOKE_SCRIPT);
            cpu.write16(t + actors::E_ANGLE, 0);
            cpu.write32(t + actors::E_SPEED, 0);
            cpu.write32(t + actors::E_SCALES, 0x1_0000);
            cpu.write32(t + actors::E_SCALES + 4, 0x1_0000);
            let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
            let kind = ((r & 1) + 64) as u16;
            cpu.write16(t + 48, kind);
            cpu.write16(t + actors::E_COLOUR, if kind == 0 { 0x70 } else { 0x90 });
            nested!(cpu, 0x0605_7454, actors::drift_effect(cpu, t));
            let p = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + 0x10));
            let n = cpu.read8(o + NUMBER);
            cpu.write8(t + S_NUMBER, n);
            let a = nested!(cpu, 0x0604_5e30, aim::angle_to(cpu, o, p));
            let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0xc00));
            cpu.write16(t + S_ANGLE, r.wrapping_add(a) as u16);
            let k = (cpu.read16(t + 48) & 1) * 2 + 7;
            cpu.write16(t + S_FLAGS, k);
            nested!(cpu, 0x0605_5b5c, bullets::rocket_set(cpu, t));
        }
    } else {
        let n = cpu.read8(o + PO_SLOWING);
        if n != 0 {
            cpu.write8(o + PO_SLOWING, n.wrapping_sub(1));
            for k in [0, 4] {
                let v = cpu.read32(o + PO_SPEED + k).wrapping_add(cpu.read16(o + PO_SLOW_BY + k / 2) as i16 as u32);
                cpu.write32(o + PO_SPEED + k, v);
            }
        }
    }
    for k in [0, 4] {
        let p = cpu.read32(o + POSITION + k).wrapping_add(cpu.read32(o + PO_SPEED + k));
        cpu.write32(o + POSITION + k, p);
    }
    let f = cpu.read32(o + FLAGS);
    let f = if cpu.read32(GAME_FRAMES) & 1 == cpu.read16(o + PO_HIDDEN_ON) as u32 { f | PO_HIDE } else { f & !PO_HIDE };
    cpu.write32(o + FLAGS, f);
    if cpu.read16(o + HIT_FLAGS) & 0x200 != 0 {
        let h = cpu.read16(o + PO_HITS) | 1;
        cpu.write16(o + PO_HITS, h);
    }
}

/// Its draw (`s` its sprite part): kind 0's frame 98 at both ends of an axis 3 dots each way,
/// tumbling by its spin, 176 dots nearer: colours 0x50 on and 0x70 on (both 0xe0 on while hit).
pub fn spinner_pod_draw(cpu: &mut Cpu, s: u32) {
    let o = s - 0x10;
    hit_flags_drawn(cpu, o + PO_HITS);
    let t = cpu.stack();
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, t));
    cpu.write32(t, 0x2000_0000);
    cpu.write8(t + 0x1f, 98);
    cpu.write8(t + 0x1e, 0);
    cpu.write8(t + 0x23, 8);
    for k in [0, 4, 8] {
        let v = cpu.read32(o + POSITION + k);
        cpu.write32(t + 4 + k, v);
    }
    cpu.write32(t + 0x10, 0xa000);
    cpu.write32(t + 0x14, 0xa000);
    let spin = cpu.read16(o + PO_SPIN).wrapping_add(1);
    cpu.write16(o + PO_SPIN, spin);
    cpu.call(SL_PUSH_MATRIX, &[]);
    let p = [0, 4, 8].map(|k| cpu.read32(o + POSITION + k));
    cpu.call(SL_TRANSLATE, &p);
    cpu.call(SL_PUSH_MATRIX, &[]);
    let spin = cpu.read16(o + PO_SPIN);
    cpu.call(SL_ROT_X, &[(spin << 11) as i16 as i32 as u32]);
    let spin = cpu.read16(o + PO_SPIN) as i16;
    cpu.call(SL_ROT_Y, &[(spin as i32 * 0x300) as i16 as i32 as u32]);
    let spin = cpu.read16(o + PO_SPIN);
    cpu.call(SL_ROT_Z, &[(spin << 9) as i16 as i32 as u32]);
    cpu.call(SL_TRANSLATE, &[0, 0, 0]);
    cpu.call(SL_CALC_POINT, &[0x3_0000, 0, 0, t + 44]);
    cpu.call(SL_CALC_POINT, &[0xfffd_0000, 0, 0, t + 56]);
    cpu.call(SL_POP_MATRIX, &[]);
    cpu.call(SL_POP_MATRIX, &[]);
    for (at, colours) in [(44, 0x50), (56, 0x70)] {
        let c = if cpu.read16(o + PO_HITS) & 1 != 0 { 0xe0 } else { colours };
        let c = cpu.read16(CHAR_BASE_0).wrapping_add(c);
        cpu.write16(t + 0x18, c);
        let (x, y, z) = (cpu.read32(t + at), cpu.read32(t + at + 4), cpu.read32(t + at + 8));
        cpu.write32(t + 4, x);
        cpu.write32(t + 8, y);
        cpu.write32(t + 12, z.wrapping_add(0xff50_0000));
        nested!(cpu, 0x0604_0960, sprites::draw(cpu, t));
    }
}

/// A spinning pod thrown from the description `d` (its place, its number +0x1e, its heading a
/// word +0x18, which frames it hides on a word +0x2c; sound 78): life 200, side 1, hit by weapon
/// 4; 3 dots a frame along its heading (an eighth of a turn more at random), slowing to a sixteenth
/// of that over 28 frames; its spin at random.
const PO_SIZE: u32 = 0xbc;
const PO_BEHAVIOUR: u32 = 0x0609_0564;
const PO_DRAW: u32 = 0x0609_06f4;
const PO_FLAGS: u32 = 0xf0_0400;
const PO_LIFE: u32 = 200;
const PO_SLOWS_FOR: u8 = 28;

pub fn spinner_pod_set(cpu: &mut Cpu, d: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, PO_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + 0x10));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HITS_AT));
    cpu.write32(o + BEHAVIOUR, PO_BEHAVIOUR);
    let n = cpu.read8(d + S_NUMBER);
    cpu.write8(o + NUMBER, n);
    cpu.write32(o + MODEL_DRAW, PO_DRAW);
    cpu.write32(o + FLAGS, PO_FLAGS);
    cpu.write8(o + SPRITE_KIND, 0);
    cpu.write32(o + HITS_AT, EF_HITS);
    cpu.write8(o + SIDE, 1);
    cpu.write8(o + HIT_WEAPON, 4);
    cpu.write32(o + LIFE, PO_LIFE);
    let (x, y) = (cpu.read32(d), cpu.read32(d + 4));
    cpu.write32(o + POSITION, x);
    cpu.write32(o + POSITION + 4, y);
    cpu.write32(o + POSITION + 8, 0);
    cpu.write8(o + PO_SLOWING, PO_SLOWS_FOR);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x2000));
    let a = (r as u16).wrapping_add(cpu.read16(d + 0x18));
    cpu.write16(o + PO_HEADING, a);
    let a = cpu.read16(o + PO_HEADING) as i16 as i32 as u32;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    cpu.write32(o + PO_SPEED, s.wrapping_mul(3));
    let a = cpu.read16(o + PO_HEADING) as i16 as i32 as u32;
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    cpu.write32(o + PO_SPEED + 4, c.wrapping_mul(3).wrapping_neg());
    for k in [0, 4] {
        let v = cpu.read32(o + PO_SPEED + k) as i32;
        let v = if v < 0 { v + 15 } else { v } >> 4;
        cpu.write16(o + PO_SLOW_BY + k / 2, (v as u16).wrapping_neg());
    }
    let h = cpu.read16(d + 0x2c);
    cpu.write16(o + PO_HIDDEN_ON, h);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write16(o + PO_SPIN, r as u16);
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 78));
}

// ---- the carrier's beams --------------------------------------------------------------------------------

/// A carrier's beam (an enemy actor drawn by its own code): its carrier (+0x6c), its stage (a
/// byte, +0x7c), which gun (a byte, +0x88: 0 the first, else the second), its hit strength (a
/// word, +0x58). It ends with its gun's part (bit 6 of the carrier's +0x96 or +0x98): it widens
/// to 3/8 (strength 10, then back to 1/4); while the carrier holds (its byte +0x3f5 at 30) it
/// stays, else it narrows away by 1/32 a frame. It hangs 96 dots below its gun, flickering.
const CB_CARRIER: u32 = 0x6c;
const CB_STAGE: u32 = 0x7c;
const CB_GUN: u32 = 0x88;
const CB_STRENGTH: u32 = 0x58;
const CB_HITS_A: u32 = 0x48;
const CB_GUN_PARTS: [u32; 2] = [0x96, 0x98];
const CB_GUN_PLACES: [u32; 2] = [0x4a8, 0x4b4];
const CB_HOLD: u32 = 0x3f5;
const CB_HOLDING: u8 = 30;
const CB_WIDE: u32 = 0x6000;
const CB_BELOW: u32 = 0x60_0000;
const CB_FLICKER: u32 = 0x8_0000;

pub fn carrier_beam_behaviour(cpu: &mut Cpu, o: u32) {
    let t = cpu.stack();
    let carrier = cpu.read32(o + CB_CARRIER);
    let gun = (cpu.read8(o + CB_GUN) != 0) as usize;
    if cpu.read16(carrier + CB_GUN_PARTS[gun]) & 0x40 != 0 {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    if cpu.read8(o + CB_STAGE) != 0 {
        let w = cpu.read32(o + SCALES).wrapping_sub(0x800);
        cpu.write32(o + SCALES, w);
        if w as i32 <= 0 {
            cpu.write8(o + OBJ_MODE, TO_STOP);
        }
    } else {
        let w = cpu.read32(o + SCALES);
        let w = nested!(cpu, 0x0604_6168, actors::approach(cpu, w, CB_WIDE, 0x400));
        cpu.write32(o + SCALES, w);
        if w == CB_WIDE {
            cpu.write16(o + CB_STRENGTH, 10);
            cpu.write32(o + SCALES, 0x4000);
        }
        cpu.write32(o + CB_HITS_A, 0xffff_ffff);
        cpu.write32(o + CB_HITS_A + 4, 0xffff_ffff);
        if cpu.read8(carrier + CB_HOLD) != CB_HOLDING {
            let s = cpu.read8(o + CB_STAGE).wrapping_add(1);
            cpu.write8(o + CB_STAGE, s);
            cpu.write16(o + CB_STRENGTH, 0);
        }
    }
    let gun = (cpu.read8(o + CB_GUN) != 0) as usize;
    let x = cpu.read32(carrier + CB_GUN_PLACES[gun]);
    cpu.write32(o + POSITION, x);
    // the SH-2 code leaves the gun's height in its frame
    let h = cpu.read32(carrier + CB_GUN_PLACES[gun] + 4);
    cpu.write32(t + 4, h);
    let y = cpu.read32(carrier + POSITION + 4).wrapping_add(CB_BELOW);
    cpu.write32(o + POSITION + 4, y);
    let f = cpu.read32(o + FLAGS) ^ CB_FLICKER;
    cpu.write32(o + FLAGS, f);
    let c = cpu.read16(CHAR_BASE_0);
    let c = if cpu.read32(GAME_FRAMES) & 1 != 0 { c } else { c.wrapping_add(64) };
    cpu.write16(o + OWN_COLOUR, c);
    let c = cpu.read16(o + OWN_COLOUR);
    cpu.write16(o + COLOUR, c);
}

/// Its draw (`s` its sprite part): its own sprite (as an attribute while it does not hit), then
/// three sprites of kind 0 (frames 77 to 88 at random, turned a quarter either way at random,
/// colours 64 on) going up the screen 38 dots apart from 40 to 50 dots above it at random,
/// 4.5 times as high as it is wide.
pub fn carrier_beam_draw(cpu: &mut Cpu, s: u32) {
    let o = s - 0x10;
    let t = cpu.stack();
    let flags = cpu.read32(o + FLAGS);
    cpu.write32(t + 44, flags);
    let f = if cpu.read16(o + CB_STRENGTH) != 0 { 0 } else { 0x100 };
    cpu.write32(o + FLAGS, f);
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, s));
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, t));
    cpu.write32(t, 0x100);
    let x = cpu.read32(o + POSITION);
    cpu.write32(t + 4, x);
    cpu.write32(t + 12, 0);
    cpu.write8(t + 0x1e, 0);
    let c = cpu.read16(CHAR_BASE_0).wrapping_add(64);
    cpu.write16(t + 0x18, c);
    cpu.write8(t + 0x23, 0xf8);
    let w = cpu.read32(o + SCALES);
    let h = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, w, 0x4_8000));
    cpu.write32(t + 0x14, h);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0xa_0000));
    let base = cpu.read32(o + POSITION + 4).wrapping_sub(r.wrapping_add(0x28_0000));
    for i in 0..3u32 {
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
        let k = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, r, 12));
        cpu.write8(t + 0x1f, (k as u8).wrapping_add(77));
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
        cpu.write16(t + 0x20, ((r & 0x8000) + 0x4000) as u16);
        cpu.write32(t + 8, (i * 0x26_0000).wrapping_add(base));
        nested!(cpu, 0x0604_0960, sprites::draw(cpu, t));
    }
    let flags = cpu.read32(t + 44);
    cpu.write32(o + FLAGS, flags);
}

/// A beam made for the carrier `carrier`'s gun `gun` (sound 34): its number, frame 0xaf of kind
/// 0, colours as the first character's, side 1, hit by weapon 5; no width yet, 5 1/4 high;
/// flickering opposite for the second gun.
const CB_SIZE: u32 = 0xbc;
const CB_BEHAVIOUR: u32 = 0x0609_0a2c;
const CB_DRAW: u32 = 0x0609_0b6c;
const CB_FLAGS: u32 = 0xc0_0400;
const CB_HITS: u32 = 0x060a_49c0;

pub fn carrier_beam_set(cpu: &mut Cpu, carrier: u32, gun: u32) {
    let gun = gun as u8;
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, CB_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + 0x10));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HITS_AT));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    let n = cpu.read8(carrier + NUMBER);
    cpu.write8(o + NUMBER, n);
    cpu.write32(o + BEHAVIOUR, CB_BEHAVIOUR);
    cpu.write32(o + FLAGS, CB_FLAGS);
    cpu.write32(o + MODEL_DRAW, CB_DRAW);
    cpu.write32(o + CB_CARRIER, carrier);
    for k in [0, 4, 8] {
        cpu.write32(o + POSITION + k, 0);
    }
    cpu.write8(o + BYTE_33, 0xf8);
    cpu.write8(o + SPRITE_FRAME, 0xaf);
    cpu.write8(o + SPRITE_KIND, 0);
    let c = cpu.read16(CHAR_BASE_0);
    cpu.write16(o + OWN_COLOUR, c);
    cpu.write16(o + COLOUR, c);
    cpu.write32(o + HITS_AT, CB_HITS);
    cpu.write8(o + SIDE, 1);
    cpu.write8(o + HIT_WEAPON, 5);
    cpu.write16(o + CB_STRENGTH, 0);
    cpu.write8(o + CB_GUN, gun);
    cpu.write32(o + SCALES, 0);
    cpu.write32(o + SCALES + 4, 0x5_4000);
    if gun != 0 {
        let f = cpu.read32(o + FLAGS) | CB_FLICKER;
        cpu.write32(o + FLAGS, f);
    }
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 34));
}

// ---- the spark shots and the flares --------------------------------------------------------------------

/// A spark shot (an enemy actor run as a bullet): its speed (x, y: +0x80), frames it speeds up
/// (a word, +0x8c) by (words: +0x8e, +0x90), its count (a byte, +0x88). It speeds up for 17
/// frames by a seventh of its direction each, grows to full size, its frame (91 to 93) by its
/// count.
const SS2_SPEED: u32 = 0x80;
const SS2_SPEEDING: u32 = 0x8c;
const SS2_SPEED_UP: u32 = 0x8e;
const SS2_COUNT: u32 = 0x88;

pub fn spark_shot_behaviour(cpu: &mut Cpu, o: u32) {
    if nested!(cpu, 0x0605_3b10, bullets::bullet_ends(cpu, o, 1)) as u8 != 0 {
        return;
    }
    let n = cpu.read16(o + SS2_SPEEDING);
    if n as i16 > 0 {
        cpu.write16(o + SS2_SPEEDING, n.wrapping_sub(1));
        for k in [0, 4] {
            let v = cpu.read32(o + SS2_SPEED + k).wrapping_add(cpu.read16(o + SS2_SPEED_UP + k / 2) as i16 as u32);
            cpu.write32(o + SS2_SPEED + k, v);
        }
    }
    for k in [0, 4] {
        let p = cpu.read32(o + POSITION + k).wrapping_add(cpu.read32(o + SS2_SPEED + k));
        cpu.write32(o + POSITION + k, p);
    }
    for k in [0, 4] {
        let s = cpu.read32(o + SCALES + k);
        let s = nested!(cpu, 0x0604_6168, actors::approach(cpu, s, 0x1_0000, 0x1800));
        cpu.write32(o + SCALES + k, s);
    }
    let c = cpu.read8(o + SS2_COUNT) as i8 as i32;
    let c = (if c < 0 { c + 3 } else { c } >> 2) as i8 as i32 as u32;
    let k = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, c, 3));
    cpu.write8(o + SPRITE_FRAME, (k as u8).wrapping_add(91));
    let c = cpu.read8(o + SS2_COUNT).wrapping_add(1);
    cpu.write8(o + SS2_COUNT, c);
}

/// A spark shot made from the description `d` (its place, its number +0x1e, its angle a word
/// +0x2c): frame 91 of kind 0, colours 0x90 on, side 1, hit by weapon 5; 1 1/2 wide, 1/8 high.
const SS2_SIZE: u32 = 0xbc;
const SS2_BEHAVIOUR: u32 = 0x0609_0d8c;
const SS2_FLAGS: u32 = 0x2000_0400;
const SS2_HITS: u32 = 0x060a_49c8;

pub fn spark_shot_set(cpu: &mut Cpu, d: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, SS2_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + 0x10));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HITS_AT));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    let n = cpu.read8(d + S_NUMBER);
    cpu.write8(o + NUMBER, n);
    cpu.write32(o + BEHAVIOUR, SS2_BEHAVIOUR);
    cpu.write32(o + FLAGS, SS2_FLAGS);
    let (x, y) = (cpu.read32(d), cpu.read32(d + 4));
    cpu.write32(o + POSITION, x);
    cpu.write32(o + POSITION + 4, y);
    cpu.write32(o + POSITION + 8, 0);
    cpu.write8(o + BYTE_33, 8);
    cpu.write8(o + SPRITE_FRAME, 91);
    cpu.write8(o + SPRITE_KIND, 0);
    let c = cpu.read16(CHAR_BASE_0).wrapping_add(0x90);
    cpu.write16(o + OWN_COLOUR, c);
    cpu.write16(o + COLOUR, c);
    cpu.write32(o + HITS_AT, SS2_HITS);
    cpu.write8(o + SIDE, 1);
    cpu.write8(o + HIT_WEAPON, 5);
    let a = cpu.read16(d + D_ANGLE_2C);
    cpu.write16(o + TURN_Z, a);
    cpu.write16(o + SS2_SPEEDING, 17);
    let a = cpu.read16(d + D_ANGLE_2C) as i16 as i32 as u32;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    let q = nested!(cpu, 0x0600_6a78, math::quotient(cpu, s, 7));
    cpu.write16(o + SS2_SPEED_UP, q as u16);
    let a = cpu.read16(d + D_ANGLE_2C) as i16 as i32 as u32;
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    let q = nested!(cpu, 0x0600_6a78, math::quotient(cpu, c.wrapping_neg(), 7));
    cpu.write16(o + SS2_SPEED_UP + 2, q as u16);
    cpu.write32(o + SCALES, 0x1_8000);
    cpu.write32(o + SCALES + 4, 0x2000);
}

/// A flare (an enemy actor run as a bullet, drawn by its own code): it drops (faster for 17
/// frames), drifting across, growing to 1/2 wide and 5/16 high.
pub fn flare_behaviour(cpu: &mut Cpu, o: u32) {
    if nested!(cpu, 0x0605_3b10, bullets::bullet_ends(cpu, o, 0)) as u8 != 0 {
        return;
    }
    let n = cpu.read16(o + SS2_SPEEDING);
    if n as i16 > 0 {
        cpu.write16(o + SS2_SPEEDING, n.wrapping_sub(1));
        let v = cpu.read32(o + SS2_SPEED + 4).wrapping_add(0x1a00);
        cpu.write32(o + SS2_SPEED + 4, v);
    }
    for (k, to, step) in [(0, 0x8000, 0x800), (4, 0x5000, 0x400)] {
        let s = cpu.read32(o + SCALES + k);
        let s = nested!(cpu, 0x0604_6168, actors::approach(cpu, s, to, step));
        cpu.write32(o + SCALES + k, s);
    }
    for k in [0, 4] {
        let p = cpu.read32(o + POSITION + k).wrapping_add(cpu.read32(o + SS2_SPEED + k));
        cpu.write32(o + POSITION + k, p);
    }
}

/// Its draw (`s` its sprite part): kind 0's frame 0xb4 (colours 0x10 on; 0xb3, 0x40 on, on
/// alternate loops) twice, crossed, its height as wide and its width (a little more at random)
/// as long.
pub fn flare_draw(cpu: &mut Cpu, s: u32) {
    let o = s - 0x10;
    let t = cpu.stack();
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, t));
    cpu.write32(t, 0x2000_0000);
    let (x, y) = (cpu.read32(o + POSITION), cpu.read32(o + POSITION + 4));
    cpu.write32(t + 4, x);
    cpu.write32(t + 8, y);
    cpu.write32(t + 12, 0);
    let h = cpu.read32(o + SCALES + 4);
    cpu.write32(t + 0x10, h);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x3000));
    let w = cpu.read32(o + SCALES).wrapping_add(r);
    cpu.write32(t + 0x14, w);
    cpu.write8(t + 0x1e, 0);
    let (frame, colours) = if cpu.read32(LOOPS) & 1 != 0 { (0xb3, 64) } else { (0xb4, 16) };
    cpu.write8(t + 0x1f, frame);
    let c = cpu.read16(CHAR_BASE_0).wrapping_add(colours);
    cpu.write16(t + 0x18, c);
    cpu.write8(t + 0x23, 8);
    for a in [0x3fff, 0xbfff] {
        cpu.write16(t + 0x20, a);
        nested!(cpu, 0x0604_0960, sprites::draw(cpu, t));
    }
}

/// A flare made from the description `d` (its place, its number +0x1e; sound 78): colours 64
/// on, side 1, hit by weapon 5; drifting across at random (up to 3/4 of a dot a frame).
const FR_BEHAVIOUR: u32 = 0x0609_0f8c;
const FR_DRAW: u32 = 0x0609_1010;
const FR_FLAGS: u32 = 0xc0_0400;
const FR_HITS: u32 = 0x060a_49d0;

pub fn flare_set(cpu: &mut Cpu, d: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, SS2_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + 0x10));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HITS_AT));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    let n = cpu.read8(d + S_NUMBER);
    cpu.write8(o + NUMBER, n);
    cpu.write32(o + BEHAVIOUR, FR_BEHAVIOUR);
    cpu.write32(o + FLAGS, FR_FLAGS);
    cpu.write32(o + MODEL_DRAW, FR_DRAW);
    let c = cpu.read16(CHAR_BASE_0).wrapping_add(64);
    cpu.write16(o + COLOUR, c);
    let (x, y) = (cpu.read32(d), cpu.read32(d + 4));
    cpu.write32(o + POSITION, x);
    cpu.write32(o + POSITION + 4, y);
    cpu.write32(o + POSITION + 8, 0);
    cpu.write8(o + BYTE_33, 8);
    cpu.write32(o + HITS_AT, FR_HITS);
    cpu.write8(o + SIDE, 1);
    cpu.write8(o + HIT_WEAPON, 5);
    cpu.write32(o + SCALES, 0);
    cpu.write32(o + SCALES + 4, 0x1_0000);
    cpu.write16(o + SS2_SPEEDING, 17);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0xc000));
    cpu.write32(o + SS2_SPEED, r);
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 78));
}

// ---- the big charges ---------------------------------------------------------------------------------------

/// A big charge's trail (an orbiter, like the charge's sparks): its height falls by an eighth a
/// frame; it ends once that is no more than an eighth of its start. Whether it ends.
pub fn big_charge_trail_fade(cpu: &mut Cpu, o: u32) -> u32 {
    let h = cpu.read32(o + SCALES + 4) as i32;
    let h = h - (if h < 0 { h + 7 } else { h } >> 3);
    cpu.write32(o + SCALES + 4, h as u32);
    ((if h < 0 { h + 15 } else { h } >> 4) <= 0x800) as u32
}

/// A trail's behaviour: placed on its orbit (ending with it) and faded; counted, 176 dots nearer
/// than its centre; lifted 18.
pub fn big_charge_trail_behaviour(cpu: &mut Cpu, o: u32) {
    if nested!(cpu, 0x0605_9418, orbits::orbit_place(cpu, o)) as u8 != 0
        || nested!(cpu, 0x0609_11d4, big_charge_trail_fade(cpu, o)) as u8 != 0
    {
        cpu.write8(o + OBJ_MODE, TO_STOP);
    } else {
        let n = cpu.read8(o + CS_COUNT).wrapping_add(1);
        cpu.write8(o + CS_COUNT, n);
        let z = cpu.read32(o + POSITION + 8).wrapping_add(CS_NEARER);
        cpu.write32(o + POSITION + 8, z);
    }
    cpu.write8(o + BYTE_33, 18);
}

/// A trail left by the big charge `p`: the same orbit at a random angle and turn, at the centre;
/// frame 0 of kind 0 in the charge's colours, thin (3/64), as long as 8 less 3/16 for each of the
/// charge's frames; an attribute every other pair of frames.
const BC_TRAIL_BEHAVIOUR: u32 = 0x0609_1210;

pub fn big_charge_trail_set(cpu: &mut Cpu, p: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, CS_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 3, blocks, PLAIN_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + 0x10));
    cpu.write32(o + BEHAVIOUR, BC_TRAIL_BEHAVIOUR);
    cpu.write8(o + CS_COUNT, 0);
    for (at, long) in [(0x3c, true), (0x40, false), (0x44, true), (0x48, true)] {
        if long {
            let v = cpu.read32(p + at);
            cpu.write32(o + at, v);
        } else {
            let v = cpu.read16(p + at);
            cpu.write16(o + at, v);
        }
    }
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write16(o + 0x58, r as u16);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x200));
    cpu.write16(o + 0x5a, r as u16);
    cpu.write32(o + 0x5c, 0);
    cpu.write32(o + 0x60, 0);
    let centre = cpu.read32(o + 0x44);
    for k in [0, 4, 8] {
        let v = cpu.read32(centre + k);
        cpu.write32(o + POSITION + k, v);
    }
    cpu.write8(o + SPRITE_FRAME, 0);
    cpu.write8(o + SPRITE_KIND, 0);
    let c = cpu.read16(p + COLOUR);
    cpu.write16(o + COLOUR, c);
    cpu.write8(o + BYTE_33, 8);
    cpu.write32(o + SCALES, 0xc00);
    let n = cpu.read8(p + CS_COUNT) as u32;
    cpu.write32(o + SCALES + 4, 0x8_0000u32.wrapping_sub(n.wrapping_mul(0x3000)));
    if cpu.read8(p + CS_COUNT) & 2 != 0 {
        cpu.write32(o + FLAGS, 0x100);
    }
}

/// A big charge growing: for its first 35 frames flipped and framed (211 to 214) at random,
/// growing toward 5/8 and a little more at random by 1/32 a frame; then shrinking; it ends gone.
/// Its height its width. Whether it ends.
pub fn big_charge_grow(cpu: &mut Cpu, o: u32) -> u32 {
    if cpu.read8(o + CS_COUNT) > 35 {
        let w = cpu.read32(o + SCALES);
        let w = nested!(cpu, 0x0604_6168, actors::approach(cpu, w, 0, 0x800));
        cpu.write32(o + SCALES, w);
        if w == 0 {
            return 1;
        }
    } else {
        let f = cpu.read32(o + FLAGS) & !CG_FLIP;
        cpu.write32(o + FLAGS, f);
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
        let f = cpu.read32(o + FLAGS) | (r & CG_FLIP);
        cpu.write32(o + FLAGS, f);
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
        cpu.write8(o + SPRITE_FRAME, ((r & 3) as u8).wrapping_add(CG_FRAME));
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
        let w = cpu.read32(o + SCALES);
        let w = nested!(cpu, 0x0604_6168, actors::approach(cpu, w, (r & 0x1fff) + CG_FULL, 0x800));
        cpu.write32(o + SCALES, w);
    }
    let w = cpu.read32(o + SCALES);
    cpu.write32(o + SCALES + 4, w);
    0
}

/// A big charge's behaviour: placed on its orbit (ending with it) and grown; counted, 176 dots
/// nearer than its centre; a trail on odd frames to its 29th.
pub fn big_charge_behaviour(cpu: &mut Cpu, o: u32) {
    if nested!(cpu, 0x0605_9418, orbits::orbit_place(cpu, o)) as u8 != 0
        || nested!(cpu, 0x0609_1384, big_charge_grow(cpu, o)) as u8 != 0
    {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    let n = cpu.read8(o + CS_COUNT).wrapping_add(1);
    cpu.write8(o + CS_COUNT, n);
    let z = cpu.read32(o + POSITION + 8).wrapping_add(CS_NEARER);
    cpu.write32(o + POSITION + 8, z);
    let n = cpu.read8(o + CS_COUNT);
    if n & 1 != 0 && n <= 29 {
        nested!(cpu, 0x0609_1268, big_charge_trail_set(cpu, o));
    }
}

/// A big charge made (sound 69) at the centre `centre`, its angle from the word at `angle_from`,
/// ending once the word at `watched` has a bit of `mask`: turning 1/16 of a turn a frame, frame
/// 214 of kind 0 in colours `colours` on, no size at first.
const BC_BEHAVIOUR: u32 = 0x0609_1428;

pub fn big_charge_set(cpu: &mut Cpu, watched: u32, mask: u32, centre: u32, angle_from: u32, colours: u32) {
    let colours = colours as u8 as u16;
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, CS_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 3, blocks, PLAIN_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + 0x10));
    cpu.write32(o + BEHAVIOUR, BC_BEHAVIOUR);
    cpu.write8(o + CS_COUNT, 0);
    cpu.write32(o + 0x3c, watched);
    cpu.write16(o + 0x40, mask as u16);
    cpu.write32(o + 0x44, centre);
    cpu.write32(o + 0x48, angle_from);
    cpu.write16(o + 0x58, 0);
    cpu.write16(o + 0x5a, CG_TURN);
    cpu.write32(o + 0x5c, 0);
    cpu.write32(o + 0x60, 0);
    for k in [0, 4, 8] {
        let v = cpu.read32(centre + k);
        cpu.write32(o + POSITION + k, v);
    }
    cpu.write8(o + SPRITE_FRAME, CG_START_FRAME);
    cpu.write8(o + SPRITE_KIND, 0);
    let c = cpu.read16(CHAR_BASE_0).wrapping_add(colours);
    cpu.write16(o + COLOUR, c);
    cpu.write8(o + BYTE_33, 0);
    cpu.write32(o + SCALES, 0);
    cpu.write32(o + SCALES + 4, 0);
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 69));
}

// ---- the barrier pieces ---------------------------------------------------------------------------------

/// A barrier piece (an enemy actor, one of 4 at its owner's points): whether it is on (the byte
/// at +0x6c), its point (+0x70: x, y, z), its size (the long at +0x74).
const BR_ON_AT: u32 = 0x6c;
const BR_POINT: u32 = 0x70;
const BR_SIZE_AT: u32 = 0x74;
const BR_SMALL: i32 = 0x57ff;
const BR_HIDE: u32 = 0x8_0000;

/// A piece at its point, 176 dots nearer, its size; hidden while small.
pub fn barrier_place(cpu: &mut Cpu, o: u32) {
    let p = cpu.read32(o + BR_POINT);
    for k in [0, 4] {
        let v = cpu.read32(p + k);
        cpu.write32(o + POSITION + k, v);
    }
    let z = cpu.read32(p + 8).wrapping_add(CS_NEARER);
    cpu.write32(o + POSITION + 8, z);
    let size_at = cpu.read32(o + BR_SIZE_AT);
    let s = cpu.read32(size_at);
    cpu.write32(o + SCALES, s);
    cpu.write32(o + SCALES + 4, s);
    let f = cpu.read32(o + FLAGS);
    let f = if cpu.read32(o + SCALES) as i32 > BR_SMALL { f & !BR_HIDE } else { f | BR_HIDE };
    cpu.write32(o + FLAGS, f);
}

/// A piece's behaviour: it ends once its owner's barrier is off, else is placed.
pub fn barrier_behaviour(cpu: &mut Cpu, o: u32) {
    let on_at = cpu.read32(o + BR_ON_AT);
    if cpu.read8(on_at) == 0 {
        cpu.write8(o + OBJ_MODE, TO_STOP);
    } else {
        nested!(cpu, 0x0609_157c, barrier_place(cpu, o));
    }
}

/// A barrier's 4 pieces made at the points from `points` (12 bytes each), on while the byte at
/// `on_at` is, sized by the long at `size_at`: side 1, hit by weapon 5.
const BR_SIZE: u32 = 0x78;
const BR_BEHAVIOUR: u32 = 0x0609_15d4;
const BR_FLAGS: u32 = 0x8008_0400;
const BR_HITS: u32 = 0x060a_49d8;

pub fn barrier_set(cpu: &mut Cpu, on_at: u32, points: u32, size_at: u32) {
    for i in 0..4u32 {
        let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, BR_SIZE)) as i8 as i32 as u32;
        let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
        if o == 0 {
            return;
        }
        nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + 0x10));
        nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HITS_AT));
        cpu.write32(o + BEHAVIOUR, BR_BEHAVIOUR);
        cpu.write32(o + FLAGS, BR_FLAGS);
        cpu.write32(o + BR_ON_AT, on_at);
        cpu.write32(o + BR_POINT, points + 12 * i);
        cpu.write32(o + BR_SIZE_AT, size_at);
        cpu.write32(o + HITS_AT, BR_HITS);
        cpu.write8(o + SIDE, 1);
        cpu.write8(o + HIT_WEAPON, 5);
        nested!(cpu, 0x0609_157c, barrier_place(cpu, o));
    }
}

/// A cross flash at the point `p` (x, y, z), 176 dots nearer: colours `colours`, 4 high, at
/// `angle`.
pub fn cross_flash_at(cpu: &mut Cpu, p: u32, colours: u32, angle: u32) {
    let t = cpu.stack();
    for k in [0, 4] {
        let v = cpu.read32(p + k);
        cpu.write32(t + actors::E_POSITION + k, v);
    }
    let z = cpu.read32(p + 8).wrapping_add(CS_NEARER);
    cpu.write32(t + actors::E_POSITION + 8, z);
    cpu.write16(t + actors::E_COLOUR, colours as u8 as u16);
    cpu.write32(t + actors::E_SCALES + 4, 0x4_0000);
    cpu.write16(t + actors::E_ANGLE, angle as u16);
    nested!(cpu, 0x0605_7e38, explosions::cross_flash(cpu, t));
}

// ---- the lunging shots -----------------------------------------------------------------------------------

/// A lunging shot (an enemy actor run as a bullet): its stage (a byte, +0x7c), its speed (x, y:
/// +0x80), its push (x, y: +0xac), whether it keeps its given heading (a byte, +0x88; else it
/// turns to the nearer player), that heading (a word, +0x8a), frames left in its stage (a byte,
/// +0x89). Gliding to a stop (16 frames, widening), it turns and lunges, pushed by the rank's
/// strength (the words at 0x060a49ec) for 16 frames as it thins; its frame flickers by the table
/// at 0x060a49e8.
const LS_STAGE: u32 = 0x7c;
const LS_SPEED: u32 = 0x80;
const LS_PUSH: u32 = 0xac;
const LS_KEEPS: u32 = 0x88;
const LS_LEFT: u32 = 0x89;
const LS_HEADING: u32 = 0x8a;
const LS_STRENGTHS: u32 = 0x060a_49ec;
const LS_FRAMES: u32 = 0x060a_49e8;

pub fn lunge_shot_behaviour(cpu: &mut Cpu, o: u32) {
    if nested!(cpu, 0x0605_3b10, bullets::bullet_ends(cpu, o, 1)) as u8 == 0 {
        if cpu.read8(o + LS_STAGE) == 0 {
            let n = cpu.read8(o + LS_LEFT).wrapping_sub(1);
            cpu.write8(o + LS_LEFT, n);
            let w = cpu.read32(o + SCALES).wrapping_add(0x800);
            cpu.write32(o + SCALES, w);
            if cpu.read8(o + LS_LEFT) as i8 <= 0 {
                let s = cpu.read8(o + LS_STAGE).wrapping_add(1);
                cpu.write8(o + LS_STAGE, s);
                cpu.write8(o + LS_LEFT, 16);
                if cpu.read8(o + LS_KEEPS) != 0 {
                    let a = cpu.read16(o + LS_HEADING);
                    cpu.write16(o + TURN_Z, a);
                } else {
                    let p = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + 0x10));
                    let a = nested!(cpu, 0x0604_5e30, aim::angle_to(cpu, o, p));
                    cpu.write16(o + TURN_Z, a as u16);
                }
                cpu.write32(o + LS_SPEED, 0);
                cpu.write32(o + LS_SPEED + 4, 0);
                for (k, cos) in [(0, false), (4, true)] {
                    let rank = cpu.read8(RANK) as i8 as i32 as u32;
                    let strength = cpu.read16(LS_STRENGTHS.wrapping_add(rank << 1)) as i16 as i32 as u32;
                    let a = cpu.read16(o + TURN_Z) as i16 as i32 as u32;
                    let (s, by) = if cos {
                        (nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a)), strength.wrapping_neg())
                    } else {
                        (nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a)), strength)
                    };
                    let m = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, by));
                    cpu.write32(o + LS_PUSH + k, m);
                }
            }
            for k in [0, 4] {
                let v = cpu.read32(o + LS_SPEED + k).wrapping_add(cpu.read32(o + LS_PUSH + k));
                cpu.write32(o + LS_SPEED + k, v);
            }
        } else if cpu.read8(o + LS_LEFT) as i8 > 0 {
            let n = cpu.read8(o + LS_LEFT).wrapping_sub(1);
            cpu.write8(o + LS_LEFT, n);
            for k in [0, 4] {
                let v = cpu.read32(o + LS_SPEED + k).wrapping_add(cpu.read32(o + LS_PUSH + k));
                cpu.write32(o + LS_SPEED + k, v);
            }
            let w = cpu.read32(o + SCALES).wrapping_add(0xffff_f800);
            cpu.write32(o + SCALES, w);
            let h = cpu.read32(o + SCALES + 4).wrapping_add(0xffff_fe80);
            cpu.write32(o + SCALES + 4, h);
        }
        for k in [0, 4] {
            let p = cpu.read32(o + POSITION + k).wrapping_add(cpu.read32(o + LS_SPEED + k));
            cpu.write32(o + POSITION + k, p);
        }
    }
    let k = cpu.read32(GAME_FRAMES) & 3;
    let f = cpu.read8(LS_FRAMES + k);
    cpu.write8(o + SPRITE_FRAME, f);
}

/// A lunging shot made from the description `d` (whether it keeps its heading a byte +0x17, its
/// angle a word +0x2c, its heading a word +0x2e, its speed a word +0x30) at the owner `owner`'s
/// gun (+0x484): its number, frame 1 of kind 0, side 1, hit by weapon 5; thrown out along its
/// angle at its speed, slowing to a stop over 16 frames; colours 64 on (as the first character's
/// when it keeps its heading). It, or 0.
const LS_SIZE: u32 = 0xbc;
const LS_BEHAVIOUR: u32 = 0x0609_1710;
const LS_FLAGS: u32 = 0x2000_0400;
const LS_HITS: u32 = 0x060a_49e0;
const LS_GUN: u32 = 0x484;

pub fn lunge_shot_set(cpu: &mut Cpu, d: u32, owner: u32) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, LS_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + 0x10));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HITS_AT));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    let n = cpu.read8(owner + NUMBER);
    cpu.write8(o + NUMBER, n);
    cpu.write32(o + BEHAVIOUR, LS_BEHAVIOUR);
    cpu.write32(o + FLAGS, LS_FLAGS);
    let (x, y) = (cpu.read32(owner + LS_GUN), cpu.read32(owner + LS_GUN + 4));
    cpu.write32(o + POSITION, x);
    cpu.write32(o + POSITION + 4, y);
    cpu.write32(o + POSITION + 8, 0);
    cpu.write8(o + BYTE_33, 126);
    cpu.write8(o + SPRITE_FRAME, 1);
    cpu.write8(o + SPRITE_KIND, 0);
    cpu.write32(o + HITS_AT, LS_HITS);
    cpu.write8(o + SIDE, 1);
    cpu.write8(o + HIT_WEAPON, 5);
    // the SH-2 code keeps the offset of the flag in its frame
    let t = cpu.stack();
    cpu.write32(t, LS_KEEPS);
    let keeps = cpu.read8(d + 0x17);
    cpu.write8(o + LS_KEEPS, keeps);
    cpu.write8(o + LS_LEFT, 16);
    let a = cpu.read16(d + 0x2c);
    cpu.write16(o + TURN_Z, a);
    let h = cpu.read16(d + 0x2e);
    cpu.write16(o + LS_HEADING, h);
    for (k, cos) in [(0, false), (4, true)] {
        let left = cpu.read8(o + LS_LEFT) as i8 as i32;
        let speed = cpu.read16(d + 0x30) as i16 as i32;
        let speed = if cos { speed.wrapping_neg() } else { speed };
        let by = (left.wrapping_mul(speed) << 12) as u32;
        let a = cpu.read16(o + TURN_Z) as i16 as i32 as u32;
        let s = if cos {
            nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a))
        } else {
            nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a))
        };
        let m = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, by));
        cpu.write32(o + LS_SPEED + k, m);
    }
    for k in [0, 4] {
        let v = cpu.read32(o + LS_SPEED + k) as i32;
        let v = if v < 0 { v + 15 } else { v } >> 4;
        cpu.write32(o + LS_PUSH + k, v.wrapping_neg() as u32);
    }
    cpu.write32(o + SCALES, 0x1000);
    cpu.write32(o + SCALES + 4, 0x8000);
    let c = cpu.read16(CHAR_BASE_0);
    let c = if cpu.read8(o + LS_KEEPS) != 0 { c } else { c.wrapping_add(64) };
    cpu.write16(o + OWN_COLOUR, c);
    let c = cpu.read16(o + OWN_COLOUR);
    cpu.write16(o + COLOUR, c);
    LS_KEEPS
}

// ---- the dropped bombs --------------------------------------------------------------------------------

/// A bomb's glow (explosions.rs's glow, run on): it scrolls down with the ground.
const GROUND_SCROLL: u32 = 0x3_4000;

pub fn bomb_glow_behaviour(cpu: &mut Cpu, g: u32) {
    nested!(cpu, 0x0605_8eac, explosions::glow_exec(cpu, g));
    let y = cpu.read32(g + POSITION + 4).wrapping_add(GROUND_SCROLL);
    cpu.write32(g + POSITION + 4, y);
}

/// A dropped bomb (an enemy actor): its stage (a byte, +0x7c), its speed (y +0x84, depth +0xac),
/// its growth (+0xb0). Falling (faster), its frame by its height (kind 57's 6 to 9, flickering);
/// at the ground it bursts (sound 88, a glow scrolling with the ground): a blast (kind 0's frames
/// by the table at 0x060a49e8, at random angles and flips) scrolling with the ground, growing ever
/// slower then shrinking; it ends gone or off the screen.
const BB_STAGE: u32 = 0x7c;
const BB_SPEED_Y: u32 = 0x84;
const BB_SPEED_Z: u32 = 0xac;
const BB_GROWTH: u32 = 0xb0;
const BB_GLOW_BEHAVIOUR: u32 = 0x0609_1a78;
const BB_FLIPS: u32 = 0x30;

pub fn bomb_behaviour(cpu: &mut Cpu, o: u32) {
    let t = cpu.stack();
    if cpu.read8(o + BB_STAGE) == 0 {
        let v = cpu.read32(o + BB_SPEED_Y).wrapping_add(0xe00);
        cpu.write32(o + BB_SPEED_Y, v);
        let v = cpu.read32(o + BB_SPEED_Z).wrapping_add(0xb00);
        cpu.write32(o + BB_SPEED_Z, v);
        if cpu.read32(o + POSITION + 8) as i32 > 0 {
            let s = cpu.read8(o + BB_STAGE);
            cpu.write8(o + BB_STAGE, s.wrapping_add(1));
            cpu.write32(o + FLAGS, 0x400);
            cpu.write32(o + BB_SPEED_Y, GROUND_SCROLL);
            cpu.write32(o + BB_SPEED_Z, s as u32);
            cpu.write32(o + BB_GROWTH, 0x1000);
            cpu.write32(o + SCALES, 0x2000);
            cpu.write8(o + SPRITE_FRAME, 0);
            cpu.write8(o + SPRITE_KIND, 0);
            let c = cpu.read16(CHAR_BASE_0);
            cpu.write16(o + OWN_COLOUR, c);
            cpu.write16(o + COLOUR, c);
            for k in [0, 4, 8] {
                let v = cpu.read32(o + POSITION + k);
                cpu.write32(t + k, v);
            }
            cpu.write32(t + 44, 0x1_4000);
            let g = nested!(cpu, 0x0605_8f34, explosions::glow_set(cpu, t));
            if g != 0 {
                cpu.write32(g + BEHAVIOUR, BB_GLOW_BEHAVIOUR);
            }
            nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 88));
        } else {
            let z = cpu.read32(o + POSITION + 8) as i32;
            for (below, frame) in [(-0x30_0000, 0u8), (-0x28_0000, 2), (-0x20_0000, 4)] {
                if (cpu.read32(o + POSITION + 8) as i32) < below {
                    cpu.write8(o + SPRITE_FRAME, frame);
                }
            }
            let _ = z;
            let frame = if (cpu.read32(o + POSITION + 8) as i32) < -0x18_0000 { 6 } else { 8 };
            cpu.write8(o + SPRITE_FRAME, frame);
            let f = (cpu.read8(GAME_FRAMES + 3) & 1).wrapping_add(cpu.read8(o + SPRITE_FRAME));
            cpu.write8(o + SPRITE_FRAME, f);
        }
    } else {
        let k = cpu.read32(GAME_FRAMES) & 3;
        let f = cpu.read8(LS_FRAMES + k);
        cpu.write8(o + SPRITE_FRAME, f);
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
        cpu.write16(o + TURN_Z, r as u16);
        let g = cpu.read32(o + BB_GROWTH).wrapping_sub(0x80);
        cpu.write32(o + BB_GROWTH, g);
        let w = cpu.read32(o + SCALES).wrapping_add(g);
        cpu.write32(o + SCALES, w);
        cpu.write32(o + SCALES + 4, w);
        if (cpu.read32(o + BB_SPEED_Z) as i32) < 0 && cpu.read32(o + SCALES) as i32 <= 0x2fff {
            cpu.write32(o + FLAGS, 0x8_0400);
        }
        let f = cpu.read32(o + FLAGS) & !BB_FLIPS;
        cpu.write32(o + FLAGS, f);
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
        let f = cpu.read32(o + FLAGS) | (r & BB_FLIPS);
        cpu.write32(o + FLAGS, f);
    }
    let y = cpu.read32(o + POSITION + 4).wrapping_add(cpu.read32(o + BB_SPEED_Y));
    cpu.write32(o + POSITION + 4, y);
    let z = cpu.read32(o + POSITION + 8).wrapping_add(cpu.read32(o + BB_SPEED_Z));
    cpu.write32(o + POSITION + 8, z);
    if cpu.read8(o + BB_STAGE) != 0
        && (nested!(cpu, 0x0605_6a24, aim::off_screen(cpu, o + 0x10)) as u8 != 0 || (cpu.read32(o + SCALES) as i32) < 0)
    {
        cpu.write8(o + OBJ_MODE, TO_STOP);
    }
}

/// A bomb dropped from the description `d` (its place, its number +0x1e): kind 57's frame 0 in
/// its colours, side 1, hit by weapon 5.
const BB_SIZE: u32 = 0xbc;
const BB_BEHAVIOUR: u32 = 0x0609_1aa0;
const BB_FLAGS: u32 = 0x8_0400;
const BB_KIND: u8 = 57;
const BB_HITS: u32 = 0x060a_49f6;

pub fn bomb_set(cpu: &mut Cpu, d: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, BB_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + 0x10));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HITS_AT));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    let n = cpu.read8(d + S_NUMBER);
    cpu.write8(o + NUMBER, n);
    cpu.write32(o + BEHAVIOUR, BB_BEHAVIOUR);
    cpu.write32(o + FLAGS, BB_FLAGS);
    for k in [0, 4, 8] {
        let v = cpu.read32(d + k);
        cpu.write32(o + POSITION + k, v);
    }
    cpu.write32(o + BB_SPEED_Y, 0);
    cpu.write8(o + BYTE_33, 8);
    cpu.write8(o + SPRITE_FRAME, 0);
    cpu.write8(o + SPRITE_KIND, BB_KIND);
    let c = cpu.read16(kind(BB_KIND as u32) + K_COLOUR);
    cpu.write16(o + OWN_COLOUR, c);
    cpu.write16(o + COLOUR, c);
    cpu.write32(o + HITS_AT, BB_HITS);
    cpu.write8(o + SIDE, 1);
    cpu.write8(o + HIT_WEAPON, 5);
}

// ---- the ground pods and the hover mines ---------------------------------------------------------------

/// A ground pod (a numbered enemy, kind 6's sprite): it scrolls down with the ground (its frame
/// turning over), blown up (ground.rs's) when shot; it ends past 128 dots down.
const GP_GONE: i32 = 0x80_0000;

pub fn ground_pod_behaviour(cpu: &mut Cpu, o: u32) {
    if cpu.read32(o + POSITION + 4) as i32 <= GP_GONE {
        if cpu.read32(o + LIFE) as i32 > 0 {
            let f = cpu.read8(o + SPRITE_FRAME).wrapping_add(1) & 31;
            cpu.write8(o + SPRITE_FRAME, f);
            let y = cpu.read32(o + POSITION + 4).wrapping_add(GROUND_SCROLL);
            cpu.write32(o + POSITION + 4, y);
            return;
        }
        nested!(cpu, 0x0605_9e4c, ground::blown_up(cpu, o));
    }
    cpu.write8(o + OBJ_MODE, TO_STOP);
}

/// A ground pod made at `p` (x, y): numbered, kind 6's frame 0 (colours 0x20 on), life 200, side
/// 1, hit by weapon 2.
const GP_SIZE: u32 = 0xbc;
const GP_BEHAVIOUR: u32 = 0x0609_1d68;
const GP_FLAGS: u32 = 0x420;
const GP_KIND: u8 = 6;
const GP_HITS: u32 = 0x0607_3278;
const GP_LIFE: u32 = 200;

pub fn ground_pod_set(cpu: &mut Cpu, p: u32) {
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, 2)) as i8;
    if n == -1 {
        return;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, GP_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, NUMBERED_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + 0x10));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HITS_AT));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + BEHAVIOUR, GP_BEHAVIOUR);
    cpu.write32(o + FLAGS, GP_FLAGS);
    cpu.write8(o + NUMBER, n as u8);
    let k = cpu.read8(o + NUMBER) as i8 as i32 as u32;
    cpu.write32(NUMBERED.wrapping_add(k << 2), o);
    cpu.write8(o + SPRITE_KIND, GP_KIND);
    cpu.write8(o + SPRITE_FRAME, 0);
    cpu.write32(o + HITS_AT, GP_HITS);
    cpu.write32(o + LIFE, GP_LIFE);
    let c = cpu.read16(kind(GP_KIND as u32) + K_COLOUR).wrapping_add(0x20);
    cpu.write16(o + COLOUR, c);
    cpu.write16(o + OWN_COLOUR, c);
    cpu.write8(o + SIDE, 1);
    cpu.write8(o + HIT_WEAPON, 2);
    let (x, y) = (cpu.read32(p), cpu.read32(p + 4));
    cpu.write32(o + POSITION, x);
    cpu.write32(o + POSITION + 4, y);
    cpu.write32(o + POSITION + 8, 0);
}

/// A hover mine (a numbered enemy drawn as two sprites, 52 dots to either side): its hit boxes
/// change by the frame; it scrolls down with the ground and ends past 136 dots down.
const HM_HITS: [u32; 2] = [0x060a_4a0e, 0x060a_49fe];
const HM_GONE: i32 = 0x88_0000;

pub fn hover_mine_behaviour(cpu: &mut Cpu, o: u32) {
    let hits = HM_HITS[(cpu.read32(GAME_FRAMES) & 1) as usize];
    cpu.write32(o + HITS_AT, hits);
    let y = cpu.read32(o + POSITION + 4).wrapping_add(GROUND_SCROLL);
    cpu.write32(o + POSITION + 4, y);
    if y as i32 > HM_GONE {
        cpu.write8(o + OBJ_MODE, TO_STOP);
    }
}

/// Its draw (`s` its sprite part): kind 57's frame 10 (colours 0x2010 on its own), 52 dots to
/// either side.
const HM_KIND: u8 = 57;
const HM_SIDE: u32 = 0x34_0000;

pub fn hover_mine_draw(cpu: &mut Cpu, s: u32) {
    let o = s - 0x10;
    let t = cpu.stack();
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, t));
    cpu.write8(t + 0x1e, HM_KIND);
    cpu.write8(t + 0x1f, 10);
    cpu.write8(t + 0x23, 0xf8);
    let c = cpu.read16(kind(HM_KIND as u32) + K_COLOUR).wrapping_add(0x2010);
    cpu.write16(t + 0x18, c);
    let y = cpu.read32(o + POSITION + 4);
    cpu.write32(t + 8, y);
    cpu.write32(t + 12, 0);
    for side in [HM_SIDE.wrapping_neg(), HM_SIDE] {
        let x = cpu.read32(o + POSITION).wrapping_add(side);
        cpu.write32(t + 4, x);
        nested!(cpu, 0x0604_0960, sprites::draw(cpu, t));
    }
}

/// A hover mine made at (`x`, `y`) for the carrier `carrier`, in its lane (a byte, +0x41c: 16
/// dots a lane), the lane then moved one along at random (between -2 and 2); numbered, life 10,
/// side 3, hit by weapon 4. With `drops` (and the carrier's +0x76 clear) a ground pod with it.
const HM_SIZE: u32 = 0xbc;
const HM_BEHAVIOUR: u32 = 0x0609_1eb0;
const HM_DRAW: u32 = 0x0609_1ef8;
const HM_FLAGS: u32 = 0xc6_0400;
const CARRIER_LANE: u32 = 0x41c;
const CARRIER_HELD: u32 = 0x76;

pub fn hover_mine_set(cpu: &mut Cpu, x: u32, y: u32, carrier: u32, drops: u32) {
    let drops = drops as u8;
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, 2)) as i8;
    if n == -1 {
        return;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, HM_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, NUMBERED_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + 0x10));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HITS_AT));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + BEHAVIOUR, HM_BEHAVIOUR);
    cpu.write32(o + FLAGS, HM_FLAGS);
    cpu.write32(o + MODEL_DRAW, HM_DRAW);
    cpu.write8(o + NUMBER, n as u8);
    let k = cpu.read8(o + NUMBER) as i8 as i32 as u32;
    cpu.write32(NUMBERED.wrapping_add(k << 2), o);
    cpu.write32(o + HITS_AT, HM_HITS[1]);
    cpu.write32(o + LIFE, 10);
    cpu.write8(o + SIDE, 3);
    cpu.write8(o + HIT_WEAPON, 4);
    let lane = cpu.read8(carrier + CARRIER_LANE) as i8 as i32 as u32;
    cpu.write32(o + POSITION, x.wrapping_add(lane << 20));
    cpu.write32(o + POSITION + 4, y);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    let lane = cpu.read8(carrier + CARRIER_LANE) as i8;
    let lane = if r & 0x10 != 0 {
        if lane > -2 { lane - 1 } else { lane + 1 }
    } else if lane > 1 {
        lane - 1
    } else {
        lane + 1
    };
    cpu.write8(carrier + CARRIER_LANE, lane as u8);
    if drops != 0 && cpu.read16(carrier + CARRIER_HELD) == 0 {
        let t = cpu.stack();
        let (x, y) = (cpu.read32(o + POSITION), cpu.read32(o + POSITION + 4));
        cpu.write32(t, x);
        cpu.write32(t + 4, y);
        nested!(cpu, 0x0609_1dc0, ground_pod_set(cpu, t));
    }
}

// ---- the carrier ------------------------------------------------------------------------------------------

/// The carrier's target: the numbered object its byte +0x41e names.
const CARRIER_TARGET: u32 = 0x41e;

pub fn carrier_target(cpu: &mut Cpu, o: u32) -> u32 {
    let n = cpu.read8(o + CARRIER_TARGET) as i8 as i32 as u32;
    cpu.read32(NUMBERED.wrapping_add(n << 2))
}

/// To step 5: 96 dots below, 45 frames; its sweep cleared, at full size.
pub fn carrier_to_5(cpu: &mut Cpu, o: u32) {
    cpu.write8(o + CARRIER_STEP, 5);
    cpu.write32(o + 0x408, 0x60_0000);
    cpu.write16(o + 0x4cc, 45);
    cpu.write32(o + 0x4d8, 0);
    cpu.write32(o + 0x4dc, 0);
    cpu.write32(o + 0x4e0, 0x1_0000);
    cpu.write32(o + 0x4e4, 0x1_0000);
}

/// Sound 83 when the carrier's +0x404 is not `v`.
pub fn carrier_hum(cpu: &mut Cpu, o: u32, v: u32) {
    if cpu.read32(o + 0x404) != v {
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 83));
    }
}

/// From rank 2, six lunging shots from its gun (sound 41), fanned at sixths of a turn, speed 7.
const CARRIER_FAN: [u16; 6] = [0xe000, 0x2000, 0xc000, 0x4000, 0xa000, 0x6000];

pub fn carrier_lunge_fan(cpu: &mut Cpu, o: u32) {
    if cpu.read8(RANK) as i8 <= 1 {
        return;
    }
    let t = cpu.stack();
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 41));
    cpu.write32(t + 20, 0);
    cpu.write16(t + 0x30, 7);
    for a in CARRIER_FAN {
        cpu.write16(t + 0x2c, a);
        nested!(cpu, 0x0609_189c, lunge_shot_set(cpu, t, o));
    }
}

/// The carrier's events, as the ground scrolls by (its two tracks, +0x414: every 256 dots of
/// scroll the next event word from 0x060a48be, by its index +0x40d; 10 restarts the list, counted
/// in +0x76). Each event: its low nibble (+0x410 by track) what the carrier does when the track
/// comes level with it; the side it works from (+0x40e by track: on even laps away from where it
/// is aiming, the other way below rank 3; else at random); its thruster glow (bits 8 to 11); and
/// by its low byte: 0xa1, 0xb1, 0xc1 a fan of lunging shots and a line of hover mines (6, 3, 6;
/// the first with ground pods), 0xb2 three more further up, 0xd0 three on the far side with pods,
/// 0xe0 its escorts' bit 1 toggled and a hold (30 at +0x3f5), 0xf0 bit 0 toggled.
/// Level with it: 0 missiles from its gun (two, 7/256 of a turn apart, while in step 3 with its
/// guard up, the second track only once +0x3f4 is set, or in step 5 with both its gun parts
/// broken); 1 to 4, 6 and 8 a new place to go (+0x404, out by the side, at a speed and its hold
/// +0x3f8; with sound 83 when it changes); 9 to step 5 on even laps, else to step 4 (sound 82).
/// Then it heads for its place (+0x3fa) and scrolls with the ground.
const CE_TRACKS: u32 = 0x414;
const CE_EVENT: u32 = 0x410;
const CE_SIDE: u32 = 0x40e;
const CE_INDEX: u32 = 0x40d;
const CE_NOW: u32 = 0x40c;
const CE_EVENTS: u32 = 0x060a_48be;
const CE_LAPS: u32 = CARRIER_HELD;
const CE_GO_HEIGHT: u32 = 0x3fc;
const CE_GO_X: u32 = 0x404;
const CE_GO_HOLD: u32 = 0x3f8;
const CE_HEADING: u32 = 0x3fa;
const CE_HOLD: u32 = 0x3f5;
const CE_GUARD: u32 = 0x408;
const CE_ONCE: u32 = 0x3f4;
const CE_GUN: u32 = 0x484;
const CE_OUT: u32 = 0x78_0000;
const CE_MINES_Y: i32 = -0x114;
const CE_MINES_UP_Y: i32 = -0x194;

fn ce_mines(cpu: &mut Cpu, o: u32, x: impl Fn(&mut Cpu) -> u32, y0: i32, count: i8, drops: u32) {
    let mut k: i8 = 0;
    while k < count {
        let x = x(cpu);
        let y = (y0 - 44 * k as i32) << 16;
        nested!(cpu, 0x0609_1f80, hover_mine_set(cpu, x, y as u32, o, drops));
        k += 1;
    }
}

pub fn carrier_events(cpu: &mut Cpu, o: u32) {
    let t = cpu.stack();
    cpu.write32(t + 72, CE_TRACKS);
    cpu.write32(t + 76, CE_EVENT);
    cpu.write32(t + 80, CE_SIDE);
    for i in 0..2u32 {
        let s = cpu.read32(o + CE_TRACKS + 4 * i).wrapping_add(0xfffc_c000);
        cpu.write32(o + CE_TRACKS + 4 * i, s);
        if s as i32 >= -0x80_0000 {
            continue;
        }
        let idx = cpu.read8(o + CE_INDEX).wrapping_add(1);
        cpu.write8(o + CE_INDEX, idx);
        let mut v = cpu.read16(CE_EVENTS + 2 * idx as u32) as u32;
        if v == 10 {
            let n = cpu.read16(o + CE_LAPS).wrapping_add(1);
            cpu.write16(o + CE_LAPS, n);
            cpu.write8(o + CE_INDEX, 0);
            v = cpu.read16(CE_EVENTS) as u32;
        }
        cpu.write8(o + CE_EVENT + i, (v & 15) as u8);
        let s = cpu.read32(o + CE_TRACKS + 4 * i).wrapping_add(0x100_0000);
        cpu.write32(o + CE_TRACKS + 4 * i, s);
        let laps = (cpu.read16(o + CE_LAPS) as i16 as i32 as u32).wrapping_add(1);
        if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, laps, 2)) == 0 {
            nested!(cpu, 0x0604_0ccc, aim::target(cpu, o, 0, t + 64));
            let side: u8 = if cpu.read32(t + 64) as i32 > 0 { 0xff } else { 1 };
            cpu.write8(o + CE_SIDE + i, side);
            if cpu.read8(RANK) as i8 <= 2 {
                let s = cpu.read8(o + CE_SIDE + i).wrapping_neg();
                cpu.write8(o + CE_SIDE + i, s);
            }
        } else {
            let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
            cpu.write8(o + CE_SIDE + i, if r & 16 != 0 { 0xff } else { 1 });
        }
        nested!(cpu, 0x0609_004c, thruster_set(cpu, o, v & 0xf00));
        let side_x = move |cpu: &mut Cpu| (cpu.read8(o + CE_SIDE + i) as i8 as i32 as u32).wrapping_mul(0xff88_0000);
        match v & 0xff {
            0xa1 | 0xb1 | 0xc1 => {
                let e = v & 0xff;
                nested!(cpu, 0x0609_2154, carrier_lunge_fan(cpu, o));
                let s = cpu.read8(o + CE_SIDE + i);
                cpu.write8(o + CARRIER_LANE, s);
                let drops = (e == 0xa1) as u32;
                cpu.write32(t + 92, drops);
                let count = if e == 0xc1 || e == 0xa1 { 6 } else { 3 };
                ce_mines(cpu, o, side_x, CE_MINES_Y, count, drops);
                cpu.write8(o + CE_EVENT + i, 1);
            }
            0xb2 => {
                nested!(cpu, 0x0609_2154, carrier_lunge_fan(cpu, o));
                let s = cpu.read8(o + CE_SIDE + i);
                cpu.write8(o + CARRIER_LANE, s);
                ce_mines(cpu, o, side_x, CE_MINES_UP_Y, 3, 0);
                cpu.write8(o + CE_EVENT + i, 2);
            }
            0xd0 => {
                let x = if cpu.read32(o + POSITION) as i32 > 0 { CE_OUT.wrapping_neg() } else { CE_OUT };
                ce_mines(cpu, o, move |_| x, CE_MINES_Y, 3, 1);
                cpu.write8(o + CE_EVENT + i, 0);
            }
            0xe0 => {
                let f = cpu.read16(o + CARRIER_ESCORTS) ^ 2;
                cpu.write16(o + CARRIER_ESCORTS, f);
                cpu.write16(o + 0x70, 0);
                cpu.write8(o + CE_EVENT + i, 0);
                cpu.write8(o + CE_HOLD, 30);
            }
            0xf0 => {
                let f = cpu.read16(o + CARRIER_ESCORTS) ^ 1;
                cpu.write16(o + CARRIER_ESCORTS, f);
                cpu.write8(o + CE_EVENT + i, 0);
            }
            _ => {}
        }
    }
    cpu.write32(t + 84, CE_SIDE);
    cpu.write32(t + 88, CE_GO_HEIGHT);
    cpu.write32(t + 92, CE_GO_X);
    for i in 0..2u32 {
        if cpu.read8(o + CE_SIDE + i) == 0 {
            continue;
        }
        let track = cpu.read32(o + CE_TRACKS + 4 * i).wrapping_neg() as i32;
        if track < cpu.read32(o + POSITION + 4).wrapping_add(0xfff0_0000) as i32 {
            continue;
        }
        let e = cpu.read8(o + CE_EVENT + i);
        cpu.write8(o + CE_NOW, e);
        let side = cpu.read8(o + CE_SIDE + i) as i8 as i32 as u32;
        let go = |cpu: &mut Cpu, x: u32, height: u16, hold: u8| {
            nested!(cpu, 0x0609_2130, carrier_hum(cpu, o, x));
            cpu.write16(o + CE_GO_HEIGHT, height);
            cpu.write32(o + CE_GO_X, x);
            cpu.write8(o + CE_GO_HOLD, hold);
        };
        match e {
            0 => {
                let state = cpu.read8(o + CARRIER_STEP);
                let fire = (state == 3
                    && cpu.read32(o + CE_GUARD) as i32 > 0
                    && (cpu.read8(o + CE_ONCE) == 0 || i != 0))
                    || (state == 5 && cpu.read16(o + 0x92) & 0x40 != 0 && cpu.read16(o + 0x94) & 0x40 != 0);
                if fire {
                    let (x, y) = (cpu.read32(o + CE_GUN), cpu.read32(o + CE_GUN + 4));
                    cpu.write32(t, x);
                    cpu.write32(t + 4, y);
                    cpu.write32(t + 8, 0);
                    cpu.write32(t + 56, 0x1_c000);
                    cpu.write16(t + 46, 40);
                    cpu.write16(t + 50, 0x2000);
                    let a = cpu.read16(o + TURN_Z);
                    cpu.write16(t + 44, a);
                    cpu.write16(t + 52, a.wrapping_add(0xe00));
                    nested!(cpu, 0x0605_49f8, bullets::missile_set(cpu, t));
                    let a = cpu.read16(t + 44).wrapping_add(0xf200);
                    cpu.write16(t + 52, a);
                    nested!(cpu, 0x0605_49f8, bullets::missile_set(cpu, t));
                }
            }
            1 => go(cpu, side.wrapping_mul(CE_OUT), 32, 80),
            2 => go(cpu, side.wrapping_mul(CE_OUT), 0x80, 24),
            3 => go(cpu, 0, 32, 80),
            4 => go(cpu, 0, 0x80, 24),
            6 => go(cpu, CE_OUT, 96, 56),
            8 => go(cpu, CE_OUT.wrapping_neg(), 96, 56),
            9 => {
                let laps = (cpu.read16(o + CE_LAPS) as i16 as i32 as u32).wrapping_add(1);
                if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, laps, 2)) == 0 {
                    nested!(cpu, 0x0609_20ec, carrier_to_5(cpu, o));
                } else {
                    cpu.write8(o + CARRIER_STEP, 4);
                    cpu.write32(o + 0x400, 0x4_c000);
                    cpu.write8(o + CE_GO_HOLD, 64);
                    cpu.write32(o + 0x4d8, 0);
                    cpu.write32(o + 0x4dc, 0);
                    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 82));
                    nested!(cpu, 0x0609_35e8, carrier_parts_shut(cpu, o));
                }
            }
            _ => {}
        }
        cpu.write8(o + CE_SIDE + i, 0);
    }
    let dx = cpu.read32(o + CE_GO_X).wrapping_sub(cpu.read32(o + POSITION));
    let dy = ((cpu.read16(o + CE_GO_HEIGHT) as i16 as i32) << 16).wrapping_neg() as u32;
    let a = nested!(cpu, 0x0606_780c, sgl::sl_atan(cpu, dx, dy));
    cpu.write16(o + CE_HEADING, (a as u16).wrapping_add(0x4000));
    let y = cpu.read32(o + POSITION + 4).wrapping_add(GROUND_SCROLL);
    cpu.write32(o + POSITION + 4, y);
}

/// The carrier's speeds (x, y, z: +0x78), its turns' speeds (words: TURN_Y's +0x88 eased by
/// +0x8a, TURN_Z's +0x8c eased by +0x8e), its forward speed (+0x400), its tail (a byte, +0x41d:
/// set, it leaves by the top).
const CR_SPEED: u32 = 0x78;
const CR_TURN_Y_SPEED: u32 = 0x88;
const CR_TURN_Y_EASE: u32 = 0x8a;
const CR_TURN_Z_SPEED: u32 = 0x8c;
const CR_TURN_Z_EASE: u32 = 0x8e;
const CR_FORWARD: u32 = 0x400;
const CR_LEAVING: u32 = 0x41d;

/// Its heading eased toward its place's (at most 0x2000, eased by 8, by its hold), banked
/// against it (at most 0x4000, by 16, by 96).
pub fn carrier_turns(cpu: &mut Cpu, o: u32) {
    let from = cpu.read16(o + TURN_Z) as u32;
    let to = cpu.read16(o + CE_HEADING) as u32;
    let hold = cpu.read8(o + CE_GO_HOLD) as u32;
    cpu.push(&[0x2000, 8, hold]);
    nested!(cpu, 0x0605_70d8, aim::turn_toward(cpu, o + CR_TURN_Z_SPEED, o + CR_TURN_Z_EASE, from, to, 0x2000, 8, hold));
    let from = cpu.read16(o + TURN_Y) as u32;
    let to = cpu.read16(o + TURN_Z).wrapping_neg() as u32;
    cpu.push(&[0x4000, 16, 96]);
    nested!(cpu, 0x0605_70d8, aim::turn_toward(cpu, o + CR_TURN_Y_SPEED, o + CR_TURN_Y_EASE, from, to, 0x4000, 16, 96));
    cpu.pop(24);
}

/// Its cruise: 4.25 dots a frame across its heading; forward at 2.25 dots a frame while above
/// its guard line (+0x408, a little more at random), else easing to 4.125.
pub fn carrier_cruise(cpu: &mut Cpu, o: u32) {
    let a = cpu.read16(o + TURN_Z) as i16 as i32 as u32;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    let x = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, 0x4_4000));
    cpu.write32(o + CR_SPEED, x);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    let line = cpu.read32(o + CE_GUARD).wrapping_add(r) as i32;
    let (to, step) = if (cpu.read32(o + POSITION + 4) as i32) < line { (0x2_4000, 0x8000) } else { (0x4_2000, 0x7000) };
    let f = cpu.read32(o + CR_FORWARD);
    let f = nested!(cpu, 0x0604_6168, actors::approach(cpu, f, to, step));
    cpu.write32(o + CR_FORWARD, f);
    let a = cpu.read16(o + TURN_Z) as i16 as i32 as u32;
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    let y = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, c, f));
    cpu.write32(o + CR_SPEED + 4, y.wrapping_neg());
}

/// Steps 0 and 8: it rises from the depths in an arc, banking as it comes (its bank 32 a dot
/// up, swung round by TURN_Y), 2 dots a frame up; once past the top of its arc, the next step,
/// level, sliding back (or on, leaving).
pub fn carrier_rising(cpu: &mut Cpu, o: u32) {
    let s = cpu.read16(o + CR_TURN_Y_SPEED) as i16 as u32;
    let s = nested!(cpu, 0x0604_6168, actors::approach(cpu, s, 0x800, 32));
    cpu.write16(o + CR_TURN_Y_SPEED, s as u16);
    let y = cpu.read32(o + POSITION + 4) as i32;
    let up = (y / 0x1_0000) as i16;
    cpu.write16(o + CR_TURN_Y_EASE, (up as i32 * -32) as u16);
    let swing = |cpu: &mut Cpu| {
        cpu.read16(o + CR_TURN_Y_EASE).wrapping_add(0x8000).wrapping_add(cpu.read16(o + TURN_Y)) as i16 as i32 as u32
    };
    let a = swing(cpu);
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a)) as i32;
    let ny = (cpu.read32(o + POSITION + 4) as i32).wrapping_neg();
    let half = (((ny.wrapping_add(if ny < 0 { 0x1_ffff } else { 0 }) >> 16) as i16 as i32) >> 1) - 32;
    cpu.write32(o + POSITION, s.wrapping_mul(half) as u32);
    let a = swing(cpu);
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a)) as i32;
    cpu.write32(o + POSITION + 8, (c.wrapping_mul(-32) as u32).wrapping_add(0xffc0_0000));
    cpu.write32(o + CR_SPEED + 4, 0xfffe_0000);
    if (cpu.read32(o + POSITION + 4) as i32) < 0 {
        let s = cpu.read8(o + CARRIER_STEP).wrapping_add(1);
        cpu.write8(o + CARRIER_STEP, s);
        cpu.write16(o + TURN_X, 0);
        cpu.write16(o + TURN_Z, 0);
        let v = if cpu.read8(o + CR_LEAVING) == 0 {
            nested!(cpu, 0x0609_3594, carrier_parts_open(cpu, o));
            0xfffc_8000
        } else {
            0x4_0000
        };
        cpu.write32(o + CR_SPEED, v);
    }
}

/// Steps 1 and 9: it swings out: sliding across (faster), turning, sinking (to step 2 once
/// moving right and past 48 dots left of the middle); leaving, it climbs away off the top
/// (step 10, hidden, still).
pub fn carrier_swinging(cpu: &mut Cpu, o: u32) {
    if cpu.read8(o + CR_LEAVING) == 0 {
        let v = cpu.read32(o + CR_SPEED + 4).wrapping_add(0xa00);
        cpu.write32(o + CR_SPEED + 4, v);
        let v = cpu.read32(o + CR_SPEED).wrapping_add(0x1400);
        cpu.write32(o + CR_SPEED, v);
        let s = cpu.read16(o + CR_TURN_Y_SPEED).wrapping_add(0xffd3);
        cpu.write16(o + CR_TURN_Y_SPEED, s);
        let v = cpu.read32(o + CR_SPEED + 8).wrapping_add(0x696);
        cpu.write32(o + CR_SPEED + 8, v);
        if cpu.read32(o + CR_SPEED) as i32 > 0 && cpu.read32(o + POSITION) as i32 > -0x30_0000 {
            let s = cpu.read8(o + CARRIER_STEP).wrapping_add(1);
            cpu.write8(o + CARRIER_STEP, s);
            let s = cpu.read16(o + CR_TURN_Y_SPEED).wrapping_mul(2);
            cpu.write16(o + CR_TURN_Y_EASE, s);
        }
    } else {
        let s = cpu.read16(o + CR_TURN_Z_SPEED).wrapping_sub(4);
        cpu.write16(o + CR_TURN_Z_SPEED, s);
        let v = cpu.read32(o + CR_SPEED).wrapping_add(0xffff_c000);
        cpu.write32(o + CR_SPEED, v);
        let v = cpu.read32(o + CR_SPEED + 4).wrapping_add(0xffff_f800);
        cpu.write32(o + CR_SPEED + 4, v);
        let s = cpu.read16(o + CR_TURN_Y_SPEED).wrapping_add(48);
        cpu.write16(o + CR_TURN_Y_SPEED, s);
        let v = cpu.read32(o + CR_SPEED + 8).wrapping_add(0xffff_ff00);
        cpu.write32(o + CR_SPEED + 8, v);
        if (cpu.read32(o + POSITION + 4) as i32) < -0x120_0000 {
            cpu.write8(o + CARRIER_STEP, 10);
            cpu.write32(o + FLAGS, 0x8008_0000);
            for k in [0, 4, 8] {
                cpu.write32(o + CR_SPEED + k, 0);
            }
        }
    }
}

/// Step 2: it settles, turning to its place, slowing, rising; still, it holds (step 3) 16 dots
/// deep.
pub fn carrier_settling(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0609_2878, carrier_turns(cpu, o));
    let v = cpu.read32(o + CR_SPEED);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0, 0x1234));
    cpu.write32(o + CR_SPEED, v);
    let v = cpu.read32(o + CR_SPEED + 8);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0, 0x1000));
    cpu.write32(o + CR_SPEED + 8, v);
    let y = cpu.read32(o + CR_SPEED + 4).wrapping_add(0xffff_f900);
    cpu.write32(o + CR_SPEED + 4, y);
    if cpu.read32(o + CR_SPEED + 8) == 0 {
        cpu.write8(o + CARRIER_STEP, 3);
        cpu.write32(o + CR_SPEED + 4, 0);
        cpu.write32(o + POSITION + 8, 0x10_0000);
    }
}

/// Step 3: guarding. Its events; its guard line (104 dots down on even laps, else 28 up). With
/// its escorts' bit 1 its guard comes up (+0x3f4 to 16 by 2, sound 81 at 14): after 40 frames it
/// faces ahead, edges toward the nearer player (an eighth of a dot a frame when 12 dots off) but
/// keeps to its lanes (104 to 136 dots out either side, or within 20 of the middle); every so
/// many frames by the rank (the bytes at 0x060a4898) lunging shots from the guns not broken (or
/// both, below the middle); above the middle flares from its two pods by turns every 25 frames,
/// its beams at frame 45; with its guns and pods broken but the core whole, every so many frames
/// by the rank (the bytes at 0x060a4a20) a shell (sound 78) behind it. Else its hold runs down,
/// then its guard goes down (sound 81 at 2). It turns and cruises.
const CG_LUNGES: u32 = 0x060a_4898;
const CG_SHELLS: u32 = 0x060a_4a20;
const CR_PARTS: [u32; 4] = [0x96, 0x98, 0x9a, 0x9c];
const CR_BROKEN: u16 = 0x40;
const CR_POD_A: u32 = 0x490;
const CR_POD_B: u32 = 0x49c;

pub fn carrier_guarding(cpu: &mut Cpu, o: u32) {
    let t = cpu.stack();
    nested!(cpu, 0x0609_21ec, carrier_events(cpu, o));
    let laps = (cpu.read16(o + CE_LAPS) as i16 as i32 as u32).wrapping_add(1);
    let guard = if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, laps, 2)) == 0 { 0x68_0000 } else { 0xffe4_0000 };
    cpu.write32(o + CE_GUARD, guard);
    let n = cpu.read8(o + NUMBER);
    cpu.write8(t + S_NUMBER, n);
    let broken = |cpu: &mut Cpu, k: usize| cpu.read16(o + CR_PARTS[k]) & CR_BROKEN != 0;
    if cpu.read16(o + CARRIER_ESCORTS) & 2 != 0 {
        let g = cpu.read8(o + CE_ONCE) as i8 as u32;
        let g = nested!(cpu, 0x0604_6168, actors::approach(cpu, g, 16, 2));
        cpu.write8(o + CE_ONCE, g as u8);
        if g as i8 == 14 {
            nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 81));
        }
        if cpu.read8(o + CE_ONCE) != 16 || cpu.read16(o + 0x70) <= 40 {
            nested!(cpu, 0x0609_2878, carrier_turns(cpu, o));
            nested!(cpu, 0x0609_28f8, carrier_cruise(cpu, o));
            return;
        }
        cpu.write16(o + CE_HEADING, 0);
        let p = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + 0x10));
        let d = cpu.read32(o + POSITION).wrapping_sub(cpu.read32(p + POSITION));
        cpu.write32(t + 44, d);
        if d as i32 > 0xc_0000 {
            let x = cpu.read32(o + POSITION).wrapping_add(0xffff_e000);
            cpu.write32(o + POSITION, x);
        }
        if (cpu.read32(t + 44) as i32) < -0xc_0000 {
            let x = cpu.read32(o + POSITION).wrapping_add(0x2000);
            cpu.write32(o + POSITION, x);
        }
        let clamp = |cpu: &mut Cpu, most: i32, least: i32| {
            if cpu.read32(o + POSITION) as i32 > most {
                cpu.write32(o + POSITION, most as u32);
            }
            if (cpu.read32(o + POSITION) as i32) < least {
                cpu.write32(o + POSITION, least as u32);
            }
        };
        let x = cpu.read32(o + POSITION) as i32;
        if x > 0x40_0000 {
            clamp(cpu, 0x88_0000, 0x68_0000);
        } else if x < -0x40_0000 {
            if (cpu.read32(o + POSITION) as i32) < -0x88_0000 {
                cpu.write32(o + POSITION, (-0x88_0000i32) as u32);
            }
            if cpu.read32(o + POSITION) as i32 > -0x68_0000 {
                cpu.write32(o + POSITION, (-0x68_0000i32) as u32);
            }
        } else {
            clamp(cpu, 0x14_0000, -0x14_0000);
        }
        let rank = cpu.read8(RANK) as i8 as i32 as u32;
        let gap = cpu.read8(CG_LUNGES.wrapping_add(rank)) as u32;
        let c = cpu.read16(o + 0x70) as u32;
        let m = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, c, gap));
        if m == 0 {
            nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 41));
            cpu.write32(t + 20, m);
            cpu.write16(t + 0x30, 7);
            for (a, k1, k2) in [(0xd000u16, 3, 1), (0x3000, 2, 0)] {
                if cpu.read32(o + POSITION + 4) as i32 > 0 || broken(cpu, k1) || broken(cpu, k2) {
                    cpu.write16(t + 0x2c, a);
                    nested!(cpu, 0x0609_189c, lunge_shot_set(cpu, t, o));
                }
            }
        }
        if (cpu.read32(o + POSITION + 4) as i32) < 0 {
            let c = cpu.read16(o + 0x70) as u32;
            if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, c, 25)) == 0 {
                let c = cpu.read16(o + 0x70) as u32;
                let (part, pod) = if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, c, 50)) == 0 { (2, CR_POD_A) } else { (3, CR_POD_B) };
                if !broken(cpu, part) {
                    let (x, y) = (cpu.read32(o + pod), cpu.read32(o + pod + 4));
                    cpu.write32(t, x);
                    cpu.write32(t + 4, y);
                    nested!(cpu, 0x0609_10dc, flare_set(cpu, t));
                }
            }
        }
        if (cpu.read32(o + POSITION + 4) as i32) < 0 && cpu.read16(o + 0x70) == 45 {
            if !broken(cpu, 0) {
                nested!(cpu, 0x0609_0c7c, carrier_beam_set(cpu, o, 0));
            }
            if !broken(cpu, 1) {
                nested!(cpu, 0x0609_0c7c, carrier_beam_set(cpu, o, 1));
            }
        }
        if broken(cpu, 0)
            && cpu.read32(o + CR_PARTS[1]) & 0x40_0040 == 0x40_0040
            && broken(cpu, 3)
            && (cpu.read32(o + POSITION + 4) as i32) < 0
        {
            let rank = cpu.read8(RANK) as i8 as i32 as u32;
            let gap = cpu.read8(CG_SHELLS.wrapping_add(rank)) as u32;
            let c = cpu.read16(o + 0x70) as u32;
            let m = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, c, gap));
            if m == 0 {
                nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 78));
                let (x, y) = (cpu.read32(o + CE_GUN), cpu.read32(o + CE_GUN + 4));
                cpu.write32(t, x);
                cpu.write32(t + 4, y);
                cpu.write32(t + 8, m);
                let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x800));
                let a = (r as u16).wrapping_add(0x8000);
                cpu.write16(t + 44, a);
                cpu.write32(t + 48, 0x1_0000);
                cpu.write16(t + 46, a);
                cpu.write16(t + 56, 0);
                cpu.write32(t + 52, 0x2_0000);
                nested!(cpu, 0x0605_587c, bullets::shell_set(cpu, t));
            }
        }
    } else {
        let h = cpu.read8(o + CE_HOLD);
        if h != 0 {
            cpu.write8(o + CE_HOLD, h.wrapping_sub(1));
        } else {
            let g = cpu.read8(o + CE_ONCE) as i8 as u32;
            let g = nested!(cpu, 0x0604_6168, actors::approach(cpu, g, 0, 2));
            cpu.write8(o + CE_ONCE, g as u8);
            if g as i8 == 2 {
                nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 81));
            }
        }
    }
    nested!(cpu, 0x0609_2878, carrier_turns(cpu, o));
    nested!(cpu, 0x0609_28f8, carrier_cruise(cpu, o));
}

/// Step 4: it dives under (its events, turning): rolling over ever faster (to 3/256 of a turn a
/// frame), sinking and surfacing by its roll; once over, 16 dots deep, level, step 5. Its forward
/// speed eases to 1/4.
const CR_ROLL: u32 = 0x4d8;
const CR_ROLL_SPEED: u32 = 0x4dc;

pub fn carrier_diving(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0609_21ec, carrier_events(cpu, o));
    nested!(cpu, 0x0609_2878, carrier_turns(cpu, o));
    let v = cpu.read32(o + CR_ROLL_SPEED);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0x300, 8));
    cpu.write32(o + CR_ROLL_SPEED, v);
    let a = cpu.read16(o + TURN_X).wrapping_add(cpu.read16(o + CR_ROLL_SPEED + 2));
    cpu.write16(o + TURN_X, a);
    let r = cpu.read32(o + CR_ROLL).wrapping_add(cpu.read32(o + CR_ROLL_SPEED));
    cpu.write32(o + CR_ROLL, r);
    let a = cpu.read16(o + TURN_X) as i16 as i32 as u32;
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    cpu.write32(o + POSITION + 8, (c << 6).wrapping_add(0xffd0_0000));
    if cpu.read32(o + CR_ROLL) as i32 > 0x1_0000 {
        cpu.write32(o + POSITION + 8, 0x10_0000);
        cpu.write16(o + TURN_X, 0);
        cpu.write16(o + 0x84, 0);
        nested!(cpu, 0x0609_20ec, carrier_to_5(cpu, o));
    }
    let f = cpu.read32(o + CR_FORWARD);
    let f = nested!(cpu, 0x0604_6168, actors::approach(cpu, f, 0x4000, 0x900));
    cpu.write32(o + CR_FORWARD, f);
    let a = cpu.read16(o + TURN_Z) as i16 as i32 as u32;
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    let y = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, c, f));
    cpu.write32(o + CR_SPEED + 4, y.wrapping_neg());
}

/// Step 5: fighting (its events, turning, cruising) for 800 frames, then guarding again. Its
/// guard comes part up (to 11, sound 81 and its glow at 10); its guard line follows the nearer
/// player (96 dots below, a quarter of the player's depth more), 1/2 a dot a frame. From frame
/// 161 to 719, every so many frames by the rank (the bytes at 0x060a48a8) a lunging shot (sound
/// 41), from its left gun on even turns. To frame 719, every 80 frames a big charge (colours 0xb0)
/// at a gun not broken, by turns: 40 frames on it throws up a barrier (sound 87, its 4 pieces at
/// the gun's points, a cross flash) which grows (by 3/64, slowing by the rank's bytes at
/// 0x060a48a0) and turns (by 3/64, slowing by 72 a frame) until its gun breaks or it shrinks away.
const CF_COUNT: u32 = 0x4cc;
const CF_LUNGES: u32 = 0x060a_48a8;
const CF_SLOWS: u32 = 0x060a_48a0;
/// Each barrier: its on flag (a byte), its gun's part, its size, its growth and turn (words),
/// its turn so far, its points.
const CF_BARRIERS: [(u32, u32, u32, u32, u32, u32, u32); 2] =
    [(0x41f, 0x92, 0x4d8, 0x4d0, 0x4d4, 0x4e0, 0x424), (0x420, 0x94, 0x4dc, 0x4d2, 0x4d6, 0x4e4, 0x454)];

pub fn carrier_fighting(cpu: &mut Cpu, o: u32) {
    let t = cpu.stack();
    nested!(cpu, 0x0609_21ec, carrier_events(cpu, o));
    nested!(cpu, 0x0609_2878, carrier_turns(cpu, o));
    nested!(cpu, 0x0609_28f8, carrier_cruise(cpu, o));
    let c = cpu.read16(o + CF_COUNT).wrapping_add(1);
    cpu.write16(o + CF_COUNT, c);
    if c as i16 > 800 {
        cpu.write8(o + CARRIER_STEP, 3);
    }
    let g = cpu.read8(o + CE_ONCE) as i8 as u32;
    let g = nested!(cpu, 0x0604_6168, actors::approach(cpu, g, 11, 1));
    cpu.write8(o + CE_ONCE, g as u8);
    if g as i8 == 10 {
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 81));
        nested!(cpu, 0x0609_3594, carrier_parts_open(cpu, o));
    }
    let p = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + 0x10));
    let py = cpu.read32(p + POSITION + 4) as i32;
    let to = if py > 0 { ((py >> 2) as u32).wrapping_add(0x60_0000) } else { 0x60_0000 };
    let gl = cpu.read32(o + CE_GUARD);
    let gl = nested!(cpu, 0x0604_6168, actors::approach(cpu, gl, to, 0x8000));
    cpu.write32(o + CE_GUARD, gl);
    let c = cpu.read16(o + CF_COUNT);
    if c.wrapping_sub(0xa1) <= 0x22e {
        let rank = cpu.read8(RANK) as i8 as i32 as u32;
        let gap = cpu.read8(CF_LUNGES.wrapping_add(rank)) as u32;
        let c = cpu.read16(o + CF_COUNT) as i16 as i32 as u32;
        let m = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, c, gap));
        if m == 0 {
            cpu.write32(t + 20, m);
            cpu.write16(t + 0x30, 7);
            let rank = cpu.read8(RANK) as i8 as i32 as u32;
            let gap = cpu.read8(CF_LUNGES.wrapping_add(rank)) as u32;
            let c = cpu.read16(o + CF_COUNT) as i16 as i32 as u32;
            let a = if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, c, gap * 2)) == 0 { 0xd000 } else { 0x3000 };
            cpu.write16(t + 0x2c, a);
            nested!(cpu, 0x0609_189c, lunge_shot_set(cpu, t, o));
            nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 41));
        }
    }
    let c = cpu.read16(o + CF_COUNT) as i16 as i32;
    if c <= 0x2cf && nested!(cpu, 0x0600_6a6e, math::remainder(cpu, c as u32, 80)) == 0 {
        let c = cpu.read16(o + CF_COUNT) as i16 as i32 as u32;
        let k = if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, c, 160)) == 0 { 0 } else { 1 };
        let (on, part, size, growth, turn, turned, points) = CF_BARRIERS[k];
        if cpu.read16(o + part) & CR_BROKEN == 0 {
            cpu.write8(o + on, 1);
            cpu.write32(o + size, 0);
            cpu.write32(o + turned, 0x8000);
            cpu.write16(o + growth, 0xc00);
            cpu.write16(o + turn, 0xc00);
            static PROLOGUE: std::sync::OnceLock<super::Prologue> = std::sync::OnceLock::new();
            cpu.nested_with_stack(&PROLOGUE, 0x0609_1494, &[0xb0], |cpu| {
                big_charge_set(cpu, o + part, 64, o + points, o + TURN_Z, 0xb0)
            });
        }
    }
    for (k, &(on, part, size, growth, turn, turned, points)) in CF_BARRIERS.iter().enumerate() {
        // the SH-2 code keeps the flag's address and the part's bit in its frame
        cpu.write32(t + 64 + 4 * k as u32, o + on);
        let n = cpu.read8(o + on);
        if n == 0 {
            continue;
        }
        let gone = (cpu.read16(o + part) & CR_BROKEN) as u32;
        cpu.write32(t + 72, gone);
        if gone != 0 || (cpu.read32(o + size) as i32) < 0 {
            cpu.write8(o + on, 0);
            continue;
        }
        let n = n.wrapping_add(1);
        cpu.write8(o + on, n);
        if n == 40 {
            nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 87));
            nested!(cpu, 0x0609_1604, barrier_set(cpu, o + on, o + points, o + size));
            let a = cpu.read16(o + TURN_Z) as u32;
            nested!(cpu, 0x0609_16c8, cross_flash_at(cpu, o + points, 0xb0, a));
            let v = cpu.read32(t + 72);
            cpu.write32(o + turned, v);
        }
        if cpu.read8(o + on) > 39 {
            let s = cpu.read32(o + size).wrapping_add(cpu.read16(o + growth) as i16 as u32);
            cpu.write32(o + size, s);
            let rank = cpu.read8(RANK) as i8 as i32 as u32;
            let slow = cpu.read8(CF_SLOWS.wrapping_add(rank)) as u16;
            let gr = cpu.read16(o + growth).wrapping_sub(slow);
            cpu.write16(o + growth, gr);
            let tu = cpu.read32(o + turned).wrapping_add(cpu.read16(o + turn) as i16 as u32);
            cpu.write32(o + turned, tu);
            let tw = cpu.read16(o + turn);
            if tw as i16 > 0 {
                cpu.write16(o + turn, tw.wrapping_sub(72));
            }
        }
    }
}

/// Step 6: after 120 frames, step 7: the results (results.rs's, in 100 frames), each player's
/// +0xc0 in its play data kept (+0x4c4, +0x4c8), the scroll moved on.
const CR_RESULTS: u32 = 0x4c0;
const CR_KEPT: u32 = 0x4c4;
const PLAY_C0: u32 = 0xc0;
const MODE_SUB: u32 = 0x0600_ebb1;

pub fn carrier_beaten(cpu: &mut Cpu, o: u32) {
    if cpu.read16(o + 0x70) > 120 {
        cpu.write8(o + CARRIER_STEP, 7);
        let r = nested!(cpu, 0x0605_284c, results::results_set(cpu, 100));
        cpu.write32(o + CR_RESULTS, r);
        for k in 0..2 {
            let v = cpu.read32(PLAY_DATA + PLAY_DATA_SIZE * k + PLAY_C0);
            cpu.write32(o + CR_KEPT + 4 * k, v);
        }
        nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
    }
}

/// Step 7: once the results end, it ends too: the mode's sub-step back one, the players' kept
/// values put back, its target's escorts' bit 1 set.
pub fn carrier_results(cpu: &mut Cpu, o: u32) {
    let r = cpu.read32(o + CR_RESULTS);
    if cpu.read8(r + OBJ_MODE) as i8 > ENDING {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        let m = cpu.read8(MODE_SUB).wrapping_sub(1);
        cpu.write8(MODE_SUB, m);
        for k in 0..2 {
            let v = cpu.read32(o + CR_KEPT + 4 * k);
            cpu.write32(PLAY_DATA + PLAY_DATA_SIZE * k + PLAY_C0, v);
        }
        let t = nested!(cpu, 0x0609_20d4, carrier_target(cpu, o));
        let f = cpu.read16(t + CARRIER_ESCORTS) | 2;
        cpu.write16(t + CARRIER_ESCORTS, f);
    }
}

/// Its parts (+0x90 to +0x9c) opened to hits.
pub fn carrier_parts_open(cpu: &mut Cpu, o: u32) {
    for at in (0x90..=0x9c).step_by(2) {
        let f = cpu.read16(o + at) & !0x20;
        cpu.write16(o + at, f);
    }
}

/// Its parts shut to hits.
pub fn carrier_parts_shut(cpu: &mut Cpu, o: u32) {
    for at in (0x90..=0x9c).step_by(2) {
        let f = cpu.read16(o + at) | 0x20;
        cpu.write16(o + at, f);
    }
}

/// Step 10 (the mirror carrier, waiting off the top): with its own marks' bit 1, step 13; with
/// its target's bit 0, step 11: it appears mirroring its target across the middle (224 dots up, 48
/// deep), shown.
pub fn carrier_mirror_waiting(cpu: &mut Cpu, o: u32) {
    let t = nested!(cpu, 0x0609_20d4, carrier_target(cpu, o));
    let marks = cpu.read16(o + CARRIER_ESCORTS) & 2;
    if marks != 0 {
        cpu.write8(o + CARRIER_STEP, 13);
        cpu.write32(o + CR_ROLL, 0);
    } else if cpu.read16(t + CARRIER_ESCORTS) & 1 != 0 {
        cpu.write8(o + CARRIER_STEP, 11);
        let x = cpu.read32(t + POSITION).wrapping_neg();
        cpu.write32(o + POSITION, x);
        cpu.write32(o + POSITION + 4, 0xff20_0000);
        cpu.write32(o + POSITION + 8, 0xffd0_0000);
        let v = cpu.read32(t + CR_SPEED).wrapping_neg();
        cpu.write32(o + CR_SPEED, v);
        let v = cpu.read32(t + CR_SPEED + 4);
        cpu.write32(o + CR_SPEED + 4, v);
        cpu.write32(o + CR_SPEED + 8, marks as u32);
        cpu.write16(o + CR_TURN_Y_SPEED, 0);
        cpu.write32(o + FLAGS, 0xc8_0000);
    }
}

/// Step 11: mirroring its target (its speed and turns the other way, a quarter of its heading),
/// coming down to 80 dots up and scrolling; from 96 dots up, more than 48 dots out, a bomb every
/// 14 frames. When its target's bit 0 clears (or its own is set), step 12, climbing away.
pub fn carrier_mirroring(cpu: &mut Cpu, o: u32) {
    let tg = nested!(cpu, 0x0609_20d4, carrier_target(cpu, o));
    let v = cpu.read32(tg + CR_SPEED).wrapping_neg();
    cpu.write32(o + CR_SPEED, v);
    let v = cpu.read32(tg + CR_SPEED + 4);
    cpu.write32(o + CR_SPEED + 4, v);
    if cpu.read16(tg + CARRIER_ESCORTS) & 1 == 0 || cpu.read16(o + CARRIER_ESCORTS) & 1 != 0 {
        cpu.write8(o + CARRIER_STEP, 12);
        cpu.write32(o + CR_SPEED + 4, 0xffff_2000);
    }
    let y = cpu.read32(o + POSITION + 4);
    let y = nested!(cpu, 0x0604_6168, actors::approach(cpu, y, 0xffb0_0000, 0x1_0000));
    cpu.write32(o + POSITION + 4, y);
    let a = (cpu.read16(tg + TURN_Z) as i16 as i32).wrapping_neg();
    let a = if a < 0 { a + 3 } else { a };
    cpu.write16(o + TURN_Z, ((a as u32) >> 2) as u16);
    let a = cpu.read16(tg + TURN_Y).wrapping_neg();
    cpu.write16(o + TURN_Y, a);
    let y = cpu.read32(o + POSITION + 4).wrapping_add(GROUND_SCROLL);
    cpu.write32(o + POSITION + 4, y);
    if y as i32 > -0x60_0000 && (cpu.read32(o + POSITION) as i32).abs() > 0x30_0000 {
        let c = cpu.read16(o + 0x70) as u32;
        if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, c, 14)) == 0 {
            let t = cpu.stack();
            let n = cpu.read8(o + NUMBER);
            cpu.write8(t + S_NUMBER, n);
            let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x10_0000));
            let x = cpu.read32(o + POSITION).wrapping_add(r);
            cpu.write32(t, x);
            let y = cpu.read32(o + POSITION + 4).wrapping_add(0x10_0000);
            cpu.write32(t + 4, y);
            let z = cpu.read32(o + POSITION + 8);
            cpu.write32(t + 8, z);
            nested!(cpu, 0x0609_1c90, bomb_set(cpu, t));
        }
    }
}

/// Step 12: the mirror climbs away (facing ahead, sliding out to its side and banking), off the
/// top back to step 10, hidden, still; scrolling with the ground.
pub fn carrier_mirror_leaving(cpu: &mut Cpu, o: u32) {
    static PROLOGUE: std::sync::OnceLock<super::Prologue> = std::sync::OnceLock::new();
    cpu.write16(o + CE_HEADING, 0);
    let from = cpu.read16(o + TURN_Z) as u32;
    cpu.nested_with_stack(&PROLOGUE, 0x0605_70d8, &[0x2000, 8, 16], |cpu| {
        aim::turn_toward(cpu, o + CR_TURN_Z_SPEED, o + CR_TURN_Z_EASE, from, 0, 0x2000, 8, 16)
    });
    let (dv, ds): (u32, u16) = if cpu.read32(o + POSITION) as i32 > 0 { (0x400, 0xffd0) } else { (0xffff_fc00, 48) };
    let v = cpu.read32(o + CR_SPEED).wrapping_add(dv);
    cpu.write32(o + CR_SPEED, v);
    let s = cpu.read16(o + CR_TURN_Y_SPEED).wrapping_add(ds);
    cpu.write16(o + CR_TURN_Y_SPEED, s);
    let v = cpu.read32(o + CR_SPEED + 4).wrapping_add(0xffff_e400);
    cpu.write32(o + CR_SPEED + 4, v);
    if (cpu.read32(o + POSITION + 4) as i32) < -0xf0_0000 {
        cpu.write8(o + CARRIER_STEP, 10);
        cpu.write32(o + CR_SPEED, 0);
        cpu.write32(o + CR_SPEED + 4, 0);
        cpu.write32(o + FLAGS, 0x8008_0000);
    }
    let y = cpu.read32(o + POSITION + 4).wrapping_add(GROUND_SCROLL);
    cpu.write32(o + POSITION + 4, y);
}

/// Step 13: a wait (its +0x4d8 counting down), then step 14 (sound 80): the last attack, it
/// rises up from the left, shown, level, its marks' bit 7 set, the boss gauge at 50000.
pub fn carrier_last_waiting(cpu: &mut Cpu, o: u32) {
    let w = cpu.read32(o + CR_ROLL).wrapping_sub(1);
    cpu.write32(o + CR_ROLL, w);
    if (w as i32) < 0 {
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 80));
        cpu.write8(o + CARRIER_STEP, 14);
        cpu.write32(o + FLAGS, 0xc8_0000);
        for at in [TURN_Z, TURN_Y, TURN_X, CR_TURN_Z_SPEED, CR_TURN_Z_EASE, CR_TURN_Y_SPEED, CR_TURN_Y_EASE] {
            cpu.write16(o + at, 0);
        }
        cpu.write32(o + POSITION, 0xffb8_e0f0);
        cpu.write32(o + POSITION + 4, 0xff20_0000);
        cpu.write32(o + POSITION + 8, 0xff90_0000);
        cpu.write32(o + CR_SPEED + 4, 0x5_0000);
        cpu.write32(o + CR_SPEED, 0x1_0000);
        let f = cpu.read16(o + CARRIER_ESCORTS) | 0x80;
        cpu.write16(o + CARRIER_ESCORTS, f);
        nested!(cpu, 0x0604_64d8, hud::boss_gauge_set(cpu, 50000));
    }
}

/// Step 14: it climbs up out of the depths, tilted by its depth, banked and turned by its speed;
/// at 32 dots it levels, stops (sound 89), its parts open to hits, and its script starts
/// (step 15, its pose at +0x360).
const CR_POSE: u32 = 0x360;
const CR_WAIT: u32 = 0x74;
const CR_SCRIPT_LAST: u32 = 0x060a_47aa;

pub fn carrier_last_rising(cpu: &mut Cpu, o: u32) {
    let z = cpu.read32(o + POSITION + 8).wrapping_add(0xffe0_0000);
    let q = nested!(cpu, 0x0600_6a78, math::quotient(cpu, z, 0x280));
    cpu.write16(o + TURN_X, q as u16);
    let b = cpu.read16(o + CR_SPEED + 2).wrapping_neg();
    cpu.write16(o + TURN_Y, b);
    let v = cpu.read32(o + CR_SPEED) as i32;
    cpu.write16(o + TURN_Z, (v / 2) as u16);
    if cpu.read32(o + POSITION + 8) as i32 > 0x1f_ffff {
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 89));
        cpu.write8(o + CARRIER_STEP, 15);
        cpu.write16(o + CR_WAIT, 120);
        nested!(cpu, 0x0605_6628, actors::script_start(cpu, o + CR_POSE, CR_SCRIPT_LAST));
        nested!(cpu, 0x0609_3594, carrier_parts_open(cpu, o));
        cpu.write32(o + CR_SPEED, 0);
        cpu.write32(o + CR_SPEED + 4, 0);
        cpu.write32(o + POSITION + 8, 0x20_0000);
        for at in [TURN_Z, TURN_Y, TURN_X] {
            cpu.write16(o + at, 0);
        }
    } else {
        let v = cpu.read32(o + CR_SPEED);
        let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0, 0x1d0));
        cpu.write32(o + CR_SPEED, v);
        let v = cpu.read32(o + CR_SPEED + 4).wrapping_add(0xffff_f200);
        cpu.write32(o + CR_SPEED + 4, v);
        let z = cpu.read32(o + POSITION + 8).wrapping_add(0x1_0000);
        cpu.write32(o + POSITION + 8, z);
    }
}

/// Step 15: it drifts up the screen, its guard going down to -3; at its script's end, step 16.
pub fn carrier_last_settling(cpu: &mut Cpu, o: u32) {
    let y = cpu.read32(o + POSITION + 4).wrapping_add(0xffff_d000);
    cpu.write32(o + POSITION + 4, y);
    let g = cpu.read8(o + CE_ONCE) as i8 as u32;
    let g = nested!(cpu, 0x0604_6168, actors::approach(cpu, g, 0xffff_fffd, 1));
    cpu.write8(o + CE_ONCE, g as u8);
    if cpu.read16(o + CR_POSE + 0x28) & 0x4000 != 0 {
        cpu.write8(o + CARRIER_STEP, 16);
    }
}

/// The last attack's next action (the plan at 0x060a4a28; 4 restarts it, counted), once its wait
/// is over; each waits 120 frames: 0 a pause; 1 step 17 (its script at 0x060a47f6); 2 step 20,
/// its escorts sent out from its unbroken guns' places (the right one marked by its bit 15, the
/// left by 14); 3 step 19 (its script at 0x060a487c).
const CL_PLAN: u32 = 0x060a_4a28;
const CL_SCRIPT_1: u32 = 0x060a_47f6;
const CL_SCRIPT_3: u32 = 0x060a_487c;

pub fn carrier_last_next(cpu: &mut Cpu, o: u32) {
    let t = cpu.stack();
    if cpu.read16(o + CR_WAIT) != 0 {
        return;
    }
    let at = cpu.read16(o + SS_PLAN_AT) as u32;
    let mut action = cpu.read8(CL_PLAN + at);
    if action == 4 {
        cpu.write16(o + SS_PLAN_AT, 0);
        action = cpu.read8(CL_PLAN);
        let n = cpu.read16(o + CE_LAPS).wrapping_add(1);
        cpu.write16(o + CE_LAPS, n);
    }
    let at = cpu.read16(o + SS_PLAN_AT).wrapping_add(1);
    cpu.write16(o + SS_PLAN_AT, at);
    match action {
        0 => {
            cpu.write16(o + CR_WAIT, 120);
            cpu.write16(o + CF_COUNT, 0);
        }
        1 | 3 => {
            let (step, script) = if action == 1 { (17, CL_SCRIPT_1) } else { (19, CL_SCRIPT_3) };
            cpu.write8(o + CARRIER_STEP, step);
            cpu.write16(o + CR_WAIT, 120);
            cpu.write16(o + CF_COUNT, 0);
            if action == 1 {
                cpu.write32(o + CR_ROLL, 0);
            }
            nested!(cpu, 0x0605_6628, actors::script_start(cpu, o + CR_POSE, script));
        }
        2 => {
            cpu.write8(o + CARRIER_STEP, 20);
            cpu.write16(o + CR_WAIT, 120);
            cpu.write16(o + CF_COUNT, 0xfffc);
            cpu.write32(t + 60, o);
            for (part, mark, gun, side) in [(1, 0x8000u16, CB_GUN_PLACES[1], 0xffffu16), (0, 0x4000, CB_GUN_PLACES[0], 1)] {
                if cpu.read16(o + CR_PARTS[part]) & CR_BROKEN == 0 && cpu.read16(o + CARRIER_ESCORTS) & mark == 0 {
                    let (x, y) = (cpu.read32(o + gun), cpu.read32(o + gun + 4));
                    cpu.write32(t, x);
                    cpu.write32(t + 4, y);
                    cpu.write16(t + 44, side);
                    nested!(cpu, 0x0609_0464, escort_set(cpu, t));
                }
            }
        }
        _ => {}
    }
}

/// The last attack's hover: up and down about 80 dots up (1/2 a dot a frame, eased), turning
/// toward the player (at most 0x480, by 20, 2) — or in step 20, while a barrier is coming up, to
/// its +0x3f6 (0x400) — and swinging 48 dots across by its heading.
const CL_HEIGHT: i32 = -0x50_0000;

pub fn carrier_last_hover(cpu: &mut Cpu, o: u32) {
    static PROLOGUE: std::sync::OnceLock<super::Prologue> = std::sync::OnceLock::new();
    let (to, step) = if (cpu.read32(o + POSITION + 4) as i32) < CL_HEIGHT { (0x8000, 0x4000) } else { (0xffff_8000, 0x6000) };
    let v = cpu.read32(o + CR_SPEED + 4);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, to, step));
    cpu.write32(o + CR_SPEED + 4, v);
    let rising = |cpu: &mut Cpu, at: u32| {
        let n = cpu.read8(o + at);
        n != 0 && n <= 63
    };
    let (to, most) = if cpu.read8(o + CARRIER_STEP) == 20 && (rising(cpu, 0x41f) || rising(cpu, 0x420)) {
        (cpu.read16(o + 0x3f6) as u32, 0x400)
    } else {
        let p = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + 0x10));
        let a = nested!(cpu, 0x0604_5e30, aim::angle_to(cpu, o, p));
        (a.wrapping_add(0xffff_8000) & 0xffff, 0x480)
    };
    let from = cpu.read16(o + TURN_Z) as u32;
    cpu.nested_with_stack(&PROLOGUE, 0x0605_70d8, &[most, 20, 2], |cpu| {
        aim::turn_toward(cpu, o + CR_TURN_Z_SPEED, o + CR_TURN_Z_EASE, from, to, most, 20, 2)
    });
    let a = cpu.read16(o + TURN_Z).wrapping_add(0x8000) as i16 as i32 as u32;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    cpu.write32(o + POSITION, s.wrapping_mul(48));
}

/// Step 16: the last attack's main step. Coming up to 32 dots deep, hovering; its wait runs
/// down; its script restarted (0x060a47d0) at its end; its next action; every 18 frames two
/// lunging shots kept on its heading (behind it when the player is above), fanned wider through
/// 288 frames (sound 86).
const CL_SCRIPT_16: u32 = 0x060a_47d0;

pub fn carrier_last_main(cpu: &mut Cpu, o: u32) {
    let t = cpu.stack();
    let z = cpu.read32(o + POSITION + 8);
    let z = nested!(cpu, 0x0604_6168, actors::approach(cpu, z, 0x20_0000, 0x4000));
    cpu.write32(o + POSITION + 8, z);
    nested!(cpu, 0x0609_3c28, carrier_last_hover(cpu, o));
    let w = cpu.read16(o + CR_WAIT) as i16 as u32;
    let w = nested!(cpu, 0x0604_6168, actors::approach(cpu, w, 0, 1));
    cpu.write16(o + CR_WAIT, w as u16);
    if cpu.read16(o + CR_POSE + 0x28) & 0x4000 != 0 {
        nested!(cpu, 0x0605_6628, actors::script_start(cpu, o + CR_POSE, CL_SCRIPT_16));
    }
    nested!(cpu, 0x0609_3abc, carrier_last_next(cpu, o));
    if cpu.read8(o + CARRIER_STEP) != 16 {
        return;
    }
    let c = cpu.read16(o + CF_COUNT).wrapping_add(1);
    cpu.write16(o + CF_COUNT, c);
    let c = cpu.read16(o + CF_COUNT) as i16 as i32 as u32;
    if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, c, 18)) != 0 {
        return;
    }
    cpu.write32(t + 20, 1);
    let p = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + 0x10));
    let ahead = (cpu.read32(p + POSITION + 4) as i32) >= cpu.read32(o + POSITION + 4) as i32;
    let z = cpu.read16(o + TURN_Z);
    cpu.write16(t + 0x2e, if ahead { z.wrapping_add(0x8000) } else { z });
    cpu.write16(t + 0x30, 6);
    let c = cpu.read16(o + CF_COUNT) as i16 as i32 as u32;
    let m = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, c, 0x120)) as i32;
    let spread = ((m / 18) << 12) as u16;
    cpu.write16(t + 0x32, spread);
    let a = cpu.read16(o + TURN_Z).wrapping_add(0xc000).wrapping_sub(spread);
    cpu.write16(t + 0x2c, a);
    nested!(cpu, 0x0609_189c, lunge_shot_set(cpu, t, o));
    let a = cpu.read16(o + TURN_Z).wrapping_add(0x4000).wrapping_add(cpu.read16(t + 0x32));
    cpu.write16(t + 0x2c, a);
    nested!(cpu, 0x0609_189c, lunge_shot_set(cpu, t, o));
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 86));
}

/// Step 17: hovering; at its script's end, step 18 (its script at 0x060a480a).
const CL_SCRIPT_18: u32 = 0x060a_480a;

pub fn carrier_last_turning(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0609_3c28, carrier_last_hover(cpu, o));
    if cpu.read16(o + CR_POSE + 0x28) & 0x4000 != 0 {
        cpu.write8(o + CARRIER_STEP, 18);
        nested!(cpu, 0x0605_6628, actors::script_start(cpu, o + CR_POSE, CL_SCRIPT_18));
    }
}

/// Step 18: the barrage. Hovering; once both pods are broken or after 800 frames, back to step
/// 16 (its script at 0x060a47d0). Every so many frames by the rank (the bytes at 0x060a48b8),
/// from each unbroken pod (sound 85) spark shots along the pod's joint (by turns one, or a pair
/// 7/256 of a turn apart); with each beat of its script the volley count (+0x4d8) restarts:
/// lunging shots kept on its heading fanned out at counts 0 and 15, 5 and 20 (from rank 2), 10
/// and 25 (from rank 1).
const CB_SPARKS: u32 = 0x060a_48b8;
const CB_JOINTS: u32 = 0x374;

pub fn carrier_barrage(cpu: &mut Cpu, o: u32) {
    let t = cpu.stack();
    nested!(cpu, 0x0609_3c28, carrier_last_hover(cpu, o));
    let c = cpu.read16(o + CF_COUNT).wrapping_add(1);
    cpu.write16(o + CF_COUNT, c);
    let broken = |cpu: &mut Cpu, k: usize| cpu.read16(o + CR_PARTS[k]) & CR_BROKEN != 0;
    if (broken(cpu, 2) && broken(cpu, 3)) || c as i16 > 800 {
        cpu.write8(o + CARRIER_STEP, 16);
        nested!(cpu, 0x0605_6628, actors::script_start(cpu, o + CR_POSE, CL_SCRIPT_16));
        return;
    }
    let gap = |cpu: &mut Cpu| {
        let rank = cpu.read8(RANK) as i8 as i32 as u32;
        cpu.read8(CB_SPARKS.wrapping_add(rank)) as u32
    };
    let g = gap(cpu);
    let c = cpu.read16(o + CF_COUNT) as i16 as i32 as u32;
    if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, c, g)) == 0 {
        let n = cpu.read8(o + NUMBER);
        cpu.write8(t + S_NUMBER, n);
        let pair = |cpu: &mut Cpu| {
            let a = cpu.read16(t + 44).wrapping_add(0x700);
            cpu.write16(t + 44, a);
            nested!(cpu, 0x0609_0e60, spark_shot_set(cpu, t));
            let a = cpu.read16(t + 44).wrapping_add(0xf200);
            cpu.write16(t + 44, a);
            nested!(cpu, 0x0609_0e60, spark_shot_set(cpu, t));
        };
        for (part, pod, joint, single_on_zero) in [(2usize, CR_POD_A, 16u32, false), (3, CR_POD_B, 20, true)] {
            if broken(cpu, part) {
                continue;
            }
            let (x, y) = (cpu.read32(o + pod), cpu.read32(o + pod + 4));
            cpu.write32(t, x);
            cpu.write32(t + 4, y);
            let joints = cpu.read32(o + CB_JOINTS);
            let j = cpu.read16(joints + joint).wrapping_add(0x4000);
            let a = if part == 2 { j.wrapping_add(cpu.read16(o + TURN_Z)) } else { cpu.read16(o + TURN_Z).wrapping_sub(j) };
            cpu.write16(t + 44, a);
            nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 85));
            let g = gap(cpu);
            let c = cpu.read16(o + CF_COUNT) as i16 as i32 as u32;
            let zero = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, c, g * 2)) == 0;
            if zero == single_on_zero {
                nested!(cpu, 0x0609_0e60, spark_shot_set(cpu, t));
            } else {
                pair(cpu);
            }
        }
    }
    if cpu.read16(o + CR_POSE + 0x28) & 0x2000 != 0 {
        cpu.write32(o + CR_ROLL, 0);
    }
    cpu.write32(t + 20, 0);
    let fan = |cpu: &mut Cpu, speed: u16, a: u16, b: u16| {
        cpu.write16(t + 0x30, speed);
        for turn in [a, b] {
            let h = cpu.read16(o + TURN_Z).wrapping_add(turn);
            cpu.write16(t + 0x2c, h);
            nested!(cpu, 0x0609_189c, lunge_shot_set(cpu, t, o));
        }
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 41));
    };
    let k = cpu.read32(o + CR_ROLL);
    if k == 0 || k == 15 {
        fan(cpu, 6, 0xb000, 0x5000);
    }
    let k = cpu.read32(o + CR_ROLL);
    if (k == 5 || k == 20) && cpu.read8(RANK) as i8 > 1 {
        fan(cpu, 10, 0xc000, 0x4000);
    }
    let k = cpu.read32(o + CR_ROLL);
    if (k == 10 || k == 25) && cpu.read8(RANK) as i8 > 0 {
        fan(cpu, 6, 0xd000, 0x3000);
    }
    let k = cpu.read32(o + CR_ROLL).wrapping_add(1);
    cpu.write32(o + CR_ROLL, k);
}

/// Step 19: the pods. Coming up to 12 dots deep, hovering, its script restarted (0x060a488a) at
/// its end; after 800 frames back to step 16 (0x060a47d0). Level, every so many frames by the
/// rank (the bytes at 0x060a48b0; one more once all four of its guns and pods are broken) a
/// spinning pod from its gun along its heading, hidden on odd or even frames by turns of 8.
const CP_PODS: u32 = 0x060a_48b0;
const CP_SCRIPT: u32 = 0x060a_488a;

pub fn carrier_pods(cpu: &mut Cpu, o: u32) {
    let t = cpu.stack();
    let z = cpu.read32(o + POSITION + 8);
    let z = nested!(cpu, 0x0604_6168, actors::approach(cpu, z, 0xc_0000, 0x4000));
    cpu.write32(o + POSITION + 8, z);
    if cpu.read16(o + CR_POSE + 0x28) & 0x4000 != 0 {
        nested!(cpu, 0x0605_6628, actors::script_start(cpu, o + CR_POSE, CP_SCRIPT));
    }
    nested!(cpu, 0x0609_3c28, carrier_last_hover(cpu, o));
    let c = cpu.read16(o + CF_COUNT).wrapping_add(1);
    cpu.write16(o + CF_COUNT, c);
    if c as i16 > 800 {
        cpu.write8(o + CARRIER_STEP, 16);
        nested!(cpu, 0x0605_6628, actors::script_start(cpu, o + CR_POSE, CL_SCRIPT_16));
        return;
    }
    let all = [0x92, 0x94, 0x9a, 0x9c].iter().all(|&at| cpu.read16(o + at) & CR_BROKEN != 0);
    cpu.write16(t + 44, all as u16);
    if cpu.read32(o + POSITION + 8) == 0xc_0000 {
        let rank = cpu.read8(RANK) as i8 as i32 as u32;
        let gap = (cpu.read8(CP_PODS.wrapping_add(rank)) as u32).wrapping_add(cpu.read16(t + 44) as i16 as u32);
        let c = cpu.read16(o + CF_COUNT) as i16 as i32 as u32;
        if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, c, gap)) == 0 {
            let n = cpu.read8(o + NUMBER);
            cpu.write8(t + S_NUMBER, n);
            let (x, y) = (cpu.read32(o + CE_GUN), cpu.read32(o + CE_GUN + 4));
            cpu.write32(t, x);
            cpu.write32(t + 4, y);
            let a = cpu.read16(o + TURN_Z);
            cpu.write16(t + 24, a);
            let hidden = ((cpu.read32(GAME_FRAMES) & 8) >> 3) as u16;
            cpu.write16(t + 44, hidden);
            nested!(cpu, 0x0609_08b0, spinner_pod_set(cpu, t));
        }
    }
}

/// Step 20: the barriers. Hovering; its count runs (held before frame 320 while it faces more than
/// 0x1bff away from the nearer player with no barrier coming up). Once its guns are both broken,
/// past frame 480, or the player is above it, it goes back to step 16 when no barrier is up. To
/// frame 399, every 80 frames a big charge (colours 0xc0) at a gun not broken, by turns, its
/// heading kept (+0x3f6), with its script (0x060a4856 or 0x060a4830); 40 frames on the barrier
/// is thrown up (as in step 5) and it swings round against it.
const CB2_SCRIPTS: [u32; 2] = [0x060a_4856, 0x060a_4830];
const CB2_SWINGS: [(u16, u16); 2] = [(0xfe80, 0xffc0), (0x180, 64)];

pub fn carrier_barriers(cpu: &mut Cpu, o: u32) {
    let t = cpu.stack();
    nested!(cpu, 0x0609_3c28, carrier_last_hover(cpu, o));
    let p = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + 0x10));
    let a = nested!(cpu, 0x0604_5e30, aim::angle_to(cpu, o, p)) & 0xffff;
    let h = cpu.read16(o + TURN_Z).wrapping_add(0x8000) as u32;
    let d = nested!(cpu, 0x0605_6580, aim::angle_between(cpu, h, a)) as i16 as i32;
    let rising = |cpu: &mut Cpu, at: u32| {
        let n = cpu.read8(o + at);
        n != 0 && n <= 39
    };
    let held = d.abs() > 0x1bff && !rising(cpu, 0x41f) && !rising(cpu, 0x420) && cpu.read16(o + CF_COUNT) as i16 <= 0x13f;
    if !held {
        let c = cpu.read16(o + CF_COUNT).wrapping_add(1);
        cpu.write16(o + CF_COUNT, c);
    }
    let broken = |cpu: &mut Cpu, at: u32| cpu.read16(o + at) & CR_BROKEN != 0;
    let leave = (broken(cpu, 0x92) && broken(cpu, 0x94))
        || cpu.read16(o + CF_COUNT) as i16 > 0x1e0
        || (cpu.read32(p + POSITION + 4) as i32) < cpu.read32(o + POSITION + 4) as i32;
    if leave && cpu.read8(o + 0x41f) == 0 && cpu.read8(o + 0x420) == 0 {
        cpu.write8(o + CARRIER_STEP, 16);
        return;
    }
    let c = cpu.read16(o + CF_COUNT) as i16 as i32;
    if c <= 0x18f && nested!(cpu, 0x0600_6a6e, math::remainder(cpu, c as u32, 80)) == 0 {
        let c = cpu.read16(o + CF_COUNT) as i16 as i32 as u32;
        let k = if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, c, 160)) == 0 { 0 } else { 1 };
        let (on, part, size, growth, turn, turned, points) = CF_BARRIERS[k];
        if cpu.read16(o + part) & CR_BROKEN == 0 {
            cpu.write8(o + on, 1);
            let z = cpu.read16(o + TURN_Z);
            cpu.write16(o + 0x3f6, z);
            cpu.write32(o + size, 0);
            cpu.write32(o + turned, 0x8000);
            cpu.write16(o + growth, 0xc00);
            cpu.write16(o + turn, 0xc00);
            cpu.push(&[0xc0]);
            nested!(cpu, 0x0609_1494, big_charge_set(cpu, o + part, 64, o + points, o + TURN_Z, 0xc0));
            nested!(cpu, 0x0605_6628, actors::script_start(cpu, o + CR_POSE, CB2_SCRIPTS[k]));
            cpu.pop(4);
        }
    }
    for (k, &(on, part, size, growth, turn, turned, points)) in CF_BARRIERS.iter().enumerate() {
        // the SH-2 code keeps the flag's address in its frame
        cpu.write32(t + 4 * k as u32, o + on);
        let n = cpu.read8(o + on);
        if n == 0 {
            continue;
        }
        let gone = (cpu.read16(o + part) & CR_BROKEN) as u32;
        if gone != 0 || (cpu.read32(o + size) as i32) < 0 {
            cpu.write8(o + on, 0);
            continue;
        }
        let n = n.wrapping_add(1);
        cpu.write8(o + on, n);
        if n == 40 {
            nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 87));
            nested!(cpu, 0x0609_1604, barrier_set(cpu, o + on, o + points, o + size));
            let a = cpu.read16(o + TURN_Z) as u32;
            nested!(cpu, 0x0609_16c8, cross_flash_at(cpu, o + points, 0xc0, a));
            cpu.write32(o + turned, gone);
            cpu.write16(o + CR_TURN_Z_SPEED, CB2_SWINGS[k].0);
            cpu.write16(o + CR_TURN_Z_EASE, CB2_SWINGS[k].1);
        }
        if cpu.read8(o + on) > 39 {
            let s = cpu.read32(o + size).wrapping_add(cpu.read16(o + growth) as i16 as u32);
            cpu.write32(o + size, s);
            let rank = cpu.read8(RANK) as i8 as i32 as u32;
            let slow = cpu.read8(CF_SLOWS.wrapping_add(rank)) as u16;
            let gr = cpu.read16(o + growth).wrapping_sub(slow);
            cpu.write16(o + growth, gr);
            let tu = cpu.read32(o + turned).wrapping_add(cpu.read16(o + turn) as i16 as u32);
            cpu.write32(o + turned, tu);
            let tw = cpu.read16(o + turn);
            if tw as i16 > 0 {
                cpu.write16(o + turn, tw.wrapping_sub(72));
            }
        }
    }
}

/// Step 21: beaten. It tips over and turns, slowing, an explosion at frame 60, sinking away
/// into the distance; gone after 210 frames.
pub fn carrier_sinking(cpu: &mut Cpu, o: u32) {
    for (at, to, step) in [(0x84u32, 0xffff_f400u32, 8u32), (CR_TURN_Z_SPEED, 0x200, 1)] {
        let v = cpu.read16(o + at) as i16 as u32;
        let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, to, step));
        cpu.write16(o + at, v as u16);
    }
    for k in [0, 4] {
        let v = cpu.read32(o + CR_SPEED + k);
        let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0, 0x1000));
        cpu.write32(o + CR_SPEED + k, v);
    }
    if cpu.read16(o + 0x70) == 60 {
        let t = cpu.stack();
        for k in [0, 4, 8] {
            let v = cpu.read32(o + POSITION + k);
            cpu.write32(t + k, v);
        }
        cpu.write32(t + 60, o);
        nested!(cpu, 0x0605_8ba4, explosions::explosion_set(cpu, t));
    }
    let z = cpu.read32(o + POSITION + 8).wrapping_add(cpu.read32(o + CR_SPEED + 8));
    cpu.write32(o + POSITION + 8, z);
    let v = cpu.read32(o + CR_SPEED + 8);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0x3_0000, 0x80));
    cpu.write32(o + CR_SPEED + 8, v);
    if cpu.read16(o + 0x70) > 0xd2 {
        cpu.write8(o + OBJ_MODE, TO_STOP);
    }
}

/// The carrier's behaviour. Its life on the boss gauge once shown (its marks' bit 7). Beaten
/// (its life gone, or past its laps: one, or two for the last attack) before its beaten step (6,
/// or 21 for the last attack): hidden; unless leaving, its target marked (+0x6e = 1) and the
/// scroll moved on; a smoke burst at its place; its beaten step; the bullets cleared; its core
/// broken and shut; the stage's score (20000 and rate 40 if shot down) and rate (10 for each part
/// broken); every life cleared. Each part whose life is gone is broken; its step's function; its
/// pose; it moves and turns; broken parts smoke and spark at their points; counted; its escorts'
/// marks cleared for the next frame.
const CR_LIFE: u32 = 0xa8;
const CR_SMOKE: u32 = 0x9f;
const CR_STATES: u32 = 0x060a_4a30;
/// Each part's smoke: its life, its flags, its points, how many points (past the first).
const CR_SMOKES: [(u32, u32, u32, u32); 6] =
    [(0xac, 0x92, 0x118, 1), (0xb0, 0x94, 0x130, 1), (0xb4, 0x96, 0x148, 11), (0xb8, 0x98, 0x1d8, 11), (0xbc, 0x9a, 0x268, 1), (0xc0, 0x9c, 0x280, 1)];

pub fn carrier_behaviour(cpu: &mut Cpu, o: u32) {
    let t = cpu.stack();
    let target = nested!(cpu, 0x0609_20d4, carrier_target(cpu, o));
    let leaving = cpu.read8(o + CR_LEAVING) != 0;
    let (beaten, laps): (u8, i16) = if leaving { (21, 2) } else { (6, 1) };
    if cpu.read16(o + CARRIER_ESCORTS) & 0x80 != 0 {
        let life = cpu.read32(o + CR_LIFE);
        nested!(cpu, 0x0604_64b8, hud::boss_gauge_life(cpu, life));
    }
    if cpu.read8(o + CARRIER_STEP) < beaten
        && ((cpu.read32(o + CR_LIFE) as i32) <= 0 || cpu.read16(o + CE_LAPS) as i16 > laps)
    {
        cpu.write32(o + FLAGS, 0x8008_0000);
        if cpu.read8(o + CR_LEAVING) == 0 {
            cpu.write16(target + CARRIER_ESCORTS, 1);
            nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
        }
        for k in [0, 4, 8] {
            let v = cpu.read32(o + POSITION + k);
            cpu.write32(t + k, v);
        }
        nested!(cpu, 0x0605_7754, explosions::smoke_burst(cpu, t));
        cpu.write8(o + CARRIER_STEP, beaten);
        nested!(cpu, 0x0605_61e0, screen::bullets_cleared(cpu));
        let f = cpu.read16(o + 0x90) | 0xc0;
        cpu.write16(o + 0x90, f);
        cpu.write16(o + 0x70, 0);
        cpu.write32(STAGE_RATE_AT, 0);
        if (cpu.read32(o + CR_LIFE) as i32) <= 0 {
            cpu.write32(STAGE_SCORE_AT, 20000);
            cpu.write32(STAGE_RATE_AT, 40);
        } else {
            cpu.write32(STAGE_SCORE_AT, 0);
        }
        for &(life, _, _, _) in &CR_SMOKES {
            if (cpu.read32(o + life) as i32) <= 0 {
                let r = cpu.read32(STAGE_RATE_AT).wrapping_add(10);
                cpu.write32(STAGE_RATE_AT, r);
            }
        }
        for at in (CR_LIFE..=0xc0).step_by(4) {
            cpu.write32(o + at, 0);
        }
    }
    for (k, &(life, flags, _, _)) in CR_SMOKES.iter().enumerate() {
        let at = o + CR_SMOKE + k as u32;
        if cpu.read8(at) == 0 && (cpu.read32(o + life) as i32) <= 0 {
            cpu.write8(at, 1);
            let f = cpu.read16(o + flags) | CR_BROKEN;
            cpu.write16(o + flags, f);
        }
    }
    let step = cpu.read8(o + CARRIER_STEP) as u32;
    match step {
        0 | 8 => nested!(cpu, 0x0609_29b0, carrier_rising(cpu, o)),
        1 | 9 => nested!(cpu, 0x0609_2ac0, carrier_swinging(cpu, o)),
        2 => nested!(cpu, 0x0609_2b94, carrier_settling(cpu, o)),
        3 => nested!(cpu, 0x0609_2c0c, carrier_guarding(cpu, o)),
        4 => nested!(cpu, 0x0609_2fcc, carrier_diving(cpu, o)),
        5 => nested!(cpu, 0x0609_30bc, carrier_fighting(cpu, o)),
        6 => nested!(cpu, 0x0609_34b4, carrier_beaten(cpu, o)),
        7 => nested!(cpu, 0x0609_3524, carrier_results(cpu, o)),
        10 => nested!(cpu, 0x0609_3654, carrier_mirror_waiting(cpu, o)),
        11 => nested!(cpu, 0x0609_36ec, carrier_mirroring(cpu, o)),
        12 => nested!(cpu, 0x0609_3804, carrier_mirror_leaving(cpu, o)),
        13 => nested!(cpu, 0x0609_38c0, carrier_last_waiting(cpu, o)),
        14 => nested!(cpu, 0x0609_3974, carrier_last_rising(cpu, o)),
        15 => nested!(cpu, 0x0609_3a68, carrier_last_settling(cpu, o)),
        16 => nested!(cpu, 0x0609_3d34, carrier_last_main(cpu, o)),
        17 => nested!(cpu, 0x0609_3e88, carrier_last_turning(cpu, o)),
        18 => nested!(cpu, 0x0609_3ed4, carrier_barrage(cpu, o)),
        19 => nested!(cpu, 0x0609_41b4, carrier_pods(cpu, o)),
        20 => nested!(cpu, 0x0609_42fc, carrier_barriers(cpu, o)),
        21 => nested!(cpu, 0x0609_46ac, carrier_sinking(cpu, o)),
        _ => {
            let f = cpu.read32(CR_STATES + 4 * step);
            cpu.call(f, &[o]);
        }
    }
    nested!(cpu, 0x0605_6644, models::pose_step(cpu, o + CR_POSE));
    for k in [0, 4, 8] {
        let p = cpu.read32(o + POSITION + k).wrapping_add(cpu.read32(o + CR_SPEED + k));
        cpu.write32(o + POSITION + k, p);
    }
    for (turn, speed) in [(TURN_X, 0x84), (TURN_Y, CR_TURN_Y_SPEED), (TURN_Z, CR_TURN_Z_SPEED)] {
        let a = cpu.read16(o + turn).wrapping_add(cpu.read16(o + speed));
        cpu.write16(o + turn, a);
    }
    for (k, &(_, _, points, last)) in CR_SMOKES.iter().enumerate() {
        let at = o + CR_SMOKE + k as u32;
        let s = cpu.read8(at) as u32;
        if s != 0 && s - 1 <= last {
            let p = (s - 1) * 12 + points + o;
            let made = nested!(cpu, 0x0605_968c, explosions::smoke_and_sparks(cpu, p, 8));
            if made as u8 != 0 {
                let s = cpu.read8(at).wrapping_add(1);
                cpu.write8(at, s);
            }
        }
    }
    let c = cpu.read16(o + 0x70).wrapping_add(1);
    cpu.write16(o + 0x70, c);
    let m = cpu.read16(o + CARRIER_ESCORTS) & 0x3fff;
    cpu.write16(o + CARRIER_ESCORTS, m);
}

/// The carrier's draw (`s` its sprite part): kind 56's models (the carrier's set, or the last
/// attack's 17 on), by its pose (the joints at +0x374). Its parts' hits drawn; while a barrier
/// is up, its colours flash (kind 1). The hull and its gun, its two arms (lifted by its guard,
/// +0x3f4) with the barrier generators (each barrier turning, sized, its 4 points and 4 at random
/// along it), its two engines (their guns, swung by its bank), its two pods, its two side guns
/// (their muzzles flickering by the loop) and its tail; then, while a barrier is up, glows at its
/// points (kind 0's frames 61 to 76 at random); and while its core is shown, two engine glows
/// flickering by the loop.
const CD_KIND: u32 = 56;
const CD_FLASH_KIND: u32 = 1;
const CD_HULL_POINTS: (u32, u32) = (0x060a_4338, 7);
const CD_ARM_POINTS: (u32, u32) = (0x060a_4370, 2);
const CD_ENGINE_A_POINTS: (u32, u32) = (0x060a_4380, 12);
const CD_ENGINE_B_POINTS: (u32, u32) = (0x060a_43e0, 12);
const CD_POD_POINTS: (u32, u32) = (0x060a_4440, 2);
const CD_SIDE_A_POINTS: (u32, u32) = (0x060a_4450, 7);
const CD_SIDE_B_POINTS: (u32, u32) = (0x060a_4480, 7);

pub fn carrier_draw(cpu: &mut Cpu, s: u32) {
    let o = s - 0x10;
    let t = cpu.stack();
    if cpu.read8(o + CR_LEAVING) != 0 {
        cpu.write32(t + 140, 0);
        cpu.write32(t + 144, 0);
    } else {
        cpu.write32(t + 140, 17);
        let g = cpu.read8(o + CE_ONCE) as i8 as i32 as u32;
        cpu.write32(t + 144, g << 16);
    }
    for k in 0..7 {
        hit_flags_drawn(cpu, o + 0x90 + 2 * k);
    }
    if cpu.read8(o + 0x41f) != 0 || cpu.read8(o + 0x420) != 0 {
        let leaving = cpu.read8(o + CR_LEAVING) != 0;
        let c = if cpu.read32(LOOPS) & 7 != 0 {
            if leaving { 0xc0 } else { 0xb0 }
        } else if leaving {
            48
        } else {
            0
        };
        let c = cpu.read16(CHAR_BASE_0).wrapping_add(c) as u32;
        nested!(cpu, 0x0604_5b1c, models::model_colour(cpu, 1, 0, c));
    }
    let bank = |cpu: &mut Cpu| cpu.read32(t + 140);
    let put = |cpu: &mut Cpu, n: u32, f: Option<u16>| {
        let b = bank(cpu);
        put_kind_model(cpu, CD_KIND, b + n, f);
    };
    let joint = |cpu: &mut Cpu, k: u32| -> u32 {
        let j = cpu.read32(o + CB_JOINTS);
        cpu.read16(j + 2 * k) as i16 as i32 as u32
    };
    let neg = |v: u32| (v.wrapping_neg()) as i16 as i32 as u32;
    let lift = |cpu: &mut Cpu| (cpu.read8(o + CE_ONCE) as i8 as i32 as u32) << 16;
    cpu.call(SL_PUSH_MATRIX, &[]);
    let p = [0, 4, 8].map(|k| cpu.read32(o + POSITION + k));
    cpu.call(SL_TRANSLATE, &p);
    cpu.call(SL_PUSH_MATRIX, &[]);
    let a = cpu.read16(o + TURN_Z) as i16 as i32 as u32;
    cpu.call(SL_ROT_Z, &[a]);
    let a = cpu.read16(o + TURN_X).wrapping_add(joint(cpu, 1) as u16) as i16 as i32 as u32;
    cpu.call(SL_ROT_X, &[a]);
    let a = cpu.read16(o + TURN_Y).wrapping_add(joint(cpu, 0) as u16) as i16 as i32 as u32;
    cpu.call(SL_ROT_Y, &[a]);
    cpu.call(SL_TRANSLATE, &[0, 0xffe0_0000, 0xfff0_0000]);
    gs_points(cpu, CD_HULL_POINTS, o + 0xc4);
    let f = cpu.read16(o + 0x90);
    if f & 0x80 == 0 {
        put(cpu, 2, Some(f));
        cpu.call(SL_CALC_POINT, &[0, 0, 0, o + CE_GUN]);
    }
    // the arms with their barrier generators
    for side in 0..2u32 {
        cpu.call(SL_PUSH_MATRIX, &[]);
        let j6 = joint(cpu, 6);
        let x = if side == 0 { lift(cpu).wrapping_add(0x10_0000) } else { 0xfff0_0000u32.wrapping_sub(lift(cpu)) };
        cpu.call(SL_TRANSLATE, &[x, j6.wrapping_mul(32), j6 << 6]);
        cpu.call(SL_PUSH_MATRIX, &[]);
        let a = joint(cpu, 3);
        cpu.call(SL_ROT_X, &[a]);
        let a = if side == 0 { joint(cpu, 2) } else { neg(joint(cpu, 2)) };
        cpu.call(SL_ROT_Y, &[a]);
        cpu.call(SL_TRANSLATE, &[0, 0xfff0_0000, 0]);
        gs_points(cpu, CD_ARM_POINTS, o + if side == 0 { 0x118 } else { 0x130 });
        let part = if side == 0 { 0x92 } else { 0x94 };
        let f = cpu.read16(o + part);
        if f & 0x80 == 0 {
            if f & 0x40 == 0 {
                put(cpu, 1 - side, Some(f));
            } else {
                put(cpu, if side == 0 { 13 } else { 12 }, None);
            }
            let (on, size, turned, points, spin, at) = if side == 0 {
                (0x41f, 0x4d8, 0x4e0, 0x424, 0xffff_f7d0u32, 44)
            } else {
                (0x420, 0x4dc, 0x4e4, 0x454, 0x830, 92)
            };
            if cpu.read8(o + on) != 0 {
                cpu.call(SL_PUSH_MATRIX, &[]);
                let a = cpu.read32(LOOPS).wrapping_mul(spin) as i16 as i32 as u32;
                cpu.call(SL_ROT_Y, &[a]);
                let (w, h) = (cpu.read32(o + size), cpu.read32(o + turned));
                cpu.call(SL_SCALE, &[w, h, w]);
                cpu.call(SL_TRANSLATE, &[0, 0xfff4_0000, 0]);
                put_kind_model(cpu, CD_FLASH_KIND, 0, None);
                let y0 = if cpu.read32(GAME_FRAMES) & 1 != 0 && cpu.read8(o + on) > 40 { 0xffd0_0000 } else { 0xffec_0000u32 };
                for i in 0..4u32 {
                    cpu.call(SL_CALC_POINT, &[0, i.wrapping_mul(0xffc8_0000).wrapping_add(y0), 0, o + points + 12 * i]);
                }
                for i in 0..4u32 {
                    let r = cpu.read32(o + turned).wrapping_mul(0x160);
                    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, r)) as i32;
                    cpu.call(SL_CALC_POINT, &[0, (r.abs()).wrapping_neg() as u32, 0, t + at + 12 * i]);
                }
                cpu.call(SL_POP_MATRIX, &[]);
            }
        }
        cpu.call(SL_POP_MATRIX, &[]);
        cpu.call(SL_POP_MATRIX, &[]);
    }
    // the engines with their guns and pods
    for side in 0..2u32 {
        cpu.call(SL_PUSH_MATRIX, &[]);
        let j6 = joint(cpu, 6);
        let lifted = cpu.read32(t + 144);
        let x = if side == 0 {
            (j6 << 8).wrapping_add(lifted.wrapping_add(0x10_0000))
        } else {
            0xfff0_0000u32.wrapping_sub(lifted).wrapping_sub(j6 << 8)
        };
        let y = 0x8_0000u32.wrapping_sub((j6 as i16 as i32 * 96) as u32);
        cpu.call(SL_TRANSLATE, &[x, y, 0]);
        cpu.call(SL_PUSH_MATRIX, &[]);
        cpu.call(SL_TRANSLATE, &[0, 0, 0]);
        let (a, b) = if side == 0 { (joint(cpu, 5), joint(cpu, 4)) } else { (neg(joint(cpu, 5)), neg(joint(cpu, 4))) };
        cpu.call(SL_ROT_Z, &[a]);
        cpu.call(SL_ROT_Y, &[b]);
        gs_points(cpu, if side == 0 { CD_ENGINE_A_POINTS } else { CD_ENGINE_B_POINTS }, o + if side == 0 { 0x148 } else { 0x1d8 });
        let part = if side == 0 { 0x96 } else { 0x98 };
        let f = cpu.read16(o + part);
        if f & 0x80 == 0 {
            if f & 0x40 != 0 {
                put(cpu, if side == 0 { 15 } else { 14 }, None);
            } else {
                put(cpu, if side == 0 { 4 } else { 3 }, Some(f));
                let gx = if side == 0 { 0x8_0000 } else { 0xfff8_0000 };
                cpu.call(SL_CALC_POINT, &[gx, 0x40_0000, 0, o + CB_GUN_PLACES[side as usize]]);
                cpu.call(SL_PUSH_MATRIX, &[]);
                cpu.call(SL_TRANSLATE, &[if side == 0 { 0x14_0000 } else { 0xffec_0000 }, 0x30_0000, 0x8_0000]);
                let bank_y = cpu.read16(o + TURN_Y) as i16;
                if (side == 0 && bank_y > 0) || (side == 1 && bank_y < 0) {
                    cpu.call(SL_ROT_Y, &[bank_y as i32 as u32]);
                }
                let f = cpu.read16(o + part);
                put(cpu, if side == 0 { 6 } else { 5 }, Some(f));
                cpu.call(SL_POP_MATRIX, &[]);
            }
        }
        cpu.call(SL_PUSH_MATRIX, &[]);
        let j7 = joint(cpu, 7);
        let y = (j7 << 8).wrapping_add(0xe_0000);
        let x = if side == 0 {
            (j7 << 6).wrapping_sub(cpu.read32(t + 144).wrapping_add(0x4_0000))
        } else {
            let v = cpu.read32(t + 144).wrapping_add(0x4_0000);
            cpu.write32(t + 144, v);
            let v = v.wrapping_sub(j7 << 6);
            cpu.write32(t + 144, v);
            v
        };
        cpu.call(SL_TRANSLATE, &[x, y, 0]);
        cpu.call(SL_PUSH_MATRIX, &[]);
        if side == 0 {
            let a = joint(cpu, 8);
            cpu.call(SL_ROT_Z, &[a]);
            let a = joint(cpu, 9);
            cpu.call(SL_ROT_X, &[a]);
        } else {
            let a = neg(joint(cpu, 10));
            cpu.call(SL_ROT_Z, &[a]);
            let a = joint(cpu, 11);
            cpu.call(SL_ROT_X, &[a]);
        }
        cpu.call(SL_TRANSLATE, &[0, 0xa_0000, 0]);
        gs_points(cpu, CD_POD_POINTS, o + if side == 0 { 0x268 } else { 0x280 });
        let part = if side == 0 { 0x9a } else { 0x9c };
        let f = cpu.read16(o + part);
        if f & 0xc0 == 0 {
            put(cpu, if side == 0 { 8 } else { 7 }, Some(f));
            cpu.call(SL_CALC_POINT, &[0, 0x8_0000, 0, o + if side == 0 { CR_POD_A } else { CR_POD_B }]);
        }
        for _ in 0..4 {
            cpu.call(SL_POP_MATRIX, &[]);
        }
    }
    // the side guns
    for side in 0..2u32 {
        cpu.call(SL_PUSH_MATRIX, &[]);
        let j6 = joint(cpu, 6);
        let j12 = joint(cpu, 12);
        let d = j6.wrapping_mul(32);
        let x = if side == 0 { d.wrapping_add(0x13_0000) } else { 0xffed_0000u32.wrapping_sub(d) };
        cpu.call(SL_TRANSLATE, &[x, 0x20_0000u32.wrapping_sub(d), (j12 << 8).wrapping_add(0x15_0000)]);
        cpu.call(SL_PUSH_MATRIX, &[]);
        if side == 0 {
            let a = joint(cpu, 15);
            cpu.call(SL_ROT_X, &[a]);
            let a = joint(cpu, 13);
            cpu.call(SL_ROT_Z, &[a]);
            let a = joint(cpu, 14);
            cpu.call(SL_ROT_Y, &[a]);
        } else {
            let a = joint(cpu, 18);
            cpu.call(SL_ROT_X, &[a]);
            let a = neg(joint(cpu, 16));
            cpu.call(SL_ROT_Z, &[a]);
            let a = neg(joint(cpu, 17));
            cpu.call(SL_ROT_Y, &[a]);
        }
        cpu.call(SL_TRANSLATE, &[0, 0, 0]);
        gs_points(cpu, if side == 0 { CD_SIDE_A_POINTS } else { CD_SIDE_B_POINTS }, o + if side == 0 { 0x298 } else { 0x2ec });
        let f = cpu.read16(o + 0x90);
        if f & 0x80 == 0 {
            put(cpu, if side == 0 { 10 } else { 9 }, Some(f));
            let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
            let l = cpu.read32(LOOPS);
            let m = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, l, 3));
            let y = (r & 3).wrapping_add(m.wrapping_mul(8)).wrapping_add(40) << 16;
            cpu.call(SL_CALC_POINT, &[0, y, 0x8_0000, o + if side == 0 { 0x340 } else { 0x34c }]);
        }
        cpu.call(SL_POP_MATRIX, &[]);
        cpu.call(SL_POP_MATRIX, &[]);
    }
    // the tail
    cpu.call(SL_PUSH_MATRIX, &[]);
    cpu.call(SL_TRANSLATE, &[0, 0x1e_0000, 0xfff6_0000]);
    cpu.call(SL_PUSH_MATRIX, &[]);
    let a = joint(cpu, 19);
    cpu.call(SL_ROT_X, &[a]);
    cpu.call(SL_TRANSLATE, &[0, 0, 0]);
    let f = cpu.read16(o + 0x90);
    if f & 0x80 == 0 {
        put(cpu, 11, Some(f));
    }
    for _ in 0..4 {
        cpu.call(SL_POP_MATRIX, &[]);
    }
    // the barriers' glows
    if cpu.read8(o + 0x41f) > 40 || cpu.read8(o + 0x420) > 40 {
        nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, t));
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
        cpu.write32(t, r & 0x100);
        let a = cpu.read16(o + TURN_Z);
        cpu.write16(t + 0x20, a);
        cpu.write8(t + 0x1e, 0);
        cpu.write8(t + 0x23, 126);
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
        cpu.write32(t + 0x14, 0xc000);
        cpu.write8(t + 0x1f, ((r & 15) as u8).wrapping_add(61));
        let c = cpu.read16(CHAR_BASE_0).wrapping_add(if cpu.read8(o + CR_LEAVING) != 0 { 112 } else { 80 });
        cpu.write16(t + 0x18, c);
        for (on, size, at) in [(0x41f, 0x4d8, 44), (0x420, 0x4dc, 92)] {
            if cpu.read8(o + on) <= 40 {
                continue;
            }
            let w = cpu.read32(o + size).wrapping_add(0x6000);
            cpu.write32(t + 0x10, w);
            for i in 0..4u32 {
                let q = t + at + 12 * i;
                let (x, y, z) = (cpu.read32(q), cpu.read32(q + 4), cpu.read32(q + 8));
                cpu.write32(t + 4, x);
                cpu.write32(t + 8, y);
                cpu.write32(t + 12, z.wrapping_add(0xff50_0000));
                nested!(cpu, 0x0604_0960, sprites::draw(cpu, t));
            }
            if on == 0x420 {
                nested!(cpu, 0x0604_0960, sprites::draw(cpu, t));
            }
        }
    }
    // the engine glows
    if cpu.read16(o + 0x90) & 0x80 == 0 {
        nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, t));
        let l = cpu.read32(LOOPS);
        let m = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, l, 3));
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000)) & 15;
        let w = m.wrapping_mul(0x3000).wrapping_add(r * 0x300).wrapping_add(0x4000);
        cpu.write32(t + 0x10, w);
        let h = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, w, 0x1_4000));
        cpu.write32(t + 0x14, h);
        let a = cpu.read16(o + TURN_Z);
        cpu.write16(t + 0x20, a);
        cpu.write8(t + 0x1e, 0);
        cpu.write8(t + 0x23, 0);
        let l = cpu.read32(LOOPS);
        let k = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, l, 3)) as i32;
        let b = cpu.read32(t + 140) != 0;
        let set = |cpu: &mut Cpu, colours: u16, frame: u8| {
            let c = cpu.read16(CHAR_BASE_0).wrapping_add(colours);
            cpu.write16(t + 0x18, c);
            cpu.write8(t + 0x1f, frame);
        };
        match k {
            0 => set(cpu, if b { 64 } else { 48 }, 0),
            1 => set(cpu, if b { 0 } else { 32 }, 1),
            2 => {
                let f = cpu.read32(t) | 0x100;
                cpu.write32(t, f);
                set(cpu, if b { 80 } else { 112 }, 1);
            }
            _ => {}
        }
        let order: [u32; 2] = if cpu.read32(LOOPS) & 1 != 0 { [0x34c, 0x340] } else { [0x340, 0x34c] };
        for at in order {
            let (x, y, z) = (cpu.read32(o + at), cpu.read32(o + at + 4), cpu.read32(o + at + 8));
            cpu.write32(t + 4, x);
            cpu.write32(t + 8, y);
            cpu.write32(t + 12, z.wrapping_add(0xff50_0000));
            nested!(cpu, 0x0604_0960, sprites::draw(cpu, t));
        }
    }
}

/// The leading carrier's start (step 0): its place to go (held 64, heading 0, 32 dots up, out 0),
/// its forward speed (3 1/4), its guard (-32 dots), its events from the top (the first track 128
/// dots ahead, the second level; both working from side 1), its lane, its escorts' bit 7; the boss
/// gauge (50000), its parts shut, sound 79.
const CL_FORWARD: u32 = 0x3_4000;
const CL_GUARD: u32 = 0xffe0_0000;
const CL_FIRST_TRACK: u32 = 0x80_0000;
const CL_SOUND: u32 = 79;

pub fn carrier_lead_start(cpu: &mut Cpu, o: u32) {
    cpu.write8(o + CARRIER_STEP, 0);
    cpu.write8(o + CE_GO_HOLD, 64);
    cpu.write16(o + CE_HEADING, 0);
    cpu.write16(o + CE_GO_HEIGHT, 32);
    cpu.write32(o + CR_FORWARD, CL_FORWARD);
    cpu.write32(o + CE_GO_X, 0);
    cpu.write32(o + CE_GUARD, CL_GUARD);
    for at in [CE_NOW, CE_INDEX, CE_EVENT, CE_EVENT + 1] {
        cpu.write8(o + at, 0);
    }
    cpu.write32(o + CE_TRACKS, CL_FIRST_TRACK);
    cpu.write32(o + CE_TRACKS + 4, 0);
    cpu.write8(o + CE_SIDE, 1);
    cpu.write8(o + CE_SIDE + 1, 1);
    cpu.write8(o + CARRIER_LANE, 0);
    cpu.write16(o + CARRIER_ESCORTS, 0x80);
    nested!(cpu, 0x0604_64d8, hud::boss_gauge_set(cpu, 50000));
    nested!(cpu, 0x0609_35e8, carrier_parts_shut(cpu, o));
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, CL_SOUND));
}

/// The tail carrier's start: step 8, turned half round, its parts shut.
pub fn carrier_tail_start(cpu: &mut Cpu, o: u32) {
    cpu.write8(o + CARRIER_STEP, 8);
    cpu.write16(o + TURN_Y, 0x8000);
    nested!(cpu, 0x0609_35e8, carrier_parts_shut(cpu, o));
}

/// The carrier's parts (parts.rs's parts_set, by the points tables its draw places them by):
/// their flags (words from +0x90), smoke (bytes from +0x9e), lives (from +0xa8); the hull's and
/// side guns' share the hull's flags and life (50000), the arms' 30000 each, the engines' 35000,
/// the pods' 30000.
const CP_PARTS: [(u32, u32, u32, u32, u32, u32); 9] = [
    (0x90, 0xa8, 50000, 0xc4, CD_HULL_POINTS.0, CD_HULL_POINTS.1),
    (0x92, 0xac, 30000, 0x118, CD_ARM_POINTS.0, CD_ARM_POINTS.1),
    (0x94, 0xb0, 30000, 0x130, CD_ARM_POINTS.0, CD_ARM_POINTS.1),
    (0x96, 0xb4, 35000, 0x148, CD_ENGINE_A_POINTS.0, CD_ENGINE_A_POINTS.1),
    (0x98, 0xb8, 35000, 0x1d8, CD_ENGINE_B_POINTS.0, CD_ENGINE_B_POINTS.1),
    (0x9a, 0xbc, 30000, 0x268, CD_POD_POINTS.0, CD_POD_POINTS.1),
    (0x9c, 0xc0, 30000, 0x280, CD_POD_POINTS.0, CD_POD_POINTS.1),
    (0x90, 0xa8, 0, 0x298, CD_SIDE_A_POINTS.0, CD_SIDE_A_POINTS.1),
    (0x90, 0xa8, 0, 0x2ec, CD_SIDE_B_POINTS.0, CD_SIDE_B_POINTS.1),
];
const CP_SMOKES: u32 = 0x9e;

pub fn carrier_parts_set(cpu: &mut Cpu, o: u32) {
    for (k, &(flags, life, start, points, table, count)) in CP_PARTS.iter().enumerate() {
        if k < 7 {
            cpu.write16(o + flags, 0);
            cpu.write8(o + CP_SMOKES + k as u32, 0);
            if k == 0 {
                cpu.write16(o + 0x358, 0);
                cpu.write16(o + 0x35a, 0);
            }
            cpu.write32(o + life, start);
        }
        let number = cpu.read8(o + NUMBER) as u32;
        cpu.push(&[table, number]);
        nested!(cpu, 0x0605_6ea4, parts::parts_set(cpu, count, o + points, o + flags, o + life, table, number));
        if k == 3 || k == 7 {
            cpu.pop(32);
        }
    }
    cpu.pop(8);
}

/// A carrier set up (`partner` the other's number, `tail` set for the second): an actor drawn as
/// a model (bits 0xc8 of its flags), at 288 dots down, 96 deep; still, unturned; its parts; the
/// leader's or the tail's start; its barriers off; its pose (20 joints from +0x38c, kind 56's) and
/// its script started.
const CS_FLAGS: u32 = 0xc8_0000;
const CS_START_Y: u32 = 0x120_0000;
const CS_START_Z: u32 = 0xffa0_0000;
const CS_JOINT_COUNT: u16 = 20;
const CS_POSE_MODEL: u32 = 0x060a_4758;
const CS_POSE_START: u32 = 0x060a_44b0;
const CS_SCRIPT_START: u32 = 0x060a_479c;

pub fn carrier_set(cpu: &mut Cpu, o: u32, partner: u32, tail: u32) {
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + 0x10));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HITS_AT));
    cpu.write32(o + BEHAVIOUR, 0x0609_477c);
    let f = cpu.read32(o + FLAGS) | CS_FLAGS;
    cpu.write32(o + FLAGS, f);
    cpu.write32(o + MODEL_DRAW, 0x0609_4c50);
    cpu.write8(o + CR_LEAVING, tail as u8);
    cpu.write8(o + CARRIER_TARGET, partner as u8);
    cpu.write32(o + POSITION, 0);
    cpu.write32(o + POSITION + 4, CS_START_Y);
    cpu.write32(o + POSITION + 8, CS_START_Z);
    for at in [CR_SPEED, CR_SPEED + 4, CR_SPEED + 8] {
        cpu.write32(o + at, 0);
    }
    for at in (0x84..=CR_TURN_Z_EASE).step_by(2) {
        cpu.write16(o + at, 0);
    }
    for at in [0x70, CARRIER_ESCORTS, 0x72, CR_WAIT, CARRIER_HELD] {
        cpu.write16(o + at, 0);
    }
    nested!(cpu, 0x0609_5f20, carrier_parts_set(cpu, o));
    if tail as u8 == 0 {
        nested!(cpu, 0x0609_5e38, carrier_lead_start(cpu, o));
    } else {
        nested!(cpu, 0x0609_5ef8, carrier_tail_start(cpu, o));
    }
    cpu.write8(o + 0x41f, 0);
    cpu.write8(o + 0x420, 0);
    cpu.write8(o + CE_ONCE, 0);
    cpu.write16(o + CR_POSE + 0x10, CS_JOINT_COUNT);
    cpu.write32(o + CB_JOINTS, o + 0x38c);
    cpu.write32(o + CR_POSE + 0x18, o + 0x3b4);
    cpu.write32(o + CR_POSE + 0x1c, CS_POSE_MODEL);
    nested!(cpu, 0x0605_65f0, models::pose_set(cpu, o + CR_POSE, CS_POSE_START));
    nested!(cpu, 0x0605_6628, actors::script_start(cpu, o + CR_POSE, CS_SCRIPT_START));
    nested!(cpu, 0x0605_6644, models::pose_step(cpu, o + CR_POSE));
}

/// The two carriers made, each the other's partner (the second the tail); kind 1's mesh off and
/// kind 56's models 17 to 33 coloured from its bank's 4th on (the tail's): the tail's number, or 0.
const CARRIER_SIZE: u32 = 0x4e8;

pub fn carriers_make(cpu: &mut Cpu) -> u32 {
    let t = cpu.stack();
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, 2)) as i8;
    cpu.write32(t, n as i32 as u32);
    if n == -1 {
        return 0;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, CARRIER_SIZE)) as i8 as i32 as u32;
    let a = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, NUMBERED_EXEC));
    if a == 0 {
        return 0;
    }
    cpu.write8(a + NUMBER, n as u8);
    let k = cpu.read8(a + NUMBER) as i8 as i32 as u32;
    cpu.write32(NUMBERED.wrapping_add(k << 2), a);
    let m = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, 2)) as i8;
    if m == -1 {
        return 0;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, CARRIER_SIZE)) as i8 as i32 as u32;
    let b = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, NUMBERED_EXEC));
    if b == 0 {
        return 0;
    }
    cpu.write8(b + NUMBER, m as u8);
    let k = cpu.read8(b + NUMBER) as i8 as i32 as u32;
    cpu.write32(NUMBERED.wrapping_add(k << 2), b);
    let m = m as i32 as u32;
    nested!(cpu, 0x0609_6114, carrier_set(cpu, a, m, 0));
    let n = cpu.read32(t);
    nested!(cpu, 0x0609_6114, carrier_set(cpu, b, n, 1));
    nested!(cpu, 0x0604_5c50, models::model_mesh(cpu, 1, 0, 0));
    for frame in 17..34 {
        let c = nested!(cpu, 0x0604_5af8, kinds::kind_colour_bank(cpu, CD_KIND)) as u16;
        nested!(cpu, 0x0604_5b1c, models::model_colour(cpu, CD_KIND, frame, c.wrapping_add(4) as u32));
    }
    m
}

// ---- the timeline's sound and message events ------------------------------------------------------

/// The stage's opening message (a timeline event, `e`): its first line by `e`'s byte +1 (from
/// 0x060a4bcc), the other three by it from the rows of 0x060a4be0, after `e`'s word +6 frames
/// (text.rs's opening_set). 1.
const OPENING_FIRST: u32 = 0x060a_4bcc;
const OPENING_ROWS: u32 = 0x060a_4be0;
const EV_STEP: u32 = 1;
const EV_WAIT: u32 = 4;

pub fn opening_event(cpu: &mut Cpu, e: u32) -> u32 {
    let k = cpu.read8(e + EV_STEP) as u32;
    let row = OPENING_ROWS + 12 * k;
    let [l2, l3, l4] = [0, 4, 8].map(|i| cpu.read32(row + i));
    let l1 = cpu.read32(OPENING_FIRST + 4 * k);
    let wait = cpu.read16(e + 6) as i16 as i32 as u32;
    cpu.nested_with_stack(&OPENING_PROLOGUE, 0x0605_99c8, &[wait], |cpu| {
        text::opening_set(cpu, l1, l2, l3, l4, wait)
    });
    1
}
static OPENING_PROLOGUE: std::sync::OnceLock<super::Prologue> = std::sync::OnceLock::new();

/// A sound data file loaded (a timeline event, `e`, its step byte +1; `n` the file): its place
/// 30016 bytes below the room's top (+0x14), loaded (the load at +0x10, its size at +0x18), then
/// copied to the sound area; 1 once done. Each frame `e`'s count (+4) runs down.
const EV_LOAD: u32 = 0x10;
const EV_PLACE: u32 = 0x14;
const EV_SIZE: u32 = 0x18;
const EV_SIZE_2: u32 = 0x1c;
const SOUND_PLACE: u32 = 0xffff_8ac0;

pub fn sound_load_event(cpu: &mut Cpu, e: u32, n: u32) -> u32 {
    let n = n & 0xff;
    let mut done = 0;
    let mut load = cpu.read32(e + EV_LOAD);
    match cpu.read8(e + EV_STEP) {
        0 => {
            let p = cpu.read32(ROOM_TOP).wrapping_add(SOUND_PLACE);
            cpu.write32(e + EV_PLACE, p);
            cpu.write8(e + EV_STEP, 1);
        }
        1 => {
            let p = cpu.read32(e + EV_PLACE);
            load = nested!(cpu, 0x0600_46d8, sound::load_sound_data(cpu, n, p));
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
            let (p, size) = (cpu.read32(e + EV_PLACE), cpu.read32(e + EV_SIZE));
            nested!(cpu, 0x0600_4adc, sound::copy_to_area(cpu, n, p, size));
            cpu.write8(e + EV_STEP, 4);
        }
        4 => done = 1,
        _ => {}
    }
    cpu.write32(e + EV_LOAD, load);
    let w = cpu.read32(e + EV_WAIT).wrapping_sub(1);
    cpu.write32(e + EV_WAIT, w);
    done
}

/// Sound data files 32 to 36 loaded (sound_load_event).
pub fn sound_load_32(cpu: &mut Cpu, e: u32) -> u32 {
    nested!(cpu, 0x0609_63cc, sound_load_event(cpu, e, 32))
}
pub fn sound_load_33(cpu: &mut Cpu, e: u32) -> u32 {
    nested!(cpu, 0x0609_63cc, sound_load_event(cpu, e, 33))
}
pub fn sound_load_34(cpu: &mut Cpu, e: u32) -> u32 {
    nested!(cpu, 0x0609_63cc, sound_load_event(cpu, e, 34))
}
pub fn sound_load_35(cpu: &mut Cpu, e: u32) -> u32 {
    nested!(cpu, 0x0609_63cc, sound_load_event(cpu, e, 35))
}
pub fn sound_load_36(cpu: &mut Cpu, e: u32) -> u32 {
    nested!(cpu, 0x0609_63cc, sound_load_event(cpu, e, 36))
}

/// The boss music's load (a timeline event, `e`): sound data files 11 and 10 loaded one after the
/// other from 0x44040 below the room's top (their sizes at +0x18 and +0x1c); once `e`'s count
/// (+4) is out, the music faded over 200 frames; once it is quiet, 2 frames on, both copied to the
/// sound area, and the count set to the files' 8 Kbyte blocks and 4 more, at least `e`'s +8; once
/// that is out, music 4 asked for (the game's +0xa4 too). 1 once done.
const BOSS_SOUND_PLACE: u32 = 0xfffb_bfc0;
const BOSS_FILES: [u32; 2] = [11, 10];
const BOSS_FADE: u32 = 200;
const BOSS_MUSIC: u32 = 4;
const MUSIC_PLAYING: u32 = 0x62;
const SOUND_MANAGER: u32 = 0x0600_edd0;
const GAME_MUSIC: u32 = 0xa4;

pub fn boss_music_event(cpu: &mut Cpu, e: u32) -> u32 {
    let mut done = 0;
    let mut load = cpu.read32(e + EV_LOAD);
    let step = cpu.read8(e + EV_STEP);
    let past = |cpu: &mut Cpu| cpu.read32(e + EV_WAIT) as i32 <= 0;
    match step {
        0 => {
            let p = cpu.read32(ROOM_TOP).wrapping_add(BOSS_SOUND_PLACE);
            cpu.write32(e + EV_PLACE, p);
            cpu.write8(e + EV_STEP, 1);
        }
        1 | 3 => {
            let p = if step == 1 {
                cpu.read32(e + EV_PLACE)
            } else {
                cpu.read32(e + EV_PLACE).wrapping_add(cpu.read32(e + EV_SIZE))
            };
            load = nested!(cpu, 0x0600_46d8, sound::load_sound_data(cpu, BOSS_FILES[step as usize / 2], p));
            if load != 0 {
                let size = nested!(cpu, 0x0600_5edc, files::load_size(cpu, load));
                cpu.write32(e + if step == 1 { EV_SIZE } else { EV_SIZE_2 }, size);
                cpu.write8(e + EV_STEP, step + 1);
            }
        }
        2 | 4 => {
            if nested!(cpu, 0x0600_6074, files::load_done(cpu, load)) != 0 {
                cpu.write8(e + EV_STEP, step + 1);
            }
        }
        5 => {
            if past(cpu) {
                nested!(cpu, 0x0600_4f0c, sound::music_fade_out(cpu, BOSS_FADE));
                cpu.write8(e + EV_STEP, 6);
            }
        }
        6 => {
            let m = cpu.read32(SOUND_MANAGER);
            if cpu.read16(m + MUSIC_PLAYING) == 0 {
                cpu.write32(e + EV_WAIT, 2);
                cpu.write8(e + EV_STEP, 7);
            }
        }
        7 => {
            if past(cpu) {
                let (p, a) = (cpu.read32(e + EV_PLACE), cpu.read32(e + EV_SIZE));
                nested!(cpu, 0x0600_4adc, sound::copy_to_area(cpu, BOSS_FILES[0], p, a));
                let (p, a, b) = (cpu.read32(e + EV_PLACE), cpu.read32(e + EV_SIZE), cpu.read32(e + EV_SIZE_2));
                nested!(cpu, 0x0600_4adc, sound::copy_to_area(cpu, BOSS_FILES[1], p.wrapping_add(a), b));
                let (a, b) = (cpu.read32(e + EV_SIZE), cpu.read32(e + EV_SIZE_2));
                let sum = a.wrapping_add(b) as i32;
                let sum = if sum < 0 { sum + 0x1fff } else { sum } as u32;
                nested!(cpu, 0x0600_815a, clib::ashiftrt_r4::<13>(cpu, sum));
                let blocks = ((sum as i32) >> 13) as u32;
                let least = cpu.read32(e + 8);
                let w = nested!(cpu, 0x0606_4f50, math::at_least(cpu, least, blocks.wrapping_add(4)));
                cpu.write32(e + EV_WAIT, w);
                cpu.write8(e + EV_STEP, 9);
            }
        }
        9 => {
            if past(cpu) {
                let g = cpu.read32(super::director::GAME);
                cpu.write16(g + GAME_MUSIC, BOSS_MUSIC as u16);
                nested!(cpu, 0x0600_4e98, sound::request_music(cpu, BOSS_MUSIC));
                cpu.write8(e + EV_STEP, 10);
            }
        }
        10 => done = 1,
        _ => {}
    }
    cpu.write32(e + EV_LOAD, load);
    let w = cpu.read32(e + EV_WAIT).wrapping_sub(1);
    cpu.write32(e + EV_WAIT, w);
    done
}

/// The boss's kinds loaded (a timeline event, `e`): from the load place, the VDP1 place and the
/// next character and colour the game keeps (+0x14, +0x18, +0x1c, +0x1e), kind 53's models read,
/// fixed and put in place (the next colour on by its count unless its mode is 8), its first 14
/// models given the high-speed shrink and lit, the shading tables ramped to (22, 24, 28) above
/// (10, 12, 14), the light turned; then kind 54's sprites the same. 1 once done.
const BK_LOAD_AT: u32 = 0x0607_40e4;
const BK_VDP1_NEXT: u32 = 0x0607_4058;
const BK_CHAR_NEXT: u32 = 0x0607_40ea;
const BK_COLOUR_NEXT: u32 = 0x0607_40e8;
const BK_CHAR: u32 = 0x1c;
const BK_COLOUR: u32 = 0x1e;
const K_ATTRS: u32 = 0xa;
const KF_CHARS: u32 = 6;
const KF_COLOUR_COUNT: u32 = 0x16;
const MODE_BITS: u16 = 0x38;

/// After a kind's install (`next` its VDP1 place on): the next character on by its file's count,
/// the next colour by its colours' (a model kind's only when its mode is not 8), the place past it.
fn bk_advance(cpu: &mut Cpu, e: u32, k: u32, next: u32, models: bool) {
    cpu.write32(e + EV_SIZE, next);
    let file = cpu.read32(kind(k));
    let c = cpu.read16(e + BK_CHAR).wrapping_add(cpu.read16(file + KF_CHARS));
    cpu.write16(e + BK_CHAR, c);
    if !models || cpu.read16(kind(k) + K_ATTRS) & MODE_BITS != 8 {
        let file = cpu.read32(kind(k));
        let c = cpu.read16(e + BK_COLOUR).wrapping_add(cpu.read16(file + KF_COLOUR_COUNT));
        cpu.write16(e + BK_COLOUR, c);
    }
    let file = cpu.read32(kind(k));
    let past = cpu.read32(file);
    cpu.write32(e + EV_PLACE, past);
}

/// A timeline event's kinds: the model kind, the sprite kind, the models lit, the shading.
struct KindsEvent {
    models: u32,
    sprites: u32,
    lit: u32,
    shading: Option<Shading>,
}

/// The shading set after a model kind: the table's ramp's top and floor, the light's turn (x, y).
#[derive(Clone, Copy)]
struct Shading {
    ramp: [u32; 3],
    floor: [u32; 3],
    light: [u32; 2],
}

const BOSS_KINDS: KindsEvent = KindsEvent { models: 53, sprites: 54, lit: 14, shading: Some(Shading { ramp: [22, 24, 28], floor: [10, 12, 14], light: [0x1000, 0xffff_e000] }) };

/// A model kind read in (`k`): fixed and put in place.
fn bk_model_place(cpu: &mut Cpu, e: u32, k: u32) {
    let at = cpu.read32(e + EV_PLACE);
    nested!(cpu, 0x0604_583c, kinds::model_kind_fix(cpu, k, at));
    let vram = cpu.read32(e + EV_SIZE);
    let first = cpu.read16(e + BK_CHAR) as i16 as i32 as u32;
    let colour = cpu.read16(e + BK_COLOUR) as u32;
    let next = nested!(cpu, 0x0604_5950, kinds::model_kind_install(cpu, k, vram, first, colour));
    bk_advance(cpu, e, k, next, true);
}

/// A sprite kind read in (`k`): fixed and put in place.
fn bk_sprites_in(cpu: &mut Cpu, e: u32, k: u32) {
    let at = cpu.read32(e + EV_PLACE);
    nested!(cpu, 0x0604_5638, kinds::sprite_kind_fix(cpu, k, at));
    let vram = cpu.read32(e + EV_SIZE);
    let first = cpu.read16(e + BK_CHAR) as i16 as i32 as u32;
    let colour = cpu.read16(e + BK_COLOUR) as u32;
    let next = nested!(cpu, 0x0604_570c, kinds::sprite_kind_install(cpu, k, vram, first, colour));
    bk_advance(cpu, e, k, next, false);
}

/// A model kind read in (`k`): fixed, put in place, its first `lit` models given the high-speed
/// shrink and lit; with `shading`, the shading table ramped and the light turned.
fn bk_models_in(cpu: &mut Cpu, e: u32, k: u32, lit: u32, shading: Option<Shading>) {
    bk_model_place(cpu, e, k);
    nested!(cpu, 0x0606_2f74, gouraud::models_hss(cpu, k, lit));
    nested!(cpu, 0x0606_3380, gouraud::models_lit(cpu, k, lit));
    if let Some(Shading { ramp, floor, light }) = shading {
        nested!(cpu, 0x0606_3168, gouraud::table_ramp(cpu, ramp[0], ramp[1], ramp[2]));
        nested!(cpu, 0x0606_30e0, gouraud::table_floor(cpu, floor[0], floor[1], floor[2]));
        nested!(cpu, 0x0606_2fe8, gouraud::light_turned(cpu, light[0], light[1], 0));
    }
}

/// The kinds' event's first step: the load place, the VDP1 place and the next character and
/// colour the game keeps, taken.
fn bk_start(cpu: &mut Cpu, e: u32) {
    let v = cpu.read32(BK_LOAD_AT);
    cpu.write32(e + EV_PLACE, v);
    let v = cpu.read32(BK_VDP1_NEXT);
    cpu.write32(e + EV_SIZE, v);
    let v = cpu.read16(BK_CHAR_NEXT);
    cpu.write16(e + BK_CHAR, v);
    let v = cpu.read16(BK_COLOUR_NEXT);
    cpu.write16(e + BK_COLOUR, v);
    cpu.write8(e + EV_STEP, 1);
}

pub fn boss_kinds_event(cpu: &mut Cpu, e: u32) -> u32 {
    kinds_event(cpu, e, &BOSS_KINDS)
}

fn kinds_event(cpu: &mut Cpu, e: u32, k: &KindsEvent) -> u32 {
    let mut done = 0;
    let mut load = cpu.read32(e + EV_LOAD);
    match cpu.read8(e + EV_STEP) {
        0 => bk_start(cpu, e),
        1 => {
            let at = cpu.read32(e + EV_PLACE);
            load = nested!(cpu, 0x0604_5910, kinds::kind_load(cpu, k.models, at));
            if load != 0 {
                cpu.write8(e + EV_STEP, 2);
            }
        }
        2 => {
            if nested!(cpu, 0x0600_6074, files::load_done(cpu, load)) != 0 {
                bk_models_in(cpu, e, k.models, k.lit, k.shading);
                cpu.write8(e + EV_STEP, 3);
            }
        }
        3 => cpu.write8(e + EV_STEP, 4),
        4 => {
            let at = cpu.read32(e + EV_PLACE);
            load = nested!(cpu, 0x0604_56cc, kinds::kind_load(cpu, k.sprites, at));
            if load != 0 {
                cpu.write8(e + EV_STEP, 5);
            }
        }
        5 => {
            if nested!(cpu, 0x0600_6074, files::load_done(cpu, load)) != 0 {
                bk_sprites_in(cpu, e, k.sprites);
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

/// The carriers' kinds loaded (a timeline event, `e`): as the boss's (boss_kinds_event), kind 56's
/// models (34 lit, the shading ramped to (28, 28, 28) above (12, 12, 12), the light the other
/// way) and kind 57's sprites. 1 once done.
const CARRIER_KINDS: KindsEvent = KindsEvent { models: 56, sprites: 57, lit: 34, shading: Some(Shading { ramp: [28, 28, 28], floor: [12, 12, 12], light: [0xffff_e000, 0x2000] }) };

pub fn carrier_kinds_event(cpu: &mut Cpu, e: u32) -> u32 {
    kinds_event(cpu, e, &CARRIER_KINDS)
}

/// A boss's event (a timeline event, `e`, its step +1): (`wait`: once the screen asks) the boss
/// made by `make` (its number at +8); once it is gone, the results (60 frames on; the results
/// object at +4) and the scroll moved on; once they end, the scroll moved on again; then, once the
/// screen asks, the next events (`next`) set going: -1. Else 0.
const MB_EVENT_RESULTS: u32 = 4;
const MB_EVENT_NUMBER: u32 = 8;
const MB_RESULTS_DELAY: u32 = 60;
const SCREEN_REQUESTED: u32 = 0x0607_4804;
const TIMELINE_NOW: u32 = 0x0607_4734;
const RESULTS_ENDED: i8 = 111;

fn boss_event(cpu: &mut Cpu, e: u32, wait: bool, make: impl Fn(&mut Cpu) -> u32, next: u32) -> u32 {
    let step = cpu.read8(e + EV_STEP);
    match step.wrapping_add(!wait as u8) {
        0 => {
            if cpu.read32(SCREEN_REQUESTED) != 0 {
                cpu.write8(e + EV_STEP, step + 1);
            }
        }
        1 => {
            let n = make(cpu) as i8;
            if n != 0 {
                cpu.write8(e + EV_STEP, step + 1);
                cpu.write32(e + MB_EVENT_NUMBER, n as i32 as u32);
            }
        }
        2 => {
            let n = cpu.read32(e + MB_EVENT_NUMBER);
            if cpu.read32(NUMBERED.wrapping_add(n << 2)) != 0 {
                return 0;
            }
            let r = nested!(cpu, 0x0605_284c, results::results_set(cpu, MB_RESULTS_DELAY));
            if r != 0 {
                cpu.write8(e + EV_STEP, step + 1);
                cpu.write32(e + MB_EVENT_RESULTS, r);
                nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
            }
        }
        3 => {
            let r = cpu.read32(e + MB_EVENT_RESULTS);
            if cpu.read8(r + OBJ_MODE) as i8 > RESULTS_ENDED {
                cpu.write8(e + EV_STEP, step + 1);
                nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
            }
        }
        4 if cpu.read32(SCREEN_REQUESTED) != 0 => {
            let t = nested!(cpu, 0x0605_7354, timeline::timeline_set(cpu, next));
            cpu.write32(TIMELINE_NOW, t);
            return 0xffff_ffff;
        }
        _ => {}
    }
    0
}

/// The mid-boss's event (boss_event: made at once; then the events at 0x060a50a0).
const MB_NEXT_EVENTS: u32 = 0x060a_50a0;

pub fn mid_boss_event(cpu: &mut Cpu, e: u32) -> u32 {
    boss_event(cpu, e, false, |cpu| nested!(cpu, 0x0608_cdb0, mb_make(cpu)), MB_NEXT_EVENTS)
}

/// The carriers' event (boss_event: made once the screen asks; then the events at 0x060a54c0).
const CARRIER_NEXT_EVENTS: u32 = 0x060a_54c0;

pub fn carrier_event(cpu: &mut Cpu, e: u32) -> u32 {
    boss_event(cpu, e, true, |cpu| nested!(cpu, 0x0609_6278, carriers_make(cpu)), CARRIER_NEXT_EVENTS)
}

/// The scenery's kinds loaded (a timeline event, `e`): as the boss's (boss_kinds_event), two
/// model kinds: 52 (5 lit, the shading ramped to (28, 27, 24) above (10, 10, 10)), then 55 (9 lit,
/// above (0, 3, 6)), the light turned to (-1/8, -1/8) by each. 1 once done.
const SCENERY_KINDS: [u32; 2] = [52, 55];

pub fn scenery_kinds_event(cpu: &mut Cpu, e: u32) -> u32 {
    let mut done = 0;
    let mut load = cpu.read32(e + EV_LOAD);
    let step = cpu.read8(e + EV_STEP);
    match step {
        0 => bk_start(cpu, e),
        1 | 3 => {
            let at = cpu.read32(e + EV_PLACE);
            load = nested!(cpu, 0x0604_5910, kinds::kind_load(cpu, SCENERY_KINDS[step as usize / 2], at));
            if load != 0 {
                cpu.write8(e + EV_STEP, step + 1);
            }
        }
        2 => {
            if nested!(cpu, 0x0600_6074, files::load_done(cpu, load)) != 0 {
                bk_models_in(cpu, e, SCENERY_KINDS[0], 5, Some(Shading { ramp: [28, 27, 24], floor: [10, 10, 10], light: [0xffff_e000; 2] }));
                cpu.write8(e + EV_STEP, step + 1);
            }
        }
        4 if nested!(cpu, 0x0600_6074, files::load_done(cpu, load)) != 0 => {
            bk_models_in(cpu, e, SCENERY_KINDS[1], 9, Some(Shading { ramp: [28, 27, 24], floor: [0, 3, 6], light: [0xffff_e000; 2] }));
            done = 1;
        }
        _ => {}
    }
    cpu.write32(e + EV_LOAD, load);
    done
}

/// The great ship's event (a timeline event): the great ship made. 1.
pub fn great_ship_event(cpu: &mut Cpu, _e: u32) -> u32 {
    nested!(cpu, 0x0608_8270, great_ship_make(cpu));
    1
}

/// The serpent's event (boss_event: made at once; then the events at 0x060a5820).
const SS_NEXT_EVENTS: u32 = 0x060a_5820;

pub fn serpent_event(cpu: &mut Cpu, e: u32) -> u32 {
    boss_event(cpu, e, false, |cpu| nested!(cpu, 0x0608_fe24, ss_make(cpu)), SS_NEXT_EVENTS)
}

/// The last boss's kinds loaded (a timeline event, `e`): as the boss's (boss_kinds_event), kind
/// 58's models (19 lit, the shading left as it is) and kind 59's sprites. 1 once done.
const LAST_KINDS: KindsEvent = KindsEvent { models: 58, sprites: 59, lit: 19, shading: None };

pub fn last_kinds_event(cpu: &mut Cpu, e: u32) -> u32 {
    kinds_event(cpu, e, &LAST_KINDS)
}

/// The last boss's event (a timeline event, `e`): boss_event's steps but for the scroll, which it
/// takes on only when the results are done. 0, the boss made (0x0609b2f4: its number kept at +8);
/// 1, once it is gone the results (60); 2, once they have ended (a mode past 111), the scroll
/// moved on; 3, once the next screen is asked for, the timeline at 0x060a5b40 and -1. Else 0.
const LAST_NEXT_EVENTS: u32 = 0x060a_5b40;
const LAST_MAKE: u32 = 0x0609_b2f4;

pub fn last_boss_event(cpu: &mut Cpu, e: u32) -> u32 {
    match cpu.read8(e + EV_STEP) {
        0 => {
            let n = nested!(cpu, LAST_MAKE, last_boss_make(cpu)) as i8;
            if n != 0 {
                cpu.write8(e + EV_STEP, 1);
                cpu.write32(e + MB_EVENT_NUMBER, n as i32 as u32);
            }
        }
        1 => {
            let n = cpu.read32(e + MB_EVENT_NUMBER);
            if cpu.read32(NUMBERED.wrapping_add(n << 2)) != 0 {
                return 0;
            }
            let r = nested!(cpu, 0x0605_284c, results::results_set(cpu, MB_RESULTS_DELAY));
            if r != 0 {
                cpu.write8(e + EV_STEP, 2);
                cpu.write32(e + MB_EVENT_RESULTS, r);
            }
        }
        2 => {
            let r = cpu.read32(e + MB_EVENT_RESULTS);
            if cpu.read8(r + OBJ_MODE) as i8 > RESULTS_ENDED {
                cpu.write8(e + EV_STEP, 3);
                nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
            }
        }
        3 if cpu.read32(SCREEN_REQUESTED) != 0 => {
            let t = nested!(cpu, 0x0605_7354, timeline::timeline_set(cpu, LAST_NEXT_EVENTS));
            cpu.write32(TIMELINE_NOW, t);
            return 0xffff_ffff;
        }
        _ => {}
    }
    0
}

/// The ground's movement (globals.rs's GROUND_DX_AT..) pointed at the stage's (a timeline event). 1.
const STAGE_GROUND_D: [u32; 3] = [0x0607_2f5c, 0x060a_4c1c, 0x0607_2f64];

pub fn ground_moves_event(cpu: &mut Cpu, _e: u32) -> u32 {
    use super::globals::{GROUND_DX_AT, GROUND_DY_AT, GROUND_DZ_AT};
    for (at, to) in [GROUND_DX_AT, GROUND_DY_AT, GROUND_DZ_AT].into_iter().zip(STAGE_GROUND_D) {
        cpu.write32(at, to);
    }
    1
}

/// The stage's last kinds loaded (a timeline event, `e`): as the boss's (boss_kinds_event), the
/// sprite kinds 61 and 19, a frame's pause, the sprite kinds 10 and 62, then kind 60's models (13
/// given the high-speed shrink). 1 once done.
const LATE_SPRITES: [u32; 4] = [61, 19, 10, 62];
const LATE_MODELS: u32 = 60;
const LATE_HSS: u32 = 13;

/// Which of the sprite kinds a step loads or puts in place.
fn late_index(step: u8) -> usize {
    match step {
        1 | 2 => 0,
        3 | 4 => 1,
        6 | 7 => 2,
        _ => 3,
    }
}

pub fn late_kinds_event(cpu: &mut Cpu, e: u32) -> u32 {
    let mut done = 0;
    let mut load = cpu.read32(e + EV_LOAD);
    let step = cpu.read8(e + EV_STEP);
    let next = |cpu: &mut Cpu| cpu.write8(e + EV_STEP, step + 1);
    match step {
        0 => bk_start(cpu, e),
        5 => next(cpu),
        1 | 3 | 6 | 8 | 10 => {
            let at = cpu.read32(e + EV_PLACE);
            load = if step == 10 {
                nested!(cpu, 0x0604_5910, kinds::kind_load(cpu, LATE_MODELS, at))
            } else {
                let k = LATE_SPRITES[late_index(step)];
                nested!(cpu, 0x0604_56cc, kinds::kind_load(cpu, k, at))
            };
            if load != 0 {
                next(cpu);
            }
        }
        2 | 4 | 7 | 9 | 11 => {
            if nested!(cpu, 0x0600_6074, files::load_done(cpu, load)) != 0 {
                if step == 11 {
                    bk_model_place(cpu, e, LATE_MODELS);
                    nested!(cpu, 0x0606_2f74, gouraud::models_hss(cpu, LATE_MODELS, LATE_HSS));
                } else {
                    let k = LATE_SPRITES[late_index(step)];
                    bk_sprites_in(cpu, e, k);
                }
                next(cpu);
            }
        }
        12 => done = 1,
        _ => {}
    }
    cpu.write32(e + EV_LOAD, load);
    done
}

// ---- the last boss --------------------------------------------------------------------------------

/// The last boss (made by 0x0609b2f4, its kinds 58 and 59): its step (+0x6c, by the table at
/// 0x060a770c), its life (+0x88, the boss gauge's while its step is past 0). With its life gone:
/// the stage's score 50000, its rate 25 more, the kill scored to whoever hit it (10000), and it
/// is beaten (0x0609af58). Then its step's function.
const LB_STEP: u32 = 0x6c;
const LB_LIFE: u32 = 0x88;
const LB_STATES: u32 = 0x060a_770c;
const LB_STAGE_SCORE: u32 = 50000;
const LB_RATE: u32 = 25;
const LB_POINTS: u32 = 10000;
const LB_BEATEN: u32 = 0x0609_af58;

pub fn last_boss_behaviour(cpu: &mut Cpu, o: u32) {
    if cpu.read8(o + LB_STEP) as i8 > 0 {
        let life = cpu.read32(o + LB_LIFE);
        nested!(cpu, 0x0604_64b8, hud::boss_gauge_life(cpu, life));
    }
    if cpu.read32(o + LB_LIFE) as i32 <= 0 {
        cpu.write32(STAGE_SCORE_AT, LB_STAGE_SCORE);
        let r = cpu.read32(STAGE_RATE_AT).wrapping_add(LB_RATE);
        cpu.write32(STAGE_RATE_AT, r);
        let by = cpu.read32(o + HIT_BY);
        let (player, weapon) = (cpu.read8(by + HIT_PLAYER) as i8 as i32 as u32, cpu.read8(by + HIT_WEAPON) as i8 as i32 as u32);
        nested!(cpu, 0x0604_3108, hits::score_kill(cpu, player, weapon, o, LB_POINTS));
        nested!(cpu, LB_BEATEN, last_boss_beaten(cpu, o));
    }
    let step = cpu.read8(o + LB_STEP) as i8 as i32 as u32;
    let f = cpu.read32(LB_STATES.wrapping_add(step << 2));
    cpu.call(f, &[o]);
}

/// A last boss's part (an actor circling its centre): its index (+0x6e, a byte), its life (+0x50,
/// copied to its own +0x88 by its index), its hits (+0x56: bit 0 marks its index's byte in the
/// table at 0x060a6064), its inner step (+0x6d), its count (+0x80), its place round the circle
/// (+0x76, turned by +0x7a each frame; its x and y from it at +0xa0 and +0xa4, 112 dots out), its
/// tilt (+0x74). Drawn into place (its point by 0x060a65e8, turned to face its way, +0x30).
/// Inner steps: 0, shown and the table's pick made (0x060a8910 by 0x060a8988 from 0x060a5fe0);
/// 1, for 96 frames (0x060979fc, then 0x06097ccc), then 3; 2, 0x060979fc then 0x06097ab8; 3, for
/// 128 frames (0x06097950, then its index's function from 0x060a66fc), then 0x06097970; 4, every
/// 4 frames 0x0609ad20 at its place, at 60 0x0609acd8 there (its turn) and its index's byte set
/// again from 0x060a6084, then 0x06097970. Out of life in steps 1 and 3: 0x060979cc.
const LP_INDEX: u32 = 0x6e;
const LP_LIFE: u32 = 0x50;
const LP_LIVES: u32 = 0x88;
const LP_HITS: u32 = 0x56;
const LP_MARKS: u32 = 0x060a_6064;
const LP_STEP: u32 = 0x6d;
const LP_COUNT: u32 = 0x80;
const LP_ROUND: u32 = 0x76;
const LP_ROUND_SPEED: u32 = 0x7a;
const LP_TILT: u32 = 0x74;
const LP_PLACE: u32 = 0xa0;
const LP_FACING: u32 = 0x30;
const LP_POINT: u32 = 0x060a_65e8;
const LP_RADIUS: u32 = 0x70_0000;
const LP_PICK: u32 = 0x060a_8910;
const LP_PICK_AT: u32 = 0x060a_8988;
const LP_PICKS: u32 = 0x060a_5fe0;
const LP_SHOWN_OFF: u32 = 0xfff7_ffff;
const LP_INDEX_FNS: u32 = 0x060a_66fc;
const LP_MARKS_AGAIN: u32 = 0x060a_6084;
const LP_SPIN: u32 = 0x0609_7ccc;
const LP_SPIN_ON: u32 = 0x0609_7ab8;
const LP_NEXT: u32 = 0x0609_7970;
const LP_LOST: u32 = 0x0609_79cc;
const LP_SPARKS: u32 = 0x0609_ad20;
const LP_BURST: u32 = 0x0609_acd8;

pub fn last_part_behaviour(cpu: &mut Cpu, o: u32) {
    let t = cpu.stack();
    let index = |cpu: &mut Cpu| cpu.read8(o + LP_INDEX) as i8 as i32 as u32;
    let k = index(cpu);
    let life = cpu.read32(o + LP_LIFE);
    cpu.write32(o + LP_LIVES + 4 * k, life);
    if cpu.read16(o + LP_HITS) & 1 != 0 {
        let k = index(cpu);
        cpu.write8(LP_MARKS + 1 + 2 * k, 1);
    }
    cpu.call(SL_PUSH_MATRIX, &[]);
    let p = [0, 4, 8].map(|i| cpu.read32(o + LP_PLACE + i));
    cpu.call(SL_TRANSLATE, &p);
    let a = cpu.read16(o + LP_TILT) as i16 as i32 as u32;
    cpu.call(SL_ROT_X, &[a]);
    let a = cpu.read16(o + LP_ROUND);
    cpu.call(SL_ROT_Y, &[a as i16 as i32 as u32]);
    cpu.write16(o + LP_FACING, a);
    cpu.call(0x0606_750c, &[LP_POINT, o + POSITION]);
    cpu.call(SL_POP_MATRIX, &[]);
    let alive = |cpu: &mut Cpu| cpu.read32(o + LP_LIFE) as i32 > 0;
    let then = match cpu.read8(o + LP_STEP) {
        0 => {
            cpu.write32(o + LP_COUNT, 0);
            let at = cpu.read32(LP_PICK_AT);
            let v = cpu.read8(LP_PICKS.wrapping_add(at)) as i8 as i32 as u32;
            cpu.write32(LP_PICK, v);
            let f = cpu.read32(o + FLAGS) & LP_SHOWN_OFF;
            cpu.write32(o + FLAGS, f);
            cpu.write8(o + LP_STEP, 1);
            None
        }
        1 if !alive(cpu) => Some(LP_LOST),
        1 => {
            nested!(cpu, 0x0609_79fc, last_part_turning(cpu, o));
            let c = cpu.read32(o + LP_COUNT).wrapping_add(1);
            cpu.write32(o + LP_COUNT, c);
            if c == 96 {
                cpu.write8(o + LP_STEP, 3);
                cpu.write32(o + LP_COUNT, 0);
            }
            Some(LP_SPIN)
        }
        2 => {
            nested!(cpu, 0x0609_79fc, last_part_turning(cpu, o));
            Some(LP_SPIN_ON)
        }
        3 if !alive(cpu) => Some(LP_LOST),
        3 => {
            nested!(cpu, 0x0609_7950, last_part_slowing(cpu, o));
            let k = index(cpu);
            let f = cpu.read32(LP_INDEX_FNS + 4 * k);
            cpu.call(f, &[o]);
            let c = cpu.read32(o + LP_COUNT).wrapping_add(1);
            cpu.write32(o + LP_COUNT, c);
            (c == 0x80).then_some(LP_NEXT)
        }
        4 => {
            let c = cpu.read32(o + LP_COUNT);
            if c & 3 == 0 {
                for i in [0, 4, 8] {
                    let v = cpu.read32(o + POSITION + i);
                    cpu.write32(t + i, v);
                }
                let c = cpu.read32(o + LP_COUNT);
                nested!(cpu, LP_SPARKS, last_spark_make(cpu, t, c & 4));
            }
            let c = cpu.read32(o + LP_COUNT);
            cpu.write32(o + LP_COUNT, c.wrapping_add(1));
            if c == 60 {
                for i in [0, 4, 8] {
                    let v = cpu.read32(o + POSITION + i);
                    cpu.write32(t + 12 + i, v);
                }
                let a = cpu.read16(o + LP_FACING) as i16 as i32 as u32;
                nested!(cpu, LP_BURST, last_burst(cpu, t + 12, a));
                let k = index(cpu);
                let v = cpu.read8(LP_MARKS_AGAIN + k);
                cpu.write8(LP_MARKS + 2 * k, v);
                Some(LP_NEXT)
            } else {
                None
            }
        }
        _ => None,
    };
    match then {
        Some(LP_NEXT) => nested!(cpu, 0x0609_7970, last_part_next(cpu, o)),
        Some(LP_SPIN_ON) => nested!(cpu, 0x0609_7ab8, last_part_spin_on(cpu, o)),
        Some(LP_SPIN) => nested!(cpu, 0x0609_7ccc, last_part_volley(cpu, o)),
        Some(f) => {
            cpu.call(f, &[o]);
        }
        None => {}
    }
    let r = cpu.read16(o + LP_ROUND).wrapping_add(cpu.read16(o + LP_ROUND_SPEED));
    cpu.write16(o + LP_ROUND, r);
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, r as i16 as i32 as u32));
    let x = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, s, LP_RADIUS));
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, r as i16 as i32 as u32));
    let y = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, c, LP_RADIUS.wrapping_neg()));
    cpu.write32(o + LP_PLACE, x);
    cpu.write32(o + LP_PLACE + 4, y.wrapping_add(LP_RADIUS));
}

/// Its round's speed eased toward still, by 4 a frame.
pub fn last_part_slowing(cpu: &mut Cpu, o: u32) {
    let v = cpu.read16(o + LP_ROUND_SPEED) as i16;
    if v > 0 {
        cpu.write16(o + LP_ROUND_SPEED, (v - 4) as u16);
    } else if v < 0 {
        cpu.write16(o + LP_ROUND_SPEED, (v + 4) as u16);
    }
}

/// Its next round (+0x6f counts them): after 5, the pick cleared and step 2 from the start;
/// else inner step 2, its speed back to its own (+0x78), hidden, its count from 0.
const LP_ROUNDS: u32 = 0x6f;
const LP_OWN_SPEED: u32 = 0x78;
const LP_HIDDEN: u32 = 0x8_0000;

pub fn last_part_next(cpu: &mut Cpu, o: u32) {
    let n = cpu.read8(o + LP_ROUNDS).wrapping_add(1);
    cpu.write8(o + LP_ROUNDS, n);
    if n == 5 {
        cpu.write32(LP_PICK, 0);
        cpu.write8(o + LP_STEP, 0);
        cpu.write8(o + LB_STEP, 2);
    } else {
        cpu.write8(o + LP_STEP, 2);
        let v = cpu.read16(o + LP_OWN_SPEED);
        cpu.write16(o + 0x7c, v);
        let f = cpu.read32(o + FLAGS) | LP_HIDDEN;
        cpu.write32(o + FLAGS, f);
        cpu.write32(o + LP_COUNT, 0);
    }
}

/// Its round's speed turned toward the nearer player: within 1/128 of a turn of facing them it
/// slows; else it speeds up their way (by 4, or 8 when going the other way), at most 128.
const LP_FACING_NEAR: i32 = 0x1ff;
const LP_MOST_SPEED: i16 = 0x80;

pub fn last_part_turning(cpu: &mut Cpu, o: u32) {
    let p = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + 0x10));
    let dx = cpu.read32(p + POSITION).wrapping_sub(cpu.read32(o + POSITION));
    let dy = cpu.read32(p + POSITION + 4).wrapping_sub(cpu.read32(o + POSITION + 4));
    let a = nested!(cpu, 0x0606_780c, sgl::sl_atan(cpu, dx, dy)) as u16;
    let d = a.wrapping_add(0xc000).wrapping_sub(cpu.read16(o + LP_ROUND)) as i16 as i32;
    if d.abs() <= LP_FACING_NEAR {
        nested!(cpu, 0x0609_7950, last_part_slowing(cpu, o));
    } else if d > 0 {
        let v = cpu.read16(o + LP_ROUND_SPEED) as i16;
        let v = v.wrapping_add(if v < 0 { 8 } else { 4 });
        cpu.write16(o + LP_ROUND_SPEED, v.min(LP_MOST_SPEED) as u16);
    } else {
        let v = cpu.read16(o + LP_ROUND_SPEED) as i16;
        let v = v.wrapping_add(if v > 0 { -8 } else { -4 });
        cpu.write16(o + LP_ROUND_SPEED, v.max(-LP_MOST_SPEED) as u16);
    }
}

/// Its quarter turn between parts (inner step 2): its turn (+0x78) eased a quarter on from +0x7c
/// over 1/170 a frame (the count, +0x80, to 1); then the next part (+0x6e, of 4) takes over with
/// its kept life: alive, inner step 1 and shown; else last_part_next.
const LP_TURN: u32 = 0x78;
const LP_TURN_FROM: u32 = 0x7c;
const LP_EASE_STEP: u32 = 0x180;
const LP_EASED: u32 = 0x1_0000;

pub fn last_part_spin_on(cpu: &mut Cpu, o: u32) {
    let c = cpu.read32(o + LP_COUNT).wrapping_add(LP_EASE_STEP);
    cpu.write32(o + LP_COUNT, c);
    if c as i32 > 0xffff {
        cpu.write32(o + LP_COUNT, LP_EASED);
    }
    let a = cpu.read16(o + LP_TURN_FROM) as i16 as i32 as u32;
    let c = cpu.read32(o + LP_COUNT);
    let v = nested!(cpu, 0x0606_2ebc, math::ease(cpu, a, a.wrapping_add(0x4000), c));
    cpu.write16(o + LP_TURN, v as u16);
    if cpu.read32(o + LP_COUNT) == LP_EASED {
        let k = cpu.read8(o + LP_INDEX).wrapping_add(1) & 3;
        cpu.write8(o + LP_INDEX, k);
        cpu.write32(o + LP_COUNT, 0);
        let k = cpu.read8(o + LP_INDEX) as i8 as i32 as u32;
        let life = cpu.read32(o + LP_LIVES + 4 * k);
        cpu.write32(o + LP_LIFE, life);
        if life as i32 > 0 {
            cpu.write8(o + LP_STEP, 1);
            let f = cpu.read32(o + FLAGS) & LP_SHOWN_OFF;
            cpu.write32(o + FLAGS, f);
        } else {
            nested!(cpu, 0x0609_7970, last_part_next(cpu, o));
        }
    }
}

/// A pair of shots from a part (its two muzzles, by 0x060a65f4 and 0x060a6600 through its
/// place and turns): each its owner's number, a speed of 1/2 to 1 (at random), turned from the
/// part's way by up to 3/8 of a turn either side less 3/16 (in 1/128ths), made by 0x0609841c;
/// `glow`: then a glow at each (explosions.rs's glow_set, its +0x2c long 0xe000).
const LPS_MUZZLES: [u32; 2] = [0x060a_65f4, 0x060a_6600];
const LPS_SPEED_BITS: u32 = 0x7000;
const LPS_SPEED: u32 = 0x8000;
const LPS_SPREAD: u32 = 0x6000;
const LPS_SPREAD_LESS: u32 = 0xffff_d000;
const LPS_SPREAD_STEP: u32 = 0xffff_fe00;
const LPS_GLOW: u32 = 0xe000;

pub fn last_part_shots(cpu: &mut Cpu, o: u32, glow: u32) {
    let t = cpu.stack();
    let d = [t, t + 64];
    cpu.write32(t + 0x80, glow);
    cpu.call(SL_PUSH_MATRIX, &[]);
    let p = [0, 4, 8].map(|i| cpu.read32(o + LP_PLACE + i));
    cpu.call(SL_TRANSLATE, &p);
    let a = cpu.read16(o + LP_TILT) as i16 as i32 as u32;
    cpu.call(SL_ROT_X, &[a]);
    let a = cpu.read16(o + LP_ROUND) as i16 as i32 as u32;
    cpu.call(SL_ROT_Y, &[a]);
    for (k, m) in LPS_MUZZLES.into_iter().enumerate() {
        cpu.call(0x0606_750c, &[m, d[k]]);
    }
    cpu.call(SL_POP_MATRIX, &[]);
    cpu.write32(d[1] + 8, 0);
    cpu.write32(d[0] + 8, 0);
    // the SH-2 code keeps the second's number's address in its frame
    cpu.write32(t + 0x84, d[1] + S_NUMBER);
    let n = cpu.read8(o + NUMBER);
    cpu.write8(d[1] + S_NUMBER, n);
    cpu.write8(d[0] + S_NUMBER, n);
    for at in d {
        let r = nested!(cpu, 0x0603_4dd4, sgl::sl_random(cpu));
        cpu.write32(at + 48, (r & LPS_SPEED_BITS).wrapping_add(LPS_SPEED));
    }
    let a = cpu.read16(o + LP_ROUND).wrapping_neg();
    cpu.write16(d[1] + S_ANGLE, a);
    cpu.write16(d[0] + S_ANGLE, a);
    for at in d {
        let r = nested!(cpu, 0x0603_4dd4, sgl::sl_random(cpu));
        let v = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, r, LPS_SPREAD));
        let v = v.wrapping_add(LPS_SPREAD_LESS) & LPS_SPREAD_STEP;
        let a = cpu.read16(at + S_ANGLE).wrapping_add(v as u16);
        cpu.write16(at + S_ANGLE, a);
    }
    for at in d {
        nested!(cpu, 0x0609_841c, bouncing_shot_set(cpu, at));
    }
    if cpu.read32(t + 0x80) != 0 {
        for at in d {
            cpu.write32(at + S_ANGLE, LPS_GLOW);
            nested!(cpu, 0x0605_8f34, explosions::glow_set(cpu, at));
        }
    }
}

/// Every 32 frames of its count, shots by the table's pick (0x060a8988): 0, a pair with glows;
/// 1, a pair without and a pair with; more, two without and one with. Sound 97.
const LP_SHOTS_SOUND: u32 = 97;

pub fn last_part_volley(cpu: &mut Cpu, o: u32) {
    if cpu.read32(o + LP_COUNT) & 31 != 0 {
        return;
    }
    let n = cpu.read32(LP_PICK_AT);
    let plain = match n {
        0 => 0,
        1 => 1,
        _ => 2,
    };
    for _ in 0..plain {
        nested!(cpu, 0x0609_7b68, last_part_shots(cpu, o, 0));
    }
    nested!(cpu, 0x0609_7b68, last_part_shots(cpu, o, 1));
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, LP_SHOTS_SOUND));
}

/// A shot from one of a part's muzzles (`k`, 12-byte points from `table`, through its place and
/// turns): its owner's number, its way the part's turned half round and `turn` more, made by
/// `fire`.
fn lp_muzzle_shot(cpu: &mut Cpu, o: u32, k: u32, turn: u32, table: u32, fire: impl Fn(&mut Cpu, u32)) {
    let t = cpu.stack();
    cpu.call(SL_PUSH_MATRIX, &[]);
    let p = [0, 4, 8].map(|i| cpu.read32(o + LP_PLACE + i));
    cpu.call(SL_TRANSLATE, &p);
    let a = cpu.read16(o + LP_TILT) as i16 as i32 as u32;
    cpu.call(SL_ROT_X, &[a]);
    let a = cpu.read16(o + LP_ROUND) as i16 as i32 as u32;
    cpu.call(SL_ROT_Y, &[a]);
    cpu.call(0x0606_750c, &[table.wrapping_add(k.wrapping_mul(12)), t]);
    cpu.call(SL_POP_MATRIX, &[]);
    cpu.write32(t + 8, 0);
    let n = cpu.read8(o + NUMBER);
    cpu.write8(t + S_NUMBER, n);
    let a = cpu.read16(o + LP_ROUND).wrapping_add(0x8000).wrapping_add(turn as u16);
    cpu.write16(t + S_ANGLE, a);
    fire(cpu, t);
}

/// A rocket from one of its fan muzzles (`k`, from 0x060a660c; bullets.rs's rocket_set_plain).
const LP_FAN_MUZZLES: u32 = 0x060a_660c;
/// Its side muzzles (from 0x060a666c).
const LP_SIDE_MUZZLES: u32 = 0x060a_666c;

pub fn last_part_fan_rocket(cpu: &mut Cpu, o: u32, k: u32, turn: u32) {
    lp_muzzle_shot(cpu, o, k, turn, LP_FAN_MUZZLES, |cpu, t| {
        nested!(cpu, 0x0605_5d84, bullets::rocket_set_plain(cpu, t));
    });
}

/// A rocket from one of its side muzzles (`k`, from 0x060a666c).
pub fn last_part_side_rocket(cpu: &mut Cpu, o: u32, k: u32, turn: u32) {
    lp_muzzle_shot(cpu, o, k, turn, LP_SIDE_MUZZLES, |cpu, t| {
        nested!(cpu, 0x0605_5d84, bullets::rocket_set_plain(cpu, t));
    });
}

/// A bullet from one of its side muzzles (`k`, from 0x060a666c; bullets.rs's bullet_set_plain).
pub fn last_part_side_bullet(cpu: &mut Cpu, o: u32, k: u32, turn: u32) {
    lp_muzzle_shot(cpu, o, k, turn, LP_SIDE_MUZZLES, |cpu, t| {
        nested!(cpu, 0x0605_3f48, bullets::bullet_set_plain(cpu, t));
    });
}

/// From frame 64 of its count, every 8 frames the next of its 8 fan muzzles fires a rocket, the
/// fan's turns narrowing to the middle and opening again the other way.
const LP_FAN_TURNS: [u16; 4] = [0x1c71, 0x1456, 0x0c3b, 0x041f];

pub fn last_part_fan(cpu: &mut Cpu, o: u32) {
    let c = cpu.read32(o + LP_COUNT);
    if c as i32 <= 63 || c & 7 != 0 {
        return;
    }
    let k = (c.wrapping_sub(64) as i32 >> 3) as u32;
    let turn = match k {
        0 | 7 => LP_FAN_TURNS[0],
        1 | 6 => LP_FAN_TURNS[1],
        2 | 5 => LP_FAN_TURNS[2],
        _ => LP_FAN_TURNS[3],
    };
    let turn = if k as i32 > 3 { turn.wrapping_neg() as i16 as i32 as u32 } else { turn as u32 };
    nested!(cpu, 0x0609_7d24, last_part_fan_rocket(cpu, o, k, turn));
}

/// Every 32 frames of its count a sweep of 6 bullets from its side muzzles, turned from 1/9 of a
/// turn one way to 1/9 the other: muzzles 0 to 5 on odd sweeps, 11 down to 6 on even.
const LP_SWEEP: [u16; 6] = [0x1c71, 0x1111, 0x05b0, 0xfa50, 0xeeef, 0xe38f];

pub fn last_part_sweep(cpu: &mut Cpu, o: u32) {
    let c = cpu.read32(o + LP_COUNT);
    if c & 31 != 0 {
        return;
    }
    let odd = (c as i32 >> 5) & 1 != 0;
    for (i, &turn) in LP_SWEEP.iter().enumerate() {
        let (k, turn) = if odd { (i as u32, turn) } else { (11 - i as u32, LP_SWEEP[5 - i]) };
        let turn = turn as i16 as i32 as u32;
        nested!(cpu, 0x0609_7e8c, last_part_side_bullet(cpu, o, k, turn));
    }
}

/// The sweep (last_part_sweep), from another of its index's functions.
pub fn last_part_sweep_again(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0609_7fc0, last_part_sweep(cpu, o));
}

/// From frame 32 of its count, every 8 frames the next of its 12 side muzzles fires a rocket,
/// turned from 1/6 of a turn in to the middle and out again the other way.
const LP_SIDE_TURNS: [u16; 6] = [0x2aaa, 0x22ea, 0x1b2a, 0x1369, 0x0ba9, 0x03e9];

pub fn last_part_side_rockets(cpu: &mut Cpu, o: u32) {
    let c = cpu.read32(o + LP_COUNT);
    if c as i32 <= 31 || c & 7 != 0 {
        return;
    }
    let k = (c.wrapping_sub(32) as i32 >> 3) as u32;
    let turn = match k {
        0..=5 => LP_SIDE_TURNS[k as usize],
        6..=11 => LP_SIDE_TURNS[11 - k as usize],
        _ => LP_SIDE_TURNS[5],
    };
    let turn = if k as i32 > 5 { turn.wrapping_neg() as i16 as i32 as u32 } else { turn as u32 };
    nested!(cpu, 0x0609_7dd8, last_part_side_rocket(cpu, o, k, turn));
}

/// The sweep (last_part_sweep), from a third of its index's functions.
pub fn last_part_sweep_third(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0609_7fc0, last_part_sweep(cpu, o));
}

/// Whether a point (x, y at `p`) is off the field: more than 184 dots either side or 120 up or
/// down.
const FIELD_X: i32 = 0xb8_0000;
const FIELD_Y: i32 = 0x78_0000;

pub fn off_field(cpu: &mut Cpu, p: u32) -> u32 {
    let y = cpu.read32(p + 4) as i32;
    let x = cpu.read32(p) as i32;
    (!(-FIELD_Y..=FIELD_Y).contains(&y) || !(-FIELD_X..=FIELD_X).contains(&x)) as u32
}

/// A bouncing shot (an actor): off the field it stops. Its bounces (+0x6c) up to the table's pick
/// (0x060a8910): past 104 dots up or down its y speed turns back, past 160 either side its x
/// speed (a bounce each). It flaps and faces its way, and moves by its speeds (+0x74, +0x78).
const BS_BOUNCES: u32 = 0x6c;
const BS_SPEED: u32 = 0x74;
const BS_EDGE_Y: u32 = 0x68_0000;
const BS_EDGE_X: u32 = 0xa0_0000;

pub fn bouncing_shot_behaviour(cpu: &mut Cpu, o: u32) {
    if nested!(cpu, 0x0609_8130, off_field(cpu, o + POSITION)) != 0 {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    let n = cpu.read8(o + BS_BOUNCES) as i8 as i32;
    if n < cpu.read32(LP_PICK) as i32 {
        for (k, edge) in [(1, BS_EDGE_Y), (0, BS_EDGE_X)] {
            if cpu.read32(o + POSITION + 4 * k).wrapping_add(edge) > 2 * edge {
                let n = cpu.read8(o + BS_BOUNCES).wrapping_add(1);
                cpu.write8(o + BS_BOUNCES, n);
                let v = cpu.read32(o + BS_SPEED + 4 * k).wrapping_neg();
                cpu.write32(o + BS_SPEED + 4 * k, v);
            }
        }
    }
    let vx = cpu.read32(o + BS_SPEED);
    nested!(cpu, 0x0605_6168, actors::flap_and_face(cpu, o, vx));
    for k in [0, 4] {
        let p = cpu.read32(o + POSITION + k).wrapping_add(cpu.read32(o + BS_SPEED + k));
        cpu.write32(o + POSITION + k, p);
    }
}

/// A shot bouncing inside the ring (an actor): off the field it stops. Its bounces (+0x6c) up to
/// the table's pick: once out past 112 dots from the middle (its +0x6d marking it out, cleared
/// back inside), its heading (+0x6e) is mirrored across the ring's (twice the way out, less
/// itself) and its speeds set from it by its speed (+0x70). It flaps, faces and moves as the
/// bouncing shot.
const RS_OUT: u32 = 0x6d;
const RS_HEADING: u32 = 0x6e;
const RS_SPEED: u32 = 0x70;
const RS_RING: i32 = 0x3100;

pub fn ring_shot_behaviour(cpu: &mut Cpu, o: u32) {
    if nested!(cpu, 0x0609_8130, off_field(cpu, o + POSITION)) != 0 {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    let n = cpu.read8(o + BS_BOUNCES) as i8 as i32;
    if n < cpu.read32(LP_PICK) as i32 {
        let (x, y) = (cpu.read32(o + POSITION), cpu.read32(o + POSITION + 4));
        let (xi, yi) = ((x >> 16) as i16 as i32, (y >> 16) as i16 as i32);
        if xi.wrapping_mul(xi).wrapping_add(yi.wrapping_mul(yi)) <= RS_RING {
            cpu.write8(o + RS_OUT, 0);
        } else if cpu.read8(o + RS_OUT) == 0 {
            let a = nested!(cpu, 0x0606_780c, sgl::sl_atan(cpu, x, y));
            let h = a.wrapping_neg().wrapping_mul(2).wrapping_sub(cpu.read16(o + RS_HEADING) as i16 as i32 as u32);
            let h = h as i16 as i32 as u32;
            cpu.write16(o + RS_HEADING, h as u16);
            let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, h));
            let v = cpu.read32(o + RS_SPEED);
            let vx = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, v, s));
            cpu.write32(o + BS_SPEED, vx);
            let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, h));
            let v = cpu.read32(o + RS_SPEED);
            let vy = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, v, c));
            cpu.write32(o + BS_SPEED + 4, vy);
            let n = cpu.read8(o + BS_BOUNCES).wrapping_add(1);
            cpu.write8(o + BS_BOUNCES, n);
            cpu.write8(o + RS_OUT, 1);
        }
    }
    let vx = cpu.read32(o + BS_SPEED);
    nested!(cpu, 0x0605_6168, actors::flap_and_face(cpu, o, vx));
    for k in [0, 4] {
        let p = cpu.read32(o + POSITION + k).wrapping_add(cpu.read32(o + BS_SPEED + k));
        cpu.write32(o + POSITION + k, p);
    }
}

/// A last boss's shot made (`d` its description: place, number, heading +0x2c, speed +0x30):
/// an actor (124 bytes, look 48), hit as a shot (flags 0x20000400, side 8, both hit masks), its
/// heading and speed kept (+0x6e, +0x70) and its speeds from them; no bounces yet. It, or 0.
const LSH_SIZE: u32 = 0x7c;
const LSH_LOOK: u32 = 48;
const LSH_FLAGS: u32 = 0x2000_0400;
const LSH_SIDE: u8 = 8;
const LSH_MASKS: [u32; 2] = [0x48, 0x4c];

pub fn last_shot_make(cpu: &mut Cpu, d: u32) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, LSH_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + 0x10));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HITS_AT));
    nested!(cpu, 0x0605_6114, parts::part_look(cpu, o, LSH_LOOK));
    let f = cpu.read32(o + FLAGS) | LSH_FLAGS;
    cpu.write32(o + FLAGS, f);
    let n = cpu.read8(d + S_NUMBER);
    cpu.write8(o + NUMBER, n);
    for k in [0, 4, 8] {
        let v = cpu.read32(d + k);
        cpu.write32(o + POSITION + k, v);
    }
    cpu.write8(o + NUMBER + 1, LSH_SIDE);
    for at in LSH_MASKS {
        cpu.write32(o + at, 0xffff_ffff);
    }
    let a = cpu.read16(d + S_ANGLE);
    cpu.write16(o + RS_HEADING, a);
    let v = cpu.read32(d + 0x30);
    cpu.write32(o + RS_SPEED, v);
    let a = a as i16 as i32 as u32;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    let v = cpu.read32(d + 0x30);
    let vx = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, v, s));
    cpu.write32(o + BS_SPEED, vx);
    let a = cpu.read16(d + S_ANGLE) as i16 as i32 as u32;
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    let v = cpu.read32(d + 0x30);
    let vy = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, v, c));
    cpu.write32(o + BS_SPEED + 4, vy);
    cpu.write8(o + BS_BOUNCES, 0);
    cpu.write8(o + RS_OUT, 0);
    o
}

/// A bouncing shot made (bouncing_shot_behaviour): it, or 0.
pub fn bouncing_shot_set(cpu: &mut Cpu, d: u32) -> u32 {
    let o = nested!(cpu, 0x0609_8324, last_shot_make(cpu, d));
    if o != 0 {
        cpu.write32(o + BEHAVIOUR, 0x0609_8174);
    }
    o
}

/// A shot bouncing inside the ring made (ring_shot_behaviour): it, or 0.
pub fn ring_shot_set(cpu: &mut Cpu, d: u32) -> u32 {
    let o = nested!(cpu, 0x0609_8324, last_shot_make(cpu, d));
    if o != 0 {
        cpu.write32(o + BEHAVIOUR, 0x0609_8224);
    }
    o
}

/// The last boss fighting (its step): its phase (+0x6d) by its script (0x0609870c moves it on),
/// its count (+0x80) to the phase's length (+0x98). Phases: 0, it starts (no life, side 1, its
/// turn's speed's aim 0, its turning part -1 and -1, its script from 0x060a7570 by the table's
/// pick); 1, it waits; 2, the next of its 4 parts (+0x70 their numbers, -1 gone) the way it
/// turns is woken (0x060996a4); 5, 6, 9, 11, 0x06098968, 0x0609899c, 0x060989d0, 0x060989fc to
/// their ends (the count at 1); 7, 0x06098e54, the bounces allowed the table's (none at its
/// end); 8, 0x06098a30 every 32 frames (16 for picks 3 and 4); 10, its arm's reach 96 dots by
/// its turn (+0xb0, +0xb4), 0x06098bd0 every 64 frames. Its turn's speed (+0x7a) eases by 2
/// toward its aim (+0x7c); its turn (+0x78) moves by it, shown at +0x30 and kept turned back at
/// 0x060a8914; then 0x0609879c.
const LF_PHASE: u32 = 0x6d;
const LF_LENGTH: u32 = 0x98;
const LF_SCRIPT: u32 = 0x84;
const LF_SCRIPTS: u32 = 0x060a_7570;
const LF_PARTS: u32 = 0x70;
const LF_TURN_SPEED: u32 = 0x7a;
const LF_TURN_AIM: u32 = 0x7c;
const LF_TURN: u32 = 0x78;
const LF_TURN_BACK: u32 = 0x060a_8914;
const LF_REACH: u32 = 0xb0;
const LF_REACH_LENGTH: u32 = 0x60_0000;
const LF_NEXT: u32 = 0x0609_870c;
const LF_AFTER: u32 = 0x0609_879c;
const LF_WAKE: u32 = 0x0609_96a4;
const LF_POSE_FIFTH: u32 = 0x0609_89fc;
const LF_SPIRAL: u32 = 0x0609_8e54;
const LF_RING: u32 = 0x0609_8a30;
const LF_ARM_SHOTS: u32 = 0x0609_8bd0;

pub fn last_boss_fighting(cpu: &mut Cpu, o: u32) {
    let next = |cpu: &mut Cpu| {
        nested!(cpu, LF_NEXT, last_script_next(cpu, o));
    };
    let count_to_length = |cpu: &mut Cpu| {
        let c = cpu.read32(o + LP_COUNT).wrapping_add(1);
        cpu.write32(o + LP_COUNT, c);
        c == cpu.read32(o + LF_LENGTH)
    };
    match cpu.read8(o + LF_PHASE) {
        0 => {
            cpu.write32(o + LP_LIFE, 0);
            cpu.write8(o + SIDE, 1);
            cpu.write16(o + LF_TURN_AIM, 0);
            cpu.write8(o + LF_PARTS - 2, 0xff);
            cpu.write8(o + LF_PARTS - 1, 0xff);
            cpu.write32(o + LP_COUNT, 0);
            let k = cpu.read32(LP_PICK_AT);
            let s = cpu.read32(LF_SCRIPTS.wrapping_add(k << 2));
            cpu.write32(o + LF_SCRIPT, s);
            next(cpu);
        }
        1 => {
            if count_to_length(cpu) {
                cpu.write32(o + LP_COUNT, 0);
                next(cpu);
            }
        }
        2 => {
            let at = o + LF_PARTS - 2;
            loop {
                let d: u8 = if cpu.read16(o + LF_TURN_SPEED) as i16 > 0 { 1 } else { 0xff };
                let b = cpu.read8(at).wrapping_add(d);
                cpu.write8(at, b);
                let b = cpu.read8(at) & 3;
                cpu.write8(at, b);
                let b = cpu.read8(at) as i8 as i32 as u32;
                if cpu.read8(o.wrapping_add(b) + LF_PARTS) != 0xff {
                    break;
                }
            }
            let b = cpu.read8(at) as i8 as i32 as u32;
            let n = cpu.read8(o.wrapping_add(b) + LF_PARTS) as i8 as i32 as u32;
            let part = cpu.read32(NUMBERED.wrapping_add(n << 2));
            nested!(cpu, LF_WAKE, last_ring_set(cpu, part));
            next(cpu);
        }
        p @ (5 | 6 | 9 | 11) => {
            match p {
                5 => nested!(cpu, 0x0609_8968, last_pose_second(cpu, o)),
                6 => nested!(cpu, 0x0609_899c, last_pose_third(cpu, o)),
                9 => nested!(cpu, 0x0609_89d0, last_pose_fourth(cpu, o)),
                _ => {
                    cpu.call(LF_POSE_FIFTH, &[o]);
                }
            }
            if cpu.read32(o + LP_COUNT) == LP_EASED {
                cpu.write32(o + LP_COUNT, 0);
                next(cpu);
            }
        }
        7 => {
            nested!(cpu, LF_SPIRAL, last_arms_volley(cpu, o));
            if count_to_length(cpu) {
                cpu.write32(LP_PICK, 0);
                next(cpu);
                cpu.write32(o + LP_COUNT, 0);
            } else {
                let k = cpu.read32(LP_PICK_AT);
                let v = cpu.read8(LP_PICKS.wrapping_add(k)) as i8 as i32 as u32;
                cpu.write32(LP_PICK, v);
            }
        }
        8 => {
            let k = cpu.read32(LP_PICK_AT) as i32;
            let mask = if (3..=4).contains(&k) { 15 } else { 31 };
            if cpu.read32(o + LP_COUNT) & mask == 0 {
                nested!(cpu, LF_RING, last_boss_missile(cpu, o));
            }
            if count_to_length(cpu) {
                cpu.write32(o + LP_COUNT, 0);
                next(cpu);
            }
        }
        10 => {
            let a = cpu.read16(o + LF_TURN) as i16 as i32 as u32;
            let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
            let x = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, s, LF_REACH_LENGTH));
            cpu.write32(o + LF_REACH, x);
            let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
            let y = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, c, LF_REACH_LENGTH));
            cpu.write32(o + LF_REACH + 4, y.wrapping_neg());
            if cpu.read32(o + LP_COUNT) & 63 == 0 {
                nested!(cpu, LF_ARM_SHOTS, last_arm_ribbon(cpu, o, 0));
            }
            if count_to_length(cpu) {
                cpu.write32(o + LP_COUNT, 0);
                next(cpu);
            }
        }
        _ => {}
    }
    let v = cpu.read16(o + LF_TURN_SPEED) as i16;
    let aim = cpu.read16(o + LF_TURN_AIM) as i16;
    if v < aim {
        cpu.write16(o + LF_TURN_SPEED, v.wrapping_add(2) as u16);
    } else if v > aim {
        cpu.write16(o + LF_TURN_SPEED, v.wrapping_sub(2) as u16);
    }
    let a = cpu.read16(o + LF_TURN).wrapping_add(cpu.read16(o + LF_TURN_SPEED));
    cpu.write16(o + LF_TURN, a);
    cpu.write16(o + LP_FACING, a);
    let a = cpu.read16(o + LF_TURN).wrapping_neg();
    cpu.write16(LF_TURN_BACK, a);
    nested!(cpu, LF_AFTER, last_boss_matrix(cpu, o));
}

/// The last boss's script on a step (`last_boss_fighting` calls it at its phases' ends): its
/// script's index (+0x6f) one on; the row there (12 bytes from the script at +0x84): a phase
/// (+0x6d; -1 ends the script: the boss is beaten), the turn's aim (+0x7c, a word), the phase's
/// length (+0x98) and a second value (+0x9c).
const LF_SCRIPT_AT: u32 = 0x6f;
const LF_ROW: u32 = 12;
const LF_VALUE: u32 = 0x9c;

pub fn last_script_next(cpu: &mut Cpu, o: u32) {
    let i = cpu.read8(o + LF_SCRIPT_AT).wrapping_add(1);
    cpu.write8(o + LF_SCRIPT_AT, i);
    let row = cpu.read32(o + LF_SCRIPT).wrapping_add((i as i8 as i32 as u32).wrapping_mul(LF_ROW));
    let phase = cpu.read8(row);
    if phase as i8 == -1 {
        nested!(cpu, LB_BEATEN, last_boss_beaten(cpu, o));
        return;
    }
    cpu.write8(o + LF_PHASE, phase);
    let aim = cpu.read16(row + 2);
    cpu.write16(o + LF_TURN_AIM, aim);
    let length = cpu.read32(row + 4);
    cpu.write32(o + LF_LENGTH, length);
    let value = cpu.read32(row + 8);
    cpu.write32(o + LF_VALUE, value);
}

/// Its turn (+0x78) made into a matrix kept at +0xb8 (SGL's matrix stack pushed and popped
/// round it): what its parts turn and place by.
const LF_MATRIX: u32 = 0xb8;

pub fn last_boss_matrix(cpu: &mut Cpu, o: u32) {
    cpu.call(SL_PUSH_MATRIX, &[]);
    let a = cpu.read16(o + LF_TURN) as i16 as i32 as u32;
    cpu.call(SL_ROT_Z, &[a]);
    cpu.call(SL_GET_MATRIX, &[o + LF_MATRIX]);
    cpu.call(SL_POP_MATRIX, &[]);
}

/// The boss's poses blended: a 16-point pose (20 bytes each: x, y, z longs and three words) at
/// 0x060a87d0 and a 13-point one (two longs each) at 0x060a8920 made by easing (math::ease) the
/// poses at `a` and `b` by its count (+0x80, up 1/128 a frame to 1: sound 99 at its start, 98 at
/// its end), and `c` and `d` likewise.
const LPB_STEP: u32 = 0x200;
const LPB_END: u32 = 0x1_0000;
const LPB_START_SOUND: u32 = 99;
const LPB_END_SOUND: u32 = 98;
const LPB_POSE: u32 = 0x060a_87d0;
const LPB_POSE_B: u32 = 0x060a_8920;
const LPB_POINTS: u32 = 16;
const LPB_POINTS_B: u32 = 13;

pub fn last_pose_blend(cpu: &mut Cpu, o: u32, a: u32, b: u32, c: u32, d: u32) {
    let t = cpu.stack();
    cpu.write32(t, a);
    cpu.write32(t + 4, b);
    cpu.write32(t + 8, c);
    if cpu.read32(o + LP_COUNT) == 0 {
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, LPB_START_SOUND));
    }
    let n = cpu.read32(o + LP_COUNT).wrapping_add(LPB_STEP);
    cpu.write32(o + LP_COUNT, n);
    if n as i32 > LPB_END as i32 {
        cpu.write32(o + LP_COUNT, LPB_END);
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, LPB_END_SOUND));
    }
    let t_now = cpu.read32(o + LP_COUNT);
    for k in 0..LPB_POINTS {
        let at = 20 * k;
        for i in [0, 4, 8] {
            let (from, to) = (cpu.read32(a + at + i), cpu.read32(b + at + i));
            let v = nested!(cpu, 0x0606_2ebc, math::ease(cpu, from, to, t_now));
            cpu.write32(LPB_POSE + at + i, v);
        }
        for i in [12, 14, 16] {
            let (from, to) = (cpu.read16(a + at + i) as i16 as i32 as u32, cpu.read16(b + at + i) as i16 as i32 as u32);
            let v = nested!(cpu, 0x0606_2ebc, math::ease(cpu, from, to, t_now));
            cpu.write16(LPB_POSE + at + i, v as u16);
        }
    }
    for k in 0..LPB_POINTS_B {
        let at = 8 * k;
        for i in [0, 4] {
            let (from, to) = (cpu.read32(c + at + i), cpu.read32(d + at + i));
            let v = nested!(cpu, 0x0606_2ebc, math::ease(cpu, from, to, t_now));
            cpu.write32(LPB_POSE_B + at + i, v);
        }
    }
}

/// The poses (0x060a6088, 0x060a61c8, 0x060a6308, 0x060a6448: the 16-point ones; 0x060a75c8,
/// 0x060a7630: the 13-point ones) it blends between, by phase: 0x06098934 the first to the
/// second, 0x06098968 the second to the third (phase 5), 0x0609899c the third to the second
/// (phase 6), 0x060989d0 the third to the fourth (phase 9), 0x060989fc the fourth to the second
/// (phase 11). The 13-point poses go the one way or the other.
fn last_pose_blend_of(cpu: &mut Cpu, o: u32, a: u32, b: u32, c: u32, d: u32) {
    static PROLOGUE: std::sync::OnceLock<super::Prologue> = std::sync::OnceLock::new();
    cpu.nested_with_stack(&PROLOGUE, 0x0609_87e4, &[d], |cpu| last_pose_blend(cpu, o, a, b, c, d));
}

pub fn last_pose_first(cpu: &mut Cpu, o: u32) {
    last_pose_blend_of(cpu, o, 0x060a_6088, 0x060a_61c8, 0x060a_75c8, 0x060a_7630);
}

pub fn last_pose_second(cpu: &mut Cpu, o: u32) {
    last_pose_blend_of(cpu, o, 0x060a_61c8, 0x060a_6308, 0x060a_7630, 0x060a_75c8);
}

pub fn last_pose_third(cpu: &mut Cpu, o: u32) {
    last_pose_blend_of(cpu, o, 0x060a_6308, 0x060a_61c8, 0x060a_75c8, 0x060a_7630);
}

pub fn last_pose_fourth(cpu: &mut Cpu, o: u32) {
    last_pose_blend_of(cpu, o, 0x060a_6308, 0x060a_6448, 0x060a_75c8, 0x060a_75c8);
}

/// A missile out along its turn (+0x78): from 128 dots out (x by its sine, y by minus its cosine),
/// pointing that way half round (a plain missile, bullets.rs's missile_set_plain), numbered as it.
const LM_REACH: u32 = 0x80_0000;

pub fn last_boss_missile(cpu: &mut Cpu, o: u32) {
    let t = cpu.stack();
    cpu.write32(t + 8, 0);
    let n = cpu.read8(o + NUMBER);
    cpu.write8(t + S_NUMBER, n);
    let a = cpu.read16(o + LF_TURN) as i16 as i32 as u32;
    cpu.write16(t + S_ANGLE, (a as u16).wrapping_add(0x8000));
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    let x = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, s, LM_REACH));
    cpu.write32(t, x);
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    let y = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, c, LM_REACH));
    cpu.write32(t + 4, y.wrapping_neg());
    nested!(cpu, 0x0605_4b60, bullets::missile_set_plain(cpu, t));
}

/// A ribbon of the boss's arm (its behaviour): a hit end (its hits, +0x56, bit 0x400) ends it.
/// Within 120 dots of the middle it is in (+0x252 cleared); out of it, and not yet turned (+0x252
/// 0) with a turn left (+0x250, counted down), its heading (+0x2e) is mirrored across the ring's
/// way out (minus twice the way, less itself) and +0x252 set; its reach (+0x7c) grows by 3/16
/// dot to its end (+0x254) and the ribbon moves (ribbons.rs).
const LR_BOUNCES: u32 = 0x250;
const LR_OUT: u32 = 0x252;
const LR_REACH_END: u32 = 0x254;
const LR_RADIUS: i32 = 0x3840;
const LR_GROW: u32 = 0x3000;
const LR_REACH: u32 = 0x7c;
const LR_HEADING: u32 = 0x2e;
const LR_END: u32 = 0x400;

pub fn last_ribbon_behaviour(cpu: &mut Cpu, o: u32) {
    if cpu.read16(o + HIT_FLAGS) & LR_END as u16 != 0 {
        nested!(cpu, 0x0606_3940, ribbons::ribbon_end(cpu, o));
        return;
    }
    let (x, y) = (cpu.read32(o + POSITION), cpu.read32(o + POSITION + 4));
    let (xi, yi) = ((x >> 16) as i16 as i32, (y >> 16) as i16 as i32);
    let out = xi.wrapping_mul(xi).wrapping_add(yi.wrapping_mul(yi)) > LR_RADIUS;
    let mut moves = true;
    if out {
        if cpu.read16(o + LR_OUT) == 0 {
            if cpu.read16(o + LR_BOUNCES) == 0 {
                nested!(cpu, 0x0606_3940, ribbons::ribbon_end(cpu, o));
                moves = false;
            } else {
                let a = nested!(cpu, 0x0606_780c, sgl::sl_atan(cpu, x, y));
                let h = a.wrapping_neg().wrapping_mul(2).wrapping_sub(cpu.read16(o + LR_HEADING) as i16 as i32 as u32);
                cpu.write16(o + LR_HEADING, h as u16);
                let n = cpu.read16(o + LR_BOUNCES).wrapping_sub(1);
                cpu.write16(o + LR_BOUNCES, n);
                cpu.write16(o + LR_OUT, 1);
            }
        }
    } else {
        cpu.write16(o + LR_OUT, 0);
    }
    if moves {
        let r = cpu.read32(o + LR_REACH).wrapping_add(LR_GROW);
        cpu.write32(o + LR_REACH, r);
        let end = cpu.read32(o + LR_REACH_END);
        if r as i32 > end as i32 {
            cpu.write32(o + LR_REACH, end);
        }
        nested!(cpu, 0x0606_3c20, ribbons::ribbon_move(cpu, o));
    }
}

/// An arm's ribbon made at the description `d` (an actor of 620 bytes, ribbon_init's with the
/// arm's behaviour), its turns left (+0x6e) from `bounces`. It, or 0.
const LRM_SIZE: u32 = 0x26c;
const LRM_BOUNCES: u32 = 0x6e;

pub fn last_ribbon_make(cpu: &mut Cpu, d: u32, bounces: u32) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, LRM_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return 0;
    }
    let r = nested!(cpu, 0x0606_3620, ribbons::ribbon_init(cpu, o, d, 0x0609_8aac));
    cpu.write8(o + LRM_BOUNCES, bounces as u8);
    r
}

/// A shot from the boss's arm: its ribbon (0x06098b74), a flare (effects.rs) and sound 41 at the
/// arm's reach (+0xb0, +0xb4), numbered as it; it goes `heading` (unless 0) or toward the nearer
/// player (aim::target at 16 frames' delay). Its speed (+0x30) and its +0x2e come by the table's
/// pick (0x060a8988) from 0x060a7584 and 0x060a5fe0, its +0x38 is 1, its +0x3c the arm's reach
/// and its +0x20 the sprite size's 0x060741fc plus 64.
const LAS_REACH: u32 = 0xb0;
const LAS_TARGET_DELAY: u32 = 16;
const LAS_SPEEDS: u32 = 0x060a_7584;
const LAS_REACHES: u32 = 0x060a_5fe0;
const LAS_SIZE_AT: u32 = 0x0607_41fc;
const LAS_SIZE_MORE: u16 = 64;
const LAS_SOUND: u32 = 41;

pub fn last_arm_ribbon(cpu: &mut Cpu, o: u32, heading: u32) {
    let t = cpu.stack();
    let heading = heading as i16 as i32 as u32;
    let (x, y) = (cpu.read32(o + LAS_REACH), cpu.read32(o + LAS_REACH + 4));
    cpu.write32(t, x);
    cpu.write32(t + 4, y);
    cpu.write32(t + 8, 0);
    let n = cpu.read8(o + NUMBER);
    cpu.write8(t + S_NUMBER, n);
    if heading != 0 {
        cpu.write16(t + S_ANGLE, heading as u16);
    } else {
        let p = t + 64;
        nested!(cpu, 0x0604_0ccc, aim::target(cpu, o, LAS_TARGET_DELAY, p));
        let (px, py) = (cpu.read32(p), cpu.read32(p + 4));
        let (dx, dy) = (px.wrapping_sub(cpu.read32(t)), py.wrapping_sub(cpu.read32(t + 4)));
        cpu.write32(p, dx);
        cpu.write32(p + 4, dy);
        let a = nested!(cpu, 0x0606_780c, sgl::sl_atan(cpu, dy.wrapping_neg(), dx));
        cpu.write16(t + S_ANGLE, a as u16);
    }
    let pick = cpu.read32(LP_PICK_AT);
    let v = cpu.read32(LAS_SPEEDS.wrapping_add(pick << 2));
    cpu.write32(t + S_SPEED, v);
    let v = cpu.read8(LAS_REACHES.wrapping_add(pick)) as i8 as i16 as u16;
    cpu.write16(t + S_FLAGS, v);
    cpu.write16(t + 0x38, 1);
    cpu.write32(t + 0x3c, o + LAS_REACH);
    let s = cpu.read16(LAS_SIZE_AT).wrapping_add(LAS_SIZE_MORE);
    cpu.write16(t + 0x20, s);
    nested!(cpu, 0x0609_8b74, last_ribbon_make(cpu, t, 0));
    nested!(cpu, 0x0606_43ec, effects::flare_set(cpu, t));
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, LAS_SOUND));
}

/// A pair of ring shots (the bouncing ones', bullets bouncing inside the ring: 0x06098440) from
/// the two muzzles at 0x060a7598 and 0x060a75a4 through the boss's turn (+0x78) and a quarter
/// turn (1/4 of a turn, by arm `i`) more each: each its owner's number, a speed of 1/2 to 1 (at
/// random) and a heading back from the turn by up to 3/8 of a turn either side less 3/16 (in
/// 1/128ths), then a glow at each (explosions.rs's glow_set, its +0x2c long 0xe000).
const LAM_MUZZLES: [u32; 2] = [0x060a_7598, 0x060a_75a4];

pub fn last_arm_shots(cpu: &mut Cpu, o: u32, i: u32) {
    let t = cpu.stack();
    let d = [t, t + 64];
    cpu.write32(t + 0x80, o + LF_TURN);
    cpu.call(SL_PUSH_MATRIX, &[]);
    let spin = ((i << 8) >> 2) << 8;
    let turn = cpu.read16(o + LF_TURN) as i16 as i32 as u32;
    let a = turn.wrapping_add(spin) as u16 as i16 as i32 as u32;
    cpu.call(SL_ROT_Z, &[a]);
    for (k, m) in LAM_MUZZLES.into_iter().enumerate() {
        cpu.call(0x0606_750c, &[m, d[k]]);
    }
    cpu.call(SL_POP_MATRIX, &[]);
    cpu.write32(d[1] + 8, 0);
    cpu.write32(d[0] + 8, 0);
    let n = cpu.read8(o + NUMBER);
    cpu.write8(d[1] + S_NUMBER, n);
    cpu.write8(d[0] + S_NUMBER, n);
    for at in d {
        let r = nested!(cpu, 0x0603_4dd4, sgl::sl_random(cpu));
        cpu.write32(at + S_SPEED, (r & LPS_SPEED_BITS).wrapping_add(LPS_SPEED));
    }
    let turn = cpu.read16(o + LF_TURN) as i16 as i32 as u32;
    let a = turn.wrapping_add(spin).wrapping_neg() as u16;
    cpu.write16(d[1] + S_ANGLE, a);
    cpu.write16(d[0] + S_ANGLE, a);
    for at in d {
        let r = nested!(cpu, 0x0603_4dd4, sgl::sl_random(cpu));
        let v = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, r, LPS_SPREAD));
        let v = v.wrapping_add(LPS_SPREAD_LESS) & LPS_SPREAD_STEP;
        let a = (cpu.read16(at + S_ANGLE) as i16 as i32 as u32).wrapping_add(v);
        cpu.write16(at + S_ANGLE, a as u16);
    }
    for at in d {
        nested!(cpu, 0x0609_8440, ring_shot_set(cpu, at));
    }
    for at in d {
        cpu.write32(at + S_ANGLE, LPS_GLOW);
        nested!(cpu, 0x0605_8f34, explosions::glow_set(cpu, at));
    }
}

/// Its 4 parts' shots (0x06098cb4) from each part still there (their numbers at +0x70: -1 gone).
pub fn last_arms_fire(cpu: &mut Cpu, o: u32) {
    for i in 0..4 {
        if cpu.read8(o + LF_PARTS + i) != 0xff {
            nested!(cpu, 0x0609_8cb4, last_arm_shots(cpu, o, i));
        }
    }
}

/// Phase 7, its parts' shots every 128 frames of its count (sound 97).
const LAV_SOUND: u32 = 97;

pub fn last_arms_volley(cpu: &mut Cpu, o: u32) {
    if cpu.read32(o + LP_COUNT) & 127 == 0 {
        nested!(cpu, 0x0609_8df0, last_arms_fire(cpu, o));
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, LAV_SOUND));
    }
}

/// One of the boss's arms (an actor; +0x70 the boss, +0x6c its index, +0x6d its step): it is
/// placed by the boss's matrix (+0xb8) at its index's point of the pose (0x060a87d0, 20 bytes a
/// point) and turned by its own (+0x6e) and the boss's (+0x78). The boss gone, it stops. Step 0:
/// its hit marks its byte of 0x060a6064 (a byte a pair, from +1) and its life (+0x50) goes to the
/// boss's list (+0x88 by index); with its life gone (but the first), its time (+0x74) is cleared,
/// the stage's rate 25 more, the kill scored (10000) and it goes to step 1. Step 1, for 60 frames:
/// every 4 frames sparks at its place (0x0609ad20, the 4's bit passed), then a burst there
/// (0x0609acd8, its turn) and the marks' byte put back from 0x060a6084, its number struck from
/// the boss's list (-1) and it stops.
const LA_INDEX: u32 = 0x6c;
const LA_STEP: u32 = 0x6d;
const LA_TURN_OFFSET: u32 = 0x6e;
const LA_BOSS: u32 = 0x70;
const LA_TIME: u32 = 0x74;
const LA_MARKS: u32 = 0x060a_6065;
const LA_POINT: u32 = 20;
const LA_KILL_RATE: u32 = 25;
const LA_KILL_POINTS: u32 = 10000;
const LA_BURST_AT: u32 = 60;

pub fn last_arm_behaviour(cpu: &mut Cpu, o: u32) {
    let t = cpu.stack();
    let boss = cpu.read32(o + LA_BOSS);
    if cpu.read32(boss + BEHAVIOUR) != 0x0609_7628 {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    cpu.call(SL_PUSH_MATRIX, &[]);
    cpu.call(SL_LOAD_MATRIX, &[boss + LF_MATRIX]);
    let k = cpu.read8(o + LA_INDEX) as i8 as i32 as u32;
    cpu.call(0x0606_750c, &[LPB_POSE + LA_POINT * k, o + POSITION]);
    cpu.call(SL_POP_MATRIX, &[]);
    let turn = cpu.read16(o + LA_TURN_OFFSET).wrapping_add(cpu.read16(boss + LF_TURN));
    cpu.write16(o + LP_FACING, turn);
    match cpu.read8(o + LA_STEP) {
        0 => {
            if cpu.read16(o + HIT_FLAGS) & 1 != 0 {
                let k = cpu.read8(o + LA_INDEX) as i8 as i32 as u32;
                cpu.write8(LA_MARKS.wrapping_add(k << 1), 1);
            }
            let k = cpu.read8(o + LA_INDEX) as i8 as i32 as u32;
            let life = cpu.read32(o + LP_LIFE);
            cpu.write32(boss.wrapping_add(k << 2) + LP_LIVES, life);
            if cpu.read8(o + LA_INDEX) == 0 || life as i32 > 0 {
                return;
            }
            cpu.write32(o + LA_TIME, 0);
            let r = cpu.read32(STAGE_RATE_AT).wrapping_add(LA_KILL_RATE);
            cpu.write32(STAGE_RATE_AT, r);
            let by = cpu.read32(o + HIT_BY);
            let (player, weapon) = (cpu.read8(by + HIT_PLAYER) as i8 as i32 as u32, cpu.read8(by + HIT_WEAPON) as i8 as i32 as u32);
            nested!(cpu, 0x0604_3108, hits::score_kill(cpu, player, weapon, o, LA_KILL_POINTS));
            cpu.write8(o + LA_STEP, 1);
        }
        1 => {
            let c = cpu.read32(o + LA_TIME);
            if c & 3 == 0 {
                for i in [0, 4, 8] {
                    let v = cpu.read32(o + POSITION + i);
                    cpu.write32(t + i, v);
                }
                let c = cpu.read32(o + LA_TIME);
                nested!(cpu, LP_SPARKS, last_spark_make(cpu, t, c & 4));
            }
            let c = cpu.read32(o + LA_TIME);
            cpu.write32(o + LA_TIME, c.wrapping_add(1));
            if c == LA_BURST_AT {
                for i in [0, 4, 8] {
                    let v = cpu.read32(o + POSITION + i);
                    cpu.write32(t + 12 + i, v);
                }
                let a = cpu.read16(o + LP_FACING) as i16 as i32 as u32;
                nested!(cpu, LP_BURST, last_burst(cpu, t + 12, a));
                let k = cpu.read8(o + LA_INDEX) as i8 as i32 as u32;
                let m = cpu.read8(LP_MARKS_AGAIN.wrapping_add(k));
                cpu.write8(LP_MARKS.wrapping_add(k << 1), m);
                let k = cpu.read8(o + LA_INDEX) as i8 as i32 as u32;
                cpu.write8(boss.wrapping_add(k) + LF_PARTS, 0xff);
                cpu.write8(o + OBJ_MODE, TO_STOP);
            }
        }
        _ => {}
    }
}

/// What an arm draws as: nothing.
pub fn last_arm_draw(_cpu: &mut Cpu, _o: u32) {}

/// The way an arm's pose turns it (a word, by its index): 1 a quarter back, 2 half, 3 a quarter
/// on; any other none.
pub fn last_arm_turn(_cpu: &mut Cpu, i: u32) -> u32 {
    match i as i32 {
        1 => 0xc000,
        2 => 0x8000,
        3 => 0x4000,
        _ => 0,
    }
}

/// An arm made for the boss `boss` (index `i`): an actor of 120 bytes with a number of the
/// enemies', 0x06098e8c its behaviour, nothing drawn; at its index's point of the pose by the
/// boss's matrix, side 1, hit by 0x060a6016, its life the boss's list's (+0x88 by index), turned
/// by its index's way (0x0609935c) from the boss's turn. Its number, or -1 with the boss's life
/// for it gone, no number or no room.
const LAM_SIZE: u32 = 0x78;
const LAM_FLAGS: u32 = 0xc0_0000;
const LAM_HIT_TABLE: u32 = 0x060a_6016;

pub fn last_arm_make(cpu: &mut Cpu, boss: u32, i: u32) -> u32 {
    let t = cpu.stack();
    cpu.write32(t, i << 2);
    cpu.write32(t + 4, boss.wrapping_add(i << 2));
    if (cpu.read32(boss.wrapping_add(i << 2) + LP_LIVES) as i32) <= 0 {
        return u32::MAX;
    }
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, 2)) as i8;
    if n == -1 {
        return u32::MAX;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, LAM_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, NUMBERED_EXEC));
    if o == 0 {
        return u32::MAX;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + 0x10));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HITS_AT));
    cpu.write32(o + BEHAVIOUR, 0x0609_8e8c);
    let f = cpu.read32(o + FLAGS) | LAM_FLAGS;
    cpu.write32(o + FLAGS, f);
    cpu.write32(o + MODEL_DRAW, 0x0609_9024);
    cpu.write8(o + NUMBER, n as u8);
    let k = cpu.read8(o + NUMBER) as i8 as i32 as u32;
    cpu.write32(NUMBERED.wrapping_add(k << 2), o);
    cpu.call(SL_PUSH_MATRIX, &[]);
    let a = cpu.read16(boss + LF_TURN) as i16 as i32 as u32;
    cpu.call(SL_ROT_Z, &[a]);
    let point = LPB_POSE + LA_POINT * i;
    cpu.write32(t, point);
    cpu.call(0x0606_750c, &[point, o + POSITION]);
    cpu.call(SL_POP_MATRIX, &[]);
    cpu.write32(o + POSITION + 8, 0);
    cpu.write8(o + BYTE_33, 0);
    cpu.write8(o + SIDE, 1);
    cpu.write32(o + HITS_AT, LAM_HIT_TABLE);
    let life = cpu.read32(boss.wrapping_add(i << 2) + LP_LIVES);
    cpu.write32(o + LP_LIFE, life);
    let way = nested!(cpu, 0x0609_935c, last_arm_turn(cpu, i));
    cpu.write16(o + LA_TURN_OFFSET, way as u16);
    let turn = way.wrapping_add(cpu.read16(boss + LF_TURN) as i16 as i32 as u32);
    cpu.write16(o + LP_FACING, turn as u16);
    cpu.write32(o + LA_BOSS, boss);
    cpu.write8(o + LA_INDEX, i as u8);
    cpu.write8(o + LA_STEP, 0);
    n as i32 as u32
}

/// A part's follower (an actor; +0x74 the boss, +0x70 its index, +0x6c its own turn): placed by
/// the boss's matrix at its index's point of the pose (0x060a87d0) and turned by its own, the
/// boss's (+0x78) and the point's third word. The boss gone, it stops.
const LF_FOLLOW_TURN: u32 = 0x6c;
const LF_FOLLOW_INDEX: u32 = 0x70;
const LF_FOLLOW_BOSS: u32 = 0x74;

pub fn last_follower_behaviour(cpu: &mut Cpu, o: u32) {
    let boss = cpu.read32(o + LF_FOLLOW_BOSS);
    if cpu.read32(boss + BEHAVIOUR) != 0x0609_7628 {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    cpu.call(SL_PUSH_MATRIX, &[]);
    cpu.call(SL_LOAD_MATRIX, &[boss + LF_MATRIX]);
    let point = LPB_POSE + LA_POINT * cpu.read32(o + LF_FOLLOW_INDEX);
    cpu.call(0x0606_750c, &[point, o + POSITION]);
    cpu.call(SL_POP_MATRIX, &[]);
    let turn = cpu.read16(o + LF_FOLLOW_TURN).wrapping_add(cpu.read16(boss + LF_TURN));
    let point = LPB_POSE + LA_POINT * cpu.read32(o + LF_FOLLOW_INDEX);
    let turn = turn.wrapping_add(cpu.read16(point + 16));
    cpu.write16(o + LP_FACING, turn);
}

/// A follower made for the boss `boss` (index `i`): an actor of 120 bytes numbered as the boss,
/// 0x0609918c its behaviour, nothing drawn; at its index's point of the pose by the boss's
/// matrix, side 1, hit by the table `hits`, its life the boss's list's (+0x88 by index), its own
/// turn `turn` (+0x6c) and (+0x30) that and the boss's. Its number (a byte, sign-extended), or 0
/// with no room.
pub fn last_follower_make(cpu: &mut Cpu, boss: u32, i: u32, hits: u32, turn: u32) -> u32 {
    let t = cpu.stack();
    cpu.write32(t, hits);
    let turn = turn as i16 as i32 as u32;
    cpu.write32(t + 4, turn);
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, LAM_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + 0x10));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HITS_AT));
    cpu.write32(o + BEHAVIOUR, 0x0609_918c);
    let f = cpu.read32(o + FLAGS) | LAM_FLAGS;
    cpu.write32(o + FLAGS, f);
    cpu.write32(o + MODEL_DRAW, 0x0609_9024);
    let n = cpu.read8(boss + NUMBER);
    cpu.write8(o + NUMBER, n);
    cpu.call(SL_PUSH_MATRIX, &[]);
    let a = cpu.read16(boss + LF_TURN) as i16 as i32 as u32;
    cpu.call(SL_ROT_Z, &[a]);
    let point = LPB_POSE + LA_POINT * i;
    cpu.call(0x0606_750c, &[point, o + POSITION]);
    let own = boss.wrapping_add(i << 2);
    cpu.call(SL_POP_MATRIX, &[]);
    cpu.write32(o + POSITION + 8, 0);
    cpu.write8(o + BYTE_33, 0);
    cpu.write8(o + SIDE, 1);
    cpu.write32(o + HITS_AT, hits);
    let life = cpu.read32(own + LP_LIVES);
    cpu.write32(o + LP_LIFE, life);
    cpu.write16(o + LF_FOLLOW_TURN, turn as u16);
    let sum = turn.wrapping_add(cpu.read16(boss + LF_TURN) as i16 as i32 as u32);
    cpu.write32(t + 4, sum);
    cpu.write16(o + LP_FACING, sum as u16);
    cpu.write32(o + LF_FOLLOW_BOSS, boss);
    cpu.write32(o + LF_FOLLOW_INDEX, i);
    cpu.read8(o + NUMBER) as i8 as i32 as u32
}

/// A ring (an actor, one of the boss's expanding rings; +0x6c what it comes from, +0x70 its step,
/// +0x74 its speed, +0x78 its radius, +0x7c the radius of its next mark, +0x80 its count) placed
/// on the circle of its radius round it (x less, y more: its sine and cosine) at the turn (+0x30)
/// of its source's boss (+0x30) and its own offset (+0x6e).
pub fn last_ring_place(cpu: &mut Cpu, o: u32) {
    let source = cpu.read32(o + LRG_SOURCE);
    let boss = cpu.read32(source + LA_BOSS);
    let a = cpu.read16(boss + LP_FACING).wrapping_add(cpu.read16(source + LA_TURN_OFFSET)) as i16 as i32 as u32;
    cpu.write16(o + LP_FACING, a as u16);
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    let r = cpu.read32(o + LRG_RADIUS);
    let v = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, r, s));
    let at = cpu.read32(o + LRG_SOURCE);
    let x = cpu.read32(at + POSITION).wrapping_sub(v);
    cpu.write32(o + POSITION, x);
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    let r = cpu.read32(o + LRG_RADIUS);
    let v = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, r, c));
    let at = cpu.read32(o + LRG_SOURCE);
    let y = cpu.read32(at + POSITION + 4).wrapping_add(v);
    cpu.write32(o + POSITION + 4, y);
}

/// A ring's behaviour: it stops when its source does (a mode past 111). Its sprite's size
/// alternates (the character base's word plus 32 or 48 by the frame count's bit 0) and its frame
/// (0x060a75c0 by bit 1). Steps (+0x70): 0, 30 frames, sound 101 and unhidden (flags 0x80000 and
/// 0x8000000 cleared); 1, the next ring made (0x060997d4), its next mark 12 dots out, count 0;
/// 2, its radius grows by its speed (up by 1/16 dot a frame) to 248 dots (352 when the boss's
/// part two along is gone), making a ring (0x060997d4) at each mark (every 24 dots more); 3, 30
/// frames; 4, its scale (+0x20) down by 1/16 a frame to nothing, then it stops. Then it is placed
/// (last_ring_place).
const LRG_SOURCE: u32 = 0x6c;
const LRG_STEP: u32 = 0x70;
const LRG_SPEED: u32 = 0x74;
const LRG_RADIUS: u32 = 0x78;
const LRG_MARK: u32 = 0x7c;
const LRG_FRAMES: u32 = 0x060a_75c0;
const LRG_SOUND: u32 = 101;
const LRG_UNHIDE: u32 = 0x7ff7_ffff;
const LRG_WAIT: u32 = 30;
const LRG_MARK_FIRST: u32 = 0xc_0000;
const LRG_MARK_STEP: u32 = 0x18_0000;
const LRG_SPEED_STEP: u32 = 0x1000;
const LRG_LIMIT: i32 = 0xf8_0000;
const LRG_LIMIT_OPEN: i32 = 0x160_0000;
const LRG_SHRINK: u32 = 0xffff_f000;

pub fn last_ring_behaviour(cpu: &mut Cpu, o: u32) {
    let source = cpu.read32(o + LRG_SOURCE);
    if cpu.read8(source + OBJ_MODE) as i8 as i32 > 111 {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    let size = cpu.read16(LAS_SIZE_AT).wrapping_add(if cpu.read32(GAME_FRAMES) & 1 != 0 { 32 } else { 48 });
    cpu.write16(o + COLOUR, size);
    cpu.write16(o + OWN_COLOUR, size);
    let bit = (cpu.read32(GAME_FRAMES) & 2) >> 1;
    let frame = cpu.read8(LRG_FRAMES + bit);
    cpu.write8(o + SPRITE_FRAME, frame);
    let step = cpu.read8(o + LRG_STEP);
    let next = |cpu: &mut Cpu| {
        let s = cpu.read8(o + LRG_STEP).wrapping_add(1);
        cpu.write8(o + LRG_STEP, s);
    };
    match step {
        0 => {
            let c = cpu.read32(o + LP_COUNT).wrapping_add(1);
            cpu.write32(o + LP_COUNT, c);
            if c == LRG_WAIT {
                nested!(cpu, 0x0600_50d4, sound::se_set(cpu, LRG_SOUND));
                let f = cpu.read32(o + FLAGS) & LRG_UNHIDE;
                cpu.write32(o + FLAGS, f);
                next(cpu);
            }
        }
        1 => {
            nested!(cpu, 0x0609_97d4, last_mark_make(cpu, o));
            let m = cpu.read32(o + LRG_MARK).wrapping_add(LRG_MARK_FIRST);
            cpu.write32(o + LRG_MARK, m);
            cpu.write32(o + LP_COUNT, 0);
            next(cpu);
        }
        2 => {
            let k = cpu.read8(source + LA_INDEX) as i8 as i32 as u32;
            let boss = cpu.read32(source + LA_BOSS);
            let gone = cpu.read8(boss.wrapping_add(k.wrapping_add(2) & 3) + LF_PARTS) == 0xff;
            let limit = if gone { LRG_LIMIT_OPEN } else { LRG_LIMIT };
            let r = cpu.read32(o + LRG_RADIUS);
            if r as i32 >= limit {
                next(cpu);
            } else {
                let v = cpu.read32(o + LRG_SPEED);
                cpu.write32(o + LRG_RADIUS, r.wrapping_add(v));
                cpu.write32(o + LRG_SPEED, v.wrapping_add(LRG_SPEED_STEP));
                if cpu.read32(o + LRG_RADIUS) as i32 >= cpu.read32(o + LRG_MARK) as i32 {
                    nested!(cpu, 0x0609_97d4, last_mark_make(cpu, o));
                    let m = cpu.read32(o + LRG_MARK).wrapping_add(LRG_MARK_STEP);
                    cpu.write32(o + LRG_MARK, m);
                }
            }
        }
        3 => {
            let c = cpu.read32(o + LP_COUNT).wrapping_add(1);
            cpu.write32(o + LP_COUNT, c);
            if c == LRG_WAIT {
                next(cpu);
            }
        }
        4 => {
            let v = cpu.read32(o + 0x20).wrapping_add(LRG_SHRINK);
            cpu.write32(o + 0x20, v);
            if v as i32 > 0 {
                cpu.write32(o + 0x24, v);
            } else {
                cpu.write32(o + 0x20, 0);
                cpu.write8(o + OBJ_MODE, TO_STOP);
                return;
            }
        }
        _ => {}
    }
    nested!(cpu, 0x0609_93a0, last_ring_place(cpu, o));
}

/// A ring made for an arm (an actor of 132 bytes; the arm's number, +0x6c the arm; 0x0609941c its
/// behaviour; flags 0xa0080000 and 0x20): at the arm's place... not set, at its own origin, its
/// depth 0, the character's 6th, a scale of 1 (+0x20, +0x24), the sprite's kind 59 and frame by
/// the frame count's bit 1 (0x060a75c0), size as the character base's word plus 32, hit by
/// 0x060a75b0 (side 1, weapon 5, both masks all), a radius and next mark of 36 dots, speed and
/// count 0, step 0; its flash (0x06099ba0) made and it placed (0x060993a0).
const LRS_SIZE: u32 = 0x84;
const LRS_FLAGS: u32 = 0xa008_0000;
const LRS_FLAGS_MORE: u32 = 0x20;
const LRS_KIND: u8 = 59;
const LRS_HITS: u32 = 0x060a_75b0;
const LRS_RADIUS: u32 = 0x24_0000;
const LRS_DEPTH: u8 = 6;
const LRS_SCALE: u32 = 0x1_0000;

pub fn last_ring_set(cpu: &mut Cpu, arm: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, LRS_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + 0x10));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HITS_AT));
    let f = cpu.read32(o + FLAGS) | LRS_FLAGS;
    cpu.write32(o + FLAGS, f);
    cpu.write32(o + BEHAVIOUR, 0x0609_941c);
    let n = cpu.read8(arm + NUMBER);
    cpu.write8(o + NUMBER, n);
    cpu.write32(o + POSITION + 8, 0);
    cpu.write8(o + BYTE_33, LRS_DEPTH);
    cpu.write32(o + 0x24, LRS_SCALE);
    cpu.write32(o + 0x20, LRS_SCALE);
    let f = cpu.read32(o + FLAGS) | LRS_FLAGS_MORE;
    cpu.write32(o + FLAGS, f);
    let bit = (cpu.read32(GAME_FRAMES) & 2) >> 1;
    let frame = cpu.read8(LRG_FRAMES + bit);
    cpu.write8(o + SPRITE_FRAME, frame);
    cpu.write8(o + SPRITE_KIND, LRS_KIND);
    let size = cpu.read16(LAS_SIZE_AT).wrapping_add(32);
    cpu.write16(o + OWN_COLOUR, size);
    cpu.write16(o + COLOUR, size);
    cpu.write32(o + HITS_AT, LRS_HITS);
    cpu.write8(o + SIDE, 1);
    cpu.write8(o + HIT_WEAPON, 5);
    cpu.write32(o + 0x48, 0xffff_ffff);
    cpu.write32(o + 0x4c, 0xffff_ffff);
    cpu.write32(o + LRG_SOURCE, arm);
    cpu.write32(o + LRG_RADIUS, LRS_RADIUS);
    cpu.write32(o + LRG_MARK, LRS_RADIUS);
    cpu.write32(o + LRG_SPEED, 0);
    cpu.write8(o + LRG_STEP, 0);
    cpu.write32(o + LP_COUNT, 0);
    nested!(cpu, 0x0609_9ba0, last_ring_flash_make(cpu, o));
    nested!(cpu, 0x0609_93a0, last_ring_place(cpu, o));
}

/// A ring's mark made from the ring `ring` (an actor of 120 bytes, flags 0x20000000, behaviour
/// 0x060995b0): numbered and placed as it, depth 5, the sprite (kind 0, frame 0xae for the first
/// radius, 36 dots; else kind 59 and a frame of 0x060a75c4 by the frame count's bit 0), size as
/// the character base's plus 32, hit by 0x060a75b0 (side 1, weapon 5, both masks all), its scale,
/// source (+0x6c) and speed... as the ring's (+0x20, +0x6c, the ring itself at +0x70 and its
/// mark's radius at +0x74).
const LMM_FLAGS: u32 = 0x2000_0000;
const LMM_FRAMES: u32 = 0x060a_75c4;
const LMM_FIRST: u32 = 0x24_0000;
const LMM_FIRST_FRAME: u8 = 0xae;

pub fn last_mark_make(cpu: &mut Cpu, ring: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, LAM_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + 0x10));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HITS_AT));
    let f = cpu.read32(o + FLAGS) | LMM_FLAGS;
    cpu.write32(o + FLAGS, f);
    cpu.write32(o + BEHAVIOUR, 0x0609_95b0);
    let n = cpu.read8(ring + NUMBER);
    cpu.write8(o + NUMBER, n);
    for i in [0, 4] {
        let v = cpu.read32(ring + POSITION + i);
        cpu.write32(o + POSITION + i, v);
    }
    cpu.write32(o + POSITION + 8, 0);
    cpu.write8(o + BYTE_33, 5);
    if cpu.read32(ring + LRG_MARK) == LMM_FIRST {
        cpu.write8(o + SPRITE_KIND, 0);
        cpu.write8(o + SPRITE_FRAME, LMM_FIRST_FRAME);
    } else {
        cpu.write8(o + SPRITE_KIND, LRS_KIND);
        let bit = cpu.read32(GAME_FRAMES) & 1;
        let frame = cpu.read8(LMM_FRAMES + bit);
        cpu.write8(o + SPRITE_FRAME, frame);
    }
    let size = cpu.read16(LAS_SIZE_AT).wrapping_add(32);
    cpu.write16(o + OWN_COLOUR, size);
    cpu.write16(o + COLOUR, size);
    cpu.write32(o + HITS_AT, LRS_HITS);
    cpu.write8(o + SIDE, 1);
    cpu.write8(o + HIT_WEAPON, 5);
    cpu.write32(o + 0x48, 0xffff_ffff);
    cpu.write32(o + 0x4c, 0xffff_ffff);
    let v = cpu.read32(ring + 0x20);
    cpu.write32(o + 0x20, v);
    let source = cpu.read32(ring + LRG_SOURCE);
    cpu.write32(o + LRG_SOURCE, source);
    cpu.write32(o + LRG_STEP, ring); // the ring itself, in the field that is a ring's step
    let mark = cpu.read32(ring + LRG_MARK);
    cpu.write32(o + LRG_SPEED, mark);
}

/// A flash made for the ring `ring` (an actor of 80 bytes, plain, flags 0x20000000, behaviour
/// 0x0609997c): numbered as it, at its source's place, its depth the ring's one on, sprite
/// frame 0xd3 of kind 0, size as the character base's plus 32, +0x48 and +0x4c 0x8000, the ring
/// at +0x3c; its +0x40 and +0x44 cleared; sound 69.
const LFM_SIZE: u32 = 0x50;
const LFM_SOUND: u32 = 69;
const LFM_FRAME: u8 = 0xd3;

pub fn last_ring_flash_make(cpu: &mut Cpu, ring: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, LFM_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, PLAIN_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + 0x10));
    let f = cpu.read32(o + FLAGS) | LMM_FLAGS;
    cpu.write32(o + FLAGS, f);
    cpu.write32(o + BEHAVIOUR, 0x0609_997c);
    let n = cpu.read8(ring + NUMBER);
    cpu.write8(o + NUMBER, n);
    let source = cpu.read32(ring + LRG_SOURCE);
    let x = cpu.read32(source + POSITION);
    cpu.write32(o + POSITION, x);
    let y = cpu.read32(source + POSITION + 4);
    cpu.write32(o + POSITION + 4, y);
    cpu.write32(o + POSITION + 8, 0);
    let d = cpu.read8(ring + BYTE_33).wrapping_add(1);
    cpu.write8(o + BYTE_33, d);
    cpu.write8(o + SPRITE_FRAME, LFM_FRAME);
    cpu.write8(o + SPRITE_KIND, 0);
    let size = cpu.read16(LAS_SIZE_AT).wrapping_add(32);
    cpu.write16(o + OWN_COLOUR, size);
    cpu.write16(o + COLOUR, size);
    cpu.write32(o + 0x48, 0x8000);
    cpu.write32(o + 0x4c, 0x8000);
    cpu.write32(o + HITS_AT, ring);
    cpu.write32(o + 0x44, 0);
    cpu.write32(o + 0x40, 0);
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, LFM_SOUND));
}

/// A ring's mark (an actor made by last_mark_make; +0x6c what its ring comes from, +0x70 the
/// ring, +0x74 its radius): it stops when its source does (a mode past 111). Its sprite's size
/// alternates (the character base's plus 32 or 48 by the frame count's bit 0), and unless its
/// kind is 0 its frame is 0x060a75c4's by bit 1. It is placed as its ring (last_ring_place) but
/// by its own radius; its scale (+0x20) is the ring's, and it stops when that is 0.
pub fn last_mark_behaviour(cpu: &mut Cpu, o: u32) {
    let source = cpu.read32(o + LRG_SOURCE);
    if cpu.read8(source + OBJ_MODE) as i8 as i32 > 111 {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    let size = cpu.read16(LAS_SIZE_AT).wrapping_add(if cpu.read32(GAME_FRAMES) & 1 != 0 { 32 } else { 48 });
    cpu.write16(o + COLOUR, size);
    cpu.write16(o + OWN_COLOUR, size);
    if cpu.read8(o + SPRITE_KIND) != 0 {
        let bit = (cpu.read32(GAME_FRAMES) & 2) >> 1;
        let frame = cpu.read8(LMM_FRAMES + bit);
        cpu.write8(o + SPRITE_FRAME, frame);
    }
    let source = cpu.read32(o + LRG_SOURCE);
    let boss = cpu.read32(source + LA_BOSS);
    let a = cpu.read16(boss + LP_FACING).wrapping_add(cpu.read16(source + LA_TURN_OFFSET)) as i16 as i32 as u32;
    cpu.write16(o + LP_FACING, a as u16);
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    let r = cpu.read32(o + LRG_SPEED);
    let v = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, r, s));
    let at = cpu.read32(o + LRG_SOURCE);
    let x = cpu.read32(at + POSITION).wrapping_sub(v);
    cpu.write32(o + POSITION, x);
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    let r = cpu.read32(o + LRG_SPEED);
    let v = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, r, c));
    let at = cpu.read32(o + LRG_SOURCE);
    let y = cpu.read32(at + POSITION + 4).wrapping_add(v);
    cpu.write32(o + POSITION + 4, y);
    let ring = cpu.read32(o + LRG_STEP);
    let scale = cpu.read32(ring + 0x20);
    cpu.write32(o + 0x20, scale);
    if scale == 0 {
        cpu.write8(o + OBJ_MODE, TO_STOP);
    }
}

/// A ring (rings.rs's ring_set) round the object `p`: flickering, following it (its place at
/// +0x14), 48 more than the character base's word at +2, a scale of 1/2 growing by 1/16, an
/// inner circle of 48 dots widening by 1 dot a frame and an outer of 80 dots, its owner `p`.
const LFR_SCALE: u32 = 0x8000;
const LFR_SCALE_RATE: u32 = 0x1000;
const LFR_INNER: u32 = 0x30_0000;
const LFR_OUTER: u32 = 0x50_0000;
const LFR_INNER_RATE: u32 = 0x1_0000;

pub fn last_flash_ring(cpu: &mut Cpu, p: u32) {
    let t = cpu.stack();
    let (x, y) = (cpu.read32(p + POSITION), cpu.read32(p + POSITION + 4));
    cpu.write32(t + 12, x);
    cpu.write32(t + 16, y);
    cpu.write32(t + 20, 0);
    for k in [4, 6, 8] {
        cpu.write16(t + k, 0);
    }
    cpu.write32(t + 32, LFR_SCALE_RATE);
    cpu.write32(t + 36, LFR_SCALE_RATE);
    let a = cpu.read16(LAS_SIZE_AT).wrapping_add(48);
    cpu.write16(t + 2, a);
    cpu.write8(t, 1);
    cpu.write32(t + 24, LFR_SCALE);
    cpu.write32(t + 28, LFR_SCALE);
    cpu.write32(t + 44, LFR_OUTER);
    cpu.write32(t + 40, LFR_INNER);
    cpu.write32(t + 48, LFR_INNER_RATE);
    cpu.write32(t + 52, 0);
    cpu.write32(t + 56, p);
    cpu.write32(t + 60, p + POSITION);
    cpu.write32(t + 64, 0);
    nested!(cpu, 0x0606_2738, rings::ring_set(cpu, t));
}

/// A ring's flash (an actor made by last_ring_flash_make; +0x3c its ring, +0x40 its step, +0x48
/// and +0x4c its two scales, +0x20 and +0x24 theirs plus a random flicker): it stops when its ring
/// does. Step 0: both scales grow by 1/128; once the ring is in its step 1 they are 4 and 1/2,
/// its frame 0xd3, its turn the ring's boss's half round, a ring made (0x060998f4), the screen's
/// fades stopped, its colour offset set to 40 and faded back to 0 at 1/128: step 1. Step 1: its turn so; the first scale
/// down by 3/16, the second up by 1/32; with that at 1 (and the turn cleared) step 2; and with
/// the ring in its step 4 one on. Step 2: one on with the ring in its step 4. Step 3: both scales
/// the ring's (+0x20). Then it is placed on the arm's circle (a radius of 28 dots) and its flags'
/// bit 8 and size flicker with the frame count (its scales more by 3 times a random 0 to 15, by
/// 1/256, and 0x2000 more on odd frames).
const LFL_STEP: u32 = 0x40;
const LFL_SCALES: u32 = 0x48;
const LFL_GROW: u32 = 0x200;
const LFL_FIRST: u32 = 0x4_0000;
const LFL_SECOND: u32 = 0x8000;
const LFL_FRAME: u8 = 0xd3;
const LFL_FADE: u32 = 40;
const LFL_FADE_SPEED: u32 = 0x200;
const LFL_FIRST_DOWN: u32 = 0xffff_d000;
const LFL_SECOND_UP: u32 = 0x800;
const LFL_FIRST_END: u32 = 0x1_0000;
const LFL_REACH: u32 = 0x1c_0000;
const LFL_FLICKER: u32 = 0x100;
const LFL_ODD_MORE: u32 = 0x2000;

pub fn last_flash_behaviour(cpu: &mut Cpu, o: u32) {
    let ring = cpu.read32(o + HITS_AT);
    if cpu.read8(ring + OBJ_MODE) as i8 as i32 > 111 {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    let step = cpu.read32(o + LFL_STEP);
    let ring_step = |cpu: &mut Cpu| {
        let ring = cpu.read32(o + HITS_AT);
        cpu.read8(ring + LRG_STEP)
    };
    let again = |cpu: &mut Cpu| {
        let s = cpu.read32(o + LFL_STEP).wrapping_add(1);
        cpu.write32(o + LFL_STEP, s);
    };
    let turned = |cpu: &mut Cpu| {
        let ring = cpu.read32(o + HITS_AT);
        let a = cpu.read16(ring + LP_FACING).wrapping_add(0x8000);
        cpu.write16(o + LP_FACING, a);
    };
    if step <= 3 {
        match step {
            0 => {
                for at in [LFL_SCALES, LFL_SCALES + 4] {
                    let v = cpu.read32(o + at).wrapping_add(LFL_GROW);
                    cpu.write32(o + at, v);
                }
                let ring = cpu.read32(o + HITS_AT);
                if cpu.read8(ring + LRG_STEP) == 1 {
                    cpu.write32(o + LFL_SCALES, LFL_FIRST);
                    cpu.write32(o + LFL_SCALES + 4, LFL_SECOND);
                    cpu.write8(o + SPRITE_FRAME, LFL_FRAME);
                    turned(cpu);
                    nested!(cpu, 0x0609_98f4, last_flash_ring(cpu, o));
                    nested!(cpu, 0x0605_e9d8, screen::stop_fades(cpu));
                    nested!(cpu, 0x0605_e6dc, screen::set_fade_offset(cpu, LFL_FADE, LFL_FADE, LFL_FADE));
                    nested!(cpu, 0x0605_e93c, screen::fade_to(cpu, 0, 0, 0, LFL_FADE_SPEED));
                    again(cpu);
                }
            }
            1 => {
                turned(cpu);
                let v = cpu.read32(o + LFL_SCALES).wrapping_add(LFL_FIRST_DOWN);
                cpu.write32(o + LFL_SCALES, v);
                let v = cpu.read32(o + LFL_SCALES + 4).wrapping_add(LFL_SECOND_UP);
                cpu.write32(o + LFL_SCALES + 4, v);
                if v == LFL_FIRST_END {
                    again(cpu);
                    cpu.write16(o + LP_FACING, 0);
                }
                if ring_step(cpu) == 4 {
                    again(cpu);
                }
            }
            2 => {
                if ring_step(cpu) == 4 {
                    again(cpu);
                }
            }
            _ => {
                let ring = cpu.read32(o + HITS_AT);
                let v = cpu.read32(ring + 0x20);
                cpu.write32(o + LFL_SCALES + 4, v);
                cpu.write32(o + LFL_SCALES, v);
            }
        }
    }
    let ring = cpu.read32(o + HITS_AT);
    let arm = cpu.read32(ring + LRG_SOURCE);
    let boss = cpu.read32(arm + LA_BOSS);
    let a = cpu.read16(boss + LP_FACING).wrapping_add(cpu.read16(arm + LA_TURN_OFFSET)) as i16 as i32 as u32;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    let v = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, LFL_REACH, s));
    let ring = cpu.read32(o + HITS_AT);
    let arm = cpu.read32(ring + LRG_SOURCE);
    let x = cpu.read32(arm + POSITION).wrapping_sub(v);
    cpu.write32(o + POSITION, x);
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    let v = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, LFL_REACH, c));
    let ring = cpu.read32(o + HITS_AT);
    let arm = cpu.read32(ring + LRG_SOURCE);
    let y = cpu.read32(arm + POSITION + 4).wrapping_add(v);
    cpu.write32(o + POSITION + 4, y);
    let r = nested!(cpu, 0x0603_4dd4, sgl::sl_random(cpu)) & 15;
    let mut flick = r.wrapping_mul(3) << 8;
    if cpu.read32(GAME_FRAMES) & 1 != 0 {
        let f = cpu.read32(o + FLAGS) | LFL_FLICKER;
        cpu.write32(o + FLAGS, f);
        flick = flick.wrapping_add(LFL_ODD_MORE);
    } else {
        let f = cpu.read32(o + FLAGS) & !LFL_FLICKER;
        cpu.write32(o + FLAGS, f);
    }
    let size = cpu.read16(LAS_SIZE_AT).wrapping_add(if cpu.read32(GAME_FRAMES) & 2 != 0 { 32 } else { 48 });
    cpu.write16(o + COLOUR, size);
    cpu.write16(o + OWN_COLOUR, size);
    let v = cpu.read32(o + LFL_SCALES).wrapping_add(flick);
    cpu.write32(o + 0x20, v);
    let v = cpu.read32(o + LFL_SCALES + 4).wrapping_add(flick);
    cpu.write32(o + 0x24, v);
}

// ---- the last boss's flying pieces and its wreck -------------------------------------------------

/// A piece that flies the blended pose's circle (made by 0x0609a218): its boss (+0x44), how far
/// along the pose's 13 points it is (+0x48: the point's number in the high word, how far on to the
/// next in the low), how fast it goes on (+0x4c), its count (+0x50), its tag (+0x3d) and its frame
/// number (+0x3e), its turn (+0x40, a word).
const LQ_BOSS: u32 = 0x44;
const LQ_ALONG: u32 = 0x48;
const LQ_SPEED: u32 = 0x4c;
const LQ_COUNT: u32 = 0x50;
const LQ_TAG: u32 = 0x3d;
const LQ_FRAME_AT: u32 = 0x3e;
const LQ_TURN: u32 = 0x40;
const LQ_BOSS_MODE: u32 = 0x9c;
const LQ_BEHAVIOUR: u32 = 0x0609_9da0;
const LQ_ANIMATION: u32 = 0x060a_76b0;
const LQ_TURNS: u32 = 0x060a_7698;
const LQ_PATH_END: u32 = 0xc_0000;
const LQ_PATH_MOST: i32 = 0xb_ffff;

/// The point of the blended pose's 13 (0x060a8920, two longs each) that `along` stands at: each
/// coordinate from the point's to the next's by the low word.
fn last_path_point(cpu: &mut Cpu, along: u32) -> (u32, u32) {
    let i = (along >> 16) as i16 as i32 as u32;
    let (at, next) = (LPB_POSE_B.wrapping_add(i << 3), LPB_POSE_B.wrapping_add((i.wrapping_add(1)) << 3));
    let t = along & 0xffff;
    let (a, b) = (cpu.read32(at), cpu.read32(next));
    let x = nested!(cpu, 0x0606_2e7c, math::lerp(cpu, a, b, t));
    let (a, b) = (cpu.read32(at + 4), cpu.read32(next + 4));
    let y = nested!(cpu, 0x0606_2e7c, math::lerp(cpu, a, b, t));
    (x, y)
}

/// A piece placed: its point of the pose turned by its boss's turn (+0x78).
pub fn last_piece_place(cpu: &mut Cpu, o: u32) {
    let along = cpu.read32(o + LQ_ALONG);
    let (u, v) = last_path_point(cpu, along);
    let boss = cpu.read32(o + LQ_BOSS);
    let a = cpu.read16(boss + LF_TURN) as i16 as i32 as u32;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    let p = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, u, c));
    let q = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, v, s));
    cpu.write32(o + POSITION, p.wrapping_sub(q));
    let p = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, u, s));
    let q = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, v, c));
    cpu.write32(o + POSITION + 4, p.wrapping_add(q));
}

/// A piece's frame, on each odd frame: its number (+0x3e) counts down (up when its tag, +0x3d,
/// is 0), wrapping 0 to 5 and 6 to 0; its sprite frame (+0x2f) is the table's for it.
pub fn last_piece_animate(cpu: &mut Cpu, o: u32) {
    if cpu.read32(GAME_FRAMES) & 1 == 0 {
        return;
    }
    let down = cpu.read8(o + LQ_TAG) as i8 != 0;
    let k = cpu.read8(o + LQ_FRAME_AT) as i8 as i32;
    cpu.write8(o + LQ_FRAME_AT, (if down { k - 1 } else { k + 1 }) as u8);
    let k = cpu.read8(o + LQ_FRAME_AT) as i8 as i32;
    if k < 0 {
        cpu.write8(o + LQ_FRAME_AT, 5);
    } else if k == 6 {
        cpu.write8(o + LQ_FRAME_AT, 0);
    }
    let k = cpu.read8(o + LQ_FRAME_AT) as i8 as i32 as u32;
    let f = cpu.read8(LQ_ANIMATION.wrapping_add(k));
    cpu.write8(o + SPRITE_FRAME, f);
}

/// A piece's behaviour once made (it comes in as 0x06099e94: 0x06099da0 after): with its boss no
/// longer the last boss it ends in a burst of sparks (a random angle, speed 1 1/8). Its place on the
/// path goes on by its speed (round the 12 points, 0xc0000 long); placed. With its boss's mode
/// 1 its turn is the table's by its point plus the boss's, and every 32 frames it fires
/// (0x0609a13c). Animated; its count up.
pub fn last_piece_behaviour(cpu: &mut Cpu, o: u32) {
    let boss = cpu.read32(o + LQ_BOSS);
    if cpu.read32(boss + BEHAVIOUR) != 0x0609_7628 {
        let t = cpu.stack();
        for k in [0, 4, 8] {
            let v = cpu.read32(o + POSITION + k);
            cpu.write32(t + actors::E_POSITION + k, v);
        }
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0));
        cpu.write16(t + actors::E_ANGLE, r as u16);
        cpu.write32(t + actors::E_SPEED, 0x1_2000);
        nested!(cpu, 0x0605_786c, explosions::spark_burst(cpu, t));
        cpu.write8(o + OBJ_MODE, TO_STOP);
    }
    let along = cpu.read32(o + LQ_ALONG).wrapping_add(cpu.read32(o + LQ_SPEED));
    cpu.write32(o + LQ_ALONG, along);
    if along as i32 > 0 {
        if along as i32 > LQ_PATH_MOST {
            cpu.write32(o + LQ_ALONG, along & 0xffff);
        }
    } else {
        cpu.write32(o + LQ_ALONG, along.wrapping_add(LQ_PATH_END));
    }
    nested!(cpu, 0x0609_9c7c, last_piece_place(cpu, o));
    let boss = cpu.read32(o + LQ_BOSS);
    if cpu.read32(boss + LQ_BOSS_MODE) == 1 {
        let i = cpu.read16(o + LQ_ALONG) as i16 as i32 as u32;
        let turn = cpu.read16(boss + LF_TURN) as i16 as i32 as u32;
        let a = cpu.read16(LQ_TURNS.wrapping_add(i << 1)) as i16 as i32 as u32;
        cpu.write16(o + LQ_TURN, a.wrapping_add(turn) as u16);
        if cpu.read32(o + LQ_COUNT) & 31 == 0 {
            nested!(cpu, 0x0609_a13c, last_piece_fire(cpu, o));
        }
    }
    nested!(cpu, 0x0609_9d44, last_piece_animate(cpu, o));
    let n = cpu.read32(o + LQ_COUNT).wrapping_add(1);
    cpu.write32(o + LQ_COUNT, n);
}

/// A piece swinging out as it is made (0x06099e94): its count up by 1/256 a frame, its place the
/// path's turned by its boss's turn and by the count's ease from 3 to 1 (its point scaled
/// by it); at a count of 1 the count goes back to 0 and it is the behaviour 0x06099da0. Animated.
const LQS_COUNT_STEP: u32 = 0x100;
const LQS_FROM: u32 = 0x3_0000;
const LQS_TO: u32 = 0x1_0000;

pub fn last_piece_swing(cpu: &mut Cpu, o: u32) {
    let n = cpu.read32(o + LQ_COUNT).wrapping_add(LQS_COUNT_STEP);
    cpu.write32(o + LQ_COUNT, n);
    let along = cpu.read32(o + LQ_ALONG);
    let (u, v) = last_path_point(cpu, along);
    let boss = cpu.read32(o + LQ_BOSS);
    let a = cpu.read16(boss + LF_TURN) as i16 as i32 as u32;
    let count = cpu.read32(o + LQ_COUNT);
    let e = nested!(cpu, 0x0606_2ebc, math::ease(cpu, LQS_FROM, LQS_TO, count));
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    let x = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, s, e));
    let count = cpu.read32(o + LQ_COUNT);
    let e = nested!(cpu, 0x0606_2ebc, math::ease(cpu, LQS_FROM, LQS_TO, count));
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    let y = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, c, e));
    let p = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, u, y));
    let q = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, v, x));
    cpu.write32(o + POSITION, p.wrapping_sub(q));
    let p = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, u, x));
    let q = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, v, y));
    cpu.write32(o + POSITION + 4, p.wrapping_add(q));
    if cpu.read32(o + LQ_COUNT) == LQS_TO {
        cpu.write32(o + LQ_COUNT, 0);
        cpu.write32(o + BEHAVIOUR, LQ_BEHAVIOUR);
    }
    nested!(cpu, 0x0609_9d44, last_piece_animate(cpu, o));
}

/// Whether the point `p` (x, y) is off the pose's hull, after turning it back (0x060a8914) and
/// holding it against the blended pose's 13 points (their corners 2, 5, 8 and 11, 4 dots out):
/// 1 or 0.
const LO_MARGIN: u32 = 0x4_0000;

pub fn last_pose_outside(cpu: &mut Cpu, p: u32) -> u32 {
    let a = cpu.read16(LF_TURN_BACK) as i16 as i32 as u32;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    let (x, y) = (cpu.read32(p), cpu.read32(p + 4));
    let m = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, x, c));
    let n = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, y, s));
    let u = m.wrapping_sub(n) as i32;
    let m = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, x, s));
    let n = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, y, c));
    let w = m.wrapping_add(n) as i32;
    let at = |cpu: &mut Cpu, k: u32| cpu.read32(LPB_POSE_B + k) as i32;
    let margin = LO_MARGIN as i32;
    if (u < at(cpu, 16).wrapping_sub(margin) && w < at(cpu, 20).wrapping_sub(margin))
        || (u < at(cpu, 40).wrapping_sub(margin) && w > at(cpu, 44).wrapping_add(margin))
        || (u > at(cpu, 64).wrapping_add(margin) && w > at(cpu, 68).wrapping_add(margin))
        || (u > at(cpu, 88).wrapping_add(margin) && w < at(cpu, 92).wrapping_sub(margin))
    {
        1
    } else {
        0
    }
}

/// A burst of smoke (drift_effect's, colour `colour`, its animation 0x06073174, a random angle,
/// speed 1/2, scales 1/4) at the piece's place.
pub fn last_piece_spark(cpu: &mut Cpu, o: u32, colour: u32) {
    let t = cpu.stack();
    cpu.write32(t + actors::E_SCRIPT, 0x0607_3174);
    cpu.write16(t + actors::E_COLOUR, colour as u16);
    for k in [0, 4, 8] {
        let v = cpu.read32(o + POSITION + k);
        cpu.write32(t + actors::E_POSITION + k, v);
    }
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write16(t + actors::E_ANGLE, r as u16);
    cpu.write32(t + actors::E_SPEED, 0x8000);
    cpu.write32(t + actors::E_SCALES, 0x4000);
    cpu.write32(t + actors::E_SCALES + 4, 0x4000);
    nested!(cpu, 0x0605_7454, actors::drift_effect(cpu, t));
}

/// A piece's bullet's behaviour: out of the pose's hull (0x06099fc4) it ends in smoke
/// (colour 0x40, 0x0609a09c), else it flies on (bullets.rs's bullet_move).
pub fn last_piece_move(cpu: &mut Cpu, o: u32) {
    if nested!(cpu, 0x0609_9fc4, last_pose_outside(cpu, o + POSITION)) != 0 {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        nested!(cpu, 0x0609_a09c, last_piece_spark(cpu, o, 0x40));
    } else {
        nested!(cpu, 0x0605_3ccc, bullets::bullet_move(cpu, o));
    }
}

/// A piece's shot at the nearer player (a shot struct on the stack: its number the piece's,
/// its place the piece's): when the player is more than 0x18ff dots squared away (in whole dots)
/// it is aimed 0x7000 times a random of up to a turn, less 0x3800, rounded to 0x800, from the
/// piece's turn (+0x40), speed 1/2; with its tag (+0x3d) set a bullet (bullets.rs's bullet_set,
/// no flags) of behaviour 0x0609a0f8, else 0x0609a3b0's.
const LPF_RANGE: i32 = 0x18ff;
const LPF_SPREAD: u32 = 0x7000;
const LPF_SPREAD_LOW: u32 = 0xffff_c800;
const LPF_SPREAD_ROUND: u32 = 0xffff_f800;
const LPF_SPEED: u32 = 0x8000;
const LPF_FLAGS: u32 = 0x2e;

pub fn last_piece_fire(cpu: &mut Cpu, o: u32) {
    let t = cpu.stack();
    let (x, y) = (cpu.read32(o + POSITION), cpu.read32(o + POSITION + 4));
    cpu.write32(t + S_POSITION, x);
    cpu.write32(t + S_POSITION + 4, y);
    cpu.write32(t + S_POSITION + 8, 0);
    let n = cpu.read8(o + NUMBER);
    cpu.write8(t + S_NUMBER, n);
    let p = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + 0x10));
    let dx = (cpu.read32(p + POSITION).wrapping_sub(x) >> 16) as i16 as i32;
    let dy = (cpu.read32(p + POSITION + 4).wrapping_sub(y) >> 16) as i16 as i32;
    if dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) <= LPF_RANGE {
        return;
    }
    let a = cpu.read16(o + LQ_TURN);
    cpu.write16(t + S_ANGLE, a);
    let r = nested!(cpu, 0x0603_4dd4, sgl::sl_random(cpu));
    let m = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, r, LPF_SPREAD));
    let spread = m.wrapping_add(LPF_SPREAD_LOW) & LPF_SPREAD_ROUND;
    let a = cpu.read16(t + S_ANGLE).wrapping_add(spread as u16);
    cpu.write16(t + S_ANGLE, a);
    cpu.write32(t + AS_SPEED, LPF_SPEED);
    if cpu.read8(o + LQ_TAG) as i8 != 0 {
        cpu.write16(t + LPF_FLAGS, 0);
        let b = nested!(cpu, 0x0605_3db0, bullets::bullet_set(cpu, t));
        if b != 0 {
            cpu.write32(b + BEHAVIOUR, 0x0609_a0f8);
        }
    } else {
        nested!(cpu, 0x0609_a3b0, last_piece_bolt_make(cpu, t));
    }
}

/// A piece's bolt made from a shot struct (a plain actor of 116 bytes, its number the shot's, its
/// place the shot's, behaviour 0x0609a300): flags 0x20000400, scales 1/2, a random turn of its
/// own (+0x30), side 1... its frame 0xc6 of kind 8 (+0x33 is 8, +0x2e 0), its colour the base's
/// plus 32, its boxes 0x060a76b6, type 5 (+0x55), no owner (+0x48, +0x4c: -1), its speed (+0x6c, +0x70)
/// the shot's (+0x30) along its angle (+0x2c).
const LPB_BOLT_SIZE: u32 = 116;
const LPB_BOLT_BOXES: u32 = 0x060a_76b6;
const LPB_BOLT_BEHAVIOUR: u32 = 0x0609_a300;
const LPB_BOLT_SPEED: u32 = 0x6c;

pub fn last_piece_bolt_make(cpu: &mut Cpu, t: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, LPB_BOLT_SIZE)) as i8 as i32 as u32;
    let b = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if b == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, b + 0x10));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, b + HITS_AT));
    let f = cpu.read32(b + FLAGS) | 0x2000_0400;
    cpu.write32(b + FLAGS, f);
    cpu.write32(b + BEHAVIOUR, LPB_BOLT_BEHAVIOUR);
    let n = cpu.read8(t + S_NUMBER);
    cpu.write8(b + NUMBER, n);
    for k in [0, 4, 8] {
        let v = cpu.read32(t + S_POSITION + k);
        cpu.write32(b + POSITION + k, v);
    }
    cpu.write32(b + 0x24, 0x8000);
    let r = nested!(cpu, 0x0603_4dd4, sgl::sl_random(cpu));
    cpu.write16(b + ANGLE, r as u16);
    cpu.write8(b + BYTE_33, 8);
    cpu.write8(b + SPRITE_FRAME, 0xc6);
    cpu.write8(b + SPRITE_KIND, 0);
    let colour = cpu.read16(CHAR_BASE).wrapping_add(32);
    cpu.write16(b + OWN_COLOUR, colour);
    cpu.write16(b + COLOUR, colour);
    cpu.write32(b + HITS_AT, LPB_BOLT_BOXES);
    cpu.write8(b + 0x54, 1);
    cpu.write8(b + 0x55, 5);
    cpu.write32(b + 0x48, u32::MAX);
    cpu.write32(b + 0x4c, u32::MAX);
    let a = cpu.read16(t + S_ANGLE) as i16 as i32 as u32;
    let speed = cpu.read32(t + AS_SPEED);
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    let dx = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, speed, s));
    cpu.write32(b + LPB_BOLT_SPEED, dx);
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    let dy = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, speed, c));
    cpu.write32(b + LPB_BOLT_SPEED + 4, dy.wrapping_neg());
}

/// A bolt (0x0609a300): off the screen it ends; out of the pose's hull it ends in smoke (the
/// colour it has); else its colour is the base's (16 more on odd frames), it moves by its
/// speed (+0x6c, +0x70) and turns by 1/16 of a turn.
pub fn last_bolt_behaviour(cpu: &mut Cpu, o: u32) {
    if nested!(cpu, 0x0605_6a24, aim::off_screen(cpu, o + 0x10)) as u8 != 0 {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    if nested!(cpu, 0x0609_9fc4, last_pose_outside(cpu, o + POSITION)) != 0 {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        let colour = cpu.read16(o + COLOUR) as u32;
        nested!(cpu, 0x0609_a09c, last_piece_spark(cpu, o, colour));
        return;
    }
    let mut colour = cpu.read16(CHAR_BASE);
    if cpu.read32(GAME_FRAMES) & 1 != 0 {
        colour = colour.wrapping_add(16);
    }
    cpu.write16(o + OWN_COLOUR, colour);
    cpu.write16(o + COLOUR, colour);
    let x = cpu.read32(o + POSITION).wrapping_add(cpu.read32(o + LPB_BOLT_SPEED));
    cpu.write32(o + POSITION, x);
    let y = cpu.read32(o + POSITION + 4).wrapping_add(cpu.read32(o + LPB_BOLT_SPEED + 4));
    cpu.write32(o + POSITION + 4, y);
    let a = cpu.read16(o + ANGLE).wrapping_add(0x1000);
    cpu.write16(o + ANGLE, a);
}

/// A piece made for the boss `p` (0x0609a218): a plain actor of 84 bytes (flags +0x20000000,
/// behaviour 0x06099e94, its number the boss's, its place the pose's point `along` (the high
/// word) on the blended pose, no z, side 0, frame 0, kind 59, its colours that kind's) with its boss,
/// its place along the path, its speed `speed`, a count of 0, its tag `tag` and its frame number 0.
const LPM_SIZE: u32 = 84;
const LPM_FLAGS: u32 = 0x2000_0000;
const LPM_KIND: u32 = 59;
const LPM_SWING: u32 = 0x0609_9e94;

pub fn last_piece_make(cpu: &mut Cpu, p: u32, along: u32, speed: u32, tag: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, LPM_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, PLAIN_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + 0x10));
    let f = cpu.read32(o + FLAGS) | LPM_FLAGS;
    cpu.write32(o + FLAGS, f);
    cpu.write32(o + BEHAVIOUR, LPM_SWING);
    let n = cpu.read8(p + NUMBER);
    cpu.write8(o + NUMBER, n);
    let i = (along >> 16) as i16 as i32 as u32;
    let at = LPB_POSE_B.wrapping_add(i << 3);
    let x = cpu.read32(at);
    cpu.write32(o + POSITION, x);
    let y = cpu.read32(at + 4);
    cpu.write32(o + POSITION + 4, y);
    cpu.write32(o + POSITION + 8, 0);
    cpu.write8(o + BYTE_33, 0);
    cpu.write8(o + SPRITE_FRAME, 0);
    cpu.write8(o + SPRITE_KIND, LPM_KIND as u8);
    let colour = cpu.read16(kind(LPM_KIND) + K_COLOUR);
    cpu.write16(o + OWN_COLOUR, colour);
    cpu.write16(o + COLOUR, colour);
    cpu.write32(o + LQ_BOSS, p);
    cpu.write32(o + LQ_ALONG, along);
    cpu.write32(o + LQ_COUNT, 0);
    cpu.write32(o + LQ_SPEED, speed);
    cpu.write8(o + LQ_TAG, tag as u8);
    cpu.write8(o + LQ_FRAME_AT, 0);
}

/// The last boss's modes that bring it on (its step +0x6d in each, its mode +0x6c, its count
/// +0x80 up through each step's move to 1; its place's x, y, z at +0xa0, its turns (words) at
/// +0x74 (x), +0x76 (y) and +0x78 (z), 0x0609879c after each).
const LS_STEP: u32 = 0x6d;
const LS_COUNT: u32 = 0x80;
const LS_PLACE: u32 = 0xa0;
const LS_TURNS: u32 = 0x74;
const LS_ARMS: u32 = 0x70;
const LS_DONE: u32 = 0x1_0000;

/// A place (three longs) moved from `a` to `b` by the count `t`, into the boss's place (+0xa0).
fn last_place_ease(cpu: &mut Cpu, o: u32, a: [u32; 3], b: [u32; 3], t: u32) {
    for k in 0..3 {
        let v = nested!(cpu, 0x0606_2ebc, math::ease(cpu, a[k], b[k], t));
        cpu.write32(o + LS_PLACE + 4 * k as u32, v);
    }
}

fn last_table(cpu: &mut Cpu, at: u32) -> [u32; 3] {
    [cpu.read32(at), cpu.read32(at + 4), cpu.read32(at + 8)]
}

/// The mode that gathers its parts (0x0609a4d8). Step 0: it is drawn into its place (the flags'
/// 0x80000, the place +0x14 to its +0xa0), its z turn kept (+0x2e from +0x76), the count 0. 1: the
/// count up by 1/128, its place eased from its own to the point 0x060a76c0, its x turn eased from
/// 0x12200 to 0 and, in the first quarter of the count, its y turn from the kept one to 0 by four
/// times the count; sound 100 half way; at the count's end (1) the count, the y and z turns are
/// 0. 2: the count up by 1/256, its place eased on to the point 0x060a76cc; at the end its arms (0x06099030, 0 to 3,
/// kept at +0x70) and followers (0x06099230, 4 to 15, with the turns of
/// 0x060a6588 and 0x060a6598) are made and, by the pick (0x060a8988), pieces
/// (0x0609a218) circle it: 2 to 4 one at 0x20000 and 1 to 4 one at 0x80000 (speed 1/128), and two at 0xb0000
/// and 0x50000 going the other way. 3: the second pose (0x06098934) until its count is full, then
/// its mode is 3.
const LG_FLAGS: u32 = 0x8_0000;
const LG_KEPT: u32 = 0x2e;
const LG_Z_TURN: u32 = 0x78;
const LG_RISE_STEP: u32 = 0x100;
const LG_FROM: u32 = 0x060a_76c0;
const LG_TO: u32 = 0x060a_76cc;
const LG_X_TURN_FROM: u32 = 0x1_2200;
const LG_QUARTER: i32 = 0x3fff;
const LG_SOUND: u32 = 100;
const LG_SOUND_AT: u32 = 0x4000;
const LG_FOLLOWER_HITS: [u32; 2] = [0x060a_6588, 0x060a_6598];
const LG_FOLLOWERS: [(u32, usize, u32); 12] = [
    (4, 0, 0x2000),
    (5, 0, 0xffff_e000),
    (6, 0, 0xffff_a000),
    (7, 0, 0x6000),
    (8, 1, 0),
    (9, 1, 0),
    (10, 1, 0x4000),
    (11, 1, 0x4000),
    (12, 1, 0),
    (13, 1, 0),
    (14, 1, 0x4000),
    (15, 1, 0x4000),
];
const LG_PIECE_SPEED: u32 = 0x200;
const LG_PIECE_BACK: u32 = 0xffff_fe00;

pub fn last_boss_gather(cpu: &mut Cpu, o: u32) {
    let step = cpu.read8(o + LS_STEP);
    let next = |cpu: &mut Cpu| {
        let s = cpu.read8(o + LS_STEP).wrapping_add(1);
        cpu.write8(o + LS_STEP, s);
    };
    match step {
        0 => {
            let f = cpu.read32(o + FLAGS) | LG_FLAGS;
            cpu.write32(o + FLAGS, f);
            for k in [0, 4, 8] {
                let v = cpu.read32(o + LS_PLACE + k);
                cpu.write32(o + POSITION + k, v);
            }
            let v = cpu.read16(o + LS_TURNS + 2);
            cpu.write16(o + LG_KEPT, v);
            cpu.write32(o + LS_COUNT, 0);
            next(cpu);
        }
        1 => {
            let t = cpu.read32(o + LS_COUNT).wrapping_add(LG_RISE_STEP);
            cpu.write32(o + LS_COUNT, t);
            let from = [cpu.read32(o + POSITION), cpu.read32(o + POSITION + 4), cpu.read32(o + POSITION + 8)];
            let to = last_table(cpu, LG_FROM);
            last_place_ease(cpu, o, from, to, t);
            let v = nested!(cpu, 0x0606_2ebc, math::ease(cpu, LG_X_TURN_FROM, 0, t));
            cpu.write16(o + LS_TURNS, v as u16);
            let t = cpu.read32(o + LS_COUNT);
            if t as i32 <= LG_QUARTER {
                let kept = cpu.read16(o + LG_KEPT) as i16 as i32 as u32;
                let v = nested!(cpu, 0x0606_2ebc, math::ease(cpu, kept, 0, t << 2));
                cpu.write16(o + LS_TURNS + 2, v as u16);
            }
            if cpu.read32(o + LS_COUNT) & 0x7fff == LG_SOUND_AT {
                nested!(cpu, 0x0600_50d4, sound::se_set(cpu, LG_SOUND));
            }
            if cpu.read32(o + LS_COUNT) == LS_DONE {
                cpu.write32(o + LS_COUNT, 0);
                cpu.write16(o + LS_TURNS + 2, 0);
                cpu.write16(o + LG_Z_TURN, 0);
                next(cpu);
            }
        }
        2 => {
            let t = cpu.read32(o + LS_COUNT).wrapping_add(LG_RISE_STEP);
            cpu.write32(o + LS_COUNT, t);
            let (from, to) = (last_table(cpu, LG_FROM), last_table(cpu, LG_TO));
            last_place_ease(cpu, o, from, to, t);
            if cpu.read32(o + LS_COUNT) == LS_DONE {
                for k in 0..3 {
                    cpu.write32(o + LS_PLACE + 4 * k, to[k as usize]);
                }
                cpu.write16(o + LS_TURNS, 0);
                cpu.write32(o + LS_COUNT, 0);
                cpu.write16(o + LS_TURNS + 6, 0);
                for i in 0..4 {
                    let a = nested!(cpu, 0x0609_9030, last_arm_make(cpu, o, i));
                    cpu.write8(o + LS_ARMS + i, a as u8);
                }
                for (i, table, turn) in LG_FOLLOWERS {
                    nested!(cpu, 0x0609_9230, last_follower_make(cpu, o, i, LG_FOLLOWER_HITS[table], turn));
                }
                let pick = cpu.read32(LP_PICK_AT) as i32;
                if (2..=4).contains(&pick) {
                    nested!(cpu, 0x0609_a218, last_piece_make(cpu, o, 0x2_0000, LG_PIECE_SPEED, 0));
                }
                if (1..=4).contains(&pick) {
                    nested!(cpu, 0x0609_a218, last_piece_make(cpu, o, 0x8_0000, LG_PIECE_SPEED, 0));
                }
                nested!(cpu, 0x0609_a218, last_piece_make(cpu, o, 0xb_0000, LG_PIECE_BACK, 1));
                nested!(cpu, 0x0609_a218, last_piece_make(cpu, o, 0x5_0000, LG_PIECE_BACK, 2));
                next(cpu);
            }
        }
        3 => {
            nested!(cpu, 0x0609_8934, last_pose_first(cpu, o));
            if cpu.read32(o + LS_COUNT) == LS_DONE {
                cpu.write8(o + LS_STEP, 0);
                cpu.write8(o + LB_STEP, 3);
            }
        }
        _ => {}
    }
    nested!(cpu, 0x0609_879c, last_boss_matrix(cpu, o));
}

/// The mode before it (0x0609a834: no matrix after). Step 0: sound 96, the count 0x4000. 1: the
/// count up by 1/64, its place eased from 0x060a76d8 to 0x060a76ec; at the end the count is 0.
/// 2: the count up by 1/128, its place eased from there to 0x060a76f8 and its turns (words, x, y, z
/// at +0x74) from the words at 0x060a76e4 to those at 0x060a7704. 3: the boss's gauge set (most
/// 0x1adb0), its step 0, its mode 1.
const LA_FROM: u32 = 0x060a_76d8;
const LA_MID: u32 = 0x060a_76ec;
const LA_TO: u32 = 0x060a_76f8;
const LA_TURNS_FROM: u32 = 0x060a_76e4;
const LA_TURNS_TO: u32 = 0x060a_7704;
const LA_START_SOUND: u32 = 96;
const LA_START_COUNT: u32 = 0x4000;
const LA_STEP_ONE: u32 = 0x200;
const LA_STEP_TWO: u32 = 0x100;
const LA_GAUGE: u32 = 0x1_adb0;

pub fn last_boss_arrival(cpu: &mut Cpu, o: u32) {
    let step = cpu.read8(o + LS_STEP);
    let next = |cpu: &mut Cpu| {
        let s = cpu.read8(o + LS_STEP).wrapping_add(1);
        cpu.write8(o + LS_STEP, s);
    };
    match step {
        0 => {
            nested!(cpu, 0x0600_50d4, sound::se_set(cpu, LA_START_SOUND));
            cpu.write32(o + LS_COUNT, LA_START_COUNT);
            next(cpu);
        }
        1 => {
            let t = cpu.read32(o + LS_COUNT).wrapping_add(LA_STEP_ONE);
            cpu.write32(o + LS_COUNT, t);
            let (from, to) = (last_table(cpu, LA_FROM), last_table(cpu, LA_MID));
            last_place_ease(cpu, o, from, to, t);
            if cpu.read32(o + LS_COUNT) == LS_DONE {
                cpu.write32(o + LS_COUNT, 0);
                next(cpu);
            }
        }
        2 => {
            let t = cpu.read32(o + LS_COUNT).wrapping_add(LA_STEP_TWO);
            cpu.write32(o + LS_COUNT, t);
            let (from, to) = (last_table(cpu, LA_MID), last_table(cpu, LA_TO));
            last_place_ease(cpu, o, from, to, t);
            for k in 0..3 {
                let a = cpu.read16(LA_TURNS_FROM + 2 * k) as i16 as i32 as u32;
                let b = cpu.read16(LA_TURNS_TO + 2 * k) as i16 as i32 as u32;
                let t = cpu.read32(o + LS_COUNT);
                let v = nested!(cpu, 0x0606_2ebc, math::ease(cpu, a, b, t));
                cpu.write16(o + LS_TURNS + 2 * k, v as u16);
            }
            if cpu.read32(o + LS_COUNT) == LS_DONE {
                cpu.write32(o + LS_COUNT, 0);
                next(cpu);
            }
        }
        3 => {
            nested!(cpu, 0x0604_64d8, hud::boss_gauge_set(cpu, LA_GAUGE));
            cpu.write8(o + LS_STEP, 0);
            cpu.write8(o + LB_STEP, 1);
        }
        _ => {}
    }
}

/// The last boss beaten (0x0609af58; the behaviour 0x0609a9d4 after it): bullets cleared, the
/// scroll on, 15 pieces of wreck (0x0609adfc, 1 to 15) made, the flags' 0x80000, drawn by
/// 0x0609b1f0, its speed (+0xac) and count (+0x80) 0 and its turns (+0x2c, +0x2e) 0.
const LW_SPEED: u32 = 0xac;
const LW_PIECES: u32 = 15;
const LW_BEHAVIOUR: u32 = 0x0609_a9d4;
const LW_DRAW: u32 = 0x0609_b1f0;

pub fn last_boss_beaten(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0605_61e0, screen::bullets_cleared(cpu));
    nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
    for i in 1..=LW_PIECES {
        nested!(cpu, 0x0609_adfc, last_wreck_make(cpu, o, i));
    }
    let f = cpu.read32(o + FLAGS) | LG_FLAGS;
    cpu.write32(o + FLAGS, f);
    cpu.write32(o + BEHAVIOUR, LW_BEHAVIOUR);
    cpu.write32(o + CORNERS_AT, LW_DRAW);
    cpu.write32(o + LW_SPEED, 0);
    cpu.write32(o + LS_COUNT, 0);
    cpu.write16(o + 0x2c, 0);
    cpu.write16(o + 0x2e, 0);
}

/// The wrecked boss's behaviour (0x0609a9d4): its count (+0x80) up; past 0x1c1 it ends once the
/// stage's file (+0x51 of the one at 0x06074740) is 28. Every 16 frames to 0x12b sparks at its
/// place (0x0609ad20, by the count's low 5 bits), at 0x12c the explosion (0x06058ba4) there, at
/// 0x14a its draw is 0x06033654. It falls: its speed (+0xac) up by 0x28f, its z (+0xa8) by that,
/// its turns (+0x2c, +0x2e) by 0x80.
const LWB_END: i32 = 0x1c1;
const LWB_SPARKS: i32 = 0x12b;
const LWB_BLAST: u32 = 0x12c;
const LWB_DRAW: u32 = 0x14a;
const LWB_DRAW_TO: u32 = 0x0603_3654;
const LWB_FILE: u32 = 0x0607_4740;
const LWB_FILE_AT: u32 = 0x51;
const LWB_FILE_DONE: i8 = 28;
const LWB_FALL: u32 = 0x28f;
const LWB_TURN: u16 = 0x80;

pub fn last_wreck_behaviour(cpu: &mut Cpu, o: u32) {
    let n = cpu.read32(o + LS_COUNT).wrapping_add(1);
    cpu.write32(o + LS_COUNT, n);
    if n as i32 > LWB_END {
        let file = cpu.read32(LWB_FILE);
        if cpu.read8(file + LWB_FILE_AT) as i8 == LWB_FILE_DONE {
            cpu.write8(o + OBJ_MODE, TO_STOP);
            return;
        }
    }
    let n = cpu.read32(o + LS_COUNT);
    if n as i32 > LWB_SPARKS {
        if n == LWB_BLAST {
            let t = cpu.stack();
            for k in [0, 4, 8] {
                let v = cpu.read32(o + POSITION + k);
                cpu.write32(t + k, v);
            }
            cpu.write32(t + 60, o);
            nested!(cpu, 0x0605_8ba4, explosions::explosion_set(cpu, t));
        } else if n == LWB_DRAW {
            cpu.write32(o + CORNERS_AT, LWB_DRAW_TO);
        }
    } else if n & 15 == 0 {
        nested!(cpu, 0x0609_ad20, last_spark_make(cpu, o + POSITION, n & 31));
    }
    let v = cpu.read32(o + LW_SPEED).wrapping_add(LWB_FALL);
    cpu.write32(o + LW_SPEED, v);
    let z = cpu.read32(o + LS_PLACE + 8).wrapping_add(v);
    cpu.write32(o + LS_PLACE + 8, z);
    for at in [0x2c, 0x2e] {
        let a = cpu.read16(o + at).wrapping_add(LWB_TURN);
        cpu.write16(o + at, a);
    }
}

/// A piece of the wreck made for the boss `p` (0x0609adfc; number `i`): a plain actor of 104
/// bytes (behaviour 0x0609aaa0, flags +0xc00000, drawn by 0x0609ab90) at the boss's place
/// (+0xa0, as +0x4c) with its turns (words at +0x44, from the boss's +0x74), turning
/// at random (+0x58, +0x5c, +0x60: a random less 0x8000; words +0x40 and +0x42, the turns' speeds:
/// -256 to 255), its life +0x64 by its number (1: 240, 2: 180, 3: 120, else a random byte).
const LWM_SIZE: u32 = 104;
const LWM_FLAGS: u32 = 0xc0_0000;
const LWM_BEHAVIOUR: u32 = 0x0609_aaa0;
const LWM_DRAW: u32 = 0x0609_ab90;
const LWM_AT: u32 = 0x4c;
const LWM_TURNS: u32 = 0x44;
const LWM_SPINS: u32 = 0x58;
const LWM_TURN_SPEEDS: u32 = 0x40;
const LWM_LIFE: u32 = 0x64;
const LWM_INDEX: u32 = 0x3c;
const LWM_SPIN_BASE: u32 = 0xffff_8000;

pub fn last_wreck_make(cpu: &mut Cpu, p: u32, i: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, LWM_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, PLAIN_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + 0x10));
    cpu.write32(o + BEHAVIOUR, LWM_BEHAVIOUR);
    let f = cpu.read32(o + FLAGS) | LWM_FLAGS;
    cpu.write32(o + FLAGS, f);
    cpu.write32(o + CORNERS_AT, LWM_DRAW);
    cpu.write8(o + BYTE_33, 0);
    for k in 0..3 {
        let v = cpu.read32(p + LS_PLACE + 4 * k);
        cpu.write32(o + LWM_AT + 4 * k, v);
    }
    for k in 0..3 {
        let v = cpu.read16(p + LS_TURNS + 2 * k);
        cpu.write16(o + LWM_TURNS + 2 * k, v);
    }
    cpu.write16(o + 0x2e, 0);
    cpu.write16(o + 0x2c, 0);
    cpu.write32(o + LWM_INDEX, i);
    for k in 0..3 {
        let r = nested!(cpu, 0x0603_4dd4, sgl::sl_random(cpu));
        cpu.write32(o + LWM_SPINS + 4 * k, r.wrapping_add(LWM_SPIN_BASE));
    }
    for k in 0..2 {
        let r = nested!(cpu, 0x0603_4dd4, sgl::sl_random(cpu));
        cpu.write16(o + LWM_TURN_SPEEDS + 2 * k, ((r & 0x1ff) as u16).wrapping_add(0xff00));
    }
    let life = match i as i32 {
        2 => 0xb4,
        1 => 0xf0,
        3 => 120,
        _ => nested!(cpu, 0x0603_4dd4, sgl::sl_random(cpu)) & 0xff,
    };
    cpu.write32(o + LWM_LIFE, life);
}

/// A piece of the wreck (0x0609aaa0): every 16 frames to 128... of its life count (+0x64) sparks
/// at its place (0x0609ad20, by the count's low 5 bits); it falls: its speed z (+0x60) up by 0x51e,
/// its place (+0x4c) by its speeds (+0x58, +0x5c, +0x60), its turns (+0x2c, +0x2e) by the words
/// at +0x40 and +0x42. At a count of 0x12c it ends: by its number up to 3 in a burst (0x0609acd8,
/// at its place, by its +0x30 turn), else in a glow (0x06058f34, size 1) there.
const LWP_FALL: u32 = 0x51e;
const LWP_END: u32 = 0x12c;
const LWP_GLOW_SCALE: u32 = 0x1_0000;
const LWP_GLOW_SCALE_AT: u32 = 44;

pub fn last_wreck_piece_behaviour(cpu: &mut Cpu, o: u32) {
    let n = cpu.read32(o + LWM_LIFE);
    if n & 15 == 0 {
        nested!(cpu, 0x0609_ad20, last_spark_make(cpu, o + POSITION, n & 31));
    }
    let v = cpu.read32(o + LWM_SPINS + 8).wrapping_add(LWP_FALL);
    cpu.write32(o + LWM_SPINS + 8, v);
    for k in 0..2 {
        let a = cpu.read32(o + LWM_AT + 4 * k).wrapping_add(cpu.read32(o + LWM_SPINS + 4 * k));
        cpu.write32(o + LWM_AT + 4 * k, a);
    }
    let z = cpu.read32(o + LWM_AT + 8).wrapping_add(v);
    cpu.write32(o + LWM_AT + 8, z);
    for k in 0..2 {
        let a = cpu.read16(o + 0x2c + 2 * k).wrapping_add(cpu.read16(o + LWM_TURN_SPEEDS + 2 * k));
        cpu.write16(o + 0x2c + 2 * k, a);
    }
    let n = cpu.read32(o + LWM_LIFE).wrapping_add(1);
    cpu.write32(o + LWM_LIFE, n);
    if n != LWP_END {
        return;
    }
    let t = cpu.stack();
    if cpu.read32(o + LWM_INDEX) as i32 <= 3 {
        for k in [0, 4, 8] {
            let v = cpu.read32(o + POSITION + k);
            cpu.write32(t + k, v);
        }
        let a = cpu.read16(o + ANGLE) as i16 as i32 as u32;
        nested!(cpu, 0x0609_acd8, last_burst(cpu, t, a));
    } else {
        for k in [0, 4, 8] {
            let v = cpu.read32(o + POSITION + k);
            cpu.write32(t + 12 + k, v);
        }
        cpu.write32(t + 12 + LWP_GLOW_SCALE_AT, LWP_GLOW_SCALE);
        nested!(cpu, 0x0605_8f34, explosions::glow_set(cpu, t + 12));
    }
    cpu.write8(o + OBJ_MODE, TO_STOP);
}

/// SGL's window and the last boss's drawing: the window's set (0x0606713c), its parts' models (kind
/// 58's, the pair for the mark byte then the model of the pair's for the flash) put each under the
/// matrices of its place and turns.
const SL_CUR_WINDOW: u32 = 0x0606_713c;
const SL_CONVERT: u32 = 0x0606_750c;
const SL_DISP_SPRITE: u32 = 0x0607_0e22;
const LD_KIND: u32 = 58;
const LD_POSE_POINT: u32 = 20;
const LD_SCREEN_POINT: u32 = 0x060a_6058;

/// The model of the last boss's file at `n` (the pair's, then its first).
fn last_model(cpu: &mut Cpu, n: u32, which: u32) -> u32 {
    let file = cpu.read32(kind(LD_KIND));
    let models = cpu.read32(file + F_MODELS);
    let pair = cpu.read32(models.wrapping_add(n << 2));
    cpu.read32(pair.wrapping_add(which << 2))
}

/// Turns by three words at `at` (x, y, z).
fn last_turn_xyz(cpu: &mut Cpu, at: u32) {
    for (k, f) in [SL_ROT_X, SL_ROT_Y, SL_ROT_Z].into_iter().enumerate() {
        let a = cpu.read16(at + 2 * k as u32) as i16 as i32 as u32;
        cpu.call(f, &[a]);
    }
}

/// A piece of wreck's draw (0x0609ab90; `d` its sprite, +0x10): its place (+0x4c) and turns
/// (+0x44), its index's point of the pose and its turns, its own turns (+0x2c, +0x2e), its
/// index's model (by the byte of 0x060a6064 at twice the index) put; its place worked out into
/// +0x14 and its depth (+0x1c) a sixteenth of its z.
pub fn last_wreck_draw(cpu: &mut Cpu, d: u32) {
    let o = d.wrapping_sub(0x10);
    let t = cpu.stack();
    cpu.call(SL_CUR_WINDOW, &[0]);
    cpu.call(SL_PUSH_MATRIX, &[]);
    cpu.write32(t, d + 68);
    let (x, y, z) = (cpu.read32(d + 60), cpu.read32(d + 64), cpu.read32(d + 68));
    cpu.call(SL_TRANSLATE, &[x, y, z]);
    last_turn_xyz(cpu, d + 52);
    let i = cpu.read32(o + LWM_INDEX);
    let pose = LPB_POSE.wrapping_add(i.wrapping_mul(LD_POSE_POINT));
    let (x, y, z) = (cpu.read32(pose), cpu.read32(pose + 4), cpu.read32(pose + 8));
    cpu.call(SL_TRANSLATE, &[x, y, z]);
    last_turn_xyz(cpu, pose + 12);
    for at in [0x2c, 0x2e] {
        let a = cpu.read16(o + at) as i16 as i32 as u32;
        cpu.call(if at == 0x2c { SL_ROT_X } else { SL_ROT_Y }, &[a]);
    }
    let mark = cpu.read8(LP_MARKS.wrapping_add(i << 1)) as i8 as i32 as u32;
    let model = last_model(cpu, mark, 0);
    cpu.call(SL_PUT_POLYGON, &[model]);
    cpu.call(SL_CONVERT, &[LD_SCREEN_POINT, d + 4]);
    let z = cpu.read32(d + 68) as i32 >> 4;
    cpu.write32(o + 0x1c, z as u32);
    cpu.call(SL_POP_MATRIX, &[]);
    cpu.call(SL_CUR_WINDOW, &[1]);
}

/// A burst of smoke puffs about `p` (puffs_around: the angle `a`, the points at 0x060a771c, 8 of
/// them) and a glow of size 2 there (0x06058f34).
const LBU_POINTS: u32 = 0x060a_771c;
const LBU_COUNT: u32 = 8;
const LBU_GLOW_SCALE: u32 = 0x2_0000;

pub fn last_burst(cpu: &mut Cpu, p: u32, a: u32) {
    let t = cpu.stack();
    nested!(cpu, 0x0606_2940, actors::puffs_around(cpu, p, a as i16 as i32 as u32, LBU_POINTS, LBU_COUNT));
    for k in [0, 4, 8] {
        let v = cpu.read32(p + k);
        cpu.write32(t + k, v);
    }
    cpu.write32(t + 44, LBU_GLOW_SCALE);
    nested!(cpu, 0x0605_8f34, explosions::glow_set(cpu, t));
}

/// A spark (smoke, drift_effect's) at the place `p`, a random bit from 0 to 4 dots off in x and
/// in y (a place 2 dots back, then 0 to 4 on), colour 0x90 (0x8090 when `n` is not 0), scales
/// 1 less its z (a byte of it), its flags a random of 0x30 more; sound 18 when `n` is not 0.
const LSP_RANGE: u32 = 0x40_0000;
const LSP_BACK: u32 = 0xffe0_0000;
const LSP_ANIMATION: u32 = 0x0607_3008;
const LSP_COLOUR: u16 = 0x90;
const LSP_COLOUR_N: u16 = 0x8090;
const LSP_SOUND: u32 = 18;

pub fn last_spark_make(cpu: &mut Cpu, p: u32, n: u32) {
    let t = cpu.stack();
    let r = nested!(cpu, 0x0603_4dd4, sgl::sl_random(cpu));
    let m = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, LSP_RANGE, r));
    let x = cpu.read32(p).wrapping_add(LSP_BACK).wrapping_add(m);
    cpu.write32(t, x);
    let r = nested!(cpu, 0x0603_4dd4, sgl::sl_random(cpu));
    let m = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, LSP_RANGE, r));
    let y = cpu.read32(p + 4).wrapping_add(LSP_BACK).wrapping_add(m);
    cpu.write32(t + 4, y);
    let z = cpu.read32(p + 8);
    cpu.write32(t + 8, z);
    cpu.write32(t + actors::E_SCRIPT, LSP_ANIMATION);
    cpu.write16(t + actors::E_COLOUR, if n != 0 { LSP_COLOUR_N } else { LSP_COLOUR });
    cpu.write16(t + actors::E_ANGLE, 0);
    cpu.write32(t + actors::E_SPEED, 0);
    let z = cpu.read32(p + 8);
    nested!(cpu, 0x0600_8164, clib::ashiftrt_r4::<8>(cpu, z));
    let s = 0x1_0000u32.wrapping_sub(((z as i32) >> 8) as u32);
    cpu.write32(t + actors::E_SCALES + 4, s);
    cpu.write32(t + actors::E_SCALES, s);
    let e = nested!(cpu, 0x0605_7454, actors::drift_effect(cpu, t));
    if e == 0 {
        return;
    }
    let r = nested!(cpu, 0x0603_4dd4, sgl::sl_random(cpu)) & 0x30;
    let f = cpu.read32(e + FLAGS) | r;
    cpu.write32(e + FLAGS, f);
    if n != 0 {
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, LSP_SOUND));
    }
}

/// The last boss's turns and place put (the beginning of its two draws; `d` its sprite, +0x10):
/// the window 0, a matrix pushed, its place (+0xa0) and its turns (words, +0x74).
fn last_boss_place_put(cpu: &mut Cpu, d: u32) {
    let o = d.wrapping_sub(0x10);
    cpu.call(SL_CUR_WINDOW, &[0]);
    cpu.call(SL_PUSH_MATRIX, &[]);
    let (x, y, z) = (cpu.read32(o + LS_PLACE), cpu.read32(o + LS_PLACE + 4), cpu.read32(o + LS_PLACE + 8));
    cpu.call(SL_TRANSLATE, &[x, y, z]);
    last_turn_xyz(cpu, d + 100);
}

/// The wrecked boss's draw (0x0609b1f0): its part 0's model (the mark byte at 0x060a6064 and its
/// first) put at its place and turns, its point 0 of the pose and that point's turns and its own
/// (+0x2c, +0x2e); its place worked out into +0x14.
pub fn last_wreck_boss_draw(cpu: &mut Cpu, d: u32) {
    last_boss_place_put(cpu, d);
    let o = d.wrapping_sub(0x10);
    let (x, y, z) = (cpu.read32(LPB_POSE), cpu.read32(LPB_POSE + 4), cpu.read32(LPB_POSE + 8));
    cpu.call(SL_TRANSLATE, &[x, y, z]);
    last_turn_xyz(cpu, LPB_POSE + 12);
    for (at, f) in [(0x2c, SL_ROT_X), (0x2e, SL_ROT_Y)] {
        let a = cpu.read16(o + at) as i16 as i32 as u32;
        cpu.call(f, &[a]);
    }
    let mark = cpu.read8(LP_MARKS) as i8 as i32 as u32;
    let model = last_model(cpu, mark, 0);
    cpu.call(SL_PUT_POLYGON, &[model]);
    cpu.call(SL_CONVERT, &[LD_SCREEN_POINT, d + 4]);
    cpu.call(SL_POP_MATRIX, &[]);
    cpu.call(SL_CUR_WINDOW, &[1]);
}

/// The last boss's draw (0x0609afd8; `d` its sprite, +0x10): its place and turns put, then each of
/// the pose's 16 points' parts under its own matrix: its model (the mark byte at 0x060a6064 and,
/// when the flash byte after it was set (cleared as read), the pair's second) put. Then
/// the flames: on even frames a character from 0x060741f8 plus the byte the pointer at 0x6a8 of the
/// data at 0x060741f4 gives (its colour the base's plus 48, a scale of 0x5000 and 0x280000 deep); on
/// odd frames the pointer at 8 (colour plus 112, 0x7000, 0x320000 deep; its second word 0x100); the scale
/// grows by a random 0 to 15 times 3 256ths; four sprites (the points 4 to 7) are put, each at
/// its point.
const LDR_FLAME_KIND_AT: u32 = 0x0607_41f4;
const LDR_FLAME_CHAR: u32 = 0x0607_41f8;
const LDR_EVEN: (u32, u32, u32, u32) = (0x6a8, 0x5000, 0x28_0000, 48);
const LDR_ODD: (u32, u32, u32, u32) = (8, 0x7000, 0x32_0000, 112);
const LDR_POINTS: u32 = 16;

pub fn last_boss_draw(cpu: &mut Cpu, d: u32) {
    let t = cpu.stack();
    last_boss_place_put(cpu, d);
    for i in 0..LDR_POINTS {
        cpu.call(SL_PUSH_MATRIX, &[]);
        let pose = LPB_POSE + LD_POSE_POINT * i;
        let (x, y, z) = (cpu.read32(pose), cpu.read32(pose + 4), cpu.read32(pose + 8));
        cpu.call(SL_TRANSLATE, &[x, y, z]);
        last_turn_xyz(cpu, pose + 12);
        let flag = LP_MARKS + 2 * i + 1;
        let flash = cpu.read8(flag) as i8 != 0;
        if flash {
            cpu.write8(flag, 0);
        }
        let mark = cpu.read8(LP_MARKS + 2 * i) as i8 as i32 as u32;
        let model = last_model(cpu, mark, flash as u32);
        cpu.call(SL_PUT_POLYGON, &[model]);
        cpu.call(SL_POP_MATRIX, &[]);
    }
    let even = cpu.read32(LOOPS) & 1 == 0;
    let (at, scale, deep, colour) = if even { LDR_EVEN } else { LDR_ODD };
    let c = cpu.read16(CHAR_BASE).wrapping_add(colour as u16);
    cpu.write16(t + 20, c);
    cpu.write16(t + 18, if even { 0 } else { 0x100 });
    let kinds = cpu.read32(LDR_FLAME_KIND_AT);
    let byte = cpu.read32(kinds + at);
    let ch = cpu.read16(LDR_FLAME_CHAR).wrapping_add(cpu.read8(byte) as u16);
    cpu.write16(t + 16, ch);
    let r = nested!(cpu, 0x0603_4dd4, sgl::sl_random(cpu)) & 15;
    cpu.write32(t + 12, scale.wrapping_add((r * 3) << 8));
    cpu.write16(t + 22, 0);
    cpu.write16(t + 24, 2);
    for k in 4..8 {
        let pose = LPB_POSE + LD_POSE_POINT * k;
        let (x, y, z) = (cpu.read32(pose), cpu.read32(pose + 4), cpu.read32(pose + 8));
        cpu.write32(t, x);
        cpu.write32(t + 4, y);
        cpu.write32(t + 8, z.wrapping_add(deep));
        cpu.call(SL_DISP_SPRITE, &[t, t + 16, 0]);
    }
    cpu.call(SL_POP_MATRIX, &[]);
    cpu.call(SL_CUR_WINDOW, &[1]);
}

/// The last boss made (0x0609b2f4): a numbered actor of 232 bytes (a number of the enemies'; none
/// free, 0), its behaviour 0x06097628, flags +0xc80000, drawn by 0x0609afd8, side 1, its place
/// 0, its life (+0x50 and +0x88) 0x1adb0 and the three lives (+0x8c, +0x90, +0x94) 0x11170, the
/// stage's score and rate (0x060746f0, 0x060746f4) 0, its mode and step 0, its place (+0xa0) and
/// turns (words, +0x74 +0x76 +0x78) from 0x060a76d8's, its pick (0x060a8988) the difficulty, the
/// poses copied in (the 16-point one at 0x060a6088 and the 13-point one at 0x060a75c8), the
/// lighting set (the table's ramp 28, floor 10, the light turned), its kind 58's models sorted
/// (frames 11 to 18 by 2, 0 to 6 by 1). Its number.
const LM_SIZE: u32 = 0xe8;
const LM_FLAGS: u32 = 0xc8_0000;
const LM_HITS: u32 = 0x060a_5fe6;
const LM_LIFE: u32 = 0x1_adb0;
const LM_LIVES: u32 = 0x1_1170;
const LM_START: u32 = 0x060a_76d8;
const LM_POSE_FROM: u32 = 0x060a_6088;
const LM_POSE_B_FROM: u32 = 0x060a_75c8;
const LM_SCORE_AT: u32 = 0x0607_46f4;
const LM_LIGHT: u32 = 0xffff_e000;
const LM_SORT: u32 = 0x0607_4750;
const DIFFICULTY_BYTE: u32 = 0x0600_ed84;
const LM_SORTS: [(u32, u32); 15] = [
    (11, 2),
    (12, 2),
    (13, 2),
    (14, 2),
    (15, 2),
    (16, 2),
    (17, 2),
    (18, 2),
    (0, 1),
    (1, 1),
    (2, 1),
    (3, 1),
    (4, 1),
    (5, 1),
    (6, 1),
];

pub fn last_boss_make(cpu: &mut Cpu) -> u32 {
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, 2)) as i8;
    if n == -1 {
        return 0;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, LM_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, NUMBERED_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + 0x10));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HITS_AT));
    cpu.write32(o + BEHAVIOUR, 0x0609_7628);
    for k in [0, 4, 8] {
        cpu.write32(o + POSITION + k, 0);
    }
    let f = cpu.read32(o + FLAGS) | LM_FLAGS;
    cpu.write32(o + FLAGS, f);
    cpu.write32(o + CORNERS_AT, 0x0609_afd8);
    cpu.write8(o + BYTE_33, 0);
    cpu.write8(o + NUMBER, n as u8);
    let number = cpu.read8(o + NUMBER) as i8 as i32 as u32;
    cpu.write32(NUMBERED.wrapping_add(number << 2), o);
    cpu.write8(o + SIDE, 1);
    cpu.write32(o + HITS_AT, LM_HITS);
    cpu.write32(o + LP_LIFE, LM_LIFE);
    cpu.write32(STAGE_RATE_AT, 0);
    cpu.write32(LM_SCORE_AT, 0);
    cpu.write32(o + LB_LIFE, LM_LIFE);
    for at in [0x8c, 0x90, 0x94] {
        cpu.write32(o + at, LM_LIVES);
    }
    cpu.write8(o + LS_STEP, 0);
    cpu.write32(o + LS_COUNT, 0);
    for k in [0, 4, 8] {
        let v = cpu.read32(LM_START + k);
        cpu.write32(o + LS_PLACE + k, v);
    }
    for (k, from) in [(0, 2), (2, 6), (4, 10)] {
        let v = cpu.read16(LM_START + from);
        cpu.write16(o + LS_TURNS + k, v);
    }
    cpu.write16(o + 0x7a, 0);
    cpu.write16(o + 0x7c, 0);
    cpu.write8(o + 0x6f, 0);
    cpu.write8(o + 0x6e, 0);
    cpu.write32(o + LQ_BOSS_MODE, 0);
    let difficulty = cpu.read8(DIFFICULTY_BYTE) as i8 as i32 as u32;
    cpu.write32(LP_PICK_AT, difficulty);
    for k in 0..16 {
        for j in 0..5 {
            let v = cpu.read32(LM_POSE_FROM + 20 * k + 4 * j);
            cpu.write32(LPB_POSE + 20 * k + 4 * j, v);
        }
    }
    for k in 0..13 {
        for j in 0..2 {
            let v = cpu.read32(LM_POSE_B_FROM + 8 * k + 4 * j);
            cpu.write32(LPB_POSE_B + 8 * k + 4 * j, v);
        }
    }
    nested!(cpu, 0x0606_3168, gouraud::table_ramp(cpu, 28, 28, 28));
    nested!(cpu, 0x0606_30e0, gouraud::table_floor(cpu, 10, 10, 10));
    nested!(cpu, 0x0606_2fe8, gouraud::light_turned(cpu, LM_LIGHT, LM_LIGHT, 0));
    for k in [16, 20, 24] {
        cpu.write32(LM_SORT + k, 0);
    }
    for (frame, sort) in LM_SORTS {
        nested!(cpu, 0x0606_3428, gouraud::model_sort(cpu, LD_KIND, frame, sort));
    }
    cpu.write8(o + LB_STEP, 0);
    cpu.read8(o + NUMBER) as i8 as i32 as u32
}

/// The director's record (states.rs's) and its flags (+0x3e); the 3D view (bg.rs's: its place on
/// its path in the first long, its x at +4, its y angle's low half at +0x16, its sine scale at
/// +0x14), the scenery's frame of reference (bg.rs's MOVER: its place up at +0x10), the rotation
/// scroll's second plane (+0x0c, +0x10 its place), the stage's screen record and the hook it
/// keeps (+0x58: called each frame to draw).
const DIRECTOR: u32 = 0x0607_46e0;
const DIRECTOR_FLAGS: u32 = 0x3e;
const SCENE_VIEW: u32 = 0x0607_4750;
const SCENE_VIEW_X: u32 = 4;
const SCENE_VIEW_TURN_X: u32 = 16;
const SCENE_VIEW_SCALE: u32 = 20;
const SCENE_VIEW_TURN_Y: u32 = 22;
const SCENE_MOVER: u32 = 0x0607_4810;
const SCENE_MOVER_TILT: u32 = 0;
const SCENE_MOVER_Y: u32 = 16;
const SCENE_PLANE: u32 = 0x0607_4770;
const SCENE_PLANE_X: u32 = 12;
const SCENE_PLANE_Y: u32 = 16;
const SCREEN_HOOK: u32 = 0x58;
/// The view's paths and the messages of the stages' scenery scenes (past the image: read at run
/// time), the models' kind (its file's list of pieces at +0x1c), the pieces' check size.
const TUNNEL_PATH: u32 = 0x060a_7788;
const TUNNEL_MESSAGE: u32 = 0x060a_7a8c;
const RAIL_PATH: u32 = 0x060a_7e70;
const RAIL_MESSAGE: u32 = 0x060a_7aec;
const PIECES_KIND: u32 = 0x31;
const PIECES_MODELS_AT: u32 = 0x1c;
const PIECES_CHECK: u32 = 0x28_0000;
const PIECES_REACH: u32 = 0x18_0000;
/// The scenery scenes' per-frame sets of SGL calls in this overlay (their own copies of SGL's
/// setters, not rewritten yet): the screen's blocks and windows by place.
const SL_WINDOW_RANGE: u32 = 0x060a_2804;
const SL_SPRITES_COPY: u32 = 0x060a_2824;
const SL_SCENE_MODE: u32 = 0x060a_26b4;
const SL_SCENE_SIZE: u32 = 0x060a_27c4;
const SL_SCENE_ON: u32 = 0x060a_26c4;
const SL_SCENE_MESSAGE: u32 = 0x060a_26fc;
const SL_SCENE_PLACE: u32 = 0x060a_2844;
const SL_SCENE_TURN: u32 = 0x060a_2864;

fn scene_scroll_next(cpu: &mut Cpu) {
    nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
}

/// Moves on to the next scroll if the director's flags have bit 0.
pub fn scroll_if_flagged(cpu: &mut Cpu) {
    let game = cpu.read32(DIRECTOR);
    if cpu.read8(game + DIRECTOR_FLAGS) & 1 != 0 {
        scene_scroll_next(cpu);
    }
}

/// The pillars' scene's start (`game`'s word +0x20 given 8.0): BG layers on, kind 49's first four
/// models set, the view at the path's start and the mover's frame cleared (its words and angles
/// zero, its scales 1.0), the draw hook set, the message made.
pub fn tunnel_start(cpu: &mut Cpu, game: u32) {
    nested!(cpu, 0x0606_e23c, sgl::sl_bg_on(cpu, 0x3_0014));
    nested!(cpu, 0x0606_2f74, gouraud::models_hss(cpu, PIECES_KIND, 4));
    cpu.write32(SCENE_VIEW, 0);
    nested!(cpu, 0x0605_f5b0, bg::view_along(cpu, TUNNEL_PATH));
    for at in [20, 16, 12, 32, 28, 24] {
        cpu.write32(SCENE_MOVER + at, 0);
    }
    for at in [4, 2, 0, 10, 8, 6] {
        cpu.write16(SCENE_MOVER + at, 0);
    }
    for at in [44, 40, 36] {
        cpu.write32(SCENE_MOVER + at, 0x1_0000);
    }
    nested!(cpu, 0x0606_e23c, sgl::sl_bg_on(cpu, 0x3_0014));
    let screen = cpu.read32(screen::SCREEN);
    cpu.write32(screen + SCREEN_HOOK, 0x0609_d68c);
    nested!(cpu, 0x0606_1620, text::message_set(cpu, TUNNEL_MESSAGE));
    cpu.write32(game + 0x20, 0x8_0000);
}

/// Two of the pieces (the first and second of `pair`'s) at (x, y, z): each checked on screen as
/// a box (size 2.5, 1.5 either side of the place in x and y), the matrix moved to the place if
/// either is, and each seen one drawn.
fn pieces_at(cpu: &mut Cpu, x: u32, y: u32, z: u32, pair: [u32; 2]) {
    let p = cpu.stack();
    let mut seen = [false; 2];
    for (k, reach) in [PIECES_REACH.wrapping_neg(), PIECES_REACH].into_iter().enumerate() {
        cpu.write32(p, x.wrapping_add(reach));
        cpu.write32(p + 4, y.wrapping_add(reach));
        cpu.write32(p + 8, z);
        let on = nested!(cpu, 0x0606_e9fc, sgl::sl_check_on_screen(cpu, p, PIECES_CHECK));
        seen[k] = on & 0x8000_0000 == 0;
    }
    if seen[0] || seen[1] {
        nested!(cpu, 0x0606_71cc, sgl::sl_push_matrix(cpu));
        nested!(cpu, 0x0606_721c, sgl::sl_translate(cpu, x, y, z));
        for k in [0, 1] {
            if seen[k] {
                let file = cpu.read32(kind(PIECES_KIND));
                let models = cpu.read32(file + PIECES_MODELS_AT);
                let piece = cpu.read32(models + 4 * pair[k]);
                let model = cpu.read32(piece);
                nested!(cpu, 0x0606_745c, sgl::sl_put_polygon(cpu, model));
            }
        }
        nested!(cpu, 0x0606_7200, sgl::sl_pop_matrix(cpu));
    }
}

/// Pieces 3 and 1 at (x, y, z).
pub fn pieces_high(cpu: &mut Cpu, x: u32, y: u32, z: u32) {
    pieces_at(cpu, x, y, z, [3, 1]);
}

/// Pieces 2 and 0 at (x, y, z).
pub fn pieces_low(cpu: &mut Cpu, x: u32, y: u32, z: u32) {
    pieces_at(cpu, x, y, z, [2, 0]);
}

/// The scenes' pieces: the high and low pairs of a row, laid out at x and y.
const PILLAR_ROWS: [(bool, u32, u32); 18] = [
    (false, 0, 0x0080_0000),
    (false, 0x0100_0000, 0x0080_0000),
    (true, 0xff40_0000, 0x0040_0000),
    (true, 0x0040_0000, 0x0040_0000),
    (true, 0x0140_0000, 0x0040_0000),
    (false, 0x01c0_0000, 0xffc0_0000),
    (false, 0x00c0_0000, 0xffc0_0000),
    (false, 0xffc0_0000, 0xffc0_0000),
    (false, 0, 0xff80_0000),
    (false, 0x0100_0000, 0xff80_0000),
    (true, 0xff40_0000, 0xff40_0000),
    (true, 0x0040_0000, 0xff40_0000),
    (true, 0x0140_0000, 0xff40_0000),
    (false, 0x01c0_0000, 0xfec0_0000),
    (false, 0x00c0_0000, 0xfec0_0000),
    (false, 0xffc0_0000, 0xfec0_0000),
    (false, 0, 0xfe80_0000),
    (false, 0x0100_0000, 0xfe80_0000),
];

/// The rail scene's rows: the same, further on.
const RAIL_ROWS: [(bool, u32, u32); 19] = [
    (false, 0x0100_0000, 0x0080_0000),
    (true, 0xff40_0000, 0x0040_0000),
    (true, 0x0040_0000, 0x0040_0000),
    (true, 0x0140_0000, 0x0040_0000),
    (false, 0x01c0_0000, 0xffc0_0000),
    (false, 0x00c0_0000, 0xffc0_0000),
    (false, 0x0100_0000, 0xff80_0000),
    (true, 0x0040_0000, 0xff40_0000),
    (true, 0x0140_0000, 0xff40_0000),
    (false, 0x01c0_0000, 0xfec0_0000),
    (false, 0x00c0_0000, 0xfec0_0000),
    (false, 0x0100_0000, 0xfe80_0000),
    (true, 0x0040_0000, 0xfe40_0000),
    (true, 0x0140_0000, 0xfe40_0000),
    (false, 0x01c0_0000, 0xfdc0_0000),
    (false, 0x00c0_0000, 0xfdc0_0000),
    (false, 0x0100_0000, 0xfd80_0000),
    (true, 0x0140_0000, 0xfd40_0000),
    (false, 0x01c0_0000, 0xfcc0_0000),
];

/// The rows drawn with the matrix about the view: the high pieces' function for a true row, the
/// low's for a false.
fn draw_rows(cpu: &mut Cpu, rows: &[(bool, u32, u32)]) {
    for &(high, x, y) in rows {
        if high {
            nested!(cpu, 0x0609_d4dc, pieces_high(cpu, x, y, 0));
        } else {
            nested!(cpu, 0x0609_d5b4, pieces_low(cpu, x, y, 0));
        }
    }
}

/// The pillars' draw (the screen's hook): window 0, the view's matrix, moved by the view's x
/// (its top byte) and the mover's y (below its top byte), the rows drawn, then the mover's
/// rotation; window 1.
pub fn tunnel_draw(cpu: &mut Cpu) {
    nested!(cpu, 0x0606_713c, sgl::sl_cur_window(cpu, 0));
    nested!(cpu, 0x0606_71cc, sgl::sl_push_matrix(cpu));
    nested!(cpu, 0x0605_f468, bg::view_matrix(cpu));
    nested!(cpu, 0x0606_71cc, sgl::sl_push_matrix(cpu));
    let x = cpu.read32(SCENE_VIEW + SCENE_VIEW_X) & 0xff00_0000;
    let y = cpu.read32(SCENE_MOVER + SCENE_MOVER_Y) & 0x00ff_ffff;
    nested!(cpu, 0x0606_721c, sgl::sl_translate(cpu, x, y, 0));
    draw_rows(cpu, &PILLAR_ROWS);
    nested!(cpu, 0x0606_7200, sgl::sl_pop_matrix(cpu));
    nested!(cpu, 0x0605_f530, bg::rotation_matrix(cpu, SCENE_MOVER));
    nested!(cpu, 0x0606_7200, sgl::sl_pop_matrix(cpu));
    nested!(cpu, 0x0606_713c, sgl::sl_cur_window(cpu, 1));
}

/// The pillars' move (`o`'s word +0x20 its speed along): the mover moved along by it, the view
/// on its path (eased), the draw.
pub fn tunnel_move(cpu: &mut Cpu, o: u32) {
    let speed = cpu.read32(o + 32);
    nested!(cpu, 0x0605_f3a4, bg::mover_move(cpu, 0, speed, 0));
    nested!(cpu, 0x0605_f648, bg::view_along_eased(cpu, TUNNEL_PATH));
    nested!(cpu, 0x0609_d68c, tunnel_draw(cpu));
}

/// The pillars' step: the view's place on by 0x100, moved; once past 3.0 the speed (above 0.25)
/// slowed by 0x800 a frame; the next scroll once past 4.0.
pub fn tunnel_step(cpu: &mut Cpu, o: u32) {
    let at = cpu.read32(SCENE_VIEW).wrapping_add(0x100);
    cpu.write32(SCENE_VIEW, at);
    nested!(cpu, 0x0609_d7f4, tunnel_move(cpu, o));
    if (cpu.read32(SCENE_VIEW) as i32) > 0x2_ffff && (cpu.read32(o + 32) as i32) > 0x4000 {
        let speed = cpu.read32(o + 32).wrapping_sub(0x800);
        cpu.write32(o + 32, speed);
    }
    if (cpu.read32(SCENE_VIEW) as i32) > 0x3_ffff {
        scene_scroll_next(cpu);
    }
}

/// The wave's lines: 224 pairs of longs (a scroll in x, a zoom in 16.16) for the line scroll
/// table, sent to VDP2's RAM each frame.
const WAVE_LINES: u32 = 0x060a_89e0;
const WAVE_COUNT: u32 = 224;
const WAVE_VRAM: u32 = 0x25e7_2000;
const WAVE_SIZE: u32 = 0x700;

/// The wave's lines cleared: no scroll, zoom 1.0.
pub fn wave_clear(cpu: &mut Cpu) {
    for k in 0..WAVE_COUNT {
        cpu.write32(WAVE_LINES + 8 * k, 0);
        cpu.write32(WAVE_LINES + 8 * k + 4, 0x1_0000);
    }
}

/// The wave's lines with no scroll and a zoom at random about 1.0 (0xff80 to 0x1007f).
pub fn wave_random(cpu: &mut Cpu) {
    for k in 0..WAVE_COUNT {
        cpu.write32(WAVE_LINES + 8 * k, 0);
        let r = nested!(cpu, 0x0603_4dd4, sgl::sl_random(cpu));
        cpu.write32(WAVE_LINES + 8 * k + 4, 0xff80 + (r & 0xff));
    }
}

/// The wave's lines for a sway of `amount`: each line's angle 0x124 on from the last, its sine
/// (by `amount`) scrolling the line by 11.0 times that less, its zoom about 1.0 by the same
/// and a random 0 to 255 less 128.
pub fn wave_set(cpu: &mut Cpu, amount: u32) {
    let mut angle: i32 = 0;
    for k in 0..WAVE_COUNT {
        let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, angle as u32));
        let sway = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, s, amount));
        let zoom = sway.wrapping_add(0x1_0000);
        let scroll = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, 0xb0_0000, 0x1_0000u32.wrapping_sub(zoom)));
        cpu.write32(WAVE_LINES + 8 * k, scroll);
        let r = nested!(cpu, 0x0603_4dd4, sgl::sl_random(cpu));
        cpu.write32(WAVE_LINES + 8 * k + 4, zoom.wrapping_sub(128).wrapping_add(r & 0xff));
        angle = (angle + 0x124) as i16 as i32;
    }
}

/// A ripple's object: its centre line (+0x10), its sway (+0x20), its half height in lines
/// (+0x18), its phase (a long at +0x14, the angle its low half), the phase's step per line (+0x1c,
/// the angle its low half), the sway's step down (+0x24), its frames (+0x28); its state in +3.
const RIPPLE_SIZE: u32 = 0x2c;
const RIPPLE_STATE: u32 = 3;
const RIPPLE_CENTRE: u32 = 0x10;
const RIPPLE_PHASE: u32 = 0x14;
const RIPPLE_PHASE_ANGLE: u32 = 0x16;
const RIPPLE_HALF: u32 = 0x18;
const RIPPLE_STEP: u32 = 0x1c;
const RIPPLE_STEP_ANGLE: u32 = 0x1e;
const RIPPLE_SWAY: u32 = 0x20;
const RIPPLE_SWAY_DOWN: u32 = 0x24;
const RIPPLE_FRAMES: u32 = 0x28;
/// Its states: started (1), over (0x70), then gone (0x73).
const RIPPLE_RUNNING: u8 = 1;
const RIPPLE_OVER: u8 = 0x70;
const RIPPLE_GONE: u8 = 0x73;
const RIPPLE_LIFE: u32 = 16;
const RIPPLE_EXEC: u32 = 0x0609_d9ac;

/// A ripple's step: not while the game is paused; the first frame sets its sway's step (a
/// sixteenth of the sway), its phase's step (0x8000 over its half height) and halves its
/// half height; each frame the lines in its range (on the table) are nudged by the sway times
/// the sine of an angle stepping on by the phase's step, half of them at random; the phase on by
/// 0x1000, the sway down by its step; after 16 frames it is over, then gone.
pub fn ripple_behaviour(cpu: &mut Cpu, o: u32) {
    if cpu.read32(GAME_FLAGS) & 0x8000_0000 != 0 {
        return;
    }
    let state = cpu.read8(o + RIPPLE_STATE) as i8;
    if state == 0 {
        cpu.write32(o + RIPPLE_PHASE, 0);
        cpu.write8(o + RIPPLE_STATE, RIPPLE_RUNNING);
        let sway = cpu.read32(o + RIPPLE_SWAY) as i32;
        let down = (if sway < 0 { sway.wrapping_add(15) } else { sway }) >> 4;
        cpu.write32(o + RIPPLE_SWAY_DOWN, down as u32);
        cpu.write32(o + RIPPLE_FRAMES, 0);
        let half = cpu.read32(o + RIPPLE_HALF);
        let step = nested!(cpu, 0x0606_62d8, clib::divide(cpu, 0x8000, half));
        cpu.write32(o + RIPPLE_STEP, step);
        let half = cpu.read32(o + RIPPLE_HALF) as i32;
        cpu.write32(o + RIPPLE_HALF, (half >> 1) as u32);
    } else if state != 1 {
        if state > 1 {
            if state == RIPPLE_OVER as i8 {
                cpu.write8(o + RIPPLE_STATE, RIPPLE_GONE);
            } else if state == RIPPLE_GONE as i8 {
                nested!(cpu, 0x0600_579c, objects::obj_free(cpu, o));
            }
        }
        return;
    }
    let phase = cpu.read16(o + RIPPLE_PHASE_ANGLE) as i16 as i32 as u32;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, phase));
    let sway = cpu.read32(o + RIPPLE_SWAY);
    let amount = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, sway, s));
    let first = cpu.read32(o + RIPPLE_CENTRE).wrapping_sub(cpu.read32(o + RIPPLE_HALF)) as i32;
    let mut angle: i32 = 0;
    let mut line = first;
    while line < (cpu.read32(o + RIPPLE_CENTRE).wrapping_add(cpu.read32(o + RIPPLE_HALF)) as i32) {
        if line >= 0 && line <= (WAVE_COUNT - 1) as i32 {
            let step = cpu.read16(o + RIPPLE_STEP_ANGLE) as i16 as i32;
            angle = (angle + step) as i16 as i32;
            let r = nested!(cpu, 0x0603_4dd4, sgl::sl_random(cpu));
            if r & 1 == 0 {
                let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, angle as u32));
                let nudge = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, amount, s));
                let at = WAVE_LINES + 8 * line as u32;
                let v = cpu.read32(at).wrapping_sub(nudge);
                cpu.write32(at, v);
            }
        }
        line += 1;
    }
    let phase = cpu.read32(o + RIPPLE_PHASE).wrapping_add(0x1000);
    cpu.write32(o + RIPPLE_PHASE, phase);
    let frames = cpu.read32(o + RIPPLE_FRAMES).wrapping_add(1);
    cpu.write32(o + RIPPLE_FRAMES, frames);
    let sway = cpu.read32(o + RIPPLE_SWAY).wrapping_sub(cpu.read32(o + RIPPLE_SWAY_DOWN));
    cpu.write32(o + RIPPLE_SWAY, sway);
    if frames == RIPPLE_LIFE {
        cpu.write8(o + RIPPLE_STATE, RIPPLE_OVER);
    }
}

/// A ripple made at line `centre`, `sway` wide, `half` lines high (0 if no room).
pub fn ripple_make(cpu: &mut Cpu, centre: u32, sway: u32, half: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, RIPPLE_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 0, blocks, RIPPLE_EXEC));
    if o != 0 {
        cpu.write32(o + RIPPLE_CENTRE, centre);
        cpu.write32(o + RIPPLE_SWAY, sway);
        cpu.write32(o + RIPPLE_HALF, half);
    }
}

/// A ripple now and then (1 frame in 64, at random): from the left (side 0) or right (side 1) or
/// one of two middle sways, at a line 48 to 175, 56 lines high.
pub fn ripples_spawn(cpu: &mut Cpu) {
    let r = nested!(cpu, 0x0603_4dd4, sgl::sl_random(cpu));
    if r & 0x3f != 0 {
        return;
    }
    let r = nested!(cpu, 0x0603_4dd4, sgl::sl_random(cpu)) & 7;
    let sway = match r {
        0 => 0x0160_0000,
        1 => 0xfea0_0000,
        r if r & 1 == 0 => 0xff50_0000,
        _ => 0x00b0_0000,
    };
    let r = nested!(cpu, 0x0603_4dd4, sgl::sl_random(cpu));
    let centre = (r & 127) + 48;
    nested!(cpu, 0x0609_dafc, ripple_make(cpu, centre, sway, 56));
}

/// The wave scene's object: its state (+8), a sway angle (a word at +0x10), the window's edge (a
/// word at +0x12), the sway's size (+0x24).
const SCENE_STATE: u32 = 8;
const SCENE_ANGLE: u32 = 0x10;
const SCENE_EDGE: u32 = 0x12;
const SCENE_SWAY: u32 = 0x24;

/// The wave scene's record (VDP2's rotation-style set-up: a block of words at 0x060a8990) and
/// the first scroll plane's (bg.rs's PLANE_A) place and zooms.
const WAVE_MAP: u32 = 0x060a_8990;
const WAVE_MAP_TABLE: u32 = 0x060a_7860;
const WAVE_MAP_VRAM: u32 = 0x25e7_6000;

/// The wave scene's start (`o` its object): its map's record set, the first plane put at its
/// place at 1.0, a layer on, and the line scroll set up (`wave_lines_set`).
pub fn wave_start(cpu: &mut Cpu, o: u32) {
    cpu.write8(WAVE_MAP, 4);
    cpu.write8(WAVE_MAP + 1, 0);
    cpu.write32(WAVE_MAP + 20, WAVE_MAP_TABLE);
    cpu.write32(WAVE_MAP + 16, WAVE_MAP_VRAM);
    cpu.write32(WAVE_MAP + 8, 0x00e0_0000);
    cpu.write32(WAVE_MAP + 12, 0x0200_0000);
    cpu.write16(WAVE_MAP + 4, 0);
    cpu.write16(WAVE_MAP + 6, 0x1fff);
    cpu.write16(WAVE_MAP + 2, 0x0090);
    cpu.write32(WAVE_MAP + 28, 0);
    cpu.write32(WAVE_MAP + 24, 0);
    cpu.write32(bg::PLANE_A + bg::X, 0x00b0_0000);
    cpu.write32(bg::PLANE_A + bg::Y, 0x0190_0000);
    cpu.write32(bg::PLANE_A + bg::SPEED_Y, 0);
    cpu.write32(bg::PLANE_A + bg::SPEED_X, 0);
    cpu.write32(bg::PLANE_A + bg::ZOOM_X, 0x1_0000);
    cpu.write32(bg::PLANE_A + bg::ZOOM_Y, 0x1_0000);
    nested!(cpu, 0x0606_e23c, sgl::sl_bg_on(cpu, 0x2_0015));
    nested!(cpu, 0x0609_dc60, wave_lines_set(cpu, o));
}

/// The wave scene's line scroll set up: its object's state (+8) 0, window blocks put (the whole
/// width, no lines), NBG1's window control 3 and line scroll control 10, the sprites copied, the
/// lines cleared.
pub fn wave_lines_set(cpu: &mut Cpu, o: u32) {
    cpu.write8(o + 8, 0);
    cpu.call(SL_WINDOW_RANGE, &[0, 0, 0x15f, 0xffff]);
    nested!(cpu, 0x0606_e17c, sgl::sl_window_control(cpu, 1, 3));
    nested!(cpu, 0x0606_e1bc, sgl::sl_line_scroll_control(cpu, 1, 10));
    cpu.call(SL_SPRITES_COPY, &[WAVE_VRAM]);
    nested!(cpu, 0x0609_d894, wave_clear(cpu));
}

/// The wave's colours: 16 words from the game's table to the stack, then 240 of its colours from
/// the table after to the colour RAM's 0x100 and the 16 to 0x1f0.
pub fn wave_colours(cpu: &mut Cpu) {
    let s = cpu.stack();
    for k in 0..16 {
        let v = cpu.read16(0x0603_1040 + 2 * k);
        cpu.write16(s + 2 * k, v);
    }
    nested!(cpu, 0x0600_5b18, palette::set_colours(cpu, 0x0603_1060, 0x100, 0xf0));
    nested!(cpu, 0x0600_5b18, palette::set_colours(cpu, s, 0x1f0, 0x10));
}

/// The wave scene's frame for `o` (states in +8: 0 begins, 1 the lines swaying in and the window
/// closing in from the top, 2 the lines random, 3 the window opening out, 4 over), the plane's
/// speed set to (dx, dy), the map streamed in if `stream`, the plane drawn and the lines sent.
pub fn wave_scene(cpu: &mut Cpu, o: u32, dx: u32, dy: u32, stream: u32) {
    nested!(cpu, 0x0609_dcbc, wave_colours(cpu));
    let state = cpu.read8(o + SCENE_STATE);
    let mut dy = dy;
    let mut tail = true;
    match state {
        0 => {
            cpu.write32(o + SCENE_SWAY, 0x2000);
            cpu.write16(o + SCENE_ANGLE, 0);
            cpu.write16(o + SCENE_EDGE, 0);
            cpu.write8(o + SCENE_STATE, 1);
            dy = 0;
        }
        1 => {
            let v = cpu.read16(o + SCENE_EDGE) as i16 as i32 + 8;
            let mut top = (112 - v) as i16 as i32;
            cpu.write16(o + SCENE_EDGE, v as u16);
            if top < 0 {
                top = 0;
            }
            let bottom = ((v + 112) as i16 as i32).min(0xdf);
            cpu.call(SL_WINDOW_RANGE, &[0, top as u16 as u32, 0x15f, bottom as u16 as u32]);
            let angle = cpu.read16(o + SCENE_ANGLE) as i16 as i32 as u32;
            let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, angle));
            let sway = cpu.read32(o + SCENE_SWAY);
            let amount = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, sway, s));
            nested!(cpu, 0x0609_d918, wave_set(cpu, amount));
            let angle = cpu.read16(o + SCENE_ANGLE).wrapping_add(0x200);
            cpu.write16(o + SCENE_ANGLE, angle);
            let sway = cpu.read32(o + SCENE_SWAY).wrapping_sub(32);
            cpu.write32(o + SCENE_SWAY, sway);
            if (sway as i32) < 0 {
                cpu.write16(o + SCENE_EDGE, 0);
                cpu.write32(o + SCENE_SWAY, 0);
                let s = cpu.read8(o + SCENE_STATE).wrapping_add(1);
                cpu.write8(o + SCENE_STATE, s);
            }
            nested!(cpu, 0x0609_db48, ripples_spawn(cpu));
        }
        2 => {
            nested!(cpu, 0x0609_d8c4, wave_random(cpu));
            nested!(cpu, 0x0609_db48, ripples_spawn(cpu));
        }
        3 => {
            let v = (cpu.read16(o + SCENE_EDGE) as i16 as i32 + 4) as i16;
            cpu.write16(o + SCENE_EDGE, v as u16);
            if v as i32 > 112 {
                cpu.write16(o + SCENE_EDGE, 112);
                nested!(cpu, 0x0606_e17c, sgl::sl_window_control(cpu, 1, 0));
                nested!(cpu, 0x0606_e1bc, sgl::sl_line_scroll_control(cpu, 1, 0));
                nested!(cpu, 0x0606_e23c, sgl::sl_bg_on(cpu, 0x3_0014));
                let s = cpu.read8(o + SCENE_STATE).wrapping_add(1);
                cpu.write8(o + SCENE_STATE, s);
            }
            let top = cpu.read16(o + SCENE_EDGE) as i16 as i32;
            cpu.call(SL_WINDOW_RANGE, &[0, top as u16 as u32, 0x15f, (0xe0 - top) as u16 as u32]);
            nested!(cpu, 0x0609_db48, ripples_spawn(cpu));
        }
        4 => tail = false,
        _ => {}
    }
    if tail {
        nested!(cpu, 0x0605_efd4, bg::plane_a_speed(cpu, dx, dy));
        if stream != 0 {
            nested!(cpu, 0x0605_f170, bg::map_stream(cpu, bg::PLANE_A, WAVE_MAP));
        }
        nested!(cpu, 0x0605_f040, bg::plane_a_frame(cpu));
        nested!(cpu, 0x0603_4a50, sgl::sl_transfer_entry(cpu, WAVE_LINES, WAVE_VRAM, WAVE_SIZE));
    }
}

/// The wave scene's step: the view's place on by 0x100 and moved (the pillars' move); the plane
/// slid by four times the view's x gone back; the wave frame; the next scroll once past 8.0.
pub fn wave_step(cpu: &mut Cpu, o: u32) {
    let view = SCENE_VIEW;
    let place = cpu.read32(view).wrapping_add(0x100);
    let x = cpu.read32(view + SCENE_VIEW_X);
    cpu.write32(view, place);
    nested!(cpu, 0x0609_d7f4, tunnel_move(cpu, o));
    let back = x.wrapping_sub(cpu.read32(view + SCENE_VIEW_X)) << 2;
    nested!(cpu, 0x0609_dd08, wave_scene(cpu, o, back.wrapping_neg(), 0xffff_0000, 1));
    if (cpu.read32(view) as i32) > 0x7_ffff {
        scene_scroll_next(cpu);
    }
}

/// The rail scene's draw: the pillars' draw with the mover's y alone (and its own rows).
pub fn rail_draw(cpu: &mut Cpu) {
    nested!(cpu, 0x0606_713c, sgl::sl_cur_window(cpu, 0));
    nested!(cpu, 0x0606_71cc, sgl::sl_push_matrix(cpu));
    nested!(cpu, 0x0605_f468, bg::view_matrix(cpu));
    nested!(cpu, 0x0606_71cc, sgl::sl_push_matrix(cpu));
    let y = cpu.read32(SCENE_MOVER + SCENE_MOVER_Y);
    nested!(cpu, 0x0606_721c, sgl::sl_translate(cpu, 0, y, 0));
    draw_rows(cpu, &RAIL_ROWS);
    nested!(cpu, 0x0606_7200, sgl::sl_pop_matrix(cpu));
    nested!(cpu, 0x0605_f530, bg::rotation_matrix(cpu, SCENE_MOVER));
    nested!(cpu, 0x0606_7200, sgl::sl_pop_matrix(cpu));
    nested!(cpu, 0x0606_713c, sgl::sl_cur_window(cpu, 1));
}

/// The rail scene's start (`o` its object): the view at its path's start, the mover's y kept
/// below 2.0 (its top bits), the object's word +0x30 -2.0, its bytes +9 0 and +8 on, the draw hook set.
pub fn rail_start(cpu: &mut Cpu, o: u32) {
    cpu.write32(SCENE_VIEW, 0);
    nested!(cpu, 0x0605_f5b0, bg::view_along(cpu, RAIL_PATH));
    let y = cpu.read32(SCENE_MOVER + SCENE_MOVER_Y) & 0x01ff_0000;
    cpu.write32(SCENE_MOVER + SCENE_MOVER_Y, y);
    cpu.write32(o + 0x30, 0xfe00_0000);
    cpu.write8(o + 9, 0);
    let s = cpu.read8(o + 8).wrapping_add(1);
    cpu.write8(o + 8, s);
    let screen = cpu.read32(screen::SCREEN);
    cpu.write32(screen + SCREEN_HOOK, 0x0609_df34);
}

/// The rail scene's step: its message sent the first frame (a byte at +9), the mover moved by
/// the object's speed (+0x20, up by 0x1000 a frame), the draw, the wave frame (plane speed
/// (0, -1.0)); the next scroll once the mover's y is past 4.5.
pub fn rail_step(cpu: &mut Cpu, o: u32) {
    if cpu.read8(o + 9) == 0 {
        cpu.call(SL_SCENE_MESSAGE, &[RAIL_MESSAGE]);
        let b = cpu.read8(o + 9).wrapping_add(1);
        cpu.write8(o + 9, b);
    }
    let speed = cpu.read32(o + 32).wrapping_add(0x1000);
    cpu.write32(o + 32, speed);
    nested!(cpu, 0x0605_f3a4, bg::mover_move(cpu, 0, speed, 0));
    nested!(cpu, 0x0609_df34, rail_draw(cpu));
    nested!(cpu, 0x0609_dd08, wave_scene(cpu, o, 0, 0xffff_0000, 1));
    if (cpu.read32(SCENE_MOVER + SCENE_MOVER_Y) as i32) > 0x047f_ffff {
        scene_scroll_next(cpu);
    }
}

/// The dome scene's start: RBG0's map put at 0x25e44000, its mode 2, the draw hook set, its size
/// 2, the colour calculation 0x40 and the colour rates of the screens -2 to -8 and -7 set.
pub fn dome_start(cpu: &mut Cpu) {
    nested!(cpu, 0x0607_1c4c, sgl::sl_map_rbg0(cpu, 0x25e4_4000));
    cpu.call(SL_SCENE_MODE, &[2]);
    let screen = cpu.read32(screen::SCREEN);
    cpu.write32(screen + SCREEN_HOOK, 0x0609_e21c);
    cpu.call(SL_SCENE_SIZE, &[2]);
    nested!(cpu, 0x0607_0ddc, sgl::sl_color_calc(cpu, 0x40));
    for (screen, rate) in [(-2i32, 31), (-1, 28), (-4, 24), (-3, 20), (-6, 16), (-5, 12), (-8, 8), (-7, 4)] {
        nested!(cpu, 0x0606_e45c, sgl::sl_col_rate(cpu, screen as u32, rate));
    }
}

/// The dome scene's draw: window 0, turned about z by the view's y angle backwards, moved down
/// by the second plane's y from 2.0, the dome's polygons (list 0x060a7e5c) put; window 1.
pub fn dome_draw(cpu: &mut Cpu) {
    nested!(cpu, 0x0606_713c, sgl::sl_cur_window(cpu, 0));
    nested!(cpu, 0x0606_71cc, sgl::sl_push_matrix(cpu));
    let turn = cpu.read16(SCENE_VIEW + SCENE_VIEW_TURN_Y) as i16 as i32;
    nested!(cpu, 0x0606_726c, sgl::sl_rot_z(cpu, (-turn) as i16 as i32 as u32));
    let y = 0x0200_0000u32.wrapping_sub(cpu.read32(SCENE_PLANE + SCENE_PLANE_Y));
    nested!(cpu, 0x0606_721c, sgl::sl_translate(cpu, 0, y, 0));
    nested!(cpu, 0x0607_14dc, sgl::put_polygons(cpu, 0x060a_7e5c, 1));
    nested!(cpu, 0x0606_7200, sgl::sl_pop_matrix(cpu));
    nested!(cpu, 0x0606_713c, sgl::sl_cur_window(cpu, 1));
}

/// The dome scene's object: its speed (+0x20) and place (+0x30).
const DOME_SPEED: u32 = 0x20;
const DOME_AT: u32 = 0x30;

/// The dome scene's step (`o`: its speed +0x20, its place +0x30): its speed up by 0x1000; the
/// view's place on (by 0x180 below 1.0, by 0x1000 below 2.0); the dome's place on by the view's
/// scale times -0.09375; the view on its path; the mover moved; the view's matrix; the second
/// plane's place from the dome's and the view's turn; the draw; the next scroll past 2.0.
pub fn dome_step(cpu: &mut Cpu, o: u32) {
    let speed = cpu.read32(o + DOME_SPEED).wrapping_add(0x1000);
    cpu.write32(o + DOME_SPEED, speed);
    let place = cpu.read32(SCENE_VIEW) as i32;
    if place <= 0xffff {
        cpu.write32(SCENE_VIEW, (place + 0x180) as u32);
    } else if place <= 0x1_ffff {
        cpu.write32(SCENE_VIEW, (place + 0x1000) as u32);
    }
    let scale = cpu.read32(SCENE_VIEW + SCENE_VIEW_SCALE);
    let m = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, 0xffe8_0000, scale));
    let at = cpu.read32(o + DOME_AT).wrapping_add(m);
    cpu.write32(o + DOME_AT, at);
    nested!(cpu, 0x0605_f648, bg::view_along_eased(cpu, RAIL_PATH));
    cpu.call(SL_SCENE_ON, &[0]);
    let speed = cpu.read32(o + DOME_SPEED);
    let scale = cpu.read32(SCENE_VIEW + SCENE_VIEW_SCALE);
    let m = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, 0x20_0000, scale));
    nested!(cpu, 0x0605_f3a4, bg::mover_move(cpu, m, speed, 0));
    nested!(cpu, 0x0606_71cc, sgl::sl_push_matrix(cpu));
    nested!(cpu, 0x0605_f468, bg::view_matrix(cpu));
    nested!(cpu, 0x0605_f530, bg::rotation_matrix(cpu, SCENE_MOVER));
    nested!(cpu, 0x0606_7200, sgl::sl_pop_matrix(cpu));
    cpu.call(SL_SCENE_ON, &[1]);
    let height = cpu.read32(SCENE_VIEW + SCENE_VIEW_TURN_X).wrapping_add(0xffff_4000).wrapping_mul(11);
    let x = cpu.read32(o + DOME_AT).wrapping_add(0x02b0_0000);
    cpu.write32(SCENE_PLANE + SCENE_PLANE_X, x);
    let m = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, 0x70_0000, height));
    let y = m.wrapping_add(0x06d4_0000);
    cpu.write32(SCENE_PLANE + SCENE_PLANE_Y, y);
    cpu.call(SL_SCENE_PLACE, &[x, y]);
    let turn = cpu.read16(SCENE_VIEW + SCENE_VIEW_TURN_Y) as i16 as i32 as u32;
    cpu.call(SL_SCENE_TURN, &[turn]);
    nested!(cpu, 0x0609_e21c, dome_draw(cpu));
    if (cpu.read32(SCENE_VIEW) as i32) > 0x1_ffff {
        scene_scroll_next(cpu);
    }
}

/// The grid scene's object: its fade (+0x38), its drop speed (+0x24), its angle (a word at +0x14),
/// its place (+0x30), its speed (+0x20), its state (+8).
const GRID_FADE: u32 = 0x38;
const GRID_DROP: u32 = 0x24;
const GRID_ANGLE: u32 = 0x14;
const GRID_AT: u32 = 0x30;
const GRID_SPEED: u32 = 0x20;

/// The grid scene's start (`o` its object): its fade 0x1f8000 and drop 0, the second plane
/// (bg.rs's PLANE_B) put at (1.0, 2.0) with its zooms 1.0, a frame, the colour calculation 0x40,
/// the rate of screen 0 31, layers on, its state and angle cleared.
pub fn grid_start(cpu: &mut Cpu, o: u32) {
    cpu.write32(o + GRID_FADE, 0x001f_8000);
    cpu.write32(o + GRID_DROP, 0);
    cpu.write32(bg::PLANE_B + bg::X, 0x0100_0000);
    cpu.write32(bg::PLANE_B + bg::Y, 0x0200_0000);
    cpu.write32(bg::PLANE_B + bg::SPEED_Y, 0);
    cpu.write32(bg::PLANE_B + bg::SPEED_X, 0);
    cpu.write32(bg::PLANE_B + bg::ZOOM_X, 0x1_0000);
    cpu.write32(bg::PLANE_B + bg::ZOOM_Y, 0x1_0000);
    nested!(cpu, 0x0605_f08c, bg::plane_b_frame(cpu));
    nested!(cpu, 0x0607_0ddc, sgl::sl_color_calc(cpu, 0x40));
    nested!(cpu, 0x0606_e45c, sgl::sl_col_rate(cpu, 0, 31));
    nested!(cpu, 0x0606_e23c, sgl::sl_bg_on(cpu, 0x3_0014));
    cpu.write8(o + 8, 0);
    cpu.write16(o + GRID_ANGLE, 0);
}

/// The grid scene's step: the view's place on by 0x200 (to 3.0, on its path) before 3.0; the
/// plane's frame; the mover moved by 2.0 times the view's scale along; the angle (a word, up by 4
/// to 48) taken off the mover's tilt; the second plane's place and drop; state 0 draws the dome
/// until the plane's y passes 5.0, then clears the draw hook; state 1 fades the colour rate out
/// and moves to the next scroll.
pub fn grid_step(cpu: &mut Cpu, o: u32) {
    let place = cpu.read32(SCENE_VIEW);
    if place as i32 <= 0x2_ffff {
        let place = place.wrapping_add(0x200);
        cpu.write32(SCENE_VIEW, place);
        nested!(cpu, 0x0605_f648, bg::view_along_eased(cpu, RAIL_PATH));
    }
    cpu.call(SL_SCENE_ON, &[0]);
    let speed = cpu.read32(o + GRID_SPEED);
    let scale = cpu.read32(SCENE_VIEW + SCENE_VIEW_SCALE);
    let m = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, 0x20_0000, scale));
    nested!(cpu, 0x0605_f3a4, bg::mover_move(cpu, m, speed, 0));
    nested!(cpu, 0x0606_71cc, sgl::sl_push_matrix(cpu));
    nested!(cpu, 0x0605_f468, bg::view_matrix(cpu));
    let angle = cpu.read16(o + GRID_ANGLE) as i16 as i32;
    if angle <= 47 {
        cpu.write16(o + GRID_ANGLE, (angle + 4) as u16);
    }
    let tilt = cpu.read16(SCENE_MOVER + SCENE_MOVER_TILT).wrapping_sub(cpu.read16(o + GRID_ANGLE));
    cpu.write16(SCENE_MOVER + SCENE_MOVER_TILT, tilt);
    nested!(cpu, 0x0605_f530, bg::rotation_matrix(cpu, SCENE_MOVER));
    nested!(cpu, 0x0606_7200, sgl::sl_pop_matrix(cpu));
    cpu.call(SL_SCENE_ON, &[1]);
    let scale = cpu.read32(SCENE_VIEW + SCENE_VIEW_SCALE);
    let m = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, 0xffe8_0000, scale));
    let at = cpu.read32(o + GRID_AT).wrapping_add(m);
    cpu.write32(o + GRID_AT, at);
    cpu.write32(SCENE_PLANE + SCENE_PLANE_X, at.wrapping_add(0x02b0_0000));
    let drop = cpu.read32(o + GRID_DROP);
    let y = cpu.read32(SCENE_PLANE + SCENE_PLANE_Y).wrapping_sub(drop);
    cpu.write32(SCENE_PLANE + SCENE_PLANE_Y, y);
    nested!(cpu, 0x0605_eff8, bg::plane_b_speed(cpu, 0, drop.wrapping_neg()));
    let drop = cpu.read32(o + GRID_DROP).wrapping_add(0x1000);
    cpu.write32(o + GRID_DROP, drop);
    let x = cpu.read32(SCENE_PLANE + SCENE_PLANE_X);
    let y = cpu.read32(SCENE_PLANE + SCENE_PLANE_Y);
    cpu.call(SL_SCENE_PLACE, &[x, y]);
    let turn = cpu.read16(SCENE_VIEW + SCENE_VIEW_TURN_Y) as i16 as i32 as u32;
    cpu.call(SL_SCENE_TURN, &[turn]);
    nested!(cpu, 0x0605_f08c, bg::plane_b_frame(cpu));
    let state = cpu.read8(o + SCENE_STATE) as i8;
    if state == 0 {
        nested!(cpu, 0x0609_e21c, dome_draw(cpu));
        if (cpu.read32(SCENE_PLANE + SCENE_PLANE_Y) as i32) <= 0x013f_ffff {
            nested!(cpu, 0x0606_e23c, sgl::sl_bg_on(cpu, 0x1_0016));
            nested!(cpu, 0x0607_0ddc, sgl::sl_color_calc(cpu, 2));
            let screen = cpu.read32(screen::SCREEN);
            cpu.write32(screen + SCREEN_HOOK, 0);
            let s = cpu.read8(o + SCENE_STATE).wrapping_add(1);
            cpu.write8(o + SCENE_STATE, s);
        }
    } else if state == 1 {
        let fade = cpu.read32(o + GRID_FADE).wrapping_sub(0x8000);
        cpu.write32(o + GRID_FADE, fade);
        if (fade as i32) >= 0 {
            nested!(cpu, 0x0606_e45c, sgl::sl_col_rate(cpu, 0, fade >> 16));
        } else {
            scene_scroll_next(cpu);
        }
    }
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
/// which stay pushed until a Pop), a plane filled with cell 0 (where, its width and height),
/// cells copied (the stage data's cells, where, how many bytes), colours set (the stage data's
/// colours, from which colour, how many), or the pushed arguments let go.
#[derive(Clone, Copy)]
enum ScreenStep {
    Put(u32, ScreenAt, u32, u32, u32, u32),
    Fill(ScreenAt, u32, u32),
    Cells(u32, ScreenAt, u32),
    Palette(u32, u32, u32),
    Pop,
}
use ScreenStep::{Cells, Fill, Palette, Pop, Put};

const STAGE_SCREENS: [ScreenStep; 245] = [
    Cells(0, At(0x25e00000), 0x172a0),
    Put(0, At(0x25e40000), 32, 32, 0x0, 0x0),
    Put(0, At(0x25e40040), 32, 32, 0x0, 0x0),
    Put(0, At(0x25e41000), 32, 32, 0x0, 0x0),
    Put(0, At(0x25e41040), 32, 32, 0x0, 0x0),
    Pop,
    Put(2, At(0x25e42000), 32, 32, 0x0, 0x0),
    Put(1, At(0x25e42040), 32, 32, 0x0, 0x0),
    Put(1, At(0x25e43000), 32, 32, 0x0, 0x0),
    Put(0, At(0x25e43040), 32, 32, 0x0, 0x0),
    Pop,
    Put(4, At(0x25e44000), 32, 32, 0x0, 0x0),
    Put(3, At(0x25e44040), 32, 32, 0x0, 0x0),
    Put(3, At(0x25e45000), 32, 32, 0x0, 0x0),
    Put(4, At(0x25e45040), 32, 32, 0x0, 0x0),
    Pop,
    Put(7, At(0x25e46000), 32, 32, 0x0, 0x0),
    Put(8, At(0x25e46040), 32, 32, 0x0, 0x0),
    Put(5, At(0x25e47000), 32, 32, 0x0, 0x0),
    Put(6, At(0x25e47040), 32, 32, 0x0, 0x0),
    Pop,
    Put(13, At(0x25e48000), 32, 32, 0x0, 0x0),
    Put(14, At(0x25e48040), 32, 32, 0x0, 0x0),
    Put(15, At(0x25e49000), 32, 32, 0x0, 0x0),
    Put(16, At(0x25e49040), 32, 32, 0x0, 0x0),
    Pop,
    Put(16, At(0x25e4a000), 32, 32, 0x0, 0x0),
    Put(14, At(0x25e4a040), 32, 32, 0x0, 0x0),
    Put(14, At(0x25e4b000), 32, 32, 0x0, 0x0),
    Put(16, At(0x25e4b040), 32, 32, 0x0, 0x0),
    Pop,
    Put(8, At(0x25e4c000), 32, 32, 0x0, 0x0),
    Put(7, At(0x25e4c040), 32, 32, 0x0, 0x0),
    Put(8, At(0x25e4d000), 32, 32, 0x0, 0x0),
    Put(7, At(0x25e4d040), 32, 32, 0x0, 0x0),
    Pop,
    Put(19, At(0x25e4e000), 32, 32, 0x0, 0x0),
    Put(19, At(0x25e4e040), 32, 32, 0x0, 0x0),
    Put(20, At(0x25e4f000), 32, 32, 0x0, 0x0),
    Put(20, At(0x25e4f040), 32, 32, 0x0, 0x0),
    Pop,
    Put(21, At(0x25e50000), 32, 32, 0x0, 0x0),
    Put(19, At(0x25e50040), 32, 32, 0x0, 0x0),
    Put(22, At(0x25e51000), 32, 32, 0x0, 0x0),
    Put(20, At(0x25e51040), 32, 32, 0x0, 0x0),
    Pop,
    Put(23, At(0x25e52000), 32, 32, 0x6, 0x5c1),
    Put(23, At(0x25e52040), 32, 32, 0x6, 0x5c1),
    Put(24, At(0x25e53000), 32, 32, 0x6, 0x5c1),
    Put(24, At(0x25e53040), 32, 32, 0x6, 0x5c1),
    Pop,
    Put(71, At(0x25e54000), 32, 32, 0x6, 0x5c1),
    Put(71, At(0x25e54040), 32, 32, 0x6, 0x5c1),
    Put(70, At(0x25e55000), 32, 32, 0x6, 0x5c1),
    Put(70, At(0x25e55040), 32, 32, 0x6, 0x5c1),
    Pop,
    Put(76, At(0x25e56000), 32, 32, 0x6, 0x5c1),
    Put(77, At(0x25e56040), 32, 32, 0x6, 0x5c1),
    Put(72, At(0x25e57000), 32, 32, 0x6, 0x5c1),
    Put(73, At(0x25e57040), 32, 32, 0x6, 0x5c1),
    Pop,
    Put(76, At(0x25e58000), 32, 32, 0x6, 0x5c1),
    Put(77, At(0x25e58040), 32, 32, 0x6, 0x5c1),
    Put(74, At(0x25e59000), 32, 32, 0x6, 0x5c1),
    Put(75, At(0x25e59040), 32, 32, 0x6, 0x5c1),
    Pop,
    Put(78, At(0x25e5a000), 32, 32, 0x6, 0x5c1),
    Put(63, At(0x25e5a040), 32, 32, 0x6, 0x5c1),
    Put(74, At(0x25e5b000), 32, 32, 0x6, 0x5c1),
    Put(75, At(0x25e5b040), 32, 32, 0x6, 0x5c1),
    Pop,
    Put(74, At(0x25e5c000), 32, 32, 0x6, 0x5c1),
    Put(75, At(0x25e5c040), 32, 32, 0x6, 0x5c1),
    Put(72, At(0x25e5d000), 32, 32, 0x6, 0x5c1),
    Put(73, At(0x25e5d040), 32, 32, 0x6, 0x5c1),
    Pop,
    Put(74, At(0x25e5e000), 32, 32, 0x6, 0x5c1),
    Put(75, At(0x25e5e040), 32, 32, 0x6, 0x5c1),
    Put(64, At(0x25e5f000), 32, 32, 0x6, 0x5c1),
    Put(65, At(0x25e5f040), 32, 32, 0x6, 0x5c1),
    Pop,
    Put(68, Room(0x28000), 32, 32, 0x6, 0x5c1),
    Put(68, Room(0x28040), 32, 32, 0x6, 0x5c1),
    Put(66, Room(0x29000), 32, 32, 0x6, 0x5c1),
    Put(67, Room(0x29040), 32, 32, 0x6, 0x5c1),
    Pop,
    Put(68, Room(0x2a000), 32, 32, 0x6, 0x5c1),
    Put(68, Room(0x2a040), 32, 32, 0x6, 0x5c1),
    Put(69, Room(0x2b000), 32, 32, 0x6, 0x5c1),
    Put(69, Room(0x2b040), 32, 32, 0x6, 0x5c1),
    Pop,
    Put(62, Room(0x3c000), 32, 32, 0x6, 0x5c1),
    Put(62, Room(0x3c040), 32, 32, 0x6, 0x5c1),
    Put(90, Room(0x36000), 32, 32, 0x0, 0x0),
    Put(89, Room(0x36040), 32, 32, 0x0, 0x0),
    Pop,
    Put(88, Room(0x37000), 32, 32, 0x0, 0x0),
    Put(89, Room(0x37040), 32, 32, 0x0, 0x0),
    Put(92, Room(0x34000), 32, 32, 0x0, 0x0),
    Put(92, Room(0x34040), 32, 32, 0x0, 0x0),
    Pop,
    Put(91, Room(0x35000), 32, 32, 0x0, 0x0),
    Put(91, Room(0x35040), 32, 32, 0x0, 0x0),
    Palette(0, 0x0, 0x1e0),
    Cells(1, At(0x25e60000), 0x2a40),
    Put(10, At(0x25e76000), 32, 32, 0x0, 0x0),
    Put(10, At(0x25e76040), 32, 32, 0x0, 0x0),
    Pop,
    Put(9, At(0x25e77000), 32, 32, 0x0, 0x0),
    Put(9, At(0x25e77040), 32, 32, 0x0, 0x0),
    Put(10, Room(0), 32, 32, 0x0, 0x0),
    Put(10, Room(0x40), 32, 32, 0x0, 0x0),
    Pop,
    Put(11, Room(0x1000), 32, 32, 0x0, 0x0),
    Put(11, Room(0x1040), 32, 32, 0x0, 0x0),
    Put(12, Room(0x2000), 32, 32, 0x0, 0x0),
    Put(12, Room(0x2040), 32, 32, 0x0, 0x0),
    Pop,
    Put(11, Room(0x3000), 32, 32, 0x0, 0x0),
    Put(11, Room(0x3040), 32, 32, 0x0, 0x0),
    Put(28, Room(0x4000), 32, 32, 0x0, 0x0),
    Put(29, Room(0x4040), 32, 32, 0x0, 0x0),
    Pop,
    Put(54, Room(0x5000), 32, 32, 0x0, 0x0),
    Put(55, Room(0x5040), 32, 32, 0x0, 0x0),
    Put(26, Room(0x6000), 32, 32, 0x0, 0x0),
    Put(27, Room(0x6040), 32, 32, 0x0, 0x0),
    Pop,
    Put(54, Room(0x7000), 32, 32, 0x0, 0x0),
    Put(55, Room(0x7040), 32, 32, 0x0, 0x0),
    Put(60, Room(0x8000), 32, 32, 0x0, 0x0),
    Put(61, Room(0x8040), 32, 32, 0x0, 0x0),
    Pop,
    Put(43, Room(0x9000), 32, 32, 0x0, 0x0),
    Put(46, Room(0x9040), 32, 32, 0x0, 0x0),
    Put(42, Room(0xa000), 32, 32, 0x0, 0x0),
    Put(45, Room(0xa040), 32, 32, 0x0, 0x0),
    Pop,
    Put(42, Room(0xb000), 32, 32, 0x0, 0x0),
    Put(45, Room(0xb040), 32, 32, 0x0, 0x0),
    Put(41, Room(0xc000), 32, 32, 0x0, 0x0),
    Put(44, Room(0xc040), 32, 32, 0x0, 0x0),
    Pop,
    Put(52, Room(0xd000), 32, 32, 0x0, 0x0),
    Put(53, Room(0xd040), 32, 32, 0x0, 0x0),
    Put(50, Room(0xe000), 32, 32, 0x0, 0x0),
    Put(51, Room(0xe040), 32, 32, 0x0, 0x0),
    Pop,
    Put(50, Room(0xf000), 32, 32, 0x0, 0x0),
    Put(51, Room(0xf040), 32, 32, 0x0, 0x0),
    Put(48, Room(0x10000), 32, 32, 0x0, 0x0),
    Put(49, Room(0x10040), 32, 32, 0x0, 0x0),
    Pop,
    Put(43, Room(0x11000), 32, 32, 0x0, 0x0),
    Put(46, Room(0x11040), 32, 32, 0x0, 0x0),
    Put(41, Room(0x12000), 32, 32, 0x0, 0x0),
    Put(44, Room(0x12040), 32, 32, 0x0, 0x0),
    Pop,
    Put(39, Room(0x13000), 32, 32, 0x0, 0x0),
    Put(40, Room(0x13040), 32, 32, 0x0, 0x0),
    Put(57, Room(0x14000), 32, 32, 0x0, 0x0),
    Fill(Room(0x14040), 32, 32),  // cell 0
    Put(39, Room(0x15000), 32, 32, 0x0, 0x0),
    Pop,
    Put(40, Room(0x15040), 32, 32, 0x0, 0x0),
    Put(38, Room(0x16000), 32, 32, 0x0, 0x0),
    Fill(Room(0x16040), 32, 32),  // cell 0
    Put(35, Room(0x17000), 32, 32, 0x0, 0x0),
    Fill(Room(0x17040), 32, 32),  // cell 0
    Put(37, Room(0x18000), 32, 32, 0x0, 0x0),
    Pop,
    Fill(Room(0x18040), 32, 32),  // cell 0
    Put(35, Room(0x19000), 32, 32, 0x0, 0x0),
    Fill(Room(0x19040), 32, 32),  // cell 0
    Put(34, Room(0x1a000), 32, 32, 0x0, 0x0),
    Fill(Room(0x1a040), 32, 32),  // cell 0
    Put(32, Room(0x1b000), 32, 32, 0x0, 0x0),
    Put(33, Room(0x1b040), 32, 32, 0x0, 0x0),
    Pop,
    Put(31, Room(0x1c000), 32, 32, 0x0, 0x0),
    Fill(Room(0x1c040), 32, 32),  // cell 0
    Put(30, Room(0x1d000), 32, 32, 0x0, 0x0),
    Fill(Room(0x1d040), 32, 32),  // cell 0
    Put(28, Room(0x1e000), 32, 32, 0x0, 0x0),
    Put(29, Room(0x1e040), 32, 32, 0x0, 0x0),
    Pop,
    Put(26, Room(0x1f000), 32, 32, 0x0, 0x0),
    Put(27, Room(0x1f040), 32, 32, 0x0, 0x0),
    Put(26, Room(0x20000), 32, 32, 0x0, 0x0),
    Put(27, Room(0x20040), 32, 32, 0x0, 0x0),
    Pop,
    Put(60, Room(0x21000), 32, 32, 0x0, 0x0),
    Put(61, Room(0x21040), 32, 32, 0x0, 0x0),
    Put(58, Room(0x22000), 32, 32, 0x0, 0x0),
    Put(59, Room(0x22040), 32, 32, 0x0, 0x0),
    Pop,
    Put(57, Room(0x23000), 32, 32, 0x0, 0x0),
    Fill(Room(0x23040), 32, 32),  // cell 0
    Fill(Room(0x24000), 32, 32),  // cell 0
    Fill(Room(0x24040), 32, 32),  // cell 0
    Put(47, Room(0x25000), 32, 32, 0x0, 0x0),
    Put(56, Room(0x25040), 32, 32, 0x0, 0x0),
    Put(25, Room(0x26000), 32, 32, 0x0, 0x0),
    Pop,
    Put(36, Room(0x26040), 32, 32, 0x0, 0x0),
    Put(25, Room(0x27000), 32, 32, 0x0, 0x0),
    Put(36, Room(0x27040), 32, 32, 0x0, 0x0),
    Put(86, Room(0x2c000), 32, 32, 0x0, 0x0),
    Pop,
    Put(86, Room(0x2c040), 32, 32, 0x0, 0x0),
    Put(85, Room(0x2d000), 32, 32, 0x0, 0x0),
    Put(85, Room(0x2d040), 32, 32, 0x0, 0x0),
    Put(87, Room(0x3b000), 32, 32, 0x0, 0x0),
    Pop,
    Put(87, Room(0x3b040), 32, 32, 0x0, 0x0),
    Put(79, Room(0x2e000), 32, 32, 0xf, 0x400),
    Put(80, Room(0x2e040), 32, 32, 0xf, 0x400),
    Put(80, Room(0x2f000), 32, 32, 0xf, 0x400),
    Pop,
    Put(79, Room(0x2f040), 32, 32, 0xf, 0x400),
    Put(93, Room(0x38000), 32, 32, 0x0, 0x4),
    Put(94, Room(0x38040), 32, 32, 0x0, 0x4),
    Put(95, Room(0x39000), 32, 32, 0x0, 0x2b),
    Pop,
    Put(95, Room(0x39040), 32, 32, 0x0, 0x2b),
    Put(96, Room(0x3a000), 32, 32, 0x0, 0x2b),
    Put(96, Room(0x3a040), 32, 32, 0x0, 0x2b),
    Palette(1, 0x100, 0x100),
    Cells(2, At(0x25e62a40), 0xb4a0),
    Put(17, At(0x25e74000), 32, 32, 0x0, 0x152),
    Pop,
    Put(18, At(0x25e74040), 32, 32, 0x0, 0x152),
    Put(18, At(0x25e75000), 32, 32, 0x0, 0x152),
    Put(17, At(0x25e75040), 32, 32, 0x0, 0x152),
    Put(81, Room(0x30000), 32, 32, 0x0, 0x0),
    Pop,
    Put(82, Room(0x30040), 32, 32, 0x0, 0x0),
    Put(81, Room(0x31000), 32, 32, 0x0, 0x0),
    Put(82, Room(0x31040), 32, 32, 0x0, 0x0),
    Put(83, Room(0x32000), 32, 32, 0x0, 0x0),
    Pop,
    Put(84, Room(0x32040), 32, 32, 0x0, 0x0),
    Put(83, Room(0x33000), 32, 32, 0x0, 0x0),
    Put(84, Room(0x33040), 32, 32, 0x0, 0x0),
    Palette(2, 0x200, 0x10),
];

/// The stage's screens' start (screen.rs's scroll table): the screen room kept; the first time
/// (its count at 0x060a777c, counted each time) the overlay's two lists of screen addresses
/// moved into the room; the stage data set; RBG0's rotation parameters and coefficient table (this
/// overlay's own SGL setters), its cells, the back colour; NBG0 and NBG1's cells and maps; the
/// screens shown and their priorities, the sprites' colour calculation and windows; then the maps,
/// cells and colours (STAGE_SCREENS); the scroll marks given.
const STAGE_ROOM: u32 = 0x0607_47a0;
const SCREEN_STARTS: u32 = 0x060a_777c;
const ROOM_LISTS: [(u32, u32); 2] = [(0x060a_7860, 4), (0x060a_7870, 38)];
const RPARA_TABLES: u32 = 0x25e3_fe00;
const KTABLE: u32 = 0x25e2_0000;
const PRIORITIES: [(u32, u32); 5] = [(1, 6), (0, 4), (5, 3), (-7i32 as u32, 5), (-8i32 as u32, 4)];
const SL_RPARA_INIT: u32 = 0x060a_2524;
const SL_KTABLE_MAKE: u32 = 0x060a_2594;
const SL_RPARA_A: [(u32, &[u32]); 3] = [(0x060a_2614, &[0]), (0x060a_2624, &[0]), (0x060a_2634, &[KTABLE, 0x61])];
const SL_RPARA_B: [(u32, &[u32]); 5] = [(0x060a_26b4, &[0]), (0x060a_26c4, &[0]), (0x060a_26d4, &[0]), (0x060a_26e4, &[0x060a_7adc]), (0x060a_2754, &[0])];
const SL_KTABLE_B: u32 = 0x060a_2764;
/// The screen record's scroll marks (+0x54) and the marks it is given.
const SCROLL_MARKS: u32 = 0x54;
const MARKS: u32 = 0x060a_7780;

pub fn stg02_start(cpu: &mut Cpu, data: u32, room: u32) {
    cpu.write32(STAGE_ROOM, room);
    let starts = cpu.read32(SCREEN_STARTS);
    cpu.write32(SCREEN_STARTS, starts.wrapping_add(1));
    if starts == 0 {
        for (list, n) in ROOM_LISTS {
            let room = cpu.read32(STAGE_ROOM);
            nested!(cpu, 0x0605_ef20, screen::offset_list(cpu, room, list, n));
        }
    }
    nested!(cpu, 0x0605_e17c, states::stage_data_set(cpu, data));
    cpu.call(SL_RPARA_INIT, &[RPARA_TABLES]);
    cpu.call(SL_KTABLE_MAKE, &[KTABLE]);
    nested!(cpu, 0x0606_e27c, sgl::sl_char_rbg0(cpu, 0, 0));
    nested!(cpu, 0x0606_e2ce, sgl::sl_page_rbg0(cpu, 0x25e0_0000, 0, 0xc000));
    cpu.call(SL_RPARA_A[0].0, SL_RPARA_A[0].1);
    nested!(cpu, 0x0607_1c4c, sgl::sl_map_rbg0(cpu, 0x25e4_0000));
    for (f, args) in &SL_RPARA_A[1..] {
        cpu.call(*f, args);
    }
    for (f, args) in SL_RPARA_B {
        cpu.call(f, args);
    }
    cpu.call(SL_KTABLE_B, &[KTABLE, 0x60]);
    nested!(cpu, 0x0607_0dec, sgl::sl_back1_col_set(cpu, 0x25e3_fffe, 0));
    nested!(cpu, 0x0606_e288, sgl::sl_char_nbg0(cpu, 0, 0));
    nested!(cpu, 0x0606_e300, sgl::sl_page_nbg0(cpu, 0x25e6_0000, 0x200, 0x8000));
    nested!(cpu, 0x0607_0c3c, sgl::sl_plane_nbg0(cpu, 0));
    nested!(cpu, 0x0607_0c4c, sgl::sl_map_nbg0(cpu, 0x25e7_6000, 0x25e7_6000, 0x25e7_6000, 0x25e7_6000));
    nested!(cpu, 0x0607_0cfc, sgl::sl_zoom_mode(cpu, 1, 1));
    nested!(cpu, 0x0606_e294, sgl::sl_char_nbg1(cpu, 0, 0));
    nested!(cpu, 0x0606_e2e4, sgl::sl_page_nbg1(cpu, 0x25e6_0000, 0x400, 0xc000));
    nested!(cpu, 0x0607_0d0c, sgl::sl_plane_nbg1(cpu, 0));
    nested!(cpu, 0x0607_0d1c, sgl::sl_map_nbg1(cpu, 0x25e7_4000, 0x25e7_4000, 0x25e7_4000, 0x25e7_4000));
    nested!(cpu, 0x0607_0cfc, sgl::sl_zoom_mode(cpu, 0, 0));
    nested!(cpu, 0x0606_dd6c, sgl::sl_scr_auto_disp(cpu, 0x17));
    nested!(cpu, 0x0606_e23c, sgl::sl_bg_on(cpu, 0x13_0004));
    for (screen, pri) in PRIORITIES {
        nested!(cpu, 0x0606_98dc, sgl::sl_priority(cpu, screen, pri));
    }
    cpu.call(SL_SCENE_SIZE, &[5]);
    nested!(cpu, 0x0607_0ddc, sgl::sl_color_calc(cpu, 0x101));
    cpu.call(0x060a_27d4, &[1]);
    cpu.call(0x060a_27f4, &[4]);
    nested!(cpu, 0x0606_713c, sgl::sl_cur_window(cpu, 0));
    nested!(cpu, 0x0606_990c, sgl::sl_zdsp_level(cpu, 7));
    nested!(cpu, 0x0606_713c, sgl::sl_cur_window(cpu, 1));
    let place = |cpu: &mut Cpu, at: ScreenAt| match at {
        Room(off) => cpu.read32(STAGE_ROOM).wrapping_add(off),
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
            Fill(at, w, h) => {
                let dst = place(cpu, at);
                nested!(cpu, 0x0605_eee4, screen::fill_cells(cpu, 0, dst, w, h));
            }
            Cells(k, at, bytes) => {
                let src = states::stage_part(cpu, states::CELLS, k);
                let dst = place(cpu, at);
                nested!(cpu, 0x0605_edf8, screen::copy_words(cpu, src, dst, bytes));
            }
            Palette(k, index, count) => {
                let colours = states::stage_part(cpu, states::COLOURS, k);
                nested!(cpu, 0x0600_5b18, palette::set_colours(cpu, colours, index, count));
            }
            Pop => cpu.pop(32),
        }
    }
    let s = cpu.read32(screen::SCREEN);
    cpu.write32(s + SCROLL_MARKS, MARKS);
}
