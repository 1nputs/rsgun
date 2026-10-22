//! STG04.BIN's code (fingerprint 5c18364ee98aacb1), which game mode 10 runs in the overlay window:
//! its timeline's events, its drones (the swoopers and the gunners) and their launchers.

use super::actors::{E_ANGLE, E_COLOUR, E_POSITION, E_SCALES, E_SCRIPT, E_SPEED};
use super::globals::{bios_setting_is_1, GAME_FLAGS, GAME_FRAMES, GAME_MODE, HELD};
use super::{actors, aim, bullets, files, gouraud, hits, dma, hud, kinds, math, models, effects, nested, objects, palette, player, ribbons, results, screen, sgl, sound, sprites, states, text, timeline, Cpu};

// ---- the stage's mode ----------------------------------------------------------------------------

/// The director's slots the mode uses (as STG05's mode uses them): its step, the load (a local
/// written back each time), the settings kept (the buffer +0x6c, the room's top +0x70, VDP1's next
/// character place +0x74, its next character and colour +0x78 and +0x7c), the demo's input
/// (+0x54), the mode's music (+0xa4).
const MODE_STEP: u32 = 0x64;
const MODE_LOAD: u32 = 0x68;
const KEPT: u32 = 0x6c;
const PLACE: u32 = 0x70;
const VDP1_PLACE: u32 = 0x74;
const NEXT_CHAR: u32 = 0x78;
const NEXT_COLOUR: u32 = 0x7c;
const DEMO_INPUT: u32 = 0x54;
const MUSIC: u32 = 0xa4;
/// The room the mode takes from the top of the play's room, and that top.
const ROOM: u32 = 0x4_4040;
const ROOM_TOP: u32 = 0x0607_4050;
const NEXT_RATE: u32 = 0x0603_2e40;
const STAGE_RATE: u32 = 0x2_0000;
/// The sound data files loaded at once (the defaults' set-up after the first, the music's number
/// after the third), and the two model kinds read and put in place.
const SOUND_FILES: [u32; 5] = [1, 0x11, 0x10, 0x2c, 0x2b];
const MODE_KINDS: [u32; 2] = [0x20, 0x33];
/// The stages' screen data files (the stages' SCRDAT files): this stage's, the fourth entry of
/// the table; the screen's scroll table, its timeline, its stage sound effects, the mode after
/// the play and after a demo, the game mode of a demo; the demo's input files (BIOS setting not
/// 1, and 1).
const SCREEN_FILE: u32 = 0x0607_35b0 + 0xc;
const SCROLLS: u32 = 0x0608_9dd8;
const EVENTS: u32 = 0x0608_7fa0;
const SOUND_EFFECTS: u32 = 0x0608_9e38;
const STAGE_SE_TABLE: u32 = 0x0600_edd4;
const NEXT_MODE: u32 = 0x0600_ed70;
const AFTER_DEMO: u8 = 0;
const AFTER_PLAY: u8 = 11;
const DEMO: u8 = 15;
const DEMO_FILES: [u32; 2] = [0x0600_8c48, 0x0600_8c18];

/// The mode's code (the director calls it each frame until it gives 1): the settings kept, the
/// sound data loaded, the stage's two model kinds read into the buffer and put in place, the
/// screen data file loaded into the buffer; once DMA is idle the screen started; the timeline
/// made (in a demo, its input file read after the buffer), the settings given back; 1.
pub fn stg04_mode(cpu: &mut Cpu, game: u32) -> u32 {
    let mut load = cpu.read32(game + MODE_LOAD);
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
                    cpu.write16(game + MUSIC, 7);
                }
            }
            let top = cpu.read32(ROOM_TOP);
            cpu.write32(game + PLACE, top);
            cpu.write8(game + MODE_STEP, 9);
        }
        9 => {
            for k in MODE_KINDS {
                let buffer = cpu.read32(game + KEPT);
                nested!(cpu, 0x0604_58cc, kinds::model_kind_read(cpu, k, buffer));
                let vram = cpu.read32(game + VDP1_PLACE);
                let first = cpu.read16(game + NEXT_CHAR) as i16 as i32 as u32;
                let colour = cpu.read16(game + NEXT_COLOUR) as u32;
                let next = nested!(cpu, 0x0604_5950, kinds::model_kind_install(cpu, k, vram, first, colour));
                cpu.write32(game + VDP1_PLACE, next);
                let file = cpu.read32(KINDS + K_SIZE * k);
                let chars = cpu.read16(game + NEXT_CHAR).wrapping_add(cpu.read16(file + KF_CHARS));
                cpu.write16(game + NEXT_CHAR, chars);
                if k == MODE_KINDS[1] {
                    let colours = cpu.read16(game + NEXT_COLOUR).wrapping_add(cpu.read16(file + KF_COLOURS));
                    cpu.write16(game + NEXT_COLOUR, colours);
                }
                let past = cpu.read32(file);
                cpu.write32(game + KEPT, past);
            }
            let name = cpu.read32(SCREEN_FILE);
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
                let room = nested!(cpu, 0x0605_fe78, states::stage_table(cpu, 3));
                let screen_room = cpu.read32(game + PLACE).wrapping_sub(room);
                cpu.write32(game + PLACE, screen_room);
                let buffer = cpu.read32(game + KEPT);
                nested!(cpu, 0x0605_e3e0, screen::screen_start(cpu, SCROLLS, buffer, screen_room));
                cpu.write8(game + MODE_STEP, 12);
            }
        }
        12 => cpu.write8(game + MODE_STEP, 13),
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
    cpu.write32(game + MODE_LOAD, load);
    done
}

// ---- the stage's events --------------------------------------------------------------------------

/// A timeline event's step (+1, a byte), the results it waits on (+4), its wait (+6, a word), the
/// number it waits on (+8), its load's handle, place and VRAM place (+0x10, +0x14, +0x18), and the
/// next character and colour it loads at (+0x1c and +0x1e, words).
const EV_STEP: u32 = 1;
const EV_RESULTS: u32 = 4;
const EV_WAIT: u32 = 6;
const EV_NUMBER: u32 = 8;
const EV_LOAD: u32 = 0x10;
const EV_AT: u32 = 0x14;
const EV_VRAM: u32 = 0x18;
const EV_CHAR: u32 = 0x1c;
const EV_COLOUR: u32 = 0x1e;
/// The opening message's lines: the first of each message's four, then the other three (12 bytes
/// for each message).
const MESSAGE_FIRST_LINES: u32 = 0x0608_7f50;
const MESSAGE_LINES: u32 = 0x0608_7f64;
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

/// The play's own places for more kinds (where the next file loads, VDP1's VRAM's next place, the
/// next character and colour), copied into an event as it loads.
const PLAY_LOAD_AT: u32 = 0x0607_40e4;
const PLAY_VRAM: u32 = 0x0607_4058;
const PLAY_CHAR: u32 = 0x0607_40ea;
const PLAY_COLOUR: u32 = 0x0607_40e8;
/// The kinds' table (16 bytes each: the file at +0, its attributes' word at +0xa, its first
/// colour at +0xc), a kind's file's characters (+6, a word), colours (+0x16, a word) and the place
/// after it (+0); a kind's attributes' mode bits (8: its colours shared).
const KINDS: u32 = 0x0607_41f0;
const K_SIZE: u32 = 16;
const K_ATTRS: u32 = 0xa;
const K_COLOUR: u32 = 0xc;
const KF_CHARS: u32 = 6;
const KF_COLOURS: u32 = 0x16;
const K_MODE_BITS: u16 = 0x38;
const K_SHARED: u16 = 8;
/// The stage's model kind and sprite kind, and how many of the model's frames are shaded (all
/// 18) and lit (all but its 12th and 13th).
const SCENERY_MODEL: u32 = 75;
const SCENERY_SPRITE: u32 = 74;
const SCENERY_FRAMES: u32 = 18;
const UNLIT_FRAMES: [u32; 2] = [11, 12];

/// Kind `k` (a model's, or a sprite's) fixed and put in place at the event's places, which move
/// on past it (its colours too unless shared with others').
fn ev_kind_install(cpu: &mut Cpu, e: u32, k: u32, model: bool) {
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
    if cpu.read16(kind + K_ATTRS) & K_MODE_BITS != K_SHARED {
        let file = cpu.read32(kind);
        let c = cpu.read16(e + EV_COLOUR).wrapping_add(cpu.read16(file + KF_COLOURS));
        cpu.write16(e + EV_COLOUR, c);
    }
}

/// The stage's kinds' event, a step a frame: model kind 75 loaded and put in place, its frames
/// shaded and 16 of the 18 lit (not the 12th and 13th); then sprite kind 74 loaded and put in
/// place (the play's next colour moves on past its colours). Then done (1).
pub fn kinds_event(cpu: &mut Cpu, e: u32) -> u32 {
    let mut load = cpu.read32(e + EV_LOAD);
    let mut done = 0;
    match cpu.read8(e + EV_STEP) {
        0 => {
            let at = cpu.read32(PLAY_LOAD_AT);
            cpu.write32(e + EV_AT, at);
            let vram = cpu.read32(PLAY_VRAM);
            cpu.write32(e + EV_VRAM, vram);
            let ch = cpu.read16(PLAY_CHAR);
            cpu.write16(e + EV_CHAR, ch);
            let colour = cpu.read16(PLAY_COLOUR);
            cpu.write16(e + EV_COLOUR, colour);
            cpu.write8(e + EV_STEP, 1);
        }
        1 => {
            let at = cpu.read32(e + EV_AT);
            load = nested!(cpu, 0x0604_5910, kinds::kind_load(cpu, SCENERY_MODEL, at));
            if load != 0 {
                cpu.write8(e + EV_STEP, 2);
            }
        }
        2 => {
            if nested!(cpu, 0x0600_6074, files::load_done(cpu, load)) != 0 {
                ev_kind_install(cpu, e, SCENERY_MODEL, true);
                let file = cpu.read32(KINDS + K_SIZE * SCENERY_MODEL);
                let next = cpu.read32(file);
                cpu.write32(e + EV_AT, next);
                nested!(cpu, 0x0606_2f74, gouraud::models_hss(cpu, SCENERY_MODEL, SCENERY_FRAMES));
                for frame in 0..SCENERY_FRAMES {
                    if !UNLIT_FRAMES.contains(&frame) {
                        nested!(cpu, 0x0606_333c, models::model_lit(cpu, SCENERY_MODEL, frame));
                    }
                }
                cpu.write8(e + EV_STEP, 3);
            }
        }
        3 => cpu.write8(e + EV_STEP, 4),
        4 => {
            let at = cpu.read32(e + EV_AT);
            load = nested!(cpu, 0x0604_56cc, kinds::kind_load(cpu, SCENERY_SPRITE, at));
            if load != 0 {
                cpu.write8(e + EV_STEP, 5);
            }
        }
        5 => {
            if nested!(cpu, 0x0600_6074, files::load_done(cpu, load)) != 0 {
                ev_kind_install(cpu, e, SCENERY_SPRITE, false);
                let file = cpu.read32(KINDS + K_SIZE * SCENERY_SPRITE);
                let colours = cpu.read16(PLAY_COLOUR).wrapping_add(cpu.read16(file + KF_COLOURS));
                cpu.write16(PLAY_COLOUR, colours);
                let next = cpu.read32(file);
                cpu.write32(e + EV_AT, next);
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

/// The boss's event: its object made (0x06086710: its number kept at +8, over again until it has
/// one); once its number's slot is empty again, the stage's results (60 frames on, kept at +4);
/// once they are over (mode past 111), the scroll moved on; then -1: done.
const NUMBERED: u32 = 0x0607_40f0;
const BOSS_SET: u32 = 0x0608_6710;
const RESULTS_DELAY: u32 = 60;
const OBJ_MODE: u32 = 3;
const TO_STOP: u8 = b'p';
const FREED: u8 = b's';

pub fn boss_event(cpu: &mut Cpu, e: u32) -> u32 {
    match cpu.read8(e + EV_STEP) {
        0 => {
            let n = cpu.call(BOSS_SET, &[]);
            if n != 0 {
                let step = cpu.read8(e + EV_STEP).wrapping_add(1);
                cpu.write8(e + EV_STEP, step);
                cpu.write32(e + EV_NUMBER, n);
            }
            0
        }
        1 => {
            let n = cpu.read32(e + EV_NUMBER);
            if cpu.read32(NUMBERED.wrapping_add(n << 2)) != 0 {
                return 0;
            }
            let r = nested!(cpu, 0x0605_284c, results::results_set(cpu, RESULTS_DELAY));
            if r != 0 {
                let step = cpu.read8(e + EV_STEP).wrapping_add(1);
                cpu.write8(e + EV_STEP, step);
                cpu.write32(e + EV_RESULTS, r);
            }
            0
        }
        2 => {
            let results = cpu.read32(e + EV_RESULTS);
            if cpu.read8(results + OBJ_MODE) as i8 > (TO_STOP - 1) as i8 {
                nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
                let step = cpu.read8(e + EV_STEP).wrapping_add(1);
                cpu.write8(e + EV_STEP, step);
            }
            0
        }
        3 => 0xffff_ffff,
        _ => 0,
    }
}

// ---- the drones' colours -------------------------------------------------------------------------

/// The palette's copy in work RAM (colours, words), and where the stage keeps the two faded sets
/// of colours it makes (words: the play's next colour as each was made).
const CRAM_IMAGE: u32 = 0x0603_0e40;
const FADED_COLOURS: u32 = 0x0608_9ed2;
const FADED_COLOURS_2: u32 = 0x0608_9ed0;
const FADE: u32 = 0x2aaa;
const FADED_COUNT: u32 = 48;

/// 48 colours (three banks) from colour `from` of the palette's copy, each of their red, green
/// and blue (five bits each) faded toward black by 1/6 (0x2aaa), set as colours from `to` on.
pub fn fade_colours(cpu: &mut Cpu, from: u32, to: u32) {
    let (from, to) = (from & 0xffff, to & 0xffff);
    let colours = cpu.stack();
    cpu.write32(colours + 2 * FADED_COUNT, to);
    for k in 0..FADED_COUNT {
        let c = cpu.read16(CRAM_IMAGE + 2 * from + 2 * k) as i16 as i32 as u32;
        let [r, g, b] = [c & 31, (c >> 5) & 31, (c >> 10) & 31].map(|v| {
            let faded = nested!(cpu, 0x0606_2e7c, math::lerp(cpu, v << 16, 0, FADE));
            (faded >> 16) as i16 as i32 as u32
        });
        cpu.write16(colours + 2 * k, ((b << 10) | (g << 5) | r) as u16);
    }
    nested!(cpu, 0x0600_5b18, palette::set_colours(cpu, colours, to, FADED_COUNT));
}

/// The stage's two faded sets of colours: kind 6's, then kind 14's, each made at the play's next
/// colour (kept at 0x06089ed2 and 0x06089ed0), which moves on 48 for each.
const FADED_KINDS: [u32; 2] = [6, 14];

pub fn colours_set(cpu: &mut Cpu) {
    let colour = cpu.read16(PLAY_COLOUR);
    cpu.write16(FADED_COLOURS, colour);
    let from = cpu.read16(KINDS + K_SIZE * FADED_KINDS[0] + K_COLOUR) as u32;
    nested!(cpu, 0x0608_0674, fade_colours(cpu, from, colour as u32));
    let colour = cpu.read16(PLAY_COLOUR).wrapping_add(FADED_COUNT as u16);
    cpu.write16(PLAY_COLOUR, colour);
    cpu.write16(FADED_COLOURS_2, colour);
    let from = cpu.read16(KINDS + K_SIZE * FADED_KINDS[1] + K_COLOUR) as u32;
    nested!(cpu, 0x0608_0674, fade_colours(cpu, from, colour as u32));
    let colour = cpu.read16(PLAY_COLOUR).wrapping_add(FADED_COUNT as u16);
    cpu.write16(PLAY_COLOUR, colour);
}

// ---- the drones ----------------------------------------------------------------------------------

/// A drone (a numbered actor): its sprite (flags +0x10, position +0x14, the sprite's colours
/// +0x28 and +0x2a, its kind and frame +0x2e and +0x2f, its turn +0x30), its number (+0x32), its
/// lift (+0x33), behaviour (+0x38), hits (+0x3c), life (+0x50), its side and type (+0x54, +0x55),
/// its phase (+0x6c), whether it has been seen on screen (+0x6d), its heading (+0x78, a word), its
/// time through its phase (+0x8c, 16.16) and its speed (+0x90).
const FLAGS: u32 = 0x10;
const POSITION: u32 = 0x14;
const COLOUR: u32 = 0x28;
const OWN_COLOUR: u32 = 0x2a;
const KIND: u32 = 0x2e;
const FRAME: u32 = 0x2f;
const TURN: u32 = 0x30;
const NUMBER: u32 = 0x32;
const LIFT: u32 = 0x33;
const BEHAVIOUR: u32 = 0x38;
const HIT: u32 = 0x3c;
const LIFE: u32 = 0x50;
const SIDE: u32 = 0x54;
const TYPE: u32 = 0x55;
const PHASE: u32 = 0x6c;
const SEEN: u32 = 0x6d;
const HEADING: u32 = 0x78;
const PHASE_TIME: u32 = 0x8c;
const SPEED: u32 = 0x90;
/// What a drone is made from (a struct on the maker's stack): its position, its turn (a word),
/// its number (a byte), its type (a word, the index into the types' table), its behaviour.
const SPEC_POSITION: u32 = 0;
const SPEC_TURN: u32 = 0x18;
const SPEC_NUMBER: u32 = 0x1e;
const SPEC_TYPE: u32 = 0x20;
const SPEC_BEHAVIOUR: u32 = 0x3c;
/// The drones' hit set, the table of their types' colour banks, a drone's flags (0x20000000: not
/// culled; 0x80000: drawn faded), its life, kind and size, where numbers are taken from and the
/// exec of a numbered actor.
const DRONE_HITS: u32 = 0x0607_3278;
const DRONE_TYPES: u32 = 0x0608_8020;
const NOT_CULLED: u32 = 0x2000_0000;
const FADED: u32 = 0x8_0000;
const DRONE_LIFE: u32 = 300;
const DRONE_KIND: u32 = 6;
const DRONE_SIZE: u32 = 0xb4;
const DRONE_NUMBERS: u32 = 2;
const NUMBERED_EXEC: u32 = 0x0604_d600;
/// The first character of the sprites' kinds (kind k's, 16 bytes each from it).
const CHAR_BASE: u32 = 0x0607_41fc;
/// A step through a phase: 0x400 (a 64th).
const STEP: u32 = 0x400;
const ONE: u32 = 0x1_0000;

/// A drone set up from `spec` (a struct on the stack: position, turn, number, type, behaviour): its
/// sprite and hits made, its flags (not culled), behaviour, number (taken in the numbers' table),
/// position, turn and heading, lift -8, hits the drones', side 1, its type's colour bank, phase 0
/// and unseen.
pub fn drone_setup(cpu: &mut Cpu, o: u32, spec: u32) {
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + FLAGS));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HIT));
    let flags = cpu.read32(o + FLAGS) | NOT_CULLED;
    cpu.write32(o + FLAGS, flags);
    let behaviour = cpu.read32(spec + SPEC_BEHAVIOUR);
    cpu.write32(o + BEHAVIOUR, behaviour);
    let number = cpu.read8(spec + SPEC_NUMBER);
    cpu.write8(o + NUMBER, number);
    cpu.write32(NUMBERED.wrapping_add(4 * (number as i8 as i32 as u32)), o);
    for k in 0..3 {
        let v = cpu.read32(spec + SPEC_POSITION + 4 * k);
        cpu.write32(o + POSITION + 4 * k, v);
    }
    let turn = cpu.read16(spec + SPEC_TURN);
    cpu.write16(o + TURN, turn);
    cpu.write16(o + HEADING, turn);
    cpu.write8(o + LIFT, 0xf8);
    cpu.write32(o + HIT, DRONE_HITS);
    cpu.write8(o + SIDE, 1);
    let ty = cpu.read16(spec + SPEC_TYPE) as u32;
    let ty = cpu.read8(DRONE_TYPES + ty);
    cpu.write8(o + TYPE, ty);
    cpu.write8(o + PHASE, 0);
    cpu.write8(o + SEEN, 0);
}

/// A drone's sprite colours (both): the word at `base` (a kind's first colour, or the faded set's)
/// with its type's colour bank (16 colours each) added.
fn drone_colour(cpu: &mut Cpu, o: u32, base: u32) {
    let ty = cpu.read8(o + TYPE) as i8 as i16;
    let colour = cpu.read16(base).wrapping_add((ty << 4) as u16);
    cpu.write16(o + OWN_COLOUR, colour);
    cpu.write16(o + COLOUR, colour);
}

/// A drone's last set-up: its frame 0, its kind 6, its life 300, its colours.
pub fn drone_defaults(cpu: &mut Cpu, o: u32) {
    cpu.write8(o + FRAME, 0);
    cpu.write8(o + KIND, DRONE_KIND as u8);
    cpu.write32(o + LIFE, DRONE_LIFE);
    drone_colour(cpu, o, CHAR_BASE + 16 * DRONE_KIND);
}

/// The effect of a small explosion (a drifting effect's description `e`): its script, its colour
/// 0x90, its scales one; made.
const EXPLOSION_SCRIPT: u32 = 0x0607_305c;
const EXPLOSION_COLOUR: u16 = 0x90;

pub fn explosion_effect(cpu: &mut Cpu, e: u32) -> u32 {
    cpu.write32(e + E_SCRIPT, EXPLOSION_SCRIPT);
    cpu.write16(e + E_COLOUR, EXPLOSION_COLOUR);
    cpu.write32(e + E_SCALES, ONE);
    cpu.write32(e + E_SCALES + 4, ONE);
    nested!(cpu, 0x0605_7454, actors::drift_effect(cpu, e))
}

/// The explosion of the sprite at `p` (its position at +4, its turn at +0x20, its kind at +0x1e):
/// an explosion effect there (drifting at 0x12000), with the sound 12 for kind 6, else 64.
const EXPLOSION_SPEED: u32 = 0x1_2000;
const SOUND_BIG: u32 = 12;
const SOUND_SMALL: u32 = 64;
const SPRITE_POSITION: u32 = 4;
const SPRITE_TURN: u32 = 0x20;
const SPRITE_KIND: u32 = 0x1e;

pub fn explosion(cpu: &mut Cpu, p: u32) -> u32 {
    let e = cpu.stack();
    for k in 0..3 {
        let v = cpu.read32(p + SPRITE_POSITION + 4 * k);
        cpu.write32(e + E_POSITION + 4 * k, v);
    }
    let turn = cpu.read16(p + SPRITE_TURN);
    cpu.write16(e + E_ANGLE, turn);
    cpu.write32(e + E_SPEED, EXPLOSION_SPEED);
    nested!(cpu, 0x0608_084c, explosion_effect(cpu, e));
    let se = if cpu.read8(p + SPRITE_KIND) as i8 == DRONE_KIND as i8 { SOUND_BIG } else { SOUND_SMALL };
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, se))
}

/// The angle from the point at `at` to where the player `o` aims at will be `delay` frames on
/// (aim.rs's target), a quarter turn on (a word, sign-extended).
pub fn aim_angle(cpu: &mut Cpu, o: u32, at: u32, delay: u32) -> u32 {
    let t = cpu.stack();
    nested!(cpu, 0x0604_0ccc, aim::target(cpu, o, delay, t));
    let dx = cpu.read32(t).wrapping_sub(cpu.read32(at));
    cpu.write32(t, dx);
    let dy = cpu.read32(t + 4).wrapping_sub(cpu.read32(at + 4));
    cpu.write32(t + 4, dy);
    let a = nested!(cpu, 0x0606_780c, sgl::sl_atan(cpu, dx, dy)) as i16;
    a.wrapping_add(0x4000) as i32 as u32
}

/// A drone's shot: a plain bullet from its place (z 0), of its number, at the angle to the player
/// 15 frames on.
const SHOT_AIM_FRAMES: u32 = 15;

pub fn gunner_shoot(cpu: &mut Cpu, o: u32) -> u32 {
    let s = cpu.stack();
    for k in 0..2 {
        let v = cpu.read32(o + POSITION + 4 * k);
        cpu.write32(s + bullets::S_POSITION + 4 * k, v);
    }
    cpu.write32(s + bullets::S_POSITION + 8, 0);
    let number = cpu.read8(o + NUMBER);
    cpu.write8(s + bullets::S_NUMBER, number);
    let angle = nested!(cpu, 0x0608_08e0, aim_angle(cpu, o, o + POSITION, SHOT_AIM_FRAMES));
    cpu.write16(s + bullets::S_ANGLE, angle as u16);
    nested!(cpu, 0x0605_3f48, bullets::bullet_set_plain(cpu, s))
}

/// SGL's check of a position with 16 dots about it (negative: off the screen).
const VIEW_SIZE: u32 = 0x10_0000;

/// Whether the drone is off the screen: 1 if so.
pub fn out_of_view(cpu: &mut Cpu, o: u32) -> u32 {
    let seen = nested!(cpu, 0x0606_e9fc, sgl::sl_check_on_screen(cpu, o + POSITION, VIEW_SIZE));
    ((seen as i32) < 0) as u32
}

/// Whether the drone has left the screen: once seen on it (its flag set the first frame it is
/// not off), off again: 1.
pub fn left_screen(cpu: &mut Cpu, o: u32) -> u32 {
    let seen = cpu.read8(o + SEEN) != 0;
    let check = nested!(cpu, 0x0606_e9fc, sgl::sl_check_on_screen(cpu, o + POSITION, VIEW_SIZE)) as i32;
    if seen {
        (check < 0) as u32
    } else {
        if check >= 0 {
            cpu.write8(o + SEEN, 1);
        }
        0
    }
}

/// A drone's move (inline in the first two behaviours): its position moved (below); its frame (+0x2f) a step on every other game frame,
/// round 32.
fn drone_move(cpu: &mut Cpu, o: u32) {
    drone_shift(cpu, o);
    if cpu.read32(GAME_FRAMES) & 1 != 0 {
        let frame = cpu.read8(o + FRAME).wrapping_add(1);
        cpu.write8(o + FRAME, frame);
        if frame == 32 {
            cpu.write8(o + FRAME, 0);
        }
    }
}

/// A drone's position moved by its speed along its heading (x by the sine, y back by the cosine).
fn drone_shift(cpu: &mut Cpu, o: u32) {
    let heading = cpu.read16(o + HEADING) as i16 as i32 as u32;
    let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, heading));
    let speed = cpu.read32(o + SPEED);
    let dx = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, speed, sin));
    let heading = cpu.read16(o + HEADING) as i16 as i32 as u32;
    let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, heading));
    let speed = cpu.read32(o + SPEED);
    let dy = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, speed, cos));
    let x = cpu.read32(o + POSITION).wrapping_add(dx);
    cpu.write32(o + POSITION, x);
    let y = cpu.read32(o + POSITION + 4).wrapping_sub(dy);
    cpu.write32(o + POSITION + 4, y);
}

/// The drone's time through its phase moved on a step (to one at most): the time.
fn phase_time_step(cpu: &mut Cpu, o: u32, step: u32) -> u32 {
    let t = cpu.read32(o + PHASE_TIME).wrapping_add(step);
    cpu.write32(o + PHASE_TIME, t);
    if t as i32 > 0xffff {
        cpu.write32(o + PHASE_TIME, ONE);
    }
    cpu.read32(o + PHASE_TIME)
}

fn next_phase(cpu: &mut Cpu, o: u32) {
    let phase = cpu.read8(o + PHASE).wrapping_add(1);
    cpu.write8(o + PHASE, phase);
}

/// The drone's colours as made (kind 6's, by its type), once it has arrived.
fn arrived_colour(cpu: &mut Cpu, o: u32) {
    drone_colour(cpu, o, CHAR_BASE + 16 * DRONE_KIND);
}

// ---- the swoopers --------------------------------------------------------------------------------

/// A swooper's (the first drones') own fields: the x it sets out from (+0x94), the x it swoops to
/// and the x it ends at (+0x98, +0x9c), the turn it starts and ends at (+0xa0, +0xa4).
const S_FROM_X: u32 = 0x94;
const S_TO_X: u32 = 0x98;
const S_END_X: u32 = 0x9c;
const S_TURN_FROM: u32 = 0xa0;
const S_TURN_TO: u32 = 0xa4;
const S_ARC_Z: u32 = 0xb0_0000;
const S_FIRST_SPEED: u32 = 0x4_0000;
const S_SWOOP_SPEED: u32 = 0x8_0000;
const S_BACK_SPEED: u32 = 0xfffe_0000;
const HALF_TURN: u32 = 0x8000;

/// A swooper's second phase (its first goes on into it): its time through it a 64th a frame; the
/// first half of it its z eased up to 0xb00000 (twice as fast); its speed eased from 0x80000 down to
/// none, its x from where it set out to the one it swoops to, its turn from 0 to a half turn. At
/// the end its speed none, its time 0, the next phase.
fn swooper_swoop(cpu: &mut Cpu, o: u32) {
    let t = phase_time_step(cpu, o, STEP);
    if t as i32 <= 0x8000 {
        let z = nested!(cpu, 0x0606_2ebc, math::ease(cpu, 0, S_ARC_Z, t * 2));
        cpu.write32(o + POSITION + 8, z);
    }
    let t = cpu.read32(o + PHASE_TIME);
    let speed = nested!(cpu, 0x0606_2ebc, math::ease(cpu, S_SWOOP_SPEED, 0, t));
    cpu.write32(o + SPEED, speed);
    let (a, b) = (cpu.read32(o + S_FROM_X), cpu.read32(o + S_TO_X));
    let x = nested!(cpu, 0x0606_2ebc, math::ease(cpu, a, b, t));
    cpu.write32(o + POSITION, x);
    let (a, b) = (cpu.read32(o + S_TURN_FROM), cpu.read32(o + S_TURN_TO));
    let turn = nested!(cpu, 0x0606_2ebc, math::ease(cpu, a, b, t));
    cpu.write16(o + TURN, turn as u16);
    if cpu.read32(o + PHASE_TIME) == ONE {
        cpu.write32(o + SPEED, 0);
        cpu.write32(o + PHASE_TIME, 0);
        next_phase(cpu, o);
    }
}

/// A swooper's behaviour. It ends (112) once it has left the screen (it was seen on it), or in an
/// explosion when its life is gone. Phase 0: its colours the faded ones, its speed 0x40000, its
/// turns 0 and a half turn (on one side of the turn), its x kept; on into phase 1: swoop (above);
/// phase 2, the time through it a 64th a frame: x eased from the swoop's to the end's, z back
/// from 0xb00000, the speed eased to -2; at the end its colours as made, its flag (drawn faded)
/// off; phase 3 on: nothing but the move.
pub fn swooper_behaviour(cpu: &mut Cpu, o: u32) {
    if nested!(cpu, 0x0608_0970, left_screen(cpu, o)) != 0 {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    if cpu.read32(o + LIFE) as i32 <= 0 {
        nested!(cpu, 0x0608_0880, explosion(cpu, o + FLAGS));
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    match cpu.read8(o + PHASE) {
        0 => {
            drone_colour(cpu, o, FADED_COLOURS);
            cpu.write32(o + SPEED, S_FIRST_SPEED);
            cpu.write32(o + S_TURN_FROM, 0);
            cpu.write32(o + S_TURN_TO, HALF_TURN);
            nested!(cpu, 0x0606_2e40, math::unwrap_angles(cpu, o + S_TURN_FROM, o + S_TURN_TO));
            let x = cpu.read32(o + POSITION);
            cpu.write32(o + S_FROM_X, x);
            cpu.write32(o + PHASE_TIME, 0);
            next_phase(cpu, o);
            swooper_swoop(cpu, o);
        }
        1 => swooper_swoop(cpu, o),
        2 => {
            let t = phase_time_step(cpu, o, STEP);
            let (a, b) = (cpu.read32(o + S_TO_X), cpu.read32(o + S_END_X));
            let x = nested!(cpu, 0x0606_2ebc, math::ease(cpu, a, b, t));
            cpu.write32(o + POSITION, x);
            let z = nested!(cpu, 0x0606_2ebc, math::ease(cpu, S_ARC_Z, 0, t));
            cpu.write32(o + POSITION + 8, z);
            let speed = nested!(cpu, 0x0606_2ebc, math::ease(cpu, 0, S_BACK_SPEED, t));
            cpu.write32(o + SPEED, speed);
            if cpu.read32(o + PHASE_TIME) == ONE {
                arrived_colour(cpu, o);
                let flags = cpu.read32(o + FLAGS) & !FADED;
                cpu.write32(o + FLAGS, flags);
                next_phase(cpu, o);
            }
        }
        _ => {}
    }
    drone_move(cpu, o);
}

/// A swooper made from `desc` (a long: how far out it ends; a word at +6: its type; a long at +8:
/// the x it sets out from): a drone (numbered from 2, list 2, 0xb4 bytes, numbered_exec's) at that
/// x, 7 dots down, its kind and life set, its flag (drawn faded); it swoops to 1.5 times the
/// reverse of that far, and ends at the reverse.
const S_START_Y: u32 = 0x70_0000;
const S_SWOOP_SCALE: u32 = 0x1_8000;
const SWOOPER: u32 = 0x0608_09c0;
const DESC_FAR: u32 = 0;
const DESC_TYPE: u32 = 6;
const DESC_X: u32 = 8;

pub fn swooper_set(cpu: &mut Cpu, desc: u32) {
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, DRONE_NUMBERS)) as i8;
    if n == -1 {
        return;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, DRONE_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, NUMBERED_EXEC));
    if o == 0 {
        return;
    }
    let spec = cpu.stack();
    cpu.write8(spec + SPEC_NUMBER, n as u8);
    cpu.write32(spec + SPEC_BEHAVIOUR, SWOOPER);
    let ty = cpu.read16(desc + DESC_TYPE);
    cpu.write16(spec + SPEC_TYPE, ty);
    let x = cpu.read32(desc + DESC_X);
    cpu.write32(spec + SPEC_POSITION, x);
    cpu.write32(spec + SPEC_POSITION + 4, S_START_Y);
    cpu.write32(spec + SPEC_POSITION + 8, 0);
    cpu.write16(spec + SPEC_TURN, 0);
    nested!(cpu, 0x0608_079c, drone_setup(cpu, o, spec));
    nested!(cpu, 0x0608_0c3c, drone_defaults(cpu, o));
    let flags = cpu.read32(o + FLAGS) | FADED;
    cpu.write32(o + FLAGS, flags);
    let far = cpu.read32(desc + DESC_FAR);
    let to = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, far.wrapping_neg(), S_SWOOP_SCALE));
    cpu.write32(o + S_TO_X, to);
    cpu.write32(o + S_END_X, far.wrapping_neg());
}

// ---- the swoopers' launcher ----------------------------------------------------------------------

/// The launcher (a plain object): its mode (+3: 0 setting up, 1 going, 112 done, 115 freed), its
/// frames (+0x2c, a long), its volleys' kinds and counts (+0x11/+0x12 for the one side, +0x13/+0x14
/// for the other: the kind -1 to 1 and how many of it so far, to 6), its angle (+0x1a, a word).
const COUNT: u32 = 0x2c;
const ANGLE: u32 = 0x1a;
const SIDES: [(u32, u32); 2] = [(0x11, 0x12), (0x13, 0x14)];
const VOLLEY_REACH: u32 = 0xa0_0000;
const VOLLEY_TURN: u16 = 0xaaa;
const VOLLEY_FRAMES: u32 = 0x240;
/// The launcher made (list 0, 76 bytes).
const LAUNCHER_SIZE: u32 = 76;

/// The launcher's volley, every 8th frame: a swooper at each side of its angle's cosine times 10
/// dots out (the second side's mirrored), each of its side's kind (0 for the third to the sixth of
/// its kind 1's steps), its count up (at 6 back to 0, its kind 2 on, past 2 back 3); its angle
/// turned 0xaaa on; its frames up one.
pub fn swooper_volley(cpu: &mut Cpu, o: u32) {
    if cpu.read32(o + COUNT) & 7 == 0 {
        let s = cpu.stack();
        let angle = cpu.read16(o + ANGLE) as i16 as i32 as u32;
        let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, angle));
        let reach = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, cos, VOLLEY_REACH));
        cpu.write32(s, reach);
        for (n, (kind_at, count_at)) in SIDES.into_iter().enumerate() {
            let kind = cpu.read8(o + kind_at) as i8 as i32 as u32;
            let count = cpu.read8(o + count_at) as i8;
            let ty = if kind == 1 && count > 2 { 0 } else { kind };
            cpu.write32(s + 4, ty);
            let x = cpu.read32(s);
            cpu.write32(s + 8, x);
            nested!(cpu, 0x0608_0c7c, swooper_set(cpu, s));
            let count = cpu.read8(o + count_at).wrapping_add(1);
            cpu.write8(o + count_at, count);
            if count as i8 == 6 {
                cpu.write8(o + count_at, 0);
                let kind = cpu.read8(o + kind_at).wrapping_add(2);
                cpu.write8(o + kind_at, kind);
                if kind as i8 > 2 {
                    cpu.write8(o + kind_at, kind.wrapping_sub(3));
                }
            }
            if n == 0 {
                let x = cpu.read32(s).wrapping_neg();
                cpu.write32(s, x);
            }
        }
        let angle = cpu.read16(o + ANGLE).wrapping_add(VOLLEY_TURN);
        cpu.write16(o + ANGLE, angle);
    }
    let count = cpu.read32(o + COUNT).wrapping_add(1);
    cpu.write32(o + COUNT, count);
}

/// The swoopers' launcher's behaviour (unless the game is held). Mode 0: its count 0, its angle
/// 0x400, its volleys' kinds 0 and 1 and counts 3 and 0; mode 1: a volley, done (112) at 576
/// frames; 112: 115; 115: freed.
pub fn swooper_launcher(cpu: &mut Cpu, o: u32) {
    if cpu.read32(GAME_FLAGS) & HELD != 0 {
        return;
    }
    match cpu.read8(o + OBJ_MODE) as i8 {
        0 => {
            cpu.write32(o + COUNT, 0);
            cpu.write16(o + ANGLE, 0x400);
            cpu.write8(o + SIDES[0].0, 0);
            cpu.write8(o + SIDES[0].1, 3);
            cpu.write8(o + SIDES[1].0, 1);
            cpu.write8(o + SIDES[1].1, 0);
            cpu.write8(o + OBJ_MODE, 1);
        }
        1 => {}
        112 => {
            cpu.write8(o + OBJ_MODE, FREED);
            return;
        }
        115 => {
            nested!(cpu, 0x0600_579c, objects::obj_free(cpu, o));
            return;
        }
        _ => return,
    }
    nested!(cpu, 0x0608_0d50, swooper_volley(cpu, o));
    if cpu.read32(o + COUNT) == VOLLEY_FRAMES {
        cpu.write8(o + OBJ_MODE, TO_STOP);
    }
}

/// The swoopers' launcher made.
pub fn swooper_launcher_set(cpu: &mut Cpu) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, LAUNCHER_SIZE)) as i8 as i32 as u32;
    nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 0, blocks, 0x0608_0e6c))
}

// ---- the gunners ---------------------------------------------------------------------------------

/// A gunner's (the second drones') own fields: the turn it starts at and the one it comes to
/// (+0x94, +0x98: the second given by its maker, its side's), the frames to its shot (+0x9c), the
/// x it sets out from and the x it comes to (+0xa0, +0xa4).
const G_TURN_FROM: u32 = 0x94;
const G_TURN_TO: u32 = 0x98;
const G_SHOT_WAIT: u32 = 0x9c;
const G_X_FROM: u32 = 0xa0;
const G_X_TO: u32 = 0xa4;
/// The stage's drone (the object whose phase, once set, sends the gunners away), and its phase
/// (the volleys' timings).
const STAGE_DRONE: u32 = 0x0608_9edc;
const STAGE_PHASE: u32 = 0x0608_9ed8;
/// Where a gunner comes to by its side's turn (below 0 or above).
const G_X_FAR: [u32; 2] = [0x30_0000, 0xffd0_0000];
const G_ARRIVE_SPEED: u32 = 0x2_0000;
const G_START_SPEED: u32 = 0x8000;
const G_SHOT_TURN_STEP: u32 = 0x180;
const GUNNER: u32 = 0x0608_1284;

/// A gunner's first phase: its colours the faded ones, its x kept (to come to 0x300000, or on the
/// side its turn is above 0, -0x300000), its turn kept and unwrapped against its side's, its
/// speed 0x8000, its time 0; on.
fn gunner_start(cpu: &mut Cpu, o: u32) {
    drone_colour(cpu, o, FADED_COLOURS);
    let x = cpu.read32(o + POSITION);
    cpu.write32(o + G_X_FROM, x);
    let far = G_X_FAR[(cpu.read32(o + G_TURN_TO) as i32 > 0) as usize];
    cpu.write32(o + G_X_TO, far);
    let turn = cpu.read16(o + TURN) as i16 as i32 as u32;
    cpu.write32(o + G_TURN_FROM, turn);
    nested!(cpu, 0x0606_2e40, math::unwrap_angles(cpu, o + G_TURN_FROM, o + G_TURN_TO));
    cpu.write32(o + PHASE_TIME, 0);
    cpu.write32(o + SPEED, G_START_SPEED);
    next_phase(cpu, o);
}

/// A gunner's second phase (its first goes on into it): its time through it counted up a frame; at
/// 576 its colours as made, its time 0, on.
fn gunner_wait(cpu: &mut Cpu, o: u32) {
    let t = cpu.read32(o + PHASE_TIME).wrapping_add(1);
    cpu.write32(o + PHASE_TIME, t);
    if t == VOLLEY_FRAMES {
        arrived_colour(cpu, o);
        cpu.write32(o + PHASE_TIME, 0);
        next_phase(cpu, o);
    }
}

/// A gunner's behaviour. It ends in an explosion when the stage's drone has its phase set, or when
/// its life is gone, and (112) off screen. Phases 0 and 1: see above; phase 2, the time through it
/// a 64th a frame: its turn eased to its side's, x eased, z back from 0xb00000, the speed from 1 to
/// 0; at the end its heading its turn, its speed 2, its turns re-made (its turn, a half turn), its
/// flag (drawn faded) off; phase 3: its shot's wait counted down (the shot at 0), the turn eased
/// 0x180 a frame; and its move.
pub fn gunner_behaviour(cpu: &mut Cpu, o: u32) {
    let stage = cpu.read32(STAGE_DRONE);
    if cpu.read32(stage + PHASE) != 0 {
        nested!(cpu, 0x0608_0880, explosion(cpu, o + FLAGS));
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    if nested!(cpu, 0x0608_0f44, out_of_view(cpu, o)) != 0 {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    if cpu.read32(o + LIFE) as i32 <= 0 {
        nested!(cpu, 0x0608_0880, explosion(cpu, o + FLAGS));
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    match cpu.read8(o + PHASE) {
        0 => {
            gunner_start(cpu, o);
            gunner_wait(cpu, o);
        }
        1 => gunner_wait(cpu, o),
        2 => {
            let t = phase_time_step(cpu, o, STEP);
            let (a, b) = (cpu.read32(o + G_TURN_FROM), cpu.read32(o + G_TURN_TO));
            let turn = nested!(cpu, 0x0606_2ebc, math::ease(cpu, a, b, t));
            cpu.write16(o + TURN, turn as u16);
            let (a, b) = (cpu.read32(o + G_X_FROM), cpu.read32(o + G_X_TO));
            let x = nested!(cpu, 0x0606_2ebc, math::ease(cpu, a, b, t));
            cpu.write32(o + POSITION, x);
            let z = nested!(cpu, 0x0606_2ebc, math::ease(cpu, S_ARC_Z, 0, t));
            cpu.write32(o + POSITION + 8, z);
            let speed = nested!(cpu, 0x0606_2ebc, math::ease(cpu, ONE, 0, t));
            cpu.write32(o + SPEED, speed);
            if cpu.read32(o + PHASE_TIME) == ONE {
                cpu.write32(o + PHASE_TIME, 0);
                let turn = cpu.read16(o + TURN);
                cpu.write16(o + HEADING, turn);
                cpu.write32(o + SPEED, G_ARRIVE_SPEED);
                cpu.write32(o + G_TURN_FROM, turn as i16 as i32 as u32);
                cpu.write32(o + G_TURN_TO, HALF_TURN);
                nested!(cpu, 0x0606_2e40, math::unwrap_angles(cpu, o + G_TURN_FROM, o + G_TURN_TO));
                let flags = cpu.read32(o + FLAGS) & !FADED;
                cpu.write32(o + FLAGS, flags);
                next_phase(cpu, o);
            }
        }
        3 => {
            let wait = cpu.read32(o + G_SHOT_WAIT);
            if wait != 0 {
                cpu.write32(o + G_SHOT_WAIT, wait - 1);
                if wait == 1 {
                    nested!(cpu, 0x0608_0928, gunner_shoot(cpu, o));
                }
            }
            let t = phase_time_step(cpu, o, G_SHOT_TURN_STEP);
            let (a, b) = (cpu.read32(o + G_TURN_FROM), cpu.read32(o + G_TURN_TO));
            let turn = nested!(cpu, 0x0606_2ebc, math::ease(cpu, a, b, t));
            cpu.write16(o + HEADING, turn as u16);
            cpu.write16(o + TURN, turn as u16);
        }
        _ => {}
    }
    drone_move(cpu, o);
}

/// A gunner made from `desc` (a long: the x it sets out from; a word at +6: its type; a long at +8:
/// the frames to its shot; a long at +12: its side's turn): a drone like the swooper, at that x,
/// 14 dots down and 11 deep, its flag (drawn faded).
const G_START_Y: u32 = 0xe0_0000;
const G_START_Z: u32 = 0xb0_0000;
const GDESC_X: u32 = 0;
const GDESC_TYPE: u32 = 6;
const GDESC_WAIT: u32 = 8;
const GDESC_TURN: u32 = 12;

pub fn gunner_set(cpu: &mut Cpu, desc: u32) {
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, DRONE_NUMBERS)) as i8;
    if n == -1 {
        return;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, DRONE_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, NUMBERED_EXEC));
    if o == 0 {
        return;
    }
    let spec = cpu.stack();
    cpu.write8(spec + SPEC_NUMBER, n as u8);
    cpu.write32(spec + SPEC_BEHAVIOUR, GUNNER);
    let ty = cpu.read16(desc + GDESC_TYPE);
    cpu.write16(spec + SPEC_TYPE, ty);
    let x = cpu.read32(desc + GDESC_X);
    cpu.write32(spec + SPEC_POSITION, x);
    cpu.write32(spec + SPEC_POSITION + 4, G_START_Y);
    cpu.write32(spec + SPEC_POSITION + 8, G_START_Z);
    cpu.write16(spec + SPEC_TURN, 0);
    nested!(cpu, 0x0608_079c, drone_setup(cpu, o, spec));
    nested!(cpu, 0x0608_0c3c, drone_defaults(cpu, o));
    let flags = cpu.read32(o + FLAGS) | FADED;
    cpu.write32(o + FLAGS, flags);
    let wait = cpu.read32(desc + GDESC_WAIT);
    cpu.write32(o + G_SHOT_WAIT, wait);
    let turn = cpu.read32(desc + GDESC_TURN);
    cpu.write32(o + G_TURN_TO, turn);
}

// ---- the gunners' launcher -----------------------------------------------------------------------

/// The gunners' launcher (a plain object): its mode (+3), its kind (+0x11, a byte, -1 to 1), its
/// frames (+0x2c), its case (+0x30, 0-3) and its place in the case (+0x34, 0-2), the frames it
/// lasts (+0x38).
const L_KIND: u32 = 0x11;
const L_CASE: u32 = 0x30;
const L_PLACE: u32 = 0x34;
const L_FRAMES: u32 = 0x38;
/// Where a gunner of each case and place comes from (x), its side's turn (by the case), and the
/// frames to its shot (by the place, and by the stage's phase being above 2 or not).
const L_X: [[u32; 3]; 4] = [
    [0x00b8_0000, 0x00e8_0000, 0x0118_0000],
    [0xff48_0000, 0xff18_0000, 0xfee8_0000],
    [0xff88_0000, 0xffb8_0000, 0xffe8_0000],
    [0x0078_0000, 0x0048_0000, 0x0018_0000],
];
const L_TURN: [u32; 4] = [0x3c00, 0xffff_c400, 0x3c00, 0xffff_c400];
const L_WAIT: [[u32; 2]; 3] = [[60, 150], [75, 110], [90, 60]];

/// The launcher's volley, every 16th frame in the first three of each 64: a gunner (a description
/// on the stack: x by its case and place, its kind, the wait to its shot by its place and the
/// stage's phase, its side's turn by its case); its place on, at 3 back to 0 with its kind one on
/// (past 2 back 3) and its case on (at 4 back to 0); its frames up one.
pub fn gunner_volley(cpu: &mut Cpu, o: u32) {
    let n = cpu.read32(o + COUNT);
    if n & 15 == 0 && n & 0x30 <= 47 {
        let s = cpu.stack();
        let kind = cpu.read8(o + L_KIND) as i8 as i32 as u32;
        cpu.write32(s + 4, kind);
        let case = cpu.read32(o + L_CASE);
        let place = cpu.read32(o + L_PLACE) as i32;
        if case <= 3 {
            cpu.write32(s + 12, L_TURN[case as usize]);
            if (0..3).contains(&place) {
                cpu.write32(s, L_X[case as usize][place as usize]);
            }
        }
        if (0..3).contains(&place) {
            let high = cpu.read32(STAGE_PHASE) as i32 > 2;
            cpu.write32(s + 8, L_WAIT[place as usize][high as usize]);
        }
        nested!(cpu, 0x0608_1570, gunner_set(cpu, s));
        let place = cpu.read32(o + L_PLACE).wrapping_add(1);
        cpu.write32(o + L_PLACE, place);
        if place == 3 {
            cpu.write32(o + L_PLACE, 0);
            let kind = cpu.read8(o + L_KIND).wrapping_add(1);
            cpu.write8(o + L_KIND, kind);
            if kind as i8 > 2 {
                cpu.write8(o + L_KIND, kind.wrapping_sub(3));
            }
            let case = cpu.read32(o + L_CASE).wrapping_add(1);
            cpu.write32(o + L_CASE, case);
            if case == 4 {
                cpu.write32(o + L_CASE, 0);
            }
        }
    }
    let count = cpu.read32(o + COUNT).wrapping_add(1);
    cpu.write32(o + COUNT, count);
}

/// The gunners' launcher's behaviour (unless the game is held). Mode 0: its kind, count, case and
/// place 0; mode 1: unless the stage's drone has its phase set, a volley, until its count reaches
/// its frames; then done (112); 112: 115; 115: freed.
pub fn gunner_launcher(cpu: &mut Cpu, o: u32) {
    if cpu.read32(GAME_FLAGS) & HELD != 0 {
        return;
    }
    match cpu.read8(o + OBJ_MODE) as i8 {
        0 => {
            cpu.write8(o + L_KIND, 0);
            cpu.write32(o + COUNT, 0);
            cpu.write32(o + L_CASE, 0);
            cpu.write32(o + L_PLACE, 0);
            cpu.write8(o + OBJ_MODE, 1);
        }
        1 => {}
        112 => {
            cpu.write8(o + OBJ_MODE, FREED);
            return;
        }
        115 => {
            nested!(cpu, 0x0600_579c, objects::obj_free(cpu, o));
            return;
        }
        _ => return,
    }
    let stage = cpu.read32(STAGE_DRONE);
    if cpu.read32(stage + PHASE) == 0 {
        nested!(cpu, 0x0608_1634, gunner_volley(cpu, o));
        if cpu.read32(o + COUNT) != cpu.read32(o + L_FRAMES) {
            return;
        }
    }
    cpu.write8(o + OBJ_MODE, TO_STOP);
}

/// The gunners' launcher made, to last the frames at `frames`.
pub fn gunner_launcher_set(cpu: &mut Cpu, frames: u32) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, LAUNCHER_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 0, blocks, 0x0608_1838));
    if o != 0 {
        let n = cpu.read32(frames);
        cpu.write32(o + L_FRAMES, n);
    }
    o
}

// ---- the chargers --------------------------------------------------------------------------------

/// A charger's (the third drones') own fields: its frames' delay and the count down of it (+0x70,
/// +0x71, bytes), the turn it starts and ends at (+0x94, +0x98), the x it starts and ends at (+0xa0,
/// +0xa4: the second given by its maker).
const C_DELAY: u32 = 0x70;
const C_COUNTDOWN: u32 = 0x71;
const C_TURN_FROM: u32 = 0x94;
const C_TURN_TO: u32 = 0x98;
const C_X_FROM: u32 = 0xa0;
const C_X_TO: u32 = 0xa4;
/// The kind a charger arrives as (its colours, its frames), its frames from the arrival's, its
/// life, its first speed, and the steps of its phases: 0x400 a frame through its approach, 0x1000
/// through its turn, 16 frames through its rest; the speed it gains a frame once it charges.
const CHARGER: u32 = 0x0608_1914;
const C_KIND: u32 = 14;
const C_FRAME: u8 = 3;
const C_LIFE: u32 = 500;
const C_FRAMES: u8 = 9;
const C_FIRST_SPEED: u32 = 0xa_0000;
const C_TURN_STEP: u32 = 0x1000;
const C_REST: u32 = 16;
const C_CHARGE: u32 = 0x2000;
const C_SOUND: u32 = 76;
/// The player's code that takes a drone's number off (called when the stage's phase is past 2).
const C_UNKNOWN: u32 = 0x0605_6070;

/// A charger's frame moved on a step (round 9).
fn charger_frame(cpu: &mut Cpu, o: u32) {
    let frame = cpu.read8(o + FRAME).wrapping_add(1);
    cpu.write8(o + FRAME, frame);
    if frame == C_FRAMES {
        cpu.write8(o + FRAME, 0);
    }
}

/// A charger's approach (its first phase goes on into it): its frame a step on every other game
/// frame; its time through it a 64th a frame; its x eased from where it set out to its end, its
/// speed from 0xa0000 to none, its turn the angle (a quarter turn back) from its x's move and its
/// speed along its heading's cosine, its z eased down from 0xb00000. At the end its colours as
/// made (kind 14's), its flag (drawn faded) off, its frame 3, its time 0, its turns re-made (its
/// turn, then the angle to the player), its heading its turn; on.
fn charger_approach(cpu: &mut Cpu, o: u32) {
    if cpu.read32(GAME_FRAMES) & 1 != 0 {
        charger_frame(cpu, o);
    }
    let t = cpu.read32(o + PHASE_TIME).wrapping_add(STEP);
    cpu.write32(o + PHASE_TIME, t);
    let was = cpu.read32(o + POSITION);
    let (a, b) = (cpu.read32(o + C_X_FROM), cpu.read32(o + C_X_TO));
    let x = nested!(cpu, 0x0606_2ebc, math::ease(cpu, a, b, t));
    cpu.write32(o + POSITION, x);
    let t = cpu.read32(o + PHASE_TIME);
    let speed = nested!(cpu, 0x0606_2ebc, math::ease(cpu, C_FIRST_SPEED, 0, t));
    cpu.write32(o + SPEED, speed);
    let heading = cpu.read16(o + HEADING) as i16 as i32 as u32;
    let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, heading));
    let speed = cpu.read32(o + SPEED);
    let ahead = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, speed, cos));
    let angle = nested!(cpu, 0x0606_780c, sgl::sl_atan(cpu, was.wrapping_sub(x), ahead)) as u16;
    cpu.write16(o + TURN, angle.wrapping_add(0xc000));
    let t = cpu.read32(o + PHASE_TIME);
    let z = nested!(cpu, 0x0606_2ebc, math::ease(cpu, S_ARC_Z, 0, t));
    cpu.write32(o + POSITION + 8, z);
    if cpu.read32(o + PHASE_TIME) != ONE {
        return;
    }
    drone_colour(cpu, o, CHAR_BASE + 16 * C_KIND);
    let flags = cpu.read32(o + FLAGS) & !FADED;
    cpu.write32(o + FLAGS, flags);
    cpu.write8(o + FRAME, C_FRAME);
    cpu.write32(o + PHASE_TIME, 0);
    let turn = cpu.read16(o + TURN);
    cpu.write32(o + C_TURN_FROM, turn as i16 as i32 as u32);
    cpu.write16(o + HEADING, turn);
    let angle = nested!(cpu, 0x0608_08e0, aim_angle(cpu, o, o + POSITION, 0));
    cpu.write32(o + C_TURN_TO, angle as i16 as i32 as u32);
    nested!(cpu, 0x0606_2e40, math::unwrap_angles(cpu, o + C_TURN_FROM, o + C_TURN_TO));
    next_phase(cpu, o);
}

/// A charger's behaviour. From its fourth phase on it ends (112) off the screen; it ends in an
/// explosion when its life is gone (taking its number off first when the stage is past its phase
/// 2). Phase 0: its colours the faded ones (the second set), its frames' delay 4 (and count), its
/// speed 0xa0000, its x kept, its turns its turn and a half turn on; on into phase 1: the
/// approach (above); phase 2, the time through it 1/16 a frame: its heading and turn eased to
/// the angle to the player, then phase 3: 16 frames, at the end a sound and its jet; phase 4: its
/// frame a step every other frame once its count (down from its delay) is out, the delay
/// shortened to 1, its speed up 0x2000 a frame. Then its move.
pub fn charger_behaviour(cpu: &mut Cpu, o: u32) {
    if cpu.read8(o + PHASE) as i8 > 3 && nested!(cpu, 0x0605_6a24, aim::off_screen(cpu, o + FLAGS)) as u8 != 0 {
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    if cpu.read32(o + LIFE) as i32 <= 0 {
        if cpu.read32(STAGE_PHASE) as i32 > 2 {
            cpu.call(C_UNKNOWN, &[o, 0]);
        }
        nested!(cpu, 0x0608_0880, explosion(cpu, o + FLAGS));
        cpu.write8(o + OBJ_MODE, TO_STOP);
        return;
    }
    match cpu.read8(o + PHASE) {
        0 => {
            drone_colour(cpu, o, FADED_COLOURS_2);
            cpu.write8(o + C_COUNTDOWN, 4);
            cpu.write8(o + C_DELAY, 4);
            cpu.write32(o + SPEED, C_FIRST_SPEED);
            cpu.write32(o + PHASE_TIME, 0);
            let x = cpu.read32(o + POSITION);
            cpu.write32(o + C_X_FROM, x);
            let turn = cpu.read16(o + TURN) as i16 as i32 as u32;
            cpu.write32(o + C_TURN_FROM, turn);
            cpu.write32(o + C_TURN_TO, turn.wrapping_add(HALF_TURN));
            nested!(cpu, 0x0606_2e40, math::unwrap_angles(cpu, o + C_TURN_FROM, o + C_TURN_TO));
            next_phase(cpu, o);
            charger_approach(cpu, o);
        }
        1 => charger_approach(cpu, o),
        2 => {
            let t = phase_time_step(cpu, o, C_TURN_STEP);
            let (a, b) = (cpu.read32(o + C_TURN_FROM), cpu.read32(o + C_TURN_TO));
            let turn = nested!(cpu, 0x0606_2ebc, math::ease(cpu, a, b, t));
            cpu.write16(o + HEADING, turn as u16);
            cpu.write16(o + TURN, turn as u16);
            if cpu.read32(o + PHASE_TIME) == ONE {
                cpu.write32(o + PHASE_TIME, 0);
                next_phase(cpu, o);
            }
        }
        3 => {
            let t = cpu.read32(o + PHASE_TIME).wrapping_add(1);
            cpu.write32(o + PHASE_TIME, t);
            if t == C_REST {
                nested!(cpu, 0x0600_50d4, sound::se_set(cpu, C_SOUND));
                nested!(cpu, 0x0606_23d4, player::jet_set(cpu, o, u32::MAX));
                cpu.write32(o + PHASE_TIME, 0);
                next_phase(cpu, o);
            }
        }
        4 => {
            if cpu.read32(GAME_FRAMES) & 1 != 0 {
                let count = cpu.read8(o + C_COUNTDOWN).wrapping_sub(1);
                cpu.write8(o + C_COUNTDOWN, count);
                if count as i8 == 0 {
                    charger_frame(cpu, o);
                    let delay = cpu.read8(o + C_DELAY);
                    if delay as i8 > 1 {
                        cpu.write8(o + C_DELAY, delay.wrapping_sub(1));
                    }
                    let delay = cpu.read8(o + C_DELAY);
                    cpu.write8(o + C_COUNTDOWN, delay);
                }
            }
            let speed = cpu.read32(o + SPEED).wrapping_add(C_CHARGE);
            cpu.write32(o + SPEED, speed);
        }
        _ => {}
    }
    drone_shift(cpu, o);
}

/// A charger's last set-up: its frame 3, its kind 14, its life 500, its colours.
pub fn charger_defaults(cpu: &mut Cpu, o: u32) {
    cpu.write8(o + FRAME, C_FRAME);
    cpu.write8(o + KIND, C_KIND as u8);
    cpu.write32(o + LIFE, C_LIFE);
    drone_colour(cpu, o, CHAR_BASE + 16 * C_KIND);
}

/// A charger made from `desc` (a long: where it comes from, 0-5 (right, left or middle, above or
/// below), a word at +6: its type, a long at +8: the x it comes to): a drone at that place (above:
/// 0xec dots up, a half turn; below: 0xec dots down, no turn), 11 deep, its flag (drawn faded).
const CDESC_CASE: u32 = 0;
const CDESC_TYPE: u32 = 6;
const CDESC_X_TO: u32 = 8;
const C_PLACES: [(u32, u32, u16); 6] = [
    (0x0100_0000, 0xff14_0000, 0x8000),
    (0xff00_0000, 0xff14_0000, 0x8000),
    (0, 0xff14_0000, 0x8000),
    (0x0100_0000, 0x00ec_0000, 0),
    (0xff00_0000, 0x00ec_0000, 0),
    (0, 0x00ec_0000, 0),
];

pub fn charger_set(cpu: &mut Cpu, desc: u32) {
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, DRONE_NUMBERS)) as i8;
    if n == -1 {
        return;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, DRONE_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, NUMBERED_EXEC));
    if o == 0 {
        return;
    }
    let spec = cpu.stack();
    cpu.write8(spec + SPEC_NUMBER, n as u8);
    cpu.write32(spec + SPEC_BEHAVIOUR, CHARGER);
    let ty = cpu.read16(desc + CDESC_TYPE);
    cpu.write16(spec + SPEC_TYPE, ty);
    let case = cpu.read32(desc + CDESC_CASE);
    if let Some(&(x, y, turn)) = C_PLACES.get(case as usize) {
        cpu.write32(spec + SPEC_POSITION, x);
        cpu.write32(spec + SPEC_POSITION + 4, y);
        cpu.write16(spec + SPEC_TURN, turn);
    }
    cpu.write32(spec + SPEC_POSITION + 8, G_START_Z);
    nested!(cpu, 0x0608_079c, drone_setup(cpu, o, spec));
    nested!(cpu, 0x0608_1c7c, charger_defaults(cpu, o));
    let flags = cpu.read32(o + FLAGS) | FADED;
    cpu.write32(o + FLAGS, flags);
    let x = cpu.read32(desc + CDESC_X_TO);
    cpu.write32(o + C_X_TO, x);
}

// ---- the chargers' launcher ----------------------------------------------------------------------

/// Where the launcher's chargers come from by its case (0-5, the case's order 0, 1, 2 then again;
/// 3-5 the above's) and the x they come to (by their place of 3): a mark each (36 dots apart).
const CL_STEP: u32 = 36 << 16;
const CL_FIRST: u32 = 0xff78_0000;
const CL_LAST: u32 = 0x0088_0000;
const CL_MIDDLE: [u32; 3] = [0, 0xffdc_0000, 0x0024_0000];

/// The launcher's volley, every 16th frame in the first three of each 64 (the same launcher as the
/// gunners'): a charger (a description on the stack: its place by its case, its kind, the x it comes
/// to by its case and place); its place on, at 3 back to 0 with its kind one on (past 2 back 3) and
/// its case on; its frames up one.
pub fn charger_volley(cpu: &mut Cpu, o: u32) {
    let n = cpu.read32(o + COUNT);
    if n & 15 == 0 && n & 0x30 <= 47 {
        let s = cpu.stack();
        let kind = cpu.read8(o + L_KIND) as i8 as i32 as u32;
        cpu.write32(s + 4, kind);
        let case = cpu.read32(o + L_CASE);
        let place = cpu.read32(o + L_PLACE);
        if case <= 5 {
            match case % 3 {
                0 => {
                    cpu.write32(s, 0);
                    cpu.write32(s + 8, place.wrapping_mul(CL_STEP).wrapping_add(CL_FIRST));
                }
                1 => {
                    cpu.write32(s, 1);
                    cpu.write32(s + 8, CL_LAST.wrapping_sub(place.wrapping_mul(CL_STEP)));
                }
                _ => match place as i32 {
                    1 => {
                        cpu.write32(s, 0);
                        cpu.write32(s + 8, CL_MIDDLE[1]);
                    }
                    2 => {
                        cpu.write32(s, 1);
                        cpu.write32(s + 8, CL_MIDDLE[2]);
                    }
                    0 => {
                        cpu.write32(s, 2);
                        cpu.write32(s + 8, CL_MIDDLE[0]);
                    }
                    _ => {}
                },
            }
        }
        if cpu.read32(o + L_CASE) as i32 > 2 {
            let at = cpu.read32(s).wrapping_add(3);
            cpu.write32(s, at);
        }
        nested!(cpu, 0x0608_1cc0, charger_set(cpu, s));
        let place = cpu.read32(o + L_PLACE).wrapping_add(1);
        cpu.write32(o + L_PLACE, place);
        if place == 3 {
            cpu.write32(o + L_PLACE, 0);
            let kind = cpu.read8(o + L_KIND).wrapping_add(1);
            cpu.write8(o + L_KIND, kind);
            if kind as i8 > 2 {
                cpu.write8(o + L_KIND, kind.wrapping_sub(3));
            }
            let case = cpu.read32(o + L_CASE).wrapping_add(1);
            cpu.write32(o + L_CASE, case);
        }
    }
    let count = cpu.read32(o + COUNT).wrapping_add(1);
    cpu.write32(o + COUNT, count);
}

/// The chargers' launcher's behaviour (unless the game is held). Mode 0: its kind, count, case and
/// place 0; mode 1: a volley, done (112) when its count reaches its frames; 112: 115; 115: freed.
pub fn charger_launcher(cpu: &mut Cpu, o: u32) {
    if cpu.read32(GAME_FLAGS) & HELD != 0 {
        return;
    }
    match cpu.read8(o + OBJ_MODE) as i8 {
        0 => {
            cpu.write8(o + L_KIND, 0);
            cpu.write32(o + COUNT, 0);
            cpu.write32(o + L_CASE, 0);
            cpu.write32(o + L_PLACE, 0);
            cpu.write8(o + OBJ_MODE, 1);
        }
        1 => {}
        112 => {
            cpu.write8(o + OBJ_MODE, FREED);
            return;
        }
        115 => {
            nested!(cpu, 0x0600_579c, objects::obj_free(cpu, o));
            return;
        }
        _ => return,
    }
    nested!(cpu, 0x0608_1dec, charger_volley(cpu, o));
    if cpu.read32(o + COUNT) == cpu.read32(o + L_FRAMES) {
        cpu.write8(o + OBJ_MODE, TO_STOP);
    }
}

/// The chargers' launcher made, to last the frames at `frames`.
pub fn charger_launcher_set(cpu: &mut Cpu, frames: u32) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, LAUNCHER_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 0, blocks, 0x0608_1f00));
    if o != 0 {
        let n = cpu.read32(frames);
        cpu.write32(o + L_FRAMES, n);
    }
    o
}

// ---- the stage's timeline, camera and scenery ----------------------------------------------------

/// A timeline of 24-byte entries (the frame it acts at, a function and the description it is given
/// at +8); `at` holds the entry's address, `frames` the frames so far. The entries due now acted
/// on, the frames up one: 1 at the end of the timeline (a frame of -1), else 0.
pub fn timeline_step(cpu: &mut Cpu, at: u32, frames: u32) -> u32 {
    let e = cpu.read32(at);
    let due = cpu.read32(e);
    if due as i32 == -1 {
        return 1;
    }
    if due == cpu.read32(frames) {
        loop {
            let e = cpu.read32(at);
            let f = cpu.read32(e + 4);
            cpu.call_returning(0x0608_1ff8, f, &[e + 8]);
            let e = cpu.read32(at);
            cpu.write32(at, e + 24);
            let next = cpu.read32(e + 24);
            if next != cpu.read32(frames) {
                break;
            }
        }
    }
    let n = cpu.read32(frames).wrapping_add(1);
    cpu.write32(frames, n);
    0
}

/// The camera (a struct): its place (x, y, z at +4), its turns about x and y (words at +0x12 and
/// +0x16), its roll (+0x18, a long) and its spin (+0x1a, a word).
const CAMERA: u32 = 0x0607_4750;
const CAM_POSITION: u32 = 4;
const CAM_TURN_X: u32 = 0x12;
const CAM_TURN_Y: u32 = 0x16;
const CAM_ROLL: u32 = 0x18;
const CAM_SPIN: u32 = 0x1a;
/// Where the stage keeps the camera's matrix once made, and the one it draws its scenery by.
const CAMERA_MATRIX: u32 = 0x0608_9ea0;
const SCENERY_MATRIX: u32 = 0x0608_9e70;
/// The view's struct (a pointer): its matrix at +0x70, a stage counter at +0x51.
const VIEW: u32 = 0x0607_4740;
const VIEW_MATRIX: u32 = 0x70;
const VIEW_COUNTER: u32 = 0x51;

/// The camera's matrix: SGL's matrix made from the camera's turns and its place taken off, then
/// moved by (`x`, `y`, `z`), kept at 0x06089ea0.
pub fn camera_matrix(cpu: &mut Cpu, x: u32, y: u32, z: u32) -> u32 {
    nested!(cpu, 0x0603_511a, sgl::sl_push_unit_matrix(cpu));
    let a = cpu.read16(CAMERA + CAM_TURN_X) as i16 as i32 as u32;
    nested!(cpu, 0x0606_72ec, sgl::sl_rot_x(cpu, a));
    let a = cpu.read16(CAMERA + CAM_TURN_Y) as i16 as i32 as u32;
    nested!(cpu, 0x0606_736c, sgl::sl_rot_y(cpu, a));
    let [cx, cy, cz] = [0, 1, 2].map(|k| cpu.read32(CAMERA + CAM_POSITION + 4 * k));
    nested!(cpu, 0x0606_721c, sgl::sl_translate(cpu, cx.wrapping_neg(), cy.wrapping_neg(), cz.wrapping_neg()));
    nested!(cpu, 0x0606_721c, sgl::sl_translate(cpu, x, y, z));
    nested!(cpu, 0x0606_db9c, sgl::sl_get_matrix(cpu, CAMERA_MATRIX));
    nested!(cpu, 0x0606_7200, sgl::sl_pop_matrix(cpu))
}

/// The steps of a scenery object's modes (a table of functions, by the byte at +0x70).
const X_MODE: u32 = 0x70;
const X_STEPS: u32 = 0x0608_8054;

/// Runs a scenery object's step for its mode.
pub fn mode_step(cpu: &mut Cpu, o: u32) {
    let mode = cpu.read8(o + X_MODE) as i8 as i32 as u32;
    let f = cpu.read32(X_STEPS.wrapping_add(mode << 2));
    cpu.call_returning(0x0608_20b8, f, &[o]);
}

/// A scenery object's sub-step (+0x71), its counter (+0x88), its wait (+0x8c), the place of its
/// script's next entry (+0xa4), the script's byte (+0x73), and its timeline's place and frames
/// (+0xdc, +0xe0).
const X_STAGE: u32 = 0x71;
const X_COUNT: u32 = 0x88;
const X_WAIT: u32 = 0x8c;
const X_SCRIPT: u32 = 0xa4;
const X_BYTE: u32 = 0x73;
const X_TIMELINE: u32 = 0xdc;
const X_FRAMES: u32 = 0xe0;
/// The scenery's kind's model (75): its frames' table at +28 of its file, which has a table of
/// polygons for each frame (by the bit it is drawn with).
const MODEL_FRAMES: u32 = 28;
/// The scenery's draw: the sprite `s` (an object's +0x10) drawn as three polygons of the kind's
/// model, the first at the object's place (its +0x98, +0x9c, +0xa0, in the camera's matrix), the
/// others 0x60 dots either side of it and 0x18 deep; the frames' numbers at +0x6b, +0x69 and +0x6a;
/// the polygon of each the one by the sprite's bit 0 of its word +0x46 (0 for the sides when its
/// mode (+0x70) is 3).
pub fn scenery_draw(cpu: &mut Cpu, s: u32) {
    nested!(cpu, 0x0606_713c, sgl::sl_cur_window(cpu, 0));
    nested!(cpu, 0x0606_71cc, sgl::sl_push_matrix(cpu));
    let mut which = cpu.read16(s + 0x46) as u32 & 1;
    let view = cpu.read32(VIEW);
    nested!(cpu, 0x0606_db7c, sgl::sl_load_matrix(cpu, view + VIEW_MATRIX));
    let o = s - 16;
    let [x, y, z] = [0xa8, 0xac, 0xb0].map(|k| cpu.read32(o + k));
    nested!(cpu, 0x0606_721c, sgl::sl_translate(cpu, x, y, z));
    nested!(cpu, 0x0606_db9c, sgl::sl_get_matrix(cpu, SCENERY_MATRIX));
    let file = cpu.read32(KINDS + K_SIZE * SCENERY_MODEL);
    let frame = |cpu: &mut Cpu, at: u32, which: u32| {
        let n = cpu.read8(at) as i8 as i32 as u32;
        let frames = cpu.read32(file + MODEL_FRAMES);
        let polygons = cpu.read32(frames.wrapping_add(n << 2));
        cpu.read32(polygons.wrapping_add(which << 2))
    };
    let pd = frame(cpu, s + 0x6b, which);
    nested!(cpu, 0x0606_745c, sgl::sl_put_polygon(cpu, pd));
    if cpu.read8(s + 0x60) == 3 {
        which = 0;
    }
    nested!(cpu, 0x0606_71cc, sgl::sl_push_matrix(cpu));
    nested!(cpu, 0x0606_721c, sgl::sl_translate(cpu, 0x60_0000, 0, 0x18_0000));
    let pd = frame(cpu, s + 0x69, which);
    nested!(cpu, 0x0606_745c, sgl::sl_put_polygon(cpu, pd));
    nested!(cpu, 0x0606_7200, sgl::sl_pop_matrix(cpu));
    nested!(cpu, 0x0606_71cc, sgl::sl_push_matrix(cpu));
    nested!(cpu, 0x0606_721c, sgl::sl_translate(cpu, 0xffa0_0000, 0, 0x18_0000));
    let pd = frame(cpu, s + 0x6a, which);
    nested!(cpu, 0x0606_745c, sgl::sl_put_polygon(cpu, pd));
    nested!(cpu, 0x0606_7200, sgl::sl_pop_matrix(cpu));
    nested!(cpu, 0x0606_7200, sgl::sl_pop_matrix(cpu));
    nested!(cpu, 0x0606_713c, sgl::sl_cur_window(cpu, 1));
}

/// A scenery object's first step: while the view's stage counter is 3, its counter 0 and on (its
/// sub-step 1); then its counter up a frame, at 424 on to its next mode (the sub-step 0).
pub fn scenery_wait(cpu: &mut Cpu, o: u32) {
    match cpu.read8(o + X_STAGE) as i8 {
        0 => {
            let view = cpu.read32(VIEW);
            if cpu.read8(view + VIEW_COUNTER) as i8 != 3 {
                return;
            }
            cpu.write32(o + X_COUNT, 0);
            cpu.write8(o + X_STAGE, 1);
        }
        1 => {
            let n = cpu.read32(o + X_COUNT).wrapping_add(1);
            cpu.write32(o + X_COUNT, n);
            if n == 0x1a8 {
                let mode = cpu.read8(o + X_MODE).wrapping_add(1);
                cpu.write8(o + X_MODE, mode);
                cpu.write8(o + X_STAGE, 0);
            }
        }
        _ => {}
    }
}

/// A scenery object's next mode, its sub-step 0, its flag (drawn faded) set, the scroll on.
pub fn scenery_next(cpu: &mut Cpu, o: u32) {
    let mode = cpu.read8(o + X_MODE).wrapping_add(1);
    cpu.write8(o + X_MODE, mode);
    cpu.write8(o + X_STAGE, 0);
    let flags = cpu.read32(o + FLAGS) | FADED;
    cpu.write32(o + FLAGS, flags);
    nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
}

/// A scenery object's set-up step: the bullets cleared, its count (+0x6c) 1, then its next mode.
pub fn scenery_start(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0605_61e0, screen::bullets_cleared(cpu));
    cpu.write32(o + 0x6c, 1);
    nested!(cpu, 0x0608_2254, scenery_next(cpu, o));
}

/// Point `n` of the stage's table (12 bytes each, from 0x06088064) on the screen, at `out`.
const POINTS: u32 = 0x0608_8064;

pub fn point_on_screen(cpu: &mut Cpu, n: u32, out: u32) {
    nested!(cpu, 0x0606_71cc, sgl::sl_push_matrix(cpu));
    nested!(cpu, 0x0606_db7c, sgl::sl_load_matrix(cpu, SCENERY_MATRIX));
    nested!(cpu, 0x0606_750c, sgl::sl_convert(cpu, POINTS.wrapping_add(12u32.wrapping_mul(n)), out));
    nested!(cpu, 0x0606_7200, sgl::sl_pop_matrix(cpu));
}

/// A ribbon shot (and its flare and sound 41) from the point at `at` for player `o`'s number, by
/// `kind`: 0 a straight one, 1 a homing one's (for a time) with its wave base random, 2 a waving
/// one; at `angle`. Another kind makes only the flare and the sound.
const RD_NUMBER: u32 = 0x1e;
const RD_COLOUR: u32 = 0x20;
const RD_HEADING: u32 = 0x2c;
const RD_TURN: u32 = 0x2e;
const RD_ACCEL: u32 = 0x30;
const RD_WAVE_BASE: u32 = 0x34;
const RD_TURN_WITHIN: u32 = 0x38;
const RD_SOURCE: u32 = 0x3c;
const SHOT_ACCEL: u32 = 0xa3d;
const SHOT_SOUND: u32 = 41;

pub fn ribbon_shot(cpu: &mut Cpu, o: u32, at: u32, kind: u32, angle: u32) {
    let d = cpu.stack();
    let number = cpu.read8(o + NUMBER);
    cpu.write8(d + RD_NUMBER, number);
    let x = cpu.read32(at);
    cpu.write32(d, x);
    let y = cpu.read32(at + 4);
    cpu.write32(d + 4, y);
    cpu.write32(d + 8, 0);
    cpu.write16(d + RD_HEADING, angle as u16);
    cpu.write32(d + RD_SOURCE, 0);
    let first = cpu.read16(CHAR_BASE);
    match kind {
        0 => {
            cpu.write32(d + RD_ACCEL, SHOT_ACCEL);
            cpu.write16(d + RD_COLOUR, first.wrapping_add(48));
            nested!(cpu, 0x0606_42e0, ribbons::ribbon_set(cpu, 0, d, 0));
        }
        1 => {
            cpu.write32(d + RD_ACCEL, SHOT_ACCEL);
            let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0));
            cpu.write32(d + RD_WAVE_BASE, (r & 63) + 48);
            cpu.write16(d + RD_COLOUR, first.wrapping_add(16));
            nested!(cpu, 0x0606_42e0, ribbons::ribbon_set(cpu, 3, d, 1));
        }
        2 => {
            cpu.write16(d + RD_TURN, 0x16c);
            cpu.write16(d + RD_TURN_WITHIN, 0x1200);
            cpu.write32(d + RD_ACCEL, ONE);
            cpu.write32(d + RD_WAVE_BASE, 0x1_8000);
            cpu.write16(d + RD_COLOUR, first.wrapping_add(64));
            nested!(cpu, 0x0606_42e0, ribbons::ribbon_set(cpu, 2, d, 0));
        }
        _ => {}
    }
    nested!(cpu, 0x0606_43ec, effects::flare_set(cpu, d));
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, SHOT_SOUND));
}

/// The stage object `o` shoots when its mask (+0x8c) leaves none of `skip` set: the place the
/// stage's function at 0x06087a78 picks (for the mask's bits of `mask` and `spare`: below 12, odd
/// ones counting from point 6 up, even ones from point 5 down) projected on the screen, a shot of
/// `kind` from there at the angle to the player 15 frames on.
pub fn masked_shot(cpu: &mut Cpu, o: u32, skip: u32, mask: u32, spare: u32, kind: u32) {
    let m = cpu.read32(o + X_WAIT);
    if skip & m != 0 {
        return;
    }
    let k = cpu.call(0x0608_7a78, &[m & mask, spare]);
    if k as i32 > 11 {
        return;
    }
    let n = if k & 1 != 0 { ((k as i32 >> 1) + 6) as u32 } else { (5 - (k as i32 >> 1)) as u32 };
    let at = cpu.stack();
    nested!(cpu, 0x0608_22b8, point_on_screen(cpu, n, at));
    let angle = nested!(cpu, 0x0608_08e0, aim_angle(cpu, o, at, 15));
    nested!(cpu, 0x0608_2310, ribbon_shot(cpu, o, at, kind, angle as i16 as i32 as u32));
}
static MASKED_SHOT_PROLOGUE: std::sync::OnceLock<super::Prologue> = std::sync::OnceLock::new();

/// The stage's first shot pattern: by the stage's phase (below 2 or past 4: 127, 0x780 and 7;
/// else 63, 0x3c0 and 6), a kind 2 shot.
pub fn shots_a(cpu: &mut Cpu, o: u32) {
    let phase = cpu.read32(STAGE_PHASE) as i32;
    let (skip, mask, spare) = if !(2..=4).contains(&phase) { (127, 0x780, 7) } else { (63, 0x3c0, 6) };
    cpu.nested_with_stack(&MASKED_SHOT_PROLOGUE, 0x0608_2424, &[2], |cpu| masked_shot(cpu, o, skip, mask, spare, 2));
}

/// The second: in phases 2 to 4, 31, 0x1e0 and 5; else 63, 0x3c0 and 6.
pub fn shots_b(cpu: &mut Cpu, o: u32) {
    let phase = cpu.read32(STAGE_PHASE) as i32;
    let (skip, mask, spare) = if (2..=4).contains(&phase) { (31, 0x1e0, 5) } else { (63, 0x3c0, 6) };
    cpu.nested_with_stack(&MASKED_SHOT_PROLOGUE, 0x0608_2424, &[2], |cpu| masked_shot(cpu, o, skip, mask, spare, 2));
}

/// The script's next entry (8 bytes: a byte, a long wait) for the object: the byte at +0x73, the
/// wait at +0x8c, the entry's place on.
pub fn script_step(cpu: &mut Cpu, o: u32) {
    let at = cpu.read32(o + X_SCRIPT);
    let b = cpu.read8(at);
    cpu.write8(o + X_BYTE, b);
    let at = cpu.read32(o + X_SCRIPT);
    let wait = cpu.read32(at + 4);
    cpu.write32(o + X_WAIT, wait);
    let at = cpu.read32(o + X_SCRIPT).wrapping_add(8);
    cpu.write32(o + X_SCRIPT, at);
}

/// The stage's leader (an object): its life (+0x50), a count of its deaths (+0x6c), who last hit it
/// (+0x40, an object), and the totals it adds to (0x060746f0).
const L_HIT_BY: u32 = 0x40;
const L_DEATHS: u32 = 0x6c;
const TOTAL: u32 = 0x0607_46f0;
const LEADER_POINTS: u32 = 0x2710;
const LEADER_LIFE: u32 = 300_000;
const LEADER_SCRIPT: u32 = 0x0608_81dc;
const LEADER_TIMELINE: u32 = 0x0608_80f4;
const LEADER_MOVES: u32 = 0x0608_81cc;
const LEADER_WAIT: u32 = 60;
/// Where the leader is shown on the screen from (a point of the stage's).
const LEADER_POINT: u32 = 0x0608_820c;

/// The leader's behaviour. Past its first two sub-steps, its life given to the boss's gauge; once
/// that is gone (once): the leader's death at 0x06082520, the bullets cleared, 40 on the total, the
/// kill scored for whoever last hit it, its flag (drawn faded) set, its death counted, its wait 0,
/// its sub-step 1. Sub-step 0: the boss's gauge made (300000), its flag off, its counter and wait
/// 0, its script and timeline set, the script's first entry, on; 1: 60 frames, then its next mode
/// (and nothing drawn); 2: the timeline stepped, its wait down a frame, the script's function for
/// the entry's byte run, the next entry at wait 0. Drawn: its place (+0x14) from the stage's point.
pub fn leader_behaviour(cpu: &mut Cpu, o: u32) {
    if cpu.read8(o + X_STAGE) as i8 > 1 {
        let life = cpu.read32(o + LIFE);
        nested!(cpu, 0x0604_64b8, hud::boss_gauge_life(cpu, life));
        if cpu.read32(o + LIFE) as i32 <= 0 {
            let deaths = cpu.read32(o + L_DEATHS);
            if deaths == 0 {
                cpu.call(0x0608_2520, &[o]);
                nested!(cpu, 0x0605_61e0, screen::bullets_cleared(cpu));
                let total = cpu.read32(TOTAL).wrapping_add(40);
                cpu.write32(TOTAL, total);
                let by = cpu.read32(o + L_HIT_BY);
                let weapon = cpu.read8(by + TYPE) as u32;
                let p = cpu.read8(by + NUMBER) as u32;
                nested!(cpu, 0x0604_3108, hits::score_kill(cpu, p, weapon, o, LEADER_POINTS));
                let flags = cpu.read32(o + FLAGS) | FADED;
                cpu.write32(o + FLAGS, flags);
                let n = cpu.read32(o + L_DEATHS).wrapping_add(1);
                cpu.write32(o + L_DEATHS, n);
                cpu.write32(o + X_WAIT, deaths);
                cpu.write8(o + X_STAGE, 1);
            }
        }
    }
    match cpu.read8(o + X_STAGE) as i8 {
        1 => {
            let n = cpu.read32(o + X_WAIT).wrapping_add(1);
            cpu.write32(o + X_WAIT, n);
            if n == LEADER_WAIT {
                nested!(cpu, 0x0608_2254, scenery_next(cpu, o));
                return;
            }
        }
        0 => {
            nested!(cpu, 0x0604_64d8, hud::boss_gauge_set(cpu, LEADER_LIFE));
            let flags = cpu.read32(o + FLAGS) & !FADED;
            cpu.write32(o + FLAGS, flags);
            cpu.write32(o + X_COUNT, 0);
            cpu.write32(o + X_WAIT, 0);
            cpu.write32(o + X_SCRIPT, LEADER_SCRIPT);
            cpu.write32(o + X_FRAMES, 0);
            cpu.write32(o + X_TIMELINE, LEADER_TIMELINE);
            nested!(cpu, 0x0608_23fc, script_step(cpu, o));
            cpu.write8(o + X_STAGE, 2);
        }
        2 => {
            nested!(cpu, 0x0608_1fcc, timeline_step(cpu, o + X_TIMELINE, o + X_FRAMES));
            let n = cpu.read32(o + X_WAIT).wrapping_sub(1);
            cpu.write32(o + X_WAIT, n);
            let byte = cpu.read8(o + X_BYTE) as i8 as i32 as u32;
            let f = cpu.read32(LEADER_MOVES.wrapping_add(byte << 2));
            cpu.call_returning(0x0608_268e, f, &[o]);
            if cpu.read32(o + X_WAIT) == 0 {
                nested!(cpu, 0x0608_23fc, script_step(cpu, o));
            }
        }
        _ => {}
    }
    nested!(cpu, 0x0606_71cc, sgl::sl_push_matrix(cpu));
    nested!(cpu, 0x0606_db7c, sgl::sl_load_matrix(cpu, SCENERY_MATRIX));
    nested!(cpu, 0x0606_750c, sgl::sl_convert(cpu, LEADER_POINT, o + POSITION));
    nested!(cpu, 0x0606_7200, sgl::sl_pop_matrix(cpu));
}

/// The director's own fields: where it holds the camera's x (+0xa8), and two objects it moves
/// (+0xb8, +0xbc: their +0x40 and +0x44 are what it sets).
const DR_CAMERA_X: u32 = 0xa8;
const DR_FIRST: u32 = 0xb8;
const DR_SECOND: u32 = 0xbc;
const DR_LAUNCH_AT: u32 = 0xc0;
const DR_LAUNCH_FRAMES: u32 = 0x180;
const DR_END_AT: u32 = 0x155;
const DR_SOUND: u32 = 73;
const DR_STEP: u32 = 0x100;
const DR_VIEW_END: i8 = 6;

/// The director's behaviour; its camera's x kept first. Sub-step 0: its count 0, on into 1: its
/// count up; at 192 the chargers' launcher for 384 frames; at 341 (and the count 0) sound 73, on;
/// 2: its count 0x100 a frame, the first object's +0x40 eased from 0x280000 to 0x800000 and the
/// second's its opposite, then on; 3: the same, +0x44 of both eased from -0x680000 to 0; 4: when
/// the view's stage counter is 6, its next mode.
pub fn director_behaviour(cpu: &mut Cpu, o: u32) {
    let view = cpu.read32(CAMERA + CAM_POSITION);
    cpu.write32(o + DR_CAMERA_X, view);
    let step = cpu.read8(o + X_STAGE);
    if step > 4 {
        return;
    }
    let advance = |cpu: &mut Cpu| {
        let n = cpu.read8(o + X_STAGE).wrapping_add(1);
        cpu.write8(o + X_STAGE, n);
    };
    match step {
        0 | 1 => {
            if step == 0 {
                cpu.write32(o + 0x6c, 0);
                cpu.write32(o + X_COUNT, 0);
                advance(cpu);
            }
            if cpu.read32(o + X_COUNT) == DR_LAUNCH_AT {
                let frames = cpu.stack();
                cpu.write32(frames, DR_LAUNCH_FRAMES);
                nested!(cpu, 0x0608_1f8c, charger_launcher_set(cpu, frames));
            }
            let n = cpu.read32(o + X_COUNT).wrapping_add(1);
            cpu.write32(o + X_COUNT, n);
            if n == DR_END_AT {
                cpu.write32(o + X_COUNT, 0);
                nested!(cpu, 0x0600_50d4, sound::se_set(cpu, DR_SOUND));
                advance(cpu);
            }
        }
        2 | 3 => {
            let t = cpu.read32(o + X_COUNT).wrapping_add(DR_STEP);
            cpu.write32(o + X_COUNT, t);
            if step == 2 {
                let v = nested!(cpu, 0x0606_2ebc, math::ease(cpu, 0x28_0000, 0x80_0000, t));
                let first = cpu.read32(o + DR_FIRST);
                if first != 0 {
                    cpu.write32(first + 0x40, v);
                }
                let second = cpu.read32(o + DR_SECOND);
                if second != 0 {
                    cpu.write32(second + 0x40, v.wrapping_neg());
                }
            } else {
                let v = nested!(cpu, 0x0606_2ebc, math::ease(cpu, 0xff98_0000, 0, t));
                for at in [DR_FIRST, DR_SECOND] {
                    let p = cpu.read32(o + at);
                    if p != 0 {
                        cpu.write32(p + 0x44, v);
                    }
                }
            }
            if cpu.read32(o + X_COUNT) == ONE {
                cpu.write32(o + X_COUNT, 0);
                advance(cpu);
            }
        }
        _ => {
            let view = cpu.read32(VIEW);
            if cpu.read8(view + VIEW_COUNTER) as i8 == DR_VIEW_END {
                let mode = cpu.read8(o + X_MODE).wrapping_add(1);
                cpu.write8(o + X_MODE, mode);
                cpu.write8(o + X_STAGE, 0);
            }
        }
    }
}

// ---- the camera's movers -------------------------------------------------------------------------

/// A camera mover (an object): its mode (+0x10), its sub-step (+0x11), the roll's speed and the
/// roll (words, +0x12, +0x14), its height (+0x1c) eased from +0x20 to +0x24 by its time (+0x30),
/// and the roll eased from +0x28 to +0x2c.
const M_MODE: u32 = 0x10;
const M_STEP: u32 = 0x11;
const M_SPEED: u32 = 0x12;
const M_ROLL: u32 = 0x14;
const M_HEIGHT: u32 = 0x1c;
const M_FROM: u32 = 0x20;
const M_TO: u32 = 0x24;
const M_ROLL_FROM: u32 = 0x28;
const M_ROLL_TO: u32 = 0x2c;
const M_TIME: u32 = 0x30;

/// The mover's roll turned by its speed, and the camera given it.
pub fn camera_roll_step(cpu: &mut Cpu, o: u32) {
    let roll = cpu.read16(o + M_ROLL).wrapping_add(cpu.read16(o + M_SPEED));
    cpu.write16(o + M_ROLL, roll);
    let roll = cpu.read16(o + M_ROLL) as i16 as i32 as u32;
    cpu.write32(CAMERA + CAM_ROLL, roll);
}

/// The mover's time on `step` (to one at most), its height eased from its start to its end and
/// the camera's y given it.
pub fn camera_height_step(cpu: &mut Cpu, o: u32, step: u32) {
    let t = cpu.read32(o + M_TIME).wrapping_add(step);
    cpu.write32(o + M_TIME, t);
    if t as i32 > 0xffff {
        cpu.write32(o + M_TIME, ONE);
    }
    let (a, b, t) = (cpu.read32(o + M_FROM), cpu.read32(o + M_TO), cpu.read32(o + M_TIME));
    let y = nested!(cpu, 0x0606_2ebc, math::ease(cpu, a, b, t));
    cpu.write32(o + M_HEIGHT, y);
    cpu.write32(CAMERA + CAM_POSITION + 4, y);
}

/// A mover that drops the camera to `to`: sub-step 0 starts from its height (time 0), then 32 a
/// frame (its mode 0 at the end); the roll kept as it is.
fn camera_drop(cpu: &mut Cpu, o: u32, to: u32) {
    match cpu.read8(o + M_STEP) as i8 {
        0 => {
            let height = cpu.read32(o + M_HEIGHT);
            cpu.write32(o + M_TIME, 0);
            cpu.write32(o + M_FROM, height);
            cpu.write32(o + M_TO, to);
            let n = cpu.read8(o + M_STEP).wrapping_add(1);
            cpu.write8(o + M_STEP, n);
        }
        1 => {}
        _ => {
            nested!(cpu, 0x0608_2890, camera_roll_step(cpu, o));
            return;
        }
    }
    nested!(cpu, 0x0608_28b0, camera_height_step(cpu, o, 32));
    if cpu.read32(o + M_TIME) == ONE {
        cpu.write8(o + M_MODE, 0);
    }
    nested!(cpu, 0x0608_2890, camera_roll_step(cpu, o));
}

/// The camera mover that drops it to -0x500000 ...
pub fn camera_drop_far(cpu: &mut Cpu, o: u32) {
    camera_drop(cpu, o, 0xffb0_0000);
}

/// ... and the one that drops it to 0x500000.
pub fn camera_drop_near(cpu: &mut Cpu, o: u32) {
    camera_drop(cpu, o, 0x0050_0000);
}

/// The mover that winds the roll's speed up to 31 (one a frame).
pub fn camera_roll_up(cpu: &mut Cpu, o: u32) {
    let speed = cpu.read16(o + M_SPEED);
    if speed as i16 as i32 <= 31 {
        cpu.write16(o + M_SPEED, speed.wrapping_add(1));
    }
    nested!(cpu, 0x0608_2890, camera_roll_step(cpu, o));
}

/// The mover that winds the roll's speed down to 0 (sub-step 0, then sub-step 1); sub-step 1: the
/// camera's roll eased back to 0 as the height is, then the mover ends (112).
pub fn camera_roll_back(cpu: &mut Cpu, o: u32) {
    match cpu.read8(o + M_STEP) as i8 {
        0 => {
            let speed = cpu.read16(o + M_SPEED);
            if speed as i16 > 0 {
                cpu.write16(o + M_SPEED, speed.wrapping_sub(1));
            } else {
                cpu.write8(o + M_STEP, 1);
            }
            nested!(cpu, 0x0608_2890, camera_roll_step(cpu, o));
            return;
        }
        1 => {
            let roll = cpu.read32(CAMERA + CAM_ROLL);
            cpu.write32(o + M_ROLL_FROM, roll);
            cpu.write32(o + M_ROLL_TO, 0);
            nested!(cpu, 0x0606_2e40, math::unwrap_angles(cpu, o + M_ROLL_FROM, o + M_ROLL_TO));
            cpu.write32(o + M_TO, 0);
            let height = cpu.read32(o + M_HEIGHT);
            cpu.write32(o + M_TIME, 0);
            cpu.write32(o + M_FROM, height);
            let n = cpu.read8(o + M_STEP).wrapping_add(1);
            cpu.write8(o + M_STEP, n);
        }
        2 => {}
        _ => return,
    }
    nested!(cpu, 0x0608_28b0, camera_height_step(cpu, o, 0x120));
    let (a, b, t) = (cpu.read32(o + M_ROLL_FROM), cpu.read32(o + M_ROLL_TO), cpu.read32(o + M_TIME));
    let roll = nested!(cpu, 0x0606_2ebc, math::ease(cpu, a, b, t));
    cpu.write32(CAMERA + CAM_ROLL, roll);
    if cpu.read32(o + M_TIME) == ONE {
        cpu.write8(o + OBJ_MODE, TO_STOP);
    }
}

/// The camera's mover object's behaviours by its mode.
const MOVERS: u32 = 0x0608_8320;

/// The camera's mover object (unless the game is held). Mode 0: its height the camera's, its roll
/// the camera's spin, its speed 0, its roll's start 0; mode 1: the movers' function by its +0x10.
/// 112: 115; 115: freed.
pub fn camera_mover(cpu: &mut Cpu, o: u32) {
    if cpu.read32(GAME_FLAGS) & HELD != 0 {
        return;
    }
    match cpu.read8(o + OBJ_MODE) as i8 {
        0 => {
            let y = cpu.read32(CAMERA + CAM_POSITION + 4);
            cpu.write32(o + M_HEIGHT, y);
            let spin = cpu.read16(CAMERA + CAM_SPIN);
            cpu.write16(o + M_ROLL, spin);
            cpu.write16(o + M_SPEED, 0);
            cpu.write32(o + 0x18, 0);
            cpu.write8(o + OBJ_MODE, 1);
        }
        1 => {}
        112 => {
            cpu.write8(o + OBJ_MODE, FREED);
            return;
        }
        115 => {
            nested!(cpu, 0x0600_579c, objects::obj_free(cpu, o));
            return;
        }
        _ => return,
    }
    let mode = cpu.read8(o + M_MODE) as i8 as i32 as u32;
    let f = cpu.read32(MOVERS.wrapping_add(mode << 2));
    cpu.call_returning(0x0608_2b5e, f, &[o]);
}

/// The camera's mover made (list 0, 52 bytes).
pub fn camera_mover_set(cpu: &mut Cpu) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 52)) as i8 as i32 as u32;
    nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 0, blocks, 0x0608_2aec))
}

/// An object whose counter (+0x94) runs on a frame; every 128th (256th in phases 0 and 1) the
/// stage's function at 0x06085748 is given flags (16 when the next count has bit 0x80, 0x100 in
/// phases 0 and 1, set; 32 when the camera's y is below 0).
pub fn pulse_shot(cpu: &mut Cpu, o: u32) {
    let (mask, bit) = if cpu.read32(STAGE_PHASE) as i32 > 1 { (127, 0x80) } else { (0xff, 0x100) };
    let n = cpu.read32(o + 0x94);
    cpu.write32(o + 0x94, n.wrapping_add(1));
    if n & mask != 0 {
        return;
    }
    let mut flags = if n.wrapping_add(1) & bit != 0 { 16 } else { 0 };
    if (cpu.read32(CAMERA + CAM_POSITION + 4) as i32) < 0 {
        flags |= 32;
    }
    cpu.call(0x0608_5748, &[o, flags]);
}

/// A drone's limits by the stage's phase (bytes, from 0x06088338), and its animation's state (+0x7c)
/// and counter (+0x7d, bytes).
const LIMITS: u32 = 0x0608_8338;
const A_STATE: u32 = 0x7c;
const A_COUNT: u32 = 0x7d;
const A_HITS: u32 = 0x77;
const A_TICKS: u32 = 0x90;

/// A drone animated every 16th game frame while its count of hits (+0x77) is below the phase's
/// limit: the stage's function at 0x06085bb4 given flags (16 and 32 by bits 0 and 1 of its ticks,
/// and its state), its state 0 counting up 16 frames to 1, 1 counting 32 to 2, 2 back to 0. Its ticks
/// up a frame.
pub fn animation_step(cpu: &mut Cpu, o: u32) {
    if cpu.read32(GAME_FRAMES) & 15 != 0 {
        return;
    }
    let phase = cpu.read32(STAGE_PHASE);
    let limit = cpu.read8(LIMITS.wrapping_add(phase)) as i8;
    let hits = cpu.read8(o + A_HITS) as i8;
    if hits < limit {
        let ticks = cpu.read32(o + A_TICKS);
        let mut flags = if ticks & 1 != 0 { 16 } else { 0 };
        if ticks & 2 != 0 {
            flags |= 32;
        }
        flags |= cpu.read8(o + A_STATE) as i8 as i32 as u32;
        cpu.call(0x0608_5bb4, &[o, flags]);
        let counting = |cpu: &mut Cpu, last: i8| {
            let n = cpu.read8(o + A_COUNT).wrapping_add(1);
            cpu.write8(o + A_COUNT, n);
            if n as i8 == last {
                cpu.write8(o + A_COUNT, 0);
                let state = cpu.read8(o + A_STATE).wrapping_add(1);
                cpu.write8(o + A_STATE, state);
            }
        };
        match cpu.read8(o + A_STATE) as i8 {
            1 => counting(cpu, 32),
            0 => counting(cpu, 16),
            2 => cpu.write8(o + A_STATE, 0),
            _ => {}
        }
    }
    let ticks = cpu.read32(o + A_TICKS).wrapping_add(1);
    cpu.write32(o + A_TICKS, ticks);
}

// ---- the boss's drone patterns -------------------------------------------------------------------

/// A drone's spec (see [`drone_setup`]) also has its flags at +0x14: bit 0 the second side, bit 4
/// (16) from a pattern's flags' bit 6.
const SPEC_FLAGS: u32 = 0x14;
/// The two places drones start at across (longs from 0x06089ee0: the one the pattern's flag 16
/// picks and the other).
const DRONE_X: u32 = 0x0608_9ee0;
/// The drone makers (each makes a numbered drone from a spec and gives it a behaviour) and the
/// behaviours the patterns give.
const MAKER_A: u32 = 0x0608_60c8;
const MAKER_B: u32 = 0x0608_6618;
const GUN_BEHAVIOUR: u32 = 0x0608_5d4c;
const PATTERN_BEHAVIOUR_C: u32 = 0x0608_6210;
/// A pattern's object: its ticks (+0x98), its step (+0x9c) and its cycle (+0xa0).
const P_TICKS: u32 = 0x98;
const P_STEP: u32 = 0x9c;
const P_CYCLE: u32 = 0xa0;
/// The camera's height (its y).
const CAMERA_Y: u32 = CAMERA + CAM_POSITION + 4;

/// A drone made from a spec built on the stack by `flags` (bit 4 (16): the drone starts at the
/// second place across, not the first; bit 5 (32): from below with its turn half way round, not
/// from above; bit 6 (64): its spec's flags 16; bits 0-1: its type) and `behaviour`: `maker` is
/// called with the spec.
pub fn make_drone(cpu: &mut Cpu, flags: u32, behaviour: u32, maker: u32) {
    let s = cpu.stack();
    let (x, mut spec_flags) = if flags & 16 == 0 { (cpu.read32(DRONE_X + 4), 1) } else { (cpu.read32(DRONE_X), 0) };
    cpu.write32(s + SPEC_POSITION, x);
    if flags & 64 != 0 {
        spec_flags |= 16;
    }
    cpu.write32(s + SPEC_FLAGS, spec_flags);
    let (y, turn) = if flags & 32 == 0 { (0x00c0_0000, 0) } else { (0xff40_0000, 0x8000) };
    cpu.write32(s + SPEC_POSITION + 4, y);
    cpu.write16(s + SPEC_TURN, turn);
    cpu.write32(s + SPEC_POSITION + 8, 0);
    cpu.write16(s + SPEC_TYPE, (flags & 3) as u16);
    cpu.write32(s + SPEC_BEHAVIOUR, behaviour);
    cpu.call_returning(0x0608_2daa, maker, &[s]);
}

/// A pattern of gun drones: every 32nd tick, except that at tick 96 of 128 its cycle goes on, a drone
/// whose type is the tick's bits 7 and 8 (mixed up by which side the camera is on) and whose side
/// is the cycle's; its ticks up one.
pub fn drone_pattern_a(cpu: &mut Cpu, o: u32) {
    let ticks = cpu.read32(o + P_TICKS);
    if ticks & 31 == 0 {
        if ticks & 127 == 96 {
            let c = cpu.read32(o + P_CYCLE).wrapping_add(1);
            cpu.write32(o + P_CYCLE, c);
        } else {
            let above = cpu.read32(CAMERA_Y) as i32 > 0;
            nested!(cpu, 0x0600_8166, super::clib::ashiftrt_r4::<7>(cpu, ticks));
            let k = ((ticks as i32 >> 7) & 3) as usize;
            let mut flags = if above { [32, 48, 16, 0][k] } else { [16, 0, 32, 48][k] };
            if cpu.read32(o + P_CYCLE) & 1 != 0 {
                flags |= 2;
            }
            nested!(cpu, 0x0608_2d44, make_drone(cpu, flags, GUN_BEHAVIOUR, MAKER_A));
        }
    }
    let t = cpu.read32(o + P_TICKS).wrapping_add(1);
    cpu.write32(o + P_TICKS, t);
}

/// A pattern of four gun drones every 128th tick (the types 0 to 3 of its cycle's side: the cycle
/// or'd with 16, 0, 48 and 32), its cycle on one (3 back to 0); its ticks up one.
pub fn drone_pattern_b(cpu: &mut Cpu, o: u32) {
    if cpu.read32(o + P_TICKS) & 127 == 0 {
        for with in [16, 0, 48, 32] {
            let flags = cpu.read32(o + P_CYCLE) | with;
            nested!(cpu, 0x0608_2d44, make_drone(cpu, flags, GUN_BEHAVIOUR, MAKER_A));
        }
        let c = cpu.read32(o + P_CYCLE);
        cpu.write32(o + P_CYCLE, c.wrapping_add(1));
        if c.wrapping_add(1) as i32 > 2 {
            cpu.write32(o + P_CYCLE, c.wrapping_sub(2));
        }
    }
    let t = cpu.read32(o + P_TICKS).wrapping_add(1);
    cpu.write32(o + P_TICKS, t);
}

/// A pattern of pairs of drones every 16th tick: a step (0 to 5; past 2 the cycle's side turns the
/// other way), its cycle (0 to 2) and the camera's side choose the flags of the first (type from
/// the cycle's next) and the second (from the cycle); each made with the maker at 0x06086618.
pub fn drone_pattern_c(cpu: &mut Cpu, o: u32) {
    if cpu.read32(o + P_TICKS) & 15 == 0 {
        let step = cpu.read32(o + P_STEP) as i32;
        let cycle = cpu.read32(o + P_CYCLE) as i32;
        let this = if step > 2 {
            if cycle + 2 > 2 { cycle - 1 } else { cycle + 2 }
        } else if cycle == 1 {
            0
        } else {
            cycle
        };
        let mut next = if cycle + 1 > 2 { cycle - 2 } else { cycle + 1 };
        if next == 1 && step > 2 {
            next = 0;
        }
        let step = step + 1;
        cpu.write32(o + P_STEP, step as u32);
        if step > 5 {
            cpu.write32(o + P_STEP, 0);
            let c = cpu.read32(o + P_CYCLE);
            cpu.write32(o + P_CYCLE, c.wrapping_add(2));
            if c.wrapping_add(2) as i32 > 2 {
                cpu.write32(o + P_CYCLE, c.wrapping_sub(1));
            }
        }
        let mut flags = next as u32;
        if cpu.read32(CAMERA_Y) as i32 > 0 {
            flags |= 32;
        }
        flags |= 64;
        nested!(cpu, 0x0608_2d44, make_drone(cpu, flags, PATTERN_BEHAVIOUR_C, MAKER_B));
        let mut flags = if cpu.read32(CAMERA_Y) as i32 > 0 { 48 } else { 16 };
        flags |= this as u32;
        flags |= 64;
        nested!(cpu, 0x0608_2d44, make_drone(cpu, flags, PATTERN_BEHAVIOUR_C, MAKER_B));
    }
    let t = cpu.read32(o + P_TICKS).wrapping_add(1);
    cpu.write32(o + P_TICKS, t);
}

/// A pattern of drones with the behaviour `behaviour`: every 16th tick, except that at tick 48 of 64
/// its cycle goes on (past 2 back to 0), a drone whose side is the tick's bit 6 and the camera's
/// (its flags 32 and 48 above the camera's middle, 16 and 0 below), its cycle's type, and flags 64
/// except the first time of three (its step 0 to 2); its ticks up one.
pub fn drone_pattern_d(cpu: &mut Cpu, o: u32, behaviour: u32) {
    let ticks = cpu.read32(o + P_TICKS);
    if ticks & 15 == 0 {
        if ticks & 63 == 48 {
            let c = cpu.read32(o + P_CYCLE);
            cpu.write32(o + P_CYCLE, c.wrapping_add(1));
            if c.wrapping_add(1) as i32 > 2 {
                cpu.write32(o + P_CYCLE, c.wrapping_sub(2));
            }
        } else {
            let mut flags = if cpu.read32(CAMERA_Y) as i32 > 0 {
                if ticks & 64 == 0 { 32 } else { 48 }
            } else if ticks & 64 == 0 {
                16
            } else {
                0
            };
            flags |= cpu.read32(o + P_CYCLE);
            let step = cpu.read32(o + P_STEP);
            if step != 0 {
                flags |= 64;
            }
            cpu.write32(o + P_STEP, step.wrapping_add(1));
            if step.wrapping_add(1) == 3 {
                cpu.write32(o + P_STEP, 0);
            }
            nested!(cpu, 0x0608_2d44, make_drone(cpu, flags, behaviour, MAKER_B));
        }
    }
    let t = cpu.read32(o + P_TICKS).wrapping_add(1);
    cpu.write32(o + P_TICKS, t);
}

/// The pattern of drones [`drone_pattern_d`] makes with the behaviour at 0x060862e4.
pub fn drone_pattern_e(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_3030, drone_pattern_d(cpu, o, 0x0608_62e4));
}

/// The same with the behaviour at 0x060864e0.
pub fn drone_pattern_f(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_3030, drone_pattern_d(cpu, o, 0x0608_64e0));
}

// ---- the boss's gates ----------------------------------------------------------------------------

/// A gate (an object): its step (+0x72, a byte), the move of its script's entry (+0x73), its
/// counter (+0x8c) and wait (+0x88), the place of its script's next entry (+0xa4; entries of 16
/// bytes), the side it is at (+0x7e), two parts (objects: +0xb8 and +0xbc; each's place across is
/// at +0x40), the places it has them at (+0xc0, +0xc4), the places it takes them to (+0xc8, +0xcc),
/// the places it kept (+0xd0, +0xd4) and its linked object (+0xd8).
const G_STEP: u32 = 0x72;
const G_MOVE: u32 = 0x73;
const G_TIME: u32 = 0x8c;
const G_WAIT: u32 = 0x88;
const G_SCRIPT: u32 = 0xa4;
const G_SIDE: u32 = 0x7e;
const G_PARTS: [u32; 2] = [0xb8, 0xbc];
const G_AT: [u32; 2] = [0xc0, 0xc4];
const G_TO: [u32; 2] = [0xc8, 0xcc];
const G_KEPT: [u32; 2] = [0xd0, 0xd4];
const G_LINK: u32 = 0xd8;
const PART_AT: u32 = 0x40;
/// A gate's counters the script's entries set (bytes +0x74 and +0x75, and the longs after them).
const G_BYTE_A: u32 = 0x74;
const G_BYTE_B: u32 = 0x75;

/// A gate's part `n` (0 or 1) put at the gate's place `from` (+0xc0/+0xc4 or +0xc8/+0xcc) plus
/// `add`, if it has the part.
fn gate_part_set(cpu: &mut Cpu, o: u32, n: usize, from: u32, add: u32) {
    let part = cpu.read32(o + G_PARTS[n]);
    if part != 0 {
        let v = cpu.read32(o + from).wrapping_add(add);
        cpu.write32(part + PART_AT, v);
    }
}

/// A gate's step on one.
fn gate_step_next(cpu: &mut Cpu, o: u32) {
    let n = cpu.read8(o + G_STEP).wrapping_add(1);
    cpu.write8(o + G_STEP, n);
}

/// A gate's parts eased: its counter on by `step` to at most 0x10000, each part's place across the
/// ease from the place it has to the place it goes to by the counter.
pub fn gate_ease(cpu: &mut Cpu, o: u32, step: u32) {
    let t = cpu.read32(o + G_TIME).wrapping_add(step);
    cpu.write32(o + G_TIME, t);
    if t as i32 > 0x1_0000 {
        cpu.write32(o + G_TIME, 0x1_0000);
    }
    for n in 0..2 {
        if cpu.read32(o + G_PARTS[n]) != 0 {
            let from = cpu.read32(o + G_AT[n]);
            let to = cpu.read32(o + G_TO[n]);
            let t = cpu.read32(o + G_TIME);
            let r = nested!(cpu, 0x0606_2ebc, math::ease(cpu, from, to, t));
            let part = cpu.read32(o + G_PARTS[n]);
            cpu.write32(part + PART_AT, r);
        }
    }
}

/// A gate eased 0x200 a frame; when its counter reaches 0x10000 its script goes on.
pub fn gate_ease_next(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_3170, gate_ease(cpu, o, 0x200));
    if cpu.read32(o + G_TIME) == 0x1_0000 {
        nested!(cpu, 0x0608_38c8, gate_next_entry(cpu, o));
    }
}

/// A gate eased 0x80 a frame; when its counter reaches 0x10000 it goes back to 0 and the script
/// goes on.
pub fn gate_ease_restart(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_3170, gate_ease(cpu, o, 0x80));
    if cpu.read32(o + G_TIME) == 0x1_0000 {
        cpu.write32(o + G_TIME, 0);
        nested!(cpu, 0x0608_38c8, gate_next_entry(cpu, o));
    }
}

/// The gate's second and third steps' ending: the counter, the bytes +0x74 and +0x75 cleared.
fn gate_bytes_clear(cpu: &mut Cpu, o: u32) {
    cpu.write32(o + G_TIME, 0);
    cpu.write8(o + G_BYTE_A, 0);
    cpu.write8(o + G_BYTE_B, 0);
}

/// A gate's steps (by its step, +0x72): 0 the parts shaken out by 0x20000 every other frame for 61
/// frames (then their goals 0x200000 apart); 1 and 2 eased 0x800 a frame (the places and goals
/// swapped back after 1; the sound 74 after 2, the bytes cleared); 3 shaken again by 0x10000 for
/// 31 frames; 4 waiting 60 frames for the script's next entry.
pub fn gate_steps_a(cpu: &mut Cpu, o: u32) {
    match cpu.read8(o + G_STEP) {
        0 => {
            let odd = cpu.read32(o + G_TIME) & 1 != 0;
            let (a, b) = if odd { (0x2_0000, 0xfffe_0000) } else { (0, 0) };
            gate_part_set(cpu, o, 0, G_AT[0], a);
            gate_part_set(cpu, o, 1, G_AT[1], b);
            let t = cpu.read32(o + G_TIME).wrapping_add(1);
            cpu.write32(o + G_TIME, t);
            if t != 61 {
                return;
            }
            for n in 0..2 {
                let v = cpu.read32(o + G_TO[n]);
                cpu.write32(o + G_KEPT[n], v);
            }
            let (a, b) = (cpu.read32(o + G_AT[0]), cpu.read32(o + G_AT[1]));
            cpu.write32(o + G_TO[0], a.wrapping_add(0x20_0000));
            cpu.write32(o + G_TO[1], b.wrapping_sub(0x20_0000));
            cpu.write32(o + G_TIME, 0);
            gate_step_next(cpu, o);
        }
        1 => {
            nested!(cpu, 0x0608_3170, gate_ease(cpu, o, 0x800));
            if cpu.read32(o + G_TIME) != 0x1_0000 {
                return;
            }
            gate_swap_places(cpu, o);
            cpu.write32(o + G_TIME, 0);
            gate_step_next(cpu, o);
        }
        2 => {
            nested!(cpu, 0x0608_3170, gate_ease(cpu, o, 0x800));
            if cpu.read32(o + G_TIME) != 0x1_0000 {
                return;
            }
            gate_bytes_clear(cpu, o);
            nested!(cpu, 0x0600_50d4, sound::se_set(cpu, GATE_SOUND));
            gate_step_next(cpu, o);
        }
        3 => {
            let odd = cpu.read32(o + G_TIME) & 1 != 0;
            let (a, b) = if odd { (0x1_0000, 0xffff_0000) } else { (0, 0) };
            gate_part_set(cpu, o, 0, G_TO[0], a);
            gate_part_set(cpu, o, 1, G_TO[1], b);
            let t = cpu.read32(o + G_TIME).wrapping_add(1);
            cpu.write32(o + G_TIME, t);
            if t == 31 {
                cpu.write32(o + G_TIME, 0);
                gate_step_next(cpu, o);
            }
        }
        4 => {
            let t = cpu.read32(o + G_TIME).wrapping_add(1);
            cpu.write32(o + G_TIME, t);
            if t == 60 {
                nested!(cpu, 0x0608_38c8, gate_next_entry(cpu, o));
            }
        }
        _ => {}
    }
}

/// The sound a gate plays as it closes.
const GATE_SOUND: u32 = 74;

/// The places a gate has and the places it goes to taken from the others and given back: the
/// places the goals, the goals the kept.
fn gate_swap_places(cpu: &mut Cpu, o: u32) {
    for n in 0..2 {
        let v = cpu.read32(o + G_TO[n]);
        cpu.write32(o + G_AT[n], v);
    }
    for n in 0..2 {
        let v = cpu.read32(o + G_KEPT[n]);
        cpu.write32(o + G_TO[n], v);
    }
}

/// The same steps for one part only, the one `side` (0 or 1) picks (the second part's shakes and
/// goals the other way): 0, 1, 2 (the sound 74 only), 3, then 4: after 30 frames drones made
/// by the function at 0x06084edc (its flags 2 when the camera is above 0, 1 for the second side)
/// whose result, if any, plays a sound 75; the counter and bytes cleared; 5 waiting 90 frames
/// for the script's next entry.
pub fn gate_steps_b(cpu: &mut Cpu, o: u32, side: u32) {
    let n = (side != 0) as usize;
    match cpu.read8(o + G_STEP) {
        0 => {
            let odd = cpu.read32(o + G_TIME) & 1 != 0;
            let add = if !odd {
                0
            } else if n == 0 {
                0x2_0000
            } else {
                0xfffe_0000
            };
            gate_part_set(cpu, o, n, G_AT[n], add);
            let t = cpu.read32(o + G_TIME).wrapping_add(1);
            cpu.write32(o + G_TIME, t);
            if t != 61 {
                return;
            }
            for k in 0..2 {
                let v = cpu.read32(o + G_TO[k]);
                cpu.write32(o + G_KEPT[k], v);
            }
            if n == 0 {
                cpu.write32(o + G_TO[0], 0x90_0000);
            } else {
                cpu.write32(o + G_TO[1], 0xff70_0000);
            }
            cpu.write32(o + G_TIME, 0);
            gate_step_next(cpu, o);
        }
        1 => {
            nested!(cpu, 0x0608_3170, gate_ease(cpu, o, 0x800));
            if cpu.read32(o + G_TIME) != 0x1_0000 {
                return;
            }
            gate_swap_places(cpu, o);
            cpu.write32(o + G_TIME, 0);
            gate_step_next(cpu, o);
        }
        2 => {
            nested!(cpu, 0x0608_3170, gate_ease(cpu, o, 0x800));
            if cpu.read32(o + G_TIME) != 0x1_0000 {
                return;
            }
            cpu.write32(o + G_TIME, 0);
            nested!(cpu, 0x0600_50d4, sound::se_set(cpu, GATE_SOUND));
            gate_step_next(cpu, o);
        }
        3 => {
            let odd = cpu.read32(o + G_TIME) & 1 != 0;
            let add = if !odd {
                0
            } else if n == 0 {
                0x1_0000
            } else {
                0xffff_0000
            };
            gate_part_set(cpu, o, n, G_TO[n], add);
            let t = cpu.read32(o + G_TIME).wrapping_add(1);
            cpu.write32(o + G_TIME, t);
            if t == 31 {
                cpu.write32(o + G_TIME, 0);
                gate_step_next(cpu, o);
            }
        }
        4 => {
            let t = cpu.read32(o + G_TIME).wrapping_add(1);
            cpu.write32(o + G_TIME, t);
            if t != 30 {
                return;
            }
            let mut flags = if cpu.read32(CAMERA_Y) as i32 > 0 { 2 } else { 0 };
            if n != 0 {
                flags |= 1;
            }
            let made = cpu.call(0x0608_4edc, &[o, flags]);
            if made != 0 {
                nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 75));
            }
            gate_bytes_clear(cpu, o);
            gate_step_next(cpu, o);
        }
        5 => {
            let t = cpu.read32(o + G_TIME).wrapping_add(1);
            cpu.write32(o + G_TIME, t);
            if t == 90 {
                nested!(cpu, 0x0608_38c8, gate_next_entry(cpu, o));
            }
        }
        _ => {}
    }
}

/// The gate's steps for its left part.
pub fn gate_steps_left(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_3494, gate_steps_b(cpu, o, 0));
}

/// The gate's steps for its right part.
pub fn gate_steps_right(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_3494, gate_steps_b(cpu, o, 1));
}

/// A gate's first step: at step 0 and counter 0 the side it is at (+0x7e) is chosen by chance: the
/// first (goals 0x280000 and -0x700000) or the second (0x700000 and -0x280000); then the steps for
/// that side.
pub fn gate_steps_random(cpu: &mut Cpu, o: u32) {
    if cpu.read8(o + G_STEP) == 0 && cpu.read32(o + G_TIME) == 0 {
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0));
        if r & 1 == 0 {
            cpu.write32(o + G_TO[0], 0x28_0000);
            cpu.write32(o + G_TO[1], 0xff90_0000);
            cpu.write8(o + G_SIDE, 0);
        } else {
            cpu.write32(o + G_TO[0], 0x70_0000);
            cpu.write32(o + G_TO[1], 0xffd8_0000);
            cpu.write8(o + G_SIDE, 1);
        }
    }
    let side = cpu.read8(o + G_SIDE) as i8 as i32 as u32;
    nested!(cpu, 0x0608_3494, gate_steps_b(cpu, o, side));
}

/// A gate's script's next entry (16 bytes at its place +0xa4: the move at +0, the linked object's
/// step at +1 (-1: unchanged), the bytes +0x74 and +0x75 at +2 and +3 (-1: unchanged), its wait at +4,
/// its goals at +8 and +12): a first byte of -1 ends the script (the function at 0x06083110).
/// Taken: the move, the wait, the linked object's step (and its +0x11 cleared), the places the
/// parts have, the goals, the step and counter 0, the bytes (their counters too), the place on 16.
pub fn gate_next_entry(cpu: &mut Cpu, o: u32) {
    let e = cpu.read32(o + G_SCRIPT);
    let mv = cpu.read8(e) as i8;
    if mv == -1 {
        cpu.call(0x0608_3110, &[o]);
        return;
    }
    cpu.write8(o + G_MOVE, mv as u8);
    let wait = cpu.read32(e + 4);
    cpu.write32(o + G_WAIT, wait);
    let link = cpu.read32(o + G_LINK);
    if link != 0 {
        let e = cpu.read32(o + G_SCRIPT);
        let step = cpu.read8(e + 1);
        if step as i8 != -1 {
            cpu.write8(link + 0x10, step);
            let link = cpu.read32(o + G_LINK);
            cpu.write8(link + 0x11, 0);
        }
    }
    for n in 0..2 {
        if cpu.read32(o + G_PARTS[n]) != 0 {
            let part = cpu.read32(o + G_PARTS[n]);
            let v = cpu.read32(part + PART_AT);
            cpu.write32(o + G_AT[n], v);
        }
    }
    let e = cpu.read32(o + G_SCRIPT);
    let (a, b) = (cpu.read32(e + 8), cpu.read32(e + 12));
    cpu.write32(o + G_TO[0], a);
    cpu.write32(o + G_TO[1], b);
    cpu.write8(o + G_STEP, 0);
    cpu.write32(o + G_TIME, 0);
    let e = cpu.read32(o + G_SCRIPT);
    let byte = cpu.read8(e + 2);
    if byte as i8 != -1 {
        cpu.write8(o + G_BYTE_A, byte);
        cpu.write32(o + 0x94, 0);
    }
    let e = cpu.read32(o + G_SCRIPT);
    let byte = cpu.read8(e + 3);
    if byte as i8 != -1 {
        cpu.write8(o + G_BYTE_B, byte);
        cpu.write32(o + P_TICKS, 0);
        cpu.write32(o + P_STEP, 0);
        cpu.write32(o + P_CYCLE, 0);
    }
    let e = cpu.read32(o + G_SCRIPT);
    cpu.write32(o + G_SCRIPT, e.wrapping_add(16));
}

/// The camera's height kept in an object (+0xb4).
pub fn note_camera_y(cpu: &mut Cpu, o: u32) {
    let y = cpu.read32(CAMERA_Y);
    cpu.write32(o + 0xb4, y);
}

// ---- the gate boss and its markers ---------------------------------------------------------------

/// The gate boss (a gate: see above): its step (+0x71), its life (+0x50), the leader pointer the
/// markers follow, the table of its moves, tables of the functions its two bytes call, where it
/// was seen on the screen (the middle of two points converted) and the camera's place for it
/// (+0xa8, +0xac, +0xb0).
const B_STEP: u32 = 0x71;
const B_LIFE: u32 = 0x50;
const B_LIFE_AT_START: u32 = 700_000;
const B_HIT: u32 = 0x0608_8034;
const B_SCRIPTS: u32 = 0x0608_8a04;
const B_MOVES: u32 = 0x0608_8364;
const B_BYTE_A_MOVES: u32 = 0x0608_8340;
const B_BYTE_B_MOVES: u32 = 0x0608_834c;
const B_POINTS: u32 = 0x0608_8a24;
const B_OUT: u32 = 0x0608_a310;
const B_FAR_POINT: u32 = 0x0608_8a18;
const B_MIDDLE: u32 = 0x0608_9ee0;
const B_VIEW_WORD: u32 = 0x0607_476a;
const B_KILL_SCORE: u32 = 10_000;
/// The score awarded in the stage's counters (a long and its total).
const B_KILLS: u32 = 0x0607_46f0;
const B_KILLS_TOTAL: u32 = 0x0607_46f4;

/// The gate boss's behaviour: the camera's height kept; step 0 made (its camera mover linked, the
/// hit set, scales 1, life 700000, not faded, its script by the stage's phase, its first entry,
/// its life gauge) then 1: while its life is above 0 the gauge shown, the function of its move
/// (+0x73) run, the drones' animation, the functions of its two bytes, its wait counted down (at
/// 0 the script's next entry); at 0 the kill scored (10000 to the weapon's shooter) and the
/// function at 0x06083110. Then drawn: the camera's matrix, two points converted, and the
/// view's word kept, a far point converted and the middle of two of the points (x, then y) kept
/// for the markers.
pub fn gate_boss_behaviour(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_39cc, note_camera_y(cpu, o));
    let step = cpu.read8(o + B_STEP) as i8;
    if step == 0 {
        let mover = nested!(cpu, 0x0608_2b7c, camera_mover_set(cpu));
        cpu.write32(o + G_LINK, mover);
        cpu.write32(o + HIT, B_HIT);
        cpu.write32(o + 0x24, ONE);
        cpu.write32(o + 0x20, ONE);
        cpu.write32(o + B_LIFE, B_LIFE_AT_START);
        let flags = cpu.read32(o + FLAGS) & !FADED;
        cpu.write32(o + FLAGS, flags);
        let phase = cpu.read32(STAGE_PHASE);
        let script = cpu.read32(B_SCRIPTS.wrapping_add(phase << 2));
        cpu.write32(o + G_SCRIPT, script);
        nested!(cpu, 0x0608_38c8, gate_next_entry(cpu, o));
        nested!(cpu, 0x0604_64d8, hud::boss_gauge_set(cpu, B_LIFE_AT_START));
        let n = cpu.read8(o + B_STEP).wrapping_add(1);
        cpu.write8(o + B_STEP, n);
    }
    if step == 0 || step == 1 {
        let life = cpu.read32(o + B_LIFE);
        if life as i32 > 0 {
            nested!(cpu, 0x0604_64b8, hud::boss_gauge_life(cpu, life));
            let mv = cpu.read8(o + G_MOVE) as i8 as i32 as u32;
            let f = cpu.read32(B_MOVES.wrapping_add(mv << 2));
            cpu.call_returning(0x0608_3aae, f, &[o]);
            nested!(cpu, 0x0608_2c10, animation_step(cpu, o));
            let a = cpu.read8(o + G_BYTE_A) as i8 as i32 as u32;
            let f = cpu.read32(B_BYTE_A_MOVES.wrapping_add(a << 2));
            cpu.call_returning(0x0608_3ac4, f, &[o]);
            let b = cpu.read8(o + G_BYTE_B) as i8 as i32 as u32;
            let f = cpu.read32(B_BYTE_B_MOVES.wrapping_add(b << 2));
            cpu.call_returning(0x0608_3ad4, f, &[o]);
            let n = cpu.read32(o + G_WAIT).wrapping_sub(1);
            cpu.write32(o + G_WAIT, n);
            if n == 0 {
                nested!(cpu, 0x0608_38c8, gate_next_entry(cpu, o));
            }
        } else {
            cpu.write32(B_KILLS_TOTAL, 100_000);
            let n = cpu.read32(B_KILLS).wrapping_add(60);
            cpu.write32(B_KILLS, n);
            let hit = cpu.read32(o + 0x40);
            let shooter = cpu.read8(hit + 0x32) as i8 as i32 as u32;
            let weapon = cpu.read8(hit + 0x55) as i8 as i32 as u32;
            nested!(cpu, 0x0604_3108, hits::score_kill(cpu, shooter, weapon, o, B_KILL_SCORE));
            cpu.call(0x0608_3110, &[o]);
            return;
        }
    }
    let [x, y, z] = [0xa8, 0xac, 0xb0].map(|k| cpu.read32(o + k));
    nested!(cpu, 0x0608_201c, camera_matrix(cpu, x, y, z));
    nested!(cpu, 0x0606_71cc, sgl::sl_push_matrix(cpu));
    nested!(cpu, 0x0606_db7c, sgl::sl_load_matrix(cpu, CAMERA_MATRIX));
    nested!(cpu, 0x0606_750c, sgl::sl_convert(cpu, B_POINTS, B_OUT));
    nested!(cpu, 0x0606_750c, sgl::sl_convert(cpu, B_POINTS + 12, B_OUT + 8));
    nested!(cpu, 0x0606_7200, sgl::sl_pop_matrix(cpu));
    let w = cpu.read16(B_VIEW_WORD);
    cpu.write16(o + 0x30, w);
    nested!(cpu, 0x0606_71cc, sgl::sl_push_matrix(cpu));
    nested!(cpu, 0x0606_db7c, sgl::sl_load_matrix(cpu, SCENERY_MATRIX));
    nested!(cpu, 0x0606_750c, sgl::sl_convert(cpu, B_FAR_POINT, o + 0x14));
    nested!(cpu, 0x0606_7200, sgl::sl_pop_matrix(cpu));
    let (a, b) = (cpu.read32(0x0608_a290), cpu.read32(B_OUT + 8));
    cpu.write32(B_MIDDLE, (a.wrapping_add(b) as i32 >> 1) as u32);
    let (a, b) = (cpu.read32(0x0608_a290 + 0x40), cpu.read32(B_OUT));
    cpu.write32(B_MIDDLE + 4, (a.wrapping_add(b) as i32 >> 1) as u32);
}

/// A marker (an object, +0x3c a byte: the set of points it shows): it ends when the leader (the
/// object at 0x06089edc) is past mode 111, and while the leader's +0x70 is 3 converts the set's
/// first point and its eight others (12 bytes each) through the camera's matrix at its place
/// (+0x40, +0x44).
const LEADER_AT: u32 = 0x0608_9edc;
const MARKER_SET: u32 = 0x3c;
const MARKER_POINT: u32 = 0x0608_8f00;
const MARKER_POINTS: u32 = 0x0608_8f18;
const MARKER_OUT: u32 = 0x0608_a360;
const MARKER_OUTS: u32 = 0x0608_a290;
pub fn marker_behaviour(cpu: &mut Cpu, o: u32) {
    let leader = cpu.read32(LEADER_AT);
    if cpu.read8(leader + 3) as i8 > 111 {
        cpu.write8(o + 3, 112);
    }
    let leader = cpu.read32(LEADER_AT);
    if cpu.read8(leader + 0x70) as i8 != 3 {
        return;
    }
    nested!(cpu, 0x0606_71cc, sgl::sl_push_matrix(cpu));
    nested!(cpu, 0x0606_db7c, sgl::sl_load_matrix(cpu, CAMERA_MATRIX));
    let (x, y) = (cpu.read32(o + 0x40), cpu.read32(o + 0x44));
    nested!(cpu, 0x0606_721c, sgl::sl_translate(cpu, x, y, 0));
    let k = cpu.read8(o + MARKER_SET) as i8 as i32 as u32;
    nested!(cpu, 0x0606_750c, sgl::sl_convert(cpu, MARKER_POINT + 12 * k, MARKER_OUT + 8 * k));
    for i in 0..8 {
        let p = MARKER_POINTS + 12 * i + 96 * k;
        let out = MARKER_OUTS + 8 * i + 64 * k;
        nested!(cpu, 0x0606_750c, sgl::sl_convert(cpu, p, out));
    }
    nested!(cpu, 0x0606_7200, sgl::sl_pop_matrix(cpu));
}

/// A marker's draw (`s` its sprite: its place +0x30, +0x34 and its set +0x2c): the scenery's model's
/// polygons of each of the set's entries (8 bytes: a place across, then the frame, until a
/// frame of -1) that are on the screen, each put at (0, its place, -0x100000) from the sprite's
/// place.
const MARKER_SETS: u32 = 0x0608_9088;
pub fn marker_draw(cpu: &mut Cpu, s: u32) {
    nested!(cpu, 0x0606_713c, sgl::sl_cur_window(cpu, 0));
    nested!(cpu, 0x0606_71cc, sgl::sl_push_matrix(cpu));
    nested!(cpu, 0x0606_db7c, sgl::sl_load_matrix(cpu, SCENERY_MATRIX));
    let (x, y) = (cpu.read32(s + 0x30), cpu.read32(s + 0x34));
    nested!(cpu, 0x0606_721c, sgl::sl_translate(cpu, x, y, 0));
    let at = cpu.stack();
    cpu.write32(at, 0);
    cpu.write32(at + 8, 0xfff0_0000);
    let k = cpu.read8(s + 0x2c) as i8 as i32 as u32;
    let mut e = cpu.read32(MARKER_SETS.wrapping_add(k << 2));
    if cpu.read32(e + 4) as i32 != -1 {
        loop {
            nested!(cpu, 0x0606_71cc, sgl::sl_push_matrix(cpu));
            let v = cpu.read32(e);
            cpu.write32(at + 4, v);
            let seen = nested!(cpu, 0x0606_e9fc, sgl::sl_check_on_screen(cpu, at, 0x80_0000)) as i32;
            if seen > 0 {
                let [px, py, pz] = [0, 4, 8].map(|j| cpu.read32(at + j));
                nested!(cpu, 0x0606_721c, sgl::sl_translate(cpu, px, py, pz));
                let file = cpu.read32(KINDS + K_SIZE * SCENERY_MODEL);
                let frames = cpu.read32(file + MODEL_FRAMES);
                let frame = cpu.read32(e + 4);
                let polygons = cpu.read32(frames.wrapping_add(frame << 2));
                let pd = cpu.read32(polygons);
                nested!(cpu, 0x0606_745c, sgl::sl_put_polygon(cpu, pd));
            }
            e = e.wrapping_add(8);
            nested!(cpu, 0x0606_7200, sgl::sl_pop_matrix(cpu));
            if cpu.read32(e + 4) as i32 == -1 {
                break;
            }
        }
    }
    nested!(cpu, 0x0606_7200, sgl::sl_pop_matrix(cpu));
    nested!(cpu, 0x0606_713c, sgl::sl_cur_window(cpu, 1));
}

/// A marker made (list 2, 0x48 bytes, the plain exec) after `p`'s place (+0x14, +0x18) and
/// +0x32, its set `set` (+0x3c), its behaviour and draw above, flags 0xc00000 (not culled, not
/// lit). The marker, or 0.
pub fn marker_set(cpu: &mut Cpu, p: u32, set: u32) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0x48)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, PLAIN_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + FLAGS));
    cpu.write32(o + BEHAVIOUR, 0x0608_3db4);
    let flags = cpu.read32(o + FLAGS) | 0x00c0_0000;
    cpu.write32(o + FLAGS, flags);
    cpu.write32(o + 0x34, 0x0608_3e9c);
    let (x, y) = (cpu.read32(p + 0x14), cpu.read32(p + 0x18));
    cpu.write32(o + 0x14, x);
    cpu.write32(o + 0x18, y);
    cpu.write32(o + 0x1c, 0);
    cpu.write8(o + 0x33, 0);
    let b = cpu.read8(p + 0x32);
    cpu.write8(o + 0x32, b);
    cpu.write8(o + MARKER_SET, set as u8);
    o
}
const PLAIN_EXEC: u32 = 0x0604_dcb4;

/// The first colour of the kind `k`'s table entry `j` (8 bytes each, the table at +4 of the kind's
/// 16): the byte at its place added to the kind's word at +8, kept in two words (0x06089102 and
/// 0x0608910e).
pub fn kind_colour_pair(cpu: &mut Cpu, k: u32, j: u32) {
    let k = k as u8 as i8 as i32 as u32;
    let j = j as u8 as u32;
    let kind = KINDS.wrapping_add(k << 4);
    let table = cpu.read32(kind + 4);
    let entry = cpu.read32(table.wrapping_add(j << 3));
    let base = cpu.read8(entry) as u32;
    let first = cpu.read16(kind + 8) as i16 as i32 as u32;
    let v = first.wrapping_add(base) as u16;
    cpu.write16(0x0608_9102, v);
    cpu.write16(0x0608_910e, v);
}

/// A colour kept in two words (0x06089106 and 0x06089112).
pub fn colour_pair_set(cpu: &mut Cpu, v: u32) {
    cpu.write16(0x0608_9106, v as u16);
    cpu.write16(0x0608_9112, v as u16);
}

/// Whether (x, y) is off the screen's play field (more than 0xd00000 across or 0x900000 down).
pub fn off_field(cpu: &mut Cpu, x: u32, y: u32) -> u32 {
    let _ = cpu;
    let abs = |v: u32| if (v as i32) < 0 { v.wrapping_neg() as i32 } else { v as i32 };
    (abs(x) > 0xd0_0000 || abs(y) > 0x90_0000) as u32
}

/// A spark (an object after a parent, +0x3c): its place (+0x14, +0x18) and turn (+0x30) the parent's
/// (+0x8c, +0x90, +0x30); it ends when the parent is past mode 111 and has +0x6f, or when it has
/// none and its +0x40 is not above 0 (the parent's end otherwise lets it go); its +0x44 counts down,
/// then its +0x40 falls by 0xc00 while above 0; on frames odd (flag 0x100: sprite colours at 32,
/// else 48) its colours kept, the table's entry (+0x88) chosen by the frame's bit 0 and its
/// +0x2f by bit 1; its scales 1 plus chance, its speed (the function at 0x0608409c) by +0x40
/// times a distance by chance.
pub fn spark_behaviour(cpu: &mut Cpu, o: u32) {
    let parent = cpu.read32(o + 0x3c);
    if parent != 0 {
        let (x, y) = (cpu.read32(parent + 0x8c), cpu.read32(parent + 0x90));
        cpu.write32(o + 0x14, x);
        cpu.write32(o + 0x18, y);
        let w = cpu.read16(parent + 0x30);
        cpu.write16(o + 0x30, w);
        if cpu.read8(parent + 3) as i8 > 111 {
            let end = cpu.read8(parent + 0x6f);
            if end != 0 {
                cpu.write8(o + 3, 112);
                return;
            }
            cpu.write32(o + 0x3c, end as u32);
        }
    } else if cpu.read32(o + 0x40) as i32 <= 0 {
        cpu.write8(o + 3, 112);
        return;
    }
    let c = cpu.read32(o + 0x44);
    if c != 0 {
        cpu.write32(o + 0x44, c.wrapping_sub(1));
    } else {
        let v = cpu.read32(o + 0x40);
        if v as i32 > 0 {
            cpu.write32(o + 0x40, v.wrapping_sub(0xc00));
        }
    }
    let frame = cpu.read32(0x0600_ebd4);
    let add = if frame & 1 != 0 { 32 } else { 48 };
    let mut flags = cpu.read32(o + FLAGS);
    if frame & 1 != 0 {
        flags |= 0x100;
    } else {
        flags &= !0x100;
    }
    cpu.write32(o + FLAGS, flags);
    let v = (cpu.read16(CHAR_BASE) as i16 as i32 as u32).wrapping_add(add);
    cpu.write16(o + 0x2a, v as u16);
    cpu.write16(o + 0x28, v as u16);
    nested!(cpu, 0x0608_4050, colour_pair_set(cpu, v & 0xffff));
    let frame = cpu.read32(0x0600_ebd4);
    let table = cpu.read32(0x0608_9118 + ((frame & 1) << 2));
    cpu.write32(o + 0x88, table);
    cpu.write8(o + 0x2f, if frame & 2 != 0 { 0xc3 } else { 0xc4 });
    let (far, mut scale) = if cpu.read32(o + FLAGS) & 0x100 != 0 {
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0)) & 0xffff;
        let extra = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, r, 0x4_0000));
        (0xc_8000u32.wrapping_add(extra), 0x1_0000u32)
    } else {
        (0xa_0000, 0xc000)
    };
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0)) & 0xffff;
    let extra = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, r, 0x4000));
    scale = scale.wrapping_add(extra);
    let at = cpu.read32(o + 0x40);
    let d = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, far, at));
    nested!(cpu, 0x0608_409c, spark_speed(cpu, o, d));
    cpu.write32(o + 0x24, scale);
    cpu.write32(o + 0x20, scale);
}

/// A spark's speed `v` along its turn (a word at +0x30): its steps (0x200000 times the turn's sine
/// and cosine) taken from its place until it is off the field (the places and the dots they
/// took (-0x200000 each) kept: +0x48 v, 0, +0x54 -v, 0, +0x60 v, +0x64 the dots, +0x6c -v, +0x70
/// the dots).
pub fn spark_speed(cpu: &mut Cpu, o: u32, v: u32) {
    let s = cpu.stack();
    cpu.write32(s, v);
    let (mut x, mut y) = (cpu.read32(o + 0x14), cpu.read32(o + 0x18));
    let mut dots = 0u32;
    let angle = cpu.read16(o + 0x30) as i16 as i32 as u32;
    let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, angle));
    let dx = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, 0x20_0000, sin));
    cpu.write32(s + 4, dx);
    let angle = cpu.read16(o + 0x30) as i16 as i32 as u32;
    let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, angle));
    let dy = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, 0x20_0000, cos)).wrapping_neg();
    while nested!(cpu, 0x0608_406c, off_field(cpu, x, y)) == 0 {
        dots = dots.wrapping_add(0xffe0_0000);
        let dx = cpu.read32(s + 4);
        y = y.wrapping_add(dy);
        x = x.wrapping_add(dx);
    }
    let v = cpu.read32(s);
    cpu.write32(o + 0x48, v);
    cpu.write32(o + 0x4c, 0);
    cpu.write32(o + 0x54, v.wrapping_neg());
    cpu.write32(o + 0x58, 0);
    cpu.write32(o + 0x60, v);
    cpu.write32(o + 0x64, dots);
    cpu.write32(o + 0x6c, v.wrapping_neg());
    cpu.write32(o + 0x70, dots);
}
