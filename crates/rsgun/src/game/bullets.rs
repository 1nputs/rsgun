//! The enemies' bullets: actors (enemy structs, list 2) made from a shot's description, each
//! moving in a line or swirling around a centre that moves, growing in over its first frames,
//! then cycling its frames; ended off screen, or with a spark where it hits the ground.

use super::actors::{E_ANGLE, E_COLOUR, E_POSITION, E_SCALES, E_SCRIPT, E_SPEED};
use super::{actors, aim, bg, effects, hits, math, nested, objects, sprites, Cpu};

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
pub const S_POSITION: u32 = 0x00;
pub const S_NUMBER: u32 = 0x1e;
pub const S_ANGLE: u32 = 0x2c;
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

/// An aimed bullet's stage (a word; its low byte read), its link (effects.rs's) and the
/// animation it plays, its speed (move_by_angle's).
const AIMED_STAGE: u32 = 0x6c;
const AIMED_STAGE_LOW: u32 = 0x6d;
const AIMED_LINK: u32 = 0x80;
const AIMED_ANIMATION: u32 = 0x0607_2bb0;
const ANGLE_SPEED: u32 = 0x5c;
/// Off the screen's edges (x, y beyond these from its centre) it ends.
const EDGE_X: i32 = 0xb3_ffff;
const EDGE_Y: i32 = 0x73_ffff;
/// The main loop's passes (globals.rs's): its lift flickers by them.
const LOOPS: u32 = super::globals::LOOPS;

/// An aimed bullet's behaviour: ended where it hit something or the ground; grown in over its
/// first frames (a frame small, one larger, then whole with its animation started), then moved
/// by its speed with its animation played and its lift flickering; ended off screen.
pub fn aimed_exec(cpu: &mut Cpu, b: u32) {
    let hit = cpu.read16(b + HIT_FLAGS) & HIT_SOMETHING != 0 || {
        let (x, y) = (cpu.read32(b + POSITION), cpu.read32(b + POSITION + 4));
        nested!(cpu, 0x0605_ef90, bg::map_hit(cpu, x, y)) != 0
    };
    if hit {
        cpu.write8(b + OBJ_MODE, ENDING);
    }
    let stage = cpu.read8(b + AIMED_STAGE_LOW);
    match stage {
        0 => {
            cpu.write8(b + FRAME, BULLET_FRAME_0);
            cpu.write32(b + SCALES + 4, BULLET_SCALE);
            cpu.write32(b + SCALES, BULLET_SCALE);
            cpu.write16(b + AIMED_STAGE, 1);
        }
        1 => {
            cpu.write32(b + SCALES + 4, GROWING);
            cpu.write32(b + SCALES, GROWING);
            cpu.write16(b + AIMED_STAGE, 2);
        }
        2 | 3 => {
            if stage == 2 {
                cpu.write32(b + SCALES + 4, FULL);
                cpu.write32(b + SCALES, FULL);
                nested!(cpu, 0x0604_6530, effects::link_init(cpu, b + AIMED_LINK, AIMED_ANIMATION));
                cpu.write16(b + AIMED_STAGE, 3);
            }
            for k in 0..2 {
                let p = cpu.read32(b + POSITION + 4 * k).wrapping_add(cpu.read32(b + ANGLE_SPEED + 4 * k));
                cpu.write32(b + POSITION + 4 * k, p);
            }
            nested!(cpu, 0x0604_6558, effects::link_step(cpu, b + SPRITE, b + AIMED_LINK));
            let lift = if cpu.read32(LOOPS) & 1 != 0 { LIFT_HIGH } else { LIFT_LOW };
            cpu.write8(b + LIFT, lift);
        }
        _ => {}
    }
    let (x, y) = (cpu.read32(b + POSITION) as i32, cpu.read32(b + POSITION + 4) as i32);
    if x.wrapping_abs() > EDGE_X || y.wrapping_abs() > EDGE_Y {
        cpu.write8(b + OBJ_MODE, ENDING);
    }
}

/// An aimed shot's description (the shooter's): its position (x, y), its flags (a long: bit 0
/// aims at the player), its direction (a word), its speed, the shooter.
pub const AS_POSITION: u32 = 0x00;
pub const AS_FLAGS: u32 = 0x14;
pub const AS_ANGLE: u32 = 0x18;
pub const AS_SPEED: u32 = 0x30;
pub const AS_SHOOTER: u32 = 0x3c;
const AIMS: u32 = 1;
/// How far back the aim looks (aim.rs's delay).
const AIM_DELAY: u32 = 20;
/// An aimed bullet's size, flags, boxes.
const AIMED_SIZE: u32 = 0xb4;
const AIMED_FLAGS: u32 = 0x3000_0400;
const AIMED_BOXES: u32 = 0x0607_2ba8;
const AIMED_EXEC: u32 = 0x0604_daa8;

/// Makes an aimed bullet from the shot `shot` (aimed at the player it picks if the shot says so,
/// the direction then kept in the shot): the bullet, or 0 when there is no room.
pub fn aimed_set(cpu: &mut Cpu, shot: u32) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, AIMED_SIZE)) as i8 as i32 as u32;
    let b = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, BULLET_LIST, blocks, ACTOR_EXEC));
    if b == 0 {
        return 0;
    }
    let shooter = cpu.read32(shot + AS_SHOOTER);
    nested!(cpu, 0x0604_62ac, actors::actor_setup(cpu, b));
    let flags = cpu.read32(b + FLAGS) | AIMED_FLAGS;
    cpu.write32(b + FLAGS, flags);
    cpu.write32(b + BEHAVIOUR, AIMED_EXEC);
    cpu.write8(b + LIFT, BULLET_LIFT);
    let number = cpu.read8(shooter + NUMBER);
    cpu.write8(b + NUMBER, number);
    cpu.write8(b + KIND, 0);
    let colour = cpu.read16(CHAR_BASE).wrapping_add(BULLET_COLOUR);
    cpu.write16(b + OWN_COLOUR, colour);
    cpu.write16(b + COLOUR, colour);
    cpu.write32(b + BOXES, AIMED_BOXES);
    let side = cpu.read8(shooter + SIDE);
    cpu.write8(b + SIDE, side);
    cpu.write8(b + TYPE, BULLET_TYPE);
    if cpu.read32(shot + AS_FLAGS) & AIMS != 0 {
        let angle = nested!(cpu, 0x0604_0df0, aim::aim(cpu, b, AIM_DELAY));
        cpu.write16(shot + AS_ANGLE, angle as u16);
    }
    let angle = cpu.read16(shot + AS_ANGLE) as i16 as i32 as u32;
    let speed = cpu.read32(shot + AS_SPEED);
    nested!(cpu, 0x0604_d8d8, actors::move_by_angle(cpu, b, speed, angle));
    for k in 0..2 {
        let v = cpu.read32(shot + AS_POSITION + 4 * k);
        cpu.write32(b + POSITION + 4 * k, v);
    }
    cpu.write32(b + POSITION + 8, 0);
    b
}

/// The accelerating bullets' fields: the frames left accelerating (a word), the acceleration (x, y
/// words).
const ACCEL_FRAMES: u32 = 0x8c;
const ACCEL: u32 = 0x8e;

/// A sixth of a pixel a frame along `angle` (x, y).
fn sixth_along(cpu: &mut Cpu, b: u32, angle: u32) {
    let s = cpu.call(SL_SIN, &[angle]) as i32;
    cpu.write16(b + ACCEL, (s / 6) as u16);
    let c = cpu.call(SL_COS, &[angle]) as i32;
    cpu.write16(b + ACCEL + 2, (c.wrapping_neg() / 6) as u16);
}

/// The bullet sped up (while it still accelerates) and moved by its speed.
fn accelerate(cpu: &mut Cpu, b: u32) {
    let n = cpu.read16(b + ACCEL_FRAMES) as i16;
    if n > 0 {
        cpu.write16(b + ACCEL_FRAMES, (n - 1) as u16);
        for k in 0..2 {
            let v = cpu.read32(b + SPEED + 4 * k).wrapping_add(cpu.read16(b + ACCEL + 2 * k) as i16 as i32 as u32);
            cpu.write32(b + SPEED + 4 * k, v);
        }
    }
    for k in 0..2 {
        let p = cpu.read32(b + POSITION + 4 * k).wrapping_add(cpu.read32(b + SPEED + 4 * k));
        cpu.write32(b + POSITION + 4 * k, p);
    }
}

/// A bullet set up from shot `shot` (its sprite and hits, its enemy fields cleared; its number,
/// flags, place, lift 8, `frame`, kind 0, `boxes`, side and type, direction): it, or 0.
fn bullet_in(cpu: &mut Cpu, shot: u32, exec: u32, frame: u8, boxes: u32) -> u32 {
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
    cpu.write32(b + BEHAVIOUR, exec);
    cpu.write32(b + FLAGS, ACCEL_FLAGS);
    for k in 0..3 {
        let v = cpu.read32(shot + S_POSITION + 4 * k);
        cpu.write32(b + POSITION + 4 * k, v);
    }
    cpu.write8(b + LIFT, BULLET_LIFT);
    cpu.write8(b + FRAME, frame);
    cpu.write8(b + KIND, 0);
    cpu.write32(b + BOXES, boxes);
    cpu.write8(b + SIDE, BULLET_SIDE);
    cpu.write8(b + TYPE, BULLET_TYPE);
    let angle = cpu.read16(shot + S_ANGLE);
    cpu.write16(b + ANGLE, angle);
    b
}

/// The accelerating bullets' flags: never culled, ....
const ACCEL_FLAGS: u32 = 0x2000_0400;

/// A pulsing bullet (frames 91-93 in turn, its size flickering from 0xc000 to 0xd000, its two
/// colours in turn each frame: the first character's plus 0 and 0x90, or with shot flag 0x10 plus
/// 0x20 and 0x30): it speeds up along its direction (a sixth of a pixel a frame) for as many frames
/// as the shot flags' low 4 bits say; its lift flickers while its frame is not the first (shot flag
/// 0x20 starts it on the second); ended off screen or with a spark where it hits the ground.
const PULSE_EXEC: u32 = 0x0605_4b9c;
const PULSE_BOXES: u32 = 0x0607_2f1c;
const PULSE_FRAME: u8 = 91;
const PULSE_SIZE: u32 = 0xc000;
const PULSE_FLICKER: u32 = 0x1000;
const PULSE_SCALE: u32 = 0xa000;
const PULSE_SECOND: u16 = 0x20;
const PULSE_BRIGHT: u16 = 0x10;
const PULSE_ACCEL_FRAMES: u16 = 0xf;
/// The pulsing bullet's frame index (a byte) and its two colours (words).
const PULSE_INDEX: u32 = 0x88;
const PULSE_COLOURS: u32 = 0xa6;

pub fn pulse_exec(cpu: &mut Cpu, b: u32) {
    if nested!(cpu, 0x0605_3b10, bullet_ends(cpu, b, 0)) as u8 != 0 {
        return;
    }
    accelerate(cpu, b);
    let i = (cpu.read8(b + PULSE_INDEX) as i8 as i32 + 1) as u32;
    let i = (nested!(cpu, 0x0600_6a6e, math::remainder(cpu, i, 3)) as i32).wrapping_abs();
    cpu.write8(b + PULSE_INDEX, i as u8);
    cpu.write8(b + FRAME, (i + PULSE_FRAME as i32) as u8);
    let s = nested!(cpu, 0x0604_5d90, math::random(cpu, PULSE_FLICKER)).wrapping_add(PULSE_SIZE);
    cpu.write32(b + SCALES, s);
    cpu.write32(b + SCALES + 4, s);
    if cpu.read8(b + PULSE_INDEX) != 0 {
        let lift = (cpu.read32(GAME_FRAMES) & 1) * 118 + 8;
        cpu.write8(b + LIFT, lift as u8);
    }
    let which = if cpu.read32(GAME_FRAMES) & 1 != 0 { 0 } else { 2 };
    let colour = cpu.read16(b + PULSE_COLOURS + which);
    cpu.write16(b + OWN_COLOUR, colour);
}

/// Makes a pulsing bullet from the shot `shot`: the bullet, or 0 when there is no room.
pub fn pulse_set(cpu: &mut Cpu, shot: u32) -> u32 {
    let b = bullet_in(cpu, shot, PULSE_EXEC, 0, PULSE_BOXES);
    if b == 0 {
        return 0;
    }
    if cpu.read16(shot + S_FLAGS) & PULSE_SECOND != 0 {
        cpu.write8(b + PULSE_INDEX, 1);
    }
    let first = cpu.read16(CHAR_BASE);
    let (a, c) = if cpu.read16(shot + S_FLAGS) & PULSE_BRIGHT != 0 { (0x20, 0x30) } else { (0, 0x90) };
    cpu.write16(b + PULSE_COLOURS, first.wrapping_add(a));
    let first = cpu.read16(CHAR_BASE);
    cpu.write16(b + PULSE_COLOURS + 2, first.wrapping_add(c));
    let colour = cpu.read16(b + PULSE_COLOURS);
    cpu.write16(b + OWN_COLOUR, colour);
    cpu.write16(b + COLOUR, colour);
    let n = cpu.read16(shot + S_FLAGS) & PULSE_ACCEL_FRAMES;
    cpu.write16(b + ACCEL_FRAMES, n);
    let angle = cpu.read16(shot + S_ANGLE) as i16 as i32 as u32;
    let s = cpu.call(SL_SIN, &[angle]);
    let ax = nested!(cpu, 0x0600_6a78, math::quotient(cpu, s, 6));
    cpu.write16(b + ACCEL, ax as u16);
    let angle = cpu.read16(shot + S_ANGLE) as i16 as i32 as u32;
    let c = cpu.call(SL_COS, &[angle]);
    let ay = nested!(cpu, 0x0600_6a78, math::quotient(cpu, c, (-6i32) as u32));
    cpu.write16(b + ACCEL + 2, ay as u16);
    cpu.write32(b + SCALES, PULSE_SCALE);
    cpu.write32(b + SCALES + 4, PULSE_SCALE);
    b
}

/// A rocket (frame 200, its width flickering on odd frames, its length growing out to its full
/// length by the shot's flags, its colours in turn each 2 frames from a pair), speeding up along its
/// direction (a sixth of a pixel a frame) for the frames the shot gives; with shot flag 8 it starts
/// still on the ground (moving against the ground's movement). It leaves a puff of smoke where it
/// starts. Ended off screen, or with a spark (turned back) where it hits the ground.
const ROCKET_EXEC: u32 = 0x0605_5a68;
const ROCKET_BOXES: u32 = 0x0607_2f42;
const ROCKET_FRAME: u8 = 200;
const ROCKET_COLOURS: u32 = 0x0607_2f4a;
const ROCKET_START: u32 = 0x1000;
/// The shot's word at S_SPEED gives a rocket's flags: its second colour pair, its full length (by
/// sixteenths less, bits 4-7), from the ground.
const S_ROCKET_FLAGS: u32 = S_SPEED;
const ROCKET_PAIR: u16 = 1;
const ROCKET_SHORTER: u16 = 0xf0;
const ROCKET_GROUNDED: u16 = 8;
/// The rocket's colour pair (a word: 0 or 2), its length's growth (a word) and full length.
const ROCKET_PAIR_AT: u32 = 0x92;
const ROCKET_GROWTH: u32 = 0x96;
const ROCKET_LENGTH: u32 = 0xac;
/// The puff of smoke (the shot's description, made a drifting effect): its animation, colour (an
/// attribute bit, flipped at random), scales, speed (back), how far behind.
const PUFF_SCRIPT: u32 = 0x0607_30d4;
const PUFF_COLOUR: u16 = 0x80e0;
const PUFF_FLIP: u32 = 0x2000;
const PUFF_SCALES: [u32; 2] = [0xc000, 0x8000];
const PUFF_SPEED: u32 = 0xfffe_0000;
const PUFF_BEHIND: u32 = 100;

pub fn rocket_exec(cpu: &mut Cpu, b: u32) {
    if nested!(cpu, 0x0605_3b10, bullet_ends(cpu, b, 1)) as u8 != 0 {
        return;
    }
    accelerate(cpu, b);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    let w = 0x1_0000u32.wrapping_sub((cpu.read32(GAME_FRAMES) & 1).wrapping_mul(r & 0x7fff));
    cpu.write32(b + SCALES, w);
    let (h, to, by) = (cpu.read32(b + SCALES + 4), cpu.read32(b + ROCKET_LENGTH), cpu.read16(b + ROCKET_GROWTH) as i16 as i32 as u32);
    let h = nested!(cpu, 0x0604_6168, actors::approach(cpu, h, to, by));
    cpu.write32(b + SCALES + 4, h);
    let i = cpu.read16(b + ROCKET_PAIR_AT) as i16 as i32 + (cpu.read32(GAME_FRAMES) >> 1 & 1) as i32;
    let colour = cpu.read16(CHAR_BASE).wrapping_add(cpu.read16(ROCKET_COLOURS.wrapping_add((i * 2) as u32)));
    cpu.write16(b + OWN_COLOUR, colour);
    // (the SH-2 code then writes its colour back unchanged)
}

/// Makes a rocket from the shot `shot` (whose description then makes its puff of smoke): the
/// rocket, or 0 when there is no room.
pub fn rocket_set(cpu: &mut Cpu, shot: u32) -> u32 {
    let b = bullet_in(cpu, shot, ROCKET_EXEC, ROCKET_FRAME, ROCKET_BOXES);
    if b == 0 {
        return 0;
    }
    let n = cpu.read16(shot + S_FLAGS);
    cpu.write16(b + ACCEL_FRAMES, n);
    let angle = cpu.read16(shot + S_ANGLE) as i16 as i32 as u32;
    sixth_along(cpu, b, angle);
    let flags = cpu.read16(shot + S_ROCKET_FLAGS);
    cpu.write16(b + ROCKET_PAIR_AT, (flags & ROCKET_PAIR) * 2);
    let flags = cpu.read16(shot + S_ROCKET_FLAGS);
    cpu.write32(b + ROCKET_LENGTH, 0x1_0000u32.wrapping_sub(((flags & ROCKET_SHORTER) as u32) << 8));
    let flags = cpu.read16(shot + S_ROCKET_FLAGS);
    cpu.write16(b + ROCKET_GROWTH, 0x1000u16.wrapping_sub((flags & ROCKET_SHORTER) << 4));
    cpu.write32(b + SCALES + 4, ROCKET_START);
    if cpu.read16(shot + S_ROCKET_FLAGS) & ROCKET_GROUNDED != 0 {
        for (k, at) in [(0, super::globals::GROUND_DX_AT), (4, super::globals::GROUND_DY_AT)] {
            let p = cpu.read32(at);
            let v = cpu.read32(p).wrapping_neg();
            cpu.write32(b + SPEED + k, v);
        }
    }
    let pair = cpu.read16(b + ROCKET_PAIR_AT) as i16 as i32;
    let colour = cpu.read16(CHAR_BASE).wrapping_add(cpu.read16(ROCKET_COLOURS.wrapping_add((pair * 2) as u32)));
    cpu.write16(b + OWN_COLOUR, colour);
    cpu.write16(b + COLOUR, colour);
    cpu.write32(shot + E_SCRIPT, PUFF_SCRIPT);
    let angle = cpu.read16(shot + S_ANGLE);
    cpu.write16(shot + E_ANGLE, angle);
    let z = cpu.read32(shot + E_POSITION + 8).wrapping_sub(PUFF_BEHIND);
    cpu.write32(shot + E_POSITION + 8, z);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write16(shot + E_COLOUR, ((r & PUFF_FLIP) as u16).wrapping_add(PUFF_COLOUR));
    cpu.write32(shot + E_SCALES, PUFF_SCALES[0]);
    cpu.write32(shot + E_SCALES + 4, PUFF_SCALES[1]);
    cpu.write32(shot + E_SPEED, PUFF_SPEED);
    let e = nested!(cpu, 0x0605_7454, actors::drift_effect(cpu, shot));
    if e != 0 {
        cpu.write8(e + LIFT, BULLET_LIFT);
    }
    b
}

/// A rocket that speeds up for 13 frames, plain (the shot's flags so set first).
pub fn rocket_set_plain(cpu: &mut Cpu, shot: u32) -> u32 {
    cpu.write16(shot + S_FLAGS, 13);
    cpu.write16(shot + S_ROCKET_FLAGS, 0);
    nested!(cpu, 0x0605_5b5c, rocket_set(cpu, shot))
}


/// The laser: a muzzle (laser_set) kept out from the gun that fires it along the gun's angle (its
/// own turn added, swinging), that sends out a segment every frame. Its power rises to 0x1000
/// while it lasts, then falls away, ending it; it sounds every 12 frames (louder at full power)
/// and flickers. Each segment (laser_segment_set) runs out from the muzzle 16 pixels a frame
/// along the muzzle's angle; one that hits something or the ground marks its length on the
/// muzzle (those further out end next frame) and ends in a puff of smoke.
const LASER_SIZE: u32 = 0x8c;
const LASER_LIST: u32 = 2;
const MUZZLE_EXEC: u32 = 0x0605_4504;
const SEGMENT_EXEC: u32 = 0x0605_41f0;
/// The laser's description (a shot's): what fires it, its turn, its distance out, its frames,
/// its swing a frame (words).
const LS_FROM: u32 = 0x3c;
const LS_TURN: u32 = 0x2c;
const LS_OUT: u32 = 0x2e;
const LS_FRAMES: u32 = 0x30;
const LS_SWING: u32 = 0x32;
/// The muzzle's fields: its gun, where its angle comes from, its distance out, the length a
/// segment hit (this frame's, last frame's), its power, frames left, turn and swing (words).
const M_GUN: u32 = 0x6c;
const M_ANGLE_AT: u32 = 0x70;
const M_OUT: u32 = 0x7c;
const M_HIT: u32 = 0x7e;
const M_LAST_HIT: u32 = 0x80;
const M_POWER: u32 = 0x82;
const M_LEFT: u32 = 0x84;
const M_TURN: u32 = 0x86;
const M_SWING: u32 = 0x88;
const MUZZLE_FLAGS: u32 = 0x2008_0400;
const MUZZLE_FRAME: u8 = 0x37;
const MUZZLE_FRAMES: u8 = 0x36;
const MUZZLE_COLOUR: u16 = 0x30;
const MUZZLE_LIFT: u8 = 10;
const MUZZLE_HEIGHT: u32 = 0x4000;
const FULL_POWER: u32 = 0x1000;
const POWER_UP: u32 = 0x40;
const POWER_DOWN: u32 = 0x80;
const SE_LASER: [u32; 2] = [34, 35];
const LASER_SOUND_EVERY: u32 = 12;
const MIRRORED: u32 = 0x10;
/// The segment's description (the muzzle's frame): its player byte and power (S_ANGLE's word).
const SG_POWER: u32 = S_ANGLE;
/// The segment's fields: its muzzle, where its angle comes from, where it runs from, the
/// muzzle's hit lengths, its length (a word).
const SG_MUZZLE: u32 = 0x6c;
const SG_ANGLE_AT: u32 = 0x70;
const SG_FROM: u32 = 0x74;
const SG_HITS: u32 = 0x78;
const SG_LENGTH: u32 = 0x7c;
const SG_STEP: u16 = 16;
const SEGMENT_BOXES: u32 = 0x0607_2efc;
const SEGMENT_FLAGS: u32 = 0x2000_0400;
const SEGMENT_LIFT: u8 = 8;
const SEGMENT_COLOURS: [u16; 2] = [0x20, 0x30];
/// Below full power a segment is drawn thinner (frame 0x5a at 0x200 and below), with the
/// attribute bit 0x100.
const SEGMENT_FRAMES: [u8; 2] = [0x59, 0x5a];
const SEGMENT_THIN: i16 = 0x200;
const SEGMENT_ATTR: u32 = 0x100;
const SEGMENT_58: u32 = 0x58;
/// The puff a segment ends in (actors.rs's drifting effect): by the attribute bit its
/// animation and width, drifting back at half a pixel a frame, at a random angle, its colour
/// 0x20 (an attribute bit at random).
const SEGMENT_PUFFS: [(u32, u32); 2] = [(0x0607_30d4, 0xa000), (0x0607_30e8, 0x1_0000)];
const SEGMENT_PUFF_SPEED: u32 = 0xffff_8000;
const SEGMENT_PUFF_COLOUR: u16 = 0x20;
const SEGMENT_PUFF_ATTR: u32 = 0x8000;
const SL_SIN_L: u32 = 0x0606_9a0c;
const SL_COS_L: u32 = 0x0606_9a30;

/// `o` placed `out` along `angle` (a word) from the place at `from` (x, y longs).
fn out_from(cpu: &mut Cpu, o: u32, from: u32, angle: u16, out: i32) {
    let a = angle as i16 as i32 as u32;
    let s = cpu.call(SL_SIN_L, &[a]) as i32;
    let x = cpu.read32(from).wrapping_add(s.wrapping_mul(out) as u32);
    cpu.write32(o + POSITION, x);
    let c = cpu.call(SL_COS_L, &[a]) as i32;
    let y = cpu.read32(from + 4).wrapping_add(c.wrapping_neg().wrapping_mul(out) as u32);
    cpu.write32(o + POSITION + 4, y);
}

/// Makes a laser from the shot description at `d`, its angle the word at `angle_at` (and its
/// turn).
pub fn laser_set(cpu: &mut Cpu, d: u32, angle_at: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, LASER_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, LASER_LIST, blocks, ACTOR_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + SPRITE));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + BOXES));
    cpu.write32(o + BEHAVIOUR, MUZZLE_EXEC);
    let n = cpu.read8(d + S_NUMBER);
    cpu.write8(o + NUMBER, n);
    cpu.write32(o + FLAGS, MUZZLE_FLAGS);
    cpu.write8(o + FRAME, MUZZLE_FRAME);
    cpu.write8(o + KIND, 0);
    let colour = cpu.read16(CHAR_BASE).wrapping_add(MUZZLE_COLOUR);
    cpu.write16(o + OWN_COLOUR, colour);
    cpu.write16(o + COLOUR, colour);
    let gun = cpu.read32(d + LS_FROM);
    cpu.write32(o + M_GUN, gun);
    cpu.write32(o + M_ANGLE_AT, angle_at);
    for (field, from) in [(M_TURN, LS_TURN), (M_OUT, LS_OUT), (M_LEFT, LS_FRAMES), (M_SWING, LS_SWING)] {
        let v = cpu.read16(d + from);
        cpu.write16(o + field, v);
    }
    cpu.write16(o + M_POWER, 0);
    let a = cpu.read16(angle_at).wrapping_add(cpu.read16(o + M_TURN));
    cpu.write16(o + ANGLE, a);
    let out = cpu.read16(o + M_OUT) as i16 as i32;
    out_from(cpu, o, gun + POSITION, a, out);
    let z = cpu.read32(gun + POSITION + 8);
    cpu.write32(o + POSITION + 8, z);
    cpu.write8(o + LIFT, MUZZLE_LIFT);
    cpu.write32(o + SCALES + 4, MUZZLE_HEIGHT);
}

/// The muzzle's behaviour (see laser_set).
pub fn muzzle_exec(cpu: &mut Cpu, o: u32) {
    let f = cpu.stack();
    let gun = cpu.read32(o + M_GUN);
    if cpu.read8(gun + OBJ_MODE) as i8 >= ENDING as i8 {
        cpu.write8(o + OBJ_MODE, ENDING);
        return;
    }
    let hit = cpu.read16(o + M_HIT);
    cpu.write16(o + M_LAST_HIT, hit);
    if hit != 0 {
        cpu.write16(o + M_HIT, 0);
    }
    let angle_at = cpu.read32(o + M_ANGLE_AT);
    let a = cpu.read16(angle_at).wrapping_add(cpu.read16(o + M_TURN));
    cpu.write16(o + ANGLE, a);
    let out = cpu.read16(o + M_OUT) as i16 as i32;
    out_from(cpu, o, gun + POSITION, a, out);
    let z = cpu.read32(gun + POSITION + 8);
    cpu.write32(o + POSITION + 8, z);
    let a = cpu.read16(o + ANGLE).wrapping_add(cpu.read16(o + M_SWING));
    cpu.write16(o + ANGLE, a);
    let left = cpu.read16(o + M_LEFT).wrapping_sub(1);
    cpu.write16(o + M_LEFT, left);
    let power = cpu.read16(o + M_POWER) as i16 as i32 as u32;
    if (left as i16) < 0 {
        let power = nested!(cpu, 0x0604_6168, actors::approach(cpu, power, 0, POWER_DOWN));
        cpu.write16(o + M_POWER, power as u16);
        if power as i16 == 0 {
            cpu.write8(o + OBJ_MODE, ENDING);
            return;
        }
    } else {
        let power = nested!(cpu, 0x0604_6168, actors::approach(cpu, power, FULL_POWER, POWER_UP));
        cpu.write16(o + M_POWER, power as u16);
    }
    let frames = cpu.read32(GAME_FRAMES);
    if nested!(cpu, 0x0600_6a6e, math::remainder(cpu, frames, LASER_SOUND_EVERY)) == 0 {
        let full = cpu.read16(o + M_POWER) as u32 == FULL_POWER;
        nested!(cpu, 0x0600_50d4, super::sound::se_set(cpu, SE_LASER[full as usize]));
    }
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write8(o + FRAME, (r as u8 & 3).wrapping_add(MUZZLE_FRAMES));
    let flags = cpu.read32(o + FLAGS) & !MIRRORED;
    cpu.write32(o + FLAGS, flags);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    let flags = cpu.read32(o + FLAGS) | r & MIRRORED;
    cpu.write32(o + FLAGS, flags);
    let power = cpu.read16(o + M_POWER) as i16 as i32;
    cpu.write32(o + SCALES, power.wrapping_mul(18) as u32);
    cpu.write32(o + SCALES + 4, power.wrapping_mul(8) as u32);
    let n = cpu.read8(o + NUMBER);
    cpu.write8(f + S_NUMBER, n);
    let power = cpu.read16(o + M_POWER);
    cpu.write16(f + SG_POWER, power);
    let from = o + POSITION;
    cpu.push(&[from]);
    nested!(cpu, 0x0605_4354, laser_segment_set(cpu, f, o, o + M_HIT, o + ANGLE, from));
    cpu.pop(4);
}

/// Makes a laser segment from the description at `d` (its player byte, its power), of `muzzle`
/// (its hit lengths at `hits`), its angle the word at `angle_at`, running from the place at
/// `from` (x, y, z).
pub fn laser_segment_set(cpu: &mut Cpu, d: u32, muzzle: u32, hits: u32, angle_at: u32, from: u32) {
    let f = cpu.stack();
    cpu.write32(f, angle_at);
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, LASER_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, LASER_LIST, blocks, ACTOR_EXEC));
    if o == 0 {
        return;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + SPRITE));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + BOXES));
    cpu.write32(o + BEHAVIOUR, SEGMENT_EXEC);
    let n = cpu.read8(d + S_NUMBER);
    cpu.write8(o + NUMBER, n);
    cpu.write32(o + BOXES, SEGMENT_BOXES);
    cpu.write8(o + SIDE, BULLET_SIDE);
    cpu.write8(o + TYPE, BULLET_TYPE);
    cpu.write8(o + KIND, 0);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    let colour = cpu.read16(CHAR_BASE).wrapping_add(SEGMENT_COLOURS[(r & 3 != 0) as usize]);
    cpu.write16(o + COLOUR, colour);
    let colour = cpu.read16(o + COLOUR);
    cpu.write16(o + OWN_COLOUR, colour);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write32(o + FLAGS, r & MIRRORED);
    let power = cpu.read16(d + SG_POWER);
    if power as u32 == FULL_POWER {
        cpu.write8(o + FRAME, SEGMENT_FRAMES[0]);
    } else {
        cpu.write8(o + FRAME, SEGMENT_FRAMES[(power as i16 <= SEGMENT_THIN) as usize]);
        let flags = cpu.read32(o + FLAGS) | SEGMENT_ATTR;
        cpu.write32(o + FLAGS, flags);
        cpu.write16(o + SEGMENT_58, 0);
    }
    let flags = cpu.read32(o + FLAGS) | SEGMENT_FLAGS;
    cpu.write32(o + FLAGS, flags);
    cpu.write32(o + SG_MUZZLE, muzzle);
    cpu.write32(o + SG_HITS, hits);
    cpu.write32(o + SG_ANGLE_AT, angle_at);
    cpu.write32(o + SG_FROM, from);
    cpu.write16(o + SG_LENGTH, SG_STEP);
    let a = cpu.read16(angle_at);
    cpu.write16(o + ANGLE, a);
    out_from(cpu, o, from, a, SG_STEP as i32);
    let z = cpu.read32(from + 8);
    cpu.write32(o + POSITION + 8, z);
    cpu.write8(o + LIFT, SEGMENT_LIFT);
    let width = (cpu.read16(d + SG_POWER) as i16 as i32) << 2;
    cpu.write32(o + SCALES, width as u32);
}

/// A laser segment's behaviour (see laser_set).
pub fn laser_segment_exec(cpu: &mut Cpu, o: u32) {
    let f = cpu.stack();
    let a = cpu.read16(o + ANGLE);
    let length = cpu.read16(o + SG_LENGTH) as i16 as i32;
    let from = cpu.read32(o + SG_FROM);
    out_from(cpu, o, from, a, length);
    cpu.write32(o + POSITION + 8, 0);
    let muzzle = cpu.read32(o + SG_MUZZLE);
    let hits = cpu.read32(o + SG_HITS);
    let off = nested!(cpu, 0x0605_6a24, aim::off_screen(cpu, o + SPRITE)) as u8 != 0;
    let last_hit = cpu.read16(hits + 2) as i16;
    if off || cpu.read8(muzzle + OBJ_MODE) as i8 >= ENDING as i8 || (last_hit != 0 && last_hit < cpu.read16(o + SG_LENGTH) as i16) {
        cpu.write8(o + OBJ_MODE, ENDING);
        return;
    }
    let angle_at = cpu.read32(o + SG_ANGLE_AT);
    let a = cpu.read16(angle_at);
    cpu.write16(o + ANGLE, a);
    let stopped = cpu.read16(o + HIT_FLAGS) & HIT_SOMETHING != 0 || {
        let (x, y) = (cpu.read32(o + POSITION), cpu.read32(o + POSITION + 4));
        nested!(cpu, 0x0605_ef90, bg::map_hit(cpu, x, y)) != 0
    };
    if !stopped {
        let length = cpu.read16(o + SG_LENGTH).wrapping_add(SG_STEP);
        cpu.write16(o + SG_LENGTH, length);
        return;
    }
    let length = cpu.read16(o + SG_LENGTH);
    cpu.write16(hits, length);
    cpu.write8(o + OBJ_MODE, ENDING);
    let (script, width) = SEGMENT_PUFFS[(cpu.read32(o + FLAGS) & SEGMENT_ATTR != 0) as usize];
    cpu.write32(f + E_SCRIPT, script);
    cpu.write32(f + E_SCALES + 4, width);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write16(f + E_COLOUR, ((r & SEGMENT_PUFF_ATTR) as u16).wrapping_add(SEGMENT_PUFF_COLOUR));
    cpu.write32(f + E_SCALES, 0x1_0000);
    for k in 0..3 {
        let v = cpu.read32(o + POSITION + 4 * k);
        cpu.write32(f + E_POSITION + 4 * k, v);
    }
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write16(f + E_ANGLE, r as u16);
    cpu.write32(f + E_SPEED, SEGMENT_PUFF_SPEED);
    nested!(cpu, 0x0605_7454, actors::drift_effect(cpu, f));
}

/// The missile: a shootable enemy (100 life) launched at two pixels a frame in the description's
/// launch direction, turning toward the nearer player (a 32nd of the way, the aim offset and
/// masked) for its homing frames, its speed (x, y) easing toward its own speed along its heading
/// (by a sixteenth of the heading's sine and cosine); a jet behind it. On the ground, or hit, it
/// bursts (sound 12, a ring); off screen it ends.
const MISSILE_EXEC: u32 = 0x0605_47ec;
const MISSILE_FLAGS: u32 = 0x2030_0400;
const MISSILE_LIFT: u8 = 8;
const MISSILE_FRAME: u8 = 18;
const MISSILE_KIND: u8 = 12;
const MISSILE_COLOUR: u32 = 0x0607_42bc;
const MISSILE_BOXES: u32 = 0x0607_2f0c;
const MISSILE_LIFE: u32 = 100;
const MISSILE_SIZE: u32 = 0xbc;
const MISSILE_LAUNCH: i32 = 2;
/// The missile's description (a shot's): its heading, homing frames, aim offset (also its
/// mask's), launch direction (words), its speed.
const MD_HOMING: u32 = 0x2e;
const MD_AIM: u32 = 0x32;
const MD_LAUNCH: u32 = 0x34;
const MD_SPEED: u32 = 0x38;
/// The missile's fields: its speed (x, y), heading, homing frames, aim offset and mask (words),
/// its own speed.
const MS_VX: u32 = 0x80;
const MS_VY: u32 = 0x84;
const MS_HEADING: u32 = 0x8c;
const MS_HOMING: u32 = 0x8e;
const MS_AIM: u32 = 0x90;
const MS_MASK: u32 = 0x92;
const MS_SPEED: u32 = 0xac;
/// The jet (explosions.rs's jet_set: from the missile, its reach and size).
const JD_FROM: u32 = 0x3c;
const JD_REACH: u32 = 0x2c;
const JD_SIZE: u32 = 0x2e;
const MISSILE_JET: (u16, u16) = (12, 4);
/// The burst: sound 12, a ring (explosions.rs's ring_set: one, 0x2000 wide, 0xc00 high, still).
const SE_MISSILE_BURST: u32 = 12;
const HIT_OR_GROUND: u16 = 0x600;
const RD_ONE: u32 = 0x20;
const RD_WIDTH: u32 = 0x2c;
const RD_HEIGHT: u32 = 0x2e;
const RD_SPEED: u32 = 0x30;
const MISSILE_RING: (u16, u16) = (0x2000, 0xc00);
/// The plain missile (missile_set_plain): homing for 120 frames, its aim offset 0x2000, launched
/// along its heading, its speed 2.25.
const PLAIN_MISSILE: (u16, u16, u32) = (120, 0x2000, 0x2_4000);

/// Makes a missile from the shot description at `d`. It, or 0.
pub fn missile_set(cpu: &mut Cpu, d: u32) -> u32 {
    let f = cpu.stack();
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, MISSILE_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, LASER_LIST, blocks, ACTOR_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + SPRITE));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + BOXES));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + BEHAVIOUR, MISSILE_EXEC);
    let n = cpu.read8(d + S_NUMBER);
    cpu.write8(o + NUMBER, n);
    cpu.write32(o + FLAGS, MISSILE_FLAGS);
    for k in 0..3 {
        let v = cpu.read32(d + 4 * k);
        cpu.write32(o + POSITION + 4 * k, v);
    }
    cpu.write8(o + LIFT, MISSILE_LIFT);
    cpu.write8(o + FRAME, MISSILE_FRAME);
    cpu.write8(o + KIND, MISSILE_KIND);
    let colour = cpu.read16(MISSILE_COLOUR).wrapping_add(16);
    cpu.write16(o + OWN_COLOUR, colour);
    cpu.write16(o + COLOUR, colour);
    cpu.write32(o + BOXES, MISSILE_BOXES);
    cpu.write8(o + SIDE, BULLET_SIDE);
    cpu.write32(o + LIFE, MISSILE_LIFE);
    let launch = cpu.read16(d + MD_LAUNCH) as i16 as i32 as u32;
    let s = cpu.call(SL_SIN_L, &[launch]) as i32;
    cpu.write32(o + MS_VX, s.wrapping_mul(MISSILE_LAUNCH) as u32);
    let launch = cpu.read16(d + MD_LAUNCH) as i16 as i32 as u32;
    let c = cpu.call(SL_COS_L, &[launch]) as i32;
    cpu.write32(o + MS_VY, c.wrapping_neg().wrapping_mul(MISSILE_LAUNCH) as u32);
    for (field, from) in [(MS_HEADING, S_ANGLE), (MS_HOMING, MD_HOMING), (MS_AIM, MD_AIM)] {
        let v = cpu.read16(d + from);
        cpu.write16(o + field, v);
    }
    let mask = cpu.read16(d + MD_AIM).wrapping_mul(2).wrapping_neg();
    cpu.write16(o + MS_MASK, mask);
    let speed = cpu.read32(d + MD_SPEED);
    cpu.write32(o + MS_SPEED, speed);
    cpu.write32(f + JD_FROM, o);
    cpu.write16(f + JD_REACH, MISSILE_JET.0);
    cpu.write16(f + JD_SIZE, MISSILE_JET.1);
    nested!(cpu, 0x0605_7fe8, super::explosions::jet_set(cpu, f));
    o
}

/// A plain missile from the shot description at `d` (its fields so set first). It, or 0.
pub fn missile_set_plain(cpu: &mut Cpu, d: u32) -> u32 {
    let (homing, aim, speed) = PLAIN_MISSILE;
    cpu.write32(d + MD_SPEED, speed);
    cpu.write16(d + MD_HOMING, homing);
    cpu.write16(d + MD_AIM, aim);
    let heading = cpu.read16(d + S_ANGLE);
    cpu.write16(d + MD_LAUNCH, heading);
    nested!(cpu, 0x0605_49f8, missile_set(cpu, d))
}

/// A sixteenth of `v` (toward zero), unsigned.
fn sixteenth_size(v: i32) -> u32 {
    (v / 16).unsigned_abs()
}

/// The missile's behaviour (see missile_set).
pub fn missile_exec(cpu: &mut Cpu, o: u32) {
    let f = cpu.stack();
    let (x, y) = (cpu.read32(o + POSITION), cpu.read32(o + POSITION + 4));
    if nested!(cpu, 0x0605_ef90, bg::map_hit(cpu, x, y)) != 0 {
        cpu.write16(o + HIT_FLAGS, HIT_SOMETHING);
    }
    let flies = cpu.read16(o + HIT_FLAGS) & HIT_SOMETHING == 0
        && nested!(cpu, 0x0605_6a24, aim::off_screen(cpu, o + SPRITE)) as u8 == 0
        && cpu.read32(o + LIFE) as i32 > 0;
    if !flies {
        if cpu.read16(o + HIT_FLAGS) & HIT_OR_GROUND != 0 {
            nested!(cpu, 0x0600_50d4, super::sound::se_set(cpu, SE_MISSILE_BURST));
            for k in 0..3 {
                let v = cpu.read32(o + POSITION + 4 * k);
                cpu.write32(f + 4 * k, v);
            }
            cpu.write16(f + RD_ONE, 1);
            cpu.write16(f + RD_WIDTH, MISSILE_RING.0);
            cpu.write16(f + RD_HEIGHT, MISSILE_RING.1);
            for k in 0..3 {
                cpu.write32(f + RD_SPEED + 4 * k, 0);
            }
            nested!(cpu, 0x0605_9144, super::explosions::ring_set(cpu, f));
        }
        cpu.write8(o + OBJ_MODE, ENDING);
        return;
    }
    let homing = cpu.read16(o + MS_HOMING) as i16;
    if homing > 0 {
        cpu.write16(o + MS_HOMING, (homing - 1) as u16);
        let (off, mask) = (cpu.read16(o + MS_AIM) as u32, cpu.read16(o + MS_MASK) as u32);
        let p = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + SPRITE));
        let a = nested!(cpu, 0x0604_5e30, aim::angle_to(cpu, o, p)) as i16 as i32 as u32;
        let to = off.wrapping_add(a) & mask;
        let h = cpu.read16(o + MS_HEADING) as i16 as i32 as u32;
        let d = nested!(cpu, 0x0604_6194, super::camera::angle_diff(cpu, h, to)) as i32;
        let h = ((d / 32) as u16).wrapping_add(cpu.read16(o + MS_HEADING));
        cpu.write16(o + MS_HEADING, h);
    }
    let h = cpu.read16(o + MS_HEADING);
    cpu.write16(o + ANGLE, h);
    let h = h as i16 as i32 as u32;
    let s = cpu.call(SL_SIN_L, &[h]) as i32;
    let step = sixteenth_size(s);
    let speed = cpu.read32(o + MS_SPEED);
    let s = cpu.call(SL_SIN_L, &[h]);
    let to = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, speed));
    let vx = cpu.read32(o + MS_VX);
    let vx = nested!(cpu, 0x0604_6168, actors::approach(cpu, vx, to, step));
    cpu.write32(o + MS_VX, vx);
    let c = cpu.call(SL_COS_L, &[h]) as i32;
    let step = sixteenth_size(c.wrapping_neg());
    let speed = cpu.read32(o + MS_SPEED);
    let c = cpu.call(SL_COS_L, &[h]) as i32;
    let to = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, c.wrapping_neg() as u32, speed));
    let vy = cpu.read32(o + MS_VY);
    let vy = nested!(cpu, 0x0604_6168, actors::approach(cpu, vy, to, step));
    cpu.write32(o + MS_VY, vy);
    for (k, field) in [MS_VX, MS_VY].into_iter().enumerate() {
        let p = o + POSITION + 4 * k as u32;
        let v = cpu.read32(p).wrapping_add(cpu.read32(o + field));
        cpu.write32(p, v);
    }
}

/// The shell: a shootable enemy (800 life) fired at its launch speed along the description's
/// launch direction, the launch easing off (by a 64th of it a frame) while its own speed eases in
/// along its heading (by a 32nd), cycling frames 6-9 every 4 frames and trailing smoke (an
/// orbiter every other frame of its fuse). A shell with a burst (kinds 1-3) lights its fuse
/// within 64 pixels of the nearer player; 50 frames later, on the ground, hit, or out of life it
/// bursts (sound 18): four bullets 0xc00 apart about the player's direction (kind 1), a ring
/// thrown toward the player (kind 3), else a cloud. Off screen it ends.
const SHELL_EXEC: u32 = 0x0605_547c;
const SHELL_FLAGS: u32 = 0x2030_0400;
const SHELL_FRAME: u8 = 6;
const SHELL_LAST_FRAME: u8 = 9;
const SHELL_KIND: u8 = 12;
const SHELL_BOXES: u32 = 0x0607_2f32;
const SHELL_LIFE: u32 = 800;
/// The description: launch direction, speed, launch speed, kind (also its colour, by threes).
const SD_LAUNCH: u32 = 0x2e;
const SD_SPEED: u32 = 0x30;
const SD_LAUNCH_SPEED: u32 = 0x34;
const SD_KIND: u32 = 0x38;
/// The shell's fields: its fuse (a byte: 0 unlit, then counting to 50), speed (x, y), heading,
/// kind, ended (the smoke watches it), launch speed (x, y) and its easing (words), speed.
const SH_FUSE: u32 = 0x7c;
const SH_VX: u32 = 0x80;
const SH_VY: u32 = 0x84;
const SH_HEADING: u32 = 0x8a;
const SH_KIND_OF: u32 = 0x8c;
const SH_ENDED: u32 = 0x8e;
const SH_EASE_X: u32 = 0xa2;
const SH_EASE_Y: u32 = 0xa4;
const SH_SPEED: u32 = 0xac;
const SH_LAUNCH_X: u32 = 0xb4;
const SH_LAUNCH_Y: u32 = 0xb8;
const FUSE_FRAMES: u8 = 50;
const FUSE_NEAR: i32 = 0x3f_ffff;
const SE_SHELL_BURST: u32 = 18;
const SPREAD_SHOTS: u8 = 4;
const SPREAD_FROM: u16 = 0xee00;
const SPREAD_STEP: u16 = 0xc00;
/// The smoke (orbits.rs's orbiter: kind 0, colours 1, shrinking, turning at random, lift 8,
/// still; ending when the shell does).
const SMOKE_FIELDS: [(u32, u16); 3] = [(0x2c, 0), (0x20, 1), (0x18, 0)];
const SMOKE_SCALES: [u32; 2] = [0x8000, 0x1_0000];
const SMOKE_SHRINK: [u32; 2] = [0xffff_ff00, 0xffff_f800];
const SMOKE_LIFT: u16 = 8;
static SHELL_SMOKE_PROLOGUE: std::sync::OnceLock<super::Prologue> = std::sync::OnceLock::new();
const ORBIT_SET: u32 = 0x0605_9554;
/// The ring burst (burst_set's ball, as the ribbon's end makes it; its speed a quarter
/// of the direction toward the player), and the cloud (a drifting effect, still).
const RING_BURST: u32 = 0x0605_5310;
const RING_FIELDS: [(u32, u16); 4] = [(0x2c, 0x200), (0x2e, 0x60), (0x38, 0xfffa), (0x3a, 1)];
const CLOUD_SCRIPT: u32 = 0x0607_305c;
const CLOUD_COLOUR: u16 = 0x90;
/// The SH-2 code keeps two field offsets in its frame.
const F_SPEED_FIELD: u32 = 0x40;
const F_HEADING_FIELD: u32 = 0x44;

/// Makes a shell from the shot description at `d`. It, or 0.
pub fn shell_set(cpu: &mut Cpu, d: u32) -> u32 {
    let f = cpu.stack();
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, MISSILE_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, LASER_LIST, blocks, ACTOR_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + SPRITE));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + BOXES));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + BEHAVIOUR, SHELL_EXEC);
    let n = cpu.read8(d + S_NUMBER);
    cpu.write8(o + NUMBER, n);
    cpu.write32(o + FLAGS, SHELL_FLAGS);
    for k in 0..3 {
        let v = cpu.read32(d + 4 * k);
        cpu.write32(o + POSITION + 4 * k, v);
    }
    cpu.write8(o + LIFT, MISSILE_LIFT);
    cpu.write8(o + FRAME, SHELL_FRAME);
    cpu.write8(o + KIND, SHELL_KIND);
    cpu.write32(f, d + SD_KIND);
    let kind = cpu.read16(d + SD_KIND) as i16 as i32 as u32;
    let c = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, kind, 3));
    let colour = cpu.read16(MISSILE_COLOUR).wrapping_add((c as u16).wrapping_mul(16));
    cpu.write16(o + OWN_COLOUR, colour);
    cpu.write16(o + COLOUR, colour);
    cpu.write32(o + BOXES, SHELL_BOXES);
    cpu.write8(o + SIDE, BULLET_SIDE);
    cpu.write32(o + LIFE, SHELL_LIFE);
    let launch = cpu.read16(d + SD_LAUNCH) as i16 as i32 as u32;
    let speed = cpu.read32(d + SD_LAUNCH_SPEED);
    let s = cpu.call(SL_SIN_L, &[launch]);
    let vx = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, speed));
    cpu.write32(o + SH_LAUNCH_X, vx);
    let speed = cpu.read32(d + SD_LAUNCH_SPEED);
    let c = cpu.call(SL_COS_L, &[launch]) as i32;
    let vy = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, c.wrapping_neg() as u32, speed));
    cpu.write32(o + SH_LAUNCH_Y, vy);
    for (field, v) in [(SH_EASE_X, SH_LAUNCH_X), (SH_EASE_Y, SH_LAUNCH_Y)] {
        let v = cpu.read32(o + v) as i32;
        cpu.write16(o + field, (v / 64).unsigned_abs() as u16);
    }
    let heading = cpu.read16(d + S_ANGLE);
    cpu.write16(o + SH_HEADING, heading);
    let kind = cpu.read16(d + SD_KIND);
    cpu.write16(o + SH_KIND_OF, kind);
    cpu.write16(o + SH_ENDED, 0);
    let speed = cpu.read32(d + SD_SPEED);
    cpu.write32(o + SH_SPEED, speed);
    o
}

/// A 32nd of `v` (toward zero), unsigned.
fn thirty_second_size(v: i32) -> u32 {
    (v / 32).unsigned_abs()
}

/// The shell's behaviour (see shell_set).
pub fn shell_exec(cpu: &mut Cpu, o: u32) {
    let f = cpu.stack();
    let (x, y) = (cpu.read32(o + POSITION), cpu.read32(o + POSITION + 4));
    if nested!(cpu, 0x0605_ef90, bg::map_hit(cpu, x, y)) != 0 {
        cpu.write16(o + HIT_FLAGS, HIT_SOMETHING);
    }
    let whole = |cpu: &mut Cpu| cpu.read32(o + LIFE) as i32 > 0 && cpu.read16(o + HIT_FLAGS) & HIT_SOMETHING == 0;
    let flies = whole(cpu)
        && nested!(cpu, 0x0605_6a24, aim::off_screen(cpu, o + SPRITE)) as u8 == 0
        && cpu.read8(o + SH_FUSE) <= FUSE_FRAMES;
    if !flies {
        cpu.write16(o + SH_ENDED, 1);
        if !whole(cpu) || cpu.read8(o + SH_FUSE) > FUSE_FRAMES {
            shell_burst(cpu, o, f);
        }
        cpu.write8(o + OBJ_MODE, ENDING);
        return;
    }
    let fuse = cpu.read8(o + SH_FUSE);
    if fuse != 0 {
        let fuse = fuse.wrapping_add(1);
        cpu.write8(o + SH_FUSE, fuse);
        if fuse & 1 == 0 {
            for (field, v) in SMOKE_FIELDS {
                cpu.write16(f + field, v);
            }
            cpu.write32(f + 0xc, SMOKE_SCALES[0]);
            cpu.write32(f + 0x10, SMOKE_SCALES[1]);
            cpu.write32(f + 0x34, SMOKE_SHRINK[0]);
            cpu.write32(f + 0x38, SMOKE_SHRINK[1]);
            let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
            cpu.write16(f + 0x2e, r as u16);
            let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x100));
            cpu.write16(f + 0x30, r as u16);
            cpu.write16(f + 0x32, SMOKE_LIFT);
            cpu.write32(f + 0x24, 0);
            cpu.write32(f + 0x28, 0);
            let (watched, centre, angle) = (o + SH_ENDED, o + POSITION, o + ANGLE);
            cpu.nested_with_stack(&SHELL_SMOKE_PROLOGUE, ORBIT_SET, &[angle], |cpu| super::orbits::orbit_set(cpu, f, watched, 1, centre, angle));
        }
    }
    if cpu.read16(o + SH_KIND_OF) != 0 {
        let p = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + SPRITE));
        let dx = cpu.read32(p + POSITION).wrapping_sub(cpu.read32(o + POSITION));
        let dy = cpu.read32(p + POSITION + 4).wrapping_sub(cpu.read32(o + POSITION + 4));
        if nested!(cpu, 0x0604_5e60, aim::length(cpu, dx, dy)) as i32 <= FUSE_NEAR {
            let fuse = cpu.read8(o + SH_FUSE).wrapping_add(1);
            cpu.write8(o + SH_FUSE, fuse);
        }
    }
    for (field, ease) in [(SH_LAUNCH_X, SH_EASE_X), (SH_LAUNCH_Y, SH_EASE_Y)] {
        let (v, step) = (cpu.read32(o + field), cpu.read16(o + ease) as i16 as i32 as u32);
        let v = nested!(cpu, 0x0604_6168, actors::approach(cpu, v, 0, step));
        cpu.write32(o + field, v);
    }
    cpu.write32(f + F_SPEED_FIELD, SH_SPEED);
    cpu.write32(f + F_HEADING_FIELD, SH_HEADING);
    let h = cpu.read16(o + SH_HEADING) as i16 as i32 as u32;
    let speed = cpu.read32(o + SH_SPEED);
    let s = cpu.call(SL_SIN_L, &[h]);
    let to = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, speed));
    let vx = cpu.read32(o + SH_VX);
    let vx = nested!(cpu, 0x0604_6168, actors::approach(cpu, vx, to, thirty_second_size(to as i32)));
    cpu.write32(o + SH_VX, vx);
    let speed = cpu.read32(o + SH_SPEED);
    let c = cpu.call(SL_COS_L, &[h]) as i32;
    let to = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, c.wrapping_neg() as u32, speed));
    let vy = cpu.read32(o + SH_VY);
    let vy = nested!(cpu, 0x0604_6168, actors::approach(cpu, vy, to, thirty_second_size(to as i32)));
    cpu.write32(o + SH_VY, vy);
    for (k, (v, launch)) in [(SH_VX, SH_LAUNCH_X), (SH_VY, SH_LAUNCH_Y)].into_iter().enumerate() {
        let p = o + POSITION + 4 * k as u32;
        let d = cpu.read32(o + v).wrapping_add(cpu.read32(o + launch));
        let v = cpu.read32(p).wrapping_add(d);
        cpu.write32(p, v);
    }
    if cpu.read32(GAME_FRAMES) & 3 == 0 {
        let frame = cpu.read8(o + FRAME).wrapping_add(1);
        cpu.write8(o + FRAME, frame);
    }
    if cpu.read8(o + FRAME) > SHELL_LAST_FRAME {
        cpu.write8(o + FRAME, SHELL_FRAME);
    }
}

/// The shell's burst, by its kind (the description at `f`).
fn shell_burst(cpu: &mut Cpu, o: u32, f: u32) {
    let n = cpu.read8(o + NUMBER);
    cpu.write8(f + S_NUMBER, n);
    for k in 0..3 {
        let v = cpu.read32(o + POSITION + 4 * k);
        cpu.write32(f + 4 * k, v);
    }
    nested!(cpu, 0x0600_50d4, super::sound::se_set(cpu, SE_SHELL_BURST));
    match cpu.read16(o + SH_KIND_OF) {
        1 => {
            let p = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + SPRITE));
            let a = nested!(cpu, 0x0604_5e30, aim::angle_to(cpu, o, p));
            cpu.write16(f + S_ANGLE, (a as u16).wrapping_add(SPREAD_FROM));
            for _ in 0..SPREAD_SHOTS {
                nested!(cpu, 0x0605_3f48, bullet_set_plain(cpu, f));
                let a = cpu.read16(f + S_ANGLE).wrapping_add(SPREAD_STEP);
                cpu.write16(f + S_ANGLE, a);
            }
        }
        3 => {
            let p = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + SPRITE));
            let a = nested!(cpu, 0x0604_5e30, aim::angle_to(cpu, o, p)) as i16 as i32 as u32;
            for (field, v) in RING_FIELDS {
                cpu.write16(f + field, v);
            }
            let s = cpu.call(SL_SIN_L, &[a]) as i32;
            cpu.write32(f + 0x30, (s / 4) as u32);
            let c = cpu.call(SL_COS_L, &[a]) as i32;
            cpu.write32(f + 0x34, (c.wrapping_neg() / 4) as u32);
            nested!(cpu, RING_BURST, burst_set(cpu, f));
        }
        _ => {
            cpu.write32(f + E_SCRIPT, CLOUD_SCRIPT);
            cpu.write16(f + E_COLOUR, CLOUD_COLOUR);
            for k in 0..3 {
                let v = cpu.read32(o + POSITION + 4 * k);
                cpu.write32(f + E_POSITION + 4 * k, v);
            }
            let a = cpu.read16(o + ANGLE);
            cpu.write16(f + E_ANGLE, a);
            cpu.write32(f + E_SPEED, 0);
            cpu.write32(f + E_SCALES, 0x1_0000);
            cpu.write32(f + E_SCALES + 4, 0x1_0000);
            nested!(cpu, 0x0605_7454, actors::drift_effect(cpu, f));
        }
    }
}

/// The burst (an expanding energy ball, or a star): growing by its growth a frame (the growth
/// changing by its acceleration, which changes by its own while above -44), drifting with its
/// speed; its boxes go once it has shrunk below half. A player it touches is pushed straight out
/// (harder at higher difficulty), and then it hits no one. Ended once its size is below zero.
const BURST_EXEC: u32 = 0x0605_4e28;
const BALL_DRAW: u32 = 0x0605_4f64;
const STAR_DRAW: u32 = 0x0605_5154;
const BURST_FLAGS: u32 = 0xc0_0400;
const BURST_LIFT: u8 = 3;
const BURST_BOXES: u32 = 0x0607_2f24;
const BURST_START: u32 = 0x4000;
const SPRITE_DRAW_AT: u32 = 0x34;
/// The burst's fields: hit flags' extra word (the star's 0: it pushes), its growth, acceleration,
/// the acceleration's change (words), its speed (x, y), the ball's second colours.
const BU_PUSHES: u32 = 0x58;
const BU_GROWTH: u32 = 0x8a;
const BU_ACCEL: u32 = 0x8c;
const BU_ACCEL_CHANGE: u32 = 0x8e;
const BU_SPEED: u32 = 0x80;
const BU_ALT: u32 = 0x98;
const BU_ACCEL_LEAST: i16 = -44;
/// What hit it (hits.rs's HIT_OTHER).
const BU_HIT_BY: u32 = 0x40;
const BU_BOXES_GO: i32 = 0x8000;
/// The description: its growth, acceleration (words), speed (x, y), the acceleration's change,
/// and its kind (a word: its low byte set for the ball, 0x100 the ball's second colours).
const BD_GROWTH: u32 = 0x2c;
const BD_ACCEL: u32 = 0x2e;
const BD_SPEED: u32 = 0x30;
const BD_ACCEL_CHANGE: u32 = 0x38;
const BD_KIND: u32 = 0x3a;
const BD_ALT: u16 = 0x100;
/// How hard a player is pushed, by the difficulty (a table; 4096ths of a pixel).
const PUSHES: u32 = 0x0607_2f2c;
const PLAYERS_AT: [u32; 2] = [0x0607_40f0, 0x0607_40f4];
const DIFFICULTY_AT: u32 = 0x0600_ed84;
const SL_ATAN_B: u32 = 0x0606_780c;

/// Makes a burst from the description at `d`. It, or 0.
pub fn burst_set(cpu: &mut Cpu, d: u32) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, MISSILE_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, LASER_LIST, blocks, ACTOR_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + SPRITE));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + BOXES));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + BEHAVIOUR, BURST_EXEC);
    let n = cpu.read8(d + S_NUMBER);
    cpu.write8(o + NUMBER, n);
    cpu.write32(o + FLAGS, BURST_FLAGS);
    if cpu.read16(d + BD_KIND) & 0xff != 0 {
        cpu.write32(o + SPRITE_DRAW_AT, BALL_DRAW);
    } else {
        cpu.write32(o + SPRITE_DRAW_AT, STAR_DRAW);
        cpu.write16(o + BU_PUSHES, 0);
    }
    let alt = (cpu.read16(d + BD_KIND) & BD_ALT != 0) as u16;
    cpu.write16(o + BU_ALT, alt);
    for k in 0..3 {
        let v = cpu.read32(d + 4 * k);
        cpu.write32(o + POSITION + 4 * k, v);
    }
    cpu.write8(o + LIFT, BURST_LIFT);
    cpu.write32(o + BOXES, BURST_BOXES);
    cpu.write8(o + SIDE, BULLET_SIDE);
    cpu.write8(o + TYPE, BULLET_TYPE);
    for (field, from) in [(BU_GROWTH, BD_GROWTH), (BU_ACCEL, BD_ACCEL), (BU_ACCEL_CHANGE, BD_ACCEL_CHANGE)] {
        let v = cpu.read16(d + from);
        cpu.write16(o + field, v);
    }
    for k in 0..2 {
        let v = cpu.read32(d + BD_SPEED + 4 * k);
        cpu.write32(o + BU_SPEED + 4 * k, v);
    }
    cpu.write32(o + SCALES, BURST_START);
    cpu.write32(o + SCALES + 4, BURST_START);
    o
}

/// The burst's behaviour (see burst_set).
pub fn burst_exec(cpu: &mut Cpu, o: u32) {
    if (cpu.read32(o + SCALES) as i32) < 0 {
        cpu.write8(o + OBJ_MODE, ENDING);
        return;
    }
    if cpu.read16(o + BU_PUSHES) == 0 && cpu.read16(o + HIT_FLAGS) & HIT_SOMETHING != 0 {
        let by = cpu.read32(o + BU_HIT_BY);
        if PLAYERS_AT.iter().any(|&at| cpu.read32(at) == by) {
            let dx = cpu.read32(by + POSITION).wrapping_sub(cpu.read32(o + POSITION));
            let dy = cpu.read32(by + POSITION + 4).wrapping_sub(cpu.read32(o + POSITION + 4));
            let a = cpu.call(SL_ATAN_B, &[dx, dy]);
            let out = (a as u16).wrapping_add(0xc000) as i16 as i32 as u32;
            let d = cpu.read8(DIFFICULTY_AT) as i8 as i32 as u32;
            let push = (cpu.read8(PUSHES.wrapping_add(d)) as u32) << 12;
            let s = cpu.call(SL_SIN_L, &[out]);
            let m = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, push));
            let x = cpu.read32(by + POSITION).wrapping_add(m);
            cpu.write32(by + POSITION, x);
            let c = cpu.call(SL_COS_L, &[out]);
            let m = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, c, push));
            let y = cpu.read32(by + POSITION + 4).wrapping_sub(m);
            cpu.write32(by + POSITION + 4, y);
        }
        cpu.write32(o + 0x48, u32::MAX);
        cpu.write32(o + 0x4c, u32::MAX);
    }
    let size = cpu.read32(o + SCALES).wrapping_add(cpu.read16(o + BU_GROWTH) as i16 as i32 as u32);
    cpu.write32(o + SCALES, size);
    cpu.write32(o + SCALES + 4, size);
    let g = cpu.read16(o + BU_GROWTH).wrapping_add(cpu.read16(o + BU_ACCEL));
    cpu.write16(o + BU_GROWTH, g);
    let a = cpu.read16(o + BU_ACCEL);
    if a as i16 > BU_ACCEL_LEAST {
        let a = cpu.read16(o + BU_ACCEL_CHANGE).wrapping_add(a);
        cpu.write16(o + BU_ACCEL, a);
    }
    if (cpu.read16(o + BU_GROWTH) as i16) < 0 && (cpu.read32(o + SCALES) as i32) < BU_BOXES_GO {
        cpu.write32(o + BOXES, 0);
    }
    for k in 0..2 {
        let p = o + POSITION + 4 * k;
        let v = cpu.read32(p).wrapping_add(cpu.read32(o + BU_SPEED + 4 * k));
        cpu.write32(p, v);
    }
}

/// The sprite struct's fields the draws fill: flags, place, scales, colour, kind, frame, angle,
/// lift.
const SS_PLACE: u32 = 4;
const SS_SCALES: u32 = 0x10;
const SS_COLOUR: u32 = 0x18;
const SS_KIND: u32 = 0x1e;
const SS_FRAME: u32 = 0x1f;
const SS_ANGLE: u32 = 0x20;
const SS_LIFT: u32 = 0x23;
const SS_ADDITIVE: u32 = 0x2000_0000;
const SS_ATTR: u32 = 0x100;
const SS_MIRRORED: u32 = 0x10;

fn burst_char(cpu: &mut Cpu, colour: u16) -> u16 {
    cpu.read16(CHAR_BASE).wrapping_add(colour)
}

/// The ball drawn (its sprite's draw; `s` its sprite): a glow (white now and then), two sparks at
/// random angles and frames half again as wide, and a core, the frames alternating.
pub fn burst_ball_draw(cpu: &mut Cpu, s: u32) {
    let f = cpu.stack();
    let o = s.wrapping_sub(SPRITE);
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, f));
    cpu.write8(f + SS_KIND, 0);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    let alt = cpu.read16(o + BU_ALT) != 0;
    let colour = if r & 3 != 0 { 0x1f0 } else if alt { 0x50 } else { 0x70 };
    let colour = burst_char(cpu, colour);
    cpu.write16(f + SS_COLOUR, colour);
    let lift = cpu.read8(o + LIFT);
    cpu.write8(f + SS_LIFT, lift);
    for k in 0..5 {
        let v = cpu.read32(o + POSITION + 4 * k);
        cpu.write32(f + SS_PLACE + 4 * k, v);
    }
    let odd = cpu.read32(LOOPS) & 1 != 0;
    if odd {
        cpu.write8(f + SS_FRAME, 0xd4);
    } else {
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
        cpu.write32(f, r & SS_ATTR);
        cpu.write8(f + SS_FRAME, 2);
    }
    let flags = cpu.read32(f) | SS_ADDITIVE;
    cpu.write32(f, flags);
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, f));
    cpu.write32(f, SS_ADDITIVE);
    let colour = match (alt, cpu.read32(LOOPS) & 1 != 0) {
        (false, true) => 0x24,
        (false, false) => 0x74,
        (true, true) => 4,
        (true, false) => 0x54,
    };
    let colour = burst_char(cpu, colour);
    cpu.write16(f + SS_COLOUR, colour);
    for _ in 0..2 {
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
        cpu.write16(f + SS_ANGLE, r as u16);
        let size = cpu.read32(o + SCALES);
        let w = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, size, 0x1_8000));
        cpu.write32(f + SS_SCALES, w);
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
        let k = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, r, 12));
        cpu.write8(f + SS_FRAME, (k as u8).wrapping_add(77));
        nested!(cpu, 0x0604_0960, sprites::draw(cpu, f));
    }
    if cpu.read32(LOOPS) & 1 != 0 {
        let colour = burst_char(cpu, 0xe0);
        cpu.write16(f + SS_COLOUR, colour);
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
        let w = (cpu.read32(o + SCALES).wrapping_add(r & 0x3fff) as i32) / 4;
        cpu.write32(f + SS_SCALES, w as u32);
        cpu.write8(f + SS_FRAME, 0xd6);
    } else {
        let size = cpu.read32(o + SCALES);
        let w = nested!(cpu, 0x0604_5d90, math::random(cpu, size));
        cpu.write32(f + SS_SCALES, w);
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
        cpu.write8(f + SS_FRAME, ((r & 3) as u8).wrapping_neg().wrapping_sub(47));
    }
    let w = cpu.read32(f + SS_SCALES);
    cpu.write32(f + SS_SCALES + 4, w);
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, f));
}

/// The star drawn (its sprite's draw; `s` its sprite): four rays 24 times its size out, turning
/// with the frame count, their frames running, and a core, the frames alternating.
pub fn burst_star_draw(cpu: &mut Cpu, s: u32) {
    let f = cpu.stack();
    let o = s.wrapping_sub(SPRITE);
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, f));
    cpu.write8(f + SS_KIND, 0);
    let lift = cpu.read8(s + SS_LIFT);
    cpu.write8(f + SS_LIFT, lift);
    let z = cpu.read32(o + POSITION + 8);
    cpu.write32(f + SS_PLACE + 8, z);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write32(f, r & SS_MIRRORED | SS_ADDITIVE);
    let colour = burst_char(cpu, 0x20);
    cpu.write16(f + SS_COLOUR, colour);
    let size = cpu.read32(o + SCALES);
    cpu.write32(f + SS_SCALES, size);
    let size = cpu.read32(o + SCALES);
    cpu.write32(f + SS_SCALES + 4, size);
    let loops = cpu.read32(LOOPS);
    cpu.write16(f + SS_ANGLE, (((loops & 1) << 13).wrapping_sub(loops << 8)) as u16);
    for k in 0..4u32 {
        let t = (cpu.read32(LOOPS).wrapping_sub(4 * k) as i32).wrapping_abs() as u32;
        let q = nested!(cpu, 0x0600_6a78, math::quotient(cpu, t, 4));
        let m = nested!(cpu, 0x0600_6a6e, math::remainder(cpu, q, 8));
        cpu.write8(f + SS_FRAME, 106u8.wrapping_sub(m as u8));
        let len = cpu.read32(o + SCALES).wrapping_mul(24);
        let a = cpu.read16(f + SS_ANGLE).wrapping_add(0xa000) as i16 as i32 as u32;
        let sn = cpu.call(SL_SIN_L, &[a]);
        let dx = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, sn, len));
        let x = cpu.read32(o + POSITION).wrapping_add(dx);
        cpu.write32(f + SS_PLACE, x);
        let len = cpu.read32(o + SCALES).wrapping_mul(24);
        let a = cpu.read16(f + SS_ANGLE).wrapping_add(0xa000) as i16 as i32 as u32;
        let cs = cpu.call(SL_COS_L, &[a]);
        let dy = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, cs, len));
        let y = cpu.read32(o + POSITION + 4).wrapping_sub(dy);
        cpu.write32(f + SS_PLACE + 4, y);
        nested!(cpu, 0x0604_0960, sprites::draw(cpu, f));
        let a = cpu.read16(f + SS_ANGLE).wrapping_add(0x4000);
        cpu.write16(f + SS_ANGLE, a);
    }
    for k in 0..2 {
        let v = cpu.read32(o + POSITION + 4 * k);
        cpu.write32(f + SS_PLACE + 4 * k, v);
    }
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write8(f + SS_FRAME, ((r & 1) as u8).wrapping_sub(45));
    if cpu.read32(LOOPS) & 1 != 0 {
        cpu.write32(f, SS_ATTR);
        let colour = burst_char(cpu, 0x20);
        cpu.write16(f + SS_COLOUR, colour);
        let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
        let w = cpu.read32(o + SCALES).wrapping_add(r & 0x3fff);
        cpu.write32(f + SS_SCALES, w);
    } else {
        let colour = burst_char(cpu, 0xc0);
        cpu.write16(f + SS_COLOUR, colour);
        let size = cpu.read32(o + SCALES) as i32;
        cpu.write32(f + SS_SCALES, (size / 2) as u32);
    }
    let flags = cpu.read32(f) | SS_ADDITIVE;
    cpu.write32(f, flags);
    let w = cpu.read32(f + SS_SCALES);
    cpu.write32(f + SS_SCALES + 4, w);
    nested!(cpu, 0x0604_0960, sprites::draw(cpu, f));
}

/// The heavy bullet: a shootable enemy (2 life) flying straight at its speed along its angle,
/// leaving a puff of smoke as it is fired (from the description: at random, 8 units nearer).
/// On the ground, or hit, it bursts (a puff thrown back); off screen it ends (its behaviour is
/// shootable_bullet).
const HEAVY_EXEC: u32 = 0x0605_3f70;
const HEAVY_FRAME: u8 = 0x10;
const HEAVY_BOXES: u32 = 0x0607_2eec;
const HEAVY_LIFE: u32 = 2;
const HEAVY_SPEED: u32 = 0x5_0000;
const HEAVY_PUFF: (u32, u16) = (0x0607_310c, 0xd0);
const HEAVY_PUFF_NEARER: u32 = 0x8_0000;
const HEAVY_PUFF_SCALE: u32 = 0x5000;
const HEAVY_PUFF_SPEED: u32 = 0xffff_4000;
const HEAVY_PUFF_ATTR: u32 = 0x8000;

/// Makes a heavy bullet from the shot description at `d` (its speed at +0x30). It, or 0.
pub fn heavy_set(cpu: &mut Cpu, d: u32) -> u32 {
    let blocks = nested!(cpu, 0x0600_57f8, objects::obj_size(cpu, MISSILE_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, objects::obj_set(cpu, LASER_LIST, blocks, ACTOR_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, 0x0604_6264, sprites::sprite_init(cpu, o + SPRITE));
    nested!(cpu, 0x0604_6284, hits::hit_init(cpu, o + BOXES));
    nested!(cpu, 0x0605_681c, actors::enemy_clear(cpu, o));
    cpu.write32(o + BEHAVIOUR, HEAVY_EXEC);
    let n = cpu.read8(d + S_NUMBER);
    cpu.write8(o + NUMBER, n);
    cpu.write32(o + FLAGS, MISSILE_FLAGS);
    for k in 0..3 {
        let v = cpu.read32(d + 4 * k);
        cpu.write32(o + POSITION + 4 * k, v);
    }
    cpu.write8(o + LIFT, MISSILE_LIFT);
    cpu.write8(o + FRAME, HEAVY_FRAME);
    cpu.write8(o + KIND, MISSILE_KIND);
    let colour = cpu.read16(MISSILE_COLOUR).wrapping_add(16);
    cpu.write16(o + OWN_COLOUR, colour);
    cpu.write16(o + COLOUR, colour);
    cpu.write32(o + BOXES, HEAVY_BOXES);
    cpu.write8(o + SIDE, BULLET_SIDE);
    cpu.write32(o + LIFE, HEAVY_LIFE);
    let a = cpu.read16(d + S_ANGLE) as i16 as i32 as u32;
    let speed = cpu.read32(d + S_SPEED);
    let s = cpu.call(SL_SIN_L, &[a]);
    let vx = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, s, speed));
    cpu.write32(o + MS_VX, vx);
    let speed = cpu.read32(d + S_SPEED);
    let c = cpu.call(SL_COS_L, &[a]) as i32;
    let vy = nested!(cpu, 0x0600_6a58, math::fixed_mul(cpu, c.wrapping_neg() as u32, speed));
    cpu.write32(o + MS_VY, vy);
    let a = cpu.read16(d + S_ANGLE);
    cpu.write16(o + ANGLE, a);
    cpu.write32(d + E_SCRIPT, HEAVY_PUFF.0);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write16(d + E_ANGLE, r as u16);
    let z = cpu.read32(d + 8).wrapping_sub(HEAVY_PUFF_NEARER);
    cpu.write32(d + 8, z);
    let r = nested!(cpu, 0x0604_5d90, math::random(cpu, 0x1_0000));
    cpu.write16(d + E_COLOUR, (r & HEAVY_PUFF_ATTR) as u16 | HEAVY_PUFF.1);
    cpu.write32(d + E_SCALES, HEAVY_PUFF_SCALE);
    cpu.write32(d + E_SCALES + 4, HEAVY_PUFF_SCALE);
    cpu.write32(d + E_SPEED, HEAVY_PUFF_SPEED);
    nested!(cpu, 0x0605_7454, actors::drift_effect(cpu, d));
    o
}

/// A heavy bullet at five pixels a frame.
pub fn heavy_bullet(cpu: &mut Cpu, d: u32) -> u32 {
    cpu.write32(d + S_SPEED, HEAVY_SPEED);
    nested!(cpu, 0x0605_4050, heavy_set(cpu, d))
}
