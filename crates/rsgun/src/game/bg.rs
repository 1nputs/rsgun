//! The backgrounds, as the stages' code drives them: the scroll planes NBG0 and NBG1 moving at
//! their speeds (sent to SGL each frame), the ground maps streamed into a plane's cells as the
//! plane scrolls and the hit test against their cells' shapes, the 3D view (along a path of keys,
//! as the view matrix) with the models placed around each key, the rotation scroll's matrix, and
//! the line windows that cut the screen along a slanted edge.

use super::{math, nested, screen, Cpu};

/// SGL's.
const SL_MUL_FX: u32 = 0x0606_e26c;
const SL_TAN: u32 = 0x0606_eadc;
const SL_ATAN: u32 = 0x0606_780c;
/// slScrPosNbg0/1 and slZoomNbg0/1 (by the work fields they set).
const SL_SCR_POS_NBG0: u32 = 0x0606_e47c;
const SL_ZOOM_NBG0: u32 = 0x0606_e46c;
const SL_SCR_POS_NBG1: u32 = 0x0606_e4cc;
const SL_ZOOM_NBG1: u32 = 0x0606_e4bc;
const SL_UNIT_MATRIX: u32 = 0x0603_512e;
const SL_PUSH_MATRIX: u32 = 0x0606_71cc;
const SL_POP_MATRIX: u32 = 0x0606_7200;
const SL_TRANSLATE: u32 = 0x0606_721c;
const SL_ROT_X: u32 = 0x0606_72ec;
const SL_ROT_Y: u32 = 0x0606_736c;
const SL_ROT_Z: u32 = 0x0606_726c;
const SL_SCALE: u32 = 0x0606_73ec;
const SL_PUT_POLYGON: u32 = 0x0606_745c;
/// By its use: negative when a point (and a size around it) is off screen.
const SL_CHECK_ON_SCREEN: u32 = 0x0606_e9fc;
/// The rotation scroll's parameters from the current matrix (two steps, not known yet).
const SL_0606E4DC: u32 = 0x0606_e4dc;
const SL_0606E50C: u32 = 0x0606_e50c;
/// Called with 0 before the view's models are drawn and 1 after (not known yet).
const SL_0606713C: u32 = 0x0606_713c;

const ONE: u32 = 0x10000;
/// Half the screen (16.16): the planes' positions and the hit test's points are from its centre.
const HALF_WIDTH: u32 = 0x00b0_0000;
const HALF_HEIGHT: u32 = 0x0070_0000;

/// Whether points hit the ground: 0 nothing does (each frame starts so), 1 the ground map at
/// MAP_HIT_MAP (MAP_HIT_X its horizontal scroll), -1 the function at MAP_HIT_FN says.
const MAP_HIT_MODE: u32 = 0x0607_47c8;
const MAP_HIT_X: u32 = 0x0607_4840;
const MAP_HIT_MAP: u32 = 0x0607_4844;
const MAP_HIT_FN: u32 = 0x0607_3ff8;
const MAP_POINT: u32 = 0x0605_f28c;

/// Nothing hits the ground.
pub fn map_hit_off(cpu: &mut Cpu) {
    cpu.write32(MAP_HIT_MODE, 0);
}

/// Points hit the ground map `map`, scrolled by `x`.
pub fn map_hit_on(cpu: &mut Cpu, x: u32, map: u32) {
    cpu.write32(MAP_HIT_X, x);
    cpu.write32(MAP_HIT_MAP, map);
    cpu.write32(MAP_HIT_MODE, 1);
}

/// Points hit the ground as `f` (x, y) says.
pub fn map_hit_by(cpu: &mut Cpu, f: u32) {
    cpu.write32(MAP_HIT_FN, f);
    cpu.write32(MAP_HIT_MODE, u32::MAX);
}

/// Whether the point (`x`, `y`: 16.16 from the screen's centre) hits the ground.
pub fn map_hit(cpu: &mut Cpu, x: u32, y: u32) -> u32 {
    let mode = cpu.read32(MAP_HIT_MODE) as i32;
    if mode == 0 {
        0
    } else if mode > 0 {
        let (scroll, map) = (cpu.read32(MAP_HIT_X), cpu.read32(MAP_HIT_MAP));
        nested!(cpu, MAP_POINT, map_point(cpu, x, y, scroll, map))
    } else {
        let f = cpu.read32(MAP_HIT_FN);
        cpu.call(f, &[x, y])
    }
}

/// A ground map: its length (in blocks of 512 lines), set once all of it is in the plane, the
/// lines it keeps in the plane ahead of the screen, where the next rows go in the plane (bytes,
/// within the plane's size less one), the screen's place in it and how far it is in the plane
/// (16.16 lines, counting up as the plane scrolls down), the plane's cells (VDP2 VRAM), each
/// block's cells (64 by 64, the last row first; -1 for none), the empty cell (in place of a
/// block that is none), and its cells' shapes.
const BLOCKS: u32 = 0x00;
const LOADED_ALL: u32 = 0x01;
const AHEAD: u32 = 0x02;
const PLANE_AT: u32 = 0x04;
const PLANE_MASK: u32 = 0x06;
const PLACE: u32 = 0x08;
const LOADED: u32 = 0x0c;
const PLANE_CELLS: u32 = 0x10;
const BLOCK_TABLE: u32 = 0x14;
const EMPTY: u32 = 0x18;
const SHAPES: u32 = 0x1c;
/// A block's lines (as a shift), its last row's cells, a row of cells (64, in bytes).
const BLOCK_SHIFT: u32 = 9;
const BLOCK_LAST_ROW: i32 = 0x1f80;
const BLOCK_SIZE: i32 = 0x2000;
const ROW_BYTES: i32 = 0x80;
/// The cells' shapes: the cells' format (0: 12-bit character numbers; else 10-bit, with the
/// horizontal and vertical flips in bits 10 and 11), each character's shape, the shapes (8 bytes
/// each, a row a byte, the leftmost pixel the top bit).
const SHAPE_FORMAT: u32 = 0x00;
const SHAPE_OF: u32 = 0x04;
const SHAPE_BITS: u32 = 0x08;
const FLIP_H: u32 = 1;
const FLIP_V: u32 = 2;

/// Whether the point (`x`, `y`) is solid in the ground map `map`, scrolled by `scroll`.
pub fn map_point(cpu: &mut Cpu, x: u32, y: u32, scroll: u32, map: u32) -> u32 {
    let length = (cpu.read8(map + BLOCKS) as i8 as i32) << 25;
    let line = cpu.read32(map + PLACE).wrapping_sub(y.wrapping_add(HALF_HEIGHT));
    if line as i32 >= length {
        return 0;
    }
    let line = (line >> 16) as i16 as i32;
    let column = (x.wrapping_add(HALF_WIDTH).wrapping_add(scroll) >> 16) as i16 as i32;
    let table = cpu.read32(map + BLOCK_TABLE);
    let block = cpu.read32(table.wrapping_add(((line >> BLOCK_SHIFT) * 4) as u32));
    if block == u32::MAX {
        return 0;
    }
    let at = (column >> 3) * 2 + BLOCK_LAST_ROW - (line & 0x1f8) * 16;
    let cell = cpu.read16(block.wrapping_add(at as u32)) as u32;
    let (row, bit) = (7 - (line & 7) as u32, column as u32 & 7);
    let shapes = cpu.read32(map + SHAPES);
    let (shape_of, bits) = (cpu.read32(shapes + SHAPE_OF), cpu.read32(shapes + SHAPE_BITS));
    let (byte, mask) = if cpu.read8(shapes + SHAPE_FORMAT) == 0 {
        let shape = cpu.read8(shape_of.wrapping_add(cell & 0xfff)) as u32;
        (cpu.read8(bits.wrapping_add(shape * 8 + row)) as u32, 0x80 >> bit)
    } else {
        let shape = cpu.read8(shape_of.wrapping_add(cell & 0x3ff)) as u32;
        let flips = cell >> 10 & 3;
        let mask = if flips & FLIP_H != 0 { 1 << bit } else { 0x80 >> bit };
        let row = if flips & FLIP_V != 0 { 7 - row } else { row };
        (cpu.read8(bits.wrapping_add(shape * 8 + row)) as u32, mask)
    };
    (byte & mask != 0) as u32
}

/// Clears the ground map's plane: every cell the empty one (queued for the VBlank).
pub fn map_clear(cpu: &mut Cpu, map: u32) {
    let (plane, size) = (cpu.read32(map + PLANE_CELLS), cpu.read16(map + PLANE_MASK) as u32 + 1);
    nested!(cpu, 0x0605_ecb4, screen::queue_fill(cpu, map + EMPTY, plane, size));
}

/// Moves the ground map with its plane's vertical speed and queues the rows it needs next into
/// the plane (each a row of cells, 8 lines; the empty cell where a block is none): -1 once the map
/// has ended, else 0.
pub fn map_stream(cpu: &mut Cpu, plane: u32, map: u32) -> u32 {
    let length = (cpu.read8(map + BLOCKS) as i8 as i32) << 25;
    let place = cpu.read32(map + PLACE).wrapping_sub(cpu.read32(plane + SPEED_Y));
    cpu.write32(map + PLACE, place);
    if place as i32 >= length {
        cpu.write32(map + PLACE, length as u32);
        return u32::MAX;
    }
    if cpu.read8(map + LOADED_ALL) != 0 {
        return 0;
    }
    let loaded = cpu.read32(map + LOADED);
    let ahead = (loaded.wrapping_sub(place) >> 16) as i16 as i32;
    let wanted = cpu.read16(map + AHEAD) as i16 as i32;
    if ahead >= wanted {
        return 0;
    }
    let rows = ((wanted - ahead) >> 3) + 1;
    let loaded = (rows << 19).wrapping_add(loaded as i32);
    cpu.write32(map + LOADED, loaded as u32);
    if loaded >= length {
        cpu.write32(map + LOADED, length as u32);
        cpu.write8(map + LOADED_ALL, 1);
    }
    let line = (cpu.read32(map + LOADED) >> 16) as i16 as i32;
    let bytes = (rows * ROW_BYTES) as u32;
    // the rows end at the line loaded to: in its block, or (at a block's start) the block before
    let table = cpu.read32(map + BLOCK_TABLE);
    let src = if line & 0x1ff == 0 {
        let block = cpu.read32(table.wrapping_add((((line >> BLOCK_SHIFT) - 1) * 4) as u32));
        (block != u32::MAX).then_some(block)
    } else {
        let block = cpu.read32(table.wrapping_add(((line >> BLOCK_SHIFT) * 4) as u32));
        (block != u32::MAX).then(|| block.wrapping_sub(((line & 0x1f8) * 16 - BLOCK_SIZE) as u32))
    };
    let at = (cpu.read16(map + PLANE_AT) as u32).wrapping_sub(bytes) & cpu.read16(map + PLANE_MASK) as u32;
    cpu.write16(map + PLANE_AT, at as u16);
    let dst = at.wrapping_add(cpu.read32(map + PLANE_CELLS));
    match src {
        Some(src) => nested!(cpu, 0x0605_ecf0, screen::queue_copy(cpu, src, dst, bytes)),
        None => nested!(cpu, 0x0605_ecb4, screen::queue_fill(cpu, map + EMPTY, dst, bytes)),
    };
    0
}

/// The scroll planes NBG0 (A) and NBG1 (B): the screen's centre on the plane, its speed (16.16
/// a frame each), and the zoom (16.16, 1 the plane's own size).
const PLANE_A: u32 = 0x0607_47e0;
const PLANE_B: u32 = 0x0607_47b0;
const X: u32 = 0x00;
const Y: u32 = 0x04;
const SPEED_X: u32 = 0x08;
const SPEED_Y: u32 = 0x0c;
const ZOOM_X: u32 = 0x10;
const ZOOM_Y: u32 = 0x14;
const PLANE_SPEED: u32 = 0x0605_f01c;
const PLANE_STEP: u32 = 0x0605_f028;
const PLANE_CORNER: u32 = 0x0605_f0d8;

/// Plane A's speed.
pub fn plane_a_speed(cpu: &mut Cpu, dx: u32, dy: u32) {
    nested!(cpu, PLANE_SPEED, plane_speed(cpu, PLANE_A, dx, dy));
}

/// Plane B's speed.
pub fn plane_b_speed(cpu: &mut Cpu, dx: u32, dy: u32) {
    nested!(cpu, PLANE_SPEED, plane_speed(cpu, PLANE_B, dx, dy));
}

/// A plane's speed.
pub fn plane_speed(cpu: &mut Cpu, plane: u32, dx: u32, dy: u32) {
    cpu.write32(plane + SPEED_X, dx);
    cpu.write32(plane + SPEED_Y, dy);
}

/// Moves a plane by its speed.
pub fn plane_step(cpu: &mut Cpu, plane: u32) {
    let x = cpu.read32(plane + X).wrapping_add(cpu.read32(plane + SPEED_X));
    cpu.write32(plane + X, x);
    let y = cpu.read32(plane + Y).wrapping_add(cpu.read32(plane + SPEED_Y));
    cpu.write32(plane + Y, y);
}

/// Plane A's frame: moved, and its zoom and top left corner set for NBG0.
pub fn plane_a_frame(cpu: &mut Cpu) {
    plane_frame(cpu, PLANE_A, SL_ZOOM_NBG0, SL_SCR_POS_NBG0);
}

/// Plane B's frame, for NBG1.
pub fn plane_b_frame(cpu: &mut Cpu) {
    plane_frame(cpu, PLANE_B, SL_ZOOM_NBG1, SL_SCR_POS_NBG1);
}

fn plane_frame(cpu: &mut Cpu, plane: u32, zoom: u32, position: u32) {
    nested!(cpu, PLANE_STEP, plane_step(cpu, plane));
    let corner = cpu.stack();
    nested!(cpu, PLANE_CORNER, plane_corner(cpu, plane, corner));
    let (zx, zy) = (cpu.read32(plane + ZOOM_X), cpu.read32(plane + ZOOM_Y));
    cpu.call(zoom, &[zx, zy]);
    let (x, y) = (cpu.read32(corner), cpu.read32(corner + 4));
    cpu.call(position, &[x, y]);
}

/// The plane's top left corner on screen, at `out` (x, y): its centre less half the screen,
/// zoomed. The SH-2 code takes the horizontal zoom's being 1 for both.
pub fn plane_corner(cpu: &mut Cpu, plane: u32, out: u32) {
    let unzoomed = cpu.read32(plane + ZOOM_X) == ONE;
    let left = if unzoomed {
        HALF_WIDTH
    } else {
        let zx = cpu.read32(plane + ZOOM_X);
        cpu.call(SL_MUL_FX, &[zx, HALF_WIDTH])
    };
    let left = cpu.read32(plane + X).wrapping_sub(left);
    let top = if cpu.read32(plane + ZOOM_X) == ONE {
        HALF_HEIGHT
    } else {
        let zy = cpu.read32(plane + ZOOM_Y);
        cpu.call(SL_MUL_FX, &[zy, HALF_HEIGHT])
    };
    let top = cpu.read32(plane + Y).wrapping_sub(top);
    cpu.write32(out, left);
    cpu.write32(out + 4, top);
}

/// Something with a position (+0x0c, three longs) moved by a speed it keeps (+0x18), at MOVER
/// (not known yet).
const MOVER: u32 = 0x0607_4810;
const MOVER_POSITION: u32 = 0x0c;
const MOVER_SPEED: u32 = 0x18;

/// MOVER's speed, and a move by it.
pub fn mover_move(cpu: &mut Cpu, dx: u32, dy: u32, dz: u32) {
    nested!(cpu, 0x0605_f3ec, move_by(cpu, MOVER, dx, dy, dz));
}

/// Sets the speed of `s` and moves it by it.
pub fn move_by(cpu: &mut Cpu, s: u32, dx: u32, dy: u32, dz: u32) {
    for (i, d) in [dx, dy, dz].into_iter().enumerate() {
        let i = 4 * i as u32;
        cpu.write32(s + MOVER_SPEED + i, d);
        let p = cpu.read32(s + MOVER_POSITION + i).wrapping_add(d);
        cpu.write32(s + MOVER_POSITION + i, p);
    }
}

/// The 3D view: its place on its path (16.16: the key, and the way on to the next), its position
/// (x, y, z) and its angles (x, y, z; 16-bit, in the low halves of longs).
const VIEW: u32 = 0x0607_4750;
const VIEW_POSITION: u32 = 0x04;
const VIEW_ANGLES: u32 = 0x10;
/// A path's key: the view's position and angles (six longs).
const KEY: u32 = 24;
/// An object's position (Base_obj's).
const POSITION: u32 = 0x14;

fn view_angle(cpu: &mut Cpu, i: u32) -> u32 {
    cpu.read16(VIEW + VIEW_ANGLES + 4 * i + 2) as i16 as i32 as u32
}

fn view_position(cpu: &mut Cpu, i: u32) -> u32 {
    cpu.read32(VIEW + VIEW_POSITION + 4 * i)
}

/// Turns the view toward the object `target`: its x angle by the height over the depth, its z
/// angle by the sideways distance.
pub fn view_toward(cpu: &mut Cpu, target: u32) {
    let dx = cpu.read32(target + POSITION).wrapping_sub(view_position(cpu, 0));
    let dy = cpu.read32(target + POSITION + 4).wrapping_sub(view_position(cpu, 1));
    let dz = view_position(cpu, 2).wrapping_sub(cpu.read32(target + POSITION + 8));
    let up = cpu.call(SL_ATAN, &[(dy as i32).wrapping_abs() as u32, dz]) as i16 as i32;
    cpu.write32(VIEW + VIEW_ANGLES, (-0x4000 - up) as u32);
    let side = cpu.call(SL_ATAN, &[dy.wrapping_neg(), dx.wrapping_neg()]) as i16 as i32;
    cpu.write32(VIEW + VIEW_ANGLES + 8, side as u32);
}

/// The view matrix: turned by the view's angles (x, y, z), then moved by its position.
pub fn view_matrix(cpu: &mut Cpu) {
    cpu.call(SL_UNIT_MATRIX, &[0]);
    let x = view_angle(cpu, 0);
    cpu.call(SL_ROT_X, &[x]);
    let y = view_angle(cpu, 1);
    cpu.call(SL_ROT_Y, &[y]);
    let z = view_angle(cpu, 2);
    cpu.call(SL_ROT_Z, &[z]);
    view_translate(cpu);
}

/// The other view matrix: moved by the view's position, then turned about y by its third angle,
/// about x by its first and about z by its second.
pub fn view_matrix_moved_first(cpu: &mut Cpu) {
    cpu.call(SL_UNIT_MATRIX, &[0]);
    view_translate(cpu);
    let third = view_angle(cpu, 2);
    cpu.call(SL_ROT_Y, &[third]);
    let first = view_angle(cpu, 0);
    cpu.call(SL_ROT_X, &[first]);
    let second = view_angle(cpu, 1);
    cpu.call(SL_ROT_Z, &[second]);
}

fn view_translate(cpu: &mut Cpu) {
    let p = [0, 1, 2].map(|i| view_position(cpu, i).wrapping_neg());
    cpu.call(SL_TRANSLATE, &p);
}

/// The rotation scroll's matrix from `o`: moved by its position (+0x0c), turned by its angles
/// (words x, y, z at +0; about y, x, then z) and scaled by its scale (+0x24).
pub fn rotation_matrix(cpu: &mut Cpu, o: u32) {
    cpu.call(SL_PUSH_MATRIX, &[]);
    let p = [0, 1, 2].map(|i| cpu.read32(o + 0x0c + 4 * i));
    cpu.call(SL_TRANSLATE, &p);
    let [x, y, z] = [0, 1, 2].map(|i| cpu.read16(o + 2 * i) as i16 as i32 as u32);
    cpu.call(SL_ROT_Y, &[y]);
    cpu.call(SL_ROT_X, &[x]);
    cpu.call(SL_ROT_Z, &[z]);
    let s = [0, 1, 2].map(|i| cpu.read32(o + 0x24 + 4 * i));
    cpu.call(SL_SCALE, &s);
    cpu.call(SL_0606E4DC, &[]);
    cpu.call(SL_0606E50C, &[]);
    cpu.call(SL_POP_MATRIX, &[]);
}

/// The view's position and angles at its place on `path` (keys of six longs), from its key to
/// the next in a line.
pub fn view_along(cpu: &mut Cpu, path: u32) {
    view_between_keys(cpu, path, false);
}

/// view_along, eased into and out of each key.
pub fn view_along_eased(cpu: &mut Cpu, path: u32) {
    view_between_keys(cpu, path, true);
}

fn view_between_keys(cpu: &mut Cpu, path: u32, eased: bool) {
    let place = cpu.read32(VIEW);
    let key = (place >> 16) as i16 as i32;
    let t = place & 0xffff;
    let from = path.wrapping_add(key.wrapping_mul(KEY as i32) as u32);
    let to = from.wrapping_add(KEY);
    for i in 0..6 {
        let (a, b) = (cpu.read32(from + 4 * i), cpu.read32(to + 4 * i));
        let v = if eased {
            nested!(cpu, 0x0606_2ebc, math::ease(cpu, a, b, t))
        } else {
            nested!(cpu, 0x0606_2e7c, math::lerp(cpu, a, b, t))
        };
        cpu.write32(VIEW + VIEW_POSITION + 4 * i, v);
    }
}

/// Draws the models placed around the view's key: `table` holds each key's (count, list); the
/// key before (if any), the key and the next are drawn.
pub fn draw_models_near(cpu: &mut Cpu, table: u32) {
    cpu.call(SL_0606713C, &[0]);
    let key = cpu.read16(VIEW) as i16 as i32;
    if key > 0 {
        let e = table.wrapping_add(((key - 1) * 8) as u32);
        let (n, list) = (cpu.read32(e), cpu.read32(e + 4));
        nested!(cpu, DRAW_MODELS, draw_models(cpu, n, list));
    }
    let e = table.wrapping_add((key * 8) as u32);
    let (n, list) = (cpu.read32(e), cpu.read32(e + 4));
    nested!(cpu, DRAW_MODELS, draw_models(cpu, n, list));
    let (n, list) = (cpu.read32(e + 8), cpu.read32(e + 12));
    nested!(cpu, DRAW_MODELS, draw_models(cpu, n, list));
    cpu.call(SL_0606713C, &[1]);
}
const DRAW_MODELS: u32 = 0x0605_f794;

/// A placed model: its position (x, y, z), its angles (words: x, y, z) and its model (its size,
/// then its polygons).
const PLACED: u32 = 24;
const PLACED_ANGLES: u32 = 0x0c;
const PLACED_MODEL: u32 = 0x14;

/// Draws the `n` models of `list` that are on screen, each moved and turned (about y, x, z; an
/// angle of 0 not turned by).
pub fn draw_models(cpu: &mut Cpu, n: u32, list: u32) {
    // the SH-2 code keeps the count in its frame
    let frame = cpu.stack();
    cpu.write32(frame, n);
    for i in 0..(n as i32).max(0) as u32 {
        let m = list.wrapping_add(PLACED * i);
        let model = cpu.read32(m + PLACED_MODEL);
        let size = cpu.read32(model);
        if (cpu.call(SL_CHECK_ON_SCREEN, &[m, size]) as i32) < 0 {
            continue;
        }
        cpu.call(SL_PUSH_MATRIX, &[]);
        let p = [0, 1, 2].map(|k| cpu.read32(m + 4 * k));
        cpu.call(SL_TRANSLATE, &p);
        for (k, rot) in [(1, SL_ROT_Y), (0, SL_ROT_X), (2, SL_ROT_Z)] {
            let a = cpu.read16(m + PLACED_ANGLES + 2 * k) as i16;
            if a != 0 {
                cpu.call(rot, &[a as i32 as u32]);
            }
        }
        let polygons = cpu.read32(model + 4);
        cpu.call(SL_PUT_POLYGON, &[polygons]);
        cpu.call(SL_POP_MATRIX, &[]);
    }
}

/// The screen's height (16.16).
const SCREEN_HEIGHT: u32 = 0x00e0_0000;

/// A line for the angle `angle` (16-bit; not known yet): the screen's height times the whole part
/// of 1 - (angle - 0xa800) / 3, a screen further down to -0x5800 and a screen up past -0x2800.
pub fn edge_line(cpu: &mut Cpu, angle: u32) -> u32 {
    let a = angle as i16 as i32;
    let third = cpu.call(SL_MUL_FX, &[0x0005_5555, (a.wrapping_sub(0xa800) << 16) as u32]);
    let part = ONE.wrapping_sub(third) >> 16;
    let line = cpu.call(SL_MUL_FX, &[SCREEN_HEIGHT, part]);
    if a <= -0x5800 {
        line.wrapping_add(SCREEN_HEIGHT)
    } else if a > -0x2800 {
        line.wrapping_sub(SCREEN_HEIGHT)
    } else {
        line
    }
}

/// A line window's entry: start and end (twice the pixel) in the high and low halves; the whole
/// line, none.
const WHOLE_LINE: u32 = 0x2be;
const NO_LINE: u32 = u32::MAX;
const CENTRE: i32 = 0xb0;
const RIGHT: i32 = 0x15f;
const LAST_LINE: i32 = 0xdf_0000;
const LAST_ENTRY: i32 = 0xde;

/// The line window `table` (a long a line) for the screen on one side of an edge through the
/// screen's centre column at line `y` (16.16), at `angle` (16-bit, to a 64th): the lines above
/// it whole, those below it none, and between them the part of each on that side (past a quarter
/// turn the other way round).
pub fn edge_window(cpu: &mut Cpu, table: u32, y: u32, angle: u32) {
    let a = (angle as i16 as i32) & !0x3f;
    let tan = cpu.call(SL_TAN, &[a as u32]);
    let w = cpu.call(SL_MUL_FX, &[HALF_WIDTH, tan]) as i32;
    let y = y as i32;
    let turned = (a + 0x3fff) as u32 & 0xffff > 0x7fff;
    let (before, after) = if turned { (NO_LINE, WHOLE_LINE) } else { (WHOLE_LINE, NO_LINE) };
    let (first, last) = if (a >= 0) != turned { (y.wrapping_sub(w), y.wrapping_add(w)) } else { (y.wrapping_add(w), y.wrapping_sub(w)) };
    let mut line = 0;
    if first > 0 {
        let n = (first >> 16) as i16 as i32;
        while line < n {
            cpu.write32(table.wrapping_add(4 * line as u32), before);
            line += 1;
        }
    }
    let last = last.min(LAST_LINE);
    let slope_angle = match (turned, a >= 0) {
        (false, true) => 0x4000 - a,
        (true, true) => a - 0x4000,
        (_, false) => 0x4000 + a,
    };
    let slope = cpu.call(SL_TAN, &[slope_angle as i16 as i32 as u32]);
    // the SH-2 code keeps it in its frame (its first long, or past a quarter turn its second)
    let frame = cpu.stack() + if turned { 4 } else { 0 };
    cpu.write32(frame, slope);
    let end = (last >> 16) as i16 as i32;
    while line < end {
        let x = (cpu.call(SL_MUL_FX, &[y.wrapping_sub(line << 16) as u32, slope]) >> 16) as i16 as i32;
        let entry = if a >= 0 {
            let v = if turned { CENTRE - x } else { x + CENTRE };
            (v.min(RIGHT) * 2) as u32
        } else {
            let v = (CENTRE - x).max(0);
            (v as u32) << 17 | WHOLE_LINE
        };
        cpu.write32(table.wrapping_add(4 * line as u32), entry);
        line += 1;
    }
    if line <= LAST_ENTRY {
        for l in line..=LAST_ENTRY {
            cpu.write32(table.wrapping_add(4 * l as u32), after);
        }
    }
}
