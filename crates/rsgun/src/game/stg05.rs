//! STG05.BIN's code, which the last stage's mode (game mode 11) runs in the overlay window
//! (fingerprint 2adefae5540dd556): its mode's code, its enemies and scenery, its scrolls.

use super::globals::{GAME_FRAMES, LOOPS};
use super::{actors, aim, bg, player, bullets, camera, clib, dma, effects, explosions, files, gouraud, hits, hud, kinds, math, models, nested, objects, palette, parts, results, ribbons, rings, screen, sgl, sound, sprites, states, text, timeline, Cpu};

/// The director's slots the mode uses (states.rs's, as STG01's mode uses them): its step, the
/// load (a local written back each time), the settings kept (the buffer +0x6c, the room's top
/// +0x70, VDP1's next character place +0x74, its next character and colour +0x78 and +0x7c), the
/// demo's input (+0x54), the mode's music (+0xa4).
const MODE_STEP: u32 = 0x64;
const LOAD: u32 = 0x68;
const KEPT: u32 = 0x6c;
const PLACE: u32 = 0x70;
const VDP1_PLACE: u32 = 0x74;
const NEXT_CHAR: u32 = 0x78;
const NEXT_COLOUR: u32 = 0x7c;
const DEMO_INPUT: u32 = 0x54;
const MUSIC: u32 = 0xa4;
const ROOM: u32 = 0x4_4040;
const ROOM_TOP: u32 = 0x0607_4050;
const NEXT_RATE: u32 = 0x0603_2e40;
const STAGE_RATE: u32 = 0x2_0000;
/// The stages' screen data files (the stages' SCRDAT files): this stage's one, the
/// fifth entry of the table.
const SCREEN_FILE: u32 = 0x0607_35b0 + 0x10;
/// The sound data files loaded at once (sound::load_sound_file), the defaults' set-up after the
/// first.
const SOUND_FILES: [u32; 5] = [2, 0x13, 0x12, 0x2e, 0x2d];
/// The one model kind put in place (kinds.rs), and the kinds' table.
const MODEL_KIND: u32 = 0x21;
const KINDS: u32 = 0x0607_41f0;
const K_SIZE: u32 = 16;
/// The stage's scroll table, its timeline, its stage sound effects, the mode after it.
const SCROLLS: u32 = 0x0609_2718;
const EVENTS: u32 = 0x0609_1184;
const SOUND_EFFECTS: u32 = 0x0609_3484;
const STAGE_SE_TABLE: u32 = 0x0600_edd4;
const NEXT_MODE: u32 = 0x0600_ed70;
const AFTER_PLAY: u8 = 0;

/// The mode's code (the director calls it each frame until it gives 1): the settings kept, the
/// sound data loaded, the stage's one model kind read into the buffer and put in place, the
/// screen data file loaded into the buffer; once DMA is idle the screen started; the timeline
/// made, the settings given back; 1.
pub fn stg05_mode(cpu: &mut Cpu, game: u32) -> u32 {
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
                    cpu.write16(game + MUSIC, 8);
                }
            }
            let top = cpu.read32(ROOM_TOP);
            cpu.write32(game + PLACE, top);
            cpu.write8(game + MODE_STEP, 9);
        }
        9 => {
            let buffer = cpu.read32(game + KEPT);
            nested!(cpu, 0x0604_58cc, kinds::model_kind_read(cpu, MODEL_KIND, buffer));
            let vram = cpu.read32(game + VDP1_PLACE);
            let first = cpu.read16(game + NEXT_CHAR) as i16 as i32 as u32;
            let colour = cpu.read16(game + NEXT_COLOUR) as u32;
            let next = nested!(cpu, 0x0604_5950, kinds::model_kind_install(cpu, MODEL_KIND, vram, first, colour));
            cpu.write32(game + VDP1_PLACE, next);
            let file = cpu.read32(KINDS + K_SIZE * MODEL_KIND);
            let chars = cpu.read16(game + NEXT_CHAR).wrapping_add(cpu.read16(file + 6));
            cpu.write16(game + NEXT_CHAR, chars);
            let past = cpu.read32(file);
            cpu.write32(game + KEPT, past);
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
                let room = nested!(cpu, 0x0605_fe78, states::stage_table(cpu, 4));
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
            cpu.write8(NEXT_MODE, AFTER_PLAY);
            cpu.write32(game + DEMO_INPUT, 0);
            states::give_back_mode_settings(cpu, game);
            cpu.write8(game + MODE_STEP, 15);
        }
        15 => done = 1,
        _ => {}
    }
    cpu.write32(game + LOAD, load);
    done
}

// ---- the skeleton's poses ------------------------------------------------------------------------

/// A pose animation (the first argument of the pose step): how many joints it moves (+0x10, a
/// word), the joints' angles now (+0x14, words) and the change a frame toward the pose in
/// view (+0x18), the table of poses (+0x1c; each a table of words), the script (+0x20, words
/// by 3: a pose number with its end flags, the flags' word with the frames the pose takes, and
/// the frames to wait after), its place in the script (+0x24), the frames left (+0x26) and the
/// flags of the script's step (+0x28).
const POSE_COUNT: u32 = 0x10;
const POSE_ANGLES: u32 = 0x14;
const POSE_RATES: u32 = 0x18;
const POSE_TABLE: u32 = 0x1c;
const POSE_SCRIPT: u32 = 0x20;
const POSE_AT: u32 = 0x24;
const POSE_FRAMES: u32 = 0x26;
const POSE_FLAGS: u32 = 0x28;
/// The script's flags: the step's pose is mirrored (bit 15 of the frames' word), the script
/// loops to its start (0x2000), it ends (0x4000).
const SCRIPT_MIRROR: u32 = 0x8000;
const SCRIPT_LOOP: u32 = 0x2000;
const SCRIPT_END: u32 = 0x4000;
/// The figure a pose animation moves (the second argument): whether its poses are mirrored
/// (+0x278, a byte), the angles aimed at (+0x98), the angles it has (+0x158), the rates they
/// turn at (+0x1b8) and those rates' goals (+0x218), each a word a joint.
const FIGURE_MIRRORED: u32 = 0x278;
const FIGURE_AIMS: u32 = 0x98;
const FIGURE_ANGLES: u32 = 0x158;
const FIGURE_RATES: u32 = 0x1b8;
const FIGURE_GOALS: u32 = 0x218;
/// The joints in a pose (a pose's words are by joint: 12 of the first limbs, each triple
/// (x, y, z) with y's and x's sign turned in the mirror, then 36 which swap sides: by 9 the
/// other limb's).
const JOINTS: u32 = 48;
const TURN_TOWARD: u32 = 0x0605_70d8;
static TURN_TOWARD_PROLOGUE: std::sync::OnceLock<super::Prologue> = std::sync::OnceLock::new();

/// A step of a figure's pose animation `anim` (`figure`'s joints): while the script has not
/// ended (its flags' end bit clear) its flags are cleared and each joint's angle goes on by its rate; with no frames
/// left the next step is read (the place on by 3 past any word with its top bit set, which
/// is flags: they are kept, the place on a word, back to the start on a loop, and the step ends
/// the animation at once if they say so), the figure's mirror switched if the step says so, and
/// each joint's rate made: the angle to the pose's (mirrored: signs turned for some, the other
/// limb's for the rest) over the frames the step takes; then a frame less. Last each of the
/// figure's angles turns toward the angle it aims at, the angle's rate eased.
pub fn pose_step(cpu: &mut Cpu, anim: u32, figure: u32) {
    let frame = cpu.stack();
    cpu.write32(frame, anim);
    cpu.write32(frame + 4, figure);
    if cpu.read16(anim + POSE_FLAGS) as u32 & SCRIPT_END == 0 {
        cpu.write16(anim + POSE_FLAGS, 0);
        let (angles, rates) = (cpu.read32(anim + POSE_ANGLES), cpu.read32(anim + POSE_RATES));
        let joints = cpu.read16(anim + POSE_COUNT) as u32;
        for i in 0..joints {
            let a = cpu.read16(angles + 2 * i).wrapping_add(cpu.read16(rates + 2 * i));
            cpu.write16(angles + 2 * i, a);
        }
        if cpu.read16(anim + POSE_FRAMES) == 0 && !next_step(cpu, anim, figure, frame) {
            return;
        }
        let left = cpu.read16(anim + POSE_FRAMES).wrapping_sub(1);
        cpu.write16(anim + POSE_FRAMES, left);
    }
    let joints = cpu.read16(anim + POSE_COUNT) as u32;
    for i in 0..joints {
        let k = 2 * i;
        let to = cpu.read16(figure + FIGURE_AIMS + k) as u32;
        let from = cpu.read16(figure + FIGURE_ANGLES + k) as u32;
        let (rate, goal) = (figure + FIGURE_RATES + k, figure + FIGURE_GOALS + k);
        cpu.nested_with_stack(&TURN_TOWARD_PROLOGUE, TURN_TOWARD, &[0x8000, 6, 0x200], |cpu| {
            aim::turn_toward(cpu, rate, goal, from, to, 0x8000, 6, 0x200)
        });
        let angle = cpu.read16(figure + FIGURE_ANGLES + k).wrapping_add(cpu.read16(rate));
        cpu.write16(figure + FIGURE_ANGLES + k, angle);
    }
}

/// The script's word at `at`.
fn word(cpu: &mut Cpu, script: u32, at: u16) -> u16 {
    cpu.read16(script + 2 * at as u32)
}

/// Reads the script's next step and makes each joint's rate (see [`pose_step`]); false if the
/// script has ended the animation (the rest of the step is not done).
fn next_step(cpu: &mut Cpu, anim: u32, figure: u32, frame: u32) -> bool {
    let at = cpu.read16(anim + POSE_AT).wrapping_add(3);
    cpu.write16(anim + POSE_AT, at);
    let script = cpu.read32(anim + POSE_SCRIPT);
    // words with the top bit set are flags
    while ({ let at = cpu.read16(anim + POSE_AT); word(cpu, script, at) } as i16) < 0 {
        let at = cpu.read16(anim + POSE_AT);
        let flags = word(cpu, script, at);
        cpu.write16(anim + POSE_FLAGS, flags);
        cpu.write16(anim + POSE_AT, at.wrapping_add(1));
        if flags as u32 & SCRIPT_LOOP != 0 {
            cpu.write16(anim + POSE_AT, 0);
        }
        if cpu.read16(anim + POSE_FLAGS) as u32 & SCRIPT_END != 0 {
            return false;
        }
    }
    let at = cpu.read16(anim + POSE_AT);
    if word(cpu, script, at.wrapping_add(1)) as u32 & SCRIPT_MIRROR != 0 {
        let m = cpu.read8(figure + FIGURE_MIRRORED) ^ 1;
        cpu.write8(figure + FIGURE_MIRRORED, m);
    }
    let at = cpu.read16(anim + POSE_AT);
    let number = word(cpu, script, at) as i16 as i32;
    let table = cpu.read32(anim + POSE_TABLE);
    let pose = cpu.read32(table.wrapping_add((number * 4) as u32));
    let frames = word(cpu, script, at.wrapping_add(1)) & 0x7fff;
    cpu.write16(anim + POSE_FRAMES, frames);
    let (angles, rates) = (cpu.read32(anim + POSE_ANGLES), cpu.read32(anim + POSE_RATES));
    let frames = frames as i16 as i32 as u32;
    let mirrored = cpu.read8(figure + FIGURE_MIRRORED) != 0;
    let (sign, swap) = if mirrored { (-1i32, 9i32) } else { (1, 0) };
    let other = -swap;
    cpu.write32(frame + 8, sign as u32);
    cpu.write32(frame + 12, other as u32);
    cpu.write32(frame + 16, swap as u32);
    for i in 0..JOINTS {
        let (from, turned) = match i {
            0..=11 => (i as i32, i % 3 != 2),
            12..=20 | 30..=38 => (i as i32 + swap, false),
            _ => (i as i32 + other, false),
        };
        if i == 12 {
            cpu.write32(frame + 20, (2 * swap) as u32);
        }
        if i == 21 {
            cpu.write32(frame + 24, (2 * other) as u32);
        }
        let from = from as u32;
        let to = cpu.read16(pose + 2 * from);
        let to = if turned { (to as i16 as i32).wrapping_mul(sign) as u32 & 0xffff } else { to as u32 };
        let now = cpu.read16(angles + 2 * i) as u32;
        let d = nested!(cpu, 0x0605_6580, aim::angle_between(cpu, now, to)) as i16 as i32 as u32;
        let rate = nested!(cpu, 0x0600_6a78, math::quotient(cpu, d, frames));
        cpu.write16(rates + 2 * i, rate as u16);
    }
    let at = cpu.read16(anim + POSE_AT);
    let more = word(cpu, script, at.wrapping_add(2));
    let frames = cpu.read16(anim + POSE_FRAMES).wrapping_add(more);
    cpu.write16(anim + POSE_FRAMES, frames);
    true
}

// ---- the fireworks' sparks (object fields shared by the actors below) -------------------------------

/// An object's fields: its flags, position (x, y, z), scales (x +0x20, y +0x24), the turn word
/// +0x30, its sprite's frame (+0x2f), the step byte (+0x3c), its behaviour (+0x38) and mode (+3).
const FLAGS: u32 = 0x10;
const POSITION: u32 = 0x14;
const SCALE_X: u32 = 0x20;
const SCALE_Y: u32 = 0x24;
const TURN: u32 = 0x30;
const FRAME: u32 = 0x2f;
const BEHAVIOUR: u32 = 0x38;
const STEP: u32 = 0x3c;
const MODE: u32 = 3;
/// The mode that ends an object.
const ENDING: u8 = 0x70;
const ACTOR_EXEC: u32 = 0x0604_d780;

/// A piece of wreckage falling away (its scales +0x20 and +0x24 shrinking 0x200 a frame, its sprite
/// frame stepping through 32, its turn word turning by the speed's low word, the speed +0x5c eased
/// toward +0x60 by 0x100, its y falling faster by 0x100 a frame): it ends once it is 0x80 dots
/// above, or a scale is gone.
pub fn falling_piece(cpu: &mut Cpu, o: u32) {
    let sx = cpu.read32(o + SCALE_X).wrapping_add(-0x200i32 as u32);
    cpu.write32(o + SCALE_X, sx);
    let sy = cpu.read32(o + SCALE_Y).wrapping_add(-0x200i32 as u32);
    cpu.write32(o + SCALE_Y, sy);
    let y = cpu.read32(o + POSITION + 4) as i32;
    if y < -0x80_0000 || sx as i32 <= 0 || sy as i32 <= 0 {
        cpu.write8(o + MODE, ENDING);
        return;
    }
    let frame = cpu.read8(o + FRAME).wrapping_add(1);
    cpu.write8(o + FRAME, if frame > 31 { 0 } else { frame });
    let (spin, spin_to) = (cpu.read32(o + 0x5c), cpu.read32(o + 0x60));
    let spin = nested!(cpu, 0x0604_6168, actors::approach(cpu, spin, spin_to, 0x100));
    cpu.write32(o + 0x5c, spin);
    let turn = cpu.read16(o + TURN).wrapping_add(cpu.read16(o + 0x5e));
    cpu.write16(o + TURN, turn);
    let fall = cpu.read32(o + 0x58).wrapping_sub(0x100);
    cpu.write32(o + 0x58, fall);
    let y = cpu.read32(o + POSITION + 4).wrapping_add(fall);
    cpu.write32(o + POSITION + 4, y);
}

/// A piece of wreckage made (list 3, 100 bytes, plain_exec's): high up at a random place across
/// 0x80 dots in x, 0x80 dots above in z up to 0x80 dots below, its sprite scaled by 0x8000-0xbfff
/// and 0xe000-0x11fff, a random turn, a random fall (0x20000 at most, down) and spin (0x800 at
/// most).
pub fn falling_piece_set(cpu: &mut Cpu) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 100)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 3, blocks, PLAIN_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + SPRITE));
    cpu.write32(o + BEHAVIOUR, FALLING_PIECE);
    let x = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x80_0000));
    cpu.write32(o + POSITION, x);
    cpu.write32(o + POSITION + 4, 0x80_0000);
    let z = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x80_0000)).wrapping_sub(0x80_0000);
    cpu.write32(o + POSITION + 8, z);
    cpu.write32(o + FLAGS, 0x100);
    cpu.write8(o + SPRITE_KIND, 6);
    cpu.write8(o + FRAME, 0);
    cpu.write16(o + SPRITE_COLOUR, 0x7f0);
    let turn = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write16(o + TURN, turn as u16);
    let sx = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x4000)).wrapping_add(0x8000);
    cpu.write32(o + SCALE_X, sx);
    let sy = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x4000)).wrapping_add(0xe000);
    cpu.write32(o + SCALE_Y, sy);
    cpu.write8(o + BYTE_33, 0);
    cpu.write32(o + 0x54, 0);
    let fall = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x2_0000)) as i32;
    cpu.write32(o + 0x58, (-fall.wrapping_abs()) as u32);
    cpu.write32(o + 0x5c, 0);
    let spin = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x800));
    cpu.write32(o + 0x60, spin);
}

// ---- the glints ----------------------------------------------------------------------------------

/// A glint (list 2, 0x98 bytes, actor_exec's), a light that sits at a place turning until the flag
/// it watches is set: the flags word it watches (+0x6c) and the mask of the flag (+0x70, a word),
/// the table of the place (+0x74: x, y, z), a spare (+0x78), the count of frames it fades (+0x7c, a
/// byte), the turn it adds a frame (+0x8a, a word) and the player's number (+0x32).
const GLINT_WATCHED: u32 = 0x6c;
const GLINT_MASK: u32 = 0x70;
const GLINT_PLACE: u32 = 0x74;
const GLINT_SPARE: u32 = 0x78;
const GLINT_FADES: u32 = 0x7c;
const GLINT_TURN: u32 = 0x8a;
const GLINT_BEHAVIOUR: u32 = 0x0608_0c9c;
const GLINT_TRAIL: u32 = 0x0608_0c2c;
const GLINT_HIT_FRAMES: u32 = 0x0609_0fd4;
const PLAIN_EXEC: u32 = 0x0604_dcb4;
const FALLING_PIECE: u32 = 0x0608_09e4;
const SPRITE: u32 = 0x10;
const HIT: u32 = 0x3c;
const CHAR_BASE: u32 = 0x0607_41fc;
const SPRITE_KIND: u32 = 0x2e;
const SPRITE_COLOUR: u32 = 0x28;
const BYTE_33: u32 = 0x33;
const NUMBER: u32 = 0x32;

/// A glint made, at the place the table `place` gives. It does nothing if there is no room.
pub fn glint_set(cpu: &mut Cpu, watched: u32, mask: u32, place: u32, spare: u32, number: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0x98)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + SPRITE));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HIT));
    cpu.write32(o + BEHAVIOUR, GLINT_BEHAVIOUR);
    cpu.write32(o + FLAGS, 0x8_1500);
    cpu.write8(o + NUMBER, number as u8);
    cpu.write32(o + HIT, GLINT_HIT_FRAMES);
    cpu.write8(o + 0x54, 1);
    cpu.write8(o + 0x55, 5);
    cpu.write8(o + GLINT_FADES, 0);
    cpu.write32(o + GLINT_WATCHED, watched);
    cpu.write16(o + GLINT_MASK, mask as u16);
    cpu.write32(o + GLINT_PLACE, place);
    cpu.write32(o + GLINT_SPARE, spare);
    cpu.write16(o + 0x88, 0);
    cpu.write16(o + GLINT_TURN, 0x1000);
    cpu.write32(o + 0x8c, 0);
    cpu.write32(o + 0x90, 0);
    for k in [0, 4, 8] {
        let v = cpu.read32(place + k);
        cpu.write32(o + POSITION + k, v);
    }
    cpu.write8(o + FRAME, 0xd6);
    cpu.write8(o + SPRITE_KIND, 0);
    cpu.write16(o + SPRITE_COLOUR, 0x780);
    cpu.write16(o + 0x2a, 0x780);
    cpu.write8(o + BYTE_33, 10);
    cpu.write32(o + SCALE_X, 0);
    cpu.write32(o + SCALE_Y, 0);
}

/// Whether the flag a glint watches is set; if not, it goes on turning and is put back at its place.
pub fn glint_released(cpu: &mut Cpu, o: u32) -> u32 {
    let watched = cpu.read32(o + GLINT_WATCHED);
    let flags = cpu.read16(watched) & cpu.read16(o + GLINT_MASK);
    if flags != 0 {
        return 1;
    }
    let turn = cpu.read16(o + TURN).wrapping_add(cpu.read16(o + GLINT_TURN));
    cpu.write16(o + TURN, turn);
    let place = cpu.read32(o + GLINT_PLACE);
    for k in [0, 4, 8] {
        let v = cpu.read32(place + k);
        cpu.write32(o + POSITION + k, v);
    }
    0
}

/// A glint's fade: for the first 40 frames its sprite's frame jumps (45 less by 0-3: a flicker)
/// and its scales grow toward 0xc000 by 0x1000; after, they shrink to nothing (1 once gone).
pub fn glint_fade(cpu: &mut Cpu, o: u32) -> u32 {
    if cpu.read8(o + GLINT_FADES) <= 40 {
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
        cpu.write8(o + FRAME, ((r & 3) as u8).wrapping_sub(45));
        let s = cpu.read32(o + SCALE_X);
        let s = nested!(cpu, 0x0604_6168, actors::approach(cpu, s, 0xc000, 0x1000));
        cpu.write32(o + SCALE_X, s);
    } else {
        let s = cpu.read32(o + SCALE_X);
        let s = nested!(cpu, 0x0604_6168, actors::approach(cpu, s, 0, 0x1000));
        cpu.write32(o + SCALE_X, s);
        if s == 0 {
            return 1;
        }
    }
    let s = cpu.read32(o + SCALE_X);
    cpu.write32(o + SCALE_Y, s);
    0
}

/// A glint's trail piece's behaviour (a shrinking afterimage): the shrink, its colour offset
/// (+0x28) 0x10 more on every other of the first 13 frames, copied to +0x2a, and its flag 0x100 on
/// the even frames of the game.
pub fn glint_trail_exec(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0605_75e4, explosions::shrink_exec(cpu, o));
    let step = cpu.read8(o + STEP).wrapping_add(1);
    cpu.write8(o + STEP, step);
    if step <= 13 && step & 1 == 0 {
        let c = cpu.read16(o + SPRITE_COLOUR).wrapping_add(0x10);
        cpu.write16(o + SPRITE_COLOUR, c);
    }
    let c = cpu.read16(o + SPRITE_COLOUR);
    cpu.write16(o + 0x2a, c);
    let flags = cpu.read32(o + FLAGS);
    let flags = if cpu.read32(GAME_FRAMES) & 1 == 0 { flags | 0x100 } else { flags & !0x100 };
    cpu.write32(o + FLAGS, flags);
}

/// A glint's behaviour: ended (mode 0x70) once the flag it watches is set or it has faded; else
/// the count of frames on, its z 0xb0 dots less, a trail piece left (afterimage of 30 frames, its
/// behaviour the trail's, flagged 0x1400), and the flag 0x80000 set while it is 0x20 dots or more
/// from z = 0.
pub fn glint_behaviour(cpu: &mut Cpu, o: u32) {
    if nested!(cpu, 0x0608_0b74, glint_released(cpu, o)) as u8 != 0 || nested!(cpu, 0x0608_0bc0, glint_fade(cpu, o)) as u8 != 0 {
        cpu.write8(o + MODE, ENDING);
        return;
    }
    let n = cpu.read8(o + GLINT_FADES).wrapping_add(1);
    cpu.write8(o + GLINT_FADES, n);
    let z = cpu.read32(o + POSITION + 8).wrapping_add(0xff50_0000);
    cpu.write32(o + POSITION + 8, z);
    let trail = nested!(cpu, 0x0605_7634, explosions::afterimage_set(cpu, o, 0, 30, 0));
    if trail != 0 {
        cpu.write32(trail + BEHAVIOUR, GLINT_TRAIL);
        cpu.write8(trail + STEP, 0);
        let flags = cpu.read32(trail + FLAGS) | 0x1400;
        cpu.write32(trail + FLAGS, flags);
    }
    let z = cpu.read32(o + POSITION + 8) as i32;
    let flags = cpu.read32(o + FLAGS);
    let flags = if z.wrapping_abs() > 0x1f_ffff { flags | 0x8_0000 } else { flags & !0x8_0000 };
    cpu.write32(o + FLAGS, flags);
}

// ---- the glow ------------------------------------------------------------------------------------

/// SGL's matrix stack and its uses (called as the SH-2 code calls them).
const SL_PUSH_MATRIX: u32 = 0x0606_71cc;
const SL_POP_MATRIX: u32 = 0x0606_7200;
const SL_TRANSLATE: u32 = 0x0606_721c;
const SL_ROT_X: u32 = 0x0606_72ec;
const SL_ROT_Z: u32 = 0x0606_726c;
const SL_SCALE: u32 = 0x0606_73ec;
const SL_PUT_POLYGON: u32 = 0x0606_745c;
/// The model the glow puts: the kinds' table's entry 32, the file's third table's first
/// model.
const GLOW_KIND: u32 = KINDS + K_SIZE * 32;
const KIND_MODELS: u32 = 0x1c;
const GLOW_BEHAVIOUR: u32 = 0x0608_0e58;
const GLOW_DRAW: u32 = 0x0608_0ea8;
/// An object's draw function (+0x34).
const DRAW: u32 = 0x34;

/// The glow's behaviour (a polygon over the picture, scaled by its x +0x20; its count +0x30): the
/// scale grows by 0x1000 a frame to 0x10000, then the count goes on to 150, then the scale
/// shrinks by 0x1000 a frame and it ends at nothing.
pub fn glow_behaviour(cpu: &mut Cpu, o: u32) {
    let count = cpu.read16(o + TURN);
    if count == 0 {
        let s = cpu.read32(o + SCALE_X).wrapping_add(0x1000);
        cpu.write32(o + SCALE_X, s);
        if s != 0x1_0000 {
            return;
        }
    } else if count == 0x96 {
        let s = cpu.read32(o + SCALE_X).wrapping_sub(0x1000);
        cpu.write32(o + SCALE_X, s);
        if s == 0 {
            cpu.write8(o + MODE, ENDING);
        }
        return;
    }
    let count = cpu.read16(o + TURN).wrapping_add(1);
    cpu.write16(o + TURN, count);
}

/// The glow's draw (its sprite `s`): the model put at the middle, 0x40 dots up and tilted a
/// quarter, turned about z by the game's frame (512 a frame), scaled by its scale (z by 3 less
/// twice it, in 16.16).
pub fn glow_draw(cpu: &mut Cpu, s: u32) {
    let o = s.wrapping_sub(SPRITE);
    cpu.call(SL_PUSH_MATRIX, &[]);
    cpu.call(SL_TRANSLATE, &[0, 0xffc0_0000, 0]);
    cpu.call(SL_ROT_X, &[0x4000]);
    let turn = (cpu.read32(GAME_FRAMES) << 8).wrapping_mul(2) as i16 as i32 as u32;
    cpu.call(SL_ROT_Z, &[turn]);
    let scale = cpu.read32(o + SCALE_X);
    cpu.call(SL_SCALE, &[scale, scale, 0x3_0000u32.wrapping_sub(scale.wrapping_mul(2))]);
    let file = cpu.read32(GLOW_KIND);
    let list = cpu.read32(file + KIND_MODELS);
    let models = cpu.read32(list);
    let model = cpu.read32(models);
    cpu.call(SL_PUT_POLYGON, &[model]);
    cpu.call(SL_POP_MATRIX, &[]);
}

/// The glow made (list 2, 0x48 bytes, plain_exec's), the fades stopped and the picture's colour
/// offset set to (100, 80, 50), then faded to nothing at speed 128.
pub fn glow_set(cpu: &mut Cpu) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0x48)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, PLAIN_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + SPRITE));
    cpu.write32(o + BEHAVIOUR, GLOW_BEHAVIOUR);
    cpu.write32(o + FLAGS, 0xc0_0000);
    cpu.write32(o + DRAW, GLOW_DRAW);
    cpu.write32(o + SCALE_X, 0);
    nested!(cpu, 0x0605_e9d8, screen::stop_fades(cpu));
    nested!(cpu, 0x0605_e6dc, screen::set_fade_offset(cpu, 100, 0x50, 0x32));
    nested!(cpu, 0x0605_e93c, screen::fade_to(cpu, 0, 0, 0, 0x80));
}

// ---- the spiral of sparks --------------------------------------------------------------------------

/// A spark of the spiral: its scale +0x24 (copied to +0x20) shrinking by +0x40, which grows by
/// 0x100 a frame, its turn (+0x30) going on by +0x46; its place round the middle by the turn
/// (x by sin 8 dots out, y by cos, 0x40 dots up); it ends when the scale is gone.
const SPARK_SHRINK: u32 = 0x40;
const SPARK_SPIN: u32 = 0x44;
const SPARK_SPIN_LOW: u32 = 0x46;
const SPARK_BEHAVIOUR: u32 = 0x0608_0fc0;
const SPARK_DRAW: u32 = 0x0608_1054;

pub fn spark_behaviour(cpu: &mut Cpu, o: u32) {
    let s = cpu.read32(o + SCALE_Y).wrapping_sub(cpu.read32(o + SPARK_SHRINK));
    cpu.write32(o + SCALE_Y, s);
    cpu.write32(o + SCALE_X, s);
    if cpu.read32(o + SCALE_Y) as i32 <= 0 {
        cpu.write8(o + MODE, ENDING);
        return;
    }
    let turn = cpu.read16(o + TURN).wrapping_add(cpu.read16(o + SPARK_SPIN_LOW));
    cpu.write16(o + TURN, turn);
    let shrink = cpu.read32(o + SPARK_SHRINK).wrapping_add(0x100);
    cpu.write32(o + SPARK_SHRINK, shrink);
    let a = cpu.read16(o + TURN) as i16 as i32 as u32;
    let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    let x = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, sin, 0xfff8_0000));
    cpu.write32(o + POSITION, x);
    let a = cpu.read16(o + TURN) as i16 as i32 as u32;
    let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    let y = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, cos, 0x8_0000));
    cpu.write32(o + POSITION + 4, y.wrapping_add(0xffc0_0000));
}

/// A spark's draw (its sprite `s`): a sprite on the draw's frame (kind 0, frame 0xb3, its colour
/// the character base's 0x40 on, its lift 8 on the odd passes and 0x7e on the even), at the spark's
/// place, scaled by its scales (x a 16th, y 16 times), turned by its turn.
const S_FLAGS: u32 = 0;
const S_POSITION: u32 = 4;
const S_SCALES: u32 = 0x10;
const S_COLOUR: u32 = 0x18;
const S_KIND: u32 = 0x1e;
const S_FRAME: u32 = 0x1f;
const S_ANGLE: u32 = 0x20;
const S_LIFT: u32 = 0x23;
const S_FLAGS_SPARK: u32 = 0x6000_0000;

pub fn spark_draw(cpu: &mut Cpu, s: u32) {
    let o = s.wrapping_sub(SPRITE);
    let d = cpu.stack();
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, d));
    cpu.write32(d + S_FLAGS, S_FLAGS_SPARK);
    cpu.write8(d + S_FRAME, 0xb3);
    cpu.write8(d + S_KIND, 0);
    let lift = if cpu.read32(LOOPS) & 1 == 0 { 0x7e } else { 8 };
    cpu.write8(d + S_LIFT, lift);
    let colour = cpu.read16(CHAR_BASE).wrapping_add(0x40);
    cpu.write16(d + S_COLOUR, colour);
    let angle = cpu.read16(o + TURN);
    cpu.write16(d + S_ANGLE, angle);
    for k in [0, 4] {
        let v = cpu.read32(o + POSITION + k);
        cpu.write32(d + S_POSITION + k, v);
    }
    cpu.write32(d + S_POSITION + 8, 0);
    let w = cpu.read32(o + SCALE_X) as i32;
    cpu.write32(d + S_SCALES, (w / 16) as u32);
    let h = cpu.read32(o + SCALE_Y) << 4;
    cpu.write32(d + S_SCALES + 4, h);
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
}

/// A spark of the spiral made (list 2, 0x48 bytes, plain_exec's): a random turn, put out round
/// the middle by it 8 dots (as the behaviour does), a random speed up to 255 and a scale up to a
/// full dot and a half more.
pub fn spark_set(cpu: &mut Cpu) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0x48)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, PLAIN_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + SPRITE));
    cpu.write32(o + BEHAVIOUR, SPARK_BEHAVIOUR);
    cpu.write32(o + FLAGS, 0xc0_0000);
    cpu.write32(o + DRAW, SPARK_DRAW);
    let turn = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write16(o + TURN, turn as u16);
    let a = cpu.read16(o + TURN) as i16 as i32 as u32;
    let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    cpu.write32(o + POSITION, sin.wrapping_mul(8));
    let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    cpu.write32(o + POSITION + 4, 0xffc0_0000u32.wrapping_sub(cos.wrapping_mul(8)));
    cpu.write32(o + POSITION + 8, 0);
    cpu.write32(o + SPARK_SHRINK, 0);
    let spin = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x100));
    cpu.write32(o + SPARK_SPIN, spin);
    let scale = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000)).wrapping_add(0x1_c000);
    cpu.write32(o + SCALE_Y, scale);
}

// ---- the follower ----------------------------------------------------------------------------------

/// A follower (list 2, 0xbc bytes, plain_exec's): it sits where its parent (+0x6c) has its
/// x, y (+0x186c, +0x1870) and z 0xb0 dots in front (+0x1874), for 0x1e1 frames (its count +0x8a).
const FOLLOW_PARENT: u32 = 0x6c;
const FOLLOW_COUNT: u32 = 0x8a;
const FOLLOWER_BEHAVIOUR: u32 = 0x0608_11c4;
const PARENT_PLACE: u32 = 0x186c;

pub fn follower_behaviour(cpu: &mut Cpu, o: u32) {
    let parent = cpu.read32(o + FOLLOW_PARENT);
    let x = cpu.read32(parent + PARENT_PLACE);
    cpu.write32(o + POSITION, x);
    let y = cpu.read32(parent + PARENT_PLACE + 4);
    cpu.write32(o + POSITION + 4, y);
    let z = cpu.read32(parent + PARENT_PLACE + 8).wrapping_add(0xff50_0000);
    cpu.write32(o + POSITION + 8, z);
    let n = cpu.read16(o + FOLLOW_COUNT).wrapping_add(1);
    cpu.write16(o + FOLLOW_COUNT, n);
    if n as i16 > 0x1e0 {
        cpu.write8(o + MODE, ENDING);
    }
}

/// A follower made for `parent`, its place set at once: it, or 0.
pub fn follower_set(cpu: &mut Cpu, parent: u32) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0xbc)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, PLAIN_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + SPRITE));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HIT));
    cpu.write32(o + BEHAVIOUR, FOLLOWER_BEHAVIOUR);
    cpu.write32(o + FLAGS, 0x8008_0000);
    cpu.write32(o + FOLLOW_PARENT, parent);
    cpu.write16(o + FOLLOW_COUNT, 0);
    nested!(cpu, 0x0608_11c4, follower_behaviour(cpu, o));
    o
}

// ---- the drifting embers ---------------------------------------------------------------------------

/// An ember (list 3, 0x88 bytes, plain_exec's), a sprite with its own speed (x +0x4c, y +0x50), a
/// goal for each (+0x5c, +0x60), a count of frames (+0x74), the side it is on (+0x40) and eight
/// words from +0x76 (its sprite's frames: it points +0x34 at them). It ends once it is more than
/// 0x70 dots from the middle in y or 0xb0 in x.
const EMBER_SPEED: u32 = 0x4c;
const EMBER_GOAL: u32 = 0x5c;
const EMBER_SIDE: u32 = 0x40;
const EMBER_COUNT: u32 = 0x74;
const EMBER_FRAMES: u32 = 0x76;
const EMBER_BEHAVIOUR: u32 = 0x0608_1290;

pub fn ember_behaviour(cpu: &mut Cpu, o: u32) {
    let x = cpu.read32(o + POSITION).wrapping_add(cpu.read32(o + EMBER_SPEED));
    cpu.write32(o + POSITION, x);
    let y = cpu.read32(o + POSITION + 4).wrapping_add(cpu.read32(o + EMBER_SPEED + 4));
    cpu.write32(o + POSITION + 4, y);
    if (y as i32).wrapping_abs() > 0x70_0000 || (x as i32).wrapping_abs() > 0xb0_0000 {
        cpu.write8(o + MODE, ENDING);
    } else {
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1000));
        let turn = cpu.read16(o + TURN).wrapping_add(r as u16);
        cpu.write16(o + TURN, turn);
        if cpu.read8(o + EMBER_COUNT) & 0x3f == 0 {
            let g = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_8000));
            cpu.write32(o + EMBER_GOAL, g);
            let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0xc000));
            let g = (cpu.read32(o + EMBER_SIDE) << 16).wrapping_add(r);
            cpu.write32(o + EMBER_GOAL + 4, g);
        }
        for k in [0, 4] {
            let (speed, goal) = (cpu.read32(o + EMBER_SPEED + k), cpu.read32(o + EMBER_GOAL + k));
            let step = if k == 0 { 0x800 } else { 0x200 };
            let speed = nested!(cpu, 0x0604_6168, actors::approach(cpu, speed, goal, step));
            cpu.write32(o + EMBER_SPEED + k, speed);
        }
    }
    let n = cpu.read8(o + EMBER_COUNT).wrapping_add(1);
    cpu.write8(o + EMBER_COUNT, n);
}

/// An ember made on side `side` (a byte: 1 or -1; it starts at 0x90 dots out to the other way in y
/// by it, a random place across x and 0x40 dots in z, a speed of up to 2 dots across and `side`
/// 1 dot a frame up or down).
pub fn ember_set(cpu: &mut Cpu, side: u32) {
    let side = side as i8 as i32;
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0x88)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 3, blocks, PLAIN_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + SPRITE));
    cpu.write32(o + BEHAVIOUR, EMBER_BEHAVIOUR);
    cpu.write32(o + DRAW, o + EMBER_FRAMES);
    let flags = cpu.read32(o + FLAGS) | 0x2000_0501;
    cpu.write32(o + FLAGS, flags);
    let colour = cpu.read16(CHAR_BASE).wrapping_add(0xe1);
    cpu.write16(o + SPRITE_COLOUR, colour);
    cpu.write8(o + BYTE_33, 0);
    let x = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x80_0000));
    cpu.write32(o + POSITION, x);
    cpu.write32(o + POSITION + 4, (side as u32).wrapping_mul(0xff90_0000));
    let z = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x40_0000));
    cpu.write32(o + POSITION + 8, z);
    let speed = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x2_0000));
    cpu.write32(o + EMBER_SPEED, speed);
    cpu.write32(o + EMBER_SPEED + 4, (side as u32) << 16);
    cpu.write32(o + EMBER_GOAL, 0);
    let goal = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1000));
    cpu.write32(o + EMBER_GOAL + 4, goal);
    cpu.write8(o + EMBER_COUNT, 0);
    cpu.write32(o + EMBER_SIDE, side as u32);
    for (k, v) in [1u16, 1, 0, 0, 1, 1, 0, 0].into_iter().enumerate() {
        cpu.write16(o + EMBER_FRAMES + 2 * k as u32, v);
    }
}

// ---- the shots that aim from an enemy --------------------------------------------------------------

/// A shot (the aimed shot's description, bullets.rs) made from enemy `o`'s player number, from
/// the point `at` (x, y) moved `reach` out along `angle` (x by sin, y by -cos), going along the
/// angle at 1.25 dots a frame (flags 2).
const SHOT_NUMBER: u32 = 0x1e;
const SHOT_ANGLE: u32 = 0x2c;
const SHOT_FLAGS: u32 = 0x2e;
const SHOT_SPEED: u32 = 0x30;

pub fn shot_from(cpu: &mut Cpu, o: u32, angle: u32, at: u32, reach: u32) {
    let angle = angle & 0xffff;
    let shot = cpu.stack();
    let number = cpu.read8(o + NUMBER);
    cpu.write8(shot + SHOT_NUMBER, number);
    let a = angle as i16 as i32 as u32;
    let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    let x = cpu.read32(at).wrapping_add(sin.wrapping_mul(reach));
    cpu.write32(shot, x);
    let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    let y = cpu.read32(at + 4).wrapping_sub(cos.wrapping_mul(reach));
    cpu.write32(shot + 4, y);
    cpu.write32(shot + 8, 0);
    cpu.write16(shot + SHOT_ANGLE, angle as u16);
    cpu.write16(shot + SHOT_FLAGS, 2);
    cpu.write32(shot + SHOT_SPEED, 0x1_4000);
    nested!(cpu, 0x0605_3db0, bullets::bullet_set(cpu, shot));
}

// ---- the halos ---------------------------------------------------------------------------------------

/// A halo (list 2, 0x74 bytes, actor_exec's): a sprite that takes the scale a long holds (the
/// address of it +0x6c) and the place a triple of longs holds (the address +0x70), the place's z
/// 0xb0 dots nearer. With the scale at 0x4000 or less it ends; its flag 0x80000 is set while the
/// place is more than 0x10 dots from z = 0.
const HALO_SCALE_AT: u32 = 0x6c;
const HALO_PLACE_AT: u32 = 0x70;
const HALO_BEHAVIOUR: u32 = 0x0608_1514;
const HALO_HIT_FRAMES: u32 = 0x0609_0fdc;
const FLAG_FAR: u32 = 0x8_0000;
/// The sprite's second colour word (shown with +0x28's).
const COLOUR_B: u32 = 0x2a;
/// The bytes of an actor's hit set-up, 1 and 5 for every actor made here.
const ACTOR_54: u32 = 0x54;
const ACTOR_55: u32 = 0x55;

pub fn halo_behaviour(cpu: &mut Cpu, o: u32) {
    let scale_at = cpu.read32(o + HALO_SCALE_AT);
    let scale = cpu.read32(scale_at);
    if scale as i32 <= 0x4000 {
        cpu.write8(o + MODE, ENDING);
        return;
    }
    cpu.write32(o + SCALE_X, scale);
    cpu.write32(o + SCALE_Y, scale);
    let place = cpu.read32(o + HALO_PLACE_AT);
    let x = cpu.read32(place);
    cpu.write32(o + POSITION, x);
    let y = cpu.read32(place + 4);
    cpu.write32(o + POSITION + 4, y);
    let z = cpu.read32(place + 8).wrapping_add(0xff50_0000);
    cpu.write32(o + POSITION + 8, z);
    let flags = cpu.read32(o + FLAGS);
    let far = (z as i32).wrapping_abs() > 0x10_0000;
    cpu.write32(o + FLAGS, if far { flags | FLAG_FAR } else { flags & !FLAG_FAR });
}

/// A halo made for the scale at `scale_at` and the place `place`, its place and scale taken at once.
pub fn halo_set(cpu: &mut Cpu, place: u32, scale_at: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0x74)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + SPRITE));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HIT));
    cpu.write32(o + BEHAVIOUR, HALO_BEHAVIOUR);
    cpu.write32(o + FLAGS, 0x8008_0400);
    cpu.write32(o + HIT, HALO_HIT_FRAMES);
    cpu.write8(o + ACTOR_54, 1);
    cpu.write8(o + ACTOR_55, 5);
    cpu.write32(o + HALO_SCALE_AT, scale_at);
    cpu.write32(o + HALO_PLACE_AT, place);
    nested!(cpu, 0x0608_1514, halo_behaviour(cpu, o));
}

/// The eight halos of a ring: one for each of the eight places (triples of longs, 12 bytes apart)
/// from `places`, all on the scale at `scale_at`.
pub fn halo_ring_set(cpu: &mut Cpu, places: u32, scale_at: u32) {
    for i in 0..8u32 {
        nested!(cpu, 0x0608_157c, halo_set(cpu, places.wrapping_add(12 * i), scale_at));
    }
}

// ---- the streaks and darts thrown from a figure --------------------------------------------------------

/// A streak's (list 2, 0x78 bytes, actor_exec's) fields: the speed it moves at each frame (x +0x6c, y
/// +0x70) and its flicker (+0x74: the parity of the game's frame it shows on).
const STREAK_SPEED: u32 = 0x6c;
const STREAK_FLICKER: u32 = 0x74;
const STREAK_BEHAVIOUR: u32 = 0x0608_1660;
const STREAK_HIT_FRAMES: u32 = 0x0609_0fe4;
const STREAK_FRAMES: u32 = 0x0609_0fec;
const STREAK_MAGIC: u32 = 0x0608_165c;
/// The sprite flag that a streak turns over each frame.
const FLAG_MIRROR: u32 = 0x10;
/// The byte (+0x18a0) a source keeps, turned over by each streak, dart or shard made from it: the
/// flicker of the next.
const SOURCE_PARITY: u32 = 0x18a0;
const SOURCE_MIRRORED: u32 = 0x278;

/// A streak's behaviour: gone once off screen. Else it shows (its flag 0x80000) on the frames
/// whose parity is its flicker's; it moves by its speed, its scales ease toward 1.07 (1.13 on the
/// frames with bit 1 set) by 0x800; its sprite's frame is a table's by a third of the game's frame;
/// its flag 0x10 turned over.
pub fn streak_behaviour(cpu: &mut Cpu, o: u32) {
    if nested!(cpu, 0x0605_6a24, aim::off_screen(cpu, o + SPRITE)) as u8 != 0 {
        cpu.write8(o + MODE, ENDING);
        return;
    }
    let frames = cpu.read32(GAME_FRAMES);
    let flags = cpu.read32(o + FLAGS);
    let shows = frames & 1 == cpu.read8(o + STREAK_FLICKER) as u32;
    cpu.write32(o + FLAGS, if shows { flags | FLAG_FAR } else { flags & !FLAG_FAR });
    for k in [0, 4] {
        let v = cpu.read32(o + POSITION + k).wrapping_add(cpu.read32(o + STREAK_SPEED + k));
        cpu.write32(o + POSITION + k, v);
    }
    let goal = if cpu.read32(GAME_FRAMES) & 2 == 0 { 0x1_1000 } else { 0x1_2000 };
    let s = cpu.read32(o + SCALE_X);
    let s = nested!(cpu, 0x0604_6168, actors::approach(cpu, s, goal, 0x800));
    cpu.write32(o + SCALE_X, s);
    cpu.write32(o + SCALE_Y, s);
    // the SH-2 code divides by three with a multiply by the constant it keeps in its code
    let magic = cpu.read32(STREAK_MAGIC);
    let third = ((cpu.read32(GAME_FRAMES) as u64 * magic as u64) >> 33) as u32;
    let frame = cpu.read8(STREAK_FRAMES + (third & 3));
    cpu.write8(o + FRAME, frame);
    let flags = cpu.read32(o + FLAGS) ^ FLAG_MIRROR;
    cpu.write32(o + FLAGS, flags);
}

/// A streak thrown from `source` at the place `at` (x, y) in the direction `angle`: its speed
/// 1.6 dots a frame that way (x by sin, y by -cos), the place already 12 frames along; its turn
/// the opposite way, its flicker the source's parity, which it turns over.
pub fn streak_set(cpu: &mut Cpu, source: u32, at: u32, angle: u32) {
    let angle = angle & 0xffff;
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0x78)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + SPRITE));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HIT));
    cpu.write32(o + BEHAVIOUR, STREAK_BEHAVIOUR);
    cpu.write32(o + FLAGS, 0x2008_0420);
    let number = cpu.read8(source + NUMBER);
    cpu.write8(o + NUMBER, number);
    for k in [0, 4] {
        let v = cpu.read32(at + k);
        cpu.write32(o + POSITION + k, v);
    }
    cpu.write32(o + POSITION + 8, 0);
    cpu.write8(o + BYTE_33, 8);
    cpu.write8(o + FRAME, 20);
    cpu.write8(o + SPRITE_KIND, 78);
    let colour = cpu.read16(CHAR_BASE);
    cpu.write16(o + COLOUR_B, colour);
    cpu.write16(o + SPRITE_COLOUR, colour);
    cpu.write32(o + HIT, STREAK_HIT_FRAMES);
    cpu.write8(o + ACTOR_54, 1);
    cpu.write8(o + ACTOR_55, 5);
    cpu.write32(o + SCALE_X, 0);
    cpu.write32(o + SCALE_Y, 0);
    let a = angle as u16 as i16 as i32 as u32;
    let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    let vx = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, sin, 0x1_a000));
    cpu.write32(o + STREAK_SPEED, vx);
    let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    let vy = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, cos, 0xfffe_6000));
    cpu.write32(o + STREAK_SPEED + 4, vy);
    let x = cpu.read32(o + POSITION).wrapping_add(vx.wrapping_mul(12));
    cpu.write32(o + POSITION, x);
    let y = cpu.read32(o + POSITION + 4).wrapping_add(vy.wrapping_mul(12));
    cpu.write32(o + POSITION + 4, y);
    cpu.write16(o + TURN, (angle as u16).wrapping_add(0x8000));
    let parity = cpu.read8(source + SOURCE_PARITY) ^ 1;
    cpu.write8(source + SOURCE_PARITY, parity);
    cpu.write8(o + STREAK_FLICKER, parity);
}

/// A dart (list 2, 0x78 bytes, actor_exec's): a streak's kin that holds its scale at 0x10000 (easing
/// by 0x3000), flickers its sprite's lift and colour with the game's frames and takes its frame
/// from a table by the frame's low two bits.
const DART_BEHAVIOUR: u32 = 0x0608_2014;
const DART_HIT_FRAMES: u32 = 0x0609_1012;
const DART_FRAMES: u32 = 0x0609_101c;

/// A dart's behaviour: gone once off screen; flagged 0x80000 on the frames whose parity is its
/// flicker's; moved by its speed; its scales eased toward 1.0 by 0x3000; on the even frames its lift
/// -8 and its colour the character base's, on the odd 126 and 16 on; its frame a table's by the
/// game's frame.
pub fn dart_behaviour(cpu: &mut Cpu, o: u32) {
    if nested!(cpu, 0x0605_6a24, aim::off_screen(cpu, o + SPRITE)) as u8 != 0 {
        cpu.write8(o + MODE, ENDING);
        return;
    }
    let shows = cpu.read32(GAME_FRAMES) & 1 == cpu.read8(o + STREAK_FLICKER) as u32;
    let flags = cpu.read32(o + FLAGS);
    cpu.write32(o + FLAGS, if shows { flags | FLAG_FAR } else { flags & !FLAG_FAR });
    for k in [0, 4] {
        let v = cpu.read32(o + POSITION + k).wrapping_add(cpu.read32(o + STREAK_SPEED + k));
        cpu.write32(o + POSITION + k, v);
    }
    let s = cpu.read32(o + SCALE_X);
    let s = nested!(cpu, 0x0604_6168, actors::approach(cpu, s, 0x1_0000, 0x3000));
    cpu.write32(o + SCALE_X, s);
    cpu.write32(o + SCALE_Y, s);
    let colour = cpu.read16(CHAR_BASE);
    if cpu.read32(GAME_FRAMES) & 1 == 0 {
        cpu.write8(o + BYTE_33, -8i8 as u8);
        cpu.write16(o + SPRITE_COLOUR, colour);
    } else {
        cpu.write8(o + BYTE_33, 126);
        cpu.write16(o + SPRITE_COLOUR, colour.wrapping_add(16));
    }
    let c = cpu.read16(o + SPRITE_COLOUR);
    cpu.write16(o + COLOUR_B, c);
    let low = cpu.read32(GAME_FRAMES) & 3;
    let frame = cpu.read8(DART_FRAMES + low);
    cpu.write8(o + FRAME, frame);
}

/// A dart thrown from `source` at the place `at` in the direction `angle`: a speed of 0x14000
/// that way, the place 8 frames along, the source's parity flipped for its flicker.
pub fn dart_set(cpu: &mut Cpu, source: u32, at: u32, angle: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0x78)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + SPRITE));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HIT));
    cpu.write32(o + BEHAVIOUR, DART_BEHAVIOUR);
    cpu.write32(o + FLAGS, 0x2008_0400);
    let number = cpu.read8(source + NUMBER);
    cpu.write8(o + NUMBER, number);
    let a = angle as u16 as i16 as i32 as u32;
    for k in [0, 4] {
        let v = cpu.read32(at + k);
        cpu.write32(o + POSITION + k, v);
    }
    cpu.write32(o + POSITION + 8, 0);
    cpu.write8(o + BYTE_33, 8);
    cpu.write8(o + FRAME, 4);
    cpu.write8(o + SPRITE_KIND, 78);
    let colour = cpu.read16(CHAR_BASE);
    cpu.write16(o + COLOUR_B, colour);
    cpu.write16(o + SPRITE_COLOUR, colour);
    cpu.write32(o + HIT, DART_HIT_FRAMES);
    cpu.write8(o + ACTOR_54, 1);
    cpu.write8(o + ACTOR_55, 5);
    cpu.write32(o + SCALE_X, 0);
    cpu.write32(o + SCALE_Y, 0);
    let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    let vx = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, sin, 0x1_4000));
    cpu.write32(o + STREAK_SPEED, vx);
    let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    let vy = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, cos, 0xfffe_c000));
    cpu.write32(o + STREAK_SPEED + 4, vy);
    let x = cpu.read32(o + POSITION).wrapping_add(vx.wrapping_mul(8));
    cpu.write32(o + POSITION, x);
    let y = cpu.read32(o + POSITION + 4).wrapping_add(vy.wrapping_mul(8));
    cpu.write32(o + POSITION + 4, y);
    let parity = cpu.read8(source + SOURCE_PARITY) ^ 1;
    cpu.write8(source + SOURCE_PARITY, parity);
    cpu.write8(o + STREAK_FLICKER, parity);
}

// ---- the bolts -------------------------------------------------------------------------------------------

/// A bolt (list 2, 0xbc bytes, actor_exec's): it grows for 16 frames, takes its heading (the
/// nearer player's, or the one it was given: +0x88 set), then flies off on a speed that
/// builds up (+0x80, +0x84, by +0xac, +0xb0) while its scales swell and narrow. Its phase +0x7c (0
/// growing, then 1), the frames left in the phase +0x89, the fixed-heading flag +0x88 and the heading
/// +0x8a.
const BOLT_PHASE: u32 = 0x7c;
const BOLT_SPEED: u32 = 0x80;
const BOLT_AIMED_NOT: u32 = 0x88;
const BOLT_COUNT: u32 = 0x89;
const BOLT_HEADING: u32 = 0x8a;
const BOLT_PUSH: u32 = 0xac;
const BOLT_BEHAVIOUR: u32 = 0x0608_1898;
const BOLT_HIT_FRAMES: u32 = 0x0609_0ff0;
const BOLT_SPEEDS: u32 = 0x0609_1000;
const BOLT_FRAMES: u32 = 0x0609_0ff8;
const DIFFICULTY: u32 = 0x0600_ed84;

/// A bolt's behaviour: ended as a bullet is (off screen or on the ground). Growing: the count
/// down by one and the x scale up by 0x800; at the count's end the second phase starts (16
/// frames), its heading the nearer player's (or the given one), its speed cleared and its push
/// made (along the heading, by the table's speed for the difficulty); the speed goes on by the
/// push each frame, in the second phase while the count lasts the x scale shrinks by 0xa00 and
/// the y scale grows by 0x600. Each frame it moves by the speed, and shows a table's frame by the
/// game's frame.
pub fn bolt_behaviour(cpu: &mut Cpu, o: u32) {
    if nested!(cpu, 0x0605_3b10, bullets::bullet_ends(cpu, o, 1)) as u8 == 0 {
        if cpu.read8(o + BOLT_PHASE) == 0 {
            let n = cpu.read8(o + BOLT_COUNT).wrapping_sub(1);
            cpu.write8(o + BOLT_COUNT, n);
            let s = cpu.read32(o + SCALE_X).wrapping_add(0x800);
            cpu.write32(o + SCALE_X, s);
            if n as i8 <= 0 {
                let phase = cpu.read8(o + BOLT_PHASE).wrapping_add(1);
                cpu.write8(o + BOLT_PHASE, phase);
                cpu.write8(o + BOLT_COUNT, 16);
                if cpu.read8(o + BOLT_AIMED_NOT) != 0 {
                    let heading = cpu.read16(o + BOLT_HEADING);
                    cpu.write16(o + TURN, heading);
                } else {
                    let player = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + SPRITE));
                    let heading = nested!(cpu, 0x0604_5e30, aim::angle_to(cpu, o, player));
                    cpu.write16(o + TURN, heading as u16);
                }
                cpu.write32(o + BOLT_SPEED, 0);
                cpu.write32(o + BOLT_SPEED + 4, 0);
                let difficulty = cpu.read8(DIFFICULTY) as i8 as i32 as u32;
                let speed = cpu.read16(BOLT_SPEEDS + 2 * difficulty) as i16 as i32 as u32;
                let a = cpu.read16(o + TURN) as i16 as i32 as u32;
                let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
                let push = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, sin, speed));
                cpu.write32(o + BOLT_PUSH, push);
                let difficulty = cpu.read8(DIFFICULTY) as i8 as i32 as u32;
                let speed = (cpu.read16(BOLT_SPEEDS + 2 * difficulty) as i16 as i32 as u32).wrapping_neg();
                let a = cpu.read16(o + TURN) as i16 as i32 as u32;
                let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
                let push = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, cos, speed));
                cpu.write32(o + BOLT_PUSH + 4, push);
            }
            bolt_speed_up(cpu, o);
        } else {
            let n = cpu.read8(o + BOLT_COUNT) as i8;
            if n > 0 {
                cpu.write8(o + BOLT_COUNT, (n - 1) as u8);
                bolt_speed_up(cpu, o);
                let sx = cpu.read32(o + SCALE_X).wrapping_add(-0xa00i32 as u32);
                cpu.write32(o + SCALE_X, sx);
                let sy = cpu.read32(o + SCALE_Y).wrapping_add(0x600);
                cpu.write32(o + SCALE_Y, sy);
            }
        }
        for k in [0, 4] {
            let v = cpu.read32(o + POSITION + k).wrapping_add(cpu.read32(o + BOLT_SPEED + k));
            cpu.write32(o + POSITION + k, v);
        }
    }
    let low = cpu.read32(GAME_FRAMES) & 7;
    let frame = cpu.read8(BOLT_FRAMES + low);
    cpu.write8(o + FRAME, frame);
}

/// A bolt's speed on by its push.
fn bolt_speed_up(cpu: &mut Cpu, o: u32) {
    for k in [0, 4] {
        let v = cpu.read32(o + BOLT_SPEED + k).wrapping_add(cpu.read32(o + BOLT_PUSH + k));
        cpu.write32(o + BOLT_SPEED + k, v);
    }
}

/// The shot description a bolt is made from (the stack struct its maker's caller builds): the
/// player's number, the fixed-heading flag, the angle, the heading's word and the whole dots of
/// the speed (the high word of the long at +0x30).
const DESC_NUMBER: u32 = 0x1e;
const DESC_FIXED: u32 = 0x17;
const DESC_ANGLE: u32 = 0x2c;
const DESC_HEADING: u32 = 0x2e;
const DESC_SPEED: u32 = 0x30;

/// A bolt made from the shot description `desc`: it takes the number, the fixed-heading flag, the
/// angle (as its turn) and the heading, its colour set by that flag; it starts at (0, -24 dots, 0)
/// with a speed of the description's speed times 20 along its turn (and the push minus a sixteenth
/// of that).
pub fn bolt_set(cpu: &mut Cpu, desc: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0xbc)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + SPRITE));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HIT));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    let number = cpu.read8(desc + DESC_NUMBER);
    cpu.write8(o + NUMBER, number);
    cpu.write32(o + BEHAVIOUR, BOLT_BEHAVIOUR);
    cpu.write32(o + FLAGS, 0x2000_0400);
    cpu.write32(o + POSITION, 0);
    cpu.write32(o + POSITION + 4, 0xffe8_0000);
    cpu.write32(o + POSITION + 8, 0);
    cpu.write8(o + BYTE_33, 9);
    cpu.write8(o + FRAME, 12);
    cpu.write8(o + SPRITE_KIND, 78);
    cpu.write32(o + HIT, BOLT_HIT_FRAMES);
    cpu.write8(o + ACTOR_54, 1);
    cpu.write8(o + ACTOR_55, 5);
    let fixed = cpu.read8(desc + DESC_FIXED);
    cpu.write8(o + BOLT_AIMED_NOT, fixed);
    cpu.write8(o + BOLT_COUNT, 20);
    let turn = cpu.read16(desc + DESC_ANGLE);
    cpu.write16(o + TURN, turn);
    let heading = cpu.read16(desc + DESC_HEADING);
    cpu.write16(o + BOLT_HEADING, heading);
    let count = cpu.read8(o + BOLT_COUNT) as i8 as i32;
    let lean = cpu.read16(desc + DESC_SPEED) as i16 as i32;
    let r8 = (lean.wrapping_mul(count) as u32) << 12;
    let a = cpu.read16(o + TURN) as i16 as i32 as u32;
    let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    let vx = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, sin, r8));
    cpu.write32(o + BOLT_SPEED, vx);
    let lean = cpu.read16(desc + DESC_SPEED) as i16 as i32;
    let r8 = (lean.wrapping_neg().wrapping_mul(count) as u32) << 11;
    let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    let vy = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, cos, r8));
    cpu.write32(o + BOLT_SPEED + 4, vy);
    for k in [0, 4] {
        let v = cpu.read32(o + BOLT_SPEED + k) as i32;
        let sixteenth = (v + if v < 0 { 15 } else { 0 }) >> 4;
        cpu.write32(o + BOLT_PUSH + k, sixteenth.wrapping_neg() as u32);
    }
    cpu.write32(o + SCALE_X, 0x1000);
    cpu.write32(o + SCALE_Y, 0x8000);
    let colour = cpu.read16(CHAR_BASE).wrapping_add(if fixed == 0 { 0x30 } else { 0x40 });
    cpu.write16(o + COLOUR_B, colour);
    cpu.write16(o + SPRITE_COLOUR, colour);
}

// ---- the orbiter and its trail ---------------------------------------------------------------------------

/// An orbiter (list 2, 0xc4 bytes, actor_exec's) goes round the centre (+0x6c, +0x70) at a radius
/// (+0x74) that grows 0xf000 a frame, its angle (+0x78, a word) turning by +0x7a (a word); its
/// turn (+0x30) is the angle a quarter on. The whole dots it moved each frame are kept in
/// 12-byte rows: x at +0xa0, y at +0xac, and the turn's high byte at +0xb8; what it moved last is
/// +0x7c and +0x7d. Four trail pieces (list 3, 0x74 bytes, plain_exec's) follow it.
const ORB_CENTRE: u32 = 0x6c;
const ORB_RADIUS: u32 = 0x74;
const ORB_ANGLE: u32 = 0x78;
const ORB_SPIN: u32 = 0x7a;
const ORB_MOVED: u32 = 0x7c;
const ORB_ROW_X: u32 = 0xa0;
const ORB_ROW_Y: u32 = 0xac;
const ORB_ROW_TURN: u32 = 0xb8;
const ORB_BEHAVIOUR: u32 = 0x0608_1c74;
const ORB_HIT_FRAMES: u32 = 0x0609_100a;
const ORB_AREA: (u32, u32, u32, u32) = (0xff40_0000, 0x00c0_0000, 0xff10_0000, 0x00f0_0000);
const PIECE_BEHAVIOUR: u32 = 0x0608_1bf8;
/// A trail piece (+0x6c its orbiter, +0x70 the place in the orbiter's rows, a word, +0x72 the
/// frame it shows, less the game's frame's half's low bit).
const PIECE_ORBITER: u32 = 0x6c;
const PIECE_PLACE: u32 = 0x70;
const PIECE_FRAME: u32 = 0x72;

/// A trail piece's behaviour: gone once its orbiter's mode is past 0x6f. Else it moves by the
/// row's entry for its place in x and in y (whole dots, 16 bits up), and its frame is its
/// base less the game's frame's low bit of half.
pub fn trail_piece_behaviour(cpu: &mut Cpu, o: u32) {
    let orbiter = cpu.read32(o + PIECE_ORBITER);
    if cpu.read8(orbiter + MODE) as i8 > 0x6f {
        cpu.write8(o + MODE, ENDING);
        return;
    }
    let place = cpu.read16(o + PIECE_PLACE) as i16 as i32 as u32;
    let dx = cpu.read8(orbiter.wrapping_add(place).wrapping_add(ORB_ROW_X)) as i8 as i32 as u32;
    let x = cpu.read32(o + POSITION).wrapping_add(dx << 16);
    cpu.write32(o + POSITION, x);
    let place = cpu.read16(o + PIECE_PLACE) as i16 as i32 as u32;
    let dy = cpu.read8(orbiter.wrapping_add(place).wrapping_add(ORB_ROW_Y)) as i8 as i32 as u32;
    let y = cpu.read32(o + POSITION + 4).wrapping_add(dy << 16);
    cpu.write32(o + POSITION + 4, y);
    let half = cpu.read32(GAME_FRAMES) >> 1;
    let parity = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, half, 2));
    let base = cpu.read8(o + PIECE_FRAME) as i8 as i32 as u32;
    cpu.write8(o + FRAME, base.wrapping_sub(parity) as u8);
}

/// The whole dots of a 16.16 number, as the SH-2 code takes them (negative ones rounded to zero).
fn whole(v: u32) -> i32 {
    let v = if (v as i32) < 0 { v.wrapping_add(0xffff) } else { v };
    (v >> 16) as u16 as i16 as i32
}

/// An orbiter's behaviour: gone if it leaves the area (192 dots up or down, 240 across:
/// `ORB_AREA`). Else its angle goes on by its spin, its turn is the angle a quarter on (the
/// other way for a negative spin), it moves to the point on the circle of its radius round
/// the centre (x by sin, y up by cos), the whole dots it moved kept at +0x7c and +0x7d and shifted
/// into the rows, its radius grown by 0xf000, its frame 47 or 46 by the game's frames.
pub fn orbiter_behaviour(cpu: &mut Cpu, o: u32) {
    let (top, bottom, left, right) = ORB_AREA;
    let out = cpu.nested_with_stack(&aim::AREA_FLAGS_PROLOGUE, aim::AREA_FLAGS, &[right], |cpu| aim::area_flags(cpu, o + SPRITE, top, bottom, left, right));
    if out as u8 != 0 {
        cpu.write8(o + MODE, ENDING);
        return;
    }
    let angle = cpu.read16(o + ORB_ANGLE).wrapping_add(cpu.read16(o + ORB_SPIN));
    cpu.write16(o + ORB_ANGLE, angle);
    let tilt = if (cpu.read16(o + ORB_SPIN) as i16) > 0 { 0x4000 } else { 0xc000 };
    cpu.write16(o + TURN, angle.wrapping_add(tilt));
    let before = cpu.read32(o + POSITION);
    let radius = cpu.read32(o + ORB_RADIUS);
    let a = cpu.read16(o + ORB_ANGLE) as i16 as i32 as u32;
    let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    let reach = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, sin, radius));
    let x = cpu.read32(o + ORB_CENTRE).wrapping_add(reach);
    cpu.write32(o + POSITION, x);
    cpu.write8(o + ORB_MOVED, whole(x).wrapping_sub(whole(before)) as u8);
    let before = cpu.read32(o + POSITION + 4);
    let radius = cpu.read32(o + ORB_RADIUS);
    let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    let reach = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, cos, radius));
    let y = cpu.read32(o + ORB_CENTRE + 4).wrapping_sub(reach);
    cpu.write32(o + POSITION + 4, y);
    cpu.write8(o + ORB_MOVED + 1, whole(y).wrapping_sub(whole(before)) as u8);
    let radius = cpu.read32(o + ORB_RADIUS).wrapping_add(0xf000);
    cpu.write32(o + ORB_RADIUS, radius);
    let dx = cpu.read8(o + ORB_MOVED) as i8 as i32 as u32;
    nested!(cpu, 0x0605_62b4, actors::shift_in_bytes(cpu, o + ORB_ROW_X, dx, 12));
    let dy = cpu.read8(o + ORB_MOVED + 1) as i8 as i32 as u32;
    nested!(cpu, 0x0605_62b4, actors::shift_in_bytes(cpu, o + ORB_ROW_Y, dy, 12));
    let half = cpu.read32(GAME_FRAMES) >> 1;
    let parity = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, half, 2));
    cpu.write8(o + FRAME, 47u32.wrapping_sub(parity) as u8);
}

/// An orbiter made for `source` at the place `at` (x, y), its angle `angle`: it starts at a
/// radius of 16 dots, spins 0x140 a frame (the other way when the source (+0x278) is
/// mirrored), its rows cleared but the turn row, which has the turn's high byte all along; and
/// its four trail pieces made (each a frame later in the row, a smaller scale and a frame
/// number up by one). Nothing more is made when there is no room.
pub fn orbiter_set(cpu: &mut Cpu, source: u32, at: u32, angle: u32) {
    let angle = angle & 0xffff;
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0xc4)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + SPRITE));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HIT));
    cpu.write32(o + BEHAVIOUR, ORB_BEHAVIOUR);
    cpu.write32(o + FLAGS, 0x3000_0400);
    let number = cpu.read8(source + NUMBER);
    cpu.write8(o + NUMBER, number);
    let x = cpu.read32(at);
    cpu.write32(o + POSITION, x);
    let y = cpu.read32(at + 4);
    cpu.write32(o + POSITION + 4, y);
    cpu.write32(o + POSITION + 8, 0);
    cpu.write8(o + BYTE_33, 8);
    cpu.write8(o + FRAME, 47);
    cpu.write8(o + SPRITE_KIND, 0);
    let colour = cpu.read16(CHAR_BASE);
    cpu.write16(o + COLOUR_B, colour);
    cpu.write16(o + SPRITE_COLOUR, colour);
    cpu.write32(o + HIT, ORB_HIT_FRAMES);
    cpu.write8(o + ACTOR_54, 1);
    cpu.write8(o + ACTOR_55, 5);
    let x = cpu.read32(o + POSITION);
    cpu.write32(o + ORB_CENTRE, x);
    let y = cpu.read32(o + POSITION + 4);
    cpu.write32(o + ORB_CENTRE + 4, y);
    cpu.write16(o + ORB_ANGLE, angle as u16);
    cpu.write16(o + TURN, (angle as u16).wrapping_add(0x4000));
    cpu.write32(o + ORB_RADIUS, 0x10_0000);
    let spin = if cpu.read8(source + SOURCE_MIRRORED) != 0 { 0xfec0 } else { 0x140 };
    cpu.write16(o + ORB_SPIN, spin);
    cpu.write8(o + ORB_MOVED, 0);
    cpu.write8(o + ORB_MOVED + 1, 0);
    nested!(cpu, 0x0605_6294, actors::fill_bytes(cpu, o + ORB_ROW_X, 0, 12));
    nested!(cpu, 0x0605_6294, actors::fill_bytes(cpu, o + ORB_ROW_Y, 0, 12));
    let turn = cpu.read16(o + TURN) as i16 as i32;
    let rounded = (turn + if turn < 0 { 0xff } else { 0 }) as u32;
    nested!(cpu, 0x0600_8164, clib::ashiftrt_r4::<8>(cpu, rounded));
    let high = (((rounded as i32) >> 8) as i8) as i32 as u32;
    nested!(cpu, 0x0605_6294, actors::fill_bytes(cpu, o + ORB_ROW_TURN, high, 12));
    for i in 0..4u32 {
        let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0x74)) as i8 as i32 as u32;
        let p = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 3, blocks, PLAIN_EXEC));
        if p == 0 {
            return;
        }
        nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, p + SPRITE));
        cpu.write32(p + FLAGS, 0x2000_0420);
        cpu.write32(p + BEHAVIOUR, PIECE_BEHAVIOUR);
        let x = cpu.read32(o + POSITION);
        cpu.write32(p + POSITION, x);
        let y = cpu.read32(o + POSITION + 4);
        cpu.write32(p + POSITION + 4, y);
        cpu.write32(p + POSITION + 8, 0);
        let b = cpu.read8(o + BYTE_33).wrapping_sub(i as u8);
        cpu.write8(p + BYTE_33, b);
        let f = cpu.read8(o + FRAME).wrapping_add(i as u8).wrapping_add(1);
        cpu.write8(p + FRAME, f);
        cpu.write8(p + PIECE_FRAME, f);
        cpu.write8(p + SPRITE_KIND, 0);
        let colour = cpu.read16(o + SPRITE_COLOUR);
        cpu.write16(p + COLOUR_B, colour);
        cpu.write16(p + SPRITE_COLOUR, colour);
        cpu.write32(p + PIECE_ORBITER, o);
        cpu.write16(p + PIECE_PLACE, (3 * (i + 1) - 1) as u16);
        cpu.write32(p + SCALE_X, 0x1_0000u32.wrapping_sub(i << 12));
    }
}

// ---- the figure's darts ----------------------------------------------------------------------------------

/// The figure (the stage's skeleton, the object the pose animation above moves) has its place at
/// +0x186c (x, y, z) and the points darts are thrown from, 0x398 and 0x47c (x, y); its aim angle
/// +0x1878 (a word), the way it faces +0x187a (a word) and the player it aims at +0x188c; the
/// frame count +0x2b0 (a word), a byte that tells it is angry +0x574 and the one turned over at each
/// throw +0x1888.
const FIG_AIM: u32 = 0x1878;
const FIG_FACING: u32 = 0x187a;
const FIG_TOGGLE: u32 = 0x1888;
const FIG_TARGET: u32 = 0x188c;
const FIG_COUNT: u32 = 0x2b0;
const FIG_ANGRY: u32 = 0x574;
const FIG_MUZZLES: [u32; 2] = [0x398, 0x47c];
const THROW_EVERY: u32 = 0x0609_1020;
const VOLLEY_EVERY: u32 = 0x0609_1028;
const QUARTER: u32 = 0x4000;

/// The figure's throw: every `THROW_EVERY`[difficulty] frames (by its count) darts (`dart_set`)
/// from its place, at its aim angle turned round: on an odd throw (the toggle counts them) four,
/// five while it is angry (+0x574 past 16), from 0xa000 on; on an even throw four from 0x6800 on, and
/// from the third difficulty on a dart from each muzzle at the nearer player. Whether it throws or not,
/// last the way it faces: away from the player, a quarter on.
pub fn figure_throw(cpu: &mut Cpu, o: u32) {
    let player = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + SPRITE));
    let back = (cpu.read16(o + FIG_AIM) as i16 as i32).wrapping_neg() as u32 & 0xffff;
    let difficulty = cpu.read8(DIFFICULTY) as i8 as i32 as u32;
    let every = cpu.read8(THROW_EVERY + difficulty) as u32;
    let count = cpu.read16(o + FIG_COUNT) as u32;
    let place = o + PARENT_PLACE;
    if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, count, every)) == 0 {
        let toggle = cpu.read8(o + FIG_TOGGLE).wrapping_add(1);
        cpu.write8(o + FIG_TOGGLE, toggle);
        if toggle & 1 != 0 {
            nested!(cpu, 0x0608_20ec, dart_set(cpu, o, place, back.wrapping_add(0xa000) & 0xffff));
            nested!(cpu, 0x0608_20ec, dart_set(cpu, o, place, back.wrapping_add(0x9000) & 0xffff));
            if cpu.read8(o + FIG_ANGRY) > 16 {
                nested!(cpu, 0x0608_20ec, dart_set(cpu, o, place, back.wrapping_add(0x8000) & 0xffff));
            }
            nested!(cpu, 0x0608_20ec, dart_set(cpu, o, place, back.wrapping_add(0x7000) & 0xffff));
            nested!(cpu, 0x0608_20ec, dart_set(cpu, o, place, back.wrapping_add(0x6000) & 0xffff));
        } else {
            for turn in [0x6800u32, 0x7800, 0x8800, 0x9800] {
                nested!(cpu, 0x0608_20ec, dart_set(cpu, o, place, back.wrapping_add(turn) & 0xffff));
            }
            if difficulty as i8 > 1 {
                for muzzle in FIG_MUZZLES {
                    let x = cpu.read32(player + POSITION).wrapping_sub(cpu.read32(o + muzzle));
                    let y = cpu.read32(player + POSITION + 4).wrapping_sub(cpu.read32(o + muzzle + 4));
                    let angle = nested!(cpu, 0x0606_780c, sgl::sl_atan(cpu, x, y));
                    nested!(cpu, 0x0608_20ec, dart_set(cpu, o, o + muzzle, angle.wrapping_add(QUARTER) & 0xffff));
                }
            }
        }
    }
    let x = cpu.read32(player + POSITION).wrapping_sub(cpu.read32(o + PARENT_PLACE));
    let y = cpu.read32(player + POSITION + 4).wrapping_sub(cpu.read32(o + PARENT_PLACE + 4));
    let angle = nested!(cpu, 0x0606_780c, sgl::sl_atan(cpu, x, y));
    cpu.write16(o + FIG_FACING, 0x8000u16.wrapping_sub(angle.wrapping_add(QUARTER) as u16));
}

/// The figure's volley: every `VOLLEY_EVERY`[difficulty] frames (by its count), by the beat of
/// 11 such periods (0-5, any later throws nothing) darts thrown from its place, at the direction of
/// its target (+0x188c) a quarter back, in a fan of one (beat 0), two (1, 5), three (2, 4) or
/// four (3).
pub fn figure_volley(cpu: &mut Cpu, o: u32) {
    let target = cpu.read32(o + FIG_TARGET);
    let x = cpu.read32(target + POSITION).wrapping_sub(cpu.read32(o + PARENT_PLACE));
    let y = cpu.read32(target + POSITION + 4).wrapping_sub(cpu.read32(o + PARENT_PLACE + 4));
    let angle = nested!(cpu, 0x0606_780c, sgl::sl_atan(cpu, x, y));
    let base = angle.wrapping_sub(QUARTER) & 0xffff;
    let difficulty = cpu.read8(DIFFICULTY) as i8 as i32 as u32;
    let every = cpu.read8(VOLLEY_EVERY + difficulty) as u32;
    let count = cpu.read16(o + FIG_COUNT) as u32;
    if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, count, every)) != 0 {
        return;
    }
    let count = cpu.read16(o + FIG_COUNT) as u32;
    let round = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, count, every.wrapping_mul(11)));
    let beat = nested!(cpu, 0x0600_6a78, math::quotient(cpu, round, every));
    let place = o + PARENT_PLACE;
    let turns: &[u32] = match beat {
        3 => &[0xb000, 0x9000, 0x7000, 0x5000],
        2 | 4 => &[0xa000, 0x8000, 0x6000],
        1 | 5 => &[0x9000, 0x7000],
        0 => &[0x8000],
        _ => &[],
    };
    for &turn in turns {
        nested!(cpu, 0x0608_20ec, dart_set(cpu, o, place, base.wrapping_add(turn) & 0xffff));
    }
}

// ---- the straight darts and the falling embers ----------------------------------------------------------

/// A shard (list 2, 0x78 bytes, actor_exec's): a dart that flies straight on its speed (+0x6c, +0x70)
/// with its flicker (+0x74) until it leaves the area (184 dots either side across, 120 up and down).
const SHARD_BEHAVIOUR: u32 = 0x0608_2570;
const SHARD_FRAMES: u32 = 0x0609_1030;
/// The area's edges, 16.16: the right and the bottom (flying on them), the left and the top.
const SHARD_RIGHT: u32 = 0x00b7_ffff;
const SHARD_LEFT: u32 = 0xff48_0000;
const SHARD_BOTTOM: u32 = 0x0077_ffff;
const SHARD_TOP: u32 = 0xff88_0000;

/// A shard's behaviour: ended once it is past the edge it flies at (its speed's sign says which),
/// else flickering as a dart's, moved by its speed, its lift -8 and colour the character base's on
/// the even frames, 126 and 16 on the odd, its frame by the game's frames.
pub fn shard_behaviour(cpu: &mut Cpu, o: u32) {
    let (sx, sy) = (cpu.read32(o + STREAK_SPEED) as i32, cpu.read32(o + STREAK_SPEED + 4) as i32);
    let x = cpu.read32(o + POSITION) as i32;
    let out_x = if sx >= 0 { x > SHARD_RIGHT as i32 } else { x <= SHARD_LEFT as i32 };
    let y = cpu.read32(o + POSITION + 4) as i32;
    let out_y = !out_x && if sy >= 0 { y > SHARD_BOTTOM as i32 } else { y <= SHARD_TOP as i32 };
    if out_x || out_y {
        cpu.write8(o + MODE, ENDING);
        return;
    }
    let shows = cpu.read32(GAME_FRAMES) & 1 == cpu.read8(o + STREAK_FLICKER) as u32;
    let flags = cpu.read32(o + FLAGS);
    cpu.write32(o + FLAGS, if shows { flags | FLAG_FAR } else { flags & !FLAG_FAR });
    for k in [0, 4] {
        let v = cpu.read32(o + POSITION + k).wrapping_add(cpu.read32(o + STREAK_SPEED + k));
        cpu.write32(o + POSITION + k, v);
    }
    let colour = cpu.read16(CHAR_BASE);
    if cpu.read32(GAME_FRAMES) & 1 == 0 {
        cpu.write8(o + BYTE_33, -8i8 as u8);
        cpu.write16(o + SPRITE_COLOUR, colour);
    } else {
        cpu.write8(o + BYTE_33, 126);
        cpu.write16(o + SPRITE_COLOUR, colour.wrapping_add(16));
    }
    let c = cpu.read16(o + SPRITE_COLOUR);
    cpu.write16(o + COLOUR_B, c);
    let low = cpu.read32(GAME_FRAMES) & 3;
    let frame = cpu.read8(SHARD_FRAMES + low);
    cpu.write8(o + FRAME, frame);
}

/// A shard thrown from `source` at the place `at` in the direction `angle` at `speed`: the colours
/// 0x40 on the character base's, the flicker the source's parity, turned over.
pub fn shard_set(cpu: &mut Cpu, source: u32, at: u32, angle: u32, speed: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0x78)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + SPRITE));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HIT));
    cpu.write32(o + BEHAVIOUR, SHARD_BEHAVIOUR);
    cpu.write32(o + FLAGS, 0x2008_0400);
    let a = angle as u16 as i16 as i32 as u32;
    let number = cpu.read8(source + NUMBER);
    cpu.write8(o + NUMBER, number);
    for k in [0, 4] {
        let v = cpu.read32(at + k);
        cpu.write32(o + POSITION + k, v);
    }
    cpu.write32(o + POSITION + 8, 0);
    cpu.write8(o + BYTE_33, 8);
    cpu.write8(o + FRAME, 0);
    cpu.write8(o + SPRITE_KIND, 78);
    let colour = cpu.read16(CHAR_BASE).wrapping_add(0x40);
    cpu.write16(o + COLOUR_B, colour);
    cpu.write16(o + SPRITE_COLOUR, colour);
    cpu.write32(o + HIT, DART_HIT_FRAMES);
    cpu.write8(o + ACTOR_54, 1);
    cpu.write8(o + ACTOR_55, 5);
    let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    let vx = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, sin, speed));
    cpu.write32(o + STREAK_SPEED, vx);
    let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    let vy = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, cos, speed.wrapping_neg()));
    cpu.write32(o + STREAK_SPEED + 4, vy);
    let parity = cpu.read8(source + SOURCE_PARITY) ^ 1;
    cpu.write8(source + SOURCE_PARITY, parity);
    cpu.write8(o + STREAK_FLICKER, parity);
}

/// An ember (list 2, 0x78 bytes, actor_exec's) falls down its speed (+0x70) with its scale x easing
/// to 1.0 by 0x1000, ended once it is past 144 dots down.
const RAIN_BEHAVIOUR: u32 = 0x0608_2a90;
const RAIN_HIT_FRAMES: u32 = 0x0609_1034;
const RAIN_FRAMES: u32 = 0x0609_103c;
const FIGURE_RAIN_SPEED: u32 = 0x1890;

/// A falling ember's behaviour: it shows on the frames of its flicker's parity (+0x74), its y goes on
/// by its speed (+0x70), its x scale eases to 1.0 by 0x1000, its frame by the game's frame over 2.
pub fn rain_behaviour(cpu: &mut Cpu, o: u32) {
    if cpu.read32(o + POSITION + 4) as i32 > 0x90_0000 {
        cpu.write8(o + MODE, ENDING);
        return;
    }
    let shows = cpu.read32(GAME_FRAMES) & 1 == cpu.read8(o + STREAK_FLICKER) as u32;
    let flags = cpu.read32(o + FLAGS);
    cpu.write32(o + FLAGS, if shows { flags | FLAG_FAR } else { flags & !FLAG_FAR });
    let s = cpu.read32(o + SCALE_X);
    let y = cpu.read32(o + POSITION + 4).wrapping_add(cpu.read32(o + STREAK_SPEED + 4));
    cpu.write32(o + POSITION + 4, y);
    let s = nested!(cpu, 0x0604_6168, actors::approach(cpu, s, 0x1_0000, 0x1000));
    cpu.write32(o + SCALE_X, s);
    let low = (cpu.read32(GAME_FRAMES) >> 1) & 3;
    let frame = cpu.read8(RAIN_FRAMES + low);
    cpu.write8(o + FRAME, frame);
}

/// A falling ember made by `source` at the place `at`: up to 6 dots off to the right of it, its fall
/// the source's rain speed (+0x1890) and a random 0x5000 more, the source's parity turned over for
/// its flicker.
pub fn rain_set(cpu: &mut Cpu, source: u32, at: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0x78)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + SPRITE));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HIT));
    cpu.write32(o + BEHAVIOUR, RAIN_BEHAVIOUR);
    cpu.write32(o + FLAGS, 0x2008_1420);
    let number = cpu.read8(source + NUMBER);
    cpu.write8(o + NUMBER, number);
    let across = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x6_0000));
    let x = cpu.read32(at).wrapping_add(across);
    cpu.write32(o + POSITION, x);
    let y = cpu.read32(at + 4);
    cpu.write32(o + POSITION + 4, y);
    cpu.write32(o + POSITION + 8, 0);
    cpu.write8(o + BYTE_33, 126);
    cpu.write8(o + FRAME, 8);
    cpu.write8(o + SPRITE_KIND, 78);
    let colour = cpu.read16(CHAR_BASE).wrapping_add(0x70);
    cpu.write16(o + COLOUR_B, colour);
    cpu.write16(o + SPRITE_COLOUR, colour);
    cpu.write32(o + HIT, RAIN_HIT_FRAMES);
    cpu.write8(o + ACTOR_54, 1);
    cpu.write8(o + ACTOR_55, 5);
    cpu.write32(o + SCALE_X, 0);
    cpu.write32(o + SCALE_Y, 0x1_0000);
    let more = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x5000));
    let fall = cpu.read32(source + FIGURE_RAIN_SPEED).wrapping_add(more);
    cpu.write32(o + STREAK_SPEED + 4, fall);
    let parity = cpu.read8(source + SOURCE_PARITY) ^ 1;
    cpu.write8(source + SOURCE_PARITY, parity);
    cpu.write8(o + STREAK_FLICKER, parity);
}

// ---- the ribbon swings ---------------------------------------------------------------------------------------

/// A ribbon (ribbons.rs's, 0x26c bytes) made to swing by `ribbon_swing`: its style +0x250 (a word: the
/// low nibble the kind of swing, bit 4 the way round), its angle +0x252 (a word), the side it goes
/// (+0x254, 1 or -1), the rate its angle turns at +0x258 and its speed +0x7c.
const SWING_STYLE: u32 = 0x250;
const SWING_ANGLE: u32 = 0x252;
const SWING_SIDE: u32 = 0x254;
const SWING_RATE: u32 = 0x258;
const SWING_SPEED: u32 = 0x7c;
const RIBBON_HIT_FLAGS: u32 = 0x56;
const RIBBON_HIT_FIRST: u32 = 0x5a;
const HIT_SOMETHING: u32 = 0x400;
const SWING_MOVER: u32 = 0x0608_2790;
const RIBBON_SIZE: u32 = 0x26c;
const FIGURE_SIDE: u32 = 0x18ac;
const FIGURE_MUZZLE: u32 = 0x398;
const FLARE_END: u32 = 0x0606_3940;

/// The swing mover of a ribbon `o` (ribbons.rs's movers): the ribbon ends (as `FLARE_END` ends it)
/// if it was hit and what it hit has no attribute. Else by the kind of swing its style (+0x250's
/// low nibble) names, its speed eases to a goal and its angle turns, the way the style's bit 4
/// says: kind 1 turns it by 0x500 a frame (the speed to 4 dots by 0x1000), kinds 2 to 4 by a rate
/// that eases to 0 by 16 (the speed to 5, 4 and 3 dots by 0x2000), any other style by 0x2f0
/// (to 4 dots). Then the head moves along x by the side times the speed, and along y by the
/// angle's sine over a radius (3 dots for kinds 1 and 4, else 4).
pub fn ribbon_swing(cpu: &mut Cpu, o: u32) {
    if cpu.read16(o + RIBBON_HIT_FLAGS) as u32 & HIT_SOMETHING != 0 && cpu.read8(o + RIBBON_HIT_FIRST) == 0 {
        nested!(cpu, FLARE_END, ribbons::ribbon_end(cpu, o));
        return;
    }
    let style = cpu.read16(o + SWING_STYLE);
    let way = if style & 0x10 != 0 { -1i32 } else { 1 };
    let kind = (style as u32 & 15).wrapping_sub(1);
    let (goal, step, radius) = match kind {
        0 => (0x4_0000, 0x1000, 0x3_0000),
        1 => (0x5_0000, 0x2000, 0x4_0000),
        3 => (0x3_0000, 0x2000, 0x3_0000),
        _ => (0x4_0000, 0x2000, 0x4_0000),
    };
    let speed = cpu.read32(o + SWING_SPEED);
    let speed = nested!(cpu, 0x0604_6168, actors::approach(cpu, speed, goal, step));
    cpu.write32(o + SWING_SPEED, speed);
    let turn = match kind {
        0 => way * 0x500,
        1..=3 => {
            let rate = cpu.read32(o + SWING_RATE);
            let rate = nested!(cpu, 0x0604_6168, actors::approach(cpu, rate, 0, 16));
            cpu.write32(o + SWING_RATE, rate);
            way.wrapping_mul(rate as i32)
        }
        _ => way * 0x2f0,
    };
    let angle = cpu.read16(o + SWING_ANGLE).wrapping_add(turn as u16);
    cpu.write16(o + SWING_ANGLE, angle);
    let speed = cpu.read32(o + SWING_SPEED);
    let a = cpu.read16(o + SWING_ANGLE) as i16 as i32 as u32;
    let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    let along = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, sin, radius));
    let side = cpu.read32(o + SWING_SIDE);
    let x = cpu.read32(o + POSITION).wrapping_add(side.wrapping_mul(speed));
    cpu.write32(o + POSITION, x);
    let y = cpu.read32(o + POSITION + 4).wrapping_add(along);
    cpu.write32(o + POSITION + 4, y);
}

/// The figure's swinging ribbon made, `kind` its style (+0x250; the low nibble the swing, bit 4
/// the way round): a ribbon (list 2) from a description on the stack, its place the figure's
/// muzzle x (+0x398) and 0, its player the figure's; its angle starts at 0x2000 (-0x2000 for the
/// other way), the swing's rate and speed by the kind, its side by the figure's flag (+0x18ac bit
/// 15); then a flare made at the place.
pub fn figure_ribbon_set(cpu: &mut Cpu, figure: u32, kind: u32) {
    let kind = kind & 0xff;
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, RIBBON_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return;
    }
    cpu.write8(o + 0x6e, 0);
    let d = cpu.stack();
    cpu.write32(d + 0x3c, 0);
    let colour = cpu.read16(CHAR_BASE).wrapping_add(0x20);
    cpu.write16(d + 0x20, colour);
    let number = cpu.read8(figure + NUMBER);
    cpu.write8(d + 0x1e, number);
    let place = cpu.read32(figure + FIGURE_MUZZLE);
    cpu.write32(d, place);
    cpu.write32(d + 4, 0);
    cpu.write32(d + 8, 0);
    nested!(cpu, 0x0606_3620, ribbons::ribbon_init(cpu, o, d, SWING_MOVER));
    cpu.write16(o + SWING_STYLE, kind as u16);
    cpu.write16(o + SWING_ANGLE, if kind & 0x10 != 0 { 0x2000 } else { 0xe000 });
    cpu.write32(o + SWING_RATE, 0);
    match kind & 15 {
        1 => {
            cpu.write32(o + SWING_SPEED, 0xfffd_8000);
        }
        n => {
            if (2..=4).contains(&n) {
                let rate = [0x800, 0x600, 0xa00][n as usize - 2];
                cpu.write32(o + SWING_RATE, rate);
                cpu.write32(o + SWING_SPEED, 0xfffc_0000);
            } else {
                cpu.write32(o + SWING_SPEED, 0xfffd_0000);
            }
        }
    }
    let side = if cpu.read32(figure + FIGURE_SIDE) & 0x8000 == 0 { 1 } else { -1i32 };
    cpu.write32(o + SWING_SIDE, side as u32);
    nested!(cpu, 0x0606_43ec, effects::flare_set(cpu, d));
}

// ---- the flashes -----------------------------------------------------------------------------------------------

/// A flash (list 2, 0xbc bytes, actor_exec's): it swells (+0x8a, a word, by 0x100 a frame; its scales
/// 12 times that) until it passes 0x1fff, when it bursts (a drifting effect of the same size made, its
/// phase +0x7c set), then it shrinks and ends.
const FLASH_PHASE: u32 = 0x7c;
const FLASH_SWELL: u32 = 0x8a;
const FLASH_BEHAVIOUR: u32 = 0x0608_2c2c;
const FLASH_HIT_FRAMES: u32 = 0x0609_1040;
const BURST_SCRIPT: u32 = 0x0607_3158;
const DRIFT_EFFECT: u32 = 0x0605_7454;
const CHAR_FRAME_LOW: u32 = GAME_FRAMES + 3;

/// A flash's behaviour: swelling, its frame by the game's frame; at the top the burst (an effect of
/// colours 48 on, script `BURST_SCRIPT`, turned 0x8000, scaled by the flash's, its lift 126), the
/// flash then a sprite of frame 0xb3 with scales 2 and 1. Afterwards it flickers its frame (0xb4 and
/// colour 0x30 on the even frames, 0xb3 and 0x70 on the odd), its scale x shrinking by 0x500 and y
/// growing by 0x1800, flagged 0x80100 once x is 0x4fff or less, and ended once x is below 0.
pub fn flash_behaviour(cpu: &mut Cpu, o: u32) {
    let phase = cpu.read8(o + FLASH_PHASE);
    if phase == 0 {
        let swell = cpu.read16(o + FLASH_SWELL).wrapping_add(0x100);
        cpu.write16(o + FLASH_SWELL, swell);
        let scale = (swell as i16 as i32).wrapping_mul(12) as u32;
        cpu.write32(o + SCALE_X, scale);
        cpu.write32(o + SCALE_Y, scale);
        let low = cpu.read8(CHAR_FRAME_LOW) & 3;
        cpu.write8(o + FRAME, low.wrapping_sub(45));
        if cpu.read16(o + FLASH_SWELL) as i16 <= 0x1fff {
            return;
        }
        cpu.write8(o + FLASH_PHASE, phase.wrapping_add(1));
        let d = cpu.stack();
        for k in [0, 4, 8] {
            let v = cpu.read32(o + POSITION + k);
            cpu.write32(d + k, v);
        }
        cpu.write32(d + actors::E_SCRIPT, BURST_SCRIPT);
        cpu.write16(d + actors::E_COLOUR, 48);
        let scale = cpu.read32(o + SCALE_X);
        cpu.write32(d + actors::E_SCALES, scale);
        cpu.write32(d + actors::E_SCALES + 4, scale);
        cpu.write16(d + actors::E_ANGLE, 0x8000);
        cpu.write32(d + actors::E_SPEED, phase as u32);
        let e = nested!(cpu, DRIFT_EFFECT, actors::drift_effect(cpu, d));
        if e != 0 {
            cpu.write8(e + BYTE_33, 126);
        }
        cpu.write32(o + FLAGS, 0x2000_0400);
        cpu.write8(o + FRAME, 0xb3);
        cpu.write32(o + SCALE_X, 0x2_0000);
        cpu.write32(o + SCALE_Y, 0x1_0000);
        return;
    }
    let colour = cpu.read16(CHAR_BASE);
    if cpu.read32(GAME_FRAMES) & 1 == 0 {
        cpu.write8(o + FRAME, 0xb4);
        cpu.write16(o + SPRITE_COLOUR, colour.wrapping_add(0x30));
    } else {
        cpu.write8(o + FRAME, 0xb3);
        cpu.write16(o + SPRITE_COLOUR, colour.wrapping_add(0x70));
    }
    let c = cpu.read16(o + SPRITE_COLOUR);
    cpu.write16(o + COLOUR_B, c);
    let sx = cpu.read32(o + SCALE_X).wrapping_add(-0x500i32 as u32);
    cpu.write32(o + SCALE_X, sx);
    let sy = cpu.read32(o + SCALE_Y).wrapping_add(0x1800);
    cpu.write32(o + SCALE_Y, sy);
    if sx as i32 <= 0x4fff {
        let flags = cpu.read32(o + FLAGS) | 0x8_0100;
        cpu.write32(o + FLAGS, flags);
    }
    if (sx as i32) < 0 {
        cpu.write8(o + MODE, ENDING);
    }
}

/// A flash made by `source` (list 2, 0xbc bytes): its x and y the nearer player's place 2 frames
/// ago (`aim::target`), then its y set to 144 dots up, lift 126, frame 1, kind 0, the colours 0x20
/// on the character base's, scales 0; the sound 85 played.
pub fn flash_set(cpu: &mut Cpu, source: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0xbc)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + SPRITE));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HIT));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    let number = cpu.read8(source + NUMBER);
    cpu.write8(o + NUMBER, number);
    cpu.write32(o + BEHAVIOUR, FLASH_BEHAVIOUR);
    cpu.write32(o + FLAGS, 0x2008_0400);
    nested!(cpu, 0x0604_0ccc, aim::target(cpu, o, 2, o + POSITION));
    cpu.write32(o + POSITION + 4, 0xff90_0000);
    cpu.write32(o + POSITION + 8, 0);
    cpu.write8(o + BYTE_33, 126);
    cpu.write8(o + FRAME, 1);
    cpu.write8(o + SPRITE_KIND, 0);
    let colour = cpu.read16(CHAR_BASE).wrapping_add(0x20);
    cpu.write16(o + COLOUR_B, colour);
    cpu.write16(o + SPRITE_COLOUR, colour);
    cpu.write32(o + HIT, FLASH_HIT_FRAMES);
    cpu.write8(o + ACTOR_54, 1);
    cpu.write8(o + ACTOR_55, 5);
    cpu.write32(o + SCALE_X, 0);
    cpu.write32(o + SCALE_Y, 0);
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 85));
}

// ---- the gatherers ---------------------------------------------------------------------------------------------

/// A gatherer (list 2, 0xbc bytes, actor_exec's) flies from the place it is made at toward its
/// parent (+0x6c) over 16 frames (count +0x8a, a word; the step +0xac, +0xb0 and the parent's place
/// at the start +0xb4, +0xb8), turning, then (phase +0x7c 1) sits there swelling and shrinking
/// (+0x8c counts its beats, +0xa8 its parity), then (phase 2) shrinks away.
const GATHER_PARENT: u32 = 0x6c;
const GATHER_PHASE: u32 = 0x7c;
const GATHER_COUNT: u32 = 0x8a;
const GATHER_BEAT: u32 = 0x8c;
const GATHER_STEP: u32 = 0xac;
const GATHER_HOME: u32 = 0xb4;
const GATHER_PARITY: u32 = 0xa8;
const GATHER_BEHAVIOUR: u32 = 0x0608_2e5c;
const GATHER_HIT_FRAMES: u32 = 0x0609_1048;
const GATHER_BEATS: u32 = 0x0609_1050;
/// The counter a parent keeps of the gatherers it has made (+0x98), and the word of its that ends
/// them (+0xa8: they end when it is past 0x528).
const PARENT_MADE: u32 = 0x98;
const PARENT_ENDS: u32 = 0xa8;
const GATHER_ENDS_AFTER: i16 = 0x528;

/// A gatherer's behaviour. Phase 0: ended if its parent's mode is past 0x6f or the parent's +0xa8
/// is past 0x528; an afterimage of 4 frames on the even game frames; its place the parent's
/// start less the step times the count (+0x8a, down a frame), turning by 0x1000; at a count of 0 the
/// next phase (its step 0, its flags 0x20000400, its scale x 0x2000, its turn 0). Next phases: the frame
/// 12 to 19 by the game's frame; in phase 1 the scale x eases (by 0x1000) to 0xc000 plus a beat's
/// 0x800 steps, shown on the frames its parity says; it goes on to phase 2 when the parent has gone, the
/// beats pass the difficulty's, or the parent's word is past 0x528; in phase 2 the scale x shrinks by
/// 0x1000 (flagged 0x80000 under 0x5000) and it ends below nothing. In both the scale y follows x
/// and the lift is -8 or 125 by a random bit.
pub fn gather_behaviour(cpu: &mut Cpu, o: u32) {
    let parent = cpu.read32(o + GATHER_PARENT);
    let phase = cpu.read8(o + GATHER_PHASE);
    if phase == 0 {
        if cpu.read8(parent + MODE) as i8 > 0x6f || cpu.read16(parent + PARENT_ENDS) as i16 > GATHER_ENDS_AFTER {
            cpu.write8(o + MODE, ENDING);
            return;
        }
        if cpu.read32(GAME_FRAMES) & 1 == 0 {
            nested!(cpu, 0x0605_7634, explosions::afterimage_set(cpu, o, 0, 4, 0));
        }
        let count = cpu.read16(o + GATHER_COUNT).wrapping_sub(1);
        cpu.write16(o + GATHER_COUNT, count);
        let left = cpu.read16(o + GATHER_COUNT) as i16 as i32;
        for k in [0, 4] {
            let step = cpu.read32(o + GATHER_STEP + k) as i32;
            let v = cpu.read32(o + GATHER_HOME + k).wrapping_sub(step.wrapping_mul(left) as u32);
            cpu.write32(o + POSITION + k, v);
        }
        let turn = cpu.read16(o + TURN).wrapping_add(0x1000);
        cpu.write16(o + TURN, turn);
        if cpu.read16(o + GATHER_COUNT) == 0 {
            cpu.write8(o + GATHER_PHASE, phase.wrapping_add(1));
            cpu.write32(o + FLAGS, 0x2000_0400);
            cpu.write32(o + GATHER_STEP, 0);
            cpu.write32(o + GATHER_STEP + 4, 0x1000);
            cpu.write32(o + SCALE_X, 0x2000);
            cpu.write16(o + TURN, 0);
        }
        return;
    }
    let frame = ((cpu.read32(GAME_FRAMES) >> 2) & 7) + 12;
    cpu.write8(o + FRAME, frame as u8);
    if phase == 1 {
        let beat = cpu.read16(o + GATHER_BEAT) as u32;
        let goal = ((beat & 3) << 11).wrapping_add(0xc000);
        let s = cpu.read32(o + SCALE_X);
        let s = nested!(cpu, 0x0604_6168, actors::approach(cpu, s, goal, 0x1000));
        cpu.write32(o + SCALE_X, s);
        let beat = cpu.read16(o + GATHER_BEAT).wrapping_add(1);
        cpu.write16(o + GATHER_BEAT, beat);
        let shows = cpu.read32(GAME_FRAMES) & 1 == cpu.read16(o + GATHER_PARITY) as u32;
        cpu.write32(o + FLAGS, if shows { 0x2008_0400 } else { 0x2000_0400 });
        let difficulty = cpu.read8(DIFFICULTY) as i8 as i32 as u32;
        let past = cpu.read8(parent + MODE) as i8 > 0x6f
            || cpu.read16(o + GATHER_BEAT) as i16 as i32 > cpu.read8(GATHER_BEATS + difficulty) as i32
            || cpu.read16(parent + PARENT_ENDS) as i16 > GATHER_ENDS_AFTER;
        if past {
            let p = cpu.read8(o + GATHER_PHASE).wrapping_add(1);
            cpu.write8(o + GATHER_PHASE, p);
        }
    } else {
        let s = cpu.read32(o + SCALE_X).wrapping_add(-0x1000i32 as u32);
        cpu.write32(o + SCALE_X, s);
        if s as i32 <= 0x4fff {
            cpu.write32(o + FLAGS, 0x2008_0400);
        }
        if (s as i32) < 0 {
            cpu.write8(o + MODE, ENDING);
        }
    }
    let s = cpu.read32(o + SCALE_X);
    cpu.write32(o + SCALE_Y, s);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write8(o + BYTE_33, if r & 0x100 == 0 { -8i8 as u8 } else { 125 });
}

/// A gatherer made by `source` at the place `at`: its step a sixteenth of the way to the source
/// (and the source's place kept), its turn random, scales 0x2000 and 0x4000, its parity the
/// count of those the source has made (+0x98).
pub fn gather_set(cpu: &mut Cpu, source: u32, at: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0xbc)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + SPRITE));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HIT));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    let number = cpu.read8(source + NUMBER);
    cpu.write8(o + NUMBER, number);
    cpu.write32(o + BEHAVIOUR, GATHER_BEHAVIOUR);
    cpu.write32(o + FLAGS, 0x2008_0400);
    for k in [0, 4] {
        let v = cpu.read32(at + k);
        cpu.write32(o + POSITION + k, v);
    }
    cpu.write32(o + POSITION + 8, 0);
    cpu.write8(o + BYTE_33, 8);
    cpu.write16(o + GATHER_COUNT, 16);
    for k in [0, 4] {
        let d = cpu.read32(source + POSITION + k).wrapping_sub(cpu.read32(o + POSITION + k)) as i32;
        cpu.write32(o + GATHER_STEP + k, ((d + if d < 0 { 15 } else { 0 }) >> 4) as u32);
    }
    for k in [0, 4] {
        let v = cpu.read32(source + POSITION + k);
        cpu.write32(o + GATHER_HOME + k, v);
    }
    cpu.write32(o + GATHER_PARENT, source);
    let turn = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write16(o + TURN, turn as u16);
    cpu.write8(o + FRAME, 12);
    cpu.write8(o + SPRITE_KIND, 78);
    let colour = cpu.read16(CHAR_BASE).wrapping_add(0xc0);
    cpu.write16(o + COLOUR_B, colour);
    cpu.write16(o + SPRITE_COLOUR, colour);
    cpu.write32(o + HIT, GATHER_HIT_FRAMES);
    cpu.write8(o + ACTOR_54, 1);
    cpu.write8(o + ACTOR_55, 5);
    cpu.write32(o + SCALE_X, 0x2000);
    cpu.write32(o + SCALE_Y, 0x4000);
    let turn = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write16(o + TURN, turn as u16);
    let made = cpu.read16(source + PARENT_MADE).wrapping_add(1);
    cpu.write16(source + PARENT_MADE, made);
    cpu.write16(o + GATHER_PARITY, made);
}

// ---- the convergers ----------------------------------------------------------------------------------------------

/// A converger (list 2, 0xbc bytes, actor_exec's, its own draw) flies from its place to the one
/// it was made for (+0x6c's place, 40 frames: +0x8a counts down; the step +0xac, +0xb0, the
/// place at the start +0xb4, +0xb8), turning, then (phase +0x7c 1) swells to a size the
/// difficulty sets, then (phase 2) shrinks away. +0x8c counts the frames of phase 1.
const CONVERGE_BEHAVIOUR: u32 = 0x0608_31a8;
const CONVERGE_DRAW: u32 = 0x0608_3348;
const CONVERGE_HIT_FRAMES: u32 = 0x0609_106c;
const CONVERGE_SIZES: u32 = 0x0609_1058;
const CONVERGE_AFTERIMAGE_FLAG: u32 = 0x100;
const CONVERGE_SOUND: u32 = 85;
/// A converger's flags once it has arrived and when it shrinks.
const CONVERGE_ARRIVED: u32 = 0x00c8_0400;
const CONVERGE_SHRINKING: u32 = 0x0008_0400;
/// How many frames phase 1 lasts, past which it is phase 2.
const CONVERGE_HOLD: i16 = 0xb4;

/// A converger's behaviour. Phase 0: the count down by one (an afterimage of 30 frames made on the
/// odd counts), its place the start less the step times the count, turning by 0x1000; at a count
/// of 0 the next phase (step 0 and 0x1000, flags `CONVERGE_ARRIVED`, scale x 0x2000, turn 0 and
/// the sound 85). Phase 1: it turns by 0x4000 a frame, its scale x eases to the difficulty's
/// size by 0xa00 and y to 0x6000 by 0x100, flag 0x80000 cleared while x is past 0x11fff, and after
/// 0xb4 frames phase 2. Phase 2: turning, the x scale shrinks by 0x1000, flagged
/// `CONVERGE_SHRINKING` under 0x3000, and it ends below 0. Phases 1 and 2 give it a lift of -8 on the even
/// game frames and 125 on the odd.
pub fn converge_behaviour(cpu: &mut Cpu, o: u32) {
    let phase = cpu.read8(o + GATHER_PHASE);
    if phase == 0 {
        let count = cpu.read16(o + GATHER_COUNT).wrapping_sub(1);
        cpu.write16(o + GATHER_COUNT, count);
        if count & 1 != 0 {
            let flags = cpu.read32(o + FLAGS) | CONVERGE_AFTERIMAGE_FLAG;
            cpu.write32(o + FLAGS, flags);
            nested!(cpu, 0x0605_7634, explosions::afterimage_set(cpu, o, 0, 30, 0));
            let flags = cpu.read32(o + FLAGS) & !CONVERGE_AFTERIMAGE_FLAG;
            cpu.write32(o + FLAGS, flags);
        }
        let left = cpu.read16(o + GATHER_COUNT) as i16 as i32;
        for k in [0, 4] {
            let step = cpu.read32(o + GATHER_STEP + k) as i32;
            let v = cpu.read32(o + GATHER_HOME + k).wrapping_sub(step.wrapping_mul(left) as u32);
            cpu.write32(o + POSITION + k, v);
        }
        let turn = cpu.read16(o + TURN).wrapping_add(0x1000);
        cpu.write16(o + TURN, turn);
        if cpu.read16(o + GATHER_COUNT) as i16 != 0 {
            return;
        }
        cpu.write8(o + GATHER_PHASE, phase.wrapping_add(1));
        cpu.write32(o + FLAGS, CONVERGE_ARRIVED);
        cpu.write32(o + GATHER_STEP, 0);
        cpu.write32(o + GATHER_STEP + 4, 0x1000);
        cpu.write32(o + SCALE_X, 0x2000);
        cpu.write16(o + TURN, 0);
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, CONVERGE_SOUND));
        return;
    }
    let turn = cpu.read16(o + TURN).wrapping_add(0x4000);
    cpu.write16(o + TURN, turn);
    if phase == 1 {
        let difficulty = cpu.read8(DIFFICULTY) as i8 as i32 as u32;
        let size = cpu.read32(CONVERGE_SIZES + 4 * difficulty);
        let s = cpu.read32(o + SCALE_X);
        let s = nested!(cpu, 0x0604_6168, actors::approach(cpu, s, size, 0xa00));
        cpu.write32(o + SCALE_X, s);
        let s = cpu.read32(o + SCALE_Y);
        let s = nested!(cpu, 0x0604_6168, actors::approach(cpu, s, 0x6000, 0x100));
        cpu.write32(o + SCALE_Y, s);
        let beat = cpu.read16(o + GATHER_BEAT).wrapping_add(1);
        cpu.write16(o + GATHER_BEAT, beat);
        if cpu.read32(o + SCALE_X) as i32 > 0x1_1fff {
            let flags = cpu.read32(o + FLAGS) & !FLAG_FAR;
            cpu.write32(o + FLAGS, flags);
        }
        if cpu.read16(o + GATHER_BEAT) as i16 > CONVERGE_HOLD {
            let p = cpu.read8(o + GATHER_PHASE).wrapping_add(1);
            cpu.write8(o + GATHER_PHASE, p);
        }
    } else {
        let s = cpu.read32(o + SCALE_X).wrapping_add(-0x1000i32 as u32);
        cpu.write32(o + SCALE_X, s);
        if s as i32 <= 0x2fff {
            cpu.write32(o + FLAGS, CONVERGE_SHRINKING);
        }
        if (s as i32) < 0 {
            cpu.write8(o + MODE, ENDING);
        }
    }
    let even = cpu.read32(GAME_FRAMES) & 1 == 0;
    cpu.write8(o + BYTE_33, if even { -8i8 as u8 } else { 125 });
}

/// A converger's draw (its sprite `s`): four sprites (frame 0xb1 on the even passes, 0xb2 on the
/// odd, lift 8, the colour the character base's) a quarter turn from each other round its turn, at
/// its place with its y scale across and its x scale (and up to 0x1000 more) up, flagged turned
/// when it is (0x80000: 0x20000100); then a glow (frame 0xd6, flags 0x100, a random angle, scales
/// 0x4800) at the same place.
pub fn converge_draw(cpu: &mut Cpu, s: u32) {
    let o = s.wrapping_sub(SPRITE);
    let d = cpu.stack();
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, d));
    let flags = if cpu.read32(o + FLAGS) & FLAG_FAR != 0 { 0x2000_0100 } else { 0x2000_0000 };
    cpu.write32(d + S_FLAGS, flags);
    for k in [0, 4] {
        let v = cpu.read32(o + POSITION + k);
        cpu.write32(d + S_POSITION + k, v);
    }
    cpu.write32(d + S_POSITION + 8, 0);
    let y_scale = cpu.read32(o + SCALE_Y);
    cpu.write32(d + S_SCALES, y_scale);
    let more = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1000));
    let x_scale = cpu.read32(o + SCALE_X).wrapping_add(more);
    cpu.write32(d + S_SCALES + 4, x_scale);
    cpu.write8(d + S_KIND, 0);
    let bit = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000)) & 0x10;
    let flags = cpu.read32(d + S_FLAGS) | bit;
    cpu.write32(d + S_FLAGS, flags);
    let frame = if cpu.read32(LOOPS) & 1 == 0 { 0xb1 } else { 0xb2 };
    cpu.write8(d + S_FRAME, frame);
    let colour = cpu.read16(CHAR_BASE);
    cpu.write16(d + S_COLOUR, colour);
    cpu.write8(d + S_LIFT, 8);
    for i in 0..4u32 {
        let turn = cpu.read16(o + TURN) as i16 as i32 as u32;
        let angle = (i << 14).wrapping_sub(1).wrapping_add(turn);
        cpu.write16(d + S_ANGLE, angle as u16);
        nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
    }
    cpu.write32(d + S_FLAGS, 0x100);
    let angle = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write16(d + S_ANGLE, angle as u16);
    cpu.write8(d + S_FRAME, 0xd6);
    cpu.write32(d + S_SCALES, 0x4800);
    cpu.write32(d + S_SCALES + 4, 0x4800);
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
}

/// A converger made by `source` for the place `at`: it starts at `at` and goes to the source's
/// place in 40 frames (a fortieth of the way a step), its turn random, scales 0x3000 and 0x6000.
pub fn converge_set(cpu: &mut Cpu, source: u32, at: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0xbc)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, ACTOR_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + SPRITE));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HIT));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    let number = cpu.read8(source + NUMBER);
    cpu.write8(o + NUMBER, number);
    cpu.write32(o + BEHAVIOUR, CONVERGE_BEHAVIOUR);
    cpu.write32(o + DRAW, CONVERGE_DRAW);
    cpu.write32(o + FLAGS, 0x2008_0400);
    for k in [0, 4] {
        let v = cpu.read32(at + k);
        cpu.write32(o + POSITION + k, v);
    }
    cpu.write32(o + POSITION + 8, 0);
    cpu.write8(o + BYTE_33, 8);
    cpu.write16(o + GATHER_COUNT, 40);
    for k in [0, 4] {
        let d = cpu.read32(source + POSITION + k).wrapping_sub(cpu.read32(o + POSITION + k)) as i32;
        cpu.write32(o + GATHER_STEP + k, (d / 40) as u32);
    }
    for k in [0, 4] {
        let v = cpu.read32(source + POSITION + k);
        cpu.write32(o + GATHER_HOME + k, v);
    }
    cpu.write32(o + GATHER_PARENT, source);
    cpu.write8(o + FRAME, 1);
    cpu.write8(o + SPRITE_KIND, 0);
    let colour = cpu.read16(CHAR_BASE);
    cpu.write16(o + COLOUR_B, colour);
    cpu.write16(o + SPRITE_COLOUR, colour);
    cpu.write32(o + HIT, CONVERGE_HIT_FRAMES);
    cpu.write8(o + ACTOR_54, 1);
    cpu.write8(o + ACTOR_55, 5);
    cpu.write32(o + SCALE_X, 0x3000);
    cpu.write32(o + SCALE_Y, 0x6000);
    let turn = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write16(o + TURN, turn as u16);
}

// ---- the figure's sphere and its modes ---------------------------------------------------------------------------

/// The figure's fields the sphere and the modes use: its mode (+0x2ac, a byte), the byte +0x574
/// (`FIG_ANGRY`), the sphere it holds (+0x188c, `FIG_TARGET`), the word +0x2c4 whose bit 0x20 the
/// modes set, the life +0x2c0 (and the one kept at +0x50), the pose script's anim at +0x6c.
const FIG_MODE: u32 = 0x2ac;
const FIG_FLAGS: u32 = 0x2c4;
const FIG_LIFE: u32 = 0x2c0;
const FIG_LIFE_KEPT: u32 = 0x50;
const FIG_GAUGE: u32 = 0x2b8;
const FIG_ANIM: u32 = 0x6c;
const FIG_MIRROR: u32 = 0x278;
/// A sphere (list 2, 0xbc bytes, plain_exec's, its own draw): its figure +0x6c, its phase +0x7c, its
/// speed +0xac, its velocity +0x80 and +0x84, its delay +0xa4, the cycles +0xa6 and frames +0xa8.
const SPHERE_BEHAVIOUR: u32 = 0x0608_35b4;
const SPHERE_DRAW: u32 = 0x0608_3934;
const SPHERE_FIGURE: u32 = 0x6c;
const SPHERE_PHASE: u32 = 0x7c;
const SPHERE_SPEED: u32 = 0xac;
const SPHERE_VELOCITY: u32 = 0x80;
const SPHERE_DELAY: u32 = 0xa4;
const SPHERE_CYCLES: u32 = 0xa6;
const SPHERE_FRAMES: u32 = 0xa8;
const SPHERE_SE: u32 = 80;
const SPHERE_SOUND_EVERY: u16 = 32;
/// The frame the sphere goes round in: after it (0x528) a gatherer's remainder by 80 is waited for.
const SPHERE_CYCLE_FRAMES: i16 = 0x527;
const GATHER_SET: u32 = 0x0608_3038;
const CONVERGE_SET: u32 = 0x0608_345c;

/// A sphere's behaviour. It ends its phase (2) when the figure's mode is not 4 (or 13 with +0x574 of
/// 3 only). Phase 0: its speed eases to 0x10000 by 0x400, then phase 1. Phase 1 (and 3): the speed
/// eases to 0x10000 by 0x1000; with a delay (+0xa4) it counts down, else the sound 80 on every
/// 32nd frame and a gatherer (`gather_set`) from the figure's place on every 8th, until frame
/// 0x528, when on each 80th a cycle is counted: past 5 it is phase 2, else phase 3 (its speed
/// 0x20000) and a converger (`converge_set`) is made. Phase 2: the speed eases to 0 by 0x800,
/// and at 0 it ends and lets the figure go (+0x188c cleared). Phase 3: the speed eases to 0 by
/// 0x2000, then phase 1. Last it moves (in phases 0 and 1): its velocity eases toward the speed's
/// share of the sine and cosine of the angle to the nearer player (a fourth of it the step).
pub fn sphere_behaviour(cpu: &mut Cpu, o: u32) {
    let fig = cpu.read32(o + SPHERE_FIGURE);
    let mode = cpu.read8(fig + FIG_MODE);
    if mode != 4 && (mode != 13 || cpu.read8(fig + FIG_ANGRY) == 3) {
        cpu.write8(o + SPHERE_PHASE, 2);
    }
    let place = fig + PARENT_PLACE;
    let mut hold = false;
    match cpu.read8(o + SPHERE_PHASE) {
        0 => {
            let v = cpu.read32(o + SPHERE_SPEED);
            let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0x1_0000, 0x400));
            cpu.write32(o + SPHERE_SPEED, v);
            if v == 0x1_0000 {
                let p = cpu.read8(o + SPHERE_PHASE).wrapping_add(1);
                cpu.write8(o + SPHERE_PHASE, p);
            }
        }
        1 => hold = true,
        2 => {
            let v = cpu.read32(o + SPHERE_SPEED);
            let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0, 0x800));
            cpu.write32(o + SPHERE_SPEED, v);
            if v == 0 {
                cpu.write8(o + MODE, ENDING);
                cpu.write32(fig + FIG_TARGET, 0);
            }
        }
        3 => {
            let v = cpu.read32(o + SPHERE_SPEED);
            let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0, 0x2000));
            cpu.write32(o + SPHERE_SPEED, v);
            if v == 0 {
                cpu.write8(o + SPHERE_PHASE, 1);
            }
            hold = true;
        }
        _ => {}
    }
    if hold {
        sphere_hold(cpu, o, place);
    }
    sphere_move(cpu, o);
}

/// The part of phases 1 and 3 past the speed's easing (see [`sphere_behaviour`]); the move follows
/// it either way.
fn sphere_hold(cpu: &mut Cpu, o: u32, place: u32) {
    let v = cpu.read32(o + SPHERE_SPEED);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0x1_0000, 0x1000));
    cpu.write32(o + SPHERE_SPEED, v);
    if cpu.read16(o + SPHERE_DELAY) as i16 != 0 {
        let d = cpu.read16(o + SPHERE_DELAY) as i16 as i32 as u32;
        let d = nested!(cpu, 0x0604_6168, actors::approach(cpu, d, 0, 1));
        cpu.write16(o + SPHERE_DELAY, d as u16);
        return;
    }
    let frames = cpu.read16(o + SPHERE_FRAMES);
    if frames as i16 > SPHERE_CYCLE_FRAMES {
        let n = cpu.read16(o + SPHERE_FRAMES) as i16 as i32 as u32;
        if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, n, 80)) != 0 {
            return;
        }
        let cycles = cpu.read16(o + SPHERE_CYCLES).wrapping_add(1);
        cpu.write16(o + SPHERE_CYCLES, cycles);
        if cycles as i16 > 5 {
            cpu.write8(o + SPHERE_PHASE, 2);
            return;
        }
        cpu.write8(o + SPHERE_PHASE, 3);
        cpu.write32(o + SPHERE_SPEED, 0x2_0000);
        nested!(cpu, CONVERGE_SET, converge_set(cpu, o, place));
        return;
    }
    if frames & (SPHERE_SOUND_EVERY - 1) == 0 {
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, SPHERE_SE));
    }
    if cpu.read16(o + SPHERE_FRAMES) & 7 != 0 {
        return;
    }
    nested!(cpu, GATHER_SET, gather_set(cpu, o, place));
}

/// A sphere's move and frame count: its velocity eases toward the sine's (x) and cosine's (y)
/// share of twice its speed, by a quarter of that (the cosine's reversed), the place going on
/// by it in phases 0 and 1.
fn sphere_move(cpu: &mut Cpu, o: u32) {
    let angle = nested!(cpu, 0x0605_6ad4, aim::angle_to_target(cpu, o, 25)) as u16 as i16 as i32 as u32;
    let twice = cpu.read32(o + SPHERE_SPEED).wrapping_mul(2) as i32;
    let quarter = (twice + if twice < 0 { 3 } else { 0 }) >> 2;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, angle));
    let step = (nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, quarter as u32)) as i32).wrapping_abs() as u32;
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, angle));
    let goal = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, twice as u32));
    let v = cpu.read32(o + SPHERE_VELOCITY);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, goal, step));
    cpu.write32(o + SPHERE_VELOCITY, v);
    let twice = twice.wrapping_neg();
    let quarter = (twice + if twice < 0 { 3 } else { 0 }) >> 2;
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, angle));
    let step = (nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, c, quarter as u32)) as i32).wrapping_abs() as u32;
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, angle));
    let goal = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, c, twice as u32));
    let v = cpu.read32(o + SPHERE_VELOCITY + 4);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, goal, step));
    cpu.write32(o + SPHERE_VELOCITY + 4, v);
    if cpu.read8(o + SPHERE_PHASE) < 2 {
        let x = cpu.read32(o + POSITION).wrapping_add(cpu.read32(o + SPHERE_VELOCITY));
        cpu.write32(o + POSITION, x);
        let y = cpu.read32(o + POSITION + 4).wrapping_add(cpu.read32(o + SPHERE_VELOCITY + 4));
        cpu.write32(o + POSITION + 4, y);
    }
    let f = cpu.read16(o + SPHERE_FRAMES).wrapping_add(1);
    cpu.write16(o + SPHERE_FRAMES, f);
}

fn whole_u32(v: u32) -> u32 {
    whole(v) as u32
}

/// A point on the sphere's ring (the sphere's draw): the place's whole part plus (x) or less (y) 23 times
/// the sine (cosine) of `angle` (the game's loop count turning it, 128 a loop) at the sphere's speed,
/// as two words at `out` (x, y).
pub fn sphere_point(cpu: &mut Cpu, o: u32, angle: u32, out: u32) {
    let turned = (angle & 0xffff).wrapping_add(cpu.read32(LOOPS).wrapping_mul(128));
    let a = turned as i16 as i32 as u32;
    let speed = cpu.read32(o + SPHERE_SPEED);
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    let dx = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, speed));
    let x = whole_u32(cpu.read32(o + POSITION)).wrapping_add(whole_u32(23u32.wrapping_mul(dx)));
    cpu.write16(out, x as u16);
    let speed = cpu.read32(o + SPHERE_SPEED);
    let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    let dy = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, c, speed));
    let y = whole_u32(cpu.read32(o + POSITION + 4)).wrapping_sub(whole_u32(23u32.wrapping_mul(dy)));
    cpu.write16(out + 2, y as u16);
}

/// A sphere's draw (its sprite `s`): nothing while its flags' top bit is set. Else three draws of
/// a sprite (flags 0x60000102, lift 126, a colour that cycles with the loop count) given four
/// corners (words at +0x2c on, x and y each): the first two the corners of a bar across the
/// sphere (the place's whole part a fixed way across, 176 dots, and the ring's points at 0x2000
/// and 0xa000 turned by the loop count; which way round the loop count's parity says); the
/// second the same from the ring at 0x6000 or 0xe000 to the figure's place; then two sprites
/// at the sphere (kind 78, frames 24 and 25, flags 0x100 on alternate loops, scaled by its speed,
/// turned by the loop count, the second the other way).
pub fn sphere_draw(cpu: &mut Cpu, s: u32) {
    let o = s.wrapping_sub(SPRITE);
    let hidden = cpu.read32(o + FLAGS) & 0x8000_0000;
    if hidden != 0 {
        return;
    }
    let d = cpu.stack();
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, d));
    cpu.write32(d + S_FLAGS, 0x6000_0102);
    cpu.write32(d + 0x24, d + 0x2c);
    cpu.write8(d + S_LIFT, 126);
    let loops = cpu.read32(LOOPS);
    let colour = cpu.read16(CHAR_BASE).wrapping_add((((loops >> 2) & 15) + 64) as u16);
    cpu.write16(d + S_COLOUR, colour);
    for k in [4, 8, 12] {
        cpu.write32(d + k, hidden);
    }
    if loops & 1 == 0 {
        cpu.write16(d + 0x2c, 0xff50);
        let y = whole_u32(cpu.read32(o + POSITION + 4));
        cpu.write16(d + 0x2e, y as u16);
        nested!(cpu, 0x0608_3868, sphere_point(cpu, o, 0xa000, d + 0x30));
        let x = cpu.read16(d + 0x2c) as i16 as i32;
        cpu.write16(d + 0x34, x.wrapping_neg() as u16);
        let y = cpu.read16(d + 0x2e);
        cpu.write16(d + 0x36, y);
        nested!(cpu, 0x0608_3868, sphere_point(cpu, o, 0x2000, d + 0x38));
    } else {
        let x = whole_u32(cpu.read32(o + POSITION));
        cpu.write16(d + 0x2c, x as u16);
        cpu.write16(d + 0x2e, 0xff90);
        nested!(cpu, 0x0608_3868, sphere_point(cpu, o, 0x2000, d + 0x30));
        let x = cpu.read16(d + 0x2c);
        cpu.write16(d + 0x34, x);
        let y = cpu.read16(d + 0x2e) as i16 as i32;
        cpu.write16(d + 0x36, y.wrapping_neg() as u16);
        nested!(cpu, 0x0608_3868, sphere_point(cpu, o, 0xa000, d + 0x38));
    }
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
    let fig = cpu.read32(o + SPHERE_FIGURE);
    let angle = if cpu.read32(LOOPS) & 1 == 0 { 0xe000 } else { 0x6000 };
    nested!(cpu, 0x0608_3868, sphere_point(cpu, o, angle, d + 0x2c));
    let x = cpu.read16(d + 0x2c);
    cpu.write16(d + 0x30, x);
    let y = cpu.read16(d + 0x2e);
    cpu.write16(d + 0x32, y);
    let x = whole_u32(cpu.read32(fig + PARENT_PLACE));
    cpu.write16(d + 0x34, x as u16);
    let y = whole_u32(cpu.read32(fig + PARENT_PLACE + 4));
    cpu.write16(d + 0x36, y as u16);
    cpu.write16(d + 0x38, x as u16);
    cpu.write16(d + 0x3a, y as u16);
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
    let loops = cpu.read32(LOOPS);
    cpu.write32(d + S_FLAGS, 0);
    if loops & 1 != 0 {
        cpu.write32(d + S_FLAGS, 0x100);
    }
    let colour = cpu.read16(CHAR_BASE).wrapping_add((((loops << 2) & 8) + 0x1f0) as u16);
    cpu.write16(d + S_COLOUR, colour);
    let turn = loops.wrapping_mul(128);
    let (x, y) = (cpu.read32(o + POSITION), cpu.read32(o + POSITION + 4));
    cpu.write32(d + S_POSITION, x);
    cpu.write32(d + S_POSITION + 4, y);
    cpu.write16(d + S_ANGLE, turn as u16);
    let speed = cpu.read32(o + SPHERE_SPEED);
    cpu.write32(d + S_SCALES, speed);
    let speed = cpu.read32(o + SPHERE_SPEED);
    cpu.write32(d + S_SCALES + 4, speed);
    cpu.write8(d + S_KIND, 78);
    cpu.write8(d + S_FRAME, 24);
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
    let loops = cpu.read32(LOOPS);
    cpu.write32(d + S_FLAGS, 0);
    if loops & 1 == 0 {
        cpu.write32(d + S_FLAGS, 0x100);
    }
    let colour = cpu.read16(CHAR_BASE).wrapping_add((((loops << 2) & 8) + 0x20) as u16);
    cpu.write16(d + S_COLOUR, colour);
    cpu.write16(d + S_ANGLE, loops.wrapping_mul(-0x600i32 as u32) as u16);
    cpu.write8(d + S_FRAME, 25);
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, d));
}

/// A sphere made for the figure `fig` (list 2, 0xbc bytes, plain_exec's, its own draw): flagged
/// 0x00c80400, at the figure's place, its delay 30 frames; the figure keeps it (+0x188c). The
/// figure's hold is cleared if there is no room.
pub fn sphere_set(cpu: &mut Cpu, fig: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0xbc)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, PLAIN_EXEC));
    if o == 0 {
        cpu.write32(fig + FIG_TARGET, o);
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + SPRITE));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + BEHAVIOUR, SPHERE_BEHAVIOUR);
    cpu.write32(o + FLAGS, 0x00c8_0400);
    cpu.write32(o + DRAW, SPHERE_DRAW);
    cpu.write32(o + SPHERE_FIGURE, fig);
    let x = cpu.read32(fig + PARENT_PLACE);
    cpu.write32(o + POSITION, x);
    let y = cpu.read32(fig + PARENT_PLACE + 4);
    cpu.write32(o + POSITION + 4, y);
    cpu.write32(o + POSITION + 8, 0);
    cpu.write32(fig + FIG_TARGET, o);
    cpu.write16(o + SPHERE_DELAY, 30);
}

/// The figure turned toward its sphere (+0x188c, if it holds one): the way it faces (+0x1882)
/// the angle from its place to the sphere's, reversed (0x8000 less); then, by the nearer player's
/// distance in x from it, a place (+0x550) it moves to: mirrored (+0x278), the mirror of the
/// other side's, with the facing turned over; not mirrored: 0xe80000 less the distance, or
/// 0x880000 when the player is 0x600000 or more away.
pub fn figure_face_sphere(cpu: &mut Cpu, fig: u32) {
    let sphere = cpu.read32(fig + FIG_TARGET);
    if sphere == 0 {
        return;
    }
    let y = cpu.read32(sphere + POSITION + 4).wrapping_sub(cpu.read32(fig + PARENT_PLACE + 4));
    let x = cpu.read32(sphere + POSITION).wrapping_sub(cpu.read32(fig + PARENT_PLACE));
    let angle = nested!(cpu, 0x0606_780c, sgl::sl_atan(cpu, x, y));
    cpu.write16(fig + FIG_FACE_ORB, 0x8000u16.wrapping_sub(angle as u16));
    let p = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, fig + SPRITE));
    let dx = cpu.read32(fig + PARENT_PLACE).wrapping_sub(cpu.read32(p + POSITION)) as i32;
    if cpu.read8(fig + FIG_MIRROR) != 0 {
        let to = if dx > -0x60_0000 { (-0xe8_0000i32).wrapping_sub(dx) } else { -0x88_0000 };
        cpu.write32(fig + FIG_AT, to as u32);
        let f = cpu.read16(fig + FIG_FACE_ORB) as i16 as i32;
        cpu.write16(fig + FIG_FACE_ORB, 0x8000u32.wrapping_sub(f as u32) as u16);
    } else {
        let to = if dx > 0x5f_ffff { 0x88_0000 } else { 0xe8_0000 - dx };
        cpu.write32(fig + FIG_AT, to as u32);
    }
}
/// The figure's place it moves to (+0x550, x) and the angle it faces the sphere at (+0x1882, a word).
const FIG_AT: u32 = 0x550;
const FIG_Y_AT: u32 = 0x554;
const FIG_FACE_ORB: u32 = 0x1882;

// ---- the figure's modes -----------------------------------------------------------------------------------------

/// The figure's other fields the modes set: the bytes +0x575 and +0x576 (cleared or set to 1 or
/// 0), +0x579 and +0x57a (8 or 10), +0x57b and +0x57c (0 or 4), the counts +0x1780 and +0x1784
/// (and their speeds +0x178c, +0x1790), the scale +0x18b8, the pull +0x1890, +0x1894, +0x1898, the
/// frame count +0x2b0 (`FIG_COUNT`).
const FIG_BYTE_575: u32 = 0x575;
const FIG_BYTE_576: u32 = 0x576;
const FIG_BYTE_579: u32 = 0x579;
const FIG_BYTE_57A: u32 = 0x57a;
const FIG_BYTE_57B: u32 = 0x57b;
const FIG_BYTE_57C: u32 = 0x57c;
const FIG_EASE_X: u32 = 0x1780;
const FIG_EASE_Y: u32 = 0x1784;
const FIG_EASE_SPEED_X: u32 = 0x178c;
const FIG_EASE_SPEED_Y: u32 = 0x1790;
const FIG_SCALE: u32 = 0x18b8;
const FIG_PULL: u32 = 0x1890;
const FIG_PLACE_Z: u32 = 0x1874;
const FIG_SHRINK: u32 = 0x29c;
const FIG_BURST_FLAGS: u32 = 0x8000_0400;
const SCREEN_FLAGS: u32 = 0x0607_405c;
const TWIN_SET: u32 = 0x0605_8278;
const FIG_SPARKS: i8 = 60;
const FIG_SPARK_RISE: u32 = 0xff50_0000;
const BURST_SOUND: u32 = 19;

/// The figure's burst (as it ends): the screen's flags given 0x80000400, the fades stopped, the
/// offset set to 50, 50, 100 (100, 50, 50 if the life +0x2c0 is not past 0) and faded to nothing
/// at 0x200; 60 twin sparks at its place (z 0xb0 dots on); the sound 19.
pub fn figure_burst(cpu: &mut Cpu, fig: u32) {
    let flags = cpu.read32(SCREEN_FLAGS) | FIG_BURST_FLAGS;
    cpu.write32(SCREEN_FLAGS, flags);
    nested!(cpu, 0x0605_e9d8, screen::stop_fades(cpu));
    if cpu.read32(fig + FIG_LIFE) as i32 > 0 {
        nested!(cpu, 0x0605_e6dc, screen::set_fade_offset(cpu, 50, 50, 100));
    } else {
        nested!(cpu, 0x0605_e6dc, screen::set_fade_offset(cpu, 100, 50, 50));
    }
    nested!(cpu, 0x0605_e93c, screen::fade_to(cpu, 0, 0, 0, 0x200));
    let p = cpu.stack();
    let x = cpu.read32(fig + PARENT_PLACE);
    cpu.write32(p, x);
    let y = cpu.read32(fig + PARENT_PLACE + 4);
    cpu.write32(p + 4, y);
    let z = cpu.read32(fig + FIG_PLACE_Z).wrapping_add(FIG_SPARK_RISE);
    cpu.write32(p + 8, z);
    let mut left = FIG_SPARKS;
    loop {
        nested!(cpu, TWIN_SET, explosions::twin_set(cpu, p));
        left = left.wrapping_sub(1);
        if left <= 0 {
            break;
        }
    }
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, BURST_SOUND));
}

/// The fades stopped, the offset set to 100, 80, 40 and faded to nothing at 0x80.
pub fn warm_fade(cpu: &mut Cpu) {
    nested!(cpu, 0x0605_e9d8, screen::stop_fades(cpu));
    nested!(cpu, 0x0605_e6dc, screen::set_fade_offset(cpu, 100, 80, 40));
    nested!(cpu, 0x0605_e93c, screen::fade_to(cpu, 0, 0, 0, 0x80));
}

/// The figure ended: the boss gauge's life 0, its count (+0x2b8) 0, its burst, the life kept
/// (+0x50) and the life +0x2c0 cleared.
pub fn figure_wreck(cpu: &mut Cpu, fig: u32) {
    nested!(cpu, 0x0604_64b8, hud::boss_gauge_life(cpu, 0));
    cpu.write32(fig + FIG_GAUGE, 0);
    nested!(cpu, 0x0608_3cf0, figure_burst(cpu, fig));
    let life = cpu.read32(fig + FIG_LIFE);
    cpu.write32(fig + FIG_LIFE_KEPT, life);
    cpu.write32(fig + FIG_LIFE, 0);
}

/// The word +0x2c4 of the figure with `bits` set.
fn fig_flags_set(cpu: &mut Cpu, fig: u32, bits: u16) {
    let f = cpu.read16(fig + FIG_FLAGS) | bits;
    cpu.write16(fig + FIG_FLAGS, f);
}

/// The figure's pose script started (its anim at +0x6c).
fn fig_script(cpu: &mut Cpu, fig: u32, script: u32) {
    nested!(cpu, 0x0605_6628, actors::script_start(cpu, fig + FIG_ANIM, script));
}

/// Mode 3: its flags' 0x20 set, its pose script 0x06090bee, the scale +0x18b8 0x10000, the
/// counts +0x1780 and +0x1784 and the way it faces (+0x1882, +0x187a) cleared, the bytes
/// +0x575, +0x576, +0x57b, +0x57c cleared, the pull +0x1890 `pull`, +0x1894 cleared.
pub fn figure_mode_3(cpu: &mut Cpu, fig: u32, pull: u32) {
    cpu.write8(fig + FIG_MODE, 3);
    fig_script(cpu, fig, 0x0609_0bee);
    cpu.write32(fig + FIG_SCALE, 0x1_0000);
    cpu.write32(fig + FIG_EASE_Y, 0);
    cpu.write32(fig + FIG_EASE_X, 0);
    cpu.write16(fig + FIG_FACE_ORB, 0);
    cpu.write16(fig + FIG_FACING, 0);
    cpu.write8(fig + FIG_BYTE_576, 0);
    cpu.write8(fig + FIG_BYTE_575, 0);
    cpu.write8(fig + FIG_BYTE_57B, 0);
    cpu.write8(fig + FIG_BYTE_57C, 0);
    cpu.write32(fig + FIG_PULL, pull & 0xff);
    cpu.write32(fig + FIG_PULL + 4, 0);
    fig_flags_set(cpu, fig, 0x20);
}

/// Mode 4: its pose script 0x06090c16, its flags' 0x20 cleared, +0x579 and +0x57a 8, +0x554
/// -0x600000, the counts +0x2b8 and +0x188c (the sphere) cleared, and the
/// gauge made for its life kept (+0x50) and 0x13880 more (the life +0x2c0).
pub fn figure_mode_4(cpu: &mut Cpu, fig: u32) {
    cpu.write8(fig + FIG_MODE, 4);
    fig_script(cpu, fig, 0x0609_0c16);
    let f = cpu.read16(fig + FIG_FLAGS) & 0xffdf;
    cpu.write16(fig + FIG_FLAGS, f);
    cpu.write8(fig + FIG_BYTE_579, 8);
    cpu.write8(fig + FIG_BYTE_57A, 8);
    cpu.write32(fig + FIG_Y_AT, 0xffa0_0000);
    cpu.write32(fig + FIG_GAUGE, 0);
    cpu.write32(fig + FIG_TARGET, 0);
    let life = cpu.read32(fig + FIG_LIFE_KEPT).wrapping_add(0x1_3880);
    cpu.write32(fig + FIG_LIFE, life);
    nested!(cpu, 0x0604_64d8, hud::boss_gauge_set(cpu, life));
}

/// Mode 6: its flags' 0x20 set, the byte +0x574 cleared, its pose script 0x06090c70.
pub fn figure_mode_6(cpu: &mut Cpu, fig: u32) {
    cpu.write8(fig + FIG_MODE, 6);
    cpu.write8(fig + FIG_ANGRY, 0);
    fig_flags_set(cpu, fig, 0x20);
    fig_script(cpu, fig, 0x0609_0c70);
}

/// Mode 10: +0x574 cleared, the count +0x298 and the easing speeds cleared, its flags' 0x20 set,
/// +0x575 and +0x576 1, +0x579 and +0x57a 10, the pull cleared, +0x57b and +0x57c 4, its pose script
/// 0x06090d0a, the sound 74; it faces the other way (+0x278 cleared).
pub fn figure_mode_10(cpu: &mut Cpu, fig: u32) {
    cpu.write8(fig + FIG_MODE, 10);
    cpu.write8(fig + FIG_ANGRY, 0);
    cpu.write32(fig + 0x298, 0);
    cpu.write32(fig + FIG_EASE_SPEED_X, 0);
    cpu.write32(fig + FIG_EASE_SPEED_Y, 0);
    fig_flags_set(cpu, fig, 0x20);
    cpu.write8(fig + FIG_BYTE_575, 1);
    cpu.write8(fig + FIG_BYTE_576, 1);
    cpu.write8(fig + FIG_BYTE_579, 10);
    cpu.write8(fig + FIG_BYTE_57A, 10);
    cpu.write32(fig + FIG_PULL, 0);
    cpu.write8(fig + FIG_BYTE_57B, 4);
    cpu.write8(fig + FIG_BYTE_57C, 4);
    fig_script(cpu, fig, 0x0609_0d0a);
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 74));
    cpu.write8(fig + FIG_MIRROR, 0);
}

/// Mode 14: +0x574 cleared, its flags' 0x20 set, the count +0x2b0 and +0x18ac cleared, +0x575 and
/// +0x576 cleared, its pose script 0x06090c08.
pub fn figure_mode_14(cpu: &mut Cpu, fig: u32) {
    cpu.write8(fig + FIG_MODE, 14);
    cpu.write8(fig + FIG_ANGRY, 0);
    fig_flags_set(cpu, fig, 0x20);
    cpu.write16(fig + FIG_COUNT, 0);
    cpu.write32(fig + 0x18ac, 0);
    cpu.write8(fig + FIG_BYTE_575, 0);
    cpu.write8(fig + FIG_BYTE_576, 0);
    fig_script(cpu, fig, 0x0609_0c08);
}

/// Mode 15: +0x574 and the count +0x2b0 cleared, its flags' 0x20 set, +0x579 and +0x57a 8, the
/// scale +0x18b8 0x10000, the counts and the way it faces cleared, the bytes +0x575, +0x576, +0x57b,
/// +0x57c cleared, the shrink +0x294 0 and +0x298 -0x80000, +0x29c -0x20000, its pose script
/// 0x06090eb4.
pub fn figure_mode_15(cpu: &mut Cpu, fig: u32) {
    cpu.write8(fig + FIG_MODE, 15);
    cpu.write8(fig + FIG_ANGRY, 0);
    cpu.write16(fig + FIG_COUNT, 0);
    fig_flags_set(cpu, fig, 0x20);
    cpu.write8(fig + FIG_BYTE_579, 8);
    cpu.write8(fig + FIG_BYTE_57A, 8);
    cpu.write32(fig + FIG_SCALE, 0x1_0000);
    cpu.write32(fig + FIG_EASE_Y, 0);
    cpu.write32(fig + FIG_EASE_X, 0);
    cpu.write16(fig + FIG_FACE_ORB, 0);
    cpu.write16(fig + FIG_FACING, 0);
    cpu.write8(fig + FIG_BYTE_576, 0);
    cpu.write8(fig + FIG_BYTE_575, 0);
    cpu.write8(fig + FIG_BYTE_57B, 0);
    cpu.write8(fig + FIG_BYTE_57C, 0);
    cpu.write32(fig + 0x294, 0);
    cpu.write32(fig + 0x298, 0xfff8_0000);
    cpu.write32(fig + FIG_SHRINK, 0xfffe_0000);
    fig_script(cpu, fig, 0x0609_0eb4);
}

/// Mode 17: +0x574 and the count +0x2b0 cleared, the way it faces (+0x278) a random bit, its
/// flags' 0x20 set, +0x579 and +0x57a 10, its pose script 0x06090eee.
pub fn figure_mode_17(cpu: &mut Cpu, fig: u32) {
    cpu.write8(fig + FIG_MODE, 17);
    cpu.write8(fig + FIG_ANGRY, 0);
    cpu.write16(fig + FIG_COUNT, 0);
    let bit = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000)) & 1;
    cpu.write8(fig + FIG_MIRROR, bit as u8);
    fig_flags_set(cpu, fig, 0x20);
    cpu.write8(fig + FIG_BYTE_579, 10);
    cpu.write8(fig + FIG_BYTE_57A, 10);
    fig_script(cpu, fig, 0x0609_0eee);
}

/// Mode 20: +0x574 and the count +0x2b0 cleared, its flags' 0x20 set, +0x29c 0x20000, its pose
/// script 0x06090f38, +0x579 and +0x57a 8.
pub fn figure_mode_20(cpu: &mut Cpu, fig: u32) {
    cpu.write8(fig + FIG_MODE, 20);
    cpu.write8(fig + FIG_ANGRY, 0);
    cpu.write16(fig + FIG_COUNT, 0);
    fig_flags_set(cpu, fig, 0x20);
    cpu.write32(fig + FIG_SHRINK, 0x2_0000);
    fig_script(cpu, fig, 0x0609_0f38);
    cpu.write8(fig + FIG_BYTE_579, 8);
    cpu.write8(fig + FIG_BYTE_57A, 8);
}

/// Mode 23: the count +0x2b0 cleared, its flags' 0x20 and 0x40 set, +0x29c -0x18000, +0x579 and
/// +0x57a 8, its pose script 0x06090fb4.
pub fn figure_mode_23(cpu: &mut Cpu, fig: u32) {
    cpu.write8(fig + FIG_MODE, 23);
    cpu.write16(fig + FIG_COUNT, 0);
    fig_flags_set(cpu, fig, 0x60);
    cpu.write32(fig + FIG_SHRINK, 0xfffe_8000);
    cpu.write8(fig + FIG_BYTE_579, 8);
    cpu.write8(fig + FIG_BYTE_57A, 8);
    fig_script(cpu, fig, 0x0609_0fb4);
}

// ---- the figure's steps -----------------------------------------------------------------------------------------

/// The figure's head and arm angles (+0x51e, +0x520, +0x522, words) turning toward those aimed
/// at (+0x53c, +0x53e, +0x540) at speeds (+0x530, +0x532, +0x534) that ease to goals
/// (+0x536, +0x538, +0x53a); the places +0x524, +0x528, +0x52c with their steps (+0x544, +0x548,
/// +0x54c) and the places aimed at (+0x550, +0x554, +0x558).
const FIG_HEAD: u32 = 0x51e;
const FIG_HEAD_AIM: u32 = 0x53c;
const FIG_HEAD_SPEED: u32 = 0x530;
const FIG_HEAD_GOAL: u32 = 0x536;
const FIG_LIMB: u32 = 0x524;
const FIG_LIMB_STEP: u32 = 0x544;
const FIG_LIMB_AIM: u32 = 0x550;
const FIG_LIMB_QUOTIENT: u32 = 0x300;
const FIG_LIMB_REACH: u32 = 48;

/// Angle `k` of the figure's head and arms turned: the word at `FIG_HEAD`, its speed and the
/// speed's goal at `FIG_HEAD_SPEED` and `FIG_HEAD_GOAL`, aimed at the word at `FIG_HEAD_AIM`, by
/// at most 0x1000 with 10 and 32 for the easing.
fn turn_head(cpu: &mut Cpu, o: u32, k: u32) {
    let to = cpu.read16(o + FIG_HEAD_AIM + 2 * k) as u32;
    let from = cpu.read16(o + FIG_HEAD + 2 * k) as u32;
    let (speed, goal) = (o + FIG_HEAD_SPEED + 2 * k, o + FIG_HEAD_GOAL + 2 * k);
    cpu.nested_with_stack(&TURN_TOWARD_PROLOGUE, TURN_TOWARD, &[0x1000, 10, 32], |cpu| {
        aim::turn_toward(cpu, speed, goal, from, to, 0x1000, 10, 32)
    });
}

/// One of the figure's places eased toward the place aimed at: the step (at `FIG_LIMB_STEP + 4 k`)
/// goes toward 1/48 of the way there (the gap, negated, by 48) by 1/0x300 of the gap's size.
fn ease_limb(cpu: &mut Cpu, o: u32, k: u32) {
    let gap = cpu.read32(o + FIG_LIMB + 4 * k).wrapping_sub(cpu.read32(o + FIG_LIMB_AIM + 4 * k));
    let size = nested!(cpu, 0x0600_6a78, math::quotient(cpu, gap, FIG_LIMB_QUOTIENT)) as i32;
    let size = size.wrapping_abs() as u32;
    let goal = nested!(cpu, 0x0600_6a78, math::quotient(cpu, gap.wrapping_neg(), FIG_LIMB_REACH));
    let step = cpu.read32(o + FIG_LIMB_STEP + 4 * k);
    let step = nested!(cpu, 0x0604_6168, actors::approach(cpu, step, goal, size));
    cpu.write32(o + FIG_LIMB_STEP + 4 * k, step);
}

/// The figure's head, arms and body settled: the three angles turned (`turn_head`), the three
/// places' steps eased (`ease_limb`) and the angles and places moved on by their speeds and steps.
pub fn figure_settle(cpu: &mut Cpu, o: u32) {
    for k in 0..3 {
        turn_head(cpu, o, k);
    }
    for k in 0..3 {
        ease_limb(cpu, o, k);
    }
    for k in 0..3 {
        let a = cpu.read16(o + FIG_HEAD + 2 * k).wrapping_add(cpu.read16(o + FIG_HEAD_SPEED + 2 * k));
        cpu.write16(o + FIG_HEAD + 2 * k, a);
    }
    for k in 0..3 {
        let p = cpu.read32(o + FIG_LIMB + 4 * k).wrapping_add(cpu.read32(o + FIG_LIMB_STEP + 4 * k));
        cpu.write32(o + FIG_LIMB + 4 * k, p);
    }
}

/// An object's place (x, y) wrapped into the range of a 0x2000000 span: past 0x1000000 it goes
/// back by a span, below -0x1000000 on by one.
pub fn wrap_place(cpu: &mut Cpu, o: u32) {
    for k in [POSITION, POSITION + 4] {
        let mut v = cpu.read32(o + k) as i32;
        if v > 0x100_0000 {
            v = v.wrapping_add(-0x200_0000);
            cpu.write32(o + k, v as u32);
        }
        if v < -0x100_0000 {
            cpu.write32(o + k, v.wrapping_add(0x200_0000) as u32);
        }
    }
}

/// The figure's drop toward the floor: its shrink (+0x29c) eased to 0x40000 by 0x1600, and, when the
/// place its mirror says (+0x564 mirrored, else +0x570) is past 0x1350000, its z (+0x1c) moved back
/// by what it is past.
pub fn figure_sink(cpu: &mut Cpu, o: u32) {
    let reach = if cpu.read8(o + FIG_MIRROR) != 0 { cpu.read32(o + 0x564) } else { cpu.read32(o + 0x570) } as i32;
    let v = cpu.read32(o + FIG_SHRINK);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0x4_0000, 0x1600));
    cpu.write32(o + FIG_SHRINK, v);
    if reach > 0x135_0000 {
        let z = cpu.read32(o + POSITION + 8).wrapping_add(0x135_0000).wrapping_sub(reach as u32);
        cpu.write32(o + POSITION + 8, z);
    }
}

/// Whether the figure is sinking and has reached the floor: its shrink is above 0 and the place its
/// mirror says (+0x564 mirrored, else +0x570) is past 0x1350000.
pub fn figure_sunk(cpu: &mut Cpu, o: u32) -> u32 {
    let reach = if cpu.read8(o + FIG_MIRROR) != 0 { cpu.read32(o + 0x564) } else { cpu.read32(o + 0x570) } as i32;
    (cpu.read32(o + FIG_SHRINK) as i32 > 0 && reach > 0x135_0000) as u32
}

/// The figure's place to move to (+0x550) eased toward the nearer player's x, by 0x20000.
pub fn figure_follow_player(cpu: &mut Cpu, o: u32) {
    let p = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + SPRITE));
    let from = cpu.read32(o + FIG_AT);
    let to = cpu.read32(p + POSITION);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, from, to, 0x2_0000));
    cpu.write32(o + FIG_AT, v);
}

/// The figure's easing of two counts (+0x1780 toward `a`, +0x1784 toward `b`): each, when its goal
/// is not 0 or it is not at 0 yet, has its speed (+0x178c, +0x1790) eased toward a thirty-second of the
/// way (the gap by 32, negated) by a 0x200th of the gap's size, and moves by that speed.
pub fn figure_ease(cpu: &mut Cpu, o: u32, a: u32, b: u32) {
    for (goal, count, speed) in [(a, FIG_EASE_X, FIG_EASE_SPEED_X), (b, FIG_EASE_Y, FIG_EASE_SPEED_Y)] {
        if goal == 0 && cpu.read32(o + count) == 0 {
            continue;
        }
        let gap = cpu.read32(o + count).wrapping_sub(goal);
        let size = nested!(cpu, 0x0600_6a78, math::quotient(cpu, gap, 0x200)) as i32;
        let size = size.wrapping_abs() as u32;
        let to = nested!(cpu, 0x0600_6a78, math::quotient(cpu, gap.wrapping_neg(), 32));
        let v = cpu.read32(o + speed);
        let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, to, size));
        cpu.write32(o + speed, v);
        let c = cpu.read32(o + count).wrapping_add(v);
        cpu.write32(o + count, c);
    }
}

/// The figure's aim (+0x540, a word) set a quarter and a half-quarter (0xc000) on from the angle
/// to the nearer player (its place less the figure's +0x2f0, +0x2f4).
pub fn figure_aim(cpu: &mut Cpu, o: u32) {
    let p = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + SPRITE));
    let y = cpu.read32(p + POSITION + 4).wrapping_sub(cpu.read32(o + 0x2f4));
    let x = cpu.read32(p + POSITION).wrapping_sub(cpu.read32(o + 0x2f0));
    let angle = nested!(cpu, 0x0606_780c, sgl::sl_atan(cpu, x, y));
    cpu.write16(o + FIG_HEAD_AIM + 4, 0xc000u32.wrapping_add(angle) as u16);
}

/// The figure's mode on one (the next), its pull +0x1890 0xc000, +0x1894 and +0x1898 cleared.
pub fn figure_next_mode(cpu: &mut Cpu, o: u32) {
    let m = cpu.read8(o + FIG_MODE).wrapping_add(1);
    cpu.write8(o + FIG_MODE, m);
    cpu.write32(o + FIG_PULL, 0xc000);
    cpu.write32(o + FIG_PULL + 4, 0);
    cpu.write32(o + FIG_PULL + 8, 0);
}

/// The figure's rise (its z, +0x1c, the height it climbs): its poses stepped; the angle it
/// turns its head to (+0x53c) eased to 0xa000, 0xc000 or 0xe000 as the height's quarter-turns
/// (the top bits of its negation) say, by a 0x80th; its arms' aims (+0x53e, +0x540) the height by
/// 0x280 and 0x200; its places to move to (+0x558 -0x400000 less the height, +0x554 by 0x3000 less);
/// its shrink toward 0x30000 by 0x400; its place wrapped and its limbs settled. Once the height
/// is above 0 its mode goes on one, the scale +0x18b8 0x20000, the pose script 0x06090bbc, the
/// pull cleared; the screen flashed (offset 255, faded to nothing in 60) and the sound 79.
pub fn figure_rise(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_0290, pose_step(cpu, o + FIG_ANIM, o));
    let height = cpu.read32(o + POSITION + 8);
    let turns = (height.wrapping_neg() & 0x0700_0000) >> 24;
    let goal = match turns {
        1 | 5 => 0xe000,
        3 | 7 => 0xa000,
        _ => 0xc000,
    };
    let head = cpu.read16(o + FIG_HEAD_AIM) as u32;
    let diff = nested!(cpu, 0x0604_6194, camera::angle_diff(cpu, head, goal));
    let step = nested!(cpu, 0x0600_6a78, math::quotient(cpu, diff, 0x80));
    let head = (cpu.read16(o + FIG_HEAD_AIM) as i16 as i32 as u32).wrapping_add(step);
    cpu.write16(o + FIG_HEAD_AIM, head as u16);
    let a = nested!(cpu, 0x0600_6a78, math::quotient(cpu, height, 0x280));
    cpu.write16(o + FIG_HEAD_AIM + 2, a as u16);
    let b = nested!(cpu, 0x0600_6a78, math::quotient(cpu, height, 0x200));
    cpu.write16(o + FIG_HEAD_AIM + 4, b as u16);
    cpu.write32(o + FIG_LIMB_AIM + 8, 0xffc0_0000u32.wrapping_sub(height));
    let y = cpu.read32(o + FIG_Y_AT).wrapping_add(-0x3000i32 as u32);
    cpu.write32(o + FIG_Y_AT, y);
    let v = cpu.read32(o + FIG_SHRINK);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0x3_0000, 0x400));
    cpu.write32(o + FIG_SHRINK, v);
    nested!(cpu, 0x0608_442c, wrap_place(cpu, o));
    nested!(cpu, 0x0608_4258, figure_settle(cpu, o));
    if cpu.read32(o + POSITION + 8) as i32 <= 0 {
        return;
    }
    let m = cpu.read8(o + FIG_MODE).wrapping_add(1);
    cpu.write8(o + FIG_MODE, m);
    cpu.write32(o + FIG_SCALE, 0x2_0000);
    fig_script(cpu, o, 0x0609_0bbc);
    cpu.write32(o + FIG_PULL, 0);
    cpu.write32(o + FIG_PULL + 8, 0);
    nested!(cpu, 0x0605_e9d8, screen::stop_fades(cpu));
    nested!(cpu, 0x0605_e6dc, screen::set_fade_offset(cpu, 0xff, 0xff, 0xff));
    nested!(cpu, 0x0605_e93c, screen::fade_to(cpu, 0, 0, 0, 60));
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 79));
}

// ---- the figure's mode steps (the table at 0x060910cc, by mode) -------------------------------------------------------

/// The figure's lift (+0x18b0, a count that rises or falls to 0) and the speed it moves at (+0x18b4).
const FIG_LIFT: u32 = 0x18b0;
const FIG_LIFT_SPEED: u32 = 0x18b4;

/// A word the SH-2 sign-extends on loading.
fn neg_word(v: u16) -> u32 {
    v as i16 as i32 as u32
}

/// The lift eased (the part the steps 2 and 4 share): `aim` its distance from the place the pull
/// puts it at; below 0 the lift is cleared; else the speed approaches a 48th of the way back by a
/// 0x300th of the distance and the lift moves by it.
fn lift_ease(cpu: &mut Cpu, o: u32, aim: u32) {
    if cpu.read32(o + FIG_LIFT) as i32 >= 0 {
        let size = nested!(cpu, 0x0600_6a78, math::quotient(cpu, aim, 0x300)) as i32;
        let size = size.wrapping_abs() as u32;
        let goal = nested!(cpu, 0x0600_6a78, math::quotient(cpu, aim.wrapping_neg(), 48));
        let v = cpu.read32(o + FIG_LIFT_SPEED);
        let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, goal, size));
        cpu.write32(o + FIG_LIFT_SPEED, v);
        let l = cpu.read32(o + FIG_LIFT).wrapping_add(v);
        cpu.write32(o + FIG_LIFT, l);
    } else {
        cpu.write32(o + FIG_LIFT, 0);
    }
}

/// Mode 2's step: the scale +0x18b8 eased to 0x10000 by 0x80, the pose step, the head aim
/// +0x53c 0xb800 less the pull's word at +0x189a, the arms' aims cleared, the place aimed at
/// (+0x554) 0x2000 on and its z (+0x558) the pull +0x1890 by 1024 less the figure's z; sunk,
/// wrapped, settled; the pull's third +0x1898 eased to 0x1c00 by 10 while the lift speed is
/// below 0; while the pull is 0, once the lift is past 0x70000 the pull is 1 and +0x1894 0x80000;
/// the lift's aim the lift less 0x80000 (pull 0) or less +0x1894 (which then falls by 0x400); the
/// lift eased (`lift_ease`); once it is 0 its flag 0x20 cleared, the scale 0x10000 and mode 3.
pub fn figure_step_2(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_0a80, falling_piece_set(cpu));
    let v = cpu.read32(o + FIG_SCALE);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0x1_0000, 0x80));
    cpu.write32(o + FIG_SCALE, v);
    nested!(cpu, 0x0608_0290, pose_step(cpu, o + FIG_ANIM, o));
    let low = neg_word(cpu.read16(o + FIG_PULL + 8 + 2));
    cpu.write16(o + FIG_HEAD_AIM, 0xffff_b800u32.wrapping_sub(low) as u16);
    cpu.write16(o + FIG_HEAD_AIM + 2, 0);
    cpu.write16(o + FIG_HEAD_AIM + 4, 0);
    let y = cpu.read32(o + FIG_Y_AT).wrapping_add(0x2000);
    cpu.write32(o + FIG_Y_AT, y);
    let z = (cpu.read32(o + FIG_PULL) << 10).wrapping_neg().wrapping_sub(cpu.read32(o + POSITION + 8));
    cpu.write32(o + FIG_LIMB_AIM + 8, z);
    nested!(cpu, 0x0608_447c, figure_sink(cpu, o));
    nested!(cpu, 0x0608_442c, wrap_place(cpu, o));
    nested!(cpu, 0x0608_4258, figure_settle(cpu, o));
    if (cpu.read32(o + FIG_LIFT_SPEED) as i32) < 0 {
        let v = cpu.read32(o + FIG_PULL + 8);
        let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0x1c00, 10));
        cpu.write32(o + FIG_PULL + 8, v);
    }
    if cpu.read32(o + FIG_PULL) == 0 && cpu.read32(o + FIG_LIFT) as i32 > 0x7_0000 {
        cpu.write32(o + FIG_PULL, 1);
        cpu.write32(o + FIG_PULL + 4, 0x8_0000);
    }
    let aim;
    if cpu.read32(o + FIG_PULL) == 0 {
        aim = cpu.read32(o + FIG_LIFT).wrapping_add(0xfff8_0000);
    } else {
        let p = cpu.read32(o + FIG_PULL + 4);
        aim = cpu.read32(o + FIG_LIFT).wrapping_sub(p);
        cpu.write32(o + FIG_PULL + 4, p.wrapping_add(-0x400i32 as u32));
    }
    lift_ease(cpu, o, aim);
    if cpu.read32(o + FIG_LIFT) == 0 {
        let f = cpu.read16(o + FIG_FLAGS) & 0xffdf;
        cpu.write16(o + FIG_FLAGS, f);
        cpu.write32(o + FIG_SCALE, 0x1_0000);
        nested!(cpu, 0x0608_3e20, figure_mode_3(cpu, o, 0));
    }
}

/// Mode 3's step: the pose step, the head aim +0x53c 0xa800 and the arm's +0x53e eight times the
/// word +0x534 (as it is), the aim at the player (`figure_aim`), the places aimed at (+0x550 0,
/// +0x554 -0x400000, +0x558 -0x100000), the count +0x298 eased to 0 by 0x4000, the figure sunk,
/// wrapped and settled, +0x1894 on one. Past 120 of it the pull (+0x1890) says the next mode:
/// 0 and 1 mode 4, 2 and 5 mode 10, 6 and 7 mode 17, 3 and 4 none.
pub fn figure_step_3(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_0290, pose_step(cpu, o + FIG_ANIM, o));
    cpu.write16(o + FIG_HEAD_AIM, 0xa800);
    let w = neg_word(cpu.read16(o + 0x534));
    cpu.write16(o + FIG_HEAD_AIM + 2, (w << 3) as u16);
    nested!(cpu, 0x0608_4638, figure_aim(cpu, o));
    cpu.write32(o + FIG_AT, 0);
    cpu.write32(o + FIG_Y_AT, 0xffc0_0000);
    cpu.write32(o + FIG_LIMB_AIM + 8, 0xfff0_0000);
    let v = cpu.read32(o + 0x298);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0, 0x4000));
    cpu.write32(o + 0x298, v);
    nested!(cpu, 0x0608_447c, figure_sink(cpu, o));
    nested!(cpu, 0x0608_442c, wrap_place(cpu, o));
    nested!(cpu, 0x0608_4258, figure_settle(cpu, o));
    let n = cpu.read32(o + FIG_PULL + 4).wrapping_add(1);
    cpu.write32(o + FIG_PULL + 4, n);
    let pull = cpu.read32(o + FIG_PULL);
    if pull > 7 || n as i32 <= 120 {
        return;
    }
    match pull {
        0 | 1 => nested!(cpu, 0x0608_3eb4, figure_mode_4(cpu, o)),
        2 | 5 => nested!(cpu, 0x0608_3f74, figure_mode_10(cpu, o)),
        6 | 7 => nested!(cpu, 0x0608_4120, figure_mode_17(cpu, o)),
        _ => {}
    }
}

/// The pose script's step flags that the steps wait for (the figure's anim's flags word at +0x94):
/// the cues 1 and 2 a script's step gives (cleared every frame by the pose step) and the
/// script's end.
const CUE_1: u32 = 1;
const CUE_2: u32 = 2;
const FIG_SCRIPT_FLAGS: u32 = FIG_ANIM + POSE_FLAGS;

/// Whether the figure's pose script gives `bits` this frame.
fn fig_cue(cpu: &mut Cpu, o: u32, bits: u32) -> bool {
    cpu.read16(o + FIG_SCRIPT_FLAGS) as u32 & bits != 0
}

/// The head aim +0x53c 0xb900 and the arm's +0x53e the word +0x534 by 32 (as the steps 4 and 5
/// set them), the aim at the player.
fn head_up_aim(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + SPRITE));
    cpu.write16(o + FIG_HEAD_AIM, 0xb900);
    let w = neg_word(cpu.read16(o + 0x534));
    cpu.write16(o + FIG_HEAD_AIM + 2, (w << 5) as u16);
    nested!(cpu, 0x0608_4638, figure_aim(cpu, o));
}

/// Mode 4's step (the figure's first fight mode): the pose step, the head's aim up (`head_up_aim`),
/// +0x550 0, its place +0x554 eased to -0x200000 by 0x8000 and +0x558 -0x200000, +0x298 eased to
/// 0x70000 by 0x8000; on the script's first cue +0x29c is -0x10000, on its second the sound 72;
/// the figure sunk, wrapped and settled; when the script ends (flag 0x4000) it starts the pose
/// script 0x06090c2a again; at a gauge count of 0x1e0 the sphere is made; with a sphere held
/// (+0x188c) past 0x257 a volley; the count +0x2b8 on one, and when the life is gone or the count
/// is past 0x960 the figure is wrecked and its mode goes on.
pub fn figure_step_4(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_0290, pose_step(cpu, o + FIG_ANIM, o));
    head_up_aim(cpu, o);
    cpu.write32(o + FIG_AT, 0);
    let y = cpu.read32(o + FIG_Y_AT);
    let y = nested!(cpu, 0x0604_6168, actors::approach(cpu, y, 0xffe0_0000, 0x8000));
    cpu.write32(o + FIG_Y_AT, y);
    cpu.write32(o + FIG_LIMB_AIM + 8, 0xffe0_0000);
    let v = cpu.read32(o + 0x298);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0x7_0000, 0x8000));
    cpu.write32(o + 0x298, v);
    if fig_cue(cpu, o, CUE_1) {
        cpu.write32(o + FIG_SHRINK, 0xffff_0000);
    }
    if fig_cue(cpu, o, CUE_2) {
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 72));
    }
    nested!(cpu, 0x0608_447c, figure_sink(cpu, o));
    nested!(cpu, 0x0608_442c, wrap_place(cpu, o));
    nested!(cpu, 0x0608_4258, figure_settle(cpu, o));
    if fig_cue(cpu, o, SCRIPT_END) {
        fig_script(cpu, o, 0x0609_0c2a);
    }
    if cpu.read32(o + FIG_GAUGE) == 0x1e0 {
        nested!(cpu, 0x0608_3b90, sphere_set(cpu, o));
    }
    if cpu.read32(o + FIG_TARGET) != 0 && cpu.read32(o + FIG_GAUGE) as i32 > 0x257 {
        nested!(cpu, 0x0608_240c, figure_volley(cpu, o));
    }
    let count = cpu.read32(o + FIG_GAUGE).wrapping_add(1);
    cpu.write32(o + FIG_GAUGE, count);
    if cpu.read32(o + FIG_LIFE) as i32 <= 0 || count as i32 > 0x960 {
        nested!(cpu, 0x0608_3ddc, figure_wreck(cpu, o));
        let m = cpu.read8(o + FIG_MODE).wrapping_add(1);
        cpu.write8(o + FIG_MODE, m);
    }
}

/// Mode 5's step: the pose step, the head's aim up, +0x550 0, +0x554 the pull +0x1890 less
/// 0x200000, +0x558 -0x200000, the pull 0x6000 less, +0x298 0x70000, +0x29c -0x10000 on the
/// script's first cue; sunk, wrapped, settled; on the cue again mode 6.
pub fn figure_step_5(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_0290, pose_step(cpu, o + FIG_ANIM, o));
    head_up_aim(cpu, o);
    cpu.write32(o + FIG_AT, 0);
    let pull = cpu.read32(o + FIG_PULL);
    cpu.write32(o + FIG_Y_AT, pull.wrapping_add(0xffe0_0000));
    cpu.write32(o + FIG_LIMB_AIM + 8, 0xffe0_0000);
    cpu.write32(o + FIG_PULL, pull.wrapping_add(-0x6000i32 as u32));
    cpu.write32(o + 0x298, 0x7_0000);
    if fig_cue(cpu, o, CUE_1) {
        cpu.write32(o + FIG_SHRINK, 0xffff_0000);
    }
    nested!(cpu, 0x0608_447c, figure_sink(cpu, o));
    nested!(cpu, 0x0608_442c, wrap_place(cpu, o));
    nested!(cpu, 0x0608_4258, figure_settle(cpu, o));
    if fig_cue(cpu, o, CUE_1) {
        nested!(cpu, 0x0608_3f34, figure_mode_6(cpu, o));
    }
}

/// Mode 6's step: the pose step, the head aim 0xb900 and the arms' aims 0, the places aimed at
/// (+0x550 0, +0x554 -0x580000, +0x558 -0x200000), +0x298 eased to 0x70000 by 0x4000; the mode
/// goes on one on the script's first cue; sunk, wrapped, settled.
pub fn figure_step_6(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_0290, pose_step(cpu, o + FIG_ANIM, o));
    cpu.write16(o + FIG_HEAD_AIM, 0xb900);
    cpu.write16(o + FIG_HEAD_AIM + 2, 0);
    cpu.write16(o + FIG_HEAD_AIM + 4, 0);
    cpu.write32(o + FIG_AT, 0);
    cpu.write32(o + FIG_Y_AT, 0xffa8_0000);
    cpu.write32(o + FIG_LIMB_AIM + 8, 0xffe0_0000);
    let v = cpu.read32(o + 0x298);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0x7_0000, 0x4000));
    cpu.write32(o + 0x298, v);
    if fig_cue(cpu, o, CUE_1) {
        let m = cpu.read8(o + FIG_MODE).wrapping_add(1);
        cpu.write8(o + FIG_MODE, m);
    }
    nested!(cpu, 0x0608_447c, figure_sink(cpu, o));
    nested!(cpu, 0x0608_442c, wrap_place(cpu, o));
    nested!(cpu, 0x0608_4258, figure_settle(cpu, o));
}

/// The figure's mode on one.
fn fig_mode_up(cpu: &mut Cpu, o: u32) {
    let m = cpu.read8(o + FIG_MODE).wrapping_add(1);
    cpu.write8(o + FIG_MODE, m);
}

/// The figure sunk, wrapped and settled, as every step ends.
fn fig_finish(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_447c, figure_sink(cpu, o));
    nested!(cpu, 0x0608_442c, wrap_place(cpu, o));
    nested!(cpu, 0x0608_4258, figure_settle(cpu, o));
}

/// Mode 7's step: the pose step, the head aim 0xc000, the arms' aims 0, +0x554 -0x480000, +0x558
/// -0x200000, +0x298 3000 less (0x3000); on the script's first cue +0x298 0xa0000, +0x29c
/// -0x66000, the mode on one, its flag 0x20 cleared and the sound 73; sunk, wrapped, settled,
/// the place it moves to eased toward the player's.
pub fn figure_step_7(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_0290, pose_step(cpu, o + FIG_ANIM, o));
    cpu.write16(o + FIG_HEAD_AIM, 0xc000);
    cpu.write16(o + FIG_HEAD_AIM + 2, 0);
    cpu.write16(o + FIG_HEAD_AIM + 4, 0);
    cpu.write32(o + FIG_Y_AT, 0xffb8_0000);
    cpu.write32(o + FIG_LIMB_AIM + 8, 0xffe0_0000);
    let v = cpu.read32(o + 0x298).wrapping_add(0xffff_d000);
    cpu.write32(o + 0x298, v);
    if fig_cue(cpu, o, CUE_1) {
        cpu.write32(o + 0x298, 0xa_0000);
        cpu.write32(o + FIG_SHRINK, 0xfff9_a000);
        fig_mode_up(cpu, o);
        let f = cpu.read16(o + FIG_FLAGS) & 0xffdf;
        cpu.write16(o + FIG_FLAGS, f);
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 73));
    }
    fig_finish(cpu, o);
    nested!(cpu, 0x0608_4528, figure_follow_player(cpu, o));
}

static GLINT_SET_PROLOGUE: std::sync::OnceLock<super::Prologue> = std::sync::OnceLock::new();

/// Mode 8's step: the pose step, the arms' aims 0; while the byte +0x574 is 0 the head aim is
/// 0xa800, +0x554 -0x3c0000, +0x558 -0x200000 and +0x29c 0x2000 on; once it is not, it goes on one
/// each frame: the head aim 0xd200, +0x554 -0x3c0000 less the byte by 0x40000, +0x558 0x200000,
/// +0x298 and +0x29c 0x40000, the sound 75. On the script's second cue the byte goes on one
/// and a glint is made (watching the flags +0x2c4's 0x40, at the place +0x404 or, mirrored,
/// +0x4e8, spare +0x30, number the byte +0x32). On its first cue the mode goes on one,
/// the sound 82 and the figure is sunk. Wrapped, settled and the place it moves to eased toward
/// the player's.
pub fn figure_step_8(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_0290, pose_step(cpu, o + FIG_ANIM, o));
    cpu.write16(o + FIG_HEAD_AIM + 2, 0);
    cpu.write16(o + FIG_HEAD_AIM + 4, 0);
    let angry = cpu.read8(o + FIG_ANGRY);
    if angry == 0 {
        cpu.write16(o + FIG_HEAD_AIM, 0xa800);
        cpu.write32(o + FIG_Y_AT, 0xffc4_0000);
        cpu.write32(o + FIG_LIMB_AIM + 8, 0xffe0_0000);
        let v = cpu.read32(o + FIG_SHRINK).wrapping_add(0x2000);
        cpu.write32(o + FIG_SHRINK, v);
    } else {
        let angry = angry.wrapping_add(1);
        cpu.write8(o + FIG_ANGRY, angry);
        cpu.write16(o + FIG_HEAD_AIM, 0xd200);
        cpu.write32(o + FIG_Y_AT, ((angry as u32) << 18).wrapping_add(0xffc4_0000));
        cpu.write32(o + FIG_LIMB_AIM + 8, 0x20_0000);
        cpu.write32(o + 0x298, 0x4_0000);
        cpu.write32(o + FIG_SHRINK, 0x4_0000);
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 75));
    }
    if fig_cue(cpu, o, CUE_2) {
        let angry = cpu.read8(o + FIG_ANGRY).wrapping_add(1);
        cpu.write8(o + FIG_ANGRY, angry);
        let place = if cpu.read8(o + FIG_MIRROR) != 0 { 0x4e8 } else { 0x404 };
        let number = cpu.read8(o + NUMBER) as i8 as i16 as u16 as u32;
        cpu.nested_with_stack(&GLINT_SET_PROLOGUE, 0x0608_0d44, &[number], |cpu| {
            glint_set(cpu, o + FIG_FLAGS, 0x40, o + place, o + 48, number)
        });
    }
    if fig_cue(cpu, o, CUE_1) {
        fig_mode_up(cpu, o);
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 82));
        nested!(cpu, 0x0608_447c, figure_sink(cpu, o));
    }
    nested!(cpu, 0x0608_442c, wrap_place(cpu, o));
    nested!(cpu, 0x0608_4258, figure_settle(cpu, o));
    nested!(cpu, 0x0608_4528, figure_follow_player(cpu, o));
}

/// Mode 9's step: the pose step, the head aim 0xb000 and the arm's aim the word +0x534 by eight,
/// the aim at the player, the places aimed at (+0x550 0, +0x554 -0x580000, +0x558 -0x300000),
/// +0x298 eased to 0 by 0x2000; when the script ends mode 3 with a pull of 2; sunk, wrapped,
/// settled.
pub fn figure_step_9(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_0290, pose_step(cpu, o + FIG_ANIM, o));
    cpu.write16(o + FIG_HEAD_AIM, 0xb000);
    let w = neg_word(cpu.read16(o + 0x534));
    cpu.write16(o + FIG_HEAD_AIM + 2, (w << 3) as u16);
    nested!(cpu, 0x0608_4638, figure_aim(cpu, o));
    cpu.write32(o + FIG_AT, 0);
    cpu.write32(o + FIG_Y_AT, 0xffa8_0000);
    cpu.write32(o + FIG_LIMB_AIM + 8, 0xffd0_0000);
    let v = cpu.read32(o + 0x298);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0, 0x2000));
    cpu.write32(o + 0x298, v);
    if fig_cue(cpu, o, SCRIPT_END) {
        nested!(cpu, 0x0608_3e20, figure_mode_3(cpu, o, 2));
    }
    fig_finish(cpu, o);
}

/// Mode 10's step: the head aim 0xb400; on the script's second cue the byte +0x574 is 1; the
/// arm's aim 0x8000 once it is not 0, else the word +0x534 by eight; the aim at the player, the
/// places aimed at (+0x550 0, +0x554 -0x280000, +0x558 -0x400000); on the first cue the mode goes on
/// one, +0x574 and the count +0x2b0 are cleared, +0x579 and +0x57a 8 and the life +0x2c0 the life
/// kept (+0x50) and 0x5cc60 more, the gauge made for it; then the pose step, the figure sunk,
/// wrapped and settled.
pub fn figure_step_10(cpu: &mut Cpu, o: u32) {
    cpu.write16(o + FIG_HEAD_AIM, 0xb400);
    if fig_cue(cpu, o, CUE_2) {
        cpu.write8(o + FIG_ANGRY, 1);
    }
    if cpu.read8(o + FIG_ANGRY) != 0 {
        cpu.write16(o + FIG_HEAD_AIM + 2, 0x8000);
    } else {
        let w = neg_word(cpu.read16(o + 0x534));
        cpu.write16(o + FIG_HEAD_AIM + 2, (w << 3) as u16);
    }
    nested!(cpu, 0x0608_4638, figure_aim(cpu, o));
    cpu.write32(o + FIG_AT, 0);
    cpu.write32(o + FIG_Y_AT, 0xffd8_0000);
    cpu.write32(o + FIG_LIMB_AIM + 8, 0xffc0_0000);
    if fig_cue(cpu, o, CUE_1) {
        fig_mode_up(cpu, o);
        cpu.write8(o + FIG_ANGRY, 0);
        cpu.write16(o + FIG_COUNT, 0);
        cpu.write8(o + FIG_BYTE_57A, 8);
        cpu.write8(o + FIG_BYTE_579, 8);
        let life = cpu.read32(o + FIG_LIFE_KEPT).wrapping_add(0x5_cc60);
        cpu.write32(o + FIG_LIFE, life);
        nested!(cpu, 0x0604_64d8, hud::boss_gauge_set(cpu, life));
    }
    nested!(cpu, 0x0608_0290, pose_step(cpu, o + FIG_ANIM, o));
    fig_finish(cpu, o);
}

/// The sound's control (a byte at +0x18bc, what the sound effect 86 gave) the figure keeps
/// for stopping it, and its fixed places the glow rings are set at (+0x1794, +0x1800).
const FIG_SOUND: u32 = 0x18bc;
const FIG_RINGS: [u32; 2] = [0x1800, 0x1794];

/// Mode 11's end (and the end of several of its phases): the pose step, the figure sunk, wrapped and
/// settled; with the life gone it is wrecked, goes to mode 15 and the sound 86 is stopped.
fn step_11_end(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_0290, pose_step(cpu, o + FIG_ANIM, o));
    fig_finish(cpu, o);
    if cpu.read32(o + FIG_LIFE) as i32 <= 0 {
        nested!(cpu, 0x0608_3ddc, figure_wreck(cpu, o));
        nested!(cpu, 0x0608_4070, figure_mode_15(cpu, o));
        let control = cpu.read8(o + FIG_SOUND) as i8 as i32 as u32;
        nested!(cpu, 0x0600_52e0, sound::se_stop(cpu, 86, control));
    }
}

/// The phase of mode 11 goes on one.
fn phase_up(cpu: &mut Cpu, o: u32) {
    let p = cpu.read8(o + FIG_ANGRY).wrapping_add(1);
    cpu.write8(o + FIG_ANGRY, p);
}

/// The phase after a throw phase's count `over`: the phase on one and the sound 86's control stopped.
fn step_11_stop_sound(cpu: &mut Cpu, o: u32, over: u32) {
    if cpu.read16(o + FIG_COUNT) as u32 <= over {
        return;
    }
    phase_up(cpu, o);
    let control = cpu.read8(o + FIG_SOUND) as i8 as i32 as u32;
    nested!(cpu, 0x0600_52e0, sound::se_stop(cpu, 86, control));
}

/// The shared tail of two of mode 11's phases: the colours warmed (`warm_fade`) and the sound 86
/// started (its control kept at +0x18bc).
fn step_11_warm(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_3da4, warm_fade(cpu));
    let control = nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 86));
    cpu.write8(o + FIG_SOUND, control as u8);
}

/// Mode 11's step (the figure's long attack), a machine by the byte +0x574 (`FIG_ANGRY`, the phase), the
/// arm's aim 0, the places aimed at (+0x550 0, +0x554 -0x500000, +0x558 0x120000):
/// 0 waits past 50 frames of the count +0x2b0, then phase 1, the flag 0x20 cleared, +0x57b and +0x57c
/// cleared and the warm fade and sound 86; 1 and 33 ease the counts to 0x18000 (`figure_ease`),
/// and once the count +0x1784 is past 0x8000 go on one, the count cleared and the glow rings set;
/// 2 and 34 ease them to 0x10000 and throw (`figure_throw`) until the count is past 0xf0 or 0x320, then
/// stop the sound and go on one; 3 and 35 ease to 0 and throw until the count falls to 0x3ff, then 3
/// goes to phase 16 (the pose script 0x06090d9c) and 35 to 0xd0 (the counts cleared); 16 waits 80
/// frames; 17 fires shots from the muzzles each frame whose count has no bits 0-1 (every 16 the sound
/// 30) and, the script ending, goes to 32 with the pose script 0x06090d58; 32 waits for the
/// script's first cue, goes to 33 and the warm fade and sound 86 again; 255 goes on one mode, the
/// pose script 0x06090d7a and the bytes +0x579, +0x57a, +0x57b or +0x57c set by the mirror; any
/// other phase goes on one with the head aim 0xa000 and no facing. Then the end of the step
/// (`step_11_end`).
pub fn figure_step_11(cpu: &mut Cpu, o: u32) {
    cpu.write16(o + FIG_HEAD_AIM + 4, 0);
    cpu.write32(o + FIG_AT, 0);
    cpu.write32(o + FIG_Y_AT, 0xffb0_0000);
    cpu.write32(o + FIG_LIMB_AIM + 8, 0x12_0000);
    let phase = cpu.read8(o + FIG_ANGRY);
    match phase {
        0 => {
            cpu.write16(o + FIG_HEAD_AIM, 0);
            if cpu.read16(o + FIG_COUNT) > 50 {
                cpu.write8(o + FIG_ANGRY, 1);
                let f = cpu.read16(o + FIG_FLAGS) & 0xffdf;
                cpu.write16(o + FIG_FLAGS, f);
                cpu.write8(o + FIG_BYTE_57B, 0);
                cpu.write8(o + FIG_BYTE_57C, 0);
                step_11_warm(cpu, o);
            }
        }
        1 | 33 => {
            cpu.write16(o + FIG_FACING, 0);
            nested!(cpu, 0x0608_456c, figure_ease(cpu, o, 0x1_8000, 0x1_8000));
            if cpu.read32(o + FIG_EASE_Y) as i32 > 0x8000 {
                phase_up(cpu, o);
                cpu.write16(o + FIG_COUNT, 0);
                nested!(cpu, 0x0608_1610, halo_ring_set(cpu, o + FIG_RINGS[0], o + FIG_EASE_Y));
                nested!(cpu, 0x0608_1610, halo_ring_set(cpu, o + FIG_RINGS[1], o + FIG_EASE_X));
            }
        }
        2 | 34 => {
            nested!(cpu, 0x0608_456c, figure_ease(cpu, o, 0x1_0000, 0x1_0000));
            nested!(cpu, 0x0608_2238, figure_throw(cpu, o));
            step_11_stop_sound(cpu, o, if phase == 2 { 0xf0 } else { 0x320 });
        }
        3 | 35 => {
            nested!(cpu, 0x0608_456c, figure_ease(cpu, o, 0, 0));
            nested!(cpu, 0x0608_2238, figure_throw(cpu, o));
            if cpu.read32(o + FIG_EASE_Y) as i32 <= 0x3ff {
                cpu.write8(o + FIG_ANGRY, if phase == 3 { 16 } else { 0xd0 });
                if phase == 3 {
                    cpu.write16(o + FIG_FACING, 0);
                }
                cpu.write32(o + FIG_EASE_Y, 0);
                cpu.write32(o + FIG_EASE_X, 0);
                cpu.write16(o + FIG_COUNT, 0);
                if phase == 3 {
                    fig_script(cpu, o, 0x0609_0d9c);
                }
            }
        }
        16 => {
            if cpu.read16(o + FIG_COUNT) > 80 {
                phase_up(cpu, o);
                cpu.write16(o + FIG_COUNT, 0);
            }
        }
        17 => {
            let count = cpu.read16(o + FIG_COUNT) as u32;
            if count & 15 == 0 {
                nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 30));
            }
            let c = cpu.read16(o + FIG_COUNT) as i16 as i32 as u32;
            if c & 3 == 0 {
                let turn = (c & 0xffff) << 9;
                let (a, b) = if c & 64 != 0 {
                    (turn.wrapping_sub(0x4000), 0x4000u32.wrapping_sub(turn))
                } else {
                    (0u32.wrapping_sub(0x4000).wrapping_sub(turn), 0x4000u32.wrapping_add(turn))
                };
                nested!(cpu, 0x0608_1490, shot_from(cpu, o, a & 0xffff, o + FIG_MUZZLES[0], 8));
                nested!(cpu, 0x0608_1490, shot_from(cpu, o, b & 0xffff, o + FIG_MUZZLES[1], 8));
            }
            if fig_cue(cpu, o, SCRIPT_END) {
                cpu.write8(o + FIG_ANGRY, 32);
                fig_script(cpu, o, 0x0609_0d58);
            }
        }
        32 => {
            if fig_cue(cpu, o, CUE_1) {
                cpu.write8(o + FIG_ANGRY, 33);
                step_11_warm(cpu, o);
            }
        }
        255 => {
            fig_mode_up(cpu, o);
            cpu.write8(o + FIG_ANGRY, 0);
            cpu.write16(o + FIG_COUNT, 0);
            fig_script(cpu, o, 0x0609_0d7a);
            if cpu.read8(o + FIG_MIRROR) == 0 {
                cpu.write8(o + FIG_BYTE_579, 8);
                cpu.write8(o + FIG_BYTE_57A, 10);
                cpu.write8(o + FIG_BYTE_57C, 3);
            } else {
                cpu.write8(o + FIG_BYTE_579, 10);
                cpu.write8(o + FIG_BYTE_57A, 8);
                cpu.write8(o + FIG_BYTE_57B, 3);
            }
        }
        _ => {
            cpu.write16(o + FIG_FACING, 0);
            cpu.write16(o + FIG_HEAD_AIM, 0xa000);
            phase_up(cpu, o);
        }
    }
    step_11_end(cpu, o);
}

/// The figure's easing with one of its two counts held at `reach` and the other let go: a mirrored
/// figure holds the second, else the first (the arguments of `figure_ease`).
fn ease_by_side(cpu: &mut Cpu, o: u32, reach: u32) {
    if cpu.read8(o + FIG_MIRROR) == 0 {
        nested!(cpu, 0x0608_456c, figure_ease(cpu, o, reach, 0));
    } else {
        nested!(cpu, 0x0608_456c, figure_ease(cpu, o, 0, reach));
    }
}

/// The glow rings set at the counts +0x1780 and +0x1784 (the mirrored side's the second's).
fn side_rings(cpu: &mut Cpu, o: u32) {
    if cpu.read8(o + FIG_MIRROR) != 0 {
        nested!(cpu, 0x0608_1610, halo_ring_set(cpu, o + FIG_RINGS[0], o + FIG_EASE_Y));
    } else {
        nested!(cpu, 0x0608_1610, halo_ring_set(cpu, o + FIG_RINGS[1], o + FIG_EASE_X));
    }
}

/// Mode 12's step: the arms' aims 0; the byte +0x574 (the swing) approaches 64 by 1 and the place
/// aimed at (+0x550, +0x554) is 120 on an arc by it (the sine of its negated quarter-turn's 256ths and
/// the cosine, the mirror turning the sign of x), the head aim its negation by 256 (+0x53c), +0x558
/// 0x100000. When the script ends the counts are eased (`ease_by_side`, 0x10000), and at 64 the mode
/// goes on one, the swing and the count +0x2b8 clear, the warm fade and the sound 86 (its control kept
/// at +0x18bc). Then the step's end without a wreck (pose step, sink, wrap, settle).
pub fn figure_step_12(cpu: &mut Cpu, o: u32) {
    cpu.write16(o + FIG_HEAD_AIM + 2, 0);
    cpu.write16(o + FIG_HEAD_AIM + 4, 0);
    let from = cpu.read8(o + FIG_ANGRY) as u32;
    let a = nested!(cpu, 0x0604_6168, actors::approach(cpu, from, 64, 1)) as u8;
    cpu.write8(o + FIG_ANGRY, a);
    let turn = ((a as u32) << 8).wrapping_neg() as u16 as i16 as i32 as u32;
    let (x, y);
    if cpu.read8(o + FIG_MIRROR) != 0 {
        let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, turn));
        x = s.wrapping_neg().wrapping_mul(-120i32 as u32);
        let up = (cpu.read8(o + FIG_ANGRY) as i8 as i32 as u32) << 8;
        let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, up));
        y = c.wrapping_mul(-120i32 as u32);
    } else {
        let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, turn));
        x = s.wrapping_neg().wrapping_mul(120);
        let c = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, turn));
        y = c.wrapping_mul(-120i32 as u32);
    }
    cpu.write32(o + FIG_AT, x);
    cpu.write32(o + FIG_Y_AT, y);
    let a = cpu.read8(o + FIG_ANGRY) as u32;
    cpu.write16(o + FIG_HEAD_AIM, (a.wrapping_neg() << 8) as u16);
    cpu.write32(o + FIG_LIMB_AIM + 8, 0x10_0000);
    if fig_cue(cpu, o, SCRIPT_END) {
        ease_by_side(cpu, o, 0x1_0000);
        if cpu.read8(o + FIG_ANGRY) == 64 {
            fig_mode_up(cpu, o);
            cpu.write8(o + FIG_ANGRY, 0);
            cpu.write32(o + FIG_GAUGE, 0);
            step_11_warm(cpu, o);
        }
    }
    nested!(cpu, 0x0608_0290, pose_step(cpu, o + FIG_ANIM, o));
    fig_finish(cpu, o);
}

/// The step's end with a wreck (as 11's, but the sound 86 is left running): the pose step, the
/// figure sunk, wrapped and settled; with the life gone it is wrecked and goes to mode 15.
fn step_13_end(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_0290, pose_step(cpu, o + FIG_ANIM, o));
    fig_finish(cpu, o);
    if cpu.read32(o + FIG_LIFE) as i32 <= 0 {
        nested!(cpu, 0x0608_3ddc, figure_wreck(cpu, o));
        nested!(cpu, 0x0608_4070, figure_mode_15(cpu, o));
    }
}

/// Mode 13's step, by the byte +0x574: 0 clears the facing +0x187a, +0x57b and +0x57c, sets
/// +0x1788 0x6000 and eases the counts (`ease_by_side`, 0x10000); once either of +0x1780 and +0x1784
/// is past 0xd000 it goes on one, the count +0x2b0 cleared, the glow rings set (`side_rings`) and the
/// sphere made. 1 faces the sphere, eases (0x10000), and past 60 of the count volleys; past 0x168
/// it goes on one, +0x1882 cleared and the sound 86 stopped. 2 faces the sphere, eases to 0, and once
/// the mirror's count (+0x1784 mirrored, else +0x1780) is down to 0x3ff clears both and the facing
/// and goes to mode 14. Then the step's end (`step_13_end`).
pub fn figure_step_13(cpu: &mut Cpu, o: u32) {
    match cpu.read8(o + FIG_ANGRY) {
        0 => {
            cpu.write16(o + FIG_FACING, 0);
            cpu.write8(o + FIG_BYTE_57B, 0);
            cpu.write8(o + FIG_BYTE_57C, 0);
            cpu.write32(o + 0x1788, 0x6000);
            ease_by_side(cpu, o, 0x1_0000);
            if cpu.read32(o + FIG_EASE_X) as i32 > 0xd000 || cpu.read32(o + FIG_EASE_Y) as i32 > 0xd000 {
                phase_up(cpu, o);
                cpu.write16(o + FIG_COUNT, 0);
                side_rings(cpu, o);
                nested!(cpu, 0x0608_3b90, sphere_set(cpu, o));
            }
        }
        1 => {
            nested!(cpu, 0x0608_3c2c, figure_face_sphere(cpu, o));
            ease_by_side(cpu, o, 0x1_0000);
            if cpu.read16(o + FIG_COUNT) > 60 {
                nested!(cpu, 0x0608_240c, figure_volley(cpu, o));
            }
            if cpu.read16(o + FIG_COUNT) > 0x168 {
                phase_up(cpu, o);
                cpu.write16(o + FIG_FACE_ORB, 0);
                let control = cpu.read8(o + FIG_SOUND) as i8 as i32 as u32;
                nested!(cpu, 0x0600_52e0, sound::se_stop(cpu, 86, control));
            }
        }
        2 => {
            nested!(cpu, 0x0608_3c2c, figure_face_sphere(cpu, o));
            nested!(cpu, 0x0608_456c, figure_ease(cpu, o, 0, 0));
            let count = if cpu.read8(o + FIG_MIRROR) != 0 { cpu.read32(o + FIG_EASE_Y) } else { cpu.read32(o + FIG_EASE_X) };
            if count as i32 <= 0x3ff {
                cpu.write32(o + FIG_EASE_Y, 0);
                cpu.write32(o + FIG_EASE_X, 0);
                cpu.write16(o + FIG_FACE_ORB, 0);
                nested!(cpu, 0x0608_4014, figure_mode_14(cpu, o));
            }
        }
        _ => {}
    }
    step_13_end(cpu, o);
}

// ---- mode 14's phases (the table at 0x06091074, by the byte +0x574) --------------------------------------------------

/// The figure's counter of attacks (+0x18ac): its bits 14-18 choose the next one.
const FIG_ATTACKS: u32 = 0x18ac;
/// The glint a phase makes: watching the flags' 0x40, at the muzzle's place the mirror says
/// (`FIG_MUZZLE_PLACES`), the spare the figure's +0x30, the number its byte +0x32.
const FIG_MUZZLE_PLACES: [u32; 2] = [0x404, 0x4e8];

/// The glint a phase of mode 14 makes at the muzzle the mirror says (`which` its place, 0x4e8 or
/// 0x404), the number pushed as the fifth argument (left pushed, as the SH-2 code does, until
/// `pop`).
fn fig14_glint(cpu: &mut Cpu, o: u32, place: u32, pop: bool) {
    let number = cpu.read8(o + NUMBER) as i8 as i16 as u16 as u32;
    if pop {
        cpu.nested_with_stack(&GLINT_SET_PROLOGUE, 0x0608_0d44, &[number], |cpu| {
            glint_set(cpu, o + FIG_FLAGS, 0x40, o + place, o + 48, number)
        });
    } else {
        cpu.push(&[number]);
        nested!(cpu, 0x0608_0d44, glint_set(cpu, o + FIG_FLAGS, 0x40, o + place, o + 48, number));
    }
}

/// Mode 14, phase 0 (waiting): the head aim 0xa800, the arms' aims 0, +0x550 -0x700000, +0x554
/// -0x400000, +0x558 0; past 120 of the count +0x2b0 it is cleared and the phase is 1; sunk.
pub fn fig14_idle(cpu: &mut Cpu, o: u32) {
    cpu.write16(o + FIG_HEAD_AIM, 0xa800);
    cpu.write16(o + FIG_HEAD_AIM + 2, 0);
    cpu.write16(o + FIG_HEAD_AIM + 4, 0);
    cpu.write32(o + FIG_AT, 0xff90_0000);
    cpu.write32(o + FIG_Y_AT, 0xffc0_0000);
    cpu.write32(o + FIG_LIMB_AIM + 8, 0);
    if cpu.read16(o + FIG_COUNT) > 120 {
        cpu.write16(o + FIG_COUNT, 0);
        cpu.write8(o + FIG_ANGRY, 1);
    }
    nested!(cpu, 0x0608_447c, figure_sink(cpu, o));
}

/// The phase with its pose script started, the flag 0x20 cleared and the attacks counted on a
/// quarter (0x4000).
fn fig14_go(cpu: &mut Cpu, o: u32, phase: u8, script: u32) {
    cpu.write8(o + FIG_ANGRY, phase);
    let f = cpu.read16(o + FIG_FLAGS) & 0xffdf;
    cpu.write16(o + FIG_FLAGS, f);
    fig_script(cpu, o, script);
    let n = cpu.read32(o + FIG_ATTACKS).wrapping_add(0x4000);
    cpu.write32(o + FIG_ATTACKS, n);
    nested!(cpu, 0x0608_447c, figure_sink(cpu, o));
}

/// Mode 14, phase 1 (choosing): the head aim 0xc000, the arm's aim the attacks' low word
/// negated, +0x550 -0x400000, +0x554 and +0x558 0; the attacks' bits 14-18 (`n`) pick: with bit 16 set
/// the phase 5 (pose script 0x06090e04); else an odd `n` the phase 2 (0x06090db0), an even one
/// the phase 8 (0x06090e76; +0x576 1, +0x57b 4, +0x554 0xa0000, +0x550 -0x680000, the flag
/// 0x20 cleared). The attacks go on a quarter and the figure is sunk.
pub fn fig14_choose(cpu: &mut Cpu, o: u32) {
    cpu.write16(o + FIG_HEAD_AIM, 0xc000);
    let low = neg_word(cpu.read16(o + FIG_ATTACKS + 2));
    cpu.write16(o + FIG_HEAD_AIM + 2, low.wrapping_neg() as u16);
    cpu.write16(o + FIG_HEAD_AIM + 4, 0);
    cpu.write32(o + FIG_AT, 0xffc0_0000);
    cpu.write32(o + FIG_Y_AT, 0);
    cpu.write32(o + FIG_LIMB_AIM + 8, 0);
    let n = (cpu.read32(o + FIG_ATTACKS) & 0x7_c000) >> 14;
    if n & 4 != 0 {
        fig14_go(cpu, o, 5, 0x0609_0e04);
    } else if n & 1 != 0 {
        fig14_go(cpu, o, 2, 0x0609_0db0);
    } else {
        cpu.write32(o + FIG_LIMB_AIM + 8, 0);
        cpu.write32(o + FIG_Y_AT, 0xa_0000);
        cpu.write32(o + FIG_AT, 0xff98_0000);
        cpu.write8(o + FIG_BYTE_576, 1);
        cpu.write8(o + FIG_BYTE_57B, 4);
        fig14_go(cpu, o, 8, 0x0609_0e76);
    }
}

/// Mode 14, phase 2: +0x558 -0x180000; on the script's first cue the phase 3, +0x294 0x20000 and
/// +0x29c -0x20000 (not sunk); else sunk.
pub fn fig14_a_ready(cpu: &mut Cpu, o: u32) {
    cpu.write32(o + FIG_LIMB_AIM + 8, 0xffe8_0000);
    if fig_cue(cpu, o, CUE_1) {
        phase_up(cpu, o);
        cpu.write32(o + 0x294, 0x2_0000);
        cpu.write32(o + FIG_SHRINK, 0xfffe_0000);
    } else {
        nested!(cpu, 0x0608_447c, figure_sink(cpu, o));
    }
}

/// Mode 14, phase 3: +0x550 on by +0x294 and +0x29c by 0x2000; on the script's first cue the
/// phase 4, +0x29c 0, a glint at the mirrored side's muzzle (+0x4e8; the number pushed
/// and left pushed), the figure sunk and the sound 76.
pub fn fig14_a_fire(cpu: &mut Cpu, o: u32) {
    let x = cpu.read32(o + FIG_AT).wrapping_add(cpu.read32(o + 0x294));
    cpu.write32(o + FIG_AT, x);
    let v = cpu.read32(o + FIG_SHRINK).wrapping_add(0x2000);
    cpu.write32(o + FIG_SHRINK, v);
    if !fig_cue(cpu, o, CUE_1) {
        return;
    }
    phase_up(cpu, o);
    cpu.write32(o + FIG_SHRINK, 0);
    fig14_glint(cpu, o, FIG_MUZZLE_PLACES[1], false);
    nested!(cpu, 0x0608_447c, figure_sink(cpu, o));
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 76));
    cpu.pop(4);
}

/// Mode 14, phase 4: +0x550 on by +0x294, which eases to 0 by 0x3000; the script ending (0x4000)
/// the phase 1; sunk.
pub fn fig14_a_after(cpu: &mut Cpu, o: u32) {
    let x = cpu.read32(o + FIG_AT).wrapping_add(cpu.read32(o + 0x294));
    cpu.write32(o + FIG_AT, x);
    let v = cpu.read32(o + 0x294);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0, 0x3000));
    cpu.write32(o + 0x294, v);
    if fig_cue(cpu, o, SCRIPT_END) {
        cpu.write8(o + FIG_ANGRY, 1);
    }
    nested!(cpu, 0x0608_447c, figure_sink(cpu, o));
}

/// Mode 14, phase 5: +0x554 -0x60000; on the script's first cue the phase 6, +0x294 -0x1c000,
/// +0x29c -0x40000, a glint at the muzzle the mirror says (`FIG_MUZZLE_PLACES`) and the sound 73
/// (not sunk); else sunk with the mirror turned over for the call.
pub fn fig14_c_fire(cpu: &mut Cpu, o: u32) {
    cpu.write32(o + FIG_Y_AT, 0xfffa_0000);
    if fig_cue(cpu, o, CUE_1) {
        phase_up(cpu, o);
        cpu.write32(o + 0x294, 0xfffe_4000);
        cpu.write32(o + FIG_SHRINK, 0xfffc_0000);
        let place = if cpu.read8(o + FIG_MIRROR) != 0 { FIG_MUZZLE_PLACES[1] } else { FIG_MUZZLE_PLACES[0] };
        fig14_glint(cpu, o, place, true);
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 73));
    } else {
        let m = cpu.read8(o + FIG_MIRROR) ^ 1;
        cpu.write8(o + FIG_MIRROR, m);
        nested!(cpu, 0x0608_447c, figure_sink(cpu, o));
        let m = cpu.read8(o + FIG_MIRROR) ^ 1;
        cpu.write8(o + FIG_MIRROR, m);
    }
}

/// Mode 14, phase 6: +0x550 on by +0x294 and +0x29c by 0x2000; a shard (speed 0x24000) from the
/// muzzle the mirror says (+0x4e8, +0x404) toward the player's place (the angle of the muzzle's
/// place less the figure's +0x186c, +0x1870, a quarter on); past difficulty 1 a second one (speed
/// 0x18000) at the angle 0x7000 less it, if the attacks' bit 15 is set, else 0x400 more; on the
/// script's first cue the phase 7, +0x29c 0, the sound 82 and the figure sunk.
pub fn fig14_c_shards(cpu: &mut Cpu, o: u32) {
    let x = cpu.read32(o + FIG_AT).wrapping_add(cpu.read32(o + 0x294));
    cpu.write32(o + FIG_AT, x);
    let v = cpu.read32(o + FIG_SHRINK).wrapping_add(0x2000);
    cpu.write32(o + FIG_SHRINK, v);
    let m = if cpu.read8(o + FIG_MIRROR) != 0 { FIG_MUZZLE_PLACES[1] } else { FIG_MUZZLE_PLACES[0] };
    let at = o + m;
    let y = cpu.read32(o + m + 4).wrapping_sub(cpu.read32(o + 0x1870));
    let x = cpu.read32(o + m).wrapping_sub(cpu.read32(o + PARENT_PLACE));
    let angle = nested!(cpu, 0x0606_780c, sgl::sl_atan(cpu, x, y));
    let angle = 0x4000u32.wrapping_add(angle) & 0xffff;
    nested!(cpu, 0x0608_2664, shard_set(cpu, o, at, angle, 0x2_4000));
    if (cpu.read8(DIFFICULTY) as i8) > 1 {
        let (a, speed) = if cpu.read32(o + FIG_ATTACKS) & 0x8000 != 0 {
            (0x7000u32.wrapping_sub(angle) & 0xffff, 0x1_8000)
        } else {
            (angle.wrapping_add(0x400) & 0xffff, 0x1_8000)
        };
        nested!(cpu, 0x0608_2664, shard_set(cpu, o, at, a, speed));
    }
    if fig_cue(cpu, o, CUE_1) {
        phase_up(cpu, o);
        cpu.write32(o + FIG_SHRINK, 0);
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 82));
        nested!(cpu, 0x0608_447c, figure_sink(cpu, o));
    }
}

/// Mode 14, phase 7: +0x550 on by +0x294, which eases to 0 by 0x4000; the script ending the
/// phase 1; sunk.
pub fn fig14_c_after(cpu: &mut Cpu, o: u32) {
    let x = cpu.read32(o + FIG_AT).wrapping_add(cpu.read32(o + 0x294));
    cpu.write32(o + FIG_AT, x);
    let v = cpu.read32(o + 0x294);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0, 0x4000));
    cpu.write32(o + 0x294, v);
    if fig_cue(cpu, o, SCRIPT_END) {
        cpu.write8(o + FIG_ANGRY, 1);
    }
    nested!(cpu, 0x0608_447c, figure_sink(cpu, o));
}

/// Mode 14, phase 8 (the ribbons): on the script's first cue the sound 77 and the pull +0x1890 1;
/// while it is not 0 it counts on, and its 2nd to 12th values (2, 6, 8, 10, 12) make two
/// ribbons each (`figure_ribbon_set` kinds 2 and 18, 3 and 19, 1 and 17, 4 and 20, 0 and 16); on
/// the second cue +0x576 and +0x57b are cleared, +0x294 -0x40000 and the pull 0; then +0x294
/// eases to 0 by 0x4000 and +0x550 goes on by it; the script ending the phase 1; sunk.
pub fn fig14_b_ribbons(cpu: &mut Cpu, o: u32) {
    if fig_cue(cpu, o, CUE_1) {
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 77));
        cpu.write32(o + FIG_PULL, 1);
    }
    let pull = cpu.read32(o + FIG_PULL);
    if pull != 0 {
        cpu.write32(o + FIG_PULL, pull.wrapping_add(1));
    }
    let n = cpu.read32(o + FIG_PULL).wrapping_sub(2);
    if n <= 10 {
        let kind = match n {
            0 => Some(2),
            4 => Some(3),
            6 => Some(1),
            8 => Some(4),
            10 => Some(0),
            _ => None,
        };
        if let Some(k) = kind {
            nested!(cpu, 0x0608_2948, figure_ribbon_set(cpu, o, k));
            nested!(cpu, 0x0608_2948, figure_ribbon_set(cpu, o, 16 + k));
        }
    }
    if fig_cue(cpu, o, CUE_2) {
        cpu.write8(o + FIG_BYTE_576, 0);
        cpu.write8(o + FIG_BYTE_57B, 0);
        cpu.write32(o + 0x294, 0xfffc_0000);
        cpu.write32(o + FIG_PULL, 0);
    }
    let v = cpu.read32(o + 0x294);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0, 0x4000));
    cpu.write32(o + 0x294, v);
    let x = cpu.read32(o + FIG_AT).wrapping_add(v);
    cpu.write32(o + FIG_AT, x);
    if fig_cue(cpu, o, SCRIPT_END) {
        cpu.write8(o + FIG_ANGRY, 1);
    }
    nested!(cpu, 0x0608_447c, figure_sink(cpu, o));
}

/// Mode 14's step: the pose step, the phase's step (the table 0x06091074 by +0x574: idle, choose,
/// two phases of the attack at the +0x4e8 muzzle, three of the one the mirror says, the ribbons),
/// the place wrapped and the figure settled; with the life gone or the attacks past 0x80000 its
/// z and +0x294 are cleared, it is wrecked and goes to mode 15.
pub fn figure_step_14(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_0290, pose_step(cpu, o + FIG_ANIM, o));
    match cpu.read8(o + FIG_ANGRY) {
        0 => nested!(cpu, 0x0608_59e4, fig14_idle(cpu, o)),
        1 => nested!(cpu, 0x0608_5a4c, fig14_choose(cpu, o)),
        2 => nested!(cpu, 0x0608_5c0c, fig14_a_ready(cpu, o)),
        3 => nested!(cpu, 0x0608_5c60, fig14_a_fire(cpu, o)),
        4 => nested!(cpu, 0x0608_5cf4, fig14_a_after(cpu, o)),
        5 => nested!(cpu, 0x0608_5d58, fig14_c_fire(cpu, o)),
        6 => nested!(cpu, 0x0608_5e1c, fig14_c_shards(cpu, o)),
        7 => nested!(cpu, 0x0608_5f78, fig14_c_after(cpu, o)),
        8 => nested!(cpu, 0x0608_5fe0, fig14_b_ribbons(cpu, o)),
        _ => {}
    }
    nested!(cpu, 0x0608_442c, wrap_place(cpu, o));
    nested!(cpu, 0x0608_4258, figure_settle(cpu, o));
    if cpu.read32(o + FIG_LIFE) as i32 <= 0 || cpu.read32(o + FIG_ATTACKS) > 0x8_0000 {
        cpu.write32(o + POSITION + 8, 0);
        cpu.write32(o + 0x294, 0);
        nested!(cpu, 0x0608_3ddc, figure_wreck(cpu, o));
        nested!(cpu, 0x0608_4070, figure_mode_15(cpu, o));
    }
}

/// Mode 15's step: the pose step (its anim kept in the frame's local), the head aim 0xa000, the arm's
/// aim 0, the aim at the player, the places aimed at (+0x550 0, +0x554 -0x500000, +0x558
/// -0x200000), +0x298 eased to 0 by 0x2000 and +0x29c to 0x40000; once the figure has sunk
/// (`figure_sunk`) the mode goes on one, +0x29c and +0x298 are cleared, its pose script is
/// 0x06090ece and it is sunk; then wrapped and settled.
pub fn figure_step_15(cpu: &mut Cpu, o: u32) {
    let local = cpu.stack();
    cpu.write32(local, o + FIG_ANIM);
    nested!(cpu, 0x0608_0290, pose_step(cpu, o + FIG_ANIM, o));
    cpu.write16(o + FIG_HEAD_AIM, 0xa000);
    cpu.write16(o + FIG_HEAD_AIM + 2, 0);
    nested!(cpu, 0x0608_4638, figure_aim(cpu, o));
    cpu.write32(o + FIG_AT, 0);
    cpu.write32(o + FIG_Y_AT, 0xffb0_0000);
    cpu.write32(o + FIG_LIMB_AIM + 8, 0xffe0_0000);
    let v = cpu.read32(o + 0x298);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0, 0x2000));
    cpu.write32(o + 0x298, v);
    let v = cpu.read32(o + FIG_SHRINK);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0x4_0000, 0x2000));
    cpu.write32(o + FIG_SHRINK, v);
    if nested!(cpu, 0x0608_44e8, figure_sunk(cpu, o)) as u8 != 0 {
        fig_mode_up(cpu, o);
        cpu.write32(o + FIG_SHRINK, 0);
        cpu.write32(o + 0x298, 0);
        fig_script(cpu, o, 0x0609_0ece);
        nested!(cpu, 0x0608_447c, figure_sink(cpu, o));
    }
    nested!(cpu, 0x0608_442c, wrap_place(cpu, o));
    nested!(cpu, 0x0608_4258, figure_settle(cpu, o));
}

/// Mode 16's step: the pose step, the head aim 0xe000, the arm's aim 0, the aim at the player, the
/// places aimed at (+0x550 0, +0x554 -0x500000, +0x558 -0x200000); when the script ends mode 3 with
/// a pull of 7; sunk, wrapped, settled.
pub fn figure_step_16(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_0290, pose_step(cpu, o + FIG_ANIM, o));
    cpu.write16(o + FIG_HEAD_AIM, 0xe000);
    cpu.write16(o + FIG_HEAD_AIM + 2, 0);
    nested!(cpu, 0x0608_4638, figure_aim(cpu, o));
    cpu.write32(o + FIG_AT, 0);
    cpu.write32(o + FIG_Y_AT, 0xffb0_0000);
    cpu.write32(o + FIG_LIMB_AIM + 8, 0xffe0_0000);
    if fig_cue(cpu, o, SCRIPT_END) {
        nested!(cpu, 0x0608_3e20, figure_mode_3(cpu, o, 7));
    }
    fig_finish(cpu, o);
}

/// Mode 17's step: the pose step, the head aim 0xaf00, the arm's aim 0, the aim at the player, the
/// places aimed at (+0x550 0, +0x554 -0x500000, +0x558 -0x180000); on the script's first cue the
/// mode goes on one, +0x29c is -0x80000 and the sound 73 (not sunk); else sunk; then wrapped and
/// settled.
pub fn figure_step_17(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_0290, pose_step(cpu, o + FIG_ANIM, o));
    cpu.write16(o + FIG_HEAD_AIM, 0xaf00);
    cpu.write16(o + FIG_HEAD_AIM + 2, 0);
    nested!(cpu, 0x0608_4638, figure_aim(cpu, o));
    cpu.write32(o + FIG_AT, 0);
    cpu.write32(o + FIG_Y_AT, 0xffb0_0000);
    cpu.write32(o + FIG_LIMB_AIM + 8, 0xffe8_0000);
    if fig_cue(cpu, o, CUE_1) {
        fig_mode_up(cpu, o);
        cpu.write32(o + FIG_SHRINK, 0xfff8_0000);
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 73));
    } else {
        nested!(cpu, 0x0608_447c, figure_sink(cpu, o));
    }
    nested!(cpu, 0x0608_442c, wrap_place(cpu, o));
    nested!(cpu, 0x0608_4258, figure_settle(cpu, o));
}

/// Mode 18's step: the pose step, the head aim 0xc000, the arms' aims 0, the places aimed at
/// (+0x550 0, +0x554 -0x600000, +0x558 0x400000); when the script ends the mode goes on one, the pull
/// +0x1890 and +0x189c cleared, +0x1894 150 and +0x1898 7, the flag 0x20 cleared, the pose script
/// 0x06090f1c and the life (+0x2c0) the life kept (+0x50) and 0x33450 more, the gauge made for it;
/// then +0x29c eased to 0 by 0x1000, wrapped and settled.
pub fn figure_step_18(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_0290, pose_step(cpu, o + FIG_ANIM, o));
    cpu.write16(o + FIG_HEAD_AIM, 0xc000);
    cpu.write16(o + FIG_HEAD_AIM + 2, 0);
    cpu.write16(o + FIG_HEAD_AIM + 4, 0);
    cpu.write32(o + FIG_AT, 0);
    cpu.write32(o + FIG_Y_AT, 0xffa0_0000);
    cpu.write32(o + FIG_LIMB_AIM + 8, 0x40_0000);
    if fig_cue(cpu, o, SCRIPT_END) {
        fig_mode_up(cpu, o);
        cpu.write32(o + FIG_PULL, 0);
        cpu.write32(o + FIG_PULL + 4, 150);
        cpu.write32(o + FIG_PULL + 8, 7);
        cpu.write32(o + FIG_PULL + 12, 0);
        let f = cpu.read16(o + FIG_FLAGS) & 0xffdf;
        cpu.write16(o + FIG_FLAGS, f);
        fig_script(cpu, o, 0x0609_0f1c);
        let life = cpu.read32(o + FIG_LIFE_KEPT).wrapping_add(0x3_3450);
        cpu.write32(o + FIG_LIFE, life);
        nested!(cpu, 0x0604_64d8, hud::boss_gauge_set(cpu, life));
    }
    let v = cpu.read32(o + FIG_SHRINK);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0, 0x1000));
    cpu.write32(o + FIG_SHRINK, v);
    nested!(cpu, 0x0608_442c, wrap_place(cpu, o));
    nested!(cpu, 0x0608_4258, figure_settle(cpu, o));
}

// ---- the figure's last modes ------------------------------------------------------------------------------

/// Mode 19's tables (each by the difficulty): how many orbiters a ring makes and the angle between
/// them (words), how many streaks a ring makes and the angle between them.
const RING_ORBITERS: u32 = 0x0609_10a8;
const RING_ORBITER_STEP: u32 = 0x0609_10ae;
const RING_STREAKS: u32 = 0x0609_1098;
const RING_STREAK_STEP: u32 = 0x0609_109e;

/// How many a ring makes (a byte of the table by the difficulty).
fn ring_size(cpu: &mut Cpu, table: u32) -> i32 {
    let d = cpu.read8(DIFFICULTY) as i8 as i32 as u32;
    cpu.read8(table + d) as i32
}

/// A ring of `RING_ORBITERS[difficulty]` orbiters from the figure's place (+0x186c) starting at
/// `angle`, each `RING_ORBITER_STEP[difficulty]` on.
fn fig19_orbiters(cpu: &mut Cpu, o: u32, mut angle: u32) {
    let d = cpu.read8(DIFFICULTY) as i8 as i32 as u32;
    let step = cpu.read16(RING_ORBITER_STEP + 2 * d) as u32;
    let mut i = 0i32;
    while i < ring_size(cpu, RING_ORBITERS) {
        nested!(cpu, 0x0608_1de4, orbiter_set(cpu, o, o + PARENT_PLACE, angle));
        angle = angle.wrapping_add(step) & 0xffff;
        i = (i + 1) as i8 as i32;
    }
}

/// The same with streaks.
fn fig19_streaks(cpu: &mut Cpu, o: u32, mut angle: u32) {
    let d = cpu.read8(DIFFICULTY) as i8 as i32 as u32;
    let step = cpu.read16(RING_STREAK_STEP + 2 * d) as u32;
    let mut i = 0i32;
    while i < ring_size(cpu, RING_STREAKS) {
        nested!(cpu, 0x0608_1724, streak_set(cpu, o, o + PARENT_PLACE, angle));
        angle = angle.wrapping_add(step) & 0xffff;
        i = (i + 1) as i8 as i32;
    }
}

/// Mode 19's step (the figure's last attack: rings and bolts from the place +0x186c, -0x180000
/// up): the pose step, the head aim 0xf000 and the arms' 0, the places aimed at (+0x550 0, +0x554
/// -0x180000, +0x558 -0x100000 less the figure's z), +0x298 eased to 0x60000 by 0x100; the pose's
/// first cue the sound 77; the pull +0x1890 falls (mirrored) or rises by 16; +0x29c eased to 0 by
/// 0x1000; wrapped and settled. Once +0x29c is 0: while +0x1894 counts down, shots from the place
/// (every other frame, 0x419 of the count by the mirror's sign) and the sound 30; else by the
/// phase +0x189c: 0 rings of orbiters (every 64th frame, the mirror turned by the count's 0x40
/// meanwhile), 1 bolts (a description on the stack: 12 as the speed, the angle the pull's word
/// (+0x189a) from a quarter by the pull's bits 1 and 2, the heading it less the count by 0x470)
/// while the pull falls by 0x100, 2 rings of orbiters, streaks and bolts, others the gauge on one.
/// At the end: with the life gone or the gauge past 0xf0 the figure is wrecked, mode 20 and the
/// places set (+0x540 from +0x1892, +0x550 0, +0x554 -0x300000).
pub fn figure_step_19(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_0290, pose_step(cpu, o + FIG_ANIM, o));
    cpu.write16(o + FIG_HEAD_AIM + 2, 0);
    cpu.write16(o + FIG_HEAD_AIM, 0xf000);
    cpu.write16(o + FIG_HEAD_AIM + 4, 0);
    cpu.write32(o + FIG_AT, 0);
    cpu.write32(o + FIG_Y_AT, 0xffe8_0000);
    let z = cpu.read32(o + 0x1c);
    cpu.write32(o + FIG_LIMB_AIM + 8, 0xfff0_0000u32.wrapping_sub(z));
    let v = cpu.read32(o + 0x298);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0x6_0000, 0x100));
    cpu.write32(o + 0x298, v);
    if fig_cue(cpu, o, CUE_1) {
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 77));
    }
    let pull = cpu.read32(o + FIG_PULL);
    let pull = if cpu.read8(o + FIG_MIRROR) != 0 { pull.wrapping_sub(16) } else { pull.wrapping_add(16) };
    cpu.write32(o + FIG_PULL, pull);
    let v = cpu.read32(o + FIG_SHRINK);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0, 0x1000));
    cpu.write32(o + FIG_SHRINK, v);
    nested!(cpu, 0x0608_442c, wrap_place(cpu, o));
    nested!(cpu, 0x0608_4258, figure_settle(cpu, o));
    if cpu.read32(o + FIG_SHRINK) == 0 {
        fig19_attack(cpu, o);
    }
    if cpu.read32(o + FIG_LIFE) as i32 <= 0 || cpu.read32(o + FIG_GAUGE) as i32 > 0xf0 {
        nested!(cpu, 0x0608_3ddc, figure_wreck(cpu, o));
        nested!(cpu, 0x0608_4198, figure_mode_20(cpu, o));
        let w = cpu.read16(o + 0x1892);
        cpu.write16(o + FIG_HEAD_AIM + 4, w);
        cpu.write32(o + FIG_AT, 0);
        cpu.write32(o + FIG_Y_AT, 0xffd0_0000);
    }
}

/// The attack part of mode 19's step (see there), once +0x29c is 0.
fn fig19_attack(cpu: &mut Cpu, o: u32) {
    let count = cpu.read16(o + FIG_COUNT);
    let left = cpu.read32(o + FIG_PULL + 4);
    if left != 0 {
        cpu.write32(o + FIG_PULL + 4, left.wrapping_sub(1));
        let r = if cpu.read8(o + FIG_MIRROR) != 0 {
            (count as u32).wrapping_mul(0x419)
        } else {
            (count as u32).wrapping_mul((-0x419i32) as u32)
        };
        let angle = r & 0xffff;
        if count & 0xf == 0 {
            nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 30));
        }
        if (left.wrapping_sub(1)) & 1 != 0 {
            nested!(cpu, 0x0608_1490, shot_from(cpu, o, angle, o + PARENT_PLACE, 18));
        }
        return;
    }
    let phase = cpu.read32(o + FIG_PULL + 12);
    match phase {
        0 => fig19_phase_0(cpu, o),
        1 => fig19_phase_1(cpu, o),
        2 => fig19_phase_2(cpu, o),
        _ => {
            let g = cpu.read32(o + FIG_GAUGE).wrapping_add(1);
            cpu.write32(o + FIG_GAUGE, g);
        }
    }
}

/// The ring of orbiters a mode-19 phase makes: a random angle, the ring from it.
fn fig19_ring(cpu: &mut Cpu, o: u32) {
    let angle = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000)) & 0xffff;
    fig19_orbiters(cpu, o, angle);
}

/// The place the figure shoots from (+0x186c, +0x1870) set: x 0, y -0x180000.
fn fig19_place(cpu: &mut Cpu, o: u32) {
    cpu.write32(o + PARENT_PLACE, 0);
    cpu.write32(o + PARENT_PLACE + 4, 0xffe8_0000);
}

/// Mode 19, phase 0: the place set; every 64th frame a ring of orbiters at a random angle (the
/// mirror byte taken from the count's 0x40 for the ring and restored); the third +0x1898 down by
/// one, at 0 or less the shots' count +0x1894 100, +0x1898 0x40000 and the phase 1.
fn fig19_phase_0(cpu: &mut Cpu, o: u32) {
    fig19_place(cpu, o);
    if cpu.read16(o + FIG_COUNT) & 0x3f != 0 {
        return;
    }
    let mirror = cpu.read8(o + FIG_MIRROR);
    let angle = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000)) & 0xffff;
    let bit = ((cpu.read16(o + FIG_COUNT) & 0x40) >> 6) as u8;
    cpu.write8(o + FIG_MIRROR, bit);
    fig19_orbiters(cpu, o, angle);
    cpu.write8(o + FIG_MIRROR, mirror);
    let n = cpu.read32(o + FIG_PULL + 8).wrapping_sub(1);
    cpu.write32(o + FIG_PULL + 8, n);
    if n as i32 > 0 {
        return;
    }
    cpu.write32(o + FIG_PULL + 4, 100);
    cpu.write32(o + FIG_PULL + 8, 0x4_0000);
    fig_phase_up19(cpu, o);
}

/// The phase (+0x189c) on one.
fn fig_phase_up19(cpu: &mut Cpu, o: u32) {
    let p = cpu.read32(o + FIG_PULL + 12).wrapping_add(1);
    cpu.write32(o + FIG_PULL + 12, p);
}

/// A bolt made from a description on the stack: the player's number (the figure's), `fixed` the
/// fixed-heading flag, 12 the speed, `angle` and `heading` its words.
fn fig19_bolt(cpu: &mut Cpu, o: u32, d: u32) {
    let number = cpu.read8(o + NUMBER);
    cpu.write8(d + DESC_NUMBER, number);
    nested!(cpu, 0x0608_1a24, bolt_set(cpu, d));
}

/// Mode 19, phase 1: the third +0x1898 down by 0x100; on every second of its 256ths in turn
/// (the odd ones skip), a bolt at the angle a quarter by its bits 1-2 less the pull's word
/// (+0x189a), its heading the angle with 0x470 times the count more, fixed (flag 1); once the
/// third is 0 or less +0x1894 is 0x96 and +0x1898 18, and the phase goes on.
fn fig19_phase_1(cpu: &mut Cpu, o: u32) {
    let before = cpu.read32(o + FIG_PULL + 8);
    let third = before.wrapping_sub(0x100);
    cpu.write32(o + FIG_PULL + 8, third);
    let r4 = if (third as i32) >= 0 { third } else { before.wrapping_sub(1) };
    nested!(cpu, 0x0600_8164, clib::ashiftrt_r4::<8>(cpu, r4));
    let n = ((r4 as i32) >> 8) as u32;
    if n & 1 != 0 {
        return;
    }
    let d = cpu.stack();
    let w = cpu.read16(o + 0x189a) as i16 as i32 as u32;
    let k = n & 6;
    let angle = if k == 0 { w.wrapping_neg() } else { (k * 0x2000).wrapping_sub(w) };
    cpu.write16(d + DESC_ANGLE, angle as u16);
    cpu.write32(d + 0x14, 1);
    cpu.write16(d + DESC_SPEED, 12);
    let heading = (cpu.read16(o + FIG_COUNT) as u32).wrapping_mul(0x470).wrapping_add(angle as u16 as i16 as i32 as u32);
    cpu.write16(d + DESC_HEADING, heading as u16);
    fig19_bolt(cpu, o, d);
    let third = cpu.read32(o + FIG_PULL + 8) as i32;
    if third > 0 {
        return;
    }
    cpu.write32(o + FIG_PULL + 4, 0x96);
    cpu.write32(o + FIG_PULL + 8, 18);
    fig_phase_up19(cpu, o);
}

/// Mode 19, phase 2: the place set; every 128th frame a ring of orbiters at a random angle, the
/// third +0x1898 down by one and at 0 or less +0x1894 400, +0x1898 12 and the phase on; while the
/// third is not a multiple of 3 (a remainder) and the count a multiple of 32 a sound (81) and a ring
/// of streaks from an angle of the seed's; while the third is even and 14 or less, every 32nd
/// frame two bolts (the flag 0, speed 12, the angles 0x4000 and 0xc000 from the figure's).
fn fig19_phase_2(cpu: &mut Cpu, o: u32) {
    fig19_place(cpu, o);
    if cpu.read16(o + FIG_COUNT) & 0x7f == 0 {
        fig19_ring(cpu, o);
        let n = cpu.read32(o + FIG_PULL + 8).wrapping_sub(1);
        cpu.write32(o + FIG_PULL + 8, n);
        if n as i32 <= 0 {
            cpu.write32(o + FIG_PULL + 4, 400);
            cpu.write32(o + FIG_PULL + 8, 12);
            fig_phase_up19(cpu, o);
        }
    }
    let third = cpu.read32(o + FIG_PULL + 8);
    if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, third, 3)) != 0 && cpu.read16(o + FIG_COUNT) & 0x1f == 0 {
        let seed = cpu.read32(0x0600_ed88);
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 81));
        fig19_streaks(cpu, o, (seed << 5) & 0xffff);
    }
    let third = cpu.read32(o + FIG_PULL + 8);
    if third & 1 != 0 || third as i32 > 14 || cpu.read16(o + FIG_COUNT) & 0x1f != 0 {
        return;
    }
    let d = cpu.stack();
    cpu.write16(d + DESC_ANGLE, 0x4000);
    cpu.write32(d + 0x14, 0);
    cpu.write16(d + DESC_SPEED, 12);
    fig19_bolt(cpu, o, d);
    cpu.write16(d + DESC_ANGLE, 0xc000);
    fig19_bolt(cpu, o, d);
}

/// Mode 20's step: the pose step, the arm's aim 0, the aim at the player, the places aimed at
/// (+0x550 0, +0x554 -0x400000, +0x558 the figure's z negated); +0x294 and +0x298 eased to 0 by
/// 0x1000 and +0x29c by 0x400; once +0x29c is 0 the mode goes on one, the count +0x2b0 and the
/// phase byte +0x574 are cleared, the head aim 0xbe00 and the pose script 0x06090f58 starts;
/// wrapped and settled.
pub fn figure_step_20(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_0290, pose_step(cpu, o + FIG_ANIM, o));
    cpu.write16(o + FIG_HEAD_AIM + 2, 0);
    nested!(cpu, 0x0608_4638, figure_aim(cpu, o));
    cpu.write32(o + FIG_AT, 0);
    cpu.write32(o + FIG_Y_AT, 0xffc0_0000);
    let z = cpu.read32(o + 0x1c);
    cpu.write32(o + FIG_LIMB_AIM + 8, z.wrapping_neg());
    let v = cpu.read32(o + 0x294);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0, 0x1000));
    cpu.write32(o + 0x294, v);
    let v = cpu.read32(o + 0x298);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0, 0x1000));
    cpu.write32(o + 0x298, v);
    let v = cpu.read32(o + FIG_SHRINK);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0, 0x400));
    cpu.write32(o + FIG_SHRINK, v);
    if v == 0 {
        fig_mode_up(cpu, o);
        cpu.write16(o + FIG_COUNT, 0);
        cpu.write8(o + FIG_ANGRY, 0);
        cpu.write16(o + FIG_HEAD_AIM, 0xbe00);
        fig_script(cpu, o, 0x0609_0f58);
    }
    nested!(cpu, 0x0608_442c, wrap_place(cpu, o));
    nested!(cpu, 0x0608_4258, figure_settle(cpu, o));
}

/// Mode 21's step: while the phase byte +0x574 is set, an ember on every 4th frame (`ember_set`
/// side 1); the pose step and the aim at the player, the head aim 16 less each frame, +0x558
/// 0x13c0000, +0x294 and +0x298 eased to 0 by 0x1000, wrapped and settled; on the pose's first cue
/// the phase byte is 1, a glow made and the sound 83; with the byte set a spark seven times on
/// every 8th frame; past 0xb4 of the count the mode goes on one, +0x18a4, +0x18a8, +0x298 are
/// cleared, +0x1890 0x24000, +0x1894 100, +0x1898 30, +0x189c 0, the pose script 0x06090f7a, the
/// byte cleared, the sound 84, the life (+0x2c0) the life kept (+0x50) and 0x41eb0 more, the gauge
/// made for it, and +0x579 and +0x57a 10.
pub fn figure_step_21(cpu: &mut Cpu, o: u32) {
    if cpu.read8(o + FIG_ANGRY) != 0 && cpu.read16(o + FIG_COUNT) & 3 == 0 {
        nested!(cpu, 0x0608_1374, ember_set(cpu, 1));
    }
    nested!(cpu, 0x0608_0290, pose_step(cpu, o + FIG_ANIM, o));
    nested!(cpu, 0x0608_4638, figure_aim(cpu, o));
    let w = (cpu.read16(o + FIG_HEAD_AIM) as i16 as i32).wrapping_add(-16) as u16;
    cpu.write16(o + FIG_HEAD_AIM, w);
    cpu.write32(o + FIG_LIMB_AIM + 8, 0x013c_0000);
    let v = cpu.read32(o + 0x294);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0, 0x1000));
    cpu.write32(o + 0x294, v);
    let v = cpu.read32(o + 0x298);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0, 0x1000));
    cpu.write32(o + 0x298, v);
    nested!(cpu, 0x0608_442c, wrap_place(cpu, o));
    nested!(cpu, 0x0608_4258, figure_settle(cpu, o));
    if fig_cue(cpu, o, CUE_1) {
        cpu.write8(o + FIG_ANGRY, 1);
        nested!(cpu, 0x0608_0f38, glow_set(cpu));
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 83));
    }
    if cpu.read8(o + FIG_ANGRY) != 0 && cpu.read32(GAME_FRAMES) & 7 == 0 {
        let mut i = 0i32;
        loop {
            nested!(cpu, 0x0608_10fc, spark_set(cpu));
            i = (i + 1) as i8 as i32;
            if i > 6 {
                break;
            }
        }
    }
    if cpu.read16(o + FIG_COUNT) > 0xb4 {
        fig_mode_up(cpu, o);
        cpu.write32(o + FIG_PULL + 20, 0);
        cpu.write32(o + FIG_PULL + 24, 0);
        cpu.write32(o + 0x298, 0);
        cpu.write32(o + FIG_PULL, 0x2_4000);
        cpu.write32(o + FIG_PULL + 4, 100);
        cpu.write32(o + FIG_PULL + 8, 30);
        cpu.write32(o + FIG_PULL + 12, 0);
        fig_script(cpu, o, 0x0609_0f7a);
        cpu.write8(o + FIG_ANGRY, 0);
        nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 84));
        let life = cpu.read32(o + FIG_LIFE_KEPT).wrapping_add(0x4_1eb0);
        cpu.write32(o + FIG_LIFE, life);
        nested!(cpu, 0x0604_64d8, hud::boss_gauge_set(cpu, life));
        cpu.write8(o + FIG_BYTE_579, 10);
        cpu.write8(o + FIG_BYTE_57A, 10);
    }
}

/// The difficulty's table of the speeds the pull of mode 22 aims at (longs).
const PULL_AIMS: u32 = 0x0609_10b8;
/// Two words the game keeps for the end: a count the dying figure adds 100 to and the score it
/// gives (0x7a120 for the life's end, 0 for the time's).
const END_COUNT: u32 = 0x0607_46f0;
const END_SCORE: u32 = 0x0607_46f4;
/// The way the figure faces (+0x1882, a word).
const FIG_FACING_NOW: u32 = 0x1882;

/// Mode 22's step: the pose step, the head aim 0xac00, its arm's aim the limb's (+0x524) 256th
/// (rounded toward 0) 0x400 on (mirrored) or off; the angle +0x540 0; every 32nd frame the places
/// aimed at are random (+0x550 below 0xc0000, +0x554 0x200000 and +0x558 0x1380000 on random
/// ones); +0x298 eased to twice the pull negated by 0x100; wrapped and settled; the pose script's
/// end sets the phase byte +0x574; at the count 300 the flag 0x20 is cleared; while the gauge is
/// 0x4af or less an ember every 3 (the gauge by 60, one on) times of the count's remainder; the
/// speed +0x18a4 goes on by +0x18a8 (which falls by 0x100) if it is above 0, else both are
/// cleared; with the phase byte set +0x1894 falls and when it is below 0 the sound 71, both
/// speeds 0x1000, the facing 128 less (unless 0xc000), the pose script 0x06090f94, the pull
/// eased to the difficulty's aim by 0x500, +0x1894 the pull's 0x400000th plus the third (+0x1898),
/// the third eased to 0 by 1 and two lines of rain (eight each, from a random x, 0x200000 more
/// each time and less on the other), the phase +0x189c on and, past 5 (not on the easiest),
/// cleared with a flash. The gauge goes on one; with the life gone or the gauge past 0x1518
/// the facing is cleared, the figure is wrecked and goes to mode 23.
pub fn figure_step_22(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_0290, pose_step(cpu, o + FIG_ANIM, o));
    cpu.write16(o + FIG_HEAD_AIM, 0xac00);
    let limb = cpu.read32(o + FIG_LIMB);
    let rounded = if (limb as i32) >= 0 { limb } else { limb.wrapping_add(0xff) };
    nested!(cpu, 0x0600_8164, clib::ashiftrt_r4::<8>(cpu, rounded));
    let turned = ((rounded as i32) >> 8) as u32;
    let aim = if cpu.read8(o + FIG_MIRROR) != 0 { turned.wrapping_add(0x400) } else { turned.wrapping_add(-0x400i32 as u32) };
    cpu.write16(o + FIG_HEAD_AIM + 2, aim as u16);
    cpu.write16(o + FIG_HEAD_AIM + 4, 0);
    if cpu.read16(o + FIG_COUNT) & 0x1f == 0 {
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0xc_0000));
        cpu.write32(o + FIG_AT, r);
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x4_0000)).wrapping_add(0x20_0000);
        cpu.write32(o + FIG_Y_AT, r);
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x10_0000)).wrapping_add(0x138_0000);
        cpu.write32(o + FIG_LIMB_AIM + 8, r);
    }
    let pull = cpu.read32(o + FIG_PULL);
    let v = cpu.read32(o + 0x298);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, pull.wrapping_neg().wrapping_mul(2), 0x100));
    cpu.write32(o + 0x298, v);
    nested!(cpu, 0x0608_442c, wrap_place(cpu, o));
    nested!(cpu, 0x0608_4258, figure_settle(cpu, o));
    if fig_cue(cpu, o, SCRIPT_END) {
        cpu.write8(o + FIG_ANGRY, 1);
    }
    if cpu.read16(o + FIG_COUNT) == 0x12c {
        let f = cpu.read16(o + FIG_FLAGS) & 0xffdf;
        cpu.write16(o + FIG_FLAGS, f);
    }
    let gauge = cpu.read32(o + FIG_GAUGE);
    if gauge as i32 <= 0x4af {
        let q = nested!(cpu, 0x0600_6a78, math::quotient(cpu, gauge, 60)).wrapping_add(1);
        let every = q.wrapping_mul(3);
        let count = cpu.read16(o + FIG_COUNT) as u32;
        if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, count, every)) == 0 {
            nested!(cpu, 0x0608_1374, ember_set(cpu, 1));
        }
    }
    if cpu.read32(o + FIG_PULL + 20) as i32 > 0 {
        let a = cpu.read32(o + FIG_PULL + 24).wrapping_add(-0x100i32 as u32);
        cpu.write32(o + FIG_PULL + 24, a);
        let s = cpu.read32(o + FIG_PULL + 20).wrapping_add(a);
        cpu.write32(o + FIG_PULL + 20, s);
    } else {
        cpu.write32(o + FIG_PULL + 20, 0);
        cpu.write32(o + FIG_PULL + 24, 0);
    }
    if cpu.read8(o + FIG_ANGRY) != 0 {
        fig22_rain(cpu, o);
    }
    let gauge = cpu.read32(o + FIG_GAUGE).wrapping_add(1);
    cpu.write32(o + FIG_GAUGE, gauge);
    let life = cpu.read32(o + FIG_LIFE) as i32;
    if life > 0 && gauge as i32 <= 0x1518 {
        return;
    }
    if life <= 0 {
        cpu.write32(END_SCORE, 0x7_a120);
        let n = cpu.read32(END_COUNT).wrapping_add(100);
        cpu.write32(END_COUNT, n);
    } else {
        cpu.write32(END_SCORE, 0);
    }
    cpu.write16(o + FIG_FACING_NOW, 0);
    nested!(cpu, 0x0608_3ddc, figure_wreck(cpu, o));
    nested!(cpu, 0x0608_4200, figure_mode_23(cpu, o));
}

/// Mode 22 with the phase byte set (`figure_step_22`'s last part): the third +0x1894 down by
/// one and, once below 0, the shower begun.
fn fig22_rain(cpu: &mut Cpu, o: u32) {
    let n = cpu.read32(o + FIG_PULL + 4).wrapping_sub(1);
    cpu.write32(o + FIG_PULL + 4, n);
    if (n as i32) >= 0 {
        return;
    }
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 71));
    cpu.write32(o + FIG_PULL + 20, 0x1000);
    cpu.write32(o + FIG_PULL + 24, 0x1000);
    let facing = cpu.read16(o + FIG_FACING_NOW);
    if facing as u32 != 0xc000 {
        cpu.write16(o + FIG_FACING_NOW, (facing as i16 as i32).wrapping_add(-128) as u16);
    }
    fig_script(cpu, o, 0x0609_0f94);
    let d = cpu.read8(DIFFICULTY) as i8 as i32 as u32;
    let aim = cpu.read32(PULL_AIMS + 4 * d);
    let pull = cpu.read32(o + FIG_PULL);
    let pull = nested!(cpu, 0x0604_6168, actors::approach(cpu, pull, aim, 0x500));
    cpu.write32(o + FIG_PULL, pull);
    let q = nested!(cpu, 0x0600_6a78, math::quotient(cpu, 0x40_0000, pull));
    let third = cpu.read32(o + FIG_PULL + 8);
    cpu.write32(o + FIG_PULL + 4, q.wrapping_add(third));
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, third, 0, 1));
    cpu.write32(o + FIG_PULL + 8, v);
    let x = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x20_0000));
    let p = cpu.stack();
    cpu.write32(p, x);
    cpu.write32(p + 8, x);
    cpu.write32(p, 0xfff0_0000u32.wrapping_add(x));
    cpu.write32(p + 4, 0xff98_0000);
    cpu.write32(p + 8, x.wrapping_add(0x10_0000));
    cpu.write32(p + 12, 0xff98_0000);
    let mut i = 0i32;
    loop {
        let a = cpu.read32(p).wrapping_add(0x20_0000);
        cpu.write32(p, a);
        nested!(cpu, 0x0608_2b18, rain_set(cpu, o, p));
        let b = cpu.read32(p + 8).wrapping_sub(0x20_0000);
        cpu.write32(p + 8, b);
        nested!(cpu, 0x0608_2b18, rain_set(cpu, o, p + 8));
        i = (i + 1) as i8 as i32;
        if i > 7 {
            break;
        }
    }
    let phase = cpu.read32(o + FIG_PULL + 12).wrapping_add(1);
    cpu.write32(o + FIG_PULL + 12, phase);
    if cpu.read8(DIFFICULTY) as i8 != 0 && phase as i32 > 5 {
        cpu.write32(o + FIG_PULL + 12, 0);
        nested!(cpu, 0x0608_2d70, flash_set(cpu, o));
    }
}

/// Mode 23's step: the pose step and the aim at the player, the head aim 64 on (its arm's aim cleared), +0x558 0x2000
/// less, +0x298 eased to 0 by 0x1000, +0x29c 0x800 on; a twin spark at the figure's place (+0x186c,
/// +0x1870, +0x1874 with 0xb0 dots toward us) each frame; past 0x500000 of its z (+0x1c) the
/// mode goes on one, +0x298 and +0x29c are cleared, the flags (+0x2c4) 0x80, the pull -0x4000, the
/// count cleared; a follower is made and an explosion at its place (or the figure's); the light
/// tables, the model light, the fades stopped and the screen's colour offset and fade set; wrapped
/// and settled.
pub fn figure_step_23(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_0290, pose_step(cpu, o + FIG_ANIM, o));
    nested!(cpu, 0x0608_4638, figure_aim(cpu, o));
    cpu.write16(o + FIG_HEAD_AIM + 2, 0);
    let a = cpu.read16(o + FIG_HEAD_AIM).wrapping_add(0x40);
    cpu.write16(o + FIG_HEAD_AIM, a);
    let v = cpu.read32(o + FIG_LIMB_AIM + 8).wrapping_add(-0x2000i32 as u32);
    cpu.write32(o + FIG_LIMB_AIM + 8, v);
    let v = cpu.read32(o + 0x298);
    let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0, 0x1000));
    cpu.write32(o + 0x298, v);
    let v = cpu.read32(o + FIG_SHRINK).wrapping_add(0x800);
    cpu.write32(o + FIG_SHRINK, v);
    let p = cpu.stack();
    for k in 0..2 {
        let v = cpu.read32(o + PARENT_PLACE + 4 * k);
        cpu.write32(p + 4 * k, v);
    }
    let z = cpu.read32(o + PARENT_PLACE + 8).wrapping_add(0xff50_0000);
    cpu.write32(p + 8, z);
    nested!(cpu, 0x0605_8278, explosions::twin_set(cpu, p));
    if cpu.read32(o + 0x1c) as i32 > 0x50_0000 {
        fig_mode_up(cpu, o);
        cpu.write32(o + 0x298, 0);
        cpu.write32(o + FIG_SHRINK, 0);
        cpu.write16(o + FIG_FLAGS, 0x80);
        cpu.write32(o + FIG_PULL, 0xffff_c000);
        cpu.write16(o + FIG_COUNT, 0);
        let f = nested!(cpu, 0x0608_1210, follower_set(cpu, o));
        if f != 0 {
            for k in 0..3 {
                let v = cpu.read32(f + POSITION + 4 * k);
                cpu.write32(p + 4 * k, v);
            }
        } else {
            for k in 0..2 {
                let v = cpu.read32(o + PARENT_PLACE + 4 * k);
                cpu.write32(p + 4 * k, v);
            }
            let z = cpu.read32(o + PARENT_PLACE + 8).wrapping_add(0xff50_0000);
            cpu.write32(p + 8, z);
        }
        cpu.write32(p + 60, f);
        nested!(cpu, 0x0605_8ba4, explosions::explosion_set(cpu, p));
        nested!(cpu, 0x0606_3168, gouraud::table_ramp(cpu, 32, 28, 24));
        nested!(cpu, 0x0606_30e0, gouraud::table_floor(cpu, 8, 5, 3));
        nested!(cpu, 0x0606_2fe8, gouraud::light_turned(cpu, 0x2000, 0x2000, 0));
        nested!(cpu, 0x0605_e9d8, screen::stop_fades(cpu));
        nested!(cpu, 0x0605_e6dc, screen::set_fade_offset(cpu, 60, 60, 60));
        nested!(cpu, 0x0605_e93c, screen::fade_to(cpu, 0, 0, 0, 0x200));
    }
    nested!(cpu, 0x0608_442c, wrap_place(cpu, o));
    nested!(cpu, 0x0608_4258, figure_settle(cpu, o));
}

/// Mode 24's step: the head aim 24 on, the aim at the player, +0x558 less the pull (+0x1890),
/// the pull 0x400 on; past 250 of the count the mode goes on one and +0x29c is -0x10000; wrapped
/// and settled.
pub fn figure_step_24(cpu: &mut Cpu, o: u32) {
    let a = cpu.read16(o + FIG_HEAD_AIM).wrapping_add(24);
    cpu.write16(o + FIG_HEAD_AIM, a);
    nested!(cpu, 0x0608_4638, figure_aim(cpu, o));
    let v = cpu.read32(o + FIG_LIMB_AIM + 8).wrapping_sub(cpu.read32(o + FIG_PULL));
    cpu.write32(o + FIG_LIMB_AIM + 8, v);
    let pull = cpu.read32(o + FIG_PULL).wrapping_add(0x400);
    cpu.write32(o + FIG_PULL, pull);
    if cpu.read16(o + FIG_COUNT) > 250 {
        fig_mode_up(cpu, o);
        cpu.write32(o + FIG_SHRINK, 0xffff_0000);
    }
    nested!(cpu, 0x0608_442c, wrap_place(cpu, o));
    nested!(cpu, 0x0608_4258, figure_settle(cpu, o));
}

/// Mode 25's step: the head aim eased to 0xa000 by 0xa0, the aim at the player, +0x554 -0x400000,
/// +0x558 less the pull, the pull eased to 0 by 0x6000; once it is 0 the mode goes on one and the
/// count is cleared; wrapped and settled.
pub fn figure_step_25(cpu: &mut Cpu, o: u32) {
    let a = cpu.read16(o + FIG_HEAD_AIM) as u32;
    let a = nested!(cpu, 0x0604_6168, actors::approach(cpu, a, 0xa000, 0xa0));
    cpu.write16(o + FIG_HEAD_AIM, a as u16);
    nested!(cpu, 0x0608_4638, figure_aim(cpu, o));
    cpu.write32(o + FIG_Y_AT, 0xffc0_0000);
    let v = cpu.read32(o + FIG_LIMB_AIM + 8).wrapping_sub(cpu.read32(o + FIG_PULL));
    cpu.write32(o + FIG_LIMB_AIM + 8, v);
    let pull = cpu.read32(o + FIG_PULL);
    let pull = nested!(cpu, 0x0604_6168, actors::approach(cpu, pull, 0, 0x6000));
    cpu.write32(o + FIG_PULL, pull);
    if pull == 0 {
        fig_mode_up(cpu, o);
        cpu.write16(o + FIG_COUNT, 0);
    }
    nested!(cpu, 0x0608_442c, wrap_place(cpu, o));
    nested!(cpu, 0x0608_4258, figure_settle(cpu, o));
}

/// Mode 26's step: the head aim eased to 0xa000 by 0xa0, the aim at the player, +0x554 -0x400000,
/// +0x558 less the pull, the pull eased to -0x80000 by 0x800; past 0x140 of the count the mode
/// goes on one, +0x29c is cleared and the flags are 0x80080000; wrapped and settled.
pub fn figure_step_26(cpu: &mut Cpu, o: u32) {
    let a = cpu.read16(o + FIG_HEAD_AIM) as u32;
    let a = nested!(cpu, 0x0604_6168, actors::approach(cpu, a, 0xa000, 0xa0));
    cpu.write16(o + FIG_HEAD_AIM, a as u16);
    nested!(cpu, 0x0608_4638, figure_aim(cpu, o));
    cpu.write32(o + FIG_Y_AT, 0xffc0_0000);
    let v = cpu.read32(o + FIG_LIMB_AIM + 8).wrapping_sub(cpu.read32(o + FIG_PULL));
    cpu.write32(o + FIG_LIMB_AIM + 8, v);
    let pull = cpu.read32(o + FIG_PULL);
    let pull = nested!(cpu, 0x0604_6168, actors::approach(cpu, pull, 0xfff8_0000, 0x800));
    cpu.write32(o + FIG_PULL, pull);
    if cpu.read16(o + FIG_COUNT) > 0x140 {
        fig_mode_up(cpu, o);
        cpu.write32(o + FIG_SHRINK, 0);
        cpu.write32(o + FLAGS, 0x8008_0000);
    }
    nested!(cpu, 0x0608_442c, wrap_place(cpu, o));
    nested!(cpu, 0x0608_4258, figure_settle(cpu, o));
}

/// Mode 27's step (the last): byte +3 is 0x70.
pub fn figure_step_27(cpu: &mut Cpu, o: u32) {
    cpu.write8(o + 3, 0x70);
}

/// The figure's table of steps by its mode (+0x2ac), words of the SH-2 addresses.
const FIG_STEPS: u32 = 0x0609_10cc;

/// The figure's behaviour: the step its mode's table entry names, the place +0x14 moved on by
/// +0x294, +0x298 and +0x29c; the head's angles +0x1878 and +0x1880 each turned toward their aims
/// (turn_toward with the speeds +0x187c, +0x1884 and the goals +0x187e, +0x1886 by 0x1000 and 0x3000)
/// and moved on by them; every other frame +0x57d and +0x57e go toward +0x57b and +0x57c by one;
/// with a life the gauge shows it; the count +0x2b0 on one.
pub fn figure_behaviour(cpu: &mut Cpu, o: u32) {
    let mode = cpu.read8(o + FIG_MODE) as u32;
    match mode {
        0 => nested!(cpu, 0x0608_4688, figure_next_mode(cpu, o)),
        1 => nested!(cpu, 0x0608_46b4, figure_rise(cpu, o)),
        2 => nested!(cpu, 0x0608_4868, figure_step_2(cpu, o)),
        3 => nested!(cpu, 0x0608_4a0c, figure_step_3(cpu, o)),
        4 => nested!(cpu, 0x0608_4b24, figure_step_4(cpu, o)),
        5 => nested!(cpu, 0x0608_4cb4, figure_step_5(cpu, o)),
        6 => nested!(cpu, 0x0608_4d94, figure_step_6(cpu, o)),
        7 => nested!(cpu, 0x0608_4e48, figure_step_7(cpu, o)),
        8 => nested!(cpu, 0x0608_4f18, figure_step_8(cpu, o)),
        9 => nested!(cpu, 0x0608_5060, figure_step_9(cpu, o)),
        10 => nested!(cpu, 0x0608_5120, figure_step_10(cpu, o)),
        11 => nested!(cpu, 0x0608_5220, figure_step_11(cpu, o)),
        12 => nested!(cpu, 0x0608_5660, figure_step_12(cpu, o)),
        13 => nested!(cpu, 0x0608_57ec, figure_step_13(cpu, o)),
        14 => nested!(cpu, 0x0608_6118, figure_step_14(cpu, o)),
        15 => nested!(cpu, 0x0608_619c, figure_step_15(cpu, o)),
        16 => nested!(cpu, 0x0608_629c, figure_step_16(cpu, o)),
        17 => nested!(cpu, 0x0608_6334, figure_step_17(cpu, o)),
        18 => nested!(cpu, 0x0608_63e0, figure_step_18(cpu, o)),
        19 => nested!(cpu, 0x0608_64e4, figure_step_19(cpu, o)),
        20 => nested!(cpu, 0x0608_6a38, figure_step_20(cpu, o)),
        21 => nested!(cpu, 0x0608_6b30, figure_step_21(cpu, o)),
        22 => nested!(cpu, 0x0608_6cd8, figure_step_22(cpu, o)),
        23 => nested!(cpu, 0x0608_7050, figure_step_23(cpu, o)),
        24 => nested!(cpu, 0x0608_71e4, figure_step_24(cpu, o)),
        25 => nested!(cpu, 0x0608_7264, figure_step_25(cpu, o)),
        26 => nested!(cpu, 0x0608_7304, figure_step_26(cpu, o)),
        27 => nested!(cpu, 0x0608_73bc, figure_step_27(cpu, o)),
        _ => {
            let entry = cpu.read32(FIG_STEPS + 4 * mode);
            cpu.call(entry, &[o]);
        }
    }
    for k in 0..3 {
        let v = cpu.read32(o + POSITION + 4 * k).wrapping_add(cpu.read32(o + 0x294 + 4 * k));
        cpu.write32(o + POSITION + 4 * k, v);
    }
    for (angle, push) in [(0x1878, [0x1000, 10, 16]), (0x1880, [0x3000, 10, 32])] {
        let (speed, goal) = (o + angle + 4, o + angle + 6);
        let from = cpu.read16(o + angle) as u32;
        let to = cpu.read16(o + angle + 2) as u32;
        cpu.push(&push);
        nested!(cpu, TURN_TOWARD, aim::turn_toward(cpu, speed, goal, from, to, push[0], push[1], push[2]));
        if angle == 0x1878 {
            let v = cpu.read16(o + angle).wrapping_add(cpu.read16(speed));
            cpu.write16(o + angle, v);
        }
    }
    cpu.pop(24);
    let v = cpu.read16(o + 0x1880).wrapping_add(cpu.read16(o + 0x1884));
    cpu.write16(o + 0x1880, v);
    if cpu.read16(o + FIG_COUNT) & 1 != 0 {
        for (at, goal) in [(0x57d, 0x57b), (0x57e, 0x57c)] {
            let v = cpu.read8(o + at) as u32;
            let g = cpu.read8(o + goal) as u32;
            let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, g, 1));
            cpu.write8(o + at, v as u8);
        }
    }
    let life = cpu.read32(o + FIG_LIFE);
    if life != 0 {
        nested!(cpu, 0x0604_64b8, hud::boss_gauge_life(cpu, life));
    }
    let n = cpu.read16(o + FIG_COUNT).wrapping_add(1);
    cpu.write16(o + FIG_COUNT, n);
}

/// A phase of the frame's low bits: the game's frame byte (+3 of its counter) less `a` times `b`,
/// 4 bits of it.
pub fn frame_phase(cpu: &mut Cpu, a: u32, b: u32) -> u32 {
    let frame = cpu.read8(GAME_FRAMES + 3) as i8 as i32 as u32;
    frame.wrapping_sub((a & 0xff) * (b & 0xff)) & 15
}

// ---- the figure made ---------------------------------------------------------------------------------------

/// The figure's parts, in the order they are made: how many points (a part's points follow each
/// other from the offset), the offset, the table of the parts' kinds (and where in them each
/// kind's depth is), and the word of the figure's the part's state is kept in (none for the last).
const FIG_PARTS: [(u32, u32, u32, u32); 16] = [
    (2, 0x2e4, 0x0608_df90, 0x2c6),
    (3, 0x2fc, 0x0608_dfa0, 0x2c8),
    (2, 0x320, 0x0608_dfb8, 0x2ca),
    (2, 0x338, 0x0608_dfc8, 0x2cc),
    (3, 0x350, 0x0608_dfd8, 0x2ce),
    (3, 0x374, 0x0608_dff0, 0x2d0),
    (1, 0x398, 0x0608_e008, 0x2d2),
    (4, 0x3a4, 0x0608_e010, 0x2d4),
    (4, 0x3d4, 0x0608_e030, 0x2d6),
    (4, 0x404, 0x0608_e050, 0x2d8),
    (3, 0x434, 0x0608_dfd8, 0x2da),
    (3, 0x458, 0x0608_dff0, 0x2dc),
    (1, 0x47c, 0x0608_e008, 0x2de),
    (4, 0x488, 0x0608_e010, 0x2e0),
    (4, 0x4b8, 0x0608_e030, 0x2e2),
    (4, 0x4e8, 0x0608_e050, 0),
];
/// The numbered objects' table (the players' and the bosses'), the figure's drawing (+0x34),
/// the flags it is made with (the drawing's), its first pose's angles and its pose script.
const NUMBERED: u32 = 0x0607_40f0;
const FIG_DRAW: u32 = 0x0608_752c;
const FIG_BEHAVIOUR: u32 = 0x0608_73cc;
const FIG_MADE_FLAGS: u32 = 0xc8_0000;
const FIG_FIRST_POSE: u32 = 0x0608_e060;
const FIG_FIRST_SCRIPT: u32 = 0x0609_0fb4;
/// The pose's joints (+0x7c their count, +0x80 where the angles to reach are, +0x84 the rates'
/// place, +0x88 the table of poses) and the joints' places in the figure (+0x98 the angles of its
/// first pose, +0xf8 the rates, +0x158 the angles now, +0x1b8 and +0x218 the speeds and the goals).
const FIG_JOINTS: u32 = 0x7c;
const FIG_JOINT_COUNT: u16 = 0x30;
const FIG_POSES: u32 = 0x0609_0a00;
/// The sprites' colour set the figure's model colours are shifted from (VDP1's word at 0x060741fc).
const COLOUR_SET: u32 = 0x0607_41fc;

/// The figure made (`o` its object, `number` its number): its sprite and hit set up, its
/// behaviour and drawing, the flags 0xc80000 on, its place (0, 0, -0x740000), its mode, scales
/// and counts cleared, its sixteen parts made (each its points, its state word and its share of
/// the life), the head, arms and body's angles and aims cleared, its pose and its script
/// started, the model colours of the kinds 0x4d (frames 4 and 5) set from the colour set, the palette
/// shifted (a ramp of seven steps) and the sound 78.
pub fn figure_set(cpu: &mut Cpu, o: u32, number: u32) {
    let number = number as i8 as i32 as u32;
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + SPRITE));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + HIT));
    cpu.write32(o + BEHAVIOUR, FIG_BEHAVIOUR);
    let flags = cpu.read32(o + FLAGS) | FIG_MADE_FLAGS;
    cpu.write32(o + FLAGS, flags);
    cpu.write32(o + DRAW, FIG_DRAW);
    cpu.write8(o + NUMBER, number as u8);
    let n = cpu.read8(o + NUMBER) as i8 as i32 as u32;
    cpu.write32(NUMBERED + 4 * n, o);
    cpu.write8(o + FIG_MODE, 0);
    cpu.write16(o + 0x2b2, 0);
    cpu.write16(o + 0x2b4, 120);
    cpu.write32(o + POSITION, 0);
    cpu.write32(o + POSITION + 4, 0);
    cpu.write32(o + POSITION + 8, 0xf8c0_0000);
    for at in [0x294, 0x298, 0x29c] {
        cpu.write32(o + at, 0);
    }
    for at in [0x2a0, 0x2a2, 0x2a4, 0x2a6, 0x2a8, 0x2aa, FIG_COUNT, 0x2ae] {
        cpu.write16(o + at, 0);
    }
    cpu.write32(o + FIG_GAUGE, 0);
    cpu.write32(o + FIG_LIFE_KEPT, 0);
    cpu.write8(o + 0x2bc, 0);
    cpu.write16(o + 0x518, 0);
    cpu.write16(o + 0x51a, 0);
    cpu.write16(o + FIG_FLAGS, 0x20);
    cpu.write32(o + FIG_LIFE, 0);
    for (k, (count, points, table, state)) in FIG_PARTS.into_iter().enumerate() {
        let n = cpu.read8(o + NUMBER) as u32;
        cpu.push(&[table, n]);
        nested!(cpu, 0x0605_6ea4, parts::parts_set(cpu, count, o + points, o + FIG_FLAGS, o + FIG_LIFE, table, n));
        if state != 0 {
            cpu.write16(o + state, 0);
        }
        if k % 4 == 3 {
            cpu.pop(32);
        }
    }
    for at in [0x51e, 0x520, 0x522] {
        cpu.write16(o + at, 0);
    }
    cpu.write32(o + FIG_LIMB, 0);
    cpu.write32(o + FIG_LIMB + 4, 0);
    let z = cpu.read32(o + 0x1c);
    cpu.write32(o + FIG_LIMB + 8, 0xffc0_0000u32.wrapping_sub(z));
    for at in [0x530, 0x532, 0x534, 0x536, 0x538, 0x53a] {
        cpu.write16(o + at, 0);
    }
    for at in [0x544, 0x548, 0x54c] {
        cpu.write32(o + at, 0);
    }
    for at in [0x53c, 0x53e, 0x540] {
        cpu.write16(o + at, 0);
    }
    for k in 0..9 {
        cpu.write32(o + FIG_AT + 4 * k, 0);
    }
    for at in [0x575, 0x576, 0x577, 0x578] {
        cpu.write8(o + at, 0);
    }
    cpu.write8(o + FIG_BYTE_579, 10);
    cpu.write8(o + FIG_BYTE_57A, 10);
    for at in [0x57b, 0x57c, 0x57d, 0x57e] {
        cpu.write8(o + at, 0);
    }
    for at in [FIG_EASE_Y, FIG_EASE_X, 0x1788] {
        cpu.write32(o + at, 0);
    }
    for k in 0..8 {
        cpu.write16(o + 0x1878 + 2 * k, 0);
    }
    cpu.write32(o + FIG_LIFT, 0);
    cpu.write32(o + FIG_LIFT_SPEED, 0);
    cpu.write8(o + FIG_SOUND, 0);
    cpu.write32(o + FIG_SCALE, 0x1_0000);
    cpu.write8(o + 0x18a0, 0);
    cpu.write16(o + FIG_JOINTS, FIG_JOINT_COUNT);
    cpu.write32(o + FIG_JOINTS + 4, o + 0x98);
    cpu.write32(o + FIG_JOINTS + 8, o + 0xf8);
    cpu.write32(o + FIG_JOINTS + 12, FIG_POSES);
    cpu.write8(o + FIG_MIRROR, 0);
    nested!(cpu, 0x0605_65f0, models::pose_set(cpu, o + FIG_ANIM, FIG_FIRST_POSE));
    fig_script(cpu, o, FIG_FIRST_SCRIPT);
    nested!(cpu, 0x0605_6644, models::pose_step(cpu, o + FIG_ANIM));
    let count = cpu.read16(o + FIG_JOINTS);
    let mut from = o + 0x98;
    let mut i = 0i32;
    while i < count as i32 {
        cpu.write16(o + 0x1b8 + 2 * i as u32, 0);
        cpu.write16(o + 0x218 + 2 * i as u32, 0);
        let w = cpu.read16(from);
        from += 2;
        cpu.write16(o + 0x158 + 2 * i as u32, w);
        i = (i + 1) as i8 as i32;
    }
    let set = cpu.read16(COLOUR_SET) as u32;
    nested!(cpu, 0x0604_5b1c, models::model_colour(cpu, 0x4d, 4, set.wrapping_add(0xb0) & 0xffff));
    let set = cpu.read16(COLOUR_SET) as u32;
    nested!(cpu, 0x0604_5b1c, models::model_colour(cpu, 0x4d, 5, set.wrapping_add(0x70) & 0xffff));
    let s = cpu.stack();
    cpu.write16(s + 6, 16);
    cpu.write32(s + 8, 0x0602_fe10);
    cpu.write16(s, 0xffc0);
    cpu.write16(s + 4, 0xff4c);
    cpu.write16(s + 2, 0xff4c);
    cpu.write32(s + 12, 0x0603_2e00);
    nested!(cpu, 0x0600_5c90, palette::colours_shift(cpu, s));
    let set = cpu.read16(COLOUR_SET) as u32;
    cpu.write32(s + 8, set.wrapping_mul(2).wrapping_add(0x0602_fe30));
    for at in [0, 2, 4] {
        cpu.write16(s + at, 0x100);
    }
    cpu.write32(s + 12, 0x0603_1d40);
    nested!(cpu, 0x0600_5c90, palette::colours_shift(cpu, s));
    cpu.write32(s + 8, 0x0603_0d30);
    for i in 0..7u32 {
        cpu.write16(s + 4, 0x100u32.wrapping_sub(30 * i) as u16);
        cpu.write16(s + 2, 0x100u32.wrapping_sub(20 * i) as u16);
        cpu.write16(s, 0x100u32.wrapping_sub(5 * i) as u16);
        cpu.write32(s + 12, 0x0603_1d40 + 32 * i);
        nested!(cpu, 0x0600_5c90, palette::colours_shift(cpu, s));
    }
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 78));
}

/// The value `value` written to the word at +6 of each of the rows (12 bytes each; their count
/// at +0xc, their place at +0x10) of the kind `kind`'s model's `index`th part.
pub fn kind_part_rows_set(cpu: &mut Cpu, kind: u32, index: u32, value: u32) {
    let kind_at = cpu.read32(KINDS + K_SIZE * kind);
    let parts = cpu.read32(kind_at + 0x1c);
    let part = cpu.read32(parts.wrapping_add(index.wrapping_mul(4)));
    let rows = cpu.read32(part + 4);
    let mut i = 0u32;
    let mut at = 0u32;
    while i < cpu.read32(rows + 12) {
        i += 1;
        let row = cpu.read32(rows + 16).wrapping_add(at);
        cpu.write16(row + 6, value as u16);
        at = at.wrapping_add(12);
    }
}

/// The word `value` written to every row of every part of the kind `kind`'s model.
pub fn kind_rows_set(cpu: &mut Cpu, kind: u32, value: u32) {
    let kind_at = cpu.read32(KINDS + K_SIZE * kind);
    let value = value & 0xffff;
    let mut i = 0i32;
    while i < cpu.read32(kind_at + 0x20) as i32 {
        nested!(cpu, 0x0608_9354, kind_part_rows_set(cpu, kind, i as u32, value));
        i += 1;
    }
}

/// The boss figure made, the number it was given (0 if none was free or it could not be made):
/// a free number from 2, the object (list 2, 0x18c0 bytes), `figure_set`, and kind 0x4c's rows'
/// words set to 0x1df8.
pub fn figure_make(cpu: &mut Cpu) -> u32 {
    let n = nested!(cpu, 0x0604_5df8, aim::free_number(cpu, 2)) as i8 as i32;
    if n == -1 {
        return 0;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 0x18c0)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, 0x0604_d600));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0608_8d40, figure_set(cpu, o, n as u32));
    nested!(cpu, 0x0608_9398, kind_rows_set(cpu, 0x4c, 0x1df8));
    n as u32
}

/// The stage's message event: its four lines (by the event's byte +1) after the wait (its word
/// +6). Done at once.
const MESSAGE_FIRST_LINES: u32 = 0x0609_1174;
const MESSAGE_LINES: u32 = 0x0609_1178;
static OPENING_SET_PROLOGUE: std::sync::OnceLock<super::Prologue> = std::sync::OnceLock::new();

pub fn message_event(cpu: &mut Cpu, e: u32) -> u32 {
    let k = cpu.read8(e + 1) as u32;
    let l1 = cpu.read32(MESSAGE_FIRST_LINES + 4 * k);
    let [l2, l3, l4] = [0, 4, 8].map(|j| cpu.read32(MESSAGE_LINES + 12 * k + j));
    let wait = cpu.read16(e + 6) as i16 as i32 as u32;
    cpu.nested_with_stack(&OPENING_SET_PROLOGUE, 0x0605_99c8, &[wait], |cpu| {
        text::opening_set(cpu, l1, l2, l3, l4, wait)
    });
    1
}

// ---- the stage's timeline events ---------------------------------------------------------------------------

/// A timeline event's fields (the events of the timeline's own table, objects of 0x20 bytes): its
/// step (+1), the frames to wait (+4, which fall a frame, kept by the sound event only), the
/// number of the figure (+8), what it loads (+0x10), the places kept as kinds load (where the next
/// goes +0x14, VDP1's next place +0x18, the next character +0x1c and colour +0x1e) or the sounds'
/// sizes (+0x18 and +0x1c).
const EV_STEP: u32 = 1;
const EV_FRAMES: u32 = 4;
const EV_FIGURE: u32 = 8;
const EV_LOAD: u32 = 0x10;
const EV_AT: u32 = 0x14;
const EV_VRAM: u32 = 0x18;
const EV_CHAR: u32 = 0x1c;
const EV_COLOUR: u32 = 0x1e;
const EV_SIZE: u32 = 0x18;
const EV_SIZE_2: u32 = 0x1c;
/// The play's own places for more kinds (where the next file loads, VDP1's VRAM's next place, the
/// next character and colour), copied into an event as it loads.
const PLAY_LOAD_AT: u32 = 0x0607_40e4;
const PLAY_VRAM: u32 = 0x0607_4058;
const PLAY_CHAR: u32 = 0x0607_40ea;
const PLAY_COLOUR: u32 = 0x0607_40e8;
/// A kind's file's characters (+6, a word), colours (+0x16, a word) and the place after it (+0); a
/// kind's attributes' word (+0xa) and its mode bits (8: its colours shared).
const K_ATTRS: u32 = 0xa;
const KF_CHARS: u32 = 6;
const KF_COLOURS: u32 = 0x16;
const K_MODE_BITS: u16 = 0x38;
const K_SHARED: u16 = 8;
/// The stage's kinds: the figure's model (all 18 frames lit), the model of the glow, the model of
/// the light (its frames 4-6 flipped, the 6th sorted by 2) and the sprite kind.
const FIGURE_MODEL: u32 = 0x4c;
const FIGURE_FRAMES: u32 = 18;
const SHADE_MODEL: u32 = 0x20;
const LIGHT_MODEL: u32 = 0x4d;
const FINAL_SPRITE: u32 = 0x4e;

/// Kind `k` fixed and put in place at the event's places, which move on past it (its colours too,
/// unless `check` and the kind's are shared with others').
fn ev_kind_install(cpu: &mut Cpu, e: u32, k: u32, model: bool, check: bool) {
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
    if !check || cpu.read16(kind + K_ATTRS) & K_MODE_BITS != K_SHARED {
        let file = cpu.read32(kind);
        let c = cpu.read16(e + EV_COLOUR).wrapping_add(cpu.read16(file + KF_COLOURS));
        cpu.write16(e + EV_COLOUR, c);
    }
    let file = cpu.read32(kind);
    let next = cpu.read32(file);
    cpu.write32(e + EV_AT, next);
}

/// The stage's kinds' event, a step a frame: the figure's model loaded, put in place and its 18
/// frames flipped and lit, the light tables and the model light set; the glow's model loaded, put in
/// place and lit; the light's model loaded, put in place, its frames 4-6 flipped and the 6th
/// sorted; the sprite kind loaded and put in place (its colours always moved past). Then done (1).
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
        1 | 4 | 7 => {
            let at = cpu.read32(e + EV_AT);
            let k = match cpu.read8(e + EV_STEP) {
                1 => FIGURE_MODEL,
                4 => SHADE_MODEL,
                _ => LIGHT_MODEL,
            };
            load = nested!(cpu, 0x0604_5910, kinds::kind_load(cpu, k, at));
            if load != 0 {
                let s = cpu.read8(e + EV_STEP) + 1;
                cpu.write8(e + EV_STEP, s);
            }
        }
        2 | 5 | 8 => {
            if nested!(cpu, 0x0600_6074, files::load_done(cpu, load)) != 0 {
                let step = cpu.read8(e + EV_STEP);
                let k = match step {
                    2 => FIGURE_MODEL,
                    5 => SHADE_MODEL,
                    _ => LIGHT_MODEL,
                };
                ev_kind_install(cpu, e, k, true, true);
                if step == 5 {
                    nested!(cpu, 0x0606_3380, gouraud::models_lit(cpu, SHADE_MODEL, 1));
                }
                cpu.write8(e + EV_STEP, step + 1);
            }
        }
        3 => {
            cpu.write8(e + EV_STEP, 4);
            for frame in 0..FIGURE_FRAMES {
                nested!(cpu, 0x0604_5cdc, models::model_flip_bits(cpu, FIGURE_MODEL, frame, 0x1000));
            }
            nested!(cpu, 0x0606_3380, gouraud::models_lit(cpu, FIGURE_MODEL, FIGURE_FRAMES));
            nested!(cpu, 0x0606_3168, gouraud::table_ramp(cpu, 24, 23, 16));
            nested!(cpu, 0x0606_30e0, gouraud::table_floor(cpu, 16, 14, 12));
            nested!(cpu, 0x0606_2fe8, gouraud::light_turned(cpu, 0x5800, 0x800, 0));
        }
        6 => cpu.write8(e + EV_STEP, 7),
        9 => {
            cpu.write8(e + EV_STEP, 10);
            nested!(cpu, 0x0604_5cdc, models::model_flip_bits(cpu, LIGHT_MODEL, 4, 0x1100));
            nested!(cpu, 0x0604_5cdc, models::model_flip_bits(cpu, LIGHT_MODEL, 5, 0x1000));
            nested!(cpu, 0x0604_5cdc, models::model_flip_bits(cpu, LIGHT_MODEL, 6, 0x1100));
            nested!(cpu, 0x0606_3428, gouraud::model_sort(cpu, LIGHT_MODEL, 6, 2));
        }
        10 => {
            let at = cpu.read32(e + EV_AT);
            load = nested!(cpu, 0x0604_56cc, kinds::kind_load(cpu, FINAL_SPRITE, at));
            if load != 0 {
                cpu.write8(e + EV_STEP, 11);
            }
        }
        11 => {
            if nested!(cpu, 0x0600_6074, files::load_done(cpu, load)) != 0 {
                ev_kind_install(cpu, e, FINAL_SPRITE, false, false);
                cpu.write8(e + EV_STEP, 12);
            }
        }
        12 => cpu.write8(e + EV_STEP, 13),
        13 => done = 1,
        _ => {}
    }
    cpu.write32(e + EV_LOAD, load);
    done
}

/// The sound event's sounds (the files 0x15 and 0x14), the frames the music fades over and where
/// the sound manager keeps whether it is playing (word +0x62 of 0x0600edd0's).
const SOUND_FILES_EVENT: [u32; 2] = [0x15, 0x14];
const MUSIC_FADE: u32 = 200;
const SOUND_MANAGER: u32 = 0x0600_edd0;
const MANAGER_PLAYING: u32 = 0x62;


/// The stage's sound event, a step a frame: the two sound files loaded at the room's top less
/// 0x44040 (the first, then the second after it), the music faded out, and once the sound manager
/// is idle the files copied into the sound area and the frames to wait set from their sizes (the
/// figure's number at +8 as the least); each frame the wait falls by one. Done (1) at step 10.
pub fn sound_event(cpu: &mut Cpu, e: u32) -> u32 {
    let mut load = cpu.read32(e + EV_LOAD);
    let mut done = 0;
    match cpu.read8(e + EV_STEP) {
        0 => {
            let top = cpu.read32(ROOM_TOP).wrapping_add(-0x4_4040i32 as u32);
            cpu.write32(e + EV_AT, top);
            let s = cpu.read8(e + EV_STEP) + 1;
            cpu.write8(e + EV_STEP, s);
        }
        1 => {
            let at = cpu.read32(e + EV_AT);
            load = nested!(cpu, 0x0600_46d8, sound::load_sound_data(cpu, SOUND_FILES_EVENT[0], at));
            if load != 0 {
                let size = nested!(cpu, 0x0600_5edc, files::load_size(cpu, load));
                cpu.write32(e + EV_SIZE, size);
                cpu.write8(e + EV_STEP, 2);
            }
        }
        2 | 4 => {
            if nested!(cpu, 0x0600_6074, files::load_done(cpu, load)) != 0 {
                let s = cpu.read8(e + EV_STEP) + 1;
                cpu.write8(e + EV_STEP, s);
            }
        }
        3 => {
            let at = cpu.read32(e + EV_AT).wrapping_add(cpu.read32(e + EV_SIZE));
            load = nested!(cpu, 0x0600_46d8, sound::load_sound_data(cpu, SOUND_FILES_EVENT[1], at));
            if load != 0 {
                let size = nested!(cpu, 0x0600_5edc, files::load_size(cpu, load));
                cpu.write32(e + EV_SIZE_2, size);
                cpu.write8(e + EV_STEP, 4);
            }
        }
        5 => {
            nested!(cpu, 0x0600_4f0c, sound::music_fade_out(cpu, MUSIC_FADE));
            cpu.write8(e + EV_STEP, 6);
        }
        6 => {
            let manager = cpu.read32(SOUND_MANAGER);
            if cpu.read16(manager + MANAGER_PLAYING) == 0 {
                cpu.write32(e + EV_FRAMES, 2);
                cpu.write8(e + EV_STEP, 7);
            }
        }
        7 => {
            if (cpu.read32(e + EV_FRAMES) as i32) <= 0 {
                let (at, size) = (cpu.read32(e + EV_AT), cpu.read32(e + EV_SIZE));
                nested!(cpu, 0x0600_4adc, sound::copy_to_area(cpu, 0x15, at, size));
                let at = cpu.read32(e + EV_AT).wrapping_add(cpu.read32(e + EV_SIZE));
                let size = cpu.read32(e + EV_SIZE_2);
                nested!(cpu, 0x0600_4adc, sound::copy_to_area(cpu, 0x14, at, size));
                let total = cpu.read32(e + EV_SIZE).wrapping_add(cpu.read32(e + EV_SIZE_2)) as i32;
                let total = total + if total < 0 { 0x1fff } else { 0 };
                nested!(cpu, 0x0600_815a, clib::ashiftrt_r4::<13>(cpu, total as u32));
                let figure = cpu.read32(e + EV_FIGURE);
                let wait = nested!(cpu, 0x0606_4f50, math::at_least(cpu, figure, ((total >> 13) + 4) as u32));
                cpu.write32(e + EV_FRAMES, wait);
                cpu.write8(e + EV_STEP, 10);
            }
        }
        10 => done = 1,
        _ => {}
    }
    cpu.write32(e + EV_LOAD, load);
    let n = cpu.read32(e + EV_FRAMES).wrapping_sub(1);
    cpu.write32(e + EV_FRAMES, n);
    done
}

/// The figure's event: its figure made (step 0, over again until one is free: its number kept at
/// +8 and the end's count and score cleared), the results made once its number's slot is empty again
/// (step 1: kept at +4), and once they are over (the results' mode past 0x6f) the scroll moved on and
/// the next timeline set (-1: done).
const AFTER_TIMELINE: u32 = 0x0609_1224;
const NEXT_TIMELINE: u32 = 0x0607_4734;

pub fn figure_event(cpu: &mut Cpu, e: u32) -> u32 {
    match cpu.read8(e + EV_STEP) {
        0 => {
            let n = nested!(cpu, 0x0608_93ec, figure_make(cpu)) as i8 as i32 as u32;
            if n == 0 {
                return 0;
            }
            let s = cpu.read8(e + EV_STEP) + 1;
            cpu.write8(e + EV_STEP, s);
            cpu.write32(e + EV_FIGURE, n);
            cpu.write32(END_SCORE, 0);
            cpu.write32(END_COUNT, 0);
            0
        }
        1 => {
            let n = cpu.read32(e + EV_FIGURE);
            if cpu.read32(NUMBERED + 4 * n) != 0 {
                return 0;
            }
            let r = nested!(cpu, 0x0605_284c, results::results_set(cpu, 60));
            if r == 0 {
                return 0;
            }
            let s = cpu.read8(e + EV_STEP) + 1;
            cpu.write8(e + EV_STEP, s);
            cpu.write32(e + EV_FRAMES, r);
            0
        }
        2 => {
            let results = cpu.read32(e + EV_FRAMES);
            if (cpu.read8(results + 3) as i8) <= 0x6f {
                return 0;
            }
            nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
            let t = nested!(cpu, 0x0605_7354, timeline::timeline_set(cpu, AFTER_TIMELINE));
            cpu.write32(NEXT_TIMELINE, t);
            0xffff_ffff
        }
        _ => 0,
    }
}

// ---- the stage's screen -------------------------------------------------------------------------------------

/// The screen's scroll object (the game's, at 0x060746e0: its flags' bit 1 at +0x3e says the
/// scroll goes on) and the object at 0x06074740 (its +0x58 the screen's own draw).
const SCREEN_OBJECT: u32 = 0x0607_46e0;
const SCROLL_FLAGS: u32 = 0x3e;
const SCREEN_DRAW_OBJECT: u32 = 0x0607_4740;
const SCREEN_DRAW: u32 = 0x58;

/// The scroll moved on if the screen object's flags say so (bit 0 of the byte +0x3e).
pub fn scroll_check(cpu: &mut Cpu) {
    let screen = cpu.read32(SCREEN_OBJECT);
    if cpu.read8(screen + SCROLL_FLAGS) & 1 != 0 {
        nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
    }
}

/// The places the screen keeps for the pictures: 224 words at 0x06093560 set to -1.
const SCREEN_PLACES: u32 = 0x0609_3560;

pub fn screen_places_clear(cpu: &mut Cpu) {
    let mut at = 0;
    while at <= 0x37c {
        cpu.write32(SCREEN_PLACES + at, 0xffff_ffff);
        at += 4;
    }
}

/// The beam's polygons (the model's polygon data at 0x06091bf0) and the beam drawn by the screen
/// (its object `v`: the angles +0x10 and +0x12, the depth +0x20): while the first angle (a quarter on)
/// is within 0x3000 of 0 the beam is put, tilted by the second angle negated, at its depth less
/// 0x700000, 0x1000000 away, stretched in x by the angle's sine (the farther of the depth and
/// 0xe00000 less it, by 0x180000, when the second angle is within 0x5000 of 0x1800 in size) or its
/// cosine (by 0xc00000).
const BEAM_POLYGONS: u32 = 0x0609_1bf0;

pub fn beam_draw(cpu: &mut Cpu, v: u32) {
    let turn = (cpu.read16(v + 0x10) as i16 as i32).wrapping_add(0x4000) as i16 as i32;
    if turn.wrapping_abs() > 0x3000 {
        return;
    }
    let tilt = (cpu.read16(v + 0x12) as i16 as i32).wrapping_neg() as i16 as i32;
    let depth = cpu.read32(v + 0x20) as i32;
    let size = if (tilt.wrapping_abs().wrapping_sub(0x1800)) as u32 > 0x5000 {
        let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, tilt as u32));
        cpu.call(0x0607_14bc, &[cos, 0xc0_0000])
    } else {
        let far = depth.wrapping_abs().max((0xe0_0000i32.wrapping_sub(depth)).wrapping_abs());
        let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, tilt as u32));
        cpu.call(0x0607_14bc, &[sin, (far as u32).wrapping_add(0x18_0000)])
    };
    cpu.call(SL_CUR_WINDOW, &[0]);
    cpu.call(SL_PUSH_MATRIX, &[]);
    cpu.call(SL_TRANSLATE, &[0, (depth as u32).wrapping_add(0xff90_0000), 0x100_0000]);
    cpu.call(SL_ROT_Z, &[tilt as u32]);
    cpu.call(SL_SCALE, &[size, 0x1_0000, 0x1_0000]);
    cpu.call(SL_PUT_POLYGONS, &[BEAM_POLYGONS, 1]);
    cpu.call(SL_POP_MATRIX, &[]);
    cpu.call(SL_CUR_WINDOW, &[1]);
}
const SL_CUR_WINDOW: u32 = 0x0606_713c;
const SL_PUT_POLYGONS: u32 = 0x0607_14dc;

/// SGL's parameter blocks the screen's set-up clears (each: a place of three longs at +0xc, three
/// words at +0 and three at +6 of angles, longs at +0x10 and +0x1c, and three scales at +0x24).
const BLOCK_A: u32 = 0x0607_4810;
const BLOCK_B: u32 = 0x0607_4770;
const BLOCK_C: u32 = 0x0607_47e0;
/// The place the figure's draw keeps the light's z in (0 at the set-up; the scroll goes on once it is not).
const LIGHT_Z: u32 = 0x0609_3558;
const SCREEN_PALETTE: u32 = 0x0609_1478;
const SCREEN_MESSAGE: u32 = 0x0609_1458;

/// A parameter block cleared: the angles (two groups of three words at +0 and +6) 0, the longs
/// at +0x10, +0x14, +0x18, +0x1c, +0x20 as given, the three scales 0x10000, the place's first +0xc.
fn block_clear(cpu: &mut Cpu, b: u32, place: u32, y: u32) {
    cpu.write32(b + 12, place);
    cpu.write32(b + 20, y);
    cpu.write32(b + 16, 0);
    cpu.write32(b + 32, 0);
    cpu.write32(b + 28, 0);
    cpu.write32(b + 24, 0);
    for at in [4, 2, 0, 10, 8, 6] {
        cpu.write16(b + at, 0);
    }
    for at in [44, 40, 36] {
        cpu.write32(b + at, 0x1_0000);
    }
}

/// The screen set up (the scroll object `s`): the planes on, the screen's palette set, the window,
/// the pictures' places cleared, the parameter blocks cleared (A: z 0x800000), the plane's frame
/// set, the scroll's +0x20 cleared and +0x24 0x100000, and the screen's message made.
pub fn screen_set(cpu: &mut Cpu, s: u32) {
    cpu.call(0x0606_e23c, &[0x12_0005]);
    nested!(cpu, 0x0600_5b18, palette::set_colours(cpu, SCREEN_PALETTE, 0x110, 0x20));
    cpu.call(0x0608_de54, &[0, 0, 0x160, 0xdf]);
    cpu.call(0x0606_e17c, &[7, 2]);
    cpu.call(0x0608_de74, &[0xa5e3_fa80]);
    nested!(cpu, 0x0608_a07c, screen_places_clear(cpu));
    cpu.call(0x0608_dcd4, &[3]);
    block_clear(cpu, BLOCK_A, 0x80_0000, 0);
    block_clear(cpu, BLOCK_B, 0, 0xff00_0000);
    cpu.write32(BLOCK_C + 4, 0);
    cpu.write32(BLOCK_C, 0);
    cpu.write32(BLOCK_C + 12, 0);
    cpu.write32(BLOCK_C + 8, 0);
    cpu.write32(BLOCK_C + 20, 0x1_0000);
    cpu.write32(BLOCK_C + 16, 0x1_0000);
    cpu.write32(s + 32, 0);
    nested!(cpu, 0x0605_f040, bg::plane_a_frame(cpu));
    cpu.write32(LIGHT_Z, 0);
    cpu.write32(s + 36, 0x10_0000);
    nested!(cpu, 0x0606_1620, text::message_set(cpu, SCREEN_MESSAGE));
}

/// The screen's step (its scroll object `s`): its +0x24 falls by 0x1000, the plane scrolls by
/// 0 and twice that negated, its frame is set and, once the light's z is not 0, the
/// scroll moves on.
pub fn screen_step(cpu: &mut Cpu, s: u32) {
    let v = cpu.read32(s + 36).wrapping_add(-0x1000i32 as u32);
    cpu.write32(s + 36, v);
    nested!(cpu, 0x0605_efd4, bg::plane_a_speed(cpu, 0, v.wrapping_mul(2).wrapping_neg()));
    nested!(cpu, 0x0605_f040, bg::plane_a_frame(cpu));
    if cpu.read32(LIGHT_Z) != 0 {
        nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
    }
}

/// The screen started: the fades stopped, the colour offset set to white and faded to nothing at
/// speed 128, the palette 0x060914b8 (16 colours from 0x100) set, the planes on, colour calculation
/// set and the screen object's draw (+0x58) the beam's.
pub fn screen_start(cpu: &mut Cpu) {
    nested!(cpu, 0x0605_e9d8, screen::stop_fades(cpu));
    nested!(cpu, 0x0605_e6dc, screen::set_fade_offset(cpu, 0xff, 0xff, 0xff));
    nested!(cpu, 0x0605_e93c, screen::fade_to(cpu, 0, 0, 0, 0x80));
    nested!(cpu, 0x0600_5b18, palette::set_colours(cpu, 0x0609_14b8, 0x100, 0x10));
    cpu.call(0x0606_e23c, &[0x2_0015]);
    cpu.call(0x0607_0ddc, &[0x150]);
    let object = cpu.read32(SCREEN_DRAW_OBJECT);
    cpu.write32(object + SCREEN_DRAW, 0x0608_a09c);
}

/// The angles the figure's draw keeps (three words at 0x06093540), the matrix it keeps (0x06093510)
/// and where the screen's lines go (0x06093560, then transferred to VDP2's table at 0x25e3fa80,
/// 0x380 bytes).
const VIEW_ANGLES: u32 = 0x0609_3540;
const VIEW_MATRIX: u32 = 0x0609_3510;
const VDP2_LINES: u32 = 0x25e3_fa80;
const SCREEN_ZOOMS: u32 = 0x0607_47e0;

/// The zooms of the screen's plane set from the scroll's angles (+0x14 times -0x4000000 and +0x10
/// by 0x2000000, each in 16.16) and the plane's frame set.
pub fn screen_angles(cpu: &mut Cpu, s: u32) {
    let a = cpu.read16(s + 0x14) as i16 as i32 as u32;
    let z = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, 0xfc00_0000, a));
    cpu.write32(SCREEN_ZOOMS, z);
    let a = cpu.read16(s + 0x10) as i16 as i32 as u32;
    let z = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, 0x0200_0000, a));
    cpu.write32(SCREEN_ZOOMS + 4, z);
    // The SH-2 code holds the zooms' place, the angle's place and SGL's multiply in r8-r10, which
    // the plane's frame pushes; a bolt's description (in the figure's last attack) reads the
    // stack it leaves.
    cpu.holding(&[(8, SCREEN_ZOOMS), (9, s + 0x10), (10, 0x0606_e26c)], |cpu| {
        nested!(cpu, 0x0605_f040, bg::plane_a_frame(cpu))
    });
}

/// The screen's frame (its scroll object `s`): the angles +0x10, +0x12 and +0x14 taken from the
/// figure's draw's, the edge line of the first (+0x20); the second block's +0x10 on by 0x40000
/// (wrapped to 0x2000000); the first block's rotation made from the kept matrix (the helper at
/// 0x0608dce4 told 0) and the second's (told 1, its +0x14 0xfe000000 less the light's z); the beam
/// drawn, the lines' table made from the edge line and the angle and transferred.
pub fn screen_frame(cpu: &mut Cpu, s: u32) {
    for k in 0..3 {
        let w = cpu.read16(VIEW_ANGLES + 2 * k);
        cpu.write16(s + 0x10 + 2 * k, w);
    }
    let angle = cpu.read16(s + 0x10) as i16 as i32 as u32;
    let line = nested!(cpu, 0x0605_f860, bg::edge_line(cpu, angle));
    cpu.write32(s + 0x20, line);
    let spin = cpu.read32(BLOCK_B + 16).wrapping_add(0x4_0000) & 0x1ff_ffff;
    cpu.write32(BLOCK_B + 16, spin);
    cpu.call(SL_PUSH_MATRIX, &[]);
    cpu.call(0x0606_db7c, &[VIEW_MATRIX]);
    cpu.call(0x0608_dce4, &[0]);
    nested!(cpu, 0x0605_f530, bg::rotation_matrix(cpu, BLOCK_A));
    let z = 0xfe00_0000u32.wrapping_sub(cpu.read32(LIGHT_Z));
    cpu.write32(BLOCK_B + 20, z);
    cpu.call(0x0608_dce4, &[1]);
    nested!(cpu, 0x0605_f530, bg::rotation_matrix(cpu, BLOCK_B));
    cpu.call(SL_POP_MATRIX, &[]);
    nested!(cpu, 0x0608_a09c, beam_draw(cpu, s));
    let line = cpu.read32(s + 0x20);
    let tilt = cpu.read16(s + 0x12) as i16 as i32 as u32;
    nested!(cpu, 0x0605_f8c4, bg::edge_window(cpu, SCREEN_PLACES, line, tilt));
    cpu.call(0x0603_4a50, &[SCREEN_PLACES, VDP2_LINES, 0x380]);
    nested!(cpu, 0x0608_a3c0, screen_angles(cpu, s));
}

// ---- the stage's opening lens ----------------------------------------------------------------------------

/// VDP2's VRAM the wave table is made in (the plane at 0x25e40000 read downward, 0x25e30000 read
/// upward, the results written from 0x25e20000).
const WAVE_FROM: u32 = 0x25e4_0000;
const WAVE_LOW: u32 = 0x25e3_0000;
const WAVE_TO: u32 = 0x25e2_0000;

/// One entry of the wave table: `v`'s 24-bit signed value (its top bit kept) times (1 + sine of
/// `angle` over 16).
fn wave_entry(cpu: &mut Cpu, v: u32, angle: u32) -> u32 {
    let sign = v & 0x8000_0000;
    let mut m = v & 0xff_ffff;
    if m & 0x80_0000 != 0 {
        m |= 0xff00_0000;
    }
    let s = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, angle as i16 as i32 as u32));
    let factor = ((s as i32) >> 4) as u32;
    let m = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, factor.wrapping_add(0x1_0000), m));
    (m & 0xff_ffff) | sign
}

/// The screen's wave made in VDP2's VRAM (once): 0x3e00 entries read downward from 0x25e40000 are
/// each made a wave entry (the angle 0x800 on each) and put back and copied upward from 0x25e20000;
/// 0x200 entries read upward from 0x25e30000 are kept in the frame; then 0x200 of those are put
/// below the first 0x3e00 (and copied on) by an index (0x1ff on the right) falling by a step that falls
/// by 128 from 0x10000. (The frame holds the 0x200 entries and the copy's place.)
pub fn screen_wave_make(cpu: &mut Cpu) {
    let f = cpu.stack();
    let mut angle = 0u32;
    let mut to = WAVE_TO;
    cpu.write32(f + 0x800, to);
    let mut from = WAVE_FROM;
    for _ in 0..0x3e00 {
        from = from.wrapping_sub(4);
        let v = cpu.read32(from);
        let out = wave_entry(cpu, v, angle);
        cpu.write32(from, out);
        angle = angle.wrapping_add(0x800);
        let to_at = cpu.read32(f + 0x800);
        cpu.write32(to_at, out);
        to = to_at.wrapping_add(4);
        cpu.write32(f + 0x800, to);
    }
    for k in 0..0x200u32 {
        let v = cpu.read32(WAVE_LOW + 4 * k);
        cpu.write32(f + 0x804, 0x1_0000);
        let out = wave_entry(cpu, v, angle);
        cpu.write32(f + 4 * k, out);
        angle = angle.wrapping_add(0x800);
    }
    let mut index = 0x01ff_0000u32;
    let mut step = 0x1_0000u32;
    for _ in 0..0x200 {
        let i = (index >> 16) as i16 as i32 as u32;
        let v = cpu.read32(f + 4 * i);
        index = index.wrapping_sub(step);
        from = from.wrapping_sub(4);
        cpu.write32(from, v);
        step = step.wrapping_add(-128i32 as u32);
        let to_at = cpu.read32(f + 0x800);
        cpu.write32(to_at, v);
        cpu.write32(f + 0x800, to_at.wrapping_add(4));
    }
}

/// A flare of the lens made (`mirrored` or not): a ring (rings.rs's, its description on the
/// stack) at (0, -0x380000), tilted a random 0x2c00 turn (negated unless mirrored) and 0x5800
/// of a random one less 0x2c00, of 0x8000 in size, the colour of the sprite set's, 0x1000 apart.
pub fn flare_set(cpu: &mut Cpu, mirrored: u32) {
    let d = cpu.stack();
    cpu.write32(d + 12, 0);
    cpu.write32(d + 16, 0xffc8_0000);
    cpu.write32(d + 20, 0);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0)) & 0xffff;
    let tilt = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, r, 0x2c00));
    cpu.write16(d + 6, if mirrored == 0 { tilt.wrapping_neg() } else { tilt } as u16);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0)) & 0xffff;
    let a = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, r, 0x5800));
    cpu.write16(d + 4, (a as u16).wrapping_add(0xd400));
    cpu.write16(d + 8, 0);
    cpu.write32(d + 24, 0x8000);
    cpu.write32(d + 28, 0x8000);
    cpu.write32(d + 44, 0x50_0000);
    cpu.write32(d + 40, 0x30_0000);
    let c = cpu.read16(COLOUR_SET);
    cpu.write16(d + 2, c);
    cpu.write32(d + 32, 0x1000);
    cpu.write32(d + 36, 0x1000);
    cpu.write32(d + 48, 0x1_0000);
    for k in [52, 56, 60, 64] {
        cpu.write32(d + k, 0);
    }
    cpu.write8(d, 1);
    nested!(cpu, 0x0606_2738, rings::ring_set(cpu, d));
}

/// The lens's lines (the scroll's `o`, its progress at +0x14, 16.16): the lens's radius +0x10 the
/// eased 0 to 0x1000000 by the progress (its high word), the lines (VDP2's line table, 0xe0 lines
/// from 0x06093560) each from the circle of the radius's: lines beyond it -1, within it the
/// first and last pixel (0xb0 +- the chord, kept to 0 and 0x15f; the first shifted up 17, the last
/// doubled), the table transferred to VDP2 (0x25e3fa80, 0x380 bytes).
pub fn lens_lines(cpu: &mut Cpu, o: u32) -> u32 {
    let progress = cpu.read32(o + 0x14);
    let t = nested!(cpu, 0x0606_2ebc, math::ease(cpu, 0, 0x100_0000, progress));
    let radius = (t >> 16) as i16 as i32;
    let mut first = 56 - radius;
    cpu.write32(o + 0x10, radius as u32);
    let mut row_offset = radius;
    if first < 0 {
        row_offset = radius + first;
        first = 0;
    }
    let mut last = (cpu.read32(o + 0x10) as i32) + 56;
    if last > 0xe0 {
        last = 0xe0;
    }
    if first > 0 {
        let mut at = 0;
        while at < first * 4 {
            cpu.write32(SCREEN_PLACES + at as u32, 0xffff_ffff);
            at += 4;
        }
    }
    let mut line = first;
    let mut y = row_offset;
    while line < last {
        if line > 55 {
            y += 1;
        } else {
            y -= 1;
        }
        let radius = cpu.read32(o + 0x10) as i32;
        let chord = nested!(cpu, 0x0607_147c, sgl::sl_isqrt(cpu, radius.wrapping_mul(radius).wrapping_sub(y.wrapping_mul(y)) as u32)) as i32;
        let left = (0xb0 - chord).max(0);
        let right = (0xb0 + chord).min(0x15f);
        cpu.write32(SCREEN_PLACES + 4 * line as u32, ((left << 16) * 2) as u32 | (right * 2) as u32);
        line += 1;
    }
    if last <= 0xdf {
        let mut at = last * 4;
        while at <= 0xdf * 4 {
            cpu.write32(SCREEN_PLACES + at as u32, 0xffff_ffff);
            at += 4;
        }
    }
    cpu.call(0x0603_4a50, &[SCREEN_PLACES, VDP2_LINES, 0x380]);
    // r0 is the 0 the frame's local held, loaded in the call's delay slot.
    0
}

/// The lens's behaviour (a mode in +3: 0 start, 1 running, 0x70 end, 0x73 gone): not while the
/// game's frozen (the top bit of the word at 0x0600ed80). Start: its progress +0x14 and +0x10, +0x18
/// cleared, the sound 87, the fades stopped, the colour offset set to (0xe0, 0xe0, 0xe0) and faded
/// to nothing at speed 0x200, the helper at 0x0608de24 told 3; mode 1. Running: the progress +0x200
/// a frame; each time it reaches 0x400 in its low 12 bits a flare (alternate sides by +0x18); below 0x10000
/// the lines made (`lens_lines`) and the colour rate of the screen -7 set from the progress (0 to 0x100000
/// by it, its high word); at 0x10000 the scroll goes on and the mode is 0x70. 0x70 becomes 0x73, which
/// frees the object.
pub fn lens_behaviour(cpu: &mut Cpu, o: u32) {
    if cpu.read32(0x0600_ed80) & 0x8000_0000 != 0 {
        return;
    }
    match cpu.read8(o + MODE) {
        0 => {
            cpu.write32(o + 0x10, 0);
            cpu.write32(o + 0x14, 0);
            cpu.write32(o + 0x18, 0);
            nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 87));
            nested!(cpu, 0x0605_e9d8, screen::stop_fades(cpu));
            nested!(cpu, 0x0605_e6dc, screen::set_fade_offset(cpu, 0xe0, 0xe0, 0xe0));
            nested!(cpu, 0x0605_e93c, screen::fade_to(cpu, 0, 0, 0, 0x200));
            cpu.call(0x0608_de24, &[3]);
            cpu.write8(o + MODE, 1);
            lens_run(cpu, o);
        }
        1 => lens_run(cpu, o),
        0x70 => cpu.write8(o + MODE, 0x73),
        0x73 => nested!(cpu, 0x0600_579c, objects::obj_free(cpu, o)),
        _ => {}
    }
}

/// The lens's running step (see `lens_behaviour`).
fn lens_run(cpu: &mut Cpu, o: u32) {
    let progress = cpu.read32(o + 0x14).wrapping_add(0x200);
    cpu.write32(o + 0x14, progress);
    if progress & 0xf00 == 0x400 {
        let side = cpu.read32(o + 0x18);
        nested!(cpu, 0x0608_a6ec, flare_set(cpu, side));
        let side = cpu.read32(o + 0x18) ^ 1;
        cpu.write32(o + 0x18, side);
    }
    let progress = cpu.read32(o + 0x14) as i32;
    if progress <= 0xffff {
        nested!(cpu, 0x0608_a7c8, lens_lines(cpu, o));
        let progress = cpu.read32(o + 0x14);
        let v = nested!(cpu, 0x0606_2e7c, math::lerp(cpu, 0, 0x10_0000, progress));
        cpu.call(0x0606_e45c, &[0xffff_fff9, v >> 16]);
    } else if progress == 0x1_0000 {
        nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
        cpu.write8(o + MODE, 0x70);
    }
}

// ---- the stage's later scenes ------------------------------------------------------------------------------

/// The view the later scenes keep (0x06074750): a progress in its first long, the place the view
/// is moved to (+4, +8, +12), two longs `world_step` makes (+16, +20), the ease the colours follow
/// (+24), and three angle words (+18, +22, +26: the low words of the longs and the one after).
const WORLD: u32 = 0x0607_4750;
const WORLD_ANGLE_X: u32 = WORLD + 18;
const WORLD_ANGLE_Y: u32 = WORLD + 22;
const WORLD_ANGLE_Z: u32 = WORLD + 26;
const SL_ROT_Y: u32 = 0x0606_736c;
const SL_LOAD_MATRIX: u32 = 0x0606_db7c;
const SL_INVERS_MATRIX: u32 = 0x0606_e4dc;
const SL_CALC_POINT: u32 = 0x0606_db2c;
const SL_GET_MATRIX: u32 = 0x0606_db9c;
const SL_UNIT_MATRIX: u32 = 0x0603_512e;
/// The helper at 0x0608dce4 (told 0 for the first block, 1 for the second) and the point the drop's
/// view looks along (its angles +12, +16, +20 from the kept ones).
const ROTATION_SIDE: u32 = 0x0608_dce4;
const VIEW_POINT: u32 = 0x0609_1c44;
const SKIP_ALL: u32 = 0x1ff_ffff;

/// The lens object made (list 0, 28 bytes), its function `lens_behaviour`.
pub fn lens_set(cpu: &mut Cpu) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 28)) as i8 as i32 as u32;
    nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 0, blocks, 0x0608_a8e0))
}

/// The drop's behaviour (a model dropping in): ends once the screen object's byte +81 is 8. Its
/// step +60 (a byte): 0, the progress +64 up by 0x100 a frame, its y +0x18 eased from 0x1280000 to
/// -0x380000 and its z +0x1c from 0x1000000 to 0xe0000 by it, the step 1 once it is 0x10000
/// (and the progress cleared); 1, once the byte is 6, +61 set and the step 2. Then the model turned.
pub fn drop_behaviour(cpu: &mut Cpu, o: u32) {
    let draw = cpu.read32(SCREEN_DRAW_OBJECT);
    let state = cpu.read8(draw + 81);
    if state == 8 {
        cpu.write8(o + MODE, ENDING);
        return;
    }
    let step = cpu.read8(o + 60) as i8 as i32;
    if step == 1 {
        if state == 6 {
            cpu.write8(o + 61, 1);
            cpu.write8(o + 60, 2);
        }
    } else if step == 0 {
        let progress = cpu.read32(o + 64).wrapping_add(0x100);
        cpu.write32(o + 64, progress);
        let y = nested!(cpu, 0x0606_2ebc, math::ease(cpu, 0x0128_0000, 0xffc8_0000, progress));
        cpu.write32(o + 0x18, y);
        let z = nested!(cpu, 0x0606_2ebc, math::ease(cpu, 0x0100_0000, 0x000e_0000, progress));
        cpu.write32(o + 0x1c, z);
        if cpu.read32(o + 64) == 0x1_0000 {
            let s = cpu.read8(o + 60).wrapping_add(1);
            cpu.write8(o + 60, s);
            cpu.write32(o + 64, 0);
        }
    }
    nested!(cpu, 0x0606_2aa0, models::spinner_turn(cpu, o));
}

/// The drop made (list 2, 72 bytes, plain_exec's): a spinner model high above (y 0x1280000, z
/// 0x1000000), lit, the model colours' ramp (31) and floor (15) set; the screen object's matrix
/// (+112) kept. Gives the object.
pub fn drop_set(cpu: &mut Cpu) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 72)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 2, blocks, PLAIN_EXEC));
    if o != 0 {
        nested!(cpu, 0x0606_2af8, models::spinner_init(cpu, o));
        cpu.write32(o + BEHAVIOUR, 0x0608_aa38);
        cpu.write32(o + POSITION, 0);
        cpu.write32(o + POSITION + 4, 0x0128_0000);
        cpu.write32(o + POSITION + 8, 0x0100_0000);
        cpu.write8(o + 61, 0);
        nested!(cpu, 0x0606_3380, gouraud::models_lit(cpu, 0x20, 1));
        nested!(cpu, 0x0606_3168, gouraud::table_ramp(cpu, 31, 31, 31));
        nested!(cpu, 0x0606_30e0, gouraud::table_floor(cpu, 15, 15, 15));
    }
    let draw = cpu.read32(SCREEN_DRAW_OBJECT);
    cpu.call(SL_GET_MATRIX, &[draw + 112]);
    o
}

/// The drop's scene started: the kept matrix inverted and a point (0, 0, 0) put through it, the
/// point's angles from the kept ones, the progress cleared, the view put along the point, the drop made.
pub fn drop_start(cpu: &mut Cpu) {
    cpu.call(SL_PUSH_MATRIX, &[]);
    cpu.call(SL_LOAD_MATRIX, &[VIEW_MATRIX]);
    cpu.call(SL_INVERS_MATRIX, &[]);
    cpu.call(SL_CALC_POINT, &[0, 0, 0, VIEW_POINT]);
    for k in 0..3 {
        let w = cpu.read16(VIEW_ANGLES + 2 * k) as i16 as i32 as u32;
        cpu.write32(VIEW_POINT + 12 + 4 * k, w);
    }
    cpu.write32(WORLD, 0);
    nested!(cpu, 0x0605_f5b0, bg::view_along(cpu, VIEW_POINT));
    cpu.call(SL_POP_MATRIX, &[]);
    nested!(cpu, 0x0608_aaf8, drop_set(cpu));
}

/// The drop's frame (the scroll object `o`): the progress up by 0x100 (to 0x10000 at most), the
/// view put along the point by it, the angles +0x10, +0x12 and +0x14 taken from the view's, the edge
/// line of the first (+0x20); the blocks' spins (+0x20000, +0x40000 wrapped to 0x2000000); each
/// block's rotation made from the kept matrix (as `screen_frame`); the beam drawn, the lines'
/// table made from the edge line and the second angle and transferred, the angles' zooms set; the
/// scroll goes on at progress 0x10000.
pub fn drop_frame(cpu: &mut Cpu, o: u32) {
    let progress = cpu.read32(WORLD).wrapping_add(0x100);
    cpu.write32(WORLD, progress);
    if progress as i32 > 0xffff {
        cpu.write32(WORLD, 0x1_0000);
    }
    nested!(cpu, 0x0605_f648, bg::view_along_eased(cpu, VIEW_POINT));
    let w = cpu.read16(WORLD_ANGLE_X);
    cpu.write16(o + 0x10, w);
    let w = cpu.read16(WORLD_ANGLE_Y);
    cpu.write16(o + 0x12, w);
    let w = cpu.read16(WORLD_ANGLE_Z);
    cpu.write16(o + 0x14, w);
    let angle = cpu.read16(WORLD_ANGLE_X) as i16 as i32 as u32;
    let line = nested!(cpu, 0x0605_f860, bg::edge_line(cpu, angle));
    cpu.write32(o + 0x20, line);
    let a = cpu.read32(BLOCK_A + 16).wrapping_add(0x2_0000) & SKIP_ALL;
    cpu.write32(BLOCK_A + 16, a);
    let b = cpu.read32(BLOCK_B + 16).wrapping_add(0x4_0000) & SKIP_ALL;
    cpu.write32(BLOCK_B + 16, b);
    cpu.call(SL_PUSH_MATRIX, &[]);
    nested!(cpu, 0x0605_f468, bg::view_matrix(cpu));
    cpu.call(ROTATION_SIDE, &[0]);
    nested!(cpu, 0x0605_f530, bg::rotation_matrix(cpu, BLOCK_A));
    cpu.call(ROTATION_SIDE, &[1]);
    nested!(cpu, 0x0605_f530, bg::rotation_matrix(cpu, BLOCK_B));
    cpu.call(SL_POP_MATRIX, &[]);
    nested!(cpu, 0x0608_a09c, beam_draw(cpu, o));
    let line = cpu.read32(o + 0x20);
    let tilt = cpu.read16(WORLD_ANGLE_Y) as i16 as i32 as u32;
    nested!(cpu, 0x0605_f8c4, bg::edge_window(cpu, SCREEN_PLACES, line, tilt));
    cpu.call(0x0603_4a50, &[SCREEN_PLACES, VDP2_LINES, 0x380]);
    nested!(cpu, 0x0608_a3c0, screen_angles(cpu, o));
    if cpu.read32(WORLD) == 0x1_0000 {
        nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
    }
}

/// A white flash's behaviour: its scales +0x20 and +0x24 growing (0x8000 and 0x4000 a frame), the
/// screen object's three colour words (+92, +94, +96) up by 40, and after 7 frames (+60) it ends.
pub fn whiten_behaviour(cpu: &mut Cpu, o: u32) {
    let v = cpu.read32(o + 0x20).wrapping_add(0x8000);
    cpu.write32(o + 0x20, v);
    let v = cpu.read32(o + 0x24).wrapping_add(0x4000);
    cpu.write32(o + 0x24, v);
    let draw = cpu.read32(SCREEN_DRAW_OBJECT);
    for k in [92, 94, 96] {
        let w = cpu.read16(draw + k).wrapping_add(40);
        cpu.write16(draw + k, w);
    }
    let n = cpu.read32(o + 60).wrapping_add(1);
    cpu.write32(o + 60, n);
    if n == 7 {
        cpu.write8(o + MODE, ENDING);
    }
}

/// The flash made (list 0, 64 bytes, plain_exec's): at y -0x380000, scales +0x24 0x4000, sprite
/// byte +0x2e 0, +0x2f -45, +0x32 0, +0x33 8, the colours (+0x28 and +0x2a) 0x70 on the character base's.
pub fn whiten_set(cpu: &mut Cpu) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, 64)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 0, blocks, PLAIN_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + SPRITE));
    cpu.write32(o + POSITION, 0);
    cpu.write32(o + POSITION + 4, 0xffc8_0000);
    cpu.write32(o + POSITION + 8, 0);
    cpu.write32(o + BEHAVIOUR, 0x0608_ad64);
    cpu.write8(o + 50, 0);
    cpu.write8(o + 51, 8);
    cpu.write32(o + SCALE_Y, 0x4000);
    cpu.write8(o + FRAME, 0xd3);
    cpu.write8(o + SPRITE_KIND, 0);
    let colour = cpu.read16(CHAR_BASE).wrapping_add(0x70);
    cpu.write16(o + COLOUR_B, colour);
    cpu.write16(o + SPRITE_COLOUR, colour);
    cpu.write32(o + 60, 0);
}

/// The next scene's screen set up (its scroll object `s`): the screen's draw (+0x58) the beam's, the
/// second block cleared, the scroll's step +8, angles +0x10 and +0x12 and longs +0x20-+0x30 cleared,
/// the helper at 0x0608dd04 told VRAM 0x25e44000, two palettes (0x06091c94: 16 from 0x80; 0x06091c74: 16
/// from 0x100), the window, NBG1's page, plane B put at (0x440000, 0x700000), the colour calculation
/// (0x52), the colour rates of screens 0 and 5 16, the flash made, the planes 0x17.
pub fn world_set(cpu: &mut Cpu, s: u32) {
    let draw = cpu.read32(SCREEN_DRAW_OBJECT);
    cpu.write32(draw + SCREEN_DRAW, 0x0608_a09c);
    block_clear(cpu, BLOCK_B, 0, 0);
    cpu.write8(s + 8, 0);
    cpu.write16(s + 0x10, 0);
    cpu.write16(s + 0x12, 0);
    for k in [0x20, 0x24, 0x28, 0x2c, 0x30] {
        cpu.write32(s + k, 0);
    }
    cpu.call(0x0608_dd04, &[0x25e4_4000]);
    nested!(cpu, 0x0600_5b18, palette::set_colours(cpu, 0x0609_1c94, 16, 0x80));
    nested!(cpu, 0x0600_5b18, palette::set_colours(cpu, 0x0609_1c74, 0x100, 16));
    cpu.call(0x0606_e17c, &[0, 3]);
    cpu.call(0x0606_e2e4, &[0x25e7_0000, 0x400, 0x8000]);
    let b = bg::PLANE_B;
    cpu.write32(b + bg::X, 0x44_0000);
    cpu.write32(b + bg::Y, 0x70_0000);
    cpu.write32(b + bg::SPEED_Y, 0);
    cpu.write32(b + bg::SPEED_X, 0);
    cpu.write32(b + bg::ZOOM_Y, 0x1_0000);
    cpu.write32(b + bg::ZOOM_X, 0x1_0000);
    nested!(cpu, 0x0605_f08c, bg::plane_b_frame(cpu));
    cpu.call(0x0607_0ddc, &[0x52]);
    cpu.call(0x0606_e45c, &[0, 16]);
    cpu.call(0x0606_e45c, &[5, 16]);
    nested!(cpu, 0x0608_adb8, whiten_set(cpu));
    cpu.call(0x0606_e23c, &[23]);
}

/// The view's motion (the scroll object `o`): the angles +0x24 and +0x28 turned on by the words +0x10
/// and +0x12, the longs +16 and +20 of the view made from the radii +0x2c and +0x30 by the cosine
/// of the word +0x26 and the sine of +0x2a; the second block's spin +0x10 on by 0x10000.
pub fn world_step(cpu: &mut Cpu, o: u32) {
    let v = cpu.read32(o + 36).wrapping_add(cpu.read16(o + 0x10) as i16 as i32 as u32);
    cpu.write32(o + 36, v);
    let v = cpu.read32(o + 40).wrapping_add(cpu.read16(o + 0x12) as i16 as i32 as u32);
    cpu.write32(o + 40, v);
    let angle = cpu.read16(o + 38) as i16 as i32 as u32;
    let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, angle));
    let r = cpu.read32(o + 44);
    let x = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, r, cos));
    cpu.write32(WORLD + 16, x);
    let angle = cpu.read16(o + 42) as i16 as i32 as u32;
    let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, angle));
    let r = cpu.read32(o + 48);
    let y = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, r, sin));
    cpu.write32(WORLD + 20, y);
    let spin = cpu.read32(BLOCK_B + 16).wrapping_add(0x1_0000) & SKIP_ALL;
    cpu.write32(BLOCK_B + 16, spin);
}

/// The view's matrix made: the unit matrix turned by the view's angles negated (x, y, z) and moved
/// by its place negated.
pub fn world_view(cpu: &mut Cpu) {
    cpu.call(SL_UNIT_MATRIX, &[0]);
    for (at, f) in [(WORLD_ANGLE_X, SL_ROT_X), (WORLD_ANGLE_Y, SL_ROT_Y), (WORLD_ANGLE_Z, SL_ROT_Z)] {
        let a = (cpu.read16(at) as i16 as i32).wrapping_neg() as i16 as i32 as u32;
        cpu.call(f, &[a]);
    }
    let (x, y, z) = (cpu.read32(WORLD + 4), cpu.read32(WORLD + 8), cpu.read32(WORLD + 12));
    cpu.call(SL_TRANSLATE, &[x.wrapping_neg(), y.wrapping_neg(), z.wrapping_neg()]);
}

/// The first plane scrolled by 4 dots a frame at the view's third angle: its speed (-4 times the
/// sine, 4 times the cosine) and its frame.
pub fn world_scroll(cpu: &mut Cpu) {
    let a = cpu.read16(WORLD_ANGLE_Z) as i16 as i32 as u32;
    let cos = nested!(cpu, 0x0606_9a30, math::sl_cos(cpu, a));
    let y = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, 0x4_0000, cos));
    let sin = nested!(cpu, 0x0606_9a0c, math::sl_sin(cpu, a));
    let x = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, 0xfffc_0000, sin));
    nested!(cpu, 0x0605_efd4, bg::plane_a_speed(cpu, x, y));
    nested!(cpu, 0x0605_f040, bg::plane_a_frame(cpu));
}

/// The second block's rotation made in the view's matrix (the matrix kept, `world_view` applied).
fn world_matrix_b(cpu: &mut Cpu) {
    cpu.call(SL_PUSH_MATRIX, &[]);
    nested!(cpu, 0x0608_b00c, world_view(cpu));
    nested!(cpu, 0x0605_f530, bg::rotation_matrix(cpu, BLOCK_B));
    cpu.call(SL_POP_MATRIX, &[]);
}

/// The scene's frame (its scroll object `o`, a step in +8): 0, a count +0x20 (a frame each; the
/// colour offset white at 7; at 8 the screen's wave made, the colours faded to nothing at speed 0x60,
/// the count cleared, the step 1); 1, the radii +0x2c and +0x30 growing (8 a frame, 32 once the first is
/// past 0x7ff) and the angle words +0x10 and +0x12 by 1 (below 0x100), by 2 (below 0x200), then the
/// step 2; 2, the ease of the view +24 from 0x80 to 0x10080 by the count (+0x100 a frame), the lens made
/// at 0x8000, the step 3 at 0x10000 (the count cleared); 2 and 3, the radii falling by 16 while
/// they are positive. Then the motion, the two blocks' rotations and the view's matrix, the scroll.
pub fn world_frame(cpu: &mut Cpu, o: u32) {
    let step = cpu.read8(o + 8);
    if step <= 3 {
        match step {
            0 => {
                let n = cpu.read32(o + 32) as i32;
                cpu.write32(o + 32, n.wrapping_add(1) as u32);
                if n == 7 {
                    nested!(cpu, 0x0605_e6dc, screen::set_fade_offset(cpu, 0xff, 0xff, 0xff));
                } else if n == 8 {
                    nested!(cpu, 0x0608_a590, screen_wave_make(cpu));
                    nested!(cpu, 0x0605_e93c, screen::fade_to(cpu, 0, 0, 0, 96));
                    cpu.write32(o + 32, 0);
                    cpu.write8(o + 8, step + 1);
                }
            }
            1 => {
                let r = cpu.read32(o + 44) as i32;
                let (r, s) = if r > 0x7ff { (r.wrapping_add(32), 32) } else { (r.wrapping_add(8), 8) };
                cpu.write32(o + 44, r as u32);
                let s = (cpu.read32(o + 48) as i32).wrapping_add(s);
                cpu.write32(o + 48, s as u32);
                let w = cpu.read16(o + 0x10);
                let wide = w as i16 as i32;
                if wide <= 0xff {
                    cpu.write16(o + 0x10, w.wrapping_add(1));
                    let v = cpu.read16(o + 0x12).wrapping_add(1);
                    cpu.write16(o + 0x12, v);
                } else if wide <= 0x1ff {
                    cpu.write16(o + 0x10, w.wrapping_add(2));
                    let v = cpu.read16(o + 0x12).wrapping_add(2);
                    cpu.write16(o + 0x12, v);
                } else {
                    cpu.write32(o + 32, 0);
                    cpu.write8(o + 8, step + 1);
                }
            }
            2 => {
                let count = cpu.read32(o + 32).wrapping_add(0x100);
                cpu.write32(o + 32, count);
                let e = nested!(cpu, 0x0606_2ebc, math::ease(cpu, 0x80, 0x1_0080, count));
                cpu.write32(WORLD + 24, e);
                let count = cpu.read32(o + 32);
                if count == 0x8000 {
                    nested!(cpu, 0x0608_aa08, lens_set(cpu));
                } else if count == 0x1_0000 {
                    cpu.write32(o + 32, 0);
                    cpu.write8(o + 8, step + 1);
                }
                world_shrink(cpu, o);
            }
            _ => world_shrink(cpu, o),
        }
    }
    nested!(cpu, 0x0608_af84, world_step(cpu, o));
    let a = cpu.read32(BLOCK_A + 16).wrapping_add(0x2_0000) & SKIP_ALL;
    cpu.write32(BLOCK_A + 16, a);
    cpu.call(SL_PUSH_MATRIX, &[]);
    nested!(cpu, 0x0605_f468, bg::view_matrix(cpu));
    cpu.call(ROTATION_SIDE, &[0]);
    nested!(cpu, 0x0605_f530, bg::rotation_matrix(cpu, BLOCK_A));
    cpu.call(SL_POP_MATRIX, &[]);
    cpu.call(SL_PUSH_MATRIX, &[]);
    nested!(cpu, 0x0608_b00c, world_view(cpu));
    cpu.call(ROTATION_SIDE, &[1]);
    nested!(cpu, 0x0605_f530, bg::rotation_matrix(cpu, BLOCK_B));
    cpu.call(SL_POP_MATRIX, &[]);
    nested!(cpu, 0x0608_b078, world_scroll(cpu));
}

/// The radii +0x2c and +0x30 of the scene's scroll object falling by 16 while the first is positive.
fn world_shrink(cpu: &mut Cpu, o: u32) {
    let r = cpu.read32(o + 44) as i32;
    if r > 0 {
        cpu.write32(o + 44, r.wrapping_sub(16) as u32);
        let s = cpu.read32(o + 48).wrapping_sub(16);
        cpu.write32(o + 48, s);
    }
}

/// The two sprites' (0x060740f0, 0x060740f4) colour words +0xa0, +0xa2, +0xa4, +0xa6 set from the
/// words at 0x0602ee20 and 0x0602ee28 with their low 12 bits cleared.
const SPRITE_PAIR: u32 = 0x0607_40f0;

pub fn world_colours(cpu: &mut Cpu) {
    for (k, table) in [(0, 0x0602_ee20u32), (4, 0x0602_ee28)] {
        let s = cpu.read32(SPRITE_PAIR + k);
        if s != 0 {
            for j in 0..4u32 {
                let w = cpu.read16(table + 2 * j) & 0xf000;
                cpu.write16(s + 0xa0 + 2 * j, w);
            }
        }
    }
}

/// The scene's start (its scroll object `o`): its step +8 and count +0x20 cleared, the helpers at
/// 0x0608dcd4 and 0x0608dce4 told 1, the view's ease 0x80, the message at 0x06091468, the byte at
/// 0x0600ed90 set, the sprites' colours.
pub fn world_start(cpu: &mut Cpu, o: u32) {
    cpu.write8(o + 8, 0);
    cpu.write32(o + 32, 0);
    cpu.call(0x0608_dcd4, &[1]);
    cpu.call(ROTATION_SIDE, &[1]);
    cpu.write32(WORLD + 24, 0x80);
    nested!(cpu, 0x0606_1620, text::message_set(cpu, 0x0609_1468));
    cpu.write8(0x0600_ed90, 1);
    nested!(cpu, 0x0608_b2b8, world_colours(cpu));
}

/// The scene's closing frame (its scroll object `o`): the motion; once the step is 0 the angle words
/// +0x10 and +0x12 fall by 2 while the first is past 0x100, and the radii by 12 down to -0x800,
/// where the scroll goes on; the second block's rotation, the scroll and the sprites' colours.
pub fn world_frame_out(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_af84, world_step(cpu, o));
    if cpu.read8(o + 8) == 0 {
        let w = cpu.read16(o + 0x10);
        if w as i16 as i32 > 0x100 {
            cpu.write16(o + 0x10, w.wrapping_sub(2));
            let v = cpu.read16(o + 0x12).wrapping_sub(2);
            cpu.write16(o + 0x12, v);
        }
        let r = cpu.read32(o + 44) as i32;
        if r > -0x800 {
            cpu.write32(o + 44, r.wrapping_sub(12) as u32);
            let s = cpu.read32(o + 48).wrapping_sub(12);
            cpu.write32(o + 48, s);
        } else {
            cpu.write32(o + 44, -0x800i32 as u32);
            nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
        }
    }
    world_matrix_b(cpu);
    nested!(cpu, 0x0608_b078, world_scroll(cpu));
    nested!(cpu, 0x0608_b2b8, world_colours(cpu));
}

/// The time left (+52 of the scene's scroll object, frames) shown as in the first stage's count:
/// the seconds left (rounded up) on the text plane, two large digits, or one, centred, below ten.
const TIME_TENS_AT: u32 = 0x25e7_e228;
const TIME_UNITS_AT: u32 = 0x25e7_e22c;
const TIME_UNIT_AT: u32 = 0x25e7_e22a;
const TIME_TWO_SPACES: u32 = 0x0608_b448;

pub fn timer_show(cpu: &mut Cpu, o: u32) {
    let seconds = (cpu.read32(o + 52) as i32).wrapping_add(60) / 60;
    let (tens, units) = (seconds / 10, seconds % 10);
    let digit = |d: i32| (d + b'0' as i32) as i8 as i32 as u32;
    if tens != 0 {
        nested!(cpu, 0x0606_111c, text::put_large_char(cpu, digit(tens), TIME_TENS_AT, 0));
        nested!(cpu, 0x0606_111c, text::put_large_char(cpu, digit(units), TIME_UNITS_AT, 0));
    } else {
        if units == 9 {
            nested!(cpu, 0x0606_1204, text::put_large_string(cpu, TIME_TWO_SPACES, TIME_TENS_AT, 0));
        }
        nested!(cpu, 0x0606_111c, text::put_large_char(cpu, digit(units), TIME_UNIT_AT, 0));
    }
}

/// The timer's start (the scroll object `o`): 3600 frames (+52), its step +8 and count +0x20
/// cleared, the helper at 0x0608da04.
pub fn timer_start(cpu: &mut Cpu, o: u32) {
    cpu.write32(o + 52, 0xe10);
    cpu.write8(o + 8, 0);
    cpu.write32(o + 32, 0);
    cpu.call(0x0608_da04, &[]);
}

/// The timer's frame (the scroll object `o`): a frame off the time, shown; the motion, the second
/// block's rotation, the view's ease +24 down by 8, the scroll and the sprites' colours; the
/// scroll goes on once the time is below 0.
pub fn timer_frame(cpu: &mut Cpu, o: u32) {
    let n = cpu.read32(o + 52).wrapping_sub(1);
    cpu.write32(o + 52, n);
    nested!(cpu, 0x0608_b44c, timer_show(cpu, o));
    nested!(cpu, 0x0608_af84, world_step(cpu, o));
    world_matrix_b(cpu);
    let e = cpu.read32(WORLD + 24).wrapping_sub(8);
    cpu.write32(WORLD + 24, e);
    nested!(cpu, 0x0608_b078, world_scroll(cpu));
    nested!(cpu, 0x0608_b2b8, world_colours(cpu));
    if (cpu.read32(o + 52) as i32) < 0 {
        nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
    }
}

/// A ring (rings.rs's, described on the stack) at (0, -0x380000), turned 0xf400 about x, 0x900 across,
/// tilted by 0x4000 and 0xc000 negated, in the colour 0x50 on the character base's; the sound 88.
pub fn ring_burst(cpu: &mut Cpu) {
    let d = cpu.stack();
    cpu.write32(d + 12, 0);
    cpu.write32(d + 16, 0xffc8_0000);
    cpu.write32(d + 20, 0);
    cpu.write16(d + 4, 0xf400);
    cpu.write16(d + 6, 0);
    cpu.write16(d + 8, 0);
    cpu.write32(d + 24, 0);
    cpu.write32(d + 28, 0);
    cpu.write32(d + 44, 0x50_0000);
    cpu.write32(d + 40, 0);
    let colour = cpu.read16(CHAR_BASE).wrapping_add(0x50);
    cpu.write16(d + 2, colour);
    cpu.write32(d + 32, 0x900);
    cpu.write32(d + 36, 0x900);
    cpu.write32(d + 48, 0x4000);
    cpu.write32(d + 52, 0xffff_c000);
    cpu.write32(d + 56, 0);
    cpu.write32(d + 60, 0);
    cpu.write32(d + 64, 0);
    cpu.write8(d, 1);
    nested!(cpu, 0x0606_2738, rings::ring_set(cpu, d));
    nested!(cpu, 0x0600_50d4, sound::se_set(cpu, 88));
}

/// The last scene's start (the scroll object `o`): 0x100 frames (+52), the colour offset sound 95,
/// the screen faded to white at speed 0x100, the time's digit place cleared.
pub fn final_start(cpu: &mut Cpu, o: u32) {
    cpu.write32(o + 52, 0x100);
    nested!(cpu, 0x0606_e25c, sgl::sl_col_offset_on(cpu, 95, 0));
    nested!(cpu, 0x0605_e93c, screen::fade_to(cpu, 0xff, 0xff, 0xff, 0x100));
    nested!(cpu, 0x0606_111c, text::put_large_char(cpu, 0x20, TIME_UNIT_AT, 0));
}

/// The last scene's frame (the scroll object `o`): the motion; at 0xc0 frames left a ring burst; the
/// second block's rotation, the scroll and the sprites' colours; a frame off (+52), and the scroll
/// goes on once it is below 0.
pub fn final_frame(cpu: &mut Cpu, o: u32) {
    nested!(cpu, 0x0608_af84, world_step(cpu, o));
    if cpu.read32(o + 52) == 0xc0 {
        nested!(cpu, 0x0608_b5b8, ring_burst(cpu));
    }
    world_matrix_b(cpu);
    nested!(cpu, 0x0608_b078, world_scroll(cpu));
    nested!(cpu, 0x0608_b2b8, world_colours(cpu));
    let n = cpu.read32(o + 52).wrapping_sub(1);
    cpu.write32(o + 52, n);
    if (n as i32) < 0 {
        nested!(cpu, 0x0605_e648, screen::scroll_next(cpu));
    }
}

/// The scene's second part's set-up (its scroll object `o`): the next mode 3, the colour offset
/// sound 91, the fades stopped and the colour offset white, NBG0's characters, page, map and
/// zoom, NBG1's, the windows, NBG3's, the plane between (VRAM split 2), the screens' priorities
/// (2: 7, 1: 5, 0: 4, 0xfff9...: 4), the planes' places ((0xb00000, 0x780000), zooms one), the
/// scroll's step +8, +9, +0x20 and +0x28 cleared, the music 9, the view's place and ease, the
/// kept matrix got from the screen object's, the sprites' pair hidden (+109, +0x90 cleared,
/// the top flag set).
pub fn part_set_up(cpu: &mut Cpu, o: u32) {
    cpu.write8(NEXT_MODE, 3);
    nested!(cpu, 0x0606_e25c, sgl::sl_col_offset_on(cpu, 91, 0));
    nested!(cpu, 0x0605_e9d8, screen::stop_fades(cpu));
    nested!(cpu, 0x0605_e6dc, screen::set_fade_offset(cpu, 0xff, 0xff, 0xff));
    nested!(cpu, 0x0606_e288, sgl::sl_char_nbg0(cpu, 0x10, 0));
    nested!(cpu, 0x0606_e300, sgl::sl_page_nbg0(cpu, 0x25e4_0000, 0x100, 0x4000));
    nested!(cpu, 0x0607_0c4c, sgl::sl_map_nbg0(cpu, 0x25e6_0000, 0x25e6_0000, 0x25e6_0000, 0x25e6_0000));
    nested!(cpu, 0x0607_0cfc, sgl::sl_zoom_mode(cpu, 1, 0));
    nested!(cpu, 0x0606_e294, sgl::sl_char_nbg1(cpu, 0x10, 0));
    nested!(cpu, 0x0606_e2e4, sgl::sl_page_nbg1(cpu, 0x25e0_0000, 0x200, 0x4000));
    nested!(cpu, 0x0607_0d1c, sgl::sl_map_nbg1(cpu, 0x25e6_4000, 0x25e6_4000, 0x25e6_4000, 0x25e6_4000));
    for (screen, v) in [(1, 0), (0, 0), (3, 3)] {
        cpu.call(0x0606_e17c, &[screen, v]);
    }
    cpu.call(0x0608_de54, &[0, 0x20, 0x15f, 0xdf]);
    nested!(cpu, 0x0606_e2b6, sgl::sl_char_nbg3(cpu, 0, 0));
    nested!(cpu, 0x0606_e2f4, sgl::sl_page_nbg3(cpu, 0x25e7_8000, 0x600, 0x8000));
    cpu.call(0x0608_dea4, &[0]);
    cpu.call(0x0608_deb4, &[0x25e6_8000, 0x25e6_8000, 0x25e6_8000, 0x25e6_8000]);
    nested!(cpu, 0x0605_eee4, screen::fill_cells(cpu, 0, 0x25e6_8000, 64, 64));
    cpu.call(0x0608_df74, &[0, 0]);
    nested!(cpu, 0x0606_98dc, sgl::sl_priority(cpu, 2, 7));
    nested!(cpu, 0x0606_dd4c, sgl::vram_split(cpu, 2));
    nested!(cpu, 0x0606_dd6c, sgl::sl_scr_auto_disp(cpu, 15));
    cpu.call(0x0606_e23c, &[0x000a_0005]);
    for plane in [bg::PLANE_A, bg::PLANE_B] {
        cpu.write32(plane + bg::X, 0xb0_0000);
        cpu.write32(plane + bg::Y, 0x78_0000);
        cpu.write32(plane + bg::SPEED_Y, 0);
        cpu.write32(plane + bg::SPEED_X, 0);
        cpu.write32(plane + bg::ZOOM_Y, 0x1_0000);
        cpu.write32(plane + bg::ZOOM_X, 0x1_0000);
        if plane == bg::PLANE_A {
            nested!(cpu, 0x0605_f040, bg::plane_a_frame(cpu));
        } else {
            nested!(cpu, 0x0605_f08c, bg::plane_b_frame(cpu));
        }
    }
    nested!(cpu, 0x0606_dd5c, sgl::sl_scr_transparent(cpu, 3));
    cpu.call(0x0607_0ddc, &[0]);
    for (screen, pri) in [(1, 5), (0, 4), (-7i32 as u32, 4)] {
        nested!(cpu, 0x0606_98dc, sgl::sl_priority(cpu, screen, pri));
    }
    cpu.write8(o + 8, 0);
    cpu.write32(o + 40, 0);
    cpu.write32(o + 32, 0);
    cpu.write8(o + 9, 0);
    nested!(cpu, 0x0600_4e98, sound::request_music(cpu, 9));
    cpu.write32(WORLD + 4, 0x21_0000);
    cpu.write32(WORLD + 8, 0xce_0000);
    cpu.write32(WORLD + 12, 0x2_0000);
    cpu.write32(WORLD + 16, 0xffff_3b60);
    cpu.write32(WORLD + 20, 0);
    cpu.write32(WORLD + 24, 0xffff_9000);
    cpu.call(SL_PUSH_MATRIX, &[]);
    nested!(cpu, 0x0605_f468, bg::view_matrix(cpu));
    let draw = cpu.read32(SCREEN_DRAW_OBJECT);
    cpu.call(SL_GET_MATRIX, &[draw + 112]);
    cpu.call(SL_POP_MATRIX, &[]);
    nested!(cpu, 0x0606_4e54, models::pods_shading(cpu));
    nested!(cpu, 0x0606_1738, player::scene_on(cpu));
    for k in [0, 4] {
        let s = cpu.read32(SPRITE_PAIR + k);
        if s != 0 {
            cpu.write8(s + 109, 0);
            cpu.write16(s + 0x90, 0);
            let f = cpu.read32(s + 16) | 0x8000_0000;
            cpu.write32(s + 16, f);
        }
    }
}

/// The picture part's cells, colours and map put in the pictures' places (the scroll object `o`: +0x20
/// the plane in use, +0x28 the picture's number; the file's parts `base`, as `screen::picture_part`
/// reads them; the pictures' table `table` of 40-byte entries: the cells' size +8, the colours' count
/// word +14, the map's width +18 and height +22): the plane's cells by a long copy, its colours, the
/// map put by long cells, the bit 2 of the word at 0x06030e30 set.
pub fn picture_load(cpu: &mut Cpu, o: u32, base: u32, table: u32) {
    let d = cpu.stack();
    let second = cpu.read32(o + 32) != 0;
    let (cells_at, map_at, colours, add) = if second { (0x25e0_0000, 0x25e6_4000, 0x200, 0) } else { (0x25e4_0000, 0x25e6_0000, 0x100, 0x2000) };
    cpu.write32(d, add);
    cpu.write32(d + 4, map_at);
    let map_part = cpu.read32(base + 8);
    cpu.write32(d + 8, map_part);
    let part = |cpu: &mut Cpu, k: u32| screen::picture_part(cpu, base, k);
    let entry = |cpu: &mut Cpu| table + 40 * cpu.read32(o + 40);
    let cells = part(cpu, 0);
    let e = entry(cpu);
    let size = cpu.read32(e + 8);
    nested!(cpu, 0x0605_ed2c, screen::queue_long_copy(cpu, cells, cells_at, size, 0x2000));
    let e = entry(cpu);
    let count = cpu.read16(e + 14) as u32;
    let colour_data = part(cpu, 1);
    nested!(cpu, 0x0600_5b18, palette::set_colours(cpu, colour_data, colours, count));
    let c = cpu.read32(0x0603_0e30) | 4;
    cpu.write32(0x0603_0e30, c);
    let e = entry(cpu);
    let char_offset = colours >> 4;
    let (w, h) = (cpu.read16(e + 18) as u32, cpu.read16(e + 22) as u32);
    let map = screen::picture_part(cpu, base, 2);
    cpu.nested_with_stack(&PICTURE_CELLS_PROLOGUE, 0x0605_ee94, &[char_offset, add], |cpu| screen::put_long_cells(cpu, map, map_at, w, h, char_offset, add));
}

static PICTURE_CELLS_PROLOGUE: std::sync::OnceLock<super::Prologue> = std::sync::OnceLock::new();
