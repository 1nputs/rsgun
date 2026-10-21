//! The ribbons: an enemy laser drawn as a strip of polygons along the last 32 places of its head.
//! The head flies by one of four movers (straight, homing, waving, for a time), speeding up; it
//! ends on the ground, on what it hits, or once its whole length is off screen, and then fades
//! (its tail drawn in over 32 frames). For its first 32 frames it is carried with what fired it.
//! Its eight polygons narrow from its head (16 points: two a place, every 4 or 5 places), and
//! three of its boxes follow its length.

use super::{aim, bg, nested, Cpu};

/// The ribbon's fields: its flags (no hits), heading (a word, where the sprite's kind and frame
/// would be), sprite draw, behaviour; its place in the trail (a byte), off screen (a byte), its
/// own flags (ENDS_IN_BURST, HALF_WIDTH), frames carried (a byte); its colour base (a word), what
/// fired it (a pointer to its x, y), that one's last place (words), its speed (the fade's count
/// once it ends), its mover; the trail (32 places: x, y words, pixels); its boxes (8 bytes each
/// from 0x108), its points (16, 12 bytes each from 0x128) and polygon attributes (7, 12 bytes
/// each from 0x1e8), SGL's polygon data (from 0x23c: the points, their count, the polygons,
/// their count, the attributes); its turn a frame and the angle within which it does not turn,
/// its speed up a frame and the waving's base speed and phase.
const OBJ_MODE: u32 = 3;
const ENDING: u8 = 0x70;
const FLAGS: u32 = 0x10;
const POSITION: u32 = 0x14;
const HEADING: u32 = 0x2e;
const NUMBER: u32 = 0x32;
const LIFT: u32 = 0x33;
const DRAW: u32 = 0x34;
const BEHAVIOUR: u32 = 0x38;
const BOXES: u32 = 0x3c;
const HIT_SET: u32 = 0x48;
const SIDE: u32 = 0x54;
const TYPE: u32 = 0x55;
const HIT_FLAGS: u32 = 0x56;
const HIT_FIRST: u32 = 0x5a;
const AT: u32 = 0x6c;
const OFF: u32 = 0x6d;
const R_FLAGS: u32 = 0x6e;
const CARRIED: u32 = 0x6f;
const COLOUR_BASE: u32 = 0x70;
const SOURCE: u32 = 0x74;
const SOURCE_AT: u32 = 0x78;
const SPEED: u32 = 0x7c;
const R_80: u32 = 0x80;
const MOVER: u32 = 0x84;
const TRAIL: u32 = 0x88;
const BOX_LIST: u32 = 0x108;
const POINTS: u32 = 0x128;
const ATTRS: u32 = 0x1e8;
const PDATA: u32 = 0x23c;
const TURN: u32 = 0x250;
const TURN_WITHIN: u32 = 0x252;
const ACCEL: u32 = 0x254;
const WAVE_BASE: u32 = 0x258;
const WAVE_PHASE: u32 = 0x25c;
/// The flags: never hit; its own: ends in a burst (else the other end), drawn half as wide.
const NO_HITS: u32 = 0x8_0000;
const RIBBON_FLAGS: u32 = 0xc0_0400;
const ENDS_IN_BURST: u8 = 1;
const HALF_WIDTH: u8 = 2;
/// The hit flags: hit something (ending it unless what it hit has an attribute).
const HIT_SOMETHING: u16 = 0x400;
const TRAIL_LEN: u32 = 32;
const TRAIL_MASK: u32 = 31;
const CARRIED_FRAMES: i8 = 31;
/// Its points' depth, and once it ends (behind).
const POINT_Z: u32 = 8;
const POINTS_COUNT: u32 = 16;
const POINT_SIZE: u32 = 12;
const RIBBON_Z: u32 = 0xfff8_0000;
const ENDED_Z: u32 = 0x9_0000;
/// The polygons (SGL's table), their attributes (a table of 7 copied, the colour by the base),
/// the boxes (4 copied: the head's and three along it).
const POLYGONS: u32 = 0x0607_3b34;
const POLYGON_COUNT: u32 = 7;
const ATTR_TABLE: u32 = 0x0607_3bc0;
const ATTR_SIZE: u32 = 12;
const ATTR_COLOUR: u32 = 6;
const BOX_TABLE: u32 = 0x0607_3c14;
const BOX_SIZE: u32 = 8;
const BOX_COUNT: u32 = 4;
const RIBBON_SIDE: u8 = 1;
const RIBBON_TYPE: u8 = 5;
const EXEC: u32 = 0x0606_39e0;
const FADE: u32 = 0x0606_3b94;
const DRAW_FN: u32 = 0x0606_3884;
/// The description (the maker's): its place (x, y), the player byte, its colour base (a word),
/// heading (from straight down, a word), turn, speed up, the waving's base, the turn's dead angle
/// (words), what fired it.
const D_NUMBER: u32 = 0x1e;
const D_COLOUR: u32 = 0x20;
const D_HEADING: u32 = 0x2c;
const D_TURN: u32 = 0x2e;
const D_ACCEL: u32 = 0x30;
const D_WAVE_BASE: u32 = 0x34;
const D_TURN_WITHIN: u32 = 0x38;
const D_SOURCE: u32 = 0x3c;
/// The maker's frame: the mover, an attribute's address (the last's next), the colour's.
const F_MOVER: u32 = 0;
const F_ATTR: u32 = 4;
const F_COLOUR: u32 = 8;

/// SGL's.
const SL_SIN: u32 = 0x0606_9a0c;
const SL_COS: u32 = 0x0606_9a30;
const SL_MUL_FX: u32 = 0x0606_e26c;
const SL_ATAN: u32 = 0x0606_780c;
const SL_PUSH_MATRIX: u32 = 0x0606_71cc;
const SL_POP_MATRIX: u32 = 0x0606_7200;
/// SGL's polygon put done on the master (sgl::put_polygons): the data at its first argument, an
/// attribute for each polygon (1).
const SL_POLYGONS: u32 = 0x0607_14dc;

/// Sets up the ribbon `o` from the description at `d`, to move by `mover`; its edges placed. It.
pub fn ribbon_init(cpu: &mut Cpu, o: u32, d: u32, mover: u32) -> u32 {
    let f = cpu.stack();
    cpu.write32(f + F_MOVER, mover);
    nested!(cpu, 0x0604_6264, super::sprites::sprite_init(cpu, o + 0x10));
    nested!(cpu, 0x0604_6284, super::hits::hit_init(cpu, o + BOXES));
    let flags = cpu.read32(o + FLAGS) | RIBBON_FLAGS;
    cpu.write32(o + FLAGS, flags);
    let n = cpu.read8(d + D_NUMBER);
    cpu.write8(o + NUMBER, n);
    cpu.write32(o + BEHAVIOUR, EXEC);
    for k in 0..2 {
        let v = cpu.read32(d + 4 * k);
        cpu.write32(o + POSITION + 4 * k, v);
    }
    cpu.write32(o + POSITION + 8, 0);
    cpu.write8(o + SIDE, RIBBON_SIDE);
    cpu.write32(o + BOXES, o + BOX_LIST);
    cpu.write8(o + TYPE, RIBBON_TYPE);
    let source = cpu.read32(d + D_SOURCE);
    cpu.write32(o + SOURCE, source);
    cpu.write8(o + LIFT, 0);
    cpu.write32(o + HIT_SET, u32::MAX);
    cpu.write32(o + HIT_SET + 4, u32::MAX);
    cpu.write32(f + F_COLOUR, d + D_COLOUR);
    let base = cpu.read16(d + D_COLOUR);
    cpu.write16(o + COLOUR_BASE, base);
    for k in 0..POLYGON_COUNT {
        let attr = o + ATTRS + ATTR_SIZE * k;
        for w in 0..6 {
            let v = cpu.read16(ATTR_TABLE + ATTR_SIZE * k + 2 * w);
            cpu.write16(attr + 2 * w, v);
        }
        let colour = cpu.read16(attr + ATTR_COLOUR).wrapping_add(cpu.read16(d + D_COLOUR));
        cpu.write16(attr + ATTR_COLOUR, colour);
        cpu.write32(f + F_ATTR, o + ATTR_SIZE * (k + 1));
    }
    cpu.write32(o + PDATA + 0x10, o + ATTRS);
    let source = cpu.read32(o + SOURCE);
    let x = cpu.read16(source);
    cpu.write16(o + SOURCE_AT, x);
    let y = (cpu.read32(source + 4) >> 16) as u16;
    cpu.write16(o + SOURCE_AT + 2, y);
    cpu.write32(o + PDATA, o + POINTS);
    cpu.write32(o + PDATA + 4, POINTS_COUNT);
    cpu.write32(o + PDATA + 8, POLYGONS);
    cpu.write32(o + PDATA + 0xc, POLYGON_COUNT);
    cpu.write32(o + DRAW, DRAW_FN);
    let mover = cpu.read32(f + F_MOVER);
    cpu.write32(o + MOVER, mover);
    cpu.write8(o + OFF, 0);
    cpu.write8(o + CARRIED, 0);
    cpu.write32(o + R_80, 0);
    cpu.write32(o + SPEED, 0);
    cpu.write8(o + AT, 0);
    let heading = 0x8000u16.wrapping_sub(cpu.read16(d + D_HEADING));
    cpu.write16(o + HEADING, heading);
    for (field, from) in [(TURN, D_TURN), (TURN_WITHIN, D_TURN_WITHIN)] {
        let v = cpu.read16(d + from);
        cpu.write16(o + field, v);
    }
    for (field, from) in [(ACCEL, D_ACCEL), (WAVE_BASE, D_WAVE_BASE)] {
        let v = cpu.read32(d + from);
        cpu.write32(o + field, v);
    }
    let (x, y) = (cpu.read32(o + POSITION) >> 16, cpu.read32(o + POSITION + 4) >> 16);
    for k in 0..TRAIL_LEN {
        cpu.write16(o + TRAIL + 4 * k, x as u16);
        cpu.write16(o + TRAIL + 4 * k + 2, y as u16);
    }
    for k in 0..POINTS_COUNT {
        cpu.write32(o + POINTS + POINT_SIZE * k + POINT_Z, RIBBON_Z);
    }
    for k in 0..BOX_COUNT {
        for w in 0..4 {
            let v = cpu.read16(BOX_TABLE + BOX_SIZE * k + 2 * w);
            cpu.write16(o + BOX_LIST + BOX_SIZE * k + 2 * w, v);
        }
    }
    nested!(cpu, 0x0606_3f70, ribbon_edges(cpu, o, 0));
    o
}

/// The ribbon drawn (its sprite's draw; `s` its sprite): its polygons.
pub fn ribbon_draw(cpu: &mut Cpu, s: u32) {
    cpu.call(SL_PUSH_MATRIX, &[]);
    cpu.call(SL_POLYGONS, &[s - 0x10 + PDATA, 1]);
    cpu.call(SL_POP_MATRIX, &[]);
}

/// The ribbon ended: it fades (FADE), its points pushed back, and it bursts (bullets.rs's burst,
/// at its head, the ball in its second colours) or flashes (ribbon_flash).
const BURST: u32 = 0x0605_5310;
const B_NUMBER: u32 = 0x1e;
const B_FIELDS: [(u32, u16); 4] = [(0x2c, 0x200), (0x2e, 0x60), (0x38, 0xfffa), (0x3a, 0x101)];

pub fn ribbon_end(cpu: &mut Cpu, o: u32) {
    let f = cpu.stack();
    cpu.write32(o + BEHAVIOUR, FADE);
    cpu.write32(o + SPEED, 0);
    for k in 0..POINTS_COUNT {
        cpu.write32(o + POINTS + POINT_SIZE * k + POINT_Z, ENDED_Z);
    }
    if cpu.read8(o + R_FLAGS) & ENDS_IN_BURST == 0 {
        nested!(cpu, 0x0606_4494, ribbon_flash(cpu, o));
        return;
    }
    for k in 0..3 {
        let v = cpu.read32(o + POSITION + 4 * k);
        cpu.write32(f + 4 * k, v);
    }
    let n = cpu.read8(o + NUMBER);
    cpu.write8(f + B_NUMBER, n);
    for (field, v) in B_FIELDS {
        cpu.write16(f + field, v);
    }
    cpu.write32(f + 0x30, 0);
    cpu.write32(f + 0x34, 0);
    nested!(cpu, BURST, super::bullets::burst_set(cpu, f));
}

/// The flash at the ribbon's head as it ends: an effect (kind 0, frame 1, in its colours, for 32
/// frames) growing from nothing by 1/16 a frame. The struct's fields past those set are what the
/// stack held.
const FL_FLAGS: u32 = 0x14;
const FL_ANGLE: u32 = 0x18;
const FL_KIND: u32 = 0x1c;
const FL_FRAME: u32 = 0x1d;
const FL_PLAYER: u32 = 0x1e;
const FL_COLOUR: u32 = 0x20;
const FL_FRAMES: u32 = 0x24;
const FL_LIFT: u32 = 0x26;
const FLASH_FRAMES: u16 = 32;
const FLASH_LIFT: u8 = 16;
const FX_SCALES: u32 = 0x20;
const FX_SCALE_SPEED: u32 = 0x54;
const FX_LIFT: u32 = 0x33;
const FLASH_GROWTH: u32 = 0x1000;
const FLASH_SINK: u8 = 0xf8;

pub fn ribbon_flash(cpu: &mut Cpu, o: u32) {
    let f = cpu.stack();
    for k in 0..3 {
        let v = cpu.read32(o + POSITION + 4 * k);
        cpu.write32(f + 4 * k, v);
    }
    cpu.write32(f + FL_FLAGS, 0);
    cpu.write16(f + FL_ANGLE, 0);
    cpu.write16(f + FL_FRAMES, FLASH_FRAMES);
    cpu.write8(f + FL_LIFT, FLASH_LIFT);
    cpu.write8(f + FL_KIND, 0);
    cpu.write8(f + FL_FRAME, 1);
    let colour = cpu.read16(o + COLOUR_BASE);
    cpu.write16(f + FL_COLOUR, colour);
    let n = cpu.read8(o + NUMBER);
    cpu.write8(f + FL_PLAYER, n);
    let e = nested!(cpu, 0x0604_e1cc, super::effects::effect_set(cpu, f));
    if e != 0 {
        cpu.write32(e + FX_SCALES, 0);
        cpu.write32(e + FX_SCALES + 4, 0);
        cpu.write32(e + FX_SCALE_SPEED, FLASH_GROWTH);
        cpu.write32(e + FX_SCALE_SPEED + 4, FLASH_GROWTH);
        cpu.write8(e + FX_LIFT, FLASH_SINK);
    }
}

/// The trail's place `k` (16.16).
fn place(cpu: &mut Cpu, o: u32, k: u32) -> (u32, u32) {
    let x = cpu.read16(o + TRAIL + 4 * k) as i16 as i32 as u32;
    let y = cpu.read16(o + TRAIL + 4 * k + 2) as i16 as i32 as u32;
    (x << 16, y << 16)
}

/// The head's place at the trail's place `k` (pixels).
fn mark(cpu: &mut Cpu, o: u32, k: u32) {
    let x = (cpu.read32(o + POSITION) >> 16) as u16;
    cpu.write16(o + TRAIL + 4 * k, x);
    let y = (cpu.read32(o + POSITION + 4) >> 16) as u16;
    cpu.write16(o + TRAIL + 4 * k + 2, y);
}

/// The ribbon's behaviour: off screen noted; ended once its head and its tail are both well off
/// screen (or, while carried, what fired it ends); else it moves (its mover), takes the next
/// place in the trail, is carried with what fired it for 32 frames, and its edges and boxes are
/// placed.
const F_HEAD: u32 = 0;
const F_TAIL: u32 = 8;
const F_MOVED: u32 = 0x10;
const F_SOURCE: u32 = 0x14;
const F_SOURCE_Y_AT: u32 = 0x18;
/// The numbered objects (what fired the ribbon, by its number).
const NUMBERED: u32 = 0x0607_40f0;

pub fn ribbon_exec(cpu: &mut Cpu, o: u32) {
    let f = cpu.stack();
    let off = nested!(cpu, 0x0606_3900, aim::point_area(cpu, o + POSITION)) != 0;
    cpu.write8(o + OFF, off as u8);
    let at = cpu.read8(o + AT) as i8 as i32 as u32;
    let (x, y) = place(cpu, o, at);
    cpu.write32(f + F_HEAD, x);
    cpu.write32(f + F_HEAD + 4, y);
    let (x, y) = place(cpu, o, at.wrapping_add(1) & TRAIL_MASK);
    cpu.write32(f + F_TAIL, x);
    cpu.write32(f + F_TAIL + 4, y);
    if nested!(cpu, 0x0606_38c0, aim::point_area_wide(cpu, f + F_HEAD)) != 0 && nested!(cpu, 0x0606_38c0, aim::point_area_wide(cpu, f + F_TAIL)) != 0 {
        cpu.write8(o + OBJ_MODE, ENDING);
        return;
    }
    let mover = cpu.read32(o + MOVER);
    cpu.call(mover, &[o]);
    let at = (cpu.read8(o + AT) as u32).wrapping_add(1) & TRAIL_MASK;
    cpu.write8(o + AT, at as u8);
    let at = cpu.read8(o + AT) as i8 as i32 as u32;
    mark(cpu, o, at);
    let source = cpu.read32(o + SOURCE);
    if source != 0 && cpu.read8(o + CARRIED) as i8 <= CARRIED_FRAMES {
        let n = cpu.read8(o + NUMBER) as i8 as i32;
        let firer = cpu.read32(NUMBERED.wrapping_add((4 * n) as u32));
        if cpu.read8(firer + OBJ_MODE) as i8 >= ENDING as i8 {
            cpu.write8(o + OBJ_MODE, ENDING);
            return;
        }
        let sx = cpu.read16(source);
        cpu.write16(f + F_SOURCE, sx);
        cpu.write32(f + F_SOURCE_Y_AT, f + F_SOURCE + 2);
        let sy = (cpu.read32(source + 4) >> 16) as u16;
        cpu.write16(f + F_SOURCE + 2, sy);
        let dx = sx.wrapping_sub(cpu.read16(o + SOURCE_AT));
        cpu.write16(f + F_MOVED, dx);
        let dy = sy.wrapping_sub(cpu.read16(o + SOURCE_AT + 2));
        cpu.write16(f + F_MOVED + 2, dy);
        cpu.write16(o + SOURCE_AT, sx);
        cpu.write16(o + SOURCE_AT + 2, sy);
        for (k, d) in [dx, dy].into_iter().enumerate() {
            let p = o + POSITION + 4 * k as u32;
            let v = cpu.read32(p).wrapping_add(((d as i16 as i32) << 16) as u32);
            cpu.write32(p, v);
        }
        for k in 0..TRAIL_LEN {
            for (w, d) in [dx, dy].into_iter().enumerate() {
                let p = o + TRAIL + 4 * k + 2 * w as u32;
                let v = cpu.read16(p).wrapping_add(d);
                cpu.write16(p, v);
            }
        }
        let c = cpu.read8(o + CARRIED).wrapping_add(1);
        cpu.write8(o + CARRIED, c);
    }
    nested!(cpu, 0x0606_3f70, ribbon_edges(cpu, o, at));
    if cpu.read32(o + FLAGS) & NO_HITS == 0 {
        nested!(cpu, 0x0606_4244, ribbon_boxes(cpu, o, at));
    }
}

/// The ended ribbon's behaviour: its head stays, its tail drawn in a place a frame (32 frames).
pub fn ribbon_fade(cpu: &mut Cpu, o: u32) {
    let n = cpu.read32(o + SPEED);
    cpu.write32(o + SPEED, n.wrapping_add(1));
    if n as i32 > TRAIL_MASK as i32 {
        cpu.write8(o + OBJ_MODE, ENDING);
        return;
    }
    let at = cpu.read8(o + AT) as i8 as i32 as u32;
    mark(cpu, o, at);
    nested!(cpu, 0x0606_3f70, ribbon_edges(cpu, o, at));
    if cpu.read32(o + FLAGS) & NO_HITS == 0 {
        nested!(cpu, 0x0606_4244, ribbon_boxes(cpu, o, at));
    }
    let at = (cpu.read8(o + AT) as u32).wrapping_add(1) & TRAIL_MASK;
    cpu.write8(o + AT, at as u8);
}

/// The head moved by its speed along its heading.
pub fn ribbon_move(cpu: &mut Cpu, o: u32) {
    let h = cpu.read16(o + HEADING) as i16 as i32 as u32;
    let s = cpu.call(SL_SIN, &[h]);
    let speed = cpu.read32(o + SPEED);
    let dx = cpu.call(SL_MUL_FX, &[speed, s]);
    let c = cpu.call(SL_COS, &[h]);
    let speed = cpu.read32(o + SPEED);
    let dy = cpu.call(SL_MUL_FX, &[speed, c]);
    let x = cpu.read32(o + POSITION).wrapping_add(dx);
    cpu.write32(o + POSITION, x);
    let y = cpu.read32(o + POSITION + 4).wrapping_add(dy);
    cpu.write32(o + POSITION + 4, y);
}

/// Whether the head is stopped: by what it hit (unless that has an attribute), or the ground.
fn stopped(cpu: &mut Cpu, o: u32) -> bool {
    if cpu.read16(o + HIT_FLAGS) & HIT_SOMETHING != 0 && cpu.read8(o + HIT_FIRST) == 0 {
        return true;
    }
    let (x, y) = (cpu.read32(o + POSITION), cpu.read32(o + POSITION + 4));
    nested!(cpu, 0x0605_ef90, bg::map_hit(cpu, x, y)) != 0
}

fn speed_up(cpu: &mut Cpu, o: u32) {
    let v = cpu.read32(o + SPEED).wrapping_add(cpu.read32(o + ACCEL));
    cpu.write32(o + SPEED, v);
}

/// The straight mover: speeding up.
pub fn ribbon_straight(cpu: &mut Cpu, o: u32) {
    if stopped(cpu, o) {
        nested!(cpu, 0x0606_3940, ribbon_end(cpu, o));
        return;
    }
    speed_up(cpu, o);
    nested!(cpu, 0x0606_3c20, ribbon_move(cpu, o));
}

/// The homing mover: turning toward the nearer player, speeding up.
pub fn ribbon_homing(cpu: &mut Cpu, o: u32) {
    if stopped(cpu, o) {
        nested!(cpu, 0x0606_3940, ribbon_end(cpu, o));
        return;
    }
    nested!(cpu, 0x0606_3ef4, ribbon_home(cpu, o));
    speed_up(cpu, o);
    nested!(cpu, 0x0606_3c20, ribbon_move(cpu, o));
}

/// The waving mover: its speed its base and a sine (its phase on by 0x28f a frame) times its
/// speed up; the base grows by 0x147 a frame; turning toward the nearer player while the two are
/// below 3.5.
const WAVE_STEP: u16 = 0x28f;
const WAVE_GROWTH: u32 = 0x147;
const WAVE_HOMES_BELOW: i32 = 0x3_8000;

pub fn ribbon_wave(cpu: &mut Cpu, o: u32) {
    if stopped(cpu, o) {
        nested!(cpu, 0x0606_3940, ribbon_end(cpu, o));
        return;
    }
    let amp = cpu.read32(o + ACCEL);
    let phase = cpu.read16(o + WAVE_PHASE) as i16 as i32 as u32;
    let s = cpu.call(SL_SIN, &[phase]);
    let wave = cpu.call(SL_MUL_FX, &[s, amp]);
    let speed = wave.wrapping_add(cpu.read32(o + WAVE_BASE));
    cpu.write32(o + SPEED, speed);
    let phase = cpu.read16(o + WAVE_PHASE).wrapping_add(WAVE_STEP);
    cpu.write16(o + WAVE_PHASE, phase);
    let base = cpu.read32(o + WAVE_BASE).wrapping_add(WAVE_GROWTH);
    cpu.write32(o + WAVE_BASE, base);
    if (cpu.read32(o + ACCEL).wrapping_add(base) as i32) < WAVE_HOMES_BELOW {
        nested!(cpu, 0x0606_3ef4, ribbon_home(cpu, o));
    }
    nested!(cpu, 0x0606_3c20, ribbon_move(cpu, o));
}

/// The heading turned toward the nearer player (by its turn) when on screen and further off than
/// its dead angle.
pub fn ribbon_home(cpu: &mut Cpu, o: u32) {
    if cpu.read8(o + OFF) != 0 {
        return;
    }
    let p = nested!(cpu, 0x0605_6a58, aim::nearer_player(cpu, o + 0x10));
    let dx = cpu.read32(p + POSITION).wrapping_sub(cpu.read32(o + POSITION));
    let dy = cpu.read32(p + POSITION + 4).wrapping_sub(cpu.read32(o + POSITION + 4));
    let to = cpu.call(SL_ATAN, &[dy, dx]) as i16;
    let h = cpu.read16(o + HEADING);
    let d = to.wrapping_sub(h as i16);
    if (d as i32).abs() > cpu.read16(o + TURN_WITHIN) as i16 as i32 {
        let turn = cpu.read16(o + TURN);
        let h = if d >= 0 { turn.wrapping_add(h) } else { h.wrapping_sub(turn) };
        cpu.write16(o + HEADING, h);
    }
}

/// The ribbon's edges from the trail's place `i` back: eight quads (two points each), between
/// places 1, 4, 4, 4, 4, 4, 5 and 5 apart, narrowing from 2 pixels either side (4 at the second)
/// to 1. (The SH-2 code leaves three calls' stack arguments pushed at a time, and keeps three of
/// the places in its frame.)
const EDGE_STEPS: [u32; 8] = [1, 4, 4, 4, 4, 4, 5, 5];
const EDGE_WIDTHS: [u32; 8] = [0x2_0000, 0x4_0000, 0x3_8000, 0x3_0000, 0x2_8000, 0x2_0000, 0x1_8000, 0x1_0000];
const QUAD: u32 = 0x18;
const F_RIBBON: u32 = 0;
const F_PLACES: [(usize, u32); 3] = [(0, 4), (3, 8), (6, 0xc)];

pub fn ribbon_edges(cpu: &mut Cpu, o: u32, i: u32) {
    let f = cpu.stack();
    cpu.write32(f + F_RIBBON, o);
    let mut at = i as i8 as i32 as u32;
    let mut from = place(cpu, o, at);
    for (k, (step, width)) in EDGE_STEPS.into_iter().zip(EDGE_WIDTHS).enumerate() {
        at = at.wrapping_sub(step) & TRAIL_MASK;
        if let Some(&(_, slot)) = F_PLACES.iter().find(|&&(n, _)| n == k) {
            cpu.write32(f + slot, at);
            if k > 0 {
                cpu.pop(36);
            }
        }
        let to = place(cpu, o, at);
        let quad = o + POINTS + QUAD * k as u32;
        cpu.push(&[to.1, width, quad]);
        nested!(cpu, 0x0606_4194, ribbon_edge(cpu, o, from.0, from.1, to.0, to.1, width, quad));
        from = to;
    }
    cpu.pop(24);
}

/// One edge: the quad's two points at (x0, y0), `width` either side across the line to (x1,
/// y1) (16.16; half as wide with HALF_WIDTH), or both at it if the two are one.
#[allow(clippy::too_many_arguments)]
pub fn ribbon_edge(cpu: &mut Cpu, o: u32, x0: u32, y0: u32, x1: u32, y1: u32, width: u32, quad: u32) {
    let f = cpu.stack();
    cpu.write32(f, y0);
    let mut width = width as i32;
    if cpu.read8(o + R_FLAGS) & HALF_WIDTH != 0 {
        width -= width >> 1;
    }
    let (dx, dy) = (x0.wrapping_sub(x1), y0.wrapping_sub(y1));
    if dx == 0 && dy == 0 {
        cpu.write32(quad, x0);
        cpu.write32(quad + POINT_SIZE, x0);
        cpu.write32(quad + POINT_SIZE + 4, y0);
        cpu.write32(quad + 4, y0);
        return;
    }
    let a = cpu.call(SL_ATAN, &[dx, dy]) as i16 as i32 as u32;
    let s = cpu.call(SL_SIN, &[a]);
    let s = cpu.call(SL_MUL_FX, &[width as u32, s]);
    let c = cpu.call(SL_COS, &[a]);
    let c = cpu.call(SL_MUL_FX, &[width as u32, c]);
    cpu.write32(quad, x0.wrapping_add(s));
    cpu.write32(quad + POINT_SIZE, x0.wrapping_sub(s));
    cpu.write32(quad + 4, y0.wrapping_sub(c));
    let y = y0.wrapping_add(c);
    cpu.write32(f, y);
    cpu.write32(quad + POINT_SIZE + 4, y);
}

/// The boxes after the head's: at the trail's places 4, 8 and 12 back from `i` (offsets from the
/// head, pixels).
pub fn ribbon_boxes(cpu: &mut Cpu, o: u32, i: u32) {
    let hx = (cpu.read32(o + POSITION) >> 16) as u16;
    let hy = (cpu.read32(o + POSITION + 4) >> 16) as u16;
    let mut at = i;
    for k in 1..BOX_COUNT {
        at = at.wrapping_sub(4) & TRAIL_MASK;
        let b = o + BOX_LIST + BOX_SIZE * k;
        let x = cpu.read16(o + TRAIL + 4 * at).wrapping_sub(hx);
        cpu.write16(b + 2, x);
        let y = cpu.read16(o + TRAIL + 4 * at + 2).wrapping_sub(hy);
        cpu.write16(b + 4, y);
    }
}

/// Makes a ribbon from the description at `d`, moving by mover `kind` (0 straight, 1 homing, 2
/// waving (its phase from 0xc000), 3 for a time), with its own `flags` (ENDS_IN_BURST,
/// HALF_WIDTH).
const RIBBON_SIZE: u32 = 0x26c;
const RIBBON_LIST: u32 = 2;
const ACTOR_EXEC: u32 = 0x0604_d780;
const MOVERS: u32 = 0x0607_3c34;
const WAVING: u32 = 2;
const WAVE_START: u16 = 0xc000;

pub fn ribbon_set(cpu: &mut Cpu, kind: u32, d: u32, flags: u32) {
    let blocks = nested!(cpu, 0x0600_57f8, super::objects::obj_size(cpu, RIBBON_SIZE)) as i8 as i32 as u32;
    let o = nested!(cpu, 0x0600_6b00, super::objects::obj_set(cpu, RIBBON_LIST, blocks, ACTOR_EXEC));
    if o == 0 {
        return;
    }
    let mover = cpu.read32(MOVERS.wrapping_add(4 * kind));
    nested!(cpu, 0x0606_3620, ribbon_init(cpu, o, d, mover));
    if kind == WAVING {
        cpu.write16(o + WAVE_PHASE, WAVE_START);
    }
    cpu.write8(o + R_FLAGS, flags as u8);
}
