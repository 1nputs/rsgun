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
/// The effects' behaviour (not rewritten yet).
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
