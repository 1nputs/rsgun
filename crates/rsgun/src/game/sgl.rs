//! SGL (Sega's graphics library, linked into the game): its work area is addressed from GBR. The
//! matrix stack: 20 matrices deep (48 bytes each, 3x4 16.16), its depth and the current one.
//!
//! SGL's code is assembly: what it leaves in r1-r7, MACH/MACL and T its callers' code carries on,
//! and some of its other functions read before they set (a polygon put reads r3 as slPushMatrix
//! left it). Each rewrite leaves them as SGL's code does ([`Cpu::leave`]); its pushes of its
//! arguments are made as any rewrite's prologue is (cpu.stack() is below them).

use super::{nested, Cpu};

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

/// SGL's sine and cosine of `a` together (16 bits of turn, 16.16), as its turns take them: the
/// octant's entry (8 bytes: index masks, index offsets, value masks, value offsets, a byte each
/// for the sine then the cosine) picks and corrects two of a quarter turn's steps. Left in r4 and
/// r5 (and r0-r2 as it leaves them).
const SIN_COS_ENTRIES: u32 = 0x0606_9ae8;
const SIN_COS_TABLE: u32 = 0x0606_9b28;
const SIN_COS_STEPS: u32 = 0x3ffe;
const SL_SIN_COS: u32 = 0x0606_9a54;

fn sin_cos(cpu: &mut Cpu, a: u32) -> (u32, u32) {
    let e = SIN_COS_ENTRIES + ((a >> 13) & 7) * 8;
    let byte = |cpu: &mut Cpu, k: u32| cpu.read8(e + k) as i8 as i32 as u32;
    let mut v = [0; 2];
    let mut last = [0; 2];
    for (k, v) in v.iter_mut().enumerate() {
        let k = k as u32;
        let i = ((a ^ byte(cpu, k)) & SIN_COS_STEPS).wrapping_add(byte(cpu, 2 + k));
        let step = cpu.read16(SIN_COS_TABLE.wrapping_add(i)) as u32;
        last[k as usize] = byte(cpu, 6 + k);
        *v = (step ^ byte(cpu, 4 + k)).wrapping_add(last[k as usize]);
    }
    cpu.leave(&[(0, last[0]), (1, last[1]), (2, e + 8), (4, v[0]), (5, v[1])]);
    (v[0], v[1])
}

pub fn sl_sin_cos(cpu: &mut Cpu, a: u32) {
    sin_cos(cpu, a);
}

/// slRotX, slRotY, slRotZ: the current matrix turned by `a` about x, y or z. In each row columns
/// i and j become c·i + s·j and c·j - s·i (each a product then MAC.L's add of the other); the
/// sine and cosine are left below the stack (where the SH-2 code pushes them for MAC.L).
fn rot(cpu: &mut Cpu, a: u32, i: u32, j: u32) {
    // the prologue pushed PR
    let sp = cpu.stack() + 4;
    let (s, c) = nested!(cpu, SL_SIN_COS, sin_cos(cpu, a));
    cpu.write32(sp - 4, c);
    cpu.write32(sp - 8, s);
    turn(cpu, sp, s, c, i, j);
}

/// The turn by sine `s` and cosine `c` (pushed below `sp`).
fn turn(cpu: &mut Cpu, sp: u32, s: u32, c: u32, i: u32, j: u32) {
    let m = matrix(cpu);
    let minus_s = s.wrapping_neg();
    let (mut sum, mut old_i, mut new_j) = (0, 0, 0);
    for r in 0..3 {
        let row = m + 16 * r;
        old_i = cpu.read32(row + 4 * i);
        let e = cpu.read32(row + 4 * j);
        sum = mac(cpu, product(c, old_i), s, e);
        cpu.write32(row + 4 * i, middle(sum));
        sum = mac(cpu, product(minus_s, old_i), c, e);
        new_j = middle(sum);
        cpu.write32(row + 4 * j, new_j);
    }
    cpu.leave(&[(0, m + 4 * (9 + j)), (1, sp), (2, (sum >> 32) as u32), (3, new_j), (4, minus_s), (5, c), (6, m), (7, old_i)]);
    cpu.leave_mac(sum);
}

pub fn sl_rot_x(cpu: &mut Cpu, a: u32) {
    rot(cpu, a, 1, 2);
}

pub fn sl_rot_y(cpu: &mut Cpu, a: u32) {
    rot(cpu, a, 2, 0);
}

pub fn sl_rot_z(cpu: &mut Cpu, a: u32) {
    rot(cpu, a, 0, 1);
}

/// slRotXSC, slRotYSC, slRotZSC: the turns from a sine and cosine given (the code after the
/// turns' call for them; its prologue pushes them).
pub fn sl_rot_x_sc(cpu: &mut Cpu, s: u32, c: u32) {
    let sp = cpu.stack() + 8;
    turn(cpu, sp, s, c, 1, 2);
}

pub fn sl_rot_y_sc(cpu: &mut Cpu, s: u32, c: u32) {
    let sp = cpu.stack() + 8;
    turn(cpu, sp, s, c, 2, 0);
}

pub fn sl_rot_z_sc(cpu: &mut Cpu, s: u32, c: u32) {
    let sp = cpu.stack() + 8;
    turn(cpu, sp, s, c, 0, 1);
}

/// slPriority: scroll screen `screen`'s priority set to `pri` in SGL's image of VDP2's registers
/// (sent at the VBlank): the screen as SGL numbers it, the byte's offset from PRINA (NBG1 0, NBG0
/// 1, NBG3 2, NBG2 3; the sprite registers' bytes below, from -8).
const PRINA_IMAGE: u32 = 0x060f_fdb8;

pub fn sl_priority(cpu: &mut Cpu, screen: u32, pri: u32) {
    cpu.write8(PRINA_IMAGE.wrapping_add(screen), pri as u8);
    cpu.leave(&[(0, PRINA_IMAGE)]);
}

/// SGL's other per-screen bytes in its VDP2 image, the screen numbered as slPriority's (the
/// byte's offset in the register): the windows' control (WCTLA-WCTLD), the line and cell scroll
/// control (SCRCTL: NBG1 0, NBG0 1), the colour calculation ratios (slColRate: CCRNA on).
const WCTLA_IMAGE: u32 = 0x060f_fd90;
const SCRCTL_IMAGE: u32 = 0x060f_fd5a;
const CCRNA_IMAGE: u32 = 0x060f_fdc8;

fn screen_byte(cpu: &mut Cpu, base: u32, screen: u32, v: u32) {
    cpu.write8(base.wrapping_add(screen), v as u8);
    cpu.leave(&[(0, base)]);
}

pub fn sl_window_control(cpu: &mut Cpu, screen: u32, v: u32) {
    screen_byte(cpu, WCTLA_IMAGE, screen, v);
}

pub fn sl_line_scroll_control(cpu: &mut Cpu, screen: u32, v: u32) {
    screen_byte(cpu, SCRCTL_IMAGE, screen, v);
}

pub fn sl_col_rate(cpu: &mut Cpu, screen: u32, v: u32) {
    screen_byte(cpu, CCRNA_IMAGE, screen, v);
}

/// slScrPosNbg0/1, slZoomNbg0/1: the scroll screen's place and zoom (x, y: 16.16) in the VDP2
/// image (SCXIN0-SCYDN0, ZMXIN0-ZMYDN0, and NBG1's).
const SCROLL_NBG0: u32 = 0x130;
const ZOOM_NBG0: u32 = 0x138;
const SCROLL_NBG1: u32 = 0x140;
const ZOOM_NBG1: u32 = 0x148;

fn screen_pair(cpu: &mut Cpu, at: u32, x: u32, y: u32) {
    let w = cpu.gbr();
    cpu.write32(w + at, x);
    cpu.write32(w + at + 4, y);
    cpu.leave(&[(0, y)]);
}

pub fn sl_scr_pos_nbg0(cpu: &mut Cpu, x: u32, y: u32) {
    screen_pair(cpu, SCROLL_NBG0, x, y);
}

pub fn sl_zoom_nbg0(cpu: &mut Cpu, x: u32, y: u32) {
    screen_pair(cpu, ZOOM_NBG0, x, y);
}

pub fn sl_scr_pos_nbg1(cpu: &mut Cpu, x: u32, y: u32) {
    screen_pair(cpu, SCROLL_NBG1, x, y);
}

pub fn sl_zoom_nbg1(cpu: &mut Cpu, x: u32, y: u32) {
    screen_pair(cpu, ZOOM_NBG1, x, y);
}

/// The colour offsets used on the screens of `on` and `b` (CLOFEN), offset B on `b`'s (CLOFSL).
const CLOFEN: u32 = 0x1d0;
const CLOFSL: u32 = 0x1d2;

pub fn sl_col_offset_on(cpu: &mut Cpu, on: u32, b: u32) {
    let w = cpu.gbr();
    cpu.write16(w + CLOFSL, b as u16);
    cpu.write16(w + CLOFEN, (b | on) as u16);
    cpu.leave(&[(0, b | on)]);
}

/// The colour calculation's control (CCCTL) but its top four bits set to `v`.
const CCCTL: u32 = 0x1ac;
const CCCTL_KEPT: u32 = 0xffff_f000;

pub fn sl_color_calc(cpu: &mut Cpu, v: u32) {
    let w = cpu.gbr();
    let c = (cpu.read16(w + CCCTL) as i16 as i32 as u32 & CCCTL_KEPT) | v;
    cpu.write16(w + CCCTL, c as u16);
    cpu.leave(&[(0, c), (1, CCCTL_KEPT)]);
}

/// The special colour calculation mode (SFCCMD): the field of two bits at `shift` (NBG0 0, NBG1
/// 2, NBG2 4, NBG3 6; the whole high byte for 8) set to `v`; the entries share their tail.
const SFCCMD: u32 = 0x1ae;

fn special_mode(cpu: &mut Cpu, keep: u32, v: u32) {
    let w = cpu.gbr();
    let c = (cpu.read16(w + SFCCMD) as i16 as i32 as u32 & keep) | v;
    cpu.write16(w + SFCCMD, c as u16);
    cpu.leave(&[(0, c), (1, keep), (4, v)]);
}

pub fn sl_special_mode_nbg0(cpu: &mut Cpu, v: u32) {
    special_mode(cpu, !0x3, v);
}

pub fn sl_special_mode_nbg1(cpu: &mut Cpu, v: u32) {
    special_mode(cpu, !0xc, v << 2);
}

pub fn sl_special_mode_nbg2(cpu: &mut Cpu, v: u32) {
    special_mode(cpu, !0x30, v << 4);
}

pub fn sl_special_mode_nbg3(cpu: &mut Cpu, v: u32) {
    // its mask made by a rotate, which leaves T set
    special_mode(cpu, !0xc0, (v << 8) >> 2);
    cpu.leave_t(true);
}

pub fn sl_special_mode_high(cpu: &mut Cpu, v: u32) {
    special_mode(cpu, 0xff, v << 8);
}

/// The 32-bit integer square root (two bits a step, 16 steps; halved at the end, T the bit
/// dropped), and SGL's square root of a 16.16 value from it.
pub fn sl_isqrt(cpu: &mut Cpu, v: u32) -> u32 {
    let (mut root, mut rest, mut bits) = (0u32, 0u32, v);
    for _ in 0..16 {
        for _ in 0..2 {
            rest = (rest << 1) | (bits >> 31);
            bits <<= 1;
        }
        root = root.wrapping_add(root).wrapping_add(1);
        if root > rest {
            root = root.wrapping_sub(1);
        } else {
            rest = rest.wrapping_sub(root);
            root = root.wrapping_add(1);
        }
    }
    cpu.leave(&[(1, 0), (2, rest), (4, 0)]);
    cpu.leave_t(root & 1 != 0);
    root >> 1
}

pub fn sl_sqrt(cpu: &mut Cpu, v: u32) -> u32 {
    nested!(cpu, 0x0607_147c, sl_isqrt(cpu, v)) << 8
}

/// slDivFX: `b` over `a` (16.16), on the divide unit.
pub fn sl_div_fx(cpu: &mut Cpu, a: u32, b: u32) -> u32 {
    cpu.write32(DVSR, a);
    let high = (b as i32 >> 16) as u32;
    cpu.write32(DVDNTH, high);
    cpu.write32(DVDNTL, b << 16);
    cpu.leave(&[(2, high), (3, DVSR), (5, b << 16)]);
    cpu.read32(DVDNTL_AGAIN)
}

/// slColOffsetA, slColOffsetB: colour offset A's (or B's) red, green and blue in the image of
/// VDP2's registers (COAR-COAB, COBR-COBB).
const COLOUR_OFFSET_A: u32 = 0x1d4;
const COLOUR_OFFSET_B: u32 = 0x1da;

fn colour_offset(cpu: &mut Cpu, at: u32, r: u32, g: u32, b: u32) {
    let w = cpu.gbr();
    for (k, v) in [r, g, b].into_iter().enumerate() {
        cpu.write16(w + at + 2 * k as u32, v as u16);
    }
    cpu.leave(&[(0, b)]);
}

pub fn sl_col_offset_a(cpu: &mut Cpu, r: u32, g: u32, b: u32) {
    colour_offset(cpu, COLOUR_OFFSET_A, r, g, b);
}

pub fn sl_col_offset_b(cpu: &mut Cpu, r: u32, g: u32, b: u32) {
    colour_offset(cpu, COLOUR_OFFSET_B, r, g, b);
}

/// SGL's square root of the 64-bit `hi:lo`, two bits a step: the high long's 16 steps, then 15
/// of the low long's (its top 30 bits). The root (its registers as it leaves them).
pub fn sl_sqrt64(cpu: &mut Cpu, hi: u32, lo: u32) -> u32 {
    let (mut root, mut rest, mut bits) = (0u32, 0u32, hi);
    for (pass, steps) in [16, 15].into_iter().enumerate() {
        if pass == 1 {
            bits = lo;
        }
        for _ in 0..steps {
            for _ in 0..2 {
                rest = (rest << 1) | (bits >> 31);
                bits <<= 1;
            }
            root = root.wrapping_add(root).wrapping_add(1);
            if root > rest {
                root = root.wrapping_sub(1);
            } else {
                rest = rest.wrapping_sub(root);
                root = root.wrapping_add(1);
            }
        }
    }
    cpu.leave(&[(1, 15), (2, rest), (3, 0), (4, lo)]);
    cpu.leave_t(true);
    root
}

/// slLookAt: the current matrix turned to look from `cam` at `target` (x, y, z each) and moved to
/// the camera: turned by `twist` about z, about x by the climb (the height over the distance and
/// the level distance over the distance) and about y by the heading (the x and z offsets over the
/// level distance), the quotients on the divide unit (each started before the next root or turn,
/// as SGL overlaps them); then moved by minus the camera. Its calls are SGL's own (cpu.call: they
/// take their registers as SGL hands them on).
const SL_ROT_Z: u32 = 0x0606_726c;
const SL_ROT_X_SC: u32 = 0x0606_72f6;
const SL_ROT_Y_SC: u32 = 0x0606_7376;
const SL_TRANSLATE: u32 = 0x0606_721c;
const SL_SQRT64: u32 = 0x0607_1c0c;

fn divide(cpu: &mut Cpu, by: u32, v: u32) {
    cpu.write32(DVSR, by);
    cpu.write32(DVDNTH, (v as i32 >> 16) as u32);
    cpu.write32(DVDNTL, v << 16);
}

pub fn sl_look_at(cpu: &mut Cpu, cam: u32, target: u32, twist: u32) {
    cpu.call(SL_ROT_Z, &[twist]);
    let c = [0, 1, 2].map(|k| cpu.read32(cam + 4 * k));
    let t = [0, 1, 2].map(|k| cpu.read32(target + 4 * k));
    let dx = c[0].wrapping_sub(t[0]);
    let dz = t[2].wrapping_sub(c[2]);
    let dy = t[1].wrapping_sub(c[1]);
    let square = |v: u32| (v as i32 as i64 * v as i32 as i64) as u64;
    let level = square(dx).wrapping_add(square(dz));
    let all = level.wrapping_add(square(dy));
    let length = cpu.call(SL_SQRT64, &[(all >> 32) as u32, all as u32]);
    divide(cpu, length, dy);
    let level_length = cpu.call(SL_SQRT64, &[(level >> 32) as u32, level as u32]);
    let sin_x = cpu.read32(DVDNTL_AGAIN);
    divide(cpu, length, level_length);
    let cos_x = cpu.read32(DVDNTL_AGAIN);
    divide(cpu, level_length, dx);
    cpu.call_with_stack(SL_ROT_X_SC, &[sin_x, cos_x], &[level_length]);
    let sin_y = cpu.read32(DVDNTL_AGAIN);
    divide(cpu, level_length, dz);
    let cos_y = cpu.read32(DVDNTL_AGAIN);
    cpu.call(SL_ROT_Y_SC, &[sin_y, cos_y]);
    cpu.call(SL_TRANSLATE, &c.map(|v| v.wrapping_neg()));
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

/// The polygon (a sprite counts as one) about to be put counted, and its depth `z` checked: the
/// polygons put (the counts' high half) against their limit, the depth past the window's near
/// (its depth over 2 to the window's flag) and short of the far limit (whole units, FAR). The
/// depth past the near, the near and the count after; or none, the registers left as the SH-2 code's check
/// (0x0607170c) leaves them, which then returns 0 from the put (it pops the put's PR).
const FAR: u32 = 0x70;

fn put_check(cpu: &mut Cpu, z: u32) -> Option<(u32, u32, u32)> {
    let w = cpu.gbr();
    cpu.write8(w + PUT_FLAG, 1);
    let n = cpu.read16(w + POLYGONS_PUT).wrapping_add(1);
    cpu.write16(w + POLYGONS_PUT, n);
    let limit = cpu.read16(w + LIMITS) as u32;
    let count = cpu.read16(w + COUNTS) as u32;
    let fail = |cpu: &mut Cpu, regs: &[(usize, u32)]| {
        cpu.leave(regs);
        cpu.leave(&[(0, 0)]);
        cpu.leave_t(true);
        None
    };
    if count >= limit {
        return fail(cpu, &[(2, z), (7, count)]);
    }
    // the SH-2 code shifts the depth right once a count down to 0 (none past 31 bits)
    let shift = cpu.read8(w + WIN_FLAG) as i8 as i32;
    let depth = cpu.read32(w + WIN_DEPTH);
    let near = if (1..32).contains(&shift) { depth >> shift } else { 0 };
    let past = z.wrapping_sub(near);
    if near as i32 >= z as i32 || past as i32 > ((cpu.read16(w + FAR) as u32) << 16) as i32 {
        return fail(cpu, &[(2, past), (3, near), (7, count + 1)]);
    }
    cpu.write16(w + COUNTS, (count + 1) as u16);
    Some((past, near, count + 1))
}

/// slPutSprite: the sprite at `pos` (x, y, z and a scale: one below 0 is the scale at the
/// window's depth, the sprite shrinking with distance; 0 puts nothing) with the attributes at
/// `attr` (SPR_ATTR: texno, atrb, colno, gstb, dir) turned by `angle`, sent to the slave: command
/// 28, or 24 with the turn's sine and cosine (SGL's table, a quadrant's index masks and value
/// masks); counted and checked as a polygon is (put_check). The command's end, or 0.
const SPRITE_COMMAND: u32 = 28;
const TURNED_SPRITE_COMMAND: u32 = 24;
const SPRITE_QUADRANTS: u32 = 0x0606_7694;

pub fn sl_put_sprite(cpu: &mut Cpu, pos: u32, attr: u32, angle: u32) -> u32 {
    let z = cpu.read32(pos + 8);
    let Some((past, near, _)) = put_check(cpu, z) else {
        return 0;
    };
    let w = cpu.gbr();
    let scale = cpu.read32(pos + 12);
    let c = cpu.read32(w + COMMANDS);
    if scale == 0 {
        cpu.leave(&[(1, 0), (2, past), (3, near), (7, c)]);
        cpu.leave_t(true);
        return 0;
    }
    let attr = attr & !1;
    let dir = cpu.read16(attr + 8) as u32;
    cpu.write32(c + 4, (dir << 16) | (past >> 16));
    let size = if scale as i32 > 0 {
        scale
    } else {
        let p = cpu.read32(w + WIN_DEPTH) as u64 * scale.wrapping_neg() as u64;
        cpu.leave_mac(p as i64);
        let mid = (p >> 16) as u32;
        cpu.write32(DVSR, past);
        cpu.write32(DVDNTH, (mid as i32 >> 16) as u32);
        cpu.write32(DVDNTL, mid << 16);
        cpu.read32(DVDNTL_AGAIN)
    };
    let word = |cpu: &mut Cpu, k: u32| cpu.read16(attr + 2 * k) as i16 as i32 as u32;
    let (texno, atrb, colno, gstb) = (word(cpu, 0), word(cpu, 1), word(cpu, 2), word(cpu, 3));
    let atrb_colno = (atrb << 16) | (colno & 0xffff);
    cpu.write32(c + 12, atrb_colno);
    cpu.write32(c + 8, (texno << 16) | (gstb & 0xffff));
    cpu.write32(c + 24, size);
    for k in 0..2 {
        let v = cpu.read32(pos + 4 * k);
        cpu.write32(c + 16 + 4 * k, v);
    }
    let angle = angle & 0xffff;
    if angle == 0 {
        cpu.write32(c, SPRITE_COMMAND);
        let next = c + 28;
        cpu.write32(w + COMMANDS, next);
        cpu.write16(SLAVE_SIGNAL, next as u16);
        cpu.leave(&[(1, SLAVE_SIGNAL), (2, atrb), (3, atrb_colno), (5, attr + 8), (6, 0), (7, c)]);
        cpu.leave_t(true);
        return next;
    }
    let e = SPRITE_QUADRANTS + ((angle >> 14) & 3) * 4;
    let byte = |cpu: &mut Cpu, k: u32| cpu.read8(e + k) as i8 as i32 as u32;
    let mut v = [0; 2];
    for (k, v) in v.iter_mut().enumerate() {
        let k = k as u32;
        let i = (angle ^ byte(cpu, k)) & SIN_COS_STEPS;
        let step = cpu.read16(SIN_COS_TABLE + i) as u32;
        *v = step.wrapping_add(2) ^ byte(cpu, 2 + k);
    }
    let [sin, cos] = v;
    cpu.write32(c + 28, sin);
    cpu.write32(c + 32, cos);
    cpu.write32(c, TURNED_SPRITE_COMMAND);
    let next = c + 36;
    cpu.write32(w + COMMANDS, next);
    cpu.write16(SLAVE_SIGNAL, next as u16);
    cpu.leave(&[(1, SLAVE_SIGNAL), (2, e + 4), (3, atrb_colno), (5, cos), (6, sin), (7, c)]);
    cpu.leave_t(false);
    next
}

/// The sprite put with two scales (slPutSprite's form for a sprite scaled differently across and
/// down): counted and checked as a polygon is (put_check), its size from the character's (the
/// sprite's table at the work area's +0x3c) times its two scales, sent as a polygon (command 40,
/// the corners in the polygon buffer's slot) rather than as a sprite. Unturned, a rectangle about
/// its place (its flips in the corners' signs). Turned, its corners turned by the angle's sine and
/// cosine (SGL's table, by quadrant) and stepped from its place by a handler chosen by its flips
/// (a jump table), each step checked against the window's edges (off them: nothing, 0); then,
/// unless the sprite says not to, the first corner the one furthest up and left. 1, or 0.
/// Written step for step from the SH-2 code (tools/port/asm2rs.py), each step with its address,
/// its registers left as it leaves them.
fn hv_off(cpu: &mut Cpu, regs: [u32; 7], mac: i64, t: bool) -> u32 {
    let regs: Vec<(usize, u32)> = regs.into_iter().enumerate().map(|(k, v)| (k + 1, v)).collect();
    cpu.leave(&regs);
    cpu.leave_mac(mac);
    cpu.leave_t(t);
    0
}

#[allow(unused_assignments, unused_variables, unused_mut, clippy::all)]
pub fn sl_put_sprite_hv(cpu: &mut Cpu, pos: u32, attr: u32, angle: u32) -> u32 {
    let w = cpu.gbr();
    let z = cpu.read32(pos + 8);
    let Some((past, near, n)) = put_check(cpu, z) else {
        return 0;
    };
    let [_, mut r1, _, _, mut r4, mut r5, mut r6, _, mut r8, mut r9, mut r10, mut r11, mut r12, mut r13, _, _] = cpu.sh2.r;
    let mut r14 = cpu.caller_r14;
    let (mut r0, mut r2, mut r3, mut r7): (u32, u32, u32, u32);
    let mut mac = (((cpu.sh2.mach as u64) << 32) | cpu.sh2.macl as u64) as i64;
    let mut t: bool;
    (r4, r5, r6) = (pos, attr, angle);
    // after the check: the count in r0 and r7, the depth past the near in r2, the near in r3
    r0 = n;
    r2 = past;
    r3 = near;
    r7 = n;
    t = false;
    // the slot of the polygon buffer and the projected sizes
    r0 = cpu.read32(w.wrapping_add(48)); // 060676b6 mov.l @(48,gbr),r0
    r1 = cpu.read16(r5) as i16 as i32 as u32; r5 = r5.wrapping_add(2); // 060676b8 mov.w @r5+,r1
    r7 = r0; // 060676ba mov r0,r7
    r0 = cpu.read16(r5.wrapping_add(4)) as i16 as i32 as u32; // 060676bc mov.w @(4,r5),r0
    r1 <<= 2; // 060676be shll2 r1
    r2 = (r0 << 16) | (r2 >> 16); // 060676c0 xtrct r0,r2
    cpu.write32(r7.wrapping_add(28), r2); // 060676c2 mov.l r2,@(28,r7)
    r0 = cpu.read32(w.wrapping_add(60)); // 060676c4 mov.l @(60,gbr),r0
    t = r1 >> 31 != 0; r1 <<= 1; // 060676c6 shll r1
    r1 = r1.wrapping_add(r0); // 060676c8 add r0,r1
    r2 = cpu.read16(r1) as i16 as i32 as u32; r1 = r1.wrapping_add(2); // 060676ca mov.w @r1+,r2
    r0 = cpu.read32(r4.wrapping_add(12)); // 060676cc mov.l @(12,r4),r0
    r3 = cpu.read16(r1) as i16 as i32 as u32; r1 = r1.wrapping_add(2); // 060676ce mov.w @r1+,r3
    mac = (mac & !0xffff_ffff) | (r2.wrapping_mul(r0) as i64 & 0xffff_ffff); // 060676d0 mul.l r0,r2
    r0 = cpu.read32(r4.wrapping_add(16)); // 060676d2 mov.l @(16,r4),r0
    r1 = cpu.read32(r1); // 060676d4 mov.l @r1,r1
    r2 = mac as u32; // 060676d6 sts macl,r2
    mac = (mac & !0xffff_ffff) | (r3.wrapping_mul(r0) as i64 & 0xffff_ffff); // 060676d8 mul.l r0,r3
    r0 = cpu.read16(r5) as i16 as i32 as u32; r5 = r5.wrapping_add(2); // 060676da mov.w @r5+,r0
    cpu.write32(r7.wrapping_add(8), r1); // 060676dc mov.l r1,@(8,r7)
    r3 = mac as u32; // 060676de sts macl,r3
    r1 = cpu.read16(r5) as i16 as i32 as u32; r5 = r5.wrapping_add(2); // 060676e0 mov.w @r5+,r1
    r5 = r5.wrapping_add(2i32 as u32); // 060676e2 add #2,r5
    r1 <<= 16; // 060676e4 shll16 r1
    r1 = (r0 << 16) | (r1 >> 16); // 060676e6 xtrct r0,r1
    r0 = 0x060717d4; // 060676e8 mov.l 0x60676f8,r0
    r5 = cpu.read16(r5) as i16 as i32 as u32; // 060676ea mov.w @r5,r5
    cpu.write32(r7.wrapping_add(4), r1); // 060676ec mov.l r1,@(4,r7)
    r1 >>= 16;
    let r1_pushed = r1;
    // the turn (0x060717d4)
    t = r6 & r6 == 0;
    r0 = (r6 & 0xffff_0000) | ((r6 & 0xff) << 8) | ((r6 >> 8) & 0xff);
    if t {
        // unturned: a rectangle about the place, off the window's edges nothing
        r6 = cpu.read16(r4) as i16 as i32 as u32; // 0607174c mov.w @r4,r6
        r0 = cpu.read16(w.wrapping_add(152)) as i16 as i32 as u32; // 0607174e mov.w @(152,gbr),r0
        r1 = r6; // 06071750 mov r6,r1
        r1 = r1.wrapping_add(r0); // 06071752 add r0,r1
        r0 = cpu.read16(w.wrapping_add(140)) as i16 as i32 as u32; // 06071754 mov.w @(140,gbr),r0
        r4 = cpu.read32(r4.wrapping_add(4)); // 06071756 mov.l @(4,r4),r4
        t = r1 > r0; // 06071758 cmp/hi r0,r1
        r0 = cpu.read16(w.wrapping_add(154)) as i16 as i32 as u32;
        if t {
            return hv_off(cpu, [r1, r2, r3, r4, r5, r6, r7], mac, t);
        }
        r1 = r4; // 0607175e mov r4,r1
        r0 <<= 16; // 06071760 shll16 r0
        r1 = r1.wrapping_add(r0); // 06071762 add r0,r1
        r0 = cpu.read16(w.wrapping_add(142)) as i16 as i32 as u32; // 06071764 mov.w @(142,gbr),r0
        r0 <<= 16; // 06071766 shll16 r0
        t = r1 > r0; // 06071768 cmp/hi r0,r1
        r0 = 3;
        if t {
            return hv_off(cpu, [r1, r2, r3, r4, r5, r6, r7], mac, t);
        }
        r5 ^= r0;
        t = r2 as i32 >= 0;
        r1 = 1;
        if !t {
            r2 = r2.wrapping_neg();
            r0 = 16;
            r5 ^= r0;
        }
        t = r3 as i32 >= 0;
        r1 <<= 16;
        if !t {
            r3 = r3.wrapping_neg();
            r0 = 32;
            r5 ^= r0;
        }
        t = r2 as i32 >= r1 as i32;
        r2 = r2.wrapping_sub(r1);
        if !t {
            r2 = 0;
        }
        t = r3 as i32 >= r1 as i32;
        r3 = r3.wrapping_sub(r1);
        if !t {
            r3 = 0;
        }
        r2 = r2.rotate_left(16); // 06071798 swap.w r2,r2
        r3 = (r2 << 16) | (r3 >> 16); // 0607179a xtrct r2,r3
        cpu.write32(r7.wrapping_add(16), r3); // 0607179c mov.l r3,@(16,r7)
        r0 = (r5 & 0xffff_0000) | ((r5 & 0xff) << 8) | ((r5 >> 8) & 0xff); // 0607179e swap.b r5,r0
        t = r0 & 15 == 0; // 060717a0 tst #15,r0
        if t {
            r0 |= 10;
        }
        r0 = (r0 & 0xffff_0000) | ((r0 & 0xff) << 8) | ((r0 >> 8) & 0xff); // 060717a6 swap.b r0,r0
        cpu.write16(r7.wrapping_add(0), r0 as u16); // 060717a8 mov.w r0,@(0,r7)
        r4 = (r6 << 16) | (r4 >> 16); // 060717aa xtrct r6,r4
        cpu.write32(r7.wrapping_add(12), r4); // 060717ac mov.l r4,@(12,r7)
        r0 = cpu.read32(w.wrapping_add(72)); // 060717ae mov.l @(72,gbr),r0
        r1 = 40i32 as u32; // 060717b0 mov #40,r1
        cpu.write32(r0.wrapping_add(0), r1); // 060717b2 mov.l r1,@(0,r0)
        cpu.write32(r0.wrapping_add(4), r7); // 060717b4 mov.l r7,@(4,r0)
        r1 = 1i32 as u32; // 060717b6 mov #1,r1
        cpu.write32(r0.wrapping_add(8), r1); // 060717b8 mov.l r1,@(8,r0)
        r1 = 0i32 as u32; // 060717ba mov #0,r1
        cpu.write32(r0.wrapping_add(12), r1); // 060717bc mov.l r1,@(12,r0)
        r0 = r0.wrapping_add(16i32 as u32); // 060717be add #16,r0
        r1 = 0x21000000; // 060717c0 mov.l 0x60719bc,r1
        cpu.write32(w.wrapping_add(72), r0); // 060717c2 mov.l r0,@(72,gbr)
        cpu.write16(r1, r0 as u16); // 060717c4 mov.w r0,@r1
        r7 = r7.wrapping_add(-36i32 as u32); // 060717c6 add #-36,r7
        r0 = r7; // 060717c8 mov r7,r0
        cpu.write32(w.wrapping_add(48), r0); // 060717ca mov.l r0,@(48,gbr)
        cpu.leave(&[(1, r1), (2, r2), (3, r3), (4, r4), (5, r5), (6, r6), (7, r7)]);
        cpu.leave_mac(mac);
        cpu.leave_t(t);
        return 1;
    }
    // turned: the four corners turned and scaled, stepped by the sprite's flips (a handler each)
    let mut r15 = cpu.stack() + 4;
    r15 = r15.wrapping_sub(4); cpu.write32(r15, r14); // 060717da mov.l r14,@-r15
    r15 = r15.wrapping_sub(4); cpu.write32(r15, r13); // 060717dc mov.l r13,@-r15
    r15 = r15.wrapping_sub(4); cpu.write32(r15, r12); // 060717de mov.l r12,@-r15
    r15 = r15.wrapping_sub(4); cpu.write32(r15, r11); // 060717e0 mov.l r11,@-r15
    r15 = r15.wrapping_sub(4); cpu.write32(r15, r10); // 060717e2 mov.l r10,@-r15
    r15 = r15.wrapping_sub(4); cpu.write32(r15, r9); // 060717e4 mov.l r9,@-r15
    r15 = r15.wrapping_sub(4); cpu.write32(r15, r8); // 060717e6 mov.l r8,@-r15
    r0 >>= 2; // 060717e8 shlr2 r0
    r0 >>= 2; // 060717ea shlr2 r0
    r15 = r15.wrapping_sub(4); cpu.write32(r15, r1); // 060717ec mov.l r1,@-r15
    r8 = 12i32 as u32; // 060717ee mov #12,r8
    r8 &= r0; // 060717f0 and r0,r8
    r0 = 0x060719ac; // 060717f2 mova 0x60719ac,r0
    r8 = r8.wrapping_add(r0); // 060717f4 add r0,r8
    r1 = cpu.read8(r8) as i8 as i32 as u32; r8 = r8.wrapping_add(1); // 060717f6 mov.b @r8+,r1
    r0 = cpu.read8(r8) as i8 as i32 as u32; r8 = r8.wrapping_add(1); // 060717f8 mov.b @r8+,r0
    r13 = r6; // 060717fa mov r6,r13
    r6 ^= r1; // 060717fc xor r1,r6
    r1 = 0x00003ffe; // 060717fe mov.w 0x60719a4,r1
    r13 ^= r0; // 06071800 xor r0,r13
    r0 = 0x06069b28; // 06071802 mov.l 0x60719a8,r0
    r6 &= r1; // 06071804 and r1,r6
    r13 &= r1; // 06071806 and r1,r13
    r1 = cpu.read8(r8) as i8 as i32 as u32; r8 = r8.wrapping_add(1); // 06071808 mov.b @r8+,r1
    r6 = cpu.read16(r0.wrapping_add(r6)) as i16 as i32 as u32; // 0607180a mov.w @(r0,r6),r6
    r13 = cpu.read16(r0.wrapping_add(r13)) as i16 as i32 as u32; // 0607180c mov.w @(r0,r13),r13
    r0 = cpu.read8(r8) as i8 as i32 as u32; // 0607180e mov.b @r8,r0
    r6 = r6 & 0xffff; // 06071810 extu.w r6,r6
    r6 = r6.wrapping_add(2i32 as u32); // 06071812 add #2,r6
    r6 ^= r1; // 06071814 xor r1,r6
    mac = r2 as i32 as i64 * r6 as i32 as i64; // 06071816 dmuls.l r2,r6
    r13 = r13 & 0xffff; // 06071818 extu.w r13,r13
    r13 = r13.wrapping_add(2i32 as u32); // 0607181a add #2,r13
    r13 ^= r0; // 0607181c xor r0,r13
    r1 = (mac >> 32) as u32; // 0607181e sts mach,r1
    r8 = mac as u32; // 06071820 sts macl,r8
    mac = r3 as i32 as i64 * r6 as i32 as i64; // 06071822 dmuls.l r3,r6
    r0 = cpu.read32(w.wrapping_add(152)); // 06071824 mov.l @(152,gbr),r0
    r8 = (r1 << 16) | (r8 >> 16); // 06071826 xtrct r1,r8
    r11 = r0; // 06071828 mov r0,r11
    r11 <<= 16; // 0607182a shll16 r11
    r1 = (mac >> 32) as u32; // 0607182c sts mach,r1
    r9 = mac as u32; // 0607182e sts macl,r9
    mac = r2 as i32 as i64 * r13 as i32 as i64; // 06071830 dmuls.l r2,r13
    r9 = (r1 << 16) | (r9 >> 16); // 06071832 xtrct r1,r9
    r10 = r0.rotate_left(16); // 06071834 swap.w r0,r10
    r10 <<= 16; // 06071836 shll16 r10
    r1 = (mac >> 32) as u32; // 06071838 sts mach,r1
    r2 = mac as u32; // 0607183a sts macl,r2
    mac = r3 as i32 as i64 * r13 as i32 as i64; // 0607183c dmuls.l r3,r13
    r0 = cpu.read32(w.wrapping_add(140)); // 0607183e mov.l @(140,gbr),r0
    r2 = (r1 << 16) | (r2 >> 16); // 06071840 xtrct r1,r2
    r13 = r0; // 06071842 mov r0,r13
    r13 <<= 16; // 06071844 shll16 r13
    r1 = (mac >> 32) as u32; // 06071846 sts mach,r1
    r3 = mac as u32; // 06071848 sts macl,r3
    r12 = r0.rotate_left(16); // 0607184a swap.w r0,r12
    r12 <<= 16; // 0607184c shll16 r12
    r0 = (r5 & 0xffff_0000) | ((r5 & 0xff) << 8) | ((r5 >> 8) & 0xff); // 0607184e swap.b r5,r0
    r3 = (r1 << 16) | (r3 >> 16); // 06071850 xtrct r1,r3
    r0 &= 15; // 06071852 and #15,r0
    r0 <<= 2; // 06071854 shll2 r0
    r1 = r0; // 06071856 mov r0,r1
    r0 = 0x060719c0; // 06071858 mova 0x60719c0,r0
    r1 = cpu.read32(r0.wrapping_add(r1)); // 0607185a mov.l @(r0,r1),r1
    r6 = cpu.read32(r4.wrapping_add(4)); // 0607185c mov.l @(4,r4),r6
    let handler = r1;
    r4 = cpu.read32(r4);
    let mut bail;
    match handler {
        0x0607_1a00 | 0x0607_1a0a => {
            if handler == 0x0607_1a00 {
                r5 = (r5 & 0xffff_0000) | ((r5 & 0xff) << 8) | ((r5 >> 8) & 0xff); // 06071a00 swap.b r5,r5
                r0 = -16i32 as u32; // 06071a02 mov #-16,r0
                r0 &= r5; // 06071a04 and r5,r0
                r0 |= 10; // 06071a06 or #10,r0
                r5 = (r0 & 0xffff_0000) | ((r0 & 0xff) << 8) | ((r0 >> 8) & 0xff); // 06071a08 swap.b r0,r5
            }
            r0 = r2; // 06071a0a mov r2,r0
            t = r0 & 1 != 0; r0 = (r0 as i32 >> 1) as u32; // 06071a0c shar r0
            r4 = r4.wrapping_sub(r0); // 06071a0e sub r0,r4
            r0 = r9; // 06071a10 mov r9,r0
            t = r0 & 1 != 0; r0 = (r0 as i32 >> 1) as u32; // 06071a12 shar r0
            r4 = r4.wrapping_add(r0); // 06071a14 add r0,r4
            r0 = r4; // 06071a16 mov r4,r0
            r0 = r0.wrapping_add(r10); // 06071a18 add r10,r0
            t = r0 > r12; // 06071a1a cmp/hi r12,r0
            bail = t;
            r0 = r3; // 06071a1e mov r3,r0
            if bail {
                return hv_off(cpu, [r1_pushed, r2, r3, r4, r5, r6, r7], mac, t);
            }
            t = r0 & 1 != 0; r0 = (r0 as i32 >> 1) as u32; // 06071a20 shar r0
            r6 = r6.wrapping_sub(r0); // 06071a22 sub r0,r6
            r0 = r8; // 06071a24 mov r8,r0
            t = r0 & 1 != 0; r0 = (r0 as i32 >> 1) as u32; // 06071a26 shar r0
            r6 = r6.wrapping_sub(r0); // 06071a28 sub r0,r6
            r0 = r6; // 06071a2a mov r6,r0
            r0 = r0.wrapping_add(r11); // 06071a2c add r11,r0
            t = r0 > r13; // 06071a2e cmp/hi r13,r0
            bail = t;
            r1 = r4.rotate_left(16); // 06071a32 swap.w r4,r1
            if bail {
                return hv_off(cpu, [r1_pushed, r2, r3, r4, r5, r6, r7], mac, t);
            }
            r4 = r4.wrapping_add(r2); // 06071a34 add r2,r4
            r0 = r4; // 06071a36 mov r4,r0
            r0 = r0.wrapping_add(r10); // 06071a38 add r10,r0
            t = r0 > r12; // 06071a3a cmp/hi r12,r0
            bail = t;
            cpu.write16(r7, r5 as u16); // 06071a3e mov.w r5,@r7
            if bail {
                return hv_off(cpu, [r1_pushed, r2, r3, r4, r5, r6, r7], mac, t);
            }
            r5 = r6; // 06071a40 mov r6,r5
            r6 = r6.wrapping_add(r8); // 06071a42 add r8,r6
            r0 = r6; // 06071a44 mov r6,r0
            r0 = r0.wrapping_add(r11); // 06071a46 add r11,r0
            t = r0 > r13; // 06071a48 cmp/hi r13,r0
            bail = t;
            r5 = (r1 << 16) | (r5 >> 16); // 06071a4c xtrct r1,r5
            if bail {
                return hv_off(cpu, [r1_pushed, r2, r3, r4, r5, r6, r7], mac, t);
            }
            r0 = r4.rotate_left(16); // 06071a4e swap.w r4,r0
            r1 = r6; // 06071a50 mov r6,r1
            r1 = (r0 << 16) | (r1 >> 16); // 06071a52 xtrct r0,r1
            r4 = r4.wrapping_sub(r9); // 06071a54 sub r9,r4
            r0 = r4; // 06071a56 mov r4,r0
            r0 = r0.wrapping_add(r10); // 06071a58 add r10,r0
            t = r0 > r12; // 06071a5a cmp/hi r12,r0
            bail = t;
            r6 = r6.wrapping_add(r3); // 06071a5e add r3,r6
            if bail {
                return hv_off(cpu, [r1_pushed, r2, r3, r4, r5, r6, r7], mac, t);
            }
            r0 = r6; // 06071a60 mov r6,r0
            r0 = r0.wrapping_add(r11); // 06071a62 add r11,r0
            t = r0 > r13; // 06071a64 cmp/hi r13,r0
            bail = t;
            r3 = r4.rotate_left(16); // 06071a68 swap.w r4,r3
            if bail {
                return hv_off(cpu, [r1_pushed, r2, r3, r4, r5, r6, r7], mac, t);
            }
            r4 = r4.wrapping_sub(r2); // 06071a6a sub r2,r4
            r10 = r10.wrapping_add(r4); // 06071a6c add r4,r10
            t = r10 > r12; // 06071a6e cmp/hi r12,r10
            bail = t;
            r9 = r6; // 06071a72 mov r6,r9
            if bail {
                return hv_off(cpu, [r1_pushed, r2, r3, r4, r5, r6, r7], mac, t);
            }
            r6 = r6.wrapping_sub(r8); // 06071a74 sub r8,r6
            r11 = r11.wrapping_add(r6); // 06071a76 add r6,r11
            t = r11 > r13; // 06071a78 cmp/hi r13,r11
            bail = t;
            r4 = r4.rotate_left(16); // 06071a7c swap.w r4,r4
            if bail {
                return hv_off(cpu, [r1_pushed, r2, r3, r4, r5, r6, r7], mac, t);
            }
            r6 = (r4 << 16) | (r6 >> 16);
        }
        0x0607_1a82 | 0x0607_1a8a | 0x0607_1a92 | 0x0607_1a9a => {
            match handler {
                0x0607_1a82 => {
                    r1 = 32;
                    r3 = r3.wrapping_neg();
                    r9 = r9.wrapping_neg();
                    r5 ^= r1;
                }
                0x0607_1a8a => {
                    r1 = 48;
                    r3 = r3.wrapping_neg();
                    r9 = r9.wrapping_neg();
                    r2 = r2.wrapping_neg();
                    r8 = r8.wrapping_neg();
                    r5 ^= r1;
                }
                0x0607_1a92 => {
                    r1 = 16;
                    r2 = r2.wrapping_neg();
                    r8 = r8.wrapping_neg();
                    r5 ^= r1;
                }
                _ => {}
            }
            r0 = r4; // 06071a9a mov r4,r0
            r0 = r0.wrapping_add(r10); // 06071a9c add r10,r0
            t = r0 > r12; // 06071a9e cmp/hi r12,r0
            bail = t;
            r0 = r6; // 06071aa2 mov r6,r0
            if bail {
                return hv_off(cpu, [r1_pushed, r2, r3, r4, r5, r6, r7], mac, t);
            }
            r0 = r0.wrapping_add(r11); // 06071aa4 add r11,r0
            t = r0 > r13; // 06071aa6 cmp/hi r13,r0
            bail = t;
            r1 = r4.rotate_left(16); // 06071aaa swap.w r4,r1
            if bail {
                return hv_off(cpu, [r1_pushed, r2, r3, r4, r5, r6, r7], mac, t);
            }
            r4 = r4.wrapping_add(r2); // 06071aac add r2,r4
            r0 = r4; // 06071aae mov r4,r0
            r0 = r0.wrapping_add(r10); // 06071ab0 add r10,r0
            t = r0 > r12; // 06071ab2 cmp/hi r12,r0
            bail = t;
            cpu.write16(r7, r5 as u16); // 06071ab6 mov.w r5,@r7
            if bail {
                return hv_off(cpu, [r1_pushed, r2, r3, r4, r5, r6, r7], mac, t);
            }
            r5 = r6; // 06071ab8 mov r6,r5
            r6 = r6.wrapping_add(r8); // 06071aba add r8,r6
            r0 = r6; // 06071abc mov r6,r0
            r0 = r0.wrapping_add(r11); // 06071abe add r11,r0
            t = r0 > r13; // 06071ac0 cmp/hi r13,r0
            bail = t;
            r0 = r4.rotate_left(16); // 06071ac4 swap.w r4,r0
            if bail {
                return hv_off(cpu, [r1_pushed, r2, r3, r4, r5, r6, r7], mac, t);
            }
            r5 = (r1 << 16) | (r5 >> 16); // 06071ac6 xtrct r1,r5
            r1 = r6; // 06071ac8 mov r6,r1
            r1 = (r0 << 16) | (r1 >> 16); // 06071aca xtrct r0,r1
            r4 = r4.wrapping_sub(r9); // 06071acc sub r9,r4
            r0 = r4; // 06071ace mov r4,r0
            r0 = r0.wrapping_add(r10); // 06071ad0 add r10,r0
            t = r0 > r12; // 06071ad2 cmp/hi r12,r0
            bail = t;
            r6 = r6.wrapping_add(r3); // 06071ad6 add r3,r6
            if bail {
                return hv_off(cpu, [r1_pushed, r2, r3, r4, r5, r6, r7], mac, t);
            }
            r0 = r6; // 06071ad8 mov r6,r0
            r0 = r0.wrapping_add(r11); // 06071ada add r11,r0
            t = r0 > r13; // 06071adc cmp/hi r13,r0
            bail = t;
            r3 = r4.rotate_left(16); // 06071ae0 swap.w r4,r3
            if bail {
                return hv_off(cpu, [r1_pushed, r2, r3, r4, r5, r6, r7], mac, t);
            }
            r4 = r4.wrapping_sub(r2); // 06071ae2 sub r2,r4
            r10 = r10.wrapping_add(r4); // 06071ae4 add r4,r10
            t = r10 > r12; // 06071ae6 cmp/hi r12,r10
            bail = t;
            r9 = r6; // 06071aea mov r6,r9
            if bail {
                return hv_off(cpu, [r1_pushed, r2, r3, r4, r5, r6, r7], mac, t);
            }
            r6 = r6.wrapping_sub(r8); // 06071aec sub r8,r6
            r11 = r11.wrapping_add(r6); // 06071aee add r6,r11
            t = r11 > r13; // 06071af0 cmp/hi r13,r11
            bail = t;
            r4 = r4.rotate_left(16); // 06071af4 swap.w r4,r4
            if bail {
                return hv_off(cpu, [r1_pushed, r2, r3, r4, r5, r6, r7], mac, t);
            }
            r6 = (r4 << 16) | (r6 >> 16);
        }
        0x0607_1b0e | 0x0607_1b16 => {
            if handler == 0x0607_1b0e {
                r1 = 16i32 as u32; // 06071b0e mov #16,r1
                r5 ^= r1; // 06071b10 xor r1,r5
                r2 = r2.wrapping_neg(); // 06071b12 neg r2,r2
                r8 = r8.wrapping_neg(); // 06071b14 neg r8,r8
            }
            r0 = r9; // 06071b16 mov r9,r0
            t = r0 & 1 != 0; r0 = (r0 as i32 >> 1) as u32; // 06071b18 shar r0
            r4 = r4.wrapping_add(r0); // 06071b1a add r0,r4
            r0 = r4; // 06071b1c mov r4,r0
            r0 = r0.wrapping_add(r10); // 06071b1e add r10,r0
            t = r0 > r12; // 06071b20 cmp/hi r12,r0
            bail = t;
            r0 = r3; // 06071b24 mov r3,r0
            if bail {
                return hv_off(cpu, [r1_pushed, r2, r3, r4, r5, r6, r7], mac, t);
            }
            t = r0 & 1 != 0; r0 = (r0 as i32 >> 1) as u32; // 06071b26 shar r0
            r6 = r6.wrapping_sub(r0); // 06071b28 sub r0,r6
            r0 = r6; // 06071b2a mov r6,r0
            r0 = r0.wrapping_add(r11); // 06071b2c add r11,r0
            t = r0 > r13; // 06071b2e cmp/hi r13,r0
            bail = t;
            r1 = r4.rotate_left(16); // 06071b32 swap.w r4,r1
            if bail {
                return hv_off(cpu, [r1_pushed, r2, r3, r4, r5, r6, r7], mac, t);
            }
            r4 = r4.wrapping_add(r2); // 06071b34 add r2,r4
            r0 = r4; // 06071b36 mov r4,r0
            r0 = r0.wrapping_add(r10); // 06071b38 add r10,r0
            t = r0 > r12; // 06071b3a cmp/hi r12,r0
            bail = t;
            cpu.write16(r7, r5 as u16); // 06071b3e mov.w r5,@r7
            if bail {
                return hv_off(cpu, [r1_pushed, r2, r3, r4, r5, r6, r7], mac, t);
            }
            r5 = r6; // 06071b40 mov r6,r5
            r6 = r6.wrapping_add(r8); // 06071b42 add r8,r6
            r0 = r6; // 06071b44 mov r6,r0
            r0 = r0.wrapping_add(r11); // 06071b46 add r11,r0
            t = r0 > r13; // 06071b48 cmp/hi r13,r0
            bail = t;
            r0 = r4.rotate_left(16); // 06071b4c swap.w r4,r0
            if bail {
                return hv_off(cpu, [r1_pushed, r2, r3, r4, r5, r6, r7], mac, t);
            }
            r5 = (r1 << 16) | (r5 >> 16); // 06071b4e xtrct r1,r5
            r1 = r6; // 06071b50 mov r6,r1
            r1 = (r0 << 16) | (r1 >> 16); // 06071b52 xtrct r0,r1
            r4 = r4.wrapping_sub(r9); // 06071b54 sub r9,r4
            r0 = r4; // 06071b56 mov r4,r0
            r0 = r0.wrapping_add(r10); // 06071b58 add r10,r0
            t = r0 > r12; // 06071b5a cmp/hi r12,r0
            bail = t;
            r6 = r6.wrapping_add(r3); // 06071b5e add r3,r6
            if bail {
                return hv_off(cpu, [r1_pushed, r2, r3, r4, r5, r6, r7], mac, t);
            }
            r0 = r6; // 06071b60 mov r6,r0
            r0 = r0.wrapping_add(r11); // 06071b62 add r11,r0
            t = r0 > r13; // 06071b64 cmp/hi r13,r0
            bail = t;
            r3 = r4.rotate_left(16); // 06071b68 swap.w r4,r3
            if bail {
                return hv_off(cpu, [r1_pushed, r2, r3, r4, r5, r6, r7], mac, t);
            }
            r4 = r4.wrapping_sub(r2); // 06071b6a sub r2,r4
            r10 = r10.wrapping_add(r4); // 06071b6c add r4,r10
            t = r10 > r12; // 06071b6e cmp/hi r12,r10
            bail = t;
            r9 = r6; // 06071b72 mov r6,r9
            if bail {
                return hv_off(cpu, [r1_pushed, r2, r3, r4, r5, r6, r7], mac, t);
            }
            r6 = r6.wrapping_sub(r8); // 06071b74 sub r8,r6
            r11 = r11.wrapping_add(r6); // 06071b76 add r6,r11
            t = r11 > r13; // 06071b78 cmp/hi r13,r11
            bail = t;
            r4 = r4.rotate_left(16); // 06071b7c swap.w r4,r4
            if bail {
                return hv_off(cpu, [r1_pushed, r2, r3, r4, r5, r6, r7], mac, t);
            }
            r6 = (r4 << 16) | (r6 >> 16);
        }
        0x0607_1b82 | 0x0607_1b8a => {
            if handler == 0x0607_1b82 {
                r1 = 32i32 as u32; // 06071b82 mov #32,r1
                r5 ^= r1; // 06071b84 xor r1,r5
                r3 = r3.wrapping_neg(); // 06071b86 neg r3,r3
                r9 = r9.wrapping_neg(); // 06071b88 neg r9,r9
            }
            r0 = r2; // 06071b8a mov r2,r0
            t = r0 & 1 != 0; r0 = (r0 as i32 >> 1) as u32; // 06071b8c shar r0
            r4 = r4.wrapping_sub(r0); // 06071b8e sub r0,r4
            r0 = r4; // 06071b90 mov r4,r0
            r0 = r0.wrapping_add(r10); // 06071b92 add r10,r0
            t = r0 > r12; // 06071b94 cmp/hi r12,r0
            bail = t;
            r0 = r8; // 06071b98 mov r8,r0
            if bail {
                return hv_off(cpu, [r1_pushed, r2, r3, r4, r5, r6, r7], mac, t);
            }
            t = r0 & 1 != 0; r0 = (r0 as i32 >> 1) as u32; // 06071b9a shar r0
            r6 = r6.wrapping_sub(r0); // 06071b9c sub r0,r6
            r0 = r6; // 06071b9e mov r6,r0
            r0 = r0.wrapping_add(r11); // 06071ba0 add r11,r0
            t = r0 > r13; // 06071ba2 cmp/hi r13,r0
            bail = t;
            r1 = r4.rotate_left(16); // 06071ba6 swap.w r4,r1
            if bail {
                return hv_off(cpu, [r1_pushed, r2, r3, r4, r5, r6, r7], mac, t);
            }
            r4 = r4.wrapping_add(r2); // 06071ba8 add r2,r4
            r0 = r4; // 06071baa mov r4,r0
            r0 = r0.wrapping_add(r10); // 06071bac add r10,r0
            t = r0 > r12; // 06071bae cmp/hi r12,r0
            bail = t;
            cpu.write16(r7, r5 as u16); // 06071bb2 mov.w r5,@r7
            if bail {
                return hv_off(cpu, [r1_pushed, r2, r3, r4, r5, r6, r7], mac, t);
            }
            r5 = r6; // 06071bb4 mov r6,r5
            r6 = r6.wrapping_add(r8); // 06071bb6 add r8,r6
            r0 = r6; // 06071bb8 mov r6,r0
            r0 = r0.wrapping_add(r11); // 06071bba add r11,r0
            t = r0 > r13; // 06071bbc cmp/hi r13,r0
            bail = t;
            r0 = r4.rotate_left(16); // 06071bc0 swap.w r4,r0
            if bail {
                return hv_off(cpu, [r1_pushed, r2, r3, r4, r5, r6, r7], mac, t);
            }
            r5 = (r1 << 16) | (r5 >> 16); // 06071bc2 xtrct r1,r5
            r1 = r6; // 06071bc4 mov r6,r1
            r1 = (r0 << 16) | (r1 >> 16); // 06071bc6 xtrct r0,r1
            r4 = r4.wrapping_sub(r9); // 06071bc8 sub r9,r4
            r0 = r4; // 06071bca mov r4,r0
            r0 = r0.wrapping_add(r10); // 06071bcc add r10,r0
            t = r0 > r12; // 06071bce cmp/hi r12,r0
            bail = t;
            r6 = r6.wrapping_add(r3); // 06071bd2 add r3,r6
            if bail {
                return hv_off(cpu, [r1_pushed, r2, r3, r4, r5, r6, r7], mac, t);
            }
            r0 = r6; // 06071bd4 mov r6,r0
            r0 = r0.wrapping_add(r11); // 06071bd6 add r11,r0
            t = r0 > r13; // 06071bd8 cmp/hi r13,r0
            bail = t;
            r3 = r4.rotate_left(16); // 06071bdc swap.w r4,r3
            if bail {
                return hv_off(cpu, [r1_pushed, r2, r3, r4, r5, r6, r7], mac, t);
            }
            r4 = r4.wrapping_sub(r2); // 06071bde sub r2,r4
            r10 = r10.wrapping_add(r4); // 06071be0 add r4,r10
            t = r10 > r12; // 06071be2 cmp/hi r12,r10
            bail = t;
            r9 = r6; // 06071be6 mov r6,r9
            if bail {
                return hv_off(cpu, [r1_pushed, r2, r3, r4, r5, r6, r7], mac, t);
            }
            r6 = r6.wrapping_sub(r8); // 06071be8 sub r8,r6
            r11 = r11.wrapping_add(r6); // 06071bea add r6,r11
            t = r11 > r13; // 06071bec cmp/hi r13,r11
            bail = t;
            r4 = r4.rotate_left(16); // 06071bf0 swap.w r4,r4
            if bail {
                return hv_off(cpu, [r1_pushed, r2, r3, r4, r5, r6, r7], mac, t);
            }
            r6 = (r4 << 16) | (r6 >> 16);
        }
        _ => unreachable!("the sprite's handler"),
    }
    // the corners on screen (unless the sprite says not to look): the first corner the one
    // furthest up and left, by the corners' words
    r9 = (r3 << 16) | (r9 >> 16); // 06071862 xtrct r3,r9
    r0 = cpu.read32(w.wrapping_add(144)); // 06071864 mov.l @(144,gbr),r0
    r3 = r0 as i16 as i32 as u32; // 06071866 exts.w r0,r3
    r2 = r0.rotate_left(16); // 06071868 swap.w r0,r2
    r0 = cpu.read32(w.wrapping_add(132)); // 0607186a mov.l @(132,gbr),r0
    r2 = r2 as i16 as i32 as u32; // 0607186c exts.w r2,r2
    r13 = r0 & 0xffff; // 0607186e extu.w r0,r13
    r12 = r0.rotate_left(16); // 06071870 swap.w r0,r12
    r0 = cpu.read32(r15); r15 = r15.wrapping_add(4); // 06071872 mov.l @r15+,r0
    r0 &= 4; // 06071874 and #4,r0
    t = r0 & r0 == 0; // 06071876 tst r0,r0
    let mut order = 0x0607_196c;
    if t {
        r0 = r5 as i16 as i32 as u32; // 0607187a exts.w r5,r0
        r0 = r0.wrapping_add(r3); // 0607187c add r3,r0
        t = r0 >= r13; // 0607187e cmp/hs r13,r0
        r12 = r12 & 0xffff;
        let mut first_ok = t;
        if !t {
            r0 = r5.rotate_left(16); // 06071884 swap.w r5,r0
            r0 = r0 as i16 as i32 as u32; // 06071886 exts.w r0,r0
            r0 = r0.wrapping_add(r2); // 06071888 add r2,r0
            t = r12 > r0; // 0607188a cmp/hi r0,r12
            first_ok = !t;
        }
        if first_ok {
            order = 0;
            r0 = r1 as i16 as i32 as u32; // 0607188e exts.w r1,r0
            r0 = r0.wrapping_add(r3); // 06071890 add r3,r0
            t = r0 >= r13; // 06071892 cmp/hs r13,r0
            r0 = r1.rotate_left(16);
            if !t {
                r0 = r0 as i16 as i32 as u32; // 06071898 exts.w r0,r0
                r0 = r0.wrapping_add(r2); // 0607189a add r2,r0
                t = r12 > r0; // 0607189c cmp/hi r0,r12
                if t {
                    order = 0x0607_193c;
                }
            }
            if order == 0 {
                r0 = r9 as i16 as i32 as u32; // 060718a0 exts.w r9,r0
                r0 = r0.wrapping_add(r3); // 060718a2 add r3,r0
                t = r0 >= r13; // 060718a4 cmp/hs r13,r0
                r0 = r9.rotate_left(16);
                if !t {
                    r0 = r0 as i16 as i32 as u32; // 060718aa exts.w r0,r0
                    r0 = r0.wrapping_add(r2); // 060718ac add r2,r0
                    t = r12 > r0; // 060718ae cmp/hi r0,r12
                    if t {
                        order = 0x0607_194c;
                    }
                }
            }
            if order == 0 {
                r0 = r6 as i16 as i32 as u32; // 060718b2 exts.w r6,r0
                r0 = r0.wrapping_add(r3); // 060718b4 add r3,r0
                t = r0 >= r13; // 060718b6 cmp/hs r13,r0
                r0 = r6.rotate_left(16);
                if !t {
                    r0 = r0 as i16 as i32 as u32; // 060718bc exts.w r0,r0
                    r0 = r0.wrapping_add(r2); // 060718be add r2,r0
                    t = r12 > r0; // 060718c0 cmp/hi r0,r12
                    if t {
                        order = 0x0607_195c;
                    }
                }
            }
            if order == 0 {
                r8 = 1i32 as u32; // 060718c4 mov #1,r8
                r8 <<= 16; // 060718c6 shll16 r8
                t = r8 & 1 != 0; r8 >>= 1; // 060718c8 shlr r8
                r2 = r6.rotate_left(16); // 060718ca swap.w r6,r2
                t = r8 & r2 == 0; // 060718cc tst r8,r2
                { let (x, b1) = r13.overflowing_sub(r13); let (y, b2) = x.overflowing_sub(t as u32); r13 = y; t = b1 || b2; } // 060718ce subc r13,r13
                r13 = !r13; // 060718d0 not r13,r13
                r13 ^= r2; // 060718d2 xor r2,r13
                t = r8 & r6 == 0; // 060718d4 tst r8,r6
                { let (x, b1) = r2.overflowing_sub(r2); let (y, b2) = x.overflowing_sub(t as u32); r2 = y; t = b1 || b2; } // 060718d6 subc r2,r2
                r2 = !r2; // 060718d8 not r2,r2
                r2 ^= r6; // 060718da xor r6,r2
                r13 = r13.wrapping_add(r2); // 060718dc add r2,r13
                r13 = r13 & 0xffff; // 060718de extu.w r13,r13
                r0 = 0x0607195c; // 060718e0 mova 0x607195c,r0
                r2 = r9.rotate_left(16); // 060718e2 swap.w r9,r2
                t = r8 & r2 == 0; // 060718e4 tst r8,r2
                { let (x, b1) = r12.overflowing_sub(r12); let (y, b2) = x.overflowing_sub(t as u32); r12 = y; t = b1 || b2; } // 060718e6 subc r12,r12
                r12 = !r12; // 060718e8 not r12,r12
                r12 ^= r2; // 060718ea xor r2,r12
                t = r8 & r9 == 0; // 060718ec tst r8,r9
                { let (x, b1) = r2.overflowing_sub(r2); let (y, b2) = x.overflowing_sub(t as u32); r2 = y; t = b1 || b2; } // 060718ee subc r2,r2
                r2 = !r2; // 060718f0 not r2,r2
                r2 ^= r9; // 060718f2 xor r9,r2
                r12 = r12.wrapping_add(r2); // 060718f4 add r2,r12
                r12 = r12 & 0xffff; // 060718f6 extu.w r12,r12
                t = r12 as i32 > r13 as i32; // 060718f8 cmp/gt r13,r12
                r2 = r1.rotate_left(16);
                if !t {
                    r13 = r12;
                    r0 = 0x0607_194c;
                }
                t = r8 & r2 == 0; // 06071902 tst r8,r2
                { let (x, b1) = r12.overflowing_sub(r12); let (y, b2) = x.overflowing_sub(t as u32); r12 = y; t = b1 || b2; } // 06071904 subc r12,r12
                r12 = !r12; // 06071906 not r12,r12
                r12 ^= r2; // 06071908 xor r2,r12
                t = r8 & r1 == 0; // 0607190a tst r8,r1
                { let (x, b1) = r2.overflowing_sub(r2); let (y, b2) = x.overflowing_sub(t as u32); r2 = y; t = b1 || b2; } // 0607190c subc r2,r2
                r2 = !r2; // 0607190e not r2,r2
                r2 ^= r1; // 06071910 xor r1,r2
                r12 = r12.wrapping_add(r2); // 06071912 add r2,r12
                r12 = r12 & 0xffff; // 06071914 extu.w r12,r12
                t = r12 as i32 > r13 as i32; // 06071916 cmp/gt r13,r12
                r2 = r5.rotate_left(16);
                if !t {
                    r13 = r12;
                    r0 = 0x0607_193c;
                }
                t = r8 & r2 == 0; // 06071920 tst r8,r2
                { let (x, b1) = r12.overflowing_sub(r12); let (y, b2) = x.overflowing_sub(t as u32); r12 = y; t = b1 || b2; } // 06071922 subc r12,r12
                r12 = !r12; // 06071924 not r12,r12
                r12 ^= r2; // 06071926 xor r2,r12
                t = r8 & r5 == 0; // 06071928 tst r8,r5
                { let (x, b1) = r2.overflowing_sub(r2); let (y, b2) = x.overflowing_sub(t as u32); r2 = y; t = b1 || b2; } // 0607192a subc r2,r2
                r2 = !r2; // 0607192c not r2,r2
                r2 ^= r5; // 0607192e xor r5,r2
                r12 = r12.wrapping_add(r2); // 06071930 add r2,r12
                t = r13 as i32 >= r12 as i32; // 06071932 cmp/ge r12,r13
                order = if t { 0x0607_196c } else { r0 };
            }
        }
    }
    match order {
        0x0607_193c => {
            r0 = cpu.read16(r7.wrapping_add(0)) as i16 as i32 as u32; // 0607193c mov.w @(0,r7),r0
            cpu.write32(r7.wrapping_add(16), r5); // 0607193e mov.l r5,@(16,r7)
            r0 ^= 16; // 06071940 xor #16,r0
            cpu.write16(r7.wrapping_add(0), r0 as u16); // 06071942 mov.w r0,@(0,r7)
            cpu.write32(r7.wrapping_add(12), r1); // 06071944 mov.l r1,@(12,r7)
            cpu.write32(r7.wrapping_add(24), r9); // 06071946 mov.l r9,@(24,r7)
            cpu.write32(r7.wrapping_add(20), r6); // 0607194a mov.l r6,@(20,r7)
        }
        0x0607_194c => {
            r0 = cpu.read16(r7.wrapping_add(0)) as i16 as i32 as u32; // 0607194c mov.w @(0,r7),r0
            cpu.write32(r7.wrapping_add(20), r5); // 0607194e mov.l r5,@(20,r7)
            r0 ^= 48; // 06071950 xor #48,r0
            cpu.write16(r7.wrapping_add(0), r0 as u16); // 06071952 mov.w r0,@(0,r7)
            cpu.write32(r7.wrapping_add(24), r1); // 06071954 mov.l r1,@(24,r7)
            cpu.write32(r7.wrapping_add(12), r9); // 06071956 mov.l r9,@(12,r7)
            cpu.write32(r7.wrapping_add(16), r6); // 0607195a mov.l r6,@(16,r7)
        }
        0x0607_195c => {
            r0 = cpu.read16(r7.wrapping_add(0)) as i16 as i32 as u32; // 0607195c mov.w @(0,r7),r0
            cpu.write32(r7.wrapping_add(24), r5); // 0607195e mov.l r5,@(24,r7)
            r0 ^= 32; // 06071960 xor #32,r0
            cpu.write16(r7.wrapping_add(0), r0 as u16); // 06071962 mov.w r0,@(0,r7)
            cpu.write32(r7.wrapping_add(20), r1); // 06071964 mov.l r1,@(20,r7)
            cpu.write32(r7.wrapping_add(16), r9); // 06071966 mov.l r9,@(16,r7)
            cpu.write32(r7.wrapping_add(12), r6); // 0607196a mov.l r6,@(12,r7)
        }
        _ => {
            cpu.write32(r7.wrapping_add(12), r5); // 0607196c mov.l r5,@(12,r7)
            cpu.write32(r7.wrapping_add(16), r1); // 0607196e mov.l r1,@(16,r7)
            cpu.write32(r7.wrapping_add(20), r9); // 06071970 mov.l r9,@(20,r7)
            cpu.write32(r7.wrapping_add(24), r6); // 06071972 mov.l r6,@(24,r7)
        }
    }
    // the command for the slot, the pushes popped
    r0 = cpu.read32(w.wrapping_add(72)); // 06071974 mov.l @(72,gbr),r0
    r1 = 40i32 as u32; // 06071976 mov #40,r1
    cpu.write32(r0.wrapping_add(0), r1); // 06071978 mov.l r1,@(0,r0)
    cpu.write32(r0.wrapping_add(4), r7); // 0607197a mov.l r7,@(4,r0)
    r1 = 1i32 as u32; // 0607197c mov #1,r1
    cpu.write32(r0.wrapping_add(8), r1); // 0607197e mov.l r1,@(8,r0)
    r1 = 0i32 as u32; // 06071980 mov #0,r1
    cpu.write32(r0.wrapping_add(12), r1); // 06071982 mov.l r1,@(12,r0)
    r0 = r0.wrapping_add(16i32 as u32); // 06071984 add #16,r0
    r1 = 0x21000000; // 06071986 mov.l 0x60719bc,r1
    cpu.write32(w.wrapping_add(72), r0); // 06071988 mov.l r0,@(72,gbr)
    cpu.write16(r1, r0 as u16); // 0607198a mov.w r0,@r1
    r8 = cpu.read32(r15); r15 = r15.wrapping_add(4); // 0607198c mov.l @r15+,r8
    r9 = cpu.read32(r15); r15 = r15.wrapping_add(4); // 0607198e mov.l @r15+,r9
    r10 = cpu.read32(r15); r15 = r15.wrapping_add(4); // 06071990 mov.l @r15+,r10
    r11 = cpu.read32(r15); r15 = r15.wrapping_add(4); // 06071992 mov.l @r15+,r11
    r12 = cpu.read32(r15); r15 = r15.wrapping_add(4); // 06071994 mov.l @r15+,r12
    r13 = cpu.read32(r15); r15 = r15.wrapping_add(4); // 06071996 mov.l @r15+,r13
    r14 = cpu.read32(r15); r15 = r15.wrapping_add(4); // 06071998 mov.l @r15+,r14
    r7 = r7.wrapping_add(-36i32 as u32); // 0607199a add #-36,r7
    r0 = r7; // 0607199c mov r7,r0
    cpu.write32(w.wrapping_add(48), r0); // 0607199e mov.l r0,@(48,gbr)
    let _ = r15;
    cpu.leave(&[(1, r1), (2, r2), (3, r3), (4, r4), (5, r5), (6, r6), (7, r7)]);
    cpu.leave_mac(mac);
    cpu.leave_t(t);
    1

}

/// A polygon command of the caller's (`cmd`: 15 words, as VDP1's command as far as can be told)
/// put at depth `z`: counted and checked as a polygon is (put_check), copied by DMA channel 0
/// (after any copy before it ends, and waited for) into the polygon buffer's next slot (36 bytes
/// below the last), the depth's whole part after the copy, and the slave sent command 40 for that
/// one polygon. 1, or 0.
const POLYGON_BUFFER: u32 = 0x30;
const QUAD_COMMAND: u32 = 40;
const QUAD_SLOT: u32 = 36;
const QUAD_WORDS: u32 = 15;
const QUAD_DEPTH: u32 = 30;
/// DMA channel 0's registers (source, destination, count, control), the DMA operation register
/// and channel 0's request selection; the copy's control (both addresses counted up, by words,
/// auto request, enabled) and the operation's (DMA on, priority by turns).
const SAR0: u32 = 0xffff_ff80;
const DAR0: u32 = 0xffff_ff84;
const TCR0: u32 = 0xffff_ff88;
const CHCR0: u32 = 0xffff_ff8c;
const DMAOR: u32 = 0xffff_ffb0;
const DRCR0: u32 = 0xffff_fe71;
const COPY_WORDS: u32 = 0x5601;
const DMA_ON: u32 = 9;

fn dma0_wait(cpu: &mut Cpu, at: u32) {
    while cpu.read32(CHCR0) & 3 == 1 {
        cpu.wait(at, 4);
    }
}

pub fn sl_put_quad(cpu: &mut Cpu, cmd: u32, z: u32) -> u32 {
    let Some((past, _, _)) = put_check(cpu, z) else {
        return 0;
    };
    let w = cpu.gbr();
    let slot = cpu.read32(w + POLYGON_BUFFER);
    let c = cpu.read32(w + COMMANDS);
    for (k, v) in [QUAD_COMMAND, slot, 1, 0].into_iter().enumerate() {
        cpu.write32(c + 4 * k as u32, v);
    }
    cpu.write16(slot + QUAD_DEPTH, (past >> 16) as u16);
    dma0_wait(cpu, 0x0606_77b6);
    cpu.read32(DMAOR);
    cpu.write32(CHCR0, 0);
    cpu.write8(DRCR0, 0);
    cpu.write32(SAR0, cmd);
    cpu.write32(DAR0, slot);
    cpu.write32(TCR0, QUAD_WORDS);
    cpu.write32(CHCR0, COPY_WORDS);
    cpu.write32(DMAOR, DMA_ON);
    dma0_wait(cpu, 0x0606_77dc);
    let next = c + 16;
    cpu.write32(w + COMMANDS, next);
    cpu.write16(SLAVE_SIGNAL, next as u16);
    cpu.write32(w + POLYGON_BUFFER, slot.wrapping_sub(QUAD_SLOT));
    cpu.leave(&[(1, SAR0), (2, past), (3, SAR0), (5, QUAD_WORDS), (6, SLAVE_SIGNAL), (7, slot)]);
    cpu.leave_t(false);
    1
}

/// SGL's on-screen check of the point at `p` (x, y, z) with a size around it: its depth through
/// the current matrix; -1 if nearer than the window's near (put_check's), -2 if past it by more
/// than the far limit, or if the point, scaled to the screen by the window's depth over its depth
/// (on the divide unit) and give or take half its size so scaled, is off the window's edges;
/// else how far past the near it is. (The SH-2 code pushes the window's depth below its frame
/// while it takes the near: PROLOGUES.)
const SCREEN_LEFT: u32 = 0x78;
const SCREEN_TOP: u32 = 0x7a;
const SCREEN_RIGHT: u32 = 0x7c;
const SCREEN_BOTTOM: u32 = 0x7e;
const NEARER: u32 = -1i32 as u32;
const OFF_SCREEN: u32 = -2i32 as u32;

pub fn sl_check_on_screen(cpu: &mut Cpu, p: u32, size: u32) -> u32 {
    let sp = cpu.stack();
    let w = cpu.gbr();
    let m = matrix(cpu);
    let pt = [0, 1, 2].map(|k| cpu.read32(p + 4 * k));
    let row = |cpu: &mut Cpu, r: u32| {
        let mut sum = 0;
        for (k, &v) in pt.iter().enumerate() {
            let e = cpu.read32(m + 16 * r + 4 * k as u32);
            sum = mac(cpu, sum, v, e);
        }
        (sum, cpu.read32(m + 16 * r + 12))
    };
    let depth = cpu.read32(w + WIN_DEPTH);
    let (zsum, m23) = row(cpu, 2);
    cpu.write32(DVDNTH, (depth as i32 >> 16) as u32);
    cpu.write32(sp - 4, depth);
    let z = middle(zsum).wrapping_add(m23);
    let shift = cpu.read8(w + WIN_FLAG) as i8 as i32;
    let near = if (1..32).contains(&shift) { depth >> shift } else { 0 };
    let common = [(1, m23), (2, near), (4, p + 12)];
    if (z as i32) < near as i32 {
        cpu.leave(&common);
        cpu.leave(&[(3, z)]);
        cpu.leave_mac(zsum);
        cpu.leave_t(false);
        return NEARER;
    }
    cpu.write32(DVSR, z);
    let past = z.wrapping_sub(near);
    cpu.write32(DVDNTL, depth << 16);
    let (xsum, m03) = row(cpu, 0);
    if past > (cpu.read16(w + FAR) as u32) << 16 {
        cpu.leave(&common);
        cpu.leave(&[(3, past)]);
        cpu.leave_mac(xsum);
        cpu.leave_t(true);
        return OFF_SCREEN;
    }
    let x = middle(xsum).wrapping_add(m03);
    let (ysum, m13) = row(cpu, 1);
    let y = middle(ysum).wrapping_add(m13);
    let q = cpu.read32(DVDNTL_AGAIN);
    let sx = middle(product(x, q));
    let hp = ((size as i32 >> 1) as u32 as u64 * q as u64) as i64;
    let half = middle(hp);
    let yq = product(y, q);
    cpu.leave(&[(3, past), (4, (hp >> 32) as u32), (5, half)]);
    cpu.leave_mac(yq);
    let edge = |cpu: &mut Cpu, at: u32| (cpu.read16(w + at) as u32) << 16;
    let off = |cpu: &mut Cpu, regs: &[(usize, u32)], t: bool| {
        cpu.leave(regs);
        cpu.leave_t(t);
        OFF_SCREEN
    };
    if (sx.wrapping_add(half) as i32) < edge(cpu, SCREEN_LEFT) as i32 {
        return off(cpu, &[(1, sx.wrapping_add(half)), (2, y)], false);
    }
    let right = edge(cpu, SCREEN_RIGHT);
    if sx.wrapping_sub(half) as i32 > right as i32 {
        return off(cpu, &[(1, sx.wrapping_sub(half)), (2, y)], true);
    }
    let sy = middle(yq);
    if (sy.wrapping_add(half) as i32) < edge(cpu, SCREEN_TOP) as i32 {
        return off(cpu, &[(1, (yq >> 32) as u32), (2, sy.wrapping_add(half))], false);
    }
    if sy.wrapping_sub(half) as i32 >= edge(cpu, SCREEN_BOTTOM) as i32 {
        return off(cpu, &[(1, (yq >> 32) as u32), (2, sy.wrapping_sub(half))], true);
    }
    cpu.leave(&[(1, (yq >> 32) as u32), (2, sy.wrapping_sub(half))]);
    cpu.leave_t(false);
    past
}

/// slDispSprite: the sprite at `pos` (x, y, z through the current matrix, and a scale: its size
/// at the window's depth, its sign ignored) with the attributes at `attr` (as slPutSprite's)
/// turned by `angle`: put where it projects (its place and size times the window's depth over
/// its depth, on the divide unit), as slPutSprite sends it; not when its depth past the window's
/// near is beyond the far limit (compared unsigned: one nearer than the near is too) or once the
/// frame's polygons are at their limit (else counted as one). 1 either way, the put flag set.
pub fn sl_disp_sprite(cpu: &mut Cpu, pos: u32, attr: u32, angle: u32) -> u32 {
    let w = cpu.gbr();
    let n = cpu.read16(w + POLYGONS_PUT).wrapping_add(1);
    cpu.write16(w + POLYGONS_PUT, n);
    let m = matrix(cpu);
    let pos = pos & !3;
    let pt = [0, 1, 2].map(|k| cpu.read32(pos + 4 * k));
    let row = |cpu: &mut Cpu, r: u32| {
        let mut sum = 0;
        for (k, &v) in pt.iter().enumerate() {
            let e = cpu.read32(m + 16 * r + 4 * k as u32);
            sum = mac(cpu, sum, v, e);
        }
        (sum, cpu.read32(m + 16 * r + 12))
    };
    let depth = cpu.read32(w + WIN_DEPTH);
    let (zsum, m23) = row(cpu, 2);
    let shift = cpu.read8(w + WIN_FLAG) as i8 as i32;
    let near = if (1..32).contains(&shift) { depth >> shift } else { 0 };
    let z = middle(zsum).wrapping_add(m23);
    let past = z.wrapping_sub(near);
    let done = |cpu: &mut Cpu, regs: &[(usize, u32)]| {
        cpu.leave(regs);
        cpu.leave_mac(zsum);
        cpu.leave_t(true);
        cpu.write8(w + PUT_FLAG, 1);
        1
    };
    if past > (cpu.read16(w + FAR) as u32) << 16 {
        return done(cpu, &[(1, (zsum >> 32) as u32), (2, m23), (3, past), (4, pos), (7, z)]);
    }
    cpu.write32(DVSR, z);
    cpu.write32(DVDNTH, (depth as i32 >> 16) as u32);
    cpu.write32(DVDNTL, depth << 16);
    let limit = cpu.read16(w + LIMITS) as u32;
    let count = cpu.read16(w + COUNTS) as u32;
    if count >= limit {
        let r7 = (depth as i32 >> 16) as u32;
        return done(cpu, &[(1, -2i32 as u32), (2, m23), (3, past), (4, pos), (7, r7)]);
    }
    cpu.write16(w + COUNTS, (count + 1) as u16);
    let attr = attr & !1;
    let c = cpu.read32(w + COMMANDS);
    let dir = cpu.read16(attr + 8) as u32;
    cpu.write32(c + 4, (dir << 16) | (past >> 16));
    let (xsum, m03) = row(cpu, 0);
    let x = middle(xsum).wrapping_add(m03);
    let word = |cpu: &mut Cpu, k: u32| cpu.read16(attr + 2 * k) as i16 as i32 as u32;
    let (texno, atrb, colno, gstb) = (word(cpu, 0), word(cpu, 1), word(cpu, 2), word(cpu, 3));
    let (ysum, m13) = row(cpu, 1);
    let y = middle(ysum).wrapping_add(m13);
    let scale = (cpu.read32(pos + 12) as i32).unsigned_abs();
    cpu.write32(c + 12, (atrb << 16) | (colno & 0xffff));
    let q = cpu.read32(DVDNTL_AGAIN);
    cpu.write32(c + 8, (texno << 16) | (gstb & 0xffff));
    cpu.write32(c + 24, middle(product(q, scale)));
    cpu.write32(c + 16, middle(product(q, x)));
    let yq = product(q, y);
    let sy = middle(yq);
    cpu.write32(c + 20, sy);
    cpu.leave_mac(yq);
    let angle = angle & 0xffff;
    let (code, regs) = if angle == 0 {
        (SPRITE_COMMAND, [(2, (yq >> 32) as u32), (4, 0), (5, attr + 8)])
    } else {
        let e = SPRITE_QUADRANTS + ((angle >> 14) & 3) * 4;
        let byte = |cpu: &mut Cpu, k: u32| cpu.read8(e + k) as i8 as i32 as u32;
        let mut v = [0; 2];
        for (k, v) in v.iter_mut().enumerate() {
            let k = k as u32;
            let i = (angle ^ byte(cpu, k)) & SIN_COS_STEPS;
            let step = cpu.read16(SIN_COS_TABLE + i) as u32;
            *v = step.wrapping_add(2) ^ byte(cpu, 2 + k);
        }
        cpu.write32(c + 28, v[0]);
        cpu.write32(c + 32, v[1]);
        (TURNED_SPRITE_COMMAND, [(2, e + 4), (4, v[0]), (5, v[1])])
    };
    cpu.write32(c, code);
    let next = c + if angle == 0 { 28 } else { 36 };
    cpu.write32(w + COMMANDS, next);
    cpu.write16(SLAVE_SIGNAL, next as u16);
    cpu.leave(&regs);
    cpu.leave(&[(1, SLAVE_SIGNAL), (3, sy), (7, y)]);
    cpu.leave_t(angle == 0);
    cpu.write8(w + PUT_FLAG, 1);
    1
}

/// slTan: the tangent of `a` (16.16), from a table of steps: the angle's 32nd of a turn picks an
/// entry (8 bytes: a handler, a mask for the index, two bytes added to the step and xored into
/// the result); each handler shifts the step left by its own amount (the further from the axis,
/// the more), the one-bit shift leaving T the bit shifted out.
const TAN_ENTRIES: u32 = 0x0606_eb04;
const TAN_STEPS: u32 = 0x0606_ec38;
const TAN_INDEX: u32 = 0x3ffc;
const TAN_HANDLERS: [(u32, u32); 5] = [(0x0606_ec04, 0), (0x0606_ec0c, 1), (0x0606_ec16, 2), (0x0606_ec20, 4), (0x0606_ec2c, 8)];

pub fn sl_tan(cpu: &mut Cpu, a: u32) -> u32 {
    let a = a.wrapping_add(2);
    let e = TAN_ENTRIES + ((a >> 10) & 0x1f) * 8;
    let handler = cpu.read32(e);
    let mask = cpu.read16(e + 4) as i16 as i32 as u32;
    let xor = cpu.read8(e + 6) as i8 as i32 as u32;
    let add = cpu.read8(e + 7) as i8 as i32 as u32;
    let i = ((a ^ mask) & TAN_INDEX) >> 1;
    let v = (cpu.read16(TAN_STEPS + i) as u32).wrapping_add(add);
    let shift = TAN_HANDLERS.iter().find(|(h, _)| *h == handler).map(|&(_, s)| s).expect("slTan's handler");
    if shift == 1 {
        cpu.leave_t(v >> 31 != 0);
    }
    cpu.leave(&[(1, handler), (2, add), (3, xor), (4, i), (5, TAN_INDEX)]);
    (v << shift) ^ xor
}

/// The current matrix inverted in place (Gauss-Jordan, as SGL's asm does it): at each of the
/// first two steps the row with the biggest pivot swapped up (its first columns kept in
/// registers, the swap noted in a code byte), the other rows reduced by it with the quotients on
/// the divide unit (each started one step before it is read; the place's products a byte lower),
/// the pivot row divided through; the swaps undone at the end. Written step for step from the
/// SH-2 code (tools/port/asm2rs.py), each step with its address, and its registers left as it
/// leaves them.
#[allow(unused_assignments, clippy::all)]
pub fn sl_inverse(cpu: &mut Cpu) {
    let w = cpu.gbr();
    let [mut r0, mut r1, mut r2, mut r3, mut r4, mut r5, mut r6, mut r7, mut r8, mut r9, mut r10, ..] = cpu.sh2.r;
    let mut mac: i64;
    let mut t: bool;
    r0 = cpu.read32(w + MATRIX);
    // the first pivot: the biggest of the first column
    r6 = r0; // 06071d34 mov r0,r6
    r0 = cpu.read32(r6.wrapping_add(0)); // 06071d36 mov.l @(0,r6),r0
    r9 = 0i32 as u32; // 06071d38 mov #0,r9
    r3 = r0; // 06071d3a mov r0,r3
    t = r0 as i32 >= 0; // 06071d3c cmp/pz r0
    r1 = cpu.read32(r6.wrapping_add(16));
    if !t {
        r3 = r0.wrapping_neg();
    }
    r4 = r1; // 06071d44 mov r1,r4
    t = r1 as i32 >= 0; // 06071d46 cmp/pz r1
    r2 = cpu.read32(r6.wrapping_add(32));
    if !t {
        r4 = r1.wrapping_neg();
    }
    t = r3 >= r4;
    r5 = r2;
    if !t {
        r3 = r4;
        r9 = 1;
    }
    t = r2 as i32 >= 0;
    if !t {
        r5 = r2.wrapping_neg();
    }
    t = r3 >= r5;
    r8 = cpu.read32(r6.wrapping_add(4));
    let swapped = if !t {
        // the third row's: rows 0 and 2 swapped (their first column kept in registers)
        r9 = cpu.read32(r6.wrapping_add(8)); // 06071d64 mov.l @(8,r6),r9
        r3 = cpu.read32(r6.wrapping_add(12)); // 06071d66 mov.l @(12,r6),r3
        r4 = cpu.read32(r6.wrapping_add(44)); // 06071d68 mov.l @(44,r6),r4
        r7 = cpu.read32(r6.wrapping_add(40)); // 06071d6a mov.l @(40,r6),r7
        r5 = cpu.read32(r6.wrapping_add(36)); // 06071d6c mov.l @(36,r6),r5
        cpu.write32(r6.wrapping_add(44), r3); // 06071d6e mov.l r3,@(44,r6)
        cpu.write32(r6.wrapping_add(12), r4); // 06071d70 mov.l r4,@(12,r6)
        cpu.write32(r6.wrapping_add(40), r9); // 06071d72 mov.l r9,@(40,r6)
        cpu.write32(r6.wrapping_add(8), r7); // 06071d74 mov.l r7,@(8,r6)
        cpu.write32(r6.wrapping_add(36), r8); // 06071d76 mov.l r8,@(36,r6)
        r3 = r2; // 06071d78 mov r2,r3
        r2 = r0; // 06071d7a mov r0,r2
        r9 = 2;
        true
    } else {
        t = r9 == 0;
        if !t {
            // the second row's: rows 0 and 1 swapped
            r9 = cpu.read32(r6.wrapping_add(8)); // 06071d84 mov.l @(8,r6),r9
            r3 = cpu.read32(r6.wrapping_add(12)); // 06071d86 mov.l @(12,r6),r3
            r4 = cpu.read32(r6.wrapping_add(28)); // 06071d88 mov.l @(28,r6),r4
            r7 = cpu.read32(r6.wrapping_add(24)); // 06071d8a mov.l @(24,r6),r7
            r5 = cpu.read32(r6.wrapping_add(20)); // 06071d8c mov.l @(20,r6),r5
            cpu.write32(r6.wrapping_add(28), r3); // 06071d8e mov.l r3,@(28,r6)
            cpu.write32(r6.wrapping_add(12), r4); // 06071d90 mov.l r4,@(12,r6)
            cpu.write32(r6.wrapping_add(24), r9); // 06071d92 mov.l r9,@(24,r6)
            cpu.write32(r6.wrapping_add(8), r7); // 06071d94 mov.l r7,@(8,r6)
            cpu.write32(r6.wrapping_add(20), r8); // 06071d96 mov.l r8,@(20,r6)
            r3 = r1; // 06071d98 mov r1,r3
            r1 = r0; // 06071d9a mov r0,r1
            r9 = 1i32 as u32; // 06071d9c mov #1,r9
            true
        } else {
            false
        }
    };
    if swapped {
        r8 = r5;
        r0 = r3;
    }
    // the first elimination (pivot row 0), the quotients on the divide unit one step behind
    mac = r1 as i32 as i64 * r8 as i32 as i64; // 06071da2 dmuls.l r1,r8
    r10 = -1i32 as u32; // 06071da4 mov #-1,r10
    r10 <<= 8; // 06071da6 shll8 r10
    cpu.write32(r10.wrapping_add(0), r0); // 06071da8 mov.l r0,@(0,r10)
    r4 = (mac >> 32) as u32; // 06071daa sts mach,r4
    r5 = mac as u32; // 06071dac sts macl,r5
    cpu.write32(r10.wrapping_add(16), r4); // 06071dae mov.l r4,@(16,r10)
    cpu.write32(r10.wrapping_add(20), r5); // 06071db0 mov.l r5,@(20,r10)
    r5 = cpu.read32(r6.wrapping_add(20)); // 06071db2 mov.l @(20,r6),r5
    r4 = cpu.read32(r10.wrapping_add(28)); // 06071db4 mov.l @(28,r10),r4
    mac = r2 as i32 as i64 * r8 as i32 as i64; // 06071db6 dmuls.l r2,r8
    r5 = r5.wrapping_sub(r4); // 06071db8 sub r4,r5
    cpu.write32(r6.wrapping_add(20), r5); // 06071dba mov.l r5,@(20,r6)
    cpu.write32(r10.wrapping_add(0), r0); // 06071dbc mov.l r0,@(0,r10)
    r4 = (mac >> 32) as u32; // 06071dbe sts mach,r4
    r5 = mac as u32; // 06071dc0 sts macl,r5
    cpu.write32(r10.wrapping_add(16), r4); // 06071dc2 mov.l r4,@(16,r10)
    cpu.write32(r10.wrapping_add(20), r5); // 06071dc4 mov.l r5,@(20,r10)
    r5 = cpu.read32(r6.wrapping_add(36)); // 06071dc6 mov.l @(36,r6),r5
    r3 = r8.rotate_left(16); // 06071dc8 swap.w r8,r3
    r3 = r3 as i16 as i32 as u32; // 06071dca exts.w r3,r3
    r8 <<= 16; // 06071dcc shll16 r8
    r4 = cpu.read32(r10.wrapping_add(28)); // 06071dce mov.l @(28,r10),r4
    cpu.write32(r10.wrapping_add(0), r0); // 06071dd0 mov.l r0,@(0,r10)
    cpu.write32(r10.wrapping_add(16), r3); // 06071dd2 mov.l r3,@(16,r10)
    cpu.write32(r10.wrapping_add(20), r8); // 06071dd4 mov.l r8,@(20,r10)
    r5 = r5.wrapping_sub(r4); // 06071dd6 sub r4,r5
    cpu.write32(r6.wrapping_add(36), r5); // 06071dd8 mov.l r5,@(36,r6)
    r8 = cpu.read32(r6.wrapping_add(8)); // 06071dda mov.l @(8,r6),r8
    r3 = cpu.read32(r10.wrapping_add(28)); // 06071ddc mov.l @(28,r10),r3
    mac = r1 as i32 as i64 * r8 as i32 as i64; // 06071dde dmuls.l r1,r8
    r3 = r3.wrapping_neg(); // 06071de0 neg r3,r3
    cpu.write32(r6.wrapping_add(4), r3); // 06071de2 mov.l r3,@(4,r6)
    cpu.write32(r10.wrapping_add(0), r0); // 06071de4 mov.l r0,@(0,r10)
    r4 = (mac >> 32) as u32; // 06071de6 sts mach,r4
    r5 = mac as u32; // 06071de8 sts macl,r5
    cpu.write32(r10.wrapping_add(16), r4); // 06071dea mov.l r4,@(16,r10)
    cpu.write32(r10.wrapping_add(20), r5); // 06071dec mov.l r5,@(20,r10)
    r5 = cpu.read32(r6.wrapping_add(24)); // 06071dee mov.l @(24,r6),r5
    r4 = cpu.read32(r10.wrapping_add(28)); // 06071df0 mov.l @(28,r10),r4
    mac = r2 as i32 as i64 * r8 as i32 as i64; // 06071df2 dmuls.l r2,r8
    r5 = r5.wrapping_sub(r4); // 06071df4 sub r4,r5
    cpu.write32(r6.wrapping_add(24), r5); // 06071df6 mov.l r5,@(24,r6)
    cpu.write32(r10.wrapping_add(0), r0); // 06071df8 mov.l r0,@(0,r10)
    r4 = (mac >> 32) as u32; // 06071dfa sts mach,r4
    r5 = mac as u32; // 06071dfc sts macl,r5
    cpu.write32(r10.wrapping_add(16), r4); // 06071dfe mov.l r4,@(16,r10)
    cpu.write32(r10.wrapping_add(20), r5); // 06071e00 mov.l r5,@(20,r10)
    r3 = r8.rotate_left(16); // 06071e02 swap.w r8,r3
    r3 = r3 as i16 as i32 as u32; // 06071e04 exts.w r3,r3
    r8 <<= 16; // 06071e06 shll16 r8
    r5 = cpu.read32(r6.wrapping_add(40)); // 06071e08 mov.l @(40,r6),r5
    r4 = cpu.read32(r10.wrapping_add(28)); // 06071e0a mov.l @(28,r10),r4
    cpu.write32(r10.wrapping_add(0), r0); // 06071e0c mov.l r0,@(0,r10)
    cpu.write32(r10.wrapping_add(16), r3); // 06071e0e mov.l r3,@(16,r10)
    cpu.write32(r10.wrapping_add(20), r8); // 06071e10 mov.l r8,@(20,r10)
    r5 = r5.wrapping_sub(r4); // 06071e12 sub r4,r5
    cpu.write32(r6.wrapping_add(40), r5); // 06071e14 mov.l r5,@(40,r6)
    r8 = cpu.read32(r6.wrapping_add(12)); // 06071e16 mov.l @(12,r6),r8
    r3 = cpu.read32(r10.wrapping_add(28)); // 06071e18 mov.l @(28,r10),r3
    r4 = r8.rotate_left(16); // 06071e1a swap.w r8,r4
    r4 = (r4 & 0xffff_0000) | ((r4 & 0xff) << 8) | ((r4 >> 8) & 0xff); // 06071e1c swap.b r4,r4
    r4 = r4 as i8 as i32 as u32; // 06071e1e exts.b r4,r4
    r8 <<= 8; // 06071e20 shll8 r8
    cpu.write32(r10.wrapping_add(0), r0); // 06071e22 mov.l r0,@(0,r10)
    cpu.write32(r10.wrapping_add(16), r4); // 06071e24 mov.l r4,@(16,r10)
    cpu.write32(r10.wrapping_add(20), r8); // 06071e26 mov.l r8,@(20,r10)
    r3 = r3.wrapping_neg(); // 06071e28 neg r3,r3
    cpu.write32(r6.wrapping_add(8), r3); // 06071e2a mov.l r3,@(8,r6)
    r8 = cpu.read32(r10.wrapping_add(28)); // 06071e2c mov.l @(28,r10),r8
    mac = r1 as i32 as i64 * r8 as i32 as i64; // 06071e2e dmuls.l r1,r8
    r5 = cpu.read32(r6.wrapping_add(28)); // 06071e30 mov.l @(28,r6),r5
    r3 = (mac >> 32) as u32; // 06071e32 sts mach,r3
    r4 = mac as u32; // 06071e34 sts macl,r4
    r3 <<= 8; // 06071e36 shll8 r3
    r3 <<= 16; // 06071e38 shll16 r3
    r4 >>= 8; // 06071e3a shlr8 r4
    r4 |= r3; // 06071e3c or r3,r4
    r5 = r5.wrapping_sub(r4); // 06071e3e sub r4,r5
    cpu.write32(r6.wrapping_add(28), r5); // 06071e40 mov.l r5,@(28,r6)
    mac = r2 as i32 as i64 * r8 as i32 as i64; // 06071e42 dmuls.l r2,r8
    r5 = cpu.read32(r6.wrapping_add(44)); // 06071e44 mov.l @(44,r6),r5
    r3 = (mac >> 32) as u32; // 06071e46 sts mach,r3
    r4 = mac as u32; // 06071e48 sts macl,r4
    r3 <<= 8; // 06071e4a shll8 r3
    r3 <<= 16; // 06071e4c shll16 r3
    r4 >>= 8; // 06071e4e shlr8 r4
    r4 |= r3; // 06071e50 or r3,r4
    r5 = r5.wrapping_sub(r4); // 06071e52 sub r4,r5
    cpu.write32(r6.wrapping_add(44), r5); // 06071e54 mov.l r5,@(44,r6)
    r8 = r8.wrapping_neg(); // 06071e56 neg r8,r8
    r8 <<= 8; // 06071e58 shll8 r8
    cpu.write32(r6.wrapping_add(12), r8); // 06071e5a mov.l r8,@(12,r6)
    r3 = r1.rotate_left(16); // 06071e5c swap.w r1,r3
    r3 = r3 as i16 as i32 as u32; // 06071e5e exts.w r3,r3
    r1 <<= 16; // 06071e60 shll16 r1
    cpu.write32(r10.wrapping_add(0), r0); // 06071e62 mov.l r0,@(0,r10)
    cpu.write32(r10.wrapping_add(16), r3); // 06071e64 mov.l r3,@(16,r10)
    cpu.write32(r10.wrapping_add(20), r1); // 06071e66 mov.l r1,@(20,r10)
    r3 = r2.rotate_left(16); // 06071e68 swap.w r2,r3
    r3 = r3 as i16 as i32 as u32; // 06071e6a exts.w r3,r3
    r2 <<= 16; // 06071e6c shll16 r2
    r8 = cpu.read32(r10.wrapping_add(28)); // 06071e6e mov.l @(28,r10),r8
    cpu.write32(r10.wrapping_add(0), r0); // 06071e70 mov.l r0,@(0,r10)
    cpu.write32(r10.wrapping_add(16), r3); // 06071e72 mov.l r3,@(16,r10)
    cpu.write32(r10.wrapping_add(20), r2); // 06071e74 mov.l r2,@(20,r10)
    cpu.write32(r6.wrapping_add(16), r8); // 06071e76 mov.l r8,@(16,r6)
    r1 = 1i32 as u32; // 06071e78 mov #1,r1
    r3 = 0i32 as u32; // 06071e7a mov #0,r3
    r5 = cpu.read32(r10.wrapping_add(28)); // 06071e7c mov.l @(28,r10),r5
    cpu.write32(r10.wrapping_add(0), r0); // 06071e7e mov.l r0,@(0,r10)
    cpu.write32(r6.wrapping_add(32), r5); // 06071e80 mov.l r5,@(32,r6)
    cpu.write32(r10.wrapping_add(16), r1); // 06071e82 mov.l r1,@(16,r10)
    cpu.write32(r10.wrapping_add(20), r3); // 06071e84 mov.l r3,@(20,r10)
    r0 = cpu.read32(r6.wrapping_add(20)); // 06071e86 mov.l @(20,r6),r0
    r3 = 0i32 as u32; // 06071e88 mov #0,r3
    r4 = r0; // 06071e8a mov r0,r4
    t = r0 as i32 >= 0; // 06071e8c cmp/pz r0
    r2 = cpu.read32(r6.wrapping_add(36));
    if !t {
        r4 = r0.wrapping_neg();
    }
    t = r2 as i32 >= 0;
    r1 = r2;
    if !t {
        r1 = r2.wrapping_neg();
    }
    // the second pivot: the bigger of rows 1 and 2 in the second column
    t = r4 >= r1;
    r8 = cpu.read32(r6.wrapping_add(16));
    if !t {
        r3 = cpu.read32(r6.wrapping_add(28)); // 06071ea2 mov.l @(28,r6),r3
        r4 = cpu.read32(r6.wrapping_add(44)); // 06071ea4 mov.l @(44,r6),r4
        r5 = cpu.read32(r6.wrapping_add(24)); // 06071ea6 mov.l @(24,r6),r5
        r1 = cpu.read32(r6.wrapping_add(40)); // 06071ea8 mov.l @(40,r6),r1
        cpu.write32(r6.wrapping_add(44), r3); // 06071eaa mov.l r3,@(44,r6)
        cpu.write32(r6.wrapping_add(28), r4); // 06071eac mov.l r4,@(28,r6)
        cpu.write32(r6.wrapping_add(40), r5); // 06071eae mov.l r5,@(40,r6)
        cpu.write32(r6.wrapping_add(24), r1); // 06071eb0 mov.l r1,@(24,r6)
        r5 = cpu.read32(r6.wrapping_add(32)); // 06071eb2 mov.l @(32,r6),r5
        cpu.write32(r6.wrapping_add(32), r8); // 06071eb4 mov.l r8,@(32,r6)
        r8 = r5; // 06071eb6 mov r5,r8
        r3 = r2; // 06071eb8 mov r2,r3
        r2 = r0; // 06071eba mov r0,r2
        r0 = r3; // 06071ebc mov r3,r0
        r3 = 2i32 as u32; // 06071ebe mov #2,r3
    }
    // the second and third eliminations
    r1 = cpu.read32(r6.wrapping_add(4)); // 06071ec0 mov.l @(4,r6),r1
    r9 <<= 8; // 06071ec2 shll8 r9
    r9 |= r3; // 06071ec4 or r3,r9
    mac = r1 as i32 as i64 * r8 as i32 as i64; // 06071ec6 dmuls.l r1,r8
    r3 = cpu.read32(r10.wrapping_add(28)); // 06071ec8 mov.l @(28,r10),r3
    cpu.write32(r10.wrapping_add(0), r0); // 06071eca mov.l r0,@(0,r10)
    cpu.write32(r6.wrapping_add(0), r3); // 06071ecc mov.l r3,@(0,r6)
    r4 = (mac >> 32) as u32; // 06071ece sts mach,r4
    r5 = mac as u32; // 06071ed0 sts macl,r5
    cpu.write32(r10.wrapping_add(16), r4); // 06071ed2 mov.l r4,@(16,r10)
    cpu.write32(r10.wrapping_add(20), r5); // 06071ed4 mov.l r5,@(20,r10)
    r4 = cpu.read32(r10.wrapping_add(28)); // 06071ed6 mov.l @(28,r10),r4
    mac = r2 as i32 as i64 * r8 as i32 as i64; // 06071ed8 dmuls.l r2,r8
    r3 = r3.wrapping_sub(r4); // 06071eda sub r4,r3
    cpu.write32(r6.wrapping_add(0), r3); // 06071edc mov.l r3,@(0,r6)
    cpu.write32(r10.wrapping_add(0), r0); // 06071ede mov.l r0,@(0,r10)
    r4 = (mac >> 32) as u32; // 06071ee0 sts mach,r4
    r5 = mac as u32; // 06071ee2 sts macl,r5
    cpu.write32(r10.wrapping_add(16), r4); // 06071ee4 mov.l r4,@(16,r10)
    cpu.write32(r10.wrapping_add(20), r5); // 06071ee6 mov.l r5,@(20,r10)
    r3 = r8.rotate_left(16); // 06071ee8 swap.w r8,r3
    r3 = r3 as i16 as i32 as u32; // 06071eea exts.w r3,r3
    r8 <<= 16; // 06071eec shll16 r8
    r5 = cpu.read32(r6.wrapping_add(32)); // 06071eee mov.l @(32,r6),r5
    r4 = cpu.read32(r10.wrapping_add(28)); // 06071ef0 mov.l @(28,r10),r4
    cpu.write32(r10.wrapping_add(0), r0); // 06071ef2 mov.l r0,@(0,r10)
    cpu.write32(r10.wrapping_add(16), r3); // 06071ef4 mov.l r3,@(16,r10)
    cpu.write32(r10.wrapping_add(20), r8); // 06071ef6 mov.l r8,@(20,r10)
    r5 = r5.wrapping_sub(r4); // 06071ef8 sub r4,r5
    cpu.write32(r6.wrapping_add(32), r5); // 06071efa mov.l r5,@(32,r6)
    r8 = cpu.read32(r6.wrapping_add(24)); // 06071efc mov.l @(24,r6),r8
    r3 = cpu.read32(r10.wrapping_add(28)); // 06071efe mov.l @(28,r10),r3
    mac = r1 as i32 as i64 * r8 as i32 as i64; // 06071f00 dmuls.l r1,r8
    r3 = r3.wrapping_neg(); // 06071f02 neg r3,r3
    cpu.write32(r6.wrapping_add(16), r3); // 06071f04 mov.l r3,@(16,r6)
    cpu.write32(r10.wrapping_add(0), r0); // 06071f06 mov.l r0,@(0,r10)
    r4 = (mac >> 32) as u32; // 06071f08 sts mach,r4
    r5 = mac as u32; // 06071f0a sts macl,r5
    cpu.write32(r10.wrapping_add(16), r4); // 06071f0c mov.l r4,@(16,r10)
    cpu.write32(r10.wrapping_add(20), r5); // 06071f0e mov.l r5,@(20,r10)
    r5 = cpu.read32(r6.wrapping_add(8)); // 06071f10 mov.l @(8,r6),r5
    r4 = cpu.read32(r10.wrapping_add(28)); // 06071f12 mov.l @(28,r10),r4
    mac = r2 as i32 as i64 * r8 as i32 as i64; // 06071f14 dmuls.l r2,r8
    r5 = r5.wrapping_sub(r4); // 06071f16 sub r4,r5
    cpu.write32(r6.wrapping_add(8), r5); // 06071f18 mov.l r5,@(8,r6)
    cpu.write32(r10.wrapping_add(0), r0); // 06071f1a mov.l r0,@(0,r10)
    r4 = (mac >> 32) as u32; // 06071f1c sts mach,r4
    r5 = mac as u32; // 06071f1e sts macl,r5
    cpu.write32(r10.wrapping_add(16), r4); // 06071f20 mov.l r4,@(16,r10)
    cpu.write32(r10.wrapping_add(20), r5); // 06071f22 mov.l r5,@(20,r10)
    r3 = r8.rotate_left(16); // 06071f24 swap.w r8,r3
    r3 = r3 as i16 as i32 as u32; // 06071f26 exts.w r3,r3
    r8 <<= 16; // 06071f28 shll16 r8
    r5 = cpu.read32(r6.wrapping_add(40)); // 06071f2a mov.l @(40,r6),r5
    r4 = cpu.read32(r10.wrapping_add(28)); // 06071f2c mov.l @(28,r10),r4
    cpu.write32(r10.wrapping_add(0), r0); // 06071f2e mov.l r0,@(0,r10)
    cpu.write32(r10.wrapping_add(16), r3); // 06071f30 mov.l r3,@(16,r10)
    cpu.write32(r10.wrapping_add(20), r8); // 06071f32 mov.l r8,@(20,r10)
    r5 = r5.wrapping_sub(r4); // 06071f34 sub r4,r5
    cpu.write32(r6.wrapping_add(40), r5); // 06071f36 mov.l r5,@(40,r6)
    r8 = cpu.read32(r6.wrapping_add(28)); // 06071f38 mov.l @(28,r6),r8
    r3 = cpu.read32(r10.wrapping_add(28)); // 06071f3a mov.l @(28,r10),r3
    r4 = r8.rotate_left(16); // 06071f3c swap.w r8,r4
    r4 = (r4 & 0xffff_0000) | ((r4 & 0xff) << 8) | ((r4 >> 8) & 0xff); // 06071f3e swap.b r4,r4
    r4 = r4 as i8 as i32 as u32; // 06071f40 exts.b r4,r4
    r8 <<= 8; // 06071f42 shll8 r8
    cpu.write32(r10.wrapping_add(0), r0); // 06071f44 mov.l r0,@(0,r10)
    cpu.write32(r10.wrapping_add(16), r4); // 06071f46 mov.l r4,@(16,r10)
    cpu.write32(r10.wrapping_add(20), r8); // 06071f48 mov.l r8,@(20,r10)
    r3 = r3.wrapping_neg(); // 06071f4a neg r3,r3
    cpu.write32(r6.wrapping_add(24), r3); // 06071f4c mov.l r3,@(24,r6)
    r8 = cpu.read32(r10.wrapping_add(28)); // 06071f4e mov.l @(28,r10),r8
    mac = r1 as i32 as i64 * r8 as i32 as i64; // 06071f50 dmuls.l r1,r8
    r5 = cpu.read32(r6.wrapping_add(12)); // 06071f52 mov.l @(12,r6),r5
    r3 = (mac >> 32) as u32; // 06071f54 sts mach,r3
    r4 = mac as u32; // 06071f56 sts macl,r4
    r3 <<= 8; // 06071f58 shll8 r3
    r3 <<= 16; // 06071f5a shll16 r3
    r4 >>= 8; // 06071f5c shlr8 r4
    r4 |= r3; // 06071f5e or r3,r4
    r5 = r5.wrapping_sub(r4); // 06071f60 sub r4,r5
    cpu.write32(r6.wrapping_add(12), r5); // 06071f62 mov.l r5,@(12,r6)
    mac = r2 as i32 as i64 * r8 as i32 as i64; // 06071f64 dmuls.l r2,r8
    r5 = cpu.read32(r6.wrapping_add(44)); // 06071f66 mov.l @(44,r6),r5
    r3 = (mac >> 32) as u32; // 06071f68 sts mach,r3
    r4 = mac as u32; // 06071f6a sts macl,r4
    r3 <<= 8; // 06071f6c shll8 r3
    r3 <<= 16; // 06071f6e shll16 r3
    r4 >>= 8; // 06071f70 shlr8 r4
    r4 |= r3; // 06071f72 or r3,r4
    r5 = r5.wrapping_sub(r4); // 06071f74 sub r4,r5
    cpu.write32(r6.wrapping_add(44), r5); // 06071f76 mov.l r5,@(44,r6)
    cpu.write32(r10.wrapping_add(0), r0); // 06071f78 mov.l r0,@(0,r10)
    r3 = r1.rotate_left(16); // 06071f7a swap.w r1,r3
    r3 = r3 as i16 as i32 as u32; // 06071f7c exts.w r3,r3
    cpu.write32(r10.wrapping_add(16), r3); // 06071f7e mov.l r3,@(16,r10)
    r1 <<= 16; // 06071f80 shll16 r1
    cpu.write32(r10.wrapping_add(20), r1); // 06071f82 mov.l r1,@(20,r10)
    r8 = r8.wrapping_neg(); // 06071f84 neg r8,r8
    r8 <<= 8; // 06071f86 shll8 r8
    cpu.write32(r6.wrapping_add(28), r8); // 06071f88 mov.l r8,@(28,r6)
    r1 = cpu.read32(r10.wrapping_add(28)); // 06071f8a mov.l @(28,r10),r1
    cpu.write32(r10.wrapping_add(0), r0); // 06071f8c mov.l r0,@(0,r10)
    cpu.write32(r6.wrapping_add(4), r1); // 06071f8e mov.l r1,@(4,r6)
    r3 = r2.rotate_left(16); // 06071f90 swap.w r2,r3
    r3 = r3 as i16 as i32 as u32; // 06071f92 exts.w r3,r3
    cpu.write32(r10.wrapping_add(16), r3); // 06071f94 mov.l r3,@(16,r10)
    r2 <<= 16; // 06071f96 shll16 r2
    cpu.write32(r10.wrapping_add(20), r2); // 06071f98 mov.l r2,@(20,r10)
    r1 = 1i32 as u32; // 06071f9a mov #1,r1
    r3 = 0i32 as u32; // 06071f9c mov #0,r3
    r2 = cpu.read32(r10.wrapping_add(28)); // 06071f9e mov.l @(28,r10),r2
    cpu.write32(r10.wrapping_add(0), r0); // 06071fa0 mov.l r0,@(0,r10)
    cpu.write32(r10.wrapping_add(16), r1); // 06071fa2 mov.l r1,@(16,r10)
    cpu.write32(r10.wrapping_add(20), r3); // 06071fa4 mov.l r3,@(20,r10)
    cpu.write32(r6.wrapping_add(36), r2); // 06071fa6 mov.l r2,@(36,r6)
    r0 = cpu.read32(r6.wrapping_add(40)); // 06071fa8 mov.l @(40,r6),r0
    r8 = cpu.read32(r6.wrapping_add(32)); // 06071faa mov.l @(32,r6),r8
    r1 = cpu.read32(r6.wrapping_add(8)); // 06071fac mov.l @(8,r6),r1
    r2 = cpu.read32(r6.wrapping_add(24)); // 06071fae mov.l @(24,r6),r2
    mac = r1 as i32 as i64 * r8 as i32 as i64; // 06071fb0 dmuls.l r1,r8
    r3 = cpu.read32(r10.wrapping_add(28)); // 06071fb2 mov.l @(28,r10),r3
    cpu.write32(r6.wrapping_add(20), r3); // 06071fb4 mov.l r3,@(20,r6)
    cpu.write32(r10.wrapping_add(0), r0); // 06071fb6 mov.l r0,@(0,r10)
    r4 = (mac >> 32) as u32; // 06071fb8 sts mach,r4
    r5 = mac as u32; // 06071fba sts macl,r5
    cpu.write32(r10.wrapping_add(16), r4); // 06071fbc mov.l r4,@(16,r10)
    cpu.write32(r10.wrapping_add(20), r5); // 06071fbe mov.l r5,@(20,r10)
    r5 = cpu.read32(r6.wrapping_add(0)); // 06071fc0 mov.l @(0,r6),r5
    r4 = cpu.read32(r10.wrapping_add(28)); // 06071fc2 mov.l @(28,r10),r4
    mac = r2 as i32 as i64 * r8 as i32 as i64; // 06071fc4 dmuls.l r2,r8
    r5 = r5.wrapping_sub(r4); // 06071fc6 sub r4,r5
    cpu.write32(r6.wrapping_add(0), r5); // 06071fc8 mov.l r5,@(0,r6)
    cpu.write32(r10.wrapping_add(0), r0); // 06071fca mov.l r0,@(0,r10)
    r4 = (mac >> 32) as u32; // 06071fcc sts mach,r4
    r5 = mac as u32; // 06071fce sts macl,r5
    cpu.write32(r10.wrapping_add(16), r4); // 06071fd0 mov.l r4,@(16,r10)
    cpu.write32(r10.wrapping_add(20), r5); // 06071fd2 mov.l r5,@(20,r10)
    r3 = r8.rotate_left(16); // 06071fd4 swap.w r8,r3
    r3 = r3 as i16 as i32 as u32; // 06071fd6 exts.w r3,r3
    r8 <<= 16; // 06071fd8 shll16 r8
    r5 = cpu.read32(r6.wrapping_add(16)); // 06071fda mov.l @(16,r6),r5
    r4 = cpu.read32(r10.wrapping_add(28)); // 06071fdc mov.l @(28,r10),r4
    cpu.write32(r10.wrapping_add(0), r0); // 06071fde mov.l r0,@(0,r10)
    cpu.write32(r10.wrapping_add(16), r3); // 06071fe0 mov.l r3,@(16,r10)
    cpu.write32(r10.wrapping_add(20), r8); // 06071fe2 mov.l r8,@(20,r10)
    r5 = r5.wrapping_sub(r4); // 06071fe4 sub r4,r5
    cpu.write32(r6.wrapping_add(16), r5); // 06071fe6 mov.l r5,@(16,r6)
    r8 = cpu.read32(r6.wrapping_add(36)); // 06071fe8 mov.l @(36,r6),r8
    r3 = cpu.read32(r10.wrapping_add(28)); // 06071fea mov.l @(28,r10),r3
    mac = r1 as i32 as i64 * r8 as i32 as i64; // 06071fec dmuls.l r1,r8
    r3 = r3.wrapping_neg(); // 06071fee neg r3,r3
    cpu.write32(r6.wrapping_add(32), r3); // 06071ff0 mov.l r3,@(32,r6)
    cpu.write32(r10.wrapping_add(0), r0); // 06071ff2 mov.l r0,@(0,r10)
    r4 = (mac >> 32) as u32; // 06071ff4 sts mach,r4
    r5 = mac as u32; // 06071ff6 sts macl,r5
    cpu.write32(r10.wrapping_add(16), r4); // 06071ff8 mov.l r4,@(16,r10)
    cpu.write32(r10.wrapping_add(20), r5); // 06071ffa mov.l r5,@(20,r10)
    r5 = cpu.read32(r6.wrapping_add(4)); // 06071ffc mov.l @(4,r6),r5
    r4 = cpu.read32(r10.wrapping_add(28)); // 06071ffe mov.l @(28,r10),r4
    mac = r2 as i32 as i64 * r8 as i32 as i64; // 06072000 dmuls.l r2,r8
    r5 = r5.wrapping_sub(r4); // 06072002 sub r4,r5
    cpu.write32(r6.wrapping_add(4), r5); // 06072004 mov.l r5,@(4,r6)
    cpu.write32(r10.wrapping_add(0), r0); // 06072006 mov.l r0,@(0,r10)
    r4 = (mac >> 32) as u32; // 06072008 sts mach,r4
    r5 = mac as u32; // 0607200a sts macl,r5
    cpu.write32(r10.wrapping_add(16), r4); // 0607200c mov.l r4,@(16,r10)
    cpu.write32(r10.wrapping_add(20), r5); // 0607200e mov.l r5,@(20,r10)
    r3 = r8.rotate_left(16); // 06072010 swap.w r8,r3
    r3 = r3 as i16 as i32 as u32; // 06072012 exts.w r3,r3
    r8 <<= 16; // 06072014 shll16 r8
    r5 = cpu.read32(r6.wrapping_add(20)); // 06072016 mov.l @(20,r6),r5
    r4 = cpu.read32(r10.wrapping_add(28)); // 06072018 mov.l @(28,r10),r4
    cpu.write32(r10.wrapping_add(0), r0); // 0607201a mov.l r0,@(0,r10)
    cpu.write32(r10.wrapping_add(16), r3); // 0607201c mov.l r3,@(16,r10)
    cpu.write32(r10.wrapping_add(20), r8); // 0607201e mov.l r8,@(20,r10)
    r5 = r5.wrapping_sub(r4); // 06072020 sub r4,r5
    cpu.write32(r6.wrapping_add(20), r5); // 06072022 mov.l r5,@(20,r6)
    r8 = cpu.read32(r6.wrapping_add(44)); // 06072024 mov.l @(44,r6),r8
    r3 = cpu.read32(r10.wrapping_add(28)); // 06072026 mov.l @(28,r10),r3
    r4 = r8.rotate_left(16); // 06072028 swap.w r8,r4
    r4 = (r4 & 0xffff_0000) | ((r4 & 0xff) << 8) | ((r4 >> 8) & 0xff); // 0607202a swap.b r4,r4
    r4 = r4 as i8 as i32 as u32; // 0607202c exts.b r4,r4
    r8 <<= 8; // 0607202e shll8 r8
    cpu.write32(r10.wrapping_add(0), r0); // 06072030 mov.l r0,@(0,r10)
    cpu.write32(r10.wrapping_add(16), r4); // 06072032 mov.l r4,@(16,r10)
    cpu.write32(r10.wrapping_add(20), r8); // 06072034 mov.l r8,@(20,r10)
    r3 = r3.wrapping_neg(); // 06072036 neg r3,r3
    cpu.write32(r6.wrapping_add(36), r3); // 06072038 mov.l r3,@(36,r6)
    r8 = cpu.read32(r10.wrapping_add(28)); // 0607203a mov.l @(28,r10),r8
    mac = r1 as i32 as i64 * r8 as i32 as i64; // 0607203c dmuls.l r1,r8
    r5 = cpu.read32(r6.wrapping_add(12)); // 0607203e mov.l @(12,r6),r5
    r3 = (mac >> 32) as u32; // 06072040 sts mach,r3
    r4 = mac as u32; // 06072042 sts macl,r4
    r3 <<= 8; // 06072044 shll8 r3
    r3 <<= 16; // 06072046 shll16 r3
    r4 >>= 8; // 06072048 shlr8 r4
    r4 |= r3; // 0607204a or r3,r4
    r5 = r5.wrapping_sub(r4); // 0607204c sub r4,r5
    mac = r2 as i32 as i64 * r8 as i32 as i64; // 0607204e dmuls.l r2,r8
    cpu.write32(r6.wrapping_add(12), r5); // 06072050 mov.l r5,@(12,r6)
    r5 = cpu.read32(r6.wrapping_add(28)); // 06072052 mov.l @(28,r6),r5
    r3 = (mac >> 32) as u32; // 06072054 sts mach,r3
    r4 = mac as u32; // 06072056 sts macl,r4
    r3 <<= 8; // 06072058 shll8 r3
    r3 <<= 16; // 0607205a shll16 r3
    r4 >>= 8; // 0607205c shlr8 r4
    r4 |= r3; // 0607205e or r3,r4
    r5 = r5.wrapping_sub(r4); // 06072060 sub r4,r5
    cpu.write32(r6.wrapping_add(28), r5); // 06072062 mov.l r5,@(28,r6)
    r3 = r1.rotate_left(16); // 06072064 swap.w r1,r3
    r3 = r3 as i16 as i32 as u32; // 06072066 exts.w r3,r3
    r1 <<= 16; // 06072068 shll16 r1
    cpu.write32(r10.wrapping_add(0), r0); // 0607206a mov.l r0,@(0,r10)
    cpu.write32(r10.wrapping_add(16), r3); // 0607206c mov.l r3,@(16,r10)
    cpu.write32(r10.wrapping_add(20), r1); // 0607206e mov.l r1,@(20,r10)
    r8 = r8.wrapping_neg(); // 06072070 neg r8,r8
    r8 <<= 8; // 06072072 shll8 r8
    cpu.write32(r6.wrapping_add(44), r8); // 06072074 mov.l r8,@(44,r6)
    r3 = r2.rotate_left(16); // 06072076 swap.w r2,r3
    r3 = r3 as i16 as i32 as u32; // 06072078 exts.w r3,r3
    r2 <<= 16; // 0607207a shll16 r2
    r1 = cpu.read32(r10.wrapping_add(28)); // 0607207c mov.l @(28,r10),r1
    cpu.write32(r10.wrapping_add(0), r0); // 0607207e mov.l r0,@(0,r10)
    cpu.write32(r10.wrapping_add(16), r3); // 06072080 mov.l r3,@(16,r10)
    cpu.write32(r10.wrapping_add(20), r2); // 06072082 mov.l r2,@(20,r10)
    cpu.write32(r6.wrapping_add(8), r1); // 06072084 mov.l r1,@(8,r6)
    r1 = 1i32 as u32; // 06072086 mov #1,r1
    r3 = 0i32 as u32; // 06072088 mov #0,r3
    r2 = cpu.read32(r10.wrapping_add(28)); // 0607208a mov.l @(28,r10),r2
    cpu.write32(r10.wrapping_add(0), r0); // 0607208c mov.l r0,@(0,r10)
    cpu.write32(r10.wrapping_add(16), r1); // 0607208e mov.l r1,@(16,r10)
    cpu.write32(r10.wrapping_add(20), r3); // 06072090 mov.l r3,@(20,r10)
    cpu.write32(r6.wrapping_add(24), r2); // 06072092 mov.l r2,@(24,r6)
    r0 = r9 & 0xff; // 06072094 extu.b r9,r0
    t = r0 & r0 == 0; // 06072096 tst r0,r0
    r8 = cpu.read32(r10.wrapping_add(28));
    // the swaps undone, by the codes kept in r9
    if !t {
        r0 = cpu.read32(r6.wrapping_add(4)); // 0607209c mov.l @(4,r6),r0
        r1 = cpu.read32(r6.wrapping_add(8)); // 0607209e mov.l @(8,r6),r1
        r2 = cpu.read32(r6.wrapping_add(20)); // 060720a0 mov.l @(20,r6),r2
        r3 = cpu.read32(r6.wrapping_add(24)); // 060720a2 mov.l @(24,r6),r3
        r4 = cpu.read32(r6.wrapping_add(36)); // 060720a4 mov.l @(36,r6),r4
        cpu.write32(r6.wrapping_add(8), r0); // 060720a6 mov.l r0,@(8,r6)
        cpu.write32(r6.wrapping_add(4), r1); // 060720a8 mov.l r1,@(4,r6)
        cpu.write32(r6.wrapping_add(24), r2); // 060720aa mov.l r2,@(24,r6)
        cpu.write32(r6.wrapping_add(20), r3); // 060720ac mov.l r3,@(20,r6)
        cpu.write32(r6.wrapping_add(36), r8); // 060720ae mov.l r8,@(36,r6)
        r8 = r4; // 060720b0 mov r4,r8
    }
    r9 >>= 8;
    t = r9 == 0;
    r9 = r9.wrapping_sub(1);
    if !t {
        r0 = cpu.read32(r6);
        r1 = cpu.read32(r6.wrapping_add(16));
        t = r9 == 0;
        r2 = cpu.read32(r6.wrapping_add(32));
        if !t {
            r3 = cpu.read32(r6.wrapping_add(8)); // 060720c4 mov.l @(8,r6),r3
            r4 = cpu.read32(r6.wrapping_add(24)); // 060720c6 mov.l @(24,r6),r4
            r5 = r8; // 060720c8 mov r8,r5
            cpu.write32(r6.wrapping_add(8), r0); // 060720ca mov.l r0,@(8,r6)
            cpu.write32(r6.wrapping_add(24), r1); // 060720cc mov.l r1,@(24,r6)
            r8 = r2;
        } else {
            r3 = cpu.read32(r6.wrapping_add(4)); // 060720d2 mov.l @(4,r6),r3
            r4 = cpu.read32(r6.wrapping_add(20)); // 060720d4 mov.l @(20,r6),r4
            r5 = cpu.read32(r6.wrapping_add(36)); // 060720d6 mov.l @(36,r6),r5
            cpu.write32(r6.wrapping_add(4), r0); // 060720d8 mov.l r0,@(4,r6)
            cpu.write32(r6.wrapping_add(20), r1); // 060720da mov.l r1,@(20,r6)
            cpu.write32(r6.wrapping_add(36), r2); // 060720dc mov.l r2,@(36,r6)
        }
        cpu.write32(r6.wrapping_add(0), r3); // 060720de mov.l r3,@(0,r6)
        cpu.write32(r6.wrapping_add(16), r4); // 060720e0 mov.l r4,@(16,r6)
        cpu.write32(r6.wrapping_add(32), r5); // 060720e2 mov.l r5,@(32,r6)
    }
    cpu.write32(r6.wrapping_add(40), r8);
    let _ = (r9, r10);
    cpu.leave(&[(0, r0), (1, r1), (2, r2), (3, r3), (4, r4), (5, r5), (6, r6), (7, r7)]);
    cpu.leave_mac(mac);
    cpu.leave_t(t);
}

/// slInversMatrix: the current matrix inverted (sl_inverse), its third column negated.
pub fn sl_invers_matrix(cpu: &mut Cpu) {
    nested!(cpu, 0x0607_1d2c, sl_inverse(cpu));
    let m = matrix(cpu);
    let v = [8, 24, 40].map(|k| cpu.read32(m + k).wrapping_neg());
    for (k, x) in [8, 24, 40].into_iter().zip(v) {
        cpu.write32(m + k, x);
    }
    cpu.leave(&[(0, m), (1, v[0]), (2, v[1]), (3, v[2])]);
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

/// slConvert: the point at `p` (x, y, z) through the current matrix and onto the screen: its
/// depth, unless nearer than the window's depth over 2 to the window's shift (then 0 and nothing
/// written); its screen x and y (the depth over its z, on the divide unit, times its x and y) to
/// `out` (16.16).
const WIN_SHIFT: u32 = WIN_FLAG;

pub fn sl_convert(cpu: &mut Cpu, p: u32, out: u32) -> u32 {
    convert(cpu, p, out, false)
}

/// The same with the screen place in whole pixels (the products' high halves).
pub fn sl_convert_pixels(cpu: &mut Cpu, p: u32, out: u32) -> u32 {
    convert(cpu, p, out, true)
}

fn convert(cpu: &mut Cpu, p: u32, out: u32, pixels: bool) -> u32 {
    let w = cpu.gbr();
    let m = matrix(cpu);
    let (px, py, pz) = (cpu.read32(p), cpu.read32(p + 4), cpu.read32(p + 8));
    let row_sum = |cpu: &mut Cpu, r: u32| {
        let mut sum = 0;
        for (k, v) in [px, py, pz].into_iter().enumerate() {
            let e = cpu.read32(m + 16 * r + 4 * k as u32);
            sum = mac(cpu, sum, v, e);
        }
        sum
    };
    let sum_z = row_sum(cpu, 2);
    let depth = cpu.read32(w + WIN_DEPTH);
    let shift = cpu.read8(w + WIN_SHIFT) as i8 as i32 as u32;
    let near = if shift == 0 || shift >= 32 { 0 } else { depth >> shift };
    let m23 = cpu.read32(m + 44);
    let z = middle(sum_z).wrapping_add(m23);
    if (z as i32) < near as i32 {
        cpu.leave(&[(1, m23), (2, (sum_z >> 32) as u32), (4, p + 12), (6, m + 44), (7, z)]);
        cpu.leave_mac(sum_z);
        cpu.leave_t(false);
        return 0;
    }
    cpu.write32(DVSR, z);
    cpu.write32(DVDNTH, (depth >> 16) as u16 as i16 as i32 as u32);
    cpu.write32(DVDNTL, depth << 16);
    let sum_y = row_sum(cpu, 1);
    let y = middle(sum_y).wrapping_add(cpu.read32(m + 28));
    let sum_x = row_sum(cpu, 0);
    let m03 = cpu.read32(m + 12);
    let x = middle(sum_x).wrapping_add(m03);
    let scale = cpu.read32(DVDNTL_AGAIN);
    let (on_y, on_x) = (product(scale, y), product(scale, x));
    let place = |v: i64| if pixels { (v >> 32) as u32 } else { middle(v) };
    cpu.write32(out + 4, place(on_y));
    cpu.write32(out, place(on_x));
    if pixels {
        cpu.leave(&[(1, m03), (2, (sum_x >> 32) as u32), (3, y), (4, x), (5, out), (6, m + 12), (7, z)]);
    } else {
        cpu.leave(&[(1, m03), (2, (on_x >> 32) as u32), (3, middle(on_x)), (4, x), (5, out), (6, m + 12), (7, z)]);
    }
    cpu.leave_mac(on_x);
    cpu.leave_t(true);
    z
}
