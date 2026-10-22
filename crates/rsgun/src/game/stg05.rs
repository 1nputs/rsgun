//! STG05.BIN's code, which the last stage's mode (game mode 11) runs in the overlay window
//! (fingerprint 2adefae5540dd556): its mode's code, its enemies and scenery, its scrolls.

use super::globals::{GAME_FRAMES, LOOPS};
use super::{actors, aim, bullets, dma, explosions, files, hits, kinds, math, nested, objects, screen, sound, sprites, states, timeline, Cpu};

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
