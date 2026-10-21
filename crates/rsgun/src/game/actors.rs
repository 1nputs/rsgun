//! The actors: the objects that play (the players, their shots, the enemies and their bullets ...):
//! each a Base_obj (objects.rs) with a sprite or model at +0x10 (sprites.rs), hit boxes
//! (hits.rs) and a behaviour. Each frame an actor runs its behaviour, flashes while hit, registers
//! its boxes and is drawn.

use super::nested;
use super::globals::{GAME_FLAGS, HELD};
use super::{hits, objects, sprites, Cpu};

/// The objects' mode byte: new, running, ending, ended.
const OBJ_MODE: u32 = 3;
const ENDING: u8 = 0x70;
const ENDED: u8 = 0x73;
/// An actor's sprite (its flags first), its own colour and its flash's (the sprite's colour is one
/// of them), its models (two: plain, flashing) and the model drawn, its behaviour, its hit flags.
const SPRITE: u32 = 0x10;
const FLAGS: u32 = 0x10;
const COLOUR: u32 = 0x28;
const OWN_COLOUR: u32 = 0x2a;
const MODEL: u32 = 0x34;
const MODELS: u32 = 0x44;
const BEHAVIOUR: u32 = 0x38;
const HIT_FLAGS: u32 = 0x56;
/// The flags: not to be hurt, a model (else a sprite), drawn by its own function.
const UNHURT: u32 = 0x4_0000;
const IS_MODEL: u32 = 0x80_0000;
const OWN_DRAW: u32 = 0x40_0000;
/// The hit flags: flashing (turned over each frame while it was hit), was hit, and those cleared
/// once the behaviour has run (touched, was hit, hit something, the spark turn).
const FLASH: u16 = 1;
const WAS_HIT: u16 = 0x200;
const CLEARED: u16 = 0x0720;
/// The flash's colour (from the first character, as the game keeps it).
const CHAR_BASE: u32 = 0x0607_41fc;
const FLASH_COLOUR: u16 = 0xe0;
/// A mask of actor flags: an actor with any of them (bit 31 aside) ends at once (main_loop.rs
/// keeps it a frame).
const END_FLAGS: u32 = 0x0607_405c;
/// end_by_flags's SH-2 entry (for nested!).
const END_BY_FLAGS: u32 = 0x0604_6138;
/// The draw's SH-2 entry (for nested!).
const DRAW: u32 = 0x0604_0960;

/// Ends `actor` if it has any of the flags to end: 1, else 0.
pub fn end_by_flags(cpu: &mut Cpu, actor: u32) -> u32 {
    let flags = cpu.read32(actor + FLAGS) & 0x7fff_ffff & cpu.read32(END_FLAGS);
    if flags == 0 {
        return 0;
    }
    cpu.write8(actor + OBJ_MODE, ENDING);
    1
}

/// `from` moved toward `to` by at most `step`.
pub fn approach(_cpu: &mut Cpu, from: u32, to: u32, step: u32) -> u32 {
    let (from, to, step) = (from as i32, to as i32, step as i32);
    if to.wrapping_sub(from).wrapping_abs() > step {
        if from >= to { from.wrapping_sub(step) as u32 } else { from.wrapping_add(step) as u32 }
    } else {
        to as u32
    }
}

/// A numbered actor's further ending modes: ENDING_KEEP (its number kept) goes to FREE, which is
/// freed; ENDED clears its number first.
const ENDING_KEEP: u8 = 0x71;
const FREE: u8 = 0x72;
/// The numbered objects (their number at +0x32).
const NUMBERED: u32 = 0x0607_40f0;
const NUMBER: u32 = 0x32;

/// A new actor starts running (not while the game is held): whether it runs this frame.
fn start(cpu: &mut Cpu, actor: u32) -> bool {
    if cpu.read32(GAME_FLAGS) & HELD != 0 {
        return false;
    }
    cpu.write8(actor + OBJ_MODE, 1);
    true
}

/// An actor's frame (its exec): while the game is not held, its behaviour (unless it ends by the
/// flags), the flash, its boxes registered; then its colour or model for the flash, and drawn.
pub fn actor_exec(cpu: &mut Cpu, actor: u32) {
    match cpu.read8(actor + OBJ_MODE) {
        1 => {}
        0 if start(cpu, actor) => {}
        ENDING => {
            cpu.write8(actor + OBJ_MODE, ENDED);
            return;
        }
        ENDED => {
            nested!(cpu, 0x0600_579c, objects::obj_free(cpu, actor));
            return;
        }
        _ => return,
    }
    actor_frame(cpu, actor);
}

/// A numbered actor's frame (one with a number among the numbered objects): as actor_exec's, but
/// ending it frees its number (or, ending by ENDING_KEEP, keeps it).
pub fn numbered_exec(cpu: &mut Cpu, actor: u32) {
    match cpu.read8(actor + OBJ_MODE) {
        1 => {}
        0 if start(cpu, actor) => {}
        ENDING => {
            cpu.write8(actor + OBJ_MODE, ENDED);
            return;
        }
        ENDING_KEEP => {
            cpu.write8(actor + OBJ_MODE, FREE);
            return;
        }
        FREE => {
            nested!(cpu, 0x0600_579c, objects::obj_free(cpu, actor));
            return;
        }
        ENDED => {
            let n = cpu.read8(actor + NUMBER) as i8 as i32;
            cpu.write32(NUMBERED.wrapping_add((4 * n) as u32), 0);
            nested!(cpu, 0x0600_579c, objects::obj_free(cpu, actor));
            return;
        }
        _ => return,
    }
    actor_frame(cpu, actor);
}

/// A plain actor's frame (no boxes, no flash): while the game is not held, its behaviour (unless it
/// ends by the flags); drawn.
pub fn plain_exec(cpu: &mut Cpu, actor: u32) {
    match cpu.read8(actor + OBJ_MODE) {
        1 => {}
        0 if start(cpu, actor) => {}
        ENDING => {
            cpu.write8(actor + OBJ_MODE, ENDED);
            return;
        }
        ENDED => {
            nested!(cpu, 0x0600_579c, objects::obj_free(cpu, actor));
            return;
        }
        _ => return,
    }
    if cpu.read32(GAME_FLAGS) & HELD == 0 {
        if nested!(cpu, END_BY_FLAGS, end_by_flags(cpu, actor)) != 0 {
            return;
        }
        let behaviour = cpu.read32(actor + BEHAVIOUR);
        cpu.call(behaviour, &[actor]);
        if cpu.read8(actor + OBJ_MODE) as i8 >= ENDING as i8 {
            return;
        }
    }
    nested!(cpu, DRAW, sprites::draw(cpu, actor + SPRITE));
}

/// The running actor's frame (actor_exec's and numbered_exec's).
fn actor_frame(cpu: &mut Cpu, actor: u32) {
    if cpu.read32(GAME_FLAGS) & HELD == 0 {
        if nested!(cpu, END_BY_FLAGS, end_by_flags(cpu, actor)) != 0 {
            return;
        }
        let hf = cpu.read16(actor + HIT_FLAGS);
        let hf = if hf & WAS_HIT != 0 { hf ^ FLASH } else { hf & !FLASH };
        cpu.write16(actor + HIT_FLAGS, hf);
        if cpu.read32(actor + FLAGS) & UNHURT != 0 {
            let hf = cpu.read16(actor + HIT_FLAGS);
            cpu.write16(actor + HIT_FLAGS, hf & !FLASH);
        }
        let behaviour = cpu.read32(actor + BEHAVIOUR);
        cpu.call(behaviour, &[actor]);
        if cpu.read8(actor + OBJ_MODE) as i8 >= ENDING as i8 {
            return;
        }
        let hf = cpu.read16(actor + HIT_FLAGS);
        cpu.write16(actor + HIT_FLAGS, hf & !CLEARED);
        nested!(cpu, hits::REGISTER_BOXES, hits::register_boxes(cpu, actor));
    } else {
        let hf = cpu.read16(actor + HIT_FLAGS);
        cpu.write16(actor + HIT_FLAGS, hf & !FLASH);
    }
    let flags = cpu.read32(actor + FLAGS);
    if flags & IS_MODEL == 0 {
        let colour = if cpu.read16(actor + HIT_FLAGS) & FLASH != 0 {
            cpu.read16(CHAR_BASE).wrapping_add(FLASH_COLOUR)
        } else {
            cpu.read16(actor + OWN_COLOUR)
        };
        cpu.write16(actor + COLOUR, colour);
    } else if flags & OWN_DRAW == 0 {
        let flash = (cpu.read16(actor + HIT_FLAGS) & FLASH) as u32;
        let models = cpu.read32(actor + MODELS);
        let model = cpu.read32(models + 4 * flash);
        cpu.write32(actor + MODEL, model);
    }
    nested!(cpu, DRAW, sprites::draw(cpu, actor + SPRITE));
}

/// An animation (a script of frame, frames-to-show byte pairs, 0xff to end): the script at +0x4c,
/// the frames left of the current at +0x50, the sprite's frame at +0x2f.
const SCRIPT: u32 = 0x4c;
const SHOWN_LEFT: u32 = 0x50;
const SPRITE_FRAME: u32 = 0x2f;
const SCRIPT_END: u8 = 0xff;

/// One frame of `actor`'s animation: the next frame taken when the current's time is out (the
/// script's end ends the actor): 1 at the end, else 0.
pub fn anim_step(cpu: &mut Cpu, actor: u32) -> u32 {
    if cpu.read8(actor + SHOWN_LEFT) as i8 <= 0 {
        let script = cpu.read32(actor + SCRIPT);
        let frame = cpu.read8(script);
        cpu.write8(actor + SPRITE_FRAME, frame);
        if frame == SCRIPT_END {
            cpu.write8(actor + OBJ_MODE, ENDING);
            return 1;
        }
        let script = cpu.read32(actor + SCRIPT);
        let time = cpu.read8(script + 1);
        cpu.write8(actor + SHOWN_LEFT, time);
        let script = cpu.read32(actor + SCRIPT);
        cpu.write32(actor + SCRIPT, script + 2);
    }
    let left = cpu.read8(actor + SHOWN_LEFT);
    cpu.write8(actor + SHOWN_LEFT, left.wrapping_sub(1));
    0
}

/// A drifting effect's behaviour: animated, moved by its speed (+0x54, +0x58) which its
/// acceleration (+0x5c, +0x60) changes; ended once the screen is handed over (the director's flag
/// bit 3).
const SPEED: u32 = 0x54;
const ACCEL: u32 = 0x5c;
const POSITION: u32 = 0x14;
const HANDED_OVER: u32 = 8;
const DIRECTOR_FLAGS: u32 = 0x0604_0cb4;
const ANIM_STEP: u32 = 0x0605_73a0;

pub fn drift(cpu: &mut Cpu, actor: u32) {
    if nested!(cpu, DIRECTOR_FLAGS, super::director::director_flags(cpu)) & HANDED_OVER != 0 {
        cpu.write8(actor + OBJ_MODE, ENDING);
        return;
    }
    nested!(cpu, ANIM_STEP, anim_step(cpu, actor));
    for k in 0..2 {
        let v = cpu.read32(actor + SPEED + 4 * k);
        let p = cpu.read32(actor + POSITION + 4 * k);
        cpu.write32(actor + POSITION + 4 * k, p.wrapping_add(v));
    }
    for k in 0..2 {
        let a = cpu.read32(actor + ACCEL + 4 * k);
        let v = cpu.read32(actor + SPEED + 4 * k);
        cpu.write32(actor + SPEED + 4 * k, v.wrapping_add(a));
    }
}

/// A drifting effect's description (as its makers fill it, on their stack): its position (x, y,
/// z), its scales (x, y), its speed and direction (a word), its colour (a word: the low 13 bits
/// from the first character's; bit 15 a sprite attribute bit (0x100 of the flags), bit 13 flipped),
/// its animation script.
pub const E_POSITION: u32 = 0x00;
pub const E_SCALES: u32 = 0x0c;
pub const E_SPEED: u32 = 0x14;
pub const E_ANGLE: u32 = 0x18;
pub const E_COLOUR: u32 = 0x20;
pub const E_SCRIPT: u32 = 0x3c;
const E_ATTR: u16 = 0x8000;
const E_UNCULLED: u16 = 0x4000;
const E_FLIPPED: u16 = 0x2000;
const E_COLOUR_BITS: u16 = 0x1fff;
/// The drifting effect: its size, list (3), flags (never culled; an attribute bit), and lift.
const DRIFT_SIZE: u32 = 100;
const DRIFT_LIST: u32 = 3;
const DRIFT_FLAGS: u32 = 0x2000_1000;
const DRIFT_ATTR: u32 = 0x100;
const DRIFT_UNCULLED: u32 = 0x2000_0000;
const DRIFT_FLIP: u32 = 0x10;
const DRIFT_LIFT: u8 = 0xf8;
const DRIFT: u32 = 0x0605_73f0;
const PLAIN_EXEC: u32 = 0x0604_dcb4;
/// The sprite's fields from the actor: its kind, angle, scales.
const SPRITE_KIND: u32 = 0x2e;
const ANGLE: u32 = 0x30;
const SCALES: u32 = 0x20;
const LIFT: u32 = 0x33;
/// SGL's sine and cosine.
const SL_SIN: u32 = 0x0606_9a0c;
const SL_COS: u32 = 0x0606_9a30;

/// Makes a drifting effect from the description at `e`: it moves off in its direction, slowed by
/// a 64th of its speed a frame, while its animation runs. The effect, or 0 when there is no room.
pub fn drift_effect(cpu: &mut Cpu, e: u32) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, DRIFT_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, DRIFT_LIST, blocks, PLAIN_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + SPRITE));
    cpu.write32(o + BEHAVIOUR, DRIFT);
    cpu.write32(o + FLAGS, DRIFT_FLAGS);
    // the SH-2 code keeps the colour's address in its frame
    let frame = cpu.stack();
    cpu.write32(frame, e + E_COLOUR);
    if cpu.read16(e + E_COLOUR) & E_ATTR != 0 {
        cpu.write32(o + FLAGS, DRIFT_FLAGS | DRIFT_ATTR);
    }
    if cpu.read16(e + E_COLOUR) & E_UNCULLED != 0 {
        let f = cpu.read32(o + FLAGS) | DRIFT_UNCULLED;
        cpu.write32(o + FLAGS, f);
    }
    if cpu.read16(e + E_COLOUR) & E_FLIPPED != 0 {
        let f = cpu.read32(o + FLAGS) | DRIFT_FLIP;
        cpu.write32(o + FLAGS, f);
    }
    for k in 0..3 {
        let v = cpu.read32(e + E_POSITION + 4 * k);
        cpu.write32(o + POSITION + 4 * k, v);
    }
    let angle = cpu.read16(e + E_ANGLE);
    cpu.write16(o + ANGLE, angle);
    let angle = angle as i16 as i32 as u32;
    let speed = cpu.read32(e + E_SPEED);
    let s = cpu.call(SL_SIN, &[angle]);
    let dx = nested!(cpu, 0x0600_6a58, super::math::fixed_mul(cpu, s, speed));
    cpu.write32(o + SPEED, dx);
    let speed = cpu.read32(e + E_SPEED);
    let c = cpu.call(SL_COS, &[angle]);
    let dy = nested!(cpu, 0x0600_6a58, super::math::fixed_mul(cpu, c.wrapping_neg(), speed));
    cpu.write32(o + SPEED + 4, dy);
    for k in 0..2 {
        let v = (cpu.read32(o + SPEED + 4 * k) as i32).wrapping_neg();
        cpu.write32(o + ACCEL + 4 * k, (v / 64) as u32);
    }
    let script = cpu.read32(e + E_SCRIPT);
    cpu.write32(o + SCRIPT, script);
    let first = cpu.read8(script);
    cpu.write8(o + SPRITE_FRAME, first);
    let time = cpu.read8(script + 1);
    cpu.write8(o + SHOWN_LEFT, time);
    cpu.write32(o + SCRIPT, script + 2);
    cpu.write8(o + SPRITE_KIND, 0);
    let colour = (cpu.read16(e + E_COLOUR) & E_COLOUR_BITS).wrapping_add(cpu.read16(CHAR_BASE));
    cpu.write16(o + COLOUR, colour);
    for k in 0..2 {
        let v = cpu.read32(e + E_SCALES + 4 * k);
        cpu.write32(o + SCALES + 4 * k, v);
    }
    cpu.write8(o + LIFT, DRIFT_LIFT);
    o
}

/// The enemies' own fields (from +0x6c to the end of their 0xbc bytes) cleared.
pub fn enemy_clear(cpu: &mut Cpu, enemy: u32) {
    // the SH-2 code keeps the first fields' offsets in its frame
    let frame = cpu.stack();
    for (i, offset) in [0x80, 0x84, 0x88, 0x89, 0x8a, 0x8c, 0x8e, 0x90, 0x92, 0x94, 0x96, 0x98, 0x9a, 0x9c].into_iter().enumerate() {
        cpu.write32(frame + 4 * i as u32, offset);
    }
    cpu.write8(enemy + 0x7c, 0);
    for offset in [0x6c, 0x70, 0x74, 0x78, 0x80, 0x84, 0xac, 0xb0, 0xb4, 0xb8] {
        cpu.write32(enemy + offset, 0);
    }
    for offset in [0x88, 0x89] {
        cpu.write8(enemy + offset, 0);
    }
    for offset in (0x8a..=0xa8).step_by(2).chain([0x7e]) {
        cpu.write16(enemy + offset, 0);
    }
}

/// An actor's speed for move_by_angle (x, y).
const ANGLE_SPEED: u32 = 0x5c;

/// The actor's speed set to `speed` in the direction `angle` (16-bit; 0 straight up), and the
/// actor moved by it.
pub fn move_by_angle(cpu: &mut Cpu, actor: u32, speed: u32, angle: u32) {
    let angle = angle as i16 as i32 as u32;
    if angle == 0 {
        cpu.write32(actor + ANGLE_SPEED, 0);
        cpu.write32(actor + ANGLE_SPEED + 4, speed.wrapping_neg());
    } else {
        let s = cpu.call(SL_SIN, &[angle]);
        let dx = nested!(cpu, 0x0600_6a58, super::math::fixed_mul(cpu, speed, s));
        cpu.write32(actor + ANGLE_SPEED, dx);
        let c = cpu.call(SL_COS, &[angle]);
        let dy = nested!(cpu, 0x0600_6a58, super::math::fixed_mul(cpu, speed.wrapping_neg(), c));
        cpu.write32(actor + ANGLE_SPEED + 4, dy);
    }
    for k in 0..2 {
        let p = cpu.read32(actor + POSITION + 4 * k).wrapping_add(cpu.read32(actor + ANGLE_SPEED + 4 * k));
        cpu.write32(actor + POSITION + 4 * k, p);
    }
}

/// A trail (an actor that draws itself, with a box per piece): its boxes (24, from +0xb4), how many
/// pieces it draws and how many places apart (bytes), how many of its places are kept (to 128)
/// and the next one's index (words), and its last 128 places (x, y in pixels, and angles: words).
const TRAIL_BOXES: u32 = 0xb4;
const TRAIL_BOX_COUNT: u32 = 24;
pub const TRAIL_PIECES: u32 = 0x174;
pub const TRAIL_SPACING: u32 = 0x175;
pub const TRAIL_KEPT: u32 = 0x176;
pub const TRAIL_NEXT: u32 = 0x178;
pub const TRAIL_X: u32 = 0x17a;
pub const TRAIL_Y: u32 = 0x27a;
const TRAIL_ANGLE: u32 = 0x37a;
const TRAIL_PLACES: u32 = 0x80;
/// A trail's flags: a model drawn by its own function, its boxes not flipped.
const TRAIL_FLAGS: u32 = IS_MODEL | OWN_DRAW | 0x1_0000;

/// The actor set up as a trail: its pieces and their spacing, no places kept, and (with `boxes`)
/// each of its boxes that one box.
pub fn trail_init(cpu: &mut Cpu, actor: u32, pieces: u32, spacing: u32, boxes: u32) {
    let flags = cpu.read32(actor + FLAGS) | TRAIL_FLAGS;
    cpu.write32(actor + FLAGS, flags);
    cpu.write32(actor + 0x3c, actor + TRAIL_BOXES);
    cpu.write16(actor + TRAIL_KEPT, 0);
    cpu.write16(actor + TRAIL_NEXT, 0);
    cpu.write8(actor + TRAIL_PIECES, pieces as u8);
    cpu.write8(actor + TRAIL_SPACING, spacing as u8);
    if boxes == 0 {
        return;
    }
    for k in 0..TRAIL_BOX_COUNT {
        let dst = actor + TRAIL_BOXES + 8 * k;
        for i in [0, 1] {
            let v = cpu.read8(boxes + i);
            cpu.write8(dst + i, v);
        }
        for i in [2, 4] {
            let v = cpu.read16(boxes + i);
            cpu.write16(dst + i, v);
        }
        for i in [6, 7] {
            let v = cpu.read8(boxes + i);
            cpu.write8(dst + i, v);
        }
    }
}

/// The whole pixels of a 16.16 value, rounded toward zero.
fn pixels(v: u32) -> u16 {
    let v = v as i32;
    ((if v < 0 { v.wrapping_add(0xffff) } else { v }) >> 16) as u16
}

/// The trail's place this frame kept (its position and `angle`, which it takes).
pub fn trail_record(cpu: &mut Cpu, actor: u32, angle: u32) {
    let next = cpu.read16(actor + TRAIL_NEXT) as i16 as i32 as u32;
    let x = pixels(cpu.read32(actor + POSITION));
    cpu.write16(actor + TRAIL_X + 2 * next, x);
    let next = cpu.read16(actor + TRAIL_NEXT) as i16 as i32 as u32;
    let y = pixels(cpu.read32(actor + POSITION + 4));
    cpu.write16(actor + TRAIL_Y + 2 * next, y);
    let next = cpu.read16(actor + TRAIL_NEXT) as i16 as i32 as u32;
    cpu.write16(actor + TRAIL_ANGLE + 2 * next, angle as u16);
    cpu.write16(actor + ANGLE, angle as u16);
    let kept = cpu.read16(actor + TRAIL_KEPT);
    if kept as i16 <= 127 {
        cpu.write16(actor + TRAIL_KEPT, kept.wrapping_add(1));
    }
    let next = cpu.read16(actor + TRAIL_NEXT).wrapping_add(1);
    cpu.write16(actor + TRAIL_NEXT, next);
    let next = cpu.read16(actor + TRAIL_NEXT) as i16 as i32 as u32;
    let next = nested!(cpu, 0x0600_6a6e, super::math::remainder(cpu, next, TRAIL_PLACES));
    cpu.write16(actor + TRAIL_NEXT, next as u16);
}

/// An actor set up: its sprite, its hit fields, the word at +0x6c cleared.
pub fn actor_setup(cpu: &mut Cpu, actor: u32) {
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, actor + SPRITE));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, actor + 0x3c));
    cpu.write16(actor + 0x6c, 0);
}

/// A player's object: its size, its behaviour (player.rs), its boxes, its kind (2), the
/// kinds' table (the second kind's colour, a player 1 sixteen more), its gauge (hud.rs), a long at
/// +0xa8 (not known yet), and each player's hit counts (seven longs, hits.rs).
const PLAYER_SIZE: u32 = 0xac;
const PLAYER_BEHAVIOUR: u32 = 0x0604_7f60;
const PLAYER_BOXES: u32 = 0x0607_26f0;
const PLAYER_KIND: u8 = 2;
const PLAYER_COLOUR: u32 = 0x0607_421c;
const PLAYER_A8: u32 = 0x0410_0000;
const WEAPON_HITS: u32 = 0x0607_40a0;

/// Makes player `st+0x1e`'s object at `st` (x, y, z): it (in the numbered objects), or 0.
pub fn player_set(cpu: &mut Cpu, st: u32) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, PLAYER_SIZE)) as i8 as i32 as u32;
    let obj = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, 1, blocks, 0x0604_d600));
    if obj == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, obj + SPRITE));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, obj + 0x3c));
    cpu.write32(obj + BEHAVIOUR, PLAYER_BEHAVIOUR);
    for k in 0..3 {
        let v = cpu.read32(st + 4 * k);
        cpu.write32(obj + 0x14 + 4 * k, v);
    }
    cpu.write8(obj + 0x33, 4);
    let p = cpu.read8(st + 0x1e);
    cpu.write8(obj + NUMBER, p);
    cpu.write8(obj + 0x2e, PLAYER_KIND);
    cpu.write8(obj + 0x2f, 0);
    let mut colour = cpu.read16(PLAYER_COLOUR);
    cpu.write16(obj + COLOUR, colour);
    if cpu.read8(obj + NUMBER) == 1 {
        colour = colour.wrapping_add(16);
        cpu.write16(obj + COLOUR, colour);
    }
    let c = cpu.read16(obj + COLOUR);
    cpu.write16(obj + OWN_COLOUR, c);
    cpu.write8(obj + 0x54, 0);
    cpu.write32(obj + 0x50, 10000);
    cpu.write32(obj + 0x3c, PLAYER_BOXES);
    cpu.write32(obj + 0xa8, PLAYER_A8);
    let n = cpu.read8(obj + NUMBER) as i8 as i32;
    let hits = WEAPON_HITS.wrapping_add((28 * n) as u32);
    for k in 0..7 {
        cpu.write32(hits + 4 * k, 0);
    }
    cpu.write16(obj + 0x6c, 1);
    let n = cpu.read8(obj + NUMBER) as i8 as i32;
    cpu.write32(NUMBERED.wrapping_add((4 * n) as u32), obj);
    obj
}

/// `v` put first in the `n` bytes at `at`, the rest moved along one (the last dropped).
pub fn shift_in_bytes(cpu: &mut Cpu, at: u32, v: u32, n: u32) {
    let (mut v, n) = (v as u8, n as i16 as i32);
    for i in 0..n.max(0) as u32 {
        let old = cpu.read8(at + i);
        cpu.write8(at + i, v);
        v = old;
    }
}

/// `v` put first in the `n` words at `at`, the rest moved along one (the last dropped).
pub fn shift_in_words(cpu: &mut Cpu, at: u32, v: u32, n: u32) {
    let (mut v, n) = (v as u16, n as i16 as i32);
    for i in 0..n.max(0) as u32 {
        let old = cpu.read16(at + 2 * i);
        cpu.write16(at + 2 * i, v);
        v = old;
    }
}

/// The actor's frame stepped through 0x81-0x84 every other frame of the game, and it faces the way
/// `v` goes (mirrored unless it is positive).
const FLAP_FIRST: u8 = 0x81;
const FLAP_LAST: u8 = 0x84;
const FRAME: u32 = 0x2f;
const MIRRORED: u32 = 0x10;

pub fn flap_and_face(cpu: &mut Cpu, actor: u32, v: u32) {
    if cpu.read32(super::globals::GAME_FRAMES) & 1 == 0 {
        let f = cpu.read8(actor + FRAME).wrapping_add(1);
        cpu.write8(actor + FRAME, f);
        if f > FLAP_LAST {
            cpu.write8(actor + FRAME, FLAP_FIRST);
        }
    }
    let flags = cpu.read32(actor + FLAGS);
    let flags = if v as i32 > 0 { flags & !MIRRORED } else { flags | MIRRORED };
    cpu.write32(actor + FLAGS, flags);
}


/// A depth box: an unseen enemy part kept at the place at +0x74 (its depth 0xb0 nearer), hittable
/// only while that depth is within its range (pixels either way). While the word at +0x6c has
/// any of WATCH_OFF it cannot be hit; with WATCH_END it ends.
const DB_WATCHED: u32 = 0x6c;
const DB_AT: u32 = 0x74;
const DB_RANGE: u32 = 0x78;
const DB_NO_HITS: u32 = 0x8_0000;
const DB_NEARER: u32 = 0xff50_0000;
const WATCH_OFF: u16 = 0xe0;
const WATCH_END: u16 = 0x40;
const DB_POSITION: u32 = 0x14;

pub fn depth_box_exec(cpu: &mut Cpu, o: u32) {
    let watched = cpu.read32(o + DB_WATCHED);
    let w = cpu.read16(watched);
    let hittable = if w & WATCH_OFF == 0 {
        let at = cpu.read32(o + DB_AT);
        for k in 0..2 {
            let v = cpu.read32(at + 4 * k);
            cpu.write32(o + DB_POSITION + 4 * k, v);
        }
        let z = cpu.read32(at + 8).wrapping_add(DB_NEARER);
        cpu.write32(o + DB_POSITION + 8, z);
        let range = cpu.read8(o + DB_RANGE) as i8 as i32;
        let z = z as i32;
        z <= range << 16 && z > range.wrapping_neg() << 16
    } else if w & WATCH_END != 0 {
        cpu.write8(o + OBJ_MODE, ENDING);
        return;
    } else {
        false
    };
    let flags = cpu.read32(o + FLAGS);
    cpu.write32(o + FLAGS, if hittable { flags & !DB_NO_HITS } else { flags | DB_NO_HITS });
}

/// Makes a depth box of kind `kind` (its range from a table: 4, 8, 12 or 16 pixels take their own
/// boxes). It, or 0.
const DB_SIZE: u32 = 0x80;
const DB_LIST: u32 = 2;
const DB_EXEC: u32 = 0x0605_6f7c;
const DB_FLAGS: u32 = 0x8008_0000;
const DB_RANGES: u32 = 0x0607_2fa4;
const DB_BOXES: [(u8, u32); 4] = [(4, 0x0607_2fe8), (8, 0x0607_2ff0), (12, 0x0607_2ff8), (16, 0x0607_3000)];
const DB_SIDE: u32 = 0x54;
const DB_TYPE: u32 = 0x55;
const DB_BOX_LIST: u32 = 0x3c;

pub fn depth_box_set(cpu: &mut Cpu, kind: u32) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, DB_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, DB_LIST, blocks, 0x0604_d780));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + SPRITE));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + DB_BOX_LIST));
    cpu.write32(o + BEHAVIOUR, DB_EXEC);
    cpu.write32(o + FLAGS, DB_FLAGS);
    let range = cpu.read8(DB_RANGES + (kind & 0xffff));
    cpu.write8(o + DB_RANGE, range);
    cpu.write8(o + DB_SIDE, 1);
    cpu.write8(o + DB_TYPE, 5);
    let range = cpu.read8(o + DB_RANGE);
    if let Some(&(_, boxes)) = DB_BOXES.iter().find(|&&(r, _)| r == range) {
        cpu.write32(o + DB_BOX_LIST, boxes);
    }
    o
}

/// The bonus item (the numbered objects' last, one at a time): drifting with its speed against
/// the ground's movement; of kind 2 it lasts its frames. Shot (out of life) it gives its points
/// to the player who shot it (sound 67, scored as weapon 6), stops being hittable and flickers
/// for 120 frames, then shows its points (effects.rs's popup) and ends. Off screen it ends.
const BONUS: u32 = 0x0607_41ec;
const BN_SHOT: u32 = 0x7c;
const BN_KIND: u32 = 0x88;
const BN_LEFT: u32 = 0x8a;
const BN_SPEED: u32 = 0x80;
const BN_POINTS: u32 = 0xac;
const BN_FRAMES: u32 = 0xb0;
const BN_SHOOTER: u32 = 0xb8;
const BN_LASTS: u8 = 2;
const BN_LIFE: u32 = 0x50;
const BN_HIT_BY: u32 = 0x40;
const BN_NUMBER: u32 = 0x32;
const BN_FRAME: u32 = 0x2f;
const BN_FIRST_FRAME: u8 = 0xa3;
const SE_BONUS: u32 = 67;
const BONUS_WEAPON: u32 = 6;
const NO_DRAW: u32 = 0x8000_0000;
/// The popup (effects.rs's): its place, points, a word 1, what it shows over, the player.
const PU_POINTS: u32 = 0x14;
const PU_ONE: u32 = 0x1a;
const PU_PLAYER: u32 = 0x1e;
const PU_OVER: u32 = 0x3c;

pub fn bonus_exec(cpu: &mut Cpu, o: u32) {
    let f = cpu.stack();
    if cpu.read8(o + BN_KIND) == BN_LASTS && cpu.read8(o + BN_SHOT) == 0 {
        let left = cpu.read32(o + BN_FRAMES).wrapping_sub(1);
        cpu.write32(o + BN_FRAMES, left);
        if left as i32 <= 0 {
            cpu.write8(o + OBJ_MODE, ENDING);
            return;
        }
    }
    let shot = cpu.read8(o + BN_SHOT);
    if shot == 0 {
        if (cpu.read32(o + BN_LIFE) as i32) < 0 {
            cpu.write8(o + BN_SHOT, shot + 1);
            let flags = cpu.read32(o + FLAGS) & !NO_DRAW | DB_NO_HITS;
            cpu.write32(o + FLAGS, flags);
            nested!(cpu, 0x0600_50d4, super::sound::se_set(cpu, SE_BONUS));
            let by = cpu.read32(o + BN_HIT_BY);
            let n = cpu.read8(by + BN_NUMBER) as i8 as i32 as u32;
            cpu.write32(o + BN_SHOOTER, n);
            let by = cpu.read32(o + BN_HIT_BY);
            let n = cpu.read8(by + BN_NUMBER) as i8 as i32 as u32;
            let points = cpu.read32(o + BN_POINTS);
            nested!(cpu, 0x0604_3108, hits::score_kill(cpu, n, BONUS_WEAPON, o, points));
        } else if nested!(cpu, 0x0605_6a24, super::aim::off_screen(cpu, o + SPRITE)) as u8 != 0 {
            cpu.write8(o + OBJ_MODE, ENDING);
            return;
        }
    } else {
        let left = cpu.read16(o + BN_LEFT).wrapping_sub(1);
        cpu.write16(o + BN_LEFT, left);
        let flicker = ((cpu.read32(GAME_FRAMES_A) >> 1) & 3) as u8;
        cpu.write8(o + BN_FRAME, flicker.wrapping_add(BN_FIRST_FRAME));
        if (cpu.read16(o + BN_LEFT) as i16) < 0 {
            cpu.write8(o + OBJ_MODE, ENDING);
            for k in 0..2 {
                let v = cpu.read32(o + DB_POSITION + 4 * k);
                cpu.write32(f + 4 * k, v);
            }
            cpu.write32(f + 8, 0);
            let points = cpu.read32(o + BN_POINTS);
            cpu.write32(f + PU_POINTS, points);
            cpu.write16(f + PU_ONE, 1);
            cpu.write32(f + PU_OVER, o);
            let n = cpu.read8(o + BN_SHOOTER + 3);
            cpu.write8(f + PU_PLAYER, n);
            nested!(cpu, 0x0604_ea48, super::effects::popup(cpu, f));
        }
    }
    use super::globals::{GROUND_DX_AT, GROUND_DY_AT};
    for (k, at) in [(0, GROUND_DX_AT), (1, GROUND_DY_AT)] {
        let at = cpu.read32(at);
        let ground = cpu.read32(at);
        let d = cpu.read32(o + BN_SPEED + 4 * k).wrapping_sub(ground);
        let p = o + DB_POSITION + 4 * k;
        let v = cpu.read32(p).wrapping_add(d);
        cpu.write32(p, v);
    }
}

/// Makes the bonus item from the description at `d` (its kind (a byte at +1), place (pixel words
/// at +4, +6), points, speed (x, y), frames) unless there is one. 1.
const BN_SIZE: u32 = 0xbc;
const BN_EXEC: u32 = 0x0605_9a84;
const BN_NUMBERED: u8 = 63;
const BN_LIFT: u8 = 0x7e;
const BN_COLOURS: u32 = 0x0607_3274;
const BN_BOXES: u32 = 0x0607_326a;
const BN_TYPE: u8 = 6;
const BN_FLICKER: u16 = 120;
const GAME_FRAMES_A: u32 = super::globals::GAME_FRAMES;

pub fn bonus_set(cpu: &mut Cpu, d: u32) -> u32 {
    if cpu.read32(BONUS) != 0 {
        return 1;
    }
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, BN_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, DB_LIST, blocks, 0x0604_d600));
    if o == 0 {
        return 1;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + SPRITE));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + DB_BOX_LIST));
    nested!(cpu, 0x0605_681c, enemy_clear(cpu, o));
    cpu.write32(o + BEHAVIOUR, BN_EXEC);
    cpu.write32(o + FLAGS, NO_DRAW);
    cpu.write8(o + BN_NUMBER, BN_NUMBERED);
    cpu.write32(BONUS, o);
    for k in 0..2 {
        let v = (cpu.read16(d + 4 + 2 * k) as i16 as i32) << 16;
        cpu.write32(o + DB_POSITION + 4 * k, v as u32);
    }
    cpu.write32(o + DB_POSITION + 8, 0);
    cpu.write8(o + 0x33, BN_LIFT);
    let kind = cpu.read8(d + 1);
    cpu.write8(o + BN_KIND, kind);
    cpu.write8(o + 0x2e, 0);
    cpu.write8(o + BN_FRAME, BN_FIRST_FRAME);
    let kind = cpu.read8(o + BN_KIND) as i8 as i32 as u32;
    let colour = cpu.read16(CHAR_BASE).wrapping_add(cpu.read8(BN_COLOURS.wrapping_add(kind)) as u16);
    cpu.write16(o + 0x2a, colour);
    cpu.write16(o + 0x28, colour);
    cpu.write32(o + BN_LIFE, 1);
    cpu.write32(o + DB_BOX_LIST, BN_BOXES);
    cpu.write8(o + DB_SIDE, 1);
    cpu.write8(o + DB_TYPE, BN_TYPE);
    cpu.write16(o + BN_LEFT, BN_FLICKER);
    for (field, from) in [(BN_SPEED, 0x14), (BN_SPEED + 4, 0x18), (BN_POINTS, 0x10), (BN_FRAMES, 0x1c)] {
        let v = cpu.read32(d + from);
        cpu.write32(o + field, v);
    }
    1
}

/// `n` bytes from `at` set to `v` (the count a byte, as the SH-2 code keeps it).
pub fn fill_bytes(cpu: &mut Cpu, at: u32, v: u32, n: u32) {
    let n = n as i16 as i32;
    let mut i: u8 = 0;
    while (i as i32) < n {
        cpu.write8(at + i as u32, v as u8);
        i = i.wrapping_add(1);
    }
}

/// A script started at `s` (a stage's: the script at +0x20, its three counts cleared).
pub fn script_start(cpu: &mut Cpu, s: u32, script: u32) {
    cpu.write32(s + 0x20, script);
    for k in 0..3 {
        cpu.write16(s + 0x24 + 2 * k, 0);
    }
}
