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

/// A trail (an actor that draws itself, with a box per piece): its boxes (24, from +0xb4), two
/// bytes of its own, how many of its places are kept (to 128) and the next one's index (words),
/// and its last 128 places (x, y in pixels, and angles: words).
const TRAIL_BOXES: u32 = 0xb4;
const TRAIL_BOX_COUNT: u32 = 24;
const TRAIL_A: u32 = 0x174;
const TRAIL_B: u32 = 0x175;
const TRAIL_KEPT: u32 = 0x176;
const TRAIL_NEXT: u32 = 0x178;
const TRAIL_X: u32 = 0x17a;
const TRAIL_Y: u32 = 0x27a;
const TRAIL_ANGLE: u32 = 0x37a;
const TRAIL_PLACES: u32 = 0x80;
/// A trail's flags: a model drawn by its own function, its boxes not flipped.
const TRAIL_FLAGS: u32 = IS_MODEL | OWN_DRAW | 0x1_0000;

/// The actor set up as a trail: its two bytes, no places kept, and (with `boxes`) each of its
/// boxes that one box.
pub fn trail_init(cpu: &mut Cpu, actor: u32, a: u32, b: u32, boxes: u32) {
    let flags = cpu.read32(actor + FLAGS) | TRAIL_FLAGS;
    cpu.write32(actor + FLAGS, flags);
    cpu.write32(actor + 0x3c, actor + TRAIL_BOXES);
    cpu.write16(actor + TRAIL_KEPT, 0);
    cpu.write16(actor + TRAIL_NEXT, 0);
    cpu.write8(actor + TRAIL_A, a as u8);
    cpu.write8(actor + TRAIL_B, b as u8);
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

/// A player's object: its size, its behaviour (not rewritten yet), its boxes, its kind (2), the
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
