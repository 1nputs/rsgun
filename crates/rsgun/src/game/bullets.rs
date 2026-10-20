//! The enemies' bullets: actors (enemy structs, list 2) made from a shot's description, each
//! moving in a line or swirling around a centre that moves, growing in over its first frames,
//! then cycling its frames; ended off screen, or with a spark where it hits the ground.

use super::actors::{E_ANGLE, E_COLOUR, E_POSITION, E_SCALES, E_SCRIPT, E_SPEED};
use super::{actors, aim, bg, hits, math, nested, objects, sprites, Cpu};

/// The objects' mode byte (ending).
const OBJ_MODE: u32 = 3;
const ENDING: u8 = 0x70;
/// An actor's sprite (from its flags), position, scales, colours, frame, lift, angle, boxes,
/// behaviour, side and type (hits.rs's), hit flags, life.
const SPRITE: u32 = 0x10;
const FLAGS: u32 = 0x10;
const POSITION: u32 = 0x14;
const SCALES: u32 = 0x20;
const COLOUR: u32 = 0x28;
const OWN_COLOUR: u32 = 0x2a;
const KIND: u32 = 0x2e;
const FRAME: u32 = 0x2f;
const NUMBER: u32 = 0x32;
const LIFT: u32 = 0x33;
const ANGLE: u32 = 0x30;
const BOXES: u32 = 0x3c;
const BEHAVIOUR: u32 = 0x38;
const LIFE: u32 = 0x50;
const SIDE: u32 = 0x54;
const TYPE: u32 = 0x55;
const HIT_FLAGS: u32 = 0x56;
/// The hit flags: was hit, hit something (here also: hit the ground).
const WAS_HIT: u16 = 0x200;
const HIT_SOMETHING: u16 = 0x400;

/// A bullet's own fields: its stage (growing in, then cycling its frames), its speed (x, y; and as
/// it started), its shot flags, and a swirl's angle, turn, radius and the radius it goes to
/// (words) and its centre (x, y).
const STAGE: u32 = 0x7c;
const SPEED: u32 = 0x80;
const START_SPEED: u32 = 0x5c;
const SHOT_FLAGS: u32 = 0x8a;
const SWIRL_ANGLE: u32 = 0x8c;
const SWIRL_TURN: u32 = 0x8e;
const SWIRL_RADIUS: u32 = 0x90;
const SWIRL_RADIUS_TO: u32 = 0x92;
const SWIRL_CENTRE: u32 = 0xac;
/// The shot flags: swirling, flickering in depth.
const SWIRLS: u16 = 1;
const FLICKERS: u16 = 2;

/// The first character's colour (the colours are from it).
const CHAR_BASE: u32 = 0x0607_41fc;
/// The game's frames (globals.rs's).
const GAME_FRAMES: u32 = super::globals::GAME_FRAMES;

/// The sparks' animations: where a bullet hits the ground (turned back from its direction), and
/// where one is taken out otherwise (any direction).
const SPARK_BACK: u32 = 0x0607_30b0;
const SPARK_ANY: u32 = 0x0607_3174;
const DRIFT_EFFECT: u32 = 0x0605_7454;

/// SGL's sine and cosine.
const SL_SIN: u32 = 0x0606_9a0c;
const SL_COS: u32 = 0x0606_9a30;

/// Whether the bullet `b` ends: off screen; or (unless it hit something) where it hits the ground,
/// with a spark (`back` 1: turned back from its direction, else any) of its colour. 1 when it
/// ends, else 0.
pub fn bullet_ends(cpu: &mut Cpu, b: u32, back: u32) -> u32 {
    let back = back as u8;
    if nested!(cpu, 0x0605_6a24, aim::off_screen(cpu, b + SPRITE)) as u8 != 0 {
        cpu.write8(b + OBJ_MODE, ENDING);
        return 1;
    }
    if cpu.read16(b + HIT_FLAGS) & HIT_SOMETHING == 0 {
        let (x, y) = (cpu.read32(b + POSITION), cpu.read32(b + POSITION + 4));
        if nested!(cpu, 0x0605_ef90, bg::map_hit(cpu, x, y)) == 0 {
            return 0;
        }
    }
    cpu.write8(b + OBJ_MODE, ENDING);
    let e = cpu.stack();
    for k in 0..3 {
        let v = cpu.read32(b + POSITION + 4 * k);
        cpu.write32(e + E_POSITION + 4 * k, v);
    }
    let colour = cpu.read16(b + COLOUR).wrapping_sub(cpu.read16(CHAR_BASE));
    cpu.write16(e + E_COLOUR, colour);
    let scale = if back == 1 {
        cpu.write32(e + E_SCRIPT, SPARK_BACK);
        let angle = cpu.read16(b + ANGLE).wrapping_add(0x8000);
        cpu.write16(e + E_ANGLE, angle);
        cpu.write32(e + E_SPEED, 0xffff_0000);
        0xc000
    } else {
        cpu.write32(e + E_SCRIPT, SPARK_ANY);
        let angle = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x10000));
        cpu.write16(e + E_ANGLE, angle as u16);
        cpu.write32(e + E_SPEED, 0x8000);
        0x4000
    };
    cpu.write32(e + E_SCALES, scale);
    cpu.write32(e + E_SCALES + 4, scale);
    nested!(cpu, DRIFT_EFFECT, actors::drift_effect(cpu, e));
    1
}

/// The growing-in scale, and the frames cycled (one each third frame).
const GROWING: u32 = 0x3000;
const FULL: u32 = 0x10000;
const FIRST_FRAME: u8 = 0x6c;
const LAST_FRAME: u8 = 0x6f;
/// The lifts a flickering bullet takes by turns.
const LIFT_LOW: u8 = 8;
const LIFT_HIGH: u8 = 8 + 118;

/// A bullet's frame after its move: unless it ended, its stage (a frame as it is, a frame grown in
/// a little, then whole on its first frame, then the frames cycled), and its lift by turns if it
/// flickers.
pub fn bullet_frame(cpu: &mut Cpu, b: u32) {
    if nested!(cpu, 0x0605_3b10, bullet_ends(cpu, b, 0)) as u8 != 0 {
        return;
    }
    match cpu.read8(b + STAGE) {
        0 => cpu.write8(b + STAGE, 1),
        1 => {
            cpu.write8(b + STAGE, 2);
            cpu.write32(b + SCALES, GROWING);
            cpu.write32(b + SCALES + 4, GROWING);
        }
        2 => {
            cpu.write8(b + STAGE, 3);
            cpu.write8(b + FRAME, FIRST_FRAME);
            cpu.write32(b + SCALES, FULL);
            cpu.write32(b + SCALES + 4, FULL);
        }
        3 => {
            let frames = cpu.read32(GAME_FRAMES);
            if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, frames, 3)) == 0 {
                let frame = cpu.read8(b + FRAME).wrapping_add(1);
                cpu.write8(b + FRAME, frame);
                if frame > LAST_FRAME {
                    cpu.write8(b + FRAME, FIRST_FRAME);
                }
            }
        }
        _ => {}
    }
    if cpu.read16(b + SHOT_FLAGS) & FLICKERS != 0 {
        let lift = if cpu.read32(GAME_FRAMES) & 1 != 0 { LIFT_HIGH } else { LIFT_LOW };
        cpu.write8(b + LIFT, lift);
    }
}

/// A bullet's behaviour in a line: moved by its speed.
pub fn bullet_move(cpu: &mut Cpu, b: u32) {
    for k in 0..2 {
        let p = cpu.read32(b + POSITION + 4 * k).wrapping_add(cpu.read32(b + SPEED + 4 * k));
        cpu.write32(b + POSITION + 4 * k, p);
    }
    nested!(cpu, BULLET_FRAME, bullet_frame(cpu, b));
}
const BULLET_FRAME: u32 = 0x0605_3bfc;

/// A swirling bullet's behaviour: its centre moved by its speed, the bullet at its radius from the
/// centre at its angle; the angle turned, the radius brought toward its target by 2.
pub fn bullet_swirl(cpu: &mut Cpu, b: u32) {
    for k in 0..2 {
        let c = cpu.read32(b + SWIRL_CENTRE + 4 * k).wrapping_add(cpu.read32(b + SPEED + 4 * k));
        cpu.write32(b + SWIRL_CENTRE + 4 * k, c);
    }
    let angle = cpu.read16(b + SWIRL_ANGLE) as i16 as i32 as u32;
    let s = cpu.call(SL_SIN, &[angle]);
    let radius = cpu.read16(b + SWIRL_RADIUS) as i16 as i32 as u32;
    let x = cpu.read32(b + SWIRL_CENTRE).wrapping_add(s.wrapping_mul(radius));
    cpu.write32(b + POSITION, x);
    let angle = cpu.read16(b + SWIRL_ANGLE) as i16 as i32 as u32;
    let c = cpu.call(SL_COS, &[angle]);
    let radius = cpu.read16(b + SWIRL_RADIUS) as i16 as i32 as u32;
    let y = cpu.read32(b + SWIRL_CENTRE + 4).wrapping_add(c.wrapping_neg().wrapping_mul(radius));
    cpu.write32(b + POSITION + 4, y);
    let angle = cpu.read16(b + SWIRL_ANGLE).wrapping_add(cpu.read16(b + SWIRL_TURN));
    cpu.write16(b + SWIRL_ANGLE, angle);
    let (radius, to) = (cpu.read16(b + SWIRL_RADIUS) as i16 as i32 as u32, cpu.read16(b + SWIRL_RADIUS_TO) as i16 as i32 as u32);
    let radius = nested!(cpu, 0x0604_6168, actors::approach(cpu, radius, to, 2));
    cpu.write16(b + SWIRL_RADIUS, radius as u16);
    nested!(cpu, BULLET_FRAME, bullet_frame(cpu, b));
}

/// A shot's description (the shooter's): its position (x, y, z), its number (a byte, the bullet's),
/// its direction (a word), its shot flags (a word), its speed, and a swirl's start angle, turn and
/// radius to go to (words).
const S_POSITION: u32 = 0x00;
const S_NUMBER: u32 = 0x1e;
const S_ANGLE: u32 = 0x2c;
const S_FLAGS: u32 = 0x2e;
const S_SPEED: u32 = 0x30;
const S_SWIRL_ANGLE: u32 = 0x36;
const S_SWIRL_TURN: u32 = 0x38;
const S_SWIRL_RADIUS_TO: u32 = 0x3a;
/// A bullet: its size, list (2), flags (never culled; ...), lift, first frame, scales, colour (from
/// the first character's), boxes, side and type.
const BULLET_SIZE: u32 = 0xbc;
const BULLET_LIST: u32 = 2;
const BULLET_FLAGS: u32 = 0x2000_0410;
const BULLET_LIFT: u8 = 8;
const BULLET_FRAME_0: u8 = 0xd6;
const BULLET_SCALE: u32 = 0x4800;
const BULLET_COLOUR: u16 = 0xa0;
const BULLET_BOXES: u32 = 0x0607_2ee4;
const BULLET_SIDE: u8 = 1;
const BULLET_TYPE: u8 = 5;
const ACTOR_EXEC: u32 = 0x0604_d780;
const BULLET_MOVE: u32 = 0x0605_3ccc;
const BULLET_SWIRL: u32 = 0x0605_3cfc;

/// Makes a bullet from the shot `shot`: the bullet, or 0 when there is no room.
pub fn bullet_set(cpu: &mut Cpu, shot: u32) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, BULLET_SIZE)) as i8 as i32 as u32;
    let b = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, BULLET_LIST, blocks, ACTOR_EXEC));
    if b == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, b + SPRITE));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, b + BOXES));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, b));
    let number = cpu.read8(shot + S_NUMBER);
    cpu.write8(b + NUMBER, number);
    cpu.write32(b + FLAGS, BULLET_FLAGS);
    for k in 0..3 {
        let v = cpu.read32(shot + S_POSITION + 4 * k);
        cpu.write32(b + POSITION + 4 * k, v);
    }
    cpu.write8(b + LIFT, BULLET_LIFT);
    cpu.write8(b + FRAME, BULLET_FRAME_0);
    cpu.write32(b + SCALES, BULLET_SCALE);
    cpu.write32(b + SCALES + 4, BULLET_SCALE);
    cpu.write8(b + KIND, 0);
    let colour = cpu.read16(CHAR_BASE).wrapping_add(BULLET_COLOUR);
    cpu.write16(b + OWN_COLOUR, colour);
    cpu.write16(b + COLOUR, colour);
    cpu.write32(b + BOXES, BULLET_BOXES);
    cpu.write8(b + SIDE, BULLET_SIDE);
    cpu.write8(b + TYPE, BULLET_TYPE);
    let angle = cpu.read16(shot + S_ANGLE) as i16 as i32 as u32;
    let speed = cpu.read32(shot + S_SPEED);
    let s = cpu.call(SL_SIN, &[angle]);
    let dx = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, speed));
    cpu.write32(b + SPEED, dx);
    let speed = cpu.read32(shot + S_SPEED);
    let angle = cpu.read16(shot + S_ANGLE) as i16 as i32 as u32;
    let c = cpu.call(SL_COS, &[angle]);
    let dy = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, c.wrapping_neg(), speed));
    cpu.write32(b + SPEED + 4, dy);
    for k in 0..2 {
        let v = cpu.read32(b + SPEED + 4 * k);
        cpu.write32(b + START_SPEED + 4 * k, v);
    }
    let flags = cpu.read16(shot + S_FLAGS);
    cpu.write16(b + SHOT_FLAGS, flags);
    if flags & SWIRLS != 0 {
        cpu.write32(b + BEHAVIOUR, BULLET_SWIRL);
        for (from, to) in [(S_SWIRL_ANGLE, SWIRL_ANGLE), (S_SWIRL_TURN, SWIRL_TURN), (S_SWIRL_RADIUS_TO, SWIRL_RADIUS_TO)] {
            let v = cpu.read16(shot + from);
            cpu.write16(b + to, v);
        }
        for k in 0..2 {
            let v = cpu.read32(b + POSITION + 4 * k);
            cpu.write32(b + SWIRL_CENTRE + 4 * k, v);
        }
    } else {
        cpu.write32(b + BEHAVIOUR, BULLET_MOVE);
    }
    b
}

/// A plain bullet: the shot's flags none and its speed 1.25 (written into the shot), made.
pub fn bullet_set_plain(cpu: &mut Cpu, shot: u32) -> u32 {
    cpu.write16(shot + S_FLAGS, 0);
    cpu.write32(shot + S_SPEED, 0x1_4000);
    nested!(cpu, 0x0605_3db0, bullet_set(cpu, shot))
}

/// The spark of a shootable bullet taken out (turned back from its direction).
const TAKEN_OUT_COLOUR: u16 = 0x90;

/// A shootable bullet's behaviour: it hits the ground like something hit; moved by its speed
/// while it has hit nothing, is on screen and has life; else ended (with a spark if it was hit or
/// hit something).
pub fn shootable_bullet(cpu: &mut Cpu, b: u32) {
    let (x, y) = (cpu.read32(b + POSITION), cpu.read32(b + POSITION + 4));
    if nested!(cpu, 0x0605_ef90, bg::map_hit(cpu, x, y)) != 0 {
        cpu.write16(b + HIT_FLAGS, HIT_SOMETHING);
    }
    let goes_on = cpu.read16(b + HIT_FLAGS) & HIT_SOMETHING == 0
        && nested!(cpu, 0x0605_6a24, aim::off_screen(cpu, b + SPRITE)) as u8 == 0
        && cpu.read32(b + LIFE) as i32 > 0;
    if goes_on {
        for k in 0..2 {
            let p = cpu.read32(b + POSITION + 4 * k).wrapping_add(cpu.read32(b + SPEED + 4 * k));
            cpu.write32(b + POSITION + 4 * k, p);
        }
        return;
    }
    if cpu.read16(b + HIT_FLAGS) & (WAS_HIT | HIT_SOMETHING) != 0 {
        let e = cpu.stack();
        cpu.write32(e + E_SCRIPT, SPARK_BACK);
        cpu.write16(e + E_COLOUR, TAKEN_OUT_COLOUR);
        for k in 0..3 {
            let v = cpu.read32(b + POSITION + 4 * k);
            cpu.write32(e + E_POSITION + 4 * k, v);
        }
        let angle = cpu.read16(b + ANGLE).wrapping_add(0x8000);
        cpu.write16(e + E_ANGLE, angle);
        cpu.write32(e + E_SPEED, 0xffff_0000);
        cpu.write32(e + E_SCALES, 0x10000);
        cpu.write32(e + E_SCALES + 4, 0x8000);
        nested!(cpu, DRIFT_EFFECT, actors::drift_effect(cpu, e));
    }
    cpu.write8(b + OBJ_MODE, ENDING);
}
