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
