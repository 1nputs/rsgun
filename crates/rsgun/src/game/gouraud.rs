//! The models' shading: the game's Gouraud table (32 shades, 4 copies each, the first black)
//! sent to VDP1's VRAM, ramps and floors of it, the light's direction, SGL's Gouraud set-up and
//! its VBlank copy, the polygons' shading bits and tables, and the points' normals (each the sum
//! of its polygons' normals, made unit length).

use super::{math, models, nested, vblank, Cpu};

/// The model data (SGL's polygon data: the points' count, the polygons (20 bytes each:
/// a normal, then four point numbers) and their count, the attributes (12 bytes each: the sort
/// byte at +1, the attributes word at +4, the Gouraud table word at +8)).
const PD_POINT_COUNT: u32 = 4;
const PD_POLYGONS: u32 = 8;
const PD_COUNT: u32 = 0xc;
const PD_ATTRS: u32 = 0x10;
const POLYGON_SIZE: u32 = 20;
const POLYGON_POINTS: u32 = 12;
const ATTR_SIZE: u32 = 12;
const ATTR_SORT: u32 = 1;
const ATTR_BITS: u32 = 4;
const ATTR_GOURAUD_TABLE: u32 = 8;

/// The models (both of each: plain and flashing) of kind `kind`'s first `count` frames given
/// SGL's high-speed shrink (HSSon).
const HSS_ON: u16 = 0x1000;

pub fn models_hss(cpu: &mut Cpu, kind: u32, count: u32) {
    let mut i = 0u32;
    while (i as i32) < count as i32 {
        let at = models::kind_model(cpu, kind, i);
        let (plain, flashing) = (cpu.read32(at), cpu.read32(at + 4));
        let mut j = 0;
        while j < cpu.read32(plain + PD_COUNT) {
            for pd in [plain, flashing] {
                let a = cpu.read32(pd + PD_ATTRS) + ATTR_SIZE * j + ATTR_BITS;
                let v = cpu.read16(a) | HSS_ON;
                cpu.write16(a, v);
            }
            j += 1;
        }
        i += 1;
    }
}

/// SGL's light set to a point straight ahead turned by `x`, `y` and `z` (angles).
const SL_PUSH_UNIT_MATRIX: u32 = 0x0603_511a;
const SL_POP_MATRIX: u32 = 0x0606_7200;
const SL_ROT_X: u32 = 0x0606_72ec;
const SL_ROT_Y: u32 = 0x0606_736c;
const SL_ROT_Z: u32 = 0x0606_726c;
const SL_CALC_POINT: u32 = 0x0606_db2c;
const SL_LIGHT: u32 = 0x0607_0f8c;
const LIGHT_AHEAD: u32 = 0xf0a3;

pub fn light_turned(cpu: &mut Cpu, x: u32, y: u32, z: u32) {
    let f = cpu.stack();
    cpu.call(SL_PUSH_UNIT_MATRIX, &[]);
    for (a, rot) in [(x, SL_ROT_X), (y, SL_ROT_Y), (z, SL_ROT_Z)] {
        cpu.call(rot, &[a as i16 as i32 as u32]);
    }
    cpu.call(SL_CALC_POINT, &[0, 0, LIGHT_AHEAD, f]);
    cpu.call(SL_LIGHT, &[f]);
    cpu.call(SL_POP_MATRIX, &[]);
}

/// The table (32 entries of 4 words) and where it goes in VDP1's VRAM (all but the first entry,
/// sent once by a VBlank transfer).
const TABLE: u32 = 0x0607_5c30;
const SHADES: u32 = 32;
const SHADE_SIZE: u32 = 8;
const TABLE_SENT: u32 = 0x0607_48c0;
const TABLE_VRAM: u32 = 0x25c7_ff08;
const TABLE_SENT_SIZE: u32 = 0xf8;
const SL_TRANSFER_ENTRY: u32 = 0x0603_4a50;

/// The table's shades 1-31 from the colours at `colours` (words; shade 0 black).
pub fn table_set(cpu: &mut Cpu, colours: u32) {
    for k in 0..4 {
        cpu.write16(TABLE + 2 * k, 0);
    }
    for i in 1..SHADES {
        let v = cpu.read16(colours + 2 * i);
        for k in 0..4 {
            cpu.write16(TABLE + SHADE_SIZE * i + 2 * k, v);
        }
    }
    if cpu.read32(TABLE_SENT) == 0 {
        cpu.write32(TABLE_SENT, 1);
        cpu.call(SL_TRANSFER_ENTRY, &[TABLE + SHADE_SIZE, TABLE_VRAM, TABLE_SENT_SIZE]);
    }
}

/// An RGB555 colour's parts (r, g, b) and the colour of parts.
fn parts(c: u16) -> (u16, u16, u16) {
    (c & 0x1f, (c >> 5) & 0x1f, (c >> 10) & 0x1f)
}

fn colour(r: u16, g: u16, b: u16) -> u16 {
    r | (b << 10) | (g << 5)
}

/// The table's shades raised to at least `r`, `g`, `b` each.
pub fn table_floor(cpu: &mut Cpu, r: u32, g: u32, b: u32) {
    let f = cpu.stack();
    let (r, g, b) = (r as u16, g as u16, b as u16);
    for i in 0..SHADES {
        let (cr, cg, cb) = parts(cpu.read16(TABLE + SHADE_SIZE * i));
        cpu.write16(f + 2 * i, colour(cr.max(r), cg.max(g), cb.max(b)));
    }
    nested!(cpu, 0x0606_3060, table_set(cpu, f));
}

/// The table a ramp from black to `r`, `g`, `b` (each shade a 32nd further).
const RAMP_STEP: u32 = 0x800;

pub fn table_ramp(cpu: &mut Cpu, r: u32, g: u32, b: u32) {
    let f = cpu.stack();
    let ends = [r & 0xffff, g & 0xffff, b & 0xffff];
    let mut t = 0u32;
    for i in 0..SHADES {
        let [cr, cg, cb] = ends.map(|e| nested!(cpu, 0x0606_2e7c, math::lerp(cpu, 0, e << 16, t)) >> 16);
        cpu.write16(f + 2 * i, (cr | (cb << 10) | (cg << 5)) as u16);
        t = t.wrapping_add(RAMP_STEP);
    }
    nested!(cpu, 0x0606_3060, table_set(cpu, f));
}

/// The default shading: a ramp to 24s, floored at 8s, the light from above and the left.
pub fn shading_default(cpu: &mut Cpu) {
    nested!(cpu, 0x0606_3168, table_ramp(cpu, 0x18, 0x18, 0x18));
    nested!(cpu, 0x0606_30e0, table_floor(cpu, 8, 8, 8));
    let a = 0xe000u16 as i16 as i32 as u32;
    nested!(cpu, 0x0606_2fe8, light_turned(cpu, a, a, 0));
}

/// SGL's Gouraud set-up (its work, 600 tables, from the next VDP1 place, its buffer) and its
/// copy each VBlank; the copy's end.
const GOURAUD_AT: u32 = 0x0607_5c20;
const VDP1_NEXT_PLACE: u32 = 0x0607_4058;
const SL_GOURAUD_WORK: u32 = 0x0607_48d0;
const SL_GOURAUD_TABLES: u32 = 600;
const SL_GOURAUD_BUFFER: u32 = 0x0607_5ba0;
const SL_INIT_GOURAUD: u32 = 0x0607_0fbc;
const SL_GOURAUD_COPY: u32 = 0x0607_10f6;

pub fn shading_init(cpu: &mut Cpu) {
    let at = cpu.read32(VDP1_NEXT_PLACE);
    cpu.write32(GOURAUD_AT, at);
    cpu.call(SL_INIT_GOURAUD, &[SL_GOURAUD_WORK, SL_GOURAUD_TABLES, at >> 3, SL_GOURAUD_BUFFER]);
    nested!(cpu, 0x0600_43d4, vblank::add_callback(cpu, SL_GOURAUD_COPY));
}

pub fn shading_end(cpu: &mut Cpu) {
    nested!(cpu, 0x0600_4408, vblank::remove_callback(cpu, SL_GOURAUD_COPY));
}

/// Kind `kind`'s first `count` models lit.
pub fn models_lit(cpu: &mut Cpu, kind: u32, count: u32) {
    let mut i = 0u32;
    while (i as i32) < count as i32 {
        nested!(cpu, 0x0606_333c, models::model_lit(cpu, kind, i));
        i += 1;
    }
}

/// Both models' polygons' sort (the sort byte's low two bits) set to `sort`.
pub fn model_sort(cpu: &mut Cpu, kind: u32, frame: u32, sort: u32) {
    let at = models::kind_model(cpu, kind, frame);
    let (plain, flashing) = (cpu.read32(at), cpu.read32(at + 4));
    let sort = sort as u8;
    let mut j = 0;
    while (j as i32) < cpu.read32(plain + PD_COUNT) as i32 {
        for pd in [plain, flashing] {
            let a = cpu.read32(pd + PD_ATTRS) + ATTR_SIZE * j + ATTR_SORT;
            let v = cpu.read8(a) & 0xfc;
            cpu.write8(a, v);
            let a = cpu.read32(pd + PD_ATTRS) + ATTR_SIZE * j + ATTR_SORT;
            let v = cpu.read8(a) | sort;
            cpu.write8(a, v);
        }
        j += 1;
    }
}

/// Each point's normal (to `out`, x, y, z each): its polygons' normals added, made unit length.
const SL_SQRT: u32 = 0x0607_14a8;
const SL_DIV: u32 = 0x0607_14bc;

pub fn point_normals(cpu: &mut Cpu, pd: u32, out: u32) {
    let (points, polygons) = (cpu.read32(pd + PD_POINT_COUNT), cpu.read32(pd + PD_COUNT));
    let mut out = out;
    let mut i = 0u32;
    while (i as i32) < points as i32 {
        let mut sum = [0u32; 3];
        let mut p = cpu.read32(pd + PD_POLYGONS);
        let mut j = 0u32;
        while (j as i32) < polygons as i32 {
            let uses = (0..4).any(|k| cpu.read16(p + POLYGON_POINTS + 2 * k) as u32 == i);
            if uses {
                for (k, s) in sum.iter_mut().enumerate() {
                    *s = s.wrapping_add(cpu.read32(p + 4 * k as u32));
                }
            }
            p += POLYGON_SIZE;
            j += 1;
        }
        let [x, y, z] = sum;
        let xx = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, x, x));
        let yy = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, y, y));
        let zz = nested!(cpu, 0x0606_e26c, math::sl_mul_fx(cpu, z, z));
        let length = cpu.call(SL_SQRT, &[xx.wrapping_add(yy).wrapping_add(zz)]);
        for v in [x, y, z] {
            let n = cpu.call(SL_DIV, &[length, v]);
            cpu.write32(out, n);
            out += 4;
        }
        i += 1;
    }
}

/// The polygons' Gouraud tables: the `i`th polygon's the `i`th from `at` (in VDP1's VRAM).
pub fn gouraud_tables(cpu: &mut Cpu, pd: u32, at: u32) {
    let first = (at & 0xf_ffff) >> 3;
    let (count, attrs) = (cpu.read32(pd + PD_COUNT), cpu.read32(pd + PD_ATTRS));
    let mut i = 0u32;
    while (i as i32) < count as i32 {
        cpu.write16(attrs + ATTR_SIZE * i + ATTR_GOURAUD_TABLE, (first + i) as u16);
        i += 1;
    }
}
