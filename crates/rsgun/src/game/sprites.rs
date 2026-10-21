//! The objects' sprites: an object's sprite (a struct in it) drawn through SGL: its kind's
//! character and frame, placed (the frame's offset turned with the sprite), brought into view
//! space, culled off screen.

use super::nested;
use super::Cpu;

/// A sprite (in an object): flags (SPRITE_*) with the attributes in their low word (ATTR_*), its
/// position (16.16 x, y, z), its scales (x, y), its colour, its kind and frame (bytes), its angle
/// (a word), and how far it is lifted toward the eye (a byte, in whole units).
const FLAGS: u32 = 0;
const ATTRS: u32 = 2;
const POS: u32 = 4;
const SCALE_X: u32 = 0x10;
const SCALE_Y: u32 = 0x14;
const COLOUR: u32 = 0x18;
const KIND: u32 = 0x1e;
const FRAME: u32 = 0x1f;
const ANGLE: u32 = 0x20;
const LIFT: u32 = 0x23;
/// The flags: off screen (set by the draw), not turned, never culled.
const OFF_SCREEN: u32 = 0x0200_0000;
const UNTURNED: u32 = 0x1000_0000;
const UNCULLED: u32 = 0x2000_0000;
/// The attributes passed on to SGL's, and the flip bits.
const ATTR_KEPT: u16 = 0x1100;
const FLIP: u16 = 0x30;

/// The kinds (16 bytes each): their frames (8 bytes each: the character's offset from the first,
/// an offset x and y in pixels, the flip), their first character and their attributes.
const KINDS: u32 = 0x0607_41f0;
const KIND_FRAMES: u32 = 4;
const KIND_CHAR: u32 = 8;
const KIND_ATTRS: u32 = 0xa;
/// Each character's width and height (8 bytes a character).
const CHAR_SIZES: u32 = 0x0600_cc70;
/// Off screen: further from the centre than half the character and this (pixels, x and y).
const MARGIN_X: u32 = 0xc0;
const MARGIN_Y: u32 = 0x80;
/// Where a sprite without depth is put, and its scale when both are one (SGL draws an exact one
/// otherwise as scaled, as far as can be told: one less).
const FLAT_Z: u32 = 0xb0_0000;

/// math.rs's turn_point (a vector turned and scaled by a struct's angle and scales), SGL's: a
/// point brought into view space (its depth back), and the sprite draws with one scale and with
/// two.
const TURN_POINT: u32 = 0x0606_4eac;
const SL_CONVERT: u32 = 0x0606_750c;
const SL_SPRITE: u32 = 0x0606_75ac;
const SL_SPRITE_HV: u32 = 0x0606_76ac;

/// The draw's frame (the frame's bottom): SGL's position (x, y, z, scales; a long after), its
/// sprite attributes (character, attributes, colour, Gouraud table, direction), and the offset
/// struct the frame's offset is turned in (x, y, ..., scales, flip bits, angle).
const P: u32 = 0;
const P_SCALE: u32 = 0xc;
const P_Z_FROM: u32 = 0x14;
const A: u32 = 0x18;
const A_ATTRS: u32 = 0x1a;
const A_COLOUR: u32 = 0x1c;
const A_GOURAUD: u32 = 0x1e;
const A_DIR: u32 = 0x20;
const O: u32 = 0x24;
const O_SCALES: u32 = 0xc;
const O_FLIP: u32 = 0x14;
const O_ANGLE: u32 = 0x18;
/// The direction's bit SGL is given always (as far as can be told).
const DIR_2: u16 = 2;

/// Draws the sprite at `s`: its flag OFF_SCREEN says whether it was culled.
pub fn draw_sprite(cpu: &mut Cpu, s: u32) {
    let f = cpu.stack();
    let kind = KINDS.wrapping_add((cpu.read8(s + KIND) as i8 as i32 * 16) as u32);
    let frames = cpu.read32(kind + KIND_FRAMES);
    let n = cpu.read8(s + FRAME) as u32;
    let frame = cpu.read32(frames + 8 * n);
    let attrs = cpu.read16(s + ATTRS) & ATTR_KEPT | cpu.read16(kind + KIND_ATTRS);
    cpu.write16(f + A_ATTRS, attrs);
    let colour = cpu.read16(s + COLOUR);
    cpu.write16(f + A_COLOUR, colour);
    cpu.write16(f + A_GOURAUD, 0);
    let first = cpu.read16(kind + KIND_CHAR);
    let ch = first.wrapping_add(cpu.read8(frame) as u16);
    cpu.write16(f + A, ch);
    let flip = cpu.read8(frame + 3) as i8 as i16 as u16;
    let dir = (flip ^ (cpu.read16(s + ATTRS) & FLIP)) | DIR_2;
    cpu.write16(f + A_DIR, dir);
    for k in 0..3 {
        let v = cpu.read32(s + POS + 4 * k);
        cpu.write32(f + P + 4 * k, v);
    }
    let sx = cpu.read32(s + SCALE_X);
    if sx == cpu.read32(s + SCALE_Y) {
        cpu.write32(f + P_SCALE, sx.wrapping_sub(1));
    } else {
        cpu.write32(f + P_SCALE, sx);
        let sy = cpu.read32(s + SCALE_Y);
        cpu.write32(f + P_SCALE + 4, sy);
    }
    let mut angle = cpu.read16(s + ANGLE) as i16 as i32;
    let (dx, dy) = (cpu.read8(frame + 1) as i8 as i32, cpu.read8(frame + 2) as i8 as i32);
    if dx != 0 || dy != 0 {
        let o = f + O;
        cpu.write32(o, (dx << 16) as u32);
        cpu.write32(o + 4, (dy << 16) as u32);
        cpu.write16(o + O_ANGLE, angle as u16);
        let sx = cpu.read32(s + SCALE_X);
        cpu.write32(o + O_SCALES, sx);
        let sy = cpu.read32(s + SCALE_Y);
        cpu.write32(o + O_SCALES + 4, sy);
        let flip = cpu.read32(s + FLAGS) & FLIP as u32;
        cpu.write32(o + O_FLIP, flip);
        nested!(cpu, TURN_POINT, super::math::turn_point(cpu, o));
        let x = cpu.read32(f + P).wrapping_add(cpu.read32(o));
        cpu.write32(f + P, x);
        let y = cpu.read32(f + P + 4).wrapping_add(cpu.read32(o + 4));
        cpu.write32(f + P + 4, y);
    }
    if cpu.read32(f + P + 8) == 0 {
        cpu.write32(f + P + 8, FLAT_Z);
    } else {
        let z = cpu.read32(s + POS + 8);
        cpu.write32(f + P_Z_FROM, z);
        let z = cpu.call(SL_CONVERT, &[f, f]);
        cpu.write32(f + P + 8, z);
        cpu.call(SL_CONVERT, &[f + P_SCALE, f + P_SCALE]);
    }
    let lift = (cpu.read8(s + LIFT) as i8 as i32) << 16;
    let z = cpu.read32(f + P + 8).wrapping_sub(lift as u32);
    cpu.write32(f + P + 8, z);
    let visible = cpu.read32(s + FLAGS) & UNCULLED != 0 || {
        let ch = cpu.read16(f + A) as u32;
        let w = (cpu.read16(CHAR_SIZES + 8 * ch) as u32 >> 1) + MARGIN_X;
        let h = (cpu.read16(CHAR_SIZES + 8 * ch + 2) as u32 >> 1) + MARGIN_Y;
        let x = (cpu.read32(f + P) as i32).wrapping_abs();
        let y = (cpu.read32(f + P + 4) as i32).wrapping_abs();
        x < (w << 16) as i32 && y < (h << 16) as i32
    };
    let flags = cpu.read32(s + FLAGS);
    if !visible {
        cpu.write32(s + FLAGS, flags | OFF_SCREEN);
        return;
    }
    if flags & UNTURNED != 0 {
        angle = 0;
    }
    if angle == -0x8000 {
        angle = 0;
        let dir = cpu.read16(f + A_DIR) ^ FLIP;
        cpu.write16(f + A_DIR, dir);
    } else if angle != 0 {
        for k in 0..2 {
            let v = cpu.read32(f + P + 4 * k).wrapping_add(0x1_0000);
            cpu.write32(f + P + 4 * k, v);
        }
    }
    let draw = if cpu.read32(s + SCALE_X) == cpu.read32(s + SCALE_Y) { SL_SPRITE } else { SL_SPRITE_HV };
    cpu.call(draw, &[f, f + A, angle as u32]);
    let flags = cpu.read32(s + FLAGS);
    cpu.write32(s + FLAGS, flags & !OFF_SCREEN);
}

/// A distorted sprite: its character drawn as a quad whose corners are turned and scaled with the
/// sprite (about the frame's offset, itself turned) and brought into view one by one; culled
/// unless a corner is on screen. The draw's frame: the corners (x, y words, from the centre), SGL's
/// command (as the polygon's; its attributes or'd with what the frame held there), the turn
/// struct, the view, the frame's offset (x, y: the pixels and the turned offset added), whether a
/// corner is on screen.
const D_CORNERS: u32 = 0;
const D_COMMAND: u32 = 0x10;
const D_TURN: u32 = 0x30;
const D_VIEW: u32 = 0x70;
const D_OFFSET: u32 = 0x78;
const D_SEEN: u32 = 0x80;

pub fn draw_distorted(cpu: &mut Cpu, s: u32) {
    let f = cpu.stack();
    let kind = KINDS.wrapping_add((cpu.read8(s + KIND) as i8 as i32 * 16) as u32);
    let frames = cpu.read32(kind + KIND_FRAMES);
    let n = cpu.read8(s + FRAME) as u32;
    let frame = cpu.read32(frames + 8 * n);
    let c = f + D_COMMAND;
    let attrs = cpu.read16(s + ATTRS) & ATTR_KEPT | cpu.read16(kind + KIND_ATTRS) | cpu.read16(c + C_ATTRS);
    cpu.write16(c + C_ATTRS, attrs);
    let colour = cpu.read16(s + COLOUR);
    cpu.write16(c + C_COLOUR, colour);
    cpu.write16(c + C_1C, 0);
    cpu.write16(c + C_1C + 2, 0);
    let dx = cpu.read8(frame + 1) as i8 as i32;
    cpu.write32(f + D_OFFSET, dx as u32);
    let dy = cpu.read8(frame + 2) as i8 as i32;
    cpu.write32(f + D_OFFSET + 4, dy as u32);
    let t = f + D_TURN;
    let z = cpu.read32(s + POS + 8);
    cpu.write32(t + T_Z, z);
    let angle = if cpu.read32(s + FLAGS) & UNTURNED != 0 { 0 } else { cpu.read16(s + ANGLE) };
    cpu.write16(t + O_ANGLE, angle);
    let sx = cpu.read32(s + SCALE_X);
    cpu.write32(t + O_SCALES, sx);
    let sy = cpu.read32(s + SCALE_Y);
    cpu.write32(t + O_SCALES + 4, sy);
    let flip = cpu.read32(s + FLAGS) & FLIP as u32;
    cpu.write32(t + O_FLIP, flip);
    if dx != 0 || dy != 0 {
        cpu.write32(t, (dx << 16) as u32);
        cpu.write32(t + 4, (dy << 16) as u32);
        nested!(cpu, TURN_POINT, super::math::turn_point(cpu, t));
        for k in 0..2 {
            let v = cpu.read32(f + D_OFFSET + 4 * k).wrapping_add(cpu.read32(t + 4 * k));
            cpu.write32(f + D_OFFSET + 4 * k, v);
        }
    }
    let first = cpu.read16(kind + KIND_CHAR) as i16 as i32;
    let ch = first + cpu.read8(frame) as i32;
    let size = CHAR_SIZES.wrapping_add((8 * ch) as u32);
    let flip = cpu.read8(frame + 3) as i8 as i16 as u16;
    let dir = (cpu.read16(s + ATTRS) & FLIP) ^ flip | DIR_2;
    cpu.write16(c + C_TYPE, dir);
    let w = cpu.read16(size + 4);
    cpu.write16(c + C_SIZE, w);
    let h = cpu.read16(size + 6);
    cpu.write16(c + C_SIZE + 2, h);
    let (w, h) = (cpu.read16(size) >> 1, cpu.read16(size + 2) >> 1);
    let (mw, mh) = (w.wrapping_neg(), h.wrapping_neg());
    for (i, (x, y)) in [(mw, mh), (w, mh), (w, h), (mw, h)].into_iter().enumerate() {
        cpu.write16(f + D_CORNERS + 4 * i as u32, x);
        cpu.write16(f + D_CORNERS + 4 * i as u32 + 2, y);
    }
    cpu.write32(t + O_FLIP, 0);
    let seen = cpu.read32(s + FLAGS) & UNCULLED;
    cpu.write32(f + D_SEEN, seen);
    let mut depth = 0;
    for i in 0..4 {
        let cx = cpu.read16(f + D_CORNERS + 4 * i) as i16 as i32;
        cpu.write32(t, (cx << 16) as u32);
        let cy = cpu.read16(f + D_CORNERS + 4 * i + 2) as i16 as i32;
        cpu.write32(t + 4, (cy << 16) as u32);
        nested!(cpu, TURN_POINT, super::math::turn_point(cpu, t));
        let x = cpu.read32(t).wrapping_add(cpu.read32(s + POS).wrapping_add(cpu.read32(f + D_OFFSET)));
        cpu.write32(t, x);
        let y = cpu.read32(t + 4).wrapping_add(cpu.read32(s + POS + 4).wrapping_add(cpu.read32(f + D_OFFSET + 4)));
        cpu.write32(t + 4, y);
        let corner = c + C_CORNERS + 4 * i;
        if cpu.read32(t + T_Z) == 0 {
            cpu.write16(corner, ((x as i32) / 0x1_0000) as u16);
            let y = cpu.read32(t + 4);
            cpu.write16(corner + 2, ((y as i32) / 0x1_0000) as u16);
            depth = FLAT_Z;
        } else {
            depth = cpu.call(SL_CONVERT_2D, &[t, f + D_VIEW]);
            let vx = cpu.read16(f + D_VIEW + 2);
            cpu.write16(corner, vx);
            let vy = cpu.read16(f + D_VIEW + 6);
            cpu.write16(corner + 2, vy);
        }
        if cpu.read32(f + D_SEEN) == 0 {
            let x = (cpu.read16(corner) as i16 as i32).abs();
            if x <= CORNER_X {
                let y = (cpu.read16(corner + 2) as i16 as i32).abs();
                if y <= CORNER_Y {
                    cpu.write32(f + D_SEEN, 1);
                }
            }
        }
    }
    if cpu.read32(f + D_SEEN) == 0 {
        let flags = cpu.read32(s + FLAGS);
        cpu.write32(s + FLAGS, flags | OFF_SCREEN);
        return;
    }
    for at in [4, 8, 10, 14] {
        let v = cpu.read16(c + C_CORNERS + at).wrapping_sub(1);
        cpu.write16(c + C_CORNERS + at, v);
    }
    let lift = (cpu.read8(s + LIFT) as i8 as i32) << 16;
    cpu.call(SL_POLYGON, &[c, depth.wrapping_sub(lift as u32)]);
    let flags = cpu.read32(s + FLAGS);
    cpu.write32(s + FLAGS, flags & !OFF_SCREEN);
}

/// A model (the same struct, for 3D): drawn by its own function (OWN_DRAW: the function at
/// MODEL), else its polygons (at MODEL) placed, turned (z, x, y: words), scaled (x, y, z; flipped
/// by the FLIP bits) inside SGL's matrix stack.
const OWN_DRAW: u32 = 0x40_0000;
const MODEL: u32 = 0x24;
const TURN_Z: u32 = 0x20;
const TURN_X: u32 = 0x1c;
const TURN_Y: u32 = 0x1e;
const SCALE_Z: u32 = 0x18;
const FLIP_X: u32 = 0x10;
const FLIP_Y: u32 = 0x20;
/// SGL's (as far as can be told): the matrix pushed, translated, turned about z, x and y, scaled,
/// the polygons put, the matrix popped.
const SL_PUSH_MATRIX: u32 = 0x0606_71cc;
const SL_TRANSLATE: u32 = 0x0606_721c;
const SL_ROT_Z: u32 = 0x0606_726c;
const SL_ROT_X: u32 = 0x0606_72ec;
const SL_ROT_Y: u32 = 0x0606_736c;
const SL_SCALE: u32 = 0x0606_73ec;
const SL_PUT_POLYGON: u32 = 0x0606_745c;
const SL_POP_MATRIX: u32 = 0x0606_7200;

/// Draws the model at `s`. (The SH-2 code holds `s` in r8 throughout: its own draw's prologue
/// pushes it, read later as an unset field.)
pub fn draw_model(cpu: &mut Cpu, s: u32) {
    cpu.holding(&[(8, s)], |cpu| model(cpu, s));
}

fn model(cpu: &mut Cpu, s: u32) {
    if cpu.read32(s + FLAGS) & OWN_DRAW != 0 {
        let draw = cpu.read32(s + MODEL);
        cpu.call(draw, &[s]);
        return;
    }
    cpu.call(SL_PUSH_MATRIX, &[]);
    let (x, y, z) = (cpu.read32(s + POS), cpu.read32(s + POS + 4), cpu.read32(s + POS + 8));
    cpu.call(SL_TRANSLATE, &[x, y, z]);
    for (field, turn) in [(TURN_Z, SL_ROT_Z), (TURN_X, SL_ROT_X), (TURN_Y, SL_ROT_Y)] {
        let a = cpu.read16(s + field) as i16 as i32 as u32;
        if a != 0 {
            cpu.call(turn, &[a]);
        }
    }
    let mut sx = cpu.read32(s + SCALE_X);
    let flags = cpu.read32(s + FLAGS);
    let mut sy = cpu.read32(s + SCALE_Y);
    if flags & FLIP_X != 0 {
        sx = sx.wrapping_neg();
    }
    if flags & FLIP_Y != 0 {
        sy = sy.wrapping_neg();
    }
    let sz = cpu.read32(s + SCALE_Z);
    cpu.call(SL_SCALE, &[sx, sy, sz]);
    let model = cpu.read32(s + MODEL);
    cpu.call(SL_PUT_POLYGON, &[model]);
    cpu.call(SL_POP_MATRIX, &[]);
}

/// The flags that pick the draw: none (bit 31), a polygon (any of POLYGON_BITS: 4 the character's
/// own size, 3 two corners (a line), else four), a model (IS_MODEL), a distorted sprite (bit 30).
const NO_DRAW: u32 = 0x8000_0000;
const POLYGON_BITS: u32 = 7;
const SPRITE_POLYGON: u32 = 4;
const TWO_CORNERS: u32 = 3;
const IS_MODEL: u32 = 0x80_0000;
const DISTORTED: u32 = 0x4000_0000;
/// A polygon's corners (x, y words, pixels, turned with the sprite: four, or two for a line).
const CORNERS: u32 = 0x24;
/// The draw's frame: SGL's polygon command (its type: 2 a textured quad, else 3-6 by the
/// attributes' low bits; its attributes, colour, the character's size, the corners, two words),
/// the turn struct a corner is placed with, where SGL puts a corner brought into view, the count of
/// corners, whether one is on screen, the corners' address.
const C_TYPE: u32 = 0;
const C_ATTRS: u32 = 4;
const C_COLOUR: u32 = 6;
const C_SIZE: u32 = 8;
const C_CORNERS: u32 = 0xc;
const C_1C: u32 = 0x1c;
const T: u32 = 0x20;
const T_Z: u32 = 8;
const VIEW: u32 = 0x60;
const COUNT: u32 = 0x68;
const SEEN: u32 = 0x6c;
const CORNERS_AT: u32 = 0x70;
/// The polygon's attributes: SGL's base, and the bit cleared for a textured quad.
const POLYGON_ATTRS: u16 = 0xc0;
const TEXTURED_CLEARS: u16 = 0x40;
/// A corner on screen: within these of the centre (pixels).
const CORNER_X: i32 = 0xaf;
const CORNER_Y: i32 = 0x6f;
/// The distorted sprite's draw. SGL's (as far as can be told): a point brought into view (its
/// screen place at VIEW +2, +6; its depth back), and the polygon command drawn.
const DRAW_DISTORTED: u32 = 0x0604_0604;
const SL_CONVERT_2D: u32 = 0x0606_76fc;
const SL_POLYGON: u32 = 0x0606_778c;
/// The rewrites' SH-2 entries (for nested!).
const DRAW_SPRITE: u32 = 0x0604_03c4;
const DRAW_MODEL: u32 = 0x0604_030c;

/// Draws the sprite, model or polygon at `s` by its flags; a polygon's flag OFF_SCREEN says
/// whether it was culled (no corner on screen).
pub fn draw(cpu: &mut Cpu, s: u32) {
    let flags = cpu.read32(s + FLAGS);
    if flags & NO_DRAW != 0 {
        return;
    }
    if flags & (IS_MODEL | POLYGON_BITS) == 0 {
        if flags & DISTORTED == 0 {
            nested!(cpu, DRAW_SPRITE, draw_sprite(cpu, s));
        } else {
            nested!(cpu, DRAW_DISTORTED, draw_distorted(cpu, s));
        }
        return;
    }
    if flags & IS_MODEL != 0 {
        nested!(cpu, DRAW_MODEL, draw_model(cpu, s));
        return;
    }
    if flags & POLYGON_BITS == 0 {
        return;
    }
    let f = cpu.stack();
    let corners = cpu.read32(s + CORNERS);
    cpu.write32(f + CORNERS_AT, corners);
    if corners == 0 {
        return;
    }
    cpu.write16(f + C_ATTRS, POLYGON_ATTRS);
    let mut attrs = cpu.read16(s + ATTRS) & ATTR_KEPT | POLYGON_ATTRS;
    cpu.write16(f + C_ATTRS, attrs);
    let colour = cpu.read16(s + COLOUR);
    cpu.write16(f + C_COLOUR, colour);
    cpu.write16(f + C_1C, 0);
    cpu.write16(f + C_1C + 2, 0);
    if cpu.read32(s + FLAGS) & SPRITE_POLYGON != 0 {
        cpu.write16(f + C_TYPE, 2);
        attrs &= !TEXTURED_CLEARS;
        cpu.write16(f + C_ATTRS, attrs);
        let kind = KINDS.wrapping_add((cpu.read8(s + KIND) as i8 as i32 * 16) as u32);
        let first = cpu.read16(kind + KIND_CHAR) as i16 as i32;
        let frames = cpu.read32(kind + KIND_FRAMES);
        let n = cpu.read8(s + FRAME) as u32;
        let frame = cpu.read32(frames + 8 * n);
        let ch = first + cpu.read8(frame) as i32;
        let size = CHAR_SIZES.wrapping_add((8 * ch) as u32);
        let w = cpu.read16(size + 4);
        cpu.write16(f + C_SIZE, w);
        let h = cpu.read16(size + 6);
        cpu.write16(f + C_SIZE + 2, h);
    } else {
        let t = (cpu.read16(s + ATTRS) & 3) + 3;
        cpu.write16(f + C_TYPE, t);
    }
    let t = f + T;
    let sx = cpu.read32(s + SCALE_X);
    cpu.write32(t + O_SCALES, sx);
    let sy = cpu.read32(s + SCALE_Y);
    cpu.write32(t + O_SCALES + 4, sy);
    let flip = cpu.read32(s + FLAGS) & FLIP as u32;
    cpu.write32(t + O_FLIP, flip);
    let angle = if cpu.read32(s + FLAGS) & UNTURNED != 0 { 0 } else { cpu.read16(s + ANGLE) };
    cpu.write16(t + O_ANGLE, angle);
    let count = if cpu.read32(s + FLAGS) & 3 == TWO_CORNERS { 2 } else { 4 };
    cpu.write32(f + COUNT, count);
    let z = cpu.read32(s + POS + 8);
    cpu.write32(t + T_Z, z);
    let seen = cpu.read32(s + FLAGS) & UNCULLED;
    cpu.write32(f + SEEN, seen);
    let mut depth = 0;
    for i in 0..count {
        let corners = cpu.read32(f + CORNERS_AT);
        let cx = cpu.read16(corners + 4 * i) as i16 as i32;
        cpu.write32(t, cx as u32);
        let cy = cpu.read16(corners + 4 * i + 2) as i16 as i32;
        cpu.write32(t + 4, cy as u32);
        if cx != 0 || cy != 0 {
            cpu.write32(t, (cx << 16) as u32);
            cpu.write32(t + 4, (cy << 16) as u32);
            nested!(cpu, TURN_POINT, super::math::turn_point(cpu, t));
        }
        let x = cpu.read32(t).wrapping_add(cpu.read32(s + POS));
        cpu.write32(t, x);
        let y = cpu.read32(t + 4).wrapping_add(cpu.read32(s + POS + 4));
        cpu.write32(t + 4, y);
        let corner = f + C_CORNERS + 4 * i;
        if cpu.read32(t + T_Z) == 0 {
            cpu.write16(corner, ((x as i32) / 0x1_0000) as u16);
            let y = cpu.read32(t + 4);
            cpu.write16(corner + 2, ((y as i32) / 0x1_0000) as u16);
            depth = FLAT_Z;
        } else {
            depth = cpu.call(SL_CONVERT_2D, &[t, f + VIEW]);
            let vx = cpu.read16(f + VIEW + 2);
            cpu.write16(corner, vx);
            let vy = cpu.read16(f + VIEW + 6);
            cpu.write16(corner + 2, vy);
        }
        if cpu.read32(f + SEEN) == 0 {
            let x = (cpu.read16(corner) as i16 as i32).abs();
            if x <= CORNER_X {
                let y = (cpu.read16(corner + 2) as i16 as i32).abs();
                if y <= CORNER_Y {
                    cpu.write32(f + SEEN, 1);
                }
            }
        }
    }
    if cpu.read32(f + SEEN) != 0 {
        let lift = (cpu.read8(s + LIFT) as i8 as i32) << 16;
        cpu.call(SL_POLYGON, &[f, depth.wrapping_sub(lift as u32)]);
        let flags = cpu.read32(s + FLAGS);
        cpu.write32(s + FLAGS, flags & !OFF_SCREEN);
    } else {
        let flags = cpu.read32(s + FLAGS);
        cpu.write32(s + FLAGS, flags | OFF_SCREEN);
    }
}

/// A sprite struct set up: no flags, scales one (x, y, and the long after: a model's z scale), the
/// turns and angle none.
pub fn sprite_init(cpu: &mut Cpu, s: u32) {
    cpu.write32(s + FLAGS, 0);
    for field in [SCALE_Y, SCALE_X, SCALE_Y + 4] {
        cpu.write32(s + field, 0x1_0000);
    }
    cpu.write32(s + 0x1c, 0);
    cpu.write16(s + ANGLE, 0);
}
