//! STG02.BIN's code, which the stage 2 mode runs in the overlay window (its screen data is
//! SCRDAT21 and SCRDAT22): its mode's code, its enemies and scenery, its scrolls.

use super::globals::{BIOS_SETTING, GAME_MODE};
use super::bullets::{AS_ANGLE, AS_FLAGS, AS_SHOOTER, AS_SPEED};
use super::{actors, aim, bg, bullets, camera, clib, dma, effects, files, hits, kinds, math, models, nested, objects, parts, screen, sgl, sound, states, timeline, Cpu};

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

