//! STG04.BIN's code (fingerprint 5c18364ee98aacb1), which game mode 10 runs in the overlay window:
//! its timeline's events, its drones (the swoopers and the gunners) and their launchers.

use super::actors::{E_ANGLE, E_COLOUR, E_POSITION, E_SCALES, E_SCRIPT, E_SPEED};
use super::globals::{GAME_FLAGS, GAME_FRAMES, HELD};
use super::{actors, aim, bullets, files, gouraud, hits, kinds, math, models, nested, objects, palette, results, screen, sgl, sound, sprites, text, Cpu};

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

/// A drone's move (inline in both behaviours): its position moved by its speed along its heading
/// (x by the sine, y back by the cosine); its frame (+0x2f) a step on every other game frame,
/// round 32.
fn drone_move(cpu: &mut Cpu, o: u32) {
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
    if cpu.read32(GAME_FRAMES) & 1 != 0 {
        let frame = cpu.read8(o + FRAME).wrapping_add(1);
        cpu.write8(o + FRAME, frame);
        if frame == 32 {
            cpu.write8(o + FRAME, 0);
        }
    }
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
