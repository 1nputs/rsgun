//! Effects: plain actors (actors.rs's plain_exec, effect_exec's behaviour) made from a struct the
//! caller fills on its stack: where (or, placed relative, where from), the scales, the sprite's
//! kind, frame and colour, the player it belongs to, how long it lasts, what it is linked to.

use super::nested;
use super::{objects, sprites, Cpu};

/// The struct: x, y, z (16.16), the scales, a long whose low byte is the flags (FX_*), the angle (a
/// word), the sprite's kind and frame, the player (0, 1; -1 none), the colour (a word), the frames
/// it lasts (a word) and a byte with them, what it is linked to, a long more for the relative.
const X: u32 = 0;
const SCALE_X: u32 = 0xc;
const SCALE_Y: u32 = 0x10;
const FLAGS: u32 = 0x17;
const ANGLE: u32 = 0x18;
const KIND: u32 = 0x1c;
const FRAME: u32 = 0x1d;
const PLAYER: u32 = 0x1e;
const COLOUR: u32 = 0x20;
const FRAMES: u32 = 0x24;
const FRAMES_2: u32 = 0x26;
const LINK: u32 = 0x28;
const RELATIVE_3C: u32 = 0x3c;
/// The flags: placed relative to something (x, y and the angle kept apart), linked.
const RELATIVE: u8 = 0x80;
const LINKED: u8 = 1;
/// The player byte for an effect of nobody's that may take list 2.
const NOBODY: i8 = -1;

/// The effect actor's fields: its sprite (kind, frame, colour, angle, position, scales), its player
/// and the byte after it, its flags (copied), two counts, the relative place (x, y, angle, a long),
/// two longs, a word, the link (12 bytes) and its frame.
const A_KIND: u32 = 0x2e;
const A_FRAME: u32 = 0x2f;
const A_COLOUR: u32 = 0x28;
const A_ANGLE: u32 = 0x30;
const A_POSITION: u32 = 0x14;
const A_SCALES: u32 = 0x20;
const A_PLAYER: u32 = 0x32;
const A_33: u32 = 0x33;
const A_FLAGS: u32 = 0x3d;
const A_COUNT_B: u32 = 0x3e;
const A_COUNT_W: u32 = 0x40;
const A_42: u32 = 0x42;
const A_REL: u32 = 0x44;
const A_REL_ANGLE: u32 = 0x4c;
const A_50: u32 = 0x50;
const A_54: u32 = 0x54;
const A_REL_3C: u32 = 0x5c;
const A_LINK: u32 = 0x70;
const A_LINK_FRAME: u32 = 0x7d;
const A_BEHAVIOUR: u32 = 0x38;
/// The byte after the player: a player's effects 4, others' -8 (not known yet).
const PLAYERS_33: u8 = 4;
const OTHERS_33: u8 = 0xf8;

const EFFECT_SIZE: u32 = 0x80;
const PLAIN_EXEC: u32 = 0x0604_dcb4;
/// The effects' behaviour (effect_exec).
const EFFECT_EXEC: u32 = 0x0604_deb4;
/// The rewrites' SH-2 entries (for nested!).
const SPRITE_INIT: u32 = 0x0604_6264;
const ACTOR_INIT: u32 = 0x0604_dd50;
const LINK_INIT: u32 = 0x0604_6530;
const OBJ_SIZE: u32 = 0x0600_57f8;
const OBJ_SET: u32 = 0x0600_6b00;

/// An actor's start: its sprite set up, three bytes and a word cleared (+0x3c-0x3e, +0x40).
pub fn actor_init(cpu: &mut Cpu, actor: u32) {
    nested!(cpu, SPRITE_INIT, sprites::sprite_init(cpu, actor + 0x10));
    for k in 0..3 {
        cpu.write8(actor + 0x3c + k, 0);
    }
    cpu.write16(actor + 0x40, 0);
}

/// A link (12 bytes): what it links to, a long cleared, a scale of one, three bytes cleared.
pub fn link_init(cpu: &mut Cpu, link: u32, to: u32) {
    cpu.write32(link, to);
    cpu.write32(link + 4, 0);
    cpu.write32(link + 8, 0x1_0000);
    for k in [0xe, 0xd, 0xc] {
        cpu.write8(link + k, 0);
    }
}

/// An effect object in list `area`: it, or 0.
fn effect_in(cpu: &mut Cpu, area: u32) -> u32 {
    let blocks = nested!(cpu, OBJ_SIZE, objects::obj_size(cpu, EFFECT_SIZE)) as i8 as i32 as u32;
    nested!(cpu, OBJ_SET, objects::obj_set(cpu, area, blocks, PLAIN_EXEC))
}

/// Makes an effect from the struct at `fx`: in list 1 for a player's, else (or if 1 is full) list
/// 3, and for nobody's list 2 last: it, or 0.
pub fn effect_set(cpu: &mut Cpu, fx: u32) -> u32 {
    let mut obj = 0;
    if cpu.read8(fx + PLAYER) <= 1 {
        obj = effect_in(cpu, 1);
    }
    if obj == 0 {
        obj = effect_in(cpu, 3);
        if obj == 0 && cpu.read8(fx + PLAYER) as i8 == NOBODY {
            obj = effect_in(cpu, 2);
        }
    }
    if obj == 0 {
        return 0;
    }
    nested!(cpu, ACTOR_INIT, actor_init(cpu, obj));
    cpu.write32(obj + A_BEHAVIOUR, EFFECT_EXEC);
    let v = cpu.read8(fx + KIND);
    cpu.write8(obj + A_KIND, v);
    let v = cpu.read8(fx + FRAME);
    cpu.write8(obj + A_FRAME, v);
    let v = cpu.read16(fx + COLOUR);
    cpu.write16(obj + A_COLOUR, v);
    let p = cpu.read8(fx + PLAYER);
    cpu.write8(obj + A_PLAYER, p);
    cpu.write8(obj + A_33, if p as i8 > 1 { OTHERS_33 } else { PLAYERS_33 });
    let flags = cpu.read8(fx + FLAGS);
    cpu.write8(obj + A_FLAGS, flags);
    if flags & RELATIVE != 0 {
        for k in 0..2 {
            let v = cpu.read32(fx + X + 4 * k);
            cpu.write32(obj + A_REL + 4 * k, v);
        }
        let a = cpu.read16(fx + ANGLE) as i16 as i32 as u32;
        cpu.write32(obj + A_REL_ANGLE, a);
        cpu.write32(obj + A_50, 0);
        let v = cpu.read32(fx + RELATIVE_3C);
        cpu.write32(obj + A_REL_3C, v);
    } else {
        for k in 0..3 {
            let v = cpu.read32(fx + X + 4 * k);
            cpu.write32(obj + A_POSITION + 4 * k, v);
        }
        let a = cpu.read16(fx + ANGLE);
        cpu.write16(obj + A_ANGLE, a);
        for field in [A_50, A_REL_ANGLE, A_REL + 4, A_REL, A_REL_3C] {
            cpu.write32(obj + field, 0);
        }
    }
    let sx = cpu.read32(fx + SCALE_X);
    cpu.write32(obj + A_SCALES, sx);
    let sy = cpu.read32(fx + SCALE_Y);
    cpu.write32(obj + A_SCALES + 4, sy);
    let frames = cpu.read16(fx + FRAMES);
    if frames != 0 {
        cpu.write16(obj + A_COUNT_W, frames.wrapping_add(1));
        let b = cpu.read8(fx + FRAMES_2);
        if b != 0 {
            cpu.write8(obj + A_COUNT_B, b.wrapping_add(1));
        }
    }
    cpu.write32(obj + A_54 + 4, 0);
    cpu.write32(obj + A_54, 0);
    cpu.write16(obj + A_42, 0);
    if cpu.read8(obj + A_FLAGS) & LINKED != 0 {
        let to = cpu.read32(fx + LINK);
        nested!(cpu, LINK_INIT, link_init(cpu, obj + A_LINK, to));
        let f = cpu.read8(fx + FRAME);
        cpu.write8(obj + A_LINK_FRAME, f);
    }
    obj
}

/// A link's fields: its script (or, with LINK_TABLE, a table of scripts by LINK_WHICH), its phase
/// (16.16: a whole step moves a frame), its speed, its flags, which script, the frame it is at.
/// A script: its last frame's index (bit 7: each frame names the sprite's kind too), then for each
/// frame (kind,) the sprite's frame and the steps it lasts.
const LINK_PHASE: u32 = 4;
const LINK_SPEED: u32 = 8;
const LINK_FLAGS: u32 = 0xc;
const LINK_WHICH: u32 = 0xd;
const LINK_AT: u32 = 0xe;
/// The flags: scripts in a table, paused, played backwards, hold at the end (paused and ended
/// both), ended.
const LINK_TABLE: u8 = 0x80;
const LINK_PAUSED: u8 = 0x10;
const LINK_BACKWARDS: u8 = 0x20;
const LINK_HOLD: u8 = 0x40;
const LINK_HELD: u8 = 0x11;
const LINK_ENDED: u8 = 2;
const WITH_KIND: u8 = 0x80;
/// A sprite's kind and frame (sprites.rs's struct).
const SPRITE_KIND: u32 = 0x1e;
const SPRITE_FRAME: u32 = 0x1f;

/// One step of the animation `link` plays on the sprite at `s`: the frame (and kind) set, the
/// phase moved by the speed (unless paused); a whole step past the frame's time moves to the next
/// frame (or the one before, played backwards), and at the end either holds or starts again
/// (flagged ended).
pub fn link_step(cpu: &mut Cpu, s: u32, link: u32) {
    let mut script = cpu.read32(link);
    if script == 0 {
        return;
    }
    if cpu.read8(link + LINK_FLAGS) & LINK_TABLE != 0 {
        let which = cpu.read8(link + LINK_WHICH) as i8 as i32;
        script = cpu.read32(script.wrapping_add((4 * which) as u32));
    }
    let mut last = cpu.read8(script);
    let at = cpu.read8(link + LINK_AT) as i8 as i32;
    let mut entry = script + 1;
    if last & WITH_KIND != 0 {
        last &= !WITH_KIND;
        entry = entry.wrapping_add((3 * at) as u32);
        let kind = cpu.read8(entry);
        cpu.write8(s + SPRITE_KIND, kind);
        entry += 1;
    } else {
        entry = entry.wrapping_add((2 * at) as u32);
    }
    let frame = cpu.read8(entry);
    cpu.write8(s + SPRITE_FRAME, frame);
    entry += 1;
    let flags = cpu.read8(link + LINK_FLAGS);
    if flags & LINK_PAUSED == 0 {
        let phase = cpu.read32(link + LINK_PHASE);
        let speed = cpu.read32(link + LINK_SPEED);
        let phase = if flags & LINK_BACKWARDS != 0 { phase.wrapping_sub(speed) } else { phase.wrapping_add(speed) };
        cpu.write32(link + LINK_PHASE, phase);
    }
    let phase = cpu.read32(link + LINK_PHASE) as i32;
    let whole = ((phase / 0x1_0000) as i16 as i32).abs();
    if whole < cpu.read8(entry) as i32 {
        return;
    }
    let last = last as i32;
    if phase >= 0 {
        let at = cpu.read8(link + LINK_AT) as i8 as i32;
        if at < last {
            cpu.write8(link + LINK_AT, (at + 1) as u8);
        } else {
            let flags = cpu.read8(link + LINK_FLAGS);
            if flags & LINK_HOLD != 0 {
                cpu.write8(link + LINK_FLAGS, flags | LINK_HELD);
            } else {
                cpu.write8(link + LINK_AT, 0);
                let flags = cpu.read8(link + LINK_FLAGS);
                cpu.write8(link + LINK_FLAGS, flags | LINK_ENDED);
            }
        }
    } else {
        let at = cpu.read8(link + LINK_AT) as i8 as i32;
        if at <= 0 {
            cpu.write8(link + LINK_AT, last as u8);
        } else {
            let at = at - 1;
            cpu.write8(link + LINK_AT, at as u8);
            if at == 0 {
                let flags = cpu.read8(link + LINK_FLAGS);
                let flags = if flags & LINK_HOLD != 0 { flags | LINK_HELD } else { flags | LINK_ENDED };
                cpu.write8(link + LINK_FLAGS, flags);
            }
        }
    }
    cpu.write16(link + LINK_PHASE, 0);
}

/// The effect's state (0 new, 1 running), its flags (the struct's, A_FLAGS: also SLOWS, LOOPS,
/// PARENT_SCALES), the frames its scales shrink over at the end, its frames left, its speed (or,
/// relative, its offset), its slowing (or relative angle), its scales' speed, its parent (relative).
const A_STATE: u32 = 0x3c;
const A_SPEED: u32 = 0x44;
const A_SLOW: u32 = 0x4c;
const A_SCALE_SPEED: u32 = 0x54;
const A_PARENT: u32 = 0x5c;
const A_SPIN: u32 = 0x42;
const A_LINK_FLAGS: u32 = 0x7c;
/// The flags: slowing to a stop over its frames (or, relative, closing in), playing its link until
/// the link ends, the parent's scales taken.
const SLOWS: u8 = 8;
const LOOPS: u8 = 2;
const PARENT_SCALES: u8 = 0x40;
/// The C library's signed division (clib.rs).
const DIVIDE: u32 = 0x0606_62d8;
const TURN_POINT: u32 = 0x0606_4eac;
const QUOTIENT: u32 = 0x0600_6a78;
const APPROACH: u32 = 0x0604_6168;
const LINK_STEP: u32 = 0x0604_6558;
const OBJ_MODE: u32 = 3;
const ENDING: u8 = 0x70;

fn end(cpu: &mut Cpu, obj: u32) {
    cpu.write8(obj + OBJ_MODE, ENDING);
}

/// An effect's behaviour: started (a slowing effect takes the slowing that stops it at its last
/// frame), it ends with its parent or (looping) its link; its frames counted down (its scales
/// shrinking over the last ones), its scales moved (ended at none), then moved (by its speed, or
/// placed by its parent: its offset turned with the parent), turned, its link played.
pub fn effect_exec(cpu: &mut Cpu, obj: u32) {
    effect_frame(cpu, obj, Variant::Linked);
}

/// A polygon effect's behaviour (polygon_set's): effect_exec's without loops or a link, its
/// colour cycled by its flags (COLOUR_*); both its scales move by the x scale's speed (as the SH-2
/// code has it).
pub fn polygon_exec(cpu: &mut Cpu, obj: u32) {
    effect_frame(cpu, obj, Variant::Polygon);
}

/// The two behaviours effect_frame plays.
#[derive(PartialEq)]
enum Variant {
    Linked,
    Polygon,
}

/// The polygon effect's colour (from its own, +0x2a): cycled up through 16, at random among 16,
/// or stepped up once a frame over its last 15 frames.
const A_OWN_COLOUR: u32 = 0x2a;
const COLOUR_MODE: u8 = 0x30;
const COLOUR_CYCLE: u8 = 0x10;
const COLOUR_RANDOM: u8 = 0x20;
const COLOUR_FADE: u8 = 0x30;
const COLOURS: u16 = 16;
const RANDOM: u32 = 0x0604_5d90;

fn effect_frame(cpu: &mut Cpu, obj: u32, variant: Variant) {
    match cpu.read8(obj + A_STATE) {
        0 => {
            let flags = cpu.read8(obj + A_FLAGS);
            if flags & SLOWS != 0 {
                let frames = cpu.read16(obj + A_COUNT_W) as i16 as i32 as u32;
                if frames != 0 && flags & RELATIVE == 0 {
                    for k in 0..2 {
                        if cpu.read32(obj + A_SLOW + 4 * k) == 0 {
                            let speed = cpu.read32(obj + A_SPEED + 4 * k);
                            let q = nested!(cpu, QUOTIENT, super::math::quotient(cpu, speed, frames));
                            cpu.write32(obj + A_SLOW + 4 * k, q.wrapping_neg());
                        }
                    }
                }
            }
            cpu.write8(obj + A_STATE, 1);
        }
        1 => {}
        _ => return,
    }
    let mut parent = 0;
    if cpu.read8(obj + A_FLAGS) & RELATIVE != 0 {
        parent = cpu.read32(obj + A_PARENT);
        if cpu.read8(parent + OBJ_MODE) as i8 >= ENDING as i8 {
            end(cpu, obj);
            return;
        }
    }
    if variant == Variant::Linked && cpu.read8(obj + A_FLAGS) & LOOPS != 0 && cpu.read8(obj + A_LINK_FLAGS) & 3 != 0 && cpu.read16(obj + A_COUNT_W) == 0 {
        end(cpu, obj);
        return;
    }
    let frames = cpu.read16(obj + A_COUNT_W) as i16 as i32;
    if frames != 0 {
        let shrink = cpu.read8(obj + A_COUNT_B) as i8 as i32;
        if shrink > 0 && frames <= shrink {
            for k in 0..2 {
                let n = cpu.read8(obj + A_COUNT_B) as i8 as i32 as u32;
                let scale = cpu.read32(obj + A_SCALES + 4 * k);
                let step = nested!(cpu, DIVIDE, super::clib::divide(cpu, scale, n));
                cpu.write32(obj + A_SCALE_SPEED + 4 * k, step.wrapping_neg());
            }
            cpu.write8(obj + A_COUNT_B, 0xff);
        }
        let left = cpu.read16(obj + A_COUNT_W).wrapping_sub(1);
        cpu.write16(obj + A_COUNT_W, left);
        if left == 0 {
            if variant == Variant::Linked && cpu.read8(obj + A_FLAGS) & LOOPS != 0 {
                let f = cpu.read8(obj + A_LINK_FLAGS);
                cpu.write8(obj + A_LINK_FLAGS, f & !2);
            } else {
                end(cpu, obj);
            }
        }
    }
    if variant == Variant::Polygon {
        colour_step(cpu, obj);
    }
    let sx = cpu.read32(obj + A_SCALES).wrapping_add(cpu.read32(obj + A_SCALE_SPEED));
    cpu.write32(obj + A_SCALES, sx);
    let y_speed = if variant == Variant::Linked { A_SCALE_SPEED + 4 } else { A_SCALE_SPEED };
    let sy = cpu.read32(obj + A_SCALES + 4).wrapping_add(cpu.read32(obj + y_speed));
    cpu.write32(obj + A_SCALES + 4, sy);
    if cpu.read32(obj + A_SCALES) as i32 <= 0 {
        if sy == 0 {
            end(cpu, obj);
        }
        cpu.write32(obj + A_SCALE_SPEED, 0);
        cpu.write32(obj + A_SCALES, 0);
    }
    if cpu.read32(obj + A_SCALES + 4) as i32 <= 0 {
        if cpu.read32(obj + A_SCALES) == 0 {
            end(cpu, obj);
        }
        cpu.write32(obj + A_SCALE_SPEED + 4, 0);
        cpu.write32(obj + A_SCALES + 4, 0);
    }
    let (angle, spin) = if parent == 0 {
        for k in 0..2 {
            let at = obj + A_SPEED + 4 * k;
            if cpu.read8(obj + A_FLAGS) & SLOWS != 0 {
                let slow = (cpu.read32(obj + A_SLOW + 4 * k) as i32).wrapping_abs() as u32;
                let v = cpu.read32(at);
                let v = nested!(cpu, APPROACH, super::actors::approach(cpu, v, 0, slow));
                cpu.write32(at, v);
            } else {
                let v = cpu.read32(at).wrapping_add(cpu.read32(obj + A_SLOW + 4 * k));
                cpu.write32(at, v);
            }
        }
        for k in 0..2 {
            let p = cpu.read32(obj + A_POSITION + 4 * k).wrapping_add(cpu.read32(obj + A_SPEED + 4 * k));
            cpu.write32(obj + A_POSITION + 4 * k, p);
        }
        (cpu.read16(obj + A_ANGLE), cpu.read16(obj + A_SPIN))
    } else {
        let a = cpu.read32(obj + A_REL_ANGLE).wrapping_add(cpu.read16(obj + A_SPIN) as i16 as i32 as u32);
        cpu.write32(obj + A_REL_ANGLE, a);
        if cpu.read8(obj + A_FLAGS) & SLOWS != 0 {
            for k in 0..2 {
                let at = obj + A_REL + 4 * k;
                let v = cpu.read32(at) as i32;
                cpu.write32(at, v.wrapping_add(v.wrapping_neg() / 16) as u32);
            }
        }
        let t = cpu.stack();
        for k in 0..2 {
            let v = cpu.read32(obj + A_REL + 4 * k);
            cpu.write32(t + 4 * k, v);
        }
        if cpu.read8(obj + A_FLAGS) & PARENT_SCALES != 0 {
            for k in 0..2 {
                let v = cpu.read32(parent + A_SCALES + 4 * k);
                cpu.write32(obj + A_SCALES + 4 * k, v);
            }
        }
        for k in 0..2 {
            let v = cpu.read32(parent + A_SCALES + 4 * k);
            cpu.write32(t + 0xc + 4 * k, v);
        }
        cpu.write32(t + 0x14, 0);
        let turn = cpu.read16(parent + A_ANGLE).wrapping_add(cpu.read16(obj + A_50 + 2));
        cpu.write16(t + 0x18, turn);
        nested!(cpu, TURN_POINT, super::math::turn_point(cpu, t));
        for k in 0..2 {
            let p = cpu.read32(parent + A_POSITION + 4 * k).wrapping_add(cpu.read32(t + 4 * k));
            cpu.write32(obj + A_POSITION + 4 * k, p);
        }
        let z = cpu.read32(parent + A_POSITION + 8);
        cpu.write32(obj + A_POSITION + 8, z);
        (cpu.read16(parent + A_ANGLE), cpu.read16(obj + A_REL_ANGLE + 2))
    };
    cpu.write16(obj + A_ANGLE, angle.wrapping_add(spin));
    if variant == Variant::Linked && cpu.read8(obj + A_FLAGS) & LINKED != 0 {
        nested!(cpu, LINK_STEP, link_step(cpu, obj + 0x10, obj + A_LINK));
    }
}

/// The polygon effect's colour step.
fn colour_step(cpu: &mut Cpu, obj: u32) {
    match cpu.read8(obj + A_FLAGS) & COLOUR_MODE {
        COLOUR_RANDOM => {
            let r = nested!(cpu, RANDOM, super::math::random(cpu, 0)) as u16;
            let colour = (r & (COLOURS - 1)).wrapping_add(cpu.read16(obj + A_OWN_COLOUR));
            cpu.write16(obj + A_COLOUR, colour);
        }
        COLOUR_CYCLE => {
            let colour = cpu.read16(obj + A_COLOUR).wrapping_add(1);
            cpu.write16(obj + A_COLOUR, colour);
            let own = cpu.read16(obj + A_OWN_COLOUR);
            if colour as u32 >= own as u32 + COLOURS as u32 {
                cpu.write16(obj + A_COLOUR, own);
            }
        }
        COLOUR_FADE => {
            let left = cpu.read16(obj + A_COUNT_W) as i16;
            if left != 0 && left < COLOURS as i16 {
                let colour = cpu.read16(obj + A_COLOUR).wrapping_add(1);
                cpu.write16(obj + A_COLOUR, colour);
            }
        }
        _ => {}
    }
}

/// A polygon effect's flags (the struct's, kept at A_FLAGS): the draw's polygon bits (sprites.rs's
/// POLYGON_BITS; with SPRITE_POLYGON the struct's kind and frame textured on it, and no colour
/// cycling).
const POLYGON_BITS: u8 = 7;
const SPRITE_POLYGON: u8 = 4;
/// Its corners (sprites.rs's CORNERS) are its own, at +0x70 (the maker's caller fills them).
const A_CORNERS_AT: u32 = 0x34;
const A_CORNERS: u32 = 0x70;
const POLYGON_EXEC: u32 = 0x0604_e3a8;

/// Makes a polygon effect from the struct at `fx` (effect_set's): in list 1 for a player's, else
/// (or if 1 is full) list 3: it, or 0.
pub fn polygon_set(cpu: &mut Cpu, fx: u32) -> u32 {
    let mut obj = 0;
    if cpu.read8(fx + PLAYER) as i8 <= 1 {
        obj = effect_in(cpu, 1);
    }
    if obj == 0 {
        obj = effect_in(cpu, 3);
        if obj == 0 {
            return 0;
        }
    }
    nested!(cpu, ACTOR_INIT, actor_init(cpu, obj));
    cpu.write32(obj + A_BEHAVIOUR, POLYGON_EXEC);
    let colour = cpu.read16(fx + COLOUR);
    cpu.write16(obj + A_COLOUR, colour);
    let p = cpu.read8(fx + PLAYER);
    cpu.write8(obj + A_PLAYER, p);
    cpu.write8(obj + A_33, if p as i8 > 1 { OTHERS_33 } else { PLAYERS_33 });
    cpu.write32(obj + A_CORNERS_AT, obj + A_CORNERS);
    let flags = cpu.read8(fx + FLAGS);
    cpu.write8(obj + A_FLAGS, flags);
    if flags & RELATIVE != 0 {
        for k in 0..2 {
            let v = cpu.read32(fx + X + 4 * k);
            cpu.write32(obj + A_REL + 4 * k, v);
        }
        let a = cpu.read16(fx + ANGLE) as i16 as i32 as u32;
        cpu.write32(obj + A_REL_ANGLE, a);
        cpu.write32(obj + A_50, 0);
        let v = cpu.read32(fx + RELATIVE_3C);
        cpu.write32(obj + A_PARENT, v);
    } else {
        for k in 0..3 {
            let v = cpu.read32(fx + X + 4 * k);
            cpu.write32(obj + A_POSITION + 4 * k, v);
        }
        let a = cpu.read16(fx + ANGLE);
        cpu.write16(obj + A_ANGLE, a);
        for field in [A_50, A_REL_ANGLE, A_REL + 4, A_REL, A_PARENT] {
            cpu.write32(obj + field, 0);
        }
    }
    for k in 0..2 {
        let v = cpu.read32(fx + SCALE_X + 4 * k);
        cpu.write32(obj + A_SCALES + 4 * k, v);
    }
    let flags = cpu.read8(obj + A_FLAGS);
    if flags & SPRITE_POLYGON != 0 {
        let f = cpu.read32(obj + 0x10) | SPRITE_POLYGON as u32;
        cpu.write32(obj + 0x10, f);
        let kind = cpu.read8(fx + KIND);
        cpu.write8(obj + A_KIND, kind);
        let frame = cpu.read8(fx + FRAME);
        cpu.write8(obj + A_FRAME, frame);
        let flags = cpu.read8(obj + A_FLAGS) & !COLOUR_MODE;
        cpu.write8(obj + A_FLAGS, flags);
    } else {
        let f = cpu.read32(obj + 0x10) | (flags & POLYGON_BITS) as u32;
        cpu.write32(obj + 0x10, f);
        let colour = cpu.read16(obj + A_COLOUR);
        cpu.write16(obj + A_OWN_COLOUR, colour);
    }
    let frames = cpu.read16(fx + FRAMES);
    if frames != 0 {
        cpu.write16(obj + A_COUNT_W, frames.wrapping_add(1));
        let b = cpu.read8(fx + FRAMES_2);
        if b != 0 {
            cpu.write8(obj + A_COUNT_B, b.wrapping_add(1));
        }
    }
    cpu.write32(obj + A_SCALE_SPEED + 4, 0);
    cpu.write32(obj + A_SCALE_SPEED, 0);
    cpu.write16(obj + A_SPIN, 0);
    obj
}

/// A growing effect's scales' acceleration (x, y).
const A_SCALE_ACCEL: u32 = 0x60;
const GROW_EXEC: u32 = 0x0604_dd88;
const ACCEL_EXEC: u32 = 0x0604_de5c;

/// A growing effect's behaviour: effect_exec's frame, then (while its shrink count is not
/// negative) its scales' speed slowed by its acceleration down to none; with none left (or a
/// negative count), it goes on as a plain effect.
pub fn grow_exec(cpu: &mut Cpu, obj: u32) {
    nested!(cpu, EFFECT_EXEC, effect_exec(cpu, obj));
    if cpu.read8(obj + A_COUNT_B) as i8 >= 0 {
        for k in 0..2 {
            let v = cpu.read32(obj + A_SCALE_SPEED + 4 * k).wrapping_add(cpu.read32(obj + A_SCALE_ACCEL + 4 * k));
            cpu.write32(obj + A_SCALE_SPEED + 4 * k, v);
        }
        for k in 0..2 {
            if cpu.read32(obj + A_SCALE_SPEED + 4 * k) as i32 <= 0 {
                cpu.write32(obj + A_SCALE_SPEED + 4 * k, 0);
            }
        }
        if cpu.read32(obj + A_SCALE_SPEED) != 0 || cpu.read32(obj + A_SCALE_SPEED + 4) != 0 {
            return;
        }
    }
    cpu.write32(obj + A_BEHAVIOUR, EFFECT_EXEC);
}

/// Makes the effect `obj` grow by `size` over `frames` (its scales' speed starting at twice the
/// mean, slowing to none).
pub fn grow_set(cpu: &mut Cpu, obj: u32, size: u32, frames: u32) {
    // the SH-2 code keeps the effect's address in its frame
    let frame = cpu.stack();
    cpu.write32(frame, obj);
    let speed = nested!(cpu, DIVIDE, super::clib::divide(cpu, size.wrapping_mul(2), frames));
    cpu.write32(obj + A_SCALE_SPEED, speed);
    cpu.write32(obj + A_SCALE_SPEED + 4, speed);
    for k in 0..2 {
        let v = cpu.read32(obj + A_SCALE_SPEED + 4 * k);
        let a = nested!(cpu, DIVIDE, super::clib::divide(cpu, v, frames));
        cpu.write32(obj + A_SCALE_ACCEL + 4 * k, a.wrapping_neg());
    }
    cpu.write32(obj + A_BEHAVIOUR, GROW_EXEC);
}

/// An accelerating effect's behaviour: effect_exec's frame, then its scales' speed changed by its
/// acceleration.
pub fn accel_exec(cpu: &mut Cpu, obj: u32) {
    nested!(cpu, EFFECT_EXEC, effect_exec(cpu, obj));
    for k in 0..2 {
        let v = cpu.read32(obj + A_SCALE_SPEED + 4 * k).wrapping_add(cpu.read32(obj + A_SCALE_ACCEL + 4 * k));
        cpu.write32(obj + A_SCALE_SPEED + 4 * k, v);
    }
}

/// Makes the effect `obj` accelerate its scales (by `ax`, `ay` a frame).
pub fn accel_set(cpu: &mut Cpu, obj: u32, ax: u32, ay: u32) {
    cpu.write32(obj + A_SCALE_ACCEL, ax);
    cpu.write32(obj + A_SCALE_ACCEL + 4, ay);
    cpu.write32(obj + A_BEHAVIOUR, ACCEL_EXEC);
}

/// The frames left counted down: ended at none.
fn count_down(cpu: &mut Cpu, obj: u32) {
    let left = cpu.read16(obj + A_COUNT_W).wrapping_sub(1);
    cpu.write16(obj + A_COUNT_W, left);
    if left == 0 {
        end(cpu, obj);
    }
}

/// A behaviour that only lasts its frames.
pub fn last_exec(cpu: &mut Cpu, obj: u32) {
    count_down(cpu, obj);
}

/// One that widens by its x scale's speed while it lasts.
pub fn widen_exec(cpu: &mut Cpu, obj: u32) {
    let v = cpu.read32(obj + A_SCALES).wrapping_add(cpu.read32(obj + A_SCALE_SPEED));
    cpu.write32(obj + A_SCALES, v);
    count_down(cpu, obj);
}

/// One that lengthens by its y scale's speed while it lasts.
pub fn lengthen_exec(cpu: &mut Cpu, obj: u32) {
    let v = cpu.read32(obj + A_SCALES + 4).wrapping_add(cpu.read32(obj + A_SCALE_SPEED + 4));
    cpu.write32(obj + A_SCALES + 4, v);
    count_down(cpu, obj);
}

/// lengthen_exec's, starting again from none past 0x39000.
const LENGTH_WRAP: i32 = 0x39000;

pub fn lengthen_wrap_exec(cpu: &mut Cpu, obj: u32) {
    let v = cpu.read32(obj + A_SCALES + 4).wrapping_add(cpu.read32(obj + A_SCALE_SPEED + 4));
    cpu.write32(obj + A_SCALES + 4, v);
    if v as i32 > LENGTH_WRAP {
        cpu.write32(obj + A_SCALES + 4, 0);
    }
    count_down(cpu, obj);
}

/// One that falls by its y speed while it lasts.
pub fn fall_exec(cpu: &mut Cpu, obj: u32) {
    let v = cpu.read32(obj + A_POSITION + 4).wrapping_add(cpu.read32(obj + A_SPEED + 4));
    cpu.write32(obj + A_POSITION + 4, v);
    count_down(cpu, obj);
}

/// A score popup (its caller's struct, effect_set's x, y, z): the score (+0x14), the multiplier
/// shown after it (a word: 1-3, else none), and the fields each digit is made from: the digit (or
/// glyph) and the player (bytes), the frames before it shows (a word; its low byte taken).
const P_SCORE: u32 = 0x14;
const P_MULTIPLIER: u32 = 0x1a;
const P_DIGIT: u32 = 0x1d;
const P_PLAYER: u32 = 0x1e;
const P_DELAY: u32 = 0x24;
const P_DELAY_LOW: u32 = 0x25;
/// The multipliers' glyphs and widths (digit widths), by the multiplier 1, 2, 3.
const MULTIPLIERS: [(u8, i32); 3] = [(0x12, 6), (0x10, 5), (0x11, 6)];
/// A digit is 8 pixels wide; the popup kept within the screen (x within 0xa0 less half its width,
/// y within 0x60, and not above -0x4c).
const DIGIT_W: u32 = 0x8_0000;
const HALF_DIGIT_W: i32 = 4;
const SCREEN_X: i32 = 0xa0;
const SCREEN_Y: u32 = 0x60_0000;
const TOP: i32 = -0x4c_0000;
const CLAMP: u32 = 0x0606_4f3c;
const REMAINDER: u32 = 0x0600_6a6e;
const DIGIT_SET: u32 = 0x0604_e948;

/// Pops up the score at `p`: its digits from the last (rightmost) leftward, each a frame group
/// later than the one before, then the multiplier's glyph left of them.
pub fn popup(cpu: &mut Cpu, p: u32) {
    let mut digits = 0;
    let mut v = cpu.read32(p + P_SCORE) as i32;
    while v > 0 {
        v /= 10;
        digits += 1;
    }
    let (glyph, extra) = match cpu.read16(p + P_MULTIPLIER) as i16 {
        m @ 1..=3 => MULTIPLIERS[m as usize - 1],
        _ => (0, 0),
    };
    let width = digits + extra;
    let x = cpu.read32(p);
    let x = nested!(cpu, CLAMP, super::math::clamp(cpu, x, ((SCREEN_X - width * HALF_DIGIT_W) << 16) as u32));
    cpu.write32(p, x);
    let y = cpu.read32(p + 4);
    let y = nested!(cpu, CLAMP, super::math::clamp(cpu, y, SCREEN_Y));
    cpu.write32(p + 4, y);
    if (y as i32) < TOP {
        cpu.write32(p + 4, TOP as u32);
    }
    let x = cpu.read32(p).wrapping_add((width << 18) as u32);
    cpu.write32(p, x);
    let mut v = cpu.read32(p + P_SCORE) as i32;
    let mut k = 0;
    while v > 0 {
        let d = nested!(cpu, REMAINDER, super::math::remainder(cpu, v as u32, 10));
        cpu.write8(p + P_DIGIT, d as u8);
        k += 1;
        cpu.write16(p + P_DELAY, (k * 4) as u16);
        nested!(cpu, DIGIT_SET, digit_set(cpu, p));
        let x = cpu.read32(p).wrapping_sub(DIGIT_W);
        cpu.write32(p, x);
        v /= 10;
    }
    if extra != 0 {
        let x = cpu.read32(p).wrapping_sub(((extra - 1) << 18) as u32);
        cpu.write32(p, x);
        cpu.write8(p + P_DIGIT, glyph);
        cpu.write16(p + P_DELAY, 0);
        nested!(cpu, DIGIT_SET, digit_set(cpu, p));
    }
}

/// A popup digit: its frame (the digit's, from 0x86), colour (a player's for the multiplier
/// glyphs: 0xc0, 0xb0 from the first character's; else 0x90), how long it stays, its lift.
const DIGIT_FRAME: u8 = 0x86;
const DIGIT_COLOUR: u16 = 0x90;
const GLYPH_COLOURS: [u16; 2] = [0xc0, 0xb0];
const DIGIT_STAYS: u16 = 60;
const DIGIT_LIFT: u8 = 0x7f;
const CHAR_BASE: u32 = 0x0607_41fc;
const DIGIT_EXEC: u32 = 0x0604_e898;

/// Makes one of a popup's digits (from `p`, popup's struct): in list 3, or else 2.
pub fn digit_set(cpu: &mut Cpu, p: u32) {
    let mut obj = effect_in(cpu, 3);
    if obj == 0 {
        obj = effect_in(cpu, 2);
        if obj == 0 {
            return;
        }
    }
    nested!(cpu, ACTOR_INIT, actor_init(cpu, obj));
    for k in 0..3 {
        let v = cpu.read32(p + X + 4 * k);
        cpu.write32(obj + A_POSITION + 4 * k, v);
    }
    cpu.write16(obj + A_ANGLE, 0);
    cpu.write32(obj + A_BEHAVIOUR, DIGIT_EXEC);
    cpu.write8(obj + A_KIND, 0);
    let digit = cpu.read8(p + P_DIGIT);
    cpu.write8(obj + A_FRAME, digit.wrapping_add(DIGIT_FRAME));
    let base = cpu.read16(CHAR_BASE);
    cpu.write16(obj + A_COLOUR, base);
    let player = cpu.read8(p + P_PLAYER);
    let colour = if digit > 9 && player <= 1 { GLYPH_COLOURS[player as usize] } else { DIGIT_COLOUR };
    cpu.write16(obj + A_COLOUR, base.wrapping_add(colour));
    cpu.write8(obj + A_PLAYER, 0xff);
    cpu.write8(obj + A_33, DIGIT_LIFT);
    cpu.write32(obj + A_SCALES + 4, 0);
    cpu.write16(obj + A_COUNT_W, DIGIT_STAYS);
    let delay = cpu.read8(p + P_DELAY_LOW);
    cpu.write8(obj + A_COUNT_B, delay);
}

/// A popup digit's behaviour: after its delay, it springs up (its height eased up past 1.25,
/// then settled back to 1), stays its frames, then shrinks away.
const SPRING_TO: i32 = 0x1_4100;
const SPRING_PAST: i32 = 0x1_3fff;
const SETTLE_STEP: u32 = 0x1000;

pub fn digit_exec(cpu: &mut Cpu, obj: u32) {
    let delay = cpu.read8(obj + A_COUNT_B);
    if delay != 0 {
        cpu.write8(obj + A_COUNT_B, delay.wrapping_sub(1));
    } else {
        match cpu.read8(obj + A_STATE) {
            0 => {
                let h = cpu.read32(obj + A_SCALES + 4) as i32;
                let h = h.wrapping_add(SPRING_TO.wrapping_sub(h) / 4);
                cpu.write32(obj + A_SCALES + 4, h as u32);
                if h > SPRING_PAST {
                    cpu.write8(obj + A_STATE, 1);
                }
            }
            1 => {
                let h = cpu.read32(obj + A_SCALES + 4);
                let h = nested!(cpu, APPROACH, super::actors::approach(cpu, h, 0x1_0000, SETTLE_STEP));
                cpu.write32(obj + A_SCALES + 4, h);
                if h == 0x1_0000 {
                    cpu.write8(obj + A_STATE, 2);
                }
            }
            _ => {}
        }
    }
    let left = cpu.read16(obj + A_COUNT_W);
    if left != 0 {
        cpu.write16(obj + A_COUNT_W, left.wrapping_sub(1));
    } else {
        let h = cpu.read32(obj + A_SCALES + 4);
        let h = nested!(cpu, APPROACH, super::actors::approach(cpu, h, 0, SETTLE_STEP));
        cpu.write32(obj + A_SCALES + 4, h);
        if h == 0 {
            end(cpu, obj);
        }
    }
}

/// A flare: a sprite of kind 0 that follows a place (if it has one), growing for 16 frames and
/// shrinking for 16 more (0x600 a frame), its frame flickering between 0xd3 and 0xd5 with the
/// game frames; it ends then, or once its player's actor ends.
const FL_FOLLOWS: u32 = 0x3c;
const FL_AGE: u32 = 0x40;
const FL_LAST_AGE: i8 = 32;
const FL_GROWING: i8 = 15;
const FL_GROWTH: u32 = 0x600;
const FL_FRAMES: [u8; 2] = [0xd5, 0xd3];
const PLAYER_ACTORS: u32 = 0x0607_40f0;
const PLAYER_LIVES: i8 = 0x6f;

pub fn flare_exec(cpu: &mut Cpu, o: u32) {
    let age = cpu.read8(o + FL_AGE);
    cpu.write8(o + FL_AGE, age.wrapping_add(1));
    if age as i8 == FL_LAST_AGE {
        cpu.write8(o + OBJ_MODE, ENDING);
        return;
    }
    let n = cpu.read8(o + A_PLAYER) as i8 as i32;
    let player = cpu.read32(PLAYER_ACTORS.wrapping_add((4 * n) as u32));
    if cpu.read8(player + OBJ_MODE) as i8 > PLAYER_LIVES {
        cpu.write8(o + OBJ_MODE, ENDING);
        return;
    }
    let at = cpu.read32(o + FL_FOLLOWS);
    if at != 0 {
        for k in 0..3 {
            let v = cpu.read32(at + 4 * k);
            cpu.write32(o + A_POSITION + 4 * k, v);
        }
    }
    let d = if cpu.read8(o + FL_AGE) as i8 > FL_GROWING { FL_GROWTH.wrapping_neg() } else { FL_GROWTH };
    for k in 0..2 {
        let v = cpu.read32(o + A_SCALES + 4 * k).wrapping_add(d);
        cpu.write32(o + A_SCALES + 4 * k, v);
    }
    let frame = FL_FRAMES[(cpu.read32(super::globals::GAME_FRAMES) & 1) as usize];
    cpu.write8(o + A_FRAME, frame);
}

/// Makes a flare from the struct at `d`: its place (x, y, z), its player (a byte, +0x1e), its
/// colour (a word, +0x20) and the place it follows (+0x3c). It, or 0.
const FLARE_SIZE: u32 = 0x44;
const FLARE_LIST: u32 = 2;
const FLARE_EXEC: u32 = 0x0606_4358;
const FLARE_LIFT: u8 = 8;
const FLARE_SCALE: u32 = 0x2000;

pub fn flare_set(cpu: &mut Cpu, d: u32) -> u32 {
    let blocks = nested!(cpu, OBJ_SIZE, objects::obj_size(cpu, FLARE_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, OBJ_SET, objects::obj_set(cpu, FLARE_LIST, blocks, PLAIN_EXEC));
    if o == 0 {
        return 0;
    }
    nested!(cpu, SPRITE_INIT, sprites::sprite_init(cpu, o + 0x10));
    cpu.write32(o + A_BEHAVIOUR, FLARE_EXEC);
    let n = cpu.read8(d + PLAYER);
    cpu.write8(o + A_PLAYER, n);
    cpu.write8(o + A_33, FLARE_LIFT);
    let colour = cpu.read16(d + COLOUR);
    cpu.write16(o + A_OWN_COLOUR, colour);
    cpu.write16(o + A_COLOUR, colour);
    cpu.write8(o + A_FRAME, FL_FRAMES[0]);
    cpu.write8(o + A_KIND, 0);
    let at = cpu.read32(d + RELATIVE_3C);
    cpu.write32(o + FL_FOLLOWS, at);
    for k in 0..3 {
        let v = cpu.read32(d + X + 4 * k);
        cpu.write32(o + A_POSITION + 4 * k, v);
    }
    cpu.write32(o + A_SCALES, FLARE_SCALE);
    cpu.write32(o + A_SCALES + 4, FLARE_SCALE);
    cpu.write8(o + FL_AGE, 0);
    o
}
