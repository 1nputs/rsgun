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
/// depth past the near, and the near; or none, the registers left as the SH-2 code's check
/// (0x0607170c) leaves them, which then returns 0 from the put (it pops the put's PR).
const FAR: u32 = 0x70;

fn put_check(cpu: &mut Cpu, z: u32) -> Option<(u32, u32)> {
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
    Some((past, near))
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
    let Some((past, near)) = put_check(cpu, z) else {
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
    let Some((past, _)) = put_check(cpu, z) else {
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
