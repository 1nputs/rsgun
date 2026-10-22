//! STG02.BIN's code, which the stage 2 mode runs in the overlay window (its screen data is
//! SCRDAT21 and SCRDAT22): its mode's code, its enemies and scenery, its scrolls.

use super::globals::{BIOS_SETTING, GAME_MODE};
use super::bullets::{AS_ANGLE, AS_FLAGS, AS_POSITION, AS_SHOOTER, AS_SPEED, S_ANGLE, S_NUMBER, S_POSITION};
use super::{actors, aim, bg, bullets, camera, clib, dma, effects, explosions, files, ground, hits, kinds, hud, math, models, nested, objects, palette, parts, results, screen, sgl, sound, sprites, states, timeline, Cpu};

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
