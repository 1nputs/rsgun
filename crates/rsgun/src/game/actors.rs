//! The actors: the objects that play (the players, their shots, the enemies and their bullets ...):
//! each a Base_obj (objects.rs) with a sprite or model at +0x10 (sprites.rs), hit boxes
//! (hits.rs) and a behaviour. Each frame an actor runs its behaviour, flashes while hit, registers
//! its boxes and is drawn.

use super::nested;
use super::globals::{GAME_FLAGS, HELD};
use super::{hits, objects, Cpu};

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
/// The draw (an actor's sprite or model, by its flags; not rewritten yet beyond sprites.rs's).
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

/// An actor's frame (its exec): while the game is not held, its behaviour (unless it ends by the
/// flags), the flash, its boxes registered; then its colour or model for the flash, and drawn.
pub fn actor_exec(cpu: &mut Cpu, actor: u32) {
    match cpu.read8(actor + OBJ_MODE) {
        1 => {}
        0 => {
            if cpu.read32(GAME_FLAGS) & HELD != 0 {
                return;
            }
            cpu.write8(actor + OBJ_MODE, 1);
        }
        ENDING => {
            cpu.write8(actor + OBJ_MODE, ENDED);
            return;
        }
        ENDED => {
            objects::obj_free(cpu, actor);
            return;
        }
        _ => return,
    }
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
    cpu.call(DRAW, &[actor + SPRITE]);
}
