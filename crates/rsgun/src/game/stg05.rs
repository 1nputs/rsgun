//! STG05.BIN's code, which the last stage's mode (game mode 11) runs in the overlay window
//! (fingerprint 2adefae5540dd556): its mode's code, its enemies and scenery, its scrolls.

use super::globals::{GAME_FRAMES, LOOPS};
use super::{actors, aim, bullets, camera, clib, dma, effects, explosions, files, hits, hud, kinds, math, nested, objects, ribbons, screen, sgl, sound, sprites, states, timeline, Cpu};

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
