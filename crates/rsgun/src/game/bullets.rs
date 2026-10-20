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
const AS_POSITION: u32 = 0x00;
const AS_FLAGS: u32 = 0x14;
const AS_ANGLE: u32 = 0x18;
const AS_SPEED: u32 = 0x30;
const AS_SHOOTER: u32 = 0x3c;
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

