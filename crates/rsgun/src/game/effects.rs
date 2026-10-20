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
/// SGL's (as far as can be told): a 16.16 division.
const SL_DIV: u32 = 0x0606_62d8;
const SL_TURN: u32 = 0x0606_4eac;
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
    if cpu.read8(obj + A_FLAGS) & LOOPS != 0 && cpu.read8(obj + A_LINK_FLAGS) & 3 != 0 && cpu.read16(obj + A_COUNT_W) == 0 {
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
                let step = cpu.call(SL_DIV, &[scale, n]);
                cpu.write32(obj + A_SCALE_SPEED + 4 * k, step.wrapping_neg());
            }
            cpu.write8(obj + A_COUNT_B, 0xff);
        }
        let left = cpu.read16(obj + A_COUNT_W).wrapping_sub(1);
        cpu.write16(obj + A_COUNT_W, left);
        if left == 0 {
            if cpu.read8(obj + A_FLAGS) & LOOPS != 0 {
                let f = cpu.read8(obj + A_LINK_FLAGS);
                cpu.write8(obj + A_LINK_FLAGS, f & !2);
            } else {
                end(cpu, obj);
            }
        }
    }
    let sx = cpu.read32(obj + A_SCALES).wrapping_add(cpu.read32(obj + A_SCALE_SPEED));
    cpu.write32(obj + A_SCALES, sx);
    let sy = cpu.read32(obj + A_SCALES + 4).wrapping_add(cpu.read32(obj + A_SCALE_SPEED + 4));
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
        cpu.call(SL_TURN, &[t]);
        for k in 0..2 {
            let p = cpu.read32(parent + A_POSITION + 4 * k).wrapping_add(cpu.read32(t + 4 * k));
            cpu.write32(obj + A_POSITION + 4 * k, p);
        }
        let z = cpu.read32(parent + A_POSITION + 8);
        cpu.write32(obj + A_POSITION + 8, z);
        (cpu.read16(parent + A_ANGLE), cpu.read16(obj + A_REL_ANGLE + 2))
    };
    cpu.write16(obj + A_ANGLE, angle.wrapping_add(spin));
    if cpu.read8(obj + A_FLAGS) & LINKED != 0 {
        nested!(cpu, LINK_STEP, link_step(cpu, obj + 0x10, obj + A_LINK));
    }
}
