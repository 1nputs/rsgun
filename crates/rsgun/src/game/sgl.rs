//! SGL (Sega's graphics library, linked into the game): its work area is addressed from GBR. The
//! matrix stack: 20 matrices deep (48 bytes each, 3x4 16.16), its depth and the current one.
//!
//! SGL's code is assembly: what it leaves in r1-r7, MACH/MACL and T its callers' code carries on,
//! and some of its other functions read before they set (a polygon put reads r3 as slPushMatrix
//! left it). Each rewrite leaves them as SGL's code does ([`Cpu::leave`]); its pushes of its
//! arguments are made as any rewrite's prologue is (cpu.stack() is below them).

use super::Cpu;

/// The work area's matrix depth (a byte) and current matrix.
const MATRIX_DEPTH: u32 = 0x20;
const MATRIX: u32 = 0x1c;
const MATRIX_SIZE: u32 = 48;
const MATRIX_MOST: i8 = 20;

/// slPushMatrix: the current matrix copied one up, which becomes current. Whether there was room.
pub fn sl_push_matrix(cpu: &mut Cpu) -> u32 {
    let w = cpu.gbr();
    let depth = cpu.read8(w + MATRIX_DEPTH) as i8;
    if depth >= MATRIX_MOST {
        cpu.leave(&[(2, MATRIX_MOST as u32)]);
        cpu.leave_t(true);
        return 0;
    }
    cpu.write8(w + MATRIX_DEPTH, depth.wrapping_add(1) as u8);
    let m = cpu.read32(w + MATRIX);
    let mut last = [0; 4];
    for k in (0..MATRIX_SIZE).step_by(16) {
        for (i, l) in last.iter_mut().enumerate() {
            *l = cpu.read32(m + k + 4 * i as u32);
        }
        for (i, &v) in last.iter().enumerate() {
            cpu.write32(m + MATRIX_SIZE + k + 4 * i as u32, v);
        }
    }
    cpu.write32(w + MATRIX, m + MATRIX_SIZE);
    let [r1, r2, r3, r4] = last;
    cpu.leave(&[(1, r1), (2, r2), (3, r3), (4, r4), (5, m + 2 * MATRIX_SIZE), (7, 0)]);
    cpu.leave_t(true);
    1
}

/// slPopMatrix: back to the matrix below. Whether there was one.
pub fn sl_pop_matrix(cpu: &mut Cpu) -> u32 {
    let w = cpu.gbr();
    let depth = cpu.read8(w + MATRIX_DEPTH) as i8;
    cpu.leave_t(depth > 0);
    if depth <= 0 {
        return 0;
    }
    cpu.write8(w + MATRIX_DEPTH, (depth - 1) as u8);
    let m = cpu.read32(w + MATRIX);
    cpu.write32(w + MATRIX, m.wrapping_sub(MATRIX_SIZE));
    1
}

/// 16.16 products as SGL takes them: the 64-bit product's middle (MAC's, xtrct).
fn product(a: u32, b: u32) -> i64 {
    a as i32 as i64 * b as i32 as i64
}

fn middle(sum: i64) -> u32 {
    (sum >> 16) as u32
}

/// The current matrix (its address).
fn matrix(cpu: &mut Cpu) -> u32 {
    let w = cpu.gbr();
    cpu.read32(w + MATRIX)
}

/// MAC.L's step: the product added to the 64-bit sum, held to 48 bits when SR's S is set.
const SR_S: u32 = 2;

fn mac(cpu: &Cpu, sum: i64, a: u32, b: u32) -> i64 {
    let sum = sum.wrapping_add(product(a, b));
    if cpu.sh2.sr & SR_S != 0 {
        sum.clamp(-0x8000_0000_0000, 0x7fff_ffff_ffff)
    } else {
        sum
    }
}

/// slTranslate: the current matrix moved by (x, y, z) as it turns them (each row's place gains
/// the row's dot with them: x's product, then y's and z's added as MAC.L adds).
pub fn sl_translate(cpu: &mut Cpu, x: u32, y: u32, z: u32) {
    // the prologue pushed z and y (cpu.stack() below them)
    let sp = cpu.stack() + 8;
    let m = matrix(cpu);
    let (mut sum, mut old) = (0, [0; 3]);
    for r in 0..3 {
        let row = m + 16 * r;
        sum = product(x, cpu.read32(row));
        for (k, v) in [(1, y), (2, z)] {
            let e = cpu.read32(row + 4 * k);
            sum = mac(cpu, sum, v, e);
        }
        old[r as usize] = cpu.read32(row + 12);
        cpu.write32(row + 12, old[r as usize].wrapping_add(middle(sum)));
    }
    let (m20, m23) = (cpu.read32(m + 32), cpu.read32(m + 44));
    cpu.leave(&[(0, m + 44), (1, old[2]), (2, (sum >> 32) as u32), (3, m23), (5, sp), (6, m20), (7, old[1])]);
    cpu.leave_mac(sum);
}

/// slScale: the current matrix's columns scaled by (x, y, z).
pub fn sl_scale(cpu: &mut Cpu, x: u32, y: u32, z: u32) {
    let m = matrix(cpu);
    let (mut p, mut old) = (0, 0);
    for r in 0..3 {
        for (k, s) in [x, y, z].into_iter().enumerate() {
            let at = m + 16 * r + 4 * k as u32;
            old = cpu.read32(at);
            p = product(s, old);
            cpu.write32(at, middle(p));
        }
    }
    cpu.leave(&[(0, m), (2, (p >> 32) as u32), (3, middle(p)), (7, old)]);
    cpu.leave_mac(p);
}

/// One up the stack, the matrix there left as it was (its depth tested unsigned). Whether there
/// was room.
pub fn sl_push_no_copy(cpu: &mut Cpu) -> u32 {
    let w = cpu.gbr();
    let depth = cpu.read8(w + MATRIX_DEPTH) as i8;
    cpu.leave(&[(2, MATRIX_MOST as u32)]);
    if depth as i32 as u32 >= MATRIX_MOST as u32 {
        cpu.leave_t(true);
        return 0;
    }
    cpu.leave_t(false);
    cpu.write8(w + MATRIX_DEPTH, depth.wrapping_add(1) as u8);
    let m = cpu.read32(w + MATRIX);
    cpu.write32(w + MATRIX, m + MATRIX_SIZE);
    1
}

/// slPushUnitMatrix: one up the stack, the matrix there the unit one. Whether there was room.
pub fn sl_push_unit_matrix(cpu: &mut Cpu) -> u32 {
    let w = cpu.gbr();
    let depth = cpu.read8(w + MATRIX_DEPTH) as i8;
    if depth >= MATRIX_MOST {
        cpu.leave(&[(2, MATRIX_MOST as u32)]);
        cpu.leave_t(true);
        return 0;
    }
    cpu.write8(w + MATRIX_DEPTH, depth.wrapping_add(1) as u8);
    let m = cpu.read32(w + MATRIX) + MATRIX_SIZE;
    cpu.write32(w + MATRIX, m);
    for r in 0..3 {
        for k in 0..4 {
            cpu.write32(m + 16 * r + 4 * k, if r == k { 0x1_0000 } else { 0 });
        }
    }
    cpu.leave(&[(1, 0x1_0000), (2, 0), (4, 0)]);
    cpu.leave_t(true);
    1
}

/// slLoadMatrix: the current matrix from SGL's MATRIX at `src` (four columns of three).
pub fn sl_load_matrix(cpu: &mut Cpu, src: u32) {
    let m = matrix(cpu);
    let mut last = [0; 3];
    for k in 0..4 {
        for (r, l) in last.iter_mut().enumerate() {
            *l = cpu.read32(src + 12 * k + 4 * r as u32);
        }
        for (r, &v) in last.iter().enumerate() {
            cpu.write32(m + 16 * r as u32 + 4 * k, v);
        }
    }
    let [r1, r2, r3] = last;
    cpu.leave(&[(0, m + 16), (1, r1), (2, r2), (3, r3), (4, src + 48), (7, 0)]);
    cpu.leave_t(true);
}

/// slGetMatrix: the current matrix to SGL's MATRIX at `dst`.
pub fn sl_get_matrix(cpu: &mut Cpu, dst: u32) {
    let m = matrix(cpu);
    let mut last = [0; 4];
    for r in 0..3 {
        for (k, l) in last.iter_mut().enumerate() {
            *l = cpu.read32(m + 16 * r + 4 * k as u32);
        }
        for (k, &v) in last.iter().enumerate() {
            cpu.write32(dst + 12 * k as u32 + 4 * r, v);
        }
    }
    let [r1, r2, r3, r5] = last;
    cpu.leave(&[(0, m + MATRIX_SIZE), (1, r1), (2, r2), (3, r3), (4, dst + 12), (5, r5), (7, 0)]);
    cpu.leave_t(true);
}

/// The slave's command buffer (where the next command goes; each command ends with the point the
/// slave is told of, by a write to the slave's input capture), the frame's counts of points and
/// polygons put (low and high halves) and their limits, the transformed points' room, the
/// polygons put this frame, and a flag the put sets.
const COMMANDS: u32 = 0x48;
const COUNTS: u32 = 0x74;
const LIMITS: u32 = 0x88;
const POINTS_ROOM: u32 = 0x24;
const POLYGONS_PUT: u32 = 0xaa;
const PUT_FLAG: u32 = 0x73;
const SLAVE_SIGNAL: u32 = 0x2100_0000;
/// The polygon command: its code, the counts and the polygon data's pointers, then the matrix.
const POLYGON_COMMAND: u32 = 20;
const POLYGON_COMMAND_SIZE: u32 = 24 + MATRIX_SIZE;
const PD_POINTS: u32 = 0;
const PD_POINT_COUNT: u32 = 4;
const PD_POLYGONS: u32 = 8;
const PD_POLYGON_COUNT: u32 = 0xc;
const PD_ATTRS: u32 = 0x10;

/// slPutPolygon: the polygon data at `pd` sent to the slave to draw under the current matrix,
/// unless the frame's points or polygons would pass their limits. Whether it was.
pub fn sl_put_polygon(cpu: &mut Cpu, pd: u32) -> u32 {
    let w = cpu.gbr();
    cpu.write8(w + PUT_FLAG, 1);
    let n = cpu.read16(w + POLYGONS_PUT).wrapping_add(1);
    cpu.write16(w + POLYGONS_PUT, n);
    let limits = cpu.read32(w + LIMITS);
    let points = cpu.read32(pd + PD_POINT_COUNT);
    let counts = cpu.read32(w + COUNTS);
    let points_after = (counts & 0xffff).wrapping_add(points);
    if points_after > limits & 0xffff {
        cpu.leave(&[(1, points), (2, limits & 0xffff), (3, limits), (5, points_after << 16)]);
        cpu.leave_t(true);
        return 0;
    }
    let polygons = cpu.read32(pd + PD_POLYGON_COUNT);
    let polygons_after = (counts >> 16).wrapping_add(polygons);
    let new_counts = (polygons_after << 16) | (points_after & 0xffff);
    if polygons_after > limits >> 16 {
        cpu.leave(&[(1, points), (2, polygons), (3, limits >> 16), (5, new_counts)]);
        cpu.leave_t(true);
        return 0;
    }
    let c = cpu.read32(w + COMMANDS);
    let m = cpu.read32(w + MATRIX);
    cpu.write32(c + 4, points);
    cpu.write32(c + 12, polygons);
    let room = cpu.read32(w + POINTS_ROOM).wrapping_add(points << 4);
    cpu.write32(w + POINTS_ROOM, room);
    let pd = pd & !3;
    let at = cpu.read32(pd + PD_POINTS) & !3;
    cpu.write32(c + 8, at);
    let at = cpu.read32(pd + PD_POLYGONS) & !3;
    cpu.write32(c + 16, at);
    let at = cpu.read32(pd + PD_ATTRS) & !1;
    cpu.write32(c + 20, at);
    cpu.write32(c, POLYGON_COMMAND);
    cpu.write32(w + COUNTS, new_counts);
    let mut last = [0; 4];
    for k in (0..MATRIX_SIZE).step_by(16) {
        for (i, l) in last.iter_mut().enumerate() {
            *l = cpu.read32(m + k + 4 * i as u32);
        }
        for (i, &v) in last.iter().enumerate() {
            cpu.write32(c + 24 + k + 4 * i as u32, v);
        }
    }
    let next = c + POLYGON_COMMAND_SIZE;
    cpu.write32(w + COMMANDS, next);
    cpu.write16(SLAVE_SIGNAL, next as u16);
    cpu.leave(&[(1, SLAVE_SIGNAL), (2, last[2]), (3, last[3]), (4, pd), (5, new_counts), (6, m), (7, c + 24)]);
    cpu.leave_t(false);
    1
}

/// slAtan: the angle of (x, y): the smaller part over the larger (on the divide unit, 64 by 32,
/// as SGL does it) looked up in its table of eighth turns, then put in its octant.
const DVSR: u32 = 0xffff_ff00;
const DVDNTH: u32 = 0xffff_ff10;
const DVDNTL: u32 = 0xffff_ff14;
const DVDNTL_AGAIN: u32 = 0xffff_ff1c;
const ATAN_TABLE: u32 = 0x0606_78c0;

pub fn sl_atan(cpu: &mut Cpu, x: u32, y: u32) -> u32 {
    let (mut x, mut y) = (x as i32, y as i32);
    let (mut x_negative, mut y_negative, mut steep) = (false, false, false);
    if x < 0 {
        x = x.wrapping_neg();
        x_negative = true;
    }
    if y < 0 {
        y = y.wrapping_neg();
        y_negative = true;
    }
    let flat = x >= y;
    if !flat {
        std::mem::swap(&mut x, &mut y);
        steep = true;
    }
    let small = y as u32 >> 3;
    cpu.write32(DVSR, x as u32);
    cpu.write32(DVDNTH, (small >> 16) as u16 as i16 as i32 as u32);
    cpu.write32(DVDNTL, small << 16);
    let q = cpu.read32(DVDNTL_AGAIN) & !1;
    let t = cpu.read16(ATAN_TABLE.wrapping_add(q)) as i16 as i32 as u32;
    let (mut r4, mut r5) = (0x4000u32, 0x8000u32);
    let a = match (steep, y_negative, x_negative) {
        (false, false, false) => t,
        (false, false, true) => {
            r5 = 0x8000u32.wrapping_sub(t);
            r5
        }
        (false, true, false) => t.wrapping_neg() & 0xffff,
        (false, true, true) => t.wrapping_add(0x8000),
        (true, false, false) => {
            r4 = 0x4000u32.wrapping_sub(t);
            r4
        }
        (true, false, true) => t.wrapping_add(0x4000),
        (true, true, false) => {
            r5 = 0xc000;
            t.wrapping_add(0xc000)
        }
        (true, true, true) => {
            r5 = 0xc000u32.wrapping_sub(t);
            r5
        }
    };
    cpu.leave(&[(1, 0xffff_fffe), (2, ATAN_TABLE), (3, DVSR), (4, r4), (5, r5)]);
    cpu.leave_t(flat);
    a
}

/// slCurWindow: window `win` (0 or 1) made current unless it is: the slave told (command 56),
/// and the work area's clipping set from the window's settings (its limits, centre, depth: the
/// fields' names as far as can be told). The last value set (or 1 when it was current).
const CUR_WINDOW: u32 = 0x72;
const WINDOW_COMMAND: u32 = 56;
const WINDOWS: u32 = 0x260f_fefc;
const W_LEFT: u32 = 0xc;
const W_TOP: u32 = 0x14;
const W_CENTRE: u32 = 0x1c;
const W_FLAG: u32 = 0x1e;
const W_DEPTH: u32 = 0x24;
const W_RIGHT: u32 = 0x38;
const WIN_FLAG: u32 = 0xac;
const WIN_DEPTH: u32 = 0x68;
const WIN_CENTRE: u32 = 0xa0;
const WIN_SIZE: u32 = 0x84;
const WIN_WIDTH: u32 = 0x90;
const WIN_SCREEN: u32 = 0x304;
const WIN_SCREEN_X: u32 = 0x98;
const WIN_SCREEN_Y: u32 = 0x9a;
const WIN_LEFT_TOP: u32 = 0x78;
const WIN_RIGHT_TOP: u32 = 0x7c;

pub fn sl_cur_window(cpu: &mut Cpu, win: u32) -> u32 {
    let w = cpu.gbr();
    let current = cpu.read8(w + CUR_WINDOW) as i8 as i32 as u32 == win;
    cpu.leave_t(current);
    if current {
        return 1;
    }
    cpu.write8(w + CUR_WINDOW, (win & 1) as u8);
    cpu.write8(w + PUT_FLAG, 0);
    let c = cpu.read32(w + COMMANDS);
    cpu.write32(c, WINDOW_COMMAND);
    cpu.write32(c + 4, win);
    cpu.write32(w + COMMANDS, c + 8);
    cpu.write16(SLAVE_SIGNAL, (c + 8) as u16);
    let s = WINDOWS.wrapping_add(win << 2);
    let flag = cpu.read16(s + W_FLAG) as u8;
    cpu.write8(w + WIN_FLAG, flag);
    let left = cpu.read32(s + W_LEFT);
    let depth = cpu.read32(s + W_DEPTH);
    cpu.write32(w + WIN_DEPTH, depth);
    let top = cpu.read32(s + W_TOP);
    let centre = cpu.read16(s + W_CENTRE);
    cpu.write16(w + WIN_CENTRE, centre);
    let right = cpu.read32(s + W_RIGHT);
    cpu.write32(w + WIN_SIZE, 0x1_0001u32.wrapping_add(top).wrapping_sub(left));
    cpu.write32(w + WIN_WIDTH, right.wrapping_sub(left));
    let swap = |v: u32| v.rotate_left(16);
    let screen = cpu.read32(w + WIN_SCREEN);
    cpu.write16(w + WIN_SCREEN_X, screen.wrapping_add(swap(right)) as u16);
    let a = swap(left).wrapping_sub(swap(right));
    let b = swap(top).wrapping_sub(swap(right));
    let screen = cpu.read32(w + WIN_SCREEN);
    cpu.write16(w + WIN_SCREEN_Y, (screen >> 16).wrapping_add(right) as u16);
    let lo = right << 16;
    let l = (left << 16).wrapping_sub(lo);
    let t = (top << 16).wrapping_sub(lo);
    let lt = (a << 16) | (l >> 16);
    cpu.write32(w + WIN_LEFT_TOP, lt);
    let rt = (b << 16) | (t >> 16);
    cpu.write32(w + WIN_RIGHT_TOP, rt);
    cpu.leave(&[(1, a), (2, swap(right)), (3, b), (4, lt), (5, rt), (6, right)]);
    rt
}

/// slCalcPoint: the point (x, y, z) through the current matrix to `out` (x, y, z).
pub fn sl_calc_point(cpu: &mut Cpu, x: u32, y: u32, z: u32, out: u32) {
    // the prologue pushed z, y and x (cpu.stack() at x)
    let sp = cpu.stack();
    let m = matrix(cpu);
    let (mut sum, mut v) = (0, 0);
    for r in 0..3 {
        let row = m + 16 * r;
        sum = 0;
        for (k, a) in [x, y, z].into_iter().enumerate() {
            let e = cpu.read32(row + 4 * k as u32);
            sum = mac(cpu, sum, a, e);
        }
        v = middle(sum).wrapping_add(cpu.read32(row + 12));
        cpu.write32(out + 4 * r, v);
    }
    let m23 = cpu.read32(m + 44);
    cpu.leave(&[(0, m + MATRIX_SIZE), (1, 0), (2, (sum >> 32) as u32), (3, v), (4, sp), (5, m23), (7, out + 12)]);
    cpu.leave_mac(sum);
    cpu.leave_t(true);
}

/// VDP2's background display bits (its BGON as SGL keeps it, a word): `bits`'s low half set, its
/// high half cleared.
const BG_ON: u32 = 0xe0;

pub fn sl_bg_on(cpu: &mut Cpu, bits: u32) {
    let w = cpu.gbr();
    let not_off = !(bits >> 16);
    let v = (cpu.read16(w + BG_ON) as i16 as i32 as u32 | bits) & not_off;
    cpu.write16(w + BG_ON, v as u16);
    cpu.leave(&[(0, v), (4, not_off)]);
}
